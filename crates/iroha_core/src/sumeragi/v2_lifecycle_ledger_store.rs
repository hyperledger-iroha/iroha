/// Typed LifecycleLedgerV1 load or persistence failure.
#[derive(Debug, Error)]
pub(in crate::sumeragi) enum LifecycleLedgerError {
    /// A filesystem operation failed.
    #[error("{0}")]
    Io(String),
    /// Frame bytes were malformed or noncanonical.
    #[error("invalid LifecycleLedgerV1 frame: {0}")]
    InvalidFrame(String),
    /// Decoded logical state violated a durable invariant.
    #[error("invalid LifecycleLedgerV1 state: {0}")]
    InvalidLedger(String),
}
/// Post-fsync receipt for one exact WAL-ahead Validate-to-Sign ledger repair.
///
/// Construction is private to [`LifecycleLedgerStoreV1`]. The receipt binds
/// both semantic keys, the typed edge and child ordinal, and the complete
/// framed ledger bytes which were published before it was returned.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct DurableWalVoteLedgerRepairReceipt {
    store_path: PathBuf,
    #[cfg(all(unix, not(target_os = "espidf")))]
    store_directory_identity: LifecycleStorageIdentity,
    context: LifecycleContext,
    parent_key: LifecycleKey,
    child_key: LifecycleKey,
    edge: DurableContinuationEdge,
    child_ordinal: u128,
    ledger_frame_hash: LifecycleDigest,
}
impl DurableWalVoteLedgerRepairReceipt {
    /// Return whether this receipt names one exact authenticated repair.
    pub(super) fn matches(&self, repair: &AuthenticatedWalVoteLifecycleRepair) -> bool {
        self.context.id() == repair.parent().key.context()
            && self.context.height() == repair.parent().key.round().height()
            && self.parent_key == repair.parent().key
            && self.child_key == repair.child().key
            && self.edge == repair.edge()
            && self.child_ordinal != 0
    }
    /// Return the durable child ordinal named by the published ledger.
    pub(super) const fn child_ordinal(&self) -> u128 {
        self.child_ordinal
    }
    /// Return the hash of the complete canonical ledger frame.
    pub(super) const fn ledger_frame_hash(&self) -> LifecycleDigest {
        self.ledger_frame_hash
    }
    /// Return whether the receipt belongs to this exact opened ledger store.
    pub(super) fn belongs_to(&self, store: &LifecycleLedgerStoreV1) -> bool {
        store
            .load()
            .ok()
            .is_some_and(|ledger| self.belongs_to_loaded(store, &ledger))
    }
    /// Validate this receipt against one already-loaded frame from its store.
    /// Keeping this comparison load-free lets the Sign-install preflight bind
    /// the frame hash and repaired-pair shape to the same read.
    pub(super) fn belongs_to_loaded(
        &self,
        store: &LifecycleLedgerStoreV1,
        ledger: &LifecycleLedgerV1,
    ) -> bool {
        let mut exact_target = self.store_path == store.path;
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            exact_target &= self.store_directory_identity == store.directory.identity;
        }
        exact_target
            && self.context == store.context
            && ledger.context() == self.context
            && encode_frame(ledger, store.max_frame_bytes)
                .ok()
                .is_some_and(|frame| {
                    LifecycleDigest::new(Hash::new(frame).into()) == self.ledger_frame_hash
                })
    }
}
/// Move-only proof that one canonical lifecycle frame physically existed.
///
/// A missing ledger path loads as the logical empty ledger for ordinary fresh
/// startup. CompleteTip recovery must distinguish that fallback from an exact
/// empty frame which an earlier lifecycle owner actually fsynced. Construction
/// therefore stays private to [`LifecycleLedgerStoreV1`] and binds the complete
/// publication target plus the canonical framed bytes observed on disk.
#[must_use = "a physically present lifecycle frame must enter its exact recovery join"]
struct AuthenticatedPresentLifecycleFrameV1 {
    store_path: PathBuf,
    #[cfg(all(unix, not(target_os = "espidf")))]
    store_directory_identity: LifecycleStorageIdentity,
    context: LifecycleContext,
    max_records: usize,
    max_frame_bytes: u64,
    ledger_frame_hash: LifecycleDigest,
}
impl AuthenticatedPresentLifecycleFrameV1 {
    fn binds_ledger(&self, ledger: &LifecycleLedgerV1) -> bool {
        ledger.context() == self.context
            && encode_frame(ledger, self.max_frame_bytes)
                .ok()
                .is_some_and(|frame| {
                    LifecycleDigest::new(Hash::new(frame).into()) == self.ledger_frame_hash
                })
    }

    fn authorizes_canonical_retired_predecessor(
        &self,
        ledger: &LifecycleLedgerV1,
        complete_tip: &crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ) -> bool {
        self.store_path.parent().is_some_and(|root| {
            complete_tip.authorizes_predecessor_lifecycle_root(root)
                && self.store_path == root.join(LEDGER_FILE)
                && self.directory_identity_still_exact(root)
        }) && self.binds_ledger(ledger)
            && complete_tip.authorizes_retired_lifecycle(ledger.context())
    }

    fn directory_identity_still_exact(&self, root: &Path) -> bool {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            bind_lifecycle_directory_path(root, false)
                .and_then(|(_, directory)| {
                    directory.metadata().map_err(|error| {
                        lifecycle_storage_io("inspect authenticated lifecycle root", root, error)
                    })
                })
                .is_ok_and(|metadata| {
                    LifecycleStorageIdentity::from_metadata(&metadata)
                        == self.store_directory_identity
                })
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = root;
            false
        }
    }

    fn exactly_matches(&self, store: &LifecycleLedgerStoreV1, ledger: &LifecycleLedgerV1) -> bool {
        let mut exact_target = self.store_path == store.path;
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            exact_target &= self.store_directory_identity == store.directory.identity;
        }
        if !exact_target
            || self.context != store.context
            || self.max_records != store.max_records
            || self.max_frame_bytes != store.max_frame_bytes
            || !self.binds_ledger(ledger)
        {
            return false;
        }
        store
            .load_with_frame_presence()
            .is_ok_and(|(opened, present)| present && opened == *ledger)
    }
}

const LEDGER_TEMPORARY_FILE: &str = "lifecycle-ledger-v1.norito.tmp";
const VALIDATE_SIDECAR_REGISTRATION_FILE: &str = "validate-sidecar-registration-v1.norito";
const VALIDATE_SIDECAR_REGISTRATION_TEMPORARY_FILE: &str =
    "validate-sidecar-registration-v1.norito.tmp";

/// Retained owner of the exact directory inode containing both lifecycle
/// durability leaves.
///
/// Production currently reaches this seam through an authenticated lifecycle
/// root path. The first-release Kura capability can replace `open_or_create`
/// with a consumed typed directory authority without changing any leaf I/O:
/// no operation below reopens a leaf or its parent by path.
#[derive(Debug)]
struct BoundLifecycleLedgerDirectory {
    expected_path: PathBuf,
    #[cfg(all(unix, not(target_os = "espidf")))]
    canonical_path: PathBuf,
    #[cfg(all(unix, not(target_os = "espidf")))]
    directory: File,
    #[cfg(all(unix, not(target_os = "espidf")))]
    identity: LifecycleStorageIdentity,
    operation_lock: std::sync::Mutex<()>,
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LifecycleStorageIdentity {
    device: u64,
    inode: u64,
}

#[cfg(all(unix, not(target_os = "espidf")))]
impl LifecycleStorageIdentity {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt as _;

        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev as u64,
            inode: stat.st_ino as u64,
        }
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[derive(Clone, Copy, Debug)]
struct BoundLifecycleStorageLeaf {
    identity: LifecycleStorageIdentity,
    length: u64,
}

#[cfg(all(unix, not(target_os = "espidf")))]
struct LifecycleDirectoryOperationGuard<'directory> {
    directory: &'directory BoundLifecycleLedgerDirectory,
    _thread_lock: std::sync::MutexGuard<'directory, ()>,
}

#[cfg(all(unix, not(target_os = "espidf")))]
impl Drop for LifecycleDirectoryOperationGuard<'_> {
    fn drop(&mut self) {
        #[cfg(not(any(
            target_os = "horizon",
            target_os = "solaris",
            target_os = "vita",
            target_os = "wasi"
        )))]
        let _ = rustix::fs::flock(
            &self.directory.directory,
            rustix::fs::FlockOperation::Unlock,
        );
    }
}

impl BoundLifecycleLedgerDirectory {
    fn open_or_create(path: &Path) -> Result<Self, LifecycleLedgerError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            let (canonical_path, directory) = bind_lifecycle_directory_path(path, true)?;
            let metadata = directory.metadata().map_err(|error| {
                lifecycle_storage_io("inspect opened lifecycle directory", path, error)
            })?;
            validate_lifecycle_directory_metadata(&metadata, path)?;
            Ok(Self {
                expected_path: path.to_path_buf(),
                canonical_path,
                identity: LifecycleStorageIdentity::from_metadata(&metadata),
                directory,
                operation_lock: std::sync::Mutex::new(()),
            })
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            Err(LifecycleLedgerError::Io(format!(
                "descriptor-relative lifecycle storage is unsupported at {}",
                path.display()
            )))
        }
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn same_directory(&self, other: &Self) -> bool {
        self.identity == other.identity
    }

    #[cfg(not(all(unix, not(target_os = "espidf"))))]
    fn same_directory(&self, _other: &Self) -> bool {
        false
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn verify_linked(&self) -> Result<(), LifecycleLedgerError> {
        let retained = self.directory.metadata().map_err(|error| {
            lifecycle_storage_io(
                "inspect retained lifecycle directory",
                &self.expected_path,
                error,
            )
        })?;
        validate_lifecycle_directory_metadata(&retained, &self.expected_path)?;
        if LifecycleStorageIdentity::from_metadata(&retained) != self.identity {
            return Err(lifecycle_invalid_storage(
                "retained lifecycle directory identity changed",
            ));
        }
        let (canonical_path, linked) = bind_lifecycle_directory_path(&self.expected_path, false)?;
        if canonical_path != self.canonical_path {
            return Err(lifecycle_invalid_storage(
                "lifecycle directory ancestry resolves to a different canonical target",
            ));
        }
        let linked_metadata = linked.metadata().map_err(|error| {
            lifecycle_storage_io(
                "inspect linked lifecycle directory",
                &self.expected_path,
                error,
            )
        })?;
        validate_lifecycle_directory_metadata(&linked_metadata, &self.expected_path)?;
        if LifecycleStorageIdentity::from_metadata(&linked_metadata) != self.identity {
            return Err(lifecycle_invalid_storage(
                "lifecycle directory ancestry no longer names the retained inode",
            ));
        }
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn lock(&self) -> Result<LifecycleDirectoryOperationGuard<'_>, LifecycleLedgerError> {
        let thread_lock = self.operation_lock.lock().map_err(|_| {
            LifecycleLedgerError::Io("lifecycle storage operation lock was poisoned".to_owned())
        })?;
        #[cfg(not(any(
            target_os = "horizon",
            target_os = "solaris",
            target_os = "vita",
            target_os = "wasi"
        )))]
        rustix::fs::flock(&self.directory, rustix::fs::FlockOperation::LockExclusive)
            .map_err(std::io::Error::from)
            .map_err(|error| {
                lifecycle_storage_io("lock lifecycle directory", &self.expected_path, error)
            })?;
        #[cfg(any(
            target_os = "horizon",
            target_os = "solaris",
            target_os = "vita",
            target_os = "wasi"
        ))]
        return Err(LifecycleLedgerError::Io(format!(
            "exclusive lifecycle storage locking is unsupported at {}",
            self.expected_path.display()
        )));
        let guard = LifecycleDirectoryOperationGuard {
            directory: self,
            _thread_lock: thread_lock,
        };
        guard.directory.verify_linked()?;
        Ok(guard)
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn inspect_leaf(
        &self,
        name: &str,
        maximum: u64,
    ) -> Result<Option<BoundLifecycleStorageLeaf>, LifecycleLedgerError> {
        let path = self.expected_path.join(name);
        let stat = match rustix::fs::statat(
            &self.directory,
            name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(stat) => stat,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => {
                return Err(lifecycle_storage_io(
                    "inspect lifecycle storage leaf",
                    &path,
                    std::io::Error::from(error),
                ));
            }
        };
        if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile
            || stat.st_nlink as u64 != 1
            || stat.st_uid != rustix::process::geteuid().as_raw()
            || stat.st_size < 0
        {
            return Err(lifecycle_invalid_storage(
                "lifecycle storage leaf is not a direct owner-owned single-link regular file",
            ));
        }
        let length = u64::try_from(stat.st_size).map_err(|_| {
            lifecycle_invalid_storage("lifecycle storage leaf has an invalid length")
        })?;
        if length > maximum {
            return Err(lifecycle_invalid_storage(
                "lifecycle storage leaf exceeds its byte bound",
            ));
        }
        Ok(Some(BoundLifecycleStorageLeaf {
            identity: LifecycleStorageIdentity::from_stat(&stat),
            length,
        }))
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn verify_open_leaf(
        &self,
        file: &File,
        name: &str,
        expected: BoundLifecycleStorageLeaf,
    ) -> Result<(), LifecycleLedgerError> {
        use std::os::unix::fs::MetadataExt as _;

        let path = self.expected_path.join(name);
        let opened = file.metadata().map_err(|error| {
            lifecycle_storage_io("inspect opened lifecycle storage leaf", &path, error)
        })?;
        let linked = self.inspect_leaf(name, expected.length)?.ok_or_else(|| {
            lifecycle_invalid_storage("opened lifecycle storage leaf is no longer linked")
        })?;
        if !opened.is_file()
            || opened.nlink() != 1
            || opened.uid() != rustix::process::geteuid().as_raw()
            || opened.len() != expected.length
            || LifecycleStorageIdentity::from_metadata(&opened) != expected.identity
            || linked.identity != expected.identity
            || linked.length != expected.length
        {
            return Err(lifecycle_invalid_storage(
                "lifecycle storage leaf changed its exact open-file identity",
            ));
        }
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn open_leaf(
        &self,
        name: &str,
        leaf: BoundLifecycleStorageLeaf,
    ) -> Result<File, LifecycleLedgerError> {
        let path = self.expected_path.join(name);
        let file = File::from(
            rustix::fs::openat(
                &self.directory,
                name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .map_err(|error| lifecycle_storage_io("open lifecycle storage leaf", &path, error))?,
        );
        self.verify_open_leaf(&file, name, leaf)?;
        Ok(file)
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn read_bounded_locked(
        &self,
        name: &str,
        maximum: u64,
    ) -> Result<Option<Vec<u8>>, LifecycleLedgerError> {
        let Some(leaf) = self.inspect_leaf(name, maximum)? else {
            self.verify_linked()?;
            return Ok(None);
        };
        let mut file = self.open_leaf(name, leaf)?;
        let mut bytes = Vec::with_capacity(usize::try_from(leaf.length).unwrap_or(0));
        Read::by_ref(&mut file)
            .take(maximum.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| {
                lifecycle_storage_io(
                    "read lifecycle storage leaf",
                    &self.expected_path.join(name),
                    error,
                )
            })?;
        let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if observed != leaf.length || observed > maximum {
            return Err(lifecycle_invalid_storage(
                "lifecycle storage leaf changed during bounded read",
            ));
        }
        self.verify_open_leaf(&file, name, leaf)?;
        self.verify_linked()?;
        Ok(Some(bytes))
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn remove_stale_temporary_locked(
        &self,
        name: &str,
        maximum: u64,
    ) -> Result<(), LifecycleLedgerError> {
        let Some(leaf) = self.inspect_leaf(name, maximum)? else {
            return Ok(());
        };
        let file = self.open_leaf(name, leaf)?;
        self.verify_open_leaf(&file, name, leaf)?;
        rustix::fs::unlinkat(&self.directory, name, rustix::fs::AtFlags::empty())
            .map_err(std::io::Error::from)
            .map_err(|error| {
                lifecycle_storage_io(
                    "remove stale lifecycle temporary",
                    &self.expected_path.join(name),
                    error,
                )
            })?;
        self.sync_locked()?;
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn create_synced_temporary_locked(
        &self,
        name: &str,
        bytes: &[u8],
        maximum: u64,
    ) -> Result<(File, BoundLifecycleStorageLeaf), LifecycleLedgerError> {
        use std::os::unix::fs::MetadataExt as _;

        let length = u64::try_from(bytes.len()).map_err(|_| {
            lifecycle_invalid_storage("lifecycle temporary length is not representable")
        })?;
        if length == 0 || length > maximum {
            return Err(lifecycle_invalid_storage(
                "lifecycle temporary payload violates its byte bound",
            ));
        }
        let path = self.expected_path.join(name);
        let mut file = File::from(
            rustix::fs::openat(
                &self.directory,
                name,
                rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::CREATE
                    | rustix::fs::OFlags::EXCL
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            )
            .map_err(std::io::Error::from)
            .map_err(|error| lifecycle_storage_io("create lifecycle temporary", &path, error))?,
        );
        let created = file.metadata().map_err(|error| {
            lifecycle_storage_io("inspect created lifecycle temporary", &path, error)
        })?;
        let empty_leaf = BoundLifecycleStorageLeaf {
            identity: LifecycleStorageIdentity::from_metadata(&created),
            length: 0,
        };
        if !created.is_file()
            || created.nlink() != 1
            || created.uid() != rustix::process::geteuid().as_raw()
        {
            return Err(lifecycle_invalid_storage(
                "exclusively created lifecycle temporary has an invalid identity",
            ));
        }
        self.verify_open_leaf(&file, name, empty_leaf)?;
        file.write_all(bytes)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
            .map_err(|error| lifecycle_storage_io("sync lifecycle temporary", &path, error))?;
        let leaf = BoundLifecycleStorageLeaf {
            identity: empty_leaf.identity,
            length,
        };
        self.verify_open_leaf(&file, name, leaf)?;
        Ok((file, leaf))
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn unlink_if_identity_locked(&self, name: &str, expected: LifecycleStorageIdentity) {
        if self
            .inspect_leaf(name, u64::MAX)
            .is_ok_and(|leaf| leaf.is_some_and(|leaf| leaf.identity == expected))
        {
            let _ = rustix::fs::unlinkat(&self.directory, name, rustix::fs::AtFlags::empty());
            let _ = self.directory.sync_all();
        }
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn publish_noreplace_locked(
        &self,
        temporary: &str,
        destination: &str,
        bytes: &[u8],
        maximum: u64,
    ) -> Result<bool, LifecycleLedgerError> {
        self.remove_stale_temporary_locked(temporary, maximum)?;
        if self.inspect_leaf(destination, maximum)?.is_some() {
            return Ok(false);
        }
        let (file, leaf) = self.create_synced_temporary_locked(temporary, bytes, maximum)?;
        let publication = (|| {
            self.verify_linked()?;
            match rename_lifecycle_leaf_noreplace(&self.directory, temporary, destination) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => return Ok(false),
                Err(error) => {
                    return Err(lifecycle_storage_io(
                        "publish lifecycle storage leaf",
                        &self.expected_path.join(destination),
                        error,
                    ));
                }
            }
            self.verify_open_leaf(&file, destination, leaf)?;
            self.sync_locked()?;
            self.verify_open_leaf(&file, destination, leaf)?;
            Ok(true)
        })();
        if publication.as_ref().is_err() || matches!(publication.as_ref(), Ok(false)) {
            self.unlink_if_identity_locked(temporary, leaf.identity);
        }
        publication
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn publish_replace_locked(
        &self,
        temporary: &str,
        destination: &str,
        bytes: &[u8],
        maximum: u64,
    ) -> Result<(), LifecycleLedgerError> {
        self.remove_stale_temporary_locked(temporary, maximum)?;
        let destination_existed = self.inspect_leaf(destination, maximum)?.is_some();
        let (file, leaf) = self.create_synced_temporary_locked(temporary, bytes, maximum)?;
        let publication = (|| {
            self.verify_linked()?;
            if destination_existed {
                rustix::fs::renameat(&self.directory, temporary, &self.directory, destination)
                    .map_err(std::io::Error::from)
                    .map_err(|error| {
                        lifecycle_storage_io(
                            "replace lifecycle storage leaf",
                            &self.expected_path.join(destination),
                            error,
                        )
                    })?;
            } else {
                rename_lifecycle_leaf_noreplace(&self.directory, temporary, destination).map_err(
                    |error| {
                        lifecycle_storage_io(
                            "publish initial lifecycle storage leaf",
                            &self.expected_path.join(destination),
                            error,
                        )
                    },
                )?;
            }
            self.verify_open_leaf(&file, destination, leaf)?;
            self.sync_locked()?;
            self.verify_open_leaf(&file, destination, leaf)
        })();
        if publication.is_err() {
            self.unlink_if_identity_locked(temporary, leaf.identity);
        }
        publication
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn remove_exact_locked(&self, name: &str, maximum: u64) -> Result<(), LifecycleLedgerError> {
        let leaf = self.inspect_leaf(name, maximum)?.ok_or_else(|| {
            lifecycle_invalid_storage("lifecycle storage leaf disappeared before retirement")
        })?;
        let file = self.open_leaf(name, leaf)?;
        self.verify_open_leaf(&file, name, leaf)?;
        rustix::fs::unlinkat(&self.directory, name, rustix::fs::AtFlags::empty())
            .map_err(std::io::Error::from)
            .map_err(|error| {
                lifecycle_storage_io(
                    "retire lifecycle storage leaf",
                    &self.expected_path.join(name),
                    error,
                )
            })?;
        self.sync_locked()
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn sync_locked(&self) -> Result<(), LifecycleLedgerError> {
        self.verify_linked()?;
        self.directory.sync_all().map_err(|error| {
            lifecycle_storage_io(
                "sync lifecycle storage directory",
                &self.expected_path,
                error,
            )
        })?;
        self.verify_linked()
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
fn bind_lifecycle_directory_path(
    path: &Path,
    create_missing: bool,
) -> Result<(PathBuf, File), LifecycleLedgerError> {
    if path.as_os_str().is_empty() {
        return Err(lifecycle_invalid_storage(
            "lifecycle directory path is empty",
        ));
    }
    match fs::symlink_metadata(path) {
        Ok(lexical) => {
            if lexical.file_type().is_symlink() || !lexical.is_dir() {
                return Err(lifecycle_invalid_storage(
                    "lifecycle storage root is a symlink or non-directory",
                ));
            }
            let canonical = fs::canonicalize(path).map_err(|error| {
                lifecycle_storage_io("canonicalize lifecycle directory", path, error)
            })?;
            let directory = open_canonical_lifecycle_directory_ancestry(&canonical)?;
            let opened = directory.metadata().map_err(|error| {
                lifecycle_storage_io("inspect bound lifecycle directory", path, error)
            })?;
            validate_lifecycle_directory_metadata(&opened, path)?;
            if LifecycleStorageIdentity::from_metadata(&lexical)
                != LifecycleStorageIdentity::from_metadata(&opened)
            {
                return Err(lifecycle_invalid_storage(
                    "lifecycle storage root changed while binding its canonical directory",
                ));
            }
            validate_lifecycle_directory_ownership(&opened)?;
            Ok((canonical, directory))
        }
        Err(error) if error.kind() == ErrorKind::NotFound && create_missing => {
            let mut missing = Vec::new();
            let mut ancestor = path.to_path_buf();
            let (mut canonical, mut directory) = loop {
                match fs::symlink_metadata(&ancestor) {
                    Ok(lexical) => {
                        if lexical.file_type().is_symlink() || !lexical.is_dir() {
                            return Err(lifecycle_invalid_storage(
                                "nearest lifecycle storage ancestor is a symlink or non-directory",
                            ));
                        }
                        let canonical = fs::canonicalize(&ancestor).map_err(|error| {
                            lifecycle_storage_io(
                                "canonicalize lifecycle directory ancestor",
                                &ancestor,
                                error,
                            )
                        })?;
                        let directory = open_canonical_lifecycle_directory_ancestry(&canonical)?;
                        let opened = directory.metadata().map_err(|error| {
                            lifecycle_storage_io(
                                "inspect lifecycle directory ancestor",
                                &ancestor,
                                error,
                            )
                        })?;
                        if LifecycleStorageIdentity::from_metadata(&lexical)
                            != LifecycleStorageIdentity::from_metadata(&opened)
                        {
                            return Err(lifecycle_invalid_storage(
                                "lifecycle storage ancestor changed while binding",
                            ));
                        }
                        break (canonical, directory);
                    }
                    Err(error) if error.kind() == ErrorKind::NotFound => {
                        let name = ancestor.file_name().ok_or_else(|| {
                            lifecycle_invalid_storage(
                                "missing lifecycle directory has no direct component name",
                            )
                        })?;
                        missing.push(name.to_os_string());
                        ancestor = ancestor
                            .parent()
                            .filter(|parent| !parent.as_os_str().is_empty())
                            .unwrap_or_else(|| Path::new("."))
                            .to_path_buf();
                    }
                    Err(error) => {
                        return Err(lifecycle_storage_io(
                            "inspect lifecycle directory ancestor",
                            &ancestor,
                            error,
                        ));
                    }
                }
            };
            for name in missing.iter().rev() {
                canonical.push(name);
                let mut created = false;
                let before = match rustix::fs::statat(
                    &directory,
                    name,
                    rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                ) {
                    Ok(stat) => stat,
                    Err(rustix::io::Errno::NOENT) => {
                        match rustix::fs::mkdirat(&directory, name, rustix::fs::Mode::RWXU) {
                            Ok(()) => created = true,
                            Err(rustix::io::Errno::EXIST) => {}
                            Err(error) => {
                                return Err(lifecycle_storage_io(
                                    "create lifecycle directory component",
                                    &canonical,
                                    std::io::Error::from(error),
                                ));
                            }
                        }
                        rustix::fs::statat(&directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                            .map_err(std::io::Error::from)
                            .map_err(|error| {
                                lifecycle_storage_io(
                                    "inspect created lifecycle directory component",
                                    &canonical,
                                    error,
                                )
                            })?
                    }
                    Err(error) => {
                        return Err(lifecycle_storage_io(
                            "inspect lifecycle directory component",
                            &canonical,
                            std::io::Error::from(error),
                        ));
                    }
                };
                if rustix::fs::FileType::from_raw_mode(before.st_mode)
                    != rustix::fs::FileType::Directory
                {
                    return Err(lifecycle_invalid_storage(
                        "created lifecycle ancestry contains a symlink or non-directory",
                    ));
                }
                let child = File::from(
                    rustix::fs::openat(
                        &directory,
                        name,
                        rustix::fs::OFlags::RDONLY
                            | rustix::fs::OFlags::DIRECTORY
                            | rustix::fs::OFlags::NOFOLLOW
                            | rustix::fs::OFlags::CLOEXEC,
                        rustix::fs::Mode::empty(),
                    )
                    .map_err(std::io::Error::from)
                    .map_err(|error| {
                        lifecycle_storage_io(
                            "open created lifecycle directory component",
                            &canonical,
                            error,
                        )
                    })?,
                );
                let opened = child.metadata().map_err(|error| {
                    lifecycle_storage_io(
                        "inspect opened lifecycle directory component",
                        &canonical,
                        error,
                    )
                })?;
                let after =
                    rustix::fs::statat(&directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                        .map_err(std::io::Error::from)
                        .map_err(|error| {
                            lifecycle_storage_io(
                                "reinspect lifecycle directory component",
                                &canonical,
                                error,
                            )
                        })?;
                let opened_identity = LifecycleStorageIdentity::from_metadata(&opened);
                if !opened.is_dir()
                    || LifecycleStorageIdentity::from_stat(&before) != opened_identity
                    || LifecycleStorageIdentity::from_stat(&after) != opened_identity
                {
                    return Err(lifecycle_invalid_storage(
                        "lifecycle directory changed during descriptor-relative creation",
                    ));
                }
                if created {
                    child.sync_all().map_err(|error| {
                        lifecycle_storage_io("sync created lifecycle directory", &canonical, error)
                    })?;
                    directory.sync_all().map_err(|error| {
                        lifecycle_storage_io(
                            "sync parent of created lifecycle directory",
                            &canonical,
                            error,
                        )
                    })?;
                }
                directory = child;
            }
            let lexical = fs::symlink_metadata(path).map_err(|error| {
                lifecycle_storage_io("inspect created lifecycle directory", path, error)
            })?;
            let opened = directory.metadata().map_err(|error| {
                lifecycle_storage_io("inspect bound created lifecycle directory", path, error)
            })?;
            if lexical.file_type().is_symlink()
                || !lexical.is_dir()
                || LifecycleStorageIdentity::from_metadata(&lexical)
                    != LifecycleStorageIdentity::from_metadata(&opened)
            {
                return Err(lifecycle_invalid_storage(
                    "created lifecycle storage root changed before binding",
                ));
            }
            validate_lifecycle_directory_ownership(&opened)?;
            Ok((canonical, directory))
        }
        Err(error) => Err(lifecycle_storage_io(
            "inspect lifecycle storage root",
            path,
            error,
        )),
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
fn open_canonical_lifecycle_directory_ancestry(
    canonical: &Path,
) -> Result<File, LifecycleLedgerError> {
    use std::path::Component;

    if !canonical.is_absolute() {
        return Err(lifecycle_invalid_storage(
            "canonical lifecycle directory path is not absolute",
        ));
    }
    let mut current = File::from(
        rustix::fs::open(
            "/",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(std::io::Error::from)
        .map_err(|error| {
            lifecycle_storage_io("open lifecycle directory anchor", Path::new("/"), error)
        })?,
    );
    let mut traversed = PathBuf::from("/");
    for component in canonical.components() {
        let name = match component {
            Component::RootDir | Component::CurDir => continue,
            Component::Normal(name) => name,
            Component::ParentDir | Component::Prefix(_) => {
                return Err(lifecycle_invalid_storage(
                    "canonical lifecycle path contains an invalid traversal component",
                ));
            }
        };
        traversed.push(name);
        let before = rustix::fs::statat(&current, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)
            .map_err(|error| {
                lifecycle_storage_io("inspect canonical lifecycle ancestry", &traversed, error)
            })?;
        if rustix::fs::FileType::from_raw_mode(before.st_mode) != rustix::fs::FileType::Directory {
            return Err(lifecycle_invalid_storage(
                "canonical lifecycle ancestry contains a symlink or non-directory",
            ));
        }
        let child = File::from(
            rustix::fs::openat(
                &current,
                name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .map_err(|error| {
                lifecycle_storage_io("open canonical lifecycle ancestry", &traversed, error)
            })?,
        );
        let opened = child.metadata().map_err(|error| {
            lifecycle_storage_io("inspect opened canonical ancestry", &traversed, error)
        })?;
        let after = rustix::fs::statat(&current, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)
            .map_err(|error| {
                lifecycle_storage_io("reinspect canonical lifecycle ancestry", &traversed, error)
            })?;
        let opened_identity = LifecycleStorageIdentity::from_metadata(&opened);
        if !opened.is_dir()
            || LifecycleStorageIdentity::from_stat(&before) != opened_identity
            || LifecycleStorageIdentity::from_stat(&after) != opened_identity
        {
            return Err(lifecycle_invalid_storage(
                "canonical lifecycle ancestry changed during no-follow traversal",
            ));
        }
        current = child;
    }
    Ok(current)
}

#[cfg(all(unix, not(target_os = "espidf")))]
fn validate_lifecycle_directory_ownership(
    metadata: &std::fs::Metadata,
) -> Result<(), LifecycleLedgerError> {
    use std::os::unix::fs::MetadataExt as _;

    if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o022 != 0 {
        return Err(lifecycle_invalid_storage(
            "lifecycle directory must be owner-owned and not group/world writable",
        ));
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "espidf")))]
fn validate_lifecycle_directory_metadata(
    metadata: &std::fs::Metadata,
    _path: &Path,
) -> Result<(), LifecycleLedgerError> {
    if !metadata.is_dir() {
        return Err(lifecycle_invalid_storage(
            "lifecycle storage root is not a direct directory",
        ));
    }
    Ok(())
}

#[cfg(all(
    unix,
    not(target_os = "espidf"),
    any(
        target_vendor = "apple",
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    )
))]
fn rename_lifecycle_leaf_noreplace(
    directory: &File,
    source: &str,
    destination: &str,
) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        directory,
        source,
        directory,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

#[cfg(all(
    unix,
    not(target_os = "espidf"),
    not(any(
        target_vendor = "apple",
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    ))
))]
fn rename_lifecycle_leaf_noreplace(
    _directory: &File,
    _source: &str,
    _destination: &str,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "atomic no-replace lifecycle publication is unavailable",
    ))
}

fn lifecycle_storage_io(
    operation: &str,
    path: &Path,
    error: std::io::Error,
) -> LifecycleLedgerError {
    LifecycleLedgerError::Io(format!("{operation} {}: {error}", path.display()))
}

fn lifecycle_invalid_storage(reason: &str) -> LifecycleLedgerError {
    LifecycleLedgerError::InvalidFrame(reason.to_owned())
}

/// Crash-safe, bounded store for one height-local LifecycleLedgerV1.
#[derive(Clone, Debug)]
pub(in crate::sumeragi) struct LifecycleLedgerStoreV1 {
    path: PathBuf,
    directory: std::sync::Arc<BoundLifecycleLedgerDirectory>,
    context: LifecycleContext,
    max_records: usize,
    max_frame_bytes: u64,
    #[cfg(test)]
    fail_persistence_for_test: bool,
}
impl LifecycleLedgerStoreV1 {
    /// Return the private sibling path reserved for the one lifecycle-owned
    /// Validate merge-sidecar registration at this height.
    #[cfg(test)]
    pub(super) fn validate_sidecar_registration_path(
        &self,
    ) -> Result<PathBuf, LifecycleLedgerError> {
        self.path
            .parent()
            .map(|root| root.join("validate-sidecar-registration-v1.norito"))
            .ok_or_else(|| {
                LifecycleLedgerError::Io(
                    "lifecycle ledger has no parent for Validate sidecar registration".to_owned(),
                )
            })
    }

    /// Return the immutable context sealed into this exact store handle.
    pub(super) const fn lifecycle_context(&self) -> LifecycleContext {
        self.context
    }

    /// Load the exact bounded Validate sidecar registration through this
    /// store's retained directory owner.
    pub(super) fn load_validate_sidecar_registration_bytes(
        &self,
        maximum: u64,
    ) -> Result<Option<Vec<u8>>, LifecycleLedgerError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.fail_persistence_if_injected()?;
            let guard = self.directory.lock()?;
            guard.directory.remove_stale_temporary_locked(
                VALIDATE_SIDECAR_REGISTRATION_TEMPORARY_FILE,
                maximum,
            )?;
            guard
                .directory
                .read_bounded_locked(VALIDATE_SIDECAR_REGISTRATION_FILE, maximum)
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = maximum;
            Err(LifecycleLedgerError::Io(format!(
                "descriptor-relative lifecycle sidecar storage is unsupported at {}",
                self.path.display()
            )))
        }
    }

    /// Publish one Validate sidecar registration without ever replacing an
    /// incumbent. `Some(bytes)` is the complete incumbent observed under the
    /// same directory lock; `None` means `bytes` was newly fsynced.
    pub(super) fn publish_validate_sidecar_registration_bytes(
        &self,
        bytes: &[u8],
        maximum: u64,
    ) -> Result<Option<Vec<u8>>, LifecycleLedgerError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.fail_persistence_if_injected()?;
            let guard = self.directory.lock()?;
            guard.directory.remove_stale_temporary_locked(
                VALIDATE_SIDECAR_REGISTRATION_TEMPORARY_FILE,
                maximum,
            )?;
            if let Some(existing) = guard
                .directory
                .read_bounded_locked(VALIDATE_SIDECAR_REGISTRATION_FILE, maximum)?
            {
                return Ok(Some(existing));
            }
            if guard.directory.publish_noreplace_locked(
                VALIDATE_SIDECAR_REGISTRATION_TEMPORARY_FILE,
                VALIDATE_SIDECAR_REGISTRATION_FILE,
                bytes,
                maximum,
            )? {
                return Ok(None);
            }
            guard
                .directory
                .read_bounded_locked(VALIDATE_SIDECAR_REGISTRATION_FILE, maximum)?
                .map(Some)
                .ok_or_else(|| {
                    lifecycle_invalid_storage(
                        "sidecar destination appeared during no-replace publication but vanished before exact read",
                    )
                })
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (bytes, maximum);
            Err(LifecycleLedgerError::Io(format!(
                "descriptor-relative lifecycle sidecar storage is unsupported at {}",
                self.path.display()
            )))
        }
    }

    /// Remove exactly the expected Validate sidecar registration and fsync
    /// the retained directory before returning.
    pub(super) fn clear_validate_sidecar_registration_bytes(
        &self,
        expected: &[u8],
        maximum: u64,
    ) -> Result<(), LifecycleLedgerError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.fail_persistence_if_injected()?;
            let guard = self.directory.lock()?;
            guard.directory.remove_stale_temporary_locked(
                VALIDATE_SIDECAR_REGISTRATION_TEMPORARY_FILE,
                maximum,
            )?;
            let observed = guard
                .directory
                .read_bounded_locked(VALIDATE_SIDECAR_REGISTRATION_FILE, maximum)?
                .ok_or_else(|| {
                    lifecycle_invalid_storage(
                        "Validate sidecar registration disappeared before exact retirement",
                    )
                })?;
            if observed != expected {
                return Err(lifecycle_invalid_storage(
                    "Validate sidecar retirement does not match the incumbent bytes",
                ));
            }
            guard
                .directory
                .remove_exact_locked(VALIDATE_SIDECAR_REGISTRATION_FILE, maximum)
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (expected, maximum);
            Err(LifecycleLedgerError::Io(format!(
                "descriptor-relative lifecycle sidecar storage is unsupported at {}",
                self.path.display()
            )))
        }
    }

    fn fail_persistence_if_injected(&self) -> Result<(), LifecycleLedgerError> {
        #[cfg(test)]
        if self.fail_persistence_for_test {
            return Err(LifecycleLedgerError::Io(
                "injected lifecycle storage persistence failure".to_owned(),
            ));
        }
        Ok(())
    }

    fn is_authorized_complete_tip_predecessor_target(
        &self,
        complete_tip: &crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ) -> bool {
        self.path.parent().is_some_and(|root| {
            complete_tip.authorizes_predecessor_lifecycle_root(root)
                && self.path == root.join(LEDGER_FILE)
        })
    }
    /// Compare the complete immutable publication target of two open handles.
    pub(super) fn same_publication_target(&self, other: &Self) -> bool {
        self.path == other.path
            && self.directory.same_directory(&other.directory)
            && self.context == other.context
            && self.max_records == other.max_records
            && self.max_frame_bytes == other.max_frame_bytes
    }
    /// Publish one exact timeout-supersession owner-open successor and mint its join proof.
    ///
    /// Keeping the staged proof, compare-and-swap, reload, and authenticated
    /// witness mint inside one private store method prevents callers from
    /// manufacturing the CompleteTip exception after an unrelated overwrite.
    #[allow(clippy::too_many_arguments)]
    fn persist_recovered_timeout_supersession_successor(
        &self,
        staged: StagedRecoveredTimeoutSupersessionSuccessorV1,
        opened: &LifecycleLedgerV1,
        reconciled: &LifecycleLedgerV1,
        successor: &LifecycleLedgerV1,
        projection: &AuthenticatedRecoveredWalControlProjection,
        control_ordinal: u128,
    ) -> Result<AuthenticatedRecoveredTimeoutSupersessionSuccessorV1, LifecycleLedgerError> {
        if !staged.exactly_matches_successor(
            self,
            opened,
            reconciled,
            successor,
            projection,
            control_ordinal,
        ) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "timeout supersession successor changed before exact publication".to_owned(),
            ));
        }
        self.persist_exact_successor(opened, successor)?;
        if self.load()? != *successor {
            return Err(LifecycleLedgerError::InvalidLedger(
                "timeout supersession successor changed after exact publication".to_owned(),
            ));
        }
        Ok(staged.into_authenticated(self, successor))
    }
    /// Open a height-local ledger under the coordinator's sealed size bounds.
    pub(in crate::sumeragi) fn open(
        root: &Path,
        context: LifecycleContext,
    ) -> Result<(Self, LifecycleLedgerV1), LifecycleLedgerError> {
        let directory = std::sync::Arc::new(BoundLifecycleLedgerDirectory::open_or_create(root)?);
        let store = Self {
            path: root.join(LEDGER_FILE),
            directory,
            context,
            max_records: MAX_LIFECYCLE_RECORDS_PER_HEIGHT,
            max_frame_bytes: MAX_LEDGER_FRAME_BYTES,
            #[cfg(test)]
            fail_persistence_for_test: false,
        };
        let ledger = store.load()?;
        Ok((store, ledger))
    }
    pub(super) fn load(&self) -> Result<LifecycleLedgerV1, LifecycleLedgerError> {
        self.load_with_frame_presence().map(|(ledger, _)| ledger)
    }
    fn load_with_frame_presence(&self) -> Result<(LifecycleLedgerV1, bool), LifecycleLedgerError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.fail_persistence_if_injected()?;
            let guard = self.directory.lock()?;
            self.load_with_frame_presence_locked(&guard)
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            Err(LifecycleLedgerError::Io(format!(
                "descriptor-relative lifecycle storage is unsupported at {}",
                self.path.display()
            )))
        }
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn load_with_frame_presence_locked(
        &self,
        guard: &LifecycleDirectoryOperationGuard<'_>,
    ) -> Result<(LifecycleLedgerV1, bool), LifecycleLedgerError> {
        guard
            .directory
            .remove_stale_temporary_locked(LEDGER_TEMPORARY_FILE, self.max_frame_bytes)?;
        let Some(bytes) = guard
            .directory
            .read_bounded_locked(LEDGER_FILE, self.max_frame_bytes)?
        else {
            return Ok((LifecycleLedgerV1::empty(self.context), false));
        };
        let ledger = decode_frame(&bytes, self.max_frame_bytes)?;
        if ledger.context() != self.context {
            return Err(LifecycleLedgerError::InvalidLedger(
                "ledger belongs to another height context".to_owned(),
            ));
        }
        ledger.validate(self.max_records)?;
        Ok((ledger, true))
    }
    /// Authenticate one exact physically present canonical frame.
    ///
    /// Returning `None` for the ordinary missing-path empty fallback is the
    /// security boundary used by non-genesis CompleteTip recovery.
    fn authenticate_present_frame(
        &self,
        expected: &LifecycleLedgerV1,
    ) -> Result<Option<AuthenticatedPresentLifecycleFrameV1>, LifecycleLedgerError> {
        let (opened, present) = self.load_with_frame_presence()?;
        if opened != *expected {
            return Err(LifecycleLedgerError::InvalidLedger(
                "lifecycle frame changed before physical-presence authentication".to_owned(),
            ));
        }
        if !present {
            return Ok(None);
        }
        let frame = encode_frame(&opened, self.max_frame_bytes)?;
        Ok(Some(AuthenticatedPresentLifecycleFrameV1 {
            store_path: self.path.clone(),
            #[cfg(all(unix, not(target_os = "espidf")))]
            store_directory_identity: self.directory.identity,
            context: self.context,
            max_records: self.max_records,
            max_frame_bytes: self.max_frame_bytes,
            ledger_frame_hash: LifecycleDigest::new(Hash::new(frame).into()),
        }))
    }
    /// Persist one exact staged successor only while the attached frame still
    /// equals the coordinator state from which it was derived.
    ///
    /// The equality read happens before any atomic replacement begins. An
    /// exact stutter confirms an already-present fsynced frame without rewriting
    /// it. When the logical empty frame has not yet been published, even an exact
    /// stutter writes it durably. Otherwise a successful return means `successor`
    /// is the exact fsynced V1 frame replacing `current`. Ordinary callers may
    /// perform only infallible in-memory publication afterward. A specialized
    /// fail-stop wrapper may immediately reload the exact frame to mint a sealed
    /// receipt; any reload failure consumes startup and publishes no live owner.
    pub(super) fn persist_exact_successor(
        &self,
        current: &LifecycleLedgerV1,
        successor: &LifecycleLedgerV1,
    ) -> Result<(), LifecycleLedgerError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.fail_persistence_if_injected()?;
            let guard = self.directory.lock()?;
            let (loaded, frame_present) = self.load_with_frame_presence_locked(&guard)?;
            if loaded != *current {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "attached lifecycle ledger changed before successor publication".to_owned(),
                ));
            }
            if current == successor && frame_present {
                return Ok(());
            }
            self.persist_locked(&guard, successor)
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (current, successor);
            Err(LifecycleLedgerError::Io(format!(
                "descriptor-relative lifecycle storage is unsupported at {}",
                self.path.display()
            )))
        }
    }
    /// Persist one ordinary coordinator successor without rewriting a sealed
    /// Validate/no-successor tombstone.
    ///
    /// Specialized recovery and all-row retirement use their own authenticated
    /// publication corridors. Ordinary live publication is append-only with
    /// respect to an `AdvancedNoSuccessor` record: the complete encoded record,
    /// including its owner, replay authority, payload, and continuation, must
    /// remain byte-for-byte unchanged in every later frame.
    fn persist_exact_ordinary_successor(
        &self,
        current: &LifecycleLedgerV1,
        successor: &LifecycleLedgerV1,
    ) -> Result<(), LifecycleLedgerError> {
        let preserves_terminal_validate_tombstones = current.records().iter().all(|record| {
            if record.work_class() != Some(LifecycleWorkClass::Validate)
                || record.terminal() != Some(Some(TerminalOutcome::Advanced))
                || record.continuation() != Some(DurableContinuation::AdvancedNoSuccessor)
            {
                return true;
            }
            successor
                .records()
                .binary_search_by_key(&record.ordinal(), LifecycleLedgerRecordV1::ordinal)
                .ok()
                .and_then(|index| successor.records().get(index))
                == Some(record)
        });
        if !preserves_terminal_validate_tombstones {
            return Err(LifecycleLedgerError::InvalidLedger(
                "ordinary lifecycle successor rewrote a terminal Validate/no-successor tombstone"
                    .to_owned(),
            ));
        }
        self.persist_exact_successor(current, successor)
    }
    /// Reload and authenticate one already-fsynced WAL repair as an exact
    /// repaired-pair stutter.
    ///
    /// This is a read-only post-fsync/install preflight. It deliberately does
    /// not expose the loaded ledger: callers learn only whether the complete
    /// current frame contains the exact authenticated parent/child pair and
    /// durable child ordinal they already own.
    pub(super) fn revalidates_durable_authenticated_wal_vote_repair(
        &self,
        durable: &DurableAuthenticatedWalVoteLifecycleRepair,
    ) -> bool {
        let Ok(loaded) = self.load() else {
            return false;
        };
        if !durable.belongs_to_loaded(self, &loaded) {
            return false;
        }
        let Ok((staged, observed_child_ordinal, changed)) =
            loaded.stage_authenticated_wal_vote_repair(durable.repair())
        else {
            return false;
        };
        !changed && observed_child_ordinal == durable.child_ordinal() && staged == loaded
    }
    /// Reopen and compare the complete exact control-Sign row without exposing it.
    pub(super) fn revalidates_authenticated_wal_control_sign(
        &self,
        projection: &AuthenticatedRecoveredWalControlProjection,
        ordinal: u128,
    ) -> bool {
        let Ok(loaded) = self.load() else {
            return false;
        };
        let Ok((staged, observed_ordinal, changed)) =
            loaded.stage_authenticated_wal_control_sign(projection)
        else {
            return false;
        };
        !changed
            && observed_ordinal == ordinal
            && staged == loaded
            && projection.exactly_matches_ledger_at(&loaded, ordinal)
    }
    /// Reopen and authenticate one Advanced control Sign with its live Broadcast.
    pub(super) fn revalidates_recovered_control_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        control: &AuthenticatedRecoveredWalControlProjection,
        broadcast: &super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
        parent_ordinal: u128,
        child_ordinal: u128,
    ) -> bool {
        let Ok(loaded) = self.load() else {
            return false;
        };
        loaded
            .authenticate_recovered_control_signed_broadcast(verified, control)
            .is_ok_and(|(recovered, parent, child)| {
                parent == parent_ordinal
                    && child == child_ordinal
                    && recovered.exactly_matches(broadcast)
            })
    }
    /// Reload and reauthenticate one control-owned Broadcast-plus-Sign pair.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn revalidates_recovered_control_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        control: &AuthenticatedRecoveredWalControlProjection,
        combined: &RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        expected: &RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    ) -> bool {
        self.load().is_ok_and(|loaded| {
            loaded
                .authenticate_recovered_control_signed_broadcast_and_sign(
                    verified, control, combined,
                )
                .is_ok_and(|observed| observed == *expected)
        })
    }
    /// Reload and reauthenticate one phase-owned Broadcast-plus-Sign pair.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn revalidates_recovered_phase_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        repair: &DurableAuthenticatedWalVoteLifecycleRepair,
        combined: &RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        expected: &RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    ) -> bool {
        self.load().is_ok_and(|loaded| {
            loaded
                .authenticate_recovered_phase_signed_broadcast_and_sign(verified, repair, combined)
                .is_ok_and(|observed| observed == *expected)
        })
    }
    /// Reopen and compare one already-fsynced Decision Fetch row.
    pub(super) fn revalidates_authenticated_wal_decision_fetch(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
        ordinal: u128,
    ) -> bool {
        let Ok(loaded) = self.load() else {
            return false;
        };
        let Ok((staged, observed_ordinal, changed)) =
            loaded.stage_authenticated_wal_decision_fetch(projection)
        else {
            return false;
        };
        !changed
            && observed_ordinal == ordinal
            && staged == loaded
            && projection.exactly_matches_ledger_at(&loaded, ordinal)
    }
    /// Reopen and compare one already-fsynced advanced Fetch plus live Store cut.
    pub(super) fn revalidates_recovered_decision_fetch_store(
        &self,
        fetch: &AuthenticatedRecoveredWalDecisionFetchProjection,
        fetch_ordinal: u128,
        store: &RecoveredDecisionFetchStoreProjectionV1,
    ) -> bool {
        self.load().is_ok_and(|loaded| {
            loaded
                .authenticate_recovered_decision_fetch_store(fetch, store)
                .is_ok_and(|(observed_fetch, _)| observed_fetch == fetch_ordinal)
        })
    }
    /// Atomically replace the ledger after validating all durable invariants.
    #[cfg(test)]
    pub(super) fn persist(&self, ledger: &LifecycleLedgerV1) -> Result<(), LifecycleLedgerError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.fail_persistence_if_injected()?;
            let guard = self.directory.lock()?;
            self.persist_locked(&guard, ledger)
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = ledger;
            Err(LifecycleLedgerError::Io(format!(
                "descriptor-relative lifecycle storage is unsupported at {}",
                self.path.display()
            )))
        }
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn persist_locked(
        &self,
        guard: &LifecycleDirectoryOperationGuard<'_>,
        ledger: &LifecycleLedgerV1,
    ) -> Result<(), LifecycleLedgerError> {
        if ledger.context() != self.context {
            return Err(LifecycleLedgerError::InvalidLedger(
                "cannot persist a foreign height context".to_owned(),
            ));
        }
        ledger.validate(self.max_records)?;
        let bytes = encode_frame(ledger, self.max_frame_bytes)?;
        guard.directory.publish_replace_locked(
            LEDGER_TEMPORARY_FILE,
            LEDGER_FILE,
            &bytes,
            self.max_frame_bytes,
        )
    }
    /// Stage and fsync one authenticated WAL-ahead lifecycle repair.
    ///
    /// The receipt is minted only after the complete replacement frame and
    /// owning directory are synced. Exact repeats are persisted idempotently
    /// and receive the same frame-bound receipt.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn persist_authenticated_wal_vote_repair(
        &self,
        ledger: &LifecycleLedgerV1,
        repair: AuthenticatedWalVoteLifecycleRepair,
    ) -> Result<
        Box<(
            LifecycleLedgerV1,
            DurableAuthenticatedWalVoteLifecycleRepair,
            bool,
        )>,
        Box<(LifecycleLedgerError, AuthenticatedWalVoteLifecycleRepair)>,
    > {
        let loaded = match self.load() {
            Ok(loaded) => loaded,
            Err(error) => return Err(Box::new((error, repair))),
        };
        if &loaded != ledger {
            return Err(Box::new((
                LifecycleLedgerError::InvalidLedger(
                    "WAL repair attempted to replace a stale ledger snapshot".to_owned(),
                ),
                repair,
            )));
        }
        let (staged, child_ordinal, changed) =
            match loaded.stage_authenticated_wal_vote_repair(&repair) {
                Ok(staged) => staged,
                Err(error) => return Err(Box::new((error, repair))),
            };
        let frame = match encode_frame(&staged, self.max_frame_bytes) {
            Ok(frame) => frame,
            Err(error) => return Err(Box::new((error, repair))),
        };
        if let Err(error) = self.persist_exact_successor(&loaded, &staged) {
            return Err(Box::new((error, repair)));
        }
        let receipt = DurableWalVoteLedgerRepairReceipt {
            store_path: self.path.clone(),
            #[cfg(all(unix, not(target_os = "espidf")))]
            store_directory_identity: self.directory.identity,
            context: self.context,
            parent_key: repair.parent().key,
            child_key: repair.child().key,
            edge: repair.edge(),
            child_ordinal,
            ledger_frame_hash: LifecycleDigest::new(Hash::new(frame).into()),
        };
        debug_assert!(receipt.belongs_to(self));
        let durable = match repair.bind_durable_ledger_receipt(receipt) {
            Ok(durable) => durable,
            Err((repair, _receipt)) => {
                return Err(Box::new((
                    LifecycleLedgerError::InvalidLedger(
                        "post-fsync WAL repair receipt did not bind its authority".to_owned(),
                    ),
                    repair,
                )));
            }
        };
        Ok(Box::new((staged, durable, changed)))
    }
    /// Bind an already-persisted Validate→Sign repair beneath a live Broadcast.
    ///
    /// This is a read-only crash-recovery counterpart to the repair fsync
    /// method. It mints the same frame-bound durable repair receipt only when
    /// the current canonical store contains the exact Advanced
    /// Validate→Advanced Sign→live Broadcast lineage. No ledger bytes are
    /// rewritten and no volatile dispatch identity is reconstructed.
    #[allow(clippy::result_large_err)]
    pub(super) fn authenticate_wal_vote_repair_for_signed_broadcast(
        &self,
        ledger: &LifecycleLedgerV1,
        repair: AuthenticatedWalVoteLifecycleRepair,
    ) -> Result<
        DurableAuthenticatedWalVoteLifecycleRepair,
        (LifecycleLedgerError, AuthenticatedWalVoteLifecycleRepair),
    > {
        let loaded = match self.load() {
            Ok(loaded) => loaded,
            Err(error) => return Err((error, repair)),
        };
        if &loaded != ledger {
            return Err((
                LifecycleLedgerError::InvalidLedger(
                    "signed Broadcast recovery observed a stale ledger snapshot".to_owned(),
                ),
                repair,
            ));
        }
        let Some((_parent_ordinal, child_ordinal, _broadcast_ordinal)) =
            loaded.recovered_phase_signed_broadcast_ordinals(&repair)
        else {
            return Err((
                LifecycleLedgerError::InvalidLedger(
                    "signed Broadcast recovery lost its exact WAL vote lineage".to_owned(),
                ),
                repair,
            ));
        };
        let frame = match encode_frame(&loaded, self.max_frame_bytes) {
            Ok(frame) => frame,
            Err(error) => return Err((error, repair)),
        };
        let receipt = DurableWalVoteLedgerRepairReceipt {
            store_path: self.path.clone(),
            #[cfg(all(unix, not(target_os = "espidf")))]
            store_directory_identity: self.directory.identity,
            context: self.context,
            parent_key: repair.parent().key,
            child_key: repair.child().key,
            edge: repair.edge(),
            child_ordinal,
            ledger_frame_hash: LifecycleDigest::new(Hash::new(frame).into()),
        };
        match repair.bind_durable_ledger_receipt(receipt) {
            Ok(durable) if durable.belongs_to_loaded(self, &loaded) => Ok(durable),
            Ok(_durable) => unreachable!(
                "new signed Broadcast repair receipt must bind its unchanged loaded frame"
            ),
            Err((repair, _receipt)) => Err((
                LifecycleLedgerError::InvalidLedger(
                    "signed Broadcast repair receipt did not bind its WAL authority".to_owned(),
                ),
                repair,
            )),
        }
    }
}
#[cfg(test)]
fn sync_ledger_directory(directory: &Path) -> Result<(), LifecycleLedgerError> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|error| {
            LifecycleLedgerError::Io(format!(
                "failed to sync lifecycle ledger directory {}: {error}",
                directory.display()
            ))
        })
}
#[cfg(test)]
fn ensure_durable_ledger_directory_with<Sync>(
    root: &Path,
    sync: &mut Sync,
) -> Result<(), LifecycleLedgerError>
where
    Sync: FnMut(&Path) -> Result<(), LifecycleLedgerError>,
{
    let parent = root
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match fs::symlink_metadata(root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(LifecycleLedgerError::InvalidFrame(
                    "ledger root is not a regular directory".to_owned(),
                ));
            }
            sync(root)?;
            if parent != root {
                sync(parent)?;
            }
            return Ok(());
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(LifecycleLedgerError::Io(format!(
                "failed to inspect lifecycle ledger root {}: {error}",
                root.display()
            )));
        }
    }
    ensure_durable_ledger_directory_with(parent, sync)?;
    match fs::create_dir(root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(LifecycleLedgerError::Io(format!(
                "failed to create lifecycle ledger root {}: {error}",
                root.display()
            )));
        }
    }
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        LifecycleLedgerError::Io(format!(
            "failed to inspect created lifecycle ledger root {}: {error}",
            root.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(LifecycleLedgerError::InvalidFrame(
            "ledger root is not a regular directory".to_owned(),
        ));
    }
    sync(root)?;
    sync(parent)?;
    Ok(())
}
impl LifecycleCoordinator {
    pub(super) fn stage_durable_transaction(&self) -> Self {
        Self {
            episode_authority: self.episode_authority.clone(),
            active_context: self.active_context,
            records: self.records.clone(),
            key_index: self.key_index.clone(),
            owner_index: self.owner_index.clone(),
            ready_index: self.ready_index.clone(),
            admission_waits: self.admission_waits.clone(),
            active_lease: self.active_lease.clone(),
            high_water: self.high_water,
            lifecycle_ordinal_authority: self.lifecycle_ordinal_authority.clone(),
            next_lease: self.next_lease,
            durable_records: self.durable_records.clone(),
            capacity_geometry: self.capacity_geometry.clone(),
            capacity_used: self.capacity_used.clone(),
            capacity_generation: self.capacity_generation.clone(),
            observed_generation: self.observed_generation.clone(),
            producer_debts: self.producer_debts.clone(),
            ledger_store: self.ledger_store.clone(),
            fault: self.fault,
        }
    }
    pub(super) fn persist_durable_projection(&self) -> Result<(), LifecycleLedgerError> {
        let Some(store) = self.ledger_store.as_ref() else {
            return Ok(());
        };
        let current = store.load()?;
        let successor = LifecycleLedgerV1::from_coordinator(self)?;
        store.persist_exact_ordinary_successor(&current, &successor)
    }
    /// Fsync this staged projection, then release its shared ordinal range.
    pub(super) fn persist_durable_projection_with_ordinal_reservation(
        &self,
        reservation: Option<&DurableLifecycleOrdinalReservation>,
    ) -> Result<(), LifecycleLedgerError> {
        if let Some(reservation) = reservation {
            reservation
                .mark_publication_started()
                .map_err(LifecycleLedgerError::InvalidLedger)?;
        }
        self.persist_durable_projection()?;
        if let Some(reservation) = reservation {
            reservation
                .commit_after_durable_publication()
                .map_err(LifecycleLedgerError::InvalidLedger)?;
        }
        Ok(())
    }

    /// Fsync one staged successor and publish its reserved ordinal range.
    pub(super) fn persist_exact_staged_successor_with_ordinal_reservation(
        &self,
        staged: &Self,
        reservation: &DurableLifecycleOrdinalReservation,
    ) -> Result<(), LifecycleLedgerError> {
        reservation
            .mark_publication_started()
            .map_err(LifecycleLedgerError::InvalidLedger)?;
        self.persist_exact_staged_successor(staged)?;
        reservation
            .commit_after_durable_publication()
            .map_err(LifecycleLedgerError::InvalidLedger)
    }
    /// Fsync one staged successor against this coordinator's exact attached
    /// LedgerV1 frame.
    ///
    /// Unlike the generic durable helper, this first-release transaction never
    /// accepts an in-memory-only coordinator. The staged copy must retain the
    /// same store identity, and the on-disk frame must still equal the live
    /// coordinator projection before it can be replaced.
    pub(super) fn persist_exact_staged_successor(
        &self,
        staged: &Self,
    ) -> Result<(), LifecycleLedgerError> {
        let store = self.ledger_store.as_ref().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "live lifecycle publication requires an attached LedgerV1 store".to_owned(),
            )
        })?;
        let staged_store = staged.ledger_store.as_ref().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "staged lifecycle successor lost its attached LedgerV1 store".to_owned(),
            )
        })?;
        if !store.same_publication_target(staged_store) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "staged lifecycle successor changed its LedgerV1 store".to_owned(),
            ));
        }
        let current = LifecycleLedgerV1::from_coordinator(self)?;
        let successor = LifecycleLedgerV1::from_coordinator(staged)?;
        store.persist_exact_ordinary_successor(&current, &successor)
    }
    /// Fsync one all-row finalized successor against this exact live owner.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn persist_exact_finalization_successor(
        self,
        staged: StagedFinalizationRetirementV1,
    ) -> Result<PublishedFinalizationRetirementV1, LifecycleLedgerError> {
        let StagedFinalizationRetirementV1 { current, retired } = staged;
        let store = self.ledger_store.clone().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "finalized lifecycle retirement requires an attached LedgerV1 store".to_owned(),
            )
        })?;
        if LifecycleLedgerV1::from_coordinator(&self)? != current
            || current.context() != retired.context()
            || current.high_water() != retired.high_water()
            || current.records().len() != retired.records().len()
            || retired
                .records()
                .iter()
                .any(|record| record.terminal() == Some(None))
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "finalized lifecycle successor changed its exact live owner".to_owned(),
            ));
        }
        store.persist_exact_successor(&current, &retired)?;
        if store.load()? != retired {
            return Err(LifecycleLedgerError::InvalidLedger(
                "published finalization successor changed before owner commit".to_owned(),
            ));
        }
        let present = store.authenticate_present_frame(&retired)?.ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "published finalization successor lost its physical frame".to_owned(),
            )
        })?;
        Ok(PublishedFinalizationRetirementV1 {
            coordinator: self,
            current,
            retained_floor: PublishedFinalizedLifecycleRetainedFloorV1 {
                store,
                ledger: retired.clone(),
                present,
            },
            retired,
        })
    }

    #[cfg(test)]
    pub(super) fn attach_empty_test_ledger(
        &mut self,
        root: &Path,
    ) -> Result<(), LifecycleLedgerError> {
        if self.ledger_store.is_some() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "coordinator already owns a lifecycle ledger store".to_owned(),
            ));
        }
        let (store, existing) = LifecycleLedgerStoreV1::open(root, self.active_context)?;
        if existing.high_water != 0 || !existing.records.is_empty() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "test ledger attachment requires a new empty store".to_owned(),
            ));
        }
        store.persist(&LifecycleLedgerV1::from_coordinator(self)?)?;
        self.ledger_store = Some(store);
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn redirect_test_ledger_to_missing_parent(&mut self, root: &Path) {
        let store = self.ledger_store.as_mut().expect("test ledger is attached");
        store.path = root.join("missing-parent").join(LEDGER_FILE);
        store.fail_persistence_for_test = true;
    }
}
fn encode_frame(
    ledger: &LifecycleLedgerV1,
    max_frame_bytes: u64,
) -> Result<Vec<u8>, LifecycleLedgerError> {
    let payload = ledger.encode();
    let payload_len = u64::try_from(payload.len()).map_err(|_| {
        LifecycleLedgerError::InvalidFrame("payload length is not representable".to_owned())
    })?;
    let frame_len = u64::try_from(HEADER_BYTES)
        .expect("header length fits u64")
        .checked_add(payload_len)
        .ok_or_else(|| LifecycleLedgerError::InvalidFrame("frame length overflowed".to_owned()))?;
    if frame_len > max_frame_bytes {
        return Err(LifecycleLedgerError::InvalidFrame(
            "frame exceeds its configured byte bound".to_owned(),
        ));
    }
    let digest = Hash::new(&payload);
    let mut frame =
        Vec::with_capacity(usize::try_from(frame_len).map_err(|_| {
            LifecycleLedgerError::InvalidFrame("frame is not addressable".to_owned())
        })?);
    frame.extend_from_slice(LEDGER_MAGIC);
    frame.extend_from_slice(&LEDGER_VERSION.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(digest.as_ref());
    frame.extend_from_slice(&payload);
    Ok(frame)
}
fn decode_frame(
    bytes: &[u8],
    max_frame_bytes: u64,
) -> Result<LifecycleLedgerV1, LifecycleLedgerError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_frame_bytes
        || bytes.len() < HEADER_BYTES
        || bytes.get(..LEDGER_MAGIC.len()) != Some(LEDGER_MAGIC.as_slice())
    {
        return Err(LifecycleLedgerError::InvalidFrame(
            "header or byte bound is invalid".to_owned(),
        ));
    }
    let version_offset = LEDGER_MAGIC.len();
    let version = u16::from_le_bytes(
        bytes[version_offset..version_offset + 2]
            .try_into()
            .map_err(|_| LifecycleLedgerError::InvalidFrame("version is truncated".to_owned()))?,
    );
    if version != LEDGER_VERSION {
        return Err(LifecycleLedgerError::InvalidFrame(format!(
            "unsupported frame version {version}"
        )));
    }
    let length_offset = version_offset + 2;
    let payload_len = u64::from_le_bytes(
        bytes[length_offset..length_offset + 8]
            .try_into()
            .map_err(|_| LifecycleLedgerError::InvalidFrame("length is truncated".to_owned()))?,
    );
    let payload_len = usize::try_from(payload_len)
        .map_err(|_| LifecycleLedgerError::InvalidFrame("payload is not addressable".to_owned()))?;
    let digest_offset = length_offset + 8;
    let payload_offset = digest_offset + HASH_BYTES;
    if payload_offset.checked_add(payload_len) != Some(bytes.len()) {
        return Err(LifecycleLedgerError::InvalidFrame(
            "frame length is inconsistent".to_owned(),
        ));
    }
    let payload = &bytes[payload_offset..];
    if Hash::new(payload).as_ref() != &bytes[digest_offset..payload_offset] {
        return Err(LifecycleLedgerError::InvalidFrame(
            "checksum mismatch".to_owned(),
        ));
    }
    let mut cursor = payload;
    let ledger = LifecycleLedgerV1::decode_all(&mut cursor).map_err(|error| {
        LifecycleLedgerError::InvalidFrame(format!("Norito decode failed: {error}"))
    })?;
    if ledger.encode() != payload {
        return Err(LifecycleLedgerError::InvalidFrame(
            "payload is not canonically encoded".to_owned(),
        ));
    }
    Ok(ledger)
}
fn work_shape_is_valid(
    work_class: LifecycleWorkClass,
    key: LifecycleKey,
    stage: LifecycleStage,
) -> bool {
    work_class.accepts_stage(key.phase(), stage)
}
fn phase_code(phase: LifecyclePhase) -> u16 {
    match phase {
        LifecyclePhase::Proposal => 1,
        LifecyclePhase::Prepare => 2,
        LifecyclePhase::Commit => 3,
        LifecyclePhase::Timeout => 4,
        LifecyclePhase::Fetch => 5,
        LifecyclePhase::Store => 6,
        LifecyclePhase::Validate => 7,
        LifecyclePhase::Apply => 8,
        LifecyclePhase::BroadcastProposal => 9,
        LifecyclePhase::BroadcastPrepareVote => 10,
        LifecyclePhase::BroadcastCommitVote => 11,
        LifecyclePhase::BroadcastPrepareQc => 12,
        LifecyclePhase::BroadcastCommitQc => 13,
        LifecyclePhase::BroadcastTimeoutVote => 14,
        LifecyclePhase::BroadcastTc => 15,
        LifecyclePhase::EnterView => 16,
        LifecyclePhase::DiagnosticProposalEquivocation => 17,
        LifecyclePhase::DiagnosticVoteEquivocation => 18,
        LifecyclePhase::DiagnosticTimeoutEquivocation => 19,
        LifecyclePhase::DiagnosticInvalidBody => 20,
        LifecyclePhase::Serve => 21,
        LifecyclePhase::ProducerTurn => 22,
    }
}
fn decode_phase(code: u16) -> Option<LifecyclePhase> {
    Some(match code {
        1 => LifecyclePhase::Proposal,
        2 => LifecyclePhase::Prepare,
        3 => LifecyclePhase::Commit,
        4 => LifecyclePhase::Timeout,
        5 => LifecyclePhase::Fetch,
        6 => LifecyclePhase::Store,
        7 => LifecyclePhase::Validate,
        8 => LifecyclePhase::Apply,
        9 => LifecyclePhase::BroadcastProposal,
        10 => LifecyclePhase::BroadcastPrepareVote,
        11 => LifecyclePhase::BroadcastCommitVote,
        12 => LifecyclePhase::BroadcastPrepareQc,
        13 => LifecyclePhase::BroadcastCommitQc,
        14 => LifecyclePhase::BroadcastTimeoutVote,
        15 => LifecyclePhase::BroadcastTc,
        16 => LifecyclePhase::EnterView,
        17 => LifecyclePhase::DiagnosticProposalEquivocation,
        18 => LifecyclePhase::DiagnosticVoteEquivocation,
        19 => LifecyclePhase::DiagnosticTimeoutEquivocation,
        20 => LifecyclePhase::DiagnosticInvalidBody,
        21 => LifecyclePhase::Serve,
        22 => LifecyclePhase::ProducerTurn,
        _ => return None,
    })
}
fn work_class_code(work_class: LifecycleWorkClass) -> u16 {
    match work_class {
        LifecycleWorkClass::SignProposal => 1,
        LifecycleWorkClass::SignVote => 2,
        LifecycleWorkClass::SignTimeout => 3,
        LifecycleWorkClass::Fetch => 4,
        LifecycleWorkClass::Store => 5,
        LifecycleWorkClass::Validate => 6,
        LifecycleWorkClass::Apply => 7,
        LifecycleWorkClass::Broadcast => 8,
        LifecycleWorkClass::EnterView => 9,
        LifecycleWorkClass::EquivocationReport => 10,
        LifecycleWorkClass::InvalidBodyReport => 11,
        LifecycleWorkClass::CertifiedServe => 12,
        LifecycleWorkClass::ProducerTurn => 13,
    }
}
fn decode_work_class(code: u16) -> Option<LifecycleWorkClass> {
    Some(match code {
        1 => LifecycleWorkClass::SignProposal,
        2 => LifecycleWorkClass::SignVote,
        3 => LifecycleWorkClass::SignTimeout,
        4 => LifecycleWorkClass::Fetch,
        5 => LifecycleWorkClass::Store,
        6 => LifecycleWorkClass::Validate,
        7 => LifecycleWorkClass::Apply,
        8 => LifecycleWorkClass::Broadcast,
        9 => LifecycleWorkClass::EnterView,
        10 => LifecycleWorkClass::EquivocationReport,
        11 => LifecycleWorkClass::InvalidBodyReport,
        12 => LifecycleWorkClass::CertifiedServe,
        13 => LifecycleWorkClass::ProducerTurn,
        _ => return None,
    })
}
fn stage_kind_code(kind: LifecycleStageKind) -> u16 {
    match kind {
        LifecycleStageKind::SignProposal => 1,
        LifecycleStageKind::SignPrepareVote => 2,
        LifecycleStageKind::SignCommitVote => 3,
        LifecycleStageKind::SignTimeoutVote => 4,
        LifecycleStageKind::FetchBody => 5,
        LifecycleStageKind::StoreBody => 6,
        LifecycleStageKind::ValidateBody => 7,
        LifecycleStageKind::ApplyDecision => 8,
        LifecycleStageKind::BroadcastProposal => 9,
        LifecycleStageKind::BroadcastPrepareVote => 10,
        LifecycleStageKind::BroadcastCommitVote => 11,
        LifecycleStageKind::BroadcastPrepareQc => 12,
        LifecycleStageKind::BroadcastCommitQc => 13,
        LifecycleStageKind::BroadcastTimeoutVote => 14,
        LifecycleStageKind::BroadcastTc => 15,
        LifecycleStageKind::EnterView => 16,
        LifecycleStageKind::ReportProposalEquivocation => 17,
        LifecycleStageKind::ReportVoteEquivocation => 18,
        LifecycleStageKind::ReportTimeoutEquivocation => 19,
        LifecycleStageKind::ReportInvalidBody => 20,
        LifecycleStageKind::CertifiedServe => 21,
        LifecycleStageKind::ProducerTurn => 22,
    }
}
fn decode_stage_kind(code: u16) -> Option<LifecycleStageKind> {
    Some(match code {
        1 => LifecycleStageKind::SignProposal,
        2 => LifecycleStageKind::SignPrepareVote,
        3 => LifecycleStageKind::SignCommitVote,
        4 => LifecycleStageKind::SignTimeoutVote,
        5 => LifecycleStageKind::FetchBody,
        6 => LifecycleStageKind::StoreBody,
        7 => LifecycleStageKind::ValidateBody,
        8 => LifecycleStageKind::ApplyDecision,
        9 => LifecycleStageKind::BroadcastProposal,
        10 => LifecycleStageKind::BroadcastPrepareVote,
        11 => LifecycleStageKind::BroadcastCommitVote,
        12 => LifecycleStageKind::BroadcastPrepareQc,
        13 => LifecycleStageKind::BroadcastCommitQc,
        14 => LifecycleStageKind::BroadcastTimeoutVote,
        15 => LifecycleStageKind::BroadcastTc,
        16 => LifecycleStageKind::EnterView,
        17 => LifecycleStageKind::ReportProposalEquivocation,
        18 => LifecycleStageKind::ReportVoteEquivocation,
        19 => LifecycleStageKind::ReportTimeoutEquivocation,
        20 => LifecycleStageKind::ReportInvalidBody,
        21 => LifecycleStageKind::CertifiedServe,
        22 => LifecycleStageKind::ProducerTurn,
        _ => return None,
    })
}
const fn predecessor_code(scope: PredecessorScope) -> u8 {
    match scope {
        PredecessorScope::Independent => 0,
        PredecessorScope::ReadyOrdinalPrefix => 1,
        PredecessorScope::ProducerHandoffBarrier => 2,
    }
}
const fn decode_predecessor(code: u8) -> Option<PredecessorScope> {
    match code {
        0 => Some(PredecessorScope::Independent),
        1 => Some(PredecessorScope::ReadyOrdinalPrefix),
        2 => Some(PredecessorScope::ProducerHandoffBarrier),
        _ => None,
    }
}
/// Substitute one structurally valid but foreign control replay authority in a test frame.
#[cfg(test)]
pub(crate) fn substitute_recovered_control_replay_authority_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let controls = ledger
        .records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            matches!(
                record.work_class(),
                Some(LifecycleWorkClass::SignProposal | LifecycleWorkClass::SignTimeout)
            )
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let [index] = controls.as_slice() else {
        return false;
    };
    let Some(foreign) = ledger.records[*index]
        .replay_authority
        .with_foreign_origin_generation_for_test()
    else {
        return false;
    };
    ledger.records[*index].replay_authority = foreign;
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}
/// Substitute a structurally valid foreign replay origin on the WAL Decision Fetch row.
#[cfg(test)]
pub(crate) fn substitute_recovered_decision_fetch_replay_authority_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let fetches = ledger
        .records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            (record.work_class() == Some(LifecycleWorkClass::Fetch)
                && record.durable_payload() == Some(DurablePayloadReference::None))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let [index] = fetches.as_slice() else {
        return false;
    };
    let Some(foreign) = ledger.records[*index]
        .replay_authority
        .with_foreign_origin_generation_for_test()
    else {
        return false;
    };
    ledger.records[*index].replay_authority = foreign;
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}
/// Substitute a valid foreign owner while retaining the exact Decision Fetch key.
#[cfg(test)]
pub(crate) fn substitute_recovered_decision_fetch_owner_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let fetches = ledger
        .records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            (record.work_class() == Some(LifecycleWorkClass::Fetch)
                && record.durable_payload() == Some(DurablePayloadReference::None))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let [index] = fetches.as_slice() else {
        return false;
    };
    let ordinal = ledger.records[*index].ordinal;
    let owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xDF; 32])), ordinal);
    ledger.records[*index].causal_root = *owner.causal_root().digest().as_bytes();
    ledger.records[*index].owner_first_ordinal = owner.first_admission_ordinal();
    ledger.records[*index].reconstruction_source = *owner.causal_root().digest().as_bytes();
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}
/// Append a valid foreign terminal row which aliases the control row's owner.
#[cfg(test)]
pub(crate) fn append_same_owner_foreign_terminal_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let controls = ledger
        .records
        .iter()
        .filter(|record| {
            matches!(
                record.work_class(),
                Some(LifecycleWorkClass::SignProposal | LifecycleWorkClass::SignTimeout)
            )
        })
        .collect::<Vec<_>>();
    let [control] = controls.as_slice() else {
        return false;
    };
    let owner = control.owner();
    let Some(ordinal) = ledger.high_water.checked_add(1) else {
        return false;
    };
    let foreign = super::replay_authority::exact_record_fixture(
        context,
        LifecycleStageKind::ReportProposalEquivocation,
        0x7F,
    );
    let Ok(terminal) = LifecycleLedgerRecordV1::new(
        foreign.key,
        owner,
        ordinal,
        foreign.work_class,
        foreign.stage,
        Some(TerminalOutcome::Cancelled),
        owner.causal_root().digest(),
        foreign.payload,
        foreign.authority,
        DurableContinuation::None,
    ) else {
        return false;
    };
    ledger.records.push(terminal);
    ledger.high_water = ordinal;
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}

/// Install exact historical timeout Broadcasts before an incumbent current control Sign.
#[cfg(all(test, feature = "bls"))]
pub(crate) fn install_timeout_broadcasts_before_current_control_for_test(
    root: &Path,
    context: LifecycleContext,
    timeout_edges: Vec<(wire::TimeoutVote, wire::TimeoutVote)>,
    incumbent_current: bool,
) -> bool {
    let Ok((store, ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let [current] = ledger.records.as_slice() else {
        return false;
    };
    if !matches!(
        current.work_class(),
        Some(LifecycleWorkClass::SignProposal | LifecycleWorkClass::SignTimeout)
    ) || current.terminal() != Some(None)
        || current.continuation() != Some(DurableContinuation::None)
    {
        return false;
    }
    if timeout_edges.is_empty() {
        return false;
    }
    let mut records = Vec::with_capacity(timeout_edges.len().saturating_mul(2).saturating_add(1));
    let mut ordinal = 1_u128;
    for (index, (unsigned, signed)) in timeout_edges.into_iter().enumerate() {
        let [parent_replay, child_replay] =
            super::replay_authority::exact_timeout_sign_broadcast_fixture(
                context, unsigned, signed,
            );
        let mut root = [0xD7; 32];
        root[31] = u8::try_from(index).unwrap_or(u8::MAX);
        let old_owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new(root)), ordinal);
        let Some(child_ordinal) = ordinal.checked_add(1) else {
            return false;
        };
        let Ok(parent) = LifecycleLedgerRecordV1::new(
            parent_replay.key,
            old_owner,
            ordinal,
            LifecycleWorkClass::SignTimeout,
            parent_replay.stage,
            Some(TerminalOutcome::Advanced),
            old_owner.causal_root().digest(),
            DurablePayloadReference::None,
            parent_replay.authority,
            DurableContinuation::successor(
                DurableContinuationEdge::SignTimeoutToBroadcast,
                child_ordinal,
            ),
        ) else {
            return false;
        };
        let Ok(child) = LifecycleLedgerRecordV1::new(
            child_replay.key,
            old_owner,
            child_ordinal,
            LifecycleWorkClass::Broadcast,
            child_replay.stage,
            None,
            old_owner.causal_root().digest(),
            DurablePayloadReference::None,
            child_replay.authority,
            DurableContinuation::None,
        ) else {
            return false;
        };
        records.extend([parent, child]);
        let Some(next_ordinal) = child_ordinal.checked_add(1) else {
            return false;
        };
        ordinal = next_ordinal;
    }
    let high_water = if incumbent_current {
        let mut current = current.clone();
        current.owner_first_ordinal = ordinal;
        current.ordinal = ordinal;
        records.push(current);
        ordinal
    } else {
        ordinal.saturating_sub(1)
    };
    let Ok(incident) = LifecycleLedgerV1::new(context, high_water, records, BTreeMap::new()) else {
        return false;
    };
    store.persist(&incident).is_ok()
}

/// Install a live non-timeout Broadcast lineage beside an incumbent control Sign.
#[cfg(all(test, feature = "bls"))]
pub(crate) fn install_non_timeout_broadcast_before_current_control_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let [current] = ledger.records.as_slice() else {
        return false;
    };
    let parent_replay =
        super::replay_authority::exact_record_fixture(context, LifecycleStageKind::SignProposal, 0);
    let child_replay = super::replay_authority::exact_record_fixture(
        context,
        LifecycleStageKind::BroadcastProposal,
        0,
    );
    let owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xD9; 32])), 1);
    let Ok(parent) = LifecycleLedgerRecordV1::new(
        parent_replay.key,
        owner,
        1,
        LifecycleWorkClass::SignProposal,
        parent_replay.stage,
        Some(TerminalOutcome::Advanced),
        owner.causal_root().digest(),
        DurablePayloadReference::None,
        parent_replay.authority,
        DurableContinuation::successor(DurableContinuationEdge::SignProposalToBroadcast, 2),
    ) else {
        return false;
    };
    let Ok(child) = LifecycleLedgerRecordV1::new(
        child_replay.key,
        owner,
        2,
        LifecycleWorkClass::Broadcast,
        child_replay.stage,
        None,
        owner.causal_root().digest(),
        DurablePayloadReference::None,
        child_replay.authority,
        DurableContinuation::None,
    ) else {
        return false;
    };
    let mut current = current.clone();
    current.owner_first_ordinal = 3;
    current.ordinal = 3;
    let Ok(incident) =
        LifecycleLedgerV1::new(context, 3, vec![parent, child, current], BTreeMap::new())
    else {
        return false;
    };
    store.persist(&incident).is_ok()
}

/// Return the closed scalar census for an obsolete-timeout/current-control test frame.
#[cfg(all(test, feature = "bls"))]
pub(crate) fn control_timeout_supersession_summary_for_test(
    root: &Path,
    context: LifecycleContext,
) -> Option<(u128, usize, usize)> {
    let (_store, ledger) = LifecycleLedgerStoreV1::open(root, context).ok()?;
    let cancelled_timeout_broadcasts = ledger
        .records
        .iter()
        .filter(|record| {
            record.work_class() == Some(LifecycleWorkClass::Broadcast)
                && record
                    .stage()
                    .is_some_and(|stage| stage.kind() == LifecycleStageKind::BroadcastTimeoutVote)
                && record.terminal() == Some(Some(TerminalOutcome::Cancelled))
        })
        .count();
    let live_controls = ledger
        .records
        .iter()
        .filter(|record| {
            matches!(
                record.work_class(),
                Some(LifecycleWorkClass::SignProposal | LifecycleWorkClass::SignTimeout)
            ) && record.terminal() == Some(None)
        })
        .count();
    Some((
        ledger.high_water(),
        cancelled_timeout_broadcasts,
        live_controls,
    ))
}

/// Inject a publication failure after the exact supersession successor is staged.
#[cfg(all(test, feature = "bls"))]
pub(in crate::sumeragi) fn control_timeout_supersession_persistence_failure_for_test(
    root: &Path,
    context: LifecycleContext,
    verified: &VerifiedHeightContext,
    projection: &AuthenticatedRecoveredWalControlProjection,
) -> bool {
    let Ok((store, opened)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let Ok((reconciled, Some(_staged_supersession))) =
        opened.reconcile_superseded_timeout_broadcast(verified, projection)
    else {
        return false;
    };
    let Ok((successor, _, _)) = reconciled.stage_authenticated_wal_control_sign(projection) else {
        return false;
    };
    let path = root.join(LEDGER_FILE);
    let temporary = path.with_extension("norito.tmp");
    let Ok(original) = fs::read(&path) else {
        return false;
    };
    if fs::create_dir(&temporary).is_err() {
        return false;
    }
    let failed = store.persist_exact_successor(&opened, &successor).is_err();
    let restored = fs::remove_dir(&temporary).is_ok();
    failed
        && restored
        && fs::read(&path).ok().as_ref() == Some(&original)
        && store.load().ok().as_ref() == Some(&opened)
}
