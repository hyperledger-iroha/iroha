//! Descriptor-relative private inventory and immutable receipt publication.

use super::*;
use std::os::unix::fs::PermissionsExt as _;

#[cfg(not(target_vendor = "apple"))]
fn private_inventory_metadata(directory: &File, name: &OsStr) -> io::Result<PrivateFileMetadata> {
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
    let access = OFlags::PATH;
    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "freebsd")))]
    let access = OFlags::RDONLY;
    let file = File::from(rustix::fs::openat(
        directory,
        name,
        access | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    let metadata = file.metadata()?;
    let mode = metadata.mode() & 0o7777;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || mode & !0o600 != 0
    {
        return Err(denied(
            "inventory requires a private single-link current-owner file",
        ));
    }
    Ok(PrivateFileMetadata {
        length: metadata.len(),
        read_only: mode == 0o400,
    })
}

pub(super) fn validate_read_only(file: &File) -> io::Result<()> {
    if validate_file(file, true)?.mode() & 0o7777 != 0o400 {
        return Err(denied("immutable private file requires exact mode 0400"));
    }
    Ok(())
}

impl Directory {
    pub(crate) fn open_exact(path: &Path) -> io::Result<Self> {
        Self::open_with_policy(path, false, true, 0, false)
    }
    pub(crate) fn visit_private_files(
        &self,
        maximum: usize,
        mut visitor: impl FnMut(&OsStr, PrivateFileMetadata) -> io::Result<()>,
    ) -> io::Result<()> {
        self.revalidate()?;
        let before = self.current().file.metadata()?;
        let mut count = 0usize;
        for entry in rustix::fs::Dir::read_from(&self.current().file)? {
            let entry = entry?;
            let name = OsStr::from_bytes(entry.file_name().to_bytes());
            if name == "." || name == ".." {
                continue;
            }
            checked_name(name)?;
            if count >= maximum {
                return Err(invalid("directory entry count exceeds the bound"));
            }
            count += 1;
            #[cfg(target_vendor = "apple")]
            let metadata = apple_inventory::private_metadata(&self.current().file, name)?;
            #[cfg(not(target_vendor = "apple"))]
            let metadata = private_inventory_metadata(&self.current().file, name)?;
            visitor(name, metadata)?;
        }
        if !unchanged(&before, &self.current().file.metadata()?) {
            return Err(changed());
        }
        self.revalidate()
    }

    pub(crate) fn create_retained_private(&self, name: &OsStr) -> io::Result<RetainedFile> {
        self.revalidate()?;
        let file = File::from(rustix::fs::openat(
            &self.current().file,
            name,
            OFlags::RDWR
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK
                | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?);
        // Only this descriptor's exclusively created file may override a restrictive umask.
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        let before = validate_file(&file, true)?;
        let retained = RetainedFile {
            directory: self.clone(),
            name: name.to_owned(),
            file,
            before,
            private: true,
            writable: true,
            read_only: false,
            publication: PublicationAuthority::ExclusiveCreation,
        };
        retained.revalidate()?;
        retained.file.sync_all()?;
        self.sync()?;
        Ok(retained)
    }

    pub(crate) fn open_retained_read_only(&self, name: &OsStr) -> io::Result<RetainedFile> {
        let mut file = self.open_retained(name, true, false)?;
        validate_read_only(&file.file)?;
        file.read_only = true;
        file.revalidate()?;
        Ok(file)
    }
}

impl RetainedFile {
    pub(crate) fn seal_read_only(mut self) -> io::Result<Self> {
        if self.publication != PublicationAuthority::ExclusiveCreation || !self.writable {
            return Err(denied("only a newly created writer may be sealed"));
        }
        self.revalidate()?;
        self.file
            .set_permissions(fs::Permissions::from_mode(0o400))?;
        self.file.sync_all()?;
        validate_read_only(&self.file)?;
        self.before = validate_file(&self.file, true)?;
        self.writable = false;
        self.read_only = true;
        self.directory.sync()?;
        self.revalidate()?;
        Ok(self)
    }

    pub(crate) fn publish_new_name(mut self, name: &OsStr) -> io::Result<Self> {
        if self.publication != PublicationAuthority::ExclusiveCreation
            || self.writable
            || !self.read_only
        {
            return Err(denied(
                "publication requires a newly created strictly sealed file",
            ));
        }
        self.revalidate()?;
        let before = self.file.metadata()?;
        publish_new(
            &self.directory.current().file,
            self.name.to_str().ok_or_else(changed)?,
            name,
        )?;
        name.clone_into(&mut self.name);
        let after = validate_file(&self.file, true)?;
        // Rename may change ctime, but must preserve every other recorded content/custody field.
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.mode() != after.mode()
            || before.uid() != after.uid()
            || before.gid() != after.gid()
            || before.nlink() != after.nlink()
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
        {
            return Err(changed());
        }
        self.before = after;
        self.publication = PublicationAuthority::None;
        self.directory.sync()?;
        self.revalidate()?;
        Ok(self)
    }
}
