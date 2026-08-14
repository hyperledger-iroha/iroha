//! Private helpers shared by crate-internal tests.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

/// Process-unique temporary directory removed recursively on drop.
pub(crate) struct TestDirectory(PathBuf);

impl TestDirectory {
    /// Create one isolated directory for a named test family.
    pub(crate) fn new(label: &str) -> Self {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "iroha-zkp-halo2-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated ZKP test directory");
        Self(path)
    }

    /// Borrow the directory path.
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
