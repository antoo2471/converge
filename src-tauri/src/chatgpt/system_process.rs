//! System programs must not load libraries from Converge's mounted AppImage.
use std::process::Command;

pub(super) fn prepare(command: &mut Command) {
    #[cfg(target_os = "linux")]
    if let (Some(appdir), Some(libraries)) = (
        std::env::var_os("APPDIR"),
        std::env::var_os("LD_LIBRARY_PATH"),
    ) {
        remove_bundle_libraries(command, std::path::Path::new(&appdir), &libraries);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = command;
}

#[cfg(target_os = "linux")]
fn remove_bundle_libraries(
    command: &mut Command,
    appdir: &std::path::Path,
    libraries: &std::ffi::OsStr,
) {
    // Ignore missing/relative mount paths and the filesystem root. Do not alter
    // ordinary installations or unrelated, user-supplied library search paths.
    if !appdir.is_absolute() || appdir.parent().is_none() {
        return;
    }
    let paths: Vec<_> = std::env::split_paths(libraries).collect();
    let retained: Vec<_> = paths
        .iter()
        .filter(|path| !path.starts_with(appdir))
        .collect();
    if retained.len() == paths.len() {
        return;
    }
    if retained.is_empty() {
        // An empty LD_LIBRARY_PATH would add the working directory to the search.
        command.env_remove("LD_LIBRARY_PATH");
    } else if let Ok(value) = std::env::join_paths(retained) {
        command.env("LD_LIBRARY_PATH", value);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::{ffi::OsStr, path::Path};

    fn override_for(appdir: &str, libraries: &OsStr) -> Option<Option<std::ffi::OsString>> {
        let mut command = Command::new("unused");
        remove_bundle_libraries(&mut command, Path::new(appdir), libraries);
        command
            .get_envs()
            .find(|(key, _)| *key == "LD_LIBRARY_PATH")
            .map(|(_, value)| value.map(ToOwned::to_owned))
    }

    #[test]
    fn removes_only_paths_inside_the_appimage() {
        assert_eq!(
            override_for("/tmp/converge.AppDir", OsStr::new(
                "/tmp/converge.AppDir/usr/lib:/opt/custom/lib:/tmp/converge.AppDir/usr/lib/x86_64-linux-gnu:/tmp/converge.AppDir-other/lib:/usr/lib"
            )),
            Some(Some("/opt/custom/lib:/tmp/converge.AppDir-other/lib:/usr/lib".into()))
        );
        assert_eq!(
            override_for(
                "/tmp/converge.AppDir",
                OsStr::new("/tmp/converge.AppDir/usr/lib")
            ),
            Some(None)
        );
    }

    #[test]
    fn leaves_unrelated_library_paths_and_invalid_mount_paths_untouched() {
        for appdir in ["/tmp/converge.AppDir", "", "relative/AppDir", "/"] {
            assert_eq!(
                override_for(appdir, OsStr::new("/opt/custom/lib:/usr/lib")),
                None
            );
        }
    }

    #[test]
    fn preserves_non_utf8_paths() {
        use std::os::unix::ffi::OsStrExt;
        assert_eq!(
            override_for(
                "/tmp/converge.AppDir",
                OsStr::from_bytes(b"/tmp/converge.AppDir/usr/lib:/opt/\xff/lib")
            ),
            Some(Some(OsStr::from_bytes(b"/opt/\xff/lib").to_owned()))
        );
    }

    #[test]
    fn system_child_receives_clean_libraries_without_changing_parent_environment() {
        let before = std::env::var_os("LD_LIBRARY_PATH");
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf '%s' \"${LD_LIBRARY_PATH-unset}\""]);
        command.env("LD_LIBRARY_PATH", "/tmp/converge.AppDir/usr/lib");
        command.env("CONVERGE_ENV_TEST", "preserved");
        remove_bundle_libraries(
            &mut command,
            Path::new("/tmp/converge.AppDir"),
            OsStr::new("/tmp/converge.AppDir/usr/lib"),
        );
        let output = command.output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"unset");
        assert_eq!(std::env::var_os("LD_LIBRARY_PATH"), before);
        assert!(command.get_envs().any(
            |(key, value)| key == "CONVERGE_ENV_TEST" && value == Some(OsStr::new("preserved"))
        ));
    }
}
