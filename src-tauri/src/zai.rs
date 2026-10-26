//! Z.ai Coding Plan account authorization through ZCode's device/poll flow.
//! Only a dedicated Coding Plan key is retained, in the system credential vault.
use crate::{
    persistence::{Provider, Secret},
    AppState,
};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Manager, State};
use tokio_util::sync::CancellationToken;

const SITE: &str = "https://zcode.z.ai";
const API: &str = "https://api.z.ai";
#[derive(Clone)]
struct Flow {
    id: String,
    device: String,
    nonce: String,
    remote: String,
    url: String,
    deadline: Instant,
    next: Instant,
    interval: Duration,
    cancel: CancellationToken,
}
#[derive(Default)]
pub struct ZaiAuth {
    flows: Mutex<HashMap<String, Flow>>,
}
impl ZaiAuth {
    fn cancel(&self, provider_id: &str, login_id: Option<&str>) -> Result<(), String> {
        let mut flows = self.flows.lock().map_err(|_| "Connexion Z.ai occupée")?;
        if flows
            .get(provider_id)
            .is_some_and(|f| login_id.is_none_or(|id| f.id == id))
        {
            if let Some(flow) = flows.remove(provider_id) {
                flow.cancel.cancel();
            }
        }
        Ok(())
    }
}
pub fn oauth(p: &Provider) -> bool {
    p.plugin == "zai" && p.request["zai"]["authMode"] == "oauth"
}
fn vault_id(id: &str) -> String {
    format!("zai-oauth/{id}")
}
pub fn credentials(p: &Provider) -> Result<Secret, String> {
    crate::persistence::secret(&vault_id(&p.id))
}
fn vault(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("app.converge.desktop", &vault_id(id))
        .map_err(|_| "Coffre Z.ai indisponible".into())
}
pub fn remove(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    if let Some(flow) = app
        .state::<ZaiAuth>()
        .flows
        .lock()
        .map_err(|_| "Connexion Z.ai occupée")?
        .remove(id)
    {
        flow.cancel.cancel();
    }
    match vault(id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Impossible de retirer la connexion Z.ai du coffre".into()),
    }
}
pub fn validate(p: &Provider) -> Result<(), String> {
    if oauth(p)
        && !((p.protocol == "openai"
            && p.url.trim_end_matches('/') == "https://api.z.ai/api/coding/paas/v4")
            || (p.protocol == "anthropic"
                && p.url.trim_end_matches('/') == "https://api.z.ai/api/anthropic"))
    {
        return Err("La connexion de compte Z.ai utilise exclusivement les endpoints officiels du Coding Plan".into());
    }
    Ok(())
}
async fn call(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: &str,
    auth: &str,
    body: Option<Value>,
    device: Option<&str>,
) -> Result<Value, String> {
    let mut request = client
        .request(method, url)
        .timeout(Duration::from_secs(15))
        .header("User-Agent", "Converge/0.1.0")
        .header("Accept", "application/json");
    if !auth.is_empty() {
        request = request.header("Authorization", auth);
    }
    if let Some(device) = device {
        request = request.header("X-Device-Mid", device);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .map_err(|_| "Le service de connexion Z.ai ne répond pas. Réessayez.")?;
    if !response.status().is_success() {
        return Err(format!(
            "Z.ai a refusé l’étape de connexion (HTTP {}).",
            response.status().as_u16()
        ));
    }
    let mut bytes = vec![];
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "Réponse Z.ai interrompue")?;
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err("Réponse de connexion Z.ai trop volumineuse".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let envelope: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Réponse de connexion Z.ai invalide")?;
    if envelope["code"] != 0
        && envelope["code"] != 200
        && envelope["code"] != "0"
        && envelope["code"] != "200"
        && !envelope["code"].is_null()
    {
        return Err("Z.ai n’a pas validé cette étape de connexion. Vérifiez votre compte et votre Coding Plan.".into());
    }
    Ok(envelope.get("data").cloned().unwrap_or(envelope))
}
fn authorize_url(raw: &str, callback: &str) -> Result<String, String> {
    let mut url = url::Url::parse(raw).map_err(|_| "Page d’autorisation Z.ai invalide")?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || !url
            .host_str()
            .is_some_and(|h| h == "z.ai" || h.ends_with(".z.ai"))
    {
        return Err("Hôte d’autorisation Z.ai refusé".into());
    }
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| k != "redirect_uri")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    url.set_query(None);
    url.query_pairs_mut()
        .extend_pairs(pairs)
        .append_pair("redirect_uri", callback);
    Ok(url.into())
}
fn personal_project(info: &Value) -> Result<(String, String), String> {
    let mut found = None;
    for organization in info["organizations"].as_array().into_iter().flatten() {
        let Some(org) = organization["organizationId"].as_str() else {
            continue;
        };
        let projects: Vec<&Value> = organization["projects"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| {
                p["projectType"] != 2 && p["projectType"] != "2" && p["projectId"].is_string()
            })
            .collect();
        if let Some(project) = projects
            .iter()
            .find(|p| {
                p["projectName"]
                    .as_str()
                    .is_some_and(|s| s.contains("默认项目"))
            })
            .copied()
            .or_else(|| projects.first().copied())
        {
            found = Some((
                org.to_owned(),
                project["projectId"].as_str().unwrap().to_owned(),
            ));
            if organization["organizationName"]
                .as_str()
                .is_some_and(|s| s.contains("默认机构"))
            {
                break;
            }
        }
    }
    found.ok_or("Aucun projet personnel Z.ai n’est disponible. Pour un Coding Plan d’équipe, utilisez sa clé dédiée.".into())
}
fn segment(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}
async fn provision(client: &reqwest::Client, p: &Provider, token: &str) -> Result<String, String> {
    provision_at(client, p, token, API).await
}
async fn provision_at(
    client: &reqwest::Client,
    p: &Provider,
    token: &str,
    api: &str,
) -> Result<String, String> {
    let login = call(
        client,
        reqwest::Method::POST,
        &format!("{api}/api/auth/z/login"),
        "",
        Some(json!({"token":token})),
        None,
    )
    .await?;
    let access = login["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Z.ai n’a pas fourni d’autorisation de compte")?;
    let auth = format!("Bearer {access}");
    let info = call(
        client,
        reqwest::Method::GET,
        &format!("{api}/api/biz/customer/getCustomerInfo"),
        &auth,
        None,
        None,
    )
    .await?;
    let (org, project) = personal_project(&info)?;
    let path = format!(
        "{api}/api/biz/v1/organization/{}/projects/{}/api_keys",
        segment(&org),
        segment(&project)
    );
    let name = format!("converge-{}", p.id);
    let keys = call(client, reqwest::Method::GET, &path, &auth, None, None).await?;
    let id = if let Some(id) = keys
        .as_array()
        .into_iter()
        .flatten()
        .find(|k| k["name"] == name)
        .and_then(|k| k["apiKey"].as_str())
        .filter(|s| !s.is_empty())
    {
        id.to_owned()
    } else {
        call(
            client,
            reqwest::Method::POST,
            &path,
            &auth,
            Some(json!({"name":name})),
            None,
        )
        .await?["apiKey"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("Z.ai n’a pas créé la clé Converge dédiée")?
            .to_owned()
    };
    let copied = call(
        client,
        reqwest::Method::GET,
        &format!("{path}/copy/{}", segment(&id)),
        &auth,
        None,
        None,
    )
    .await?;
    let secret = copied["secretKey"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Z.ai n’a pas fourni la clé du Coding Plan")?;
    let key = format!("{id}.{secret}");
    let plans = call(
        client,
        reqwest::Method::GET,
        &format!("{api}/api/biz/subscription/list"),
        &key,
        None,
        None,
    )
    .await?;
    if !plans.as_array().into_iter().flatten().any(|plan| {
        plan["status"]
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("VALID"))
    }) {
        return Err("Le compte Z.ai n’a pas de Coding Plan personnel actif. La connexion n’a pas été enregistrée.".into());
    }
    Ok(key)
}
fn pending(flow: &Flow) -> Value {
    json!({"state":"pending","loginId":flow.id,"authUrl":flow.url})
}
#[tauri::command]
pub async fn zai_auth_action(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider: Provider,
    action: String,
    login_id: Option<String>,
) -> Result<Value, String> {
    if provider.plugin != "zai" || uuid::Uuid::parse_str(&provider.id).is_err() {
        return Err("Connexion Z.ai invalide".into());
    }
    let runtime = app.state::<ZaiAuth>();
    if action == "logout" {
        remove(&app, &provider.id)?;
        return Ok(json!({"state":"disconnected"}));
    }
    if action == "cancel" {
        runtime.cancel(&provider.id, login_id.as_deref())?;
        return Ok(json!({"state":"disconnected"}));
    }
    if action == "status" {
        return Ok(
            json!({"state":if credentials(&provider)?.key.is_empty(){"disconnected"}else{"connected"}}),
        );
    }
    if action == "start" {
        if let Some(old) = runtime
            .flows
            .lock()
            .map_err(|_| "Connexion occupée")?
            .remove(&provider.id)
        {
            old.cancel.cancel();
        }
        let device = uuid::Uuid::new_v4().to_string();
        let nonce = format!(
            "Bearer {}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let data = call(
            &state.client,
            reqwest::Method::POST,
            &format!("{SITE}/api/v1/oauth/cli/init"),
            &nonce,
            Some(json!({"provider":"zai"})),
            Some(&device),
        )
        .await?;
        let remote = data["flow_id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() < 500)
            .ok_or("Z.ai n’a pas créé de session d’autorisation")?
            .to_owned();
        // A private loopback page replaces ZCode's application deep link. No
        // authorization parameters are parsed or logged by this page.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| "Retour de connexion local indisponible")?;
        let address = listener
            .local_addr()
            .map_err(|_| "Retour de connexion local indisponible")?;
        let back = format!(
            "{SITE}/app/oauth/login?{}",
            url::form_urlencoded::Serializer::new(String::new())
                .append_pair("redirect", &format!("http://{address}/complete"))
                .finish()
        );
        let url = authorize_url(
            data["authorize_url"].as_str().ok_or("Page Z.ai absente")?,
            &back,
        )?;
        let cancel = CancellationToken::new();
        let callback_cancel = cancel.clone();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            tokio::select! {_=callback_cancel.cancelled()=>{},_=tokio::time::sleep(Duration::from_secs(600))=>{},accepted=listener.accept()=>{if let Ok((mut socket,_))=accepted{let mut bytes=[0u8;4096];let _=tokio::time::timeout(Duration::from_secs(3),socket.read(&mut bytes)).await;let body="<!doctype html><meta charset=utf-8><title>Converge</title><p>Vous pouvez revenir dans Converge.</p>";let response=format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Security-Policy: default-src 'none'\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());let _=socket.write_all(response.as_bytes()).await;}}}
        });
        let interval =
            Duration::from_secs(data["poll_interval_sec"].as_u64().unwrap_or(2).clamp(1, 10));
        let flow = Flow {
            id: uuid::Uuid::new_v4().to_string(),
            device,
            nonce,
            remote,
            url,
            deadline: Instant::now() + Duration::from_secs(300),
            next: Instant::now() + interval,
            interval,
            cancel,
        };
        let result = pending(&flow);
        if let Some(old) = runtime
            .flows
            .lock()
            .map_err(|_| "Connexion occupée")?
            .insert(provider.id, flow)
        {
            old.cancel.cancel();
        }
        return Ok(result);
    }
    if action != "poll" {
        return Err("Action Z.ai inconnue".into());
    }
    let flow = {
        let mut flows = runtime.flows.lock().map_err(|_| "Connexion occupée")?;
        let f = flows
            .get_mut(&provider.id)
            .ok_or("Autorisation expirée. Relancez la connexion Z.ai.")?;
        if Some(f.id.as_str()) != login_id.as_deref() {
            return Err("Session d’autorisation Z.ai différente".into());
        }
        if Instant::now() > f.deadline {
            f.cancel.cancel();
            return Err("La connexion Z.ai a expiré. Relancez-la.".into());
        }
        if Instant::now() < f.next {
            return Ok(pending(f));
        }
        f.next = Instant::now() + f.interval;
        f.clone()
    };
    let result = tokio::select! {_=flow.cancel.cancelled()=>return Err("Connexion Z.ai annulée".into()),result=async {
        let got=call(&state.client,reqwest::Method::GET,&format!("{SITE}/api/v1/oauth/cli/poll/{}",segment(&flow.remote)),&flow.nonce,None,Some(&flow.device)).await?;
        if got["status"]=="pending" || got["status"].is_null(){return Ok(None)}
        if got["status"]!="ready"{return Err("La connexion Z.ai a été refusée ou a expiré.".into())}
        let token=got["zai"]["access_token"].as_str().filter(|s|!s.is_empty()).ok_or("Z.ai n’a pas transmis d’autorisation")?;
        provision(&state.client,&provider,token).await.map(Some)
    }=>result}?;
    let Some(key) = result else {
        return Ok(pending(&flow));
    };
    let mut flows = runtime.flows.lock().map_err(|_| "Connexion occupée")?;
    if !flows.get(&provider.id).is_some_and(|f| f.id == flow.id) {
        return Err("Connexion Z.ai annulée".into());
    }
    let secret = Secret {
        key,
        headers: Default::default(),
    };
    vault(&provider.id)?
        .set_password(&serde_json::to_string(&secret).map_err(|_| "Identifiants Z.ai invalides")?)
        .map_err(|_| "Impossible de protéger la connexion Z.ai dans le coffre")?;
    flows.remove(&provider.id);
    Ok(json!({"state":"connected"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_gui_cancellation_does_not_cancel_a_new_login() {
        let auth = ZaiAuth::default();
        let cancel = CancellationToken::new();
        auth.flows.lock().unwrap().insert(
            "provider".into(),
            Flow {
                id: "new".into(),
                device: String::new(),
                nonce: String::new(),
                remote: String::new(),
                url: String::new(),
                deadline: Instant::now() + Duration::from_secs(10),
                next: Instant::now(),
                interval: Duration::from_secs(1),
                cancel: cancel.clone(),
            },
        );
        auth.cancel("provider", Some("old")).unwrap();
        assert!(!cancel.is_cancelled());
        auth.cancel("provider", Some("new")).unwrap();
        assert!(cancel.is_cancelled());
    }
    #[tokio::test]
    async fn account_exchange_creates_only_a_dedicated_plan_key() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for i in 0..6 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = vec![];
                let end;
                loop {
                    let mut part = [0u8; 4096];
                    let n = socket.read(&mut part).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&part[..n]);
                    if let Some(p) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        end = p + 4;
                        break;
                    }
                }
                let header = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                let len = header
                    .lines()
                    .find_map(|l| {
                        l.strip_prefix("content-length:")
                            .map(|s| s.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                while bytes.len() < end + len {
                    let mut part = [0u8; 4096];
                    let n = socket.read(&mut part).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&part[..n]);
                }
                assert!(header.contains("user-agent: converge/"));
                let data = match i {
                    0 => {
                        assert!(header.starts_with("post /api/auth/z/login "));
                        assert!(!header.contains("authorization:"));
                        assert_eq!(
                            serde_json::from_slice::<Value>(&bytes[end..]).unwrap()["token"],
                            "browser-authorization"
                        );
                        json!({"access_token":"business-authorization"})
                    }
                    1 => {
                        assert!(header.contains("authorization: bearer business-authorization"));
                        json!({"organizations":[{"organizationId":"org","projects":[{"projectType":1,"projectId":"project"}]}]})
                    }
                    2 => {
                        assert!(header.starts_with(
                            "get /api/biz/v1/organization/org/projects/project/api_keys "
                        ));
                        json!([])
                    }
                    3 => {
                        assert!(header.starts_with(
                            "post /api/biz/v1/organization/org/projects/project/api_keys "
                        ));
                        let body: Value = serde_json::from_slice(&bytes[end..]).unwrap();
                        assert!(body["name"].as_str().unwrap().starts_with("converge-"));
                        json!({"apiKey":"dedicated-id"})
                    }
                    4 => {
                        assert!(header.starts_with("get /api/biz/v1/organization/org/projects/project/api_keys/copy/dedicated-id "));
                        json!({"secretKey":"dedicated-secret"})
                    }
                    _ => {
                        assert!(header.contains("authorization: dedicated-id.dedicated-secret"));
                        json!([{"status":"VALID","productName":"GLM Coding Plan"}])
                    }
                };
                let body = json!({"code":0,"data":data}).to_string();
                let reply=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        let p:Provider=serde_json::from_value(json!({"id":uuid::Uuid::new_v4().to_string(),"name":"Z.ai","plugin":"zai","protocol":"openai","url":"https://api.z.ai/api/coding/paas/v4","headers":{},"models":[],"capabilities":{},"enabled":true})).unwrap();
        let key = provision_at(&reqwest::Client::new(), &p, "browser-authorization", &root)
            .await
            .unwrap();
        assert_eq!(key, "dedicated-id.dedicated-secret");
        server.await.unwrap();
    }
    #[tokio::test]
    #[ignore = "Creates an unapproved ephemeral Z.ai authorization flow; no browser or account login"]
    async fn live_zai_authorization_init() {
        assert_eq!(std::env::var("CONVERGE_ZAI_FLOW_TEST").as_deref(), Ok("1"));
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let nonce = format!(
            "Bearer {}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let device = uuid::Uuid::new_v4().to_string();
        let data = call(
            &client,
            reqwest::Method::POST,
            &format!("{SITE}/api/v1/oauth/cli/init"),
            &nonce,
            Some(json!({"provider":"zai"})),
            Some(&device),
        )
        .await
        .unwrap();
        assert!(data["flow_id"].is_string());
        let url = authorize_url(
            data["authorize_url"].as_str().unwrap(),
            "https://zcode.z.ai/app/oauth/login",
        )
        .unwrap();
        println!(
            "Z.ai OAuth init accepted; authorization host: {}",
            url::Url::parse(&url).unwrap().host_str().unwrap()
        );
    }
    #[test]
    fn rejects_external_authorization_hosts() {
        assert!(authorize_url("https://evil.test/login", "http://localhost").is_err());
        assert!(authorize_url("http://chat.z.ai/login", "http://localhost").is_err());
        assert!(authorize_url(
            "https://chat.z.ai/login?redirect_uri=old",
            "https://zcode.z.ai/app/oauth/login"
        )
        .unwrap()
        .contains("redirect_uri=https"));
    }
    #[test]
    fn chooses_personal_project_without_granting_team_access() {
        let info = json!({"organizations":[{"organizationId":"o","projects":[{"projectId":"team","projectType":2},{"projectId":"personal","projectType":1}]}]});
        assert_eq!(
            personal_project(&info).unwrap(),
            ("o".into(), "personal".into())
        );
        assert!(personal_project(&json!({"organizations":[]})).is_err());
    }
}
