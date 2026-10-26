use crate::AppState;
use base64::Engine;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[tauri::command(async)]
pub fn paste_clipboard_image(app: tauri::AppHandle) -> Result<Value, String> {
    use image::ImageEncoder;
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let image = app
        .clipboard()
        .read_image()
        .map_err(|_| "Le presse-papiers ne contient pas d’image lisible")?;
    if image.width() as u64 * image.height() as u64 > 25_000_000 {
        return Err("L’image du presse-papiers est trop grande".into());
    }
    let mut bytes = vec![];
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            image.rgba(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    if bytes.len() > 10 * 1024 * 1024 {
        return Err("L’image dépasse 10 Mo".into());
    }
    Ok(
        json!({"id":uuid::Uuid::new_v4().to_string(),"name":"Image du presse-papiers.png","mime":"image/png","data":base64::engine::general_purpose::STANDARD.encode(bytes)}),
    )
}

#[tauri::command]
pub async fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
    let parsed = url::Url::parse(&url).map_err(|_| "Lien invalide")?;
    if !["https", "http"].contains(&parsed.scheme())
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || url.len() > 8192
    {
        return Err("Seuls les liens web sans identifiants sont autorisés".into());
    }
    if !app
        .dialog()
        .message(format!("Ouvrir ce lien dans votre navigateur ?\n\n{url}"))
        .title("Converge · Lien externe")
        .buttons(MessageDialogButtons::OkCancel)
        .blocking_show()
    {
        return Ok(());
    }
    open_browser(&url)
}
/// Only internal callers may skip the general link confirmation. OAuth callers
/// must bind the URL to an explicit pending login before reaching this helper.
pub(crate) fn open_browser(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|_| "Lien invalide")?;
    if !["https", "http"].contains(&parsed.scheme())
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || url.len() > 8192
    {
        return Err("Lien web refusé".into());
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            UI::Shell::ShellExecuteW, UI::WindowsAndMessaging::SW_SHOWNORMAL,
        };
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
            return Err("Impossible d’ouvrir le navigateur par défaut".into());
        }
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn schemas() -> Vec<(String, String, Value)> {
    let definitions = [
        ("skills.read", "Read an enabled skill's SKILL.md or a relative text resource", vec![("skillId", "string"), ("path", "string")]),
        ("mcp.list", "Discover tools, resources and prompts from an enabled MCP connection", vec![("serverId", "string")]),
        ("mcp.call", "Call a discovered MCP tool after approval", vec![("serverId", "string"), ("toolName", "string"), ("arguments", "object")]),
        ("mcp.read", "Read a resource from an enabled MCP server", vec![("serverId", "string"), ("uri", "string")]),
        ("mcp.prompt", "Get a prompt from an enabled MCP server", vec![("serverId", "string"), ("name", "string"), ("arguments", "object")]),
        (
            "filesystem.read",
            "Read a UTF-8 file within the approved root",
            vec![("path", "string")],
        ),
        (
            "filesystem.list",
            "List entries in the approved root",
            vec![("path", "string")],
        ),
        (
            "filesystem.write",
            "Write a file after explicit approval",
            vec![("path", "string"), ("content", "string")],
        ),
        (
            "coding.search",
            "Search project source files",
            vec![("path", "string"), ("query", "string")],
        ),
        (
            "coding.patch",
            "Replace exactly one occurrence in a project file",
            vec![
                ("path", "string"),
                ("search", "string"),
                ("replace", "string"),
            ],
        ),
        (
            "terminal.execute",
            "Run a command in the OS sandbox (Windows AppContainer, Linux bubblewrap, macOS Seatbelt), network denied. Read-only by default on the approved folder; use readOnly:false for a writable disposable copy.",
            vec![("command", "string")],
        ),
        (
            "web.fetch",
            "Extract text from an HTTPS page",
            vec![("url", "string")],
        ),
        (
            "web.search",
            "Search the public Web. Uses built-in Bing or an optional SearXNG instance; returns URLs and snippets.",
            vec![("query", "string")],
        ),
        (
            "web.images",
            "Search images with built-in Bing or optional SearXNG",
            vec![("query", "string")],
        ),
        (
            "computer.capture",
            "Capture the real desktop after approval",
            vec![],
        ),
        ("computer.windows", "List visible windows with identifiers and physical bounds. Choose a target with computer.focus to receive its screenshot.", vec![]),
        ("computer.actions", "Execute a short ordered group of computer actions, then return a fresh screenshot. Coordinates use the latest screenshot pixels, NOT desktop pixels. Escape stops execution.", vec![("actions", "array")]),
        (
            "computer.focus",
            "Focus a window after approval",
            vec![("windowId", "number")],
        ),
        (
            "computer.click",
            "Click a position on the real desktop after approval",
            vec![("x", "number"), ("y", "number")],
        ),
        (
            "computer.type",
            "Type text on the real desktop after approval",
            vec![("text", "string")],
        ),
        (
            "computer.key",
            "Send a shortcut such as Ctrl+C after approval",
            vec![("key", "string")],
        ),
        (
            "computer.scroll",
            "Scroll on the real desktop after approval",
            vec![("amount", "number")],
        ),
    ];
    definitions.into_iter().map(|(name,description,fields)|{
        let mut properties=fields.iter().map(|(name,kind)|(name.to_string(),json!({"type":kind}))).collect::<serde_json::Map<_,_>>();
        if name.starts_with("filesystem.") || name.starts_with("coding.") || name == "terminal.execute" { properties.insert("workspace".into(),json!({"type":"string","description":"Absolute workspace folder to use, or a requested new folder; requires user approval before access."})); }
        if name.starts_with("computer.") {
            properties.insert("settleMs".into(),json!({"type":"integer","minimum":0,"maximum":5000,"description":"Delay after the action before the screenshot, default 700 ms. Increase for slow animations or loading."}));
            properties.insert("windowId".into(),json!({"type":"number","description":"Target window ID from computer.windows."}));
            properties.insert("frameId".into(),json!({"type":"string","description":"ID of the latest observed screenshot; stale IDs are rejected."}));
        }
        if name == "computer.capture" { properties.insert("scope".into(),json!({"type":"string","enum":["window","desktop"],"description":"Default window: capture the focused target only. desktop: whole virtual screen."})); }
        if name == "computer.click" { properties.insert("button".into(),json!({"type":"string","enum":["left","right","middle"]})); }
        if name == "computer.actions" {
            properties.insert("actions".into(),json!({"type":"array","minItems":1,"maxItems":12,"items":{"type":"object","properties":{"type":{"type":"string","enum":["click","double_click","move","drag","type","key","scroll","wait"]},"x":{"type":"number"},"y":{"type":"number"},"button":{"type":"string","enum":["left","right","middle"]},"path":{"type":"array","minItems":2,"maxItems":100,"items":{"type":"object","properties":{"x":{"type":"number"},"y":{"type":"number"}},"required":["x","y"],"additionalProperties":false}},"text":{"type":"string"},"key":{"type":"string"},"amount":{"type":"number"},"milliseconds":{"type":"integer","minimum":1,"maximum":2000},"frameId":{"type":"string"}},"required":["type"],"additionalProperties":false}}));
        }
        if name == "web.search" || name == "web.images" { properties.insert("endpoint".into(),json!({"type":"string","description":"Optional configured SearXNG URL. Omit for built-in search. Never invent a server."})); }
        if name == "terminal.execute" { properties.insert("readOnly".into(),json!({"type":"boolean","description":"Default true: read the approved live workspace without copying it; writes are denied by Windows. False: execute in a disposable writable snapshot."})); properties.insert("shell".into(),json!({"type":"string","enum":if cfg!(windows) { vec!["auto","powershell","cmd"] } else { vec!["auto","sh","bash","zsh"] },"description":"Default auto: native CLI commands use cmd for low startup latency; PowerShell syntax uses powershell. Explicit selection is respected."})); properties.insert("cwd".into(),json!({"type":"string","description":"Working subdirectory inside the workspace. Use the project subfolder to avoid copying unrelated files."})); properties.insert("applyChanges".into(),json!({"type":"boolean","description":"Ask Converge to apply changes from the isolated copy after the command."})); }
        (name.replace('.',"_"),description.to_string(),json!({"type":"object","properties":properties,"required":fields.iter().map(|f|f.0).collect::<Vec<_>>(),"additionalProperties":false}))
    }).collect()
}

pub(crate) fn scoped(root: &Path, relative: &str, write: bool) -> Result<PathBuf, String> {
    let rel = Path::new(relative);
    if rel.is_absolute()
        || rel.components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err("Utilisez un chemin relatif dans le dossier autorisé".into());
    }
    let candidate = root.join(rel);
    let target = if write {
        crate::workspaces::future_path(&candidate)?
    } else {
        candidate
            .canonicalize()
            .map_err(|_| "Fichier introuvable")?
    };
    if !target.starts_with(root) {
        return Err("Le chemin sort du dossier autorisé (lien symbolique inclus)".into());
    }
    Ok(target)
}
pub(crate) fn check_enabled(state: &AppState, name: &str) -> Result<(), String> {
    let raw: Option<String> = state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .query_row("SELECT data FROM state WHERE id=1", [], |r| r.get(0))
        .ok();
    let workspace: Value = raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null);
    let default = matches!(name, "filesystem.read" | "filesystem.list");
    let enabled = workspace["settings"]["enabledTools"]
        .as_array()
        .map(|list| list.iter().any(|v| v == name))
        .unwrap_or(default);
    if !enabled {
        return Err(format!(
            "L’outil {name} est désactivé. Activez-le dans Paramètres → Outils et permissions."
        ));
    }
    Ok(())
}
pub(crate) async fn execute_inner(
    app: &tauri::AppHandle,
    state: &AppState,
    name: &str,
    args: &Value,
    approved_root: Option<PathBuf>,
) -> Result<String, String> {
    let get = |key: &str| {
        args[key]
            .as_str()
            .ok_or_else(|| format!("Argument manquant : {key}"))
    };
    match name {
        "skills.read" | "mcp.list" | "mcp.call" | "mcp.read" | "mcp.prompt" => {
            crate::extensions::execute(app, state, name, args).await
        }
        "filesystem.read" | "filesystem.write" | "filesystem.list" | "coding.search"
        | "coding.patch" => {
            let root = approved_root
                .clone()
                .ok_or("Choisissez d’abord un dossier dans les paramètres des outils")?;
            execute_filesystem(&root, name, args)
        }
        "web.fetch" => {
            let u = crate::providers::validate_url(get("url")?)?;
            let r = state
                .client
                .get(u)
                .timeout(std::time::Duration::from_secs(25))
                .send()
                .await
                .map_err(|_| "Page inaccessible")?;
            if !r.status().is_success() {
                return Err(format!("HTTP {}", r.status()));
            }
            use futures_util::StreamExt;
            let mut stream = r.bytes_stream();
            let mut bytes = vec![];
            while let Some(chunk) = stream.next().await {
                bytes.extend_from_slice(&chunk.map_err(|_| "Page incomplète")?);
                if bytes.len() > 2 * 1024 * 1024 {
                    return Err("Page trop volumineuse".into());
                }
            }
            let html = String::from_utf8_lossy(&bytes);
            let doc = scraper::Html::parse_document(&html);
            let selector = scraper::Selector::parse("body").map_err(|e| e.to_string())?;
            Ok(doc
                .select(&selector)
                .flat_map(|e| e.text())
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(50000)
                .collect())
        }
        "web.search" | "web.images" => {
            crate::search::execute(&state.client, args, name == "web.images").await
        }
        "terminal.execute" => {
            crate::terminal::execute(state, app, args, None, approved_root, None).await
        }
        name if name.starts_with("computer.") => {
            let name = name.to_owned();
            let args = args.clone();
            let cid = args["convergeConversationId"]
                .as_str()
                .ok_or("Conversation Computer Use manquante")?
                .to_owned();
            let token = state
                .requests
                .lock()
                .map_err(|e| e.to_string())?
                .get(&format!(
                    "agent:{}",
                    args["requestId"].as_str().unwrap_or_default()
                ))
                .cloned()
                .unwrap_or_default();
            crate::desktop_control::begin(app, &cid, token.clone())?;
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                use tauri::Manager;
                let state = app.state::<AppState>();
                let mut sessions = state.computer_target.lock().map_err(|e| e.to_string())?;
                crate::computer::execute(&name, &args, sessions.entry(cid).or_default(), &token)
            })
            .await
            .map_err(|e| e.to_string())?
        }
        _ => Err("Outil inconnu ou non autorisé".into()),
    }
}
pub(crate) fn execute_filesystem(root: &Path, name: &str, args: &Value) -> Result<String, String> {
    execute_filesystem_progress(root, name, args, None, None)
}
pub(crate) fn execute_filesystem_progress(
    root: &Path,
    name: &str,
    args: &Value,
    channel: Option<&tauri::ipc::Channel<Value>>,
    cancel: Option<&tokio_util::sync::CancellationToken>,
) -> Result<String, String> {
    let get = |key: &str| {
        args[key]
            .as_str()
            .ok_or_else(|| format!("Argument manquant : {key}"))
    };
    let path = scoped(&root, get("path")?, name == "filesystem.write")?;
    match name {
        "filesystem.read" => {
            if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 {
                return Err("Limite de lecture : 2 Mo".into());
            }
            std::fs::read_to_string(path).map_err(|e| e.to_string())
        }
        "filesystem.write" => {
            let text = get("content")?;
            if text.len() > 2 * 1024 * 1024 {
                return Err("Limite d’écriture : 2 Mo".into());
            }
            crate::file_changes::write(root, get("path")?, text, channel, cancel)
        }
        "coding.patch" => {
            let find = get("search")?;
            let replace = get("replace")?;
            if find.is_empty() {
                return Err("Le texte à remplacer ne peut pas être vide".into());
            }
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            if text.len() > 2 * 1024 * 1024 {
                return Err("Fichier trop volumineux".into());
            }
            if text.matches(find).count() != 1 {
                return Err("Le texte recherché doit apparaître exactement une fois ; aucun changement effectué".into());
            }
            crate::file_changes::write(
                root,
                get("path")?,
                &text.replacen(find, replace, 1),
                channel,
                cancel,
            )
        }
        "filesystem.list" => {
            let list=std::fs::read_dir(path).map_err(|e|e.to_string())?.take(500).filter_map(Result::ok).map(|e|json!({"name":e.file_name().to_string_lossy(),"directory":e.file_type().map(|t|t.is_dir()).unwrap_or(false)})).collect::<Vec<_>>();
            Ok(json!(list).to_string())
        }
        _ => {
            let query = get("query")?;
            if query.is_empty() {
                return Err("Recherche vide".into());
            }
            let mut results = vec![];
            search(&root, &path, query, 0, &mut results)?;
            Ok(json!(results).to_string())
        }
    }
}
fn search(
    root: &Path,
    path: &Path,
    query: &str,
    depth: u8,
    results: &mut Vec<Value>,
) -> Result<(), String> {
    if depth > 6 || results.len() >= 100 {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .take(1000)
        .filter_map(Result::ok)
    {
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            if ![".git", "node_modules", "target"]
                .contains(&entry.file_name().to_string_lossy().as_ref())
            {
                search(root, &path, query, depth + 1, results)?
            }
        } else if kind.is_file() && entry.metadata().map(|m| m.len() < 256000).unwrap_or(false) {
            if let Ok(text) = std::fs::read_to_string(&path) {
                for (n, line) in text.lines().enumerate() {
                    if line.contains(query) {
                        results.push(json!({"path":path.strip_prefix(root).unwrap_or(&path).to_string_lossy(),"line":n+1,"text":line.chars().take(300).collect::<String>()}));
                        if results.len() >= 100 {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
#[tauri::command]
pub async fn read_attachment(path: String) -> Result<Value, String> {
    let path = Path::new(&path);
    if tokio::fs::metadata(path)
        .await
        .map_err(|e| e.to_string())?
        .len()
        > 10 * 1024 * 1024
    {
        return Err("Pièce jointe limitée à 10 Mo".into());
    }
    let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
    if bytes.len() > 10 * 1024 * 1024 {
        return Err("Pièce jointe limitée à 10 Mo".into());
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "text/plain",
    };
    let data = if ext == "pdf" {
        pdf_extract::extract_text_from_mem(&bytes)
            .map_err(|e| format!("Lecture PDF impossible : {e}"))?
    } else if ext == "docx" {
        use std::io::Read;
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let mut document = archive
            .by_name("word/document.xml")
            .map_err(|_| "Document Word invalide")?;
        if document.size() > 20 * 1024 * 1024 {
            return Err("Document décompressé trop volumineux".into());
        }
        let mut xml = String::new();
        document
            .read_to_string(&mut xml)
            .map_err(|e| e.to_string())?;
        let html = xml
            .replace("</w:p>", "\n")
            .replace("</w:tr>", "\n")
            .replace("</w:tc>", "\t");
        scraper::Html::parse_fragment(&html)
            .root_element()
            .text()
            .collect::<Vec<_>>()
            .join("")
    } else if mime == "text/plain" {
        String::from_utf8(bytes).map_err(|_|"Format de document non reconnu. Formats pris en charge : texte UTF-8, PDF, DOCX et images.".to_string())?
    } else {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    };
    Ok(
        json!({"id":uuid::Uuid::new_v4().to_string(),"name":path.file_name().unwrap_or_default().to_string_lossy(),"mime":mime,"data":data}),
    )
}
#[tauri::command]
pub async fn import_file(path: String) -> Result<String, String> {
    if tokio::fs::metadata(&path)
        .await
        .map_err(|e| e.to_string())?
        .len()
        > 128 * 1024 * 1024
    {
        return Err("Import limité à 128 Mo".into());
    }
    tokio::fs::read_to_string(path)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn export_file(path: String, content: String) -> Result<(), String> {
    let bytes = if content.starts_with("data:image/") {
        let (_, b64) = content.split_once(',').ok_or("Image invalide")?;
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|_| "Image invalide")?
    } else {
        content.into_bytes()
    };
    tokio::fs::write(path, bytes)
        .await
        .map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    #[test]
    fn denies_traversal() {
        assert!(super::scoped(std::path::Path::new("."), "../secret", false).is_err());
        assert!(super::scoped(std::path::Path::new("."), "C:\\Windows\\secret", false).is_err());
    }
}
