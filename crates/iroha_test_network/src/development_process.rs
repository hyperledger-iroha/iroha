//! Explicit developer-tool selection before native owned process creation.

use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use iroha_fs::ProcessCommand;

/// Prepare an unqualified developer command from an explicit path or native PATH entry.
///
/// Release admission is checked before any developer path is inspected. This
/// operation supplies no release authority and cannot select a release fallback.
///
/// # Errors
/// Refuses active or incomplete release anchors, missing/nonexecutable tools, and
/// paths which cannot be resolved to an absolute native executable.
pub fn command(tool: impl AsRef<OsStr>) -> color_eyre::Result<ProcessCommand> {
    command_in_repo(tool.as_ref(), &crate::compiled_repo_path())
}

/// Prepare a developer command only after ruling out release authority for this repository.
///
/// # Errors
/// Returns release admission errors before inspecting the tool, or native selection errors.
pub(crate) fn command_in_repo(tool: &OsStr, repo: &Path) -> color_eyre::Result<ProcessCommand> {
    require_development_mode_in_repo(repo)?;
    let path = select(tool, std::env::var_os("PATH").as_deref())?;
    Ok(ProcessCommand::new(path)?)
}

/// Require developer mode before optional tool discovery or an absent-tool skip.
///
/// # Errors
/// Refuses active or incomplete release authority, including failed admission.
pub fn require_development_process_mode() -> color_eyre::Result<()> {
    require_development_mode_in_repo(&crate::compiled_repo_path())
}

fn require_development_mode_in_repo(repo: &Path) -> color_eyre::Result<()> {
    if crate::release_program_contract(repo)?.is_some() {
        return Err(color_eyre::eyre::eyre!(
            "developer tool discovery is forbidden while release admission is active"
        ));
    }
    Ok(())
}

fn executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn select(tool: &OsStr, search_path: Option<&OsStr>) -> io::Result<PathBuf> {
    let path = Path::new(tool);
    if tool.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty developer tool",
        ));
    }
    if path.is_absolute() || path.components().count() != 1 {
        let path = path.canonicalize()?;
        if !executable(&path) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "selected developer tool is not an executable file",
            ));
        }
        return Ok(path);
    }
    if let Some(search_path) = search_path {
        for directory in std::env::split_paths(search_path) {
            let candidate = directory.join(path);
            if executable(&candidate) {
                return candidate.canonicalize();
            }
            #[cfg(windows)]
            if path.extension().is_none() {
                let candidate = candidate.with_extension("exe");
                if executable(&candidate) {
                    return candidate.canonicalize();
                }
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("developer tool {tool:?} was not found on PATH"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, crate::native_program_fixture::image()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        path
    }

    #[test]
    fn explicit_original_and_path_selection_resolve_one_absolute_tool() {
        let root = tempfile::tempdir().unwrap();
        let name = if cfg!(windows) { "probe.exe" } else { "probe" };
        let original = fixture(root.path(), name);
        let expected = original.canonicalize().unwrap();
        assert_eq!(select(original.as_os_str(), None).unwrap(), expected);
        let path = std::env::join_paths([root.path()]).unwrap();
        assert_eq!(select(OsStr::new(name), Some(&path)).unwrap(), expected);
        #[cfg(windows)]
        assert_eq!(select(OsStr::new("probe"), Some(&path)).unwrap(), expected);
    }

    #[test]
    fn missing_empty_and_directory_tools_are_refused() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            select(OsStr::new(""), None).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            select(OsStr::new("missing-native-tool"), None)
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(
            select(root.path().as_os_str(), None).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(!executable(root.path()));
    }

    #[cfg(unix)]
    #[test]
    fn path_search_skips_nonexecutable_files_without_changing_their_access() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let refused = fixture(&first, "probe");
        fs::set_permissions(&refused, fs::Permissions::from_mode(0o600)).unwrap();
        let accepted = fixture(&second, "probe");
        let path = std::env::join_paths([first, second]).unwrap();
        assert_eq!(
            select(OsStr::new("probe"), Some(&path)).unwrap(),
            accepted.canonicalize().unwrap()
        );
        assert_eq!(
            fs::metadata(refused).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
