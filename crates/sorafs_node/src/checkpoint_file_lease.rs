//! Process-local ownership of an already opened checkpoint lock-file identity.
//!
//! Callers retain their file validation and operating-system lock. This lease
//! closes same-process alias races without serializing unrelated storage roots.

use std::{collections::BTreeSet, fs::File, io, sync::Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FileIdentity {
    volume: u64,
    index: u64,
}

static OWNED_FILES: Mutex<BTreeSet<FileIdentity>> = Mutex::new(BTreeSet::new());

/// One active in-process lease; registry entries disappear when owners drop.
pub(crate) struct CheckpointFileLease {
    identity: FileIdentity,
}

impl CheckpointFileLease {
    /// Claim a validated open file, returning `None` while that identity is owned.
    pub(crate) fn try_acquire(file: &File) -> io::Result<Option<Self>> {
        let identity = file_identity(file)?;
        let mut owned = OWNED_FILES
            .lock()
            .map_err(|_| io::Error::other("checkpoint file ownership registry is poisoned"))?;
        // The mutex protects only this in-memory set operation. File I/O and
        // OS locking happen outside it, so independent files never report busy.
        if !owned.insert(identity) {
            return Ok(None);
        }
        Ok(Some(Self { identity }))
    }
}

impl Drop for CheckpointFileLease {
    fn drop(&mut self) {
        OWNED_FILES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.identity);
    }
}

#[cfg(unix)]
fn file_identity(file: &File) -> io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = file.metadata()?;
    Ok(FileIdentity {
        volume: metadata.dev(),
        index: metadata.ino(),
    })
}

#[cfg(windows)]
fn file_identity(file: &File) -> io::Result<FileIdentity> {
    use std::os::windows::fs::MetadataExt as _;
    let metadata = file.metadata()?;
    Ok(FileIdentity {
        volume: metadata
            .volume_serial_number()
            .map(u64::from)
            .ok_or_else(|| io::Error::other("checkpoint file has no stable volume identity"))?,
        index: metadata
            .file_index()
            .ok_or_else(|| io::Error::other("checkpoint file has no stable file identity"))?,
    })
}

#[cfg(not(any(unix, windows)))]
fn file_identity(_file: &File) -> io::Result<FileIdentity> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "checkpoint file ownership is unsupported on this platform",
    ))
}

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "sorafs-checkpoint-file-lease-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&path).expect("create unique lease test directory");
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reopened_file_and_cloned_handle_share_one_lease_until_drop() {
        let directory = TestDirectory::new();
        let path = directory.0.join("lock");
        let file = File::create(&path).expect("create lock");
        let reopened = File::open(&path).expect("reopen lock");
        let cloned = file.try_clone().expect("clone open lock");
        let lease = CheckpointFileLease::try_acquire(&file)
            .expect("claim first handle")
            .expect("first owner");
        assert!(
            CheckpointFileLease::try_acquire(&reopened)
                .unwrap()
                .is_none()
        );
        assert!(CheckpointFileLease::try_acquire(&cloned).unwrap().is_none());
        drop(lease);
        assert!(
            CheckpointFileLease::try_acquire(&reopened)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn hardlink_alias_cannot_acquire_an_owned_file_identity() {
        let directory = TestDirectory::new();
        let path = directory.0.join("lock");
        let alias = directory.0.join("alias");
        let file = File::create(&path).expect("create lock");
        fs::hard_link(&path, &alias).expect("create identity alias");
        let alias_file = File::open(&alias).expect("open alias");
        let _lease = CheckpointFileLease::try_acquire(&file).unwrap().unwrap();
        assert!(
            CheckpointFileLease::try_acquire(&alias_file)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn independent_files_are_owned_concurrently_across_threads() {
        let directory = TestDirectory::new();
        let first = File::create(directory.0.join("first")).unwrap();
        let second = File::create(directory.0.join("second")).unwrap();
        let _first_lease = CheckpointFileLease::try_acquire(&first).unwrap().unwrap();
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    assert!(CheckpointFileLease::try_acquire(&first).unwrap().is_none());
                    let _second_lease = CheckpointFileLease::try_acquire(&second)
                        .unwrap()
                        .expect("an independent identity must not report contention");
                })
                .join()
                .expect("ownership thread completes");
        });
    }
}
