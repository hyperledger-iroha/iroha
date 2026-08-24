//! Crash-safe local journal for lane-owned queue reservations.
//!
//! A reservation is local scheduling state rather than consensus state, but losing it can make
//! the same transaction eligible for both the global scheduler and an independently ticking lane.
//! The journal therefore uses checksummed, length-delimited frames and synchronizes every state
//! transition before the queue exposes it to callers. The first-release admission-bound layout is V6
//! only: its bootstrap digest binds the exact no-prune operation schema, and earlier or unknown
//! frame envelopes are retained and rejected without legacy decoding.
#[cfg(test)]
use super::LaneQueueReservationRecoveryPhaseV1;
use super::{
    LaneQueueFifoOrderV5, LaneQueueReservationKeyV2, LaneQueueReservationOwnerPhaseV6,
    LaneQueueReservationReconciliationSnapshotV1, LaneQueueReservationRecordV5,
    LaneQueueReservationReleaseBarrierV3, LaneQueueReservationReleaseCompletionV5,
    QueuePlanReservationPhaseV1,
};
use crate::secure_file_metadata::{self, SecureMetadata};
use crate::sumeragi::v2_core::{
    CanonicalIdentityProjection, CheckedProductionTransition, IDENTITY_DOMAIN_DURABLE_ARTIFACT,
    IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER, IDENTITY_KIND_LANE_QUEUE_RESERVATION,
    IN_FLIGHT_RESERVATION_ACTION_COMMIT, IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE,
    IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT, IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE,
    IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE, IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,
    IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT, IN_FLIGHT_RESERVATION_ACTION_RESERVE,
    IN_FLIGHT_RESERVATION_STATE_ABSENT, IN_FLIGHT_RESERVATION_STATE_COMMITTED,
    IN_FLIGHT_RESERVATION_STATE_LIVE, IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED,
    IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED, ProductionInFlightReservationOwnerProjection,
    ProductionInFlightReservationTransitionProjection,
    check_production_in_flight_reservation_transition,
};
use iroha_crypto::{Hash, HashOf, sha256_reader_bounded};
use iroha_data_model::{
    merge::MAX_MERGE_EXECUTION_ENTRYPOINTS, nexus::LaneId, transaction::TransactionEntrypoint,
};
use norito::codec::{Decode, Encode};
#[cfg(test)]
use std::sync::Barrier;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
const RESERVATION_JOURNAL_FRAME_DOMAIN: &[u8] = b"iroha:queue-lane-reservation-frame:v6";
const RESERVATION_JOURNAL_BOOTSTRAP_DOMAIN: &[u8] = b"iroha:queue-lane-reservation-bootstrap:v6";
const RESERVATION_JOURNAL_OPERATION_SCHEMA_V6: &[u8] =
    b"iroha:queue-lane-reservation-operations:v6:plan-tombstoned";
const RESERVATION_JOURNAL_FRAME_MAGIC: [u8; 8] = *b"IRQRJNL6";
const RESERVATION_JOURNAL_FRAME_COMMIT: [u8; 8] = *b"IRQRDONE";
const RESERVATION_JOURNAL_FRAME_FORMAT_VERSION: u16 = 6;
const FRAME_HEADER_BYTES: u64 = 8 + 2 + 4 + 4;
const FRAME_TRAILER_BYTES: u64 = Hash::LENGTH as u64 + 8;
const FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT: usize = 1;
const FRAME_DECODE_ALLOCATION_AMPLIFICATION_LIMIT: usize = 26;
const FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 64 * 1024;
const CHECKED_STATE_EMPTY_DOMAIN: &[u8] = b"iroha:queue-lane-reservation-checked-state-empty:v1\0";
const CHECKED_STATE_STEP_DOMAIN: &[u8] = b"iroha:queue-lane-reservation-checked-state-step:v1\0";
const CHECKED_IDENTITY_PROJECTION_DOMAIN: &[u8] =
    b"iroha:queue-lane-reservation-checked-identity-projection:v1\0";
const CHECKED_OWNER_PROJECTION_DOMAIN: &[u8] =
    b"iroha:queue-lane-reservation-checked-owner-projection:v1\0";
const CHECKED_TRANSITION_PROJECTION_DOMAIN: &[u8] =
    b"iroha:queue-lane-reservation-checked-transition-projection:v1\0";
const CHECKED_TRANSITION_COVERAGE_EMPTY_DOMAIN: &[u8] =
    b"iroha:queue-lane-reservation-checked-transition-coverage-empty:v1\0";
const CHECKED_TRANSITION_COVERAGE_STEP_DOMAIN: &[u8] =
    b"iroha:queue-lane-reservation-checked-transition-coverage-step:v1\0";
const CHECKED_TRANSITION_COVERAGE_FINAL_DOMAIN: &[u8] =
    b"iroha:queue-lane-reservation-checked-transition-coverage-final:v1\0";
const SNAPSHOT_REPLAY_FILE_CONTENT_DOMAIN: &[u8] =
    b"iroha:queue-lane-reservation-snapshot-replay-file-content:v1\0";
const SNAPSHOT_RECONCILIATION_RECORD_DOMAIN: &[u8] =
    b"iroha:queue-lane-reservation-snapshot-reconciliation-record:v1\0";
const SNAPSHOT_RECONCILIATION_EMPTY_DOMAIN: &[u8] =
    b"iroha:queue-lane-reservation-snapshot-reconciliation-empty:v1\0";
const SNAPSHOT_RECONCILIATION_STEP_DOMAIN: &[u8] =
    b"iroha:queue-lane-reservation-snapshot-reconciliation-step:v1\0";
const SNAPSHOT_RECONCILIATION_FINAL_DOMAIN: &[u8] =
    b"iroha:queue-lane-reservation-snapshot-reconciliation-final:v1\0";
/// Version of the unchanged reservation record/FIFO/completion payloads nested in V6 frames.
pub const LANE_QUEUE_RESERVATION_JOURNAL_VERSION: u16 = 5;
/// Explicit resource limits for reservation-journal append and startup replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LaneQueueReservationJournalLimits {
    /// File size at which compaction should be considered.
    pub(super) max_bytes_before_compact: u64,
    /// Maximum canonical Norito payload bytes in one frame.
    pub(super) max_frame_payload_bytes: u64,
    /// Maximum total file bytes accepted or appended.
    pub(super) max_file_bytes: u64,
    /// Maximum queue-owned transaction identities retained at any replay prefix.
    pub(super) max_owned_transactions: usize,
}
impl LaneQueueReservationJournalLimits {
    /// Construct explicit reservation-journal limits.
    pub(super) const fn new(
        max_bytes_before_compact: u64,
        max_frame_payload_bytes: u64,
        max_file_bytes: u64,
        max_owned_transactions: usize,
    ) -> Self {
        Self {
            max_bytes_before_compact,
            max_frame_payload_bytes,
            max_file_bytes,
            max_owned_transactions,
        }
    }
    fn validate(self) -> io::Result<Self> {
        if self.max_bytes_before_compact == 0 {
            return Err(invalid_input(
                "lane reservation journal compaction threshold must be nonzero",
            ));
        }
        if self.max_frame_payload_bytes == 0 || self.max_frame_payload_bytes > u64::from(u32::MAX) {
            return Err(invalid_input(
                "lane reservation journal frame payload limit must be in 1..=u32::MAX",
            ));
        }
        if self.max_owned_transactions == 0 {
            return Err(invalid_input(
                "lane reservation journal ownership limit must be nonzero",
            ));
        }
        if self.max_bytes_before_compact > self.max_file_bytes {
            return Err(invalid_input(
                "lane reservation journal compaction threshold exceeds its file limit",
            ));
        }
        let bootstrap_payload = norito::encode_canonical(&bootstrap_frame()).map_err(|error| {
            invalid_input(format!(
                "lane reservation journal bootstrap cannot be encoded: {error}"
            ))
        })?;
        let bootstrap_payload_bytes = u64::try_from(bootstrap_payload.len())
            .map_err(|_| invalid_input("lane reservation journal bootstrap exceeds u64"))?;
        if bootstrap_payload_bytes == 0 || bootstrap_payload_bytes > self.max_frame_payload_bytes {
            return Err(invalid_input(
                "lane reservation journal frame limit cannot hold the V6 bootstrap payload",
            ));
        }
        let bootstrap_frame_bytes = FRAME_HEADER_BYTES
            .checked_add(bootstrap_payload_bytes)
            .and_then(|bytes| bytes.checked_add(FRAME_TRAILER_BYTES))
            .ok_or_else(|| invalid_input("lane reservation journal bootstrap size overflow"))?;
        if bootstrap_frame_bytes > self.max_file_bytes {
            return Err(invalid_input(
                "lane reservation journal file limit cannot hold the V6 bootstrap frame",
            ));
        }
        Ok(self)
    }
}
/// Test-only durability boundary injected into the next append.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReservationJournalAppendFault {
    /// Persist only a prefix of the encoded frame, as if the write failed partway through.
    PartialWrite,
    /// Persist the complete frame, then report the same ambiguity as a failed `sync_all`.
    SyncAfterFullWrite,
    /// Persist and synchronize the complete frame, then fail checked replay publication.
    AfterSyncBeforeReplayPublication,
}
/// Test-only durability boundary injected into the next compaction.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReservationJournalCompactionFault {
    /// Replace the journal inode, then fail before the parent-directory sync is acknowledged.
    AfterRenameBeforeParentSync,
    /// Synchronize the replacement, then fail checked replay publication.
    AfterSyncBeforeReplayPublication,
}
/// One append-only reservation journal operation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
enum LaneQueueReservationJournalFrameV6 {
    /// Typed file marker. Every initialized V6 journal begins with exactly this frame.
    Bootstrap {
        /// Exact first-release persistence format version.
        version: u16,
        /// Domain-separated identity of the V6 envelope and operation schema.
        format_digest: Hash,
    },
    /// Complete compacted state; only emitted into a newly rewritten journal.
    Snapshot {
        /// Reservations that still own queue transactions.
        live: Vec<LaneQueueReservationRecordV5>,
        /// Exact commits retained until the pending-plan tombstone and marker are durable.
        committed: Vec<LaneQueueReservationKeyV2>,
        /// Exact committed subset whose V4 QueuePlan tombstone crossed its durable sync boundary.
        plan_tombstoned: Vec<LaneQueueReservationKeyV2>,
        /// Ordered release claims prepared against exact live reservations.
        release_barriers: Vec<LaneQueueReservationReleaseBarrierV3>,
        /// Completed releases retained until FIFO restoration is acknowledged.
        completed_releases: Vec<LaneQueueReservationReleaseCompletionV5>,
    },
    /// Atomically install one or more live reservations.
    PutBatch(Vec<LaneQueueReservationRecordV5>),
    /// Atomically release one or more exact reservations back to normal queue ownership.
    ReleaseBatch(Vec<LaneQueueReservationKeyV2>),
    /// Permanently consume one exact reservation.
    Commit(LaneQueueReservationKeyV2),
    /// Record that the exact V4 QueuePlan tombstone is independently durable.
    PlanTombstoned(LaneQueueReservationKeyV2),
    /// Forget a marked commit barrier after both journals crossed their durability boundaries.
    ForgetCommit(LaneQueueReservationKeyV2),
    /// Durably claim an exact FIFO-ordered live reservation set for release.
    PrepareRelease(LaneQueueReservationReleaseBarrierV3),
    /// Atomically move the exact prepared live records into restartable completion state.
    CompleteRelease(LaneQueueReservationReleaseCompletionV5),
    /// Forget only the completion bound to this exact release identity.
    ForgetRelease(LaneQueueReservationReleaseBarrierV3),
}
/// Replayed live reservation set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct LaneQueueReservationReplay {
    records: Vec<LaneQueueReservationRecordV5>,
    committed: Vec<LaneQueueReservationKeyV2>,
    plan_tombstoned: Vec<LaneQueueReservationKeyV2>,
    release_barriers: Vec<LaneQueueReservationReleaseBarrierV3>,
    completed_releases: Vec<LaneQueueReservationReleaseCompletionV5>,
}
/// Non-authorizing identity retained after an exact startup snapshot replay.
///
/// This receipt identifies the canonical snapshot frame, its complete ordered
/// primitive-owner coverage, and the resulting checked replay state. It can be
/// copied into later reconciliation evidence, but only the move-only
/// [`LaneReservationSnapshotReplaySeal`] authorizes the initial Queue
/// publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LaneReservationSnapshotReplayReceipt {
    frame_digest: Hash,
    owner_transition_count: usize,
    owner_transition_coverage_identity: Hash,
    canonical_reconciliation_identity: Hash,
    replay_state_identity: Hash,
}
impl LaneReservationSnapshotReplayReceipt {
    /// Return the number of exact durable owners covered by primitive replay.
    #[must_use]
    pub(crate) const fn owner_transition_count(&self) -> usize {
        self.owner_transition_count
    }
    /// Recompute the exact canonical owner/state/record/release coverage from
    /// one Queue reconciliation snapshot.
    pub(crate) fn binds_reconciliation_snapshot(
        &self,
        snapshot: &LaneQueueReservationReconciliationSnapshotV1,
    ) -> io::Result<bool> {
        let owners = canonical_reconciliation_owners_from_snapshot(snapshot)?;
        let projections = owners
            .values()
            .map(|owner| recover_snapshot_transition_projection(owner.ownership));
        let owner_transition_coverage_identity =
            transition_projection_coverage_identity(projections)?;
        Ok(owners.len() == self.owner_transition_count
            && owner_transition_coverage_identity == self.owner_transition_coverage_identity
            && canonical_reconciliation_identity(&owners)?
                == self.canonical_reconciliation_identity)
    }
}
/// Move-only checked transition result before it is bound to exact storage.
#[must_use = "checked snapshot replay must be bound to its exact journal before publication"]
struct CheckedSnapshotReplayTransitionSeal {
    receipt: LaneReservationSnapshotReplayReceipt,
    authorization_domain: Arc<()>,
    replay_shape: CheckedReplayStateShape,
    transition_generation: u64,
    maximum_owned_transactions: usize,
}
/// Move-only authorization to publish one exact journal replay into Queue.
///
/// The seal is process-local and never serialized. Besides the checked
/// primitive replay identity, it binds the locked journal inode, metadata
/// revision, byte length, and complete content digest observed by `open`.
#[must_use = "snapshot replay authorization must reach the Queue install boundary"]
pub(super) struct LaneReservationSnapshotReplaySeal {
    transition: CheckedSnapshotReplayTransitionSeal,
    file_identity: JournalFileIdentity,
    file_revision: JournalFileRevision,
    known_len: u64,
    file_content_identity: Hash,
}
impl LaneQueueReservationReplay {
    /// Borrow replayed live records.
    pub(super) fn records(&self) -> &[LaneQueueReservationRecordV5] {
        &self.records
    }
    /// Borrow exact commit barriers awaiting or protecting queue-plan cleanup.
    pub(super) fn committed(&self) -> &[LaneQueueReservationKeyV2] {
        &self.committed
    }
    /// Borrow the exact committed subset whose V4 QueuePlan tombstone is durably marked.
    pub(super) fn plan_tombstoned(&self) -> &[LaneQueueReservationKeyV2] {
        &self.plan_tombstoned
    }
    /// Borrow exact prepared ordered-release barriers.
    pub(super) fn release_barriers(&self) -> &[LaneQueueReservationReleaseBarrierV3] {
        &self.release_barriers
    }
    /// Borrow completed releases awaiting or protecting FIFO restoration.
    pub(super) fn completed_releases(&self) -> &[LaneQueueReservationReleaseCompletionV5] {
        &self.completed_releases
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DurableReservationOwnership {
    Live(LaneQueueReservationKeyV2),
    Committed(LaneQueueReservationKeyV2),
    Prepared {
        key: LaneQueueReservationKeyV2,
        barrier_digest: Hash,
    },
    Completed {
        key: LaneQueueReservationKeyV2,
        barrier_digest: Hash,
    },
}
impl DurableReservationOwnership {
    const fn key(self) -> LaneQueueReservationKeyV2 {
        match self {
            Self::Live(key) | Self::Committed(key) => key,
            Self::Prepared { key, .. } | Self::Completed { key, .. } => key,
        }
    }
    fn refinement_projection(self) -> ProductionInFlightReservationOwnerProjection {
        let (state, key, release_identity) = match self {
            Self::Live(key) => (
                IN_FLIGHT_RESERVATION_STATE_LIVE,
                key,
                CanonicalIdentityProjection::zero(),
            ),
            Self::Committed(key) => (
                IN_FLIGHT_RESERVATION_STATE_COMMITTED,
                key,
                CanonicalIdentityProjection::zero(),
            ),
            Self::Prepared {
                key,
                barrier_digest,
            } => (
                IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED,
                key,
                release_refinement_identity(barrier_digest),
            ),
            Self::Completed {
                key,
                barrier_digest,
            } => (
                IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED,
                key,
                release_refinement_identity(barrier_digest),
            ),
        };
        ProductionInFlightReservationOwnerProjection {
            state,
            reservation_identity: reservation_refinement_identity(key),
            release_identity,
        }
    }
    const fn release_digest(self) -> Option<Hash> {
        match self {
            Self::Live(_) | Self::Committed(_) => None,
            Self::Prepared { barrier_digest, .. } | Self::Completed { barrier_digest, .. } => {
                Some(barrier_digest)
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CanonicalReconciliationOwner {
    ownership: DurableReservationOwnership,
    record_identity: Option<Hash>,
    plan_tombstoned: bool,
}
fn canonical_reconciliation_record_identity(
    record: &LaneQueueReservationRecordV5,
) -> io::Result<Hash> {
    let encoded = norito::encode_canonical(record).map_err(|error| {
        invalid_data(format!(
            "lane reservation reconciliation record cannot encode: {error}"
        ))
    })?;
    Ok(Hash::new_from_chunks(&[
        SNAPSHOT_RECONCILIATION_RECORD_DOMAIN,
        &encoded,
    ]))
}
fn canonical_reconciliation_owners_from_state(
    state: &IndexedReservationReplayState,
) -> io::Result<BTreeMap<HashOf<TransactionEntrypoint>, CanonicalReconciliationOwner>> {
    let mut completed_records = BTreeMap::new();
    for completion in state.completed_releases.values() {
        let release_digest = completion.value.barrier.digest();
        for record in &completion.value.ordered_records {
            let hash = record.key.entrypoint_hash;
            if completed_records
                .insert(hash, (record, release_digest))
                .is_some()
            {
                return Err(invalid_data(
                    "checked replay contains duplicate completed reconciliation records",
                ));
            }
        }
    }
    let mut owners = BTreeMap::new();
    for (hash, ownership) in &state.ownership {
        let record_identity = match ownership {
            DurableReservationOwnership::Live(key)
            | DurableReservationOwnership::Prepared { key, .. } => {
                let record = state.live.get(hash).ok_or_else(|| {
                    invalid_data("checked replay owner is missing its exact live record")
                })?;
                if record.value.key != *key {
                    return Err(invalid_data(
                        "checked replay live record disagrees with its exact owner",
                    ));
                }
                Some(canonical_reconciliation_record_identity(&record.value)?)
            }
            DurableReservationOwnership::Committed(_) => None,
            DurableReservationOwnership::Completed {
                key,
                barrier_digest,
            } => {
                let (record, record_barrier_digest) =
                    completed_records.get(hash).ok_or_else(|| {
                        invalid_data(
                            "checked replay completed owner is missing its exact completion record",
                        )
                    })?;
                if record.key != *key || record_barrier_digest != barrier_digest {
                    return Err(invalid_data(
                        "checked replay completion record disagrees with its exact owner",
                    ));
                }
                Some(canonical_reconciliation_record_identity(record)?)
            }
        };
        let plan_tombstoned = match state.plan_tombstoned.get(hash) {
            Some(marked) => {
                if marked.value != ownership.key()
                    || !matches!(ownership, DurableReservationOwnership::Committed(key) if *key == marked.value)
                {
                    return Err(invalid_data(
                        "checked replay PlanTombstoned marker is not the exact committed owner",
                    ));
                }
                true
            }
            None => false,
        };
        owners.insert(
            *hash,
            CanonicalReconciliationOwner {
                ownership: *ownership,
                record_identity,
                plan_tombstoned,
            },
        );
    }
    if completed_records.len()
        != owners
            .values()
            .filter(|owner| {
                matches!(
                    owner.ownership,
                    DurableReservationOwnership::Completed { .. }
                )
            })
            .count()
    {
        return Err(invalid_data(
            "checked replay completion records are not covered exactly once",
        ));
    }
    if state.plan_tombstoned.len()
        != owners
            .values()
            .filter(|owner| owner.plan_tombstoned)
            .count()
    {
        return Err(invalid_data(
            "checked replay PlanTombstoned markers are not covered exactly once",
        ));
    }
    Ok(owners)
}
fn canonical_reconciliation_owners_from_snapshot(
    snapshot: &LaneQueueReservationReconciliationSnapshotV1,
) -> io::Result<BTreeMap<HashOf<TransactionEntrypoint>, CanonicalReconciliationOwner>> {
    let mut expected_groups = Vec::new();
    let mut expected_group_indexes = BTreeMap::new();
    let mut owners = BTreeMap::new();
    for observed in &snapshot.ordered_records {
        let record = LaneQueueReservationRecordV5 {
            version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
            key: observed.key,
            enqueue_timestamp_ms: observed.enqueue_timestamp_ms,
            fifo_order: LaneQueueFifoOrderV5::new(observed.fifo_ordinal).map_err(invalid_data)?,
        };
        record.validate().map_err(invalid_data)?;
        let hash = record.key.entrypoint_hash;
        if owners
            .insert(
                hash,
                CanonicalReconciliationOwner {
                    ownership: DurableReservationOwnership::Live(record.key),
                    record_identity: Some(canonical_reconciliation_record_identity(&record)?),
                    plan_tombstoned: false,
                },
            )
            .is_some()
        {
            return Err(invalid_data(
                "reconciliation snapshot contains duplicate live owners",
            ));
        }
        let identity = super::LaneQueueReservationGroupIdentityV1::from_key(&record.key);
        if identity != observed.group {
            return Err(invalid_data(
                "reconciliation record disagrees with its exact proposal group",
            ));
        }
        let group_index = match expected_group_indexes.get(&identity).copied() {
            Some(index) => index,
            None => {
                let index = expected_groups.len();
                expected_group_indexes.insert(identity, index);
                expected_groups.push(super::LaneQueueReservationReconciliationGroupV1 {
                    identity,
                    ordered_keys: Vec::new(),
                });
                index
            }
        };
        expected_groups[group_index].ordered_keys.push(record.key);
    }
    if expected_groups != snapshot.ordered_groups {
        return Err(invalid_data(
            "reconciliation proposal groups do not cover each live owner exactly once",
        ));
    }
    for key in &snapshot.commit_barriers {
        key.validate().map_err(invalid_data)?;
        if owners
            .insert(
                key.entrypoint_hash,
                CanonicalReconciliationOwner {
                    ownership: DurableReservationOwnership::Committed(*key),
                    record_identity: None,
                    plan_tombstoned: false,
                },
            )
            .is_some()
        {
            return Err(invalid_data(
                "reconciliation snapshot commit barrier overlaps another exact owner",
            ));
        }
    }
    for barrier in &snapshot.prepared_release_barriers {
        barrier.validate().map_err(invalid_data)?;
        let barrier_digest = barrier.digest();
        for key in &barrier.ordered_keys {
            let owner = owners.get_mut(&key.entrypoint_hash).ok_or_else(|| {
                invalid_data("reconciliation prepared release is missing its exact live owner")
            })?;
            if owner.ownership != DurableReservationOwnership::Live(*key) {
                return Err(invalid_data(
                    "reconciliation prepared release overlaps or changes an exact owner",
                ));
            }
            owner.ownership = DurableReservationOwnership::Prepared {
                key: *key,
                barrier_digest,
            };
        }
    }
    for completion in &snapshot.completed_releases {
        completion.validate().map_err(invalid_data)?;
        let barrier_digest = completion.barrier.digest();
        for record in &completion.ordered_records {
            let hash = record.key.entrypoint_hash;
            if owners
                .insert(
                    hash,
                    CanonicalReconciliationOwner {
                        ownership: DurableReservationOwnership::Completed {
                            key: record.key,
                            barrier_digest,
                        },
                        record_identity: Some(canonical_reconciliation_record_identity(record)?),
                        plan_tombstoned: false,
                    },
                )
                .is_some()
            {
                return Err(invalid_data(
                    "reconciliation completion overlaps another exact owner",
                ));
            }
        }
    }
    let mut phases = BTreeMap::new();
    for phase in &snapshot.ordered_owner_phases {
        phase.key.validate().map_err(invalid_data)?;
        if phases.insert(phase.key.entrypoint_hash, *phase).is_some() {
            return Err(invalid_data(
                "reconciliation snapshot contains duplicate owner-phase coverage",
            ));
        }
    }
    if phases.len() != owners.len() {
        return Err(invalid_data(
            "reconciliation owner phases do not cover every exact owner once",
        ));
    }
    for (hash, owner) in &mut owners {
        let phase = phases.get(hash).ok_or_else(|| {
            invalid_data("reconciliation snapshot owner is missing its exact phase")
        })?;
        if phase.key != owner.ownership.key() {
            return Err(invalid_data(
                "reconciliation snapshot owner phase changes the exact reservation key",
            ));
        }
        let expected_reservation_phase = match owner.ownership {
            DurableReservationOwnership::Live(_) => LaneQueueReservationOwnerPhaseV6::Live,
            DurableReservationOwnership::Committed(_) => {
                LaneQueueReservationOwnerPhaseV6::CommitBarrier
            }
            DurableReservationOwnership::Prepared { .. } => {
                LaneQueueReservationOwnerPhaseV6::ReleasePrepared
            }
            DurableReservationOwnership::Completed { .. } => {
                LaneQueueReservationOwnerPhaseV6::ReleaseCompleted
            }
        };
        if phase.reservation_phase != expected_reservation_phase {
            return Err(invalid_data(
                "reconciliation snapshot owner phase disagrees with its V6 owner family",
            ));
        }
        match phase.reservation_phase {
            LaneQueueReservationOwnerPhaseV6::CommitBarrier => {
                if phase.plan_tombstone_marked
                    && phase.queue_plan_phase != QueuePlanReservationPhaseV1::Tombstoned
                {
                    return Err(invalid_data(
                        "reconciliation V6 PlanTombstoned marker conflicts with a live V4 phase",
                    ));
                }
            }
            LaneQueueReservationOwnerPhaseV6::Live
            | LaneQueueReservationOwnerPhaseV6::ReleasePrepared
            | LaneQueueReservationOwnerPhaseV6::ReleaseCompleted => {
                if phase.plan_tombstone_marked
                    || phase.queue_plan_phase != QueuePlanReservationPhaseV1::Live
                {
                    return Err(invalid_data(
                        "reconciliation non-commit owner must carry one live unmarked V4 phase",
                    ));
                }
            }
        }
        owner.plan_tombstoned = phase.plan_tombstone_marked;
    }
    Ok(owners)
}
fn canonical_reconciliation_identity(
    owners: &BTreeMap<HashOf<TransactionEntrypoint>, CanonicalReconciliationOwner>,
) -> io::Result<Hash> {
    let mut rolling = Hash::new(SNAPSHOT_RECONCILIATION_EMPTY_DOMAIN);
    for owner in owners.values() {
        let owner_identity =
            checked_owner_projection_digest(owner.ownership.refinement_projection());
        let record_present = [u8::from(owner.record_identity.is_some())];
        let plan_tombstoned = [u8::from(owner.plan_tombstoned)];
        rolling = match owner.record_identity {
            Some(record_identity) => Hash::new_from_chunks(&[
                SNAPSHOT_RECONCILIATION_STEP_DOMAIN,
                rolling.as_ref(),
                owner_identity.as_ref(),
                &record_present,
                &plan_tombstoned,
                record_identity.as_ref(),
            ]),
            None => Hash::new_from_chunks(&[
                SNAPSHOT_RECONCILIATION_STEP_DOMAIN,
                rolling.as_ref(),
                owner_identity.as_ref(),
                &record_present,
                &plan_tombstoned,
            ]),
        };
    }
    let count = u64::try_from(owners.len())
        .map_err(|_| invalid_data("reconciliation owner count exceeds u64"))?;
    Ok(Hash::new_from_chunks(&[
        SNAPSHOT_RECONCILIATION_FINAL_DOMAIN,
        rolling.as_ref(),
        &count.to_be_bytes(),
    ]))
}
fn recover_snapshot_transition_projection(
    ownership: DurableReservationOwnership,
) -> ProductionInFlightReservationTransitionProjection {
    let requested_release_identity = ownership.release_digest().map_or(
        CanonicalIdentityProjection::zero(),
        release_refinement_identity,
    );
    ProductionInFlightReservationTransitionProjection {
        action: IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,
        requested_reservation_identity: reservation_refinement_identity(ownership.key()),
        requested_release_identity,
        before: optional_owner_refinement_projection(None),
        after: ownership.refinement_projection(),
    }
}
fn reservation_refinement_identity(key: LaneQueueReservationKeyV2) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(
        IDENTITY_DOMAIN_DURABLE_ARTIFACT,
        IDENTITY_KIND_LANE_QUEUE_RESERVATION,
        *key.digest().as_ref(),
    )
}
fn release_refinement_identity(digest: Hash) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(
        IDENTITY_DOMAIN_DURABLE_ARTIFACT,
        IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER,
        *digest.as_ref(),
    )
}
fn checked_transition_frame_digest(frame: &LaneQueueReservationJournalFrameV6) -> io::Result<Hash> {
    let encoded = norito::encode_canonical(frame).map_err(|error| {
        invalid_data(format!(
            "lane reservation journal checked frame cannot encode: {error}"
        ))
    })?;
    Ok(Hash::new_from_chunks(&[
        b"iroha:queue-lane-reservation-checked-transition:v1\0",
        &encoded,
    ]))
}
fn checked_identity_projection_digest(identity: CanonicalIdentityProjection) -> Hash {
    let tags = [identity.domain, identity.kind];
    let word0 = identity.word0.to_be_bytes();
    let word1 = identity.word1.to_be_bytes();
    let word2 = identity.word2.to_be_bytes();
    let word3 = identity.word3.to_be_bytes();
    Hash::new_from_chunks(&[
        CHECKED_IDENTITY_PROJECTION_DOMAIN,
        &tags,
        &word0,
        &word1,
        &word2,
        &word3,
    ])
}
fn checked_owner_projection_digest(owner: ProductionInFlightReservationOwnerProjection) -> Hash {
    let state = [owner.state];
    let reservation = checked_identity_projection_digest(owner.reservation_identity);
    let release = checked_identity_projection_digest(owner.release_identity);
    Hash::new_from_chunks(&[
        CHECKED_OWNER_PROJECTION_DOMAIN,
        &state,
        reservation.as_ref(),
        release.as_ref(),
    ])
}
fn checked_transition_projection_digest(
    transition: ProductionInFlightReservationTransitionProjection,
) -> Hash {
    let action = [transition.action];
    let requested_reservation =
        checked_identity_projection_digest(transition.requested_reservation_identity);
    let requested_release =
        checked_identity_projection_digest(transition.requested_release_identity);
    let before = checked_owner_projection_digest(transition.before);
    let after = checked_owner_projection_digest(transition.after);
    Hash::new_from_chunks(&[
        CHECKED_TRANSITION_PROJECTION_DOMAIN,
        &action,
        requested_reservation.as_ref(),
        requested_release.as_ref(),
        before.as_ref(),
        after.as_ref(),
    ])
}
fn checked_transition_coverage_identity(
    transitions: &[CheckedProductionTransition<
        ProductionInFlightReservationTransitionProjection,
    >],
) -> io::Result<Hash> {
    transition_projection_coverage_identity(
        transitions
            .iter()
            .map(|transition| *transition.accepted_projection()),
    )
}
fn transition_projection_coverage_identity(
    transitions: impl IntoIterator<Item = ProductionInFlightReservationTransitionProjection>,
) -> io::Result<Hash> {
    let mut rolling = Hash::new(CHECKED_TRANSITION_COVERAGE_EMPTY_DOMAIN);
    let mut count = 0_u64;
    for transition in transitions {
        let transition = checked_transition_projection_digest(transition);
        rolling = Hash::new_from_chunks(&[
            CHECKED_TRANSITION_COVERAGE_STEP_DOMAIN,
            rolling.as_ref(),
            transition.as_ref(),
        ]);
        count = count
            .checked_add(1)
            .ok_or_else(|| invalid_data("checked reservation transition count exceeds u64"))?;
    }
    Ok(Hash::new_from_chunks(&[
        CHECKED_TRANSITION_COVERAGE_FINAL_DOMAIN,
        rolling.as_ref(),
        &count.to_be_bytes(),
    ]))
}
fn resulting_checked_state_identity(
    expected_state_identity: Hash,
    frame_digest: Hash,
    transition_coverage_identity: Hash,
    next_generation: u64,
) -> Hash {
    Hash::new_from_chunks(&[
        CHECKED_STATE_STEP_DOMAIN,
        expected_state_identity.as_ref(),
        frame_digest.as_ref(),
        transition_coverage_identity.as_ref(),
        &next_generation.to_be_bytes(),
    ])
}
fn optional_owner_refinement_projection(
    owner: Option<DurableReservationOwnership>,
) -> ProductionInFlightReservationOwnerProjection {
    owner.map_or(
        ProductionInFlightReservationOwnerProjection {
            state: IN_FLIGHT_RESERVATION_STATE_ABSENT,
            reservation_identity: CanonicalIdentityProjection::zero(),
            release_identity: CanonicalIdentityProjection::zero(),
        },
        DurableReservationOwnership::refinement_projection,
    )
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct OrderedReplayValue<T> {
    order: u64,
    value: T,
}
/// In-memory identity of one exact replay-state instance.
///
/// Cloning an indexed state deliberately creates a fresh domain. Logical state
/// equality ignores the domain, while checked authorizations use pointer
/// identity so a token cannot cross into an independently mutable clone with
/// an equal generation and history root. The domain is never serialized and
/// cannot affect deterministic replay output.
#[derive(Debug)]
struct CheckedReplayAuthorizationDomain(Arc<()>);
impl Default for CheckedReplayAuthorizationDomain {
    fn default() -> Self {
        Self(Arc::new(()))
    }
}
impl Clone for CheckedReplayAuthorizationDomain {
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl PartialEq for CheckedReplayAuthorizationDomain {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}
impl Eq for CheckedReplayAuthorizationDomain {}
impl CheckedReplayAuthorizationDomain {
    fn authorization(&self) -> Arc<()> {
        Arc::clone(&self.0)
    }
    fn authorizes(&self, authorization: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.0, authorization)
    }
}
/// O(1) structural witness supplementing the exact checked history root.
///
/// Every supported state mutation advances the generation and history root.
/// These cardinalities and the next replay order additionally fail closed if
/// an internal caller corrupts a collection shape without crossing that
/// mutation boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CheckedReplayStateShape {
    live: usize,
    committed: usize,
    plan_tombstoned: usize,
    release_barriers: usize,
    completed_releases: usize,
    ownership: usize,
    fifo_ordinals: usize,
    live_lane_incarnations: usize,
    next_order: u64,
}
/// Move-only authorization for one exact validated journal frame.
///
/// The canonical frame digest and ordered owner-token coverage prevent using
/// the authorization for a different group/action. The state-instance domain,
/// structural shape, generation, and expected/resulting checked-history roots
/// prevent exchange between independently mutable or divergent states. This is
/// in-memory authorization only and does not change the V6 persistence layout.
#[must_use = "checked journal authorization must reach its exact mutation boundary"]
struct PreparedReservationJournalTransition {
    authorization_domain: Arc<()>,
    frame_digest: Hash,
    maximum_owned_transactions: usize,
    expected_generation: u64,
    next_generation: u64,
    expected_shape: CheckedReplayStateShape,
    expected_state_identity: Hash,
    resulting_state_identity: Hash,
    owner_transition_count: usize,
    owner_transition_coverage_identity: Hash,
    owner_transitions:
        Vec<CheckedProductionTransition<ProductionInFlightReservationTransitionProjection>>,
}
/// Indexed retained journal state.
///
/// Every transition is validated against these indexes before any mutation. The order tags
/// preserve the historical vector ordering exposed by [`LaneQueueReservationReplay`] without
/// making replay scan the retained vectors for every frame.
#[derive(Clone, Debug, PartialEq, Eq)]
struct IndexedReservationReplayState {
    live: BTreeMap<HashOf<TransactionEntrypoint>, OrderedReplayValue<LaneQueueReservationRecordV5>>,
    committed:
        BTreeMap<HashOf<TransactionEntrypoint>, OrderedReplayValue<LaneQueueReservationKeyV2>>,
    plan_tombstoned:
        BTreeMap<HashOf<TransactionEntrypoint>, OrderedReplayValue<LaneQueueReservationKeyV2>>,
    release_barriers: BTreeMap<Hash, OrderedReplayValue<LaneQueueReservationReleaseBarrierV3>>,
    completed_releases: BTreeMap<Hash, OrderedReplayValue<LaneQueueReservationReleaseCompletionV5>>,
    ownership: BTreeMap<HashOf<TransactionEntrypoint>, DurableReservationOwnership>,
    fifo_ordinals: BTreeMap<u64, HashOf<TransactionEntrypoint>>,
    live_by_lane_incarnation: BTreeMap<(LaneId, Hash), BTreeSet<HashOf<TransactionEntrypoint>>>,
    next_order: u64,
    transition_generation: u64,
    /// O(1) hash-chain root of canonical checked transitions since reconstruction.
    checked_state_identity: Hash,
    /// Unique in-memory domain preventing authorization exchange between clones.
    authorization_domain: CheckedReplayAuthorizationDomain,
}
impl Default for IndexedReservationReplayState {
    fn default() -> Self {
        Self {
            live: BTreeMap::new(),
            committed: BTreeMap::new(),
            plan_tombstoned: BTreeMap::new(),
            release_barriers: BTreeMap::new(),
            completed_releases: BTreeMap::new(),
            ownership: BTreeMap::new(),
            fifo_ordinals: BTreeMap::new(),
            live_by_lane_incarnation: BTreeMap::new(),
            next_order: 0,
            transition_generation: 0,
            checked_state_identity: Hash::new(CHECKED_STATE_EMPTY_DOMAIN),
            authorization_domain: CheckedReplayAuthorizationDomain::default(),
        }
    }
}
impl IndexedReservationReplayState {
    fn checked_shape(&self) -> CheckedReplayStateShape {
        CheckedReplayStateShape {
            live: self.live.len(),
            committed: self.committed.len(),
            plan_tombstoned: self.plan_tombstoned.len(),
            release_barriers: self.release_barriers.len(),
            completed_releases: self.completed_releases.len(),
            ownership: self.ownership.len(),
            fifo_ordinals: self.fifo_ordinals.len(),
            live_lane_incarnations: self.live_by_lane_incarnation.len(),
            next_order: self.next_order,
        }
    }
    fn from_replay(
        replay: &LaneQueueReservationReplay,
        maximum: usize,
    ) -> io::Result<(Self, CheckedSnapshotReplayTransitionSeal)> {
        let mut state = Self::default();
        let frame = LaneQueueReservationJournalFrameV6::Snapshot {
            live: replay.records.clone(),
            committed: replay.committed.clone(),
            plan_tombstoned: replay.plan_tombstoned.clone(),
            release_barriers: replay.release_barriers.clone(),
            completed_releases: replay.completed_releases.clone(),
        };
        let frame_digest = checked_transition_frame_digest(&frame)?;
        let replay_is_empty = replay.records.is_empty()
            && replay.committed.is_empty()
            && replay.plan_tombstoned.is_empty()
            && replay.release_barriers.is_empty()
            && replay.completed_releases.is_empty();
        let (owner_transition_count, owner_transition_coverage_identity, replay_state_identity) =
            if replay_is_empty {
                (
                    0,
                    checked_transition_coverage_identity(&[])?,
                    state.checked_state_identity,
                )
            } else {
                let prepared = state.prepare_checked_transition(&frame, maximum)?;
                let receipt_fields = (
                    prepared.owner_transition_count,
                    prepared.owner_transition_coverage_identity,
                    prepared.resulting_state_identity,
                );
                state.apply_checked_transition(&frame, maximum, prepared)?;
                receipt_fields
            };
        let canonical_owners = canonical_reconciliation_owners_from_state(&state)?;
        let receipt = LaneReservationSnapshotReplayReceipt {
            frame_digest,
            owner_transition_count,
            owner_transition_coverage_identity,
            canonical_reconciliation_identity: canonical_reconciliation_identity(
                &canonical_owners,
            )?,
            replay_state_identity,
        };
        let seal = CheckedSnapshotReplayTransitionSeal {
            receipt,
            authorization_domain: state.authorization_domain.authorization(),
            replay_shape: state.checked_shape(),
            transition_generation: state.transition_generation,
            maximum_owned_transactions: maximum,
        };
        Ok((state, seal))
    }
    fn replay(&self) -> LaneQueueReservationReplay {
        LaneQueueReservationReplay {
            records: ordered_values(&self.live),
            committed: ordered_values(&self.committed),
            plan_tombstoned: ordered_values(&self.plan_tombstoned),
            release_barriers: ordered_values(&self.release_barriers),
            completed_releases: ordered_values(&self.completed_releases),
        }
    }
    fn transition(
        &mut self,
        frame: &LaneQueueReservationJournalFrameV6,
        maximum: usize,
    ) -> io::Result<()> {
        let prepared = self.prepare_checked_transition(frame, maximum)?;
        self.apply_checked_transition(frame, maximum, prepared)
    }
    fn prepare_checked_transition(
        &mut self,
        frame: &LaneQueueReservationJournalFrameV6,
        maximum: usize,
    ) -> io::Result<PreparedReservationJournalTransition> {
        validate_frame_cardinality(frame, maximum)?;
        self.transition_semantics(frame, maximum, false)?;
        let next_generation = self
            .transition_generation
            .checked_add(1)
            .ok_or_else(|| invalid_data("lane reservation transition generation overflow"))?;
        let frame_digest = checked_transition_frame_digest(frame)?;
        let owner_transitions = self.check_in_flight_transition(frame, maximum)?;
        let owner_transition_coverage_identity =
            checked_transition_coverage_identity(&owner_transitions)?;
        let resulting_state_identity = resulting_checked_state_identity(
            self.checked_state_identity,
            frame_digest,
            owner_transition_coverage_identity,
            next_generation,
        );
        Ok(PreparedReservationJournalTransition {
            authorization_domain: self.authorization_domain.authorization(),
            frame_digest,
            maximum_owned_transactions: maximum,
            expected_generation: self.transition_generation,
            next_generation,
            expected_shape: self.checked_shape(),
            expected_state_identity: self.checked_state_identity,
            resulting_state_identity,
            owner_transition_count: owner_transitions.len(),
            owner_transition_coverage_identity,
            owner_transitions,
        })
    }
    fn apply_checked_transition(
        &mut self,
        frame: &LaneQueueReservationJournalFrameV6,
        maximum: usize,
        prepared: PreparedReservationJournalTransition,
    ) -> io::Result<()> {
        if checked_transition_frame_digest(frame)? != prepared.frame_digest {
            return Err(invalid_data(
                "checked lane reservation transition does not authorize this exact frame",
            ));
        }
        if maximum != prepared.maximum_owned_transactions {
            return Err(invalid_data(
                "checked lane reservation transition uses a different ownership bound",
            ));
        }
        if !self
            .authorization_domain
            .authorizes(&prepared.authorization_domain)
        {
            return Err(invalid_data(
                "checked lane reservation transition belongs to a different exact state instance",
            ));
        }
        if self.transition_generation != prepared.expected_generation {
            return Err(invalid_data(
                "checked lane reservation transition is stale after another state operation",
            ));
        }
        if self.checked_shape() != prepared.expected_shape {
            return Err(invalid_data(
                "checked lane reservation transition belongs to a different exact pre-state shape",
            ));
        }
        if self.checked_state_identity != prepared.expected_state_identity {
            return Err(invalid_data(
                "checked lane reservation transition belongs to a different exact pre-state",
            ));
        }
        let expected_next_generation =
            prepared.expected_generation.checked_add(1).ok_or_else(|| {
                invalid_data("checked lane reservation transition generation overflow")
            })?;
        if prepared.next_generation != expected_next_generation {
            return Err(invalid_data(
                "checked lane reservation transition has a non-contiguous generation",
            ));
        }
        if prepared.owner_transition_count != prepared.owner_transitions.len()
            || checked_transition_coverage_identity(&prepared.owner_transitions)?
                != prepared.owner_transition_coverage_identity
        {
            return Err(invalid_data(
                "checked lane reservation transition has altered, missing, or reordered owner evidence",
            ));
        }
        if resulting_checked_state_identity(
            prepared.expected_state_identity,
            prepared.frame_digest,
            prepared.owner_transition_coverage_identity,
            prepared.next_generation,
        ) != prepared.resulting_state_identity
        {
            return Err(invalid_data(
                "checked lane reservation transition has a mismatched resulting state identity",
            ));
        }
        // Revalidate full semantics and derive the exact owner projections from
        // the still-unmodified state after the durable-I/O handoff. Comparing
        // projections, rather than only their digest, connects every move-only
        // token to the actual mutation pre-state without a full-state clone.
        self.transition_semantics(frame, maximum, false)?;
        let current_owner_transitions = self.check_in_flight_transition(frame, maximum)?;
        if current_owner_transitions.len() != prepared.owner_transition_count
            || current_owner_transitions
                .iter()
                .map(|checked| checked.accepted_projection())
                .ne(prepared
                    .owner_transitions
                    .iter()
                    .map(|checked| checked.accepted_projection()))
        {
            return Err(invalid_data(
                "checked lane reservation transition owner evidence no longer matches the exact pre-state",
            ));
        }
        // The transition helpers validate every fallible condition before
        // entering their `apply` blocks. We just performed that validation on
        // this exact, exclusively borrowed state. Consume both independently
        // checked token sets at the linearization point, then apply in
        // O(changed entries * log(retained entries)) rather than cloning all
        // retained indexes for every append and replayed frame.
        for checked in current_owner_transitions {
            let _accepted_projection = checked.into_projection();
        }
        for checked in prepared.owner_transitions {
            let _accepted_projection = checked.into_projection();
        }
        self.transition_semantics(frame, maximum, true)?;
        self.transition_generation = prepared.next_generation;
        self.checked_state_identity = prepared.resulting_state_identity;
        Ok(())
    }
    fn transition_semantics(
        &mut self,
        frame: &LaneQueueReservationJournalFrameV6,
        maximum: usize,
        apply: bool,
    ) -> io::Result<()> {
        match frame {
            LaneQueueReservationJournalFrameV6::Bootstrap { .. } => Err(invalid_data(
                "lane reservation journal bootstrap cannot appear as a state operation",
            )),
            LaneQueueReservationJournalFrameV6::Snapshot {
                live,
                committed,
                plan_tombstoned,
                release_barriers,
                completed_releases,
            } => self.transition_snapshot(
                live,
                committed,
                plan_tombstoned,
                release_barriers,
                completed_releases,
                maximum,
                apply,
            ),
            LaneQueueReservationJournalFrameV6::PutBatch(records) => {
                self.transition_put_batch(records, maximum, apply)
            }
            LaneQueueReservationJournalFrameV6::ReleaseBatch(keys) => {
                self.transition_release_batch(keys, apply)
            }
            LaneQueueReservationJournalFrameV6::Commit(key) => {
                self.transition_commit(*key, maximum, apply)
            }
            LaneQueueReservationJournalFrameV6::PlanTombstoned(key) => {
                self.transition_plan_tombstoned(*key, apply)
            }
            LaneQueueReservationJournalFrameV6::ForgetCommit(key) => {
                self.transition_forget_commit(*key, apply)
            }
            LaneQueueReservationJournalFrameV6::PrepareRelease(barrier) => {
                self.transition_prepare_release(barrier, apply)
            }
            LaneQueueReservationJournalFrameV6::CompleteRelease(completion) => {
                self.transition_complete_release(completion, apply)
            }
            LaneQueueReservationJournalFrameV6::ForgetRelease(barrier) => {
                self.transition_forget_release(barrier, apply)
            }
        }
    }
    fn authorize_in_flight_owner_transition(
        &self,
        action: u8,
        key: LaneQueueReservationKeyV2,
        release_digest: Option<Hash>,
        before: Option<DurableReservationOwnership>,
        after: Option<DurableReservationOwnership>,
    ) -> io::Result<CheckedProductionTransition<ProductionInFlightReservationTransitionProjection>>
    {
        let projection = ProductionInFlightReservationTransitionProjection {
            action,
            requested_reservation_identity: reservation_refinement_identity(key),
            requested_release_identity: release_digest.map_or(
                CanonicalIdentityProjection::zero(),
                release_refinement_identity,
            ),
            before: optional_owner_refinement_projection(before),
            after: optional_owner_refinement_projection(after),
        };
        check_production_in_flight_reservation_transition(projection).ok_or_else(|| {
            invalid_data(
                "lane reservation journal transition violates the checked primitive \
                 identity/state relation",
            )
        })
    }
    fn retain_in_flight_owner_transition(
        &self,
        retained: &mut Vec<
            CheckedProductionTransition<ProductionInFlightReservationTransitionProjection>,
        >,
        action: u8,
        key: LaneQueueReservationKeyV2,
        release_digest: Option<Hash>,
        before: Option<DurableReservationOwnership>,
        after: Option<DurableReservationOwnership>,
    ) -> io::Result<()> {
        retained.push(self.authorize_in_flight_owner_transition(
            action,
            key,
            release_digest,
            before,
            after,
        )?);
        Ok(())
    }
    fn check_in_flight_transition(
        &self,
        frame: &LaneQueueReservationJournalFrameV6,
        maximum: usize,
    ) -> io::Result<
        Vec<CheckedProductionTransition<ProductionInFlightReservationTransitionProjection>>,
    > {
        let mut retained = Vec::new();
        match frame {
            LaneQueueReservationJournalFrameV6::Bootstrap { .. } => Err(invalid_data(
                "lane reservation journal bootstrap cannot appear as a state operation",
            )),
            LaneQueueReservationJournalFrameV6::Snapshot {
                live,
                committed,
                plan_tombstoned,
                release_barriers,
                completed_releases,
            } => {
                let mut candidate = Self::default();
                candidate.transition_snapshot(
                    live,
                    committed,
                    plan_tombstoned,
                    release_barriers,
                    completed_releases,
                    maximum,
                    true,
                )?;
                let mut targets = BTreeMap::<
                    HashOf<TransactionEntrypoint>,
                    (LaneQueueReservationKeyV2, Option<Hash>),
                >::new();
                for owner in self
                    .ownership
                    .values()
                    .chain(candidate.ownership.values())
                    .copied()
                {
                    let key = owner.key();
                    let target = (key, owner.release_digest());
                    if let Some(existing) = targets.insert(key.entrypoint_hash, target)
                        && existing != target
                    {
                        return Err(invalid_data(
                            "reservation snapshot changes an existing primitive owner identity",
                        ));
                    }
                }
                for (hash, (key, release_digest)) in targets {
                    self.retain_in_flight_owner_transition(
                        &mut retained,
                        IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,
                        key,
                        release_digest,
                        self.ownership.get(&hash).copied(),
                        candidate.ownership.get(&hash).copied(),
                    )?;
                }
                Ok(())
            }
            LaneQueueReservationJournalFrameV6::PutBatch(records) => {
                let mut seen = BTreeSet::new();
                for record in records {
                    let key = record.key;
                    if !seen.insert(key.entrypoint_hash) {
                        continue;
                    }
                    let before = self.ownership.get(&key.entrypoint_hash).copied();
                    let after = before.or(Some(DurableReservationOwnership::Live(key)));
                    self.retain_in_flight_owner_transition(
                        &mut retained,
                        IN_FLIGHT_RESERVATION_ACTION_RESERVE,
                        key,
                        None,
                        before,
                        after,
                    )?;
                }
                Ok(())
            }
            LaneQueueReservationJournalFrameV6::ReleaseBatch(keys) => {
                for key in keys {
                    let before = self.ownership.get(&key.entrypoint_hash).copied();
                    let after = if before == Some(DurableReservationOwnership::Live(*key)) {
                        None
                    } else {
                        before
                    };
                    self.retain_in_flight_owner_transition(
                        &mut retained,
                        IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT,
                        *key,
                        None,
                        before,
                        after,
                    )?;
                }
                Ok(())
            }
            LaneQueueReservationJournalFrameV6::Commit(key) => {
                let before = self.ownership.get(&key.entrypoint_hash).copied();
                self.retain_in_flight_owner_transition(
                    &mut retained,
                    IN_FLIGHT_RESERVATION_ACTION_COMMIT,
                    *key,
                    None,
                    before,
                    Some(DurableReservationOwnership::Committed(*key)),
                )
            }
            LaneQueueReservationJournalFrameV6::PlanTombstoned(_) => Ok(()),
            LaneQueueReservationJournalFrameV6::ForgetCommit(key) => {
                let before = self.ownership.get(&key.entrypoint_hash).copied();
                let after = if before == Some(DurableReservationOwnership::Committed(*key)) {
                    None
                } else {
                    before
                };
                self.retain_in_flight_owner_transition(
                    &mut retained,
                    IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT,
                    *key,
                    None,
                    before,
                    after,
                )
            }
            LaneQueueReservationJournalFrameV6::PrepareRelease(barrier) => {
                let release_digest = barrier.digest();
                for key in &barrier.ordered_keys {
                    let before = self.ownership.get(&key.entrypoint_hash).copied();
                    let after = match before {
                        Some(DurableReservationOwnership::Live(existing)) if existing == *key => {
                            Some(DurableReservationOwnership::Prepared {
                                key: *key,
                                barrier_digest: release_digest,
                            })
                        }
                        Some(
                            owner @ (DurableReservationOwnership::Prepared { .. }
                            | DurableReservationOwnership::Completed { .. }),
                        ) => Some(owner),
                        _ => {
                            return Err(invalid_data(
                                "prepared release has no checked primitive owner transition",
                            ));
                        }
                    };
                    self.retain_in_flight_owner_transition(
                        &mut retained,
                        IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE,
                        *key,
                        Some(release_digest),
                        before,
                        after,
                    )?;
                }
                Ok(())
            }
            LaneQueueReservationJournalFrameV6::CompleteRelease(completion) => {
                let release_digest = completion.barrier.digest();
                for record in &completion.ordered_records {
                    let key = record.key;
                    let before = self.ownership.get(&key.entrypoint_hash).copied();
                    let after = match before {
                        Some(DurableReservationOwnership::Prepared {
                            key: existing,
                            barrier_digest,
                        }) if existing == key && barrier_digest == release_digest => {
                            Some(DurableReservationOwnership::Completed {
                                key,
                                barrier_digest: release_digest,
                            })
                        }
                        Some(owner @ DurableReservationOwnership::Completed { .. }) => Some(owner),
                        _ => {
                            return Err(invalid_data(
                                "completed release has no checked primitive owner transition",
                            ));
                        }
                    };
                    self.retain_in_flight_owner_transition(
                        &mut retained,
                        IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE,
                        key,
                        Some(release_digest),
                        before,
                        after,
                    )?;
                }
                Ok(())
            }
            LaneQueueReservationJournalFrameV6::ForgetRelease(barrier) => {
                let release_digest = barrier.digest();
                let has_completion = self.completed_releases.contains_key(&release_digest);
                for key in &barrier.ordered_keys {
                    let before = self.ownership.get(&key.entrypoint_hash).copied();
                    let after = if has_completion
                        && before
                            == Some(DurableReservationOwnership::Completed {
                                key: *key,
                                barrier_digest: release_digest,
                            }) {
                        None
                    } else {
                        before
                    };
                    self.retain_in_flight_owner_transition(
                        &mut retained,
                        IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE,
                        *key,
                        Some(release_digest),
                        before,
                        after,
                    )?;
                }
                Ok(())
            }
        }?;
        Ok(retained)
    }
    fn transition_snapshot(
        &mut self,
        live: &[LaneQueueReservationRecordV5],
        committed: &[LaneQueueReservationKeyV2],
        plan_tombstoned: &[LaneQueueReservationKeyV2],
        release_barriers: &[LaneQueueReservationReleaseBarrierV3],
        completed_releases: &[LaneQueueReservationReleaseCompletionV5],
        maximum: usize,
        apply: bool,
    ) -> io::Result<()> {
        let mut candidate = Self::default();
        let mut committed_seen = BTreeMap::new();
        let mut committed_additional = 0_usize;
        for key in committed {
            key.validate().map_err(invalid_data)?;
            if let Some(existing) = committed_seen.insert(key.entrypoint_hash, *key) {
                if existing != *key {
                    return Err(invalid_data(
                        "snapshot contains conflicting commit barriers for one entrypoint hash",
                    ));
                }
            } else {
                committed_additional = committed_additional
                    .checked_add(1)
                    .ok_or_else(|| invalid_data("lane reservation ownership count overflow"))?;
            }
        }
        candidate.ensure_owner_capacity(committed_additional, maximum)?;
        let (mut order, next_order) = candidate.order_range(committed_additional)?;
        for key in committed {
            if candidate.committed.contains_key(&key.entrypoint_hash) {
                continue;
            }
            candidate.committed.insert(
                key.entrypoint_hash,
                OrderedReplayValue { order, value: *key },
            );
            candidate.ownership.insert(
                key.entrypoint_hash,
                DurableReservationOwnership::Committed(*key),
            );
            order = order
                .checked_add(1)
                .ok_or_else(|| invalid_data("lane reservation replay order overflow"))?;
        }
        candidate.next_order = next_order;
        for key in plan_tombstoned {
            candidate.transition_plan_tombstoned(*key, true)?;
        }
        candidate.transition_put_batch(live, maximum, true)?;
        for barrier in release_barriers {
            candidate.transition_prepare_release(barrier, true)?;
        }
        for completion in completed_releases {
            candidate.transition_snapshot_completion(completion, maximum, true)?;
        }
        if apply {
            *self = candidate;
        }
        Ok(())
    }
    fn transition_put_batch(
        &mut self,
        records: &[LaneQueueReservationRecordV5],
        maximum: usize,
        apply: bool,
    ) -> io::Result<()> {
        let mut batch_by_hash = BTreeMap::new();
        let mut batch_by_ordinal = BTreeMap::new();
        let mut additional = 0_usize;
        for record in records {
            record.validate().map_err(invalid_data)?;
            let hash = record.key.entrypoint_hash;
            match self.ownership.get(&hash) {
                Some(DurableReservationOwnership::Live(key)) => {
                    if *key != record.key
                        || self.live.get(&hash).map(|entry| &entry.value) != Some(record)
                    {
                        return Err(invalid_data(
                            "conflicting live reservation for one entrypoint hash",
                        ));
                    }
                }
                Some(DurableReservationOwnership::Committed(_)) => {
                    return Err(invalid_data(
                        "reservation put reuses a entrypoint protected by a commit barrier",
                    ));
                }
                Some(DurableReservationOwnership::Prepared { .. }) => {
                    return Err(invalid_data(
                        "reservation put overlaps a prepared ordered release barrier",
                    ));
                }
                Some(DurableReservationOwnership::Completed { .. }) => {
                    return Err(invalid_data(
                        "reservation put overlaps a completed release awaiting durable cleanup",
                    ));
                }
                None => {}
            }
            if self
                .fifo_ordinals
                .get(&record.fifo_order.ordinal)
                .is_some_and(|existing| *existing != hash)
            {
                return Err(invalid_data(
                    "reservation put reuses a durable FIFO ordinal",
                ));
            }
            if let Some(existing) = batch_by_hash.insert(hash, record) {
                if existing != record {
                    return Err(invalid_data(
                        "reservation batch contains conflicting transaction identities",
                    ));
                }
            } else if !self.ownership.contains_key(&hash) {
                additional = additional
                    .checked_add(1)
                    .ok_or_else(|| invalid_data("lane reservation ownership count overflow"))?;
            }
            if let Some(existing_hash) = batch_by_ordinal.insert(record.fifo_order.ordinal, hash)
                && existing_hash != hash
            {
                return Err(invalid_data(
                    "reservation batch contains duplicate durable FIFO ordinals",
                ));
            }
        }
        self.ensure_owner_capacity(additional, maximum)?;
        let (mut order, next_order) = self.order_range(additional)?;
        if !apply {
            return Ok(());
        }
        let mut inserted = BTreeSet::new();
        for record in records {
            let hash = record.key.entrypoint_hash;
            if self.ownership.contains_key(&hash) || !inserted.insert(hash) {
                continue;
            }
            self.live.insert(
                hash,
                OrderedReplayValue {
                    order,
                    value: record.clone(),
                },
            );
            self.ownership
                .insert(hash, DurableReservationOwnership::Live(record.key));
            self.fifo_ordinals.insert(record.fifo_order.ordinal, hash);
            self.live_by_lane_incarnation
                .entry((record.key.lane_id, record.key.lane_incarnation))
                .or_default()
                .insert(hash);
            order = order
                .checked_add(1)
                .expect("validated lane reservation replay order range");
        }
        self.next_order = next_order;
        Ok(())
    }
    fn transition_release_batch(
        &mut self,
        keys: &[LaneQueueReservationKeyV2],
        apply: bool,
    ) -> io::Result<()> {
        let mut hashes = BTreeSet::new();
        let mut removals = Vec::new();
        for key in keys {
            key.validate().map_err(invalid_data)?;
            if !hashes.insert(key.entrypoint_hash) {
                return Err(invalid_data(
                    "lane reservation release batch contains a duplicate entrypoint",
                ));
            }
            if matches!(
                self.ownership.get(&key.entrypoint_hash),
                Some(DurableReservationOwnership::Prepared { .. })
            ) {
                return Err(invalid_data(
                    "immediate release overlaps a prepared ordered release barrier",
                ));
            }
            if self
                .ownership
                .get(&key.entrypoint_hash)
                .is_some_and(|owner| *owner == DurableReservationOwnership::Live(*key))
            {
                removals.push(self.validate_live_secondary_indexes(key.entrypoint_hash, *key)?);
            }
        }
        if apply {
            for record in &removals {
                self.remove_preflighted_live(record);
            }
        }
        Ok(())
    }
    fn transition_commit(
        &mut self,
        key: LaneQueueReservationKeyV2,
        maximum: usize,
        apply: bool,
    ) -> io::Result<()> {
        key.validate().map_err(invalid_data)?;
        let owner = self.ownership.get(&key.entrypoint_hash).copied();
        match owner {
            Some(DurableReservationOwnership::Live(existing)) if existing != key => {
                return Err(invalid_data(
                    "reservation commit conflicts with a different live reservation identity",
                ));
            }
            Some(DurableReservationOwnership::Committed(existing)) if existing != key => {
                return Err(invalid_data(
                    "reservation commit conflicts with an existing commit barrier",
                ));
            }
            Some(
                DurableReservationOwnership::Prepared { .. }
                | DurableReservationOwnership::Completed { .. },
            ) => {
                return Err(invalid_data(
                    "reservation commit overlaps an ordered release claim",
                ));
            }
            None => {
                return Err(invalid_data(
                    "reservation commit requires an exact live reservation",
                ));
            }
            _ => {}
        }
        let live_removal = match owner {
            Some(DurableReservationOwnership::Live(existing)) => {
                Some(self.validate_live_secondary_indexes(key.entrypoint_hash, existing)?)
            }
            _ => None,
        };
        let needs_commit = !matches!(
            owner,
            Some(DurableReservationOwnership::Committed(existing)) if existing == key
        );
        self.ensure_owner_capacity(0, maximum)?;
        let (order, next_order) = self.order_range(if needs_commit { 1 } else { 0 })?;
        if !apply {
            return Ok(());
        }
        if let Some(record) = &live_removal {
            self.remove_preflighted_live(record);
        }
        if needs_commit {
            self.committed.insert(
                key.entrypoint_hash,
                OrderedReplayValue { order, value: key },
            );
            self.ownership.insert(
                key.entrypoint_hash,
                DurableReservationOwnership::Committed(key),
            );
            self.next_order = next_order;
        }
        Ok(())
    }
    fn transition_forget_commit(
        &mut self,
        key: LaneQueueReservationKeyV2,
        apply: bool,
    ) -> io::Result<()> {
        key.validate().map_err(invalid_data)?;
        let owner = self.ownership.get(&key.entrypoint_hash).copied();
        match owner {
            Some(DurableReservationOwnership::Committed(existing)) if existing == key => {
                if self
                    .plan_tombstoned
                    .get(&key.entrypoint_hash)
                    .is_none_or(|marked| marked.value != key)
                {
                    return Err(invalid_data(
                        "reservation ForgetCommit requires the exact durable V6 PlanTombstoned marker",
                    ));
                }
            }
            Some(_) => {
                return Err(invalid_data(
                    "reservation ForgetCommit conflicts with another exact durable owner",
                ));
            }
            None => {
                if self.plan_tombstoned.contains_key(&key.entrypoint_hash) {
                    return Err(invalid_data(
                        "reservation PlanTombstoned marker exists without its exact commit barrier",
                    ));
                }
                return Ok(());
            }
        }
        if apply {
            self.plan_tombstoned.remove(&key.entrypoint_hash);
            self.committed.remove(&key.entrypoint_hash);
            self.ownership.remove(&key.entrypoint_hash);
        }
        Ok(())
    }
    fn transition_plan_tombstoned(
        &mut self,
        key: LaneQueueReservationKeyV2,
        apply: bool,
    ) -> io::Result<()> {
        key.validate().map_err(invalid_data)?;
        if self
            .ownership
            .get(&key.entrypoint_hash)
            .is_none_or(|owner| *owner != DurableReservationOwnership::Committed(key))
        {
            return Err(invalid_data(
                "reservation PlanTombstoned marker requires its exact commit barrier",
            ));
        }
        if let Some(existing) = self.plan_tombstoned.get(&key.entrypoint_hash) {
            return if existing.value == key {
                Ok(())
            } else {
                Err(invalid_data(
                    "reservation PlanTombstoned marker conflicts with another exact key",
                ))
            };
        }
        let (order, next_order) = self.order_range(1)?;
        if apply {
            self.plan_tombstoned.insert(
                key.entrypoint_hash,
                OrderedReplayValue { order, value: key },
            );
            self.next_order = next_order;
        }
        Ok(())
    }
    fn transition_prepare_release(
        &mut self,
        barrier: &LaneQueueReservationReleaseBarrierV3,
        apply: bool,
    ) -> io::Result<()> {
        barrier.validate().map_err(invalid_data)?;
        let digest = barrier.digest();
        if let Some(existing) = self.release_barriers.get(&digest) {
            if existing.value != *barrier {
                return Err(invalid_data(
                    "ordered release barrier digest identifies conflicting claims",
                ));
            }
            for key in &barrier.ordered_keys {
                if !self
                    .ownership
                    .get(&key.entrypoint_hash)
                    .is_some_and(|owner| {
                        *owner
                            == DurableReservationOwnership::Prepared {
                                key: *key,
                                barrier_digest: digest,
                            }
                    })
                    || self
                        .live
                        .get(&key.entrypoint_hash)
                        .is_none_or(|record| record.value.key != *key)
                {
                    return Err(invalid_data(
                        "exact prepared release no longer matches live ownership",
                    ));
                }
            }
            return Ok(());
        }
        if let Some(existing) = self.completed_releases.get(&digest) {
            if existing.value.barrier != *barrier {
                return Err(invalid_data(
                    "ordered release barrier digest identifies a conflicting completion",
                ));
            }
            return Ok(());
        }
        for key in &barrier.ordered_keys {
            match self.ownership.get(&key.entrypoint_hash) {
                Some(DurableReservationOwnership::Live(existing)) if *existing == *key => {}
                Some(DurableReservationOwnership::Live(_)) => {
                    return Err(invalid_data(
                        "ordered release barrier conflicts with the exact live reservation",
                    ));
                }
                Some(DurableReservationOwnership::Committed(_)) => {
                    return Err(invalid_data(
                        "ordered release barrier overlaps a committed reservation",
                    ));
                }
                Some(DurableReservationOwnership::Prepared { .. }) => {
                    return Err(invalid_data(
                        "conflicting ordered release barriers overlap one entrypoint",
                    ));
                }
                Some(DurableReservationOwnership::Completed { .. }) => {
                    return Err(invalid_data(
                        "ordered release barrier overlaps a conflicting completed release",
                    ));
                }
                None => {
                    return Err(invalid_data(
                        "ordered release barrier references a missing live reservation",
                    ));
                }
            }
        }
        let (order, next_order) = self.order_range(1)?;
        if apply {
            self.release_barriers.insert(
                digest,
                OrderedReplayValue {
                    order,
                    value: barrier.clone(),
                },
            );
            for key in &barrier.ordered_keys {
                self.ownership.insert(
                    key.entrypoint_hash,
                    DurableReservationOwnership::Prepared {
                        key: *key,
                        barrier_digest: digest,
                    },
                );
            }
            self.next_order = next_order;
        }
        Ok(())
    }
    fn transition_complete_release(
        &mut self,
        completion: &LaneQueueReservationReleaseCompletionV5,
        apply: bool,
    ) -> io::Result<()> {
        completion.validate().map_err(invalid_data)?;
        let digest = completion.barrier.digest();
        if let Some(existing) = self.completed_releases.get(&digest) {
            if existing.value == *completion {
                return Ok(());
            }
            return Err(invalid_data(
                "conflicting completed releases overlap one entrypoint",
            ));
        }
        let Some(prepared) = self.release_barriers.get(&digest) else {
            return Err(invalid_data(
                "ordered release completion has no exact prepared barrier",
            ));
        };
        if prepared.value != completion.barrier {
            return Err(invalid_data(
                "ordered release completion conflicts with its prepared barrier digest",
            ));
        }
        let mut removals = Vec::with_capacity(completion.ordered_records.len());
        for record in &completion.ordered_records {
            let hash = record.key.entrypoint_hash;
            if !self.ownership.get(&hash).is_some_and(|owner| {
                *owner
                    == DurableReservationOwnership::Prepared {
                        key: record.key,
                        barrier_digest: digest,
                    }
            }) {
                return Err(invalid_data(
                    "ordered release completion overlaps conflicting durable ownership",
                ));
            }
            let live_record = self.validate_live_secondary_indexes(hash, record.key)?;
            if live_record != *record {
                return Err(invalid_data(
                    "ordered release completion record differs from exact live ownership",
                ));
            }
            removals.push(live_record);
        }
        let (order, next_order) = self.order_range(1)?;
        if apply {
            self.release_barriers.remove(&digest);
            for record in &removals {
                let hash = record.key.entrypoint_hash;
                self.remove_preflighted_live(record);
                self.fifo_ordinals.insert(record.fifo_order.ordinal, hash);
                self.ownership.insert(
                    hash,
                    DurableReservationOwnership::Completed {
                        key: record.key,
                        barrier_digest: digest,
                    },
                );
            }
            self.completed_releases.insert(
                digest,
                OrderedReplayValue {
                    order,
                    value: completion.clone(),
                },
            );
            self.next_order = next_order;
        }
        Ok(())
    }
    fn transition_snapshot_completion(
        &mut self,
        completion: &LaneQueueReservationReleaseCompletionV5,
        maximum: usize,
        apply: bool,
    ) -> io::Result<()> {
        completion.validate().map_err(invalid_data)?;
        let digest = completion.barrier.digest();
        if let Some(existing) = self.completed_releases.get(&digest) {
            if existing.value == *completion {
                return Ok(());
            }
            return Err(invalid_data(
                "snapshot contains conflicting overlapping completed releases",
            ));
        }
        if let Some(prepared) = self.release_barriers.get(&digest) {
            return Err(invalid_data(if prepared.value == completion.barrier {
                "snapshot completed release overlaps a prepared release"
            } else {
                "snapshot release digest identifies conflicting prepared and completed claims"
            }));
        }
        for record in &completion.ordered_records {
            let hash = record.key.entrypoint_hash;
            if self.ownership.contains_key(&hash) {
                return Err(invalid_data(
                    "snapshot completed release overlaps live, committed, or prepared ownership",
                ));
            }
            if self
                .fifo_ordinals
                .get(&record.fifo_order.ordinal)
                .is_some_and(|existing| *existing != hash)
            {
                return Err(invalid_data(
                    "snapshot completed releases reuse one durable FIFO ordinal",
                ));
            }
        }
        self.ensure_owner_capacity(completion.ordered_records.len(), maximum)?;
        let (order, next_order) = self.order_range(1)?;
        if apply {
            for record in &completion.ordered_records {
                let hash = record.key.entrypoint_hash;
                self.fifo_ordinals.insert(record.fifo_order.ordinal, hash);
                self.ownership.insert(
                    hash,
                    DurableReservationOwnership::Completed {
                        key: record.key,
                        barrier_digest: digest,
                    },
                );
            }
            self.completed_releases.insert(
                digest,
                OrderedReplayValue {
                    order,
                    value: completion.clone(),
                },
            );
            self.next_order = next_order;
        }
        Ok(())
    }
    fn transition_forget_release(
        &mut self,
        barrier: &LaneQueueReservationReleaseBarrierV3,
        apply: bool,
    ) -> io::Result<()> {
        barrier.validate().map_err(invalid_data)?;
        let digest = barrier.digest();
        if let Some(prepared) = self.release_barriers.get(&digest) {
            if prepared.value == *barrier {
                return Err(invalid_data(
                    "cannot forget an ordered release before exact completion",
                ));
            }
            return Err(invalid_data(
                "ordered release barrier digest identifies conflicting prepared claims",
            ));
        }
        let Some(completed) = self.completed_releases.get(&digest) else {
            return Ok(());
        };
        if completed.value.barrier != *barrier {
            return Err(invalid_data(
                "ordered release barrier digest identifies a conflicting completion",
            ));
        }
        if apply {
            let completion = self
                .completed_releases
                .remove(&digest)
                .expect("validated completed release")
                .value;
            for record in completion.ordered_records {
                let hash = record.key.entrypoint_hash;
                if self.ownership.get(&hash).is_some_and(|owner| {
                    *owner
                        == DurableReservationOwnership::Completed {
                            key: record.key,
                            barrier_digest: digest,
                        }
                }) {
                    self.ownership.remove(&hash);
                    if self
                        .fifo_ordinals
                        .get(&record.fifo_order.ordinal)
                        .is_some_and(|existing| *existing == hash)
                    {
                        self.fifo_ordinals.remove(&record.fifo_order.ordinal);
                    }
                }
            }
        }
        Ok(())
    }
    fn ensure_owner_capacity(&self, additional: usize, maximum: usize) -> io::Result<()> {
        let observed = self
            .ownership
            .len()
            .checked_add(additional)
            .ok_or_else(|| invalid_data("lane reservation ownership count overflow"))?;
        if observed > maximum {
            Err(ownership_bound_error(observed, maximum))
        } else {
            Ok(())
        }
    }
    fn order_range(&self, count: usize) -> io::Result<(u64, u64)> {
        let count = u64::try_from(count)
            .map_err(|_| invalid_data("lane reservation replay order exceeds u64"))?;
        let next = self
            .next_order
            .checked_add(count)
            .ok_or_else(|| invalid_data("lane reservation replay order overflow"))?;
        Ok((self.next_order, next))
    }
    fn validate_live_secondary_indexes(
        &self,
        hash: HashOf<TransactionEntrypoint>,
        expected_key: LaneQueueReservationKeyV2,
    ) -> io::Result<LaneQueueReservationRecordV5> {
        if expected_key.entrypoint_hash != hash {
            return Err(invalid_data(
                "live reservation index key differs from the exact reservation hash",
            ));
        }
        let record = self
            .live
            .get(&hash)
            .ok_or_else(|| invalid_data("live reservation index has no exact record"))?;
        if record.value.key != expected_key {
            return Err(invalid_data(
                "live reservation index has a conflicting reservation identity",
            ));
        }
        if self.fifo_ordinals.get(&record.value.fifo_order.ordinal) != Some(&hash) {
            return Err(invalid_data(
                "live reservation FIFO index differs from the exact record",
            ));
        }
        let lane = (expected_key.lane_id, expected_key.lane_incarnation);
        if !self
            .live_by_lane_incarnation
            .get(&lane)
            .is_some_and(|hashes| hashes.contains(&hash))
        {
            return Err(invalid_data(
                "live reservation lane-incarnation index differs from the exact record",
            ));
        }
        Ok(record.value.clone())
    }
    fn remove_preflighted_live(&mut self, record: &LaneQueueReservationRecordV5) {
        let hash = record.key.entrypoint_hash;
        let lane = (record.key.lane_id, record.key.lane_incarnation);
        debug_assert_eq!(self.live.get(&hash).map(|entry| &entry.value), Some(record));
        debug_assert_eq!(
            self.fifo_ordinals.get(&record.fifo_order.ordinal),
            Some(&hash)
        );
        debug_assert!(
            self.live_by_lane_incarnation
                .get(&lane)
                .is_some_and(|hashes| hashes.contains(&hash))
        );
        self.live.remove(&hash);
        self.fifo_ordinals.remove(&record.fifo_order.ordinal);
        self.ownership.remove(&hash);
        let remove_lane = self
            .live_by_lane_incarnation
            .get_mut(&lane)
            .is_some_and(|hashes| {
                hashes.remove(&hash);
                hashes.is_empty()
            });
        if remove_lane {
            self.live_by_lane_incarnation.remove(&lane);
        }
    }
}
fn ordered_values<K, T: Clone>(values: &BTreeMap<K, OrderedReplayValue<T>>) -> Vec<T> {
    let mut ordered = values.values().collect::<Vec<_>>();
    ordered.sort_by_key(|entry| entry.order);
    ordered
        .into_iter()
        .map(|entry| entry.value.clone())
        .collect()
}
/// Append-only reservation journal with crash repair and atomic compaction.
pub(super) struct LaneQueueReservationJournal {
    path: PathBuf,
    limits: LaneQueueReservationJournalLimits,
    file: File,
    file_identity: JournalFileIdentity,
    file_revision: JournalFileRevision,
    known_len: u64,
    parent: File,
    parent_identity: JournalFileIdentity,
    replay_state: IndexedReservationReplayState,
    terminal_frames: u64,
    poisoned: bool,
    #[cfg(test)]
    next_append_fault: Option<(usize, ReservationJournalAppendFault)>,
    #[cfg(test)]
    next_compaction_fault: Option<ReservationJournalCompactionFault>,
    #[cfg(test)]
    append_handoff: Option<(Arc<Barrier>, Arc<Barrier>)>,
}
impl LaneQueueReservationJournal {
    /// Open, repair, and replay a reservation journal.
    #[cfg(test)]
    pub(super) fn open(
        path: impl AsRef<Path>,
        max_bytes_before_compact: u64,
    ) -> io::Result<(Self, LaneQueueReservationReplay)> {
        let max_bytes_before_compact =
            max_bytes_before_compact.max(minimum_bootstrap_frame_bytes()?);
        let (journal, replay, _installation_seal) = Self::open_with_limits(
            path,
            LaneQueueReservationJournalLimits::new(
                max_bytes_before_compact,
                u64::from(u32::MAX),
                u64::MAX,
                usize::MAX,
            ),
        )?;
        Ok((journal, replay))
    }
    /// Open, repair, and replay using the exact configured runtime budgets.
    pub(super) fn open_with_limits(
        path: impl AsRef<Path>,
        limits: LaneQueueReservationJournalLimits,
    ) -> io::Result<(
        Self,
        LaneQueueReservationReplay,
        LaneReservationSnapshotReplaySeal,
    )> {
        let limits = limits.validate()?;
        let requested_path = path.as_ref();
        prepare_regular_journal_parent(requested_path)?;
        let path = canonical_journal_path(requested_path)?;
        prepare_regular_journal_parent(&path)?;
        reject_missing_canonical_with_compaction_temp(&path)?;
        prepare_regular_journal_path(&path)?;
        let mut file = open_regular_append(&path)?;
        lock_regular_journal(&path, &file)?;
        let file_identity = verify_open_regular_path(&path, &file)?;
        ensure_durable_v6_bootstrap(&path, &mut file, limits)?;
        repair_suffix(&path, &mut file, limits)?;
        let known_len = file.metadata()?.len();
        ensure_file_bound(known_len, limits)?;
        let parent = open_regular_parent(&path)?;
        let parent_identity = verify_open_regular_parent(&path, &parent)?;
        let replay = replay_open_file(
            &path,
            &mut file,
            file_identity,
            known_len,
            &parent,
            parent_identity,
            limits,
        )?;
        reconcile_compaction_temp(&path, limits, &replay)?;
        validate_file_snapshot(
            &path,
            &file,
            file_identity,
            known_len,
            &parent,
            parent_identity,
        )?;
        let (replay_state, transition_seal) =
            IndexedReservationReplayState::from_replay(&replay, limits.max_owned_transactions)?;
        let file_revision = journal_file_revision(&secure_file_metadata::from_file(&file)?);
        let file_content_identity = checked_file_content_identity(
            &path,
            &mut file,
            file_identity,
            file_revision,
            known_len,
            &parent,
            parent_identity,
        )?;
        let installation_seal = LaneReservationSnapshotReplaySeal {
            transition: transition_seal,
            file_identity,
            file_revision,
            known_len,
            file_content_identity,
        };
        Ok((
            Self {
                path,
                limits,
                file,
                file_identity,
                file_revision,
                known_len,
                parent,
                parent_identity,
                replay_state,
                terminal_frames: 0,
                poisoned: false,
                #[cfg(test)]
                next_append_fault: None,
                #[cfg(test)]
                next_compaction_fault: None,
                #[cfg(test)]
                append_handoff: None,
            },
            replay,
            installation_seal,
        ))
    }
    /// Revalidate and consume the exact startup replay at Queue publication.
    ///
    /// The locked journal is hashed and replayed again under its original file,
    /// parent, revision, and length identity. The returned receipt is evidence
    /// only; it cannot authorize a second publication.
    pub(super) fn consume_snapshot_replay_seal(
        &mut self,
        seal: LaneReservationSnapshotReplaySeal,
    ) -> io::Result<LaneReservationSnapshotReplayReceipt> {
        let LaneReservationSnapshotReplaySeal {
            transition,
            file_identity,
            file_revision,
            known_len,
            file_content_identity,
        } = seal;
        if file_identity != self.file_identity
            || file_revision != self.file_revision
            || known_len != self.known_len
        {
            return Err(invalid_data(
                "lane reservation snapshot replay seal belongs to another exact journal revision",
            ));
        }
        if transition.maximum_owned_transactions != self.limits.max_owned_transactions
            || transition.transition_generation != self.replay_state.transition_generation
            || transition.replay_shape != self.replay_state.checked_shape()
            || transition.receipt.replay_state_identity != self.replay_state.checked_state_identity
            || !self
                .replay_state
                .authorization_domain
                .authorizes(&transition.authorization_domain)
        {
            return Err(invalid_data(
                "lane reservation snapshot replay seal does not authorize this exact replay state",
            ));
        }
        self.verify_cached_storage_unchanged()?;
        let current_content_identity = checked_file_content_identity(
            &self.path,
            &mut self.file,
            self.file_identity,
            self.file_revision,
            self.known_len,
            &self.parent,
            self.parent_identity,
        )?;
        if current_content_identity != file_content_identity {
            return Err(invalid_data(
                "lane reservation journal content changed after checked snapshot replay",
            ));
        }
        let replay = replay_open_file(
            &self.path,
            &mut self.file,
            self.file_identity,
            self.known_len,
            &self.parent,
            self.parent_identity,
            self.limits,
        )?;
        self.verify_cached_storage_unchanged()?;
        if replay != self.replay_state.replay() {
            return Err(invalid_data(
                "lane reservation journal replay changed before Queue publication",
            ));
        }
        let frame = LaneQueueReservationJournalFrameV6::Snapshot {
            live: replay.records,
            committed: replay.committed,
            plan_tombstoned: replay.plan_tombstoned,
            release_barriers: replay.release_barriers,
            completed_releases: replay.completed_releases,
        };
        if checked_transition_frame_digest(&frame)? != transition.receipt.frame_digest {
            return Err(invalid_data(
                "lane reservation snapshot replay frame identity changed before Queue publication",
            ));
        }
        Ok(transition.receipt)
    }
    /// Durably append an atomic reservation batch.
    pub(super) fn put_batch(
        &mut self,
        records: Vec<LaneQueueReservationRecordV5>,
    ) -> io::Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        self.append_durable(&LaneQueueReservationJournalFrameV6::PutBatch(records))
    }
    /// Durably release one exact reservation.
    pub(super) fn release(&mut self, key: LaneQueueReservationKeyV2) -> io::Result<()> {
        self.release_batch(vec![key])
    }
    /// Durably release one exact reservation batch as one journal transition.
    pub(super) fn release_batch(&mut self, keys: Vec<LaneQueueReservationKeyV2>) -> io::Result<()> {
        if keys.is_empty() {
            return Ok(());
        }
        self.append_durable(&LaneQueueReservationJournalFrameV6::ReleaseBatch(keys))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }
    /// Durably commit one exact reservation.
    pub(super) fn commit(&mut self, key: LaneQueueReservationKeyV2) -> io::Result<()> {
        self.append_durable(&LaneQueueReservationJournalFrameV6::Commit(key))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }
    /// Durably mark that the exact V4 QueuePlan tombstone is synchronized.
    pub(super) fn plan_tombstoned(&mut self, key: LaneQueueReservationKeyV2) -> io::Result<()> {
        self.append_durable(&LaneQueueReservationJournalFrameV6::PlanTombstoned(key))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }
    /// Durably forget one exact commit barrier after queue-plan cleanup.
    pub(super) fn forget_commit(&mut self, key: LaneQueueReservationKeyV2) -> io::Result<()> {
        self.append_durable(&LaneQueueReservationJournalFrameV6::ForgetCommit(key))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }
    /// Durably prepare an exact FIFO-ordered release claim.
    pub(super) fn prepare_release(
        &mut self,
        barrier: LaneQueueReservationReleaseBarrierV3,
    ) -> io::Result<()> {
        barrier.validate().map_err(invalid_data)?;
        self.append_durable(&LaneQueueReservationJournalFrameV6::PrepareRelease(barrier))
    }
    /// Durably complete an exact prepared release as one atomic journal transition.
    pub(super) fn complete_release(
        &mut self,
        completion: LaneQueueReservationReleaseCompletionV5,
    ) -> io::Result<()> {
        completion.validate().map_err(invalid_data)?;
        self.append_durable(&LaneQueueReservationJournalFrameV6::CompleteRelease(
            completion,
        ))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }
    /// Durably forget only the completion for this exact full release barrier.
    pub(super) fn forget_release(
        &mut self,
        barrier: LaneQueueReservationReleaseBarrierV3,
    ) -> io::Result<()> {
        barrier.validate().map_err(invalid_data)?;
        self.append_durable(&LaneQueueReservationJournalFrameV6::ForgetRelease(barrier))?;
        self.terminal_frames = self.terminal_frames.saturating_add(1);
        Ok(())
    }
    fn append_durable(&mut self, frame: &LaneQueueReservationJournalFrameV6) -> io::Result<()> {
        if self.poisoned {
            return Err(io::Error::other(
                "lane reservation journal is poisoned after a failed durability boundary",
            ));
        }
        if let Err(error) = self.verify_cached_storage_unchanged() {
            self.poisoned = true;
            return Err(error);
        }
        if matches!(
            frame,
            LaneQueueReservationJournalFrameV6::Snapshot { .. }
                | LaneQueueReservationJournalFrameV6::Bootstrap { .. }
        ) {
            return Err(invalid_data(
                "lane reservation bootstrap and snapshots cannot be appended as runtime operations",
            ));
        }
        // Validate the complete semantic transition against the same persistent indexes used by
        // startup replay before encoding or touching storage. Retain the move-only checked
        // identity/frame authorization across the complete staged append and synchronization.
        let prepared = self
            .replay_state
            .prepare_checked_transition(frame, self.limits.max_owned_transactions)?;
        let encoded = encode_frame_with_limit(frame, self.limits.max_frame_payload_bytes)?;
        // Exhausting a configured file budget is deterministic and happens before storage is
        // touched. Keep the journal usable (for example, so the caller can compact it) rather
        // than misclassifying that admission rejection as an ambiguous durability boundary.
        let expected_end = self.preflight_append_end(&encoded)?;
        let prepared = match self.append_staged(&encoded, expected_end, prepared) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        if let Err(error) = self.replay_state.apply_checked_transition(
            frame,
            self.limits.max_owned_transactions,
            prepared,
        ) {
            // The complete frame may already be durable while memory still
            // reflects its predecessor. Latch ambiguity and require startup
            // replay instead of panicking or attempting an in-process retry.
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
    }
    fn preflight_append_end(&self, encoded: &[u8]) -> io::Result<u64> {
        let encoded_len = u64::try_from(encoded.len())
            .map_err(|_| invalid_data("lane reservation journal frame length exceeds u64"))?;
        let expected_end = self
            .known_len
            .checked_add(encoded_len)
            .ok_or_else(|| invalid_data("lane reservation journal append length overflow"))?;
        ensure_file_bound(expected_end, self.limits)?;
        Ok(expected_end)
    }
    fn append_staged(
        &mut self,
        encoded: &[u8],
        expected_end: u64,
        authorization: PreparedReservationJournalTransition,
    ) -> io::Result<PreparedReservationJournalTransition> {
        let header_end_in_frame = usize::try_from(FRAME_HEADER_BYTES)
            .expect("reservation frame header length fits usize");
        let commit_len = RESERVATION_JOURNAL_FRAME_COMMIT.len();
        let commit_start_in_frame = encoded
            .len()
            .checked_sub(commit_len)
            .ok_or_else(|| invalid_data("lane reservation journal frame is shorter than commit"))?;
        let header_end = self
            .known_len
            .checked_add(FRAME_HEADER_BYTES)
            .ok_or_else(|| invalid_data("lane reservation journal header position overflow"))?;
        let body_end =
            self.known_len
                .checked_add(u64::try_from(commit_start_in_frame).map_err(|_| {
                    invalid_data("lane reservation journal body position exceeds u64")
                })?)
                .ok_or_else(|| invalid_data("lane reservation journal body position overflow"))?;
        self.verify_cached_storage_at_len(self.known_len)?;
        #[cfg(test)]
        if let Some((reached, resume)) = self.append_handoff.take() {
            reached.wait();
            resume.wait();
        }
        #[cfg(test)]
        let injected_fault = match self.next_append_fault {
            Some((0, fault)) => {
                self.next_append_fault = None;
                Some(fault)
            }
            Some((remaining, fault)) => {
                self.next_append_fault = Some((remaining.saturating_sub(1), fault));
                None
            }
            None => None,
        };
        #[cfg(test)]
        let inject_partial = matches!(
            injected_fault,
            Some(ReservationJournalAppendFault::PartialWrite)
        );
        #[cfg(not(test))]
        let inject_partial = false;
        #[cfg(test)]
        let inject_after_full_write = matches!(
            injected_fault,
            Some(ReservationJournalAppendFault::SyncAfterFullWrite)
        );
        #[cfg(not(test))]
        let inject_after_full_write = false;
        #[cfg(test)]
        let inject_after_sync_before_replay_publication = matches!(
            injected_fault,
            Some(ReservationJournalAppendFault::AfterSyncBeforeReplayPublication)
        );
        if inject_partial {
            // The durable header establishes that any following short body is one interrupted
            // append rather than an unknown file format.
            self.file.write_all(&encoded[..header_end_in_frame])?;
            self.file.sync_all()?;
            self.verify_cached_storage_at_len(header_end)?;
            let body = &encoded[header_end_in_frame..commit_start_in_frame];
            let prefix_len = body.len().div_ceil(2).min(body.len().saturating_sub(1));
            self.file.write_all(&body[..prefix_len])?;
            return Err(io::Error::other(
                "injected partial lane reservation journal staged-body failure",
            ));
        }
        self.file.write_all(&encoded[..header_end_in_frame])?;
        self.verify_cached_storage_at_len(header_end)?;
        self.file.sync_all()?;
        self.verify_cached_storage_at_len(header_end)?;
        self.file
            .write_all(&encoded[header_end_in_frame..commit_start_in_frame])?;
        self.verify_cached_storage_at_len(body_end)?;
        self.file.sync_all()?;
        self.verify_cached_storage_at_len(body_end)?;
        self.file.write_all(&encoded[commit_start_in_frame..])?;
        self.verify_cached_storage_at_len(expected_end)?;
        if inject_after_full_write {
            return Err(io::Error::other(
                "injected lane reservation journal sync failure after a complete staged frame",
            ));
        }
        self.file.sync_all()?;
        self.verify_cached_storage_at_len(expected_end)?;
        self.parent.sync_all()?;
        self.verify_cached_storage_at_len(expected_end)?;
        self.known_len = expected_end;
        self.file_revision = journal_file_revision(&secure_file_metadata::from_file(&self.file)?);
        #[cfg(test)]
        let authorization = if inject_after_sync_before_replay_publication {
            let mut authorization = authorization;
            authorization.expected_state_identity =
                Hash::new(b"injected checked replay publication failure");
            authorization
        } else {
            authorization
        };
        Ok(authorization)
    }
    fn verify_cached_storage_at_len(&self, expected_len: u64) -> io::Result<()> {
        validate_file_snapshot(
            &self.path,
            &self.file,
            self.file_identity,
            expected_len,
            &self.parent,
            self.parent_identity,
        )
    }
    /// Inject one ambiguous append boundary for queue-level fail-closed tests.
    #[cfg(test)]
    pub(super) fn inject_next_append_fault(&mut self, fault: ReservationJournalAppendFault) {
        self.inject_append_fault_after(0, fault);
    }
    /// Pause the next append before it touches storage for queue-lock concurrency tests.
    #[cfg(test)]
    pub(super) fn install_append_handoff(&mut self, reached: Arc<Barrier>, resume: Arc<Barrier>) {
        self.append_handoff = Some((reached, resume));
    }
    /// Inject one ambiguous append boundary after `successful_appends_before_fault` appends.
    #[cfg(test)]
    pub(super) fn inject_append_fault_after(
        &mut self,
        successful_appends_before_fault: usize,
        fault: ReservationJournalAppendFault,
    ) {
        self.next_append_fault = Some((successful_appends_before_fault, fault));
    }
    /// Inject one ambiguous compaction boundary for queue-level fail-closed tests.
    #[cfg(test)]
    pub(super) fn inject_next_compaction_fault(
        &mut self,
        fault: ReservationJournalCompactionFault,
    ) {
        self.next_compaction_fault = Some(fault);
    }
    /// Whether an append or replacement may be disk-ahead of published memory.
    pub(super) const fn durability_ambiguous(&self) -> bool {
        self.poisoned
    }
    /// Atomically rewrite only the currently live exact records when worthwhile.
    pub(super) fn compact_if_needed(
        &mut self,
        live: &[LaneQueueReservationRecordV5],
        committed: &[LaneQueueReservationKeyV2],
        plan_tombstoned: &[LaneQueueReservationKeyV2],
        release_barriers: &[LaneQueueReservationReleaseBarrierV3],
        completed_releases: &[LaneQueueReservationReleaseCompletionV5],
    ) -> io::Result<bool> {
        if self.poisoned {
            return Err(io::Error::other(
                "lane reservation journal is poisoned after a failed durability boundary",
            ));
        }
        self.verify_cached_storage_unchanged()?;
        let file_size = self.file.metadata()?.len();
        let retained_state_len = live
            .len()
            .saturating_add(committed.len())
            .saturating_add(plan_tombstoned.len())
            .saturating_add(release_barriers.len())
            .saturating_add(completed_releases.len());
        if file_size <= self.limits.max_bytes_before_compact
            && self.terminal_frames <= u64::try_from(retained_state_len).unwrap_or(u64::MAX)
        {
            return Ok(false);
        }
        let tmp = self.path.with_extension("reservation-compact.tmp");
        reject_existing_compaction_temp(&tmp)?;
        let snapshot = canonical_snapshot(
            live,
            committed,
            plan_tombstoned,
            release_barriers,
            completed_releases,
        )?;
        let mut compacted_replay_state = IndexedReservationReplayState::default();
        if let Some(frame) = snapshot.clone() {
            validate_snapshot_frame(frame, self.limits)?;
        }
        let prepared_compacted_transition = snapshot
            .as_ref()
            .map(|frame| {
                compacted_replay_state
                    .prepare_checked_transition(frame, self.limits.max_owned_transactions)
            })
            .transpose()?;
        #[cfg(test)]
        let mut prepared_compacted_transition = prepared_compacted_transition;
        let canonical_replay = replay_path(&self.path, self.limits)?;
        if canonical_snapshot(
            canonical_replay.records(),
            canonical_replay.committed(),
            canonical_replay.plan_tombstoned(),
            canonical_replay.release_barriers(),
            canonical_replay.completed_releases(),
        )? != snapshot
        {
            return Err(invalid_data(
                "lane reservation compaction input does not match the exact durable journal state",
            ));
        }
        self.verify_cached_storage_unchanged()?;
        let compacted = encode_compacted_journal_with_limits(snapshot.as_ref(), self.limits)?;
        let compacted_len = u64::try_from(compacted.len())
            .map_err(|_| invalid_data("lane reservation compacted journal exceeds u64"))?;
        ensure_file_bound(compacted_len, self.limits)?;
        let tmp_file = {
            let mut file = OpenOptions::new()
                .create_new(true)
                .read(true)
                .append(true)
                .open(&tmp)?;
            lock_regular_journal(&tmp, &file)?;
            let tmp_identity = verify_open_regular_path(&tmp, &file)?;
            write_staged_bytes(&mut file, &compacted)?;
            file.sync_all()?;
            if verify_open_regular_path(&tmp, &file)? != tmp_identity
                || file.metadata()?.len() != compacted_len
                || verify_open_regular_parent(&tmp, &self.parent)? != self.parent_identity
            {
                return Err(invalid_data(
                    "lane reservation compaction temp identity or length changed while writing",
                ));
            }
            file
        };
        let tmp_identity = verify_open_regular_path(&tmp, &tmp_file)?;
        #[cfg(test)]
        let injected_compaction_fault = self.next_compaction_fault.take();
        if let Err(error) = persist_atomic_replacement(&tmp, &self.path) {
            self.poisoned = true;
            return Err(error);
        }
        let replacement_is_exact = match (|| -> io::Result<bool> {
            Ok(
                verify_open_regular_path(&self.path, &tmp_file)? == tmp_identity
                    && tmp_file.metadata()?.len() == compacted_len
                    && verify_open_regular_parent(&self.path, &self.parent)?
                        == self.parent_identity,
            )
        })() {
            Ok(is_exact) => is_exact,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        if !replacement_is_exact {
            self.poisoned = true;
            return Err(invalid_data(
                "lane reservation compaction replacement changed during promotion",
            ));
        }
        #[cfg(test)]
        if matches!(
            injected_compaction_fault,
            Some(ReservationJournalCompactionFault::AfterRenameBeforeParentSync)
        ) {
            self.poisoned = true;
            return Err(io::Error::other(
                "injected lane reservation journal compaction failure after rename",
            ));
        }
        if let Err(error) = tmp_file.sync_all() {
            self.poisoned = true;
            return Err(error);
        }
        if let Err(error) = self.parent.sync_all() {
            self.poisoned = true;
            return Err(error);
        }
        let reopened_identity = match verify_open_regular_path(&self.path, &tmp_file) {
            Ok(identity) => identity,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        self.file = tmp_file;
        self.file_identity = reopened_identity;
        self.known_len = compacted_len;
        if let Err(error) = self.verify_cached_storage_at_len(self.known_len) {
            self.poisoned = true;
            return Err(error);
        }
        let replacement_metadata = match secure_file_metadata::from_file(&self.file) {
            Ok(metadata) => metadata,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        self.file_revision = journal_file_revision(&replacement_metadata);
        #[cfg(test)]
        if matches!(
            injected_compaction_fault,
            Some(ReservationJournalCompactionFault::AfterSyncBeforeReplayPublication)
        ) {
            let Some(prepared) = prepared_compacted_transition.as_mut() else {
                self.poisoned = true;
                return Err(io::Error::other(
                    "injected checked compaction publication failure without a snapshot",
                ));
            };
            prepared.expected_state_identity =
                Hash::new(b"injected checked compaction publication failure");
        }
        match (snapshot.as_ref(), prepared_compacted_transition) {
            (Some(frame), Some(prepared)) => {
                if let Err(error) = compacted_replay_state.apply_checked_transition(
                    frame,
                    self.limits.max_owned_transactions,
                    prepared,
                ) {
                    // The replacement is already durable. Keep the previous
                    // in-memory replay state, poison this owner, and require
                    // restart reconstruction from the canonical file.
                    self.poisoned = true;
                    return Err(error);
                }
            }
            (None, None) => {}
            _ => {
                self.poisoned = true;
                return Err(invalid_data(
                    "checked compaction snapshot authorization is internally inconsistent",
                ));
            }
        }
        self.replay_state = compacted_replay_state;
        self.terminal_frames = 0;
        Ok(true)
    }
    fn verify_cached_storage_unchanged(&self) -> io::Result<()> {
        self.verify_cached_storage_at_len(self.known_len)?;
        if journal_file_revision(&secure_file_metadata::from_file(&self.file)?)
            != self.file_revision
        {
            return Err(invalid_data(
                "lane reservation journal metadata changed outside its durable owner",
            ));
        }
        Ok(())
    }
}
fn validate_snapshot_frame(
    frame: LaneQueueReservationJournalFrameV6,
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<()> {
    let mut state = IndexedReservationReplayState::default();
    state.transition(&frame, limits.max_owned_transactions)
}
fn replay_path(
    path: &Path,
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<LaneQueueReservationReplay> {
    let mut file = open_regular_read(path)?;
    let identity = verify_open_regular_path(path, &file)?;
    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
    let len = file.metadata()?.len();
    replay_open_file(
        path,
        &mut file,
        identity,
        len,
        &parent,
        parent_identity,
        limits,
    )
}
fn replay_open_file(
    path: &Path,
    file: &mut File,
    identity: JournalFileIdentity,
    len: u64,
    parent: &File,
    parent_identity: JournalFileIdentity,
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<LaneQueueReservationReplay> {
    replay_open_file_after_initial_hash(
        path,
        file,
        identity,
        len,
        parent,
        parent_identity,
        limits,
        || Ok(()),
    )
}
fn replay_open_file_after_initial_hash<F>(
    path: &Path,
    file: &mut File,
    identity: JournalFileIdentity,
    len: u64,
    parent: &File,
    parent_identity: JournalFileIdentity,
    limits: LaneQueueReservationJournalLimits,
    after_initial_hash: F,
) -> io::Result<LaneQueueReservationReplay>
where
    F: FnOnce() -> io::Result<()>,
{
    ensure_file_bound(len, limits)?;
    validate_file_snapshot(path, file, identity, len, parent, parent_identity)?;
    let revision = journal_file_revision(&secure_file_metadata::from_file(file)?);
    let before_digest = hash_open_journal(file, len)?;
    if journal_file_revision(&secure_file_metadata::from_file(file)?) != revision {
        return Err(invalid_data(
            "lane reservation journal metadata changed while hashing before replay",
        ));
    }
    after_initial_hash()?;
    let mut state = IndexedReservationReplayState::default();
    let scanned_len = scan_frames(file, len, limits, None, |frame| {
        if !matches!(frame, LaneQueueReservationJournalFrameV6::Bootstrap { .. }) {
            state.transition(&frame, limits.max_owned_transactions)?;
        }
        Ok(())
    })?;
    if scanned_len != len {
        return Err(invalid_data(
            "lane reservation journal replay did not consume the exact retained file",
        ));
    }
    let after_digest = hash_open_journal(file, len)?;
    validate_file_snapshot(path, file, identity, len, parent, parent_identity)?;
    if journal_file_revision(&secure_file_metadata::from_file(file)?) != revision
        || after_digest != before_digest
    {
        return Err(invalid_data(
            "lane reservation journal content or metadata changed during replay",
        ));
    }
    Ok(state.replay())
}
fn hash_open_journal(file: &mut File, expected_len: u64) -> io::Result<[u8; Hash::LENGTH]> {
    file.seek(SeekFrom::Start(0))?;
    let (digest, observed_len) = sha256_reader_bounded(&mut *file, expected_len)?;
    if observed_len != expected_len {
        return Err(invalid_data(format!(
            "lane reservation journal hash consumed {observed_len} bytes, expected {expected_len}"
        )));
    }
    Ok(digest)
}
fn checked_file_content_identity(
    path: &Path,
    file: &mut File,
    identity: JournalFileIdentity,
    expected_revision: JournalFileRevision,
    expected_len: u64,
    parent: &File,
    parent_identity: JournalFileIdentity,
) -> io::Result<Hash> {
    validate_file_snapshot(path, file, identity, expected_len, parent, parent_identity)?;
    if journal_file_revision(&secure_file_metadata::from_file(file)?) != expected_revision {
        return Err(invalid_data(
            "lane reservation journal metadata changed before content authentication",
        ));
    }
    let digest = hash_open_journal(file, expected_len)?;
    validate_file_snapshot(path, file, identity, expected_len, parent, parent_identity)?;
    if journal_file_revision(&secure_file_metadata::from_file(file)?) != expected_revision {
        return Err(invalid_data(
            "lane reservation journal metadata changed during content authentication",
        ));
    }
    Ok(Hash::new_from_chunks(&[
        SNAPSHOT_REPLAY_FILE_CONTENT_DOMAIN,
        &expected_len.to_be_bytes(),
        &digest,
    ]))
}
#[cfg(test)]
fn durable_ownership_from_replay(
    replay: &LaneQueueReservationReplay,
    maximum: usize,
) -> io::Result<BTreeMap<HashOf<TransactionEntrypoint>, DurableReservationOwnership>> {
    let mut ownership =
        BTreeMap::<HashOf<TransactionEntrypoint>, DurableReservationOwnership>::new();
    let mut insert =
        |key: LaneQueueReservationKeyV2, state: DurableReservationOwnership| -> io::Result<()> {
            if let Some(existing) = ownership.get(&key.entrypoint_hash)
                && existing.key() != key
            {
                return Err(invalid_data(
                    "lane reservation replay contains conflicting durable ownership",
                ));
            }
            ownership.insert(key.entrypoint_hash, state);
            if ownership.len() > maximum {
                return Err(ownership_bound_error(ownership.len(), maximum));
            }
            Ok(())
        };
    for record in replay.records() {
        insert(record.key, DurableReservationOwnership::Live(record.key))?;
    }
    for key in replay.committed() {
        insert(*key, DurableReservationOwnership::Committed(*key))?;
    }
    for barrier in replay.release_barriers() {
        let barrier_digest = barrier.digest();
        for key in &barrier.ordered_keys {
            insert(
                *key,
                DurableReservationOwnership::Prepared {
                    key: *key,
                    barrier_digest,
                },
            )?;
        }
    }
    for completion in replay.completed_releases() {
        let barrier_digest = completion.barrier.digest();
        for record in &completion.ordered_records {
            insert(
                record.key,
                DurableReservationOwnership::Completed {
                    key: record.key,
                    barrier_digest,
                },
            )?;
        }
    }
    Ok(ownership)
}
#[cfg(test)]
fn collect_owned_hashes_bounded(
    records: &[LaneQueueReservationRecordV5],
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &[LaneQueueReservationReleaseCompletionV5],
    maximum: usize,
) -> io::Result<BTreeSet<HashOf<TransactionEntrypoint>>> {
    let mut owned = BTreeSet::new();
    let mut insert = |hash| {
        owned.insert(hash);
        if owned.len() > maximum {
            Err(ownership_bound_error(owned.len(), maximum))
        } else {
            Ok(())
        }
    };
    for record in records {
        insert(record.key.entrypoint_hash)?;
    }
    for key in committed {
        insert(key.entrypoint_hash)?;
    }
    for barrier in release_barriers {
        for key in &barrier.ordered_keys {
            insert(key.entrypoint_hash)?;
        }
    }
    for completion in completed_releases {
        for record in &completion.ordered_records {
            insert(record.key.entrypoint_hash)?;
        }
    }
    Ok(owned)
}
fn ownership_bound_error(observed: usize, maximum: usize) -> io::Error {
    invalid_data(format!(
        "lane reservation replay owns {observed} transactions, above configured limit {maximum}"
    ))
}
fn validate_frame_cardinality(
    frame: &LaneQueueReservationJournalFrameV6,
    maximum_snapshot_state: usize,
) -> io::Result<()> {
    let check_group = |label: &str, count: usize| {
        if count > MAX_MERGE_EXECUTION_ENTRYPOINTS {
            Err(invalid_data(format!(
                "lane reservation journal {label} count {count} exceeds canonical limit \
                 {MAX_MERGE_EXECUTION_ENTRYPOINTS}"
            )))
        } else {
            Ok(())
        }
    };
    let check_snapshot_state = |label: &str, count: usize| {
        if count > maximum_snapshot_state {
            Err(invalid_data(format!(
                "lane reservation journal {label} count {count} exceeds configured ownership \
                 limit {maximum_snapshot_state}"
            )))
        } else {
            Ok(())
        }
    };
    match frame {
        LaneQueueReservationJournalFrameV6::Bootstrap { .. }
        | LaneQueueReservationJournalFrameV6::Commit(_)
        | LaneQueueReservationJournalFrameV6::PlanTombstoned(_)
        | LaneQueueReservationJournalFrameV6::ForgetCommit(_) => Ok(()),
        LaneQueueReservationJournalFrameV6::Snapshot {
            live,
            committed,
            plan_tombstoned,
            release_barriers,
            completed_releases,
        } => {
            if live.is_empty()
                && committed.is_empty()
                && plan_tombstoned.is_empty()
                && release_barriers.is_empty()
                && completed_releases.is_empty()
            {
                return Err(invalid_data(
                    "lane reservation journal snapshot must retain at least one state record",
                ));
            }
            check_snapshot_state("snapshot live reservation", live.len())?;
            check_snapshot_state("snapshot commit barrier", committed.len())?;
            check_snapshot_state("snapshot plan-tombstoned barrier", plan_tombstoned.len())?;
            check_snapshot_state("snapshot prepared release", release_barriers.len())?;
            check_snapshot_state("snapshot completed release", completed_releases.len())?;
            for barrier in release_barriers {
                check_group(
                    "snapshot prepared release member",
                    barrier.ordered_keys.len(),
                )?;
            }
            for completion in completed_releases {
                check_group(
                    "snapshot completed release member",
                    completion.ordered_records.len(),
                )?;
                check_group(
                    "snapshot completed release barrier member",
                    completion.barrier.ordered_keys.len(),
                )?;
            }
            Ok(())
        }
        LaneQueueReservationJournalFrameV6::PutBatch(records) => {
            if records.is_empty() {
                return Err(invalid_data(
                    "lane reservation journal put batch must not be empty",
                ));
            }
            check_group("put batch", records.len())
        }
        LaneQueueReservationJournalFrameV6::ReleaseBatch(keys) => {
            if keys.is_empty() {
                return Err(invalid_data(
                    "lane reservation journal release batch must not be empty",
                ));
            }
            check_group("release batch", keys.len())
        }
        LaneQueueReservationJournalFrameV6::PrepareRelease(barrier)
        | LaneQueueReservationJournalFrameV6::ForgetRelease(barrier) => {
            check_group("ordered release member", barrier.ordered_keys.len())
        }
        LaneQueueReservationJournalFrameV6::CompleteRelease(completion) => {
            check_group("completed release member", completion.ordered_records.len())?;
            check_group(
                "completed release barrier member",
                completion.barrier.ordered_keys.len(),
            )
        }
    }
}
#[cfg(test)]
fn preflight_frame_ownership_bound(
    records: &[LaneQueueReservationRecordV5],
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &[LaneQueueReservationReleaseCompletionV5],
    frame: &LaneQueueReservationJournalFrameV6,
    maximum: usize,
) -> io::Result<()> {
    match frame {
        LaneQueueReservationJournalFrameV6::Snapshot {
            live,
            committed,
            plan_tombstoned: _,
            release_barriers,
            completed_releases,
        } => {
            collect_owned_hashes_bounded(
                live,
                committed,
                release_barriers,
                completed_releases,
                maximum,
            )?;
        }
        LaneQueueReservationJournalFrameV6::PutBatch(batch) => {
            let mut owned = collect_owned_hashes_bounded(
                records,
                committed,
                release_barriers,
                completed_releases,
                maximum,
            )?;
            for record in batch {
                owned.insert(record.key.entrypoint_hash);
                if owned.len() > maximum {
                    return Err(ownership_bound_error(owned.len(), maximum));
                }
            }
        }
        // These transitions only remove ownership or move an already-owned identity between
        // representations. The preceding successful replay prefix is therefore still bounded.
        LaneQueueReservationJournalFrameV6::Bootstrap { .. }
        | LaneQueueReservationJournalFrameV6::ReleaseBatch(_)
        | LaneQueueReservationJournalFrameV6::Commit(_)
        | LaneQueueReservationJournalFrameV6::PlanTombstoned(_)
        | LaneQueueReservationJournalFrameV6::ForgetCommit(_)
        | LaneQueueReservationJournalFrameV6::PrepareRelease(_)
        | LaneQueueReservationJournalFrameV6::CompleteRelease(_)
        | LaneQueueReservationJournalFrameV6::ForgetRelease(_) => {}
    }
    Ok(())
}
#[cfg(test)]
fn apply_frame(
    records: &mut Vec<LaneQueueReservationRecordV5>,
    committed: &mut Vec<LaneQueueReservationKeyV2>,
    plan_tombstoned: &mut Vec<LaneQueueReservationKeyV2>,
    release_barriers: &mut Vec<LaneQueueReservationReleaseBarrierV3>,
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV5>,
    frame: LaneQueueReservationJournalFrameV6,
) -> io::Result<()> {
    apply_frame_with_ownership_limit(
        records,
        committed,
        plan_tombstoned,
        release_barriers,
        completed_releases,
        frame,
        usize::MAX,
    )
}
#[cfg(test)]
fn apply_frame_with_ownership_limit(
    records: &mut Vec<LaneQueueReservationRecordV5>,
    committed: &mut Vec<LaneQueueReservationKeyV2>,
    plan_tombstoned: &mut Vec<LaneQueueReservationKeyV2>,
    release_barriers: &mut Vec<LaneQueueReservationReleaseBarrierV3>,
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV5>,
    frame: LaneQueueReservationJournalFrameV6,
    maximum: usize,
) -> io::Result<()> {
    validate_frame_cardinality(&frame, maximum)?;
    preflight_frame_ownership_bound(
        records,
        committed,
        release_barriers,
        completed_releases,
        &frame,
        maximum,
    )?;
    match frame {
        LaneQueueReservationJournalFrameV6::Bootstrap { .. } => {
            return Err(invalid_data(
                "lane reservation journal bootstrap cannot appear as a state operation",
            ));
        }
        LaneQueueReservationJournalFrameV6::Snapshot {
            live,
            committed: snapshot_committed,
            plan_tombstoned: snapshot_plan_tombstoned,
            release_barriers: snapshot_release_barriers,
            completed_releases: snapshot_completed_releases,
        } => {
            let mut snapshot_live = Vec::<LaneQueueReservationRecordV5>::new();
            let mut validated_committed = Vec::<LaneQueueReservationKeyV2>::new();
            let mut validated_plan_tombstoned = Vec::<LaneQueueReservationKeyV2>::new();
            let mut validated_release_barriers = Vec::<LaneQueueReservationReleaseBarrierV3>::new();
            let mut validated_completed_releases =
                Vec::<LaneQueueReservationReleaseCompletionV5>::new();
            let mut committed_by_hash = BTreeMap::new();
            for key in snapshot_committed {
                key.validate().map_err(invalid_data)?;
                if let Some(existing) = committed_by_hash.get(&key.entrypoint_hash) {
                    if *existing != key {
                        return Err(invalid_data(
                            "snapshot contains conflicting commit barriers for one entrypoint hash",
                        ));
                    }
                } else {
                    committed_by_hash.insert(key.entrypoint_hash, key);
                    validated_committed.push(key);
                }
            }
            for key in snapshot_plan_tombstoned {
                apply_plan_tombstoned(&validated_committed, &mut validated_plan_tombstoned, key)?;
            }
            apply_put_batch(
                &mut snapshot_live,
                &validated_committed,
                &validated_release_barriers,
                &validated_completed_releases,
                live,
            )?;
            for barrier in snapshot_release_barriers {
                apply_prepare_release(
                    &snapshot_live,
                    &validated_committed,
                    &mut validated_release_barriers,
                    &validated_completed_releases,
                    barrier,
                )?;
            }
            for completion in snapshot_completed_releases {
                apply_snapshot_completion(
                    &snapshot_live,
                    &validated_committed,
                    &validated_release_barriers,
                    &mut validated_completed_releases,
                    completion,
                )?;
            }
            *records = snapshot_live;
            *committed = validated_committed;
            *plan_tombstoned = validated_plan_tombstoned;
            *release_barriers = validated_release_barriers;
            *completed_releases = validated_completed_releases;
        }
        LaneQueueReservationJournalFrameV6::PutBatch(batch) => {
            apply_put_batch(
                records,
                committed,
                release_barriers,
                completed_releases,
                batch,
            )?;
        }
        LaneQueueReservationJournalFrameV6::ReleaseBatch(keys) => {
            if keys.is_empty() {
                return Err(invalid_data(
                    "lane reservation release batch must not be empty",
                ));
            }
            let mut entrypoint_hashes = BTreeSet::new();
            for key in &keys {
                key.validate().map_err(invalid_data)?;
                if !entrypoint_hashes.insert(key.entrypoint_hash) {
                    return Err(invalid_data(
                        "lane reservation release batch contains a duplicate entrypoint",
                    ));
                }
                if release_barriers
                    .iter()
                    .any(|barrier| barrier_contains_entrypoint_hash(barrier, key))
                {
                    return Err(invalid_data(
                        "immediate release overlaps a prepared ordered release barrier",
                    ));
                }
            }
            // Exact tombstones are deliberately harmless when replayed twice and must never
            // remove a later reservation with the same entrypoint hash but a different full plan.
            let exact_keys = keys
                .into_iter()
                .map(|key| (key.entrypoint_hash, key))
                .collect::<BTreeMap<_, _>>();
            records.retain(|record| {
                exact_keys
                    .get(&record.key.entrypoint_hash)
                    .is_none_or(|key| *key != record.key)
            });
        }
        LaneQueueReservationJournalFrameV6::Commit(key) => {
            apply_commit(
                records,
                committed,
                release_barriers,
                completed_releases,
                key,
            )?;
        }
        LaneQueueReservationJournalFrameV6::PlanTombstoned(key) => {
            apply_plan_tombstoned(committed, plan_tombstoned, key)?;
        }
        LaneQueueReservationJournalFrameV6::ForgetCommit(key) => {
            key.validate().map_err(invalid_data)?;
            let committed_owner = committed
                .iter()
                .find(|committed_key| committed_key.entrypoint_hash == key.entrypoint_hash);
            if let Some(existing) = committed_owner {
                if *existing != key || !plan_tombstoned.contains(&key) {
                    return Err(invalid_data(
                        "reservation ForgetCommit requires its exact V6 PlanTombstoned marker",
                    ));
                }
            } else {
                let hash = key.entrypoint_hash;
                if plan_tombstoned
                    .iter()
                    .any(|marked| marked.entrypoint_hash == hash)
                {
                    return Err(invalid_data(
                        "reservation PlanTombstoned marker exists without its exact commit barrier",
                    ));
                }
                if records
                    .iter()
                    .any(|record| record.key.entrypoint_hash == hash)
                    || release_barriers.iter().any(|barrier| {
                        barrier
                            .ordered_keys
                            .iter()
                            .any(|owned| owned.entrypoint_hash == hash)
                    })
                    || completed_releases.iter().any(|completion| {
                        completion
                            .ordered_records
                            .iter()
                            .any(|record| record.key.entrypoint_hash == hash)
                    })
                {
                    return Err(invalid_data(
                        "reservation ForgetCommit conflicts with another exact durable owner",
                    ));
                }
                return Ok(());
            }
            plan_tombstoned.retain(|marked| *marked != key);
            committed.retain(|committed_key| *committed_key != key);
        }
        LaneQueueReservationJournalFrameV6::PrepareRelease(barrier) => {
            apply_prepare_release(
                records,
                committed,
                release_barriers,
                completed_releases,
                barrier,
            )?;
        }
        LaneQueueReservationJournalFrameV6::CompleteRelease(completion) => {
            apply_complete_release(
                records,
                committed,
                release_barriers,
                completed_releases,
                completion,
            )?;
        }
        LaneQueueReservationJournalFrameV6::ForgetRelease(barrier) => {
            apply_forget_release(release_barriers, completed_releases, barrier)?;
        }
    }
    Ok(())
}
#[cfg(test)]
fn apply_plan_tombstoned(
    committed: &[LaneQueueReservationKeyV2],
    plan_tombstoned: &mut Vec<LaneQueueReservationKeyV2>,
    key: LaneQueueReservationKeyV2,
) -> io::Result<()> {
    key.validate().map_err(invalid_data)?;
    let Some(existing) = committed
        .iter()
        .find(|committed_key| committed_key.entrypoint_hash == key.entrypoint_hash)
    else {
        return Err(invalid_data(
            "reservation PlanTombstoned marker requires its exact commit barrier",
        ));
    };
    if *existing != key {
        return Err(invalid_data(
            "reservation PlanTombstoned marker conflicts with another exact commit barrier",
        ));
    }
    if !plan_tombstoned.contains(&key) {
        plan_tombstoned.push(key);
    }
    Ok(())
}
#[cfg(test)]
fn apply_put_batch(
    records: &mut Vec<LaneQueueReservationRecordV5>,
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &[LaneQueueReservationReleaseCompletionV5],
    batch: Vec<LaneQueueReservationRecordV5>,
) -> io::Result<()> {
    // Validate the entire frame before applying any record. A valid frame is one atomic
    // transition even when it contains multiple lane candidates.
    let committed_hashes = committed
        .iter()
        .map(|key| key.entrypoint_hash)
        .collect::<BTreeSet<_>>();
    let prepared_hashes = release_barriers
        .iter()
        .flat_map(|barrier| barrier.ordered_keys.iter().map(|key| key.entrypoint_hash))
        .collect::<BTreeSet<_>>();
    let completed_hashes = completed_releases
        .iter()
        .flat_map(|completion| {
            completion
                .barrier
                .ordered_keys
                .iter()
                .map(|key| key.entrypoint_hash)
        })
        .collect::<BTreeSet<_>>();
    let existing_by_hash = records
        .iter()
        .map(|record| (record.key.entrypoint_hash, record))
        .collect::<BTreeMap<_, _>>();
    let mut occupied_fifo_ordinals = records
        .iter()
        .map(|record| (record.fifo_order.ordinal, record.key.entrypoint_hash))
        .chain(completed_releases.iter().flat_map(|completion| {
            completion
                .ordered_records
                .iter()
                .map(|record| (record.fifo_order.ordinal, record.key.entrypoint_hash))
        }))
        .collect::<BTreeMap<_, _>>();
    let mut batch_by_hash = BTreeMap::new();
    for record in &batch {
        record.validate().map_err(invalid_data)?;
        if committed_hashes.contains(&record.key.entrypoint_hash) {
            return Err(invalid_data(
                "reservation put reuses a entrypoint protected by a commit barrier",
            ));
        }
        if prepared_hashes.contains(&record.key.entrypoint_hash) {
            return Err(invalid_data(
                "reservation put overlaps a prepared ordered release barrier",
            ));
        }
        if completed_hashes.contains(&record.key.entrypoint_hash) {
            return Err(invalid_data(
                "reservation put overlaps a completed release awaiting durable cleanup",
            ));
        }
        if let Some(existing) = existing_by_hash.get(&record.key.entrypoint_hash)
            && **existing != *record
        {
            return Err(invalid_data(
                "conflicting live reservation for one entrypoint hash",
            ));
        }
        if occupied_fifo_ordinals
            .get(&record.fifo_order.ordinal)
            .is_some_and(|hash| *hash != record.key.entrypoint_hash)
        {
            return Err(invalid_data(
                "reservation put reuses a durable FIFO ordinal",
            ));
        }
        if let Some(existing) = batch_by_hash.insert(record.key.entrypoint_hash, record)
            && existing != record
        {
            return Err(invalid_data(
                "reservation batch contains conflicting transaction identities",
            ));
        }
        if let Some(existing_hash) =
            occupied_fifo_ordinals.insert(record.fifo_order.ordinal, record.key.entrypoint_hash)
            && existing_hash != record.key.entrypoint_hash
        {
            return Err(invalid_data(
                "reservation batch contains duplicate durable FIFO ordinals",
            ));
        }
    }
    for record in batch {
        if !records.iter().any(|existing| existing == &record) {
            records.push(record);
        }
    }
    Ok(())
}
#[cfg(test)]
fn apply_commit(
    records: &mut Vec<LaneQueueReservationRecordV5>,
    committed: &mut Vec<LaneQueueReservationKeyV2>,
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &[LaneQueueReservationReleaseCompletionV5],
    key: LaneQueueReservationKeyV2,
) -> io::Result<()> {
    key.validate().map_err(invalid_data)?;
    if release_barriers
        .iter()
        .any(|barrier| barrier_contains_entrypoint_hash(barrier, &key))
        || completed_releases
            .iter()
            .any(|completion| barrier_contains_entrypoint_hash(&completion.barrier, &key))
    {
        return Err(invalid_data(
            "reservation commit overlaps an ordered release claim",
        ));
    }
    if let Some(existing) = records
        .iter()
        .find(|record| record.key.entrypoint_hash == key.entrypoint_hash)
        && existing.key != key
    {
        return Err(invalid_data(
            "reservation commit conflicts with a different live reservation identity",
        ));
    }
    if let Some(existing) = committed
        .iter()
        .find(|existing| existing.entrypoint_hash == key.entrypoint_hash)
    {
        if *existing != key {
            return Err(invalid_data(
                "reservation commit conflicts with an existing commit barrier",
            ));
        }
        records.retain(|record| record.key != key);
        return Ok(());
    }
    if !records.iter().any(|record| record.key == key) {
        return Err(invalid_data(
            "reservation commit requires an exact live reservation",
        ));
    }
    records.retain(|record| record.key != key);
    committed.push(key);
    Ok(())
}
#[cfg(test)]
fn apply_prepare_release(
    records: &[LaneQueueReservationRecordV5],
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &mut Vec<LaneQueueReservationReleaseBarrierV3>,
    completed_releases: &[LaneQueueReservationReleaseCompletionV5],
    barrier: LaneQueueReservationReleaseBarrierV3,
) -> io::Result<()> {
    barrier.validate().map_err(invalid_data)?;
    if committed
        .iter()
        .any(|key| barrier_contains_entrypoint_hash(&barrier, key))
    {
        return Err(invalid_data(
            "ordered release barrier overlaps a committed reservation",
        ));
    }
    for existing in release_barriers.iter() {
        if release_barriers_overlap(existing, &barrier) && existing != &barrier {
            return Err(invalid_data(
                "conflicting ordered release barriers overlap one entrypoint",
            ));
        }
    }
    let completed_exact = completed_releases
        .iter()
        .any(|completion| completion.barrier == barrier);
    for completion in completed_releases {
        if release_barriers_overlap(&completion.barrier, &barrier) && completion.barrier != barrier
        {
            return Err(invalid_data(
                "ordered release barrier overlaps a conflicting completed release",
            ));
        }
    }
    // A retry after CompleteRelease must be harmless even though ownership has
    // already moved out of the live set.
    if completed_exact {
        return Ok(());
    }
    for key in &barrier.ordered_keys {
        let Some(record) = records
            .iter()
            .find(|record| record.key.entrypoint_hash == key.entrypoint_hash)
        else {
            return Err(invalid_data(
                "ordered release barrier references a missing live reservation",
            ));
        };
        if record.key != *key {
            return Err(invalid_data(
                "ordered release barrier conflicts with the exact live reservation",
            ));
        }
    }
    if !release_barriers.contains(&barrier) {
        release_barriers.push(barrier);
    }
    Ok(())
}
#[cfg(test)]
fn apply_complete_release(
    records: &mut Vec<LaneQueueReservationRecordV5>,
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &mut Vec<LaneQueueReservationReleaseBarrierV3>,
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV5>,
    completion: LaneQueueReservationReleaseCompletionV5,
) -> io::Result<()> {
    completion.validate().map_err(invalid_data)?;
    if committed
        .iter()
        .any(|key| barrier_contains_entrypoint_hash(&completion.barrier, key))
    {
        return Err(invalid_data(
            "ordered release completion overlaps a committed reservation",
        ));
    }
    let mut completed_exact = false;
    for existing in completed_releases.iter() {
        if release_barriers_overlap(&existing.barrier, &completion.barrier) {
            if existing != &completion {
                return Err(invalid_data(
                    "conflicting completed releases overlap one entrypoint",
                ));
            }
            completed_exact = true;
        }
        if completed_fifo_orders_overlap(existing, &completion) && existing != &completion {
            return Err(invalid_data(
                "completed releases reuse one durable FIFO ordinal",
            ));
        }
    }
    if completed_exact {
        return Ok(());
    }
    let Some(barrier_position) = release_barriers
        .iter()
        .position(|barrier| barrier == &completion.barrier)
    else {
        return Err(invalid_data(
            "ordered release completion has no exact prepared barrier",
        ));
    };
    if release_barriers.iter().any(|barrier| {
        barrier != &completion.barrier && release_barriers_overlap(barrier, &completion.barrier)
    }) {
        return Err(invalid_data(
            "ordered release completion overlaps a conflicting prepared barrier",
        ));
    }
    for expected in &completion.ordered_records {
        let Some(live) = records
            .iter()
            .find(|record| record.key.entrypoint_hash == expected.key.entrypoint_hash)
        else {
            return Err(invalid_data(
                "ordered release completion references a missing live reservation",
            ));
        };
        if live != expected {
            return Err(invalid_data(
                "ordered release completion record differs from exact live ownership",
            ));
        }
    }
    records.retain(|record| !completion.ordered_records.contains(record));
    release_barriers.remove(barrier_position);
    completed_releases.push(completion);
    Ok(())
}
#[cfg(test)]
fn apply_snapshot_completion(
    records: &[LaneQueueReservationRecordV5],
    committed: &[LaneQueueReservationKeyV2],
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV5>,
    completion: LaneQueueReservationReleaseCompletionV5,
) -> io::Result<()> {
    completion.validate().map_err(invalid_data)?;
    for key in &completion.barrier.ordered_keys {
        if records
            .iter()
            .any(|record| record.key.entrypoint_hash == key.entrypoint_hash)
            || committed
                .iter()
                .any(|committed| committed.entrypoint_hash == key.entrypoint_hash)
            || release_barriers
                .iter()
                .any(|barrier| barrier_contains_entrypoint_hash(barrier, key))
        {
            return Err(invalid_data(
                "snapshot completed release overlaps live, committed, or prepared ownership",
            ));
        }
    }
    for existing in completed_releases.iter() {
        if release_barriers_overlap(&existing.barrier, &completion.barrier)
            && existing != &completion
        {
            return Err(invalid_data(
                "snapshot contains conflicting overlapping completed releases",
            ));
        }
        if completed_fifo_orders_overlap(existing, &completion) && existing != &completion {
            return Err(invalid_data(
                "snapshot completed releases reuse one durable FIFO ordinal",
            ));
        }
    }
    if completion.ordered_records.iter().any(|completed| {
        records.iter().any(|live| {
            live.key.entrypoint_hash != completed.key.entrypoint_hash
                && live.fifo_order.ordinal == completed.fifo_order.ordinal
        })
    }) {
        return Err(invalid_data(
            "snapshot completed release reuses a live durable FIFO ordinal",
        ));
    }
    if !completed_releases.contains(&completion) {
        completed_releases.push(completion);
    }
    Ok(())
}
#[cfg(test)]
fn completed_fifo_orders_overlap(
    left: &LaneQueueReservationReleaseCompletionV5,
    right: &LaneQueueReservationReleaseCompletionV5,
) -> bool {
    left.ordered_records.iter().any(|left_record| {
        right.ordered_records.iter().any(|right_record| {
            left_record.key.entrypoint_hash != right_record.key.entrypoint_hash
                && left_record.fifo_order.ordinal == right_record.fifo_order.ordinal
        })
    })
}
#[cfg(test)]
fn apply_forget_release(
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &mut Vec<LaneQueueReservationReleaseCompletionV5>,
    barrier: LaneQueueReservationReleaseBarrierV3,
) -> io::Result<()> {
    barrier.validate().map_err(invalid_data)?;
    if release_barriers.contains(&barrier) {
        return Err(invalid_data(
            "cannot forget an ordered release before exact completion",
        ));
    }
    completed_releases.retain(|completion| completion.barrier != barrier);
    Ok(())
}
#[cfg(test)]
fn barrier_contains_entrypoint_hash(
    barrier: &LaneQueueReservationReleaseBarrierV3,
    key: &LaneQueueReservationKeyV2,
) -> bool {
    barrier
        .ordered_keys
        .iter()
        .any(|barrier_key| barrier_key.entrypoint_hash == key.entrypoint_hash)
}
#[cfg(test)]
fn release_barriers_overlap(
    left: &LaneQueueReservationReleaseBarrierV3,
    right: &LaneQueueReservationReleaseBarrierV3,
) -> bool {
    left.ordered_keys
        .iter()
        .any(|key| barrier_contains_entrypoint_hash(right, key))
}
#[cfg(test)]
fn encode_frame(frame: &LaneQueueReservationJournalFrameV6) -> io::Result<Vec<u8>> {
    encode_frame_with_limit(frame, u64::from(u32::MAX))
}
fn encode_frame_with_limit(
    frame: &LaneQueueReservationJournalFrameV6,
    max_frame_payload_bytes: u64,
) -> io::Result<Vec<u8>> {
    let payload = norito::encode_canonical(frame).map_err(io::Error::other)?;
    if payload.is_empty() {
        return Err(invalid_data(
            "lane reservation journal frame payload must not be empty",
        ));
    }
    let len = u32::try_from(payload.len())
        .map_err(|_| invalid_data("lane reservation journal frame is too large"))?;
    if u64::from(len) > max_frame_payload_bytes {
        return Err(invalid_data(
            "lane reservation journal frame exceeds the configured payload limit",
        ));
    }
    let version_bytes = RESERVATION_JOURNAL_FRAME_FORMAT_VERSION.to_le_bytes();
    let len_bytes = len.to_le_bytes();
    let len_guard = (!len).to_le_bytes();
    let checksum = frame_checksum(&version_bytes, &len_bytes, &len_guard, &payload);
    let mut framed = Vec::with_capacity(
        RESERVATION_JOURNAL_FRAME_MAGIC
            .len()
            .saturating_add(version_bytes.len())
            .saturating_add(len_bytes.len())
            .saturating_add(len_guard.len())
            .saturating_add(payload.len())
            .saturating_add(Hash::LENGTH)
            .saturating_add(RESERVATION_JOURNAL_FRAME_COMMIT.len()),
    );
    framed.extend_from_slice(&RESERVATION_JOURNAL_FRAME_MAGIC);
    framed.extend_from_slice(&version_bytes);
    framed.extend_from_slice(&len_bytes);
    framed.extend_from_slice(&len_guard);
    framed.extend_from_slice(&payload);
    framed.extend_from_slice(checksum.as_ref());
    framed.extend_from_slice(&RESERVATION_JOURNAL_FRAME_COMMIT);
    Ok(framed)
}
fn bootstrap_frame() -> LaneQueueReservationJournalFrameV6 {
    let version = RESERVATION_JOURNAL_FRAME_FORMAT_VERSION;
    let version_bytes = version.to_le_bytes();
    LaneQueueReservationJournalFrameV6::Bootstrap {
        version,
        format_digest: Hash::new_from_chunks(&[
            RESERVATION_JOURNAL_BOOTSTRAP_DOMAIN,
            RESERVATION_JOURNAL_OPERATION_SCHEMA_V6,
            &RESERVATION_JOURNAL_FRAME_MAGIC,
            &version_bytes,
            &RESERVATION_JOURNAL_FRAME_COMMIT,
        ]),
    }
}
#[cfg(test)]
fn minimum_bootstrap_frame_bytes() -> io::Result<u64> {
    u64::try_from(encode_frame(&bootstrap_frame())?.len())
        .map_err(|_| invalid_input("lane reservation bootstrap frame exceeds u64"))
}
fn validate_bootstrap(frame: &LaneQueueReservationJournalFrameV6) -> io::Result<()> {
    if frame == &bootstrap_frame() {
        Ok(())
    } else {
        Err(invalid_data(
            "lane reservation journal has an invalid V6 bootstrap claim",
        ))
    }
}
fn frame_checksum(version: &[u8; 2], len: &[u8; 4], len_guard: &[u8; 4], payload: &[u8]) -> Hash {
    Hash::new_from_chunks(&[
        RESERVATION_JOURNAL_FRAME_DOMAIN,
        version,
        len,
        len_guard,
        payload,
    ])
}
#[cfg(test)]
fn encode_compacted_journal(
    snapshot: Option<&LaneQueueReservationJournalFrameV6>,
) -> io::Result<Vec<u8>> {
    let limits =
        LaneQueueReservationJournalLimits::new(u64::MAX, u64::from(u32::MAX), u64::MAX, usize::MAX);
    encode_compacted_journal_with_limits(snapshot, limits)
}
fn encode_compacted_journal_with_limits(
    snapshot: Option<&LaneQueueReservationJournalFrameV6>,
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<Vec<u8>> {
    let mut encoded = encode_frame_with_limit(&bootstrap_frame(), limits.max_frame_payload_bytes)?;
    if let Some(snapshot) = snapshot {
        encoded.extend_from_slice(&encode_frame_with_limit(
            snapshot,
            limits.max_frame_payload_bytes,
        )?);
    }
    Ok(encoded)
}
fn canonical_snapshot(
    live: &[LaneQueueReservationRecordV5],
    committed: &[LaneQueueReservationKeyV2],
    plan_tombstoned: &[LaneQueueReservationKeyV2],
    release_barriers: &[LaneQueueReservationReleaseBarrierV3],
    completed_releases: &[LaneQueueReservationReleaseCompletionV5],
) -> io::Result<Option<LaneQueueReservationJournalFrameV6>> {
    if live.is_empty()
        && committed.is_empty()
        && plan_tombstoned.is_empty()
        && release_barriers.is_empty()
        && completed_releases.is_empty()
    {
        return Ok(None);
    }
    let mut live = live.to_vec();
    live.sort_by(|left, right| {
        left.fifo_order
            .ordinal
            .cmp(&right.fifo_order.ordinal)
            .then_with(|| left.key.entrypoint_hash.cmp(&right.key.entrypoint_hash))
    });
    let mut committed = committed.to_vec();
    committed.sort_by_key(|key| key.entrypoint_hash);
    let mut plan_tombstoned = plan_tombstoned.to_vec();
    plan_tombstoned.sort_by_key(|key| key.entrypoint_hash);
    let mut release_barriers = release_barriers.to_vec();
    release_barriers
        .sort_by_key(|barrier| barrier.ordered_keys.first().map(|key| key.entrypoint_hash));
    let mut completed_releases = completed_releases.to_vec();
    completed_releases.sort_by_key(|completion| {
        completion
            .barrier
            .ordered_keys
            .first()
            .map(|key| key.entrypoint_hash)
    });
    Ok(Some(LaneQueueReservationJournalFrameV6::Snapshot {
        live,
        committed,
        plan_tombstoned,
        release_barriers,
        completed_releases,
    }))
}
fn write_staged_encoded_frame(file: &mut File, encoded: &[u8]) -> io::Result<()> {
    let header_end =
        usize::try_from(FRAME_HEADER_BYTES).expect("reservation frame header fits usize");
    let commit_start = encoded
        .len()
        .checked_sub(RESERVATION_JOURNAL_FRAME_COMMIT.len())
        .ok_or_else(|| invalid_data("lane reservation frame is shorter than its commit marker"))?;
    if commit_start < header_end {
        return Err(invalid_data(
            "lane reservation frame is shorter than its staged envelope",
        ));
    }
    file.write_all(&encoded[..header_end])?;
    file.sync_all()?;
    file.write_all(&encoded[header_end..commit_start])?;
    file.sync_all()?;
    file.write_all(&encoded[commit_start..])?;
    file.sync_all()
}
fn write_staged_bytes(file: &mut File, encoded: &[u8]) -> io::Result<()> {
    file.write_all(encoded)?;
    file.sync_all()
}
fn ensure_durable_v6_bootstrap(
    path: &Path,
    file: &mut File,
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<()> {
    let expected = encode_frame_with_limit(&bootstrap_frame(), limits.max_frame_payload_bytes)?;
    let identity = verify_open_regular_path(path, file)?;
    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
    let len = file.metadata()?.len();
    ensure_file_bound(len, limits)?;
    let expected_len = u64::try_from(expected.len())
        .map_err(|_| invalid_data("lane reservation bootstrap exceeds u64"))?;
    if len == 0 {
        file.seek(SeekFrom::Start(0))?;
        write_staged_encoded_frame(file, &expected)?;
    } else if len < expected_len {
        let actual_len = usize::try_from(len)
            .map_err(|_| invalid_data("lane reservation bootstrap prefix exceeds usize"))?;
        let mut actual = vec![0_u8; actual_len];
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut actual)?;
        if !expected.starts_with(&actual) {
            return Err(invalid_data(
                "lane reservation journal has a corrupt or unsupported initial V6 frame",
            ));
        }
        file.set_len(0)?;
        file.sync_all()?;
        parent.sync_all()?;
        file.seek(SeekFrom::Start(0))?;
        write_staged_encoded_frame(file, &expected)?;
    }
    file.sync_all()?;
    parent.sync_all()?;
    let final_len = file.metadata()?.len();
    if verify_open_regular_path(path, file)? != identity
        || verify_open_regular_parent(path, &parent)? != parent_identity
        || final_len < expected_len
    {
        return Err(invalid_data(
            "lane reservation journal storage changed while establishing its V6 bootstrap",
        ));
    }
    Ok(())
}
fn repair_suffix(
    path: &Path,
    file: &mut File,
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<()> {
    let file_identity = verify_open_regular_path(path, file)?;
    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
    let file_len = file.metadata()?.len();
    ensure_file_bound(file_len, limits)?;
    let repaired_len = scan_frames(file, file_len, limits, Some(path), |_frame| Ok(()))?;
    // A fully formed frame may have been observed after an indeterminate final `sync_all`.
    // Adopt it only after retrying both durability boundaries; this closes the two-crash window
    // where startup could otherwise publish page-cache bytes and lose them on the next restart.
    file.sync_all()?;
    parent.sync_all()?;
    validate_file_snapshot(
        path,
        file,
        file_identity,
        repaired_len,
        &parent,
        parent_identity,
    )
}
fn scan_frames<F>(
    file: &mut File,
    scan_len: u64,
    limits: LaneQueueReservationJournalLimits,
    repair_path: Option<&Path>,
    mut handle: F,
) -> io::Result<u64>
where
    F: FnMut(LaneQueueReservationJournalFrameV6) -> io::Result<()>,
{
    ensure_file_bound(scan_len, limits)?;
    let mut position = 0_u64;
    let mut saw_bootstrap = false;
    while position < scan_len {
        let remaining = scan_len
            .checked_sub(position)
            .ok_or_else(|| invalid_data("lane reservation journal scan position underflow"))?;
        // Phase one writes and syncs at most one fixed header. At any nonzero frame boundary an
        // arbitrary suffix no longer than that header is therefore an interrupted append,
        // including a full-length torn header whose magic/length guard cannot be parsed.
        if repair_path.is_some() && position != 0 && remaining <= FRAME_HEADER_BYTES {
            let path = repair_path.expect("repair path checked above");
            truncate_suffix(file, position, path)?;
            return Ok(position);
        }
        if remaining < FRAME_HEADER_BYTES {
            return Err(invalid_data(
                "lane reservation journal has an incomplete or legacy frame header",
            ));
        }
        file.seek(SeekFrom::Start(position))?;
        let mut magic = [0_u8; RESERVATION_JOURNAL_FRAME_MAGIC.len()];
        file.read_exact(&mut magic)?;
        if magic != RESERVATION_JOURNAL_FRAME_MAGIC {
            return Err(invalid_data(
                "lane reservation journal frame magic mismatch; only bootstrapped V6 is supported",
            ));
        }
        let mut version_bytes = [0_u8; 2];
        file.read_exact(&mut version_bytes)?;
        if u16::from_le_bytes(version_bytes) != RESERVATION_JOURNAL_FRAME_FORMAT_VERSION {
            return Err(invalid_data(
                "lane reservation journal frame version mismatch; only V6 is supported",
            ));
        }
        let mut len_bytes = [0_u8; 4];
        file.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes);
        let mut len_guard = [0_u8; 4];
        file.read_exact(&mut len_guard)?;
        if u32::from_le_bytes(len_guard) != !len {
            return Err(invalid_data(
                "lane reservation journal frame length guard mismatch",
            ));
        }
        let payload_len = u64::from(len);
        if payload_len == 0 || payload_len > limits.max_frame_payload_bytes {
            return Err(invalid_data(
                "lane reservation journal frame exceeds the configured payload limit",
            ));
        }
        let frame_len = FRAME_HEADER_BYTES
            .checked_add(payload_len)
            .and_then(|bytes| bytes.checked_add(FRAME_TRAILER_BYTES))
            .ok_or_else(|| invalid_data("lane reservation journal frame length overflow"))?;
        let frame_end = position
            .checked_add(frame_len)
            .ok_or_else(|| invalid_data("lane reservation journal frame position overflow"))?;
        if frame_end > scan_len {
            if let Some(path) = repair_path.filter(|_| position != 0) {
                truncate_suffix(file, position, path)?;
                return Ok(position);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "lane reservation journal has an incomplete frame",
            ));
        }
        let payload_len = usize::try_from(payload_len)
            .map_err(|_| invalid_data("lane reservation journal frame length exceeds usize"))?;
        let mut payload = vec![0_u8; payload_len];
        file.read_exact(&mut payload)?;
        let mut checksum = [0_u8; Hash::LENGTH];
        file.read_exact(&mut checksum)?;
        let mut commit = [0_u8; RESERVATION_JOURNAL_FRAME_COMMIT.len()];
        file.read_exact(&mut commit)?;
        if commit != RESERVATION_JOURNAL_FRAME_COMMIT {
            return Err(invalid_data(
                "lane reservation journal commit marker mismatch",
            ));
        }
        if frame_checksum(&version_bytes, &len_bytes, &len_guard, &payload).as_ref() != &checksum {
            return Err(invalid_data("lane reservation journal checksum mismatch"));
        }
        let frame = decode_frame(&payload, limits)?;
        match &frame {
            LaneQueueReservationJournalFrameV6::Bootstrap { .. }
                if position == 0 && !saw_bootstrap =>
            {
                validate_bootstrap(&frame)?;
                saw_bootstrap = true;
            }
            LaneQueueReservationJournalFrameV6::Bootstrap { .. } => {
                return Err(invalid_data(
                    "lane reservation journal bootstrap must appear exactly once at offset zero",
                ));
            }
            _ if !saw_bootstrap => {
                return Err(invalid_data(
                    "lane reservation journal operation appears before its V6 bootstrap",
                ));
            }
            _ => {}
        }
        handle(frame)?;
        position = frame_end;
    }
    if !saw_bootstrap {
        return Err(invalid_data(
            "lane reservation journal is missing its durable V6 bootstrap",
        ));
    }
    Ok(position)
}
fn truncate_suffix(file: &mut File, valid_end: u64, path: &Path) -> io::Result<()> {
    let identity = verify_open_regular_path(path, file)?;
    let parent = open_regular_parent(path)?;
    let parent_identity = verify_open_regular_parent(path, &parent)?;
    file.set_len(valid_end)?;
    file.sync_all()?;
    parent.sync_all()?;
    validate_file_snapshot(path, file, identity, valid_end, &parent, parent_identity)
}
fn decode_frame(
    payload: &[u8],
    limits: LaneQueueReservationJournalLimits,
) -> io::Result<LaneQueueReservationJournalFrameV6> {
    let configured_payload_limit =
        usize::try_from(limits.max_frame_payload_bytes).unwrap_or(usize::MAX);
    if payload.is_empty() || payload.len() > configured_payload_limit {
        return Err(invalid_data(
            "lane reservation journal payload exceeds the configured frame limit",
        ));
    }
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
            "lane reservation journal payload is not a canonical uncompressed archive: {error}"
        ))
    })?;
    let payload_budget = payload.len();
    let aggregate_element_budget =
        payload_budget.saturating_mul(FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT);
    // The calibrated multiplier admits Norito's deterministic owned-object overhead for small
    // frames, while the configured frame budget caps decoder-owned allocations for a hostile
    // near-limit archive. Together with the retained payload and canonical re-encoding buffers,
    // the explicitly budgeted working set is at most three configured frame budgets plus this
    // fixed overhead. Chunked envelope hashing creates no additional payload-sized copy.
    let aggregate_allocation_budget =
        frame_decode_allocation_budget(payload_budget, limits.max_frame_payload_bytes)?;
    let decode_limits = norito::DecodeLimits::new(
        payload_budget,
        payload_budget,
        aggregate_element_budget,
        aggregate_allocation_budget,
        128,
    );
    let frame = norito::decode_canonical_with_limits::<LaneQueueReservationJournalFrameV6>(
        payload,
        decode_limits,
    )
    .map_err(|error| {
        if matches!(&error, norito::Error::NonCanonicalEncoding) {
            invalid_data("lane reservation journal payload is not canonically encoded")
        } else {
            invalid_data(format!(
                "lane reservation journal payload cannot be decoded: {error}"
            ))
        }
    })?;
    Ok(frame)
}
fn frame_decode_allocation_budget(
    payload_bytes: usize,
    configured_frame_bytes: u64,
) -> io::Result<usize> {
    let calibrated = payload_bytes
        .checked_mul(FRAME_DECODE_ALLOCATION_AMPLIFICATION_LIMIT)
        .and_then(|bytes| bytes.checked_add(FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES))
        .ok_or_else(|| {
            invalid_data("lane reservation journal payload allocation budget overflow")
        })?;
    let configured_ceiling = usize::try_from(configured_frame_bytes)
        .unwrap_or(usize::MAX)
        .saturating_add(FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES);
    Ok(calibrated.min(configured_ceiling))
}
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = open_regular_parent(path)?;
    let identity = verify_open_regular_parent(path, &parent)?;
    parent.sync_all()?;
    if verify_open_regular_parent(path, &parent)? != identity {
        return Err(invalid_data(
            "lane reservation journal parent identity changed while synchronizing",
        ));
    }
    Ok(())
}
fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}
fn canonical_journal_path(path: &Path) -> io::Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        invalid_data("lane reservation journal path must end in a regular file name")
    })?;
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
type JournalFileRevision = (u64, i64, i64, i64, i64, u64, u32, u32, u32);
#[cfg(windows)]
type JournalFileRevision = (u64, u64, u64, u32, Option<u32>);
#[cfg(not(any(unix, windows)))]
type JournalFileRevision = ();
#[cfg(unix)]
fn journal_file_identity(metadata: &SecureMetadata) -> JournalFileIdentity {
    use std::os::unix::fs::MetadataExt as _;
    (metadata.dev(), metadata.ino())
}
#[cfg(windows)]
fn journal_file_identity(metadata: &SecureMetadata) -> JournalFileIdentity {
    (metadata.volume_serial_number(), metadata.file_index())
}
#[cfg(not(any(unix, windows)))]
fn journal_file_identity(_metadata: &SecureMetadata) -> JournalFileIdentity {}
#[cfg(unix)]
fn journal_file_revision(metadata: &SecureMetadata) -> JournalFileRevision {
    use std::os::unix::fs::MetadataExt as _;
    (
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
        metadata.nlink(),
        metadata.mode(),
        metadata.uid(),
        metadata.gid(),
    )
}
#[cfg(windows)]
fn journal_file_revision(metadata: &SecureMetadata) -> JournalFileRevision {
    (
        metadata.file_size(),
        metadata.creation_time(),
        metadata.last_write_time(),
        metadata.file_attributes(),
        metadata.number_of_links(),
    )
}
#[cfg(not(any(unix, windows)))]
fn journal_file_revision(_metadata: &SecureMetadata) -> JournalFileRevision {}
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
fn journal_file_is_single_link(metadata: &SecureMetadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}
#[cfg(windows)]
fn journal_file_is_reparse_point(metadata: &SecureMetadata) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}
#[cfg(not(windows))]
fn journal_file_is_reparse_point(_metadata: &SecureMetadata) -> bool {
    false
}
fn journal_file_is_indirect(metadata: &SecureMetadata) -> bool {
    metadata.file_type().is_symlink() || journal_file_is_reparse_point(metadata)
}
fn verify_open_regular_directory(path: &Path, directory: &File) -> io::Result<JournalFileIdentity> {
    let path_metadata = secure_file_metadata::from_path(path)?;
    let opened = secure_file_metadata::from_file(directory)?;
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
            "lane reservation journal parent must be a direct directory with stable identity",
        ));
    }
    if path_identity != opened_identity {
        return Err(invalid_data(
            "lane reservation journal parent path changed while opening its handle",
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
        "lane reservation journal directory identity is unsupported on this platform",
    ))
}
fn open_regular_parent(path: &Path) -> io::Result<File> {
    open_regular_directory(parent_directory(path))
}
fn verify_open_regular_parent(path: &Path, parent: &File) -> io::Result<JournalFileIdentity> {
    verify_open_regular_directory(parent_directory(path), parent)
}
fn prepare_regular_journal_parent(path: &Path) -> io::Result<()> {
    let parent = parent_directory(path);
    // Do not create a directory chain here: a journal cannot prove that every newly linked
    // ancestor survived without independently syncing each ancestor. The storage owner must
    // establish its durable parent before opening this journal.
    let directory = open_regular_directory(parent).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "lane reservation journal requires a pre-existing direct durable parent {}: {error}",
                parent.display()
            ),
        )
    })?;
    verify_open_regular_directory(parent, &directory)?;
    Ok(())
}
fn prepare_regular_journal_path(path: &Path) -> io::Result<()> {
    prepare_regular_journal_parent(path)?;
    match secure_file_metadata::from_path(path) {
        Ok(metadata) => {
            if journal_file_is_indirect(&metadata) || !metadata.is_file() {
                return Err(invalid_data(
                    "lane reservation journal path must be a direct regular file",
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
fn reject_missing_canonical_with_compaction_temp(path: &Path) -> io::Result<()> {
    let tmp = path.with_extension("reservation-compact.tmp");
    let canonical_missing =
        fs::symlink_metadata(path).is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
    if canonical_missing && fs::symlink_metadata(&tmp).is_ok() {
        return Err(invalid_data(
            "lane reservation compaction temp cannot recreate a missing unauthenticated canonical journal",
        ));
    }
    Ok(())
}
fn reconcile_compaction_temp(
    path: &Path,
    limits: LaneQueueReservationJournalLimits,
    replay: &LaneQueueReservationReplay,
) -> io::Result<()> {
    let tmp = path.with_extension("reservation-compact.tmp");
    let metadata = match secure_file_metadata::from_path(&tmp) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Ok(metadata) => metadata,
        Err(error) => return Err(error),
    };
    if journal_file_is_indirect(&metadata)
        || !metadata.is_file()
        || !journal_file_is_single_link(&metadata)
    {
        return Err(invalid_data(
            "lane reservation compaction temp must be a direct single-link regular file",
        ));
    }
    ensure_file_bound(metadata.len(), limits)?;
    let snapshot = canonical_snapshot(
        replay.records(),
        replay.committed(),
        replay.plan_tombstoned(),
        replay.release_barriers(),
        replay.completed_releases(),
    )?;
    let expected = encode_compacted_journal_with_limits(snapshot.as_ref(), limits)?;
    let temp_len = metadata.len();
    let expected_len = u64::try_from(expected.len())
        .map_err(|_| invalid_data("lane reservation compacted journal exceeds u64"))?;
    if temp_len > expected_len {
        return Err(invalid_data(
            "lane reservation compaction temp exceeds the authenticated compacted state",
        ));
    }
    let mut temp = open_regular_read(&tmp)?;
    let temp_identity = verify_open_regular_path(&tmp, &temp)?;
    let actual_len = usize::try_from(temp_len)
        .map_err(|_| invalid_data("lane reservation compaction temp exceeds usize"))?;
    let mut actual = Vec::with_capacity(actual_len);
    temp.read_to_end(&mut actual)?;
    if !expected.starts_with(&actual) {
        return Err(invalid_data(
            "lane reservation compaction temp is not an authenticated prefix of canonical state",
        ));
    }
    if verify_open_regular_path(&tmp, &temp)? != temp_identity || temp.metadata()?.len() != temp_len
    {
        return Err(invalid_data(
            "lane reservation compaction temp identity or length changed during reconciliation",
        ));
    }
    drop(temp);
    let before_remove = secure_file_metadata::from_path(&tmp)?;
    if journal_file_identity(&before_remove) != temp_identity
        || !journal_file_is_single_link(&before_remove)
    {
        return Err(invalid_data(
            "lane reservation compaction temp changed before reconciliation cleanup",
        ));
    }
    fs::remove_file(&tmp)?;
    sync_parent_directory(path)?;
    match fs::symlink_metadata(&tmp) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(invalid_data(
            "lane reservation compaction temp reappeared during reconciliation",
        )),
        Err(error) => Err(error),
    }
}
fn reject_existing_compaction_temp(path: &Path) -> io::Result<()> {
    match secure_file_metadata::from_path(path) {
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
                format!("lane reservation compaction temp collision with {kind}"),
            ))
        }
        Err(error) => Err(error),
    }
}
fn validate_regular_path(path: &Path) -> io::Result<()> {
    let metadata = secure_file_metadata::from_path(path)?;
    let identity = journal_file_identity(&metadata);
    if journal_file_is_indirect(&metadata)
        || !metadata.is_file()
        || !journal_file_identity_available(identity)
    {
        return Err(invalid_data(
            "lane reservation journal path must be a direct regular file with stable identity",
        ));
    }
    if !journal_file_is_single_link(&metadata) {
        return Err(invalid_data(
            "lane reservation journal must have exactly one filesystem link",
        ));
    }
    Ok(())
}
fn verify_open_regular_path(path: &Path, file: &File) -> io::Result<JournalFileIdentity> {
    let path_metadata = secure_file_metadata::from_path(path)?;
    let opened = secure_file_metadata::from_file(file)?;
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
            "opened lane reservation journal and path must be direct regular files with stable identities",
        ));
    }
    if !journal_file_is_single_link(&path_metadata) || !journal_file_is_single_link(&opened) {
        return Err(invalid_data(
            "lane reservation journal must have exactly one filesystem link",
        ));
    }
    if path_identity != opened_identity {
        return Err(invalid_data(
            "lane reservation journal path changed while its handle was open",
        ));
    }
    Ok(opened_identity)
}
fn validate_file_snapshot(
    path: &Path,
    file: &File,
    file_identity: JournalFileIdentity,
    expected_len: u64,
    parent: &File,
    parent_identity: JournalFileIdentity,
) -> io::Result<()> {
    if verify_open_regular_path(path, file)? != file_identity
        || file.metadata()?.len() != expected_len
        || verify_open_regular_parent(path, parent)? != parent_identity
    {
        return Err(invalid_data(
            "lane reservation journal file, parent identity, or length changed",
        ));
    }
    Ok(())
}
fn persist_atomic_replacement(temporary: &Path, destination: &Path) -> io::Result<()> {
    // `rename` cannot replace an existing destination on Windows. `TempPath::persist` selects
    // native replacement semantics on both supported platforms and preserves failed artifacts for
    // authenticated startup reconciliation.
    let mut temporary = tempfile::TempPath::try_from_path(temporary)?;
    temporary.disable_cleanup(true);
    temporary.persist(destination).map_err(|error| error.error)
}
fn open_regular_append(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let file = OpenOptions::new().append(true).read(true).open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}
fn lock_regular_journal(path: &Path, file: &File) -> io::Result<()> {
    match file.try_lock() {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "lane reservation journal is already owned by another process",
            ));
        }
        Err(fs::TryLockError::Error(error)) => return Err(error),
    }
    verify_open_regular_path(path, file)?;
    Ok(())
}
fn open_regular_read(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let file = File::open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}
fn ensure_file_bound(file_len: u64, limits: LaneQueueReservationJournalLimits) -> io::Result<()> {
    if file_len > limits.max_file_bytes {
        Err(invalid_data(format!(
            "lane reservation journal file size {file_len} exceeds configured limit {}",
            limits.max_file_bytes
        )))
    } else {
        Ok(())
    }
}
fn invalid_input(error: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
}
fn invalid_data(error: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision};
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        nexus::{DataSpaceId, LaneId},
        transaction::TransactionEntrypoint,
    };
    use std::{fs::OpenOptions, io::Write};
    const V3_RESERVATION_JOURNAL_FRAME_DOMAIN: &[u8] = b"iroha:queue-lane-reservation-frame:v3";
    const V3_RESERVATION_JOURNAL_FRAME_MAGIC: [u8; 8] = *b"IRQRJNL3";
    const V4_RESERVATION_JOURNAL_FRAME_DOMAIN: &[u8] = b"iroha:queue-lane-reservation-frame:v4";
    const V4_RESERVATION_JOURNAL_BOOTSTRAP_DOMAIN: &[u8] =
        b"iroha:queue-lane-reservation-bootstrap:v4";
    const V4_RESERVATION_JOURNAL_FRAME_MAGIC: [u8; 8] = *b"IRQRJNL4";
    /// Exact prefix of the retired V3 frame schema needed to encode its old Put and Release tags.
    #[allow(dead_code)]
    #[derive(Clone, Debug, PartialEq, Eq, Encode)]
    enum LaneQueueReservationJournalFrameV3Fixture {
        Snapshot {
            live: Vec<LaneQueueReservationRecordV5>,
            committed: Vec<LaneQueueReservationKeyV2>,
            release_barriers: Vec<LaneQueueReservationReleaseBarrierV3>,
            completed_releases: Vec<LaneQueueReservationReleaseCompletionV5>,
        },
        PutBatch(Vec<LaneQueueReservationRecordV5>),
        Release(LaneQueueReservationKeyV2),
    }
    /// Retired V4 bootstrap envelope. Its complete bytes must be retained and rejected.
    #[derive(Clone, Debug, PartialEq, Eq, Encode)]
    enum LaneQueueReservationJournalFrameV4Fixture {
        Bootstrap { version: u16, format_digest: Hash },
    }
    /// Prefix of the superseded development-only V5 operation enum. The last
    /// tag used to name an unauthenticated lane-wide removal operation; current
    /// V5 must reject its bytes instead of interpreting the shifted tag as an
    /// ordered release.
    #[allow(dead_code)]
    #[derive(Clone, Debug, PartialEq, Eq, Encode)]
    enum LaneQueueReservationJournalFrameV6RetiredRemovalFixture {
        Bootstrap {
            version: u16,
            format_digest: Hash,
        },
        Snapshot {
            live: Vec<LaneQueueReservationRecordV5>,
            committed: Vec<LaneQueueReservationKeyV2>,
            release_barriers: Vec<LaneQueueReservationReleaseBarrierV3>,
            completed_releases: Vec<LaneQueueReservationReleaseCompletionV5>,
        },
        PutBatch(Vec<LaneQueueReservationRecordV5>),
        ReleaseBatch(Vec<LaneQueueReservationKeyV2>),
        Commit(LaneQueueReservationKeyV2),
        ForgetCommit(LaneQueueReservationKeyV2),
        RetiredLaneWideRemoval {
            lane_id: LaneId,
            lane_incarnation: Hash,
        },
    }
    fn typed_hash<T>(label: &[u8]) -> HashOf<T> {
        HashOf::from_untyped_unchecked(Hash::new(label))
    }
    fn record(seed: u8, incarnation_seed: u8) -> LaneQueueReservationRecordV5 {
        let route = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(7));
        let entrypoint_hash = typed_hash::<TransactionEntrypoint>(&[seed, 2]);
        LaneQueueReservationRecordV5 {
            version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
            key: LaneQueueReservationKeyV2 {
                version: LaneQueueReservationKeyV2::VERSION,
                entrypoint_hash,
                queue_plan_admission_binding_hash: Hash::new([seed, 9]),
                routing_plan_digest: Hash::new([seed, 3]),
                coordinator_leg: RouteLeg::new(route, RouteLegRole::Coordinator),
                lane_id: route.lane_id,
                dataspace_id: route.dataspace_id,
                lane_incarnation: Hash::new([incarnation_seed, 4]),
                proposal_height: 11,
                lane_block_height: 5,
                lane_block_view: 2,
                reservation_owner_hash: Hash::new([seed, 5]),
                proposal_identity_hash: Hash::new([seed, 6]),
            },
            enqueue_timestamp_ms: 42,
            fifo_order: LaneQueueFifoOrderV5 {
                version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
                ordinal: u64::from(seed),
            },
        }
    }
    fn indexed_record(index: usize) -> LaneQueueReservationRecordV5 {
        let mut record = record(1, 1);
        let index = u64::try_from(index).expect("fixture index fits u64");
        let identity = index.saturating_add(1).to_le_bytes();
        record.key.entrypoint_hash = HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            b"indexed-reservation-entrypoint",
            &identity,
        ]));
        record.key.queue_plan_admission_binding_hash =
            Hash::new_from_chunks(&[b"indexed-reservation-admission", &identity]);
        record.key.routing_plan_digest =
            Hash::new_from_chunks(&[b"indexed-reservation-plan", &identity]);
        record.key.reservation_owner_hash =
            Hash::new_from_chunks(&[b"indexed-reservation-owner", &identity]);
        record.key.proposal_identity_hash =
            Hash::new_from_chunks(&[b"indexed-reservation-proposal", &identity]);
        record.fifo_order.ordinal = index.saturating_add(1);
        record.enqueue_timestamp_ms = index;
        record
    }
    #[test]
    fn snapshot_replay_seal_covers_empty_and_live_owner_replays() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("snapshot-replay-seal.norito");
        let limits = LaneQueueReservationJournalLimits::new(
            1024 * 1024,
            u64::from(u32::MAX),
            2 * 1024 * 1024,
            8,
        );
        let (mut journal, replay, seal) =
            LaneQueueReservationJournal::open_with_limits(&path, limits)
                .expect("open empty checked journal");
        assert_eq!(replay, LaneQueueReservationReplay::default());
        let receipt = journal
            .consume_snapshot_replay_seal(seal)
            .expect("consume exact empty replay seal");
        assert_eq!(receipt.owner_transition_count(), 0);
        journal
            .put_batch(vec![indexed_record(0)])
            .expect("persist one exact live owner");
        drop(journal);
        let (mut journal, replay, seal) =
            LaneQueueReservationJournal::open_with_limits(&path, limits)
                .expect("reopen checked live-owner journal");
        assert_eq!(replay.records().len(), 1);
        let receipt = journal
            .consume_snapshot_replay_seal(seal)
            .expect("consume exact live-owner replay seal");
        assert_eq!(receipt.owner_transition_count(), 1);
    }
    #[test]
    fn snapshot_replay_seal_rejects_changed_journal_before_publication() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("changed-snapshot-replay-seal.norito");
        let limits = LaneQueueReservationJournalLimits::new(
            1024 * 1024,
            u64::from(u32::MAX),
            2 * 1024 * 1024,
            8,
        );
        let (mut journal, _replay, seal) =
            LaneQueueReservationJournal::open_with_limits(&path, limits)
                .expect("open checked journal");
        let mut competing_writer = OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open explicit corruption writer");
        competing_writer
            .write_all(b"changed-after-replay")
            .expect("change journal after replay");
        competing_writer.sync_all().expect("sync changed journal");
        assert!(
            journal.consume_snapshot_replay_seal(seal).is_err(),
            "exact file identity revalidation must reject post-replay changes"
        );
    }
    #[test]
    fn snapshot_replay_receipt_rejects_same_count_owner_identity_drift() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("snapshot-replay-owner-identity.norito");
        let limits = LaneQueueReservationJournalLimits::new(
            1024 * 1024,
            u64::from(u32::MAX),
            2 * 1024 * 1024,
            8,
        );
        let record = indexed_record(0);
        let key = record.key;
        let (mut journal, _replay, seal) =
            LaneQueueReservationJournal::open_with_limits(&path, limits)
                .expect("open checked journal");
        journal
            .consume_snapshot_replay_seal(seal)
            .expect("consume initial empty replay seal");
        journal
            .put_batch(vec![record])
            .expect("persist exact live owner");
        journal.commit(key).expect("persist exact commit owner");
        drop(journal);
        let (mut journal, replay, seal) =
            LaneQueueReservationJournal::open_with_limits(&path, limits)
                .expect("reopen committed owner");
        assert_eq!(replay.committed(), &[key]);
        let receipt = journal
            .consume_snapshot_replay_seal(seal)
            .expect("consume exact committed replay seal");
        let mut snapshot = LaneQueueReservationReconciliationSnapshotV1 {
            commit_barriers: vec![key],
            ordered_owner_phases: vec![LaneQueueReservationRecoveryPhaseV1 {
                key,
                reservation_phase: LaneQueueReservationOwnerPhaseV6::CommitBarrier,
                queue_plan_phase: QueuePlanReservationPhaseV1::Live,
                plan_tombstone_marked: false,
            }],
            ..LaneQueueReservationReconciliationSnapshotV1::default()
        };
        assert!(
            receipt
                .binds_reconciliation_snapshot(&snapshot)
                .expect("validate exact reconciliation identity")
        );
        let mut unmarked_tombstone_window = snapshot.clone();
        unmarked_tombstone_window.ordered_owner_phases[0].queue_plan_phase =
            QueuePlanReservationPhaseV1::Tombstoned;
        assert!(
            receipt
                .binds_reconciliation_snapshot(&unmarked_tombstone_window)
                .expect("validate unmarked V4/V6 crash window"),
            "V4 Tombstoned must remain independent of the still-pending V6 marker"
        );
        let mut missing_phase = snapshot.clone();
        missing_phase.ordered_owner_phases.clear();
        assert!(
            receipt
                .binds_reconciliation_snapshot(&missing_phase)
                .is_err(),
            "owner phases must partition every durable owner exactly once"
        );
        snapshot.commit_barriers[0].reservation_owner_hash =
            Hash::new(b"same-count-different-reservation-owner");
        assert!(
            !receipt
                .binds_reconciliation_snapshot(&snapshot)
                .expect("validate drifted reconciliation identity"),
            "equal owner cardinality cannot substitute another exact reservation identity"
        );
    }
    #[test]
    fn snapshot_replay_receipt_binds_exact_plan_tombstoned_marker_phase() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("snapshot-replay-plan-marker.norito");
        let limits = LaneQueueReservationJournalLimits::new(
            1024 * 1024,
            u64::from(u32::MAX),
            2 * 1024 * 1024,
            8,
        );
        let record = indexed_record(1);
        let key = record.key;
        let (mut journal, _replay, seal) =
            LaneQueueReservationJournal::open_with_limits(&path, limits)
                .expect("open checked journal");
        journal
            .consume_snapshot_replay_seal(seal)
            .expect("consume initial empty replay seal");
        journal.put_batch(vec![record]).expect("persist live owner");
        journal.commit(key).expect("persist commit owner");
        journal
            .plan_tombstoned(key)
            .expect("persist exact plan marker");
        drop(journal);
        let (mut journal, _replay, seal) =
            LaneQueueReservationJournal::open_with_limits(&path, limits)
                .expect("reopen marked owner");
        let receipt = journal
            .consume_snapshot_replay_seal(seal)
            .expect("consume marked replay seal");
        let snapshot = LaneQueueReservationReconciliationSnapshotV1 {
            commit_barriers: vec![key],
            ordered_owner_phases: vec![LaneQueueReservationRecoveryPhaseV1 {
                key,
                reservation_phase: LaneQueueReservationOwnerPhaseV6::CommitBarrier,
                queue_plan_phase: QueuePlanReservationPhaseV1::Tombstoned,
                plan_tombstone_marked: true,
            }],
            ..LaneQueueReservationReconciliationSnapshotV1::default()
        };
        assert!(
            receipt
                .binds_reconciliation_snapshot(&snapshot)
                .expect("bind exact marked phase")
        );
        let mut marked_live = snapshot.clone();
        marked_live.ordered_owner_phases[0].queue_plan_phase = QueuePlanReservationPhaseV1::Live;
        assert!(
            receipt.binds_reconciliation_snapshot(&marked_live).is_err(),
            "a durable V6 marker must conflict with a live V4 phase"
        );
        let mut dropped_marker = snapshot;
        dropped_marker.ordered_owner_phases[0].plan_tombstone_marked = false;
        assert!(
            !receipt
                .binds_reconciliation_snapshot(&dropped_marker)
                .expect("compare dropped marker"),
            "same owner and V4 phase cannot substitute a missing V6 marker"
        );
    }
    fn release_barrier(
        records: &[LaneQueueReservationRecordV5],
        release_seed: u8,
    ) -> LaneQueueReservationReleaseBarrierV3 {
        let first = records.first().expect("release fixture is non-empty");
        LaneQueueReservationReleaseBarrierV3 {
            version: LaneQueueReservationReleaseBarrierV3::VERSION,
            network_id: super::super::queue_test_network_id(),
            epoch: 3,
            lane_id: first.key.lane_id,
            dataspace_id: first.key.dataspace_id,
            lane_incarnation: first.key.lane_incarnation,
            proposal_height: first.key.proposal_height,
            lane_block_height: first.key.lane_block_height,
            lane_block_view: first.key.lane_block_view,
            origin_descriptor_hash: Hash::new([release_seed, 8]),
            origin_proposal_hash: Hash::new([release_seed, 9]),
            executable_payload_hash: Hash::new([release_seed, 10]),
            retirement_hash: Hash::new([release_seed, 11]),
            ordered_keys: records.iter().map(|record| record.key).collect(),
        }
    }
    fn release_completion(
        records: &[LaneQueueReservationRecordV5],
        release_seed: u8,
    ) -> LaneQueueReservationReleaseCompletionV5 {
        LaneQueueReservationReleaseCompletionV5 {
            version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
            barrier: release_barrier(records, release_seed),
            ordered_records: records.to_vec(),
        }
    }
    #[test]
    fn durable_frames_ignore_ambient_layout_and_survive_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("canonical-ambient-restart.norito");
        let expected = record(17, 3);
        let operation = LaneQueueReservationJournalFrameV6::PutBatch(vec![expected.clone()]);
        let canonical_payload =
            norito::encode_canonical(&operation).expect("encode canonical operation payload");
        let mut expected_file = encode_frame(&bootstrap_frame()).expect("encode bootstrap");
        expected_file.extend_from_slice(&encode_frame(&operation).expect("encode operation"));
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            let ambient_payload =
                norito::to_bytes(&operation).expect("encode alternate-layout ambient operation");
            assert_ne!(ambient_payload, canonical_payload);
            let (mut journal, replay) = LaneQueueReservationJournal::open(&path, u64::MAX)
                .expect("open under alternate ambient layout");
            assert!(replay.records().is_empty());
            journal
                .put_batch(vec![expected.clone()])
                .expect("persist under alternate ambient layout");
            drop(journal);
            assert_eq!(
                norito::to_bytes(&operation).expect("encode ambient operation after journal calls"),
                ambient_payload,
                "canonical journal helpers must restore the caller's ambient layout"
            );
        }
        assert_eq!(
            fs::read(&path).expect("read canonical journal bytes"),
            expected_file,
            "bootstrap, payload, checksum, and commit bytes must be ambient-invariant"
        );
        let (_journal, replay) =
            LaneQueueReservationJournal::open(&path, u64::MAX).expect("reopen canonical journal");
        assert_eq!(replay.records(), &[expected]);
    }
    include!("reservation_journal_codec_tests.rs");
    #[test]
    fn retired_v5_lane_wide_removal_fails_closed_at_bootstrap_and_operation_decode() {
        let retired = record(19, 4);
        let payload = norito::encode_canonical(
            &LaneQueueReservationJournalFrameV6RetiredRemovalFixture::RetiredLaneWideRemoval {
                lane_id: retired.key.lane_id,
                lane_incarnation: retired.key.lane_incarnation,
            },
        )
        .expect("encode retired V5 lane-wide removal fixture");
        let limits = LaneQueueReservationJournalLimits::new(
            u64::MAX,
            u64::from(u32::MAX),
            u64::MAX,
            usize::MAX,
        );
        let decode_error = decode_frame(&payload, limits)
            .expect_err("retired V5 operation tag must not decode as an ordered release");
        assert_eq!(decode_error.kind(), io::ErrorKind::InvalidData);
        let version = RESERVATION_JOURNAL_FRAME_FORMAT_VERSION;
        let version_bytes = version.to_le_bytes();
        let payload_len = u32::try_from(payload.len()).expect("retired payload length fits u32");
        let payload_len_bytes = payload_len.to_le_bytes();
        let payload_len_guard = (!payload_len).to_le_bytes();
        let payload_checksum = frame_checksum(
            &version_bytes,
            &payload_len_bytes,
            &payload_len_guard,
            &payload,
        );
        let mut retired_operation_frame = Vec::new();
        retired_operation_frame.extend_from_slice(&RESERVATION_JOURNAL_FRAME_MAGIC);
        retired_operation_frame.extend_from_slice(&version_bytes);
        retired_operation_frame.extend_from_slice(&payload_len_bytes);
        retired_operation_frame.extend_from_slice(&payload_len_guard);
        retired_operation_frame.extend_from_slice(&payload);
        retired_operation_frame.extend_from_slice(payload_checksum.as_ref());
        retired_operation_frame.extend_from_slice(&RESERVATION_JOURNAL_FRAME_COMMIT);
        let retired_bootstrap = LaneQueueReservationJournalFrameV6::Bootstrap {
            version,
            format_digest: Hash::new_from_chunks(&[
                RESERVATION_JOURNAL_BOOTSTRAP_DOMAIN,
                &RESERVATION_JOURNAL_FRAME_MAGIC,
                &version_bytes,
                &RESERVATION_JOURNAL_FRAME_COMMIT,
            ]),
        };
        let mut bytes =
            encode_frame(&retired_bootstrap).expect("encode retired V5 bootstrap fixture");
        bytes.extend_from_slice(&retired_operation_frame);
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("retired-v5-lane-wide-removal.norito");
        fs::write(&path, &bytes).expect("write retired V5 bootstrap fixture");
        let error = LaneQueueReservationJournal::open(&path, u64::MAX)
            .err()
            .expect("retired V5 schema digest must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("invalid V6 bootstrap claim"),
            "unexpected retired V5 bootstrap rejection: {error}"
        );
        assert_eq!(
            fs::read(&path).expect("retain retired V5 evidence"),
            bytes,
            "retired V5 evidence must not be repaired or rewritten"
        );
    }
    fn encode_v3_fixture_frame(frame: &LaneQueueReservationJournalFrameV3Fixture) -> Vec<u8> {
        let payload = norito::to_bytes(frame).expect("encode V3 reservation frame fixture");
        let len = u32::try_from(payload.len()).expect("bounded V3 fixture length");
        let len_bytes = len.to_le_bytes();
        let mut checksum_preimage = Vec::new();
        checksum_preimage.extend_from_slice(V3_RESERVATION_JOURNAL_FRAME_DOMAIN);
        checksum_preimage.extend_from_slice(&len_bytes);
        checksum_preimage.extend_from_slice(&payload);
        let checksum = Hash::new(checksum_preimage);
        let mut framed = Vec::new();
        framed.extend_from_slice(&V3_RESERVATION_JOURNAL_FRAME_MAGIC);
        framed.extend_from_slice(&len_bytes);
        framed.extend_from_slice(&payload);
        framed.extend_from_slice(checksum.as_ref());
        framed.extend_from_slice(&RESERVATION_JOURNAL_FRAME_COMMIT);
        framed
    }
    fn encode_v4_bootstrap_fixture() -> Vec<u8> {
        let version = 4_u16;
        let version_bytes = version.to_le_bytes();
        let frame = LaneQueueReservationJournalFrameV4Fixture::Bootstrap {
            version,
            format_digest: Hash::new_from_chunks(&[
                V4_RESERVATION_JOURNAL_BOOTSTRAP_DOMAIN,
                &V4_RESERVATION_JOURNAL_FRAME_MAGIC,
                &version_bytes,
                &RESERVATION_JOURNAL_FRAME_COMMIT,
            ]),
        };
        let payload = norito::to_bytes(&frame).expect("encode V4 reservation bootstrap fixture");
        let len = u32::try_from(payload.len()).expect("bounded V4 fixture length");
        let len_bytes = len.to_le_bytes();
        let len_guard = (!len).to_le_bytes();
        let checksum = Hash::new_from_chunks(&[
            V4_RESERVATION_JOURNAL_FRAME_DOMAIN,
            &version_bytes,
            &len_bytes,
            &len_guard,
            &payload,
        ]);
        let mut framed = Vec::new();
        framed.extend_from_slice(&V4_RESERVATION_JOURNAL_FRAME_MAGIC);
        framed.extend_from_slice(&version_bytes);
        framed.extend_from_slice(&len_bytes);
        framed.extend_from_slice(&len_guard);
        framed.extend_from_slice(&payload);
        framed.extend_from_slice(checksum.as_ref());
        framed.extend_from_slice(&RESERVATION_JOURNAL_FRAME_COMMIT);
        framed
    }
    #[test]
    fn reservation_record_rejects_legacy_or_zero_fifo_order_identity() {
        let mut records = Vec::new();
        let mut committed = Vec::new();
        let mut legacy = record(4, 1);
        legacy.fifo_order.version = LANE_QUEUE_RESERVATION_JOURNAL_VERSION - 1;
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV6::PutBatch(vec![legacy]),
            )
            .is_err()
        );
        let mut zero = record(5, 1);
        zero.fifo_order.ordinal = 0;
        assert!(
            apply_unprotected_frame(
                &mut records,
                &mut committed,
                LaneQueueReservationJournalFrameV6::PutBatch(vec![zero]),
            )
            .is_err()
        );
        assert!(records.is_empty());
    }
    // Release, recovery, and checked-transition tests retain the parent test path.
    include!("reservation_journal_recovery_tests.rs");
}
