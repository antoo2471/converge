use crate::AppState;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};
use tauri::{Manager, State};

pub fn future_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() || path.components().any(|p| matches!(p, Component::ParentDir)) {
        return Err("Le chemin doit être absolu, sans remontée de dossier".into());
    }
    let mut existing = path.to_path_buf();
    let mut tail = Vec::new();
    while !existing.exists() {
        tail.push(
            existing
                .file_name()
                .ok_or("Chemin invalide")?
                .to_os_string(),
        );
        if !existing.pop() {
            return Err("Emplacement introuvable".into());
        }
    }
    let mut canonical = existing.canonicalize().map_err(|e| e.to_string())?;
    for part in tail.into_iter().rev() {
        canonical.push(part);
    }
    Ok(canonical)
}
pub fn display(path: &Path) -> String {
    let s = path.to_string_lossy();
    if let Some(unc) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
    }
}
pub fn roots(state: &AppState, conversation: &str) -> Result<Vec<PathBuf>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut query = db.prepare("SELECT path FROM conversation_workspaces WHERE conversation_id=?1 ORDER BY position,rowid").map_err(|e|e.to_string())?;
    let rows = query
        .query_map([conversation], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| r.map(PathBuf::from).map_err(|e| e.to_string()))
        .collect()
}
pub fn add(
    state: &AppState,
    conversation: &str,
    path: &Path,
    create: bool,
) -> Result<PathBuf, String> {
    let canonical = future_path(path)?;
    if create {
        std::fs::create_dir_all(&canonical).map_err(|e| e.to_string())?;
    }
    let canonical = canonical.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.is_dir() {
        return Err("Choisissez un dossier".into());
    }
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_workspaces WHERE conversation_id=?1 AND path=?2)",rusqlite::params![conversation,canonical.to_string_lossy()],|r|r.get(0)).map_err(|e|e.to_string())?;
    if exists {
        return Ok(canonical);
    }
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM conversation_workspaces WHERE conversation_id=?1",
            [conversation],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if count >= 20 {
        return Err("Maximum de 20 espaces de travail par conversation".into());
    }
    db.execute("INSERT OR IGNORE INTO conversation_workspaces(conversation_id,path,position) VALUES(?1,?2,?3)", rusqlite::params![conversation,canonical.to_string_lossy(),count]).map_err(|e|e.to_string())?;
    Ok(canonical)
}
fn validate(conversation: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(conversation)
        .map(|_| ())
        .map_err(|_| "Conversation invalide".into())
}
#[tauri::command]
pub fn conversation_workspace_context(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<AppState>,
    conversation_id: String,
) -> Result<Value, String> {
    crate::permissions::trusted(&window)?;
    validate(&conversation_id)?;
    Ok(
        json!({"folders":roots(&state,&conversation_id)?.iter().map(|p|display(p)).collect::<Vec<_>>(),"desktop":app.path().desktop_dir().ok().map(|p|display(&p)),"home":app.path().home_dir().ok().map(|p|display(&p))}),
    )
}
#[tauri::command(async)]
pub fn choose_conversation_workspace(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<AppState>,
    conversation_id: String,
) -> Result<Value, String> {
    use tauri_plugin_dialog::DialogExt;
    crate::permissions::trusted(&window)?;
    validate(&conversation_id)?;
    let mut selected = None;
    if let Some(folder) = app
        .dialog()
        .file()
        .set_title("Ajouter un espace de travail à cette conversation")
        .blocking_pick_folder()
    {
        let path = folder.into_path().map_err(|e| e.to_string())?;
        selected = Some(display(&add(&state, &conversation_id, &path, false)?));
    }
    Ok(
        json!({"folders":roots(&state,&conversation_id)?.iter().map(|p|display(p)).collect::<Vec<_>>(),"selected":selected}),
    )
}
#[tauri::command]
pub fn remove_conversation_workspace(
    window: tauri::WebviewWindow,
    state: State<AppState>,
    conversation_id: String,
    path: String,
) -> Result<Vec<String>, String> {
    crate::permissions::trusted(&window)?;
    validate(&conversation_id)?;
    let path = future_path(Path::new(&path))?;
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .execute(
            "DELETE FROM conversation_workspaces WHERE conversation_id=?1 AND path=?2",
            rusqlite::params![conversation_id, path.to_string_lossy()],
        )
        .map_err(|e| e.to_string())?;
    Ok(roots(&state, &conversation_id)?
        .iter()
        .map(|p| display(p))
        .collect())
}
pub fn absolute(app: &tauri::AppHandle, value: &str) -> Result<Option<PathBuf>, String> {
    if value == "@desktop" || value.starts_with("@desktop/") || value.starts_with("@desktop\\") {
        return future_path(
            &app.path().desktop_dir().map_err(|e| e.to_string())?.join(
                value
                    .trim_start_matches("@desktop")
                    .trim_start_matches(['/', '\\']),
            ),
        )
        .map(Some);
    }
    if value.starts_with("~/") || value.starts_with("~\\") {
        return future_path(
            &app.path()
                .home_dir()
                .map_err(|e| e.to_string())?
                .join(&value[2..]),
        )
        .map(Some);
    }
    if Path::new(value).is_absolute() {
        future_path(Path::new(value)).map(Some)
    } else {
        Ok(None)
    }
}
pub fn select_root(
    app: &tauri::AppHandle,
    state: &AppState,
    conversation: &str,
    name: &str,
    args: &Value,
) -> Result<(Option<PathBuf>, bool), String> {
    let roots = roots(state, conversation)?;
    let requested = if name == "terminal.apply_changes" {
        args["root"].as_str()
    } else if name.starts_with("terminal.") {
        args["cwd"].as_str().or(args["workspace"].as_str())
    } else {
        args["path"].as_str()
    };
    if let Some(target) = requested.map(|p| absolute(app, p)).transpose()?.flatten() {
        if let Some(root) = roots
            .iter()
            .filter(|r| target.starts_with(r))
            .max_by_key(|r| r.components().count())
        {
            return Ok((Some(root.clone()), false));
        }
        let proposal = if name.starts_with("terminal.") {
            target
        } else {
            target
                .parent()
                .ok_or("Dossier de destination invalide")?
                .to_path_buf()
        };
        return Ok((Some(proposal), true));
    }
    if let Some(workspace) = args["workspace"].as_str() {
        let requested =
            absolute(app, workspace)?.ok_or("L’espace de travail doit être un chemin absolu")?;
        let approved = roots.contains(&requested);
        return Ok((Some(requested), !approved));
    }
    Ok((roots.first().cloned(), false))
}
pub fn normalize_args(app: &tauri::AppHandle, args: &mut Value, root: &Path) -> Result<(), String> {
    for field in ["path", "cwd"] {
        if let Some(path) = args[field].as_str() {
            if let Some(target) = absolute(app, path)? {
                let relative = target.strip_prefix(root).map_err(|_| {
                    "L’emplacement demandé est hors de l’espace de travail autorisé"
                })?;
                args[field] = json!(if relative.as_os_str().is_empty() {
                    ".".into()
                } else {
                    relative.to_string_lossy().into_owned()
                });
            }
        }
    }
    Ok(())
}
