use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};
use tokio_util::sync::CancellationToken;

struct Lease {
    conversation: String,
    tokens: Vec<CancellationToken>,
}
#[derive(Default)]
pub struct DesktopControl {
    active: Mutex<Option<Lease>>,
    start_gate: Mutex<()>,
    runs: Mutex<HashMap<String, (String, String)>>,
}
pub struct RunGuard {
    app: tauri::AppHandle,
    id: String,
}
impl Drop for RunGuard {
    fn drop(&mut self) {
        if let Ok(mut runs) = self.app.state::<DesktopControl>().runs.lock() {
            runs.remove(&self.id);
        }
    }
}
pub fn track_run(
    app: &tauri::AppHandle,
    conversation: Option<&str>,
    request_key: String,
) -> Option<RunGuard> {
    let conversation = conversation?;
    let id = uuid::Uuid::new_v4().to_string();
    app.state::<DesktopControl>()
        .runs
        .lock()
        .ok()?
        .insert(id.clone(), (conversation.into(), request_key));
    Some(RunGuard {
        app: app.clone(),
        id,
    })
}
// A later approval request belongs to the same control session. Escape must
// cancel it too, even before that next action has reached execute_inner.
pub fn attach(app: &tauri::AppHandle, conversation: &str, token: CancellationToken) {
    if let Ok(mut active) = app.state::<DesktopControl>().active.lock() {
        if let Some(lease) = active
            .as_mut()
            .filter(|lease| lease.conversation == conversation)
        {
            lease.tokens.push(token);
        }
    }
}

pub fn begin(
    app: &tauri::AppHandle,
    conversation: &str,
    token: CancellationToken,
) -> Result<(), String> {
    let runtime = app.state::<DesktopControl>();
    // Never hold `active` while dispatching window work to the main thread:
    // Escape is handled there and must be able to acquire that same lock.
    let _start = runtime.start_gate.lock().map_err(|_| "Contrôle occupé")?;
    let mut active = runtime.active.lock().map_err(|_| "Contrôle occupé")?;
    if let Some(lease) = active.as_mut() {
        if lease.conversation != conversation {
            return Err("Une autre conversation contrôle déjà le bureau".into());
        }
        lease.tokens.push(token);
        return Ok(());
    }
    *active = Some(Lease {
        conversation: conversation.into(),
        tokens: vec![token.clone()],
    });
    drop(active);
    let shortcut = Shortcut::new(None, Code::Escape);
    if app.global_shortcut().register(shortcut).is_err() {
        release(app, Some(conversation), false);
        return Err(
            "Impossible d’activer Échap pour reprendre le contrôle. Aucune action envoyée.".into(),
        );
    }
    let window = if let Some(window) = app.get_webview_window("computer-control") {
        window
    } else {
        match tauri::WebviewWindowBuilder::new(
            app,
            "computer-control",
            tauri::WebviewUrl::App("control.html".into()),
        )
        .title("Converge computer control")
        .decorations(false)
        .inner_size(430.0, 64.0)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .build()
        {
            Ok(window) => window,
            Err(_) => {
                release(app, Some(conversation), false);
                return Err(
                    "L’indicateur de contrôle ne peut pas être affiché. Aucune action envoyée."
                        .into(),
                );
            }
        }
    };
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let scale = monitor.scale_factor();
        let size = monitor.size();
        let origin = monitor.position();
        let _ = window.set_position(tauri::PhysicalPosition::new(
            origin.x + (size.width as i32 - (430.0 * scale) as i32) / 2,
            origin.y + (16.0 * scale) as i32,
        ));
    }
    if window.show().is_err() {
        release(app, Some(conversation), false);
        return Err("Indicateur de contrôle indisponible".into());
    }
    if token.is_cancelled()
        || runtime
            .active
            .lock()
            .map_err(|_| "Contrôle occupé")?
            .is_none()
    {
        let _ = window.hide();
        let _ = app.global_shortcut().unregister(shortcut);
        return Err("CANCELLED: Contrôle arrêté pendant son initialisation".into());
    }
    Ok(())
}
pub fn release(app: &tauri::AppHandle, conversation: Option<&str>, user_stop: bool) {
    let runtime = app.state::<DesktopControl>();
    let lease = if let Ok(mut active) = runtime.active.lock() {
        if conversation.is_some_and(|id| active.as_ref().is_some_and(|a| a.conversation != id)) {
            return;
        }
        active.take()
    } else {
        None
    };
    if let Some(lease) = lease {
        // Stop the native inference stream too. Recovery must not depend on
        // React receiving an event while the model is thinking between tools.
        if user_stop {
            let keys = runtime
                .runs
                .lock()
                .map(|runs| {
                    runs.values()
                        .filter(|(cid, _)| cid == &lease.conversation)
                        .map(|(_, key)| key.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if let Ok(requests) = app.state::<crate::AppState>().requests.lock() {
                for key in keys {
                    if let Some(token) = requests.get(&key) {
                        token.cancel();
                    }
                }
            }
        }
        for token in lease.tokens {
            token.cancel();
        }
        let _ = app
            .global_shortcut()
            .unregister(Shortcut::new(None, Code::Escape));
        if let Some(window) = app.get_webview_window("computer-control") {
            let _ = window.hide();
        }
        if user_stop {
            let _ = app.emit_to(
                "main",
                "computer-control-stopped",
                json!({"conversationId":lease.conversation}),
            );
        }
    }
}
pub fn shortcut_handler(
    app: &tauri::AppHandle,
    shortcut: &Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if shortcut == &Shortcut::new(None, Code::Escape) && event.state() == ShortcutState::Pressed {
        release(app, None, true);
    }
}
#[tauri::command]
pub fn computer_control_stop(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if !matches!(window.label(), "main" | "computer-control") {
        return Err("Fenêtre non autorisée".into());
    }
    release(&app, None, true);
    Ok(())
}
#[tauri::command]
pub fn computer_control_finish(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    conversation_id: String,
) -> Result<(), String> {
    crate::permissions::trusted(&window)?;
    release(&app, Some(&conversation_id), false);
    Ok(())
}
