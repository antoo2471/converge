use serde_json::{json, Value};
use std::{io::Write, path::Path};
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

pub fn write(
    root: &Path,
    relative: &str,
    text: &str,
    channel: Option<&Channel<Value>>,
    cancel: Option<&CancellationToken>,
) -> Result<String, String> {
    let target = crate::tools::scoped(root, relative, true)?;
    let old = if target.exists() {
        if std::fs::metadata(&target).map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 {
            return Err("Fichier trop volumineux pour une modification sûre".into());
        }
        std::fs::read_to_string(&target).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let diff = similar::TextDiff::configure()
        .timeout(std::time::Duration::from_millis(300))
        .diff_lines(old.as_str(), text);
    let added = diff
        .iter_all_changes()
        .filter(|c| c.tag() == similar::ChangeTag::Insert)
        .count();
    let removed = diff
        .iter_all_changes()
        .filter(|c| c.tag() == similar::ChangeTag::Delete)
        .count();
    if let Some(channel) = channel {
        channel.send(json!({"type":"file_changes","added":added,"removed":removed,"diff":diff.unified_diff().context_radius(3).to_string().chars().take(80000).collect::<String>(),"path":relative})).map_err(|e|e.to_string())?;
    }
    let parent = target.parent().ok_or("Dossier parent manquant")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    if crate::tools::scoped(root, relative, true)? != target {
        return Err("Le chemin a changé pendant l’écriture".into());
    }
    let temporary = parent.join(format!(".converge-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let mut written = 0;
        for bytes in text.as_bytes().chunks(32768) {
            if cancel.is_some_and(|c| c.is_cancelled()) {
                return Err("CANCELLED: Écriture annulée, fichier original conservé".into());
            }
            file.write_all(bytes).map_err(|e| e.to_string())?;
            written += bytes.len();
            if let Some(channel) = channel {
                let _ = channel
                    .send(json!({"type":"file_progress","written":written,"total":text.len()}));
            }
        }
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        if cancel.is_some_and(|c| c.is_cancelled()) {
            return Err("CANCELLED: Écriture annulée, fichier original conservé".into());
        }
        if crate::tools::scoped(root, relative, true)? != target {
            return Err("Le chemin a changé pendant l’écriture".into());
        }
        std::fs::rename(&temporary, &target).map_err(|e| e.to_string())?;
        Ok(format!(
            "Fichier enregistré : {} (+{added} -{removed})",
            crate::workspaces::display(&target)
        ))
    })();
    if temporary.exists() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
#[path = "file_changes_tests.rs"]
mod tests;
