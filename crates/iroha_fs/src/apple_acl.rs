//! Descriptor-bound macOS extended ACL admission (mode bits do not include these grants).
#![allow(
    unsafe_code,
    reason = "macOS exposes extended ACLs only through its native ACL API"
)]

use std::{ffi::c_void, fs::File, io, os::fd::AsRawFd as _, ptr::null_mut};

unsafe extern "C" {
    fn acl_get_fd_np(fd: i32, kind: i32) -> *mut c_void;
    fn acl_valid(acl: *mut c_void) -> i32;
    fn acl_get_entry(acl: *mut c_void, entry_id: i32, entry: *mut *mut c_void) -> i32;
    fn acl_get_tag_type(entry: *mut c_void, tag: *mut i32) -> i32;
    fn acl_get_permset_mask_np(entry: *mut c_void, mask: *mut u64) -> i32;
    fn acl_free(acl: *mut c_void) -> i32;
}

struct Acl(*mut c_void);
impl Drop for Acl {
    fn drop(&mut self) {
        // SAFETY: this object owns the ACL allocation from acl_get_fd_np.
        unsafe {
            acl_free(self.0);
        }
    }
}

fn success(status: i32) -> io::Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(super) fn validate(file: &File, private: bool) -> io::Result<()> {
    // SAFETY: the descriptor remains live, and ACL_TYPE_EXTENDED is 0x100 in sys/acl.h.
    let pointer = unsafe { acl_get_fd_np(file.as_raw_fd(), 0x100) };
    if pointer.is_null() {
        let error = io::Error::last_os_error();
        // Darwin's acl_get_fd_np queries FILESEC_ACL on the retained descriptor; absent
        // FILESEC_ACL is reported as ENOENT. The file itself was already fstat-validated.
        if error.raw_os_error() == Some(2) {
            return Ok(());
        }
        return Err(error);
    }
    let acl = Acl(pointer);
    // SAFETY: acl owns a complete native ACL allocation.
    unsafe {
        success(acl_valid(acl.0))?;
    }
    let mut which = 0; // ACL_FIRST_ENTRY; subsequent iterations use ACL_NEXT_ENTRY (-1).
    loop {
        let mut entry = null_mut();
        // SAFETY: ACL and output remain valid; the entry is borrowed from acl.
        if unsafe { acl_get_entry(acl.0, which, &raw mut entry) } != 0 {
            let error = io::Error::last_os_error();
            // macOS documents EINVAL for an exhausted ACL. The ACL was validated above.
            if error.raw_os_error() == Some(22) {
                return Ok(());
            }
            return Err(error);
        }
        which = -1;
        let mut tag = 0;
        let mut permissions = 0;
        // SAFETY: both output scalars and the native entry remain valid.
        unsafe {
            success(acl_get_tag_type(entry, &raw mut tag))?;
            success(acl_get_permset_mask_np(entry, &raw mut permissions))?;
        }
        if tag == 2 {
            continue;
        } // ACL_EXTENDED_DENY cannot grant authority.
        // sys/acl.h: write/add, delete, append, delete-child, write-attributes/extended-
        // attributes/security, and change-owner. Unknown granting tags fail closed.
        let mutation = (1 << 2)
            | (1 << 4)
            | (1 << 5)
            | (1 << 6)
            | (1 << 8)
            | (1 << 10)
            | (1 << 12)
            | (1 << 13);
        if tag != 1 || (private && permissions != 0) || permissions & mutation != 0 {
            return Err(super::denied(
                "extended ACL grants access outside native custody",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{PrivateDirectory, PublishMode};
    use std::{fs, process::Command};

    #[test]
    fn macos_extended_grants_are_checked_even_when_modes_remain_private() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
        directory
            .write_atomic("secret", b"private", PublishMode::CreateNew)
            .unwrap();
        let path = directory.path().join("secret");
        assert!(
            Command::new("chmod")
                .args(["+a", "everyone allow read"])
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        assert!(directory.read("secret", 7).is_err());
        assert!(crate::read_regular(&path, 7).is_ok());
        assert!(
            Command::new("chmod")
                .args(["-N"])
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(directory.read("secret", 7).unwrap().as_slice(), b"private");
        assert!(
            Command::new("chmod")
                .args(["+a", "everyone allow add_file"])
                .arg(directory.path())
                .status()
                .unwrap()
                .success()
        );
        assert!(directory.revalidate().is_err());
        assert!(crate::read_regular(&path, 7).is_err());
        assert!(
            Command::new("chmod")
                .args(["-N"])
                .arg(directory.path())
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(fs::read(&path).unwrap(), b"private");
    }
}
