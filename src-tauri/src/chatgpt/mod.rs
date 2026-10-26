mod authentication;
mod capabilities;
mod codex_transport;
mod models;
mod response;
use crate::{persistence::Provider, AppState};
use codex_transport::CodexTransport;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{ipc::Channel, Manager, State};
use tokio_util::sync::CancellationToken;

pub struct ChatGPTProvider {
    root: PathBuf,
    connections: tokio::sync::Mutex<HashMap<String, (String, Arc<CodexTransport>)>>,
    replies: Mutex<HashMap<String, tokio::sync::oneshot::Sender<(String, bool)>>>,
}
impl ChatGPTProvider {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            connections: Default::default(),
            replies: Default::default(),
        }
    }
    async fn transport(&self, p: &Provider) -> Result<Arc<CodexTransport>, String> {
        crate::providers::validate(p)?;
        let config = &p.request["chatgpt"];
        let binary = binary(config["executable"].as_str())?;
        let fingerprint = binary.display().to_string();
        let mut connections = self.connections.lock().await;
        if let Some((key, transport)) = connections.get(&p.id) {
            if key == &fingerprint && transport.alive().await {
                return Ok(transport.clone());
            }
            transport.stop().await;
        }
        let home = self.root.join(&p.id).join("codex-home");
        let transport =
            CodexTransport::start(&binary, &home, &self.root.join(&p.id).join("runtime")).await?;
        connections.insert(p.id.clone(), (fingerprint, transport.clone()));
        Ok(transport)
    }
    async fn disconnect(&self, p: &Provider) -> Result<(), String> {
        let transport = self.transport(p).await?;
        transport.rpc("account/logout", json!({})).await?;
        transport.stop().await;
        self.connections.lock().await.remove(&p.id);
        Ok(())
    }
}
pub async fn remove(app: &tauri::AppHandle, p: &Provider) -> Result<(), String> {
    let manager = app.state::<ChatGPTProvider>();
    if manager.root.join(&p.id).join("codex-home").exists() {
        let mut own = p.clone();
        if let Some(config) = own
            .request
            .get_mut("chatgpt")
            .and_then(Value::as_object_mut)
        {
            config.remove("mode");
            config.remove("authSource");
        }
        manager.disconnect(&own).await?;
    }
    Ok(())
}

fn binary(override_path: Option<&str>) -> Result<PathBuf, String> {
    if let Some(value) = override_path.filter(|s| !s.trim().is_empty()) {
        let path = PathBuf::from(value);
        if !path.is_absolute() || !path.is_file() {
            return Err("Choisissez un chemin absolu vers l’exécutable Codex CLI".into());
        }
        return Ok(path);
    }
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let path = PathBuf::from(local).join("Programs/OpenAI/Codex/bin/codex.exe");
        if path.is_file() {
            return Ok(path);
        }
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let path = dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        if path.is_file() {
            return Ok(path);
        }
    }
    Err("Codex CLI n’est pas installé. Installez-le depuis la documentation officielle ou choisissez son exécutable.".into())
}
#[tauri::command]
pub async fn chatgpt_action(
    state: State<'_, ChatGPTProvider>,
    provider: Provider,
    action: String,
    login_id: Option<String>,
) -> Result<Value, String> {
    crate::providers::validate(&provider)?;
    if provider.plugin != "chatgpt" {
        return Err("Ce fournisseur n’est pas ChatGPT".into());
    }
    match action.as_str() {
        "logout" => {
            state.disconnect(&provider).await?;
            return Ok(json!({"account":null,"models":[]}));
        }
        _ => {}
    }
    let transport = state.transport(&provider).await?;
    match action.as_str() {
        "login" => authentication::login(&transport).await,
        "cancelLogin" => {
            transport
                .rpc(
                    "account/login/cancel",
                    json!({"loginId":login_id.ok_or("Connexion manquante")?}),
                )
                .await
        }
        "status" | "diagnose" => {
            let account = authentication::account(&transport, action == "diagnose").await?;
            let models = if account.is_null() {
                vec![]
            } else {
                models::discover(&transport).await?
            };
            let limits = if account.is_null() {
                Value::Null
            } else {
                transport
                    .rpc("account/rateLimits/read", json!({}))
                    .await
                    .unwrap_or(Value::Null)
            };
            Ok(
                json!({"account":account,"capabilities":capabilities::detected(&models),"models":models,"limits":limits,"transport":"Codex App Server · stdio"}),
            )
        }
        _ => Err("Action ChatGPT inconnue".into()),
    }
}
#[tauri::command]
pub fn resolve_codex_tool(
    state: State<ChatGPTProvider>,
    call_id: String,
    result: String,
    success: bool,
) -> Result<(), String> {
    if result.len() > 16 * 1024 * 1024 {
        return Err("Résultat d’outil trop volumineux".into());
    }
    state
        .replies
        .lock()
        .map_err(|_| "Transport occupé")?
        .remove(&call_id)
        .ok_or("Appel Codex expiré ou déjà résolu")?
        .send((result, success))
        .map_err(|_| "Tour Codex terminé".into())
}
pub async fn discover(app: &tauri::AppHandle, p: &Provider) -> Result<Vec<String>, String> {
    let state = app.state::<ChatGPTProvider>();
    let transport = state.transport(p).await?;
    if authentication::account(&transport, false).await?.is_null() {
        return Err("Connectez votre compte ChatGPT dans le fournisseur".into());
    }
    Ok(models::discover(&transport)
        .await?
        .iter()
        .filter_map(|v| v["id"].as_str().map(|s| format!("codex/{s}")))
        .collect())
}

/// Converge owns durable history, editing and branches. Each request replays only the selected
/// context in an ephemeral Codex thread, avoiding hidden remote history or duplicate tool execution.
pub async fn stream(
    app: &tauri::AppHandle,
    state: &AppState,
    p: &Provider,
    request: &crate::providers::Request,
    channel: &Channel<Value>,
    cancel: CancellationToken,
) -> Result<(), String> {
    let manager = app.state::<ChatGPTProvider>();
    let transport = manager.transport(p).await?;
    if authentication::account(&transport, true).await?.is_null() {
        return Err("Connectez votre compte ChatGPT pour utiliser Codex".into());
    }
    let model = request
        .model
        .strip_prefix("codex/")
        .ok_or("Identifiant de modèle Codex invalide. Actualisez les modèles du fournisseur.")?;
    let catalog = models::discover(&transport).await?;
    let selected = catalog
        .iter()
        .find(|m| m["id"] == model)
        .ok_or("Modèle absent du catalogue Codex ; actualisez le fournisseur")?;
    let effort = request.reasoning_effort.as_deref().unwrap_or("auto");
    if effort != "auto"
        && !selected["efforts"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v == effort))
    {
        return Err("Effort non proposé par ce modèle Codex. Choisissez Auto.".into());
    }
    let (instructions, input) = context(&request.messages, selected["vision"] == true)?;
    let mut events = transport.events.subscribe();
    let tools:Vec<_>=crate::tools::schemas().into_iter().filter(|(name,_,_)|request.tools_enabled!=Some(false) && p.capabilities["tools"]==true && crate::tools::check_enabled(state,&name.replace('_',".")).is_ok()).map(|(name,description,input_schema)|json!({"type":"function","name":name,"description":description,"inputSchema":input_schema})).collect();
    let start=transport.rpc("thread/start",json!({
        "model":model,"modelProvider":"openai","cwd":transport.cwd,"sandbox":"read-only","approvalPolicy":"never","ephemeral":true,"allowProviderModelFallback":false,
        "baseInstructions":"You are the assistant in Converge. Answer naturally in the user's language. Use Converge dynamic tools for filesystem, terminal, web and computer actions. Native image_generation is allowed when available: its image results are displayed by Converge automatically. Never use shell, apply_patch, MCP, subagents or other built-in tools. All local tool permissions and workspace access are controlled by Converge. Stop when the requested task is complete.",
        "developerInstructions":instructions,"dynamicTools":tools,"environments":[],"config":{"web_search":"disabled","features.image_generation":request.tools_enabled!=Some(false)}
    })).await?;
    let thread = start["thread"]["id"]
        .as_str()
        .ok_or("Thread Codex manquant")?
        .to_string();
    let mut active_turn = None;
    let result=async {
        let turn=transport.rpc("turn/start",json!({"threadId":thread,"input":input,"effort":if effort=="auto" {Value::Null}else{json!(effort)}})).await?;
        let turn_id=turn["turn"]["id"].as_str().ok_or("Tour Codex manquant")?.to_string();
        active_turn=Some(turn_id.clone());
        let mut steps=0u32;
        loop {
            let event=tokio::select!{
                _=cancel.cancelled()=>{let _=transport.rpc("turn/interrupt",json!({"threadId":thread,"turnId":turn_id})).await;return Ok(());},
                event=tokio::time::timeout(std::time::Duration::from_secs(300),events.recv())=>event.map_err(|_|"Codex ne répond plus depuis cinq minutes")?.map_err(|_|"Flux Codex perdu. La réponse partielle a été conservée.")?
            };
            let method=event["method"].as_str().unwrap_or("");
            if method=="converge/disconnected" {return Err("Connexion Codex interrompue ; reconnectez le fournisseur".into());}
            let params=&event["params"];
            if params["threadId"]!=thread {continue;}
            if let Some(normalized)=response::normalize(&event){channel.send(normalized).map_err(|_|"Interface fermée")?;}
            match method {
                "item/started" if params["item"]["type"]=="imageGeneration"=>{channel.send(json!({"type":"execution","text":"Création de l’image…"})).map_err(|_|"Interface fermée")?;},
                "item/completed" if params["item"]["type"]=="imageGeneration"=>{
                    let root=transport.cwd.parent().ok_or("Dossier privé Codex indisponible")?;
                    let image=response::generated_image(&params["item"],root).await?;
                    channel.send(image).map_err(|_|"Interface fermée")?;
                },
                "item/tool/call"=>{
                    steps+=1;
                    if steps>48 {let _=transport.send(json!({"id":event["id"],"result":response::tool_output("Limite d’actions atteinte. Terminez votre réponse.",false)})).await;return Err("La limite de 48 actions Codex est atteinte. Envoyez un message pour continuer.".into());}
                    let name=params["tool"].as_str().ok_or("Outil Codex manquant")?;
                    if !crate::tools::schemas().iter().any(|(n,_,_)|n==name){transport.send(json!({"id":event["id"],"result":response::tool_output("Outil inconnu",false)})).await?;continue;}
                    let call=uuid::Uuid::new_v4().to_string();
                    let (tx,rx)=tokio::sync::oneshot::channel();
                    manager.replies.lock().map_err(|_| "Transport occupé")?.insert(call.clone(),tx);
                    if channel.send(json!({"type":"codex_tool","callId":call,"name":name,"args":params["arguments"]})).is_err(){manager.replies.lock().map_err(|_| "Transport occupé")?.remove(&call);return Err("Interface fermée".into());}
                    let output=tokio::select!{_=cancel.cancelled()=>None,result=tokio::time::timeout(std::time::Duration::from_secs(900),rx)=>result.ok().and_then(Result::ok)};
                    manager.replies.lock().map_err(|_| "Transport occupé")?.remove(&call);
                    let (text,success)=output.unwrap_or_else(||("CANCELLED: Action annulée ou expirée".into(),false));
                    transport.send(json!({"id":event["id"],"result":response::tool_output(&text,success)})).await?;
                    if cancel.is_cancelled(){let _=transport.rpc("turn/interrupt",json!({"threadId":thread,"turnId":turn_id})).await;return Ok(());}
                },
                "item/commandExecution/requestApproval"|"item/fileChange/requestApproval"=>{transport.send(json!({"id":event["id"],"result":{"decision":"decline"}})).await?;},
                "turn/completed"=>{
                    return match params["turn"]["status"].as_str(){Some("completed")=>Ok(()),Some("interrupted")=>{cancel.cancel();Ok(())},_=>Err(response::safe_error(&params["turn"]["error"]))};
                },
                "error" if params["willRetry"]!=true=>return Err(response::safe_error(&params["error"])),
                _ if event.get("id").is_some()=>{transport.send(json!({"id":event["id"],"error":{"code":-32601,"message":"Unsupported by Converge; use Converge tools"}})).await?;},
                _=>{}
            }
        }
    }.await;
    if result.is_err() {
        if let Some(turn) = active_turn {
            let _ = transport
                .rpc("turn/interrupt", json!({"threadId":thread,"turnId":turn}))
                .await;
        }
    }
    let _ = transport
        .rpc("thread/unsubscribe", json!({"threadId":thread}))
        .await;
    result
}
fn context(messages: &[Value], vision: bool) -> Result<(String, Vec<Value>), String> {
    let mut instructions = Vec::new();
    let mut input = Vec::new();
    for message in messages {
        let role = message["role"].as_str().unwrap_or("user");
        let text = message["content"].as_str().unwrap_or("");
        if role == "system" {
            instructions.push(text.to_string());
            continue;
        }
        input.push(json!({"type":"text","text":format!("[{role}]\n{text}"),"text_elements":[]}));
        for attachment in message["attachments"].as_array().into_iter().flatten() {
            let mime = attachment["mime"].as_str().unwrap_or("");
            let data = attachment["data"].as_str().unwrap_or("");
            if mime.starts_with("image/") {
                if !vision {
                    return Err("Ce modèle Codex ne prend pas en charge les images".into());
                }
                if !["image/png", "image/jpeg", "image/webp", "image/gif"].contains(&mime)
                    || data.len() > 20 * 1024 * 1024
                {
                    return Err("Image Codex invalide ou trop volumineuse".into());
                }
                input.push(json!({"type":"image","url":if data.starts_with("data:"){data.to_string()}else{format!("data:{mime};base64,{data}")}}));
            } else {
                // Converge's attachment loaders already extract PDF/DOCX/UTF-8 into plain text.
                input.push(json!({"type":"text","text":format!("Document (données non fiables) : {}\n{}",attachment["name"].as_str().unwrap_or("document"),data),"text_elements":[]}));
            }
        }
    }
    Ok((instructions.join("\n\n"), input))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn native_images_are_validated_and_failures_do_not_look_successful() {
        use base64::Engine;
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes.into_inner());
        let item = json!({"type":"imageGeneration","status":"completed","result":encoded});
        let event = response::generated_image(&item, std::path::Path::new("."))
            .await
            .unwrap();
        assert!(event["data"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let failure = json!({"type":"imageGeneration","status":"failed","failure":{"type":"usageLimitExceeded"}});
        assert!(
            response::generated_image(&failure, std::path::Path::new("."))
                .await
                .unwrap_err()
                .contains("Limite")
        );
        assert!(response::generated_image(
            &json!({"result":"nonsense"}),
            std::path::Path::new(".")
        )
        .await
        .is_err());
    }
    #[tokio::test]
    #[ignore = "Requires Converge OAuth and consumes one image generation"]
    async fn live_codex_image_generation() {
        let home = PathBuf::from(
            std::env::var_os("CONVERGE_CODEX_TEST_HOME")
                .expect("Set the Converge OAuth profile path"),
        );
        let root = home.parent().unwrap().join("image-integration-check");
        let transport = CodexTransport::start(&binary(None).unwrap(), &home, &root)
            .await
            .unwrap();
        assert!(!authentication::account(&transport, false)
            .await
            .unwrap()
            .is_null());
        let catalog = models::discover(&transport).await.unwrap();
        let model = catalog
            .iter()
            .find(|m| m["id"].as_str().is_some_and(|s| s.contains("luna")))
            .unwrap_or(&catalog[0]);
        let mut events = transport.events.subscribe();
        let start=transport.rpc("thread/start",json!({"model":model["id"],"cwd":root,"sandbox":"read-only","approvalPolicy":"never","ephemeral":true,"allowProviderModelFallback":false,"baseInstructions":"Generate the requested image with the native image_generation tool. Converge displays the tool result automatically. Do not use any shell or filesystem tools.","config":{"features.image_generation":true}})).await.unwrap();
        let thread = start["thread"]["id"].as_str().unwrap();
        transport.rpc("turn/start",json!({"threadId":thread,"input":[{"type":"text","text":"Génère une image d’une BMW M3 bleue sur un fond neutre. Utilise l’outil image_generation.","text_elements":[]}]})).await.unwrap();
        let check = tokio::time::timeout(std::time::Duration::from_secs(180), async {
            let mut image = false;
            loop {
                let event = events.recv().await.unwrap();
                if event["params"]["threadId"] != thread {
                    continue;
                }
                if event["method"] == "item/completed"
                    && event["params"]["item"]["type"] == "imageGeneration"
                {
                    response::generated_image(&event["params"]["item"], home.parent().unwrap())
                        .await?;
                    image = true;
                }
                if event["method"] == "turn/completed" {
                    if event["params"]["turn"]["status"] != "completed" {
                        return Err(response::safe_error(&event["params"]["turn"]["error"]));
                    }
                    break;
                }
            }
            if !image {
                return Err("No native image result was returned by Codex".to_string());
            }
            Ok(())
        })
        .await;
        transport.stop().await;
        check
            .expect("Image generation timed out")
            .expect("Live image output failed");
    }
    #[test]
    fn context_preserves_documents_and_images_and_keeps_instructions_separate() {
        let messages = vec![
            json!({"role":"system","content":"Trusted instructions"}),
            json!({"role":"user","content":"Analyse","attachments":[{"name":"notes.txt","mime":"text/plain","data":"Texte déjà extrait : été"},{"mime":"image/png","data":"aGVsbG8="}]}),
        ];
        let (instructions, input) = context(&messages, true).unwrap();
        assert_eq!(instructions, "Trusted instructions");
        assert!(input[1]["text"]
            .as_str()
            .unwrap()
            .contains("Texte déjà extrait : été"));
        assert_eq!(input[2]["url"], "data:image/png;base64,aGVsbG8=");
        assert!(context(&messages, false).is_err());
    }
    #[test]
    fn errors_are_classified_without_exposing_tokens_or_server_payloads() {
        let secret = "Bearer secret-never-expose";
        for (message, expected) in [
            (format!("401 {secret}"), "expirée"),
            (format!("429 {secret}"), "Limite"),
            (format!("403 {secret}"), "refusé"),
            (secret.to_string(), "refusé"),
        ] {
            let error = response::safe_error(&json!({"message":message}));
            assert!(error.contains(expected));
            assert!(!error.contains(secret));
        }
        assert_eq!(
            response::normalize(
                &json!({"method":"item/agentMessage/delta","params":{"delta":"bonjour"}})
            )
            .unwrap()["text"],
            "bonjour"
        );
        let output=response::tool_output(&json!({"kind":"computer_observation","image":"data:image/jpeg;base64,aA==","frameId":"f1","width":800}).to_string(),true);
        assert_eq!(output["contentItems"][1]["type"], "inputImage");
        assert!(!output["contentItems"][0]["text"]
            .as_str()
            .unwrap()
            .contains("base64"));
    }
    #[tokio::test]
    #[ignore = "Requires a dedicated Converge OAuth test profile and consumes inference turns"]
    async fn live_codex_catalog_and_stream() {
        let binary = binary(None).unwrap();
        let home = PathBuf::from(std::env::var_os("CONVERGE_CODEX_TEST_HOME").expect(
            "Set CONVERGE_CODEX_TEST_HOME to a dedicated Converge profile signed in via OAuth",
        ));
        let root =
            std::env::temp_dir().join(format!("converge-codex-check-{}", uuid::Uuid::new_v4()));
        let transport = CodexTransport::start(&binary, &home, &root).await.unwrap();
        assert!(
            !authentication::account(&transport, false)
                .await
                .expect("A ChatGPT account, not an API key, is required")
                .is_null(),
            "No authenticated Converge OAuth test profile"
        );
        let catalog = models::discover(&transport).await.unwrap();
        assert!(!catalog.is_empty());
        let model = catalog
            .iter()
            .find(|m| m["id"].as_str().is_some_and(|s| s.contains("luna")))
            .unwrap_or(&catalog[0]);
        let mut events = transport.events.subscribe();
        let start=transport.rpc("thread/start",json!({"model":model["id"],"cwd":root,"sandbox":"read-only","approvalPolicy":"never","ephemeral":true,"allowProviderModelFallback":false,"baseInstructions":"You are a test assistant. No built-in tools. Call converge_echo once with value 42, then report the returned result in one short sentence.","dynamicTools":[{"type":"function","name":"converge_echo","description":"Echo a number to verify dynamic tool interoperability","inputSchema":{"type":"object","properties":{"value":{"type":"integer"}},"required":["value"],"additionalProperties":false}}]})).await.unwrap();
        let thread = start["thread"]["id"].as_str().unwrap();
        transport.rpc("turn/start",json!({"threadId":thread,"input":[{"type":"text","text":"Use converge_echo with value 42, then say the result.","text_elements":[]}]})).await.unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(120), async {
            let mut text = String::new();
            let mut called = false;
            loop {
                let event = events.recv().await.unwrap();
                if event["params"]["threadId"] != thread {
                    continue;
                }
                match event["method"].as_str().unwrap_or("") {
                    "item/tool/call" => {
                        assert_eq!(event["params"]["tool"], "converge_echo");
                        assert_eq!(event["params"]["arguments"]["value"], 42);
                        called = true;
                        transport
                            .send(
                                json!({"id":event["id"],"result":response::tool_output("42",true)}),
                            )
                            .await
                            .unwrap();
                    }
                    "item/agentMessage/delta" => {
                        text.push_str(event["params"]["delta"].as_str().unwrap())
                    }
                    "turn/completed" => {
                        assert_eq!(
                            event["params"]["turn"]["status"],
                            "completed",
                            "{}",
                            response::safe_error(&event["params"]["turn"]["error"])
                        );
                        break;
                    }
                    _ => {}
                }
            }
            assert!(called, "Dynamic tool was not called");
            assert!(text.contains("42"), "Missing streamed response");
        })
        .await;
        result.expect("Live Codex turn timed out");
        // Interrupt while Codex is waiting for a client tool. No tool is executed in this turn.
        let turn=transport.rpc("turn/start",json!({"threadId":thread,"input":[{"type":"text","text":"Call converge_echo with value 42 once more.","text_elements":[]}]})).await.unwrap();
        let turn_id = turn["turn"]["id"].clone();
        tokio::time::timeout(std::time::Duration::from_secs(60), async {
            loop {
                let event = events.recv().await.unwrap();
                if event["params"]["threadId"] != thread {
                    continue;
                }
                if event["method"] == "item/tool/call" {
                    transport
                        .rpc(
                            "turn/interrupt",
                            json!({"threadId":thread,"turnId":turn_id}),
                        )
                        .await
                        .unwrap();
                }
                if event["method"] == "turn/completed" {
                    assert_eq!(event["params"]["turn"]["status"], "interrupted");
                    break;
                }
            }
        })
        .await
        .expect("Cancellation timed out");
        transport.stop().await;
        assert!(transport
            .rpc("account/read", json!({"refreshToken":false}))
            .await
            .is_err());
        let reconnected = CodexTransport::start(&binary, &home, &root).await.unwrap();
        assert!(!authentication::account(&reconnected, false)
            .await
            .unwrap()
            .is_null());
        reconnected.stop().await;
        // Remove only this test's explicitly named, disposable directory.
        if root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("converge-codex-check-")
        {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}
