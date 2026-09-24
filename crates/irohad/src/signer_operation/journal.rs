//! Immutable private receipt staging pinned through every filesystem ancestor.

use rustix::fs::{AtFlags, FileType, Mode, OFlags};
use sorafs_manifest::signer::{
    final_promotion::SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1,
    receipt::SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1,
    stream_token::SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1,
};
use std::{
    collections::HashSet,
    ffi::OsString,
    fs::{File, Metadata, Permissions},
    io::Write as _,
    os::unix::{
        ffi::OsStrExt as _,
        fs::{FileExt as _, MetadataExt as _, PermissionsExt as _},
    },
    path::{Component, Path},
    sync::{Arc, Mutex},
};
use zeroize::Zeroizing;

const MAX_RECORDS: usize = 65_536;
const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const SUFFIX: &str = ".receipt.norito";
const PENDING_RESERVE_SUFFIX: &str = ".pending-reserve.norito";
const PENDING_RESERVE_IN_PROGRESS_SUFFIX: &str = ".pending-reserve.inflight";
const PENDING_RESERVE_DIRECTORY: &str = "pending-reserve-v1";
const PENDING_RESERVE_MAX_RECORD_BYTES: usize = 136 * 1024;

#[derive(Clone, Copy)]
struct JournalProfile {
    suffix: &'static str,
    in_progress_suffix: Option<&'static str>,
    required_leaf: Option<&'static str>,
    max_record_bytes: usize,
    max_records: usize,
    max_total_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingReserveCheckpoint {
    TombstoneDurable,
    SignedBytesDurable,
}
impl JournalProfile {
    const fn receipt(purpose: SignerReceiptPurposeV1) -> Self {
        Self {
            suffix: SUFFIX,
            in_progress_suffix: None,
            required_leaf: None,
            max_record_bytes: purpose.max_bytes(),
            max_records: MAX_RECORDS,
            max_total_bytes: MAX_TOTAL_BYTES,
        }
    }

    const PENDING_RESERVE: Self = Self {
        suffix: PENDING_RESERVE_SUFFIX,
        in_progress_suffix: Some(PENDING_RESERVE_IN_PROGRESS_SUFFIX),
        required_leaf: Some(PENDING_RESERVE_DIRECTORY),
        max_record_bytes: PENDING_RESERVE_MAX_RECORD_BYTES,
        max_records: 4096,
        max_total_bytes: MAX_TOTAL_BYTES,
    };
}

/// Immutable receipt family accepted by one producer's journal.
///
/// This selects the exact shared public receipt ceiling, not an arbitrary caller budget.
/// The owning producer still verifies its full canonical receipt and authoritative completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerReceiptPurposeV1 {
    /// Exact reviewed final production-promotion provenance receipts.
    FinalPromotionProvenance,
    /// Exact reviewed aggregate release-manifest operation receipts.
    ReleaseManifest,
    /// Exact provider-scoped stream-token operation receipts.
    StreamToken,
}
impl SignerReceiptPurposeV1 {
    const fn max_bytes(self) -> usize {
        match self {
            Self::FinalPromotionProvenance => SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1,
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
/// ancestor is opened without symlink following and retained by the writer, its read-only
/// capabilities and every pinned receipt. A nonblocking exclusive directory lease lasts until
/// their final shared owner drops, preventing independent instances/processes from racing the
/// aggregate retention ceiling. Unsupported locking fails closed. Records are immutable,
/// single-link mode-0400 files. Failed partial writes remain fail-closed tombstones;
/// automatic cleanup never removes a substituted path.
pub struct SignerReceiptJournalV1 {
    reader: SignerReceiptJournalReaderV1,
}

struct JournalInner {
    lineage: Vec<Directory>,
    owner: u32,
    mutation: Mutex<()>,
    profile: JournalProfile,
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
        let inner = JournalInner::open(path, JournalProfile::receipt(purpose))?;
        Ok(Self {
            reader: SignerReceiptJournalReaderV1 { inner, purpose },
        })
    }
    /// Immutable receipt purpose, checked by the owning producer before any key operation.
    #[must_use]
    pub fn purpose(&self) -> SignerReceiptPurposeV1 {
        self.reader.purpose()
    }
    /// Grant read-only access to this exact existing lease; never reopen the path.
    pub(super) fn reader(&self) -> SignerReceiptJournalReaderV1 {
        SignerReceiptJournalReaderV1 {
            inner: Arc::clone(&self.reader.inner),
            purpose: self.reader.purpose,
        }
    }
    pub(super) fn stage(
        &self,
        operation_id: [u8; 32],
        bytes: &[u8],
    ) -> Result<PinnedReceipt, SignerReceiptJournalErrorV1> {
        self.reader.inner.stage(operation_id, bytes)
    }
    pub(super) fn recover(
        &self,
        operation_id: [u8; 32],
    ) -> Result<PinnedReceipt, SignerReceiptJournalErrorV1> {
        self.reader().recover(operation_id)
    }
}

/// Separate pending-Reserve directory lease over the same pinned immutable-file mechanism.
///
/// This is not a receipt purpose and cannot be passed to a receipt producer. Recovery exposes
/// bytes only; it has no signing or transport entry point.
pub(super) struct SignerPendingReserveFilesV1 {
    inner: Arc<JournalInner>,
}
impl SignerPendingReserveFilesV1 {
    /// Open the dedicated owner-only `pending-reserve-v1` directory and pin every ancestor.
    pub(super) fn open(path: &Path) -> Result<Self, SignerReceiptJournalErrorV1> {
        Ok(Self {
            inner: JournalInner::open(path, JournalProfile::PENDING_RESERVE)?,
        })
    }

    /// Durably stage one immutable operation record and retain its exact file identity.
    pub(super) fn stage(
        &self,
        operation_id: [u8; 32],
        bytes: &[u8],
    ) -> Result<PinnedReceipt, SignerReceiptJournalErrorV1> {
        self.inner.stage_pending_reserve(operation_id, bytes)
    }

    /// Read back one already staged operation without creating submission authority.
    pub(super) fn recover(
        &self,
        operation_id: [u8; 32],
    ) -> Result<PinnedReceipt, SignerReceiptJournalErrorV1> {
        self.inner.recover(operation_id)
    }
}

impl JournalInner {
    fn open(
        path: &Path,
        profile: JournalProfile,
    ) -> Result<Arc<Self>, SignerReceiptJournalErrorV1> {
        let fail = || SignerReceiptJournalErrorV1;
        if !path.is_absolute()
            || profile
                .required_leaf
                .is_some_and(|leaf| path.file_name() != Some(std::ffi::OsStr::new(leaf)))
        {
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
        let inner = Arc::new(JournalInner {
            lineage,
            owner,
            mutation: Mutex::new(()),
            profile,
        });
        inner.verify_lineage()?;
        inner.inventory()?;
        Ok(inner)
    }
}

/// Read-only capability over an already opened journal's exact directory lease.
///
/// There is no path constructor, write method, dereference, or writer conversion. Only the
/// owning signer-operation module can obtain this capability; receipt bytes remain internal.
pub(super) struct SignerReceiptJournalReaderV1 {
    inner: Arc<JournalInner>,
    purpose: SignerReceiptPurposeV1,
}
impl SignerReceiptJournalReaderV1 {
    /// Exact receipt-family ceiling retained by the original writer's lease.
    pub(super) fn purpose(&self) -> SignerReceiptPurposeV1 {
        self.purpose
    }
    /// Pin bounded untrusted receipt bytes; semantic verification remains the purpose owner.
    pub(super) fn recover(
        &self,
        operation_id: [u8; 32],
    ) -> Result<PinnedReceipt, SignerReceiptJournalErrorV1> {
        self.inner.recover(operation_id)
    }
}

impl JournalInner {
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
        let mut seen_pending_ids = HashSet::new();
        if self.profile.in_progress_suffix.is_some() {
            seen_pending_ids
                .try_reserve(self.profile.max_records)
                .map_err(|_| SignerReceiptJournalErrorV1)?;
        }
        for entry in entries {
            let entry = entry.map_err(|_| SignerReceiptJournalErrorV1)?;
            let name = entry.file_name();
            if matches!(name.to_bytes(), b"." | b"..") {
                continue;
            }
            let name_text =
                std::str::from_utf8(name.to_bytes()).map_err(|_| SignerReceiptJournalErrorV1)?;
            let (id, in_progress) = if let Some(id) = name_text.strip_suffix(self.profile.suffix) {
                (id, false)
            } else if let Some(id) = self
                .profile
                .in_progress_suffix
                .and_then(|suffix| name_text.strip_suffix(suffix))
            {
                (id, true)
            } else {
                return Err(SignerReceiptJournalErrorV1);
            };
            if id.len() != 64
                || !id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err(SignerReceiptJournalErrorV1);
            }
            if self.profile.in_progress_suffix.is_some() {
                let mut operation_id = [0; 32];
                hex::decode_to_slice(id, &mut operation_id)
                    .map_err(|_| SignerReceiptJournalErrorV1)?;
                if seen_pending_ids.len() >= self.profile.max_records
                    || !seen_pending_ids.insert(operation_id)
                {
                    return Err(SignerReceiptJournalErrorV1);
                }
            }
            let stat = rustix::fs::statat(self.directory(), name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|_| SignerReceiptJournalErrorV1)?;
            let mode = stat.st_mode & 0o7777;
            if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
                || stat.st_nlink != 1
                || stat.st_uid != self.owner
                || if in_progress {
                    // Creation is masked by the process umask until the file is chmodded.
                    mode & !0o600 != 0
                } else {
                    mode != 0o400 || stat.st_size <= 0
                }
                || stat.st_size < 0
                || stat.st_size as u64 > self.profile.max_record_bytes as u64
            {
                return Err(SignerReceiptJournalErrorV1);
            }
            count = count.checked_add(1).ok_or(SignerReceiptJournalErrorV1)?;
            size = size
                .checked_add(stat.st_size as u64)
                .ok_or(SignerReceiptJournalErrorV1)?;
            if count > self.profile.max_records || size > self.profile.max_total_bytes {
                return Err(SignerReceiptJournalErrorV1);
            }
        }
        self.verify_lineage()?;
        Ok((count, size))
    }
    fn stage(
        self: &Arc<Self>,
        operation_id: [u8; 32],
        bytes: &[u8],
    ) -> Result<PinnedReceipt, SignerReceiptJournalErrorV1> {
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| SignerReceiptJournalErrorV1)?;
        if operation_id == [0; 32]
            || bytes.is_empty()
            || bytes.len() > self.profile.max_record_bytes
        {
            return Err(SignerReceiptJournalErrorV1);
        }
        let (count, size) = self.inventory()?;
        if count >= self.profile.max_records
            || size + bytes.len() as u64 > self.profile.max_total_bytes
        {
            return Err(SignerReceiptJournalErrorV1);
        }
        let name = format!("{}{}", hex::encode(operation_id), self.profile.suffix);
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
            journal: Arc::clone(self),
            name,
            file,
            identity,
            bytes: Zeroizing::new(bytes.to_vec()),
        };
        pinned.recheck()?;
        Ok(pinned)
    }

    fn stage_pending_reserve(
        self: &Arc<Self>,
        operation_id: [u8; 32],
        bytes: &[u8],
    ) -> Result<PinnedReceipt, SignerReceiptJournalErrorV1> {
        self.stage_pending_reserve_with(operation_id, bytes, |_| Ok(()))
    }

    fn stage_pending_reserve_with(
        self: &Arc<Self>,
        operation_id: [u8; 32],
        bytes: &[u8],
        mut checkpoint: impl FnMut(PendingReserveCheckpoint) -> rustix::io::Result<()>,
    ) -> Result<PinnedReceipt, SignerReceiptJournalErrorV1> {
        let fail = || SignerReceiptJournalErrorV1;
        if self.profile.in_progress_suffix != Some(PENDING_RESERVE_IN_PROGRESS_SUFFIX)
            || !cfg!(any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "redox"
            ))
        {
            return Err(fail());
        }
        let _guard = self.mutation.lock().map_err(|_| fail())?;
        if operation_id == [0; 32]
            || bytes.is_empty()
            || bytes.len() > self.profile.max_record_bytes
        {
            return Err(fail());
        }
        let (count, size) = self.inventory()?;
        if count >= self.profile.max_records
            || size + bytes.len() as u64 > self.profile.max_total_bytes
        {
            return Err(fail());
        }
        let id = hex::encode(operation_id);
        let final_name = format!("{}{PENDING_RESERVE_SUFFIX}", id);
        let in_progress_name = format!("{}{PENDING_RESERVE_IN_PROGRESS_SUFFIX}", id);
        match rustix::fs::statat(
            self.directory(),
            final_name.as_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Err(rustix::io::Errno::NOENT) => {}
            _ => return Err(fail()),
        }
        let fd = rustix::fs::openat(
            self.directory(),
            in_progress_name.as_str(),
            OFlags::RDWR
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK
                | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| fail())?;
        let mut file = File::from(fd);
        // This name is a durable one-use tombstone even if writing the signed bytes fails.
        self.directory().sync_all().map_err(|_| fail())?;
        checkpoint(PendingReserveCheckpoint::TombstoneDurable).map_err(|_| fail())?;
        file.write_all(bytes).map_err(|_| fail())?;
        file.set_permissions(Permissions::from_mode(0o400))
            .map_err(|_| fail())?;
        file.sync_all().map_err(|_| fail())?;
        let identity = file.metadata().map_err(|_| fail())?;
        let mut pinned = PinnedReceipt {
            journal: Arc::clone(self),
            name: in_progress_name,
            file,
            identity,
            bytes: Zeroizing::new(bytes.to_vec()),
        };
        pinned.recheck()?;
        checkpoint(PendingReserveCheckpoint::SignedBytesDurable).map_err(|_| fail())?;
        publish_pending_reserve_no_replace(self.directory(), &pinned.name, &final_name)?;
        pinned.name = final_name;
        self.directory().sync_all().map_err(|_| fail())?;
        // Rename may change ctime. Retain the pre-rename inode/shape check, then pin its
        // post-publication metadata before the final path and byte readback.
        let published_identity = pinned.file.metadata().map_err(|_| fail())?;
        if !same_directory(&pinned.identity, &published_identity)
            || pinned.identity.len() != published_identity.len()
            || pinned.identity.nlink() != published_identity.nlink()
        {
            return Err(fail());
        }
        pinned.identity = published_identity;
        pinned.recheck()?;
        Ok(pinned)
    }

    fn recover(
        self: &Arc<Self>,
        operation_id: [u8; 32],
    ) -> Result<PinnedReceipt, SignerReceiptJournalErrorV1> {
        self.verify_lineage()?;
        if operation_id == [0; 32] {
            return Err(SignerReceiptJournalErrorV1);
        }
        let name = format!("{}{}", hex::encode(operation_id), self.profile.suffix);
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
        let bytes = read_stable(&file, &identity, self.owner, self.profile)?;
        let pinned = PinnedReceipt {
            journal: Arc::clone(self),
            name,
            file,
            identity,
            bytes,
        };
        pinned.recheck()?;
        Ok(pinned)
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "redox"
))]
fn publish_pending_reserve_no_replace(
    directory: &File,
    in_progress_name: &str,
    final_name: &str,
) -> Result<(), SignerReceiptJournalErrorV1> {
    rustix::fs::renameat_with(
        directory,
        in_progress_name,
        directory,
        final_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|_| SignerReceiptJournalErrorV1)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "redox"
)))]
fn publish_pending_reserve_no_replace(
    _directory: &File,
    _in_progress_name: &str,
    _final_name: &str,
) -> Result<(), SignerReceiptJournalErrorV1> {
    Err(SignerReceiptJournalErrorV1)
}

/// Exact immutable byte snapshot retaining the original journal lease until drop.
pub(super) struct PinnedReceipt {
    journal: Arc<JournalInner>,
    name: String,
    file: File,
    identity: Metadata,
    bytes: Zeroizing<Vec<u8>>,
}
impl PinnedReceipt {
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
                self.journal.profile,
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
    profile: JournalProfile,
) -> Result<Zeroizing<Vec<u8>>, SignerReceiptJournalErrorV1> {
    let before = file.metadata().map_err(|_| SignerReceiptJournalErrorV1)?;
    if !before.is_file()
        || before.nlink() != 1
        || before.uid() != owner
        || before.mode() & 0o7777 != 0o400
        || before.len() == 0
        || before.len() > profile.max_record_bytes as u64
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
