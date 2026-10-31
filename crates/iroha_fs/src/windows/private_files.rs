//! Protected read-only receipt ACLs and retained native no-replace publication.

use super::*;
use windows_sys::Win32::{
    Security::{
        Authorization::SetSecurityInfo, GetSecurityDescriptorDacl,
        PROTECTED_DACL_SECURITY_INFORMATION,
    },
    Storage::FileSystem::{FILE_GENERIC_EXECUTE, FILE_GENERIC_READ},
};

fn read_only_acl(file: &File) -> io::Result<bool> {
    sealed_acl(file, FILE_GENERIC_READ)
}

fn sealed_acl(file: &File, access: u32) -> io::Result<bool> {
    let user = UserSid::current()?;
    let mut owner = null_mut();
    let mut dacl: *mut ACL = null_mut();
    let mut descriptor = null_mut();
    // SAFETY: retained file handle and initialized output slots remain live.
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &raw mut owner,
            null_mut(),
            &raw mut dacl,
            null_mut(),
            &raw mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status.cast_signed()));
    }
    let _allocation = LocalAllocation(descriptor);
    if !user.equals(owner) || dacl.is_null() {
        return Ok(false);
    }
    let mut control = 0;
    let mut revision = 0;
    // SAFETY: GetSecurityInfo supplied a complete descriptor and bounded ACL.
    unsafe {
        win_ok(GetSecurityDescriptorControl(
            descriptor,
            &raw mut control,
            &raw mut revision,
        ))?;
        if control & SE_DACL_PROTECTED == 0 || (*dacl).AceCount != 1 {
            return Ok(false);
        }
    }
    let mut ace = null_mut();
    // SAFETY: the sole ACE is retained inside the descriptor allocation.
    unsafe {
        win_ok(GetAce(dacl, 0, &raw mut ace))?;
    }
    // SAFETY: all valid ACEs begin with ACE_HEADER.
    let header = unsafe { &*ace.cast::<ACE_HEADER>() };
    if header.AceType != 0
        || header.AceFlags != 0
        || usize::from(header.AceSize) < size_of::<ACCESS_ALLOWED_ACE>()
    {
        return Ok(false);
    }
    // SAFETY: checked the allowed-ACE type and minimum layout above.
    let allowed = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
    Ok(allowed.Mask == access && user.equals(from_ref(&allowed.SidStart).cast_mut().cast()))
}

pub(super) fn validate_executable(file: &File) -> io::Result<()> {
    if !sealed_acl(file, FILE_GENERIC_READ | FILE_GENERIC_EXECUTE)? {
        return Err(denied(
            "private executable requires protected owner-read-execute DACL",
        ));
    }
    Ok(())
}

pub(super) fn validate_read_only(file: &File) -> io::Result<()> {
    if !read_only_acl(file)? {
        return Err(denied(
            "immutable private file requires protected owner-read-only DACL",
        ));
    }
    Ok(())
}

fn set_read_only_acl(file: &File) -> io::Result<()> {
    set_sealed_acl(file, false)
}

pub(super) fn set_sealed_acl(file: &File, executable: bool) -> io::Result<()> {
    let user = UserSid::current()?;
    let mut sid_text = null_mut();
    // SAFETY: the current-user SID is valid and the output is owned until return.
    unsafe {
        win_ok(ConvertSidToStringSidW(user.pointer(), &raw mut sid_text))?;
    }
    let _sid_allocation = LocalAllocation(sid_text.cast());
    let mut length = 0;
    // SAFETY: ConvertSidToStringSidW returns a terminated UTF-16 allocation.
    unsafe {
        while *sid_text.add(length) != 0 {
            length += 1;
        }
    }
    // SAFETY: measured initialized SID string excludes the terminator.
    let sid = String::from_utf16(unsafe { std::slice::from_raw_parts(sid_text, length) })
        .map_err(|_| invalid("native SID text is invalid"))?;
    let access = if executable { "FRFX" } else { "FR" };
    let sddl = wide(OsStr::new(&format!("O:{sid}D:P(A;;{access};;;{sid})")))?;
    let mut descriptor = null_mut();
    // SAFETY: terminated source remains live and output is one owned native allocation.
    unsafe {
        win_ok(ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &raw mut descriptor,
            null_mut(),
        ))?;
    }
    let _allocation = LocalAllocation(descriptor);
    let mut present = 0;
    let mut defaulted = 0;
    let mut dacl = null_mut();
    // SAFETY: the complete descriptor is retained through the synchronous ACL update.
    unsafe {
        win_ok(GetSecurityDescriptorDacl(
            descriptor,
            &raw mut present,
            &raw mut dacl,
            &raw mut defaulted,
        ))?;
        if present == 0 || dacl.is_null() {
            return Err(denied("missing immutable DACL"));
        }
        let status = SetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            dacl,
            null_mut(),
        );
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status.cast_signed()));
        }
    }
    if executable {
        validate_executable(file)
    } else {
        validate_read_only(file)
    }
}

impl Directory {
    pub(crate) fn open_exact(path: &Path) -> io::Result<Self> {
        Self::open(path, false)
    }
    pub(crate) fn visit_private_files(
        &self,
        maximum: usize,
        mut visitor: impl FnMut(&OsStr, PrivateFileMetadata) -> io::Result<()>,
    ) -> io::Result<()> {
        self.revalidate()?;
        // NTFS directory timestamps can remain unchanged after a fast child mutation.
        // Keep a latched native notification across the entire streamed callback scan.
        let changes = InventoryChanges::begin(self.path())?;
        let before = snapshot(&self.current().file, self.current().private, true)?;
        for (count, entry) in std::fs::read_dir(self.path())?.enumerate() {
            let name = entry?.file_name();
            checked_name(&name)?;
            if count >= maximum {
                return Err(invalid("directory entry count exceeds the bound"));
            }
            let file = open_file(
                &self.path().join(&name),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                OPEN_EXISTING,
                false,
                false,
            )?;
            let value = snapshot(&file, true, false)?;
            let read_only = read_only_acl(&file)?;
            visitor(
                &name,
                PrivateFileMetadata {
                    length: value.length,
                    read_only,
                },
            )?;
        }
        if before != snapshot(&self.current().file, self.current().private, true)? {
            return Err(changed());
        }
        changes.require_unchanged()?;
        self.revalidate()
    }

    pub(crate) fn create_retained_private(&self, name: &OsStr) -> io::Result<RetainedFile> {
        self.open_retained(name, true, true)
    }

    pub(crate) fn open_retained_read_only(&self, name: &OsStr) -> io::Result<RetainedFile> {
        RetainedFile::open_read_only(self.clone(), name.to_owned())
    }

    pub(crate) fn open_retained_executable(&self, name: &OsStr) -> io::Result<RetainedFile> {
        // Ordinary read admission retains a GENERIC_READ handle with FILE_SHARE_READ
        // only; recovered execution never acquires the creator's write/delete rights.
        let mut file = self.open_retained(name, false, false)?;
        file.executable = true;
        file.revalidate()?;
        Ok(file)
    }

    pub(crate) fn open_retained_optional(
        &self,
        name: &OsStr,
        private: bool,
    ) -> io::Result<Option<RetainedFile>> {
        // Preserve ordinary retained admission's exclusive sharing fence.
        let file = match open_file(
            &self.path().join(name),
            GENERIC_READ,
            FILE_SHARE_READ,
            OPEN_EXISTING,
            false,
            false,
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let before = snapshot(&file, private, false)?;
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
        Ok(Some(retained))
    }

    pub(crate) fn open_retained_read_only_optional(
        &self,
        name: &OsStr,
    ) -> io::Result<Option<RetainedFile>> {
        self.admit_sealed_optional(name, self.clone(), AncestryCheck::EachCall)
    }

    // The same optional sealed admission inside a closed read transaction, which owns the
    // complete ancestry checks at its entry and exit.
    pub(crate) fn open_scoped_read_only_optional(
        &self,
        name: &OsStr,
    ) -> io::Result<Option<RetainedFile<&Self>>> {
        self.admit_sealed_optional(name, self, AncestryCheck::EnclosingScope)
    }

    // Sole strict sealed admission for both owners of the ancestry checks: one leaf recipe.
    fn admit_sealed_optional<D: Borrow<Self>>(
        &self,
        name: &OsStr,
        directory: D,
        ancestry: AncestryCheck,
    ) -> io::Result<Option<RetainedFile<D>>> {
        // Preserve retained-file sharing rights. Only this initial native open
        // establishes absence; snapshot, ACL and named revalidation never do.
        let file = match open_file(
            &self.path().join(name),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            OPEN_EXISTING,
            false,
            false,
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let before = snapshot(&file, true, false)?;
        validate_read_only(&file)?;
        let retained = RetainedFile {
            directory,
            name: name.to_owned(),
            file,
            before,
            links: LinkPolicy::Single,
            private: true,
            writable: false,
            read_only: true,
            executable: false,
            publishable: false,
            ancestry,
        };
        retained.revalidate()?;
        Ok(Some(retained))
    }

    pub(crate) fn create_borrowed_private<'a>(
        &'a self,
        name: &'a OsStr,
    ) -> io::Result<RetainedFile<&'a Self, &'a OsStr>> {
        RetainedFile::open(self, name, true, true)
    }

    pub(crate) fn open_borrowed_read_only<'a>(
        &'a self,
        name: &'a OsStr,
    ) -> io::Result<RetainedFile<&'a Self, &'a OsStr>> {
        RetainedFile::open_read_only(self, name)
    }
}

/// One fixed native namespace/security notification for a retained local NTFS directory.
#[derive(Debug)]
pub(super) struct InventoryChanges(isize);

impl InventoryChanges {
    pub(super) fn begin(path: &Path) -> io::Result<Self> {
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_DIR_NAME,
            FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE,
            FILE_NOTIFY_CHANGE_SECURITY, FILE_NOTIFY_CHANGE_SIZE,
        };
        Self::begin_filtered(
            path,
            FILE_NOTIFY_CHANGE_FILE_NAME
                | FILE_NOTIFY_CHANGE_DIR_NAME
                | FILE_NOTIFY_CHANGE_ATTRIBUTES
                | FILE_NOTIFY_CHANGE_SECURITY
                | FILE_NOTIFY_CHANGE_SIZE
                | FILE_NOTIFY_CHANGE_LAST_WRITE,
        )
    }

    pub(super) fn begin_security(path: &Path) -> io::Result<Self> {
        Self::begin_filtered(
            path,
            windows_sys::Win32::Storage::FileSystem::FILE_NOTIFY_CHANGE_SECURITY,
        )
    }

    fn begin_filtered(path: &Path, filter: u32) -> io::Result<Self> {
        use windows_sys::Win32::Storage::FileSystem::FindFirstChangeNotificationW;
        let path = wide(path.as_os_str())?;
        // SAFETY: the path is terminated and its original directory/ancestors remain pinned.
        let handle = unsafe { FindFirstChangeNotificationW(path.as_ptr(), 0, filter) };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // Notification handles are process-wide kernel objects; integer storage allows
        // shared immutable polling across threads. Only Drop closes this unique handle.
        Ok(Self(handle as isize))
    }

    pub(super) fn require_unchanged(&self) -> io::Result<()> {
        use windows_sys::Win32::{
            Foundation::{WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
            System::Threading::WaitForSingleObject,
        };
        // SAFETY: this owner retains the notification handle until Drop. Never reset its latch.
        match unsafe { WaitForSingleObject(self.0 as _, 0) } {
            WAIT_TIMEOUT => Ok(()),
            WAIT_OBJECT_0 => Err(changed()),
            WAIT_FAILED => Err(io::Error::last_os_error()),
            _ => Err(io::Error::other("unexpected inventory notification state")),
        }
    }
}

impl Drop for InventoryChanges {
    fn drop(&mut self) {
        // SAFETY: exactly this owner closes the live notification; it is not a file handle.
        unsafe {
            windows_sys::Win32::Storage::FileSystem::FindCloseChangeNotification(self.0 as _);
        }
    }
}

impl<D: Borrow<Directory>, N: AsRef<OsStr>> RetainedFile<D, N> {
    fn open_read_only(directory: D, name: N) -> io::Result<Self> {
        let parent = directory.borrow();
        parent.revalidate()?;
        // Metadata-frozen creator capabilities can still hold their original write/delete
        // rights. Admit a read-only handle without requiring those exact retained objects to
        // close; strict DACL and snapshot checks continue to govern every use.
        let file = open_file(
            &parent.path().join(name.as_ref()),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            OPEN_EXISTING,
            false,
            false,
        )?;
        let before = snapshot(&file, true, false)?;
        validate_read_only(&file)?;
        let retained = Self {
            directory,
            name,
            file,
            before,
            links: LinkPolicy::Single,
            private: true,
            writable: false,
            read_only: true,
            executable: false,
            publishable: false,
            ancestry: AncestryCheck::EachCall,
        };
        retained.revalidate()?;
        Ok(retained)
    }
    pub(crate) fn seal_read_only(mut self) -> io::Result<Self> {
        if !self.publishable || !self.writable {
            return Err(denied("only a newly created writer may be sealed"));
        }
        self.revalidate()?;
        self.file.sync_all()?;
        set_read_only_acl(&self.file)?;
        self.file.sync_all()?;
        self.before = snapshot(&self.file, true, false)?;
        self.writable = false;
        self.read_only = true;
        self.directory.borrow().sync()?;
        self.revalidate()?;
        Ok(self)
    }

    pub(crate) fn seal_executable(mut self) -> io::Result<Self> {
        if !self.publishable || !self.writable {
            return Err(denied(
                "only a newly created writer may be sealed executable",
            ));
        }
        self.revalidate()?;
        self.file.sync_all()?;
        set_sealed_acl(&self.file, true)?;
        self.file.sync_all()?;
        self.before = snapshot(&self.file, true, false)?;
        self.writable = false;
        self.executable = true;
        self.directory.borrow().sync()?;
        self.revalidate()?;
        Ok(self)
    }

    pub(crate) fn publish_with_name<M: AsRef<OsStr>>(
        self,
        name: M,
    ) -> io::Result<RetainedFile<D, M>> {
        if !self.publishable || self.writable || !self.read_only {
            return Err(denied(
                "publication requires a newly created strictly sealed file",
            ));
        }
        self.revalidate()?;
        let before = snapshot(&self.file, true, false)?;
        rename_handle(
            &self.file,
            &self.directory.borrow().path().join(name.as_ref()),
            PublishMode::CreateNew,
        )?;
        self.file.sync_all()?;
        let after = snapshot(&self.file, true, false)?;
        if before.id != after.id
            || before.length != after.length
            || before.modified != after.modified
            || before.attributes != after.attributes
        {
            return Err(changed());
        }
        let published = RetainedFile {
            directory: self.directory,
            name,
            file: self.file,
            before: after,
            links: LinkPolicy::Single,
            private: self.private,
            writable: self.writable,
            read_only: self.read_only,
            executable: self.executable,
            publishable: false,
            ancestry: self.ancestry,
        };
        published.directory.borrow().sync()?;
        published.revalidate()?;
        Ok(published)
    }
}

impl RetainedFile {
    pub(crate) fn publish_new_name(self, name: &OsStr) -> io::Result<Self> {
        self.publish_with_name(name.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_acl_is_exact_and_distinct_from_receipt_access() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = Directory::open(&temporary.path().join("private"), true).unwrap();
        let mut pending = directory
            .create_retained_private(OsStr::new("native"))
            .unwrap();
        pending.file_mut().write_all(b"native").unwrap();
        let sealed = pending.seal_executable().unwrap();
        assert!(sealed_acl(&sealed.file, FILE_GENERIC_READ | FILE_GENERIC_EXECUTE).unwrap());
        assert!(!read_only_acl(&sealed.file).unwrap());
        assert!(validate_read_only(&sealed.file).is_err());
        sealed.revalidate().unwrap();
        // The retained creator can alter ACLs internally, but the public capability
        // exposes no such handle and subsequent custody must reject even narrower access.
        set_read_only_acl(&sealed.file).unwrap();
        assert!(read_only_acl(&sealed.file).unwrap());
        assert!(sealed.revalidate().is_err());
    }

    #[test]
    fn borrowed_reopen_rejects_shared_links_even_with_an_exact_read_only_dacl() {
        let temporary = tempfile::tempdir().unwrap();
        let directory =
            PrivateDirectory::open_or_create(temporary.path().join("receipts")).unwrap();
        let name = OsStr::new("receipt");
        let path = directory.path().join(name);
        // Build the hostile native fixture before applying the exact immutable ACL. This
        // does not use a retained receipt constructor or bypass any production validation.
        let mut file = open_file(
            &path,
            GENERIC_READ | GENERIC_WRITE | WRITE_DAC,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            CREATE_NEW,
            false,
            true,
        )
        .unwrap();
        file.write_all(b"original").unwrap();
        std::fs::hard_link(&path, directory.path().join("shared")).unwrap();
        set_read_only_acl(&file).unwrap();
        assert!(read_only_acl(&file).unwrap());
        drop(file);
        for name in [name, OsStr::new("shared")] {
            let error = directory.open_borrowed_read_only(name, 16).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(error.to_string().contains("shared links"));
        }
    }

    #[test]
    fn borrowed_reopen_rejects_native_file_and_directory_reparse_points() {
        use std::os::windows::fs::{symlink_dir, symlink_file};

        let temporary = tempfile::tempdir().unwrap();
        let directory =
            PrivateDirectory::open_or_create(temporary.path().join("receipts")).unwrap();
        let mut writer = directory
            .create_borrowed_private(OsStr::new("receipt"), 16)
            .unwrap();
        writer.write_all(b"original").unwrap();
        let sealed = writer.seal_read_only().unwrap();
        let before = sealed.snapshot().unwrap();
        // Native Windows qualification must provide symlink privilege or Developer Mode;
        // do not silently skip the reparse-point boundary when that fixture cannot be made.
        symlink_file("receipt", directory.path().join("redirect"))
            .expect("native Windows reparse fixture requires symlink creation authority");
        assert!(
            directory
                .open_borrowed_read_only(OsStr::new("redirect"), 16)
                .is_err()
        );
        assert!(
            directory
                .create_borrowed_private(OsStr::new("redirect"), 16)
                .is_err()
        );
        let alias = temporary.path().join("directory-redirect");
        symlink_dir(directory.path(), &alias)
            .expect("native Windows reparse fixture requires symlink creation authority");
        assert!(PrivateDirectory::open_exact(&alias).is_err());
        assert_eq!(sealed.snapshot().unwrap(), before);
        std::fs::remove_file(directory.path().join("redirect")).unwrap();
        std::fs::remove_dir(alias).unwrap();
    }
}
