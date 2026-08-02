//! Handle-rooted filesystem operations for durable Governance DAG state.
//!
//! Production mutations are resolved component-by-component below a retained
//! directory handle. Linux and macOS use the `*at` family. Windows uses
//! `NtCreateFile` for root-directory-relative opens and
//! `SetFileInformationByHandle` for rename/disposition. Other targets fail
//! closed because they are not V1 native release targets.

use std::{
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::Arc,
};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt as _;

#[cfg(unix)]
unsafe extern "C" {
    fn geteuid() -> std::os::raw::c_uint;
}

const DEFAULT_CHILD_ENTRY_LIMIT: usize = 1_000_000;

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
        self.file_binding_with_policy(name, max_bytes, false)
    }

    /// Retain one private direct child and its exact name binding, if present.
    pub(super) fn private_file_binding(
        &self,
        name: &OsStr,
        max_bytes: usize,
    ) -> io::Result<Option<FileBinding>> {
        self.file_binding_with_policy(name, max_bytes, true)
    }

    fn file_binding_with_policy(
        &self,
        name: &OsStr,
        max_bytes: usize,
        private: bool,
    ) -> io::Result<Option<FileBinding>> {
        validate_component(name)?;
        self.verify()?;
        let handle = match platform::open_file(&self.handle, name, false) {
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

    /// Return the stable identity of a private direct regular child, if present.
    pub(super) fn private_file_identity(&self, name: &OsStr) -> io::Result<Option<FileIdentity>> {
        self.file_identity_with_policy(name, true)
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
            |file| file.sync_all(),
            |directory| directory.sync_all(),
        )
    }

    fn atomic_write_with_sync<FileSync, DirectorySync>(
        &self,
        name: &OsStr,
        temporary_name: &OsStr,
        data: &[u8],
        expected: ExpectedFile,
        mut sync_file: FileSync,
        mut sync_directory: DirectorySync,
    ) -> io::Result<()>
    where
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
            validate_regular_file_metadata(&temporary_path, &temporary_metadata)?;
            #[cfg(unix)]
            if temporary_metadata.mode() & 0o600 != 0o600 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "governance atomic temporary `{}` lacks owner read/write mode: {:o}",
                        temporary_path.display(),
                        temporary_metadata.mode() & 0o7777
                    ),
                ));
            }
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
            platform::rename_open_file(
                &self.handle,
                &temporary,
                temporary_name,
                name,
                matches!(&expected, ExpectedFile::Identity(_)),
            )
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
            renamed = true;
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
            validate_regular_file_metadata(&self.display_path.join(name), &promoted_metadata)?;
            if file_identity(&promoted_metadata)? != temporary_identity {
                return Err(io::Error::other(format!(
                    "governance atomic target `{}` is not the promoted temporary object",
                    self.display_path.join(name).display()
                )));
            }
            sync_directory(&self.handle).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "sync governance atomic directory `{}`: {error}",
                        self.display_path.display()
                    ),
                )
            })?;
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
            validate_regular_file_metadata(&self.display_path.join(name), &durable_metadata)?;
            if file_identity(&durable_metadata)? != temporary_identity {
                return Err(io::Error::other(format!(
                    "governance atomic target `{}` changed before durable readback",
                    self.display_path.join(name).display()
                )));
            }
            Ok(())
        })();
        if result.is_err() && !renamed {
            let _ = platform::remove_open_file(
                &self.handle,
                &temporary,
                temporary_name,
                file_identity(&temporary.metadata()?).ok(),
            );
        }
        result
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
    pub(super) fn remove_atomic_temps_for(&self, target_name: &str) -> io::Result<usize> {
        validate_component(OsStr::new(target_name))?;
        self.remove_atomic_temps_matching(DEFAULT_CHILD_ENTRY_LIMIT, |candidate| {
            candidate == target_name
        })
    }

    /// Remove bounded atomic crash temporaries whose decoded target is allowed.
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

    /// Remove one direct regular child by exact opened identity.
    pub(super) fn remove_file_if_exists(&self, name: &OsStr) -> io::Result<bool> {
        validate_component(name)?;
        self.verify()?;
        let file = match platform::open_file(&self.handle, name, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        let metadata = file.metadata()?;
        validate_regular_file_metadata(&self.display_path.join(name), &metadata)?;
        let identity = file_identity(&metadata)?;
        let linked = platform::open_file(&self.handle, name, false)?;
        let linked_metadata = linked.metadata()?;
        validate_regular_file_metadata(&self.display_path.join(name), &linked_metadata)?;
        if file_identity(&linked_metadata)? != identity {
            return Err(io::Error::other(format!(
                "governance state `{}` changed before deletion",
                self.display_path.join(name).display()
            )));
        }
        platform::remove_open_file(&self.handle, &file, name, Some(identity))?;
        drop(linked);
        drop(file);
        self.require_file_name_absent(name)?;
        self.handle.sync_all()?;
        self.verify()?;
        Ok(true)
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

    /// Remove one direct empty child directory by its exact retained identity.
    pub(super) fn remove_empty_directory_if_exists(&self, name: &OsStr) -> io::Result<bool> {
        validate_component(name)?;
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot remove children",
            ));
        }
        self.verify()?;
        let child = match self.open_directory(name) {
            Ok(child) => child,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        if !child.child_names_bounded(1)?.is_empty() {
            return Err(io::Error::other(format!(
                "governance directory `{}` is not empty",
                child.display_path.display()
            )));
        }
        self.verify()?;
        child.verify()?;
        let identity = child.identity;
        platform::remove_open_directory(&self.handle, &child.handle, name, Some(identity))?;
        drop(child);
        match self.open_directory(name) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(replacement) => {
                drop(replacement);
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "governance directory `{}` was replaced during removal",
                        self.display_path.join(name).display()
                    ),
                ));
            }
            Err(error) => return Err(error),
        }
        self.handle.sync_all()?;
        self.verify()?;
        Ok(true)
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
        self.atomic_write_with_sync(
            name,
            temporary_name,
            data,
            expected,
            sync_file,
            sync_directory,
        )
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

    use super::{FileIdentity, RootedDirectory, file_identity};

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
    #[cfg(target_os = "linux")]
    const AT_REMOVE_DIRECTORY: c_int = 0x200;
    #[cfg(target_os = "macos")]
    const AT_REMOVE_DIRECTORY: c_int = 0x80;
    #[cfg(target_os = "linux")]
    const RENAME_NO_REPLACE: c_uint = 1;
    #[cfg(target_os = "macos")]
    const RENAME_EXCLUSIVE: c_uint = 0x0000_0004;

    unsafe extern "C" {
        fn openat(directory: c_int, path: *const c_char, flags: c_int, ...) -> c_int;
        fn mkdirat(directory: c_int, path: *const c_char, mode: c_uint) -> c_int;
        fn renameat(
            source_directory: c_int,
            source: *const c_char,
            destination_directory: c_int,
            destination: *const c_char,
        ) -> c_int;
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
        replace: bool,
    ) -> io::Result<()> {
        let temporary_name = c_name(temporary_name)?;
        let target_name = c_name(target_name)?;
        let result = if replace {
            // SAFETY: both names are direct components below the retained
            // directory descriptor and remain valid for the call.
            unsafe {
                renameat(
                    parent.as_raw_fd(),
                    temporary_name.as_ptr(),
                    parent.as_raw_fd(),
                    target_name.as_ptr(),
                )
            }
        } else {
            #[cfg(target_os = "linux")]
            {
                // SAFETY: as above; RENAME_NOREPLACE gives atomic create-only
                // promotion when recovery expects an absent destination.
                unsafe {
                    renameat2(
                        parent.as_raw_fd(),
                        temporary_name.as_ptr(),
                        parent.as_raw_fd(),
                        target_name.as_ptr(),
                        RENAME_NO_REPLACE,
                    )
                }
            }
            #[cfg(target_os = "macos")]
            {
                // SAFETY: as above; RENAME_EXCL is the macOS create-only
                // counterpart of Linux RENAME_NOREPLACE.
                unsafe {
                    renameatx_np(
                        parent.as_raw_fd(),
                        temporary_name.as_ptr(),
                        parent.as_raw_fd(),
                        target_name.as_ptr(),
                        RENAME_EXCLUSIVE,
                    )
                }
            }
        };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub(super) fn remove_open_file(
        parent: &File,
        _file: &File,
        name: &OsStr,
        expected: Option<FileIdentity>,
    ) -> io::Result<()> {
        let linked = open_file(parent, name, false)?;
        let linked_identity = file_identity(&linked.metadata()?)?;
        if expected.is_some_and(|expected| expected != linked_identity) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance temporary changed before unlink",
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
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
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
        replace: bool,
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
            (*info).replace_or_flags = u32::from(replace);
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
        _replace: bool,
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
        fs, io,
    };

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;
    #[cfg(target_os = "macos")]
    use std::process::Command;
    #[cfg(not(windows))]
    use std::sync::Arc;
    #[cfg(target_os = "linux")]
    use std::{
        ffi::{CString, c_char, c_int, c_void},
        os::fd::AsRawFd as _,
    };

    use tempfile::tempdir;

    use super::{ExpectedFile, RootedDirectory};

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
    fn rooted_empty_directory_removal_is_identity_bound_and_idempotent() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        fs::create_dir(temp.path().join("orphan")).expect("seed empty orphan directory");

        assert!(
            root.remove_empty_directory_if_exists(OsStr::new("orphan"))
                .expect("remove exact empty orphan")
        );
        assert!(!temp.path().join("orphan").exists());
        assert!(
            !root
                .remove_empty_directory_if_exists(OsStr::new("orphan"))
                .expect("missing orphan removal is idempotent")
        );
    }

    #[test]
    fn rooted_empty_directory_removal_rejects_nonempty_children() {
        let temp = tempdir().expect("tempdir");
        let root = test_root(temp.path());
        let retained = temp.path().join("retained");
        fs::create_dir(&retained).expect("seed retained directory");
        fs::write(retained.join("state"), b"retained").expect("seed retained child");

        root.remove_empty_directory_if_exists(OsStr::new("retained"))
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
