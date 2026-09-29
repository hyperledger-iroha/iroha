#[derive(Debug)]
struct BoundProgressPromotionError {
    published: bool,
    source: std::io::Error,
}
/// Stable classification for a failed bound progress-sidecar recovery pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoundProgressRecoveryFailure {
    /// The on-disk protocol state remains structurally recoverable, but an I/O
    /// or durability operation did not complete.
    RetryableIo,
    /// The namespace or protocol state is hostile, malformed, or ambiguous.
    InvalidData,
}
impl BoundProgressRecoveryFailure {
    fn from_io(error: &std::io::Error) -> Self {
        match error.kind() {
            ErrorKind::InvalidData
            | ErrorKind::InvalidInput
            | ErrorKind::NotFound
            | ErrorKind::AlreadyExists
            | ErrorKind::PermissionDenied
            | ErrorKind::UnexpectedEof => Self::InvalidData,
            _ => Self::RetryableIo,
        }
    }
    fn from_kura(error: &Error) -> Self {
        match error {
            Error::IO(source, _) | Error::MkDir(source, _) => Self::from_io(source),
            _ => Self::InvalidData,
        }
    }
}
/// Raw and independently scanned Kura disk-usage state exposed only to crate tests.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DiskUsageAccountingSnapshotForTesting {
    /// Whether the enforced-usage cache is currently valid.
    pub(crate) enforced_initialized: bool,
    /// Whether the total-usage cache is currently valid.
    pub(crate) total_initialized: bool,
    /// Raw cached enforced bytes without triggering a refresh.
    pub(crate) cached_enforced_bytes: u64,
    /// Raw cached total bytes without triggering a refresh.
    pub(crate) cached_total_bytes: u64,
    /// Exact enforced bytes from a read-only filesystem scan.
    pub(crate) exact_enforced_bytes: u64,
    /// Exact total bytes from a read-only filesystem scan.
    pub(crate) exact_total_bytes: u64,
}
#[derive(Debug, Default)]
struct TotalDiskUsageAccountingState {
    generation: u64,
    mutations_in_flight: usize,
}
impl Kura {
    #[cfg(all(unix, not(target_os = "espidf")))]
    fn open_safety_wal_store_root_directory(
        store_root: &Path,
        store_root_lock_file: &std::fs::File,
    ) -> Result<BoundProgressDirectory> {
        use std::os::unix::fs::MetadataExt as _;
        let lock_path = store_root.join(STORE_ROOT_LOCK_FILE_NAME);
        let lock_before = store_root_lock_file
            .metadata()
            .map_err(|error| Error::IO(error, lock_path.clone()))?;
        let root = Self::open_bound_progress_directory(store_root, store_root)?;
        let entry_before = rustix::fs::statat(
            &root.file,
            STORE_ROOT_LOCK_FILE_NAME,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(std::io::Error::from)
        .map_err(|error| Error::IO(error, lock_path.clone()))?;
        let linked_lock = std::fs::File::from(
            rustix::fs::openat(
                &root.file,
                STORE_ROOT_LOCK_FILE_NAME,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .map_err(|error| Error::IO(error, lock_path.clone()))?,
        );
        let linked_metadata = linked_lock
            .metadata()
            .map_err(|error| Error::IO(error, lock_path.clone()))?;
        let entry_after = rustix::fs::statat(
            &root.file,
            STORE_ROOT_LOCK_FILE_NAME,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(std::io::Error::from)
        .map_err(|error| Error::IO(error, lock_path.clone()))?;
        let lock_after = store_root_lock_file
            .metadata()
            .map_err(|error| Error::IO(error, lock_path.clone()))?;
        if rustix::fs::FileType::from_raw_mode(entry_before.st_mode)
            != rustix::fs::FileType::RegularFile
            || entry_before.st_nlink as u64 != 1
            || entry_before.st_dev as u64 != linked_metadata.dev()
            || entry_before.st_ino as u64 != linked_metadata.ino()
            || entry_after.st_dev as u64 != linked_metadata.dev()
            || entry_after.st_ino as u64 != linked_metadata.ino()
            || !Self::sidecar_file_metadata_unchanged(&lock_before, &linked_metadata)
            || !Self::sidecar_file_metadata_unchanged(&lock_before, &lock_after)
        {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "opened Kura store root does not retain its exact locked identity",
                ),
                lock_path,
            ));
        }
        Ok(root)
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    fn open_or_create_bound_storage_child_directory(
        &self,
        parent: &BoundProgressDirectory,
        name: &std::ffi::OsStr,
    ) -> Result<BoundProgressDirectory> {
        let expected_path = parent.expected_path.join(name);
        if !self.bound_storage_directory_unchanged(parent) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "safety-WAL parent directory changed before child binding",
                ),
                parent.expected_path.clone(),
            ));
        }
        match rustix::fs::mkdirat(&parent.file, name, rustix::fs::Mode::RWXU) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => {}
            Err(error) => return Err(Error::IO(std::io::Error::from(error), expected_path)),
        }
        let child =
            Self::open_bound_progress_child_directory(&self.store_root, parent, &expected_path)?;
        if !self.bound_storage_directory_unchanged(parent) {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    "safety-WAL parent directory changed while opening its child",
                ),
                parent.expected_path.clone(),
            ));
        }
        parent
            .file
            .sync_all()
            .map_err(|error| Error::IO(error, parent.expected_path.clone()))?;
        Ok(child)
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    fn bound_storage_directory_unchanged(&self, directory: &BoundProgressDirectory) -> bool {
        use std::os::unix::fs::MetadataExt as _;
        let Ok(opened) = directory.file.metadata() else {
            return false;
        };
        if !opened.is_dir()
            || !Self::sidecar_directory_binding_unchanged(&directory.metadata, &opened)
        {
            return false;
        }
        if directory.entry_name.is_none() {
            let Ok(linked) = std::fs::symlink_metadata(&self.store_root) else {
                return false;
            };
            return !linked.file_type().is_symlink()
                && linked.is_dir()
                && linked.dev() == opened.dev()
                && linked.ino() == opened.ino();
        }
        let Ok(canonical) = std::fs::canonicalize(&directory.expected_path) else {
            return false;
        };
        canonical == directory.canonical_path
            && canonical.starts_with(&self.store_root_directory.canonical_path)
            && std::fs::symlink_metadata(&directory.expected_path).is_ok_and(|linked| {
                !linked.file_type().is_symlink()
                    && linked.is_dir()
                    && linked.dev() == opened.dev()
                    && linked.ino() == opened.ino()
            })
    }
}
