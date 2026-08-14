//! Process-lifetime ownership lock for durable replay ledgers.
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};
/// Exclusive advisory lock held for as long as a persistent ledger is open.
#[derive(Debug)]
pub(super) struct ExclusiveLedgerLock {
    _file: File,
}
impl ExclusiveLedgerLock {
    /// Acquire the sidecar lock associated with `ledger_path`.
    pub(super) fn acquire(ledger_path: &Path) -> io::Result<Self> {
        if let Some(parent) = ledger_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let lock_path = lock_path(ledger_path);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;
        file.try_lock().map_err(|error| {
            io::Error::other(format!(
                "failed to acquire exclusive replay-ledger lock {}: {error}",
                lock_path.display()
            ))
        })?;
        Ok(Self { _file: file })
    }
}
fn lock_path(ledger_path: &Path) -> PathBuf {
    let mut path = OsString::from(ledger_path.as_os_str());
    path.push(".lock");
    PathBuf::from(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    #[test]
    fn ledger_lock_excludes_concurrent_owner_and_releases_on_drop() {
        let directory = tempdir().expect("temporary directory");
        let ledger = directory.path().join("replays.norito");
        let first = ExclusiveLedgerLock::acquire(&ledger).expect("first owner");
        let error =
            ExclusiveLedgerLock::acquire(&ledger).expect_err("second owner must be rejected");
        assert!(
            error.to_string().contains("exclusive replay-ledger lock"),
            "unexpected lock error: {error}"
        );
        drop(first);
        ExclusiveLedgerLock::acquire(&ledger).expect("lock released with owner");
    }
}
