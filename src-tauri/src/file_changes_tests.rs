use super::*;
#[test]
fn counts_real_diff_and_cancel_preserves_original() {
    let root = std::env::temp_dir().join(format!("converge-diff-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
    let captured = events.clone();
    let channel = Channel::new(move |body| {
        if let tauri::ipc::InvokeResponseBody::Json(json) = body {
            captured
                .lock()
                .unwrap()
                .push(serde_json::from_str(&json).unwrap());
        }
        Ok(())
    });
    write(&root, "nested/site.txt", "one\ntwo\n", Some(&channel), None).unwrap();
    write(
        &root,
        "nested/site.txt",
        "one\nthree\nfour\n",
        Some(&channel),
        None,
    )
    .unwrap();
    let rows = events.lock().unwrap();
    let diff = rows
        .iter()
        .filter(|e| e["type"] == "file_changes")
        .last()
        .unwrap();
    assert_eq!(diff["added"], 2);
    assert_eq!(diff["removed"], 1);
    drop(rows);
    let token = CancellationToken::new();
    token.cancel();
    assert!(write(&root, "nested/site.txt", "bad", None, Some(&token)).is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("nested/site.txt")).unwrap(),
        "one\nthree\nfour\n"
    );
    assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("converge-diff-test-"));
    std::fs::remove_dir_all(root).unwrap();
}
