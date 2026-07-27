//! Internal durable state machine and atomic checkpoint store for transaction forwarders.
//!
//! Domain wrappers retain responsibility for validating the exact native
//! instruction and its finalized-chain reconciliation result. This module owns
//! only the reusable crash states and the hardened single-writer persistence
//! protocol.

use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write as _},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[cfg(unix)]
use std::os::unix::fs::{
    DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
};
#[cfg(windows)]
use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};

use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

static CHECKPOINT_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static CHECKPOINT_PROCESS_LOCKS: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

#[cfg(any(target_os = "linux", target_os = "android"))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0x0002_0000 | 0x0008_0000;
#[cfg(any(target_os = "macos", target_os = "ios"))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0x0000_0100 | 0x0100_0000;
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
#[cfg(windows)]
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

/// Durable crash state shared by native transaction forwarders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub(crate) enum StoredDeliveryStateV1 {
    /// Semantic material exists but no signer may currently own it.
    Ready,
    /// A signer owns the material and cannot submit it directly.
    Signing,
    /// Exact signed bytes are durable and have not been exposed to a submitter.
    Signed,
    /// Submission may have happened and must be reconciled before retry.
    Ambiguous,
    /// The exact transaction is known pending or applied.
    Submitted,
}

/// Minimal mutable record required by the durable delivery state machine.
pub(crate) trait DeliveryRecord {
    /// Exact signed envelope retained by this domain.
    type Transaction: Clone;

    /// Current durable state.
    fn delivery_state(&self) -> StoredDeliveryStateV1;
    /// Replace the durable state.
    fn set_delivery_state(&mut self, state: StoredDeliveryStateV1);
    /// Attempts consumed by signing and proven-absent resubmission.
    fn attempts(&self) -> u32;
    /// Replace the attempt counter.
    fn set_attempts(&mut self, attempts: u32);
    /// Finalized height preceding the current transaction attempt.
    fn baseline_finalized_height(&self) -> u64;
    /// Replace the finalized baseline height.
    fn set_baseline_finalized_height(&mut self, height: u64);
    /// Finalized hash paired with the baseline height.
    fn baseline_finalized_block_hash(&self) -> [u8; 32];
    /// Replace the finalized baseline hash.
    fn set_baseline_finalized_block_hash(&mut self, block_hash: [u8; 32]);
    /// Borrow the exact signed envelope, when present.
    fn signed_transaction(&self) -> Option<&Self::Transaction>;
    /// Replace the exact signed envelope.
    fn set_signed_transaction(&mut self, transaction: Option<Self::Transaction>);
}

/// Finalized block anchor used to prove retry absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FinalizedCursorV1 {
    pub(crate) height: u64,
    pub(crate) block_hash: [u8; 32],
}

/// Result of a bounded transition that can exhaust its attempt budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetryBoundOutcome {
    /// The entry remains pending.
    Pending,
    /// The wrapper must atomically move the entry to its dead-letter set.
    Exhausted,
}

/// State-machine transition error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum DeliveryTransitionError {
    /// A finalized cursor is zero.
    #[error("finalized cursor is invalid")]
    InvalidFinalizedCursor,
    /// The requested transition is unsafe from the current crash state.
    #[error("delivery transition is invalid")]
    InvalidTransition,
    /// The attempt counter overflowed or was already exhausted.
    #[error("delivery retry bound is exhausted")]
    RetryExhausted,
}

/// Validate a non-zero finalized cursor.
pub(crate) fn validate_finalized_cursor(
    cursor: FinalizedCursorV1,
) -> Result<(), DeliveryTransitionError> {
    if cursor.height == 0 || cursor.block_hash == [0; 32] {
        return Err(DeliveryTransitionError::InvalidFinalizedCursor);
    }
    Ok(())
}

/// Reset a signer-only crash state, for which submission was impossible.
pub(crate) fn recover_interrupted_signing<R: DeliveryRecord>(entry: &mut R) -> bool {
    if entry.delivery_state() != StoredDeliveryStateV1::Signing {
        return false;
    }
    entry.set_delivery_state(StoredDeliveryStateV1::Ready);
    entry.set_baseline_finalized_height(0);
    entry.set_baseline_finalized_block_hash([0; 32]);
    true
}

/// Validate the structural invariants of one stored delivery.
pub(crate) fn validate_delivery<R: DeliveryRecord>(entry: &R, max_attempts: u32) -> bool {
    let has_baseline =
        entry.baseline_finalized_height() != 0 && entry.baseline_finalized_block_hash() != [0; 32];
    let has_empty_baseline =
        entry.baseline_finalized_height() == 0 && entry.baseline_finalized_block_hash() == [0; 32];
    let valid_state = match entry.delivery_state() {
        StoredDeliveryStateV1::Ready => has_empty_baseline && entry.signed_transaction().is_none(),
        StoredDeliveryStateV1::Signing => has_baseline && entry.signed_transaction().is_none(),
        StoredDeliveryStateV1::Signed
        | StoredDeliveryStateV1::Ambiguous
        | StoredDeliveryStateV1::Submitted => {
            has_baseline && entry.signed_transaction().is_some() && entry.attempts() != 0
        }
    };
    valid_state && entry.attempts() <= max_attempts
}

/// Durably claim a ready entry before invoking an isolated signer.
pub(crate) fn claim_for_signing<R: DeliveryRecord>(
    entry: &mut R,
    cursor: FinalizedCursorV1,
    max_attempts: u32,
) -> Result<(), DeliveryTransitionError> {
    validate_finalized_cursor(cursor)?;
    if entry.delivery_state() != StoredDeliveryStateV1::Ready
        || entry.signed_transaction().is_some()
    {
        return Err(DeliveryTransitionError::InvalidTransition);
    }
    if entry.attempts() >= max_attempts {
        return Err(DeliveryTransitionError::RetryExhausted);
    }
    entry.set_baseline_finalized_height(cursor.height);
    entry.set_baseline_finalized_block_hash(cursor.block_hash);
    entry.set_delivery_state(StoredDeliveryStateV1::Signing);
    Ok(())
}

/// Attach an exact signed envelope to an isolated signing claim.
pub(crate) fn store_signed_transaction<R: DeliveryRecord>(
    entry: &mut R,
    transaction: R::Transaction,
) -> Result<(), DeliveryTransitionError> {
    if entry.delivery_state() != StoredDeliveryStateV1::Signing
        || entry.signed_transaction().is_some()
    {
        return Err(DeliveryTransitionError::InvalidTransition);
    }
    let attempts = entry
        .attempts()
        .checked_add(1)
        .ok_or(DeliveryTransitionError::RetryExhausted)?;
    entry.set_attempts(attempts);
    entry.set_signed_transaction(Some(transaction));
    entry.set_delivery_state(StoredDeliveryStateV1::Signed);
    Ok(())
}

/// Release a signer claim that could not have submitted a transaction.
pub(crate) fn release_signing_claim<R: DeliveryRecord>(
    entry: &mut R,
) -> Result<(), DeliveryTransitionError> {
    if entry.delivery_state() != StoredDeliveryStateV1::Signing
        || entry.signed_transaction().is_some()
    {
        return Err(DeliveryTransitionError::InvalidTransition);
    }
    entry.set_baseline_finalized_height(0);
    entry.set_baseline_finalized_block_hash([0; 32]);
    entry.set_delivery_state(StoredDeliveryStateV1::Ready);
    Ok(())
}

/// Enter the ambiguous crash state before exposing exact bytes to a submitter.
pub(crate) fn begin_submission<R: DeliveryRecord>(
    entry: &mut R,
) -> Result<R::Transaction, DeliveryTransitionError> {
    if entry.delivery_state() != StoredDeliveryStateV1::Signed {
        return Err(DeliveryTransitionError::InvalidTransition);
    }
    let transaction = entry
        .signed_transaction()
        .cloned()
        .ok_or(DeliveryTransitionError::InvalidTransition)?;
    entry.set_delivery_state(StoredDeliveryStateV1::Ambiguous);
    Ok(transaction)
}

/// Mark an ambiguous transaction as known pending or applied.
pub(crate) fn mark_submitted<R: DeliveryRecord>(
    entry: &mut R,
) -> Result<(), DeliveryTransitionError> {
    if entry.delivery_state() != StoredDeliveryStateV1::Ambiguous
        || entry.signed_transaction().is_none()
    {
        return Err(DeliveryTransitionError::InvalidTransition);
    }
    entry.set_delivery_state(StoredDeliveryStateV1::Submitted);
    Ok(())
}

/// Return a known pre-queue failure to the signed state without changing bytes.
pub(crate) fn mark_not_submitted<R: DeliveryRecord>(
    entry: &mut R,
) -> Result<(), DeliveryTransitionError> {
    if entry.delivery_state() != StoredDeliveryStateV1::Ambiguous
        || entry.signed_transaction().is_none()
    {
        return Err(DeliveryTransitionError::InvalidTransition);
    }
    entry.set_delivery_state(StoredDeliveryStateV1::Signed);
    Ok(())
}

/// Re-enable the same exact envelope only after finalized absence is proven.
pub(crate) fn mark_finalized_absent<R: DeliveryRecord>(
    entry: &mut R,
    cursor: FinalizedCursorV1,
    max_attempts: u32,
) -> Result<RetryBoundOutcome, DeliveryTransitionError> {
    validate_finalized_cursor(cursor)?;
    if !matches!(
        entry.delivery_state(),
        StoredDeliveryStateV1::Ambiguous | StoredDeliveryStateV1::Submitted
    ) || entry.signed_transaction().is_none()
        || cursor.height <= entry.baseline_finalized_height()
    {
        return Err(DeliveryTransitionError::InvalidTransition);
    }
    if entry.attempts() >= max_attempts {
        return Ok(RetryBoundOutcome::Exhausted);
    }
    let attempts = entry
        .attempts()
        .checked_add(1)
        .ok_or(DeliveryTransitionError::RetryExhausted)?;
    entry.set_attempts(attempts);
    entry.set_baseline_finalized_height(cursor.height);
    entry.set_baseline_finalized_block_hash(cursor.block_hash);
    entry.set_delivery_state(StoredDeliveryStateV1::Signed);
    Ok(RetryBoundOutcome::Pending)
}

/// Discard a terminally rejected envelope so the semantic operation can be re-signed.
///
/// This intentionally retains the proof-forwarder's established behavior:
/// callers establish the terminal pipeline result before invoking the
/// transition, while the state machine only applies the bounded re-sign rule.
pub(crate) fn mark_transaction_rejected<R: DeliveryRecord>(
    entry: &mut R,
    max_attempts: u32,
) -> RetryBoundOutcome {
    if entry.attempts() >= max_attempts {
        return RetryBoundOutcome::Exhausted;
    }
    entry.set_delivery_state(StoredDeliveryStateV1::Ready);
    entry.set_baseline_finalized_height(0);
    entry.set_baseline_finalized_block_hash([0; 32]);
    entry.set_signed_transaction(None);
    RetryBoundOutcome::Pending
}

/// Hardened checkpoint-store error shared by all wrappers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum CheckpointStoreError {
    /// Checkpoint path is unsafe or inaccessible.
    #[error("checkpoint I/O failed")]
    Io,
    /// Checkpoint exceeds its configured wire-size ceiling.
    #[error("checkpoint exceeds its byte limit")]
    TooLarge,
    /// A second writer owns this checkpoint.
    #[error("checkpoint writer is busy")]
    Busy,
    /// Another runtime changed the checkpoint.
    #[error("checkpoint changed concurrently")]
    Stale,
    /// Atomic replacement may be visible but directory durability is unknown.
    #[error("checkpoint durability is uncertain")]
    DurabilityUncertain,
    /// The in-process writer registry was poisoned.
    #[error("checkpoint writer registry is poisoned")]
    RuntimePoisoned,
}

/// Atomic, private, single-writer checkpoint byte store.
#[derive(Debug)]
pub(crate) struct AtomicCheckpointStore {
    root: PathBuf,
    root_identity: StateDirectoryIdentity,
    checkpoint_path: PathBuf,
    lock_path: PathBuf,
    checkpoint_file_name: &'static str,
    max_bytes: u64,
}

impl AtomicCheckpointStore {
    /// Open or create a private state directory.
    pub(crate) fn new(
        root: &Path,
        checkpoint_file_name: &'static str,
        lock_file_name: &'static str,
        max_bytes: u64,
    ) -> Result<Self, CheckpointStoreError> {
        ensure_private_state_directory(root)?;
        let root = fs::canonicalize(root).map_err(|_| CheckpointStoreError::Io)?;
        let root_identity = state_directory_identity(&root)?;
        Ok(Self {
            checkpoint_path: root.join(checkpoint_file_name),
            lock_path: root.join(lock_file_name),
            root,
            root_identity,
            checkpoint_file_name,
            max_bytes,
        })
    }

    fn verify_root_identity(&self) -> Result<(), CheckpointStoreError> {
        if state_directory_identity(&self.root)? != self.root_identity {
            return Err(CheckpointStoreError::Io);
        }
        Ok(())
    }

    /// Read the exact canonical checkpoint bytes and their optimistic fingerprint.
    pub(crate) fn load_bytes(
        &self,
    ) -> Result<(Option<Vec<u8>>, Option<[u8; 32]>), CheckpointStoreError> {
        self.verify_root_identity()?;
        let _writer = CheckpointWriterGuard::acquire(&self.lock_path)?;
        self.verify_root_identity()?;
        let bytes = read_checkpoint_bytes(&self.checkpoint_path, self.max_bytes)?;
        self.verify_root_identity()?;
        let fingerprint = bytes
            .as_deref()
            .map(blake3::hash)
            .map(|digest| *digest.as_bytes());
        Ok((bytes, fingerprint))
    }

    /// Atomically replace the checkpoint after an optimistic concurrency check.
    pub(crate) fn commit_bytes(
        &self,
        bytes: &[u8],
        expected_fingerprint: Option<[u8; 32]>,
    ) -> Result<[u8; 32], CheckpointStoreError> {
        self.verify_root_identity()?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.max_bytes {
            return Err(CheckpointStoreError::TooLarge);
        }
        let _writer = CheckpointWriterGuard::acquire(&self.lock_path)?;
        self.verify_root_identity()?;
        let current = read_checkpoint_bytes(&self.checkpoint_path, self.max_bytes)?;
        self.verify_root_identity()?;
        let fingerprint = current
            .as_deref()
            .map(blake3::hash)
            .map(|digest| *digest.as_bytes());
        if fingerprint != expected_fingerprint {
            return Err(CheckpointStoreError::Stale);
        }
        let temp_path = self.root.join(format!(
            ".{}.{}.{}.tmp",
            self.checkpoint_file_name,
            std::process::id(),
            CHECKPOINT_TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let result = write_checkpoint_temp(&temp_path, bytes).and_then(|temp_file| {
            self.verify_root_identity()
                .map_err(|_| CheckpointStoreError::DurabilityUncertain)?;
            let latest = read_checkpoint_bytes(&self.checkpoint_path, self.max_bytes)?;
            self.verify_root_identity()
                .map_err(|_| CheckpointStoreError::DurabilityUncertain)?;
            let latest_fingerprint = latest
                .as_deref()
                .map(blake3::hash)
                .map(|digest| *digest.as_bytes());
            if latest_fingerprint != expected_fingerprint {
                return Err(CheckpointStoreError::Stale);
            }
            self.verify_root_identity()
                .map_err(|_| CheckpointStoreError::DurabilityUncertain)?;
            validate_checkpoint_temp(&temp_path, &temp_file, bytes.len())?;
            persist_atomic_replacement(&temp_path, &self.checkpoint_path)
                .map_err(|_| CheckpointStoreError::Io)?;
            validate_persisted_checkpoint(&self.checkpoint_path, &temp_file, bytes.len())
                .map_err(|_| CheckpointStoreError::DurabilityUncertain)?;
            self.verify_root_identity()
                .map_err(|_| CheckpointStoreError::DurabilityUncertain)?;
            sync_directory(&self.root).map_err(|_| CheckpointStoreError::DurabilityUncertain)?;
            self.verify_root_identity()
                .map_err(|_| CheckpointStoreError::DurabilityUncertain)
        });
        result?;
        let persisted = read_checkpoint_bytes(&self.checkpoint_path, self.max_bytes)
            .map_err(|_| CheckpointStoreError::DurabilityUncertain)?
            .ok_or(CheckpointStoreError::DurabilityUncertain)?;
        self.verify_root_identity()
            .map_err(|_| CheckpointStoreError::DurabilityUncertain)?;
        if persisted != bytes {
            return Err(CheckpointStoreError::DurabilityUncertain);
        }
        Ok(*blake3::hash(bytes).as_bytes())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StateDirectoryIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial_number: u32,
    #[cfg(windows)]
    file_index: u64,
    #[cfg(all(not(unix), not(windows)))]
    _unsupported: (),
}

fn state_directory_identity(path: &Path) -> Result<StateDirectoryIdentity, CheckpointStoreError> {
    let (_directory, metadata) = open_stable_state_directory(path)?;
    state_directory_identity_from_metadata(&metadata)
}

#[cfg(unix)]
fn state_directory_identity_from_metadata(
    metadata: &fs::Metadata,
) -> Result<StateDirectoryIdentity, CheckpointStoreError> {
    Ok(StateDirectoryIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(windows)]
fn state_directory_identity_from_metadata(
    metadata: &fs::Metadata,
) -> Result<StateDirectoryIdentity, CheckpointStoreError> {
    Ok(StateDirectoryIdentity {
        volume_serial_number: metadata
            .volume_serial_number()
            .ok_or(CheckpointStoreError::Io)?,
        file_index: metadata.file_index().ok_or(CheckpointStoreError::Io)?,
    })
}

#[cfg(all(not(unix), not(windows)))]
fn state_directory_identity_from_metadata(
    _metadata: &fs::Metadata,
) -> Result<StateDirectoryIdentity, CheckpointStoreError> {
    Err(CheckpointStoreError::Io)
}

/// Guard used by focused alias/hard-link persistence tests.
pub(crate) struct CheckpointWriterGuard {
    _process_guard: CheckpointProcessGuard,
    _file: File,
}

impl CheckpointWriterGuard {
    pub(crate) fn acquire(path: &Path) -> Result<Self, CheckpointStoreError> {
        let process_guard = CheckpointProcessGuard::acquire(path)?;
        let before_open = match fs::symlink_metadata(path) {
            Ok(metadata) => {
                validate_regular_file_metadata(&metadata, u64::MAX, true)?;
                Some(metadata)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(CheckpointStoreError::Io),
        };
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        options.mode(0o600);
        configure_direct_file_open(&mut options)?;
        let file = options.open(path).map_err(|_| CheckpointStoreError::Io)?;
        let opened = file.metadata().map_err(|_| CheckpointStoreError::Io)?;
        validate_regular_file_metadata(&opened, u64::MAX, true)?;
        if before_open
            .as_ref()
            .is_some_and(|before| !file_metadata_unchanged(before, &opened))
        {
            return Err(CheckpointStoreError::Io);
        }
        let linked = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
        validate_regular_file_metadata(&linked, u64::MAX, true)?;
        if !file_metadata_unchanged(&opened, &linked) {
            return Err(CheckpointStoreError::Io);
        }
        match file.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(CheckpointStoreError::Busy),
            Err(fs::TryLockError::Error(_)) => return Err(CheckpointStoreError::Io),
        }
        let locked_file = file.metadata().map_err(|_| CheckpointStoreError::Io)?;
        let locked_path = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
        validate_regular_file_metadata(&locked_file, u64::MAX, true)?;
        validate_regular_file_metadata(&locked_path, u64::MAX, true)?;
        if !file_metadata_unchanged(&opened, &locked_file)
            || !file_metadata_unchanged(&opened, &locked_path)
        {
            return Err(CheckpointStoreError::Io);
        }
        Ok(Self {
            _process_guard: process_guard,
            _file: file,
        })
    }
}

struct CheckpointProcessGuard {
    path: PathBuf,
}

impl CheckpointProcessGuard {
    fn acquire(path: &Path) -> Result<Self, CheckpointStoreError> {
        let parent = path.parent().ok_or(CheckpointStoreError::Io)?;
        let file_name = path.file_name().ok_or(CheckpointStoreError::Io)?;
        let path = fs::canonicalize(parent)
            .map_err(|_| CheckpointStoreError::Io)?
            .join(file_name);
        let mut held = CHECKPOINT_PROCESS_LOCKS
            .lock()
            .map_err(|_| CheckpointStoreError::RuntimePoisoned)?;
        if !held.insert(path.clone()) {
            return Err(CheckpointStoreError::Busy);
        }
        drop(held);
        Ok(Self { path })
    }
}

impl Drop for CheckpointProcessGuard {
    fn drop(&mut self) {
        if let Ok(mut held) = CHECKPOINT_PROCESS_LOCKS.lock() {
            held.remove(&self.path);
        }
    }
}

fn ensure_private_state_directory(path: &Path) -> Result<(), CheckpointStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(CheckpointStoreError::Io);
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            builder.mode(0o700);
            builder.create(path).map_err(|_| CheckpointStoreError::Io)?;
        }
        Err(_) => return Err(CheckpointStoreError::Io),
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CheckpointStoreError::Io);
    }
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| CheckpointStoreError::Io)?;
    state_directory_identity(path).map(drop)?;
    Ok(())
}

fn read_checkpoint_bytes(
    path: &Path,
    max_bytes: u64,
) -> Result<Option<Vec<u8>>, CheckpointStoreError> {
    let path_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(CheckpointStoreError::Io),
    };
    if path_metadata.len() > max_bytes {
        return Err(CheckpointStoreError::TooLarge);
    }
    validate_regular_file_metadata(&path_metadata, max_bytes, false)?;
    let mut options = OpenOptions::new();
    options.read(true);
    configure_direct_file_open(&mut options)?;
    let mut file = options.open(path).map_err(|_| CheckpointStoreError::Io)?;
    let opened = file.metadata().map_err(|_| CheckpointStoreError::Io)?;
    validate_regular_file_metadata(&opened, max_bytes, false)?;
    if !file_metadata_unchanged(&path_metadata, &opened) {
        return Err(CheckpointStoreError::Io);
    }
    let linked = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    validate_regular_file_metadata(&linked, max_bytes, false)?;
    if !file_metadata_unchanged(&opened, &linked) {
        return Err(CheckpointStoreError::Io);
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(path_metadata.len())
            .unwrap_or(usize::MAX)
            .min(usize::try_from(max_bytes).unwrap_or(usize::MAX)),
    );
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| CheckpointStoreError::Io)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(CheckpointStoreError::TooLarge);
    }
    let file_after = file.metadata().map_err(|_| CheckpointStoreError::Io)?;
    let path_after = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    validate_regular_file_metadata(&file_after, max_bytes, false)?;
    validate_regular_file_metadata(&path_after, max_bytes, false)?;
    if !file_metadata_unchanged(&opened, &file_after)
        || !file_metadata_unchanged(&file_after, &path_after)
    {
        return Err(CheckpointStoreError::Io);
    }
    Ok(Some(bytes))
}

fn validate_regular_file_metadata(
    metadata: &fs::Metadata,
    max_bytes: u64,
    allow_lock: bool,
) -> Result<(), CheckpointStoreError> {
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || (!allow_lock && metadata.len() > max_bytes)
    {
        return Err(CheckpointStoreError::Io);
    }
    #[cfg(unix)]
    {
        if metadata.nlink() != 1 || metadata.permissions().mode() & 0o077 != 0 {
            return Err(CheckpointStoreError::Io);
        }
    }
    #[cfg(windows)]
    {
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || metadata.number_of_links() != Some(1)
            || metadata.volume_serial_number().is_none()
            || metadata.file_index().is_none()
        {
            return Err(CheckpointStoreError::Io);
        }
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = (metadata, max_bytes, allow_lock);
        return Err(CheckpointStoreError::Io);
    }
    Ok(())
}

fn write_checkpoint_temp(path: &Path, bytes: &[u8]) -> Result<File, CheckpointStoreError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    configure_direct_file_open(&mut options)?;
    let mut file = options.open(path).map_err(|_| CheckpointStoreError::Io)?;
    let opened = file.metadata().map_err(|_| CheckpointStoreError::Io)?;
    validate_regular_file_metadata(&opened, u64::MAX, false)?;
    let linked = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    validate_regular_file_metadata(&linked, u64::MAX, false)?;
    if !file_metadata_unchanged(&opened, &linked) {
        return Err(CheckpointStoreError::Io);
    }
    file.write_all(bytes)
        .map_err(|_| CheckpointStoreError::Io)?;
    file.sync_all().map_err(|_| CheckpointStoreError::Io)?;
    let file_after = file.metadata().map_err(|_| CheckpointStoreError::Io)?;
    let path_after = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    let expected_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    validate_regular_file_metadata(&file_after, expected_len, false)?;
    validate_regular_file_metadata(&path_after, expected_len, false)?;
    if file_after.len() != expected_len
        || path_after.len() != expected_len
        || !same_file_identity(&opened, &file_after)
        || !file_metadata_unchanged(&file_after, &path_after)
    {
        return Err(CheckpointStoreError::Io);
    }
    Ok(file)
}

fn validate_checkpoint_temp(
    path: &Path,
    file: &File,
    expected_len: usize,
) -> Result<(), CheckpointStoreError> {
    let opened = file.metadata().map_err(|_| CheckpointStoreError::Io)?;
    let linked = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    let expected_len = u64::try_from(expected_len).unwrap_or(u64::MAX);
    validate_regular_file_metadata(&opened, expected_len, false)?;
    validate_regular_file_metadata(&linked, expected_len, false)?;
    if opened.len() != expected_len
        || linked.len() != expected_len
        || !file_metadata_unchanged(&opened, &linked)
    {
        return Err(CheckpointStoreError::Io);
    }
    Ok(())
}

fn validate_persisted_checkpoint(
    path: &Path,
    file: &File,
    expected_len: usize,
) -> Result<(), CheckpointStoreError> {
    let opened = file.metadata().map_err(|_| CheckpointStoreError::Io)?;
    let linked = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    let expected_len = u64::try_from(expected_len).unwrap_or(u64::MAX);
    validate_regular_file_metadata(&opened, expected_len, false)?;
    validate_regular_file_metadata(&linked, expected_len, false)?;
    if opened.len() != expected_len
        || linked.len() != expected_len
        || !file_metadata_unchanged(&opened, &linked)
    {
        return Err(CheckpointStoreError::Io);
    }
    Ok(())
}

fn persist_atomic_replacement(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    // `std::fs::rename` does not replace an existing Windows destination. `TempPath::persist`
    // selects native replacement semantics on all release targets. Cleanup remains disabled so a
    // failed promotion leaves the recognizable artifact available to crash reconciliation.
    let mut temporary = tempfile::TempPath::try_from_path(temporary)?;
    temporary.disable_cleanup(true);
    temporary.persist(destination).map_err(|error| error.error)
}

fn sync_directory(path: &Path) -> Result<(), CheckpointStoreError> {
    let (directory, opened) = open_stable_state_directory(path)?;
    directory.sync_all().map_err(|_| CheckpointStoreError::Io)?;
    let file_after = directory.metadata().map_err(|_| CheckpointStoreError::Io)?;
    let path_after = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    validate_state_directory_metadata(&file_after)?;
    validate_state_directory_metadata(&path_after)?;
    if !directory_metadata_unchanged(&opened, &file_after)
        || !directory_metadata_unchanged(&file_after, &path_after)
    {
        return Err(CheckpointStoreError::Io);
    }
    Ok(())
}

fn open_stable_state_directory(path: &Path) -> Result<(File, fs::Metadata), CheckpointStoreError> {
    let before = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    validate_state_directory_metadata(&before)?;
    let mut options = OpenOptions::new();
    options.read(true);
    configure_direct_directory_open(&mut options)?;
    let directory = options.open(path).map_err(|_| CheckpointStoreError::Io)?;
    let opened = directory.metadata().map_err(|_| CheckpointStoreError::Io)?;
    validate_state_directory_metadata(&opened)?;
    let after = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    validate_state_directory_metadata(&after)?;
    if !directory_metadata_unchanged(&before, &opened)
        || !directory_metadata_unchanged(&opened, &after)
    {
        return Err(CheckpointStoreError::Io);
    }
    Ok((directory, opened))
}

fn validate_state_directory_metadata(metadata: &fs::Metadata) -> Result<(), CheckpointStoreError> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CheckpointStoreError::Io);
    }
    #[cfg(unix)]
    {
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(CheckpointStoreError::Io);
        }
    }
    #[cfg(windows)]
    {
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || metadata.volume_serial_number().is_none()
            || metadata.file_index().is_none()
        {
            return Err(CheckpointStoreError::Io);
        }
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = metadata;
        return Err(CheckpointStoreError::Io);
    }
    Ok(())
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios"
))]
fn configure_direct_file_open(options: &mut OpenOptions) -> Result<(), CheckpointStoreError> {
    options.custom_flags(SAFE_OPEN_FLAGS);
    Ok(())
}

#[cfg(windows)]
fn configure_direct_file_open(options: &mut OpenOptions) -> Result<(), CheckpointStoreError> {
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    Ok(())
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    windows
)))]
fn configure_direct_file_open(_options: &mut OpenOptions) -> Result<(), CheckpointStoreError> {
    Err(CheckpointStoreError::Io)
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios"
))]
fn configure_direct_directory_open(options: &mut OpenOptions) -> Result<(), CheckpointStoreError> {
    options.custom_flags(SAFE_OPEN_FLAGS);
    Ok(())
}

#[cfg(windows)]
fn configure_direct_directory_open(options: &mut OpenOptions) -> Result<(), CheckpointStoreError> {
    // `File::sync_all` maps to `FlushFileBuffers`, which requires a write-capable handle.
    options.write(true);
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    Ok(())
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    windows
)))]
fn configure_direct_directory_open(_options: &mut OpenOptions) -> Result<(), CheckpointStoreError> {
    Err(CheckpointStoreError::Io)
}

#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(windows)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
}

#[cfg(all(not(unix), not(windows)))]
fn same_file_identity(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn file_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    same_file_identity(left, right)
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(windows)]
fn file_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    same_file_identity(left, right)
        && left.number_of_links() == Some(1)
        && right.number_of_links() == Some(1)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
}

#[cfg(all(not(unix), not(windows)))]
fn file_metadata_unchanged(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn directory_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    same_file_identity(left, right)
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(windows)]
fn directory_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    same_file_identity(left, right)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
}

#[cfg(all(not(unix), not(windows)))]
fn directory_metadata_unchanged(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
    use tempfile::TempDir;

    fn private_directory(path: &Path) {
        ensure_private_state_directory(path).expect("create private state directory");
    }

    fn assert_distinct_directory_identities() {
        let outer = TempDir::new().expect("temporary directory");
        let first = outer.path().join("first");
        let second = outer.path().join("second");
        private_directory(&first);
        private_directory(&second);
        assert_ne!(
            state_directory_identity(&first).expect("first identity"),
            state_directory_identity(&second).expect("second identity")
        );
    }

    fn assert_hardlinked_checkpoint_is_rejected() {
        let directory = TempDir::new().expect("temporary directory");
        let store =
            AtomicCheckpointStore::new(directory.path(), "checkpoint.to", "checkpoint.lock", 1024)
                .expect("checkpoint store");
        let outside = directory.path().join("outside.to");
        fs::write(&outside, b"outside").expect("outside file");
        #[cfg(unix)]
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o600))
            .expect("private outside file");
        fs::hard_link(&outside, &store.checkpoint_path).expect("checkpoint hard link");
        assert_eq!(
            store
                .load_bytes()
                .expect_err("reject hardlinked checkpoint"),
            CheckpointStoreError::Io
        );
    }

    fn assert_hardlinked_lock_is_rejected() {
        let directory = TempDir::new().expect("temporary directory");
        private_directory(directory.path());
        let lock_path = directory.path().join("checkpoint.lock");
        drop(CheckpointWriterGuard::acquire(&lock_path).expect("create lock file"));
        let alias = directory.path().join("checkpoint-lock-alias");
        fs::hard_link(&lock_path, &alias).expect("lock hard link");
        assert!(matches!(
            CheckpointWriterGuard::acquire(&lock_path),
            Err(CheckpointStoreError::Io)
        ));
    }

    fn assert_root_path_substitution_is_rejected() {
        let outer = TempDir::new().expect("temporary directory");
        let state = outer.path().join("state");
        let displaced = outer.path().join("displaced");
        let store = AtomicCheckpointStore::new(&state, "checkpoint.to", "checkpoint.lock", 1024)
            .expect("checkpoint store");
        fs::rename(&state, &displaced).expect("displace state directory");
        private_directory(&state);
        assert_eq!(
            store
                .load_bytes()
                .expect_err("reject substituted state root"),
            CheckpointStoreError::Io
        );
        assert!(
            !state.join("checkpoint.to").exists(),
            "replacement root must not receive checkpoint bytes"
        );
    }

    fn assert_os_lock_contention_is_busy() {
        let directory = TempDir::new().expect("temporary directory");
        private_directory(directory.path());
        let lock_path = directory.path().join("checkpoint.lock");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        options.mode(0o600);
        configure_direct_file_open(&mut options).expect("configure direct lock open");
        let lock_file = options.open(&lock_path).expect("open lock file");
        lock_file.try_lock().expect("own operating-system lock");
        assert!(matches!(
            CheckpointWriterGuard::acquire(&lock_path),
            Err(CheckpointStoreError::Busy)
        ));
        drop(lock_file);
        drop(CheckpointWriterGuard::acquire(&lock_path).expect("lock becomes available"));
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn checkpoint_store_replaces_existing_destination() {
        let directory = TempDir::new().expect("temporary directory");
        let store =
            AtomicCheckpointStore::new(directory.path(), "checkpoint.to", "checkpoint.lock", 1024)
                .expect("checkpoint store");
        let first = store
            .commit_bytes(b"first", None)
            .expect("first checkpoint");
        assert_eq!(first, *blake3::hash(b"first").as_bytes());
        let (_, fingerprint) = store.load_bytes().expect("load first checkpoint");
        let second = store
            .commit_bytes(b"second", fingerprint)
            .expect("replace existing checkpoint");
        assert_eq!(second, *blake3::hash(b"second").as_bytes());
        assert_eq!(
            store.load_bytes().expect("load replacement").0.as_deref(),
            Some(&b"second"[..])
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_checkpoint_open_rejects_symlink_and_hardlink_targets() {
        use std::os::unix::fs::symlink;

        let symlink_directory = TempDir::new().expect("temporary directory");
        let store = AtomicCheckpointStore::new(
            symlink_directory.path(),
            "checkpoint.to",
            "checkpoint.lock",
            1024,
        )
        .expect("checkpoint store");
        let outside = symlink_directory.path().join("outside.to");
        fs::write(&outside, b"outside").expect("outside file");
        symlink(&outside, &store.checkpoint_path).expect("checkpoint symlink");
        assert_eq!(
            store.load_bytes().expect_err("reject symlink checkpoint"),
            CheckpointStoreError::Io
        );

        let lock_directory = TempDir::new().expect("temporary directory");
        private_directory(lock_directory.path());
        let outside_lock = lock_directory.path().join("outside.lock");
        fs::write(&outside_lock, b"outside").expect("outside lock");
        fs::set_permissions(&outside_lock, fs::Permissions::from_mode(0o600))
            .expect("private outside lock");
        let lock_path = lock_directory.path().join("checkpoint.lock");
        symlink(&outside_lock, &lock_path).expect("lock symlink");
        assert!(matches!(
            CheckpointWriterGuard::acquire(&lock_path),
            Err(CheckpointStoreError::Io)
        ));

        assert_hardlinked_checkpoint_is_rejected();
        assert_hardlinked_lock_is_rejected();
    }

    #[cfg(unix)]
    #[test]
    fn unix_checkpoint_identity_path_substitution_and_lock_contention_are_fenced() {
        assert_distinct_directory_identities();
        assert_root_path_substitution_is_rejected();
        assert_os_lock_contention_is_busy();
    }

    #[cfg(windows)]
    #[test]
    fn windows_direct_open_uses_reparse_safe_flags() {
        assert_ne!(FILE_FLAG_OPEN_REPARSE_POINT, 0);
        assert_ne!(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS, 0);
    }

    #[cfg(windows)]
    #[test]
    fn windows_checkpoint_identity_and_hardlinks_are_fenced() {
        assert_distinct_directory_identities();
        assert_hardlinked_checkpoint_is_rejected();
        assert_hardlinked_lock_is_rejected();
    }

    #[cfg(windows)]
    #[test]
    fn windows_checkpoint_path_substitution_and_lock_contention_are_fenced() {
        assert_root_path_substitution_is_rejected();
        assert_os_lock_contention_is_busy();
    }
}
