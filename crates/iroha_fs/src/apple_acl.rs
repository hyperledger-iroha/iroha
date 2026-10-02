//! Descriptor-bound macOS extended ACL admission (mode bits do not include these grants).
#![allow(
    unsafe_code,
    reason = "macOS exposes extended ACLs only through its native ACL API"
)]

use std::{ffi::c_void, fs::File, io, os::fd::AsRawFd as _, ptr::null_mut};

unsafe extern "C" {
    fn acl_get_fd_np(fd: i32, kind: i32) -> *mut c_void;
    fn acl_valid(acl: *mut c_void) -> i32;
    fn acl_copy_int_native(bytes: *const c_void) -> *mut c_void;
    fn acl_get_entry(acl: *mut c_void, entry_id: i32, entry: *mut *mut c_void) -> i32;
    fn acl_get_tag_type(entry: *mut c_void, tag: *mut i32) -> i32;
    fn acl_get_permset_mask_np(entry: *mut c_void, mask: *mut u64) -> i32;
    fn acl_free(acl: *mut c_void) -> i32;
}

struct Acl(*mut c_void);
impl Drop for Acl {
    fn drop(&mut self) {
        // SAFETY: this object owns the ACL allocation from acl_get_fd_np or acl_copy_int_native.
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
    validate_acl(&Acl(pointer), private)
}

fn validate_acl(acl: &Acl, private: bool) -> io::Result<()> {
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

/// Import only a complete bounded Darwin `kauth_filesec` returned by the same metadata request.
pub(super) fn validate_native_private(bytes: &[u8]) -> io::Result<()> {
    // sys/kauth.h: magic, two GUIDs, entry count, flags, then 24-byte kauth_ace entries.
    const PREFIX: usize = 44;
    const MAX_ENTRIES: usize = 128;
    #[repr(C, align(8))]
    struct NativeFilesec([u8; PREFIX + 24 * MAX_ENTRIES]);
    if bytes.len() < PREFIX
        || u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) != 0x012c_c16d
    {
        return Err(super::invalid("incomplete native ACL security header"));
    }
    let count = u32::from_ne_bytes([bytes[36], bytes[37], bytes[38], bytes[39]]);
    if count == u32::MAX {
        return if bytes.len() == PREFIX {
            Ok(())
        } else {
            Err(super::invalid("invalid native absent-ACL extent"))
        };
    }
    let count = count as usize;
    if count > MAX_ENTRIES || bytes.len() != PREFIX + 24 * count {
        return Err(super::invalid("invalid native ACL entry extent"));
    }
    let mut aligned = NativeFilesec([0; PREFIX + 24 * MAX_ENTRIES]);
    aligned.0[..bytes.len()].copy_from_slice(bytes);
    // SAFETY: checked the native magic/count and complete finite extent before passing the
    // native parser an aligned pointer. Its returned allocation is owned and validated below.
    let pointer = unsafe { acl_copy_int_native(aligned.0.as_ptr().cast()) };
    if pointer.is_null() {
        return Err(io::Error::last_os_error());
    }
    validate_acl(&Acl(pointer), true)
}

#[cfg(test)]
mod tests {
    use super::validate_native_private;
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
    #[test]
    fn native_security_import_rejects_incomplete_headers_and_entry_extents() {
        for length in [0, 1, 43, 44, 68] {
            assert!(validate_native_private(&vec![0; length]).is_err());
        }
        let mut absent = [0; 44];
        absent[..4].copy_from_slice(&0x012c_c16d_u32.to_ne_bytes());
        absent[36..40].copy_from_slice(&u32::MAX.to_ne_bytes());
        validate_native_private(&absent).unwrap();
        let mut trailing = absent.to_vec();
        trailing.push(0);
        assert!(validate_native_private(&trailing).is_err());
        for count in [1_u32, 128, 129, u32::MAX - 1] {
            let mut incomplete = absent;
            incomplete[36..40].copy_from_slice(&count.to_ne_bytes());
            assert!(validate_native_private(&incomplete).is_err());
        }
    }
}
