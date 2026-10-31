//! Exact-handle original-file moves through retained NTFS namespace parents.

use super::*;

impl Directory {
    pub(crate) fn open_movable(&self, name: &OsStr) -> io::Result<RetainedFile> {
        self.open_movable_with_policy(name, false)
    }

    pub(crate) fn open_movable_private(&self, name: &OsStr) -> io::Result<RetainedFile> {
        self.open_movable_with_policy(name, true)
    }

    fn open_movable_with_policy(&self, name: &OsStr, private: bool) -> io::Result<RetainedFile> {
        self.revalidate()?;
        let file = open_file(
            &self.path().join(name),
            GENERIC_READ | DELETE,
            FILE_SHARE_READ,
            OPEN_EXISTING,
            false,
            false,
        )?;
        validate_current_owner(&file)?;
        let before = if private {
            journal_snapshot(&file)?.0
        } else {
            snapshot(&file, false, false)?
        };
        let retained = RetainedFile {
            directory: self.clone(),
            name: name.to_owned(),
            file,
            before,
            links: LinkPolicy::Single,
            private,
            writable: false,
            read_only: false,
            executable: false,
            publishable: false,
            ancestry: AncestryCheck::EachCall,
        };
        retained.revalidate()?;
        Ok(retained)
    }

    pub(crate) fn require_absent(&self, name: &OsStr) -> io::Result<()> {
        self.revalidate()?;
        let result = open_file(
            &self.path().join(name),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            OPEN_EXISTING,
            false,
            false,
        );
        self.revalidate()?;
        match result {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
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
        rename_handle(
            &self.file,
            &self.directory.path().join(name),
            PublishMode::CreateNew,
        )?;
        // Update only memory before Published: no fallible observation may obscure rename.
        name.clone_into(&mut self.name);
        Ok(())
    }

    pub(crate) fn finish_private_rename(&mut self) -> io::Result<()> {
        let after = journal_snapshot(&self.file)?.0;
        let mut expected = self.before;
        expected.changed = after.changed;
        if expected != after {
            return Err(changed());
        }
        self.before = after;
        self.revalidate()
    }

    pub(crate) fn finish_private_for_move(
        mut self,
    ) -> io::Result<(Self, Directory, std::ffi::OsString)> {
        if !self.publishable || !self.writable || !self.private || self.read_only || self.executable
        {
            return Err(denied(
                "private move finish requires the exclusive original writer",
            ));
        }
        self.revalidate()?;
        self.file.sync_all()?;
        self.before = snapshot(&self.file, true, false)?;
        self.writable = false;
        self.directory.sync()?;
        self.revalidate()?;
        let directory = self.directory.clone();
        let name = self.name.clone();
        Ok((self, directory, name))
    }

    pub(crate) fn into_move_parts(self) -> io::Result<(Self, Directory, std::ffi::OsString)> {
        self.revalidate()?;
        if !self.publishable || self.writable || !(self.read_only || self.executable) {
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
        // Both complete ancestor chains retain non-delete-sharing handles. The DELETE
        // source handle was opened WRITE_THROUGH; no source path is reopened for rename.
        rename_handle(
            &self.file,
            &destination.path().join(name),
            PublishMode::CreateNew,
        )?;
        published()?;
        validate_current_owner(&self.file)?;
        let after = snapshot(&self.file, false, false)?;
        let mut expected = self.before;
        expected.changed = after.changed;
        if expected != after {
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
