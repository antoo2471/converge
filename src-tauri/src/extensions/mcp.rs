use super::*;
use rmcp::{
    model::{
        CallToolRequestParams, ClientCapabilities, ClientConfig, GetPromptRequestParams,
        Implementation, ProtocolVersion, ReadResourceRequestParams,
    },
    service::RunningService,
    transport::{
        auth::{
            AuthClient, AuthError, AuthorizationManager, AuthorizationRequest, CredentialStore,
            OAuthState, StoredCredentials,
        },
        streamable_http_client::StreamableHttpClientTransportConfig,
        StreamableHttpClientTransport, TokioChildProcess,
    },
    RoleClient, ServiceExt,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex as AsyncMutex,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    #[serde(default = "enabled_by_default")]
    pub auto_connect: bool,
    pub transport: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: String,
    #[serde(default = "no_auth")]
    pub auth: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_port: Option<u16>,
}
fn enabled_by_default() -> bool {
    true
}
fn no_auth() -> String {
    "none".into()
}
#[derive(Default, Serialize, Deserialize)]
pub struct Secrets {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub client_secret: String,
}
#[derive(Default, Deserialize)]
pub struct SecretsPatch {
    pub token: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub env: Option<HashMap<String, String>>,
    pub client_secret: Option<String>,
}
fn merge_secret(mut value: Secrets, patch: SecretsPatch) -> Secrets {
    if let Some(v) = patch.token {
        value.token = v
    }
    if let Some(v) = patch.headers {
        value.headers = v
    }
    if let Some(v) = patch.env {
        value.env = v
    }
    if let Some(v) = patch.client_secret {
        value.client_secret = v
    }
    value
}
type Client = RunningService<RoleClient, ClientConfig>;
struct Session {
    client: Arc<Client>,
    tools: Vec<Value>,
    resources: Vec<Value>,
    prompts: Vec<Value>,
    vault: Option<Vault>,
}
struct AuthFlow {
    id: String,
    url: String,
    cancel: CancellationToken,
    vault: Vault,
    error: Option<String>,
    finished: bool,
}
#[derive(Default)]
struct Inner {
    sessions: AsyncMutex<HashMap<String, Session>>,
    auth: Mutex<HashMap<String, AuthFlow>>,
    connecting: AsyncMutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}
#[derive(Clone, Default)]
pub struct Runtime {
    inner: Arc<Inner>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(flows) = self.auth.get_mut() {
            for f in flows.values() {
                f.cancel.cancel();
                f.vault.invalidate();
            }
        }
        for s in self.sessions.get_mut().values() {
            s.client.cancellation_token().cancel();
            if let Some(v) = &s.vault {
                v.invalidate();
            }
        }
    }
}
fn entry(kind: &str, id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("app.converge.desktop", &format!("extensions/{kind}/{id}"))
        .map_err(|_| "Coffre des extensions indisponible".into())
}
fn clear_entry(kind: &str, id: &str) -> Result<(), String> {
    match entry(kind, id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Suppression du secret impossible".into()),
    }
}
fn secrets(id: &str) -> Result<Secrets, String> {
    match entry("mcp", id)?.get_password() {
        Ok(s) => serde_json::from_str(&s).map_err(|_| "Identifiants MCP invalides".into()),
        Err(keyring::Error::NoEntry) => Ok(Default::default()),
        Err(_) => Err("Coffre MCP inaccessible".into()),
    }
}
#[derive(Clone)]
struct Vault {
    id: String,
    url: String,
    valid: Arc<Mutex<bool>>,
}
impl Vault {
    fn new(c: &Config) -> Self {
        Self {
            id: c.id.clone(),
            url: c.url.clone(),
            valid: Arc::new(Mutex::new(true)),
        }
    }
    fn invalidate(&self) {
        if let Ok(mut valid) = self.valid.lock() {
            *valid = false;
        }
    }
}
#[async_trait::async_trait]
impl CredentialStore for Vault {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        let valid = self
            .valid
            .lock()
            .map_err(|_| AuthError::CredentialStoreError("Coffre occupé".into()))?;
        if !*valid {
            return Ok(None);
        }
        let entry = entry("mcp-oauth", &self.id).map_err(AuthError::CredentialStoreError)?;
        match entry.get_password() {
            Ok(s) => {
                let v: Value = serde_json::from_str(&s)
                    .map_err(|_| AuthError::CredentialStoreError("Coffre invalide".into()))?;
                if v["url"] != self.url {
                    return Ok(None);
                }
                serde_json::from_value(v["credentials"].clone())
                    .map(Some)
                    .map_err(|_| AuthError::CredentialStoreError("Autorisation invalide".into()))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AuthError::CredentialStoreError(
                "Coffre inaccessible".into(),
            )),
        }
    }
    async fn save(&self, value: StoredCredentials) -> Result<(), AuthError> {
        let valid = self
            .valid
            .lock()
            .map_err(|_| AuthError::CredentialStoreError("Coffre occupé".into()))?;
        if !*valid {
            return Err(AuthError::CredentialStoreError("Connexion retirée".into()));
        }
        entry("mcp-oauth", &self.id)
            .map_err(AuthError::CredentialStoreError)?
            .set_password(&json!({"url":self.url,"credentials":value}).to_string())
            .map_err(|_| AuthError::CredentialStoreError("Stockage sécurisé impossible".into()))
    }
    async fn clear(&self) -> Result<(), AuthError> {
        let _guard = self
            .valid
            .lock()
            .map_err(|_| AuthError::CredentialStoreError("Coffre occupé".into()))?;
        clear_entry("mcp-oauth", &self.id).map_err(AuthError::CredentialStoreError)
    }
}
fn load_config(state: &AppState, id: &str) -> Result<Config, String> {
    serde_json::from_value(super::record(state, "mcp", id)?)
        .map_err(|_| "Configuration MCP invalide".into())
}
fn validate(c: &Config) -> Result<(), String> {
    if c.redirect_port == Some(0) {
        return Err("Le port de retour OAuth doit être compris entre 1 et 65535".into());
    }
    if uuid::Uuid::parse_str(&c.id).is_err()
        || c.name.trim().is_empty()
        || c.name.len() > 120
        || !["http", "stdio"].contains(&c.transport.as_str())
        || !["none", "token", "oauth"].contains(&c.auth.as_str())
        || c.args.len() > 100
        || c.args.iter().any(|a| a.len() > 4000 || a.contains('\0'))
        || c.cwd.len() > 2000
        || c.client_id.len() > 2000
        || c.scopes.len() > 100
    {
        return Err("Configuration MCP invalide".into());
    }
    if c.transport == "http" {
        let url = url::Url::parse(&c.url).map_err(|_| "URL MCP invalide")?;
        let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
        if c.url.len() > 8192
            || !(url.scheme() == "https" || (url.scheme() == "http" && local))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.query_pairs().any(|(k, _)| {
                [
                    "token",
                    "key",
                    "api_key",
                    "apikey",
                    "access_token",
                    "authorization",
                    "secret",
                ]
                .contains(&k.to_ascii_lowercase().as_str())
            })
        {
            return Err("Utilisez HTTPS (ou HTTP sur localhost) et placez les identifiants dans les champs sécurisés.".into());
        }
    } else if c.command.trim().is_empty()
        || c.command.len() > 2000
        || c.command.contains(['\0', '\r', '\n'])
        || c.auth != "none"
    {
        return Err(
            "Indiquez un exécutable local ; son environnement configure l’authentification.".into(),
        );
    }
    Ok(())
}
fn info() -> ClientConfig {
    ClientConfig::new(
        ClientCapabilities::default(),
        Implementation::new("Converge", env!("CARGO_PKG_VERSION")),
    )
}
fn info_for(c: &Config) -> ClientConfig {
    let client = info();
    if url::Url::parse(&c.url).is_ok_and(|url| is_google_mcp(&url)) {
        // Google's current MCP servers implement the initialize lifecycle.
        // The SDK's latest default is 2026-07-28 (server/discover instead).
        client.with_protocol_version(ProtocolVersion::V_2025_03_26)
    } else {
        client
    }
}
fn http() -> Result<reqwest13::Client, String> {
    reqwest13::Client::builder()
        .redirect(reqwest13::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "Client MCP indisponible".into())
}
async fn auth_manager(c: &Config, vault: Vault) -> Result<AuthorizationManager, String> {
    let mut m = AuthorizationManager::new(&c.url)
        .await
        .map_err(|_| "Découverte OAuth MCP impossible. Vérifiez l’URL du serveur.")?;
    m.set_credential_store(vault);
    Ok(m)
}
async fn session(client: Client, vault: Option<Vault>) -> Result<Session, String> {
    let tools = tokio::time::timeout(Duration::from_secs(25), client.list_all_tools())
        .await
        .map_err(|_| "Le serveur ne fournit pas ses outils")?
        .map_err(|_| "Liste des outils MCP inaccessible")?;
    if tools.len() > 1000 {
        return Err("Le serveur déclare plus de 1 000 outils".into());
    }
    let tools = tools
        .into_iter()
        .map(|t| serde_json::to_value(t).unwrap_or(Value::Null))
        .collect::<Vec<_>>();
    if serde_json::to_vec(&tools)
        .map_err(|_| "Outils invalides")?
        .len()
        > 2 * 1024 * 1024
    {
        return Err("Les schémas MCP dépassent 2 Mo".into());
    }
    let caps = client.peer_info().map(|i| i.capabilities.clone());
    let resources = if caps.as_ref().is_some_and(|c| c.resources.is_some()) {
        tokio::time::timeout(Duration::from_secs(10), client.list_all_resources())
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default()
            .into_iter()
            .take(500)
            .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
            .collect()
    } else {
        vec![]
    };
    let prompts = if caps.as_ref().is_some_and(|c| c.prompts.is_some()) {
        tokio::time::timeout(Duration::from_secs(10), client.list_all_prompts())
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default()
            .into_iter()
            .take(500)
            .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
            .collect()
    } else {
        vec![]
    };
    Ok(Session {
        client: Arc::new(client),
        tools,
        resources,
        prompts,
        vault,
    })
}
fn local_command(c: &Config, s: &Secrets) -> Result<tokio::process::Command, String> {
    let mut command = tokio::process::Command::new(&c.command);
    // npm's extensionless/.cmd launchers are not Win32 executables. Resolve npm
    // through its installed Node entry, never interpolate command arguments.
    #[cfg(windows)]
    if ["npx", "npx.cmd", "npm", "npm.cmd"].contains(&c.command.to_ascii_lowercase().as_str()) {
        let node = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|p| p.join("node.exe"))
            .find(|p| p.is_file())
            .ok_or("Node.js est requis pour ce serveur npm")?;
        let script = node.parent().unwrap().join("node_modules/npm/bin").join(
            if c.command.starts_with("npx") {
                "npx-cli.js"
            } else {
                "npm-cli.js"
            },
        );
        if !script.is_file() {
            return Err(
                "Le lanceur npm est introuvable. Indiquez node.exe et le script du serveur.".into(),
            );
        }
        command = tokio::process::Command::new(node);
        command.arg(script);
    }
    command.args(&c.args).env_clear();
    for key in [
        "PATH",
        "PATHEXT",
        "SYSTEMROOT",
        "WINDIR",
        "COMSPEC",
        "USERPROFILE",
        "HOME",
        "APPDATA",
        "LOCALAPPDATA",
        "TEMP",
        "TMP",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .envs(&s.env)
        .stderr(Stdio::null())
        .kill_on_drop(true);
    if !c.cwd.is_empty() {
        command.current_dir(&c.cwd);
    } else {
        let cwd = std::env::temp_dir().join("converge-mcp").join(&c.id);
        std::fs::create_dir_all(&cwd).map_err(|_| "Dossier de travail MCP indisponible")?;
        command.current_dir(cwd);
    }
    #[cfg(windows)]
    {
        command.creation_flags(0x08000000);
    }
    Ok(command)
}
fn http_config(c: &Config, s: &Secrets) -> Result<StreamableHttpClientTransportConfig, String> {
    let mut cfg = StreamableHttpClientTransportConfig::with_uri(c.url.as_str());
    for (key, value) in &s.headers {
        let name = reqwest13::header::HeaderName::from_bytes(key.as_bytes())
            .map_err(|_| "Nom de header invalide")?;
        if [
            "host",
            "content-length",
            "connection",
            "transfer-encoding",
            "mcp-session-id",
            "mcp-protocol-version",
        ]
        .contains(&name.as_str())
            || (c.auth == "oauth" && name == "authorization")
        {
            return Err("Header MCP réservé".into());
        }
        cfg.custom_headers.insert(
            name,
            reqwest13::header::HeaderValue::from_str(value)
                .map_err(|_| "Valeur de header invalide")?,
        );
    }
    Ok(cfg)
}
async fn connect(c: &Config) -> Result<Session, String> {
    connect_with_credentials(c, secrets(&c.id)?).await
}
async fn connect_with_credentials(c: &Config, s: Secrets) -> Result<Session, String> {
    validate(c)?;
    if !c.enabled {
        return Err("Ce plugin est désactivé".into());
    }

    if c.transport == "stdio" {
        let transport = TokioChildProcess::new(local_command(c, &s)?).map_err(|_| {
            "Impossible de lancer le serveur MCP. Vérifiez l’exécutable et ses arguments."
        })?;
        let client = tokio::time::timeout(Duration::from_secs(40), info_for(&c).serve(transport))
            .await
            .map_err(|_| "Le serveur MCP ne répond pas après son lancement")?
            .map_err(|_| "Le serveur local n’a pas terminé l’initialisation MCP")?;
        return session(client, None).await;
    }
    let mut cfg = http_config(c, &s)?;
    if c.auth == "oauth" {
        let vault = Vault::new(c);
        let mut manager = auth_manager(c, vault.clone()).await?;
        if !manager
            .initialize_from_store()
            .await
            .map_err(|_| "La connexion OAuth ne peut pas être restaurée. Reconnectez le plugin.")?
        {
            return Err("Connectez ce plugin avec son compte OAuth dans Plugins.".into());
        }
        let transport =
            StreamableHttpClientTransport::with_client(AuthClient::new(http()?, manager), cfg);
        let client = tokio::time::timeout(Duration::from_secs(40), info_for(&c).serve(transport))
            .await
            .map_err(|_| "Connexion MCP expirée")?
            .map_err(|_| "Connexion MCP refusée. Reconnectez le compte dans Plugins.")?;
        return session(client, Some(vault)).await;
    }
    if c.auth == "token" {
        if s.token.is_empty() {
            return Err("Ajoutez le token du plugin".into());
        }
        cfg.auth_header = Some(s.token);
    }
    let transport = StreamableHttpClientTransport::with_client(http()?, cfg);
    let client = tokio::time::timeout(Duration::from_secs(40), info_for(&c).serve(transport))
        .await
        .map_err(|_| "Connexion MCP expirée")?
        .map_err(|e| {
            if e.auth_challenge().is_some() {
                "Ce serveur demande une connexion. Choisissez OAuth ou fournissez son token."
            } else {
                "Connexion MCP impossible. Vérifiez l’URL et le format Streamable HTTP."
            }
        })?;
    session(client, None).await
}
async fn ensure(rt: &Runtime, state: &AppState, id: &str) -> Result<Arc<Client>, String> {
    let gate = {
        let mut gates = rt.inner.connecting.lock().await;
        gates
            .entry(id.into())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    };
    let _gate = gate.lock().await;
    let c = load_config(state, id)?;
    if !c.enabled {
        return Err("Ce plugin est désactivé".into());
    }
    if let Some(s) = rt
        .inner
        .sessions
        .lock()
        .await
        .get(id)
        .filter(|s| !s.client.is_closed())
    {
        return Ok(s.client.clone());
    }
    let s = connect(&c).await?;
    let client = s.client.clone();
    rt.inner.sessions.lock().await.insert(id.into(), s);
    Ok(client)
}
async fn disconnect(rt: &Runtime, id: &str, clear: bool) -> Result<(), String> {
    if let Some(f) = rt
        .inner
        .auth
        .lock()
        .map_err(|_| "Connexion occupée")?
        .remove(id)
    {
        f.cancel.cancel();
        f.vault.invalidate();
    }
    if let Some(s) = rt.inner.sessions.lock().await.remove(id) {
        if let Some(v) = s.vault {
            v.invalidate();
        }
        s.client.cancellation_token().cancel();
    }
    if clear {
        clear_entry("mcp-oauth", id)?;
    }
    Ok(())
}
pub async fn restore_connections(app: tauri::AppHandle) {
    use futures_util::StreamExt;
    let state = app.state::<AppState>();
    let Ok(records) = super::records(&state, "mcp") else {
        return;
    };
    let configs = records
        .into_iter()
        .filter_map(|v| serde_json::from_value::<Config>(v).ok())
        .filter(|c| c.enabled && c.auto_connect && c.auth == "oauth" && c.transport == "http")
        .collect::<Vec<_>>();
    futures_util::stream::iter(configs)
        .for_each_concurrent(3, |c| {
            let app = app.clone();
            async move {
                let state = app.state::<AppState>();
                let runtime = app.state::<Extensions>();
                // Restore an existing grant only; never open a browser on startup.
                let vault = Vault::new(&c);
                if matches!(vault.load().await, Ok(Some(_))) {
                    let _ = ensure(&runtime.mcp, &state, &c.id).await;
                }
            }
        })
        .await;
}
pub async fn snapshot(rt: &Runtime, state: &AppState) -> Result<Vec<Value>, String> {
    let sessions = rt.inner.sessions.lock().await;
    let auth = rt.inner.auth.lock().map_err(|_| "Connexions occupées")?;
    Ok(super::records(state,"mcp")?.into_iter().map(|mut c|{let id=c["id"].as_str().unwrap_or("").to_owned();let s=sessions.get(&id).filter(|s|!s.client.is_closed());c["connected"]=json!(s.is_some());c["toolCount"]=json!(s.map(|s|s.tools.len()).unwrap_or(0));c["toolNames"]=json!(s.map(|s|s.tools.iter().take(100).map(|t|json!({"name":t["name"],"description":t["description"].as_str().unwrap_or("").chars().take(180).collect::<String>()})).collect::<Vec<_>>()).unwrap_or_default());if let Some(f)=auth.get(&id){c["loginId"]=json!(f.id);c["authUrl"]=json!(f.url);c["authPending"]=json!(!f.finished&&f.error.is_none());c["error"]=json!(f.error);}c}).collect())
}
pub async fn execute(
    rt: &Runtime,
    state: &AppState,
    name: &str,
    args: &Value,
) -> Result<String, String> {
    let id = args["serverId"].as_str().ok_or("Plugin MCP manquant")?;
    let client = ensure(rt, state, id).await?;
    if name == "mcp.list" {
        let sessions = rt.inner.sessions.lock().await;
        let s = sessions.get(id).ok_or("Connexion MCP perdue")?;
        return Ok(
            json!({"tools":s.tools,"resources":s.resources,"prompts":s.prompts}).to_string(),
        );
    }
    let value = match name {
        "mcp.call" => {
            let tool = args["toolName"].as_str().ok_or("Outil MCP manquant")?;
            let parameters = args["arguments"]
                .as_object()
                .ok_or("Arguments MCP invalides")?
                .clone();
            {
                let sessions = rt.inner.sessions.lock().await;
                if !sessions
                    .get(id)
                    .is_some_and(|s| s.tools.iter().any(|t| t["name"] == tool))
                {
                    return Err("Cet outil n’est pas déclaré par le serveur MCP".into());
                }
            }
            let result = tokio::time::timeout(
                Duration::from_secs(180),
                client.call_tool(
                    CallToolRequestParams::new(tool.to_owned()).with_arguments(parameters),
                ),
            )
            .await
            .map_err(|_| "L’outil MCP a dépassé trois minutes")?
            .map_err(|_| "Le serveur MCP n’a pas terminé l’outil")?;
            serde_json::to_value(result).map_err(|_| "Résultat MCP invalide")?
        }
        "mcp.read" => serde_json::to_value(
            tokio::time::timeout(
                Duration::from_secs(40),
                client.read_resource(ReadResourceRequestParams::new(
                    args["uri"].as_str().ok_or("URI manquante")?.to_owned(),
                )),
            )
            .await
            .map_err(|_| "Lecture MCP expirée")?
            .map_err(|_| "Ressource MCP inaccessible")?,
        )
        .map_err(|_| "Ressource invalide")?,
        "mcp.prompt" => serde_json::to_value(
            tokio::time::timeout(
                Duration::from_secs(40),
                client.get_prompt(
                    GetPromptRequestParams::new(
                        args["name"].as_str().ok_or("Prompt manquant")?.to_owned(),
                    )
                    .with_arguments(args["arguments"].as_object().cloned().unwrap_or_default()),
                ),
            )
            .await
            .map_err(|_| "Prompt MCP expiré")?
            .map_err(|_| "Prompt MCP inaccessible")?,
        )
        .map_err(|_| "Prompt invalide")?,
        _ => return Err("Action MCP inconnue".into()),
    };
    let result = value.to_string();
    if result.len() > 2 * 1024 * 1024 {
        return Err("Le résultat MCP dépasse 2 Mo".into());
    }
    Ok(result)
}
async fn callback(
    listener: &TcpListener,
    host: &str,
    expected: &str,
) -> Result<(TcpStream, String), String> {
    loop {
        let (mut socket, _) = listener
            .accept()
            .await
            .map_err(|_| "Retour OAuth indisponible")?;
        let request = tokio::time::timeout(Duration::from_secs(3), async {
            let mut data = vec![];
            let mut buf = [0u8; 1024];
            while data.len() < 8192 {
                let n = socket.read(&mut buf).await.ok()?;
                if n == 0 {
                    return None;
                }
                data.extend_from_slice(&buf[..n]);
                if data.windows(4).any(|v| v == b"\r\n\r\n") {
                    return String::from_utf8(data).ok();
                }
            }
            None
        })
        .await
        .ok()
        .flatten();
        if let Some(request) = request {
            let first = request
                .lines()
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect::<Vec<_>>();
            let hosts = request
                .lines()
                .skip(1)
                .filter_map(|l| l.split_once(':'))
                .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
                .map(|(_, v)| v.trim())
                .collect::<Vec<_>>();
            if first.len() == 3
                && first[0] == "GET"
                && first[1].starts_with("/oauth/callback?")
                && hosts == [host]
            {
                let raw = format!("http://{host}{}", first[1]);
                if let Ok(url) = url::Url::parse(&raw) {
                    let states = url
                        .query_pairs()
                        .filter(|(k, _)| k == "state")
                        .map(|(_, v)| v.to_string())
                        .collect::<Vec<_>>();
                    let codes = url.query_pairs().filter(|(k, _)| k == "code").count();
                    let errors = url.query_pairs().filter(|(k, _)| k == "error").count();
                    if states == [expected]
                        && ((codes == 1 && errors == 0) || (codes == 0 && errors == 1))
                    {
                        return Ok((socket, raw));
                    }
                }
            }
        }
        let _ = socket
            .write_all(
                b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await;
    }
}
fn is_google_mcp(resource: &url::Url) -> bool {
    let google_hosts = [
        "gmailmcp.googleapis.com",
        "drivemcp.googleapis.com",
        "docsmcp.googleapis.com",
        "sheetsmcp.googleapis.com",
        "slidesmcp.googleapis.com",
        "calendarmcp.googleapis.com",
        "chatmcp.googleapis.com",
        "people.googleapis.com",
        "classroommcp.googleapis.com",
    ];
    resource.scheme() == "https"
        && resource.port_or_known_default() == Some(443)
        && resource.path() == "/mcp/v1"
        && resource
            .host_str()
            .is_some_and(|host| google_hosts.contains(&host))
}
fn oauth_browser_url(raw: &str, resource: &str) -> Result<String, String> {
    let mut url = url::Url::parse(raw).map_err(|_| "Page OAuth invalide")?;
    let resource = url::Url::parse(resource).map_err(|_| "Endpoint MCP invalide")?;
    if url.host_str() == Some("accounts.google.com") && is_google_mcp(&resource) {
        let pairs = url
            .query_pairs()
            .filter(|(key, _)| key != "access_type" && key != "prompt")
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect::<Vec<_>>();
        url.set_query(None);
        url.query_pairs_mut()
            .extend_pairs(pairs)
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent");
    }
    Ok(url.into())
}
fn google_diagnostic_message(status: u16, body: &Value) -> String {
    // Never forward upstream text, URLs, headers or token material to the UI.
    let reasons = body["error"]["details"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|detail| detail["reason"].as_str())
        .collect::<Vec<_>>();
    let (code, advice) = if reasons.contains(&"SERVICE_DISABLED") {
        ("SERVICE_DISABLED", "Activez l’API du produit et son service MCP dans le projet Google Cloud du client OAuth.")
    } else if reasons.contains(&"ACCESS_TOKEN_SCOPE_INSUFFICIENT") {
        ("ACCESS_TOKEN_SCOPE_INSUFFICIENT", "Ajoutez les scopes indiqués dans la configuration Google, puis reconnectez le compte pour accorder ces accès.")
    } else if reasons.contains(&"USER_PROJECT_DENIED") || reasons.contains(&"CONSUMER_INVALID") {
        (
            "PROJECT_ACCESS_DENIED",
            "Vérifiez le projet Google Cloud associé au client OAuth et vos droits sur ce projet.",
        )
    } else {
        ("", match status {
            401 => "Google n’accepte pas le jeton pour ce serveur. Reconnectez le compte avec le client et les scopes prévus pour ce MCP.",
            403 => "Vérifiez les API activées, l’option client pour agent IA et les restrictions de votre organisation Google.",
            429 => "Le quota Google est atteint. Attendez avant de réessayer.",
            _ => "Vérifiez la disponibilité du service MCP Google et la configuration de votre projet.",
        })
    };
    format!("L’autorisation OAuth a abouti, mais Google refuse la connexion MCP (HTTP {status}{}). {advice}", if code.is_empty() { String::new() } else { format!(", {code}") })
}
async fn google_connection_diagnostic(
    c: &Config,
    s: &Secrets,
    client: &AuthClient<reqwest13::Client>,
) -> Option<String> {
    // One bounded, non-mutating initialize probe, only to the exact official resource
    // already authorized by the user. No tools or other destinations are invoked.
    if !url::Url::parse(&c.url)
        .ok()
        .is_some_and(|url| is_google_mcp(&url))
    {
        return None;
    }
    let probe = async {
        let token = client.get_access_token().await.ok()?;
        let mut response = client.http_client.post(&c.url)
            .headers(http_config(c, s).ok()?.custom_headers.into_iter().collect())
            .bearer_auth(token)
            .header("Accept", "application/json, text/event-stream")
            .json(&json!({"jsonrpc":"2.0","id":"converge-diagnostic","method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"Converge","version":env!("CARGO_PKG_VERSION")}}}))
            .send().await.ok()?;
        if response.status().is_success() {
            return None;
        }
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if bytes.len() + chunk.len() > 65536 {
                break;
            }
            bytes.extend_from_slice(&chunk);
        }
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Some(google_diagnostic_message(status, &body))
    };
    tokio::time::timeout(Duration::from_secs(8), probe)
        .await
        .ok()
        .flatten()
}
async fn authorize(rt: &Runtime, c: Config) -> Result<Value, String> {
    disconnect(rt, &c.id, false).await?;
    let port = c
        .redirect_port
        .unwrap_or(if c.client_id.is_empty() { 0 } else { 32145 });
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|_| "Retour OAuth local indisponible")?;
    let host = listener
        .local_addr()
        .map_err(|_| "Retour local indisponible")?
        .to_string();
    let vault = Vault::new(&c);
    let manager = auth_manager(&c, vault.clone()).await?;
    let mut oauth = OAuthState::Unauthorized(manager);
    let mut request = AuthorizationRequest::new(format!("http://{host}/oauth/callback"))
        .with_client_name("Converge");
    if !c.client_id.is_empty() {
        request = request.with_preregistered_client(&c.client_id);
    }
    if !c.scopes.is_empty() {
        request = request.with_scopes(c.scopes.clone());
    }
    let secret = secrets(&c.id)?;
    if !secret.client_secret.is_empty() {
        request = request.with_client_secret(secret.client_secret.clone());
    }
    tokio::time::timeout(Duration::from_secs(60),oauth.start_authorization(request)).await.map_err(|_|"Découverte OAuth expirée")?.map_err(|_|"Le serveur n’a pas pu préparer OAuth. Vérifiez l’accès au service et, si nécessaire, votre ID et secret client OAuth.")?;
    let url = oauth
        .get_authorization_url()
        .await
        .map_err(|_| "Page OAuth indisponible")?;
    let url = oauth_browser_url(&url, &c.url)?;
    let parsed = url::Url::parse(&url).map_err(|_| "Page OAuth invalide")?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || url.len() > 8192
    {
        return Err("Page OAuth refusée : HTTPS est requis".into());
    }
    let expected = parsed
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.to_string())
        .ok_or("État OAuth absent")?;
    let id = uuid::Uuid::new_v4().to_string();
    let cancel = CancellationToken::new();
    rt.inner
        .auth
        .lock()
        .map_err(|_| "Connexion occupée")?
        .insert(
            c.id.clone(),
            AuthFlow {
                id: id.clone(),
                url: url.clone(),
                cancel: cancel.clone(),
                vault: vault.clone(),
                error: None,
                finished: false,
            },
        );
    let runtime = rt.clone();
    let login = id.clone();
    tokio::spawn(async move {
        let operation = async {
            let (mut socket, callback_url) = callback(&listener, &host, &expected).await?;
            let result = async {
                oauth
                    .handle_callback_url(&callback_url)
                    .await
                    .map_err(|_| "L’autorisation MCP a été refusée ou a expiré")?;
                let manager = oauth
                    .into_authorization_manager()
                    .ok_or("Autorisation MCP absente")?;
                let authenticated = AuthClient::new(http()?, manager);
                let transport = StreamableHttpClientTransport::with_client(
                    authenticated.clone(),
                    http_config(&c, &secret)?,
                );
                let initialized = tokio::time::timeout(Duration::from_secs(40), info_for(&c).serve(transport))
                    .await
                    .map_err(|_| "Connexion MCP expirée")?;
                let client = match initialized {
                    Ok(client) => client,
                    Err(_) => return Err(google_connection_diagnostic(&c, &secret, &authenticated).await
                        .unwrap_or_else(|| "L’autorisation OAuth a abouti, mais l’initialisation du serveur MCP a échoué.".into())),
                };
                let connected = session(client, Some(vault.clone())).await?;
                if cancel.is_cancelled() {
                    return Err("Connexion annulée".into());
                }
                let mut sessions = runtime.inner.sessions.lock().await;
                {
                    let flows = runtime.inner.auth.lock().map_err(|_| "Connexion occupée")?;
                    if !flows
                        .get(&c.id)
                        .is_some_and(|f| f.id == login && !f.cancel.is_cancelled())
                    {
                        connected.client.cancellation_token().cancel();
                        return Err("Connexion annulée".into());
                    }
                }
                sessions.insert(c.id.clone(), connected);
                Ok::<(), String>(())
            }
            .await;
            let text = if result.is_ok() {
                "Connexion terminée. Revenez dans Converge.".to_string()
            } else {
                format!(
                    "Connexion non finalisée.\n{}\nRevenez dans Converge pour réessayer.",
                    result
                        .as_ref()
                        .err()
                        .map(String::as_str)
                        .unwrap_or("Connexion indisponible")
                )
            };
            let reply=format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len());
            let _ = socket.write_all(reply.as_bytes()).await;
            result
        };
        let result = tokio::select! {_=cancel.cancelled()=>Err("Connexion annulée".into()),r=tokio::time::timeout(Duration::from_secs(600),operation)=>r.unwrap_or_else(|_|Err("Connexion OAuth expirée".into()))};
        if let Ok(mut flows) = runtime.inner.auth.lock() {
            if let Some(flow) = flows.get_mut(&c.id).filter(|f| f.id == login) {
                match result {
                    Ok(()) => flow.finished = true,
                    Err(e) => flow.error = Some(e),
                }
            }
        }
    });
    Ok(json!({"loginId":id,"authUrl":url}))
}
#[tauri::command]
pub async fn mcp_action(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    action: String,
    id: Option<String>,
    config: Option<Config>,
    secret: Option<SecretsPatch>,
    login_id: Option<String>,
) -> Result<Value, String> {
    crate::permissions::trusted(&window)?;
    let runtime = &app.state::<Extensions>().mcp;
    if action == "save" {
        let c = config.ok_or("Configuration MCP absente")?;
        validate(&c)?;
        // Revoke the running generation before editing endpoint or credentials.
        let gate = {
            let mut gates = runtime.inner.connecting.lock().await;
            gates
                .entry(c.id.clone())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };
        let _gate = gate.lock().await;
        let old = super::record(&state, "mcp", &c.id).ok();
        let changed = old.as_ref().is_some_and(|old| {
            old["url"] != c.url
                || old["auth"] != c.auth
                || old["clientId"] != c.client_id
                || old["redirectPort"] != json!(c.redirect_port)
                || old["scopes"] != json!(c.scopes)
        });
        disconnect(runtime, &c.id, changed).await?;
        if let Some(patch) = secret {
            let secret = merge_secret(secrets(&c.id)?, patch);
            if serde_json::to_vec(&secret)
                .map_err(|_| "Identifiants invalides")?
                .len()
                > 128 * 1024
            {
                return Err("Identifiants trop volumineux".into());
            }
            if secret.env.keys().any(|k| {
                k.is_empty()
                    || k.len() > 120
                    || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            }) {
                return Err("Nom de variable d’environnement invalide".into());
            }
            entry("mcp", &c.id)?
                .set_password(
                    &serde_json::to_string(&secret).map_err(|_| "Identifiants invalides")?,
                )
                .map_err(|_| "Stockage sécurisé impossible")?;
        }
        let value = serde_json::to_value(&c).map_err(|_| "Configuration invalide")?;
        super::put(&state, "mcp", &c.id, &value)?;
        return Ok(value);
    }
    let id = id.ok_or("Plugin MCP manquant")?;
    uuid::Uuid::parse_str(&id).map_err(|_| "Identifiant MCP invalide")?;
    if action == "openAuth" {
        let url = {
            let flows = runtime.inner.auth.lock().map_err(|_| "Connexion occupée")?;
            let flow = flows
                .get(&id)
                .filter(|f| {
                    login_id.as_deref() == Some(f.id.as_str()) && !f.finished && f.error.is_none()
                })
                .ok_or("Cette autorisation n’est plus active")?;
            flow.url.clone()
        };
        crate::tools::open_browser(&url)?;
        return Ok(Value::Null);
    }
    if action == "remove" {
        super::record(&state, "mcp", &id)?;
        disconnect(runtime, &id, true).await?;
        clear_entry("mcp", &id)?;
        super::delete(&state, "mcp", &id)?;
        return Ok(Value::Null);
    }
    if action == "disconnect" || action == "logout" {
        let mut c = load_config(&state, &id)?;
        c.auto_connect = false;
        super::put(
            &state,
            "mcp",
            &id,
            &serde_json::to_value(&c).map_err(|_| "Configuration invalide")?,
        )?;
        disconnect(runtime, &id, action == "logout").await?;
        return Ok(Value::Null);
    }
    if action == "cancel" {
        let mut flows = runtime.inner.auth.lock().map_err(|_| "Connexion occupée")?;
        if flows
            .get(&id)
            .is_some_and(|f| login_id.as_deref() == Some(f.id.as_str()))
        {
            if let Some(f) = flows.remove(&id) {
                f.cancel.cancel();
                f.vault.invalidate();
            }
        }
        return Ok(Value::Null);
    }
    if action == "connect" {
        let mut c = load_config(&state, &id)?;
        c.auto_connect = true;
        super::put(
            &state,
            "mcp",
            &id,
            &serde_json::to_value(&c).map_err(|_| "Configuration invalide")?,
        )?;
        ensure(runtime, &state, &id).await?;
        return Ok(Value::Null);
    }
    if action == "authorize" {
        let c = load_config(&state, &id)?;
        if !c.enabled || c.auth != "oauth" || c.transport != "http" {
            return Err("Activez le plugin et choisissez OAuth".into());
        }
        return authorize(runtime, c).await;
    }
    Err("Action MCP inconnue".into())
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
