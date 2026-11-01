//! Mutable private journal originals with explicit Windows publication state.
//!
//! These handles share reads and deletion, but exclude competing writers. A caller must
//! serialize its journal protocol and retain its original directory and descriptors across
//! failures. Named identity checks detect substitutions; they do not reserve leaf names
//! against another same-user process that ignores the caller's lock.

use super::*;

impl OwnerDirectory {
    /// Revalidate one still-held delete-pending private journal original.
    ///
    /// This proves current writable, non-reparse regular-file custody and zero remaining
    /// NTFS links with delete-pending state, not pathname absence. It also applies to an
    /// admitted predecessor after confirmed retained-handle replacement. The caller must
    /// retain its publication/disposition state and separately join the captured identity;
    /// this observation alone does not establish which transition occurred.
    ///
    /// # Errors
    /// Refuses changed parent/private custody, a live non-disposed file, or native I/O failure.
    pub fn require_retired_private_journal(&self, original: &File) -> io::Result<()> {
        self.inner.require_retired_private_journal(original)
    }

    /// Mark this exact private journal original for deletion without reopening it.
    ///
    /// The caller retains the original across every error. `disposed` must start false
    /// and becomes true immediately after native disposition succeeds, before any
    /// fallible final check. That state means delete-pending, not completed namespace
    /// removal: the caller explicitly closes its original and syncs the retained parent
    /// only after reconciling this phase. A disposed original must never be retired again.
    ///
    /// # Errors
    /// Refuses unsafe or substituted names, invalid state and native disposition errors.
    pub fn retire_private_journal(
        &self,
        name: impl AsRef<OsStr>,
        original: &File,
        disposed: &mut bool,
    ) -> io::Result<()> {
        self.retire_private_journal_observed(name.as_ref(), original, disposed, |_| Ok(()))
    }

    fn retire_private_journal_observed(
        &self,
        name: &OsStr,
        original: &File,
        disposed: &mut bool,
        mut observe: impl FnMut(bool) -> io::Result<()>,
    ) -> io::Result<()> {
        let name = checked_name(name)?;
        if *disposed {
            return Err(invalid(
                "private journal original is already delete-pending",
            ));
        }
        self.inner
            .retire_private_journal_observed(name, original, disposed, &mut observe)
    }

    /// Open an optional private journal original with write and native rename authority.
    ///
    /// Unlike ordinary locks and readers, this journal-specific handle shares deletion so
    /// the next phase may replace it while it is retained. Only the first native open can
    /// establish absence. The caller decides whether a recovered temporary may be written.
    ///
    /// # Errors
    /// Refuses unsafe custody, reparse points, hardlinks, competing writers and native errors.
    pub fn open_private_journal_optional(
        &self,
        name: impl AsRef<OsStr>,
    ) -> io::Result<Option<File>> {
        self.inner
            .open_private_journal_optional(checked_name(name.as_ref())?)
    }

    /// Create a private journal, retaining its original before any post-create validation.
    ///
    /// The slot must be empty. On a post-create error it stays populated, so the caller can
    /// retain its pending phase without reopening or deleting an uncertain pathname. The
    /// original excludes other writers and shares read/delete authority for journal moves.
    ///
    /// # Errors
    /// Refuses occupied slots/names, unsafe custody and native creation or durability errors.
    pub fn create_private_journal_into(
        &self,
        name: impl AsRef<OsStr>,
        original: &mut Option<File>,
    ) -> io::Result<()> {
        self.create_private_journal_into_observed(name.as_ref(), original, |_| Ok(()))
    }

    fn create_private_journal_into_observed(
        &self,
        name: &OsStr,
        original: &mut Option<File>,
        observe: impl FnOnce(&File) -> io::Result<()>,
    ) -> io::Result<()> {
        let name = checked_name(name)?;
        if original.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "private journal creation requires an empty original slot",
            ));
        }
        self.inner.create_private_journal_into(name, original)?;
        let file = original.as_ref().ok_or_else(changed)?;
        let result = (|| {
            observe(file)?;
            FileSnapshot::private_journal(file)?;
            self.require_same_lock(name, file)?;
            file.sync_all()?;
            self.sync()?;
            self.require_same_lock(name, file)
        })();
        let final_check = self.revalidate();
        result.and(final_check)
    }

    /// Publish one retained private journal over an exact predecessor or an absent name.
    ///
    /// `published` must initially be false and is set immediately after native rename,
    /// before any observer or fallible post-publication check. A true value on error means
    /// the caller must reconcile and sync that same original, never repeat this rename.
    /// The operation does not sync the file or directory; the journal protocol owns those
    /// boundaries. Replacement uses native POSIX rename semantics so the held predecessor
    /// remains valid. Its private custody and captured identity are checked again after
    /// publication, including on returned observer errors. Both originals remain with the
    /// caller on every result; callers must recheck a displaced original on later retries
    /// and immediately before releasing it.
    ///
    /// # Errors
    /// Refuses changed source/predecessor custody, an occupied expected absence, invalid
    /// names/state, observer failures and native errors. No failure cleans up either name.
    pub fn publish_private_journal_observed(
        &self,
        source_name: impl AsRef<OsStr>,
        destination_name: impl AsRef<OsStr>,
        source: &File,
        predecessor: Option<&File>,
        published: &mut bool,
        observe: impl FnMut(AtomicWriteStep) -> io::Result<()>,
    ) -> io::Result<()> {
        let source_name = checked_name(source_name.as_ref())?;
        let destination_name = checked_name(destination_name.as_ref())?;
        if *published || source_name == destination_name {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "journal publication requires an unpublished original and distinct names",
            ));
        }
        self.inner.publish_private_journal_observed(
            source_name,
            destination_name,
            source,
            predecessor,
            published,
            observe,
        )
    }

    /// Publish a retained private journal from another retained directory on this volume.
    ///
    /// This has the same original, predecessor and publication-latch contract as
    /// [`Self::publish_private_journal_observed`]. Both parents remain borrowed across
    /// all results and are revalidated independently on completion. The caller owns file
    /// durability and both directory barriers; no error closes or removes an original.
    ///
    /// # Errors
    /// Refuses unsafe names, changed originals/parents, a different volume, occupied
    /// expected absence, an already-published latch, observer errors or native failures.
    pub fn publish_private_journal_from_observed(
        &self,
        source_location: (&OwnerDirectory, impl AsRef<OsStr>),
        destination_name: impl AsRef<OsStr>,
        source: &File,
        predecessor: Option<&File>,
        published: &mut bool,
        observe: impl FnMut(AtomicWriteStep) -> io::Result<()>,
    ) -> io::Result<()> {
        let (source_directory, source_name) = source_location;
        let result = (|| {
            let source_name = checked_name(source_name.as_ref())?;
            let destination_name = checked_name(destination_name.as_ref())?;
            if *published
                || (self.identity()? == source_directory.identity()?
                    && source_name == destination_name)
            {
                return Err(invalid(
                    "journal publication requires distinct original names and an unpublished latch",
                ));
            }
            self.inner.publish_private_journal_from_observed(
                (&source_directory.inner, source_name),
                destination_name,
                source,
                predecessor,
                published,
                observe,
            )
        })();
        let source_check = source_directory.revalidate();
        let destination_check = self.revalidate();
        result.and(source_check).and(destination_check)
    }
}

#[cfg(test)]
#[path = "journal_windows_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "journal_cross_parent_windows_tests.rs"]
mod cross_parent_tests;
