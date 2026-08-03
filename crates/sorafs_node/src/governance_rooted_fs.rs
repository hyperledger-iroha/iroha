//! Handle-rooted filesystem operations for durable Governance DAG state.
//!
//! Production mutations are resolved component-by-component below a retained
//! directory handle. Linux and macOS use the `*at` family. Windows uses
//! `NtCreateFile` for root-directory-relative opens and
//! `SetFileInformationByHandle` for rename/disposition. Other targets fail
//! closed because they are not V1 native release targets.

use std::{
    ffi::{OsStr, OsString},
    fmt,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use norito::derive::{NoritoDeserialize, NoritoSerialize};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt as _;

#[cfg(unix)]
unsafe extern "C" {
    fn geteuid() -> std::os::raw::c_uint;
}

const DEFAULT_CHILD_ENTRY_LIMIT: usize = 1_000_000;
const TWO_SLOT_FORMAT_VERSION_V1: u8 = 1;
const TWO_SLOT_HEADER_RESERVED_BYTES_V1: usize = 128;
const TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1: usize = 64;
const TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1: usize = 64;
pub(super) const TWO_SLOT_MAX_PAYLOAD_BYTES_V1: usize = 196 * 1024 * 1024;
const TWO_SLOT_STORE_NAME_MAX_BYTES_V1: usize = 128;
const TWO_SLOT_STAGE_ENTRY_HARD_CAP_V1: usize = 16;
const TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1: usize = 16;
const TWO_SLOT_LOST_FOUND_TOTAL_MAX_BYTES_V1: u64 = 1024 * 1024 * 1024;
const TWO_SLOT_NAMES_V1: [&str; 2] = ["slot-0.v1", "slot-1.v1"];
const TWO_SLOT_COMMIT_MARKER_V1: [u8; 16] = *b"iroha-slot-v1-ok";
const TWO_SLOT_ZERO_DIGEST: [u8; 32] = [0; 32];
static TWO_SLOT_STAGE_COUNTER: AtomicU64 = AtomicU64::new(0);
const ATOMIC_RETAINED_SUFFIX_V1: &str = ".retained-v1-";
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
/// Number of canonical V1 predecessor slots reserved per atomic target.
pub(super) const ATOMIC_RETAINED_SLOT_COUNT_V1: usize = 1_024;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
const ATOMIC_RETAINED_SLOT_WIDTH_V1: usize = 4;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
/// Aggregate retained-predecessor byte ceiling for one directory.
pub(super) const ATOMIC_RETAINED_TOTAL_MAX_BYTES_V1: u64 = 1024 * 1024 * 1024;

#[cfg(any(windows, test))]
mod windows_dacl {
    use std::{io, mem::size_of};

    const ACL_HEADER_BYTES: usize = 8;
    const ACE_HEADER_BYTES: usize = 4;
    const BASIC_ACE_SID_OFFSET: usize = 8;
    const OBJECT_ACE_BASE_SID_OFFSET: usize = 12;
    const OBJECT_TYPE_PRESENT: u32 = 0x1;
    const INHERITED_OBJECT_TYPE_PRESENT: u32 = 0x2;
    const ACL_REVISION: u8 = 2;
    const ACL_REVISION_DS: u8 = 4;
    const ACCESS_ALLOWED_ACE_TYPE: u8 = 0x00;
    const ACCESS_DENIED_ACE_TYPE: u8 = 0x01;
    const ACCESS_ALLOWED_OBJECT_ACE_TYPE: u8 = 0x05;
    const ACCESS_DENIED_OBJECT_ACE_TYPE: u8 = 0x06;
    const ACCESS_ALLOWED_CALLBACK_ACE_TYPE: u8 = 0x09;
    const ACCESS_DENIED_CALLBACK_ACE_TYPE: u8 = 0x0a;
    const ACCESS_ALLOWED_CALLBACK_OBJECT_ACE_TYPE: u8 = 0x0b;
    const ACCESS_DENIED_CALLBACK_OBJECT_ACE_TYPE: u8 = 0x0c;
    const VALID_INHERITANCE_FLAGS: u8 = 0x1f;
    const MAX_ACES: usize = 4_096;
    const SID_REVISION: u8 = 1;
    const SID_MAX_SUB_AUTHORITIES: usize = 15;
    const FILE_WRITE_DATA: u32 = 0x0000_0002;
    const FILE_APPEND_DATA: u32 = 0x0000_0004;
    const FILE_WRITE_EA: u32 = 0x0000_0010;
    const FILE_DELETE_CHILD: u32 = 0x0000_0040;
    const FILE_WRITE_ATTRIBUTES: u32 = 0x0000_0100;
    const DELETE: u32 = 0x0001_0000;
    const WRITE_DAC: u32 = 0x0004_0000;
    const WRITE_OWNER: u32 = 0x0008_0000;
    const ACCESS_SYSTEM_SECURITY: u32 = 0x0100_0000;
    const MAXIMUM_ALLOWED: u32 = 0x0200_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const GENERIC_ALL: u32 = 0x1000_0000;
    const MUTATION_ACCESS: u32 = FILE_WRITE_DATA
        | FILE_APPEND_DATA
        | FILE_WRITE_EA
        | FILE_DELETE_CHILD
        | FILE_WRITE_ATTRIBUTES
        | DELETE
        | WRITE_DAC
        | WRITE_OWNER
        | ACCESS_SYSTEM_SECURITY
        | MAXIMUM_ALLOWED
        | GENERIC_WRITE
        | GENERIC_ALL;

    const LOCAL_SYSTEM_SID: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0];
    const BUILTIN_ADMINISTRATORS_SID: &[u8] = &[1, 2, 0, 0, 0, 0, 0, 5, 32, 0, 0, 0, 32, 2, 0, 0];
    const CREATOR_OWNER_SID: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0];
    const OWNER_RIGHTS_SID: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 3, 4, 0, 0, 0];

    pub(super) fn sid_encoded_length(bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() < 8 {
            return Err(invalid_data("Windows SID header is truncated"));
        }
        if bytes[0] != SID_REVISION {
            return Err(invalid_data("Windows SID revision is not canonical"));
        }
        let sub_authorities = usize::from(bytes[1]);
        if sub_authorities > SID_MAX_SUB_AUTHORITIES {
            return Err(invalid_data("Windows SID exceeds its sub-authority bound"));
        }
        let length = 8usize
            .checked_add(
                sub_authorities
                    .checked_mul(size_of::<u32>())
                    .ok_or_else(|| invalid_data("Windows SID length overflow"))?,
            )
            .ok_or_else(|| invalid_data("Windows SID length overflow"))?;
        if length > bytes.len() {
            return Err(invalid_data("Windows SID is truncated"));
        }
        Ok(length)
    }

    pub(super) fn dacl_encoded_length(bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() < ACL_HEADER_BYTES {
            return Err(invalid_data("Windows DACL header is truncated"));
        }
        let length = usize::from(read_u16(bytes, 2)?);
        if length < ACL_HEADER_BYTES || length > bytes.len() || length % size_of::<u32>() != 0 {
            return Err(invalid_data(
                "Windows DACL length is truncated or noncanonical",
            ));
        }
        Ok(length)
    }

    pub(super) fn validate(owner_sid: Option<&[u8]>, dacl: Option<&[u8]>) -> io::Result<()> {
        let owner_sid =
            owner_sid.ok_or_else(|| invalid_data("Windows governance owner SID is null"))?;
        let dacl = dacl.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Windows governance DACL is null and grants unrestricted access",
            )
        })?;
        if sid_encoded_length(owner_sid)? != owner_sid.len() {
            return Err(invalid_data(
                "Windows governance owner SID has trailing bytes",
            ));
        }
        let acl_length = dacl_encoded_length(dacl)?;
        if acl_length != dacl.len() {
            return Err(invalid_data("Windows governance DACL has trailing bytes"));
        }
        if !matches!(dacl[0], ACL_REVISION | ACL_REVISION_DS) || dacl[1] != 0 {
            return Err(invalid_data(
                "Windows governance DACL revision or reserved byte is noncanonical",
            ));
        }
        if read_u16(dacl, 6)? != 0 {
            return Err(invalid_data(
                "Windows governance DACL reserved field is nonzero",
            ));
        }
        let ace_count = usize::from(read_u16(dacl, 4)?);
        if ace_count > MAX_ACES {
            return Err(invalid_data(
                "Windows governance DACL exceeds its ACE bound",
            ));
        }

        let mut offset = ACL_HEADER_BYTES;
        for _ in 0..ace_count {
            let header_end = offset
                .checked_add(ACE_HEADER_BYTES)
                .ok_or_else(|| invalid_data("Windows ACE header offset overflow"))?;
            if header_end > dacl.len() {
                return Err(invalid_data("Windows ACE header is truncated"));
            }
            let ace_type = dacl[offset];
            let ace_flags = dacl[offset + 1];
            if ace_flags & !VALID_INHERITANCE_FLAGS != 0 {
                return Err(invalid_data("Windows ACE flags are unsupported"));
            }
            let ace_length = usize::from(read_u16(dacl, offset + 2)?);
            if ace_length < BASIC_ACE_SID_OFFSET || ace_length % size_of::<u32>() != 0 {
                return Err(invalid_data("Windows ACE length is noncanonical"));
            }
            let ace_end = offset
                .checked_add(ace_length)
                .ok_or_else(|| invalid_data("Windows ACE length overflow"))?;
            if ace_end > dacl.len() {
                return Err(invalid_data("Windows ACE is truncated"));
            }
            let ace = &dacl[offset..ace_end];
            let (allowed, callback, sid_offset) = match ace_type {
                ACCESS_ALLOWED_ACE_TYPE => (true, false, BASIC_ACE_SID_OFFSET),
                ACCESS_DENIED_ACE_TYPE => (false, false, BASIC_ACE_SID_OFFSET),
                ACCESS_ALLOWED_CALLBACK_ACE_TYPE => (true, true, BASIC_ACE_SID_OFFSET),
                ACCESS_DENIED_CALLBACK_ACE_TYPE => (false, true, BASIC_ACE_SID_OFFSET),
                ACCESS_ALLOWED_OBJECT_ACE_TYPE => (true, false, object_sid_offset(ace)?),
                ACCESS_DENIED_OBJECT_ACE_TYPE => (false, false, object_sid_offset(ace)?),
                ACCESS_ALLOWED_CALLBACK_OBJECT_ACE_TYPE => (true, true, object_sid_offset(ace)?),
                ACCESS_DENIED_CALLBACK_OBJECT_ACE_TYPE => (false, true, object_sid_offset(ace)?),
                _ => {
                    return Err(invalid_data(
                        "Windows governance DACL contains an unsupported ACE type",
                    ));
                }
            };
            if sid_offset > ace.len() {
                return Err(invalid_data("Windows ACE SID offset is truncated"));
            }
            let sid_length = sid_encoded_length(&ace[sid_offset..])?;
            let sid_end = sid_offset
                .checked_add(sid_length)
                .ok_or_else(|| invalid_data("Windows ACE SID length overflow"))?;
            if sid_end > ace.len() || (!callback && sid_end != ace.len()) {
                return Err(invalid_data(
                    "Windows ACE SID or callback payload is noncanonical",
                ));
            }
            let mask = read_u32(ace, 4)?;
            let sid = &ace[sid_offset..sid_end];
            if allowed && mask & MUTATION_ACCESS != 0 && !is_administrative_sid(sid, owner_sid) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Windows governance DACL grants mutation authority to a non-administrative principal",
                ));
            }
            offset = ace_end;
        }
        if offset != dacl.len() {
            return Err(invalid_data(
                "Windows DACL contains undeclared trailing ACE or padding bytes",
            ));
        }
        Ok(())
    }

    fn object_sid_offset(ace: &[u8]) -> io::Result<usize> {
        if ace.len() < OBJECT_ACE_BASE_SID_OFFSET {
            return Err(invalid_data("Windows object ACE header is truncated"));
        }
        let object_flags = read_u32(ace, 8)?;
        if object_flags & !(OBJECT_TYPE_PRESENT | INHERITED_OBJECT_TYPE_PRESENT) != 0 {
            return Err(invalid_data("Windows object ACE flags are unsupported"));
        }
        let guid_count = usize::from(object_flags & OBJECT_TYPE_PRESENT != 0)
            + usize::from(object_flags & INHERITED_OBJECT_TYPE_PRESENT != 0);
        OBJECT_ACE_BASE_SID_OFFSET
            .checked_add(
                guid_count
                    .checked_mul(16)
                    .ok_or_else(|| invalid_data("Windows object ACE GUID length overflow"))?,
            )
            .ok_or_else(|| invalid_data("Windows object ACE SID offset overflow"))
    }

    fn is_administrative_sid(sid: &[u8], owner_sid: &[u8]) -> bool {
        sid == owner_sid
            || sid == LOCAL_SYSTEM_SID
            || sid == BUILTIN_ADMINISTRATORS_SID
            || sid == CREATOR_OWNER_SID
            || sid == OWNER_RIGHTS_SID
    }

    fn read_u16(bytes: &[u8], offset: usize) -> io::Result<u16> {
        let end = offset
            .checked_add(size_of::<u16>())
            .ok_or_else(|| invalid_data("Windows ACL integer offset overflow"))?;
        let raw = bytes
            .get(offset..end)
            .ok_or_else(|| invalid_data("Windows ACL integer is truncated"))?;
        Ok(u16::from_le_bytes([raw[0], raw[1]]))
    }

    fn read_u32(bytes: &[u8], offset: usize) -> io::Result<u32> {
        let end = offset
            .checked_add(size_of::<u32>())
            .ok_or_else(|| invalid_data("Windows ACL integer offset overflow"))?;
        let raw = bytes
            .get(offset..end)
            .ok_or_else(|| invalid_data("Windows ACL integer is truncated"))?;
        Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    fn invalid_data(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, message)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const OWNER_SID: &[u8] = &[1, 2, 0, 0, 0, 0, 0, 5, 21, 0, 0, 0, 7, 0, 0, 0];
        const EVERYONE_SID: &[u8] = &[1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0];

        fn basic_ace(ace_type: u8, mask: u32, sid: &[u8]) -> Vec<u8> {
            let length = BASIC_ACE_SID_OFFSET + sid.len();
            let mut ace = Vec::with_capacity(length);
            ace.extend_from_slice(&[
                ace_type,
                0,
                u8::try_from(length & 0xff).expect("ACE low length fits"),
                u8::try_from(length >> 8).expect("ACE high length fits"),
            ]);
            ace.extend_from_slice(&mask.to_le_bytes());
            ace.extend_from_slice(sid);
            ace
        }

        fn acl(aces: &[Vec<u8>]) -> Vec<u8> {
            let length = ACL_HEADER_BYTES + aces.iter().map(Vec::len).sum::<usize>();
            let mut acl = vec![ACL_REVISION, 0];
            acl.extend_from_slice(
                &u16::try_from(length)
                    .expect("test ACL length fits")
                    .to_le_bytes(),
            );
            acl.extend_from_slice(
                &u16::try_from(aces.len())
                    .expect("test ACE count fits")
                    .to_le_bytes(),
            );
            acl.extend_from_slice(&0_u16.to_le_bytes());
            for ace in aces {
                acl.extend_from_slice(ace);
            }
            acl
        }

        #[test]
        fn source_contract_accepts_owner_system_and_read_only_grants() {
            let dacl = acl(&[
                basic_ace(ACCESS_ALLOWED_ACE_TYPE, GENERIC_ALL, OWNER_SID),
                basic_ace(
                    ACCESS_ALLOWED_ACE_TYPE,
                    WRITE_DAC | WRITE_OWNER,
                    LOCAL_SYSTEM_SID,
                ),
                basic_ace(ACCESS_ALLOWED_ACE_TYPE, 0x0012_0089, EVERYONE_SID),
            ]);
            validate(Some(OWNER_SID), Some(&dacl))
                .expect("owner, system, and read-only grants are canonical");
        }

        #[test]
        fn source_contract_rejects_untrusted_mutation_null_and_truncation() {
            let untrusted = acl(&[basic_ace(
                ACCESS_ALLOWED_ACE_TYPE,
                FILE_WRITE_DATA,
                EVERYONE_SID,
            )]);
            assert_eq!(
                validate(Some(OWNER_SID), Some(&untrusted))
                    .expect_err("Everyone write grant must fail")
                    .kind(),
                io::ErrorKind::PermissionDenied
            );
            assert_eq!(
                validate(None, Some(&untrusted))
                    .expect_err("null owner must fail")
                    .kind(),
                io::ErrorKind::InvalidData
            );
            assert_eq!(
                validate(Some(OWNER_SID), None)
                    .expect_err("null DACL must fail")
                    .kind(),
                io::ErrorKind::PermissionDenied
            );
            let mut truncated = acl(&[basic_ace(
                ACCESS_ALLOWED_ACE_TYPE,
                FILE_WRITE_DATA,
                OWNER_SID,
            )]);
            truncated.pop();
            assert_eq!(
                validate(Some(OWNER_SID), Some(&truncated))
                    .expect_err("truncated DACL must fail")
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }

        #[test]
        fn source_contract_rejects_unknown_ace_type() {
            let dacl = acl(&[basic_ace(0xff, 0, EVERYONE_SID)]);
            assert_eq!(
                validate(Some(OWNER_SID), Some(&dacl))
                    .expect_err("unknown ACE type must fail")
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }

        #[test]
        fn source_contract_rejects_undeclared_aligned_acl_bytes() {
            let mut dacl = acl(&[basic_ace(
                ACCESS_ALLOWED_ACE_TYPE,
                0x0012_0089,
                EVERYONE_SID,
            )]);
            dacl.extend_from_slice(&[0; 4]);
            let declared_length = u16::try_from(dacl.len())
                .expect("test DACL length fits")
                .to_le_bytes();
            dacl[2..4].copy_from_slice(&declared_length);
            assert_eq!(
                validate(Some(OWNER_SID), Some(&dacl))
                    .expect_err("undeclared aligned DACL bytes must fail")
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
    }
}

/// Reject descriptor-bound extended ACLs that can grant mutation authority.
///
/// Linux POSIX/NFSv4-style ACL attributes and macOS extended ACL entries are
/// qualified. Other targets return `Unsupported` so callers fail closed.
pub(super) fn validate_retained_directory_acl(handle: &File, path: &Path) -> io::Result<()> {
    platform::validate_directory_acl(handle, path)
}

#[cfg(any(target_os = "linux", test))]
fn stable_linux_acl_attribute_names<F>(path: &Path, mut read: F) -> io::Result<Vec<u8>>
where
    F: FnMut() -> io::Result<Option<Vec<u8>>>,
{
    const MAX_RETRIES: usize = 3;

    for _ in 0..MAX_RETRIES {
        let Some(first) = read()? else {
            continue;
        };
        let Some(second) = read()? else {
            continue;
        };
        if first == second {
            return Ok(second);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        format!(
            "descriptor-bound ACL attributes for `{}` changed during inspection",
            path.display()
        ),
    ))
}

/// Stable identity of one opened filesystem object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial_number: u32,
    #[cfg(windows)]
    file_index: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotFileIdentityV1 {
    platform: u8,
    first: u64,
    second: u64,
}

impl From<FileIdentity> for TwoSlotFileIdentityV1 {
    fn from(identity: FileIdentity) -> Self {
        #[cfg(unix)]
        {
            Self {
                platform: 1,
                first: identity.device,
                second: identity.inode,
            }
        }
        #[cfg(windows)]
        {
            Self {
                platform: 2,
                first: u64::from(identity.volume_serial_number),
                second: identity.file_index,
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = identity;
            Self {
                platform: 0,
                first: 0,
                second: 0,
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotBindingMaterialV1 {
    format_version: u8,
    store_name_digest: [u8; 32],
    domain: [u8; 32],
    store_nonce: [u8; 32],
    max_payload_bytes: u64,
    header_region_bytes: u64,
    record_header_region_bytes: u64,
    commit_trailer_region_bytes: u64,
    init_lock_identity: TwoSlotFileIdentityV1,
    slot_identities: [TwoSlotFileIdentityV1; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotHeaderV1 {
    binding: TwoSlotBindingMaterialV1,
    binding_digest: [u8; 32],
    slot_id: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotHeaderRegionV1 {
    header: TwoSlotHeaderV1,
    reserved: [u8; TWO_SLOT_HEADER_RESERVED_BYTES_V1],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotRecordHeaderV1 {
    format_version: u8,
    binding_digest: [u8; 32],
    slot_id: u8,
    generation: u64,
    predecessor_digest: [u8; 32],
    payload_len: u64,
    payload_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotRecordHeaderRegionV1 {
    header: TwoSlotRecordHeaderV1,
    reserved: [u8; TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotCommitTrailerV1 {
    format_version: u8,
    binding_digest: [u8; 32],
    slot_id: u8,
    generation: u64,
    record_digest: [u8; 32],
    commit_marker: [u8; 16],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotCommitTrailerRegionV1 {
    trailer: TwoSlotCommitTrailerV1,
    reserved: [u8; TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1],
}

#[derive(Debug, Clone, Copy)]
struct TwoSlotLayoutV1 {
    header_region_bytes: usize,
    record_header_region_bytes: usize,
    payload_offset: u64,
    trailer_offset: u64,
    commit_trailer_region_bytes: usize,
    slot_file_bytes: u64,
}

/// Immutable identity and byte bounds for one local two-slot V1 store.
///
/// `store_nonce` is a caller-owned stable identifier. Callers must persist or
/// derive the same non-zero value on every restart; this layer never invents a
/// replacement nonce while opening an existing store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TwoSlotStoreConfigV1 {
    store_name: String,
    domain: [u8; 32],
    store_nonce: [u8; 32],
    max_payload_bytes: usize,
}

impl TwoSlotStoreConfigV1 {
    /// Validate and construct one stable two-slot store identity.
    pub(super) fn try_new(
        store_name: impl Into<OsString>,
        domain: [u8; 32],
        store_nonce: [u8; 32],
        max_payload_bytes: usize,
    ) -> io::Result<Self> {
        let store_name = store_name.into();
        validate_component(&store_name)?;
        let store_name = store_name.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot store name must be canonical UTF-8",
            )
        })?;
        let name_bytes = store_name.as_bytes().len();
        if name_bytes == 0
            || name_bytes > TWO_SLOT_STORE_NAME_MAX_BYTES_V1
            || store_name.starts_with('.')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot store name is hidden or exceeds its V1 byte bound",
            ));
        }
        if domain == TWO_SLOT_ZERO_DIGEST || store_nonce == TWO_SLOT_ZERO_DIGEST {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot domain and stable store nonce must be non-zero",
            ));
        }
        if max_payload_bytes == 0 || max_payload_bytes > TWO_SLOT_MAX_PAYLOAD_BYTES_V1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot payload bound is outside the V1 limit",
            ));
        }
        Ok(Self {
            store_name: store_name.to_owned(),
            domain,
            store_nonce,
            max_payload_bytes,
        })
    }
}

#[derive(Debug, Clone)]
struct TwoSlotFileV1 {
    handle: Arc<File>,
    identity: FileIdentity,
    name: OsString,
}

/// One exact selected record returned by a two-slot V1 store.
///
/// The snapshot binds the complete store identity as well as its selected
/// generation and record digest, so it can be used as a compare-and-swap
/// predecessor without accepting a snapshot from another store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TwoSlotSnapshotV1 {
    domain: [u8; 32],
    store_nonce: [u8; 32],
    max_payload_bytes: usize,
    binding_digest: [u8; 32],
    generation: u64,
    record_digest: [u8; 32],
    payload: Vec<u8>,
}

impl TwoSlotSnapshotV1 {
    /// Return the committed monotonic generation.
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    /// Borrow the exact committed payload.
    pub(super) fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Return the domain-separated digest of this complete record.
    pub(super) fn record_digest(&self) -> [u8; 32] {
        self.record_digest
    }
}

/// A bounded local store backed by two fixed, retained private files.
#[derive(Debug, Clone)]
pub(super) struct TwoSlotStoreV1 {
    directory: RootedDirectory,
    config: TwoSlotStoreConfigV1,
    layout: TwoSlotLayoutV1,
    init_lock_identity: FileIdentity,
    binding_digest: [u8; 32],
    slots: [TwoSlotFileV1; 2],
    process_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TwoSlotCommittedRecordV1 {
    slot_id: usize,
    generation: u64,
    predecessor_digest: [u8; 32],
    record_digest: [u8; 32],
    payload: Vec<u8>,
}

#[derive(Debug)]
struct TwoSlotStageV1 {
    name: OsString,
    directory: RootedDirectory,
    byte_count: u64,
    complete: bool,
}

#[derive(Debug)]
struct TwoSlotStageInventoryV1 {
    byte_count: u64,
    has_full_pair: bool,
    canonical_header_count: usize,
}

fn two_slot_codec_error(label: &str, error: impl fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{label} is not canonical Norito: {error}"),
    )
}

fn encode_two_slot_value<T: norito::NoritoSerialize>(
    value: &T,
    label: &str,
) -> io::Result<Vec<u8>> {
    norito::to_bytes(value).map_err(|error| two_slot_codec_error(label, error))
}

fn decode_two_slot_value<T>(bytes: &[u8], label: &str) -> io::Result<T>
where
    for<'decode> T: norito::NoritoDeserialize<'decode> + norito::NoritoSerialize,
{
    let value =
        norito::decode_from_bytes(bytes).map_err(|error| two_slot_codec_error(label, error))?;
    let canonical = encode_two_slot_value(&value, label)?;
    if canonical != bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} uses a noncanonical Norito encoding"),
        ));
    }
    Ok(value)
}

fn zero_two_slot_binding_material() -> TwoSlotBindingMaterialV1 {
    TwoSlotBindingMaterialV1 {
        format_version: TWO_SLOT_FORMAT_VERSION_V1,
        store_name_digest: TWO_SLOT_ZERO_DIGEST,
        domain: TWO_SLOT_ZERO_DIGEST,
        store_nonce: TWO_SLOT_ZERO_DIGEST,
        max_payload_bytes: 0,
        header_region_bytes: 0,
        record_header_region_bytes: 0,
        commit_trailer_region_bytes: 0,
        init_lock_identity: TwoSlotFileIdentityV1 {
            platform: 0,
            first: 0,
            second: 0,
        },
        slot_identities: [
            TwoSlotFileIdentityV1 {
                platform: 0,
                first: 0,
                second: 0,
            },
            TwoSlotFileIdentityV1 {
                platform: 0,
                first: 0,
                second: 0,
            },
        ],
    }
}

fn two_slot_layout(max_payload_bytes: usize) -> io::Result<TwoSlotLayoutV1> {
    let header_region_bytes = encode_two_slot_value(
        &TwoSlotHeaderRegionV1 {
            header: TwoSlotHeaderV1 {
                binding: zero_two_slot_binding_material(),
                binding_digest: TWO_SLOT_ZERO_DIGEST,
                slot_id: 0,
            },
            reserved: [0; TWO_SLOT_HEADER_RESERVED_BYTES_V1],
        },
        "governance two-slot header region",
    )?
    .len();
    let record_header_region_bytes = encode_two_slot_value(
        &TwoSlotRecordHeaderRegionV1 {
            header: TwoSlotRecordHeaderV1 {
                format_version: TWO_SLOT_FORMAT_VERSION_V1,
                binding_digest: TWO_SLOT_ZERO_DIGEST,
                slot_id: 0,
                generation: 0,
                predecessor_digest: TWO_SLOT_ZERO_DIGEST,
                payload_len: 0,
                payload_digest: TWO_SLOT_ZERO_DIGEST,
            },
            reserved: [0; TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1],
        },
        "governance two-slot record-header region",
    )?
    .len();
    let commit_trailer_region_bytes = encode_two_slot_value(
        &TwoSlotCommitTrailerRegionV1 {
            trailer: TwoSlotCommitTrailerV1 {
                format_version: TWO_SLOT_FORMAT_VERSION_V1,
                binding_digest: TWO_SLOT_ZERO_DIGEST,
                slot_id: 0,
                generation: 0,
                record_digest: TWO_SLOT_ZERO_DIGEST,
                commit_marker: TWO_SLOT_COMMIT_MARKER_V1,
            },
            reserved: [0; TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1],
        },
        "governance two-slot commit-trailer region",
    )?
    .len();
    let payload_offset = header_region_bytes
        .checked_add(record_header_region_bytes)
        .ok_or_else(|| io::Error::other("governance two-slot payload offset overflowed"))?;
    let trailer_offset = payload_offset
        .checked_add(max_payload_bytes)
        .ok_or_else(|| io::Error::other("governance two-slot trailer offset overflowed"))?;
    let slot_file_bytes = trailer_offset
        .checked_add(commit_trailer_region_bytes)
        .ok_or_else(|| io::Error::other("governance two-slot file length overflowed"))?;
    Ok(TwoSlotLayoutV1 {
        header_region_bytes,
        record_header_region_bytes,
        payload_offset: u64::try_from(payload_offset)
            .map_err(|_| io::Error::other("governance two-slot payload offset exceeds u64"))?,
        trailer_offset: u64::try_from(trailer_offset)
            .map_err(|_| io::Error::other("governance two-slot trailer offset exceeds u64"))?,
        commit_trailer_region_bytes,
        slot_file_bytes: u64::try_from(slot_file_bytes)
            .map_err(|_| io::Error::other("governance two-slot file length exceeds u64"))?,
    })
}

fn two_slot_store_name_digest(config: &TwoSlotStoreConfigV1) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance.two-slot.store-name.v1\0");
    hasher.update(config.store_name.as_bytes());
    *hasher.finalize().as_bytes()
}

fn two_slot_store_namespace(config: &TwoSlotStoreConfigV1) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = two_slot_store_name_digest(config);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn two_slot_binding_material(
    config: &TwoSlotStoreConfigV1,
    layout: TwoSlotLayoutV1,
    init_lock_identity: FileIdentity,
    identities: [FileIdentity; 2],
) -> io::Result<TwoSlotBindingMaterialV1> {
    Ok(TwoSlotBindingMaterialV1 {
        format_version: TWO_SLOT_FORMAT_VERSION_V1,
        store_name_digest: two_slot_store_name_digest(config),
        domain: config.domain,
        store_nonce: config.store_nonce,
        max_payload_bytes: u64::try_from(config.max_payload_bytes)
            .map_err(|_| io::Error::other("governance two-slot payload bound exceeds u64"))?,
        header_region_bytes: u64::try_from(layout.header_region_bytes)
            .map_err(|_| io::Error::other("governance two-slot header region exceeds u64"))?,
        record_header_region_bytes: u64::try_from(layout.record_header_region_bytes).map_err(
            |_| io::Error::other("governance two-slot record-header region exceeds u64"),
        )?,
        commit_trailer_region_bytes: u64::try_from(layout.commit_trailer_region_bytes).map_err(
            |_| io::Error::other("governance two-slot commit-trailer region exceeds u64"),
        )?,
        init_lock_identity: init_lock_identity.into(),
        slot_identities: identities.map(Into::into),
    })
}

fn two_slot_binding_digest(material: &TwoSlotBindingMaterialV1) -> io::Result<[u8; 32]> {
    let encoded = encode_two_slot_value(material, "governance two-slot binding material")?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance.two-slot.binding.v1\0");
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}

fn two_slot_record_digest(header: &TwoSlotRecordHeaderV1, payload: &[u8]) -> io::Result<[u8; 32]> {
    let encoded = encode_two_slot_value(header, "governance two-slot record header")?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance.two-slot.record.v1\0");
    hasher.update(&encoded);
    hasher.update(payload);
    Ok(*hasher.finalize().as_bytes())
}

fn read_exact_file_region(file: &File, offset: u64, bytes: usize) -> io::Result<Vec<u8>> {
    let mut region = vec![0; bytes];
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt as _;
        file.read_exact_at(&mut region, offset)?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt as _;
        let mut read = 0_usize;
        while read < region.len() {
            let position = offset
                .checked_add(
                    u64::try_from(read).map_err(|_| {
                        io::Error::other("governance two-slot read offset exceeds u64")
                    })?,
                )
                .ok_or_else(|| io::Error::other("governance two-slot read offset overflowed"))?;
            let count = file.seek_read(&mut region[read..], position)?;
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "governance two-slot region is truncated",
                ));
            }
            read = read
                .checked_add(count)
                .ok_or_else(|| io::Error::other("governance two-slot read length overflowed"))?;
        }
    }
    #[cfg(any(unix, windows))]
    {
        Ok(region)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, offset, region);
        Err(platform::unsupported())
    }
}

fn write_exact_file_region(file: &File, offset: u64, bytes: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt as _;
        return file.write_all_at(bytes, offset);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt as _;
        let mut written = 0_usize;
        while written < bytes.len() {
            let position = offset
                .checked_add(u64::try_from(written).map_err(|_| {
                    io::Error::other("governance two-slot write offset exceeds u64")
                })?)
                .ok_or_else(|| io::Error::other("governance two-slot write offset overflowed"))?;
            let count = file.seek_write(&bytes[written..], position)?;
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "governance two-slot region write made no progress",
                ));
            }
            written = written
                .checked_add(count)
                .ok_or_else(|| io::Error::other("governance two-slot write length overflowed"))?;
        }
        return Ok(());
    }
    #[cfg(not(any(unix, windows)))]
    Err(platform::unsupported())
}

/// One exact opened regular-file binding retained across later verification.
#[derive(Debug, Clone)]
pub(super) struct FileBinding {
    handle: Arc<File>,
    identity: FileIdentity,
    parent: RootedDirectory,
    name: OsString,
    max_bytes: usize,
    private: bool,
}

impl FileBinding {
    /// Return the stable identity of the retained object.
    pub(super) fn identity(&self) -> FileIdentity {
        self.identity
    }

    /// Revalidate the retained object and its parent-relative name.
    pub(super) fn verify(&self) -> io::Result<()> {
        self.parent.verify_file_binding(
            &self.name,
            &self.handle,
            self.identity,
            self.max_bytes,
            self.private,
        )
    }
}

/// Bytes and an exact retained binding read through one opened file.
#[derive(Debug, Clone)]
pub(super) struct FileSnapshot {
    bytes: Vec<u8>,
    binding: FileBinding,
}

impl FileSnapshot {
    /// Borrow the authenticated bytes.
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consume the snapshot and return its bytes.
    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Clone the exact opened binding for later snapshot-wide verification.
    pub(super) fn binding(&self) -> FileBinding {
        self.binding.clone()
    }
}

/// Required destination state for one atomic replacement.
#[derive(Debug, Clone)]
pub(super) enum ExpectedFile {
    /// The destination must not exist at promotion time.
    Missing,
    /// The destination must still be this exact object at promotion time.
    Identity(FileBinding),
}

/// One exact regular file retained below a rooted directory.
#[derive(Debug)]
pub(super) struct RetainedFile {
    binding: FileBinding,
}

impl RetainedFile {
    /// Borrow the exact opened file handle.
    pub(super) fn handle(&self) -> &File {
        &self.binding.handle
    }

    /// Revalidate the handle and its current parent-relative binding.
    pub(super) fn verify(&self) -> io::Result<()> {
        self.binding.verify()
    }
}

#[derive(Debug, Clone)]
struct DirectoryBinding {
    parent: Arc<File>,
    parent_identity: FileIdentity,
    name: OsString,
}

/// One retained, stable directory capability.
#[derive(Debug, Clone)]
pub(super) struct RootedDirectory {
    handle: Arc<File>,
    identity: FileIdentity,
    /// Exact initial Windows owner SID; ownership changes are authority changes
    /// and therefore invalidate this retained capability.
    #[cfg(windows)]
    owner_sid: Vec<u8>,
    display_path: PathBuf,
    binding: Option<DirectoryBinding>,
    writable: bool,
}

impl RootedDirectory {
    /// Wrap a directory handle already retained by the Governance root guard.
    pub(super) fn from_retained(
        display_path: PathBuf,
        handle: Arc<File>,
        writable: bool,
    ) -> io::Result<Self> {
        platform::ensure_supported()?;
        let metadata = handle.metadata()?;
        validate_directory_metadata(&display_path, &metadata)?;
        #[cfg(windows)]
        let owner_sid = platform::directory_owner_sid(&handle, &display_path)?;
        let directory = Self {
            identity: file_identity(&metadata)?,
            handle,
            #[cfg(windows)]
            owner_sid,
            display_path,
            binding: None,
            writable,
        };
        directory.verify_handle()?;
        Ok(directory)
    }

    /// Open and retain a release-qualified root directory.
    #[cfg(windows)]
    pub(super) fn open_root(path: &Path, writable: bool) -> io::Result<Self> {
        platform::ensure_supported()?;
        let handle = Arc::new(platform::open_root(path, writable)?);
        Self::from_retained(path.to_path_buf(), handle, writable)
    }

    /// Revalidate the retained object and its current pathname binding.
    pub(super) fn verify_path_binding(&self, path: &Path) -> io::Result<()> {
        self.verify_handle()?;
        let linked = fs::symlink_metadata(path)?;
        validate_directory_metadata(path, &linked)?;
        if file_identity(&linked)? != self.identity {
            return Err(io::Error::other(format!(
                "governance directory path `{}` no longer names its retained object",
                path.display()
            )));
        }
        Ok(())
    }

    /// Revalidate this directory's retained handle and parent-relative binding.
    pub(super) fn verify(&self) -> io::Result<()> {
        self.verify_handle()?;
        if let Some(binding) = &self.binding {
            let parent_metadata = binding.parent.metadata()?;
            validate_directory_metadata(&self.display_path, &parent_metadata)?;
            if file_identity(&parent_metadata)? != binding.parent_identity {
                return Err(io::Error::other(format!(
                    "retained parent for governance directory `{}` changed identity",
                    self.display_path.display()
                )));
            }
            let linked = platform::open_directory(&binding.parent, &binding.name, self.writable)?;
            let linked_metadata = linked.metadata()?;
            validate_directory_metadata(&self.display_path, &linked_metadata)?;
            if file_identity(&linked_metadata)? != self.identity {
                return Err(io::Error::other(format!(
                    "governance directory binding `{}` was substituted",
                    self.display_path.display()
                )));
            }
        }
        self.verify_handle()
    }

    /// Return a path-free digest of this exact retained directory identity.
    pub(super) fn identity_digest(&self) -> io::Result<[u8; 32]> {
        self.verify()?;
        let identity = TwoSlotFileIdentityV1::from(self.identity);
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"sorafs.governance-rooted-fs.directory-identity.v1\0");
        hasher.update(&[identity.platform]);
        hasher.update(&identity.first.to_le_bytes());
        hasher.update(&identity.second.to_le_bytes());
        let digest = *hasher.finalize().as_bytes();
        self.verify()?;
        Ok(digest)
    }

    fn verify_handle(&self) -> io::Result<()> {
        let before = self.handle.metadata()?;
        validate_directory_metadata(&self.display_path, &before)?;
        if file_identity(&before)? != self.identity {
            return Err(io::Error::other(format!(
                "retained governance directory `{}` changed identity",
                self.display_path.display()
            )));
        }
        #[cfg(windows)]
        if platform::directory_owner_sid(&self.handle, &self.display_path)? != self.owner_sid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "retained governance directory `{}` changed owner SID",
                    self.display_path.display()
                ),
            ));
        }
        #[cfg(not(windows))]
        platform::validate_directory_acl(&self.handle, &self.display_path)?;
        let after = self.handle.metadata()?;
        validate_directory_metadata(&self.display_path, &after)?;
        if file_identity(&after)? != self.identity {
            return Err(io::Error::other(format!(
                "retained governance directory `{}` changed identity during ACL inspection",
                self.display_path.display()
            )));
        }
        Ok(())
    }

    /// Validate descriptor-bound ACL policy for this exact directory.
    #[cfg(windows)]
    pub(super) fn validate_acl(&self) -> io::Result<()> {
        self.verify_handle()?;
        validate_retained_directory_acl(&self.handle, &self.display_path)?;
        self.verify_handle()
    }

    /// Flush this exact directory handle.
    pub(super) fn sync_all(&self) -> io::Result<()> {
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot be flushed as a writer",
            ));
        }
        self.verify()?;
        self.handle.sync_all()?;
        self.verify()
    }

    /// Open one direct child directory without following links/reparse points.
    pub(super) fn open_directory(&self, name: &OsStr) -> io::Result<Self> {
        validate_component(name)?;
        self.verify()?;
        let handle = Arc::new(platform::open_directory(&self.handle, name, self.writable)?);
        let metadata = handle.metadata()?;
        let display_path = self.display_path.join(name);
        validate_directory_metadata(&display_path, &metadata)?;
        #[cfg(windows)]
        let owner_sid = platform::directory_owner_sid(&handle, &display_path)?;
        let child = Self {
            handle,
            identity: file_identity(&metadata)?,
            #[cfg(windows)]
            owner_sid,
            display_path,
            binding: Some(DirectoryBinding {
                parent: Arc::clone(&self.handle),
                parent_identity: self.identity,
                name: name.to_os_string(),
            }),
            writable: self.writable,
        };
        self.verify()?;
        child.verify()?;
        Ok(child)
    }

    /// Open or durably create one direct child directory.
    pub(super) fn open_or_create_directory(&self, name: &OsStr) -> io::Result<Self> {
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot create children",
            ));
        }
        match self.open_directory(name) {
            Ok(directory) => Ok(directory),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.verify()?;
                match platform::create_directory(&self.handle, name) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }
                let directory = self.open_directory(name)?;
                self.handle.sync_all()?;
                self.verify()?;
                directory.verify()?;
                Ok(directory)
            }
            Err(error) => Err(error),
        }
    }

    /// Create one direct child directory without adopting a pre-existing name.
    fn create_child_directory_exclusive(&self, name: &OsStr) -> io::Result<Self> {
        validate_component(name)?;
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot create a two-slot staging directory",
            ));
        }
        self.verify()?;
        platform::create_directory(&self.handle, name)?;
        let child = self.open_directory(name)?;
        self.verify()?;
        child.verify()?;
        Ok(child)
    }

    /// Move one exact retained child directory to a create-only rooted name.
    ///
    /// This operation never replaces or removes a pathname. The returned
    /// capability is reopened below the destination parent and checked against
    /// the exact source-directory identity retained before the rename.
    fn move_child_directory_exclusive(
        &self,
        child: Self,
        destination_parent: &Self,
        destination_name: &OsStr,
    ) -> io::Result<Self> {
        validate_component(destination_name)?;
        if !self.writable || !destination_parent.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot move two-slot recovery state",
            ));
        }
        let binding = child.binding.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance root cannot be moved as a two-slot child",
            )
        })?;
        if binding.parent_identity != self.identity || !Arc::ptr_eq(&binding.parent, &self.handle) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "two-slot staging directory belongs to another retained parent",
            ));
        }
        self.verify()?;
        destination_parent.verify()?;
        child.verify()?;
        let source_name = binding.name.clone();
        let identity = child.identity;
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            platform::rename_exclusive(
                &self.handle,
                &source_name,
                &destination_parent.handle,
                destination_name,
            )?;
        }
        #[cfg(windows)]
        {
            platform::rename_open_file(
                &destination_parent.handle,
                &child.handle,
                &source_name,
                destination_name,
            )?;
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            return Err(platform::unsupported());
        }

        let installed = destination_parent.open_directory(destination_name)?;
        if installed.identity != identity || file_identity(&child.handle.metadata()?)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "two-slot directory rename installed a substituted object",
            ));
        }
        match self.open_directory(&source_name) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(replacement) => {
                drop(replacement);
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "two-slot source name was substituted during directory rename",
                ));
            }
            Err(error) => return Err(error),
        }
        installed.verify()?;
        Ok(installed)
    }

    /// Open or atomically initialize a bounded two-fixed-slot V1 store.
    pub(super) fn open_or_create_two_slot_store_v1(
        &self,
        config: TwoSlotStoreConfigV1,
        initial_payload: &[u8],
    ) -> io::Result<TwoSlotStoreV1> {
        open_or_create_two_slot_store_v1_with(self, config, initial_payload, |_| Ok(()))
    }

    /// Load an already initialized two-slot store without mutation.
    pub(super) fn load_existing_two_slot_store_v1(
        &self,
        config: TwoSlotStoreConfigV1,
    ) -> io::Result<TwoSlotSnapshotV1> {
        load_existing_two_slot_store_v1(self, config)
    }

    #[cfg(test)]
    fn open_or_create_two_slot_store_v1_with_init_hook<Hook>(
        &self,
        config: TwoSlotStoreConfigV1,
        initial_payload: &[u8],
        after_step: Hook,
    ) -> io::Result<TwoSlotStoreV1>
    where
        Hook: FnMut(&'static str) -> io::Result<()>,
    {
        open_or_create_two_slot_store_v1_with(self, config, initial_payload, after_step)
    }

    /// Resolve a relative target below this retained directory.
    pub(super) fn resolve_parent(
        &self,
        relative: &Path,
        create_directories: bool,
    ) -> io::Result<(Self, OsString)> {
        if relative.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "rooted governance path must be relative",
            ));
        }
        let mut components = relative.components().peekable();
        let mut directory = self.clone();
        while let Some(component) = components.next() {
            let Component::Normal(name) = component else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "rooted governance path contains a non-canonical component",
                ));
            };
            validate_component(name)?;
            if components.peek().is_none() {
                return Ok((directory, name.to_os_string()));
            }
            directory = if create_directories {
                directory.open_or_create_directory(name)?
            } else {
                directory.open_directory(name)?
            };
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "rooted governance target is empty",
        ))
    }

    /// Read one direct child through a no-follow handle.
    pub(super) fn read_file(&self, name: &OsStr, max_bytes: usize) -> io::Result<FileSnapshot> {
        self.read_file_with_policy(name, max_bytes, false)
    }

    /// Read one private direct child through a no-follow handle.
    pub(super) fn read_private_file(
        &self,
        name: &OsStr,
        max_bytes: usize,
    ) -> io::Result<FileSnapshot> {
        self.read_file_with_policy(name, max_bytes, true)
    }

    fn read_file_with_policy(
        &self,
        name: &OsStr,
        max_bytes: usize,
        private: bool,
    ) -> io::Result<FileSnapshot> {
        validate_component(name)?;
        self.verify()?;
        let mut file = platform::open_file(&self.handle, name, false)?;
        let before = file.metadata()?;
        let path = self.display_path.join(name);
        validate_file_metadata(&path, &before, max_bytes, private)?;
        let identity = file_identity(&before)?;
        let max_bytes_u64 = u64::try_from(max_bytes)
            .map_err(|_| io::Error::other("governance file byte limit exceeds u64"))?;
        let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(max_bytes));
        (&mut file)
            .take(max_bytes_u64.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() > max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance state `{}` exceeds {max_bytes} bytes",
                    path.display()
                ),
            ));
        }
        let after = file.metadata()?;
        validate_file_metadata(&path, &after, max_bytes, private)?;
        if !metadata_stable_during_read(&before, &after) {
            return Err(io::Error::other(format!(
                "governance state `{}` changed while reading",
                path.display()
            )));
        }
        let linked = platform::open_file(&self.handle, name, false)?;
        let linked_metadata = linked.metadata()?;
        validate_file_metadata(&path, &linked_metadata, max_bytes, private)?;
        if file_identity(&linked_metadata)? != identity {
            return Err(io::Error::other(format!(
                "governance state `{}` changed while reading",
                path.display()
            )));
        }
        self.verify()?;
        Ok(FileSnapshot {
            bytes,
            binding: FileBinding {
                handle: Arc::new(file),
                identity,
                parent: self.clone(),
                name: name.to_os_string(),
                max_bytes,
                private,
            },
        })
    }

    /// Open or create one private direct child and retain its exact binding.
    pub(super) fn open_or_create_private_file(
        &self,
        name: &OsStr,
        max_bytes: usize,
    ) -> io::Result<RetainedFile> {
        validate_component(name)?;
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot create private files",
            ));
        }
        self.verify()?;
        let handle = match platform::open_read_write_file(&self.handle, name) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match platform::create_file(&self.handle, name) {
                    Ok(file) => file,
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        platform::open_read_write_file(&self.handle, name)?
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        };
        let metadata = handle.metadata()?;
        let path = self.display_path.join(name);
        validate_file_metadata(&path, &metadata, max_bytes, true)?;
        let identity = file_identity(&metadata)?;
        self.verify_file_binding(name, &handle, identity, max_bytes, true)?;
        Ok(RetainedFile {
            binding: FileBinding {
                handle: Arc::new(handle),
                identity,
                parent: self.clone(),
                name: name.to_os_string(),
                max_bytes,
                private: true,
            },
        })
    }

    fn verify_file_binding(
        &self,
        name: &OsStr,
        handle: &File,
        expected: FileIdentity,
        max_bytes: usize,
        private: bool,
    ) -> io::Result<()> {
        validate_component(name)?;
        self.verify()?;
        let path = self.display_path.join(name);
        let retained_metadata = handle.metadata()?;
        validate_file_metadata(&path, &retained_metadata, max_bytes, private)?;
        if file_identity(&retained_metadata)? != expected {
            return Err(io::Error::other(format!(
                "retained governance file `{}` changed identity",
                path.display()
            )));
        }
        let linked = platform::open_file(&self.handle, name, false)?;
        let linked_metadata = linked.metadata()?;
        validate_file_metadata(&path, &linked_metadata, max_bytes, private)?;
        if file_identity(&linked_metadata)? != expected {
            return Err(io::Error::other(format!(
                "governance file binding `{}` was substituted",
                path.display()
            )));
        }
        self.verify()
    }

    /// Return the stable identity of a direct regular child, if present.
    pub(super) fn file_identity(&self, name: &OsStr) -> io::Result<Option<FileIdentity>> {
        self.file_identity_with_policy(name, false)
    }

    /// Retain one direct regular child and its exact name binding, if present.
    pub(super) fn file_binding(
        &self,
        name: &OsStr,
        max_bytes: usize,
    ) -> io::Result<Option<FileBinding>> {
        self.file_binding_with_policy_and_access(name, max_bytes, false, false)
    }

    /// Retain one direct regular child with deletion access to its exact handle.
    pub(super) fn removal_file_binding(
        &self,
        name: &OsStr,
        max_bytes: usize,
    ) -> io::Result<Option<FileBinding>> {
        self.file_binding_with_policy_and_access(name, max_bytes, false, true)
    }

    /// Retain one private direct regular child with deletion access to its exact handle.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn private_removal_file_binding(
        &self,
        name: &OsStr,
        max_bytes: usize,
    ) -> io::Result<Option<FileBinding>> {
        self.file_binding_with_policy_and_access(name, max_bytes, true, true)
    }

    fn file_binding_with_policy_and_access(
        &self,
        name: &OsStr,
        max_bytes: usize,
        private: bool,
        delete_access: bool,
    ) -> io::Result<Option<FileBinding>> {
        validate_component(name)?;
        self.verify()?;
        let handle = match platform::open_file(&self.handle, name, delete_access) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.verify()?;
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let metadata = handle.metadata()?;
        let path = self.display_path.join(name);
        validate_file_metadata(&path, &metadata, max_bytes, private)?;
        let identity = file_identity(&metadata)?;
        self.verify_file_binding(name, &handle, identity, max_bytes, private)?;
        Ok(Some(FileBinding {
            handle: Arc::new(handle),
            identity,
            parent: self.clone(),
            name: name.to_os_string(),
            max_bytes,
            private,
        }))
    }

    fn file_identity_with_policy(
        &self,
        name: &OsStr,
        private: bool,
    ) -> io::Result<Option<FileIdentity>> {
        validate_component(name)?;
        self.verify()?;
        match platform::open_file(&self.handle, name, false) {
            Ok(file) => {
                let metadata = file.metadata()?;
                let path = self.display_path.join(name);
                if private {
                    validate_private_regular_file_metadata(&path, &metadata)?;
                } else {
                    validate_regular_file_metadata(&path, &metadata)?;
                }
                let identity = file_identity(&metadata)?;
                self.verify()?;
                Ok(Some(identity))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.verify()?;
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// Atomically replace the target only if its stable identity is unchanged.
    ///
    /// Linux/macOS exchange both bindings before retaining the predecessor.
    /// Windows supports create-only installation and exact-byte no-ops, but
    /// fails changed existing-target replacement closed because it has no
    /// rooted atomic exchange that preserves every raced object. Retained
    /// generations are immutable online; saturation requires offline archival
    /// or cleanup while the writer is stopped.
    pub(super) fn atomic_write(
        &self,
        name: &OsStr,
        temporary_name: &OsStr,
        data: &[u8],
        expected: ExpectedFile,
    ) -> io::Result<()> {
        self.atomic_write_with_sync(
            name,
            temporary_name,
            data,
            expected,
            || Ok(()),
            |file| file.sync_all(),
            |directory| directory.sync_all(),
        )
    }

    fn atomic_write_with_sync<BeforePromote, FileSync, DirectorySync>(
        &self,
        name: &OsStr,
        temporary_name: &OsStr,
        data: &[u8],
        expected: ExpectedFile,
        before_promote: BeforePromote,
        mut sync_file: FileSync,
        mut sync_directory: DirectorySync,
    ) -> io::Result<()>
    where
        BeforePromote: FnOnce() -> io::Result<()>,
        FileSync: FnMut(&File) -> io::Result<()>,
        DirectorySync: FnMut(&File) -> io::Result<()>,
    {
        validate_component(name)?;
        validate_component(temporary_name)?;
        if name == temporary_name {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance atomic target and temporary name must differ",
            ));
        }
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot write",
            ));
        }
        self.verify().map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "verify governance atomic directory `{}`: {error}",
                    self.display_path.display()
                ),
            )
        })?;
        verify_expected_file(self, name, &expected).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "verify governance atomic predecessor `{}`: {error}",
                    self.display_path.join(name).display()
                ),
            )
        })?;
        if let ExpectedFile::Identity(expected_binding) = &expected {
            let current = if expected_binding.private {
                self.read_private_file(name, expected_binding.max_bytes)?
            } else {
                self.read_file(name, expected_binding.max_bytes)?
            };
            if current.binding.identity != expected_binding.identity {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "governance atomic predecessor `{}` changed before exact-byte comparison",
                        self.display_path.join(name).display()
                    ),
                ));
            }
            if current.bytes == data {
                expected_binding.verify()?;
                current.binding.verify()?;
                self.verify()?;
                return Ok(());
            }
            #[cfg(windows)]
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Windows governance existing-target replacement is disabled because the platform has no rooted atomic exchange that preserves every raced object",
            ));
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let retained_name = match &expected {
            ExpectedFile::Identity(binding) => {
                let metadata = binding.handle.metadata()?;
                validate_regular_file_metadata(&self.display_path.join(name), &metadata)?;
                Some(self.available_atomic_retained_name(name, metadata.len(), binding.private)?)
            }
            ExpectedFile::Missing => None,
        };
        let mut temporary =
            platform::create_file(&self.handle, temporary_name).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "create governance atomic temporary `{}`: {error}",
                        self.display_path.join(temporary_name).display()
                    ),
                )
            })?;
        let temporary_path = self.display_path.join(temporary_name);
        #[cfg(windows)]
        let mut renamed = false;
        let result = (|| {
            temporary.write_all(data).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "write governance atomic temporary `{}`: {error}",
                        temporary_path.display()
                    ),
                )
            })?;
            sync_file(&temporary).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "sync governance atomic temporary `{}`: {error}",
                        temporary_path.display()
                    ),
                )
            })?;
            let temporary_metadata = temporary.metadata()?;
            validate_private_regular_file_metadata(&temporary_path, &temporary_metadata)?;
            let temporary_identity = file_identity(&temporary_metadata)?;
            verify_expected_file(self, name, &expected).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "reverify governance atomic predecessor `{}`: {error}",
                        self.display_path.join(name).display()
                    ),
                )
            })?;
            self.verify().map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "reverify governance atomic directory `{}`: {error}",
                        self.display_path.display()
                    ),
                )
            })?;
            before_promote()?;
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            if retained_name.is_some() {
                platform::exchange_open_file(&self.handle, &temporary, temporary_name, name)
            } else {
                platform::rename_open_file(&self.handle, &temporary, temporary_name, name)
            }
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "promote governance atomic temporary `{}` to `{}`: {error}",
                        temporary_path.display(),
                        self.display_path.join(name).display()
                    ),
                )
            })?;
            #[cfg(windows)]
            {
                platform::rename_open_file(
                    &self.handle,
                    &temporary,
                    temporary_name,
                    name,
                )
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "promote governance atomic temporary `{}` to `{}` without replacement: {error}",
                            temporary_path.display(),
                            self.display_path.join(name).display()
                        ),
                    )
                })?;
                renamed = true;
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
            platform::rename_open_file(&self.handle, &temporary, temporary_name, name).map_err(
                |error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "promote governance atomic temporary `{}` to `{}`: {error}",
                            temporary_path.display(),
                            self.display_path.join(name).display()
                        ),
                    )
                },
            )?;
            sync_directory(&self.handle).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "sync governance atomic directory `{}`: {error}",
                        self.display_path.display()
                    ),
                )
            })?;
            let promoted = platform::open_file(&self.handle, name, false).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "open promoted governance atomic target `{}`: {error}",
                        self.display_path.join(name).display()
                    ),
                )
            })?;
            let promoted_metadata = promoted.metadata()?;
            validate_private_regular_file_metadata(
                &self.display_path.join(name),
                &promoted_metadata,
            )?;
            if file_identity(&promoted_metadata)? != temporary_identity {
                return Err(io::Error::other(format!(
                    "governance atomic target `{}` is not the promoted temporary object",
                    self.display_path.join(name).display()
                )));
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            if let Some(retained_name) = retained_name.as_deref() {
                let (expected_identity, expected_max_bytes, expected_private) = match &expected {
                    ExpectedFile::Identity(binding) => {
                        (binding.identity, binding.max_bytes, binding.private)
                    }
                    ExpectedFile::Missing => unreachable!("retained replacement has an identity"),
                };
                let predecessor = platform::open_file(&self.handle, temporary_name, false)
                    .map_err(|error| {
                        io::Error::new(
                            error.kind(),
                            format!(
                                "open exchanged governance atomic predecessor `{}`: {error}",
                                temporary_path.display()
                            ),
                        )
                    })?;
                let predecessor_metadata = predecessor.metadata()?;
                validate_file_metadata(
                    &temporary_path,
                    &predecessor_metadata,
                    expected_max_bytes,
                    expected_private,
                )?;
                if file_identity(&predecessor_metadata)? != expected_identity {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        format!(
                            "governance atomic predecessor `{}` was substituted during exchange; both objects were preserved",
                            self.display_path.join(name).display()
                        ),
                    ));
                }
                platform::rename_exclusive(
                    &self.handle,
                    temporary_name,
                    &self.handle,
                    retained_name,
                )
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "retain governance atomic predecessor `{}` as `{}`: {error}; the predecessor remains preserved for offline recovery",
                            temporary_path.display(),
                            self.display_path.join(retained_name).display()
                        ),
                    )
                })?;
                sync_directory(&self.handle).map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "sync governance atomic predecessor retention directory `{}`: {error}",
                            self.display_path.display()
                        ),
                    )
                })?;
                let retained_path = self.display_path.join(retained_name);
                let retained = platform::open_file(&self.handle, retained_name, false)?;
                let retained_metadata = retained.metadata()?;
                validate_file_metadata(
                    &retained_path,
                    &retained_metadata,
                    expected_max_bytes,
                    expected_private,
                )?;
                if file_identity(&retained_metadata)? != expected_identity
                    || file_identity(&predecessor.metadata()?)? != expected_identity
                    || retained_metadata.len() != predecessor_metadata.len()
                {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "retained governance atomic predecessor was substituted; every observed object remains preserved for offline inspection",
                    ));
                }
                self.require_file_name_absent(temporary_name)?;
            }
            self.verify().map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "verify durable governance atomic directory `{}`: {error}",
                        self.display_path.display()
                    ),
                )
            })?;
            let durable = platform::open_file(&self.handle, name, false).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "open durable governance atomic target `{}`: {error}",
                        self.display_path.join(name).display()
                    ),
                )
            })?;
            let durable_metadata = durable.metadata()?;
            validate_private_regular_file_metadata(
                &self.display_path.join(name),
                &durable_metadata,
            )?;
            if file_identity(&durable_metadata)? != temporary_identity {
                return Err(io::Error::other(format!(
                    "governance atomic target `{}` changed before durable readback",
                    self.display_path.join(name).display()
                )));
            }
            Ok(())
        })();
        #[cfg(windows)]
        if result.is_err() && !renamed {
            let _ = platform::remove_open_file(
                &self.handle,
                &temporary,
                temporary_name,
                file_identity(&temporary.metadata()?).ok(),
            );
        }
        // POSIX has no conditional unlink-by-descriptor, so a failed
        // transaction keeps every ambiguous object available for recovery.
        // Successful replacement retains the exact predecessor in a bounded
        // V1 slot. Windows never enters existing-target replacement.
        result
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn available_atomic_retained_name(
        &self,
        target_name: &OsStr,
        predecessor_bytes: u64,
        private: bool,
    ) -> io::Result<OsString> {
        let target_name = target_name.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance atomic retention target is not canonical UTF-8",
            )
        })?;
        let mut occupied = [false; ATOMIC_RETAINED_SLOT_COUNT_V1];
        let mut retained_bytes = 0_u64;
        for name in self.child_names_bounded(DEFAULT_CHILD_ENTRY_LIMIT)? {
            let Some(name_utf8) = name.to_str() else {
                continue;
            };
            let Some((retained_target, slot)) = atomic_retained_target_and_slot(name_utf8) else {
                if is_atomic_retained_candidate_for(name_utf8, target_name) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "governance atomic retention name `{}` is not canonical; offline inspection is required",
                            self.display_path.join(&name).display()
                        ),
                    ));
                }
                continue;
            };
            let retained = platform::open_file(&self.handle, &name, false)?;
            let metadata = retained.metadata()?;
            validate_file_metadata(
                &self.display_path.join(&name),
                &metadata,
                usize::MAX,
                private && retained_target == target_name,
            )?;
            let identity = file_identity(&metadata)?;
            let linked = platform::open_file(&self.handle, &name, false)?;
            let linked_metadata = linked.metadata()?;
            validate_file_metadata(
                &self.display_path.join(&name),
                &linked_metadata,
                usize::MAX,
                private && retained_target == target_name,
            )?;
            if file_identity(&linked_metadata)? != identity {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "governance atomic retained generation changed during bounded inventory",
                ));
            }
            retained_bytes = retained_bytes.checked_add(metadata.len()).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance atomic retained-generation byte total overflowed",
                )
            })?;
            if retained_target == target_name {
                occupied[slot] = true;
            }
        }
        let total = retained_bytes
            .checked_add(predecessor_bytes)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance atomic retained-generation byte total overflowed",
                )
            })?;
        if total > ATOMIC_RETAINED_TOTAL_MAX_BYTES_V1 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "governance atomic retained generations would exceed the {}-byte V1 aggregate bound; stop the writer and archive or clear retained generations offline",
                    ATOMIC_RETAINED_TOTAL_MAX_BYTES_V1
                ),
            ));
        }
        let slot = occupied
            .iter()
            .position(|occupied| !occupied)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "all {ATOMIC_RETAINED_SLOT_COUNT_V1} V1 predecessor slots for `{target_name}` are occupied; stop the writer and archive or clear them offline"
                    ),
                )
            })?;
        atomic_retained_name(OsStr::new(target_name), slot)
    }

    /// Atomically write, binding replacement to the currently opened target.
    pub(super) fn atomic_replace_current(
        &self,
        name: &OsStr,
        temporary_name: &OsStr,
        data: &[u8],
    ) -> io::Result<()> {
        let expected = match self.file_binding(name, usize::MAX)? {
            Some(binding) => ExpectedFile::Identity(binding),
            None => ExpectedFile::Missing,
        };
        self.atomic_write(name, temporary_name, data, expected)
    }

    /// Enumerate direct child names while retaining this exact directory.
    #[cfg(test)]
    pub(super) fn child_names(&self) -> io::Result<Vec<OsString>> {
        self.child_names_bounded(DEFAULT_CHILD_ENTRY_LIMIT)
    }

    /// Enumerate at most `max_entries` direct child names.
    pub(super) fn child_names_bounded(&self, max_entries: usize) -> io::Result<Vec<OsString>> {
        if max_entries == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance directory enumeration bound must be positive",
            ));
        }
        self.verify()?;
        let mut names = platform::child_names(self, max_entries)?;
        names.sort();
        self.verify()?;
        Ok(names)
    }

    /// Remove matching atomic crash temporaries below this exact directory.
    #[cfg(any(windows, test))]
    pub(super) fn remove_atomic_temps_for(&self, target_name: &str) -> io::Result<usize> {
        validate_component(OsStr::new(target_name))?;
        self.remove_atomic_temps_matching(DEFAULT_CHILD_ENTRY_LIMIT, |candidate| {
            candidate == target_name
        })
    }

    /// Remove bounded atomic crash temporaries whose decoded target is allowed.
    #[cfg(any(windows, test))]
    pub(super) fn remove_atomic_temps_matching<Allowed>(
        &self,
        max_entries: usize,
        mut allowed: Allowed,
    ) -> io::Result<usize>
    where
        Allowed: FnMut(&str) -> bool,
    {
        let mut removed = 0usize;
        for name in self.child_names_bounded(max_entries)? {
            let Some(name_utf8) = name.to_str() else {
                continue;
            };
            let Some(target_name) = atomic_temp_target_name(name_utf8) else {
                continue;
            };
            if !allowed(target_name) {
                continue;
            }
            self.verify()?;
            let file = platform::open_file(&self.handle, &name, true)?;
            let metadata = file.metadata()?;
            validate_regular_file_metadata(&self.display_path.join(&name), &metadata)?;
            let identity = file_identity(&metadata)?;
            let linked = platform::open_file(&self.handle, &name, false)?;
            let linked_metadata = linked.metadata()?;
            validate_regular_file_metadata(&self.display_path.join(&name), &linked_metadata)?;
            if file_identity(&linked_metadata)? != identity {
                return Err(io::Error::other(format!(
                    "governance atomic temporary `{}` changed before recovery",
                    self.display_path.join(&name).display()
                )));
            }
            platform::remove_open_file(&self.handle, &file, &name, Some(identity))?;
            drop(linked);
            drop(file);
            self.require_file_name_absent(&name)?;
            removed = removed.saturating_add(1);
        }
        if removed != 0 {
            self.handle.sync_all()?;
        }
        self.verify()?;
        Ok(removed)
    }

    /// Atomically isolate one exact regular-file binding without unlinking it.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn isolate_file_binding(
        &self,
        binding: FileBinding,
        quarantine: &Self,
        quarantine_name: &OsStr,
    ) -> io::Result<FileSnapshot> {
        self.isolate_file_binding_with(binding, quarantine, quarantine_name, || Ok(()))
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn isolate_file_binding_with<BeforeRename>(
        &self,
        binding: FileBinding,
        quarantine: &Self,
        quarantine_name: &OsStr,
        before_rename: BeforeRename,
    ) -> io::Result<FileSnapshot>
    where
        BeforeRename: FnOnce() -> io::Result<()>,
    {
        self.isolate_file_binding_with_sync(
            binding,
            quarantine,
            quarantine_name,
            before_rename,
            |directory| directory.sync_all(),
            |directory| directory.sync_all(),
        )
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn isolate_file_binding_with_sync<BeforeRename, SyncSource, SyncQuarantine>(
        &self,
        binding: FileBinding,
        quarantine: &Self,
        quarantine_name: &OsStr,
        before_rename: BeforeRename,
        sync_source: SyncSource,
        sync_quarantine: SyncQuarantine,
    ) -> io::Result<FileSnapshot>
    where
        BeforeRename: FnOnce() -> io::Result<()>,
        SyncSource: FnOnce(&File) -> io::Result<()>,
        SyncQuarantine: FnOnce(&File) -> io::Result<()>,
    {
        validate_component(quarantine_name)?;
        if !self.writable || !quarantine.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot isolate recovery children",
            ));
        }
        self.verify()?;
        quarantine.verify()?;
        binding.verify()?;
        if binding.parent.identity != self.identity
            || !Arc::ptr_eq(&binding.parent.handle, &self.handle)
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "planned governance file binding belongs to a different parent",
            ));
        }
        if self.identity == quarantine.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance recovery quarantine must be a distinct directory",
            ));
        }
        let FileBinding {
            handle,
            identity,
            name,
            max_bytes,
            private,
            ..
        } = binding;
        before_rename()?;
        platform::rename_exclusive(&self.handle, &name, &quarantine.handle, quarantine_name)?;
        let source_sync = sync_source(&self.handle);
        let quarantine_sync = sync_quarantine(&quarantine.handle);
        source_sync?;
        quarantine_sync?;

        let snapshot = quarantine.read_file_with_policy(quarantine_name, max_bytes, private)?;
        if snapshot.binding.identity != identity || file_identity(&handle.metadata()?)? != identity
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance recovery quarantine captured a substituted file; the preserved entry requires offline inspection",
            ));
        }
        self.require_file_name_absent(&name)?;
        snapshot.binding.verify()?;
        self.verify()?;
        quarantine.verify()?;
        Ok(snapshot)
    }

    /// Remove one direct regular child by exact opened identity.
    #[cfg(any(windows, test))]
    pub(super) fn remove_file_binding(&self, binding: FileBinding) -> io::Result<()> {
        self.verify()?;
        binding.verify()?;
        if binding.parent.identity != self.identity
            || !Arc::ptr_eq(&binding.parent.handle, &self.handle)
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "planned governance file binding belongs to a different parent",
            ));
        }
        let FileBinding {
            handle,
            identity,
            name,
            ..
        } = binding;
        platform::remove_open_file(&self.handle, &handle, &name, Some(identity))?;
        drop(handle);
        self.require_file_name_absent(&name)?;
        self.handle.sync_all()?;
        self.verify()
    }

    fn require_file_name_absent(&self, name: &OsStr) -> io::Result<()> {
        match platform::open_file(&self.handle, name, false) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Ok(replacement) => {
                drop(replacement);
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "governance file `{}` was replaced during removal",
                        self.display_path.join(name).display()
                    ),
                ))
            }
            Err(error) => Err(error),
        }
    }

    /// Atomically isolate one exact empty directory without unlinking it.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn isolate_empty_directory_binding(
        &self,
        child: Self,
        quarantine: &Self,
        quarantine_name: &OsStr,
    ) -> io::Result<()> {
        self.isolate_empty_directory_binding_with(child, quarantine, quarantine_name, || Ok(()))
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn isolate_empty_directory_binding_with<BeforeRename>(
        &self,
        child: Self,
        quarantine: &Self,
        quarantine_name: &OsStr,
        before_rename: BeforeRename,
    ) -> io::Result<()>
    where
        BeforeRename: FnOnce() -> io::Result<()>,
    {
        self.isolate_empty_directory_binding_with_sync(
            child,
            quarantine,
            quarantine_name,
            before_rename,
            |directory| directory.sync_all(),
            |directory| directory.sync_all(),
        )
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn isolate_empty_directory_binding_with_sync<BeforeRename, SyncSource, SyncQuarantine>(
        &self,
        child: Self,
        quarantine: &Self,
        quarantine_name: &OsStr,
        before_rename: BeforeRename,
        sync_source: SyncSource,
        sync_quarantine: SyncQuarantine,
    ) -> io::Result<()>
    where
        BeforeRename: FnOnce() -> io::Result<()>,
        SyncSource: FnOnce(&File) -> io::Result<()>,
        SyncQuarantine: FnOnce(&File) -> io::Result<()>,
    {
        validate_component(quarantine_name)?;
        let binding = child.binding.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance root cannot be isolated as a retained child",
            )
        })?;
        validate_component(&binding.name)?;
        if !self.writable || !quarantine.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot isolate recovery children",
            ));
        }
        self.verify()?;
        child.verify()?;
        quarantine.verify()?;
        if binding.parent_identity != self.identity || !Arc::ptr_eq(&binding.parent, &self.handle) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "planned governance directory binding belongs to a different parent",
            ));
        }
        if self.identity == quarantine.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance recovery quarantine must be a distinct directory",
            ));
        }
        if !child.child_names_bounded(1)?.is_empty() {
            return Err(io::Error::other(format!(
                "governance directory `{}` is not empty",
                child.display_path.display()
            )));
        }
        let name = binding.name.clone();
        let identity = child.identity;
        before_rename()?;
        platform::rename_exclusive(&self.handle, &name, &quarantine.handle, quarantine_name)?;
        let source_sync = sync_source(&self.handle);
        let quarantine_sync = sync_quarantine(&quarantine.handle);
        source_sync?;
        quarantine_sync?;

        let isolated = quarantine.open_directory(quarantine_name)?;
        if isolated.identity != identity || file_identity(&child.handle.metadata()?)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance recovery quarantine captured a substituted directory; the preserved entry requires offline inspection",
            ));
        }
        if !isolated.child_names_bounded(1)?.is_empty() {
            return Err(io::Error::other(
                "isolated governance recovery directory changed after quarantine",
            ));
        }
        match self.open_directory(&name) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(replacement) => {
                drop(replacement);
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "governance directory was replaced during recovery isolation",
                ));
            }
            Err(error) => return Err(error),
        }
        isolated.verify()?;
        self.verify()?;
        quarantine.verify()
    }

    /// Remove one direct empty child directory by its exact retained identity.
    #[cfg(any(windows, test))]
    pub(super) fn remove_empty_directory_binding(&self, child: Self) -> io::Result<()> {
        let binding = child.binding.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance root cannot be removed as a retained child",
            )
        })?;
        validate_component(&binding.name)?;
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot remove children",
            ));
        }
        self.verify()?;
        child.verify()?;
        if binding.parent_identity != self.identity || !Arc::ptr_eq(&binding.parent, &self.handle) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "planned governance directory binding belongs to a different parent",
            ));
        }
        if !child.child_names_bounded(1)?.is_empty() {
            return Err(io::Error::other(format!(
                "governance directory `{}` is not empty",
                child.display_path.display()
            )));
        }
        let name = binding.name.clone();
        let identity = child.identity;
        platform::remove_open_directory(&self.handle, &child.handle, &name, Some(identity))?;
        drop(child);
        match self.open_directory(&name) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(replacement) => {
                drop(replacement);
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "governance directory `{}` was replaced during removal",
                        self.display_path.join(&name).display()
                    ),
                ));
            }
            Err(error) => return Err(error),
        }
        self.handle.sync_all()?;
        self.verify()
    }

    #[cfg(test)]
    fn atomic_write_with_test_sync<FileSync, DirectorySync>(
        &self,
        name: &OsStr,
        temporary_name: &OsStr,
        data: &[u8],
        expected: ExpectedFile,
        sync_file: FileSync,
        sync_directory: DirectorySync,
    ) -> io::Result<()>
    where
        FileSync: FnMut(&File) -> io::Result<()>,
        DirectorySync: FnMut(&File) -> io::Result<()>,
    {
        self.atomic_write_with_test_hooks(
            name,
            temporary_name,
            data,
            expected,
            || Ok(()),
            sync_file,
            sync_directory,
        )
    }

    #[cfg(test)]
    fn atomic_write_with_test_hooks<BeforePromote, FileSync, DirectorySync>(
        &self,
        name: &OsStr,
        temporary_name: &OsStr,
        data: &[u8],
        expected: ExpectedFile,
        before_promote: BeforePromote,
        sync_file: FileSync,
        sync_directory: DirectorySync,
    ) -> io::Result<()>
    where
        BeforePromote: FnOnce() -> io::Result<()>,
        FileSync: FnMut(&File) -> io::Result<()>,
        DirectorySync: FnMut(&File) -> io::Result<()>,
    {
        self.atomic_write_with_sync(
            name,
            temporary_name,
            data,
            expected,
            before_promote,
            sync_file,
            sync_directory,
        )
    }
}

fn two_slot_file_byte_limit(layout: TwoSlotLayoutV1) -> io::Result<usize> {
    usize::try_from(layout.slot_file_bytes)
        .map_err(|_| io::Error::other("governance two-slot file length exceeds host limits"))
}

fn expected_two_slot_header_region(
    material: &TwoSlotBindingMaterialV1,
    binding_digest: [u8; 32],
    slot_id: usize,
) -> io::Result<Vec<u8>> {
    let slot_id = u8::try_from(slot_id)
        .map_err(|_| io::Error::other("governance two-slot slot id exceeds u8"))?;
    encode_two_slot_value(
        &TwoSlotHeaderRegionV1 {
            header: TwoSlotHeaderV1 {
                binding: material.clone(),
                binding_digest,
                slot_id,
            },
            reserved: [0; TWO_SLOT_HEADER_RESERVED_BYTES_V1],
        },
        "governance two-slot header region",
    )
}

fn open_existing_two_slot_file(
    directory: &RootedDirectory,
    name: &OsStr,
    layout: TwoSlotLayoutV1,
) -> io::Result<TwoSlotFileV1> {
    validate_component(name)?;
    directory.verify()?;
    let handle = if directory.writable {
        platform::open_read_write_file(&directory.handle, name)?
    } else {
        platform::open_file(&directory.handle, name, false)?
    };
    let metadata = handle.metadata()?;
    let path = directory.display_path.join(name);
    let max_bytes = two_slot_file_byte_limit(layout)?;
    validate_file_metadata(&path, &metadata, max_bytes, true)?;
    if metadata.len() != layout.slot_file_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance two-slot file `{}` has length {}, expected {}",
                path.display(),
                metadata.len(),
                layout.slot_file_bytes
            ),
        ));
    }
    let identity = file_identity(&metadata)?;
    directory.verify_file_binding(name, &handle, identity, max_bytes, true)?;
    Ok(TwoSlotFileV1 {
        handle: Arc::new(handle),
        identity,
        name: name.to_os_string(),
    })
}

fn verify_two_slot_file(store: &TwoSlotStoreV1, slot: &TwoSlotFileV1) -> io::Result<()> {
    let max_bytes = two_slot_file_byte_limit(store.layout)?;
    let path = store.directory.display_path.join(&slot.name);
    let metadata = slot.handle.metadata()?;
    validate_file_metadata(&path, &metadata, max_bytes, true)?;
    if metadata.len() != store.layout.slot_file_bytes || file_identity(&metadata)? != slot.identity
    {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "governance two-slot file `{}` changed identity or length",
                path.display()
            ),
        ));
    }
    store
        .directory
        .verify_file_binding(&slot.name, &slot.handle, slot.identity, max_bytes, true)
}

fn verify_two_slot_headers(store: &TwoSlotStoreV1) -> io::Result<()> {
    store.directory.verify()?;
    let mut children = store
        .directory
        .child_names_bounded(TWO_SLOT_NAMES_V1.len())?;
    children.sort();
    let mut expected_children = TWO_SLOT_NAMES_V1.map(OsString::from).to_vec();
    expected_children.sort();
    if children != expected_children {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot store inventory diverged from its exact V1 pair",
        ));
    }
    if store.slots[0].identity == store.slots[1].identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot files alias the same filesystem object",
        ));
    }
    let material = two_slot_binding_material(
        &store.config,
        store.layout,
        store.init_lock_identity,
        [store.slots[0].identity, store.slots[1].identity],
    )?;
    if two_slot_binding_digest(&material)? != store.binding_digest {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot in-memory binding digest diverged",
        ));
    }
    for (slot_id, slot) in store.slots.iter().enumerate() {
        verify_two_slot_file(store, slot)?;
        let actual = read_exact_file_region(&slot.handle, 0, store.layout.header_region_bytes)?;
        let decoded: TwoSlotHeaderRegionV1 =
            decode_two_slot_value(&actual, "governance two-slot header region")?;
        if decoded.reserved != [0; TWO_SLOT_HEADER_RESERVED_BYTES_V1]
            || actual != expected_two_slot_header_region(&material, store.binding_digest, slot_id)?
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance two-slot immutable header `{}` diverged",
                    store.directory.display_path.join(&slot.name).display()
                ),
            ));
        }
    }
    store.directory.verify()
}

fn open_existing_two_slot_store(
    directory: RootedDirectory,
    config: TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
) -> io::Result<TwoSlotStoreV1> {
    let mut children = directory.child_names_bounded(TWO_SLOT_NAMES_V1.len())?;
    children.sort();
    let mut expected = TWO_SLOT_NAMES_V1.map(OsString::from).to_vec();
    expected.sort();
    if children != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot store inventory is not the exact V1 pair",
        ));
    }
    let layout = two_slot_layout(config.max_payload_bytes)?;
    let slots = [
        open_existing_two_slot_file(&directory, OsStr::new(TWO_SLOT_NAMES_V1[0]), layout)?,
        open_existing_two_slot_file(&directory, OsStr::new(TWO_SLOT_NAMES_V1[1]), layout)?,
    ];
    if slots[0].identity == slots[1].identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot files alias the same identity",
        ));
    }
    let material = two_slot_binding_material(
        &config,
        layout,
        init_lock_identity,
        [slots[0].identity, slots[1].identity],
    )?;
    let binding_digest = two_slot_binding_digest(&material)?;
    let store = TwoSlotStoreV1 {
        directory,
        config,
        layout,
        init_lock_identity,
        binding_digest,
        slots,
        process_lock: Arc::new(Mutex::new(())),
    };
    verify_two_slot_headers(&store)?;
    Ok(store)
}

fn read_two_slot_record_once(
    store: &TwoSlotStoreV1,
    slot_id: usize,
) -> io::Result<Option<TwoSlotCommittedRecordV1>> {
    let slot = store.slots.get(slot_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance two-slot id is invalid",
        )
    })?;
    let trailer_before = read_exact_file_region(
        &slot.handle,
        store.layout.trailer_offset,
        store.layout.commit_trailer_region_bytes,
    )?;
    let record_region = read_exact_file_region(
        &slot.handle,
        u64::try_from(store.layout.header_region_bytes).map_err(|_| {
            io::Error::other("governance two-slot record-header offset exceeds u64")
        })?,
        store.layout.record_header_region_bytes,
    )?;
    let absent_if_zero_trailer_stable = || {
        let trailer_after = read_exact_file_region(
            &slot.handle,
            store.layout.trailer_offset,
            store.layout.commit_trailer_region_bytes,
        )?;
        if trailer_before == trailer_after {
            Ok(None)
        } else {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot trailer changed during invalid-record read",
            ))
        }
    };
    let corrupt_if_trailer_stable = || {
        let trailer_after = read_exact_file_region(
            &slot.handle,
            store.layout.trailer_offset,
            store.layout.commit_trailer_region_bytes,
        )?;
        if trailer_before == trailer_after {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot record has a stable nonzero malformed commit trailer or committed body",
            ))
        } else {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot trailer changed during malformed-record read",
            ))
        }
    };
    if trailer_before.iter().all(|byte| *byte == 0) {
        return absent_if_zero_trailer_stable();
    }
    let trailer_region: TwoSlotCommitTrailerRegionV1 =
        match decode_two_slot_value(&trailer_before, "governance two-slot commit trailer") {
            Ok(trailer) => trailer,
            Err(_) => return corrupt_if_trailer_stable(),
        };
    let record_region: TwoSlotRecordHeaderRegionV1 =
        match decode_two_slot_value(&record_region, "governance two-slot record-header region") {
            Ok(record) => record,
            Err(_) => return corrupt_if_trailer_stable(),
        };
    let trailer = trailer_region.trailer;
    let header = record_region.header;
    let expected_slot =
        u8::try_from(slot_id).map_err(|_| io::Error::other("governance two-slot id exceeds u8"))?;
    if trailer_region.reserved != [0; TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1]
        || record_region.reserved != [0; TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1]
        || trailer.format_version != TWO_SLOT_FORMAT_VERSION_V1
        || header.format_version != TWO_SLOT_FORMAT_VERSION_V1
        || trailer.binding_digest != store.binding_digest
        || header.binding_digest != store.binding_digest
        || trailer.slot_id != expected_slot
        || header.slot_id != expected_slot
        || trailer.generation == 0
        || trailer.generation != header.generation
        || (header.generation == 1 && header.predecessor_digest != TWO_SLOT_ZERO_DIGEST)
        || (header.generation > 1 && header.predecessor_digest == TWO_SLOT_ZERO_DIGEST)
        || trailer.commit_marker != TWO_SLOT_COMMIT_MARKER_V1
    {
        return corrupt_if_trailer_stable();
    }
    let payload_len = match usize::try_from(header.payload_len) {
        Ok(payload_len) if payload_len <= store.config.max_payload_bytes => payload_len,
        _ => return corrupt_if_trailer_stable(),
    };
    let payload = read_exact_file_region(&slot.handle, store.layout.payload_offset, payload_len)?;
    let trailer_after = read_exact_file_region(
        &slot.handle,
        store.layout.trailer_offset,
        store.layout.commit_trailer_region_bytes,
    )?;
    if trailer_before != trailer_after {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot trailer changed while reading its record",
        ));
    }
    let record_digest = two_slot_record_digest(&header, &payload)?;
    if trailer.record_digest != record_digest
        || header.payload_digest != *blake3::hash(&payload).as_bytes()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot committed record or payload digest is invalid",
        ));
    }
    Ok(Some(TwoSlotCommittedRecordV1 {
        slot_id,
        generation: header.generation,
        predecessor_digest: header.predecessor_digest,
        record_digest,
        payload,
    }))
}

fn read_two_slot_record_stable(
    store: &TwoSlotStoreV1,
    slot_id: usize,
) -> io::Result<Option<TwoSlotCommittedRecordV1>> {
    const MAX_RETRIES: usize = 3;
    for _ in 0..MAX_RETRIES {
        match read_two_slot_record_once(store, slot_id) {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
            result => return result,
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "governance two-slot record did not stabilize during bounded read",
    ))
}

fn select_two_slot_record_unlocked(store: &TwoSlotStoreV1) -> io::Result<TwoSlotCommittedRecordV1> {
    verify_two_slot_headers(store)?;
    let left = read_two_slot_record_stable(store, 0)?;
    let right = read_two_slot_record_stable(store, 1)?;
    verify_two_slot_headers(store)?;
    match (left, right) {
        (None, None) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot store has no committed record",
        )),
        (Some(record), None) | (None, Some(record)) => Ok(record),
        (Some(left), Some(right)) => {
            if left.generation == right.generation {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot records have an ambiguous equal generation",
                ));
            }
            let (older, newer) = if left.generation < right.generation {
                (left, right)
            } else {
                (right, left)
            };
            if older.generation.checked_add(1) != Some(newer.generation)
                || newer.predecessor_digest != older.record_digest
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot records are nonconsecutive or have divergent lineage",
                ));
            }
            Ok(newer)
        }
    }
}

fn two_slot_snapshot(
    store: &TwoSlotStoreV1,
    record: TwoSlotCommittedRecordV1,
) -> TwoSlotSnapshotV1 {
    TwoSlotSnapshotV1 {
        domain: store.config.domain,
        store_nonce: store.config.store_nonce,
        max_payload_bytes: store.config.max_payload_bytes,
        binding_digest: store.binding_digest,
        generation: record.generation,
        record_digest: record.record_digest,
        payload: record.payload,
    }
}

struct TwoSlotOsLock<'file> {
    file: &'file File,
    locked: bool,
}

impl<'file> TwoSlotOsLock<'file> {
    fn acquire(file: &'file File) -> io::Result<Self> {
        File::lock(file)?;
        Ok(Self { file, locked: true })
    }

    fn release(mut self) -> io::Result<()> {
        let result = File::unlock(self.file);
        if result.is_ok() {
            self.locked = false;
        }
        result
    }
}

impl Drop for TwoSlotOsLock<'_> {
    fn drop(&mut self) {
        if self.locked {
            let _ = File::unlock(self.file);
        }
    }
}

struct TwoSlotInitFileLockV1 {
    root: RootedDirectory,
    name: OsString,
    handle: File,
    identity: FileIdentity,
    locked: bool,
}

impl TwoSlotInitFileLockV1 {
    fn acquire(root: &RootedDirectory, config: &TwoSlotStoreConfigV1) -> io::Result<Self> {
        let name = two_slot_init_lock_name(config);
        root.verify()?;
        let handle = match platform::open_read_write_file(&root.handle, &name) {
            Ok(handle) => handle,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match platform::create_file(&root.handle, &name) {
                    Ok(handle) => handle,
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        platform::open_read_write_file(&root.handle, &name)?
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        };
        let metadata = handle.metadata()?;
        let path = root.display_path.join(&name);
        validate_file_metadata(&path, &metadata, 0, true)?;
        if metadata.len() != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot init lock must remain an empty fixed file",
            ));
        }
        let identity = file_identity(&metadata)?;
        root.verify_file_binding(&name, &handle, identity, 0, true)?;
        // Every contender establishes the empty lock file and its parent
        // binding durably before it can serialize initialization. This covers
        // the race where a non-creator opens the name before the creator has
        // reached its parent-directory fsync.
        handle.sync_all()?;
        root.sync_all()?;
        File::lock(&handle)?;
        let lock = Self {
            root: root.clone(),
            name,
            handle,
            identity,
            locked: true,
        };
        lock.verify()?;
        Ok(lock)
    }

    fn verify(&self) -> io::Result<()> {
        let metadata = self.handle.metadata()?;
        let path = self.root.display_path.join(&self.name);
        validate_file_metadata(&path, &metadata, 0, true)?;
        if metadata.len() != 0 || file_identity(&metadata)? != self.identity {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot init lock changed identity or length",
            ));
        }
        self.root
            .verify_file_binding(&self.name, &self.handle, self.identity, 0, true)
    }

    fn release(mut self) -> io::Result<()> {
        let verification = self.verify();
        let unlock = File::unlock(&self.handle);
        if unlock.is_ok() {
            self.locked = false;
        }
        match (verification, unlock) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), _) | (Ok(()), Err(error)) => Err(error),
        }
    }
}

impl Drop for TwoSlotInitFileLockV1 {
    fn drop(&mut self) {
        if self.locked {
            let _ = File::unlock(&self.handle);
        }
    }
}

impl TwoSlotStoreV1 {
    fn with_exclusive_lock<ResultValue>(
        &self,
        operation: impl FnOnce(&Self) -> io::Result<ResultValue>,
    ) -> io::Result<ResultValue> {
        let process_guard = self
            .process_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let os_lock = TwoSlotOsLock::acquire(&self.slots[0].handle)?;
        verify_two_slot_headers(self)?;
        let result = operation(self);
        let unlock = os_lock.release();
        drop(process_guard);
        match (result, unlock) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    /// Load the highest complete record after strict pair and lineage checks.
    pub(super) fn load(&self) -> io::Result<TwoSlotSnapshotV1> {
        self.with_exclusive_lock(|store| {
            select_two_slot_record_unlocked(store).map(|record| two_slot_snapshot(store, record))
        })
    }

    /// Commit one direct successor of `expected`, or return an exact-byte no-op.
    pub(super) fn compare_and_swap(
        &self,
        expected: &TwoSlotSnapshotV1,
        payload: &[u8],
    ) -> io::Result<TwoSlotSnapshotV1> {
        self.compare_and_swap_with(expected, payload, |_| Ok(()))
    }

    fn compare_and_swap_with<Hook>(
        &self,
        expected: &TwoSlotSnapshotV1,
        payload: &[u8],
        mut after_step: Hook,
    ) -> io::Result<TwoSlotSnapshotV1>
    where
        Hook: FnMut(&'static str) -> io::Result<()>,
    {
        if payload.len() > self.config.max_payload_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot successor payload is outside its configured bound",
            ));
        }
        if expected.domain != self.config.domain
            || expected.store_nonce != self.config.store_nonce
            || expected.max_payload_bytes != self.config.max_payload_bytes
            || expected.binding_digest != self.binding_digest
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot predecessor belongs to another store or layout",
            ));
        }
        self.with_exclusive_lock(|store| {
            let current = select_two_slot_record_unlocked(store)?;
            if current.generation != expected.generation
                || current.record_digest != expected.record_digest
                || current.payload != expected.payload
            {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "governance two-slot compare-and-swap predecessor changed",
                ));
            }
            if current.payload == payload {
                return Ok(two_slot_snapshot(store, current));
            }
            let generation = current.generation.checked_add(1).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot generation exhausted",
                )
            })?;
            let inactive_id = 1_usize.checked_sub(current.slot_id).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance active slot id is invalid",
                )
            })?;
            verify_two_slot_headers(store)?;
            let active_before = read_two_slot_record_stable(store, current.slot_id)?
                .ok_or_else(|| io::Error::other("governance active slot disappeared"))?;
            if active_before != current {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "governance active two-slot record changed before commit",
                ));
            }
            let inactive = &store.slots[inactive_id];
            write_exact_file_region(
                &inactive.handle,
                store.layout.trailer_offset,
                &vec![0; store.layout.commit_trailer_region_bytes],
            )?;
            after_step("inactive-zero-trailer-written")?;
            inactive.handle.sync_all()?;
            after_step("inactive-trailer-invalidated")?;
            verify_two_slot_headers(store)?;

            let slot_id = u8::try_from(inactive_id)
                .map_err(|_| io::Error::other("governance inactive slot id exceeds u8"))?;
            let header = TwoSlotRecordHeaderV1 {
                format_version: TWO_SLOT_FORMAT_VERSION_V1,
                binding_digest: store.binding_digest,
                slot_id,
                generation,
                predecessor_digest: current.record_digest,
                payload_len: u64::try_from(payload.len()).map_err(|_| {
                    io::Error::other("governance two-slot payload length exceeds u64")
                })?,
                payload_digest: *blake3::hash(payload).as_bytes(),
            };
            let header_region = encode_two_slot_value(
                &TwoSlotRecordHeaderRegionV1 {
                    header: header.clone(),
                    reserved: [0; TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1],
                },
                "governance two-slot record-header region",
            )?;
            if header_region.len() != store.layout.record_header_region_bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot record-header layout changed",
                ));
            }
            write_exact_file_region(
                &inactive.handle,
                u64::try_from(store.layout.header_region_bytes).map_err(|_| {
                    io::Error::other("governance two-slot record offset exceeds u64")
                })?,
                &header_region,
            )?;
            // The authenticated length is the sole semantic payload boundary.
            // Bytes beyond it are private fixed-slot residue and are never
            // decoded, hashed, or returned; avoiding a full 192 MiB wipe keeps
            // short governance commits bounded by their actual payload size.
            write_exact_file_region(&inactive.handle, store.layout.payload_offset, payload)?;
            after_step("inactive-record-written")?;
            inactive.handle.sync_all()?;
            after_step("inactive-record-synced")?;
            verify_two_slot_headers(store)?;

            let record_digest = two_slot_record_digest(&header, payload)?;
            let trailer_region = encode_two_slot_value(
                &TwoSlotCommitTrailerRegionV1 {
                    trailer: TwoSlotCommitTrailerV1 {
                        format_version: TWO_SLOT_FORMAT_VERSION_V1,
                        binding_digest: store.binding_digest,
                        slot_id,
                        generation,
                        record_digest,
                        commit_marker: TWO_SLOT_COMMIT_MARKER_V1,
                    },
                    reserved: [0; TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1],
                },
                "governance two-slot commit-trailer region",
            )?;
            if trailer_region.len() != store.layout.commit_trailer_region_bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot commit-trailer layout changed",
                ));
            }
            write_exact_file_region(
                &inactive.handle,
                store.layout.trailer_offset,
                &trailer_region,
            )?;
            after_step("inactive-commit-trailer-written")?;
            inactive.handle.sync_all()?;
            after_step("inactive-commit-trailer-synced")?;
            verify_two_slot_headers(store)?;
            let active_after = read_two_slot_record_stable(store, current.slot_id)?
                .ok_or_else(|| io::Error::other("governance active slot became invalid"))?;
            if active_after != current {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "governance active slot changed during successor commit",
                ));
            }
            let selected = select_two_slot_record_unlocked(store)?;
            if selected.slot_id != inactive_id
                || selected.generation != generation
                || selected.record_digest != record_digest
                || selected.payload != payload
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot durable successor readback diverged",
                ));
            }
            after_step("successor-readback-verified")?;
            Ok(two_slot_snapshot(store, selected))
        })
    }

    #[cfg(test)]
    fn compare_and_swap_with_test_hook<Hook>(
        &self,
        expected: &TwoSlotSnapshotV1,
        payload: &[u8],
        after_step: Hook,
    ) -> io::Result<TwoSlotSnapshotV1>
    where
        Hook: FnMut(&'static str) -> io::Result<()>,
    {
        self.compare_and_swap_with(expected, payload, after_step)
    }
}

fn two_slot_stage_prefix(config: &TwoSlotStoreConfigV1) -> String {
    format!(
        ".iroha-two-slot-{}-stage-v1-",
        two_slot_store_namespace(config)
    )
}

fn two_slot_lost_found_name(config: &TwoSlotStoreConfigV1) -> OsString {
    format!(
        ".iroha-two-slot-{}-lost-found-v1",
        two_slot_store_namespace(config)
    )
    .into()
}

fn two_slot_init_lock_name(config: &TwoSlotStoreConfigV1) -> OsString {
    format!(
        ".iroha-two-slot-{}-init-lock-v1",
        two_slot_store_namespace(config)
    )
    .into()
}

fn is_canonical_two_slot_stage_name(name: &OsStr, prefix: &str) -> bool {
    fn is_lower_hex(byte: u8) -> bool {
        byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
    }

    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(suffix) = name.strip_prefix(prefix) else {
        return false;
    };
    let Some((process, sequence)) = suffix.split_once('-') else {
        return false;
    };
    process.len() == 16
        && sequence.len() == 16
        && process.bytes().all(is_lower_hex)
        && sequence.bytes().all(is_lower_hex)
}

fn is_canonical_two_slot_lost_found_entry(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(index) = name.strip_prefix("entry-v1-") else {
        return false;
    };
    index.len() == 4 && index.bytes().all(|byte| byte.is_ascii_digit())
}

fn two_slot_stage_inventory(
    directory: &RootedDirectory,
    layout: TwoSlotLayoutV1,
) -> io::Result<TwoSlotStageInventoryV1> {
    let names = directory.child_names_bounded(TWO_SLOT_NAMES_V1.len() + 1)?;
    let max_bytes = two_slot_file_byte_limit(layout)?;
    let mut seen = [false; TWO_SLOT_NAMES_V1.len()];
    let mut byte_count = 0_u64;
    let mut canonical_header_count = 0_usize;
    for name in names {
        let Some(slot_id) = TWO_SLOT_NAMES_V1
            .iter()
            .position(|expected| name == OsStr::new(expected))
        else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance two-slot recovery directory `{}` contains a non-slot entry",
                    directory.display_path.display()
                ),
            ));
        };
        if seen[slot_id] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot recovery inventory contains a duplicate slot name",
            ));
        }
        seen[slot_id] = true;
        let handle = platform::open_read_write_file(&directory.handle, &name)?;
        let metadata = handle.metadata()?;
        let path = directory.display_path.join(&name);
        validate_file_metadata(&path, &metadata, max_bytes, true)?;
        let identity = file_identity(&metadata)?;
        directory.verify_file_binding(&name, &handle, identity, max_bytes, true)?;
        byte_count = byte_count.checked_add(metadata.len()).ok_or_else(|| {
            io::Error::other("governance two-slot recovery byte count overflowed")
        })?;
        if metadata.len() == layout.slot_file_bytes {
            let encoded = read_exact_file_region(&handle, 0, layout.header_region_bytes)?;
            if let Ok(region) = decode_two_slot_value::<TwoSlotHeaderRegionV1>(
                &encoded,
                "governance two-slot recovery header",
            ) && region.reserved == [0; TWO_SLOT_HEADER_RESERVED_BYTES_V1]
                && region.header.binding.format_version == TWO_SLOT_FORMAT_VERSION_V1
                && region.header.slot_id == u8::try_from(slot_id).unwrap_or(u8::MAX)
            {
                canonical_header_count =
                    canonical_header_count.checked_add(1).ok_or_else(|| {
                        io::Error::other("governance two-slot header count overflowed")
                    })?;
            }
        }
        directory.verify_file_binding(&name, &handle, identity, max_bytes, true)?;
    }
    directory.verify()?;
    Ok(TwoSlotStageInventoryV1 {
        byte_count,
        has_full_pair: seen.into_iter().all(|present| present),
        canonical_header_count,
    })
}

fn two_slot_initial_stage_is_complete(
    store: &TwoSlotStoreV1,
    initial_payload: &[u8],
) -> io::Result<bool> {
    store.with_exclusive_lock(|store| {
        verify_two_slot_headers(store)?;
        let left = read_two_slot_record_stable(store, 0)?;
        let right = read_two_slot_record_stable(store, 1)?;
        match (left, right) {
            (None, None) => Ok(false),
            (Some(record), None)
                if record.slot_id == 0
                    && record.generation == 1
                    && record.predecessor_digest == TWO_SLOT_ZERO_DIGEST
                    && record.payload == initial_payload =>
            {
                Ok(true)
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot stage contains a divergent committed history",
            )),
        }
    })
}

fn classify_two_slot_stage(
    name: OsString,
    directory: RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
    initial_payload: &[u8],
) -> io::Result<TwoSlotStageV1> {
    let layout = two_slot_layout(config.max_payload_bytes)?;
    let inventory = two_slot_stage_inventory(&directory, layout)?;
    let complete = match open_existing_two_slot_store(
        directory.clone(),
        config.clone(),
        init_lock_identity,
    ) {
        Ok(store) => two_slot_initial_stage_is_complete(&store, initial_payload)?,
        Err(error) => {
            if inventory.has_full_pair && inventory.canonical_header_count == 2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "governance two-slot stage `{}` has complete typed headers but a divergent binding: {error}",
                        directory.display_path.display()
                    ),
                ));
            }
            false
        }
    };
    Ok(TwoSlotStageV1 {
        name,
        directory,
        byte_count: inventory.byte_count,
        complete,
    })
}

fn collect_two_slot_stages(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
    initial_payload: &[u8],
) -> io::Result<Vec<TwoSlotStageV1>> {
    let prefix = two_slot_stage_prefix(config);
    let mut stage_names = Vec::new();
    for name in root.child_names_bounded(DEFAULT_CHILD_ENTRY_LIMIT)? {
        if !name.as_encoded_bytes().starts_with(prefix.as_bytes()) {
            continue;
        }
        if !is_canonical_two_slot_stage_name(&name, &prefix) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance two-slot stage name `{}` is noncanonical",
                    name.to_string_lossy()
                ),
            ));
        }
        stage_names.push(name);
        if stage_names.len() > TWO_SLOT_STAGE_ENTRY_HARD_CAP_V1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot staging entry cap is exceeded",
            ));
        }
    }
    stage_names.sort();
    stage_names
        .into_iter()
        .map(|name| {
            let directory = root.open_directory(&name)?;
            classify_two_slot_stage(name, directory, config, init_lock_identity, initial_payload)
        })
        .collect()
}

fn open_or_create_two_slot_lost_found(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
) -> io::Result<RootedDirectory> {
    let name = two_slot_lost_found_name(config);
    match root.open_directory(&name) {
        Ok(directory) => Ok(directory),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match root.create_child_directory_exclusive(&name) {
                Ok(directory) => {
                    root.sync_all()?;
                    Ok(directory)
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    root.open_directory(&name)
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

fn two_slot_lost_found_state(
    directory: &RootedDirectory,
    layout: TwoSlotLayoutV1,
) -> io::Result<([bool; TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1], usize, u64)> {
    let names = directory.child_names_bounded(TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1)?;
    let mut occupied = [false; TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1];
    let mut total_bytes = 0_u64;
    for name in names {
        if !is_canonical_two_slot_lost_found_entry(&name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance two-slot lost+found entry `{}` is noncanonical",
                    name.to_string_lossy()
                ),
            ));
        }
        let index = name
            .to_str()
            .and_then(|name| name.strip_prefix("entry-v1-"))
            .and_then(|index| index.parse::<usize>().ok())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot lost+found index is invalid",
                )
            })?;
        if index >= occupied.len() || occupied[index] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot lost+found index is duplicated or out of bounds",
            ));
        }
        occupied[index] = true;
        let child = directory.open_directory(&name)?;
        let inventory = two_slot_stage_inventory(&child, layout)?;
        total_bytes = total_bytes
            .checked_add(inventory.byte_count)
            .ok_or_else(|| io::Error::other("governance lost+found byte count overflowed"))?;
        if total_bytes > TWO_SLOT_LOST_FOUND_TOTAL_MAX_BYTES_V1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot lost+found byte cap is exceeded",
            ));
        }
    }
    let entry_count = occupied.iter().filter(|occupied| **occupied).count();
    directory.verify()?;
    Ok((occupied, entry_count, total_bytes))
}

fn quarantine_two_slot_stages(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    stages: Vec<TwoSlotStageV1>,
) -> io::Result<bool> {
    if stages.is_empty() {
        return Ok(true);
    }
    let layout = two_slot_layout(config.max_payload_bytes)?;
    let lost_found = open_or_create_two_slot_lost_found(root, config)?;
    let (mut occupied, entry_count, existing_bytes) =
        two_slot_lost_found_state(&lost_found, layout)?;
    let required_bytes = stages.iter().try_fold(0_u64, |total, stage| {
        total
            .checked_add(stage.byte_count)
            .ok_or_else(|| io::Error::other("governance two-slot quarantine byte count overflowed"))
    })?;
    if entry_count
        .checked_add(stages.len())
        .is_none_or(|count| count > TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1)
        || existing_bytes
            .checked_add(required_bytes)
            .is_none_or(|bytes| bytes > TWO_SLOT_LOST_FOUND_TOTAL_MAX_BYTES_V1)
    {
        return Ok(false);
    }
    for stage in stages {
        let inventory = two_slot_stage_inventory(&stage.directory, layout)?;
        if inventory.byte_count != stage.byte_count {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot stage changed before quarantine",
            ));
        }
        let index = occupied
            .iter()
            .position(|occupied| !*occupied)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "governance two-slot lost+found has no free bounded entry",
                )
            })?;
        let destination_name = OsString::from(format!("entry-v1-{index:04}"));
        root.move_child_directory_exclusive(
            stage.directory.clone(),
            &lost_found,
            &destination_name,
        )?;
        root.sync_all()?;
        lost_found.sync_all()?;
        occupied[index] = true;
    }
    Ok(true)
}

fn create_unique_two_slot_stage(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
) -> io::Result<(OsString, RootedDirectory)> {
    let prefix = two_slot_stage_prefix(config);
    let existing = root
        .child_names_bounded(DEFAULT_CHILD_ENTRY_LIMIT)?
        .into_iter()
        .filter(|name| name.as_encoded_bytes().starts_with(prefix.as_bytes()))
        .count();
    if existing >= TWO_SLOT_STAGE_ENTRY_HARD_CAP_V1 {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot staging entry cap is exhausted",
        ));
    }
    for _ in 0..TWO_SLOT_STAGE_ENTRY_HARD_CAP_V1 {
        let sequence = TWO_SLOT_STAGE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = OsString::from(format!(
            "{prefix}{:016x}-{sequence:016x}",
            u64::from(std::process::id())
        ));
        match root.create_child_directory_exclusive(&name) {
            Ok(directory) => return Ok((name, directory)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "governance two-slot could not allocate a unique bounded staging name",
    ))
}

fn create_two_slot_file<Hook>(
    directory: &RootedDirectory,
    name: &OsStr,
    layout: TwoSlotLayoutV1,
    labels: [&'static str; 2],
    after_step: &mut Hook,
) -> io::Result<TwoSlotFileV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    let handle = platform::create_file(&directory.handle, name)?;
    let path = directory.display_path.join(name);
    let max_bytes = two_slot_file_byte_limit(layout)?;
    let before = handle.metadata()?;
    validate_file_metadata(&path, &before, max_bytes, true)?;
    if before.len() != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "new governance two-slot file was not empty",
        ));
    }
    let identity = file_identity(&before)?;
    directory.verify_file_binding(name, &handle, identity, max_bytes, true)?;
    after_step(labels[0])?;
    handle.set_len(layout.slot_file_bytes)?;
    let after = handle.metadata()?;
    validate_file_metadata(&path, &after, max_bytes, true)?;
    if after.len() != layout.slot_file_bytes || file_identity(&after)? != identity {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "new governance two-slot file changed while being sized",
        ));
    }
    directory.verify_file_binding(name, &handle, identity, max_bytes, true)?;
    after_step(labels[1])?;
    Ok(TwoSlotFileV1 {
        handle: Arc::new(handle),
        identity,
        name: name.to_os_string(),
    })
}

fn write_two_slot_record_unlocked<Hook>(
    store: &TwoSlotStoreV1,
    slot_id: usize,
    generation: u64,
    predecessor_digest: [u8; 32],
    payload: &[u8],
    labels: [&'static str; 6],
    after_step: &mut Hook,
) -> io::Result<TwoSlotCommittedRecordV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    if generation == 0
        || (generation == 1 && predecessor_digest != TWO_SLOT_ZERO_DIGEST)
        || (generation > 1 && predecessor_digest == TWO_SLOT_ZERO_DIGEST)
        || payload.len() > store.config.max_payload_bytes
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance two-slot record generation, lineage, or payload is invalid",
        ));
    }
    let slot = store.slots.get(slot_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance two-slot id is invalid",
        )
    })?;
    verify_two_slot_headers(store)?;
    write_exact_file_region(
        &slot.handle,
        store.layout.trailer_offset,
        &vec![0; store.layout.commit_trailer_region_bytes],
    )?;
    after_step(labels[0])?;
    slot.handle.sync_all()?;
    after_step(labels[1])?;
    verify_two_slot_headers(store)?;

    let encoded_slot_id =
        u8::try_from(slot_id).map_err(|_| io::Error::other("governance two-slot id exceeds u8"))?;
    let header = TwoSlotRecordHeaderV1 {
        format_version: TWO_SLOT_FORMAT_VERSION_V1,
        binding_digest: store.binding_digest,
        slot_id: encoded_slot_id,
        generation,
        predecessor_digest,
        payload_len: u64::try_from(payload.len())
            .map_err(|_| io::Error::other("governance two-slot payload exceeds u64"))?,
        payload_digest: *blake3::hash(payload).as_bytes(),
    };
    let header_region = encode_two_slot_value(
        &TwoSlotRecordHeaderRegionV1 {
            header: header.clone(),
            reserved: [0; TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1],
        },
        "governance two-slot record-header region",
    )?;
    if header_region.len() != store.layout.record_header_region_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot record-header layout changed",
        ));
    }
    let record_offset = u64::try_from(store.layout.header_region_bytes)
        .map_err(|_| io::Error::other("governance two-slot record offset exceeds u64"))?;
    write_exact_file_region(&slot.handle, record_offset, &header_region)?;
    write_exact_file_region(&slot.handle, store.layout.payload_offset, payload)?;
    after_step(labels[2])?;
    slot.handle.sync_all()?;
    after_step(labels[3])?;
    verify_two_slot_headers(store)?;

    let record_digest = two_slot_record_digest(&header, payload)?;
    let trailer_region = encode_two_slot_value(
        &TwoSlotCommitTrailerRegionV1 {
            trailer: TwoSlotCommitTrailerV1 {
                format_version: TWO_SLOT_FORMAT_VERSION_V1,
                binding_digest: store.binding_digest,
                slot_id: encoded_slot_id,
                generation,
                record_digest,
                commit_marker: TWO_SLOT_COMMIT_MARKER_V1,
            },
            reserved: [0; TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1],
        },
        "governance two-slot commit-trailer region",
    )?;
    if trailer_region.len() != store.layout.commit_trailer_region_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot commit-trailer layout changed",
        ));
    }
    write_exact_file_region(&slot.handle, store.layout.trailer_offset, &trailer_region)?;
    after_step(labels[4])?;
    slot.handle.sync_all()?;
    after_step(labels[5])?;
    verify_two_slot_headers(store)?;
    let committed = read_two_slot_record_stable(store, slot_id)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot committed record failed exact readback",
        )
    })?;
    if committed.generation != generation
        || committed.predecessor_digest != predecessor_digest
        || committed.record_digest != record_digest
        || committed.payload != payload
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot committed record readback diverged",
        ));
    }
    Ok(committed)
}

fn initialize_two_slot_stage<Hook>(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
    initial_payload: &[u8],
    after_step: &mut Hook,
) -> io::Result<TwoSlotStageV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    let layout = two_slot_layout(config.max_payload_bytes)?;
    let (name, directory) = create_unique_two_slot_stage(root, config)?;
    after_step("stage-directory-created")?;
    root.sync_all()?;
    after_step("stage-parent-synced")?;

    let slot_0 = create_two_slot_file(
        &directory,
        OsStr::new(TWO_SLOT_NAMES_V1[0]),
        layout,
        ["slot-0-created", "slot-0-sized"],
        after_step,
    )?;
    slot_0.handle.sync_all()?;
    after_step("slot-0-sized-and-synced")?;
    let slot_1 = create_two_slot_file(
        &directory,
        OsStr::new(TWO_SLOT_NAMES_V1[1]),
        layout,
        ["slot-1-created", "slot-1-sized"],
        after_step,
    )?;
    slot_1.handle.sync_all()?;
    after_step("slot-1-sized-and-synced")?;

    let material = two_slot_binding_material(
        config,
        layout,
        init_lock_identity,
        [slot_0.identity, slot_1.identity],
    )?;
    let binding_digest = two_slot_binding_digest(&material)?;
    for (slot_id, slot) in [&slot_0, &slot_1].into_iter().enumerate() {
        let header = expected_two_slot_header_region(&material, binding_digest, slot_id)?;
        if header.len() != layout.header_region_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot immutable header layout changed",
            ));
        }
        write_exact_file_region(&slot.handle, 0, &header)?;
        after_step(if slot_id == 0 {
            "slot-0-header-written"
        } else {
            "slot-1-header-written"
        })?;
        slot.handle.sync_all()?;
        after_step(if slot_id == 0 {
            "slot-0-header-synced"
        } else {
            "slot-1-header-synced"
        })?;
    }
    let store = TwoSlotStoreV1 {
        directory: directory.clone(),
        config: config.clone(),
        layout,
        init_lock_identity,
        binding_digest,
        slots: [slot_0, slot_1],
        process_lock: Arc::new(Mutex::new(())),
    };
    write_two_slot_record_unlocked(
        &store,
        0,
        1,
        TWO_SLOT_ZERO_DIGEST,
        initial_payload,
        [
            "initial-trailer-invalidated",
            "initial-trailer-invalidation-synced",
            "initial-record-written",
            "initial-record-synced",
            "initial-commit-trailer-written",
            "initial-commit-trailer-synced",
        ],
        after_step,
    )?;
    after_step("initial-record-readback-verified")?;
    directory.sync_all()?;
    after_step("stage-directory-synced")?;
    let stage =
        classify_two_slot_stage(name, directory, config, init_lock_identity, initial_payload)?;
    if !stage.complete {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "new governance two-slot stage is not complete after durable initialization",
        ));
    }
    Ok(stage)
}

fn promote_two_slot_stage<Hook>(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
    initial_payload: &[u8],
    stage: TwoSlotStageV1,
    after_step: &mut Hook,
) -> io::Result<TwoSlotStoreV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    let stage = classify_two_slot_stage(
        stage.name,
        stage.directory,
        config,
        init_lock_identity,
        initial_payload,
    )?;
    if !stage.complete {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot stage became incomplete before promotion",
        ));
    }
    after_step("before-directory-rename")?;
    let stage = classify_two_slot_stage(
        stage.name,
        stage.directory,
        config,
        init_lock_identity,
        initial_payload,
    )?;
    if !stage.complete {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot stage changed at the promotion boundary",
        ));
    }
    let installed =
        root.move_child_directory_exclusive(stage.directory, root, OsStr::new(&config.store_name))?;
    after_step("directory-renamed")?;
    root.sync_all()?;
    after_step("parent-synced")?;
    let store = open_existing_two_slot_store(installed, config.clone(), init_lock_identity)?;
    if !two_slot_initial_stage_is_complete(&store, initial_payload)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "promoted governance two-slot store lost its initial record",
        ));
    }
    after_step("initialization-postcheck")?;
    Ok(store)
}

fn open_valid_two_slot_canonical(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
) -> io::Result<Option<TwoSlotStoreV1>> {
    match root.open_directory(OsStr::new(&config.store_name)) {
        Ok(directory) => {
            let store =
                open_existing_two_slot_store(directory, config.clone(), init_lock_identity)?;
            let _ = store.load()?;
            Ok(Some(store))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn load_existing_two_slot_store_v1(
    root: &RootedDirectory,
    config: TwoSlotStoreConfigV1,
) -> io::Result<TwoSlotSnapshotV1> {
    let store = open_existing_read_only_two_slot_store_v1(root, config)?;
    let snapshot = store.load()?;
    root.verify()?;
    Ok(snapshot)
}

fn open_existing_read_only_two_slot_store_v1(
    root: &RootedDirectory,
    config: TwoSlotStoreConfigV1,
) -> io::Result<TwoSlotStoreV1> {
    if root.writable {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "existing governance two-slot reads require a read-only rooted capability",
        ));
    }
    root.verify()?;
    let init_lock_name = two_slot_init_lock_name(&config);
    let init_lock = platform::open_file(&root.handle, &init_lock_name, false)?;
    let init_lock_metadata = init_lock.metadata()?;
    let init_lock_path = root.display_path.join(&init_lock_name);
    validate_file_metadata(&init_lock_path, &init_lock_metadata, 0, true)?;
    if init_lock_metadata.len() != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot init lock must remain an empty fixed file",
        ));
    }
    let init_lock_identity = file_identity(&init_lock_metadata)?;
    root.verify_file_binding(&init_lock_name, &init_lock, init_lock_identity, 0, true)?;
    let directory = root.open_directory(OsStr::new(&config.store_name))?;
    let store = open_existing_two_slot_store(directory, config, init_lock_identity)?;
    let _ = store.load()?;
    root.verify_file_binding(&init_lock_name, &init_lock, init_lock_identity, 0, true)?;
    root.verify()?;
    Ok(store)
}

fn open_or_create_two_slot_store_v1_once<Hook>(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
    initial_payload: &[u8],
    after_step: &mut Hook,
) -> io::Result<TwoSlotStoreV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    root.verify()?;
    if let Some(store) = open_valid_two_slot_canonical(root, config, init_lock_identity)? {
        let stages = collect_two_slot_stages(root, config, init_lock_identity, initial_payload)?;
        // A valid canonical store remains available even when bounded
        // preservation space is full. Every stage remains untouched for
        // offline archival, while divergent stages still fail during the
        // classification above.
        let _all_preserved = quarantine_two_slot_stages(root, config, stages)?;
        return Ok(store);
    }

    let mut stages = collect_two_slot_stages(root, config, init_lock_identity, initial_payload)?;
    if let Some(index) = stages.iter().position(|stage| stage.complete) {
        let selected = stages.remove(index);
        if !quarantine_two_slot_stages(root, config, stages)? {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot lost+found capacity is exhausted; archive it offline",
            ));
        }
        return promote_two_slot_stage(
            root,
            config,
            init_lock_identity,
            initial_payload,
            selected,
            after_step,
        );
    }
    if !quarantine_two_slot_stages(root, config, stages)? {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot lost+found capacity is exhausted; archive it offline",
        ));
    }
    let stage = initialize_two_slot_stage(
        root,
        config,
        init_lock_identity,
        initial_payload,
        after_step,
    )?;
    promote_two_slot_stage(
        root,
        config,
        init_lock_identity,
        initial_payload,
        stage,
        after_step,
    )
}

fn open_or_create_two_slot_store_v1_with<Hook>(
    root: &RootedDirectory,
    config: TwoSlotStoreConfigV1,
    initial_payload: &[u8],
    mut after_step: Hook,
) -> io::Result<TwoSlotStoreV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    if !root.writable {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "read-only governance directory cannot open a mutable two-slot store",
        ));
    }
    if initial_payload.len() > config.max_payload_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance two-slot initial payload exceeds its configured bound",
        ));
    }
    let init_file_lock = TwoSlotInitFileLockV1::acquire(root, &config)?;
    const RACE_RETRIES: usize = 4;
    let result = (|| {
        for attempt in 0..RACE_RETRIES {
            init_file_lock.verify()?;
            match open_or_create_two_slot_store_v1_once(
                root,
                &config,
                init_file_lock.identity,
                initial_payload,
                &mut after_step,
            ) {
                Err(error)
                    if attempt + 1 < RACE_RETRIES
                        && matches!(
                            error.kind(),
                            io::ErrorKind::AlreadyExists
                                | io::ErrorKind::NotFound
                                | io::ErrorKind::WouldBlock
                        ) =>
                {
                    continue;
                }
                result => return result,
            }
        }
        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot initialization race did not converge",
        ))
    })();
    let unlock = init_file_lock.release();
    match (result, unlock) {
        (Ok(store), Ok(())) => Ok(store),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}

fn verify_expected_file(
    directory: &RootedDirectory,
    name: &OsStr,
    expected: &ExpectedFile,
) -> io::Result<()> {
    if let ExpectedFile::Identity(binding) = expected {
        binding.verify().map_err(|error| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                format!("governance atomic predecessor binding changed: {error}"),
            )
        })?;
    }
    match (expected, directory.file_identity(name)?) {
        (ExpectedFile::Missing, None) => Ok(()),
        (ExpectedFile::Identity(expected), Some(actual)) if expected.identity() == actual => Ok(()),
        _ => Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "governance atomic target `{}` changed before promotion",
                directory.display_path.join(name).display()
            ),
        )),
    }
}

fn validate_component(name: &OsStr) -> io::Result<()> {
    if name.is_empty() || name == OsStr::new(".") || name == OsStr::new("..") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance path component is empty or relative",
        ));
    }
    platform::validate_component(name)
}

fn validate_directory_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::other(format!(
            "governance directory `{}` must be a real directory",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.mode() & 0o022 != 0 {
        return Err(io::Error::other(format!(
            "governance directory `{}` must not be group/world writable",
            path.display()
        )));
    }
    platform::validate_non_reparse(metadata)?;
    let _ = file_identity(metadata)?;
    Ok(())
}

fn validate_regular_file_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "governance state `{}` must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(io::Error::other(format!(
            "governance state `{}` must have exactly one hard link",
            path.display()
        )));
    }
    #[cfg(windows)]
    if metadata.number_of_links() != Some(1) {
        return Err(io::Error::other(format!(
            "governance state `{}` must have exactly one hard link",
            path.display()
        )));
    }
    platform::validate_non_reparse(metadata)?;
    let _ = file_identity(metadata)?;
    Ok(())
}

fn validate_private_regular_file_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    validate_regular_file_metadata(path, metadata)?;
    #[cfg(unix)]
    {
        let effective_uid = unsafe { geteuid() };
        if metadata.uid() != effective_uid || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "private governance state `{}` must be owned by UID {effective_uid} and mode 0600 or stricter",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

fn validate_file_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    max_bytes: usize,
    private: bool,
) -> io::Result<()> {
    if private {
        validate_private_regular_file_metadata(path, metadata)?;
    } else {
        validate_regular_file_metadata(path, metadata)?;
    }
    let max_bytes = u64::try_from(max_bytes)
        .map_err(|_| io::Error::other("governance file byte limit exceeds u64"))?;
    if metadata.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance state `{}` exceeds {max_bytes} bytes",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn file_identity(metadata: &fs::Metadata) -> io::Result<FileIdentity> {
    #[cfg(unix)]
    {
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(windows)]
    {
        Ok(FileIdentity {
            volume_serial_number: metadata.volume_serial_number().ok_or_else(|| {
                io::Error::other("Windows governance object lacks a volume serial number")
            })?,
            file_index: metadata
                .file_index()
                .ok_or_else(|| io::Error::other("Windows governance object lacks a file index"))?,
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        Err(platform::unsupported())
    }
}

#[cfg(unix)]
fn metadata_stable_during_read(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    file_identity(before).ok() == file_identity(after).ok()
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
}

#[cfg(windows)]
fn metadata_stable_during_read(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    file_identity(before).ok() == file_identity(after).ok()
        && before.len() == after.len()
        && before.last_write_time() == after.last_write_time()
        && before.creation_time() == after.creation_time()
}

#[cfg(not(any(unix, windows)))]
fn metadata_stable_during_read(_before: &fs::Metadata, _after: &fs::Metadata) -> bool {
    false
}

/// Return whether a name claims the legacy atomic-temporary namespace for
/// `target`, including malformed names that require offline inspection.
pub(super) fn is_atomic_temp_candidate_for(name: &str, target: &str) -> bool {
    name.strip_prefix('.')
        .and_then(|name| name.strip_prefix(target))
        .is_some_and(|suffix| suffix.starts_with(".tmp-"))
}

#[cfg(any(windows, test))]
fn atomic_temp_target_name(name: &str) -> Option<&str> {
    let name = name.strip_prefix('.')?;
    let (target_name, suffix) = name.rsplit_once(".tmp-")?;
    if target_name.is_empty() {
        return None;
    }
    let Some((pid, counter)) = suffix.split_once('-') else {
        return None;
    };
    if pid.is_empty()
        || counter.is_empty()
        || !pid.bytes().all(|byte| byte.is_ascii_digit())
        || !counter.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some(target_name)
}

/// Return one bounded V1 sibling slot used to retain an exact predecessor.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn atomic_retained_name(name: &OsStr, slot: usize) -> io::Result<OsString> {
    if slot >= ATOMIC_RETAINED_SLOT_COUNT_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance atomic retained-generation slot exceeds the V1 bound",
        ));
    }
    let name = name.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance atomic retention target is not canonical UTF-8",
        )
    })?;
    validate_component(OsStr::new(name))?;
    let retained = OsString::from(format!(".{name}{ATOMIC_RETAINED_SUFFIX_V1}{slot:04}"));
    validate_component(&retained)?;
    Ok(retained)
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn atomic_retained_target_and_slot(name: &str) -> Option<(&str, usize)> {
    let name = name.strip_prefix('.')?;
    let (target, slot) = name.rsplit_once(ATOMIC_RETAINED_SUFFIX_V1)?;
    if target.is_empty()
        || slot.len() != ATOMIC_RETAINED_SLOT_WIDTH_V1
        || !slot.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let slot = slot.parse::<usize>().ok()?;
    (slot < ATOMIC_RETAINED_SLOT_COUNT_V1).then_some((target, slot))
}

/// Return whether a name claims the V1 retained namespace for `target`.
pub(super) fn is_atomic_retained_candidate_for(name: &str, target: &str) -> bool {
    name.strip_prefix('.')
        .and_then(|name| name.strip_prefix(target))
        .is_some_and(|suffix| suffix.starts_with(ATOMIC_RETAINED_SUFFIX_V1))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod platform {
    #[cfg(target_os = "macos")]
    use std::os::raw::c_void;
    use std::{
        ffi::{CString, OsStr, OsString},
        fs::{self, File},
        io,
        os::{
            fd::{AsRawFd, FromRawFd, RawFd},
            raw::{c_char, c_int, c_uint},
            unix::ffi::OsStrExt as _,
        },
        path::Path,
    };

    #[cfg(test)]
    use super::FileIdentity;
    use super::{RootedDirectory, file_identity};

    #[cfg(target_os = "linux")]
    const O_CREATE: c_int = 0x40;
    #[cfg(target_os = "macos")]
    const O_CREATE: c_int = 0x200;
    #[cfg(target_os = "linux")]
    const O_EXCLUSIVE: c_int = 0x80;
    #[cfg(target_os = "macos")]
    const O_EXCLUSIVE: c_int = 0x800;
    #[cfg(target_os = "linux")]
    const O_CLOSE_ON_EXEC: c_int = 0x8_0000;
    #[cfg(target_os = "macos")]
    const O_CLOSE_ON_EXEC: c_int = 0x100_0000;
    #[cfg(target_os = "linux")]
    const O_NO_FOLLOW: c_int = 0x2_0000;
    #[cfg(target_os = "macos")]
    const O_NO_FOLLOW: c_int = 0x100;
    #[cfg(target_os = "linux")]
    const O_DIRECTORY: c_int = 0x1_0000;
    #[cfg(target_os = "macos")]
    const O_DIRECTORY: c_int = 0x10_0000;
    const O_READ_ONLY: c_int = 0;
    const O_READ_WRITE: c_int = 2;
    #[cfg(all(test, target_os = "linux"))]
    const AT_REMOVE_DIRECTORY: c_int = 0x200;
    #[cfg(all(test, target_os = "macos"))]
    const AT_REMOVE_DIRECTORY: c_int = 0x80;
    #[cfg(target_os = "linux")]
    const RENAME_NOREPLACE: c_uint = 1;
    #[cfg(target_os = "linux")]
    const RENAME_EXCHANGE: c_uint = 2;
    #[cfg(target_os = "macos")]
    const RENAME_EXCL: c_uint = 0x0000_0004;
    #[cfg(target_os = "macos")]
    const RENAME_SWAP: c_uint = 0x0000_0002;

    unsafe extern "C" {
        fn openat(directory: c_int, path: *const c_char, flags: c_int, ...) -> c_int;
        fn mkdirat(directory: c_int, path: *const c_char, mode: c_uint) -> c_int;
        #[cfg(test)]
        fn unlinkat(directory: c_int, path: *const c_char, flags: c_int) -> c_int;
    }

    #[cfg(target_os = "linux")]
    unsafe extern "C" {
        fn renameat2(
            source_directory: c_int,
            source: *const c_char,
            destination_directory: c_int,
            destination: *const c_char,
            flags: c_uint,
        ) -> c_int;
    }

    #[cfg(target_os = "macos")]
    unsafe extern "C" {
        fn renameatx_np(
            source_directory: c_int,
            source: *const c_char,
            destination_directory: c_int,
            destination: *const c_char,
            flags: c_uint,
        ) -> c_int;
        fn acl_get_fd_np(fd: c_int, acl_type: c_int) -> *mut c_void;
        fn acl_get_entry(acl: *mut c_void, entry_id: c_int, entry: *mut *mut c_void) -> c_int;
        fn acl_get_tag_type(entry: *mut c_void, tag_type: *mut c_int) -> c_int;
        fn acl_get_permset_mask_np(entry: *mut c_void, mask: *mut u64) -> c_int;
        fn acl_free(value: *mut c_void) -> c_int;
    }

    #[cfg(target_os = "linux")]
    unsafe extern "C" {
        fn flistxattr(fd: c_int, list: *mut c_char, size: usize) -> isize;
    }

    pub(super) fn ensure_supported() -> io::Result<()> {
        Ok(())
    }

    pub(super) fn validate_component(name: &OsStr) -> io::Result<()> {
        let bytes = name.as_bytes();
        if bytes.contains(&0) || bytes.contains(&b'/') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance path component contains a separator or NUL",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_non_reparse(_metadata: &fs::Metadata) -> io::Result<()> {
        Ok(())
    }

    pub(super) fn validate_directory_acl(handle: &File, path: &Path) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            validate_linux_directory_acl(handle, path)
        }
        #[cfg(target_os = "macos")]
        {
            validate_macos_directory_acl(handle, path)
        }
    }

    #[cfg(target_os = "linux")]
    fn validate_linux_directory_acl(handle: &File, path: &Path) -> io::Result<()> {
        let names = super::stable_linux_acl_attribute_names(path, || {
            read_linux_xattr_names_once(handle, path)
        })?;
        validate_linux_acl_attribute_names(&names, path)
    }

    #[cfg(target_os = "linux")]
    fn read_linux_xattr_names_once(handle: &File, path: &Path) -> io::Result<Option<Vec<u8>>> {
        const MAX_XATTR_LIST_BYTES: usize = 64 * 1024;
        const ERANGE: i32 = 34;

        // SAFETY: the descriptor is retained and a null buffer with zero size
        // requests the exact descriptor-bound xattr-list length.
        let required = unsafe { flistxattr(handle.as_raw_fd(), std::ptr::null_mut(), 0) };
        if required < 0 {
            let error = io::Error::last_os_error();
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "cannot inspect descriptor-bound ACL attributes for `{}`: {error}",
                    path.display(),
                ),
            ));
        }
        let required = usize::try_from(required)
            .map_err(|_| io::Error::other("Linux xattr list length exceeds usize"))?;
        if required > MAX_XATTR_LIST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "descriptor-bound ACL attribute list for `{}` exceeds {MAX_XATTR_LIST_BYTES} bytes",
                    path.display()
                ),
            ));
        }
        if required == 0 {
            return Ok(Some(Vec::new()));
        }
        let mut names = vec![0_u8; required];
        // SAFETY: `names` is writable for exactly its advertised size and the
        // retained descriptor remains valid throughout the call.
        let read = unsafe {
            flistxattr(
                handle.as_raw_fd(),
                names.as_mut_ptr().cast::<c_char>(),
                names.len(),
            )
        };
        if read < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERANGE) {
                return Ok(None);
            }
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "cannot read descriptor-bound ACL attributes for `{}`: {error}",
                    path.display()
                ),
            ));
        }
        let read = usize::try_from(read)
            .map_err(|_| io::Error::other("Linux xattr read length exceeds usize"))?;
        if read > names.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Linux xattr list read exceeded its descriptor-bound buffer",
            ));
        }
        names.truncate(read);
        Ok(Some(names))
    }

    #[cfg(target_os = "linux")]
    fn validate_linux_acl_attribute_names(names: &[u8], path: &Path) -> io::Result<()> {
        if names.is_empty() {
            return Ok(());
        }
        if names.last() != Some(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Linux descriptor-bound xattr list is not NUL terminated",
            ));
        }
        for name in names[..names.len() - 1].split(|byte| *byte == 0) {
            if name.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Linux descriptor-bound xattr list contains an empty name",
                ));
            }
            let protected_namespace = [b"system.".as_slice(), b"security.", b"trusted."]
                .iter()
                .any(|prefix| name.starts_with(prefix));
            let mentions_acl = name
                .windows(3)
                .any(|window| window.eq_ignore_ascii_case(b"acl"));
            if protected_namespace && mentions_acl {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "governance directory `{}` has descriptor-bound ACL authority `{}`",
                        path.display(),
                        String::from_utf8_lossy(name)
                    ),
                ));
            }
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn validate_macos_directory_acl(handle: &File, path: &Path) -> io::Result<()> {
        const ACL_TYPE_EXTENDED: c_int = 0x0000_0100;
        const ACL_FIRST_ENTRY: c_int = 0;
        const ACL_NEXT_ENTRY: c_int = -1;
        const ACL_EXTENDED_ALLOW: c_int = 1;
        const ACL_EXTENDED_DENY: c_int = 2;
        const ACL_MAX_ENTRIES: usize = 128;
        const ENOENT: i32 = 2;
        const EINVAL: i32 = 22;
        const MUTATION_PERMISSIONS: u64 = (1 << 2)
            | (1 << 4)
            | (1 << 5)
            | (1 << 6)
            | (1 << 8)
            | (1 << 10)
            | (1 << 12)
            | (1 << 13);

        struct Acl(*mut c_void);
        impl Drop for Acl {
            fn drop(&mut self) {
                // SAFETY: the pointer, when non-null, was allocated by
                // `acl_get_fd_np` and is freed exactly once here.
                let _ = unsafe { acl_free(self.0) };
            }
        }

        // SAFETY: the retained descriptor stays valid and the ACL type is the
        // macOS extended ACL type from `<sys/acl.h>`.
        let raw_acl = unsafe { acl_get_fd_np(handle.as_raw_fd(), ACL_TYPE_EXTENDED) };
        if raw_acl.is_null() {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ENOENT) {
                return Ok(());
            }
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "cannot inspect descriptor-bound macOS ACL for `{}`: {error}",
                    path.display()
                ),
            ));
        }
        let acl = Acl(raw_acl);
        let mut entry_id = ACL_FIRST_ENTRY;
        let mut entry_count = 0usize;
        loop {
            let mut entry = std::ptr::null_mut();
            // SAFETY: `acl` owns a valid ACL and `entry` is writable output.
            let result = unsafe { acl_get_entry(acl.0, entry_id, &mut entry) };
            if result != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(EINVAL) {
                    return Ok(());
                }
                {
                    return Err(io::Error::new(
                        error.kind(),
                        format!(
                            "cannot enumerate descriptor-bound macOS ACL for `{}`: {error}",
                            path.display()
                        ),
                    ));
                }
            }
            entry_count = entry_count.saturating_add(1);
            if entry_count > ACL_MAX_ENTRIES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "descriptor-bound macOS ACL for `{}` exceeds {ACL_MAX_ENTRIES} entries",
                        path.display()
                    ),
                ));
            }
            let mut tag_type = 0;
            // SAFETY: `entry` was returned from the live ACL above.
            if unsafe { acl_get_tag_type(entry, &mut tag_type) } != 0 {
                return Err(io::Error::new(
                    io::Error::last_os_error().kind(),
                    format!(
                        "cannot inspect descriptor-bound macOS ACL tag for `{}`",
                        path.display()
                    ),
                ));
            }
            match tag_type {
                ACL_EXTENDED_DENY => {}
                ACL_EXTENDED_ALLOW => {
                    let mut permissions = 0_u64;
                    // SAFETY: `entry` remains owned by the live ACL and the
                    // mask output points to initialized writable storage.
                    if unsafe { acl_get_permset_mask_np(entry, &mut permissions) } != 0 {
                        return Err(io::Error::new(
                            io::Error::last_os_error().kind(),
                            format!(
                                "cannot inspect descriptor-bound macOS ACL permissions for `{}`",
                                path.display()
                            ),
                        ));
                    }
                    if permissions & MUTATION_PERMISSIONS != 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            format!(
                                "governance directory `{}` has a descriptor-bound ACL mutation grant",
                                path.display()
                            ),
                        ));
                    }
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "governance directory `{}` has an unknown descriptor-bound ACL tag",
                            path.display()
                        ),
                    ));
                }
            }
            entry_id = ACL_NEXT_ENTRY;
        }
    }

    fn c_name(name: &OsStr) -> io::Result<CString> {
        validate_component(name)?;
        CString::new(name.as_bytes()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance path component contains NUL",
            )
        })
    }

    fn file_from_fd(fd: RawFd) -> io::Result<File> {
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `openat` returned a fresh owned descriptor on success.
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    pub(super) fn open_directory(parent: &File, name: &OsStr, _writable: bool) -> io::Result<File> {
        let name = c_name(name)?;
        // SAFETY: the parent descriptor and NUL-terminated component are valid
        // for the duration of the call; a successful descriptor is owned.
        let fd = unsafe {
            openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                O_READ_ONLY | O_DIRECTORY | O_NO_FOLLOW | O_CLOSE_ON_EXEC,
                0,
            )
        };
        file_from_fd(fd)
    }

    pub(super) fn create_directory(parent: &File, name: &OsStr) -> io::Result<()> {
        let name = c_name(name)?;
        // SAFETY: the parent descriptor and component pointer remain valid for
        // the call and mode 0700 grants no group/world mutation authority.
        if unsafe { mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub(super) fn open_file(parent: &File, name: &OsStr, _delete_access: bool) -> io::Result<File> {
        let name = c_name(name)?;
        // SAFETY: arguments remain valid for the call and the returned
        // descriptor, if any, is converted into its unique owner.
        let fd = unsafe {
            openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                O_READ_ONLY | O_NO_FOLLOW | O_CLOSE_ON_EXEC,
                0,
            )
        };
        file_from_fd(fd)
    }

    pub(super) fn open_read_write_file(parent: &File, name: &OsStr) -> io::Result<File> {
        let name = c_name(name)?;
        // SAFETY: arguments remain valid for the call and the returned
        // descriptor, if any, is converted into its unique owner.
        let fd = unsafe {
            openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                O_READ_WRITE | O_NO_FOLLOW | O_CLOSE_ON_EXEC,
                0,
            )
        };
        file_from_fd(fd)
    }

    pub(super) fn create_file(parent: &File, name: &OsStr) -> io::Result<File> {
        let name = c_name(name)?;
        // SAFETY: arguments remain valid and O_EXCL prevents an attacker-owned
        // existing name from being opened as our temporary.
        let fd = unsafe {
            openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                O_READ_WRITE | O_CREATE | O_EXCLUSIVE | O_NO_FOLLOW | O_CLOSE_ON_EXEC,
                0o600,
            )
        };
        file_from_fd(fd)
    }

    pub(super) fn rename_open_file(
        parent: &File,
        _temporary: &File,
        temporary_name: &OsStr,
        target_name: &OsStr,
    ) -> io::Result<()> {
        rename_exclusive(parent, temporary_name, parent, target_name)
    }

    pub(super) fn exchange_open_file(
        parent: &File,
        _temporary: &File,
        temporary_name: &OsStr,
        target_name: &OsStr,
    ) -> io::Result<()> {
        let temporary_name = c_name(temporary_name)?;
        let target_name = c_name(target_name)?;
        #[cfg(target_os = "linux")]
        // SAFETY: both names are direct components below the same retained
        // directory descriptor. RENAME_EXCHANGE preserves both bindings.
        let result = unsafe {
            renameat2(
                parent.as_raw_fd(),
                temporary_name.as_ptr(),
                parent.as_raw_fd(),
                target_name.as_ptr(),
                RENAME_EXCHANGE,
            )
        };
        #[cfg(target_os = "macos")]
        // SAFETY: as above; RENAME_SWAP is the macOS atomic exchange primitive.
        let result = unsafe {
            renameatx_np(
                parent.as_raw_fd(),
                temporary_name.as_ptr(),
                parent.as_raw_fd(),
                target_name.as_ptr(),
                RENAME_SWAP,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub(super) fn rename_exclusive(
        source_parent: &File,
        source_name: &OsStr,
        destination_parent: &File,
        destination_name: &OsStr,
    ) -> io::Result<()> {
        let source_name = c_name(source_name)?;
        let destination_name = c_name(destination_name)?;
        #[cfg(target_os = "linux")]
        // SAFETY: both names are direct components below retained directory
        // descriptors. RENAME_NOREPLACE prevents destination substitution or
        // overwrite while atomically isolating the current source binding.
        let result = unsafe {
            renameat2(
                source_parent.as_raw_fd(),
                source_name.as_ptr(),
                destination_parent.as_raw_fd(),
                destination_name.as_ptr(),
                RENAME_NOREPLACE,
            )
        };
        #[cfg(target_os = "macos")]
        // SAFETY: as above; RENAME_EXCL is the macOS create-only counterpart
        // of Linux RENAME_NOREPLACE.
        let result = unsafe {
            renameatx_np(
                source_parent.as_raw_fd(),
                source_name.as_ptr(),
                destination_parent.as_raw_fd(),
                destination_name.as_ptr(),
                RENAME_EXCL,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[cfg(test)]
    pub(super) fn remove_open_file(
        parent: &File,
        file: &File,
        name: &OsStr,
        expected: Option<FileIdentity>,
    ) -> io::Result<()> {
        let actual = file_identity(&file.metadata()?)?;
        if expected.is_some_and(|expected| expected != actual) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "retained governance file changed before unlink",
            ));
        }
        let linked = open_file(parent, name, false)?;
        let linked_identity = file_identity(&linked.metadata()?)?;
        if linked_identity != actual {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance file binding changed before unlink",
            ));
        }
        let name = c_name(name)?;
        // SAFETY: the direct component and retained directory descriptor are
        // valid. Identity was checked immediately above; failure is propagated.
        if unsafe { unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    #[cfg(test)]
    pub(super) fn remove_open_directory(
        parent: &File,
        directory: &File,
        name: &OsStr,
        expected: Option<FileIdentity>,
    ) -> io::Result<()> {
        let actual = file_identity(&directory.metadata()?)?;
        if expected.is_some_and(|expected| expected != actual) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance directory changed before removal",
            ));
        }
        let linked = open_directory(parent, name, true)?;
        let linked_identity = file_identity(&linked.metadata()?)?;
        if linked_identity != actual {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance directory binding changed before removal",
            ));
        }
        let name = c_name(name)?;
        // SAFETY: the direct component and retained parent descriptor are
        // valid, and both the supplied and freshly opened child identities
        // were checked immediately above. The kernel requires the child to be
        // empty for `AT_REMOVEDIR`.
        if unsafe { unlinkat(parent.as_raw_fd(), name.as_ptr(), AT_REMOVE_DIRECTORY) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub(super) fn child_names(
        directory: &RootedDirectory,
        max_entries: usize,
    ) -> io::Result<Vec<OsString>> {
        #[cfg(target_os = "linux")]
        let anchor = Path::new("/proc/self/fd").join(directory.handle.as_raw_fd().to_string());
        #[cfg(target_os = "macos")]
        let anchor = Path::new("/.vol")
            .join(directory.identity.device.to_string())
            .join(directory.identity.inode.to_string());
        let metadata = fs::metadata(&anchor)?;
        if !metadata.is_dir() || file_identity(&metadata)? != directory.identity {
            return Err(io::Error::other(
                "governance retained directory enumeration anchor is substituted",
            ));
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(anchor)? {
            names.push(entry?.file_name());
            if names.len() > max_entries {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance directory exceeds its entry bound",
                ));
            }
        }
        Ok(names)
    }
}

#[cfg(windows)]
mod platform {
    use std::{
        ffi::{OsStr, OsString, c_void},
        fs::{self, File, OpenOptions},
        io,
        mem::{offset_of, size_of},
        os::windows::{
            ffi::{OsStrExt as _, OsStringExt as _},
            fs::{MetadataExt as _, OpenOptionsExt as _},
            io::{AsRawHandle as _, FromRawHandle as _, RawHandle},
        },
        path::{Component, Path, PathBuf},
        ptr,
    };

    use super::{FileIdentity, RootedDirectory, file_identity, windows_dacl};

    type Handle = *mut c_void;
    type NtStatus = i32;

    const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const DELETE_ACCESS: u32 = 0x0001_0000;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const FILE_OPEN: u32 = 1;
    const FILE_CREATE: u32 = 2;
    const FILE_DIRECTORY_FILE: u32 = 0x0000_0001;
    const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x0000_0020;
    const FILE_NON_DIRECTORY_FILE: u32 = 0x0000_0040;
    const FILE_OPEN_FOR_BACKUP_INTENT: u32 = 0x0000_4000;
    const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const OBJ_CASE_INSENSITIVE: u32 = 0x0000_0040;
    const FILE_RENAME_INFO_CLASS: u32 = 3;
    const FILE_DISPOSITION_INFO_CLASS: u32 = 4;
    const FILE_ID_BOTH_DIRECTORY_INFO_CLASS: u32 = 10;
    const FILE_ID_BOTH_DIRECTORY_RESTART_INFO_CLASS: u32 = 11;
    const ERROR_NO_MORE_FILES: i32 = 18;
    const ERROR_SUCCESS: u32 = 0;
    const SE_FILE_OBJECT: i32 = 1;
    const OWNER_SECURITY_INFORMATION: u32 = 0x1;
    const DACL_SECURITY_INFORMATION: u32 = 0x4;
    const SE_DACL_PRESENT: u16 = 0x0004;
    const SE_SELF_RELATIVE: u16 = 0x8000;
    const SECURITY_DESCRIPTOR_REVISION: u32 = 1;
    const SECURITY_DESCRIPTOR_HEADER_BYTES: usize = 20;
    const SECURITY_DESCRIPTOR_OWNER_OFFSET: usize = 4;
    const SECURITY_DESCRIPTOR_DACL_OFFSET: usize = 16;
    const MAX_SECURITY_DESCRIPTOR_BYTES: usize = 1024 * 1024;

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }

    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root_directory: Handle,
        object_name: *mut UnicodeString,
        attributes: u32,
        security_descriptor: *mut c_void,
        security_quality_of_service: *mut c_void,
    }

    #[repr(C)]
    struct IoStatusBlock {
        status_or_pointer: isize,
        information: usize,
    }

    #[repr(C)]
    struct FileRenameInfo {
        replace_or_flags: u32,
        root_directory: Handle,
        file_name_length: u32,
        file_name: [u16; 1],
    }

    #[repr(C)]
    struct FileDispositionInfo {
        delete_file: u8,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct WindowsDaclSnapshot {
        control: u16,
        owner_sid: Vec<u8>,
        dacl: Vec<u8>,
    }

    struct LocalSecurityDescriptor(Handle);

    impl Drop for LocalSecurityDescriptor {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: `GetSecurityInfo` allocated this descriptor with
                // LocalAlloc-compatible storage and ownership is unique here.
                let _ = unsafe { local_free(self.0) };
            }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FileIdBothDirectoryInfo {
        next_entry_offset: u32,
        file_index: u32,
        creation_time: i64,
        last_access_time: i64,
        last_write_time: i64,
        change_time: i64,
        end_of_file: i64,
        allocation_size: i64,
        file_attributes: u32,
        file_name_length: u32,
        ea_size: u32,
        short_name_length: i8,
        short_name: [u16; 12],
        file_id: i64,
        file_name: [u16; 1],
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        #[link_name = "NtCreateFile"]
        fn nt_create_file(
            file_handle: *mut Handle,
            desired_access: u32,
            object_attributes: *mut ObjectAttributes,
            io_status_block: *mut IoStatusBlock,
            allocation_size: *mut i64,
            file_attributes: u32,
            share_access: u32,
            create_disposition: u32,
            create_options: u32,
            ea_buffer: *mut c_void,
            ea_length: u32,
        ) -> NtStatus;
        #[link_name = "RtlNtStatusToDosError"]
        fn rtl_nt_status_to_dos_error(status: NtStatus) -> u32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "GetFileInformationByHandleEx"]
        fn get_file_information_by_handle_ex(
            file: Handle,
            information_class: u32,
            information: *mut c_void,
            buffer_size: u32,
        ) -> i32;
        #[link_name = "SetFileInformationByHandle"]
        fn set_file_information_by_handle(
            file: Handle,
            information_class: u32,
            information: *const c_void,
            buffer_size: u32,
        ) -> i32;
        #[link_name = "LocalFree"]
        fn local_free(memory: Handle) -> Handle;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        #[link_name = "GetSecurityInfo"]
        fn get_security_info(
            handle: Handle,
            object_type: i32,
            security_information: u32,
            owner: *mut Handle,
            group: *mut Handle,
            dacl: *mut Handle,
            sacl: *mut Handle,
            security_descriptor: *mut Handle,
        ) -> u32;
        #[link_name = "GetSecurityDescriptorControl"]
        fn get_security_descriptor_control(
            security_descriptor: Handle,
            control: *mut u16,
            revision: *mut u32,
        ) -> i32;
        #[link_name = "GetSecurityDescriptorLength"]
        fn get_security_descriptor_length(security_descriptor: Handle) -> u32;
        #[link_name = "IsValidSecurityDescriptor"]
        fn is_valid_security_descriptor(security_descriptor: Handle) -> i32;
    }

    pub(super) fn ensure_supported() -> io::Result<()> {
        Ok(())
    }

    pub(super) fn validate_component(name: &OsStr) -> io::Result<()> {
        let mut saw_unit = false;
        for unit in name.encode_wide() {
            saw_unit = true;
            if unit == 0
                || unit == u16::from(b'/')
                || unit == u16::from(b'\\')
                || unit == u16::from(b':')
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "governance path component contains a separator, stream marker, or NUL",
                ));
            }
        }
        if !saw_unit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance path component is empty",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_non_reparse(metadata: &fs::Metadata) -> io::Result<()> {
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::other(
                "Windows governance object is backed by a reparse point",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_directory_acl(handle: &File, path: &Path) -> io::Result<()> {
        qualified_directory_dacl_snapshot(handle, path).map(drop)
    }

    pub(super) fn directory_owner_sid(handle: &File, path: &Path) -> io::Result<Vec<u8>> {
        qualified_directory_dacl_snapshot(handle, path).map(|snapshot| snapshot.owner_sid)
    }

    fn qualified_directory_dacl_snapshot(
        handle: &File,
        path: &Path,
    ) -> io::Result<WindowsDaclSnapshot> {
        let first = read_directory_dacl_snapshot(handle, path)?;
        windows_dacl::validate(Some(&first.owner_sid), Some(&first.dacl))?;
        let second = read_directory_dacl_snapshot(handle, path)?;
        windows_dacl::validate(Some(&second.owner_sid), Some(&second.dacl))?;
        if first != second {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "descriptor-bound Windows DACL for `{}` changed during inspection",
                    path.display()
                ),
            ));
        }
        Ok(second)
    }

    fn read_directory_dacl_snapshot(handle: &File, path: &Path) -> io::Result<WindowsDaclSnapshot> {
        let mut owner = ptr::null_mut();
        let mut dacl = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        // SAFETY: the retained directory handle has READ_CONTROL through
        // GENERIC_READ. All requested outputs point to initialized storage and
        // optional group/SACL outputs are null.
        let status = unsafe {
            get_security_info(
                handle.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                ptr::null_mut(),
                &mut dacl,
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        let descriptor_guard = LocalSecurityDescriptor(descriptor);
        if status != ERROR_SUCCESS {
            let error = io::Error::from_raw_os_error(i32::try_from(status).unwrap_or(i32::MAX));
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "cannot inspect descriptor-bound Windows DACL for `{}`: {error}",
                    path.display()
                ),
            ));
        }
        if descriptor_guard.0.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "GetSecurityInfo returned a null security descriptor",
            ));
        }
        if owner.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "GetSecurityInfo returned a null governance owner SID",
            ));
        }
        if dacl.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "GetSecurityInfo returned a null governance DACL",
            ));
        }
        // SAFETY: the descriptor was returned successfully by GetSecurityInfo.
        if unsafe { is_valid_security_descriptor(descriptor_guard.0) } == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "GetSecurityInfo returned an invalid security descriptor",
            ));
        }
        let mut control = 0_u16;
        let mut revision = 0_u32;
        // SAFETY: both outputs are initialized writable values and the
        // descriptor remains owned by `descriptor_guard`.
        if unsafe {
            get_security_descriptor_control(descriptor_guard.0, &mut control, &mut revision)
        } == 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cannot read Windows security descriptor control flags",
            ));
        }
        if revision != SECURITY_DESCRIPTOR_REVISION
            || control & (SE_DACL_PRESENT | SE_SELF_RELATIVE) != SE_DACL_PRESENT | SE_SELF_RELATIVE
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows governance security descriptor is not canonical self-relative DACL state",
            ));
        }
        // SAFETY: the descriptor remains live and was validated above.
        let descriptor_length =
            usize::try_from(unsafe { get_security_descriptor_length(descriptor_guard.0) })
                .map_err(|_| io::Error::other("Windows security descriptor exceeds usize"))?;
        if !(SECURITY_DESCRIPTOR_HEADER_BYTES..=MAX_SECURITY_DESCRIPTOR_BYTES)
            .contains(&descriptor_length)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows governance security descriptor length is out of bounds",
            ));
        }
        // SAFETY: GetSecurityDescriptorLength describes the complete live,
        // self-relative allocation returned by GetSecurityInfo.
        let descriptor_bytes = unsafe {
            std::slice::from_raw_parts(descriptor_guard.0.cast::<u8>(), descriptor_length)
        };
        if descriptor_bytes[0]
            != u8::try_from(SECURITY_DESCRIPTOR_REVISION).expect("revision fits u8")
            || descriptor_bytes[1] != 0
            || u16::from_le_bytes([descriptor_bytes[2], descriptor_bytes[3]]) != control
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows governance security descriptor header is noncanonical",
            ));
        }
        let encoded_owner_offset =
            descriptor_u32(descriptor_bytes, SECURITY_DESCRIPTOR_OWNER_OFFSET)?;
        let encoded_dacl_offset =
            descriptor_u32(descriptor_bytes, SECURITY_DESCRIPTOR_DACL_OFFSET)?;
        let owner_offset =
            descriptor_pointer_offset(descriptor_guard.0, descriptor_length, owner, "owner SID")?;
        let dacl_offset =
            descriptor_pointer_offset(descriptor_guard.0, descriptor_length, dacl, "DACL")?;
        if encoded_owner_offset == 0
            || encoded_dacl_offset == 0
            || usize::try_from(encoded_owner_offset).ok() != Some(owner_offset)
            || usize::try_from(encoded_dacl_offset).ok() != Some(dacl_offset)
            || owner_offset < SECURITY_DESCRIPTOR_HEADER_BYTES
            || dacl_offset < SECURITY_DESCRIPTOR_HEADER_BYTES
            || owner_offset % size_of::<u32>() != 0
            || dacl_offset % size_of::<u32>() != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows governance security descriptor offsets are noncanonical",
            ));
        }
        let owner_length = windows_dacl::sid_encoded_length(&descriptor_bytes[owner_offset..])?;
        let owner_end = owner_offset
            .checked_add(owner_length)
            .ok_or_else(|| io::Error::other("Windows owner SID length overflow"))?;
        let dacl_length = windows_dacl::dacl_encoded_length(&descriptor_bytes[dacl_offset..])?;
        let dacl_end = dacl_offset
            .checked_add(dacl_length)
            .ok_or_else(|| io::Error::other("Windows DACL length overflow"))?;
        if owner_end > descriptor_bytes.len()
            || dacl_end > descriptor_bytes.len()
            || (owner_offset < dacl_end && dacl_offset < owner_end)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows governance security descriptor is truncated or overlapping",
            ));
        }
        Ok(WindowsDaclSnapshot {
            control,
            owner_sid: descriptor_bytes[owner_offset..owner_end].to_vec(),
            dacl: descriptor_bytes[dacl_offset..dacl_end].to_vec(),
        })
    }

    fn descriptor_u32(bytes: &[u8], offset: usize) -> io::Result<u32> {
        let end = offset
            .checked_add(size_of::<u32>())
            .ok_or_else(|| io::Error::other("Windows descriptor offset overflow"))?;
        let raw = bytes.get(offset..end).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Windows security descriptor integer is truncated",
            )
        })?;
        Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    fn descriptor_pointer_offset(
        descriptor: Handle,
        descriptor_length: usize,
        field: Handle,
        field_name: &str,
    ) -> io::Result<usize> {
        let descriptor_start = descriptor as usize;
        let descriptor_end = descriptor_start
            .checked_add(descriptor_length)
            .ok_or_else(|| io::Error::other("Windows security descriptor address overflow"))?;
        let field_address = field as usize;
        if !(descriptor_start..descriptor_end).contains(&field_address) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Windows security descriptor {field_name} pointer is outside its allocation"
                ),
            ));
        }
        field_address
            .checked_sub(descriptor_start)
            .ok_or_else(|| io::Error::other("Windows security descriptor pointer underflow"))
    }

    pub(super) fn open_root(path: &Path, writable: bool) -> io::Result<File> {
        if !path.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Windows governance root must be absolute",
            ));
        }
        let components = path.components().collect::<Vec<_>>();
        if components
            .iter()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Windows governance root contains a relative component",
            ));
        }
        let first_normal = components
            .iter()
            .position(|component| matches!(component, Component::Normal(_)))
            .unwrap_or(components.len());
        let mut volume_root = PathBuf::new();
        for component in &components[..first_normal] {
            match component {
                Component::Prefix(_) | Component::RootDir => {
                    volume_root.push(component.as_os_str());
                }
                Component::Normal(_) | Component::CurDir | Component::ParentDir => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Windows governance root has a malformed volume prefix",
                    ));
                }
            }
        }
        if volume_root.as_os_str().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Windows governance root has no volume prefix",
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true);
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
        let mut directory = options.open(&volume_root)?;
        let volume_metadata = directory.metadata()?;
        if !volume_metadata.is_dir() {
            return Err(io::Error::other(
                "Windows governance volume root is not a directory",
            ));
        }
        validate_non_reparse(&volume_metadata)?;
        let _ = file_identity(&volume_metadata)?;

        for (position, component) in components[first_normal..].iter().enumerate() {
            let Component::Normal(name) = component else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Windows governance root has a non-canonical component",
                ));
            };
            let is_last = position + 1 == components.len() - first_normal;
            directory = open_directory(&directory, name, writable && is_last)?;
            let metadata = directory.metadata()?;
            if !metadata.is_dir() {
                return Err(io::Error::other(format!(
                    "Windows governance ancestor `{}` is not a directory",
                    path.display()
                )));
            }
            validate_non_reparse(&metadata)?;
            let _ = file_identity(&metadata)?;
        }
        if first_normal == components.len() && writable {
            let mut writable_options = OpenOptions::new();
            writable_options.read(true).write(true);
            writable_options
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
            directory = writable_options.open(&volume_root)?;
        }
        let linked = fs::symlink_metadata(path)?;
        validate_non_reparse(&linked)?;
        if !linked.is_dir() || file_identity(&linked)? != file_identity(&directory.metadata()?)? {
            return Err(io::Error::other(
                "Windows governance root changed during component-wise open",
            ));
        }
        Ok(directory)
    }

    fn nt_error(status: NtStatus) -> io::Error {
        // SAFETY: conversion is a pure ntdll status mapping.
        let win32 = unsafe { rtl_nt_status_to_dos_error(status) };
        io::Error::from_raw_os_error(i32::try_from(win32).unwrap_or(i32::MAX))
    }

    fn nt_open_relative(
        parent: &File,
        name: &OsStr,
        desired_access: u32,
        disposition: u32,
        options: u32,
    ) -> io::Result<File> {
        nt_open_relative_with_share(
            parent,
            name,
            desired_access,
            disposition,
            options,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        )
    }

    fn nt_open_relative_with_share(
        parent: &File,
        name: &OsStr,
        desired_access: u32,
        disposition: u32,
        options: u32,
        share_access: u32,
    ) -> io::Result<File> {
        validate_component(name)?;
        let mut name_wide = name.encode_wide().collect::<Vec<_>>();
        let byte_len = name_wide
            .len()
            .checked_mul(size_of::<u16>())
            .and_then(|length| u16::try_from(length).ok())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Windows governance path component is too long",
                )
            })?;
        let mut unicode = UnicodeString {
            length: byte_len,
            maximum_length: byte_len,
            buffer: name_wide.as_mut_ptr(),
        };
        let mut attributes = ObjectAttributes {
            length: u32::try_from(size_of::<ObjectAttributes>())
                .expect("OBJECT_ATTRIBUTES fits u32"),
            root_directory: parent.as_raw_handle(),
            object_name: &mut unicode,
            attributes: OBJ_CASE_INSENSITIVE,
            security_descriptor: ptr::null_mut(),
            security_quality_of_service: ptr::null_mut(),
        };
        let mut status_block = IoStatusBlock {
            status_or_pointer: 0,
            information: 0,
        };
        let mut handle: Handle = ptr::null_mut();
        // SAFETY: every pointer references initialized storage that outlives
        // the call. On success the returned handle has unique ownership.
        let status = unsafe {
            nt_create_file(
                &mut handle,
                desired_access,
                &mut attributes,
                &mut status_block,
                ptr::null_mut(),
                FILE_ATTRIBUTE_NORMAL,
                share_access,
                disposition,
                options | FILE_SYNCHRONOUS_IO_NONALERT | FILE_OPEN_REPARSE_POINT,
                ptr::null_mut(),
                0,
            )
        };
        if status < 0 {
            return Err(nt_error(status));
        }
        if handle.is_null() {
            return Err(io::Error::other(
                "NtCreateFile returned a null governance handle",
            ));
        }
        // SAFETY: `NtCreateFile` returned a fresh owned handle above.
        Ok(unsafe { File::from_raw_handle(handle as RawHandle) })
    }

    pub(super) fn open_directory(parent: &File, name: &OsStr, writable: bool) -> io::Result<File> {
        let access = GENERIC_READ
            | SYNCHRONIZE
            | if writable {
                GENERIC_WRITE | DELETE_ACCESS
            } else {
                0
            };
        nt_open_relative(
            parent,
            name,
            access,
            FILE_OPEN,
            FILE_DIRECTORY_FILE | FILE_OPEN_FOR_BACKUP_INTENT,
        )
    }

    pub(super) fn create_directory(parent: &File, name: &OsStr) -> io::Result<()> {
        nt_open_relative(
            parent,
            name,
            GENERIC_READ | GENERIC_WRITE | SYNCHRONIZE,
            FILE_CREATE,
            FILE_DIRECTORY_FILE | FILE_OPEN_FOR_BACKUP_INTENT,
        )
        .map(drop)
    }

    pub(super) fn open_file(parent: &File, name: &OsStr, delete_access: bool) -> io::Result<File> {
        nt_open_relative(
            parent,
            name,
            GENERIC_READ | SYNCHRONIZE | if delete_access { DELETE_ACCESS } else { 0 },
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        )
    }

    pub(super) fn open_read_write_file(parent: &File, name: &OsStr) -> io::Result<File> {
        nt_open_relative(
            parent,
            name,
            GENERIC_READ | GENERIC_WRITE | SYNCHRONIZE,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
        )
    }

    pub(super) fn create_file(parent: &File, name: &OsStr) -> io::Result<File> {
        nt_open_relative(
            parent,
            name,
            GENERIC_READ | GENERIC_WRITE | DELETE_ACCESS | SYNCHRONIZE,
            FILE_CREATE,
            FILE_NON_DIRECTORY_FILE,
        )
    }

    pub(super) fn rename_open_file(
        parent: &File,
        temporary: &File,
        _temporary_name: &OsStr,
        target_name: &OsStr,
    ) -> io::Result<()> {
        validate_component(target_name)?;
        let target_wide = target_name.encode_wide().collect::<Vec<_>>();
        let target_byte_len = target_wide
            .len()
            .checked_mul(size_of::<u16>())
            .and_then(|length| u32::try_from(length).ok())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Windows governance target name is too long",
                )
            })?;
        let file_name_offset = offset_of!(FileRenameInfo, file_name);
        let total_bytes = file_name_offset
            .checked_add(usize::try_from(target_byte_len).unwrap_or(usize::MAX))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Windows governance rename buffer is too large",
                )
            })?;
        let word_count = total_bytes.div_ceil(size_of::<usize>());
        let mut storage = vec![0usize; word_count];
        let info = storage.as_mut_ptr().cast::<FileRenameInfo>();
        // SAFETY: `storage` is aligned for every field, has at least
        // `total_bytes`, and `file_name_offset` is the actual repr(C) offset.
        unsafe {
            (*info).replace_or_flags = 0;
            (*info).root_directory = parent.as_raw_handle();
            (*info).file_name_length = target_byte_len;
            ptr::copy_nonoverlapping(
                target_wide.as_ptr(),
                storage
                    .as_mut_ptr()
                    .cast::<u8>()
                    .add(file_name_offset)
                    .cast::<u16>(),
                target_wide.len(),
            );
        }
        let total_bytes = u32::try_from(total_bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Windows governance rename buffer exceeds u32",
            )
        })?;
        // SAFETY: the temporary handle and initialized rename buffer remain
        // valid for the call.
        let result = unsafe {
            set_file_information_by_handle(
                temporary.as_raw_handle(),
                FILE_RENAME_INFO_CLASS,
                info.cast(),
                total_bytes,
            )
        };
        if result != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub(super) fn remove_open_file(
        _parent: &File,
        file: &File,
        _name: &OsStr,
        expected: Option<FileIdentity>,
    ) -> io::Result<()> {
        let actual = file_identity(&file.metadata()?)?;
        if expected.is_some_and(|expected| expected != actual) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Windows governance temporary handle changed identity",
            ));
        }
        let info = FileDispositionInfo { delete_file: 1 };
        // SAFETY: disposition applies to the exact retained file handle and the
        // fixed-size input structure is initialized for the duration of call.
        let result = unsafe {
            set_file_information_by_handle(
                file.as_raw_handle(),
                FILE_DISPOSITION_INFO_CLASS,
                ptr::from_ref(&info).cast(),
                u32::try_from(size_of::<FileDispositionInfo>())
                    .expect("FILE_DISPOSITION_INFO fits u32"),
            )
        };
        if result != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub(super) fn remove_open_directory(
        parent: &File,
        directory: &File,
        name: &OsStr,
        expected: Option<FileIdentity>,
    ) -> io::Result<()> {
        let actual = file_identity(&directory.metadata()?)?;
        if expected.is_some_and(|expected| expected != actual) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Windows governance directory handle changed identity",
            ));
        }
        let linked = open_directory(parent, name, true)?;
        let linked_identity = file_identity(&linked.metadata()?)?;
        if linked_identity != actual {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Windows governance directory binding changed before removal",
            ));
        }
        let info = FileDispositionInfo { delete_file: 1 };
        // SAFETY: disposition applies to the exact retained, reparse-free
        // directory handle. Windows refuses the operation unless it is empty.
        let result = unsafe {
            set_file_information_by_handle(
                directory.as_raw_handle(),
                FILE_DISPOSITION_INFO_CLASS,
                ptr::from_ref(&info).cast(),
                u32::try_from(size_of::<FileDispositionInfo>())
                    .expect("FILE_DISPOSITION_INFO fits u32"),
            )
        };
        if result != 0 {
            drop(linked);
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub(super) fn child_names(
        directory: &RootedDirectory,
        max_entries: usize,
    ) -> io::Result<Vec<OsString>> {
        enumerate_directory_handle(&directory.handle, max_entries)
    }

    fn enumerate_directory_handle(
        directory: &File,
        max_entries: usize,
    ) -> io::Result<Vec<OsString>> {
        const BUFFER_BYTES: usize = 64 * 1024;

        let word_count = BUFFER_BYTES.div_ceil(size_of::<usize>());
        let mut storage = vec![0usize; word_count];
        let mut restart = true;
        let mut names = Vec::new();
        loop {
            storage.fill(0);
            let information_class = if restart {
                FILE_ID_BOTH_DIRECTORY_RESTART_INFO_CLASS
            } else {
                FILE_ID_BOTH_DIRECTORY_INFO_CLASS
            };
            // SAFETY: `storage` is writable and aligned, and its advertised
            // byte length remains valid for the duration of the call.
            let result = unsafe {
                get_file_information_by_handle_ex(
                    directory.as_raw_handle(),
                    information_class,
                    storage.as_mut_ptr().cast(),
                    u32::try_from(BUFFER_BYTES).expect("directory buffer fits u32"),
                )
            };
            if result == 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(ERROR_NO_MORE_FILES) {
                    break;
                }
                return Err(error);
            }
            restart = false;
            // SAFETY: `storage` owns at least BUFFER_BYTES initialized bytes
            // and remains alive for the complete parser call.
            let buffer =
                unsafe { std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), BUFFER_BYTES) };
            parse_directory_buffer(buffer, &mut names, max_entries)?;
        }
        Ok(names)
    }

    fn parse_directory_buffer(
        buffer: &[u8],
        names: &mut Vec<OsString>,
        max_entries: usize,
    ) -> io::Result<()> {
        let file_name_offset = offset_of!(FileIdBothDirectoryInfo, file_name);
        let mut offset = 0usize;
        loop {
            let header_end = offset.checked_add(file_name_offset).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows directory enumeration offset overflow",
                )
            })?;
            if header_end > buffer.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows directory enumeration header exceeds its buffer",
                ));
            }
            let fixed_end = offset
                .checked_add(size_of::<FileIdBothDirectoryInfo>())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Windows directory enumeration fixed entry overflow",
                    )
                })?;
            if fixed_end > buffer.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows directory enumeration fixed entry exceeds its buffer",
                ));
            }
            // SAFETY: the fixed header is within the initialized kernel output
            // buffer. `read_unaligned` avoids assuming entry alignment.
            let entry = unsafe {
                ptr::read_unaligned(
                    buffer
                        .as_ptr()
                        .add(offset)
                        .cast::<FileIdBothDirectoryInfo>(),
                )
            };
            let file_name_bytes = usize::try_from(entry.file_name_length).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows directory file name length exceeds usize",
                )
            })?;
            if file_name_bytes % size_of::<u16>() != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows directory file name has an odd byte length",
                ));
            }
            let name_end = header_end.checked_add(file_name_bytes).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows directory file name length overflow",
                )
            })?;
            if name_end > buffer.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows directory file name exceeds its buffer",
                ));
            }
            let unit_count = file_name_bytes / size_of::<u16>();
            let mut units = Vec::with_capacity(unit_count);
            for position in 0..unit_count {
                // SAFETY: each two-byte unit is within `name_end`; unaligned
                // reads avoid additional layout assumptions.
                units.push(unsafe {
                    ptr::read_unaligned(
                        buffer
                            .as_ptr()
                            .add(header_end + position * size_of::<u16>())
                            .cast::<u16>(),
                    )
                });
            }
            let name = OsString::from_wide(&units);
            if name != OsStr::new(".") && name != OsStr::new("..") {
                names.push(name);
                if names.len() > max_entries {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Windows governance directory exceeds its entry bound",
                    ));
                }
            }
            if entry.next_entry_offset == 0 {
                break;
            }
            let next = usize::try_from(entry.next_entry_offset).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows directory next-entry offset exceeds usize",
                )
            })?;
            let consumed = file_name_offset
                .checked_add(file_name_bytes)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Windows directory entry length overflow",
                    )
                })?;
            if next < consumed {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows directory next-entry offset overlaps its current entry",
                ));
            }
            offset = offset.checked_add(next).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows directory next-entry offset overflow",
                )
            })?;
            if offset >= buffer.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Windows directory next-entry offset exceeds its buffer",
                ));
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use std::process::Command;

        use super::*;
        use tempfile::tempdir;

        fn write_entry(
            buffer: &mut [u8],
            offset: usize,
            name: &[u16],
            next_entry_offset: u32,
            declared_name_bytes: u32,
        ) {
            let file_name_offset = offset_of!(FileIdBothDirectoryInfo, file_name);
            assert!(offset + size_of::<FileIdBothDirectoryInfo>() <= buffer.len());
            assert!(offset + file_name_offset + name.len() * size_of::<u16>() <= buffer.len());
            // SAFETY: the zeroed structure contains only integer fields and is
            // written unaligned into the explicitly bounded test buffer.
            let mut entry = unsafe { std::mem::zeroed::<FileIdBothDirectoryInfo>() };
            entry.next_entry_offset = next_entry_offset;
            entry.file_name_length = declared_name_bytes;
            unsafe {
                ptr::write_unaligned(
                    buffer
                        .as_mut_ptr()
                        .add(offset)
                        .cast::<FileIdBothDirectoryInfo>(),
                    entry,
                );
                for (position, unit) in name.iter().enumerate() {
                    ptr::write_unaligned(
                        buffer
                            .as_mut_ptr()
                            .add(offset + file_name_offset + position * size_of::<u16>())
                            .cast::<u16>(),
                        *unit,
                    );
                }
            }
        }

        #[test]
        fn directory_buffer_parser_rejects_odd_and_overlapping_bounds() {
            let mut odd = vec![0_u8; size_of::<FileIdBothDirectoryInfo>() + 16];
            write_entry(&mut odd, 0, &[u16::from(b'a')], 0, 1);
            assert_eq!(
                parse_directory_buffer(&odd, &mut Vec::new(), 8)
                    .expect_err("odd UTF-16 byte count must fail")
                    .kind(),
                io::ErrorKind::InvalidData
            );

            let file_name_offset = offset_of!(FileIdBothDirectoryInfo, file_name);
            let mut overlapping = vec![0_u8; size_of::<FileIdBothDirectoryInfo>() + 32];
            write_entry(
                &mut overlapping,
                0,
                &[u16::from(b'a'), u16::from(b'b')],
                u32::try_from(file_name_offset).expect("offset fits u32"),
                4,
            );
            assert_eq!(
                parse_directory_buffer(&overlapping, &mut Vec::new(), 8)
                    .expect_err("overlapping next entry must fail")
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }

        #[test]
        fn directory_buffer_parser_enforces_bound_across_restart_pages() {
            let mut names = Vec::new();
            let mut first = vec![0_u8; size_of::<FileIdBothDirectoryInfo>() + 16];
            write_entry(&mut first, 0, &[u16::from(b'a')], 0, 2);
            parse_directory_buffer(&first, &mut names, 1).expect("parse first page");

            let mut restarted = vec![0_u8; size_of::<FileIdBothDirectoryInfo>() + 16];
            write_entry(&mut restarted, 0, &[u16::from(b'b')], 0, 2);
            assert_eq!(
                parse_directory_buffer(&restarted, &mut names, 1)
                    .expect_err("second restart page must retain the aggregate bound")
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }

        #[test]
        fn descriptor_bound_dacl_rejects_post_open_everyone_mutation_grant() {
            let temp = tempdir().expect("create Windows ACL test directory");
            let handle = open_root(temp.path(), false).expect("retain Windows ACL test directory");
            validate_directory_acl(&handle, temp.path())
                .expect("default temporary-directory DACL must be release-qualified");

            let grant = Command::new("icacls")
                .arg(temp.path())
                .arg("/grant")
                .arg("*S-1-1-0:(OI)(CI)M")
                .status()
                .expect("execute icacls mutation grant");
            assert!(grant.success(), "icacls mutation grant must succeed");
            let result = validate_directory_acl(&handle, temp.path());
            let remove = Command::new("icacls")
                .arg(temp.path())
                .arg("/remove:g")
                .arg("*S-1-1-0")
                .status()
                .expect("execute icacls mutation-grant cleanup");
            assert!(
                remove.success(),
                "icacls mutation-grant cleanup must succeed"
            );
            assert_eq!(
                result
                    .expect_err("post-open Everyone mutation grant must fail closed")
                    .kind(),
                io::ErrorKind::PermissionDenied
            );
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod platform {
    use std::{
        ffi::{OsStr, OsString},
        fs::{self, File},
        io,
    };

    use super::{FileIdentity, RootedDirectory};

    pub(super) fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "rooted Governance DAG filesystem operations are unsupported on this platform",
        )
    }

    pub(super) fn ensure_supported() -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn validate_component(_name: &OsStr) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn validate_non_reparse(_metadata: &fs::Metadata) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn validate_directory_acl(
        _handle: &File,
        _path: &std::path::Path,
    ) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn open_directory(
        _parent: &File,
        _name: &OsStr,
        _writable: bool,
    ) -> io::Result<File> {
        Err(unsupported())
    }

    pub(super) fn create_directory(_parent: &File, _name: &OsStr) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn open_file(
        _parent: &File,
        _name: &OsStr,
        _delete_access: bool,
    ) -> io::Result<File> {
        Err(unsupported())
    }

    pub(super) fn open_read_write_file(_parent: &File, _name: &OsStr) -> io::Result<File> {
        Err(unsupported())
    }

    pub(super) fn create_file(_parent: &File, _name: &OsStr) -> io::Result<File> {
        Err(unsupported())
    }

    pub(super) fn rename_open_file(
        _parent: &File,
        _temporary: &File,
        _temporary_name: &OsStr,
        _target_name: &OsStr,
    ) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn remove_open_file(
        _parent: &File,
        _file: &File,
        _name: &OsStr,
        _expected: Option<FileIdentity>,
    ) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn remove_open_directory(
        _parent: &File,
        _directory: &File,
        _name: &OsStr,
        _expected: Option<FileIdentity>,
    ) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn child_names(
        _directory: &RootedDirectory,
        _max_entries: usize,
    ) -> io::Result<Vec<OsString>> {
        Err(unsupported())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::{OsStr, OsString},
        fs,
        io::{self, Seek as _, SeekFrom, Write as _},
        panic::AssertUnwindSafe,
        sync::{Arc, Barrier, mpsc},
        thread,
        time::Duration,
    };

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use std::cell::Cell;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;
    #[cfg(target_os = "macos")]
    use std::process::Command;
    #[cfg(target_os = "linux")]
    use std::{
        ffi::{CString, c_char, c_int, c_void},
        os::fd::AsRawFd as _,
    };

    use tempfile::tempdir;

    use super::{
        ExpectedFile, RootedDirectory, TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1, TWO_SLOT_NAMES_V1,
        TWO_SLOT_ZERO_DIGEST, TwoSlotInitFileLockV1, TwoSlotSnapshotV1, TwoSlotStageV1,
        TwoSlotStoreConfigV1, TwoSlotStoreV1, decode_two_slot_value, encode_two_slot_value,
        initialize_two_slot_stage, open_existing_two_slot_store, read_exact_file_region,
        two_slot_init_lock_name, two_slot_lost_found_name, two_slot_stage_prefix,
        write_exact_file_region, write_two_slot_record_unlocked,
    };

    fn test_root(path: &std::path::Path) -> RootedDirectory {
        #[cfg(windows)]
        {
            RootedDirectory::open_root(path, true).expect("retain rooted Windows test directory")
        }
        #[cfg(not(windows))]
        {
            let handle = Arc::new(fs::File::open(path).expect("open test root"));
            RootedDirectory::from_retained(path.to_path_buf(), handle, true)
                .expect("retain rooted test directory")
        }
    }

    fn read_only_test_root(path: &std::path::Path) -> RootedDirectory {
        #[cfg(windows)]
        {
            RootedDirectory::open_root(path, false)
                .expect("retain read-only rooted Windows test directory")
        }
        #[cfg(not(windows))]
        {
            let handle = Arc::new(fs::File::open(path).expect("open read-only test root"));
            RootedDirectory::from_retained(path.to_path_buf(), handle, false)
                .expect("retain read-only rooted test directory")
        }
    }

    fn two_slot_config(name: &str) -> TwoSlotStoreConfigV1 {
        TwoSlotStoreConfigV1::try_new(name, [0x51; 32], [0xa7; 32], 512)
            .expect("valid bounded two-slot test config")
    }

    fn two_slot_fault(label: &'static str) -> io::Error {
        io::Error::other(format!("injected two-slot fault after {label}"))
    }

    fn raw_test_record(
        store: &TwoSlotStoreV1,
        slot_id: usize,
        generation: u64,
        predecessor_digest: [u8; 32],
        payload: &[u8],
    ) {
        let mut no_fault = |_| Ok(());
        store
            .with_exclusive_lock(|store| {
                write_two_slot_record_unlocked(
                    store,
                    slot_id,
                    generation,
                    predecessor_digest,
                    payload,
                    ["test"; 6],
                    &mut no_fault,
                )
                .map(drop)
            })
            .expect("write exact test record");
    }

    fn initialize_test_stage(
        root: &RootedDirectory,
        config: &TwoSlotStoreConfigV1,
        payload: &[u8],
    ) -> TwoSlotStageV1 {
        let lock = TwoSlotInitFileLockV1::acquire(root, config).expect("lock test initializer");
        let mut no_fault = |_| Ok(());
        let stage = initialize_two_slot_stage(root, config, lock.identity, payload, &mut no_fault)
            .expect("create complete test stage");
        lock.release().expect("unlock test initializer");
        stage
    }

    fn try_load_test_canonical(
        root: &RootedDirectory,
        config: &TwoSlotStoreConfigV1,
    ) -> io::Result<TwoSlotSnapshotV1> {
        let lock = TwoSlotInitFileLockV1::acquire(root, config)?;
        let result = root
            .open_directory(OsStr::new(&config.store_name))
            .and_then(|directory| {
                open_existing_two_slot_store(directory, config.clone(), lock.identity)
            })
            .and_then(|store| store.load());
        let unlock = lock.release();
        match (result, unlock) {
            (Ok(snapshot), Ok(())) => Ok(snapshot),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }

    fn root_two_slot_stage_names(
        root: &RootedDirectory,
        config: &TwoSlotStoreConfigV1,
    ) -> Vec<OsString> {
        let prefix = two_slot_stage_prefix(config);
        root.child_names()
            .expect("enumerate test root")
            .into_iter()
            .filter(|name| name.as_encoded_bytes().starts_with(prefix.as_bytes()))
            .collect()
    }

    #[test]
    fn two_slot_store_initializes_noops_and_reads_shorter_payload_exactly() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("bounded-store");
        let initial = vec![0x5a; 257];
        let store = root
            .open_or_create_two_slot_store_v1(config, &initial)
            .expect("initialize two-slot store");
        let first = store.load().expect("load initial record");
        assert_eq!(first.generation(), 1);
        assert_eq!(first.payload(), initial);

        let before = store
            .slots
            .iter()
            .map(|slot| {
                read_exact_file_region(
                    &slot.handle,
                    0,
                    usize::try_from(store.layout.slot_file_bytes).expect("test slot fits usize"),
                )
                .expect("read exact slot")
            })
            .collect::<Vec<_>>();
        let no_op = store
            .compare_and_swap(&first, &initial)
            .expect("exact payload is a no-op");
        assert_eq!(no_op, first);
        let after = store
            .slots
            .iter()
            .map(|slot| {
                read_exact_file_region(
                    &slot.handle,
                    0,
                    usize::try_from(store.layout.slot_file_bytes).expect("test slot fits usize"),
                )
                .expect("read exact slot")
            })
            .collect::<Vec<_>>();
        assert_eq!(after, before, "no-op must not write either fixed slot");

        let slot_1_long = vec![0x6b; 301];
        let second = store
            .compare_and_swap(&no_op, &slot_1_long)
            .expect("commit long payload to slot one");
        let third = store
            .compare_and_swap(&second, &vec![0x7c; 299])
            .expect("advance through slot zero");
        let short = b"x";
        let fourth = store
            .compare_and_swap(&third, short)
            .expect("reuse slot one with a shorter payload");
        assert_eq!(fourth.generation(), 4);
        assert_eq!(fourth.payload(), short);
        write_exact_file_region(
            &store.slots[1].handle,
            store.layout.payload_offset + 100,
            &[0xe1],
        )
        .expect("mutate unauthenticated private stale tail");
        assert_eq!(store.load().expect("reload exact short payload"), fourth);

        let mut names = store
            .directory
            .child_names_bounded(2)
            .expect("enumerate fixed inventory");
        names.sort();
        let mut expected = TWO_SLOT_NAMES_V1.map(OsString::from).to_vec();
        expected.sort();
        assert_eq!(names, expected);
        for slot in &store.slots {
            assert_eq!(
                slot.handle.metadata().expect("slot metadata").len(),
                store.layout.slot_file_bytes
            );
        }
    }

    #[test]
    fn two_slot_store_loads_through_a_read_only_root_without_initializing() {
        let temp = tempdir().expect("tempdir");
        let config = two_slot_config("read-only-store");
        let writer_root = test_root(temp.path());
        let store = writer_root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("initialize writer store");
        let initial = store.load().expect("load initial writer record");
        store
            .compare_and_swap(&initial, b"committed")
            .expect("commit writer successor");
        drop(store);
        drop(writer_root);

        let reader_root = read_only_test_root(temp.path());
        let read_only_store =
            super::open_existing_read_only_two_slot_store_v1(&reader_root, config.clone())
                .expect("open exact store through read-only descriptors");
        let read_only_predecessor = read_only_store.load().expect("load read-only predecessor");
        for slot in &read_only_store.slots {
            let error =
                write_exact_file_region(&slot.handle, read_only_store.layout.payload_offset, b"X")
                    .expect_err("retained reader slot descriptor must reject writes");
            assert_ne!(error.kind(), io::ErrorKind::Interrupted);
        }
        assert!(
            read_only_store
                .compare_and_swap(&read_only_predecessor, b"forbidden")
                .is_err(),
            "a store reopened through read-only descriptors must not commit"
        );
        let snapshot = reader_root
            .load_existing_two_slot_store_v1(config)
            .expect("load existing store through a read-only capability");
        assert_eq!(snapshot.generation(), 2);
        assert_eq!(snapshot.payload(), b"committed");

        let absent = two_slot_config("absent-read-only-store");
        let error = reader_root
            .load_existing_two_slot_store_v1(absent.clone())
            .expect_err("read-only loading must not initialize an absent store");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(
            !temp.path().join(&absent.store_name).exists(),
            "read-only loading must not create the requested store"
        );
        assert!(
            !temp.path().join(two_slot_init_lock_name(&absent)).exists(),
            "read-only loading must not create an initializer lock"
        );
    }

    #[test]
    fn read_only_root_cannot_open_or_initialize_mutable_two_slot_store() {
        let temp = tempdir().expect("tempdir");
        let writable_root = test_root(temp.path());
        let existing_config = two_slot_config("existing-store");
        let existing_store = writable_root
            .open_or_create_two_slot_store_v1(existing_config.clone(), b"existing")
            .expect("initialize existing test store");
        let before = writable_root.child_names().expect("enumerate test root");

        let read_only_root = read_only_test_root(temp.path());
        let existing_error = read_only_root
            .open_or_create_two_slot_store_v1(existing_config, b"replacement")
            .expect_err("read-only root must not return a mutable existing store");
        assert_eq!(existing_error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            writable_root.child_names().expect("re-enumerate test root"),
            before,
            "read-only open must not create an initializer or staging artifact"
        );
        assert_eq!(
            existing_store
                .load()
                .expect("reload existing store")
                .payload(),
            b"existing",
            "read-only open must not mutate the existing store"
        );

        let absent_config = two_slot_config("absent-store");
        let absent_error = read_only_root
            .open_or_create_two_slot_store_v1(absent_config, b"initial")
            .expect_err("read-only root must not initialize an absent store");
        assert_eq!(absent_error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            writable_root
                .child_names()
                .expect("enumerate after rejection"),
            before,
            "read-only initialization must not create an init lock, stage, or canonical store"
        );
    }

    #[test]
    fn two_slot_store_remains_two_fixed_files_after_more_than_1024_updates() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("long-lived-store");
        let store = root
            .open_or_create_two_slot_store_v1(config, b"initial")
            .expect("initialize two-slot store");
        let mut snapshot = store.load().expect("load initial record");
        for generation in 0..1_025_u64 {
            let payload = format!("bounded-update-{generation:04}");
            snapshot = store
                .compare_and_swap(&snapshot, payload.as_bytes())
                .expect("commit bounded update");
        }
        assert_eq!(snapshot.generation(), 1_026);
        assert_eq!(store.load().expect("load final update"), snapshot);
        assert_eq!(
            store
                .directory
                .child_names_bounded(2)
                .expect("bounded inventory")
                .len(),
            2
        );
        let logical_bytes = store
            .slots
            .iter()
            .map(|slot| slot.handle.metadata().expect("slot metadata").len())
            .sum::<u64>();
        assert_eq!(logical_bytes, store.layout.slot_file_bytes * 2);
    }

    #[test]
    fn two_slot_initialization_recovers_after_every_injected_boundary() {
        const LABELS: &[&str] = &[
            "stage-directory-created",
            "stage-parent-synced",
            "slot-0-created",
            "slot-0-sized",
            "slot-0-sized-and-synced",
            "slot-1-created",
            "slot-1-sized",
            "slot-1-sized-and-synced",
            "slot-0-header-written",
            "slot-0-header-synced",
            "slot-1-header-written",
            "slot-1-header-synced",
            "initial-trailer-invalidated",
            "initial-trailer-invalidation-synced",
            "initial-record-written",
            "initial-record-synced",
            "initial-commit-trailer-written",
            "initial-commit-trailer-synced",
            "initial-record-readback-verified",
            "stage-directory-synced",
            "before-directory-rename",
            "directory-renamed",
            "parent-synced",
            "initialization-postcheck",
        ];

        for &fault_label in LABELS {
            let temp = tempdir().expect("tempdir");
            let root = test_root(temp.path());
            let config = two_slot_config("faulted-init");
            let error = root
                .open_or_create_two_slot_store_v1_with_init_hook(
                    config.clone(),
                    b"initial",
                    |step| {
                        if step == fault_label {
                            Err(two_slot_fault(fault_label))
                        } else {
                            Ok(())
                        }
                    },
                )
                .expect_err("fault must stop this initialization attempt");
            assert!(error.to_string().contains(fault_label));
            let recovered = root
                .open_or_create_two_slot_store_v1(config.clone(), b"initial")
                .unwrap_or_else(|error| panic!("recover after {fault_label}: {error}"));
            let snapshot = recovered
                .load()
                .unwrap_or_else(|error| panic!("load after {fault_label}: {error}"));
            assert_eq!(snapshot.generation(), 1, "fault label {fault_label}");
            assert_eq!(snapshot.payload(), b"initial", "fault label {fault_label}");
            assert!(
                root_two_slot_stage_names(&root, &config).is_empty(),
                "recovery must preserve stages in lost+found after {fault_label}"
            );
        }
    }

    #[test]
    fn two_slot_cas_recovers_after_every_injected_boundary() {
        const LABELS: &[&str] = &[
            "inactive-zero-trailer-written",
            "inactive-trailer-invalidated",
            "inactive-record-written",
            "inactive-record-synced",
            "inactive-commit-trailer-written",
            "inactive-commit-trailer-synced",
            "successor-readback-verified",
        ];
        for &fault_label in LABELS {
            let temp = tempdir().expect("tempdir");
            let root = test_root(temp.path());
            let store = root
                .open_or_create_two_slot_store_v1(two_slot_config("faulted-cas"), b"old")
                .expect("initialize CAS store");
            let old = store.load().expect("load old record");
            let error = store
                .compare_and_swap_with_test_hook(&old, b"new", |step| {
                    if step == fault_label {
                        Err(two_slot_fault(fault_label))
                    } else {
                        Ok(())
                    }
                })
                .expect_err("fault must stop CAS call");
            assert!(error.to_string().contains(fault_label));
            let observed = store.load().expect("load after CAS fault");
            let committed = matches!(
                fault_label,
                "inactive-commit-trailer-written"
                    | "inactive-commit-trailer-synced"
                    | "successor-readback-verified"
            );
            if committed {
                assert_eq!(observed.generation(), 2);
                assert_eq!(observed.payload(), b"new");
            } else {
                assert_eq!(observed, old);
                let retried = store
                    .compare_and_swap(&observed, b"new")
                    .expect("reuse torn peer slot");
                assert_eq!(retried.generation(), 2);
                assert_eq!(retried.payload(), b"new");
            }
        }
    }

    #[test]
    fn two_slot_compare_and_swap_serializes_concurrent_writers() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("concurrent-cas");
        let first_store = root
            .open_or_create_two_slot_store_v1(config.clone(), b"old")
            .expect("initialize concurrent store");
        let second_store = root
            .open_or_create_two_slot_store_v1(config, b"old")
            .expect("reopen with independent handles");
        assert!(!Arc::ptr_eq(
            &first_store.slots[0].handle,
            &second_store.slots[0].handle
        ));
        let expected = first_store.load().expect("load predecessor");
        let barrier = Arc::new(Barrier::new(3));
        let mut writers = Vec::new();
        for (store, payload) in [
            (first_store.clone(), b"left".as_slice()),
            (second_store, b"right".as_slice()),
        ] {
            let expected = expected.clone();
            let barrier = Arc::clone(&barrier);
            let payload = payload.to_vec();
            writers.push(thread::spawn(move || {
                barrier.wait();
                store.compare_and_swap(&expected, &payload)
            }));
        }
        barrier.wait();
        let results = writers
            .into_iter()
            .map(|writer| writer.join().expect("writer did not panic"))
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        let failure = results
            .iter()
            .find_map(|result| result.as_ref().err())
            .expect("one stale writer must fail");
        assert_eq!(failure.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(first_store.load().expect("load winner").generation(), 2);
    }

    #[test]
    fn two_slot_open_create_is_concurrent_and_canonical_wins() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("concurrent-init");
        let barrier = Arc::new(Barrier::new(3));
        let mut openers = Vec::new();
        for _ in 0..2 {
            let root = root.clone();
            let config = config.clone();
            let barrier = Arc::clone(&barrier);
            openers.push(thread::spawn(move || {
                barrier.wait();
                root.open_or_create_two_slot_store_v1(config, b"initial")
                    .and_then(|store| store.load())
            }));
        }
        barrier.wait();
        for opener in openers {
            let snapshot = opener
                .join()
                .expect("opener did not panic")
                .expect("concurrent open/create succeeds");
            assert_eq!(snapshot.generation(), 1);
            assert_eq!(snapshot.payload(), b"initial");
        }

        let canonical = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("open canonical");
        let current = canonical.load().expect("load canonical");
        let advanced = canonical
            .compare_and_swap(&current, b"canonical-winner")
            .expect("advance canonical");
        let extra = initialize_test_stage(&root, &config, b"initial");
        drop(extra);
        let reopened = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("canonical wins over exact race stage");
        assert_eq!(reopened.load().expect("load canonical winner"), advanced);
        assert!(root_two_slot_stage_names(&root, &config).is_empty());
        let lost = root
            .open_directory(&two_slot_lost_found_name(&config))
            .expect("race stage preserved in lost+found");
        assert_eq!(lost.child_names().expect("lost+found entries").len(), 1);
    }

    #[test]
    fn two_slot_init_file_lock_blocks_independent_handle_until_release() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("init-lock-handoff");
        let first = TwoSlotInitFileLockV1::acquire(&root, &config).expect("first init lock");
        let first_identity = first.identity;
        let (started_tx, started_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let second_root = root.clone();
        let second_config = config.clone();
        let waiter = thread::spawn(move || {
            started_tx.send(()).expect("signal waiter start");
            let second = TwoSlotInitFileLockV1::acquire(&second_root, &second_config)
                .expect("second init lock");
            acquired_tx
                .send(second.identity)
                .expect("signal second acquisition");
            second.release().expect("release second init lock");
        });
        started_rx.recv().expect("waiter started");
        assert_eq!(
            acquired_rx.recv_timeout(Duration::from_millis(150)),
            Err(mpsc::RecvTimeoutError::Timeout),
            "independent init-lock handle must block"
        );
        first.release().expect("release first init lock");
        assert_eq!(
            acquired_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("second lock acquires after handoff"),
            first_identity
        );
        waiter.join().expect("init-lock waiter did not panic");
    }

    #[test]
    fn two_slot_unrelated_store_progresses_while_another_os_lock_is_held() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let blocked = root
            .open_or_create_two_slot_store_v1(two_slot_config("blocked-store"), b"blocked")
            .expect("initialize blocked store");
        let independent = root
            .open_or_create_two_slot_store_v1(two_slot_config("independent-store"), b"independent")
            .expect("initialize independent store");
        let blocker = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(blocked.directory.display_path.join(TWO_SLOT_NAMES_V1[0]))
            .expect("open independent blocker handle");
        fs::File::lock(&blocker).expect("hold blocked store OS lock");

        let (blocked_started_tx, blocked_started_rx) = mpsc::channel();
        let blocked_wait_store = blocked.clone();
        let blocked_waiter = thread::spawn(move || {
            blocked_started_tx.send(()).expect("signal blocked load");
            blocked_wait_store.load()
        });
        blocked_started_rx.recv().expect("blocked load started");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            match blocked.process_lock.try_lock() {
                Err(std::sync::TryLockError::WouldBlock) => break,
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    drop(poisoned.into_inner());
                }
                Ok(guard) => drop(guard),
            }
            assert!(
                std::time::Instant::now() < deadline,
                "blocked load did not reach its per-store OS-lock wait"
            );
            thread::yield_now();
        }
        let (independent_tx, independent_rx) = mpsc::channel();
        let independent_waiter = thread::spawn(move || {
            independent_tx
                .send(independent.load())
                .expect("send independent result");
        });
        let independent_result = independent_rx.recv_timeout(Duration::from_secs(2));
        fs::File::unlock(&blocker).expect("release blocked store OS lock");
        let blocked_result = blocked_waiter
            .join()
            .expect("blocked waiter did not panic")
            .expect("blocked store resumes");
        independent_waiter
            .join()
            .expect("independent waiter did not panic");
        assert_eq!(blocked_result.payload(), b"blocked");
        assert_eq!(
            independent_result
                .expect("unrelated store progresses before blocker release")
                .expect("independent load succeeds")
                .payload(),
            b"independent"
        );
    }

    #[test]
    fn two_slot_empty_initial_payload_roundtrips() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let store = root
            .open_or_create_two_slot_store_v1(two_slot_config("empty-payload"), b"")
            .expect("initialize empty payload");
        let snapshot = store.load().expect("load empty payload");
        assert_eq!(snapshot.generation(), 1);
        assert!(snapshot.payload().is_empty());
        assert_eq!(
            store
                .compare_and_swap(&snapshot, b"")
                .expect("empty exact no-op"),
            snapshot
        );
    }

    #[test]
    fn two_slot_recovery_promotes_lexically_first_complete_stage() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("deterministic-init");
        let first = initialize_test_stage(&root, &config, b"initial");
        let second = initialize_test_stage(&root, &config, b"initial");
        let mut candidates = [
            (first.name.clone(), first.directory.identity),
            (second.name.clone(), second.directory.identity),
        ];
        candidates.sort_by(|left, right| left.0.cmp(&right.0));
        let expected_identity = candidates[0].1;
        drop((first, second));

        let store = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("promote deterministic stage");
        assert_eq!(store.directory.identity, expected_identity);
        assert_eq!(store.load().expect("load promoted stage").generation(), 1);
        let lost = root
            .open_directory(&two_slot_lost_found_name(&config))
            .expect("other complete stage is preserved");
        assert_eq!(lost.child_names().expect("lost+found entries").len(), 1);
    }

    #[test]
    fn two_slot_selection_rejects_ambiguous_generations_and_bad_lineage() {
        for case in ["equal", "gap", "lineage"] {
            let temp = tempdir().expect("tempdir");
            let root = test_root(temp.path());
            let store = root
                .open_or_create_two_slot_store_v1(two_slot_config(case), b"one")
                .expect("initialize record-selection case");
            let first = store.load().expect("load generation one");
            match case {
                "equal" => raw_test_record(&store, 1, 1, TWO_SLOT_ZERO_DIGEST, b"other"),
                "gap" => raw_test_record(&store, 1, 3, first.record_digest(), b"three"),
                "lineage" => raw_test_record(&store, 1, 2, [0x99; 32], b"two"),
                _ => unreachable!(),
            }
            let error = store
                .load()
                .expect_err("ambiguous history must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "case {case}");
        }
    }

    #[test]
    fn two_slot_compare_and_swap_rejects_foreign_snapshot() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let left = root
            .open_or_create_two_slot_store_v1(two_slot_config("left-store"), b"left")
            .expect("initialize left store");
        let right = root
            .open_or_create_two_slot_store_v1(two_slot_config("right-store"), b"right")
            .expect("initialize right store");
        let foreign = left.load().expect("load foreign snapshot");
        let error = right
            .compare_and_swap(&foreign, b"substitute")
            .expect_err("foreign snapshot must not authorize CAS");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            right.load().expect("right store unchanged").payload(),
            b"right"
        );
    }

    #[test]
    fn two_slot_canonical_header_corruption_fails_without_overwrite() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("corrupt-canonical");
        let store = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("initialize canonical store");
        let identity = store.directory.identity;
        let offset = u64::try_from(store.layout.header_region_bytes - 1)
            .expect("test header offset fits u64");
        write_exact_file_region(&store.slots[0].handle, offset, &[0x7f])
            .expect("corrupt immutable reserved byte");
        store.slots[0].handle.sync_all().expect("sync corruption");
        assert!(store.load().is_err());
        assert!(
            root.open_or_create_two_slot_store_v1(config.clone(), b"replacement")
                .is_err(),
            "invalid canonical must never be overwritten"
        );
        assert_eq!(
            root.open_directory(OsStr::new(&config.store_name))
                .expect("canonical remains present")
                .identity,
            identity
        );
    }

    #[test]
    fn two_slot_binding_detects_slot_substitution_and_hard_links() {
        let substitution_temp = tempdir().expect("tempdir");
        let substitution_root = test_root(substitution_temp.path());
        let substitution_store = substitution_root
            .open_or_create_two_slot_store_v1(two_slot_config("substitution"), b"initial")
            .expect("initialize substitution store");
        let canonical_path = substitution_store.directory.display_path.clone();
        let slot_path = canonical_path.join(TWO_SLOT_NAMES_V1[1]);
        let preserved_path = canonical_path.join("preserved-slot");
        fs::rename(&slot_path, &preserved_path).expect("preserve original slot");
        let replacement = fs::File::create(&slot_path).expect("create replacement slot");
        replacement
            .set_len(substitution_store.layout.slot_file_bytes)
            .expect("size replacement slot");
        #[cfg(unix)]
        fs::set_permissions(&slot_path, fs::Permissions::from_mode(0o600))
            .expect("make replacement private");
        assert!(substitution_store.load().is_err());
        assert!(preserved_path.exists());
        assert!(slot_path.exists());

        let hard_link_temp = tempdir().expect("tempdir");
        let hard_link_root = test_root(hard_link_temp.path());
        let hard_link_store = hard_link_root
            .open_or_create_two_slot_store_v1(two_slot_config("hard-link"), b"initial")
            .expect("initialize hard-link store");
        let slot_path = hard_link_store
            .directory
            .display_path
            .join(TWO_SLOT_NAMES_V1[0]);
        let alias_path = hard_link_store.directory.display_path.join("slot-alias");
        fs::hard_link(&slot_path, &alias_path).expect("create hard link");
        assert!(hard_link_store.load().is_err());
        assert!(slot_path.exists());
        assert!(alias_path.exists());
    }

    #[test]
    fn two_slot_promotion_rejects_source_substitution_and_new_hard_link() {
        let substitution_temp = tempdir().expect("tempdir");
        let substitution_root = test_root(substitution_temp.path());
        let substitution_config = two_slot_config("stage-substitution");
        let mut substituted = false;
        let mut substituted_stage_name = None;
        let result = substitution_root.open_or_create_two_slot_store_v1_with_init_hook(
            substitution_config.clone(),
            b"initial",
            |step| {
                if step == "before-directory-rename" && !substituted {
                    let stage = root_two_slot_stage_names(&substitution_root, &substitution_config)
                        .into_iter()
                        .next()
                        .expect("stage exists before promotion");
                    let stage_path = substitution_temp.path().join(&stage);
                    let detached = substitution_temp.path().join("detached-stage");
                    fs::rename(&stage_path, &detached).expect("detach exact stage");
                    fs::create_dir(&stage_path).expect("install substituted stage directory");
                    substituted_stage_name = Some(stage);
                    substituted = true;
                }
                Ok(())
            },
        );
        if let Ok(store) = result {
            assert_eq!(
                store
                    .load()
                    .expect("only the exact original may be trusted")
                    .payload(),
                b"initial"
            );
        }
        let stage_name = substituted_stage_name.expect("substitution hook ran");
        let mut candidates = vec![
            substitution_temp.path().join("detached-stage"),
            substitution_temp
                .path()
                .join(&substitution_config.store_name),
            substitution_temp.path().join(stage_name),
        ];
        let lost_path = substitution_temp
            .path()
            .join(two_slot_lost_found_name(&substitution_config));
        if lost_path.is_dir() {
            candidates.extend(
                fs::read_dir(&lost_path)
                    .expect("inspect substitution lost+found")
                    .map(|entry| entry.expect("lost+found entry").path()),
            );
        }
        let inventories = candidates
            .iter()
            .filter(|path| path.is_dir())
            .map(|path| {
                fs::read_dir(path)
                    .expect("inspect preserved object")
                    .count()
            })
            .collect::<Vec<_>>();
        assert!(inventories.iter().any(|entries| *entries == 2));
        assert!(inventories.contains(&0));

        let hard_link_temp = tempdir().expect("tempdir");
        let hard_link_root = test_root(hard_link_temp.path());
        let hard_link_config = two_slot_config("stage-hard-link");
        let mut linked = false;
        let result = hard_link_root.open_or_create_two_slot_store_v1_with_init_hook(
            hard_link_config.clone(),
            b"initial",
            |step| {
                if step == "before-directory-rename" && !linked {
                    let stage = root_two_slot_stage_names(&hard_link_root, &hard_link_config)
                        .into_iter()
                        .next()
                        .expect("stage exists before promotion");
                    let stage = hard_link_temp.path().join(stage);
                    fs::hard_link(stage.join(TWO_SLOT_NAMES_V1[0]), stage.join("slot-alias"))
                        .expect("hard-link stage slot");
                    linked = true;
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        if hard_link_root
            .open_directory(OsStr::new(&hard_link_config.store_name))
            .is_ok()
        {
            assert!(
                try_load_test_canonical(&hard_link_root, &hard_link_config).is_err(),
                "hard-linked promotion target must never be trusted"
            );
        }
        assert_eq!(
            root_two_slot_stage_names(&hard_link_root, &hard_link_config).len(),
            1
        );
    }

    #[test]
    fn two_slot_lost_found_preserves_multiple_stages_and_uses_free_slot() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("lost-found");
        let lost = root
            .open_or_create_directory(&two_slot_lost_found_name(&config))
            .expect("create lost+found");
        lost.create_child_directory_exclusive(OsStr::new("entry-v1-0000"))
            .expect("preoccupy first lost+found slot");
        let prefix = two_slot_stage_prefix(&config);
        for suffix in [
            "0000000000000000-0000000000000000",
            "0000000000000000-0000000000000001",
        ] {
            root.create_child_directory_exclusive(OsStr::new(&format!("{prefix}{suffix}")))
                .expect("create incomplete stage");
        }
        let store = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("preserve partial stages and continue");
        assert_eq!(
            store.load().expect("load initialized store").payload(),
            b"initial"
        );
        assert!(root_two_slot_stage_names(&root, &config).is_empty());
        let mut names = lost.child_names().expect("lost+found inventory");
        names.sort();
        assert_eq!(
            names,
            ["entry-v1-0000", "entry-v1-0001", "entry-v1-0002"].map(OsString::from)
        );
    }

    #[test]
    fn two_slot_lost_found_saturation_fails_without_deleting_stage() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("saturated-lost-found");
        let lost = root
            .open_or_create_directory(&two_slot_lost_found_name(&config))
            .expect("create lost+found");
        for index in 0..TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1 {
            lost.create_child_directory_exclusive(OsStr::new(&format!("entry-v1-{index:04}")))
                .expect("fill bounded lost+found");
        }
        let stage_name = OsString::from(format!(
            "{}0000000000000000-0000000000000000",
            two_slot_stage_prefix(&config)
        ));
        root.create_child_directory_exclusive(&stage_name)
            .expect("create stage requiring preservation");
        let error = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect_err("saturated lost+found must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(
            lost.child_names().expect("lost+found remains full").len(),
            16
        );
        assert_eq!(root_two_slot_stage_names(&root, &config), vec![stage_name]);
        assert!(
            root.open_directory(OsStr::new(&config.store_name)).is_err(),
            "canonical must not be installed after failed preservation"
        );
    }

    #[test]
    fn two_slot_valid_canonical_survives_saturated_lost_found_and_exact_stage() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("available-canonical");
        let canonical = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("initialize canonical");
        let initial = canonical.load().expect("load initial canonical");
        let advanced = canonical
            .compare_and_swap(&initial, b"canonical")
            .expect("advance canonical");
        let lost = root
            .open_or_create_directory(&two_slot_lost_found_name(&config))
            .expect("create lost+found");
        for index in 0..TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1 {
            lost.create_child_directory_exclusive(OsStr::new(&format!("entry-v1-{index:04}")))
                .expect("fill lost+found");
        }
        let exact_stage = initialize_test_stage(&root, &config, b"initial");
        let stage_name = exact_stage.name.clone();
        drop(exact_stage);
        let reopened = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("valid canonical remains available");
        assert_eq!(reopened.load().expect("load canonical"), advanced);
        assert_eq!(root_two_slot_stage_names(&root, &config), vec![stage_name]);
        assert_eq!(
            lost.child_names().expect("lost+found stays bounded").len(),
            16
        );
    }

    #[test]
    fn two_slot_uppercase_stage_suffix_is_rejected_and_preserved() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("lowercase-stage");
        let name = OsString::from(format!(
            "{}000000000000000A-0000000000000000",
            two_slot_stage_prefix(&config)
        ));
        root.create_child_directory_exclusive(&name)
            .expect("create uppercase lookalike stage");
        let error = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect_err("uppercase stage namespace must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(temp.path().join(&name).is_dir());
        assert!(
            root.open_directory(OsStr::new(&config.store_name)).is_err(),
            "canonical remains absent"
        );
    }

    #[test]
    fn two_slot_nonempty_lost_found_does_not_block_clean_initialization() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("nonempty-lost-found");
        let lost = root
            .open_or_create_directory(&two_slot_lost_found_name(&config))
            .expect("create lost+found");
        lost.create_child_directory_exclusive(OsStr::new("entry-v1-0000"))
            .expect("create preserved entry");
        let store = root
            .open_or_create_two_slot_store_v1(config, b"initial")
            .expect("nonempty lost+found is not a global stop");
        assert_eq!(
            store.load().expect("load clean store").payload(),
            b"initial"
        );
    }

    #[test]
    fn two_slot_preoccupied_canonical_is_never_overwritten() {
        let file_temp = tempdir().expect("tempdir");
        let file_root = test_root(file_temp.path());
        let file_config = two_slot_config("preoccupied-file");
        let file_path = file_temp.path().join(&file_config.store_name);
        fs::write(&file_path, b"sentinel").expect("preoccupy canonical file");
        #[cfg(unix)]
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o600))
            .expect("make sentinel private");
        assert!(
            file_root
                .open_or_create_two_slot_store_v1(file_config, b"initial")
                .is_err()
        );
        assert_eq!(fs::read(&file_path).expect("sentinel remains"), b"sentinel");

        let directory_temp = tempdir().expect("tempdir");
        let directory_root = test_root(directory_temp.path());
        let directory_config = two_slot_config("preoccupied-directory");
        let directory_path = directory_temp.path().join(&directory_config.store_name);
        fs::create_dir(&directory_path).expect("preoccupy canonical directory");
        fs::write(directory_path.join("sentinel"), b"keep").expect("write sentinel child");
        assert!(
            directory_root
                .open_or_create_two_slot_store_v1(directory_config, b"initial")
                .is_err()
        );
        assert_eq!(
            fs::read(directory_path.join("sentinel")).expect("sentinel child remains"),
            b"keep"
        );
    }

    #[test]
    fn two_slot_init_lock_substitution_and_hard_link_fail_closed() {
        let hard_link_temp = tempdir().expect("tempdir");
        let hard_link_root = test_root(hard_link_temp.path());
        let hard_link_config = two_slot_config("linked-init-lock");
        let hard_link_store = hard_link_root
            .open_or_create_two_slot_store_v1(hard_link_config.clone(), b"initial")
            .expect("initialize hard-link lock store");
        let lock_path = hard_link_temp
            .path()
            .join(two_slot_init_lock_name(&hard_link_config));
        let alias_path = hard_link_temp.path().join("init-lock-alias");
        fs::hard_link(&lock_path, &alias_path).expect("hard-link init lock");
        assert!(
            hard_link_root
                .open_or_create_two_slot_store_v1(hard_link_config, b"initial")
                .is_err()
        );
        assert_eq!(
            hard_link_store
                .load()
                .expect("already-open exact store remains readable")
                .payload(),
            b"initial"
        );
        assert!(lock_path.exists());
        assert!(alias_path.exists());

        let substitution_temp = tempdir().expect("tempdir");
        let substitution_root = test_root(substitution_temp.path());
        let substitution_config = two_slot_config("substituted-init-lock");
        let substitution_store = substitution_root
            .open_or_create_two_slot_store_v1(substitution_config.clone(), b"initial")
            .expect("initialize substitution lock store");
        let lock_path = substitution_temp
            .path()
            .join(two_slot_init_lock_name(&substitution_config));
        let preserved_path = substitution_temp.path().join("preserved-init-lock");
        fs::rename(&lock_path, &preserved_path).expect("preserve original init lock");
        fs::File::create(&lock_path).expect("install replacement init lock");
        #[cfg(unix)]
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600))
            .expect("make replacement init lock private");
        assert!(
            substitution_root
                .open_or_create_two_slot_store_v1(substitution_config, b"initial")
                .is_err(),
            "canonical headers bind the original init-lock identity"
        );
        assert_eq!(
            substitution_store
                .load()
                .expect("already-open exact store remains readable")
                .payload(),
            b"initial"
        );
        assert!(lock_path.exists());
        assert!(preserved_path.exists());
    }

    #[test]
    fn two_slot_cas_detects_mid_commit_slot_substitution_and_hard_link() {
        let substitution_temp = tempdir().expect("tempdir");
        let substitution_root = test_root(substitution_temp.path());
        let substitution_store = substitution_root
            .open_or_create_two_slot_store_v1(two_slot_config("cas-substitution"), b"old")
            .expect("initialize substitution CAS store");
        let expected = substitution_store.load().expect("load predecessor");
        let slot_path = substitution_store
            .directory
            .display_path
            .join(TWO_SLOT_NAMES_V1[1]);
        let preserved_path = substitution_store
            .directory
            .display_path
            .join("preserved-inactive");
        let mut substituted = false;
        let result =
            substitution_store.compare_and_swap_with_test_hook(&expected, b"new", |step| {
                if step == "inactive-zero-trailer-written" && !substituted {
                    fs::rename(&slot_path, &preserved_path).expect("preserve inactive slot");
                    let replacement = fs::File::create(&slot_path).expect("replace inactive slot");
                    replacement
                        .set_len(substitution_store.layout.slot_file_bytes)
                        .expect("size replacement slot");
                    #[cfg(unix)]
                    fs::set_permissions(&slot_path, fs::Permissions::from_mode(0o600))
                        .expect("make replacement private");
                    substituted = true;
                }
                Ok(())
            });
        assert!(result.is_err());
        assert!(slot_path.exists());
        assert!(preserved_path.exists());

        let hard_link_temp = tempdir().expect("tempdir");
        let hard_link_root = test_root(hard_link_temp.path());
        let hard_link_store = hard_link_root
            .open_or_create_two_slot_store_v1(two_slot_config("cas-hard-link"), b"old")
            .expect("initialize hard-link CAS store");
        let expected = hard_link_store.load().expect("load predecessor");
        let slot_path = hard_link_store
            .directory
            .display_path
            .join(TWO_SLOT_NAMES_V1[1]);
        let alias_path = hard_link_store
            .directory
            .display_path
            .join("inactive-alias");
        let mut linked = false;
        let result = hard_link_store.compare_and_swap_with_test_hook(&expected, b"new", |step| {
            if step == "inactive-zero-trailer-written" && !linked {
                fs::hard_link(&slot_path, &alias_path).expect("hard-link inactive slot");
                linked = true;
            }
            Ok(())
        });
        assert!(result.is_err());
        assert!(slot_path.exists());
        assert!(alias_path.exists());
    }

    #[test]
    fn two_slot_stable_nonzero_partial_trailer_fails_closed() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let store = root
            .open_or_create_two_slot_store_v1(two_slot_config("partial-record"), b"old")
            .expect("initialize partial-record store");
        let record_offset =
            u64::try_from(store.layout.header_region_bytes).expect("record offset fits u64");
        let active_record = read_exact_file_region(
            &store.slots[0].handle,
            record_offset,
            store.layout.record_header_region_bytes,
        )
        .expect("read active record header");
        write_exact_file_region(
            &store.slots[1].handle,
            record_offset,
            &active_record[..active_record.len() / 2],
        )
        .expect("write partial record header");
        let active_trailer = read_exact_file_region(
            &store.slots[0].handle,
            store.layout.trailer_offset,
            store.layout.commit_trailer_region_bytes,
        )
        .expect("read active trailer");
        write_exact_file_region(
            &store.slots[1].handle,
            store.layout.trailer_offset,
            &active_trailer[..active_trailer.len() / 2],
        )
        .expect("write partial commit trailer");
        store.slots[1].handle.sync_all().expect("sync torn bytes");
        let error = store
            .load()
            .expect_err("stable nonzero torn trailer must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn two_slot_exact_zero_trailer_allows_interrupted_body_reuse() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let store = root
            .open_or_create_two_slot_store_v1(two_slot_config("zero-trailer"), b"old")
            .expect("initialize zero-trailer store");
        let old = store.load().expect("load old record");
        let record_offset =
            u64::try_from(store.layout.header_region_bytes).expect("record offset fits u64");
        let active_record = read_exact_file_region(
            &store.slots[0].handle,
            record_offset,
            store.layout.record_header_region_bytes,
        )
        .expect("read active record header");
        write_exact_file_region(
            &store.slots[1].handle,
            record_offset,
            &active_record[..active_record.len() / 2],
        )
        .expect("write interrupted record body under zero trailer");
        store.slots[1]
            .handle
            .sync_all()
            .expect("sync interrupted body");
        assert_eq!(store.load().expect("ignore exact-zero inactive slot"), old);
        let recovered = store
            .compare_and_swap(&old, b"new")
            .expect("reuse exact-zero inactive slot");
        assert_eq!(recovered.generation(), 2);
        assert_eq!(recovered.payload(), b"new");
    }

    #[test]
    fn two_slot_newest_committed_corruption_never_falls_back() {
        for case in [
            "trailer-decode",
            "trailer-field",
            "record-digest",
            "header-decode",
            "payload",
            "oversized-length",
        ] {
            let temp = tempdir().expect("tempdir");
            let root = test_root(temp.path());
            let store = root
                .open_or_create_two_slot_store_v1(two_slot_config(case), b"old")
                .expect("initialize corruption case");
            let old = store.load().expect("load predecessor");
            let newest = store
                .compare_and_swap(&old, b"newest")
                .expect("commit newest record");
            assert_eq!(newest.generation(), 2);
            let slot = &store.slots[1];
            let record_offset =
                u64::try_from(store.layout.header_region_bytes).expect("record offset fits u64");

            match case {
                "trailer-decode" => {
                    write_exact_file_region(
                        &slot.handle,
                        store.layout.trailer_offset,
                        &vec![0xff; store.layout.commit_trailer_region_bytes],
                    )
                    .expect("corrupt trailer encoding");
                }
                "trailer-field" | "record-digest" => {
                    let bytes = read_exact_file_region(
                        &slot.handle,
                        store.layout.trailer_offset,
                        store.layout.commit_trailer_region_bytes,
                    )
                    .expect("read committed trailer");
                    let mut region: super::TwoSlotCommitTrailerRegionV1 =
                        decode_two_slot_value(&bytes, "test commit trailer")
                            .expect("decode committed trailer");
                    if case == "trailer-field" {
                        region.trailer.commit_marker[0] ^= 1;
                    } else {
                        region.trailer.record_digest[0] ^= 1;
                    }
                    let bytes = encode_two_slot_value(&region, "test commit trailer")
                        .expect("encode corrupted trailer");
                    write_exact_file_region(&slot.handle, store.layout.trailer_offset, &bytes)
                        .expect("write corrupted trailer");
                }
                "header-decode" => {
                    write_exact_file_region(
                        &slot.handle,
                        record_offset,
                        &vec![0xff; store.layout.record_header_region_bytes],
                    )
                    .expect("corrupt record header encoding");
                }
                "payload" => {
                    write_exact_file_region(&slot.handle, store.layout.payload_offset, b"X")
                        .expect("corrupt committed payload");
                }
                "oversized-length" => {
                    let bytes = read_exact_file_region(
                        &slot.handle,
                        record_offset,
                        store.layout.record_header_region_bytes,
                    )
                    .expect("read committed header");
                    let mut region: super::TwoSlotRecordHeaderRegionV1 =
                        decode_two_slot_value(&bytes, "test record header")
                            .expect("decode committed header");
                    region.header.payload_len = u64::try_from(store.config.max_payload_bytes)
                        .expect("bound fits u64")
                        .checked_add(1)
                        .expect("test bound has a successor");
                    let bytes = encode_two_slot_value(&region, "test record header")
                        .expect("encode oversized header");
                    write_exact_file_region(&slot.handle, record_offset, &bytes)
                        .expect("write oversized header");
                }
                _ => unreachable!(),
            }
            slot.handle.sync_all().expect("sync corruption");
            let error = store
                .load()
                .expect_err("newest committed corruption must not fall back");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "case {case}");
        }
    }

    #[test]
    fn two_slot_process_mutex_and_init_file_lock_recover_after_panics() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let config = two_slot_config("poison-recovery");
        let store = root
            .open_or_create_two_slot_store_v1(config.clone(), b"initial")
            .expect("initialize poison test store");
        let process_panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _: io::Result<()> = store.with_exclusive_lock(|_| panic!("poison process lock"));
        }));
        assert!(process_panic.is_err());
        assert_eq!(
            store.load().expect("recover process lock").payload(),
            b"initial"
        );
        assert!(store.process_lock.is_poisoned());

        let init_temp = tempdir().expect("tempdir");
        let init_root = test_root(init_temp.path());
        let init_config = two_slot_config("init-poison-recovery");
        let init_panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = init_root.open_or_create_two_slot_store_v1_with_init_hook(
                init_config.clone(),
                b"initial",
                |step| {
                    if step == "stage-directory-created" {
                        panic!("poison init lock");
                    }
                    Ok(())
                },
            );
        }));
        assert!(init_panic.is_err());
        let recovered = init_root
            .open_or_create_two_slot_store_v1(init_config, b"initial")
            .expect("recover init lock and partial stage");
        assert_eq!(
            recovered.load().expect("load recovered init").payload(),
            b"initial"
        );
    }

    #[cfg(target_os = "linux")]
    unsafe extern "C" {
        fn fsetxattr(
            fd: c_int,
            name: *const c_char,
            value: *const c_void,
            size: usize,
            flags: c_int,
        ) -> c_int;
        fn fremovexattr(fd: c_int, name: *const c_char) -> c_int;
    }

    #[cfg(target_os = "linux")]
    fn install_linux_default_acl(handle: &fs::File) -> CString {
        fn push_acl_entry(bytes: &mut Vec<u8>, tag: u16, permissions: u16, id: u32) {
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&permissions.to_le_bytes());
            bytes.extend_from_slice(&id.to_le_bytes());
        }

        let name = CString::new("system.posix_acl_default").expect("ACL xattr name");
        let mut acl = 2_u32.to_le_bytes().to_vec();
        let undefined_id = u32::MAX;
        push_acl_entry(&mut acl, 0x01, 0o7, undefined_id);
        push_acl_entry(&mut acl, 0x02, 0o7, 65_534);
        push_acl_entry(&mut acl, 0x04, 0o0, undefined_id);
        push_acl_entry(&mut acl, 0x10, 0o7, undefined_id);
        push_acl_entry(&mut acl, 0x20, 0o0, undefined_id);
        // SAFETY: the descriptor and NUL-terminated name are valid and the
        // ACL buffer follows Linux's fixed little-endian POSIX ACL xattr ABI.
        let installed = unsafe {
            fsetxattr(
                handle.as_raw_fd(),
                name.as_ptr(),
                acl.as_ptr().cast(),
                acl.len(),
                0,
            )
        };
        assert_eq!(
            installed,
            0,
            "install descriptor-bound POSIX default ACL: {}",
            io::Error::last_os_error()
        );
        name
    }

    #[cfg(target_os = "linux")]
    fn remove_linux_default_acl(handle: &fs::File, name: &CString) {
        // SAFETY: the retained descriptor and NUL-terminated xattr name remain
        // valid for this cleanup call.
        assert_eq!(
            unsafe { fremovexattr(handle.as_raw_fd(), name.as_ptr()) },
            0
        );
    }

    #[test]
    fn windows_dacl_qualification_source_contract_is_handle_bound() {
        let source = include_str!("governance_rooted_fs.rs");
        assert!(source.contains("#[link_name = \"GetSecurityInfo\"]"));
        assert!(source.contains("#[link_name = \"GetSecurityDescriptorControl\"]"));
        assert!(source.contains("#[link_name = \"LocalFree\"]"));
        assert!(source.contains("handle.as_raw_handle(),"));
        let pathname_api = ["GetNamed", "SecurityInfo"].concat();
        assert!(!source.contains(&pathname_api));
    }

    #[test]
    fn windows_atomic_replacement_source_contract_is_non_destructive() {
        let source = include_str!("governance_rooted_fs.rs");
        assert!(source.contains("(*info).replace_or_flags = 0;"));
        assert!(source.contains("without replacement: {error}"));
        assert!(source.contains("Windows governance existing-target replacement is disabled"));
        assert!(source.contains("metadata.number_of_links() != Some(1)"));
        let destructive_match = ["matches!(&expected, ExpectedFile::", "Identity(_))"].concat();
        assert!(!source.contains(&destructive_match));
    }

    #[test]
    fn linux_acl_stability_contract_rejects_equal_length_churn() {
        let mut snapshots = std::collections::VecDeque::from([
            b"user.a\0".to_vec(),
            b"user.b\0".to_vec(),
            b"user.a\0".to_vec(),
            b"user.b\0".to_vec(),
            b"user.a\0".to_vec(),
            b"user.b\0".to_vec(),
        ]);
        let error = super::stable_linux_acl_attribute_names(
            std::path::Path::new("synthetic-linux-directory"),
            || {
                Ok(Some(
                    snapshots
                        .pop_front()
                        .expect("bounded stability reader call"),
                ))
            },
        )
        .expect_err("equal-length ACL-name substitution must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(
            snapshots.is_empty(),
            "both snapshots in every retry are read"
        );
    }

    #[cfg(windows)]
    #[test]
    fn rooted_directory_pins_initial_windows_owner_sid() {
        let temp = tempdir().expect("tempdir");
        let mut root = test_root(temp.path());
        root.owner_sid[0] ^= 1;
        assert_eq!(
            root.verify()
                .expect_err("substituted pinned owner SID must fail closed")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn retained_directory_acl_policy_accepts_plain_directory() {
        let temp = tempdir().expect("tempdir");
        let handle = fs::File::open(temp.path()).expect("open plain directory");
        super::validate_retained_directory_acl(&handle, temp.path())
            .expect("plain descriptor has no ACL mutation grant");
    }

    #[cfg(target_os = "macos")]
    fn change_macos_acl(path: &std::path::Path, operation: &str, acl: Option<&str>) {
        let mut command = Command::new("chmod");
        command.arg(operation);
        if let Some(acl) = acl {
            command.arg(acl);
        }
        let status = command
            .arg(path)
            .status()
            .expect("execute macOS chmod ACL operation");
        assert!(status.success(), "macOS chmod ACL operation must succeed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn retained_directory_acl_policy_rejects_mutation_allow_entry() {
        let temp = tempdir().expect("tempdir");
        change_macos_acl(temp.path(), "+a", Some("everyone allow add_file"));
        let handle = fs::File::open(temp.path()).expect("open ACL directory");
        let result = super::validate_retained_directory_acl(&handle, temp.path());
        change_macos_acl(temp.path(), "-RN", None);
        let error = result.expect_err("ACL add-file grant must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn retained_directory_acl_policy_accepts_deny_only_entry() {
        let temp = tempdir().expect("tempdir");
        change_macos_acl(temp.path(), "+a", Some("everyone deny delete"));
        let handle = fs::File::open(temp.path()).expect("open deny-ACL directory");
        let result = super::validate_retained_directory_acl(&handle, temp.path());
        change_macos_acl(temp.path(), "-RN", None);
        result.expect("deny-only ACL must not grant mutation authority");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn retained_directory_acl_policy_rejects_posix_default_acl() {
        let temp = tempdir().expect("tempdir");
        let handle = fs::File::open(temp.path()).expect("open ACL directory");
        let name = install_linux_default_acl(&handle);
        let result = super::validate_retained_directory_acl(&handle, temp.path());
        remove_linux_default_acl(&handle, &name);
        let error = result.expect_err("POSIX ACL attribute must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rooted_descendant_rejects_post_capture_acl_mutation() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("child")).expect("create child");
        let root = test_root(temp.path());
        let child = root
            .open_directory(OsStr::new("child"))
            .expect("retain child");
        let name = install_linux_default_acl(&child.handle);
        let result = child.verify();
        remove_linux_default_acl(&child.handle, &name);
        assert_eq!(
            result
                .expect_err("post-capture descendant ACL must fail closed")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn rooted_descendant_rejects_post_capture_acl_mutation() {
        let temp = tempdir().expect("tempdir");
        let child_path = temp.path().join("child");
        fs::create_dir(&child_path).expect("create child");
        let root = test_root(temp.path());
        let child = root
            .open_directory(OsStr::new("child"))
            .expect("retain child");
        change_macos_acl(&child_path, "+a", Some("everyone allow add_file"));
        let result = child.verify();
        change_macos_acl(&child_path, "-RN", None);
        assert_eq!(
            result
                .expect_err("post-capture descendant ACL must fail closed")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn rooted_atomic_write_rejects_equal_length_identity_substitution() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"first").expect("seed target");
        let snapshot = root
            .read_file(OsStr::new("state"), 16)
            .expect("read original target");
        fs::remove_file(temp.path().join("state")).expect("remove original target");
        fs::write(temp.path().join("state"), b"other").expect("replace with equal length");
        let error = root
            .atomic_write(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-1"),
                b"next",
                ExpectedFile::Identity(snapshot.binding()),
            )
            .expect_err("identity substitution must fail");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read target"),
            b"other"
        );
    }

    #[test]
    fn rooted_atomic_exact_bytes_are_storage_idempotent() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"unchanged").expect("seed exact state");
        let snapshot = root
            .read_file(OsStr::new("state"), 32)
            .expect("bind exact state");

        root.atomic_write(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-9"),
            b"unchanged",
            ExpectedFile::Identity(snapshot.binding()),
        )
        .expect("exact-byte retry is a verified no-op");
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read exact state"),
            b"unchanged"
        );
        assert!(!temp.path().join(".state.tmp-1-9").exists());
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        assert!(!temp.path().join(".state.retained-v1-0000").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_write_replaces_the_exact_existing_destination() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 32)
            .expect("read predecessor");
        root.atomic_write(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-10"),
            b"successor",
            ExpectedFile::Identity(predecessor.binding()),
        )
        .expect("replace the exact existing destination");
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read successor"),
            b"successor"
        );
        assert!(!temp.path().join(".state.tmp-1-10").exists());
        assert_eq!(
            fs::read(temp.path().join(".state.retained-v1-0000"))
                .expect("read exact retained predecessor"),
            b"predecessor"
        );
        let successor = root
            .read_file(OsStr::new("state"), 32)
            .expect("retain first successor");
        root.atomic_write(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-11"),
            b"second-successor",
            ExpectedFile::Identity(successor.binding()),
        )
        .expect("use the next bounded retained-generation slot");
        assert_eq!(
            fs::read(temp.path().join(".state.retained-v1-0001"))
                .expect("read second retained predecessor"),
            b"successor"
        );
    }

    #[cfg(windows)]
    #[test]
    fn rooted_atomic_write_fails_closed_for_changed_windows_target() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 32)
            .expect("bind Windows predecessor");

        let error = root
            .atomic_write(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-10"),
                b"successor",
                ExpectedFile::Identity(predecessor.binding()),
            )
            .expect_err("Windows changed-target replacement must fail before mutation");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read untouched predecessor"),
            b"predecessor"
        );
        assert!(!temp.path().join(".state.tmp-1-10").exists());
        assert!(!temp.path().join(".state.retained-v1-0000").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_exchange_preserves_a_substituted_target_and_predecessor() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("state");
        let detached = temp.path().join("detached-predecessor");
        let temporary = temp.path().join(".state.tmp-1-20");
        fs::write(&target, b"expected-predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        let error = root
            .atomic_write_with_test_hooks(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-20"),
                b"prepared-successor",
                ExpectedFile::Identity(predecessor.binding()),
                || {
                    fs::rename(&target, &detached).expect("detach expected predecessor");
                    fs::write(&target, b"racing-replacement").expect("install replacement");
                    Ok(())
                },
                |file| file.sync_all(),
                |directory| directory.sync_all(),
            )
            .expect_err("exchange must detect the substituted predecessor");
        assert!(error.to_string().contains("substituted during exchange"));
        assert_eq!(
            fs::read(&target).expect("read target"),
            b"prepared-successor"
        );
        assert_eq!(
            fs::read(&temporary).expect("read preserved replacement"),
            b"racing-replacement"
        );
        assert_eq!(
            fs::read(&detached).expect("read detached predecessor"),
            b"expected-predecessor"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_exchange_preserves_a_substituted_prepared_object() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("state");
        let temporary = temp.path().join(".state.tmp-1-21");
        let detached_prepared = temp.path().join("detached-prepared");
        fs::write(&target, b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        root.atomic_write_with_test_hooks(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-21"),
            b"prepared-successor",
            ExpectedFile::Identity(predecessor.binding()),
            || {
                fs::rename(&temporary, &detached_prepared).expect("detach prepared object");
                fs::write(&temporary, b"racing-replacement").expect("replace prepared name");
                Ok(())
            },
            |file| file.sync_all(),
            |directory| directory.sync_all(),
        )
        .expect_err("promoted identity substitution must fail closed");
        assert_eq!(
            fs::read(&target).expect("read target"),
            b"racing-replacement"
        );
        assert_eq!(
            fs::read(&temporary).expect("read preserved predecessor"),
            b"predecessor"
        );
        assert_eq!(
            fs::read(&detached_prepared).expect("read detached prepared bytes"),
            b"prepared-successor"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_retention_never_overwrites_a_prepopulated_slot() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("state");
        let retained = temp.path().join(".state.retained-v1-0000");
        let temporary = temp.path().join(".state.tmp-1-22");
        fs::write(&target, b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        root.atomic_write_with_test_hooks(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-22"),
            b"successor",
            ExpectedFile::Identity(predecessor.binding()),
            || {
                fs::write(&retained, b"prepopulated-slot").expect("race retention slot");
                Ok(())
            },
            |file| file.sync_all(),
            |directory| directory.sync_all(),
        )
        .expect_err("exclusive retention must reject a populated destination");
        assert_eq!(fs::read(&target).expect("read target"), b"successor");
        assert_eq!(
            fs::read(&temporary).expect("read preserved predecessor"),
            b"predecessor"
        );
        assert_eq!(
            fs::read(&retained).expect("read prepopulated slot"),
            b"prepopulated-slot"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_retention_does_not_mutate_a_racing_hardlink() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("state");
        let external = temp.path().join("external-predecessor-link");
        let temporary = temp.path().join(".state.tmp-1-23");
        fs::write(&target, b"predecessor-bytes").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        root.atomic_write_with_test_hooks(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-23"),
            b"successor-bytes",
            ExpectedFile::Identity(predecessor.binding()),
            || {
                fs::hard_link(&target, &external).expect("race an external hard link");
                Ok(())
            },
            |file| file.sync_all(),
            |directory| directory.sync_all(),
        )
        .expect_err("post-check hard link must stop retention");
        assert_eq!(fs::read(&target).expect("read target"), b"successor-bytes");
        assert_eq!(
            fs::read(&temporary).expect("read exchanged predecessor"),
            b"predecessor-bytes"
        );
        assert_eq!(
            fs::read(&external).expect("read external predecessor link"),
            b"predecessor-bytes"
        );
    }

    #[test]
    fn rooted_child_binding_rejects_ancestor_replacement() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("child")).expect("create child");
        let root = test_root(temp.path());
        let child = root
            .open_directory(OsStr::new("child"))
            .expect("retain child");
        fs::rename(temp.path().join("child"), temp.path().join("original"))
            .expect("rename retained child");
        fs::create_dir(temp.path().join("child")).expect("create replacement child");
        let error = child
            .atomic_replace_current(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-2"),
                b"must-not-land",
            )
            .expect_err("substituted ancestor must fail");
        assert!(!temp.path().join("child/state").exists());
        assert!(!temp.path().join("original/state").exists());
        assert!(error.to_string().contains("substituted"));
    }

    #[cfg(unix)]
    #[test]
    fn rooted_child_open_rejects_symlink() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("outside")).expect("create outside");
        std::os::unix::fs::symlink(temp.path().join("outside"), temp.path().join("child"))
            .expect("create child symlink");
        let root = test_root(temp.path());
        assert!(
            root.open_directory(OsStr::new("child")).is_err(),
            "no-follow traversal must reject symlinks"
        );
    }

    #[test]
    fn rooted_atomic_write_propagates_directory_sync_failure() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let error = root
            .atomic_write_with_test_sync(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-3"),
                b"durability-uncertain",
                ExpectedFile::Missing,
                |file| file.sync_all(),
                |_directory| Err(io::Error::other("injected directory sync failure")),
            )
            .expect_err("directory sync failure must propagate");
        assert!(
            error
                .to_string()
                .contains("injected directory sync failure")
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_replacement_preserves_both_generations_when_exchange_sync_fails() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");
        let sync_calls = Cell::new(0_usize);

        root.atomic_write_with_test_sync(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-30"),
            b"successor",
            ExpectedFile::Identity(predecessor.binding()),
            |file| file.sync_all(),
            |_directory| {
                let call = sync_calls.get() + 1;
                sync_calls.set(call);
                if call == 1 {
                    Err(io::Error::other("injected exchange sync failure"))
                } else {
                    Ok(())
                }
            },
        )
        .expect_err("exchange directory sync failure must propagate");
        assert_eq!(sync_calls.get(), 1);
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read successor"),
            b"successor"
        );
        assert_eq!(
            fs::read(temp.path().join(".state.tmp-1-30"))
                .expect("read preserved predecessor temporary"),
            b"predecessor"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_replacement_preserves_retained_generation_when_retention_sync_fails() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");
        let sync_calls = Cell::new(0_usize);

        root.atomic_write_with_test_sync(
            OsStr::new("state"),
            OsStr::new(".state.tmp-1-31"),
            b"successor",
            ExpectedFile::Identity(predecessor.binding()),
            |file| file.sync_all(),
            |_directory| {
                let call = sync_calls.get() + 1;
                sync_calls.set(call);
                if call == 2 {
                    Err(io::Error::other("injected retention sync failure"))
                } else {
                    Ok(())
                }
            },
        )
        .expect_err("retention directory sync failure must propagate");
        assert_eq!(sync_calls.get(), 2);
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read successor"),
            b"successor"
        );
        assert_eq!(
            fs::read(temp.path().join(".state.retained-v1-0000"))
                .expect("read retained predecessor"),
            b"predecessor"
        );
        assert!(!temp.path().join(".state.tmp-1-31").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_replacement_fails_closed_when_retention_slots_are_saturated() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        for slot in 0..super::ATOMIC_RETAINED_SLOT_COUNT_V1 {
            fs::write(
                temp.path().join(format!(".state.retained-v1-{slot:04}")),
                b"retained",
            )
            .expect("fill retained-generation slot");
        }
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        let error = root
            .atomic_write(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-32"),
                b"must-not-land",
                ExpectedFile::Identity(predecessor.binding()),
            )
            .expect_err("saturated retention must fail before creating a successor");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(error.to_string().contains("offline"));
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read unchanged predecessor"),
            b"predecessor"
        );
        assert!(!temp.path().join(".state.tmp-1-32").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_replacement_enforces_retention_aggregate_byte_bound() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"predecessor").expect("seed predecessor");
        let mut retained = fs::File::create(temp.path().join(".other.retained-v1-0000"))
            .expect("seed sparse retained generation");
        retained
            .seek(SeekFrom::Start(
                super::ATOMIC_RETAINED_TOTAL_MAX_BYTES_V1 - 1,
            ))
            .expect("seek sparse retained generation");
        retained
            .write_all(&[0])
            .expect("extend sparse retained generation");
        let predecessor = root
            .read_file(OsStr::new("state"), 64)
            .expect("retain predecessor");

        let error = root
            .atomic_write(
                OsStr::new("state"),
                OsStr::new(".state.tmp-1-33"),
                b"must-not-land",
                ExpectedFile::Identity(predecessor.binding()),
            )
            .expect_err("aggregate retention bound must fail before exchange");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(error.to_string().contains("aggregate bound"));
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read unchanged predecessor"),
            b"predecessor"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_atomic_write_preserves_a_pre_rename_temporary_after_failure() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let temporary_name = OsStr::new(".state.tmp-1-4");
        let error = root
            .atomic_write_with_test_sync(
                OsStr::new("state"),
                temporary_name,
                b"preserved-for-recovery",
                ExpectedFile::Missing,
                |_file| Err(io::Error::other("injected file sync failure")),
                |_directory| Ok(()),
            )
            .expect_err("file sync failure must stop before rename");
        assert!(error.to_string().contains("injected file sync failure"));
        assert!(!temp.path().join("state").exists());
        assert_eq!(
            fs::read(temp.path().join(temporary_name))
                .expect("failed transaction temporary remains recoverable"),
            b"preserved-for-recovery"
        );
    }

    #[test]
    fn atomic_temp_candidate_classifier_is_target_exact_and_fail_closed() {
        assert!(super::is_atomic_temp_candidate_for(
            ".state.tmp-42000-1",
            "state"
        ));
        assert!(super::is_atomic_temp_candidate_for(
            ".state.tmp-malformed",
            "state"
        ));
        assert!(!super::is_atomic_temp_candidate_for(
            ".other.tmp-42000-1",
            "state"
        ));
        assert!(!super::is_atomic_temp_candidate_for(
            ".stateful.tmp-42000-1",
            "state"
        ));
        assert!(!super::is_atomic_temp_candidate_for(
            "state.tmp-42000-1",
            "state"
        ));
    }

    #[test]
    fn rooted_recovery_removes_only_matching_atomic_temporaries() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join(".state.tmp-42000-1"), b"stale").expect("seed stale temp");
        fs::write(temp.path().join(".other.tmp-42000-1"), b"other").expect("seed unrelated temp");
        assert_eq!(
            root.remove_atomic_temps_for("state")
                .expect("recover matching temp"),
            1
        );
        assert!(!temp.path().join(".state.tmp-42000-1").exists());
        assert!(temp.path().join(".other.tmp-42000-1").exists());
    }

    #[test]
    fn rooted_bounded_atomic_temp_recovery_filters_decoded_targets() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join(".state.tmp-42000-1"), b"stale").expect("seed allowed temp");
        fs::write(temp.path().join(".other.tmp-42000-2"), b"other").expect("seed rejected temp");
        fs::write(temp.path().join("retained"), b"retained").expect("seed retained file");

        assert_eq!(
            root.remove_atomic_temps_matching(3, |target| target == "state")
                .expect("recover bounded allowed temp"),
            1
        );
        assert!(!temp.path().join(".state.tmp-42000-1").exists());
        assert!(temp.path().join(".other.tmp-42000-2").exists());
        assert_eq!(
            fs::read(temp.path().join("retained")).expect("read retained file"),
            b"retained"
        );
    }

    #[test]
    fn rooted_child_enumeration_is_deterministically_sorted() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("zeta"), b"z").expect("seed zeta");
        fs::write(temp.path().join("alpha"), b"a").expect("seed alpha");
        fs::write(temp.path().join("middle"), b"m").expect("seed middle");

        assert_eq!(
            root.child_names().expect("enumerate retained directory"),
            ["alpha", "middle", "zeta"].map(OsString::from)
        );
    }

    #[test]
    fn rooted_child_enumeration_rejects_bound_overflow() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        for name in ["one", "two", "three"] {
            fs::write(temp.path().join(name), name.as_bytes()).expect("seed bounded child");
        }

        let error = root
            .child_names_bounded(2)
            .expect_err("enumeration overflow must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn rooted_empty_directory_binding_removes_empty_child() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::create_dir(temp.path().join("orphan")).expect("seed empty orphan directory");
        let retained = root
            .open_directory(OsStr::new("orphan"))
            .expect("retain empty orphan directory");

        root.remove_empty_directory_binding(retained)
            .expect("remove exact empty orphan");
        assert!(!temp.path().join("orphan").exists());
    }

    #[test]
    fn rooted_exact_file_removal_preserves_a_name_substitution() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("orphan");
        let original = temp.path().join("original");
        fs::write(&target, b"planned-orphan").expect("seed planned orphan");
        let binding = root
            .removal_file_binding(OsStr::new("orphan"), 64)
            .expect("retain planned orphan")
            .expect("planned orphan exists");
        fs::rename(&target, &original).expect("detach planned orphan");
        fs::write(&target, b"replacement").expect("install replacement");

        root.remove_file_binding(binding)
            .expect_err("exact removal must reject a substituted name");
        assert_eq!(
            fs::read(&target).expect("replacement remains"),
            b"replacement"
        );
        assert_eq!(
            fs::read(&original).expect("planned orphan remains detached"),
            b"planned-orphan"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_private_removal_binding_enforces_private_file_policy() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("private-orphan");
        fs::write(&target, b"private recovery state").expect("seed private orphan");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600))
            .expect("secure private orphan mode");

        let binding = root
            .private_removal_file_binding(OsStr::new("private-orphan"), 64)
            .expect("retain private orphan")
            .expect("private orphan exists");
        root.remove_file_binding(binding)
            .expect("remove exact private orphan");
        assert!(!target.exists());

        fs::write(&target, b"exposed recovery state").expect("seed exposed orphan");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
            .expect("set exposed orphan mode");
        let error = root
            .private_removal_file_binding(OsStr::new("private-orphan"), 64)
            .expect_err("non-private recovery state must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn rooted_exact_directory_removal_preserves_a_name_substitution() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let target = temp.path().join("orphan");
        let original = temp.path().join("original");
        fs::create_dir(&target).expect("seed planned directory");
        let retained = root
            .open_directory(OsStr::new("orphan"))
            .expect("retain planned directory");
        fs::rename(&target, &original).expect("detach planned directory");
        fs::create_dir(&target).expect("install replacement directory");

        root.remove_empty_directory_binding(retained)
            .expect_err("exact removal must reject a substituted directory name");
        assert!(target.is_dir(), "replacement directory must remain");
        assert!(original.is_dir(), "planned directory must remain detached");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_file_isolation_preserves_a_replacement_installed_at_the_destructive_gap() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create retained quarantine");
        let target = temp.path().join("orphan");
        let detached = temp.path().join("detached");
        fs::write(&target, b"planned-orphan").expect("seed planned orphan");
        let binding = root
            .removal_file_binding(OsStr::new("orphan"), 64)
            .expect("retain planned orphan")
            .expect("planned orphan exists");

        root.isolate_file_binding_with(binding, &quarantine, OsStr::new("file-slot"), || {
            fs::rename(&target, &detached).expect("detach checked inode in race hook");
            fs::write(&target, b"replacement").expect("install racing replacement");
            Ok(())
        })
        .expect_err("post-check name substitution must fail after preserving both files");
        assert_eq!(
            fs::read(&detached).expect("checked inode remains detached"),
            b"planned-orphan"
        );
        assert_eq!(
            fs::read(temp.path().join(".quarantine").join("file-slot"))
                .expect("replacement remains quarantined"),
            b"replacement"
        );
        assert!(
            !target.exists(),
            "the raced name was isolated, not unlinked"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_file_isolation_never_overwrites_a_prepopulated_destination() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create quarantine");
        fs::write(temp.path().join("orphan"), b"planned-orphan").expect("seed source");
        let binding = root
            .removal_file_binding(OsStr::new("orphan"), 64)
            .expect("retain source")
            .expect("source exists");

        root.isolate_file_binding_with(binding, &quarantine, OsStr::new("file-slot"), || {
            fs::write(
                temp.path().join(".quarantine").join("file-slot"),
                b"prepopulated",
            )
            .expect("prepopulate destination slot");
            Ok(())
        })
        .expect_err("exclusive isolation must reject a populated destination");
        assert_eq!(
            fs::read(temp.path().join("orphan")).expect("read unchanged source"),
            b"planned-orphan"
        );
        assert_eq!(
            fs::read(temp.path().join(".quarantine").join("file-slot")).expect("read destination"),
            b"prepopulated"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_file_isolation_attempts_both_parent_syncs_when_source_sync_fails() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create quarantine");
        fs::write(temp.path().join("orphan"), b"planned-orphan").expect("seed source");
        let binding = root
            .removal_file_binding(OsStr::new("orphan"), 64)
            .expect("retain source")
            .expect("source exists");
        let source_syncs = Cell::new(0_usize);
        let quarantine_syncs = Cell::new(0_usize);

        let error = root
            .isolate_file_binding_with_sync(
                binding,
                &quarantine,
                OsStr::new("file-slot"),
                || Ok(()),
                |_directory| {
                    source_syncs.set(source_syncs.get() + 1);
                    Err(io::Error::other("injected source-parent sync failure"))
                },
                |_directory| {
                    quarantine_syncs.set(quarantine_syncs.get() + 1);
                    Ok(())
                },
            )
            .expect_err("source-parent sync failure must propagate");
        assert!(error.to_string().contains("source-parent sync failure"));
        assert_eq!(source_syncs.get(), 1);
        assert_eq!(quarantine_syncs.get(), 1);
        assert!(!temp.path().join("orphan").exists());
        assert_eq!(
            fs::read(temp.path().join(".quarantine").join("file-slot"))
                .expect("read preserved quarantined source"),
            b"planned-orphan"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_file_isolation_propagates_quarantine_parent_sync_failure() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create quarantine");
        fs::write(temp.path().join("orphan"), b"planned-orphan").expect("seed source");
        let binding = root
            .removal_file_binding(OsStr::new("orphan"), 64)
            .expect("retain source")
            .expect("source exists");
        let source_syncs = Cell::new(0_usize);
        let quarantine_syncs = Cell::new(0_usize);

        let error = root
            .isolate_file_binding_with_sync(
                binding,
                &quarantine,
                OsStr::new("file-slot"),
                || Ok(()),
                |_directory| {
                    source_syncs.set(source_syncs.get() + 1);
                    Ok(())
                },
                |_directory| {
                    quarantine_syncs.set(quarantine_syncs.get() + 1);
                    Err(io::Error::other("injected quarantine-parent sync failure"))
                },
            )
            .expect_err("quarantine-parent sync failure must propagate");
        assert!(error.to_string().contains("quarantine-parent sync failure"));
        assert_eq!(source_syncs.get(), 1);
        assert_eq!(quarantine_syncs.get(), 1);
        assert!(!temp.path().join("orphan").exists());
        assert_eq!(
            fs::read(temp.path().join(".quarantine").join("file-slot"))
                .expect("read preserved quarantined source"),
            b"planned-orphan"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_directory_isolation_preserves_a_replacement_installed_at_the_destructive_gap() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create retained quarantine");
        let target = temp.path().join("orphan");
        let detached = temp.path().join("detached");
        fs::create_dir(&target).expect("seed planned directory");
        let retained = root
            .open_directory(OsStr::new("orphan"))
            .expect("retain planned directory");

        root.isolate_empty_directory_binding_with(
            retained,
            &quarantine,
            OsStr::new("directory-slot"),
            || {
                fs::rename(&target, &detached).expect("detach checked directory in race hook");
                fs::create_dir(&target).expect("install racing replacement directory");
                Ok(())
            },
        )
        .expect_err("post-check directory substitution must preserve both directories");
        assert!(detached.is_dir(), "checked directory remains detached");
        assert!(
            temp.path()
                .join(".quarantine")
                .join("directory-slot")
                .is_dir(),
            "replacement directory remains quarantined"
        );
        assert!(!target.exists(), "the raced directory was never unlinked");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_directory_isolation_attempts_both_parent_syncs_when_source_sync_fails() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create quarantine");
        fs::create_dir(temp.path().join("orphan")).expect("seed source directory");
        let child = root
            .open_directory(OsStr::new("orphan"))
            .expect("retain source directory");
        let source_syncs = Cell::new(0_usize);
        let quarantine_syncs = Cell::new(0_usize);

        let error = root
            .isolate_empty_directory_binding_with_sync(
                child,
                &quarantine,
                OsStr::new("directory-slot"),
                || Ok(()),
                |_directory| {
                    source_syncs.set(source_syncs.get() + 1);
                    Err(io::Error::other("injected directory-source sync failure"))
                },
                |_directory| {
                    quarantine_syncs.set(quarantine_syncs.get() + 1);
                    Ok(())
                },
            )
            .expect_err("directory source-parent sync failure must propagate");
        assert!(error.to_string().contains("directory-source sync failure"));
        assert_eq!(source_syncs.get(), 1);
        assert_eq!(quarantine_syncs.get(), 1);
        assert!(!temp.path().join("orphan").exists());
        assert!(
            temp.path()
                .join(".quarantine")
                .join("directory-slot")
                .is_dir()
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn rooted_directory_isolation_propagates_quarantine_parent_sync_failure() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let quarantine = root
            .open_or_create_directory(OsStr::new(".quarantine"))
            .expect("create quarantine");
        fs::create_dir(temp.path().join("orphan")).expect("seed source directory");
        let child = root
            .open_directory(OsStr::new("orphan"))
            .expect("retain source directory");
        let source_syncs = Cell::new(0_usize);
        let quarantine_syncs = Cell::new(0_usize);

        let error = root
            .isolate_empty_directory_binding_with_sync(
                child,
                &quarantine,
                OsStr::new("directory-slot"),
                || Ok(()),
                |_directory| {
                    source_syncs.set(source_syncs.get() + 1);
                    Ok(())
                },
                |_directory| {
                    quarantine_syncs.set(quarantine_syncs.get() + 1);
                    Err(io::Error::other(
                        "injected directory-quarantine sync failure",
                    ))
                },
            )
            .expect_err("directory quarantine-parent sync failure must propagate");
        assert!(
            error
                .to_string()
                .contains("directory-quarantine sync failure")
        );
        assert_eq!(source_syncs.get(), 1);
        assert_eq!(quarantine_syncs.get(), 1);
        assert!(!temp.path().join("orphan").exists());
        assert!(
            temp.path()
                .join(".quarantine")
                .join("directory-slot")
                .is_dir()
        );
    }

    #[test]
    fn rooted_empty_directory_removal_rejects_nonempty_children() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let retained = temp.path().join("retained");
        fs::create_dir(&retained).expect("seed retained directory");
        fs::write(retained.join("state"), b"retained").expect("seed retained child");
        let retained_binding = root
            .open_directory(OsStr::new("retained"))
            .expect("retain nonempty directory");

        root.remove_empty_directory_binding(retained_binding)
            .expect_err("nonempty retained directory must not be removed");
        assert_eq!(
            fs::read(retained.join("state")).expect("read retained child"),
            b"retained"
        );
    }

    #[test]
    fn rooted_read_enforces_its_byte_bound() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::write(temp.path().join("state"), b"12345").expect("seed bounded state");

        let exact = root
            .read_file(OsStr::new("state"), 5)
            .expect("read at exact byte bound");
        assert_eq!(exact.bytes(), b"12345");
        let error = root
            .read_file(OsStr::new("state"), 4)
            .expect_err("oversized state must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(windows)]
    #[test]
    fn rooted_read_rejects_windows_hardlinks() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let state = temp.path().join("state");
        fs::write(&state, b"linked-state").expect("seed Windows state");
        fs::hard_link(&state, temp.path().join("state-link"))
            .expect("create Windows governance hardlink");

        let error = root
            .read_file(OsStr::new("state"), 32)
            .expect_err("Windows governance files with multiple links must fail closed");
        assert!(error.to_string().contains("exactly one hard link"));
    }

    #[test]
    fn retained_private_file_rejects_name_substitution() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let retained = root
            .open_or_create_private_file(OsStr::new(".service.lock"), 4096)
            .expect("retain private file");
        fs::rename(
            temp.path().join(".service.lock"),
            temp.path().join("original.lock"),
        )
        .expect("detach retained file");
        fs::write(temp.path().join(".service.lock"), b"replacement")
            .expect("install name replacement");
        #[cfg(unix)]
        fs::set_permissions(
            temp.path().join(".service.lock"),
            fs::Permissions::from_mode(0o600),
        )
        .expect("secure replacement mode");

        let error = retained
            .verify()
            .expect_err("retained file substitution must fail closed");
        assert!(error.to_string().contains("substituted"));
        assert_eq!(
            fs::read(temp.path().join(".service.lock")).expect("read replacement"),
            b"replacement"
        );
    }

    #[test]
    fn rooted_recovery_is_idempotent_across_restart() {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".state.tmp-42000-7"), b"crash").expect("seed crash temporary");
        {
            let first = test_root(temp.path());
            assert_eq!(
                first
                    .remove_atomic_temps_for("state")
                    .expect("first restart recovery"),
                1
            );
        }
        let restarted = test_root(temp.path());
        assert_eq!(
            restarted
                .remove_atomic_temps_for("state")
                .expect("second restart recovery"),
            0
        );
        restarted
            .atomic_replace_current(
                OsStr::new("state"),
                OsStr::new(".state.tmp-42000-8"),
                b"restarted",
            )
            .expect("write after restart recovery");
        assert_eq!(
            fs::read(temp.path().join("state")).expect("read restarted state"),
            b"restarted"
        );
    }

    #[cfg(windows)]
    #[test]
    fn rooted_file_open_rejects_reparse_point() {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join("target"), b"target").expect("seed target");
        std::os::windows::fs::symlink_file(temp.path().join("target"), temp.path().join("linked"))
            .expect("create Windows file symlink");
        let root = test_root(temp.path());
        root.read_file(OsStr::new("linked"), 16)
            .expect_err("reparse-backed file must fail closed");
    }

    #[cfg(windows)]
    #[test]
    fn windows_disposition_deletes_the_opened_object_after_name_replacement() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let name = OsStr::new(".state.tmp-42000-2");
        let stale = temp.path().join(name);
        let moved = temp.path().join("opened-stale-object");
        fs::write(&stale, b"opened-object").expect("seed stale object");
        let opened =
            super::platform::open_file(&root.handle, name, true).expect("open exact stale object");
        let identity = super::file_identity(&opened.metadata().expect("inspect opened object"))
            .expect("capture Windows file identity");
        fs::rename(&stale, &moved).expect("move opened stale object");
        fs::write(&stale, b"name-replacement").expect("replace stale pathname");

        super::platform::remove_open_file(&root.handle, &opened, name, Some(identity))
            .expect("mark exact opened object for deletion");
        drop(opened);

        assert!(!moved.exists(), "the opened stale object must be deleted");
        assert_eq!(
            fs::read(&stale).expect("read replacement"),
            b"name-replacement",
            "a later pathname replacement must remain untouched"
        );
    }
}
