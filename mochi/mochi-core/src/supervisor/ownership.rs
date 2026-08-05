use super::*;

const SUPERVISOR_LOCK_FILE: &str = ".supervisor.lock";

/// Persistent, process-lifetime ownership of one supervised network root.
#[derive(Debug)]
pub(super) struct SupervisorOwnershipLock {
    root: PathBuf,
    file: fs::File,
}

impl SupervisorOwnershipLock {
    pub(super) fn acquire(root: &Path) -> Result<Arc<Self>> {
        reject_symlink(root, "supervisor network root")?;
        fs::create_dir_all(root)?;
        reject_symlink(root, "supervisor network root")?;
        let root = fs::canonicalize(root)?;
        let path = root.join(SUPERVISOR_LOCK_FILE);
        reject_symlink(&path, "supervisor ownership lock")?;

        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options.open(&path)?;
        validate_lock_file(&path, &file)?;
        file.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => {
                SupervisorError::SupervisorLocked { path: path.clone() }
            }
            fs::TryLockError::Error(error) => SupervisorError::Io(error),
        })?;
        validate_lock_file(&path, &file)?;
        Ok(Arc::new(Self { root, file }))
    }

    pub(super) fn ensure_root(&self, root: &Path) -> Result<()> {
        if !self.matches_root(root)? {
            return Err(SupervisorError::Config(format!(
                "replacement supervisor root `{}` differs from owned root `{}`",
                root.display(),
                self.root.display()
            )));
        }
        Ok(())
    }

    pub(super) fn matches_root(&self, root: &Path) -> Result<bool> {
        reject_symlink(root, "supervisor network root")?;
        match fs::canonicalize(root) {
            Ok(root) => Ok(root == self.root),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// Clone the ownership descriptor into a child process's standard input.
    ///
    /// `File::try_clone` preserves the same underlying lock ownership. The
    /// descriptor is then inherited across `exec` as fd 0, so an orphaned peer
    /// continues fencing this network root after its controller exits.
    pub(super) fn child_stdin(&self) -> Result<Stdio> {
        Ok(Stdio::from(self.file.try_clone()?))
    }
}

fn reject_symlink(path: &Path, label: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(SupervisorError::GenerationValidation(format!(
                "{label} `{}` must not be a symbolic link",
                path.display()
            )))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn validate_lock_file(path: &Path, file: &fs::File) -> Result<()> {
    let opened = file.metadata()?;
    let named = fs::symlink_metadata(path)?;
    if !opened.is_file() || !named.is_file() || named.file_type().is_symlink() {
        return Err(SupervisorError::GenerationValidation(format!(
            "supervisor ownership lock `{}` must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if opened.dev() != named.dev() || opened.ino() != named.ino() || opened.nlink() != 1 {
            return Err(SupervisorError::GenerationValidation(format!(
                "supervisor ownership lock `{}` changed while it was opened",
                path.display()
            )));
        }
        if opened.permissions().mode() & 0o077 != 0 {
            return Err(SupervisorError::GenerationValidation(format!(
                "supervisor ownership lock `{}` must be owner-only",
                path.display()
            )));
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn inherited_child_stdin_keeps_ownership_lock_until_child_exit() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("network");
        let owner = SupervisorOwnershipLock::acquire(&root).expect("acquire owner");
        let mut child = Command::new("/bin/sleep")
            .arg("30")
            .stdin(owner.child_stdin().expect("clone owner into child stdin"))
            .spawn()
            .expect("spawn lock-retaining child");

        drop(owner);
        let error = SupervisorOwnershipLock::acquire(&root)
            .expect_err("orphan child must retain ownership");
        assert!(matches!(error, SupervisorError::SupervisorLocked { .. }));

        child.kill().expect("terminate child");
        child.wait().expect("reap child");
        SupervisorOwnershipLock::acquire(&root)
            .expect("child exit must release inherited ownership");
    }

    #[test]
    fn ownership_acquisition_creates_only_the_lock_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("network");
        let _owner = SupervisorOwnershipLock::acquire(&root).expect("acquire owner");
        let entries = fs::read_dir(&root)
            .expect("read owned root")
            .map(|entry| entry.expect("root entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, [OsString::from(SUPERVISOR_LOCK_FILE)]);
        let mode = fs::metadata(root.join(SUPERVISOR_LOCK_FILE))
            .expect("lock metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "ownership lock must be owner-only");
    }

    #[test]
    fn ownership_lock_symlink_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("network");
        fs::create_dir(&root).expect("create root");
        let target = temp.path().join("attacker-lock");
        fs::write(&target, b"attacker").expect("write target");
        symlink(&target, root.join(SUPERVISOR_LOCK_FILE)).expect("link ownership lock");
        let error =
            SupervisorOwnershipLock::acquire(&root).expect_err("symlink lock must be rejected");
        assert!(matches!(error, SupervisorError::GenerationValidation(_)));
    }

    #[test]
    fn existing_insecure_ownership_lock_mode_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("network");
        fs::create_dir(&root).expect("create root");
        let lock = root.join(SUPERVISOR_LOCK_FILE);
        fs::write(&lock, b"").expect("create existing lock");
        let mut permissions = fs::metadata(&lock).expect("lock metadata").permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(&lock, permissions).expect("set insecure lock mode");

        let error = SupervisorOwnershipLock::acquire(&root)
            .expect_err("group/world-accessible lock must be rejected");
        assert!(matches!(error, SupervisorError::GenerationValidation(_)));
        assert!(error.to_string().contains("owner-only"));
    }

    #[test]
    fn hard_linked_ownership_lock_inode_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("network");
        fs::create_dir(&root).expect("create root");
        let lock = root.join(SUPERVISOR_LOCK_FILE);
        fs::write(&lock, b"").expect("create existing lock");
        let mut permissions = fs::metadata(&lock).expect("lock metadata").permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&lock, permissions).expect("secure lock mode");
        fs::hard_link(&lock, temp.path().join("lock-hard-link")).expect("hard-link lock inode");

        let error = SupervisorOwnershipLock::acquire(&root)
            .expect_err("multiply-linked lock inode must be rejected");
        assert!(matches!(error, SupervisorError::GenerationValidation(_)));
        assert!(error.to_string().contains("changed while it was opened"));
    }
}
