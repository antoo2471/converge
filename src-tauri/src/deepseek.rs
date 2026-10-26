//! DeepSeek's public Harness PKCE protocol, with a Converge-owned grant.
use crate::{
    persistence::{Provider, Secret},
    AppState,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use futures_util::StreamExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{Manager, State};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::sync::CancellationToken;

const PLATFORM: &str = "https://platform.deepseek.com";
const API: &str = "https://api.deepseek.com";
fn client_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "desktop-win"
    } else if cfg!(target_os = "macos") {
        "desktop-mac"
    } else {
        "web"
    }
}
struct Flow {
    id: String,
    url: Option<String>,
    cancel: CancellationToken,
    finished: bool,
    error: Option<String>,
}
#[derive(Default)]
pub struct DeepSeekAuth {
    flows: Arc<Mutex<HashMap<String, Flow>>>,
}
impl Drop for DeepSeekAuth {
    fn drop(&mut self) {
        if let Ok(flows) = self.flows.lock() {
            for flow in flows.values() {
                flow.cancel.cancel();
            }
        }
    }
}
pub fn oauth(p: &Provider) -> bool {
    p.plugin == "deepseek" && p.request["deepseek"]["authMode"] == "oauth"
}
pub fn validate(p: &Provider) -> Result<(), String> {
    if p.plugin != "deepseek" {
        return Ok(());
    }
    if uuid::Uuid::parse_str(&p.id).is_err()
        || !["oauth", "api-key"].contains(&p.request["deepseek"]["authMode"].as_str().unwrap_or(""))
    {
        return Err("Configuration DeepSeek invalide".into());
    }
    if oauth(p)
        && (p.url.trim_end_matches('/') != API
            || p.protocol != "anthropic"
            || p.request["chatPath"] != "anthropic/v1/messages"
            || !p.headers.is_empty()
            || p.request.get("modelsPath").is_some()
            || p.request.get("body").is_some())
    {
        return Err("La connexion de compte DeepSeek utilise uniquement le service officiel et son format Messages.".into());
    }
    Ok(())
}
fn vault(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("app.converge.desktop", &format!("deepseek-oauth/{id}"))
        .map_err(|_| "Coffre DeepSeek indisponible".into())
}
pub fn credentials(p: &Provider) -> Result<Secret, String> {
    validate(p)?;
    let secret = crate::persistence::secret(&format!("deepseek-oauth/{}", p.id))?;
    if secret.key.is_empty() {
        return Err("Connectez votre compte DeepSeek dans les fournisseurs.".into());
    }
    Ok(secret)
}
fn clear(id: &str) -> Result<(), String> {
    match vault(id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Impossible de retirer le compte DeepSeek du coffre".into()),
    }
}
pub fn remove(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    cancel(&app.state::<DeepSeekAuth>().flows, id, None)?;
    clear(id)
}
fn cancel(
    flows: &Mutex<HashMap<String, Flow>>,
    pid: &str,
    login: Option<&str>,
) -> Result<(), String> {
    let mut flows = flows.lock().map_err(|_| "Connexion DeepSeek occupée")?;
    if flows
        .get(pid)
        .is_some_and(|f| login.is_none_or(|id| f.id == id))
    {
        if let Some(flow) = flows.remove(pid) {
            flow.cancel.cancel();
        }
    }
    Ok(())
}
fn browser_url(raw: &str, path: &str) -> Result<String, String> {
    let u = url::Url::parse(raw).map_err(|_| "Page DeepSeek invalide")?;
    if u.origin().ascii_serialization() != PLATFORM
        || u.path() != path
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
    {
        return Err("Page DeepSeek refusée".into());
    }
    Ok(u.into())
}
async fn platform(
    client: &reqwest::Client,
    origin: &str,
    method: &str,
    body: Value,
) -> Result<Value, String> {
    let response = client
        .post(format!("{origin}/auth-api/v0/dsh/{method}"))
        .header("User-Agent", "Converge/0.1.0")
        // auth_init tolerates their absence, but auth_exchange requires the
        // platform identity. Describe Converge itself, not a Harness version.
        .header("x-client-platform", client_platform())
        .header("x-client-version", env!("CARGO_PKG_VERSION"))
        .header("x-client-locale", "en_US")
        .header("x-client-bundle-id", "")
        .timeout(Duration::from_secs(20))
        .json(&body)
        .send()
        .await
        .map_err(|_| "Le service de connexion DeepSeek ne répond pas")?;
    if !response.status().is_success() {
        return Err(format!(
            "DeepSeek a refusé la connexion (HTTP {}).",
            response.status().as_u16()
        ));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "Réponse DeepSeek interrompue")?;
        if bytes.len() + chunk.len() > 65536 {
            return Err("Réponse de connexion DeepSeek trop volumineuse".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let data: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Réponse de connexion DeepSeek invalide")?;
    if data["code"] != 0 || data["data"]["biz_code"] != 0 {
        return Err("DeepSeek n’a pas validé cette connexion. Réessayez depuis Converge.".into());
    }
    Ok(data["data"]["biz_data"].clone())
}
fn callback_code(request: &str, state: &str, host: &str) -> Option<String> {
    let mut lines = request.split("\r\n");
    let first = lines.next()?.split_whitespace().collect::<Vec<_>>();
    if first.len() != 3 || first[0] != "GET" || !first[2].starts_with("HTTP/1.") {
        return None;
    }
    let hosts = lines
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.trim())
        .collect::<Vec<_>>();
    if hosts != [host] {
        return None;
    }
    let u = url::Url::parse(&format!("http://{host}{}", first[1])).ok()?;
    if !first[1].starts_with('/') || u.path() != "/oauth/callback" {
        return None;
    }
    let pairs = u.query_pairs().collect::<Vec<_>>();
    let states = pairs
        .iter()
        .filter(|(k, _)| k == "state")
        .collect::<Vec<_>>();
    let codes = pairs
        .iter()
        .filter(|(k, _)| k == "code")
        .collect::<Vec<_>>();
    if states.len() != 1 || codes.len() != 1 {
        return None;
    }
    let received = states[0].1.as_bytes();
    if received.len() != state.len()
        || received
            .iter()
            .zip(state.as_bytes())
            .fold(0u8, |a, (x, y)| a | (x ^ y))
            != 0
    {
        return None;
    }
    let code = &codes[0].1;
    if code.is_empty() || code.len() > 4096 || code.contains(['\r', '\n', '\0']) {
        return None;
    }
    Some(code.to_string())
}
async fn receive(
    listener: &TcpListener,
    state: &str,
    host: &str,
) -> Result<(TcpStream, String), String> {
    loop {
        let (mut socket, _) = listener
            .accept()
            .await
            .map_err(|_| "Retour DeepSeek indisponible")?;
        let request = tokio::time::timeout(Duration::from_secs(3), async {
            let mut bytes = Vec::new();
            let mut buf = [0u8; 1024];
            while bytes.len() < 8192 {
                let n = socket.read(&mut buf).await.ok()?;
                if n == 0 {
                    return None;
                }
                bytes.extend_from_slice(&buf[..n]);
                if bytes.windows(4).any(|s| s == b"\r\n\r\n") {
                    return String::from_utf8(bytes).ok();
                }
            }
            None
        })
        .await
        .ok()
        .flatten();
        if let Some(code) = request
            .as_deref()
            .and_then(|r| callback_code(r, state, host))
        {
            return Ok((socket, code));
        }
        let _ = socket.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n").await;
    }
}
fn save_token(id: &str, data: &Value) -> Result<(), String> {
    let token = data["token"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 16000 && s.bytes().all(|b| (33..=126).contains(&b)))
        .ok_or("Autorisation DeepSeek absente")?;
    browser_url(
        data["authorized_url"]
            .as_str()
            .ok_or("Retour DeepSeek absent")?,
        "/dsh/authorized",
    )?;
    vault(id)?
        .set_password(
            &serde_json::to_string(&Secret {
                key: token.into(),
                headers: HashMap::new(),
            })
            .map_err(|_| "Autorisation invalide")?,
        )
        .map_err(|_| "Échec du stockage sécurisé de la connexion DeepSeek".into())
}
#[tauri::command]
pub async fn deepseek_auth_action(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider: Provider,
    action: String,
    login_id: Option<String>,
) -> Result<Value, String> {
    validate(&provider)?;
    if !oauth(&provider) {
        return Err("Choisissez la connexion de compte DeepSeek".into());
    }
    let runtime = app.state::<DeepSeekAuth>();
    if action == "cancel" {
        cancel(&runtime.flows, &provider.id, login_id.as_deref())?;
        return Ok(json!({"state":"disconnected"}));
    }
    if action == "logout" {
        remove(&app, &provider.id)?;
        return Ok(json!({"state":"disconnected"}));
    }
    if action == "status" || action == "poll" {
        let flows = runtime
            .flows
            .lock()
            .map_err(|_| "Connexion DeepSeek occupée")?;
        if let Some(flow) = flows
            .get(&provider.id)
            .filter(|f| login_id.as_deref().is_none_or(|id| f.id == id))
        {
            if let Some(error) = &flow.error {
                return Err(error.clone());
            }
            if flow.finished {
                return Ok(json!({"state":"connected"}));
            }
            return Ok(json!({"state":"pending","loginId":flow.id,"authUrl":flow.url}));
        }
        return Ok(
            json!({"state":if credentials(&provider).is_ok(){"connected"}else{"disconnected"}}),
        );
    }
    if action != "start" {
        return Err("Action DeepSeek inconnue".into());
    }
    cancel(&runtime.flows, &provider.id, None)?;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| "Retour de connexion local indisponible")?;
    let host = listener
        .local_addr()
        .map_err(|_| "Retour local indisponible")?
        .to_string();
    let redirect = format!("http://{host}/oauth/callback");
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let csrf = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let id = uuid::Uuid::new_v4().to_string();
    let cancel_token = CancellationToken::new();
    runtime
        .flows
        .lock()
        .map_err(|_| "Connexion DeepSeek occupée")?
        .insert(
            provider.id.clone(),
            Flow {
                id: id.clone(),
                url: None,
                cancel: cancel_token.clone(),
                finished: false,
                error: None,
            },
        );
    let initialized = platform(&state.client, PLATFORM, "auth_init", json!({"code_challenge":challenge,"code_challenge_method":"S256","state":csrf,"redirect_uri":redirect,"locale":"en_US","login_source":"desktop"})).await;
    let init = match initialized {
        Ok(v) => v,
        Err(e) => {
            cancel(&runtime.flows, &provider.id, Some(&id))?;
            return Err(e);
        }
    };
    let parsed = (|| -> Result<(String, String, u64), String> {
        Ok((
            browser_url(
                init["authorize_url"]
                    .as_str()
                    .ok_or("Page DeepSeek absente")?,
                "/dsh/authorize",
            )?,
            init["authorize_id"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() < 4096)
                .ok_or("Session DeepSeek absente")?
                .into(),
            init["expires_in"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or("Expiration DeepSeek absente")?
                .min(600),
        ))
    })();
    let (url, authorize_id, expires) = match parsed {
        Ok(v) => v,
        Err(e) => {
            cancel(&runtime.flows, &provider.id, Some(&id))?;
            return Err(e);
        }
    };
    {
        let mut flows = runtime
            .flows
            .lock()
            .map_err(|_| "Connexion DeepSeek occupée")?;
        let flow = flows
            .get_mut(&provider.id)
            .filter(|f| f.id == id)
            .ok_or("Connexion DeepSeek annulée")?;
        flow.url = Some(url.clone());
    }
    let flows = runtime.flows.clone();
    let client = state.client.clone();
    let pid = provider.id;
    let login = id.clone();
    tokio::spawn(async move {
        let operation = async {
            let (mut socket, code) = receive(&listener, &csrf, &host).await?;
            let result = platform(&client, PLATFORM, "auth_exchange", json!({"code":code,"code_verifier":verifier,"redirect_uri":redirect,"device_id":pid,"device_model":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),"os_version":std::env::consts::OS})).await;
            let result = result.and_then(|data| {
                let mut flows = flows.lock().map_err(|_| "Connexion DeepSeek occupée")?;
                let flow = flows
                    .get_mut(&pid)
                    .filter(|f| f.id == login && !f.cancel.is_cancelled())
                    .ok_or("Connexion DeepSeek annulée")?;
                save_token(&pid, &data)?;
                flow.finished = true;
                Ok(())
            });
            let body = if result.is_ok() {
                "Compte DeepSeek connecté. Vous pouvez revenir dans Converge."
            } else {
                "Connexion DeepSeek non finalisée. Revenez dans Converge pour réessayer."
            };
            let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = socket.write_all(reply.as_bytes()).await;
            result
        };
        let result = tokio::select! { _ = cancel_token.cancelled() => Err("Connexion DeepSeek annulée".to_string()), r = tokio::time::timeout(Duration::from_secs(expires), operation) => r.unwrap_or_else(|_| Err("Connexion DeepSeek expirée. Relancez-la depuis Converge.".into())) };
        if let Err(error) = result {
            if let Ok(mut map) = flows.lock() {
                if let Some(f) = map.get_mut(&pid).filter(|f| f.id == login && !f.finished) {
                    f.error = Some(error);
                }
            }
            let _ = platform(
                &client,
                PLATFORM,
                "auth_cancel",
                json!({"authorize_id":authorize_id,"code_verifier":verifier}),
            )
            .await;
        }
    });
    Ok(json!({"state":"pending","loginId":id,"authUrl":url}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "Initializes and cancels a real DeepSeek PKCE flow, without opening the browser or signing in"]
    async fn live_deepseek_authorization_init_and_cancel() {
        assert_eq!(
            std::env::var("CONVERGE_DEEPSEEK_FLOW_TEST").as_deref(),
            Ok("1")
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let verifier = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let initialized = platform(&client, PLATFORM, "auth_init", json!({"code_challenge":URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),"code_challenge_method":"S256","state":uuid::Uuid::new_v4().to_string(),"redirect_uri":format!("http://{}/oauth/callback",listener.local_addr().unwrap()),"locale":"en_US","login_source":"desktop"})).await.unwrap();
        let valid_url = browser_url(
            initialized["authorize_url"].as_str().unwrap(),
            "/dsh/authorize",
        );
        let cancellation = platform(
            &client,
            PLATFORM,
            "auth_cancel",
            json!({"authorize_id":initialized["authorize_id"],"code_verifier":verifier}),
        )
        .await;
        valid_url.unwrap();
        cancellation.unwrap();
        // A deliberately invalid code must reach business validation, rather
        // than fail the exchange request schema with HTTP 422. No user grant.
        let rejected = platform(
            &client,
            PLATFORM,
            "auth_exchange",
            json!({
                "code":"converge-schema-test-invalid-code", "code_verifier":verifier,
                "redirect_uri":format!("http://{}/oauth/callback",listener.local_addr().unwrap()),
                "device_id":uuid::Uuid::new_v4().to_string(),
                "device_model":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),
                "os_version":std::env::consts::OS
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(
            rejected,
            "DeepSeek n’a pas validé cette connexion. Réessayez depuis Converge."
        );
        println!("DeepSeek PKCE initialization and cancellation accepted; no account authorized.");
    }
    #[tokio::test]
    async fn real_loopback_rejects_invalid_callback_then_accepts_valid_code() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let host = listener.local_addr().unwrap().to_string();
        let target = host.clone();
        let server = tokio::spawn(async move {
            let (mut socket, code) = receive(&listener, "private-state", &target).await.unwrap();
            assert_eq!(code, "private-code");
            socket
                .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
        });
        let client = reqwest::Client::new();
        assert_eq!(
            client
                .get(format!(
                    "http://{host}/oauth/callback?state=wrong&code=wrong"
                ))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
        assert_eq!(
            client
                .get(format!(
                    "http://{host}/oauth/callback?state=private-state&code=private-code"
                ))
                .send()
                .await
                .unwrap()
                .status(),
            204
        );
        server.await.unwrap();
    }
    #[tokio::test]
    async fn platform_exchange_validates_envelopes_without_exposing_remote_secrets() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for (method, response) in [
                (
                    "auth_init",
                    json!({"code":0,"data":{"biz_code":0,"biz_data":{"authorize_id":"id","expires_in":600}}}),
                ),
                (
                    "auth_exchange",
                    json!({"code":0,"data":{"biz_code":403,"biz_data":{"token":"never-expose-this-token"}}}),
                ),
            ] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut buf = [0u8; 1024];
                loop {
                    let n = socket.read(&mut buf).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buf[..n]);
                    if bytes.windows(4).any(|s| s == b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8_lossy(&bytes);
                assert!(request.starts_with(&format!("POST /auth-api/v0/dsh/{method} ")));
                let headers = request.to_ascii_lowercase();
                assert!(
                    headers.contains(&format!("\r\nx-client-platform: {}\r\n", client_platform()))
                );
                assert!(headers.contains(&format!(
                    "\r\nx-client-version: {}\r\n",
                    env!("CARGO_PKG_VERSION")
                )));
                assert!(headers.contains("\r\nx-client-locale: en_us\r\n"));
                assert!(headers.contains("\r\nuser-agent: converge/"));
                let body = response.to_string();
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        let client = reqwest::Client::new();
        assert_eq!(
            platform(&client, &root, "auth_init", json!({}))
                .await
                .unwrap()["authorize_id"],
            "id"
        );
        let error = platform(&client, &root, "auth_exchange", json!({}))
            .await
            .unwrap_err();
        assert!(!error.contains("never-expose"));
        server.await.unwrap();
    }
    #[test]
    fn oauth_rejects_destination_and_request_overrides() {
        let p: Provider = serde_json::from_value(json!({"id":uuid::Uuid::new_v4().to_string(),"name":"DeepSeek","plugin":"deepseek","protocol":"anthropic","url":API,"headers":{},"models":[],"enabled":true,"capabilities":{},"request":{"deepseek":{"authMode":"oauth"},"chatPath":"anthropic/v1/messages"}})).unwrap();
        validate(&p).unwrap();
        let mut v = p.clone();
        v.url = "https://evil.test".into();
        assert!(validate(&v).is_err());
        let mut v = p.clone();
        v.request["chatPath"] = json!("other");
        assert!(validate(&v).is_err());
        let mut v = p.clone();
        v.request["body"] = json!({});
        assert!(validate(&v).is_err());
        let mut v = p;
        v.headers
            .insert("x-dsh-auth-token".into(), "override".into());
        assert!(validate(&v).is_err());
    }
    #[test]
    fn callback_checks_state_host_path_and_duplicates() {
        let request = "GET /oauth/callback?code=private-code&state=expected HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n";
        assert_eq!(
            callback_code(request, "expected", "127.0.0.1:1234"),
            Some("private-code".into())
        );
        for bad in [
            request.replace("expected", "wrong"),
            request.replace("/oauth/callback", "/other"),
            request.replace("state=expected", "state=expected&state=expected"),
            request.replace("Host: 127.0.0.1:1234", "Host: evil.test"),
            request.replace("GET ", "POST "),
        ] {
            assert!(callback_code(&bad, "expected", "127.0.0.1:1234").is_none());
        }
        assert!(browser_url("https://evil.test/dsh/authorize", "/dsh/authorize").is_err());
    }
    #[test]
    fn stale_cancel_preserves_new_login() {
        let runtime = DeepSeekAuth::default();
        let token = CancellationToken::new();
        runtime.flows.lock().unwrap().insert(
            "provider".into(),
            Flow {
                id: "new".into(),
                url: None,
                cancel: token.clone(),
                finished: false,
                error: None,
            },
        );
        cancel(&runtime.flows, "provider", Some("old")).unwrap();
        assert!(!token.is_cancelled());
        cancel(&runtime.flows, "provider", Some("new")).unwrap();
        assert!(token.is_cancelled());
    }
}
