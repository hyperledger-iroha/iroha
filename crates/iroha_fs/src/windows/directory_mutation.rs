//! Borrowed directory mutations with originals retained across every ordinary failure.
//!
//! This capability deliberately shares deletion. Caller serialization and repeated original
//! identity/name checks detect another same-user actor; this is not a namespace reservation.
//! Ordinary `OwnerDirectory` and reader sharing policies remain separate and unchanged.

use super::*;
use std::sync::{OnceLock, atomic::AtomicBool};
use windows_sys::Win32::{
    Foundation::{HANDLE, RtlNtStatusToDosError},
    Storage::FileSystem::{FILE_DISPOSITION_INFO_EX, FileDispositionInfoEx},
};

const SHARE: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;

// These documented native layouts allow an exclusive directory create to return the exact
// original in the same syscall. CreateDirectoryW followed by an open cannot provide that.
#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}

#[repr(C)]
struct ObjectAttributes {
    length: u32,
    root_directory: HANDLE,
    object_name: *mut UnicodeString,
    attributes: u32,
    security_descriptor: *mut c_void,
    security_quality_of_service: *mut c_void,
}

#[repr(C)]
struct IoStatusBlock {
    status_or_pointer: usize,
    information: usize,
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtSetInformationFile(
        file: HANDLE,
        status: *mut IoStatusBlock,
        information: *mut c_void,
        length: u32,
        class: i32,
    ) -> i32;

    fn NtCreateFile(
        file: *mut HANDLE,
        access: u32,
        attributes: *mut ObjectAttributes,
        status: *mut IoStatusBlock,
        allocation_size: *mut i64,
        file_attributes: u32,
        share: u32,
        disposition: u32,
        options: u32,
        ea_buffer: *mut c_void,
        ea_length: u32,
    ) -> i32;
}

fn relative_open(
    parent: &File,
    name: &OsStr,
    access: u32,
    create: bool,
    descriptor: *mut c_void,
) -> io::Result<File> {
    let mut name = wide(checked_name(name)?)?;
    let length = u16::try_from((name.len() - 1) * 2)
        .map_err(|_| invalid("native directory component is too long"))?;
    let mut unicode = UnicodeString {
        length,
        maximum_length: length,
        buffer: name.as_mut_ptr(),
    };
    let mut attributes = ObjectAttributes {
        length: native_size::<ObjectAttributes>()?,
        root_directory: parent.as_raw_handle(),
        object_name: &raw mut unicode,
        attributes: 0x40, // OBJ_CASE_INSENSITIVE; the new handle is not inheritable.
        security_descriptor: descriptor,
        security_quality_of_service: null_mut(),
    };
    let mut status = IoStatusBlock {
        status_or_pointer: 0,
        information: 0,
    };
    let mut file = null_mut();
    // SAFETY: every pointer references a live, correctly aligned native layout. RootDirectory
    // is the retained parent, ObjectName is one checked component, and FILE_CREATE is exclusive.
    let result = unsafe {
        NtCreateFile(
            &raw mut file,
            access | READ_CONTROL | SYNCHRONIZE_ACCESS,
            &raw mut attributes,
            &raw mut status,
            null_mut(),
            FILE_ATTRIBUTE_DIRECTORY,
            SHARE,
            if create { 2 } else { 1 }, // FILE_CREATE / FILE_OPEN
            0x1 | 0x20 | 0x0020_0000 // DIRECTORY_FILE, SYNCHRONOUS_IO_NONALERT, OPEN_REPARSE_POINT
                | if access & (FILE_WRITE_ATTRIBUTES | DELETE) != 0 { 0x2 } else { 0 }, // WRITE_THROUGH
            null_mut(),
            0,
        )
    };
    if result < 0 {
        // SAFETY: conversion is defined for any NTSTATUS and has no borrowed data.
        return Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(result) }.cast_signed(),
        ));
    }
    // SAFETY: successful synchronous NtCreateFile returned one owned non-inheritable handle.
    Ok(unsafe { File::from_raw_handle(file) })
}

#[derive(Debug)]
struct Original {
    file: File,
    identity: OnceLock<FileIdentity>,
    removed: AtomicBool,
}

impl Original {
    fn new(file: File) -> Self {
        Self {
            file,
            identity: OnceLock::new(),
            removed: AtomicBool::new(false),
        }
    }

    fn identity(&self) -> io::Result<FileIdentity> {
        let actual = identity(&self.file)?;
        let expected = self.identity.get_or_init(|| actual);
        if *expected != actual {
            return Err(changed());
        }
        Ok(actual)
    }

    fn revalidate(&self) -> io::Result<()> {
        self.identity()?;
        if self.removed.load(Ordering::Acquire) {
            let basic: FILE_BASIC_INFO = info(&self.file, FileBasicInfo)?;
            let standard: FILE_STANDARD_INFO = info(&self.file, FileStandardInfo)?;
            if basic.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
                || basic.FileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
                || !standard.DeletePending
            {
                return Err(changed());
            }
            validate_acl(&self.file, false, true)?;
            validate_current_owner(&self.file)?;
        } else {
            snapshot(&self.file, false, true)?;
        }
        Ok(())
    }
}

/// Observable boundaries of an original-directory mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectoryMutationStep {
    /// The created original is installed in the caller's slot before admission.
    Created,
    /// Native no-replace rename succeeded; the publication latch is already true.
    Published,
    /// Native empty-only disposition succeeded and its scoped handle has been closed.
    Removed,
}

/// A retained directory original for a caller-owned, retryable namespace transaction.
///
/// Every mutation borrows this owner. Originals and captured ancestors survive errors, and
/// no method returns a raw handle or a consuming error. These dedicated handles share rename
/// authority: serialize the surrounding protocol, retain parents, and authenticate the named
/// originals at each phase. This capability does not alter ordinary reader/name exclusion.
#[derive(Clone, Debug)]
pub struct DirectoryMutation {
    original: Arc<Original>,
    ancestors: Vec<Arc<Original>>,
}

fn walk(path: &Path, mutation: bool) -> io::Result<DirectoryMutation> {
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(invalid("native drive path required"));
    };
    if !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        || !matches!(components.next(), Some(Component::RootDir))
    {
        return Err(invalid(
            "directory mutations require an absolute local drive path",
        ));
    }
    let mut root_name = prefix.as_os_str().to_owned();
    root_name.push("\\");
    let root = open_file(
        Path::new(&root_name),
        FILE_READ_ATTRIBUTES,
        SHARE,
        OPEN_EXISTING,
        true,
        false,
    )?;
    snapshot(&root, false, true)?;
    let mut filesystem = [0u16; 32];
    // SAFETY: retained root and bounded filesystem-name output; unused outputs are null.
    unsafe {
        win_ok(GetVolumeInformationByHandleW(
            root.as_raw_handle(),
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            null_mut(),
            filesystem.as_mut_ptr(),
            32,
        ))?;
    }
    let end = filesystem
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(filesystem.len());
    if String::from_utf16_lossy(&filesystem[..end]) != "NTFS" {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "directory mutations require NTFS",
        ));
    }
    let names: Vec<_> = components
        .map(|part| match part {
            Component::Normal(name) => checked_name(name),
            _ => Err(invalid("normal native directory components required")),
        })
        .collect::<io::Result<_>>()?;
    if names.len() >= 128 || (mutation && names.is_empty()) {
        return Err(invalid("native directory mutation depth is invalid"));
    }
    let mut original = Arc::new(Original::new(root));
    let mut ancestors = Vec::with_capacity(names.len());
    for (index, name) in names.iter().enumerate() {
        original.revalidate()?;
        let access = FILE_READ_ATTRIBUTES
            | if mutation && index + 1 == names.len() {
                FILE_WRITE_ATTRIBUTES | DELETE
            } else {
                0
            };
        let file = relative_open(&original.file, name, access, false, null_mut())?;
        ancestors.push(original);
        original = Arc::new(Original::new(file));
        original.revalidate()?;
    }
    let result = DirectoryMutation {
        original,
        ancestors,
    };
    result.revalidate_original()?;
    if mutation {
        validate_current_owner(&result.original.file)?;
    }
    Ok(result)
}

impl DirectoryMutation {
    /// Capture an existing current-user directory, without creating or repairing anything.
    ///
    /// # Errors
    /// Refuses nonlocal paths, reparse points, unsafe ACLs, foreign ownership and native errors.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let result = walk(path.as_ref(), true)?;
        result.require_at(path.as_ref())?;
        Ok(result)
    }

    /// Return the identity of this retained original, never an independently opened name.
    ///
    /// # Errors
    /// Refuses an original identity or native observation failure.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.original.identity()
    }

    fn revalidate_original(&self) -> io::Result<()> {
        for ancestor in &self.ancestors {
            ancestor.revalidate()?;
        }
        self.original.revalidate()
    }

    /// Require this same original at an exact current name, with safe retained ancestry.
    ///
    /// This also permits a caller-authenticated move to a new name: the retained original,
    /// rather than the new name, remains the identity authority.
    ///
    /// # Errors
    /// Refuses missing/substituted names, changed custody and unsafe ancestry.
    pub fn require_at(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.revalidate_original()?;
        validate_current_owner(&self.original.file)?;
        let named = walk(path.as_ref(), false)?;
        if named.identity()? != self.identity()? {
            return Err(io::Error::other("native directory original was replaced"));
        }
        let named_check = named.revalidate_original();
        let original_check = self.revalidate_original();
        named_check.and(original_check)
    }

    fn require_child(&self, child: &Self, name: &OsStr) -> io::Result<()> {
        self.revalidate_original()?;
        child.revalidate_original()?;
        let named = relative_open(
            &self.original.file,
            name,
            FILE_READ_ATTRIBUTES,
            false,
            null_mut(),
        )?;
        snapshot(&named, false, true)?;
        if identity(&named)? != child.identity()? {
            return Err(io::Error::other("native directory original was replaced"));
        }
        validate_current_owner(&child.original.file)
    }

    fn require_absent(&self, name: &OsStr) -> io::Result<()> {
        match relative_open(
            &self.original.file,
            name,
            FILE_READ_ATTRIBUTES,
            false,
            null_mut(),
        ) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "native directory destination is occupied",
            )),
        }
    }

    /// Exclusively create a private child and install its original before any postcheck.
    ///
    /// A populated slot must be retained even when this returns an error. The caller resumes
    /// validation of that original; it must never create or adopt the name a second time.
    ///
    /// # Errors
    /// Refuses occupied slots/names, unsafe parents, native creation or final validation errors.
    pub fn create_child_into(&self, path: &Path, slot: &mut Option<Self>) -> io::Result<()> {
        self.create_child_into_observed(path, slot, |_| Ok(()))
    }

    fn create_child_into_observed(
        &self,
        path: &Path,
        slot: &mut Option<Self>,
        mut observe: impl FnMut(DirectoryMutationStep) -> io::Result<()>,
    ) -> io::Result<()> {
        if slot.is_some() {
            return Err(invalid("native directory creation slot is occupied"));
        }
        let parent = path
            .parent()
            .ok_or_else(|| invalid("native directory parent required"))?;
        let name = checked_name(
            path.file_name()
                .ok_or_else(|| invalid("native directory leaf required"))?,
        )?;
        let mut ancestors = self.ancestors.clone();
        ancestors.push(Arc::clone(&self.original));
        if ancestors.len() >= 128 {
            return Err(invalid("native directory depth bound exceeded"));
        }
        let result = (|| {
            self.require_at(parent)?;
            self.require_absent(name)?;
            let file = with_owner_security_attributes(|attributes| {
                // SAFETY: with_owner_security_attributes owns this valid SECURITY_ATTRIBUTES.
                let descriptor =
                    unsafe { (*attributes.cast::<SECURITY_ATTRIBUTES>()).lpSecurityDescriptor };
                relative_open(
                    &self.original.file,
                    name,
                    FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES | DELETE,
                    true,
                    descriptor,
                )
            })??;
            *slot = Some(Self {
                original: Arc::new(Original::new(file)),
                ancestors,
            });
            observe(DirectoryMutationStep::Created)
        })();
        let child_check = slot.as_ref().map_or(Ok(()), |child| {
            child.revalidate_original().and_then(|()| {
                snapshot(&child.original.file, true, true)?;
                self.require_child(child, name)
            })
        });
        let parent_check = self.require_at(parent);
        result.and(child_check).and(parent_check)
    }

    /// Rename this original without replacement, retaining it and both parents on failure.
    ///
    /// `published` starts false and becomes true immediately after the kernel rename succeeds,
    /// before observer/final checks. A true latch must be reconciled, never renamed again.
    ///
    /// # Errors
    /// Refuses substituted names, occupied targets, cross-volume moves and native/final errors.
    pub fn rename_no_replace(
        &self,
        from: &Self,
        source: &Path,
        to: &Self,
        target: &Path,
        published: &mut bool,
    ) -> io::Result<()> {
        self.rename_no_replace_observed(from, source, to, target, published, |_| Ok(()))
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "test observer preserves the exact borrowed production operation"
    )]
    fn rename_no_replace_observed(
        &self,
        from: &Self,
        source: &Path,
        to: &Self,
        target: &Path,
        published: &mut bool,
        mut observe: impl FnMut(DirectoryMutationStep) -> io::Result<()>,
    ) -> io::Result<()> {
        if *published {
            return Err(invalid("native directory rename already published"));
        }
        let source_parent = source
            .parent()
            .ok_or_else(|| invalid("source parent required"))?;
        let target_parent = target
            .parent()
            .ok_or_else(|| invalid("target parent required"))?;
        let source_name = checked_name(
            source
                .file_name()
                .ok_or_else(|| invalid("source leaf required"))?,
        )?;
        let target_name = checked_name(
            target
                .file_name()
                .ok_or_else(|| invalid("target leaf required"))?,
        )?;
        let result = (|| {
            from.require_at(source_parent)?;
            to.require_at(target_parent)?;
            from.require_child(self, source_name)?;
            to.require_absent(target_name)?;
            rename_relative(&self.original.file, &to.original.file, target_name)?;
            *published = true;
            observe(DirectoryMutationStep::Published)
        })();
        let name_check = if *published {
            to.require_child(self, target_name)
                .and_then(|()| from.require_absent(source_name))
        } else {
            from.require_child(self, source_name)
        };
        let from_check = from.require_at(source_parent);
        let to_check = to.require_at(target_parent);
        result.and(name_check).and(from_check).and(to_check)
    }

    /// Remove this exact empty directory without closing the authoritative original.
    ///
    /// Kernel POSIX disposition enforces emptiness. A scoped parent-relative handle,
    /// authenticated against the continuously retained original, performs disposition and
    /// closes. The original remains the identity authority throughout; a substituted child
    /// is never admitted. `removed` latches before any fallible completion observation.
    ///
    /// # Errors
    /// Refuses populated/substituted directories, changed parents and native/final errors.
    pub fn remove_empty(&self, parent: &Self, path: &Path, removed: &mut bool) -> io::Result<()> {
        self.remove_empty_observed(parent, path, removed, |_| Ok(()))
    }

    fn remove_empty_observed(
        &self,
        parent: &Self,
        path: &Path,
        removed: &mut bool,
        mut observe: impl FnMut(DirectoryMutationStep) -> io::Result<()>,
    ) -> io::Result<()> {
        if *removed || self.original.removed.load(Ordering::Acquire) {
            return Err(invalid("native directory removal already latched"));
        }
        let parent_path = path
            .parent()
            .ok_or_else(|| invalid("native directory parent required"))?;
        let name = checked_name(
            path.file_name()
                .ok_or_else(|| invalid("native directory leaf required"))?,
        )?;
        let result = (|| {
            parent.require_at(parent_path)?;
            parent.require_child(self, name)?;
            let expected = self.identity()?;
            // Open one scoped link-aware handle through the retained parent. This never
            // replaces the authoritative original or selects identity through a pathname.
            let deleting = relative_open(
                &parent.original.file,
                name,
                DELETE | FILE_READ_ATTRIBUTES,
                false,
                null_mut(),
            )?;
            if identity(&deleting)? != expected || self.identity()? != expected {
                return Err(changed());
            }
            snapshot(&deleting, false, true)?;
            validate_current_owner(&deleting)?;
            self.revalidate_original()?;
            parent.require_at(parent_path)?;
            parent.require_child(self, name)?;
            let disposition = FILE_DISPOSITION_INFO_EX { Flags: 3 }; // DELETE | POSIX_SEMANTICS, never IGNORE_READONLY.
            // SAFETY: typed input and the exact DELETE-capable original; NTFS enforces empty-only.
            unsafe {
                win_ok(SetFileInformationByHandle(
                    deleting.as_raw_handle(),
                    FileDispositionInfoEx,
                    from_ref(&disposition).cast(),
                    native_size::<FILE_DISPOSITION_INFO_EX>()?,
                ))?;
            }
            *removed = true;
            self.original.removed.store(true, Ordering::Release);
            drop(deleting);
            observe(DirectoryMutationStep::Removed)
        })();
        let name_check = if *removed {
            self.require_removed()
                .and_then(|()| parent.require_absent(name))
        } else {
            parent.require_child(self, name)
        };
        let parent_check = parent.require_at(parent_path);
        result.and(name_check).and(parent_check)
    }

    /// Revalidate this retained original after its confirmed empty-only removal.
    /// Explicitly removed ancestors remain retained and are checked in their own latched
    /// state; an externally delete-pending original is never admitted by this observation.
    ///
    /// # Errors
    /// Refuses changed identity, custody/type or an original that is not delete-pending.
    pub fn require_removed(&self) -> io::Result<()> {
        if !self.original.removed.load(Ordering::Acquire) {
            return Err(changed());
        }
        self.revalidate_original()
    }

    /// Order this exact directory's metadata and eagerly recheck its name on failure too.
    ///
    /// # Errors
    /// Refuses changed original/name/ancestry or a native durability barrier error.
    pub fn sync_at(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let result = self
            .require_at(path.as_ref())
            .and_then(|()| sync_directory_metadata(&self.original.file));
        let final_check = self.require_at(path.as_ref());
        result.and(final_check)
    }
}

fn rename_relative(original: &File, parent: &File, name: &OsStr) -> io::Result<()> {
    let name = wide(checked_name(name)?)?;
    let name_bytes = (name.len() - 1) * 2;
    // FILE_RENAME_INFO has the native FILE_RENAME_INFORMATION layout. Include its
    // complete extent and the UTF-16 tail, including trailing structure padding.
    let bytes = size_of::<FILE_RENAME_INFO>()
        .checked_add(name.len() * 2)
        .ok_or_else(|| invalid("rename allocation too long"))?;
    let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
    let mut status = IoStatusBlock {
        status_or_pointer: 0,
        information: 0,
    };
    // SAFETY: aligned native rename information and IO_STATUS_BLOCK remain live through
    // the synchronous call. Source and target parent are retained originals; class 10
    // uses the false ReplaceIfExists field, so the kernel arbitrates collisions.
    let result = unsafe {
        let value = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*value).Anonymous.ReplaceIfExists = false;
        (*value).RootDirectory = parent.as_raw_handle();
        (*value).FileNameLength =
            u32::try_from(name_bytes).map_err(|_| invalid("rename name too long"))?;
        std::ptr::copy_nonoverlapping(name.as_ptr(), (*value).FileName.as_mut_ptr(), name.len());
        NtSetInformationFile(
            original.as_raw_handle(),
            &raw mut status,
            value.cast(),
            u32::try_from(bytes).map_err(|_| invalid("rename allocation too long"))?,
            10, // FileRenameInformation; the destination is relative to the retained parent.
        )
    };
    if result < 0 {
        // SAFETY: conversion accepts every NTSTATUS and has no borrowed data.
        return Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(result) }.cast_signed(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
