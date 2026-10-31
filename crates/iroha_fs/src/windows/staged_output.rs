//! Same-original native staging, safe destination admission and publication latch.

use super::*;

struct CreatedStage(Option<File>);

impl Drop for CreatedStage {
    fn drop(&mut self) {
        if let Some(file) = &self.0 {
            let _ = custody_io::dispose(file);
        }
    }
}

impl Directory {
    pub(crate) fn create_output_stage(&self, name: &OsStr) -> io::Result<Option<RetainedFile>> {
        self.create_output_stage_observed(name, |_| Ok(()))
    }

    pub(crate) fn create_output_stage_observed(
        &self,
        name: &OsStr,
        after_create: impl FnOnce(&File) -> io::Result<()>,
    ) -> io::Result<Option<RetainedFile>> {
        self.revalidate()?;
        let file = match open_file(
            &self.path().join(name),
            GENERIC_READ | GENERIC_WRITE | DELETE,
            FILE_SHARE_READ,
            CREATE_NEW,
            false,
            true,
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(None),
            Err(error) => return Err(error),
        };
        // Install cleanup ownership before validation or callbacks can fail/unwind.
        let mut created = CreatedStage(Some(file));
        let result = (|| {
            let file = created.0.as_ref().expect("created original");
            after_create(file)?;
            let before = snapshot(file, true, false)?;
            validate_current_owner(file)?;
            let named = open_file(
                &self.path().join(name),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                OPEN_EXISTING,
                false,
                false,
            )?;
            if snapshot(&named, true, false)? != before || snapshot(file, true, false)? != before {
                return Err(changed());
            }
            Ok(before)
        })();
        let custody = self.revalidate();
        let before = result.and_then(|before| custody.map(|()| before))?;
        Ok(Some(RetainedFile {
            directory: self.clone(),
            name: name.to_owned(),
            file: created.0.take().expect("created original"),
            before,
            links: LinkPolicy::Single,
            private: true,
            writable: true,
            read_only: false,
            executable: false,
            publishable: true,
            ancestry: AncestryCheck::EachCall,
        }))
    }

    fn output_predecessor(&self, name: &OsStr) -> io::Result<Option<File>> {
        self.revalidate()?;
        // DELETE sharing and explicit POSIX replacement keep this exact predecessor
        // valid through publication checks; ordinary retained readers stay unchanged.
        let file = match open_file(
            &self.path().join(name),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            OPEN_EXISTING,
            false,
            false,
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        self.check_output_predecessor(name, &file)?;
        Ok(Some(file))
    }

    fn check_displaced_output_predecessor(&self, file: &File, before: Snapshot) -> io::Result<()> {
        let result = (|| {
            self.revalidate()?;
            // Preserve the output's safe public-read policy; do not require a private DACL.
            validate_acl(file, false, false)?;
            validate_current_owner(file)?;
            let basic: FILE_BASIC_INFO = info(file, FileBasicInfo)?;
            let standard: FILE_STANDARD_INFO = info(file, FileStandardInfo)?;
            if !standard.DeletePending
                || standard.Directory
                || standard.NumberOfLinks != 0
                || basic.FileAttributes != before.attributes
                || basic.FileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY)
                    != 0
                || identity(file)? != before.id
                || u64::try_from(standard.EndOfFile).map_err(|_| changed())? != before.length
                || basic.LastWriteTime != before.modified
            {
                return Err(changed());
            }
            // The namespace transition may update ChangeTime. The admitted original's
            // identity, extent, last write, attributes and custody remain fixed.
            Ok(())
        })();
        let final_check = self.revalidate();
        result.and(final_check)
    }

    fn check_output_predecessor(&self, name: &OsStr, file: &File) -> io::Result<Snapshot> {
        self.revalidate()?;
        let before = snapshot(file, false, false)?;
        validate_current_owner(file)?;
        let named = open_file(
            &self.path().join(name),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            OPEN_EXISTING,
            false,
            false,
        )?;
        validate_current_owner(&named)?;
        if snapshot(&named, false, false)? != before || snapshot(file, false, false)? != before {
            return Err(changed());
        }
        self.revalidate()?;
        Ok(before)
    }
}

impl RetainedFile {
    pub(crate) fn discard_output_stage(&self) -> io::Result<()> {
        if !self.publishable || !self.writable || !self.private {
            return Err(denied("discard requires an unpublished private stage"));
        }
        // The creator retained DELETE access. Dispose its actual original even
        // when changed custody prevents a successful final validation.
        custody_io::dispose(&self.file)
    }

    pub(crate) fn publish_output_stage(
        &mut self,
        destination: &OsStr,
        published: &mut bool,
        observe: &mut impl FnMut(AtomicWriteStep) -> io::Result<()>,
    ) -> io::Result<()> {
        if *published || !self.publishable || !self.writable || !self.private {
            return Err(denied("publication requires its unpublished private stage"));
        }
        let destination = checked_name(destination)?.to_owned();
        if destination == self.name {
            return Err(invalid("output destination equals staging name"));
        }
        self.require_current_owner()?;
        let before = snapshot(&self.file, true, false)?;
        let predecessor = self.directory.output_predecessor(&destination)?;
        let previous = predecessor
            .as_ref()
            .map(|file| self.directory.check_output_predecessor(&destination, file))
            .transpose()?;
        let result = (|| {
            observe(AtomicWriteStep::Publish)?;
            self.require_current_owner()?;
            if snapshot(&self.file, true, false)? != before {
                return Err(changed());
            }
            if let Some(file) = &predecessor {
                if Some(
                    self.directory
                        .check_output_predecessor(&destination, file)?,
                ) != previous
                {
                    return Err(changed());
                }
            } else {
                self.directory.require_absent(&destination)?;
            }
            let destination_path = self.directory.path().join(&destination);
            if predecessor.is_some() {
                replace_retaining_destination(&self.file, &destination_path)?;
            } else {
                rename_handle(&self.file, &destination_path, PublishMode::CreateNew)?;
            }
            // These infallible state changes precede every fallible post-rename step.
            *published = true;
            self.publishable = false;
            self.name = destination;
            observe(AtomicWriteStep::Published)?;
            self.file.sync_all()?;
            let after = snapshot(&self.file, true, false)?;
            let mut expected = before;
            expected.changed = after.changed;
            if after != expected {
                return Err(changed());
            }
            self.require_current_owner()?;
            observe(AtomicWriteStep::SyncDirectory)?;
            self.directory.sync()?;
            self.require_current_owner()
        })();
        // Keep the old original until this independent check has run even if an earlier
        // observer, sync or custody operation returned an error after native publication.
        let predecessor_check = if *published {
            match (predecessor.as_ref(), previous) {
                (Some(file), Some(before)) => self
                    .directory
                    .check_displaced_output_predecessor(file, before),
                (None, None) => Ok(()),
                _ => Err(changed()),
            }
        } else {
            Ok(())
        };
        let parent_check = self.directory.revalidate();
        result.and(predecessor_check).and(parent_check)
    }
}
