//! Immutable private receipt staging pinned through every filesystem ancestor.

use rustix::fs::{AtFlags, FileType, Mode, OFlags};
use sorafs_manifest::signer::{
    receipt::SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1,
    stream_token::SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1,
};
use std::{
    ffi::OsString,
    fs::{File, Metadata, Permissions},
    io::Write as _,
    os::unix::{
        ffi::OsStrExt as _,
        fs::{FileExt as _, MetadataExt as _, PermissionsExt as _},
    },
    path::{Component, Path},
    sync::Mutex,
};
use zeroize::Zeroizing;

const MAX_RECORDS: usize = 65_536;
const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const SUFFIX: &str = ".receipt.norito";

/// Immutable receipt family accepted by one producer's journal.
///
/// This selects the exact shared public receipt ceiling, not an arbitrary caller budget.
/// The owning producer still verifies its full canonical receipt and authoritative completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerReceiptPurposeV1 {
    /// Exact reviewed aggregate release-manifest operation receipts.
    ReleaseManifest,
    /// Exact provider-scoped stream-token operation receipts.
    StreamToken,
}
impl SignerReceiptPurposeV1 {
    const fn max_bytes(self) -> usize {
        match self {
            Self::ReleaseManifest => SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1,
            Self::StreamToken => SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1,
        }
    }
}

/// Payload-free failure of receipt staging, bounds, private permissions or retained identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignerReceiptJournalErrorV1;
impl std::fmt::Display for SignerReceiptJournalErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("signer private receipt journal unavailable")
    }
}
impl std::error::Error for SignerReceiptJournalErrorV1 {}

struct Directory {
    name: OsString,
    file: File,
    identity: Metadata,
}

/// Mandatory durable receipt staging with no key material and no path-following fallback.
///
/// The directory must already exist, be owned by the current UID and have mode 0700. Every
/// ancestor is opened without symlink following and retained until the journal is dropped.
/// A nonblocking exclusive directory lease is held for the full journal lifetime, preventing
/// independent instances/processes from racing the aggregate retention ceiling. Unsupported
/// locking fails closed. Records are immutable, single-link mode-0400 files. Failed partial writes are retained as
/// fail-closed tombstones; automatic cleanup never removes a substituted path.
pub struct SignerReceiptJournalV1 {
    lineage: Vec<Directory>,
    owner: u32,
    mutation: Mutex<()>,
    purpose: SignerReceiptPurposeV1,
}
impl std::fmt::Debug for SignerReceiptJournalV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SignerReceiptJournalV1")
            .finish_non_exhaustive()
    }
}
impl SignerReceiptJournalV1 {
    /// Pin an existing canonical private journal directory and its complete ancestor lineage.
    ///
    /// # Errors
    /// Rejects symlinks, unsafe ownership/modes, noncanonical paths and malformed/oversized journals.
    pub fn open(
        path: &Path,
        purpose: SignerReceiptPurposeV1,
    ) -> Result<Self, SignerReceiptJournalErrorV1> {
        let fail = || SignerReceiptJournalErrorV1;
        if !path.is_absolute() {
            return Err(fail());
        }
        let mut reconstructed = std::path::PathBuf::from("/");
        let owner = rustix::process::geteuid().as_raw();
        let root = File::from(
            rustix::fs::open("/", directory_flags(), Mode::empty()).map_err(|_| fail())?,
        );
        let identity = root.metadata().map_err(|_| fail())?;
        let mut lineage = vec![Directory {
            name: OsString::from("/"),
            file: root,
            identity,
        }];
        for component in path.components() {
            let Component::Normal(name) = component else {
                if component == Component::RootDir {
                    continue;
                }
                return Err(fail());
            };
            reconstructed.push(name);
            let parent = &lineage.last().ok_or_else(fail)?.file;
            let before =
                rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|_| fail())?;
            let file = File::from(
                rustix::fs::openat(parent, name, directory_flags(), Mode::empty())
                    .map_err(|_| fail())?,
            );
            let identity = file.metadata().map_err(|_| fail())?;
            if !directory_safe(&identity, owner) || !stat_matches(&before, &identity) {
                return Err(fail());
            }
            lineage.push(Directory {
                name: name.to_owned(),
                file,
                identity,
            });
        }
        if reconstructed.as_os_str().as_bytes() != path.as_os_str().as_bytes() || lineage.len() < 2
        {
            return Err(fail());
        }
        let leaf = &lineage.last().ok_or_else(fail)?.identity;
        if leaf.uid() != owner || leaf.mode() & 0o7777 != 0o700 {
            return Err(fail());
        }
        rustix::fs::flock(
            &lineage.last().ok_or_else(fail)?.file,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        )
        .map_err(|_| fail())?;
        let journal = Self {
            lineage,
            owner,
            mutation: Mutex::new(()),
            purpose,
        };
        journal.verify_lineage()?;
        journal.inventory()?;
        Ok(journal)
    }
    /// Immutable receipt purpose, checked by the owning producer before any key operation.
    #[must_use]
    pub const fn purpose(&self) -> SignerReceiptPurposeV1 {
        self.purpose
    }
    fn directory(&self) -> &File {
        &self
            .lineage
            .last()
            .expect("validated nonempty lineage")
            .file
    }
    fn verify_lineage(&self) -> Result<(), SignerReceiptJournalErrorV1> {
        for (index, directory) in self.lineage.iter().enumerate() {
            let current = directory
                .file
                .metadata()
                .map_err(|_| SignerReceiptJournalErrorV1)?;
            if !directory_safe(&current, self.owner)
                || !same_directory(&current, &directory.identity)
            {
                return Err(SignerReceiptJournalErrorV1);
            }
            if index > 0 {
                let stat = rustix::fs::statat(
                    &self.lineage[index - 1].file,
                    &directory.name,
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(|_| SignerReceiptJournalErrorV1)?;
                if !stat_matches(&stat, &directory.identity) {
                    return Err(SignerReceiptJournalErrorV1);
                }
            }
        }
        Ok(())
    }
    fn inventory(&self) -> Result<(usize, u64), SignerReceiptJournalErrorV1> {
        self.verify_lineage()?;
        let entries = rustix::fs::Dir::read_from(self.directory())
            .map_err(|_| SignerReceiptJournalErrorV1)?;
        let mut count = 0_usize;
        let mut size = 0_u64;
        for entry in entries {
            let entry = entry.map_err(|_| SignerReceiptJournalErrorV1)?;
            let name = entry.file_name();
            if matches!(name.to_bytes(), b"." | b"..") {
                continue;
            }
            let name_text =
                std::str::from_utf8(name.to_bytes()).map_err(|_| SignerReceiptJournalErrorV1)?;
            let id = name_text
                .strip_suffix(SUFFIX)
                .ok_or(SignerReceiptJournalErrorV1)?;
            if id.len() != 64
                || !id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err(SignerReceiptJournalErrorV1);
            }
            let stat = rustix::fs::statat(self.directory(), name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|_| SignerReceiptJournalErrorV1)?;
            if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
                || stat.st_nlink != 1
                || stat.st_uid != self.owner
                || stat.st_mode & 0o7777 != 0o400
                || stat.st_size <= 0
                || stat.st_size as u64 > self.purpose.max_bytes() as u64
            {
                return Err(SignerReceiptJournalErrorV1);
            }
            count = count.checked_add(1).ok_or(SignerReceiptJournalErrorV1)?;
            size = size
                .checked_add(stat.st_size as u64)
                .ok_or(SignerReceiptJournalErrorV1)?;
            if count > MAX_RECORDS || size > MAX_TOTAL_BYTES {
                return Err(SignerReceiptJournalErrorV1);
            }
        }
        self.verify_lineage()?;
        Ok((count, size))
    }
    pub(super) fn stage(
        &self,
        operation_id: [u8; 32],
        bytes: &[u8],
    ) -> Result<PinnedReceipt<'_>, SignerReceiptJournalErrorV1> {
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| SignerReceiptJournalErrorV1)?;
        if operation_id == [0; 32] || bytes.is_empty() || bytes.len() > self.purpose.max_bytes() {
            return Err(SignerReceiptJournalErrorV1);
        }
        let (count, size) = self.inventory()?;
        if count >= MAX_RECORDS || size + bytes.len() as u64 > MAX_TOTAL_BYTES {
            return Err(SignerReceiptJournalErrorV1);
        }
        let name = format!("{}{SUFFIX}", hex::encode(operation_id));
        let fd = rustix::fs::openat(
            self.directory(),
            name.as_str(),
            OFlags::RDWR
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK
                | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| SignerReceiptJournalErrorV1)?;
        let mut file = File::from(fd);
        file.write_all(bytes)
            .map_err(|_| SignerReceiptJournalErrorV1)?;
        file.set_permissions(Permissions::from_mode(0o400))
            .map_err(|_| SignerReceiptJournalErrorV1)?;
        file.sync_all().map_err(|_| SignerReceiptJournalErrorV1)?;
        self.directory()
            .sync_all()
            .map_err(|_| SignerReceiptJournalErrorV1)?;
        let identity = file.metadata().map_err(|_| SignerReceiptJournalErrorV1)?;
        let pinned = PinnedReceipt {
            journal: self,
            name,
            file,
            identity,
            bytes: Zeroizing::new(bytes.to_vec()),
        };
        pinned.recheck()?;
        Ok(pinned)
    }
    pub(super) fn recover(
        &self,
        operation_id: [u8; 32],
    ) -> Result<PinnedReceipt<'_>, SignerReceiptJournalErrorV1> {
        self.verify_lineage()?;
        if operation_id == [0; 32] {
            return Err(SignerReceiptJournalErrorV1);
        }
        let name = format!("{}{SUFFIX}", hex::encode(operation_id));
        let file = File::from(
            rustix::fs::openat(
                self.directory(),
                name.as_str(),
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| SignerReceiptJournalErrorV1)?,
        );
        let identity = file.metadata().map_err(|_| SignerReceiptJournalErrorV1)?;
        let bytes = read_stable(&file, &identity, self.owner, self.purpose)?;
        let pinned = PinnedReceipt {
            journal: self,
            name,
            file,
            identity,
            bytes,
        };
        pinned.recheck()?;
        Ok(pinned)
    }
}

pub(super) struct PinnedReceipt<'a> {
    journal: &'a SignerReceiptJournalV1,
    name: String,
    file: File,
    identity: Metadata,
    bytes: Zeroizing<Vec<u8>>,
}
impl PinnedReceipt<'_> {
    pub(super) fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
    pub(super) fn recheck(&self) -> Result<(), SignerReceiptJournalErrorV1> {
        self.journal.verify_lineage()?;
        let stat = rustix::fs::statat(
            self.journal.directory(),
            self.name.as_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| SignerReceiptJournalErrorV1)?;
        if !stat_matches(&stat, &self.identity)
            || read_stable(
                &self.file,
                &self.identity,
                self.journal.owner,
                self.journal.purpose,
            )?
            .as_slice()
                != self.bytes.as_slice()
        {
            return Err(SignerReceiptJournalErrorV1);
        }
        self.journal.verify_lineage()
    }
}
fn directory_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC
}
fn directory_safe(metadata: &Metadata, owner: u32) -> bool {
    metadata.is_dir()
        && (metadata.uid() == 0 || metadata.uid() == owner)
        && (metadata.mode() & 0o022 == 0 || (metadata.uid() == 0 && metadata.mode() & 0o1000 != 0))
}
fn same_directory(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
}
fn stat_matches(stat: &rustix::fs::Stat, metadata: &Metadata) -> bool {
    stat.st_dev as u64 == metadata.dev()
        && stat.st_ino as u64 == metadata.ino()
        && stat.st_mode as u32 == metadata.mode()
        && stat.st_uid == metadata.uid()
}
fn read_stable(
    file: &File,
    expected: &Metadata,
    owner: u32,
    purpose: SignerReceiptPurposeV1,
) -> Result<Zeroizing<Vec<u8>>, SignerReceiptJournalErrorV1> {
    let before = file.metadata().map_err(|_| SignerReceiptJournalErrorV1)?;
    if !before.is_file()
        || before.nlink() != 1
        || before.uid() != owner
        || before.mode() & 0o7777 != 0o400
        || before.len() == 0
        || before.len() > purpose.max_bytes() as u64
        || !same_file(&before, expected)
    {
        return Err(SignerReceiptJournalErrorV1);
    }
    let mut bytes = Zeroizing::new(vec![0; before.len() as usize]);
    let mut offset = 0;
    while offset < bytes.len() {
        let count = file
            .read_at(&mut bytes[offset..], offset as u64)
            .map_err(|_| SignerReceiptJournalErrorV1)?;
        if count == 0 {
            return Err(SignerReceiptJournalErrorV1);
        }
        offset += count;
    }
    let after = file.metadata().map_err(|_| SignerReceiptJournalErrorV1)?;
    if !same_file(&before, &after) {
        return Err(SignerReceiptJournalErrorV1);
    }
    Ok(bytes)
}
fn same_file(left: &Metadata, right: &Metadata) -> bool {
    same_directory(left, right)
        && left.len() == right.len()
        && left.nlink() == right.nlink()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(test)]
mod tests;
