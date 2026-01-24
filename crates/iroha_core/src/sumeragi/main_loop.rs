//! The main event loop that powers sumeragi.
#![cfg_attr(test, allow(unnameable_test_items))]
use std::{
    borrow::Cow,
    cell::Cell,
    collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry},
    convert::TryFrom,
    fs,
    num::NonZeroUsize,
    ops::Bound::{Excluded, Unbounded},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool, mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::state::StateViewContextGuard;
use blake3::{Hasher as Blake3Hasher, hash as blake3_hash};
use eyre::{Result, eyre};
use iroha_config::parameters::actual::{
    AdaptiveObservability, Common as CommonConfig, ConsensusMode, DaManifestPolicy,
    LaneConfig as LaneConfigSnapshot, NodeRole, Sumeragi as SumeragiConfig, SumeragiNposTimeouts,
};
use iroha_crypto::{Hash, HashOf, MerkleTree, PrivateKey, PublicKey, Signature};
use iroha_data_model::{
    ChainId, Encode as _,
    account::AccountId,
    block::{
        BlockHeader, BlockSignature, SignedBlock,
        consensus::{RbcReadySignature, SumeragiMembershipStatus},
    },
    consensus::{
        VALIDATOR_SET_HASH_VERSION_V1, ValidatorElectionOutcome, ValidatorSetCheckpoint,
        VrfEpochRecord, VrfLateRevealRecord, VrfParticipantRecord,
    },
    da::{
        commitment::DaCommitmentRecord,
        prelude::{DaCommitmentBundle, DaPinIntentBundle},
        types::StorageTicketId,
    },
    events::{EventBox, pipeline::PipelineEventBox},
    isi::register::RegisterPeerWithPop,
    merge::{MergeCommitteeSignature, MergeQuorumCertificate},
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    peer::PeerId,
    sorafs::pin_registry::ManifestDigest,
    transaction::{Executable, SignedTransaction},
};
use iroha_logger::prelude::*;
use iroha_p2p::network::data_frame_wire_len;
use iroha_p2p::{Broadcast, Post, Priority, UpdateTopology, frame_plaintext_cap};
use iroha_primitives::numeric::Numeric;
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

use iroha_data_model::consensus::Qc;
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
    exec::{parent_state_from_witness, post_state_from_witness},
    penalties::PenaltyApplier,
    rbc_status,
    stake_snapshot::{
        CommitStakeSnapshot, commit_stake_snapshot_from_map, fallback_stake_for_world,
        stake_map_from_world, stake_quorum_reached_for_snapshot,
    },
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
    state::{
        CellVecExt, StakeSnapshot, State, StateView, WorldReadOnly,
        compute_confidential_feature_digest,
    },
    sumeragi::evidence::EvidenceStore,
    telemetry::{MissingBlockFetchOutcome, MissingBlockFetchTargetKind},
    tx::AcceptedTransaction,
};

mod background;
mod block_sync;
mod commit;
mod kura;
mod locked_qc;
mod mode;
mod pacing;
mod pending_block;
mod pending_rbc;
mod proposal_handlers;
mod proposals;
mod propose;
mod qc;
mod rbc;
mod reschedule;

static WARNED_IGNORED_CONFIG_OVERRIDES: AtomicBool = AtomicBool::new(false);
mod roster;
mod validation;
mod votes;
mod vrf;

use locked_qc::{
    LockedQcRejection, ensure_locked_qc_allows, qc_extends_locked_if_present,
    realign_locked_to_committed_if_extends,
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
use proposals::{ProposalCache, detect_proposal_mismatch, evidence_within_horizon};
use roster::{
    apply_roster_indices_to_manager, canonicalize_roster, compute_membership_view_hash,
    compute_roster_indices_from_topology, derive_active_topology_for_mode,
    derive_active_topology_from_views, derive_local_validator_index_for_mode,
    roster_member_allowed_bls,
};
#[cfg(test)]
use roster::{derive_active_topology, derive_local_validator_index};
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
/// Multiplier for consensus rebroadcasts when block times are sub-second.
const REBROADCAST_COOLDOWN_MULTIPLIER_FAST: u32 = 1;
/// Block-time threshold (ms) for tightening rebroadcast cadence.
const REBROADCAST_COOLDOWN_FAST_THRESHOLD_MS: u64 = 1_000;
/// Payload rebroadcasts (block payloads/RBC chunks) are heavier, so keep them slower.
const PAYLOAD_REBROADCAST_COOLDOWN_MULTIPLIER: u32 = 2;
/// Cap the number of missing READY senders logged per deferral.
const READY_MISSING_LOG_LIMIT: usize = 8;
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
    let fast_threshold = Duration::from_millis(REBROADCAST_COOLDOWN_FAST_THRESHOLD_MS);
    let multiplier = if block_time != Duration::ZERO && block_time < fast_threshold {
        REBROADCAST_COOLDOWN_MULTIPLIER_FAST
    } else {
        REBROADCAST_COOLDOWN_MULTIPLIER
    };
    saturating_mul_duration(base, multiplier)
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
    view_window: u64,
    now: Instant,
) -> u64 {
    let next_view = view.saturating_add(1);
    phase_tracker.on_view_change(height, next_view, now);
    new_view_tracker.remove(height, view);
    let min_view = if view_window == 0 {
        next_view
    } else {
        next_view.saturating_sub(view_window)
    };
    new_view_tracker.drop_below_view(height, min_view);
    pacemaker.next_deadline = now;
    next_view
}

const PROPOSAL_CACHE_LIMIT: usize = 128;
const PROPOSALS_SEEN_HEIGHT_WINDOW: u64 = PROPOSAL_CACHE_LIMIT as u64;
const PROPOSALS_SEEN_VIEW_WINDOW: u64 = PROPOSAL_CACHE_LIMIT as u64;
const VOTE_CACHE_HEIGHT_WINDOW: u64 = PROPOSAL_CACHE_LIMIT as u64;
const VOTE_CACHE_VIEW_WINDOW: u64 = PROPOSAL_CACHE_LIMIT as u64;
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

#[cfg(test)]
fn new_view_target(
    highest_qc: crate::sumeragi::consensus::QcHeaderRef,
    target_height: Option<u64>,
    target_view: Option<u64>,
) -> (u64, u64) {
    let height = target_height.unwrap_or_else(|| highest_qc.height.saturating_add(1));
    let view = target_view.unwrap_or_else(|| {
        if height > highest_qc.height {
            0
        } else {
            highest_qc.view.saturating_add(1)
        }
    });
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

// Avoid bootstrap deadlocks by allowing initial proposals before any online peers are reported.
fn should_defer_for_online_peers(
    online_total: usize,
    required: usize,
    offline_grace_expired: bool,
    online_peers: usize,
    last_successful_proposal: Option<Instant>,
) -> bool {
    if online_total >= required || offline_grace_expired {
        return false;
    }
    if online_peers == 0 && last_successful_proposal.is_none() {
        return false;
    }
    true
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
    #[error("QC mode tag does not match active consensus mode")]
    ModeTagMismatch,
    #[error("QC validator set does not match active roster")]
    ValidatorSetMismatch,
    #[error("QC view does not match the block view (expected {expected}, got {actual})")]
    ViewMismatch { expected: u64, actual: u64 },
    #[error("QC aggregate does not match subject/bitmap")]
    AggregateMismatch,
    #[error("QC subject mismatch for signer {signer}")]
    SubjectMismatch { signer: ValidatorIndex },
    #[error("NEW_VIEW QC highest certificate mismatch")]
    HighestQcMismatch,
    #[error("QC contains an invalid signature from signer {signer}")]
    InvalidSignature { signer: ValidatorIndex },
    #[error("QC signer {signer} not present in block signatures")]
    SignerMissingFromBlock { signer: ValidatorIndex },
    #[error("QC is missing a stake snapshot required for NPoS quorum checks")]
    StakeSnapshotUnavailable,
    #[error("QC does not reach stake quorum")]
    StakeQuorumMissing,
}

impl QcValidationError {
    fn telemetry_reason(&self) -> &'static str {
        match self {
            Self::BitmapLengthMismatch { .. } => "bitmap_length_mismatch",
            Self::SignerOutOfBounds { .. } => "signer_out_of_bounds",
            Self::InsufficientSigners { .. } => "insufficient_signers",
            Self::MissingVotes { .. } => "missing_votes",
            Self::DuplicateSigners => "duplicate_signers",
            Self::ModeTagMismatch => "mode_tag_mismatch",
            Self::ValidatorSetMismatch => "validator_set_mismatch",
            Self::ViewMismatch { .. } => "view_mismatch",
            Self::AggregateMismatch => "aggregate_mismatch",
            Self::SubjectMismatch { .. } => "subject_mismatch",
            Self::HighestQcMismatch => "highest_qc_mismatch",
            Self::InvalidSignature { .. } => "invalid_signature",
            Self::SignerMissingFromBlock { .. } => "signer_missing_from_block",
            Self::StakeSnapshotUnavailable => "stake_snapshot_unavailable",
            Self::StakeQuorumMissing => "stake_quorum_missing",
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
/// Duration to suppress repeated RBC mismatch logs for the same peer/kind.
const RBC_MISMATCH_LOG_THROTTLE: Duration = Duration::from_secs(5);
/// Retain RBC mismatch log entries for this long before pruning.
const RBC_MISMATCH_LOG_RETENTION: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum InvalidSigKind {
    Vote,
    RbcInit,
    RbcReady,
    RbcDeliver,
}

impl InvalidSigKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Vote => "vote",
            Self::RbcInit => "rbc_init",
            Self::RbcReady => "rbc_ready",
            Self::RbcDeliver => "rbc_deliver",
        }
    }

    #[cfg(feature = "telemetry")]
    fn label(self) -> &'static str {
        self.as_str()
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RbcMismatchLogOutcome {
    Logged,
    Throttled,
}

impl RbcMismatchLogOutcome {
    fn should_log(self) -> bool {
        matches!(self, Self::Logged)
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RbcMismatchLogKey {
    kind: super::status::RbcMismatchKind,
    peer: PeerId,
}

#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_field_names)]
struct RbcMismatchLogState {
    last_logged_at: Instant,
    last_height: u64,
    last_view: u64,
}

#[derive(Default)]
struct RbcMismatchThrottle {
    entries: BTreeMap<RbcMismatchLogKey, RbcMismatchLogState>,
}

impl RbcMismatchThrottle {
    fn record(
        &mut self,
        peer: &PeerId,
        kind: super::status::RbcMismatchKind,
        height: u64,
        view: u64,
        now: Instant,
    ) -> RbcMismatchLogOutcome {
        self.entries.retain(|_, entry| {
            now.saturating_duration_since(entry.last_logged_at) <= RBC_MISMATCH_LOG_RETENTION
        });

        let key = RbcMismatchLogKey {
            kind,
            peer: peer.clone(),
        };
        match self.entries.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(RbcMismatchLogState {
                    last_logged_at: now,
                    last_height: height,
                    last_view: view,
                });
                RbcMismatchLogOutcome::Logged
            }
            Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                let elapsed = now.saturating_duration_since(state.last_logged_at);
                let advanced_height = height > state.last_height
                    || (height == state.last_height && view > state.last_view);
                if elapsed >= RBC_MISMATCH_LOG_THROTTLE || advanced_height {
                    *state = RbcMismatchLogState {
                        last_logged_at: now,
                        last_height: height,
                        last_view: view,
                    };
                    RbcMismatchLogOutcome::Logged
                } else {
                    RbcMismatchLogOutcome::Throttled
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct InvalidSigPenaltyEntry {
    window_start: Instant,
    count: u32,
    last_seen: Instant,
    suppressed_until: Option<Instant>,
}

struct InvalidSigPenalty {
    threshold: u32,
    window: Duration,
    cooldown: Duration,
    entries: BTreeMap<InvalidSigKey, InvalidSigPenaltyEntry>,
}

impl InvalidSigPenalty {
    fn new(config: &SumeragiConfig) -> Self {
        Self {
            threshold: config.gating.invalid_sig_penalty_threshold,
            window: config.gating.invalid_sig_penalty_window,
            cooldown: config.gating.invalid_sig_penalty_cooldown,
            entries: BTreeMap::new(),
        }
    }

    fn is_suppressed(&mut self, kind: InvalidSigKind, signer: u64, now: Instant) -> bool {
        if self.threshold == 0 {
            return false;
        }
        self.prune(now);
        let key = InvalidSigKey { kind, signer };
        let Some(entry) = self.entries.get_mut(&key) else {
            return false;
        };
        entry.last_seen = now;
        if let Some(until) = entry.suppressed_until {
            if now < until {
                return true;
            }
            entry.suppressed_until = None;
            entry.count = 0;
            entry.window_start = now;
        }
        false
    }

    fn record(&mut self, kind: InvalidSigKind, signer: u64, now: Instant) -> bool {
        if self.threshold == 0 {
            return false;
        }
        self.prune(now);
        let key = InvalidSigKey { kind, signer };
        let entry = self.entries.entry(key).or_insert(InvalidSigPenaltyEntry {
            window_start: now,
            count: 0,
            last_seen: now,
            suppressed_until: None,
        });
        entry.last_seen = now;
        if let Some(until) = entry.suppressed_until {
            if now < until {
                return false;
            }
            entry.suppressed_until = None;
            entry.count = 0;
            entry.window_start = now;
        }
        if now.saturating_duration_since(entry.window_start) > self.window {
            entry.window_start = now;
            entry.count = 0;
        }
        entry.count = entry.count.saturating_add(1);
        if entry.count >= self.threshold {
            entry.count = 0;
            entry.window_start = now;
            if !self.cooldown.is_zero() {
                entry.suppressed_until = Some(now.checked_add(self.cooldown).unwrap_or(now));
            }
            return true;
        }
        false
    }

    fn prune(&mut self, now: Instant) {
        let retention = self.window.saturating_add(self.cooldown);
        self.entries
            .retain(|_, entry| now.saturating_duration_since(entry.last_seen) <= retention);
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
        | QcValidationError::ModeTagMismatch
        | QcValidationError::ValidatorSetMismatch
        | QcValidationError::ViewMismatch { .. }
        | QcValidationError::AggregateMismatch
        | QcValidationError::DuplicateSigners
        | QcValidationError::HighestQcMismatch
        | QcValidationError::SubjectMismatch { .. } => Some(Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidQc {
                certificate: qc.clone(),
                reason: qc_validation_reason(err).to_owned(),
            },
        }),
        QcValidationError::InsufficientSigners { .. }
        | QcValidationError::MissingVotes { .. }
        | QcValidationError::StakeSnapshotUnavailable
        | QcValidationError::StakeQuorumMissing => None,
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

#[allow(clippy::too_many_arguments)]
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
    world: &impl WorldReadOnly,
    pops: &BTreeMap<PublicKey, Vec<u8>>,
    chain_id: &ChainId,
    consensus_mode: ConsensusMode,
    stake_snapshot: Option<&CommitStakeSnapshot>,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> (
    Result<QcValidationOutcome, QcValidationError>,
    Option<crate::sumeragi::consensus::Evidence>,
) {
    match validate_qc_against_votes(
        vote_log,
        qc,
        topology,
        world,
        pops,
        chain_id,
        consensus_mode,
        stake_snapshot,
        mode_tag,
        prf_seed,
    ) {
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

fn signer_peers_for_topology(
    signers: &BTreeSet<ValidatorIndex>,
    topology: &super::network_topology::Topology,
) -> Result<BTreeSet<PeerId>, QcValidationError> {
    let roster_len = topology.as_ref().len();
    let mut peers = BTreeSet::new();
    for signer in signers {
        let idx = usize::try_from(*signer).unwrap_or(usize::MAX);
        let Some(peer) = topology.as_ref().get(idx) else {
            return Err(QcValidationError::SignerOutOfBounds {
                signer: idx,
                topology_len: roster_len,
            });
        };
        peers.insert(peer.clone());
    }
    Ok(peers)
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

fn compute_topology_rotation_offset(
    base: &super::network_topology::Topology,
    rotated: &super::network_topology::Topology,
) -> Option<usize> {
    if base.as_ref().is_empty() || rotated.as_ref().is_empty() {
        return Some(0);
    }
    // Assumes rotated is a rotation of base.
    // Find where rotated[0] is in base.
    let leader = &rotated.as_ref()[0];
    base.position(leader.public_key())
}

fn map_canonical_to_view_index(
    canonical_idx: ValidatorIndex,
    rotation_offset: usize,
    topology_len: usize,
) -> Option<ValidatorIndex> {
    let c = usize::try_from(canonical_idx).ok()?;
    if c >= topology_len {
        return None;
    }
    // rotated left by offset.
    // view_idx = (c + len - offset) % len
    let v = (c + topology_len - rotation_offset) % topology_len;
    ValidatorIndex::try_from(v).ok()
}

#[cfg(test)]
fn view_index_for_canonical_signer(
    canonical_idx: ValidatorIndex,
    signature_topology: &super::network_topology::Topology,
    canonical_topology: &super::network_topology::Topology,
) -> Option<ValidatorIndex> {
    let roster_len = canonical_topology.as_ref().len();
    if roster_len == 0 {
        return None;
    }
    let offset = compute_topology_rotation_offset(canonical_topology, signature_topology)?;
    map_canonical_to_view_index(canonical_idx, offset, roster_len)
}

fn normalize_signer_indices_to_view(
    signers: &BTreeSet<ValidatorIndex>,
    signature_topology: &super::network_topology::Topology,
    canonical_topology: &super::network_topology::Topology,
) -> BTreeSet<ValidatorIndex> {
    if signers.is_empty() {
        return BTreeSet::new();
    }
    let roster_len = canonical_topology.as_ref().len();
    if roster_len == 0 {
        return BTreeSet::new();
    }
    let offset = match compute_topology_rotation_offset(canonical_topology, signature_topology) {
        Some(offset) => offset,
        None => return BTreeSet::new(),
    };
    let mut normalized = BTreeSet::new();
    for signer in signers {
        if let Some(view_idx) = map_canonical_to_view_index(*signer, offset, roster_len) {
            normalized.insert(view_idx);
        }
    }
    normalized
}

fn select_new_view_highest_qc_from_votes(
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
    signers: &BTreeSet<ValidatorIndex>,
    height: u64,
    view: u64,
    epoch: u64,
) -> Option<crate::sumeragi::consensus::QcRef> {
    let mut selected: Option<crate::sumeragi::consensus::QcRef> = None;
    for signer in signers {
        let Some(vote) = vote_log.get(&(
            crate::sumeragi::consensus::Phase::NewView,
            height,
            view,
            epoch,
            *signer,
        )) else {
            continue;
        };
        let Some(candidate) = vote.highest_qc else {
            continue;
        };
        let valid_phase = matches!(
            candidate.phase,
            crate::sumeragi::consensus::Phase::Commit | crate::sumeragi::consensus::Phase::Prepare
        );
        if !valid_phase {
            continue;
        }
        selected = Some(selected.map_or(candidate, |current| {
            let incoming = (candidate.height, candidate.view);
            let existing = (current.height, current.view);
            let promotes_phase = incoming == existing
                && candidate.phase == crate::sumeragi::consensus::Phase::Commit
                && current.phase != crate::sumeragi::consensus::Phase::Commit;
            if incoming > existing || promotes_phase {
                candidate
            } else {
                current
            }
        }));
    }
    selected
}

fn qc_bls_preimage(
    qc: &crate::sumeragi::consensus::Qc,
    chain_id: &ChainId,
    mode_tag: &str,
) -> Vec<u8> {
    let vote = crate::sumeragi::consensus::Vote {
        phase: qc.phase,
        block_hash: qc.subject_block_hash,
        parent_state_root: qc.parent_state_root,
        post_state_root: qc.post_state_root,
        height: qc.height,
        view: qc.view,
        epoch: qc.epoch,
        highest_qc: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    vote_preimage(chain_id, mode_tag, &vote)
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
    pops: &BTreeMap<PublicKey, Vec<u8>>,
    chain_id: &ChainId,
    mode_tag: &str,
) -> bool {
    if qc.mode_tag != mode_tag {
        return false;
    }
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
    let mut public_keys: Vec<&PublicKey> = Vec::with_capacity(parsed_signers.present.len());
    let mut signer_pops: Vec<Vec<u8>> = Vec::with_capacity(parsed_signers.present.len());
    for signer in &parsed_signers.present {
        let Ok(idx) = usize::try_from(*signer) else {
            return false;
        };
        let Some(peer) = canonical_topology.as_ref().get(idx) else {
            return false;
        };
        let pk = peer.public_key();
        let Some(pop) = pops.get(pk) else {
            return false;
        };
        public_keys.push(pk);
        signer_pops.push(pop.clone());
    }
    let preimage = qc_bls_preimage(qc, chain_id, mode_tag);
    let pop_refs: Vec<&[u8]> = signer_pops.iter().map(Vec::as_slice).collect();
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &qc.aggregate.bls_aggregate_signature,
        &public_keys,
        &pop_refs,
    )
    .is_ok()
}

fn kickstart_pacemaker_after_commit<F>(
    queue_len: usize,
    backpressure: propose::ProposalBackpressure,
    mut trigger: F,
) -> bool
where
    F: FnMut(Instant) -> bool,
{
    if queue_len > 0 && !backpressure.should_defer() {
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
            topology.canonicalize_order();
            if let Some(seed) = prf_seed {
                topology.shuffle_prf(seed, height);
            } else {
                warn!(
                    height,
                    view, "skipping PRF shuffle for {context}: missing seed"
                );
            }
            topology.nth_rotation(view);
        }
        NPOS_TAG => {
            topology.canonicalize_order();
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
    let view = block.header().view_change_index();
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

fn resolve_stake_snapshot_for_roster(
    stake_snapshot: Option<&CommitStakeSnapshot>,
    world: &impl WorldReadOnly,
    roster: &[PeerId],
) -> Option<CommitStakeSnapshot> {
    if let Some(snapshot) = stake_snapshot {
        if snapshot.matches_roster(roster) {
            return Some(snapshot.clone());
        }
        warn!(
            roster_len = roster.len(),
            "stake snapshot roster mismatch; recomputing"
        );
    }
    CommitStakeSnapshot::from_roster(world, roster)
}

fn resolve_stake_snapshot_for_roster_from_cache(
    stake_snapshot: Option<&CommitStakeSnapshot>,
    roster: &[PeerId],
    stake_map: &BTreeMap<PeerId, Numeric>,
    fallback_stake: &Numeric,
) -> Option<CommitStakeSnapshot> {
    if let Some(snapshot) = stake_snapshot {
        if snapshot.matches_roster(roster) {
            return Some(snapshot.clone());
        }
        warn!(
            roster_len = roster.len(),
            "stake snapshot roster mismatch; recomputing"
        );
    }
    commit_stake_snapshot_from_map(roster, stake_map, fallback_stake)
}

#[derive(Debug, Clone)]
struct RosterValidationInputs {
    pops: BTreeMap<PublicKey, Vec<u8>>,
    stake_snapshot: Option<CommitStakeSnapshot>,
}

impl RosterValidationInputs {
    fn from_cache(
        cache: &RosterValidationCache,
        roster: &[PeerId],
        consensus_mode: ConsensusMode,
        stake_snapshot: Option<&CommitStakeSnapshot>,
    ) -> Self {
        let stake_snapshot = match consensus_mode {
            ConsensusMode::Permissioned => None,
            ConsensusMode::Npos => resolve_stake_snapshot_for_roster_from_cache(
                stake_snapshot,
                roster,
                &cache.stake_map,
                &cache.fallback_stake,
            ),
        };
        let mut pops = BTreeMap::new();
        for peer in roster {
            if let Some(pop) = cache.pops.get(peer.public_key()) {
                pops.insert(peer.public_key().clone(), pop.clone());
            }
        }
        Self {
            pops,
            stake_snapshot,
        }
    }

    #[cfg(test)]
    fn from_world(
        world: &impl WorldReadOnly,
        roster: &[PeerId],
        consensus_mode: ConsensusMode,
        stake_snapshot: Option<&CommitStakeSnapshot>,
    ) -> Self {
        let stake_map = stake_map_from_world(world);
        let fallback_stake = fallback_stake_for_world(world);
        let stake_snapshot = match consensus_mode {
            ConsensusMode::Permissioned => None,
            ConsensusMode::Npos => resolve_stake_snapshot_for_roster_from_cache(
                stake_snapshot,
                roster,
                &stake_map,
                &fallback_stake,
            ),
        };
        let mut pops_by_key = BTreeMap::new();
        for (_id, record) in world.consensus_keys().iter() {
            if let Some(pop) = record.pop.as_ref() {
                pops_by_key
                    .entry(record.public_key.clone())
                    .or_insert_with(|| pop.clone());
            }
        }
        let mut pops = BTreeMap::new();
        for peer in roster {
            if let Some(pop) = pops_by_key.get(peer.public_key()) {
                pops.insert(peer.public_key().clone(), pop.clone());
            }
        }
        Self {
            pops,
            stake_snapshot,
        }
    }
}

#[derive(Debug, Clone)]
struct RosterValidationCache {
    epoch_schedule: super::EpochScheduleSnapshot,
    pops: BTreeMap<PublicKey, Vec<u8>>,
    stake_map: BTreeMap<PeerId, Numeric>,
    fallback_stake: Numeric,
}

impl RosterValidationCache {
    fn from_world(
        world: &impl WorldReadOnly,
        fallback_epoch_length: u64,
        fallback_pops: Option<&BTreeMap<PublicKey, Vec<u8>>>,
    ) -> Self {
        let epoch_schedule =
            super::EpochScheduleSnapshot::from_world_with_fallback(world, fallback_epoch_length);
        let mut pops = BTreeMap::new();
        for (_id, record) in world.consensus_keys().iter() {
            if let Some(pop) = record.pop.as_ref() {
                pops.entry(record.public_key.clone())
                    .or_insert_with(|| pop.clone());
            }
        }
        if let Some(fallback_pops) = fallback_pops {
            for (pk, pop) in fallback_pops {
                pops.entry(pk.clone()).or_insert_with(|| pop.clone());
            }
        }
        let stake_map = stake_map_from_world(world);
        let fallback_stake = fallback_stake_for_world(world);
        Self {
            epoch_schedule,
            pops,
            stake_map,
            fallback_stake,
        }
    }

    fn refresh_from_world(
        &mut self,
        world: &impl WorldReadOnly,
        fallback_epoch_length: u64,
        fallback_pops: Option<&BTreeMap<PublicKey, Vec<u8>>>,
    ) {
        *self = Self::from_world(world, fallback_epoch_length, fallback_pops);
    }

    fn expected_epoch(&self, height: u64, consensus_mode: ConsensusMode) -> u64 {
        if matches!(consensus_mode, ConsensusMode::Permissioned) {
            return 0;
        }
        self.epoch_schedule.epoch_for_height(height)
    }

    fn inputs_for_roster(
        &self,
        roster: &[PeerId],
        consensus_mode: ConsensusMode,
        stake_snapshot: Option<&CommitStakeSnapshot>,
    ) -> RosterValidationInputs {
        RosterValidationInputs::from_cache(self, roster, consensus_mode, stake_snapshot)
    }

    fn stake_snapshot_for_roster(&self, roster: &[PeerId]) -> Option<CommitStakeSnapshot> {
        commit_stake_snapshot_from_map(roster, &self.stake_map, &self.fallback_stake)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_block_sync_qc(
    qc: &crate::sumeragi::consensus::Qc,
    topology: &super::network_topology::Topology,
    world: &impl WorldReadOnly,
    block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
    block_view: u64,
    pops: &BTreeMap<PublicKey, Vec<u8>>,
    chain_id: &ChainId,
    consensus_mode: ConsensusMode,
    stake_snapshot: Option<&CommitStakeSnapshot>,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Result<(BTreeSet<crate::sumeragi::consensus::ValidatorIndex>, usize), QcValidationError> {
    let signature_topology = topology_for_view(topology, qc.height, qc.view, mode_tag, prf_seed);
    let roster_len = topology.as_ref().len();
    let required = signature_topology.min_votes_for_commit().max(1);
    let voting_len = roster_len;
    if qc.mode_tag != mode_tag {
        return Err(QcValidationError::ModeTagMismatch);
    }
    if mode_tag == PERMISSIONED_TAG && qc.view != block_view {
        return Err(QcValidationError::ViewMismatch {
            expected: block_view,
            actual: qc.view,
        });
    }
    if !qc_validator_set_matches_topology(qc, topology) {
        return Err(QcValidationError::ValidatorSetMismatch);
    }
    let parsed_signers = qc_signer_indices(qc, roster_len, voting_len)?;
    // Normalize block signer indices to canonical topology so view rotations align with QC bitmaps.
    let block_signature_topology =
        topology_for_view(topology, qc.height, block_view, mode_tag, prf_seed);
    let block_signers_canonical =
        normalize_signer_indices_to_canonical(block_signers, &block_signature_topology, topology);
    if block_signers_canonical.len() >= parsed_signers.voting.len() {
        if let Some(missing) = parsed_signers
            .voting
            .iter()
            .find(|signer| !block_signers_canonical.contains(*signer))
        {
            return Err(QcValidationError::SignerMissingFromBlock { signer: *missing });
        }
    }
    let resolved_snapshot = match consensus_mode {
        ConsensusMode::Permissioned => None,
        ConsensusMode::Npos => {
            resolve_stake_snapshot_for_roster(stake_snapshot, world, topology.as_ref())
        }
    };
    match consensus_mode {
        ConsensusMode::Permissioned => {
            if parsed_signers.voting.len() < required {
                return Err(QcValidationError::InsufficientSigners {
                    collected: parsed_signers.voting.len(),
                    required,
                });
            }
        }
        ConsensusMode::Npos => {
            let snapshot = resolved_snapshot
                .as_ref()
                .ok_or(QcValidationError::StakeSnapshotUnavailable)?;
            let signer_peers = signer_peers_for_topology(&parsed_signers.voting, topology)?;
            match stake_quorum_reached_for_snapshot(snapshot, topology.as_ref(), &signer_peers) {
                Ok(true) => {}
                Ok(false) => return Err(QcValidationError::StakeQuorumMissing),
                Err(_) => return Err(QcValidationError::StakeSnapshotUnavailable),
            }
        }
    }
    if !qc_aggregate_consistent(qc, topology, pops, chain_id, mode_tag) {
        return Err(QcValidationError::AggregateMismatch);
    }
    // Return the raw bitmap indices so cached signers reproduce the QC bitmap correctly.
    Ok((parsed_signers.voting, parsed_signers.present.len()))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn derive_block_sync_qc_from_signers(
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    block_view: u64,
    block_epoch: u64,
    parent_state_root: Hash,
    post_state_root: Hash,
    commit_topology: &[PeerId],
    consensus_mode: ConsensusMode,
    stake_snapshot: Option<&CommitStakeSnapshot>,
    mode_tag: &str,
    block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
    aggregate_signature: Vec<u8>,
) -> Option<crate::sumeragi::consensus::Qc> {
    let roster_len = commit_topology.len();
    if roster_len == 0 {
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
    match consensus_mode {
        ConsensusMode::Permissioned => {
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
        }
        ConsensusMode::Npos => {
            let snapshot = stake_snapshot?;
            let mut signer_peers = BTreeSet::new();
            for signer in block_signers {
                let Ok(idx) = usize::try_from(*signer) else {
                    return None;
                };
                let peer = commit_topology.get(idx)?;
                signer_peers.insert(peer.clone());
            }
            match stake_quorum_reached_for_snapshot(snapshot, commit_topology, &signer_peers) {
                Ok(true) => {}
                Ok(false) => {
                    warn!(
                        incoming_hash = %block_hash,
                        block_signers = block_signers.len(),
                        "dropping derived block sync QC: insufficient stake quorum"
                    );
                    return None;
                }
                Err(_) => {
                    warn!(
                        incoming_hash = %block_hash,
                        block_signers = block_signers.len(),
                        "dropping derived block sync QC: stake snapshot unavailable"
                    );
                    return None;
                }
            }
        }
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
        parent_state_root,
        post_state_root,
        height: block_height,
        view: block_view,
        epoch: block_epoch,
        mode_tag: mode_tag.to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set,
        aggregate: crate::sumeragi::consensus::QcAggregate {
            signers_bitmap,
            bls_aggregate_signature: aggregate_signature,
        },
    })
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
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
    world: &impl WorldReadOnly,
    pops: &BTreeMap<PublicKey, Vec<u8>>,
    chain_id: &ChainId,
    consensus_mode: ConsensusMode,
    stake_snapshot: Option<&CommitStakeSnapshot>,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Result<QcSignerTally, QcValidationError> {
    validate_qc_against_votes(
        vote_log,
        qc,
        topology,
        world,
        pops,
        chain_id,
        consensus_mode,
        stake_snapshot,
        mode_tag,
        prf_seed,
    )
    .map(
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

#[allow(clippy::too_many_arguments)]
fn tally_qc_against_block_signers(
    qc: &crate::sumeragi::consensus::Qc,
    topology: &super::network_topology::Topology,
    world: &impl WorldReadOnly,
    block_signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
    block_view: u64,
    pops: &BTreeMap<PublicKey, Vec<u8>>,
    chain_id: &ChainId,
    consensus_mode: ConsensusMode,
    stake_snapshot: Option<&CommitStakeSnapshot>,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Result<QcSignerTally, QcValidationError> {
    let _ = validate_block_sync_qc(
        qc,
        topology,
        world,
        block_signers,
        block_view,
        pops,
        chain_id,
        consensus_mode,
        stake_snapshot,
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

fn validate_new_view_qc_highest(
    qc: &crate::sumeragi::consensus::Qc,
) -> Result<crate::sumeragi::consensus::QcRef, QcValidationError> {
    let Some(highest) = qc.highest_qc else {
        return Err(QcValidationError::HighestQcMismatch);
    };
    let valid_phase = matches!(highest.phase, crate::sumeragi::consensus::Phase::Commit)
        || matches!(highest.phase, crate::sumeragi::consensus::Phase::Prepare);
    if !valid_phase {
        return Err(QcValidationError::HighestQcMismatch);
    }
    if highest.subject_block_hash != qc.subject_block_hash {
        return Err(QcValidationError::HighestQcMismatch);
    }
    let expected_height = highest.height.saturating_add(1);
    if qc.height != expected_height {
        return Err(QcValidationError::HighestQcMismatch);
    }
    if highest.epoch > qc.epoch {
        return Err(QcValidationError::HighestQcMismatch);
    }
    Ok(highest)
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
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
    world: &impl WorldReadOnly,
    pops: &BTreeMap<PublicKey, Vec<u8>>,
    chain_id: &ChainId,
    consensus_mode: ConsensusMode,
    stake_snapshot: Option<&CommitStakeSnapshot>,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Result<QcValidationOutcome, QcValidationError> {
    let signature_topology = topology_for_view(topology, qc.height, qc.view, mode_tag, prf_seed);
    let roster_len = topology.as_ref().len();
    let required = signature_topology.min_votes_for_commit();
    let voting_len = roster_len;
    let qc_highest = if qc.phase == crate::sumeragi::consensus::Phase::NewView {
        Some(validate_new_view_qc_highest(qc)?)
    } else {
        None
    };
    if qc.mode_tag != mode_tag {
        return Err(QcValidationError::ModeTagMismatch);
    }
    if !qc_validator_set_matches_topology(qc, topology) {
        return Err(QcValidationError::ValidatorSetMismatch);
    }
    let parsed_signers = qc_signer_indices(qc, roster_len, voting_len)?;
    match consensus_mode {
        ConsensusMode::Permissioned => {
            if parsed_signers.voting.len() < required {
                return Err(QcValidationError::InsufficientSigners {
                    collected: parsed_signers.voting.len(),
                    required,
                });
            }
        }
        ConsensusMode::Npos => {
            let resolved_snapshot =
                resolve_stake_snapshot_for_roster(stake_snapshot, world, topology.as_ref());
            let snapshot = resolved_snapshot
                .as_ref()
                .ok_or(QcValidationError::StakeSnapshotUnavailable)?;
            let signer_peers = signer_peers_for_topology(&parsed_signers.voting, topology)?;
            match stake_quorum_reached_for_snapshot(snapshot, topology.as_ref(), &signer_peers) {
                Ok(true) => {}
                Ok(false) => return Err(QcValidationError::StakeQuorumMissing),
                Err(_) => return Err(QcValidationError::StakeSnapshotUnavailable),
            }
        }
    }

    if !qc_aggregate_consistent(qc, topology, pops, chain_id, mode_tag) {
        return Err(QcValidationError::AggregateMismatch);
    }
    let mut missing = 0usize;
    let rotation_offset =
        compute_topology_rotation_offset(topology, &signature_topology).unwrap_or(0);

    for signer in &parsed_signers.voting {
        let Some(view_signer) = map_canonical_to_view_index(*signer, rotation_offset, roster_len)
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
        if let Some(qc_highest) = qc_highest {
            let Some(vote_highest) = vote.highest_qc else {
                return Err(QcValidationError::HighestQcMismatch);
            };
            let valid_phase = matches!(
                vote_highest.phase,
                crate::sumeragi::consensus::Phase::Commit
                    | crate::sumeragi::consensus::Phase::Prepare
            );
            if !valid_phase {
                return Err(QcValidationError::HighestQcMismatch);
            }
            if vote_highest.subject_block_hash != qc.subject_block_hash {
                return Err(QcValidationError::HighestQcMismatch);
            }
            let vote_rank = (
                vote_highest.height,
                vote_highest.view,
                phase_rank(vote_highest.phase),
            );
            let qc_rank = (
                qc_highest.height,
                qc_highest.view,
                phase_rank(qc_highest.phase),
            );
            if vote_rank > qc_rank {
                return Err(QcValidationError::HighestQcMismatch);
            }
            if vote_highest.height == qc_highest.height
                && vote_highest.view == qc_highest.view
                && vote_highest.epoch != qc_highest.epoch
            {
                return Err(QcValidationError::HighestQcMismatch);
            }
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

fn fallback_qc_tally_from_bitmap(
    qc: &crate::sumeragi::consensus::Qc,
    topology: &super::network_topology::Topology,
    _world: &impl WorldReadOnly,
    pops: &BTreeMap<PublicKey, Vec<u8>>,
    chain_id: &ChainId,
    mode_tag: &str,
    prf_seed: Option<[u8; 32]>,
) -> Option<QcSignerTally> {
    let _signature_topology = topology_for_view(topology, qc.height, qc.view, mode_tag, prf_seed);
    if !qc_aggregate_consistent(qc, topology, pops, chain_id, mode_tag) {
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

#[derive(Debug, Clone, Copy)]
struct StateRoots {
    parent_state_root: Hash,
    post_state_root: Hash,
}

fn exec_roots_for_state_block(
    state_block: &mut crate::state::StateBlock<'_>,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
) -> Option<StateRoots> {
    let exec_witness = state_block.take_exec_witness().or_else(|| {
        warn!(
            height,
            view,
            block = %block_hash,
            "execution witness missing after validation; capturing fallback"
        );
        state_block.capture_exec_witness();
        state_block.take_exec_witness()
    });
    exec_witness.map(|witness| StateRoots {
        parent_state_root: parent_state_from_witness(&witness),
        post_state_root: post_state_from_witness(&witness),
    })
}

/// Run stateless and stateful validation for a block before emitting votes.
fn validate_block_for_voting(
    block: SignedBlock,
    topology: &mut super::network_topology::Topology,
    chain_id: &ChainId,
    genesis_account: &AccountId,
    state: &State,
    voting_block: &mut Option<VotingBlock>,
) -> Result<Option<StateRoots>, BlockValidationError> {
    let block_hash = block.hash();
    let height = block.header().height().get();
    let view = block.header().view_change_index();
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
        Ok((_validated, mut state_block)) => {
            let roots = exec_roots_for_state_block(&mut state_block, block_hash, height, view);
            drop(state_block);
            Ok(roots)
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
    epoch: u64,
    reason: String,
) -> crate::sumeragi::consensus::Evidence {
    let proposer = proposer_index_from_block(block);
    let view = block.header().view_change_index();
    let proposal = Actor::build_consensus_proposal(block, payload_hash, qc, proposer, view, epoch);
    invalid_proposal_evidence(proposal, reason)
}

#[cfg(test)]
fn new_view_highest_qc_phase_valid(qc: &crate::sumeragi::consensus::Qc) -> bool {
    matches!(
        qc.phase,
        crate::sumeragi::consensus::Phase::Commit | crate::sumeragi::consensus::Phase::Prepare
    )
}

#[derive(Debug, Clone)]
struct MissingBlockRequest {
    height: u64,
    view: u64,
    phase: crate::sumeragi::consensus::Phase,
    priority: MissingBlockPriority,
    retry_window: Duration,
    view_change_window: Option<Duration>,
    first_seen: Instant,
    last_requested: Instant,
    view_change_triggered_view: Option<u64>,
    attempts: u32,
}

impl MissingBlockRequest {
    fn view_change_triggered_in_view(&self) -> bool {
        self.view_change_triggered_view == Some(self.view)
    }

    /// Return true once the missing-block dwell time exceeds the view-change window for the
    /// current view. Subsequent calls within the same view return false to avoid repeated
    /// view changes.
    fn mark_view_change_if_due(&mut self, now: Instant) -> bool {
        let Some(window) = self.view_change_window else {
            return false;
        };
        if self.view_change_triggered_in_view() || window == Duration::ZERO {
            return false;
        }
        let dwell = now.saturating_duration_since(self.first_seen);
        if dwell >= window {
            self.view_change_triggered_view = Some(self.view);
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MissingBlockPriority {
    Background,
    Consensus,
}

fn phase_rank(phase: crate::sumeragi::consensus::Phase) -> u8 {
    match phase {
        crate::sumeragi::consensus::Phase::Prepare => 0,
        crate::sumeragi::consensus::Phase::Commit => 1,
        crate::sumeragi::consensus::Phase::NewView => 2,
    }
}

#[allow(clippy::too_many_arguments)]
fn touch_missing_block_request(
    requests: &mut BTreeMap<HashOf<BlockHeader>, MissingBlockRequest>,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    phase: crate::sumeragi::consensus::Phase,
    priority: MissingBlockPriority,
    now: Instant,
    retry_window: Duration,
    view_change_window: Option<Duration>,
) -> bool {
    match requests.entry(block_hash) {
        Entry::Vacant(vacant) => {
            vacant.insert(MissingBlockRequest {
                height,
                view,
                phase,
                priority,
                retry_window,
                view_change_window,
                first_seen: now,
                last_requested: now,
                view_change_triggered_view: None,
                attempts: 0,
            });
            true
        }
        Entry::Occupied(mut occupied) => {
            let stats = occupied.get_mut();
            let priority_upgrade = priority > stats.priority;
            let view_advanced = view > stats.view;
            let height_advanced = height > stats.height;
            if stats.view != view {
                if view_advanced {
                    warn!(
                        height,
                        view,
                        stored_view = stats.view,
                        block = ?block_hash,
                        "updating missing-block request view after mismatch"
                    );
                    stats.view = view;
                } else {
                    debug!(
                        height,
                        view,
                        stored_view = stats.view,
                        block = ?block_hash,
                        "ignoring stale missing-block request view"
                    );
                }
            }
            if height_advanced {
                stats.height = height;
            }
            if view_advanced || height_advanced {
                stats.first_seen = now;
                stats.last_requested = now;
                stats.attempts = 0;
                if view_advanced {
                    stats.view_change_triggered_view = None;
                }
            }
            if phase_rank(phase) > phase_rank(stats.phase) {
                stats.phase = phase;
            }
            match priority.cmp(&stats.priority) {
                std::cmp::Ordering::Greater => {
                    stats.priority = priority;
                    stats.retry_window = retry_window;
                    stats.view_change_window = view_change_window;
                }
                std::cmp::Ordering::Equal => {
                    stats.retry_window = if stats.retry_window == Duration::ZERO {
                        retry_window
                    } else {
                        stats.retry_window.min(retry_window)
                    };
                    stats.view_change_window = match (stats.view_change_window, view_change_window)
                    {
                        (Some(existing), Some(window)) => Some(existing.min(window)),
                        (Some(existing), None) => Some(existing),
                        (None, Some(window)) => Some(window),
                        (None, None) => None,
                    };
                }
                std::cmp::Ordering::Less => {}
            }
            let retry_due = view_advanced
                || height_advanced
                || priority_upgrade
                || now.saturating_duration_since(stats.last_requested) >= stats.retry_window;
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
    view: u64,
    phase: crate::sumeragi::consensus::Phase,
    priority: MissingBlockPriority,
    signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
    topology: &super::network_topology::Topology,
    now: Instant,
    retry_window: Duration,
    view_change_window: Option<Duration>,
    signer_fallback_attempts: u32,
) -> MissingBlockFetchDecision {
    if !touch_missing_block_request(
        requests,
        block_hash,
        height,
        view,
        phase,
        priority,
        now,
        retry_window,
        view_change_window,
    ) {
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
    qc_evidence_present: bool,
    commit_cert_present: bool,
    checkpoint_present: bool,
    missing_block_requested: bool,
    block_height: u64,
    local_height: u64,
) -> bool {
    // Require commit evidence unless we explicitly requested the next missing payload.
    if commit_cert_present || qc_evidence_present || signature_quorum_met || checkpoint_present {
        return true;
    }

    if block_signers == 0 {
        return false;
    }

    if missing_block_requested {
        return block_height <= local_height.saturating_add(1);
    }

    block_height <= local_height.saturating_add(1)
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
    view_change_window: Option<Duration>,
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
        view,
        phase,
        MissingBlockPriority::Consensus,
        signers,
        topology,
        now,
        retry_window,
        view_change_window,
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
            &self.roster_validation_cache,
        )
        .or_else(|| {
            block_sync_history_roster_for_block(
                consensus_mode,
                parent_hash,
                parent_height,
                None,
                &self.chain_id,
                &self.roster_validation_cache,
            )
        }) {
            (selection.roster, selection.source.as_str())
        } else if let Some(hint) = roster_hint.filter(|hint| !hint.is_empty()) {
            (hint.to_vec(), "roster_hint")
        } else if !commit_topology.is_empty() {
            (commit_topology.to_vec(), "commit_topology")
        } else {
            let view = self.state.view();
            let roster = derive_active_topology_for_mode(
                &view,
                self.common_config.trusted_peers.value(),
                self.common_config.peer.id(),
                consensus_mode,
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
        let view_change_window = if gap == 1 { Some(retry_window) } else { None };
        let mut requests = core::mem::take(&mut self.pending.missing_block_requests);
        let decision = plan_missing_block_fetch(
            &mut requests,
            parent_hash,
            parent_height,
            block_view,
            crate::sumeragi::consensus::Phase::Commit,
            MissingBlockPriority::Background,
            &signers,
            &topology,
            now,
            retry_window,
            view_change_window,
            self.config.recovery.missing_block_signer_fallback_attempts,
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
        let dwell_ms = dwell.as_millis().try_into().unwrap_or(u64::MAX);
        self.note_missing_block_fetch_metrics(&decision, retry_window, targets_len, dwell);
        super::status::record_missing_block_fetch(targets_len, dwell_ms);

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
                pending.block.header().view_change_index(),
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
    senders: BTreeSet<PeerId>,
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

    #[cfg(test)]
    fn count_with_local(&self, local: Option<&PeerId>) -> usize {
        let mut count = self.senders.len();
        if let Some(peer) = local {
            if !self.senders.contains(peer) {
                count = count.saturating_add(1);
            }
        }
        count
    }

    fn count_in_roster(&self, roster: &BTreeSet<PeerId>, local: Option<&PeerId>) -> usize {
        let mut count = self
            .senders
            .iter()
            .filter(|peer| roster.contains(*peer))
            .count();
        if let Some(peer) = local {
            if roster.contains(peer) && !self.senders.contains(peer) {
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
        sender: PeerId,
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
    fn count_with_local(&self, height: u64, view: u64, local: Option<&PeerId>) -> usize {
        self.entries
            .get(&(height, view))
            .map_or(0, |entry| entry.count_with_local(local))
    }

    fn prune(&mut self, committed_height: u64) {
        self.entries
            .retain(|(entry_height, _), _| *entry_height > committed_height);
    }

    fn drop_below_height(&mut self, min_height: u64) {
        self.entries
            .retain(|(entry_height, _), _| *entry_height >= min_height);
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
        local: Option<&PeerId>,
        roster: &[PeerId],
    ) -> Option<NewViewSelection> {
        if roster.is_empty() {
            return None;
        }
        let roster_set: BTreeSet<_> = roster.iter().cloned().collect();
        self.highest_entry_mut(|_, _, entry| entry.count_in_roster(&roster_set, local) >= required)
            .map(|(key, entry)| {
                let quorum = entry.count_in_roster(&roster_set, local);
                if let Some(peer) = local {
                    if roster_set.contains(peer) {
                        entry.senders.insert(peer.clone());
                    }
                }
                NewViewSelection {
                    key,
                    highest_qc: entry.highest_qc,
                    quorum,
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
    HighestQcView,
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

fn non_rbc_payload_budget(
    block_max_payload_bytes: Option<NonZeroUsize>,
    payload_frame_cap: usize,
) -> usize {
    let frame_cap = payload_frame_cap.saturating_sub(NON_RBC_FRAME_HEADROOM_BYTES);
    let config_cap = block_max_payload_bytes.map_or(frame_cap, NonZeroUsize::get);
    config_cap.min(frame_cap)
}

fn consensus_block_wire_len(origin: &PeerId, msg: &BlockMessage) -> usize {
    consensus_block_wire_len_owned(origin, msg.clone())
}

fn consensus_block_wire_len_owned(origin: &PeerId, msg: BlockMessage) -> usize {
    let payload = NetworkMessage::SumeragiBlock(Box::new(msg));
    data_frame_wire_len(
        origin,
        Some(origin),
        iroha_config::parameters::defaults::network::RELAY_TTL,
        Priority::High,
        &payload,
    )
}

fn rbc_chunk_payload_cap(origin: &PeerId, payload_frame_cap: usize) -> usize {
    // Use max values for varint fields so the cap holds for any height/view/index.
    let mut chunk = crate::sumeragi::consensus::RbcChunk {
        block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH])),
        height: u64::MAX,
        view: u64::MAX,
        epoch: u64::MAX,
        idx: u32::MAX,
        bytes: Vec::new(),
    };
    let base_len = consensus_block_wire_len_owned(origin, BlockMessage::RbcChunk(chunk.clone()));
    if base_len >= payload_frame_cap {
        return 0;
    }
    let mut low = 0usize;
    let mut high = payload_frame_cap - base_len;
    while low < high {
        let mid = (low + high).div_ceil(2);
        chunk.bytes.resize(mid, 0);
        let encoded_len =
            consensus_block_wire_len_owned(origin, BlockMessage::RbcChunk(chunk.clone()));
        if encoded_len <= payload_frame_cap {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    low
}

fn is_peer_admin_instruction(instr: &iroha_data_model::isi::InstructionBox) -> bool {
    let id = iroha_data_model::isi::Instruction::id(&**instr).to_ascii_lowercase();
    id.contains("registerpeer") || id.contains("unregisterpeer")
}

#[derive(Debug, Clone, Copy, Default)]
struct RbcBacklogSummary {
    sessions_pending: usize,
    missing_chunks_total: usize,
}

impl RbcBacklogSummary {
    fn has_backlog(self) -> bool {
        self.sessions_pending > 0
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
    fn pending_fast_path_timeout(&self, view: &StateView<'_>, mode: ConsensusMode) -> Duration {
        let block_time = self.block_time_for_mode(view, mode);
        let commit_time = self.commit_timeout_for_mode(view, mode);
        let base = if commit_time == Duration::ZERO {
            block_time
        } else {
            block_time.min(commit_time)
        };
        base.max(Duration::from_millis(1))
    }

    fn pending_block_has_votes(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> bool {
        self.vote_log
            .values()
            .any(|vote| vote.block_hash == block_hash && vote.height == height && vote.view == view)
    }

    fn pending_block_has_qc(
        &self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
    ) -> bool {
        let expected_epoch = self.epoch_for_height(height);
        cached_qc_for(
            &self.qc_cache,
            crate::sumeragi::consensus::Phase::Commit,
            block_hash,
            height,
            view,
            expected_epoch,
        )
        .is_some()
            || cached_qc_for(
                &self.qc_cache,
                crate::sumeragi::consensus::Phase::Prepare,
                block_hash,
                height,
                view,
                expected_epoch,
            )
            .is_some()
    }

    fn pending_fast_unblock_due(
        &self,
        pending: &PendingBlock,
        now: Instant,
        fast_timeout: Duration,
    ) -> bool {
        if fast_timeout == Duration::ZERO {
            return false;
        }
        if pending.precommit_vote_sent || pending.commit_qc_seen {
            return false;
        }
        let block_hash = pending.block.hash();
        if self.pending_block_has_votes(block_hash, pending.height, pending.view) {
            return false;
        }
        if self.pending_block_has_qc(block_hash, pending.height, pending.view) {
            return false;
        }
        pending.progress_age(now) >= fast_timeout
    }

    fn active_pending_blocks_len(&self) -> usize {
        let view = self.state.view();
        self.active_pending_blocks_len_for_tip(view.height(), view.latest_block_hash())
    }

    fn active_pending_blocks_len_for_tip(
        &self,
        tip_height: usize,
        tip_hash: Option<HashOf<BlockHeader>>,
    ) -> usize {
        self.pending
            .pending_blocks
            .values()
            // Only pending blocks that extend the current tip should gate proposals/view changes.
            .filter(|pending| {
                !pending.aborted
                    && pending_extends_tip(
                        pending.height,
                        pending.block.header().prev_block_hash(),
                        tip_height,
                        tip_hash,
                    )
            })
            .count()
    }

    fn has_active_pending_blocks(&self) -> bool {
        let view = self.state.view();
        let tip_height = view.height();
        let tip_hash = view.latest_block_hash();
        self.pending.pending_blocks.values().any(|pending| {
            !pending.aborted
                && pending_extends_tip(
                    pending.height,
                    pending.block.header().prev_block_hash(),
                    tip_height,
                    tip_hash,
                )
        })
    }

    fn blocking_pending_blocks_len(&self) -> usize {
        let now = Instant::now();
        let view = self.state.view();
        let tip_height = view.height();
        let tip_hash = view.latest_block_hash();
        let pending_height = u64::try_from(tip_height.saturating_add(1)).unwrap_or(u64::MAX);
        let (consensus_mode, _, _) = self.consensus_context_for_height(pending_height);
        let fast_timeout = self.pending_fast_path_timeout(&view, consensus_mode);
        self.pending
            .pending_blocks
            .values()
            .filter(|pending| {
                !pending.aborted
                    && pending_extends_tip(
                        pending.height,
                        pending.block.header().prev_block_hash(),
                        tip_height,
                        tip_hash,
                    )
                    && (pending.commit_qc_seen
                        || (pending.last_quorum_reschedule.is_none()
                            && !self.pending_fast_unblock_due(pending, now, fast_timeout)))
            })
            .count()
    }

    fn blocking_pending_blocks_len_with_progress(&self, now: Instant) -> usize {
        let da_enabled = self.runtime_da_enabled();
        let quorum_timeout = self.quorum_timeout(da_enabled);
        if quorum_timeout == Duration::ZERO {
            return self.blocking_pending_blocks_len();
        }
        let stall_grace = self
            .config
            .pacemaker
            .pending_stall_grace
            .min(quorum_timeout);
        let view = self.state.view();
        let tip_height = view.height();
        let tip_hash = view.latest_block_hash();
        self.pending
            .pending_blocks
            .values()
            .filter(|pending| {
                if pending.aborted {
                    return false;
                }
                if !pending_extends_tip(
                    pending.height,
                    pending.block.header().prev_block_hash(),
                    tip_height,
                    tip_hash,
                ) {
                    return false;
                }
                if !(pending.commit_qc_seen || pending.last_quorum_reschedule.is_none()) {
                    return false;
                }
                if pending.commit_qc_seen {
                    return true;
                }
                let progress_age = pending.progress_age(now);
                progress_age >= stall_grace && progress_age < quorum_timeout
            })
            .count()
    }

    fn has_blocking_pending_blocks(&self) -> bool {
        self.blocking_pending_blocks_len() > 0
    }

    fn request_commit_pipeline(&mut self) {
        self.pending.commit_pipeline_wakeup = true;
    }

    fn rbc_rebroadcast_active_with_tip_and_session(
        &self,
        key: super::rbc_store::SessionKey,
        tip_height: usize,
        tip_hash: Option<HashOf<BlockHeader>>,
        session: Option<&RbcSession>,
    ) -> bool {
        if let Some(pending) = self.pending.pending_blocks.get(&key.0) {
            if !pending.aborted
                && pending.height == key.1
                && pending.view == key.2
                && pending_extends_tip(
                    pending.height,
                    pending.block.header().prev_block_hash(),
                    tip_height,
                    tip_hash,
                )
            {
                return true;
            }
        }
        if let Some(inflight) = self.subsystems.commit.inflight.as_ref() {
            if inflight.block_hash == key.0
                && !inflight.pending.aborted
                && inflight.pending.height == key.1
                && inflight.pending.view == key.2
                && pending_extends_tip(
                    inflight.pending.height,
                    inflight.pending.block.header().prev_block_hash(),
                    tip_height,
                    tip_hash,
                )
            {
                return true;
            }
        }
        if self
            .pending
            .pending_processing
            .get()
            .is_some_and(|hash| hash == key.0)
        {
            return true;
        }
        let session = if let Some(session) = session {
            Some(session)
        } else {
            self.subsystems.da_rbc.rbc.sessions.get(&key)
        };
        let Some(session) = session else {
            return false;
        };
        if session.is_invalid() {
            return false;
        }
        let Some(block_header) = session.block_header.as_ref() else {
            return false;
        };
        if block_header.hash() != key.0 || block_header.view_change_index() != key.2 {
            return false;
        }
        let block_height = block_header.height().get();
        let tip_height_u64 = u64::try_from(tip_height).unwrap_or(u64::MAX);
        let extends_tip = pending_extends_tip(
            block_height,
            block_header.prev_block_hash(),
            tip_height,
            tip_hash,
        );
        let matches_tip =
            tip_hash.is_some_and(|hash| hash == key.0) && block_height == tip_height_u64;
        extends_tip || matches_tip
    }

    fn rbc_rebroadcast_active_with_tip(
        &self,
        key: super::rbc_store::SessionKey,
        tip_height: usize,
        tip_hash: Option<HashOf<BlockHeader>>,
    ) -> bool {
        self.rbc_rebroadcast_active_with_tip_and_session(key, tip_height, tip_hash, None)
    }

    fn rbc_rebroadcast_active(&self, key: super::rbc_store::SessionKey) -> bool {
        let view = self.state.view();
        let tip_height = view.height();
        let tip_hash = view.latest_block_hash();
        drop(view);
        self.rbc_rebroadcast_active_with_tip(key, tip_height, tip_hash)
    }

    fn touch_pending_progress(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        now: Instant,
    ) {
        if let Some(pending) = self.pending.pending_blocks.get_mut(&block_hash) {
            if !pending.aborted && pending.height == height && pending.view == view {
                pending.touch_progress(now);
            }
        }
        if let Some(inflight) = self.subsystems.commit.inflight.as_mut() {
            if inflight.block_hash == block_hash
                && !inflight.pending.aborted
                && inflight.pending.height == height
                && inflight.pending.view == view
            {
                inflight.pending.touch_progress(now);
            }
        }
    }

    fn rbc_backlog_summary(&self) -> RbcBacklogSummary {
        if !self.runtime_da_enabled() {
            return RbcBacklogSummary::default();
        }
        let view = self.state.view();
        let tip_height = view.height();
        let tip_height_u64 = u64::try_from(tip_height).unwrap_or(u64::MAX);
        let tip_hash = view.latest_block_hash();
        drop(view);
        let mut summary = RbcBacklogSummary::default();
        for (key, session) in &self.subsystems.da_rbc.rbc.sessions {
            if !self.rbc_rebroadcast_active_with_tip_and_session(
                *key,
                tip_height,
                tip_hash,
                Some(session),
            ) {
                continue;
            }
            if session.is_invalid() {
                continue;
            }
            let total_chunks = session.total_chunks();
            let received_chunks = session.received_chunks();
            let missing_chunks = total_chunks != 0 && received_chunks < total_chunks;
            let roster = self.rbc_session_roster(*key);
            let roster_source = self
                .rbc_session_roster_source(*key)
                .unwrap_or(RbcRosterSource::Init);
            let ready_quorum = if !roster.is_empty() && roster_source.is_authoritative() {
                let topology = super::network_topology::Topology::new(roster);
                session.ready_signatures.len() >= self.rbc_deliver_quorum(&topology)
            } else {
                false
            };
            let payload_available =
                self.block_payload_available_locally(key.0) || session.delivered;
            if missing_chunks || !ready_quorum || !payload_available {
                summary.sessions_pending = summary.sessions_pending.saturating_add(1);
                if missing_chunks {
                    let missing = total_chunks.saturating_sub(received_chunks);
                    summary.missing_chunks_total = summary
                        .missing_chunks_total
                        .saturating_add(usize::try_from(missing).unwrap_or(usize::MAX));
                }
            }
        }
        let pending_payload_sessions = self
            .subsystems
            .da_rbc
            .rbc
            .pending
            .keys()
            .filter(|key| {
                self.rbc_rebroadcast_active_with_tip(**key, tip_height, tip_hash)
                    || key.1 == tip_height_u64
                    || key.1 == tip_height_u64.saturating_add(1)
            })
            .count();
        summary.sessions_pending = summary
            .sessions_pending
            .saturating_add(pending_payload_sessions);
        summary
    }

    fn has_unresolved_rbc_backlog(&self) -> bool {
        self.rbc_backlog_summary().has_backlog()
    }

    fn rbc_backlog_exceeds_pacemaker_soft_limits(&self, summary: RbcBacklogSummary) -> bool {
        if !summary.has_backlog() {
            return false;
        }
        summary.sessions_pending > self.config.pacemaker.rbc_backlog_session_soft_limit
            || summary.missing_chunks_total > self.config.pacemaker.rbc_backlog_chunk_soft_limit
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

#[cfg(test)]
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
    highest_qc: Option<crate::sumeragi::consensus::QcRef>,
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
    epoch: u64,
) -> Option<crate::sumeragi::consensus::Qc> {
    qc_cache_for_subject(qc_cache, hash)
        .find(|qc| qc.phase == phase && qc.height == height && qc.view == view && qc.epoch == epoch)
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
    consensus_payload_frame_cap: usize,
    events_sender: EventsSender,
    state: Arc<State>,
    queue: Arc<Queue>,
    kura: Arc<Kura>,
    network: IrohaNetwork,
    subsystems: ActorSubsystems,
    block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>>,
    #[allow(dead_code)] // Retained until background broadcast wiring consumes the gossiper handle.
    peers_gossiper: PeersGossiperHandle,
    #[allow(dead_code)] // Retained until Genesis gossip orchestration consumes the handle.
    genesis_network: GenesisWithPubKey,
    block_count: BlockCount,
    block_sync_gossip_limit: usize,
    last_advertised_topology: BTreeSet<PeerId>,
    last_commit_topology_hash: Option<HashOf<Vec<PeerId>>>,
    last_commit_topology_membership_hash: Option<HashOf<Vec<PeerId>>>,
    last_committed_height: u64,
    #[cfg(feature = "telemetry")]
    telemetry: Telemetry,
    epoch_roster_provider: Option<Arc<WsvEpochRosterAdapter>>,
    background_post_tx: Option<mpsc::SyncSender<BackgroundPost>>,
    wake_tx: Option<mpsc::SyncSender<()>>,
    phase_tracker: PhaseTracker,
    /// Tracks when queued work first appeared for the current height/view.
    queue_ready_since: Option<QueueReadySince>,
    phase_ema: PhaseEma,
    evidence_store: EvidenceStore,
    invalid_sig_log: InvalidSigThrottle,
    rbc_mismatch_log: RbcMismatchThrottle,
    invalid_sig_penalty: InvalidSigPenalty,
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
    deferred_votes: BTreeMap<
        HashOf<BlockHeader>,
        BTreeMap<votes::VoteLogKey, crate::sumeragi::consensus::Vote>,
    >,
    deferred_qcs: BTreeMap<QcVoteKey, crate::sumeragi::consensus::Qc>,
    vote_roster_cache: BTreeMap<HashOf<BlockHeader>, VoteRosterCacheEntry>,
    qc_cache: BTreeMap<QcVoteKey, crate::sumeragi::consensus::Qc>,
    qc_signer_tally: BTreeMap<QcVoteKey, QcSignerTally>,
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
    relay_backpressure: RelayBackpressure,
    queue_drop_backpressure: QueueDropBackpressure,
    queue_block_backpressure: QueueBlockBackpressure,
    new_view_rebroadcast_log: NewViewRebroadcastThrottle,
    proposal_rebroadcast_log: PayloadRebroadcastThrottle,
    payload_rebroadcast_log: PayloadRebroadcastThrottle,
    block_sync_rebroadcast_log: PayloadRebroadcastThrottle,
    block_sync_fetch_log: PayloadRebroadcastThrottle,
    roster_validation_cache: RosterValidationCache,
}

#[derive(Debug, Clone)]
struct VoteRosterCacheEntry {
    roster: Vec<PeerId>,
    height: u64,
    view: u64,
}

struct ActorSubsystems {
    validation: ValidationState,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitTopologyChange {
    None,
    OrderOnly,
    Membership,
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
        return Err(std::io::Error::other("DA spool path is not a directory"));
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
    #[allow(clippy::option_option)]
    commitment_bundle: Option<Option<DaCommitmentBundle>>,
    #[allow(clippy::option_option)]
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
        let stamp = if let Some(stamp) = scan_da_spool_stamp(spool_dir)? {
            stamp
        } else {
            if self.dir_stamp.is_some() {
                self.invalidate();
            }
            return Ok(false);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RbcRosterSource {
    /// Locally derived commit-topology snapshot (authoritative).
    Derived,
    /// Unverified roster snapshot (e.g., from RBC INIT or persisted sessions).
    Init,
}

impl RbcRosterSource {
    fn is_authoritative(self) -> bool {
        matches!(self, Self::Derived)
    }

    fn merge(self, other: Self) -> Self {
        if other.is_authoritative() && !self.is_authoritative() {
            other
        } else {
            self
        }
    }
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
    sealed_pin_intents: BTreeSet<(u32, u64, u64)>,
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
    session_roster_sources: BTreeMap<super::rbc_store::SessionKey, RbcRosterSource>,
    status_handle: rbc_status::Handle,
    payload_rebroadcast_last_sent: BTreeMap<super::rbc_store::SessionKey, Instant>,
    ready_rebroadcast_last_sent: BTreeMap<super::rbc_store::SessionKey, Instant>,
    ready_deferral: BTreeMap<super::rbc_store::SessionKey, RbcReadyDeferral>,
    deliver_deferral: BTreeMap<super::rbc_store::SessionKey, RbcDeliverDeferral>,
    outbound_chunks: BTreeMap<super::rbc_store::SessionKey, RbcOutboundChunks>,
    outbound_cursor: Option<super::rbc_store::SessionKey>,
    rebroadcast_cursor: Option<super::rbc_store::SessionKey>,
    persisted_full_sessions: BTreeSet<super::rbc_store::SessionKey>,
    persist_tx: Option<mpsc::SyncSender<rbc::RbcPersistWork>>,
    persist_rx: Option<mpsc::Receiver<rbc::RbcPersistResult>>,
    persist_inflight: BTreeSet<super::rbc_store::SessionKey>,
}

#[derive(Debug)]
struct RbcOutboundChunks {
    chunks: Vec<crate::sumeragi::consensus::RbcChunk>,
    cursor: usize,
    targets: Vec<(usize, PeerId)>,
}

#[derive(Debug, Clone, Copy)]
struct RbcDeliverDeferral {
    last_attempt: Instant,
    ready_count: usize,
    received_chunks: u32,
    total_chunks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RbcReadyDeferralReason {
    CommitRosterMissing,
    CommitRosterUnverified,
    MissingChunksOrReadyQuorum,
    ChunkRootMissing,
    LocalNotInCommitTopology,
}

#[derive(Debug, Clone, Copy)]
struct RbcReadyDeferral {
    last_attempt: Instant,
    reason: RbcReadyDeferralReason,
    ready_count: usize,
    required_ready: usize,
    received_chunks: u32,
    total_chunks: u32,
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
    missing_block_requests: BTreeMap<HashOf<BlockHeader>, MissingBlockRequest>,
    pending_processing: Cell<Option<HashOf<BlockHeader>>>,
    pending_processing_parent: Cell<Option<HashOf<BlockHeader>>>,
    last_commit_pipeline_run: Instant,
    commit_pipeline_wakeup: bool,
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

struct ValidationState {
    work_txs: Vec<mpsc::SyncSender<validation::ValidationWork>>,
    result_rx: Option<mpsc::Receiver<validation::ValidationResult>>,
    inflight: BTreeMap<HashOf<BlockHeader>, u64>,
    next_id: u64,
    next_worker: usize,
}

impl ValidationState {
    fn new() -> Self {
        Self {
            work_txs: Vec::new(),
            result_rx: None,
            inflight: BTreeMap::new(),
            next_id: 0,
            next_worker: 0,
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
    pacemaker_backpressure_tracker: pacing::PacemakerBackpressureTracker,
    last_pacemaker_attempt: Option<Instant>,
    last_successful_proposal: Option<Instant>,
    propose_attempt_monitor: ProposeAttemptMonitor,
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
            self.wake_tx.clone(),
            self.config.persistence.commit_work_queue_cap,
            self.config.persistence.commit_result_queue_cap,
        );
        self.subsystems.commit.work_tx = Some(commit_handle.work_tx);
        self.subsystems.commit.result_rx = Some(commit_handle.result_rx);
        commit_handle.join_handle
    }

    pub(super) fn attach_validation_worker(&mut self) -> Vec<std::thread::JoinHandle<()>> {
        let validation_handle = validation::spawn_validation_workers(
            Arc::clone(&self.state),
            self.common_config.chain.clone(),
            self.genesis_account.clone(),
            self.wake_tx.clone(),
            self.config.worker.validation_worker_threads,
            self.config.worker.validation_work_queue_cap,
            self.config.worker.validation_result_queue_cap,
        );
        self.subsystems.validation.work_txs = validation_handle.work_txs;
        self.subsystems.validation.result_rx = Some(validation_handle.result_rx);
        validation_handle.join_handles
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
    pacemaker_backpressure_tracker: &mut pacing::PacemakerBackpressureTracker,
    forced_view_after_timeout: &mut Option<(u64, u64)>,
    last_pacemaker_attempt: &mut Option<Instant>,
    last_successful_proposal: &mut Option<Instant>,
    tick_counter: &mut u64,
    qc_signer_tally: &mut BTreeMap<QcVoteKey, QcSignerTally>,
    voting_block: &mut Option<VotingBlock>,
    pending_roster_activation: &mut Option<(u64, Vec<PeerId>)>,
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
    *pacemaker_backpressure_tracker = pacing::PacemakerBackpressureTracker::new();
    *forced_view_after_timeout = None;
    *last_pacemaker_attempt = None;
    *last_successful_proposal = None;
    *tick_counter = 0;
    qc_signer_tally.clear();
    *voting_block = None;
    *pending_roster_activation = None;
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
    QcHint,
    CommitCheckpointPairHint,
    ValidatorCheckpointHint,
    QcHistory,
    ValidatorCheckpointHistory,
    RosterSidecar,
    CommitRosterJournal,
    CommitTopologySnapshot,
    TrustedPeersFallback,
}

impl BlockSyncRosterSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::QcHint => "commit_qc_hint",
            Self::CommitCheckpointPairHint => "commit_checkpoint_pair_hint",
            Self::ValidatorCheckpointHint => "validator_checkpoint_hint",
            Self::QcHistory => "commit_qc_history",
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
    commit_qc: Option<Qc>,
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
    EpochMismatch {
        expected: u64,
        actual: u64,
    },
    PhaseMismatch,
    ModeTagMismatch,
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

fn apply_roster_selection_to_block_sync_update(
    update: &mut super::message::BlockSyncUpdate,
    selection: &BlockSyncRosterSelection,
) {
    update.commit_qc.clone_from(&selection.commit_qc);
    update
        .validator_checkpoint
        .clone_from(&selection.checkpoint);
    update.stake_snapshot.clone_from(&selection.stake_snapshot);
}

// Block sync updates are only verifiable when they carry roster evidence.
fn block_sync_update_has_roster(
    update: &super::message::BlockSyncUpdate,
    consensus_mode: ConsensusMode,
) -> bool {
    let has_roster_hint = update.commit_qc.is_some() || update.validator_checkpoint.is_some();
    match consensus_mode {
        ConsensusMode::Permissioned => has_roster_hint,
        ConsensusMode::Npos => has_roster_hint && update.stake_snapshot.is_some(),
    }
}

fn block_sync_update_wire_len(origin: &PeerId, update: &super::message::BlockSyncUpdate) -> usize {
    consensus_block_wire_len_owned(origin, BlockMessage::BlockSyncUpdate(update.clone()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewChangeCause {
    CommitFailure,
    QuorumTimeout,
    StakeQuorumTimeout,
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
            Self::StakeQuorumTimeout => "stake_quorum_timeout",
            Self::CensorshipEvidence => "censorship_evidence",
            Self::MissingPayload => "missing_payload",
            Self::MissingQc => "missing_qc",
            Self::ValidationReject => "validation_reject",
        }
    }
}

fn view_change_cause_for_quorum(vote_count: usize, stake_quorum_missing: bool) -> ViewChangeCause {
    if vote_count == 0 {
        ViewChangeCause::MissingQc
    } else if stake_quorum_missing {
        ViewChangeCause::StakeQuorumTimeout
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
            topology.nth_rotation(view);
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
    let view = block.header().view_change_index();
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

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn selection_from_roster_artifacts(
    commit_qc: Option<&Qc>,
    checkpoint: Option<&ValidatorSetCheckpoint>,
    stake_snapshot: Option<&CommitStakeSnapshot>,
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    block_view: Option<u64>,
    source: BlockSyncRosterSource,
    consensus_mode: ConsensusMode,
    chain_id: &ChainId,
    mode_tag: &'static str,
    roster_cache: &RosterValidationCache,
) -> Option<BlockSyncRosterSelection> {
    let expected_epoch = roster_cache.expected_epoch(block_height, consensus_mode);
    let allow_genesis_stub = matches!(source, BlockSyncRosterSource::CommitRosterJournal)
        && block_height == 1
        && block_view.is_none_or(|view| view == 0);
    let mut cert_inputs: Option<RosterValidationInputs> = None;
    let mut checkpoint_inputs: Option<RosterValidationInputs> = None;
    let validated_cert = commit_qc.and_then(|cert| {
        let inputs = cert_inputs.get_or_insert_with(|| {
            roster_cache.inputs_for_roster(&cert.validator_set, consensus_mode, stake_snapshot)
        });
        match validate_commit_qc_roster(
            cert,
            block_hash,
            block_height,
            block_view,
            consensus_mode,
            expected_epoch,
            chain_id,
            mode_tag,
            allow_genesis_stub,
            inputs,
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
    let epoch = match (consensus_mode, validated_cert.as_ref()) {
        (ConsensusMode::Npos, Some((_roster, cert))) => cert.epoch,
        (ConsensusMode::Npos, None) => expected_epoch,
        _ => 0,
    };
    let checkpoint_roots = validated_cert
        .as_ref()
        .map(|(_, cert)| (cert.parent_state_root, cert.post_state_root))
        .or_else(|| {
            super::status::precommit_signers_for(block_hash).and_then(|record| {
                if record.height == block_height
                    && record.epoch == epoch
                    && block_view.is_none_or(|view| view == record.view)
                {
                    Some((record.parent_state_root, record.post_state_root))
                } else {
                    None
                }
            })
        });
    let validated_checkpoint = checkpoint.and_then(|chk| {
        let reuse_cert_inputs =
            commit_qc.is_some_and(|cert| cert.validator_set == chk.validator_set);
        let inputs = if reuse_cert_inputs {
            cert_inputs.get_or_insert_with(|| {
                roster_cache.inputs_for_roster(&chk.validator_set, consensus_mode, stake_snapshot)
            })
        } else {
            checkpoint_inputs.get_or_insert_with(|| {
                roster_cache.inputs_for_roster(&chk.validator_set, consensus_mode, stake_snapshot)
            })
        };
        match validate_checkpoint_roster(
            chk,
            block_hash,
            block_height,
            block_view,
            consensus_mode,
            chain_id,
            mode_tag,
            epoch,
            checkpoint_roots,
            allow_genesis_stub,
            inputs,
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
            if chk.view != cert.view {
                warn!(
                    height = block_height,
                    block = %block_hash,
                    cert_view = cert.view,
                    checkpoint_view = chk.view,
                    "commit certificate and checkpoint views differ; preferring commit certificate"
                );
                let stake_snapshot = stake_snapshot
                    .filter(|snapshot| snapshot.matches_roster(&roster))
                    .cloned();
                return Some(BlockSyncRosterSelection {
                    roster,
                    source,
                    commit_qc: Some(cert.clone()),
                    checkpoint: None,
                    stake_snapshot,
                });
            }
            if chk.parent_state_root != cert.parent_state_root
                || chk.post_state_root != cert.post_state_root
            {
                warn!(
                    height = block_height,
                    block = %block_hash,
                    "commit certificate and checkpoint roots differ; preferring commit certificate"
                );
                let stake_snapshot = stake_snapshot
                    .filter(|snapshot| snapshot.matches_roster(&roster))
                    .cloned();
                return Some(BlockSyncRosterSelection {
                    roster,
                    source,
                    commit_qc: Some(cert.clone()),
                    checkpoint: None,
                    stake_snapshot,
                });
            }
            let stake_snapshot = stake_snapshot
                .filter(|snapshot| snapshot.matches_roster(&roster))
                .cloned();
            Some(BlockSyncRosterSelection {
                roster,
                source,
                commit_qc: Some(cert.clone()),
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
                commit_qc: Some(cert.clone()),
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
                commit_qc: None,
                checkpoint: Some(chk.clone()),
                stake_snapshot,
            })
        }
        (None, None) => None,
    }
}

fn block_sync_history_roster_for_block(
    consensus_mode: ConsensusMode,
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    block_view: Option<u64>,
    chain_id: &ChainId,
    roster_cache: &RosterValidationCache,
) -> Option<BlockSyncRosterSelection> {
    let mode_tag = match consensus_mode {
        ConsensusMode::Permissioned => PERMISSIONED_TAG,
        ConsensusMode::Npos => NPOS_TAG,
    };
    let mut cert = super::status::commit_qc_history()
        .into_iter()
        .filter(|cert| cert.subject_block_hash == block_hash && cert.height <= block_height)
        .max_by(|a, b| a.height.cmp(&b.height).then_with(|| a.view.cmp(&b.view)));
    let checkpoint = super::status::validator_checkpoint_history()
        .into_iter()
        .filter(|chk| chk.block_hash == block_hash && chk.height <= block_height)
        .max_by(|a, b| a.height.cmp(&b.height));

    if cert.is_none() {
        if let Some(record) = super::status::precommit_signers_for(block_hash) {
            let expected_epoch = roster_cache.expected_epoch(record.height, consensus_mode);
            let view_matches = block_view.is_none_or(|view| view == record.view);
            if record.height <= block_height
                && view_matches
                && record.epoch == expected_epoch
                && record.mode_tag.as_str() == mode_tag
            {
                cert = derive_block_sync_qc_from_signers(
                    block_hash,
                    record.height,
                    record.view,
                    record.epoch,
                    record.parent_state_root,
                    record.post_state_root,
                    &record.validator_set,
                    consensus_mode,
                    record.stake_snapshot.as_ref(),
                    record.mode_tag.as_str(),
                    &record.signers,
                    record.bls_aggregate_signature.clone(),
                );
            }
        }
    }

    if cert.is_none() && checkpoint.is_none() {
        return None;
    }

    let source = if cert.is_some() {
        BlockSyncRosterSource::QcHistory
    } else {
        BlockSyncRosterSource::ValidatorCheckpointHistory
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
        chain_id,
        mode_tag,
        roster_cache,
    )
}

fn persisted_roster_for_block(
    state: &State,
    kura: &Kura,
    consensus_mode: ConsensusMode,
    block_height: u64,
    block_hash: HashOf<BlockHeader>,
    block_view: Option<u64>,
    roster_cache: &RosterValidationCache,
) -> Option<BlockSyncRosterSelection> {
    let mode_tag = match consensus_mode {
        ConsensusMode::Permissioned => PERMISSIONED_TAG,
        ConsensusMode::Npos => NPOS_TAG,
    };
    if let Some(snapshot) = state.commit_roster_snapshot_for_block(block_height, block_hash) {
        if let Some(selection) = selection_from_roster_artifacts(
            Some(&snapshot.commit_qc),
            Some(&snapshot.validator_checkpoint),
            snapshot.stake_snapshot.as_ref(),
            block_hash,
            block_height,
            block_view,
            BlockSyncRosterSource::CommitRosterJournal,
            consensus_mode,
            &state.chain_id,
            mode_tag,
            roster_cache,
        ) {
            if let Some(cert) = selection.commit_qc.as_ref() {
                status::record_commit_qc(cert.clone());
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
            meta.commit_qc.as_ref(),
            meta.validator_checkpoint.as_ref(),
            meta.stake_snapshot.as_ref(),
            block_hash,
            block_height,
            block_view,
            BlockSyncRosterSource::RosterSidecar,
            consensus_mode,
            &state.chain_id,
            mode_tag,
            roster_cache,
        ) {
            if let Some(cert) = selection.commit_qc.as_ref() {
                status::record_commit_qc(cert.clone());
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
    roster_cache: &RosterValidationCache,
) -> super::message::BlockSyncUpdate {
    let block_hash = block.hash();
    let block_height = block.header().height().get();
    let block_view = block.header().view_change_index();
    let mut update = super::message::BlockSyncUpdate::from(block);
    let consensus_mode = {
        let world = state.world.view();
        let consensus_mode = super::effective_consensus_mode_for_height_from_world(
            &world,
            block_height,
            fallback_consensus_mode,
        );
        drop(world);
        consensus_mode
    };
    let selection = persisted_roster_for_block(
        state,
        kura,
        consensus_mode,
        block_height,
        block_hash,
        Some(block_view),
        roster_cache,
    )
    .or_else(|| {
        block_sync_history_roster_for_block(
            consensus_mode,
            block_hash,
            block_height,
            Some(block_view),
            &state.chain_id,
            roster_cache,
        )
    })
    .or_else(|| {
        // Use the active roster so checkpoint signature indices match consensus ordering.
        let world = state.world.view();
        let commit_topology = state.commit_topology.view();
        let roster =
            derive_active_topology_from_views(&world, commit_topology.as_slice(), trusted, me);
        let source = if commit_topology.is_empty() {
            BlockSyncRosterSource::TrustedPeersFallback
        } else {
            BlockSyncRosterSource::CommitTopologySnapshot
        };
        drop(commit_topology);
        drop(world);
        if roster.is_empty() {
            None
        } else {
            Some(BlockSyncRosterSelection {
                roster: roster.clone(),
                source,
                commit_qc: None,
                checkpoint: None,
                stake_snapshot: None,
            })
        }
    });
    if let Some(selection) = selection.as_ref() {
        apply_roster_selection_to_block_sync_update(&mut update, selection);
    }
    if matches!(consensus_mode, ConsensusMode::Npos) && update.stake_snapshot.is_none() {
        if let Some(roster) = selection.as_ref().map(|sel| sel.roster.as_slice()) {
            update.stake_snapshot = roster_cache.stake_snapshot_for_roster(roster);
        }
    }
    update
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn validate_commit_qc_roster(
    cert: &Qc,
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    block_view: Option<u64>,
    consensus_mode: ConsensusMode,
    expected_epoch: u64,
    chain_id: &ChainId,
    mode_tag: &str,
    allow_genesis_stub: bool,
    inputs: &RosterValidationInputs,
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
    if cert.epoch != expected_epoch {
        return Err(RosterValidationError::EpochMismatch {
            expected: expected_epoch,
            actual: cert.epoch,
        });
    }
    if cert.phase != crate::sumeragi::consensus::Phase::Commit {
        return Err(RosterValidationError::PhaseMismatch);
    }
    if cert.mode_tag != mode_tag {
        return Err(RosterValidationError::ModeTagMismatch);
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
    let genesis_stub = allow_genesis_stub
        && block_height == 1
        && cert.view == 0
        && cert.aggregate.bls_aggregate_signature.is_empty()
        && cert.aggregate.signers_bitmap.iter().all(|byte| *byte == 0);
    if cert.aggregate.bls_aggregate_signature.is_empty() && !genesis_stub {
        return Err(RosterValidationError::AggregateSignatureMissing);
    }
    if genesis_stub {
        return Ok(cert.validator_set.clone());
    }
    let mut signer_indices = BTreeSet::new();
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
            if !signer_indices.insert(idx) {
                return Err(RosterValidationError::DuplicateSigner(idx_u32));
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
            if signer_indices.len() < required {
                return Err(RosterValidationError::CommitQuorumMissing {
                    votes: signer_indices.len(),
                    required,
                });
            }
        }
        ConsensusMode::Npos => {
            let snapshot = inputs
                .stake_snapshot
                .as_ref()
                .filter(|snapshot| snapshot.matches_roster(&cert.validator_set))
                .ok_or(RosterValidationError::StakeSnapshotUnavailable)?;
            match stake_quorum_reached_for_snapshot(snapshot, &cert.validator_set, &signer_peers) {
                Ok(true) => {}
                Ok(false) => return Err(RosterValidationError::StakeQuorumMissing),
                Err(_) => return Err(RosterValidationError::StakeSnapshotUnavailable),
            }
        }
    }
    let preimage = qc_bls_preimage(cert, chain_id, mode_tag);
    let mut public_keys: Vec<&PublicKey> = Vec::with_capacity(signer_indices.len());
    let mut pop_refs: Vec<&[u8]> = Vec::with_capacity(signer_indices.len());
    for idx in &signer_indices {
        let Some(peer) = cert.validator_set.get(*idx) else {
            return Err(RosterValidationError::SignerOutOfRange {
                signer: u32::try_from(*idx).unwrap_or(u32::MAX),
                roster_len: roster_len.try_into().unwrap_or(u32::MAX),
            });
        };
        let pk = peer.public_key();
        let Some(pop) = inputs.pops.get(pk) else {
            return Err(RosterValidationError::AggregateSignatureInvalid);
        };
        public_keys.push(pk);
        pop_refs.push(pop.as_slice());
    }
    if iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &cert.aggregate.bls_aggregate_signature,
        &public_keys,
        &pop_refs,
    )
    .is_err()
    {
        return Err(RosterValidationError::AggregateSignatureInvalid);
    }
    Ok(cert.validator_set.clone())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn validate_checkpoint_roster(
    checkpoint: &ValidatorSetCheckpoint,
    block_hash: HashOf<BlockHeader>,
    block_height: u64,
    block_view: Option<u64>,
    consensus_mode: ConsensusMode,
    chain_id: &ChainId,
    mode_tag: &str,
    epoch: u64,
    roots: Option<(Hash, Hash)>,
    allow_genesis_stub: bool,
    inputs: &RosterValidationInputs,
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
    if let Some(block_view) = block_view {
        if checkpoint.view != block_view {
            return Err(RosterValidationError::ViewMismatch {
                expected: block_view,
                actual: checkpoint.view,
            });
        }
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
    let genesis_stub = allow_genesis_stub
        && block_height == 1
        && checkpoint.view == 0
        && checkpoint.bls_aggregate_signature.is_empty()
        && checkpoint.signers_bitmap.iter().all(|byte| *byte == 0);
    if checkpoint.bls_aggregate_signature.is_empty() && !genesis_stub {
        return Err(RosterValidationError::AggregateSignatureMissing);
    }
    if genesis_stub {
        return Ok(checkpoint.validator_set.clone());
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
            let snapshot = inputs
                .stake_snapshot
                .as_ref()
                .filter(|snapshot| snapshot.matches_roster(&checkpoint.validator_set))
                .ok_or(RosterValidationError::StakeSnapshotUnavailable)?;
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
    if let Some((parent_state_root, post_state_root)) = roots {
        if checkpoint.parent_state_root != parent_state_root
            || checkpoint.post_state_root != post_state_root
        {
            return Err(RosterValidationError::AggregateSignatureInvalid);
        }
    }
    let parent_state_root = checkpoint.parent_state_root;
    let post_state_root = checkpoint.post_state_root;
    let view = block_view.unwrap_or(checkpoint.view);
    let vote = crate::sumeragi::consensus::Vote {
        phase: crate::sumeragi::consensus::Phase::Commit,
        block_hash,
        parent_state_root,
        post_state_root,
        height: block_height,
        view,
        epoch,
        highest_qc: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    let preimage = vote_preimage(chain_id, mode_tag, &vote);
    let mut public_keys: Vec<&PublicKey> = Vec::with_capacity(signer_indices.len());
    let mut pop_refs: Vec<&[u8]> = Vec::with_capacity(signer_indices.len());
    for idx in &signer_indices {
        let Some(peer) = checkpoint.validator_set.get(*idx) else {
            return Err(RosterValidationError::SignerOutOfRange {
                signer: u32::try_from(*idx).unwrap_or(u32::MAX),
                roster_len: roster_len.try_into().unwrap_or(u32::MAX),
            });
        };
        let pk = peer.public_key();
        let Some(pop) = inputs.pops.get(pk) else {
            return Err(RosterValidationError::AggregateSignatureInvalid);
        };
        public_keys.push(pk);
        pop_refs.push(pop.as_slice());
    }
    if iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &checkpoint.bls_aggregate_signature,
        &public_keys,
        &pop_refs,
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
    cert_hint: Option<&Qc>,
    checkpoint_hint: Option<&ValidatorSetCheckpoint>,
    stake_snapshot_hint: Option<&CommitStakeSnapshot>,
    state: &State,
    trusted: &iroha_config::parameters::actual::TrustedPeers,
    me: &PeerId,
    consensus_mode: ConsensusMode,
    mode_tag: &'static str,
    allow_uncertified: bool,
    roster_cache: &RosterValidationCache,
) -> Option<BlockSyncRosterSelection> {
    let block_view = block.header().view_change_index();
    if let Some(selection) = persisted {
        return Some(selection);
    }

    if let Some(snapshot) = state.commit_roster_snapshot_for_block(block_height, block_hash) {
        if let Some(selection) = selection_from_roster_artifacts(
            Some(&snapshot.commit_qc),
            Some(&snapshot.validator_checkpoint),
            snapshot.stake_snapshot.as_ref(),
            block_hash,
            block_height,
            Some(block_view),
            BlockSyncRosterSource::CommitRosterJournal,
            consensus_mode,
            &state.chain_id,
            mode_tag,
            roster_cache,
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
            &state.chain_id,
            mode_tag,
            roster_cache,
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
            BlockSyncRosterSource::QcHint,
            consensus_mode,
            &state.chain_id,
            mode_tag,
            roster_cache,
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
            &state.chain_id,
            mode_tag,
            roster_cache,
        ) {
            return Some(selection);
        }
    }

    if let Some(history) = block_sync_history_roster_for_block(
        consensus_mode,
        block_hash,
        block_height,
        Some(block_view),
        &state.chain_id,
        roster_cache,
    ) {
        return Some(history);
    }

    if allow_uncertified {
        let view = state.view();
        let roster = derive_active_topology_for_mode(&view, trusted, me, consensus_mode);
        let source = if view.commit_topology().is_empty() {
            BlockSyncRosterSource::TrustedPeersFallback
        } else {
            BlockSyncRosterSource::CommitTopologySnapshot
        };
        let stake_snapshot = match consensus_mode {
            ConsensusMode::Npos => roster_cache.stake_snapshot_for_roster(&roster),
            ConsensusMode::Permissioned => None,
        };
        drop(view);

        if !roster.is_empty() {
            return Some(BlockSyncRosterSelection {
                roster,
                source,
                commit_qc: None,
                checkpoint: None,
                stake_snapshot,
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
    fn synthesize_commit_qc(
        state: &State,
        block: &SignedBlock,
        roster: &[PeerId],
        mode_tag: &str,
        epoch: u64,
        consensus_mode: ConsensusMode,
    ) -> Option<(Qc, Option<CommitStakeSnapshot>)> {
        if roster.is_empty() {
            return None;
        }
        if roster.iter().any(|peer| !roster_member_allowed_bls(peer)) {
            return None;
        }
        let height = block.header().height().get();
        let view = block.header().view_change_index();
        let cert = status::commit_qc_history()
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
                    record.parent_state_root,
                    record.post_state_root,
                    roster,
                    consensus_mode,
                    record.stake_snapshot.as_ref(),
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
        if let Some((cert, stake_snapshot)) = Self::synthesize_commit_qc(
            self.state.as_ref(),
            block,
            roster,
            mode_tag,
            self.epoch_for_height(height),
            consensus_mode,
        ) {
            let checkpoint = ValidatorSetCheckpoint::new(
                cert.height,
                cert.view,
                cert.subject_block_hash,
                cert.parent_state_root,
                cert.post_state_root,
                cert.validator_set.clone(),
                cert.aggregate.signers_bitmap.clone(),
                cert.aggregate.bls_aggregate_signature.clone(),
                cert.validator_set_hash_version,
                None,
            );
            self.state
                .record_commit_roster(&cert, &checkpoint, stake_snapshot);
        }
    }

    fn effective_commit_topology_from_view(&self, view: &StateView<'_>) -> Vec<PeerId> {
        derive_active_topology_for_mode(
            view,
            self.common_config.trusted_peers.value(),
            self.common_config.peer.id(),
            self.consensus_mode,
        )
    }

    fn effective_commit_topology(&self) -> Vec<PeerId> {
        let view = self.state.view();
        self.effective_commit_topology_from_view(&view)
    }

    fn rbc_roster_for_session(&self, key: super::rbc_store::SessionKey) -> Vec<PeerId> {
        let (consensus_mode, _mode_tag, _prf_seed) = self.consensus_context_for_height(key.1);
        let mut roster = self.roster_for_vote_with_mode(key.0, key.1, key.2, consensus_mode);
        if roster.is_empty() {
            let committed_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
            let committed_epoch = self.epoch_for_height(committed_height);
            let session_epoch = self.epoch_for_height(key.1);
            let payload_known = self.block_known_locally(key.0);
            let allow_fallback = key.1 <= committed_height.saturating_add(1)
                || (payload_known && session_epoch == committed_epoch);
            if allow_fallback {
                let view = self.state.view();
                let fallback = derive_active_topology_for_mode(
                    &view,
                    self.common_config.trusted_peers.value(),
                    self.common_config.peer.id(),
                    consensus_mode,
                );
                drop(view);
                if !fallback.is_empty() {
                    if payload_known && key.1 > committed_height.saturating_add(1) {
                        debug!(
                            height = key.1,
                            committed_height,
                            committed_epoch,
                            "using active topology for RBC roster beyond committed height within epoch"
                        );
                    }
                    roster = fallback;
                }
            }
        }
        roster
    }

    fn rbc_session_roster(&self, key: super::rbc_store::SessionKey) -> Vec<PeerId> {
        self.subsystems
            .da_rbc
            .rbc
            .session_rosters
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }

    fn rbc_session_roster_source(
        &self,
        key: super::rbc_store::SessionKey,
    ) -> Option<RbcRosterSource> {
        self.subsystems
            .da_rbc
            .rbc
            .session_roster_sources
            .get(&key)
            .copied()
    }

    fn allow_unverified_rbc_roster(&self, key: super::rbc_store::SessionKey) -> bool {
        let (consensus_mode, _mode_tag, _prf_seed) = self.consensus_context_for_height(key.1);
        if !matches!(consensus_mode, ConsensusMode::Permissioned) {
            return false;
        }
        self.rbc_roster_for_session(key).is_empty()
    }

    fn ensure_rbc_session_roster(&mut self, key: super::rbc_store::SessionKey) -> Vec<PeerId> {
        if let Some(roster) = self
            .subsystems
            .da_rbc
            .rbc
            .session_rosters
            .get(&key)
            .cloned()
        {
            let roster_source = self
                .rbc_session_roster_source(key)
                .unwrap_or(RbcRosterSource::Init);
            if !roster_source.is_authoritative() {
                if let Some((refreshed, _updated)) = self.refresh_derived_rbc_session_roster(key) {
                    return refreshed;
                }
            }
            return roster;
        }
        let roster = self.rbc_roster_for_session(key);
        if !roster.is_empty() {
            self.record_rbc_session_roster(key, roster.clone(), RbcRosterSource::Derived);
        }
        roster
    }

    fn refresh_derived_rbc_session_roster(
        &mut self,
        key: super::rbc_store::SessionKey,
    ) -> Option<(Vec<PeerId>, bool)> {
        let existing_source = self
            .rbc_session_roster_source(key)
            .unwrap_or(RbcRosterSource::Init);
        if existing_source.is_authoritative() {
            return None;
        }
        let roster = self.rbc_roster_for_session(key);
        if roster.is_empty() {
            return None;
        }
        let existing_roster = self.rbc_session_roster(key);
        let updated = existing_roster != roster;
        if updated || !existing_roster.is_empty() {
            self.record_rbc_session_roster(key, roster.clone(), RbcRosterSource::Derived);
        }
        Some((roster, updated || !existing_roster.is_empty()))
    }

    #[allow(clippy::too_many_lines)]
    fn record_rbc_session_roster(
        &mut self,
        key: super::rbc_store::SessionKey,
        roster: Vec<PeerId>,
        source: RbcRosterSource,
    ) {
        if roster.is_empty() {
            return;
        }
        match self.subsystems.da_rbc.rbc.session_rosters.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(roster);
                self.subsystems
                    .da_rbc
                    .rbc
                    .session_roster_sources
                    .insert(key, source);
                if source.is_authoritative() {
                    self.subsystems
                        .da_rbc
                        .rbc
                        .persisted_full_sessions
                        .remove(&key);
                }
            }
            Entry::Occupied(mut entry) => {
                let existing_roster = entry.get();
                let existing_source = self
                    .subsystems
                    .da_rbc
                    .rbc
                    .session_roster_sources
                    .get(&key)
                    .copied()
                    .unwrap_or(RbcRosterSource::Init);
                if existing_roster == &roster {
                    let merged = existing_source.merge(source);
                    if merged != existing_source {
                        self.subsystems
                            .da_rbc
                            .rbc
                            .session_roster_sources
                            .insert(key, merged);
                        if merged.is_authoritative() && !existing_source.is_authoritative() {
                            self.subsystems
                                .da_rbc
                                .rbc
                                .persisted_full_sessions
                                .remove(&key);
                        }
                    }
                    return;
                }
                if source.is_authoritative() && !existing_source.is_authoritative() {
                    entry.insert(roster);
                    self.subsystems
                        .da_rbc
                        .rbc
                        .session_roster_sources
                        .insert(key, source);
                    self.subsystems
                        .da_rbc
                        .rbc
                        .persisted_full_sessions
                        .remove(&key);
                    if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get_mut(&key) {
                        session.ready_signatures.clear();
                        session.sent_ready = false;
                        session.delivered = false;
                        session.deliver_sender = None;
                        session.deliver_signature = None;
                    }
                    self.clear_pending_rbc(&key);
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
                    self.subsystems.da_rbc.rbc.deliver_deferral.remove(&key);
                    if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
                        self.update_rbc_status_entry(key, &session, false);
                        self.persist_rbc_session(key, &session);
                    }
                    self.publish_rbc_backlog_snapshot();
                } else if source.is_authoritative() && existing_source.is_authoritative() {
                    let local_peer = self.common_config.peer.id();
                    let local_missing = !existing_roster.iter().any(|peer| peer == local_peer);
                    let local_present = roster.iter().any(|peer| peer == local_peer);
                    let roster_superset = roster.len() > existing_roster.len()
                        && existing_roster
                            .iter()
                            .all(|peer| roster.iter().any(|candidate| candidate == peer));
                    if (local_missing && local_present) || roster_superset {
                        info!(
                            block = %key.0,
                            height = key.1,
                            view = key.2,
                            local_peer = %local_peer,
                            "refreshing authoritative RBC roster to include newly observed peers"
                        );
                        entry.insert(roster);
                        self.subsystems
                            .da_rbc
                            .rbc
                            .session_roster_sources
                            .insert(key, source);
                        self.subsystems
                            .da_rbc
                            .rbc
                            .persisted_full_sessions
                            .remove(&key);
                        if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get_mut(&key) {
                            session.ready_signatures.clear();
                            session.sent_ready = false;
                            session.delivered = false;
                            session.deliver_sender = None;
                            session.deliver_signature = None;
                        }
                        self.clear_pending_rbc(&key);
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
                        self.subsystems.da_rbc.rbc.deliver_deferral.remove(&key);
                        if let Some(session) =
                            self.subsystems.da_rbc.rbc.sessions.get(&key).cloned()
                        {
                            self.update_rbc_status_entry(key, &session, false);
                            self.persist_rbc_session(key, &session);
                        }
                        self.publish_rbc_backlog_snapshot();
                    } else {
                        warn!(
                            block = %key.0,
                            height = key.1,
                            view = key.2,
                            "conflicting authoritative RBC roster snapshot; keeping the first"
                        );
                    }
                } else if !source.is_authoritative() && !existing_source.is_authoritative() {
                    debug!(
                        block = %key.0,
                        height = key.1,
                        view = key.2,
                        "refreshing unverified RBC roster snapshot"
                    );
                    entry.insert(roster);
                    self.subsystems
                        .da_rbc
                        .rbc
                        .session_roster_sources
                        .insert(key, source);
                    if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get_mut(&key) {
                        session.ready_signatures.clear();
                        session.sent_ready = false;
                        session.delivered = false;
                        session.deliver_sender = None;
                        session.deliver_signature = None;
                    }
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
                    self.subsystems.da_rbc.rbc.deliver_deferral.remove(&key);
                    if let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
                        self.update_rbc_status_entry(key, &session, false);
                        self.persist_rbc_session(key, &session);
                    }
                    self.publish_rbc_backlog_snapshot();
                }
            }
        }
    }

    fn clear_rbc_session_roster(&mut self, key: &super::rbc_store::SessionKey) {
        self.subsystems.da_rbc.rbc.session_rosters.remove(key);
        self.subsystems
            .da_rbc
            .rbc
            .session_roster_sources
            .remove(key);
    }

    fn record_invalid_signature(
        &mut self,
        kind: InvalidSigKind,
        height: u64,
        view: u64,
        signer: u64,
    ) -> InvalidSigOutcome {
        let now = Instant::now();
        let outcome = self.invalid_sig_log.record(kind, height, view, signer, now);
        if self.invalid_sig_penalty.record(kind, signer, now) {
            let cooldown_ms =
                u64::try_from(self.config.gating.invalid_sig_penalty_cooldown.as_millis())
                    .unwrap_or(u64::MAX);
            warn!(
                height,
                view,
                signer,
                kind = kind.as_str(),
                threshold = self.config.gating.invalid_sig_penalty_threshold,
                window_ms =
                    u64::try_from(self.config.gating.invalid_sig_penalty_window.as_millis(),)
                        .unwrap_or(u64::MAX),
                cooldown_ms,
                "suppressing signer after repeated invalid signatures"
            );
        }
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

    fn record_rbc_mismatch(
        &mut self,
        peer: &PeerId,
        kind: super::status::RbcMismatchKind,
        height: u64,
        view: u64,
    ) -> RbcMismatchLogOutcome {
        super::status::record_rbc_mismatch(peer, kind);
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.inc_rbc_mismatch(peer, kind);
        }
        self.rbc_mismatch_log
            .record(peer, kind, height, view, Instant::now())
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
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(qc.height);
        let key = Self::qc_tally_key(qc);
        if let Some(tally) = self.qc_signer_tally.get(&key) {
            return Some(tally.clone());
        }

        let pops = &self.roster_validation_cache.pops;
        let (result, evidence, fallback_tally, sync_err) = {
            let state_view = self.state.view();
            let world = state_view.world();
            let stake_snapshot = match consensus_mode {
                ConsensusMode::Permissioned => None,
                ConsensusMode::Npos => CommitStakeSnapshot::from_roster(world, topology.as_ref()),
            };
            let (result, evidence) = validate_qc_with_evidence(
                &self.vote_log,
                qc,
                topology,
                world,
                pops,
                &self.common_config.chain,
                consensus_mode,
                stake_snapshot.as_ref(),
                mode_tag,
                prf_seed,
            );
            let mut fallback_tally = None;
            let mut sync_err = None;
            if matches!(&result, Err(QcValidationError::MissingVotes { .. })) {
                match tally_qc_against_block_signers(
                    qc,
                    topology,
                    world,
                    block_signers,
                    block_view,
                    pops,
                    &self.common_config.chain,
                    consensus_mode,
                    stake_snapshot.as_ref(),
                    mode_tag,
                    prf_seed,
                ) {
                    Ok(tally) => fallback_tally = Some(tally),
                    Err(err) => {
                        sync_err = Some(err);
                        fallback_tally = fallback_qc_tally_from_bitmap(
                            qc,
                            topology,
                            world,
                            pops,
                            &self.common_config.chain,
                            mode_tag,
                            prf_seed,
                        );
                    }
                }
            }
            (result, evidence, fallback_tally, sync_err)
        };
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
                    if let Some(sync_err) = sync_err {
                        record_qc_validation_error(self.telemetry_handle(), &sync_err);
                    }
                    if let Some(tally) = fallback_tally {
                        self.note_validated_qc_tally(qc, tally.clone());
                        return Some(tally);
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

    fn epoch_for_height_from_view(&self, view: &StateView<'_>, height: u64) -> u64 {
        let consensus_mode =
            super::effective_consensus_mode_for_height(view, height, self.config.consensus_mode);
        if matches!(consensus_mode, ConsensusMode::Permissioned) {
            return 0;
        }
        let schedule = super::EpochScheduleSnapshot::from_world_with_fallback(
            view.world(),
            self.config.npos.epoch_length_blocks,
        );
        let schedule_epoch = schedule.epoch_for_height(height);
        if height <= schedule.last_finalized_end() {
            return schedule_epoch;
        }
        self.epoch_manager
            .as_ref()
            .map_or(schedule_epoch, |manager| manager.epoch_for_height(height))
    }

    fn epoch_for_height(&self, height: u64) -> u64 {
        let view = self.state.view();
        self.epoch_for_height_from_view(&view, height)
    }

    fn genesis_block_hash(&self) -> Option<HashOf<BlockHeader>> {
        let height = NonZeroUsize::new(1)?;
        let block = self.kura.get_block(height)?;
        if block.header().prev_block_hash().is_some() {
            return None;
        }
        Some(block.hash())
    }

    fn genesis_roster_from_genesis_block(&self) -> Vec<PeerId> {
        let Some(genesis) = self.genesis_network.genesis.as_ref() else {
            return Vec::new();
        };
        let mut roster = Vec::new();
        for tx in genesis.0.external_transactions() {
            let Executable::Instructions(isi) = tx.instructions() else {
                continue;
            };
            for instruction in isi {
                if let Some(register) = instruction.as_any().downcast_ref::<RegisterPeerWithPop>() {
                    roster.push(register.peer.clone());
                }
            }
        }
        roster
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
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        Some(crate::sumeragi::consensus::Qc {
            phase: qc.phase,
            subject_block_hash: qc.subject_block_hash,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: qc.height,
            view: qc.view,
            epoch: qc.epoch,
            mode_tag: mode_tag.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: crate::sumeragi::consensus::QcAggregate {
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

    fn ensure_genesis_commit_roster(&mut self) {
        const GENESIS_HEIGHT: u64 = 1;
        let Some(genesis_hash) = self.genesis_block_hash() else {
            return;
        };
        if self
            .state
            .commit_roster_snapshot_for_block(GENESIS_HEIGHT, genesis_hash)
            .is_some()
        {
            return;
        }
        let (consensus_mode, mode_tag, _) = self.consensus_context_for_height(GENESIS_HEIGHT);
        let mut roster = self.genesis_roster_from_genesis_block();
        if roster.is_empty() {
            let view = self.state.view();
            roster = derive_active_topology_for_mode(
                &view,
                self.common_config.trusted_peers.value(),
                self.common_config.peer.id(),
                consensus_mode,
            );
        }
        roster = roster::canonicalize_roster_for_mode(roster, consensus_mode);
        if roster.is_empty() {
            warn!(
                height = GENESIS_HEIGHT,
                block = %genesis_hash,
                "skipping genesis commit roster seed: empty roster"
            );
            return;
        }
        let bitmap_len = roster.len().div_ceil(8);
        let zero_root = Hash::prehashed([0u8; Hash::LENGTH]);
        let epoch = self.epoch_for_height(GENESIS_HEIGHT);
        let qc = crate::sumeragi::consensus::Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: genesis_hash,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: GENESIS_HEIGHT,
            view: 0,
            epoch,
            mode_tag: mode_tag.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster.clone(),
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap: vec![0; bitmap_len],
                bls_aggregate_signature: Vec::new(),
            },
        };
        let checkpoint = ValidatorSetCheckpoint::new(
            qc.height,
            qc.view,
            qc.subject_block_hash,
            qc.parent_state_root,
            qc.post_state_root,
            qc.validator_set.clone(),
            qc.aggregate.signers_bitmap.clone(),
            qc.aggregate.bls_aggregate_signature.clone(),
            qc.validator_set_hash_version,
            None,
        );
        let stake_snapshot = match consensus_mode {
            ConsensusMode::Permissioned => None,
            ConsensusMode::Npos => {
                CommitStakeSnapshot::from_roster(self.state.view().world(), &roster)
            }
        };
        self.state
            .record_commit_roster(&qc, &checkpoint, stake_snapshot);
        info!(
            height = GENESIS_HEIGHT,
            block = %genesis_hash,
            roster_len = roster.len(),
            "seeded genesis commit roster from genesis block"
        );
    }

    fn latest_committed_qc(&self) -> Option<crate::sumeragi::consensus::QcHeaderRef> {
        let view = self.state.view();
        let block = view.latest_block()?;
        let header = block.header();
        let height = header.height().get();
        let epoch = self.epoch_for_height_from_view(&view, height);
        Some(crate::sumeragi::consensus::QcHeaderRef {
            height,
            view: header.view_change_index(),
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

    fn block_payload_available_for_progress(&self, hash: HashOf<BlockHeader>) -> bool {
        if let Some(pending) = self.pending.pending_blocks.get(&hash) {
            return !matches!(pending.validation_status, ValidationStatus::Invalid);
        }
        if let Some(inflight) = self.subsystems.commit.inflight.as_ref()
            && inflight.block_hash == hash
        {
            return !matches!(
                inflight.pending.validation_status,
                ValidationStatus::Invalid
            );
        }
        if self
            .pending
            .pending_processing
            .get()
            .is_some_and(|pending| pending == hash)
        {
            return true;
        }
        self.kura.get_block_height_by_hash(hash).is_some()
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

    fn block_known_for_lock(&self, hash: HashOf<BlockHeader>) -> bool {
        self.pending
            .pending_blocks
            .get(&hash)
            .is_some_and(|pending| {
                !pending.aborted && pending.validation_status == ValidationStatus::Valid
            })
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

    fn local_block_height_view(&self, hash: HashOf<BlockHeader>) -> Option<(u64, u64)> {
        if let Some(height) = self.kura.get_block_height_by_hash(hash) {
            if let Some(block) = self.kura.get_block(height) {
                let height = u64::try_from(height.get()).ok()?;
                let view = block.header().view_change_index();
                return Some((height, view));
            }
        }
        if let Some(pending) = self.pending.pending_blocks.get(&hash) {
            if !pending.aborted {
                return Some((pending.height, pending.block.header().view_change_index()));
            }
        }
        if let Some(inflight) = self.subsystems.commit.inflight.as_ref() {
            if inflight.block_hash == hash && !inflight.pending.aborted {
                return Some((
                    inflight.pending.height,
                    inflight.pending.block.header().view_change_index(),
                ));
            }
        }
        None
    }

    fn reconcile_new_view_tracker_with_local_blocks(&mut self) {
        let entries = std::mem::take(&mut self.subsystems.propose.new_view_tracker.entries);
        let mut updated_entries = BTreeMap::new();
        for (key, mut entry) in entries {
            let Some((local_height, local_view)) =
                self.local_block_height_view(entry.highest_qc.subject_block_hash)
            else {
                updated_entries.insert(key, entry);
                continue;
            };
            if local_height != entry.highest_qc.height {
                warn!(
                    height = key.0,
                    view = key.1,
                    highest_height = entry.highest_qc.height,
                    local_height,
                    block = %entry.highest_qc.subject_block_hash,
                    "dropping NEW_VIEW entry with highest QC height mismatch against local block"
                );
                continue;
            }
            if local_view != entry.highest_qc.view {
                debug!(
                    height = key.0,
                    view = key.1,
                    highest_view = entry.highest_qc.view,
                    local_view,
                    block = %entry.highest_qc.subject_block_hash,
                    "correcting NEW_VIEW highest QC view to match local block header"
                );
                entry.highest_qc.view = local_view;
            }
            updated_entries.insert(key, entry);
        }
        self.subsystems.propose.new_view_tracker.entries = updated_entries;
    }

    fn highest_qc_extends_locked(&self, highest: crate::sumeragi::consensus::QcHeaderRef) -> bool {
        if !self.block_known_for_lock(highest.subject_block_hash) {
            return true;
        }
        qc_extends_locked_if_present(
            self.locked_qc,
            highest,
            |hash, height| self.parent_hash_for(hash, height),
            |hash| self.block_known_for_lock(hash),
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Evidence,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::StaleHeight,
            );
            return Ok(false);
        }
        let (subject_height, _) = super::evidence::evidence_subject_height_view(evidence);
        let fallback_height = u64::try_from(self.state.view().height()).unwrap_or(u64::MAX);
        let evidence_height = subject_height.unwrap_or(fallback_height);
        let (consensus_mode, mode_tag, prf_seed) =
            self.consensus_context_for_height(evidence_height);
        let topology_peers = match &evidence.payload {
            crate::sumeragi::consensus::EvidencePayload::DoubleVote { v1, .. } => {
                self.roster_for_vote_with_mode(v1.block_hash, v1.height, v1.view, consensus_mode)
            }
            crate::sumeragi::consensus::EvidencePayload::InvalidQc { certificate, .. } => self
                .roster_for_vote_with_mode(
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Evidence,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::RosterMissing,
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
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::Evidence,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::Duplicate,
            );
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
        mut config: SumeragiConfig,
        common_config: CommonConfig,
        consensus_frame_cap: usize,
        consensus_payload_frame_cap: usize,
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
        wake_tx: Option<mpsc::SyncSender<()>>,
        block_payload_dedup: Arc<Mutex<BlockPayloadDedupCache>>,
        rbc_status_handle: rbc_status::Handle,
    ) -> Result<Self> {
        let consensus_frame_cap = frame_plaintext_cap(consensus_frame_cap);
        let mut consensus_payload_frame_cap = frame_plaintext_cap(consensus_payload_frame_cap);
        if consensus_payload_frame_cap < consensus_frame_cap {
            warn!(
                payload_cap = consensus_payload_frame_cap,
                consensus_cap = consensus_frame_cap,
                "consensus payload frame cap below consensus cap; clamping to consensus cap"
            );
            consensus_payload_frame_cap = consensus_frame_cap;
        }
        let local_peer_id = common_config.peer.id();
        let rbc_chunk_cap = rbc_chunk_payload_cap(local_peer_id, consensus_payload_frame_cap);
        if rbc_chunk_cap == 0 {
            return Err(eyre!(
                "consensus payload frame cap {consensus_payload_frame_cap} is too small to encode RBC chunk headers"
            ));
        }
        if config.rbc.chunk_max_bytes > rbc_chunk_cap {
            warn!(
                configured = config.rbc.chunk_max_bytes,
                capped = rbc_chunk_cap,
                consensus_payload_frame_cap,
                "clamping rbc_chunk_max_bytes to fit consensus payload frame cap"
            );
            config.rbc.chunk_max_bytes = rbc_chunk_cap;
        }

        let chain_id = common_config.chain.clone();
        let chain_hash = Self::derive_chain_hash(&chain_id);
        let rbc_manifest = super::rbc_store::SoftwareManifest::current();
        let backpressure_gate = BackpressureGate::new(queue.backpressure_handle().subscribe());
        let now = Instant::now();
        let mut pending_roster_activation: Option<(u64, Vec<PeerId>)> = None;
        let mut roster_to_install_now: Option<Vec<PeerId>> = None;
        let (
            consensus_mode,
            npos_collectors,
            epoch_manager,
            epoch_params,
            pacemaker_timeouts,
            pacemaker_block_time,
        ) = {
            let view = state.view();
            let mode = super::effective_consensus_mode(&view, config.consensus_mode);
            let commit_topology = derive_active_topology_for_mode(
                &view,
                common_config.trusted_peers.value(),
                common_config.peer.id(),
                mode,
            );
            let pacemaker_block_time = super::resolve_npos_block_time(&view, &config.npos);
            let pacemaker_timeouts = if matches!(mode, ConsensusMode::Npos) {
                super::resolve_npos_timeouts(&view, &config.npos)
            } else {
                SumeragiNposTimeouts::from_block_time(pacemaker_block_time)
            };
            if !WARNED_IGNORED_CONFIG_OVERRIDES.load(Ordering::Relaxed) {
                let mut ignored = Vec::new();
                if config.collectors.k != iroha_config::parameters::defaults::sumeragi::COLLECTORS_K
                {
                    ignored.push("sumeragi.collectors.k");
                }
                if config.collectors.redundant_send_r
                    != iroha_config::parameters::defaults::sumeragi::COLLECTORS_REDUNDANT_SEND_R
                {
                    ignored.push("sumeragi.collectors.redundant_send_r");
                }
                if config.npos.block_time != pacemaker_block_time {
                    ignored.push("sumeragi.npos.block_time_ms");
                }
                if view.world.sumeragi_npos_parameters().is_some() {
                    let derived = SumeragiNposTimeouts::from_block_time(pacemaker_block_time);
                    let timeouts_overridden = config.npos.timeouts.propose != derived.propose
                        || config.npos.timeouts.prevote != derived.prevote
                        || config.npos.timeouts.precommit != derived.precommit
                        || config.npos.timeouts.exec != derived.exec
                        || config.npos.timeouts.witness != derived.witness
                        || config.npos.timeouts.commit != derived.commit
                        || config.npos.timeouts.da != derived.da
                        || config.npos.timeouts.aggregator != derived.aggregator;
                    if timeouts_overridden {
                        ignored.push("sumeragi.npos.timeouts.*");
                    }
                    if config.npos.epoch_length_blocks
                        != iroha_config::parameters::defaults::sumeragi::EPOCH_LENGTH_BLOCKS
                    {
                        ignored.push("sumeragi.npos.epoch_length_blocks");
                    }
                    if config.npos.use_stake_snapshot_roster
                        != iroha_config::parameters::defaults::sumeragi::USE_STAKE_SNAPSHOT_ROSTER
                    {
                        ignored.push("sumeragi.npos.use_stake_snapshot_roster");
                    }
                    if config.npos.vrf.commit_window_blocks
                        != iroha_config::parameters::defaults::sumeragi::npos::VRF_COMMIT_WINDOW_BLOCKS
                        || config.npos.vrf.reveal_window_blocks
                            != iroha_config::parameters::defaults::sumeragi::npos::VRF_REVEAL_WINDOW_BLOCKS
                        || config.npos.vrf.commit_deadline_offset_blocks
                            != iroha_config::parameters::defaults::sumeragi::VRF_COMMIT_DEADLINE_OFFSET
                        || config.npos.vrf.reveal_deadline_offset_blocks
                            != iroha_config::parameters::defaults::sumeragi::VRF_REVEAL_DEADLINE_OFFSET
                    {
                        ignored.push("sumeragi.npos.vrf.*");
                    }
                    if config.npos.election.max_validators
                        != iroha_config::parameters::defaults::sumeragi::npos::MAX_VALIDATORS
                        || config.npos.election.min_self_bond
                            != iroha_config::parameters::defaults::sumeragi::npos::MIN_SELF_BOND
                        || config.npos.election.min_nomination_bond
                            != iroha_config::parameters::defaults::sumeragi::npos::MIN_NOMINATION_BOND
                        || config.npos.election.max_nominator_concentration_pct
                            != iroha_config::parameters::defaults::sumeragi::npos::MAX_NOMINATOR_CONCENTRATION_PCT
                        || config.npos.election.seat_band_pct
                            != iroha_config::parameters::defaults::sumeragi::npos::SEAT_BAND_PCT
                        || config.npos.election.max_entity_correlation_pct
                            != iroha_config::parameters::defaults::sumeragi::npos::MAX_ENTITY_CORRELATION_PCT
                        || config.npos.election.finality_margin_blocks
                            != iroha_config::parameters::defaults::sumeragi::npos::FINALITY_MARGIN_BLOCKS
                    {
                        ignored.push("sumeragi.npos.election.*");
                    }
                    if config.npos.reconfig.evidence_horizon_blocks
                        != iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_EVIDENCE_HORIZON_BLOCKS
                        || config.npos.reconfig.activation_lag_blocks
                            != iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS
                        || config.npos.reconfig.slashing_delay_blocks
                            != iroha_config::parameters::defaults::sumeragi::npos::SLASHING_DELAY_BLOCKS
                    {
                        ignored.push("sumeragi.npos.reconfig.*");
                    }
                }
                if !ignored.is_empty()
                    && !WARNED_IGNORED_CONFIG_OVERRIDES.swap(true, Ordering::Relaxed)
                {
                    warn!(
                        ignored = ?ignored,
                        "local sumeragi overrides are ignored when on-chain parameters are present"
                    );
                }
            }
            let mut collectors = if matches!(mode, ConsensusMode::Npos) {
                super::load_npos_collector_config(&view)
            } else {
                None
            };
            let epoch_params = super::load_npos_epoch_params(&view, &config);
            let height = view.height() as u64;
            let schedule = super::EpochScheduleSnapshot::from_world_with_fallback(
                view.world(),
                epoch_params.epoch_length_blocks,
            );
            let target_epoch = schedule.epoch_for_height(height);
            let epoch_seed_for_height = super::prf_seed_for_height(&view, height);
            let record_for_target_epoch = view.world().vrf_epochs().get(&target_epoch).cloned();
            let last_epoch_record = view
                .world()
                .vrf_epochs()
                .iter()
                .last()
                .map(|(_, record)| record.clone());
            drop(view);
            let roster_len = commit_topology.len();
            let initial_indices = compute_roster_indices_from_topology(
                &commit_topology,
                epoch_roster_provider.as_ref(),
            );
            let mut em = EpochManager::new_from_chain(&common_config.chain);
            em.set_params(
                epoch_params.epoch_length_blocks,
                epoch_params.commit_deadline_offset,
                epoch_params.reveal_deadline_offset,
            );
            if let Some(record) = record_for_target_epoch.as_ref() {
                em.restore_from_record(record);
            } else {
                em.set_epoch_seed(epoch_seed_for_height);
                em.set_epoch(target_epoch);
            }
            apply_roster_indices_to_manager(&mut em, roster_len, initial_indices);
            let seed = em.seed();
            if matches!(mode, ConsensusMode::Npos) {
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
                        k: config.collectors.k,
                        redundant_send_r: config.collectors.redundant_send_r,
                    });
                }
            }
            let manager = Some(em);
            (
                mode,
                collectors,
                manager,
                Some(epoch_params),
                pacemaker_timeouts,
                pacemaker_block_time,
            )
        };
        let qc_rebuild_cooldown = pacemaker_block_time.max(REBROADCAST_COOLDOWN_FLOOR);
        let initial_qc_rebuild = now.checked_sub(qc_rebuild_cooldown).unwrap_or(now);
        let commit_pipeline_cooldown = qc_rebuild_cooldown;
        let initial_commit_pipeline_run = now.checked_sub(commit_pipeline_cooldown).unwrap_or(now);
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
        let (staged_mode_tag, staged_mode_activation_height) = {
            let view = state.view();
            let params = view.world.parameters().sumeragi();
            staged_mode_info(params)
        };
        let mode_tag = match consensus_mode {
            ConsensusMode::Permissioned => PERMISSIONED_TAG,
            ConsensusMode::Npos => NPOS_TAG,
        };
        super::status::set_mode_tags(mode_tag, staged_mode_tag, staged_mode_activation_height);
        #[cfg(feature = "telemetry")]
        telemetry.set_mode_tags(mode_tag, staged_mode_tag, staged_mode_activation_height);

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
        let mut rbc_session_roster_sources = BTreeMap::new();
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
                                    rbc_session_roster_sources.insert(key, RbcRosterSource::Init);
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
            let max_chunks = config.rbc.pending_max_chunks.min(hard_chunk_cap).max(1);
            let hard_bytes = config.rbc.chunk_max_bytes.saturating_mul(hard_chunk_cap);
            let max_bytes = config
                .rbc
                .pending_max_bytes
                .min(hard_bytes)
                .max(config.rbc.chunk_max_bytes);
            (max_chunks, max_bytes)
        };
        Self::update_rbc_backlog_snapshot(
            &rbc_sessions,
            &BTreeMap::new(),
            pending_caps,
            config.rbc.pending_ttl,
            config.rbc.pending_session_limit,
            telemetry_option,
        );

        debug!(chain=?common_config.chain, mode=?consensus_mode, "initialising Sumeragi actor");

        let genesis_account = Self::determine_genesis_account(state.as_ref())?;
        let lane_relay = LaneRelayBroadcaster::new(network.clone());
        let pacemaker_base_interval = pacemaker_base_interval_with_propose_timeout(
            pacemaker_block_time,
            pacemaker_timeouts.propose,
            &config,
        );
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
                    .unwrap_or(config.collectors.redundant_send_r);
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
                u64::try_from(config.pacemaker.max_backoff.as_millis()).unwrap_or(u64::MAX);
            let backoff_mul = u64::from(config.pacemaker.backoff_multiplier);
            let rtt_floor_mul = u64::from(config.pacemaker.rtt_floor_multiplier);
            let jitter_frac = u64::from(config.pacemaker.jitter_frac_permille);
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
            session_roster_sources: rbc_session_roster_sources,
            status_handle: rbc_status_handle,
            payload_rebroadcast_last_sent: BTreeMap::new(),
            ready_rebroadcast_last_sent: BTreeMap::new(),
            ready_deferral: BTreeMap::new(),
            deliver_deferral: BTreeMap::new(),
            outbound_chunks: BTreeMap::new(),
            outbound_cursor: None,
            rebroadcast_cursor: None,
            persisted_full_sessions: BTreeSet::new(),
            persist_tx: None,
            persist_rx: None,
            persist_inflight: BTreeSet::new(),
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
            pacemaker_backpressure_tracker: pacing::PacemakerBackpressureTracker::new(),
            last_pacemaker_attempt: None,
            last_successful_proposal: None,
            propose_attempt_monitor: ProposeAttemptMonitor::new(),
        };
        let subsystems = ActorSubsystems {
            validation: ValidationState::new(),
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
        let roster_validation_cache = {
            let world = state.world.view();
            let trusted_pops = &common_config.trusted_peers.value().pops;
            let cache = RosterValidationCache::from_world(
                &world,
                config.npos.epoch_length_blocks,
                Some(trusted_pops),
            );
            drop(world);
            cache
        };
        let invalid_sig_penalty = InvalidSigPenalty::new(&config);

        let mut actor = Self {
            config,
            consensus_mode,
            common_config,
            chain_id,
            chain_hash,
            consensus_frame_cap,
            consensus_payload_frame_cap,
            events_sender,
            state,
            queue,
            kura,
            network,
            subsystems,
            block_payload_dedup,
            peers_gossiper,
            genesis_network,
            block_count,
            block_sync_gossip_limit,
            last_advertised_topology: BTreeSet::new(),
            last_commit_topology_hash: None,
            last_commit_topology_membership_hash: None,
            last_committed_height: block_count.0 as u64,
            #[cfg(feature = "telemetry")]
            telemetry,
            epoch_roster_provider,
            background_post_tx,
            wake_tx,
            phase_tracker: PhaseTracker::new(now),
            queue_ready_since: None,
            phase_ema: PhaseEma::new(&pacemaker_timeouts),
            evidence_store: EvidenceStore::new(),
            invalid_sig_log: InvalidSigThrottle::default(),
            rbc_mismatch_log: RbcMismatchThrottle::default(),
            invalid_sig_penalty,
            genesis_account,
            vote_log: BTreeMap::new(),
            deferred_votes: BTreeMap::new(),
            deferred_qcs: BTreeMap::new(),
            vote_roster_cache: BTreeMap::new(),
            qc_cache: BTreeMap::new(),
            qc_signer_tally: BTreeMap::new(),
            epoch_manager,
            npos_collectors,
            pending: PendingBlockState {
                pending_blocks: BTreeMap::new(),
                missing_block_requests: BTreeMap::new(),
                pending_processing: Cell::new(None),
                pending_processing_parent: Cell::new(None),
                last_commit_pipeline_run: initial_commit_pipeline_run,
                commit_pipeline_wakeup: false,
            },
            voting_block: None,
            pending_roster_activation,
            pending_mode_flip: None,
            highest_qc: None,
            locked_qc: None,
            tick_counter: 0,
            tick_timing: TickTimingMonitor::new(now),
            last_qc_rebuild: initial_qc_rebuild,
            relay_backpressure: RelayBackpressure::default(),
            queue_drop_backpressure: QueueDropBackpressure::default(),
            queue_block_backpressure: QueueBlockBackpressure::default(),
            new_view_rebroadcast_log: NewViewRebroadcastThrottle::default(),
            proposal_rebroadcast_log: PayloadRebroadcastThrottle::default(),
            payload_rebroadcast_log: PayloadRebroadcastThrottle::default(),
            block_sync_rebroadcast_log: PayloadRebroadcastThrottle::default(),
            block_sync_fetch_log: PayloadRebroadcastThrottle::default(),
            roster_validation_cache,
        };
        actor.refresh_p2p_topology();
        if let Some(roster) = roster_to_install_now {
            if let Err(err) = actor.install_elected_roster(&roster) {
                iroha_logger::warn!(?err, "failed to install elected roster on startup");
            } else {
                actor.pending_roster_activation = None;
            }
        }
        actor.refresh_commit_topology_state(&actor.effective_commit_topology());
        actor.ensure_genesis_commit_roster();
        if let Some(committed_qc) = actor.latest_committed_qc() {
            actor.highest_qc = Some(committed_qc);
            actor.locked_qc = Some(committed_qc);
            super::status::set_highest_qc(committed_qc.height, committed_qc.view);
            super::status::set_highest_qc_hash(committed_qc.subject_block_hash);
            super::status::set_locked_qc(
                committed_qc.height,
                committed_qc.view,
                Some(committed_qc.subject_block_hash),
            );
            #[cfg(feature = "telemetry")]
            {
                actor.telemetry.set_highest_qc_height(committed_qc.height);
                actor.telemetry.set_locked_qc_height(committed_qc.height);
                actor.telemetry.set_locked_qc_view(committed_qc.view);
            }
        } else {
            super::status::set_highest_qc(0, 0);
            super::status::set_highest_qc_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
                [0; Hash::LENGTH],
            )));
            super::status::set_locked_qc(0, 0, None);
            #[cfg(feature = "telemetry")]
            {
                actor.telemetry.set_highest_qc_height(0);
                actor.telemetry.set_locked_qc_height(0);
                actor.telemetry.set_locked_qc_view(0);
            }
        }
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
            queue_len = actor.queue.active_len(),
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
        let (consensus_mode, mode_tag, prf_seed) = self.consensus_context_for_height(height);
        if matches!(consensus_mode, ConsensusMode::Npos) && prf_seed.is_none() {
            return Err(eyre!(
                "missing NPoS PRF seed for height {height} in leader selection"
            ));
        }
        rotate_topology_for_mode(
            topology,
            height,
            view,
            mode_tag,
            prf_seed,
            "leader selection",
        );
        Ok(topology.leader_index())
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
                            self.config.collectors.k,
                            self.config.collectors.redundant_send_r,
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
            rbc_chunk_max_bytes: u64::try_from(sumeragi.rbc.chunk_max_bytes).unwrap_or(u64::MAX),
            rbc_session_ttl_ms: u64::try_from(sumeragi.rbc.session_ttl.as_millis())
                .unwrap_or(u64::MAX),
            rbc_store_max_sessions: u32::try_from(sumeragi.rbc.store_max_sessions)
                .unwrap_or(u32::MAX),
            rbc_store_soft_sessions: u32::try_from(sumeragi.rbc.store_soft_sessions)
                .unwrap_or(u32::MAX),
            rbc_store_max_bytes: u64::try_from(sumeragi.rbc.store_max_bytes).unwrap_or(u64::MAX),
            rbc_store_soft_bytes: u64::try_from(sumeragi.rbc.store_soft_bytes).unwrap_or(u64::MAX),
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
        super::status::set_mode_flip_kill_switch(self.config.mode_flip.enabled);
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
            .set_mode_flip_kill_switch(self.config.mode_flip.enabled);
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
        let flip_blocked = !self.config.mode_flip.enabled && effective_mode != self.consensus_mode;
        if flip_blocked && !super::status::mode_flip_blocked() {
            warn!(
                configured_mode = ?self.consensus_mode,
                effective_mode = ?effective_mode,
                staged_mode = staged_mode_tag,
                staged_height = staged_mode_activation_height,
                "mode flip is blocked by sumeragi.mode_flip.enabled=false; on-chain upgrades will not apply until re-enabled"
            );
            let now_ms = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
                .unwrap_or(0);
            super::status::note_mode_flip_blocked("mode_flip_disabled", now_ms);
            #[cfg(feature = "telemetry")]
            self.telemetry
                .inc_mode_flip_blocked(self.mode_tag(), now_ms);
        } else if self.config.mode_flip.enabled {
            super::status::clear_mode_flip_blocked();
        }
        if self.config.mode_flip.enabled {
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

    fn missing_block_next_due(&self, now: Instant) -> Option<Instant> {
        let mut next_due: Option<Instant> = None;
        for stats in self.pending.missing_block_requests.values() {
            let mut candidate = if stats.retry_window.is_zero() {
                Some(now)
            } else {
                stats.last_requested.checked_add(stats.retry_window)
            };
            if !stats.view_change_triggered_in_view() {
                if let Some(window) = stats.view_change_window {
                    if !window.is_zero() {
                        if let Some(deadline) = stats.first_seen.checked_add(window) {
                            candidate = Some(candidate.map_or(deadline, |prev| prev.min(deadline)));
                        }
                    }
                }
            }
            let Some(candidate) = candidate else {
                continue;
            };
            let candidate = if candidate < now { now } else { candidate };
            if candidate == now {
                return Some(now);
            }
            next_due = Some(next_due.map_or(candidate, |prev| prev.min(candidate)));
        }
        next_due
    }

    fn merge_deadline(current: Option<Instant>, candidate: Option<Instant>) -> Option<Instant> {
        match (current, candidate) {
            (Some(existing), Some(next)) => Some(existing.min(next)),
            (None, Some(next)) => Some(next),
            (Some(existing), None) => Some(existing),
            (None, None) => None,
        }
    }

    fn pending_block_next_due(&self, now: Instant) -> Option<Instant> {
        if self.pending.pending_blocks.is_empty() {
            return None;
        }

        let da_enabled = self.runtime_da_enabled();
        let quorum_timeout = self.quorum_timeout(da_enabled);
        let quorum_reschedule_cooldown = quorum_timeout.max(QUORUM_RESCHEDULE_COOLDOWN);
        let retention_factor = self
            .config
            .recovery
            .missing_block_signer_fallback_attempts
            .saturating_add(2)
            .max(4);
        let aborted_retention = quorum_reschedule_cooldown.saturating_mul(retention_factor);
        let rebroadcast_cooldown = self.rebroadcast_cooldown();
        let (tip_height, tip_hash) = {
            let view = self.state.view();
            (view.height(), view.latest_block_hash())
        };

        let mut next_due: Option<Instant> = None;
        for (hash, pending) in &self.pending.pending_blocks {
            if pending.aborted {
                if aborted_retention == Duration::ZERO {
                    return Some(now);
                }
                let expiry = pending
                    .inserted_at
                    .checked_add(aborted_retention)
                    .map_or(now, |deadline| deadline.max(now));
                next_due = Self::merge_deadline(next_due, Some(expiry));
                continue;
            }

            if !pending_extends_tip(
                pending.height,
                pending.block.header().prev_block_hash(),
                tip_height,
                tip_hash,
            ) {
                continue;
            }

            if pending.kura_aborted {
                next_due = Self::merge_deadline(next_due, Some(now));
            } else if let Some(deadline) = pending.next_kura_retry {
                next_due = Self::merge_deadline(next_due, Some(deadline.max(now)));
            }

            let has_precommit_votes = pending.precommit_vote_sent
                || self.vote_log.values().any(|vote| {
                    vote.phase == crate::sumeragi::consensus::Phase::Commit
                        && vote.block_hash == *hash
                        && vote.height == pending.height
                        && vote.view == pending.view
                });
            if has_precommit_votes && !pending.commit_qc_seen {
                let expected_epoch = self.epoch_for_height(pending.height);
                let has_precommit_qc = cached_qc_for(
                    &self.qc_cache,
                    crate::sumeragi::consensus::Phase::Commit,
                    *hash,
                    pending.height,
                    pending.view,
                    expected_epoch,
                )
                .is_some();
                if !has_precommit_qc {
                    let deadline = pending
                        .last_precommit_rebroadcast
                        .and_then(|last| last.checked_add(rebroadcast_cooldown))
                        .unwrap_or(now)
                        .max(now);
                    next_due = Self::merge_deadline(next_due, Some(deadline));
                }
            }

            if quorum_timeout != Duration::ZERO {
                let age = now.saturating_duration_since(pending.inserted_at);
                let deadline = if age < quorum_timeout {
                    pending
                        .inserted_at
                        .checked_add(quorum_timeout)
                        .unwrap_or(now)
                } else {
                    pending
                        .last_quorum_reschedule
                        .and_then(|last| last.checked_add(quorum_reschedule_cooldown))
                        .unwrap_or(now)
                };
                let deadline = deadline.max(now);
                next_due = Self::merge_deadline(next_due, Some(deadline));
            }
        }

        next_due
    }

    fn idle_view_next_due(&self, now: Instant) -> Option<Instant> {
        if self.has_active_pending_blocks() {
            return None;
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
        let Some(age) = self.phase_tracker.view_age(height, now) else {
            // Avoid scheduling idle view changes before the round tracker is seeded.
            return None;
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
            self.runtime_da_enabled(),
        );
        if timeout == Duration::ZERO {
            return None;
        }
        if age >= timeout {
            return Some(now);
        }
        Some(now + timeout.saturating_sub(age))
    }

    fn rbc_next_due(&self, now: Instant) -> Option<Instant> {
        let rbc = &self.subsystems.da_rbc.rbc;
        if rbc.sessions.is_empty()
            && rbc.pending.is_empty()
            && rbc.persist_inflight.is_empty()
            && rbc.outbound_chunks.is_empty()
        {
            return None;
        }

        let mut next_due: Option<Instant> = None;
        let payload_cooldown = self.payload_rebroadcast_cooldown();
        let ready_cooldown = self.rebroadcast_cooldown();

        for (key, session) in &rbc.sessions {
            if session.delivered || session.is_invalid() {
                continue;
            }

            if !session.sent_ready {
                let roster = self.rbc_session_roster(*key);
                let roster_source = self
                    .rbc_session_roster_source(*key)
                    .unwrap_or(RbcRosterSource::Init);
                if !roster.is_empty() && roster_source.is_authoritative() {
                    return Some(now);
                }
            }

            let roster = self.rbc_session_roster(*key);
            let roster_source = self
                .rbc_session_roster_source(*key)
                .unwrap_or(RbcRosterSource::Init);
            if roster.is_empty() || !roster_source.is_authoritative() {
                continue;
            }
            let topology = super::network_topology::Topology::new(roster);
            let required = self.rbc_deliver_quorum(&topology);
            let ready_quorum = session.ready_signatures.len() >= required;
            let total_chunks = session.total_chunks();
            let missing_chunks = total_chunks != 0 && session.received_chunks() < total_chunks;
            if ready_quorum && !missing_chunks {
                continue;
            }

            if missing_chunks || !ready_quorum {
                let deadline = rbc
                    .payload_rebroadcast_last_sent
                    .get(key)
                    .and_then(|last| last.checked_add(payload_cooldown))
                    .unwrap_or(now)
                    .max(now);
                next_due = Self::merge_deadline(next_due, Some(deadline));
            }
            if !ready_quorum && !session.ready_signatures.is_empty() {
                let deadline = rbc
                    .ready_rebroadcast_last_sent
                    .get(key)
                    .and_then(|last| last.checked_add(ready_cooldown))
                    .unwrap_or(now)
                    .max(now);
                next_due = Self::merge_deadline(next_due, Some(deadline));
            }
        }

        if !rbc.outbound_chunks.is_empty() {
            next_due = Self::merge_deadline(next_due, Some(now));
        }

        let ttl = self.config.rbc.session_ttl;
        if ttl != Duration::ZERO {
            let now_system = SystemTime::now();
            if let Some(due_in) = rbc.status_handle.next_stale_due(ttl, now_system) {
                let deadline = now.checked_add(due_in).unwrap_or(now);
                next_due = Self::merge_deadline(next_due, Some(deadline));
            }
        }

        next_due
    }

    pub(super) fn refresh_worker_loop_config(&mut self, cfg: &mut super::WorkerLoopConfig) {
        let view = self.state.view();
        let mode = super::effective_consensus_mode(&view, self.consensus_mode);
        let params = view.world.parameters().sumeragi();
        let block_time = self.block_time_for_mode(&view, mode);
        let commit_time = self.commit_timeout_for_mode(&view, mode);
        let da_enabled = params.da_enabled();
        drop(view);

        let time_budget = super::worker_time_budget(
            block_time,
            commit_time,
            da_enabled,
            self.da_quorum_timeout_multiplier(),
            self.config.worker.iteration_budget_cap,
        );
        let vote_rx_drain_budget = super::vote_rx_drain_budget(
            block_time,
            commit_time,
            da_enabled,
            self.da_quorum_timeout_multiplier(),
            Duration::from_secs(8),
            self.config.worker.iteration_budget_cap,
        )
        .max(time_budget);
        let non_vote_drain_budget = super::cap_drain_budget(
            block_time / 2,
            time_budget,
            self.config.worker.iteration_budget_cap,
        );
        let rbc_chunk_drain_budget = super::cap_rbc_drain_budget(
            block_time / 2,
            time_budget,
            self.config.worker.iteration_budget_cap,
        );
        let block_rx_drain_budget = super::cap_drain_budget(
            block_time / 4,
            time_budget,
            self.config.worker.iteration_budget_cap,
        );
        let starve_max = block_time.max(commit_time).max(time_budget);
        let tick_min_gap = super::idle_tick_gap(
            block_time,
            commit_time,
            time_budget,
            self.config.worker.tick_work_budget_cap,
        );
        let mut tick_max_gap = if block_time.is_zero() {
            time_budget
        } else {
            block_time.min(time_budget)
        };
        tick_max_gap = tick_max_gap.max(tick_min_gap);

        cfg.time_budget = time_budget;
        cfg.drain_budget_cap = self.config.worker.iteration_drain_budget_cap;
        cfg.vote_rx_drain_budget = vote_rx_drain_budget;
        cfg.block_payload_rx_drain_budget = non_vote_drain_budget;
        cfg.block_rx_drain_budget = block_rx_drain_budget;
        cfg.rbc_chunk_rx_drain_budget = rbc_chunk_drain_budget;
        cfg.tick_min_gap = tick_min_gap;
        cfg.tick_max_gap = tick_max_gap;
        cfg.block_rx_starve_max = starve_max;
        cfg.non_vote_starve_max = starve_max;
    }

    pub(super) fn next_tick_deadline(&self, now: Instant) -> Option<Instant> {
        let queue_len = self.queue.active_len();
        if queue_len > 0 {
            return Some(now);
        }
        if self.pending.commit_pipeline_wakeup {
            return Some(now);
        }
        let mut next_due = Self::merge_deadline(None, self.pending_block_next_due(now));

        if !self.deferred_votes.is_empty() || !self.deferred_qcs.is_empty() {
            return Some(now);
        }

        next_due = Self::merge_deadline(next_due, self.rbc_next_due(now));

        if let Some(inflight) = self.subsystems.commit.inflight.as_ref() {
            let timeout = self.config.persistence.commit_inflight_timeout;
            if !timeout.is_zero() {
                let deadline = inflight.enqueue_time.checked_add(timeout).unwrap_or(now);
                next_due = Self::merge_deadline(next_due, Some(deadline.max(now)));
            }
        }

        next_due = Self::merge_deadline(next_due, self.missing_block_next_due(now));
        next_due = Self::merge_deadline(next_due, self.idle_view_next_due(now));

        next_due
    }

    pub(super) fn should_tick(&self) -> bool {
        self.next_tick_deadline(Instant::now()).is_some()
    }

    fn tick_work_budget_deadline(&self, tick_start: Instant) -> Option<Instant> {
        let cap = self.config.worker.tick_work_budget_cap;
        if cap.is_zero() {
            None
        } else {
            Some(tick_start.checked_add(cap).unwrap_or(tick_start))
        }
    }

    fn tick_budget_exhausted(tick_deadline: Option<Instant>, now: Instant) -> bool {
        matches!(tick_deadline, Some(deadline) if now >= deadline)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn tick(&mut self) -> bool {
        let tick_start = Instant::now();
        self.tick_counter = self.tick_counter.saturating_add(1);
        if self.tick_counter.is_multiple_of(50) {
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.heartbeat");
            let view = self.state.view();
            let backpressure = self.queue_backpressure_state();
            iroha_logger::info!(
                height = view.height(),
                queue_len = self.queue.active_len(),
                queue_cap = backpressure.capacity().get(),
                queue_saturated = backpressure.is_saturated(),
                tick = self.tick_counter,
                "sumeragi tick heartbeat"
            );
        }
        let mut progress = self.tick_mode_management();
        let expired_culled = {
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.cull_expired");
            self.queue.cull_expired_entries_if_due()
        };
        if expired_culled > 0 {
            progress = true;
        }
        let rbc_persist_progress = self.poll_rbc_persist_results_inner();
        let now = tick_start;
        let tick_deadline = self.tick_work_budget_deadline(tick_start);
        let queue_len = self.queue.active_len();
        let adaptive_progress = self.apply_adaptive_observability(now);
        let (refresh_progress, refresh_cost) = {
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.refresh_backpressure_state");
            let step_start = Instant::now();
            let progress = self.refresh_backpressure_state();
            (progress, step_start.elapsed())
        };
        let (committed_progress, committed_poll_cost) = {
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.poll_committed_blocks");
            let step_start = Instant::now();
            let progress = self.poll_committed_blocks();
            (progress, step_start.elapsed())
        };
        let deferred_qc_progress = {
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.replay_deferred_qcs");
            self.try_replay_deferred_qcs()
        };
        let deferred_vote_progress = {
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.replay_deferred_votes");
            self.try_replay_deferred_votes()
        };
        let (missing_block_progress, missing_block_cost) = {
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.retry_missing_block");
            let step_start = Instant::now();
            let progress = self.retry_missing_block_requests(now);
            (progress, step_start.elapsed())
        };
        let (reschedule_progress, reschedule_cost) = {
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.reschedule_pending");
            let step_start = Instant::now();
            let progress = self.reschedule_stale_pending_blocks();
            (progress, step_start.elapsed())
        };
        let (idle_view_progress, idle_view_cost) = {
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.force_view_change_if_idle");
            let step_start = Instant::now();
            let progress = self.force_view_change_if_idle(now);
            (progress, step_start.elapsed())
        };
        let rbc_rebroadcast_progress = self.rebroadcast_stalled_rbc_payloads(now);
        let rbc_outbound_progress = self.flush_rbc_outbound_chunks(now);
        let rbc_session_ttl_progress = self.prune_stale_rbc_sessions(SystemTime::now());
        let mut commit_pipeline_cost = Duration::ZERO;
        let mut propose_cost = Duration::ZERO;
        progress |= rbc_persist_progress
            || adaptive_progress
            || refresh_progress
            || committed_progress
            || deferred_qc_progress
            || deferred_vote_progress
            || missing_block_progress
            || reschedule_progress
            || idle_view_progress
            || rbc_rebroadcast_progress
            || rbc_outbound_progress
            || rbc_session_ttl_progress;
        let commit_wakeup = self.pending.commit_pipeline_wakeup;
        if should_run_commit_pipeline_on_tick(self.active_pending_blocks_len())
            || self.subsystems.commit.inflight.is_some()
            || commit_wakeup
        {
            self.pending.commit_pipeline_wakeup = false;
            let pipeline_start = Instant::now();
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.commit_pipeline");
            self.process_commit_candidates_with_trigger(CommitPipelineTrigger::Tick, tick_deadline);
            commit_pipeline_cost = pipeline_start.elapsed();
            progress = true;
        }
        let proposal_backpressure = {
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.proposal_backpressure");
            self.proposal_backpressure_at(now)
        };
        if let Some(telemetry) = self.telemetry_handle() {
            let pending_blocks_total = self.pending.pending_blocks.len() as u64;
            let pending_blocks_blocking = if pending_blocks_total == 0 {
                0
            } else {
                self.blocking_pending_blocks_len() as u64
            };
            let commit_inflight_queue_depth = u64::from(self.subsystems.commit.inflight.is_some());
            telemetry.record_pending_block_metrics(
                pending_blocks_total,
                pending_blocks_blocking,
                commit_inflight_queue_depth,
            );
        }
        let queue_ready = queue_len > 0 && !proposal_backpressure.active_pending;
        if queue_ready {
            self.subsystems.propose.pacemaker.next_deadline = now;
        }
        if queue_ready
            && !proposal_backpressure.should_defer()
            && self.subsystems.commit.inflight.is_none()
        {
            let propose_start = Instant::now();
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.pacemaker_propose_ready");
            if Self::tick_budget_exhausted(tick_deadline, propose_start) {
                self.subsystems.propose.pacemaker.next_deadline = propose_start;
            } else {
                if self.on_pacemaker_propose_ready(now) {
                    progress = true;
                }
            }
            propose_cost = propose_cost.saturating_add(propose_start.elapsed());
        }
        let state = proposal_backpressure.queue_state;
        let pacemaker_eval_start = Instant::now();
        let (log_initial_deferral, log_fire_deferral, should_attempt_proposal) =
            Self::evaluate_pacemaker(
                &mut self.subsystems.propose.pacemaker,
                &mut self.subsystems.propose.pacemaker_backpressure,
                proposal_backpressure,
                now,
            );
        let pacemaker_eval_cost = pacemaker_eval_start.elapsed();
        let deferring = proposal_backpressure.should_defer() && !should_attempt_proposal;
        let telemetry = self.telemetry_handle().cloned();
        self.subsystems
            .propose
            .pacemaker_backpressure_tracker
            .update(proposal_backpressure, deferring, now, telemetry);
        if log_initial_deferral || log_fire_deferral {
            self.on_pacemaker_backpressure_deferral(now, state);
        }
        if should_attempt_proposal && self.subsystems.commit.inflight.is_none() {
            let propose_start = Instant::now();
            let _view_ctx = StateViewContextGuard::new("sumeragi.tick.pacemaker_attempt");
            if Self::tick_budget_exhausted(tick_deadline, propose_start) {
                self.subsystems.propose.pacemaker.next_deadline = propose_start;
            } else {
                if self.on_pacemaker_propose_ready(now) {
                    progress = true;
                }
            }
            propose_cost = propose_cost.saturating_add(propose_start.elapsed());
        }
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.observe_pacemaker_eval_ms(pacemaker_eval_cost);
            if propose_cost != Duration::ZERO {
                telemetry.observe_pacemaker_propose_ms(propose_cost);
            }
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
                missing_block_ms = missing_block_cost.as_millis(),
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
        parent_view: Option<u64>,
    ) -> Result<(), HintMismatch> {
        if &hint.block_hash != block_hash {
            return Err(HintMismatch::BlockHash);
        }
        let block_height = header.height().get();
        if hint.height != block_height {
            return Err(HintMismatch::Height);
        }
        let block_view = header.view_change_index();
        if hint.view != block_view {
            return Err(HintMismatch::View);
        }
        if hint.highest_qc.height.saturating_add(1) != block_height {
            return Err(HintMismatch::HighestQcHeight);
        }
        if let Some(parent_hash) = header.prev_block_hash() {
            if hint.highest_qc.subject_block_hash != parent_hash {
                return Err(HintMismatch::HighestQcParentHash);
            }
        }
        if let Some(parent_view) = parent_view {
            if hint.highest_qc.view != parent_view {
                return Err(HintMismatch::HighestQcView);
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
        let prev_height = self.phase_tracker.round_height;
        let prev_view = self.phase_tracker.round_view;
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

        super::status::set_view_change_index(view);
        let advanced = match prev_height {
            Some(prev_height) if prev_height == height => view > prev_view,
            _ => view > 0,
        };
        if advanced {
            super::status::inc_view_change_install();
            if let Some(telemetry) = self.telemetry_handle() {
                telemetry.inc_view_change_install();
            }
        }
    }

    fn note_view_change_from_block(&mut self, height: u64, view: u64) {
        let prev_height = self.phase_tracker.round_height;
        let prev_view = self.phase_tracker.round_view;
        if let Some(prev_height) = prev_height {
            if height < prev_height {
                return;
            }
            if height == prev_height && view < prev_view {
                return;
            }
        }
        let advanced = match prev_height {
            Some(prev_height) if prev_height == height => view > prev_view,
            _ => view > 0,
        };
        if prev_height != Some(height) || view > prev_view {
            self.phase_tracker
                .on_view_change(height, view, Instant::now());
        }
        super::status::set_view_change_index(view);
        if advanced {
            super::status::inc_view_change_install();
            if let Some(telemetry) = self.telemetry_handle() {
                telemetry.inc_view_change_install();
            }
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

    fn record_consensus_message_handling(
        &self,
        kind: super::status::ConsensusMessageKind,
        outcome: super::status::ConsensusMessageOutcome,
        reason: super::status::ConsensusMessageReason,
    ) {
        super::status::record_consensus_message_handling(kind, outcome, reason);
        #[cfg(feature = "telemetry")]
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.note_consensus_message_handling(
                kind.as_str(),
                outcome.as_str(),
                reason.as_str(),
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn on_block_message(&mut self, msg: super::InboundBlockMessage) -> Result<()> {
        let super::InboundBlockMessage {
            message: msg,
            sender,
        } = msg;
        debug!(message=%Self::block_message_kind(&msg), "received consensus block message");
        self.note_message_received(&msg);
        if let Some((height, view)) = Self::block_message_height_view(&msg) {
            if self.should_drop_future_consensus_message(
                height,
                view,
                Self::block_message_kind(&msg),
            ) {
                match &msg {
                    BlockMessage::BlockCreated(created) => {
                        let header = created.block.header();
                        let block_hash = created.block.hash();
                        let height = header.height().get();
                        let view = header.view_change_index();
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::BlockCreated,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::FutureWindow,
                        );
                        if let Some(parent_hash) = header.prev_block_hash() {
                            let local_height = {
                                let view = self.state.view();
                                u64::try_from(view.height()).unwrap_or(u64::MAX)
                            };
                            let expected_height = local_height.saturating_add(1);
                            let active_commit_topology = self.effective_commit_topology();
                            let (consensus_mode, _, _) = self.consensus_context_for_height(height);
                            let mut commit_topology = self.roster_for_vote_with_mode(
                                block_hash,
                                height,
                                view,
                                consensus_mode,
                            );
                            if commit_topology.is_empty() {
                                commit_topology.clone_from(&active_commit_topology);
                            }
                            let expected_usize = usize::try_from(expected_height).ok();
                            let actual_usize = usize::try_from(height).ok();
                            self.request_missing_parent(
                                block_hash,
                                height,
                                view,
                                parent_hash,
                                &commit_topology,
                                None,
                                expected_usize,
                                actual_usize,
                                "block_created_future_window",
                            );
                            if height > expected_height.saturating_add(1) {
                                self.request_missing_parents_for_gap(
                                    &active_commit_topology,
                                    None,
                                    "block_created_future_gap",
                                );
                            }
                        }
                    }
                    BlockMessage::ExecWitness(_) => {
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::ExecWitness,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::FutureWindow,
                        );
                    }
                    BlockMessage::ProposalHint(_) => {
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::ProposalHint,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::FutureWindow,
                        );
                    }
                    BlockMessage::Proposal(_) => {
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::Proposal,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::FutureWindow,
                        );
                    }
                    BlockMessage::QcVote(_) => {
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::QcVote,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::FutureWindow,
                        );
                    }
                    BlockMessage::Qc(_) => {
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::Qc,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::FutureWindow,
                        );
                    }
                    BlockMessage::RbcInit(_) => {
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::RbcInit,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::FutureWindow,
                        );
                    }
                    BlockMessage::RbcChunk(_) => {
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::RbcChunk,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::FutureWindow,
                        );
                    }
                    BlockMessage::RbcReady(_) => {
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::RbcReady,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::FutureWindow,
                        );
                    }
                    BlockMessage::RbcDeliver(deliver) => {
                        let key =
                            Self::session_key(&deliver.block_hash, deliver.height, deliver.view);
                        self.record_consensus_message_handling(
                            super::status::ConsensusMessageKind::RbcDeliver,
                            super::status::ConsensusMessageOutcome::Dropped,
                            super::status::ConsensusMessageReason::FutureWindow,
                        );
                        self.request_missing_block_for_pending_rbc(
                            key,
                            "rbc_deliver_future_window",
                            None,
                        );
                    }
                    _ => {}
                }
                return Ok(());
            }
        }
        match msg {
            BlockMessage::ConsensusParams(advert) => self.handle_consensus_params(advert),
            BlockMessage::BlockCreated(block) => self.handle_block_created(block, sender),
            BlockMessage::BlockSyncUpdate(update) => self.handle_block_sync_update(update, sender),
            BlockMessage::ProposalHint(hint) => self.handle_proposal_hint(hint),
            BlockMessage::QcVote(vote) => {
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
            BlockMessage::Qc(cert) => self.handle_qc(cert),
            BlockMessage::VrfCommit(commit) => self.handle_vrf_commit(commit),
            BlockMessage::VrfReveal(reveal) => self.handle_vrf_reveal(reveal),
            BlockMessage::ExecWitness(witness) => {
                self.handle_exec_witness(witness);
                Ok(())
            }
            BlockMessage::RbcInit(init) => self.handle_rbc_init(init, sender),
            BlockMessage::RbcChunk(chunk) => self.handle_rbc_chunk(chunk, sender),
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
            let expected = crate::merge::merge_qc_message_digest(&self.chain_id, candidate);
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
            let view = candidate.view;
            let message_digest = crate::merge::merge_qc_message_digest(&self.chain_id, &candidate);
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

    fn prepare_block_sync_update_for_broadcast(
        &self,
        update: &mut super::message::BlockSyncUpdate,
        consensus_mode: ConsensusMode,
    ) -> bool {
        if !self.trim_block_sync_update_for_frame_cap(update) {
            return false;
        }
        block_sync_update_has_roster(update, consensus_mode)
    }

    fn trim_block_sync_update_for_frame_cap(
        &self,
        update: &mut super::message::BlockSyncUpdate,
    ) -> bool {
        let origin = self.common_config.peer.id();
        let payload_cap = self.consensus_payload_frame_cap;
        let mut wire_len = block_sync_update_wire_len(origin, update);
        if wire_len <= payload_cap {
            return true;
        }

        if !update.commit_votes.is_empty() {
            update.commit_votes.clear();
            wire_len = block_sync_update_wire_len(origin, update);
            if wire_len <= payload_cap {
                return true;
            }
        }

        let height = update.block.header().height().get();
        let (consensus_mode, _, _) = self.consensus_context_for_height(height);

        match consensus_mode {
            ConsensusMode::Permissioned => {
                if update.stake_snapshot.is_some() {
                    update.stake_snapshot = None;
                    wire_len = block_sync_update_wire_len(origin, update);
                    if wire_len <= payload_cap {
                        return true;
                    }
                }
                let checkpoint = update.validator_checkpoint.clone();
                let commit_qc = update.commit_qc.clone();
                if let (Some(checkpoint), Some(commit_qc)) = (checkpoint, commit_qc) {
                    update.commit_qc = None;
                    wire_len = block_sync_update_wire_len(origin, update);
                    if wire_len <= payload_cap {
                        return true;
                    }
                    update.commit_qc = Some(commit_qc.clone());
                    update.validator_checkpoint = None;
                    wire_len = block_sync_update_wire_len(origin, update);
                    if wire_len <= payload_cap {
                        return true;
                    }
                    update.validator_checkpoint = Some(checkpoint);
                    update.commit_qc = Some(commit_qc);
                }
            }
            ConsensusMode::Npos => {
                let checkpoint = update.validator_checkpoint.clone();
                let commit_qc = update.commit_qc.clone();
                if let (Some(checkpoint), Some(commit_qc)) = (checkpoint, commit_qc) {
                    update.commit_qc = None;
                    wire_len = block_sync_update_wire_len(origin, update);
                    if wire_len <= payload_cap {
                        return true;
                    }
                    update.commit_qc = Some(commit_qc.clone());
                    update.validator_checkpoint = None;
                    wire_len = block_sync_update_wire_len(origin, update);
                    if wire_len <= payload_cap {
                        return true;
                    }
                    update.validator_checkpoint = Some(checkpoint);
                    update.commit_qc = Some(commit_qc);
                }
            }
        }

        false
    }

    fn block_message_frame_cap(&self, msg: &BlockMessage) -> usize {
        match msg {
            BlockMessage::BlockCreated(_)
            | BlockMessage::BlockSyncUpdate(_)
            | BlockMessage::FetchPendingBlock(_)
            | BlockMessage::Proposal(_)
            | BlockMessage::RbcChunk(_)
            | BlockMessage::RbcInit(_)
            | BlockMessage::RbcDeliver(_)
            | BlockMessage::RbcReady(_) => self.consensus_payload_frame_cap,
            BlockMessage::ConsensusParams(_)
            | BlockMessage::ExecWitness(_)
            | BlockMessage::ProposalHint(_)
            | BlockMessage::Qc(_)
            | BlockMessage::QcVote(_)
            | BlockMessage::VrfCommit(_)
            | BlockMessage::VrfReveal(_) => self.consensus_frame_cap,
        }
    }

    fn prepare_background_block_message(&self, msg: &mut BlockMessage) -> bool {
        let origin = self.common_config.peer.id();
        let mut wire_len = consensus_block_wire_len(origin, msg);
        let cap = self.block_message_frame_cap(msg);
        if let BlockMessage::BlockSyncUpdate(update) = msg {
            if wire_len > cap {
                if !self.trim_block_sync_update_for_frame_cap(update) {
                    warn!(
                        kind = Self::block_message_kind(msg),
                        wire_len, cap, "dropping consensus message over frame cap"
                    );
                    return false;
                }
                wire_len = consensus_block_wire_len(origin, msg);
            }
        }
        if wire_len > cap {
            warn!(
                kind = Self::block_message_kind(msg),
                wire_len, cap, "dropping consensus message over frame cap"
            );
            return false;
        }
        true
    }

    fn schedule_background(&mut self, request: BackgroundRequest) {
        let request = match request {
            BackgroundRequest::Post { peer, mut msg } => {
                if !self.prepare_background_block_message(&mut msg) {
                    return;
                }
                BackgroundRequest::Post { peer, msg }
            }
            BackgroundRequest::Broadcast { mut msg } => {
                if !self.prepare_background_block_message(&mut msg) {
                    return;
                }
                BackgroundRequest::Broadcast { msg }
            }
            other => other,
        };
        if self.config.debug.disable_background_worker {
            self.dispatch_background_inline(request);
            return;
        }
        let bypass_queue = matches!(
            &request,
            BackgroundRequest::Post {
                msg: BlockMessage::Proposal(_),
                ..
            } | BackgroundRequest::Post {
                msg: BlockMessage::BlockCreated(_),
                ..
            } | BackgroundRequest::Post {
                msg: BlockMessage::RbcInit(_),
                ..
            } | BackgroundRequest::Post {
                msg: BlockMessage::RbcChunk(_),
                ..
            } | BackgroundRequest::Broadcast {
                msg: BlockMessage::Proposal(_),
            } | BackgroundRequest::Broadcast {
                msg: BlockMessage::BlockCreated(_),
            } | BackgroundRequest::Broadcast {
                msg: BlockMessage::RbcInit(_),
            } | BackgroundRequest::Broadcast {
                msg: BlockMessage::RbcChunk(_),
            }
        );
        if bypass_queue {
            self.dispatch_background_fallback(request);
            return;
        }
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
        if let Err(request) = dispatched {
            self.dispatch_background_fallback(*request);
        }
    }

    fn dispatch_background_inline(&mut self, request: BackgroundRequest) {
        #[cfg(feature = "telemetry")]
        let enqueued_at = Instant::now();
        #[cfg(feature = "telemetry")]
        let (kind, peer_for_metrics, msg_for_metrics) = match &request {
            BackgroundRequest::Post { peer, msg } => ("Post", Some(peer.clone()), Some(msg)),
            BackgroundRequest::PostControlFlow { peer, .. } => {
                ("PostControlFlow", Some(peer.clone()), None)
            }
            BackgroundRequest::Broadcast { msg } => ("Broadcast", None, Some(msg)),
            BackgroundRequest::BroadcastControlFlow { .. } => ("BroadcastControlFlow", None, None),
        };

        #[cfg(feature = "telemetry")]
        {
            self.telemetry.inc_bg_post_enqueued(kind);
            if let Some(peer) = peer_for_metrics.as_ref() {
                self.telemetry.inc_post_to_peer(peer);
                self.telemetry.inc_bg_post_queue_depth_for_peer(peer);
            }
            if let Some(msg) = msg_for_metrics {
                self.telemetry.note_consensus_message_sent(msg);
            }
        }

        self.dispatch_background_fallback(request);

        #[cfg(feature = "telemetry")]
        {
            self.telemetry.dec_bg_post_queue_depth();
            if let Some(peer) = peer_for_metrics.as_ref() {
                self.telemetry.dec_bg_post_queue_depth_for_peer(peer);
            }
            let age_ms = enqueued_at.elapsed().as_secs_f64() * 1000.0;
            self.telemetry.observe_bg_post_age_ms(kind, age_ms);
        }
    }

    fn dispatch_background_fallback(&mut self, request: BackgroundRequest) {
        match request {
            BackgroundRequest::Post { peer, msg } => {
                let priority = msg.priority();
                self.network.post(iroha_p2p::Post {
                    data: NetworkMessage::SumeragiBlock(Box::new(msg)),
                    peer_id: peer,
                    priority,
                });
            }
            BackgroundRequest::PostControlFlow { peer, frame } => {
                self.network.post(iroha_p2p::Post {
                    data: NetworkMessage::SumeragiControlFlow(Box::new(frame)),
                    peer_id: peer,
                    priority: iroha_p2p::Priority::High,
                });
            }
            BackgroundRequest::Broadcast { msg } => {
                let priority = msg.priority();
                self.network.broadcast(iroha_p2p::Broadcast {
                    data: NetworkMessage::SumeragiBlock(Box::new(msg)),
                    priority,
                });
            }
            BackgroundRequest::BroadcastControlFlow { frame } => {
                self.network.broadcast(iroha_p2p::Broadcast {
                    data: NetworkMessage::SumeragiControlFlow(Box::new(frame)),
                    priority: iroha_p2p::Priority::High,
                });
            }
        }
    }

    fn block_message_height_view(msg: &BlockMessage) -> Option<(u64, u64)> {
        match msg {
            BlockMessage::BlockCreated(block) => {
                let header = block.block.header();
                Some((header.height().get(), header.view_change_index()))
            }
            BlockMessage::BlockSyncUpdate(_)
            | BlockMessage::ConsensusParams(_)
            | BlockMessage::VrfCommit(_)
            | BlockMessage::VrfReveal(_)
            | BlockMessage::FetchPendingBlock(_) => None,
            BlockMessage::ExecWitness(witness) => Some((witness.height, witness.view)),
            BlockMessage::RbcInit(init) => Some((init.height, init.view)),
            BlockMessage::RbcChunk(chunk) => Some((chunk.height, chunk.view)),
            BlockMessage::RbcReady(ready) => Some((ready.height, ready.view)),
            BlockMessage::RbcDeliver(deliver) => Some((deliver.height, deliver.view)),
            BlockMessage::ProposalHint(hint) => Some((hint.height, hint.view)),
            BlockMessage::Proposal(proposal) => {
                Some((proposal.header.height, proposal.header.view))
            }
            BlockMessage::QcVote(vote) => Some((vote.height, vote.view)),
            BlockMessage::Qc(cert) => Some((cert.height, cert.view)),
        }
    }

    fn block_message_kind(msg: &BlockMessage) -> &'static str {
        match msg {
            BlockMessage::BlockCreated(_) => "BlockCreated",
            BlockMessage::BlockSyncUpdate(_) => "BlockSyncUpdate",
            BlockMessage::ConsensusParams(_) => "ConsensusParams",
            BlockMessage::QcVote(vote) => match vote.phase {
                crate::sumeragi::consensus::Phase::Prepare => "PrepareVote",
                crate::sumeragi::consensus::Phase::Commit => "QcVote",
                crate::sumeragi::consensus::Phase::NewView => "NewViewVote",
            },
            BlockMessage::Qc(cert) => match cert.phase {
                crate::sumeragi::consensus::Phase::Prepare => "PrepareCert",
                crate::sumeragi::consensus::Phase::Commit => "CommitCert",
                crate::sumeragi::consensus::Phase::NewView => "NewViewCert",
            },
            BlockMessage::VrfCommit(_) => "VrfCommit",
            BlockMessage::VrfReveal(_) => "VrfReveal",
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

    fn block_time_for_mode(&self, view: &StateView<'_>, mode: ConsensusMode) -> Duration {
        match mode {
            ConsensusMode::Permissioned => view.world.parameters().sumeragi().block_time(),
            ConsensusMode::Npos => super::resolve_npos_block_time(view, &self.config.npos),
        }
    }

    fn commit_timeout_for_mode(&self, view: &StateView<'_>, mode: ConsensusMode) -> Duration {
        match mode {
            ConsensusMode::Permissioned => view.world.parameters().sumeragi().commit_time(),
            ConsensusMode::Npos => super::resolve_npos_timeouts(view, &self.config.npos).commit,
        }
    }

    fn is_observer(&self) -> bool {
        matches!(self.config.role, NodeRole::Observer)
    }

    fn npos_prf_seed(&self) -> Option<[u8; 32]> {
        self.npos_collectors
            .map(|cfg| cfg.seed)
            .or_else(|| self.epoch_manager.as_ref().map(EpochManager::seed))
    }

    fn local_validator_index(&self, view: &StateView<'_>) -> Option<ValidatorIndex> {
        if self.is_observer() {
            return None;
        }
        derive_local_validator_index_for_mode(
            view,
            self.common_config.trusted_peers.value(),
            self.common_config.peer.id(),
            self.consensus_mode,
        )
    }

    fn local_validator_index_for_topology(
        &self,
        topology: &super::network_topology::Topology,
    ) -> Option<ValidatorIndex> {
        if self.is_observer() {
            return None;
        }
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
        let roster_hash = rbc::rbc_roster_hash(&commit_topology);
        let topology = super::network_topology::Topology::new(commit_topology);
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
        let signature_topology = topology_for_view(&topology, key.1, key.2, mode_tag, prf_seed);
        let sender = self.local_validator_index_for_topology(&signature_topology)?;

        let (block_hash, height, view_idx) = key;
        let chunk_root = session
            .expected_chunk_root
            .or_else(|| session.chunk_root())?;
        let mut ready = RbcReady {
            block_hash,
            height,
            view: view_idx,
            epoch: session.epoch,
            roster_hash,
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
        let roster_hash = rbc::rbc_roster_hash(&commit_topology);
        let topology = super::network_topology::Topology::new(commit_topology);
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
        let signature_topology = topology_for_view(&topology, key.1, key.2, mode_tag, prf_seed);
        let sender = self.local_validator_index_for_topology(&signature_topology)?;

        let (block_hash, height, view_idx) = key;
        let chunk_root = session
            .expected_chunk_root
            .or_else(|| session.chunk_root())?;
        let ready_signatures = session
            .ready_signatures
            .iter()
            .map(|entry| RbcReadySignature {
                sender: entry.sender,
                signature: entry.signature.clone(),
            })
            .collect();
        let mut deliver = RbcDeliver {
            block_hash,
            height,
            view: view_idx,
            epoch: session.epoch,
            roster_hash,
            chunk_root,
            sender,
            signature: Vec::new(),
            ready_signatures,
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
            self.config.debug.rbc.force_deliver_quorum_one,
        )
    }

    fn should_drop_future_consensus_message(
        &self,
        height: u64,
        view: u64,
        kind: &'static str,
    ) -> bool {
        let height_window = self.config.gating.future_height_window;
        let view_window = self.config.gating.future_view_window;
        if height_window == 0 && view_window == 0 {
            return false;
        }
        let committed_qc = self.latest_committed_qc();
        let base_height =
            active_round_height(self.highest_qc, committed_qc, self.last_committed_height);
        if height_window != 0 && height > base_height.saturating_add(height_window) {
            debug!(
                height,
                view,
                base_height,
                height_window,
                kind,
                "dropping consensus message beyond future height window"
            );
            return true;
        }
        if view_window == 0 {
            return false;
        }
        if height != base_height {
            return false;
        }
        let Some(base_view) = self.phase_tracker.current_view(height) else {
            return false;
        };
        if matches!(kind, "NewViewVote" | "NewViewCert") {
            // Allow view-change signals to flow so lagging peers can catch up.
            return false;
        }
        if let Some(view_age) = self.phase_tracker.view_age(height, Instant::now()) {
            if view_age >= self.commit_quorum_timeout() {
                return false;
            }
        }
        if view > base_view.saturating_add(view_window) {
            debug!(
                height,
                view,
                base_height,
                base_view,
                view_window,
                kind,
                "dropping consensus message beyond future view window"
            );
            return true;
        }
        false
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
            .rbc
            .pending_max_chunks
            .min(hard_chunk_cap)
            .max(1);
        let chunk_max_bytes = self.config.rbc.chunk_max_bytes.max(1);
        let hard_byte_cap = chunk_max_bytes.saturating_mul(hard_chunk_cap);
        let max_bytes = self
            .config
            .rbc
            .pending_max_bytes
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
        if height <= committed_height {
            let committed_hash = usize::try_from(height)
                .ok()
                .and_then(NonZeroUsize::new)
                .and_then(|nz_height| self.kura.get_block(nz_height))
                .map(|block| block.hash());
            if committed_hash.is_some_and(|hash| hash != *block_hash) {
                return true;
            }
        }
        let present_in_kura = self
            .kura
            .get_block_height_by_hash(*block_hash)
            .and_then(|block_height| {
                let height = u64::try_from(block_height.get()).ok()?;
                Some(height <= committed_height)
            })
            .unwrap_or(false);
        let committed = rbc_message_committed(committed_height, height, present_in_kura);
        if committed
            && self.runtime_da_enabled()
            && !self.block_payload_available_locally(*block_hash)
        {
            return false;
        }
        committed
    }

    fn rebuild_rbc_init(&self, key: super::rbc_store::SessionKey) -> Option<RbcInit> {
        let session = self.subsystems.da_rbc.rbc.sessions.get(&key)?;
        if session.is_invalid() {
            return None;
        }
        let roster = {
            let existing = self.rbc_session_roster(key);
            if existing.is_empty() {
                self.rbc_roster_for_session(key)
            } else {
                existing
            }
        };
        if roster.is_empty() {
            return None;
        }
        let roster_hash = rbc::rbc_roster_hash(&roster);
        let payload_hash = session.payload_hash()?;
        let chunk_digests = session
            .expected_chunk_digests
            .clone()
            .or_else(|| session.all_chunk_digests())?;
        let chunk_root = session
            .expected_chunk_root
            .or_else(|| session.chunk_root())?;
        let block_header = session.block_header?;
        let leader_signature = session.leader_signature.clone()?;
        Some(RbcInit {
            block_hash: key.0,
            height: key.1,
            view: key.2,
            epoch: session.epoch,
            roster,
            roster_hash,
            total_chunks: session.total_chunks(),
            chunk_digests,
            payload_hash,
            chunk_root,
            block_header,
            leader_signature,
        })
    }

    fn rebuild_rbc_init_from_block(
        &self,
        block: &SignedBlock,
        key: super::rbc_store::SessionKey,
    ) -> Option<RbcInit> {
        let roster = self.rbc_roster_for_session(key);
        if roster.is_empty() {
            return None;
        }
        let payload_bytes = self::proposals::block_payload_bytes(block);
        let payload_hash = Hash::new(&payload_bytes);
        let epoch = self.epoch_for_height(key.1);
        let session = match Self::build_rbc_session_from_payload(
            &payload_bytes,
            payload_hash,
            self.config.rbc.chunk_max_bytes,
            epoch,
        ) {
            Ok(session) => session,
            Err(err) => {
                debug!(
                    height = key.1,
                    view = key.2,
                    error = %err,
                    "failed to rebuild RBC INIT from block payload"
                );
                return None;
            }
        };
        let chunk_digests = session.expected_chunk_digests.clone()?;
        let chunk_root = session.expected_chunk_root?;
        let roster_hash = rbc::rbc_roster_hash(&roster);
        let mut topology = super::network_topology::Topology::new(roster.clone());
        let leader_index = match self.leader_index_for(&mut topology, key.1, key.2) {
            Ok(idx) => idx,
            Err(err) => {
                debug!(
                    ?err,
                    height = key.1,
                    view = key.2,
                    "failed to rebuild RBC INIT: leader index unavailable"
                );
                return None;
            }
        };
        let leader_index = u64::try_from(leader_index).ok()?;
        let leader_signature = block
            .signatures()
            .find(|signature| signature.index() == leader_index)?
            .clone();
        let block_header = block.header();
        Some(RbcInit {
            block_hash: key.0,
            height: key.1,
            view: key.2,
            epoch,
            roster,
            roster_hash,
            total_chunks: session.total_chunks(),
            chunk_digests,
            payload_hash,
            chunk_root,
            block_header,
            leader_signature,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn maybe_emit_rbc_ready(&mut self, key: super::rbc_store::SessionKey) -> Result<()> {
        let Some(mut session) = self.subsystems.da_rbc.rbc.sessions.remove(&key) else {
            return Ok(());
        };
        if session.is_invalid() {
            self.subsystems.da_rbc.rbc.ready_deferral.remove(&key);
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            return Ok(());
        }

        let now = Instant::now();
        let mut invalidated = false;
        let result = (|| -> Result<Option<RbcReady>> {
            if session.sent_ready {
                return Ok(None);
            }
            let total_chunks = session.total_chunks();
            let received_chunks = session.received_chunks();
            let ready_count = session.ready_signatures.len();

            let roster_source = self
                .rbc_session_roster_source(key)
                .unwrap_or(RbcRosterSource::Init);
            let allow_unverified = self.allow_unverified_rbc_roster(key);
            let commit_topology = self.ensure_rbc_session_roster(key);
            if commit_topology.is_empty() {
                if self.should_emit_rbc_ready_deferral(
                    key,
                    now,
                    RbcReadyDeferralReason::CommitRosterMissing,
                    ready_count,
                    0,
                    received_chunks,
                    total_chunks,
                    self.rebroadcast_cooldown(),
                ) {
                    info!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        local_peer = %self.common_config.peer.id(),
                        roster_source = ?roster_source,
                        allow_unverified,
                        ready = ready_count,
                        received = received_chunks,
                        total = total_chunks,
                        "deferring local RBC READY: commit roster unavailable"
                    );
                }
                return Ok(None);
            }
            let roster_len = commit_topology.len();
            let local_peer_id = self.common_config.peer.id();
            let local_in_roster = commit_topology.iter().any(|peer| peer == local_peer_id);
            if !roster_source.is_authoritative() && !allow_unverified {
                if self.should_emit_rbc_ready_deferral(
                    key,
                    now,
                    RbcReadyDeferralReason::CommitRosterUnverified,
                    ready_count,
                    0,
                    received_chunks,
                    total_chunks,
                    self.rebroadcast_cooldown(),
                ) {
                    info!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        local_peer = %self.common_config.peer.id(),
                        roster_source = ?roster_source,
                        allow_unverified,
                        roster_len,
                        local_in_roster,
                        ready = ready_count,
                        received = received_chunks,
                        total = total_chunks,
                        "deferring local RBC READY: commit roster unverified"
                    );
                }
                return Ok(None);
            }

            if total_chunks != 0 && received_chunks < total_chunks {
                let topology = super::network_topology::Topology::new(commit_topology);
                // Allow READY after f+1 READYs to unblock quorum even if chunks lag.
                let ready_threshold = topology.min_votes_for_view_change();
                if ready_count < ready_threshold {
                    if self.should_emit_rbc_ready_deferral(
                        key,
                        now,
                        RbcReadyDeferralReason::MissingChunksOrReadyQuorum,
                        ready_count,
                        ready_threshold,
                        received_chunks,
                        total_chunks,
                        self.payload_rebroadcast_cooldown(),
                    ) {
                        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
                        let signature_topology =
                            topology_for_view(&topology, key.1, key.2, mode_tag, prf_seed);
                        let local_ready_sender =
                            self.local_validator_index_for_topology(&signature_topology);
                        let (
                            ready_senders,
                            missing_ready_total,
                            missing_ready,
                            missing_ready_peers,
                        ) = {
                            let ready_senders: BTreeSet<_> = session
                                .ready_signatures
                                .iter()
                                .map(|entry| entry.sender)
                                .collect();
                            let ready_senders_vec =
                                ready_senders.iter().copied().collect::<Vec<_>>();
                            let mut missing_ready_total = 0usize;
                            let mut missing_ready = Vec::new();
                            let mut missing_ready_peers = Vec::new();
                            for (idx, peer) in signature_topology.as_ref().iter().enumerate() {
                                let idx = match ValidatorIndex::try_from(idx) {
                                    Ok(idx) => idx,
                                    Err(_) => continue,
                                };
                                if ready_senders.contains(&idx) {
                                    continue;
                                }
                                missing_ready_total = missing_ready_total.saturating_add(1);
                                if missing_ready.len() < READY_MISSING_LOG_LIMIT {
                                    missing_ready.push(idx);
                                    missing_ready_peers.push(peer.clone());
                                }
                            }
                            (
                                ready_senders_vec,
                                missing_ready_total,
                                missing_ready,
                                missing_ready_peers,
                            )
                        };
                        info!(
                            height = key.1,
                            view = key.2,
                            block = %key.0,
                            local_peer = %self.common_config.peer.id(),
                            roster_source = ?roster_source,
                            allow_unverified,
                            local_in_roster,
                            local_ready_sender = ?local_ready_sender,
                            roster_len,
                            ready = ready_count,
                            required = ready_threshold,
                            ready_senders = ?ready_senders,
                            missing_ready_total,
                            missing_ready = ?missing_ready,
                            missing_ready_peers = ?missing_ready_peers,
                            received = received_chunks,
                            total_chunks,
                            "deferring local RBC READY: awaiting chunks or READY quorum"
                        );
                    }
                    return Ok(None);
                }
            }

            let computed_root = session.chunk_root();
            match (session.expected_chunk_root, computed_root) {
                (Some(expected_root), Some(computed_root)) => {
                    if computed_root != expected_root {
                        session.invalid = true;
                        invalidated = true;
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
                (None, Some(computed_root)) => {
                    session.expected_chunk_root = Some(computed_root);
                }
                (None, None) => {
                    if self.should_emit_rbc_ready_deferral(
                        key,
                        now,
                        RbcReadyDeferralReason::ChunkRootMissing,
                        ready_count,
                        0,
                        received_chunks,
                        total_chunks,
                        self.payload_rebroadcast_cooldown(),
                    ) {
                        info!(
                            height = key.1,
                            view = key.2,
                            block = %key.0,
                            local_peer = %self.common_config.peer.id(),
                            roster_len,
                            ready = ready_count,
                            received = received_chunks,
                            total_chunks,
                            "deferring local RBC READY: chunk root unavailable"
                        );
                    }
                    return Ok(None);
                }
                (Some(_), None) => {}
            }

            if let Some(ready) = self.build_rbc_ready(key, &session) {
                let _ = session.record_ready(ready.sender, ready.signature.clone());
                session.sent_ready = true;
                Ok(Some(ready))
            } else if self.is_observer() {
                info!(
                    height = key.1,
                    view = key.2,
                    block = %key.0,
                    local_peer = %self.common_config.peer.id(),
                    roster_len,
                    "observer node skipping RBC READY"
                );
                session.sent_ready = true;
                Ok(None)
            } else if !local_in_roster {
                if self.should_emit_rbc_ready_deferral(
                    key,
                    now,
                    RbcReadyDeferralReason::LocalNotInCommitTopology,
                    ready_count,
                    0,
                    received_chunks,
                    total_chunks,
                    self.rebroadcast_cooldown(),
                ) {
                    info!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        local_peer = %self.common_config.peer.id(),
                        roster_len,
                        ready = ready_count,
                        received = received_chunks,
                        total = total_chunks,
                        "deferring local RBC READY: local peer missing from commit topology"
                    );
                }
                Ok(None)
            } else {
                warn!(
                    height = key.1,
                    view = key.2,
                    block = %key.0,
                    local_peer = %self.common_config.peer.id(),
                    roster_len,
                    "unable to build RBC READY despite local peer in commit topology"
                );
                Ok(None)
            }
        })();

        let ready_to_send = result?;
        let mismatch_detected = session.invalid && !session.sent_ready;
        let sent_ready = session.sent_ready;
        self.subsystems.da_rbc.rbc.sessions.insert(key, session);
        if let Some(updated) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
            self.update_rbc_status_entry(key, &updated, false);
            self.persist_rbc_session(key, &updated);
        }
        if invalidated {
            self.clear_pending_rbc(&key);
        }
        if sent_ready || mismatch_detected {
            self.subsystems.da_rbc.rbc.ready_deferral.remove(&key);
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
                local_peer = %self.common_config.peer.id(),
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
        roster_hash: Hash,
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
                roster_hash,
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
        if !self.rbc_rebroadcast_active(key) {
            return;
        }
        let roster = self.ensure_rbc_session_roster(key);
        if roster.is_empty() {
            return;
        }
        let roster_hash = rbc::rbc_roster_hash(&roster);
        let Some(readies) = Self::rbc_ready_bundle(key, session, roster_hash) else {
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
        let expected_hash = readies.first().map(|ready| ready.roster_hash);
        let topology_peers = self.ensure_rbc_session_roster(key);
        let local_peer_id = self.common_config.peer.id().clone();
        if topology_peers.is_empty() {
            return;
        }
        if let Some(expected_hash) = expected_hash {
            let computed_hash = rbc::rbc_roster_hash(&topology_peers);
            if computed_hash != expected_hash {
                warn!(
                    ?key,
                    ?expected_hash,
                    ?computed_hash,
                    "skipping RBC READY rebroadcast: roster hash mismatch"
                );
                return;
            }
        }
        if !self.should_rebroadcast_rbc_ready(&topology_peers, key) {
            let topology = super::network_topology::Topology::new(topology_peers.clone());
            let required = self.rbc_deliver_quorum(&topology);
            let ready_quorum = required != 0 && readies.len() >= required;
            if ready_quorum {
                return;
            }
            let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
            let signature_topology = topology_for_view(&topology, key.1, key.2, mode_tag, prf_seed);
            let local_idx = self.local_validator_index_for_topology(&signature_topology);
            let local_ready =
                local_idx.is_some_and(|idx| readies.iter().any(|ready| ready.sender == idx));
            // Ensure local READY evidence eventually propagates even if initial sends were lost.
            if !local_ready {
                return;
            }
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
        roster: &[PeerId],
    ) -> Option<(
        crate::sumeragi::consensus::RbcInit,
        Vec<crate::sumeragi::consensus::RbcChunk>,
    )> {
        if session.total_chunks() == 0 {
            return None;
        }
        if roster.is_empty() {
            return None;
        }
        let payload_hash = session.payload_hash()?;
        let chunk_digests = session
            .expected_chunk_digests
            .clone()
            .or_else(|| session.all_chunk_digests())?;
        let chunk_root = session
            .expected_chunk_root
            .or_else(|| session.chunk_root())?;
        let block_header = session.block_header?;
        let leader_signature = session.leader_signature.clone()?;
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
        let roster_hash = rbc::rbc_roster_hash(roster);
        let init = crate::sumeragi::consensus::RbcInit {
            block_hash: key.0,
            height: key.1,
            view: key.2,
            epoch: session.epoch,
            roster: roster.to_vec(),
            roster_hash,
            total_chunks: session.total_chunks(),
            chunk_digests,
            payload_hash,
            chunk_root,
            block_header,
            leader_signature,
        };
        Some((init, chunks))
    }

    fn should_rebroadcast_rbc_payload(
        &self,
        roster: &[PeerId],
        key: super::rbc_store::SessionKey,
    ) -> bool {
        if roster.is_empty() {
            return false;
        }
        let leader_override = self.local_is_rbc_payload_leader(roster, key);
        let seed = rbc::shuffle_seed(&key.0, key.1, key.2);
        // Limit payload rebroadcasts to a deterministic f+1 subset to avoid message storms.
        let rebroadcaster =
            rbc::is_payload_rebroadcaster(roster, self.common_config.peer.id(), seed);
        let should_rebroadcast = rebroadcaster || leader_override;
        if !should_rebroadcast {
            #[cfg(feature = "telemetry")]
            self.telemetry.inc_rbc_rebroadcast_skipped("payload");
        }
        should_rebroadcast
    }

    fn local_is_rbc_payload_leader(
        &self,
        roster: &[PeerId],
        key: super::rbc_store::SessionKey,
    ) -> bool {
        if roster.is_empty() {
            return false;
        }
        let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
        let topology = super::network_topology::Topology::new(roster.to_vec());
        let rotated = topology_for_view(&topology, key.1, key.2, mode_tag, prf_seed);
        rotated
            .as_ref()
            .first()
            .is_some_and(|peer| peer == self.common_config.peer.id())
    }

    fn should_rebroadcast_rbc_ready(
        &self,
        roster: &[PeerId],
        key: super::rbc_store::SessionKey,
    ) -> bool {
        if roster.is_empty() {
            return false;
        }
        let seed = rbc::shuffle_seed(&key.0, key.1, key.2);
        let rebroadcaster = rbc::is_ready_rebroadcaster(roster, self.common_config.peer.id(), seed);
        if !rebroadcaster {
            #[cfg(feature = "telemetry")]
            self.telemetry.inc_rbc_rebroadcast_skipped("ready");
        }
        rebroadcaster
    }

    fn enqueue_rbc_payload_chunks(
        &mut self,
        key: super::rbc_store::SessionKey,
        chunks: Vec<crate::sumeragi::consensus::RbcChunk>,
        roster: &[PeerId],
    ) -> bool {
        if chunks.is_empty() || roster.is_empty() {
            return false;
        }
        let target_count = rbc::rbc_chunk_target_count(roster.len(), self.config.rbc.chunk_fanout);
        if target_count == 0 {
            return false;
        }
        let seed = rbc::shuffle_seed(&key.0, key.1, key.2);
        let local_peer_id = self.common_config.peer.id().clone();
        let targets = rbc::select_rbc_chunk_targets(roster, &local_peer_id, seed, target_count);
        if targets.is_empty() {
            return false;
        }
        if let Some(existing) = self.subsystems.da_rbc.rbc.outbound_chunks.get(&key) {
            if existing.cursor < existing.chunks.len() {
                trace!(
                    height = key.1,
                    view = key.2,
                    "skipping RBC chunk queue: broadcast already in progress"
                );
                return false;
            }
        }
        self.subsystems.da_rbc.rbc.outbound_chunks.insert(
            key,
            RbcOutboundChunks {
                chunks,
                cursor: 0,
                targets,
            },
        );
        true
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
        let topology_peers = self.ensure_rbc_session_roster(key);
        if topology_peers.is_empty() {
            return;
        }
        let chunk_count = chunks.len();
        let queued = self.enqueue_rbc_payload_chunks(key, chunks, &topology_peers);
        info!(
            height = key.1,
            view = key.2,
            block = %key.0,
            ready = ready_count,
            chunk_count,
            queued,
            "queued RBC INIT and chunk rebroadcast while awaiting READY quorum"
        );
        let local_peer_id = self.common_config.peer.id().clone();
        for peer in &topology_peers {
            if peer == &local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessage::RbcInit(init.clone()),
            });
        }
        self.subsystems
            .da_rbc
            .rbc
            .payload_rebroadcast_last_sent
            .insert(key, now);
    }

    fn rebroadcast_rbc_payload(&mut self, key: super::rbc_store::SessionKey, session: &RbcSession) {
        if !self.rbc_rebroadcast_active(key) {
            return;
        }
        let now = Instant::now();
        let cooldown = self.payload_rebroadcast_cooldown();
        let queue_drop = self.queue_drop_backpressure_active(now, cooldown);
        let queue_block = self.queue_block_backpressure_active(now, cooldown);
        if queue_drop || queue_block {
            let queue_depths = super::status::worker_queue_depth_snapshot();
            trace!(
                height = key.1,
                view = key.2,
                block = %key.0,
                block_payload_rx_depth = queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                queue_drop,
                queue_block,
                "skipping RBC payload rebroadcast due to consensus queue backpressure"
            );
            return;
        }
        let roster = self.rbc_session_roster(key);
        if roster.is_empty() {
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                "skipping RBC payload rebroadcast: roster missing"
            );
            return;
        }
        if !self.should_rebroadcast_rbc_payload(&roster, key) {
            return;
        }
        let Some((init, chunks)) = Self::rbc_payload_bundle(key, session, &roster) else {
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

    fn rebroadcast_rbc_payload_for_missing_init(
        &mut self,
        key: super::rbc_store::SessionKey,
        session: &RbcSession,
    ) {
        if self.is_observer() || !self.runtime_da_enabled() {
            return;
        }
        if !self.rbc_rebroadcast_active(key) {
            return;
        }
        let now = Instant::now();
        let cooldown = self.payload_rebroadcast_cooldown();
        let queue_drop = self.queue_drop_backpressure_active(now, cooldown);
        let queue_block = self.queue_block_backpressure_active(now, cooldown);
        if queue_drop || queue_block {
            let queue_depths = super::status::worker_queue_depth_snapshot();
            trace!(
                height = key.1,
                view = key.2,
                block = %key.0,
                block_payload_rx_depth = queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                queue_drop,
                queue_block,
                "skipping missing-INIT RBC rebroadcast due to consensus queue backpressure"
            );
            return;
        }
        let roster = self.rbc_session_roster(key);
        if roster.is_empty() {
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                "skipping missing-INIT RBC rebroadcast: roster missing"
            );
            return;
        }
        let Some((init, chunks)) = Self::rbc_payload_bundle(key, session, &roster) else {
            debug!(
                height = key.1,
                view = key.2,
                block = %key.0,
                "skipping missing-INIT RBC rebroadcast: payload unavailable"
            );
            return;
        };
        debug!(
            height = key.1,
            view = key.2,
            block = %key.0,
            "rebroadcasting RBC INIT after reconstructing missing INIT"
        );
        self.rebroadcast_rbc_payload_bundle(key, init, chunks, session.ready_signatures.len());
    }

    fn rbc_payload_rebroadcast_due(
        &self,
        key: &super::rbc_store::SessionKey,
        now: Instant,
        cooldown: Duration,
    ) -> bool {
        self.subsystems
            .da_rbc
            .rbc
            .payload_rebroadcast_last_sent
            .get(key)
            .is_none_or(|last| now.saturating_duration_since(*last) >= cooldown)
    }

    fn rbc_ready_rebroadcast_due(
        &self,
        key: &super::rbc_store::SessionKey,
        now: Instant,
        cooldown: Duration,
    ) -> bool {
        self.subsystems
            .da_rbc
            .rbc
            .ready_rebroadcast_last_sent
            .get(key)
            .is_none_or(|last| now.saturating_duration_since(*last) >= cooldown)
    }

    fn flush_pending_rbc_if_roster_ready(&mut self, key: super::rbc_store::SessionKey) -> bool {
        if !self.subsystems.da_rbc.rbc.pending.contains_key(&key) {
            return false;
        }
        let mut roster = self.rbc_session_roster(key);
        if roster.is_empty() {
            roster = self.ensure_rbc_session_roster(key);
        }
        if roster.is_empty() {
            return false;
        }
        if let Err(err) = self.flush_pending_rbc(key) {
            warn!(?err, ?key, "failed to flush pending RBC messages");
            return false;
        }
        true
    }

    #[allow(clippy::too_many_lines)]
    fn rebroadcast_stalled_rbc_payloads(&mut self, now: Instant) -> bool {
        if self.is_observer() {
            return false;
        }
        if !self.runtime_da_enabled() {
            return false;
        }
        if self.subsystems.da_rbc.rbc.sessions.is_empty() {
            self.subsystems.da_rbc.rbc.rebroadcast_cursor = None;
            return false;
        }

        let payload_cooldown = self.payload_rebroadcast_cooldown();
        if self.relay_backpressure_active(now, payload_cooldown) {
            trace!("skipping RBC payload rebroadcast due to relay backpressure");
            return false;
        }
        let queue_drop = self.queue_drop_backpressure_active(now, payload_cooldown);
        let queue_block = self.queue_block_backpressure_active(now, payload_cooldown);
        let payload_backpressure = queue_drop || queue_block;
        if payload_backpressure {
            let queue_depths = super::status::worker_queue_depth_snapshot();
            trace!(
                block_payload_rx_depth = queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                queue_drop,
                queue_block,
                "suspending RBC payload rebroadcast due to consensus queue backpressure"
            );
        }
        let ready_cooldown = self.rebroadcast_cooldown();
        let view = self.state.view();
        let tip_height = view.height();
        let tip_hash = view.latest_block_hash();
        drop(view);
        // Rotate through sessions with a per-tick budget so large backlogs do not trigger storms.
        let total_sessions = self.subsystems.da_rbc.rbc.sessions.len();
        let session_budget = self.config.rbc.rebroadcast_sessions_per_tick.max(1);
        let limit = session_budget.min(total_sessions);
        let cursor = self.subsystems.da_rbc.rbc.rebroadcast_cursor;
        let mut keys = Vec::with_capacity(limit);
        let mut last_key = cursor;
        if let Some(cursor_key) = cursor {
            for (key, _) in self
                .subsystems
                .da_rbc
                .rbc
                .sessions
                .range((Excluded(cursor_key), Unbounded))
            {
                last_key = Some(*key);
                if self.rbc_rebroadcast_active_with_tip(*key, tip_height, tip_hash) {
                    keys.push(*key);
                    if keys.len() >= limit {
                        break;
                    }
                }
            }
            if keys.len() < limit {
                for (key, _) in self.subsystems.da_rbc.rbc.sessions.range(..=cursor_key) {
                    last_key = Some(*key);
                    if self.rbc_rebroadcast_active_with_tip(*key, tip_height, tip_hash) {
                        keys.push(*key);
                        if keys.len() >= limit {
                            break;
                        }
                    }
                }
            }
        } else {
            for key in self.subsystems.da_rbc.rbc.sessions.keys() {
                last_key = Some(*key);
                if self.rbc_rebroadcast_active_with_tip(*key, tip_height, tip_hash) {
                    keys.push(*key);
                    if keys.len() >= limit {
                        break;
                    }
                }
            }
        }
        let mut progress = false;

        for key in keys {
            if self.flush_pending_rbc_if_roster_ready(key) {
                progress = true;
            }
            let attempt_ready =
                self.subsystems
                    .da_rbc
                    .rbc
                    .sessions
                    .get(&key)
                    .is_some_and(|session| {
                        !session.sent_ready && !session.is_invalid() && !session.delivered
                    });
            if attempt_ready {
                let was_sent = self
                    .subsystems
                    .da_rbc
                    .rbc
                    .sessions
                    .get(&key)
                    .is_none_or(|session| session.sent_ready);
                if let Err(err) = self.maybe_emit_rbc_ready(key) {
                    debug!(
                        height = key.1,
                        view = key.2,
                        ?err,
                        "failed to re-attempt RBC READY emission"
                    );
                }
                let now_sent = self
                    .subsystems
                    .da_rbc
                    .rbc
                    .sessions
                    .get(&key)
                    .map_or(was_sent, |session| session.sent_ready);
                if !was_sent && now_sent {
                    progress = true;
                }
            }
            if !self.subsystems.da_rbc.rbc.sessions.contains_key(&key) {
                continue;
            }
            let roster = self.rbc_session_roster(key);
            let roster_source = self
                .rbc_session_roster_source(key)
                .unwrap_or(RbcRosterSource::Init);
            if roster.is_empty() || !roster_source.is_authoritative() {
                continue;
            }
            let Some(session) = self.subsystems.da_rbc.rbc.sessions.get(&key) else {
                continue;
            };
            if session.delivered || session.is_invalid() {
                continue;
            }
            let roster_hash = rbc::rbc_roster_hash(&roster);
            let topology = super::network_topology::Topology::new(roster.clone());
            let required = self.rbc_deliver_quorum(&topology);
            let ready_count = session.ready_signatures.len();
            let total_chunks = session.total_chunks();
            let missing_chunks = total_chunks != 0 && session.received_chunks() < total_chunks;
            let ready_quorum = ready_count >= required;
            if ready_quorum && !missing_chunks {
                continue;
            }
            let should_rebroadcast_payload = missing_chunks || !ready_quorum;
            let payload_bundle = if should_rebroadcast_payload
                && !payload_backpressure
                && self.should_rebroadcast_rbc_payload(&roster, key)
                && self.rbc_payload_rebroadcast_due(&key, now, payload_cooldown)
            {
                Self::rbc_payload_bundle(key, session, &roster)
            } else {
                None
            };
            let ready_bundle = if !ready_quorum
                && !session.ready_signatures.is_empty()
                && self.rbc_ready_rebroadcast_due(&key, now, ready_cooldown)
            {
                Self::rbc_ready_bundle(key, session, roster_hash)
            } else {
                None
            };

            if let Some((init, chunks)) = payload_bundle {
                self.rebroadcast_rbc_payload_bundle(key, init, chunks, ready_count);
                progress = true;
            }
            if let Some(readies) = ready_bundle {
                self.rebroadcast_rbc_ready_bundle(key, readies);
                progress = true;
            }
        }

        self.subsystems.da_rbc.rbc.rebroadcast_cursor = last_key;
        progress
    }

    fn flush_rbc_outbound_chunks(&mut self, now: Instant) -> bool {
        if self.is_observer() {
            return false;
        }
        if !self.runtime_da_enabled() {
            return false;
        }
        if self.subsystems.da_rbc.rbc.outbound_chunks.is_empty() {
            self.subsystems.da_rbc.rbc.outbound_cursor = None;
            return false;
        }

        let cooldown = self.payload_rebroadcast_cooldown();
        if self.relay_backpressure_active(now, cooldown) {
            trace!("skipping RBC outbound chunk flush due to relay backpressure");
            return false;
        }
        let queue_drop = self.queue_drop_backpressure_active(now, cooldown);
        let queue_block = self.queue_block_backpressure_active(now, cooldown);
        if queue_drop || queue_block {
            let queue_depths = super::status::worker_queue_depth_snapshot();
            trace!(
                block_payload_rx_depth = queue_depths.block_payload_rx,
                rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                queue_drop,
                queue_block,
                "skipping RBC outbound chunk flush due to consensus queue backpressure"
            );
            return false;
        }

        let mut remaining = self.config.rbc.payload_chunks_per_tick.max(1);
        let total_sessions = self.subsystems.da_rbc.rbc.outbound_chunks.len();
        let cursor = self.subsystems.da_rbc.rbc.outbound_cursor;
        let mut keys = Vec::with_capacity(total_sessions);
        if let Some(cursor_key) = cursor {
            for (key, _) in self
                .subsystems
                .da_rbc
                .rbc
                .outbound_chunks
                .range((Excluded(cursor_key), Unbounded))
            {
                keys.push(*key);
            }
            if keys.len() < total_sessions {
                for (key, _) in self
                    .subsystems
                    .da_rbc
                    .rbc
                    .outbound_chunks
                    .range(..=cursor_key)
                {
                    keys.push(*key);
                }
            }
        } else {
            keys.extend(self.subsystems.da_rbc.rbc.outbound_chunks.keys().copied());
        }

        let mut last_key = cursor;
        let mut to_remove = Vec::new();
        let mut sent_any = false;

        for key in keys {
            if remaining == 0 {
                break;
            }
            let (targets, chunks_to_send, exhausted, to_send) = {
                let Some(entry) = self.subsystems.da_rbc.rbc.outbound_chunks.get_mut(&key) else {
                    continue;
                };
                let available = entry.chunks.len().saturating_sub(entry.cursor);
                if available == 0 {
                    to_remove.push(key);
                    continue;
                }
                let to_send = available.min(remaining);
                let end = entry.cursor.saturating_add(to_send);
                let targets = entry.targets.clone();
                let chunks_to_send: Vec<_> = entry.chunks[entry.cursor..end].to_vec();
                entry.cursor = end;
                let exhausted = entry.cursor >= entry.chunks.len();
                (targets, chunks_to_send, exhausted, to_send)
            };
            remaining = remaining.saturating_sub(to_send);
            last_key = Some(key);
            sent_any = true;
            if exhausted {
                to_remove.push(key);
            }
            for chunk in &chunks_to_send {
                self.schedule_rbc_chunk_posts(chunk, &targets);
            }
        }

        for key in to_remove {
            self.subsystems.da_rbc.rbc.outbound_chunks.remove(&key);
        }

        self.subsystems.da_rbc.rbc.outbound_cursor = last_key;
        sent_any
    }

    fn should_emit_rbc_deliver_deferral(
        &mut self,
        key: super::rbc_store::SessionKey,
        now: Instant,
        ready_count: usize,
        received_chunks: u32,
        total_chunks: u32,
        cooldown: Duration,
    ) -> bool {
        let entry = self.subsystems.da_rbc.rbc.deliver_deferral.entry(key);
        match entry {
            Entry::Vacant(slot) => {
                slot.insert(RbcDeliverDeferral {
                    last_attempt: now,
                    ready_count,
                    received_chunks,
                    total_chunks,
                });
                true
            }
            Entry::Occupied(mut slot) => {
                let last = *slot.get();
                let progressed = ready_count > last.ready_count
                    || received_chunks > last.received_chunks
                    || total_chunks != last.total_chunks;
                let elapsed = now.saturating_duration_since(last.last_attempt);
                if progressed || elapsed >= cooldown {
                    slot.insert(RbcDeliverDeferral {
                        last_attempt: now,
                        ready_count,
                        received_chunks,
                        total_chunks,
                    });
                    true
                } else {
                    false
                }
            }
        }
    }

    fn should_emit_rbc_ready_deferral(
        &mut self,
        key: super::rbc_store::SessionKey,
        now: Instant,
        reason: RbcReadyDeferralReason,
        ready_count: usize,
        required_ready: usize,
        received_chunks: u32,
        total_chunks: u32,
        cooldown: Duration,
    ) -> bool {
        let entry = self.subsystems.da_rbc.rbc.ready_deferral.entry(key);
        match entry {
            Entry::Vacant(slot) => {
                slot.insert(RbcReadyDeferral {
                    last_attempt: now,
                    reason,
                    ready_count,
                    required_ready,
                    received_chunks,
                    total_chunks,
                });
                true
            }
            Entry::Occupied(mut slot) => {
                let last = *slot.get();
                let progressed = reason != last.reason
                    || ready_count > last.ready_count
                    || required_ready != last.required_ready
                    || received_chunks > last.received_chunks
                    || total_chunks != last.total_chunks;
                let elapsed = now.saturating_duration_since(last.last_attempt);
                if progressed || elapsed >= cooldown {
                    slot.insert(RbcReadyDeferral {
                        last_attempt: now,
                        reason,
                        ready_count,
                        required_ready,
                        received_chunks,
                        total_chunks,
                    });
                    true
                } else {
                    false
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn maybe_emit_rbc_deliver(&mut self, key: super::rbc_store::SessionKey) -> Result<()> {
        let Some(mut session) = self.subsystems.da_rbc.rbc.sessions.remove(&key) else {
            return Ok(());
        };
        if session.delivered || session.is_invalid() {
            self.subsystems.da_rbc.rbc.ready_deferral.remove(&key);
            self.subsystems.da_rbc.rbc.deliver_deferral.remove(&key);
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            return Ok(());
        }

        let ready_count = session.ready_signatures.len();
        let received_chunks = session.received_chunks();
        let total_chunks = session.total_chunks();
        let commit_topology = self.ensure_rbc_session_roster(key);
        if commit_topology.is_empty() {
            if self.should_emit_rbc_deliver_deferral(
                key,
                Instant::now(),
                ready_count,
                received_chunks,
                total_chunks,
                self.rebroadcast_cooldown(),
            ) {
                info!(
                    height = key.1,
                    view = key.2,
                    block = %key.0,
                    local_peer = %self.common_config.peer.id(),
                    ready = ready_count,
                    received = received_chunks,
                    total = total_chunks,
                    "deferring RBC DELIVER: commit roster unavailable"
                );
            }
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            return Ok(());
        }
        let roster_len = commit_topology.len();
        let roster_source = self
            .rbc_session_roster_source(key)
            .unwrap_or(RbcRosterSource::Init);
        let allow_unverified = self.allow_unverified_rbc_roster(key);
        if !roster_source.is_authoritative() && !allow_unverified {
            if self.should_emit_rbc_deliver_deferral(
                key,
                Instant::now(),
                ready_count,
                received_chunks,
                total_chunks,
                self.rebroadcast_cooldown(),
            ) {
                info!(
                    height = key.1,
                    view = key.2,
                    block = %key.0,
                    local_peer = %self.common_config.peer.id(),
                    roster_source = ?roster_source,
                    allow_unverified,
                    roster_len,
                    ready = ready_count,
                    received = received_chunks,
                    total = total_chunks,
                    "deferring RBC DELIVER: commit roster unverified"
                );
            }
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            return Ok(());
        }
        let topology = super::network_topology::Topology::new(commit_topology.clone());
        let required = self.rbc_deliver_quorum(&topology);
        let missing_chunks = total_chunks != 0 && received_chunks < total_chunks;
        if ready_count < required {
            let cooldown = if missing_chunks {
                self.payload_rebroadcast_cooldown()
            } else {
                self.rebroadcast_cooldown()
            };
            if !self.should_emit_rbc_deliver_deferral(
                key,
                Instant::now(),
                ready_count,
                received_chunks,
                total_chunks,
                cooldown,
            ) {
                self.subsystems.da_rbc.rbc.sessions.insert(key, session);
                return Ok(());
            }
            let (_, mode_tag, prf_seed) = self.consensus_context_for_height(key.1);
            let signature_topology = topology_for_view(&topology, key.1, key.2, mode_tag, prf_seed);
            let (missing_ready_total, missing_ready, missing_ready_peers) = {
                let ready_senders: BTreeSet<_> = session
                    .ready_signatures
                    .iter()
                    .map(|entry| entry.sender)
                    .collect();
                let mut missing_ready_total = 0usize;
                let mut missing_ready = Vec::new();
                let mut missing_ready_peers = Vec::new();
                for (idx, peer) in signature_topology.as_ref().iter().enumerate() {
                    let idx = match ValidatorIndex::try_from(idx) {
                        Ok(idx) => idx,
                        Err(_) => continue,
                    };
                    if ready_senders.contains(&idx) {
                        continue;
                    }
                    missing_ready_total = missing_ready_total.saturating_add(1);
                    if missing_ready.len() < READY_MISSING_LOG_LIMIT {
                        missing_ready.push(idx);
                        missing_ready_peers.push(peer.clone());
                    }
                }
                (missing_ready_total, missing_ready, missing_ready_peers)
            };
            let (pending_ready_total, pending_ready, pending_ready_peers) = self
                .subsystems
                .da_rbc
                .rbc
                .pending
                .get(&key)
                .map(|pending| {
                    let senders: BTreeSet<_> =
                        pending.ready.iter().map(|ready| ready.sender).collect();
                    let pending_ready_total = senders.len();
                    let mut pending_ready = Vec::new();
                    let mut pending_ready_peers = Vec::new();
                    for sender in senders.iter().take(READY_MISSING_LOG_LIMIT).copied() {
                        pending_ready.push(sender);
                        if let Ok(idx) = usize::try_from(sender) {
                            if let Some(peer) = signature_topology.as_ref().get(idx) {
                                pending_ready_peers.push(peer.clone());
                            }
                        }
                    }
                    (pending_ready_total, pending_ready, pending_ready_peers)
                })
                .unwrap_or_else(|| (0, Vec::new(), Vec::new()));
            iroha_logger::info!(
                height = key.1,
                view = key.2,
                block = %key.0,
                local_peer = %self.common_config.peer.id(),
                roster_source = ?roster_source,
                roster_len,
                ready = ready_count,
                required,
                missing_ready_total,
                missing_ready = ?missing_ready,
                missing_ready_peers = ?missing_ready_peers,
                pending_ready_total,
                pending_ready = ?pending_ready,
                pending_ready_peers = ?pending_ready_peers,
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
        if missing_chunks {
            if !self.should_emit_rbc_deliver_deferral(
                key,
                Instant::now(),
                ready_count,
                received_chunks,
                total_chunks,
                self.payload_rebroadcast_cooldown(),
            ) {
                self.subsystems.da_rbc.rbc.sessions.insert(key, session);
                return Ok(());
            }
            info!(
                height = key.1,
                view = key.2,
                block = %key.0,
                local_peer = %self.common_config.peer.id(),
                roster_len,
                received = received_chunks,
                total = total_chunks,
                "deferring RBC DELIVER: payload chunks not yet complete"
            );
            self.rebroadcast_rbc_payload(key, &session);
            self.subsystems.da_rbc.rbc.sessions.insert(key, session);
            return Ok(());
        }
        if let Some(computed_root) = session.chunk_root() {
            if let Some(expected_root) = session.expected_chunk_root {
                if expected_root != computed_root {
                    session.invalid = true;
                    warn!(
                        height = key.1,
                        view = key.2,
                        block = %key.0,
                        ?expected_root,
                        ?computed_root,
                        "RBC chunk-root mismatch detected; refusing to emit DELIVER"
                    );
                    self.subsystems.da_rbc.rbc.sessions.insert(key, session);
                    self.subsystems.da_rbc.rbc.deliver_deferral.remove(&key);
                    self.clear_pending_rbc(&key);
                    if let Some(updated) = self.subsystems.da_rbc.rbc.sessions.get(&key).cloned() {
                        self.update_rbc_status_entry(key, &updated, false);
                        self.persist_rbc_session(key, &updated);
                    }
                    self.publish_rbc_backlog_snapshot();
                    return Ok(());
                }
            } else {
                session.expected_chunk_root = Some(computed_root);
            }
        }

        let deliver = if let Some(deliver) = self.build_rbc_deliver(key, &session) {
            deliver
        } else {
            debug!(
                height = key.1,
                view = key.2,
                ready = ready_count,
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
        self.subsystems.da_rbc.rbc.deliver_deferral.remove(&key);
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
            local_peer = %self.common_config.peer.id(),
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

        self.recover_block_from_rbc_session(key);
        self.process_commit_candidates();

        Ok(())
    }

    fn commit_quorum_timeout(&self) -> Duration {
        let view = self.state.view();
        let params = view.world.parameters().sumeragi();
        let block_time = self.block_time_for_mode(&view, self.consensus_mode);
        let commit_time = self.commit_timeout_for_mode(&view, self.consensus_mode);
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
        let block_time = self.block_time_for_mode(&view, self.consensus_mode);
        drop(view);
        rebroadcast_cooldown_from_block_time(block_time)
    }

    fn payload_rebroadcast_cooldown(&self) -> Duration {
        let view = self.state.view();
        let block_time = self.block_time_for_mode(&view, self.consensus_mode);
        drop(view);
        payload_rebroadcast_cooldown_from_block_time(block_time)
    }

    fn relay_backpressure_active(&mut self, now: Instant, cooldown: Duration) -> bool {
        self.relay_backpressure.active(now, cooldown)
    }

    fn queue_drop_backpressure_active(&mut self, now: Instant, cooldown: Duration) -> bool {
        self.queue_drop_backpressure.active(now, cooldown)
    }

    fn queue_block_backpressure_active(&mut self, now: Instant, cooldown: Duration) -> bool {
        self.queue_block_backpressure.active(now, cooldown)
    }

    fn quorum_timeout(&self, da_enabled: bool) -> Duration {
        let view = self.state.view();
        let block_time = self.block_time_for_mode(&view, self.consensus_mode);
        let commit_time = self.commit_timeout_for_mode(&view, self.consensus_mode);
        drop(view);

        commit_quorum_timeout_from_durations(
            block_time,
            commit_time,
            da_enabled,
            self.da_quorum_timeout_multiplier(),
        )
    }

    fn da_quorum_timeout_multiplier(&self) -> u32 {
        self.config.da.quorum_timeout_multiplier.max(1)
    }

    fn availability_timeout(&self, quorum_timeout: Duration, da_enabled: bool) -> Duration {
        availability_timeout_from_quorum(
            quorum_timeout,
            da_enabled,
            self.config.da.availability_timeout_multiplier.max(1),
            self.config.da.availability_timeout_floor,
        )
    }

    fn backpressure_override_due(&self, now: Instant) -> bool {
        if self.queue.active_len() == 0 {
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
        let Some(age) = self.phase_tracker.view_age(height, now) else {
            return false;
        };
        let current_view = self.phase_tracker.current_view(height).unwrap_or(0);
        let proposal_seen = self
            .subsystems
            .propose
            .proposals_seen
            .contains(&(height, current_view));
        let timeout = idle_view_timeout(
            proposal_seen,
            self.commit_quorum_timeout(),
            self.subsystems.propose.pacemaker.propose_interval,
            self.runtime_da_enabled(),
        );
        age >= timeout
    }

    fn force_view_change_if_idle(&mut self, now: Instant) -> bool {
        if self.has_active_pending_blocks() {
            return false;
        }
        if self.subsystems.commit.inflight.is_some() {
            return false;
        }
        if !self.pending.missing_block_requests.is_empty() {
            // Avoid view-change churn while we're still synchronizing missing payloads.
            self.queue_ready_since = None;
            return false;
        }
        if self.queue.active_len() == 0 {
            // Skip idle view-change churn when no work is queued.
            self.queue_ready_since = None;
            return false;
        }
        let rbc_backlog = self.has_unresolved_rbc_backlog();
        let relay_backpressure = self.relay_backpressure_active(now, self.rebroadcast_cooldown());
        let queue_depths = super::status::worker_queue_depth_snapshot();
        let block_payload_cap =
            u64::try_from(self.config.queues.block_payload.max(1)).unwrap_or(u64::MAX);
        let rbc_chunk_cap = u64::try_from(self.config.queues.rbc_chunks.max(1)).unwrap_or(u64::MAX);
        let consensus_queue_backpressure = queue_depths.block_payload_rx >= block_payload_cap
            || queue_depths.rbc_chunk_rx >= rbc_chunk_cap;
        let consensus_queue_backlog = queue_depths.vote_rx > 0
            || queue_depths.block_payload_rx > 0
            || queue_depths.rbc_chunk_rx > 0
            || queue_depths.block_rx > 0;

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

        let view_age = if let Some(age) = self.phase_tracker.view_age(height, now) {
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
        let queue_since = match self.queue_ready_since {
            Some(entry) if entry.height == height && entry.view == current_view => {
                Some(entry.since)
            }
            _ => {
                self.queue_ready_since = Some(QueueReadySince {
                    height,
                    view: current_view,
                    since: now,
                });
                None
            }
        };
        let queue_since = match queue_since {
            Some(since) => Some(since),
            None => {
                if !proposal_seen {
                    return false;
                }
                Some(now)
            }
        };
        if !proposal_seen {
            // Avoid rotating the leader before the pacemaker has attempted a proposal.
            let last_attempt = self.subsystems.propose.last_pacemaker_attempt;
            let attempted = queue_since
                .and_then(|since| last_attempt.map(|attempt| attempt >= since))
                .unwrap_or(false);
            if !attempted {
                return false;
            }
        }
        let timeout = idle_view_timeout(
            proposal_seen,
            self.commit_quorum_timeout(),
            self.subsystems.propose.pacemaker.propose_interval,
            self.runtime_da_enabled(),
        );
        let age = if proposal_seen {
            view_age
        } else {
            now.saturating_duration_since(queue_since.unwrap_or(now))
        };
        let timed_out = idle_round_timed_out(true, age, timeout);
        if rbc_backlog
            || relay_backpressure
            || consensus_queue_backpressure
            || consensus_queue_backlog
        {
            if timed_out {
                debug!(
                    rbc_backlog,
                    relay_backpressure,
                    consensus_queue_backpressure,
                    consensus_queue_backlog,
                    vote_rx_depth = queue_depths.vote_rx,
                    block_payload_rx_depth = queue_depths.block_payload_rx,
                    rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
                    block_rx_depth = queue_depths.block_rx,
                    "skipping idle view-change despite timeout due to unresolved backpressure"
                );
            } else {
                if rbc_backlog {
                    trace!("skipping idle view-change due to unresolved RBC backlog");
                }
                if relay_backpressure {
                    trace!("skipping idle view-change due to relay backpressure");
                }
                if consensus_queue_backpressure {
                    trace!("skipping idle view-change due to consensus queue backpressure");
                }
                if consensus_queue_backlog {
                    trace!("skipping idle view-change due to consensus queue backlog");
                }
            }
            return false;
        }
        if !timed_out {
            return false;
        }

        let next_view = current_view.saturating_add(1);
        if !proposal_seen {
            if let Some(telemetry) = self.telemetry_handle() {
                telemetry.inc_proposal_gap();
            }
        }
        warn!(
            height,
            view = current_view,
            next_view,
            age_ms = age.as_millis(),
            timeout_ms = timeout.as_millis(),
            proposal_seen,
            rbc_backlog,
            relay_backpressure,
            consensus_queue_backpressure,
            consensus_queue_backlog,
            vote_rx_depth = queue_depths.vote_rx,
            block_payload_rx_depth = queue_depths.block_payload_rx,
            rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
            block_rx_depth = queue_depths.block_rx,
            "no proposal observed before cutoff; rotating leader via view change"
        );
        self.trigger_view_change_with_cause(height, current_view, ViewChangeCause::MissingQc);
        true
    }

    fn prune_stale_view_state(&mut self, height: u64, min_view: u64) {
        let da_enabled = self.runtime_da_enabled();
        // Keep DA availability state across view changes until payloads are durably resolved.
        // In DA mode, retain stale pending payloads (as aborted) so block sync can still
        // serve missing payloads after view changes.
        let stale_pending: Vec<_> = self
            .pending
            .pending_blocks
            .iter()
            .filter(|(_, pending)| pending.height == height && pending.view < min_view)
            .map(|(hash, _)| *hash)
            .collect();
        let mut pending_removed = 0usize;
        let mut pending_retained = 0usize;
        for hash in stale_pending {
            if let Some(mut pending) = self.pending.pending_blocks.remove(&hash) {
                let committed = self.kura.get_block_height_by_hash(hash).is_some();
                let invalid = matches!(pending.validation_status, ValidationStatus::Invalid);
                if da_enabled && !committed && !invalid {
                    if !pending.aborted {
                        pending.mark_aborted();
                    }
                    self.pending.pending_blocks.insert(hash, pending);
                    pending_retained = pending_retained.saturating_add(1);
                    continue;
                }
                if !da_enabled || committed || invalid {
                    self.clean_rbc_sessions_for_block(hash, pending.height);
                }
                pending_removed = pending_removed.saturating_add(1);
            }
        }

        let mut missing_removed = 0usize;
        let stale_missing: Vec<_> = self
            .pending
            .missing_block_requests
            .iter()
            .filter(|(_, request)| request.height == height && request.view < min_view)
            .filter(|(hash, _)| !da_enabled || self.block_payload_available_locally(**hash))
            .map(|(hash, _)| *hash)
            .collect();
        for hash in stale_missing {
            if self.pending.missing_block_requests.remove(&hash).is_some() {
                missing_removed = missing_removed.saturating_add(1);
            }
        }

        let stale_rbc: Vec<_> = self
            .subsystems
            .da_rbc
            .rbc
            .sessions
            .iter()
            .filter(|(key, _)| key.1 == height && key.2 < min_view)
            .filter(|(key, session)| {
                !da_enabled || session.is_invalid() || self.block_payload_available_locally(key.0)
            })
            .map(|(key, _)| *key)
            .collect();
        let mut rbc_removed = 0usize;
        for key in stale_rbc {
            self.purge_rbc_state(key, key.0, key.1, key.2);
            rbc_removed = rbc_removed.saturating_add(1);
        }

        if pending_removed > 0 || pending_retained > 0 || missing_removed > 0 || rbc_removed > 0 {
            info!(
                height,
                min_view,
                pending_removed,
                pending_retained,
                missing_removed,
                rbc_removed,
                "pruned stale view state after view change"
            );
        }
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
        self.subsystems.da_rbc.rbc.ready_deferral.remove(&key);
        self.subsystems.da_rbc.rbc.deliver_deferral.remove(&key);
        self.subsystems.da_rbc.rbc.outbound_chunks.remove(&key);
        if self.subsystems.da_rbc.rbc.outbound_cursor == Some(key) {
            self.subsystems.da_rbc.rbc.outbound_cursor = None;
        }
        self.subsystems
            .da_rbc
            .rbc
            .persisted_full_sessions
            .remove(&key);
        self.subsystems.da_rbc.rbc.persist_inflight.remove(&key);
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

    #[allow(clippy::too_many_lines)]
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
        let queue_depths = super::status::worker_queue_depth_snapshot();
        let active_queue_len = self.queue.active_len();
        let pending_blocks = self.pending.pending_blocks.len();
        let missing_blocks = self.pending.missing_block_requests.len();
        let rbc_sessions = self.subsystems.da_rbc.rbc.sessions.len();
        let commit_inflight = self
            .subsystems
            .commit
            .inflight
            .as_ref()
            .map(|inflight| (inflight.pending.height, inflight.pending.view));
        let highest_qc = self
            .highest_qc
            .as_ref()
            .map(|qc| (qc.height, qc.view, qc.phase));
        let locked_qc = self
            .locked_qc
            .as_ref()
            .map(|qc| (qc.height, qc.view, qc.phase));
        info!(
            height,
            view,
            cause = cause.as_str(),
            active_queue_len,
            pending_blocks,
            missing_blocks,
            rbc_sessions,
            commit_inflight = ?commit_inflight,
            highest_qc = ?highest_qc,
            locked_qc = ?locked_qc,
            vote_rx_depth = queue_depths.vote_rx,
            block_payload_rx_depth = queue_depths.block_payload_rx,
            rbc_chunk_rx_depth = queue_depths.rbc_chunk_rx,
            block_rx_depth = queue_depths.block_rx,
            consensus_rx_depth = queue_depths.consensus_rx,
            lane_relay_rx_depth = queue_depths.lane_relay_rx,
            background_rx_depth = queue_depths.background_rx,
            "view change triggered"
        );
        let now = Instant::now();
        let prev_view = self.phase_tracker.current_view(height);
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
            self.config.gating.future_view_window,
            now,
        );
        super::status::set_view_change_index(next_view);
        super::status::inc_view_change_suggest();
        if let Some(telemetry) = self.telemetry_handle() {
            telemetry.inc_view_change_suggest();
        }
        let advanced = prev_view.map_or(next_view > 0, |prev| next_view > prev);
        if advanced {
            super::status::inc_view_change_install();
            if let Some(telemetry) = self.telemetry_handle() {
                telemetry.inc_view_change_install();
            }
        }
        self.subsystems.propose.forced_view_after_timeout = Some((height, next_view));
        self.prune_stale_view_state(height, next_view);
        // Drop earlier views for the active height so proposals_seen can't grow unbounded.
        self.subsystems
            .propose
            .proposals_seen
            .retain(|(entry_height, entry_view)| {
                *entry_height != height || *entry_view >= next_view
            });
        self.prune_vote_caches_horizon(self.last_committed_height);
        record_view_change_cause_with_telemetry(cause, self.telemetry_handle());
        let committed_qc = self.latest_committed_qc();
        if let Some(mut highest_qc) = self.highest_qc.or(committed_qc) {
            let valid_phase = matches!(
                highest_qc.phase,
                crate::sumeragi::consensus::Phase::Commit
                    | crate::sumeragi::consensus::Phase::Prepare
            );
            let expected_height = highest_qc.height.saturating_add(1);
            if !valid_phase || height != expected_height {
                if let Some(committed) = committed_qc {
                    highest_qc = committed;
                } else {
                    debug!(
                        height,
                        view = next_view,
                        phase = ?highest_qc.phase,
                        expected_height,
                        "skipping NEW_VIEW vote: highest certificate does not align with target height"
                    );
                }
            }
            let valid_phase = matches!(
                highest_qc.phase,
                crate::sumeragi::consensus::Phase::Commit
                    | crate::sumeragi::consensus::Phase::Prepare
            );
            let expected_height = highest_qc.height.saturating_add(1);
            if valid_phase && height == expected_height {
                let (consensus_mode, _, _) = self.consensus_context_for_height(height);
                let roster = self.roster_for_new_view_with_mode(
                    highest_qc.subject_block_hash,
                    height,
                    next_view,
                    consensus_mode,
                );
                if roster.is_empty() {
                    debug!(
                        height,
                        view = next_view,
                        highest_height = highest_qc.height,
                        highest_view = highest_qc.view,
                        "skipping NEW_VIEW vote: empty commit topology"
                    );
                } else {
                    let topology = super::network_topology::Topology::new(roster);
                    self.emit_new_view_vote(height, next_view, highest_qc, &topology);
                }
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
    da_enabled: bool,
) -> Duration {
    if proposal_seen {
        return commit_timeout;
    }
    if da_enabled {
        // DA payload propagation and worker-loop drain can lag; give one extra propose interval.
        let grace = commit_timeout.saturating_add(propose_interval);
        return grace.max(Duration::from_millis(1));
    }
    // No proposal observed: rotate sooner to recover from a missing leader, but cap
    // the timeout so we don't exceed the normal commit window.
    let grace = propose_interval.saturating_mul(4);
    grace.min(commit_timeout).max(Duration::from_millis(1))
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
    rbc_session_roster_sources: &mut BTreeMap<super::rbc_store::SessionKey, RbcRosterSource>,
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
        rbc_session_roster_sources.remove(key);
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
        config.da.enabled,
        config.da.quorum_timeout_multiplier,
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
    // NEW_VIEW advances from the highest QC (prepare/commit) or the latest committed height.
    let candidate = match (highest_qc, committed_qc) {
        (Some(highest), Some(committed)) => {
            if (highest.height, highest.view) >= (committed.height, committed.view) {
                Some(highest)
            } else {
                Some(committed)
            }
        }
        (Some(highest), None) => Some(highest),
        (None, Some(committed)) => Some(committed),
        (None, None) => None,
    };
    candidate.map_or_else(
        || committed_height.saturating_add(1),
        |qc| qc.height.saturating_add(1),
    )
}

fn pacemaker_base_interval_with_propose_timeout(
    block_time: Duration,
    propose_timeout: Duration,
    config: &SumeragiConfig,
) -> Duration {
    let propose_seed = propose_timeout;
    let rtt_mul = config.pacemaker.rtt_floor_multiplier.max(1);
    let propose_floor = saturating_mul_duration(propose_seed, rtt_mul);
    let max_backoff = config.pacemaker.max_backoff;
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

#[derive(Debug, Default, Clone, Copy)]
struct RelayDropCounters {
    subscriber_queue_full: u64,
    dropped_post: u64,
    dropped_broadcast: u64,
    post_overflow: u64,
    cap_violations: u64,
}

impl RelayDropCounters {
    fn collect() -> Self {
        let cap_violations = iroha_p2p::network::cap_violations_consensus()
            .saturating_add(iroha_p2p::network::cap_violations_control())
            .saturating_add(iroha_p2p::network::cap_violations_block_sync())
            .saturating_add(iroha_p2p::network::cap_violations_tx_gossip())
            .saturating_add(iroha_p2p::network::cap_violations_peer_gossip())
            .saturating_add(iroha_p2p::network::cap_violations_health())
            .saturating_add(iroha_p2p::network::cap_violations_other());
        Self {
            subscriber_queue_full: iroha_p2p::network::subscriber_queue_full_count(),
            dropped_post: iroha_p2p::network::dropped_post_count(),
            dropped_broadcast: iroha_p2p::network::dropped_broadcast_count(),
            post_overflow: iroha_p2p::network::post_overflow_count(),
            cap_violations,
        }
    }

    fn total(self) -> u64 {
        self.subscriber_queue_full
            .saturating_add(self.dropped_post)
            .saturating_add(self.dropped_broadcast)
            .saturating_add(self.post_overflow)
            .saturating_add(self.cap_violations)
    }
}

#[cfg(test)]
mod relay_drop_counters_tests {
    use super::RelayDropCounters;

    #[test]
    fn relay_drop_counters_total_sums_fields() {
        let counters = RelayDropCounters {
            subscriber_queue_full: 3,
            dropped_post: 5,
            dropped_broadcast: 7,
            post_overflow: 11,
            cap_violations: 13,
        };
        assert_eq!(counters.total(), 39);
    }
}

#[cfg(test)]
mod queue_drop_backpressure_tests {
    use std::time::{Duration, Instant};

    use super::QueueDropBackpressure;
    use crate::sumeragi::status;
    use crate::sumeragi::status::WorkerQueueKind;

    #[test]
    fn queue_drop_backpressure_tracks_recent_drops() {
        status::reset_worker_loop_snapshot_for_tests();
        let mut backpressure = QueueDropBackpressure::new();
        let now = Instant::now();
        assert!(!backpressure.active(now, Duration::from_secs(1)));

        status::record_worker_queue_drop(WorkerQueueKind::BlockPayload);
        let after_drop = now + Duration::from_millis(1);
        assert!(backpressure.active(after_drop, Duration::from_secs(5)));

        let later = after_drop + Duration::from_secs(6);
        assert!(!backpressure.active(later, Duration::from_secs(5)));
    }
}

#[cfg(test)]
mod queue_block_backpressure_tests {
    use std::time::{Duration, Instant};

    use super::QueueBlockBackpressure;
    use crate::sumeragi::status;
    use crate::sumeragi::status::WorkerQueueKind;

    #[test]
    fn queue_block_backpressure_tracks_recent_blocks() {
        status::reset_worker_loop_snapshot_for_tests();
        let mut backpressure = QueueBlockBackpressure::new();
        let now = Instant::now();
        assert!(!backpressure.active(now, Duration::from_secs(1)));

        status::record_worker_queue_blocked(
            WorkerQueueKind::BlockPayload,
            Duration::from_millis(5),
        );
        let after_block = now + Duration::from_millis(1);
        assert!(backpressure.active(after_block, Duration::from_secs(5)));

        let later = after_block + Duration::from_secs(6);
        assert!(!backpressure.active(later, Duration::from_secs(5)));
    }
}

#[derive(Debug)]
struct RelayBackpressure {
    last_drop_count: u64,
    last_drop_at: Option<Instant>,
}

impl RelayBackpressure {
    fn new() -> Self {
        Self {
            last_drop_count: Self::drop_count(),
            last_drop_at: None,
        }
    }

    fn drop_count() -> u64 {
        RelayDropCounters::collect().total()
    }

    fn active(&mut self, now: Instant, cooldown: Duration) -> bool {
        let current = Self::drop_count();
        if current > self.last_drop_count {
            self.last_drop_count = current;
            self.last_drop_at = Some(now);
        }
        self.last_drop_at
            .is_some_and(|last| now.saturating_duration_since(last) < cooldown)
    }

    #[cfg(test)]
    fn reset_to_current(&mut self) {
        self.last_drop_count = Self::drop_count();
        self.last_drop_at = None;
    }
}

impl Default for RelayBackpressure {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct QueueDropBackpressure {
    last_drop_count: u64,
    last_drop_at: Option<Instant>,
}

impl QueueDropBackpressure {
    fn new() -> Self {
        Self {
            last_drop_count: Self::drop_count(),
            last_drop_at: None,
        }
    }

    fn drop_count() -> u64 {
        let dropped = super::status::snapshot()
            .worker_loop
            .queue_diagnostics
            .dropped_total;
        dropped
            .block_payload_rx
            .saturating_add(dropped.rbc_chunk_rx)
    }

    fn active(&mut self, now: Instant, cooldown: Duration) -> bool {
        let current = Self::drop_count();
        if current > self.last_drop_count {
            self.last_drop_count = current;
            self.last_drop_at = Some(now);
        }
        self.last_drop_at
            .is_some_and(|last| now.saturating_duration_since(last) < cooldown)
    }

    #[cfg(test)]
    fn reset_to_current(&mut self) {
        self.last_drop_count = Self::drop_count();
        self.last_drop_at = None;
    }
}

impl Default for QueueDropBackpressure {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct QueueBlockBackpressure {
    last_blocked_count: u64,
    last_blocked_at: Option<Instant>,
}

impl QueueBlockBackpressure {
    fn new() -> Self {
        Self {
            last_blocked_count: Self::blocked_count(),
            last_blocked_at: None,
        }
    }

    fn blocked_count() -> u64 {
        let blocked = super::status::snapshot()
            .worker_loop
            .queue_diagnostics
            .blocked_total;
        blocked
            .block_payload_rx
            .saturating_add(blocked.rbc_chunk_rx)
    }

    fn active(&mut self, now: Instant, cooldown: Duration) -> bool {
        let current = Self::blocked_count();
        if current > self.last_blocked_count {
            self.last_blocked_count = current;
            self.last_blocked_at = Some(now);
        }
        self.last_blocked_at
            .is_some_and(|last| now.saturating_duration_since(last) < cooldown)
    }

    #[cfg(test)]
    fn reset_to_current(&mut self) {
        self.last_blocked_count = Self::blocked_count();
        self.last_blocked_at = None;
    }
}

impl Default for QueueBlockBackpressure {
    fn default() -> Self {
        Self::new()
    }
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

#[derive(Debug, Default)]
struct NewViewRebroadcastThrottle {
    last_sent: BTreeMap<(HashOf<BlockHeader>, u64, u64), Instant>,
}

impl NewViewRebroadcastThrottle {
    fn allow(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        now: Instant,
        cooldown: Duration,
    ) -> bool {
        let key = (block_hash, height, view);
        if let Some(previous) = self.last_sent.get(&key) {
            if cooldown > Duration::ZERO && now.saturating_duration_since(*previous) < cooldown {
                return false;
            }
        }
        self.last_sent.insert(key, now);

        if cooldown > Duration::ZERO {
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
    #[allow(dead_code)]
    CollectDa,
    CollectPrepare,
    CollectCommit,
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
    const COMMIT: u8 = 1 << 4;

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
            commit: Ema::new(PACEMAKER_PHASE_EMA_ALPHA, duration_ms(timeouts.commit)),
        }
    }

    fn update(&mut self, phase: PipelinePhase, sample_ms: f64) -> f64 {
        match phase {
            PipelinePhase::Propose => self.propose.update(sample_ms),
            PipelinePhase::CollectDa => self.collect_da.update(sample_ms),
            PipelinePhase::CollectPrepare => self.collect_prevote.update(sample_ms),
            PipelinePhase::CollectCommit => self.collect_precommit.update(sample_ms),
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
            PipelinePhase::Commit => self.commit.current(),
        }
    }

    pub(super) fn total_duration(&self) -> Duration {
        let total_ms = self.propose.current()
            + self.collect_da.current()
            + self.collect_prevote.current()
            + self.collect_precommit.current()
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

#[derive(Clone, Copy, Debug)]
struct QueueReadySince {
    height: u64,
    view: u64,
    since: Instant,
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

/// Return `true` when we have observed a delivered RBC payload with a complete
/// chunk set that matches the given block hash, height, and payload digest without
/// being marked invalid.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChunkIngestOutcome {
    Accepted,
    Duplicate,
    OutOfBounds,
    DigestMismatch,
    ExpectedDigestMissing,
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
    /// Chunk digest count mismatched the advertised chunk count.
    #[error("RBC chunk digest count mismatch: expected {total_chunks}, observed {observed}")]
    DigestCountMismatch {
        /// Reported chunk count in the RBC session.
        total_chunks: u32,
        /// Observed digest count.
        observed: usize,
    },
}

#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct RbcSession {
    total_chunks: u32,
    payload_hash: Option<Hash>,
    expected_chunk_root: Option<Hash>,
    expected_chunk_digests: Option<Vec<[u8; 32]>>,
    block_header: Option<BlockHeader>,
    leader_signature: Option<BlockSignature>,
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
        expected_chunk_digests: Option<Vec<[u8; 32]>>,
        epoch: u64,
    ) -> Result<Self, RbcSessionError> {
        if total_chunks > RBC_MAX_TOTAL_CHUNKS {
            return Err(RbcSessionError::TooManyChunks {
                total_chunks,
                max_chunks: RBC_MAX_TOTAL_CHUNKS,
            });
        }
        if let Some(ref digests) = expected_chunk_digests {
            if digests.len() != usize::try_from(total_chunks).unwrap_or(usize::MAX) {
                return Err(RbcSessionError::DigestCountMismatch {
                    total_chunks,
                    observed: digests.len(),
                });
            }
        }
        let capacity = usize::try_from(total_chunks).unwrap_or(RBC_MAX_TOTAL_CHUNKS as usize);
        Ok(Self {
            total_chunks,
            payload_hash,
            expected_chunk_root,
            expected_chunk_digests,
            block_header: None,
            leader_signature: None,
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
        Self::new(total_chunks, payload_hash, expected_chunk_root, None, epoch)
            .expect("test configuration should respect RBC chunk cap")
    }

    /// Seed the block header and leader signature for test-only RBC sessions.
    #[cfg(test)]
    pub(crate) fn test_set_block_header_and_signature(&mut self, block: &SignedBlock) {
        let signature = block
            .signatures()
            .next()
            .cloned()
            .expect("test block must include a leader signature");
        self.block_header = Some(block.header());
        self.leader_signature = Some(signature);
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
            && self.received_chunks == self.total_chunks
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

        let chunk_digests = self
            .expected_chunk_digests
            .clone()
            .or_else(|| self.all_chunk_digests())
            .unwrap_or_default();
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
            block_header: self.block_header,
            leader_signature: self.leader_signature.clone(),
            total_chunks: self.total_chunks,
            chunk_digests,
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

    #[allow(clippy::too_many_lines)]
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
            chunk_digests,
            payload_hash,
            expected_chunk_root,
            computed_chunk_root,
            invalid,
            ready_signatures,
            delivered,
            deliver_sender,
            deliver_signature,
            sent_ready,
            chunks,
            epoch,
            block_header,
            leader_signature,
            ..
        } = persisted.clone();
        let expected_chunk_root = expected_chunk_root.or(computed_chunk_root);

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

        let expected_chunk_digests = if !chunk_digests.is_empty() {
            if chunk_digests.len() != usize::try_from(total_chunks).unwrap_or(usize::MAX) {
                return Err(PersistedLoadError::DigestCountMismatch {
                    total_chunks,
                    observed: chunk_digests.len(),
                });
            }
            Some(chunk_digests)
        } else if received_chunks == total_chunks {
            let mut digests = Vec::with_capacity(data.len());
            for entry in &data {
                let entry = entry
                    .as_ref()
                    .expect("received_chunks == total_chunks implies all chunk slots filled");
                digests.push(entry.digest);
            }
            Some(digests)
        } else {
            None
        };

        let ready_signatures = ready_signatures
            .into_iter()
            .map(|sig| ReadySignature {
                sender: sig.sender,
                signature: sig.signature,
            })
            .collect();

        let mut session = Self::new(
            total_chunks,
            payload_hash,
            expected_chunk_root,
            expected_chunk_digests,
            epoch,
        )
        .map_err(|err| match err {
            RbcSessionError::TooManyChunks {
                total_chunks,
                max_chunks,
            } => PersistedLoadError::TooManyChunks {
                total_chunks,
                max_chunks,
            },
            RbcSessionError::DigestCountMismatch {
                total_chunks,
                observed,
            } => PersistedLoadError::DigestCountMismatch {
                total_chunks,
                observed,
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
        session.block_header = block_header;
        session.leader_signature = leader_signature;
        session.drop_mismatched_chunks();
        session.recovered_from_disk = true;
        Ok(session)
    }

    pub(crate) fn recovered_from_disk(&self) -> bool {
        self.recovered_from_disk
    }

    pub(crate) fn ingest_chunk(&mut self, idx: u32, bytes: Vec<u8>, sender: Option<u32>) {
        let _ = self.note_chunk(idx, bytes, sender);
    }

    pub(super) fn ingest_chunk_with_outcome(
        &mut self,
        idx: u32,
        bytes: Vec<u8>,
        sender: Option<u32>,
    ) -> ChunkIngestOutcome {
        self.note_chunk(idx, bytes, sender)
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
        let _ = self.note_chunk(idx, bytes, Some(sender));
    }

    fn drop_mismatched_chunks(&mut self) -> usize {
        let Some(expected) = self.expected_chunk_digests.as_ref() else {
            return 0;
        };
        if expected.len() != self.chunks.len() {
            self.invalid = true;
            return 0;
        }

        let mut dropped = 0usize;
        for (idx, slot) in self.chunks.iter_mut().enumerate() {
            let Some(entry) = slot.as_ref() else {
                continue;
            };
            if expected[idx] != entry.digest {
                *slot = None;
                dropped = dropped.saturating_add(1);
            }
        }

        if dropped > 0 {
            self.received_chunks = self
                .chunks
                .iter()
                .filter(|entry| entry.is_some())
                .count()
                .try_into()
                .unwrap_or(u32::MAX);
        }

        dropped
    }

    fn note_chunk(&mut self, idx: u32, bytes: Vec<u8>, _sender: Option<u32>) -> ChunkIngestOutcome {
        if idx >= self.total_chunks {
            return ChunkIngestOutcome::OutOfBounds;
        }
        let mut hasher = Sha256::new();
        sha2::digest::Update::update(&mut hasher, &bytes);
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&hasher.finalize());

        if let Some(expected) = self.expected_chunk_digests.as_ref() {
            let Some(expected_digest) = expected.get(idx as usize) else {
                self.invalid = true;
                return ChunkIngestOutcome::ExpectedDigestMissing;
            };
            if expected_digest != &digest {
                return ChunkIngestOutcome::DigestMismatch;
            }
        }

        let slot = &mut self.chunks[idx as usize];
        if slot.is_none() {
            *slot = Some(RbcChunkEntry { bytes, digest });
            self.received_chunks = self.received_chunks.saturating_add(1);
            return ChunkIngestOutcome::Accepted;
        }
        ChunkIngestOutcome::Duplicate
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
    /// Persisted chunk digest list length mismatched the chunk count.
    #[error("persisted RBC digest count mismatch: expected {total_chunks}, observed {observed}")]
    DigestCountMismatch {
        /// Persisted chunk count.
        total_chunks: u32,
        /// Observed digest count.
        observed: usize,
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
        return Err(std::io::Error::other(
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
