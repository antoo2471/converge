use crate::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

pub fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS state(id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS providers(id TEXT PRIMARY KEY, data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS conversations(id TEXT PRIMARY KEY,data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS images(id TEXT PRIMARY KEY,data TEXT NOT NULL); CREATE VIRTUAL TABLE IF NOT EXISTS conversation_search USING fts5(conversation_id UNINDEXED,title,body,tokenize='unicode61 remove_diacritics 2'); CREATE TABLE IF NOT EXISTS conversation_workspaces(conversation_id TEXT NOT NULL,path TEXT NOT NULL,position INTEGER NOT NULL,PRIMARY KEY(conversation_id,path)); PRAGMA user_version=3;")?;
    Ok(db)
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Provider {
    #[serde(default)]
    pub request: Value,
    pub id: String,
    pub name: String,
    pub plugin: String,
    pub protocol: String,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub models: Vec<String>,
    pub capabilities: Value,
    pub enabled: bool,
}
/// Discard retired transports on load without accessing shared CLI credentials.
fn normalize_provider(p: &mut Provider) {
    if p.plugin != "chatgpt" {
        return;
    }
    let was_shared = p.request["chatgpt"]["authSource"] == "codex";
    if let Some(config) = p.request.get_mut("chatgpt").and_then(Value::as_object_mut) {
        config.remove("authSource");
        config.remove("mode");
        if was_shared {
            config.remove("models");
        }
    }
    if was_shared {
        p.models.clear();
        p.capabilities["vision"] = Value::Bool(false);
    }
}
use std::collections::HashMap;
#[derive(Default, Serialize, Deserialize)]
pub struct Secret {
    pub key: String,
    pub headers: HashMap<String, String>,
}
fn entry(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("app.converge.desktop", id)
        .map_err(|_| "Coffre du système indisponible".into())
}
pub fn secret(id: &str) -> Result<Secret, String> {
    match entry(id)?.get_password() {
        Ok(s) => {
            serde_json::from_str(&s).map_err(|_| "Identifiants invalides dans le coffre".into())
        }
        Err(keyring::Error::NoEntry) => Ok(Secret::default()),
        Err(_) => Err("Impossible de lire le coffre du système".into()),
    }
}
pub fn provider(state: &AppState, id: &str) -> Result<Provider, String> {
    let data: String = state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .query_row("SELECT data FROM providers WHERE id=?1", [id], |r| r.get(0))
        .map_err(|_| "Fournisseur introuvable".to_string())?;
    let mut p: Provider = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    normalize_provider(&mut p);
    if !p.enabled {
        return Err("Fournisseur déconnecté".into());
    }
    Ok(p)
}
fn providers(db: &Connection) -> Result<Vec<Provider>, String> {
    let mut query = db
        .prepare("SELECT data FROM providers ORDER BY rowid")
        .map_err(|e| e.to_string())?;
    let rows = query
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| {
        let mut p: Provider =
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        normalize_provider(&mut p);
        Ok(p)
    })
    .collect()
}
#[tauri::command(async)]
pub fn load_workspace(state: State<AppState>) -> Result<Option<Value>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let data: Option<String> = db
        .query_row("SELECT data FROM state WHERE id=1", [], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    let mut value = if let Some(data) = data {
        serde_json::from_str::<Value>(&data).map_err(|e| e.to_string())?
    } else {
        serde_json::json!({"version":1,"conversations":[],"projects":[],"images":[]})
    };
    value["providers"] = serde_json::to_value(providers(&db)?).map_err(|e| e.to_string())?;
    for table in ["conversations", "images"] {
        let mut query = db
            .prepare(&format!("SELECT data FROM {table} ORDER BY rowid"))
            .map_err(|e| e.to_string())?;
        let records = query
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        // Old v1 single-document databases migrate transactionally on their next save.
        if !records.is_empty() || value[table].is_null() {
            value[table] = Value::Array(
                records
                    .iter()
                    .map(|s| serde_json::from_str::<Value>(s).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
    }
    Ok(Some(value))
}
#[tauri::command(async)]
pub fn save_workspace(state: State<AppState>, mut workspace: Value) -> Result<(), String> {
    save_workspace_inner(&state, &mut workspace)
}
fn save_workspace_inner(state: &AppState, workspace: &mut Value) -> Result<(), String> {
    if workspace["version"] != 1 {
        return Err("Version de données non supportée".into());
    }
    workspace
        .as_object_mut()
        .ok_or("Format invalide")?
        .remove("providers");
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    for table in ["conversations", "images"] {
        let items = workspace
            .as_object_mut()
            .ok_or("Format invalide")?
            .remove(table)
            .ok_or("Données manquantes")?;
        let items = items.as_array().ok_or("Liste attendue")?;
        let mut ids = std::collections::HashSet::new();
        for item in items {
            let id = item["id"]
                .as_str()
                .ok_or("Identifiant de document manquant")?;
            if id.len() > 100 || !ids.insert(id.to_string()) {
                return Err("Identifiant de document invalide ou dupliqué".into());
            }
            let data = serde_json::to_string(item).map_err(|e| e.to_string())?;
            if data.len() > 64 * 1024 * 1024 {
                return Err("Document trop volumineux (64 Mo maximum)".into());
            }
            let changed=tx.execute(&format!("INSERT INTO {table}(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data WHERE {table}.data != excluded.data"),params![id,data]).map_err(|e|e.to_string())?;
            if table == "conversations" && changed > 0 {
                tx.execute(
                    "DELETE FROM conversation_search WHERE conversation_id=?1",
                    [id],
                )
                .map_err(|e| e.to_string())?;
                let text = item["messages"]
                    .as_array()
                    .map(|messages| {
                        messages
                            .iter()
                            .filter_map(|m| m["content"].as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                tx.execute(
                    "INSERT INTO conversation_search(conversation_id,title,body) VALUES(?1,?2,?3)",
                    params![id, item["title"].as_str().unwrap_or_default(), text],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        let existing = tx
            .prepare(&format!("SELECT id FROM {table}"))
            .map_err(|e| e.to_string())?
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        for id in existing {
            if !ids.contains(&id) {
                tx.execute(&format!("DELETE FROM {table} WHERE id=?1"), [&id])
                    .map_err(|e| e.to_string())?;
                if table == "conversations" {
                    tx.execute(
                        "DELETE FROM conversation_search WHERE conversation_id=?1",
                        [id],
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    let data = serde_json::to_string(&workspace).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO state(id,data) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
        [data],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command(async)]
pub fn search_conversations(state: State<AppState>, query: String) -> Result<Vec<Value>, String> {
    if query.len() > 500 {
        return Err("Recherche trop longue".into());
    }
    let query = query
        .split_whitespace()
        .take(12)
        .map(|word| format!("\"{}\"*", word.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ");
    if query.is_empty() {
        return Ok(vec![]);
    }
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut statement=db.prepare("SELECT conversation_id,title,snippet(conversation_search,2,'','','…',24) FROM conversation_search WHERE conversation_search MATCH ?1 ORDER BY rank LIMIT 50").map_err(|e|e.to_string())?;
    let results=statement.query_map([query],|row|Ok(serde_json::json!({"id":row.get::<_,String>(0)?,"title":row.get::<_,String>(1)?,"snippet":row.get::<_,String>(2)?}))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
    Ok(results)
}
#[tauri::command(async)]
pub fn save_provider(
    state: State<AppState>,
    mut provider: Provider,
    key: Option<String>,
) -> Result<(), String> {
    crate::providers::validate(&provider)?;
    if (provider.plugin == "chatgpt"
        || provider.protocol == "cli"
        || crate::zai::oauth(&provider)
        || crate::deepseek::oauth(&provider))
        && key.as_ref().is_some_and(|s| !s.is_empty())
    {
        return Err("Ce fournisseur utilise une connexion OAuth, pas une clé API".into());
    }
    if uuid::Uuid::parse_str(&provider.id).is_err() {
        return Err("Identifiant fournisseur invalide".into());
    }
    let mut credentials = secret(&provider.id)?;
    if let Some(key) = key {
        credentials.key = key;
    }
    if !provider.headers.is_empty() {
        credentials.headers = provider.headers.clone();
    }
    if !credentials.key.is_empty() || !credentials.headers.is_empty() {
        entry(&provider.id)?
            .set_password(&serde_json::to_string(&credentials).map_err(|e| e.to_string())?)
            .map_err(|_| "Échec du stockage sécurisé ; connexion non enregistrée".to_string())?;
    }
    if !provider.enabled {
        let prefix = format!("{}:", provider.id);
        for (key, token) in state.requests.lock().map_err(|e| e.to_string())?.iter() {
            if key.starts_with(&prefix) {
                token.cancel()
            }
        }
    }
    provider.headers.clear();
    let data = serde_json::to_string(&provider).map_err(|e| e.to_string())?;
    state.db.lock().map_err(|e|e.to_string())?.execute("INSERT INTO providers(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",params![provider.id,data]).map_err(|e|e.to_string())?;
    Ok(())
}
#[tauri::command]
pub async fn remove_provider(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<(), String> {
    let p: Option<Provider> = {
        let db = state.db.lock().map_err(|_| "Base de données occupée")?;
        let data: Option<String> = db
            .query_row(
                "SELECT data FROM providers WHERE id=?1",
                [&provider_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| "Lecture du fournisseur impossible")?;
        data.map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|_| "Fournisseur invalide")?
    };
    let prefix = format!("{provider_id}:");
    for (key, token) in state.requests.lock().map_err(|e| e.to_string())?.iter() {
        if key.starts_with(&prefix) {
            token.cancel()
        }
    }
    if let Some(p) = p {
        if p.plugin == "chatgpt" {
            crate::chatgpt::remove(&app, &p).await?;
        } else if crate::cli_providers::supports(&p.plugin) {
            crate::cli_providers::remove(&app, &p).await?;
        }
        if p.plugin == "zai" {
            crate::zai::remove(&app, &p.id)?;
        }
        if p.plugin == "deepseek" {
            crate::deepseek::remove(&app, &p.id)?;
        }
    }
    match entry(&provider_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(_) => return Err("Impossible de supprimer les identifiants du coffre".into()),
    }
    state
        .db
        .lock()
        .map_err(|e| e.to_string())?
        .execute("DELETE FROM providers WHERE id=?1", [provider_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn retired_codex_sessions_are_removed_without_reusing_their_models() {
        let mut p:super::Provider=serde_json::from_value(serde_json::json!({"id":"legacy","name":"ChatGPT","plugin":"chatgpt","protocol":"codex","url":"https://chatgpt.com","headers":{},"models":["codex/model"],"enabled":true,"capabilities":{"vision":true,"tools":true,"images":false,"context":32768},"request":{"chatgpt":{"authSource":"codex","mode":"chat","models":[{"id":"model"}]}}})).unwrap();
        super::normalize_provider(&mut p);
        assert!(p.models.is_empty());
        assert!(p.request["chatgpt"].get("authSource").is_none());
        assert!(p.request["chatgpt"].get("mode").is_none());
        assert_eq!(p.capabilities["vision"], false);
    }
    #[test]
    fn database_starts_empty() {
        let db = super::open(std::path::Path::new(":memory:")).unwrap();
        assert!(super::providers(&db).unwrap().is_empty());
    }
    #[test]
    fn incremental_persistence_indexes_text_and_removes_deleted_documents() {
        use serde_json::json;
        use std::sync::Mutex;
        let state = crate::AppState {
            db: Mutex::new(super::open(std::path::Path::new(":memory:")).unwrap()),
            client: reqwest::Client::new(),
            requests: Mutex::new(Default::default()),

            terminal_gate: tokio::sync::Mutex::new(()),
            artifacts: Mutex::new(Default::default()),
            computer_target: Default::default(),
            permissions: Default::default(),
        };
        let mut w = json!({"version":1,"providers":[{"apiKey":"never-persist"}],"projects":[],"settings":{},"images":[],"conversations":[{"id":"c1","title":"Notes","messages":[{"content":"Un éléphant dans le contexte"}]}]});
        super::save_workspace_inner(&state, &mut w).unwrap();
        {
            let db = state.db.lock().unwrap();
            let count:i64=db.query_row("SELECT COUNT(*) FROM conversation_search WHERE conversation_search MATCH 'elephant'",[],|r|r.get(0)).unwrap();
            assert_eq!(count, 1);
            let metadata: String = db
                .query_row("SELECT data FROM state", [], |r| r.get(0))
                .unwrap();
            assert!(!metadata.contains("never-persist"));
            assert!(!metadata.contains("conversations"));
        }
        let mut empty =
            json!({"version":1,"projects":[],"settings":{},"images":[],"conversations":[]});
        super::save_workspace_inner(&state, &mut empty).unwrap();
        let db = state.db.lock().unwrap();
        assert_eq!(
            db.query_row::<i64, _, _>("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
                .unwrap(),
            0
        );
        assert_eq!(
            db.query_row::<i64, _, _>("SELECT COUNT(*) FROM conversation_search", [], |r| r.get(0))
                .unwrap(),
            0
        );
    }
}
