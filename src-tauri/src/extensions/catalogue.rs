use super::*;
use std::io::{Cursor, Read};
use std::path::Path;

fn source_parts(source: &str) -> Result<(&str, &str), String> {
    let parts = source.split('/').collect::<Vec<_>>();
    if parts.len() != 2
        || parts.iter().any(|p| {
            p.is_empty()
                || p.len() > 100
                || p.starts_with('.')
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_ .".contains(&b) && b != b' ')
        })
    {
        return Err("Source GitHub du skill invalide".into());
    }
    Ok((parts[0], parts[1]))
}

#[tauri::command]
pub async fn skills_catalogue(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    query: String,
) -> Result<Value, String> {
    crate::permissions::trusted(&window)?;
    if query.len() > 200 {
        return Err("Recherche trop longue".into());
    }
    let response = state
        .client
        .get("https://skills.sh/api/search")
        .query(&[
            (
                "q",
                if query.trim().is_empty() {
                    "skills"
                } else {
                    query.trim()
                },
            ),
            ("limit", "30"),
        ])
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| "La bibliothèque skills.sh est inaccessible")?;
    let data = bounded_response(response, 2 * 1024 * 1024).await?;
    let value: Value = serde_json::from_slice(&data).map_err(|_| "Réponse skills.sh invalide")?;
    let entries = value["skills"].as_array().ok_or("Catalogue skills.sh invalide")?.iter().take(30)
        .filter(|entry| entry["source"].as_str().is_some_and(|s| source_parts(s).is_ok()))
        .map(|entry| json!({"id":entry["id"],"name":entry["name"],"source":entry["source"],"skillId":entry["skillId"],"installs":entry["installs"]})).collect::<Vec<_>>();
    Ok(json!({"skills":entries}))
}
async fn bounded_response(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, String> {
    if !response.status().is_success() {
        return Err(format!(
            "Le service a renvoyé HTTP {}",
            response.status().as_u16()
        ));
    }
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err("Téléchargement trop volumineux".into());
    }
    let mut data = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Téléchargement interrompu")?
    {
        if data.len() + chunk.len() > limit {
            return Err("Téléchargement trop volumineux".into());
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}
fn extract_skill(data: Vec<u8>, destination: &Path, name: &str) -> Result<PathBuf, String> {
    let mut zip =
        zip::ZipArchive::new(Cursor::new(data)).map_err(|_| "Archive du dépôt invalide")?;
    if zip.len() > 20000 {
        return Err("Le dépôt contient trop de fichiers".into());
    }
    let mut selected = None;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|_| "Archive invalide")?;
        let path = entry
            .enclosed_name()
            .ok_or("Chemin d’archive extérieur refusé")?;
        if path.file_name().is_some_and(|n| n == "SKILL.md") && entry.size() <= 256 * 1024 {
            let mut text = String::new();
            entry
                .by_ref()
                .take(256 * 1024 + 1)
                .read_to_string(&mut text)
                .map_err(|_| "Skill non UTF-8")?;
            let metadata = super::skills::skill_meta(&text).ok();
            if metadata.is_some_and(|(n, _)| n == name)
                || path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|n| n == name)
            {
                if selected.is_some() {
                    return Err("Plusieurs skills correspondent à ce nom dans le dépôt".into());
                }
                selected = path.parent().map(Path::to_path_buf);
            }
        }
    }
    let selected = selected.ok_or("Ce skill n’existe plus dans la branche par défaut du dépôt")?;
    let mut total = 0u64;
    let mut count = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|_| "Archive invalide")?;
        let path = entry
            .enclosed_name()
            .ok_or("Chemin d’archive extérieur refusé")?;
        let Ok(relative) = path.strip_prefix(&selected) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        if entry.unix_mode().is_some_and(|v| v & 0o170000 == 0o120000) {
            return Err("Lien symbolique refusé".into());
        }
        count += 1;
        total += entry.size();
        if count > 2000 || total > 64 * 1024 * 1024 {
            return Err("Le skill dépasse les limites d’import".into());
        }
        let path = destination.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(path).map_err(|_| "Extraction impossible")?;
            continue;
        }
        std::fs::create_dir_all(path.parent().ok_or("Chemin invalide")?)
            .map_err(|_| "Extraction impossible")?;
        let mut file = std::fs::File::create(path).map_err(|_| "Extraction impossible")?;
        if std::io::copy(&mut entry.by_ref().take(64 * 1024 * 1024 + 1), &mut file)
            .map_err(|_| "Extraction impossible")?
            > 64 * 1024 * 1024
        {
            return Err("Fichier trop volumineux".into());
        }
    }
    Ok(destination.to_path_buf())
}
#[tauri::command]
pub async fn install_catalogue_skill(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    source: String,
    skill: String,
) -> Result<Value, String> {
    crate::permissions::trusted(&window)?;
    let (owner, repo) = source_parts(&source)?;
    if skill.is_empty() || skill.len() > 120 {
        return Err("Nom de skill invalide".into());
    }
    let response = state
        .client
        .get(format!(
            "https://codeload.github.com/{owner}/{repo}/zip/HEAD"
        ))
        .timeout(std::time::Duration::from_secs(90))
        .send()
        .await
        .map_err(|_| "Téléchargement GitHub impossible")?;
    let bytes = bounded_response(response, 32 * 1024 * 1024).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let runtime = app.state::<Extensions>();
        let temp = runtime
            .root
            .join("downloads")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&temp).map_err(|_| "Dossier d’import indisponible")?;
        let result = (|| {
            let path = extract_skill(bytes, &temp, &skill)?;
            let state = app.state::<AppState>();
            let mut record = super::skills::action(
                &runtime.root,
                &state,
                "import",
                None,
                path.to_str(),
                None,
                None,
            )?;
            record["source"] = json!(source);
            record["catalogueSkill"] = json!(skill);
            super::put(
                &state,
                "skill",
                record["id"].as_str().ok_or("Import invalide")?,
                &record,
            )?;
            Ok(record)
        })();
        let _ = std::fs::remove_dir_all(&temp); // Generated UUID under our private downloads folder only.
        result
    })
    .await
    .map_err(|_| "Import interrompu")?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive(files: &[(&str, &str)]) -> Vec<u8> {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, text) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn import_selects_one_skill_with_resources_and_rejects_traversal() {
        let root =
            std::env::temp_dir().join(format!("converge-catalogue-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let text = "---\nname: chosen\ndescription: Selected skill\n---\nUse resource.txt";
        let data = archive(&[
            ("repo/skills/chosen/SKILL.md", text),
            ("repo/skills/chosen/resource.txt", "selected"),
            ("repo/skills/other/private.txt", "not selected"),
        ]);
        extract_skill(data, &root, "chosen").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("resource.txt")).unwrap(),
            "selected"
        );
        assert!(!root.join("private.txt").exists());
        let data = archive(&[
            ("repo/skills/chosen/SKILL.md", text),
            ("../outside.txt", "escape"),
        ]);
        assert!(extract_skill(data, &root, "chosen").is_err());
        assert!(root
            .canonicalize()
            .unwrap()
            .starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn github_source_cannot_inject_urls_or_paths() {
        assert!(source_parts("vercel-labs/agent-skills").is_ok());
        for source in [
            "../secrets",
            "https://evil.test/x",
            "owner/repo/extra",
            "owner/../../escape",
        ] {
            assert!(source_parts(source).is_err());
        }
    }
}
