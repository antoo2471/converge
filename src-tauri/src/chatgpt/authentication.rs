use super::codex_transport::CodexTransport;
use serde_json::{json, Value};
pub async fn account(transport: &CodexTransport, refresh: bool) -> Result<Value, String> {
    let data = transport
        .rpc("account/read", json!({"refreshToken":refresh}))
        .await?;
    let a = &data["account"];
    if !a.is_null() && a["type"] != "chatgpt" {
        return Err("Cette session Codex utilise une clé API. Connectez-vous avec ChatGPT pour utiliser votre abonnement.".into());
    }
    Ok(if a.is_null() {
        Value::Null
    } else {
        json!({"type":"chatgpt","email":a["email"],"plan":a["planType"]})
    })
}
pub async fn login(transport: &CodexTransport) -> Result<Value, String> {
    let data = transport
        .rpc("account/login/start", json!({"type":"chatgpt"}))
        .await?;
    let url = data["authUrl"]
        .as_str()
        .ok_or("URL de connexion Codex manquante")?;
    let parsed = url::Url::parse(url).map_err(|_| "URL de connexion Codex invalide")?;
    if parsed.scheme() != "https"
        || !matches!(parsed.host_str(), Some("auth.openai.com" | "chatgpt.com"))
    {
        return Err("Origine de connexion Codex inattendue".into());
    }
    open_browser(url)?;
    Ok(json!({"loginId":data["loginId"],"authUrl":url}))
}
pub(super) fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let operation: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        let link: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                link.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as isize <= 32 {
            return Err("Impossible d’ouvrir le navigateur pour la connexion ChatGPT".into());
        }
    }
    #[cfg(not(windows))]
    {
        let mut command = std::process::Command::new(if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        });
        super::system_process::prepare(&mut command);
        command
            .arg(url)
            .spawn()
            .map_err(|_| "Impossible d’ouvrir le navigateur")?;
    }
    Ok(())
}
