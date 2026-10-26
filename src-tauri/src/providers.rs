use crate::{
    persistence::{provider, secret, Provider, Secret},
    AppState,
};
use base64::Engine;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{ipc::Channel, State};

pub fn validate(p: &Provider) -> Result<(), String> {
    crate::zai::validate(p)?;
    crate::deepseek::validate(p)?;
    if crate::cli_providers::supports(&p.plugin) {
        return crate::cli_providers::validate(p);
    }
    if p.plugin == "chatgpt" {
        if uuid::Uuid::parse_str(&p.id).is_err()
            || p.protocol != "codex"
            || p.name.trim().is_empty()
            || p.name.len() > 120
            || !p.headers.is_empty()
            || p.capabilities["images"] == true
            || p.models.len() > 1000
            || p.models
                .iter()
                .any(|m| !m.starts_with("codex/") || m.len() > 500)
        {
            return Err("Configuration ChatGPT invalide".into());
        }
        let config = &p.request["chatgpt"];
        if config["authSource"]
            .as_str()
            .is_some_and(|s| s != "converge")
            || config["mode"].as_str().is_some_and(|s| s != "codex")
        {
            return Err("Cette ancienne configuration ChatGPT a été retirée. Reconnectez le compte dans Converge.".into());
        }
        return Ok(());
    }
    if !crate::plugins::accepts(&p.plugin, &p.protocol) {
        return Err("Plugin ou protocole non supporté".into());
    }
    validate_url(&p.url)?;
    for key in ["chatPath", "modelsPath"] {
        if let Some(path) = p.request[key].as_str() {
            if path.len() > 500
                || path.contains("://")
                || path.contains("..")
                || path.contains(['?', '#', '\\', '\0'])
            {
                return Err("Les chemins de requête doivent rester relatifs à l’URL de base, sans paramètres".into());
            }
        }
    }
    if let Some(body) = p.request.get("body") {
        if !body.is_null() && (!body.is_object() || body.to_string().len() > 64000) {
            return Err(
                "Les paramètres supplémentaires doivent être un objet JSON de moins de 64 Ko"
                    .into(),
            );
        }
    }
    if p.name.trim().is_empty() || p.name.len() > 120 {
        return Err("Nom de connexion invalide".into());
    }
    if p.models.len() > 1000
        || p.models
            .iter()
            .any(|m| m.trim().is_empty() || m.len() > 500)
        || p.headers.len() > 64
    {
        return Err("Configuration fournisseur trop volumineuse".into());
    }
    if !p.capabilities["vision"].is_boolean()
        || !p.capabilities["tools"].is_boolean()
        || !p.capabilities["images"].is_boolean()
        || !p.capabilities["context"]
            .as_u64()
            .is_some_and(|n| (512..=2000000).contains(&n))
    {
        return Err("Capacités fournisseur invalides".into());
    }
    Ok(())
}
pub fn validate_url(value: &str) -> Result<url::Url, String> {
    let u = url::Url::parse(value).map_err(|_| "URL invalide".to_string())?;
    let local = matches!(
        u.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if (u.scheme() != "https" && !(u.scheme() == "http" && local))
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err(
            "Utilisez HTTPS, ou HTTP sur localhost, sans identifiants ni paramètres dans l’URL"
                .into(),
        );
    }
    Ok(u)
}
fn endpoint(p: &Provider, path: &str) -> String {
    format!(
        "{}/{}",
        p.url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}
fn auth(
    mut r: reqwest::RequestBuilder,
    p: &Provider,
    s: &Secret,
) -> Result<reqwest::RequestBuilder, String> {
    if crate::deepseek::oauth(p) {
        crate::deepseek::validate(p)?;
        return Ok(r
            .header("x-dsh-auth-token", &s.key)
            .header("anthropic-version", "2023-06-01")
            .header("User-Agent", "Converge/0.1.0"));
    }
    for (k, v) in s.headers.iter().chain(p.headers.iter()) {
        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
            .map_err(|_| "Nom de header invalide")?;
        if matches!(
            name.as_str(),
            "host" | "content-length" | "connection" | "transfer-encoding"
        ) {
            return Err("Header réservé".into());
        }
        let val =
            reqwest::header::HeaderValue::from_str(v).map_err(|_| "Valeur de header invalide")?;
        r = r.header(name, val);
    }
    if !s.key.is_empty() {
        r = if p.protocol == "gemini" {
            r.header("x-goog-api-key", &s.key)
        } else if p.protocol == "anthropic" {
            r.header("x-api-key", &s.key)
        } else {
            r.bearer_auth(&s.key)
        }
    }
    if p.protocol == "anthropic" {
        r = r.header("anthropic-version", "2023-06-01");
    }
    Ok(r)
}
async fn checked(response: reqwest::Response) -> Result<reqwest::Response, String> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(format!(
            "Le serveur a renvoyé HTTP {}. Vérifiez l’URL, la clé et le modèle.",
            response.status().as_u16()
        ))
    }
}
fn opencode_error(status: u16, body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body).to_ascii_lowercase();
    if status == 429 && (text.contains("freeusagelimit") || text.contains("free usage")) {
        return "Le quota gratuit OpenCode est épuisé ou temporairement limité. Vérifiez le quota dans la console Zen ; aucun autre modèle n’a été utilisé à votre place.".into();
    }
    if text.contains("freetiererror")
        || text.contains("free tier can only be used from within opencode")
    {
        return "OpenCode réserve ce modèle gratuit à son application officielle. Dans Converge, choisissez un modèle accessible avec votre abonnement Go ou vos crédits Zen.".into();
    }
    if text.contains("subscription") || text.contains("subscriptionerror") {
        return "OpenCode refuse l’accès au service Go : vérifiez que cette clé appartient à un compte avec un abonnement Go actif. Pour une clé Zen, choisissez « Zen · crédits » dans le fournisseur.".into();
    }
    if text.contains("regionerror") || text.contains("privacy") || text.contains("region is not") {
        return "OpenCode bloque ce modèle avec les règles de confidentialité ou de région de votre espace. Vérifiez les autorisations du modèle dans la console OpenCode.".into();
    }
    match status {
        401=>"OpenCode refuse cette clé API. Vérifiez la clé de votre espace dans la configuration du fournisseur.".into(),
        402=>"OpenCode indique que les crédits ou l’abonnement nécessaires à ce modèle ne sont pas disponibles.".into(),
        429=>"La limite d’utilisation OpenCode est atteinte. Vérifiez le quota de votre compte.".into(),
        403 if text.contains("<html") || text.contains("<!doctype")=>"L’accès HTTP à OpenCode est bloqué avant une réponse API valide (403). Vérifiez le réseau ou le proxy utilisé pour joindre opencode.ai.".into(),
        403=>"OpenCode refuse l’accès à ce modèle (403). Vérifiez le service choisi (Go ou Zen), les droits du compte et les autorisations du modèle dans sa console.".into(),
        _=>format!("OpenCode a renvoyé HTTP {status}. Vérifiez le service et le modèle configurés."),
    }
}
async fn checked_provider(
    response: reqwest::Response,
    p: &Provider,
) -> Result<reqwest::Response, String> {
    if p.plugin == "deepseek" && !response.status().is_success() {
        let status = response.status().as_u16();
        return Err(match status {
            401 if crate::deepseek::oauth(p) => "La connexion DeepSeek a expiré ou a été refusée. Reconnectez le compte dans les fournisseurs.".into(),
            402 => "Le solde ou le quota DeepSeek est insuffisant. Vérifiez votre compte sur la plateforme DeepSeek.".into(),
            429 => "DeepSeek limite temporairement les requêtes. Réessayez dans un instant.".into(),
            _ => format!("DeepSeek a renvoyé HTTP {status}. Vérifiez les droits de votre compte et le modèle sélectionné."),
        });
    }
    if p.plugin != "opencode" || response.status().is_success() {
        return checked(response).await;
    }
    let status = response.status().as_u16();
    let mut body = vec![];
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else { break };
        let remaining = 16 * 1024 - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        if body.len() >= 16 * 1024 {
            break;
        }
    }
    Err(opencode_error(status, &body))
}
#[tauri::command]
pub async fn discover_models(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider: Provider,
    key: Option<String>,
) -> Result<Vec<String>, String> {
    if provider.plugin == "chatgpt" {
        return crate::chatgpt::discover(&app, &provider).await;
    }
    if provider.protocol == "cli" {
        return crate::cli_providers::discover(&app, &provider).await;
    }
    discover_inner(&state, provider, key).await
}
async fn discover_inner(
    state: &AppState,
    provider: Provider,
    key: Option<String>,
) -> Result<Vec<String>, String> {
    validate(&provider)?;
    let mut s = if crate::zai::oauth(&provider) {
        crate::zai::credentials(&provider)?
    } else if crate::deepseek::oauth(&provider) {
        crate::deepseek::credentials(&provider)?
    } else {
        secret(&provider.id)?
    };
    if let Some(key) = key {
        if crate::deepseek::oauth(&provider) {
            return Err("Le compte DeepSeek n’utilise pas de clé API manuelle".into());
        }
        s.key = key;
    }
    let default_path = match provider.protocol.as_str() {
        "ollama" => "api/tags",
        "comfyui" => "object_info/CheckpointLoaderSimple",
        _ => {
            if provider.plugin == "opencode" {
                "v1/models"
            } else {
                "models"
            }
        }
    };
    let path = provider.request["modelsPath"]
        .as_str()
        .filter(|v| !v.is_empty())
        .unwrap_or(default_path);
    let response=auth(state.client.get(endpoint(&provider,path)).timeout(std::time::Duration::from_secs(12)),&provider,&s)?.send().await.map_err(|_|"Serveur inaccessible. Vérifiez qu’il est démarré et que l’URL inclut /v1 pour une API compatible OpenAI.".to_string())?;
    let body: Value = checked_provider(response, &provider)
        .await?
        .json()
        .await
        .map_err(|_| "Réponse de découverte invalide".to_string())?;
    if provider.protocol == "comfyui" {
        return Ok(
            body["CheckpointLoaderSimple"]["input"]["required"]["ckpt_name"][0]
                .as_array()
                .ok_or("Aucun checkpoint ComfyUI détecté")?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
        );
    }
    let list = if provider.protocol == "ollama" {
        body["models"].as_array()
    } else {
        body["data"].as_array()
    }
    .ok_or("Le serveur n’a pas renvoyé de liste de modèles")?;
    let mut models: Vec<String> = list
        .iter()
        .filter_map(|m| {
            m[if provider.protocol == "ollama" {
                "name"
            } else {
                "id"
            }]
            .as_str()
            .map(String::from)
        })
        .collect();
    models.sort();
    models.dedup();
    Ok(models)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    #[serde(default)]
    pub(crate) conversation_id: Option<String>,
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) model: String,
    pub(crate) messages: Vec<Value>,
    temperature: f64,
    pub(crate) max_tokens: u32,
    #[serde(default)]
    pub(crate) reasoning_effort: Option<String>,
    #[serde(default)]
    pub(crate) tools_enabled: Option<bool>,
}
fn apply_effort(
    body: &mut Value,
    protocol: &str,
    model: &str,
    effort: Option<&str>,
) -> Result<(), String> {
    let effort = effort.unwrap_or("auto");
    if effort == "auto" {
        return Ok(());
    }
    if !["low", "medium", "high", "xhigh", "max"].contains(&effort) {
        return Err("Effort de raisonnement invalide".into());
    }
    match protocol {
        "ollama" => {
            if !model.to_ascii_lowercase().contains("gpt-oss")
                || !["low", "medium", "high"].contains(&effort)
            {
                return Err(
                    "Ce modèle Ollama ne fournit pas ces niveaux d’effort. Choisissez Auto.".into(),
                );
            }
            body["think"] = json!(effort);
        }
        "anthropic" => body["output_config"]["effort"] = json!(effort),
        "responses" => body["reasoning"]["effort"] = json!(effort),
        "gemini" => {
            if !["low", "medium", "high"].contains(&effort) {
                return Err("Choisissez Auto, Faible, Moyen ou Élevé pour Gemini".into());
            }
            body["generationConfig"]["thinkingConfig"]["thinkingLevel"] =
                json!(effort.to_uppercase());
        }
        _ => {
            body["reasoning_effort"] = json!(effort);
            if ["gpt-5", "gpt-6", "o1", "o3", "o4"]
                .iter()
                .any(|prefix| model.starts_with(prefix))
            {
                body.as_object_mut().unwrap().remove("temperature");
                if let Some(tokens) = body.as_object_mut().unwrap().remove("max_tokens") {
                    body["max_completion_tokens"] = tokens;
                }
            }
        }
    }
    Ok(())
}
fn normalized(messages: &[Value], protocol: &str) -> Vec<Value> {
    messages.iter().map(|m|{
        let text=m["content"].as_str().unwrap_or_default();
        let attachments=m["attachments"].as_array();
        if protocol=="ollama"{
            let images:Vec<&str>=attachments.into_iter().flatten().filter(|a|a["mime"].as_str().unwrap_or("").starts_with("image/")).filter_map(|a|a["data"].as_str()).collect();
            json!({"role":m["role"],"content":text,"images":images})
        }else{
            let mut content=vec![if protocol=="anthropic"{json!({"type":"text","text":text})}else{json!({"type":"text","text":text})}];
            for a in attachments.into_iter().flatten(){
                let mime=a["mime"].as_str().unwrap_or("");let data=a["data"].as_str().unwrap_or("");
                if mime.starts_with("image/"){content.push(if protocol=="anthropic"{json!({"type":"image","source":{"type":"base64","media_type":mime,"data":data}})}else{json!({"type":"image_url","image_url":{"url":format!("data:{mime};base64,{data}")}})});}
            }
            json!({"role":m["role"],"content":content})
        }
    }).collect()
}
fn response_input(messages: &[Value]) -> Vec<Value> {
    messages.iter().map(|m|{
        let assistant=m["role"]=="assistant";
        let content:Vec<Value>=m["content"].as_array().into_iter().flatten().map(|b|{
            if b["type"]=="image_url"{json!({"type":"input_image","image_url":b["image_url"]["url"]})}
            else{json!({"type":if assistant{"output_text"}else{"input_text"},"text":b["text"]})}
        }).collect();
        json!({"role":m["role"],"content":content})
    }).collect()
}
fn gemini_body(messages: &[Value], temperature: f64, max_tokens: u32) -> Value {
    let system = messages
        .iter()
        .filter(|m| m["role"] == "system")
        .filter_map(|m| m["content"].as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let contents: Vec<Value> = messages
        .iter()
        .filter(|m| m["role"] != "system")
        .map(|m| {
            let mut parts = vec![json!({"text":m["content"].as_str().unwrap_or_default()})];
            for a in m["attachments"].as_array().into_iter().flatten() {
                if a["mime"].as_str().is_some_and(|s| s.starts_with("image/")) {
                    parts.push(json!({"inlineData":{"mimeType":a["mime"],"data":a["data"]}}));
                }
            }
            json!({"role":if m["role"]=="assistant"{"model"}else{"user"},"parts":parts})
        })
        .collect();
    json!({"systemInstruction":{"parts":[{"text":system}]},"contents":contents,"generationConfig":{"temperature":temperature,"maxOutputTokens":max_tokens}})
}
fn opencode_route<'a>(
    model: &str,
    fallback: &'a str,
    go: bool,
) -> Result<(&'a str, String), String> {
    if !model
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err("Identifiant de modèle OpenCode invalide".into());
    }
    let protocol = if model.starts_with("gemini-") {
        "gemini"
    } else if ["gpt-", "grok-", "muse-"]
        .iter()
        .any(|s| model.starts_with(s))
    {
        "responses"
    } else if model.starts_with("claude-")
        || (go && (model == "minimax-m3" || model.starts_with("qwen3.")))
        || (!go && model.starts_with("qwen3.") && model != "qwen3.8-max")
    {
        "anthropic"
    } else {
        fallback
    };
    Ok((
        protocol,
        match protocol {
            "responses" => "openai/v1/responses".into(),
            "anthropic" => "anthropic/v1/messages".into(),
            "gemini" => format!("google/v1beta/models/{model}:streamGenerateContent"),
            _ => "openai/v1/chat/completions".into(),
        },
    ))
}
#[cfg(test)]
fn delta(v: &Value, protocol: &str) -> Option<String> {
    match protocol {
        "ollama" => v["message"]["content"].as_str(),
        "anthropic" => v["delta"]["text"].as_str(),
        _ => v["choices"][0]["delta"]["content"].as_str(),
    }
    .map(str::to_owned)
}
#[tauri::command]
pub async fn stream_chat(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: Request,
    on_event: Channel<Value>,
) -> Result<(), String> {
    let _desktop_run = crate::desktop_control::track_run(
        &app,
        request.conversation_id.as_deref(),
        format!("{}:{}", request.provider_id, request.id),
    );
    let p = provider(&state, &request.provider_id)?;
    if p.plugin == "chatgpt" || p.protocol == "cli" {
        validate(&p)?;
        if uuid::Uuid::parse_str(&request.id).is_err()
            || !p.models.contains(&request.model)
            || request.messages.len() > 500
            || !(1..=131072).contains(&request.max_tokens)
            || !(0.0..=2.0).contains(&request.temperature)
        {
            return Err("Requête du fournisseur invalide".into());
        }
        let key = format!("{}:{}", p.id, request.id);
        let token = tokio_util::sync::CancellationToken::new();
        {
            let mut requests = state.requests.lock().map_err(|_| "Requêtes occupées")?;
            if requests.contains_key(&key) {
                return Err("Requête déjà en cours".into());
            }
            requests.insert(key.clone(), token.clone());
        }
        let result = if p.protocol == "cli" {
            crate::cli_providers::stream(&app, &p, &request, &on_event, token.clone()).await
        } else {
            crate::chatgpt::stream(&app, &state, &p, &request, &on_event, token.clone()).await
        };
        state
            .requests
            .lock()
            .map_err(|_| "Requêtes occupées")?
            .remove(&key);
        let _ = on_event.send(json!({"type":"done","cancelled":token.is_cancelled()}));
        return result;
    }
    stream_chat_inner(&state, request, on_event).await
}
async fn stream_chat_inner(
    state: &AppState,
    request: Request,
    on_event: Channel<Value>,
) -> Result<(), String> {
    if uuid::Uuid::parse_str(&request.id).is_err() {
        return Err("Identifiant de requête invalide".into());
    }
    let request_key = format!("{}:{}", request.provider_id, request.id);
    let mut p = provider(&state, &request.provider_id)?;
    validate(&p)?;
    if p.protocol == "comfyui" {
        return Err("ComfyUI est un moteur d’images. Choisissez un modèle de conversation.".into());
    }
    if !p.models.contains(&request.model) {
        return Err("Ce modèle n’est pas configuré pour cette connexion".into());
    }
    if p.plugin == "opencode" {
        let (protocol, path) = opencode_route(
            &request.model,
            &p.protocol,
            p.url.trim_end_matches('/').ends_with("/go"),
        )?;
        p.protocol = protocol.into();
        p.request["chatPath"] = json!(path);
    }
    if request.messages.len() > 500
        || !(1..=131072).contains(&request.max_tokens)
        || !(0.0..=2.0).contains(&request.temperature)
    {
        return Err("Paramètres de génération hors limites".into());
    }
    let token = tokio_util::sync::CancellationToken::new();
    {
        let mut requests = state.requests.lock().map_err(|e| e.to_string())?;
        if requests.contains_key(&request_key) {
            return Err("Requête déjà en cours".into());
        }
        requests.insert(request_key.clone(), token.clone());
    }
    let result=async {
        let s=if crate::zai::oauth(&p){crate::zai::credentials(&p)?}else if crate::deepseek::oauth(&p){crate::deepseek::credentials(&p)?}else{secret(&p.id)?};
        let messages=normalized(&request.messages,&p.protocol);
        let (path,mut body)=match p.protocol.as_str(){
            "ollama"=>("api/chat",json!({"model":request.model,"messages":messages,"stream":true,"options":{"temperature":request.temperature,"num_predict":request.max_tokens}})),
            "anthropic"=>{
                let system=request.messages.iter().filter(|m|m["role"]=="system").filter_map(|m|m["content"].as_str()).collect::<Vec<_>>().join("\n\n");
                let messages:Vec<_>=messages.into_iter().filter(|m|m["role"]!="system").collect();
                ("messages",json!({"model":request.model,"messages":messages,"system":system,"stream":true,"temperature":request.temperature.min(1.0),"max_tokens":request.max_tokens}))
            },
            "responses"=>("responses",json!({"model":request.model,"input":response_input(&messages),"stream":true,"store":false,"max_output_tokens":request.max_tokens})),
            "gemini"=>("",gemini_body(&request.messages,request.temperature,request.max_tokens)),
            _=>("chat/completions",json!({"model":request.model,"messages":messages,"stream":true,"temperature":request.temperature,"max_tokens":request.max_tokens})),
        };
        if p.capabilities["tools"]==true && request.tools_enabled!=Some(false) {
            let schemas=crate::tools::schemas();
            body["tools"]=Value::Array(schemas.into_iter().map(|(name,description,parameters)|if p.protocol=="anthropic"{json!({"name":name,"description":description,"input_schema":parameters})}else if p.protocol=="responses"{json!({"type":"function","name":name,"description":description,"parameters":parameters,"strict":false})}else if p.protocol=="gemini"{json!({"functionDeclarations":[{"name":name,"description":description,"parameters":parameters}]})}else{json!({"type":"function","function":{"name":name,"description":description,"parameters":parameters}})}).collect());
        }
        let path=p.request["chatPath"].as_str().filter(|v|!v.is_empty()).unwrap_or(path);
        if let Some(extra)=p.request["body"].as_object(){for(key,value)in extra{if !["model","messages","system","stream","tools"].contains(&key.as_str()){body[key]=value.clone();}}}
        apply_effort(&mut body,&p.protocol,&request.model,request.reasoning_effort.as_deref())?;
        let mut http=auth(state.client.post(endpoint(&p,path)).header("User-Agent","Converge/0.1.0"),&p,&s)?;
        if p.protocol=="gemini"{http=http.query(&[("alt","sse")]);}
        if p.plugin=="opencode" {if let Some(cid)=request.conversation_id.as_deref().filter(|s|uuid::Uuid::parse_str(s).is_ok()){http=http.header("x-opencode-session",cid);}}
        let response=tokio::select!{_ = token.cancelled()=>return Ok(()), r=http.json(&body).send()=>r.map_err(|_|"Connexion interrompue ou délai dépassé".to_string())?};
        if response.status().as_u16()==400 && request.reasoning_effort.as_deref().is_some_and(|e|e!="auto") {return Err("Le serveur a refusé la requête avec cet effort de raisonnement. Vérifiez les niveaux acceptés par ce modèle ou choisissez Auto.".into());}
        let mut stream=checked_provider(response,&p).await?.bytes_stream();
        let mut decoder=crate::streaming::Decoder::new(&p.protocol);let mut accumulated=crate::streaming::Accumulator::new(&p.protocol);let mut total=0usize; let mut tool_progress = String::new(); let mut last_tool_progress = std::time::Instant::now() - std::time::Duration::from_secs(1);
        loop {
            let next=tokio::select!{_ = token.cancelled()=>break, chunk=tokio::time::timeout(std::time::Duration::from_secs(90),stream.next())=>chunk.map_err(|_|"Le serveur ne répond plus depuis 90 secondes".to_string())?};
            let Some(chunk)=next else{break};let chunk=chunk.map_err(|_|"Flux interrompu par le serveur".to_string())?;
            total+=chunk.len();if total>32*1024*1024{return Err("Réponse trop volumineuse".into())}
            for value in decoder.feed(&chunk)?{if let Some(text)=accumulated.ingest(&value)?{on_event.send(json!({"type":"delta","text":text})).map_err(|e|e.to_string())?;} if last_tool_progress.elapsed() >= std::time::Duration::from_millis(40) { last_tool_progress = std::time::Instant::now(); let progress = accumulated.progress(); let serialized = serde_json::to_string(&progress).map_err(|e|e.to_string())?; if serialized != tool_progress { tool_progress = serialized; for event in progress { on_event.send(event).map_err(|e|e.to_string())?; } } }}
        }
        if !token.is_cancelled(){for value in decoder.finish()?{if let Some(text)=accumulated.ingest(&value)?{on_event.send(json!({"type":"delta","text":text})).map_err(|e|e.to_string())?;}}if !accumulated.done{return Err("Flux terminé sans événement de fin ; la réponse partielle a été conservée".into())}for event in accumulated.tools()?{on_event.send(event).map_err(|e|e.to_string())?;}}Ok(())
    }.await;
    state
        .requests
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&request_key);
    on_event
        .send(json!({"type":"done","cancelled":token.is_cancelled()}))
        .ok();
    result
}
#[tauri::command]
pub fn cancel_request(state: State<AppState>, request_id: String) -> Result<(), String> {
    if uuid::Uuid::parse_str(&request_id).is_err() {
        return Err("Identifiant de requête invalide".into());
    }
    let suffix = format!(":{request_id}");
    for (key, token) in state.requests.lock().map_err(|e| e.to_string())?.iter() {
        if key == &request_id || key.ends_with(&suffix) {
            token.cancel()
        }
    }
    Ok(())
}
#[tauri::command]
pub async fn generate_image(
    state: State<'_, AppState>,
    provider_id: String,
    model: String,
    prompt: String,
    size: String,
    image: Option<String>,
    request_id: Option<String>,
) -> Result<Vec<String>, String> {
    let id = request_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err("Identifiant de requête invalide".into());
    }
    let id = format!("{provider_id}:{id}");
    let token = tokio_util::sync::CancellationToken::new();
    {
        let mut map = state.requests.lock().map_err(|e| e.to_string())?;
        if map.contains_key(&id) {
            return Err("Requête déjà en cours".into());
        }
        map.insert(id.clone(), token.clone());
    }
    let result = tokio::select! {_=token.cancelled()=>Err("Attente de génération d’image annulée".into()),result=generate_image_inner(&state,provider_id,model,prompt,size,image)=>result};
    state
        .requests
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&id);
    result
}
async fn generate_image_inner(
    state: &AppState,
    provider_id: String,
    model: String,
    prompt: String,
    size: String,
    image: Option<String>,
) -> Result<Vec<String>, String> {
    let p = provider(&state, &provider_id)?;
    validate(&p)?;
    if p.capabilities["images"] != true {
        return Err("Activez la capacité de génération d’images pour ce moteur".into());
    }
    if !p.models.contains(&model)
        || prompt.len() > 20000
        || !["1024x1024", "1024x1536", "1536x1024"].contains(&size.as_str())
    {
        return Err("Paramètres image invalides".into());
    }
    if p.protocol == "comfyui" {
        return comfy_image(&state, &p, &model, &prompt, &size, image).await;
    }
    if p.protocol != "openai" {
        return Err("Utilisez un endpoint d’images compatible OpenAI ou ComfyUI".into());
    }
    let request = if let Some(data) = image {
        let (_, data) = data.split_once(',').ok_or("Image de référence invalide")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|_| "Image de référence invalide")?;
        if bytes.len() > 25 * 1024 * 1024 {
            return Err("Image de référence trop volumineuse".into());
        }
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name("image.png")
            .mime_str("image/png")
            .map_err(|e| e.to_string())?;
        let form = reqwest::multipart::Form::new()
            .text("model", model.clone())
            .text("prompt", prompt.clone())
            .text("size", size.clone())
            .text("n", "1")
            .text("response_format", "b64_json")
            .part("image", part);
        auth(
            state.client.post(endpoint(&p, "images/edits")),
            &p,
            &secret(&p.id)?,
        )?
        .multipart(form)
    } else {
        auth(
            state.client.post(endpoint(&p, "images/generations")),
            &p,
            &secret(&p.id)?,
        )?
        .json(
            &json!({"model":model,"prompt":prompt,"n":1,"size":size,"response_format":"b64_json"}),
        )
    };
    let response = request
        .send()
        .await
        .map_err(|_| "Moteur d’images inaccessible".to_string())?;
    let body: Value = checked(response)
        .await?
        .json()
        .await
        .map_err(|_| "Réponse image invalide")?;
    let mut images = vec![];
    for item in body["data"].as_array().ok_or("Aucune image retournée")? {
        if let Some(data) = item["b64_json"].as_str() {
            images.push(format!("data:image/png;base64,{data}"));
        } else if let Some(link) = item["url"].as_str() {
            let u = url::Url::parse(link).map_err(|_| "URL d’image invalide")?;
            if !u.username().is_empty() || u.password().is_some() {
                return Err("URL d’image avec identifiants refusée".into());
            }
            if u.scheme() != "https" {
                return Err("URL d’image non sécurisée".into());
            }
            let response = state
                .client
                .get(u)
                .send()
                .await
                .map_err(|_| "Téléchargement image impossible")?;
            let response = checked(response).await?;
            let mut stream = response.bytes_stream();
            let mut bytes = vec![];
            while let Some(chunk) = stream.next().await {
                bytes.extend_from_slice(&chunk.map_err(|_| "Image incomplète")?);
                if bytes.len() > 25 * 1024 * 1024 {
                    return Err("Image trop volumineuse".into());
                }
            }
            images.push(format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ));
        }
    }
    if images.is_empty() {
        return Err("Le moteur n’a retourné aucune image".into());
    }
    Ok(images)
}
async fn comfy_image(
    state: &AppState,
    p: &Provider,
    model: &str,
    prompt: &str,
    size: &str,
    image: Option<String>,
) -> Result<Vec<String>, String> {
    let secret = secret(&p.id)?;
    let (width, height) = size.split_once('x').ok_or("Dimensions invalides")?;
    let width: u32 = width.parse().map_err(|_| "Largeur invalide")?;
    let height: u32 = height.parse().map_err(|_| "Hauteur invalide")?;
    let mut graph = json!({
        "3":{"class_type":"KSampler","inputs":{"cfg":7,"denoise":1,"latent_image":["5",0],"model":["4",0],"negative":["7",0],"positive":["6",0],"sampler_name":"euler","scheduler":"normal","seed":(uuid::Uuid::new_v4().as_u128() % 9007199254740991u128) as u64,"steps":20}},
        "4":{"class_type":"CheckpointLoaderSimple","inputs":{"ckpt_name":model}},
        "5":{"class_type":"EmptyLatentImage","inputs":{"batch_size":1,"height":height,"width":width}},
        "6":{"class_type":"CLIPTextEncode","inputs":{"clip":["4",1],"text":prompt}},
        "7":{"class_type":"CLIPTextEncode","inputs":{"clip":["4",1],"text":""}},
        "8":{"class_type":"VAEDecode","inputs":{"samples":["3",0],"vae":["4",2]}},
        "9":{"class_type":"SaveImage","inputs":{"filename_prefix":"Converge","images":["8",0]}}
    });
    let mut uploaded_image = String::new();
    if let Some(data) = image {
        let (_, data) = data.split_once(',').ok_or("Image de référence invalide")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|_| "Image de référence invalide")?;
        if bytes.len() > 25 * 1024 * 1024 {
            return Err("Image de référence trop volumineuse".into());
        }
        let filename = format!("converge-{}.png", uuid::Uuid::new_v4());
        let form = reqwest::multipart::Form::new().part(
            "image",
            reqwest::multipart::Part::bytes(bytes)
                .file_name(filename)
                .mime_str("image/png")
                .map_err(|e| e.to_string())?,
        );
        let response = auth(state.client.post(endpoint(p, "upload/image")), p, &secret)?
            .multipart(form)
            .send()
            .await
            .map_err(|_| "Envoi de l’image à ComfyUI impossible")?;
        let value: Value = checked(response)
            .await?
            .json()
            .await
            .map_err(|_| "Réponse upload ComfyUI invalide")?;
        let name = value["name"]
            .as_str()
            .ok_or("ComfyUI n’a pas confirmé l’image")?;
        uploaded_image = name.to_string();
        graph["10"] = json!({"class_type":"LoadImage","inputs":{"image":name}});
        graph["5"] = json!({"class_type":"VAEEncode","inputs":{"pixels":["10",0],"vae":["4",2]}});
        graph["3"]["inputs"]["denoise"] = json!(0.7);
    }
    if let Some(template) = p.capabilities["workflow"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
    {
        if template.len() > 1024 * 1024 {
            return Err("Workflow trop volumineux".into());
        }
        graph = serde_json::from_str(template)
            .map_err(|_| "Le workflow ComfyUI doit être un objet JSON au format API")?;
        let variables = json!({"prompt":prompt,"model":model,"width":width,"height":height,"seed":(uuid::Uuid::new_v4().as_u128()%9007199254740991u128)as u64,"image":uploaded_image});
        interpolate_workflow(&mut graph, &variables, 0)?;
        if !graph.is_object() {
            return Err("Le workflow doit contenir un objet de nœuds au format API".into());
        }
    }
    let response = auth(state.client.post(endpoint(p, "prompt")), p, &secret)?
        .json(&json!({"prompt":graph,"client_id":uuid::Uuid::new_v4().to_string()}))
        .send()
        .await
        .map_err(|_| "ComfyUI inaccessible")?;
    let value: Value = checked(response)
        .await?
        .json()
        .await
        .map_err(|_| "Réponse ComfyUI invalide")?;
    let prompt_id=value["prompt_id"].as_str().ok_or("Workflow refusé par ComfyUI : vérifiez la compatibilité du checkpoint avec les nœuds standards")?;
    for _ in 0..600 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let response = auth(
            state
                .client
                .get(endpoint(p, &format!("history/{prompt_id}"))),
            p,
            &secret,
        )?
        .send()
        .await
        .map_err(|_| "Suivi ComfyUI interrompu")?;
        let history: Value = checked(response)
            .await?
            .json()
            .await
            .map_err(|_| "Historique ComfyUI invalide")?;
        let entry = &history[prompt_id];
        if entry.is_null() {
            continue;
        }
        if entry["status"]["status_str"] == "error" {
            return Err("Le workflow ComfyUI a échoué. Vérifiez le checkpoint et la mémoire GPU dans ComfyUI.".into());
        }
        if let Some(outputs) = entry["outputs"].as_object() {
            let mut result = vec![];
            for output in outputs.values() {
                if let Some(images) = output["images"].as_array() {
                    for image in images {
                        let mut u =
                            url::Url::parse(&endpoint(p, "view")).map_err(|e| e.to_string())?;
                        u.query_pairs_mut()
                            .append_pair("filename", image["filename"].as_str().unwrap_or(""))
                            .append_pair("subfolder", image["subfolder"].as_str().unwrap_or(""))
                            .append_pair("type", image["type"].as_str().unwrap_or("output"));
                        let response = auth(state.client.get(u), p, &secret)?
                            .send()
                            .await
                            .map_err(|_| "Lecture de l’image ComfyUI impossible")?;
                        let bytes = checked(response)
                            .await?
                            .bytes()
                            .await
                            .map_err(|_| "Image ComfyUI incomplète")?;
                        if bytes.len() > 25 * 1024 * 1024 {
                            return Err("Image ComfyUI trop volumineuse".into());
                        }
                        result.push(format!(
                            "data:image/png;base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(bytes)
                        ));
                    }
                }
            }
            if !result.is_empty() {
                return Ok(result);
            }
        }
    }
    Err("Délai ComfyUI dépassé après 10 minutes".into())
}
fn interpolate_workflow(value: &mut Value, variables: &Value, depth: u8) -> Result<(), String> {
    if depth > 32 {
        return Err("Workflow trop profondément imbriqué".into());
    }
    match value {
        Value::String(text) => {
            for key in ["prompt", "model", "width", "height", "seed", "image"] {
                let marker = format!("{{{{{key}}}}}");
                if text == &marker {
                    *value = variables[key].clone();
                    return Ok(());
                }
            }
            for key in ["prompt", "model", "image"] {
                *text = text.replace(
                    &format!("{{{{{key}}}}}"),
                    variables[key].as_str().unwrap_or(""),
                );
            }
        }
        Value::Array(array) => {
            for item in array {
                interpolate_workflow(item, variables, depth + 1)?
            }
        }
        Value::Object(object) => {
            for item in object.values_mut() {
                interpolate_workflow(item, variables, depth + 1)?
            }
        }
        _ => {}
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn deepseek_account_uses_its_own_header_without_api_key_fallback() {
        let p: super::Provider = serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"name":"DeepSeek","plugin":"deepseek","protocol":"anthropic","url":"https://api.deepseek.com","headers":{},"models":[],"enabled":true,"capabilities":{},"request":{"deepseek":{"authMode":"oauth"},"chatPath":"anthropic/v1/messages"}})).unwrap();
        let secret = super::Secret {
            key: "owned-account-token".into(),
            headers: std::collections::HashMap::from([(
                "Authorization".into(),
                "unrelated-api-key".into(),
            )]),
        };
        let request = super::auth(
            reqwest::Client::new().post("https://api.deepseek.com/anthropic/v1/messages"),
            &p,
            &secret,
        )
        .unwrap()
        .build()
        .unwrap();
        assert_eq!(request.headers()["x-dsh-auth-token"], "owned-account-token");
        assert!(!request.headers().contains_key("Authorization"));
        assert!(!request.headers().contains_key("x-api-key"));
    }
    #[test]
    fn opencode_errors_do_not_blame_keys_for_plan_or_free_tier_limits() {
        assert!(opencode_error(
            403,
            br#"{"error":{"name":"FreeTierError","message":"secret-test-key"}}"#
        )
        .contains("application officielle"));
        assert!(
            opencode_error(403, br#"{"error":{"message":"Go subscription required"}}"#)
                .contains("abonnement Go")
        );
        assert!(
            opencode_error(429, br#"{"error":{"name":"FreeUsageLimitError"}}"#)
                .contains("quota gratuit")
        );
        assert!(
            !opencode_error(403, br#"{"secret":"secret-test-key"}"#).contains("secret-test-key")
        );
    }
    #[tokio::test]
    #[ignore = "Uses the configured OpenCode connection for a minimal diagnostic request"]
    async fn live_opencode_diagnostic() {
        assert_eq!(
            std::env::var("CONVERGE_OPENCODE_DIAGNOSTIC").as_deref(),
            Ok("1")
        );
        let dbpath = std::path::PathBuf::from(std::env::var_os("APPDATA").unwrap())
            .join("app.converge.desktop/converge.sqlite");
        let db = rusqlite::Connection::open_with_flags(
            dbpath,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let data: String = db
            .query_row(
                "SELECT data FROM providers WHERE json_extract(data,'$.plugin')='opencode' LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let mut p: Provider = serde_json::from_str(&data).unwrap();
        let (protocol, path) =
            opencode_route("beam-free", &p.protocol, p.url.ends_with("/go")).unwrap();
        p.protocol = protocol.into();
        let secret = secret(&p.id).unwrap();
        let response=auth(reqwest::Client::new().post(endpoint(&p,&path)).header("User-Agent","Converge/0.1.0").header("x-opencode-session",uuid::Uuid::new_v4().to_string()).timeout(std::time::Duration::from_secs(25)),&p,&secret).unwrap().json(&json!({"model":"beam-free","messages":[{"role":"user","content":"Reply OK"}],"max_tokens":16,"stream":false})).send().await.unwrap();
        let status = response.status().as_u16();
        match checked_provider(response, &p).await {
            Ok(_) => println!("OpenCode diagnostic: HTTP {status}, request accepted"),
            Err(reason) => println!("OpenCode diagnostic: HTTP {status}, {reason}"),
        };
    }
    use super::*;
    #[test]
    fn rejects_credential_leaks() {
        assert!(validate_url("http://example.com/v1").is_err());
        assert!(validate_url("https://secret@example.com").is_err());
        assert!(validate_url("https://example.com?key=x").is_err());
        assert!(validate_url("http://localhost:11434").is_ok());
    }
    #[test]
    fn reasoning_effort_uses_real_protocol_fields_and_auto_omits_it() {
        let mut body = json!({"temperature":0.7,"max_tokens":4096});
        apply_effort(&mut body, "openai", "gpt-6-luna", Some("high")).unwrap();
        assert_eq!(body["reasoning_effort"], "high");
        assert!(body.get("temperature").is_none());
        assert_eq!(body["max_completion_tokens"], 4096);
        let mut body = json!({});
        apply_effort(&mut body, "anthropic", "claude", Some("medium")).unwrap();
        assert_eq!(body["output_config"]["effort"], "medium");
        let mut body = json!({});
        apply_effort(&mut body, "ollama", "gpt-oss:20b", Some("low")).unwrap();
        assert_eq!(body["think"], "low");
        assert!(apply_effort(&mut body, "ollama", "qwen", Some("high")).is_err());
        let mut body = json!({});
        apply_effort(&mut body, "openai", "model", Some("auto")).unwrap();
        assert_eq!(body, json!({}));
        assert!(apply_effort(&mut body, "openai", "model", Some("fake")).is_err());
    }
    #[test]
    fn reads_protocol_deltas() {
        assert_eq!(
            delta(&json!({"message":{"content":"é"}}), "ollama"),
            Some("é".into())
        );
        assert_eq!(
            delta(&json!({"delta":{"text":"ok"}}), "anthropic"),
            Some("ok".into())
        );
    }

    fn state_for(protocol: &str, url: String) -> (AppState, String) {
        let db = crate::persistence::open(std::path::Path::new(":memory:")).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let p = json!({"id":id,"name":"Protocol contract fixture","plugin":if protocol=="responses"{"zai"}else{"custom"},"protocol":protocol,"url":url,"headers":{},"models":["contract-model"],"capabilities":{"tools":true,"vision":true,"images":true,"context":32768},"enabled":true});
        db.execute(
            "INSERT INTO providers(id,data) VALUES(?1,?2)",
            rusqlite::params![id, p.to_string()],
        )
        .unwrap();
        (
            AppState {
                db: std::sync::Mutex::new(db),
                client: reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .unwrap(),
                requests: std::sync::Mutex::new(Default::default()),

                terminal_gate: tokio::sync::Mutex::new(()),
                artifacts: std::sync::Mutex::new(Default::default()),
                computer_target: Default::default(),
                permissions: Default::default(),
            },
            id,
        )
    }
    async fn fixture(
        body: String,
        delay: bool,
    ) -> (String, tokio::task::JoinHandle<(String, Vec<u8>)>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut data = vec![];
            let header_end;
            loop {
                let mut b = [0u8; 4096];
                let n = socket.read(&mut b).await.unwrap();
                assert!(n > 0);
                data.extend_from_slice(&b[..n]);
                if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = i + 4;
                    break;
                }
            }
            let header = String::from_utf8_lossy(&data[..header_end]).to_string();
            let length = header
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            while data.len() < header_end + length {
                let mut b = [0u8; 4096];
                let n = socket.read(&mut b).await.unwrap();
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&b[..n]);
            }
            let response=format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
            socket.write_all(response.as_bytes()).await.unwrap();
            for part in body.as_bytes().chunks(7) {
                if socket.write_all(part).await.is_err() {
                    break;
                }
                if delay {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await
                }
            }
            (header, data[header_end..].to_vec())
        });
        (url, task)
    }
    fn channel() -> (Channel<Value>, std::sync::Arc<std::sync::Mutex<Vec<Value>>>) {
        let events = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
        let captured = events.clone();
        (
            Channel::new(move |body| {
                if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                    captured
                        .lock()
                        .unwrap()
                        .push(serde_json::from_str(&s).unwrap());
                }
                Ok(())
            }),
            events,
        )
    }
    #[tokio::test]
    async fn real_http_stream_contracts_for_all_three_protocols() {
        for (protocol,body,path) in [
            ("ollama","{\"message\":{\"content\":\"été\"},\"done\":true}","/api/chat"),
            ("openai","data: {\"choices\":[{\"delta\":{\"content\":\"été\"}}]}\r\n\r\ndata: [DONE]\r\n\r\n","/chat/completions"),
            ("anthropic","event: content_block_delta\ndata: {\"delta\":{\"text\":\"été\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n","/messages"),
            ("responses","data: {\"type\":\"response.output_text.delta\",\"delta\":\"été\"}\n\ndata: {\"type\":\"response.completed\"}\n\n","/responses"),
        ]{
            let(url,server)=fixture(body.into(),false).await;let(state,pid)=state_for(protocol,url);let(channel,events)=channel();
            stream_chat_inner(&state,Request{conversation_id:None,id:uuid::Uuid::new_v4().to_string(),provider_id:pid,model:"contract-model".into(),messages:vec![json!({"role":"user","content":"Bonjour"})],temperature:0.7,max_tokens:512,reasoning_effort:None,tools_enabled:None},channel).await.unwrap();
            let(header,body)=server.await.unwrap();assert!(header.starts_with(&format!("POST {path} ")),"{header}");let request:Value=serde_json::from_slice(&body).unwrap();assert_eq!(request["stream"],true);assert_eq!(request["model"],"contract-model");assert!(request["tools"].is_array());if protocol=="responses"{assert!(request["input"].is_array());assert!(request.get("messages").is_none());assert_eq!(request["tools"][0]["type"],"function");}
            let events=events.lock().unwrap();assert!(events.iter().any(|v|v["type"]=="delta"&&v["text"]=="été"),"{events:?}");assert_eq!(events.last().unwrap()["type"],"done");assert!(state.requests.lock().unwrap().is_empty());
        }
    }
    #[tokio::test]
    async fn cancellation_interrupts_real_http_stream_and_removes_request() {
        let body = format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{}\"}}}}]}}\n\n",
            "x".repeat(1000)
        );
        let (url, server) = fixture(body, true).await;
        let (state, pid) = state_for("openai", url);
        let (channel, events) = channel();
        let id = uuid::Uuid::new_v4().to_string();
        let request = Request {
            conversation_id: None,
            id: id.clone(),
            provider_id: pid,
            model: "contract-model".into(),
            messages: vec![],
            temperature: 0.7,
            max_tokens: 512,
            reasoning_effort: None,
            tools_enabled: None,
        };
        let cancel = async {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            state
                .requests
                .lock()
                .unwrap()
                .iter()
                .find(|(key, _)| key.ends_with(&id))
                .unwrap()
                .1
                .cancel()
        };
        let (result, _) = tokio::join!(stream_chat_inner(&state, request, channel), cancel);
        result.unwrap();
        server.abort();
        assert_eq!(events.lock().unwrap().last().unwrap()["cancelled"], true);
        assert!(state.requests.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn incomplete_stream_is_reported_without_losing_partial_text() {
        let (url, server) = fixture(
            "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n".into(),
            false,
        )
        .await;
        let (state, pid) = state_for("openai", url);
        let (channel, events) = channel();
        let result = stream_chat_inner(
            &state,
            Request {
                conversation_id: None,
                id: uuid::Uuid::new_v4().to_string(),
                provider_id: pid,
                model: "contract-model".into(),
                messages: vec![],
                temperature: 0.7,
                max_tokens: 512,
                reasoning_effort: None,
                tools_enabled: None,
            },
            channel,
        )
        .await;
        server.await.unwrap();
        assert!(result.is_err());
        assert!(events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e["text"] == "partial"));
    }
    #[tokio::test]
    async fn openai_image_edit_uses_multipart_and_generation_uses_json() {
        let expected="iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aB1cAAAAASUVORK5CYII=";
        for edit in [false, true] {
            let (url, server) =
                fixture(json!({"data":[{"b64_json":expected}]}).to_string(), false).await;
            let (state, pid) = state_for("openai", url);
            let images = generate_image_inner(
                &state,
                pid,
                "contract-model".into(),
                "Test".into(),
                "1024x1024".into(),
                edit.then(|| format!("data:image/png;base64,{expected}")),
            )
            .await
            .unwrap();
            assert_eq!(images.len(), 1);
            let (header, body) = server.await.unwrap();
            if edit {
                assert!(header.starts_with("POST /images/edits "));
                assert!(header.contains("multipart/form-data"));
                assert!(String::from_utf8_lossy(&body).contains("name=\"image\""));
            } else {
                assert!(header.starts_with("POST /images/generations "));
                assert_eq!(
                    serde_json::from_slice::<Value>(&body).unwrap()["prompt"],
                    "Test"
                );
            }
        }
    }
}
