//! Canonical private workspace, secrets and retained log fixtures for profile proofs.

use color_eyre::eyre::{Result, eyre};
use iroha_fs::{PrivateDirectory, PublishMode, RetainedFile};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Owner-only workspace below the canonical temporary directory, whose ancestors are trusted
/// (no symlink and no group- or world-writable component), as `node_secrets` requires.
pub(super) struct Workspace {
    directory: PrivateDirectory,
    temporary: tempfile::TempDir,
}

impl Workspace {
    pub(super) fn path(&self) -> &Path {
        self.directory.path()
    }

    pub(super) fn revalidate(&self) -> Result<()> {
        Ok(self.directory.revalidate()?)
    }

    pub(super) fn keep(self) -> PathBuf {
        let Self {
            directory,
            temporary,
        } = self;
        let path = directory.path().to_owned();
        drop(directory);
        let _ = temporary.keep();
        path
    }
}

pub(super) fn workspace() -> Result<Workspace> {
    let base = fs::canonicalize(std::env::temp_dir())?;
    let mut builder = tempfile::Builder::new();
    builder.prefix("sora-nexus-profile-network-");
    #[cfg(unix)]
    builder.permissions(fs::Permissions::from_mode(0o700));
    let temporary = builder.tempdir_in(base)?;
    let directory = PrivateDirectory::open_or_create(temporary.path().join("private"))?;
    Ok(Workspace {
        directory,
        temporary,
    })
}

pub(super) fn owner_only_dir(path: &Path) -> Result<()> {
    PrivateDirectory::open_or_create(path)?.revalidate()?;
    Ok(())
}

pub(super) fn private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = PrivateDirectory::open_exact(
        path.parent()
            .ok_or_else(|| eyre!("private file has no parent"))?,
    )?;
    parent.write_atomic(
        path.file_name()
            .ok_or_else(|| eyre!("private file has no name"))?,
        bytes,
        PublishMode::CreateNew,
    )?;
    parent.revalidate()?;
    Ok(())
}

pub(super) fn private_log(path: &Path) -> Result<RetainedFile> {
    let parent = PrivateDirectory::open_exact(
        path.parent()
            .ok_or_else(|| eyre!("private log has no parent"))?,
    )?;
    let original = RetainedFile::create_new_private(path)?;
    original.revalidate()?;
    parent.revalidate()?;
    Ok(original)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_fs::ReaderDirectory;
    use std::io::{Read as _, Seek as _, SeekFrom, Write as _};

    #[test]
    fn canonical_workspace_and_private_files_preserve_exact_bytes_and_create_new() {
        let workspace = workspace().unwrap();
        workspace.revalidate().unwrap();
        let directory = workspace.path().join("secrets");
        owner_only_dir(&directory).unwrap();
        let file = directory.join("secret");
        private_file(&file, b"original secret\n").unwrap();
        assert!(private_file(&file, b"replacement\n").is_err());
        assert_eq!(
            PrivateDirectory::open_exact(&directory)
                .unwrap()
                .read("secret", 64)
                .unwrap()
                .as_slice(),
            b"original secret\n"
        );
        assert_eq!(
            RetainedFile::open_private(&file)
                .unwrap()
                .snapshot()
                .unwrap()
                .local_parts()
                .2,
            16
        );
        #[cfg(unix)]
        {
            assert_eq!(
                fs::metadata(workspace.path()).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        workspace.revalidate().unwrap();
    }

    #[test]
    fn existing_nonprivate_directory_is_refused_without_repair() {
        let workspace = workspace().unwrap();
        let path = workspace.path().join("nonprivate");
        owner_only_dir(&path).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        #[cfg(windows)]
        {
            let result = std::process::Command::new("icacls")
                .arg(&path)
                .args(["/grant", "*S-1-1-0:(RX)", "/Q"])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        let reader = ReaderDirectory::open(&path).unwrap();
        let before = reader.snapshot().unwrap();
        assert!(owner_only_dir(&path).is_err());
        assert!(PrivateDirectory::open_exact(&path).is_err());
        assert_eq!(reader.snapshot().unwrap(), before);
    }

    #[test]
    fn existing_shared_private_leaf_never_becomes_a_new_secret_or_log() {
        let workspace = workspace().unwrap();
        let original = workspace.path().join("secret");
        let alias = workspace.path().join("alias");
        private_file(&original, b"original").unwrap();
        fs::hard_link(&original, &alias).unwrap();
        assert!(private_file(&alias, b"replacement").is_err());
        assert!(private_log(&alias).is_err());
        assert!(RetainedFile::open_private(&original).is_err());
        fs::remove_file(alias).unwrap();
        assert_eq!(
            workspace.directory.read("secret", 16).unwrap().as_slice(),
            b"original"
        );
    }

    const LOG_CHILD: &str = "IROHA_PROFILE_PRIVATE_LOG_CHILD";

    #[test]
    fn log_child() {
        if std::env::var_os(LOG_CHILD).is_some() {
            std::io::stdout()
                .write_all(b"profile-original-log\n")
                .unwrap();
        }
    }

    #[test]
    fn inherited_child_writer_keeps_the_original_private_log_and_parent() {
        let workspace = workspace().unwrap();
        let path = workspace.path().join("child.log");
        let mut original = private_log(&path).unwrap();
        let identity = original.identity().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "profile_files::tests::log_child", "--nocapture"])
            .env(LOG_CHILD, "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::from(
                original.file().try_clone().unwrap(),
            ))
            .stderr(std::process::Stdio::inherit())
            .status()
            .unwrap();
        assert!(status.success());
        original.file().sync_all().unwrap();
        original.revalidate().unwrap();
        assert_eq!(original.identity().unwrap(), identity);
        original.file_mut().seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = String::new();
        original.file_mut().read_to_string(&mut bytes).unwrap();
        assert!(bytes.contains("profile-original-log\n"));
        original.revalidate().unwrap();
        workspace.revalidate().unwrap();
    }

    #[test]
    fn keeping_failure_evidence_releases_original_before_retaining_path() {
        let workspace = workspace().unwrap();
        let path = workspace.path().to_owned();
        private_file(&path.join("evidence"), b"diagnostic").unwrap();
        assert_eq!(workspace.keep(), path);
        assert_eq!(
            PrivateDirectory::open_exact(&path)
                .unwrap()
                .read("evidence", 32)
                .unwrap()
                .as_slice(),
            b"diagnostic"
        );
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
