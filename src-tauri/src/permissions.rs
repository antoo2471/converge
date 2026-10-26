use crate::AppState;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Mutex,
};
use tauri::{ipc::Channel, State};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

type Approval = Result<Option<PathBuf>, String>;
struct Pending {
    conversation: String,
    requires_root: bool,
    root: Option<PathBuf>,
    new_workspace: bool,
    sender: oneshot::Sender<Approval>,
}
#[derive(Default)]
struct Decisions {
    pending: HashMap<String, Pending>,
    conversations: HashSet<String>,
}
#[derive(Default)]
pub struct PermissionManager {
    decisions: Mutex<Decisions>,
}
#[derive(Clone)]
pub struct Context {
    pub conversation: String,
    pub channel: Channel<Value>,
    pub cancel: CancellationToken,
    pub root: Option<PathBuf>,
    pub new_workspace: bool,
}

pub(crate) fn trusted(window: &tauri::WebviewWindow) -> Result<(), String> {
    let url = window.url().map_err(|e| e.to_string())?;
    if window.label() != "main" || !trusted_origin(&url) {
        return Err("Les permissions ne sont accessibles que depuis Converge".into());
    }
    Ok(())
}
fn trusted_origin(url: &url::Url) -> bool {
    (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (matches!(url.scheme(), "http" | "https") && url.host_str() == Some("tauri.localhost"))
        || (cfg!(debug_assertions)
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
            && url.port() == Some(1420))
}
pub async fn authorize(state: &AppState, context: &Context, name: &str, args: &Value) -> Approval {
    let permission_name = if name == "terminal.apply_changes" {
        "terminal.execute"
    } else {
        name
    };
    crate::tools::check_enabled(state, permission_name)?;
    if name == "computer.actions" {
        for action in args["actions"]
            .as_array()
            .ok_or("Liste d’actions manquante")?
        {
            let tool = match action["type"].as_str() {
                Some("click" | "move" | "double_click" | "drag") => "computer.click",
                Some("type") => "computer.type",
                Some("key") => "computer.key",
                Some("scroll") => "computer.scroll",
                Some("wait") => "computer.actions",
                _ => return Err("Action inconnue".into()),
            };
            crate::tools::check_enabled(state, tool)?;
        }
    }
    let root = context.root.clone();
    if name == "terminal.apply_changes"
        && !args["root"]
            .as_str()
            .and_then(|p| crate::workspaces::future_path(std::path::Path::new(p)).ok())
            .is_some_and(|target| root.as_ref().is_some_and(|r| target.starts_with(r)))
    {
        return Err(
            "Le dossier autorisé a changé ; les modifications n’ont pas été appliquées".into(),
        );
    }
    let requires_root = name.starts_with("filesystem.")
        || name.starts_with("coding.")
        || name.starts_with("terminal.");
    let request = uuid::Uuid::new_v4().to_string();
    let (sender, receiver) = oneshot::channel();
    {
        let mut decisions = state
            .permissions
            .decisions
            .lock()
            .map_err(|e| e.to_string())?;
        if decisions.conversations.contains(&context.conversation)
            && (!requires_root
                || (!context.new_workspace
                    && root.as_ref().is_some_and(|r| {
                        crate::workspaces::roots(state, &context.conversation)
                            .unwrap_or_default()
                            .contains(r)
                    })))
        {
            return Ok(root);
        }
        decisions.pending.insert(
            request.clone(),
            Pending {
                conversation: context.conversation.clone(),
                requires_root,
                root: root.clone(),
                new_workspace: context.new_workspace,
                sender,
            },
        );
    }
    if let Err(e) = context.channel.send(json!({"id":request,"conversationId":context.conversation,"name":name,"args":args,"root":root.as_ref().map(|p|crate::workspaces::display(p)),"requiresRoot":requires_root,"addWorkspace":context.new_workspace})) {
        state.permissions.decisions.lock().map_err(|e|e.to_string())?.pending.remove(&request);
        return Err(e.to_string());
    }
    let result = tokio::select! {
        _ = context.cancel.cancelled() => Err("CANCELLED: Action annulée".into()),
        _ = tokio::time::sleep(std::time::Duration::from_secs(600)) => Err("CANCELLED: Demande d’autorisation expirée".into()),
        result = receiver => result.unwrap_or_else(|_|Err("CANCELLED: Demande fermée".into())),
    };
    state
        .permissions
        .decisions
        .lock()
        .map_err(|e| e.to_string())?
        .pending
        .remove(&request);
    crate::tools::check_enabled(state, permission_name)?;
    if name == "computer.actions" {
        for action in args["actions"]
            .as_array()
            .ok_or("Liste d’actions manquante")?
        {
            let tool = match action["type"].as_str() {
                Some("click" | "move" | "double_click" | "drag") => "computer.click",
                Some("type") => "computer.type",
                Some("key") => "computer.key",
                Some("scroll") => "computer.scroll",
                Some("wait") => "computer.actions",
                _ => return Err("Action inconnue".into()),
            };
            crate::tools::check_enabled(state, tool)?;
        }
    }
    result
}

#[tauri::command]
pub fn resolve_tool_permission(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    request_id: String,
    decision: String,
    expected_root: Option<String>,
) -> Result<(), String> {
    trusted(&window)?;
    resolve(&state, &request_id, &decision, expected_root)
}
fn resolve(
    state: &AppState,
    request_id: &str,
    decision: &str,
    expected_root: Option<String>,
) -> Result<(), String> {
    if !["allow", "deny", "conversation"].contains(&decision) {
        return Err("Décision invalide".into());
    }
    let mut decisions = state
        .permissions
        .decisions
        .lock()
        .map_err(|e| e.to_string())?;
    let pending = decisions
        .pending
        .get(request_id)
        .ok_or("Cette demande n’est plus active")?;
    let mut root = pending.root.clone();
    if decision != "deny" && pending.requires_root {
        let expected = expected_root.ok_or("Choisissez un espace de travail pour cette action")?;
        let chosen = crate::workspaces::future_path(std::path::Path::new(&expected))?;
        let registered = crate::workspaces::roots(state, &pending.conversation)?;
        if !registered.contains(&chosen) {
            if !pending.new_workspace || pending.root.as_ref() != Some(&chosen) {
                return Err(
                    "Ajoutez ce dossier aux espaces de travail de cette conversation".into(),
                );
            }
            root = Some(crate::workspaces::add(
                state,
                &pending.conversation,
                &chosen,
                true,
            )?);
        } else {
            root = Some(chosen);
        }
    }
    let pending = decisions.pending.remove(request_id).unwrap();
    if decision == "conversation" {
        decisions.conversations.insert(pending.conversation.clone());
        let same: Vec<String> = decisions
            .pending
            .iter()
            .filter(|(_, p)| {
                p.conversation == pending.conversation
                    && (!p.requires_root
                        || (!p.new_workspace
                            && p.root.as_ref().is_some_and(|r| {
                                crate::workspaces::roots(state, &p.conversation)
                                    .unwrap_or_default()
                                    .contains(r)
                            })))
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in same {
            if let Some(p) = decisions.pending.remove(&id) {
                let _ = p.sender.send(Ok(p.root.clone()));
            }
        }
    }
    let _ = pending.sender.send(if decision == "deny" {
        Err(
            "REFUSED: Action refusée par l’utilisateur. Ne pas réessayer sans nouvelle demande."
                .into(),
        )
    } else {
        Ok(root)
    });
    Ok(())
}
#[tauri::command]
pub fn revoke_conversation_permissions(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), String> {
    trusted(&window)?;
    let mut decisions = state
        .permissions
        .decisions
        .lock()
        .map_err(|e| e.to_string())?;
    decisions.conversations.remove(&conversation_id);
    Ok(())
}

#[tauri::command]
pub async fn execute_agent_tool(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    request_id: String,
    name: String,
    mut args: Value,
    on_permission: Channel<Value>,
    on_output: Channel<Value>,
) -> Result<String, String> {
    trusted(&window)?;
    if uuid::Uuid::parse_str(&conversation_id).is_err()
        || uuid::Uuid::parse_str(&request_id).is_err()
        || !args.is_object()
        || args.to_string().len() > 1024 * 1024
    {
        return Err("Requête d’outil invalide".into());
    }
    if !crate::tools::schemas()
        .iter()
        .any(|(tool, _, _)| tool.replace('_', ".") == name)
    {
        return Err("Outil inconnu".into());
    }
    let token = CancellationToken::new();
    let key = format!("agent:{request_id}");
    {
        let mut requests = state.requests.lock().map_err(|e| e.to_string())?;
        if requests.contains_key(&key) {
            return Err("Requête déjà en cours".into());
        }
        requests.insert(key.clone(), token.clone());
    }
    let requires_root = name.starts_with("filesystem.")
        || name.starts_with("coding.")
        || name.starts_with("terminal.");
    let (root, new_workspace) = if requires_root {
        crate::workspaces::select_root(&app, &state, &conversation_id, &name, &args)?
    } else {
        (None, false)
    };
    let mut context = Context {
        conversation: conversation_id,
        channel: on_permission,
        cancel: token.clone(),
        root,
        new_workspace,
    };
    let result = async {
        if name.starts_with("computer.") {
            crate::desktop_control::attach(&app, &context.conversation, token.clone());
        }
        let root = authorize(&state, &context, &name, &args).await?;
        if token.is_cancelled() { return Err("CANCELLED: Action annulée".into()); }
        on_output.send(json!({"type":"started"})).map_err(|e|e.to_string())?;
        args["requestId"] = json!(request_id);
        args["convergeConversationId"] = json!(context.conversation);
        if let Some(root) = &root { crate::workspaces::normalize_args(&app, &mut args, root)?; }
        context.root = root.clone(); context.new_workspace = false;
        if name == "terminal.execute" {
            crate::terminal::execute(&state, &app, &args, Some(on_output.clone()), root, Some(context)).await
        } else if name.starts_with("filesystem.") || name.starts_with("coding.") {
            crate::tools::execute_filesystem_progress(root.as_deref().ok_or("Espace de travail manquant")?, &name, &args, Some(&on_output), Some(&token))
        } else {
            tokio::select! { _ = token.cancelled() => Err("CANCELLED: Action annulée".into()), result = crate::tools::execute_inner(&app, &state, &name, &args, root) => result }
        }
    }.await;
    state
        .requests
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&key);
    let _ = on_output.send(json!({"type":"finished"}));
    if token.is_cancelled() {
        Err("CANCELLED: Action annulée".into())
    } else {
        result
    }
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
