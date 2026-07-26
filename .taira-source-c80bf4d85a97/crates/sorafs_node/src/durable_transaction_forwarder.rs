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
use std::os::unix::{
    fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    io::AsRawFd as _,
};

use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

static CHECKPOINT_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static CHECKPOINT_PROCESS_LOCKS: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

#[cfg(unix)]
const LOCK_EXCLUSIVE_NONBLOCKING: std::os::raw::c_int = 2 | 4;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0x0002_0000 | 0x0008_0000;
#[cfg(any(target_os = "macos", target_os = "ios"))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0x0000_0100 | 0x0100_0000;
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))
))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0;

#[cfg(unix)]
unsafe extern "C" {
    fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int;
}

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
    /// Rename may be visible but directory durability is unknown.
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
        let result = write_checkpoint_temp(&temp_path, bytes).and_then(|()| {
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
            fs::rename(&temp_path, &self.checkpoint_path).map_err(|_| CheckpointStoreError::Io)?;
            self.verify_root_identity()
                .map_err(|_| CheckpointStoreError::DurabilityUncertain)?;
            sync_directory(&self.root).map_err(|_| CheckpointStoreError::DurabilityUncertain)?;
            self.verify_root_identity()
                .map_err(|_| CheckpointStoreError::DurabilityUncertain)
        });
        if result.is_err() && self.verify_root_identity().is_ok() {
            let _ = fs::remove_file(&temp_path);
        }
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
}

fn state_directory_identity(path: &Path) -> Result<StateDirectoryIdentity, CheckpointStoreError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CheckpointStoreError::Io);
    }
    Ok(StateDirectoryIdentity {
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
    })
}

/// Guard used by focused alias/hard-link persistence tests.
pub(crate) struct CheckpointWriterGuard {
    _process_guard: CheckpointProcessGuard,
    _file: File,
}

impl CheckpointWriterGuard {
    pub(crate) fn acquire(path: &Path) -> Result<Self, CheckpointStoreError> {
        let process_guard = CheckpointProcessGuard::acquire(path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            options.mode(0o600);
            options.custom_flags(SAFE_OPEN_FLAGS);
        }
        let file = options.open(path).map_err(|_| CheckpointStoreError::Io)?;
        validate_open_regular_file(path, &file, 0, true)?;
        #[cfg(unix)]
        if unsafe { flock(file.as_raw_fd(), LOCK_EXCLUSIVE_NONBLOCKING) } != 0 {
            return Err(CheckpointStoreError::Busy);
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
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(CheckpointStoreError::Io);
    }
    #[cfg(unix)]
    if path_metadata.nlink() != 1 {
        return Err(CheckpointStoreError::Io);
    }
    if path_metadata.len() > max_bytes {
        return Err(CheckpointStoreError::TooLarge);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(SAFE_OPEN_FLAGS);
    let mut file = options.open(path).map_err(|_| CheckpointStoreError::Io)?;
    validate_open_regular_file(path, &file, max_bytes, false)?;
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
    let reopened = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
    #[cfg(unix)]
    if reopened.dev() != path_metadata.dev()
        || reopened.ino() != path_metadata.ino()
        || reopened.nlink() != 1
    {
        return Err(CheckpointStoreError::Io);
    }
    Ok(Some(bytes))
}

fn validate_open_regular_file(
    path: &Path,
    file: &File,
    max_bytes: u64,
    allow_lock: bool,
) -> Result<(), CheckpointStoreError> {
    let metadata = file.metadata().map_err(|_| CheckpointStoreError::Io)?;
    if !metadata.is_file() || (!allow_lock && metadata.len() > max_bytes) {
        return Err(CheckpointStoreError::Io);
    }
    #[cfg(unix)]
    {
        let path_metadata = fs::symlink_metadata(path).map_err(|_| CheckpointStoreError::Io)?;
        if path_metadata.file_type().is_symlink()
            || path_metadata.dev() != metadata.dev()
            || path_metadata.ino() != metadata.ino()
            || metadata.nlink() != 1
        {
            return Err(CheckpointStoreError::Io);
        }
    }
    Ok(())
}

fn write_checkpoint_temp(path: &Path, bytes: &[u8]) -> Result<(), CheckpointStoreError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
        options.custom_flags(SAFE_OPEN_FLAGS);
    }
    let mut file = options.open(path).map_err(|_| CheckpointStoreError::Io)?;
    validate_open_regular_file(path, &file, u64::MAX, false)?;
    file.write_all(bytes)
        .map_err(|_| CheckpointStoreError::Io)?;
    file.sync_all().map_err(|_| CheckpointStoreError::Io)?;
    validate_open_regular_file(
        path,
        &file,
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        false,
    )?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), CheckpointStoreError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| CheckpointStoreError::Io)
}
