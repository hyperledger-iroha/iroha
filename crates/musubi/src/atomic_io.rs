//! Durable, root-confined atomic file access for Musubi project state.
//!
//! Callers first bind access to a trusted directory with [`AtomicWriteRoot`] and then provide only
//! a normal relative path. This deliberately prevents a path read from a lockfile or registry
//! response from becoming an arbitrary filesystem target.
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::fd::AsRawFd as _;
#[cfg(unix)]
use std::os::unix::fs::{
    DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
};
use std::{
    ffi::OsString,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const TEMP_CREATE_ATTEMPTS: u64 = 128;
const POST_LINK_CLEANUP_ATTEMPTS: usize = 2;
const TEMP_FILE_PREFIX: &str = ".musubi-tmp-";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
#[cfg(all(test, unix))]
std::thread_local! {
    static TEST_DIRECTORY_SYNC_FAILURES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static TEST_IMMUTABLE_READ_FIFO_SUBSTITUTIONS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}
#[cfg(all(test, any(target_os = "linux", target_os = "android")))]
std::thread_local! {
    static TEST_AFTER_DESCRIPTOR_TARGET_BIND: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
    static TEST_BEFORE_DESCRIPTOR_ROOT_REVALIDATION: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}
#[cfg(all(test, any(target_os = "linux", target_os = "android")))]
pub(crate) fn install_descriptor_root_read_test_hooks(
    after_target_bind: impl FnOnce() + 'static,
    before_revalidation: impl FnOnce() + 'static,
) {
    TEST_AFTER_DESCRIPTOR_TARGET_BIND.with(|hook| {
        assert!(
            hook.borrow_mut()
                .replace(Box::new(after_target_bind))
                .is_none(),
            "descriptor-target after-bind hook must not already be installed"
        );
    });
    TEST_BEFORE_DESCRIPTOR_ROOT_REVALIDATION.with(|hook| {
        assert!(
            hook.borrow_mut()
                .replace(Box::new(before_revalidation))
                .is_none(),
            "descriptor-root before-revalidation hook must not already be installed"
        );
    });
}
#[cfg(all(test, any(target_os = "linux", target_os = "android")))]
fn run_test_after_descriptor_target_bind() {
    TEST_AFTER_DESCRIPTOR_TARGET_BIND.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
}
#[cfg(all(test, any(target_os = "linux", target_os = "android")))]
fn run_test_before_descriptor_root_revalidation() {
    TEST_BEFORE_DESCRIPTOR_ROOT_REVALIDATION.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
}
/// Result of installing immutable bytes at a previously absent path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicInstallOutcome {
    /// This call installed the immutable file.
    Installed,
    /// The destination already contained exactly the requested bytes.
    AlreadyPresent,
}
/// Stable category for an atomic-write failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicWriteErrorCode {
    /// The platform cannot provide the required race-safe atomic replacement primitive.
    UnsupportedPlatform,
    /// The requested destination was empty, absolute, or contained a non-normal component.
    InvalidRelativePath,
    /// The configured write root was not a stable, real directory.
    UnsafeRoot,
    /// A destination ancestor was a symbolic link.
    SymlinkAncestor,
    /// A destination ancestor was missing or was not a directory.
    UnsafeParent,
    /// The target was unsafe, unsupported, hard linked, or larger than the operation bound.
    UnsafeTarget,
    /// An existing safe regular destination contained different immutable bytes.
    ImmutableConflict,
    /// The root, parent chain, destination, or private temporary file changed during the write.
    ConcurrentModification,
    /// All bounded private temporary-file names were already occupied.
    TemporaryNameExhausted,
    /// A filesystem operation failed.
    Io,
}
impl AtomicWriteErrorCode {
    /// Return the stable diagnostic spelling for this failure category.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "MUSUBI_ATOMIC_UNSUPPORTED_PLATFORM",
            Self::InvalidRelativePath => "MUSUBI_ATOMIC_INVALID_RELATIVE_PATH",
            Self::UnsafeRoot => "MUSUBI_ATOMIC_UNSAFE_ROOT",
            Self::SymlinkAncestor => "MUSUBI_ATOMIC_SYMLINK_ANCESTOR",
            Self::UnsafeParent => "MUSUBI_ATOMIC_UNSAFE_PARENT",
            Self::UnsafeTarget => "MUSUBI_ATOMIC_UNSAFE_TARGET",
            Self::ImmutableConflict => "MUSUBI_ATOMIC_IMMUTABLE_CONFLICT",
            Self::ConcurrentModification => "MUSUBI_ATOMIC_CONCURRENT_MODIFICATION",
            Self::TemporaryNameExhausted => "MUSUBI_ATOMIC_TEMPORARY_NAME_EXHAUSTED",
            Self::Io => "MUSUBI_ATOMIC_IO",
        }
    }
}
/// Error returned by an [`AtomicWriteRoot`] operation.
#[derive(Debug)]
pub struct AtomicWriteError {
    code: AtomicWriteErrorCode,
    path: PathBuf,
    operation: &'static str,
    source: Option<io::Error>,
    recovery: Option<String>,
}
impl AtomicWriteError {
    fn new(code: AtomicWriteErrorCode, path: impl Into<PathBuf>, operation: &'static str) -> Self {
        Self {
            code,
            path: path.into(),
            operation,
            source: None,
            recovery: None,
        }
    }
    fn io(path: impl Into<PathBuf>, operation: &'static str, source: io::Error) -> Self {
        Self {
            code: AtomicWriteErrorCode::Io,
            path: path.into(),
            operation,
            source: Some(source),
            recovery: None,
        }
    }
    fn with_recovery_failure(mut self, recovery: &Self) -> Self {
        let recovery = recovery.to_string();
        self.recovery = Some(match self.recovery.take() {
            Some(mut existing) => {
                existing.push_str("; additional recovery failure: ");
                existing.push_str(&recovery);
                existing
            }
            None => recovery,
        });
        self
    }
    /// Return the stable failure category.
    #[must_use]
    pub const fn code(&self) -> AtomicWriteErrorCode {
        self.code
    }
    /// Return the path at which validation or I/O failed.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}
impl fmt::Display for AtomicWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} while attempting to {} `{}`",
            self.code.as_str(),
            self.operation,
            self.path.display()
        )?;
        if let Some(source) = &self.source {
            write!(formatter, ": {source}")?;
        }
        if let Some(recovery) = &self.recovery {
            write!(
                formatter,
                "; cleanup or parent-directory durability was not proven: {recovery}"
            )?;
        }
        Ok(())
    }
}
impl std::error::Error for AtomicWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| -> &(dyn std::error::Error + 'static) { source })
    }
}
/// Trusted directory within which Musubi may atomically access project-state files.
#[derive(Debug)]
pub struct AtomicWriteRoot {
    canonical_root: PathBuf,
    root_identity: FileIdentity,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    root_handle: File,
}
impl AtomicWriteRoot {
    /// Create or bind one private user-state root without following a requested descendant link.
    ///
    /// The path must be absolute. The deepest existing ancestor is first bound to its canonical
    /// directory identity. Missing normal components are then created one at a time with mode
    /// `0700`, with the parent identity revalidated and synchronized after every creation. The
    /// final root must be a real `0700` directory even when it already existed. The returned root
    /// uses the canonical path so later journal access cannot be redirected by changing a lexical
    /// ancestor of the platform path.
    ///
    /// # Errors
    ///
    /// Returns an error for relative/traversing paths, symlinks, non-directory components,
    /// non-private existing roots, identity changes, unsupported platforms, or I/O failures.
    pub fn open_or_create_private(root: &Path) -> Result<Self, AtomicWriteError> {
        #[cfg(not(unix))]
        {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::UnsupportedPlatform,
                root,
                "create a private atomic write root on this platform",
            ));
        }
        #[cfg(unix)]
        {
            let canonical_root = create_or_open_private_root(root)?;
            Self::new(&canonical_root)
        }
    }
    /// Bind future writes to an existing, non-symlink directory.
    ///
    /// Musubi V1 enables this writer on Unix. Other targets fail closed until
    /// they expose a safe handle-relative replace-existing operation.
    ///
    /// # Errors
    ///
    /// Returns an error if `root` is missing, is a symlink, is not a directory,
    /// changes during canonicalization, or cannot be inspected.
    pub fn new(root: &Path) -> Result<Self, AtomicWriteError> {
        if !cfg!(unix) {
            // TODO: Enable Windows after a safe handle-relative replace-existing primitive is
            // available to this crate. Dropping a retained directory handle around
            // `std::fs::rename` would reintroduce the reparse/substitution race this writer is
            // intended to exclude.
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::UnsupportedPlatform,
                root,
                "bind a race-safe atomic write root on this platform",
            ));
        }
        let linked = fs::symlink_metadata(root)
            .map_err(|error| AtomicWriteError::io(root, "inspect the write root", error))?;
        if linked.file_type().is_symlink() || !linked.is_dir() {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::UnsafeRoot,
                root,
                "bind the write root",
            ));
        }
        let canonical_root = fs::canonicalize(root)
            .map_err(|error| AtomicWriteError::io(root, "canonicalize the write root", error))?;
        let canonical_metadata = fs::symlink_metadata(&canonical_root).map_err(|error| {
            AtomicWriteError::io(&canonical_root, "inspect the canonical write root", error)
        })?;
        if canonical_metadata.file_type().is_symlink()
            || !canonical_metadata.is_dir()
            || !same_file(&linked, &canonical_metadata)
        {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::UnsafeRoot,
                root,
                "bind a stable write root",
            ));
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let root_handle = open_directory_no_follow(&canonical_root, &canonical_metadata)?;
        Ok(Self {
            canonical_root,
            root_identity: FileIdentity::from_metadata(&canonical_metadata),
            #[cfg(any(target_os = "linux", target_os = "android"))]
            root_handle,
        })
    }
    /// Return the canonical trusted root.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.canonical_root
    }
    /// Durably replace one root-relative regular file.
    ///
    /// The destination parent must already exist. The method rejects absolute paths, traversal,
    /// symlink parents and targets, non-regular targets, and hard-linked targets. V1 currently
    /// enables replacement only on Unix. It creates a private temporary file in the destination
    /// directory with `create_new`, writes and flushes all bytes, synchronizes the file, atomically
    /// renames it, and synchronizes the parent directory. Existing permissions are retained; a new
    /// file is private (`0600`) on Unix.
    ///
    /// A concurrent change is rejected instead of overwriting the unexpected
    /// entry. Failure cleanup removes only the temporary inode created by this
    /// call; a substituted path is deliberately left untouched.
    ///
    /// # Errors
    ///
    /// Returns a categorized validation or filesystem error. An error from the
    /// final parent-directory synchronization means the rename completed but
    /// crash durability could not be confirmed.
    pub fn replace(&self, relative: &Path, contents: &[u8]) -> Result<(), AtomicWriteError> {
        validate_relative_path(relative)?;
        self.validate_root()?;
        let target = self.canonical_root.join(relative);
        let parent = target.parent().ok_or_else(|| {
            AtomicWriteError::new(
                AtomicWriteErrorCode::InvalidRelativePath,
                relative,
                "resolve the destination parent",
            )
        })?;
        let parent_chain = self.validate_parent_chain(parent)?;
        let target_snapshot = inspect_target(&target)?;
        let retained_permissions = target_snapshot.permissions();
        let mut pending = PendingTemp::create(parent)?;
        if let Some(permissions) = retained_permissions {
            pending.file.set_permissions(permissions).map_err(|error| {
                AtomicWriteError::io(&pending.path, "set temporary-file permissions", error)
            })?;
        }
        pending.file.write_all(contents).map_err(|error| {
            AtomicWriteError::io(&pending.path, "write the temporary file", error)
        })?;
        pending.file.flush().map_err(|error| {
            AtomicWriteError::io(&pending.path, "flush the temporary file", error)
        })?;
        pending.file.sync_all().map_err(|error| {
            AtomicWriteError::io(&pending.path, "synchronize the temporary file", error)
        })?;
        self.validate_root()?;
        validate_directory_snapshots(&parent_chain)?;
        validate_target_snapshot(&target, &target_snapshot)?;
        pending.validate_path_identity()?;
        fs::rename(&pending.path, &target).map_err(|error| {
            AtomicWriteError::io(&target, "atomically replace the destination", error)
        })?;
        pending.disarm();
        sync_directory(
            parent,
            parent_chain.last().expect("parent chain is non-empty"),
        )?;
        Ok(())
    }
    /// Load one bounded immutable root-relative regular file.
    ///
    /// The destination parent must already exist. The method rejects absolute paths, traversal,
    /// symlink parents and targets, non-regular targets, and hard-linked targets. It opens the
    /// destination without following links and in nonblocking mode, binds the open file to the
    /// inspected single-link inode, reads at most one byte beyond `max_bytes`, and revalidates the
    /// file, parent chain, and root. A destination that remains absent throughout validation
    /// returns `None`.
    ///
    /// # Errors
    ///
    /// Returns a categorized validation or filesystem error. A file larger than
    /// `max_bytes` is rejected as [`AtomicWriteErrorCode::UnsafeTarget`].
    pub fn load_immutable(
        &self,
        relative: &Path,
        max_bytes: usize,
    ) -> Result<Option<Vec<u8>>, AtomicWriteError> {
        validate_relative_path(relative)?;
        if PLATFORM_SECURE_OPEN_FLAGS.is_none() {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::UnsupportedPlatform,
                relative,
                "load immutable bytes with no-follow access",
            ));
        }
        self.validate_root()?;
        let target = self.canonical_root.join(relative);
        let parent = target.parent().ok_or_else(|| {
            AtomicWriteError::new(
                AtomicWriteErrorCode::InvalidRelativePath,
                relative,
                "resolve the immutable source parent",
            )
        })?;
        let parent_chain = self.validate_parent_chain(parent)?;
        let linked = match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                validate_single_link_immutable_metadata(&target, &metadata)?;
                metadata
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.validate_root()?;
                validate_directory_snapshots(&parent_chain)?;
                match fs::symlink_metadata(&target) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        self.validate_root()?;
                        validate_directory_snapshots(&parent_chain)?;
                        return Ok(None);
                    }
                    Ok(_) => {
                        return Err(AtomicWriteError::new(
                            AtomicWriteErrorCode::ConcurrentModification,
                            &target,
                            "revalidate an absent immutable source",
                        ));
                    }
                    Err(error) => {
                        return Err(AtomicWriteError::io(
                            &target,
                            "reinspect the absent immutable source",
                            error,
                        ));
                    }
                }
            }
            Err(error) => {
                return Err(AtomicWriteError::io(
                    &target,
                    "inspect the immutable source",
                    error,
                ));
            }
        };
        let expected_identity = FileIdentity::from_metadata(&linked);
        let read = read_immutable_target_bounded(&target, max_bytes, Some(expected_identity));
        let revalidation = (|| {
            self.validate_root()?;
            validate_directory_snapshots(&parent_chain)
        })();
        match preserve_primary_result(read, revalidation)? {
            ImmutableReadOutcome::Within(bytes) => Ok(Some(bytes)),
            ImmutableReadOutcome::Exceeded => Err(AtomicWriteError::new(
                AtomicWriteErrorCode::UnsafeTarget,
                &target,
                "enforce the immutable source size bound",
            )),
        }
    }
    /// Load one private immutable file through the retained root descriptor.
    ///
    /// Linux and Android expose an open directory descriptor as
    /// `/proc/self/fd/<fd>`. Appending the validated relative name makes the final
    /// file open resolve below the retained directory even if the canonical root pathname is
    /// temporarily replaced. The descriptor anchor and canonical root identity are checked before
    /// and after the read. Other platforms fail closed; safe `std` does not expose `openat` or an
    /// equivalent descriptor-rooted open. This requires a kernel-provided procfs at `/proc`; the
    /// procfs entry must resolve to the retained directory inode or the read is rejected.
    ///
    /// # Errors
    ///
    /// Returns a categorized validation or filesystem error. The target must be a
    /// single-link private regular file no larger than `max_bytes`.
    pub(crate) fn load_private_descriptor_rooted(
        &self,
        relative: &Path,
        max_bytes: usize,
    ) -> Result<Option<Vec<u8>>, AtomicWriteError> {
        validate_relative_path(relative)?;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            let _ = (self, max_bytes);
            // TODO: Enable this read on other targets only after safe `std` or an approved
            // workspace primitive provides a descriptor-rooted, no-follow final open.
            Err(AtomicWriteError::new(
                AtomicWriteErrorCode::UnsupportedPlatform,
                relative,
                "load private bytes through a retained root descriptor",
            ))
        }
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            self.validate_retained_root()?;
            let descriptor_root =
                PathBuf::from("/proc/self/fd").join(self.root_handle.as_raw_fd().to_string());
            self.validate_descriptor_root(&descriptor_root)?;
            let target = descriptor_root.join(relative);
            let read = read_private_immutable_target_bounded(&target, max_bytes);
            #[cfg(test)]
            run_test_before_descriptor_root_revalidation();
            let revalidation = (|| {
                self.validate_descriptor_root(&descriptor_root)?;
                self.validate_retained_root()
            })();
            preserve_primary_result(read, revalidation)
        }
    }
    /// Durably install one immutable root-relative regular file without replacement.
    ///
    /// The destination and parent constraints are the same as for [`Self::replace`].
    /// A private `create_new` temporary file is written, flushed, and synchronized in
    /// the destination directory. Unix `link(2)` semantics then install that inode at
    /// the destination only if the name is still absent. The temporary name is removed
    /// only while it still identifies the inode created by this call, and the parent is
    /// synchronized before a bounded nonblocking no-follow readback verifies the exact bytes.
    ///
    /// An existing single-link regular file with identical bytes is idempotent. Any different
    /// regular-file contents return [`AtomicWriteErrorCode::ImmutableConflict`]; symlinks, hard
    /// links, directories, and special files return [`AtomicWriteErrorCode::UnsafeTarget`]. No
    /// existing destination is overwritten.
    ///
    /// TODO: Qualify this path for production with descriptor-relative
    /// `renameat`/`linkat`/`unlinkat` no-replace operations once a permitted safe dependency and
    /// lockfile update are available. The std-only implementation uses the module's existing
    /// identity-checked pathname cleanup model and therefore does not claim protection from a
    /// hostile same-UID substitution between its final metadata check and temporary-name unlink. A
    /// crash before cleanup or a cleanup failure under hostile substitution can leave a recoverable
    /// exact-owned temporary name as a second link to the installed inode. Such a residue is never
    /// deleted by a broad scan; safely discovering and recovering it is part of that production
    /// gate.
    ///
    /// # Errors
    ///
    /// Returns a categorized validation, immutable-conflict, or filesystem error. An
    /// error after the no-clobber link may mean the requested bytes are present but
    /// cleanup, synchronization, or exact readback could not be proven. Retrying never
    /// overwrites the destination, but an unrecovered two-link residue can continue to
    /// fail closed as [`AtomicWriteErrorCode::UnsafeTarget`].
    #[expect(
        clippy::too_many_lines,
        reason = "the immutable-install state machine keeps identity validation, no-clobber publication, cleanup, durability, and readback in one auditable sequence"
    )]
    pub fn install_immutable(
        &self,
        relative: &Path,
        contents: &[u8],
    ) -> Result<AtomicInstallOutcome, AtomicWriteError> {
        validate_relative_path(relative)?;
        if PLATFORM_SECURE_OPEN_FLAGS.is_none() {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::UnsupportedPlatform,
                relative,
                "install immutable bytes with qualified no-follow readback",
            ));
        }
        self.validate_root()?;
        let target = self.canonical_root.join(relative);
        let parent = target.parent().ok_or_else(|| {
            AtomicWriteError::new(
                AtomicWriteErrorCode::InvalidRelativePath,
                relative,
                "resolve the immutable destination parent",
            )
        })?;
        let parent_chain = self.validate_parent_chain(parent)?;
        match fs::symlink_metadata(&target) {
            Ok(_) => {
                let binding = bind_existing_immutable_target(&target, contents);
                let result = match binding {
                    Ok(identity) => {
                        let durability = sync_directory_bounded(
                            parent,
                            parent_chain.last().expect("parent chain is non-empty"),
                        );
                        let readback =
                            revalidate_existing_immutable_target(&target, contents, identity);
                        preserve_primary_result(durability, readback)
                            .map(|()| AtomicInstallOutcome::AlreadyPresent)
                    }
                    Err(error) => Err(error),
                };
                let revalidation = (|| {
                    self.validate_root()?;
                    validate_directory_snapshots(&parent_chain)
                })();
                return preserve_primary_result(result, revalidation);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(AtomicWriteError::io(
                    &target,
                    "inspect the immutable destination",
                    error,
                ));
            }
        }
        let mut pending = PendingTemp::create(parent)?;
        let prepared = (|| {
            pending.file.write_all(contents).map_err(|error| {
                AtomicWriteError::io(&pending.path, "write the immutable temporary file", error)
            })?;
            pending.file.flush().map_err(|error| {
                AtomicWriteError::io(&pending.path, "flush the immutable temporary file", error)
            })?;
            pending.file.sync_all().map_err(|error| {
                AtomicWriteError::io(
                    &pending.path,
                    "synchronize the immutable temporary file",
                    error,
                )
            })?;
            self.validate_root()?;
            validate_directory_snapshots(&parent_chain)?;
            pending.validate_path_identity_with_links(1)
        })();
        if let Err(error) = prepared {
            let cleanup = cleanup_pending_and_sync(
                pending,
                1,
                parent,
                parent_chain.last().expect("parent chain is non-empty"),
            );
            return preserve_primary_result(Err(error), cleanup);
        }
        match fs::hard_link(&pending.path, &target) {
            Ok(()) => {
                let installed_identity = pending.identity;
                let installed = (|| {
                    validate_new_immutable_link(&target, &pending, 2)?;
                    pending.remove_owned(2)?;
                    validate_installed_immutable_target(&target, installed_identity)?;
                    sync_directory(
                        parent,
                        parent_chain.last().expect("parent chain is non-empty"),
                    )?;
                    self.validate_root()?;
                    validate_directory_snapshots(&parent_chain)?;
                    if !readback_immutable_target(&target, contents, Some(installed_identity))? {
                        return Err(AtomicWriteError::new(
                            AtomicWriteErrorCode::ConcurrentModification,
                            &target,
                            "verify newly installed immutable bytes",
                        ));
                    }
                    Ok(AtomicInstallOutcome::Installed)
                })();
                match installed {
                    Ok(outcome) => Ok(outcome),
                    Err(error) => {
                        let recovery = recover_post_link_temp_and_sync(
                            &mut pending,
                            parent,
                            parent_chain.last().expect("parent chain is non-empty"),
                        );
                        preserve_primary_result(Err(error), recovery)
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let binding = bind_existing_immutable_target(&target, contents);
                let cleanup = cleanup_pending_and_sync(
                    pending,
                    1,
                    parent,
                    parent_chain.last().expect("parent chain is non-empty"),
                );
                let result = match binding {
                    Ok(identity) => {
                        let readback =
                            revalidate_existing_immutable_target(&target, contents, identity);
                        preserve_primary_result(cleanup, readback)
                            .map(|()| AtomicInstallOutcome::AlreadyPresent)
                    }
                    Err(error) => preserve_primary_result(Err(error), cleanup),
                };
                let revalidation = (|| {
                    self.validate_root()?;
                    validate_directory_snapshots(&parent_chain)
                })();
                preserve_primary_result(result, revalidation)
            }
            Err(error) => {
                let install_error = AtomicWriteError::io(
                    &target,
                    "atomically install the immutable destination",
                    error,
                );
                let cleanup = cleanup_pending_and_sync(
                    pending,
                    1,
                    parent,
                    parent_chain.last().expect("parent chain is non-empty"),
                );
                preserve_primary_result(Err(install_error), cleanup)
            }
        }
    }
    fn validate_root(&self) -> Result<(), AtomicWriteError> {
        let current = fs::symlink_metadata(&self.canonical_root).map_err(|error| {
            AtomicWriteError::io(&self.canonical_root, "revalidate the write root", error)
        })?;
        if current.file_type().is_symlink()
            || !current.is_dir()
            || !self.root_identity.matches(&current)
        {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::ConcurrentModification,
                &self.canonical_root,
                "revalidate the write root",
            ));
        }
        Ok(())
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn validate_retained_root(&self) -> Result<(), AtomicWriteError> {
        let opened_before = self.root_handle.metadata().map_err(|error| {
            AtomicWriteError::io(
                &self.canonical_root,
                "inspect the retained write-root directory",
                error,
            )
        })?;
        let current = fs::symlink_metadata(&self.canonical_root).map_err(|error| {
            AtomicWriteError::io(&self.canonical_root, "revalidate the write root", error)
        })?;
        let opened_after = self.root_handle.metadata().map_err(|error| {
            AtomicWriteError::io(
                &self.canonical_root,
                "reinspect the retained write-root directory",
                error,
            )
        })?;
        if !opened_before.is_dir()
            || current.file_type().is_symlink()
            || !current.is_dir()
            || !opened_after.is_dir()
            || !self.root_identity.matches(&opened_before)
            || !self.root_identity.matches(&current)
            || !self.root_identity.matches(&opened_after)
        {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::ConcurrentModification,
                &self.canonical_root,
                "revalidate the retained write root",
            ));
        }
        Ok(())
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn validate_descriptor_root(&self, descriptor_root: &Path) -> Result<(), AtomicWriteError> {
        let opened_before = self.root_handle.metadata().map_err(|error| {
            AtomicWriteError::io(
                descriptor_root,
                "inspect the retained descriptor root",
                error,
            )
        })?;
        let anchored = fs::metadata(descriptor_root).map_err(|_| {
            AtomicWriteError::new(
                AtomicWriteErrorCode::UnsupportedPlatform,
                descriptor_root,
                "resolve the retained descriptor root through procfs",
            )
        })?;
        let opened_after = self.root_handle.metadata().map_err(|error| {
            AtomicWriteError::io(
                descriptor_root,
                "reinspect the retained descriptor root",
                error,
            )
        })?;
        if !opened_before.is_dir()
            || !anchored.is_dir()
            || !opened_after.is_dir()
            || !self.root_identity.matches(&opened_before)
            || !self.root_identity.matches(&anchored)
            || !self.root_identity.matches(&opened_after)
        {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::ConcurrentModification,
                descriptor_root,
                "bind the retained descriptor root",
            ));
        }
        Ok(())
    }
    fn validate_parent_chain(
        &self,
        parent: &Path,
    ) -> Result<Vec<DirectorySnapshot>, AtomicWriteError> {
        let relative_parent = parent.strip_prefix(&self.canonical_root).map_err(|_| {
            AtomicWriteError::new(
                AtomicWriteErrorCode::InvalidRelativePath,
                parent,
                "confine the destination parent",
            )
        })?;
        let root_metadata = fs::symlink_metadata(&self.canonical_root).map_err(|error| {
            AtomicWriteError::io(&self.canonical_root, "inspect the write root", error)
        })?;
        let mut snapshots = vec![DirectorySnapshot::new(
            self.canonical_root.clone(),
            &root_metadata,
        )];
        let mut current = self.canonical_root.clone();
        for component in relative_parent.components() {
            let Component::Normal(component) = component else {
                return Err(AtomicWriteError::new(
                    AtomicWriteErrorCode::InvalidRelativePath,
                    parent,
                    "validate the destination parent",
                ));
            };
            current.push(component);
            let metadata = match fs::symlink_metadata(&current) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Err(AtomicWriteError::new(
                        AtomicWriteErrorCode::UnsafeParent,
                        &current,
                        "use an existing destination parent",
                    ));
                }
                Err(error) => {
                    return Err(AtomicWriteError::io(
                        &current,
                        "inspect a destination ancestor",
                        error,
                    ));
                }
            };
            if metadata.file_type().is_symlink() {
                return Err(AtomicWriteError::new(
                    AtomicWriteErrorCode::SymlinkAncestor,
                    &current,
                    "validate a destination ancestor",
                ));
            }
            if !metadata.is_dir() {
                return Err(AtomicWriteError::new(
                    AtomicWriteErrorCode::UnsafeParent,
                    &current,
                    "validate a destination ancestor",
                ));
            }
            snapshots.push(DirectorySnapshot::new(current.clone(), &metadata));
        }
        let canonical_parent = fs::canonicalize(parent).map_err(|error| {
            AtomicWriteError::io(parent, "canonicalize the destination parent", error)
        })?;
        if canonical_parent != parent {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::SymlinkAncestor,
                parent,
                "confine the destination parent",
            ));
        }
        Ok(snapshots)
    }
}
#[cfg(unix)]
#[expect(
    clippy::too_many_lines,
    reason = "private-root creation keeps each ancestor identity and durability check adjacent to the filesystem operation it protects"
)]
fn create_or_open_private_root(root: &Path) -> Result<PathBuf, AtomicWriteError> {
    if !root.is_absolute()
        || root
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::InvalidRelativePath,
            root,
            "validate an absolute private write-root path",
        ));
    }
    if PLATFORM_SECURE_OPEN_FLAGS.is_none() {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::UnsupportedPlatform,
            root,
            "create a private write root with no-follow access",
        ));
    }
    let mut existing = root.to_path_buf();
    let mut missing = Vec::<OsString>::new();
    let linked_existing = loop {
        match fs::symlink_metadata(&existing) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(AtomicWriteError::new(
                        AtomicWriteErrorCode::UnsafeRoot,
                        &existing,
                        "bind the deepest existing private-root ancestor",
                    ));
                }
                break metadata;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let component = existing.file_name().ok_or_else(|| {
                    AtomicWriteError::new(
                        AtomicWriteErrorCode::InvalidRelativePath,
                        root,
                        "resolve a missing private-root component",
                    )
                })?;
                missing.push(component.to_os_string());
                existing = existing
                    .parent()
                    .ok_or_else(|| {
                        AtomicWriteError::new(
                            AtomicWriteErrorCode::InvalidRelativePath,
                            root,
                            "resolve the private-root ancestor",
                        )
                    })?
                    .to_path_buf();
            }
            Err(error) => {
                return Err(AtomicWriteError::io(
                    &existing,
                    "inspect a private-root ancestor",
                    error,
                ));
            }
        }
    };
    let canonical_existing = fs::canonicalize(&existing).map_err(|error| {
        AtomicWriteError::io(&existing, "canonicalize the private-root ancestor", error)
    })?;
    let canonical_metadata = fs::symlink_metadata(&canonical_existing).map_err(|error| {
        AtomicWriteError::io(
            &canonical_existing,
            "inspect the canonical private-root ancestor",
            error,
        )
    })?;
    if canonical_metadata.file_type().is_symlink()
        || !canonical_metadata.is_dir()
        || !same_file(&linked_existing, &canonical_metadata)
    {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            &existing,
            "bind the canonical private-root ancestor",
        ));
    }
    let mut current = canonical_existing;
    for component in missing.into_iter().rev() {
        let parent_metadata = fs::symlink_metadata(&current).map_err(|error| {
            AtomicWriteError::io(&current, "inspect a private-root parent", error)
        })?;
        if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::UnsafeParent,
                &current,
                "validate a private-root parent",
            ));
        }
        let parent_snapshot = DirectorySnapshot::new(current.clone(), &parent_metadata);
        let next = current.join(component);
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        match builder.create(&next) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(AtomicWriteError::io(
                    &next,
                    "create a private write-root component",
                    error,
                ));
            }
        }
        let linked = fs::symlink_metadata(&next).map_err(|error| {
            AtomicWriteError::io(&next, "inspect a private write-root component", error)
        })?;
        if !private_root_metadata_is_safe(&linked) {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::UnsafeRoot,
                &next,
                "validate a private write-root component",
            ));
        }
        validate_directory_snapshots(std::slice::from_ref(&parent_snapshot))?;
        let directory = open_private_directory_no_follow(&next, &linked)?;
        directory.sync_all().map_err(|error| {
            AtomicWriteError::io(&next, "synchronize a private write-root component", error)
        })?;
        sync_directory(&current, &parent_snapshot)?;
        current = next;
    }
    let linked = fs::symlink_metadata(&current)
        .map_err(|error| AtomicWriteError::io(&current, "inspect the private write root", error))?;
    if !private_root_metadata_is_safe(&linked) {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::UnsafeRoot,
            &current,
            "validate the private write root",
        ));
    }
    let _directory = open_private_directory_no_follow(&current, &linked)?;
    Ok(current)
}
#[cfg(unix)]
fn open_private_directory_no_follow(
    path: &Path,
    linked: &fs::Metadata,
) -> Result<File, AtomicWriteError> {
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(
        PLATFORM_SECURE_OPEN_FLAGS
            .expect("Unix private-root creation requires no-follow/nonblocking support"),
    );
    let directory = options.open(path).map_err(|error| {
        AtomicWriteError::io(path, "open a private write-root directory", error)
    })?;
    let opened = directory.metadata().map_err(|error| {
        AtomicWriteError::io(path, "inspect an open private write-root directory", error)
    })?;
    let after = fs::symlink_metadata(path).map_err(|error| {
        AtomicWriteError::io(path, "reinspect a private write-root directory", error)
    })?;
    if !private_root_metadata_is_safe(linked)
        || !private_root_metadata_is_safe(&opened)
        || !private_root_metadata_is_safe(&after)
        || !same_file(linked, &opened)
        || !same_file(&opened, &after)
    {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            path,
            "bind an open private write-root directory",
        ));
    }
    Ok(directory)
}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_directory_no_follow(path: &Path, linked: &fs::Metadata) -> Result<File, AtomicWriteError> {
    let secure_open_flags = PLATFORM_SECURE_OPEN_FLAGS.ok_or_else(|| {
        AtomicWriteError::new(
            AtomicWriteErrorCode::UnsupportedPlatform,
            path,
            "retain a no-follow write-root directory handle",
        )
    })?;
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(secure_open_flags);
    let directory = options.open(path).map_err(|error| {
        AtomicWriteError::io(path, "retain the write-root directory handle", error)
    })?;
    let opened = directory.metadata().map_err(|error| {
        AtomicWriteError::io(path, "inspect the retained write-root directory", error)
    })?;
    let after = fs::symlink_metadata(path).map_err(|error| {
        AtomicWriteError::io(path, "reinspect the retained write-root directory", error)
    })?;
    if linked.file_type().is_symlink()
        || !linked.is_dir()
        || !opened.is_dir()
        || after.file_type().is_symlink()
        || !after.is_dir()
        || !same_file(linked, &opened)
        || !same_file(&opened, &after)
    {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            path,
            "bind the retained write-root directory",
        ));
    }
    Ok(directory)
}
#[cfg(unix)]
fn private_root_metadata_is_safe(metadata: &fs::Metadata) -> bool {
    metadata.is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.permissions().mode() & 0o7777 == 0o700
}
fn validate_relative_path(relative: &Path) -> Result<(), AtomicWriteError> {
    let mut count = 0_usize;
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::InvalidRelativePath,
                relative,
                "validate a root-relative destination",
            ));
        }
        count += 1;
    }
    if count == 0 {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::InvalidRelativePath,
            relative,
            "validate a non-empty destination",
        ));
    }
    Ok(())
}
#[derive(Debug)]
struct DirectorySnapshot {
    path: PathBuf,
    identity: FileIdentity,
}
impl DirectorySnapshot {
    fn new(path: PathBuf, metadata: &fs::Metadata) -> Self {
        Self {
            path,
            identity: FileIdentity::from_metadata(metadata),
        }
    }
}
fn validate_directory_snapshots(snapshots: &[DirectorySnapshot]) -> Result<(), AtomicWriteError> {
    for snapshot in snapshots {
        let metadata = fs::symlink_metadata(&snapshot.path).map_err(|error| {
            AtomicWriteError::io(&snapshot.path, "revalidate a destination ancestor", error)
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || !snapshot.identity.matches(&metadata)
        {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::ConcurrentModification,
                &snapshot.path,
                "revalidate a destination ancestor",
            ));
        }
    }
    Ok(())
}
#[derive(Debug)]
enum TargetSnapshot {
    Absent,
    Present {
        identity: FileIdentity,
        permissions: fs::Permissions,
    },
}
impl TargetSnapshot {
    fn permissions(&self) -> Option<fs::Permissions> {
        match self {
            Self::Absent => None,
            Self::Present { permissions, .. } => Some(permissions.clone()),
        }
    }
}
fn inspect_target(target: &Path) -> Result<TargetSnapshot, AtomicWriteError> {
    match fs::symlink_metadata(target) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || has_multiple_hard_links(&metadata)
            {
                return Err(AtomicWriteError::new(
                    AtomicWriteErrorCode::UnsafeTarget,
                    target,
                    "validate the destination",
                ));
            }
            Ok(TargetSnapshot::Present {
                identity: FileIdentity::from_metadata(&metadata),
                permissions: metadata.permissions(),
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(TargetSnapshot::Absent),
        Err(error) => Err(AtomicWriteError::io(
            target,
            "inspect the destination",
            error,
        )),
    }
}
fn validate_target_snapshot(
    target: &Path,
    snapshot: &TargetSnapshot,
) -> Result<(), AtomicWriteError> {
    match (snapshot, fs::symlink_metadata(target)) {
        (TargetSnapshot::Absent, Err(error)) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        (TargetSnapshot::Present { identity, .. }, Ok(metadata))
            if metadata.is_file()
                && !metadata.file_type().is_symlink()
                && !has_multiple_hard_links(&metadata)
                && identity.matches(&metadata) =>
        {
            Ok(())
        }
        (_, Err(error)) if error.kind() != io::ErrorKind::NotFound => Err(AtomicWriteError::io(
            target,
            "revalidate the destination",
            error,
        )),
        _ => Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            target,
            "revalidate the destination",
        )),
    }
}
fn bind_existing_immutable_target(
    target: &Path,
    contents: &[u8],
) -> Result<FileIdentity, AtomicWriteError> {
    let metadata = match fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::ConcurrentModification,
                target,
                "revalidate the immutable destination",
            ));
        }
        Err(error) => {
            return Err(AtomicWriteError::io(
                target,
                "inspect the immutable destination",
                error,
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::UnsafeTarget,
            target,
            "validate the immutable destination",
        ));
    }
    if has_multiple_hard_links(&metadata) {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::UnsafeTarget,
            target,
            "reject a hard-linked immutable destination",
        ));
    }
    let identity = FileIdentity::from_metadata(&metadata);
    if readback_immutable_target(target, contents, Some(identity))? {
        Ok(identity)
    } else {
        Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ImmutableConflict,
            target,
            "preserve existing immutable bytes",
        ))
    }
}
fn revalidate_existing_immutable_target(
    target: &Path,
    contents: &[u8],
    expected_identity: FileIdentity,
) -> Result<(), AtomicWriteError> {
    if readback_immutable_target(target, contents, Some(expected_identity))? {
        Ok(())
    } else {
        Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            target,
            "revalidate exact immutable bytes after directory synchronization",
        ))
    }
}
fn validate_new_immutable_link(
    target: &Path,
    pending: &PendingTemp,
    expected_links: u64,
) -> Result<(), AtomicWriteError> {
    pending.validate_path_identity_with_links(expected_links)?;
    let target_metadata = fs::symlink_metadata(target).map_err(|error| {
        AtomicWriteError::io(
            target,
            "inspect the newly linked immutable destination",
            error,
        )
    })?;
    if target_metadata.file_type().is_symlink()
        || !target_metadata.is_file()
        || hard_link_count(&target_metadata) != expected_links
        || !pending.identity.matches(&target_metadata)
    {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            target,
            "bind the newly linked immutable destination",
        ));
    }
    Ok(())
}
fn validate_installed_immutable_target(
    target: &Path,
    expected_identity: FileIdentity,
) -> Result<(), AtomicWriteError> {
    let metadata = fs::symlink_metadata(target).map_err(|error| {
        AtomicWriteError::io(target, "inspect the installed immutable destination", error)
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || has_multiple_hard_links(&metadata)
        || !expected_identity.matches(&metadata)
    {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            target,
            "revalidate the installed immutable destination",
        ));
    }
    Ok(())
}
fn readback_immutable_target(
    target: &Path,
    contents: &[u8],
    expected_identity: Option<FileIdentity>,
) -> Result<bool, AtomicWriteError> {
    match read_immutable_target_bounded(target, contents.len(), expected_identity)? {
        ImmutableReadOutcome::Within(observed) => Ok(observed == contents),
        ImmutableReadOutcome::Exceeded => Ok(false),
    }
}
enum ImmutableReadOutcome {
    Within(Vec<u8>),
    Exceeded,
}
#[expect(
    clippy::too_many_lines,
    reason = "bounded immutable readback keeps the pre-open, open-handle, and post-read identity checks together as one fail-closed state machine"
)]
fn read_immutable_target_bounded(
    target: &Path,
    max_bytes: usize,
    expected_identity: Option<FileIdentity>,
) -> Result<ImmutableReadOutcome, AtomicWriteError> {
    let linked_before = inspect_single_link_immutable_target(target)?;
    if expected_identity.is_some_and(|identity| !identity.matches(&linked_before)) {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            target,
            "bind immutable readback to the installed inode",
        ));
    }
    let secure_open_flags = PLATFORM_SECURE_OPEN_FLAGS.ok_or_else(|| {
        AtomicWriteError::new(
            AtomicWriteErrorCode::UnsupportedPlatform,
            target,
            "open the immutable destination without following links or blocking",
        )
    })?;
    #[cfg(all(test, unix))]
    substitute_immutable_read_target_with_fifo_for_test(target)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(secure_open_flags);
    #[cfg(not(unix))]
    let _ = secure_open_flags;
    let mut file = match options.open(target) {
        Ok(file) => file,
        Err(error) => {
            match fs::symlink_metadata(target) {
                Ok(metadata)
                    if metadata.file_type().is_symlink()
                        || !metadata.is_file()
                        || has_multiple_hard_links(&metadata) =>
                {
                    return Err(AtomicWriteError::new(
                        AtomicWriteErrorCode::UnsafeTarget,
                        target,
                        "open a safe immutable destination without following links",
                    ));
                }
                Ok(metadata) if !same_file(&linked_before, &metadata) => {
                    return Err(AtomicWriteError::new(
                        AtomicWriteErrorCode::ConcurrentModification,
                        target,
                        "bind immutable readback after an open failure",
                    ));
                }
                Err(inspect_error) if inspect_error.kind() == io::ErrorKind::NotFound => {
                    return Err(AtomicWriteError::new(
                        AtomicWriteErrorCode::ConcurrentModification,
                        target,
                        "bind immutable readback after an open failure",
                    ));
                }
                Ok(_) | Err(_) => {}
            }
            return Err(AtomicWriteError::io(
                target,
                "open the immutable destination without following links",
                error,
            ));
        }
    };
    let opened_before = file.metadata().map_err(|error| {
        AtomicWriteError::io(target, "inspect the open immutable destination", error)
    })?;
    if !opened_before.is_file()
        || has_multiple_hard_links(&opened_before)
        || !same_file(&linked_before, &opened_before)
        || expected_identity.is_some_and(|identity| !identity.matches(&opened_before))
    {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            target,
            "bind the open immutable destination",
        ));
    }
    let max_bytes_u64 = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    let metadata_exceeds_bound = opened_before.len() > max_bytes_u64;
    let read_limit = max_bytes_u64.saturating_add(1);
    let mut observed = Vec::with_capacity(max_bytes.min(64 * 1024));
    if !metadata_exceeds_bound {
        (&mut file)
            .take(read_limit)
            .read_to_end(&mut observed)
            .map_err(|error| AtomicWriteError::io(target, "read immutable bytes", error))?;
    }
    let opened_after = file.metadata().map_err(|error| {
        AtomicWriteError::io(target, "reinspect the open immutable destination", error)
    })?;
    let linked_after = inspect_single_link_immutable_target(target)?;
    if !same_file(&opened_before, &opened_after)
        || !same_file(&opened_after, &linked_after)
        || opened_before.len() != opened_after.len()
        || expected_identity.is_some_and(|identity| !identity.matches(&opened_after))
    {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            target,
            "revalidate immutable readback",
        ));
    }
    if metadata_exceeds_bound || observed.len() > max_bytes {
        return Ok(ImmutableReadOutcome::Exceeded);
    }
    if u64::try_from(observed.len()).unwrap_or(u64::MAX) != opened_after.len() {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            target,
            "verify the immutable readback length",
        ));
    }
    Ok(ImmutableReadOutcome::Within(observed))
}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn read_private_immutable_target_bounded(
    target: &Path,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, AtomicWriteError> {
    let before = match fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match fs::symlink_metadata(target) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Ok(_) => {
                    return Err(AtomicWriteError::new(
                        AtomicWriteErrorCode::ConcurrentModification,
                        target,
                        "revalidate an absent descriptor-rooted target",
                    ));
                }
                Err(error) => {
                    return Err(AtomicWriteError::io(
                        target,
                        "reinspect an absent descriptor-rooted target",
                        error,
                    ));
                }
            }
            #[cfg(test)]
            run_test_after_descriptor_target_bind();
            return match fs::symlink_metadata(target) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                Ok(_) => Err(AtomicWriteError::new(
                    AtomicWriteErrorCode::ConcurrentModification,
                    target,
                    "verify descriptor-rooted target absence",
                )),
                Err(error) => Err(AtomicWriteError::io(
                    target,
                    "verify descriptor-rooted target absence",
                    error,
                )),
            };
        }
        Err(error) => {
            return Err(AtomicWriteError::io(
                target,
                "inspect the descriptor-rooted target",
                error,
            ));
        }
    };
    validate_single_link_immutable_metadata(target, &before)?;
    if before.permissions().mode() & 0o077 != 0 {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::UnsafeTarget,
            target,
            "validate private descriptor-rooted target permissions",
        ));
    }
    let identity = FileIdentity::from_metadata(&before);
    #[cfg(test)]
    run_test_after_descriptor_target_bind();
    let read = read_immutable_target_bounded(target, max_bytes, Some(identity))?;
    let after = inspect_single_link_immutable_target(target)?;
    if after.permissions().mode() & 0o077 != 0 || !same_immutable_file_snapshot(&before, &after) {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            target,
            "revalidate the private descriptor-rooted target",
        ));
    }
    match read {
        ImmutableReadOutcome::Within(bytes) => Ok(Some(bytes)),
        ImmutableReadOutcome::Exceeded => Err(AtomicWriteError::new(
            AtomicWriteErrorCode::UnsafeTarget,
            target,
            "enforce the descriptor-rooted target size bound",
        )),
    }
}
#[cfg(any(target_os = "linux", target_os = "android"))]
fn same_immutable_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.nlink() == right.nlink()
        && left.mode() == right.mode()
}
fn inspect_single_link_immutable_target(target: &Path) -> Result<fs::Metadata, AtomicWriteError> {
    let metadata = match fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::ConcurrentModification,
                target,
                "revalidate the immutable destination",
            ));
        }
        Err(error) => {
            return Err(AtomicWriteError::io(
                target,
                "inspect the immutable destination",
                error,
            ));
        }
    };
    validate_single_link_immutable_metadata(target, &metadata)?;
    Ok(metadata)
}
fn validate_single_link_immutable_metadata(
    target: &Path,
    metadata: &fs::Metadata,
) -> Result<(), AtomicWriteError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() || has_multiple_hard_links(metadata)
    {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::UnsafeTarget,
            target,
            "validate immutable readback target",
        ));
    }
    Ok(())
}
fn cleanup_pending_and_sync(
    mut pending: PendingTemp,
    expected_links: u64,
    parent: &Path,
    parent_snapshot: &DirectorySnapshot,
) -> Result<(), AtomicWriteError> {
    let mut cleanup_error = None;
    for _ in 0..POST_LINK_CLEANUP_ATTEMPTS {
        match pending.remove_owned(expected_links) {
            Ok(()) => {
                cleanup_error = None;
                break;
            }
            Err(error) => {
                cleanup_error = Some(error);
                if !pending.armed {
                    break;
                }
            }
        }
    }
    if pending.armed {
        // Leave an unproven entry in place rather than let Drop perform an unreported,
        // post-synchronization pathname deletion.
        pending.disarm();
    }
    let cleanup = cleanup_error.map_or(Ok(()), Err);
    let sync = sync_directory_bounded(parent, parent_snapshot);
    preserve_primary_result(cleanup, sync)
}
fn recover_post_link_temp_and_sync(
    pending: &mut PendingTemp,
    parent: &Path,
    parent_snapshot: &DirectorySnapshot,
) -> Result<(), AtomicWriteError> {
    let mut cleanup_error = None;
    for _ in 0..POST_LINK_CLEANUP_ATTEMPTS {
        if !pending.armed {
            break;
        }
        match pending.remove_owned_exact() {
            Ok(()) => {
                cleanup_error = None;
                break;
            }
            Err(error) => cleanup_error = Some(error),
        }
    }
    if pending.armed {
        // Preserve the exact path for explicit recovery. Drop must not make a third,
        // unreported attempt after the final parent-directory synchronization.
        pending.disarm();
    }
    let cleanup = cleanup_error.map_or(Ok(()), Err);
    let sync = sync_directory_bounded(parent, parent_snapshot);
    preserve_primary_result(cleanup, sync)
}
fn sync_directory_bounded(
    parent: &Path,
    parent_snapshot: &DirectorySnapshot,
) -> Result<(), AtomicWriteError> {
    let mut last_error = None;
    for _ in 0..POST_LINK_CLEANUP_ATTEMPTS {
        match sync_directory(parent, parent_snapshot) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.expect("the bounded synchronization loop executes at least once"))
}
fn preserve_primary_result<T>(
    primary: Result<T, AtomicWriteError>,
    recovery: Result<(), AtomicWriteError>,
) -> Result<T, AtomicWriteError> {
    match (primary, recovery) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(_), Err(recovery)) => Err(recovery),
        (Err(primary), Err(recovery)) => Err(primary.with_recovery_failure(&recovery)),
    }
}
struct PendingTemp {
    path: PathBuf,
    file: File,
    identity: FileIdentity,
    armed: bool,
}
impl PendingTemp {
    fn create(parent: &Path) -> Result<Self, AtomicWriteError> {
        let process = std::process::id();
        for _ in 0..TEMP_CREATE_ATTEMPTS {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let mut name = OsString::from(TEMP_FILE_PREFIX);
            name.push(process.to_string());
            name.push("-");
            name.push(sequence.to_string());
            let path = parent.join(name);
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            match options.open(&path) {
                Ok(file) => {
                    let metadata = file.metadata().map_err(|error| {
                        AtomicWriteError::io(&path, "inspect the temporary file", error)
                    })?;
                    return Ok(Self {
                        path,
                        file,
                        identity: FileIdentity::from_metadata(&metadata),
                        armed: true,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(AtomicWriteError::io(
                        &path,
                        "create a private temporary file",
                        error,
                    ));
                }
            }
        }
        Err(AtomicWriteError::new(
            AtomicWriteErrorCode::TemporaryNameExhausted,
            parent,
            "create a private temporary file",
        ))
    }
    fn validate_path_identity(&self) -> Result<(), AtomicWriteError> {
        self.validate_path_identity_with_links(1)
    }
    fn validate_path_identity_with_links(
        &self,
        expected_links: u64,
    ) -> Result<(), AtomicWriteError> {
        if self.owned_link_count()? != expected_links {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::ConcurrentModification,
                &self.path,
                "revalidate the temporary-file link count",
            ));
        }
        Ok(())
    }
    fn owned_link_count(&self) -> Result<u64, AtomicWriteError> {
        if !self.has_owned_name() {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::ConcurrentModification,
                &self.path,
                "validate the private temporary-file name",
            ));
        }
        let linked = fs::symlink_metadata(&self.path).map_err(|error| {
            AtomicWriteError::io(&self.path, "revalidate the temporary file", error)
        })?;
        let opened = self.file.metadata().map_err(|error| {
            AtomicWriteError::io(&self.path, "inspect the open temporary file", error)
        })?;
        if linked.file_type().is_symlink()
            || !linked.is_file()
            || !opened.is_file()
            || !self.identity.matches(&linked)
            || !self.identity.matches(&opened)
            || !same_file(&linked, &opened)
            || hard_link_count(&linked) == 0
            || hard_link_count(&linked) != hard_link_count(&opened)
        {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::ConcurrentModification,
                &self.path,
                "revalidate the temporary file",
            ));
        }
        Ok(hard_link_count(&opened))
    }
    fn remove_owned(&mut self, expected_links: u64) -> Result<(), AtomicWriteError> {
        self.validate_path_identity_with_links(expected_links)?;
        self.remove_validated_owned(expected_links)
    }
    fn remove_owned_exact(&mut self) -> Result<(), AtomicWriteError> {
        let links = self.owned_link_count()?;
        self.remove_validated_owned(links)
    }
    fn remove_validated_owned(&mut self, expected_links: u64) -> Result<(), AtomicWriteError> {
        fs::remove_file(&self.path).map_err(|error| {
            AtomicWriteError::io(&self.path, "remove the owned temporary file", error)
        })?;
        self.armed = false;
        let opened = self.file.metadata().map_err(|error| {
            AtomicWriteError::io(&self.path, "reinspect the unlinked temporary file", error)
        })?;
        if !opened.is_file()
            || !self.identity.matches(&opened)
            || hard_link_count(&opened) != expected_links.saturating_sub(1)
        {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::ConcurrentModification,
                &self.path,
                "verify temporary-file unlink",
            ));
        }
        Ok(())
    }
    fn has_owned_name(&self) -> bool {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(TEMP_FILE_PREFIX))
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for PendingTemp {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if !self.has_owned_name() {
            return;
        }
        let Ok(metadata) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.is_file()
            && !metadata.file_type().is_symlink()
            && cleanup_identity_matches(&self.identity, &metadata)
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}
fn sync_directory(path: &Path, snapshot: &DirectorySnapshot) -> Result<(), AtomicWriteError> {
    #[cfg(all(test, unix))]
    if TEST_DIRECTORY_SYNC_FAILURES.with(|remaining| {
        let count = remaining.get();
        if count == 0 {
            false
        } else {
            remaining.set(count - 1);
            true
        }
    }) {
        return Err(AtomicWriteError::io(
            path,
            "synchronize the destination directory",
            io::Error::other("injected directory synchronization failure"),
        ));
    }
    let secure_open_flags = PLATFORM_SECURE_OPEN_FLAGS.ok_or_else(|| {
        AtomicWriteError::new(
            AtomicWriteErrorCode::UnsupportedPlatform,
            path,
            "open the destination directory without following links or blocking",
        )
    })?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(secure_open_flags);
    #[cfg(not(unix))]
    let _ = secure_open_flags;
    let directory = options
        .open(path)
        .map_err(|error| AtomicWriteError::io(path, "open the destination directory", error))?;
    let metadata = directory.metadata().map_err(|error| {
        AtomicWriteError::io(path, "inspect the open destination directory", error)
    })?;
    if !metadata.is_dir() || !snapshot.identity.matches(&metadata) {
        return Err(AtomicWriteError::new(
            AtomicWriteErrorCode::ConcurrentModification,
            path,
            "revalidate the open destination directory",
        ));
    }
    directory
        .sync_all()
        .map_err(|error| AtomicWriteError::io(path, "synchronize the destination directory", error))
}
#[cfg(all(test, unix))]
fn substitute_immutable_read_target_with_fifo_for_test(
    path: &Path,
) -> Result<(), AtomicWriteError> {
    let substitute = TEST_IMMUTABLE_READ_FIFO_SUBSTITUTIONS.with(|remaining| {
        let count = remaining.get();
        remaining.set(count.saturating_sub(1));
        count != 0
    });
    if !substitute {
        return Ok(());
    }
    fs::remove_file(path).map_err(|error| {
        AtomicWriteError::io(
            path,
            "remove the immutable FIFO substitution fixture",
            error,
        )
    })?;
    let status = std::process::Command::new("mkfifo")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| {
            AtomicWriteError::io(
                path,
                "create the immutable FIFO substitution fixture",
                error,
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(AtomicWriteError::io(
            path,
            "create the immutable FIFO substitution fixture",
            io::Error::other("mkfifo returned an unsuccessful status"),
        ))
    }
}
#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct FileIdentity {
    device: u64,
    inode: u64,
}
#[cfg(unix)]
impl FileIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    fn matches(self, metadata: &fs::Metadata) -> bool {
        self.device == metadata.dev() && self.inode == metadata.ino()
    }
}
#[cfg(not(unix))]
#[derive(Clone, Copy, Debug)]
struct FileIdentity;
#[cfg(not(unix))]
impl FileIdentity {
    fn from_metadata(_metadata: &fs::Metadata) -> Self {
        Self
    }
    fn matches(self, _metadata: &fs::Metadata) -> bool {
        true
    }
}
#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(not(unix))]
fn same_file(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    true
}
#[cfg(unix)]
fn cleanup_identity_matches(identity: &FileIdentity, metadata: &fs::Metadata) -> bool {
    identity.matches(metadata)
}
#[cfg(not(unix))]
fn cleanup_identity_matches(_identity: &FileIdentity, _metadata: &fs::Metadata) -> bool {
    // Without a stable std-only file identity, leaking a private temporary file
    // is safer than deleting a path that another process may have substituted.
    false
}
#[cfg(unix)]
fn has_multiple_hard_links(metadata: &fs::Metadata) -> bool {
    metadata.nlink() != 1
}
#[cfg(not(unix))]
fn has_multiple_hard_links(_metadata: &fs::Metadata) -> bool {
    false
}
#[cfg(unix)]
fn hard_link_count(metadata: &fs::Metadata) -> u64 {
    metadata.nlink()
}
#[cfg(not(unix))]
fn hard_link_count(_metadata: &fs::Metadata) -> u64 {
    1
}
#[cfg(all(
    target_os = "android",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
        target_arch = "x86",
        target_arch = "x86_64"
    ))
))]
compile_error!("Musubi atomic file reads are not qualified for this Android architecture");
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("Musubi atomic file reads are not qualified for this Unix target");
#[cfg(all(target_os = "android", target_arch = "riscv64"))]
const fn platform_no_follow_flag() -> i32 {
    0x400000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
const fn platform_no_follow_flag() -> i32 {
    0x8000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
const fn platform_no_follow_flag() -> i32 {
    0x20000
}
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x8000
}
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
const fn platform_no_follow_flag() -> i32 {
    0x20000
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x100
}
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6"
    )
))]
const fn platform_nonblocking_flag() -> i32 {
    0x80
}
#[cfg(all(
    target_os = "linux",
    any(target_arch = "sparc", target_arch = "sparc64")
))]
const fn platform_nonblocking_flag() -> i32 {
    0x4000
}
#[cfg(any(
    target_os = "android",
    all(
        target_os = "linux",
        not(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "sparc",
            target_arch = "sparc64"
        ))
    )
))]
const fn platform_nonblocking_flag() -> i32 {
    0x800
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn platform_nonblocking_flag() -> i32 {
    0x4
}
#[cfg(unix)]
const PLATFORM_SECURE_OPEN_FLAGS: Option<i32> =
    Some(platform_no_follow_flag() | platform_nonblocking_flag());
#[cfg(not(unix))]
const PLATFORM_SECURE_OPEN_FLAGS: Option<i32> = None;
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    struct DirectorySyncFailureReset;
    #[cfg(unix)]
    impl Drop for DirectorySyncFailureReset {
        fn drop(&mut self) {
            TEST_DIRECTORY_SYNC_FAILURES.with(|remaining| remaining.set(0));
        }
    }
    #[cfg(unix)]
    fn inject_directory_sync_failures(count: usize) -> DirectorySyncFailureReset {
        TEST_DIRECTORY_SYNC_FAILURES.with(|remaining| remaining.set(count));
        DirectorySyncFailureReset
    }
    #[cfg(unix)]
    #[test]
    fn creates_and_replaces_a_root_confined_file() {
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        writer
            .replace(Path::new("Musubi.lock"), b"schema = \"musubi-lock\"\n")
            .expect("create lockfile");
        writer
            .replace(
                Path::new("Musubi.lock"),
                b"schema = \"musubi-lock\"\nversion = 1\n",
            )
            .expect("replace lockfile");
        assert_eq!(
            fs::read(root.path().join("Musubi.lock")).expect("read lockfile"),
            b"schema = \"musubi-lock\"\nversion = 1\n"
        );
        assert!(fs::read_dir(root.path()).expect("read root").all(|entry| {
            !entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".musubi-tmp-")
        }));
    }
    #[cfg(unix)]
    #[test]
    fn creates_and_reopens_a_nested_private_write_root() {
        use std::os::unix::fs::PermissionsExt as _;
        let holder = tempfile::tempdir().expect("private-root holder");
        let requested = holder.path().join("state/iroha/musubi");
        let writer = AtomicWriteRoot::open_or_create_private(&requested)
            .expect("create nested private root");
        assert_eq!(
            writer.path(),
            fs::canonicalize(&requested)
                .expect("canonical private root")
                .as_path()
        );
        for relative in ["state", "state/iroha", "state/iroha/musubi"] {
            let metadata = fs::symlink_metadata(holder.path().join(relative))
                .expect("private component metadata");
            assert!(metadata.is_dir());
            assert!(!metadata.file_type().is_symlink());
            assert_eq!(metadata.permissions().mode() & 0o7777, 0o700);
        }
        let reopened = AtomicWriteRoot::open_or_create_private(&requested)
            .expect("reopen identical private root");
        assert_eq!(reopened.path(), writer.path());
    }
    #[cfg(unix)]
    #[test]
    fn private_write_root_creation_rejects_links_and_nonprivate_existing_roots() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let outside = tempfile::tempdir().expect("outside root");
        let holder = tempfile::tempdir().expect("private-root holder");
        let linked_parent = holder.path().join("linked-state");
        symlink(outside.path(), &linked_parent).expect("linked state parent");
        let error = AtomicWriteRoot::open_or_create_private(&linked_parent.join("musubi"))
            .expect_err("linked ancestor rejected");
        assert_eq!(error.code(), AtomicWriteErrorCode::UnsafeRoot);
        assert!(!outside.path().join("musubi").exists());
        let public_root = holder.path().join("public-state");
        fs::create_dir(&public_root).expect("public state root");
        fs::set_permissions(&public_root, fs::Permissions::from_mode(0o755))
            .expect("make state root nonprivate");
        let error = AtomicWriteRoot::open_or_create_private(&public_root)
            .expect_err("nonprivate existing root rejected");
        assert_eq!(error.code(), AtomicWriteErrorCode::UnsafeRoot);
        assert_eq!(
            fs::symlink_metadata(&public_root)
                .expect("preserved public root")
                .permissions()
                .mode()
                & 0o7777,
            0o755
        );
    }
    #[cfg(unix)]
    #[test]
    fn installs_immutable_file_privately_and_idempotently() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        let relative = Path::new("checkpoints/release.norito");
        fs::create_dir(root.path().join("checkpoints")).expect("checkpoint directory");
        assert_eq!(
            writer
                .install_immutable(relative, b"exact-release-checkpoint")
                .expect("first immutable install"),
            AtomicInstallOutcome::Installed
        );
        assert_eq!(
            writer
                .install_immutable(relative, b"exact-release-checkpoint")
                .expect("idempotent immutable install"),
            AtomicInstallOutcome::AlreadyPresent
        );
        let target = root.path().join(relative);
        assert_eq!(
            fs::read(&target).expect("read immutable target"),
            b"exact-release-checkpoint"
        );
        assert_eq!(
            fs::metadata(&target)
                .expect("immutable target metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(
            fs::read_dir(target.parent().expect("target parent"))
                .expect("read target parent")
                .all(|entry| {
                    !entry
                        .expect("directory entry")
                        .file_name()
                        .to_string_lossy()
                        .starts_with(TEMP_FILE_PREFIX)
                })
        );
    }
    #[cfg(unix)]
    #[test]
    fn immutable_retry_resynchronizes_an_exact_destination_after_interrupted_install() {
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        let relative = Path::new("checkpoints/release.norito");
        fs::create_dir(root.path().join("checkpoints")).expect("checkpoint directory");
        let _reset = inject_directory_sync_failures(POST_LINK_CLEANUP_ATTEMPTS + 1);
        let first_error = writer
            .install_immutable(relative, b"exact-release-checkpoint")
            .expect_err("post-link directory synchronization failure must be reported");
        assert_eq!(first_error.code(), AtomicWriteErrorCode::Io);
        assert!(
            first_error
                .to_string()
                .contains("cleanup or parent-directory durability was not proven")
        );
        TEST_DIRECTORY_SYNC_FAILURES.with(|remaining| assert_eq!(remaining.get(), 0));
        let target = root.path().join(relative);
        assert_eq!(
            fs::read(&target).expect("read installed target after reported failure"),
            b"exact-release-checkpoint"
        );
        assert_eq!(
            hard_link_count(&fs::metadata(&target).expect("single-link target metadata")),
            1
        );
        TEST_DIRECTORY_SYNC_FAILURES.with(|remaining| remaining.set(POST_LINK_CLEANUP_ATTEMPTS));
        writer
            .install_immutable(relative, b"exact-release-checkpoint")
            .expect_err("exact-existing retry must prove parent-directory durability");
        TEST_DIRECTORY_SYNC_FAILURES.with(|remaining| assert_eq!(remaining.get(), 0));
        TEST_DIRECTORY_SYNC_FAILURES.with(|remaining| remaining.set(0));
        assert_eq!(
            writer
                .install_immutable(relative, b"exact-release-checkpoint")
                .expect("durable exact-existing retry"),
            AtomicInstallOutcome::AlreadyPresent
        );
    }
    #[cfg(unix)]
    #[test]
    fn loads_an_installed_immutable_file_within_the_bound() {
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        let relative = Path::new("checkpoints/release.norito");
        let contents = b"exact-release-checkpoint";
        fs::create_dir(root.path().join("checkpoints")).expect("checkpoint directory");
        writer
            .install_immutable(relative, contents)
            .expect("install immutable checkpoint");
        assert_eq!(
            writer
                .load_immutable(relative, contents.len())
                .expect("load immutable checkpoint"),
            Some(contents.to_vec())
        );
    }
    #[cfg(unix)]
    #[test]
    fn immutable_load_rejects_a_fifo_substitution_without_blocking() {
        use std::os::unix::fs::FileTypeExt as _;
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        let relative = Path::new("checkpoints/release.norito");
        let contents = b"exact-release-checkpoint";
        fs::create_dir(root.path().join("checkpoints")).expect("checkpoint directory");
        writer
            .install_immutable(relative, contents)
            .expect("install immutable checkpoint");
        TEST_IMMUTABLE_READ_FIFO_SUBSTITUTIONS.with(|remaining| remaining.set(1));
        let error = writer
            .load_immutable(relative, contents.len())
            .expect_err("a FIFO substituted after inspection must fail without blocking");
        assert_eq!(error.code(), AtomicWriteErrorCode::ConcurrentModification);
        assert!(
            fs::symlink_metadata(root.path().join(relative))
                .expect("substituted FIFO metadata")
                .file_type()
                .is_fifo()
        );
        TEST_IMMUTABLE_READ_FIFO_SUBSTITUTIONS.with(|remaining| assert_eq!(remaining.get(), 0));
    }
    #[cfg(unix)]
    #[test]
    fn loading_a_missing_immutable_file_returns_none() {
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        fs::create_dir(root.path().join("checkpoints")).expect("checkpoint directory");
        assert_eq!(
            writer
                .load_immutable(Path::new("checkpoints/missing.norito"), 1024)
                .expect("load absent immutable checkpoint"),
            None
        );
    }
    #[cfg(unix)]
    #[test]
    fn immutable_load_rejects_oversized_and_unsafe_targets() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        writer
            .install_immutable(Path::new("oversized"), b"four")
            .expect("install oversized fixture");
        let oversized = writer
            .load_immutable(Path::new("oversized"), 3)
            .expect_err("oversized immutable target rejected");
        assert_eq!(oversized.code(), AtomicWriteErrorCode::UnsafeTarget);
        let outside = tempfile::tempdir().expect("outside root");
        let outside_file = outside.path().join("outside");
        fs::write(&outside_file, b"outside").expect("outside file");
        symlink(&outside_file, root.path().join("unsafe-link")).expect("target symlink");
        let unsafe_target = writer
            .load_immutable(Path::new("unsafe-link"), 1024)
            .expect_err("symlink immutable target rejected");
        assert_eq!(unsafe_target.code(), AtomicWriteErrorCode::UnsafeTarget);
    }
    #[cfg(unix)]
    #[test]
    fn immutable_install_rejects_conflicting_bytes_without_overwrite() {
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        let relative = Path::new("release.norito");
        writer
            .install_immutable(relative, b"first")
            .expect("first immutable install");
        let error = writer
            .install_immutable(relative, b"different")
            .expect_err("different immutable bytes must conflict");
        assert_eq!(error.code(), AtomicWriteErrorCode::ImmutableConflict);
        assert_eq!(
            fs::read(root.path().join(relative)).expect("read preserved target"),
            b"first"
        );
    }
    #[cfg(unix)]
    #[test]
    fn immutable_install_rejects_symlink_and_hard_link_targets() {
        use std::os::unix::fs::symlink;
        let outside = tempfile::tempdir().expect("outside root");
        let outside_file = outside.path().join("outside");
        fs::write(&outside_file, b"outside").expect("outside file");
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        symlink(&outside_file, root.path().join("symlink-target")).expect("target symlink");
        let error = writer
            .install_immutable(Path::new("symlink-target"), b"blocked")
            .expect_err("symlink target rejected");
        assert_eq!(error.code(), AtomicWriteErrorCode::UnsafeTarget);
        assert_eq!(
            fs::read(&outside_file).expect("outside contents"),
            b"outside"
        );
        let source = root.path().join("hard-source");
        fs::write(&source, b"hard-linked").expect("hard-link source");
        fs::hard_link(&source, root.path().join("hard-target")).expect("hard-link target");
        let error = writer
            .install_immutable(Path::new("hard-target"), b"hard-linked")
            .expect_err("hard-linked target rejected");
        assert_eq!(error.code(), AtomicWriteErrorCode::UnsafeTarget);
        assert_eq!(fs::read(source).expect("source contents"), b"hard-linked");
    }
    #[cfg(unix)]
    #[test]
    fn concurrent_identical_immutable_installers_are_idempotent() {
        use std::{
            sync::{Arc, Barrier},
            thread,
        };
        let root = tempfile::tempdir().expect("temporary root");
        let writer = Arc::new(AtomicWriteRoot::new(root.path()).expect("bind root"));
        let barrier = Arc::new(Barrier::new(8));
        let mut handles = Vec::with_capacity(8);
        for _ in 0..8 {
            let writer = Arc::clone(&writer);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                writer
                    .install_immutable(Path::new("concurrent"), b"identical")
                    .map_err(|error| error.code())
            }));
        }
        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("immutable installer thread"))
            .collect::<Vec<_>>();
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Ok(AtomicInstallOutcome::Installed)))
                .count(),
            1
        );
        // A racing caller may observe the winner's short-lived two-link state and fail
        // closed. Joining every caller before retry proves the stable idempotent result.
        assert!(results.iter().all(|result| matches!(
            result,
            Ok(AtomicInstallOutcome::Installed | AtomicInstallOutcome::AlreadyPresent)
                | Err(AtomicWriteErrorCode::UnsafeTarget
                    | AtomicWriteErrorCode::ConcurrentModification)
        )));
        assert_eq!(
            writer
                .install_immutable(Path::new("concurrent"), b"identical")
                .expect("sequential idempotent retry"),
            AtomicInstallOutcome::AlreadyPresent
        );
        assert_eq!(
            fs::read(root.path().join("concurrent")).expect("read concurrent target"),
            b"identical"
        );
        assert!(fs::read_dir(root.path()).expect("read root").all(|entry| {
            !entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .starts_with(TEMP_FILE_PREFIX)
        }));
    }
    #[cfg(unix)]
    #[test]
    fn concurrent_different_immutable_installers_never_overwrite() {
        use std::{
            sync::{Arc, Barrier},
            thread,
        };
        let root = tempfile::tempdir().expect("temporary root");
        let writer = Arc::new(AtomicWriteRoot::new(root.path()).expect("bind root"));
        let barrier = Arc::new(Barrier::new(2));
        let mut handles = Vec::with_capacity(2);
        for contents in [b"first".to_vec(), b"second".to_vec()] {
            let writer = Arc::clone(&writer);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                writer
                    .install_immutable(Path::new("different"), &contents)
                    .map_err(|error| error.code())
            }));
        }
        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("immutable installer thread"))
            .collect::<Vec<_>>();
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Ok(AtomicInstallOutcome::Installed)))
                .count(),
            1
        );
        // The loser may report the final conflict immediately or fail closed while the
        // winner is removing its staging link. The sequential checks below bind the result.
        assert!(results.iter().all(|result| matches!(
            result,
            Ok(AtomicInstallOutcome::Installed)
                | Err(AtomicWriteErrorCode::ImmutableConflict
                    | AtomicWriteErrorCode::UnsafeTarget
                    | AtomicWriteErrorCode::ConcurrentModification)
        )));
        let installed = fs::read(root.path().join("different")).expect("read installed target");
        assert!(installed == b"first" || installed == b"second");
        assert_eq!(
            writer
                .install_immutable(Path::new("different"), &installed)
                .expect("sequential idempotent retry"),
            AtomicInstallOutcome::AlreadyPresent
        );
        let conflicting = if installed == b"first" {
            &b"second"[..]
        } else {
            &b"first"[..]
        };
        let error = writer
            .install_immutable(Path::new("different"), conflicting)
            .expect_err("sequential different retry conflicts");
        assert_eq!(error.code(), AtomicWriteErrorCode::ImmutableConflict);
        assert_eq!(
            fs::read(root.path().join("different")).expect("reread installed target"),
            installed
        );
        assert!(fs::read_dir(root.path()).expect("read root").all(|entry| {
            !entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .starts_with(TEMP_FILE_PREFIX)
        }));
    }
    #[cfg(unix)]
    #[test]
    fn rejects_absolute_and_traversing_destinations() {
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        for path in [
            Path::new(""),
            Path::new("../escape"),
            Path::new("a/../escape"),
        ] {
            let error = writer.replace(path, b"blocked").expect_err("path rejected");
            assert_eq!(error.code(), AtomicWriteErrorCode::InvalidRelativePath);
        }
        let absolute = root.path().join("absolute");
        let error = writer
            .replace(&absolute, b"blocked")
            .expect_err("absolute path rejected");
        assert_eq!(error.code(), AtomicWriteErrorCode::InvalidRelativePath);
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlink_roots_parents_and_targets() {
        use std::os::unix::fs::symlink;
        let outside = tempfile::tempdir().expect("outside root");
        let holder = tempfile::tempdir().expect("holder root");
        let linked_root = holder.path().join("linked-root");
        symlink(outside.path(), &linked_root).expect("root symlink");
        assert_eq!(
            AtomicWriteRoot::new(&linked_root)
                .expect_err("symlink root rejected")
                .code(),
            AtomicWriteErrorCode::UnsafeRoot
        );
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        let linked_parent = root.path().join("linked-parent");
        symlink(outside.path(), &linked_parent).expect("parent symlink");
        assert_eq!(
            writer
                .replace(Path::new("linked-parent/file"), b"blocked")
                .expect_err("symlink parent rejected")
                .code(),
            AtomicWriteErrorCode::SymlinkAncestor
        );
        let outside_file = outside.path().join("outside-file");
        fs::write(&outside_file, b"unchanged").expect("outside file");
        symlink(&outside_file, root.path().join("target")).expect("target symlink");
        assert_eq!(
            writer
                .replace(Path::new("target"), b"blocked")
                .expect_err("symlink target rejected")
                .code(),
            AtomicWriteErrorCode::UnsafeTarget
        );
        assert_eq!(
            fs::read(outside_file).expect("outside contents"),
            b"unchanged"
        );
    }
    #[cfg(unix)]
    #[test]
    fn rejects_hard_linked_targets_and_creates_private_new_files() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().expect("temporary root");
        let writer = AtomicWriteRoot::new(root.path()).expect("bind root");
        writer
            .replace(Path::new("private"), b"secret-free state")
            .expect("write private file");
        let mode = fs::metadata(root.path().join("private"))
            .expect("private metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let linked = root.path().join("linked");
        fs::hard_link(root.path().join("private"), &linked).expect("hard link");
        let error = writer
            .replace(Path::new("private"), b"blocked")
            .expect_err("hard-linked target rejected");
        assert_eq!(error.code(), AtomicWriteErrorCode::UnsafeTarget);
        assert_eq!(
            fs::read(linked).expect("linked contents"),
            b"secret-free state"
        );
    }
    #[cfg(unix)]
    #[test]
    fn pending_temporary_file_cleanup_is_identity_bound() {
        let root = tempfile::tempdir().expect("temporary root");
        let path = {
            let pending = PendingTemp::create(root.path()).expect("create pending file");
            pending.path.clone()
        };
        assert!(!path.exists());
    }
    #[cfg(not(unix))]
    #[test]
    fn atomic_replacement_fails_closed_on_unsupported_platforms() {
        let root = tempfile::tempdir().expect("temporary root");
        let error = AtomicWriteRoot::new(root.path()).expect_err("platform rejected");
        assert_eq!(error.code(), AtomicWriteErrorCode::UnsupportedPlatform);
    }
}
