use std::path::PathBuf;
pub fn executable_paths() -> Vec<PathBuf> {
    #[allow(unused_mut)]
    let mut paths =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    #[cfg(unix)]
    {
        paths
            .extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"].map(PathBuf::from));
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            paths.extend([home.join(".local/bin"), home.join(".cargo/bin")]);
            if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
                paths.extend(entries.flatten().map(|p| p.path().join("bin")));
            }
        }
    }
    let mut unique = Vec::new();
    for path in paths.into_iter().take(128) {
        if let Ok(path) = path.canonicalize() {
            if path.is_dir() && !unique.contains(&path) {
                unique.push(path);
            }
        }
    }
    unique
}
#[cfg(unix)]
pub fn executable(name: &str) -> Option<PathBuf> {
    executable_paths()
        .into_iter()
        .map(|p| p.join(name))
        .find(|p| p.is_file())
}
pub fn path_value() -> std::ffi::OsString {
    // cmd/npm wrappers do not consistently understand Win32 extended paths.
    let paths = executable_paths().into_iter().map(|path| {
        #[cfg(windows)]
        {
            let text = path.to_string_lossy();
            if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
                return PathBuf::from(format!(r"\\{rest}"));
            }
            if let Some(rest) = text.strip_prefix(r"\\?\") {
                return PathBuf::from(rest);
            }
        }
        path
    });
    std::env::join_paths(paths).unwrap_or_default()
}
#[cfg(windows)]
pub fn powershell() -> Option<PathBuf> {
    static PROGRAM: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    PROGRAM
        .get_or_init(|| {
            use std::os::windows::process::CommandExt;
            let direct = std::env::var_os("ProgramFiles")
                .map(PathBuf::from)?
                .join("PowerShell/7/pwsh.exe");
            if direct.is_file() {
                return Some(direct);
            }
            // Resolve Microsoft Store execution aliases once, outside the sandbox.
            // The actual shell and user command still execute inside AppContainer.
            let executable = executable_paths()
                .into_iter()
                .map(|p| p.join("pwsh.exe"))
                .find(|p| p.is_file())?;
            let output = std::process::Command::new(executable)
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "[Console]::Write($PSHOME)",
                ])
                .creation_flags(0x08000000)
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let home = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
            let path = home.join("pwsh.exe");
            if !path.is_file() {
                return None;
            }
            let store = std::env::var_os("ProgramFiles")
                .map(PathBuf::from)?
                .join("WindowsApps");
            if home.starts_with(&store) {
                // Store apphosts cannot be started with a different AppContainer
                // identity. Cache the installed, immutable runtime (including its
                // licences), rather than running user commands on the host.
                let package = home.file_name()?.to_str()?;
                if !package.starts_with("Microsoft.PowerShell_")
                    || !package
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                {
                    return None;
                }
                let cache = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
                    .join("app.converge.desktop/runtimes");
                let target = cache.join(package);
                if !target.join(".complete").is_file() {
                    let pending = cache.join(format!("prepare-{}", uuid::Uuid::new_v4()));
                    std::fs::create_dir_all(&pending).ok()?;
                    copy_runtime(&home, &pending, &mut 0, &mut 0).ok()?;
                    std::fs::write(pending.join(".complete"), b"installed runtime").ok()?;
                    std::fs::rename(pending, &target).ok()?;
                }
                return Some(target.join("pwsh.exe"));
            }
            Some(path)
        })
        .clone()
}
#[cfg(windows)]
fn copy_runtime(
    source: &std::path::Path,
    target: &std::path::Path,
    bytes: &mut u64,
    count: &mut usize,
) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        let destination = target.join(entry.file_name());
        if kind.is_dir() {
            copy_runtime(&entry.path(), &destination, bytes, count)?;
        } else if kind.is_file() {
            *count += 1;
            *bytes += entry.metadata()?.len();
            if *bytes > 512 * 1024 * 1024 || *count > 20000 {
                return Err(std::io::Error::other("Runtime trop volumineux"));
            }
            std::fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}
pub fn terminal_text(bytes: &[u8]) -> String {
    let text = if bytes.starts_with(&[255, 254]) {
        String::from_utf16_lossy(
            &bytes[2..]
                .chunks_exact(2)
                .map(|p| u16::from_le_bytes([p[0], p[1]]))
                .collect::<Vec<_>>(),
        )
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    decode_clixml(&text, 0)
}
fn decode_clixml(text: &str, depth: usize) -> String {
    if depth >= 32 {
        return text.to_owned();
    }
    if let Some((before, xml)) = text.split_once("#< CLIXML") {
        // PowerShell stdout and stderr share a pipe. Plain stdout may follow
        // the closing XML object; do not discard it when decoding stderr.
        let (xml, after) = xml
            .split_once("</Objs>")
            .map(|(xml, after)| (xml.to_owned() + "</Objs>", after))
            .unwrap_or_else(|| (xml.to_string(), ""));
        let document = scraper::Html::parse_fragment(&xml);
        let selector = scraper::Selector::parse("s").unwrap();
        let messages = document
            .select(&selector)
            .filter(|n| {
                n.value().attr("s").is_none_or(|s| {
                    ["error", "warning", "output"]
                        .iter()
                        .any(|kind| s.eq_ignore_ascii_case(kind))
                })
            })
            .map(|n| {
                n.text()
                    .collect::<String>()
                    .replace("_x000D_", "")
                    .replace("_x000A_", "\n")
                    .replace("_x005F_", "_")
            })
            .collect::<Vec<_>>();
        if !messages.is_empty() {
            return format!(
                "{before}{}\n{}",
                messages.join("\n"),
                decode_clixml(after, depth + 1)
            );
        }
    }
    text.to_owned()
}
#[cfg(test)]
mod tests {
    #[test]
    fn powershell_xml_is_readable_without_losing_errors() {
        let text=super::terminal_text(b"#< CLIXML\n<Objs><S S=\"Error\">node : missing_x000D__x000A_next &amp; detail</S></Objs>");
        assert!(text.contains("node : missing\nnext & detail"));
        assert!(!text.contains("<Objs"));
        let text = super::terminal_text(
            b"before\n#< CLIXML\n<Objs><S S=\"Error\">warning</S></Objs>COMMAND_RAN\n",
        );
        assert!(text.starts_with("before"));
        assert!(text.contains("warning"));
        assert!(text.contains("COMMAND_RAN"));
    }
}
