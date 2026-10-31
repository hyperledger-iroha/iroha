//! Original-object no-replace moves with retained source and destination custody.

use super::*;
use std::ffi::OsString;

/// Boundaries of an original-file move.
///
/// Observations precede each operation except [`Self::Published`], which follows the
/// successful native rename before any fallible validation or synchronization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileMoveStep {
    /// Rename the original file to an absent destination.
    Rename,
    /// The native rename succeeded; latch possible publication before returning an error.
    Published,
    /// Persist the destination namespace after the rename.
    SyncDestination,
    /// Persist the source namespace after the rename.
    SyncSource,
    /// Validate both parents, destination snapshot and original-name absence.
    Validate,
}

/// An original current-owner single-link file admitted for a no-replace move.
///
/// Read and seek retain the exact admission snapshot and original namespace. This owner
/// exposes no writer, raw descriptor or clone. On Windows its first open requests delete
/// authority and denies other writers/deleters; existing ACLs are never modified.
///
/// ```compile_fail
/// use iroha_fs::MovableFile;
/// fn expose(file: MovableFile) { let _ = file.file(); }
/// ```
///
/// ```compile_fail
/// use std::io::Write;
/// use iroha_fs::MovableFile;
/// fn rewrite(mut file: MovableFile) { file.write_all(b"replacement").unwrap(); }
/// ```
#[derive(Debug)]
pub struct MovableFile {
    inner: platform::RetainedFile,
    source: platform::Directory,
    name: OsString,
    pending_private_rename: bool,
}

/// A moved original file with both namespace parents retained.
///
/// Destination observations retain both original ancestry chains. Source-name absence is a
/// separate check, so a caller may subsequently reuse that name for an authenticated new file.
#[derive(Debug)]
pub struct MovedFile {
    inner: platform::RetainedFile,
    source: platform::Directory,
    source_name: OsString,
    original: FileSnapshot,
}

impl OwnerDirectory {
    /// Retain a current-owner regular child with authority to move its original object.
    ///
    /// Only single-link files are admitted. The caller must complete and sync source bytes
    /// before admission; a move persists namespace changes and does not author file contents.
    /// This does not upgrade a read-only build-input owner or alter the original file's ACL.
    ///
    /// # Errors
    /// Refuses invalid names, shared links, foreign ownership, unsafe custody and native
    /// errors. Windows additionally refuses conflicting sharing or missing delete authority.
    pub fn open_movable_regular(&self, name: impl AsRef<OsStr>) -> io::Result<MovableFile> {
        let name = checked_name(name.as_ref())?;
        Ok(MovableFile {
            inner: self.inner.open_movable(name)?,
            source: self.inner.clone(),
            name: name.to_owned(),
            pending_private_rename: false,
        })
    }
}

impl PrivateDirectory {
    /// Remove a private child only if its deletion original matches the admitted snapshot.
    ///
    /// This retains the same private parent authority and canonical native deletion recipe as
    /// [`OwnerDirectory::remove_private_if_snapshot`]. A changed original is never removed.
    ///
    /// # Errors
    /// Refuses unsafe names, changed identity/content metadata, nonprivate custody and I/O errors.
    pub fn remove_private_if_snapshot(
        &self,
        name: impl AsRef<OsStr>,
        expected: &FileSnapshot,
    ) -> io::Result<bool> {
        let name = checked_name(name.as_ref())?;
        self.revalidate()?;
        let result = self
            .inner
            .remove_private_if_snapshot_native(name, &expected.inner);
        self.revalidate()?;
        result
    }

    /// Retain an existing writable-private original for reversible same-directory moves.
    ///
    /// The initial native open has move authority and retains private policy throughout its
    /// lifetime. It never upgrades a recovered reader or repairs existing permissions.
    ///
    /// # Errors
    /// Refuses unsafe names, ownership, permissions, links, sharing and native errors.
    pub fn open_movable_private(&self, name: impl AsRef<OsStr>) -> io::Result<MovableFile> {
        let name = checked_name(name.as_ref())?;
        Ok(MovableFile {
            inner: self.inner.open_movable_private(name)?,
            source: self.inner.clone(),
            name: name.to_owned(),
            pending_private_rename: false,
        })
    }
}

impl MovableFile {
    pub(crate) fn from_private_creator(inner: platform::RetainedFile) -> io::Result<Self> {
        let (inner, source, name) = inner.finish_private_for_move()?;
        Ok(Self {
            inner,
            source,
            name,
            pending_private_rename: false,
        })
    }

    pub(crate) fn from_created(inner: platform::RetainedFile) -> io::Result<Self> {
        let (inner, source, name) = inner.into_move_parts()?;
        Ok(Self {
            inner,
            source,
            name,
            pending_private_rename: false,
        })
    }

    /// Recheck the exact source snapshot, name and retained ancestry.
    ///
    /// # Errors
    /// Refuses changed custody, content metadata or namespace and native observation errors.
    pub fn revalidate(&self) -> io::Result<()> {
        if self.pending_private_rename {
            return Err(io::Error::other("private rename requires reconciliation"));
        }
        self.inner.revalidate()
    }

    /// Rename a private original within its retained directory without replacing anything.
    ///
    /// Unlike the consuming cross-directory move, this retains the original handle even when
    /// publication or durability checks fail. A later call can move that same original back to
    /// an absent previous name. Callers persist recovery intent before the first mutation.
    /// `Published` runs immediately after native rename; an error never implies rollback.
    ///
    /// # Errors
    /// Refuses nonprivate/read-only capabilities, unsafe custody, occupied destinations and
    /// native/observer failures. The caller still owns this capability on every error.
    pub fn rename_private_no_replace_observed(
        &mut self,
        name: impl AsRef<OsStr>,
        mut observe: impl FnMut(FileMoveStep) -> io::Result<()>,
    ) -> io::Result<()> {
        let name = checked_name(name.as_ref())?;
        if self.pending_private_rename {
            self.inner.finish_private_rename()?;
            self.pending_private_rename = false;
        }
        self.revalidate()?;
        self.source.require_absent(name)?;
        observe(FileMoveStep::Rename)?;
        self.inner.rename_private_original(name)?;
        name.clone_into(&mut self.name);
        self.pending_private_rename = true;
        let publication = observe(FileMoveStep::Published);
        self.inner.finish_private_rename()?;
        self.pending_private_rename = false;
        publication?;
        observe(FileMoveStep::SyncDestination)?;
        self.source.sync()?;
        observe(FileMoveStep::SyncSource)?;
        self.source.sync()?;
        observe(FileMoveStep::Validate)?;
        self.revalidate()
    }

    /// Return the exact revalidated source snapshot.
    ///
    /// # Errors
    /// Refuses any change since admission and native observation errors.
    pub fn snapshot(&self) -> io::Result<FileSnapshot> {
        self.revalidate()?;
        Ok(FileSnapshot {
            inner: self.inner.snapshot()?,
        })
    }

    /// Return the retained original native identity.
    ///
    /// # Errors
    /// Refuses changed source custody or native observation errors.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.revalidate()?;
        self.inner.identity()
    }

    /// Move the original object to an absent name and persist both namespaces.
    ///
    /// The destination must be on the same filesystem. The native move never replaces an
    /// existing entry. Content/custody metadata must remain identical except for the native
    /// rename change timestamp. Callers authenticate bytes around this metadata-only operation
    /// when their format requires content authenticity. Concurrent same-user Unix mutation is
    /// checked with metadata snapshots; no mandatory lock prevents it.
    ///
    /// # Errors
    /// Refuses changed custody, existing destinations, unsupported atomic no-replace moves
    /// and native I/O errors. An error may follow a completed rename: neither name is removed
    /// or rolled back. The caller must persist its own intent before this operation and
    /// authenticate remaining objects during recovery; an error is not permission to retry.
    pub fn move_no_replace(
        self,
        destination: &OwnerDirectory,
        name: impl AsRef<OsStr>,
    ) -> io::Result<MovedFile> {
        self.move_no_replace_observed(destination, name, |_| Ok(()))
    }

    /// Move the original object with interruption points at each namespace boundary.
    ///
    /// The observer cannot grant replacement authority or bypass post-rename validation.
    /// [`FileMoveStep::Published`] runs immediately after the native rename, before any
    /// fallible native checks. Callers must latch publication before their observer can fail.
    /// Failure preserves all remaining names and never performs cleanup. The same durability
    /// and recovery requirements as [`Self::move_no_replace`] apply.
    ///
    /// # Errors
    /// Returns observer failures and the custody/publication errors of [`Self::move_no_replace`].
    pub fn move_no_replace_observed(
        self,
        destination: &OwnerDirectory,
        name: impl AsRef<OsStr>,
        mut before: impl FnMut(FileMoveStep) -> io::Result<()>,
    ) -> io::Result<MovedFile> {
        let name = checked_name(name.as_ref())?;
        let original = self.snapshot()?;
        destination.revalidate()?;
        destination.inner.require_absent(name)?;
        before(FileMoveStep::Rename)?;
        self.revalidate()?;
        destination.revalidate()?;
        let moved = MovedFile {
            inner: self
                .inner
                .move_no_replace(&destination.inner, name, || before(FileMoveStep::Published))?,
            source: self.source,
            source_name: self.name,
            original,
        };
        moved.require_source_absent()?;
        before(FileMoveStep::SyncDestination)?;
        moved.require_source_absent()?;
        destination.sync()?;
        before(FileMoveStep::SyncSource)?;
        moved.require_source_absent()?;
        moved.source.sync()?;
        before(FileMoveStep::Validate)?;
        moved.require_source_absent()?;
        Ok(moved)
    }
}

impl MovedFile {
    /// Recheck both original parents and the exact destination snapshot.
    ///
    /// # Errors
    /// Refuses replaced parents, a changed destination or native errors.
    pub fn revalidate(&self) -> io::Result<()> {
        self.source.revalidate()?;
        self.inner.revalidate()?;
        self.source.revalidate()
    }

    /// Additionally require the original source name to remain absent.
    ///
    /// Successful move admission performs this check after both namespace syncs. Later
    /// callers may deliberately reuse that name while continuing destination revalidation.
    ///
    /// # Errors
    /// Refuses changed custody, a recreated source name and native observation errors.
    pub fn require_source_absent(&self) -> io::Result<()> {
        self.revalidate()?;
        self.source.require_absent(&self.source_name)?;
        self.revalidate()
    }

    /// Return the original admission snapshot, before the native rename timestamp changed.
    #[must_use]
    pub const fn original_snapshot(&self) -> FileSnapshot {
        self.original
    }

    /// Return the exact post-rename snapshot after revalidating both namespaces.
    ///
    /// # Errors
    /// Refuses changed destination/source custody and native observation errors.
    pub fn snapshot(&self) -> io::Result<FileSnapshot> {
        self.revalidate()?;
        let result = FileSnapshot {
            inner: self.inner.snapshot()?,
        };
        self.revalidate()?;
        Ok(result)
    }

    /// Return the same native identity admitted at the original source.
    ///
    /// # Errors
    /// Refuses changed destination/source custody and native observation errors.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.revalidate()?;
        self.inner.identity()
    }
}

macro_rules! retained_reader {
    ($owner:ty) => {
        impl Read for $owner {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                self.revalidate()?;
                let result = self.inner.file_mut().read(bytes);
                self.revalidate()?;
                result
            }
        }

        impl Seek for $owner {
            fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
                self.revalidate()?;
                let result = self.inner.file_mut().seek(position);
                self.revalidate()?;
                result
            }
        }
    };
}

retained_reader!(MovableFile);
retained_reader!(MovedFile);

#[cfg(test)]
mod tests;
