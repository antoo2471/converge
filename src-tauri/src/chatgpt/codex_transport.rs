//! Private stdio JSON-RPC connection. Never log RPC payloads: login responses can contain credentials.
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::Path,
    process::Stdio,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{broadcast, oneshot},
};

pub struct CodexTransport {
    input: tokio::sync::Mutex<ChildStdin>,
    child: tokio::sync::Mutex<Child>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>,
    pub events: broadcast::Sender<Value>,
    sequence: AtomicU64,
    pub cwd: std::path::PathBuf,
}
impl CodexTransport {
    pub async fn start(binary: &Path, home: &Path, cwd: &Path) -> Result<Arc<Self>, String> {
        std::fs::create_dir_all(home).map_err(|_| "Impossible de préparer le profil Codex")?;
        std::fs::create_dir_all(cwd)
            .map_err(|_| "Impossible de préparer le dossier privé Codex")?;
        let mut cmd = Command::new(binary);
        cmd.arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .env("CODEX_HOME", home)
            .current_dir(cwd)
            .env_remove("OPENAI_API_KEY")
            .env_remove("OPENAI_BASE_URL")
            .env_remove("CODEX_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        // Converge owns every tool and permission. Do not load personal plugins, hooks or local instructions.
        for setting in [
            "model_provider=\"openai\"",
            "approval_policy=\"never\"",
            "sandbox_mode=\"read-only\"",
            "web_search=\"disabled\"",
            "features.shell_tool=false",
            "features.apply_patch_freeform=false",
            "features.apps=false",
            "features.hooks=false",
            "features.multi_agent=false",
            "features.remote_plugin=false",
            "features.memories=false",
            "features.goals=false",
            "features.shell_snapshot=false",
            "features.skills=false",
            "project_doc_max_bytes=0",
            "check_for_update_on_startup=false",
        ] {
            cmd.arg("-c").arg(setting);
        }
        cmd.arg("-c").arg("cli_auth_credentials_store=\"keyring\"");
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        let mut child = cmd.spawn().map_err(|_| "Codex introuvable ou impossible à démarrer. Installez Codex CLI ou indiquez son exécutable dans les paramètres avancés.")?;
        let input = child.stdin.take().ok_or("Entrée Codex indisponible")?;
        let output = child.stdout.take().ok_or("Sortie Codex indisponible")?;
        let (events, _) = broadcast::channel(2048);
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>> =
            Default::default();
        let transport = Arc::new(Self {
            input: tokio::sync::Mutex::new(input),
            child: tokio::sync::Mutex::new(child),
            pending: pending.clone(),
            events: events.clone(),
            sequence: AtomicU64::new(1),
            cwd: cwd.to_owned(),
        });
        tokio::spawn(async move {
            let mut reader = BufReader::new(output);
            loop {
                let mut bytes = Vec::new();
                // Bound each frame before allocation can grow without limit.
                let mut limited = tokio::io::AsyncReadExt::take(&mut reader, 32 * 1024 * 1024 + 1);
                let read = limited.read_until(b'\n', &mut bytes).await;
                if !matches!(read, Ok(n) if n > 0 && n <= 32 * 1024 * 1024) {
                    break;
                }
                let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
                    break;
                };
                if value.get("method").is_none() {
                    if let Some(id) = value["id"].as_u64() {
                        let tx = pending.lock().ok().and_then(|mut p| p.remove(&id));
                        if let Some(tx) = tx {
                            let result = if value.get("error").is_some() {
                                Err(super::response::safe_error(&value["error"]))
                            } else {
                                Ok(value["result"].clone())
                            };
                            let _ = tx.send(result);
                        }
                    }
                } else {
                    let _ = events.send(value);
                }
            }
            if let Ok(mut pending) = pending.lock() {
                for (_, tx) in pending.drain() {
                    let _ = tx.send(Err(
                        "Connexion Codex interrompue. Reconnectez le fournisseur.".into(),
                    ));
                }
            }
            let _ = events.send(json!({"method":"converge/disconnected"}));
        });
        transport.rpc("initialize", json!({"clientInfo":{"name":"converge","title":"Converge","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true}})).await?;
        transport.send(json!({"method":"initialized"})).await?;
        Ok(transport)
    }
    pub async fn alive(&self) -> bool {
        matches!(self.child.lock().await.try_wait(), Ok(None))
    }
    pub async fn send(&self, value: Value) -> Result<(), String> {
        let mut data = serde_json::to_vec(&value).map_err(|_| "Requête Codex invalide")?;
        data.push(b'\n');
        let mut input = self.input.lock().await;
        input
            .write_all(&data)
            .await
            .map_err(|_| "Connexion Codex interrompue")?;
        input
            .flush()
            .await
            .map_err(|_| "Connexion Codex interrompue".into())
    }
    pub async fn rpc(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.sequence.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .map_err(|_| "Transport occupé")?
            .insert(id, tx);
        let result = async {
            self.send(json!({"id":id,"method":method,"params":params}))
                .await?;
            tokio::time::timeout(std::time::Duration::from_secs(30), rx)
                .await
                .map_err(|_| "Codex ne répond pas (30 s)")?
                .map_err(|_| "Connexion Codex fermée")?
        }
        .await;
        self.pending
            .lock()
            .map_err(|_| "Transport occupé")?
            .remove(&id);
        result
    }
    pub async fn stop(&self) {
        let _ = self.child.lock().await.kill().await;
    }
}
