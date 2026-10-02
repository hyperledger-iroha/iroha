//! Atomic descriptor-relative macOS inventory metadata, including unreadable tombstone ACLs.
#![allow(
    unsafe_code,
    reason = "macOS exports descriptor-relative security attributes through getattrlistat"
)]

use super::{PrivateFileMetadata, apple_acl, denied, invalid};
use std::{
    ffi::{CString, OsStr, c_char, c_void},
    fs::File,
    io,
    os::{fd::AsRawFd as _, unix::ffi::OsStrExt as _},
};

#[repr(C)]
struct AttributeList {
    count: u16,
    reserved: u16,
    common: u32,
    volume: u32,
    directory: u32,
    file: u32,
    fork: u32,
}
#[repr(C, align(8))]
struct AttributeBuffer([u8; 8192]);

unsafe extern "C" {
    fn getattrlistat(
        directory: i32,
        name: *const c_char,
        attributes: *mut AttributeList,
        buffer: *mut c_void,
        length: usize,
        options: std::ffi::c_ulong,
    ) -> i32;
}

/// Read complete metadata from one no-follow child lookup anchored to the retained parent.
pub(super) fn private_metadata(directory: &File, name: &OsStr) -> io::Result<PrivateFileMetadata> {
    // Unlike O_EVTONLY, this metadata syscall does not demand permission to read file data.
    // sys/{attr,unistd,kauth}.h and getattrlist(2) define the layout and supported attributes.
    // Deliberately omit RETURNED_ATTRS/PACK_INVAL_ATTRS and require the complete fixed layout
    // below. Native errors and omitted/unsupported fields fail closed; no default field or
    // missing returned-mask bit is accepted as ACL authority.
    let mut attributes = AttributeList {
        count: 5,
        reserved: 0,
        // OBJTYPE | OWNERID | ACCESSMASK | EXTENDED_SECURITY.
        common: 0x0000_0008 | 0x0000_8000 | 0x0002_0000 | 0x0040_0000,
        volume: 0,
        directory: 0,
        // LINKCOUNT | DATALENGTH (the data fork, matching fstat length).
        file: 0x0000_0001 | 0x0000_0200,
        fork: 0,
    };
    super::checked_name(name)?;
    let name = CString::new(name.as_bytes()).map_err(|_| invalid("invalid inventory basename"))?;
    let mut buffer = AttributeBuffer([0; 8192]);
    // SAFETY: parent descriptor and single checked basename remain live; the initialized list
    // and aligned bounded buffer have their exact C layouts. FSOPT_NOFOLLOW (1) does not follow the final
    // symlink, and REPORT_FULLSIZE (4) makes truncation visible in the returned length.
    let result = unsafe {
        getattrlistat(
            directory.as_raw_fd(),
            name.as_ptr(),
            &raw mut attributes,
            buffer.0.as_mut_ptr().cast(),
            buffer.0.len(),
            1 | 4,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    decode_private_metadata(&buffer.0, rustix::process::geteuid().as_raw())
}

fn word(bytes: &[u8], offset: usize) -> io::Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| invalid("incomplete native metadata"))?;
    Ok(u32::from_ne_bytes(
        value
            .try_into()
            .map_err(|_| invalid("invalid native metadata word"))?,
    ))
}

fn decode_private_metadata(buffer: &[u8], owner: u32) -> io::Result<PrivateFileMetadata> {
    // Attributes are returned in fixed bit order with four-byte alignment. The sole variable
    // attribute's reference starts at 16 and its complete security payload starts at 36.
    const FIXED: usize = 36;
    let length = word(buffer, 0)? as usize;
    let bytes = buffer
        .get(..length)
        .filter(|bytes| bytes.len() >= FIXED)
        .ok_or_else(|| invalid("truncated native inventory metadata"))?;
    let mode = word(bytes, 12)?;
    if word(bytes, 4)? != 1
        || mode & 0o170_000 != 0o100_000
        || mode & !0o177_777 != 0
        || word(bytes, 8)? != owner
        || word(bytes, 24)? != 1
        || mode & 0o7777 & !0o600 != 0
    {
        return Err(denied(
            "inventory requires a private single-link current-owner file",
        ));
    }
    let offset = word(bytes, 16)?.cast_signed();
    let security_length = word(bytes, 20)? as usize;
    if offset != 20 || FIXED.checked_add(security_length) != Some(length) {
        return Err(invalid("invalid native security attribute extent"));
    }
    if security_length != 0 {
        apple_acl::validate_native_private(&bytes[FIXED..])?;
    }
    let length = i64::from_ne_bytes(
        bytes[28..36]
            .try_into()
            .map_err(|_| invalid("incomplete native data-fork length"))?,
    );
    let length = u64::try_from(length).map_err(|_| invalid("negative native data-fork length"))?;
    Ok(PrivateFileMetadata {
        length,
        read_only: mode & 0o7777 == 0o400,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response() -> Vec<u8> {
        let mut bytes = vec![0; 36];
        for (offset, value) in [
            (0, 36),
            (4, 1),
            (8, 501),
            (12, 0o100_000),
            (16, 20),
            (24, 1),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&u32::to_ne_bytes(value));
        }
        bytes
    }

    #[test]
    fn native_inventory_decoder_accepts_complete_zero_mode_and_exact_readonly_metadata() {
        let mut bytes = response();
        let zero = decode_private_metadata(&bytes, 501).unwrap();
        assert_eq!(zero.len(), 0);
        assert!(!zero.is_read_only());
        bytes[12..16].copy_from_slice(&0o100_400_u32.to_ne_bytes());
        bytes[28..36].copy_from_slice(&91_i64.to_ne_bytes());
        let sealed = decode_private_metadata(&bytes, 501).unwrap();
        assert_eq!(sealed.len(), 91);
        assert!(sealed.is_read_only());
    }

    #[test]
    fn native_inventory_decoder_rejects_truncation_unsupported_objects_and_malformed_security() {
        for length in 0..36 {
            assert!(decode_private_metadata(&response()[..length], 501).is_err());
        }
        for (offset, value) in [
            (0, 8193_u32),
            (4, 5),
            (8, 502),
            (12, 0o100_644),
            (24, 2),
            (16, 0),
            (16, u32::MAX),
            (20, 1),
            (20, u32::MAX),
        ] {
            let mut bytes = response();
            bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
            assert!(
                decode_private_metadata(&bytes, 501).is_err(),
                "offset {offset}, value {value}"
            );
        }
        let mut negative = response();
        negative[28..36].copy_from_slice(&(-1_i64).to_ne_bytes());
        assert!(decode_private_metadata(&negative, 501).is_err());
        for security_length in [1, 43, 44, 68] {
            let mut bytes = response();
            bytes.resize(36 + security_length, 0);
            let reported = u32::try_from(bytes.len()).unwrap();
            bytes[..4].copy_from_slice(&reported.to_ne_bytes());
            bytes[20..24].copy_from_slice(&u32::try_from(security_length).unwrap().to_ne_bytes());
            assert!(
                decode_private_metadata(&bytes, 501).is_err(),
                "malformed ACL length {security_length}"
            );
        }
    }
}
