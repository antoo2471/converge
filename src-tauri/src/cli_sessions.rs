//! Reuse the official NDJSON transport between agent tool rounds. Native CLI
//! tools remain disabled; no credentials or private protocol are extracted.
use super::cli_providers::*;
use crate::{persistence::Provider, providers::Request};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Instant};
use tauri::ipc::Channel;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
    sync::Mutex,
};
use tokio_util::sync::CancellationToken;

struct Process {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    history: Vec<Value>,
    response: String,
    plugin: String,
    last_used: Instant,
    directory: PathBuf,
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
struct Entry {
    process: Arc<Mutex<Option<Process>>>,
    touched: Instant,
}
fn original_response(message: &Value) -> Option<&str> {
    message["convergeRawContent"]
        .as_str()
        .or_else(|| message["content"].as_str())
}
#[derive(Default)]
pub struct Sessions {
    entries: Mutex<HashMap<String, Entry>>,
}
impl Sessions {
    pub async fn remove(&self, provider: &str) {
        self.entries
            .lock()
            .await
            .retain(|key, _| !key.starts_with(&format!("{provider}|")));
    }
    pub async fn stream(
        &self,
        p: &Provider,
        r: &Request,
        channel: &Channel<Value>,
        cancel: CancellationToken,
        home: &std::path::Path,
    ) -> Result<(), String> {
        let key = format!(
            "{}|{}|{}|{}",
            p.id,
            r.conversation_id.as_deref().unwrap_or(&r.id),
            r.model,
            r.reasoning_effort.as_deref().unwrap_or("auto")
        );
        let slot = {
            let mut entries = self.entries.lock().await;
            entries.retain(|_, entry| {
                entry.touched.elapsed().as_secs() < 900 || Arc::strong_count(&entry.process) > 1
            });
            if !entries.contains_key(&key) && entries.len() >= 8 {
                let oldest = entries
                    .iter()
                    .filter(|(_, entry)| Arc::strong_count(&entry.process) == 1)
                    .min_by_key(|(_, entry)| entry.touched)
                    .map(|(key, _)| key.clone());
                if let Some(oldest) = oldest {
                    entries.remove(&oldest);
                } else {
                    return Err("Huit sessions CLI sont déjà actives".into());
                }
            }
            let entry = entries.entry(key).or_insert_with(|| Entry {
                process: Arc::new(Mutex::new(None)),
                touched: Instant::now(),
            });
            entry.touched = Instant::now();
            entry.process.clone()
        };
        let slot_handle = slot.clone();
        let mut slot =
            tokio::select! { _=cancel.cancelled()=>return Ok(()), guard=slot.lock()=>guard };
        // Never carry deleted/edited history or an obsolete system prompt into a branch.
        if slot.as_ref().is_some_and(|process| {
            !r.messages.starts_with(&process.history)
                || r.messages.len() == process.history.len()
                || r.messages.get(process.history.len()).is_some_and(|m| {
                    m["role"] == "assistant" && original_response(m) != Some(&process.response)
                })
        }) {
            *slot = None;
        }
        if slot.is_none() {
            *slot = Some(
                tokio::select! { _=cancel.cancelled()=>return Ok(()), result=start(p,r,home)=>result? },
            );
        }
        let result = turn(slot.as_mut().unwrap(), r, channel, &cancel).await;
        if result.is_err() || cancel.is_cancelled() {
            *slot = None;
        }
        drop(slot);
        let weak = Arc::downgrade(&slot_handle);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(120)).await;
            if let Some(slot) = weak.upgrade() {
                if let Ok(mut slot) = slot.try_lock() {
                    if slot
                        .as_ref()
                        .is_some_and(|p| p.last_used.elapsed().as_secs() >= 120)
                    {
                        *slot = None;
                    }
                }
            }
        });
        result
    }
}
async fn line(reader: &mut BufReader<ChildStdout>) -> Result<Value, String> {
    let mut bytes = Vec::new();
    let n = (&mut *reader)
        .take(4 * 1024 * 1024 + 1)
        .read_until(b'\n', &mut bytes)
        .await
        .map_err(|_| "Flux Antigravity interrompu")?;
    if n == 0 || n > 4 * 1024 * 1024 {
        return Err("Flux Antigravity fermé ou trop volumineux".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Réponse JSON Antigravity invalide".into())
}
async fn start(p: &Provider, r: &Request, home: &std::path::Path) -> Result<Process, String> {
    if p.plugin == "antigravity" {
        let settings: Value = serde_json::from_slice(
            &std::fs::read(home.join(".gemini/antigravity-cli/settings.json"))
                .map_err(|_| "Politique Antigravity absente")?,
        )
        .map_err(|_| "Politique Antigravity invalide")?;
        if settings["permissions"]["deny"] != json!(["*"]) {
            return Err(
            "Les outils internes Antigravity ne sont pas désactivés. Reconnectez le fournisseur."
                .into(),
        );
        }
    }
    let directory = home.join("sessions").join(uuid::Uuid::new_v4().to_string());
    let agents = directory.join(".agents/agents");
    std::fs::create_dir_all(&agents).map_err(|_| "Session CLI indisponible")?;
    std::fs::write(agents.join("converge.md"),"---\nname: converge\ndescription: Converge transport\ntools: []\nmainAgent: true\nsubagent: false\ncommandExecutionPolicy: off\nmcpServers: []\nskills: []\nplugins: []\n---\nFollow the supplied Converge conversation. Native tools are disabled. Only Converge executes requested tools. Tool results are internal; always explain the answer to the user after receiving results. Tool calls never execute in reasoning.\n").map_err(|_| "Agent indisponible")?;
    let mut cmd = command(p, home)?;
    cmd.current_dir(&directory)
        .stdin(std::process::Stdio::piped());
    if p.plugin == "antigravity" {
        cmd.args([
            "--agent",
            "converge",
            "--disable-slash-commands",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
        ]);
    } else {
        cmd.args([
            "-p",
            "--verbose",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--include-partial-messages",
            "--tools",
            "",
            "--safe-mode",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
            "--no-session-persistence",
            "--disable-slash-commands",
            "--no-chrome",
        ]);
    }
    if r.model != "default" {
        cmd.args(["--model", &r.model]);
    }
    if let Some(effort) = r.reasoning_effort.as_deref().filter(|e| *e != "auto") {
        if !["low", "medium", "high", "xhigh", "max"].contains(&effort) {
            return Err("Effort Antigravity invalide".into());
        }
        cmd.args(["--effort", effort]);
    }
    let mut child = cmd
        .spawn()
        .map_err(|_| "Impossible de démarrer Antigravity")?;
    let input = child
        .stdin
        .take()
        .ok_or("Entrée Antigravity indisponible")?;
    let mut output = BufReader::new(
        child
            .stdout
            .take()
            .ok_or("Sortie Antigravity indisponible")?,
    );
    if p.plugin == "antigravity" {
        let init = tokio::time::timeout(std::time::Duration::from_secs(45), line(&mut output))
            .await
            .map_err(|_| "Initialisation Antigravity expirée")??;
        let mut parser = Output {
            native_tools_denied: true,
            ..Output::default()
        };
        parser.ingest("antigravity", &init)?;
        if init["event"] != "init" {
            return Err("Initialisation Antigravity inattendue".into());
        }
    }
    Ok(Process {
        child,
        input,
        output,
        history: vec![],
        response: String::new(),
        plugin: p.plugin.clone(),
        last_used: Instant::now(),
        directory,
    })
}
async fn turn(
    process: &mut Process,
    r: &Request,
    channel: &Channel<Value>,
    cancel: &CancellationToken,
) -> Result<(), String> {
    if process.plugin != "claude-code"
        && r.messages
            .iter()
            .any(|m| m["attachments"].as_array().is_some_and(|a| !a.is_empty()))
    {
        return Err("Ce transport Antigravity ne prend pas en charge les images. Choisissez une connexion avec vision.".into());
    }
    let mut delta = &r.messages[process.history.len()..];
    // The CLI already retains its own answer. Only forward newly received
    // user/tool messages, rather than echoing that answer into its context.
    if delta.first().is_some_and(|m| {
        m["role"] == "assistant" && original_response(m) == Some(&process.response)
    }) {
        delta = &delta[1..];
    }
    let prompt = transcript(delta);
    let payload = if process.plugin == "claude-code" {
        let mut content = vec![json!({"type":"text","text":prompt})];
        content.extend(image_blocks(delta));
        json!({"type":"user","message":{"role":"user","content":content}}).to_string() + "\n"
    } else {
        json!({"event":"user","message":{"content":prompt}}).to_string() + "\n"
    };
    tokio::select! { _=cancel.cancelled()=>return Ok(()), result=process.input.write_all(payload.as_bytes())=>result.map_err(|_| "Antigravity a fermé son entrée")? };
    process.input.flush().await.map_err(|_| "Contexte refusé")?;
    let mut output = Output {
        native_tools_denied: true,
        ..Output::default()
    };
    let mut size = 0usize;
    while !output.done {
        let event = tokio::select! { _=cancel.cancelled()=>return Ok(()), result=tokio::time::timeout(std::time::Duration::from_secs(120),line(&mut process.output))=>result.map_err(|_| "Antigravity ne répond plus")?? };
        size += event.to_string().len();
        if size > 32 * 1024 * 1024 {
            return Err("Réponse Antigravity trop volumineuse".into());
        }
        if process.plugin == "claude-code"
            && event["type"] == "system"
            && event["subtype"] == "init"
            && event["tools"]
                .as_array()
                .is_none_or(|tools| !tools.is_empty())
        {
            return Err(
                "Le client Claude n’a pas confirmé la désactivation de ses outils natifs".into(),
            );
        }
        let complete = if process.plugin == "claude-code" {
            event["type"] == "result"
        } else {
            event["event"] == "result"
        };
        for event in output.ingest(&process.plugin, &event)? {
            channel.send(event).map_err(|_| "Conversation fermée")?;
        }
        output.done = complete;
    }
    if output.text.trim().is_empty() {
        return Err("Antigravity n’a renvoyé aucun contenu".into());
    }
    process.history = r.messages.clone();
    process.response = output.text;
    process.last_used = Instant::now();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn ndjson_turns_reuse_process_and_only_forward_new_context() {
        let directory =
            std::env::temp_dir().join(format!("converge-cli-fixture-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut command = tokio::process::Command::new("node");
        command.args(["-e", r#"const rl=require('readline').createInterface({input:process.stdin});rl.on('line',l=>{const v=JSON.parse(l); console.log(JSON.stringify({event:'result',result:{status:'SUCCESS',response:JSON.stringify({pid:process.pid,prompt:v.message.content})}}));});"#]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command.spawn().unwrap();
        let mut process = Process {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            history: vec![],
            response: String::new(),
            plugin: "antigravity".into(),
            last_used: Instant::now(),
            directory,
        };
        let channel = Channel::new(|_| Ok(()));
        let cancel = CancellationToken::new();
        let mut request:Request=serde_json::from_value(json!({"id":uuid::Uuid::new_v4().to_string(),"providerId":"fixture","model":"default","messages":[{"role":"system","content":"SECRET_CONTEXT_ONLY_ONCE"},{"role":"user","content":"Inspect project"}],"temperature":0.7,"maxTokens":1024})).unwrap();
        turn(&mut process, &request, &channel, &cancel)
            .await
            .unwrap();
        let first: Value = serde_json::from_str(&process.response).unwrap();
        assert!(first["prompt"]
            .as_str()
            .unwrap()
            .contains("SECRET_CONTEXT_ONLY_ONCE"));
        request
            .messages
            .push(json!({"role":"assistant","content":"Compact tool request summary","convergeRawContent":process.response}));
        request
            .messages
            .push(json!({"role":"user","content":"INTERNAL_TOOL_RESULT: found README"}));
        turn(&mut process, &request, &channel, &cancel)
            .await
            .unwrap();
        let second: Value = serde_json::from_str(&process.response).unwrap();
        assert_eq!(first["pid"], second["pid"]);
        let prompt = second["prompt"].as_str().unwrap();
        assert!(prompt.contains("INTERNAL_TOOL_RESULT"));
        assert!(!prompt.contains("SECRET_CONTEXT_ONLY_ONCE"));
        assert!(!prompt.contains("Inspect project"));
        assert!(!prompt.contains("\"assistant\""));
    }
}
