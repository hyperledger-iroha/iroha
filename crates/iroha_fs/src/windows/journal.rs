//! Exact original private journal creation and publication under retained native parents.

use super::*;

impl Directory {
    pub(crate) fn require_retired_private_journal(&self, original: &File) -> io::Result<()> {
        let result = (|| {
            self.revalidate()?;
            validate_acl(original, true, false)?;
            let basic: FILE_BASIC_INFO = info(original, FileBasicInfo)?;
            let standard: FILE_STANDARD_INFO = info(original, FileStandardInfo)?;
            // Disposition decrements the admitted single NTFS link to zero while
            // this original remains live. This does not assert pathname absence.
            let refused_attributes = FILE_ATTRIBUTE_REPARSE_POINT
                | FILE_ATTRIBUTE_DIRECTORY
                | windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_READONLY;
            if basic.FileAttributes & refused_attributes != 0
                || !standard.DeletePending
                || standard.Directory
                || standard.NumberOfLinks != 0
            {
                return Err(changed());
            }
            Ok(())
        })();
        let final_check = self.revalidate();
        result.and(final_check)
    }

    pub(crate) fn retire_private_journal_observed(
        &self,
        name: &OsStr,
        original: &File,
        disposed: &mut bool,
        observe: &mut impl FnMut(bool) -> io::Result<()>,
    ) -> io::Result<()> {
        let result = (|| {
            let before = self.require_journal_original(name, original)?;
            observe(false)?;
            if self.require_journal_original(name, original)? != before {
                return Err(changed());
            }
            custody_io::dispose(original)?;
            // This is the kernel transition, not proof that the last handle closed.
            *disposed = true;
            observe(true)?;
            self.require_retired_private_journal(original)?;
            if identity(original)? != before.id {
                return Err(changed());
            }
            Ok(())
        })();
        let custody = self.revalidate();
        result.and(custody)
    }

    fn require_journal_original(&self, name: &OsStr, original: &File) -> io::Result<Snapshot> {
        self.revalidate()?;
        let before = journal_snapshot(original)?.0;
        let named = self
            .open_regular_native(name)?
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        if journal_snapshot(&named)?.0 != before || journal_snapshot(original)?.0 != before {
            return Err(changed());
        }
        self.revalidate()?;
        Ok(before)
    }

    pub(crate) fn open_private_journal_optional(&self, name: &OsStr) -> io::Result<Option<File>> {
        self.revalidate()?;
        let result = match open_file(
            &self.path().join(name),
            GENERIC_READ | GENERIC_WRITE | DELETE,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            OPEN_EXISTING,
            false,
            false,
        ) {
            Ok(file) => self
                .require_journal_original(name, &file)
                .map(|_| Some(file)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        };
        let final_check = self.revalidate();
        match result {
            Ok(file) => final_check.map(|()| file),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn create_private_journal_into(
        &self,
        name: &OsStr,
        original: &mut Option<File>,
    ) -> io::Result<()> {
        self.revalidate()?;
        let file = open_file(
            &self.path().join(name),
            GENERIC_READ | GENERIC_WRITE | DELETE,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            CREATE_NEW,
            false,
            true,
        )?;
        // No fallible operation may precede transfer to the caller's pending owner.
        *original = Some(file);
        Ok(())
    }

    pub(crate) fn publish_private_journal_observed(
        &self,
        source_name: &OsStr,
        destination_name: &OsStr,
        source: &File,
        predecessor: Option<&File>,
        published: &mut bool,
        mut observe: impl FnMut(AtomicWriteStep) -> io::Result<()>,
    ) -> io::Result<()> {
        let mut previous = None;
        let result = (|| {
            let before = self.require_journal_original(source_name, source)?;
            previous = predecessor
                .map(|file| self.require_journal_original(destination_name, file))
                .transpose()?;
            if previous.is_some_and(|value| value.id == before.id) {
                return Err(changed());
            }
            if predecessor.is_none() {
                self.require_absent(destination_name)?;
            }
            observe(AtomicWriteStep::Publish)?;
            if self.require_journal_original(source_name, source)? != before {
                return Err(changed());
            }
            match predecessor {
                Some(file) => {
                    if Some(self.require_journal_original(destination_name, file)?) != previous {
                        return Err(changed());
                    }
                }
                None => self.require_absent(destination_name)?,
            }
            let destination = self.path().join(destination_name);
            if predecessor.is_some() {
                replace_retaining_destination(source, &destination)?;
            } else {
                rename_handle(source, &destination, PublishMode::CreateNew)?;
            }
            *published = true;
            observe(AtomicWriteStep::Published)?;
            let after = self.require_journal_original(destination_name, source)?;
            let mut expected = before;
            expected.changed = after.changed;
            if expected != after {
                return Err(changed());
            }
            self.require_absent(source_name)
        })();
        // Preserve the first refusal, including an observer failure after publication.
        let predecessor_check = if *published {
            (|| {
                if let Some(file) = predecessor {
                    self.require_retired_private_journal(file)?;
                    if Some(identity(file)?) != previous.map(|snapshot| snapshot.id) {
                        return Err(changed());
                    }
                }
                Ok(())
            })()
        } else {
            Ok(())
        };
        let final_check = self.revalidate();
        result.and(predecessor_check).and(final_check)
    }
    pub(crate) fn publish_private_journal_from_observed(
        &self,
        source_location: (&Directory, &OsStr),
        destination_name: &OsStr,
        source: &File,
        predecessor: Option<&File>,
        published: &mut bool,
        mut observe: impl FnMut(AtomicWriteStep) -> io::Result<()>,
    ) -> io::Result<()> {
        let (source_directory, source_name) = source_location;
        let mut previous = None;
        let result = (|| {
            let before = source_directory.require_journal_original(source_name, source)?;
            self.revalidate()?;
            if before.id.volume != self.identity()?.volume {
                return Err(invalid("journal publication requires the original volume"));
            }
            previous = predecessor
                .map(|file| self.require_journal_original(destination_name, file))
                .transpose()?;
            if previous.is_some_and(|value| value.id == before.id) {
                return Err(changed());
            }
            if predecessor.is_none() {
                self.require_absent(destination_name)?;
            }
            observe(AtomicWriteStep::Publish)?;
            if source_directory.require_journal_original(source_name, source)? != before {
                return Err(changed());
            }
            match predecessor {
                Some(file) => {
                    if Some(self.require_journal_original(destination_name, file)?) != previous {
                        return Err(changed());
                    }
                }
                None => self.require_absent(destination_name)?,
            }
            let destination = self.path().join(destination_name);
            if predecessor.is_some() {
                replace_retaining_destination(source, &destination)?;
            } else {
                rename_handle(source, &destination, PublishMode::CreateNew)?;
            }
            *published = true;
            observe(AtomicWriteStep::Published)?;
            let after = self.require_journal_original(destination_name, source)?;
            let mut expected = before;
            expected.changed = after.changed;
            if expected != after {
                return Err(changed());
            }
            source_directory.require_absent(source_name)
        })();
        // Preserve the first refusal, including an observer failure after publication.
        let predecessor_check = if *published {
            (|| {
                if let Some(file) = predecessor {
                    self.require_retired_private_journal(file)?;
                    if Some(identity(file)?) != previous.map(|snapshot| snapshot.id) {
                        return Err(changed());
                    }
                }
                Ok(())
            })()
        } else {
            Ok(())
        };
        let source_check = source_directory.revalidate();
        let final_check = self.revalidate();
        result
            .and(predecessor_check)
            .and(source_check)
            .and(final_check)
    }
}
