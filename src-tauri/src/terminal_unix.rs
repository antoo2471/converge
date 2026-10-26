use serde_json::{json, Value};
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

#[cfg(target_os = "macos")]
fn quoted(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}
pub(super) fn run(
    root: &Path,
    command: &str,
    timeout: u64,
    cancel: CancellationToken,
    channel: Option<Channel<Value>>,
    read_only: bool,
    shell: &str,
) -> Result<String, String> {
    use std::os::unix::process::CommandExt;
    let interpreter = match shell {
        "auto" | "sh" => "/bin/sh",
        "bash" => "/bin/bash",
        "zsh" => "/bin/zsh",
        _ => return Err("Cet OS utilise sh, bash ou zsh, pas cmd/PowerShell".into()),
    };
    let temp = std::env::temp_dir().join(format!("converge-terminal-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp).map_err(|_| "Cache du terminal indisponible")?;
    let temp = temp
        .canonicalize()
        .map_err(|_| "Cache du terminal indisponible")?;
    let root_path = root
        .canonicalize()
        .map_err(|_| "Espace de travail indisponible")?;
    let root = root_path.as_path();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(temp.clone());
    let paths = crate::host_environment::executable_paths();
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let mut runtime_dirs = paths
        .iter()
        .filter(|p| {
            p.parent().is_some()
                && !home.as_ref().is_some_and(|h| h == *p || h.starts_with(p))
                && !p.starts_with(root)
                && [
                    "node", "npm", "git", "python3", "cargo", "rustc", "rg", "dotnet",
                ]
                .iter()
                .any(|name| p.join(name).is_file())
        })
        .cloned()
        .collect::<Vec<_>>();
    for directory in runtime_dirs.clone() {
        for name in ["node", "npm", "python3", "cargo", "rustc", "rg", "dotnet"] {
            if let Ok(binary) = directory.join(name).canonicalize() {
                if let Some(parent) = binary.parent() {
                    runtime_dirs.push(parent.to_path_buf());
                    if parent.file_name().is_some_and(|n| n == "bin") {
                        if let Some(prefix) = parent.parent() {
                            for suffix in ["lib", "libexec"] {
                                let path = prefix.join(suffix);
                                if path.is_dir() {
                                    runtime_dirs.push(path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    for prefix in ["/opt/homebrew", "/usr/local"] {
        for suffix in ["Cellar", "lib"] {
            let path = std::path::PathBuf::from(prefix).join(suffix);
            if path.is_dir() {
                runtime_dirs.push(path);
            }
        }
    }
    runtime_dirs.sort();
    runtime_dirs.dedup();
    #[cfg(target_os = "linux")]
    let (mut cmd, isolation) = {
        let binary = crate::host_environment::executable("bwrap").ok_or("Le terminal isolé nécessite bubblewrap. Installez le paquet bubblewrap de votre distribution.")?;
        let mut cmd = Command::new(binary);
        cmd.args([
            "--die-with-parent",
            "--new-session",
            "--unshare-all",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
        ]);
        for path in [
            "/usr",
            "/bin",
            "/sbin",
            "/lib",
            "/lib64",
            "/etc/ld.so.cache",
            "/etc/alternatives",
        ] {
            if Path::new(path).exists() {
                cmd.args(["--ro-bind", path, path]);
            }
        }
        for path in &runtime_dirs {
            if !path.starts_with("/usr") && !path.starts_with("/bin") {
                cmd.arg("--ro-bind").arg(path).arg(path);
            }
        }
        cmd.arg(if read_only { "--ro-bind" } else { "--bind" })
            .arg(root)
            .arg(root);
        cmd.arg("--bind")
            .arg(&temp)
            .arg(&temp)
            .arg("--chdir")
            .arg(root)
            .arg("--")
            .arg(interpreter)
            .arg("-c")
            .arg(command);
        (cmd, "bubblewrap")
    };
    #[cfg(target_os = "macos")]
    let (mut cmd, isolation) = {
        if !Path::new("/usr/bin/sandbox-exec").is_file() {
            return Err(
                "Le mécanisme d’isolation sandbox-exec est absent de cette installation macOS"
                    .into(),
            );
        }
        let mut policy = String::from("(version 1)(deny default)(allow process-exec)(allow process-fork)(allow sysctl-read)(allow file-read-metadata)(allow signal (target same-sandbox))(allow file-read* (subpath \"/System\") (subpath \"/usr\") (subpath \"/bin\") (subpath \"/sbin\") (subpath \"/Library/Apple\") (literal \"/dev/null\") (literal \"/dev/random\") (literal \"/dev/urandom\"))");
        for path in &runtime_dirs {
            policy.push_str(&format!(
                "(allow file-read* (subpath \"{}\"))",
                quoted(path)
            ));
        }
        policy.push_str(&format!(
            "(allow file-read* (subpath \"{}\"))(allow file-read* file-write* (subpath \"{}\"))",
            quoted(root),
            quoted(&temp)
        ));
        if !read_only {
            policy.push_str(&format!(
                "(allow file-write* (subpath \"{}\"))",
                quoted(root)
            ));
        }
        let mut cmd = Command::new("/usr/bin/sandbox-exec");
        cmd.arg("-p")
            .arg(policy)
            .arg(interpreter)
            .arg("-c")
            .arg(command);
        (cmd, "macOS Seatbelt")
    };
    cmd.current_dir(root)
        .env_clear()
        .env("PATH", crate::host_environment::path_value())
        .env("HOME", &temp)
        .env("TMPDIR", &temp)
        .env("TMP", &temp)
        .env("TEMP", &temp)
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = cmd
        .spawn()
        .map_err(|_| "Impossible de lancer le terminal isolé")?;
    let pid = child.id();
    let (sender, receiver) = mpsc::sync_channel(16);
    let mut readers = vec![];
    for stream in [
        Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
        Box::new(child.stderr.take().unwrap()) as Box<dyn Read + Send>,
    ] {
        let sender = sender.clone();
        readers.push(std::thread::spawn(move || {
            let mut stream = stream;
            let mut buffer = [0u8; 8192];
            while let Ok(n) = stream.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                if sender.send(buffer[..n].to_vec()).is_err() {
                    break;
                }
            }
        }));
    }
    drop(sender);
    let started = Instant::now();
    let mut output = Vec::new();
    let mut status = None;
    loop {
        if let Ok(bytes) = receiver.recv_timeout(Duration::from_millis(20)) {
            if let Some(channel) = &channel {
                let _ =
                    channel.send(json!({"type":"output","text":String::from_utf8_lossy(&bytes)}));
            }
            output.extend_from_slice(&bytes);
        }
        if cancel.is_cancelled()
            || started.elapsed().as_secs() >= timeout
            || output.len() > 2 * 1024 * 1024
        {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(if cancel.is_cancelled() {
                "CANCELLED: Commande annulée"
            } else {
                "Commande arrêtée : délai ou limite de sortie"
            }
            .into());
        }
        if let Ok(Some(exit)) = child.try_wait() {
            status = Some(exit);
            break;
        }
    }
    // Close any surviving children before joining readers, including background jobs.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    for bytes in receiver.iter() {
        if output.len() + bytes.len() > 2 * 1024 * 1024 {
            return Err("Sortie du terminal trop volumineuse".into());
        }
        output.extend_from_slice(&bytes);
    }
    for reader in readers {
        let _ = reader.join();
    }
    let status = status.unwrap();
    let code = status.code().unwrap_or(-1);
    if let Some(channel) = &channel {
        let _ = channel.send(json!({"type":"exit","code":code}));
    }
    let text = format!(
        "{}\n[Exit: {code} · {interpreter} · {} ms · {isolation} · réseau désactivé · {}]",
        crate::host_environment::terminal_text(&output),
        started.elapsed().as_millis(),
        if read_only {
            "lecture seule"
        } else {
            "copie isolée"
        }
    );
    if status.success() {
        Ok(text)
    } else {
        Err(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_sandbox_executes_node_preserves_workspace_and_denies_outside_reads() {
        let base =
            std::env::temp_dir().join(format!("converge-unix-test-{}", uuid::Uuid::new_v4()));
        let root = base.join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(base.join("private.txt"), "PRIVATE_SENTINEL").unwrap();
        std::fs::write(root.join("visible.txt"), "VISIBLE").unwrap();
        let output=run(&root,"node --version; cat visible.txt; if cat ../private.txt 2>/dev/null; then exit 2; fi; if echo BAD > blocked.txt 2>/dev/null; then exit 3; fi; echo ISOLATION_OK",30,CancellationToken::new(),None,true,"sh").unwrap();
        assert!(output.contains("ISOLATION_OK"), "{output}");
        assert!(output.contains("VISIBLE"));
        assert!(!output.contains("PRIVATE_SENTINEL"));
        assert!(!root.join("blocked.txt").exists());
        let output = run(
            &root,
            "node -e 'require(\"fs\").writeFileSync(\"written.txt\",\"OK\")'",
            30,
            CancellationToken::new(),
            None,
            false,
            "sh",
        )
        .unwrap();
        assert!(output.contains("Exit: 0"));
        assert_eq!(
            std::fs::read_to_string(root.join("written.txt")).unwrap(),
            "OK"
        );
        assert!(base
            .canonicalize()
            .unwrap()
            .starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(base).unwrap();
    }
}
