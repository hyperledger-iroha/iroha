//! OS-selected current-user local data with native retained custody.

use super::*;
use std::{ffi::OsString, os::windows::ffi::OsStringExt as _};
use windows_sys::Win32::{
    Security::{TOKEN_DUPLICATE, TOKEN_IMPERSONATE},
    Storage::FileSystem::GetDriveTypeW,
    System::{Com::CoTaskMemFree, WindowsProgramming::DRIVE_FIXED},
    UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath},
};

struct FolderAllocation(*mut u16);
impl Drop for FolderAllocation {
    fn drop(&mut self) {
        // SAFETY: SHGetKnownFolderPath owns this COM-task allocation, including error paths.
        unsafe { CoTaskMemFree(self.0.cast()) };
    }
}

/// Retain the effective Windows user's OS-selected local application-data directory.
///
/// The actual effective token selects the known folder; environment variables do not
/// supply its identity. The result requires a fixed local NTFS drive, current-user leaf
/// ownership and safe original ancestors. OS-configured local folder redirection is
/// admitted under the same checks; network drives, UNC paths and reparse points are
/// refused. Existing permissions are never repaired.
///
/// # Errors
/// Refuses token/profile lookup failures, unsafe or non-local custody and native errors.
pub fn current_user_local_data_directory() -> io::Result<OwnerDirectory> {
    let token = current_user_token(TOKEN_QUERY | TOKEN_IMPERSONATE | TOKEN_DUPLICATE)?;
    let user = UserSid::from_token(&token)?;
    let mut raw = null_mut();
    // SAFETY: exact effective token remains live; the valid output slot receives an OS allocation.
    let status = unsafe {
        SHGetKnownFolderPath(
            &FOLDERID_LocalAppData,
            0,
            token.as_raw_handle(),
            &raw mut raw,
        )
    };
    let allocation = FolderAllocation(raw);
    if status < 0 {
        return Err(io::Error::other(format!(
            "current-user local-data lookup failed: HRESULT 0x{:08x}",
            status.cast_unsigned()
        )));
    }
    if allocation.0.is_null() {
        return Err(invalid("known folder returned an empty native path"));
    }
    let mut length = 0usize;
    // SAFETY: a successful known-folder query returns an allocated null-terminated UTF-16 path.
    while unsafe { *allocation.0.add(length) } != 0 {
        length += 1;
        if length >= 32_768 {
            return Err(invalid("known folder exceeds the native path bound"));
        }
    }
    // SAFETY: the preceding scan found the terminator inside this live OS allocation.
    let path = PathBuf::from(OsString::from_wide(unsafe {
        std::slice::from_raw_parts(allocation.0, length)
    }));
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Err(invalid("known folder is not an absolute local drive path"));
    };
    if !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) {
        return Err(invalid("known folder requires a fixed local drive"));
    }
    let mut drive = prefix.as_os_str().to_owned();
    drive.push("\\");
    let drive = wide(&drive)?;
    // SAFETY: drive is a live null-terminated native root path; no output buffers are involved.
    if unsafe { GetDriveTypeW(drive.as_ptr()) } != DRIVE_FIXED {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "known folder is not on a fixed local drive",
        ));
    }
    let inner = Directory::open_owned(&path, false)?;
    inner.require_current_owner()?;
    if !user.equals(UserSid::current()?.pointer()) {
        return Err(changed());
    }
    Ok(OwnerDirectory { inner })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_local_data_retains_current_owner_and_full_identity() {
        let first = current_user_local_data_directory().unwrap();
        let second = current_user_local_data_directory().unwrap();
        assert!(first.path().is_absolute());
        assert_eq!(first.path(), second.path());
        assert_eq!(first.identity().unwrap(), second.identity().unwrap());
        first.inner.require_current_owner().unwrap();
        drop(second);
        first.revalidate().unwrap();
    }

    #[test]
    fn effective_thread_token_selects_the_same_users_original_folder() {
        use windows_sys::Win32::Security::{ImpersonateSelf, RevertToSelf, SecurityImpersonation};
        struct Revert;
        impl Drop for Revert {
            fn drop(&mut self) {
                // SAFETY: balances this thread's successful ImpersonateSelf call on every exit.
                assert_ne!(unsafe { RevertToSelf() }, 0);
            }
        }
        let expected = current_user_local_data_directory().unwrap();
        // SAFETY: impersonate only this process's own identity on this test thread.
        assert_ne!(unsafe { ImpersonateSelf(SecurityImpersonation) }, 0);
        let observed = {
            let _revert = Revert;
            current_user_local_data_directory()
        }
        .unwrap();
        assert_eq!(expected.identity().unwrap(), observed.identity().unwrap());
        assert_eq!(expected.path(), observed.path());
    }
}
