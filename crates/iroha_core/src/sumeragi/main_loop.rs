//! The main event loop that powers sumeragi.
#![cfg_attr(test, allow(unnameable_test_items))]
// TODO: remove temporary allowances once NPoS scheduling and telemetry hooks are fully wired.
use std::{
    borrow::Cow,
    cell::Cell,
    collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry},
    convert::TryFrom,
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use blake3::{Hasher as Blake3Hasher, hash as blake3_hash};
use eyre::{Result, eyre};
use iroha_config::parameters::actual::{
    AdaptiveObservability, Common as CommonConfig, ConsensusMode, DaManifestPolicy,
    LaneConfig as LaneConfigSnapshot, ProofPolicy, Sumeragi as SumeragiConfig,
};
use iroha_crypto::{Hash, HashOf, MerkleTree, PrivateKey, Signature};
use iroha_data_model::{
    ChainId, Encode as _,
    account::AccountId,
    block::{BlockHeader, SignedBlock, consensus::SumeragiMembershipStatus},
    consensus::{
        ExecutionQcRecord, VALIDATOR_SET_HASH_VERSION_V1, ValidatorElectionOutcome,
        ValidatorElectionParameters, ValidatorSetCheckpoint, VrfEpochRecord, VrfLateRevealRecord,
        VrfParticipantRecord,
    },
    da::{
        commitment::DaCommitmentRecord,
        prelude::{DaCommitmentBundle, DaPinIntentBundle},
        types::StorageTicketId,
    },
    events::{EventBox, pipeline::PipelineEventBox},
    merge::{MergeCommitteeSignature, MergeQuorumCertificate},
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    peer::PeerId,
    sorafs::pin_registry::ManifestDigest,
    transaction::{Executable, SignedTransaction},
};
use iroha_logger::prelude::*;
use iroha_p2p::{Broadcast, Post, Priority, UpdateTopology};
use iroha_primitives::numeric::{Numeric, NumericSpec};
use iroha_primitives::time::TimeSource;
#[cfg(all(test, feature = "telemetry"))]
use rand::{SeedableRng, rngs::StdRng, seq::SliceRandom};
use sha2::{Digest as ShaDigest, Sha256};
use thiserror::Error;

// BLS signatures are mandatory for validators; consensus does not support non-BLS builds.
#[cfg(not(feature = "bls"))]
compile_error!(
    "The `bls` feature is mandatory for iroha_core consensus; rebuild with `--features bls`"
);

use iroha_data_model::consensus::CommitCertificate;
use iroha_genesis::GENESIS_DOMAIN_ID;

use super::{
    BackgroundPost, BackgroundRequest, GenesisWithPubKey, RbcStoreConfig, VotingBlock,
    WsvEpochRosterAdapter,
    collectors::{CollectorPlan, deterministic_collectors},
    consensus::{
        ExecWitness, ExecWitnessMsg, NPOS_TAG, PERMISSIONED_TAG, RbcDeliver, RbcInit, RbcReady,
        ValidatorIndex, qc_signer_count, rbc_deliver_preimage, rbc_ready_preimage, vote_preimage,
    },
    da::{GateReason, ManifestGateKind},
    election,
    epoch::{EpochManager, EpochSnapshot, VrfNoteResult},
    epoch_report,
    exec::{build_exec_vote_from_witness, parent_state_from_witness, post_state_from_witness},
    penalties::PenaltyApplier,
    rbc_status,
    stake_snapshot::{CommitStakeSnapshot, stake_map_from_world},
    *,
};
#[cfg_attr(not(feature = "telemetry"), allow(unused_imports))]
use crate::telemetry::Telemetry;
use crate::{
    EventsSender, IrohaNetwork, NetworkMessage,
    block::{BlockBuilder, BlockValidationError, ValidBlock},
    kura::{BlockCount, Kura},
    nexus::lane_relay::LaneRelayBroadcaster,
    peers_gossiper::PeersGossiperHandle,
    queue::{BackpressureState, Queue, RoutingDecision},
    state::{CellVecExt, StakeSnapshot, State, StateView, compute_confidential_feature_digest},
    sumeragi::evidence::EvidenceStore,
    telemetry::{MissingBlockFetchOutcome, MissingBlockFetchTargetKind},
    tx::AcceptedTransaction,
};

// Temporary lint allowances scoped to the Sumeragi submodules while NPoS scaffolding evolves.
macro_rules! allow_scaffold_mod {
    ($($name:ident),+ $(,)?) => {
        $(
            #[allow(
                dead_code,
                clippy::large_types_passed_by_value,
                clippy::needless_pass_by_value,
                clippy::unused_self,
                clippy::unnecessary_wraps,
                clippy::assigning_clones
            )]
            mod $name;
        )+
    };
}

allow_scaffold_mod!(
    background,
    block_sync,
    commit,
    exec_qc,
    kura,
    locked_qc,
    mode,
    pacing,
    pending_block,
    pending_rbc,
    proposal_handlers,
    proposals,
    propose,
    qc,
    rbc,
    reschedule,
    roster,
    validation,
    votes,
    vrf,
);

#[cfg(test)]
use exec_qc::{
    ExecQcGateStatus, ExecQcMissingParentDecision, exec_qc_gate_status,
    exec_qc_missing_parent_decision, persist_execution_qc_record_in_wsv,
};
use locked_qc::{
    LockedQcRejection, ensure_locked_qc_allows, qc_extends_locked_if_present,
    qc_extends_locked_with_lookup, realign_locked_to_committed_if_extends,
};
use pacing::{
    AdaptiveAction, AdaptiveObservabilityMetrics, AdaptiveObservabilityState, BackpressureGate,
    Pacemaker, PacemakerBackpressure, ProposeAttemptMonitor, TickTimingMonitor,
};
use pending_block::{
    DaGateStatus, PendingBlock, ValidationGateOutcome, ValidationStatus, recompute_da_gate_status,
    record_da_gate_telemetry,
};
use pending_rbc::PendingRbcMessages;
#[cfg(test)]
use pending_rbc::{PendingChunkOutcome, PendingRbcDropReason};
use proposal_handlers::invalid_proposal_evidence;
#[cfg(test)]
use proposals::ProposalMismatch;
use proposals::{
    ProposalCache, block_payload_bytes, detect_proposal_mismatch, evidence_within_horizon,
};
use roster::{
    apply_roster_indices_to_manager, canonicalize_roster, compute_membership_view_hash,
    compute_roster_indices_from_topology, derive_active_topology, derive_local_validator_index,
    roster_member_allowed_bls,
};
use vrf::VrfActor;
#[cfg(test)]
use vrf::derive_vrf_material_from_key;

#[cfg(feature = "dev-tests")]
pub(crate) mod test_time {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };

    static SCALE: AtomicU64 = AtomicU64::new(1);

    /// Set a time scale factor for tests (>=1). When >1, timeouts are divided by this scale.
    pub fn set_time_scale(scale: u64) {
        SCALE.store(scale.max(1), Ordering::Relaxed);
    }

    pub fn scale(d: Duration) -> Duration {
        let s = SCALE.load(Ordering::Relaxed);
        if s > 1 { d / s } else { d }
    }
}

/// Hard cap on RBC chunks per payload to bound memory usage (default chunk size 64 KiB → 64 MiB cap).
const RBC_MAX_TOTAL_CHUNKS: u32 = 1024;
/// Reserve room for headers/signatures when RBC is disabled so `BlockCreated` frames
/// stay under the consensus topic cap.
const NON_RBC_FRAME_HEADROOM_BYTES: usize = 8 * 1024;
/// Minimum backoff for consensus rebroadcasts to avoid tight loops on tiny block times.
const REBROADCAST_COOLDOWN_FLOOR: Duration = Duration::from_millis(200);
/// Base multiplier for consensus rebroadcasts (votes/block sync/READY).
const REBROADCAST_COOLDOWN_MULTIPLIER: u32 = 2;
/// Payload rebroadcasts (block payloads/RBC chunks) are heavier, so keep them slower.
const PAYLOAD_REBROADCAST_COOLDOWN_MULTIPLIER: u32 = 2;
/// EMA smoothing factor for pacemaker phase latencies.
pub(super) const PACEMAKER_PHASE_EMA_ALPHA: f64 = 0.2;
/// Log when the gap between tick invocations exceeds this threshold.
const TICK_LAG_LOG_THRESHOLD: Duration = Duration::from_millis(500);
/// Log when a single tick iteration takes longer than this threshold.
const TICK_COST_LOG_THRESHOLD: Duration = Duration::from_millis(200);
/// Log when commit-pipeline processing for a block exceeds this threshold.
const COMMIT_PIPELINE_BLOCK_LOG_THRESHOLD: Duration = Duration::from_millis(500);
/// Log when pending-block reschedule timing exceeds this threshold.
const RESCHEDULE_TIMING_LOG_THRESHOLD: Duration = Duration::from_millis(500);
/// Cooldown between consecutive tick timing logs to avoid log spam.
const TICK_TIMING_LOG_COOLDOWN: Duration = Duration::from_secs(5);
/// Cooldown between consecutive proposal-attempt logs when queued transactions exist.
const PROPOSE_ATTEMPT_LOG_COOLDOWN: Duration = Duration::from_secs(2);
/// Minimum backoff between consecutive quorum reschedules for the same pending block.
const QUORUM_RESCHEDULE_COOLDOWN: Duration = Duration::from_millis(800);
/// Align rebroadcast cadence with the block time while enforcing a safe floor.
fn rebroadcast_cooldown_from_block_time(block_time: Duration) -> Duration {
    let base = if block_time == Duration::ZERO {
        REBROADCAST_COOLDOWN_FLOOR
    } else {
        block_time.max(REBROADCAST_COOLDOWN_FLOOR)
    };
    saturating_mul_duration(base, REBROADCAST_COOLDOWN_MULTIPLIER)
}

/// Align payload rebroadcast cadence to the block time while keeping a larger buffer.
fn payload_rebroadcast_cooldown_from_block_time(block_time: Duration) -> Duration {
    saturating_mul_duration(
        rebroadcast_cooldown_from_block_time(block_time),
        PAYLOAD_REBROADCAST_COOLDOWN_MULTIPLIER,
    )
}

fn bump_view_after_quorum_timeout(
    phase_tracker: &mut PhaseTracker,
    pacemaker: &mut Pacemaker,
    new_view_tracker: &mut NewViewTracker,
    height: u64,
    view: u64,
    now: Instant,
) -> u64 {
    let next_view = view.saturating_add(1);
    phase_tracker.on_view_change(height, next_view, now);
    new_view_tracker.remove(height, view);
    new_view_tracker.drop_below_view(height, next_view);
    pacemaker.next_deadline = now;
    next_view
}

const PROPOSAL_CACHE_LIMIT: usize = 128;
fn missing_quorum_stale(
    pending_age: Duration,
    commit_timeout: Duration,
    quorum_reached: bool,
) -> bool {
    commit_timeout != Duration::ZERO && !quorum_reached && pending_age >= commit_timeout
}

fn emit_pipeline_events(events_sender: &EventsSender, events: Vec<PipelineEventBox>) {
    for event in events {
        if let Err(err) = events_sender.send(EventBox::Pipeline(event)) {
            debug!(?err, "failed to forward pipeline event");
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum CommitPipelineTrigger {
    Event,
    Tick,
}

fn prevote_quorum_stale(
    qc_phase: Option<crate::sumeragi::consensus::Phase>,
    pending_age: Duration,
    quorum_timeout: Duration,
) -> bool {
    matches!(qc_phase, Some(crate::sumeragi::consensus::Phase::Prepare))
        && quorum_timeout != Duration::ZERO
        && pending_age >= quorum_timeout
}

fn new_view_target(
    highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    target_height: Option<u64>,
    target_view: Option<u64>,
) -> (u64, u64) {
    let height = target_height.unwrap_or_else(|| highest_qc.height.saturating_add(1));
    let view = target_view.unwrap_or_else(|| highest_qc.view.saturating_add(1));
    (height, view)
}

fn pending_extends_tip(
    pending_height: u64,
    pending_parent: Option<HashOf<BlockHeader>>,
    state_height: usize,
    tip_hash: Option<HashOf<BlockHeader>>,
) -> bool {
    u64::try_from(state_height.saturating_add(1))
        .map(|expected_height| pending_height == expected_height && pending_parent == tip_hash)
        .unwrap_or(false)
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ParsedQcSigners {
    /// All unique signers present in the bitmap.
    present: BTreeSet<ValidatorIndex>,
    /// Signers that belong to the voting set (currently the full roster).
    voting: BTreeSet<ValidatorIndex>,
}

fn qc_voting_signer_count(qc: &crate::sumeragi::consensus::Qc, voting_len: usize) -> usize {
    let mut count = 0usize;
    for (byte_idx, byte) in qc.aggregate.signers_bitmap.iter().enumerate() {
        for bit in 0u8..8 {
            if byte & (1u8 << bit) == 0 {
                continue;
            }
            let idx = byte_idx * 8 + usize::from(bit);
            if idx < voting_len {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

fn voting_signer_count(signers: &BTreeSet<ValidatorIndex>, voting_len: usize) -> usize {
    signers
        .iter()
        .filter_map(|signer| usize::try_from(*signer).ok())
        .filter(|idx| *idx < voting_len)
        .count()
}

fn realign_qcs_after_failed_commit(
    locked: Option<crate::sumeragi::consensus::QcHeaderRef>,
    highest: Option<crate::sumeragi::consensus::QcHeaderRef>,
    failed_hash: HashOf<BlockHeader>,
    latest_committed: Option<crate::sumeragi::consensus::QcHeaderRef>,
) -> (
    Option<crate::sumeragi::consensus::QcHeaderRef>,
    Option<crate::sumeragi::consensus::QcHeaderRef>,
) {
    let mut new_locked = locked;
    let mut new_highest = highest;
    if matches!(
        new_locked,
        Some(lock) if lock.subject_block_hash == failed_hash
    ) {
        new_locked = latest_committed.or(new_locked);
    }
    if matches!(
        new_highest,
        Some(highest) if highest.subject_block_hash == failed_hash
    ) {
        new_highest = latest_committed.or(new_highest);
    }
    (new_locked, new_highest)
}

fn requeue_block_transactions(
    queue: &Queue,
    state: &State,
    txs: Vec<SignedTransaction>,
) -> (usize, usize, usize, Vec<HashOf<SignedTransaction>>) {
    let mut requeued = 0usize;
    let mut failures = 0usize;
    let mut duplicate_failures = 0usize;
    let mut gossip_hashes: Vec<_> = Vec::new();
    for tx in txs {
        let tx_hash = tx.hash();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let state_view = state.view();
        match queue.push(accepted, state_view) {
            Ok(()) => requeued = requeued.saturating_add(1),
            Err(push_err) => match push_err.err {
                crate::queue::Error::IsInQueue => {
                    duplicate_failures = duplicate_failures.saturating_add(1);
                    debug!("transaction already in queue during requeue; keeping pending block");
                }
                err => {
                    failures = failures.saturating_add(1);
                    warn!(?err, "failed to requeue transaction after commit failure");
                }
            },
        }
        gossip_hashes.push(tx_hash);
    }
    if !gossip_hashes.is_empty() {
        queue.requeue_gossip_hashes(gossip_hashes.clone());
    }
    (requeued, failures, duplicate_failures, gossip_hashes)
}

fn drop_pending_block_and_requeue(
    pending_blocks: &mut BTreeMap<HashOf<BlockHeader>, PendingBlock>,
    pending_hash: HashOf<BlockHeader>,
    queue: &Queue,
    state: &State,
) -> Option<(usize, usize, usize, usize)> {
    let pending = pending_blocks.remove(&pending_hash)?;
    let txs = pending.block.transactions_vec().clone();
    let tx_count = txs.len();
    let (requeued, failures, duplicate_failures, _) = requeue_block_transactions(queue, state, txs);
    Some((tx_count, requeued, failures, duplicate_failures))
}

#[inline]
fn drop_pending_after_requeue(failures: usize, _duplicate_failures: usize) -> bool {
    failures > 0
}

#[derive(Debug)]
struct QcCommitFailureOutcome {
    pending: PendingBlock,
    locked_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    highest_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    clean_block_hash: bool,
    requeued: usize,
    failed_requeues: usize,
    view_change_triggered: bool,
    drop_pending: bool,
}

#[derive(Debug)]
struct PrevBlockMismatchOutcome {
    requeued: usize,
    failures: usize,
}

#[allow(clippy::too_many_arguments)]
fn handle_commit_failure_with_qc_quorum(
    mut pending: PendingBlock,
    failed_block: SignedBlock,
    block_hash: HashOf<BlockHeader>,
    _pending_height: u64,
    _pending_view: u64,
    queue: &Queue,
    state: &State,
    locked_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    highest_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    latest_committed: Option<crate::sumeragi::consensus::QcHeaderRef>,
) -> QcCommitFailureOutcome {
    pending.tx_batch = None;
    let (requeued, failed_requeues, duplicate_requeues, _) =
        requeue_block_transactions(queue, state, failed_block.transactions_vec().clone());
    let (new_locked, new_highest) =
        realign_qcs_after_failed_commit(locked_qc, highest_qc, block_hash, latest_committed);
    pending.block = failed_block;
    pending.mark_aborted();

    QcCommitFailureOutcome {
        pending,
        locked_qc: new_locked,
        highest_qc: new_highest,
        clean_block_hash: true,
        requeued,
        failed_requeues,
        view_change_triggered: true,
        drop_pending: drop_pending_after_requeue(failed_requeues, duplicate_requeues),
    }
}

fn handle_prev_block_mismatch(
    queue: &Queue,
    state: &State,
    txs: Vec<SignedTransaction>,
) -> PrevBlockMismatchOutcome {
    let (requeued, failures, _duplicates, _) = requeue_block_transactions(queue, state, txs);
    PrevBlockMismatchOutcome { requeued, failures }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum QcValidationError {
    #[error("signer bitmap length mismatch (expected {expected}, got {actual})")]
    BitmapLengthMismatch { expected: usize, actual: usize },
    #[error("signer index {signer} is outside the active topology of length {topology_len}")]
    SignerOutOfBounds { signer: usize, topology_len: usize },
    #[error("QC has insufficient signers (collected {collected}, required {required})")]
    InsufficientSigners { collected: usize, required: usize },
    #[error("QC is missing {missing} required votes")]
    MissingVotes { missing: usize },
    #[error("QC contains duplicate signer bits")]
    DuplicateSigners,
    #[error("QC validator set does not match active roster")]
    ValidatorSetMismatch,
    #[error("QC aggregate does not match subject/bitmap")]
    AggregateMismatch,
    #[error("QC subject mismatch for signer {signer}")]
    SubjectMismatch { signer: ValidatorIndex },
    #[error("QC contains an invalid signature from signer {signer}")]
    InvalidSignature { signer: ValidatorIndex },
    #[error("QC signer {signer} not present in block signatures")]
    SignerMissingFromBlock { signer: ValidatorIndex },
}

impl QcValidationError {
    fn telemetry_reason(&self) -> &'static str {
        match self {
            Self::BitmapLengthMismatch { .. } => "bitmap_length_mismatch",
            Self::SignerOutOfBounds { .. } => "signer_out_of_bounds",
            Self::InsufficientSigners { .. } => "insufficient_signers",
            Self::MissingVotes { .. } => "missing_votes",
            Self::DuplicateSigners => "duplicate_signers",
            Self::ValidatorSetMismatch => "validator_set_mismatch",
            Self::AggregateMismatch => "aggregate_mismatch",
            Self::SubjectMismatch { .. } => "subject_mismatch",
            Self::InvalidSignature { .. } => "invalid_signature",
            Self::SignerMissingFromBlock { .. } => "signer_missing_from_block",
        }
    }
}

/// Result of QC validation, including the signers and coverage metadata.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct QcValidationOutcome {
    signers: Vec<ValidatorIndex>,
    missing_votes: usize,
    present_signers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QcSignerTally {
    voting_signers: BTreeSet<ValidatorIndex>,
    present_signers: usize,
}

impl QcSignerTally {
    fn voting_len(&self) -> usize {
        self.voting_signers.len()
    }
}

/// Duration to suppress repeated invalid-signature logs for the same peer/kind.
const INVALID_SIG_LOG_THROTTLE: Duration = Duration::from_secs(5);
/// Retain invalid-signature log entries for this long before pruning.
const INVALID_SIG_LOG_RETENTION: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum InvalidSigKind {
    Vote,
    RbcReady,
    RbcDeliver,
}

impl InvalidSigKind {
    #[cfg(feature = "telemetry")]
    fn label(self) -> &'static str {
        match self {
            Self::Vote => "vote",
            Self::RbcReady => "rbc_ready",
            Self::RbcDeliver => "rbc_deliver",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InvalidSigOutcome {
    Logged,
    Throttled,
}

impl InvalidSigOutcome {
    #[cfg(feature = "telemetry")]
    fn label(self) -> &'static str {
        match self {
            Self::Logged => "logged",
            Self::Throttled => "throttled",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct InvalidSigKey {
    kind: InvalidSigKind,
    signer: u64,
}

#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_field_names)]
struct InvalidSigState {
    last_logged_at: Instant,
    last_height: u64,
    last_view: u64,
}

#[derive(Default)]
struct InvalidSigThrottle {
    entries: BTreeMap<InvalidSigKey, InvalidSigState>,
}

impl InvalidSigThrottle {
    fn record(
        &mut self,
        kind: InvalidSigKind,
        height: u64,
        view: u64,
        signer: u64,
        now: Instant,
    ) -> InvalidSigOutcome {
        self.entries.retain(|_, entry| {
            now.saturating_duration_since(entry.last_logged_at) <= INVALID_SIG_LOG_RETENTION
        });

        let key = InvalidSigKey { kind, signer };
        match self.entries.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(InvalidSigState {
                    last_logged_at: now,
                    last_height: height,
                    last_view: view,
                });
                InvalidSigOutcome::Logged
            }
            Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                let elapsed = now.saturating_duration_since(state.last_logged_at);
                let advanced_height = height > state.last_height
                    || (height == state.last_height && view > state.last_view);
                if elapsed >= INVALID_SIG_LOG_THROTTLE || advanced_height {
                    *state = InvalidSigState {
                        last_logged_at: now,
                        last_height: height,
                        last_view: view,
                    };
                    InvalidSigOutcome::Logged
                } else {
                    InvalidSigOutcome::Throttled
                }
            }
        }
    }
}

fn qc_validation_reason(err: &QcValidationError) -> &'static str {
    err.telemetry_reason()
}

fn qc_validation_error_to_evidence(
    qc: &crate::sumeragi::consensus::Qc,
    err: &QcValidationError,
) -> Option<crate::sumeragi::consensus::Evidence> {
    use crate::sumeragi::consensus::{Evidence, EvidenceKind, EvidencePayload};
    match *err {
        QcValidationError::BitmapLengthMismatch { .. }
        | QcValidationError::SignerOutOfBounds { .. }
        | QcValidationError::InvalidSignature { .. }
        | QcValidationError::SignerMissingFromBlock { .. }
        | QcValidationError::ValidatorSetMismatch
        | QcValidationError::AggregateMismatch
        | QcValidationError::DuplicateSigners
        | QcValidationError::SubjectMismatch { .. } => Some(Evidence {
            kind: EvidenceKind::InvalidCommitCertificate,
            payload: EvidencePayload::InvalidCommitCertificate {
                certificate: qc.clone(),
                reason: qc_validation_reason(err).to_owned(),
            },
        }),
        QcValidationError::InsufficientSigners { .. } | QcValidationError::MissingVotes { .. } => {
            None
        }
    }
}

fn record_qc_validation_error(
    telemetry: Option<&crate::telemetry::Telemetry>,
    err: &QcValidationError,
) {
    if let Some(telemetry) = telemetry {
        telemetry.note_qc_validation_error(qc_validation_reason(err));
    }
}

fn validate_qc_with_evidence(
    vote_log: &BTreeMap<
        (
            crate::sumeragi::consensus::Phase,
            u64,
            u64,
            u64,
            crate::sumeragi::consensus::ValidatorIndex,
        ),
        crate::sumeragi::consensus::Vote,
    >,
    qc: &crate::sumeragi::consensus::Qc,
    topology: &super::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> (
    Result<QcValidationOutcome, QcValidationError>,
    Option<crate::sumeragi::consensus::Evidence>,
) {
    match validate_qc_against_votes(vote_log, qc, topology, chain_id, mode_tag, prf_seed) {
        Ok(outcome) => (Ok(outcome), None),
        Err(err) => (Err(err), qc_validation_error_to_evidence(qc, &err)),
    }
}

impl core::ops::Deref for QcValidationOutcome {
    type Target = [ValidatorIndex];

    fn deref(&self) -> &Self::Target {
        &self.signers
    }
}

fn qc_signer_indices(
    qc: &crate::sumeragi::consensus::Qc,
    topology_len: usize,
    voting_len: usize,
) -> Result<ParsedQcSigners, QcValidationError> {
    parse_signers_bitmap(&qc.aggregate.signers_bitmap, topology_len, voting_len)
}

fn parse_signers_bitmap(
    bitmap: &[u8],
    topology_len: usize,
    voting_len: usize,
) -> Result<ParsedQcSigners, QcValidationError> {
    let expected_len = topology_len.div_ceil(8);
    if bitmap.len() != expected_len {
        return Err(QcValidationError::BitmapLengthMismatch {
            expected: expected_len,
            actual: bitmap.len(),
        });
    }
    let mut parsed = ParsedQcSigners::default();
    for (byte_idx, byte) in bitmap.iter().enumerate() {
        for bit in 0u8..8 {
            if byte & (1u8 << bit) == 0 {
                continue;
            }
            let idx = byte_idx * 8 + usize::from(bit);
            if idx >= topology_len {
                return Err(QcValidationError::SignerOutOfBounds {
                    signer: idx,
                    topology_len,
                });
            }
            let signer = ValidatorIndex::try_from(idx).map_err(|_| {
                QcValidationError::SignerOutOfBounds {
                    signer: idx,
                    topology_len,
                }
            })?;
            if !parsed.present.insert(signer) {
                return Err(QcValidationError::DuplicateSigners);
            }
            if idx < voting_len {
                parsed.voting.insert(signer);
            }
        }
    }
    Ok(parsed)
}

fn normalize_signer_indices_to_canonical(
    signers: &BTreeSet<ValidatorIndex>,
    signature_topology: &super::network_topology::Topology,
    canonical_topology: &super::network_topology::Topology,
) -> BTreeSet<ValidatorIndex> {
    if signers.is_empty() {
        return BTreeSet::new();
    }
    let mut normalized = BTreeSet::new();
    for signer in signers {
        let Ok(idx) = usize::try_from(*signer) else {
            continue;
        };
        let Some(peer) = signature_topology.as_ref().get(idx) else {
            continue;
        };
        let Some(canonical_idx) = canonical_topology.as_ref().iter().position(|p| p == peer) else {
            continue;
        };
        if let Ok(canonical) = ValidatorIndex::try_from(canonical_idx) {
            normalized.insert(canonical);
        }
    }
    normalized
}

fn normalize_signer_indices_to_view(
    signers: &BTreeSet<ValidatorIndex>,
    signature_topology: &super::network_topology::Topology,
    canonical_topology: &super::network_topology::Topology,
) -> BTreeSet<ValidatorIndex> {
    if signers.is_empty() {
        return BTreeSet::new();
    }
    let mut normalized = BTreeSet::new();
    for signer in signers {
        if let Some(view_idx) =
            view_index_for_canonical_signer(*signer, signature_topology, canonical_topology)
        {
            normalized.insert(view_idx);
        }
    }
    normalized
}

fn view_index_for_canonical_signer(
    signer: ValidatorIndex,
    signature_topology: &super::network_topology::Topology,
    canonical_topology: &super::network_topology::Topology,
) -> Option<ValidatorIndex> {
    let idx = usize::try_from(signer).ok()?;
    let peer = canonical_topology.as_ref().get(idx)?;
    let view_idx = signature_topology.as_ref().iter().position(|p| p == peer)?;
    ValidatorIndex::try_from(view_idx).ok()
}

fn qc_bls_preimage(
    qc: &crate::sumeragi::consensus::Qc,
    chain_id: &ChainId,
    mode_tag: &str,
) -> Vec<u8> {
    let vote = crate::sumeragi::consensus::Vote {
        phase: qc.phase,
        block_hash: qc.subject_block_hash,
        height: qc.height,
        view: qc.view,
        epoch: qc.epoch,
        highest_cert: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    vote_preimage(chain_id, mode_tag, &vote)
}

fn exec_qc_bls_preimage(
    qc: &crate::sumeragi::consensus::ExecutionQC,
    chain_id: &ChainId,
    mode_tag: &str,
) -> Vec<u8> {
    let vote = crate::sumeragi::consensus::ExecVote {
        block_hash: qc.subject_block_hash,
        parent_state_root: qc.parent_state_root,
        post_state_root: qc.post_state_root,
        height: qc.height,
        view: qc.view,
        epoch: qc.epoch,
        signer: 0,
        bls_sig: Vec::new(),
    };
    crate::sumeragi::consensus::bls_preimage::exec_vote(chain_id, mode_tag, &vote)
}

fn qc_validator_set_matches_topology(
    qc: &crate::sumeragi::consensus::Qc,
    canonical_topology: &super::network_topology::Topology,
) -> bool {
    if qc.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1 {
        return false;
    }
    if HashOf::new(&qc.validator_set) != qc.validator_set_hash {
        return false;
    }
    qc.validator_set.as_slice() == canonical_topology.as_ref()
}

fn qc_aggregate_consistent(
    qc: &crate::sumeragi::consensus::Qc,
    canonical_topology: &super::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
) -> bool {
    if !qc_validator_set_matches_topology(qc, canonical_topology) {
        return false;
    }
    if qc.aggregate.bls_aggregate_signature.is_empty() {
        return false;
    }
    let roster_len = canonical_topology.as_ref().len();
    if roster_len == 0 {
        return false;
    }
    let parsed_signers = match qc_signer_indices(qc, roster_len, roster_len) {
        Ok(parsed) => parsed,
        Err(_) => return false,
    };
    if parsed_signers.present.is_empty() {
        return false;
    }
    let mut public_keys: Vec<&[u8]> = Vec::with_capacity(parsed_signers.present.len());
    for signer in &parsed_signers.present {
        let Ok(idx) = usize::try_from(*signer) else {
            return false;
        };
        let Some(peer) = canonical_topology.as_ref().get(idx) else {
            return false;
        };
        let (_, payload) = peer.public_key().to_bytes();
        public_keys.push(payload);
    }
    let preimage = qc_bls_preimage(qc, chain_id, mode_tag);
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &qc.aggregate.bls_aggregate_signature,
        &public_keys,
    )
    .is_ok()
}

fn exec_qc_aggregate_consistent(
    qc: &crate::sumeragi::consensus::ExecutionQC,
    signature_topology: &super::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
    signers: &ParsedQcSigners,
) -> bool {
    if qc.aggregate.bls_aggregate_signature.is_empty() {
        return false;
    }
    if signers.present.is_empty() {
        return false;
    }
    let mut public_keys: Vec<&[u8]> = Vec::with_capacity(signers.present.len());
    for signer in &signers.present {
        let Ok(idx) = usize::try_from(*signer) else {
            return false;
        };
        let Some(peer) = signature_topology.as_ref().get(idx) else {
            return false;
        };
        let (_, payload) = peer.public_key().to_bytes();
        public_keys.push(payload);
    }
    let preimage = exec_qc_bls_preimage(qc, chain_id, mode_tag);
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &qc.aggregate.bls_aggregate_signature,
        &public_keys,
    )
    .is_ok()
}

fn kickstart_pacemaker_after_commit<F>(
    queue_len: usize,
    defer_proposals: bool,
    mut trigger: F,
) -> bool
where
    F: FnMut(Instant) -> bool,
{
    if queue_len > 0 && !defer_proposals {
        let now = Instant::now();
        let _ = trigger(now);
        return true;
    }
    false
}

fn rotate_topology_for_mode(
    topology: &mut super::network_topology::Topology,
    height: u64,
    view: u64,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
    context: &'static str,
) {
    match mode_tag {
        PERMISSIONED_TAG => {
            if let Ok(view_usize) = usize::try_from(view) {
                topology.nth_rotation(view_usize);
            } else {
                warn!(
                    view,
                    "skipping topology rotation for {context}: view index exceeds usize"
                );
            }
        }
        NPOS_TAG => {
            if let Some(seed) = prf_seed {
                let leader = topology.leader_index_prf(seed, height, view);
                topology.rotate_preserve_view_to_front(leader);
            } else {
                warn!(
                    height,
                    view, "skipping topology rotation for {context}: missing PRF seed"
                );
            }
        }
        _ => {}
    }
}

fn topology_for_block_signatures(
    block: &SignedBlock,
    topology: &super::network_topology::Topology,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> super::network_topology::Topology {
    let mut rotated = topology.clone();
    let view = u64::from(block.header().view_change_index());
    let height = block.header().height().get();
    rotate_topology_for_mode(
        &mut rotated,
        height,
        view,
        mode_tag,
        prf_seed,
        "block signatures",
    );
    rotated
}

pub(super) fn topology_for_view(
    topology: &super::network_topology::Topology,
    height: u64,
    view: u64,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> super::network_topology::Topology {
    let mut rotated = topology.clone();
    rotate_topology_for_mode(
        &mut rotated,
        height,
        view,
        mode_tag,
        prf_seed,
        "QC validation",
    );
    rotated
}

pub(crate) fn validated_block_signers(
    block: &SignedBlock,
    topology: &super::network_topology::Topology,
    state_view: &StateView<'_>,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Result<
    BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
    crate::block::SignatureVerificationError,
> {
    let signature_topology = topology_for_block_signatures(block, topology, mode_tag, prf_seed);
    crate::block::ValidBlock::validate_signatures_subset(block, &signature_topology, state_view)?;

    let mut signers = BTreeSet::new();
    for signature in signature_topology.filter_signatures_by_roles(
        &[
            super::network_topology::Role::Leader,
            super::network_topology::Role::ValidatingPeer,
            super::network_topology::Role::SetBValidator,
            super::network_topology::Role::ProxyTail,
        ],
        block.signatures(),
    ) {
        let signer = crate::sumeragi::consensus::ValidatorIndex::try_from(signature.index())
            .map_err(|_| crate::block::SignatureVerificationError::UnknownSignatory)?;
        signers.insert(signer);
    }
    Ok(signers)
}

pub(crate) fn validate_block_sync_qc(
    qc: &crate::sumeragi::consensus::Qc,
    topology: &super::network_topology::Topology,
    block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
    block_view: u64,
    chain_id: &ChainId,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Result<(BTreeSet<crate::sumeragi::consensus::ValidatorIndex>, usize), QcValidationError> {
    let signature_topology = topology_for_view(topology, qc.height, qc.view, mode_tag, prf_seed);
    let roster_len = topology.as_ref().len();
    let required = signature_topology.min_votes_for_commit().max(1);
    let voting_len = roster_len;
    if !qc_validator_set_matches_topology(qc, topology) {
        return Err(QcValidationError::ValidatorSetMismatch);
    }
    let parsed_signers = qc_signer_indices(qc, roster_len, voting_len)?;
    if parsed_signers.voting.len() < required {
        return Err(QcValidationError::InsufficientSigners {
            collected: parsed_signers.voting.len(),
            required,
        });
    }
    if !qc_aggregate_consistent(qc, topology, chain_id, mode_tag) {
        return Err(QcValidationError::AggregateMismatch);
    }
    let block_signature_topology =
        topology_for_view(topology, qc.height, block_view, mode_tag, prf_seed);
    let normalized_block_signers =
        normalize_signer_indices_to_canonical(block_signers, &block_signature_topology, topology);
    if normalized_block_signers.len() >= required {
        if let Some(missing) = parsed_signers
            .voting
            .iter()
            .find(|signer| !normalized_block_signers.contains(signer))
            .copied()
        {
            return Err(QcValidationError::SignerMissingFromBlock { signer: missing });
        }
    }
    // Return the raw bitmap indices so cached signers reproduce the QC bitmap correctly.
    Ok((parsed_signers.voting, parsed_signers.present.len()))
}

#[allow(clippy::too_many_arguments)]
fn derive_block_sync_qc_from_signers(
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    block_view: u64,
    block_epoch: u64,
    commit_topology: &[PeerId],
    mode_tag: &str,
    block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
    aggregate_signature: Vec<u8>,
) -> Option<crate::sumeragi::consensus::Qc> {
    let roster_len = commit_topology.len();
    if roster_len == 0 {
        return None;
    }
    let min_votes = if roster_len > 3 {
        ((roster_len.saturating_sub(1)) / 3) * 2 + 1
    } else {
        roster_len
    };
    if block_signers.len() < min_votes {
        warn!(
            incoming_hash = %block_hash,
            block_signers = block_signers.len(),
            min_votes,
            "dropping derived block sync QC: insufficient commit signatures"
        );
        return None;
    }
    if block_signers.iter().any(|idx| {
        usize::try_from(*idx)
            .ok()
            .is_some_and(|val| val >= roster_len)
    }) {
        warn!(
            incoming_hash = %block_hash,
            roster_len,
            "dropping derived block sync QC: block signer index exceeds roster length"
        );
        return None;
    }
    if aggregate_signature.is_empty() {
        warn!(
            incoming_hash = %block_hash,
            block_signers = block_signers.len(),
            "dropping derived block sync QC: missing aggregate signature"
        );
        return None;
    }
    let mut signers_bitmap = vec![0u8; roster_len.div_ceil(8)];
    for signer in block_signers {
        let Ok(idx) = usize::try_from(*signer) else {
            continue;
        };
        if idx >= roster_len {
            continue;
        }
        let byte = idx / 8;
        let bit = idx % 8;
        signers_bitmap[byte] |= 1u8 << bit;
    }
    let validator_set = commit_topology.to_vec();
    Some(crate::sumeragi::consensus::Qc {
        phase: crate::sumeragi::consensus::Phase::Commit,
        subject_block_hash: block_hash,
        height: block_height,
        view: block_view,
        epoch: block_epoch,
        mode_tag: mode_tag.to_string(),
        highest_cert: None,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set,
        aggregate: crate::sumeragi::consensus::CommitAggregate {
            signers_bitmap,
            bls_aggregate_signature: aggregate_signature,
        },
    })
}

#[cfg(test)]
fn tally_qc_against_votes(
    vote_log: &BTreeMap<
        (
            crate::sumeragi::consensus::Phase,
            u64,
            u64,
            u64,
            crate::sumeragi::consensus::ValidatorIndex,
        ),
        crate::sumeragi::consensus::Vote,
    >,
    qc: &crate::sumeragi::consensus::Qc,
    topology: &super::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Result<QcSignerTally, QcValidationError> {
    validate_qc_against_votes(vote_log, qc, topology, chain_id, mode_tag, prf_seed).map(
        |QcValidationOutcome {
             signers,
             present_signers,
             ..
         }| QcSignerTally {
            voting_signers: signers.into_iter().collect(),
            present_signers,
        },
    )
}

fn tally_qc_against_block_signers(
    qc: &crate::sumeragi::consensus::Qc,
    topology: &super::network_topology::Topology,
    block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
    block_view: u64,
    chain_id: &ChainId,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Result<QcSignerTally, QcValidationError> {
    let _ = validate_block_sync_qc(
        qc,
        topology,
        block_signers,
        block_view,
        chain_id,
        mode_tag,
        prf_seed,
    )?;
    // Keep bitmap indices as-is so cached signers reproduce the QC bitmap correctly.
    let roster_len = topology.as_ref().len();
    let parsed = qc_signer_indices(qc, roster_len, roster_len)?;
    Ok(QcSignerTally {
        voting_signers: parsed.voting,
        present_signers: parsed.present.len(),
    })
}

fn validate_qc_against_votes(
    vote_log: &BTreeMap<
        (
            crate::sumeragi::consensus::Phase,
            u64,
            u64,
            u64,
            crate::sumeragi::consensus::ValidatorIndex,
        ),
        crate::sumeragi::consensus::Vote,
    >,
    qc: &crate::sumeragi::consensus::Qc,
    topology: &super::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Result<QcValidationOutcome, QcValidationError> {
    let signature_topology = topology_for_view(topology, qc.height, qc.view, mode_tag, prf_seed);
    let roster_len = topology.as_ref().len();
    let required = signature_topology.min_votes_for_commit();
    let voting_len = roster_len;
    if !qc_validator_set_matches_topology(qc, topology) {
        return Err(QcValidationError::ValidatorSetMismatch);
    }
    let parsed_signers = qc_signer_indices(qc, roster_len, voting_len)?;
    if parsed_signers.voting.len() < required {
        return Err(QcValidationError::InsufficientSigners {
            collected: parsed_signers.voting.len(),
            required,
        });
    }

    if !qc_aggregate_consistent(qc, topology, chain_id, mode_tag) {
        return Err(QcValidationError::AggregateMismatch);
    }
    let mut missing = 0usize;
    for signer in &parsed_signers.voting {
        let Some(view_signer) =
            view_index_for_canonical_signer(*signer, &signature_topology, topology)
        else {
            let signer = usize::try_from(*signer).unwrap_or(usize::MAX);
            return Err(QcValidationError::SignerOutOfBounds {
                signer,
                topology_len: roster_len,
            });
        };
        let key = (qc.phase, qc.height, qc.view, qc.epoch, view_signer);
        let Some(vote) = vote_log.get(&key) else {
            missing += 1;
            continue;
        };
        if vote.block_hash != qc.subject_block_hash {
            return Err(QcValidationError::SubjectMismatch { signer: *signer });
        }
        if !vote_signature_valid(vote, &signature_topology, chain_id, mode_tag) {
            return Err(QcValidationError::InvalidSignature { signer: *signer });
        }
    }

    if missing > 0 {
        return Err(QcValidationError::MissingVotes { missing });
    }

    Ok(QcValidationOutcome {
        signers: parsed_signers.voting.into_iter().collect(),
        missing_votes: missing,
        present_signers: parsed_signers.present.len(),
    })
}

fn validate_execution_qc(
    qc: &crate::sumeragi::consensus::ExecutionQC,
    topology: &super::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Result<(), QcValidationError> {
    let signature_topology = topology_for_view(topology, qc.height, qc.view, mode_tag, prf_seed);
    let roster_len = signature_topology.as_ref().len();
    let required = signature_topology.min_votes_for_commit().max(1);
    let parsed_signers =
        parse_signers_bitmap(&qc.aggregate.signers_bitmap, roster_len, roster_len)?;
    if parsed_signers.voting.len() < required {
        return Err(QcValidationError::InsufficientSigners {
            collected: parsed_signers.voting.len(),
            required,
        });
    }
    if !exec_qc_aggregate_consistent(qc, &signature_topology, chain_id, mode_tag, &parsed_signers) {
        return Err(QcValidationError::AggregateMismatch);
    }
    Ok(())
}

fn fallback_qc_tally_from_bitmap(
    qc: &crate::sumeragi::consensus::Qc,
    topology: &super::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Option<QcSignerTally> {
    let _signature_topology = topology_for_view(topology, qc.height, qc.view, mode_tag, prf_seed);
    if !qc_aggregate_consistent(qc, topology, chain_id, mode_tag) {
        return None;
    }
    let roster_len = topology.as_ref().len();
    let parsed_signers = qc_signer_indices(qc, roster_len, roster_len).ok()?;
    Some(QcSignerTally {
        voting_signers: parsed_signers.voting.into_iter().collect(),
        present_signers: parsed_signers.present.len(),
    })
}

fn build_signers_bitmap(signers: &BTreeSet<ValidatorIndex>, roster_len: usize) -> Vec<u8> {
    if roster_len == 0 {
        return Vec::new();
    }
    let mut bitmap = vec![0u8; roster_len.div_ceil(8)];
    for signer in signers {
        let Ok(idx) = usize::try_from(*signer) else {
            continue;
        };
        if idx >= roster_len {
            continue;
        }
        let byte = idx / 8;
        let bit = idx % 8;
        bitmap[byte] |= 1u8 << bit;
    }
    bitmap
}

fn aggregate_vote_signatures(
    vote_log: &BTreeMap<
        (
            crate::sumeragi::consensus::Phase,
            u64,
            u64,
            u64,
            crate::sumeragi::consensus::ValidatorIndex,
        ),
        crate::sumeragi::consensus::Vote,
    >,
    phase: crate::sumeragi::consensus::Phase,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    epoch: u64,
    signers: &BTreeSet<ValidatorIndex>,
) -> Result<Vec<u8>, QcAggregateError> {
    if signers.is_empty() {
        return Err(QcAggregateError::AggregateFailed);
    }
    let mut signatures = Vec::with_capacity(signers.len());
    for signer in signers {
        let key = (phase, height, view, epoch, *signer);
        let Some(vote) = vote_log.get(&key) else {
            return Err(QcAggregateError::MissingVote { signer: *signer });
        };
        if vote.block_hash != block_hash {
            return Err(QcAggregateError::MissingVote { signer: *signer });
        }
        if vote.bls_sig.is_empty() {
            return Err(QcAggregateError::MissingBlsSignature { signer: *signer });
        }
        signatures.push(vote.bls_sig.as_slice());
    }
    iroha_crypto::bls_normal_aggregate_signatures(&signatures)
        .map_err(|_| QcAggregateError::AggregateFailed)
}

#[allow(clippy::too_many_arguments)]
fn aggregate_exec_vote_signatures(
    exec_vote_log: &BTreeMap<
        (u64, u64, u64, crate::sumeragi::consensus::ValidatorIndex),
        crate::sumeragi::consensus::ExecVote,
    >,
    block_hash: HashOf<BlockHeader>,
    parent_state_root: Hash,
    post_state_root: Hash,
    height: u64,
    view: u64,
    epoch: u64,
    signers: &BTreeSet<ValidatorIndex>,
) -> Result<Vec<u8>, ExecQcAggregateError> {
    if signers.is_empty() {
        return Err(ExecQcAggregateError::AggregateFailed);
    }
    let mut signatures = Vec::with_capacity(signers.len());
    for signer in signers {
        let key = (height, view, epoch, *signer);
        let Some(vote) = exec_vote_log.get(&key) else {
            return Err(ExecQcAggregateError::MissingVote { signer: *signer });
        };
        if vote.block_hash != block_hash
            || vote.parent_state_root != parent_state_root
            || vote.post_state_root != post_state_root
        {
            return Err(ExecQcAggregateError::MismatchedVote { signer: *signer });
        }
        if vote.bls_sig.is_empty() {
            return Err(ExecQcAggregateError::MissingBlsSignature { signer: *signer });
        }
        signatures.push(vote.bls_sig.as_slice());
    }
    iroha_crypto::bls_normal_aggregate_signatures(&signatures)
        .map_err(|_| ExecQcAggregateError::AggregateFailed)
}

fn exec_vote_groups_for_block(
    exec_vote_log: &BTreeMap<
        (u64, u64, u64, crate::sumeragi::consensus::ValidatorIndex),
        crate::sumeragi::consensus::ExecVote,
    >,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    epoch: u64,
) -> BTreeMap<(Hash, Hash), BTreeSet<ValidatorIndex>> {
    let mut groups: BTreeMap<(Hash, Hash), BTreeSet<ValidatorIndex>> = BTreeMap::new();
    for vote in exec_vote_log.values() {
        if vote.block_hash != block_hash
            || vote.height != height
            || vote.view != view
            || vote.epoch != epoch
        {
            continue;
        }
        groups
            .entry((vote.parent_state_root, vote.post_state_root))
            .or_default()
            .insert(vote.signer);
    }
    groups
}

fn vote_signature_valid(
    vote: &crate::sumeragi::consensus::Vote,
    topology: &super::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
) -> bool {
    vote_signature_check(vote, topology, chain_id, mode_tag).is_ok()
}

/// Errors produced while verifying consensus vote signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VoteSignatureError {
    /// Signer index cannot be represented as usize.
    SignerIndexOverflow(u64),
    /// Signer index exceeds the active roster length.
    SignerOutOfRange {
        /// Signer index from the vote payload.
        signer: u32,
        /// Length of the active roster used for validation.
        roster_len: u32,
    },
    /// Signature payload fails cryptographic validation.
    SignatureInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QcAggregateError {
    MissingVote {
        signer: crate::sumeragi::consensus::ValidatorIndex,
    },
    MissingBlsSignature {
        signer: crate::sumeragi::consensus::ValidatorIndex,
    },
    AggregateFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecQcAggregateError {
    MissingVote {
        signer: crate::sumeragi::consensus::ValidatorIndex,
    },
    MismatchedVote {
        signer: crate::sumeragi::consensus::ValidatorIndex,
    },
    MissingBlsSignature {
        signer: crate::sumeragi::consensus::ValidatorIndex,
    },
    AggregateFailed,
}

pub(super) fn vote_signature_check(
    vote: &crate::sumeragi::consensus::Vote,
    topology: &super::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
) -> Result<(), VoteSignatureError> {
    let signer_raw = vote.signer;
    let Ok(idx) = usize::try_from(signer_raw) else {
        return Err(VoteSignatureError::SignerIndexOverflow(u64::from(
            signer_raw,
        )));
    };
    let Some(peer) = topology.as_ref().get(idx) else {
        return Err(VoteSignatureError::SignerOutOfRange {
            signer: idx.try_into().unwrap_or(u32::MAX),
            roster_len: topology.as_ref().len().try_into().unwrap_or(u32::MAX),
        });
    };
    let preimage = vote_preimage(chain_id, mode_tag, vote);
    if vote.bls_sig.is_empty() {
        return Err(VoteSignatureError::SignatureInvalid);
    }
    let bls_signature = Signature::from_bytes(&vote.bls_sig);
    bls_signature
        .verify(peer.public_key(), &preimage)
        .map_err(|_| VoteSignatureError::SignatureInvalid)
}

pub(super) fn exec_vote_signature_check(
    vote: &crate::sumeragi::consensus::ExecVote,
    topology: &super::network_topology::Topology,
    chain_id: &ChainId,
    mode_tag: &str,
) -> Result<(), VoteSignatureError> {
    let signer_raw = vote.signer;
    let Ok(idx) = usize::try_from(signer_raw) else {
        return Err(VoteSignatureError::SignerIndexOverflow(u64::from(
            signer_raw,
        )));
    };
    let Some(peer) = topology.as_ref().get(idx) else {
        return Err(VoteSignatureError::SignerOutOfRange {
            signer: idx.try_into().unwrap_or(u32::MAX),
            roster_len: topology.as_ref().len().try_into().unwrap_or(u32::MAX),
        });
    };
    if vote.bls_sig.is_empty() {
        return Err(VoteSignatureError::SignatureInvalid);
    }
    let preimage = crate::sumeragi::consensus::bls_preimage::exec_vote(chain_id, mode_tag, vote);
    let bls_signature = Signature::from_bytes(&vote.bls_sig);
    bls_signature
        .verify(peer.public_key(), &preimage)
        .map_err(|_| VoteSignatureError::SignatureInvalid)
}

/// Run stateless and stateful validation for a block before emitting votes.
fn validate_block_for_voting(
    block: SignedBlock,
    topology: &mut super::network_topology::Topology,
    chain_id: &ChainId,
    genesis_account: &AccountId,
    state: &State,
    voting_block: &mut Option<VotingBlock>,
) -> Result<(), BlockValidationError> {
    let time_source = TimeSource::new_system();
    let validation = ValidBlock::validate_keep_voting_block(
        block,
        topology,
        chain_id,
        genesis_account,
        &time_source,
        state,
        voting_block,
        false,
    )
    .unpack(|_| {});

    match validation {
        Ok((_validated, state_block)) => {
            drop(state_block);
            Ok(())
        }
        Err((_block, err)) => Err(*err),
    }
}

const VALIDATION_REASON_STATELESS: &str = "stateless";
const VALIDATION_REASON_EXECUTION: &str = "execution";
const VALIDATION_REASON_PREV_HASH: &str = "prev_hash";
const VALIDATION_REASON_PREV_HEIGHT: &str = "prev_height";
const VALIDATION_REASON_TOPOLOGY: &str = "topology";

fn validation_reject_reason_label(err: &BlockValidationError) -> &'static str {
    match err {
        BlockValidationError::PrevBlockHashMismatch { .. } => VALIDATION_REASON_PREV_HASH,
        BlockValidationError::PrevBlockHeightMismatch { .. } => VALIDATION_REASON_PREV_HEIGHT,
        BlockValidationError::TopologyMismatch { .. } => VALIDATION_REASON_TOPOLOGY,
        BlockValidationError::HasCommittedTransactions
        | BlockValidationError::EmptyBlock
        | BlockValidationError::DuplicateTransactions
        | BlockValidationError::TransactionAccept(_)
        | BlockValidationError::MerkleRootMismatch
        | BlockValidationError::SignatureVerification(_)
        | BlockValidationError::DaShardCursor(_)
        | BlockValidationError::AxtEnvelopeValidationFailed(_) => VALIDATION_REASON_EXECUTION,
        BlockValidationError::ConfidentialFeaturesMismatch { .. }
        | BlockValidationError::ProofPolicyHashMismatch { .. }
        | BlockValidationError::ViewChangeIndexTooLarge
        | BlockValidationError::InvalidGenesis(_)
        | BlockValidationError::BlockInThePast
        | BlockValidationError::BlockInTheFuture
        | BlockValidationError::TransactionInTheFuture => VALIDATION_REASON_STATELESS,
    }
}

fn proposer_index_from_block(block: &SignedBlock) -> u32 {
    block
        .signatures()
        .next()
        .and_then(|sig| u32::try_from(sig.index()).ok())
        .unwrap_or(0)
}

fn build_invalid_proposal_evidence(
    block: &SignedBlock,
    payload_hash: Hash,
    qc: crate::sumeragi::consensus::QcHeaderRef,
    reason: String,
) -> crate::sumeragi::consensus::Evidence {
    let proposer = proposer_index_from_block(block);
    let view = u64::from(block.header().view_change_index());
    let proposal = Actor::build_consensus_proposal(block, payload_hash, qc, proposer, view);
    invalid_proposal_evidence(proposal, reason)
}

#[cfg(test)]
fn new_view_highest_qc_phase_valid(qc: &crate::sumeragi::consensus::Qc) -> bool {
    matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit)
}

#[derive(Debug, Clone)]
struct MissingBlockRequest {
    height: u64,
    first_seen: Instant,
    last_requested: Instant,
    view_change_triggered: bool,
    attempts: u32,
}

impl MissingBlockRequest {
    /// Return true once the missing-block dwell time exceeds the retry window. Subsequent calls
    /// after triggering return false to avoid repeated view changes.
    fn mark_view_change_if_due(&mut self, now: Instant, retry_window: Duration) -> bool {
        if self.view_change_triggered || retry_window == Duration::ZERO {
            return false;
        }
        let dwell = now.saturating_duration_since(self.first_seen);
        if dwell >= retry_window {
            self.view_change_triggered = true;
            return true;
        }
        false
    }
}

fn touch_missing_block_request(
    requests: &mut BTreeMap<HashOf<BlockHeader>, MissingBlockRequest>,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    now: Instant,
    retry_window: Duration,
) -> bool {
    match requests.entry(block_hash) {
        Entry::Vacant(vacant) => {
            vacant.insert(MissingBlockRequest {
                height,
                first_seen: now,
                last_requested: now,
                view_change_triggered: false,
                attempts: 0,
            });
            true
        }
        Entry::Occupied(mut occupied) => {
            let stats = occupied.get_mut();
            stats.height = stats.height.max(height);
            let retry_due = now.saturating_duration_since(stats.last_requested) >= retry_window;
            if retry_due {
                stats.last_requested = now;
            }
            retry_due
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum MissingBlockFetchDecision {
    Requested {
        targets: Vec<PeerId>,
        target_kind: MissingBlockFetchTargetKind,
    },
    Backoff,
    NoTargets,
}

/// Plan a missing-block fetch on QC-first arrival, respecting retry backoff and signer preference
/// with a configurable fallback to the full commit topology.
#[allow(clippy::too_many_arguments)]
fn plan_missing_block_fetch(
    requests: &mut BTreeMap<HashOf<BlockHeader>, MissingBlockRequest>,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
    topology: &super::network_topology::Topology,
    now: Instant,
    retry_window: Duration,
    signer_fallback_attempts: u32,
) -> MissingBlockFetchDecision {
    if !touch_missing_block_request(requests, block_hash, height, now, retry_window) {
        return MissingBlockFetchDecision::Backoff;
    }

    let attempts = requests.get(&block_hash).map_or(0, |stats| stats.attempts);
    let prefer_signers =
        !signers.is_empty() && signer_fallback_attempts > 0 && attempts < signer_fallback_attempts;
    let signer_targets: Vec<_> = signers
        .iter()
        .filter_map(|signer| usize::try_from(*signer).ok())
        .filter_map(|idx| topology.as_ref().get(idx).cloned())
        .collect();
    let (targets, target_kind) = if prefer_signers && !signer_targets.is_empty() {
        (signer_targets, MissingBlockFetchTargetKind::Signers)
    } else {
        (
            topology.as_ref().to_vec(),
            MissingBlockFetchTargetKind::Topology,
        )
    };
    if targets.is_empty() {
        MissingBlockFetchDecision::NoTargets
    } else {
        if let Some(stats) = requests.get_mut(&block_hash) {
            stats.attempts = stats.attempts.saturating_add(1);
            stats.last_requested = now;
        }
        MissingBlockFetchDecision::Requested {
            targets,
            target_kind,
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn block_sync_quorum_available(
    block_signers: usize,
    _commit_quorum: usize,
    signature_quorum_met: bool,
    candidate_qc_present: bool,
    commit_cert_present: bool,
    _missing_block_requested: bool,
    _block_height: u64,
    _local_height: u64,
) -> bool {
    // Missing-block fetches still require some signed evidence unless a QC/cert is present.
    commit_cert_present || candidate_qc_present || signature_quorum_met || block_signers > 0
}

fn send_missing_block_request(
    network: &IrohaNetwork,
    peer_id: &PeerId,
    block_hash: HashOf<BlockHeader>,
    targets: &[PeerId],
) {
    if targets.is_empty() {
        return;
    }

    let request = super::message::FetchPendingBlock {
        requester: peer_id.clone(),
        block_hash,
    };
    let message = BlockMessage::FetchPendingBlock(request);
    let post = crate::NetworkMessage::SumeragiBlock(Box::new(message));
    for peer in targets {
        network.post(Post {
            data: post.clone(),
            peer_id: peer.clone(),
            priority: Priority::High,
        });
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn defer_qc_for_missing_block<F>(
    block_known: bool,
    retry_window: Duration,
    now: Instant,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    phase: crate::sumeragi::consensus::Phase,
    signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
    topology: &super::network_topology::Topology,
    signer_fallback_attempts: u32,
    requests: &mut BTreeMap<HashOf<BlockHeader>, MissingBlockRequest>,
    telemetry: Option<&crate::telemetry::Telemetry>,
    mut request_missing_block: F,
) -> bool
where
    F: FnMut(&[PeerId]),
{
    if block_known {
        return false;
    }

    let telemetry_ref = telemetry;
    if let Some(telemetry) = telemetry_ref {
        telemetry.set_missing_block_retry_window_ms(
            retry_window.as_millis().try_into().unwrap_or(u64::MAX),
        );
    }

    let decision = plan_missing_block_fetch(
        requests,
        block_hash,
        height,
        signers,
        topology,
        now,
        retry_window,
        signer_fallback_attempts,
    );
    let (dwell, since_last_request, attempts) = requests.get(&block_hash).map_or(
        (Duration::ZERO, Duration::ZERO, 0),
        |stats: &MissingBlockRequest| {
            (
                now.saturating_duration_since(stats.first_seen),
                now.saturating_duration_since(stats.last_requested),
                stats.attempts,
            )
        },
    );
    let dwell_ms = dwell.as_millis().try_into().unwrap_or(u64::MAX);
    let since_last_ms = since_last_request
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let retry_window_ms = retry_window.as_millis();

    let (outcome, targets_len, target_kind) = match &decision {
        MissingBlockFetchDecision::Requested {
            targets,
            target_kind,
        } => {
            request_missing_block(targets);
            iroha_logger::info!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                targets = ?targets,
                target_kind = target_kind.label(),
                dwell_ms,
                since_last_ms,
                attempts,
                "requested missing block payload from fetch targets"
            );
            (
                MissingBlockFetchOutcome::Requested,
                targets.len(),
                Some(*target_kind),
            )
        }
        MissingBlockFetchDecision::NoTargets => {
            iroha_logger::warn!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                dwell_ms,
                retry_window_ms,
                since_last_ms,
                attempts,
                "deferring QC aggregation but no peers available to request missing block"
            );
            (MissingBlockFetchOutcome::NoTargets, 0, None)
        }
        MissingBlockFetchDecision::Backoff => {
            iroha_logger::info!(
                height,
                view,
                phase = ?phase,
                block = ?block_hash,
                dwell_ms,
                retry_window_ms,
                since_last_ms,
                attempts,
                "deferring QC aggregation during missing-block fetch backoff"
            );
            (MissingBlockFetchOutcome::Backoff, 0, None)
        }
    };

    if let Some(telemetry) = telemetry_ref {
        telemetry.note_missing_block_fetch(outcome, targets_len, dwell, target_kind);
    }
    super::status::record_missing_block_fetch(targets_len, dwell_ms);

    if let Some(telemetry) = telemetry_ref {
        let oldest_ms = requests
            .values()
            .filter_map(|stats| stats.first_seen.elapsed().as_millis().try_into().ok())
            .min()
            .unwrap_or(0);
        telemetry.set_missing_block_inflight(requests.len(), oldest_ms);
    }

    iroha_logger::info!(
        height,
        view,
        phase = ?phase,
        block = ?block_hash,
        dwell_ms,
        since_last_ms,
        attempts,
        "deferring QC aggregation: block payload not yet known locally"
    );
    true
}

fn clear_missing_block_request(
    requests: &mut BTreeMap<HashOf<BlockHeader>, MissingBlockRequest>,
    block_hash: &HashOf<BlockHeader>,
) -> Option<MissingBlockRequest> {
    requests.remove(block_hash)
}

impl Actor {
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn request_missing_parent(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        block_height: u64,
        block_view: u64,
        parent_hash: HashOf<BlockHeader>,
        commit_topology: &[PeerId],
        roster_hint: Option<&[PeerId]>,
        expected_height: Option<usize>,
        actual_height: Option<usize>,
        trigger: &'static str,
    ) {
        if self.block_payload_available_locally(parent_hash) {
            return;
        }
        let local_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
        if block_height <= local_height.saturating_add(1) {
            return;
        }
        let gap = block_height.saturating_sub(local_height.saturating_add(1));
        let parent_height = block_height.saturating_sub(1);
        let (consensus_mode, _mode_tag, _prf_seed) =
            self.consensus_context_for_height(parent_height);
        let (roster, roster_source) = if let Some(selection) = persisted_roster_for_block(
            self.state.as_ref(),
            &self.kura,
            consensus_mode,
            parent_height,
            parent_hash,
            None,
        )
        .or_else(|| {
            block_sync_history_roster_for_block(
                self.state.as_ref(),
                consensus_mode,
                parent_hash,
                parent_height,
                None,
            )
        }) {
            (selection.roster, selection.source.as_str())
        } else if let Some(hint) = roster_hint.filter(|hint| !hint.is_empty()) {
            (hint.to_vec(), "roster_hint")
        } else if !commit_topology.is_empty() {
            (commit_topology.to_vec(), "commit_topology")
        } else {
            let view = self.state.view();
            let roster = derive_active_topology(
                &view,
                self.common_config.trusted_peers.value(),
                self.common_config.peer.id(),
            );
            drop(view);
            (roster, "trusted_peers")
        };
        if roster.is_empty() {
            debug!(
                height = block_height,
                view = block_view,
                block = %block_hash,
                missing_parent = ?parent_hash,
                trigger,
                "skipping missing-parent fetch: empty roster"
            );
            return;
        }

        let topology = super::network_topology::Topology::new(roster);
        let mut retry_window = self.quorum_timeout(self.runtime_da_enabled());
        if gap > 1 {
            let fast_retry = self.rebroadcast_cooldown();
            if fast_retry < retry_window {
                retry_window = fast_retry;
            }
        }
        let now = Instant::now();
        let signers = BTreeSet::new();
        let mut requests = core::mem::take(&mut self.pending.missing_block_requests);
        let decision = plan_missing_block_fetch(
            &mut requests,
            parent_hash,
            parent_height,
            &signers,
            &topology,
            now,
            retry_window,
            self.config.missing_block_signer_fallback_attempts,
        );
        self.pending.missing_block_requests = requests;
        let dwell = self
            .pending
            .missing_block_requests
            .get(&parent_hash)
            .map(|stats| now.saturating_duration_since(stats.first_seen))
            .unwrap_or_default();
        let targets_len = match &decision {
            MissingBlockFetchDecision::Requested { targets, .. } => targets.len(),
            _ => 0,
        };
        self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);

        match decision {
            MissingBlockFetchDecision::Requested {
                targets,
                target_kind,
            } => {
                self.request_missing_block(parent_hash, &targets);
                info!(
                    height = block_height,
                    view = block_view,
                    expected_height = ?expected_height,
                    actual_height = ?actual_height,
                    local_height,
                    gap,
                    block = %block_hash,
                    missing_parent = ?parent_hash,
                    targets = ?targets,
                    target_kind = target_kind.label(),
                    roster_source,
                    trigger,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms = dwell.as_millis(),
                    "requested missing parent block"
                );
            }
            MissingBlockFetchDecision::NoTargets => {
                warn!(
                    height = block_height,
                    view = block_view,
                    expected_height = ?expected_height,
                    actual_height = ?actual_height,
                    local_height,
                    gap,
                    block = %block_hash,
                    missing_parent = ?parent_hash,
                    roster_source,
                    trigger,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms = dwell.as_millis(),
                    "missing parent fetch deferred: no targets available"
                );
            }
            MissingBlockFetchDecision::Backoff => {
                trace!(
                    height = block_height,
                    view = block_view,
                    expected_height = ?expected_height,
                    actual_height = ?actual_height,
                    local_height,
                    gap,
                    block = %block_hash,
                    missing_parent = ?parent_hash,
                    roster_source,
                    trigger,
                    retry_window_ms = retry_window.as_millis(),
                    dwell_ms = dwell.as_millis(),
                    "missing parent fetch skipped due to backoff"
                );
            }
        }
    }

    pub(super) fn request_missing_parents_for_gap(
        &mut self,
        commit_topology: &[PeerId],
        roster_hint: Option<&[PeerId]>,
        trigger: &'static str,
    ) {
        let local_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
        let mut parents = Vec::new();
        let mut seen = BTreeSet::new();
        for pending in self.pending.pending_blocks.values() {
            if pending.height <= local_height.saturating_add(1) {
                continue;
            }
            let Some(parent_hash) = pending.block.header().prev_block_hash() else {
                continue;
            };
            if !seen.insert(parent_hash) {
                continue;
            }
            parents.push((
                pending.block.hash(),
                pending.height,
                u64::from(pending.block.header().view_change_index()),
                parent_hash,
            ));
        }
        for (block_hash, block_height, block_view, parent_hash) in parents {
            self.request_missing_parent(
                block_hash,
                block_height,
                block_view,
                parent_hash,
                commit_topology,
                roster_hint,
                None,
                None,
                trigger,
            );
        }
    }
}

#[derive(Debug)]
struct NewViewEntry {
    senders: BTreeSet<ValidatorIndex>,
    highest_qc: crate::sumeragi::consensus::QcHeaderRef,
}

impl NewViewEntry {
    fn new(highest_qc: crate::sumeragi::consensus::QcHeaderRef) -> Self {
        Self {
            senders: BTreeSet::new(),
            highest_qc,
        }
    }

    fn update_highest(&mut self, candidate: crate::sumeragi::consensus::QcHeaderRef) {
        let incoming = (candidate.height, candidate.view);
        let existing = (self.highest_qc.height, self.highest_qc.view);
        let promotes_phase = incoming == existing
            && candidate.phase == crate::sumeragi::consensus::Phase::Commit
            && self.highest_qc.phase != crate::sumeragi::consensus::Phase::Commit;
        if incoming > existing || promotes_phase {
            self.highest_qc = candidate;
        }
    }

    fn count_with_local(&self, local: Option<ValidatorIndex>) -> usize {
        let mut count = self.senders.len();
        if let Some(idx) = local {
            if !self.senders.contains(&idx) {
                count = count.saturating_add(1);
            }
        }
        count
    }
}

#[derive(Debug, Default)]
struct NewViewTracker {
    entries: BTreeMap<(u64, u64), NewViewEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NewViewSelection {
    key: (u64, u64),
    highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    quorum: usize,
}

#[derive(Clone, Debug)]
struct LaneAllocation {
    lane_id: LaneId,
    tx_count: u64,
    rbc_bytes_total: u64,
    teu_total: u64,
    total_chunks: u32,
}

#[derive(Clone, Debug)]
struct DataspaceAllocation {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    tx_count: u64,
    rbc_bytes_total: u64,
    teu_total: u64,
    total_chunks: u32,
}

fn build_commitment_snapshots_from_totals(
    lane_totals: BTreeMap<LaneId, (u64, u64, u64, u64)>,
    dataspace_totals: BTreeMap<(LaneId, DataSpaceId), (u64, u64, u64, u64)>,
    block_hash: HashOf<BlockHeader>,
    height: u64,
) -> (
    Vec<super::status::LaneCommitmentSnapshot>,
    Vec<super::status::DataspaceCommitmentSnapshot>,
) {
    let lane_commitments = lane_totals
        .into_iter()
        .map(
            |(lane_id, (tx_count, total_chunks, rbc_bytes_total, teu_total))| {
                super::status::LaneCommitmentSnapshot {
                    block_height: height,
                    lane_id: lane_id.as_u32(),
                    tx_count,
                    total_chunks,
                    rbc_bytes_total,
                    teu_total,
                    block_hash,
                }
            },
        )
        .collect();

    let dataspace_commitments = dataspace_totals
        .into_iter()
        .map(
            |((lane_id, dataspace_id), (tx_count, total_chunks, rbc_bytes_total, teu_total))| {
                super::status::DataspaceCommitmentSnapshot {
                    block_height: height,
                    lane_id: lane_id.as_u32(),
                    dataspace_id: dataspace_id.as_u64(),
                    tx_count,
                    total_chunks,
                    rbc_bytes_total,
                    teu_total,
                    block_hash,
                }
            },
        )
        .collect();

    (lane_commitments, dataspace_commitments)
}

fn interleave_lane_indices(routing_decisions: &[RoutingDecision]) -> Vec<usize> {
    let total = routing_decisions.len();
    if total <= 1 {
        return (0..total).collect();
    }
    let mut per_lane: BTreeMap<LaneId, VecDeque<usize>> = BTreeMap::new();
    for (idx, decision) in routing_decisions.iter().enumerate() {
        per_lane.entry(decision.lane_id).or_default().push_back(idx);
    }

    let mut order = Vec::with_capacity(total);
    while order.len() < total {
        let mut progress = false;
        for indices in per_lane.values_mut() {
            if let Some(idx) = indices.pop_front() {
                order.push(idx);
                progress = true;
                if order.len() == total {
                    break;
                }
            }
        }
        if !progress {
            break;
        }
    }

    if order.len() == total {
        order
    } else {
        (0..total).collect()
    }
}

fn round_duration_ms(value: f64) -> u64 {
    #[allow(clippy::cast_precision_loss)]
    let upper = u64::MAX as f64;
    let clamped = value.clamp(0.0, upper);
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    {
        clamped.round() as u64
    }
}

impl NewViewTracker {
    fn record(
        &mut self,
        height: u64,
        view: u64,
        sender: ValidatorIndex,
        highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    ) -> usize {
        let entry = self
            .entries
            .entry((height, view))
            .or_insert_with(|| NewViewEntry::new(highest_qc));
        entry.update_highest(highest_qc);
        entry.senders.insert(sender);
        entry.senders.len()
    }

    #[cfg(test)]
    fn count(&self, height: u64, view: u64) -> usize {
        self.entries
            .get(&(height, view))
            .map_or(0, |entry| entry.senders.len())
    }

    #[cfg(test)]
    fn count_with_local(&self, height: u64, view: u64, local: Option<ValidatorIndex>) -> usize {
        self.entries
            .get(&(height, view))
            .map_or(0, |entry| entry.count_with_local(local))
    }

    fn prune(&mut self, committed_height: u64) {
        self.entries
            .retain(|(entry_height, _), _| *entry_height > committed_height);
    }

    fn remove(&mut self, height: u64, view: u64) {
        self.entries.remove(&(height, view));
    }

    fn drop_below_view(&mut self, height: u64, min_view: u64) {
        self.entries.retain(|(entry_height, entry_view), _| {
            *entry_height != height || *entry_view >= min_view
        });
    }

    fn highest_entry_mut<F>(&mut self, mut predicate: F) -> Option<((u64, u64), &mut NewViewEntry)>
    where
        F: FnMut(u64, u64, &NewViewEntry) -> bool,
    {
        for (key, entry) in self.entries.iter_mut().rev() {
            if predicate(key.0, key.1, entry) {
                return Some((*key, entry));
            }
        }
        None
    }

    fn select_with_quorum(
        &mut self,
        required: usize,
        local: Option<ValidatorIndex>,
    ) -> Option<NewViewSelection> {
        self.highest_entry_mut(|_, _, entry| entry.count_with_local(local) >= required)
            .map(|(key, entry)| {
                if let Some(idx) = local {
                    entry.senders.insert(idx);
                }
                NewViewSelection {
                    key,
                    highest_qc: entry.highest_qc,
                    quorum: entry.senders.len(),
                }
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HintMismatch {
    BlockHash,
    Height,
    View,
    HighestQcHeight,
    HighestQcParentHash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MissingBlockClearReason {
    /// The block payload is available locally (committed or buffered).
    PayloadAvailable,
    /// The request is obsolete (e.g., conflicts with an already committed block).
    Obsolete,
}

impl MissingBlockClearReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::PayloadAvailable => "payload_available",
            Self::Obsolete => "obsolete",
        }
    }
}

const fn missing_block_clear_allowed(
    block_known_locally: bool,
    reason: MissingBlockClearReason,
) -> bool {
    match reason {
        MissingBlockClearReason::PayloadAvailable => block_known_locally,
        MissingBlockClearReason::Obsolete => true,
    }
}

fn empty_block_disfavored(tx_count: usize, queue_len: usize, has_nonempty_pending: bool) -> bool {
    tx_count == 0 && (queue_len > 0 || has_nonempty_pending)
}

fn non_rbc_payload_budget(
    block_max_payload_bytes: Option<NonZeroUsize>,
    consensus_frame_cap: usize,
) -> usize {
    let frame_cap = consensus_frame_cap.saturating_sub(NON_RBC_FRAME_HEADROOM_BYTES);
    let config_cap = block_max_payload_bytes.map_or(frame_cap, NonZeroUsize::get);
    config_cap.min(frame_cap)
}

fn is_peer_admin_instruction(instr: &iroha_data_model::isi::InstructionBox) -> bool {
    let id = iroha_data_model::isi::Instruction::id(&**instr).to_ascii_lowercase();
    id.contains("registerpeer") || id.contains("unregisterpeer")
}

#[allow(
    dead_code,
    clippy::large_types_passed_by_value,
    clippy::needless_pass_by_value,
    clippy::unused_self,
    clippy::unnecessary_wraps,
    clippy::assigning_clones
)]
impl Actor {
    fn active_pending_blocks_len(&self) -> usize {
        self.pending
            .pending_blocks
            .values()
            .filter(|pending| !pending.aborted)
            .count()
    }

    fn has_active_pending_blocks(&self) -> bool {
        self.pending
            .pending_blocks
            .values()
            .any(|pending| !pending.aborted)
    }

    fn is_peer_admin_transaction(tx: &AcceptedTransaction<'_>) -> bool {
        match tx.as_ref().instructions() {
            Executable::Instructions(batch) => batch.iter().any(is_peer_admin_instruction),
            _ => false,
        }
    }
}

fn pending_block_stale_for_tip(
    pending_height: u64,
    pending_parent: Option<HashOf<BlockHeader>>,
    committed_height: usize,
    committed_hash: Option<HashOf<BlockHeader>>,
) -> bool {
    let Some(committed_hash) = committed_hash else {
        return false;
    };
    let expected_parent_height =
        u64::try_from(committed_height.saturating_add(1)).unwrap_or(u64::MAX);
    expected_parent_height == pending_height
        && (pending_parent.is_none() || pending_parent != Some(committed_hash))
}

fn chain_extends_tip<F>(
    mut head: HashOf<BlockHeader>,
    mut height: u64,
    tip_height: u64,
    tip_hash: HashOf<BlockHeader>,
    mut parent_lookup: F,
) -> Option<bool>
where
    F: FnMut(HashOf<BlockHeader>, u64) -> Option<HashOf<BlockHeader>>,
{
    if height < tip_height {
        return Some(false);
    }
    if height == tip_height {
        return Some(head == tip_hash);
    }

    while height > tip_height {
        let parent = parent_lookup(head, height)?;
        head = parent;
        height = height.saturating_sub(1);
    }

    Some(height == tip_height && head == tip_hash)
}

fn child_qc_extending_lock<F, I>(
    lock: crate::sumeragi::consensus::QcHeaderRef,
    qcs: I,
    mut parent_lookup: F,
) -> Option<crate::sumeragi::consensus::QcHeaderRef>
where
    F: FnMut(HashOf<BlockHeader>, u64) -> Option<HashOf<BlockHeader>>,
    I: IntoIterator<Item = crate::sumeragi::consensus::QcHeaderRef>,
{
    let target_height = lock.height.checked_add(1)?;
    qcs.into_iter().find(|candidate| {
        candidate.phase == crate::sumeragi::consensus::Phase::Commit
            && candidate.height == target_height
            && (parent_lookup(candidate.subject_block_hash, candidate.height)
                == Some(lock.subject_block_hash))
    })
}

type QcVoteKey = (
    crate::sumeragi::consensus::Phase,
    HashOf<BlockHeader>,
    u64,
    u64,
    u64,
);

struct QcBuildContext {
    phase: crate::sumeragi::consensus::Phase,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    epoch: u64,
    mode_tag: String,
    highest_cert: Option<crate::sumeragi::consensus::CommitCertificateRef>,
}

fn qc_cache_for_subject(
    qc_cache: &BTreeMap<QcVoteKey, crate::sumeragi::consensus::Qc>,
    subject: HashOf<BlockHeader>,
) -> impl Iterator<Item = &crate::sumeragi::consensus::Qc> {
    qc_cache
        .iter()
        .filter(move |((_, hash, _, _, _), _)| *hash == subject)
        .map(|(_, qc)| qc)
}

fn cached_qc_for(
    qc_cache: &BTreeMap<QcVoteKey, crate::sumeragi::consensus::Qc>,
    phase: crate::sumeragi::consensus::Phase,
    hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
) -> Option<crate::sumeragi::consensus::Qc> {
    qc_cache_for_subject(qc_cache, hash)
        .find(|qc| qc.phase == phase && qc.height == height && qc.view == view)
        .cloned()
}

fn stale_qc_candidates<'a, I, F, G>(
    votes: I,
    required: usize,
    block_known: F,
    qc_present: G,
) -> Vec<(QcVoteKey, usize)>
where
    I: IntoIterator<Item = &'a crate::sumeragi::consensus::Vote>,
    F: Fn(HashOf<BlockHeader>) -> bool,
    G: Fn(&QcVoteKey) -> bool,
{
    let mut grouped: BTreeMap<QcVoteKey, BTreeSet<crate::sumeragi::consensus::ValidatorIndex>> =
        BTreeMap::new();
    for vote in votes {
        if !matches!(
            vote.phase,
            crate::sumeragi::consensus::Phase::Prepare | crate::sumeragi::consensus::Phase::Commit
        ) {
            continue;
        }
        let key = (
            vote.phase,
            vote.block_hash,
            vote.height,
            vote.view,
            vote.epoch,
        );
        grouped.entry(key).or_default().insert(vote.signer);
    }

    grouped
        .into_iter()
        .filter_map(|(key, signers)| {
            if signers.len() < required || qc_present(&key) || !block_known(key.1) {
                return None;
            }
            Some((key, signers.len()))
        })
        .collect()
}

fn rebuild_qc_candidates_with<'a, I, F, G, H>(
    votes: I,
    required: usize,
    block_known: F,
    qc_present: G,
    mut handle: H,
) -> Vec<(QcVoteKey, usize)>
where
    I: IntoIterator<Item = &'a crate::sumeragi::consensus::Vote>,
    F: Fn(HashOf<BlockHeader>) -> bool,
    G: Fn(&QcVoteKey) -> bool,
    H: FnMut(QcVoteKey, usize),
{
    let candidates = stale_qc_candidates(votes, required, block_known, qc_present);
    for (key, count) in &candidates {
        handle(*key, *count);
    }
    candidates
}

/// Lightweight wrapper around the consensus main loop.
///
/// This struct subsumes the former stub actor as we integrate the
/// vote/QC pipeline (`roadmap.md`, Milestone A3). For now it records inbound
/// traffic and wires ancillary services (RBC persistence, telemetry handles) so
/// follow-up patches can focus on state-machine transitions.
pub(super) struct Actor {
    config: SumeragiConfig,
    consensus_mode: ConsensusMode,
    common_config: CommonConfig,
    chain_id: ChainId,
    chain_hash: Hash,
    consensus_frame_cap: usize,
    events_sender: EventsSender,
    state: Arc<State>,
    queue: Arc<Queue>,
    kura: Arc<Kura>,
    network: IrohaNetwork,
    subsystems: ActorSubsystems,
    #[allow(dead_code)] // Retained until background broadcast wiring consumes the gossiper handle.
    peers_gossiper: PeersGossiperHandle,
    #[allow(dead_code)] // Retained until Genesis gossip orchestration consumes the handle.
    genesis_network: GenesisWithPubKey,
    block_count: BlockCount,
    block_sync_gossip_limit: usize,
    last_advertised_topology: BTreeSet<PeerId>,
    last_commit_topology_hash: Option<HashOf<Vec<PeerId>>>,
    last_committed_height: u64,
    #[cfg(feature = "telemetry")]
    telemetry: Telemetry,
    epoch_roster_provider: Option<Arc<WsvEpochRosterAdapter>>,
    background_post_tx: Option<mpsc::SyncSender<BackgroundPost>>,
    phase_tracker: PhaseTracker,
    phase_ema: PhaseEma,
    evidence_store: EvidenceStore,
    invalid_sig_log: InvalidSigThrottle,
    genesis_account: AccountId,
    vote_log: BTreeMap<
        (
            crate::sumeragi::consensus::Phase,
            u64,
            u64,
            u64,
            crate::sumeragi::consensus::ValidatorIndex,
        ),
        crate::sumeragi::consensus::Vote,
    >,
    exec_vote_log: BTreeMap<
        (u64, u64, u64, crate::sumeragi::consensus::ValidatorIndex),
        crate::sumeragi::consensus::ExecVote,
    >,
    qc_cache: BTreeMap<QcVoteKey, crate::sumeragi::consensus::Qc>,
    qc_signer_tally: BTreeMap<QcVoteKey, QcSignerTally>,
    execution_qc_cache: BTreeMap<HashOf<BlockHeader>, crate::sumeragi::consensus::ExecutionQC>,
    epoch_manager: Option<EpochManager>,
    npos_collectors: Option<NposCollectorConfig>,
    pending: PendingBlockState,
    voting_block: Option<VotingBlock>,
    pending_roster_activation: Option<(u64, Vec<PeerId>)>,
    /// Staged runtime consensus mode awaiting a live flip.
    pending_mode_flip: Option<ConsensusMode>,
    highest_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    locked_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    tick_counter: u64,
    tick_timing: TickTimingMonitor,
    last_qc_rebuild: Instant,
    payload_rebroadcast_log: PayloadRebroadcastThrottle,
    block_sync_rebroadcast_log: PayloadRebroadcastThrottle,
    block_sync_fetch_log: PayloadRebroadcastThrottle,
}

struct ActorSubsystems {
    commit: CommitState,
    propose: ProposeState,
    da_rbc: DaRbcState,
    vrf: VrfActor,
    merge: MergeLaneState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CacheOutcome {
    Hit,
    Miss,
}

impl CacheOutcome {
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Hit, Self::Hit) => Self::Hit,
            _ => Self::Miss,
        }
    }

    #[cfg(feature = "telemetry")]
    fn as_telemetry(self) -> crate::telemetry::CacheResult {
        match self {
            Self::Hit => crate::telemetry::CacheResult::Hit,
            Self::Miss => crate::telemetry::CacheResult::Miss,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SpoolDirStamp {
    fingerprint: [u8; 32],
    entries: usize,
    total_bytes: u64,
}

impl SpoolDirStamp {
    fn from_entries(entries: &[(String, u64, Option<SystemTime>)]) -> Self {
        let mut hasher = Blake3Hasher::new();
        let mut total_bytes = 0u64;
        for (name, len, modified) in entries {
            hasher.update(name.as_bytes());
            hasher.update(&len.to_le_bytes());
            total_bytes = total_bytes.saturating_add(*len);
            let stamp = modified
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .unwrap_or_default();
            hasher.update(&stamp.as_secs().to_le_bytes());
            hasher.update(&stamp.subsec_nanos().to_le_bytes());
        }
        let fingerprint = *hasher.finalize().as_bytes();
        Self {
            fingerprint,
            entries: entries.len(),
            total_bytes,
        }
    }
}

fn is_da_spool_file(name: &str) -> bool {
    (name.starts_with("da-commitment-")
        || name.starts_with("da-pin-intent-")
        || name.starts_with("da-receipt-"))
        && name.ends_with(".norito")
}

fn scan_da_spool_stamp(spool_dir: &Path) -> Result<Option<SpoolDirStamp>, std::io::Error> {
    let metadata = match fs::metadata(spool_dir) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "DA spool path is not a directory",
        ));
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(spool_dir)? {
        let entry = match entry {
            Ok(value) => value,
            Err(err) => {
                warn!(?err, "failed to read DA spool entry");
                continue;
            }
        };
        let name = match entry.file_name().to_str() {
            Some(value) => value.to_string(),
            None => continue,
        };
        if !is_da_spool_file(&name) {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(meta) => meta,
            Err(err) => {
                warn!(
                    ?err,
                    path = %entry.path().display(),
                    "failed to read DA spool metadata"
                );
                continue;
            }
        };
        entries.push((name, metadata.len(), metadata.modified().ok()));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Some(SpoolDirStamp::from_entries(&entries)))
}

#[derive(Debug, Default)]
struct DaSpoolCache {
    dir_stamp: Option<SpoolDirStamp>,
    commitment_bundle: Option<Option<DaCommitmentBundle>>,
    pin_bundle: Option<Option<DaPinIntentBundle>>,
    receipt_entries: Option<Vec<crate::da::receipts::DaReceiptEntry>>,
}

impl DaSpoolCache {
    fn invalidate(&mut self) {
        self.dir_stamp = None;
        self.commitment_bundle = None;
        self.pin_bundle = None;
        self.receipt_entries = None;
    }

    fn refresh_if_stale(&mut self, spool_dir: &Path) -> Result<bool, std::io::Error> {
        let stamp = match scan_da_spool_stamp(spool_dir)? {
            Some(stamp) => stamp,
            None => {
                if self.dir_stamp.is_some() {
                    self.invalidate();
                }
                return Ok(false);
            }
        };
        if self.dir_stamp != Some(stamp) {
            self.dir_stamp = Some(stamp);
            self.commitment_bundle = None;
            self.pin_bundle = None;
            self.receipt_entries = None;
        }
        Ok(true)
    }

    fn load_commitment_bundle(
        &mut self,
        spool_dir: &Path,
    ) -> Result<(Option<DaCommitmentBundle>, CacheOutcome), crate::da::commitments::DaSpoolError>
    {
        let exists = match self.refresh_if_stale(spool_dir) {
            Ok(exists) => exists,
            Err(source) => {
                return Err(crate::da::commitments::DaSpoolError::ReadDir {
                    path: spool_dir.to_path_buf(),
                    source,
                });
            }
        };

        if let Some(cached) = &self.commitment_bundle {
            return Ok((cached.clone(), CacheOutcome::Hit));
        }

        if !exists {
            self.commitment_bundle = Some(None);
            return Ok((None, CacheOutcome::Miss));
        }

        let bundle = crate::da::commitments::load_commitment_bundle(spool_dir)?;
        self.commitment_bundle = Some(bundle.clone());
        Ok((bundle, CacheOutcome::Miss))
    }

    fn load_pin_bundle(
        &mut self,
        spool_dir: &Path,
    ) -> Result<
        (Option<DaPinIntentBundle>, CacheOutcome),
        crate::da::pin_intents::DaPinIntentSpoolError,
    > {
        let exists = match self.refresh_if_stale(spool_dir) {
            Ok(exists) => exists,
            Err(source) => {
                return Err(crate::da::pin_intents::DaPinIntentSpoolError::ReadDir {
                    path: spool_dir.to_path_buf(),
                    source,
                });
            }
        };

        if let Some(cached) = &self.pin_bundle {
            return Ok((cached.clone(), CacheOutcome::Hit));
        }

        if !exists {
            self.pin_bundle = Some(None);
            return Ok((None, CacheOutcome::Miss));
        }

        let intents =
            crate::da::pin_intents::load_pin_intents(spool_dir)?.map(DaPinIntentBundle::new);
        self.pin_bundle = Some(intents.clone());
        Ok((intents, CacheOutcome::Miss))
    }

    fn load_receipt_entries(
        &mut self,
        spool_dir: &Path,
    ) -> Result<
        (Vec<crate::da::receipts::DaReceiptEntry>, CacheOutcome),
        crate::da::receipts::DaReceiptSpoolError,
    > {
        let exists = match self.refresh_if_stale(spool_dir) {
            Ok(exists) => exists,
            Err(source) => {
                return Err(crate::da::receipts::DaReceiptSpoolError::ReadDir {
                    path: spool_dir.to_path_buf(),
                    source,
                });
            }
        };

        if let Some(cached) = &self.receipt_entries {
            return Ok((cached.clone(), CacheOutcome::Hit));
        }

        if !exists {
            let empty = Vec::new();
            self.receipt_entries = Some(empty.clone());
            return Ok((empty, CacheOutcome::Miss));
        }

        let entries = crate::da::receipts::load_receipt_entries(spool_dir)?;
        self.receipt_entries = Some(entries.clone());
        Ok((entries, CacheOutcome::Miss))
    }
}

struct DaRbcState {
    da: DaState,
    rbc: RbcState,
    spool_dir: PathBuf,
    spool_cache: DaSpoolCache,
    manifest_cache: crate::sumeragi::main_loop::ManifestSpoolCache,
}

struct MergeLaneState {
    lane_relay: LaneRelayBroadcaster<IrohaNetwork>,
    committee: MergeCommitteeState,
}

#[derive(Debug)]
struct DaState {
    /// Per-block DA bundles sealed by this node (in-memory index).
    da_bundles: BTreeMap<u64, DaCommitmentBundle>,
    /// Per-block DA pin intent bundles sealed by this node (in-memory index).
    da_pin_bundles: BTreeMap<u64, DaPinIntentBundle>,
    /// Keys of commitments already sealed to avoid duplicates.
    sealed_commitments: BTreeSet<iroha_data_model::da::commitment::DaCommitmentKey>,
    /// Keys of pin intents already sealed to avoid duplicates.
    sealed_pin_intents: BTreeSet<(u32, u64, u64, iroha_data_model::da::types::StorageTicketId)>,
}

impl DaState {
    fn new() -> Self {
        Self {
            da_bundles: BTreeMap::new(),
            da_pin_bundles: BTreeMap::new(),
            sealed_commitments: BTreeSet::new(),
            sealed_pin_intents: BTreeSet::new(),
        }
    }
}

struct RbcState {
    store_cfg: Option<RbcStoreConfig>,
    chunk_store: Option<super::rbc_store::ChunkStore>,
    manifest: super::rbc_store::SoftwareManifest,
    sessions: BTreeMap<super::rbc_store::SessionKey, RbcSession>,
    pending: BTreeMap<super::rbc_store::SessionKey, PendingRbcMessages>,
    session_rosters: BTreeMap<super::rbc_store::SessionKey, Vec<PeerId>>,
    status_handle: rbc_status::Handle,
    payload_rebroadcast_last_sent: BTreeMap<super::rbc_store::SessionKey, Instant>,
    ready_rebroadcast_last_sent: BTreeMap<super::rbc_store::SessionKey, Instant>,
    persisted_full_sessions: BTreeSet<super::rbc_store::SessionKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MergeCommitteeKey {
    epoch_id: u64,
    view: u64,
    message_digest: Hash,
}

#[derive(Debug)]
struct MergeCommitteeEntry {
    candidate: Option<crate::merge::MergeLedgerCandidate>,
    signatures: BTreeMap<ValidatorIndex, Vec<u8>>,
}

#[derive(Debug)]
struct MergeCommitteeState {
    pending: BTreeMap<MergeCommitteeKey, MergeCommitteeEntry>,
}

impl MergeCommitteeState {
    fn new() -> Self {
        Self {
            pending: BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
struct PendingBlockState {
    pending_blocks: BTreeMap<HashOf<BlockHeader>, PendingBlock>,
    pending_replay_last_sent: BTreeMap<HashOf<BlockHeader>, Instant>,
    missing_block_requests: BTreeMap<HashOf<BlockHeader>, MissingBlockRequest>,
    pending_processing: Cell<Option<HashOf<BlockHeader>>>,
    pending_processing_parent: Cell<Option<HashOf<BlockHeader>>>,
    last_commit_pipeline_run: Instant,
}

struct CommitInFlight {
    id: u64,
    lock: crate::sumeragi::consensus::QcHeaderRef,
    block_hash: HashOf<BlockHeader>,
    pending: PendingBlock,
    commit_topology: Vec<PeerId>,
    signature_topology: Vec<PeerId>,
    qc_signers: Option<BTreeSet<ValidatorIndex>>,
    allow_quorum_bypass: bool,
    post_commit_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    enqueue_time: Instant,
}

struct CommitState {
    work_tx: Option<mpsc::SyncSender<commit::CommitWork>>,
    result_rx: Option<mpsc::Receiver<commit::CommitResult>>,
    inflight: Option<CommitInFlight>,
    next_id: u64,
}

impl CommitState {
    fn new() -> Self {
        Self {
            work_tx: None,
            result_rx: None,
            inflight: None,
            next_id: 0,
        }
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }
}

struct ProposeState {
    backpressure_gate: BackpressureGate,
    pacemaker: Pacemaker,
    forced_view_after_timeout: Option<(u64, u64)>,
    proposal_cache: ProposalCache,
    collector_plan: Option<CollectorPlan>,
    collector_plan_subject: Option<(u64, u64)>,
    collector_plan_targets: Vec<PeerId>,
    collectors_contacted: BTreeSet<PeerId>,
    collector_redundant_limit: u8,
    adaptive_cfg: AdaptiveObservability,
    adaptive_state: AdaptiveObservabilityState,
    collector_role_index: Option<ValidatorIndex>,
    new_view_tracker: NewViewTracker,
    proposals_seen: BTreeSet<(u64, u64)>,
    pacemaker_backpressure: PacemakerBackpressure,
    last_pacemaker_attempt: Option<Instant>,
    last_successful_proposal: Option<Instant>,
    propose_attempt_monitor: ProposeAttemptMonitor,
    /// Tracks the last payload hash used to emit an empty-child fallback along with the emission
    /// time so idle-slot retries can be rate-limited per parent block.
    last_empty_child_attempt: Option<(Hash, Instant)>,
}

#[allow(
    dead_code,
    clippy::large_types_passed_by_value,
    clippy::needless_pass_by_value,
    clippy::unused_self,
    clippy::unnecessary_wraps,
    clippy::assigning_clones
)]
impl Actor {
    fn commit_min_votes(&self, topology: &super::network_topology::Topology) -> usize {
        topology.min_votes_for_commit()
    }

    pub(super) fn attach_commit_worker(&mut self) -> std::thread::JoinHandle<()> {
        let commit_handle = commit::spawn_commit_worker(
            Arc::clone(&self.state),
            Arc::clone(&self.kura),
            self.common_config.chain.clone(),
            self.genesis_account.clone(),
        );
        self.subsystems.commit.work_tx = Some(commit_handle.work_tx);
        self.subsystems.commit.result_rx = Some(commit_handle.result_rx);
        commit_handle.join_handle
    }
}

fn sumeragi_da_enabled(state: &State) -> bool {
    let view = state.view();
    let da_enabled = view.world.parameters().sumeragi().da_enabled();
    drop(view);

    da_enabled
}

#[allow(clippy::too_many_arguments)] // Helper resets multiple subsystems; keep signature explicit.
fn reset_runtime_state_for_mode_flip(
    pacemaker: &mut Pacemaker,
    new_view_tracker: &mut NewViewTracker,
    phase_tracker: &mut PhaseTracker,
    propose_attempt_monitor: &mut ProposeAttemptMonitor,
    pacemaker_backpressure: &mut PacemakerBackpressure,
    forced_view_after_timeout: &mut Option<(u64, u64)>,
    last_pacemaker_attempt: &mut Option<Instant>,
    last_successful_proposal: &mut Option<Instant>,
    tick_counter: &mut u64,
    qc_signer_tally: &mut BTreeMap<QcVoteKey, QcSignerTally>,
    exec_vote_log: &mut BTreeMap<
        (u64, u64, u64, crate::sumeragi::consensus::ValidatorIndex),
        crate::sumeragi::consensus::ExecVote,
    >,
    voting_block: &mut Option<VotingBlock>,
    pending_roster_activation: &mut Option<(u64, Vec<PeerId>)>,
    last_empty_child_attempt: &mut Option<(Hash, Instant)>,
    rbc_status_handle: &rbc_status::Handle,
    vrf: &mut VrfActor,
    base_pacemaker_interval: Duration,
    now: Instant,
) {
    pacemaker.set_interval(base_pacemaker_interval, now);
    *new_view_tracker = NewViewTracker::default();
    *phase_tracker = PhaseTracker::new(now);
    *propose_attempt_monitor = ProposeAttemptMonitor::new();
    *pacemaker_backpressure = PacemakerBackpressure::new();
    *forced_view_after_timeout = None;
    *last_pacemaker_attempt = None;
    *last_successful_proposal = None;
    *tick_counter = 0;
    qc_signer_tally.clear();
    exec_vote_log.clear();
    *voting_block = None;
    *pending_roster_activation = None;
    *last_empty_child_attempt = None;
    rbc_status_handle.clear();
    vrf.reset();
}

/// Update the pending mode flip tracker based on the currently effective consensus mode.
///
/// Returns `Some(target_mode)` when a new pending flip is recorded; clears `pending` when the
/// effective mode matches the current mode.
fn update_pending_mode_flip(
    current: ConsensusMode,
    effective: ConsensusMode,
    pending: &mut Option<ConsensusMode>,
) -> Option<ConsensusMode> {
    if effective == current {
        *pending = None;
        return None;
    }
    if *pending == Some(effective) {
        return None;
    }
    *pending = Some(effective);
    Some(effective)
}

fn staged_mode_info(
    params: &iroha_data_model::parameter::system::SumeragiParameters,
) -> (Option<&'static str>, Option<u64>) {
    let staged_tag = params.next_mode.map(|mode| match mode {
        iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned => {
            super::consensus::PERMISSIONED_TAG
        }
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos => {
            super::consensus::NPOS_TAG
        }
    });
    (staged_tag, params.mode_activation_height)
}

fn mode_activation_lag(
    current_height: u64,
    activation_height: Option<u64>,
    effective_mode: ConsensusMode,
    current_mode: ConsensusMode,
) -> Option<u64> {
    activation_height.and_then(|activation| {
        (current_height >= activation && effective_mode != current_mode)
            .then_some(current_height.saturating_sub(activation) + 1)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockSyncRosterSource {
    CommitCertificateHint,
    CommitCheckpointPairHint,
    ValidatorCheckpointHint,
    CommitCertificateHistory,
    ValidatorCheckpointHistory,
    RosterSidecar,
    CommitRosterJournal,
    CommitTopologySnapshot,
    TrustedPeersFallback,
}

impl BlockSyncRosterSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CommitCertificateHint => "commit_certificate_hint",
            Self::CommitCheckpointPairHint => "commit_checkpoint_pair_hint",
            Self::ValidatorCheckpointHint => "validator_checkpoint_hint",
            Self::CommitCertificateHistory => "commit_certificate_history",
            Self::ValidatorCheckpointHistory => "validator_checkpoint_history",
            Self::RosterSidecar => "roster_sidecar",
            Self::CommitRosterJournal => "commit_roster_journal",
            Self::CommitTopologySnapshot => "commit_topology_snapshot",
            Self::TrustedPeersFallback => "trusted_peers_fallback",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BlockSyncRosterSelection {
    roster: Vec<PeerId>,
    source: BlockSyncRosterSource,
    commit_certificate: Option<CommitCertificate>,
    checkpoint: Option<ValidatorSetCheckpoint>,
    stake_snapshot: Option<CommitStakeSnapshot>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RosterValidationError {
    BlockHashMismatch,
    HeightMismatch {
        expected: u64,
        actual: u64,
    },
    ViewMismatch {
        expected: u64,
        actual: u64,
    },
    EmptyRoster,
    ValidatorSetHashVersionMismatch {
        expected: u16,
        actual: u16,
    },
    ValidatorSetHashMismatch,
    SignerOutOfRange {
        signer: u32,
        roster_len: u32,
    },
    SignerBitmapLengthMismatch {
        expected: usize,
        actual: usize,
    },
    DuplicateSigner(u32),
    AggregateSignatureMissing,
    AggregateSignatureInvalid,
    CommitQuorumMissing {
        votes: usize,
        required: usize,
    },
    StakeSnapshotUnavailable,
    StakeQuorumMissing,
    CheckpointExpired {
        block_height: u64,
        expires_at_height: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StakeQuorumError {
    MissingStake,
    SignerOutOfRoster,
    Overflow,
    ZeroTotal,
    SnapshotMismatch,
}

fn apply_roster_selection_to_block_sync_update(
    update: &mut super::message::BlockSyncUpdate,
    selection: &BlockSyncRosterSelection,
) {
    update
        .commit_certificate
        .clone_from(&selection.commit_certificate);
    update
        .validator_checkpoint
        .clone_from(&selection.checkpoint);
    update.stake_snapshot.clone_from(&selection.stake_snapshot);
}

fn stake_quorum_reached_for_peers(
    view: &StateView<'_>,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
) -> Result<bool, StakeQuorumError> {
    let mut stake_map = stake_map_from_world(view.world());
    if stake_map.is_empty() {
        for peer in roster {
            stake_map.insert(peer.clone(), Numeric::from(1_u64));
        }
    }

    let roster_set: BTreeSet<_> = roster.iter().cloned().collect();
    let mut total = Numeric::from(0_u64);
    for peer in roster {
        let Some(stake) = stake_map.get(peer) else {
            return Err(StakeQuorumError::MissingStake);
        };
        total = total
            .checked_add(stake.clone())
            .ok_or(StakeQuorumError::Overflow)?;
    }
    if total.is_zero() {
        return Err(StakeQuorumError::ZeroTotal);
    }

    let mut signed = Numeric::from(0_u64);
    for peer in signers {
        if !roster_set.contains(peer) {
            return Err(StakeQuorumError::SignerOutOfRoster);
        }
        let Some(stake) = stake_map.get(peer) else {
            return Err(StakeQuorumError::MissingStake);
        };
        signed = signed
            .checked_add(stake.clone())
            .ok_or(StakeQuorumError::Overflow)?;
    }

    let signed_scaled = signed
        .checked_mul(Numeric::from(3_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    let total_scaled = total
        .checked_mul(Numeric::from(2_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    Ok(signed_scaled >= total_scaled)
}

fn stake_quorum_reached_for_snapshot(
    snapshot: &CommitStakeSnapshot,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
) -> Result<bool, StakeQuorumError> {
    if !snapshot.matches_roster(roster) {
        return Err(StakeQuorumError::SnapshotMismatch);
    }
    let mut stake_map: BTreeMap<PeerId, Numeric> = BTreeMap::new();
    for entry in &snapshot.entries {
        let entry_stake = stake_map
            .entry(entry.peer_id.clone())
            .or_insert_with(|| entry.stake.clone());
        if entry.stake > *entry_stake {
            *entry_stake = entry.stake.clone();
        }
    }

    let roster_set: BTreeSet<_> = roster.iter().cloned().collect();
    let mut total = Numeric::from(0_u64);
    for peer in roster {
        let Some(stake) = stake_map.get(peer) else {
            return Err(StakeQuorumError::MissingStake);
        };
        total = total
            .checked_add(stake.clone())
            .ok_or(StakeQuorumError::Overflow)?;
    }
    if total.is_zero() {
        return Err(StakeQuorumError::ZeroTotal);
    }

    let mut signed = Numeric::from(0_u64);
    for peer in signers {
        if !roster_set.contains(peer) {
            return Err(StakeQuorumError::SignerOutOfRoster);
        }
        let Some(stake) = stake_map.get(peer) else {
            return Err(StakeQuorumError::MissingStake);
        };
        signed = signed
            .checked_add(stake.clone())
            .ok_or(StakeQuorumError::Overflow)?;
    }

    let signed_scaled = signed
        .checked_mul(Numeric::from(3_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    let total_scaled = total
        .checked_mul(Numeric::from(2_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    Ok(signed_scaled >= total_scaled)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewChangeCause {
    CommitFailure,
    QuorumTimeout,
    CensorshipEvidence,
    MissingPayload,
    MissingQc,
    ValidationReject,
}

impl ViewChangeCause {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CommitFailure => "commit_failure",
            Self::QuorumTimeout => "quorum_timeout",
            Self::CensorshipEvidence => "censorship_evidence",
            Self::MissingPayload => "missing_payload",
            Self::MissingQc => "missing_qc",
            Self::ValidationReject => "validation_reject",
        }
    }
}

fn view_change_cause_for_quorum(vote_count: usize) -> ViewChangeCause {
    if vote_count == 0 {
        ViewChangeCause::MissingQc
    } else {
        ViewChangeCause::QuorumTimeout
    }
}

#[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
fn record_view_change_cause_with_telemetry(
    cause: ViewChangeCause,
    telemetry: Option<&crate::telemetry::Telemetry>,
) {
    super::status::record_view_change_cause(cause.as_str());
    #[cfg(feature = "telemetry")]
    if let Some(telemetry) = telemetry {
        telemetry.note_view_change_cause(cause.as_str());
    }
}

fn pending_replay_due(last_sent: Option<Instant>, now: Instant, cooldown: Duration) -> bool {
    last_sent.is_none_or(|sent| now.saturating_duration_since(sent) >= cooldown)
}

#[cfg(test)]
fn signature_topology_for_roster(
    roster: &[PeerId],
    height: u64,
    view: u64,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> super::network_topology::Topology {
    let mut topology = super::network_topology::Topology::new(roster.to_vec());
    match mode_tag {
        PERMISSIONED_TAG => {
            if let Ok(view_usize) = usize::try_from(view) {
                topology.nth_rotation(view_usize);
            } else {
                warn!(
                    view,
                    "skipping topology rotation for roster signatures: view exceeds usize"
                );
            }
        }
        NPOS_TAG => {
            if let Some(seed) = prf_seed {
                let leader = topology.leader_index_prf(seed, height, view);
                topology.rotate_preserve_view_to_front(leader);
            } else {
                warn!(
                    height,
                    view, "missing PRF seed for NPoS roster signature alignment"
                );
            }
        }
        _ => {}
    }
    topology
}

#[cfg(test)]
fn canonicalize_block_signatures_for_roster(
    block: &SignedBlock,
    roster: &[PeerId],
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Vec<iroha_data_model::block::BlockSignature> {
    if roster.is_empty() {
        return Vec::new();
    }
    let height = block.header().height().get();
    let view = u64::from(block.header().view_change_index());
    let signature_topology =
        signature_topology_for_roster(roster, height, view, mode_tag, prf_seed);
    let canonical_topology = super::network_topology::Topology::new(roster.to_vec());
    let mut mapped = BTreeSet::new();
    let mut invalid = 0usize;
    for sig in block.signatures() {
        let Ok(idx) = usize::try_from(sig.index()) else {
            invalid = invalid.saturating_add(1);
            continue;
        };
        let Some(peer) = signature_topology.as_ref().get(idx) else {
            invalid = invalid.saturating_add(1);
            continue;
        };
        let Some(canonical_idx) = canonical_topology.position(peer.public_key()) else {
            invalid = invalid.saturating_add(1);
            continue;
        };
        mapped.insert(iroha_data_model::block::BlockSignature::new(
            canonical_idx as u64,
            sig.signature().clone(),
        ));
    }
    if invalid > 0 {
        debug!(
            height,
            view,
            invalid,
            roster_len = roster.len(),
            "skipped invalid block signature indices while canonicalizing roster"
        );
    }
    mapped.into_iter().collect()
}

#[allow(clippy::too_many_arguments)]
fn selection_from_roster_artifacts(
    commit_certificate: Option<&CommitCertificate>,
    checkpoint: Option<&ValidatorSetCheckpoint>,
    stake_snapshot: Option<&CommitStakeSnapshot>,
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    block_view: Option<u64>,
    source: BlockSyncRosterSource,
    consensus_mode: ConsensusMode,
    state: &State,
    mode_tag: &'static str,
) -> Option<BlockSyncRosterSelection> {
    let epoch = epoch_for_height_from_state(state, block_height, consensus_mode);
    let validated_cert =
        commit_certificate.and_then(|cert| {
            match validate_commit_certificate_roster(
                cert,
                block_hash,
                block_height,
                block_view,
                consensus_mode,
                stake_snapshot,
            ) {
                Ok(roster) => Some((roster, cert)),
                Err(err) => {
                    warn!(
                        ?err,
                        block = %block_hash,
                        "rejecting commit certificate roster hint"
                    );
                    None
                }
            }
        });
    let validated_checkpoint = checkpoint.and_then(|chk| {
        match validate_checkpoint_roster(
            chk,
            block_hash,
            block_height,
            block_view,
            consensus_mode,
            stake_snapshot,
            &state.chain_id,
            mode_tag,
            epoch,
        ) {
            Ok(roster) => Some((roster, chk)),
            Err(err) => {
                warn!(
                    ?err,
                    block = %block_hash,
                    "rejecting checkpoint roster hint"
                );
                None
            }
        }
    });
    match (validated_cert, validated_checkpoint) {
        (Some((roster, cert)), Some((checkpoint_roster, chk))) => {
            if roster != checkpoint_roster {
                warn!(
                    cert_roster = roster.len(),
                    checkpoint_roster = checkpoint_roster.len(),
                    "commit certificate and checkpoint rosters differ; preferring commit certificate"
                );
            }
            let stake_snapshot = stake_snapshot
                .filter(|snapshot| snapshot.matches_roster(&roster))
                .cloned();
            Some(BlockSyncRosterSelection {
                roster,
                source,
                commit_certificate: Some(cert.clone()),
                checkpoint: Some(chk.clone()),
                stake_snapshot,
            })
        }
        (Some((roster, cert)), None) => {
            let stake_snapshot = stake_snapshot
                .filter(|snapshot| snapshot.matches_roster(&roster))
                .cloned();
            Some(BlockSyncRosterSelection {
                roster,
                source,
                commit_certificate: Some(cert.clone()),
                checkpoint: None,
                stake_snapshot,
            })
        }
        (None, Some((roster, chk))) => {
            let stake_snapshot = stake_snapshot
                .filter(|snapshot| snapshot.matches_roster(&roster))
                .cloned();
            Some(BlockSyncRosterSelection {
                roster,
                source,
                commit_certificate: None,
                checkpoint: Some(chk.clone()),
                stake_snapshot,
            })
        }
        (None, None) => None,
    }
}

fn epoch_for_height_from_state(state: &State, height: u64, consensus_mode: ConsensusMode) -> u64 {
    if matches!(consensus_mode, ConsensusMode::Permissioned) {
        return 0;
    }
    if height == 0 {
        return 0;
    }
    let view = state.view();
    let epoch_length = view
        .world()
        .sumeragi_npos_parameters()
        .map_or(
            iroha_config::parameters::defaults::sumeragi::EPOCH_LENGTH_BLOCKS,
            |params| params.epoch_length_blocks,
        )
        .max(1);
    height.saturating_sub(1) / epoch_length
}

fn block_sync_history_roster_for_block(
    state: &State,
    consensus_mode: ConsensusMode,
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    block_view: Option<u64>,
) -> Option<BlockSyncRosterSelection> {
    let cert = super::status::commit_certificate_history()
        .into_iter()
        .filter(|cert| cert.subject_block_hash == block_hash && cert.height <= block_height)
        .max_by(|a, b| a.height.cmp(&b.height).then_with(|| a.view.cmp(&b.view)));
    let checkpoint = super::status::validator_checkpoint_history()
        .into_iter()
        .filter(|chk| chk.block_hash == block_hash && chk.height <= block_height)
        .max_by(|a, b| a.height.cmp(&b.height));

    if cert.is_none() && checkpoint.is_none() {
        return None;
    }

    let source = if cert.is_some() {
        BlockSyncRosterSource::CommitCertificateHistory
    } else {
        BlockSyncRosterSource::ValidatorCheckpointHistory
    };
    let mode_tag = match consensus_mode {
        ConsensusMode::Permissioned => PERMISSIONED_TAG,
        ConsensusMode::Npos => NPOS_TAG,
    };
    let mut roster_height = block_height;
    let mut roster_view = block_view;
    let mut checkpoint = checkpoint.as_ref();
    if let Some(cert) = cert.as_ref() {
        if cert.height != block_height {
            roster_height = cert.height;
            roster_view = Some(cert.view);
            checkpoint = checkpoint.filter(|chk| chk.height == cert.height);
        }
    } else if let Some(chk) = checkpoint {
        if chk.height != block_height {
            roster_height = chk.height;
        }
    }
    selection_from_roster_artifacts(
        cert.as_ref(),
        checkpoint,
        None,
        block_hash,
        roster_height,
        roster_view,
        source,
        consensus_mode,
        state,
        mode_tag,
    )
}

fn persisted_roster_for_block(
    state: &State,
    kura: &Kura,
    consensus_mode: ConsensusMode,
    block_height: u64,
    block_hash: HashOf<BlockHeader>,
    block_view: Option<u64>,
) -> Option<BlockSyncRosterSelection> {
    let mode_tag = match consensus_mode {
        ConsensusMode::Permissioned => PERMISSIONED_TAG,
        ConsensusMode::Npos => NPOS_TAG,
    };
    if let Some(snapshot) = state.commit_roster_snapshot_for_block(block_height, block_hash) {
        if let Some(selection) = selection_from_roster_artifacts(
            Some(&snapshot.commit_certificate),
            Some(&snapshot.validator_checkpoint),
            snapshot.stake_snapshot.as_ref(),
            block_hash,
            block_height,
            block_view,
            BlockSyncRosterSource::CommitRosterJournal,
            consensus_mode,
            state,
            mode_tag,
        ) {
            if let Some(cert) = selection.commit_certificate.as_ref() {
                status::record_commit_certificate(cert.clone());
            }
            if let Some(checkpoint) = selection.checkpoint.as_ref() {
                status::record_validator_checkpoint(checkpoint.clone());
            }
            return Some(selection);
        }
        warn!(
            height = block_height,
            block = %block_hash,
            "persisted commit roster snapshot failed validation"
        );
    }

    if let Some(meta) = kura.read_roster_metadata(block_height).and_then(|meta| {
        if meta.block_hash == block_hash {
            Some(meta)
        } else {
            warn!(
                expected = %block_hash,
                stored = %meta.block_hash,
                height = block_height,
                "ignoring roster sidecar with mismatched hash"
            );
            None
        }
    }) {
        if let Some(selection) = selection_from_roster_artifacts(
            meta.commit_certificate.as_ref(),
            meta.validator_checkpoint.as_ref(),
            meta.stake_snapshot.as_ref(),
            block_hash,
            block_height,
            block_view,
            BlockSyncRosterSource::RosterSidecar,
            consensus_mode,
            state,
            mode_tag,
        ) {
            if let Some(cert) = selection.commit_certificate.as_ref() {
                status::record_commit_certificate(cert.clone());
            }
            if let Some(checkpoint) = selection.checkpoint.as_ref() {
                status::record_validator_checkpoint(checkpoint.clone());
            }
            return Some(selection);
        }
        warn!(
            height = block_height,
            block = %block_hash,
            "roster sidecar present but failed validation"
        );
    }
    None
}

fn block_sync_update_with_roster(
    block: &SignedBlock,
    state: &State,
    kura: &Kura,
    fallback_consensus_mode: ConsensusMode,
    trusted: &iroha_config::parameters::actual::TrustedPeers,
    me: &PeerId,
) -> super::message::BlockSyncUpdate {
    let block_hash = block.hash();
    let block_height = block.header().height().get();
    let block_view = u64::from(block.header().view_change_index());
    let mut update = super::message::BlockSyncUpdate::from(block);
    let (consensus_mode, _mode_tag, _prf_seed) = {
        let state_view = state.view();
        let consensus_mode = super::effective_consensus_mode_for_height(
            &state_view,
            block_height,
            fallback_consensus_mode,
        );
        let mode_tag = match consensus_mode {
            ConsensusMode::Permissioned => PERMISSIONED_TAG,
            ConsensusMode::Npos => NPOS_TAG,
        };
        let prf_seed = matches!(consensus_mode, ConsensusMode::Npos)
            .then(|| super::npos_seed_for_height(&state_view, block_height));
        (consensus_mode, mode_tag, prf_seed)
    };
    let selection = persisted_roster_for_block(
        state,
        kura,
        consensus_mode,
        block_height,
        block_hash,
        Some(block_view),
    )
    .or_else(|| {
        block_sync_history_roster_for_block(
            state,
            consensus_mode,
            block_hash,
            block_height,
            Some(block_view),
        )
    })
    .or_else(|| {
        let view = state.view();
        // Use the active roster so checkpoint signature indices match consensus ordering.
        let roster = derive_active_topology(&view, trusted, me);
        let source = if view.commit_topology().is_empty() {
            BlockSyncRosterSource::TrustedPeersFallback
        } else {
            BlockSyncRosterSource::CommitTopologySnapshot
        };
        drop(view);
        if roster.is_empty() {
            None
        } else {
            Some(BlockSyncRosterSelection {
                roster: roster.clone(),
                source,
                commit_certificate: None,
                checkpoint: None,
                stake_snapshot: None,
            })
        }
    });
    if let Some(selection) = selection {
        apply_roster_selection_to_block_sync_update(&mut update, &selection);
    }
    update
}

fn validate_commit_certificate_roster(
    cert: &CommitCertificate,
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    block_view: Option<u64>,
    consensus_mode: ConsensusMode,
    stake_snapshot: Option<&CommitStakeSnapshot>,
) -> Result<Vec<PeerId>, RosterValidationError> {
    if cert.subject_block_hash != block_hash {
        return Err(RosterValidationError::BlockHashMismatch);
    }
    if cert.height != block_height {
        return Err(RosterValidationError::HeightMismatch {
            expected: block_height,
            actual: cert.height,
        });
    }
    if let Some(block_view) = block_view {
        if cert.view != block_view {
            return Err(RosterValidationError::ViewMismatch {
                expected: block_view,
                actual: cert.view,
            });
        }
    }
    if cert.validator_set.is_empty() {
        return Err(RosterValidationError::EmptyRoster);
    }
    if cert.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1 {
        return Err(RosterValidationError::ValidatorSetHashVersionMismatch {
            expected: VALIDATOR_SET_HASH_VERSION_V1,
            actual: cert.validator_set_hash_version,
        });
    }
    if HashOf::new(&cert.validator_set) != cert.validator_set_hash {
        return Err(RosterValidationError::ValidatorSetHashMismatch);
    }
    let roster_len = cert.validator_set.len();
    let expected_bitmap_len = roster_len.div_ceil(8);
    if cert.aggregate.signers_bitmap.len() != expected_bitmap_len {
        return Err(RosterValidationError::SignerBitmapLengthMismatch {
            expected: expected_bitmap_len,
            actual: cert.aggregate.signers_bitmap.len(),
        });
    }
    let mut signer_peers = BTreeSet::new();
    for (byte_idx, byte) in cert.aggregate.signers_bitmap.iter().enumerate() {
        if *byte == 0 {
            continue;
        }
        for bit in 0..8 {
            if (byte >> bit) & 1 == 0 {
                continue;
            }
            let idx = byte_idx * 8 + bit;
            let idx_u32 = u32::try_from(idx).unwrap_or(u32::MAX);
            if idx >= roster_len {
                return Err(RosterValidationError::SignerOutOfRange {
                    signer: idx_u32,
                    roster_len: roster_len.try_into().unwrap_or(u32::MAX),
                });
            }
            let validator = cert.validator_set.get(idx).ok_or_else(|| {
                RosterValidationError::SignerOutOfRange {
                    signer: idx_u32,
                    roster_len: roster_len.try_into().unwrap_or(u32::MAX),
                }
            })?;
            signer_peers.insert(validator.clone());
        }
    }
    match consensus_mode {
        ConsensusMode::Permissioned => {
            let required = super::network_topology::commit_quorum_from_len(roster_len).max(1);
            if signer_peers.len() < required {
                return Err(RosterValidationError::CommitQuorumMissing {
                    votes: signer_peers.len(),
                    required,
                });
            }
        }
        ConsensusMode::Npos => {
            let snapshot = stake_snapshot.ok_or(RosterValidationError::StakeSnapshotUnavailable)?;
            match stake_quorum_reached_for_snapshot(snapshot, &cert.validator_set, &signer_peers) {
                Ok(true) => {}
                Ok(false) => return Err(RosterValidationError::StakeQuorumMissing),
                Err(_) => return Err(RosterValidationError::StakeSnapshotUnavailable),
            }
        }
    }
    Ok(cert.validator_set.clone())
}

fn validate_checkpoint_roster(
    checkpoint: &ValidatorSetCheckpoint,
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    block_view: Option<u64>,
    consensus_mode: ConsensusMode,
    stake_snapshot: Option<&CommitStakeSnapshot>,
    chain_id: &ChainId,
    mode_tag: &str,
    epoch: u64,
) -> Result<Vec<PeerId>, RosterValidationError> {
    if checkpoint.block_hash != block_hash {
        return Err(RosterValidationError::BlockHashMismatch);
    }
    if checkpoint.height != block_height {
        return Err(RosterValidationError::HeightMismatch {
            expected: block_height,
            actual: checkpoint.height,
        });
    }
    if checkpoint.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1 {
        return Err(RosterValidationError::ValidatorSetHashVersionMismatch {
            expected: VALIDATOR_SET_HASH_VERSION_V1,
            actual: checkpoint.validator_set_hash_version,
        });
    }
    if let Some(expires_at_height) = checkpoint.expires_at_height {
        if block_height >= expires_at_height {
            return Err(RosterValidationError::CheckpointExpired {
                block_height,
                expires_at_height,
            });
        }
    }
    if checkpoint.validator_set.is_empty() {
        return Err(RosterValidationError::EmptyRoster);
    }
    if HashOf::new(&checkpoint.validator_set) != checkpoint.validator_set_hash {
        return Err(RosterValidationError::ValidatorSetHashMismatch);
    }
    let roster_len = checkpoint.validator_set.len();
    let expected_bitmap_len = roster_len.div_ceil(8);
    if checkpoint.signers_bitmap.len() != expected_bitmap_len {
        return Err(RosterValidationError::SignerBitmapLengthMismatch {
            expected: expected_bitmap_len,
            actual: checkpoint.signers_bitmap.len(),
        });
    }
    if checkpoint.bls_aggregate_signature.is_empty() {
        return Err(RosterValidationError::AggregateSignatureMissing);
    }
    let mut signer_indices = BTreeSet::new();
    let mut signer_peers = BTreeSet::new();
    for (byte_idx, byte) in checkpoint.signers_bitmap.iter().enumerate() {
        if *byte == 0 {
            continue;
        }
        for bit in 0..8 {
            if (byte >> bit) & 1 == 0 {
                continue;
            }
            let idx = byte_idx * 8 + bit;
            let idx_u32 = u32::try_from(idx).unwrap_or(u32::MAX);
            if idx >= roster_len {
                return Err(RosterValidationError::SignerOutOfRange {
                    signer: idx_u32,
                    roster_len: roster_len.try_into().unwrap_or(u32::MAX),
                });
            }
            if !signer_indices.insert(idx) {
                return Err(RosterValidationError::DuplicateSigner(idx_u32));
            }
            let validator = checkpoint.validator_set.get(idx).ok_or_else(|| {
                RosterValidationError::SignerOutOfRange {
                    signer: idx_u32,
                    roster_len: roster_len.try_into().unwrap_or(u32::MAX),
                }
            })?;
            signer_peers.insert(validator.clone());
        }
    }
    match consensus_mode {
        ConsensusMode::Permissioned => {
            let required = super::network_topology::commit_quorum_from_len(roster_len).max(1);
            if signer_indices.len() < required {
                return Err(RosterValidationError::CommitQuorumMissing {
                    votes: signer_indices.len(),
                    required,
                });
            }
        }
        ConsensusMode::Npos => {
            let snapshot = stake_snapshot.ok_or(RosterValidationError::StakeSnapshotUnavailable)?;
            match stake_quorum_reached_for_snapshot(
                snapshot,
                &checkpoint.validator_set,
                &signer_peers,
            ) {
                Ok(true) => {}
                Ok(false) => return Err(RosterValidationError::StakeQuorumMissing),
                Err(_) => return Err(RosterValidationError::StakeSnapshotUnavailable),
            }
        }
    }
    let view = block_view.unwrap_or(0);
    let vote = crate::sumeragi::consensus::Vote {
        phase: crate::sumeragi::consensus::Phase::Commit,
        block_hash,
        height: block_height,
        view,
        epoch,
        highest_cert: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    let preimage = vote_preimage(chain_id, mode_tag, &vote);
    let mut public_keys: Vec<&[u8]> = Vec::with_capacity(signer_indices.len());
    for idx in &signer_indices {
        let Some(peer) = checkpoint.validator_set.get(*idx) else {
            return Err(RosterValidationError::SignerOutOfRange {
                signer: u32::try_from(*idx).unwrap_or(u32::MAX),
                roster_len: roster_len.try_into().unwrap_or(u32::MAX),
            });
        };
        let (_, payload) = peer.public_key().to_bytes();
        public_keys.push(payload);
    }
    if iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &checkpoint.bls_aggregate_signature,
        &public_keys,
    )
    .is_err()
    {
        return Err(RosterValidationError::AggregateSignatureInvalid);
    }
    Ok(checkpoint.validator_set.clone())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn select_block_sync_roster(
    block: &SignedBlock,
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    persisted: Option<BlockSyncRosterSelection>,
    cert_hint: Option<&CommitCertificate>,
    checkpoint_hint: Option<&ValidatorSetCheckpoint>,
    stake_snapshot_hint: Option<&CommitStakeSnapshot>,
    state: &State,
    trusted: &iroha_config::parameters::actual::TrustedPeers,
    me: &PeerId,
    consensus_mode: ConsensusMode,
    mode_tag: &'static str,
    allow_uncertified: bool,
) -> Option<BlockSyncRosterSelection> {
    let block_view = u64::from(block.header().view_change_index());
    if let Some(selection) = persisted {
        return Some(selection);
    }

    if let Some(snapshot) = state.commit_roster_snapshot_for_block(block_height, block_hash) {
        if let Some(selection) = selection_from_roster_artifacts(
            Some(&snapshot.commit_certificate),
            Some(&snapshot.validator_checkpoint),
            snapshot.stake_snapshot.as_ref(),
            block_hash,
            block_height,
            Some(block_view),
            BlockSyncRosterSource::CommitRosterJournal,
            consensus_mode,
            state,
            mode_tag,
        ) {
            return Some(selection);
        }
        warn!(
            height = block_height,
            block = %block_hash,
            "commit roster journal present but artifacts failed validation"
        );
    }

    if let (Some(cert_hint), Some(checkpoint_hint)) = (cert_hint, checkpoint_hint) {
        if let Some(selection) = selection_from_roster_artifacts(
            Some(cert_hint),
            Some(checkpoint_hint),
            stake_snapshot_hint,
            block_hash,
            block_height,
            Some(block_view),
            BlockSyncRosterSource::CommitCheckpointPairHint,
            consensus_mode,
            state,
            mode_tag,
        ) {
            return Some(selection);
        }
    }

    if let Some(cert_hint) = cert_hint {
        if let Some(selection) = selection_from_roster_artifacts(
            Some(cert_hint),
            checkpoint_hint,
            stake_snapshot_hint,
            block_hash,
            block_height,
            Some(block_view),
            BlockSyncRosterSource::CommitCertificateHint,
            consensus_mode,
            state,
            mode_tag,
        ) {
            return Some(selection);
        }
    } else if let Some(checkpoint_hint) = checkpoint_hint {
        if let Some(selection) = selection_from_roster_artifacts(
            None,
            Some(checkpoint_hint),
            stake_snapshot_hint,
            block_hash,
            block_height,
            Some(block_view),
            BlockSyncRosterSource::ValidatorCheckpointHint,
            consensus_mode,
            state,
            mode_tag,
        ) {
            return Some(selection);
        }
    }

    if let Some(history) = block_sync_history_roster_for_block(
        state,
        consensus_mode,
        block_hash,
        block_height,
        Some(block_view),
    ) {
        return Some(history);
    }

    if allow_uncertified {
        let view = state.view();
        let roster = derive_active_topology(&view, trusted, me);
        let source = if view.commit_topology().is_empty() {
            BlockSyncRosterSource::TrustedPeersFallback
        } else {
            BlockSyncRosterSource::CommitTopologySnapshot
        };
        drop(view);

        if !roster.is_empty() {
            return Some(BlockSyncRosterSelection {
                roster,
                source,
                commit_certificate: None,
                checkpoint: None,
                stake_snapshot: None,
            });
        }
    }

    None
}

fn block_sync_ready_for_qc(block_known: bool, creation_result: &Result<()>) -> bool {
    creation_result.is_ok() && block_known
}

fn block_sync_apply_qc_after_block<T, F>(
    creation_result: Result<()>,
    block_known_after_creation: bool,
    qc_to_apply: Option<T>,
    mut apply_qc: F,
) -> Result<()>
where
    F: FnMut(T) -> Result<()>,
{
    match creation_result {
        Ok(()) if block_known_after_creation => {
            if let Some(qc) = qc_to_apply {
                apply_qc(qc)?;
            }
            Ok(())
        }
        Ok(()) => Ok(()),
        Err(err) => Err(err),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TopologyRefreshDecision {
    NoPeers,
    Unchanged,
    AdvertiseChanged,
    AdvertiseForStrays { stray_count: usize },
}

fn topology_refresh_decision(
    current: &BTreeSet<PeerId>,
    last_advertised: &BTreeSet<PeerId>,
    online_outside_topology: &[PeerId],
) -> TopologyRefreshDecision {
    if current.is_empty() {
        return TopologyRefreshDecision::NoPeers;
    }
    if current == last_advertised {
        if online_outside_topology.is_empty() {
            TopologyRefreshDecision::Unchanged
        } else {
            TopologyRefreshDecision::AdvertiseForStrays {
                stray_count: online_outside_topology.len(),
            }
        }
    } else {
        TopologyRefreshDecision::AdvertiseChanged
    }
}

#[allow(
    dead_code,
    clippy::large_types_passed_by_value,
    clippy::needless_pass_by_value,
    clippy::unused_self,
    clippy::unnecessary_wraps,
    clippy::assigning_clones
)]
impl Actor {
    fn synthesize_commit_certificate(
        state: &State,
        block: &SignedBlock,
        roster: &[PeerId],
        mode_tag: &str,
        epoch: u64,
        consensus_mode: ConsensusMode,
    ) -> Option<(CommitCertificate, Option<CommitStakeSnapshot>)> {
        if roster.is_empty() {
            return None;
        }
        if roster.iter().any(|peer| !roster_member_allowed_bls(peer)) {
            return None;
        }
        let height = block.header().height().get();
        let view = u64::from(block.header().view_change_index());
        let cert = status::commit_certificate_history()
            .into_iter()
            .find(|candidate| {
                candidate.height == height
                    && candidate.subject_block_hash == block.hash()
                    && matches!(candidate.phase, crate::sumeragi::consensus::Phase::Commit)
            })
            .or_else(|| {
                let record = status::precommit_signers_for(block.hash())?;
                if record.height != height || record.view != view || record.epoch != epoch {
                    return None;
                }
                if record.mode_tag != mode_tag {
                    return None;
                }
                if record.validator_set.as_slice() != roster {
                    return None;
                }
                derive_block_sync_qc_from_signers(
                    block.hash(),
                    height,
                    view,
                    epoch,
                    roster,
                    mode_tag,
                    &record.signers,
                    record.bls_aggregate_signature,
                )
            })?;
        if cert.mode_tag != mode_tag {
            return None;
        }
        if cert.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1 {
            return None;
        }
        if HashOf::new(&cert.validator_set) != cert.validator_set_hash {
            return None;
        }
        if cert.validator_set.as_slice() != roster {
            return None;
        }
        let stake_snapshot = match consensus_mode {
            ConsensusMode::Permissioned => None,
            ConsensusMode::Npos => Some(CommitStakeSnapshot::from_roster(
                state.view().world(),
                roster,
            )?),
        };
        Some((cert, stake_snapshot))
    }

    fn persist_roster_sidecar_for_commit(&self, block: &SignedBlock, roster: &[PeerId]) {
        let height = block.header().height().get();
        let (consensus_mode, mode_tag, _) = self.consensus_context_for_height(height);
        if let Some((cert, stake_snapshot)) = Self::synthesize_commit_certificate(
            self.state.as_ref(),
            block,
            roster,
            mode_tag,
            self.epoch_for_height(height),
            consensus_mode,
        ) {
            super::status::record_commit_certificate(cert.clone());
            let sidecar = crate::kura::RosterSidecar::new_v1(
                block.header().height().get(),
                block.hash(),
                Some(cert),
                None,
                stake_snapshot,
            );
            self.kura.write_roster_metadata(&sidecar);
        }
    }

    fn effective_commit_topology_from_view(&self, view: &StateView<'_>) -> Vec<PeerId> {
        derive_active_topology(
            view,
            self.common_config.trusted_peers.value(),
            self.common_config.peer.id(),
        )
    }

    fn effective_commit_topology(&self) -> Vec<PeerId> {
        let view = self.state.view();
        self.effective_commit_topology_from_view(&view)
    }

    fn rbc_roster_for_session(&self, key: super::rbc_store::SessionKey) -> Vec<PeerId> {
        let (consensus_mode, _mode_tag, _prf_seed) = self.consensus_context_for_height(key.1);
        self.roster_for_vote_with_mode(key.0, key.1, key.2, consensus_mode)
    }

    fn rbc_session_roster(&self, key: super::rbc_store::SessionKey) -> Vec<PeerId> {
        self.subsystems
            .da_rbc
            .rbc
            .session_rosters
            .get(&key)
            .cloned()
            .unwrap_or_else(|| self.rbc_roster_for_session(key))
    }

    fn record_rbc_session_roster(
        &mut self,
        key: super::rbc_store::SessionKey,
        roster: Vec<PeerId>,
    ) {
        if roster.is_empty() {
            return;
        }
        self.subsystems
            .da_rbc
            .rbc
            .session_rosters
            .entry(key)
            .or_insert(roster);
    }

    fn clear_rbc_session_roster(&mut self, key: &super::rbc_store::SessionKey) {
        self.subsystems.da_rbc.rbc.session_rosters.remove(key);
    }

    fn record_invalid_signature(
        &mut self,
        kind: InvalidSigKind,
        height: u64,
        view: u64,
        signer: u64,
    ) -> InvalidSigOutcome {
        let outcome = self
            .invalid_sig_log
            .record(kind, height, view, signer, Instant::now());
        self.record_invalid_signature_metric(kind, outcome);
        outcome
    }

    fn record_invalid_signature_metric(&self, kind: InvalidSigKind, outcome: InvalidSigOutcome) {
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.inc_invalid_signature(kind.label(), outcome.label());
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = (kind, outcome);
    }

    fn qc_tally_key(qc: &crate::sumeragi::consensus::Qc) -> QcVoteKey {
        (
            qc.phase,
            qc.subject_block_hash,
            qc.height,
            qc.view,
            qc.epoch,
        )
    }

    fn note_validated_qc_tally(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        tally: QcSignerTally,
    ) {
        let key = Self::qc_tally_key(qc);
        self.qc_signer_tally.insert(key, tally);
    }

    fn validated_qc_tally_for_commit(
        &mut self,
        qc: &crate::sumeragi::consensus::Qc,
        block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
        block_view: u64,
        topology: &super::network_topology::Topology,
    ) -> Option<QcSignerTally> {
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(qc.height);
        let key = Self::qc_tally_key(qc);
        if let Some(tally) = self.qc_signer_tally.get(&key) {
            return Some(tally.clone());
        }

        let (result, evidence) = validate_qc_with_evidence(
            &self.vote_log,
            qc,
            topology,
            &self.common_config.chain,
            mode_tag,
            prf_seed,
        );
        match result {
            Ok(QcValidationOutcome {
                signers,
                present_signers,
                ..
            }) => {
                let tally = QcSignerTally {
                    voting_signers: signers.into_iter().collect(),
                    present_signers,
                };
                self.note_validated_qc_tally(qc, tally.clone());
                Some(tally)
            }
            Err(err) => {
                record_qc_validation_error(self.telemetry_handle(), &err);
                if let Some(evidence) = evidence {
                    let _ = self.handle_evidence(evidence);
                }
                if matches!(err, QcValidationError::MissingVotes { .. }) {
                    match tally_qc_against_block_signers(
                        qc,
                        topology,
                        block_signers,
                        block_view,
                        &self.common_config.chain,
                        mode_tag,
                        prf_seed,
                    ) {
                        Ok(tally) => {
                            self.note_validated_qc_tally(qc, tally.clone());
                            return Some(tally);
                        }
                        Err(sync_err) => {
                            record_qc_validation_error(self.telemetry_handle(), &sync_err);
                            if let Some(tally) = fallback_qc_tally_from_bitmap(
                                qc,
                                topology,
                                &self.common_config.chain,
                                mode_tag,
                                prf_seed,
                            ) {
                                self.note_validated_qc_tally(qc, tally.clone());
                                return Some(tally);
                            }
                        }
                    }
                }
                None
            }
        }
    }

    fn current_height_and_roster(&self) -> (u64, usize, Vec<u32>) {
        let view = self.state.view();
        let height = view.height() as u64;
        let topology = self.effective_commit_topology_from_view(&view);
        drop(view);
        let indices =
            compute_roster_indices_from_topology(&topology, self.epoch_roster_provider.as_ref());
        (height, topology.len(), indices)
    }

    fn runtime_da_enabled(&self) -> bool {
        sumeragi_da_enabled(&self.state)
    }

    /// Deterministic fallback roster derived from trusted peers.
    /// Uses the configured `others` ordering and appends `self` if missing.
    fn trusted_topology(&self) -> Vec<PeerId> {
        let trusted = self.common_config.trusted_peers.value();
        let mut roster = super::filter_validators_from_trusted(trusted);
        let me = self.common_config.peer.id();
        if roster.is_empty() {
            roster = trusted.clone().into_non_empty_vec().into_iter().collect();
        }
        if !roster.iter().any(|p| p == me) {
            roster.push(me.clone());
        }
        if roster.len() <= 1 {
            iroha_logger::warn!(
                roster_len = roster.len(),
                trusted_peers = trusted.others.len(),
                pops = trusted.pops.len(),
                me = ?me,
                "trusted topology resolved to a single peer"
            );
        }
        canonicalize_roster(roster)
    }

    fn current_epoch(&self) -> u64 {
        self.epoch_manager
            .as_ref()
            .map_or(0, super::epoch::EpochManager::epoch)
    }

    fn epoch_for_height(&self, height: u64) -> u64 {
        self.epoch_manager
            .as_ref()
            .map_or(0, |manager| manager.epoch_for_height(height))
    }

    fn genesis_block_hash(&self) -> Option<HashOf<BlockHeader>> {
        let height = NonZeroUsize::new(1)?;
        let block = self.kura.get_block(height)?;
        if block.header().prev_block_hash().is_some() {
            return None;
        }
        Some(block.hash())
    }

    fn qc_header_matches_genesis(&self, qc: &crate::sumeragi::consensus::QcHeaderRef) -> bool {
        if qc.phase != crate::sumeragi::consensus::Phase::Commit {
            return false;
        }
        if qc.height != 1 || qc.view != 0 {
            return false;
        }
        if qc.epoch != self.epoch_for_height(1) {
            return false;
        }
        let Some(genesis_hash) = self.genesis_block_hash() else {
            return false;
        };
        qc.subject_block_hash == genesis_hash
    }

    fn genesis_qc_stub_for_header(
        &self,
        qc: crate::sumeragi::consensus::QcHeaderRef,
        _topology_len: usize,
    ) -> Option<crate::sumeragi::consensus::Qc> {
        if !self.qc_header_matches_genesis(&qc) {
            return None;
        }
        let validator_set = self.effective_commit_topology();
        if validator_set.is_empty() {
            return None;
        }
        let bitmap_len = validator_set.len().div_ceil(8);
        let (_, mode_tag, _) = self.consensus_context_for_height(qc.height);
        Some(crate::sumeragi::consensus::Qc {
            phase: qc.phase,
            subject_block_hash: qc.subject_block_hash,
            height: qc.height,
            view: qc.view,
            epoch: qc.epoch,
            mode_tag: mode_tag.to_string(),
            highest_cert: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: crate::sumeragi::consensus::CommitAggregate {
                signers_bitmap: vec![0; bitmap_len],
                bls_aggregate_signature: Vec::new(),
            },
        })
    }

    fn is_genesis_qc_stub(&self, qc: &crate::sumeragi::consensus::Qc, topology_len: usize) -> bool {
        if !self.qc_header_matches_genesis(&Self::qc_to_header_ref(qc)) {
            return false;
        }
        let expected_len = topology_len.div_ceil(8);
        if qc.aggregate.signers_bitmap.len() != expected_len {
            return false;
        }
        if qc.aggregate.signers_bitmap.iter().any(|byte| *byte != 0) {
            return false;
        }
        qc.aggregate.bls_aggregate_signature.is_empty()
    }

    fn latest_committed_qc(&self) -> Option<crate::sumeragi::consensus::QcHeaderRef> {
        let view = self.state.view();
        let block = view.latest_block()?;
        let header = block.header();
        let height = header.height().get();
        let epoch = self.epoch_for_height(height);
        Some(crate::sumeragi::consensus::QcHeaderRef {
            height,
            view: u64::from(header.view_change_index()),
            epoch,
            subject_block_hash: block.hash(),
            phase: crate::sumeragi::consensus::Phase::Commit,
        })
    }

    fn record_membership_snapshot(
        &mut self,
        height: u64,
        view: u64,
        epoch: u64,
        topology: &super::network_topology::Topology,
    ) {
        let hash =
            compute_membership_view_hash(&self.chain_id, height, view, epoch, topology.as_ref());
        super::status::set_membership_view_hash(hash, height, view, epoch);
        #[cfg(feature = "telemetry")]
        self.telemetry
            .set_membership_view_hash(height, view, epoch, hash);
        self.broadcast_consensus_params(SumeragiMembershipStatus {
            height,
            view,
            epoch,
            view_hash: Some(hash),
        });
    }

    fn broadcast_consensus_params(&mut self, membership: SumeragiMembershipStatus) {
        let (collectors_k, redundant_send_r) =
            self.collector_plan_params_for_height(membership.height);
        let collectors_k = u16::try_from(collectors_k).unwrap_or_else(|_| {
            warn!(
                collectors_k,
                "collectors_k exceeds u16::MAX; clamping in consensus params advert"
            );
            u16::MAX
        });
        let advert = super::message::ConsensusParamsAdvert {
            collectors_k,
            redundant_send_r,
            membership: Some(membership),
        };
        self.schedule_background(BackgroundRequest::Broadcast {
            msg: BlockMessage::ConsensusParams(advert),
        });
    }

    fn determine_genesis_account(state: &State) -> Result<AccountId> {
        let view = state.view();
        let maybe_account = view
            .world
            .accounts_iter()
            .find(|entry| entry.id().domain() == &*GENESIS_DOMAIN_ID)
            .map(|entry| entry.id().clone());
        drop(view);
        maybe_account.ok_or_else(|| eyre!("genesis account not found in world state"))
    }

    fn parent_hash_for(
        &self,
        hash: HashOf<BlockHeader>,
        height: u64,
    ) -> Option<HashOf<BlockHeader>> {
        if let Some(pending) = self.pending.pending_blocks.get(&hash) {
            return pending.block.header().prev_block_hash();
        }
        if let Some(inflight) = self.subsystems.commit.inflight.as_ref()
            && inflight.block_hash == hash
        {
            return inflight.pending.block.header().prev_block_hash();
        }
        if let Some(processing_hash) = self.pending.pending_processing.get()
            && processing_hash == hash
        {
            return self.pending.pending_processing_parent.get();
        }
        let Ok(height_usize) = usize::try_from(height) else {
            return None;
        };
        let nz_height = NonZeroUsize::new(height_usize)?;
        let block = self.kura.get_block(nz_height)?;
        if block.hash() != hash {
            warn!(
                ?hash,
                stored = %block.hash(),
                height,
                "block hash mismatch while tracing QC ancestry"
            );
            return None;
        }
        block.header().prev_block_hash()
    }

    fn block_payload_available_locally(&self, hash: HashOf<BlockHeader>) -> bool {
        self.pending.pending_blocks.contains_key(&hash)
            || self
                .subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| inflight.block_hash == hash)
            || self
                .pending
                .pending_processing
                .get()
                .is_some_and(|pending| pending == hash)
            || self.kura.get_block_height_by_hash(hash).is_some()
    }

    fn block_known_locally(&self, hash: HashOf<BlockHeader>) -> bool {
        self.pending
            .pending_blocks
            .get(&hash)
            .is_some_and(|pending| !pending.aborted)
            || self
                .subsystems
                .commit
                .inflight
                .as_ref()
                .is_some_and(|inflight| inflight.block_hash == hash && !inflight.pending.aborted)
            || self
                .pending
                .pending_processing
                .get()
                .is_some_and(|pending| pending == hash)
            || self.kura.get_block_height_by_hash(hash).is_some()
    }

    fn highest_qc_extends_locked(&self, highest: crate::sumeragi::consensus::QcHeaderRef) -> bool {
        qc_extends_locked_if_present(
            self.locked_qc,
            highest,
            |hash, height| self.parent_hash_for(hash, height),
            |hash| self.block_known_locally(hash),
        )
    }

    fn maybe_realign_locked_to_committed_tip(&mut self) -> bool {
        let Some(committed_qc) = self.latest_committed_qc() else {
            return false;
        };
        let Some(locked_qc) = self.locked_qc else {
            return false;
        };
        let committed_hash = committed_qc.subject_block_hash;
        let extends_tip = chain_extends_tip(
            locked_qc.subject_block_hash,
            locked_qc.height,
            committed_qc.height,
            committed_hash,
            |hash, height| self.parent_hash_for(hash, height),
        );
        if matches!(extends_tip, Some(false)) {
            info!(
                locked_height = locked_qc.height,
                locked_hash = %locked_qc.subject_block_hash,
                committed_height = committed_qc.height,
                committed_hash = %committed_hash,
                "realigning locked QC to committed tip after divergence"
            );
            self.locked_qc = Some(committed_qc);
            super::status::set_locked_qc(
                committed_qc.height,
                committed_qc.view,
                Some(committed_hash),
            );
            return true;
        }
        false
    }

    fn evidence_horizon_context(&self) -> Option<(u64, u64)> {
        let view = self.state.view();
        let current_height = u64::try_from(view.height()).unwrap_or(u64::MAX);
        let from_wsv = view
            .world()
            .sumeragi_npos_parameters()
            .map(|params| params.evidence_horizon_blocks());
        drop(view);
        let configured_horizon = matches!(self.consensus_mode, ConsensusMode::Npos)
            .then_some(self.config.npos.reconfig.evidence_horizon_blocks);
        from_wsv
            .or(configured_horizon)
            .map(|hz| (current_height, hz))
    }

    fn evidence_is_fresh(&self, evidence: &crate::sumeragi::consensus::Evidence) -> bool {
        let Some((current_height, horizon)) = self.evidence_horizon_context() else {
            return true;
        };
        let (subject_height, _) = super::evidence::evidence_subject_height_view(evidence);
        if evidence_within_horizon(current_height, horizon, subject_height) {
            true
        } else {
            debug!(
                evidence_kind = ?evidence.kind,
                current_height,
                horizon,
                subject_height = subject_height.unwrap_or(current_height),
                "dropping stale evidence beyond configured horizon"
            );
            false
        }
    }

    fn record_evidence(&mut self, evidence: &crate::sumeragi::consensus::Evidence) -> Result<bool> {
        if !self.evidence_is_fresh(evidence) {
            return Ok(false);
        }
        let (subject_height, _) = super::evidence::evidence_subject_height_view(evidence);
        let fallback_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
        let evidence_height = subject_height.unwrap_or(fallback_height);
        let (consensus_mode, mode_tag, prf_seed) =
            self.consensus_context_for_height(evidence_height);
        let topology_peers =
            match &evidence.payload {
                crate::sumeragi::consensus::EvidencePayload::DoubleVote { v1, .. } => self
                    .roster_for_vote_with_mode(v1.block_hash, v1.height, v1.view, consensus_mode),
                crate::sumeragi::consensus::EvidencePayload::DoubleExecVote { v1, .. } => self
                    .roster_for_vote_with_mode(v1.block_hash, v1.height, v1.view, consensus_mode),
                crate::sumeragi::consensus::EvidencePayload::InvalidCommitCertificate {
                    certificate,
                    ..
                } => self.roster_for_vote_with_mode(
                    certificate.subject_block_hash,
                    certificate.height,
                    certificate.view,
                    consensus_mode,
                ),
                crate::sumeragi::consensus::EvidencePayload::InvalidProposal { .. }
                | crate::sumeragi::consensus::EvidencePayload::Censorship { .. } => {
                    self.effective_commit_topology()
                }
            };
        if topology_peers.is_empty() {
            debug!(
                evidence_kind = ?evidence.kind,
                height = evidence_height,
                "dropping evidence with empty commit topology"
            );
            return Ok(false);
        }
        let topology = super::network_topology::Topology::new(topology_peers);
        let context = super::evidence::EvidenceValidationContext {
            topology: &topology,
            chain_id: &self.common_config.chain,
            mode_tag,
            prf_seed,
        };
        if !self.evidence_store.insert(evidence, &context) {
            return Ok(false);
        }
        Ok(super::evidence::persist_record(
            self.state.as_ref(),
            evidence,
            &context,
        ))
    }

    pub(super) fn record_and_broadcast_evidence(
        &mut self,
        evidence: crate::sumeragi::consensus::Evidence,
    ) -> Result<()> {
        let recorded = self.record_evidence(&evidence)?;
        if recorded {
            self.schedule_background(BackgroundRequest::BroadcastControlFlow {
                frame: ControlFlow::Evidence(evidence),
            });
        }
        Ok(())
    }

    fn handle_evidence(&mut self, evidence: crate::sumeragi::consensus::Evidence) -> Result<()> {
        let recorded = self.record_evidence(&evidence)?;
        if recorded
            && matches!(
                evidence.kind,
                crate::sumeragi::consensus::EvidenceKind::Censorship
            )
        {
            let committed_qc = self.latest_committed_qc();
            let committed_height = committed_qc.as_ref().map_or_else(
                || u64::try_from(self.state.view().height()).unwrap_or(0),
                |qc| qc.height,
            );
            let active_height =
                active_round_height(self.highest_qc, committed_qc, committed_height);
            self.trigger_view_change_with_cause(
                active_height,
                0,
                ViewChangeCause::CensorshipEvidence,
            );
        }
        Ok(())
    }

    fn new_view_gossip_targets(
        &self,
        topology: &[PeerId],
        sender: Option<crate::sumeragi::consensus::ValidatorIndex>,
    ) -> Vec<PeerId> {
        if topology.is_empty() || self.block_sync_gossip_limit == 0 {
            return Vec::new();
        }
        let local_peer = self.common_config.peer.id();
        let sender_peer = sender
            .and_then(|idx| usize::try_from(idx).ok())
            .and_then(|idx| topology.get(idx));
        let mut targets = Vec::new();
        for peer in topology {
            if peer == local_peer {
                continue;
            }
            if sender_peer.is_some_and(|sender_peer| sender_peer == peer) {
                continue;
            }
            targets.push(peer.clone());
            if targets.len() >= self.block_sync_gossip_limit {
                break;
            }
        }
        targets
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn new(
        config: SumeragiConfig,
        common_config: CommonConfig,
        consensus_frame_cap: usize,
        events_sender: EventsSender,
        state: Arc<State>,
        queue: Arc<Queue>,
        kura: Arc<Kura>,
        network: IrohaNetwork,
        da_spool_dir: PathBuf,
        peers_gossiper: PeersGossiperHandle,
        genesis_network: GenesisWithPubKey,
        block_count: BlockCount,
        block_sync_gossip_limit: usize,
        #[cfg(feature = "telemetry")] telemetry: Telemetry,
        epoch_roster_provider: Option<Arc<WsvEpochRosterAdapter>>,
        rbc_store: Option<RbcStoreConfig>,
        background_post_tx: Option<mpsc::SyncSender<BackgroundPost>>,
        rbc_status_handle: rbc_status::Handle,
    ) -> Result<Self> {
        let chain_id = common_config.chain.clone();
        let chain_hash = Self::derive_chain_hash(&chain_id);
        let rbc_manifest = super::rbc_store::SoftwareManifest::current();
        let backpressure_gate = BackpressureGate::new(queue.backpressure_handle().subscribe());
        let now = Instant::now();
        let block_time = {
            let view = state.view();
            view.world.parameters().sumeragi().block_time()
        };
        let qc_rebuild_cooldown = block_time.max(REBROADCAST_COOLDOWN_FLOOR);
        let initial_qc_rebuild = now.checked_sub(qc_rebuild_cooldown).unwrap_or(now);
        let commit_pipeline_cooldown = qc_rebuild_cooldown;
        let initial_commit_pipeline_run = now.checked_sub(commit_pipeline_cooldown).unwrap_or(now);
        let mut pending_roster_activation: Option<(u64, Vec<PeerId>)> = None;
        let mut roster_to_install_now: Option<Vec<PeerId>> = None;
        let (consensus_mode, npos_collectors, epoch_manager, epoch_params) = {
            let view = state.view();
            let commit_topology = derive_active_topology(
                &view,
                common_config.trusted_peers.value(),
                common_config.peer.id(),
            );
            let mode = super::effective_consensus_mode(&view, config.consensus_mode);
            let mut collectors = if matches!(mode, ConsensusMode::Npos) {
                super::load_npos_collector_config(&view)
            } else {
                None
            };
            let epoch_params = matches!(mode, ConsensusMode::Npos)
                .then(|| super::load_npos_epoch_params(&view, &config));
            let last_epoch_record = view
                .world()
                .vrf_epochs()
                .iter()
                .last()
                .map(|(_, record)| record.clone());
            let epoch_seed = super::latest_epoch_seed(&view);
            drop(view);
            let roster_len = commit_topology.len();
            let initial_indices = compute_roster_indices_from_topology(
                &commit_topology,
                epoch_roster_provider.as_ref(),
            );
            let manager = if matches!(mode, ConsensusMode::Npos) {
                let epoch_params =
                    epoch_params.expect("epoch params should be available in NPoS mode");
                let mut em = EpochManager::new_from_chain(&common_config.chain);
                em.set_params(
                    epoch_params.epoch_length_blocks,
                    epoch_params.commit_deadline_offset,
                    epoch_params.reveal_deadline_offset,
                );
                if let Some(record) = last_epoch_record.as_ref() {
                    em.restore_from_record(record);
                } else {
                    em.set_epoch_seed(epoch_seed);
                }
                apply_roster_indices_to_manager(&mut em, roster_len, initial_indices);
                let seed = em.seed();
                if let Some(record) = last_epoch_record.as_ref() {
                    if let Some((activate_at, roster, apply_now)) =
                        Self::activation_plan_from_vrf_record(block_count.0 as u64, record)
                    {
                        if apply_now {
                            roster_to_install_now = Some(roster);
                        } else {
                            pending_roster_activation = Some((activate_at, roster));
                        }
                    }
                }
                if let Some(cfg) = collectors.as_mut() {
                    cfg.seed = seed;
                } else {
                    collectors = Some(NposCollectorConfig {
                        seed,
                        k: config.npos.k_aggregators,
                        redundant_send_r: config.npos.redundant_send_r,
                    });
                }
                Some(em)
            } else {
                None
            };
            (mode, collectors, manager, epoch_params)
        };
        if let Some(manager) = epoch_manager.as_ref() {
            if let Some(params) = epoch_params {
                super::status::set_epoch_parameters(
                    params.epoch_length_blocks,
                    params.commit_deadline_offset,
                    params.reveal_deadline_offset,
                );
                #[cfg(feature = "telemetry")]
                telemetry.set_epoch_parameters(
                    params.epoch_length_blocks,
                    params.commit_deadline_offset,
                    params.reveal_deadline_offset,
                );
            }
            let seed = manager.seed();
            super::status::set_prf_context(seed, block_count.0 as u64, 0);
            #[cfg(feature = "telemetry")]
            telemetry.set_prf_context(Some(seed), block_count.0 as u64, 0);
        } else if let Some(cfg) = npos_collectors {
            super::status::set_prf_context(cfg.seed, block_count.0 as u64, 0);
            #[cfg(feature = "telemetry")]
            telemetry.set_prf_context(Some(cfg.seed), block_count.0 as u64, 0);
        }

        let chunk_store = if let Some(cfg) = rbc_store.as_ref() {
            if cfg.max_sessions == 0 || cfg.max_bytes == 0 {
                rbc_status_handle.configure(None);
                None
            } else {
                rbc_status_handle.configure(Some(rbc_status::StoreConfig {
                    dir: cfg.dir.clone(),
                    ttl: cfg.ttl,
                    capacity: cfg.max_sessions,
                }));
                Some(
                    super::rbc_store::ChunkStore::new(
                        cfg.dir.clone(),
                        cfg.ttl,
                        cfg.soft_sessions,
                        cfg.soft_bytes,
                        cfg.max_sessions,
                        cfg.max_bytes,
                    )
                    .map_err(|err| eyre!("failed to initialise RBC chunk store: {err}"))?,
                )
            }
        } else {
            rbc_status_handle.configure(None);
            None
        };

        let mut rbc_sessions = BTreeMap::new();
        let mut rbc_session_rosters = BTreeMap::new();
        let mut initial_rbc_store_pressure: Option<super::rbc_store::StorePressure> = None;
        if let Some(store) = chunk_store.as_ref() {
            match store.load(&chain_hash, &rbc_manifest) {
                Ok(load) => {
                    let super::rbc_store::LoadResult {
                        sessions,
                        removed,
                        pressure,
                    } = load;
                    initial_rbc_store_pressure = Some(pressure);
                    for persisted in sessions {
                        match RbcSession::from_persisted_unchecked(&persisted) {
                            Ok(mut session) => {
                                let key: super::rbc_store::SessionKey =
                                    (persisted.block_hash, persisted.height, persisted.view);
                                if session.lane_allocations.is_empty()
                                    && session.dataspace_allocations.is_empty()
                                {
                                    if let Some(summary) = rbc_status_handle.get(&key) {
                                        session.adopt_allocations_from_summary(&summary);
                                    }
                                }
                                let ready_count = u64::try_from(session.ready_signatures.len())
                                    .unwrap_or(u64::MAX);
                                let summary = super::rbc_status::Summary {
                                    block_hash: key.0,
                                    height: key.1,
                                    view: key.2,
                                    total_chunks: session.total_chunks(),
                                    received_chunks: session.received_chunks(),
                                    ready_count,
                                    delivered: session.delivered,
                                    payload_hash: session.payload_hash(),
                                    recovered_from_disk: session.recovered_from_disk(),
                                    invalid: session.is_invalid(),
                                    lane_backlog: session.lane_backlog_entries(),
                                    dataspace_backlog: session.dataspace_backlog_entries(),
                                };
                                rbc_status_handle.update(summary, SystemTime::now());
                                let roster = persisted.session_roster.clone();
                                rbc_sessions.insert(key, session);
                                if !roster.is_empty() {
                                    rbc_session_rosters.insert(key, roster);
                                }
                            }
                            Err(err) => {
                                warn!(?persisted.block_hash, ?err, "failed to rebuild persisted RBC session");
                            }
                        }
                    }
                    if !removed.is_empty() {
                        warn!(
                            removed = removed.len(),
                            "removed {} stale RBC sessions while loading persisted snapshot",
                            removed.len()
                        );
                        super::status::record_rbc_store_evictions(&removed);
                        #[cfg(feature = "telemetry")]
                        telemetry.inc_rbc_store_evictions(removed.len() as u64);
                    }
                }
                Err(err) => {
                    warn!(?err, "failed to load persisted RBC sessions");
                }
            }
        }

        #[cfg(feature = "telemetry")]
        let telemetry_option = Some(&telemetry);
        #[cfg(not(feature = "telemetry"))]
        let telemetry_option: Option<&crate::telemetry::Telemetry> = None;
        let pending_caps = {
            let hard_chunk_cap = usize::try_from(RBC_MAX_TOTAL_CHUNKS).unwrap_or(usize::MAX);
            let max_chunks = config.rbc_pending_max_chunks.min(hard_chunk_cap).max(1);
            let hard_bytes = config.rbc_chunk_max_bytes.saturating_mul(hard_chunk_cap);
            let max_bytes = config
                .rbc_pending_max_bytes
                .min(hard_bytes)
                .max(config.rbc_chunk_max_bytes);
            (max_chunks, max_bytes)
        };
        Self::update_rbc_backlog_snapshot(
            &rbc_sessions,
            &BTreeMap::new(),
            pending_caps,
            config.rbc_pending_ttl,
            telemetry_option,
        );

        debug!(chain=?common_config.chain, mode=?consensus_mode, "initialising Sumeragi actor");

        let genesis_account = Self::determine_genesis_account(state.as_ref())?;
        let lane_relay = LaneRelayBroadcaster::new(network.clone());
        let npos_timeouts = config.npos.timeouts;
        let pacemaker_base_interval = pacemaker_base_interval(block_time, &config);
        let collector_redundant_limit = match consensus_mode {
            ConsensusMode::Permissioned => {
                let view = state.view();
                let redundant = view
                    .world
                    .parameters()
                    .sumeragi()
                    .collectors_redundant_send_r;
                drop(view);
                redundant.max(1)
            }
            ConsensusMode::Npos => {
                let redundant = npos_collectors
                    .as_ref()
                    .map(|cfg| cfg.redundant_send_r)
                    .or_else(|| {
                        let view = state.view();
                        let cfg = super::load_npos_collector_config(&view);
                        drop(view);
                        cfg.map(|cfg| cfg.redundant_send_r)
                    })
                    .unwrap_or(config.npos.redundant_send_r);
                redundant.max(1)
            }
        };
        let adaptive_cfg = config.adaptive_observability;
        let adaptive_state = AdaptiveObservabilityState::new(
            adaptive_cfg,
            pacemaker_base_interval,
            collector_redundant_limit,
            super::status::da_gate_missing_local_data_total(),
        );

        #[cfg(feature = "telemetry")]
        {
            let max_backoff_ms =
                u64::try_from(config.npos.pacemaker_max_backoff.as_millis()).unwrap_or(u64::MAX);
            let backoff_mul = u64::from(config.npos.pacemaker_backoff_multiplier);
            let rtt_floor_mul = u64::from(config.npos.pacemaker_rtt_floor_multiplier);
            let jitter_frac = u64::from(config.npos.pacemaker_jitter_frac_permille);
            let jitter_ms = max_backoff_ms.saturating_mul(jitter_frac) / 1000;
            let view_target_ms =
                u64::try_from(pacemaker_base_interval.as_millis()).unwrap_or(u64::MAX);
            telemetry.set_pacemaker_config(backoff_mul, rtt_floor_mul, max_backoff_ms);
            telemetry.set_pacemaker_jitter_permille(jitter_frac);
            telemetry.set_pacemaker_jitter_ms(jitter_ms);
            telemetry.set_pacemaker_backoff_ms(0);
            telemetry.set_pacemaker_rtt_floor_ms(0);
            telemetry.set_pacemaker_view_timeout_target_ms(view_target_ms);
            telemetry.set_pacemaker_view_timeout_remaining_ms(view_target_ms);
        }

        let rbc_state = RbcState {
            store_cfg: rbc_store,
            chunk_store,
            manifest: rbc_manifest,
            sessions: rbc_sessions,
            pending: BTreeMap::new(),
            session_rosters: rbc_session_rosters,
            status_handle: rbc_status_handle,
            payload_rebroadcast_last_sent: BTreeMap::new(),
            ready_rebroadcast_last_sent: BTreeMap::new(),
            persisted_full_sessions: BTreeSet::new(),
        };
        let propose_state = ProposeState {
            backpressure_gate,
            pacemaker: Pacemaker::new(pacemaker_base_interval, now),
            forced_view_after_timeout: None,
            proposal_cache: ProposalCache::new(PROPOSAL_CACHE_LIMIT),
            collector_plan: None,
            collector_plan_subject: None,
            collector_plan_targets: Vec::new(),
            collectors_contacted: BTreeSet::new(),
            collector_redundant_limit,
            adaptive_cfg,
            adaptive_state,
            collector_role_index: None,
            new_view_tracker: NewViewTracker::default(),
            proposals_seen: BTreeSet::new(),
            pacemaker_backpressure: PacemakerBackpressure::new(),
            last_pacemaker_attempt: None,
            last_successful_proposal: None,
            propose_attempt_monitor: ProposeAttemptMonitor::new(),
            last_empty_child_attempt: None,
        };
        let subsystems = ActorSubsystems {
            commit: CommitState::new(),
            propose: propose_state,
            da_rbc: DaRbcState {
                da: DaState::new(),
                rbc: rbc_state,
                spool_dir: da_spool_dir,
                spool_cache: DaSpoolCache::default(),
                manifest_cache: crate::sumeragi::main_loop::ManifestSpoolCache::default(),
            },
            vrf: VrfActor::new(),
            merge: MergeLaneState {
                lane_relay,
                committee: MergeCommitteeState::new(),
            },
        };

        let mut actor = Self {
            config,
            consensus_mode,
            common_config,
            chain_id,
            chain_hash,
            consensus_frame_cap,
            events_sender,
            state,
            queue,
            kura,
            network,
            subsystems,
            peers_gossiper,
            genesis_network,
            block_count,
            block_sync_gossip_limit,
            last_advertised_topology: BTreeSet::new(),
            last_commit_topology_hash: None,
            last_committed_height: block_count.0 as u64,
            #[cfg(feature = "telemetry")]
            telemetry,
            epoch_roster_provider,
            background_post_tx,
            phase_tracker: PhaseTracker::new(now),
            phase_ema: PhaseEma::new(&npos_timeouts),
            evidence_store: EvidenceStore::new(),
            invalid_sig_log: InvalidSigThrottle::default(),
            genesis_account,
            vote_log: BTreeMap::new(),
            exec_vote_log: BTreeMap::new(),
            qc_cache: BTreeMap::new(),
            qc_signer_tally: BTreeMap::new(),
            execution_qc_cache: BTreeMap::new(),
            epoch_manager,
            npos_collectors,
            pending: PendingBlockState {
                pending_blocks: BTreeMap::new(),
                pending_replay_last_sent: BTreeMap::new(),
                missing_block_requests: BTreeMap::new(),
                pending_processing: Cell::new(None),
                pending_processing_parent: Cell::new(None),
                last_commit_pipeline_run: initial_commit_pipeline_run,
            },
            voting_block: None,
            pending_roster_activation,
            pending_mode_flip: None,
            highest_qc: None,
            locked_qc: None,
            tick_counter: 0,
            tick_timing: TickTimingMonitor::new(now),
            last_qc_rebuild: initial_qc_rebuild,
            payload_rebroadcast_log: PayloadRebroadcastThrottle::default(),
            block_sync_rebroadcast_log: PayloadRebroadcastThrottle::default(),
            block_sync_fetch_log: PayloadRebroadcastThrottle::default(),
        };
        actor.refresh_p2p_topology();
        if let Some(roster) = roster_to_install_now {
            if let Err(err) = actor.install_elected_roster(&roster) {
                iroha_logger::warn!(?err, "failed to install elected roster on startup");
            } else {
                actor.pending_roster_activation = None;
            }
        }
        actor.refresh_commit_topology_state(HashOf::new(&actor.effective_commit_topology()));
        // Publish initial status so operator endpoints are populated even before the
        // first tick, and to make stalled actor detection easier in tests.
        super::status::set_tx_queue_backpressure(
            actor.subsystems.propose.backpressure_gate.state(),
        );
        super::status::set_leader_index(
            actor
                .local_validator_index(&actor.state.view())
                .map(u64::from)
                .unwrap_or_default(),
        );
        iroha_logger::info!(
            height = actor.state.view().height(),
            queue_len = actor.queue.tx_len(),
            "sumeragi actor initialized"
        );
        if let Some(pressure) = initial_rbc_store_pressure {
            actor.update_rbc_store_pressure(pressure);
        } else if actor.subsystems.da_rbc.rbc.chunk_store.is_some() {
            actor.update_rbc_store_pressure(super::rbc_store::StorePressure::Normal {
                sessions: actor.subsystems.da_rbc.rbc.sessions.len(),
                bytes: 0,
            });
        }
        Ok(actor)
    }

    fn derive_chain_hash(chain_id: &ChainId) -> Hash {
        Hash::new(chain_id.clone().into_inner().as_bytes())
    }

    fn emit_pipeline_event(&self, event: PipelineEventBox) {
        if let Err(err) = self.events_sender.send(EventBox::Pipeline(event)) {
            debug!(?err, "failed to forward pipeline event");
        }
    }

    fn leader_index_for(
        &self,
        topology: &mut super::network_topology::Topology,
        height: u64,
        view: u64,
    ) -> Result<usize> {
        let (consensus_mode, _, prf_seed) = self.consensus_context_for_height(height);
        match consensus_mode {
            ConsensusMode::Permissioned => {
                let view_usize = usize::try_from(view)
                    .map_err(|_| eyre!("view {view} exceeds platform limits"))?;
                topology.nth_rotation(view_usize);
                Ok(topology.leader_index())
            }
            ConsensusMode::Npos => {
                let seed = prf_seed.ok_or_else(|| {
                    eyre!("missing NPoS PRF seed for height {height} in leader selection")
                })?;
                let leader = topology.leader_index_prf(seed, height, view);
                topology.rotate_preserve_view_to_front(leader);
                Ok(topology.leader_index())
            }
        }
    }

    fn collector_plan_params(&self) -> (usize, u8) {
        self.collector_plan_params_for_mode(self.consensus_mode)
    }

    fn collector_plan_params_for_height(&self, height: u64) -> (usize, u8) {
        let (consensus_mode, _, _) = self.consensus_context_for_height(height);
        self.collector_plan_params_for_mode(consensus_mode)
    }

    fn collector_plan_params_for_mode(&self, consensus_mode: ConsensusMode) -> (usize, u8) {
        match consensus_mode {
            ConsensusMode::Permissioned => {
                let view = self.state.view();
                let params = view.world.parameters().sumeragi();
                let k = usize::from(params.collectors_k);
                let redundant = params.collectors_redundant_send_r;
                drop(view);
                (k, redundant)
            }
            ConsensusMode::Npos => self.npos_collectors.map_or_else(
                || {
                    let view = self.state.view();
                    super::load_npos_collector_config(&view).map_or(
                        (
                            self.config.npos.k_aggregators,
                            self.config.npos.redundant_send_r,
                        ),
                        |cfg| (cfg.k, cfg.redundant_send_r),
                    )
                },
                |cfg| (cfg.k, cfg.redundant_send_r),
            ),
        }
    }

    fn reset_collector_state(&mut self) {
        self.subsystems.propose.collector_plan = None;
        self.subsystems.propose.collector_plan_subject = None;
        self.subsystems.propose.collector_plan_targets.clear();
        self.subsystems.propose.collectors_contacted.clear();
        self.subsystems.propose.collector_role_index = None;
        let (_, redundant_r) = self.collector_plan_params();
        self.subsystems.propose.collector_redundant_limit = redundant_r.max(1);
        self.subsystems
            .propose
            .adaptive_state
            .update_base_collector_limit(self.subsystems.propose.collector_redundant_limit);
        super::status::set_collectors_targeted_current(0);
        #[cfg(feature = "telemetry")]
        self.telemetry.set_collectors_targeted_current(0);
    }

    fn init_collector_plan(
        &mut self,
        topology: &super::network_topology::Topology,
        height: u64,
        view: u64,
    ) {
        let (consensus_mode, _, prf_seed) = self.consensus_context_for_height(height);
        let epoch = self.epoch_for_height(height);
        self.record_membership_snapshot(height, view, epoch, topology);
        self.reset_collector_state();
        self.subsystems.propose.collector_plan_subject = Some((height, view));
        let (k, redundant_r) = self.collector_plan_params_for_mode(consensus_mode);
        self.subsystems.propose.collector_redundant_limit = redundant_r.max(1);
        self.subsystems
            .propose
            .adaptive_state
            .update_base_collector_limit(self.subsystems.propose.collector_redundant_limit);
        if k == 0 {
            return;
        }

        let targets = deterministic_collectors(topology, consensus_mode, k, prf_seed, height, view);
        if targets.is_empty() {
            return;
        }

        self.subsystems
            .propose
            .collector_plan_targets
            .clone_from(&targets);
        self.subsystems.propose.collector_plan = Some(CollectorPlan::new(targets));
        self.subsystems.propose.collector_role_index = {
            let view_snapshot = self.state.view();
            let idx = self.local_validator_index(&view_snapshot);
            drop(view_snapshot);
            idx
        };

        if let Some(plan) = self.subsystems.propose.collector_plan.as_mut() {
            if let Some(primary) = plan.next() {
                self.note_collector_contact(primary, false);
            }
        }
    }

    fn ensure_collector_plan(
        &mut self,
        topology: &super::network_topology::Topology,
        height: u64,
        view: u64,
    ) {
        if self.subsystems.propose.collector_plan_subject == Some((height, view)) {
            return;
        }
        self.init_collector_plan(topology, height, view);
    }

    fn note_collector_contact(&mut self, peer: PeerId, redundant: bool) {
        let inserted = self
            .subsystems
            .propose
            .collectors_contacted
            .insert(peer.clone());
        if inserted {
            let count = self.subsystems.propose.collectors_contacted.len() as u64;
            super::status::set_collectors_targeted_current(count);
            #[cfg(feature = "telemetry")]
            self.telemetry.set_collectors_targeted_current(count);
        }

        if redundant && inserted {
            super::status::inc_redundant_sends();
            #[cfg(feature = "telemetry")]
            {
                self.telemetry.inc_redundant_send();
                if let Some(idx) = self
                    .subsystems
                    .propose
                    .collector_plan_targets
                    .iter()
                    .position(|candidate| candidate == &peer)
                {
                    self.telemetry.inc_redundant_send_for_collector(idx);
                }
                self.telemetry.inc_redundant_send_for_peer(&peer);
                if let Some(role_idx) = self.subsystems.propose.collector_role_index {
                    if let Ok(idx) = usize::try_from(role_idx) {
                        self.telemetry.inc_redundant_send_for_role(idx);
                    }
                }
            }
        }
    }

    fn next_redundant_collector(&mut self) -> Option<PeerId> {
        let limit = usize::from(self.subsystems.propose.collector_redundant_limit.max(1));
        if self.subsystems.propose.collectors_contacted.len() >= limit {
            if self
                .subsystems
                .propose
                .collector_plan
                .as_mut()
                .is_some_and(super::collectors::CollectorPlan::trigger_gossip)
            {
                super::status::inc_gossip_fallback();
                #[cfg(feature = "telemetry")]
                self.telemetry.inc_gossip_fallback();
            }
            return None;
        }
        self.subsystems
            .propose
            .collector_plan
            .as_mut()
            .and_then(|plan| {
                let next = plan.next();
                if next.is_none() && plan.trigger_gossip() {
                    super::status::inc_gossip_fallback();
                    #[cfg(feature = "telemetry")]
                    self.telemetry.inc_gossip_fallback();
                }
                next
            })
    }

    fn finalize_collector_plan(&mut self, committed: bool) {
        if committed {
            let count = self.subsystems.propose.collectors_contacted.len() as u64;
            super::status::observe_collectors_targeted_per_block(count);
            #[cfg(feature = "telemetry")]
            self.telemetry.observe_collectors_targeted_per_block(count);
        }
        self.reset_collector_state();
    }

    fn recompute_consensus_caps(&self) -> iroha_p2p::ConsensusConfigCaps {
        let sumeragi = &self.config;
        let (collectors_k, redundant_send_r) = self.collector_plan_params();
        let da_enabled = sumeragi_da_enabled(&self.state);
        let config_caps = iroha_p2p::ConsensusConfigCaps {
            collectors_k: u16::try_from(collectors_k).unwrap_or(u16::MAX),
            redundant_send_r,
            da_enabled,
            require_execution_qc: sumeragi.require_execution_qc,
            require_wsv_exec_qc: sumeragi.require_wsv_exec_qc,
            rbc_chunk_max_bytes: u64::try_from(sumeragi.rbc_chunk_max_bytes).unwrap_or(u64::MAX),
            rbc_session_ttl_ms: u64::try_from(sumeragi.rbc_session_ttl.as_millis())
                .unwrap_or(u64::MAX),
            rbc_store_max_sessions: u32::try_from(sumeragi.rbc_store_max_sessions)
                .unwrap_or(u32::MAX),
            rbc_store_soft_sessions: u32::try_from(sumeragi.rbc_store_soft_sessions)
                .unwrap_or(u32::MAX),
            rbc_store_max_bytes: u64::try_from(sumeragi.rbc_store_max_bytes).unwrap_or(u64::MAX),
            rbc_store_soft_bytes: u64::try_from(sumeragi.rbc_store_soft_bytes).unwrap_or(u64::MAX),
        };
        super::status::set_consensus_caps(&config_caps);
        config_caps
    }

    fn apply_adaptive_observability(&mut self, now: Instant) -> bool {
        let metrics = AdaptiveObservabilityMetrics::gather();
        match self.subsystems.propose.adaptive_state.evaluate(
            self.subsystems.propose.adaptive_cfg,
            metrics,
            &mut self.subsystems.propose.pacemaker,
            &mut self.subsystems.propose.collector_redundant_limit,
            now,
        ) {
            AdaptiveAction::Applied => {
                info!(
                    qc_latency_ms = metrics.max_qc_latency_ms,
                    missing_local_data = metrics.missing_local_data_total,
                    collector_limit = self.subsystems.propose.collector_redundant_limit,
                    propose_interval_ms = self
                        .subsystems
                        .propose
                        .pacemaker
                        .propose_interval
                        .as_millis(),
                    "adaptive observability widened collector fan-out/pacemaker interval"
                );
                true
            }
            AdaptiveAction::Reset => {
                info!(
                    collector_limit = self.subsystems.propose.collector_redundant_limit,
                    propose_interval_ms = self
                        .subsystems
                        .propose
                        .pacemaker
                        .propose_interval
                        .as_millis(),
                    "adaptive observability reset to baseline thresholds"
                );
                true
            }
            AdaptiveAction::None => false,
        }
    }

    fn tick_mode_management(&mut self) -> bool {
        let (effective_mode, staged_mode_tag, staged_mode_activation_height, current_height) = {
            let view = self.state.view();
            let params = view.world.parameters().sumeragi();
            let (staged_tag, staged_height) = staged_mode_info(params);
            (
                super::effective_consensus_mode(&view, self.config.consensus_mode),
                staged_tag,
                staged_height,
                view.height() as u64,
            )
        };
        let mode_activation_lag_blocks = mode_activation_lag(
            current_height,
            staged_mode_activation_height,
            effective_mode,
            self.consensus_mode,
        );
        super::status::set_mode_tags(
            self.mode_tag(),
            staged_mode_tag,
            staged_mode_activation_height,
        );
        super::status::set_mode_activation_lag(mode_activation_lag_blocks);
        super::status::set_mode_flip_kill_switch(self.config.mode_flip_enabled);
        #[cfg(feature = "telemetry")]
        self.telemetry.set_mode_tags(
            self.mode_tag(),
            staged_mode_tag,
            staged_mode_activation_height,
        );
        #[cfg(feature = "telemetry")]
        self.telemetry
            .set_mode_activation_lag(mode_activation_lag_blocks);
        #[cfg(feature = "telemetry")]
        self.telemetry
            .set_mode_flip_kill_switch(self.config.mode_flip_enabled);
        if let Some(target_mode) = update_pending_mode_flip(
            self.consensus_mode,
            effective_mode,
            &mut self.pending_mode_flip,
        ) {
            info!(
                current_mode = ?self.consensus_mode,
                staged_mode = ?target_mode,
                "pending consensus mode flip detected"
            );
        }
        let flip_blocked = !self.config.mode_flip_enabled && effective_mode != self.consensus_mode;
        if flip_blocked && !super::status::mode_flip_blocked() {
            let now_ms = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
                .unwrap_or(0);
            super::status::note_mode_flip_blocked("mode_flip_disabled", now_ms);
            #[cfg(feature = "telemetry")]
            self.telemetry
                .inc_mode_flip_blocked(self.mode_tag(), now_ms);
        } else if self.config.mode_flip_enabled {
            super::status::clear_mode_flip_blocked();
        }
        if self.config.mode_flip_enabled {
            if let Some(target_mode) = self.pending_mode_flip
                && target_mode == effective_mode
            {
                match self.apply_mode_flip(target_mode) {
                    Ok(()) => {
                        return true;
                    }
                    Err(err) => {
                        let now_ms = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
                            .unwrap_or(0);
                        super::status::note_mode_flip_failure("mode_flip_apply_failed", now_ms);
                        #[cfg(feature = "telemetry")]
                        self.telemetry
                            .inc_mode_flip_failure(self.mode_tag(), now_ms);
                        warn!(
                            ?err,
                            current_mode = ?self.consensus_mode,
                            target_mode = ?target_mode,
                            "failed to apply runtime consensus mode flip"
                        );
                    }
                }
            }
        }
        false
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn tick(&mut self) -> bool {
        let tick_start = Instant::now();
        self.tick_counter = self.tick_counter.saturating_add(1);
        if self.tick_counter.is_multiple_of(50) {
            let view = self.state.view();
            let backpressure = self.queue_backpressure_state();
            iroha_logger::info!(
                height = view.height(),
                queue_len = self.queue.tx_len(),
                queue_cap = backpressure.capacity().get(),
                queue_saturated = backpressure.is_saturated(),
                tick = self.tick_counter,
                "sumeragi tick heartbeat"
            );
        }
        let mut progress = self.tick_mode_management();
        let now = tick_start;
        let queue_len = self.queue.tx_len();
        let queue_ready = queue_len > 0 && self.active_pending_blocks_len() == 0;
        if queue_ready {
            self.subsystems.propose.pacemaker.next_deadline = now;
        }
        let adaptive_progress = self.apply_adaptive_observability(now);
        let (refresh_progress, refresh_cost) = {
            let step_start = Instant::now();
            let progress = self.refresh_backpressure_state();
            (progress, step_start.elapsed())
        };
        let (committed_progress, committed_poll_cost) = {
            let step_start = Instant::now();
            let progress = self.poll_committed_blocks();
            (progress, step_start.elapsed())
        };
        let (reschedule_progress, reschedule_cost) = {
            let step_start = Instant::now();
            let progress = self.reschedule_stale_pending_blocks();
            (progress, step_start.elapsed())
        };
        let (idle_view_progress, idle_view_cost) = {
            let step_start = Instant::now();
            let progress = self.force_view_change_if_idle(now);
            (progress, step_start.elapsed())
        };
        let mut commit_pipeline_cost = Duration::ZERO;
        let mut propose_cost = Duration::ZERO;
        progress |= adaptive_progress
            || refresh_progress
            || committed_progress
            || reschedule_progress
            || idle_view_progress;
        if should_run_commit_pipeline_on_tick(self.active_pending_blocks_len())
            || self.subsystems.commit.inflight.is_some()
        {
            let pipeline_start = Instant::now();
            self.process_commit_candidates_with_trigger(CommitPipelineTrigger::Tick);
            commit_pipeline_cost = pipeline_start.elapsed();
            progress = true;
        }
        if queue_ready && !self.should_defer_proposal() && self.subsystems.commit.inflight.is_none()
        {
            let propose_start = Instant::now();
            if self.on_pacemaker_propose_ready(now) {
                progress = true;
            }
            propose_cost = propose_cost.saturating_add(propose_start.elapsed());
        }
        // Refresh backpressure before computing the snapshot we pass to the pacemaker so
        // both the gating decision and the tracker see the same state.
        let should_defer = self.should_defer_proposal();
        let state = self.subsystems.propose.backpressure_gate.state();
        let pacemaker_eval_start = Instant::now();
        let (log_initial_deferral, log_fire_deferral, should_attempt_proposal) =
            Self::evaluate_pacemaker(
                &mut self.subsystems.propose.pacemaker,
                &mut self.subsystems.propose.pacemaker_backpressure,
                state,
                now,
                should_defer,
            );
        let pacemaker_eval_cost = pacemaker_eval_start.elapsed();
        if log_initial_deferral || log_fire_deferral {
            self.on_pacemaker_backpressure_deferral(now, state);
        }
        if should_attempt_proposal && self.subsystems.commit.inflight.is_none() {
            let propose_start = Instant::now();
            if self.on_pacemaker_propose_ready(now) {
                progress = true;
            }
            propose_cost = propose_cost.saturating_add(propose_start.elapsed());
        }
        let tick_cost = tick_start.elapsed();
        let timing = self.tick_timing.observe(tick_start, tick_cost);
        if timing.log_gap || timing.log_cost {
            let pacemaker_remaining = self
                .subsystems
                .propose
                .pacemaker
                .next_deadline
                .checked_duration_since(tick_start)
                .unwrap_or_default();
            let since_last_attempt = self
                .subsystems
                .propose
                .last_pacemaker_attempt
                .map(|last| tick_start.saturating_duration_since(last));
            let since_last_success = self
                .subsystems
                .propose
                .last_successful_proposal
                .map(|last| tick_start.saturating_duration_since(last));
            iroha_logger::warn!(
                since_last_ms = timing.since_last_tick.as_millis(),
                tick_cost_ms = timing.tick_cost.as_millis(),
                pacemaker_remaining_ms = pacemaker_remaining.as_millis(),
                last_pacemaker_attempt_ms = since_last_attempt.map(|d| d.as_millis()),
                last_proposal_ms = since_last_success.map(|d| d.as_millis()),
                refresh_ms = refresh_cost.as_millis(),
                committed_poll_ms = committed_poll_cost.as_millis(),
                reschedule_ms = reschedule_cost.as_millis(),
                idle_view_ms = idle_view_cost.as_millis(),
                commit_pipeline_ms = commit_pipeline_cost.as_millis(),
                pacemaker_eval_ms = pacemaker_eval_cost.as_millis(),
                propose_ms = propose_cost.as_millis(),
                progress,
                queue_ready,
                backpressure_saturated = state.is_saturated(),
                queue_len,
                pending_blocks = self.pending.pending_blocks.len(),
                tick = self.tick_counter,
                "sumeragi tick loop lagging; pacemaker may be stalling"
            );
        }
        progress
    }

    fn validate_block_against_hint(
        block_hash: &HashOf<BlockHeader>,
        header: &BlockHeader,
        hint: &super::message::ProposalHint,
    ) -> Result<(), HintMismatch> {
        if &hint.block_hash != block_hash {
            return Err(HintMismatch::BlockHash);
        }
        let block_height = header.height().get();
        if hint.height != block_height {
            return Err(HintMismatch::Height);
        }
        let block_view = u64::from(header.view_change_index());
        if hint.view != block_view {
            return Err(HintMismatch::View);
        }
        if hint.highest_cert.height.saturating_add(1) != block_height {
            return Err(HintMismatch::HighestQcHeight);
        }
        if let Some(parent_hash) = header.prev_block_hash() {
            if hint.highest_cert.subject_block_hash != parent_hash {
                return Err(HintMismatch::HighestQcParentHash);
            }
        }
        Ok(())
    }

    fn record_phase_sample(&mut self, phase: PipelinePhase, height: u64, view: u64) {
        if let Some(current_height) = self.phase_tracker.round_height {
            if height < current_height {
                return;
            }
            if height == current_height && view < self.phase_tracker.round_view {
                return;
            }
        }
        let now = Instant::now();
        let duration = if let Some(duration) = self.phase_tracker.record(phase, height, view, now) {
            Some(duration)
        } else {
            self.phase_tracker.start_new_round(height, now);
            self.phase_tracker.record(phase, height, view, now)
        };
        if let Some(duration) = duration {
            self.update_phase_metrics(phase, duration);
        }
    }

    fn update_phase_metrics(&mut self, phase: PipelinePhase, duration: Duration) {
        let ms = u64::try_from(duration.as_millis().min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
        let ema_ms = round_duration_ms(
            self.phase_ema
                .update(phase, duration.as_secs_f64() * 1_000.0),
        );

        match phase {
            PipelinePhase::Propose => {
                super::status::set_phase_propose_ms(ms);
                super::status::set_phase_propose_ema_ms(ema_ms);
            }
            PipelinePhase::CollectDa => {
                super::status::set_phase_collect_da_ms(ms);
                super::status::set_phase_collect_da_ema_ms(ema_ms);
            }
            PipelinePhase::CollectPrepare => {
                super::status::set_phase_collect_prevote_ms(ms);
                super::status::set_phase_collect_prevote_ema_ms(ema_ms);
            }
            PipelinePhase::CollectCommit => {
                super::status::set_phase_collect_precommit_ms(ms);
                super::status::set_phase_collect_precommit_ema_ms(ema_ms);
            }
            PipelinePhase::CollectExec => {
                super::status::set_phase_collect_exec_ms(ms);
                super::status::set_phase_collect_exec_ema_ms(ema_ms);
            }
            PipelinePhase::CollectWitness => {
                super::status::set_phase_collect_witness_ms(ms);
                super::status::set_phase_collect_witness_ema_ms(ema_ms);
            }
            PipelinePhase::Commit => {
                super::status::set_phase_commit_ms(ms);
                super::status::set_phase_commit_ema_ms(ema_ms);
            }
        }

        let total_ema_ms = u64::try_from(
            self.phase_ema
                .total_duration()
                .as_millis()
                .min(u128::from(u64::MAX)),
        )
        .unwrap_or(u64::MAX);
        #[cfg(feature = "telemetry")]
        #[allow(clippy::cast_precision_loss)]
        self.telemetry
            .set_phase_latency_total_ema_ms(total_ema_ms as f64);

        super::status::set_phase_pipeline_total_ema_ms(total_ema_ms);
    }

    #[cfg(feature = "telemetry")]
    fn note_message_received(&self, msg: &BlockMessage) {
        self.telemetry.note_consensus_message_received(msg);
    }

    #[cfg(not(feature = "telemetry"))]
    fn note_message_received(&self, _msg: &BlockMessage) {}

    pub(super) fn on_block_message(&mut self, msg: BlockMessage) -> Result<()> {
        debug!(message=%Self::block_message_kind(&msg), "received consensus block message");
        self.note_message_received(&msg);
        match msg {
            BlockMessage::ConsensusParams(advert) => self.handle_consensus_params(advert),
            BlockMessage::BlockCreated(block) => self.handle_block_created(block),
            BlockMessage::BlockSyncUpdate(update) => self.handle_block_sync_update(update),
            BlockMessage::ProposalHint(hint) => self.handle_proposal_hint(hint),
            BlockMessage::CommitVote(vote) => {
                info!(
                    phase = ?vote.phase,
                    height = vote.height,
                    view = vote.view,
                    epoch = vote.epoch,
                    signer = vote.signer,
                    block_hash = %vote.block_hash,
                    "processing incoming commit vote"
                );
                self.handle_vote(vote);
                Ok(())
            }
            BlockMessage::CommitCertificate(cert) => self.handle_qc(cert),
            BlockMessage::VrfCommit(commit) => self.handle_vrf_commit(commit),
            BlockMessage::VrfReveal(reveal) => self.handle_vrf_reveal(reveal),
            BlockMessage::ExecVote(vote) => {
                self.handle_exec_vote(vote);
                Ok(())
            }
            BlockMessage::ExecutionQC(qc) => self.handle_execution_qc(qc),
            BlockMessage::ExecWitness(witness) => {
                self.handle_exec_witness(witness);
                Ok(())
            }
            BlockMessage::RbcInit(init) => self.handle_rbc_init(init),
            BlockMessage::RbcChunk(chunk) => self.handle_rbc_chunk(chunk),
            BlockMessage::RbcReady(ready) => self.handle_rbc_ready(ready),
            BlockMessage::RbcDeliver(deliver) => self.handle_rbc_deliver(deliver),
            BlockMessage::FetchPendingBlock(request) => self.handle_fetch_pending_block(request),
            BlockMessage::Proposal(proposal) => self.handle_proposal(proposal),
        }
    }

    pub(super) fn on_lane_relay_message(&mut self, message: super::LaneRelayMessage) -> Result<()> {
        match message {
            super::LaneRelayMessage::Envelope(envelope) => {
                self.on_lane_relay(envelope)?;
                self.handle_merge_entry_candidates()
            }
            super::LaneRelayMessage::MergeSignature(signature) => {
                self.on_merge_signature(signature)?;
                self.handle_merge_entry_candidates()
            }
        }
    }

    fn on_merge_signature(&mut self, signature: MergeCommitteeSignature) -> Result<()> {
        let commit_topology = self.state.commit_topology.view();
        let roster_len = commit_topology.len();
        if roster_len == 0 {
            iroha_logger::warn!(
                epoch = signature.epoch_id,
                "merge signature received without a commit roster; ignoring"
            );
            return Ok(());
        }
        if signature.view != 0 {
            iroha_logger::warn!(
                epoch = signature.epoch_id,
                view = signature.view,
                "merge signature view not supported; ignoring"
            );
            return Ok(());
        }
        let signer_idx = match usize::try_from(signature.signer) {
            Ok(idx) => idx,
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    signer = signature.signer,
                    "merge signature signer index exceeds usize"
                );
                return Ok(());
            }
        };
        if signer_idx >= roster_len {
            iroha_logger::warn!(
                signer = signer_idx,
                roster_len,
                epoch = signature.epoch_id,
                "merge signature signer out of bounds; ignoring"
            );
            return Ok(());
        }
        if signature.bls_sig.is_empty() {
            iroha_logger::warn!(
                signer = signer_idx,
                epoch = signature.epoch_id,
                "merge signature missing BLS payload; ignoring"
            );
            return Ok(());
        }

        let peer = &commit_topology[signer_idx];
        let bls_signature = Signature::from_bytes(&signature.bls_sig);
        if let Err(err) = bls_signature.verify(peer.public_key(), signature.message_digest.as_ref())
        {
            iroha_logger::warn!(
                ?err,
                signer = signer_idx,
                epoch = signature.epoch_id,
                "merge signature verification failed"
            );
            return Ok(());
        }

        let key = MergeCommitteeKey {
            epoch_id: signature.epoch_id,
            view: signature.view,
            message_digest: signature.message_digest,
        };
        let entry = self
            .subsystems
            .merge
            .committee
            .pending
            .entry(key)
            .or_insert_with(|| MergeCommitteeEntry {
                candidate: None,
                signatures: BTreeMap::new(),
            });

        if let Some(candidate) = entry.candidate.as_ref() {
            let expected =
                crate::merge::merge_qc_message_digest(&self.chain_id, signature.view, candidate);
            if expected != signature.message_digest {
                iroha_logger::warn!(
                    epoch = signature.epoch_id,
                    signer = signer_idx,
                    expected = %expected,
                    actual = %signature.message_digest,
                    "merge signature digest mismatch; ignoring"
                );
                return Ok(());
            }
        }

        match entry.signatures.entry(signature.signer) {
            Entry::Vacant(slot) => {
                slot.insert(signature.bls_sig);
            }
            Entry::Occupied(_) => {
                return Ok(());
            }
        }

        Ok(())
    }

    fn build_merge_signature(
        &self,
        signer: ValidatorIndex,
        view: u64,
        candidate: &crate::merge::MergeLedgerCandidate,
        message_digest: Hash,
    ) -> MergeCommitteeSignature {
        let signature = Signature::new(
            self.common_config.key_pair.private_key(),
            message_digest.as_ref(),
        );
        MergeCommitteeSignature {
            epoch_id: candidate.epoch_id,
            view,
            signer,
            message_digest,
            bls_sig: signature.payload().to_vec(),
        }
    }

    fn handle_merge_entry_candidates(&mut self) -> Result<()> {
        let candidates = self.state.merge_entry_candidates_from_lane_relays();
        if candidates.is_empty() {
            return Ok(());
        }

        let commit_topology = self.state.commit_topology.view();
        if commit_topology.is_empty() {
            iroha_logger::warn!(
                pending = candidates.len(),
                "merge entry candidates available without commit roster; deferring"
            );
            return Ok(());
        }

        let roster_len = commit_topology.len();
        let topology = super::network_topology::Topology::new(commit_topology.clone());
        let local_index = self.local_validator_index_for_topology(&topology);
        let mut ordered_keys = Vec::with_capacity(candidates.len());

        for candidate in candidates {
            // TODO: Thread merge-committee view changes once merge view-change support is implemented.
            let view = 0;
            let message_digest =
                crate::merge::merge_qc_message_digest(&self.chain_id, view, &candidate);
            let key = MergeCommitteeKey {
                epoch_id: candidate.epoch_id,
                view,
                message_digest,
            };
            ordered_keys.push(key);

            let local_signature = local_index
                .map(|signer| self.build_merge_signature(signer, view, &candidate, message_digest));

            let mut broadcast_signature: Option<MergeCommitteeSignature> = None;
            {
                let entry = self
                    .subsystems
                    .merge
                    .committee
                    .pending
                    .entry(key)
                    .or_insert_with(|| MergeCommitteeEntry {
                        candidate: Some(candidate.clone()),
                        signatures: BTreeMap::new(),
                    });

                let candidate_matches = entry
                    .candidate
                    .as_ref()
                    .is_none_or(|existing| existing == &candidate);
                if candidate_matches {
                    if entry.candidate.is_none() {
                        entry.candidate = Some(candidate.clone());
                    }

                    if let Some(signature) = local_signature {
                        if let Entry::Vacant(slot) = entry.signatures.entry(signature.signer) {
                            slot.insert(signature.bls_sig.clone());
                            broadcast_signature = Some(signature);
                        }
                    }
                } else {
                    iroha_logger::warn!(
                        epoch = candidate.epoch_id,
                        "merge candidate mismatch; skipping local signature"
                    );
                }
            }

            if let Some(signature) = broadcast_signature {
                self.network.broadcast(Broadcast {
                    data: NetworkMessage::MergeCommitteeSignature(Box::new(signature)),
                    priority: Priority::High,
                });
            }
        }

        self.try_commit_merge_candidates(&ordered_keys, roster_len);
        Ok(())
    }

    fn try_commit_merge_candidates(
        &mut self,
        ordered_keys: &[MergeCommitteeKey],
        roster_len: usize,
    ) {
        if roster_len == 0 {
            return;
        }
        let required = crate::sumeragi::network_topology::commit_quorum_from_len(roster_len);

        for key in ordered_keys {
            let (candidate, qc) = {
                let Some(entry) = self.subsystems.merge.committee.pending.get(key) else {
                    continue;
                };
                let Some(candidate) = entry.candidate.as_ref() else {
                    break;
                };

                let mut signers = BTreeSet::new();
                for signer in entry.signatures.keys().copied() {
                    let Ok(idx) = usize::try_from(signer) else {
                        continue;
                    };
                    if idx < roster_len {
                        signers.insert(signer);
                    }
                }
                if signers.len() < required {
                    break;
                }

                let mut signature_refs = Vec::with_capacity(signers.len());
                for signer in &signers {
                    if let Some(sig) = entry.signatures.get(signer) {
                        signature_refs.push(sig.as_slice());
                    }
                }
                let aggregate_signature =
                    match iroha_crypto::bls_normal_aggregate_signatures(&signature_refs) {
                        Ok(sig) => sig,
                        Err(err) => {
                            iroha_logger::warn!(
                                ?err,
                                epoch = key.epoch_id,
                                "failed to aggregate merge signatures"
                            );
                            break;
                        }
                    };
                let signers_bitmap = build_signers_bitmap(&signers, roster_len);
                let qc = MergeQuorumCertificate::new(
                    key.view,
                    key.epoch_id,
                    signers_bitmap,
                    aggregate_signature,
                    key.message_digest,
                );
                (candidate.clone(), qc)
            };

            match self.state.commit_merge_entry(candidate.into_entry(qc)) {
                Ok(_) => {
                    self.subsystems.merge.committee.pending.remove(key);
                }
                Err(err) => {
                    iroha_logger::warn!(?err, epoch = key.epoch_id, "failed to commit merge entry");
                    break;
                }
            }
        }
    }

    pub(super) fn on_lane_relay(&mut self, envelope: LaneRelayEnvelope) -> Result<()> {
        match self.state.record_lane_relay(&envelope) {
            Ok(insert) => {
                if matches!(insert, crate::state::LaneRelayInsert::Duplicate) {
                    iroha_logger::debug!(
                        lane_id = %envelope.lane_id,
                        dataspace_id = %envelope.dataspace_id,
                        block_height = envelope.block_height,
                        "duplicate lane relay received; ignoring"
                    );
                }
            }
            Err(err) => {
                iroha_logger::warn!(
                    lane_id = %envelope.lane_id,
                    dataspace_id = %envelope.dataspace_id,
                    block_height = envelope.block_height,
                    error_kind = err.as_label(),
                    error = %err,
                    "dropping invalid lane relay envelope"
                );
                // Still surface the envelope to status for operator visibility (it will be rejected).
                super::status::push_lane_relay_envelope(envelope);
            }
        }
        Ok(())
    }

    pub(super) fn on_consensus_control(&mut self, msg: ControlFlow) -> Result<()> {
        debug!(message=%Self::consensus_control_kind(&msg), "received consensus control-frame");
        match msg {
            ControlFlow::Evidence(ev) => self.handle_evidence(ev),
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    pub(super) fn on_background_request(&mut self, request: BackgroundRequest) -> Result<()> {
        self.schedule_background(request);
        Ok(())
    }

    fn schedule_background(&mut self, request: BackgroundRequest) {
        let dispatched = {
            #[cfg(feature = "telemetry")]
            {
                background::dispatch_background_request(
                    self.background_post_tx.as_ref(),
                    request,
                    &self.telemetry,
                )
            }
            #[cfg(not(feature = "telemetry"))]
            {
                background::dispatch_background_request(self.background_post_tx.as_ref(), request)
            }
        };
        let _ = dispatched;
    }

    fn block_message_kind(msg: &BlockMessage) -> &'static str {
        match msg {
            BlockMessage::BlockCreated(_) => "BlockCreated",
            BlockMessage::BlockSyncUpdate(_) => "BlockSyncUpdate",
            BlockMessage::ConsensusParams(_) => "ConsensusParams",
            BlockMessage::CommitVote(vote) => match vote.phase {
                crate::sumeragi::consensus::Phase::Prepare => "PrepareVote",
                crate::sumeragi::consensus::Phase::Commit => "CommitVote",
                crate::sumeragi::consensus::Phase::NewView => "NewViewVote",
            },
            BlockMessage::CommitCertificate(cert) => match cert.phase {
                crate::sumeragi::consensus::Phase::Prepare => "PrepareCert",
                crate::sumeragi::consensus::Phase::Commit => "CommitCert",
                crate::sumeragi::consensus::Phase::NewView => "NewViewCert",
            },
            BlockMessage::VrfCommit(_) => "VrfCommit",
            BlockMessage::VrfReveal(_) => "VrfReveal",
            BlockMessage::ExecVote(_) => "ExecVote",
            BlockMessage::ExecutionQC(_) => "ExecutionQC",
            BlockMessage::ExecWitness(_) => "ExecWitness",
            BlockMessage::RbcInit(_) => "RbcInit",
            BlockMessage::RbcChunk(_) => "RbcChunk",
            BlockMessage::RbcReady(_) => "RbcReady",
            BlockMessage::RbcDeliver(_) => "RbcDeliver",
            BlockMessage::FetchPendingBlock(_) => "FetchPendingBlock",
            BlockMessage::ProposalHint(_) => "ProposalHint",
            BlockMessage::Proposal(_) => "Proposal",
        }
    }

    fn consensus_control_kind(msg: &ControlFlow) -> &'static str {
        match msg {
            ControlFlow::Evidence(_) => "Evidence",
        }
    }

    fn mode_tag(&self) -> &'static str {
        match self.consensus_mode {
            ConsensusMode::Permissioned => PERMISSIONED_TAG,
            ConsensusMode::Npos => NPOS_TAG,
        }
    }

    fn npos_prf_seed(&self) -> Option<[u8; 32]> {
        self.npos_collectors
            .map(|cfg| cfg.seed)
            .or_else(|| self.epoch_manager.as_ref().map(EpochManager::seed))
    }

    fn local_validator_index(&self, view: &StateView<'_>) -> Option<ValidatorIndex> {
        derive_local_validator_index(
            view,
            self.common_config.trusted_peers.value(),
            self.common_config.peer.id(),
        )
    }

    fn local_validator_index_for_topology(
        &self,
        topology: &super::network_topology::Topology,
    ) -> Option<ValidatorIndex> {
        topology
            .position(self.common_config.peer.id().public_key())
            .and_then(|idx| ValidatorIndex::try_from(idx).ok())
    }

    fn build_rbc_ready(
        &self,
        key: super::rbc_store::SessionKey,
        session: &RbcSession,
    ) -> Option<RbcReady> {
        let commit_topology = self.rbc_session_roster(key);
        if commit_topology.is_empty() {
            return None;
        }
        let topology = super::network_topology::Topology::new(commit_topology);
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
        let signature_topology = topology_for_view(&topology, key.1, key.2, mode_tag, prf_seed);
        let sender = self.local_validator_index_for_topology(&signature_topology)?;

        let (block_hash, height, view_idx) = key;
        let chunk_root = session.expected_chunk_root?;
        let mut ready = RbcReady {
            block_hash,
            height,
            view: view_idx,
            epoch: session.epoch,
            chunk_root,
            sender,
            signature: Vec::new(),
        };

        let preimage = rbc_ready_preimage(&self.chain_id, mode_tag, &ready);
        let signature = Signature::new(self.common_config.key_pair.private_key(), &preimage);
        ready.signature = signature.payload().to_vec();
        Some(ready)
    }

    fn build_rbc_deliver(
        &self,
        key: super::rbc_store::SessionKey,
        session: &RbcSession,
    ) -> Option<RbcDeliver> {
        let commit_topology = self.rbc_session_roster(key);
        if commit_topology.is_empty() {
            return None;
        }
        let topology = super::network_topology::Topology::new(commit_topology);
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
        let signature_topology = topology_for_view(&topology, key.1, key.2, mode_tag, prf_seed);
        let sender = self.local_validator_index_for_topology(&signature_topology)?;

        let (block_hash, height, view_idx) = key;
        let chunk_root = session.expected_chunk_root?;
        let mut deliver = RbcDeliver {
            block_hash,
            height,
            view: view_idx,
            epoch: session.epoch,
            chunk_root,
            sender,
            signature: Vec::new(),
        };

        let preimage = rbc_deliver_preimage(&self.chain_id, mode_tag, &deliver);
        let signature = Signature::new(self.common_config.key_pair.private_key(), &preimage);
        deliver.signature = signature.payload().to_vec();
        Some(deliver)
    }

    fn rbc_deliver_quorum_with_debug(
        topology: &super::network_topology::Topology,
        force_quorum_one: bool,
    ) -> usize {
        if force_quorum_one {
            1
        } else {
            topology.min_votes_for_commit()
        }
    }

    fn rbc_deliver_quorum(&self, topology: &super::network_topology::Topology) -> usize {
        Self::rbc_deliver_quorum_with_debug(
            topology,
            self.config.debug_rbc_force_deliver_quorum_one,
        )
    }

    fn stale_view(&self, height: u64, view: u64) -> Option<u64> {
        let local_view = self.phase_tracker.current_view(height)?;
        (view < local_view).then_some(local_view)
    }

    fn drop_stale_view(&self, height: u64, view: u64, kind: &'static str) -> bool {
        let Some(local_view) = self.stale_view(height, view) else {
            return false;
        };
        debug!(
            height,
            view, local_view, kind, "dropping consensus message for stale view"
        );
        true
    }

    fn pending_rbc_caps(&self) -> (usize, usize) {
        let hard_chunk_cap = usize::try_from(RBC_MAX_TOTAL_CHUNKS).unwrap_or(usize::MAX);
        let max_chunks = self
            .config
            .rbc_pending_max_chunks
            .min(hard_chunk_cap)
            .max(1);
        let chunk_max_bytes = self.config.rbc_chunk_max_bytes.max(1);
        let hard_byte_cap = chunk_max_bytes.saturating_mul(hard_chunk_cap);
        let max_bytes = self
            .config
            .rbc_pending_max_bytes
            .min(hard_byte_cap)
            .max(chunk_max_bytes);
        (max_chunks, max_bytes)
    }

    fn rbc_message_stale(
        &self,
        block_hash: &HashOf<BlockHeader>,
        height: crate::sumeragi::consensus::Height,
    ) -> bool {
        let committed_height = {
            let view = self.state.view();
            view.height() as u64
        };
        let present_in_kura = self.kura.get_block_height_by_hash(*block_hash).is_some();
        rbc_message_committed(committed_height, height, present_in_kura)
    }

    fn rebuild_rbc_init(&self, key: super::rbc_store::SessionKey) -> Option<RbcInit> {
        let session = self.subsystems.da_rbc.rbc.sessions.get(&key)?;
        if session.is_invalid() {
            return None;
        }
        let payload_hash = session.payload_hash()?;
        let chunk_root = session
            .expected_chunk_root
            .or_else(|| session.chunk_root())?;
        Some(RbcInit {
            block_hash: key.0,
            height: key.1,
            view: key.2,
            epoch: session.epoch,
            total_chunks: session.total_chunks(),
            payload_hash,
            chunk_root,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn maybe_emit_rbc_ready(&mut self, key: super::rbc_store::SessionKey) -> Result<()> {
        let Some(mut session) = self.subsystems.da_rbc.rbc.sessions.remove(&key) else {
            return Ok(());
        };
        if session.is_invalid() {
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            return Ok(());
        }

        let result = (|| -> Result<Option<RbcReady>> {
            if session.sent_ready {
                return Ok(None);
            }

            let total = session.total_chunks();
            if total != 0 && session.received_chunks() < total {
                debug!(
                    height = key.1,
                    view = key.2,
                    received = session.received_chunks(),
                    total,
                    "deferring local RBC READY until all chunks are received"
                );
                return Ok(None);
            }

            if let Some(expected_root) = session.expected_chunk_root {
                if let Some(computed_root) = session.chunk_root() {
                    if computed_root != expected_root {
                        session.invalid = true;
                        warn!(
                            height = key.1,
                            view = key.2,
                            ?expected_root,
                            ?computed_root,
                            "RBC chunk-root mismatch detected; refusing to emit READY"
                        );
                        return Ok(None);
                    }
                }
            }

            if let Some(ready) = self.build_rbc_ready(key, &session) {
                let _ = session.record_ready(ready.sender, ready.signature.clone());
                session.sent_ready = true;
                Ok(Some(ready))
            } else {
                debug!(
                    height = key.1,
                    view = key.2,
                    "unable to build RBC READY (no validator index)"
                );
                session.sent_ready = true;
                Ok(None)
            }
        })();

        let ready_to_send = result?;
        let mismatch_detected = session.invalid && !session.sent_ready;
        self.subsystems.da_rbc.rbc.sessions.insert(key, session);
        if let Some(updated) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &updated, false);
            self.persist_rbc_session(key, &updated);
        }
        self.publish_rbc_backlog_snapshot();

        if mismatch_detected {
            return Ok(());
        }
        if let Some(ready) = ready_to_send {
            let topology_peers = self.rbc_session_roster(key);
            let local_peer_id = self.common_config.peer.id().clone();
            iroha_logger::info!(
                height = key.1,
                view = key.2,
                block = %key.0,
                sender = ready.sender,
                "sending local RBC READY to commit topology"
            );
            if self.should_fork_ready(ready.sender) {
                let forked = Self::fork_ready_message(ready.clone());
                debug!(
                    height = forked.height,
                    view = forked.view,
                    sender = forked.sender,
                    "sending conflicting RBC READY to commit topology due to debug mask"
                );
                for peer in &topology_peers {
                    if peer == &local_peer_id {
                        continue;
                    }
                    self.schedule_background(BackgroundRequest::Post {
                        peer: peer.clone(),
                        msg: BlockMessage::RbcReady(forked.clone()),
                    });
                }
            }
            for peer in &topology_peers {
                if peer == &local_peer_id {
                    continue;
                }
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: BlockMessage::RbcReady(ready.clone()),
                });
            }
        }

        self.maybe_emit_rbc_deliver(key)?;

        Ok(())
    }

    fn rbc_ready_bundle(
        key: super::rbc_store::SessionKey,
        session: &RbcSession,
    ) -> Option<Vec<RbcReady>> {
        let chunk_root = session
            .expected_chunk_root
            .or_else(|| session.chunk_root())?;
        if session.ready_signatures.is_empty() {
            return None;
        }
        let mut messages = Vec::with_capacity(session.ready_signatures.len());
        for entry in &session.ready_signatures {
            messages.push(RbcReady {
                block_hash: key.0,
                height: key.1,
                view: key.2,
                epoch: session.epoch,
                chunk_root,
                sender: entry.sender,
                signature: entry.signature.clone(),
            });
        }
        Some(messages)
    }

    fn rebroadcast_rbc_ready_set(
        &mut self,
        key: super::rbc_store::SessionKey,
        session: &RbcSession,
    ) {
        let Some(readies) = Self::rbc_ready_bundle(key, session) else {
            return;
        };
        self.rebroadcast_rbc_ready_bundle(key, readies);
    }

    fn rebroadcast_rbc_ready_bundle(
        &mut self,
        key: super::rbc_store::SessionKey,
        readies: Vec<RbcReady>,
    ) {
        if readies.is_empty() {
            return;
        }
        let topology_peers = self.rbc_session_roster(key);
        let local_peer_id = self.common_config.peer.id().clone();
        if topology_peers.is_empty() {
            return;
        }
        let now = Instant::now();
        let cooldown = self.rebroadcast_cooldown();
        if let Some(last) = self
            .subsystems
            .da_rbc
            .rbc
            .ready_rebroadcast_last_sent
            .get(&key)
        {
            if now.saturating_duration_since(*last) < cooldown {
                return;
            }
        }
        for ready in readies {
            for peer in &topology_peers {
                if peer == &local_peer_id {
                    continue;
                }
                self.schedule_background(BackgroundRequest::Post {
                    peer: peer.clone(),
                    msg: BlockMessage::RbcReady(ready.clone()),
                });
            }
        }
        self.subsystems
            .da_rbc
            .rbc
            .ready_rebroadcast_last_sent
            .insert(key, now);
    }

    fn rbc_payload_bundle(
        key: super::rbc_store::SessionKey,
        session: &RbcSession,
    ) -> Option<(
        crate::sumeragi::consensus::RbcInit,
        Vec<crate::sumeragi::consensus::RbcChunk>,
    )> {
        if session.total_chunks() == 0 {
            return None;
        }
        let payload_hash = session.payload_hash()?;
        let chunk_root = session
            .expected_chunk_root
            .or_else(|| session.chunk_root())?;
        let mut chunks = Vec::with_capacity(session.total_chunks() as usize);
        for idx in 0..session.total_chunks() {
            if let Some(bytes) = session.chunk_bytes(idx) {
                chunks.push(crate::sumeragi::consensus::RbcChunk {
                    block_hash: key.0,
                    height: key.1,
                    view: key.2,
                    epoch: session.epoch,
                    idx,
                    bytes: bytes.to_vec(),
                });
            }
        }
        if chunks.is_empty() {
            return None;
        }
        let init = crate::sumeragi::consensus::RbcInit {
            block_hash: key.0,
            height: key.1,
            view: key.2,
            epoch: session.epoch,
            total_chunks: session.total_chunks(),
            payload_hash,
            chunk_root,
        };
        Some((init, chunks))
    }

    fn rebroadcast_rbc_payload_bundle(
        &mut self,
        key: super::rbc_store::SessionKey,
        init: crate::sumeragi::consensus::RbcInit,
        chunks: Vec<crate::sumeragi::consensus::RbcChunk>,
        ready_count: usize,
    ) {
        let now = Instant::now();
        let cooldown = self.payload_rebroadcast_cooldown();
        if let Some(last) = self
            .subsystems
            .da_rbc
            .rbc
            .payload_rebroadcast_last_sent
            .get(&key)
        {
            if now.saturating_duration_since(*last) < cooldown {
                return;
            }
        }
        if chunks.is_empty() {
            return;
        }
        for chunk in &chunks {
            self.schedule_rbc_chunk_broadcast(chunk.clone());
        }
        info!(
            height = key.1,
            view = key.2,
            block = %key.0,
            ready = ready_count,
            chunk_count = chunks.len(),
            "rebroadcasting RBC INIT and cached chunks while awaiting READY quorum"
        );
        let topology_peers = self.rbc_session_roster(key);
        let local_peer_id = self.common_config.peer.id().clone();
        for peer in &topology_peers {
            if peer == &local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessage::RbcInit(init),
            });
        }
        self.subsystems
            .da_rbc
            .rbc
            .payload_rebroadcast_last_sent
            .insert(key, now);
    }

    fn rebroadcast_rbc_payload(&mut self, key: super::rbc_store::SessionKey, session: &RbcSession) {
        let Some((init, chunks)) = Self::rbc_payload_bundle(key, session) else {
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                "skipping RBC payload rebroadcast: payload missing"
            );
            return;
        };
        self.rebroadcast_rbc_payload_bundle(key, init, chunks, session.ready_signatures.len());
    }

    fn maybe_emit_rbc_deliver(&mut self, key: super::rbc_store::SessionKey) -> Result<()> {
        let Some(mut session) = self.subsystems.da_rbc.rbc.sessions.remove(&key) else {
            return Ok(());
        };

        if session.delivered || session.is_invalid() {
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            return Ok(());
        }

        let commit_topology = self.rbc_session_roster(key);
        if commit_topology.is_empty() {
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            return Ok(());
        }
        let topology = super::network_topology::Topology::new(commit_topology.clone());
        let required = self.rbc_deliver_quorum(&topology);
        if session.ready_signatures.len() < required {
            iroha_logger::info!(
                height = key.1,
                view = key.2,
                block = %key.0,
                ready = session.ready_signatures.len(),
                required,
                senders = ?session
                    .ready_signatures
                    .iter()
                    .map(|entry| entry.sender)
                    .collect::<Vec<_>>(),
                "deferring RBC DELIVER: READY quorum not yet satisfied"
            );
            self.rebroadcast_rbc_payload(key, &session);
            if !session.ready_signatures.is_empty() {
                self.rebroadcast_rbc_ready_set(key, &session);
            }
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            return Ok(());
        }

        let deliver = if let Some(deliver) = self.build_rbc_deliver(key, &session) {
            deliver
        } else {
            debug!(
                height = key.1,
                view = key.2,
                ready = session.ready_signatures.len(),
                required,
                "READY quorum met but local validator index unavailable; waiting for external RBC DELIVER"
            );
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            if let Some(updated) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
                self.update_rbc_status_entry(key, &updated, false);
                self.persist_rbc_session(key, &updated);
            }
            return Ok(());
        };

        let first_deliver = session.record_deliver(deliver.sender, deliver.signature.clone());
        let delivered_bytes = session.delivered_payload_bytes();
        let ready_count = session.ready_signatures.len();
        let ready_senders: Vec<_> = session
            .ready_signatures
            .iter()
            .map(|entry| entry.sender)
            .collect();
        let deliver_sender = deliver.sender;

        self.subsystems.da_rbc.rbc.sessions.insert(key, session);
        if let Some(updated) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &updated, false);
            self.persist_rbc_session(key, &updated);
        }
        let telemetry_ref = self.telemetry_handle();
        self.publish_rbc_backlog_snapshot();

        if first_deliver {
            if let Some(telemetry) = telemetry_ref {
                telemetry.inc_rbc_deliver_broadcasts();
                if let Some(bytes) = delivered_bytes {
                    telemetry.add_rbc_payload_bytes_delivered(bytes);
                }
            }
        }

        iroha_logger::info!(
            height = key.1,
            view = key.2,
            block = %key.0,
            ready = ready_count,
            required,
            deliver_sender,
            senders = ?ready_senders,
            "sending RBC DELIVER to commit topology after READY quorum"
        );
        let local_peer_id = self.common_config.peer.id().clone();
        for peer in &commit_topology {
            if peer == &local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessage::RbcDeliver(deliver.clone()),
            });
        }

        self.process_commit_candidates();

        Ok(())
    }

    fn commit_quorum_timeout(&self) -> Duration {
        let view = self.state.view();
        let params = view.world.parameters().sumeragi();
        let block_time = params.block_time();
        let commit_time = params.commit_time();
        let da_enabled = params.da_enabled();
        drop(view);

        commit_quorum_timeout_from_durations(
            block_time,
            commit_time,
            da_enabled,
            self.da_quorum_timeout_multiplier(),
        )
    }

    fn rebroadcast_cooldown(&self) -> Duration {
        let view = self.state.view();
        let block_time = view.world.parameters().sumeragi().block_time();
        drop(view);
        rebroadcast_cooldown_from_block_time(block_time)
    }

    fn payload_rebroadcast_cooldown(&self) -> Duration {
        let view = self.state.view();
        let block_time = view.world.parameters().sumeragi().block_time();
        drop(view);
        payload_rebroadcast_cooldown_from_block_time(block_time)
    }

    fn quorum_timeout(&self, da_enabled: bool) -> Duration {
        let view = self.state.view();
        let params = view.world.parameters().sumeragi();
        let block_time = params.block_time();
        let commit_time = params.commit_time();
        drop(view);

        commit_quorum_timeout_from_durations(
            block_time,
            commit_time,
            da_enabled,
            self.da_quorum_timeout_multiplier(),
        )
    }

    fn da_quorum_timeout_multiplier(&self) -> u32 {
        self.config.da_quorum_timeout_multiplier.max(1)
    }

    fn availability_timeout(&self, quorum_timeout: Duration, da_enabled: bool) -> Duration {
        availability_timeout_from_quorum(
            quorum_timeout,
            da_enabled,
            self.config.da_availability_timeout_multiplier.max(1),
            self.config.da_availability_timeout_floor,
        )
    }

    fn force_view_change_if_idle(&mut self, now: Instant) -> bool {
        if self.has_active_pending_blocks() {
            return false;
        }

        let committed_qc = self.latest_committed_qc();
        let committed_height = committed_qc.as_ref().map_or_else(
            || {
                let view_snapshot = self.state.view();
                view_snapshot.height() as u64
            },
            |qc| qc.height,
        );
        let height = active_round_height(self.highest_qc, committed_qc, committed_height);

        let current_view = self.phase_tracker.current_view(height).unwrap_or(0);

        let age = if let Some(age) = self.phase_tracker.view_age(height, now) {
            age
        } else {
            // Seed view tracking if we never observed a view-change for this height yet.
            self.phase_tracker.start_new_round(height, now);
            if let Some(age) = self.phase_tracker.view_age(height, now) {
                age
            } else {
                return false;
            }
        };
        let proposal_seen = self
            .subsystems
            .propose
            .proposals_seen
            .contains(&(height, current_view));
        let timeout = idle_view_timeout(
            proposal_seen,
            self.commit_quorum_timeout(),
            self.subsystems.propose.pacemaker.propose_interval,
        );
        if !idle_round_timed_out(true, age, timeout) {
            return false;
        }

        let next_view = current_view.saturating_add(1);
        warn!(
            height,
            view = current_view,
            next_view,
            age_ms = age.as_millis(),
            timeout_ms = timeout.as_millis(),
            proposal_seen,
            "no proposal observed before cutoff; rotating leader via view change"
        );
        self.trigger_view_change_with_cause(height, current_view, ViewChangeCause::MissingQc);
        true
    }

    #[allow(clippy::too_many_lines)]
    fn empty_child_fallback(&self, now: Instant) -> Option<EmptyChildContext> {
        let lock = self.locked_qc?;
        if lock.phase != crate::sumeragi::consensus::Phase::Commit {
            return None;
        }
        if self.find_child_qc_extending_lock(lock).is_some() {
            return None;
        }
        let (committed_height, block_time) = {
            let view = self.state.view();
            (
                view.height() as u64,
                view.world().parameters().sumeragi().block_time(),
            )
        };
        let child_height = lock.height.saturating_add(1);
        if child_height <= committed_height {
            return None;
        }
        if self
            .pending
            .pending_blocks
            .values()
            .any(|candidate| !candidate.aborted && candidate.height == child_height)
        {
            return None;
        }
        let pending = self.pending.pending_blocks.get(&lock.subject_block_hash);
        let (parent_payload_hash, pending_age, backoff) = if let Some(pending) = pending {
            let pending_age = now.saturating_duration_since(pending.inserted_at);
            (
                pending.payload_hash,
                pending_age,
                self.commit_quorum_timeout(),
            )
        } else {
            // Fall back to the committed parent payload when the pending entry was already pruned.
            let parent_payload_hash = {
                let height_usize = usize::try_from(lock.height).ok()?;
                let nz_height = NonZeroUsize::new(height_usize)?;
                let block = self.kura.get_block(nz_height)?;
                if block.hash() != lock.subject_block_hash {
                    warn!(
                        expected = %lock.subject_block_hash,
                        observed = %block.hash(),
                        height = lock.height,
                        "locked QC does not match committed block hash"
                    );
                    return None;
                }
                Hash::new(block_payload_bytes(&block))
            };
            let backoff = block_time.max(Duration::from_millis(1));
            let pending_age = self
                .subsystems
                .propose
                .last_successful_proposal
                .map(|ts| now.saturating_duration_since(ts))
                .or_else(|| self.phase_tracker.view_age(child_height, now))
                .unwrap_or(backoff);
            (parent_payload_hash, pending_age, backoff)
        };
        if child_height == 2 && self.queue.tx_len() == 0 {
            // Avoid immediately emitting an empty post-genesis block; give clients time to
            // submit their first transactions before falling back to an empty child.
            const EMPTY_CHILD_STARTUP_GRACE: Duration = Duration::from_secs(10);
            if pending_age < EMPTY_CHILD_STARTUP_GRACE {
                return None;
            }
        }
        if self
            .subsystems
            .propose
            .proposal_cache
            .proposals
            .keys()
            .any(|(height, _)| *height == child_height)
        {
            return None;
        }
        if self
            .subsystems
            .propose
            .proposal_cache
            .hints
            .keys()
            .any(|(height, _)| *height == child_height)
        {
            return None;
        }

        if backoff != Duration::ZERO && pending_age < backoff {
            return None;
        }
        if !empty_child_backoff_allows(
            self.subsystems.propose.last_empty_child_attempt,
            parent_payload_hash,
            now,
            backoff,
        ) {
            return None;
        }

        let highest_qc = self
            .highest_qc
            .filter(|candidate| {
                candidate.phase == crate::sumeragi::consensus::Phase::Commit
                    && qc_extends_locked_with_lookup(lock, *candidate, |hash, height| {
                        self.parent_hash_for(hash, height)
                    })
            })
            .unwrap_or(lock);
        let (target_height, target_view) = new_view_target(highest_qc, Some(child_height), None);

        Some(EmptyChildContext {
            target_height,
            target_view,
            highest_qc,
            parent_payload_hash,
        })
    }

    fn record_empty_child_attempt(&mut self, parent_payload_hash: Hash, now: Instant) {
        self.subsystems.propose.last_empty_child_attempt = Some((parent_payload_hash, now));
    }

    fn purge_rbc_state(
        &mut self,
        key: super::rbc_store::SessionKey,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) {
        self.subsystems.da_rbc.rbc.sessions.remove(&key);
        self.clear_rbc_session_roster(&key);
        self.subsystems.da_rbc.rbc.status_handle.remove(&key);
        self.subsystems.da_rbc.rbc.pending.remove(&key);
        self.subsystems
            .da_rbc
            .rbc
            .payload_rebroadcast_last_sent
            .remove(&key);
        self.subsystems
            .da_rbc
            .rbc
            .ready_rebroadcast_last_sent
            .remove(&key);
        self.subsystems
            .da_rbc
            .rbc
            .persisted_full_sessions
            .remove(&key);
        if self.ensure_rbc_chunk_store() {
            if let Some(store) = self.subsystems.da_rbc.rbc.chunk_store.as_ref() {
                if let Err(err) = store.remove(&key) {
                    warn!(
                        ?err,
                        ?block_hash,
                        height,
                        view,
                        "failed to purge persisted RBC session"
                    );
                }
            }
        }

        self.publish_rbc_backlog_snapshot();
    }

    fn trigger_view_change_after_validation_reject(
        &mut self,
        height: u64,
        view: u64,
        block_hash: HashOf<BlockHeader>,
    ) {
        let latest_committed = self.latest_committed_qc();
        let (new_locked, new_highest) = realign_qcs_after_failed_commit(
            self.locked_qc,
            self.highest_qc,
            block_hash,
            latest_committed,
        );
        if new_locked != self.locked_qc || new_highest != self.highest_qc {
            debug!(
                height,
                view,
                block = %block_hash,
                locked_height = new_locked.map(|qc| qc.height),
                highest_height = new_highest.map(|qc| qc.height),
                "realigning Highest/Locked QC after validation rejection"
            );
            self.locked_qc = new_locked;
            self.highest_qc = new_highest;
        }
        info!(
            height,
            view,
            block = %block_hash,
            "triggering view change after validation rejection"
        );
        self.trigger_view_change_with_cause(height, view, ViewChangeCause::ValidationReject);
    }

    fn trigger_view_change_after_commit_failure(&mut self, height: u64, view: u64) {
        info!(
            height,
            view, "triggering view change after commit failure with QC quorum"
        );
        self.trigger_view_change_with_cause(height, view, ViewChangeCause::CommitFailure);
    }

    fn trigger_view_change_with_cause(&mut self, height: u64, view: u64, cause: ViewChangeCause) {
        if let Some(current_height) = self.phase_tracker.round_height {
            if height < current_height {
                debug!(
                    height,
                    view, current_height, "dropping view-change trigger for stale height"
                );
                return;
            }
        }
        let now = Instant::now();
        let base_view = self
            .phase_tracker
            .current_view(height)
            .map_or(view, |current| current.max(view));
        if base_view != view {
            debug!(
                height,
                view, base_view, "upgrading view-change trigger from stale view"
            );
        }
        self.assert_proposal_seen(height, base_view, cause.as_str());
        let next_view = bump_view_after_quorum_timeout(
            &mut self.phase_tracker,
            &mut self.subsystems.propose.pacemaker,
            &mut self.subsystems.propose.new_view_tracker,
            height,
            base_view,
            now,
        );
        self.subsystems.propose.forced_view_after_timeout = Some((height, next_view));
        record_view_change_cause_with_telemetry(cause, self.telemetry_handle());
        let committed_qc = self.latest_committed_qc();
        if let Some(mut highest_qc) = self.highest_qc.or(committed_qc) {
            if highest_qc.phase != crate::sumeragi::consensus::Phase::Commit {
                if let Some(committed) = committed_qc {
                    highest_qc = committed;
                } else {
                    debug!(
                        height,
                        view = next_view,
                        phase = ?highest_qc.phase,
                        "skipping NEW_VIEW vote: highest certificate is not commit"
                    );
                }
            }
            if highest_qc.phase == crate::sumeragi::consensus::Phase::Commit
                && height == highest_qc.height.saturating_add(1)
            {
                let (consensus_mode, _, _) = self.consensus_context_for_height(height);
                let roster = self.roster_for_new_view_with_mode(
                    highest_qc.subject_block_hash,
                    height,
                    next_view,
                    consensus_mode,
                );
                let topology = super::network_topology::Topology::new(roster);
                self.emit_new_view_vote(height, next_view, highest_qc, &topology);
            } else {
                debug!(
                    height,
                    view = next_view,
                    highest_height = highest_qc.height,
                    highest_view = highest_qc.view,
                    "skipping NEW_VIEW vote: highest certificate does not match target height"
                );
            }
        }
        self.rebroadcast_highest_pending_block(now);
    }
}

fn idle_round_timed_out(pending_empty: bool, age: Duration, timeout: Duration) -> bool {
    pending_empty && timeout != Duration::ZERO && age >= timeout
}

fn idle_view_timeout(
    proposal_seen: bool,
    commit_timeout: Duration,
    propose_interval: Duration,
) -> Duration {
    if proposal_seen {
        commit_timeout
    } else {
        let grace = propose_interval.saturating_mul(4);
        commit_timeout.saturating_add(grace)
    }
}

fn saturating_mul_duration(duration: Duration, mul: u32) -> Duration {
    let millis = duration.as_millis();
    let scaled = millis.saturating_mul(u128::from(mul));
    let capped = scaled.min(u128::from(u64::MAX));
    Duration::from_millis(u64::try_from(capped).unwrap_or(u64::MAX))
}

fn rbc_message_committed(
    committed_height: u64,
    message_height: u64,
    block_present_in_kura: bool,
) -> bool {
    message_height <= committed_height || block_present_in_kura
}

type RbcLaneTotals = BTreeMap<LaneId, (u64, u64, u64, u64)>;
type RbcDataspaceTotals = BTreeMap<(LaneId, DataSpaceId), (u64, u64, u64, u64)>;

fn drain_rbc_state_for_block(
    block_hash: HashOf<BlockHeader>,
    rbc_sessions: &mut BTreeMap<super::rbc_store::SessionKey, RbcSession>,
    pending_rbc: &mut BTreeMap<super::rbc_store::SessionKey, PendingRbcMessages>,
    rbc_session_rosters: &mut BTreeMap<super::rbc_store::SessionKey, Vec<PeerId>>,
    rbc_status_handle: &rbc_status::Handle,
    chunk_store: Option<&super::rbc_store::ChunkStore>,
) -> (RbcLaneTotals, RbcDataspaceTotals) {
    let keys: Vec<_> = rbc_sessions
        .keys()
        .filter(|(hash, _, _)| *hash == block_hash)
        .copied()
        .collect();

    for key in &keys {
        pending_rbc.remove(key);
        rbc_session_rosters.remove(key);
        if let Some(store) = chunk_store {
            if let Err(err) = store.remove(key) {
                warn!(
                    ?err,
                    block = %key.0,
                    height = key.1,
                    view = key.2,
                    "failed to purge persisted RBC session"
                );
            }
        }
    }

    let mut lane_totals: RbcLaneTotals = BTreeMap::new();
    let mut dataspace_totals: RbcDataspaceTotals = BTreeMap::new();

    for key in keys {
        if let Some(session) = rbc_sessions.remove(&key) {
            let ready_count = u64::try_from(session.ready_signatures.len()).unwrap_or(u64::MAX);
            rbc_status_handle.update(
                rbc_status::Summary {
                    block_hash: key.0,
                    height: key.1,
                    view: key.2,
                    total_chunks: session.total_chunks(),
                    received_chunks: session.received_chunks(),
                    ready_count,
                    delivered: session.delivered,
                    payload_hash: session.payload_hash(),
                    recovered_from_disk: session.recovered_from_disk(),
                    invalid: session.is_invalid(),
                    lane_backlog: session.lane_backlog_entries(),
                    dataspace_backlog: session.dataspace_backlog_entries(),
                },
                SystemTime::now(),
            );
            for alloc in session.lane_allocations {
                let entry = lane_totals.entry(alloc.lane_id).or_insert((0, 0, 0, 0));
                entry.0 = entry.0.saturating_add(alloc.tx_count);
                entry.1 = entry.1.saturating_add(u64::from(alloc.total_chunks));
                entry.2 = entry.2.saturating_add(alloc.rbc_bytes_total);
                entry.3 = entry.3.saturating_add(alloc.teu_total);
            }
            for alloc in session.dataspace_allocations {
                let entry = dataspace_totals
                    .entry((alloc.lane_id, alloc.dataspace_id))
                    .or_insert((0, 0, 0, 0));
                entry.0 = entry.0.saturating_add(alloc.tx_count);
                entry.1 = entry.1.saturating_add(u64::from(alloc.total_chunks));
                entry.2 = entry.2.saturating_add(alloc.rbc_bytes_total);
                entry.3 = entry.3.saturating_add(alloc.teu_total);
            }
        }
    }

    (lane_totals, dataspace_totals)
}

/// Derive the commit quorum timeout from the configured block time and commit timeout seed.
#[cfg(test)]
fn commit_quorum_timeout_for_config(config: &SumeragiConfig) -> Duration {
    commit_quorum_timeout_from_durations(
        config.npos.block_time,
        config.npos.timeouts.commit,
        config.da_enabled,
        config.da_quorum_timeout_multiplier,
    )
}

/// Derive the commit quorum timeout from the on-chain sumeragi parameters.
#[cfg(test)]
fn commit_quorum_timeout_for_params(
    params: &iroha_data_model::parameter::system::SumeragiParameters,
) -> Duration {
    commit_quorum_timeout_from_durations(
        params.block_time(),
        params.commit_time(),
        params.da_enabled(),
        iroha_config::parameters::defaults::sumeragi::DA_QUORUM_TIMEOUT_MULTIPLIER,
    )
}

/// Derive the commit quorum timeout from block/commit times.
pub(super) fn commit_quorum_timeout_from_durations(
    block_time: Duration,
    commit_time: Duration,
    da_enabled: bool,
    da_quorum_timeout_multiplier: u32,
) -> Duration {
    if commit_time == Duration::ZERO {
        warn!(
            block_time_ms = block_time.as_millis(),
            "commit timeout is zero; defaulting to block_time to preserve view-change liveness"
        );
        return block_time.max(Duration::from_millis(1));
    }

    // DA runs need additional slack for payload dissemination; non-DA keeps a floor
    // to avoid overly aggressive view-change churn on small pipelines.
    let base = if da_enabled {
        let commit_window = saturating_mul_duration(commit_time, 4);
        block_time.saturating_add(commit_window)
    } else {
        block_time.max(commit_time).max(Duration::from_secs(2))
    };
    let base = if da_enabled {
        saturating_mul_duration(base, da_quorum_timeout_multiplier.max(1))
    } else {
        base
    };
    base.max(Duration::from_millis(1))
}

fn active_round_height(
    highest_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    committed_qc: Option<crate::sumeragi::consensus::QcHeaderRef>,
    committed_height: u64,
) -> u64 {
    // NEW_VIEW requires a precommit HighestQC, so ignore prevote-only headers for round height.
    let highest_precommit =
        highest_qc.filter(|qc| qc.phase == crate::sumeragi::consensus::Phase::Commit);
    highest_precommit.or(committed_qc).map_or_else(
        || committed_height.saturating_add(1),
        |qc| qc.height.saturating_add(1),
    )
}

fn pacemaker_base_interval(block_time: Duration, config: &SumeragiConfig) -> Duration {
    let propose_seed = config.npos.timeouts.propose;
    let rtt_mul = config.npos.pacemaker_rtt_floor_multiplier.max(1);
    let propose_floor = saturating_mul_duration(propose_seed, rtt_mul);
    let max_backoff = config.npos.pacemaker_max_backoff;
    block_time.max(propose_floor).min(max_backoff)
}

fn availability_timeout_from_quorum(
    quorum_timeout: Duration,
    da_enabled: bool,
    da_availability_timeout_multiplier: u32,
    da_availability_timeout_floor: Duration,
) -> Duration {
    if !da_enabled {
        return quorum_timeout;
    }
    let base = quorum_timeout.max(da_availability_timeout_floor);
    saturating_mul_duration(base, da_availability_timeout_multiplier.max(1))
}

#[cfg(test)]
fn availability_gate_timeout_exceeded(pending_age: Duration, timeout: Duration) -> bool {
    timeout != Duration::ZERO && pending_age >= timeout
}

fn should_run_commit_pipeline_on_tick(pending_blocks: usize) -> bool {
    pending_blocks > 0
}

fn precommit_vote_count(qc: &crate::sumeragi::consensus::Qc, roster_len: usize) -> usize {
    if matches!(qc.phase, crate::sumeragi::consensus::Phase::Commit) {
        qc_voting_signer_count(qc, roster_len)
    } else {
        0
    }
}

/// Inputs used when assembling an empty-child fallback proposal.
#[derive(Debug, Clone, Copy)]
struct EmptyChildContext {
    target_height: u64,
    target_view: u64,
    highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    parent_payload_hash: Hash,
}

/// Decide whether another empty-child emission is allowed for the same parent payload.
fn empty_child_backoff_allows(
    last_attempt: Option<(Hash, Instant)>,
    parent_payload_hash: Hash,
    now: Instant,
    backoff: Duration,
) -> bool {
    let Some((last_hash, last_instant)) = last_attempt else {
        return true;
    };
    if backoff == Duration::ZERO {
        return true;
    }
    last_hash != parent_payload_hash || now.saturating_duration_since(last_instant) >= backoff
}

#[derive(Debug, Default)]
struct PayloadRebroadcastThrottle {
    last_sent: BTreeMap<HashOf<BlockHeader>, Instant>,
}

impl PayloadRebroadcastThrottle {
    fn allow(&mut self, block_hash: HashOf<BlockHeader>, now: Instant, cooldown: Duration) -> bool {
        if let Some(previous) = self.last_sent.get(&block_hash) {
            if cooldown > Duration::ZERO && now.saturating_duration_since(*previous) < cooldown {
                return false;
            }
        }
        self.last_sent.insert(block_hash, now);

        if cooldown > Duration::ZERO {
            // Retain only entries that have seen activity within a reasonable window so the
            // throttle map does not grow without bound in long-running nodes.
            let expiry_window = cooldown.saturating_mul(8);
            self.last_sent
                .retain(|_, recorded| now.saturating_duration_since(*recorded) <= expiry_window);
        }
        true
    }

    fn clear(&mut self) {
        self.last_sent.clear();
    }
}

fn distinct_epochs_for_block_votes(
    vote_log: &BTreeMap<
        (
            crate::sumeragi::consensus::Phase,
            u64,
            u64,
            u64,
            crate::sumeragi::consensus::ValidatorIndex,
        ),
        crate::sumeragi::consensus::Vote,
    >,
    phase: crate::sumeragi::consensus::Phase,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
) -> BTreeSet<u64> {
    vote_log
        .values()
        .filter_map(|stored| {
            (stored.phase == phase
                && stored.block_hash == block_hash
                && stored.height == height
                && stored.view == view)
                .then_some(stored.epoch)
        })
        .collect()
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PipelinePhase {
    Propose,
    CollectDa,
    CollectPrepare,
    CollectCommit,
    CollectExec,
    CollectWitness,
    Commit,
}

impl PipelinePhase {
    #[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
    #[allow(dead_code)]
    fn telemetry_label(self) -> &'static str {
        match self {
            Self::Propose => "propose",
            Self::CollectDa => "collect_da",
            Self::CollectPrepare => "collect_prepare",
            Self::CollectCommit => "collect_commit",
            Self::CollectExec => "collect_exec",
            Self::CollectWitness => "collect_witness",
            Self::Commit => "commit",
        }
    }
}

#[derive(Clone, Copy)]
struct PhaseRecordFlags(u8);

impl PhaseRecordFlags {
    const PROPOSE: u8 = 1 << 0;
    const COLLECT_DA: u8 = 1 << 1;
    const COLLECT_PREPARE: u8 = 1 << 2;
    const COLLECT_COMMIT: u8 = 1 << 3;
    const COLLECT_EXEC: u8 = 1 << 4;
    const COLLECT_WITNESS: u8 = 1 << 5;
    const COMMIT: u8 = 1 << 6;

    const fn new() -> Self {
        Self(0)
    }

    fn reset(&mut self) {
        self.0 = 0;
    }

    fn mask(phase: PipelinePhase) -> u8 {
        match phase {
            PipelinePhase::Propose => Self::PROPOSE,
            PipelinePhase::CollectDa => Self::COLLECT_DA,
            PipelinePhase::CollectPrepare => Self::COLLECT_PREPARE,
            PipelinePhase::CollectCommit => Self::COLLECT_COMMIT,
            PipelinePhase::CollectExec => Self::COLLECT_EXEC,
            PipelinePhase::CollectWitness => Self::COLLECT_WITNESS,
            PipelinePhase::Commit => Self::COMMIT,
        }
    }

    fn is_recorded(self, phase: PipelinePhase) -> bool {
        self.0 & Self::mask(phase) != 0
    }

    fn mark(&mut self, phase: PipelinePhase) {
        self.0 |= Self::mask(phase);
    }
}

#[derive(Clone, Copy)]
pub(super) struct Ema {
    value_ms: f64,
    alpha: f64,
    initialized: bool,
}

impl Ema {
    pub(super) const fn new(alpha: f64, seed_ms: f64) -> Self {
        Self {
            value_ms: seed_ms,
            alpha,
            initialized: true,
        }
    }

    fn update(&mut self, sample_ms: f64) -> f64 {
        let clamped = sample_ms.max(0.0);
        if self.initialized {
            self.value_ms = self
                .alpha
                .mul_add(clamped, (1.0 - self.alpha) * self.value_ms);
        } else {
            self.value_ms = clamped;
            self.initialized = true;
        }
        self.value_ms
    }

    fn current(&self) -> f64 {
        self.value_ms
    }
}

pub(super) struct PhaseEma {
    propose: Ema,
    collect_da: Ema,
    collect_prevote: Ema,
    collect_precommit: Ema,
    collect_exec: Ema,
    collect_witness: Ema,
    commit: Ema,
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

impl PhaseEma {
    pub(super) fn new(timeouts: &iroha_config::parameters::actual::SumeragiNposTimeouts) -> Self {
        Self {
            propose: Ema::new(PACEMAKER_PHASE_EMA_ALPHA, duration_ms(timeouts.propose)),
            collect_da: Ema::new(PACEMAKER_PHASE_EMA_ALPHA, duration_ms(timeouts.da)),
            collect_prevote: Ema::new(PACEMAKER_PHASE_EMA_ALPHA, duration_ms(timeouts.prevote)),
            collect_precommit: Ema::new(PACEMAKER_PHASE_EMA_ALPHA, duration_ms(timeouts.precommit)),
            collect_exec: Ema::new(PACEMAKER_PHASE_EMA_ALPHA, duration_ms(timeouts.exec)),
            collect_witness: Ema::new(PACEMAKER_PHASE_EMA_ALPHA, duration_ms(timeouts.witness)),
            commit: Ema::new(PACEMAKER_PHASE_EMA_ALPHA, duration_ms(timeouts.commit)),
        }
    }

    fn update(&mut self, phase: PipelinePhase, sample_ms: f64) -> f64 {
        match phase {
            PipelinePhase::Propose => self.propose.update(sample_ms),
            PipelinePhase::CollectDa => self.collect_da.update(sample_ms),
            PipelinePhase::CollectPrepare => self.collect_prevote.update(sample_ms),
            PipelinePhase::CollectCommit => self.collect_precommit.update(sample_ms),
            PipelinePhase::CollectExec => self.collect_exec.update(sample_ms),
            PipelinePhase::CollectWitness => self.collect_witness.update(sample_ms),
            PipelinePhase::Commit => self.commit.update(sample_ms),
        }
    }

    #[allow(dead_code)]
    fn current(&self, phase: PipelinePhase) -> f64 {
        match phase {
            PipelinePhase::Propose => self.propose.current(),
            PipelinePhase::CollectDa => self.collect_da.current(),
            PipelinePhase::CollectPrepare => self.collect_prevote.current(),
            PipelinePhase::CollectCommit => self.collect_precommit.current(),
            PipelinePhase::CollectExec => self.collect_exec.current(),
            PipelinePhase::CollectWitness => self.collect_witness.current(),
            PipelinePhase::Commit => self.commit.current(),
        }
    }

    pub(super) fn total_duration(&self) -> Duration {
        let total_ms = self.propose.current()
            + self.collect_da.current()
            + self.collect_prevote.current()
            + self.collect_precommit.current()
            // Execution/Witness latencies stay observability-only and remain excluded from the
            // pacemaker control loop even though they have dedicated timeout seeds.
            + self.commit.current();
        Duration::from_millis(round_duration_ms(total_ms))
    }
}

pub(super) struct PhaseTracker {
    round_height: Option<u64>,
    round_view: u64,
    round_start: Instant,
    last_marker: Instant,
    recorded: PhaseRecordFlags,
}

impl PhaseTracker {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            round_height: None,
            round_view: 0,
            round_start: now,
            last_marker: now,
            recorded: PhaseRecordFlags::new(),
        }
    }

    pub(super) fn start_new_round(&mut self, height: u64, now: Instant) {
        self.round_height = Some(height);
        self.round_view = 0;
        self.round_start = now;
        self.last_marker = now;
        self.recorded.reset();
    }

    pub(super) fn on_view_change(&mut self, height: u64, view: u64, now: Instant) {
        if self.round_height != Some(height) {
            self.round_height = Some(height);
        }
        self.round_view = view;
        self.round_start = now;
        self.last_marker = now;
        self.recorded.reset();
    }

    pub(super) fn record(
        &mut self,
        phase: PipelinePhase,
        height: u64,
        view: u64,
        now: Instant,
    ) -> Option<Duration> {
        if let Some(round_height) = self.round_height {
            if round_height != height {
                return None;
            }
        } else {
            return None;
        }

        if self.round_view != view {
            self.round_view = view;
            self.round_start = now;
            self.last_marker = now;
            self.recorded.reset();
        }

        if self.recorded.is_recorded(phase) {
            return None;
        }

        let duration = now.saturating_duration_since(self.last_marker);
        self.recorded.mark(phase);
        self.last_marker = now;
        Some(duration)
    }

    pub(super) fn view_age(&self, height: u64, now: Instant) -> Option<Duration> {
        if self.round_height != Some(height) {
            return None;
        }
        Some(now.saturating_duration_since(self.round_start))
    }

    pub(super) fn current_view(&self, height: u64) -> Option<u64> {
        if self.round_height != Some(height) {
            return None;
        }
        Some(self.round_view)
    }
}

/// Return `true` when we have observed a delivered RBC payload that matches the given
/// block hash, height, and payload digest without being marked invalid.
fn rbc_payload_matches(
    sessions: &BTreeMap<super::rbc_store::SessionKey, RbcSession>,
    handle: &rbc_status::Handle,
    block_hash: &HashOf<BlockHeader>,
    height: u64,
    payload_hash: &Hash,
) -> bool {
    let start = (*block_hash, height, 0);
    let end = (*block_hash, height, u64::MAX);
    for (_, session) in sessions.range(start..=end) {
        if session.delivered_payload_matches(payload_hash) {
            return true;
        }
    }
    handle.delivered_payload_matches(block_hash, height, payload_hash)
}

#[derive(Clone, Debug)]
struct RbcChunkEntry {
    bytes: Vec<u8>,
    digest: [u8; 32],
}

#[derive(Clone, Debug, Default)]
struct ReadySignature {
    sender: u32,
    signature: Vec<u8>,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RbcSessionError {
    /// The payload advertised more chunks than the implementation allows.
    #[error("too many RBC chunks: {total_chunks} (cap {max_chunks})")]
    TooManyChunks {
        /// Reported chunk count in the RBC session.
        total_chunks: u32,
        /// Hard cap enforced by the implementation.
        max_chunks: u32,
    },
}

#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct RbcSession {
    total_chunks: u32,
    payload_hash: Option<Hash>,
    expected_chunk_root: Option<Hash>,
    epoch: u64,
    chunks: Vec<Option<RbcChunkEntry>>,
    received_chunks: u32,
    ready_signatures: Vec<ReadySignature>,
    sent_ready: bool,
    delivered: bool,
    deliver_sender: Option<u32>,
    deliver_signature: Option<Vec<u8>>,
    invalid: bool,
    recovered_from_disk: bool,
    lane_allocations: Vec<LaneAllocation>,
    dataspace_allocations: Vec<DataspaceAllocation>,
}

impl RbcSession {
    pub(crate) fn new(
        total_chunks: u32,
        payload_hash: Option<Hash>,
        expected_chunk_root: Option<Hash>,
        epoch: u64,
    ) -> Result<Self, RbcSessionError> {
        if total_chunks > RBC_MAX_TOTAL_CHUNKS {
            return Err(RbcSessionError::TooManyChunks {
                total_chunks,
                max_chunks: RBC_MAX_TOTAL_CHUNKS,
            });
        }
        let capacity = usize::try_from(total_chunks).unwrap_or(RBC_MAX_TOTAL_CHUNKS as usize);
        Ok(Self {
            total_chunks,
            payload_hash,
            expected_chunk_root,
            epoch,
            chunks: vec![None; capacity],
            received_chunks: 0,
            ready_signatures: Vec::new(),
            sent_ready: false,
            delivered: false,
            deliver_sender: None,
            deliver_signature: None,
            invalid: false,
            recovered_from_disk: false,
            lane_allocations: Vec::new(),
            dataspace_allocations: Vec::new(),
        })
    }

    #[cfg(test)]
    pub(crate) fn test_new(
        total_chunks: u32,
        payload_hash: Option<Hash>,
        expected_chunk_root: Option<Hash>,
        epoch: u64,
    ) -> Self {
        Self::new(total_chunks, payload_hash, expected_chunk_root, epoch)
            .expect("test configuration should respect RBC chunk cap")
    }

    pub(crate) fn total_chunks(&self) -> u32 {
        self.total_chunks
    }

    pub(crate) fn received_chunks(&self) -> u32 {
        self.received_chunks
    }

    pub(crate) fn payload_hash(&self) -> Option<Hash> {
        self.payload_hash
    }

    fn set_allocations(
        &mut self,
        lane_allocations: Vec<LaneAllocation>,
        dataspace_allocations: Vec<DataspaceAllocation>,
    ) {
        self.lane_allocations = lane_allocations;
        self.dataspace_allocations = dataspace_allocations;
    }

    fn approx_pending_chunks(&self, allocated: u32) -> u32 {
        if self.delivered {
            return 0;
        }
        if self.total_chunks == 0 {
            return allocated;
        }
        let received = u128::from(self.received_chunks).saturating_mul(u128::from(allocated))
            / u128::from(self.total_chunks);
        let received = received.min(u128::from(allocated));
        allocated.saturating_sub(u32::try_from(received).unwrap_or(u32::MAX))
    }

    pub(crate) fn lane_backlog_entries(&self) -> Vec<super::status::LaneRbcSnapshot> {
        self.lane_allocations
            .iter()
            .map(|alloc| super::status::LaneRbcSnapshot {
                lane_id: alloc.lane_id.as_u32(),
                tx_count: alloc.tx_count,
                total_chunks: u64::from(alloc.total_chunks),
                pending_chunks: u64::from(self.approx_pending_chunks(alloc.total_chunks)),
                rbc_bytes_total: alloc.rbc_bytes_total,
            })
            .collect()
    }

    pub(crate) fn dataspace_backlog_entries(&self) -> Vec<super::status::DataspaceRbcSnapshot> {
        self.dataspace_allocations
            .iter()
            .map(|alloc| super::status::DataspaceRbcSnapshot {
                lane_id: alloc.lane_id.as_u32(),
                dataspace_id: alloc.dataspace_id.as_u64(),
                tx_count: alloc.tx_count,
                total_chunks: u64::from(alloc.total_chunks),
                pending_chunks: u64::from(self.approx_pending_chunks(alloc.total_chunks)),
                rbc_bytes_total: alloc.rbc_bytes_total,
            })
            .collect()
    }

    pub(crate) fn adopt_allocations_from_summary(&mut self, summary: &rbc_status::Summary) {
        if !self.lane_allocations.is_empty() || !self.dataspace_allocations.is_empty() {
            return;
        }

        let mut lane_allocations: Vec<LaneAllocation> = summary
            .lane_backlog
            .iter()
            .map(|entry| {
                let capped = entry.total_chunks.min(u64::from(u32::MAX));
                LaneAllocation {
                    lane_id: LaneId::new(entry.lane_id),
                    tx_count: entry.tx_count,
                    rbc_bytes_total: entry.rbc_bytes_total,
                    teu_total: 0,
                    total_chunks: u32::try_from(capped).unwrap_or(u32::MAX),
                }
            })
            .collect();

        let mut dataspace_allocations: Vec<DataspaceAllocation> = summary
            .dataspace_backlog
            .iter()
            .map(|entry| {
                let capped = entry.total_chunks.min(u64::from(u32::MAX));
                DataspaceAllocation {
                    lane_id: LaneId::new(entry.lane_id),
                    dataspace_id: DataSpaceId::new(entry.dataspace_id),
                    tx_count: entry.tx_count,
                    rbc_bytes_total: entry.rbc_bytes_total,
                    teu_total: 0,
                    total_chunks: u32::try_from(capped).unwrap_or(u32::MAX),
                }
            })
            .collect();

        if lane_allocations.is_empty() && dataspace_allocations.is_empty() {
            return;
        }

        lane_allocations.sort_by_key(|alloc| alloc.lane_id.as_u32());
        dataspace_allocations.sort_by(|a, b| {
            a.lane_id
                .as_u32()
                .cmp(&b.lane_id.as_u32())
                .then_with(|| a.dataspace_id.as_u64().cmp(&b.dataspace_id.as_u64()))
        });

        self.set_allocations(lane_allocations, dataspace_allocations);
    }

    pub(crate) fn delivered_payload_matches(&self, payload_hash: &Hash) -> bool {
        self.delivered
            && !self.is_invalid()
            && matches!(self.payload_hash(), Some(hash) if &hash == payload_hash)
    }

    pub(crate) fn chunk_bytes(&self, idx: u32) -> Option<&[u8]> {
        let idx = idx as usize;
        self.chunks
            .get(idx)?
            .as_ref()
            .map(|entry| entry.bytes.as_slice())
    }

    pub(crate) fn chunk_digest(&self, idx: u32) -> Option<[u8; 32]> {
        let idx = idx as usize;
        self.chunks.get(idx)?.as_ref().map(|entry| entry.digest)
    }

    pub(crate) fn all_chunk_digests(&self) -> Option<Vec<[u8; 32]>> {
        if self.total_chunks == 0 {
            return Some(Vec::new());
        }
        if self.received_chunks != self.total_chunks {
            return None;
        }
        let mut digests = Vec::with_capacity(self.total_chunks as usize);
        for entry in &self.chunks {
            let entry = entry.as_ref()?;
            digests.push(entry.digest);
        }
        Some(digests)
    }

    pub(crate) fn chunk_root(&self) -> Option<Hash> {
        if self.total_chunks == 0 {
            return self.expected_chunk_root;
        }
        let digests = self.all_chunk_digests()?;
        let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests);
        tree.root().map(Hash::from)
    }

    pub(super) fn to_persisted(
        &self,
        key: super::rbc_store::SessionKey,
        chain_hash: Hash,
        manifest: &super::rbc_store::SoftwareManifest,
        session_roster: &[PeerId],
    ) -> super::rbc_store::PersistedSession {
        use super::rbc_store::{PersistedChunk, PersistedReady, PersistedSession};

        let mut chunks = Vec::new();
        for (idx, entry) in self.chunks.iter().enumerate() {
            if let Some(entry) = entry {
                let chunk_idx =
                    u32::try_from(idx).expect("chunk index fits within u32::MAX for persistence");
                chunks.push(PersistedChunk {
                    idx: chunk_idx,
                    bytes: entry.bytes.clone(),
                });
            }
        }

        let ready_signatures = self
            .ready_signatures
            .iter()
            .map(|sig| PersistedReady {
                sender: sig.sender,
                signature: sig.signature.clone(),
            })
            .collect();

        let computed_root = self.chunk_root();
        let now_ms = TimeSource::new_system()
            .now()
            .as_millis()
            .min(u128::from(u64::MAX));
        let now_ms = u64::try_from(now_ms).expect("min(u128, u64::MAX) fits into u64");

        PersistedSession {
            format_version: super::rbc_store::PERSIST_VERSION,
            chain_hash,
            software_manifest: manifest.clone(),
            block_hash: key.0,
            height: key.1,
            view: key.2,
            epoch: self.epoch,
            total_chunks: self.total_chunks,
            payload_hash: self.payload_hash,
            expected_chunk_root: self.expected_chunk_root,
            computed_chunk_root: computed_root,
            invalid: self.invalid,
            sent_ready: self.sent_ready,
            ready_signatures,
            delivered: self.delivered,
            deliver_sender: self.deliver_sender,
            deliver_signature: self.deliver_signature.clone(),
            chunks,
            last_updated_ms: now_ms,
            session_roster: session_roster.to_vec(),
        }
    }

    pub(super) fn from_persisted_unchecked(
        persisted: &super::rbc_store::PersistedSession,
    ) -> Result<Self, PersistedLoadError> {
        use super::rbc_store::PersistedSession;

        fn build_chunk(bytes: Vec<u8>) -> RbcChunkEntry {
            let mut hasher = Sha256::new();
            sha2::digest::Update::update(&mut hasher, &bytes);
            let mut digest = [0u8; 32];
            digest.copy_from_slice(&hasher.finalize());
            RbcChunkEntry { bytes, digest }
        }

        let PersistedSession {
            total_chunks,
            payload_hash,
            expected_chunk_root,
            invalid,
            ready_signatures,
            delivered,
            deliver_sender,
            deliver_signature,
            sent_ready,
            chunks,
            epoch,
            ..
        } = persisted.clone();

        if total_chunks > RBC_MAX_TOTAL_CHUNKS {
            return Err(PersistedLoadError::TooManyChunks {
                total_chunks,
                max_chunks: RBC_MAX_TOTAL_CHUNKS,
            });
        }

        let capacity = usize::try_from(total_chunks)
            .unwrap_or_else(|_| usize::try_from(RBC_MAX_TOTAL_CHUNKS).unwrap());
        let mut data = vec![None; capacity];
        for chunk in chunks {
            let idx = usize::try_from(chunk.idx).map_err(|_| {
                PersistedLoadError::ChunkIndexOutOfBounds {
                    idx: chunk.idx,
                    total_chunks,
                }
            })?;
            if idx >= data.len() {
                return Err(PersistedLoadError::ChunkIndexOutOfBounds {
                    idx: chunk.idx,
                    total_chunks,
                });
            }
            if data[idx].is_some() {
                return Err(PersistedLoadError::DuplicateChunkIndex(chunk.idx));
            }
            data[idx] = Some(build_chunk(chunk.bytes));
        }

        let received_chunks =
            u32::try_from(data.iter().filter(|entry| entry.is_some()).count()).unwrap_or(u32::MAX);
        if let Some(hash) = payload_hash {
            if received_chunks == total_chunks {
                let mut aggregate = Vec::new();
                for entry in data.iter().flatten() {
                    aggregate.extend_from_slice(&entry.bytes);
                }
                if Hash::new(&aggregate) != hash {
                    return Err(PersistedLoadError::PayloadHashMismatch);
                }
            }
        }

        let ready_signatures = ready_signatures
            .into_iter()
            .map(|sig| ReadySignature {
                sender: sig.sender,
                signature: sig.signature,
            })
            .collect();

        let mut session = Self::new(total_chunks, payload_hash, expected_chunk_root, epoch)
            .map_err(|err| match err {
                RbcSessionError::TooManyChunks {
                    total_chunks,
                    max_chunks,
                } => PersistedLoadError::TooManyChunks {
                    total_chunks,
                    max_chunks,
                },
            })?;
        session.chunks = data;
        session.received_chunks = received_chunks;
        session.ready_signatures = ready_signatures;
        session.sent_ready = sent_ready;
        session.delivered = delivered;
        session.deliver_sender = deliver_sender;
        session.deliver_signature = deliver_signature;
        session.invalid = invalid;
        session.recovered_from_disk = true;
        Ok(session)
    }

    pub(crate) fn recovered_from_disk(&self) -> bool {
        self.recovered_from_disk
    }

    pub(crate) fn ingest_chunk(&mut self, idx: u32, bytes: Vec<u8>, sender: Option<u32>) {
        self.note_chunk(idx, bytes, sender);
    }

    pub(crate) fn record_ready(&mut self, sender: u32, signature: Vec<u8>) -> bool {
        if let Some(existing) = self
            .ready_signatures
            .iter()
            .find(|entry| entry.sender == sender)
        {
            if existing.signature != signature {
                self.invalid = true;
            }
            return false;
        }
        self.ready_signatures
            .push(ReadySignature { sender, signature });
        true
    }

    pub(crate) fn record_deliver(&mut self, sender: u32, signature: Vec<u8>) -> bool {
        if self.delivered {
            if self.deliver_sender == Some(sender)
                && self
                    .deliver_signature
                    .as_deref()
                    .is_some_and(|sig| sig == signature.as_slice())
            {
                return false;
            }
            if self.deliver_sender == Some(sender) {
                self.invalid = true;
            }
            return false;
        }
        self.delivered = true;
        self.deliver_sender = Some(sender);
        self.deliver_signature = Some(signature);
        true
    }

    pub(crate) fn is_invalid(&self) -> bool {
        self.invalid
    }

    #[cfg(test)]
    pub(crate) fn test_note_chunk(&mut self, idx: u32, bytes: Vec<u8>, sender: u32) {
        self.note_chunk(idx, bytes, Some(sender));
    }

    fn note_chunk(&mut self, idx: u32, bytes: Vec<u8>, _sender: Option<u32>) {
        if idx >= self.total_chunks {
            return;
        }
        let slot = &mut self.chunks[idx as usize];
        if slot.is_none() {
            let mut hasher = Sha256::new();
            sha2::digest::Update::update(&mut hasher, &bytes);
            let mut digest = [0u8; 32];
            digest.copy_from_slice(&hasher.finalize());
            *slot = Some(RbcChunkEntry { bytes, digest });
            self.received_chunks = self.received_chunks.saturating_add(1);
        }
    }

    #[cfg(test)]
    pub(crate) fn test_set_delivered(&mut self, delivered: bool) {
        self.delivered = delivered;
    }

    #[cfg(test)]
    pub(crate) fn test_set_sent_ready(&mut self, sent_ready: bool) {
        self.sent_ready = sent_ready;
    }

    pub(crate) fn delivered_payload_bytes(&self) -> Option<u64> {
        if !self.delivered || self.received_chunks != self.total_chunks {
            return None;
        }
        let mut total: u64 = 0;
        for entry in &self.chunks {
            let entry = entry.as_ref()?;
            total = total.saturating_add(entry.bytes.len() as u64);
        }
        Some(total)
    }
}

/// Errors encountered while reconstructing persisted RBC payloads from disk.
#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum PersistedLoadError {
    /// Encountered a duplicate chunk index while loading persisted chunks.
    #[error("duplicate chunk index {0}")]
    DuplicateChunkIndex(u32),
    /// Observed a chunk index outside the expected bounds for the payload.
    #[error("chunk index {idx} out of bounds for total {total_chunks}")]
    ChunkIndexOutOfBounds {
        /// Offending chunk index.
        idx: u32,
        /// Total number of chunks expected for the payload.
        total_chunks: u32,
    },
    /// The accumulated payload hash did not match the stored digest.
    #[error("payload hash mismatch")]
    PayloadHashMismatch,
    /// Persisted snapshot advertises more chunks than supported.
    #[error("persisted RBC session exceeds chunk cap: {total_chunks} (cap {max_chunks})")]
    TooManyChunks {
        /// Persisted chunk count.
        total_chunks: u32,
        /// Hard cap enforced at runtime.
        max_chunks: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ManifestSpoolKey {
    lane: u32,
    epoch: u64,
    sequence: u64,
    ticket: StorageTicketId,
}

impl ManifestSpoolKey {
    fn from_record(record: &DaCommitmentRecord) -> Self {
        Self {
            lane: record.lane_id.as_u32(),
            epoch: record.epoch,
            sequence: record.sequence,
            ticket: record.storage_ticket,
        }
    }
}

#[derive(Debug, Clone)]
struct ManifestSpoolEntry {
    path: PathBuf,
    file_modified: Option<SystemTime>,
    file_len: u64,
    digest: Option<ManifestDigest>,
}

#[derive(Debug)]
enum ManifestSpoolLookupError {
    SpoolScan(std::io::Error),
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl ManifestSpoolEntry {
    fn digest(&mut self) -> Result<(ManifestDigest, CacheOutcome), ManifestSpoolLookupError> {
        let metadata =
            fs::metadata(&self.path).map_err(|source| ManifestSpoolLookupError::Read {
                path: self.path.clone(),
                source,
            })?;
        let file_len = metadata.len();
        let file_modified = metadata.modified().ok();
        if self.file_len != file_len || self.file_modified != file_modified {
            self.file_len = file_len;
            self.file_modified = file_modified;
            self.digest = None;
        }
        if let Some(digest) = self.digest {
            return Ok((digest, CacheOutcome::Hit));
        }
        let bytes = fs::read(&self.path).map_err(|source| ManifestSpoolLookupError::Read {
            path: self.path.clone(),
            source,
        })?;
        let digest = ManifestDigest::new(*blake3_hash(&bytes).as_bytes());
        self.digest = Some(digest);
        Ok((digest, CacheOutcome::Miss))
    }
}

struct ManifestSpoolScan {
    stamp: SpoolDirStamp,
    entries: BTreeMap<ManifestSpoolKey, Vec<ManifestSpoolEntry>>,
}

fn scan_manifest_spool(spool_dir: &Path) -> Result<Option<ManifestSpoolScan>, std::io::Error> {
    let metadata = match fs::metadata(spool_dir) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "manifest spool path is not a directory",
        ));
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(spool_dir)? {
        let entry = match entry {
            Ok(value) => value,
            Err(err) => {
                warn!(?err, "failed to read manifest spool entry");
                continue;
            }
        };
        let name = match entry.file_name().to_str() {
            Some(value) => value.to_string(),
            None => continue,
        };
        if !name.starts_with("manifest-") || !name.ends_with(".norito") {
            continue;
        }
        let Some(key) = parse_manifest_spool_key(&name) else {
            continue;
        };
        let metadata = match entry.metadata() {
            Ok(meta) => meta,
            Err(err) => {
                warn!(
                    ?err,
                    path = %entry.path().display(),
                    "failed to read manifest spool metadata"
                );
                continue;
            }
        };
        entries.push((
            name,
            key,
            entry.path(),
            metadata.len(),
            metadata.modified().ok(),
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let stamp_entries = entries
        .iter()
        .map(|(name, _, _, len, modified)| (name.clone(), *len, *modified))
        .collect::<Vec<_>>();
    let stamp = SpoolDirStamp::from_entries(&stamp_entries);

    let mut map = BTreeMap::new();
    for (_, key, path, file_len, file_modified) in entries {
        map.entry(key)
            .or_insert_with(Vec::new)
            .push(ManifestSpoolEntry {
                path,
                file_modified,
                file_len,
                digest: None,
            });
    }

    Ok(Some(ManifestSpoolScan {
        stamp,
        entries: map,
    }))
}

#[derive(Debug, Default)]
struct ManifestSpoolCache {
    dir_stamp: Option<SpoolDirStamp>,
    entries: BTreeMap<ManifestSpoolKey, Vec<ManifestSpoolEntry>>,
}

impl ManifestSpoolCache {
    fn refresh_if_stale(&mut self, spool_dir: &Path) -> Result<bool, std::io::Error> {
        let Some(scan) = scan_manifest_spool(spool_dir)? else {
            self.dir_stamp = None;
            self.entries.clear();
            return Ok(false);
        };
        if self.dir_stamp != Some(scan.stamp) {
            self.dir_stamp = Some(scan.stamp);
            self.entries = scan.entries;
        }
        Ok(true)
    }

    fn force_refresh(&mut self, spool_dir: &Path) -> Result<bool, std::io::Error> {
        let Some(scan) = scan_manifest_spool(spool_dir)? else {
            self.dir_stamp = None;
            self.entries.clear();
            return Ok(false);
        };
        self.dir_stamp = Some(scan.stamp);
        self.entries = scan.entries;
        Ok(true)
    }

    fn manifest_digest_for_commitment(
        &mut self,
        spool_dir: &Path,
        record: &DaCommitmentRecord,
    ) -> Result<(Option<(ManifestDigest, PathBuf)>, CacheOutcome), ManifestSpoolLookupError> {
        let exists = self
            .refresh_if_stale(spool_dir)
            .map_err(ManifestSpoolLookupError::SpoolScan)?;
        if !exists {
            return Ok((None, CacheOutcome::Miss));
        }
        let key = ManifestSpoolKey::from_record(record);
        if let Some(entries) = self.entries.get_mut(&key) {
            return Self::select_manifest_candidate(entries, record.manifest_hash);
        }

        let exists = self
            .force_refresh(spool_dir)
            .map_err(ManifestSpoolLookupError::SpoolScan)?;
        if !exists {
            return Ok((None, CacheOutcome::Miss));
        }
        match self.entries.get_mut(&key) {
            Some(entries) => {
                let (observed, outcome) =
                    Self::select_manifest_candidate(entries, record.manifest_hash)?;
                Ok((observed, outcome.merge(CacheOutcome::Miss)))
            }
            None => Ok((None, CacheOutcome::Miss)),
        }
    }

    fn select_manifest_candidate(
        entries: &mut [ManifestSpoolEntry],
        expected: ManifestDigest,
    ) -> Result<(Option<(ManifestDigest, PathBuf)>, CacheOutcome), ManifestSpoolLookupError> {
        if entries.is_empty() {
            return Ok((None, CacheOutcome::Miss));
        }
        let mut outcome = CacheOutcome::Hit;
        let mut observed = None;
        for entry in entries {
            let (digest, entry_outcome) = entry.digest()?;
            outcome = outcome.merge(entry_outcome);
            if observed.is_none() {
                observed = Some((digest, entry.path.clone()));
            }
            if digest == expected {
                return Ok((Some((digest, entry.path.clone())), outcome));
            }
        }
        Ok((observed, outcome))
    }
}

fn parse_manifest_spool_key(name: &str) -> Option<ManifestSpoolKey> {
    let name = name.strip_suffix(".norito")?;
    let rest = name.strip_prefix("manifest-")?;
    let mut parts = rest.splitn(5, '-');
    let lane_hex = parts.next()?;
    let epoch_hex = parts.next()?;
    let sequence_hex = parts.next()?;
    let ticket_hex = parts.next()?;
    let lane = u32::from_str_radix(lane_hex, 16).ok()?;
    let epoch = u64::from_str_radix(epoch_hex, 16).ok()?;
    let sequence = u64::from_str_radix(sequence_hex, 16).ok()?;
    let mut ticket_bytes = [0u8; 32];
    hex::decode_to_slice(ticket_hex, &mut ticket_bytes).ok()?;
    Some(ManifestSpoolKey {
        lane,
        epoch,
        sequence,
        ticket: StorageTicketId::new(ticket_bytes),
    })
}

/// Errors raised by the manifest guard when validating DA commitments.
#[derive(Debug, Error)]
enum ManifestGuardError {
    /// Spool directory could not be scanned.
    #[error("failed to scan manifest spool {spool}: {source}")]
    SpoolScan {
        /// Lane id tied to the commitment.
        lane: u32,
        /// Epoch the commitment belongs to.
        epoch: u64,
        /// Sequence number within the epoch.
        sequence: u64,
        /// Spool directory being scanned.
        spool: PathBuf,
        /// I/O error raised during the scan.
        #[source]
        source: std::io::Error,
    },
    /// Manifest file could not be read.
    #[error("failed to read manifest {path}: {source}")]
    Read {
        /// Lane id tied to the commitment.
        lane: u32,
        /// Epoch the commitment belongs to.
        epoch: u64,
        /// Sequence number within the epoch.
        sequence: u64,
        /// Path to the manifest file.
        path: PathBuf,
        /// I/O error raised while reading.
        #[source]
        source: std::io::Error,
    },
    /// Manifest bytes did not match the commitment hash.
    #[error(
        "manifest hash mismatch for lane {lane} epoch {epoch} seq {sequence}: expected {expected_hex}, observed {observed_hex} ({path})"
    )]
    HashMismatch {
        /// Lane id tied to the commitment.
        lane: u32,
        /// Epoch the commitment belongs to.
        epoch: u64,
        /// Sequence number within the epoch.
        sequence: u64,
        /// Expected manifest hash encoded in hex.
        expected_hex: String,
        /// Observed manifest hash encoded in hex.
        observed_hex: String,
        /// Path to the manifest file.
        path: PathBuf,
    },
    /// Manifest could not be found in the spool directory.
    #[error("manifest missing for lane {lane} epoch {epoch} seq {sequence} in spool {spool}")]
    Missing {
        /// Lane id tied to the commitment.
        lane: u32,
        /// Epoch the commitment belongs to.
        epoch: u64,
        /// Sequence number within the epoch.
        sequence: u64,
        /// Spool directory scanned for the manifest.
        spool: PathBuf,
    },
}

impl ManifestGuardError {
    fn lane_epoch_sequence(&self) -> (u32, u64, u64) {
        match self {
            Self::SpoolScan {
                lane,
                epoch,
                sequence,
                ..
            }
            | Self::Read {
                lane,
                epoch,
                sequence,
                ..
            }
            | Self::HashMismatch {
                lane,
                epoch,
                sequence,
                ..
            }
            | Self::Missing {
                lane,
                epoch,
                sequence,
                ..
            } => (*lane, *epoch, *sequence),
        }
    }

    fn gate_reason(&self) -> GateReason {
        match self {
            Self::SpoolScan {
                lane,
                epoch,
                sequence,
                ..
            } => GateReason::ManifestGuard {
                lane: LaneId::new(*lane),
                epoch: *epoch,
                sequence: *sequence,
                kind: ManifestGateKind::SpoolScan,
            },
            Self::Read {
                lane,
                epoch,
                sequence,
                ..
            } => GateReason::ManifestGuard {
                lane: LaneId::new(*lane),
                epoch: *epoch,
                sequence: *sequence,
                kind: ManifestGateKind::ReadFailed,
            },
            Self::HashMismatch {
                lane,
                epoch,
                sequence,
                ..
            } => GateReason::ManifestGuard {
                lane: LaneId::new(*lane),
                epoch: *epoch,
                sequence: *sequence,
                kind: ManifestGateKind::HashMismatch,
            },
            Self::Missing {
                lane,
                epoch,
                sequence,
                ..
            } => GateReason::ManifestGuard {
                lane: LaneId::new(*lane),
                epoch: *epoch,
                sequence: *sequence,
                kind: ManifestGateKind::Missing,
            },
        }
    }
}

#[derive(Debug)]
enum ManifestGuardOutcome {
    /// Manifest is present and matches the advertised hash.
    Pass,
    /// Manifest is missing or unreadable on an audit-only lane; proceed with a warning.
    Warn(ManifestGuardError),
    /// Manifest is missing or mismatched and the lane requires strict enforcement.
    Reject(ManifestGuardError),
}

#[cfg(feature = "telemetry")]
fn manifest_guard_reason(error: &ManifestGuardError) -> crate::telemetry::ManifestGuardReason {
    match error {
        ManifestGuardError::SpoolScan { .. } => crate::telemetry::ManifestGuardReason::SpoolScan,
        ManifestGuardError::Read { .. } => crate::telemetry::ManifestGuardReason::ReadError,
        ManifestGuardError::HashMismatch { .. } => {
            crate::telemetry::ManifestGuardReason::HashMismatch
        }
        ManifestGuardError::Missing { .. } => crate::telemetry::ManifestGuardReason::Missing,
    }
}

fn enforce_manifest_available_for_commitment(
    manifest_cache: &mut ManifestSpoolCache,
    spool_dir: &Path,
    record: &DaCommitmentRecord,
) -> (CacheOutcome, Result<(), ManifestGuardError>) {
    let lane = record.lane_id.as_u32();
    let epoch = record.epoch;
    let sequence = record.sequence;
    let (observed, cache_outcome) =
        match manifest_cache.manifest_digest_for_commitment(spool_dir, record) {
            Ok(result) => result,
            Err(err) => {
                let guard_err = match err {
                    ManifestSpoolLookupError::SpoolScan(source) => ManifestGuardError::SpoolScan {
                        lane,
                        epoch,
                        sequence,
                        spool: spool_dir.to_path_buf(),
                        source,
                    },
                    ManifestSpoolLookupError::Read { path, source } => ManifestGuardError::Read {
                        lane,
                        epoch,
                        sequence,
                        path,
                        source,
                    },
                };
                return (CacheOutcome::Miss, Err(guard_err));
            }
        };

    let Some((observed, path)) = observed else {
        return (
            cache_outcome,
            Err(ManifestGuardError::Missing {
                lane,
                epoch,
                sequence,
                spool: spool_dir.to_path_buf(),
            }),
        );
    };

    if observed != record.manifest_hash {
        return (
            cache_outcome,
            Err(ManifestGuardError::HashMismatch {
                lane,
                epoch,
                sequence,
                expected_hex: hex::encode(record.manifest_hash.as_bytes()),
                observed_hex: hex::encode(observed.as_bytes()),
                path,
            }),
        );
    }

    (cache_outcome, Ok(()))
}

fn manifest_guard_outcome(
    manifest_cache: &mut ManifestSpoolCache,
    spool_dir: &Path,
    record: &DaCommitmentRecord,
    policy: DaManifestPolicy,
) -> (ManifestGuardOutcome, CacheOutcome) {
    let (cache_outcome, result) =
        enforce_manifest_available_for_commitment(manifest_cache, spool_dir, record);
    let outcome = match result {
        Ok(()) => ManifestGuardOutcome::Pass,
        Err(err @ ManifestGuardError::HashMismatch { .. }) => ManifestGuardOutcome::Reject(err),
        Err(err) => match policy {
            DaManifestPolicy::Strict => ManifestGuardOutcome::Reject(err),
            DaManifestPolicy::AuditOnly => ManifestGuardOutcome::Warn(err),
        },
    };
    (outcome, cache_outcome)
}

fn manifests_available_for_block(
    manifest_cache: &mut ManifestSpoolCache,
    spool_dir: &Path,
    lane_config: &LaneConfigSnapshot,
    block: &SignedBlock,
    cache_outcome: &mut CacheOutcome,
) -> Result<Vec<ManifestGuardError>, ManifestGuardError> {
    let Some(bundle) = block.da_commitments() else {
        return Ok(Vec::new());
    };

    let mut warnings = Vec::new();
    for record in &bundle.commitments {
        let policy = lane_config.manifest_policy(record.lane_id);
        let (outcome, cache) = manifest_guard_outcome(manifest_cache, spool_dir, record, policy);
        *cache_outcome = cache_outcome.merge(cache);
        match outcome {
            ManifestGuardOutcome::Pass => {}
            ManifestGuardOutcome::Warn(err) => warnings.push(err),
            ManifestGuardOutcome::Reject(err) => return Err(err),
        }
    }

    Ok(warnings)
}

fn manifest_available_for_commitment(
    manifest_cache: &mut ManifestSpoolCache,
    spool_dir: &Path,
    record: &DaCommitmentRecord,
    policy: DaManifestPolicy,
) -> (bool, CacheOutcome) {
    let (outcome, cache_outcome) =
        manifest_guard_outcome(manifest_cache, spool_dir, record, policy);
    let allowed = match outcome {
        ManifestGuardOutcome::Pass => true,
        ManifestGuardOutcome::Warn(err) => {
            warn!(
                ?err,
                lane = record.lane_id.as_u32(),
                epoch = record.epoch,
                sequence = record.sequence,
                ?policy,
                "proceeding with DA commitment without a verified manifest (audit-only lane)"
            );
            true
        }
        ManifestGuardOutcome::Reject(err) => {
            warn!(
                ?err,
                lane = record.lane_id.as_u32(),
                epoch = record.epoch,
                sequence = record.sequence,
                "dropping DA commitment: manifest guard failed"
            );
            false
        }
    };
    (allowed, cache_outcome)
}

fn validate_da_bundle_caps(
    bundle: &DaCommitmentBundle,
    max_commitments: usize,
    max_openings: usize,
) -> Result<()> {
    let blob_count = bundle.commitments.len();
    if blob_count > max_commitments {
        return Err(eyre!(
            "DA bundle contains {blob_count} commitments which exceeds cap {max_commitments}"
        ));
    }

    for record in &bundle.commitments {
        let ticket_hex = hex::encode(record.storage_ticket.as_ref());
        match &record.proof_digest {
            Some(digest) => {
                let bytes = digest.as_ref();
                let is_zero_like = bytes[..Hash::LENGTH - 1].iter().all(|byte| *byte == 0)
                    && bytes[Hash::LENGTH - 1] <= 1;
                if is_zero_like {
                    return Err(eyre!(
                        "DA commitment for storage ticket {ticket_hex} has zeroed proof_digest"
                    ));
                }
            }
            None => {
                return Err(eyre!(
                    "DA commitment for storage ticket {ticket_hex} is missing proof_digest"
                ));
            }
        }
    }

    if blob_count > max_openings {
        return Err(eyre!(
            "DA proof openings cap exceeded: {blob_count} openings > {max_openings} limit"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests;
