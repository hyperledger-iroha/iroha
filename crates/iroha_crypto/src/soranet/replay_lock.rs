//! Process-lifetime ownership lock for durable replay ledgers.
use super::snapshot_file::CustodiedSnapshotPath;
#[cfg(unix)]
use super::snapshot_file::SNAPSHOT_O_NOFOLLOW_FLAG;
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
};
/// Exclusive advisory lock held for as long as a persistent ledger is open.
#[derive(Debug)]
pub(super) struct ExclusiveLedgerLock {
    _file: File,
    custody: CustodiedSnapshotPath,
}
impl ExclusiveLedgerLock {
    /// Acquire the sidecar lock associated with `ledger_path`.
    pub(super) fn acquire(ledger_path: &Path) -> io::Result<Self> {
        // A ledger named `foo.lock` could otherwise replace the lock inode
        // protecting `foo` while its original owner is still running.
        if has_reserved_lock_suffix(ledger_path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "replay ledger path uses the reserved .lock suffix: {}",
                    ledger_path.display()
                ),
            ));
        }
        let custody = CustodiedSnapshotPath::prepare(ledger_path)?;
        let lock_path = lock_path(custody.destination());
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600).custom_flags(SNAPSHOT_O_NOFOLLOW_FLAG);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt as _;
            const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
            options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        }
        custody.verify_parent()?;
        let file = options.open(&lock_path)?;
        custody.validate_opened_file(&lock_path, &file, "replay ledger lock")?;
        file.try_lock().map_err(|error| {
            io::Error::other(format!(
                "failed to acquire exclusive replay-ledger lock {}: {error}",
                lock_path.display()
            ))
        })?;
        custody.validate_opened_file(&lock_path, &file, "replay ledger lock")?;
        Ok(Self {
            _file: file,
            custody,
        })
    }
    pub(super) fn custody(&self) -> &CustodiedSnapshotPath {
        &self.custody
    }
}
fn has_reserved_lock_suffix(path: &Path) -> bool {
    path.file_name().is_some_and(|file_name| {
        if let Some(file_name) = file_name.to_str() {
            // Case-insensitive filesystems can apply Unicode case mappings in
            // addition to ASCII folding (for example, Kelvin sign U+212A maps
            // to `k` on the default macOS filesystem).
            return file_name
                .trim_end_matches(['.', ' '])
                .to_lowercase()
                .ends_with(".lock");
        }
        let bytes = file_name.as_encoded_bytes();
        // Preserve the ASCII check for non-UTF-8 Unix names. Win32 normalizes
        // terminal spaces and dots for non-verbatim paths, so `foo.lock.` can
        // name the same file as `foo.lock`.
        let normalized_len = bytes
            .iter()
            .rposition(|byte| !matches!(byte, b'.' | b' '))
            .map_or(0, |index| index + 1);
        bytes[..normalized_len]
            .get(normalized_len.saturating_sub(5)..)
            .is_some_and(|suffix| suffix.eq_ignore_ascii_case(b".lock"))
    })
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
    #[cfg(unix)]
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
    #[cfg(unix)]
    #[test]
    fn ledger_lock_is_owner_private_and_rejects_symlinks_or_hard_links() {
        use std::{
            io::Write as _,
            os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, symlink},
        };
        let directory = tempdir().expect("temporary directory");
        let canonical_parent = std::fs::canonicalize(directory.path()).expect("canonical parent");

        let ledger = directory.path().join("private.norito");
        let owner = ExclusiveLedgerLock::acquire(&ledger).expect("create private lock");
        let created_lock = lock_path(&canonical_parent.join("private.norito"));
        let metadata = std::fs::symlink_metadata(&created_lock).expect("lock metadata");
        assert_eq!(metadata.mode() & 0o777, 0o600);
        assert_eq!(metadata.nlink(), 1);
        drop(owner);

        let symlink_ledger = directory.path().join("symlink.norito");
        let symlink_lock = lock_path(&canonical_parent.join("symlink.norito"));
        symlink(&created_lock, &symlink_lock).expect("create lock symlink");
        ExclusiveLedgerLock::acquire(&symlink_ledger)
            .expect_err("a symlinked lock path must fail closed");

        let hardlink_ledger = directory.path().join("hardlink.norito");
        let hardlink_lock = lock_path(&canonical_parent.join("hardlink.norito"));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true).mode(0o600);
        let mut seed = options.open(&hardlink_lock).expect("create lock fixture");
        seed.write_all(b"lock").expect("write lock fixture");
        std::fs::hard_link(&hardlink_lock, canonical_parent.join("lock-alias"))
            .expect("create lock hard link");
        ExclusiveLedgerLock::acquire(&hardlink_ledger)
            .expect_err("a multiply-linked lock file must fail closed");
    }
    #[test]
    fn ledger_lock_rejects_reserved_sidecar_suffix() {
        for path in [
            "replays.norito.lock",
            "other.LoCk",
            "windows.lock.",
            "windows.LOCK ... ",
            "unicode.locK",
        ] {
            assert!(has_reserved_lock_suffix(Path::new(path)));
            let error = ExclusiveLedgerLock::acquire(Path::new(path))
                .expect_err("the reserved sidecar suffix must be rejected before custody");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert!(error.to_string().contains("reserved .lock suffix"));
        }
        assert!(!has_reserved_lock_suffix(Path::new("ledger.lock.backup")));
    }
    #[cfg(unix)]
    #[test]
    fn ledger_lock_reserves_its_live_sidecar_name() {
        use std::os::unix::ffi::OsStringExt as _;

        let directory = tempdir().expect("temporary directory");
        let ledger = directory.path().join("replays.norito");
        let owner = ExclusiveLedgerLock::acquire(&ledger).expect("primary ledger owner");

        let colliding_ledger = lock_path(&ledger);
        let error = ExclusiveLedgerLock::acquire(&colliding_ledger)
            .expect_err("a sidecar path must not become another ledger path");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("reserved .lock suffix"));

        let mut non_utf8_name = vec![0xFF];
        non_utf8_name.extend_from_slice(b".LOCK");
        let non_utf8 = directory.path().join(OsString::from_vec(non_utf8_name));
        let error = ExclusiveLedgerLock::acquire(&non_utf8)
            .expect_err("non-UTF-8 names must retain the reserved suffix check");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        ExclusiveLedgerLock::acquire(&directory.path().join("ledger.lock.backup"))
            .expect("non-terminal lock text is not reserved");
        drop(owner);
    }
}
