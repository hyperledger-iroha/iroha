//! Durable, root-confined atomic file replacement for Musubi project state.
//!
//! Callers first bind writes to a trusted directory with [`AtomicWriteRoot`]
//! and then provide only a normal relative path. This deliberately prevents a
//! path read from a lockfile or registry response from becoming an arbitrary
//! filesystem write target.

use std::{
    ffi::OsString,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};

const TEMP_CREATE_ATTEMPTS: u64 = 128;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
    /// The destination was a symlink, hard link, directory, or other unsupported file type.
    UnsafeTarget,
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
            Self::ConcurrentModification => "MUSUBI_ATOMIC_CONCURRENT_MODIFICATION",
            Self::TemporaryNameExhausted => "MUSUBI_ATOMIC_TEMPORARY_NAME_EXHAUSTED",
            Self::Io => "MUSUBI_ATOMIC_IO",
        }
    }
}

/// Error returned by [`AtomicWriteRoot::replace`].
#[derive(Debug)]
pub struct AtomicWriteError {
    code: AtomicWriteErrorCode,
    path: PathBuf,
    operation: &'static str,
    source: Option<io::Error>,
}

impl AtomicWriteError {
    fn new(code: AtomicWriteErrorCode, path: impl Into<PathBuf>, operation: &'static str) -> Self {
        Self {
            code,
            path: path.into(),
            operation,
            source: None,
        }
    }

    fn io(path: impl Into<PathBuf>, operation: &'static str, source: io::Error) -> Self {
        Self {
            code: AtomicWriteErrorCode::Io,
            path: path.into(),
            operation,
            source: Some(source),
        }
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

/// Trusted directory within which Musubi may atomically replace files.
#[derive(Debug)]
pub struct AtomicWriteRoot {
    canonical_root: PathBuf,
    root_identity: FileIdentity,
}

impl AtomicWriteRoot {
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
        Ok(Self {
            canonical_root,
            root_identity: FileIdentity::from_metadata(&canonical_metadata),
        })
    }

    /// Return the canonical trusted root.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.canonical_root
    }

    /// Durably replace one root-relative regular file.
    ///
    /// The destination parent must already exist. The method rejects absolute
    /// paths, traversal, symlink parents and targets, non-regular targets, and
    /// hard-linked targets. V1 currently enables replacement only on Unix. It
    /// creates a private temporary file in the
    /// destination directory with `create_new`, writes and flushes all bytes,
    /// synchronizes the file, atomically renames it, and synchronizes the
    /// parent directory. Existing permissions are retained; a new file is
    /// private (`0600`) on Unix.
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
            let mut name = OsString::from(".musubi-tmp-");
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
        {
            return Err(AtomicWriteError::new(
                AtomicWriteErrorCode::ConcurrentModification,
                &self.path,
                "revalidate the temporary file",
            ));
        }
        Ok(())
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
    let directory = File::open(path)
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

#[cfg(test)]
mod tests {
    use super::*;

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
