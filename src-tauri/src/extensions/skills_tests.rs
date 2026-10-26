use super::*;
const VALID:&str="---\nname: test-skill\ndescription: |\n  Instructions for the test.\n---\n\nRead references/guide.md before proceeding.\n";
#[test]
fn importing_a_skill_keeps_resources_and_enforces_enabled_state() {
    let root =
        std::env::temp_dir().join(format!("converge-skills-contract-{}", uuid::Uuid::new_v4()));
    let source = root.join("source");
    let library = root.join("library");
    std::fs::create_dir_all(source.join("references")).unwrap();
    std::fs::write(source.join("SKILL.md"), VALID).unwrap();
    std::fs::write(
        source.join("references/guide.md"),
        "Actual reference content",
    )
    .unwrap();
    let state = super::super::test_state();
    let skill = action(
        &library,
        &state,
        "import",
        None,
        Some(source.to_str().unwrap()),
        None,
        None,
    )
    .unwrap();
    let id = skill["id"].as_str().unwrap();
    assert_eq!(skill["name"], "test-skill");
    assert!(read(
        &library,
        &state,
        &json!({"skillId":id,"path":"references/guide.md"})
    )
    .unwrap()
    .contains("Actual reference content"));
    assert!(read(
        &library,
        &state,
        &json!({"skillId":id,"path":"../source/SKILL.md"})
    )
    .is_err());
    action(
        &library,
        &state,
        "toggle",
        Some(id),
        None,
        None,
        Some(false),
    )
    .unwrap();
    assert!(read(&library, &state, &json!({"skillId":id,"path":"SKILL.md"})).is_err());
    assert!(
        action(&library, &state, "view", Some(id), None, None, None).unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("test-skill")
    );
    action(&library, &state, "remove", Some(id), None, None, None).unwrap();
    assert!(source.join("SKILL.md").is_file());
    assert!(super::super::records(&state, "skill").unwrap().is_empty());
    assert!(root
        .canonicalize()
        .unwrap()
        .starts_with(std::env::temp_dir().canonicalize().unwrap()));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn zip_import_rejects_traversal_and_missing_skill_metadata() {
    use std::io::Write;
    let root = std::env::temp_dir().join(format!("converge-skill-zip-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("bad.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    zip.start_file("../escape.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"refuse this").unwrap();
    zip.finish().unwrap();
    let state = super::super::test_state();
    assert!(action(
        &root,
        &state,
        "import",
        None,
        Some(path.to_str().unwrap()),
        None,
        None
    )
    .is_err());
    assert!(!root.join("escape.txt").exists());
    assert!(skill_meta("---\nname: missing-description\n---\ntext").is_err());
    assert!(action(&root, &state, "create", None, None, Some(VALID), None).is_ok());
    assert!(root
        .canonicalize()
        .unwrap()
        .starts_with(std::env::temp_dir().canonicalize().unwrap()));
    std::fs::remove_dir_all(root).unwrap();
}
