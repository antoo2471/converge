use serde_json::{json, Value};
use tauri::{ipc::Channel, Manager, State};
use tauri_plugin_updater::UpdaterExt;
#[derive(Default)]
pub struct Updates(pub tokio::sync::Mutex<()>);
#[tauri::command]
pub async fn app_update_check(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Value, String> {
    crate::permissions::trusted(&window)?;
    let update = app.updater().map_err(|_| "Configuration des mises à jour invalide")?.check().await
        .map_err(|_| "Impossible de vérifier les mises à jour. Vérifiez votre connexion et la disponibilité de la release GitHub.")?;
    Ok(update
        .map(|u| json!({"version":u.version,"currentVersion":u.current_version,"notes":u.body}))
        .unwrap_or(Value::Null))
}
#[tauri::command]
pub async fn app_update_install(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, Updates>,
    version: String,
    on_event: Channel<Value>,
) -> Result<(), String> {
    crate::permissions::trusted(&window)?;
    let _guard = state
        .0
        .try_lock()
        .map_err(|_| "Une mise à jour est déjà en cours")?;
    let control = app.state::<crate::AppState>();
    if !control
        .requests
        .lock()
        .map_err(|_| "Application occupée")?
        .is_empty()
    {
        return Err(
            "Attendez la fin des générations et des outils avant d’installer la mise à jour".into(),
        );
    }
    let update = app
        .updater()
        .map_err(|_| "Configuration invalide")?
        .check()
        .await
        .map_err(|_| "Mise à jour inaccessible")?
        .ok_or("Aucune mise à jour disponible")?;
    if update.version != version {
        return Err("La version disponible a changé. Vérifiez les mises à jour de nouveau.".into());
    }
    let mut downloaded = 0u64;
    let bytes=update.download(|length, total| { downloaded += length as u64; let _ = on_event.send(json!({"downloaded":downloaded,"total":total})); },||{}).await
        .map_err(|_| "La mise à jour n’a pas été installée. Vérifiez le réseau, l’espace disponible et la signature de la release.")?;
    if !control
        .requests
        .lock()
        .map_err(|_| "Application occupée")?
        .is_empty()
    {
        return Err(
            "Une tâche a commencé pendant le téléchargement. Relancez l’installation après sa fin."
                .into(),
        );
    }
    update
        .install(bytes)
        .map_err(|_| "Installation de la mise à jour impossible")?;
    app.restart();
}
