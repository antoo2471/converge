//! Subscription transports run unmodified official clients. Credentials stay with
//! those clients; stdout is parsed, stderr is never surfaced or logged.
use crate::{persistence::Provider, providers::Request};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Mutex,
};
use tauri::{ipc::Channel, Manager};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};
use tokio_util::sync::CancellationToken;

pub struct CliProviders {
    root: PathBuf,
    logins: Mutex<HashMap<String, std::process::Child>>,
}
impl CliProviders {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            logins: Default::default(),
        }
    }
}
impl Drop for CliProviders {
    fn drop(&mut self) {
        if let Ok(logins) = self.logins.get_mut() {
            for child in logins.values_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}
pub async fn remove(app: &tauri::AppHandle, p: &Provider) -> Result<(), String> {
    validate(p)?;
    let runtime = app.state::<CliProviders>();
    if let Some(mut child) = runtime
        .logins
        .lock()
        .map_err(|_| "Connexion occupée")?
        .remove(&p.id)
    {
        let _ = child.kill();
        let _ = child.wait();
    }
    // Delete only this connection's private client profile.
    let home = runtime.root.join(&p.id);
    if home.exists() {
        let root = runtime
            .root
            .canonicalize()
            .map_err(|_| "Profils indisponibles")?;
        let path = home.canonicalize().map_err(|_| "Profil indisponible")?;
        if !path.starts_with(&root) || path == root {
            return Err("Chemin du profil refusé".into());
        }
        std::fs::remove_dir_all(path)
            .map_err(|_| "Fermez le client officiel avant de supprimer sa connexion")?;
    }
    Ok(())
}
pub fn supports(plugin: &str) -> bool {
    matches!(
        plugin,
        "claude-code" | "grok" | "kimi" | "minimax" | "antigravity"
    )
}
fn binary_name(plugin: &str) -> Result<&'static str, String> {
    match plugin {
        "claude-code" => Ok("claude"),
        "grok" => Ok("grok"),
        "kimi" => Ok("kimi"),
        "minimax" => Ok("mmx"),
        "antigravity" => Ok("agy"),
        _ => Err("Client officiel inconnu".into()),
    }
}
pub fn validate(p: &Provider) -> Result<(), String> {
    if !supports(&p.plugin)
        || p.protocol != "cli"
        || uuid::Uuid::parse_str(&p.id).is_err()
        || p.name.trim().is_empty()
        || p.name.len() > 120
        || p.models.len() > 1000
        || p.models
            .iter()
            .any(|m| m.is_empty() || m.len() > 500 || m.contains(['\0', '\n', '\r']))
        || !p.headers.is_empty()
        || p.capabilities["images"] != false
        || !p.capabilities["vision"].is_boolean()
        || !p.capabilities["tools"].is_boolean()
        || !p.capabilities["context"]
            .as_u64()
            .is_some_and(|n| (512..=2_000_000).contains(&n))
        || (p.plugin != "claude-code" && p.capabilities["vision"] == true)
    {
        return Err("Configuration du client officiel invalide".into());
    }
    if p.request["cli"]["executable"]
        .as_str()
        .is_some_and(|s| s.len() > 2000 || s.contains(['\0', '\n', '\r']))
    {
        return Err("Chemin de client invalide".into());
    }
    Ok(())
}
struct Program {
    executable: PathBuf,
    prefix: Vec<String>,
}
fn resolve(p: &Provider) -> Result<Program, String> {
    let name = binary_name(&p.plugin)?;
    let custom = p.request["cli"]["executable"]
        .as_str()
        .filter(|s| !s.trim().is_empty());
    let mut candidates = vec![];
    if let Some(path) = custom {
        candidates.push(PathBuf::from(path));
    } else {
        for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
            candidates.push(dir.join(format!("{name}.exe")));
            candidates.push(dir.join(name));
            if name == "mmx" {
                candidates.push(dir.join("node_modules/mmx-cli"));
            }
        }
        if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
            let home = PathBuf::from(home);
            for dir in [
                ".local/bin",
                ".grok/bin",
                "AppData/Local/Microsoft/WinGet/Links",
                "AppData/Roaming/npm",
            ] {
                candidates.push(home.join(dir).join(format!("{name}.exe")));
                if name == "mmx" {
                    candidates.push(home.join(dir).join("node_modules/mmx-cli"));
                }
            }
        }
    }
    for path in candidates {
        // npm's .cmd wrapper is not a shell transport. Resolve its declared JS entry
        // and run Node directly, so messages/models can never become shell syntax.
        let package = if path.extension().is_some_and(|s| s == "cmd") {
            path.parent()
                .unwrap_or(Path::new("."))
                .join("node_modules/mmx-cli")
        } else {
            path.clone()
        };
        if name == "mmx" && package.join("package.json").is_file() {
            let manifest: Value = serde_json::from_slice(
                &std::fs::read(package.join("package.json"))
                    .map_err(|_| "Manifeste mmx illisible")?,
            )
            .map_err(|_| "Manifeste mmx invalide")?;
            let entry = manifest["bin"]["mmx"]
                .as_str()
                .or_else(|| manifest["bin"].as_str())
                .ok_or("Entrée mmx absente")?;
            let package = package
                .canonicalize()
                .map_err(|_| "Paquet mmx introuvable")?;
            let script = package
                .join(entry)
                .canonicalize()
                .map_err(|_| "Entrée mmx introuvable")?;
            if !script.starts_with(&package) {
                return Err("Entrée mmx hors du paquet".into());
            }
            if let Some(node) = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|d| d.join(if cfg!(windows) { "node.exe" } else { "node" }))
                .find(|p| p.is_file())
            {
                return Ok(Program {
                    executable: node,
                    prefix: vec![script
                        .to_string_lossy()
                        .strip_prefix("\\\\?\\")
                        .unwrap_or(&script.to_string_lossy())
                        .to_owned()],
                });
            }
        }
        if path.is_file()
            && !path
                .extension()
                .is_some_and(|s| s == "cmd" || s == "bat" || s == "ps1")
        {
            return Ok(Program {
                executable: path.canonicalize().map_err(|_| "Client introuvable")?,
                prefix: vec![],
            });
        }
    }
    Err(format!(
        "Le client officiel {name} est introuvable. Installez-le ou indiquez son exécutable."
    ))
}
fn profile(root: &Path, p: &Provider) -> Result<PathBuf, String> {
    validate(p)?;
    let home = root.join(&p.id);
    std::fs::create_dir_all(&home)
        .map_err(|_| "Impossible de préparer le profil du fournisseur")?;
    if p.plugin == "antigravity" {
        let config = home.join(".gemini/antigravity-cli");
        std::fs::create_dir_all(&config).map_err(|_| "Profil Antigravity indisponible")?;
        // The official CLI's Deny policy takes priority over workspace defaults.
        let path = config.join("settings.json");
        let mut settings: Value = if path.exists() {
            serde_json::from_slice(
                &std::fs::read(&path).map_err(|_| "Configuration Antigravity illisible")?,
            )
            .map_err(|_| "Configuration Antigravity invalide")?
        } else {
            json!({})
        };
        if !settings.is_object() {
            return Err("Configuration Antigravity invalide".into());
        }
        let permissions = json!({"deny":["*"],"allow":[],"ask":[]});
        if settings["permissions"] != permissions {
            // Preserve auth selection and all other settings written by the
            // official login. Polling must not overwrite an in-progress login.
            settings["permissions"] = permissions;
            std::fs::write(
                path,
                serde_json::to_vec(&settings).map_err(|_| "Politique Antigravity invalide")?,
            )
            .map_err(|_| "Politique Antigravity indisponible")?;
        }
    }
    Ok(home)
}
fn environment(command: &mut std::process::Command, p: &Provider, home: &Path) {
    let key = match p.plugin.as_str() {
        "claude-code" => Some("CLAUDE_CONFIG_DIR"),
        "grok" => Some("GROK_HOME"),
        "kimi" => Some("KIMI_SHARE_DIR"),
        "minimax" => Some("MMX_CONFIG_DIR"),
        _ => None,
    };
    if let Some(key) = key {
        command.env(key, home);
    }
    if p.plugin == "grok" {
        // GROK_HOME alone does not isolate Claude/Cursor compatibility scanners.
        command.env("USERPROFILE", home).env("HOME", home);
        let disabled = json!({"skills":false,"rules":false,"agents":false,"mcps":false,"hooks":false,"sessions":false});
        command.env(
            "GROK_CONFIG",
            json!({"compat":{"cursor":disabled,"claude":disabled,"codex":disabled},"hooks":{}})
                .to_string(),
        );
    }
    if p.plugin == "antigravity" {
        // Verified against the installed CLI: all .gemini configuration and state
        // are created under this process-local home, not the user's profile.
        command.env("USERPROFILE", home).env("HOME", home);
        for key in [
            "ANTIGRAVITY_GEMINI_DIR",
            "ANTIGRAVITY_CONFIG_REL_DIR",
            "ANTIGRAVITY_APP_DATA_DIR",
            "ANTIGRAVITY_PERM_GRANTS",
        ] {
            command.env_remove(key);
        }
    }
    // Never accidentally charge API credentials inherited from the IDE.
    for key in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "XAI_API_KEY",
        "XAI_BASE_URL",
        "MINIMAX_API_KEY",
        "MINIMAX_BASE_URL",
        "KIMI_API_KEY",
        "KIMI_BASE_URL",
        "MOONSHOT_API_KEY",
        "GOOGLE_API_KEY",
        "GEMINI_API_KEY",
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        "GEMINI_BASE_URL",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "GOOGLE_GENAI_USE_VERTEXAI",
        "GOOGLE_GENAI_USE_ENTERPRISE",
    ] {
        command.env_remove(key);
    }
    command.env("NO_COLOR", "1");
}
fn command(p: &Provider, home: &Path) -> Result<Command, String> {
    let program = resolve(p)?;
    let mut command = std::process::Command::new(program.executable);
    command.args(program.prefix);
    environment(&mut command, p, home);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut command = Command::from(command);
    command
        .current_dir(home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    Ok(command)
}
async fn capture(mut command: Command) -> Result<String, String> {
    let mut child = command
        .spawn()
        .map_err(|_| "Impossible de lancer le client officiel")?;
    let mut output = child
        .stdout
        .take()
        .ok_or("Sortie du client indisponible")?
        .take(2 * 1024 * 1024 + 1);
    let mut bytes = vec![];
    tokio::time::timeout(
        std::time::Duration::from_secs(25),
        output.read_to_end(&mut bytes),
    )
    .await
    .map_err(|_| "Le client officiel ne répond pas. Vérifiez sa connexion.")?
    .map_err(|_| "Sortie du client interrompue")?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Catalogue du client trop volumineux".into());
    }
    let status = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
        .await
        .map_err(|_| "Le client ne se termine pas")?
        .map_err(|_| "Client interrompu")?;
    if !status.success() {
        return Err("Le client officiel a refusé la demande. Vérifiez votre connexion et les mises à jour du client.".into());
    }
    String::from_utf8(bytes).map_err(|_| "Sortie du client non UTF-8".into())
}
fn login_args(plugin: &str) -> &'static [&'static str] {
    match plugin {
        "claude-code" => &["auth", "login", "--claudeai"],
        "grok" => &["login", "--oauth"],
        "kimi" => &["login"],
        "minimax" => &["auth", "login", "--recommend", "--region=global"],
        _ => &[],
    }
}
#[tauri::command]
pub async fn cli_provider_action(
    app: tauri::AppHandle,
    provider: Provider,
    action: String,
) -> Result<Value, String> {
    validate(&provider)?;
    let runtime = app.state::<CliProviders>();
    let home = profile(&runtime.root, &provider)?;
    if action == "cancelLogin" {
        if let Some(mut child) = runtime
            .logins
            .lock()
            .map_err(|_| "Connexion occupée")?
            .remove(&provider.id)
        {
            let _ = child.kill();
            let _ = child.wait();
        }
        return Ok(json!({"available":resolve(&provider).is_ok(),"login":"cancelled"}));
    }
    let program = match resolve(&provider) {
        Ok(p) => p,
        Err(e) => return Ok(json!({"available":false,"message":e})),
    };
    if action == "login" {
        let mut logins = runtime.logins.lock().map_err(|_| "Connexion occupée")?;
        if let Some(child) = logins.get_mut(&provider.id) {
            if child
                .try_wait()
                .map_err(|_| "Connexion interrompue")?
                .is_none()
            {
                return Ok(json!({"available":true,"login":"running"}));
            }
        }
        let mut cmd = std::process::Command::new(program.executable);
        cmd.args(program.prefix)
            .args(login_args(&provider.plugin))
            .current_dir(&home);
        environment(&mut cmd, &provider, &home);
        // These are interactive OAuth clients, not background services. They own
        // their browser callback/device flow and may require a native console.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x00000010);
        }
        let child = cmd
            .spawn()
            .map_err(|_| "Impossible d’ouvrir la connexion du client officiel")?;
        logins.insert(provider.id.clone(), child);
        return Ok(json!({"available":true,"login":"running"}));
    }
    if action == "logout" {
        if provider.plugin == "antigravity" {
            remove(&app, &provider).await?;
            return Ok(json!({"available":true,"authenticated":false}));
        }
        let mut cmd = command(&provider, &home)?;
        if matches!(provider.plugin.as_str(), "claude-code" | "minimax") {
            cmd.args(["auth", "logout"]);
        } else {
            cmd.arg("logout");
        }
        capture(cmd).await?;
        return Ok(json!({"available":true,"authenticated":false}));
    }
    if action != "status" {
        return Err("Action de connexion inconnue".into());
    }
    let login = {
        let mut logins = runtime.logins.lock().map_err(|_| "Connexion occupée")?;
        if let Some(child) = logins.get_mut(&provider.id) {
            match child.try_wait().map_err(|_| "Connexion interrompue")? {
                None => "running",
                Some(status) if status.success() => "completed",
                _ => "failed",
            }
        } else {
            "idle"
        }
    };
    let mut result = json!({"available":true,"login":login});
    if matches!(provider.plugin.as_str(), "claude-code" | "minimax") && login != "running" {
        let mut cmd = command(&provider, &home)?;
        cmd.args(["auth", "status"]);
        if provider.plugin == "minimax" {
            cmd.args(["--output", "json"]);
        }
        if let Ok(output) = capture(cmd).await {
            if let Ok(value) = serde_json::from_str::<Value>(&output) {
                // Only retain a boolean, never an account record or token/key.
                result["authenticated"] = json!(
                    value["loggedIn"] == true
                        || value["authMethod"]
                            .as_str()
                            .is_some_and(|s| matches!(s, "claude.ai" | "oauth_token"))
                        || value["method"] == "oauth"
                );
            }
        }
    }
    Ok(result)
}
pub async fn discover(app: &tauri::AppHandle, p: &Provider) -> Result<Vec<String>, String> {
    let runtime = app.state::<CliProviders>();
    let home = profile(&runtime.root, p)?;
    if !matches!(p.plugin.as_str(), "grok" | "antigravity") {
        // These clients resolve the account's default themselves. Do not invent
        // an entitlement catalogue or read private credential/config files.
        resolve(p)?;
        return Ok(if p.plugin == "claude-code" {
            vec!["sonnet".into(), "opus".into(), "haiku".into()]
        } else {
            vec!["default".into()]
        });
    }
    let mut cmd = command(p, &home)?;
    cmd.arg("models");
    let output = capture(cmd).await?;
    let mut models = vec![];
    for line in output.lines() {
        let s = line.trim().trim_start_matches(['*', '>', ' ']);
        if let Some(id) = s.split_whitespace().next() {
            if id.len() < 150
                && id.contains('-')
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
                && !id.starts_with('-')
            {
                models.push(id.to_owned());
            }
        }
    }
    models.sort();
    models.dedup();
    if models.is_empty() {
        return Err("Le client n’a pas renvoyé de catalogue lisible. Utilisez le modèle du compte ou saisissez son identifiant.".into());
    }
    Ok(models)
}
fn transcript(messages: &[Value]) -> String {
    let messages: Vec<Value> = messages
        .iter()
        .map(|m| json!({"role":m["role"],"content":m["content"]}))
        .collect();
    format!("Respond to the final user request in this Converge conversation. The system entries contain the Converge instructions and tool protocol. Use only that protocol for tools; Converge executes them. Conversation:\n{}",serde_json::to_string(&messages).unwrap_or_default())
}
fn image_blocks(messages: &[Value]) -> Vec<Value> {
    messages
        .iter()
        .flat_map(|m| m["attachments"].as_array().into_iter().flatten())
        .filter_map(|a| {
            let mime = a["mime"].as_str()?;
            let data = a["data"].as_str()?;
            if !matches!(
                mime,
                "image/png" | "image/jpeg" | "image/webp" | "image/gif"
            ) {
                return None;
            }
            Some(json!({"type":"image","source":{"type":"base64","media_type":mime,"data":data}}))
        })
        .collect()
}
/// Parser shared by the Claude/Grok Anthropic wire stream and the other CLI
/// envelopes. Completed messages are a fallback only, never duplicated deltas.
#[derive(Default)]
struct Output {
    done: bool,
    text: String,
    partial: bool,
    native_tools_denied: bool,
}
fn client_error(plugin: &str, value: &Value) -> String {
    // Inspect only bounded error data and return fixed copy; never expose a
    // server's raw message, which can contain tokens or request content.
    let mut parts = Vec::new();
    for entry in [
        &value["message"],
        &value["error"],
        &value["errors"],
        &value["result"],
    ] {
        if !entry.is_null() {
            parts.push(entry.to_string());
        }
    }
    let text = parts
        .join(" ")
        .chars()
        .take(32768)
        .collect::<String>()
        .to_ascii_lowercase();
    if plugin == "grok"
        && (text.contains("upgrade required")
            || text.contains("outdated")
            || text.contains("http_status\\\": 426"))
    {
        return "Le client Grok installé est trop ancien pour le service (HTTP 426). Mettez-le à jour avec « grok update --stable », puis réessayez.".into();
    }
    if text.contains("401")
        || text.contains("unauthorized")
        || text.contains("authentication required")
        || text.contains("not logged in")
    {
        return "Le client officiel demande une reconnexion. Ouvrez ce fournisseur et reconnectez votre compte.".into();
    }
    if text.contains("429")
        || text.contains("rate limit")
        || text.contains("quota")
        || text.contains("balance exhausted")
    {
        return "Le service a atteint une limite d’utilisation. Vérifiez le quota de votre compte dans le client officiel.".into();
    }
    if text.contains("model")
        && [
            "not found",
            "not available",
            "not allowed",
            "not supported",
            "unknown model",
        ]
        .iter()
        .any(|s| text.contains(s))
    {
        return "Ce modèle n’est pas accessible avec cette connexion. Actualisez les modèles dans la configuration du fournisseur.".into();
    }
    "Le client officiel a signalé une erreur. Vérifiez le compte, le modèle et le quota dans son application.".into()
}
impl Output {
    fn ingest(&mut self, plugin: &str, v: &Value) -> Result<Vec<Value>, String> {
        if plugin == "grok"
            && v["type"] == "system"
            && v["subtype"] == "init"
            && v["tools"].as_array().is_none_or(|tools| !tools.is_empty())
        {
            return Err("Grok n’a pas confirmé la désactivation de ses outils internes. Mettez à jour le client officiel avant de réessayer.".into());
        }
        if v["is_error"] == true
            || v["type"] == "error"
            || (v["event"] == "result" && v["result"]["status"] != "SUCCESS")
        {
            return Err(client_error(plugin, v));
        }
        let mut events = vec![];
        let e = if v["type"] == "stream_event" {
            &v["event"]
        } else {
            v
        };
        if e["type"] == "content_block_delta" {
            if let Some(text) = e["delta"]["thinking"].as_str() {
                events.push(json!({"type":"reasoning","text":text}));
            }
            if let Some(text) = e["delta"]["text"].as_str() {
                self.partial = true;
                self.text.push_str(text);
                events.push(json!({"type":"delta","text":text}));
            }
        }
        if plugin == "antigravity" {
            if v["event"] == "init"
                && !self.native_tools_denied
                && v["init"]["tools"].as_array().is_none_or(|t| !t.is_empty())
            {
                return Err("Le client Antigravity n’a pas désactivé ses outils internes. Mettez-le à jour avant de le connecter à Converge.".into());
            }
            if let Some(text) = v["step_update"]["text_delta"].as_str() {
                self.partial = true;
                self.text.push_str(text);
                events.push(json!({"type":"delta","text":text}));
            }
            if v["event"] == "result" {
                self.done = true;
                if !self.partial {
                    if let Some(text) = v["result"]["response"].as_str() {
                        self.text.push_str(text);
                        events.push(json!({"type":"delta","text":text}));
                    }
                }
            }
        } else {
            let m = if v["type"] == "assistant" {
                &v["message"]
            } else {
                v
            };
            if m["role"] == "assistant" && !self.partial {
                let text = if let Some(s) = m["content"].as_str() {
                    s.to_owned()
                } else {
                    m["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|b| b["type"] == "text")
                        .filter_map(|b| b["text"].as_str())
                        .collect::<String>()
                };
                self.text.push_str(&text);
                if !text.is_empty() {
                    events.push(json!({"type":"delta","text":text}));
                }
            }
            if v["type"] == "result" {
                self.done = true;
                if self.text.is_empty() {
                    if let Some(text) = v["result"].as_str() {
                        self.text.push_str(text);
                        events.push(json!({"type":"delta","text":text}));
                    }
                }
            }
            if e["type"] == "message_stop" {
                self.done = true;
            }
        }
        Ok(events)
    }
}
pub async fn stream(
    app: &tauri::AppHandle,
    p: &Provider,
    r: &Request,
    channel: &Channel<Value>,
    cancel: CancellationToken,
) -> Result<(), String> {
    let runtime = app.state::<CliProviders>();
    let home = profile(&runtime.root, p)?;
    if !p.models.contains(&r.model)
        || uuid::Uuid::parse_str(&r.id).is_err()
        || r.messages.len() > 500
    {
        return Err("Requête du client officiel invalide".into());
    }
    let cwd = home.join("requests").join(&r.id);
    std::fs::create_dir_all(&cwd).map_err(|_| "Dossier de requête indisponible")?;
    let result = stream_inner(p, r, channel, cancel, &home, &cwd).await;
    // Only generated request files live here, never account credentials.
    let _ = std::fs::remove_dir_all(&cwd);
    result
}
async fn stream_inner(
    p: &Provider,
    r: &Request,
    channel: &Channel<Value>,
    cancel: CancellationToken,
    home: &Path,
    cwd: &Path,
) -> Result<(), String> {
    let mut cmd = command(p, home)?;
    cmd.current_dir(cwd).stdin(Stdio::piped());
    let prompt = transcript(&r.messages);
    let images = image_blocks(&r.messages);
    if !images.is_empty() && p.plugin != "claude-code" {
        return Err("Ce transport de compte ne prend pas en charge les images. Choisissez une connexion avec vision.".into());
    }
    let mut payload = String::new();
    match p.plugin.as_str() {
        "claude-code" => {
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
            let mut content = vec![json!({"type":"text","text":prompt})];
            content.extend(images);
            payload = json!({"type":"user","message":{"role":"user","content":content}})
                .to_string()
                + "\n";
        }
        "grok" => {
            let file = cwd.join("prompt.txt");
            std::fs::write(&file, prompt).map_err(|_| "Contexte Grok indisponible")?;
            // Grok treats an empty allowlist as its default tools. A denylist
            // wins over the allowlist and also removes its always-on MCP helpers.
            cmd.args(["--prompt-file"]).arg(file).args([
                "--tools",
                "search_tool",
                "--disallowed-tools",
                "search_tool,use_tool",
                "--disable-web-search",
                "--no-subagents",
                "--permission-mode",
                "dontAsk",
                "--max-turns",
                "1",
                "--output-format",
                "streaming-messages-json",
                "--include-partial-messages",
            ]);
        }
        "kimi" => {
            std::fs::write(cwd.join("system.md"),"You are the Converge assistant. Follow the supplied conversation instructions. Native tools are disabled; use Converge tools only.").map_err(|_|"Instructions Kimi indisponibles")?;
            let agent = cwd.join("agent.yaml");
            std::fs::write(&agent,"version: 1\nagent:\n  name: converge\n  system_prompt_path: system.md\n  tools: []\n  subagents: {}\n").map_err(|_|"Agent Kimi indisponible")?;
            cmd.arg("--agent-file").arg(agent).args([
                "--print",
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
                "--max-steps-per-turn",
                "1",
                "--max-ralph-iterations",
                "0",
                "--mcp-config",
                "{\"mcpServers\":{}}",
            ]);
            payload = json!({"role":"user","content":prompt}).to_string() + "\n";
        }
        "minimax" => {
            cmd.args([
                "text",
                "chat",
                "--messages-file",
                "-",
                "--stream",
                "--output",
                "text",
                "--max-tokens",
            ])
            .arg(r.max_tokens.to_string());
            payload = serde_json::to_string(
                &r.messages
                    .iter()
                    .map(|m| json!({"role":m["role"],"content":m["content"]}))
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| "Contexte MiniMax invalide")?;
        }
        "antigravity" => {
            let agent_dir = cwd.join(".agents/agents");
            std::fs::create_dir_all(&agent_dir).map_err(|_| "Agent Antigravity indisponible")?;
            std::fs::write(agent_dir.join("converge.md"),"---\nname: converge\ndescription: Converge chat transport\ntools: []\nmainAgent: true\nsubagent: false\ncommandExecutionPolicy: off\nmcpServers: []\nskills: []\nplugins: []\n---\nRespond using the supplied Converge conversation and tool protocol.\n").map_err(|_|"Agent Antigravity indisponible")?;
            cmd.args([
                "--agent",
                "converge",
                "--disable-slash-commands",
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
            ]);
            // Wait for init and verify zero native tools before transmitting data.
            payload = json!({"event":"user","message":{"content":prompt}}).to_string() + "\n";
        }
        _ => return Err("Client officiel non supporté".into()),
    }
    if r.model != "default" {
        cmd.args(["--model", &r.model]);
    }
    if let Some(effort) = r.reasoning_effort.as_deref().filter(|e| *e != "auto") {
        if matches!(p.plugin.as_str(), "claude-code" | "grok" | "antigravity")
            && ["low", "medium", "high", "xhigh", "max"].contains(&effort)
        {
            cmd.arg(if p.plugin == "grok" {
                "--reasoning-effort"
            } else {
                "--effort"
            })
            .arg(effort);
        } else {
            return Err(
                "Ce client ne propose pas cet effort de raisonnement. Choisissez Auto.".into(),
            );
        }
    }
    let mut child = cmd
        .spawn()
        .map_err(|_| "Impossible de démarrer le client officiel")?;
    let mut input = Some(child.stdin.take().ok_or("Entrée du client indisponible")?);
    let stdout = child.stdout.take().ok_or("Sortie du client indisponible")?;
    let native_tools_denied = if p.plugin == "antigravity" {
        let settings: Value = serde_json::from_slice(
            &std::fs::read(home.join(".gemini/antigravity-cli/settings.json"))
                .map_err(|_| "Politique Antigravity absente")?,
        )
        .map_err(|_| "Politique Antigravity invalide")?;
        if settings["permissions"]["deny"] != json!(["*"]) {
            return Err("Politique Antigravity modifiée : reconnectez le fournisseur".into());
        }
        true
    } else {
        false
    };
    let mut output = Output {
        native_tools_denied,
        ..Output::default()
    };
    if p.plugin != "antigravity" {
        let mut stdin = input.take().unwrap();
        tokio::select! {_=cancel.cancelled()=>{child.kill().await.ok();return Ok(())},r=tokio::time::timeout(std::time::Duration::from_secs(30),stdin.write_all(payload.as_bytes()))=>r.map_err(|_|"Le client ne lit pas le contexte")?.map_err(|_|"Contexte refusé par le client")?};
        drop(stdin); // Close the pipe: shutdown() alone does not deliver EOF on Windows.
    }
    let mut reader = BufReader::new(stdout);
    let mut total = 0usize;
    if p.plugin == "minimax" {
        // mmx's official text transport streams raw UTF-8, not JSON. Decode only
        // complete code points, retaining fragmented multibyte sequences.
        let mut pending = vec![];
        let mut buffer = [0u8; 8192];
        loop {
            let n = tokio::select! {_=cancel.cancelled()=>{child.kill().await.ok();return Ok(())}, n=tokio::time::timeout(std::time::Duration::from_secs(120),reader.read(&mut buffer))=>n.map_err(|_|"MiniMax ne répond plus")?.map_err(|_|"Flux MiniMax interrompu")?};
            if n == 0 {
                break;
            }
            total += n;
            if total > 32 * 1024 * 1024 {
                return Err("Réponse trop volumineuse".into());
            }
            pending.extend_from_slice(&buffer[..n]);
            let len = match std::str::from_utf8(&pending) {
                Ok(_) => pending.len(),
                Err(e) if e.error_len().is_none() => e.valid_up_to(),
                Err(_) => return Err("Flux MiniMax non UTF-8".into()),
            };
            if len > 0 {
                let text = std::str::from_utf8(&pending[..len]).unwrap();
                output.text.push_str(text);
                channel
                    .send(json!({"type":"delta","text":text}))
                    .map_err(|_| "Conversation fermée")?;
                pending.drain(..len);
            }
        }
        if !pending.is_empty() {
            return Err("Flux MiniMax tronqué".into());
        }
        output.done = true;
    } else {
        loop {
            let mut bytes = vec![];
            let mut limited = (&mut reader).take(4 * 1024 * 1024 + 1);
            let n = tokio::select! {_=cancel.cancelled()=>{child.kill().await.ok();return Ok(())}, n=tokio::time::timeout(std::time::Duration::from_secs(120),limited.read_until(b'\n',&mut bytes))=>n.map_err(|_|"Le client ne répond plus depuis deux minutes")?.map_err(|_|"Flux du client interrompu")?};
            if n == 0 {
                break;
            }
            total += n;
            if n > 4 * 1024 * 1024 || total > 32 * 1024 * 1024 {
                return Err("Réponse du client trop volumineuse".into());
            }
            if bytes.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
                "Le client n’a pas renvoyé le protocole JSON attendu. Mettez-le à jour."
            })?;
            for event in output.ingest(&p.plugin, &value)? {
                channel.send(event).map_err(|_| "Conversation fermée")?;
            }
            if p.plugin == "antigravity" && value["event"] == "init" {
                let mut stdin = input.take().ok_or("Initialisation Antigravity répétée")?;
                tokio::select! {_=cancel.cancelled()=>{child.kill().await.ok();return Ok(())},r=tokio::time::timeout(std::time::Duration::from_secs(30),stdin.write_all(payload.as_bytes()))=>r.map_err(|_|"Antigravity ne lit pas le contexte")?.map_err(|_|"Contexte Antigravity refusé")?};
                drop(stdin);
            }
        }
    }
    let status = tokio::select! {_=cancel.cancelled()=>{child.kill().await.ok();return Ok(())},s=tokio::time::timeout(std::time::Duration::from_secs(15),child.wait())=>s.map_err(|_|"Le client ne termine pas sa réponse")?.map_err(|_|"Client interrompu")?};
    if !status.success() {
        return Err(format!("Le client officiel a échoué (code {}). Vérifiez la connexion, les droits sur le modèle et le quota dans son application.",status.code().unwrap_or(-1)));
    }
    if !output.done && p.plugin != "kimi" {
        return Err("Le client a terminé sans confirmer la fin de la réponse".into());
    }
    if output.text.trim().is_empty() {
        return Err("Le client n’a renvoyé aucun contenu".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outdated_grok_is_reported_without_exposing_the_server_payload() {
        let error=Output::default().ingest("grok",&json!({"type":"result","is_error":true,"errors":["API error (status 426 Upgrade Required): Grok CLI outdated. private-token"]})).unwrap_err();
        assert!(error.contains("426"));
        assert!(error.contains("grok update"));
        assert!(!error.contains("private-token"));
    }
    #[test]
    fn grok_must_confirm_that_no_native_tools_are_available() {
        assert!(Output::default()
            .ingest(
                "grok",
                &json!({"type":"system","subtype":"init","tools":["write"]})
            )
            .is_err());
        assert!(Output::default()
            .ingest(
                "grok",
                &json!({"type":"system","subtype":"init","tools":[]})
            )
            .is_ok());
    }
    #[test]
    fn grok_does_not_inherit_foreign_hook_configuration() {
        let p:Provider=serde_json::from_value(json!({"id":uuid::Uuid::new_v4().to_string(),"name":"Grok","plugin":"grok","protocol":"cli","url":"https://grok.com","models":[],"capabilities":{},"enabled":true})).unwrap();
        let mut cmd = std::process::Command::new("grok");
        let home = Path::new("C:/isolated-converge-grok");
        environment(&mut cmd, &p, home);
        let env: HashMap<_, _> = cmd.get_envs().collect();
        assert_eq!(
            env[std::ffi::OsStr::new("USERPROFILE")],
            Some(home.as_os_str())
        );
        assert_eq!(
            env[std::ffi::OsStr::new("GROK_HOME")],
            Some(home.as_os_str())
        );
        let config: Value = serde_json::from_str(
            env[std::ffi::OsStr::new("GROK_CONFIG")]
                .unwrap()
                .to_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(config["compat"]["cursor"]["hooks"], false);
        assert_eq!(config["compat"]["claude"]["hooks"], false);
    }
    #[tokio::test]
    #[ignore = "Requires a Converge-owned Grok OAuth profile and consumes one inference turn"]
    async fn live_grok_converge_stream() {
        let pid = std::env::var("CONVERGE_GROK_TEST_PROFILE")
            .expect("Set CONVERGE_GROK_TEST_PROFILE to the Converge provider UUID");
        assert!(uuid::Uuid::parse_str(&pid).is_ok());
        let root = PathBuf::from(std::env::var_os("APPDATA").unwrap()).join("app.converge.desktop");
        let db = rusqlite::Connection::open_with_flags(
            root.join("converge.sqlite"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let data: String = db
            .query_row("SELECT data FROM providers WHERE id=?1", [&pid], |row| {
                row.get(0)
            })
            .unwrap();
        let p: Provider = serde_json::from_str(&data).unwrap();
        assert_eq!(p.plugin, "grok");
        assert!(p.models.contains(&"grok-4.7".into()));
        let request:Request=serde_json::from_value(json!({"id":uuid::Uuid::new_v4().to_string(),"providerId":pid,"model":"grok-4.7","messages":[{"role":"user","content":"Reply with exactly OK. Do not use tools."}],"temperature":0.7,"maxTokens":64,"toolsEnabled":false})).unwrap();
        let home = profile(&root.join("accounts"), &p).unwrap();
        let cwd = home.join("requests").join(&request.id);
        std::fs::create_dir_all(&cwd).unwrap();
        let events = std::sync::Arc::new(Mutex::new(Vec::<Value>::new()));
        let captured = events.clone();
        let channel = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                captured
                    .lock()
                    .unwrap()
                    .push(serde_json::from_str(&s).unwrap());
            }
            Ok(())
        });
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            stream_inner(
                &p,
                &request,
                &channel,
                CancellationToken::new(),
                &home,
                &cwd,
            ),
        )
        .await;
        std::fs::remove_dir_all(&cwd).unwrap();
        result.unwrap().unwrap();
        let text = events
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v["type"] == "delta")
            .filter_map(|v| v["text"].as_str())
            .collect::<String>();
        assert_eq!(text.trim(), "OK");
    }
    #[test]
    fn partial_messages_are_not_duplicated() {
        let mut out = Output::default();
        assert_eq!(out.ingest("claude-code",&json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Salut"}}})).unwrap()[0]["text"],"Salut");
        assert!(out.ingest("claude-code",&json!({"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Salut"}]}})).unwrap().is_empty());
        out.ingest(
            "claude-code",
            &json!({"type":"result","result":"Salut","is_error":false}),
        )
        .unwrap();
        assert!(out.done);
        assert_eq!(out.text, "Salut");
    }
    #[test]
    fn antigravity_tools_are_checked_before_input() {
        assert!(Output::default()
            .ingest(
                "antigravity",
                &json!({"event":"init","init":{"tools":["run_command"]}})
            )
            .is_err());
        assert!(Output::default()
            .ingest("antigravity", &json!({"event":"init","init":{"tools":[]}}))
            .is_ok());
    }
    #[test]
    fn errors_do_not_expose_client_payloads() {
        let error = Output::default()
            .ingest(
                "grok",
                &json!({"type":"error","error":{"token":"secret-token"}}),
            )
            .unwrap_err();
        assert!(!error.contains("secret-token"));
    }
    #[test]
    fn login_uses_subscription_not_api_auth() {
        assert!(login_args("grok").contains(&"--oauth"));
        assert!(login_args("claude-code").contains(&"--claudeai"));
        assert!(login_args("minimax").contains(&"--recommend"));
    }
    #[test]
    fn antigravity_permission_setup_preserves_official_auth_settings() {
        let root = std::env::temp_dir().join(format!(
            "converge-profile-contract-{}",
            uuid::Uuid::new_v4()
        ));
        let p:Provider=serde_json::from_value(json!({"id":uuid::Uuid::new_v4().to_string(),"name":"Account","plugin":"antigravity","protocol":"cli","url":"https://antigravity.google","headers":{},"models":["default"],"enabled":true,"capabilities":{"vision":false,"tools":true,"images":false,"context":32768}})).unwrap();
        let home = profile(&root, &p).unwrap();
        let settings = home.join(".gemini/antigravity-cli/settings.json");
        std::fs::write(&settings,r#"{"security":{"auth":{"selectedType":"oauth-personal"}},"permissions":{"allow":["*"]}}"#).unwrap();
        profile(&root, &p).unwrap();
        let value: Value = serde_json::from_slice(&std::fs::read(settings).unwrap()).unwrap();
        assert_eq!(value["security"]["auth"]["selectedType"], "oauth-personal");
        assert_eq!(value["permissions"]["deny"], json!(["*"]));
        assert_eq!(value["permissions"]["allow"], json!([]));
        std::fs::remove_dir_all(root).unwrap();
    }
    fn mmx_fixture(
        script: &str,
    ) -> (
        PathBuf,
        Provider,
        Request,
        Channel<Value>,
        std::sync::Arc<Mutex<Vec<Value>>>,
    ) {
        let root =
            std::env::temp_dir().join(format!("converge-cli-contract-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("package.json"),
            r#"{"bin":{"mmx":"fixture.cjs"}}"#,
        )
        .unwrap();
        std::fs::write(root.join("fixture.cjs"), script).unwrap();
        let p:Provider=serde_json::from_value(json!({"id":uuid::Uuid::new_v4().to_string(),"plugin":"minimax","protocol":"cli","name":"Contract","url":"https://platform.minimax.io","headers":{},"models":["default"],"enabled":true,"capabilities":{"vision":false,"tools":true,"images":false,"context":32768},"request":{"cli":{"executable":root.to_string_lossy()}}})).unwrap();
        let r:Request=serde_json::from_value(json!({"id":uuid::Uuid::new_v4().to_string(),"providerId":p.id,"model":"default","messages":[{"role":"system","content":"instructions"},{"role":"user","content":"bonjour"}],"temperature":0.7,"maxTokens":512})).unwrap();
        let events = std::sync::Arc::new(Mutex::new(vec![]));
        let captured = events.clone();
        let channel = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                captured
                    .lock()
                    .unwrap()
                    .push(serde_json::from_str(&s).unwrap());
            }
            Ok(())
        });
        (root, p, r, channel, events)
    }
    #[tokio::test]
    async fn official_client_transport_preserves_history_and_fragmented_utf8() {
        let (root, p, r, channel, events) = mmx_fixture(
            r#"
let input=''; process.stdin.on('data', d=>input+=d); process.stdin.on('end',()=>{
 const m=JSON.parse(input);
 if(m[0].role!=='system') process.exit(2);
 if(m[1].content!=='bonjour') process.exit(3);
 if(!process.argv.includes('--messages-file')) process.exit(4);
 const bytes=Buffer.from('Été 🐈');
 let i=0; const timer=setInterval(()=>{if(i===bytes.length){clearInterval(timer);return;}process.stdout.write(bytes.subarray(i,i+1));i++;},3);
});
"#,
        );
        stream_inner(&p, &r, &channel, CancellationToken::new(), &root, &root)
            .await
            .unwrap();
        let text = events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|v| v["text"].as_str())
            .collect::<String>();
        assert_eq!(text, "Été 🐈");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn cancellation_stops_a_stalled_client_process() {
        let (root, p, r, channel, _) =
            mmx_fixture("process.stdin.resume();setInterval(()=>{},1000);");
        let cancel = CancellationToken::new();
        let later = cancel.clone();
        let stop = async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            later.cancel();
        };
        let (result, _) = tokio::join!(stream_inner(&p, &r, &channel, cancel, &root, &root), stop);
        result.unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
