//! Native temporary placement and control-file I/O for the real reward crash test.
//!
//! These helpers contain no State, certificate, archive or replay substitution.

use std::{io, path::Path, process::Command};

/// Keep every child-created temporary inside its parent's disposable case root.
pub(super) fn configure_temporary_directories(command: &mut Command, root: &Path) {
    command.env("TMPDIR", root);
    #[cfg(windows)]
    command.env("TEMP", root).env("TMP", root);
}

/// Publish the fixture's original carrier coordinates before arming process exit.
#[cfg(unix)]
pub(super) fn publish_original_control(root: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::{fs::File, io::Write as _};
    // Preserve the existing Unix fixture's file-then-directory durability sequence.
    let mut manifest = File::create(root.join("original.norito"))?;
    manifest.write_all(bytes)?;
    manifest.sync_all()?;
    File::open(root)?.sync_all()
}

/// Publish the fixture coordinates through retained native directory authority.
#[cfg(windows)]
pub(super) fn publish_original_control(root: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = iroha_fs::OwnerDirectory::open(root)?;
    parent.write_atomic("original.norito", bytes, iroha_fs::PublishMode::CreateNew)?;
    parent.revalidate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn child_temporary_directory_is_inside_its_original_case_root() {
        const CHILD_ROOT: &str = "IROHA_REWARD_CRASH_TEMP_FIXTURE_ROOT";
        if let Some(root) = std::env::var_os(CHILD_ROOT) {
            let root = fs::canonicalize(root).unwrap();
            let child = tempfile::tempdir().unwrap();
            let selected = fs::canonicalize(child.path()).unwrap();
            assert!(selected.starts_with(&root));
            assert_ne!(selected, root);
            fs::write(child.path().join("native-temp-placement"), b"original").unwrap();
            return;
        }
        let parent = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(parent.path()).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        let test = concat!(
            module_path!(),
            "::child_temporary_directory_is_inside_its_original_case_root"
        );
        command
            .args(["--exact", test.split_once("::").unwrap().1, "--nocapture"])
            .env(CHILD_ROOT, &root);
        configure_temporary_directories(&mut command, &root);
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
    }

    #[test]
    fn original_control_is_recoverable_after_native_publication() {
        let root = tempfile::tempdir().unwrap();
        let original_root = iroha_fs::FileMetadata::symlink_metadata(root.path())
            .unwrap()
            .identity();
        let bytes = b"canonical coordinates supplied by the crash owner\n";
        publish_original_control(root.path(), bytes).unwrap();
        let path = root.path().join("original.norito");
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(
            iroha_fs::FileMetadata::symlink_metadata(root.path())
                .unwrap()
                .identity(),
            original_root
        );
        #[cfg(windows)]
        {
            let original = iroha_fs::RetainedFile::open_private(&path).unwrap();
            let snapshot = original.snapshot().unwrap();
            assert!(publish_original_control(root.path(), b"replacement").is_err());
            original.revalidate().unwrap();
            assert_eq!(original.snapshot().unwrap(), snapshot);
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
    }
}
