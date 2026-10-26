use super::*;
use std::sync::Arc;
fn fixture() -> Arc<AppState> {
    let db = crate::persistence::open(std::path::Path::new(":memory:")).unwrap();
    db.execute("INSERT INTO state(id,data) VALUES(1,?1)",[json!({"settings":{"enabledTools":["web.fetch","filesystem.read","filesystem.write","coding.patch","terminal.execute"]}}).to_string()]).unwrap();
    Arc::new(AppState {
        db: Mutex::new(db),
        client: reqwest::Client::new(),
        requests: Mutex::new(Default::default()),
        terminal_gate: tokio::sync::Mutex::new(()),
        artifacts: Mutex::new(Default::default()),
        computer_target: Default::default(),
        permissions: Default::default(),
    })
}
fn context(
    cid: &str,
    root: Option<PathBuf>,
    new_workspace: bool,
) -> (Context, tokio::sync::mpsc::UnboundedReceiver<Value>) {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let channel = Channel::new(move |body| {
        if let tauri::ipc::InvokeResponseBody::Json(json) = body {
            let _ = sender.send(serde_json::from_str(&json).unwrap());
        }
        Ok(())
    });
    (
        Context {
            conversation: cid.into(),
            channel,
            cancel: CancellationToken::new(),
            root,
            new_workspace,
        },
        receiver,
    )
}
async fn event(receiver: &mut tokio::sync::mpsc::UnboundedReceiver<Value>) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(2), receiver.recv())
        .await
        .unwrap()
        .unwrap()
}
fn temp() -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("converge-workspaces-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    root.canonicalize().unwrap()
}
fn clean(root: PathBuf) {
    assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("converge-workspaces-test-"));
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn denial_and_cancellation_never_grant_access() {
    let state = fixture();
    let (ctx, mut events) = context("one", None, false);
    let task = {
        let state = state.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move { authorize(&state, &ctx, "web.fetch", &json!({})).await })
    };
    let request = event(&mut events).await;
    assert!(!task.is_finished());
    resolve(&state, request["id"].as_str().unwrap(), "deny", None).unwrap();
    assert!(task.await.unwrap().unwrap_err().starts_with("REFUSED:"));
    let task = {
        let state = state.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move { authorize(&state, &ctx, "web.fetch", &json!({})).await })
    };
    event(&mut events).await;
    ctx.cancel.cancel();
    assert!(task.await.unwrap().unwrap_err().starts_with("CANCELLED:"));
    assert!(state
        .permissions
        .decisions
        .lock()
        .unwrap()
        .pending
        .is_empty());
    assert!(state
        .permissions
        .decisions
        .lock()
        .unwrap()
        .conversations
        .is_empty());
}
#[tokio::test]
async fn workspaces_and_grants_stay_in_their_conversation() {
    let state = fixture();
    let root = temp();
    let second = temp();
    crate::workspaces::add(&state, "one", &root, false).unwrap();
    crate::workspaces::add(&state, "one", &second, false).unwrap();
    assert_eq!(crate::workspaces::roots(&state, "one").unwrap().len(), 2);
    assert!(crate::workspaces::roots(&state, "two").unwrap().is_empty());
    let (ctx, mut events) = context("one", Some(root.clone()), false);
    let task = {
        let state = state.clone();
        tokio::spawn(async move {
            authorize(
                &state,
                &ctx,
                "filesystem.read",
                &json!({"path":"hello.txt"}),
            )
            .await
        })
    };
    let request = event(&mut events).await;
    resolve(
        &state,
        request["id"].as_str().unwrap(),
        "conversation",
        Some(crate::workspaces::display(&root)),
    )
    .unwrap();
    assert!(task.await.unwrap().is_ok());
    let (ctx, _) = context("one", Some(second.clone()), false);
    assert!(
        authorize(&state, &ctx, "filesystem.read", &json!({"path":"a"}))
            .await
            .is_ok()
    );
    let (ctx, mut events) = context("two", None, false);
    let task = {
        let state = state.clone();
        tokio::spawn(async move { authorize(&state, &ctx, "web.fetch", &json!({})).await })
    };
    let request = event(&mut events).await;
    resolve(&state, request["id"].as_str().unwrap(), "deny", None).unwrap();
    assert!(task.await.unwrap().is_err());
    clean(root);
    clean(second);
}
#[tokio::test]
async fn proposed_location_is_added_only_after_consent_and_nested_write_succeeds() {
    let state = fixture();
    let base = temp();
    let project = base.join("new-project");
    let (ctx, mut events) = context("one", Some(project.clone()), true);
    let task = {
        let state = state.clone();
        tokio::spawn(async move {
            authorize(
                &state,
                &ctx,
                "filesystem.write",
                &json!({"path":"site/index.html","content":"hello\n"}),
            )
            .await
        })
    };
    let request = event(&mut events).await;
    assert_eq!(request["addWorkspace"], true);
    assert!(!project.exists());
    assert!(crate::workspaces::roots(&state, "one").unwrap().is_empty());
    resolve(
        &state,
        request["id"].as_str().unwrap(),
        "allow",
        Some(crate::workspaces::display(&project)),
    )
    .unwrap();
    let root = task.await.unwrap().unwrap().unwrap();
    assert_eq!(
        crate::workspaces::roots(&state, "one").unwrap(),
        vec![root.clone()]
    );
    crate::tools::execute_filesystem(
        &root,
        "filesystem.write",
        &json!({"path":"site/nested/index.html","content":"hello\n"}),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(project.join("site/nested/index.html")).unwrap(),
        "hello\n"
    );
    assert!(crate::tools::execute_filesystem(
        &root,
        "filesystem.write",
        &json!({"path":"../escape.txt","content":"no"})
    )
    .is_err());
    let denied = base.join("denied");
    let (ctx, mut events) = context("one", Some(denied.clone()), true);
    let task = {
        let state = state.clone();
        tokio::spawn(async move {
            authorize(
                &state,
                &ctx,
                "filesystem.write",
                &json!({"path":"x","content":"no"}),
            )
            .await
        })
    };
    let request = event(&mut events).await;
    resolve(&state, request["id"].as_str().unwrap(), "deny", None).unwrap();
    assert!(task.await.unwrap().is_err());
    assert!(!denied.exists());
    clean(base);
}
#[tokio::test]
async fn disabled_tools_remain_disabled() {
    let state = fixture();
    state
        .permissions
        .decisions
        .lock()
        .unwrap()
        .conversations
        .insert("one".into());
    let (ctx, _) = context("one", None, false);
    assert!(authorize(&state, &ctx, "computer.click", &json!({}))
        .await
        .is_err());
}

#[tokio::test]
async fn grouped_computer_actions_cannot_bypass_disabled_inputs() {
    let state = fixture();
    state
        .db
        .lock()
        .unwrap()
        .execute(
            "UPDATE state SET data=?1 WHERE id=1",
            [
                json!({"settings":{"enabledTools":["computer.actions","computer.type"]}})
                    .to_string(),
            ],
        )
        .unwrap();
    state
        .permissions
        .decisions
        .lock()
        .unwrap()
        .conversations
        .insert("one".into());
    let (ctx, _) = context("one", None, false);
    assert!(authorize(
        &state,
        &ctx,
        "computer.actions",
        &json!({"actions":[{"type":"click","x":0,"y":0}]})
    )
    .await
    .is_err());
    assert!(authorize(
        &state,
        &ctx,
        "computer.actions",
        &json!({"actions":[{"type":"type","text":"allowed"}]})
    )
    .await
    .is_ok());
}
