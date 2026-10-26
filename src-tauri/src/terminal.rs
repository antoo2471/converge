use crate::AppState;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

pub async fn execute(
    state: &AppState,
    app: &tauri::AppHandle,
    args: &Value,
    channel: Option<Channel<Value>>,
    approved_root: Option<PathBuf>,
    permission_context: Option<crate::permissions::Context>,
) -> Result<String, String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
    let _guard = state.terminal_gate.lock().await;
    let workspace_root =
        approved_root.ok_or("Choisissez un dossier de travail avant d’utiliser le terminal")?;
    let root = if let Some(cwd) = args["cwd"].as_str() {
        let target = crate::tools::scoped(&workspace_root, cwd, true)?;
        std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        crate::tools::scoped(&workspace_root, cwd, false)?
    } else {
        workspace_root
    };
    let command = args["command"]
        .as_str()
        .ok_or("Argument command manquant")?
        .to_owned();
    if command.len() > 32000 || command.contains('\0') {
        return Err("Commande invalide".into());
    }
    let timeout = args["timeoutSeconds"].as_u64().unwrap_or(120).clamp(1, 600);
    let apply = args["applyChanges"] == true;
    let read_only = args["readOnly"].as_bool().unwrap_or(!apply);
    if read_only && apply {
        return Err("readOnly and applyChanges are incompatible".into());
    }
    let shell = select_shell(&command, args["shell"].as_str())?.to_owned();
    let request_id = args["requestId"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let token = CancellationToken::new();
    {
        let mut map = state.requests.lock().map_err(|e| e.to_string())?;
        if map.contains_key(&request_id) {
            return Err("Identifiant d’exécution déjà utilisé".into());
        }
        map.insert(request_id.clone(), token.clone());
    }
    let sandbox = std::env::temp_dir().join(format!("converge-sandbox-{}", uuid::Uuid::new_v4()));
    let sandbox_copy = sandbox.clone();
    let root_copy = root.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let setup=std::time::Instant::now();
        if let Some(c)=&channel { let _=c.send(json!({"type":"phase","text":if read_only {"Préparation du terminal"} else {"Préparation de la copie isolée"}})); }

        let execution_root = if read_only {
            root_copy
        } else {
            let mut size = 0u64;
            let mut count = 0usize;
            copy_snapshot(&root_copy, &sandbox_copy, &mut size, &mut count)?;
            sandbox_copy
        };
        if token.is_cancelled() {
            return Err("CANCELLED: Action annulée".into());
        }
        if let Some(c)=&channel {let _=c.send(json!({"type":"phase","text":"Démarrage de la commande","setupMs":setup.elapsed().as_millis()}));}
        isolated_with_options(
            &execution_root,
            &command,
            timeout,
            token,
            channel,
            read_only,
            &shell,
        )
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|v| v);
    state
        .requests
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&request_id);
    let mut result = outcome;
    if result.is_ok() && apply {
        let changes = changed_files(&sandbox, &root);
        match changes {
            Ok(paths) if !paths.is_empty() => {
                let listing = paths
                    .iter()
                    .take(100)
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                let yes = if let Some(context) = &permission_context {
                    crate::permissions::authorize(state, context, "terminal.apply_changes", &json!({"files": paths.iter().map(|p|p.display().to_string()).collect::<Vec<_>>(), "root": root.display().to_string()})).await.is_ok()
                } else {
                    app.dialog().message(format!("Le terminal a modifié {} fichier(s) dans sa copie isolée.\n\n{}\n\nAppliquer ces changements au dossier original ?",paths.len(),listing)).title("Converge · Appliquer les modifications").buttons(MessageDialogButtons::OkCancel).blocking_show()
                };
                if yes {
                    for relative in paths {
                        let target =
                            crate::tools::scoped(&root, &relative.to_string_lossy(), true)?;
                        let source = sandbox.join(&relative);
                        if !source.is_file() {
                            continue;
                        }
                        let bytes = std::fs::read(source).map_err(|e| e.to_string())?;
                        std::fs::write(target, bytes).map_err(|e| e.to_string())?;
                    }
                    if let Ok(ref mut output) = result {
                        output.push_str("\nModifications appliquées au dossier original. Les suppressions ne sont pas synchronisées.")
                    }
                } else if let Ok(ref mut output) = result {
                    output.push_str(
                        "\nApplication des modifications refusée ; dossier original conservé.",
                    )
                }
            }
            Err(e) => result = Err(e),
            _ => {}
        }
    }
    // The path is generated here and canonicalized before recursive removal.
    if let (Ok(target), Ok(temp)) = (sandbox.canonicalize(), std::env::temp_dir().canonicalize()) {
        if target.starts_with(&temp)
            && target
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("converge-sandbox-"))
        {
            let _ = std::fs::remove_dir_all(target);
        }
    }
    result
}
fn select_shell<'a>(command: &str, requested: Option<&'a str>) -> Result<&'a str, String> {
    #[cfg(unix)]
    return match requested.unwrap_or("auto") {
        "auto" | "sh" => Ok("auto"),
        "bash" => Ok("bash"),
        "zsh" => Ok("zsh"),
        _ => Err("Interpréteur Unix attendu : auto, sh, bash ou zsh".into()),
    };
    #[cfg(windows)]
    match requested.unwrap_or("auto") {
        "cmd" => Ok("cmd"),
        "powershell" => Ok("powershell"),
        "auto" => {
            let executable = command
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            let safe = [
                "git", "node", "npm", "npx", "pnpm", "yarn", "python", "python3", "py", "cargo",
                "rustc", "dotnet", "rg", "whoami", "hostname",
            ];
            if safe.contains(&executable.trim_end_matches(".exe").trim_end_matches(".cmd"))
                && !command.contains(['$', '`', ';', '@', '\'', '|', '\n', '\r'])
            {
                Ok("cmd")
            } else {
                Ok("powershell")
            }
        }
        _ => Err("Interpréteur inconnu : auto, powershell ou cmd".into()),
    }
}
fn excluded(name: &str) -> bool {
    [".git", "target", "dist", ".env", ".env.local"].contains(&name)
}
fn copy_snapshot(from: &Path, to: &Path, size: &mut u64, count: &mut usize) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
    {
        let name = entry.file_name();
        if excluded(&name.to_string_lossy()) {
            continue;
        }
        let meta = entry.file_type().map_err(|e| e.to_string())?;
        if meta.is_symlink() {
            continue;
        }
        let dest = to.join(name);
        if meta.is_dir() {
            copy_snapshot(&entry.path(), &dest, size, count)?
        } else if meta.is_file() {
            *size += entry.metadata().map_err(|e| e.to_string())?.len();
            *count += 1;
            if *size > 512 * 1024 * 1024 || *count > 40000 {
                return Err("Le dossier dépasse la limite de copie isolée (512 Mo / 40 000 fichiers). Sélectionnez un sous-dossier.".into());
            }
            std::fs::copy(entry.path(), dest).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn changed_files(sandbox: &Path, root: &Path) -> Result<Vec<PathBuf>, String> {
    fn visit(
        base: &Path,
        current: &Path,
        root: &Path,
        out: &mut Vec<PathBuf>,
    ) -> Result<(), String> {
        for e in std::fs::read_dir(current)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
        {
            let kind = e.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink()
                || excluded(&e.file_name().to_string_lossy())
                || e.file_name() == "node_modules"
            {
                continue;
            }
            if kind.is_dir() {
                visit(base, &e.path(), root, out)?
            } else if kind.is_file() {
                let path = e.path();
                let relative = path.strip_prefix(base).map_err(|e| e.to_string())?;
                if e.metadata().map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024 {
                    continue;
                }
                let new = std::fs::read(&path).map_err(|e| e.to_string())?;
                if std::fs::read(root.join(relative)).ok().as_ref() != Some(&new) {
                    out.push(relative.to_path_buf());
                    if out.len() > 100 {
                        return Err("Plus de 100 fichiers modifiés : utilisez des patchs ciblés pour appliquer les changements".into());
                    }
                }
            }
        }
        Ok(())
    }
    let mut out = vec![];
    visit(sandbox, sandbox, root, &mut out)?;
    Ok(out)
}
#[cfg(test)]
fn isolated(
    root: &Path,
    command: &str,
    timeout: u64,
    cancel: CancellationToken,
    channel: Option<Channel<Value>>,
) -> Result<String, String> {
    isolated_with_options(root, command, timeout, cancel, channel, false, "cmd")
}
#[cfg(windows)]
fn isolated_with_options(
    root: &Path,
    command: &str,
    timeout: u64,
    cancel: CancellationToken,
    channel: Option<Channel<Value>>,
    read_only: bool,
    shell: &str,
) -> Result<String, String> {
    use std::{
        mem::{size_of, size_of_val, zeroed},
        ptr::*,
    };
    use windows_sys::Win32::{
        Foundation::*,
        Security::{Authorization::*, Isolation::*, *},
        Storage::FileSystem::*,
        System::{JobObjects::*, Pipes::*, Threading::*},
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn os_error(context: &str) -> String {
        format!("{context} : {}", std::io::Error::last_os_error())
    }
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
                    CloseHandle(self.0);
                }
            }
        }
    }
    struct Profile {
        sid: PSID,
        name: Vec<u16>,
    }
    impl Drop for Profile {
        fn drop(&mut self) {
            unsafe {
                FreeSid(self.sid);
                DeleteAppContainerProfile(self.name.as_ptr());
            }
        }
    }
    // Revoke only our unique SID, preserving unrelated ACL entries. Windows propagates
    // removal to inherited child ACEs. The guard runs on every exit, before profile deletion.
    struct FolderAccess {
        path: Vec<u16>,
        sid: PSID,
        propagate: bool,
    }
    unsafe fn write_dacl(path: &[u16], acl: *mut ACL, propagate: bool) -> u32 {
        if propagate {
            SetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                acl,
                null(),
            )
        } else {
            // MAXIMUM_ALLOWED explicitly disables SetSecurityInfo propagation.
            // Ancestor metadata access must never walk the disk's descendants.
            let handle = Handle(CreateFileW(
                path.as_ptr(),
                0x02000000,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                null_mut(),
            ));
            if handle.0 == INVALID_HANDLE_VALUE {
                return 5;
            }
            SetSecurityInfo(
                handle.0,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                acl,
                null(),
            )
        }
    }
    impl Drop for FolderAccess {
        fn drop(&mut self) {
            unsafe {
                let (mut current, mut descriptor) = (null_mut(), null_mut());
                if GetNamedSecurityInfoW(
                    self.path.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    &mut current,
                    null_mut(),
                    &mut descriptor,
                ) == 0
                {
                    let mut entry: EXPLICIT_ACCESS_W = zeroed();
                    entry.grfAccessMode = REVOKE_ACCESS;
                    entry.Trustee.TrusteeForm = TRUSTEE_IS_SID;
                    entry.Trustee.TrusteeType = TRUSTEE_IS_UNKNOWN;
                    entry.Trustee.ptstrName = self.sid as _;
                    let mut updated = null_mut();
                    if SetEntriesInAclW(1, &entry, current, &mut updated) == 0 {
                        write_dacl(&self.path, updated, self.propagate);
                        LocalFree(updated as _);
                    }
                    LocalFree(descriptor);
                }
            }
        }
    }
    struct Attributes {
        data: Vec<usize>,
    }
    unsafe fn grant_access(
        path: &Path,
        sid: PSID,
        permissions: u32,
        inherit: u32,
    ) -> Result<FolderAccess, String> {
        let path = wide(&path.to_string_lossy());
        let (mut current, mut descriptor) = (null_mut(), null_mut());
        let code = GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut current,
            null_mut(),
            &mut descriptor,
        );
        if code != 0 {
            return Err("Lecture des permissions du runtime impossible".into());
        }
        let mut entry: EXPLICIT_ACCESS_W = zeroed();
        entry.grfAccessMode = GRANT_ACCESS;
        entry.grfAccessPermissions = permissions;
        entry.grfInheritance = inherit;
        entry.Trustee.TrusteeForm = TRUSTEE_IS_SID;
        entry.Trustee.TrusteeType = TRUSTEE_IS_UNKNOWN;
        entry.Trustee.ptstrName = sid as _;
        let mut updated = null_mut();
        let result = SetEntriesInAclW(1, &entry, current, &mut updated);
        let result = if result == 0 {
            write_dacl(&path, updated, inherit != 0)
        } else {
            result
        };
        if !updated.is_null() {
            LocalFree(updated as _);
        }
        LocalFree(descriptor);
        if result != 0 {
            return Err("Accès au runtime impossible".into());
        }
        Ok(FolderAccess {
            path,
            sid,
            propagate: inherit != 0,
        })
    }
    impl Drop for Attributes {
        fn drop(&mut self) {
            unsafe {
                DeleteProcThreadAttributeList(self.data.as_mut_ptr() as _);
            }
        }
    }
    unsafe {
        let name = wide(&format!("Converge.Terminal.{}", uuid::Uuid::new_v4()));
        let mut sid = null_mut();
        let hr = CreateAppContainerProfile(
            name.as_ptr(),
            name.as_ptr(),
            name.as_ptr(),
            null(),
            0,
            &mut sid,
        );
        if hr < 0 {
            return Err(format!(
                "Impossible de créer l’AppContainer Windows (0x{:08X})",
                hr as u32
            ));
        }
        let profile = Profile { sid, name };
        // The approved live folder is read-only; writable execution uses a disposable copy.
        let path = wide(&root.to_string_lossy());
        let mut old_acl = null_mut();
        let mut sd = null_mut();
        let code = GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut old_acl,
            null_mut(),
            &mut sd,
        );
        if code != 0 {
            return Err(format!("Lecture ACL impossible : {code}"));
        }
        let mut access: EXPLICIT_ACCESS_W = zeroed();
        access.grfAccessPermissions = if read_only {
            FILE_GENERIC_READ | FILE_GENERIC_EXECUTE
        } else {
            FILE_ALL_ACCESS
        };
        access.grfAccessMode = GRANT_ACCESS;
        access.grfInheritance = SUB_CONTAINERS_AND_OBJECTS_INHERIT;
        access.Trustee.TrusteeForm = TRUSTEE_IS_SID;
        access.Trustee.TrusteeType = TRUSTEE_IS_UNKNOWN;
        access.Trustee.ptstrName = profile.sid as _;
        let mut acl = null_mut();
        let code = SetEntriesInAclW(1, &access, old_acl, &mut acl);
        if code != 0 {
            LocalFree(sd);
            return Err(format!("Création ACL impossible : {code}"));
        }
        let code = SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            acl,
            null(),
        );
        LocalFree(acl as _);
        LocalFree(sd);
        if code != 0 {
            return Err(format!("Accès au dossier isolé refusé : {code}"));
        }
        let _folder_access = FolderAccess {
            path: path.clone(),
            sid: profile.sid,
            propagate: true,
        };
        // Expose installed executable directories, never the home or disk root.
        // These ACLs are read/execute-only and revoked with the unique sandbox SID.
        let home =
            std::env::var_os("USERPROFILE").and_then(|p| PathBuf::from(p).canonicalize().ok());
        let mut _runtime_access = Vec::new();
        // Runtime path resolution needs to stat/traverse ancestors. Grant no
        // directory listing or file reads there, and do not inherit this ACE.
        let mut ancestors = std::collections::HashSet::new();
        for directory in
            std::iter::once(root.to_path_buf()).chain(crate::host_environment::executable_paths())
        {
            for ancestor in directory.ancestors().skip(1) {
                if ancestors.insert(ancestor.to_path_buf()) {
                    if let Ok(access) = grant_access(
                        ancestor,
                        profile.sid,
                        FILE_READ_ATTRIBUTES | FILE_TRAVERSE,
                        0,
                    ) {
                        _runtime_access.push(access);
                    }
                }
            }
        }
        let requested = command
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_')
            .map(|v| v.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let public_roots = ["ProgramFiles", "ProgramFiles(x86)", "SystemRoot"]
            .iter()
            .filter_map(|key| {
                std::env::var_os(key).and_then(|p| PathBuf::from(p).canonicalize().ok())
            })
            .collect::<Vec<_>>();
        for directory in crate::host_environment::executable_paths() {
            if public_roots.iter().any(|root| directory.starts_with(root)) {
                continue;
            }
            if directory.parent().is_none()
                || home
                    .as_ref()
                    .is_some_and(|home| home == &directory || home.starts_with(&directory))
            {
                continue;
            }
            if [
                "node.exe",
                "npm.cmd",
                "git.exe",
                "python.exe",
                "cargo.exe",
                "rustc.exe",
                "rg.exe",
                "dotnet.exe",
                "pnpm.cmd",
                "yarn.cmd",
            ]
            .iter()
            .any(|name| {
                let stem = name.trim_end_matches(".exe").trim_end_matches(".cmd");
                let needed = requested.iter().any(|word| {
                    word == stem
                        || word == name
                        || (stem == "node"
                            && ["npm", "npx", "pnpm", "yarn"].contains(&word.as_str()))
                });
                needed && directory.join(name).is_file()
            }) {
                if let Ok(access) = grant_access(
                    &directory,
                    profile.sid,
                    FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
                    SUB_CONTAINERS_AND_OBJECTS_INHERIT,
                ) {
                    _runtime_access.push(access);
                }
            }
        }
        let mut security: SECURITY_ATTRIBUTES = zeroed();
        security.nLength = size_of::<SECURITY_ATTRIBUTES>() as u32;
        security.bInheritHandle = 1;
        let (mut reader, mut writer) = (null_mut(), null_mut());
        if CreatePipe(&mut reader, &mut writer, &security, 0) == 0 {
            return Err(os_error("Création du pipe"));
        }
        let reader = Handle(reader);
        let writer = Handle(writer);
        SetHandleInformation(reader.0, HANDLE_FLAG_INHERIT, 0);
        let nul = wide("NUL");
        let input = Handle(CreateFileW(
            nul.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &security,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        ));
        if input.0 == INVALID_HANDLE_VALUE {
            return Err(os_error("Création stdin"));
        }
        let mut bytes = 0usize;
        InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut bytes);
        let mut attributes = Attributes {
            data: vec![0usize; bytes.div_ceil(size_of::<usize>())],
        };
        let list = attributes.data.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        if InitializeProcThreadAttributeList(list, 2, 0, &mut bytes) == 0 {
            return Err(os_error("Attributs processus"));
        }
        let caps = SECURITY_CAPABILITIES {
            AppContainerSid: profile.sid,
            Capabilities: null_mut(),
            CapabilityCount: 0,
            Reserved: 0,
        };
        if UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            &caps as *const _ as _,
            size_of::<SECURITY_CAPABILITIES>(),
            null_mut(),
            null(),
        ) == 0
        {
            return Err(os_error("Isolation AppContainer"));
        }
        let handles = [writer.0, input.0];
        if UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            handles.as_ptr() as _,
            size_of_val(&handles),
            null_mut(),
            null(),
        ) == 0
        {
            return Err(os_error("Liste des handles"));
        }
        let mut startup: STARTUPINFOEXW = zeroed();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = list;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdOutput = writer.0;
        startup.StartupInfo.hStdError = writer.0;
        startup.StartupInfo.hStdInput = input.0;
        let system = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let (executable, arguments) = if shell == "powershell" {
            let program=crate::host_environment::powershell().ok_or("Le terminal PowerShell isolé nécessite PowerShell 7 récent (pwsh). Installez-le, puis relancez Converge. Les commandes cmd et Node restent disponibles.")?;
            if let Some(directory) = program.parent() {
                if !public_roots.iter().any(|root| directory.starts_with(root)) {
                    _runtime_access.push(grant_access(
                        directory,
                        profile.sid,
                        FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
                        SUB_CONTAINERS_AND_OBJECTS_INHERIT,
                    )?);
                }
            }
            use base64::Engine;
            let location = root.to_string_lossy();
            let location = location
                .strip_prefix(r"\\?\")
                .unwrap_or(&location)
                .replace('\'', "''");
            let script = format!("[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new(); $ProgressPreference = 'SilentlyContinue'; $ErrorActionPreference = 'Stop'; New-PSDrive -Name Converge -PSProvider FileSystem -Root '{location}' | Out-Null; Set-Location Converge:\\; {command}");
            let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            (
                program.to_string_lossy().into_owned(),
                format!(
                    "powershell.exe -NoLogo -NoProfile -NonInteractive -OutputFormat Text -EncodedCommand {encoded}"
                ),
            )
        } else {
            (
                format!("{system}\\System32\\cmd.exe"),
                format!("cmd.exe /d /s /c \"{command}\""),
            )
        };
        let exe = wide(&executable);
        let mut line = wide(&arguments);
        let mut environment = std::collections::BTreeMap::<String, String>::new();
        for key in [
            "ALLUSERSPROFILE",
            "APPDATA",
            "LOCALAPPDATA",
            "ProgramData",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "SystemDrive",
            "SystemRoot",
            "USERPROFILE",
            "WINDIR",
        ] {
            if let Ok(value) = std::env::var(key) {
                environment.insert(key.to_string(), value);
            }
        }
        environment.insert(
            "PATH".into(),
            crate::host_environment::path_value()
                .to_string_lossy()
                .into_owned(),
        );
        environment.insert("TEMP".into(), root.to_string_lossy().to_string());
        environment.insert("TMP".into(), root.to_string_lossy().to_string());
        environment.insert(
            "NODE_OPTIONS".into(),
            "--preserve-symlinks --preserve-symlinks-main".into(),
        );
        let mut env: Vec<u16> = environment
            .iter()
            .flat_map(|(k, v)| format!("{k}={v}\0").encode_utf16().collect::<Vec<_>>())
            .collect();
        env.push(0);
        let mut pi: PROCESS_INFORMATION = zeroed();
        if CreateProcessW(
            exe.as_ptr(),
            line.as_mut_ptr(),
            null(),
            null(),
            1,
            EXTENDED_STARTUPINFO_PRESENT
                | CREATE_SUSPENDED
                | CREATE_NO_WINDOW
                | CREATE_UNICODE_ENVIRONMENT,
            env.as_ptr() as _,
            path.as_ptr(),
            &startup.StartupInfo,
            &mut pi,
        ) == 0
        {
            return Err(os_error("Lancement du processus isolé"));
        }
        let process = Handle(pi.hProcess);
        let thread = Handle(pi.hThread);
        let job = Handle(CreateJobObjectW(null(), null()));
        if job.0.is_null() {
            TerminateProcess(process.0, 1);
            return Err(os_error("Création du Job Object"));
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limits.BasicLimitInformation.ActiveProcessLimit = 32;
        limits.ProcessMemoryLimit = 1024 * 1024 * 1024;
        if SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as _,
            size_of_val(&limits) as u32,
        ) == 0
            || AssignProcessToJobObject(job.0, process.0) == 0
        {
            TerminateProcess(process.0, 1);
            return Err(os_error("Application des limites processus"));
        }
        drop(writer);
        ResumeThread(thread.0);
        if let Some(c) = &channel {
            let _ = c.send(json!({"type":"phase","text":"Commande en cours"}));
        }
        let started = std::time::Instant::now();
        let mut output = Vec::new();
        let mut exit = STILL_ACTIVE as u32;
        loop {
            let mut available = 0u32;
            PeekNamedPipe(
                reader.0,
                null_mut(),
                0,
                null_mut(),
                &mut available,
                null_mut(),
            );
            if available > 0 {
                let mut buffer = vec![0u8; (available as usize).min(16384)];
                let mut read = 0;
                if ReadFile(
                    reader.0,
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    &mut read,
                    null_mut(),
                ) != 0
                {
                    buffer.truncate(read as usize);
                    output.extend_from_slice(&buffer);
                    if let Some(c) = &channel {
                        let _ = c
                            .send(json!({"type":"output","text":String::from_utf8_lossy(&buffer)}));
                    }
                }
            }
            GetExitCodeProcess(process.0, &mut exit);
            if exit != STILL_ACTIVE as u32 && available == 0 {
                break;
            }
            if cancel.is_cancelled()
                || started.elapsed().as_secs() >= timeout
                || output.len() > 8 * 1024 * 1024
            {
                TerminateJobObject(job.0, 1);
                output.extend_from_slice(b"\n[Arret : annulation, delai ou limite de sortie]");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        drop(job);
        let text = crate::host_environment::terminal_text(&output);
        if let Some(c) = &channel {
            let _ = c.send(json!({"type":"exit","code":exit,"cancelled":cancel.is_cancelled()}));
        }
        let result=format!("{text}\n[Exit: {exit} · {shell} · {} ms · AppContainer Windows · réseau désactivé · {}]",started.elapsed().as_millis(),if read_only {"lecture seule"} else {"copie isolée"});
        if cancel.is_cancelled() {
            Err(format!("CANCELLED: {result}"))
        } else if exit != 0 {
            Err(result)
        } else {
            Ok(result)
        }
    }
}
#[cfg(unix)]
#[path = "terminal_unix.rs"]
mod platform;
#[cfg(unix)]
fn isolated_with_options(
    root: &Path,
    command: &str,
    timeout: u64,
    cancel: CancellationToken,
    channel: Option<Channel<Value>>,
    read_only: bool,
    shell: &str,
) -> Result<String, String> {
    platform::run(root, command, timeout, cancel, channel, read_only, shell)
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn installed_node_and_npm_are_available_inside_the_appcontainer() {
        let root =
            std::env::temp_dir().join(format!("converge-node-path-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let result = super::isolated_with_options(
            &root,
            "node --version && npm --version && echo NODE_NPM_PATH_OK",
            30,
            tokio_util::sync::CancellationToken::new(),
            None,
            true,
            "cmd",
        );
        let resolved = root.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        let _ = std::fs::remove_dir_all(resolved);
        let result = result.unwrap();
        assert!(result.contains("NODE_NPM_PATH_OK"), "{result}");
    }
    #[test]
    fn automatic_shell_preserves_powershell_semantics() {
        assert_eq!(
            super::select_shell("git status --short", None).unwrap(),
            "cmd"
        );
        assert_eq!(super::select_shell("npm test", None).unwrap(), "cmd");
        for command in [
            "Get-ChildItem .",
            "git status | Select-Object -First 1",
            "node -e '$env:USERPROFILE'",
            "echo $env:USERPROFILE",
        ] {
            assert_eq!(super::select_shell(command, None).unwrap(), "powershell");
        }
        assert_eq!(
            super::select_shell("git status", Some("powershell")).unwrap(),
            "powershell"
        );
        assert!(super::select_shell("git status", Some("other")).is_err());
    }
    #[test]
    fn appcontainer_runs_writes_and_denies_private_files() {
        let base =
            std::env::temp_dir().join(format!("converge-isolation-test-{}", uuid::Uuid::new_v4()));
        let inside = base.join("workspace");
        std::fs::create_dir_all(&inside).unwrap();
        let outside = base.join("private.txt");
        std::fs::write(&outside, "CONVERGE_PRIVATE_SENTINEL").unwrap();
        let command=format!("echo CONVERGE_EXECUTED & echo WRITTEN>proof.txt & type \"{}\" & echo CONVERGE_FINISHED",outside.display());
        let result = super::isolated(
            &inside,
            &command,
            10,
            tokio_util::sync::CancellationToken::new(),
            None,
        )
        .unwrap();
        assert!(result.contains("CONVERGE_EXECUTED"), "{result}");
        assert!(result.contains("CONVERGE_FINISHED"), "{result}");
        assert!(
            !result.contains("CONVERGE_PRIVATE_SENTINEL"),
            "AppContainer leaked a private file: {result}"
        );
        assert!(std::fs::read_to_string(inside.join("proof.txt"))
            .unwrap()
            .contains("WRITTEN"));
        let resolved = base.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(resolved).unwrap();
    }
    #[test]
    fn readonly_powershell_lists_large_workspace_blocks_writes_and_outside_reads() {
        let base =
            std::env::temp_dir().join(format!("converge-readonly-test-{}", uuid::Uuid::new_v4()));
        let inside = base.join("workspace");
        std::fs::create_dir_all(inside.join("nested")).unwrap();
        std::fs::write(inside.join("nested/visible.txt"), "VISIBLE_NESTED").unwrap();
        let big = std::fs::File::create(inside.join("large.bin")).unwrap();
        // A sparse file preserves the >512 MB regression without reserving it.
        use std::os::windows::io::AsRawHandle;
        let mut returned = 0;
        assert_ne!(
            unsafe {
                windows_sys::Win32::System::IO::DeviceIoControl(
                    big.as_raw_handle() as _,
                    0x000900C4,
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            },
            0
        );
        big.set_len(600 * 1024 * 1024).unwrap();
        drop(big);
        let outside = base.join("private.txt");
        std::fs::write(&outside, "PRIVATE_SENTINEL").unwrap();
        let command = format!("Get-ChildItem -LiteralPath '{}' | Select-Object -ExpandProperty Name; Get-Content nested/visible.txt; try {{ Set-Content blocked.txt 'WRONG' -ErrorAction Stop }} catch {{ 'WRITE_DENIED' }}; try {{ Get-Content -LiteralPath '{}' -ErrorAction Stop }} catch {{ 'OUTSIDE_DENIED' }}", inside.display(), outside.display());
        let result = super::isolated_with_options(
            &inside,
            &command,
            30,
            tokio_util::sync::CancellationToken::new(),
            None,
            true,
            "powershell",
        )
        .unwrap();
        assert!(result.contains("large.bin"), "{result}");
        assert!(result.contains("VISIBLE_NESTED"), "{result}");
        assert!(result.contains("WRITE_DENIED"), "{result}");
        assert!(result.contains("OUTSIDE_DENIED"), "{result}");
        assert!(!result.contains("PRIVATE_SENTINEL"), "{result}");
        assert!(!inside.join("blocked.txt").exists());
        // An inherited SID ACE must be removed on completion, including nested files.
        for path in [&inside, &inside.join("nested/visible.txt")] {
            let acl = std::process::Command::new("icacls")
                .arg(path)
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&acl.stdout);
            assert!(
                !text.contains("S-1-15-2-"),
                "Leaked AppContainer ACL: {text}"
            );
        }
        let resolved = base.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(resolved).unwrap();
    }
    #[test]
    #[ignore = "latency measurement"]
    fn measure_command_startup() {
        let root = std::env::temp_dir().join(format!("converge-latency-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        for shell in ["cmd", "powershell"] {
            for _ in 0..3 {
                let time = std::time::Instant::now();
                let result = super::isolated_with_options(
                    &root,
                    "echo LATENCY_OK",
                    10,
                    tokio_util::sync::CancellationToken::new(),
                    None,
                    true,
                    shell,
                )
                .unwrap();
                assert!(result.contains("LATENCY_OK"));
                println!("{shell}: {} ms", time.elapsed().as_millis());
            }
        }
        let target = root.canonicalize().unwrap();
        assert!(target.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(target).unwrap();
    }
    #[test]
    fn appcontainer_cancellation_kills_child_processes() {
        let base =
            std::env::temp_dir().join(format!("converge-cancel-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let token = tokio_util::sync::CancellationToken::new();
        let other = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            other.cancel();
        });
        let started = std::time::Instant::now();
        let result = super::isolated(&base, "for /l %i in (1,0,2) do @rem wait", 10, token, None)
            .unwrap_err();
        assert!(started.elapsed().as_secs() < 5, "{result}");
        assert!(result.contains("Arret"), "{result}");
        let resolved = base.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(resolved).unwrap();
    }
}
