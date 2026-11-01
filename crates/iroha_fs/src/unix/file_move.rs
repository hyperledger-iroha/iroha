//! Descriptor-relative original-object no-replace moves.

use super::*;

impl Directory {
    pub(crate) fn open_movable(&self, name: &OsStr) -> io::Result<RetainedFile> {
        let retained = self.open_retained(name, false, false)?;
        if retained.before.uid() != rustix::process::geteuid().as_raw() {
            return Err(denied("movable file requires current ownership"));
        }
        retained.revalidate()?;
        Ok(retained)
    }

    pub(crate) fn open_movable_private(&self, name: &OsStr) -> io::Result<RetainedFile> {
        let retained = self.open_retained(name, true, false)?;
        journal_snapshot(&retained.file)?;
        retained.revalidate()?;
        Ok(retained)
    }

    pub(crate) fn require_absent(&self, name: &OsStr) -> io::Result<()> {
        self.revalidate()?;
        let result = rustix::fs::statat(&self.current().file, name, AtFlags::SYMLINK_NOFOLLOW);
        self.revalidate()?;
        match result {
            Err(rustix::io::Errno::NOENT) => Ok(()),
            Err(error) => Err(error.into()),
            Ok(_) => Err(io::ErrorKind::AlreadyExists.into()),
        }
    }
}

impl RetainedFile {
    pub(crate) fn rename_private_original(&mut self, name: &OsStr) -> io::Result<()> {
        if !self.private || self.writable || self.read_only || self.executable {
            return Err(denied(
                "rename requires retained writable-private move authority",
            ));
        }
        self.revalidate()?;
        journal_snapshot(&self.file)?;
        self.directory.require_absent(name)?;
        rename_no_replace(
            &self.directory.current().file,
            &self.name,
            &self.directory.current().file,
            name,
        )?;
        name.clone_into(&mut self.name);
        Ok(())
    }

    pub(crate) fn finish_private_rename(&mut self) -> io::Result<()> {
        journal_snapshot(&self.file)?;
        let after = validate_file(&self.file, true)?;
        let mut expected = FileSnapshot::from_metadata(&self.before);
        let actual = FileSnapshot::from_metadata(&after);
        expected.changed = actual.changed;
        if expected != actual {
            return Err(changed());
        }
        self.before = after;
        self.revalidate()
    }

    pub(crate) fn finish_private_for_move(
        mut self,
    ) -> io::Result<(Self, Directory, std::ffi::OsString)> {
        if self.publication != PublicationAuthority::ExclusiveCreation
            || !self.writable
            || !self.private
            || self.read_only
            || self.executable
        {
            return Err(denied(
                "private move finish requires the exclusive original writer",
            ));
        }
        self.revalidate()?;
        self.file.sync_all()?;
        self.before = validate_file(&self.file, true)?;
        self.writable = false;
        self.directory.sync()?;
        self.revalidate()?;
        let directory = self.directory.clone();
        let name = self.name.clone();
        Ok((self, directory, name))
    }

    pub(crate) fn into_move_parts(self) -> io::Result<(Self, Directory, std::ffi::OsString)> {
        self.revalidate()?;
        if self.publication != PublicationAuthority::ExclusiveCreation
            || self.writable
            || !(self.read_only || self.executable)
        {
            return Err(denied(
                "move conversion requires original sealed creation authority",
            ));
        }
        let directory = self.directory.clone();
        let name = self.name.clone();
        Ok((self, directory, name))
    }

    pub(crate) fn move_no_replace(
        mut self,
        destination: &Directory,
        name: &OsStr,
        published: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<Self> {
        self.revalidate()?;
        destination.revalidate()?;
        rename_no_replace(
            &self.directory.current().file,
            &self.name,
            &destination.current().file,
            name,
        )?;
        published()?;
        let after = validate_file(&self.file, false)?;
        let mut expected = FileSnapshot::from_metadata(&self.before);
        let actual = FileSnapshot::from_metadata(&after);
        // Native rename may advance ctime; every other exact snapshot field is preserved.
        expected.changed = actual.changed;
        if expected != actual {
            return Err(changed());
        }
        self.directory.revalidate()?;
        self.directory.require_absent(&self.name)?;
        self.directory = destination.clone();
        name.clone_into(&mut self.name);
        self.before = after;
        self.revalidate()?;
        Ok(self)
    }
}

#[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
pub(super) fn rename_no_replace(
    source: &File,
    source_name: &OsStr,
    destination: &File,
    name: &OsStr,
) -> io::Result<()> {
    Ok(rustix::fs::renameat_with(
        source,
        source_name,
        destination,
        name,
        rustix::fs::RenameFlags::NOREPLACE,
    )?)
}

#[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android")))]
pub(super) fn rename_no_replace(_: &File, _: &OsStr, _: &File, _: &OsStr) -> io::Result<()> {
    // TODO: add a native atomic no-replace rename for other Unix platforms before admitting
    // moves there. Link/unlink would violate the single-link source and interruption contract.
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic original-file move is unsupported on this Unix host",
    ))
}
