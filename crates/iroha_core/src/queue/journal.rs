//! Crash-safe local Norito journal for pending queue routing plans.

#[cfg(test)]
use std::{
    collections::VecDeque,
    sync::{
        Arc, Barrier, Mutex as StdMutex,
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error as StdError,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::transaction::{SignedTransaction, TransactionEntrypoint};
use norito::codec::{Decode, Encode};

use crate::torii_proxy::QueuePlanAdmissionBindingV2;

use super::{
    LaneQueueReservationKeyV2, QueuePlanAdmissionContextV2, QueuePlanGlobalAdmissionIdentityV2,
    RoutingPlan,
};

const QUEUE_PLAN_JOURNAL_FRAME_DOMAIN: &[u8] = b"iroha:queue-plan-journal-frame:v4";
const QUEUE_PLAN_JOURNAL_RECORD_CLAIM_DOMAIN: &[u8] = b"iroha:queue-plan-journal-record-claim:v4";
const QUEUE_PLAN_JOURNAL_BOOTSTRAP_DOMAIN: &[u8] = b"iroha:queue-plan-journal-bootstrap:v4";
const QUEUE_PLAN_JOURNAL_SNAPSHOT_DOMAIN: &[u8] = b"iroha:queue-plan-journal-snapshot:v4";
const QUEUE_PLAN_JOURNAL_FRAME_MAGIC: [u8; 8] = *b"IRQPJNL4";
const QUEUE_PLAN_JOURNAL_FRAME_COMMIT: [u8; 8] = *b"IRQPEND4";
const QUEUE_PLAN_JOURNAL_FRAME_FORMAT_VERSION: u16 = 4;
const FRAME_HEADER_BYTES: u64 = 8 + 2 + 4 + 4;
const FRAME_TRAILER_BYTES: u64 = Hash::LENGTH as u64 + 8;
// Norito's cumulative element counter for canonical V4 frames stays below the
// wire length. Keep the production allowance at that exact linear bound; the
// calibration tests below independently measure payload-heavy and
// allocation-dense frames.
const FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT: usize = 1;
// Owned decoding retains the canonical archive while materializing nested
// transaction and routing values. Calibrate both a payload-heavy frame and an
// allocation-dense frame, then exercise the latter at the maximum admitted
// instruction count, so this multiplier is not based on one object-graph
// shape.
const FRAME_DECODE_ALLOCATION_AMPLIFICATION_LIMIT: usize = 26;
const FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 64 * 1024;

/// Version of durable queue plan journal records.
pub const QUEUE_PLAN_JOURNAL_VERSION: u16 = 4;

type SignedTxHash = HashOf<SignedTransaction>;
type QueuePlanJournalKey = HashOf<TransactionEntrypoint>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct QueuePlanJournalLivePosition {
    plan_digest: Hash,
    claim_digest: Hash,
    ownership_position: u64,
    record: QueuePlanJournalRecordV4,
}

impl QueuePlanJournalLivePosition {
    fn global_admission_binding(&self) -> io::Result<QueuePlanAdmissionBindingV2> {
        let durable_admission = super::QueuePlanDurableAdmissionV2 {
            version: super::QUEUE_PLAN_DURABLE_ADMISSION_VERSION_V2,
            context: self.record.admission_context.clone(),
            global_admission_identity: self.record.global_admission_identity.clone(),
            routing_plan: self.record.routing_plan.clone(),
            entrypoint_hash: self.record.entrypoint_hash.clone(),
            signed_transaction_hash: self.record.signed_transaction_hash.clone(),
            enqueue_timestamp_ms: self.record.enqueue_timestamp_ms,
            journal_record_digest: self.claim_digest,
        };
        let binding = QueuePlanAdmissionBindingV2::try_from_durable_admission(&durable_admission)
            .map_err(invalid_data)?;
        binding
            .validate_for_transaction_and_plan(&self.record.entrypoint, &self.record.routing_plan)
            .map_err(invalid_data)?;
        Ok(binding)
    }

    fn validate_global_admission_for_reservation_commit(
        &self,
        key: &LaneQueueReservationKeyV2,
    ) -> io::Result<()> {
        if self.record.entrypoint_hash != key.entrypoint_hash
            || self.plan_digest != key.routing_plan_digest
            || self.record.admission_context.proposal_height != key.proposal_height
        {
            return Err(invalid_data(
                "queue plan journal global-admission tombstone does not match the live entrypoint, routing plan, or admitting height",
            ));
        }
        self.global_admission_binding()?
            .validate_for_lane_reservation_commit(key)
            .map_err(invalid_data)
    }
}

/// One exact removal carried by an atomic queue-plan journal batch tombstone.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct QueuePlanJournalRemovalV4 {
    /// Typed canonical queue identity.
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Full routing-plan digest that was removed.
    plan_digest: Hash,
    /// Digest of the exact live Put claim being tombstoned.
    claim_digest: Hash,
}

/// Explicit resource limits for queue plan journal append and replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueuePlanJournalLimits {
    /// File size at which compaction should be considered.
    pub max_bytes_before_compact: u64,
    /// Maximum Norito payload bytes in any one frame.
    pub max_frame_payload_bytes: u64,
    /// Maximum total journal file bytes accepted or appended.
    pub max_file_bytes: u64,
    /// Maximum distinct live entrypoint identities at every replay prefix.
    ///
    /// Repeated Put frames for one entrypoint replace its latest plan without consuming another
    /// slot. A matching Remove releases the slot. Enforcing this while scanning prevents a
    /// final-small journal from amplifying the reconstruction map with an unbounded Put prefix.
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
        let bootstrap_payload = norito::encode_canonical(&bootstrap_frame()).map_err(|error| {
            invalid_input(format!(
                "queue plan journal bootstrap cannot be encoded: {error}"
            ))
        })?;
        let bootstrap_payload_bytes = u64::try_from(bootstrap_payload.len())
            .map_err(|_| invalid_input("queue plan journal bootstrap exceeds u64"))?;
        if bootstrap_payload_bytes == 0 || bootstrap_payload_bytes > self.max_frame_payload_bytes {
            return Err(invalid_input(
                "queue plan journal frame limit cannot hold the V4 bootstrap payload",
            ));
        }
        let bootstrap_frame_bytes = FRAME_HEADER_BYTES
            .checked_add(bootstrap_payload_bytes)
            .and_then(|bytes| bytes.checked_add(FRAME_TRAILER_BYTES))
            .ok_or_else(|| invalid_input("queue plan journal bootstrap frame size overflow"))?;
        if bootstrap_frame_bytes > self.max_file_bytes {
            return Err(invalid_input(
                "queue plan journal file limit cannot hold the V4 bootstrap frame",
            ));
        }
        if bootstrap_frame_bytes > self.max_bytes_before_compact {
            return Err(invalid_input(
                "queue plan journal compaction threshold cannot be smaller than its V4 bootstrap frame",
            ));
        }
        Ok(self)
    }
}

/// Pending transaction routing-plan journal record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct QueuePlanJournalRecordV4 {
    /// Record format version.
    pub version: u16,
    /// Canonical transaction entrypoint.
    pub entrypoint: TransactionEntrypoint,
    /// Canonical queue identity: the typed hash of `entrypoint`.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Real signed-transaction hash when this entrypoint contains one.
    pub signed_transaction_hash: Option<SignedTxHash>,
    /// Full routing plan admitted for this transaction.
    pub routing_plan: RoutingPlan,
    /// Exact coherent committed generation that admitted `routing_plan`.
    pub admission_context: QueuePlanAdmissionContextV2,
    /// Canonical ingress enqueue timestamp in milliseconds.
    pub enqueue_timestamp_ms: u64,
    /// Global chain/request identity for a globally certified admission.
    ///
    /// Ordinary internal queue ownership has no global identity. QueuePlanSynced ownership must
    /// carry this field and reproduce it exactly after restart.
    pub global_admission_identity: Option<QueuePlanGlobalAdmissionIdentityV2>,
}

impl QueuePlanJournalRecordV4 {
    /// Construct a version-4 journal record.
    #[must_use]
    pub fn new(
        entrypoint: TransactionEntrypoint,
        routing_plan: RoutingPlan,
        admission_context: QueuePlanAdmissionContextV2,
        enqueue_timestamp_ms: u64,
        global_admission_identity: Option<QueuePlanGlobalAdmissionIdentityV2>,
    ) -> Self {
        let entrypoint_hash = entrypoint.hash();
        let signed_transaction_hash = match &entrypoint {
            TransactionEntrypoint::External(signed) => Some(signed.hash()),
            TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction().hash()),
            TransactionEntrypoint::SealedCommitment(_)
            | TransactionEntrypoint::PrivateKaigi(_)
            | TransactionEntrypoint::Time(_) => None,
        };
        Self {
            version: QUEUE_PLAN_JOURNAL_VERSION,
            entrypoint,
            entrypoint_hash,
            signed_transaction_hash,
            routing_plan,
            admission_context,
            enqueue_timestamp_ms,
            global_admission_identity,
        }
    }

    /// Digest paired with removals to avoid deleting a re-admitted hash with a new plan.
    #[must_use]
    pub fn plan_digest(&self) -> Hash {
        self.routing_plan.digest()
    }

    /// Digest of the exact canonical record persisted by a strict durable Put.
    ///
    /// This excludes the append-only frame envelope so admission certificates can
    /// rederive the claim from the submitted entrypoint, routing plan, and enqueue
    /// timestamp without depending on a journal file offset.
    ///
    /// # Errors
    /// Returns a Norito encoding error if the canonical record cannot be encoded.
    pub fn claim_digest(&self) -> Result<Hash, norito::Error> {
        norito::encode_canonical(self).map(|bytes| {
            Hash::new_from_chunks(&[QUEUE_PLAN_JOURNAL_RECORD_CLAIM_DOMAIN, bytes.as_slice()])
        })
    }
}

/// One append-only queue plan journal operation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
enum QueuePlanJournalFrameV4 {
    /// Typed file-format marker atomically installed before any ownership operation.
    Bootstrap {
        /// Bootstrap format version.
        version: u16,
        /// Domain-separated identity of this exact first-release journal layout.
        format_digest: Hash,
    },
    /// Add or replace a pending queue record.
    Put(QueuePlanJournalRecordV4),
    /// Tombstone a pending queue record.
    Remove {
        /// Typed canonical queue identity.
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        /// Full routing-plan digest that was removed.
        plan_digest: Hash,
        /// Digest of the exact live Put claim being tombstoned.
        claim_digest: Hash,
    },
    /// Atomically tombstone a validated set of pending queue records.
    ///
    /// Replay applies this frame only when every removal matches the live state at the same
    /// prefix. One absent, duplicate, or mismatched removal invalidates the complete frame.
    RemoveBatch(Vec<QueuePlanJournalRemovalV4>),
}

/// Typed failure from a strict, synchronously durable journal replacement.
#[derive(Debug)]
pub enum QueuePlanJournalStrictPutError {
    /// The replacement is known not to be live after the method returns.
    DefinitelyNotLive {
        /// Underlying validation, capacity, write, or synchronization error.
        source: io::Error,
        /// Whether the journal must remain fail-closed until restart repair.
        journal_faulted: bool,
    },
    /// The replacement may or may not be live and requires restart reconciliation.
    OutcomeIndeterminate {
        /// Underlying append or synchronization error.
        source: io::Error,
    },
}

impl QueuePlanJournalStrictPutError {
    /// Return whether the replacement outcome requires reconciliation.
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
                "queue plan journal replacement is definitely not live (journal_faulted={journal_faulted}): {source}"
            ),
            Self::OutcomeIndeterminate { source } => write!(
                formatter,
                "queue plan journal replacement outcome is indeterminate: {source}"
            ),
        }
    }
}

impl StdError for QueuePlanJournalStrictPutError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(QueuePlanJournalStrictPutError::source(self))
    }
}

/// Result of an exact, synchronously durable queue-plan tombstone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueuePlanJournalExactRemoveResult {
    /// The exact currently live routing plan was durably tombstoned.
    Removed,
    /// No live record exists for this entrypoint; a prior identical removal is already complete.
    AlreadyAbsent,
}

/// Append-only queue plan journal with bounded repair and atomic compaction.
pub struct QueuePlanJournal {
    path: PathBuf,
    limits: QueuePlanJournalLimits,
    durable_writes: bool,
    file: File,
    file_identity: JournalFileIdentity,
    known_len: u64,
    parent: File,
    parent_identity: JournalFileIdentity,
    tombstones: u64,
    poisoned: bool,
    #[cfg(test)]
    replay_scans: AtomicUsize,
    #[cfg(test)]
    injected_faults: StdMutex<VecDeque<QueuePlanJournalTestFault>>,
    #[cfg(test)]
    exact_remove_failure_after: Option<usize>,
    #[cfg(test)]
    append_handoff: StdMutex<Option<(Arc<Barrier>, Arc<Barrier>)>>,
}

/// Test-only journal phase fault.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QueuePlanJournalTestFault {
    /// Fail before writing any bytes of a strict replacement Put frame.
    ReplaceBeforeAppend,
    /// Write only a strict replacement Put frame prefix and report ambiguity.
    ReplacePartialWrite,
    /// Leave one full-length garbage header as a simulated phase-one torn write.
    ReplaceHeaderFullTear,
    /// Stop after durably publishing the frame header and body/checksum, before its commit marker.
    ReplaceAfterBodySync,
    /// Write only a commit-marker prefix after the durable frame body.
    ReplaceCommitPartialWrite,
    /// Write the complete replacement Put, then report ambiguity before commit-marker sync.
    ReplaceAfterFullWrite,
    /// Report failure before strict replacement file synchronization.
    ReplaceSync,
    /// Report failure after replacement file sync but before parent-directory sync.
    ReplaceParentSync,
    /// Report failure after general file sync but before parent-directory sync.
    GeneralParentSync,
    /// Fail after creating the compaction replacement but before writing it.
    CompactionAfterTempCreate,
    /// Fail after replacing the journal path but before syncing its parent.
    CompactionAfterRename,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppendPhase {
    Replace,
    #[cfg(test)]
    OrdinaryPut,
    OrdinaryRemove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SyncPhase {
    Replace,
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
    path: PathBuf,
    file: File,
    file_identity: JournalFileIdentity,
    parent: File,
    parent_identity: JournalFileIdentity,
    snapshot_len: u64,
    snapshot_digest: Hash,
    live_positions: BTreeMap<QueuePlanJournalKey, QueuePlanJournalLivePosition>,
    removed_positions: BTreeMap<QueuePlanJournalKey, QueuePlanJournalLivePosition>,
}

struct PendingCompactionTemp {
    path: PathBuf,
    file: File,
    file_identity: JournalFileIdentity,
    snapshot_len: u64,
    parent: File,
    parent_identity: JournalFileIdentity,
}

impl PendingCompactionTemp {
    fn verify(&self) -> io::Result<()> {
        if verify_open_regular_parent(&self.path, &self.parent)? != self.parent_identity {
            return Err(invalid_data(
                "queue plan journal compaction temp parent identity changed during recovery",
            ));
        }
        if verify_open_regular_path(&self.path, &self.file)? != self.file_identity {
            return Err(invalid_data(
                "queue plan journal compaction temp path no longer identifies its recovery snapshot",
            ));
        }
        let metadata = self.file.metadata()?;
        if journal_file_identity(&metadata) != self.file_identity
            || !journal_file_is_single_link(&metadata)
            || metadata.len() != self.snapshot_len
        {
            return Err(invalid_data(
                "queue plan journal compaction temp identity, link count, or length changed during recovery",
            ));
        }
        if verify_open_regular_parent(&self.path, &self.parent)? != self.parent_identity {
            return Err(invalid_data(
                "queue plan journal compaction temp parent changed while validating recovery",
            ));
        }
        Ok(())
    }
}

impl QueuePlanJournalReplay {
    fn verify_snapshot_storage(&self) -> io::Result<()> {
        if verify_open_regular_parent(&self.path, &self.parent)? != self.parent_identity {
            return Err(invalid_data(
                "queue plan journal replay parent identity changed after snapshot",
            ));
        }
        if verify_open_regular_path(&self.path, &self.file)? != self.file_identity {
            return Err(invalid_data(
                "queue plan journal replay path no longer identifies its snapshot file",
            ));
        }
        let metadata = self.file.metadata()?;
        if journal_file_identity(&metadata) != self.file_identity
            || !journal_file_is_single_link(&metadata)
            || metadata.len() != self.snapshot_len
        {
            return Err(invalid_data(
                "queue plan journal replay snapshot identity, link count, or length changed",
            ));
        }
        if verify_open_regular_parent(&self.path, &self.parent)? != self.parent_identity {
            return Err(invalid_data(
                "queue plan journal replay parent identity changed while validating snapshot",
            ));
        }
        Ok(())
    }

    fn verify_snapshot_content(&mut self) -> io::Result<()> {
        self.verify_snapshot_storage()?;
        let observed = journal_snapshot_digest(&mut self.file, self.snapshot_len)?;
        self.verify_snapshot_storage()?;
        if observed != self.snapshot_digest {
            return Err(invalid_data(
                "queue plan journal replay snapshot content changed after preparation",
            ));
        }
        Ok(())
    }

    /// Return the number of live records captured by this replay snapshot.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live_positions.len()
    }

    /// Authenticate and materialize every live record in original append order.
    ///
    /// The complete prepared snapshot, every materialized record, and the complete snapshot
    /// again are verified before any record leaves this method. This makes the returned vector
    /// the only externally visible result of replay verification: a malformed later record or
    /// same-length snapshot drift cannot expose an authenticated-looking prefix.
    ///
    /// # Errors
    /// Returns I/O, malformed-frame, materialized-record, or snapshot consistency errors.
    pub fn into_verified_records(mut self) -> io::Result<Vec<QueuePlanJournalRecordV4>> {
        let records = self.live_positions.len();
        self.verify_snapshot_content()?;

        let mut ordered = std::mem::take(&mut self.live_positions)
            .into_iter()
            .collect::<Vec<_>>();
        ordered.sort_unstable_by_key(|(_key, live)| live.ownership_position);
        let mut verified = Vec::with_capacity(records);
        for (entrypoint_hash, live) in ordered {
            self.verify_snapshot_storage()?;
            let record = live.record;
            let claim_digest = record.claim_digest().map_err(|error| {
                invalid_data(format!(
                    "queue plan journal materialized live frame claim cannot be encoded: {error}"
                ))
            })?;
            if record.entrypoint_hash != entrypoint_hash
                || record.plan_digest() != live.plan_digest
                || claim_digest != live.claim_digest
            {
                return Err(invalid_data(
                    "queue plan journal materialized live frame identity, plan digest, or claim changed",
                ));
            }
            verified.push(record);
        }
        self.verify_snapshot_content()?;
        Ok(verified)
    }

    /// Visit verified live records in original append order with `handle_record`.
    ///
    /// Replay authenticates and materializes the complete bounded snapshot before the first
    /// callback. Errors returned by the callback can still occur after an earlier callback has
    /// completed.
    ///
    /// # Errors
    /// Returns I/O errors, malformed-frame errors, snapshot consistency errors, or errors
    /// returned by `handle_record`.
    pub fn for_each_record<F>(self, mut handle_record: F) -> io::Result<usize>
    where
        F: FnMut(QueuePlanJournalRecordV4) -> io::Result<()>,
    {
        let records = self.into_verified_records()?;
        let record_count = records.len();
        for record in records {
            handle_record(record)?;
        }
        Ok(record_count)
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
    /// Open or create a bounded V4 queue plan journal.
    ///
    /// An empty canonical file is only an initialization state: before this method returns it is
    /// atomically replaced by a durable typed V4 bootstrap frame. Every initialized journal must
    /// begin with exactly that marker, and every nonempty legacy, headerless, or unknown layout
    /// fails closed. Startup repairs only a bounded terminal tear admitted by the staged V4 write
    /// protocol after the bootstrap and durably synchronizes both the repaired file and its parent
    /// directory.
    ///
    /// # Errors
    /// Returns path validation, repair, limit, corruption, or file-opening errors.
    pub fn open_with_limits(
        path: impl AsRef<Path>,
        limits: QueuePlanJournalLimits,
        durable_writes: bool,
    ) -> io::Result<Self> {
        let limits = limits.validate()?;
        let requested_path = normalize_platform_managed_alias(path.as_ref())?;
        prepare_regular_journal_parent(&requested_path)?;
        let path = canonical_journal_path(&requested_path)?;
        prepare_regular_journal_parent(&path)?;
        let pending_compaction = open_pending_compaction_temp(&path.with_extension("tmp"), limits)?;
        if pending_compaction.is_some() {
            match fs::symlink_metadata(&path) {
                Ok(canonical)
                    if !journal_file_is_indirect(&canonical)
                        && canonical.is_file()
                        && journal_file_is_single_link(&canonical)
                        && canonical.len() != 0 => {}
                Ok(_) => {
                    return Err(invalid_data(
                        "queue plan journal compaction temp requires an initialized single-link canonical journal",
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Err(invalid_data(
                        "queue plan journal compaction temp cannot recreate a missing canonical journal without a source-bound completion marker",
                    ));
                }
                Err(error) => return Err(error),
            }
        }
        prepare_regular_journal_path(&path)?;
        ensure_durable_v4_bootstrap(&path, limits)?;
        repair_incomplete_tail(&path, limits)?;
        if let Some(pending_compaction) = pending_compaction {
            reconcile_pending_compaction_temp(&path, limits, pending_compaction)?;
        }
        let mut file = open_regular_append(&path)?;
        let file_identity = verify_open_regular_path(&path, &file)?;
        let known_len = file.metadata()?.len();
        let parent = open_regular_parent(&path)?;
        let parent_identity = verify_open_regular_parent(&path, &parent)?;
        // Bind `known_len` only after validating the exact bytes seen through the cached append
        // handle. This closes the interval between startup repair and handle binding in which a
        // second writer could otherwise extend the inode and have unvalidated bytes adopted as the
        // journal's trusted length.
        scan_file(
            &mut file,
            known_len,
            limits,
            ScanMode::Strict,
            None,
            |_position, _frame| Ok(()),
        )?;
        if verify_open_regular_path(&path, &file)? != file_identity
            || file.metadata()?.len() != known_len
            || verify_open_regular_parent(&path, &parent)? != parent_identity
        {
            return Err(invalid_data(
                "queue plan journal file, parent identity, or length changed while binding its cached handles",
            ));
        }
        Ok(Self {
            path,
            limits,
            durable_writes,
            file,
            file_identity,
            known_len,
            parent,
            parent_identity,
            tombstones: 0,
            poisoned: false,
            #[cfg(test)]
            replay_scans: AtomicUsize::new(0),
            #[cfg(test)]
            injected_faults: StdMutex::new(VecDeque::new()),
            #[cfg(test)]
            exact_remove_failure_after: None,
            #[cfg(test)]
            append_handoff: StdMutex::new(None),
        })
    }

    /// Return whether an append or durability boundary requires restart repair.
    #[cfg(test)]
    #[must_use]
    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Return the number of full content-bound replay scans performed by this open journal.
    #[cfg(test)]
    #[must_use]
    pub fn replay_scan_count(&self) -> usize {
        self.replay_scans.load(AtomicOrdering::Relaxed)
    }

    /// Reset the test-only full replay-scan counter.
    #[cfg(test)]
    pub fn reset_replay_scan_count(&self) {
        self.replay_scans.store(0, AtomicOrdering::Relaxed);
    }

    /// Durably replace the live record for one canonical queue identity.
    ///
    /// The replacement publishes its fixed header, body/checksum, and commit marker through three
    /// ordered file-durability phases, followed by parent-directory synchronization. Replay treats
    /// the latest complete Put for an entrypoint as authoritative, so a retry is idempotent and no
    /// compensating Remove is ever emitted. Replacement-only history is compacted proactively at
    /// the configured threshold and forced once before a capacity rejection. Once any replacement
    /// bytes may have been written, every failure is indeterminate and poisons this open journal
    /// until restart repair determines which complete frame is live.
    ///
    /// # Errors
    /// Returns a typed result distinguishing a pre-append rejection from an outcome requiring
    /// restart reconciliation.
    pub fn replace_strict_durable(
        &mut self,
        record: QueuePlanJournalRecordV4,
    ) -> Result<(), QueuePlanJournalStrictPutError> {
        if self.poisoned {
            return Err(QueuePlanJournalStrictPutError::indeterminate(
                poisoned_journal_error(),
            ));
        }

        let replacement = encode_frame(&QueuePlanJournalFrameV4::Put(record), self.limits)
            .map_err(|error| QueuePlanJournalStrictPutError::definitely_not_live(error, false))?;
        if let Err(initial_capacity_error) = self.ensure_append_capacity(replacement.len()) {
            if self.poisoned {
                return Err(QueuePlanJournalStrictPutError::indeterminate(
                    initial_capacity_error,
                ));
            }
            // Replacement-only histories have no tombstones, so waiting for a Remove-triggered
            // compaction can permanently exhaust `max_file_bytes`. A forced preflight rewrite
            // retains one latest Put per entrypoint in its original FIFO ownership order. The
            // incoming replacement is not part of that rewrite, hence any preflight failure is
            // definitely-not-live even though the journal itself must remain faulted.
            if let Err(error) = self.compact(true) {
                self.poisoned = true;
                return Err(QueuePlanJournalStrictPutError::definitely_not_live(
                    error, true,
                ));
            }
            if let Err(error) = self.ensure_append_capacity(replacement.len()) {
                return Err(if self.poisoned {
                    QueuePlanJournalStrictPutError::indeterminate(error)
                } else {
                    QueuePlanJournalStrictPutError::definitely_not_live(error, false)
                });
            }
        }

        if let Err(failure) = self.append_encoded(&replacement, AppendPhase::Replace) {
            let error = if failure.definitely_incomplete && !failure.journal_faulted {
                QueuePlanJournalStrictPutError::definitely_not_live(failure.source, false)
            } else {
                QueuePlanJournalStrictPutError::indeterminate(failure.source)
            };
            return Err(error);
        }

        if let Err(error) = self.sync_all_raw(SyncPhase::Replace) {
            return Err(QueuePlanJournalStrictPutError::indeterminate(error));
        }
        // Compact only after the replacement frame and its parent entry are durable. If this
        // rewrite fails, the replacement may be reachable through either the old append history
        // or the renamed compacted file, so the only sound result is indeterminate reconciliation.
        if self.known_len > self.limits.max_bytes_before_compact
            && let Err(error) = self.compact(false)
        {
            self.poisoned = true;
            return Err(QueuePlanJournalStrictPutError::indeterminate(error));
        }
        Ok(())
    }

    /// Durably tombstone the exact currently live routing plan for one entrypoint.
    ///
    /// The journal first reconstructs and content-binds its bounded live snapshot. An absent
    /// entrypoint is an idempotent success. A present entrypoint with any other plan digest fails
    /// closed without appending; a stale cleanup can therefore never delete a replacement plan
    /// for the same queue identity.
    ///
    /// # Errors
    /// Returns malformed-snapshot, mismatched-plan, capacity, compaction, append, or durability
    /// errors. Any ambiguous append or synchronization boundary poisons this open journal.
    pub fn remove_exact_strict_durable(
        &mut self,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        plan_digest: Hash,
        claim_digest: Hash,
    ) -> io::Result<QueuePlanJournalExactRemoveResult> {
        self.remove_many_exact_strict_durable([(entrypoint_hash, plan_digest, claim_digest)])?
            .into_iter()
            .next()
            .ok_or_else(|| invalid_data("queue plan journal exact tombstone result is missing"))
    }

    /// Durably tombstone a batch of exact currently live routing plans.
    ///
    /// The complete content-bound live snapshot and every requested identity are validated before
    /// the first Remove append. Absent targets are idempotent successes. If capacity requires a
    /// compaction, all targets are revalidated as one batch before append. A crash may therefore
    /// leave only a durable prefix of tombstones, and retry safely treats that prefix as absent.
    ///
    /// # Errors
    /// Returns malformed-snapshot, duplicate-target, mismatched-claim, capacity, compaction,
    /// append, or durability errors. Any ambiguous append or synchronization boundary poisons this
    /// open journal.
    pub fn remove_many_exact_strict_durable<I>(
        &mut self,
        removals: I,
    ) -> io::Result<Vec<QueuePlanJournalExactRemoveResult>>
    where
        I: IntoIterator<Item = (HashOf<TransactionEntrypoint>, Hash, Hash)>,
    {
        self.ensure_healthy()?;
        let removals = removals.into_iter().collect::<Vec<_>>();
        if removals.is_empty() {
            return Ok(Vec::new());
        }
        let mut unique_targets = BTreeMap::new();
        for (entrypoint_hash, plan_digest, claim_digest) in &removals {
            if unique_targets
                .insert(*entrypoint_hash, (*plan_digest, *claim_digest))
                .is_some()
            {
                return Err(invalid_input(
                    "queue plan journal exact tombstone batch contains a duplicate entrypoint",
                ));
            }
        }

        let classify =
            |mut replay: QueuePlanJournalReplay|
             -> io::Result<Vec<QueuePlanJournalExactRemoveResult>> {
                replay.verify_snapshot_content()?;
                removals
                    .iter()
                    .map(
                        |(entrypoint_hash, plan_digest, claim_digest)| match replay
                            .live_positions
                            .get(entrypoint_hash)
                        {
                            None => Ok(QueuePlanJournalExactRemoveResult::AlreadyAbsent),
                            Some(live)
                                if live.plan_digest != *plan_digest
                                    || live.claim_digest != *claim_digest =>
                            {
                                Err(invalid_data(
                                    "queue plan journal exact tombstone does not match the currently live claim",
                                ))
                            }
                            Some(_) => Ok(QueuePlanJournalExactRemoveResult::Removed),
                        },
                    )
                    .collect()
            };
        let results = classify(self.prepare_replay()?)?;
        let mut encoded_frames = Vec::new();
        let mut encoded_bytes = 0_usize;
        for ((entrypoint_hash, plan_digest, claim_digest), result) in removals.iter().zip(&results)
        {
            if *result == QueuePlanJournalExactRemoveResult::AlreadyAbsent {
                continue;
            }
            let encoded = encode_frame(
                &QueuePlanJournalFrameV4::Remove {
                    entrypoint_hash: *entrypoint_hash,
                    plan_digest: *plan_digest,
                    claim_digest: *claim_digest,
                },
                self.limits,
            )?;
            encoded_bytes = encoded_bytes.checked_add(encoded.len()).ok_or_else(|| {
                invalid_data("queue plan journal exact tombstone batch capacity overflow")
            })?;
            encoded_frames.push(encoded);
        }
        if encoded_frames.is_empty() {
            return Ok(results);
        }

        if let Err(initial_capacity_error) = self.ensure_append_capacity(encoded_bytes) {
            if self.poisoned {
                return Err(initial_capacity_error);
            }
            self.compact(true)?;
            if classify(self.prepare_replay()?)? != results {
                return Err(invalid_data(
                    "queue plan journal exact tombstone batch changed during preflight compaction",
                ));
            }
            self.ensure_append_capacity(encoded_bytes)?;
        }
        for encoded in encoded_frames {
            self.append_encoded(&encoded, AppendPhase::OrdinaryRemove)
                .map_err(|failure| failure.source)?;
            self.tombstones = self.tombstones.saturating_add(1);
            #[cfg(test)]
            if let Some(remaining) = self.exact_remove_failure_after {
                if remaining <= 1 {
                    self.exact_remove_failure_after = None;
                    self.poisoned = true;
                    return Err(io::Error::other(
                        "injected queue plan journal failure after a durable exact tombstone prefix",
                    ));
                }
                self.exact_remove_failure_after = Some(remaining - 1);
            }
        }
        self.sync_all_raw(SyncPhase::General)?;
        Ok(results)
    }

    /// Atomically and durably tombstone a bounded batch of exact queue-plan claims.
    ///
    /// Every `(entrypoint, plan, claim)` identity is checked against one complete,
    /// content-bound replay snapshot before append. A target must be either the exact currently
    /// live claim or the exact claim retained by its latest tombstone; an unproven absence,
    /// duplicate, mismatch, or ABA replacement rejects the complete batch without appending.
    /// All currently live targets are encoded in one staged `RemoveBatch` frame and that complete
    /// frame is synchronously persisted before success is returned. Replay therefore observes
    /// either every new tombstone or none of them, never a durable member prefix.
    ///
    /// # Errors
    /// Returns malformed-target, duplicate, absent, mismatched, snapshot, capacity, compaction,
    /// append, or synchronization errors. Any ambiguous append or synchronization boundary
    /// poisons this open journal.
    pub fn remove_many_exact_atomic_strict_durable(
        &mut self,
        removals: &[(HashOf<TransactionEntrypoint>, Hash, Hash)],
    ) -> io::Result<Vec<QueuePlanJournalExactRemoveResult>> {
        self.remove_many_exact_atomic_strict_durable_inner(removals, false)
    }

    /// Atomically and durably tombstone a bounded batch that must be wholly live.
    ///
    /// Unlike [`Self::remove_many_exact_atomic_strict_durable`], this startup-publication form
    /// rejects an exactly retained tombstone before append. Success therefore proves that every
    /// requested live claim participated in the one durable `RemoveBatch`; the caller has no
    /// post-durability outcome vector to interpret.
    ///
    /// # Errors
    /// Returns the same errors as [`Self::remove_many_exact_atomic_strict_durable`], and rejects
    /// any target that is already absent before writing.
    pub fn remove_all_live_exact_atomic_strict_durable(
        &mut self,
        removals: &[(HashOf<TransactionEntrypoint>, Hash, Hash)],
    ) -> io::Result<()> {
        let _ = self.remove_many_exact_atomic_strict_durable_inner(removals, true)?;
        Ok(())
    }

    fn remove_many_exact_atomic_strict_durable_inner(
        &mut self,
        removals: &[(HashOf<TransactionEntrypoint>, Hash, Hash)],
        require_all_live: bool,
    ) -> io::Result<Vec<QueuePlanJournalExactRemoveResult>> {
        self.ensure_healthy()?;
        if removals.is_empty() {
            return Ok(Vec::new());
        }
        if removals.len() > self.limits.max_live_records {
            return Err(invalid_input(
                "queue plan journal atomic exact-removal batch exceeds the live-record limit",
            ));
        }

        let requested = removals
            .iter()
            .map(
                |(entrypoint_hash, plan_digest, claim_digest)| QueuePlanJournalRemovalV4 {
                    entrypoint_hash: *entrypoint_hash,
                    plan_digest: *plan_digest,
                    claim_digest: *claim_digest,
                },
            )
            .collect::<Vec<_>>();
        validate_frame(&QueuePlanJournalFrameV4::RemoveBatch(requested.clone()))?;
        let entrypoints = requested
            .iter()
            .map(|removal| removal.entrypoint_hash)
            .collect::<BTreeSet<_>>();

        let classify = |mut replay: QueuePlanJournalReplay| -> io::Result<(
            Vec<QueuePlanJournalExactRemoveResult>,
            Vec<QueuePlanJournalRemovalV4>,
        )> {
            replay.verify_snapshot_content()?;
            let mut outcomes = Vec::with_capacity(requested.len());
            let mut live_removals = Vec::with_capacity(requested.len());
            for requested in &requested {
                if let Some(live) = replay.live_positions.get(&requested.entrypoint_hash) {
                    if live.plan_digest != requested.plan_digest
                        || live.claim_digest != requested.claim_digest
                    {
                        return Err(invalid_data(
                            "queue plan journal atomic exact-removal target does not match the currently live claim",
                        ));
                    }
                    outcomes.push(QueuePlanJournalExactRemoveResult::Removed);
                    live_removals.push(requested.clone());
                    continue;
                }
                let Some(removed) = replay.removed_positions.get(&requested.entrypoint_hash) else {
                    return Err(invalid_data(
                        "queue plan journal atomic exact-removal target is neither live nor exactly tombstoned",
                    ));
                };
                if removed.plan_digest != requested.plan_digest
                    || removed.claim_digest != requested.claim_digest
                {
                    return Err(invalid_data(
                        "queue plan journal atomic exact-removal target does not match its retained tombstone",
                    ));
                }
                outcomes.push(QueuePlanJournalExactRemoveResult::AlreadyAbsent);
            }
            replay.verify_snapshot_content()?;
            Ok((outcomes, live_removals))
        };

        let (outcomes, live_removals) =
            classify(self.prepare_replay_with_removed_entrypoints(Some(&entrypoints))?)?;
        if require_all_live
            && (live_removals.len() != requested.len()
                || outcomes
                    .iter()
                    .any(|outcome| *outcome != QueuePlanJournalExactRemoveResult::Removed))
        {
            return Err(invalid_data(
                "queue plan journal atomic live-removal batch contains an already-absent target",
            ));
        }
        if live_removals.is_empty() {
            return Ok(outcomes);
        }
        let encoded = encode_frame(
            &QueuePlanJournalFrameV4::RemoveBatch(live_removals.clone()),
            self.limits,
        )?;
        if let Err(initial_capacity_error) = self.ensure_append_capacity(encoded.len()) {
            if self.poisoned {
                return Err(initial_capacity_error);
            }
            if outcomes.contains(&QueuePlanJournalExactRemoveResult::AlreadyAbsent) {
                // Compaction intentionally drops tombstone history. Preserve exact retry
                // provenance for the already-absent members instead of weakening their claims.
                return Err(initial_capacity_error);
            }
            self.compact(true)?;
            let compacted =
                classify(self.prepare_replay_with_removed_entrypoints(Some(&entrypoints))?)?;
            if compacted != (outcomes.clone(), live_removals.clone()) {
                return Err(invalid_data(
                    "queue plan journal atomic exact-removal batch changed during preflight compaction",
                ));
            }
            self.ensure_append_capacity(encoded.len())?;
        }

        self.append_encoded(&encoded, AppendPhase::OrdinaryRemove)
            .map_err(|failure| failure.source)?;
        self.tombstones = self
            .tombstones
            .saturating_add(u64::try_from(live_removals.len()).unwrap_or(u64::MAX));
        self.sync_all_raw(SyncPhase::General)?;
        Ok(outcomes)
    }

    /// Durably tombstone one live global admission using its externally committed binding hash.
    ///
    /// This restart-safe form does not trust the caller to retain the process-local journal claim
    /// digest. It reconstructs and validates the complete global admission binding from the live
    /// V4 record in a content-bound replay snapshot, checks the exact entrypoint, routing-plan
    /// digest, and canonical binding hash supplied by the caller, then delegates to the ordinary
    /// exact tombstone with the live record's claim digest. An absent entrypoint is an idempotent
    /// success; every mismatch against a live record fails closed without appending.
    ///
    /// # Errors
    /// Returns malformed-snapshot, non-global-record, binding-validation, identity, binding-hash,
    /// capacity, compaction, append, or durability errors. Any ambiguous append or
    /// synchronization boundary poisons this open journal.
    pub fn remove_exact_global_admission_binding_strict_durable(
        &mut self,
        key: &LaneQueueReservationKeyV2,
    ) -> io::Result<QueuePlanJournalExactRemoveResult> {
        self.ensure_healthy()?;
        key.validate().map_err(invalid_data)?;
        let entrypoint_hash = key.entrypoint_hash.clone();
        let (live_plan_digest, live_claim_digest) = {
            let mut replay = self.prepare_replay()?;
            replay.verify_snapshot_content()?;
            let Some(live) = replay.live_positions.get(&entrypoint_hash) else {
                return Ok(QueuePlanJournalExactRemoveResult::AlreadyAbsent);
            };
            live.validate_global_admission_for_reservation_commit(key)?;
            (live.plan_digest, live.claim_digest)
        };

        self.remove_exact_strict_durable(entrypoint_hash, live_plan_digest, live_claim_digest)
    }

    /// Atomically and durably tombstone a validated batch of global queue-plan admissions.
    ///
    /// The complete input is validated against one content-bound replay snapshot before any
    /// append. Each entrypoint may appear only once. Live entries must match the full externally
    /// committed reservation key; an absent entry is accepted only when this journal still
    /// carries the exact prior tombstone for the same global admission, which makes a crash
    /// between this batch and reservation-journal `ForgetCommit` safe to retry. One absent,
    /// mismatched, duplicate, or ABA-replaced entry rejects the entire batch without appending.
    /// Every newly removed claim is encoded in one staged `RemoveBatch` frame and synchronously
    /// persisted before success is returned.
    ///
    /// # Errors
    /// Returns malformed-key, duplicate, absent, mismatched, ABA, snapshot, capacity, compaction,
    /// append, or synchronization errors. Any ambiguous append or synchronization boundary
    /// poisons this open journal.
    pub fn remove_exact_global_admission_bindings_strict_durable(
        &mut self,
        keys: &[LaneQueueReservationKeyV2],
    ) -> io::Result<Vec<QueuePlanJournalExactRemoveResult>> {
        self.ensure_healthy()?;
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        if keys.len() > self.limits.max_live_records {
            return Err(invalid_input(
                "queue plan journal atomic removal batch exceeds the live-record limit",
            ));
        }

        let mut entrypoints = BTreeSet::new();
        for key in keys {
            key.validate().map_err(invalid_data)?;
            if !entrypoints.insert(key.entrypoint_hash.clone()) {
                return Err(invalid_data(
                    "queue plan journal atomic removal batch contains a duplicate entrypoint",
                ));
            }
        }

        let mut outcomes = Vec::with_capacity(keys.len());
        let mut live_candidates = Vec::with_capacity(keys.len());
        {
            let mut replay = self.prepare_replay_with_removed_entrypoints(Some(&entrypoints))?;
            replay.verify_snapshot_content()?;
            for key in keys {
                if let Some(live) = replay.live_positions.get(&key.entrypoint_hash) {
                    live.validate_global_admission_for_reservation_commit(key)?;
                    let removal = QueuePlanJournalRemovalV4 {
                        entrypoint_hash: key.entrypoint_hash.clone(),
                        plan_digest: live.plan_digest,
                        claim_digest: live.claim_digest,
                    };
                    outcomes.push(QueuePlanJournalExactRemoveResult::Removed);
                    live_candidates.push((*key, removal));
                    continue;
                }
                let Some(removed) = replay.removed_positions.get(&key.entrypoint_hash) else {
                    return Err(invalid_data(
                        "queue plan journal atomic removal target is neither live nor exactly tombstoned",
                    ));
                };
                removed.validate_global_admission_for_reservation_commit(key)?;
                outcomes.push(QueuePlanJournalExactRemoveResult::AlreadyAbsent);
            }
        }

        if live_candidates.is_empty() {
            return Ok(outcomes);
        }
        let removals = live_candidates
            .iter()
            .map(|(_key, removal)| removal.clone())
            .collect::<Vec<_>>();
        let encoded = encode_frame(&QueuePlanJournalFrameV4::RemoveBatch(removals), self.limits)?;
        if let Err(initial_capacity_error) = self.ensure_append_capacity(encoded.len()) {
            if self.poisoned {
                return Err(initial_capacity_error);
            }
            if outcomes.contains(&QueuePlanJournalExactRemoveResult::AlreadyAbsent) {
                // Compaction intentionally drops tombstone history. Keep exact retry evidence
                // intact when this batch contains barriers left over from a previously durable
                // batch; the operator can enlarge the bounded journal corridor and retry.
                return Err(initial_capacity_error);
            }
            self.compact(true)?;
            let mut replay = self.prepare_replay()?;
            replay.verify_snapshot_content()?;
            for (key, expected) in &live_candidates {
                let Some(live) = replay.live_positions.get(&key.entrypoint_hash) else {
                    return Err(invalid_data(
                        "queue plan journal atomic removal target disappeared during preflight compaction",
                    ));
                };
                live.validate_global_admission_for_reservation_commit(key)?;
                if live.plan_digest != expected.plan_digest
                    || live.claim_digest != expected.claim_digest
                {
                    return Err(invalid_data(
                        "queue plan journal atomic removal target changed during preflight compaction",
                    ));
                }
            }
            self.ensure_append_capacity(encoded.len())?;
        }
        self.append_encoded(&encoded, AppendPhase::OrdinaryRemove)
            .map_err(|failure| failure.source)?;
        self.tombstones = self
            .tombstones
            .saturating_add(u64::try_from(live_candidates.len()).unwrap_or(u64::MAX));
        self.sync_all_raw(SyncPhase::General)?;
        Ok(outcomes)
    }

    /// Append a Put frame and return deferred durability work for the caller.
    ///
    /// # Errors
    /// Returns validation, capacity, encoding, or I/O errors.
    #[cfg(test)]
    pub fn put_deferred_flush(
        &mut self,
        record: QueuePlanJournalRecordV4,
    ) -> io::Result<QueuePlanJournalFlush> {
        self.ensure_healthy()?;
        let encoded = encode_frame(&QueuePlanJournalFrameV4::Put(record), self.limits)?;
        self.ensure_append_capacity(encoded.len())?;
        self.append_encoded(&encoded, AppendPhase::OrdinaryPut)
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
        I: IntoIterator<Item = (HashOf<TransactionEntrypoint>, Hash, Hash)>,
    {
        self.ensure_healthy()?;
        let mut encoded_frames = Vec::new();
        let mut encoded_bytes = 0_usize;
        for (entrypoint_hash, plan_digest, claim_digest) in removals {
            let encoded = encode_frame(
                &QueuePlanJournalFrameV4::Remove {
                    entrypoint_hash,
                    plan_digest,
                    claim_digest,
                },
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
        if flush.sync_data {
            self.sync_data_verified()?;
        }
        if flush.compact
            && let Err(error) = self.compact_if_needed()
        {
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
    }

    /// Synchronize appended data while retaining exclusive journal ownership.
    ///
    /// # Errors
    /// Returns path-identity, link-count, or synchronization errors. Every failure poisons this
    /// open journal because an unlocked or stale-inode synchronization cannot prove durability.
    pub fn sync_data_verified(&mut self) -> io::Result<()> {
        self.ensure_healthy()?;
        if let Err(error) = self.verify_cached_storage() {
            self.poisoned = true;
            return Err(error);
        }
        if let Err(error) = self.file.sync_data() {
            self.poisoned = true;
            return Err(error);
        }
        if let Err(error) = self.verify_cached_storage() {
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
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

    /// Pause the next append before it touches storage for queue-lock concurrency tests.
    #[cfg(test)]
    pub(super) fn install_append_handoff(&self, reached: Arc<Barrier>, resume: Arc<Barrier>) {
        *self
            .append_handoff
            .lock()
            .expect("queue plan journal append handoff mutex poisoned") = Some((reached, resume));
    }

    /// Fail a strict exact-removal batch after this many tombstones have been durably appended.
    #[cfg(test)]
    pub(super) fn inject_exact_remove_failure_after_durable_tombstones(
        &mut self,
        durable_tombstones: usize,
    ) {
        assert!(
            durable_tombstones > 0,
            "an exact-remove prefix fault requires at least one durable tombstone"
        );
        self.exact_remove_failure_after = Some(durable_tombstones);
    }

    /// Replay live records from disk.
    ///
    /// # Errors
    /// Returns I/O, bound, consistency, or malformed-frame errors.
    #[cfg(test)]
    pub fn replay(&self) -> io::Result<Vec<QueuePlanJournalRecordV4>> {
        self.prepare_replay()?.into_verified_records()
    }

    /// Prepare an inode- and length-stable replay snapshot.
    ///
    /// # Errors
    /// Returns I/O, bound, consistency, or malformed-frame errors.
    pub fn prepare_replay(&self) -> io::Result<QueuePlanJournalReplay> {
        self.prepare_replay_with_removed_entrypoints(None)
    }

    fn prepare_replay_with_removed_entrypoints(
        &self,
        removed_entrypoints: Option<&BTreeSet<QueuePlanJournalKey>>,
    ) -> io::Result<QueuePlanJournalReplay> {
        self.ensure_healthy()?;
        #[cfg(test)]
        self.replay_scans.fetch_add(1, AtomicOrdering::Relaxed);
        self.verify_cached_storage()?;
        let mut file = open_regular_read(&self.path)?;
        if verify_open_regular_path(&self.path, &file)? != self.file_identity {
            return Err(invalid_data(
                "queue plan journal replay handle does not match the cached append handle",
            ));
        }
        let snapshot_len = file.metadata()?.len();
        ensure_file_bound(snapshot_len, self.limits)?;
        let snapshot_digest = journal_snapshot_digest(&mut file, snapshot_len)?;
        if verify_open_regular_path(&self.path, &file)? != self.file_identity
            || file.metadata()?.len() != snapshot_len
        {
            return Err(invalid_data(
                "queue plan journal identity or length changed while starting replay content binding",
            ));
        }
        self.verify_cached_storage()?;
        let mut live_positions =
            BTreeMap::<QueuePlanJournalKey, QueuePlanJournalLivePosition>::new();
        let mut removed_positions =
            BTreeMap::<QueuePlanJournalKey, QueuePlanJournalLivePosition>::new();
        scan_file(
            &mut file,
            snapshot_len,
            self.limits,
            ScanMode::Strict,
            None,
            |position, frame| {
                match frame {
                    QueuePlanJournalFrameV4::Bootstrap { .. } => {}
                    QueuePlanJournalFrameV4::Put(record) => {
                        let key = record.entrypoint_hash;
                        if !live_positions.contains_key(&key)
                            && live_positions.len() >= self.limits.max_live_records
                        {
                            return Err(invalid_data(
                                "queue plan journal distinct live-record reconstruction limit exceeded",
                            ));
                        }
                        let plan_digest = record.plan_digest();
                        let claim_digest = record.claim_digest().map_err(|error| {
                            invalid_data(format!(
                                "queue plan journal live frame claim cannot be encoded: {error}"
                            ))
                        })?;
                        if removed_entrypoints.is_some_and(|entrypoints| entrypoints.contains(&key))
                        {
                            removed_positions.remove(&key);
                        }
                        match live_positions.entry(key) {
                            std::collections::btree_map::Entry::Occupied(mut entry) => {
                                let live = entry.get_mut();
                                live.plan_digest = plan_digest;
                                live.claim_digest = claim_digest;
                                live.record = record;
                            }
                            std::collections::btree_map::Entry::Vacant(entry) => {
                                entry.insert(QueuePlanJournalLivePosition {
                                    plan_digest,
                                    claim_digest,
                                    ownership_position: position,
                                    record,
                                });
                            }
                        }
                    }
                    QueuePlanJournalFrameV4::Remove {
                        entrypoint_hash,
                        plan_digest,
                        claim_digest,
                    } => {
                        if live_positions.get(&entrypoint_hash).is_some_and(|live| {
                            live.plan_digest == plan_digest && live.claim_digest == claim_digest
                        }) && let Some(removed) = live_positions.remove(&entrypoint_hash)
                        {
                            if removed_entrypoints
                                .is_some_and(|entrypoints| entrypoints.contains(&entrypoint_hash))
                            {
                                removed_positions.insert(entrypoint_hash, removed);
                            }
                        }
                    }
                    QueuePlanJournalFrameV4::RemoveBatch(removals) => {
                        if removals.iter().any(|removal| {
                            !live_positions
                                .get(&removal.entrypoint_hash)
                                .is_some_and(|live| {
                                    live.plan_digest == removal.plan_digest
                                        && live.claim_digest == removal.claim_digest
                                })
                        }) {
                            return Err(invalid_data(
                                "queue plan journal atomic RemoveBatch does not match its complete live prefix",
                            ));
                        }
                        for removal in removals {
                            let removed = live_positions.remove(&removal.entrypoint_hash).ok_or_else(
                                || {
                                    invalid_data(
                                        "queue plan journal atomic RemoveBatch target disappeared while applying",
                                    )
                                },
                            )?;
                            if removed_entrypoints.is_some_and(|entrypoints| {
                                entrypoints.contains(&removal.entrypoint_hash)
                            }) {
                                removed_positions.insert(removal.entrypoint_hash, removed);
                            }
                        }
                    }
                }
                Ok(())
            },
        )?;
        if verify_open_regular_path(&self.path, &file)? != self.file_identity
            || file.metadata()?.len() != snapshot_len
        {
            return Err(invalid_data(
                "queue plan journal identity or length changed while preparing replay",
            ));
        }
        self.verify_cached_storage()?;
        if journal_snapshot_digest(&mut file, snapshot_len)? != snapshot_digest {
            return Err(invalid_data(
                "queue plan journal content changed while preparing replay",
            ));
        }
        if verify_open_regular_path(&self.path, &file)? != self.file_identity
            || file.metadata()?.len() != snapshot_len
        {
            return Err(invalid_data(
                "queue plan journal identity or length changed while binding replay content",
            ));
        }
        self.verify_cached_storage()?;
        file.seek(SeekFrom::Start(0))?;
        Ok(QueuePlanJournalReplay {
            path: self.path.clone(),
            file,
            file_identity: self.file_identity,
            parent: self.parent.try_clone()?,
            parent_identity: self.parent_identity,
            snapshot_len,
            snapshot_digest,
            live_positions,
            removed_positions,
        })
    }

    /// Count live records through the same bounded, content-bound snapshot used by replay.
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
        self.compact(false)
    }

    fn compact(&mut self, force: bool) -> io::Result<()> {
        self.ensure_healthy()?;
        let size = self.verify_cached_storage_or_poison()?.len();
        ensure_file_bound(size, self.limits)?;
        let replay = if force || size > self.limits.max_bytes_before_compact {
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
        self.verify_cached_storage_or_poison()?;
        reject_existing_compaction_temp(&tmp)?;
        let compact_result = (|| -> io::Result<(File, JournalFileIdentity, u64)> {
            self.verify_cached_storage_or_poison()?;
            let mut replacement = open_new_regular(&tmp)?;
            let replacement_identity = verify_open_regular_path(&tmp, &replacement)?;
            if verify_open_regular_parent(&self.path, &self.parent)? != self.parent_identity {
                return Err(invalid_data(
                    "queue plan journal parent changed while creating compaction replacement",
                ));
            }
            replacement.sync_all()?;
            self.parent.sync_all()?;
            if verify_open_regular_path(&tmp, &replacement)? != replacement_identity
                || replacement.metadata()?.len() != 0
                || verify_open_regular_parent(&self.path, &self.parent)? != self.parent_identity
            {
                return Err(invalid_data(
                    "queue plan journal empty compaction replacement changed across creation synchronization",
                ));
            }
            #[cfg(test)]
            if self.take_fault(QueuePlanJournalTestFault::CompactionAfterTempCreate) {
                return Err(io::Error::other(
                    "injected queue plan journal compaction failure after temp creation",
                ));
            }
            let bootstrap = encode_bootstrap_frame(self.limits)?;
            write_staged_frame(&mut replacement, &bootstrap)?;
            let mut written = u64::try_from(bootstrap.len())
                .map_err(|_| invalid_data("queue plan journal bootstrap exceeds u64"))?;
            replay.for_each_record(|record| {
                let encoded = encode_frame(&QueuePlanJournalFrameV4::Put(record), self.limits)?;
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
                write_staged_frame(&mut replacement, &encoded)
            })?;
            if verify_open_regular_path(&tmp, &replacement)? != replacement_identity
                || replacement.metadata()?.len() != written
            {
                return Err(invalid_data(
                    "queue plan journal compaction replacement identity or length changed while writing",
                ));
            }
            replacement.sync_all()?;
            if verify_open_regular_path(&tmp, &replacement)? != replacement_identity
                || replacement.metadata()?.len() != written
            {
                return Err(invalid_data(
                    "queue plan journal compaction replacement identity or length changed while synchronizing",
                ));
            }
            self.verify_cached_storage_or_poison()?;
            persist_atomic_replacement(&tmp, &self.path)?;
            if verify_open_regular_path(&self.path, &replacement)? != replacement_identity
                || replacement.metadata()?.len() != written
            {
                return Err(invalid_data(
                    "queue plan journal compaction rename did not install the verified replacement bytes",
                ));
            }
            if verify_open_regular_parent(&self.path, &self.parent)? != self.parent_identity {
                return Err(invalid_data(
                    "queue plan journal parent changed during compaction rename",
                ));
            }
            #[cfg(test)]
            if self.take_fault(QueuePlanJournalTestFault::CompactionAfterRename) {
                return Err(io::Error::other(
                    "injected queue plan journal compaction failure after rename",
                ));
            }
            self.parent.sync_all()?;
            if verify_open_regular_parent(&self.path, &self.parent)? != self.parent_identity
                || verify_open_regular_path(&self.path, &replacement)? != replacement_identity
                || replacement.metadata()?.len() != written
            {
                return Err(invalid_data(
                    "queue plan journal compaction identities changed across parent synchronization",
                ));
            }
            let append = open_regular_append(&self.path)?;
            if verify_open_regular_path(&self.path, &append)? != replacement_identity
                || append.metadata()?.len() != written
            {
                return Err(invalid_data(
                    "queue plan journal compaction append handle does not match replacement",
                ));
            }
            drop(replacement);
            Ok((append, replacement_identity, written))
        })();
        match compact_result {
            Ok((file, file_identity, known_len)) => {
                self.file = file;
                self.file_identity = file_identity;
                self.known_len = known_len;
                self.tombstones = 0;
                self.verify_cached_storage_or_poison().map(|_| ())
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

    fn verify_cached_parent(&self) -> io::Result<()> {
        let identity = verify_open_regular_parent(&self.path, &self.parent)?;
        if identity != self.parent_identity {
            return Err(invalid_data(
                "queue plan journal cached parent identity no longer matches its canonical path",
            ));
        }
        Ok(())
    }

    fn verify_cached_storage(&self) -> io::Result<fs::Metadata> {
        self.verify_cached_storage_at_len(self.known_len)
    }

    fn verify_cached_storage_at_len(&self, expected_len: u64) -> io::Result<fs::Metadata> {
        self.verify_cached_parent()?;
        let identity = verify_open_regular_path(&self.path, &self.file)?;
        if identity != self.file_identity {
            return Err(invalid_data(
                "queue plan journal cached append handle no longer matches its canonical path",
            ));
        }
        let metadata = self.file.metadata()?;
        if journal_file_identity(&metadata) != self.file_identity
            || !journal_file_is_single_link(&metadata)
            || metadata.len() != expected_len
        {
            return Err(invalid_data(
                "queue plan journal cached append handle identity, link count, or length changed",
            ));
        }
        self.verify_cached_parent()?;
        Ok(metadata)
    }

    fn verify_cached_storage_or_poison(&mut self) -> io::Result<fs::Metadata> {
        match self.verify_cached_storage() {
            Ok(metadata) => Ok(metadata),
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }

    fn sync_cached_parent(&mut self) -> io::Result<()> {
        if let Err(error) = self.verify_cached_parent() {
            self.poisoned = true;
            return Err(error);
        }
        if let Err(error) = self.parent.sync_all() {
            self.poisoned = true;
            return Err(error);
        }
        if let Err(error) = self.verify_cached_storage() {
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
    }

    fn ensure_append_capacity(&mut self, additional_bytes: usize) -> io::Result<()> {
        let additional_bytes = u64::try_from(additional_bytes)
            .map_err(|_| invalid_data("queue plan journal append size exceeds u64"))?;
        let current_bytes = self.verify_cached_storage_or_poison()?.len();
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
        let start_len = match self.verify_cached_storage_or_poison() {
            Ok(metadata) => metadata.len(),
            Err(source) => {
                return Err(AppendFailure {
                    source,
                    definitely_incomplete: true,
                    journal_faulted: true,
                });
            }
        };
        let (header_end_in_frame, commit_start_in_frame) = match staged_frame_boundaries(encoded) {
            Ok(boundaries) => boundaries,
            Err(source) => {
                return Err(AppendFailure {
                    source,
                    definitely_incomplete: true,
                    journal_faulted: false,
                });
            }
        };
        let expected_end = match u64::try_from(encoded.len())
            .ok()
            .and_then(|encoded_len| start_len.checked_add(encoded_len))
        {
            Some(expected_end) => expected_end,
            None => {
                return Err(AppendFailure {
                    source: invalid_data("queue plan journal append end offset overflow"),
                    definitely_incomplete: true,
                    journal_faulted: false,
                });
            }
        };
        let header_end = match u64::try_from(header_end_in_frame)
            .ok()
            .and_then(|bytes| start_len.checked_add(bytes))
        {
            Some(header_end) => header_end,
            None => {
                return Err(AppendFailure {
                    source: invalid_data("queue plan journal staged header end overflow"),
                    definitely_incomplete: true,
                    journal_faulted: false,
                });
            }
        };
        let body_end = match u64::try_from(commit_start_in_frame)
            .ok()
            .and_then(|bytes| start_len.checked_add(bytes))
        {
            Some(body_end) => body_end,
            None => {
                return Err(AppendFailure {
                    source: invalid_data("queue plan journal staged body end overflow"),
                    definitely_incomplete: true,
                    journal_faulted: false,
                });
            }
        };

        #[cfg(test)]
        if let Some((reached, resume)) = self
            .append_handoff
            .lock()
            .expect("queue plan journal append handoff mutex poisoned")
            .take()
        {
            reached.wait();
            resume.wait();
        }

        #[cfg(test)]
        if phase == AppendPhase::Replace
            && self.take_fault(QueuePlanJournalTestFault::ReplaceBeforeAppend)
        {
            return Err(AppendFailure {
                source: io::Error::other(
                    "injected queue plan journal failure before strict replacement append",
                ),
                definitely_incomplete: true,
                journal_faulted: false,
            });
        }

        #[cfg(test)]
        let inject_partial = match phase {
            AppendPhase::Replace => self.take_fault(QueuePlanJournalTestFault::ReplacePartialWrite),
            AppendPhase::OrdinaryPut | AppendPhase::OrdinaryRemove => false,
        };
        #[cfg(test)]
        let inject_full_header_tear = phase == AppendPhase::Replace
            && self.take_fault(QueuePlanJournalTestFault::ReplaceHeaderFullTear);
        #[cfg(test)]
        let inject_after_body_sync = phase == AppendPhase::Replace
            && self.take_fault(QueuePlanJournalTestFault::ReplaceAfterBodySync);
        #[cfg(test)]
        let inject_partial_commit = phase == AppendPhase::Replace
            && self.take_fault(QueuePlanJournalTestFault::ReplaceCommitPartialWrite);
        #[cfg(test)]
        let inject_after_full_write = phase == AppendPhase::Replace
            && self.take_fault(QueuePlanJournalTestFault::ReplaceAfterFullWrite);
        #[cfg(not(test))]
        let inject_partial = {
            let _ = phase;
            false
        };
        #[cfg(not(test))]
        let inject_full_header_tear = false;
        #[cfg(not(test))]
        let inject_after_body_sync = false;
        #[cfg(not(test))]
        let inject_partial_commit = false;
        #[cfg(not(test))]
        let inject_after_full_write = false;

        if inject_partial {
            let prefix_len = header_end_in_frame
                .div_ceil(2)
                .min(header_end_in_frame.saturating_sub(1));
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

        if inject_full_header_tear {
            let torn_header = vec![0xA5_u8; header_end_in_frame];
            let write_result = self.file.write_all(&torn_header);
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
                    "injected full-length torn queue plan journal header",
                ),
                definitely_incomplete: true,
                journal_faulted: true,
            });
        }

        if let Err(source) = self.file.write_all(&encoded[..header_end_in_frame]) {
            let definitely_incomplete =
                self.append_is_definitely_incomplete(start_len, encoded.len());
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete,
                journal_faulted: true,
            });
        }
        if let Err(source) = self.verify_cached_storage_at_len(header_end) {
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete: self
                    .append_is_definitely_incomplete(start_len, encoded.len()),
                journal_faulted: true,
            });
        }
        if let Err(source) = self.file.sync_all() {
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete: self
                    .append_is_definitely_incomplete(start_len, encoded.len()),
                journal_faulted: true,
            });
        }
        if let Err(source) = self.verify_cached_storage_at_len(header_end) {
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete: self
                    .append_is_definitely_incomplete(start_len, encoded.len()),
                journal_faulted: true,
            });
        }

        if let Err(source) = self
            .file
            .write_all(&encoded[header_end_in_frame..commit_start_in_frame])
        {
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete: self
                    .append_is_definitely_incomplete(start_len, encoded.len()),
                journal_faulted: true,
            });
        }
        if let Err(source) = self.verify_cached_storage_at_len(body_end) {
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete: self
                    .append_is_definitely_incomplete(start_len, encoded.len()),
                journal_faulted: true,
            });
        }
        if let Err(source) = self.file.sync_all() {
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete: self
                    .append_is_definitely_incomplete(start_len, encoded.len()),
                journal_faulted: true,
            });
        }
        if let Err(source) = self.verify_cached_storage_at_len(body_end) {
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete: self
                    .append_is_definitely_incomplete(start_len, encoded.len()),
                journal_faulted: true,
            });
        }
        if inject_after_body_sync {
            self.poisoned = true;
            return Err(AppendFailure {
                source: io::Error::other(
                    "injected queue plan journal failure after durable replacement body",
                ),
                definitely_incomplete: true,
                journal_faulted: true,
            });
        }

        if inject_partial_commit {
            let commit = &encoded[commit_start_in_frame..];
            let prefix_len = commit.len().div_ceil(2).min(commit.len().saturating_sub(1));
            if let Err(source) = self.file.write_all(&commit[..prefix_len]) {
                self.poisoned = true;
                return Err(AppendFailure {
                    source,
                    definitely_incomplete: self
                        .append_is_definitely_incomplete(start_len, encoded.len()),
                    journal_faulted: true,
                });
            }
            self.poisoned = true;
            return Err(AppendFailure {
                source: io::Error::new(
                    io::ErrorKind::WriteZero,
                    "injected partial queue plan journal commit-marker write",
                ),
                definitely_incomplete: true,
                journal_faulted: true,
            });
        }

        if let Err(source) = self.file.write_all(&encoded[commit_start_in_frame..]) {
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete: self
                    .append_is_definitely_incomplete(start_len, encoded.len()),
                journal_faulted: true,
            });
        }
        if inject_after_full_write {
            self.poisoned = true;
            return Err(AppendFailure {
                source: io::Error::other(
                    "injected queue plan journal failure after complete replacement append",
                ),
                definitely_incomplete: false,
                journal_faulted: true,
            });
        }
        if let Err(source) = self.verify_cached_storage_at_len(expected_end) {
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete: self
                    .append_is_definitely_incomplete(start_len, encoded.len()),
                journal_faulted: true,
            });
        }
        if let Err(source) = self.file.sync_all() {
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete: self
                    .append_is_definitely_incomplete(start_len, encoded.len()),
                journal_faulted: true,
            });
        }
        if let Err(source) = self.verify_cached_storage_at_len(expected_end) {
            self.poisoned = true;
            return Err(AppendFailure {
                source,
                definitely_incomplete: self
                    .append_is_definitely_incomplete(start_len, encoded.len()),
                journal_faulted: true,
            });
        }
        self.known_len = expected_end;
        match self.verify_cached_storage_or_poison() {
            Ok(metadata) if metadata.len() == expected_end => {}
            Ok(metadata) => {
                self.poisoned = true;
                return Err(AppendFailure {
                    source: invalid_data(format!(
                        "queue plan journal append length changed concurrently: expected {expected_end}, observed {}",
                        metadata.len()
                    )),
                    definitely_incomplete: metadata.len() < expected_end,
                    journal_faulted: true,
                });
            }
            Err(source) => {
                return Err(AppendFailure {
                    source,
                    definitely_incomplete: false,
                    journal_faulted: true,
                });
            }
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

    fn sync_all_raw(&mut self, phase: SyncPhase) -> io::Result<()> {
        self.verify_cached_storage_or_poison()?;
        #[cfg(test)]
        {
            let injected = match phase {
                SyncPhase::Replace => self.take_fault(QueuePlanJournalTestFault::ReplaceSync),
                SyncPhase::General => false,
            };
            if injected {
                self.poisoned = true;
                return Err(io::Error::other(format!(
                    "injected queue plan journal {phase:?} sync failure"
                )));
            }
        }
        #[cfg(not(test))]
        let _ = phase;

        if let Err(error) = self.file.sync_all() {
            self.poisoned = true;
            return Err(error);
        }
        self.verify_cached_storage_or_poison()?;
        #[cfg(test)]
        {
            let injected = match phase {
                SyncPhase::Replace => self.take_fault(QueuePlanJournalTestFault::ReplaceParentSync),
                SyncPhase::General => self.take_fault(QueuePlanJournalTestFault::GeneralParentSync),
            };
            if injected {
                self.poisoned = true;
                return Err(io::Error::other(format!(
                    "injected queue plan journal {phase:?} parent-directory sync failure"
                )));
            }
        }
        self.sync_cached_parent()
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
    frame: &QueuePlanJournalFrameV4,
    limits: QueuePlanJournalLimits,
) -> io::Result<Vec<u8>> {
    validate_frame(frame)?;
    let payload = norito::encode_canonical(frame).map_err(io::Error::other)?;
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

fn staged_frame_boundaries(encoded: &[u8]) -> io::Result<(usize, usize)> {
    let header_end = usize::try_from(FRAME_HEADER_BYTES)
        .map_err(|_| invalid_data("queue plan journal frame header exceeds usize"))?;
    let commit_bytes = QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len();
    let commit_start = encoded.len().checked_sub(commit_bytes).ok_or_else(|| {
        invalid_data("queue plan journal frame is shorter than its commit marker")
    })?;
    if header_end >= commit_start
        || encoded
            .get(commit_start..)
            .is_none_or(|commit| commit != QUEUE_PLAN_JOURNAL_FRAME_COMMIT)
    {
        return Err(invalid_data(
            "queue plan journal staged frame has invalid header/body/commit boundaries",
        ));
    }
    Ok((header_end, commit_start))
}

fn write_staged_frame(file: &mut File, encoded: &[u8]) -> io::Result<()> {
    // Recovery relies on this order: no body byte is issued before the exact fixed header is
    // durable, and no commit-marker byte is issued before the exact body/checksum is durable.
    let (header_end, commit_start) = staged_frame_boundaries(encoded)?;
    file.write_all(&encoded[..header_end])?;
    file.sync_all()?;
    file.write_all(&encoded[header_end..commit_start])?;
    file.sync_all()?;
    file.write_all(&encoded[commit_start..])?;
    file.sync_all()
}

fn bootstrap_frame() -> QueuePlanJournalFrameV4 {
    QueuePlanJournalFrameV4::Bootstrap {
        version: QUEUE_PLAN_JOURNAL_VERSION,
        format_digest: Hash::new(QUEUE_PLAN_JOURNAL_BOOTSTRAP_DOMAIN),
    }
}

fn encode_bootstrap_frame(limits: QueuePlanJournalLimits) -> io::Result<Vec<u8>> {
    encode_frame(&bootstrap_frame(), limits)
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

fn validate_frame(frame: &QueuePlanJournalFrameV4) -> io::Result<()> {
    match frame {
        QueuePlanJournalFrameV4::Bootstrap {
            version,
            format_digest,
        } => {
            if *version != QUEUE_PLAN_JOURNAL_VERSION
                || *format_digest != Hash::new(QUEUE_PLAN_JOURNAL_BOOTSTRAP_DOMAIN)
            {
                return Err(invalid_data(
                    "queue plan journal bootstrap identity does not match V4",
                ));
            }
        }
        QueuePlanJournalFrameV4::Put(record) => {
            if record.version != QUEUE_PLAN_JOURNAL_VERSION {
                return Err(invalid_data(format!(
                    "unsupported queue plan journal record version {}; expected {}",
                    record.version, QUEUE_PLAN_JOURNAL_VERSION
                )));
            }
            if record.entrypoint_hash != record.entrypoint.hash() {
                return Err(invalid_data(
                    "queue plan journal entrypoint hash does not match its canonical entrypoint",
                ));
            }
            let expected_signed_hash = match &record.entrypoint {
                TransactionEntrypoint::External(signed) => Some(signed.hash()),
                TransactionEntrypoint::SealedReveal(reveal) => {
                    Some(reveal.signed_transaction().hash())
                }
                TransactionEntrypoint::SealedCommitment(_)
                | TransactionEntrypoint::PrivateKaigi(_)
                | TransactionEntrypoint::Time(_) => None,
            };
            if record.signed_transaction_hash != expected_signed_hash {
                return Err(invalid_data(
                    "queue plan journal signed-transaction hash does not match its entrypoint",
                ));
            }
            record
                .admission_context
                .validate_for_routing_plan(&record.routing_plan)
                .map_err(invalid_data)?;
            if let Some(identity) = record.global_admission_identity.as_ref() {
                if identity.version != super::QUEUE_PLAN_GLOBAL_ADMISSION_IDENTITY_VERSION_V2 {
                    return Err(invalid_data(
                        "queue plan journal global-admission identity version is unsupported",
                    ));
                }
                if identity.chain_id_digest == Hash::prehashed([0; Hash::LENGTH])
                    || identity.request_id == Hash::prehashed([0; Hash::LENGTH])
                {
                    return Err(invalid_data(
                        "queue plan journal global-admission identity contains a zero hash",
                    ));
                }
            }
        }
        QueuePlanJournalFrameV4::Remove {
            entrypoint_hash,
            plan_digest,
            claim_digest,
        } => {
            if entrypoint_hash.as_ref().iter().all(|byte| *byte == 0)
                || *plan_digest == Hash::prehashed([0; Hash::LENGTH])
                || *claim_digest == Hash::prehashed([0; Hash::LENGTH])
            {
                return Err(invalid_data(
                    "queue plan journal Remove contains a zero identity",
                ));
            }
        }
        QueuePlanJournalFrameV4::RemoveBatch(removals) => {
            if removals.is_empty() {
                return Err(invalid_data(
                    "queue plan journal RemoveBatch must not be empty",
                ));
            }
            let mut entrypoints = BTreeSet::new();
            for removal in removals {
                if removal
                    .entrypoint_hash
                    .as_ref()
                    .iter()
                    .all(|byte| *byte == 0)
                    || removal.plan_digest == Hash::prehashed([0; Hash::LENGTH])
                    || removal.claim_digest == Hash::prehashed([0; Hash::LENGTH])
                {
                    return Err(invalid_data(
                        "queue plan journal RemoveBatch contains a zero identity",
                    ));
                }
                if !entrypoints.insert(removal.entrypoint_hash.clone()) {
                    return Err(invalid_data(
                        "queue plan journal RemoveBatch contains a duplicate entrypoint",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn decode_frame(
    payload: &[u8],
    limits: QueuePlanJournalLimits,
) -> io::Result<QueuePlanJournalFrameV4> {
    let configured_payload_limit =
        usize::try_from(limits.max_frame_payload_bytes).unwrap_or(usize::MAX);
    if payload.is_empty() || payload.len() > configured_payload_limit {
        return Err(invalid_data(
            "queue plan journal payload exceeds the configured frame limit",
        ));
    }
    // Journal writers emit only the canonical uncompressed archive.
    // Reject compressed envelopes before the owned decoder can decompress an
    // attacker-controlled archive whose wire size is much smaller than its
    // declared payload.
    let advertised_flags = payload
        .get(norito::core::Header::SIZE - 1)
        .copied()
        .unwrap_or_else(norito::core::default_encode_flags);
    let preflight = {
        let _payload_context = norito::core::PayloadCtxGuard::enter(payload);
        // Preflight the archive under its advertised layout so ambient caller
        // state cannot cause a false mismatch. The canonical decoder below
        // independently requires the one fixed durable layout.
        let _advertised_flags = norito::core::DecodeFlagsGuard::enter(advertised_flags);
        norito::core::from_bytes_view(payload).map(|_| ())
    };
    preflight.map_err(|error| {
        invalid_data(format!(
            "queue plan journal payload is not a canonical uncompressed archive: {error}"
        ))
    })?;
    let payload_budget = payload.len();
    // Norito retains an aligned archive copy while constructing the owned
    // entrypoint and routing plan. Bound that deterministic wire-to-owned
    // amplification separately from element counts. Small transactions have
    // a comparatively large fixed object-graph cost, so reserve 64 KiB for
    // decoder-owned archive and container metadata instead of leaving small
    // frames unaccounted. Allocation bombs remain capped by a fixed allowance
    // plus the calibrated multiple of the already bounded frame length.
    let aggregate_element_budget =
        payload_budget.saturating_mul(FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT);
    let aggregate_allocation_budget = frame_decode_allocation_budget(payload_budget)
        .ok_or_else(|| invalid_data("queue plan journal payload allocation budget overflow"))?;
    let decode_limits = norito::DecodeLimits::new(
        payload_budget,
        payload_budget,
        aggregate_element_budget,
        aggregate_allocation_budget,
        128,
    );
    let frame =
        norito::decode_canonical_with_limits::<QueuePlanJournalFrameV4>(payload, decode_limits)
            .map_err(|error| {
                if matches!(&error, norito::Error::NonCanonicalEncoding) {
                    invalid_data("queue plan journal payload is not canonically encoded")
                } else {
                    invalid_data(format!(
                        "queue plan journal payload cannot be decoded: {error}"
                    ))
                }
            })?;
    validate_frame(&frame)?;
    Ok(frame)
}

fn frame_decode_allocation_budget(payload_bytes: usize) -> Option<usize> {
    payload_bytes
        .checked_mul(FRAME_DECODE_ALLOCATION_AMPLIFICATION_LIMIT)?
        .checked_add(FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES)
}

fn repair_incomplete_tail(path: &Path, limits: QueuePlanJournalLimits) -> io::Result<()> {
    let mut file = open_regular_read_write(path)?;
    let file_identity = verify_open_regular_path(path, &file)?;
    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
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
    let repaired_len = file.metadata()?.len();
    if repaired_len > file_len {
        return Err(invalid_data(
            "queue plan journal length increased concurrently during startup repair",
        ));
    }
    if verify_open_regular_path(path, &file)? != file_identity
        || verify_open_regular_parent(path, &parent)? != parent_identity
        || file.metadata()?.len() != repaired_len
    {
        return Err(invalid_data(
            "queue plan journal identity or length changed while validating startup repair",
        ));
    }
    // Retry both durability boundaries unconditionally. A complete frame observed after an
    // indeterminate pre-sync append must itself become durable before startup publishes it; syncing
    // only the parent can otherwise replay page-cache bytes that a second crash subsequently loses.
    file.sync_all()?;
    if verify_open_regular_path(path, &file)? != file_identity
        || verify_open_regular_parent(path, &parent)? != parent_identity
        || file.metadata()?.len() != repaired_len
    {
        return Err(invalid_data(
            "queue plan journal identity or length changed while synchronizing adopted startup bytes",
        ));
    }
    parent.sync_all()?;
    if verify_open_regular_path(path, &file)? != file_identity
        || verify_open_regular_parent(path, &parent)? != parent_identity
        || file.metadata()?.len() != repaired_len
    {
        return Err(invalid_data(
            "queue plan journal identity or length changed while completing startup repair",
        ));
    }
    Ok(())
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
    F: FnMut(u64, QueuePlanJournalFrameV4) -> io::Result<()>,
{
    ensure_file_bound(scan_len, limits)?;
    let actual_len = file.metadata()?.len();
    if scan_len > actual_len {
        return Err(invalid_data(
            "queue plan journal replay snapshot exceeds the opened file",
        ));
    }
    if scan_len == 0 {
        return Err(invalid_data(
            "queue plan journal is missing its durable V4 bootstrap frame",
        ));
    }

    let mut position = 0_u64;
    let mut saw_bootstrap = false;
    while position < scan_len {
        let Some((frame_end, frame)) =
            read_frame_at_position(file, position, scan_len, limits, mode, repair_path)?
        else {
            return Ok(());
        };
        match &frame {
            QueuePlanJournalFrameV4::Bootstrap { .. } if position == 0 && !saw_bootstrap => {
                saw_bootstrap = true;
            }
            QueuePlanJournalFrameV4::Bootstrap { .. } => {
                return Err(invalid_data(
                    "queue plan journal bootstrap frame must appear exactly once at offset zero",
                ));
            }
            _ if !saw_bootstrap => {
                return Err(invalid_data(
                    "queue plan journal operation appears before its durable V4 bootstrap frame",
                ));
            }
            _ => {}
        }
        handle(position, frame)?;
        position = frame_end;
    }
    if !saw_bootstrap {
        return Err(invalid_data(
            "queue plan journal is missing its durable V4 bootstrap frame",
        ));
    }
    Ok(())
}

/// Read and validate one exact frame from an already length- and identity-bound snapshot.
///
/// `scan_file` validates the snapshot once before iterating. Keeping all envelope parsing here
/// gives startup repair, strict replay indexing, and adversarial frame tests one canonical decoder.
fn read_frame_at_position(
    file: &mut File,
    position: u64,
    scan_len: u64,
    limits: QueuePlanJournalLimits,
    mode: ScanMode,
    repair_path: Option<&Path>,
) -> io::Result<Option<(u64, QueuePlanJournalFrameV4)>> {
    ensure_file_bound(scan_len, limits)?;
    if position >= scan_len {
        return Err(invalid_data(
            "queue plan journal frame offset is outside the replay snapshot",
        ));
    }
    file.seek(SeekFrom::Start(position))?;
    let remaining = scan_len
        .checked_sub(position)
        .ok_or_else(|| invalid_data("queue plan journal scan position underflow"))?;
    // Phase one writes at most the fixed header, and phase two cannot start until that exact
    // header has survived `sync_all`. Therefore a short terminal suffix is an interrupted
    // phase-one write. A complete, structurally recognizable header must still have its declared
    // bound validated before repair; otherwise an attacker can disguise an oversized frame as a
    // crash tear. This deliberately does not apply at offset zero: a partial bootstrap cannot
    // establish that the file is a V4 journal.
    if mode == ScanMode::RepairTerminalTear && position != 0 && remaining <= FRAME_HEADER_BYTES {
        if remaining == FRAME_HEADER_BYTES {
            let header_len = usize::try_from(FRAME_HEADER_BYTES)
                .map_err(|_| invalid_data("queue plan journal header length exceeds usize"))?;
            let mut header = vec![0_u8; header_len];
            file.read_exact(&mut header)?;
            let version_offset = QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len();
            let length_offset = version_offset + 2;
            let guard_offset = length_offset + 4;
            let magic_matches =
                header[..QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len()] == QUEUE_PLAN_JOURNAL_FRAME_MAGIC;
            let version = u16::from_le_bytes(
                header[version_offset..length_offset]
                    .try_into()
                    .expect("fixed queue journal version field"),
            );
            let declared = u32::from_le_bytes(
                header[length_offset..guard_offset]
                    .try_into()
                    .expect("fixed queue journal length field"),
            );
            let guard = u32::from_le_bytes(
                header[guard_offset..guard_offset + 4]
                    .try_into()
                    .expect("fixed queue journal length guard"),
            );
            if magic_matches
                && version == QUEUE_PLAN_JOURNAL_FRAME_FORMAT_VERSION
                && guard == !declared
                && (declared == 0 || u64::from(declared) > limits.max_frame_payload_bytes)
            {
                return Err(invalid_data(
                    "queue plan journal frame exceeds the configured payload limit",
                ));
            }
        }
        let path = repair_path
            .ok_or_else(|| invalid_data("queue plan journal repair path is unavailable"))?;
        truncate_journal_tail(file, position, path)?;
        return Ok(None);
    }
    if remaining < FRAME_HEADER_BYTES {
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
        if mode == ScanMode::RepairTerminalTear && position != 0 {
            let path = repair_path
                .ok_or_else(|| invalid_data("queue plan journal repair path is unavailable"))?;
            truncate_journal_tail(file, position, path)?;
            return Ok(None);
        }
        if position == 0 {
            return Err(invalid_data(
                "queue plan journal has an incomplete bootstrap frame",
            ));
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
        if mode == ScanMode::RepairTerminalTear && position != 0 && frame_end == scan_len {
            let path = repair_path
                .ok_or_else(|| invalid_data("queue plan journal repair path is unavailable"))?;
            truncate_journal_tail(file, position, path)?;
            return Ok(None);
        }
        return Err(invalid_data(
            "queue plan journal frame commit marker mismatch",
        ));
    }
    if frame_checksum(&version_bytes, &len_bytes, &len_guard, &payload).as_ref() != &checksum {
        return Err(invalid_data("queue plan journal frame checksum mismatch"));
    }
    let frame = decode_frame(&payload, limits)?;
    Ok(Some((frame_end, frame)))
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
    let identity = verify_open_regular_path(path, file)?;
    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
    file.set_len(valid_end)?;
    file.sync_all()?;
    if verify_open_regular_path(path, file)? != identity
        || verify_open_regular_parent(path, &parent)? != parent_identity
        || file.metadata()?.len() != valid_end
    {
        return Err(invalid_data(
            "queue plan journal file, parent identity, or length changed while truncating a torn tail",
        ));
    }
    sync_parent(path)?;
    if verify_open_regular_path(path, file)? != identity
        || verify_open_regular_parent(path, &parent)? != parent_identity
        || file.metadata()?.len() != valid_end
    {
        return Err(invalid_data(
            "queue plan journal file, parent identity, or length changed across torn-tail parent synchronization",
        ));
    }
    Ok(())
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

fn journal_snapshot_digest(file: &mut File, snapshot_len: u64) -> io::Result<Hash> {
    file.seek(SeekFrom::Start(0))?;
    let snapshot_len_bytes = snapshot_len.to_le_bytes();
    let mut digest =
        Hash::new_from_chunks(&[QUEUE_PLAN_JOURNAL_SNAPSHOT_DOMAIN, &snapshot_len_bytes]);
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 8 * 1024];
    while offset < snapshot_len {
        let remaining = snapshot_len
            .checked_sub(offset)
            .ok_or_else(|| invalid_data("queue plan journal snapshot offset underflow"))?;
        let take = usize::try_from(remaining.min(8 * 1024))
            .map_err(|_| invalid_data("queue plan journal snapshot chunk exceeds usize"))?;
        file.read_exact(&mut buffer[..take])?;
        let offset_bytes = offset.to_le_bytes();
        digest = Hash::new_from_chunks(&[
            QUEUE_PLAN_JOURNAL_SNAPSHOT_DOMAIN,
            digest.as_ref(),
            &offset_bytes,
            &buffer[..take],
        ]);
        offset = offset
            .checked_add(
                u64::try_from(take)
                    .map_err(|_| invalid_data("queue plan journal snapshot chunk exceeds u64"))?,
            )
            .ok_or_else(|| invalid_data("queue plan journal snapshot offset overflow"))?;
    }
    Ok(digest)
}

#[cfg(test)]
fn read_frames(
    path: &Path,
    limits: QueuePlanJournalLimits,
) -> io::Result<Vec<QueuePlanJournalFrameV4>> {
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
            if !matches!(&frame, QueuePlanJournalFrameV4::Bootstrap { .. }) {
                frames.push(frame);
            }
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

#[cfg(target_os = "macos")]
fn normalize_platform_managed_alias(path: &Path) -> io::Result<PathBuf> {
    const MANAGED_ALIASES: [(&str, &str); 2] = [("/var", "/private/var"), ("/tmp", "/private/tmp")];

    for (alias, destination) in MANAGED_ALIASES {
        let alias = Path::new(alias);
        let Ok(relative) = path.strip_prefix(alias) else {
            continue;
        };
        if relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(invalid_input(
                "queue plan journal path below a platform-managed alias cannot contain `..`",
            ));
        }
        let alias_metadata = fs::symlink_metadata(alias)?;
        if !alias_metadata.file_type().is_symlink() {
            return Ok(path.to_path_buf());
        }
        let destination = Path::new(destination);
        if fs::canonicalize(alias)? != destination {
            return Err(invalid_data(format!(
                "queue plan journal platform-managed alias {} does not resolve to {}",
                alias.display(),
                destination.display()
            )));
        }
        let destination_metadata = fs::symlink_metadata(destination)?;
        if journal_file_is_indirect(&destination_metadata) || !destination_metadata.is_dir() {
            return Err(invalid_data(format!(
                "queue plan journal platform-managed alias destination {} must be a direct directory",
                destination.display()
            )));
        }

        // Resolve only the fixed, root-owned macOS alias. Every caller-controlled component in
        // `relative` remains lexical and is subsequently checked by
        // `prepare_regular_journal_parent`, so a symlink below `/var` or `/tmp` still fails closed.
        return Ok(destination.join(relative));
    }
    Ok(path.to_path_buf())
}

#[cfg(not(target_os = "macos"))]
fn normalize_platform_managed_alias(path: &Path) -> io::Result<PathBuf> {
    Ok(path.to_path_buf())
}

fn canonical_journal_path(path: &Path) -> io::Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| invalid_input("queue plan journal path must end in a regular file name"))?;
    let parent = fs::canonicalize(parent_directory(path))?;
    Ok(parent.join(file_name))
}

#[cfg(unix)]
type JournalFileIdentity = (u64, u64);
#[cfg(windows)]
type JournalFileIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type JournalFileIdentity = ();

#[cfg(unix)]
fn journal_file_identity(metadata: &fs::Metadata) -> JournalFileIdentity {
    use std::os::unix::fs::MetadataExt as _;

    (metadata.dev(), metadata.ino())
}

#[cfg(windows)]
fn journal_file_identity(metadata: &fs::Metadata) -> JournalFileIdentity {
    use std::os::windows::fs::MetadataExt as _;

    (metadata.volume_serial_number(), metadata.file_index())
}

#[cfg(not(any(unix, windows)))]
fn journal_file_identity(_metadata: &fs::Metadata) -> JournalFileIdentity {}

#[cfg(unix)]
const fn journal_file_identity_available(_identity: JournalFileIdentity) -> bool {
    true
}

#[cfg(windows)]
const fn journal_file_identity_available(identity: JournalFileIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}

#[cfg(not(any(unix, windows)))]
const fn journal_file_identity_available(_identity: JournalFileIdentity) -> bool {
    false
}

fn journal_file_is_single_link(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}

#[cfg(windows)]
fn journal_file_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn journal_file_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn journal_file_is_indirect(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || journal_file_is_reparse_point(metadata)
}

fn verify_open_regular_directory(
    directory_path: &Path,
    directory: &File,
) -> io::Result<JournalFileIdentity> {
    let path_metadata = fs::symlink_metadata(directory_path)?;
    let opened = directory.metadata()?;
    let path_identity = journal_file_identity(&path_metadata);
    let opened_identity = journal_file_identity(&opened);
    if journal_file_is_indirect(&path_metadata)
        || journal_file_is_indirect(&opened)
        || !path_metadata.is_dir()
        || !opened.is_dir()
        || !journal_file_identity_available(path_identity)
        || !journal_file_identity_available(opened_identity)
    {
        return Err(invalid_data(
            "queue plan journal parent must be a direct directory with a stable filesystem identity",
        ));
    }
    if opened_identity != path_identity {
        return Err(invalid_data(
            "queue plan journal parent path changed while its directory handle was open",
        ));
    }
    Ok(opened_identity)
}

#[cfg(any(unix, windows))]
fn open_regular_directory(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        options.custom_flags(
            (rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::NOFOLLOW).bits() as i32,
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        // `FlushFileBuffers`, which backs `File::sync_all`, requires a write-capable handle.
        options.write(true);
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    }
    let directory = options.open(path)?;
    verify_open_regular_directory(path, &directory)?;
    Ok(directory)
}

#[cfg(not(any(unix, windows)))]
fn open_regular_directory(_path: &Path) -> io::Result<File> {
    Err(invalid_data(
        "queue plan journal directory identity is unsupported on this platform",
    ))
}

fn verify_open_regular_parent(path: &Path, parent: &File) -> io::Result<JournalFileIdentity> {
    verify_open_regular_directory(parent_directory(path), parent)
}

fn open_regular_parent(path: &Path) -> io::Result<File> {
    open_regular_directory(parent_directory(path))
}

fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = open_regular_parent(path)?;
    let identity = verify_open_regular_parent(path, &parent)?;
    parent.sync_all()?;
    if verify_open_regular_parent(path, &parent)? != identity {
        return Err(invalid_data(
            "queue plan journal parent identity changed while synchronizing",
        ));
    }
    Ok(())
}

fn persist_atomic_replacement(temporary: &Path, destination: &Path) -> io::Result<()> {
    // `std::fs::rename` atomically replaces an existing destination on Unix but rejects it on
    // Windows. `TempPath::persist` uses the native replace operation on both platforms. Cleanup is
    // deliberately disabled: after any failed promotion the recognizable `.tmp` artifact must
    // remain for fail-closed startup reconciliation instead of being silently discarded on drop.
    let mut temporary = tempfile::TempPath::try_from_path(temporary)?;
    temporary.disable_cleanup(true);
    temporary.persist(destination).map_err(|error| error.error)
}

fn ensure_durable_v4_bootstrap(path: &Path, limits: QueuePlanJournalLimits) -> io::Result<()> {
    let expected = encode_bootstrap_frame(limits)?;
    let canonical = open_regular_read(path)?;
    let canonical_identity = verify_open_regular_path(path, &canonical)?;
    let canonical_len = canonical.metadata()?.len();
    if canonical_len != 0 {
        reject_existing_bootstrap_temp(&path.with_extension("bootstrap.tmp"))?;
        return Ok(());
    }

    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
    let temporary_path = path.with_extension("bootstrap.tmp");
    let mut replacement = match open_recoverable_bootstrap_temp(&temporary_path, &expected)? {
        Some(replacement) => replacement,
        None => {
            let mut replacement = open_new_regular(&temporary_path)?;
            let replacement_identity = verify_open_regular_path(&temporary_path, &replacement)?;
            replacement.sync_all()?;
            parent.sync_all()?;
            if verify_open_regular_path(&temporary_path, &replacement)? != replacement_identity
                || replacement.metadata()?.len() != 0
                || verify_open_regular_parent(path, &parent)? != parent_identity
            {
                return Err(invalid_data(
                    "queue plan journal empty bootstrap replacement changed across creation synchronization",
                ));
            }
            write_staged_frame(&mut replacement, &expected)?;
            if verify_open_regular_path(&temporary_path, &replacement)? != replacement_identity
                || replacement.metadata()?.len()
                    != u64::try_from(expected.len())
                        .map_err(|_| invalid_data("queue plan bootstrap exceeds u64"))?
            {
                return Err(invalid_data(
                    "queue plan journal bootstrap replacement changed while being written",
                ));
            }
            replacement
        }
    };
    let replacement_identity = verify_open_regular_path(&temporary_path, &replacement)?;
    if replacement.metadata()?.len()
        != u64::try_from(expected.len())
            .map_err(|_| invalid_data("queue plan bootstrap exceeds u64"))?
    {
        return Err(invalid_data(
            "queue plan journal bootstrap replacement has the wrong length",
        ));
    }
    replacement.seek(SeekFrom::Start(0))?;
    let mut actual = Vec::with_capacity(expected.len());
    replacement.read_to_end(&mut actual)?;
    if actual != expected {
        return Err(invalid_data(
            "queue plan journal bootstrap replacement bytes are not canonical V4",
        ));
    }
    if verify_open_regular_path(path, &canonical)? != canonical_identity
        || canonical.metadata()?.len() != 0
        || verify_open_regular_parent(path, &parent)? != parent_identity
    {
        return Err(invalid_data(
            "queue plan journal canonical file or parent changed before bootstrap publication",
        ));
    }
    persist_atomic_replacement(&temporary_path, path)?;
    if verify_open_regular_path(path, &replacement)? != replacement_identity
        || replacement.metadata()?.len()
            != u64::try_from(expected.len())
                .map_err(|_| invalid_data("queue plan bootstrap exceeds u64"))?
        || verify_open_regular_parent(path, &parent)? != parent_identity
    {
        return Err(invalid_data(
            "queue plan journal bootstrap publication changed file or parent identity",
        ));
    }
    parent.sync_all()?;
    if verify_open_regular_path(path, &replacement)? != replacement_identity
        || replacement.metadata()?.len()
            != u64::try_from(expected.len())
                .map_err(|_| invalid_data("queue plan bootstrap exceeds u64"))?
        || verify_open_regular_parent(path, &parent)? != parent_identity
    {
        return Err(invalid_data(
            "queue plan journal bootstrap publication changed across parent synchronization",
        ));
    }
    Ok(())
}

fn open_recoverable_bootstrap_temp(path: &Path, expected: &[u8]) -> io::Result<Option<File>> {
    let metadata = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Ok(metadata) => metadata,
        Err(error) => return Err(error),
    };
    if journal_file_is_indirect(&metadata)
        || !metadata.is_file()
        || !journal_file_is_single_link(&metadata)
        || usize::try_from(metadata.len()).unwrap_or(usize::MAX) > expected.len()
    {
        return Err(invalid_data(
            "queue plan journal bootstrap temp must be a bounded single-link regular file",
        ));
    }
    let mut file = open_regular_read_write(path)?;
    let identity = verify_open_regular_path(path, &file)?;
    let (header_end, commit_start) = staged_frame_boundaries(expected)?;
    let mut bytes = Vec::with_capacity(expected.len());
    file.read_to_end(&mut bytes)?;
    if verify_open_regular_path(path, &file)? != identity
        || file.metadata()?.len() != metadata.len()
    {
        return Err(invalid_data(
            "queue plan journal bootstrap temp changed while being recovered",
        ));
    }
    if bytes == expected {
        file.sync_all()?;
        if verify_open_regular_path(path, &file)? != identity
            || file.metadata()?.len()
                != u64::try_from(expected.len())
                    .map_err(|_| invalid_data("queue plan bootstrap exceeds u64"))?
        {
            return Err(invalid_data(
                "queue plan journal complete bootstrap temp changed while synchronizing",
            ));
        }
        file.seek(SeekFrom::Start(0))?;
        return Ok(Some(file));
    }
    // Phase two cannot begin until the complete header is durable. An arbitrary terminal suffix
    // no longer than that header is therefore a recoverable phase-one tear; longer artifacts must
    // carry the exact staged header before their body can be classified as uncommitted residue.
    if bytes.len() > header_end && bytes[..header_end] != expected[..header_end] {
        return Err(invalid_data(
            "queue plan journal bootstrap temp does not have a recognizable staged V4 header",
        ));
    }
    let commit_is_valid = bytes.len() == expected.len()
        && bytes
            .get(commit_start..)
            .is_some_and(|commit| commit == QUEUE_PLAN_JOURNAL_FRAME_COMMIT);
    if commit_is_valid && bytes[..commit_start] != expected[..commit_start] {
        return Err(invalid_data(
            "queue plan journal bootstrap temp has a committed noncanonical staged body",
        ));
    }
    // A staged bootstrap crash can leave an exact header prefix, a full durable header followed
    // by an arbitrary full-length torn body, or a durable body followed by a torn commit marker.
    // The bootstrap payload is static and contains no queue ownership, so rebuild the recognized
    // artifact in place through the same three durability phases. Keeping the verified handle
    // avoids a close/reopen identity gap.
    file.set_len(0)?;
    file.sync_all()?;
    file.seek(SeekFrom::Start(0))?;
    write_staged_frame(&mut file, expected)?;
    if verify_open_regular_path(path, &file)? != identity
        || file.metadata()?.len()
            != u64::try_from(expected.len())
                .map_err(|_| invalid_data("queue plan bootstrap exceeds u64"))?
    {
        return Err(invalid_data(
            "queue plan journal bootstrap temp changed while extending its canonical prefix",
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut completed = Vec::with_capacity(expected.len());
    file.read_to_end(&mut completed)?;
    if completed != expected || verify_open_regular_path(path, &file)? != identity {
        return Err(invalid_data(
            "queue plan journal bootstrap temp extension is not canonical V4",
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(Some(file))
}

fn reject_existing_bootstrap_temp(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "queue plan journal bootstrap temp conflicts with an initialized journal",
        )),
        Err(error) => Err(error),
    }
}

fn open_pending_compaction_temp(
    path: &Path,
    limits: QueuePlanJournalLimits,
) -> io::Result<Option<PendingCompactionTemp>> {
    let metadata = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Ok(metadata) => metadata,
        Err(error) => return Err(error),
    };
    if journal_file_is_indirect(&metadata)
        || !metadata.is_file()
        || !journal_file_is_single_link(&metadata)
    {
        return Err(invalid_data(
            "queue plan journal compaction temp must be a direct single-link regular file",
        ));
    }
    ensure_file_bound(metadata.len(), limits).map_err(|_| {
        invalid_data("queue plan journal compaction temp exceeds the configured file limit")
    })?;
    let file = open_regular_read(path)?;
    let file_identity = verify_open_regular_path(path, &file)?;
    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
    let pending = PendingCompactionTemp {
        path: path.to_path_buf(),
        file,
        file_identity,
        snapshot_len: metadata.len(),
        parent,
        parent_identity,
    };
    pending.verify()?;
    Ok(Some(pending))
}

fn reconcile_pending_compaction_temp(
    canonical_path: &Path,
    limits: QueuePlanJournalLimits,
    mut pending: PendingCompactionTemp,
) -> io::Result<()> {
    pending.verify()?;
    let mut canonical = open_regular_read(canonical_path)?;
    let canonical_identity = verify_open_regular_path(canonical_path, &canonical)?;
    let canonical_len = canonical.metadata()?.len();
    ensure_file_bound(canonical_len, limits)?;
    let canonical_parent = open_regular_parent(canonical_path)?;
    let canonical_parent_identity = verify_open_regular_parent(canonical_path, &canonical_parent)?;
    if canonical_parent_identity != pending.parent_identity {
        return Err(invalid_data(
            "queue plan journal canonical and compaction temp do not share one stable parent",
        ));
    }
    let canonical_digest = journal_snapshot_digest(&mut canonical, canonical_len)?;
    verify_bound_compaction_canonical(
        canonical_path,
        &canonical,
        canonical_identity,
        canonical_len,
        &canonical_parent,
        canonical_parent_identity,
    )?;

    let mut live_positions = BTreeMap::<QueuePlanJournalKey, QueuePlanJournalLivePosition>::new();
    scan_file(
        &mut canonical,
        canonical_len,
        limits,
        ScanMode::Strict,
        None,
        |position, frame| {
            match frame {
                QueuePlanJournalFrameV4::Bootstrap { .. } => {}
                QueuePlanJournalFrameV4::Put(record) => {
                    let key = record.entrypoint_hash;
                    if !live_positions.contains_key(&key)
                        && live_positions.len() >= limits.max_live_records
                    {
                        return Err(invalid_data(
                            "queue plan journal compaction recovery live-record limit exceeded",
                        ));
                    }
                    let plan_digest = record.plan_digest();
                    let claim_digest = record.claim_digest().map_err(|error| {
                        invalid_data(format!(
                            "queue plan journal compaction recovery claim cannot be encoded: {error}"
                        ))
                    })?;
                    match live_positions.entry(key) {
                        std::collections::btree_map::Entry::Occupied(mut entry) => {
                            let live = entry.get_mut();
                            live.plan_digest = plan_digest;
                            live.claim_digest = claim_digest;
                            live.record = record;
                        }
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(QueuePlanJournalLivePosition {
                                plan_digest,
                                claim_digest,
                                ownership_position: position,
                                record,
                            });
                        }
                    }
                }
                QueuePlanJournalFrameV4::Remove {
                    entrypoint_hash,
                    plan_digest,
                    claim_digest,
                } => {
                    if live_positions.get(&entrypoint_hash).is_some_and(|live| {
                        live.plan_digest == plan_digest && live.claim_digest == claim_digest
                    }) {
                        live_positions.remove(&entrypoint_hash);
                    }
                }
                QueuePlanJournalFrameV4::RemoveBatch(removals) => {
                    if removals.iter().any(|removal| {
                        !live_positions
                            .get(&removal.entrypoint_hash)
                            .is_some_and(|live| {
                                live.plan_digest == removal.plan_digest
                                    && live.claim_digest == removal.claim_digest
                            })
                    }) {
                        return Err(invalid_data(
                            "queue plan journal compaction recovery RemoveBatch does not match its complete live prefix",
                        ));
                    }
                    for removal in removals {
                        live_positions.remove(&removal.entrypoint_hash);
                    }
                }
            }
            Ok(())
        },
    )?;
    verify_bound_compaction_canonical(
        canonical_path,
        &canonical,
        canonical_identity,
        canonical_len,
        &canonical_parent,
        canonical_parent_identity,
    )?;
    if journal_snapshot_digest(&mut canonical, canonical_len)? != canonical_digest {
        return Err(invalid_data(
            "queue plan journal canonical content changed while indexing compaction recovery",
        ));
    }

    let mut ordered = live_positions.into_iter().collect::<Vec<_>>();
    ordered.sort_unstable_by_key(|(_key, live)| live.ownership_position);
    pending.file.seek(SeekFrom::Start(0))?;
    let mut expected_len = 0_u64;
    let mut terminal_incomplete = compare_compaction_prefix_chunk(
        &mut pending.file,
        pending.snapshot_len,
        &mut expected_len,
        &encode_bootstrap_frame(limits)?,
    )?;
    for (entrypoint_hash, live) in ordered {
        pending.verify()?;
        let record = live.record;
        let claim_digest = record.claim_digest().map_err(|error| {
            invalid_data(format!(
                "queue plan journal compaction recovery materialized claim cannot be encoded: {error}"
            ))
        })?;
        if record.entrypoint_hash != entrypoint_hash
            || record.plan_digest() != live.plan_digest
            || claim_digest != live.claim_digest
        {
            return Err(invalid_data(
                "queue plan journal compaction recovery materialized live claim changed",
            ));
        }
        terminal_incomplete |= compare_compaction_prefix_chunk(
            &mut pending.file,
            pending.snapshot_len,
            &mut expected_len,
            &encode_frame(&QueuePlanJournalFrameV4::Put(record), limits)?,
        )?;
    }
    if pending.snapshot_len > expected_len {
        return Err(invalid_data(
            "queue plan journal compaction temp is longer than the deterministic compacted journal",
        ));
    }
    pending.verify()?;
    verify_bound_compaction_canonical(
        canonical_path,
        &canonical,
        canonical_identity,
        canonical_len,
        &canonical_parent,
        canonical_parent_identity,
    )?;
    if journal_snapshot_digest(&mut canonical, canonical_len)? != canonical_digest {
        return Err(invalid_data(
            "queue plan journal canonical content changed while reconciling compaction recovery",
        ));
    }
    if pending.snapshot_len == expected_len && !terminal_incomplete {
        pending.file.seek(SeekFrom::Start(0))?;
        scan_file(
            &mut pending.file,
            pending.snapshot_len,
            limits,
            ScanMode::Strict,
            None,
            |_position, _frame| Ok(()),
        )?;
        pending.verify()?;
    }

    // The canonical journal was independently repaired, strictly decoded, and bound to one
    // inode. An unpromoted compaction artifact is therefore non-authoritative. Keep its verified
    // handle open across unlink, bracket the pathname operation with identity checks, and durably
    // publish that deletion. Rust's portable filesystem API has no inode-conditional unlink, so
    // this still relies on the journal directory not being concurrently writable by an untrusted
    // process; the checks detect cooperative/concurrent replacement but cannot make a hostile
    // pathname race atomic. If a crash occurs before the directory sync, reconciliation is safe
    // to repeat.
    pending.verify()?;
    fs::remove_file(&pending.path)?;
    let removed_metadata = pending.file.metadata()?;
    if journal_file_identity(&removed_metadata) != pending.file_identity
        || removed_metadata.len() != pending.snapshot_len
    {
        return Err(invalid_data(
            "queue plan journal compaction temp handle changed while removing recovered artifact",
        ));
    }
    match fs::symlink_metadata(&pending.path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(invalid_data(
                "queue plan journal compaction temp pathname was replaced during recovery",
            ));
        }
        Err(error) => return Err(error),
    }
    if verify_open_regular_parent(&pending.path, &pending.parent)? != pending.parent_identity {
        return Err(invalid_data(
            "queue plan journal compaction temp parent changed while removing recovered artifact",
        ));
    }
    pending.parent.sync_all()?;
    match fs::symlink_metadata(&pending.path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(invalid_data(
                "queue plan journal compaction temp pathname reappeared during recovery",
            ));
        }
        Err(error) => return Err(error),
    }
    if verify_open_regular_parent(&pending.path, &pending.parent)? != pending.parent_identity {
        return Err(invalid_data(
            "queue plan journal compaction temp parent changed across recovery synchronization",
        ));
    }
    verify_bound_compaction_canonical(
        canonical_path,
        &canonical,
        canonical_identity,
        canonical_len,
        &canonical_parent,
        canonical_parent_identity,
    )
}

fn verify_bound_compaction_canonical(
    path: &Path,
    file: &File,
    file_identity: JournalFileIdentity,
    snapshot_len: u64,
    parent: &File,
    parent_identity: JournalFileIdentity,
) -> io::Result<()> {
    if verify_open_regular_parent(path, parent)? != parent_identity
        || verify_open_regular_path(path, file)? != file_identity
    {
        return Err(invalid_data(
            "queue plan journal canonical identity changed during compaction recovery",
        ));
    }
    let metadata = file.metadata()?;
    if journal_file_identity(&metadata) != file_identity
        || !journal_file_is_single_link(&metadata)
        || metadata.len() != snapshot_len
    {
        return Err(invalid_data(
            "queue plan journal canonical identity, link count, or length changed during compaction recovery",
        ));
    }
    if verify_open_regular_parent(path, parent)? != parent_identity {
        return Err(invalid_data(
            "queue plan journal canonical parent changed while validating compaction recovery",
        ));
    }
    Ok(())
}

fn compare_compaction_prefix_chunk(
    temporary: &mut File,
    temporary_len: u64,
    expected_offset: &mut u64,
    expected: &[u8],
) -> io::Result<bool> {
    let (header_end, commit_start) = staged_frame_boundaries(expected)?;
    let expected_bytes = u64::try_from(expected.len())
        .map_err(|_| invalid_data("queue plan journal compacted frame exceeds u64"))?;
    let chunk_start = *expected_offset;
    let chunk_end = chunk_start
        .checked_add(expected_bytes)
        .ok_or_else(|| invalid_data("queue plan journal deterministic compaction size overflow"))?;
    if chunk_start < temporary_len {
        let compared_bytes = temporary_len
            .min(chunk_end)
            .checked_sub(chunk_start)
            .ok_or_else(|| invalid_data("queue plan journal compaction prefix underflow"))?;
        let compared_bytes = usize::try_from(compared_bytes)
            .map_err(|_| invalid_data("queue plan journal compaction prefix exceeds usize"))?;
        if compared_bytes <= header_end && temporary_len <= chunk_end {
            // No body byte can be written before a complete exact header has been synchronized.
            // At this terminal bound, even a full-length garbage header is phase-one residue.
            *expected_offset = chunk_end;
            return Ok(true);
        }
        compare_file_bytes(
            temporary,
            chunk_start,
            &expected[..header_end],
            "queue plan journal compaction temp staged header is not deterministic",
        )?;

        let terminates_in_frame = temporary_len <= chunk_end;
        if terminates_in_frame {
            let commit_is_complete = compared_bytes == expected.len();
            let commit_is_valid = if commit_is_complete {
                let commit_offset = chunk_start
                    .checked_add(u64::try_from(commit_start).map_err(|_| {
                        invalid_data("queue plan journal compaction commit offset exceeds u64")
                    })?)
                    .ok_or_else(|| {
                        invalid_data("queue plan journal compaction commit offset overflow")
                    })?;
                let mut commit = [0_u8; QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len()];
                temporary.seek(SeekFrom::Start(commit_offset))?;
                temporary.read_exact(&mut commit)?;
                commit == QUEUE_PLAN_JOURNAL_FRAME_COMMIT
            } else {
                false
            };
            if !commit_is_valid {
                // The durable staged header proves this is the terminal frame currently being
                // written. Without a complete commit marker, arbitrary payload/checksum bytes are
                // uncommitted crash residue and the independently validated canonical journal
                // remains authoritative.
                *expected_offset = chunk_end;
                return Ok(true);
            }
        }
        compare_file_bytes(
            temporary,
            chunk_start,
            &expected[..compared_bytes],
            "queue plan journal committed compaction frame differs from deterministic output",
        )?;
    }
    *expected_offset = chunk_end;
    Ok(false)
}

fn compare_file_bytes(
    file: &mut File,
    offset: u64,
    expected: &[u8],
    mismatch: &'static str,
) -> io::Result<()> {
    file.seek(SeekFrom::Start(offset))?;
    let mut checked = 0_usize;
    let mut buffer = [0_u8; 8 * 1024];
    while checked < expected.len() {
        let take = (expected.len() - checked).min(buffer.len());
        file.read_exact(&mut buffer[..take])?;
        if buffer[..take] != expected[checked..checked + take] {
            return Err(invalid_data(mismatch));
        }
        checked += take;
    }
    Ok(())
}

fn prepare_regular_journal_path(path: &Path) -> io::Result<()> {
    prepare_regular_journal_parent(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if journal_file_is_indirect(&metadata) || !metadata.is_file() {
                return Err(invalid_data(
                    "queue plan journal path must be a direct regular file",
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let file = open_new_regular(path)?;
            file.sync_all()?;
        }
        Err(error) => return Err(error),
    }
    sync_parent_directory(path)
}

fn validate_directory_chain(path: &Path, require_complete: bool) -> io::Result<()> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        current.push(component.as_os_str());
        if matches!(component, std::path::Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if journal_file_is_indirect(&metadata) || !metadata.is_dir() {
                    return Err(invalid_data(format!(
                        "queue plan journal directory component {} must be a direct directory",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound && !require_complete => {
                return Ok(());
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn prepare_regular_journal_parent(path: &Path) -> io::Result<()> {
    let target = parent_directory(path);
    validate_directory_chain(target, false)?;

    let mut cursor = target.to_path_buf();
    let mut missing = Vec::new();
    let existing = loop {
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) => {
                if journal_file_is_indirect(&metadata) || !metadata.is_dir() {
                    return Err(invalid_data(
                        "queue plan journal parent chain contains a non-directory or indirect component",
                    ));
                }
                break cursor;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing.push(cursor.clone());
                cursor = parent_directory(&cursor).to_path_buf();
            }
            Err(error) => return Err(error),
        }
    };
    let existing_handle = open_regular_directory(&existing)?;
    let existing_identity = verify_open_regular_directory(&existing, &existing_handle)?;
    if verify_open_regular_directory(&existing, &existing_handle)? != existing_identity {
        return Err(invalid_data(
            "queue plan journal existing parent identity changed during preparation",
        ));
    }

    for directory in missing.into_iter().rev() {
        let owner_path = parent_directory(&directory);
        let owner = open_regular_directory(owner_path)?;
        let owner_identity = verify_open_regular_directory(owner_path, &owner)?;
        match fs::symlink_metadata(&directory) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(invalid_data(
                    "queue plan journal parent path appeared concurrently during creation",
                ));
            }
            Err(error) => return Err(error),
        }
        fs::create_dir(&directory)?;
        let child = open_regular_directory(&directory)?;
        let child_identity = verify_open_regular_directory(&directory, &child)?;
        child.sync_all()?;
        if verify_open_regular_directory(&directory, &child)? != child_identity
            || verify_open_regular_directory(owner_path, &owner)? != owner_identity
        {
            return Err(invalid_data(
                "queue plan journal nested parent identity changed while synchronizing the new directory",
            ));
        }
        owner.sync_all()?;
        if verify_open_regular_directory(&directory, &child)? != child_identity
            || verify_open_regular_directory(owner_path, &owner)? != owner_identity
        {
            return Err(invalid_data(
                "queue plan journal nested parent identity changed across parent synchronization",
            ));
        }
    }
    validate_directory_chain(target, true)?;
    if verify_open_regular_directory(&existing, &existing_handle)? != existing_identity {
        return Err(invalid_data(
            "queue plan journal existing parent identity changed across nested creation",
        ));
    }
    let target_handle = open_regular_directory(target)?;
    verify_open_regular_directory(target, &target_handle)?;
    Ok(())
}

fn reject_existing_compaction_temp(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(metadata) => {
            let kind = if journal_file_is_indirect(&metadata) {
                "symlink or reparse point"
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
    let identity = journal_file_identity(&metadata);
    if journal_file_is_indirect(&metadata)
        || !metadata.is_file()
        || !journal_file_identity_available(identity)
    {
        return Err(invalid_data(
            "queue plan journal path must be a direct regular file with a stable filesystem identity",
        ));
    }
    if !journal_file_is_single_link(&metadata) {
        return Err(invalid_data(
            "queue plan journal must have exactly one filesystem link",
        ));
    }
    Ok(())
}

fn verify_open_regular_path(path: &Path, file: &File) -> io::Result<JournalFileIdentity> {
    let path_metadata = fs::symlink_metadata(path)?;
    let opened = file.metadata()?;
    let path_identity = journal_file_identity(&path_metadata);
    let opened_identity = journal_file_identity(&opened);
    if journal_file_is_indirect(&path_metadata)
        || journal_file_is_indirect(&opened)
        || !path_metadata.is_file()
        || !opened.is_file()
        || !journal_file_identity_available(path_identity)
        || !journal_file_identity_available(opened_identity)
    {
        return Err(invalid_data(
            "opened queue plan journal and its path must be direct regular files with stable filesystem identities",
        ));
    }
    if !journal_file_is_single_link(&path_metadata) || !journal_file_is_single_link(&opened) {
        return Err(invalid_data(
            "queue plan journal must have exactly one filesystem link",
        ));
    }
    if opened_identity != path_identity {
        return Err(invalid_data(
            "queue plan journal path changed while its file handle was open",
        ));
    }
    Ok(opened_identity)
}

fn configure_direct_regular_open(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
}

fn open_new_regular(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create_new(true).read(true).write(true);
    configure_direct_regular_open(&mut options);
    let file = options.open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn open_regular_append(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let mut options = OpenOptions::new();
    options.append(true).read(true);
    configure_direct_regular_open(&mut options);
    let file = options.open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn open_regular_read_write(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    configure_direct_regular_open(&mut options);
    let file = options.open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn open_regular_read(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    configure_direct_regular_open(&mut options);
    let file = options.open(path)?;
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

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_managed_alias_normalization_is_exact_and_cannot_escape() {
        assert_eq!(
            normalize_platform_managed_alias(Path::new("/var/folders/journal.norito"))
                .expect("normalize fixed /var alias"),
            Path::new("/private/var/folders/journal.norito")
        );
        assert_eq!(
            normalize_platform_managed_alias(Path::new("/tmp/journal.norito"))
                .expect("normalize fixed /tmp alias"),
            Path::new("/private/tmp/journal.norito")
        );
        assert_eq!(
            normalize_platform_managed_alias(Path::new("/variable/journal.norito"))
                .expect("a component prefix must not match a string prefix"),
            Path::new("/variable/journal.norito")
        );
        assert_eq!(
            normalize_platform_managed_alias(Path::new("/var/../tmp/journal.norito"))
                .expect_err("a lexical parent must not escape the fixed alias")
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }

    fn record(label: &str) -> QueuePlanJournalRecordV4 {
        record_with_message(label, label.to_owned())
    }

    fn record_with_message(label: &str, message: String) -> QueuePlanJournalRecordV4 {
        let instruction: InstructionBox = Log::new(Level::INFO, message).into();
        record_with_instructions(label, [instruction])
    }

    fn record_with_instructions(
        label: &str,
        instructions: impl IntoIterator<Item = InstructionBox>,
    ) -> QueuePlanJournalRecordV4 {
        let chain_id = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("chain id");
        let (account_id, keypair) = gen_account_in(label);
        let tx = TransactionBuilder::new(
            chain_id,
            account_id,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .sign(keypair.private_key());
        let accepted = crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(tx));
        let routing_plan = RoutingPlan::single(super::super::RoutingDecision::default());
        let validators = vec![iroha_data_model::peer::PeerId::new(
            keypair.public_key().clone(),
        )];
        let admission_context = QueuePlanAdmissionContextV2 {
            version: super::super::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V2,
            authority_height: 0,
            proposal_height: 1,
            predecessor_block_hash: None,
            routing_plan_digest: routing_plan.digest(),
            route_incarnations: vec![super::super::QueuePlanRouteIncarnationV2 {
                leg: routing_plan.coordinator_leg(),
                lane_incarnation: Hash::new(b"journal-test-incarnation"),
                validator_set_hash_version:
                    iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validators),
                validator_set: validators.clone(),
                validator_count: 1,
                durability_threshold: 1,
            }],
        };
        QueuePlanJournalRecordV4::new(
            accepted.entrypoint().clone(),
            routing_plan,
            admission_context,
            42,
            None,
        )
    }

    fn with_single_route(
        mut record: QueuePlanJournalRecordV4,
        lane_id: u32,
        dataspace_id: u32,
    ) -> QueuePlanJournalRecordV4 {
        let routing_plan = RoutingPlan::single(super::super::RoutingDecision::new(
            iroha_data_model::nexus::LaneId::new(lane_id),
            iroha_data_model::nexus::DataSpaceId::new(dataspace_id.into()),
        ));
        record.admission_context.routing_plan_digest = routing_plan.digest();
        record.admission_context.route_incarnations[0].leg = routing_plan.coordinator_leg();
        record.routing_plan = routing_plan;
        record
    }

    fn admission_binding_for_record(
        record: &QueuePlanJournalRecordV4,
    ) -> QueuePlanAdmissionBindingV2 {
        let durable_admission = super::super::QueuePlanDurableAdmissionV2 {
            version: super::super::QUEUE_PLAN_DURABLE_ADMISSION_VERSION_V2,
            context: record.admission_context.clone(),
            global_admission_identity: record.global_admission_identity.clone(),
            routing_plan: record.routing_plan.clone(),
            entrypoint_hash: record.entrypoint_hash.clone(),
            signed_transaction_hash: record.signed_transaction_hash.clone(),
            enqueue_timestamp_ms: record.enqueue_timestamp_ms,
            journal_record_digest: record.claim_digest().expect("hash global journal claim"),
        };
        let binding = QueuePlanAdmissionBindingV2::try_from_durable_admission(&durable_admission)
            .expect("reconstruct global admission binding");
        binding
            .validate_for_transaction_and_plan(&record.entrypoint, &record.routing_plan)
            .expect("validate binding against its exact V4 record");
        binding
    }

    fn globally_bound_record(
        label: &str,
    ) -> (QueuePlanJournalRecordV4, QueuePlanAdmissionBindingV2) {
        let mut record = record(label);
        let chain_id_digest = Hash::new_from_chunks(&[b"journal-test-chain", label.as_bytes()]);
        record.global_admission_identity = Some(QueuePlanGlobalAdmissionIdentityV2 {
            version: super::super::QUEUE_PLAN_GLOBAL_ADMISSION_IDENTITY_VERSION_V2,
            chain_id_digest,
            request_id: crate::torii_proxy::queue_plan_synced_request_id_from_chain_digest(
                chain_id_digest,
                record.entrypoint_hash.clone(),
            ),
        });
        let binding = admission_binding_for_record(&record);
        (record, binding)
    }

    fn reservation_key_for_record(
        record: &QueuePlanJournalRecordV4,
        binding_hash: Hash,
    ) -> LaneQueueReservationKeyV2 {
        let coordinator = record
            .admission_context
            .route_incarnations
            .first()
            .expect("global journal fixture has a coordinator");
        LaneQueueReservationKeyV2 {
            version: LaneQueueReservationKeyV2::VERSION,
            signed_transaction_hash: HashOf::from_untyped_unchecked(Hash::from(
                record.entrypoint_hash.clone(),
            )),
            entrypoint_hash: record.entrypoint_hash.clone(),
            queue_plan_admission_binding_hash: binding_hash,
            routing_plan_digest: record.plan_digest(),
            coordinator_leg: coordinator.leg,
            lane_id: coordinator.leg.route.lane_id,
            dataspace_id: coordinator.leg.route.dataspace_id,
            lane_incarnation: coordinator.lane_incarnation,
            proposal_height: record.admission_context.proposal_height,
            lane_block_height: 1,
            lane_block_view: 0,
            reservation_owner_hash: Hash::new(b"journal reservation owner"),
            proposal_identity_hash: Hash::new(b"journal reservation proposal"),
        }
    }

    #[test]
    fn queue_plan_journal_claim_digest_binds_exact_v4_record_bytes_and_context() {
        let exact = record("claim-digest-exact");
        let exact_bytes =
            norito::encode_canonical(&exact).expect("encode exact canonical journal record");
        let exact_digest = exact.claim_digest().expect("hash exact journal record");
        assert_eq!(
            exact_digest,
            Hash::new_from_chunks(&[
                QUEUE_PLAN_JOURNAL_RECORD_CLAIM_DOMAIN,
                exact_bytes.as_slice(),
            ])
        );

        let mut timestamp_drift = exact.clone();
        timestamp_drift.enqueue_timestamp_ms =
            timestamp_drift.enqueue_timestamp_ms.saturating_add(1);
        assert_ne!(
            timestamp_drift
                .claim_digest()
                .expect("hash timestamp-drift record"),
            exact_digest
        );

        let mut plan_drift = exact.clone();
        plan_drift.routing_plan = RoutingPlan::single(super::super::RoutingDecision::new(
            iroha_data_model::nexus::LaneId::new(9),
            iroha_data_model::nexus::DataSpaceId::new(12),
        ));
        assert_ne!(
            plan_drift.claim_digest().expect("hash plan-drift record"),
            exact_digest
        );

        let mut context_drift = exact.clone();
        context_drift.admission_context.route_incarnations[0].lane_incarnation =
            Hash::new(b"journal-test-recreated-incarnation");
        assert_ne!(
            context_drift
                .claim_digest()
                .expect("hash context-drift record"),
            exact_digest
        );
    }

    #[test]
    fn v4_put_rejects_every_noncanonical_admission_context_before_append() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("noncanonical-context-v4.norito");
        let mut journal = open(&path).expect("open V4 journal");
        let exact = record("noncanonical-context");
        let mut mutations = Vec::new();

        let mut wrong_version = exact.clone();
        wrong_version.admission_context.version =
            super::super::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V2.saturating_add(1);
        mutations.push(wrong_version);

        let mut noncontiguous_height = exact.clone();
        noncontiguous_height.admission_context.proposal_height = noncontiguous_height
            .admission_context
            .proposal_height
            .saturating_add(1);
        mutations.push(noncontiguous_height);

        let mut unexpected_genesis_predecessor = exact.clone();
        unexpected_genesis_predecessor
            .admission_context
            .predecessor_block_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
            b"forged-genesis-predecessor",
        )));
        mutations.push(unexpected_genesis_predecessor);

        let mut wrong_plan_digest = exact.clone();
        wrong_plan_digest.admission_context.routing_plan_digest =
            Hash::new(b"wrong-routing-plan-digest");
        mutations.push(wrong_plan_digest);

        let mut noncanonical_plan = exact.clone();
        if let RoutingPlan::Single(leg) = &mut noncanonical_plan.routing_plan {
            leg.role = super::super::RouteLegRole::Participant;
        }
        noncanonical_plan.admission_context.route_incarnations[0].leg =
            noncanonical_plan.routing_plan.coordinator_leg();
        mutations.push(noncanonical_plan);

        let mut missing_leg = exact.clone();
        missing_leg.admission_context.route_incarnations.clear();
        mutations.push(missing_leg);

        let mut zero_incarnation = exact.clone();
        zero_incarnation.admission_context.route_incarnations[0].lane_incarnation =
            Hash::prehashed([0; Hash::LENGTH]);
        mutations.push(zero_incarnation);

        let mut zero_validator_hash = exact.clone();
        zero_validator_hash.admission_context.route_incarnations[0].validator_set_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]));
        mutations.push(zero_validator_hash);

        let mut wrong_threshold = exact;
        wrong_threshold.admission_context.route_incarnations[0].durability_threshold = 2;
        mutations.push(wrong_threshold);

        for mutation in mutations {
            let error = journal
                .put_deferred_flush(mutation)
                .expect_err("noncanonical V4 context must fail before append");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
        assert_eq!(
            fs::metadata(&path)
                .expect("noncanonical context journal metadata")
                .len(),
            u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap length"),
            "rejected contexts must not write bytes after the durable bootstrap"
        );
    }

    fn raw_frame(frame: &QueuePlanJournalFrameV4) -> Vec<u8> {
        let payload = norito::encode_canonical(frame).expect("encode canonical frame payload");
        let len = u32::try_from(payload.len()).expect("payload length");
        encode_payload(&payload, len).expect("frame payload")
    }

    fn raw_bootstrap_frame() -> Vec<u8> {
        raw_frame(&bootstrap_frame())
    }

    #[test]
    fn durable_frames_and_claims_ignore_ambient_layout_and_survive_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("canonical-ambient-restart.norito");
        let expected = record("canonical-ambient-restart");
        let canonical_record =
            norito::encode_canonical(&expected).expect("encode canonical record");
        let expected_claim = expected.claim_digest().expect("hash canonical record");
        let mut expected_file = raw_bootstrap_frame();
        expected_file
            .extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(expected.clone())));

        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            let ambient_record =
                norito::to_bytes(&expected).expect("encode alternate-layout ambient record");
            assert_ne!(ambient_record, canonical_record);
            assert_eq!(
                expected
                    .claim_digest()
                    .expect("hash record under alternate ambient layout"),
                expected_claim
            );

            let mut journal = open(&path).expect("open under alternate ambient layout");
            journal
                .replace_strict_durable(expected.clone())
                .expect("persist under alternate ambient layout");
            drop(journal);

            assert_eq!(
                norito::to_bytes(&expected).expect("encode ambient record after journal calls"),
                ambient_record,
                "canonical journal helpers must restore the caller's ambient layout"
            );
        }

        assert_eq!(
            fs::read(&path).expect("read canonical journal bytes"),
            expected_file,
            "bootstrap, payload, checksum, and commit bytes must be ambient-invariant"
        );
        let reopened = open(&path).expect("reopen canonical journal");
        assert_eq!(
            reopened.replay().expect("replay canonical journal"),
            vec![expected]
        );
    }

    #[test]
    fn frame_decoder_rejects_an_advertised_alternate_layout() {
        let frame = QueuePlanJournalFrameV4::Put(record("alternate-layout-frame"));
        let canonical = norito::encode_canonical(&frame).expect("encode canonical frame payload");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&frame).expect("encode alternate-layout frame payload")
        };
        assert_ne!(alternate, canonical);
        assert_eq!(
            norito::decode_from_bytes::<QueuePlanJournalFrameV4>(&alternate)
                .expect("ordinary Norito accepts the advertised alternate layout"),
            frame
        );

        let error = decode_frame(&alternate, limits(1))
            .expect_err("durable frame decoding must reject alternate layouts");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "queue plan journal payload is not canonically encoded"
        );
    }

    #[test]
    fn bootstrap_limits_fail_before_creating_or_mutating_the_journal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bootstrap_payload = norito::encode_canonical(&bootstrap_frame())
            .expect("encode canonical bootstrap payload");
        let bootstrap_payload_len =
            u64::try_from(bootstrap_payload.len()).expect("bootstrap payload length");
        let bootstrap_frame_len =
            u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap frame length");
        let cases = [
            (
                "payload",
                QueuePlanJournalLimits::new(
                    bootstrap_frame_len,
                    bootstrap_payload_len.saturating_sub(1),
                    TEST_MAX_BYTES,
                    1,
                ),
            ),
            (
                "file",
                QueuePlanJournalLimits::new(
                    bootstrap_frame_len.saturating_sub(1),
                    TEST_MAX_BYTES,
                    bootstrap_frame_len.saturating_sub(1),
                    1,
                ),
            ),
            (
                "threshold",
                QueuePlanJournalLimits::new(
                    bootstrap_frame_len.saturating_sub(1),
                    TEST_MAX_BYTES,
                    TEST_MAX_BYTES,
                    1,
                ),
            ),
        ];

        for (case, limits) in cases {
            let path = dir.path().join(format!("bootstrap-limit-{case}.norito"));
            let error = QueuePlanJournal::open_with_limits(&path, limits, true)
                .err()
                .expect("bootstrap limit must fail");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "case={case}");
            assert!(
                !path.exists(),
                "case={case} mutated the journal before validating bootstrap limits"
            );
            assert!(
                !path.with_extension("bootstrap.tmp").exists(),
                "case={case} created a bootstrap temp before validating limits"
            );
        }
    }

    fn decode_frame_with_budgets(
        payload: &[u8],
        element_budget: usize,
        allocation_budget: usize,
    ) -> Result<QueuePlanJournalFrameV4, norito::Error> {
        let payload_budget = payload.len();
        let limits = norito::DecodeLimits::new(
            payload_budget,
            payload_budget,
            element_budget,
            allocation_budget,
            128,
        );
        norito::decode_from_bytes_with_limits::<QueuePlanJournalFrameV4>(payload, limits)
    }

    fn minimum_decode_budgets(payload: &[u8]) -> (usize, usize) {
        let measurement_element_ceiling = payload.len().saturating_mul(64);
        let measurement_allocation_ceiling =
            payload.len().saturating_mul(64).saturating_add(1024 * 1024);
        if let Err(error) = decode_frame_with_budgets(
            payload,
            measurement_element_ceiling,
            measurement_allocation_ceiling,
        ) {
            panic!("measurement ceiling must decode the canonical frame: {error}");
        }

        let mut lower_element_budget = 0usize;
        let mut upper_element_budget = measurement_element_ceiling;
        while lower_element_budget.saturating_add(1) < upper_element_budget {
            let midpoint = lower_element_budget + (upper_element_budget - lower_element_budget) / 2;
            if decode_frame_with_budgets(payload, midpoint, measurement_allocation_ceiling).is_ok()
            {
                upper_element_budget = midpoint;
            } else {
                lower_element_budget = midpoint;
            }
        }

        let mut lower_allocation_budget = 0usize;
        let mut upper_allocation_budget = measurement_allocation_ceiling;
        while lower_allocation_budget.saturating_add(1) < upper_allocation_budget {
            let midpoint =
                lower_allocation_budget + (upper_allocation_budget - lower_allocation_budget) / 2;
            if decode_frame_with_budgets(payload, measurement_element_ceiling, midpoint).is_ok() {
                upper_allocation_budget = midpoint;
            } else {
                lower_allocation_budget = midpoint;
            }
        }
        (upper_element_budget, upper_allocation_budget)
    }

    #[test]
    fn v4_journal_replays_puts_and_exact_removes() {
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
                            first.entrypoint_hash,
                            first.plan_digest(),
                            first.claim_digest().expect("hash first claim"),
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
    fn exact_strict_tombstone_removes_once_and_is_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("exact-strict-remove.norito");
        let expected = record("exact-strict-remove");
        let mut journal = open(&path).expect("open");
        journal
            .replace_strict_durable(expected.clone())
            .expect("install exact owner");

        assert_eq!(
            journal
                .remove_exact_strict_durable(
                    expected.entrypoint_hash,
                    expected.plan_digest(),
                    expected.claim_digest().expect("hash expected claim"),
                )
                .expect("remove exact owner"),
            QueuePlanJournalExactRemoveResult::Removed
        );
        let removed_len = path.metadata().expect("removed metadata").len();
        assert!(
            journal.replay().expect("replay exact removal").is_empty(),
            "exact tombstone must remove the live owner"
        );
        assert_eq!(
            journal
                .remove_exact_strict_durable(
                    expected.entrypoint_hash,
                    expected.plan_digest(),
                    expected.claim_digest().expect("hash expected claim"),
                )
                .expect("repeat exact removal"),
            QueuePlanJournalExactRemoveResult::AlreadyAbsent
        );
        assert_eq!(
            path.metadata().expect("idempotent metadata").len(),
            removed_len,
            "AlreadyAbsent must append no second tombstone"
        );
    }

    #[test]
    fn exact_strict_tombstone_rejects_stale_plan_without_append() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("exact-strict-remove-stale.norito");
        let original = record("exact-strict-remove-stale");
        let replacement = with_single_route(original.clone(), 79, 83);
        let mut journal = open(&path).expect("open");
        journal
            .replace_strict_durable(replacement.clone())
            .expect("install replacement owner");
        let before = fs::read(&path).expect("read replacement journal");

        let error = journal
            .remove_exact_strict_durable(
                original.entrypoint_hash,
                original.plan_digest(),
                original.claim_digest().expect("hash original claim"),
            )
            .expect_err("stale plan digest must not remove replacement");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read rejected stale removal"),
            before,
            "digest mismatch must append no tombstone"
        );
        assert_eq!(
            journal.replay().expect("replay retained replacement"),
            vec![replacement]
        );
    }

    #[test]
    fn exact_tombstone_batch_validates_every_claim_before_append() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("exact-strict-remove-batch-preflight.norito");
        let first = record("exact-strict-remove-batch-first");
        let second = record("exact-strict-remove-batch-second");
        let mut journal = open(&path).expect("open");
        journal
            .replace_strict_durable(first.clone())
            .expect("install first owner");
        journal
            .replace_strict_durable(second.clone())
            .expect("install second owner");
        let before = fs::read(&path).expect("read complete live journal");

        let error = journal
            .remove_many_exact_strict_durable([
                (
                    first.entrypoint_hash,
                    first.plan_digest(),
                    first.claim_digest().expect("hash first claim"),
                ),
                (
                    second.entrypoint_hash,
                    second.plan_digest(),
                    Hash::new(b"tampered second batch claim"),
                ),
            ])
            .expect_err("one stale claim must reject the complete tombstone batch");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read rejected batch"),
            before,
            "batch preflight must append no prefix before every exact claim validates",
        );
        assert_eq!(
            journal.replay().expect("replay both retained owners"),
            vec![first.clone(), second.clone()],
        );

        let exact = [
            (
                first.entrypoint_hash,
                first.plan_digest(),
                first.claim_digest().expect("rehash first claim"),
            ),
            (
                second.entrypoint_hash,
                second.plan_digest(),
                second.claim_digest().expect("hash second claim"),
            ),
        ];
        assert_eq!(
            journal
                .remove_many_exact_strict_durable(exact)
                .expect("remove complete exact batch"),
            vec![
                QueuePlanJournalExactRemoveResult::Removed,
                QueuePlanJournalExactRemoveResult::Removed,
            ],
        );
        let removed_len = path.metadata().expect("removed batch metadata").len();
        assert_eq!(
            journal
                .remove_many_exact_strict_durable(exact)
                .expect("retry complete exact batch"),
            vec![
                QueuePlanJournalExactRemoveResult::AlreadyAbsent,
                QueuePlanJournalExactRemoveResult::AlreadyAbsent,
            ],
        );
        assert_eq!(
            path.metadata().expect("idempotent batch metadata").len(),
            removed_len,
            "an idempotent batch retry must append no tombstones",
        );
    }

    #[test]
    fn exact_tombstone_batch_restart_completes_durable_prefix_idempotently() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("exact-strict-remove-batch-prefix-restart.norito");
        let records = [
            record("exact-strict-remove-prefix-first"),
            record("exact-strict-remove-prefix-second"),
            record("exact-strict-remove-prefix-third"),
        ];
        let exact = records
            .iter()
            .map(|record| {
                (
                    record.entrypoint_hash,
                    record.plan_digest(),
                    record.claim_digest().expect("hash exact prefix claim"),
                )
            })
            .collect::<Vec<_>>();
        {
            let mut journal = open(&path).expect("open");
            for record in &records {
                journal
                    .replace_strict_durable(record.clone())
                    .expect("install exact prefix owner");
            }
            journal.inject_exact_remove_failure_after_durable_tombstones(2);

            let error = journal
                .remove_many_exact_strict_durable(exact.iter().copied())
                .expect_err("fail after the durable two-tombstone prefix");

            assert_eq!(error.kind(), io::ErrorKind::Other);
            assert!(
                journal.is_poisoned(),
                "the interrupted batch handle must require restart repair",
            );
        }

        let mut journal = open(&path).expect("restart after exact tombstone prefix");
        assert_eq!(
            journal.replay().expect("replay durable tombstone prefix"),
            vec![records[2].clone()],
            "the first two exact tombstones must survive the simulated crash",
        );
        assert_eq!(
            journal
                .remove_many_exact_strict_durable(exact.iter().copied())
                .expect("idempotently finish interrupted exact tombstone batch"),
            vec![
                QueuePlanJournalExactRemoveResult::AlreadyAbsent,
                QueuePlanJournalExactRemoveResult::AlreadyAbsent,
                QueuePlanJournalExactRemoveResult::Removed,
            ],
        );
        let completed_len = path.metadata().expect("completed batch metadata").len();
        assert!(journal.replay().expect("replay completed batch").is_empty());
        assert_eq!(
            journal
                .remove_many_exact_strict_durable(exact)
                .expect("repeat completed exact tombstone batch"),
            vec![QueuePlanJournalExactRemoveResult::AlreadyAbsent; 3],
        );
        assert_eq!(
            path.metadata().expect("idempotent retry metadata").len(),
            completed_len,
            "the completed retry must append no duplicate tombstones",
        );
    }

    #[test]
    fn exact_atomic_tombstone_batch_is_one_frame_and_restart_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("exact-atomic-remove-batch.norito");
        let records = [
            record("exact-atomic-remove-first"),
            record("exact-atomic-remove-second"),
            record("exact-atomic-remove-third"),
        ];
        let removals = records
            .iter()
            .map(|record| {
                (
                    record.entrypoint_hash,
                    record.plan_digest(),
                    record.claim_digest().expect("hash atomic removal claim"),
                )
            })
            .collect::<Vec<_>>();
        let mut journal = open(&path).expect("open");
        for record in &records {
            journal
                .replace_strict_durable(record.clone())
                .expect("install atomic removal owner");
        }
        let frame_count_before = read_frames(&path, limits(64))
            .expect("read live frames")
            .len();
        journal.reset_replay_scan_count();

        assert_eq!(
            journal
                .remove_many_exact_atomic_strict_durable(&removals)
                .expect("atomically tombstone exact owners"),
            vec![QueuePlanJournalExactRemoveResult::Removed; records.len()],
        );
        assert_eq!(
            journal.replay_scan_count(),
            1,
            "atomic exact-removal preflight must use one bounded snapshot",
        );
        let frames = read_frames(&path, limits(64)).expect("read atomic removal frames");
        assert_eq!(
            frames.len(),
            frame_count_before + 1,
            "the complete removal set must append exactly one frame",
        );
        let Some(QueuePlanJournalFrameV4::RemoveBatch(persisted)) = frames.last() else {
            panic!("the complete exact removal set must be one RemoveBatch frame");
        };
        assert_eq!(
            persisted,
            &removals
                .iter()
                .map(
                    |(entrypoint_hash, plan_digest, claim_digest)| QueuePlanJournalRemovalV4 {
                        entrypoint_hash: *entrypoint_hash,
                        plan_digest: *plan_digest,
                        claim_digest: *claim_digest,
                    },
                )
                .collect::<Vec<_>>(),
        );
        let removed_len = path.metadata().expect("atomic removal metadata").len();
        drop(journal);

        let mut journal = open(&path).expect("restart after atomic removal");
        journal.reset_replay_scan_count();
        assert_eq!(
            journal
                .remove_many_exact_atomic_strict_durable(&removals)
                .expect("retry exact atomic removal"),
            vec![QueuePlanJournalExactRemoveResult::AlreadyAbsent; records.len()],
        );
        assert_eq!(
            journal.replay_scan_count(),
            1,
            "an exact atomic retry must use one bounded snapshot",
        );
        assert_eq!(
            path.metadata().expect("atomic retry metadata").len(),
            removed_len,
            "an exact atomic retry must append no second RemoveBatch",
        );
        assert!(journal.replay().expect("replay atomic removal").is_empty());
    }

    #[test]
    fn exact_atomic_live_tombstone_batch_rejects_retry_before_append() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("exact-atomic-live-remove-batch.norito");
        let records = [
            record("exact-atomic-live-remove-first"),
            record("exact-atomic-live-remove-second"),
        ];
        let removals = records
            .iter()
            .map(|record| {
                (
                    record.entrypoint_hash,
                    record.plan_digest(),
                    record.claim_digest().expect("hash live removal claim"),
                )
            })
            .collect::<Vec<_>>();
        let mut journal = open(&path).expect("open");
        for record in records {
            journal
                .replace_strict_durable(record)
                .expect("install live atomic removal owner");
        }

        journal
            .remove_all_live_exact_atomic_strict_durable(&removals)
            .expect("atomically remove the wholly live batch");
        let removed = fs::read(&path).expect("read wholly live removal journal");
        let error = journal
            .remove_all_live_exact_atomic_strict_durable(&removals)
            .expect_err("the startup publication form must reject an already-absent retry");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("already-absent target"));
        assert_eq!(
            fs::read(&path).expect("read rejected live-removal retry"),
            removed,
            "the all-live precondition must reject before another frame is appended",
        );
    }

    #[test]
    fn exact_atomic_tombstone_batch_rejects_every_later_mismatch_without_append() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("exact-atomic-remove-batch-rejections.norito");
        let first = record("exact-atomic-rejection-first");
        let second = record("exact-atomic-rejection-second");
        let absent = record("exact-atomic-rejection-absent");
        let first_removal = (
            first.entrypoint_hash,
            first.plan_digest(),
            first.claim_digest().expect("hash first atomic claim"),
        );
        let second_removal = (
            second.entrypoint_hash,
            second.plan_digest(),
            second.claim_digest().expect("hash second atomic claim"),
        );
        let absent_removal = (
            absent.entrypoint_hash,
            absent.plan_digest(),
            absent.claim_digest().expect("hash absent atomic claim"),
        );
        let mut journal = open(&path).expect("open");
        journal
            .replace_strict_durable(first.clone())
            .expect("install first atomic owner");
        journal
            .replace_strict_durable(second.clone())
            .expect("install second atomic owner");
        let baseline = fs::read(&path).expect("read atomic rejection baseline");

        let stale_claim = (
            second.entrypoint_hash,
            second.plan_digest(),
            Hash::new(b"stale later exact atomic claim"),
        );
        let error = journal
            .remove_many_exact_atomic_strict_durable(&[first_removal, stale_claim])
            .expect_err("a stale later claim must reject the complete atomic batch");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read stale-claim rejection"),
            baseline,
            "a valid first member must not escape before a later claim rejects",
        );

        let error = journal
            .remove_many_exact_atomic_strict_durable(&[first_removal, absent_removal])
            .expect_err("an unproven later absence must reject the complete atomic batch");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read absent-member rejection"),
            baseline,
            "an unproven later absence must append no member prefix",
        );

        let error = journal
            .remove_many_exact_atomic_strict_durable(&[second_removal, second_removal])
            .expect_err("a duplicate exact atomic target must reject");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read duplicate rejection"),
            baseline,
            "a duplicate target must append no batch",
        );
        assert_eq!(
            journal.replay().expect("replay rejected atomic batches"),
            vec![first, second],
        );
    }

    #[test]
    fn exact_atomic_tombstone_batch_parent_sync_failure_replays_whole_frame() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("exact-atomic-remove-parent-sync.norito");
        let records = [
            record("exact-atomic-parent-sync-first"),
            record("exact-atomic-parent-sync-second"),
        ];
        let removals = records
            .iter()
            .map(|record| {
                (
                    record.entrypoint_hash,
                    record.plan_digest(),
                    record.claim_digest().expect("hash parent-sync claim"),
                )
            })
            .collect::<Vec<_>>();
        {
            let mut journal = open(&path).expect("open");
            for record in &records {
                journal
                    .replace_strict_durable(record.clone())
                    .expect("install parent-sync owner");
            }
            journal.inject_fault(QueuePlanJournalTestFault::GeneralParentSync);

            let error = journal
                .remove_many_exact_atomic_strict_durable(&removals)
                .expect_err("parent synchronization ambiguity must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::Other);
            assert!(journal.is_poisoned());
        }

        let journal = open(&path).expect("restart after parent-sync ambiguity");
        assert!(
            journal
                .replay()
                .expect("replay parent-sync ambiguity")
                .is_empty(),
            "one fully staged RemoveBatch must remove either every member or none",
        );
        let frames = read_frames(&path, limits(64)).expect("read parent-sync frames");
        let Some(QueuePlanJournalFrameV4::RemoveBatch(persisted)) = frames.last() else {
            panic!("the parent-sync ambiguity must retain one whole RemoveBatch");
        };
        assert_eq!(persisted.len(), records.len());
    }

    #[test]
    fn exact_strict_tombstone_rejects_same_plan_aba_claim_after_compaction_and_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("exact-strict-remove-same-plan-aba.norito");
        let original = record("exact-strict-remove-same-plan-aba");
        let original_claim_digest = original.claim_digest().expect("hash original claim");
        let mut replacement = original.clone();
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        let replacement_claim_digest = replacement.claim_digest().expect("hash replacement claim");
        assert_eq!(replacement.plan_digest(), original.plan_digest());
        assert_ne!(replacement_claim_digest, original_claim_digest);

        {
            let mut journal = open(&path).expect("open");
            journal
                .replace_strict_durable(original.clone())
                .expect("install original claim");
            journal
                .replace_strict_durable(replacement.clone())
                .expect("install same-plan replacement");
            journal.compact(true).expect("compact replacement claim");
        }

        let mut journal = open(&path).expect("restart compacted journal");
        let before = fs::read(&path).expect("read replacement journal");
        let error = journal
            .remove_exact_strict_durable(
                original.entrypoint_hash,
                original.plan_digest(),
                original_claim_digest,
            )
            .expect_err("stale same-plan claim must not delete replacement");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read rejected stale removal"),
            before,
            "same-plan claim mismatch must append no tombstone"
        );
        assert_eq!(
            journal.replay().expect("replay retained replacement"),
            vec![replacement]
        );
    }

    #[test]
    fn exact_global_binding_tombstone_reconstructs_live_claim_after_restart_and_is_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("exact-global-binding-remove.norito");
        let (record, binding) = globally_bound_record("exact-global-binding-remove");
        let entrypoint_hash = record.entrypoint_hash.clone();
        let plan_digest = record.plan_digest();
        let binding_hash = binding.canonical_hash();
        let key = reservation_key_for_record(&record, binding_hash);
        let live_claim_digest = record.claim_digest().expect("hash live global claim");
        {
            let mut journal = open(&path).expect("open");
            journal
                .replace_strict_durable(record)
                .expect("install global claim");
        }

        let mut journal = open(&path).expect("restart without caller claim digest");
        assert_eq!(
            journal
                .remove_exact_global_admission_binding_strict_durable(&key)
                .expect("remove reconstructed global claim"),
            QueuePlanJournalExactRemoveResult::Removed
        );
        assert!(
            journal
                .replay()
                .expect("replay global binding removal")
                .is_empty()
        );
        let frames = read_frames(&path, limits(64)).expect("read exact removal frames");
        assert!(matches!(
            frames.last(),
            Some(QueuePlanJournalFrameV4::Remove {
                entrypoint_hash: removed_entrypoint_hash,
                plan_digest: removed_plan_digest,
                claim_digest: removed_claim_digest,
            }) if *removed_entrypoint_hash == entrypoint_hash
                && *removed_plan_digest == plan_digest
                && *removed_claim_digest == live_claim_digest
        ));
        let removed_len = path.metadata().expect("removed metadata").len();
        assert_eq!(
            journal
                .remove_exact_global_admission_binding_strict_durable(&key)
                .expect("repeat reconstructed global removal"),
            QueuePlanJournalExactRemoveResult::AlreadyAbsent
        );
        assert_eq!(
            path.metadata().expect("idempotent metadata").len(),
            removed_len,
            "AlreadyAbsent must append no second tombstone"
        );
    }

    #[test]
    fn exact_global_binding_batch_tombstone_is_atomic_and_constant_scan() {
        const RECORDS: usize = 128;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("exact-global-binding-batch.norito");
        let batch_limits = limits(RECORDS);
        let mut records = Vec::with_capacity(RECORDS);
        let mut keys = Vec::with_capacity(RECORDS);
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, batch_limits, true).expect("open");
        let mut flush = QueuePlanJournalFlush::default();
        for index in 0..RECORDS {
            let (record, binding) =
                globally_bound_record(&format!("exact-global-binding-batch-{index}"));
            keys.push(reservation_key_for_record(
                &record,
                binding.canonical_hash(),
            ));
            flush = flush.combine(
                journal
                    .put_deferred_flush(record.clone())
                    .expect("append global batch claim"),
            );
            records.push(record);
        }
        journal
            .flush_deferred(flush)
            .expect("flush global batch claims");
        journal.reset_replay_scan_count();

        let outcomes = journal
            .remove_exact_global_admission_bindings_strict_durable(&keys)
            .expect("remove exact global batch");

        assert_eq!(
            outcomes,
            vec![QueuePlanJournalExactRemoveResult::Removed; RECORDS]
        );
        assert_eq!(
            journal.replay_scan_count(),
            1,
            "batch validation must reconstruct the journal exactly once"
        );
        let frames = read_frames(&path, batch_limits).expect("read atomic batch frames");
        let Some(QueuePlanJournalFrameV4::RemoveBatch(removals)) = frames.last() else {
            panic!("the complete removal set must be one atomic RemoveBatch frame");
        };
        assert_eq!(removals.len(), RECORDS);
        assert_eq!(
            removals
                .iter()
                .map(|removal| removal.entrypoint_hash.clone())
                .collect::<Vec<_>>(),
            keys.iter()
                .map(|key| key.entrypoint_hash.clone())
                .collect::<Vec<_>>()
        );
        let removed_len = path.metadata().expect("removed metadata").len();
        drop(journal);

        let mut journal =
            QueuePlanJournal::open_with_limits(&path, batch_limits, true).expect("restart");
        journal.reset_replay_scan_count();
        assert_eq!(
            journal
                .remove_exact_global_admission_bindings_strict_durable(&keys)
                .expect("retry exact durable batch"),
            vec![QueuePlanJournalExactRemoveResult::AlreadyAbsent; RECORDS]
        );
        assert_eq!(
            journal.replay_scan_count(),
            1,
            "exact tombstone retry must remain one bounded replay scan"
        );
        assert_eq!(
            path.metadata().expect("retry metadata").len(),
            removed_len,
            "an exactly tombstoned retry must append no second batch"
        );
        assert!(journal.replay().expect("replay removed batch").is_empty());
    }

    #[test]
    fn exact_global_binding_batch_rejects_duplicate_absent_and_aba_without_append() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("exact-global-binding-batch-rejections.norito");
        let (first, first_binding) = globally_bound_record("batch-rejection-first");
        let first_key = reservation_key_for_record(&first, first_binding.canonical_hash());
        let (second, second_binding) = globally_bound_record("batch-rejection-second");
        let second_key = reservation_key_for_record(&second, second_binding.canonical_hash());
        let (absent, absent_binding) = globally_bound_record("batch-rejection-absent");
        let absent_key = reservation_key_for_record(&absent, absent_binding.canonical_hash());
        let mut journal = open(&path).expect("open");
        journal
            .replace_strict_durable(first.clone())
            .expect("install first global claim");
        journal
            .replace_strict_durable(second.clone())
            .expect("install second global claim");

        let before_duplicate = fs::read(&path).expect("read duplicate baseline");
        let duplicate_error = journal
            .remove_exact_global_admission_bindings_strict_durable(&[first_key, first_key])
            .expect_err("duplicate batch key must fail closed");
        assert_eq!(duplicate_error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read duplicate rejection"),
            before_duplicate
        );

        let absent_error = journal
            .remove_exact_global_admission_bindings_strict_durable(&[first_key, absent_key])
            .expect_err("unproven absent batch key must reject the complete batch");
        assert_eq!(absent_error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read absent rejection"),
            before_duplicate,
            "a valid key preceding an absent key must not be partially tombstoned"
        );

        let mut replacement = first.clone();
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        let replacement_binding = admission_binding_for_record(&replacement);
        assert_ne!(
            replacement_binding.canonical_hash(),
            first_binding.canonical_hash()
        );
        journal
            .replace_strict_durable(replacement.clone())
            .expect("install same-entrypoint ABA replacement");
        let before_aba = fs::read(&path).expect("read ABA baseline");
        let aba_error = journal
            .remove_exact_global_admission_bindings_strict_durable(&[second_key, first_key])
            .expect_err("stale ABA key must reject the complete batch");
        assert_eq!(aba_error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read ABA rejection"),
            before_aba,
            "a valid key preceding an ABA key must not be partially tombstoned"
        );
        let replayed = journal.replay().expect("replay rejected batch claims");
        assert_eq!(replayed.len(), 2);
        assert!(replayed.contains(&replacement));
        assert!(replayed.contains(&second));

        let (retry_original, retry_binding) = globally_bound_record("batch-retry-then-put");
        let retry_key = reservation_key_for_record(&retry_original, retry_binding.canonical_hash());
        journal
            .replace_strict_durable(retry_original.clone())
            .expect("install retry original");
        assert_eq!(
            journal
                .remove_exact_global_admission_bindings_strict_durable(&[retry_key])
                .expect("tombstone retry original"),
            vec![QueuePlanJournalExactRemoveResult::Removed]
        );
        let mut retry_replacement = retry_original;
        retry_replacement.enqueue_timestamp_ms =
            retry_replacement.enqueue_timestamp_ms.saturating_add(1);
        journal
            .replace_strict_durable(retry_replacement)
            .expect("install later ABA owner");
        let before_later_put = fs::read(&path).expect("read later-Put baseline");
        let retry_error = journal
            .remove_exact_global_admission_bindings_strict_durable(&[retry_key])
            .expect_err("a later Put must invalidate exact tombstone retry provenance");
        assert_eq!(retry_error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read rejected old retry"),
            before_later_put
        );
    }

    #[test]
    fn torn_global_binding_remove_batch_replays_all_live_claims() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("torn-global-binding-batch.norito");
        let (first, _) = globally_bound_record("torn-batch-first");
        let (second, _) = globally_bound_record("torn-batch-second");
        {
            let mut journal = open(&path).expect("open");
            journal
                .replace_strict_durable(first.clone())
                .expect("install first claim");
            journal
                .replace_strict_durable(second.clone())
                .expect("install second claim");
        }
        let canonical = fs::read(&path).expect("read canonical claims");
        let torn_batch = raw_frame(&QueuePlanJournalFrameV4::RemoveBatch(vec![
            QueuePlanJournalRemovalV4 {
                entrypoint_hash: first.entrypoint_hash.clone(),
                plan_digest: first.plan_digest(),
                claim_digest: first.claim_digest().expect("hash first claim"),
            },
            QueuePlanJournalRemovalV4 {
                entrypoint_hash: second.entrypoint_hash.clone(),
                plan_digest: second.plan_digest(),
                claim_digest: second.claim_digest().expect("hash second claim"),
            },
        ]));
        let mut append = OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open torn append");
        append
            .write_all(&torn_batch[..torn_batch.len() - 3])
            .expect("append torn batch");
        append.sync_all().expect("sync torn batch");
        drop(append);

        let journal = open(&path).expect("repair torn batch");
        assert_eq!(
            fs::read(&path).expect("read repaired journal"),
            canonical,
            "a batch without its complete commit marker must be truncated as one unit"
        );
        assert_eq!(
            journal.replay().expect("replay repaired batch"),
            vec![first, second]
        );
    }

    #[test]
    fn exact_global_binding_tombstone_rejects_wrong_hash_without_append() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("exact-global-binding-wrong-hash.norito");
        let (record, binding) = globally_bound_record("exact-global-binding-wrong-hash");
        let mut journal = open(&path).expect("open");
        journal
            .replace_strict_durable(record.clone())
            .expect("install global claim");
        let before = fs::read(&path).expect("read live global claim");
        let wrong_binding_hash =
            Hash::new_from_chunks(&[binding.canonical_hash().as_ref(), b"forged-binding-hash"]);
        let key = reservation_key_for_record(&record, wrong_binding_hash);

        let error = journal
            .remove_exact_global_admission_binding_strict_durable(&key)
            .expect_err("wrong canonical binding hash must fail closed");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read rejected binding removal"),
            before,
            "binding-hash mismatch must append no tombstone"
        );
        assert_eq!(
            journal.replay().expect("replay retained global claim"),
            vec![record]
        );
    }

    #[test]
    fn exact_global_binding_tombstone_rejects_route_mismatch_and_ordinary_claim() {
        let dir = tempfile::tempdir().expect("tempdir");
        let global_path = dir.path().join("exact-global-binding-wrong-route.norito");
        let (global_record, binding) = globally_bound_record("exact-global-binding-wrong-route");
        let mut global_journal = open(&global_path).expect("open global journal");
        global_journal
            .replace_strict_durable(global_record.clone())
            .expect("install global claim");
        let global_before = fs::read(&global_path).expect("read global claim");
        let mut wrong_route_key =
            reservation_key_for_record(&global_record, binding.canonical_hash());
        wrong_route_key.routing_plan_digest = Hash::new(b"forged-routing-plan-digest");

        let route_error = global_journal
            .remove_exact_global_admission_binding_strict_durable(&wrong_route_key)
            .expect_err("wrong routing-plan digest must fail closed");
        assert_eq!(route_error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&global_path).expect("read rejected route removal"),
            global_before,
            "route mismatch must append no tombstone"
        );
        assert_eq!(
            global_journal
                .replay()
                .expect("replay retained route owner"),
            vec![global_record]
        );

        let ordinary_path = dir.path().join("exact-global-binding-ordinary.norito");
        let ordinary_record = record("exact-global-binding-ordinary");
        let mut ordinary_journal = open(&ordinary_path).expect("open ordinary journal");
        ordinary_journal
            .replace_strict_durable(ordinary_record.clone())
            .expect("install ordinary claim");
        let ordinary_before = fs::read(&ordinary_path).expect("read ordinary claim");
        let ordinary_key = reservation_key_for_record(&ordinary_record, binding.canonical_hash());
        let ordinary_error = ordinary_journal
            .remove_exact_global_admission_binding_strict_durable(&ordinary_key)
            .expect_err("ordinary claim must not reconstruct as a global binding");
        assert_eq!(ordinary_error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&ordinary_path).expect("read rejected ordinary removal"),
            ordinary_before,
            "non-global claim must append no tombstone"
        );
        assert_eq!(
            ordinary_journal
                .replay()
                .expect("replay retained ordinary owner"),
            vec![ordinary_record]
        );
    }

    #[test]
    fn exact_global_binding_tombstone_rejects_same_plan_aba_after_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("exact-global-binding-same-plan-aba.norito");
        let (original, original_binding) =
            globally_bound_record("exact-global-binding-same-plan-aba");
        let mut replacement = original.clone();
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        replacement.global_admission_identity = Some(QueuePlanGlobalAdmissionIdentityV2 {
            version: super::super::QUEUE_PLAN_GLOBAL_ADMISSION_IDENTITY_VERSION_V2,
            chain_id_digest: original_binding.chain_id_digest,
            request_id: original_binding.request_id,
        });
        let replacement_binding = admission_binding_for_record(&replacement);
        assert_eq!(replacement.plan_digest(), original.plan_digest());
        assert_ne!(
            replacement_binding.canonical_hash(),
            original_binding.canonical_hash()
        );
        {
            let mut journal = open(&path).expect("open");
            journal
                .replace_strict_durable(original.clone())
                .expect("install original global claim");
            journal
                .replace_strict_durable(replacement.clone())
                .expect("install same-plan replacement");
            journal.compact(true).expect("compact replacement claim");
        }

        let mut journal = open(&path).expect("restart compacted journal");
        let before = fs::read(&path).expect("read replacement claim");
        let original_key = reservation_key_for_record(&original, original_binding.canonical_hash());
        let error = journal
            .remove_exact_global_admission_binding_strict_durable(&original_key)
            .expect_err("stale same-plan global binding must not delete replacement");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("read rejected stale global removal"),
            before,
            "same-plan ABA mismatch must append no tombstone"
        );
        assert_eq!(
            journal.replay().expect("replay retained replacement"),
            vec![replacement.clone()]
        );
        let replacement_key =
            reservation_key_for_record(&replacement, replacement_binding.canonical_hash());
        assert_eq!(
            journal
                .remove_exact_global_admission_binding_strict_durable(&replacement_key)
                .expect("remove exact replacement binding"),
            QueuePlanJournalExactRemoveResult::Removed
        );
    }

    #[test]
    fn exact_strict_tombstone_forces_bounded_preflight_compaction() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("exact-strict-remove-preflight.norito");
        let original = record("exact-strict-remove-preflight");
        let mut replacement = original.clone();
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        let bootstrap_len = raw_bootstrap_frame().len();
        let original_len = raw_frame(&QueuePlanJournalFrameV4::Put(original.clone())).len();
        let replacement_len = raw_frame(&QueuePlanJournalFrameV4::Put(replacement.clone())).len();
        let max_file_bytes = bootstrap_len
            .checked_add(original_len)
            .and_then(|bytes| bytes.checked_add(replacement_len))
            .expect("full replacement history");
        let bounded_limits = QueuePlanJournalLimits::new(
            u64::try_from(max_file_bytes).expect("compaction threshold"),
            TEST_MAX_BYTES,
            u64::try_from(max_file_bytes).expect("file limit"),
            1,
        );
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, bounded_limits, true).expect("open");
        journal
            .put_deferred_flush(original)
            .expect("append original owner");
        journal
            .put_deferred_flush(replacement.clone())
            .expect("append replacement owner");
        journal
            .sync_all_with_parent()
            .expect("sync full replacement history");

        assert_eq!(
            journal
                .remove_exact_strict_durable(
                    replacement.entrypoint_hash,
                    replacement.plan_digest(),
                    replacement.claim_digest().expect("hash replacement claim"),
                )
                .expect("compact before exact tombstone"),
            QueuePlanJournalExactRemoveResult::Removed
        );
        assert!(
            journal
                .replay()
                .expect("replay compacted exact removal")
                .is_empty()
        );
        assert!(
            path.metadata().expect("compacted removal metadata").len()
                <= u64::try_from(max_file_bytes).expect("file limit")
        );
    }

    #[test]
    fn strict_replace_success_shadows_same_key_and_stays_healthy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-success.norito");
        let original = record("strict-success");
        let mut replacement = original.clone();
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        let mut journal = open(&path).expect("open");

        journal
            .replace_strict_durable(original)
            .expect("initial strict replacement");
        journal
            .replace_strict_durable(replacement.clone())
            .expect("same-key strict replacement");

        assert!(!journal.is_poisoned());
        assert_eq!(journal.replay().expect("replay"), vec![replacement]);
    }

    #[test]
    fn repeated_strict_replacements_compact_boundedly_and_preserve_fifo_ownership() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-replacement-bounded.norito");
        let mut first = record("strict-replacement-bounded-first");
        let second = record("strict-replacement-bounded-second");
        let bootstrap_bytes = raw_bootstrap_frame().len();
        let first_frame_bytes = raw_frame(&QueuePlanJournalFrameV4::Put(first.clone())).len();
        let second_frame_bytes = raw_frame(&QueuePlanJournalFrameV4::Put(second.clone())).len();
        let compacted_bytes = bootstrap_bytes
            .checked_add(first_frame_bytes)
            .expect("bootstrap plus first frame")
            .checked_add(second_frame_bytes)
            .expect("compacted fixture size");
        let max_file_bytes = compacted_bytes
            .checked_add(first_frame_bytes)
            .expect("one replacement history frame");
        let bounded_limits = QueuePlanJournalLimits::new(
            u64::try_from(compacted_bytes).expect("compaction threshold"),
            TEST_MAX_BYTES,
            u64::try_from(max_file_bytes).expect("journal file limit"),
            2,
        );
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, bounded_limits, true).expect("open");
        journal
            .replace_strict_durable(first.clone())
            .expect("admit first owner");
        journal
            .replace_strict_durable(second.clone())
            .expect("admit second owner");

        for generation in 1..=64_u64 {
            first.enqueue_timestamp_ms = first.enqueue_timestamp_ms.saturating_add(1);
            journal
                .replace_strict_durable(first.clone())
                .unwrap_or_else(|error| {
                    panic!("replace generation {generation} within the bounded journal: {error}")
                });
            assert!(
                path.metadata().expect("journal metadata").len()
                    <= u64::try_from(compacted_bytes).expect("compacted fixture size"),
                "generation {generation} left replacement history above the compaction threshold"
            );
            assert_eq!(
                journal.replay().expect("replay bounded replacements"),
                vec![first.clone(), second.clone()],
                "same-entrypoint replacement must retain its original FIFO ownership"
            );
        }

        assert_eq!(
            read_frames(&path, bounded_limits).expect("read final compacted frames"),
            vec![
                QueuePlanJournalFrameV4::Put(first),
                QueuePlanJournalFrameV4::Put(second),
            ]
        );
    }

    #[test]
    fn strict_replace_forces_preflight_compaction_after_history_reaches_file_limit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-replacement-preflight.norito");
        let mut latest = record("strict-replacement-preflight");
        let bootstrap_bytes = raw_bootstrap_frame().len();
        let frame_bytes = raw_frame(&QueuePlanJournalFrameV4::Put(latest.clone())).len();
        let max_file_bytes = bootstrap_bytes
            .checked_add(frame_bytes.checked_mul(3).expect("three-frame history"))
            .expect("bootstrap plus history");
        let bounded_limits = QueuePlanJournalLimits::new(
            u64::try_from(max_file_bytes).expect("compaction threshold"),
            TEST_MAX_BYTES,
            u64::try_from(max_file_bytes).expect("journal file limit"),
            1,
        );
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, bounded_limits, true).expect("open");
        for generation in 0..3_u64 {
            latest.enqueue_timestamp_ms = latest.enqueue_timestamp_ms.saturating_add(generation);
            journal
                .put_deferred_flush(latest.clone())
                .expect("append replacement-only history");
        }
        journal
            .sync_all_with_parent()
            .expect("sync full replacement history");
        assert_eq!(
            path.metadata().expect("full journal metadata").len(),
            u64::try_from(max_file_bytes).expect("journal file limit")
        );

        latest.enqueue_timestamp_ms = latest.enqueue_timestamp_ms.saturating_add(1);
        journal
            .replace_strict_durable(latest.clone())
            .expect("forced preflight compaction must free replacement capacity");

        assert_eq!(journal.replay().expect("replay replacement"), vec![latest]);
        assert_eq!(
            path.metadata().expect("compacted journal metadata").len(),
            u64::try_from(
                bootstrap_bytes
                    .checked_add(frame_bytes.checked_mul(2).expect("two-frame history"))
                    .expect("bootstrap plus two-frame history")
            )
            .expect("two-frame history length")
        );
    }

    #[test]
    fn failed_preflight_compaction_is_definitely_not_live_and_faults_the_journal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("strict-replacement-preflight-failure.norito");
        let mut latest = record("strict-replacement-preflight-failure");
        let bootstrap_bytes = raw_bootstrap_frame().len();
        let frame_bytes = raw_frame(&QueuePlanJournalFrameV4::Put(latest.clone())).len();
        let max_file_bytes = bootstrap_bytes
            .checked_add(frame_bytes.checked_mul(2).expect("two-frame history"))
            .expect("bootstrap plus history");
        let bounded_limits = QueuePlanJournalLimits::new(
            u64::try_from(max_file_bytes).expect("compaction threshold"),
            TEST_MAX_BYTES,
            u64::try_from(max_file_bytes).expect("journal file limit"),
            1,
        );
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, bounded_limits, true).expect("open");
        journal
            .put_deferred_flush(latest.clone())
            .expect("append first history frame");
        latest.enqueue_timestamp_ms = latest.enqueue_timestamp_ms.saturating_add(1);
        journal
            .put_deferred_flush(latest.clone())
            .expect("append second history frame");
        journal
            .sync_all_with_parent()
            .expect("sync full replacement history");
        journal.inject_fault(QueuePlanJournalTestFault::CompactionAfterTempCreate);

        latest.enqueue_timestamp_ms = latest.enqueue_timestamp_ms.saturating_add(1);
        let error = journal
            .replace_strict_durable(latest)
            .expect_err("preflight compaction fault must reject replacement");

        assert!(!error.is_indeterminate());
        assert!(error.journal_faulted());
        assert!(journal.is_poisoned());
        assert!(path.with_extension("tmp").is_file());
        assert_eq!(
            path.metadata().expect("original journal metadata").len(),
            u64::try_from(max_file_bytes).expect("original journal length"),
            "the incoming replacement cannot enter the authoritative journal before preflight succeeds"
        );
    }

    #[test]
    fn failed_post_durability_compaction_is_indeterminate_and_retains_replacement() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("strict-replacement-post-compact-failure.norito");
        let original = record("strict-replacement-post-compact-failure");
        let mut replacement = original.clone();
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        let bootstrap_bytes = raw_bootstrap_frame().len();
        let frame_bytes = raw_frame(&QueuePlanJournalFrameV4::Put(original.clone())).len();
        let bounded_limits = QueuePlanJournalLimits::new(
            u64::try_from(
                bootstrap_bytes
                    .checked_add(frame_bytes)
                    .expect("bootstrap plus one-frame compaction threshold"),
            )
            .expect("one-frame compaction threshold"),
            TEST_MAX_BYTES,
            u64::try_from(
                bootstrap_bytes
                    .checked_add(frame_bytes.checked_mul(2).expect("two-frame journal limit"))
                    .expect("bootstrap plus two-frame journal limit"),
            )
            .expect("two-frame journal limit"),
            1,
        );
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, bounded_limits, true).expect("open");
        journal
            .replace_strict_durable(original.clone())
            .expect("seed owner");
        journal.inject_fault(QueuePlanJournalTestFault::CompactionAfterTempCreate);

        let error = journal
            .replace_strict_durable(replacement.clone())
            .expect_err("post-durability compaction fault must require reconciliation");

        assert!(error.is_indeterminate());
        assert!(error.journal_faulted());
        assert!(journal.is_poisoned());
        assert!(path.with_extension("tmp").is_file());
        assert_eq!(
            read_frames(&path, bounded_limits).expect("read authoritative append history"),
            vec![
                QueuePlanJournalFrameV4::Put(original),
                QueuePlanJournalFrameV4::Put(replacement),
            ],
            "the durably appended replacement remains in the authoritative pre-compaction history"
        );
    }

    #[test]
    fn strict_replace_accepts_exact_single_frame_capacity() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-capacity.norito");
        let expected = record("strict-capacity");
        let put_bytes = raw_frame(&QueuePlanJournalFrameV4::Put(expected.clone()));
        let max_file_bytes = u64::try_from(
            raw_bootstrap_frame()
                .len()
                .checked_add(put_bytes.len())
                .expect("bootstrap plus Put frame"),
        )
        .expect("bootstrap plus Put frame length");
        let strict_limits = QueuePlanJournalLimits::new(
            u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap length"),
            TEST_MAX_BYTES,
            max_file_bytes,
            64,
        );
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, strict_limits, true).expect("open");

        journal
            .replace_strict_durable(expected.clone())
            .expect("one exact replacement frame must fit");

        assert!(!journal.is_poisoned());
        assert_eq!(path.metadata().expect("metadata").len(), max_file_bytes);
        assert_eq!(journal.replay().expect("replay"), vec![expected]);
    }

    #[test]
    fn strict_replace_prewrite_failure_is_definitely_not_live_and_healthy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-prewrite.norito");
        let expected = record("strict-prewrite");
        let mut journal = open(&path).expect("open");
        journal.inject_fault(QueuePlanJournalTestFault::ReplaceBeforeAppend);

        let error = journal
            .replace_strict_durable(expected)
            .expect_err("prewrite replacement must fail");

        assert!(!error.is_indeterminate());
        assert!(!error.journal_faulted());
        assert!(!journal.is_poisoned());
        assert_eq!(
            path.metadata().expect("metadata").len(),
            u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap length")
        );
        drop(journal);
        assert!(
            open(&path)
                .expect("reopen after prewrite failure")
                .replay()
                .expect("replay after prewrite failure")
                .is_empty()
        );
    }

    #[test]
    fn strict_replace_partial_write_is_indeterminate_and_repairs_to_prior_owner() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-partial.norito");
        let original = record("strict-partial");
        let mut replacement = original.clone();
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        let mut journal = open(&path).expect("open");
        journal
            .replace_strict_durable(original.clone())
            .expect("seed original owner");
        journal.inject_fault(QueuePlanJournalTestFault::ReplacePartialWrite);

        let error = journal
            .replace_strict_durable(replacement)
            .expect_err("partial replacement must fail");

        assert!(error.is_indeterminate());
        assert!(error.journal_faulted());
        assert!(journal.is_poisoned());
        drop(journal);
        assert_eq!(
            open(&path).expect("repair").replay().expect("replay"),
            vec![original]
        );
    }

    #[test]
    fn staged_uncommitted_replace_faults_repair_to_prior_owner() {
        for fault in [
            QueuePlanJournalTestFault::ReplaceHeaderFullTear,
            QueuePlanJournalTestFault::ReplaceAfterBodySync,
            QueuePlanJournalTestFault::ReplaceCommitPartialWrite,
        ] {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("strict-staged-{fault:?}.norito"));
            let original = record(&format!("strict-staged-{fault:?}"));
            let mut replacement = original.clone();
            replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
            let mut journal = open(&path).expect("open");
            journal
                .replace_strict_durable(original.clone())
                .expect("seed original owner");
            journal.inject_fault(fault);

            let error = journal
                .replace_strict_durable(replacement)
                .expect_err("uncommitted staged replacement must fail");

            assert!(error.is_indeterminate(), "fault={fault:?}");
            assert!(error.journal_faulted(), "fault={fault:?}");
            assert!(journal.is_poisoned(), "fault={fault:?}");
            drop(journal);
            assert_eq!(
                open(&path)
                    .unwrap_or_else(|error| panic!("repair {fault:?}: {error}"))
                    .replay()
                    .expect("replay repaired staged write"),
                vec![original],
                "fault={fault:?}"
            );
        }
    }

    #[test]
    fn first_strict_replace_partial_write_repairs_to_durable_bootstrap() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-first-partial.norito");
        let mut journal = open(&path).expect("open");
        journal.inject_fault(QueuePlanJournalTestFault::ReplacePartialWrite);

        let error = journal
            .replace_strict_durable(record("strict-first-partial"))
            .expect_err("partial first replacement must fail");

        assert!(error.is_indeterminate());
        assert!(error.journal_faulted());
        assert!(journal.is_poisoned());
        drop(journal);
        let repaired = open(&path).expect("repair first replacement");
        assert!(
            repaired
                .replay()
                .expect("replay repaired first replacement")
                .is_empty()
        );
        drop(repaired);
        assert_eq!(
            fs::read(&path).expect("read repaired first replacement"),
            raw_bootstrap_frame()
        );
    }

    #[test]
    fn strict_replace_complete_write_ambiguity_replays_new_owner_after_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-full-write.norito");
        let original = record("strict-full-write");
        let mut replacement = original.clone();
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        let mut journal = open(&path).expect("open");
        journal
            .replace_strict_durable(original)
            .expect("seed original owner");
        journal.inject_fault(QueuePlanJournalTestFault::ReplaceAfterFullWrite);

        let error = journal
            .replace_strict_durable(replacement.clone())
            .expect_err("complete replacement append ambiguity must fail");

        assert!(error.is_indeterminate());
        assert!(journal.is_poisoned());
        drop(journal);
        let first_restart = open(&path).expect("first restart adopts complete replacement");
        assert_eq!(
            first_restart.replay().expect("first restart replay"),
            vec![replacement.clone()]
        );
        drop(first_restart);
        assert_eq!(
            open(&path)
                .expect("second restart after adopted-byte synchronization")
                .replay()
                .expect("second restart replay"),
            vec![replacement]
        );
    }

    #[test]
    fn startup_adopts_and_resynchronizes_complete_presync_remove_before_second_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("adopt-complete-presync-remove.norito");
        let owner = record("adopt-complete-presync-remove");
        let mut journal = open(&path).expect("open");
        journal
            .replace_strict_durable(owner.clone())
            .expect("seed durable owner");
        drop(journal);

        let plan_digest = owner.plan_digest();
        let entrypoint_hash = owner.entrypoint_hash;
        let remove = raw_frame(&QueuePlanJournalFrameV4::Remove {
            entrypoint_hash,
            plan_digest,
            claim_digest: owner.claim_digest().expect("hash owner claim"),
        });
        let mut append = OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open unsynchronized Remove append");
        append
            .write_all(&remove)
            .expect("write complete Remove before file synchronization");
        drop(append);

        let first_restart = open(&path).expect("first restart adopts complete Remove");
        assert!(
            first_restart
                .replay()
                .expect("first restart replay")
                .is_empty()
        );
        drop(first_restart);
        assert!(
            open(&path)
                .expect("second restart after adopted Remove synchronization")
                .replay()
                .expect("second restart replay")
                .is_empty()
        );
    }

    #[test]
    fn strict_replace_sync_failure_is_indeterminate_and_replays_new_owner() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-sync.norito");
        let original = record("strict-sync");
        let mut replacement = original.clone();
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        let mut journal = open(&path).expect("open");
        journal
            .replace_strict_durable(original)
            .expect("seed original owner");
        journal.inject_fault(QueuePlanJournalTestFault::ReplaceSync);

        let error = journal
            .replace_strict_durable(replacement.clone())
            .expect_err("replacement sync must fail");

        assert!(error.is_indeterminate());
        assert!(journal.is_poisoned());
        drop(journal);
        assert_eq!(
            open(&path).expect("reopen").replay().expect("replay"),
            vec![replacement]
        );
    }

    #[test]
    fn strict_replace_parent_sync_failure_is_indeterminate_and_replays_new_owner() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("strict-parent-sync.norito");
        let original = record("strict-parent-sync");
        let mut replacement = original.clone();
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        let mut journal = open(&path).expect("open");
        journal
            .replace_strict_durable(original)
            .expect("seed original owner");
        journal.inject_fault(QueuePlanJournalTestFault::ReplaceParentSync);

        let error = journal
            .replace_strict_durable(replacement.clone())
            .expect_err("replacement parent sync must fail");

        assert!(error.is_indeterminate());
        assert!(error.journal_faulted());
        assert!(journal.is_poisoned());
        drop(journal);
        assert_eq!(
            open(&path).expect("reopen").replay().expect("replay"),
            vec![replacement]
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
    fn initial_truncated_v4_header_fails_closed_without_rewrite() {
        let frame = raw_frame(&QueuePlanJournalFrameV4::Put(record(
            "initial-header-prefix",
        )));
        let header_len = usize::try_from(FRAME_HEADER_BYTES).expect("header");

        for cut in 1..header_len {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("initial-prefix-{cut}.norito"));
            let prefix = frame[..cut].to_vec();
            fs::write(&path, &prefix).expect("write initial header prefix");

            let error = open(&path)
                .err()
                .expect("an initial truncated header must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "cut={cut}");
            assert_eq!(
                fs::read(&path).expect("retain initial prefix"),
                prefix,
                "cut={cut}"
            );
        }
    }

    #[test]
    fn every_ambiguous_initial_magic_prefix_fails_closed_without_rewrite() {
        let layouts: [(&str, &[u8]); 3] = [
            ("v4", &QUEUE_PLAN_JOURNAL_FRAME_MAGIC),
            ("legacy-v2", b"IRQPJNL2"),
            ("unknown", b"UNKNOWN!"),
        ];
        for (layout, magic) in layouts {
            for cut in 1..=7 {
                let dir = tempfile::tempdir().expect("tempdir");
                let path = dir
                    .path()
                    .join(format!("initial-{layout}-prefix-{cut}.norito"));
                let prefix = magic[..cut].to_vec();
                fs::write(&path, &prefix).expect("write ambiguous initial prefix");

                let error = open(&path)
                    .err()
                    .expect("an ambiguous initial prefix must fail closed");
                assert_eq!(
                    error.kind(),
                    io::ErrorKind::InvalidData,
                    "layout={layout}, cut={cut}"
                );
                assert_eq!(
                    fs::read(&path).expect("retain ambiguous initial prefix"),
                    prefix,
                    "layout={layout}, cut={cut}"
                );
            }
        }
    }

    #[test]
    fn headerless_complete_v4_operation_header_fails_closed_without_rewrite() {
        let frame = raw_frame(&QueuePlanJournalFrameV4::Put(record(
            "complete-initial-v4-header",
        )));
        let header_len = usize::try_from(FRAME_HEADER_BYTES).expect("header");
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("complete-initial-v4-header.norito");
        let bytes = frame[..header_len].to_vec();
        fs::write(&path, &bytes).expect("write complete headerless V4 operation header");

        let error = open(&path)
            .err()
            .expect("headerless V4 operation must not bootstrap implicitly");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read(&path).expect("retain headerless evidence"), bytes);
    }

    #[test]
    fn duplicate_or_wrong_bootstrap_fails_closed_without_rewrite() {
        let mut duplicate = raw_bootstrap_frame();
        duplicate.extend_from_slice(&raw_bootstrap_frame());
        let wrong = raw_frame(&QueuePlanJournalFrameV4::Bootstrap {
            version: QUEUE_PLAN_JOURNAL_VERSION,
            format_digest: Hash::new(b"wrong-bootstrap-domain"),
        });

        for (case, bytes) in [("duplicate", duplicate), ("wrong", wrong)] {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("bootstrap-{case}.norito"));
            fs::write(&path, &bytes).expect("write invalid bootstrap fixture");

            let error = open(&path)
                .err()
                .expect("invalid bootstrap layout must fail closed");

            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "case={case}");
            assert_eq!(
                fs::read(&path).expect("retain invalid bootstrap evidence"),
                bytes,
                "case={case}"
            );
        }
    }

    #[test]
    fn every_canonical_bootstrap_temp_prefix_recovers_to_one_durable_marker() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bootstrap-prefix-recovery.norito");
        let temporary = path.with_extension("bootstrap.tmp");
        let bootstrap = raw_bootstrap_frame();

        for cut in 0..=bootstrap.len() {
            fs::write(&path, []).expect("reset empty canonical journal");
            fs::write(&temporary, &bootstrap[..cut]).expect("write bootstrap temp prefix");

            let journal = open(&path).unwrap_or_else(|error| {
                panic!("recover exact bootstrap prefix at cut {cut}: {error}")
            });
            assert!(
                journal
                    .replay()
                    .expect("replay bootstrapped journal")
                    .is_empty(),
                "cut={cut}"
            );
            drop(journal);
            assert_eq!(
                fs::read(&path).expect("read canonical bootstrap"),
                bootstrap,
                "cut={cut}"
            );
            assert!(!temporary.exists(), "cut={cut}");
        }
    }

    #[test]
    fn staged_bootstrap_temp_tears_rebuild_to_one_durable_marker() {
        let bootstrap = raw_bootstrap_frame();
        let header = usize::try_from(FRAME_HEADER_BYTES).expect("header");
        let commit_start = bootstrap
            .len()
            .checked_sub(QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len())
            .expect("commit start");
        let checksum_start = commit_start
            .checked_sub(Hash::LENGTH)
            .expect("checksum start");
        let mut body = bootstrap.clone();
        body[header] ^= 0x80;
        body[commit_start..].fill(0);

        let mut checksum = bootstrap.clone();
        checksum[checksum_start] ^= 0x80;
        checksum[commit_start..].fill(0);

        let mut commit = bootstrap.clone();
        commit[commit_start..].fill(0);
        let cases = [
            ("header", vec![0xA5_u8; header]),
            ("body", body),
            ("checksum", checksum),
            ("commit", commit),
        ];

        for (case, torn) in cases {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("bootstrap-{case}-tear.norito"));
            let temporary = path.with_extension("bootstrap.tmp");
            fs::write(&path, []).expect("write empty canonical");
            fs::write(&temporary, torn).expect("write staged bootstrap tear");

            let journal = open(&path)
                .unwrap_or_else(|error| panic!("recover staged bootstrap {case} tear: {error}"));
            assert!(journal.replay().expect("replay bootstrap").is_empty());
            drop(journal);
            assert_eq!(
                fs::read(&path).expect("read rebuilt bootstrap"),
                bootstrap,
                "case={case}"
            );
            assert!(!temporary.exists(), "case={case}");
        }
    }

    #[test]
    fn committed_noncanonical_bootstrap_temp_is_retained_and_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bootstrap-committed-corrupt.norito");
        let temporary = path.with_extension("bootstrap.tmp");
        fs::write(&path, []).expect("write empty canonical");
        let mut corrupt = raw_bootstrap_frame();
        let payload_offset = usize::try_from(FRAME_HEADER_BYTES).expect("header");
        corrupt[payload_offset] ^= 0x80;
        fs::write(&temporary, &corrupt).expect("write committed corrupt bootstrap temp");

        let error = open(&path)
            .err()
            .expect("committed noncanonical bootstrap must fail closed");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(fs::read(&path).expect("retain canonical").is_empty());
        assert_eq!(
            fs::read(&temporary).expect("retain committed corrupt temp"),
            corrupt
        );
    }

    #[test]
    fn malformed_bootstrap_temp_is_retained_and_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bootstrap-malformed-temp.norito");
        let temporary = path.with_extension("bootstrap.tmp");
        fs::write(&path, []).expect("write empty canonical journal");
        let malformed = b"not-a-v4-bootstrap-longer-than-one-staged-header".to_vec();
        fs::write(&temporary, &malformed).expect("write malformed bootstrap temp");

        let error = open(&path)
            .err()
            .expect("malformed bootstrap temp must fail closed");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&path).expect("retain empty canonical"),
            Vec::<u8>::new()
        );
        assert_eq!(
            fs::read(&temporary).expect("retain malformed bootstrap evidence"),
            malformed
        );
    }

    #[test]
    fn every_first_put_prefix_after_bootstrap_repairs_to_the_marker() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("first-put-prefix-recovery.norito");
        let bootstrap = raw_bootstrap_frame();
        let first = record("first-put-prefix-recovery");
        let put = raw_frame(&QueuePlanJournalFrameV4::Put(first));

        for cut in 1..put.len() {
            let mut bytes = bootstrap.clone();
            bytes.extend_from_slice(&put[..cut]);
            fs::write(&path, bytes).expect("write torn first Put");

            let journal = open(&path)
                .unwrap_or_else(|error| panic!("repair first Put at cut {cut}: {error}"));
            assert!(
                journal
                    .replay()
                    .expect("replay repaired first Put")
                    .is_empty(),
                "cut={cut}"
            );
            drop(journal);
            assert_eq!(
                fs::read(&path).expect("read repaired journal"),
                bootstrap,
                "cut={cut}"
            );
        }
    }

    #[test]
    fn corrupt_initial_v4_version_and_length_guard_fail_without_rewrite() {
        let frame = raw_frame(&QueuePlanJournalFrameV4::Put(record(
            "corrupt-initial-v4-header",
        )));
        let header_len = usize::try_from(FRAME_HEADER_BYTES).expect("header");
        let version_offset = QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len();
        let guard_offset = version_offset + 2 + 4;
        let mut wrong_version = frame[..header_len].to_vec();
        wrong_version[version_offset] ^= 0x01;
        let mut wrong_guard = frame[..header_len].to_vec();
        wrong_guard[guard_offset] ^= 0x01;

        for (case, bytes) in [("version", wrong_version), ("length-guard", wrong_guard)] {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("corrupt-initial-{case}.norito"));
            fs::write(&path, &bytes).expect("write corrupt complete header");

            let error = open(&path)
                .err()
                .expect("corrupt complete header must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "case={case}");
            assert_eq!(
                fs::read(&path).expect("retain corrupt complete header"),
                bytes,
                "case={case}"
            );
        }
    }

    #[test]
    fn every_recognizable_terminal_v4_prefix_is_repaired_after_valid_frame() {
        let committed = record("committed-before-terminal-prefix");
        let terminal = record("terminal-prefix");
        let committed_frame = raw_frame(&QueuePlanJournalFrameV4::Put(committed.clone()));
        let terminal_frame = raw_frame(&QueuePlanJournalFrameV4::Put(terminal.clone()));
        let payload_len = terminal_frame.len()
            - usize::try_from(FRAME_HEADER_BYTES + FRAME_TRAILER_BYTES)
                .expect("constant frame overhead");
        let cuts = [
            1,
            QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len() - 1,
            QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len() + 1,
            usize::try_from(FRAME_HEADER_BYTES).expect("header") - 1,
            usize::try_from(FRAME_HEADER_BYTES).expect("header"),
            usize::try_from(FRAME_HEADER_BYTES).expect("header") + payload_len / 2,
            terminal_frame.len() - QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len() / 2,
            terminal_frame.len() - 1,
        ];

        for cut in cuts {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("prefix-{cut}.norito"));
            let mut bytes = raw_bootstrap_frame();
            bytes.extend_from_slice(&committed_frame);
            bytes.extend_from_slice(&terminal_frame[..cut]);
            fs::write(&path, bytes).expect("write terminal prefix");

            let mut journal = open(&path).expect("repair recognizable prefix");
            assert_eq!(
                path.metadata().expect("metadata").len(),
                u64::try_from(
                    raw_bootstrap_frame()
                        .len()
                        .checked_add(committed_frame.len())
                        .expect("bootstrap plus committed frame")
                )
                .expect("committed frame length"),
                "cut={cut}"
            );
            assert_eq!(
                journal.replay().expect("replay committed frame"),
                vec![committed.clone()],
                "cut={cut}"
            );
            journal
                .replace_strict_durable(terminal.clone())
                .expect("append after repair");
            assert_eq!(
                journal.replay().expect("replay"),
                vec![committed.clone(), terminal.clone()],
                "cut={cut}"
            );
        }
    }

    #[test]
    fn arbitrary_phase_one_header_tears_are_repaired_after_valid_history() {
        let committed_record = record("committed-before-garbage-header");
        let committed = raw_frame(&QueuePlanJournalFrameV4::Put(committed_record.clone()));
        let header = usize::try_from(FRAME_HEADER_BYTES).expect("header length");
        for tear_len in [1, header / 2, header.saturating_sub(1), header] {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir
                .path()
                .join(format!("terminal-garbage-header-{tear_len}.norito"));
            let mut expected = raw_bootstrap_frame();
            expected.extend_from_slice(&committed);
            let mut bytes = expected.clone();
            bytes.resize(bytes.len() + tear_len, 0xA5);
            fs::write(&path, bytes).expect("write full-length torn header");

            let journal = open(&path)
                .unwrap_or_else(|error| panic!("repair {tear_len}-byte header tear: {error}"));
            assert_eq!(
                journal.replay().expect("replay committed owner"),
                vec![committed_record.clone()],
                "tear_len={tear_len}"
            );
            drop(journal);
            assert_eq!(
                fs::read(&path).expect("read repaired journal"),
                expected,
                "tear_len={tear_len}"
            );
        }
    }

    #[test]
    fn terminal_invalid_layouts_longer_than_one_header_do_not_truncate_valid_v4_history() {
        let committed = raw_frame(&QueuePlanJournalFrameV4::Put(record(
            "committed-before-invalid-tail",
        )));
        let header = usize::try_from(FRAME_HEADER_BYTES).expect("header length");
        for (case, prefix) in [("legacy-v2", b"IRQPJNL2"), ("unknown", b"UNKNOWN!")] {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("terminal-{case}.norito"));
            let mut suffix = prefix.to_vec();
            suffix.resize(header.saturating_add(1), 0x5A);
            let mut bytes = raw_bootstrap_frame();
            bytes.extend_from_slice(&committed);
            bytes.extend_from_slice(&suffix);
            fs::write(&path, &bytes).expect("write invalid terminal prefix");

            let error = open(&path)
                .err()
                .expect("invalid terminal prefix must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "case={case}");
            assert_eq!(
                fs::read(&path).expect("retain complete journal bytes"),
                bytes,
                "case={case}"
            );
        }
    }

    #[test]
    fn truncate_file_sync_then_parent_failure_is_restart_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("truncate-parent-failure.norito");
        let frame = raw_frame(&QueuePlanJournalFrameV4::Put(record("truncate-parent")));
        let bootstrap = raw_bootstrap_frame();
        let mut bytes = bootstrap.clone();
        bytes.extend_from_slice(&frame[..frame.len() / 2]);
        fs::write(&path, bytes).expect("write recognizable torn frame");
        let mut file = open_regular_read_write(&path).expect("open torn journal");

        let bootstrap_len = u64::try_from(bootstrap.len()).expect("bootstrap length");
        let error =
            truncate_journal_tail_with_parent_sync(&mut file, bootstrap_len, &path, |_path| {
                Err(io::Error::other(
                    "injected parent failure after truncate file sync",
                ))
            })
            .expect_err("parent sync must fail after durable truncation");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(file.metadata().expect("metadata").len(), bootstrap_len);
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

        let v2_path = dir.path().join("legacy-v2.norito");
        let v2_prefix = b"IRQPJNL2";
        fs::write(&v2_path, v2_prefix).expect("write recognizable V2 prefix");
        let v2_error = open(&v2_path)
            .err()
            .expect("first-release V4 must reject V2 persistence");
        assert_eq!(v2_error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&v2_path).expect("retain legacy V2 bytes"),
            v2_prefix
        );
    }

    #[test]
    fn complete_corruption_and_unsupported_versions_fail_without_truncation() {
        let valid = raw_frame(&QueuePlanJournalFrameV4::Put(record("corrupt")));
        let payload_offset = usize::try_from(FRAME_HEADER_BYTES).expect("header");
        let cases = [
            {
                let mut bytes = valid.clone();
                bytes[payload_offset] ^= 0x80;
                ("payload", bytes)
            },
            {
                let mut bytes = valid.clone();
                let checksum_offset =
                    bytes.len() - QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len() - Hash::LENGTH;
                bytes[checksum_offset] ^= 0x80;
                ("checksum", bytes)
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
                    raw_frame(&QueuePlanJournalFrameV4::Put(unsupported)),
                )
            },
        ];

        for (label, corrupt_frame) in cases {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("{label}.norito"));
            let mut bytes = raw_bootstrap_frame();
            bytes.extend_from_slice(&corrupt_frame);
            fs::write(&path, &bytes).expect("write corrupt case");
            assert!(open(&path).is_err(), "{label} must fail closed");
            assert_eq!(fs::read(&path).expect("retain evidence"), bytes, "{label}");
        }
    }

    #[test]
    fn full_length_uncommitted_body_checksum_and_marker_tears_truncate_only_terminal_frame() {
        let committed_record = record("committed-before-full-length-tear");
        let committed = raw_frame(&QueuePlanJournalFrameV4::Put(committed_record.clone()));
        let terminal = raw_frame(&QueuePlanJournalFrameV4::Put(record(
            "full-length-uncommitted-terminal",
        )));
        let header = usize::try_from(FRAME_HEADER_BYTES).expect("header");
        let commit_start = terminal
            .len()
            .checked_sub(QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len())
            .expect("commit start");
        let checksum_start = commit_start
            .checked_sub(Hash::LENGTH)
            .expect("checksum start");
        let mut body_tear = terminal.clone();
        body_tear[header] ^= 0x80;
        body_tear[commit_start..].fill(0);

        let mut checksum_tear = terminal.clone();
        checksum_tear[checksum_start] ^= 0x80;
        checksum_tear[commit_start..].fill(0);

        let mut marker_tear = terminal;
        marker_tear[commit_start..].fill(0);
        let cases = [
            ("body", body_tear),
            ("checksum", checksum_tear),
            ("commit", marker_tear),
        ];

        for (case, torn) in cases {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("full-length-{case}-tear.norito"));
            let mut expected = raw_bootstrap_frame();
            expected.extend_from_slice(&committed);
            let mut bytes = expected.clone();
            bytes.extend_from_slice(&torn);
            fs::write(&path, bytes).expect("write full-length staged tear");

            let journal = open(&path)
                .unwrap_or_else(|error| panic!("repair full-length {case} tear: {error}"));
            assert_eq!(
                journal.replay().expect("replay committed owner"),
                vec![committed_record.clone()],
                "case={case}"
            );
            drop(journal);
            assert_eq!(
                fs::read(&path).expect("read repaired history"),
                expected,
                "case={case}"
            );
        }
    }

    #[test]
    fn invalid_commit_marker_is_never_repaired_in_the_middle_of_history() {
        let mut invalid = raw_frame(&QueuePlanJournalFrameV4::Put(record(
            "mid-history-invalid-commit",
        )));
        let commit_start = invalid
            .len()
            .checked_sub(QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len())
            .expect("commit start");
        invalid[commit_start..].fill(0);
        let following = raw_frame(&QueuePlanJournalFrameV4::Put(record(
            "after-mid-history-invalid-commit",
        )));
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("mid-history-invalid-commit.norito");
        let mut bytes = raw_bootstrap_frame();
        bytes.extend_from_slice(&invalid);
        bytes.extend_from_slice(&following);
        fs::write(&path, &bytes).expect("write invalid mid-history marker");

        let error = open(&path)
            .err()
            .expect("invalid mid-history marker must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read(&path).expect("retain corrupt history"), bytes);
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
        let mut oversized_bytes = raw_bootstrap_frame();
        oversized_bytes.extend_from_slice(&header);
        fs::write(&oversized_frame, &oversized_bytes).expect("write oversized header");
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
        assert_eq!(
            frame_decode_allocation_budget(usize::MAX),
            None,
            "allocation-budget arithmetic must fail closed instead of saturating"
        );
        let frame = QueuePlanJournalFrameV4::Put(record_with_message(
            "decode-budget",
            "x".repeat(256 * 1024),
        ));
        let payload =
            norito::encode_canonical(&frame).expect("encode large canonical frame payload");
        let payload_len = u64::try_from(payload.len()).expect("payload length fits u64");
        let exact_limits = QueuePlanJournalLimits::new(1, payload_len, TEST_MAX_BYTES, 1);
        let configured_element_budget = payload
            .len()
            .saturating_mul(FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT);
        let configured_allocation_budget =
            frame_decode_allocation_budget(payload.len()).expect("fixture allocation budget");
        let (minimum_element_budget, minimum_allocation_budget) = minimum_decode_budgets(&payload);
        assert!(
            configured_element_budget >= minimum_element_budget,
            "configured element budget {configured_element_budget} is below measured canonical minimum {minimum_element_budget}"
        );
        assert!(
            configured_element_budget.saturating_sub(minimum_element_budget) <= payload.len(),
            "configured element budget {configured_element_budget} must remain within one frame ({}) of the measured minimum {minimum_element_budget}",
            payload.len()
        );
        assert!(
            configured_allocation_budget >= minimum_allocation_budget,
            "configured allocation budget {configured_allocation_budget} is below measured canonical minimum {minimum_allocation_budget}"
        );
        assert!(
            configured_allocation_budget.saturating_sub(minimum_allocation_budget)
                <= payload
                    .len()
                    .saturating_add(FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES),
            "configured allocation budget {configured_allocation_budget} must remain within one frame plus fixed metadata overhead ({}) of the measured minimum {minimum_allocation_budget}",
            payload
                .len()
                .saturating_add(FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES)
        );

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
    fn decode_budget_covers_maximum_allocation_dense_instruction_vector() {
        const CALIBRATION_INSTRUCTION_COUNT: usize = 4_096;

        let calibration_instructions =
            std::iter::repeat_with(|| InstructionBox::from(Log::new(Level::INFO, String::new())))
                .take(CALIBRATION_INSTRUCTION_COUNT);
        let calibration_frame = QueuePlanJournalFrameV4::Put(record_with_instructions(
            "allocation-calibration",
            calibration_instructions,
        ));
        let calibration_payload = norito::encode_canonical(&calibration_frame)
            .expect("encode allocation calibration frame");
        let configured_element_budget = calibration_payload
            .len()
            .checked_mul(FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT)
            .expect("calibration element budget");
        let configured_allocation_budget =
            frame_decode_allocation_budget(calibration_payload.len())
                .expect("calibration allocation budget");
        let (minimum_element_budget, minimum_allocation_budget) =
            minimum_decode_budgets(&calibration_payload);

        assert!(
            configured_element_budget >= minimum_element_budget,
            "configured element budget {configured_element_budget} is below the allocation-dense minimum {minimum_element_budget} for {CALIBRATION_INSTRUCTION_COUNT} instructions and {} wire bytes",
            calibration_payload.len()
        );
        assert!(
            configured_allocation_budget >= minimum_allocation_budget,
            "configured allocation budget {configured_allocation_budget} is below the allocation-dense minimum {minimum_allocation_budget} for {CALIBRATION_INSTRUCTION_COUNT} instructions and {} wire bytes",
            calibration_payload.len()
        );

        let instruction_count = usize::try_from(
            iroha_config::parameters::defaults::transaction::max_instructions().get(),
        )
        .expect("default transaction instruction limit fits usize");
        assert_eq!(
            instruction_count, 100_000,
            "fixture must track the production admission maximum"
        );
        let instructions =
            std::iter::repeat_with(|| InstructionBox::from(Log::new(Level::INFO, String::new())))
                .take(instruction_count);
        let frame = QueuePlanJournalFrameV4::Put(record_with_instructions(
            "allocation-dense",
            instructions,
        ));
        let payload = norito::encode_canonical(&frame).expect("encode allocation-dense frame");
        let payload_len = u64::try_from(payload.len()).expect("payload length fits u64");
        let exact_limits = QueuePlanJournalLimits::new(
            1,
            payload_len,
            payload_len
                .checked_add(FRAME_HEADER_BYTES)
                .and_then(|bytes| bytes.checked_add(FRAME_TRAILER_BYTES))
                .expect("fixture framed length fits u64"),
            1,
        );
        assert_eq!(
            decode_frame(&payload, exact_limits)
                .expect("decode allocation-dense frame at configured limits"),
            frame
        );
    }

    #[test]
    fn compressed_frame_is_rejected_before_owned_decompression() {
        let frame = QueuePlanJournalFrameV4::Put(record_with_message(
            "compressed-frame",
            "z".repeat(2 * 1024 * 1024),
        ));
        let canonical = norito::encode_canonical(&frame).expect("encode canonical frame");
        let compressed =
            norito::to_compressed_bytes(&frame, Some(norito::CompressionConfig::default()))
                .expect("compress frame");
        assert!(
            compressed.len().saturating_mul(16) < canonical.len(),
            "fixture must have decompression amplification"
        );
        let limits = QueuePlanJournalLimits::new(
            1,
            u64::try_from(compressed.len()).expect("compressed length fits u64"),
            TEST_MAX_BYTES,
            1,
        );
        let error =
            decode_frame(&compressed, limits).expect_err("compressed journal frame must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("unsupported compression"),
            "compressed input must fail during the uncompressed archive preflight: {error}"
        );
    }

    #[test]
    fn replay_rejects_exactly_one_distinct_identity_above_live_bound() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live-bound.norito");
        {
            let mut journal =
                QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
            for label in ["one", "two"] {
                journal
                    .put_deferred_flush(record(label))
                    .expect("append bounded record");
            }
            journal.sync_all_with_parent().expect("sync");
        }

        let journal = QueuePlanJournal::open_with_limits(&path, limits(1), true).expect("reopen");
        let error = journal
            .prepare_replay()
            .err()
            .expect("max + 1 distinct identities must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains("distinct live-record reconstruction limit exceeded"),
            "unexpected bound error: {error}"
        );
    }

    #[test]
    fn replay_rejects_transient_distinct_identity_amplification_even_if_final_set_is_small() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("transient-live-prefix.norito");
        let records = (0..16)
            .map(|index| record(&format!("transient-{index}")))
            .collect::<Vec<_>>();
        {
            let mut journal =
                QueuePlanJournal::open_with_limits(&path, limits(records.len()), true)
                    .expect("open");
            for record in &records {
                journal
                    .put_deferred_flush(record.clone())
                    .expect("append transient owner");
            }
            journal
                .remove_many_deferred_flush(records[..records.len() - 1].iter().map(|record| {
                    (
                        record.entrypoint_hash,
                        record.plan_digest(),
                        record.claim_digest().expect("hash transient claim"),
                    )
                }))
                .expect("append delayed tombstones");
            journal.sync_all_with_parent().expect("sync fixture");
        }

        let journal = QueuePlanJournal::open_with_limits(&path, limits(1), true).expect("reopen");
        let error = journal
            .prepare_replay()
            .err()
            .expect("transient reconstruction amplification must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains("distinct live-record reconstruction limit exceeded"),
            "unexpected transient-bound error: {error}"
        );
    }

    #[test]
    fn replay_allows_long_put_remove_history_with_bounded_live_cardinality() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bounded-long-history.norito");
        let mut journal = QueuePlanJournal::open_with_limits(&path, limits(1), true).expect("open");
        for index in 0..64 {
            let historical = record(&format!("historical-{index}"));
            journal
                .put_deferred_flush(historical.clone())
                .expect("append historical Put");
            journal
                .remove_many_deferred_flush([(
                    historical.entrypoint_hash,
                    historical.plan_digest(),
                    historical.claim_digest().expect("hash historical claim"),
                )])
                .expect("append matching historical Remove");
        }
        let live = record("long-history-live");
        journal
            .put_deferred_flush(live.clone())
            .expect("append final live owner");
        journal.sync_all_with_parent().expect("sync history");

        assert_eq!(
            journal.replay().expect("replay bounded history"),
            vec![live]
        );
    }

    #[test]
    fn replay_same_entrypoint_replacements_do_not_grow_cardinality_and_stale_remove_is_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("same-entrypoint-replacements.norito");
        let original = record("same-entrypoint");
        let original_digest = original.plan_digest();
        let mut latest = original.clone();
        let mut journal = QueuePlanJournal::open_with_limits(&path, limits(1), true).expect("open");
        journal
            .put_deferred_flush(original.clone())
            .expect("append original plan");
        for route in 1..=128 {
            latest = with_single_route(original.clone(), route, route.saturating_add(1_000));
            journal
                .put_deferred_flush(latest.clone())
                .expect("append same-entrypoint replacement");
        }
        journal
            .remove_many_deferred_flush([(
                original.entrypoint_hash,
                original_digest,
                original.claim_digest().expect("hash original claim"),
            )])
            .expect("append stale original-plan Remove");
        journal.sync_all_with_parent().expect("sync replacements");

        assert_eq!(
            journal.replay().expect("replay latest replacement"),
            vec![latest],
            "a stale plan-specific Remove must not delete the latest plan for the same entrypoint"
        );
    }

    #[test]
    fn replacement_preserves_original_fifo_ownership_through_compaction_and_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("replacement-fifo-compaction.norito");
        let first = record("replacement-fifo-first");
        let second = record("replacement-fifo-second");
        let replacement = with_single_route(first.clone(), 17, 29);
        let compact_limits = QueuePlanJournalLimits::new(
            u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap length"),
            TEST_MAX_BYTES,
            TEST_MAX_BYTES,
            2,
        );
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, compact_limits, true).expect("open");
        for record in [&first, &second, &replacement] {
            journal
                .put_deferred_flush(record.clone())
                .expect("append FIFO fixture");
        }
        journal.sync_all_with_parent().expect("sync FIFO fixture");

        assert_eq!(
            journal.replay().expect("replay before compaction"),
            vec![replacement.clone(), second.clone()],
            "replacing A after B must preserve A's original ownership position"
        );
        journal
            .compact_if_needed()
            .expect("compact replacement history");
        assert_eq!(
            read_frames(&path, compact_limits).expect("read compacted FIFO frames"),
            vec![
                QueuePlanJournalFrameV4::Put(replacement.clone()),
                QueuePlanJournalFrameV4::Put(second.clone()),
            ]
        );
        drop(journal);

        let reopened =
            QueuePlanJournal::open_with_limits(&path, compact_limits, true).expect("reopen");
        assert_eq!(
            reopened.replay().expect("replay compacted FIFO history"),
            vec![replacement, second]
        );
    }

    #[test]
    fn matching_remove_ends_ownership_before_same_entrypoint_is_reinserted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("replacement-fifo-remove-reinsert.norito");
        let first = record("replacement-remove-first");
        let second = record("replacement-remove-second");
        let replacement = with_single_route(first.clone(), 31, 37);
        let reinserted = with_single_route(first.clone(), 41, 43);
        let mut journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
        for record in [&first, &second, &replacement] {
            journal
                .put_deferred_flush(record.clone())
                .expect("append ownership fixture");
        }
        journal
            .remove_many_deferred_flush([(
                replacement.entrypoint_hash,
                replacement.plan_digest(),
                replacement.claim_digest().expect("hash replacement claim"),
            )])
            .expect("remove latest ownership");
        journal
            .put_deferred_flush(reinserted.clone())
            .expect("reinsert same entrypoint");
        journal
            .sync_all_with_parent()
            .expect("sync ownership fixture");

        assert_eq!(
            journal.replay().expect("replay reset ownership"),
            vec![second, reinserted],
            "a Put after a matching Remove acquires a new FIFO ownership position"
        );
    }

    #[test]
    fn prepared_replay_rejects_same_length_content_tamper_before_callback() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tampered-latest-offset.norito");
        let first = record("tampered-offset-first");
        let second = record("tampered-offset-second");
        let replacement = with_single_route(first.clone(), 47, 53);
        let mut journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
        for record in [&first, &second, &replacement] {
            journal
                .put_deferred_flush(record.clone())
                .expect("append tamper fixture");
        }
        journal.sync_all_with_parent().expect("sync tamper fixture");
        let verified_replay = journal
            .prepare_replay()
            .expect("prepare owned replay snapshot");
        let callback_replay = journal
            .prepare_replay()
            .expect("prepare callback replay snapshot");

        let replacement_position = u64::try_from(
            raw_bootstrap_frame().len()
                + raw_frame(&QueuePlanJournalFrameV4::Put(first)).len()
                + raw_frame(&QueuePlanJournalFrameV4::Put(second)).len(),
        )
        .expect("replacement position fits u64");
        let payload_position = replacement_position
            .checked_add(FRAME_HEADER_BYTES)
            .expect("payload position fits u64");
        let mut tamper = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("open fixture for in-place tamper");
        tamper
            .seek(SeekFrom::Start(payload_position))
            .expect("seek payload byte");
        let mut byte = [0_u8; 1];
        tamper.read_exact(&mut byte).expect("read payload byte");
        byte[0] ^= 0x01;
        tamper
            .seek(SeekFrom::Start(payload_position))
            .expect("rewind payload byte");
        tamper.write_all(&byte).expect("tamper payload byte");
        tamper.sync_all().expect("publish in-place tamper");

        let error = verified_replay
            .into_verified_records()
            .expect_err("tampered latest frame must return no owned replay");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("snapshot content changed"),
            "unexpected owned-replay tamper error: {error}",
        );

        let mut callbacks = 0_usize;
        let error = callback_replay
            .for_each_record(|_record| {
                callbacks = callbacks.saturating_add(1);
                Ok(())
            })
            .expect_err("tampered latest frame must fail content-bound replay");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("snapshot content changed"),
            "unexpected tamper error: {error}"
        );
        assert_eq!(callbacks, 0, "tampered owner must not reach the callback");
    }

    #[test]
    fn materialized_replay_rejects_wrong_record_identity_before_callback() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("wrong-indexed-put.norito");
        let first = record("wrong-index-first");
        let second = record("wrong-index-second");
        let mut journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
        journal
            .put_deferred_flush(first.clone())
            .expect("append first");
        journal
            .put_deferred_flush(second.clone())
            .expect("append second");
        journal.sync_all_with_parent().expect("sync fixture");
        let mut replay = journal.prepare_replay().expect("prepare replay snapshot");
        replay
            .live_positions
            .get_mut(&first.entrypoint_hash)
            .expect("first live index")
            .record = second;

        let mut callbacks = 0_usize;
        let error = replay
            .for_each_record(|_record| {
                callbacks = callbacks.saturating_add(1);
                Ok(())
            })
            .expect_err("wrong materialized Put identity must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains("materialized live frame identity"),
            "unexpected materialized-identity error: {error}"
        );
        assert_eq!(
            callbacks, 0,
            "wrong materialized Put must not reach callback"
        );
    }

    #[test]
    fn materialized_replay_rejects_later_record_corruption_before_any_callback() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("wrong-later-indexed-put.norito");
        let first = record("wrong-later-index-first");
        let second = record("wrong-later-index-second");
        let second_key = second.entrypoint_hash;
        let mut journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
        journal
            .put_deferred_flush(first.clone())
            .expect("append first");
        journal.put_deferred_flush(second).expect("append second");
        journal.sync_all_with_parent().expect("sync fixture");
        let mut replay = journal.prepare_replay().expect("prepare replay snapshot");
        replay
            .live_positions
            .get_mut(&second_key)
            .expect("later live index")
            .record = first;

        let mut callbacks = 0_usize;
        let error = replay
            .for_each_record(|_record| {
                callbacks = callbacks.saturating_add(1);
                Ok(())
            })
            .expect_err("wrong later materialized Put identity must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains("materialized live frame identity"),
            "unexpected later materialized-identity error: {error}",
        );
        assert_eq!(
            callbacks, 0,
            "a valid earlier record must remain private when a later record is corrupt",
        );
    }

    #[test]
    fn materialized_replay_rejects_same_identity_and_plan_with_changed_claim() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("changed-indexed-claim.norito");
        let original = record("changed-indexed-claim");
        let key = original.entrypoint_hash;
        let mut journal = QueuePlanJournal::open_with_limits(&path, limits(1), true).expect("open");
        journal
            .put_deferred_flush(original)
            .expect("append original claim");
        journal.sync_all_with_parent().expect("sync original claim");
        let mut replay = journal.prepare_replay().expect("prepare replay snapshot");
        let materialized = &mut replay
            .live_positions
            .get_mut(&key)
            .expect("materialized claim")
            .record;
        materialized.enqueue_timestamp_ms = materialized.enqueue_timestamp_ms.saturating_add(1);

        let mut callbacks = 0_usize;
        let error = replay
            .for_each_record(|_| {
                callbacks = callbacks.saturating_add(1);
                Ok(())
            })
            .expect_err("changed materialized claim must fail replay");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("or claim changed"),
            "unexpected claim mutation error: {error}"
        );
        assert_eq!(callbacks, 0, "changed claim must not reach the callback");
    }

    #[test]
    fn prepared_replay_rejects_valid_historical_remove_rewrite_before_callback() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("changed-historical-remove.norito");
        let first = record("historical-remove-first");
        let second = record("historical-remove-second");
        let first_put = raw_frame(&QueuePlanJournalFrameV4::Put(first.clone()));
        let second_put = raw_frame(&QueuePlanJournalFrameV4::Put(second.clone()));
        let original_remove = raw_frame(&QueuePlanJournalFrameV4::Remove {
            entrypoint_hash: first.entrypoint_hash,
            plan_digest: first.plan_digest(),
            claim_digest: first.claim_digest().expect("hash first claim"),
        });
        let changed_remove = raw_frame(&QueuePlanJournalFrameV4::Remove {
            entrypoint_hash: second.entrypoint_hash,
            plan_digest: second.plan_digest(),
            claim_digest: second.claim_digest().expect("hash second claim"),
        });
        assert_eq!(
            original_remove.len(),
            changed_remove.len(),
            "fixed-size exact tombstone rewrite must preserve frame length"
        );
        let mut journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
        journal
            .put_deferred_flush(first.clone())
            .expect("append first owner");
        journal
            .put_deferred_flush(second.clone())
            .expect("append second owner");
        journal
            .remove_many_deferred_flush([(
                first.entrypoint_hash,
                first.plan_digest(),
                first.claim_digest().expect("hash first claim"),
            )])
            .expect("append original historical Remove");
        journal
            .sync_all_with_parent()
            .expect("sync historical Remove fixture");
        let replay = journal.prepare_replay().expect("prepare replay snapshot");

        let remove_position = u64::try_from(
            raw_bootstrap_frame()
                .len()
                .checked_add(first_put.len())
                .and_then(|bytes| bytes.checked_add(second_put.len()))
                .expect("historical Remove position"),
        )
        .expect("historical Remove position fits u64");
        let mut tamper = OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open historical Remove");
        tamper
            .seek(SeekFrom::Start(remove_position))
            .expect("seek historical Remove");
        tamper
            .write_all(&changed_remove)
            .expect("rewrite valid historical Remove");
        tamper.sync_all().expect("publish valid Remove rewrite");

        let mut callbacks = 0_usize;
        let error = replay
            .for_each_record(|_| {
                callbacks = callbacks.saturating_add(1);
                Ok(())
            })
            .expect_err("historical semantic rewrite must invalidate prepared replay");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("snapshot content changed"),
            "unexpected historical rewrite error: {error}"
        );
        assert_eq!(
            callbacks, 0,
            "historical semantic rewrite must fail before any callback"
        );
    }

    #[test]
    fn compaction_preserves_live_fifo_order_and_uses_v4_frames() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("compact.norito");
        let first = record("compact-first");
        let second = record("compact-second");
        let third = record("compact-third");
        let fourth = record("compact-fourth");
        let compact_limits = QueuePlanJournalLimits::new(
            u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap length"),
            TEST_MAX_BYTES,
            TEST_MAX_BYTES,
            64,
        );
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
            .remove_many_deferred_flush([(
                second.entrypoint_hash,
                second.plan_digest(),
                second.claim_digest().expect("hash second claim"),
            )])
            .expect("remove second");

        journal.compact_if_needed().expect("compact");
        journal
            .replace_strict_durable(fourth.clone())
            .expect("append through rebound post-compaction handle");

        assert_eq!(
            journal.replay().expect("replay"),
            vec![first.clone(), third.clone(), fourth.clone()]
        );
        assert_eq!(
            read_frames(&path, compact_limits).expect("read compacted frames"),
            vec![
                QueuePlanJournalFrameV4::Put(first),
                QueuePlanJournalFrameV4::Put(third),
                QueuePlanJournalFrameV4::Put(fourth),
            ]
        );
        assert!(
            fs::read(&path)
                .expect("read compacted journal bytes")
                .starts_with(&raw_bootstrap_frame()),
            "compaction must retain the exact durable V4 bootstrap as the first frame"
        );
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn compaction_failure_after_temp_creation_is_reconciled_on_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("compact-failure.norito");
        let compact_limits = QueuePlanJournalLimits::new(
            u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap length"),
            TEST_MAX_BYTES,
            TEST_MAX_BYTES,
            64,
        );
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, compact_limits, true).expect("open");
        let expected = record("compact-failure");
        journal
            .put_deferred_flush(expected.clone())
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
                .expect("restart reconciles recognized empty compaction temp")
                .replay()
                .expect("replay authoritative canonical journal"),
            vec![expected]
        );
        assert!(
            !path.with_extension("tmp").exists(),
            "reconciled unpromoted temp must be durably removed"
        );
    }

    #[test]
    fn compaction_recovery_validates_atomic_remove_batch_prefix() {
        let first = record("compact-remove-batch-first");
        let second = record("compact-remove-batch-second");
        let exact_removals = vec![
            QueuePlanJournalRemovalV4 {
                entrypoint_hash: first.entrypoint_hash.clone(),
                plan_digest: first.plan_digest(),
                claim_digest: first.claim_digest().expect("hash first claim"),
            },
            QueuePlanJournalRemovalV4 {
                entrypoint_hash: second.entrypoint_hash.clone(),
                plan_digest: second.plan_digest(),
                claim_digest: second.claim_digest().expect("hash second claim"),
            },
        ];
        let mut canonical = raw_bootstrap_frame();
        canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(first.clone())));
        canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(second.clone())));
        canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::RemoveBatch(
            exact_removals.clone(),
        )));

        let valid_dir = tempfile::tempdir().expect("valid tempdir");
        let valid_path = valid_dir.path().join("compact-remove-batch-valid.norito");
        fs::write(&valid_path, &canonical).expect("write valid canonical");
        fs::write(valid_path.with_extension("tmp"), raw_bootstrap_frame())
            .expect("write valid compaction prefix");
        let journal = QueuePlanJournal::open_with_limits(&valid_path, limits(2), true)
            .expect("valid atomic batch must reconcile compaction recovery");
        assert!(
            journal
                .replay()
                .expect("replay valid atomic batch")
                .is_empty()
        );
        assert!(!valid_path.with_extension("tmp").exists());

        let absent = record("compact-remove-batch-absent");
        let mut invalid_removals = exact_removals;
        invalid_removals[1] = QueuePlanJournalRemovalV4 {
            entrypoint_hash: absent.entrypoint_hash.clone(),
            plan_digest: absent.plan_digest(),
            claim_digest: absent.claim_digest().expect("hash absent claim"),
        };
        let mut invalid_canonical = raw_bootstrap_frame();
        invalid_canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(first)));
        invalid_canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(second)));
        invalid_canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::RemoveBatch(
            invalid_removals,
        )));
        let invalid_dir = tempfile::tempdir().expect("invalid tempdir");
        let invalid_path = invalid_dir
            .path()
            .join("compact-remove-batch-invalid.norito");
        fs::write(&invalid_path, &invalid_canonical).expect("write invalid canonical");
        fs::write(invalid_path.with_extension("tmp"), raw_bootstrap_frame())
            .expect("write invalid compaction prefix");

        let error = QueuePlanJournal::open_with_limits(&invalid_path, limits(2), true)
            .err()
            .expect("compaction recovery must reject a partially matching atomic batch");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains("compaction recovery RemoveBatch does not match"),
            "unexpected recovery error: {error}"
        );
        assert_eq!(
            fs::read(&invalid_path).expect("retain invalid canonical"),
            invalid_canonical
        );
        assert!(invalid_path.with_extension("tmp").is_file());
    }

    #[test]
    fn recognized_compaction_prefixes_are_reconciled_against_canonical_state() {
        let first = record("compact-prefix-first");
        let second = record("compact-prefix-second");
        let replacement = with_single_route(first.clone(), 71, 73);
        let mut canonical = raw_bootstrap_frame();
        canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(first)));
        canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(second.clone())));
        canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(
            replacement.clone(),
        )));
        let mut compacted = raw_bootstrap_frame();
        compacted.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(
            replacement.clone(),
        )));
        let second_position = compacted.len();
        compacted.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(second.clone())));
        let header = usize::try_from(FRAME_HEADER_BYTES).expect("frame header");
        let bootstrap_len = raw_bootstrap_frame().len();
        let mut cuts = vec![
            0,
            1,
            header.saturating_sub(1),
            header,
            bootstrap_len.saturating_sub(1),
            bootstrap_len,
            bootstrap_len.saturating_add(1),
            second_position.saturating_sub(1),
            second_position,
            second_position.saturating_add(1),
            compacted.len().saturating_sub(1),
            compacted.len(),
        ];
        cuts.sort_unstable();
        cuts.dedup();

        for cut in cuts {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("compact-prefix-{cut}.norito"));
            fs::write(&path, &canonical).expect("write canonical history");
            fs::write(path.with_extension("tmp"), &compacted[..cut])
                .expect("write recognized compaction prefix");

            let journal = QueuePlanJournal::open_with_limits(&path, limits(2), true)
                .unwrap_or_else(|error| {
                    panic!("reconcile compaction prefix at cut {cut}: {error}")
                });
            assert_eq!(
                journal.replay().expect("replay canonical after recovery"),
                vec![replacement.clone(), second.clone()],
                "cut={cut}"
            );
            drop(journal);
            assert_eq!(
                fs::read(&path).expect("retain canonical history"),
                canonical,
                "unpromoted compaction recovery must not replace the authoritative canonical, cut={cut}"
            );
            assert!(!path.with_extension("tmp").exists(), "cut={cut}");
        }
    }

    #[test]
    fn staged_compaction_temp_tears_are_durably_discarded_against_canonical_state() {
        let expected_record = record("compact-staged-tear");
        let bootstrap = raw_bootstrap_frame();
        let put = raw_frame(&QueuePlanJournalFrameV4::Put(expected_record.clone()));
        let mut canonical = bootstrap.clone();
        canonical.extend_from_slice(&put);
        let header = usize::try_from(FRAME_HEADER_BYTES).expect("header");
        let bootstrap_commit = bootstrap
            .len()
            .checked_sub(QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len())
            .expect("bootstrap commit");
        let mut bootstrap_body = bootstrap.clone();
        bootstrap_body[header] ^= 0x80;
        bootstrap_body[bootstrap_commit..].fill(0);

        let put_commit = put
            .len()
            .checked_sub(QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len())
            .expect("Put commit");
        let put_checksum = put_commit.checked_sub(Hash::LENGTH).expect("Put checksum");

        let mut put_body = put.clone();
        put_body[header] ^= 0x80;
        put_body[put_commit..].fill(0);
        let mut full_put_body = bootstrap.clone();
        full_put_body.extend_from_slice(&put_body);

        let mut put_checksum_tear = put.clone();
        put_checksum_tear[put_checksum] ^= 0x80;
        put_checksum_tear[put_commit..].fill(0);
        let mut full_put_checksum = bootstrap.clone();
        full_put_checksum.extend_from_slice(&put_checksum_tear);

        let mut put_commit_tear = put;
        put_commit_tear[put_commit..].fill(0);
        let mut full_put_commit = bootstrap;
        full_put_commit.extend_from_slice(&put_commit_tear);
        let cases = [
            ("bootstrap-header", vec![0xA5_u8; header]),
            ("bootstrap-body", bootstrap_body),
            ("put-body", full_put_body),
            ("put-checksum", full_put_checksum),
            ("put-commit", full_put_commit),
        ];

        for (case, temporary_bytes) in cases {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("compact-staged-{case}.norito"));
            let temporary = path.with_extension("tmp");
            fs::write(&path, &canonical).expect("write canonical history");
            fs::write(&temporary, temporary_bytes).expect("write staged compaction tear");

            let journal = QueuePlanJournal::open_with_limits(&path, limits(1), true)
                .unwrap_or_else(|error| panic!("reconcile staged {case} tear: {error}"));
            assert_eq!(
                journal.replay().expect("replay canonical"),
                vec![expected_record.clone()],
                "case={case}"
            );
            drop(journal);
            assert_eq!(
                fs::read(&path).expect("retain canonical bytes"),
                canonical,
                "case={case}"
            );
            assert!(!temporary.exists(), "case={case}");
        }
    }

    #[test]
    fn committed_corrupt_compaction_temp_is_retained_and_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("compact-unexpected-v4.norito");
        let canonical_record = record("compact-expected");
        let mut canonical = raw_bootstrap_frame();
        canonical.extend_from_slice(&raw_frame(&QueuePlanJournalFrameV4::Put(canonical_record)));
        let mut unexpected = canonical.clone();
        let put_payload_offset = raw_bootstrap_frame()
            .len()
            .checked_add(usize::try_from(FRAME_HEADER_BYTES).expect("header"))
            .expect("Put payload offset");
        unexpected[put_payload_offset] ^= 0x80;
        fs::write(&path, &canonical).expect("write canonical");
        fs::write(path.with_extension("tmp"), &unexpected).expect("write unexpected V4 temp");

        let error = QueuePlanJournal::open_with_limits(&path, limits(1), true)
            .err()
            .expect("unrelated canonical V4 temp must fail closed");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains("committed compaction frame differs from deterministic output"),
            "unexpected recovery error: {error}"
        );
        assert_eq!(fs::read(&path).expect("retain canonical"), canonical);
        assert_eq!(
            fs::read(path.with_extension("tmp")).expect("retain unexpected temp"),
            unexpected
        );
    }

    #[test]
    fn orphaned_compaction_prefixes_cannot_recreate_a_missing_canonical_path() {
        let first = record("compact-orphaned-first");
        let second = record("compact-orphaned-second");
        let bootstrap = raw_bootstrap_frame();
        let first_put = raw_frame(&QueuePlanJournalFrameV4::Put(first));
        let second_put = raw_frame(&QueuePlanJournalFrameV4::Put(second));
        let mut first_of_two = bootstrap.clone();
        first_of_two.extend_from_slice(&first_put);
        let mut apparently_complete = first_of_two.clone();
        apparently_complete.extend_from_slice(&second_put);
        let partial_bootstrap = bootstrap[..bootstrap.len() - 1].to_vec();

        for (case, orphaned) in [
            ("partial-bootstrap", partial_bootstrap),
            ("bootstrap-only", bootstrap),
            ("first-of-two", first_of_two),
            ("apparently-complete", apparently_complete),
        ] {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(format!("compact-orphaned-{case}.norito"));
            fs::write(path.with_extension("tmp"), &orphaned)
                .expect("write orphaned compaction prefix");

            let error = QueuePlanJournal::open_with_limits(&path, limits(2), true)
                .err()
                .expect("orphaned replacement cannot prove completeness");

            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "case={case}");
            assert!(!path.exists(), "case={case}");
            assert_eq!(
                fs::read(path.with_extension("tmp")).expect("retain orphaned evidence"),
                orphaned,
                "case={case}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn compaction_recovery_rejects_temp_path_identity_swap_without_unlinking_replacement() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("compact-temp-swap.norito");
        let temporary = path.with_extension("tmp");
        let displaced = path.with_extension("tmp.displaced");
        let expected = record("compact-temp-swap");
        let mut journal = open(&path).expect("create canonical");
        journal
            .put_deferred_flush(expected)
            .expect("append canonical owner");
        journal
            .sync_all_with_parent()
            .expect("sync canonical owner");
        drop(journal);
        fs::write(&temporary, raw_bootstrap_frame()).expect("write recognized temp prefix");
        let pending = open_pending_compaction_temp(&temporary, limits(1))
            .expect("open pending temp")
            .expect("pending temp exists");

        fs::rename(&temporary, &displaced).expect("displace verified temp pathname");
        let replacement = b"must-not-be-unlinked".to_vec();
        fs::write(&temporary, &replacement).expect("install distinct temp pathname");
        let error = reconcile_pending_compaction_temp(&path, limits(1), pending)
            .expect_err("temp identity swap must fail before unlink");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(&temporary).expect("retain replacement pathname"),
            replacement
        );
        assert_eq!(
            fs::read(&displaced).expect("retain originally verified temp"),
            raw_bootstrap_frame()
        );
    }

    #[test]
    fn compaction_rename_then_parent_failure_recovers_replacement_on_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("compact-post-rename.norito");
        let compact_limits = QueuePlanJournalLimits::new(
            u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap length"),
            TEST_MAX_BYTES,
            TEST_MAX_BYTES,
            64,
        );
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
            .remove_many_deferred_flush([(
                removed.entrypoint_hash,
                removed.plan_digest(),
                removed.claim_digest().expect("hash removed claim"),
            )])
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
    fn cached_append_handle_rejects_atomic_path_replacement_without_split_brain() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bound.norito");
        let displaced = dir.path().join("bound.displaced");
        let original = record("bound-original");
        let stale_append = record("bound-stale-append");
        let fresh_append = record("bound-fresh-append");
        let mut stale = open(&path).expect("open original journal");
        stale
            .replace_strict_durable(original.clone())
            .expect("seed original journal");

        fs::rename(&path, &displaced).expect("atomically displace journal pathname");
        fs::write(&path, []).expect("install distinct journal pathname");
        let displaced_before = fs::read(&displaced).expect("read displaced journal");
        let mut fresh = open(&path).expect("open replacement pathname concurrently");

        let error = stale
            .replace_strict_durable(stale_append)
            .expect_err("stale append handle must reject replaced pathname");
        assert!(error.is_indeterminate());
        assert!(error.journal_faulted());
        assert!(stale.is_poisoned());
        assert_eq!(
            fs::read(&displaced).expect("read displaced journal after rejection"),
            displaced_before,
            "the stale inode must receive no acknowledged append"
        );
        assert!(
            fresh.replay().expect("replay fresh journal").is_empty(),
            "the newly bound journal must not inherit stale-inode bytes"
        );

        fresh
            .replace_strict_durable(fresh_append.clone())
            .expect("fresh bound handle remains writable");
        assert_eq!(
            fresh.replay().expect("replay fresh append"),
            vec![fresh_append]
        );
        drop(stale);
        assert_eq!(
            open(&displaced)
                .expect("open displaced journal directly")
                .replay()
                .expect("replay displaced original"),
            vec![original]
        );
    }

    #[test]
    fn second_same_inode_handle_rejects_unobserved_length_change() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("two-handles.norito");
        let first_record = record("two-handles-first");
        let mut first = open(&path).expect("open first handle");
        let mut stale_second = open(&path).expect("open second handle at same length");

        first
            .replace_strict_durable(first_record.clone())
            .expect("first handle appends");
        let error = stale_second
            .replace_strict_durable(record("two-handles-rejected"))
            .expect_err("second handle must not append across an unobserved length change");

        assert!(error.is_indeterminate());
        assert!(stale_second.is_poisoned());
        assert_eq!(
            first.replay().expect("replay first handle"),
            vec![first_record]
        );
    }

    #[cfg(unix)]
    #[test]
    fn cached_append_handle_rejects_post_open_hardlink_count_drift() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("hardlink-drift.norito");
        let alias = dir.path().join("hardlink-drift.alias");
        let original = record("hardlink-drift-original");
        let mut journal = open(&path).expect("open journal");
        journal
            .replace_strict_durable(original.clone())
            .expect("seed journal");
        let original_bytes = fs::read(&path).expect("read original bytes");
        fs::hard_link(&path, &alias).expect("add second filesystem link");

        let error = journal
            .replace_strict_durable(record("hardlink-drift-rejected"))
            .expect_err("link-count drift must fail closed before append");
        assert!(error.is_indeterminate());
        assert!(journal.is_poisoned());
        assert_eq!(
            fs::read(&path).expect("read rejected journal"),
            original_bytes
        );

        drop(journal);
        fs::remove_file(&alias).expect("remove adversarial hardlink");
        assert_eq!(
            open(&path)
                .expect("reopen single-link journal")
                .replay()
                .expect("replay original"),
            vec![original]
        );
    }

    #[cfg(unix)]
    #[test]
    fn cached_parent_handle_rejects_directory_replacement_before_sync() {
        let dir = tempfile::tempdir().expect("tempdir");
        let live_parent = dir.path().join("live");
        let displaced_parent = dir.path().join("live.displaced");
        fs::create_dir(&live_parent).expect("create live parent");
        let path = live_parent.join("queue.norito");
        let original = record("parent-original");
        let mut journal = open(&path).expect("open parent-bound journal");
        journal
            .replace_strict_durable(original.clone())
            .expect("seed parent-bound journal");

        fs::rename(&live_parent, &displaced_parent).expect("displace parent directory");
        fs::create_dir(&live_parent).expect("install distinct parent directory");
        fs::write(&path, []).expect("install distinct journal in replacement parent");

        let error = journal
            .sync_data_verified()
            .expect_err("cached parent identity drift must reject synchronization");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(journal.is_poisoned());
        drop(journal);
        assert_eq!(
            open(&displaced_parent.join("queue.norito"))
                .expect("open original journal through displaced parent")
                .replay()
                .expect("replay original parent-bound journal"),
            vec![original]
        );
        assert!(
            open(&path)
                .expect("open replacement-parent journal")
                .replay()
                .expect("replay replacement-parent journal")
                .is_empty()
        );
    }

    #[cfg(unix)]
    #[test]
    fn prepared_replay_rejects_path_replacement_before_streaming() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("replay-bound.norito");
        let displaced = dir.path().join("replay-bound.displaced");
        let expected = record("replay-bound");
        let mut journal = open(&path).expect("open journal");
        journal
            .replace_strict_durable(expected)
            .expect("seed replay snapshot");
        let replay = journal.prepare_replay().expect("prepare bound replay");

        fs::rename(&path, &displaced).expect("displace replay pathname");
        fs::write(&path, []).expect("install replacement replay pathname");
        let mut callbacks = 0_usize;
        let error = replay
            .for_each_record(|_| {
                callbacks = callbacks.saturating_add(1);
                Ok(())
            })
            .expect_err("prepared replay must reject a different path identity");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            callbacks, 0,
            "path replacement must fail before any callback",
        );
    }

    #[test]
    fn prepared_replay_rejects_snapshot_length_extension_before_streaming() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("replay-length-bound.norito");
        let expected = record("replay-length-bound");
        let appended = record("replay-length-extension");
        let mut journal = open(&path).expect("open journal");
        journal
            .replace_strict_durable(expected)
            .expect("seed replay snapshot");
        let replay = journal.prepare_replay().expect("prepare bound replay");

        let mut concurrent = OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open concurrent append handle");
        concurrent
            .write_all(&raw_frame(&QueuePlanJournalFrameV4::Put(appended)))
            .expect("extend replay snapshot");
        concurrent.sync_all().expect("publish extension");

        let mut callbacks = 0_usize;
        let error = replay
            .for_each_record(|_| {
                callbacks = callbacks.saturating_add(1);
                Ok(())
            })
            .expect_err("prepared replay must reject a changed snapshot length");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(callbacks, 0, "length drift must fail before streaming");
    }

    #[test]
    fn nested_parent_creation_is_restart_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("new")
            .join("nested")
            .join("journal")
            .join("queue.norito");
        let expected = record("nested-parent-durability");

        let mut journal = open(&path).expect("create nested durable journal parent");
        journal
            .replace_strict_durable(expected.clone())
            .expect("persist nested-parent owner");
        drop(journal);

        assert!(path.parent().expect("journal parent").is_dir());
        assert_eq!(
            open(&path)
                .expect("restart through existing nested parent")
                .replay()
                .expect("replay nested-parent owner"),
            vec![expected]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_or_hardlinked_journal_and_untrusted_compaction_temp_are_rejected() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("target.norito");
        fs::write(&target, []).expect("target");
        let linked = dir.path().join("linked.norito");
        symlink(&target, &linked).expect("symlink");
        assert!(open(&linked).is_err());

        let real_parent = dir.path().join("real-parent");
        fs::create_dir(&real_parent).expect("create real parent");
        let indirect_parent = dir.path().join("indirect-parent");
        symlink(&real_parent, &indirect_parent).expect("symlink parent component");
        let indirect_path = indirect_parent.join("nested").join("queue.norito");
        assert_eq!(
            open(&indirect_path)
                .err()
                .expect("indirect parent component must fail")
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert!(
            !real_parent.join("nested").exists(),
            "parent-chain rejection must precede directory creation"
        );

        let hardlinked = dir.path().join("hardlinked.norito");
        fs::hard_link(&target, &hardlinked).expect("hardlink");
        let hardlink_error = open(&hardlinked).err().expect("hardlink must fail closed");
        assert_eq!(hardlink_error.kind(), io::ErrorKind::InvalidData);
        assert!(
            hardlink_error
                .to_string()
                .contains("exactly one filesystem link")
        );

        let path = dir.path().join("stale-temp.norito");
        fs::write(&path, []).expect("journal");
        fs::write(path.with_extension("tmp"), b"stale").expect("temp");
        assert_eq!(
            open(&path).err().expect("stale temp").kind(),
            io::ErrorKind::InvalidData
        );

        let symlink_temp_path = dir.path().join("symlink-temp.norito");
        let symlink_temp = symlink_temp_path.with_extension("tmp");
        symlink(&target, &symlink_temp).expect("temp symlink");
        assert_eq!(
            open(&symlink_temp_path).err().expect("symlink temp").kind(),
            io::ErrorKind::InvalidData
        );
        assert!(
            !symlink_temp_path.exists(),
            "temp rejection must occur before creating a new journal"
        );

        let hardlink_temp_path = dir.path().join("hardlink-temp.norito");
        let hardlink_temp = hardlink_temp_path.with_extension("tmp");
        fs::hard_link(&target, &hardlink_temp).expect("hardlinked temp");
        assert_eq!(
            open(&hardlink_temp_path)
                .err()
                .expect("hardlinked temp")
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert!(
            !hardlink_temp_path.exists(),
            "hardlink temp rejection must occur before creating a new journal"
        );

        let oversized_temp_path = dir.path().join("oversized-temp.norito");
        let oversized_temp = oversized_temp_path.with_extension("tmp");
        let oversized = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&oversized_temp)
            .expect("create oversized temp");
        oversized
            .set_len(TEST_MAX_BYTES + 1)
            .expect("extend oversized temp");
        drop(oversized);
        assert_eq!(
            open(&oversized_temp_path)
                .err()
                .expect("oversized temp")
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert!(
            !oversized_temp_path.exists(),
            "oversized temp rejection must occur before creating a new journal"
        );
    }
}
