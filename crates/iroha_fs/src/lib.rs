//! Retained native filesystem authority for private Iroha runtime material.
//!
//! Private directories and files belong to the effective user. Operations reject links,
//! traversal, shared file custody and replaced ancestors; reads retain and recheck the original
//! descriptor. Unix uses descriptor-relative operations, owner modes and directory `fsync`.
//! Windows uses protected current-user DACLs, retained non-delete-sharing ancestor handles,
//! reparse rejection, file IDs and write-through native publication. No codec or network policy
//! belongs in this crate. Filesystem/media honesty remains an operating-system assumption.

use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use zeroize::Zeroizing;

mod private_files;
pub use private_files::{PendingPrivateFile, PrivateFileMetadata, SealedPrivateFile};

#[cfg(unix)]
#[path = "unix.rs"]
mod platform;
#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
use windows as platform;
#[cfg(not(any(unix, windows)))]
compile_error!("iroha_fs requires a native Unix or Windows host");

/// Whether atomic publication must create a new name or may replace an existing owned file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishMode {
    /// Fail with [`io::ErrorKind::AlreadyExists`] if the destination exists.
    CreateNew,
    /// Replace a regular, single-link, current-owner destination atomically.
    Replace,
}

/// Stable kernel identity of an opened file, independent of its pathname.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileIdentity {
    volume: u64,
    object: [u8; 16],
}

impl FileIdentity {
    /// Read a file's native volume and object identity from its retained descriptor.
    ///
    /// # Errors
    /// Returns the native error if the descriptor's identity cannot be read.
    pub fn of(file: &File) -> io::Result<Self> {
        platform::identity(file)
    }
}

/// Exact native file snapshot for comparing separately retained opens without retaining all files.
///
/// Binds kernel identity, length, timestamps and native custody-relevant metadata. A later open
/// must independently pass native ownership, link and access validation before comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSnapshot {
    inner: platform::FileSnapshot,
}

impl FileSnapshot {
    /// Capture exact writable private journal custody (Unix mode 0600, or the actual
    /// protected current-user Windows DACL and non-read-only native object).
    /// # Errors
    /// Refuses non-writable private originals, shared links or native I/O errors.
    pub fn private_journal(file: &File) -> io::Result<Self> {
        Ok(Self {
            inner: platform::journal_snapshot(file)?,
        })
    }
    /// Read exact native file identity, length, timestamps and custody metadata.
    ///
    /// This validates a regular single-link file and its native access restrictions. The
    /// retained directory/file owner must separately revalidate the pathname and ancestors.
    /// # Errors
    /// Refuses unsafe custody, links, changed native metadata or native I/O errors.
    pub fn of(file: &File, private: bool) -> io::Result<Self> {
        Ok(Self {
            inner: platform::snapshot_file(file, private)?,
        })
    }
}

/// Read bytes at an exact offset, independently of its prior cursor.
/// Unix uses native `pread`; Windows uses native `seek_read`, which moves the physical cursor.
/// A mutable owner must reanchor its write cursor before appending; descriptor identity is retained.
/// This is a DATA operation; callers retain and revalidate the original native custody.
/// # Errors
/// Returns the genuine native I/O error.
pub fn read_at(file: &File, buffer: &mut [u8], offset: u64) -> io::Result<usize> {
    #[cfg(unix)]
    {
        std::os::unix::fs::FileExt::read_at(file, buffer, offset)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::FileExt::seek_read(file, buffer, offset)
    }
}

/// Fill an exact offset range independently of its prior cursor.
/// # Errors
/// Refuses truncation, offset overflow or native I/O failure.
pub fn read_exact_at(file: &File, mut buffer: &mut [u8], mut offset: u64) -> io::Result<()> {
    while !buffer.is_empty() {
        match read_at(file, buffer, offset) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => {
                offset = offset
                    .checked_add(count as u64)
                    .ok_or_else(|| io::Error::other("native read offset overflow"))?;
                buffer = &mut buffer[count..];
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// An owner-private directory and the retained authority for all of its ancestors.
#[derive(Debug)]
pub struct PrivateDirectory {
    inner: platform::Directory,
}

impl PrivateDirectory {
    /// Open a private directory, creating missing components with private access at creation.
    ///
    /// Existing directories are validated, never silently hardened. Relative paths are resolved
    /// against the current directory; parent traversal and untrusted links are rejected.
    ///
    /// # Errors
    /// Refuses unsafe paths, permissions, filesystems, replaced ancestors, and native I/O errors.
    pub fn open_or_create(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            inner: platform::Directory::open(&absolute(path.as_ref())?, true)?,
        })
    }

    /// Open an existing owner-private directory without creating anything.
    ///
    /// # Errors
    /// Refuses a missing directory, unsafe custody or native I/O errors.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            inner: platform::Directory::open(&absolute(path.as_ref())?, false)?,
        })
    }

    /// The absolute retained directory path, for display and explicit child-process arguments.
    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    /// Recheck custody and identity of the directory and its retained ancestors.
    ///
    /// # Errors
    /// Returns an error if custody changed or a native check fails.
    pub fn revalidate(&self) -> io::Result<()> {
        self.inner.revalidate()
    }

    /// List bounded direct child names through this retained directory authority.
    ///
    /// Names are sorted and portable; no child is followed or recursively visited. Callers
    /// hold their operation lock when the list controls a subsequent mutation.
    ///
    /// # Errors
    /// Refuses unsafe custody, invalid child names, excessive entries and native I/O errors.
    pub fn entries(&self, maximum: usize) -> io::Result<Vec<std::ffi::OsString>> {
        self.inner.entries(maximum)
    }

    /// Read this directory's retained kernel identity.
    ///
    /// # Errors
    /// Returns an error if custody changed or native identity cannot be read.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.inner.identity()
    }

    /// Open or create one private child directory.
    ///
    /// # Errors
    /// Refuses invalid names, unsafe existing custody and native I/O errors.
    pub fn ensure_child(&self, name: impl AsRef<OsStr>) -> io::Result<Self> {
        let name = checked_name(name.as_ref())?;
        Ok(Self {
            inner: self.inner.child(name, true, false)?,
        })
    }

    /// Open one existing private child without creating anything.
    ///
    /// # Errors
    /// Refuses invalid names, missing children, unsafe custody and native errors.
    pub fn open_child(&self, name: impl AsRef<OsStr>) -> io::Result<Self> {
        Ok(Self {
            inner: self
                .inner
                .child(checked_name(name.as_ref())?, false, false)?,
        })
    }

    /// Create one private child directory, failing if its name already exists.
    ///
    /// # Errors
    /// Refuses invalid or existing names, changed custody and native I/O errors.
    pub fn create_child(&self, name: impl AsRef<OsStr>) -> io::Result<Self> {
        let name = checked_name(name.as_ref())?;
        Ok(Self {
            inner: self.inner.child(name, true, true)?,
        })
    }

    /// Read one private single-link regular file under an explicit byte limit.
    ///
    /// Secret storage allocates the declared length once and is zeroized on all return paths.
    /// A metadata, identity, length, or namespace change fails the read.
    ///
    /// # Errors
    /// Refuses unsafe custody, oversized or changing files, and native I/O errors.
    pub fn read(&self, name: impl AsRef<OsStr>, maximum: usize) -> io::Result<Zeroizing<Vec<u8>>> {
        let name = checked_name(name.as_ref())?;
        self.inner.read(name, maximum, true)
    }

    /// Stage, sync and atomically publish bytes with private access from the first write.
    ///
    /// An error after publication can mean durability is uncertain. Callers must reconcile the
    /// destination before retrying operations whose contents represent irreversible intent.
    ///
    /// # Errors
    /// Refuses unsafe custody, create-new collisions, failed native writes or failed durability.
    pub fn write_atomic(
        &self,
        name: impl AsRef<OsStr>,
        bytes: &[u8],
        mode: PublishMode,
    ) -> io::Result<()> {
        let name = checked_name(name.as_ref())?;
        self.inner.write_atomic(name, bytes, mode, true)
    }

    /// Open or create a private read/write lock file without truncation or taking its lock.
    ///
    /// The caller uses [`File::try_lock`] and retains the handle for the ownership lifetime.
    /// Cloned handles may be inherited by supervised children to preserve that ownership.
    ///
    /// # Errors
    /// Refuses unsafe custody, invalid names and native I/O errors.
    pub fn open_lock(&self, name: impl AsRef<OsStr>) -> io::Result<File> {
        self.inner.open_mutable(checked_name(name.as_ref())?, false)
    }

    /// Create a new private read/write lock file, refusing an existing name.
    ///
    /// # Errors
    /// Refuses existing names, unsafe custody and native I/O errors.
    pub fn create_lock(&self, name: impl AsRef<OsStr>) -> io::Result<File> {
        self.inner
            .open_exact_lock(checked_name(name.as_ref())?, true)
    }

    /// Open an existing private read/write lock without creating or truncating it.
    ///
    /// # Errors
    /// Refuses missing names, unsafe custody and native I/O errors.
    pub fn open_existing_lock(&self, name: impl AsRef<OsStr>) -> io::Result<File> {
        self.inner
            .open_exact_lock(checked_name(name.as_ref())?, false)
    }

    /// Remove this exact empty directory and durably publish its absence in the parent.
    ///
    /// All descendant handles must be dropped first. Nonempty directories are refused.
    /// An error after removal can indicate uncertain durability and requires reconciliation.
    ///
    /// # Errors
    /// Refuses live descendants, nonempty or replaced directories and native I/O errors.
    pub fn remove_empty(self) -> io::Result<()> {
        self.inner.remove_empty()
    }

    /// Open a private ownership lock suitable for retaining in supervised child processes.
    ///
    /// Windows additionally denies other writable opens while any inherited handle remains.
    /// The caller still acquires the advisory lock and retains every returned handle.
    ///
    /// # Errors
    /// Refuses unsafe custody, competing native ownership and native I/O errors.
    pub fn open_ownership_lock(&self, name: impl AsRef<OsStr>) -> io::Result<File> {
        self.inner.open_ownership_lock(checked_name(name.as_ref())?)
    }

    /// Open or create an append-only private log file without truncating an existing log.
    ///
    /// Unix callers use [`File::sync_all`] at their durability boundary. Windows appends use
    /// native write-through; that append-only handle does not request general write access.
    ///
    /// # Errors
    /// Refuses unsafe custody, invalid names and native I/O errors.
    pub fn open_append(&self, name: impl AsRef<OsStr>) -> io::Result<File> {
        self.inner.open_mutable(checked_name(name.as_ref())?, true)
    }

    /// Open an existing private regular file for read-only streaming, such as a live log tail.
    ///
    /// The descriptor retains its exact object; concurrently appended bytes are permitted, so
    /// this does not promise an immutable snapshot. Keep this directory alive during access.
    ///
    /// # Errors
    /// Refuses missing files, unsafe custody, invalid names and native I/O errors.
    pub fn open_read(&self, name: impl AsRef<OsStr>) -> io::Result<File> {
        self.inner.open_readonly(checked_name(name.as_ref())?)
    }

    /// Publish this exact completed directory under an absent sibling name.
    ///
    /// Callers must validate and sync the complete tree first and drop all retained descendants.
    /// Existing destinations are never replaced. The returned directory retains its identity
    /// and the updated path. An error after native publication may require reconciliation.
    ///
    /// # Errors
    /// Refuses replacement mode, existing names, live descendants, changed custody and I/O errors.
    pub fn rename_to_sibling(self, name: impl AsRef<OsStr>, mode: PublishMode) -> io::Result<Self> {
        if mode != PublishMode::CreateNew {
            return Err(invalid(
                "directory publication requires an absent destination",
            ));
        }
        Ok(Self {
            inner: self
                .inner
                .rename_to_sibling(checked_name(name.as_ref())?, mode)?,
        })
    }

    /// Remove this directory's contents, retaining exactly the listed direct child files.
    ///
    /// The caller must hold its persistent operation and runtime locks. Retained names must
    /// identify private regular files. Traversal never follows links, and refuses foreign
    /// ownership, shared writable entries, or more than 100,000 entries. Errors may leave a
    /// partially cleared directory; the directory and retained lock identities stay intact.
    ///
    /// # Errors
    /// Refuses unsafe or missing retained files, unsafe removal entries, bounds and I/O errors.
    pub fn clear_contents_preserving(&self, names: &[&str]) -> io::Result<()> {
        let preserved = names
            .iter()
            .map(|name| checked_name(OsStr::new(name)))
            .collect::<io::Result<Vec<_>>>()?;
        self.inner.clear_contents_preserving(&preserved)
    }

    /// Complete the native directory-publication durability boundary.
    ///
    /// # Errors
    /// Returns an error if custody changed or native durability cannot be completed.
    pub fn sync(&self) -> io::Result<()> {
        self.inner.sync()
    }
}

/// Read-only directory authority retaining every native ancestor and exact namespace.
/// This creates no directory, file or writable owner capability.
#[derive(Debug)]
pub struct ReaderDirectory {
    inner: platform::Directory,
}
impl ReaderDirectory {
    /// Retain an existing regular directory under genuine native custody.
    /// # Errors
    /// Refuses links, unsafe native permissions, foreign mutation or unavailable storage.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            inner: platform::Directory::open_reader(&absolute(path.as_ref())?)?,
        })
    }
    /// Recheck the original directory and all retained ancestors.
    /// # Errors
    /// Refuses replaced namespace or changed native custody.
    pub fn revalidate(&self) -> io::Result<()> {
        self.inner.revalidate()
    }
    /// Capture exact native directory metadata while retaining its original authority.
    /// # Errors
    /// Refuses changed custody or native I/O errors.
    pub fn snapshot(&self) -> io::Result<FileSnapshot> {
        Ok(FileSnapshot {
            inner: self.inner.snapshot_directory()?,
        })
    }
}

/// A retained project directory that permits readers while excluding foreign mutation.
///
/// Newly created directories and published files are owner-private. Existing project roots may
/// retain their reader permissions; this type never hardens or changes an existing directory.
#[derive(Debug)]
pub struct OwnerDirectory {
    inner: platform::Directory,
}

impl OwnerDirectory {
    /// Open an existing safe project directory and retain all of its ancestor identities.
    ///
    /// # Errors
    /// Refuses missing directories, unsafe ownership or write access, links and native errors.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            inner: platform::Directory::open_owned(&absolute(path.as_ref())?, false)?,
        })
    }

    /// Open a safe project directory, creating missing components privately.
    ///
    /// # Errors
    /// Refuses unsafe existing custody, links, traversal and native creation errors.
    pub fn open_or_create(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            inner: platform::Directory::open_owned(&absolute(path.as_ref())?, true)?,
        })
    }

    /// Return the retained absolute project path.
    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    /// Recheck retained custody and every ancestor's identity.
    ///
    /// # Errors
    /// Returns an error for replaced paths, changed access or native I/O failures.
    pub fn revalidate(&self) -> io::Result<()> {
        self.inner.revalidate()
    }

    /// Read this directory's retained kernel identity.
    ///
    /// # Errors
    /// Returns an error if custody changed or native identity cannot be read.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.inner.identity()
    }

    /// Open one existing safe child without creating anything.
    ///
    /// # Errors
    /// Refuses invalid names, missing children, unsafe custody and native errors.
    pub fn open_child(&self, name: impl AsRef<OsStr>) -> io::Result<Self> {
        Ok(Self {
            inner: self
                .inner
                .child_owned(checked_name(name.as_ref())?, false, false)?,
        })
    }

    /// Open one safe child or create it privately if absent.
    ///
    /// # Errors
    /// Refuses invalid names, unsafe existing custody and native errors.
    pub fn ensure_child(&self, name: impl AsRef<OsStr>) -> io::Result<Self> {
        Ok(Self {
            inner: self
                .inner
                .child_owned(checked_name(name.as_ref())?, true, false)?,
        })
    }

    /// Create one private child, rejecting an existing name.
    ///
    /// # Errors
    /// Refuses invalid names, existing children, unsafe custody and native errors.
    pub fn create_child(&self, name: impl AsRef<OsStr>) -> io::Result<Self> {
        Ok(Self {
            inner: self
                .inner
                .child_owned(checked_name(name.as_ref())?, true, true)?,
        })
    }

    /// Create a private child under this retained safe owner directory.
    ///
    /// # Errors
    /// Refuses existing names, unsafe custody and native I/O errors.
    pub fn create_private_child(&self, name: impl AsRef<OsStr>) -> io::Result<PrivateDirectory> {
        Ok(PrivateDirectory {
            inner: self.inner.child(checked_name(name.as_ref())?, true, true)?,
        })
    }

    /// Atomically publish a complete private directory containing the supplied direct files.
    ///
    /// Every file and the staging directory are synced before the destination becomes visible.
    /// Existing destinations are never replaced. A crash before publication can leave a private
    /// temporary sibling, but cannot leave a partial directory at the requested name. Empty bytes
    /// may initialize a lock file; callers acquire it only after this method returns.
    ///
    /// # Errors
    /// Rejects duplicate/invalid names, more than 128 files, unsafe custody and native failures.
    /// An error after publication can mean uncertain durability; reconcile the destination before
    /// retrying rather than treating it as absent or deleting it.
    pub fn publish_private_child(
        &self,
        name: impl AsRef<OsStr>,
        files: &[(&str, &[u8])],
    ) -> io::Result<PrivateDirectory> {
        let name = checked_name(name.as_ref())?;
        if files.is_empty() || files.len() > 128 {
            return Err(invalid("completed private directory requires 1..128 files"));
        }
        let mut names = std::collections::BTreeSet::new();
        for (file, _) in files {
            if !names.insert(checked_name(OsStr::new(file))?) {
                return Err(invalid("duplicate completed private directory filename"));
            }
        }
        match self.inner.child(name, false, false) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "private destination already exists",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut staging = None;
        for _ in 0..32 {
            match self.create_private_child(temporary_name()) {
                Ok(directory) => {
                    staging = Some(directory);
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        let directory =
            staging.ok_or_else(|| io::Error::other("cannot allocate private directory staging"))?;
        for (file, bytes) in files {
            directory.write_atomic(file, bytes, PublishMode::CreateNew)?;
        }
        directory.sync()?;
        directory.rename_to_sibling(name, PublishMode::CreateNew)
    }

    /// Read one bounded stable source file through this retained directory.
    ///
    /// # Errors
    /// Refuses links, shared write access, oversized or changed files and native errors.
    pub fn read_regular(
        &self,
        name: impl AsRef<OsStr>,
        maximum: usize,
    ) -> io::Result<Zeroizing<Vec<u8>>> {
        self.inner
            .read(checked_name(name.as_ref())?, maximum, false)
    }

    /// Publish private bytes atomically, optionally replacing one safe owned regular file.
    ///
    /// # Errors
    /// Refuses unsafe targets, create-new collisions, native writes and durability failures.
    pub fn write_atomic(
        &self,
        name: impl AsRef<OsStr>,
        bytes: &[u8],
        mode: PublishMode,
    ) -> io::Result<()> {
        self.inner
            .write_atomic(checked_name(name.as_ref())?, bytes, mode, false)
    }

    /// Open or create a private lock file without locking or truncating it.
    ///
    /// # Errors
    /// Refuses invalid names, unsafe custody and native errors.
    pub fn open_lock(&self, name: impl AsRef<OsStr>) -> io::Result<File> {
        self.inner.open_mutable(checked_name(name.as_ref())?, false)
    }

    /// Complete this directory's native publication durability boundary.
    ///
    /// # Errors
    /// Returns an error if custody changed or durability cannot be completed.
    pub fn sync(&self) -> io::Result<()> {
        self.inner.sync()
    }

    /// Publish this exact completed directory under an absent sibling name.
    ///
    /// Callers validate and sync the tree first and drop every retained descendant. Existing
    /// destinations are never replaced. Native publication preserves the directory identity.
    ///
    /// # Errors
    /// Refuses replacement mode, existing names, live descendants, changed custody and I/O errors.
    pub fn rename_to_sibling(self, name: impl AsRef<OsStr>, mode: PublishMode) -> io::Result<Self> {
        if mode != PublishMode::CreateNew {
            return Err(invalid(
                "directory publication requires an absent destination",
            ));
        }
        Ok(Self {
            inner: self
                .inner
                .rename_to_sibling(checked_name(name.as_ref())?, mode)?,
        })
    }
}

/// An exact regular file with retained no-follow ancestor and object authority.
///
/// Callers bound streaming reads and validate content snapshots as required by their format.
/// [`Self::revalidate`] checks custody and identity; it permits intentional writes to this object.
#[derive(Debug)]
pub struct RetainedFile {
    inner: platform::RetainedFile,
}

impl RetainedFile {
    /// Open an existing regular source for read-only streaming.
    ///
    /// # Errors
    /// Refuses unsafe ownership, shared writes, links, multiple links and native errors.
    pub fn open_regular(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::open(path.as_ref(), false, false)
    }

    /// Retain one public original with Unix 0644/0444 or the equivalent native Windows
    /// read access without foreign mutation. This authenticates custody, never its contents.
    /// # Errors
    /// Refuses unsafe permission shape, links, changed ancestors or native errors.
    pub fn open_public_original(path: impl AsRef<Path>) -> io::Result<Self> {
        let original = Self::open_regular(path)?;
        platform::validate_public_original(original.file())?;
        original.revalidate()?;
        Ok(original)
    }

    /// Open an existing private regular file for read-only streaming.
    ///
    /// # Errors
    /// Refuses nonprivate or unsafe custody, multiple links and native errors.
    pub fn open_private(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::open(path.as_ref(), true, false)
    }

    /// Create a new private regular file with read/write access, retaining its exact identity.
    ///
    /// # Errors
    /// Refuses existing names, unsafe ancestors, links and native creation errors.
    pub fn create_new_private(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::open(path.as_ref(), true, true)
    }

    fn open(path: &Path, private: bool, create_new: bool) -> io::Result<Self> {
        let path = absolute(path)?;
        let name = checked_name(
            path.file_name()
                .ok_or_else(|| invalid("file path has no name"))?,
        )?;
        let parent = path
            .parent()
            .ok_or_else(|| invalid("file path has no parent"))?;
        let directory = if create_new {
            platform::Directory::open_owned(parent, false)?
        } else {
            platform::Directory::open_reader(parent)?
        };
        Ok(Self {
            inner: directory.open_retained(name, private, create_new)?,
        })
    }

    /// Borrow the exact descriptor while retaining every ancestor.
    pub fn file(&self) -> &File {
        self.inner.file()
    }

    /// Borrow the descriptor mutably for bounded streaming or seek operations.
    pub fn file_mut(&mut self) -> &mut File {
        self.inner.file_mut()
    }

    /// Sync a completed writer and freeze its exact metadata and content snapshot.
    ///
    /// The original descriptor and all ancestors remain retained. Subsequent revalidation rejects
    /// changes to the sealed file; no close/reopen gap is introduced.
    ///
    /// # Errors
    /// Refuses changed custody, failed synchronization and native snapshot failures.
    pub fn seal(self) -> io::Result<Self> {
        Ok(Self {
            inner: self.inner.seal()?,
        })
    }

    /// Capture exact native identity and metadata after revalidating this retained file.
    ///
    /// # Errors
    /// Refuses custody, namespace or sealed-content changes and native snapshot failures.
    pub fn snapshot(&self) -> io::Result<FileSnapshot> {
        Ok(FileSnapshot {
            inner: self.inner.snapshot()?,
        })
    }

    /// Recheck ownership, link count, path and retained object identity after streaming.
    ///
    /// # Errors
    /// Refuses any custody or namespace change and native I/O failures.
    pub fn revalidate(&self) -> io::Result<()> {
        self.inner.revalidate()
    }

    /// Return the exact retained object's kernel identity.
    ///
    /// # Errors
    /// Returns a native error if the retained identity cannot be read.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.inner.identity()
    }
}

/// Read a private current-owner file through retained no-follow native handles.
///
/// # Errors
/// Refuses unsafe custody, oversized or changing files, and native I/O errors.
pub fn read_private(path: impl AsRef<Path>, maximum: usize) -> io::Result<Zeroizing<Vec<u8>>> {
    read_external(path.as_ref(), maximum, true)
}

/// Read a bounded stable regular source file without requiring private reader permissions.
///
/// The file must still have one link and belong to the current user or the operating system;
/// writable-by-others sources and unsafe ancestors are rejected.
///
/// # Errors
/// Refuses unsafe custody, oversized or changing files, and native I/O errors.
pub fn read_regular(path: impl AsRef<Path>, maximum: usize) -> io::Result<Zeroizing<Vec<u8>>> {
    read_external(path.as_ref(), maximum, false)
}

fn read_external(path: &Path, maximum: usize, private: bool) -> io::Result<Zeroizing<Vec<u8>>> {
    let path = absolute(path)?;
    let name = checked_name(
        path.file_name()
            .ok_or_else(|| invalid("file path has no name"))?,
    )?;
    let parent = path
        .parent()
        .ok_or_else(|| invalid("file path has no parent"))?;
    platform::read_external(parent, name, maximum, private)
}

fn absolute(path: &Path) -> io::Result<PathBuf> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(invalid(
            "a nonempty path without parent traversal is required",
        ));
    }
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    if path.components().count() > 128 {
        return Err(invalid("path exceeds the native custody depth bound"));
    }
    Ok(path)
}

fn checked_name(name: &OsStr) -> io::Result<&OsStr> {
    let path = Path::new(name);
    let mut parts = path.components();
    if !matches!(parts.next(), Some(Component::Normal(_))) || parts.next().is_some() {
        return Err(invalid("file name must be one normal path component"));
    }
    // Use one portable namespace: reject alternate streams, Win32 trimming and reserved devices
    // on every host so artifacts cannot change identity when a workspace moves platforms.
    let text = name
        .to_str()
        .ok_or_else(|| invalid("private file name must be UTF-8"))?;
    if text.contains([':', '\\', '/', '<', '>', '|', '"', '?', '*'])
        || text.chars().any(|character| character <= '\u{1f}')
        || text.ends_with(['.', ' '])
    {
        return Err(invalid("file name has a nonportable spelling"));
    }
    let stem = text
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|n| {
            (n.len() == 1 && matches!(n.as_bytes()[0], b'1'..=b'9')) || matches!(n, "¹" | "²" | "³")
        })
    }) {
        return Err(invalid("reserved device name is not a regular file name"));
    }
    Ok(name)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn denied(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}
fn changed() -> io::Error {
    io::Error::other("retained filesystem identity or custody changed")
}

fn temporary_name() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        ".iroha-fs-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

fn bounded_read(file: &mut File, length: u64, maximum: usize) -> io::Result<Zeroizing<Vec<u8>>> {
    let length = usize::try_from(length).map_err(|_| invalid("file exceeds the read bound"))?;
    if length > maximum {
        return Err(invalid("file exceeds the read bound"));
    }
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(length)
        .map_err(|_| io::Error::other("bounded file allocation refused"))?;
    bytes.resize(length, 0);
    file.read_exact(&mut bytes)?;
    let mut extra = zeroize::Zeroizing::new([0u8; 1]);
    if file.read(extra.as_mut())? != 0 {
        return Err(changed());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod native_offset_custody_tests {
    use super::*;
    use std::io::{Seek, SeekFrom, Write};

    #[test]
    fn offset_read_then_tail_append_retains_exact_native_original() {
        let temporary = tempfile::tempdir().unwrap();
        let root = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
        let mut file = root.create_lock("journal").unwrap();
        file.try_lock().unwrap();
        file.write_all(b"first").unwrap();
        file.sync_all().unwrap();
        let identity = FileIdentity::of(&file).unwrap();
        let mut buffer = [0; 3];
        read_exact_at(&file, &mut buffer, 1).unwrap();
        assert_eq!(&buffer, b"irs");
        // The real native journal always selects and checks its acknowledged tail first.
        assert_eq!(file.seek(SeekFrom::End(0)).unwrap(), 5);
        file.write_all(b"second").unwrap();
        file.sync_all().unwrap();
        let mut complete = [0; 11];
        read_exact_at(&file, &mut complete, 0).unwrap();
        assert_eq!(&complete, b"firstsecond");
        assert_eq!(FileIdentity::of(&file).unwrap(), identity);
        assert!(FileSnapshot::private_journal(&file).is_ok());
        assert!(read_exact_at(&file, &mut [0; 1], 11).is_err());
        root.revalidate().unwrap();
    }

    #[test]
    fn reader_directory_keeps_namespace_and_original_public_bytes() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let owner = OwnerDirectory::open(&root).unwrap();
        owner
            .write_atomic("public", b"original", PublishMode::CreateNew)
            .unwrap();
        let directory = ReaderDirectory::open(&root).unwrap();
        let namespace = directory.snapshot().unwrap();
        let original = RetainedFile::open_regular(root.join("public")).unwrap();
        let before = original.snapshot().unwrap();
        let mut bytes = [0; 8];
        read_exact_at(original.file(), &mut bytes, 0).unwrap();
        assert_eq!(&bytes, b"original");
        assert_eq!(original.snapshot().unwrap(), before);
        assert_eq!(directory.snapshot().unwrap(), namespace);
        assert!(
            owner
                .write_atomic("public", b"replacement", PublishMode::CreateNew)
                .is_err()
        );
        assert_eq!(original.snapshot().unwrap(), before);
    }
}
