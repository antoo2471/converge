use super::*;
use std::{
    fs,
    io::Read,
    path::{Component, Path},
};
const MAX_BYTES: u64 = 64 * 1024 * 1024;
pub(super) fn skill_meta(text: &str) -> Result<(String, String), String> {
    if text.len() > 256 * 1024 {
        return Err("SKILL.md dépasse 256 Ko".into());
    }
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let front = text
        .strip_prefix("---\n")
        .and_then(|s| s.split_once("\n---").map(|v| v.0))
        .ok_or("Ajoutez un en-tête YAML entre --- avec name et description.")?;
    let data: Value = serde_yaml::from_str(front).map_err(|_| "En-tête YAML du skill invalide")?;
    let name = data["name"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 120)
        .ok_or("Le skill doit posséder un nom (120 caractères maximum).")?;
    let description = data["description"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 4000)
        .ok_or("Le skill doit posséder une description (4 000 caractères maximum).")?;
    Ok((name.trim().into(), description.trim().into()))
}
fn walk(
    source: &Path,
    target: &Path,
    root: &Path,
    total: &mut u64,
    count: &mut usize,
) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|_| "Création du dossier du skill impossible")?;
    for entry in fs::read_dir(source).map_err(|_| "Dossier du skill illisible")? {
        let entry = entry.map_err(|_| "Fichier du skill illisible")?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|_| "Fichier du skill illisible")?;
        let canonical = path
            .canonicalize()
            .map_err(|_| "Chemin du skill invalide")?;
        if meta.file_type().is_symlink() || !canonical.starts_with(root) {
            return Err("Les liens et les fichiers extérieurs au skill sont refusés.".into());
        }
        *count += 1;
        if *count > 2000 {
            return Err("Le skill dépasse 2 000 fichiers.".into());
        }
        let destination = target.join(entry.file_name());
        if meta.is_dir() {
            walk(&path, &destination, root, total, count)?
        } else if meta.is_file() {
            *total += meta.len();
            if *total > MAX_BYTES {
                return Err("Le skill dépasse 64 Mo.".into());
            }
            fs::copy(path, destination).map_err(|_| "Copie du skill impossible")?;
        }
    }
    Ok(())
}
fn archive(source: &Path, target: &Path) -> Result<(), String> {
    let file = fs::File::open(source).map_err(|_| "Archive illisible")?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "Archive ZIP invalide")?;
    if zip.len() > 2000 {
        return Err("L’archive dépasse 2 000 fichiers.".into());
    }
    let mut total = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|_| "Entrée ZIP invalide")?;
        let path = entry
            .enclosed_name()
            .ok_or("L’archive contient un chemin extérieur")?;
        if entry.unix_mode().is_some_and(|v| v & 0o170000 == 0o120000) {
            return Err("L’archive contient un lien symbolique".into());
        }
        let out = target.join(path);
        if entry.is_dir() {
            fs::create_dir_all(out).map_err(|_| "Extraction impossible")?;
            continue;
        }
        total += entry.size();
        if total > MAX_BYTES {
            return Err("L’archive décompressée dépasse 64 Mo".into());
        }
        fs::create_dir_all(out.parent().ok_or("Chemin ZIP invalide")?)
            .map_err(|_| "Extraction impossible")?;
        let mut output = fs::File::create(out).map_err(|_| "Extraction impossible")?;
        let copied = std::io::copy(&mut entry.by_ref().take(MAX_BYTES + 1), &mut output)
            .map_err(|_| "Extraction impossible")?;
        if copied > MAX_BYTES {
            return Err("Fichier ZIP trop volumineux".into());
        }
    }
    Ok(())
}
fn find_skill(root: &Path, depth: usize) -> Result<PathBuf, String> {
    if root.join("SKILL.md").is_file() {
        return Ok(root.to_path_buf());
    }
    if depth > 4 {
        return Err("Aucun SKILL.md dans le dossier".into());
    }
    let mut found = vec![];
    for entry in fs::read_dir(root)
        .map_err(|_| "Dossier illisible")?
        .flatten()
    {
        if entry.path().is_dir() {
            if let Ok(p) = find_skill(&entry.path(), depth + 1) {
                found.push(p)
            }
        }
    }
    if found.len() == 1 {
        Ok(found.remove(0))
    } else {
        Err("Sélectionnez un dossier contenant un seul SKILL.md.".into())
    }
}
fn owned(root: &Path, id: &str) -> Result<PathBuf, String> {
    uuid::Uuid::parse_str(id).map_err(|_| "Identifiant du skill invalide")?;
    Ok(root.join("skills").join(id))
}
fn remove_owned(root: &Path, id: &str) -> Result<(), String> {
    let path = owned(root, id)?;
    if path.exists() {
        let canonical = path
            .canonicalize()
            .map_err(|_| "Dossier du skill inaccessible")?;
        let base = root
            .join("skills")
            .canonicalize()
            .map_err(|_| "Bibliothèque inaccessible")?;
        if canonical == base || !canonical.starts_with(&base) {
            return Err("Dossier du skill refusé".into());
        }
        fs::remove_dir_all(&canonical).map_err(|_| "Impossible de supprimer le skill")?;
    }
    Ok(())
}
pub fn action(
    root: &Path,
    state: &AppState,
    action: &str,
    id: Option<&str>,
    source: Option<&str>,
    content: Option<&str>,
    enabled: Option<bool>,
) -> Result<Value, String> {
    if action == "view" {
        return serde_json::from_str(&read_file(
            root,
            state,
            &json!({"skillId":id.ok_or("Skill manquant")?,"path":"SKILL.md"}),
            false,
        )?)
        .map_err(|_| "Skill invalide".into());
    }
    if action == "toggle" {
        let id = id.ok_or("Skill manquant")?;
        let mut record = super::record(state, "skill", id)?;
        record["enabled"] = json!(enabled.ok_or("État manquant")?);
        super::put(state, "skill", id, &record)?;
        return Ok(record);
    }
    if action == "remove" {
        let id = id.ok_or("Skill manquant")?;
        super::record(state, "skill", id)?;
        remove_owned(root, id)?;
        super::delete(state, "skill", id)?;
        return Ok(Value::Null);
    }
    if action != "import" && action != "create" {
        return Err("Action de skill inconnue".into());
    }
    if let Some(content) = content {
        skill_meta(content)?;
    }
    if let Some(source) = source {
        if source.len() > 4000 || source.contains('\0') {
            return Err("Chemin du skill invalide".into());
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let destination = owned(root, &id)?;
    fs::create_dir_all(&destination).map_err(|_| "Bibliothèque de skills indisponible")?;
    let result = (|| {
        let path = if action == "create" {
            fs::write(
                destination.join("SKILL.md"),
                content.ok_or("Contenu du skill manquant")?,
            )
            .map_err(|_| "Enregistrement du skill impossible")?;
            destination.clone()
        } else {
            let source = Path::new(source.ok_or("Choisissez un skill à importer")?);
            if source.is_dir() {
                let source = source.canonicalize().map_err(|_| "Dossier introuvable")?;
                if destination.starts_with(&source) {
                    return Err("Sélectionnez le dossier du skill, pas son dossier parent.".into());
                }
                walk(&source, &destination, &source, &mut 0, &mut 0)?;
            } else if source.file_name().is_some_and(|s| s == "SKILL.md") {
                if fs::metadata(source).map_err(|_| "Skill illisible")?.len() > 256 * 1024 {
                    return Err("SKILL.md dépasse 256 Ko".into());
                }
                fs::copy(source, destination.join("SKILL.md")).map_err(|_| "Copie impossible")?;
            } else {
                archive(source, &destination)?;
            }
            find_skill(&destination, 0)?
        };
        let text = fs::read_to_string(path.join("SKILL.md"))
            .map_err(|_| "SKILL.md doit être un texte UTF-8")?;
        let (name, description) = skill_meta(&text)?;
        let relative = path
            .strip_prefix(&destination)
            .map_err(|_| "Chemin du skill refusé")?
            .to_string_lossy()
            .to_string();
        let record = json!({"id":id,"name":name,"description":description,"enabled":true,"subdirectory":relative});
        super::put(state, "skill", &id, &record)?;
        Ok(record)
    })();
    if result.is_err() {
        let _ = remove_owned(root, &id);
    }
    result
}
pub fn read(root: &Path, state: &AppState, args: &Value) -> Result<String, String> {
    read_file(root, state, args, true)
}
fn read_file(
    root: &Path,
    state: &AppState,
    args: &Value,
    require_enabled: bool,
) -> Result<String, String> {
    let id = args["skillId"].as_str().ok_or("Skill manquant")?;
    let meta = super::record(state, "skill", id)?;
    if require_enabled && meta["enabled"] != true {
        return Err("Ce skill est désactivé".into());
    }
    let base = owned(root, id)?
        .join(meta["subdirectory"].as_str().unwrap_or(""))
        .canonicalize()
        .map_err(|_| "Skill introuvable")?;
    let path = args["path"].as_str().unwrap_or("SKILL.md");
    if path.len() > 500
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err("Le chemin doit rester dans le skill".into());
    }
    let path = base
        .join(path)
        .canonicalize()
        .map_err(|_| "Ressource du skill introuvable")?;
    if !path.starts_with(&base) {
        return Err("Ressource extérieure au skill refusée".into());
    }
    if fs::metadata(&path)
        .map_err(|_| "Ressource illisible")?
        .len()
        > 256 * 1024
    {
        return Err("Ressource trop volumineuse (256 Ko maximum)".into());
    }
    let text = fs::read_to_string(path).map_err(|_| "La ressource n’est pas un texte UTF-8")?;
    Ok(json!({"skill":meta["name"],"content":text}).to_string())
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;
