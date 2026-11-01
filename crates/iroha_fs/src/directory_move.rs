//! Original directory moves with explicit namespace publication and retained ancestry.

use super::*;
use std::{error::Error, ffi::OsString, fmt};

/// Observed boundaries of an atomic no-replace directory move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectoryMoveStep {
    /// Immediately before the native move; original custody is checked again afterward.
    Rename,
    /// Immediately after native publication, before any fallible post-publication work.
    Published,
    /// Before persisting the destination parent's namespace.
    SyncDestination,
    /// Before persisting the original source parent's namespace.
    SyncSource,
    /// Before the final original-object and namespace checks.
    Validate,
}

/// A directory-move refusal with its exact native publication boundary.
///
/// No error grants cleanup or replacement authority. Once publication may be visible, a
/// caller must retain its durable intent and authenticate both namespaces during recovery.
#[derive(Debug)]
pub struct DirectoryMoveError {
    error: io::Error,
    published: bool,
}

impl DirectoryMoveError {
    /// Whether the native rename succeeded before the reported failure.
    #[must_use]
    pub const fn publication_may_be_visible(&self) -> bool {
        self.published
    }

    /// Inspect the underlying custody, observer or native I/O failure.
    #[must_use]
    pub const fn io_error(&self) -> &io::Error {
        &self.error
    }
}

impl fmt::Display for DirectoryMoveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl Error for DirectoryMoveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}

/// The original moved directory, retaining both original ancestry chains.
///
/// This owner has no mutable child operations. Consume it with [`Self::into_directory`]
/// after completing the move protocol to permit subsequent directory mutations.
#[derive(Debug)]
pub struct MovedDirectory {
    inner: platform::MovedDirectory,
    source: platform::Directory,
    source_name: OsString,
}

impl OwnerDirectory {
    /// Move this original directory to an absent name below another retained owner.
    ///
    /// Both parents must belong to the same filesystem. Live descendants, shared source
    /// owners and destinations within the original directory are refused. The caller must
    /// finish and sync tree contents before moving; this operation persists both namespace
    /// changes, without recursively authenticating or syncing descendants. Concurrent Unix
    /// mutation is detected through custody snapshots rather than a mandatory lock.
    ///
    /// # Errors
    /// Refuses changed custody, existing targets, unsupported atomic no-replace operations
    /// and native errors. Inspect [`DirectoryMoveError::publication_may_be_visible`] before
    /// deciding recovery: no failure removes names or rolls back a completed move.
    pub fn move_no_replace(
        self,
        destination: &Self,
        name: impl AsRef<OsStr>,
    ) -> Result<MovedDirectory, DirectoryMoveError> {
        self.move_no_replace_observed(destination, name, |_| Ok(()))
    }

    /// Move an original directory with deterministic interruption boundaries.
    ///
    /// Original snapshots are captured before the first observer, and rechecked before the
    /// native move. Publication is recorded before invoking the `Published` observer.
    ///
    /// # Errors
    /// Returns observer failures and the custody/publication errors of [`Self::move_no_replace`].
    pub fn move_no_replace_observed(
        self,
        destination: &Self,
        name: impl AsRef<OsStr>,
        mut observe: impl FnMut(DirectoryMoveStep) -> io::Result<()>,
    ) -> Result<MovedDirectory, DirectoryMoveError> {
        let mut published = false;
        let result = (|| {
            let name = checked_name(name.as_ref())?;
            destination.revalidate()?;
            destination.inner.require_absent(name)?;
            let source_name = self
                .inner
                .path()
                .file_name()
                .ok_or_else(changed)?
                .to_owned();
            let prepared = self.inner.prepare_directory_move(&destination.inner)?;
            let source = prepared.source_parent();
            observe(DirectoryMoveStep::Rename)?;
            let inner = prepared.publish(&destination.inner, name, || {
                published = true;
                observe(DirectoryMoveStep::Published)
            })?;
            let moved = MovedDirectory {
                inner,
                source,
                source_name,
            };
            moved.require_source_absent()?;
            observe(DirectoryMoveStep::SyncDestination)?;
            moved.require_source_absent()?;
            destination.sync()?;
            observe(DirectoryMoveStep::SyncSource)?;
            moved.require_source_absent()?;
            moved.source.sync()?;
            observe(DirectoryMoveStep::Validate)?;
            moved.require_source_absent()?;
            Ok(moved)
        })();
        result.map_err(|error| DirectoryMoveError { error, published })
    }
}

impl PrivateDirectory {
    /// Move this exact private directory below another retained private directory.
    ///
    /// Both private policies and their original ancestry handles survive the transfer.
    /// The caller must finish and sync descendants and release their handles first.
    /// Existing destinations are never replaced. No error grants cleanup authority;
    /// inspect [`DirectoryMoveError::publication_may_be_visible`] before recovery.
    ///
    /// # Errors
    /// Refuses changed custody, live descendants, existing targets, observer failures
    /// and the native publication errors of [`OwnerDirectory::move_no_replace_observed`].
    pub fn move_no_replace_observed(
        self,
        destination: &Self,
        name: impl AsRef<OsStr>,
        observe: impl FnMut(DirectoryMoveStep) -> io::Result<()>,
    ) -> Result<MovedDirectory, DirectoryMoveError> {
        // Wrapping the same platform owners preserves their private policy flags;
        // neither original is reopened by pathname or readmitted under weaker policy.
        OwnerDirectory { inner: self.inner }.move_no_replace_observed(
            &OwnerDirectory {
                inner: destination.inner.clone(),
            },
            name,
            observe,
        )
    }
}

impl MovedDirectory {
    /// Recheck both retained ancestry chains and the exact moved object's snapshot.
    ///
    /// # Errors
    /// Refuses changed namespace, native identity, custody metadata or observation errors.
    pub fn revalidate(&self) -> io::Result<()> {
        self.source.revalidate_move_parent()?;
        self.inner.revalidate()?;
        self.source.revalidate_move_parent()
    }

    /// Require the original source name to remain absent, in addition to revalidation.
    ///
    /// Later authorized source-name reuse is permitted by [`Self::revalidate`].
    ///
    /// # Errors
    /// Refuses a recreated source entry, changed custody or native observation errors.
    pub fn require_source_absent(&self) -> io::Result<()> {
        self.revalidate()?;
        self.source.require_absent(&self.source_name)?;
        self.revalidate()
    }

    /// Return the original identity after checking both retained namespace chains.
    ///
    /// # Errors
    /// Refuses changed custody or native observation errors.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.revalidate()?;
        self.inner.identity()
    }

    /// Transfer the same original directory into an ordinary mutable directory owner.
    ///
    /// The source ancestry remains retained through the final revalidation and ownership
    /// transfer. Further operations retain the destination ancestry alone.
    ///
    /// # Errors
    /// Refuses any change before the transfer; performs no cleanup on failure.
    pub fn into_directory(self) -> io::Result<OwnerDirectory> {
        self.revalidate()?;
        let result = OwnerDirectory {
            inner: self.inner.into_directory(),
        };
        self.source.revalidate_move_parent()?;
        result.revalidate()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod private_tests;
