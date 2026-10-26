use crate::AppState;
use std::collections::{HashMap, VecDeque};
use tauri::{Manager, State};
#[derive(Default)]
pub struct ArtifactStore {
    documents: HashMap<String, String>,
    order: VecDeque<String>,
    runtime: Option<String>,
}
pub const POLICY:&str="default-src 'none'; script-src 'unsafe-inline' http://artifact.localhost https://artifact.localhost artifact: blob:; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; connect-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'; object-src 'none'";
#[tauri::command]
pub fn register_artifact(
    state: State<AppState>,
    code: String,
    kind: String,
    runtime: Option<String>,
) -> Result<String, String> {
    if code.len() > 2 * 1024 * 1024 || !["html", "react"].contains(&kind.as_str()) {
        return Err("Artifact invalide ou trop volumineux".into());
    }
    let mut store = state.artifacts.lock().map_err(|e| e.to_string())?;
    if let Some(runtime) = runtime {
        if runtime.len() > 5 * 1024 * 1024 {
            return Err("Runtime trop volumineux".into());
        }
        if store.runtime.is_none() {
            store.runtime = Some(runtime)
        }
    }
    let head="<meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><style>body{margin:0;padding:24px;font:14px system-ui,sans-serif;color:#202522;background:#fafbf8}*{box-sizing:border-box}button,input,select{font:inherit}button{cursor:pointer}</style>";
    let content = if kind == "react" {
        format!("<!doctype html><html><head>{head}</head><body><div id=\"app\"></div><script src=\"/runtime.js\"></script></body></html>")
    } else {
        format!("<!doctype html><html><head>{head}</head><body>{code}</body></html>")
    };
    let id = uuid::Uuid::new_v4().to_string();
    store.documents.insert(id.clone(), content);
    store.order.push_back(id.clone());
    while store.order.len() > 32 {
        if let Some(id) = store.order.pop_front() {
            store.documents.remove(&id);
        }
    }
    Ok(format!("/{id}.html"))
}
#[tauri::command]
pub fn release_artifact(state: State<AppState>, path: String) -> Result<(), String> {
    let id = path.trim_start_matches('/').trim_end_matches(".html");
    let mut store = state.artifacts.lock().map_err(|e| e.to_string())?;
    store.documents.remove(id);
    store.order.retain(|v| v != id);
    Ok(())
}
pub fn response(app: &tauri::AppHandle, path: &str) -> tauri::http::Response<Vec<u8>> {
    let state = app.state::<AppState>();
    let (body, mime, status) = if let Ok(store) = state.artifacts.lock() {
        if path == "/runtime.js" {
            (
                store.runtime.clone().unwrap_or_default(),
                "text/javascript; charset=utf-8",
                200,
            )
        } else {
            let id = path.trim_start_matches('/').trim_end_matches(".html");
            match store.documents.get(id) {
                Some(html) => (html.clone(), "text/html; charset=utf-8", 200),
                None => (
                    "Artifact expiré. Relancez la prévisualisation.".into(),
                    "text/plain; charset=utf-8",
                    404,
                ),
            }
        }
    } else {
        ("Runtime indisponible".into(), "text/plain", 503)
    };
    tauri::http::Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Content-Security-Policy", POLICY)
        .header("X-Content-Type-Options", "nosniff")
        .header("Cache-Control", "no-store")
        .body(body.into_bytes())
        .expect("Static response headers")
}
