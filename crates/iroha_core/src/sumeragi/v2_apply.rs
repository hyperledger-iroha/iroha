//! Idempotent production application of a durable Sumeragi v2 decision.
//!
//! A CommitQC is written to the safety WAL before this module is invoked. The
//! application transaction then re-loads the exact validated body, advances
//! Kura and WSV at most once, and finally persists the canonical v2 finality
//! sidecar. Restart may observe Kura/WSV already at the decided height while
//! the sidecar is absent; that state is completed without re-applying the
//! block or validating it against a later state.

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    sync::Arc,
    time::Duration,
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::{
        BlockHeader, CertifiedMergeLedgerReference, SignedBlock,
        consensus::{
            LaneBlockDescriptorV1, LaneBlockProposalPayloadHintV1, LaneBlockProposalV1,
            SumeragiLanePayloadOwnership,
        },
        consensus_v2 as wire,
    },
    events::EventBox,
    merge::MergeLedgerEntry,
    transaction::SignedTransaction,
};
use iroha_primitives::time::TimeSource;
use norito::codec::Encode;
use thiserror::Error;

use super::{
    message::CanonicalExecutedBlockNeedV1,
    network_topology::Topology,
    v2::VerifiedHeightContext,
    v2_body_store::{BodyValidationError, V2BodyStore, ValidatedBodyReceipt},
    v2_core::{
        CanonicalIdentityProjection, EventTag, IDENTITY_DOMAIN_CONTEXT,
        IDENTITY_DOMAIN_DURABLE_ARTIFACT, IDENTITY_DOMAIN_PAYLOAD, IDENTITY_DOMAIN_SUBJECT,
        IDENTITY_KIND_BLOCK_HEADER, IDENTITY_KIND_CANONICAL_PAYLOAD,
        IDENTITY_KIND_DURABLE_BODY_FRAME, IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
        IDENTITY_KIND_EXECUTION_COMMITMENT, IDENTITY_KIND_FINALITY_ARTIFACT,
        IDENTITY_KIND_PAYLOAD_MANIFEST, IDENTITY_KIND_QUORUM_CERTIFICATE,
        IDENTITY_KIND_WIRE_BLOCK_SUBJECT, IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        ProductionApplicationTraceProjection, ProductionDecisionIdentityProjection,
        ProductionDurableBodyIdentityProjection, ProductionQuorumCertificateIdentityProjection,
        TagProjection, check_production_application_transition,
    },
    v2_effects::{ApplyTask, DurableApplyCompletion, EffectWorkId},
};
use crate::{
    EventsSender,
    block::{BlockValidationError, ValidBlock},
    kura::{
        AutonomousLaneReservationEvidenceError, AutonomousLaneReservationEvidenceV1,
        AutonomousLaneSlotRetirementV1, CommitManifest, Kura, KuraV2CommitReceipt,
    },
    lane_consensus::LaneExecutablePayloadV1,
    queue::{
        LaneQueueReservationError, LaneQueueReservationGroupIdentityV1,
        LaneQueueReservationOutcome, LaneQueueReservationReconciliationGroupV1,
        LaneQueueReservationReconciliationSnapshotV1, LaneQueueReservationReleaseBarrierV3, Queue,
        RoutingDecision,
    },
    state::{MergeLedgerCommitError, MergeLedgerPublicationMode, State},
};

/// Fail-closed error while consuming or recovering durable lane reservations.
#[derive(Debug, Error)]
pub(crate) enum V2ReservationLifecycleError {
    /// A persisted height or collection size cannot be represented on this platform.
    #[error(transparent)]
    Integer(#[from] std::num::TryFromIntError),
    /// Canonical merge history could not be read.
    #[error(transparent)]
    Kura(#[from] crate::kura::Error),
    /// A committed merge batch contains malformed reservation evidence.
    #[error(transparent)]
    Merge(#[from] MergeLedgerCommitError),
    /// The reservation journal rejected an exact retain/release/commit operation.
    #[error(transparent)]
    Queue(#[from] LaneQueueReservationError),
    /// Exact Kura reservation evidence could not be classified under one stable snapshot.
    #[error(transparent)]
    AutonomousEvidence(#[from] AutonomousLaneReservationEvidenceError),
    /// Queue ownership changed while the read-only startup plan was being built.
    #[error("lane reservation ownership changed during startup reconciliation preflight")]
    QueueSnapshotChanged,
    /// A crash barrier could not reach its terminal durable Queue boundary.
    #[error("lane reservation {kind} barrier remains after exact startup reconciliation")]
    IncompleteQueueBarrier {
        /// Barrier family which did not complete.
        kind: &'static str,
    },
    /// The verified consensus height and committed State tip cannot describe one startup cut.
    #[error(
        "verified active height {active_height} is incompatible with committed State height {state_height}"
    )]
    ActiveHeightMismatch {
        /// Verified height context selected by startup recovery.
        active_height: u64,
        /// Authoritative committed WSV height.
        state_height: u64,
    },
    /// A replayed reservation names a proposal height beyond the verified startup context.
    #[error(
        "autonomous reservation proposal height {proposal_height} is newer than verified active height {active_height}"
    )]
    FutureReservation {
        /// Reservation proposal height.
        proposal_height: u64,
        /// Verified startup height.
        active_height: u64,
    },
    /// Historical route/incarnation state differs from the durable reservation identity.
    #[error(
        "autonomous reservation for lane {lane_id:?} at proposal height {proposal_height} has a stale route or incarnation"
    )]
    StaleReservationContext {
        /// Coordinator lane.
        lane_id: iroha_data_model::nexus::LaneId,
        /// Historical proposal height.
        proposal_height: u64,
    },
    /// Only part of one atomic reservation group appears in committed State.
    #[error(
        "autonomous reservation group for lane {lane_id:?} at proposal height {proposal_height} is only partially committed in State"
    )]
    PartialCommittedGroup {
        /// Coordinator lane.
        lane_id: iroha_data_model::nexus::LaneId,
        /// Historical proposal height.
        proposal_height: u64,
    },
    /// A replayed Queue commit barrier lacks independently authenticated committed membership.
    #[error(
        "reservation commit barrier transaction {transaction_hash} is absent from committed State"
    )]
    UncommittedCommitBarrier {
        /// Transaction retained by the commit barrier.
        transaction_hash: HashOf<SignedTransaction>,
    },
    /// A committed reservation resolves to more than one carrier or to a different group carrier.
    #[error(
        "committed autonomous reservation group for lane {lane_id:?} at proposal height {proposal_height} has inconsistent carrier heights"
    )]
    CommittedCarrierMismatch {
        /// Coordinator lane.
        lane_id: iroha_data_model::nexus::LaneId,
        /// Historical proposal height.
        proposal_height: u64,
    },
    /// The canonical transaction index required for exact committed recovery is unavailable.
    #[error(
        "canonical transaction index is unavailable for committed transaction {transaction_hash}"
    )]
    CommittedTransactionIndexUnavailable {
        /// Committed transaction whose carrier cannot be resolved exactly.
        transaction_hash: HashOf<SignedTransaction>,
    },
    /// A pending certified merge entry contains a partial, reordered, or split reservation group.
    #[error(
        "pending certified merge evidence conflicts with the autonomous reservation group for lane {lane_id:?} at proposal height {proposal_height}"
    )]
    PendingMergeBindingMismatch {
        /// Coordinator lane.
        lane_id: iroha_data_model::nexus::LaneId,
        /// Historical proposal height.
        proposal_height: u64,
    },
    /// A finalized proposal height has no independently verified finality artifact.
    #[error("canonical proposal height {height} is missing verified Sumeragi v2 finality")]
    MissingCanonicalFinality {
        /// Finalized proposal height.
        height: u64,
    },
    /// The finality-authenticated canonical block body is locally unavailable.
    #[error(
        "canonical proposal height {height} has no retained block body; authenticated historical recovery is required"
    )]
    MissingCanonicalBody {
        /// Finalized proposal height.
        height: u64,
    },
    /// Canonical finality/body context differs from the reservation's verified chain or epoch.
    #[error(
        "canonical proposal height {height} has a conflicting chain, epoch, header, or hash binding"
    )]
    CanonicalContextMismatch {
        /// Conflicting finalized height.
        height: u64,
    },
    /// A canonical autonomous envelope is malformed even though its block finalized.
    #[error("canonical autonomous payload envelope at height {height} is invalid: {detail}")]
    InvalidCanonicalEnvelope {
        /// Finalized carrier height.
        height: u64,
        /// Exact decoder failure.
        detail: String,
    },
    /// A canonical ordinary lane-ownership anchor cannot reconstruct its exact proposal.
    #[error("canonical lane payload ownership at height {height} is invalid")]
    InvalidCanonicalOwnership {
        /// Finalized carrier height.
        height: u64,
    },
    /// The canonical body carries another attempt at the reservation's exact slot.
    #[error(
        "canonical block at height {height} carries a conflicting autonomous attempt for lane {lane_id:?}"
    )]
    CanonicalAttemptConflict {
        /// Canonical global height.
        height: u64,
        /// Conflicting coordinator lane.
        lane_id: iroha_data_model::nexus::LaneId,
    },
    /// Canonical payload bytes exist but the exact local Kura payload disappeared.
    #[error(
        "canonical autonomous carrier at height {height} has no exact local Kura payload for lane {lane_id:?}"
    )]
    CanonicalCarrierMissingKuraPayload {
        /// Canonical global height.
        height: u64,
        /// Coordinator lane.
        lane_id: iroha_data_model::nexus::LaneId,
    },
    /// Queue publication reached an historical retain action without the exact
    /// durable recovery record installed during the preceding planning pass.
    #[error(
        "historical autonomous recovery {recovery_id} for lane {lane_id:?} is not durably installed"
    )]
    HistoricalRecoveryInstallationMissing {
        /// Immutable recovery-record identity.
        recovery_id: Hash,
        /// Coordinator lane.
        lane_id: iroha_data_model::nexus::LaneId,
    },
    /// Lane-local certification forbids treating an absent canonical payload as a loser.
    #[error(
        "certified autonomous payload for lane {lane_id:?} at height {height} cannot be released as a terminal loser"
    )]
    CertifiedTerminalLoser {
        /// Coordinator lane.
        lane_id: iroha_data_model::nexus::LaneId,
        /// Original proposal height.
        height: u64,
    },
    /// Certification or pending merge evidence exists before an exact canonical anchor.
    #[error(
        "certified autonomous payload for lane {lane_id:?} at height {height} has no finalized exact canonical carrier"
    )]
    CertifiedPayloadMissingCanonicalCarrier {
        /// Coordinator lane.
        lane_id: iroha_data_model::nexus::LaneId,
        /// Original proposal height.
        height: u64,
    },
    /// A durable retirement conflicts with the exact canonical payload carrier.
    #[error(
        "retired autonomous payload for lane {lane_id:?} remains present in canonical block {height}"
    )]
    RetiredCanonicalCarrier {
        /// Coordinator lane.
        lane_id: iroha_data_model::nexus::LaneId,
        /// Canonical global height.
        height: u64,
    },
    /// A retirement exists before its proposal height has a canonical decision.
    #[error(
        "autonomous retirement for lane {lane_id:?} at height {height} has no finalized canonical carrier"
    )]
    UnfinalizedRetirement {
        /// Coordinator lane.
        lane_id: iroha_data_model::nexus::LaneId,
        /// Undecided proposal height.
        height: u64,
    },
    /// A release barrier would return an already committed transaction to FIFO ownership.
    #[error("release barrier transaction {transaction_hash} is already committed in State")]
    ReleaseBarrierCommittedTransaction {
        /// Conflicting transaction.
        transaction_hash: HashOf<SignedTransaction>,
    },
    /// A durable Queue release barrier is not the exact Kura retirement projection.
    #[error("queue release barrier {retirement_hash} has invalid group membership")]
    InvalidReleaseBarrierGroup {
        /// Digest of the conflicting retirement.
        retirement_hash: Hash,
    },
    /// Committed State retains a reservation without matching merge evidence.
    #[error("committed transaction {transaction_hash} has no exact durable merge reservation")]
    MissingCommittedBinding {
        /// Committed transaction whose reservation cannot be authenticated.
        transaction_hash: HashOf<SignedTransaction>,
    },
    /// Journal ownership differs from the committed merge evidence.
    #[error("committed transaction {transaction_hash} has a conflicting live reservation")]
    CommittedBindingMismatch {
        /// Committed transaction with mismatched reservation ownership.
        transaction_hash: HashOf<SignedTransaction>,
    },
    /// A merge entry names a transaction that State did not commit.
    #[error("merge reservation transaction {transaction_hash} is absent from committed State")]
    UncommittedMergeTransaction {
        /// Transaction missing from committed membership.
        transaction_hash: HashOf<SignedTransaction>,
    },
    /// The canonical carrier lost its exact full merge entry.
    #[error("committed merge carrier lost sidecar {entry_hash}")]
    MissingCommittedMergeEntry {
        /// Hash committed by the carrier's compact reference.
        entry_hash: HashOf<MergeLedgerEntry>,
    },
    /// The full entry no longer matches the carrier's compact projection.
    #[error("committed merge sidecar {entry_hash} differs from its carrier reference")]
    CommittedMergeReferenceMismatch {
        /// Hash committed by the carrier's compact reference.
        entry_hash: HashOf<MergeLedgerEntry>,
    },
    /// Queue retained a release barrier whose exact Kura retirement is absent.
    #[error("queue release barrier {retirement_hash} has no exact durable Kura retirement")]
    MissingReleaseRetirement {
        /// Digest of the missing retirement identity.
        retirement_hash: Hash,
    },
    /// Queue and Kura disagree on a release barrier's full slot/payload binding.
    #[error("queue release barrier {retirement_hash} conflicts with durable Kura retirement")]
    ReleaseRetirementMismatch {
        /// Digest of the conflicting retirement identity.
        retirement_hash: Hash,
    },
}

/// Typed startup disposition counts for durable autonomous reservation owners.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LaneReservationReconciliationSummary {
    /// Unique durable reservation owners observed across live and crash-barrier state.
    pub(crate) recovered: usize,
    /// Applied owners consumed through exact committed merge evidence.
    pub(crate) finalized_committed: usize,
    /// Current-height live payload owners retained for ordinary lane recovery.
    pub(crate) retained_current: usize,
    /// Canonically anchored owners retained because lane-local certification is durable.
    pub(crate) retained_certified: usize,
    /// Owners retained by one exact uncommitted certified merge sidecar.
    pub(crate) retained_pending_merge: usize,
    /// Historical canonical owners retained behind installed recovery work.
    pub(crate) retained_historical_recovery: usize,
    /// Strictly absent owners returned directly to global FIFO.
    pub(crate) released_strictly_absent: usize,
    /// Exact losing payload owners retired through Kura before FIFO release.
    pub(crate) released_terminal_loser: usize,
    /// Owners whose previously durable retirement/release hand-off was resumed.
    pub(crate) resumed_retirement: usize,
}

fn finalize_certified_merge_reservations(
    state: &State,
    queue: &Queue,
    entry: &MergeLedgerEntry,
) -> Result<usize, V2ReservationLifecycleError> {
    let reservations = crate::state::certified_merge_queue_reservations(entry)?;
    let mut finalized = 0usize;
    for (transaction_hash, reservation) in reservations {
        if !state.has_committed_transaction(transaction_hash) {
            return Err(V2ReservationLifecycleError::UncommittedMergeTransaction {
                transaction_hash,
            });
        }
        if queue.commit_lane_reservation(&reservation)? == LaneQueueReservationOutcome::Finalized {
            finalized = finalized.saturating_add(1);
        }
    }
    Ok(finalized)
}

fn committed_block_merge_entry(
    kura: &Kura,
    block: &SignedBlock,
) -> Result<Option<MergeLedgerEntry>, V2ReservationLifecycleError> {
    let Some(reference) = block
        .execution_context()
        .and_then(|bundle| bundle.merge_entry.as_ref())
    else {
        return Ok(None);
    };
    let entry = kura.merge_entry_by_hash(reference.entry_hash)?.ok_or(
        V2ReservationLifecycleError::MissingCommittedMergeEntry {
            entry_hash: reference.entry_hash,
        },
    )?;
    if !reference.matches_entry(&entry) {
        return Err(
            V2ReservationLifecycleError::CommittedMergeReferenceMismatch {
                entry_hash: reference.entry_hash,
            },
        );
    }
    Ok(Some(entry))
}

fn finalize_committed_block_merge_reservations(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    block: &SignedBlock,
) -> Result<usize, V2ReservationLifecycleError> {
    let Some(entry) = committed_block_merge_entry(kura, block)? else {
        return Ok(0);
    };
    finalize_certified_merge_reservations(state, queue, &entry)
}

/// Execute or resume the complete crash-safe retirement/release hand-off.
///
/// This is the single production ordering implementation shared by live lane
/// work and startup reconciliation:
///
/// 1. Kura persists the exact slot retirement and `ReleasePending` claims.
/// 2. Queue persists the exact ordered barrier while reservations remain live.
/// 3. Kura changes the exact claims to `Released`.
/// 4. Queue completes ownership transfer, restores FIFO order, and forgets the
///    replay barrier.
pub(crate) fn retire_autonomous_lane_slot_and_release_reservations(
    kura: &Kura,
    queue: &Queue,
    retirement: &crate::kura::AutonomousLaneSlotRetirementV1,
    expected_chain_id_hash: Hash,
    expected_epoch: u64,
) -> Result<usize, V2ReservationLifecycleError> {
    kura.persist_autonomous_lane_slot_retirement(
        retirement,
        expected_chain_id_hash,
        expected_epoch,
    )?;
    let barrier = retirement.queue_release_barrier()?;
    let _ = queue.prepare_lane_reservation_release_barrier(&barrier)?;
    kura.finalize_autonomous_lane_slot_release(
        retirement,
        &barrier,
        expected_chain_id_hash,
        expected_epoch,
    )?;
    Ok(queue.finalize_lane_reservation_release_barrier(&barrier)?)
}

fn reservation_group_identity(
    key: &crate::queue::LaneQueueReservationKeyV2,
) -> LaneQueueReservationGroupIdentityV1 {
    LaneQueueReservationGroupIdentityV1 {
        lane_id: key.lane_id,
        dataspace_id: key.dataspace_id,
        lane_incarnation: key.lane_incarnation,
        proposal_height: key.proposal_height,
        lane_block_height: key.lane_block_height,
        lane_block_view: key.lane_block_view,
        reservation_owner_hash: key.reservation_owner_hash,
        proposal_identity_hash: key.proposal_identity_hash,
    }
}

fn reservation_key_matches_group(
    key: &crate::queue::LaneQueueReservationKeyV2,
    group: &LaneQueueReservationGroupIdentityV1,
) -> bool {
    reservation_group_identity(key) == *group
}

fn canonical_payload_contains_group_in_order(
    payload: &LaneExecutablePayloadV1,
    group: &LaneQueueReservationReconciliationGroupV1,
) -> bool {
    payload.reservation_keys == group.ordered_keys
}

fn autonomous_payload_overlaps_group_transaction_identity(
    payload: &LaneExecutablePayloadV1,
    group: &LaneQueueReservationReconciliationGroupV1,
) -> bool {
    payload.reservation_keys.iter().any(|candidate| {
        group.ordered_keys.iter().any(|expected| {
            candidate.signed_transaction_hash == expected.signed_transaction_hash
                || candidate.entrypoint_hash == expected.entrypoint_hash
        })
    })
}

fn proposal_from_canonical_lane_ownership(
    ownership: &SumeragiLanePayloadOwnership,
    block_hash: HashOf<BlockHeader>,
) -> Option<LaneBlockProposalV1> {
    let descriptor_hash = ownership.lane_block_descriptor_hash?;
    let descriptor = LaneBlockDescriptorV1 {
        lane_id: ownership.lane_id,
        dataspace_id: ownership.dataspace_id,
        lane_incarnation: ownership.lane_incarnation,
        proposal_height: ownership.proposal_height,
        previous_lane_block_height: ownership.previous_lane_block_height,
        previous_lane_block_descriptor_hash: ownership.previous_lane_block_descriptor_hash,
        lane_block_height: ownership.lane_block_height,
        lane_block_view: ownership.lane_block_view,
        subject_hash: ownership.subject_hash,
        payload_ownership_hash: ownership.payload_ownership_hash,
        rbc_instance_hash: ownership.rbc_instance_hash,
        accepted_candidate_indices: ownership.accepted_candidate_indices.clone(),
        accepted_transaction_hashes: ownership.accepted_transaction_hashes.clone(),
        validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&ownership.lane_block_descriptor_validator_set),
        validator_set: ownership.lane_block_descriptor_validator_set.clone(),
        validator_count: ownership.lane_block_descriptor_validator_count,
        min_quorum: ownership.lane_block_descriptor_min_quorum,
        qc_mode_tag: ownership.qc_mode_tag.clone(),
        descriptor_hash,
    };
    if descriptor.computed_descriptor_hash() != descriptor_hash {
        return None;
    }
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: Some(LaneBlockProposalPayloadHintV1 {
            proposal_height: ownership.proposal_height,
            proposal_view: ownership.proposal_view,
            proposal_block_hash: block_hash,
        }),
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    Some(proposal)
}

/// Immutable, finality-authenticated installation input for one unfinished
/// historical autonomous lane proposal.
///
/// This Encode-only value is an in-memory planner DTO, not a durable schema.
/// Kura publishes a separately versioned, decodeable recovery record only
/// after the referenced executable payload, historical PoPs, and execution
/// input are validated and durable. The Queue startup gate may then treat an
/// exact durable-record read-back as a persistent owner which the lane adapter
/// can hydrate after publication resumes.
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
pub(crate) struct HistoricalAutonomousReservationInstallV1 {
    /// Schema version of the installation identity.
    pub(crate) version: u16,
    /// Domain-separated digest of every remaining field.
    pub(crate) recovery_id: Hash,
    /// Exact finality/execution identity of the canonical global carrier.
    pub(crate) canonical_body: CanonicalExecutedBlockNeedV1,
    /// Complete frozen consensus context which authenticated the carrier.
    pub(crate) historical_context: wire::HeightContext,
    /// Redundant context identifier used by reservation-identity validation.
    pub(crate) historical_context_id: wire::HeightContextId,
    /// Hash of the complete historical context, including its roster. Validator
    /// PoPs are not carried by `HeightContext` and must be pinned separately by
    /// the durable installer.
    pub(crate) historical_context_hash: HashOf<wire::HeightContext>,
    /// Canonical global view of the carrier block.
    pub(crate) carrier_view: u64,
    /// Exact producer-authenticated payload, with its canonical global hint.
    pub(crate) payload: LaneExecutablePayloadV1,
    /// Exact FIFO-ordered Queue ownership group carried by the payload.
    pub(crate) reservation_group: LaneQueueReservationReconciliationGroupV1,
}

impl HistoricalAutonomousReservationInstallV1 {
    pub(crate) const VERSION: u16 = 1;
    const DIGEST_DOMAIN: &'static [u8] =
        b"iroha:sumeragi:historical-autonomous-reservation-recovery:v1\0";

    fn new(
        canonical_body: CanonicalExecutedBlockNeedV1,
        historical_context: wire::HeightContext,
        carrier_view: u64,
        payload: LaneExecutablePayloadV1,
        reservation_group: LaneQueueReservationReconciliationGroupV1,
    ) -> Self {
        let historical_context_id = historical_context.id();
        let historical_context_hash = HashOf::new(&historical_context);
        let mut install = Self {
            version: Self::VERSION,
            recovery_id: Hash::prehashed([0; Hash::LENGTH]),
            canonical_body,
            historical_context,
            historical_context_id,
            historical_context_hash,
            carrier_view,
            payload,
            reservation_group,
        };
        install.recovery_id = install.computed_recovery_id();
        install
    }

    /// Recompute the exact immutable record identity. Kura must reject any
    /// installation whose stored identity differs from this value.
    #[must_use]
    pub(crate) fn computed_recovery_id(&self) -> Hash {
        let mut canonical = self.clone();
        canonical.recovery_id = Hash::prehashed([0; Hash::LENGTH]);
        let identity: Hash = HashOf::new(&canonical).into();
        Hash::new_from_chunks(&[Self::DIGEST_DOMAIN, identity.as_ref()])
    }

    #[must_use]
    pub(crate) fn has_valid_identity(&self) -> bool {
        self.version == Self::VERSION
            && self.historical_context.id() == self.historical_context_id
            && HashOf::new(&self.historical_context) == self.historical_context_hash
            && self.computed_recovery_id() == self.recovery_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CanonicalAutonomousCarrierDisposition {
    NotFinalized,
    Absent,
    /// A unique canonical autonomous envelope contains the complete executable
    /// payload needed to install historical certification work.
    ExactAutonomous(HistoricalAutonomousReservationInstallV1),
    /// The canonical body contains only ordinary ownership. This authenticates
    /// the proposal but cannot reconstruct autonomous executable bytes.
    ExactOrdinary,
}

impl CanonicalAutonomousCarrierDisposition {
    fn is_exact(&self) -> bool {
        matches!(self, Self::ExactAutonomous(_) | Self::ExactOrdinary)
    }

    fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
}

enum CanonicalAutonomousCarrierInspection {
    Available(CanonicalAutonomousCarrierDisposition),
    MissingBody(CanonicalExecutedBlockNeedV1),
}

fn collect_canonical_executed_block_need(
    needs: &mut BTreeMap<u64, CanonicalExecutedBlockNeedV1>,
    need: CanonicalExecutedBlockNeedV1,
) -> Result<(), V2ReservationLifecycleError> {
    match needs.entry(need.height) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(need);
        }
        std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &need => {}
        std::collections::btree_map::Entry::Occupied(_) => {
            return Err(V2ReservationLifecycleError::CanonicalContextMismatch {
                height: need.height,
            });
        }
    }
    Ok(())
}

/// Revalidate the exact finalized proposal body before using envelope absence
/// as terminal-loser evidence. A retained header/hash alone never proves that
/// the canonical body omitted a reservation group.
fn canonical_autonomous_carrier_disposition(
    state: &State,
    kura: &Kura,
    active_context: &wire::HeightContext,
    state_height: u64,
    chain_hash: Hash,
    expected_epoch: u64,
    group: &LaneQueueReservationReconciliationGroupV1,
    expected_payload: Option<&LaneExecutablePayloadV1>,
) -> Result<CanonicalAutonomousCarrierInspection, V2ReservationLifecycleError> {
    let height = group.identity.proposal_height;
    if height > state_height {
        return Ok(CanonicalAutonomousCarrierInspection::Available(
            CanonicalAutonomousCarrierDisposition::NotFinalized,
        ));
    }
    let (retained_header, finality) = kura
        .v2_finality_artifact_with_header(height)?
        .ok_or(V2ReservationLifecycleError::MissingCanonicalFinality { height })?;
    let execution_commitment = finality.commit_qc.execution_commitment;
    if finality.height_context.chain_id != active_context.chain_id
        || finality.height_context.epoch != expected_epoch
        || finality.verify().is_err()
        || finality.validate_for_header(&retained_header).is_err()
        || finality.height != height
        || retained_header.height().get() != height
        || finality.block_hash != retained_header.hash()
        || state.committed_block_hash_at_height(height) != Some(finality.block_hash)
        || (height > 1
            && state.committed_block_hash_at_height(height - 1)
                != retained_header.prev_block_hash())
        || (height == 1 && retained_header.prev_block_hash().is_some())
        || Hash::new(
            finality
                .height_context
                .chain_id
                .clone()
                .into_inner()
                .as_bytes(),
        ) != chain_hash
        || execution_commitment.validate().is_err()
    {
        return Err(V2ReservationLifecycleError::CanonicalContextMismatch { height });
    }
    let need = CanonicalExecutedBlockNeedV1 {
        height,
        block_hash: finality.block_hash,
        finality_artifact_hash: HashOf::new(&finality),
        execution_commitment,
        executed_block_wire_hash: execution_commitment.executed_block_wire_hash,
    };
    let block_height = NonZeroUsize::new(usize::try_from(height)?)
        .ok_or(V2ReservationLifecycleError::MissingCanonicalBody { height })?;
    let Some(block) = kura.get_block_without_merge_sidecar(block_height) else {
        return Ok(CanonicalAutonomousCarrierInspection::MissingBody(need));
    };
    let executed_block_wire = block
        .encode_wire()
        .map_err(|_| V2ReservationLifecycleError::CanonicalContextMismatch { height })?;
    let executed_block_wire_len = u64::try_from(executed_block_wire.len())
        .map_err(|_| V2ReservationLifecycleError::CanonicalContextMismatch { height })?;
    let executed_block_wire_hash = Hash::new(&executed_block_wire);
    if finality.height_context.chain_id != active_context.chain_id
        || finality.height_context.epoch != expected_epoch
        || finality.height != height
        || finality.block_hash != block.hash()
        || retained_header != block.header()
        || executed_block_wire_len != execution_commitment.executed_block_wire_len
        || executed_block_wire_hash != need.executed_block_wire_hash
        || Hash::new(
            finality
                .height_context
                .chain_id
                .clone()
                .into_inner()
                .as_bytes(),
        ) != chain_hash
    {
        return Err(V2ReservationLifecycleError::CanonicalContextMismatch { height });
    }

    let mut exact_autonomous = None;
    let mut exact_ordinary = false;
    let Some(bundle) = block.execution_context() else {
        return Ok(CanonicalAutonomousCarrierInspection::Available(
            CanonicalAutonomousCarrierDisposition::Absent,
        ));
    };
    for envelope in &bundle.autonomous_lane_payloads {
        let payload = crate::lane_consensus::decode_autonomous_lane_payload_envelope(
            envelope,
            chain_hash,
            expected_epoch,
        )
        .map_err(
            |error| V2ReservationLifecycleError::InvalidCanonicalEnvelope {
                height,
                detail: error.to_string(),
            },
        )?;
        let descriptor = &payload.origin_proposal.descriptor;
        let same_slot = descriptor.lane_id == group.identity.lane_id
            && descriptor.dataspace_id == group.identity.dataspace_id
            && descriptor.lane_incarnation == group.identity.lane_incarnation
            && descriptor.proposal_height == group.identity.proposal_height
            && descriptor.lane_block_height == group.identity.lane_block_height
            && descriptor.lane_block_view == group.identity.lane_block_view;
        let overlaps_group =
            autonomous_payload_overlaps_group_transaction_identity(&payload, group);
        if !same_slot && !overlaps_group {
            continue;
        }

        let payload_matches = match expected_payload {
            Some(expected) => {
                if let Some(hint) = expected.origin_proposal.payload_block_hint
                    && (hint.proposal_height != height
                        || hint.proposal_view != block.header().view_change_index()
                        || hint.proposal_block_hash != block.hash())
                {
                    return Err(V2ReservationLifecycleError::CanonicalAttemptConflict {
                        height,
                        lane_id: group.identity.lane_id,
                    });
                }
                let mut normalized = expected.clone();
                normalized.origin_proposal.payload_block_hint = None;
                payload == normalized && payload.reservation_keys == group.ordered_keys
            }
            None => canonical_payload_contains_group_in_order(&payload, group),
        };
        if !same_slot || !payload_matches || exact_autonomous.is_some() || exact_ordinary {
            return Err(V2ReservationLifecycleError::CanonicalAttemptConflict {
                height,
                lane_id: group.identity.lane_id,
            });
        }
        let hint = LaneBlockProposalPayloadHintV1 {
            proposal_height: height,
            proposal_view: block.header().view_change_index(),
            proposal_block_hash: block.hash(),
        };
        let anchored = payload
            .attach_global_hint_exact(hint, chain_hash, expected_epoch)
            .map_err(
                |error| V2ReservationLifecycleError::InvalidCanonicalEnvelope {
                    height,
                    detail: error.to_string(),
                },
            )?;
        let (reservation_owner_hash, proposal_identity_hash) =
            crate::sumeragi::lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal(
                chain_hash,
                finality.height_context.id(),
                expected_epoch,
                &anchored.origin_proposal,
                &anchored.producer,
            )
            .map_err(
                |error| V2ReservationLifecycleError::InvalidCanonicalEnvelope {
                    height,
                    detail: error.to_string(),
                },
            )?;
        if anchored.reservation_keys.iter().any(|key| {
            key.reservation_owner_hash != reservation_owner_hash
                || key.proposal_identity_hash != proposal_identity_hash
        }) {
            return Err(V2ReservationLifecycleError::CanonicalAttemptConflict {
                height,
                lane_id: group.identity.lane_id,
            });
        }
        exact_autonomous = Some(HistoricalAutonomousReservationInstallV1::new(
            need,
            finality.height_context.clone(),
            block.header().view_change_index(),
            anchored,
            group.clone(),
        ));
    }
    let group_entrypoint_hashes = group
        .ordered_keys
        .iter()
        .map(|key| Hash::from(key.entrypoint_hash))
        .collect::<Vec<_>>();
    for ownership in &bundle.lane_payload_ownerships {
        let same_slot = ownership.lane_id == group.identity.lane_id
            && ownership.dataspace_id == group.identity.dataspace_id
            && ownership.lane_incarnation == group.identity.lane_incarnation
            && ownership.proposal_height == group.identity.proposal_height
            && ownership.lane_block_height == group.identity.lane_block_height
            && ownership.lane_block_view == group.identity.lane_block_view;
        let overlaps_group = ownership
            .accepted_transaction_hashes
            .iter()
            .any(|hash| group_entrypoint_hashes.contains(hash));
        if !same_slot && !overlaps_group {
            continue;
        }

        let Some(proposal) = proposal_from_canonical_lane_ownership(ownership, block.hash()) else {
            return Err(V2ReservationLifecycleError::InvalidCanonicalOwnership { height });
        };
        let ownership_matches = match expected_payload {
            Some(expected) => {
                let mut hint_neutral = expected.clone();
                if let Some(existing_hint) = hint_neutral.origin_proposal.payload_block_hint {
                    if existing_hint.proposal_height != height
                        || existing_hint.proposal_view != block.header().view_change_index()
                        || existing_hint.proposal_block_hash != block.hash()
                    {
                        return Err(V2ReservationLifecycleError::CanonicalAttemptConflict {
                            height,
                            lane_id: group.identity.lane_id,
                        });
                    }
                    hint_neutral.origin_proposal.payload_block_hint = None;
                }
                let hint = proposal
                    .payload_block_hint
                    .expect("canonical ownership reconstruction always attaches a hint");
                let anchored = hint_neutral
                    .attach_global_hint_exact(hint, chain_hash, expected_epoch)
                    .map_err(
                        |error| V2ReservationLifecycleError::InvalidCanonicalEnvelope {
                            height,
                            detail: error.to_string(),
                        },
                    )?;
                anchored.origin_proposal == proposal
                    && expected.entrypoint_hashes == ownership.accepted_transaction_hashes
                    && expected.reservation_keys == group.ordered_keys
            }
            None => ownership.accepted_transaction_hashes == group_entrypoint_hashes,
        };
        if !same_slot || !ownership_matches || exact_autonomous.is_some() || exact_ordinary {
            return Err(V2ReservationLifecycleError::CanonicalAttemptConflict {
                height,
                lane_id: group.identity.lane_id,
            });
        }
        exact_ordinary = true;
    }
    Ok(CanonicalAutonomousCarrierInspection::Available(
        if let Some(install) = exact_autonomous {
            CanonicalAutonomousCarrierDisposition::ExactAutonomous(install)
        } else if exact_ordinary {
            CanonicalAutonomousCarrierDisposition::ExactOrdinary
        } else {
            CanonicalAutonomousCarrierDisposition::Absent
        },
    ))
}

#[derive(Clone)]
struct ReservationReconciliationGroupInput {
    group: LaneQueueReservationReconciliationGroupV1,
    /// Queue owners which remain after any earlier per-key Commit/ForgetCommit
    /// transitions. Committed groups replace `group.ordered_keys` with the
    /// complete canonical MergeLedger membership during read-only preflight,
    /// but mutate only this observed subset.
    owned_keys: Vec<crate::queue::LaneQueueReservationKeyV2>,
    release_barrier: Option<LaneQueueReservationReleaseBarrierV3>,
    committed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ReservationRetainDisposition {
    Current,
    Certified,
    PendingMerge,
    HistoricalRecovery(HistoricalAutonomousReservationInstallV1),
}

enum ReservationReconciliationAction {
    Commit(Vec<crate::queue::LaneQueueReservationKeyV2>),
    Retain {
        keys: Vec<crate::queue::LaneQueueReservationKeyV2>,
        disposition: ReservationRetainDisposition,
    },
    Retire {
        retirement: AutonomousLaneSlotRetirementV1,
        epoch: u64,
        resumed: bool,
    },
    DirectRelease,
}

/// Read-only startup classification result.
///
/// A recovery outcome contains only immutable, finality-authenticated body
/// identities. A ready plan owns every Queue mutation input and can be
/// applied only after its exact Queue snapshot is revalidated.
pub(crate) enum LaneReservationReconciliationPlanning {
    /// Every dependency is locally durable; mutation may begin under the
    /// process fail-stop operation.
    Ready(LaneReservationReconciliationPlan),
    /// One or more canonical executed bodies must be recovered first.
    RecoverCanonicalBodies(Vec<CanonicalExecutedBlockNeedV1>),
    /// Exact historical autonomous work must cross its durable Kura/State
    /// installation boundary before Queue publication may resume.
    InstallHistoricalAutonomousRecoveries(Vec<HistoricalAutonomousReservationInstallV1>),
}

/// Complete immutable mutation plan for one Queue/Kura/State startup cut.
pub(crate) struct LaneReservationReconciliationPlan {
    snapshot: LaneQueueReservationReconciliationSnapshotV1,
    release_barriers: Vec<LaneQueueReservationReleaseBarrierV3>,
    commit_barriers: Vec<crate::queue::LaneQueueReservationKeyV2>,
    actions: Vec<ReservationReconciliationAction>,
    direct_release: Vec<crate::queue::LaneQueueReservationKeyV2>,
    chain_hash: Hash,
    recovered: usize,
}

fn exact_pending_merge_for_group(
    group: &LaneQueueReservationReconciliationGroupV1,
    pending_by_transaction: &BTreeMap<
        HashOf<SignedTransaction>,
        (
            HashOf<MergeLedgerEntry>,
            crate::queue::LaneQueueReservationKeyV2,
        ),
    >,
    pending_by_entry: &BTreeMap<
        HashOf<MergeLedgerEntry>,
        Vec<crate::queue::LaneQueueReservationKeyV2>,
    >,
) -> Result<bool, V2ReservationLifecycleError> {
    let mut entry_hash = None;
    let mut matched = 0usize;
    for key in &group.ordered_keys {
        let Some((candidate_hash, pending_key)) =
            pending_by_transaction.get(&key.signed_transaction_hash)
        else {
            continue;
        };
        if pending_key != key || entry_hash.is_some_and(|existing| existing != *candidate_hash) {
            return Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
                lane_id: group.identity.lane_id,
                proposal_height: group.identity.proposal_height,
            });
        }
        entry_hash = Some(*candidate_hash);
        matched = matched.saturating_add(1);
    }
    if matched == 0 {
        return Ok(false);
    }
    if matched != group.ordered_keys.len() {
        return Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
            lane_id: group.identity.lane_id,
            proposal_height: group.identity.proposal_height,
        });
    }
    let entry_hash = entry_hash.expect("a matched pending group has one entry hash");
    let exact_group = pending_by_entry
        .get(&entry_hash)
        .into_iter()
        .flatten()
        .filter(|key| reservation_key_matches_group(key, &group.identity))
        .copied()
        .collect::<Vec<_>>();
    if exact_group != group.ordered_keys {
        return Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
            lane_id: group.identity.lane_id,
            proposal_height: group.identity.proposal_height,
        });
    }
    Ok(true)
}

fn exact_committed_carrier_height_for_group(
    group: &LaneQueueReservationReconciliationGroupV1,
    carrier_heights: &[BTreeSet<NonZeroUsize>],
) -> Result<NonZeroUsize, V2ReservationLifecycleError> {
    if carrier_heights.len() != group.ordered_keys.len() {
        return Err(V2ReservationLifecycleError::CommittedCarrierMismatch {
            lane_id: group.identity.lane_id,
            proposal_height: group.identity.proposal_height,
        });
    }
    let mut exact_carrier = None;
    for (key, heights) in group.ordered_keys.iter().zip(carrier_heights) {
        let mut heights = heights.iter().copied();
        let Some(height) = heights.next() else {
            return Err(V2ReservationLifecycleError::MissingCommittedBinding {
                transaction_hash: key.signed_transaction_hash,
            });
        };
        if heights.next().is_some() || exact_carrier.is_some_and(|existing| existing != height) {
            return Err(V2ReservationLifecycleError::CommittedCarrierMismatch {
                lane_id: group.identity.lane_id,
                proposal_height: group.identity.proposal_height,
            });
        }
        exact_carrier = Some(height);
    }
    exact_carrier.ok_or(V2ReservationLifecycleError::CommittedCarrierMismatch {
        lane_id: group.identity.lane_id,
        proposal_height: group.identity.proposal_height,
    })
}

/// Build one immutable Queue/Kura/State reservation reconciliation plan.
///
/// Every Queue group, release barrier, State membership bit, committed merge
/// binding, pending merge binding, canonical body, and Kura attempt is
/// validated without mutation. A finalized hash without its exact body yields
/// a bounded authenticated recovery need and is never interpreted as proof
/// that a payload lost.
pub(crate) fn plan_lane_reservation_ownership(
    state: &State,
    queue: &Queue,
    kura: &Kura,
    verified_active_context: &VerifiedHeightContext,
) -> Result<LaneReservationReconciliationPlanning, V2ReservationLifecycleError> {
    let active_context = verified_active_context.context();
    let live = queue.live_lane_reservations();
    let release_barriers = queue.lane_reservation_release_barriers();
    let commit_barriers = queue.lane_reservation_commit_barriers();
    if live.is_empty() && release_barriers.is_empty() && commit_barriers.is_empty() {
        let snapshot = queue.lane_reservation_reconciliation_snapshot()?;
        return Ok(LaneReservationReconciliationPlanning::Ready(
            LaneReservationReconciliationPlan {
                snapshot,
                release_barriers,
                commit_barriers,
                actions: Vec::new(),
                direct_release: Vec::new(),
                chain_hash: Hash::new(active_context.chain_id.clone().into_inner().as_bytes()),
                recovered: 0,
            },
        ));
    }

    let state_height = u64::try_from(state.committed_height())?;
    if active_context.height < state_height
        || active_context.height > state_height.saturating_add(1)
    {
        return Err(V2ReservationLifecycleError::ActiveHeightMismatch {
            active_height: active_context.height,
            state_height,
        });
    }
    let snapshot = queue.lane_reservation_reconciliation_snapshot()?;
    let chain_hash = Hash::new(active_context.chain_id.clone().into_inner().as_bytes());
    let world = state.world_view();
    let nexus = state.nexus_snapshot();

    let mut inputs = snapshot
        .ordered_groups
        .iter()
        .cloned()
        .map(|group| {
            let owned_keys = group.ordered_keys.clone();
            ReservationReconciliationGroupInput {
                group,
                owned_keys,
                release_barrier: None,
                committed: false,
            }
        })
        .collect::<Vec<_>>();
    let mut group_indexes = inputs
        .iter()
        .enumerate()
        .map(|(index, input)| (input.group.identity, index))
        .collect::<BTreeMap<_, _>>();
    let mut unique_recovered = snapshot
        .ordered_records
        .iter()
        .map(|record| record.key.signed_transaction_hash)
        .collect::<BTreeSet<_>>();

    // Existing barriers are themselves crash state. Fold each into the same
    // classifier input and authenticate its exact Kura retirement before any
    // Queue operation is resumed.
    for barrier in &release_barriers {
        barrier.validate().map_err(|_| {
            V2ReservationLifecycleError::InvalidReleaseBarrierGroup {
                retirement_hash: barrier.retirement_hash,
            }
        })?;
        if barrier.chain_id_hash != chain_hash {
            return Err(V2ReservationLifecycleError::ReleaseRetirementMismatch {
                retirement_hash: barrier.retirement_hash,
            });
        }
        for key in &barrier.ordered_keys {
            unique_recovered.insert(key.signed_transaction_hash);
            if state.has_committed_transaction(key.signed_transaction_hash) {
                return Err(
                    V2ReservationLifecycleError::ReleaseBarrierCommittedTransaction {
                        transaction_hash: key.signed_transaction_hash,
                    },
                );
            }
        }
        let identity = reservation_group_identity(
            barrier
                .ordered_keys
                .first()
                .expect("validated release barrier is non-empty"),
        );
        if barrier
            .ordered_keys
            .iter()
            .any(|key| !reservation_key_matches_group(key, &identity))
        {
            return Err(V2ReservationLifecycleError::InvalidReleaseBarrierGroup {
                retirement_hash: barrier.retirement_hash,
            });
        }
        let group = LaneQueueReservationReconciliationGroupV1 {
            identity,
            ordered_keys: barrier.ordered_keys.clone(),
        };
        match group_indexes.get(&identity).copied() {
            Some(index) => {
                if inputs[index].group != group || inputs[index].release_barrier.is_some() {
                    return Err(V2ReservationLifecycleError::InvalidReleaseBarrierGroup {
                        retirement_hash: barrier.retirement_hash,
                    });
                }
                inputs[index].release_barrier = Some(barrier.clone());
            }
            None => {
                let index = inputs.len();
                group_indexes.insert(identity, index);
                inputs.push(ReservationReconciliationGroupInput {
                    owned_keys: group.ordered_keys.clone(),
                    group,
                    release_barrier: Some(barrier.clone()),
                    committed: false,
                });
            }
        }
    }

    // A crash between per-key Commit and ForgetCommit transitions can leave
    // any exact subset of a committed proposal group split between live
    // records and Commit barriers. Fold both owner forms into one group before
    // consulting State or Kura. Canonical MergeLedger evidence below restores
    // the complete ordered membership; digest order is never treated as group
    // order.
    for key in &commit_barriers {
        unique_recovered.insert(key.signed_transaction_hash);
        let identity = reservation_group_identity(key);
        match group_indexes.get(&identity).copied() {
            Some(index) => {
                let input = &mut inputs[index];
                if input.release_barrier.is_some()
                    || input.owned_keys.iter().any(|owned| {
                        owned.signed_transaction_hash == key.signed_transaction_hash
                            || *owned == *key
                    })
                {
                    return Err(V2ReservationLifecycleError::CommittedBindingMismatch {
                        transaction_hash: key.signed_transaction_hash,
                    });
                }
                input.owned_keys.push(*key);
                input.group.ordered_keys.push(*key);
            }
            None => {
                let index = inputs.len();
                group_indexes.insert(identity, index);
                inputs.push(ReservationReconciliationGroupInput {
                    group: LaneQueueReservationReconciliationGroupV1 {
                        identity,
                        ordered_keys: vec![*key],
                    },
                    owned_keys: vec![*key],
                    release_barrier: None,
                    committed: false,
                });
            }
        }
    }

    for input in &mut inputs {
        if input.group.identity.proposal_height > active_context.height {
            return Err(V2ReservationLifecycleError::FutureReservation {
                proposal_height: input.group.identity.proposal_height,
                active_height: active_context.height,
            });
        }
        if state.lane_incarnation_at_height(
            input.group.identity.lane_id,
            input.group.identity.proposal_height,
        ) != Some(input.group.identity.lane_incarnation)
            || crate::state::nexus_active_lane_dataspace_at_height(
                input.group.identity.lane_id,
                &nexus,
                input.group.identity.proposal_height,
            ) != Some(input.group.identity.dataspace_id)
        {
            return Err(V2ReservationLifecycleError::StaleReservationContext {
                lane_id: input.group.identity.lane_id,
                proposal_height: input.group.identity.proposal_height,
            });
        }
        let committed_count = input
            .owned_keys
            .iter()
            .filter(|key| state.has_committed_transaction(key.signed_transaction_hash))
            .count();
        if committed_count != 0 && committed_count != input.owned_keys.len() {
            return Err(V2ReservationLifecycleError::PartialCommittedGroup {
                lane_id: input.group.identity.lane_id,
                proposal_height: input.group.identity.proposal_height,
            });
        }
        input.committed = committed_count == input.owned_keys.len();
        if input.committed {
            if input.release_barrier.is_some() {
                return Err(V2ReservationLifecycleError::PartialCommittedGroup {
                    lane_id: input.group.identity.lane_id,
                    proposal_height: input.group.identity.proposal_height,
                });
            }
        }
    }
    for barrier in &commit_barriers {
        if !state.has_committed_transaction(barrier.signed_transaction_hash) {
            return Err(V2ReservationLifecycleError::UncommittedCommitBarrier {
                transaction_hash: barrier.signed_transaction_hash,
            });
        }
    }

    // Reconstruct each committed proposal's complete canonical ordered group
    // from its exact indexed MergeLedger carrier. Queue ownership can be any
    // exact non-empty subset after a crash between per-key Commit/ForgetCommit
    // transitions, but the carrier must contain one unique full group; every
    // full-group transaction must already be in State and indexed to that same
    // carrier. No Queue owner is consumed until all groups pass this preflight.
    let mut globally_seen_committed = BTreeMap::new();
    for input in inputs.iter_mut().filter(|input| input.committed) {
        let observed_group = LaneQueueReservationReconciliationGroupV1 {
            identity: input.group.identity,
            ordered_keys: input.owned_keys.clone(),
        };
        let mut observed_hashes = BTreeSet::new();
        for key in &input.owned_keys {
            if !reservation_key_matches_group(key, &input.group.identity)
                || !observed_hashes.insert(key.signed_transaction_hash)
            {
                return Err(V2ReservationLifecycleError::CommittedBindingMismatch {
                    transaction_hash: key.signed_transaction_hash,
                });
            }
        }
        let observed_heights = input
            .owned_keys
            .iter()
            .map(|key| {
                kura.get_block_heights_by_transaction_hash(key.signed_transaction_hash)
                    .ok_or(
                        V2ReservationLifecycleError::CommittedTransactionIndexUnavailable {
                            transaction_hash: key.signed_transaction_hash,
                        },
                    )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let carrier_height =
            exact_committed_carrier_height_for_group(&observed_group, &observed_heights)?;
        let carrier_entry = kura
            .get_merge_entry_by_carrier_height(carrier_height)?
            .ok_or_else(|| V2ReservationLifecycleError::MissingCommittedBinding {
                transaction_hash: input.owned_keys[0].signed_transaction_hash,
            })?;
        let carrier_reservations =
            crate::state::certified_merge_queue_reservations(&carrier_entry)?;
        let matching_positions = carrier_reservations
            .iter()
            .enumerate()
            .filter_map(|(index, (_, key))| {
                reservation_key_matches_group(key, &input.group.identity).then_some(index)
            })
            .collect::<Vec<_>>();
        if matching_positions.is_empty()
            || matching_positions
                .windows(2)
                .any(|positions| positions[1] != positions[0].saturating_add(1))
            || matching_positions.len() > crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS
        {
            return Err(V2ReservationLifecycleError::CommittedCarrierMismatch {
                lane_id: input.group.identity.lane_id,
                proposal_height: input.group.identity.proposal_height,
            });
        }
        let full_keys = matching_positions
            .iter()
            .map(|index| carrier_reservations[*index].1)
            .collect::<Vec<_>>();
        let full_by_transaction = full_keys
            .iter()
            .map(|key| (key.signed_transaction_hash, *key))
            .collect::<BTreeMap<_, _>>();
        if full_by_transaction.len() != full_keys.len() {
            return Err(V2ReservationLifecycleError::CommittedCarrierMismatch {
                lane_id: input.group.identity.lane_id,
                proposal_height: input.group.identity.proposal_height,
            });
        }
        for observed in &input.owned_keys {
            if full_by_transaction.get(&observed.signed_transaction_hash) != Some(observed) {
                return Err(V2ReservationLifecycleError::CommittedBindingMismatch {
                    transaction_hash: observed.signed_transaction_hash,
                });
            }
        }
        for key in &full_keys {
            if !state.has_committed_transaction(key.signed_transaction_hash) {
                return Err(V2ReservationLifecycleError::PartialCommittedGroup {
                    lane_id: input.group.identity.lane_id,
                    proposal_height: input.group.identity.proposal_height,
                });
            }
            if let Some(existing_identity) =
                globally_seen_committed.insert(key.signed_transaction_hash, input.group.identity)
                && existing_identity != input.group.identity
            {
                return Err(V2ReservationLifecycleError::CommittedCarrierMismatch {
                    lane_id: input.group.identity.lane_id,
                    proposal_height: input.group.identity.proposal_height,
                });
            }
        }
        let full_group = LaneQueueReservationReconciliationGroupV1 {
            identity: input.group.identity,
            ordered_keys: full_keys,
        };
        let full_heights = full_group
            .ordered_keys
            .iter()
            .map(|key| {
                kura.get_block_heights_by_transaction_hash(key.signed_transaction_hash)
                    .ok_or(
                        V2ReservationLifecycleError::CommittedTransactionIndexUnavailable {
                            transaction_hash: key.signed_transaction_hash,
                        },
                    )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if exact_committed_carrier_height_for_group(&full_group, &full_heights)? != carrier_height {
            return Err(V2ReservationLifecycleError::CommittedCarrierMismatch {
                lane_id: input.group.identity.lane_id,
                proposal_height: input.group.identity.proposal_height,
            });
        }
        input.group = full_group;
    }

    // Pending merge evidence is bounded by Kura's configured sidecar budget.
    // Index it once so a partial or split group is rejected before mutations.
    let mut pending_by_transaction = BTreeMap::new();
    let mut pending_by_entry = BTreeMap::new();
    for (entry_hash, entry) in kura.pending_certified_merge_entries()? {
        let reservations = crate::state::certified_merge_queue_reservations(&entry)?;
        let keys = reservations.iter().map(|(_, key)| *key).collect::<Vec<_>>();
        for (transaction_hash, key) in reservations {
            if pending_by_transaction
                .insert(transaction_hash, (entry_hash, key))
                .is_some()
            {
                return Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
                    lane_id: key.lane_id,
                    proposal_height: key.proposal_height,
                });
            }
        }
        pending_by_entry.insert(entry_hash, keys);
    }

    let evidence_inputs = inputs
        .iter()
        .filter(|input| !input.committed)
        .map(|input| input.group.clone())
        .collect::<Vec<_>>();
    let evidence_epochs = evidence_inputs
        .iter()
        .map(|group| {
            crate::sumeragi::epoch_for_height_from_world(&world, group.identity.proposal_height)
        })
        .collect::<Vec<_>>();
    let evidence = kura.classify_autonomous_lane_reservation_groups(
        &evidence_inputs,
        chain_hash,
        &evidence_epochs,
    )?;
    debug_assert_eq!(evidence.len(), evidence_inputs.len());
    let mut evidence = evidence.into_iter();

    let mut actions = Vec::with_capacity(inputs.len());
    let mut direct_release_groups = BTreeSet::new();
    let mut missing_bodies = BTreeMap::<u64, CanonicalExecutedBlockNeedV1>::new();
    let mut historical_installs = BTreeMap::<Hash, HistoricalAutonomousReservationInstallV1>::new();
    for input in &inputs {
        if input.committed {
            actions.push(ReservationReconciliationAction::Commit(
                input.owned_keys.clone(),
            ));
            continue;
        }
        let epoch = crate::sumeragi::epoch_for_height_from_world(
            &world,
            input.group.identity.proposal_height,
        );
        let pending_merge = exact_pending_merge_for_group(
            &input.group,
            &pending_by_transaction,
            &pending_by_entry,
        )?;
        let group_evidence = evidence
            .next()
            .expect("Kura preserves one evidence result per uncommitted group");
        let expected_payload = match &group_evidence {
            AutonomousLaneReservationEvidenceV1::StrictlyAbsent => None,
            AutonomousLaneReservationEvidenceV1::ExactLive { payload, .. }
            | AutonomousLaneReservationEvidenceV1::ExactRetired { payload, .. } => Some(payload),
        };
        let canonical = canonical_autonomous_carrier_disposition(
            state,
            kura,
            active_context,
            state_height,
            chain_hash,
            epoch,
            &input.group,
            expected_payload,
        )?;
        let canonical = match canonical {
            CanonicalAutonomousCarrierInspection::Available(disposition) => disposition,
            CanonicalAutonomousCarrierInspection::MissingBody(need) => {
                collect_canonical_executed_block_need(&mut missing_bodies, need)?;
                continue;
            }
        };

        let action = match group_evidence {
            AutonomousLaneReservationEvidenceV1::StrictlyAbsent => {
                if pending_merge || input.release_barrier.is_some() {
                    return Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
                        lane_id: input.group.identity.lane_id,
                        proposal_height: input.group.identity.proposal_height,
                    });
                }
                match canonical {
                    CanonicalAutonomousCarrierDisposition::ExactAutonomous(install) => {
                        if kura.historical_autonomous_lane_recovery_matches(&install)? {
                            ReservationReconciliationAction::Retain {
                                keys: input.group.ordered_keys.clone(),
                                disposition: ReservationRetainDisposition::HistoricalRecovery(
                                    install,
                                ),
                            }
                        } else {
                            match historical_installs.entry(install.recovery_id) {
                                std::collections::btree_map::Entry::Vacant(entry) => {
                                    entry.insert(install);
                                }
                                std::collections::btree_map::Entry::Occupied(entry)
                                    if entry.get() == &install => {}
                                std::collections::btree_map::Entry::Occupied(_) => {
                                    return Err(
                                        V2ReservationLifecycleError::CanonicalAttemptConflict {
                                            height: input.group.identity.proposal_height,
                                            lane_id: input.group.identity.lane_id,
                                        },
                                    );
                                }
                            }
                            continue;
                        }
                    }
                    CanonicalAutonomousCarrierDisposition::ExactOrdinary => {
                        return Err(
                            V2ReservationLifecycleError::CanonicalCarrierMissingKuraPayload {
                                height: input.group.identity.proposal_height,
                                lane_id: input.group.identity.lane_id,
                            },
                        );
                    }
                    CanonicalAutonomousCarrierDisposition::NotFinalized
                    | CanonicalAutonomousCarrierDisposition::Absent => {
                        direct_release_groups.insert(input.group.identity);
                        ReservationReconciliationAction::DirectRelease
                    }
                }
            }
            AutonomousLaneReservationEvidenceV1::ExactLive {
                payload,
                certification,
            } => {
                if input.release_barrier.is_some() {
                    return Err(V2ReservationLifecycleError::InvalidReleaseBarrierGroup {
                        retirement_hash: input
                            .release_barrier
                            .as_ref()
                            .expect("checked present")
                            .retirement_hash,
                    });
                }
                if pending_merge {
                    if !certification.is_certified() {
                        return Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
                            lane_id: input.group.identity.lane_id,
                            proposal_height: input.group.identity.proposal_height,
                        });
                    }
                    if !canonical.is_exact() {
                        return Err(if canonical.is_absent() {
                            V2ReservationLifecycleError::CertifiedTerminalLoser {
                                lane_id: input.group.identity.lane_id,
                                height: input.group.identity.proposal_height,
                            }
                        } else {
                            V2ReservationLifecycleError::CertifiedPayloadMissingCanonicalCarrier {
                                lane_id: input.group.identity.lane_id,
                                height: input.group.identity.proposal_height,
                            }
                        });
                    }
                    ReservationReconciliationAction::Retain {
                        keys: input.group.ordered_keys.clone(),
                        disposition: ReservationRetainDisposition::PendingMerge,
                    }
                } else if certification.is_certified() {
                    if !canonical.is_exact() {
                        return Err(if canonical.is_absent() {
                            V2ReservationLifecycleError::CertifiedTerminalLoser {
                                lane_id: input.group.identity.lane_id,
                                height: input.group.identity.proposal_height,
                            }
                        } else {
                            V2ReservationLifecycleError::CertifiedPayloadMissingCanonicalCarrier {
                                lane_id: input.group.identity.lane_id,
                                height: input.group.identity.proposal_height,
                            }
                        });
                    }
                    ReservationReconciliationAction::Retain {
                        keys: input.group.ordered_keys.clone(),
                        disposition: ReservationRetainDisposition::Certified,
                    }
                } else {
                    match canonical {
                        CanonicalAutonomousCarrierDisposition::NotFinalized => {
                            ReservationReconciliationAction::Retain {
                                keys: input.group.ordered_keys.clone(),
                                disposition: ReservationRetainDisposition::Current,
                            }
                        }
                        CanonicalAutonomousCarrierDisposition::ExactAutonomous(install) => {
                            if kura.historical_autonomous_lane_recovery_matches(&install)? {
                                ReservationReconciliationAction::Retain {
                                    keys: input.group.ordered_keys.clone(),
                                    disposition: ReservationRetainDisposition::HistoricalRecovery(
                                        install,
                                    ),
                                }
                            } else {
                                match historical_installs.entry(install.recovery_id) {
                                    std::collections::btree_map::Entry::Vacant(entry) => {
                                        entry.insert(install);
                                    }
                                    std::collections::btree_map::Entry::Occupied(entry)
                                        if entry.get() == &install => {}
                                    std::collections::btree_map::Entry::Occupied(_) => {
                                        return Err(
                                            V2ReservationLifecycleError::CanonicalAttemptConflict {
                                                height: input.group.identity.proposal_height,
                                                lane_id: input.group.identity.lane_id,
                                            },
                                        );
                                    }
                                }
                                continue;
                            }
                        }
                        CanonicalAutonomousCarrierDisposition::ExactOrdinary => {
                            return Err(
                                V2ReservationLifecycleError::CanonicalCarrierMissingKuraPayload {
                                    height: input.group.identity.proposal_height,
                                    lane_id: input.group.identity.lane_id,
                                },
                            );
                        }
                        CanonicalAutonomousCarrierDisposition::Absent => {
                            ReservationReconciliationAction::Retire {
                                retirement: AutonomousLaneSlotRetirementV1::from_payload(&payload),
                                epoch,
                                resumed: false,
                            }
                        }
                    }
                }
            }
            AutonomousLaneReservationEvidenceV1::ExactRetired {
                payload: _,
                retirement,
                certification,
            } => {
                if certification.is_certified() {
                    return Err(V2ReservationLifecycleError::CertifiedTerminalLoser {
                        lane_id: input.group.identity.lane_id,
                        height: input.group.identity.proposal_height,
                    });
                }
                if pending_merge {
                    return Err(V2ReservationLifecycleError::PendingMergeBindingMismatch {
                        lane_id: input.group.identity.lane_id,
                        proposal_height: input.group.identity.proposal_height,
                    });
                }
                if canonical.is_exact() {
                    return Err(V2ReservationLifecycleError::RetiredCanonicalCarrier {
                        lane_id: input.group.identity.lane_id,
                        height: input.group.identity.proposal_height,
                    });
                }
                if matches!(
                    canonical,
                    CanonicalAutonomousCarrierDisposition::NotFinalized
                ) {
                    return Err(V2ReservationLifecycleError::UnfinalizedRetirement {
                        lane_id: input.group.identity.lane_id,
                        height: input.group.identity.proposal_height,
                    });
                }
                let exact_barrier = retirement.queue_release_barrier()?;
                if input
                    .release_barrier
                    .as_ref()
                    .is_some_and(|barrier| *barrier != exact_barrier)
                {
                    return Err(V2ReservationLifecycleError::ReleaseRetirementMismatch {
                        retirement_hash: exact_barrier.retirement_hash,
                    });
                }
                ReservationReconciliationAction::Retire {
                    retirement,
                    epoch,
                    resumed: true,
                }
            }
        };
        actions.push(action);
    }
    debug_assert!(evidence.next().is_none());

    // Materialize the direct-release vector from Queue's immutable global FIFO
    // snapshot, never from digest or proposal-group order.
    let direct_release = snapshot
        .ordered_records
        .iter()
        .filter(|record| direct_release_groups.contains(&record.group))
        .map(|record| record.key)
        .collect::<Vec<_>>();

    // Queue does not expose barriers and live records from one combined
    // observer. Startup has not constructed lane work yet, and this exact
    // stability recheck additionally rejects any unexpected concurrent owner
    // transition before the first mutation.
    if queue.lane_reservation_reconciliation_snapshot()? != snapshot
        || queue.lane_reservation_release_barriers() != release_barriers
        || queue.lane_reservation_commit_barriers() != commit_barriers
    {
        return Err(V2ReservationLifecycleError::QueueSnapshotChanged);
    }

    if !missing_bodies.is_empty() {
        return Ok(
            LaneReservationReconciliationPlanning::RecoverCanonicalBodies(
                missing_bodies.into_values().collect(),
            ),
        );
    }

    if !historical_installs.is_empty() {
        return Ok(
            LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(
                historical_installs.into_values().collect(),
            ),
        );
    }

    Ok(LaneReservationReconciliationPlanning::Ready(
        LaneReservationReconciliationPlan {
            snapshot,
            release_barriers,
            commit_barriers,
            actions,
            direct_release,
            chain_hash,
            recovered: unique_recovered.len(),
        },
    ))
}

/// Apply one previously completed immutable reconciliation plan.
///
/// The Queue snapshot and every crash-barrier family are rechecked before the
/// first mutation. Callers must hold the process fail-stop operation across
/// this function.
pub(crate) fn apply_lane_reservation_reconciliation_plan(
    queue: &Queue,
    kura: &Kura,
    plan: LaneReservationReconciliationPlan,
) -> Result<LaneReservationReconciliationSummary, V2ReservationLifecycleError> {
    let LaneReservationReconciliationPlan {
        snapshot,
        release_barriers,
        commit_barriers,
        actions,
        direct_release,
        chain_hash,
        recovered,
    } = plan;
    // Recovery durability is checked before acquiring any Queue ownership
    // locks. The immutable Kura record is the authority which permits the
    // startup gate to publish this historical owner.
    for action in &actions {
        let ReservationReconciliationAction::Retain {
            disposition: ReservationRetainDisposition::HistoricalRecovery(install),
            ..
        } = action
        else {
            continue;
        };
        if !kura.historical_autonomous_lane_recovery_matches(install)? {
            return Err(
                V2ReservationLifecycleError::HistoricalRecoveryInstallationMissing {
                    recovery_id: install.recovery_id,
                    lane_id: install.reservation_group.identity.lane_id,
                },
            );
        }
    }
    if queue.lane_reservation_reconciliation_snapshot()? != snapshot
        || queue.lane_reservation_release_barriers() != release_barriers
        || queue.lane_reservation_commit_barriers() != commit_barriers
    {
        return Err(V2ReservationLifecycleError::QueueSnapshotChanged);
    }

    // Mutations are independently idempotent and ordered so committed work is
    // consumed before any release.
    let mut summary = LaneReservationReconciliationSummary {
        recovered,
        ..LaneReservationReconciliationSummary::default()
    };
    for action in &actions {
        if let ReservationReconciliationAction::Commit(keys) = action {
            for key in keys {
                if queue.commit_lane_reservation(key)? == LaneQueueReservationOutcome::Finalized {
                    summary.finalized_committed = summary.finalized_committed.saturating_add(1);
                }
            }
        }
    }
    for action in actions {
        match action {
            ReservationReconciliationAction::Commit(_)
            | ReservationReconciliationAction::DirectRelease => {}
            ReservationReconciliationAction::Retain { keys, disposition } => {
                for key in &keys {
                    let _ = queue.retain_lane_reservation(key)?;
                }
                let count = keys.len();
                match disposition {
                    ReservationRetainDisposition::Current => {
                        summary.retained_current = summary.retained_current.saturating_add(count);
                    }
                    ReservationRetainDisposition::Certified => {
                        summary.retained_certified =
                            summary.retained_certified.saturating_add(count);
                    }
                    ReservationRetainDisposition::PendingMerge => {
                        summary.retained_pending_merge =
                            summary.retained_pending_merge.saturating_add(count);
                    }
                    ReservationRetainDisposition::HistoricalRecovery(_) => {
                        summary.retained_historical_recovery =
                            summary.retained_historical_recovery.saturating_add(count);
                    }
                }
            }
            ReservationReconciliationAction::Retire {
                retirement,
                epoch,
                resumed,
            } => {
                let released = retire_autonomous_lane_slot_and_release_reservations(
                    kura,
                    queue,
                    &retirement,
                    chain_hash,
                    epoch,
                )?;
                if resumed {
                    summary.resumed_retirement =
                        summary.resumed_retirement.saturating_add(released);
                } else {
                    summary.released_terminal_loser =
                        summary.released_terminal_loser.saturating_add(released);
                }
            }
        }
    }
    summary.released_strictly_absent = queue.release_lane_reservations_in_order(&direct_release)?;
    if !queue.lane_reservation_commit_barriers().is_empty() {
        return Err(V2ReservationLifecycleError::IncompleteQueueBarrier { kind: "commit" });
    }
    if !queue.lane_reservation_release_barriers().is_empty() {
        return Err(V2ReservationLifecycleError::IncompleteQueueBarrier { kind: "release" });
    }
    queue.complete_lane_reservation_startup_reconciliation()?;
    Ok(summary)
}

fn application_typed_identity<T>(
    domain: u8,
    kind: u8,
    hash: HashOf<T>,
) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}

fn application_hash_identity(domain: u8, kind: u8, hash: Hash) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}

fn application_decision_projection(
    decision: wire::QuorumCertificateRef,
) -> ProductionDecisionIdentityProjection {
    ProductionDecisionIdentityProjection {
        context_id: application_typed_identity(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
            decision.round.context_id.0,
        ),
        height: decision.round.height,
        view: decision.round.view,
        proposal_height: decision.proposal_round.height,
        proposal_view: decision.proposal_round.view,
        phase: decision.phase as u8,
        subject: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
            HashOf::new(&decision.subject),
        ),
        block_hash: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            decision.subject.block_hash,
        ),
        payload_hash: application_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_CANONICAL_PAYLOAD,
            decision.subject.payload_hash,
        ),
        execution_commitment: application_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTION_COMMITMENT,
            HashOf::new(&decision.execution_commitment),
        ),
        executed_block_wire_hash: application_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
            decision.execution_commitment.executed_block_wire_hash,
        ),
    }
}

fn application_certificate_projection(
    certificate: &wire::QuorumCertificate,
) -> Option<ProductionQuorumCertificateIdentityProjection> {
    Some(ProductionQuorumCertificateIdentityProjection {
        decision: application_decision_projection(certificate.as_ref()),
        certificate: application_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_QUORUM_CERTIFICATE,
            HashOf::new(certificate),
        ),
        signer_count: u64::try_from(certificate.signers.len()).ok()?,
        aggregate_signature_len: u64::try_from(certificate.aggregate_signature.len()).ok()?,
    })
}

fn application_body_projection(
    receipt: &ValidatedBodyReceipt,
) -> ProductionDurableBodyIdentityProjection {
    let durable = receipt.durable();
    let subject = durable.subject();
    ProductionDurableBodyIdentityProjection {
        context_id: application_typed_identity(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
            durable.context_id().0,
        ),
        height: durable.round().height,
        view: durable.round().view,
        subject: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
            HashOf::new(&subject),
        ),
        block_hash: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            subject.block_hash,
        ),
        payload_hash: application_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_CANONICAL_PAYLOAD,
            subject.payload_hash,
        ),
        manifest: application_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_PAYLOAD_MANIFEST,
            durable.manifest_hash(),
        ),
        frame: application_hash_identity(
            IDENTITY_DOMAIN_DURABLE_ARTIFACT,
            IDENTITY_KIND_DURABLE_BODY_FRAME,
            durable.frame_hash(),
        ),
    }
}

fn prospective_application_refinement_projection(
    context: &wire::HeightContext,
    task: &ApplyTask,
    proposal_block_hash: HashOf<BlockHeader>,
    canonical_proposal_wire_hash: Hash,
    artifact: &wire::finality::V2FinalityArtifact,
) -> Option<ProductionApplicationTraceProjection> {
    let context_id = application_typed_identity(
        IDENTITY_DOMAIN_CONTEXT,
        IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        context.id().0,
    );
    let artifact_hash = HashOf::new(artifact);
    Some(ProductionApplicationTraceProjection {
        task_tag: TagProjection {
            height: task.tag().height(),
            view: task.tag().view(),
            generation: task.tag().generation().get(),
        },
        owner_tag: TagProjection {
            height: task.authorized_owner_tag().height(),
            view: task.authorized_owner_tag().view(),
            generation: task.authorized_owner_tag().generation().get(),
        },
        task_generation: task.tag().generation().get(),
        context_id,
        context_height: context.height,
        commit_qc: application_certificate_projection(task.certificate())?,
        validated_body: application_body_projection(task.validated_receipt()),
        validated_execution_commitment: application_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTION_COMMITMENT,
            HashOf::new(&task.validated_receipt().execution_commitment()),
        ),
        proposal_block_hash: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            proposal_block_hash,
        ),
        proposal_payload_hash: application_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_CANONICAL_PAYLOAD,
            canonical_proposal_wire_hash,
        ),
        committed_block_hash: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            task.subject().block_hash,
        ),
        executed_block_wire_hash: application_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
            task.certificate()
                .execution_commitment
                .executed_block_wire_hash,
        ),
        kura_decision: application_decision_projection(task.certificate().as_ref()),
        kura_artifact_hash: application_typed_identity(
            IDENTITY_DOMAIN_DURABLE_ARTIFACT,
            IDENTITY_KIND_FINALITY_ARTIFACT,
            artifact_hash,
        ),
        artifact_context_id: application_typed_identity(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
            artifact.height_context.id().0,
        ),
        artifact_height: artifact.height,
        artifact_subject: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
            HashOf::new(&artifact.subject),
        ),
        artifact_block_hash: application_typed_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            artifact.block_hash,
        ),
        artifact_commit_qc: application_certificate_projection(&artifact.commit_qc)?,
        artifact_hash: application_typed_identity(
            IDENTITY_DOMAIN_DURABLE_ARTIFACT,
            IDENTITY_KIND_FINALITY_ARTIFACT,
            artifact_hash,
        ),
        state_height_after: context.height,
        task_work_id: task.id().get(),
        completion_work_id: task.id().get(),
    })
}

/// Complete native identity crossing the durable application boundary.
///
/// The type is process-local and intentionally has no codec implementation.
/// It retains full typed consensus and durability evidence. Canonical proposal,
/// executed-block, body-frame, and artifact links use the existing native
/// 256-bit hash values without projection or truncation; those comparisons rely
/// on the repository's reviewed collision-resistance contract.
#[derive(Clone, Debug)]
#[must_use]
pub(crate) struct DurableApplicationEvidence {
    task_tag: EventTag,
    owner_tag: EventTag,
    task_generation: u64,
    task_work_id: EffectWorkId,
    context: wire::HeightContext,
    commit_qc: wire::QuorumCertificate,
    subject: wire::BlockSubject,
    execution_commitment: wire::ExecutionCommitment,
    validated_receipt: ValidatedBodyReceipt,
    validated_manifest_hash: HashOf<wire::PayloadManifest>,
    validated_body_frame_hash: Hash,
    proposal_block_hash: HashOf<BlockHeader>,
    canonical_proposal_wire_hash: Hash,
    committed_block_hash: HashOf<BlockHeader>,
    executed_block_wire_hash: Hash,
    kura_receipt: KuraV2CommitReceipt,
    artifact: wire::finality::V2FinalityArtifact,
    artifact_hash: HashOf<wire::finality::V2FinalityArtifact>,
    completion_work_id: EffectWorkId,
    state_height_after: usize,
}

impl DurableApplicationEvidence {
    /// Reducer incarnation which created the Apply task.
    pub(crate) const fn task_tag(&self) -> EventTag {
        self.task_tag
    }

    /// Reducer incarnation captured by the executor when it authorized Apply.
    pub(crate) const fn owner_tag(&self) -> EventTag {
        self.owner_tag
    }

    /// Actor-local task generation, distinct from consensus view.
    pub(crate) const fn task_generation(&self) -> u64 {
        self.task_generation
    }

    /// Stable asynchronous work owner assigned to the Apply task.
    pub(crate) const fn task_work_id(&self) -> EffectWorkId {
        self.task_work_id
    }

    /// Complete immutable height context governing application.
    pub(crate) const fn context(&self) -> &wire::HeightContext {
        &self.context
    }

    /// Complete CommitQC, including canonical signers and aggregate signature.
    pub(crate) const fn commit_qc(&self) -> &wire::QuorumCertificate {
        &self.commit_qc
    }

    /// Exact round carried by the CommitQC.
    pub(crate) const fn commit_round(&self) -> wire::ConsensusRound {
        self.commit_qc.round
    }

    /// Exact phase carried by the CommitQC.
    pub(crate) const fn commit_phase(&self) -> wire::GlobalPhase {
        self.commit_qc.phase
    }

    /// Canonically ordered CommitQC signer indices.
    pub(crate) fn commit_signers(&self) -> &[wire::ValidatorIndex] {
        &self.commit_qc.signers
    }

    /// Complete CommitQC aggregate-signature evidence.
    pub(crate) fn commit_aggregate_signature(&self) -> &[u8] {
        &self.commit_qc.aggregate_signature
    }

    /// Exact decided subject repeated independently by the Apply task.
    pub(crate) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }

    /// Exact deterministic execution commitment authenticated by the CommitQC.
    pub(crate) const fn execution_commitment(&self) -> wire::ExecutionCommitment {
        self.execution_commitment
    }

    /// Durable validation receipt for the proposal bytes being applied.
    pub(crate) const fn validated_receipt(&self) -> &ValidatedBodyReceipt {
        &self.validated_receipt
    }

    /// Frozen context carried by the validated durable body receipt.
    pub(crate) const fn validated_context_id(&self) -> wire::HeightContextId {
        self.validated_receipt.durable().context_id()
    }

    /// Proposal round carried by the validated durable body receipt.
    pub(crate) const fn validated_round(&self) -> wire::ConsensusRound {
        self.validated_receipt.durable().round()
    }

    /// Proposal subject carried by the validated durable body receipt.
    pub(crate) const fn validated_subject(&self) -> wire::BlockSubject {
        self.validated_receipt.durable().subject()
    }

    /// Manifest identity carried by the validated durable body receipt.
    pub(crate) const fn validated_manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.validated_manifest_hash
    }

    /// Hash of the complete checksummed body frame that passed validation.
    pub(crate) const fn validated_body_frame_hash(&self) -> Hash {
        self.validated_body_frame_hash
    }

    /// Header identity of the resultless proposal loaded from the body store.
    pub(crate) const fn proposal_block_hash(&self) -> HashOf<BlockHeader> {
        self.proposal_block_hash
    }

    /// Hash of the exact canonical resultless proposal wire.
    pub(crate) const fn canonical_proposal_wire_hash(&self) -> Hash {
        self.canonical_proposal_wire_hash
    }

    /// Header identity of the canonical result-bearing committed block.
    pub(crate) const fn committed_block_hash(&self) -> HashOf<BlockHeader> {
        self.committed_block_hash
    }

    /// Hash of the exact canonical result-bearing committed block wire.
    pub(crate) const fn executed_block_wire_hash(&self) -> Hash {
        self.executed_block_wire_hash
    }

    /// Complete non-forgeable Kura finality receipt.
    pub(crate) const fn kura_receipt(&self) -> &KuraV2CommitReceipt {
        &self.kura_receipt
    }

    /// Height durably acknowledged by Kura.
    pub(crate) fn kura_height(&self) -> u64 {
        self.kura_receipt.height()
    }

    /// Canonical block header hash durably acknowledged by Kura.
    pub(crate) fn kura_block_hash(&self) -> HashOf<BlockHeader> {
        self.kura_receipt.block_hash()
    }

    /// Frozen height-context identifier durably acknowledged by Kura.
    pub(crate) fn kura_context_id(&self) -> wire::HeightContextId {
        self.kura_receipt.context_id()
    }

    /// Exact subject durably acknowledged by Kura.
    pub(crate) fn kura_subject(&self) -> wire::BlockSubject {
        self.kura_receipt.subject()
    }

    /// Exact CommitQC reference durably acknowledged by Kura.
    pub(crate) fn kura_certificate(&self) -> wire::QuorumCertificateRef {
        self.kura_receipt.certificate()
    }

    /// Exact finality-artifact identity durably acknowledged by Kura.
    pub(crate) fn kura_artifact_hash(&self) -> HashOf<wire::finality::V2FinalityArtifact> {
        self.kura_receipt.artifact_hash()
    }

    /// Complete finality artifact stored beside the committed block.
    pub(crate) const fn artifact(&self) -> &wire::finality::V2FinalityArtifact {
        &self.artifact
    }

    /// Native typed hash of the complete finality artifact.
    pub(crate) const fn artifact_hash(&self) -> HashOf<wire::finality::V2FinalityArtifact> {
        self.artifact_hash
    }

    /// Work identifier carried by the typed completion.
    pub(crate) const fn completion_work_id(&self) -> EffectWorkId {
        self.completion_work_id
    }

    /// Exact committed State height observed after all durable publications.
    pub(crate) const fn state_height_after(&self) -> usize {
        self.state_height_after
    }

    /// Project each independently retained application identity into the pure
    /// production/Verus kernel. Cardinalities fail closed if they cannot be
    /// represented by the shared fixed-width surface.
    pub(crate) fn application_refinement_projection(
        &self,
    ) -> Option<ProductionApplicationTraceProjection> {
        let context_id = application_typed_identity(
            IDENTITY_DOMAIN_CONTEXT,
            IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
            self.context().id().0,
        );
        let artifact = self.artifact();
        Some(ProductionApplicationTraceProjection {
            task_tag: TagProjection {
                height: self.task_tag().height(),
                view: self.task_tag().view(),
                generation: self.task_tag().generation().get(),
            },
            owner_tag: TagProjection {
                height: self.owner_tag().height(),
                view: self.owner_tag().view(),
                generation: self.owner_tag().generation().get(),
            },
            task_generation: self.task_generation(),
            context_id,
            context_height: self.context().height,
            commit_qc: application_certificate_projection(self.commit_qc())?,
            validated_body: application_body_projection(self.validated_receipt()),
            validated_execution_commitment: application_typed_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_EXECUTION_COMMITMENT,
                HashOf::new(&self.validated_receipt().execution_commitment()),
            ),
            proposal_block_hash: application_typed_identity(
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_BLOCK_HEADER,
                self.proposal_block_hash(),
            ),
            proposal_payload_hash: application_hash_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_CANONICAL_PAYLOAD,
                self.canonical_proposal_wire_hash(),
            ),
            committed_block_hash: application_typed_identity(
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_BLOCK_HEADER,
                self.committed_block_hash(),
            ),
            executed_block_wire_hash: application_hash_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
                self.executed_block_wire_hash(),
            ),
            kura_decision: application_decision_projection(self.kura_certificate()),
            kura_artifact_hash: application_typed_identity(
                IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                IDENTITY_KIND_FINALITY_ARTIFACT,
                self.kura_artifact_hash(),
            ),
            artifact_context_id: application_typed_identity(
                IDENTITY_DOMAIN_CONTEXT,
                IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
                artifact.height_context.id().0,
            ),
            artifact_height: artifact.height,
            artifact_subject: application_typed_identity(
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
                HashOf::new(&artifact.subject),
            ),
            artifact_block_hash: application_typed_identity(
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_BLOCK_HEADER,
                artifact.block_hash,
            ),
            artifact_commit_qc: application_certificate_projection(&artifact.commit_qc)?,
            artifact_hash: application_typed_identity(
                IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                IDENTITY_KIND_FINALITY_ARTIFACT,
                self.artifact_hash(),
            ),
            state_height_after: u64::try_from(self.state_height_after()).ok()?,
            task_work_id: self.task_work_id().get(),
            completion_work_id: self.completion_work_id().get(),
        })
    }

    /// Check every redundant task, wire, durability, and completion identity.
    pub(crate) fn is_exact(&self) -> bool {
        let context = self.context();
        let certificate = self.commit_qc();
        let artifact = self.artifact();
        let Ok(context_height) = usize::try_from(context.height) else {
            return false;
        };
        context.validate().is_ok()
            && certificate.validate(context).is_ok()
            && self.task_tag().height() == context.height
            // Lifecycle ownership is independent of the certificate's
            // intrinsic consensus round. The executor mints this owner only
            // after matching the effect tag to the current reducer tag.
            && self.task_tag() == self.owner_tag()
            && self.task_tag().generation().get() == self.task_generation()
            && self.commit_phase() == wire::GlobalPhase::Commit
            && self.commit_round().context_id == context.id()
            && self.commit_round().height == context.height
            && certificate.subject == self.subject()
            && certificate.execution_commitment == self.execution_commitment()
            && self.commit_signers() == artifact.commit_qc.signers.as_slice()
            && self.commit_aggregate_signature()
                == artifact.commit_qc.aggregate_signature.as_slice()
            && self.validated_context_id() == context.id()
            && self.validated_round().height == context.height
            // The durable body must be the exact same-round proposal body
            // authenticated by the CommitQC.
            && self.validated_round() == certificate.proposal_round
            && self.validated_subject() == self.subject()
            && self.validated_manifest_hash() == self.validated_receipt().durable().manifest_hash()
            && self.validated_body_frame_hash() == self.validated_receipt().durable().frame_hash()
            && self.validated_receipt().execution_commitment() == self.execution_commitment()
            && self.proposal_block_hash() == self.subject().block_hash
            && self.canonical_proposal_wire_hash() == self.subject().payload_hash
            && self.committed_block_hash() == self.subject().block_hash
            && self.executed_block_wire_hash()
                == self.execution_commitment().executed_block_wire_hash
            && self.kura_receipt().height() == self.kura_height()
            && self.kura_height() == context.height
            && self.kura_context_id() == context.id()
            && self.kura_block_hash() == self.committed_block_hash()
            && self.kura_subject() == self.subject()
            && self.kura_certificate() == certificate.as_ref()
            && self.kura_artifact_hash() == self.artifact_hash()
            && &artifact.height_context == context
            && artifact.height == context.height
            && artifact.subject == self.subject()
            && artifact.block_hash == self.committed_block_hash()
            && &artifact.commit_qc == certificate
            && HashOf::new(artifact) == self.artifact_hash()
            && self.completion_work_id() == self.task_work_id()
            && self.state_height_after() == context_height
    }
}

/// Immutable dependencies of the single v2 application service.
pub(crate) struct V2ApplyService {
    state: Arc<State>,
    queue: Arc<Queue>,
    kura: Arc<Kura>,
    provider_ingest_finalized_archive:
        Option<Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>>,
    reputation_finalized_archive:
        Option<Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>>,
    chain_id: ChainId,
    block_cadence: Duration,
    genesis_account: AccountId,
    events_sender: EventsSender,
    validator_set_pops: Vec<Vec<u8>>,
    #[cfg(test)]
    fail_after_kura_store: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_after_wsv_checkpoint: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_after_provider_ingest_archive_capture: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    fail_after_reputation_archive_capture: std::sync::atomic::AtomicBool,
}

impl V2ApplyService {
    fn classify_candidate_validation_error(
        merge_reference: Option<&CertifiedMergeLedgerReference>,
        failed_block: &SignedBlock,
        error: &BlockValidationError,
    ) -> V2ApplyError {
        if let BlockValidationError::MissingCertifiedMergeSidecar { entry_hash } = error {
            return match merge_reference {
                Some(reference) if reference.entry_hash == *entry_hash => {
                    V2ApplyError::MissingCertifiedMergeSidecar {
                        reference: reference.clone(),
                    }
                }
                _ => V2ApplyError::Validation(
                    "validator reported a missing certified merge sidecar that is not bound to the candidate execution context"
                        .to_owned(),
                ),
            };
        }
        let rejected_result_count = failed_block
            .has_results()
            .then(|| {
                failed_block
                    .results()
                    .filter(|result| result.is_err())
                    .count()
            })
            .unwrap_or(0);
        if rejected_result_count == 0 {
            V2ApplyError::Validation(error.to_string())
        } else {
            V2ApplyError::Validation(format!(
                "{error}; rejected transaction result count: {rejected_result_count}"
            ))
        }
    }

    fn validate_lane_payload_plan(
        &self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<(), V2ApplyError> {
        // Genesis instructions bootstrap the lane catalog itself and therefore cannot be routed
        // through a pre-existing committed lane plan. The canonical genesis validator below still
        // enforces its authority, chain, transaction, Merkle, and result invariants.
        if context.height == 1 && body.execution_context().is_none() {
            return Ok(());
        }
        let external_count = body.external_entrypoint_count();
        let Some(bundle) = body.execution_context() else {
            return if external_count == 0 {
                Ok(())
            } else {
                Err(V2ApplyError::Validation(
                    "Sumeragi v2 candidate has external entrypoints without execution context"
                        .to_owned(),
                ))
            };
        };
        if super::v2_lane_work::canonical_v2_lane_payload_matches_kura(
            self.state.as_ref(),
            self.kura.as_ref(),
            context,
            body,
        ) {
            return Ok(());
        }
        let routes = bundle
            .external
            .iter()
            .map(|entry| RoutingDecision::new(entry.lane_id, entry.dataspace_id))
            .collect::<Vec<_>>();
        let hashes = bundle
            .external
            .iter()
            .map(|entry| Hash::from(entry.entrypoint_hash))
            .collect::<Vec<_>>();
        let view = body.header().view_change_index();
        let leader = context
            .roster
            .get(usize::try_from(context.leader(view)).map_err(|_| {
                V2ApplyError::Validation("Sumeragi v2 leader index overflows usize".to_owned())
            })?)
            .ok_or_else(|| {
                V2ApplyError::Validation("Sumeragi v2 leader index is out of range".to_owned())
            })?;
        let expected = super::lane_planner::prepare_v2_lane_payload_plan(
            self.state.as_ref(),
            self.kura.as_ref(),
            context,
            view,
            &leader.validator,
            &routes,
            &hashes,
        )
        .map_err(|error| V2ApplyError::Validation(error.to_string()))?;
        if !expected.unavailable_indices.is_empty()
            || expected.ownerships != bundle.lane_payload_ownerships
        {
            return Err(V2ApplyError::Validation(
                "Sumeragi v2 lane ownerships differ from deterministic committed-state planning"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    /// Construct the serialized state/Kura application adapter.
    pub(crate) fn new(
        state: Arc<State>,
        queue: Arc<Queue>,
        kura: Arc<Kura>,
        provider_ingest_finalized_archive: Option<
            Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>,
        >,
        reputation_finalized_archive: Option<
            Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>,
        >,
        chain_id: ChainId,
        block_cadence: Duration,
        genesis_account: AccountId,
        events_sender: EventsSender,
        validator_set_pops: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            state,
            queue,
            kura,
            provider_ingest_finalized_archive,
            reputation_finalized_archive,
            chain_id,
            block_cadence,
            genesis_account,
            events_sender,
            validator_set_pops,
            #[cfg(test)]
            fail_after_kura_store: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_after_wsv_checkpoint: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_after_provider_ingest_archive_capture: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_after_reputation_archive_capture: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Apply one exact CommitQC task or complete its interrupted sidecar write.
    pub(crate) fn execute(
        &self,
        context: &wire::HeightContext,
        body_store: &mut V2BodyStore,
        task: &ApplyTask,
    ) -> Result<DurableApplyCompletion, V2ApplyError> {
        context.validate()?;
        if task.subject() != task.certificate().subject
            || task.certificate().phase != wire::GlobalPhase::Commit
            || task.certificate().round.context_id != context.id()
            || task.certificate().round.height != context.height
        {
            return Err(V2ApplyError::TaskMismatch);
        }
        task.certificate().execution_commitment.validate()?;
        if task.certificate().execution_commitment
            != task.validated_receipt().execution_commitment()
        {
            return Err(V2ApplyError::ExecutionCommitmentMismatch);
        }
        let body = body_store.load(task.validated_receipt().durable())?;
        let proposal_block_hash = body.hash();
        let canonical_proposal_wire_hash = body
            .canonical_proposal_wire_hash()
            .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?;
        if !body.is_resultless_proposal()
            || proposal_block_hash != task.subject().block_hash
            || body.header().height().get() != context.height
            || body.header().prev_block_hash() != task.subject().parent_block_hash
            || canonical_proposal_wire_hash != task.subject().payload_hash
        {
            return Err(V2ApplyError::TaskMismatch);
        }
        // Authenticate the exact durable decision and its association with the selected body
        // before pruning carrier sidecars or crossing either Kura/WSV commit boundary.
        // `ApplyTask` deliberately retains the wire certificate, so this adapter must not rely
        // only on the upstream reducer having verified it. A malformed decision remains a pure
        // rejection, never a crash image whose canonical block/state lacks valid finality.
        let artifact = wire::finality::V2FinalityArtifact::new(
            context.clone(),
            task.subject(),
            task.certificate().clone(),
            self.validator_set_pops.clone(),
        );
        artifact.validate_for_header(&body.header())?;
        artifact
            .verify()
            .map_err(V2ApplyError::FinalityCryptography)?;
        let prospective_application = prospective_application_refinement_projection(
            context,
            task,
            proposal_block_hash,
            canonical_proposal_wire_hash,
            &artifact,
        )
        .ok_or_else(|| {
            V2ApplyError::Validation(
                "prospective application identity cannot be represented losslessly".to_owned(),
            )
        })?;
        let checked_application = check_production_application_transition(prospective_application)
            .ok_or_else(|| {
                V2ApplyError::Validation(
                    "prospective durable application does not refine its Decision completion"
                        .to_owned(),
                )
            })?;
        let prospective_application = checked_application.into_projection();

        let height = usize::try_from(context.height).map_err(|_| V2ApplyError::HeightOverflow)?;
        let height = NonZeroUsize::new(height).ok_or(V2ApplyError::HeightOverflow)?;
        let state_height = self.state.committed_height();
        if state_height > height.get() {
            return Err(V2ApplyError::StateAhead {
                state_height,
                decision_height: height.get(),
            });
        }
        let durable_hash = self.kura.get_durable_block_hash(height);
        if durable_hash.is_some_and(|hash| hash != task.subject().block_hash) {
            return Err(V2ApplyError::KuraConflict);
        }

        if state_height < height.get() {
            if state_height.saturating_add(1) != height.get() {
                return Err(V2ApplyError::StateGap {
                    state_height,
                    decision_height: height.get(),
                });
            }
        } else if durable_hash.is_none() {
            // WSV cannot be ahead of its canonical block log. Continuing here
            // would manufacture a sidecar for state that Kura cannot identify.
            return Err(V2ApplyError::StateAheadOfKura);
        }

        // The durable CommitQC and exact validated body now identify the only
        // carrier that can ever apply at this height. Keep its immutable
        // compact reference (including an earlier lock origin view) and
        // release every losing pending sidecar before validation can defer on
        // a missing exact entry. A failure after this point remains safe: the
        // decided reference survives, while no losing carrier can become
        // canonical.
        self.retain_decided_merge_sidecar(context, &body)?;

        let committed_block = if state_height < height.get() {
            self.validate_and_apply(
                context,
                body,
                true,
                task.validated_receipt().execution_commitment(),
                &artifact,
            )?;
            self.kura
                .get_block(height)
                .ok_or(V2ApplyError::StateAheadOfKura)?
        } else {
            // WSV is already committed. The proposal body is deliberately
            // resultless, so recovery must authenticate and retain Kura's
            // canonical result-bearing execution image rather than replacing
            // it with the proposal carrier.
            let committed = self
                .kura
                .get_block(height)
                .ok_or(V2ApplyError::StateAheadOfKura)?;
            let committed_wire = committed
                .encode_wire()
                .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?;
            if committed
                .canonical_proposal_wire_hash()
                .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?
                != task.subject().payload_hash
                || u64::try_from(committed_wire.len()).ok()
                    != Some(
                        task.certificate()
                            .execution_commitment
                            .executed_block_wire_len,
                    )
                || Hash::new(&committed_wire)
                    != task
                        .certificate()
                        .execution_commitment
                        .executed_block_wire_hash
            {
                return Err(V2ApplyError::ExecutionCommitmentMismatch);
            }
            self.kura.store_block(Arc::clone(&committed))?;
            committed
        };
        let committed_block_hash = committed_block.hash();
        let executed_block_wire_hash = committed_block
            .executed_block_wire_hash()
            .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?;

        // Repair or confirm the durable finality boundary before any derived
        // publication. Fresh application already crossed this boundary inside
        // `validate_and_apply`; these calls are deliberately idempotent so
        // restart can repair each individual artifact.
        let receipt = self
            .kura
            .store_v2_finality_artifact(&artifact)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required("v2 finality artifact", &error)
            })?;

        // The strict restart-repair path authenticates Native AMX evidence
        // against both finality and the post-WSV Kura metadata join. Publish
        // that join first on every fresh or recovery attempt, then repair or
        // confirm the exact manifests, receipts, and latest indexes while the
        // prune guard keeps their canonical carrier stable.
        self.persist_post_apply_metadata(context, task, &artifact)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required("post-apply metadata", &error)
            })?;
        self.kura
            .repair_native_amx_participant_application_evidence(committed_block.as_ref())
            .map_err(|error| {
                V2ApplyError::committed_recovery_required(
                    "Native AMX participant manifest/receipt repair",
                    &error,
                )
            })?;

        self.publish_committed_block_merge_entry(committed_block.as_ref())?;

        // Queue ownership is a third durable boundary after Kura and WSV. An
        // exact retry reaches this point even when State already crossed its
        // commit boundary, so a crash cannot leave merge-applied transactions
        // permanently reserved or eligible for replay.
        finalize_committed_block_merge_reservations(
            self.state.as_ref(),
            self.queue.as_ref(),
            self.kura.as_ref(),
            committed_block.as_ref(),
        )
        .map_err(|error| {
            V2ApplyError::committed_recovery_required("merge reservation finalization", &error)
        })?;

        self.kura
            .promote_kagemusha_topup_finality_sidecar(&artifact, &receipt)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required(
                    "Kagemusha finality sidecar promotion",
                    &error,
                )
            })?;
        let artifact_hash = HashOf::new(&artifact);
        let evidence = DurableApplicationEvidence {
            task_tag: task.tag(),
            owner_tag: task.authorized_owner_tag(),
            task_generation: task.tag().generation().get(),
            task_work_id: task.id(),
            context: context.clone(),
            commit_qc: task.certificate().clone(),
            subject: task.subject(),
            execution_commitment: task.validated_receipt().execution_commitment(),
            validated_receipt: task.validated_receipt().clone(),
            validated_manifest_hash: task.validated_receipt().durable().manifest_hash(),
            validated_body_frame_hash: task.validated_receipt().durable().frame_hash(),
            proposal_block_hash,
            canonical_proposal_wire_hash,
            committed_block_hash,
            executed_block_wire_hash,
            kura_receipt: receipt,
            artifact,
            artifact_hash,
            completion_work_id: task.id(),
            state_height_after: self.state.committed_height(),
        };
        self.finish_durable_apply_completion_against(evidence, prospective_application)
    }

    #[cfg(test)]
    fn finish_durable_apply_completion(
        &self,
        evidence: DurableApplicationEvidence,
    ) -> Result<DurableApplyCompletion, V2ApplyError> {
        let application_trace = evidence
            .application_refinement_projection()
            .ok_or_else(|| {
                V2ApplyError::committed_recovery_required(
                    "application refinement evidence",
                    &"native application identity cannot be represented losslessly",
                )
            })?;
        let checked_application = check_production_application_transition(application_trace)
            .ok_or_else(|| {
                V2ApplyError::committed_recovery_required(
                    "application refinement evidence",
                    &"durable application does not refine its Decision completion",
                )
            })?;
        self.finish_durable_apply_completion_against(
            evidence,
            checked_application.into_projection(),
        )
    }

    fn finish_durable_apply_completion_against(
        &self,
        evidence: DurableApplicationEvidence,
        prospective_application: ProductionApplicationTraceProjection,
    ) -> Result<DurableApplyCompletion, V2ApplyError> {
        if !evidence.is_exact() {
            return Err(V2ApplyError::committed_recovery_required(
                "exact application evidence",
                &"native identity mismatch after durable application",
            ));
        }
        let application_trace = evidence
            .application_refinement_projection()
            .ok_or_else(|| {
                V2ApplyError::committed_recovery_required(
                    "application refinement evidence",
                    &"native application identity cannot be represented losslessly",
                )
            })?;
        if application_trace != prospective_application {
            return Err(V2ApplyError::committed_recovery_required(
                "application refinement evidence",
                &"durable application differs from its pre-authorized Decision completion",
            ));
        }
        Ok(DurableApplyCompletion::new(
            evidence.completion_work_id,
            evidence.kura_receipt,
            evidence.artifact,
        ))
    }

    fn publish_committed_block_merge_entry(
        &self,
        committed_block: &SignedBlock,
    ) -> Result<(), V2ApplyError> {
        let entry =
            committed_block_merge_entry(self.kura.as_ref(), committed_block).map_err(|error| {
                V2ApplyError::committed_recovery_required("merge cache publication", &error)
            })?;
        let Some(entry) = entry else {
            return Ok(());
        };
        self.state
            .ensure_globally_committed_merge_entry_applied(&entry)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required("merge cache publication", &error)
            })?;
        // The atomic State commit above is the authority that the merge batch
        // reached WSV. Publish its exact lane-application receipts only after
        // that check and before queue reservation finalization. The Kura
        // writer is idempotent, so a crash at any receipt/frontier boundary is
        // repaired by retrying this same committed carrier without replaying
        // economic effects.
        self.kura
            .persist_merge_lane_block_application_receipts_from_committed_log(&entry)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required(
                    "merge application receipt publication",
                    &error,
                )
            })?;
        let (_, event) = self
            .state
            .record_globally_committed_merge_entry(&entry, MergeLedgerPublicationMode::LiveCommit)
            .map_err(|error| {
                V2ApplyError::committed_recovery_required("merge cache publication", &error)
            })?;
        if let Some(event) = event {
            let _ = self.events_sender.send(EventBox::Pipeline(event));
        }
        Ok(())
    }

    fn retain_decided_merge_sidecar(
        &self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<(), V2ApplyError> {
        let reference = body
            .execution_context()
            .and_then(|bundle| bundle.merge_entry.as_ref());
        self.kura
            .retain_pending_certified_merge_entry_for_locked_carrier(context.height, reference)?;
        Ok(())
    }

    /// Run the exact production proposal validator without applying its state
    /// overlay.
    ///
    /// The body store calls this only after authenticating the immutable
    /// origin-view block signature. Dropping the returned `StateBlock` keeps
    /// Prepare validation side-effect free while exercising the same
    /// deterministic execution path used during application.
    pub(crate) fn validate_candidate(
        &self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<wire::ExecutionCommitment, V2ApplyError> {
        if !body.is_resultless_proposal() {
            return Err(V2ApplyError::ResultBearingProposal);
        }
        self.validate_lane_payload_plan(context, body)?;
        let merge_reference = body
            .execution_context()
            .and_then(|bundle| bundle.merge_entry.as_ref());
        let topology = Topology::new(context.roster.iter().map(|entry| entry.validator.clone()));
        let mut voting_block = None;
        let result = ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
            body.clone(),
            &topology,
            &self.chain_id,
            &self.genesis_account,
            &TimeSource::new_system(),
            self.block_cadence,
            crate::block::valid::SumeragiV2ValidationContext::from_height_context(context),
            self.state.as_ref(),
            &mut voting_block,
        )
        .unpack(|_| {});
        let (valid, mut state_block) = result.map_err(|(failed_block, error)| {
            Self::classify_candidate_validation_error(
                merge_reference,
                failed_block.as_ref(),
                error.as_ref(),
            )
        })?;
        let witness = state_block
            .take_exec_witness()
            .ok_or(V2ApplyError::ExecutionCommitmentUnavailable)?;
        let native_amx_manifest =
            crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
                valid.as_ref(),
            )
            .map_err(V2ApplyError::ExecutionCommitment)?;
        crate::sumeragi::exec::execution_commitment_from_validated_block(
            &witness,
            &native_amx_manifest,
            valid.as_ref(),
        )
        .map_err(|error| V2ApplyError::ExecutionCommitment(error.to_owned()))
    }

    fn validate_and_apply(
        &self,
        context: &wire::HeightContext,
        body: iroha_data_model::block::SignedBlock,
        store_block: bool,
        expected_execution_commitment: wire::ExecutionCommitment,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<(), V2ApplyError> {
        if !body.is_resultless_proposal() {
            return Err(V2ApplyError::ResultBearingProposal);
        }
        self.validate_lane_payload_plan(context, &body)?;
        let block_hash = body.hash();
        let merge_reference = body
            .execution_context()
            .and_then(|bundle| bundle.merge_entry.clone());
        let topology = Topology::new(context.roster.iter().map(|entry| entry.validator.clone()));
        let mut voting_block = None;
        let mut pipeline_events = Vec::new();
        let (valid_block, mut state_block) =
            ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
                body,
                &topology,
                &self.chain_id,
                &self.genesis_account,
                &TimeSource::new_system(),
                self.block_cadence,
                crate::block::valid::SumeragiV2ValidationContext::from_height_context(context),
                self.state.as_ref(),
                &mut voting_block,
            )
            .unpack(|event| pipeline_events.push(event))
            .map_err(|(failed_block, error)| {
                Self::classify_candidate_validation_error(
                    merge_reference.as_ref(),
                    failed_block.as_ref(),
                    error.as_ref(),
                )
            })?;
        let witness = state_block
            .take_exec_witness()
            .ok_or(V2ApplyError::ExecutionCommitmentUnavailable)?;
        let native_amx_manifest =
            crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
                valid_block.as_ref(),
            )
            .map_err(V2ApplyError::ExecutionCommitment)?;
        let actual_execution_commitment =
            crate::sumeragi::exec::execution_commitment_from_validated_block(
                &witness,
                &native_amx_manifest,
                valid_block.as_ref(),
            )
            .map_err(|error| V2ApplyError::ExecutionCommitment(error.to_owned()))?;
        if actual_execution_commitment != expected_execution_commitment {
            return Err(V2ApplyError::ExecutionCommitmentMismatch);
        }
        // Persist the witness-derived leaf/path projection before either the
        // canonical block log or WSV advances. Promotion is deliberately
        // deferred until Kura has durably persisted the exact finality
        // artifact; a crash at any intermediate point leaves an idempotent
        // stage that restart can complete without replaying committed state.
        self.kura.stage_kagemusha_topup_finality_sidecar(
            context.height,
            block_hash,
            &witness,
            expected_execution_commitment,
        )?;
        let committed_block = valid_block
            .commit_with_verified_v2_artifact(artifact, actual_execution_commitment)
            .unpack(|event| pipeline_events.push(event))
            .map_err(|(_, error)| V2ApplyError::Commit(error.to_string()))?;

        // Kura owns the first irreversible commit point. This call is also the
        // idempotent repair boundary for a durable block whose merge
        // association was interrupted after its block fsync.
        let pre_wsv_finality_receipt = if store_block {
            self.kura.store_block(committed_block.clone())?;
            #[cfg(test)]
            if self
                .fail_after_kura_store
                .swap(false, std::sync::atomic::Ordering::Relaxed)
            {
                return Err(V2ApplyError::InjectedCrashAfterKuraStore);
            }
            let receipt = self
                .kura
                .store_v2_finality_artifact(artifact)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required(
                        "pre-WSV v2 finality artifact",
                        &error,
                    )
                })?;
            Some(receipt)
        } else {
            None
        };
        let native_amx_prepublication = if store_block {
            Some(
                self.kura
                    .prepublish_native_amx_participant_application_evidence(
                        committed_block.as_ref(),
                    )
                    .map_err(|error| {
                        V2ApplyError::committed_recovery_required(
                            "pre-WSV Native AMX participant evidence publication",
                            &error,
                        )
                    })?,
            )
        } else {
            None
        };
        let native_amx_frontiers = State::native_amx_participant_frontier_markers(
            committed_block.as_ref(),
        )
        .map_err(|error| {
            V2ApplyError::committed_recovery_required(
                "pre-WSV Native AMX participant frontier projection",
                &error,
            )
        })?;
        let native_amx_prepublication_matches_state = match native_amx_prepublication.as_ref() {
            Some(token) => token.authenticates_state_frontiers(
                committed_block.as_ref(),
                &native_amx_manifest,
                artifact,
                &native_amx_frontiers,
            ),
            None => native_amx_frontiers.is_empty(),
        };
        if !native_amx_prepublication_matches_state {
            return Err(V2ApplyError::committed_recovery_required(
                "pre-WSV Native AMX participant evidence publication",
                &"read-back token differs from the exact State frontier projection",
            ));
        }

        // `apply_without_execution_with_verified_v2_finality` stages the
        // Native participant frontiers in the State overlay. Do not construct
        // that overlay until every canonical manifest leaf has a durable,
        // read-back-authenticated manifest/receipt/latest-index triple.
        let commit_topology = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect();
        let state_events = state_block
            .apply_without_execution_with_verified_v2_finality(&committed_block, commit_topology);

        // Stage the exact would-be committed WSV hash while the validated
        // `StateBlock` overlay is still available. Kura is already durable at
        // this point, so the checkpoint must cross its own fsync boundary
        // before live State can advance. This closes the otherwise
        // unrecoverable crash window where restart observes State at the Kura
        // tip but has no authenticated hash with which to distinguish the
        // exact committed overlay from stale or corrupted memory.
        //
        // The checkpoint deliberately remains unbound until
        // `persist_post_apply_metadata` publishes the complete commit
        // manifest. A crash before State commit replays the overlay and must
        // reproduce this byte-identical hash; a crash after State commit can
        // authenticate the already-applied tip directly.
        #[cfg(test)]
        let staged_snapshot_bytes_for_test = store_block
            .then(|| crate::snapshot::canonical_staged_state_snapshot_bytes(&state_block));
        if store_block {
            let staged_checkpoint =
                crate::snapshot::canonical_staged_state_snapshot_hash(&state_block);
            self.kura
                .store_wsv_checkpoint(context.height, block_hash, staged_checkpoint)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required("pre-WSV recovery checkpoint", &error)
                })?;
        }
        #[cfg(test)]
        if self
            .fail_after_wsv_checkpoint
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return Err(V2ApplyError::InjectedCrashAfterWsvCheckpoint);
        }
        // TODO: Add an automatic governed retention controller and deployment
        // policy before treating this bounded archive as suitable for indefinite
        // node operation. Explicit Kura-authenticated, sealed-CAS-approved prefix
        // compaction is available; reaching a configured ceiling without an
        // authorized retention decision intentionally remains fail-stop.
        if let Some(archive) = self.provider_ingest_finalized_archive.as_ref() {
            let receipt = pre_wsv_finality_receipt.as_ref().ok_or_else(|| {
                V2ApplyError::CommittedRecoveryRequired {
                    stage: "provider-ingest finalized archive capture",
                    detail: "the exact pre-WSV Kura finality receipt is unavailable".to_owned(),
                }
            })?;
            archive
                .capture_kura_authenticated_view(&state_block, self.kura.as_ref(), receipt)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required(
                        "provider-ingest finalized archive capture",
                        &error,
                    )
                })?;
            #[cfg(test)]
            if self
                .fail_after_provider_ingest_archive_capture
                .swap(false, std::sync::atomic::Ordering::Relaxed)
            {
                return Err(V2ApplyError::InjectedCrashAfterProviderIngestArchiveCapture);
            }
        }
        if let Some(archive) = self.reputation_finalized_archive.as_ref() {
            let receipt = pre_wsv_finality_receipt.as_ref().ok_or_else(|| {
                V2ApplyError::CommittedRecoveryRequired {
                    stage: "reputation finalized archive capture",
                    detail: "the exact pre-WSV Kura finality receipt is unavailable".to_owned(),
                }
            })?;
            archive
                .capture_kura_authenticated_view(&state_block, self.kura.as_ref(), receipt)
                .map_err(|error| {
                    V2ApplyError::committed_recovery_required(
                        "reputation finalized archive capture",
                        &error,
                    )
                })?;
            #[cfg(test)]
            if self
                .fail_after_reputation_archive_capture
                .swap(false, std::sync::atomic::Ordering::Relaxed)
            {
                return Err(V2ApplyError::InjectedCrashAfterReputationArchiveCapture);
            }
        }
        state_block.commit().map_err(|error| {
            V2ApplyError::committed_recovery_required("WSV publication after Kura commit", &error)
        })?;
        #[cfg(test)]
        if let Some(staged) = staged_snapshot_bytes_for_test {
            let committed = crate::snapshot::canonical_state_snapshot_bytes(self.state.as_ref());
            if staged != committed {
                panic!(
                    "staged/committed WSV snapshot mismatch after block commit: {}",
                    snapshot_mismatch_context(&staged, &committed),
                );
            }
        }

        self.queue.remove_committed_hashes(
            committed_block
                .as_ref()
                .external_entrypoints_cloned()
                .map(|entrypoint| HashOf::from_untyped_unchecked(Hash::from(entrypoint.hash()))),
            None,
        );
        let nexus = self.state.nexus_snapshot();
        let compliance = self.queue.lane_compliance_engine();
        self.queue
            .reconfigure_nexus_with_state(&nexus, self.state.as_ref(), compliance);

        for event in pipeline_events {
            let _ = self.events_sender.send(EventBox::Pipeline(event));
        }
        for event in state_events {
            let _ = self.events_sender.send(event);
        }
        Ok(())
    }

    fn persist_post_apply_metadata(
        &self,
        context: &wire::HeightContext,
        task: &ApplyTask,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<(), V2ApplyError> {
        let block_hash = task.subject().block_hash;
        let checkpoint = crate::snapshot::canonical_state_snapshot_hash(self.state.as_ref());
        self.kura
            .store_wsv_checkpoint(context.height, block_hash, checkpoint)?;
        let manifest =
            CommitManifest::new(context.height, block_hash, None, None, checkpoint, None)
                .with_authenticated_v2_commit_authority(artifact);
        self.kura.store_commit_manifest(manifest)?;
        Ok(())
    }

    #[cfg(test)]
    fn fail_after_kura_store_for_test(&self) {
        self.fail_after_kura_store
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(test)]
    fn fail_after_wsv_checkpoint_for_test(&self) {
        self.fail_after_wsv_checkpoint
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(test)]
    fn fail_after_provider_ingest_archive_capture_for_test(&self) {
        self.fail_after_provider_ingest_archive_capture
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(test)]
    fn fail_after_reputation_archive_capture_for_test(&self) {
        self.fail_after_reputation_archive_capture
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Fail-closed application or recovery failure.
#[derive(Debug, Error)]
pub(crate) enum V2ApplyError {
    /// Frozen wire input is malformed.
    #[error(transparent)]
    Wire(#[from] wire::ValidationError),
    /// Finality artifact is malformed.
    #[error(transparent)]
    Finality(#[from] wire::finality::V2FinalityValidationError),
    /// Frozen PoPs or the exact CommitQC failed cryptographic verification.
    #[error("invalid Sumeragi v2 durable finality cryptography: {0}")]
    FinalityCryptography(wire::finality::V2QuorumCertificateVerificationError),
    /// Exact-body loading or marker verification failed.
    #[error(transparent)]
    Body(#[from] super::v2_body_store::V2BodyStoreError),
    /// Kura persistence or canonical association failed.
    #[error(transparent)]
    Kura(#[from] crate::kura::Error),
    /// Apply task and frozen context do not identify one exact decision.
    #[error("Sumeragi v2 Apply task differs from its frozen context or body")]
    TaskMismatch,
    /// Height cannot be represented by local storage indexes.
    #[error("Sumeragi v2 decision height is not representable")]
    HeightOverflow,
    /// WSV is unexpectedly ahead of the decision.
    #[error("WSV height {state_height} is ahead of v2 decision height {decision_height}")]
    StateAhead {
        /// Current WSV height.
        state_height: usize,
        /// Decided height.
        decision_height: usize,
    },
    /// More than one unapplied height separates WSV and the decision.
    #[error("WSV height {state_height} has a gap before v2 decision height {decision_height}")]
    StateGap {
        /// Current WSV height.
        state_height: usize,
        /// Decided height.
        decision_height: usize,
    },
    /// Kura already contains a different block at the decided height.
    #[error("Kura contains a conflicting block at the Sumeragi v2 decision height")]
    KuraConflict,
    /// WSV reports application but Kura has no canonical block.
    #[error("WSV is ahead of Kura while completing a Sumeragi v2 decision")]
    StateAheadOfKura,
    /// Deterministic validation rejected the exact durable body.
    #[error("Sumeragi v2 application validation failed: {0}")]
    Validation(String),
    /// Proposal ingress carried execution results or a result-root commitment.
    #[error("Sumeragi v2 proposal body must be resultless")]
    ResultBearingProposal,
    /// Deterministic validation did not produce the StateBlock execution witness.
    #[error("Sumeragi v2 validation produced no execution witness")]
    ExecutionCommitmentUnavailable,
    /// Execution-witness projection itself was malformed.
    #[error("invalid Sumeragi v2 execution commitment: {0}")]
    ExecutionCommitment(String),
    /// A proposal or executed block could not be encoded canonically.
    #[error("invalid canonical Sumeragi v2 block: {0}")]
    CanonicalBlock(String),
    /// The signed or persisted execution result differs from deterministic replay.
    #[error("Sumeragi v2 execution commitment differs from deterministic validation")]
    ExecutionCommitmentMismatch,
    /// The candidate is otherwise valid but its exact certified merge sidecar
    /// has not reached durable local storage yet.
    #[error("certified merge sidecar `{}` is not available locally yet", reference.entry_hash)]
    MissingCertifiedMergeSidecar {
        /// Compact, certificate-bound reference used for bounded recovery.
        reference: CertifiedMergeLedgerReference,
    },
    /// Certificate-aware block commit conversion failed.
    #[error("Sumeragi v2 block commit conversion failed: {0}")]
    Commit(String),
    /// Kura or WSV crossed the canonical commit point but the complete durable transition failed.
    #[error("Sumeragi v2 committed transition requires restart recovery at {stage}: {detail}")]
    CommittedRecoveryRequired {
        /// Post-commit stage that could not be completed.
        stage: &'static str,
        /// Underlying persistence diagnostic.
        detail: String,
    },
    /// Test-only crash boundary after Kura commits and before WSV publication.
    #[cfg(test)]
    #[error("injected crash after Kura store and before WSV commit")]
    InjectedCrashAfterKuraStore,
    /// Test-only crash boundary after the staged WSV checkpoint and before
    /// live State publication.
    #[cfg(test)]
    #[error("injected crash after staged WSV checkpoint and before WSV commit")]
    InjectedCrashAfterWsvCheckpoint,
    /// Test-only crash boundary after the immutable provider-ingest
    /// projection is durable and before live State publication.
    #[cfg(test)]
    #[error("injected crash after provider-ingest archive capture and before WSV commit")]
    InjectedCrashAfterProviderIngestArchiveCapture,
    /// Test-only crash boundary after the immutable reputation projection is
    /// durable and before live State publication.
    #[cfg(test)]
    #[error("injected crash after reputation archive capture and before WSV commit")]
    InjectedCrashAfterReputationArchiveCapture,
}

impl V2ApplyError {
    fn committed_recovery_required(stage: &'static str, error: &impl std::fmt::Display) -> Self {
        Self::CommittedRecoveryRequired {
            stage,
            detail: error.to_string(),
        }
    }

    /// Return whether the live consensus process must stop producing output until restart.
    #[must_use]
    pub(crate) const fn requires_restart_recovery(&self) -> bool {
        match self {
            Self::Kura(error) => error.requires_restart_recovery(),
            Self::CommittedRecoveryRequired { .. } => true,
            #[cfg(test)]
            Self::InjectedCrashAfterKuraStore
            | Self::InjectedCrashAfterWsvCheckpoint
            | Self::InjectedCrashAfterProviderIngestArchiveCapture
            | Self::InjectedCrashAfterReputationArchiveCapture => true,
            _ => false,
        }
    }
}

impl BodyValidationError for V2ApplyError {
    fn missing_certified_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
        match self {
            Self::MissingCertifiedMergeSidecar { reference } => Some(reference),
            _ => None,
        }
    }
}

#[cfg(test)]
fn snapshot_mismatch_context(staged: &[u8], committed: &[u8]) -> String {
    let first_difference = staged
        .iter()
        .zip(committed)
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| staged.len().min(committed.len()));
    let context_start = first_difference.saturating_sub(256);
    let staged_end = first_difference.saturating_add(768).min(staged.len());
    let committed_end = first_difference.saturating_add(768).min(committed.len());
    format!(
        "first_difference={first_difference}, staged_len={}, committed_len={}, \
         staged_context={:?}, committed_context={:?}",
        staged.len(),
        committed.len(),
        String::from_utf8_lossy(&staged[context_start..staged_end]),
        String::from_utf8_lossy(&committed[context_start..committed_end]),
    )
}

#[cfg(test)]
mod tests {
    /// Compatibility shim kept inside the test module only for older focused
    /// fixtures which deliberately exercise the single-process no-network
    /// boundary. Production callers must handle the typed recovery plan.
    fn reconcile_lane_reservation_ownership(
        state: &State,
        queue: &Queue,
        kura: &Kura,
        verified_active_context: &VerifiedHeightContext,
    ) -> Result<LaneReservationReconciliationSummary, V2ReservationLifecycleError> {
        match plan_lane_reservation_ownership(state, queue, kura, verified_active_context)? {
            LaneReservationReconciliationPlanning::Ready(plan) => {
                apply_lane_reservation_reconciliation_plan(queue, kura, plan)
            }
            LaneReservationReconciliationPlanning::RecoverCanonicalBodies(needs) => {
                let height = needs.first().map_or(0, |need| need.height);
                Err(V2ReservationLifecycleError::MissingCanonicalBody { height })
            }
            LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(
                installs,
            ) => {
                let install = installs
                    .first()
                    .expect("historical recovery planning is never empty");
                Err(
                    V2ReservationLifecycleError::HistoricalRecoveryInstallationMissing {
                        recovery_id: install.recovery_id,
                        lane_id: install.reservation_group.identity.lane_id,
                    },
                )
            }
        }
    }

    include!("tests/v2_apply_unsealed_00.rs");
    include!("tests/v2_apply_unsealed_01.rs");
    include!("tests/v2_apply_unsealed_02.rs");
}
