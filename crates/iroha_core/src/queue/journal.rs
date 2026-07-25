//! Crash-safe local Norito journal for pending queue routing plans.

use std::{
    collections::BTreeMap,
    error::Error as StdError,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
#[cfg(test)]
use std::{collections::VecDeque, sync::Mutex as StdMutex};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::transaction::{SignedTransaction, TransactionEntrypoint};
use norito::codec::{Decode, Encode};

use super::RoutingPlan;

const QUEUE_PLAN_JOURNAL_FRAME_DOMAIN: &[u8] = b"iroha:queue-plan-journal-frame:v2";
const QUEUE_PLAN_JOURNAL_FRAME_MAGIC: [u8; 8] = *b"IRQPJNL2";
const QUEUE_PLAN_JOURNAL_FRAME_COMMIT: [u8; 8] = *b"IRQPEND2";
const QUEUE_PLAN_JOURNAL_FRAME_FORMAT_VERSION: u16 = 2;
const FRAME_HEADER_BYTES: u64 = 8 + 2 + 4 + 4;
const FRAME_TRAILER_BYTES: u64 = Hash::LENGTH as u64 + 8;
const FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT: usize = 4;
const FRAME_DECODE_ALLOCATION_AMPLIFICATION_LIMIT: usize = 6;
const FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 64 * 1024;

/// Version of durable queue plan journal records.
pub const QUEUE_PLAN_JOURNAL_VERSION: u16 = 2;

type SignedTxHash = HashOf<SignedTransaction>;
type QueuePlanJournalKey = (SignedTxHash, Hash);

/// Explicit resource limits for queue plan journal append and replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueuePlanJournalLimits {
    /// File size at which compaction should be considered.
    pub max_bytes_before_compact: u64,
    /// Maximum Norito payload bytes in any one frame.
    pub max_frame_payload_bytes: u64,
    /// Maximum total journal file bytes accepted or appended.
    pub max_file_bytes: u64,
    /// Maximum live records retained by replay.
    pub max_live_records: usize,
}

impl QueuePlanJournalLimits {
    /// Construct explicit queue plan journal limits.
    #[must_use]
    pub const fn new(
        max_bytes_before_compact: u64,
        max_frame_payload_bytes: u64,
        max_file_bytes: u64,
        max_live_records: usize,
    ) -> Self {
        Self {
            max_bytes_before_compact,
            max_frame_payload_bytes,
            max_file_bytes,
            max_live_records,
        }
    }

    fn validate(self) -> io::Result<Self> {
        if self.max_bytes_before_compact == 0 {
            return Err(invalid_input(
                "queue plan journal compaction threshold must be nonzero",
            ));
        }
        if self.max_frame_payload_bytes == 0 || self.max_frame_payload_bytes > u64::from(u32::MAX) {
            return Err(invalid_input(
                "queue plan journal frame payload limit must be in 1..=u32::MAX",
            ));
        }
        if self.max_live_records == 0 {
            return Err(invalid_input(
                "queue plan journal live-record limit must be nonzero",
            ));
        }
        let minimum_file_bytes = FRAME_HEADER_BYTES
            .checked_add(FRAME_TRAILER_BYTES)
            .and_then(|bytes| bytes.checked_add(1))
            .expect("constant frame size must fit u64");
        if self.max_file_bytes < minimum_file_bytes {
            return Err(invalid_input(
                "queue plan journal file limit cannot hold one framed payload byte",
            ));
        }
        if self.max_bytes_before_compact > self.max_file_bytes {
            return Err(invalid_input(
                "queue plan journal compaction threshold exceeds the file limit",
            ));
        }
        Ok(self)
    }
}

/// Pending transaction routing-plan journal record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct QueuePlanJournalRecordV2 {
    /// Record format version.
    pub version: u16,
    /// Canonical transaction entrypoint.
    pub entrypoint: TransactionEntrypoint,
    /// Compatibility signed-transaction hash used by queue indexes.
    pub signed_transaction_hash: SignedTxHash,
    /// Full routing plan admitted for this transaction.
    pub routing_plan: RoutingPlan,
    /// Local enqueue timestamp in milliseconds.
    pub enqueue_timestamp_ms: u64,
}

impl QueuePlanJournalRecordV2 {
    /// Construct a version-2 journal record.
    #[must_use]
    pub fn new(
        entrypoint: TransactionEntrypoint,
        signed_transaction_hash: SignedTxHash,
        routing_plan: RoutingPlan,
        enqueue_timestamp_ms: u64,
    ) -> Self {
        Self {
            version: QUEUE_PLAN_JOURNAL_VERSION,
            entrypoint,
            signed_transaction_hash,
            routing_plan,
            enqueue_timestamp_ms,
        }
    }

    /// Digest paired with removals to avoid deleting a re-admitted hash with a new plan.
    #[must_use]
    pub fn plan_digest(&self) -> Hash {
        self.routing_plan.digest()
    }
}

/// One append-only queue plan journal operation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
enum QueuePlanJournalFrameV2 {
    /// Add or replace a pending queue record.
    Put(QueuePlanJournalRecordV2),
    /// Tombstone a pending queue record.
    Remove {
        /// Compatibility signed-transaction hash.
        hash: SignedTxHash,
        /// Full routing-plan digest that was removed.
        plan_digest: Hash,
    },
}

/// Typed failure from a strict, synchronously durable journal Put.
#[derive(Debug)]
pub enum QueuePlanJournalStrictPutError {
    /// The exact Put is known not to be live after the method returns.
    DefinitelyNotLive {
        /// Underlying validation, capacity, write, or synchronization error.
        source: io::Error,
        /// Whether the journal must remain fail-closed until restart repair.
        journal_faulted: bool,
    },
    /// The exact Put may or may not be live and requires restart reconciliation.
    OutcomeIndeterminate {
        /// Underlying append or synchronization error.
        source: io::Error,
    },
}

impl QueuePlanJournalStrictPutError {
    /// Return whether the exact Put outcome requires reconciliation.
    #[must_use]
    pub const fn is_indeterminate(&self) -> bool {
        matches!(self, Self::OutcomeIndeterminate { .. })
    }

    /// Return whether the journal must be faulted after this error.
    #[must_use]
    pub const fn journal_faulted(&self) -> bool {
        match self {
            Self::DefinitelyNotLive {
                journal_faulted, ..
            } => *journal_faulted,
            Self::OutcomeIndeterminate { .. } => true,
        }
    }

    /// Borrow the underlying I/O error.
    #[must_use]
    pub const fn source(&self) -> &io::Error {
        match self {
            Self::DefinitelyNotLive { source, .. } | Self::OutcomeIndeterminate { source } => {
                source
            }
        }
    }

    /// Consume this error and return its underlying I/O error.
    #[must_use]
    pub fn into_source(self) -> io::Error {
        match self {
            Self::DefinitelyNotLive { source, .. } | Self::OutcomeIndeterminate { source } => {
                source
            }
        }
    }

    fn definitely_not_live(source: io::Error, journal_faulted: bool) -> Self {
        Self::DefinitelyNotLive {
            source,
            journal_faulted,
        }
    }

    fn indeterminate(source: io::Error) -> Self {
        Self::OutcomeIndeterminate { source }
    }
}

impl fmt::Display for QueuePlanJournalStrictPutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefinitelyNotLive {
                source,
                journal_faulted,
            } => write!(
                formatter,
                "queue plan journal Put is definitely not live (journal_faulted={journal_faulted}): {source}"
            ),
            Self::OutcomeIndeterminate { source } => write!(
                formatter,
                "queue plan journal Put outcome is indeterminate: {source}"
            ),
        }
    }
}

impl StdError for QueuePlanJournalStrictPutError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(QueuePlanJournalStrictPutError::source(self))
    }
}

/// Append-only queue plan journal with bounded repair and atomic compaction.
pub struct QueuePlanJournal {
    path: PathBuf,
    limits: QueuePlanJournalLimits,
    durable_writes: bool,
    file: File,
    tombstones: u64,
    poisoned: bool,
    #[cfg(test)]
    injected_faults: StdMutex<VecDeque<QueuePlanJournalTestFault>>,
}

/// Test-only journal phase fault.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QueuePlanJournalTestFault {
    /// Fail before writing any bytes of a strict Put frame.
    PutBeforeAppend,
    /// Write only a strict Put frame prefix and report failure.
    PutPartialWrite,
    /// Report failure before the strict Put file synchronization.
    PutSync,
    /// Report failure after strict Put file sync but before parent-directory sync.
    PutParentSync,
    /// Fail before appending the strict cleanup Remove.
    CleanupBeforeAppend,
    /// Write only a cleanup Remove frame prefix and report failure.
    CleanupPartialWrite,
    /// Write the complete cleanup Remove, then report append ambiguity.
    CleanupAfterFullWrite,
    /// Report failure before strict cleanup file synchronization.
    CleanupSync,
    /// Report failure after cleanup file sync but before parent-directory sync.
    CleanupParentSync,
    /// Report failure after general file sync but before parent-directory sync.
    GeneralParentSync,
    /// Fail after creating the compaction replacement but before writing it.
    CompactionAfterTempCreate,
    /// Fail after replacing the journal path but before syncing its parent.
    CompactionAfterRename,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppendPhase {
    Put,
    Cleanup,
    OrdinaryRemove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SyncPhase {
    Put,
    Cleanup,
    General,
}

struct AppendFailure {
    source: io::Error,
    definitely_incomplete: bool,
    journal_faulted: bool,
}

/// Deferred durability work requested by a journal append.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueuePlanJournalFlush {
    sync_data: bool,
    compact: bool,
}

/// Prepared live-record replay bound to one inode and file-length snapshot.
pub struct QueuePlanJournalReplay {
    file: File,
    snapshot_len: u64,
    limits: QueuePlanJournalLimits,
    live_positions: BTreeMap<QueuePlanJournalKey, u64>,
}

impl QueuePlanJournalReplay {
    /// Return the number of live records captured by this replay snapshot.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live_positions.len()
    }

    /// Stream live records in original append order to `handle_record`.
    ///
    /// # Errors
    /// Returns I/O errors, malformed-frame errors, snapshot consistency errors, or errors
    /// returned by `handle_record`.
    pub fn for_each_record<F>(mut self, mut handle_record: F) -> io::Result<usize>
    where
        F: FnMut(QueuePlanJournalRecordV2) -> io::Result<()>,
    {
        let records = self.live_positions.len();
        if records == 0 {
            return Ok(0);
        }

        self.file.seek(SeekFrom::Start(0))?;
        scan_file(
            &mut self.file,
            self.snapshot_len,
            self.limits,
            ScanMode::Strict,
            None,
            |position, frame| {
                if let QueuePlanJournalFrameV2::Put(record) = frame {
                    let key = (record.signed_transaction_hash, record.plan_digest());
                    if self.live_positions.get(&key).copied() == Some(position) {
                        self.live_positions.remove(&key);
                        handle_record(record)?;
                    }
                }
                Ok(())
            },
        )?;
        if !self.live_positions.is_empty() {
            return Err(invalid_data(
                "queue plan journal replay snapshot lost live frame positions",
            ));
        }
        Ok(records)
    }
}

impl QueuePlanJournalFlush {
    /// Merge two deferred flush requests.
    #[must_use]
    #[cfg(test)]
    pub fn combine(self, other: Self) -> Self {
        Self {
            sync_data: self.sync_data || other.sync_data,
            compact: self.compact || other.compact,
        }
    }

    /// Return whether there is any deferred work.
    #[must_use]
    pub fn is_needed(self) -> bool {
        self.sync_data || self.compact
    }

    /// Return whether the journal data file should be synced.
    #[must_use]
    pub fn sync_data(self) -> bool {
        self.sync_data
    }

    /// Return whether journal compaction should be considered.
    #[must_use]
    pub fn compact(self) -> bool {
        self.compact
    }
}

impl QueuePlanJournal {
    /// Open or create a bounded V2 queue plan journal.
    ///
    /// Empty files are valid. Every nonempty legacy or unknown layout fails closed. Startup
    /// repairs only a recognizable terminal V2 frame prefix and durably synchronizes both the
    /// truncated file and its parent directory.
    ///
    /// # Errors
    /// Returns path validation, repair, limit, corruption, or file-opening errors.
    pub fn open_with_limits(
        path: impl AsRef<Path>,
        limits: QueuePlanJournalLimits,
        durable_writes: bool,
    ) -> io::Result<Self> {
        let limits = limits.validate()?;
        let path = path.as_ref().to_path_buf();
        prepare_regular_journal_parent(&path)?;
        reject_existing_compaction_temp(&path.with_extension("tmp"))?;
        prepare_regular_journal_path(&path)?;
        repair_incomplete_tail(&path, limits)?;
        let file = open_regular_append(&path)?;
        Ok(Self {
            path,
            limits,
            durable_writes,
            file,
            tombstones: 0,
            poisoned: false,
            #[cfg(test)]
            injected_faults: StdMutex::new(VecDeque::new()),
        })
    }

    /// Return whether an append or durability boundary requires restart repair.
    #[cfg(test)]
    #[must_use]
    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Atomically perform an exact Put and its synchronous durability boundary.
    ///
    /// Both the Put and its exact cleanup Remove are encoded and capacity-checked before any
    /// bytes are written. If Put synchronization fails, the cleanup Remove is appended and
    /// synchronized while this method still has exclusive mutable access to the journal.
    ///
    /// # Errors
    /// Returns a typed result distinguishing a definitely absent Put from an outcome requiring
    /// restart reconciliation.
    pub fn put_strict_durable(
        &mut self,
        record: QueuePlanJournalRecordV2,
    ) -> Result<(), QueuePlanJournalStrictPutError> {
        if self.poisoned {
            return Err(QueuePlanJournalStrictPutError::indeterminate(
                poisoned_journal_error(),
            ));
        }

        let hash = record.signed_transaction_hash;
        let plan_digest = record.plan_digest();
        let put = encode_frame(&QueuePlanJournalFrameV2::Put(record), self.limits)
            .map_err(|error| QueuePlanJournalStrictPutError::definitely_not_live(error, false))?;
        let cleanup = encode_frame(
            &QueuePlanJournalFrameV2::Remove { hash, plan_digest },
            self.limits,
        )
        .map_err(|error| QueuePlanJournalStrictPutError::definitely_not_live(error, false))?;
        let reserved_bytes = put.len().checked_add(cleanup.len()).ok_or_else(|| {
            QueuePlanJournalStrictPutError::definitely_not_live(
                invalid_data("queue plan journal strict Put capacity overflow"),
                false,
            )
        })?;
        self.ensure_append_capacity(reserved_bytes)
            .map_err(|error| QueuePlanJournalStrictPutError::definitely_not_live(error, false))?;

        if let Err(failure) = self.append_encoded(&put, AppendPhase::Put) {
            let error = if failure.definitely_incomplete {
                QueuePlanJournalStrictPutError::definitely_not_live(
                    failure.source,
                    failure.journal_faulted,
                )
            } else {
                QueuePlanJournalStrictPutError::indeterminate(failure.source)
            };
            return Err(error);
        }

        let put_sync_error = match self.sync_all_raw(SyncPhase::Put) {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };

        #[cfg(test)]
        if self.take_fault(QueuePlanJournalTestFault::CleanupBeforeAppend) {
            self.poisoned = true;
            return Err(QueuePlanJournalStrictPutError::indeterminate(
                io::Error::other(format!(
                    "queue plan journal Put sync failed ({put_sync_error}); injected cleanup pre-append failure"
                )),
            ));
        }

        if let Err(cleanup_failure) = self.append_encoded(&cleanup, AppendPhase::Cleanup) {
            self.poisoned = true;
            return Err(QueuePlanJournalStrictPutError::indeterminate(
                io::Error::other(format!(
                    "queue plan journal Put sync failed ({put_sync_error}); cleanup append failed ({})",
                    cleanup_failure.source
                )),
            ));
        }
        self.tombstones = self.tombstones.saturating_add(1);
        if let Err(cleanup_sync_error) = self.sync_all_raw(SyncPhase::Cleanup) {
            self.poisoned = true;
            return Err(QueuePlanJournalStrictPutError::indeterminate(
                io::Error::other(format!(
                    "queue plan journal Put sync failed ({put_sync_error}); cleanup sync failed ({cleanup_sync_error})"
                )),
            ));
        }

        Err(QueuePlanJournalStrictPutError::definitely_not_live(
            put_sync_error,
            false,
        ))
    }

    /// Append a Put frame and return deferred durability work for the caller.
    ///
    /// # Errors
    /// Returns validation, capacity, encoding, or I/O errors.
    #[cfg(test)]
    pub fn put_deferred_flush(
        &mut self,
        record: QueuePlanJournalRecordV2,
    ) -> io::Result<QueuePlanJournalFlush> {
        self.ensure_healthy()?;
        let encoded = encode_frame(&QueuePlanJournalFrameV2::Put(record), self.limits)?;
        self.ensure_append_capacity(encoded.len())?;
        self.append_encoded(&encoded, AppendPhase::Put)
            .map_err(|failure| failure.source)?;
        Ok(QueuePlanJournalFlush {
            sync_data: self.durable_writes,
            compact: false,
        })
    }

    /// Append multiple exact Remove frames and return deferred durability work.
    ///
    /// All frames are encoded and the complete batch is capacity-checked before the first append.
    ///
    /// # Errors
    /// Returns validation, capacity, encoding, or I/O errors.
    pub fn remove_many_deferred_flush<I>(
        &mut self,
        removals: I,
    ) -> io::Result<QueuePlanJournalFlush>
    where
        I: IntoIterator<Item = (SignedTxHash, Hash)>,
    {
        self.ensure_healthy()?;
        let mut encoded_frames = Vec::new();
        let mut encoded_bytes = 0_usize;
        for (hash, plan_digest) in removals {
            let encoded = encode_frame(
                &QueuePlanJournalFrameV2::Remove { hash, plan_digest },
                self.limits,
            )?;
            encoded_bytes = encoded_bytes
                .checked_add(encoded.len())
                .ok_or_else(|| invalid_data("queue plan journal Remove batch capacity overflow"))?;
            let encoded_bytes_u64 = u64::try_from(encoded_bytes)
                .map_err(|_| invalid_data("queue plan journal Remove batch exceeds u64"))?;
            if encoded_bytes_u64 > self.limits.max_file_bytes {
                return Err(invalid_data(
                    "queue plan journal Remove batch exceeds the file limit",
                ));
            }
            encoded_frames.push(encoded);
        }
        if encoded_frames.is_empty() {
            return Ok(QueuePlanJournalFlush::default());
        }
        self.ensure_append_capacity(encoded_bytes)?;
        for encoded in encoded_frames {
            self.append_encoded(&encoded, AppendPhase::OrdinaryRemove)
                .map_err(|failure| failure.source)?;
            self.tombstones = self.tombstones.saturating_add(1);
        }
        Ok(QueuePlanJournalFlush {
            sync_data: self.durable_writes,
            compact: true,
        })
    }

    /// Run deferred durability work produced by append methods.
    ///
    /// # Errors
    /// Returns sync or compaction I/O errors.
    #[cfg(test)]
    pub fn flush_deferred(&mut self, flush: QueuePlanJournalFlush) -> io::Result<()> {
        self.ensure_healthy()?;
        if !flush.is_needed() {
            return Ok(());
        }
        if flush.sync_data
            && let Err(error) = self.file.sync_data()
        {
            self.poisoned = true;
            return Err(error);
        }
        if flush.compact
            && let Err(error) = self.compact_if_needed()
        {
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
    }

    /// Clone the append file handle so callers can sync without holding the journal mutex.
    ///
    /// # Errors
    /// Returns I/O errors from duplicating the file handle or a poisoned-journal error.
    pub fn sync_file_clone(&self) -> io::Result<File> {
        self.ensure_healthy()?;
        self.file.try_clone()
    }

    /// Force journal contents, file metadata, and its parent entry to stable storage.
    ///
    /// # Errors
    /// Returns file or parent-directory synchronization errors. Any failure poisons this open
    /// journal because the caller cannot prove which boundary reached stable storage.
    pub fn sync_all_with_parent(&mut self) -> io::Result<()> {
        self.ensure_healthy()?;
        if let Err(error) = self.sync_all_raw(SyncPhase::General) {
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
    }

    /// Install one phase fault after existing scripted faults.
    #[cfg(test)]
    pub(super) fn inject_fault(&self, fault: QueuePlanJournalTestFault) {
        self.injected_faults
            .lock()
            .expect("queue plan journal fault script mutex poisoned")
            .push_back(fault);
    }

    /// Replace the phase-fault script.
    #[cfg(test)]
    pub(super) fn inject_fault_script<I>(&self, faults: I)
    where
        I: IntoIterator<Item = QueuePlanJournalTestFault>,
    {
        let mut script = self
            .injected_faults
            .lock()
            .expect("queue plan journal fault script mutex poisoned");
        script.clear();
        script.extend(faults);
    }

    /// Replay live records from disk.
    ///
    /// # Errors
    /// Returns I/O, bound, consistency, or malformed-frame errors.
    #[cfg(test)]
    pub fn replay(&self) -> io::Result<Vec<QueuePlanJournalRecordV2>> {
        let replay = self.prepare_replay()?;
        let mut records = Vec::with_capacity(replay.len());
        replay.for_each_record(|record| {
            records.push(record);
            Ok(())
        })?;
        Ok(records)
    }

    /// Prepare an inode- and length-stable replay snapshot.
    ///
    /// # Errors
    /// Returns I/O, bound, consistency, or malformed-frame errors.
    pub fn prepare_replay(&self) -> io::Result<QueuePlanJournalReplay> {
        let mut file = open_regular_read(&self.path)?;
        let snapshot_len = file.metadata()?.len();
        ensure_file_bound(snapshot_len, self.limits)?;
        let mut live_positions = BTreeMap::<QueuePlanJournalKey, u64>::new();
        scan_file(
            &mut file,
            snapshot_len,
            self.limits,
            ScanMode::Strict,
            None,
            |position, frame| {
                match frame {
                    QueuePlanJournalFrameV2::Put(record) => {
                        let key = (record.signed_transaction_hash, record.plan_digest());
                        live_positions.insert(key, position);
                    }
                    QueuePlanJournalFrameV2::Remove { hash, plan_digest } => {
                        live_positions.remove(&(hash, plan_digest));
                    }
                }
                Ok(())
            },
        )?;
        if live_positions.len() > self.limits.max_live_records {
            return Err(invalid_data(
                "queue plan journal final live-record replay limit exceeded",
            ));
        }
        file.seek(SeekFrom::Start(0))?;
        Ok(QueuePlanJournalReplay {
            file,
            snapshot_len,
            limits: self.limits,
            live_positions,
        })
    }

    /// Count live records without materializing full journal records.
    ///
    /// # Errors
    /// Returns I/O, bound, consistency, or malformed-frame errors.
    pub fn live_record_count(&self) -> io::Result<usize> {
        Ok(self.prepare_replay()?.len())
    }

    /// Atomically rewrite only live records when the configured threshold warrants it.
    ///
    /// # Errors
    /// Returns I/O, collision, replay, bound, sync, or rename errors. Once the temporary
    /// replacement is created, every failure poisons this open journal.
    pub fn compact_if_needed(&mut self) -> io::Result<()> {
        self.ensure_healthy()?;
        let size = self.file.metadata()?.len();
        ensure_file_bound(size, self.limits)?;
        let replay = if size > self.limits.max_bytes_before_compact {
            Some(self.prepare_replay()?)
        } else if self.tombstones == 0 {
            None
        } else {
            let replay = self.prepare_replay()?;
            let live_records = u64::try_from(replay.len()).unwrap_or(u64::MAX);
            (self.tombstones > live_records).then_some(replay)
        };
        let Some(replay) = replay else {
            return Ok(());
        };

        let tmp = self.path.with_extension("tmp");
        reject_existing_compaction_temp(&tmp)?;
        let compact_result = (|| -> io::Result<File> {
            let mut replacement = OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(&tmp)?;
            #[cfg(test)]
            if self.take_fault(QueuePlanJournalTestFault::CompactionAfterTempCreate) {
                return Err(io::Error::other(
                    "injected queue plan journal compaction failure after temp creation",
                ));
            }
            let mut written = 0_u64;
            replay.for_each_record(|record| {
                let encoded = encode_frame(&QueuePlanJournalFrameV2::Put(record), self.limits)?;
                let encoded_len = u64::try_from(encoded.len())
                    .map_err(|_| invalid_data("queue plan journal frame exceeds u64"))?;
                written = written
                    .checked_add(encoded_len)
                    .ok_or_else(|| invalid_data("queue plan journal compaction size overflow"))?;
                if written > self.limits.max_file_bytes {
                    return Err(invalid_data(
                        "queue plan journal compacted file exceeds the file limit",
                    ));
                }
                replacement.write_all(&encoded)
            })?;
            replacement.sync_all()?;
            fs::rename(&tmp, &self.path)?;
            #[cfg(test)]
            if self.take_fault(QueuePlanJournalTestFault::CompactionAfterRename) {
                return Err(io::Error::other(
                    "injected queue plan journal compaction failure after rename",
                ));
            }
            sync_parent_directory(&self.path)?;
            let append = open_regular_append(&self.path)?;
            drop(replacement);
            Ok(append)
        })();
        match compact_result {
            Ok(file) => {
                self.file = file;
                self.tombstones = 0;
                Ok(())
            }
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }

    fn ensure_healthy(&self) -> io::Result<()> {
        if self.poisoned {
            Err(poisoned_journal_error())
        } else {
            Ok(())
        }
    }

    fn ensure_append_capacity(&self, additional_bytes: usize) -> io::Result<()> {
        let additional_bytes = u64::try_from(additional_bytes)
            .map_err(|_| invalid_data("queue plan journal append size exceeds u64"))?;
        let current_bytes = self.file.metadata()?.len();
        ensure_file_bound(current_bytes, self.limits)?;
        let resulting_bytes = current_bytes
            .checked_add(additional_bytes)
            .ok_or_else(|| invalid_data("queue plan journal append size overflow"))?;
        if resulting_bytes > self.limits.max_file_bytes {
            return Err(invalid_data(
                "queue plan journal append exceeds the file limit",
            ));
        }
        Ok(())
    }

    fn append_encoded(&mut self, encoded: &[u8], phase: AppendPhase) -> Result<(), AppendFailure> {
        let start_len = match self.file.metadata() {
            Ok(metadata) => metadata.len(),
            Err(source) => {
                return Err(AppendFailure {
                    source,
                    definitely_incomplete: true,
                    journal_faulted: false,
                });
            }
        };

        #[cfg(test)]
        if phase == AppendPhase::Put && self.take_fault(QueuePlanJournalTestFault::PutBeforeAppend)
        {
            return Err(AppendFailure {
                source: io::Error::other(
                    "injected queue plan journal failure before strict Put append",
                ),
                definitely_incomplete: true,
                journal_faulted: false,
            });
        }

        #[cfg(test)]
        let inject_partial = match phase {
            AppendPhase::Put => self.take_fault(QueuePlanJournalTestFault::PutPartialWrite),
            AppendPhase::Cleanup => self.take_fault(QueuePlanJournalTestFault::CleanupPartialWrite),
            AppendPhase::OrdinaryRemove => false,
        };
        #[cfg(test)]
        let inject_after_full_write = phase == AppendPhase::Cleanup
            && self.take_fault(QueuePlanJournalTestFault::CleanupAfterFullWrite);
        #[cfg(not(test))]
        let inject_partial = {
            let _ = phase;
            false
        };
        #[cfg(not(test))]
        let inject_after_full_write = false;

        if inject_partial {
            let prefix_len = encoded
                .len()
                .div_ceil(2)
                .min(encoded.len().saturating_sub(1));
            let write_result = self.file.write_all(&encoded[..prefix_len]);
            self.poisoned = true;
            if let Err(source) = write_result {
                return Err(AppendFailure {
                    source,
                    definitely_incomplete: self
                        .append_is_definitely_incomplete(start_len, encoded.len()),
                    journal_faulted: true,
                });
            }
            return Err(AppendFailure {
                source: io::Error::new(
                    io::ErrorKind::WriteZero,
                    "injected partial queue plan journal frame write",
                ),
                definitely_incomplete: true,
                journal_faulted: true,
            });
        }

        if let Err(source) = self.file.write_all(encoded) {
            let definitely_incomplete =
                self.append_is_definitely_incomplete(start_len, encoded.len());
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete,
                journal_faulted: true,
            });
        }
        if inject_after_full_write {
            self.poisoned = true;
            return Err(AppendFailure {
                source: io::Error::other(
                    "injected queue plan journal failure after complete cleanup append",
                ),
                definitely_incomplete: false,
                journal_faulted: true,
            });
        }
        Ok(())
    }

    fn append_is_definitely_incomplete(&self, start_len: u64, encoded_len: usize) -> bool {
        let Ok(encoded_len) = u64::try_from(encoded_len) else {
            return false;
        };
        let Some(expected_end) = start_len.checked_add(encoded_len) else {
            return false;
        };
        self.file
            .metadata()
            .is_ok_and(|metadata| metadata.len() < expected_end)
    }

    fn sync_all_raw(&self, phase: SyncPhase) -> io::Result<()> {
        #[cfg(test)]
        {
            let injected = match phase {
                SyncPhase::Put => self.take_fault(QueuePlanJournalTestFault::PutSync),
                SyncPhase::Cleanup => self.take_fault(QueuePlanJournalTestFault::CleanupSync),
                SyncPhase::General => false,
            };
            if injected {
                return Err(io::Error::other(format!(
                    "injected queue plan journal {phase:?} sync failure"
                )));
            }
        }
        #[cfg(not(test))]
        let _ = phase;

        self.file.sync_all()?;
        #[cfg(test)]
        {
            let injected = match phase {
                SyncPhase::Put => self.take_fault(QueuePlanJournalTestFault::PutParentSync),
                SyncPhase::Cleanup => self.take_fault(QueuePlanJournalTestFault::CleanupParentSync),
                SyncPhase::General => self.take_fault(QueuePlanJournalTestFault::GeneralParentSync),
            };
            if injected {
                return Err(io::Error::other(format!(
                    "injected queue plan journal {phase:?} parent-directory sync failure"
                )));
            }
        }
        sync_parent_directory(&self.path)
    }

    #[cfg(test)]
    fn take_fault(&self, expected: QueuePlanJournalTestFault) -> bool {
        let mut faults = self
            .injected_faults
            .lock()
            .expect("queue plan journal fault script mutex poisoned");
        if faults.front().copied() == Some(expected) {
            faults.pop_front();
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScanMode {
    RepairTerminalTear,
    Strict,
}

fn encode_frame(
    frame: &QueuePlanJournalFrameV2,
    limits: QueuePlanJournalLimits,
) -> io::Result<Vec<u8>> {
    validate_frame(frame)?;
    let payload = norito::to_bytes(frame).map_err(io::Error::other)?;
    let payload_len = u64::try_from(payload.len())
        .map_err(|_| invalid_data("queue plan journal frame payload exceeds u64"))?;
    if payload_len == 0 || payload_len > limits.max_frame_payload_bytes {
        return Err(invalid_data(
            "queue plan journal frame exceeds the configured payload limit",
        ));
    }
    let len = u32::try_from(payload.len())
        .map_err(|_| invalid_data("queue plan journal frame payload exceeds u32"))?;
    encode_payload(&payload, len)
}

fn encode_payload(payload: &[u8], len: u32) -> io::Result<Vec<u8>> {
    if usize::try_from(len).ok() != Some(payload.len()) {
        return Err(invalid_data(
            "queue plan journal payload length does not match its frame header",
        ));
    }
    let version = QUEUE_PLAN_JOURNAL_FRAME_FORMAT_VERSION.to_le_bytes();
    let len_bytes = len.to_le_bytes();
    let len_guard = (!len).to_le_bytes();
    let checksum = frame_checksum(&version, &len_bytes, &len_guard, payload);
    let framed_capacity = QUEUE_PLAN_JOURNAL_FRAME_MAGIC
        .len()
        .checked_add(version.len())
        .and_then(|bytes| bytes.checked_add(len_bytes.len()))
        .and_then(|bytes| bytes.checked_add(len_guard.len()))
        .and_then(|bytes| bytes.checked_add(payload.len()))
        .and_then(|bytes| bytes.checked_add(Hash::LENGTH))
        .and_then(|bytes| bytes.checked_add(QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len()))
        .ok_or_else(|| invalid_data("queue plan journal framed size exceeds usize"))?;
    let mut framed = Vec::with_capacity(framed_capacity);
    framed.extend_from_slice(&QUEUE_PLAN_JOURNAL_FRAME_MAGIC);
    framed.extend_from_slice(&version);
    framed.extend_from_slice(&len_bytes);
    framed.extend_from_slice(&len_guard);
    framed.extend_from_slice(payload);
    framed.extend_from_slice(checksum.as_ref());
    framed.extend_from_slice(&QUEUE_PLAN_JOURNAL_FRAME_COMMIT);
    Ok(framed)
}

fn frame_checksum(version: &[u8; 2], len: &[u8; 4], len_guard: &[u8; 4], payload: &[u8]) -> Hash {
    let mut preimage = Vec::with_capacity(
        QUEUE_PLAN_JOURNAL_FRAME_DOMAIN
            .len()
            .saturating_add(version.len())
            .saturating_add(len.len())
            .saturating_add(len_guard.len())
            .saturating_add(payload.len()),
    );
    preimage.extend_from_slice(QUEUE_PLAN_JOURNAL_FRAME_DOMAIN);
    preimage.extend_from_slice(version);
    preimage.extend_from_slice(len);
    preimage.extend_from_slice(len_guard);
    preimage.extend_from_slice(payload);
    Hash::new(preimage)
}

fn validate_frame(frame: &QueuePlanJournalFrameV2) -> io::Result<()> {
    if let QueuePlanJournalFrameV2::Put(record) = frame
        && record.version != QUEUE_PLAN_JOURNAL_VERSION
    {
        return Err(invalid_data(format!(
            "unsupported queue plan journal record version {}; expected {}",
            record.version, QUEUE_PLAN_JOURNAL_VERSION
        )));
    }
    Ok(())
}

fn decode_frame(
    payload: &[u8],
    limits: QueuePlanJournalLimits,
) -> io::Result<QueuePlanJournalFrameV2> {
    let configured_payload_limit =
        usize::try_from(limits.max_frame_payload_bytes).unwrap_or(usize::MAX);
    if payload.is_empty() || payload.len() > configured_payload_limit {
        return Err(invalid_data(
            "queue plan journal payload exceeds the configured frame limit",
        ));
    }
    let payload_budget = payload.len();
    // Norito retains an aligned archive copy while constructing the owned
    // entrypoint and routing plan. Bound that deterministic wire-to-owned
    // amplification separately from element counts. Small transactions have
    // a comparatively large fixed object-graph cost, so reserve 64 KiB for
    // decoder-owned archive and container metadata instead of multiplying the
    // entire (potentially large) frame by an excessive factor. Allocation
    // bombs remain capped by a fixed allowance plus six times the already
    // bounded frame length.
    let aggregate_element_budget =
        payload_budget.saturating_mul(FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT);
    let aggregate_allocation_budget = frame_decode_allocation_budget(payload_budget);
    let decode_limits = norito::DecodeLimits::new(
        payload_budget,
        payload_budget,
        aggregate_element_budget,
        aggregate_allocation_budget,
        128,
    );
    let frame =
        norito::decode_from_bytes_with_limits::<QueuePlanJournalFrameV2>(payload, decode_limits)
            .map_err(|error| {
                invalid_data(format!(
                    "queue plan journal payload cannot be decoded: {error}"
                ))
            })?;
    validate_frame(&frame)?;
    Ok(frame)
}

fn frame_decode_allocation_budget(payload_bytes: usize) -> usize {
    payload_bytes
        .saturating_mul(FRAME_DECODE_ALLOCATION_AMPLIFICATION_LIMIT)
        .saturating_add(FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES)
}

fn repair_incomplete_tail(path: &Path, limits: QueuePlanJournalLimits) -> io::Result<()> {
    let mut file = open_regular_read_write(path)?;
    let file_len = file.metadata()?.len();
    ensure_file_bound(file_len, limits)?;
    scan_file(
        &mut file,
        file_len,
        limits,
        ScanMode::RepairTerminalTear,
        Some(path),
        |_position, _frame| Ok(()),
    )?;
    // Retry parent durability unconditionally. This closes a crash or prior error between file
    // creation/repair and directory synchronization.
    sync_parent_directory(path)
}

fn scan_file<F>(
    file: &mut File,
    scan_len: u64,
    limits: QueuePlanJournalLimits,
    mode: ScanMode,
    repair_path: Option<&Path>,
    mut handle: F,
) -> io::Result<()>
where
    F: FnMut(u64, QueuePlanJournalFrameV2) -> io::Result<()>,
{
    ensure_file_bound(scan_len, limits)?;
    let actual_len = file.metadata()?.len();
    if scan_len > actual_len {
        return Err(invalid_data(
            "queue plan journal replay snapshot exceeds the opened file",
        ));
    }

    let mut position = 0_u64;
    while position < scan_len {
        file.seek(SeekFrom::Start(position))?;
        let remaining = scan_len
            .checked_sub(position)
            .ok_or_else(|| invalid_data("queue plan journal scan position underflow"))?;
        if remaining < FRAME_HEADER_BYTES {
            let remaining = usize::try_from(remaining)
                .map_err(|_| invalid_data("queue plan journal header prefix exceeds usize"))?;
            let mut prefix = vec![0_u8; remaining];
            file.read_exact(&mut prefix)?;
            if mode == ScanMode::RepairTerminalTear && valid_header_prefix(&prefix, limits) {
                let path = repair_path
                    .ok_or_else(|| invalid_data("queue plan journal repair path is unavailable"))?;
                return truncate_journal_tail(file, position, path);
            }
            return Err(invalid_data(
                "queue plan journal has an invalid or legacy terminal header",
            ));
        }

        let mut magic = [0_u8; QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len()];
        file.read_exact(&mut magic)?;
        if magic != QUEUE_PLAN_JOURNAL_FRAME_MAGIC {
            let message = if position == 0 {
                "queue plan journal uses an unsupported, legacy, or corrupt frame magic"
            } else {
                "queue plan journal frame magic mismatch"
            };
            return Err(invalid_data(message));
        }

        let mut version_bytes = [0_u8; 2];
        file.read_exact(&mut version_bytes)?;
        let version = u16::from_le_bytes(version_bytes);
        if version != QUEUE_PLAN_JOURNAL_FRAME_FORMAT_VERSION {
            return Err(invalid_data(format!(
                "unsupported queue plan journal frame version {version}"
            )));
        }

        let mut len_bytes = [0_u8; 4];
        file.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes);
        let mut len_guard = [0_u8; 4];
        file.read_exact(&mut len_guard)?;
        if u32::from_le_bytes(len_guard) != !len {
            return Err(invalid_data(
                "queue plan journal frame length guard mismatch",
            ));
        }
        let payload_len = u64::from(len);
        if payload_len == 0 || payload_len > limits.max_frame_payload_bytes {
            return Err(invalid_data(
                "queue plan journal frame exceeds the configured payload limit",
            ));
        }
        let frame_len = FRAME_HEADER_BYTES
            .checked_add(payload_len)
            .and_then(|bytes| bytes.checked_add(FRAME_TRAILER_BYTES))
            .ok_or_else(|| invalid_data("queue plan journal frame length overflow"))?;
        let frame_end = position
            .checked_add(frame_len)
            .ok_or_else(|| invalid_data("queue plan journal frame position overflow"))?;
        if frame_end > scan_len {
            if mode == ScanMode::RepairTerminalTear {
                let path = repair_path
                    .ok_or_else(|| invalid_data("queue plan journal repair path is unavailable"))?;
                return truncate_journal_tail(file, position, path);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "queue plan journal has an incomplete frame",
            ));
        }

        let payload_len = usize::try_from(payload_len)
            .map_err(|_| invalid_data("queue plan journal payload length exceeds usize"))?;
        let mut payload = vec![0_u8; payload_len];
        file.read_exact(&mut payload)?;
        let mut checksum = [0_u8; Hash::LENGTH];
        file.read_exact(&mut checksum)?;
        let mut commit = [0_u8; QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len()];
        file.read_exact(&mut commit)?;
        if commit != QUEUE_PLAN_JOURNAL_FRAME_COMMIT {
            return Err(invalid_data(
                "queue plan journal frame commit marker mismatch",
            ));
        }
        if frame_checksum(&version_bytes, &len_bytes, &len_guard, &payload).as_ref() != &checksum {
            return Err(invalid_data("queue plan journal frame checksum mismatch"));
        }
        let frame = decode_frame(&payload, limits)?;
        handle(position, frame)?;
        position = frame_end;
    }
    Ok(())
}

fn valid_header_prefix(prefix: &[u8], limits: QueuePlanJournalLimits) -> bool {
    let magic_prefix_len = prefix.len().min(QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len());
    if prefix[..magic_prefix_len] != QUEUE_PLAN_JOURNAL_FRAME_MAGIC[..magic_prefix_len] {
        return false;
    }
    if prefix.len() <= QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len() {
        return true;
    }

    let version_offset = QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len();
    let version_bytes = QUEUE_PLAN_JOURNAL_FRAME_FORMAT_VERSION.to_le_bytes();
    let version_prefix_len = (prefix.len() - version_offset).min(version_bytes.len());
    if prefix[version_offset..version_offset + version_prefix_len]
        != version_bytes[..version_prefix_len]
    {
        return false;
    }
    if prefix.len() <= version_offset + version_bytes.len() + 4 {
        if prefix.len() == version_offset + version_bytes.len() + 4 {
            let len_offset = version_offset + version_bytes.len();
            let mut len_bytes = [0_u8; 4];
            len_bytes.copy_from_slice(&prefix[len_offset..len_offset + 4]);
            let payload_len = u64::from(u32::from_le_bytes(len_bytes));
            return payload_len != 0 && payload_len <= limits.max_frame_payload_bytes;
        }
        return true;
    }

    let len_offset = version_offset + version_bytes.len();
    let mut len_bytes = [0_u8; 4];
    len_bytes.copy_from_slice(&prefix[len_offset..len_offset + 4]);
    let len = u32::from_le_bytes(len_bytes);
    let payload_len = u64::from(len);
    if payload_len == 0 || payload_len > limits.max_frame_payload_bytes {
        return false;
    }
    let len_guard = (!len).to_le_bytes();
    let guard_offset = len_offset + len_bytes.len();
    let guard_prefix_len = prefix.len() - guard_offset;
    prefix[guard_offset..] == len_guard[..guard_prefix_len]
}

fn truncate_journal_tail(file: &mut File, valid_end: u64, path: &Path) -> io::Result<()> {
    truncate_journal_tail_with_parent_sync(file, valid_end, path, sync_parent_directory)
}

fn truncate_journal_tail_with_parent_sync<F>(
    file: &mut File,
    valid_end: u64,
    path: &Path,
    sync_parent: F,
) -> io::Result<()>
where
    F: FnOnce(&Path) -> io::Result<()>,
{
    file.set_len(valid_end)?;
    file.sync_all()?;
    sync_parent(path)
}

fn ensure_file_bound(file_len: u64, limits: QueuePlanJournalLimits) -> io::Result<()> {
    if file_len > limits.max_file_bytes {
        Err(invalid_data(format!(
            "queue plan journal file size {file_len} exceeds configured limit {}",
            limits.max_file_bytes
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
fn read_frames(
    path: &Path,
    limits: QueuePlanJournalLimits,
) -> io::Result<Vec<QueuePlanJournalFrameV2>> {
    let mut frames = Vec::new();
    let mut file = open_regular_read(path)?;
    let file_len = file.metadata()?.len();
    scan_file(
        &mut file,
        file_len,
        limits,
        ScanMode::Strict,
        None,
        |_position, frame| {
            frames.push(frame);
            Ok(())
        },
    )?;
    Ok(frames)
}

fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = parent_directory(path);
    let metadata = fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid_data(
            "queue plan journal parent must be a non-symlink directory",
        ));
    }
    File::open(parent)?.sync_all()
}

fn prepare_regular_journal_path(path: &Path) -> io::Result<()> {
    prepare_regular_journal_parent(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(invalid_data(
                    "queue plan journal path must be a non-symlink regular file",
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let file = OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(path)?;
            verify_open_regular_path(path, &file)?;
            file.sync_all()?;
        }
        Err(error) => return Err(error),
    }
    sync_parent_directory(path)
}

fn prepare_regular_journal_parent(path: &Path) -> io::Result<()> {
    let parent = parent_directory(path);
    fs::create_dir_all(parent)?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(invalid_data(
            "queue plan journal parent must be a non-symlink directory",
        ));
    }
    Ok(())
}

fn reject_existing_compaction_temp(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(metadata) => {
            let kind = if metadata.file_type().is_symlink() {
                "symlink"
            } else if metadata.is_file() {
                "regular file"
            } else if metadata.is_dir() {
                "directory"
            } else {
                "non-regular file"
            };
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("queue plan journal compaction temp collision with {kind}"),
            ))
        }
        Err(error) => Err(error),
    }
}

fn validate_regular_path(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid_data(
            "queue plan journal path must be a non-symlink regular file",
        ));
    }
    Ok(())
}

fn verify_open_regular_path(path: &Path, file: &File) -> io::Result<()> {
    validate_regular_path(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() {
        return Err(invalid_data(
            "opened queue plan journal is not a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let path_metadata = fs::metadata(path)?;
        if opened.dev() != path_metadata.dev() || opened.ino() != path_metadata.ino() {
            return Err(invalid_data(
                "queue plan journal path changed while it was being opened",
            ));
        }
    }
    Ok(())
}

fn open_regular_append(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let file = OpenOptions::new().append(true).read(true).open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn open_regular_read_write(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn open_regular_read(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let file = File::open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn poisoned_journal_error() -> io::Error {
    io::Error::other("queue plan journal is poisoned after an ambiguous durability boundary")
}

fn invalid_data(error: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn invalid_input(error: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};

    use iroha_data_model::{
        isi::{InstructionBox, Log},
        transaction::TransactionBuilder,
    };
    use iroha_logger::Level;
    use iroha_test_samples::gen_account_in;

    use super::*;

    const TEST_MAX_BYTES: u64 = 4 * 1024 * 1024;

    fn limits(max_live_records: usize) -> QueuePlanJournalLimits {
        QueuePlanJournalLimits::new(
            1024 * 1024,
            TEST_MAX_BYTES,
            TEST_MAX_BYTES,
            max_live_records,
        )
    }

    fn open(path: &Path) -> io::Result<QueuePlanJournal> {
        QueuePlanJournal::open_with_limits(path, limits(64), true)
    }

    fn record(label: &str) -> QueuePlanJournalRecordV2 {
        record_with_message(label, label.to_owned())
    }

    fn record_with_message(label: &str, message: String) -> QueuePlanJournalRecordV2 {
        let instruction: InstructionBox = Log::new(Level::INFO, message).into();
        let chain_id = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("chain id");
        let (account_id, keypair) = gen_account_in(label);
        let tx = TransactionBuilder::new(
            chain_id,
            account_id,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(keypair.private_key());
        let accepted = crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(tx));
        QueuePlanJournalRecordV2::new(
            accepted.entrypoint().clone(),
            accepted.hash(),
            RoutingPlan::single(super::super::RoutingDecision::default()),
            42,
        )
    }

    fn raw_frame(frame: &QueuePlanJournalFrameV2) -> Vec<u8> {
        let payload = norito::to_bytes(frame).expect("encode raw payload");
        let len = u32::try_from(payload.len()).expect("payload length");
        encode_payload(&payload, len).expect("frame payload")
    }

    #[test]
    fn v2_journal_replays_puts_and_exact_removes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("queue-plan.norito");
        let first = record("first");
        let second = record("second");
        {
            let mut journal = open(&path).expect("open journal");
            let flush = journal
                .put_deferred_flush(first.clone())
                .expect("put first")
                .combine(
                    journal
                        .put_deferred_flush(second.clone())
                        .expect("put second"),
                )
                .combine(
                    journal
                        .remove_many_deferred_flush([(
                            first.signed_transaction_hash,
                            first.plan_digest(),
                        )])
                        .expect("remove first"),
                );
            journal.flush_deferred(flush).expect("flush");
        }

        let journal = open(&path).expect("reopen");
        assert_eq!(journal.replay().expect("replay"), vec![second]);
        assert_eq!(
            read_frames(&path, limits(64)).expect("read frames").len(),
            3
        );
    }

    #[test]
    fn strict_put_success_is_live_and_healthy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-success.norito");
        let expected = record("strict-success");
        let mut journal = open(&path).expect("open");

        journal
            .put_strict_durable(expected.clone())
            .expect("strict Put");

        assert!(!journal.is_poisoned());
        assert_eq!(journal.replay().expect("replay"), vec![expected]);
    }

    #[test]
    fn strict_put_preflights_cleanup_capacity_before_writing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-capacity.norito");
        let expected = record("strict-capacity");
        let put_bytes = raw_frame(&QueuePlanJournalFrameV2::Put(expected.clone()));
        let max_file_bytes = u64::try_from(put_bytes.len()).expect("Put frame length");
        let strict_limits = QueuePlanJournalLimits::new(1, TEST_MAX_BYTES, max_file_bytes, 64);
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, strict_limits, true).expect("open");

        let error = journal
            .put_strict_durable(expected)
            .expect_err("cleanup capacity must be reserved before Put");

        assert!(!error.is_indeterminate());
        assert!(!error.journal_faulted());
        assert!(!journal.is_poisoned());
        assert_eq!(path.metadata().expect("metadata").len(), 0);
    }

    #[test]
    fn strict_put_prewrite_failure_is_definitely_not_live_and_healthy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-prewrite.norito");
        let expected = record("strict-prewrite");
        let mut journal = open(&path).expect("open");
        journal.inject_fault(QueuePlanJournalTestFault::PutBeforeAppend);

        let error = journal
            .put_strict_durable(expected)
            .expect_err("prewrite Put must fail");

        assert!(!error.is_indeterminate());
        assert!(!error.journal_faulted());
        assert!(!journal.is_poisoned());
        assert_eq!(path.metadata().expect("metadata").len(), 0);
    }

    #[test]
    fn put_sync_failure_with_durable_cleanup_is_definitely_not_live() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-cleanup.norito");
        let expected = record("strict-cleanup");
        let mut journal = open(&path).expect("open");
        journal.inject_fault(QueuePlanJournalTestFault::PutSync);

        let error = journal
            .put_strict_durable(expected)
            .expect_err("Put sync must fail");

        assert!(!error.is_indeterminate());
        assert!(!error.journal_faulted());
        assert!(!journal.is_poisoned());
        assert!(journal.replay().expect("replay").is_empty());
    }

    #[test]
    fn put_parent_sync_failure_with_durable_cleanup_is_definitely_not_live() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-put-parent-sync.norito");
        let expected = record("strict-put-parent-sync");
        let mut journal = open(&path).expect("open");
        journal.inject_fault(QueuePlanJournalTestFault::PutParentSync);

        let error = journal
            .put_strict_durable(expected)
            .expect_err("Put parent sync must fail");

        assert!(!error.is_indeterminate());
        assert!(!error.journal_faulted());
        assert!(!journal.is_poisoned());
        assert!(journal.replay().expect("replay").is_empty());
    }

    #[test]
    fn partial_put_is_definitely_not_live_but_faults_until_repair() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-partial-put.norito");
        let expected = record("strict-partial-put");
        let mut journal = open(&path).expect("open");
        journal.inject_fault(QueuePlanJournalTestFault::PutPartialWrite);

        let error = journal
            .put_strict_durable(expected)
            .expect_err("partial Put must fail");

        assert!(!error.is_indeterminate());
        assert!(error.journal_faulted());
        assert!(journal.is_poisoned());
        drop(journal);
        assert!(
            open(&path)
                .expect("repair")
                .replay()
                .expect("replay")
                .is_empty()
        );
    }

    #[test]
    fn cleanup_prewrite_failure_is_indeterminate_and_replays_put() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cleanup-prewrite.norito");
        let expected = record("cleanup-prewrite");
        let mut journal = open(&path).expect("open");
        journal.inject_fault_script([
            QueuePlanJournalTestFault::PutSync,
            QueuePlanJournalTestFault::CleanupBeforeAppend,
        ]);

        let error = journal
            .put_strict_durable(expected.clone())
            .expect_err("cleanup prewrite must fail");

        assert!(error.is_indeterminate());
        assert!(error.journal_faulted());
        assert!(journal.is_poisoned());
        drop(journal);
        assert_eq!(
            open(&path).expect("reopen").replay().expect("replay"),
            vec![expected]
        );
    }

    #[test]
    fn cleanup_partial_write_is_indeterminate_and_repairs_to_put() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cleanup-partial.norito");
        let expected = record("cleanup-partial");
        let mut journal = open(&path).expect("open");
        journal.inject_fault_script([
            QueuePlanJournalTestFault::PutSync,
            QueuePlanJournalTestFault::CleanupPartialWrite,
        ]);

        let error = journal
            .put_strict_durable(expected.clone())
            .expect_err("cleanup partial write must fail");

        assert!(error.is_indeterminate());
        assert!(journal.is_poisoned());
        drop(journal);
        assert_eq!(
            open(&path).expect("repair").replay().expect("replay"),
            vec![expected]
        );
    }

    #[test]
    fn cleanup_after_full_write_is_still_reported_indeterminate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cleanup-full-append.norito");
        let expected = record("cleanup-full-append");
        let mut journal = open(&path).expect("open");
        journal.inject_fault_script([
            QueuePlanJournalTestFault::PutSync,
            QueuePlanJournalTestFault::CleanupAfterFullWrite,
        ]);

        let error = journal
            .put_strict_durable(expected)
            .expect_err("cleanup append acknowledgement must fail");

        assert!(error.is_indeterminate());
        assert!(journal.is_poisoned());
        drop(journal);
        assert!(
            open(&path)
                .expect("reopen")
                .replay()
                .expect("replay")
                .is_empty()
        );
    }

    #[test]
    fn cleanup_sync_failure_is_indeterminate_even_when_replay_is_removed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cleanup-sync.norito");
        let expected = record("cleanup-sync");
        let mut journal = open(&path).expect("open");
        journal.inject_fault_script([
            QueuePlanJournalTestFault::PutSync,
            QueuePlanJournalTestFault::CleanupSync,
        ]);

        let error = journal
            .put_strict_durable(expected)
            .expect_err("cleanup sync must fail");

        assert!(error.is_indeterminate());
        assert!(journal.is_poisoned());
        drop(journal);
        assert!(
            open(&path)
                .expect("reopen")
                .replay()
                .expect("replay")
                .is_empty()
        );
    }

    #[test]
    fn cleanup_parent_sync_failure_is_indeterminate_after_file_sync() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cleanup-parent-sync.norito");
        let expected = record("cleanup-parent-sync");
        let mut journal = open(&path).expect("open");
        journal.inject_fault_script([
            QueuePlanJournalTestFault::PutSync,
            QueuePlanJournalTestFault::CleanupParentSync,
        ]);

        let error = journal
            .put_strict_durable(expected)
            .expect_err("cleanup parent sync must fail");

        assert!(error.is_indeterminate());
        assert!(error.journal_faulted());
        assert!(journal.is_poisoned());
        drop(journal);
        assert!(
            open(&path)
                .expect("reopen after cleanup parent failure")
                .replay()
                .expect("replay")
                .is_empty(),
            "the file-synced cleanup is the recovered local state even though parent sync failed"
        );
    }

    #[test]
    fn general_parent_sync_failure_poisoned_until_restart_recovery() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("general-parent-sync.norito");
        let expected = record("general-parent-sync");
        let mut journal = open(&path).expect("open");
        journal
            .put_deferred_flush(expected.clone())
            .expect("append deferred fixture");
        journal.inject_fault(QueuePlanJournalTestFault::GeneralParentSync);

        journal
            .sync_all_with_parent()
            .expect_err("general parent sync must fail");

        assert!(journal.is_poisoned());
        drop(journal);
        assert_eq!(
            open(&path).expect("reopen").replay().expect("replay"),
            vec![expected]
        );
    }

    #[test]
    fn every_recognizable_terminal_v2_prefix_is_repaired_before_append() {
        let expected = record("terminal-prefix");
        let frame = raw_frame(&QueuePlanJournalFrameV2::Put(expected.clone()));
        let payload_len = frame.len()
            - usize::try_from(FRAME_HEADER_BYTES + FRAME_TRAILER_BYTES)
                .expect("constant frame overhead");
        let cuts = [
            1,
            QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len() - 1,
            QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len() + 1,
            usize::try_from(FRAME_HEADER_BYTES).expect("header") - 1,
            usize::try_from(FRAME_HEADER_BYTES).expect("header"),
            usize::try_from(FRAME_HEADER_BYTES).expect("header") + payload_len / 2,
            frame.len() - QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len() / 2,
            frame.len() - 1,
        ];

        for cut in cuts {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("prefix-{cut}.norito"));
            fs::write(&path, &frame[..cut]).expect("write prefix");

            let mut journal = open(&path).expect("repair recognizable prefix");
            assert_eq!(path.metadata().expect("metadata").len(), 0, "cut={cut}");
            journal
                .put_strict_durable(expected.clone())
                .expect("append after repair");
            assert_eq!(journal.replay().expect("replay"), vec![expected.clone()]);
        }
    }

    #[test]
    fn truncate_file_sync_then_parent_failure_is_restart_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("truncate-parent-failure.norito");
        let frame = raw_frame(&QueuePlanJournalFrameV2::Put(record("truncate-parent")));
        fs::write(&path, &frame[..frame.len() / 2]).expect("write recognizable torn frame");
        let mut file = open_regular_read_write(&path).expect("open torn journal");

        let error = truncate_journal_tail_with_parent_sync(&mut file, 0, &path, |_path| {
            Err(io::Error::other(
                "injected parent failure after truncate file sync",
            ))
        })
        .expect_err("parent sync must fail after durable truncation");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(file.metadata().expect("metadata").len(), 0);
        drop(file);
        assert!(
            open(&path)
                .expect("restart retries parent durability")
                .replay()
                .expect("replay")
                .is_empty()
        );
    }

    #[test]
    fn nonempty_legacy_v1_layout_fails_closed_without_rewrite() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("legacy-v1.norito");
        let legacy_payload = norito::to_bytes(&record("legacy")).expect("legacy-ish payload");
        let mut legacy = u32::try_from(legacy_payload.len())
            .expect("payload length")
            .to_le_bytes()
            .to_vec();
        legacy.extend_from_slice(&legacy_payload);
        fs::write(&path, &legacy).expect("write legacy");

        let error = open(&path).err().expect("legacy must fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read(&path).expect("retain legacy"), legacy);
    }

    #[test]
    fn complete_corruption_and_unsupported_versions_fail_without_truncation() {
        let valid = raw_frame(&QueuePlanJournalFrameV2::Put(record("corrupt")));
        let payload_offset = usize::try_from(FRAME_HEADER_BYTES).expect("header");
        let cases = [
            {
                let mut bytes = valid.clone();
                bytes[payload_offset] ^= 0x80;
                ("checksum", bytes)
            },
            {
                let mut bytes = valid.clone();
                let last = bytes.len() - 1;
                bytes[last] ^= 0x80;
                ("commit", bytes)
            },
            {
                let mut bytes = valid.clone();
                bytes[QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len()] ^= 0x01;
                ("outer-version", bytes)
            },
            {
                let mut bytes = valid.clone();
                let len_offset = QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len() + 2;
                bytes[len_offset] ^= 0x01;
                ("length-guard", bytes)
            },
            (
                "norito",
                encode_payload(&[0xA5, 0x5A, 0xC3], 3).expect("encode invalid Norito payload"),
            ),
            {
                let mut unsupported = record("unsupported-record");
                unsupported.version = QUEUE_PLAN_JOURNAL_VERSION + 1;
                (
                    "record-version",
                    raw_frame(&QueuePlanJournalFrameV2::Put(unsupported)),
                )
            },
        ];

        for (label, bytes) in cases {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("{label}.norito"));
            fs::write(&path, &bytes).expect("write corrupt case");
            assert!(open(&path).is_err(), "{label} must fail closed");
            assert_eq!(fs::read(&path).expect("retain evidence"), bytes, "{label}");
        }
    }

    #[test]
    fn oversized_declared_frame_and_file_fail_before_allocation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let oversized_frame = dir.path().join("oversized-frame.norito");
        let declared = u32::try_from(TEST_MAX_BYTES + 1).expect("declared length");
        let mut header = QUEUE_PLAN_JOURNAL_FRAME_MAGIC.to_vec();
        header.extend_from_slice(&QUEUE_PLAN_JOURNAL_FRAME_FORMAT_VERSION.to_le_bytes());
        header.extend_from_slice(&declared.to_le_bytes());
        header.extend_from_slice(&(!declared).to_le_bytes());
        fs::write(&oversized_frame, &header).expect("write oversized header");
        assert_eq!(
            open(&oversized_frame)
                .err()
                .expect("oversized frame")
                .kind(),
            io::ErrorKind::InvalidData
        );

        let oversized_file = dir.path().join("oversized-file.norito");
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&oversized_file)
            .expect("create oversized file");
        file.set_len(TEST_MAX_BYTES + 1).expect("extend file");
        drop(file);
        assert_eq!(
            open(&oversized_file).err().expect("oversized file").kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn decode_budget_accepts_exact_wire_limit_and_rejects_one_byte_over() {
        let frame = QueuePlanJournalFrameV2::Put(record_with_message(
            "decode-budget",
            "x".repeat(256 * 1024),
        ));
        let payload = norito::to_bytes(&frame).expect("encode large canonical frame payload");
        let payload_len = u64::try_from(payload.len()).expect("payload length fits u64");
        let exact_limits = QueuePlanJournalLimits::new(1, payload_len, TEST_MAX_BYTES, 1);

        assert_eq!(
            decode_frame(&payload, exact_limits).expect("decode at exact configured wire limit"),
            frame
        );

        let one_byte_under = QueuePlanJournalLimits::new(1, payload_len - 1, TEST_MAX_BYTES, 1);
        let error = decode_frame(&payload, one_byte_under)
            .expect_err("one byte above the configured frame limit must fail before decode");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains("exceeds the configured frame limit")
        );
    }

    #[test]
    fn replay_enforces_live_record_bound() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live-bound.norito");
        {
            let mut journal =
                QueuePlanJournal::open_with_limits(&path, limits(3), true).expect("open");
            for label in ["one", "two", "three"] {
                journal
                    .put_deferred_flush(record(label))
                    .expect("append bounded record");
            }
            journal.sync_all_with_parent().expect("sync");
        }

        let journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("reopen");
        assert_eq!(
            journal
                .prepare_replay()
                .err()
                .expect("live bound must fail")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn replay_applies_live_bound_to_final_set_not_transient_put_prefix() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("transient-live-prefix.norito");
        let old = record("transient-old");
        let replacement = record("transient-replacement");
        {
            let mut journal =
                QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
            let flush = journal
                .put_deferred_flush(old.clone())
                .expect("append old owner")
                .combine(
                    journal
                        .put_deferred_flush(replacement.clone())
                        .expect("append replacement before old tombstone"),
                )
                .combine(
                    journal
                        .remove_many_deferred_flush([(
                            old.signed_transaction_hash,
                            old.plan_digest(),
                        )])
                        .expect("append delayed old tombstone"),
                );
            journal
                .flush_deferred(flush)
                .expect("flush valid final set");
        }

        let journal = QueuePlanJournal::open_with_limits(&path, limits(1), true).expect("reopen");
        assert_eq!(
            journal.replay().expect("replay bounded final owner"),
            vec![replacement],
            "a capacity-one final set remains valid even when the append prefix briefly held two Puts"
        );
    }

    #[test]
    fn compaction_preserves_live_fifo_order_and_uses_v2_frames() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("compact.norito");
        let first = record("compact-first");
        let second = record("compact-second");
        let third = record("compact-third");
        let compact_limits = QueuePlanJournalLimits::new(1, TEST_MAX_BYTES, TEST_MAX_BYTES, 64);
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, compact_limits, true).expect("open");
        journal
            .put_deferred_flush(first.clone())
            .expect("put first");
        journal
            .put_deferred_flush(second.clone())
            .expect("put second");
        journal
            .put_deferred_flush(third.clone())
            .expect("put third");
        journal
            .remove_many_deferred_flush([(second.signed_transaction_hash, second.plan_digest())])
            .expect("remove second");

        journal.compact_if_needed().expect("compact");

        assert_eq!(
            journal.replay().expect("replay"),
            vec![first.clone(), third.clone()]
        );
        assert_eq!(
            read_frames(&path, compact_limits).expect("read compacted frames"),
            vec![
                QueuePlanJournalFrameV2::Put(first),
                QueuePlanJournalFrameV2::Put(third),
            ]
        );
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn compaction_failure_after_temp_creation_poisoned_and_restart_rejects_temp() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("compact-failure.norito");
        let compact_limits = QueuePlanJournalLimits::new(1, TEST_MAX_BYTES, TEST_MAX_BYTES, 64);
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, compact_limits, true).expect("open");
        journal
            .put_deferred_flush(record("compact-failure"))
            .expect("append compaction fixture");
        journal.inject_fault(QueuePlanJournalTestFault::CompactionAfterTempCreate);

        let error = journal
            .compact_if_needed()
            .expect_err("post-create compaction failure must propagate");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(journal.is_poisoned());
        assert!(path.with_extension("tmp").is_file());
        drop(journal);
        assert_eq!(
            QueuePlanJournal::open_with_limits(&path, compact_limits, true)
                .err()
                .expect("restart must reject unverified compaction temp")
                .kind(),
            io::ErrorKind::AlreadyExists
        );
    }

    #[test]
    fn compaction_rename_then_parent_failure_recovers_replacement_on_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("compact-post-rename.norito");
        let compact_limits = QueuePlanJournalLimits::new(1, TEST_MAX_BYTES, TEST_MAX_BYTES, 64);
        let first = record("compact-post-rename-first");
        let removed = record("compact-post-rename-removed");
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, compact_limits, true).expect("open");
        journal
            .put_deferred_flush(first.clone())
            .expect("append retained record");
        journal
            .put_deferred_flush(removed.clone())
            .expect("append removed record");
        journal
            .remove_many_deferred_flush([(removed.signed_transaction_hash, removed.plan_digest())])
            .expect("append tombstone");
        journal.inject_fault(QueuePlanJournalTestFault::CompactionAfterRename);

        journal
            .compact_if_needed()
            .expect_err("post-rename parent failure must propagate");

        assert!(journal.is_poisoned());
        assert!(
            !path.with_extension("tmp").exists(),
            "rename must consume the replacement before the injected parent failure"
        );
        drop(journal);
        assert_eq!(
            QueuePlanJournal::open_with_limits(&path, compact_limits, true)
                .expect("restart validates renamed replacement")
                .replay()
                .expect("replay replacement"),
            vec![first]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_journal_and_stale_compaction_temp_are_rejected() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("target.norito");
        fs::write(&target, []).expect("target");
        let linked = dir.path().join("linked.norito");
        symlink(&target, &linked).expect("symlink");
        assert!(open(&linked).is_err());

        let path = dir.path().join("stale-temp.norito");
        fs::write(&path, []).expect("journal");
        fs::write(path.with_extension("tmp"), b"stale").expect("temp");
        assert_eq!(
            open(&path).err().expect("stale temp").kind(),
            io::ErrorKind::AlreadyExists
        );

        let symlink_temp_path = dir.path().join("symlink-temp.norito");
        let symlink_temp = symlink_temp_path.with_extension("tmp");
        symlink(&target, &symlink_temp).expect("temp symlink");
        assert_eq!(
            open(&symlink_temp_path).err().expect("symlink temp").kind(),
            io::ErrorKind::AlreadyExists
        );
        assert!(
            !symlink_temp_path.exists(),
            "temp rejection must occur before creating a new journal"
        );
    }
}
