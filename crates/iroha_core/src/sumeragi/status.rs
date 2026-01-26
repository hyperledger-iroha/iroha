//! Minimal snapshot of Sumeragi status for operator/light-client queries.
//! Stores leader index and `HighestQC` tuple using atomics.

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
#[cfg(test)]
use std::sync::Condvar;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering as StdOrdering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iroha_config::parameters::actual::ConsensusMode;
use iroha_crypto::{
    Hash, Hash as UntypedHash, HashOf,
    privacy::{CommitmentScheme, LanePrivacyCommitment},
};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{LaneBlockCommitment, SumeragiMembershipStatus, ValidatorIndex},
    },
    consensus::{ConsensusKeyRecord, Qc, ValidatorElectionOutcome, ValidatorSetCheckpoint},
    isi::settlement::{SettlementAtomicity, SettlementExecutionOrder},
    nexus::{LaneId, LaneRelayEnvelope, LaneRelayError},
    peer::PeerId,
};
use iroha_primitives::numeric::Numeric;
use iroha_telemetry::metrics;
use norito::codec::{Decode, Encode};

pub use crate::sumeragi::da::ManifestGateKind;
use crate::{
    governance::manifest::{GovernanceRules, LaneManifestStatus, RuntimeUpgradeHook},
    queue::BackpressureState,
    sumeragi::da::{GateReason, GateSatisfaction},
    sumeragi::stake_snapshot::CommitStakeSnapshot,
    telemetry::TxGossipSnapshot,
};

/// Last-observed reason that data-availability tracking reported missing data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DaGateReasonSnapshot {
    /// No DA availability shortfall has been recorded.
    #[default]
    None,
    /// Missing local data required to validate.
    MissingLocalData,
    /// Manifest was not found on disk.
    ManifestMissing,
    /// Manifest hash mismatched the commitment.
    ManifestHashMismatch,
    /// Manifest file could not be read.
    ManifestReadFailed,
    /// Spool scan failed while searching for manifests.
    ManifestSpoolScan,
}

impl DaGateReasonSnapshot {
    const fn as_code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::MissingLocalData => 1,
            Self::ManifestMissing => 3,
            Self::ManifestHashMismatch => 4,
            Self::ManifestReadFailed => 5,
            Self::ManifestSpoolScan => 6,
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            1 => Self::MissingLocalData,
            3 => Self::ManifestMissing,
            4 => Self::ManifestHashMismatch,
            5 => Self::ManifestReadFailed,
            6 => Self::ManifestSpoolScan,
            _ => Self::None,
        }
    }

    /// String label used for telemetry payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MissingLocalData => "missing_local_data",
            Self::ManifestMissing => "manifest_missing",
            Self::ManifestHashMismatch => "manifest_hash_mismatch",
            Self::ManifestReadFailed => "manifest_read_failed",
            Self::ManifestSpoolScan => "manifest_spool_scan",
        }
    }
}

impl From<GateReason> for DaGateReasonSnapshot {
    fn from(value: GateReason) -> Self {
        match value {
            GateReason::MissingLocalData => Self::MissingLocalData,
            GateReason::ManifestGuard { kind, .. } => match kind {
                ManifestGateKind::Missing => Self::ManifestMissing,
                ManifestGateKind::HashMismatch => Self::ManifestHashMismatch,
                ManifestGateKind::ReadFailed => Self::ManifestReadFailed,
                ManifestGateKind::SpoolScan => Self::ManifestSpoolScan,
            },
        }
    }
}

/// Last-observed satisfaction condition that cleared the DA gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DaGateSatisfactionSnapshot {
    /// No satisfaction recorded.
    #[default]
    None,
    /// Missing local data recovered.
    MissingDataRecovered,
}

impl DaGateSatisfactionSnapshot {
    const fn as_code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::MissingDataRecovered => 1,
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            1 => Self::MissingDataRecovered,
            _ => Self::None,
        }
    }

    /// String label used for telemetry payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MissingDataRecovered => "missing_data_recovered",
        }
    }
}

impl From<GateSatisfaction> for DaGateSatisfactionSnapshot {
    fn from(value: GateSatisfaction) -> Self {
        match value {
            GateSatisfaction::MissingDataRecovered => Self::MissingDataRecovered,
        }
    }
}

/// Snapshot of data-availability tracking state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DaGateSnapshot {
    /// Most recent reason that reported missing availability evidence.
    pub reason: DaGateReasonSnapshot,
    /// Most recent condition that satisfied availability tracking.
    pub last_satisfied: DaGateSatisfactionSnapshot,
    /// Count of times local data was missing.
    pub missing_local_data_total: u64,
    /// Count of times manifest guard reported missing/invalid manifests.
    pub manifest_guard_total: u64,
}

/// Snapshot of RBC abort counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RbcAbortSnapshot {
    /// Total times an RBC session was aborted.
    pub total: u64,
    /// Height associated with the last recorded abort.
    pub last_height: u64,
    /// View associated with the last recorded abort.
    pub last_view: u64,
}

/// Classifies per-peer RBC payload mismatches for telemetry and status reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RbcMismatchKind {
    /// Chunk digest does not match the declared digest list.
    ChunkDigest,
    /// Payload hash does not match the expected value.
    PayloadHash,
    /// Merkle root for chunk digests does not match the expected root.
    ChunkRoot,
}

impl RbcMismatchKind {
    /// Telemetry label for the mismatch kind.
    pub fn label(self) -> &'static str {
        match self {
            Self::ChunkDigest => "chunk_digest",
            Self::PayloadHash => "payload_hash",
            Self::ChunkRoot => "chunk_root",
        }
    }
}

/// Snapshot of per-peer RBC payload mismatch counters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RbcMismatchSnapshot {
    /// Per-peer mismatch counters and last-observed timestamps.
    pub entries: Vec<RbcMismatchEntry>,
}

/// Per-peer mismatch counters tracked for RBC payload validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RbcMismatchEntry {
    /// Peer associated with the mismatch counts.
    pub peer_id: PeerId,
    /// Count of RBC chunk digest mismatches attributed to the peer.
    pub chunk_digest_mismatch_total: u64,
    /// Count of payload-hash mismatches attributed to the peer.
    pub payload_hash_mismatch_total: u64,
    /// Count of chunk-root mismatches attributed to the peer.
    pub chunk_root_mismatch_total: u64,
    /// Timestamp (ms since UNIX epoch) when the last mismatch was recorded.
    pub last_timestamp_ms: u64,
}

/// Snapshot of membership mismatch tracking.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MembershipMismatchSnapshot {
    /// Peers currently flagged for mismatched membership hashes.
    pub active_peers: Vec<PeerId>,
    /// Last peer observed with a mismatch (best-effort).
    pub last_peer: Option<PeerId>,
    /// Height associated with the last mismatch.
    pub last_height: u64,
    /// View associated with the last mismatch.
    pub last_view: u64,
    /// Epoch associated with the last mismatch.
    pub last_epoch: u64,
    /// Local membership hash observed during the last mismatch.
    pub last_local_hash: Option<[u8; 32]>,
    /// Remote membership hash observed during the last mismatch.
    pub last_remote_hash: Option<[u8; 32]>,
    /// Milliseconds since UNIX epoch when the last mismatch was recorded.
    pub last_timestamp_ms: u64,
}

#[derive(Clone, Debug)]
struct MembershipMismatchContext {
    peer: PeerId,
    height: u64,
    view: u64,
    epoch: u64,
    local_hash: [u8; 32],
    remote_hash: [u8; 32],
    timestamp_ms: u64,
}

#[derive(Clone, Debug, Default)]
struct MembershipMismatchState {
    active: bool,
    consecutive: u32,
    last_mismatch: Option<MembershipMismatchContext>,
}

#[derive(Debug, Default)]
struct MembershipMismatchRegistry {
    entries: BTreeMap<PeerId, MembershipMismatchState>,
    last: Option<MembershipMismatchContext>,
}

#[derive(Clone, Debug, Default)]
struct RbcMismatchCounts {
    chunk_digest_mismatch_total: u64,
    payload_hash_mismatch_total: u64,
    chunk_root_mismatch_total: u64,
    last_timestamp_ms: u64,
}

static LEADER_INDEX: AtomicU64 = AtomicU64::new(0);
static HIGHEST_QC_HEIGHT: AtomicU64 = AtomicU64::new(0);
static HIGHEST_QC_VIEW: AtomicU64 = AtomicU64::new(0);
static LOCKED_QC_HEIGHT: AtomicU64 = AtomicU64::new(0);
static LOCKED_QC_VIEW: AtomicU64 = AtomicU64::new(0);

// Optional subject block hash for the current HighestQC (best-effort).
static HIGHEST_QC_HASH: OnceLock<Mutex<Option<UntypedHash>>> = OnceLock::new();
static LOCKED_QC_HASH: OnceLock<Mutex<Option<UntypedHash>>> = OnceLock::new();

static RBC_ABORT_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBC_ABORT_LAST_HEIGHT: AtomicU64 = AtomicU64::new(0);
static RBC_ABORT_LAST_VIEW: AtomicU64 = AtomicU64::new(0);

// PRF leader context (NPoS): last seed/height/view used to compute leader index.
static PRF_HEIGHT: AtomicU64 = AtomicU64::new(0);
static PRF_VIEW: AtomicU64 = AtomicU64::new(0);
static PRF_SEED: OnceLock<Mutex<Option<[u8; 32]>>> = OnceLock::new();
static MEMBERSHIP_HASH_SET: AtomicBool = AtomicBool::new(false);
static MEMBERSHIP_HEIGHT: AtomicU64 = AtomicU64::new(0);
static MEMBERSHIP_VIEW: AtomicU64 = AtomicU64::new(0);
static MEMBERSHIP_EPOCH: AtomicU64 = AtomicU64::new(0);
static MEMBERSHIP_VIEW_HASH: OnceLock<Mutex<[u8; 32]>> = OnceLock::new();
static MEMBERSHIP_MISMATCH_REGISTRY: OnceLock<Mutex<MembershipMismatchRegistry>> = OnceLock::new();
static RBC_MISMATCH_REGISTRY: OnceLock<Mutex<BTreeMap<PeerId, RbcMismatchCounts>>> =
    OnceLock::new();
static MODE_TAG: OnceLock<Mutex<String>> = OnceLock::new();
static STAGED_MODE_TAG: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static STAGED_MODE_ACTIVATION_HEIGHT: OnceLock<Mutex<Option<u64>>> = OnceLock::new();
static MODE_ACTIVATION_LAG_BLOCKS: OnceLock<Mutex<Option<u64>>> = OnceLock::new();
static CONSENSUS_CAPS: OnceLock<Mutex<Option<iroha_p2p::ConsensusConfigCaps>>> = OnceLock::new();
#[allow(dead_code)]
static MODE_FLIP_KILL_SWITCH: AtomicBool = AtomicBool::new(true);
#[allow(dead_code)]
static MODE_FLIP_BLOCKED: AtomicBool = AtomicBool::new(false);
#[allow(dead_code)]
static MODE_FLIP_SUCCESS_TOTAL: AtomicU64 = AtomicU64::new(0);
#[allow(dead_code)]
static MODE_FLIP_FAIL_TOTAL: AtomicU64 = AtomicU64::new(0);
#[allow(dead_code)]
static MODE_FLIP_BLOCKED_TOTAL: AtomicU64 = AtomicU64::new(0);
#[allow(dead_code)]
static MODE_LAST_FLIP_TS_MS: AtomicU64 = AtomicU64::new(0);
#[allow(dead_code)]
static MODE_LAST_FLIP_TS_SET: AtomicBool = AtomicBool::new(false);
#[allow(dead_code)]
static MODE_LAST_FLIP_ERROR: OnceLock<Mutex<Option<String>>> = OnceLock::new();

// Last observed per-phase latencies (ms). Best-effort snapshots for operator dashboards.
static LAST_PROPOSE_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_DA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_PREVOTE_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_PRECOMMIT_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_AGG_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COMMIT_MS: AtomicU64 = AtomicU64::new(0);
static LAST_PROPOSE_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_DA_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_PREVOTE_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_PRECOMMIT_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_AGG_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COMMIT_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_PIPELINE_TOTAL_EMA_MS: AtomicU64 = AtomicU64::new(0);
static GOSSIP_FALLBACK_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_VOTE_EVICT_CAPACITY_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_VOTE_EVICT_EXPIRED_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_BLOCK_CREATED_EVICT_CAPACITY_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_BLOCK_CREATED_EVICT_EXPIRED_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_PROPOSAL_EVICT_CAPACITY_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_PROPOSAL_EVICT_EXPIRED_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_RBC_INIT_EVICT_CAPACITY_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_RBC_INIT_EVICT_EXPIRED_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_RBC_READY_EVICT_CAPACITY_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_RBC_READY_EVICT_EXPIRED_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_RBC_DELIVER_EVICT_CAPACITY_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_RBC_DELIVER_EVICT_EXPIRED_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_BLOCK_SYNC_UPDATE_EVICT_CAPACITY_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_BLOCK_SYNC_UPDATE_EVICT_EXPIRED_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_FETCH_PENDING_BLOCK_EVICT_CAPACITY_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_FETCH_PENDING_BLOCK_EVICT_EXPIRED_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_RBC_CHUNK_EVICT_CAPACITY_TOTAL: AtomicU64 = AtomicU64::new(0);
static DEDUP_RBC_CHUNK_EVICT_EXPIRED_TOTAL: AtomicU64 = AtomicU64::new(0);
static BG_POST_DROP_POST_TOTAL: AtomicU64 = AtomicU64::new(0);
static BG_POST_DROP_BROADCAST_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_CREATED_DROPPED_BY_LOCK_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_CREATED_HINT_MISMATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_CREATED_PROPOSAL_MISMATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static MESSAGE_HANDLING_TOTALS: OnceLock<Mutex<BTreeMap<ConsensusMessageHandlingKey, u64>>> =
    OnceLock::new();
static MISSING_BLOCK_FETCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static MISSING_BLOCK_FETCH_LAST_TARGETS: AtomicU64 = AtomicU64::new(0);
static MISSING_BLOCK_FETCH_LAST_DWELL_MS: AtomicU64 = AtomicU64::new(0);
static BLOCK_SYNC_DROP_INVALID_SIGNATURES_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_SYNC_QC_REPLACED_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_SYNC_QC_DERIVE_FAILED_TOTAL: AtomicU64 = AtomicU64::new(0);
static VALIDATION_REJECT_TOTAL: AtomicU64 = AtomicU64::new(0);
static VALIDATION_REJECT_REASON: OnceLock<Mutex<Option<&'static str>>> = OnceLock::new();
static VALIDATION_REJECT_STATELESS_TOTAL: AtomicU64 = AtomicU64::new(0);
static VALIDATION_REJECT_EXECUTION_TOTAL: AtomicU64 = AtomicU64::new(0);
static VALIDATION_REJECT_PREV_HASH_TOTAL: AtomicU64 = AtomicU64::new(0);
static VALIDATION_REJECT_PREV_HEIGHT_TOTAL: AtomicU64 = AtomicU64::new(0);
static VALIDATION_REJECT_TOPOLOGY_TOTAL: AtomicU64 = AtomicU64::new(0);
static VALIDATION_REJECT_LAST_HEIGHT: OnceLock<Mutex<Option<u64>>> = OnceLock::new();
static VALIDATION_REJECT_LAST_VIEW: OnceLock<Mutex<Option<u64>>> = OnceLock::new();
static VALIDATION_REJECT_LAST_BLOCK: OnceLock<Mutex<Option<UntypedHash>>> = OnceLock::new();
static VALIDATION_REJECT_LAST_TS_MS: AtomicU64 = AtomicU64::new(0);
static PEER_KEY_POLICY_REJECT_TOTAL: AtomicU64 = AtomicU64::new(0);
static PEER_KEY_POLICY_REASON: OnceLock<Mutex<Option<&'static str>>> = OnceLock::new();
static PEER_KEY_POLICY_MISSING_HSM_TOTAL: AtomicU64 = AtomicU64::new(0);
static PEER_KEY_POLICY_ALGO_TOTAL: AtomicU64 = AtomicU64::new(0);
static PEER_KEY_POLICY_PROVIDER_TOTAL: AtomicU64 = AtomicU64::new(0);
static PEER_KEY_POLICY_LEAD_TIME_TOTAL: AtomicU64 = AtomicU64::new(0);
static PEER_KEY_POLICY_ID_COLLISION_TOTAL: AtomicU64 = AtomicU64::new(0);
static PEER_KEY_POLICY_ACTIVATION_PAST_TOTAL: AtomicU64 = AtomicU64::new(0);
static PEER_KEY_POLICY_EXPIRY_TOTAL: AtomicU64 = AtomicU64::new(0);
static PEER_KEY_POLICY_LAST_TS_MS: AtomicU64 = AtomicU64::new(0);
static BLOCK_SYNC_ROSTER_COMMIT_CERT_HINT_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_SYNC_ROSTER_CHECKPOINT_HINT_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_SYNC_ROSTER_COMMIT_CERT_HISTORY_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_SYNC_ROSTER_CHECKPOINT_HISTORY_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_SYNC_ROSTER_ROSTER_SIDECAR_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_SYNC_ROSTER_COMMIT_ROSTER_JOURNAL_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_SYNC_ROSTER_DROP_MISSING_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_SYNC_ROSTER_DROP_UNSOLICITED_SHARE_BLOCKS_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_COMMIT_FAILURE_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_QUORUM_TIMEOUT_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_STAKE_QUORUM_TIMEOUT_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_DA_GATE_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_CENSORSHIP_EVIDENCE_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_MISSING_PAYLOAD_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_MISSING_QC_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_VALIDATION_REJECT_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_LAST_TS_MS: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_LAST_LABEL: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static VIEW_CHANGE_CAUSE_LAST_COMMIT_FAILURE_TS_MS: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_LAST_QUORUM_TIMEOUT_TS_MS: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_LAST_STAKE_QUORUM_TIMEOUT_TS_MS: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_LAST_DA_GATE_TS_MS: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_LAST_CENSORSHIP_EVIDENCE_TS_MS: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_LAST_MISSING_PAYLOAD_TS_MS: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_LAST_MISSING_QC_TS_MS: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_CAUSE_LAST_VALIDATION_REJECT_TS_MS: AtomicU64 = AtomicU64::new(0);
static KURA_STORE_FAILURE_TOTAL: AtomicU64 = AtomicU64::new(0);
static KURA_STORE_ABORT_TOTAL: AtomicU64 = AtomicU64::new(0);
static KURA_STAGE_TOTAL: AtomicU64 = AtomicU64::new(0);
static KURA_STAGE_ROLLBACK_TOTAL: AtomicU64 = AtomicU64::new(0);
static KURA_STAGE_LAST_HEIGHT: AtomicU64 = AtomicU64::new(0);
static KURA_STAGE_LAST_VIEW: AtomicU64 = AtomicU64::new(0);
static KURA_STAGE_LAST_HASH: OnceLock<Mutex<Option<UntypedHash>>> = OnceLock::new();
static KURA_STAGE_ROLLBACK_LAST_HEIGHT: AtomicU64 = AtomicU64::new(0);
static KURA_STAGE_ROLLBACK_LAST_VIEW: AtomicU64 = AtomicU64::new(0);
static KURA_STAGE_ROLLBACK_LAST_HASH: OnceLock<Mutex<Option<UntypedHash>>> = OnceLock::new();
static KURA_STAGE_ROLLBACK_LAST_REASON: OnceLock<Mutex<Option<&'static str>>> = OnceLock::new();
static KURA_LOCK_RESET_TOTAL: AtomicU64 = AtomicU64::new(0);
static KURA_LOCK_RESET_LAST_HEIGHT: AtomicU64 = AtomicU64::new(0);
static KURA_LOCK_RESET_LAST_VIEW: AtomicU64 = AtomicU64::new(0);
static KURA_LOCK_RESET_LAST_HASH: OnceLock<Mutex<Option<UntypedHash>>> = OnceLock::new();
static KURA_LOCK_RESET_LAST_REASON: OnceLock<Mutex<Option<&'static str>>> = OnceLock::new();
static KURA_STORE_LAST_HEIGHT: AtomicU64 = AtomicU64::new(0);
static KURA_STORE_LAST_VIEW: AtomicU64 = AtomicU64::new(0);
static KURA_STORE_LAST_HASH: OnceLock<Mutex<Option<UntypedHash>>> = OnceLock::new();
static KURA_STORE_LAST_RETRY_ATTEMPT: AtomicU64 = AtomicU64::new(0);
static KURA_STORE_LAST_RETRY_BACKOFF_MS: AtomicU64 = AtomicU64::new(0);
static PACEMAKER_BACKPRESSURE_DEFERRALS_TOTAL: AtomicU64 = AtomicU64::new(0);
static COMMIT_PIPELINE_TICK_TOTAL: AtomicU64 = AtomicU64::new(0);
static COMMIT_INFLIGHT_ACTIVE: AtomicBool = AtomicBool::new(false);
static COMMIT_INFLIGHT_ID: AtomicU64 = AtomicU64::new(0);
static COMMIT_INFLIGHT_HEIGHT: AtomicU64 = AtomicU64::new(0);
static COMMIT_INFLIGHT_VIEW: AtomicU64 = AtomicU64::new(0);
static COMMIT_INFLIGHT_STARTED_MS: AtomicU64 = AtomicU64::new(0);
static COMMIT_INFLIGHT_TIMEOUT_MS: AtomicU64 = AtomicU64::new(0);
static COMMIT_INFLIGHT_TIMEOUT_TOTAL: AtomicU64 = AtomicU64::new(0);
static COMMIT_INFLIGHT_LAST_TIMEOUT_MS: AtomicU64 = AtomicU64::new(0);
static COMMIT_INFLIGHT_LAST_TIMEOUT_HEIGHT: AtomicU64 = AtomicU64::new(0);
static COMMIT_INFLIGHT_LAST_TIMEOUT_VIEW: AtomicU64 = AtomicU64::new(0);
static COMMIT_INFLIGHT_LAST_TIMEOUT_TS_MS: AtomicU64 = AtomicU64::new(0);
static COMMIT_PAUSE_TOTAL: AtomicU64 = AtomicU64::new(0);
static COMMIT_RESUME_TOTAL: AtomicU64 = AtomicU64::new(0);
static COMMIT_PAUSE_STARTED_MS: AtomicU64 = AtomicU64::new(0);
static COMMIT_INFLIGHT_HASH: OnceLock<Mutex<Option<UntypedHash>>> = OnceLock::new();
static COMMIT_INFLIGHT_LAST_TIMEOUT_HASH: OnceLock<Mutex<Option<UntypedHash>>> = OnceLock::new();
static COMMIT_PAUSE_QUEUE_DEPTHS: OnceLock<Mutex<WorkerQueueDepthSnapshot>> = OnceLock::new();
static COMMIT_RESUME_QUEUE_DEPTHS: OnceLock<Mutex<WorkerQueueDepthSnapshot>> = OnceLock::new();
static COMMIT_QUORUM_HEIGHT: AtomicU64 = AtomicU64::new(0);
static COMMIT_QUORUM_VIEW: AtomicU64 = AtomicU64::new(0);
static COMMIT_QUORUM_PRESENT: AtomicU64 = AtomicU64::new(0);
static COMMIT_QUORUM_COUNTED: AtomicU64 = AtomicU64::new(0);
static COMMIT_QUORUM_SET_B: AtomicU64 = AtomicU64::new(0);
static COMMIT_QUORUM_REQUIRED: AtomicU64 = AtomicU64::new(0);
static COMMIT_QUORUM_LAST_UPDATED_MS: AtomicU64 = AtomicU64::new(0);
static COMMIT_QUORUM_HASH: OnceLock<Mutex<Option<UntypedHash>>> = OnceLock::new();
static VIEW_CHANGE_INDEX: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_PROOF_ACCEPTED_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_PROOF_STALE_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_PROOF_REJECTED_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_SUGGEST_TOTAL: AtomicU64 = AtomicU64::new(0);
static VIEW_CHANGE_INSTALL_TOTAL: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static VIEW_CHANGE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(test)]
static VIEW_CHANGE_CAUSE_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static LANE_RELAY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(test)]
static MESSAGE_HANDLING_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static VOTE_VALIDATION_DROPS_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static MISSING_BLOCK_FETCH_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static BLOCK_SYNC_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static PREVOTE_TIMEOUT_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(test)]
static COMMIT_HISTORY_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static KURA_STORE_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static DA_GATE_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static QC_STATUS_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static VALIDATION_REJECT_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static RBC_STATUS_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();

static AVAILABILITY_STATS: OnceLock<Mutex<AvailabilityStats>> = OnceLock::new();
static QC_LATENCY_MS: OnceLock<Mutex<BTreeMap<&'static str, u64>>> = OnceLock::new();
static RBC_BACKLOG: OnceLock<Mutex<RbcBacklogSnapshot>> = OnceLock::new();
static PENDING_RBC_STATE: OnceLock<Mutex<PendingRbcSnapshot>> = OnceLock::new();
static PENDING_RBC_DROPS_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_DROPS_CAP_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_DROPS_TTL_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_DROPS_CAP_BYTES_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_DROPS_TTL_BYTES_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_DROPPED_BYTES_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_EVICTED_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_STASH_READY_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_STASH_READY_INIT_MISSING_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_STASH_READY_ROSTER_MISSING_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_STASH_READY_ROSTER_HASH_MISMATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_STASH_READY_ROSTER_UNVERIFIED_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_STASH_DELIVER_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_STASH_DELIVER_INIT_MISSING_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_STASH_DELIVER_ROSTER_MISSING_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_STASH_DELIVER_ROSTER_HASH_MISMATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_STASH_DELIVER_ROSTER_UNVERIFIED_TOTAL: AtomicU64 = AtomicU64::new(0);
static PENDING_RBC_STASH_CHUNK_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBC_STORE_SESSIONS: AtomicU64 = AtomicU64::new(0);
static RBC_STORE_BYTES: AtomicU64 = AtomicU64::new(0);
static RBC_STORE_PRESSURE_LEVEL: AtomicU8 = AtomicU8::new(0);
static RBC_STORE_PRESSURE_LOG_LEVEL: AtomicU8 = AtomicU8::new(0);
static RBC_STORE_PRESSURE_LOG_LAST_SECS: AtomicU64 = AtomicU64::new(0);
const RBC_STORE_PRESSURE_LOG_INTERVAL_SECS: u64 = 60;
static RBC_STORE_BACKPRESSURE_DEFERRALS_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBC_STORE_PERSIST_DROPS_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBC_STORE_EVICTIONS_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBC_DELIVER_DEFER_READY_TOTAL: AtomicU64 = AtomicU64::new(0);
static RBC_DELIVER_DEFER_CHUNKS_TOTAL: AtomicU64 = AtomicU64::new(0);
static DA_RESCHEDULE_TOTAL: AtomicU64 = AtomicU64::new(0);
static DA_GATE_MISSING_LOCAL_DATA_TOTAL: AtomicU64 = AtomicU64::new(0);
static DA_GATE_LAST_REASON: AtomicU8 = AtomicU8::new(DaGateReasonSnapshot::None.as_code());
static DA_GATE_LAST_SATISFIED: AtomicU8 = AtomicU8::new(DaGateSatisfactionSnapshot::None.as_code());
static DA_GATE_MANIFEST_GUARD_TOTAL: AtomicU64 = AtomicU64::new(0);
const RBC_STORE_RECENT_EVICTIONS_CAP: usize = 32;
const VOTE_VALIDATION_DROPS_CAP: usize = 256;
static RBC_STORE_RECENT_EVICTIONS: OnceLock<Mutex<VecDeque<RbcEvictedSession>>> = OnceLock::new();
static VOTE_VALIDATION_DROPS: OnceLock<Mutex<VecDeque<VoteValidationDropEntry>>> = OnceLock::new();
static VOTE_VALIDATION_DROPS_BY_PEER: OnceLock<
    Mutex<BTreeMap<VoteValidationDropPeerKey, VoteValidationDropPeerState>>,
> = OnceLock::new();
static VOTE_VALIDATION_DROPS_TOTAL: AtomicU64 = AtomicU64::new(0);
static SETTLEMENT_STATUS: OnceLock<Mutex<SettlementStatusState>> = OnceLock::new();
static LANE_ACTIVITY: OnceLock<Mutex<Vec<LaneActivitySnapshot>>> = OnceLock::new();
static ACCESS_SET_SOURCES: OnceLock<Mutex<AccessSetSourceSummary>> = OnceLock::new();
static DATASPACE_ACTIVITY: OnceLock<Mutex<Vec<DataspaceActivitySnapshot>>> = OnceLock::new();
static RBC_LANE_BACKLOG: OnceLock<Mutex<Vec<LaneRbcSnapshot>>> = OnceLock::new();
static RBC_DATASPACE_BACKLOG: OnceLock<Mutex<Vec<DataspaceRbcSnapshot>>> = OnceLock::new();
static LANE_COMMITMENTS: OnceLock<Mutex<Vec<LaneCommitmentSnapshot>>> = OnceLock::new();
static DATASPACE_COMMITMENTS: OnceLock<Mutex<Vec<DataspaceCommitmentSnapshot>>> = OnceLock::new();
static LANE_SETTLEMENT_COMMITMENTS: OnceLock<Mutex<Vec<LaneBlockCommitment>>> = OnceLock::new();
static LANE_RELAY_ENVELOPES: OnceLock<Mutex<Vec<LaneRelayEnvelope>>> = OnceLock::new();
static LANE_GOVERNANCE: OnceLock<Mutex<Vec<LaneGovernanceSnapshot>>> = OnceLock::new();
static NEXUS_FEE_STATUS: OnceLock<Mutex<NexusFeeSnapshot>> = OnceLock::new();
static NEXUS_STAKING_STATUS: OnceLock<Mutex<BTreeMap<LaneId, NexusStakingLaneSnapshot>>> =
    OnceLock::new();

static PIPELINE_CONFLICT_RATE_BPS: AtomicU64 = AtomicU64::new(0);
static QC_REBUILD_ATTEMPTS_TOTAL: AtomicU64 = AtomicU64::new(0);
static QC_REBUILD_SUCCESSES_TOTAL: AtomicU64 = AtomicU64::new(0);
static QC_MISSING_VOTES_ACCEPTED_TOTAL: AtomicU64 = AtomicU64::new(0);
static QC_QUORUM_WITHOUT_QC_TOTAL: AtomicU64 = AtomicU64::new(0);
static PREVOTE_TIMEOUT_TOTAL: AtomicU64 = AtomicU64::new(0);
const LANE_RELAY_ENVELOPES_CAP: usize = 64;
const VALIDATOR_CHECKPOINT_HISTORY_CAP: usize = 64;
const KEY_LIFECYCLE_HISTORY_CAP: usize = 128;
static VALIDATOR_CHECKPOINT_HISTORY: OnceLock<Mutex<VecDeque<ValidatorSetCheckpoint>>> =
    OnceLock::new();
static COMMIT_CERT_HISTORY: OnceLock<Mutex<VecDeque<Qc>>> = OnceLock::new();
static PRECOMMIT_SIGNER_HISTORY: OnceLock<Mutex<VecDeque<PrecommitSignerRecord>>> = OnceLock::new();
static KEY_LIFECYCLE_HISTORY: OnceLock<Mutex<VecDeque<ConsensusKeyRecord>>> = OnceLock::new();
const NPOS_ELECTION_HISTORY_CAP: usize = 32;
static NPOS_ELECTION_HISTORY: OnceLock<Mutex<VecDeque<ValidatorElectionOutcome>>> = OnceLock::new();
const DEFAULT_COMMIT_CERT_HISTORY_CAP: usize = 512;
static COMMIT_CERT_HISTORY_CAP: OnceLock<AtomicUsize> = OnceLock::new();

fn commit_cert_history_cap() -> usize {
    COMMIT_CERT_HISTORY_CAP
        .get_or_init(|| AtomicUsize::new(DEFAULT_COMMIT_CERT_HISTORY_CAP))
        .load(StdOrdering::Relaxed)
}

/// Set the commit certificate history cap at runtime (should be called during initialization).
pub fn set_commit_cert_history_cap(cap: usize) {
    COMMIT_CERT_HISTORY_CAP
        .get_or_init(|| AtomicUsize::new(DEFAULT_COMMIT_CERT_HISTORY_CAP))
        .store(cap, StdOrdering::Relaxed);
}

/// Actor responsible for paying a Nexus fee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NexusFeePayer {
    /// Transaction authority paid the fee.
    Payer,
    /// A sponsor covered the fee.
    Sponsor,
}

/// Aggregated Nexus fee debit outcomes for status/telemetry surfacing.
#[derive(Clone, Debug, Default)]
pub struct NexusFeeSnapshot {
    /// Total fee debits applied successfully.
    pub charged_total: u64,
    /// Successful debits that used the payer account.
    pub charged_via_payer_total: u64,
    /// Successful debits that used a sponsor account.
    pub charged_via_sponsor_total: u64,
    /// Rejections because sponsorship was disabled.
    pub sponsor_disabled_total: u64,
    /// Rejections because the sponsor did not authorize the payer.
    pub sponsor_unauthorized_total: u64,
    /// Rejections because the fee exceeded `sponsor_max_fee`.
    pub sponsor_cap_exceeded_total: u64,
    /// Failures due to config/asset parsing errors.
    pub config_errors_total: u64,
    /// Failures while executing the fee transfer.
    pub transfer_failures_total: u64,
    /// Last attempted fee amount (base units) if available.
    pub last_amount: Option<u128>,
    /// Asset definition id used for the last attempt.
    pub last_asset_id: Option<String>,
    /// Payer classification for the last attempt.
    pub last_payer: Option<NexusFeePayer>,
    /// Account id string for the last attempt.
    pub last_payer_id: Option<String>,
    /// Most recent error message (if any).
    pub last_error: Option<String>,
}

/// Outcome emitted when attempting to debit Nexus fees.
#[derive(Clone, Debug)]
pub enum NexusFeeEvent {
    /// Fee charged successfully.
    Charged {
        /// Whether payer or sponsor covered the fee.
        payer_kind: NexusFeePayer,
        /// Account id that paid.
        payer_id: String,
        /// Amount charged (base units).
        amount: u128,
        /// Asset definition id string.
        asset_id: String,
    },
    /// Sponsorship was disabled.
    SponsorDisabled {
        /// Account attempting to sponsor the fee.
        payer_id: String,
    },
    /// Sponsor did not authorize the payer.
    SponsorUnauthorized {
        /// Sponsor account that was requested.
        sponsor_id: String,
        /// Transaction authority that attempted to use the sponsor.
        authority_id: String,
    },
    /// Sponsorship exceeded configured cap.
    SponsorCapExceeded {
        /// Account that attempted to sponsor.
        payer_id: String,
        /// Maximum allowed fee in base units.
        max_fee: u64,
        /// Attempted fee in base units.
        attempted_fee: u128,
    },
    /// Fee transfer failed to apply.
    TransferFailed {
        /// Payer classification.
        payer_kind: NexusFeePayer,
        /// Account that attempted to pay.
        payer_id: String,
        /// Amount attempted (base units).
        amount: u128,
        /// Asset definition id string.
        asset_id: String,
        /// Human-readable reason.
        reason: String,
    },
    /// Fee failed due to invalid configuration.
    ConfigInvalid {
        /// Human-readable error cause.
        reason: String,
    },
}

/// Per-lane staking summary for Nexus public lanes.
#[derive(Clone, Debug)]
pub struct NexusStakingLaneSnapshot {
    /// Lane identifier.
    pub lane_id: LaneId,
    /// Total bonded stake recorded.
    pub bonded: Numeric,
    /// Total pending-unbond stake recorded.
    pub pending_unbond: Numeric,
    /// Total slashes applied.
    pub slash_total: u64,
}

impl Default for NexusStakingLaneSnapshot {
    fn default() -> Self {
        Self {
            lane_id: LaneId::new(0),
            bonded: Numeric::zero(),
            pending_unbond: Numeric::zero(),
            slash_total: 0,
        }
    }
}

/// Aggregated Nexus staking snapshot (all lanes).
#[derive(Clone, Debug, Default)]
pub struct NexusStakingSnapshot {
    /// Per-lane staking summaries.
    pub lanes: Vec<NexusStakingLaneSnapshot>,
}

// Whether this node has been removed from the world state (peer unregistered).
static LOCAL_REMOVED_FROM_WORLD: AtomicBool = AtomicBool::new(false);

/// Record whether the local peer is present in the world state.
pub fn set_local_removed_from_world(removed: bool) {
    LOCAL_REMOVED_FROM_WORLD.store(removed, Ordering::Relaxed);
}

/// Check if the local peer has been removed from the world state.
pub fn local_peer_removed() -> bool {
    LOCAL_REMOVED_FROM_WORLD.load(Ordering::Relaxed)
}

/// Outcome classification for settlement telemetry snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettlementOutcomeKind {
    /// Settlement executed successfully.
    Success,
    /// Settlement execution failed (preconditions or execution error).
    Failure,
}

impl SettlementOutcomeKind {
    /// String label used for metrics and status JSON.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            SettlementOutcomeKind::Success => "success",
            SettlementOutcomeKind::Failure => "failure",
        }
    }
}

/// Aggregated settlement telemetry counters captured by the local peer.
#[derive(Clone, Debug, Default)]
pub struct SettlementStatusSnapshot {
    /// Delivery-versus-payment telemetry snapshot.
    pub dvp: DvpSettlementSnapshot,
    /// Payment-versus-payment telemetry snapshot.
    pub pvp: PvpSettlementSnapshot,
}

/// Derived counters and the last event snapshot for `DvP` settlements.
#[derive(Clone, Debug, Default)]
pub struct DvpSettlementSnapshot {
    /// Successful `DvP` executions observed locally.
    pub success_total: u64,
    /// Failed `DvP` executions observed locally.
    pub failure_total: u64,
    /// Final-state counter map keyed by `none|delivery_only|payment_only|both`.
    pub final_state_totals: BTreeMap<String, u64>,
    /// Failure reason counters keyed by telemetry label.
    pub failure_reasons: BTreeMap<String, u64>,
    /// Last observed `DvP` settlement event.
    pub last_event: Option<DvpSettlementEventSnapshot>,
}

/// Telemetry snapshot describing a single `DvP` settlement event.
#[derive(Clone, Debug)]
pub struct DvpSettlementEventSnapshot {
    /// Milliseconds since Unix epoch when the event was recorded.
    pub observed_at_ms: u64,
    /// Settlement identifier provided by the instruction.
    pub settlement_id: Option<String>,
    /// Execution order recorded for the settlement plan.
    pub plan_order: SettlementExecutionOrder,
    /// Atomicity policy applied to the settlement plan.
    pub plan_atomicity: SettlementAtomicity,
    /// Outcome classification (success/failure).
    pub outcome: SettlementOutcomeKind,
    /// Failure reason label when outcome is failure.
    pub failure_reason: Option<String>,
    /// Final state label (`none`, `delivery_only`, `payment_only`, `both`).
    pub final_state_label: String,
    /// Whether the delivery leg remained committed after execution.
    pub delivery_committed: bool,
    /// Whether the payment leg remained committed after execution.
    pub payment_committed: bool,
}

impl Default for DvpSettlementEventSnapshot {
    fn default() -> Self {
        Self {
            observed_at_ms: 0,
            settlement_id: None,
            plan_order: SettlementExecutionOrder::DeliveryThenPayment,
            plan_atomicity: SettlementAtomicity::AllOrNothing,
            outcome: SettlementOutcomeKind::Success,
            failure_reason: None,
            final_state_label: "none".to_string(),
            delivery_committed: false,
            payment_committed: false,
        }
    }
}

/// Derived counters and the last event snapshot for `PvP` settlements.
#[derive(Clone, Debug, Default)]
pub struct PvpSettlementSnapshot {
    /// Successful `PvP` executions observed locally.
    pub success_total: u64,
    /// Failed `PvP` executions observed locally.
    pub failure_total: u64,
    /// Final-state counter map keyed by `none|primary_only|counter_only|both`.
    pub final_state_totals: BTreeMap<String, u64>,
    /// Failure reason counters keyed by telemetry label.
    pub failure_reasons: BTreeMap<String, u64>,
    /// Last observed `PvP` settlement event.
    pub last_event: Option<PvpSettlementEventSnapshot>,
}

/// Telemetry snapshot describing a single `PvP` settlement event.
#[derive(Clone, Debug)]
pub struct PvpSettlementEventSnapshot {
    /// Milliseconds since Unix epoch when the event was recorded.
    pub observed_at_ms: u64,
    /// Settlement identifier provided by the instruction.
    pub settlement_id: Option<String>,
    /// Execution order recorded for the settlement plan.
    pub plan_order: SettlementExecutionOrder,
    /// Atomicity policy applied to the settlement plan.
    pub plan_atomicity: SettlementAtomicity,
    /// Outcome classification (success/failure).
    pub outcome: SettlementOutcomeKind,
    /// Failure reason label when outcome is failure.
    pub failure_reason: Option<String>,
    /// Final state label (`none`, `primary_only`, `counter_only`, `both`).
    pub final_state_label: String,
    /// Whether the primary leg remained committed after execution.
    pub primary_committed: bool,
    /// Whether the counter leg remained committed after execution.
    pub counter_committed: bool,
    /// Observed FX window in milliseconds (time between committed legs).
    pub fx_window_ms: Option<u64>,
}

impl Default for PvpSettlementEventSnapshot {
    fn default() -> Self {
        Self {
            observed_at_ms: 0,
            settlement_id: None,
            plan_order: SettlementExecutionOrder::DeliveryThenPayment,
            plan_atomicity: SettlementAtomicity::AllOrNothing,
            outcome: SettlementOutcomeKind::Success,
            failure_reason: None,
            final_state_label: "none".to_string(),
            primary_committed: false,
            counter_committed: false,
            fx_window_ms: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct SettlementStatusState {
    dvp: DvpSettlementSnapshot,
    pvp: PvpSettlementSnapshot,
}

fn settlement_status_slot() -> &'static Mutex<SettlementStatusState> {
    SETTLEMENT_STATUS.get_or_init(|| Mutex::new(SettlementStatusState::default()))
}

/// Update payload produced when a `DvP` settlement completes.
#[derive(Clone, Debug)]
pub struct DvpSettlementEventUpdate {
    /// Milliseconds since Unix epoch when the event was recorded.
    pub observed_at_ms: u64,
    /// Settlement identifier provided by the instruction (if any).
    pub settlement_id: Option<String>,
    /// Execution order recorded for the settlement plan.
    pub plan_order: SettlementExecutionOrder,
    /// Atomicity policy applied to the settlement plan.
    pub plan_atomicity: SettlementAtomicity,
    /// Outcome classification (success or failure).
    pub outcome: SettlementOutcomeKind,
    /// Failure reason label when outcome is failure.
    pub failure_reason: Option<String>,
    /// Final state label (`none`, `delivery_only`, `payment_only`, or `both`).
    pub final_state_label: String,
    /// Whether the delivery leg remained committed after execution.
    pub delivery_committed: bool,
    /// Whether the payment leg remained committed after execution.
    pub payment_committed: bool,
}

/// Update payload produced when a `PvP` settlement completes.
#[derive(Clone, Debug)]
pub struct PvpSettlementEventUpdate {
    /// Milliseconds since Unix epoch when the event was recorded.
    pub observed_at_ms: u64,
    /// Settlement identifier provided by the instruction (if any).
    pub settlement_id: Option<String>,
    /// Execution order recorded for the settlement plan.
    pub plan_order: SettlementExecutionOrder,
    /// Atomicity policy applied to the settlement plan.
    pub plan_atomicity: SettlementAtomicity,
    /// Outcome classification (success or failure).
    pub outcome: SettlementOutcomeKind,
    /// Failure reason label when outcome is failure.
    pub failure_reason: Option<String>,
    /// Final state label (`none`, `primary_only`, `counter_only`, or `both`).
    pub final_state_label: String,
    /// Whether the primary leg remained committed after execution.
    pub primary_committed: bool,
    /// Whether the counter leg remained committed after execution.
    pub counter_committed: bool,
    /// Observed FX window in milliseconds (time between committed legs).
    pub fx_window_ms: Option<u64>,
}

/// Record a `DvP` settlement telemetry update.
pub fn record_dvp_settlement_event(update: DvpSettlementEventUpdate) {
    let mut guard = settlement_status_slot()
        .lock()
        .expect("settlement status mutex poisoned");
    let entry = &mut guard.dvp;
    match update.outcome {
        SettlementOutcomeKind::Success => {
            entry.success_total = entry.success_total.saturating_add(1)
        }
        SettlementOutcomeKind::Failure => {
            entry.failure_total = entry.failure_total.saturating_add(1)
        }
    }
    *entry
        .final_state_totals
        .entry(update.final_state_label.clone())
        .or_default() += 1;
    if let Some(reason) = update.failure_reason.clone() {
        *entry.failure_reasons.entry(reason).or_default() += 1;
    }
    entry.last_event = Some(DvpSettlementEventSnapshot {
        observed_at_ms: update.observed_at_ms,
        settlement_id: update.settlement_id,
        plan_order: update.plan_order,
        plan_atomicity: update.plan_atomicity,
        outcome: update.outcome,
        failure_reason: update.failure_reason,
        final_state_label: update.final_state_label,
        delivery_committed: update.delivery_committed,
        payment_committed: update.payment_committed,
    });
}

/// Record a `PvP` settlement telemetry update.
pub fn record_pvp_settlement_event(update: PvpSettlementEventUpdate) {
    let mut guard = settlement_status_slot()
        .lock()
        .expect("settlement status mutex poisoned");
    let entry = &mut guard.pvp;
    match update.outcome {
        SettlementOutcomeKind::Success => {
            entry.success_total = entry.success_total.saturating_add(1)
        }
        SettlementOutcomeKind::Failure => {
            entry.failure_total = entry.failure_total.saturating_add(1)
        }
    }
    *entry
        .final_state_totals
        .entry(update.final_state_label.clone())
        .or_default() += 1;
    if let Some(reason) = update.failure_reason.clone() {
        *entry.failure_reasons.entry(reason).or_default() += 1;
    }
    entry.last_event = Some(PvpSettlementEventSnapshot {
        observed_at_ms: update.observed_at_ms,
        settlement_id: update.settlement_id,
        plan_order: update.plan_order,
        plan_atomicity: update.plan_atomicity,
        outcome: update.outcome,
        failure_reason: update.failure_reason,
        final_state_label: update.final_state_label,
        primary_committed: update.primary_committed,
        counter_committed: update.counter_committed,
        fx_window_ms: update.fx_window_ms,
    });
}

/// Read-only snapshot of settlement telemetry state.
pub fn settlement_snapshot() -> SettlementStatusSnapshot {
    let guard = settlement_status_slot()
        .lock()
        .expect("settlement status mutex poisoned");
    SettlementStatusSnapshot {
        dvp: guard.dvp.clone(),
        pvp: guard.pvp.clone(),
    }
}

fn rbc_store_evictions_slot() -> &'static Mutex<VecDeque<RbcEvictedSession>> {
    RBC_STORE_RECENT_EVICTIONS
        .get_or_init(|| Mutex::new(VecDeque::with_capacity(RBC_STORE_RECENT_EVICTIONS_CAP)))
}
static COLLECTORS_TARGETED_CURRENT: AtomicU64 = AtomicU64::new(0);
static COLLECTORS_TARGETED_LAST_COMMIT: AtomicU64 = AtomicU64::new(0);
static REDUNDANT_SEND_TOTAL: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_DEPTH: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_CAPACITY: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_SATURATED: AtomicBool = AtomicBool::new(false);
static WORKER_QUEUE_VOTE_DEPTH: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BLOCK_PAYLOAD_DEPTH: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_RBC_CHUNK_DEPTH: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BLOCK_DEPTH: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_CONSENSUS_DEPTH: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_LANE_RELAY_DEPTH: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BACKGROUND_DEPTH: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_VOTE_BLOCKED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BLOCK_PAYLOAD_BLOCKED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_RBC_CHUNK_BLOCKED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BLOCK_BLOCKED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_CONSENSUS_BLOCKED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_LANE_RELAY_BLOCKED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BACKGROUND_BLOCKED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_VOTE_BLOCKED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BLOCK_PAYLOAD_BLOCKED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_RBC_CHUNK_BLOCKED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BLOCK_BLOCKED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_CONSENSUS_BLOCKED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_LANE_RELAY_BLOCKED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BACKGROUND_BLOCKED_MS_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_VOTE_BLOCKED_MAX_MS: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BLOCK_PAYLOAD_BLOCKED_MAX_MS: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_RBC_CHUNK_BLOCKED_MAX_MS: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BLOCK_BLOCKED_MAX_MS: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_CONSENSUS_BLOCKED_MAX_MS: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_LANE_RELAY_BLOCKED_MAX_MS: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BACKGROUND_BLOCKED_MAX_MS: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_VOTE_DROPPED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BLOCK_PAYLOAD_DROPPED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_RBC_CHUNK_DROPPED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BLOCK_DROPPED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_CONSENSUS_DROPPED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_LANE_RELAY_DROPPED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_QUEUE_BACKGROUND_DROPPED_TOTAL: AtomicU64 = AtomicU64::new(0);
static WORKER_STAGE_ID: AtomicU64 = AtomicU64::new(0);
static WORKER_STAGE_STARTED_MS: AtomicU64 = AtomicU64::new(0);
static WORKER_LAST_ITERATION_MS: AtomicU64 = AtomicU64::new(0);
static EPOCH_LENGTH_BLOCKS: AtomicU64 = AtomicU64::new(0);
static EPOCH_COMMIT_DEADLINE_OFFSET: AtomicU64 = AtomicU64::new(0);
static EPOCH_REVEAL_DEADLINE_OFFSET: AtomicU64 = AtomicU64::new(0);
static VRF_PENALTY_EPOCH: AtomicU64 = AtomicU64::new(0);
static VRF_NON_REVEAL_TOTAL: AtomicU64 = AtomicU64::new(0);
static VRF_NO_PARTICIPATION_TOTAL: AtomicU64 = AtomicU64::new(0);
static VRF_LATE_REVEALS_TOTAL: AtomicU64 = AtomicU64::new(0);
static CONSENSUS_PENALTIES_APPLIED_TOTAL: AtomicU64 = AtomicU64::new(0);
static CONSENSUS_PENALTIES_PENDING: AtomicU64 = AtomicU64::new(0);
static VRF_PENALTIES_APPLIED_TOTAL: AtomicU64 = AtomicU64::new(0);
static VRF_PENALTIES_PENDING: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct AvailabilityStats {
    total_votes: u64,
    per_peer: BTreeMap<PeerId, CollectorEntry>,
}

#[derive(Clone)]
struct CollectorEntry {
    idx: u64,
    votes: u64,
}

fn availability_slot() -> &'static Mutex<AvailabilityStats> {
    AVAILABILITY_STATS.get_or_init(|| Mutex::new(AvailabilityStats::default()))
}

fn qc_latency_slot() -> &'static Mutex<BTreeMap<&'static str, u64>> {
    QC_LATENCY_MS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn rbc_backlog_slot() -> &'static Mutex<RbcBacklogSnapshot> {
    RBC_BACKLOG.get_or_init(|| Mutex::new(RbcBacklogSnapshot::default()))
}

fn pending_rbc_slot() -> &'static Mutex<PendingRbcSnapshot> {
    PENDING_RBC_STATE.get_or_init(|| Mutex::new(PendingRbcSnapshot::default()))
}

/// Snapshot entry describing availability votes ingested by a collector.
#[derive(Clone, Debug)]
pub struct AvailabilityCollectorSnapshot {
    /// Collector topology index.
    pub collector_idx: u64,
    /// Collector peer identifier.
    pub peer: PeerId,
    /// Number of availability votes ingested by this collector.
    pub votes_ingested: u64,
}

/// Aggregated availability vote ingestion snapshot for `/v1/sumeragi/telemetry`.
#[derive(Clone, Debug, Default)]
pub struct AvailabilitySnapshot {
    /// Total availability votes ingested by this node.
    pub total: u64,
    /// Per-collector vote counts keyed by topology index and peer id.
    pub collectors: Vec<AvailabilityCollectorSnapshot>,
}

/// Aggregated RBC backlog metrics snapshot for `/v1/sumeragi/telemetry`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RbcBacklogSnapshot {
    /// Total missing chunks across active sessions.
    pub total_missing_chunks: u64,
    /// Maximum missing chunks within any single session.
    pub max_missing_chunks: u64,
    /// Number of sessions that have not yet delivered.
    pub pending_sessions: u64,
}

/// Pending (pre-INIT) RBC stash entry.
#[derive(Copy, Clone, Debug)]
pub struct PendingRbcEntrySnapshot {
    /// Block hash associated with the pending session.
    pub block_hash: HashOf<BlockHeader>,
    /// Block height for the pending session.
    pub height: u64,
    /// View index for the pending session.
    pub view: u64,
    /// Number of chunk frames currently buffered.
    pub chunks: u64,
    /// Total chunk payload bytes currently buffered.
    pub bytes: u64,
    /// READY frames currently buffered.
    pub ready: u64,
    /// DELIVER frames currently buffered.
    pub deliver: u64,
    /// Chunk frames dropped for this session due to caps.
    pub dropped_chunks: u64,
    /// Chunk payload bytes dropped for this session due to caps.
    pub dropped_bytes: u64,
    /// READY frames dropped for this session due to caps.
    pub dropped_ready: u64,
    /// DELIVER frames dropped for this session due to caps.
    pub dropped_deliver: u64,
    /// Age in milliseconds since the first pending message was recorded.
    pub age_ms: u64,
}

impl Default for PendingRbcEntrySnapshot {
    fn default() -> Self {
        Self {
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH])),
            height: 0,
            view: 0,
            chunks: 0,
            bytes: 0,
            ready: 0,
            deliver: 0,
            dropped_chunks: 0,
            dropped_bytes: 0,
            dropped_ready: 0,
            dropped_deliver: 0,
            age_ms: 0,
        }
    }
}

/// Aggregated pending RBC stash metrics.
#[derive(Clone, Debug, Default)]
pub struct PendingRbcSnapshot {
    /// Current pending sessions awaiting INIT.
    pub sessions: u64,
    /// Maximum pending sessions retained (hard cap).
    pub session_cap: u64,
    /// Aggregate pending chunk frames across sessions.
    pub chunks: u64,
    /// Aggregate pending chunk payload bytes across sessions.
    pub bytes: u64,
    /// Configured per-session chunk cap.
    pub max_chunks_per_session: u64,
    /// Configured per-session byte cap.
    pub max_bytes_per_session: u64,
    /// Configured TTL (milliseconds) before pending entries expire.
    pub ttl_ms: u64,
    /// Total pending frames dropped due to caps.
    pub drops_total: u64,
    /// Total pending frames dropped due to cap enforcement.
    pub drops_cap_total: u64,
    /// Aggregate payload/signature bytes dropped due to caps.
    pub drops_cap_bytes_total: u64,
    /// Total pending frames dropped due to TTL expiry.
    pub drops_ttl_total: u64,
    /// Aggregate payload/signature bytes dropped due to TTL expiry.
    pub drops_ttl_bytes_total: u64,
    /// Total pending bytes dropped across all reasons.
    pub drops_bytes_total: u64,
    /// Total pending sessions evicted (TTL expiry or stash-cap eviction).
    pub evicted_total: u64,
    /// Total READY frames stashed before processing.
    pub stash_ready_total: u64,
    /// READY frames stashed because INIT has not arrived yet.
    pub stash_ready_init_missing_total: u64,
    /// READY frames stashed because the commit roster is missing.
    pub stash_ready_roster_missing_total: u64,
    /// READY frames stashed because the commit roster hash mismatched.
    pub stash_ready_roster_hash_mismatch_total: u64,
    /// READY frames stashed while the commit roster is unverified.
    pub stash_ready_roster_unverified_total: u64,
    /// Total DELIVER frames stashed before processing.
    pub stash_deliver_total: u64,
    /// DELIVER frames stashed because INIT has not arrived yet.
    pub stash_deliver_init_missing_total: u64,
    /// DELIVER frames stashed because the commit roster is missing.
    pub stash_deliver_roster_missing_total: u64,
    /// DELIVER frames stashed because the commit roster hash mismatched.
    pub stash_deliver_roster_hash_mismatch_total: u64,
    /// DELIVER frames stashed while the commit roster is unverified.
    pub stash_deliver_roster_unverified_total: u64,
    /// Chunk frames stashed before INIT arrives.
    pub stash_chunk_total: u64,
    /// Pending sessions with per-session drop counters.
    pub entries: Vec<PendingRbcEntrySnapshot>,
}

/// Kind of pending RBC message stashed before processing.
#[derive(Clone, Copy, Debug)]
pub enum PendingRbcStashKind {
    /// READY messages stashed before processing.
    Ready,
    /// DELIVER messages stashed before processing.
    Deliver,
    /// Chunk messages stashed before INIT arrives.
    Chunk,
}

/// Reason a pending RBC message was stashed.
#[derive(Clone, Copy, Debug)]
pub enum PendingRbcStashReason {
    /// INIT has not arrived yet for this session.
    InitMissing,
    /// Commit roster is missing for the target height/view.
    RosterMissing,
    /// Commit roster hash mismatched the message metadata.
    RosterHashMismatch,
    /// Commit roster is present but not yet verified.
    RosterUnverified,
}

/// Per-lane execution summary for operator dashboards.
#[derive(Clone, Copy, Debug, Default)]
pub struct LaneActivitySnapshot {
    /// Lane identifier (numeric).
    pub lane_id: u32,
    /// Transactions executed for this lane.
    pub tx_vertices: u64,
    /// Conflict edges among those transactions.
    pub tx_edges: u64,
    /// Overlay fragments executed for this lane.
    pub overlay_count: u64,
    /// Total overlay instructions executed for this lane.
    pub overlay_instr_total: u64,
    /// Total overlay bytes executed for this lane.
    pub overlay_bytes_total: u64,
    /// Approximate number of RBC chunks attributed to this lane.
    pub rbc_chunks: u64,
    /// Approximate total RBC payload bytes attributed to this lane.
    pub rbc_bytes_total: u64,
}

/// Summary of access-set sources used for IVM transactions in the latest block.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccessSetSourceSummary {
    /// Transactions using manifest-level access-set hints.
    pub manifest_hints: u64,
    /// Transactions using entrypoint-level access-set hints.
    pub entrypoint_hints: u64,
    /// Transactions derived from the dynamic prepass (merged sources).
    pub prepass_merge: u64,
    /// Transactions that fell back to the conservative global set.
    pub conservative_fallback: u64,
}

/// Per-dataspace execution summary for operator dashboards.
#[derive(Clone, Copy, Debug, Default)]
pub struct DataspaceActivitySnapshot {
    /// Owning lane identifier (numeric).
    pub lane_id: u32,
    /// Dataspace identifier.
    pub dataspace_id: u64,
    /// Transactions executed for this dataspace.
    pub tx_served: u64,
}

/// Aggregated per-lane RBC backlog snapshot for operator dashboards.
#[derive(Clone, Copy, Debug, Default, Encode, Decode, PartialEq, Eq)]
pub struct LaneRbcSnapshot {
    /// Lane identifier (numeric).
    pub lane_id: u32,
    /// Transactions contributing payload bytes in this lane across active sessions.
    pub tx_count: u64,
    /// Total RBC chunks attributed to this lane across active sessions.
    pub total_chunks: u64,
    /// RBC chunks still pending delivery for this lane across active sessions.
    pub pending_chunks: u64,
    /// Total RBC payload bytes attributed to this lane across active sessions.
    pub rbc_bytes_total: u64,
}

/// Aggregated per-dataspace RBC backlog snapshot for operator dashboards.
#[derive(Clone, Copy, Debug, Default, Encode, Decode, PartialEq, Eq)]
pub struct DataspaceRbcSnapshot {
    /// Owning lane identifier (numeric).
    pub lane_id: u32,
    /// Dataspace identifier (numeric).
    pub dataspace_id: u64,
    /// Transactions contributing payload bytes for this dataspace across active sessions.
    pub tx_count: u64,
    /// Total RBC chunks attributed to this dataspace across active sessions.
    pub total_chunks: u64,
    /// RBC chunks still pending delivery for this dataspace across active sessions.
    pub pending_chunks: u64,
    /// Total RBC payload bytes attributed to this dataspace across active sessions.
    pub rbc_bytes_total: u64,
}

/// Aggregated per-lane commitment summary for recently committed blocks.
#[derive(Clone, Copy, Debug)]
pub struct LaneCommitmentSnapshot {
    /// Block height associated with the commitment.
    pub block_height: u64,
    /// Lane identifier (numeric).
    pub lane_id: u32,
    /// Number of transactions routed to this lane in the block.
    pub tx_count: u64,
    /// Total RBC chunks attributed to this lane.
    pub total_chunks: u64,
    /// Total RBC payload bytes attributed to this lane.
    pub rbc_bytes_total: u64,
    /// Total TEU attributed to this lane.
    pub teu_total: u64,
    /// Block hash identifying the commitment.
    pub block_hash: HashOf<BlockHeader>,
}

/// Aggregated per-dataspace commitment summary for recently committed blocks.
#[derive(Clone, Copy, Debug)]
pub struct DataspaceCommitmentSnapshot {
    /// Block height associated with the commitment.
    pub block_height: u64,
    /// Lane identifier (numeric).
    pub lane_id: u32,
    /// Dataspace identifier (numeric).
    pub dataspace_id: u64,
    /// Number of transactions routed to this dataspace.
    pub tx_count: u64,
    /// Total RBC chunks attributed to this dataspace.
    pub total_chunks: u64,
    /// Total RBC payload bytes attributed to this dataspace.
    pub rbc_bytes_total: u64,
    /// Total TEU attributed to this dataspace.
    pub teu_total: u64,
    /// Block hash identifying the commitment.
    pub block_hash: HashOf<BlockHeader>,
}

/// Governance manifest snapshot for a lane.
#[derive(Clone, Debug, Default)]
pub struct LaneGovernanceSnapshot {
    /// Numeric lane identifier.
    pub lane_id: u32,
    /// Human-readable lane alias.
    pub alias: String,
    /// Dataspace identifier bound to the lane.
    pub dataspace_id: u64,
    /// Declarative visibility profile (`public` / `restricted`).
    pub visibility: String,
    /// Storage profile advertised for the lane.
    pub storage_profile: String,
    /// Governance module configured for the lane, if any.
    pub governance: Option<String>,
    /// Whether the lane requires a governance manifest.
    pub manifest_required: bool,
    /// Whether a manifest has been loaded and validated.
    pub manifest_ready: bool,
    /// Source path for the manifest (best-effort; operator visibility).
    pub manifest_path: Option<String>,
    /// Validator identifiers derived from the manifest.
    pub validator_ids: Vec<String>,
    /// Quorum threshold applied to the lane (if provided).
    pub quorum: Option<u32>,
    /// Protected namespaces enforced by the manifest.
    pub protected_namespaces: Vec<String>,
    /// Runtime-upgrade governance hook snapshot when configured.
    pub runtime_upgrade: Option<LaneRuntimeUpgradeHookSnapshot>,
    /// Privacy commitments advertised by the lane manifest.
    pub privacy_commitments: Vec<LanePrivacyCommitmentSnapshot>,
}

/// Snapshot of a privacy commitment registered for a lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LanePrivacyCommitmentSnapshot {
    /// Stable identifier assigned to the commitment.
    pub id: u16,
    /// Scheme-specific metadata captured at registry time.
    pub scheme: LanePrivacyCommitmentSchemeSnapshot,
}

/// Scheme metadata surfaced for observability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanePrivacyCommitmentSchemeSnapshot {
    /// Merkle-root commitment and audit-path depth budget.
    Merkle {
        /// Root hash that commits to the private dataset.
        root: [u8; 32],
        /// Maximum Merkle proof depth the lane operator promises to serve.
        max_depth: u8,
    },
    /// zk-SNARK circuit commitment exposing the hash bindings.
    Snark {
        /// Circuit identifier within the manifest's SNARK registry.
        circuit_id: u16,
        /// BLAKE3 digest of the verifying key used for audits.
        verifying_key_digest: [u8; 32],
        /// Hash of the public statement constrained by the circuit.
        statement_hash: [u8; 32],
        /// Hash of the proof artifact stored alongside the commitment.
        proof_hash: [u8; 32],
    },
}

impl From<&LanePrivacyCommitment> for LanePrivacyCommitmentSnapshot {
    fn from(commitment: &LanePrivacyCommitment) -> Self {
        let scheme = match commitment.scheme() {
            CommitmentScheme::Merkle(merkle) => LanePrivacyCommitmentSchemeSnapshot::Merkle {
                root: hash_of_bytes(*merkle.root()),
                max_depth: merkle.max_depth(),
            },
            CommitmentScheme::Snark(snark) => LanePrivacyCommitmentSchemeSnapshot::Snark {
                circuit_id: snark.circuit_id().get(),
                verifying_key_digest: *snark.verifying_key_digest(),
                statement_hash: *snark.statement_hash(),
                proof_hash: *snark.proof_hash(),
            },
        };
        Self {
            id: commitment.id().get(),
            scheme,
        }
    }
}

fn hash_of_bytes<T>(hash: HashOf<T>) -> [u8; 32] {
    let untyped: UntypedHash = hash.into();
    untyped.into()
}

/// Runtime-upgrade governance hook snapshot.
#[derive(Clone, Debug, Default)]
pub struct LaneRuntimeUpgradeHookSnapshot {
    /// Whether runtime-upgrade instructions are allowed.
    pub allow: bool,
    /// Whether runtime-upgrade instructions must include metadata.
    pub require_metadata: bool,
    /// Metadata key enforced by the manifest, if specified.
    pub metadata_key: Option<String>,
    /// Allowed metadata identifiers when an allowlist is configured.
    pub allowed_ids: Vec<String>,
}

/// Update the current leader index. Best-effort, for observability only.
pub fn set_leader_index(idx: u64) {
    LEADER_INDEX.store(idx, Ordering::Relaxed);
}

/// Update the current `HighestQC` tuple (height, view). Best-effort, for observability only.
pub fn set_highest_qc(height: u64, view: u64) {
    HIGHEST_QC_HEIGHT.store(height, Ordering::Relaxed);
    HIGHEST_QC_VIEW.store(view, Ordering::Relaxed);
}

/// Update the subject block hash for the current `HighestQC` (best-effort).
pub fn set_highest_qc_hash(hash: HashOf<BlockHeader>) {
    let h = UntypedHash::from(hash);
    let slot = HIGHEST_QC_HASH.get_or_init(|| Mutex::new(None));
    *slot.lock().unwrap() = Some(h);
}

/// Get the subject block hash for the current `HighestQC` if known.
pub fn highest_qc_hash() -> Option<HashOf<BlockHeader>> {
    let slot = HIGHEST_QC_HASH.get_or_init(|| Mutex::new(None));
    slot.lock()
        .unwrap()
        .as_ref()
        .map(Clone::clone)
        .map(HashOf::<BlockHeader>::from_untyped_unchecked)
}

fn set_locked_qc_hash(hash: Option<HashOf<BlockHeader>>) {
    let slot = LOCKED_QC_HASH.get_or_init(|| Mutex::new(None));
    *slot.lock().unwrap() = hash.map(UntypedHash::from);
}

fn locked_qc_hash() -> Option<HashOf<BlockHeader>> {
    let slot = LOCKED_QC_HASH.get_or_init(|| Mutex::new(None));
    slot.lock()
        .unwrap()
        .as_ref()
        .map(Clone::clone)
        .map(HashOf::<BlockHeader>::from_untyped_unchecked)
}

fn membership_view_hash() -> Option<[u8; 32]> {
    if !MEMBERSHIP_HASH_SET.load(Ordering::Relaxed) {
        return None;
    }
    MEMBERSHIP_VIEW_HASH
        .get()
        .and_then(|slot| slot.lock().ok().map(|hash| *hash))
}

/// Snapshot the current membership view hash for mismatch detection.
pub fn membership_snapshot() -> Option<SumeragiMembershipStatus> {
    let view_hash = membership_view_hash()?;
    Some(SumeragiMembershipStatus {
        height: MEMBERSHIP_HEIGHT.load(Ordering::Relaxed),
        view: MEMBERSHIP_VIEW.load(Ordering::Relaxed),
        epoch: MEMBERSHIP_EPOCH.load(Ordering::Relaxed),
        view_hash: Some(view_hash),
    })
}

fn membership_mismatch_registry()
-> Option<std::sync::MutexGuard<'static, MembershipMismatchRegistry>> {
    MEMBERSHIP_MISMATCH_REGISTRY
        .get_or_init(|| Mutex::new(MembershipMismatchRegistry::default()))
        .lock()
        .ok()
}

/// Record a membership hash mismatch for the given peer and return the consecutive count.
pub fn record_membership_mismatch(
    peer: PeerId,
    height: u64,
    view: u64,
    epoch: u64,
    local_hash: [u8; 32],
    remote_hash: [u8; 32],
) -> u32 {
    let now_ms = now_timestamp_ms();
    let Some(mut registry) = membership_mismatch_registry() else {
        return 0;
    };
    let context = MembershipMismatchContext {
        peer,
        height,
        view,
        epoch,
        local_hash,
        remote_hash,
        timestamp_ms: now_ms,
    };
    let consecutive = {
        let entry = registry
            .entries
            .entry(context.peer.clone())
            .or_insert_with(MembershipMismatchState::default);
        entry.active = true;
        entry.consecutive = entry.consecutive.saturating_add(1).max(1);
        entry.last_mismatch = Some(context.clone());
        entry.consecutive
    };
    registry.last = Some(context);
    consecutive
}

/// Clear any recorded membership mismatch for the given peer.
pub fn clear_membership_mismatch(peer: &PeerId) {
    let Some(mut registry) = membership_mismatch_registry() else {
        return;
    };
    registry.entries.remove(peer);
}

/// Return the consecutive mismatch count for the given peer.
pub fn membership_mismatch_consecutive(peer: &PeerId) -> u32 {
    membership_mismatch_registry()
        .and_then(|registry| registry.entries.get(peer).map(|entry| entry.consecutive))
        .unwrap_or(0)
}

/// Snapshot the current membership mismatch registry.
pub fn membership_mismatch_snapshot() -> MembershipMismatchSnapshot {
    let Some(registry) = membership_mismatch_registry() else {
        return MembershipMismatchSnapshot::default();
    };
    let active_peers = registry
        .entries
        .iter()
        .filter(|(_, entry)| entry.active)
        .map(|(peer, _)| peer.clone())
        .collect();
    let last = registry.last.clone();
    MembershipMismatchSnapshot {
        active_peers,
        last_peer: last.as_ref().map(|ctx| ctx.peer.clone()),
        last_height: last.as_ref().map_or(0, |ctx| ctx.height),
        last_view: last.as_ref().map_or(0, |ctx| ctx.view),
        last_epoch: last.as_ref().map_or(0, |ctx| ctx.epoch),
        last_local_hash: last.as_ref().map(|ctx| ctx.local_hash),
        last_remote_hash: last.as_ref().map(|ctx| ctx.remote_hash),
        last_timestamp_ms: last.as_ref().map_or(0, |ctx| ctx.timestamp_ms),
    }
}

fn rbc_mismatch_registry()
-> Option<std::sync::MutexGuard<'static, BTreeMap<PeerId, RbcMismatchCounts>>> {
    RBC_MISMATCH_REGISTRY
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .ok()
}

/// Record an RBC payload mismatch attributed to the given peer.
pub fn record_rbc_mismatch(peer: &PeerId, kind: RbcMismatchKind) {
    let now_ms = now_timestamp_ms();
    let Some(mut registry) = rbc_mismatch_registry() else {
        return;
    };
    let entry = registry.entry(peer.clone()).or_default();
    entry.last_timestamp_ms = now_ms;
    match kind {
        RbcMismatchKind::ChunkDigest => {
            entry.chunk_digest_mismatch_total = entry.chunk_digest_mismatch_total.saturating_add(1);
        }
        RbcMismatchKind::PayloadHash => {
            entry.payload_hash_mismatch_total = entry.payload_hash_mismatch_total.saturating_add(1);
        }
        RbcMismatchKind::ChunkRoot => {
            entry.chunk_root_mismatch_total = entry.chunk_root_mismatch_total.saturating_add(1);
        }
    }
}

/// Snapshot the per-peer RBC mismatch registry.
pub fn rbc_mismatch_snapshot() -> RbcMismatchSnapshot {
    let Some(registry) = rbc_mismatch_registry() else {
        return RbcMismatchSnapshot::default();
    };
    let entries = registry
        .iter()
        .map(|(peer, entry)| RbcMismatchEntry {
            peer_id: peer.clone(),
            chunk_digest_mismatch_total: entry.chunk_digest_mismatch_total,
            payload_hash_mismatch_total: entry.payload_hash_mismatch_total,
            chunk_root_mismatch_total: entry.chunk_root_mismatch_total,
            last_timestamp_ms: entry.last_timestamp_ms,
        })
        .collect();
    RbcMismatchSnapshot { entries }
}

#[cfg(test)]
/// Reset the RBC mismatch registry for unit tests.
pub fn reset_rbc_mismatch_for_tests() {
    if let Some(mut registry) = rbc_mismatch_registry() {
        registry.clear();
    }
}

#[cfg(test)]
/// Reset vote validation drop history for unit tests.
pub fn reset_vote_validation_drops_for_tests() {
    if let Some(slot) = VOTE_VALIDATION_DROPS.get() {
        let mut guard = slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.clear();
    }
    if let Some(mut registry) = vote_validation_drop_peer_registry() {
        registry.clear();
    }
    VOTE_VALIDATION_DROPS_TOTAL.store(0, Ordering::Relaxed);
}

#[cfg(test)]
/// Reset the membership mismatch registry for unit tests.
pub fn reset_membership_mismatch_for_tests() {
    if let Some(mut registry) = membership_mismatch_registry() {
        registry.entries.clear();
        registry.last = None;
    }
}

#[cfg(test)]
/// Reset the membership snapshot counters for unit tests.
pub fn reset_membership_snapshot_for_tests() {
    MEMBERSHIP_HEIGHT.store(0, Ordering::Relaxed);
    MEMBERSHIP_VIEW.store(0, Ordering::Relaxed);
    MEMBERSHIP_EPOCH.store(0, Ordering::Relaxed);
    MEMBERSHIP_HASH_SET.store(false, Ordering::Relaxed);
    if let Some(slot) = MEMBERSHIP_VIEW_HASH.get() {
        if let Ok(mut guard) = slot.lock() {
            *guard = [0u8; 32];
        }
    }
}

/// Set PRF context (seed/height/view) used for leader selection (best-effort).
pub fn set_prf_context(seed: [u8; 32], height: u64, view: u64) {
    PRF_HEIGHT.store(height, Ordering::Relaxed);
    PRF_VIEW.store(view, Ordering::Relaxed);
    let slot = PRF_SEED.get_or_init(|| Mutex::new(None));
    *slot.lock().unwrap() = Some(seed);
}

/// Record the current and staged consensus mode tags and activation height (best-effort).
pub fn set_mode_tags(
    mode_tag: &str,
    staged_mode_tag: Option<&str>,
    staged_mode_activation_height: Option<u64>,
) {
    let mode_slot = MODE_TAG.get_or_init(|| Mutex::new(String::new()));
    *mode_slot.lock().unwrap() = mode_tag.to_string();
    let staged_slot = STAGED_MODE_TAG.get_or_init(|| Mutex::new(None));
    *staged_slot.lock().unwrap() = staged_mode_tag.map(ToOwned::to_owned);
    let activation_slot = STAGED_MODE_ACTIVATION_HEIGHT.get_or_init(|| Mutex::new(None));
    *activation_slot.lock().unwrap() = staged_mode_activation_height;
}

/// Record the number of blocks since a staged mode activation height was reached without a flip.
/// `None` resets the lag to an unset state.
pub fn set_mode_activation_lag(lag_blocks: Option<u64>) {
    let slot = MODE_ACTIVATION_LAG_BLOCKS.get_or_init(|| Mutex::new(None));
    *slot.lock().unwrap() = lag_blocks;
}

/// Record whether runtime mode flips are currently allowed by configuration.
pub fn set_mode_flip_kill_switch(enabled: bool) {
    MODE_FLIP_KILL_SWITCH.store(enabled, Ordering::Relaxed);
}

/// Whether the last recorded flip attempt was blocked.
pub fn mode_flip_blocked() -> bool {
    MODE_FLIP_BLOCKED.load(Ordering::Relaxed)
}

/// Clear any recorded block state for mode flips.
pub fn clear_mode_flip_blocked() {
    MODE_FLIP_BLOCKED.store(false, Ordering::Relaxed);
}

fn set_last_flip_error(reason: Option<String>) {
    let slot = MODE_LAST_FLIP_ERROR.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = reason;
    }
}

fn set_last_flip_timestamp(now_ms: u64) {
    MODE_LAST_FLIP_TS_MS.store(now_ms, Ordering::Relaxed);
    MODE_LAST_FLIP_TS_SET.store(true, Ordering::Relaxed);
}

/// Record a successful runtime consensus mode flip.
pub fn note_mode_flip_success(now_ms: u64) {
    MODE_FLIP_SUCCESS_TOTAL.fetch_add(1, Ordering::Relaxed);
    MODE_FLIP_BLOCKED.store(false, Ordering::Relaxed);
    set_last_flip_timestamp(now_ms);
    set_last_flip_error(None);
}

/// Record a failed attempt to flip consensus mode along with the reason.
pub fn note_mode_flip_failure(reason: &str, now_ms: u64) {
    MODE_FLIP_FAIL_TOTAL.fetch_add(1, Ordering::Relaxed);
    MODE_FLIP_BLOCKED.store(false, Ordering::Relaxed);
    set_last_flip_timestamp(now_ms);
    set_last_flip_error(Some(reason.to_owned()));
}

/// Record that a flip was blocked (for example by a kill switch).
pub fn note_mode_flip_blocked(reason: &str, now_ms: u64) {
    MODE_FLIP_BLOCKED.store(true, Ordering::Relaxed);
    MODE_FLIP_BLOCKED_TOTAL.fetch_add(1, Ordering::Relaxed);
    set_last_flip_timestamp(now_ms);
    set_last_flip_error(Some(reason.to_owned()));
}

/// Snapshot of the recorded consensus mode tags.
pub fn mode_tags() -> (String, Option<String>, Option<u64>, Option<u64>) {
    let mode = MODE_TAG
        .get()
        .and_then(|slot| slot.lock().ok().map(|s| s.clone()))
        .unwrap_or_default();
    let staged = STAGED_MODE_TAG
        .get()
        .and_then(|slot| slot.lock().ok().map(|s| s.clone()))
        .unwrap_or(None);
    let activation = STAGED_MODE_ACTIVATION_HEIGHT
        .get()
        .and_then(|slot| slot.lock().ok().map(|v| *v))
        .unwrap_or(None);
    let lag = MODE_ACTIVATION_LAG_BLOCKS
        .get()
        .and_then(|slot| slot.lock().ok().map(|v| *v))
        .unwrap_or(None);
    (mode, staged, activation, lag)
}

/// Store the latest consensus handshake caps (best-effort; used for status/telemetry).
pub fn set_consensus_caps(caps: &iroha_p2p::ConsensusConfigCaps) {
    let slot = CONSENSUS_CAPS.get_or_init(|| Mutex::new(None));
    *slot.lock().unwrap() = Some(*caps);
}

/// Fetch the last recorded consensus handshake caps.
pub fn consensus_caps() -> Option<iroha_p2p::ConsensusConfigCaps> {
    CONSENSUS_CAPS
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|caps| *caps))
}

/// Record the latest deterministic membership view hash snapshot.
pub fn set_membership_view_hash(hash: [u8; 32], height: u64, view: u64, epoch: u64) {
    MEMBERSHIP_HEIGHT.store(height, Ordering::Relaxed);
    MEMBERSHIP_VIEW.store(view, Ordering::Relaxed);
    MEMBERSHIP_EPOCH.store(epoch, Ordering::Relaxed);
    let slot = MEMBERSHIP_VIEW_HASH.get_or_init(|| Mutex::new([0u8; 32]));
    *slot.lock().unwrap() = hash;
    MEMBERSHIP_HASH_SET.store(true, Ordering::Relaxed);
}

/// Snapshot PRF context if known: (seed, height, view).
pub fn prf_context() -> (Option<[u8; 32]>, u64, u64) {
    let h = PRF_HEIGHT.load(Ordering::Relaxed);
    let v = PRF_VIEW.load(Ordering::Relaxed);
    let seed = PRF_SEED
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap()
        .as_ref()
        .copied();
    (seed, h, v)
}

/// Record the latest VRF penalty snapshot for operator surfaces.
pub fn set_vrf_penalties(
    epoch: u64,
    committed_no_reveal: u64,
    no_participation: u64,
    late_reveals: u64,
) {
    VRF_PENALTY_EPOCH.store(epoch, Ordering::Relaxed);
    VRF_NON_REVEAL_TOTAL.store(committed_no_reveal, Ordering::Relaxed);
    VRF_NO_PARTICIPATION_TOTAL.store(no_participation, Ordering::Relaxed);
    VRF_LATE_REVEALS_TOTAL.store(late_reveals, Ordering::Relaxed);
}

/// Update the late-reveal counter outside of epoch finalization (e.g., when a late reveal is accepted mid-epoch).
pub fn set_vrf_late_reveals_total(late_reveals: u64) {
    VRF_LATE_REVEALS_TOTAL.store(late_reveals, Ordering::Relaxed);
}

/// Record epoch scheduling parameters (length and commit/reveal window offsets).
pub fn set_epoch_parameters(length_blocks: u64, commit_offset: u64, reveal_offset: u64) {
    EPOCH_LENGTH_BLOCKS.store(length_blocks, Ordering::Relaxed);
    EPOCH_COMMIT_DEADLINE_OFFSET.store(commit_offset, Ordering::Relaxed);
    EPOCH_REVEAL_DEADLINE_OFFSET.store(reveal_offset, Ordering::Relaxed);
}

/// Return the last recorded VRF penalty snapshot (epoch, committed-no-reveal total, no-participation total, late-reveals total).
pub fn vrf_penalty_snapshot() -> (u64, u64, u64, u64) {
    (
        VRF_PENALTY_EPOCH.load(Ordering::Relaxed),
        VRF_NON_REVEAL_TOTAL.load(Ordering::Relaxed),
        VRF_NO_PARTICIPATION_TOTAL.load(Ordering::Relaxed),
        VRF_LATE_REVEALS_TOTAL.load(Ordering::Relaxed),
    )
}

/// Increment the applied consensus-penalty counter.
pub fn inc_consensus_penalties_applied(delta: u64) {
    if delta == 0 {
        return;
    }
    CONSENSUS_PENALTIES_APPLIED_TOTAL.fetch_add(delta, Ordering::Relaxed);
}

/// Increment the applied VRF-penalty counter.
pub fn inc_vrf_penalties_applied(delta: u64) {
    if delta == 0 {
        return;
    }
    VRF_PENALTIES_APPLIED_TOTAL.fetch_add(delta, Ordering::Relaxed);
}

/// Record pending penalty counts (consensus and VRF) for status surfaces.
pub fn set_penalties_pending(consensus: u64, vrf: u64) {
    CONSENSUS_PENALTIES_PENDING.store(consensus, Ordering::Relaxed);
    VRF_PENALTIES_PENDING.store(vrf, Ordering::Relaxed);
}

/// Snapshot penalty counters for status endpoints.
pub fn penalty_counters() -> (u64, u64, u64, u64) {
    (
        CONSENSUS_PENALTIES_APPLIED_TOTAL.load(Ordering::Relaxed),
        CONSENSUS_PENALTIES_PENDING.load(Ordering::Relaxed),
        VRF_PENALTIES_APPLIED_TOTAL.load(Ordering::Relaxed),
        VRF_PENALTIES_PENDING.load(Ordering::Relaxed),
    )
}

/// Update the current `LockedQC` tuple (height, view). Best-effort, monotonic.
pub fn set_locked_qc(height: u64, view: u64, subject: Option<HashOf<BlockHeader>>) {
    if height == 0 && view == 0 {
        LOCKED_QC_HEIGHT.store(0, Ordering::Relaxed);
        LOCKED_QC_VIEW.store(0, Ordering::Relaxed);
        set_locked_qc_hash(None);
        return;
    }
    // Monotonic update by (height,view)
    let cur_h = LOCKED_QC_HEIGHT.load(Ordering::Relaxed);
    let cur_v = LOCKED_QC_VIEW.load(Ordering::Relaxed);
    if (height, view) > (cur_h, cur_v) {
        LOCKED_QC_HEIGHT.store(height, Ordering::Relaxed);
        LOCKED_QC_VIEW.store(view, Ordering::Relaxed);
        set_locked_qc_hash(subject);
        return;
    }
    if (height, view) == (cur_h, cur_v) {
        if let Some(subject) = subject {
            if locked_qc_hash() != Some(subject) {
                set_locked_qc_hash(Some(subject));
            }
        }
    }
}

/// Update the latest accepted view-change index.
pub fn set_view_change_index(view: u64) {
    VIEW_CHANGE_INDEX.store(view, Ordering::Relaxed);
}

/// Record that a view-change proof advanced the chain (accepted).
pub fn inc_view_change_proof_accepted() {
    VIEW_CHANGE_PROOF_ACCEPTED_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Record that a view-change proof was structurally valid but stale.
pub fn inc_view_change_proof_stale() {
    VIEW_CHANGE_PROOF_STALE_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Record that a view-change proof was rejected (invalid payload/signature).
pub fn inc_view_change_proof_rejected() {
    VIEW_CHANGE_PROOF_REJECTED_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Record that the local node suggested a view change.
pub fn inc_view_change_suggest() {
    VIEW_CHANGE_SUGGEST_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Record that a view change was installed (proof advanced).
pub fn inc_view_change_install() {
    VIEW_CHANGE_INSTALL_TOTAL.fetch_add(1, Ordering::Relaxed);
}

fn nexus_fee_slot() -> &'static Mutex<NexusFeeSnapshot> {
    NEXUS_FEE_STATUS.get_or_init(|| Mutex::new(NexusFeeSnapshot::default()))
}

fn nexus_staking_slot() -> &'static Mutex<BTreeMap<LaneId, NexusStakingLaneSnapshot>> {
    NEXUS_STAKING_STATUS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Record a Nexus fee debit outcome for later status/telemetry surfacing.
pub fn record_nexus_fee_event(event: NexusFeeEvent) {
    let mut guard = nexus_fee_slot()
        .lock()
        .expect("nexus fee status mutex poisoned");
    match event {
        NexusFeeEvent::Charged {
            payer_kind,
            payer_id,
            amount,
            asset_id,
        } => {
            guard.charged_total = guard.charged_total.saturating_add(1);
            match payer_kind {
                NexusFeePayer::Payer => {
                    guard.charged_via_payer_total = guard.charged_via_payer_total.saturating_add(1);
                }
                NexusFeePayer::Sponsor => {
                    guard.charged_via_sponsor_total =
                        guard.charged_via_sponsor_total.saturating_add(1);
                }
            }
            guard.last_amount = Some(amount);
            guard.last_asset_id = Some(asset_id);
            guard.last_payer = Some(payer_kind);
            guard.last_payer_id = Some(payer_id);
            guard.last_error = None;
        }
        NexusFeeEvent::SponsorDisabled { payer_id } => {
            guard.sponsor_disabled_total = guard.sponsor_disabled_total.saturating_add(1);
            guard.last_payer = Some(NexusFeePayer::Sponsor);
            guard.last_payer_id = Some(payer_id);
            guard.last_error = Some("sponsorship disabled".to_string());
        }
        NexusFeeEvent::SponsorUnauthorized {
            sponsor_id,
            authority_id,
        } => {
            guard.sponsor_unauthorized_total = guard.sponsor_unauthorized_total.saturating_add(1);
            guard.last_payer = Some(NexusFeePayer::Sponsor);
            guard.last_payer_id = Some(sponsor_id);
            guard.last_error = Some(format!(
                "sponsor not authorized for authority {authority_id}"
            ));
        }
        NexusFeeEvent::SponsorCapExceeded {
            payer_id,
            max_fee,
            attempted_fee,
        } => {
            guard.sponsor_cap_exceeded_total = guard.sponsor_cap_exceeded_total.saturating_add(1);
            guard.last_payer = Some(NexusFeePayer::Sponsor);
            guard.last_payer_id = Some(payer_id);
            guard.last_amount = Some(attempted_fee);
            guard.last_error = Some(format!("sponsor_max_fee exceeded (max={max_fee})"));
        }
        NexusFeeEvent::TransferFailed {
            payer_kind,
            payer_id,
            amount,
            asset_id,
            reason,
        } => {
            guard.transfer_failures_total = guard.transfer_failures_total.saturating_add(1);
            guard.last_payer = Some(payer_kind);
            guard.last_payer_id = Some(payer_id);
            guard.last_amount = Some(amount);
            guard.last_asset_id = Some(asset_id);
            guard.last_error = Some(reason);
        }
        NexusFeeEvent::ConfigInvalid { reason } => {
            guard.config_errors_total = guard.config_errors_total.saturating_add(1);
            guard.last_error = Some(reason);
        }
    }
}

fn update_staking_lane<F>(lane_id: LaneId, mut update: F)
where
    F: FnMut(&mut NexusStakingLaneSnapshot),
{
    let mut guard = nexus_staking_slot()
        .lock()
        .expect("nexus staking status mutex poisoned");
    let entry = guard
        .entry(lane_id)
        .or_insert_with(|| NexusStakingLaneSnapshot {
            lane_id,
            ..NexusStakingLaneSnapshot::default()
        });
    update(entry);
}

fn adjust_numeric_value(current: Numeric, delta: &Numeric, increase: bool) -> Numeric {
    if delta.is_zero() {
        return current;
    }
    if increase {
        let base = current.clone();
        current.checked_add(delta.clone()).unwrap_or_else(|| {
            iroha_logger::warn!(
                %base,
                %delta,
                "nexus staking accumulator overflowed; clamping to Numeric::zero()"
            );
            Numeric::zero()
        })
    } else {
        let base = current.clone();
        current.checked_sub(delta.clone()).unwrap_or_else(|| {
            iroha_logger::warn!(
                %base,
                %delta,
                "nexus staking accumulator underflowed; clamping to Numeric::zero()"
            );
            Numeric::zero()
        })
    }
}

/// Record a bonded stake delta for a Nexus lane.
pub fn record_public_lane_bonded_delta(lane_id: LaneId, amount: &Numeric, increase: bool) {
    update_staking_lane(lane_id, |snapshot| {
        snapshot.bonded = adjust_numeric_value(snapshot.bonded.clone(), amount, increase);
    });
}

/// Record a pending-unbond delta for a Nexus lane.
pub fn record_public_lane_pending_unbond_delta(lane_id: LaneId, amount: &Numeric, increase: bool) {
    update_staking_lane(lane_id, |snapshot| {
        snapshot.pending_unbond =
            adjust_numeric_value(snapshot.pending_unbond.clone(), amount, increase);
    });
}

/// Record a slash event for a Nexus lane.
pub fn record_public_lane_slash(lane_id: LaneId) {
    update_staking_lane(lane_id, |snapshot| {
        snapshot.slash_total = snapshot.slash_total.saturating_add(1);
    });
}

/// Latest aggregated Nexus fee snapshot.
pub fn nexus_fee_snapshot() -> NexusFeeSnapshot {
    nexus_fee_slot()
        .lock()
        .expect("nexus fee status mutex poisoned")
        .clone()
}

/// Latest aggregated Nexus staking snapshot.
pub fn nexus_staking_snapshot() -> NexusStakingSnapshot {
    let guard = nexus_staking_slot()
        .lock()
        .expect("nexus staking status mutex poisoned");
    let mut lanes: Vec<_> = guard.values().cloned().collect();
    lanes.sort_by_key(|lane| lane.lane_id.as_u32());
    NexusStakingSnapshot { lanes }
}

/// Shared lock for tests that mutate global Nexus fee state.
pub fn nexus_fee_test_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Clear Nexus economics snapshots (test-only helper).
pub fn reset_nexus_economics_for_tests() {
    {
        let mut guard = nexus_fee_slot()
            .lock()
            .expect("nexus fee status mutex poisoned");
        *guard = NexusFeeSnapshot::default();
    }
    {
        let mut guard = nexus_staking_slot()
            .lock()
            .expect("nexus staking status mutex poisoned");
        guard.clear();
    }
}

/// Latest RBC sessions evicted from the on-disk store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RbcEvictedSession {
    /// Block hash associated with the evicted session (typed as raw bytes for JSON convenience).
    pub block_hash: [u8; 32],
    /// Block height for the evicted session.
    pub height: u64,
    /// View index for the evicted session.
    pub view: u64,
}

/// Snapshot of kura persistence failures.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KuraStoreSnapshot {
    /// Total times a block failed to enqueue for persistence.
    pub failures_total: u64,
    /// Total times kura persistence retries were exhausted for a block.
    pub abort_total: u64,
    /// Total times a block reached the staging phase before persistence.
    pub stage_total: u64,
    /// Total times a staged commit was rolled back before WSV application.
    pub rollback_total: u64,
    /// Height of the last staged block (best-effort).
    pub stage_last_height: u64,
    /// View of the last staged block (best-effort).
    pub stage_last_view: u64,
    /// Hash of the last staged block (best-effort).
    pub stage_last_hash: Option<HashOf<BlockHeader>>,
    /// Height of the last staged commit rolled back (best-effort).
    pub rollback_last_height: u64,
    /// View of the last staged commit rolled back (best-effort).
    pub rollback_last_view: u64,
    /// Hash of the last staged commit rolled back (best-effort).
    pub rollback_last_hash: Option<HashOf<BlockHeader>>,
    /// Reason label for the last rollback (best-effort).
    pub rollback_last_reason: Option<&'static str>,
    /// Total times Highest/Locked QC were reset after a kura abort.
    pub lock_reset_total: u64,
    /// Height associated with the last lock reset (best-effort).
    pub lock_reset_last_height: u64,
    /// View associated with the last lock reset (best-effort).
    pub lock_reset_last_view: u64,
    /// Hash associated with the last lock reset (best-effort).
    pub lock_reset_last_hash: Option<HashOf<BlockHeader>>,
    /// Reason label for the last lock reset (best-effort).
    pub lock_reset_last_reason: Option<&'static str>,
    /// Last observed retry attempt count.
    pub last_retry_attempt: u64,
    /// Last observed retry backoff in milliseconds.
    pub last_retry_backoff_ms: u64,
    /// Height of the last block that failed to persist (best-effort).
    pub last_height: u64,
    /// View of the last block that failed to persist (best-effort).
    pub last_view: u64,
    /// Hash of the last block that failed to persist (best-effort).
    pub last_hash: Option<HashOf<BlockHeader>>,
}

/// Snapshot of block-sync roster selection and drop counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockSyncRosterSnapshot {
    /// Total times a commit certificate hint was used.
    pub commit_qc_hint_total: u64,
    /// Total times a validator-checkpoint hint was used.
    pub checkpoint_hint_total: u64,
    /// Total times commit-certificate history was used.
    pub commit_qc_history_total: u64,
    /// Total times validator-checkpoint history was used.
    pub checkpoint_history_total: u64,
    /// Total times a roster sidecar was used.
    pub roster_sidecar_total: u64,
    /// Total times a commit-roster journal snapshot was used.
    pub commit_roster_journal_total: u64,
    /// Total block-sync drops due to missing/invalid roster proofs.
    pub drop_missing_total: u64,
    /// Total block-sync `ShareBlocks` drops without a matching request.
    pub drop_unsolicited_share_blocks_total: u64,
}

/// Consensus message kinds tracked for drop/deferral telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConsensusMessageKind {
    /// Block payload creation (`BlockCreated`).
    BlockCreated,
    /// Block-sync update batches (`BlockSyncUpdate`).
    BlockSyncUpdate,
    /// Consensus-parameter advertisements (`ConsensusParams`).
    ConsensusParams,
    /// Proposal hints (`ProposalHint`).
    ProposalHint,
    /// Proposals (`Proposal`).
    Proposal,
    /// Commit votes (`QcVote`).
    QcVote,
    /// Commit certificates (`Qc`).
    Qc,
    /// VRF commit broadcasts (`VrfCommit`).
    VrfCommit,
    /// VRF reveal broadcasts (`VrfReveal`).
    VrfReveal,
    /// Execution witness payloads (`ExecWitness`).
    ExecWitness,
    /// RBC init payloads (`RbcInit`).
    RbcInit,
    /// RBC chunk payloads (`RbcChunk`).
    RbcChunk,
    /// RBC ready messages (`RbcReady`).
    RbcReady,
    /// RBC delivery notifications (`RbcDeliver`).
    RbcDeliver,
    /// Fetch-pending-block requests (`FetchPendingBlock`).
    FetchPendingBlock,
    /// Consensus control-flow evidence.
    Evidence,
}

impl ConsensusMessageKind {
    /// Stable label for telemetry and status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ConsensusMessageKind::BlockCreated => "block_created",
            ConsensusMessageKind::BlockSyncUpdate => "block_sync_update",
            ConsensusMessageKind::ConsensusParams => "consensus_params",
            ConsensusMessageKind::ProposalHint => "proposal_hint",
            ConsensusMessageKind::Proposal => "proposal",
            ConsensusMessageKind::QcVote => "qc_vote",
            ConsensusMessageKind::Qc => "qc",
            ConsensusMessageKind::VrfCommit => "vrf_commit",
            ConsensusMessageKind::VrfReveal => "vrf_reveal",
            ConsensusMessageKind::ExecWitness => "exec_witness",
            ConsensusMessageKind::RbcInit => "rbc_init",
            ConsensusMessageKind::RbcChunk => "rbc_chunk",
            ConsensusMessageKind::RbcReady => "rbc_ready",
            ConsensusMessageKind::RbcDeliver => "rbc_deliver",
            ConsensusMessageKind::FetchPendingBlock => "fetch_pending_block",
            ConsensusMessageKind::Evidence => "evidence",
        }
    }
}

/// Outcome recorded for consensus-message handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConsensusMessageOutcome {
    /// The message was dropped.
    Dropped,
    /// The message was deferred/stashed for later processing.
    Deferred,
}

impl ConsensusMessageOutcome {
    /// Stable label for telemetry and status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ConsensusMessageOutcome::Dropped => "dropped",
            ConsensusMessageOutcome::Deferred => "deferred",
        }
    }
}

/// Reason a consensus message was dropped or deferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConsensusMessageReason {
    /// Beyond the configured future height/view window.
    FutureWindow,
    /// Height is at or below the committed tip.
    StaleHeight,
    /// View is older than the local view.
    StaleView,
    /// Duplicate payload already seen.
    Duplicate,
    /// Conflicting vote already recorded for the signer.
    ConflictingVote,
    /// Locked QC gate rejected the payload.
    LockedQc,
    /// Missing highest QC reference.
    MissingHighestQc,
    /// Highest QC header mismatch.
    HighestQcMismatch,
    /// `BlockCreated` hint validation failed.
    HintMismatch,
    /// Payload hash mismatched the expected value.
    PayloadMismatch,
    /// Payload failed basic validation.
    InvalidPayload,
    /// Payload exceeds size limits.
    PayloadTooLarge,
    /// Incoming block conflicts with a committed block hash.
    CommitConflict,
    /// No verifiable roster available for the payload.
    RosterMissing,
    /// Signature validation failed for the payload.
    InvalidSignature,
    /// Missing quorum evidence for block sync.
    QuorumMissing,
    /// Payload could not be applied locally.
    PayloadUnapplied,
    /// Signature mismatch deferred while the node catches up.
    SignatureMismatchDeferred,
    /// Payload deferred while commit/validation work is in flight.
    CommitPipelineActive,
    /// QC aggregate verification pending on background worker.
    AggregateVerifyDeferred,
    /// Delivery channel is unavailable.
    EnqueueFailed,
    /// Sender is currently penalized.
    PenalizedSender,
    /// Epoch mismatch on the payload.
    EpochMismatch,
    /// Payload already committed locally.
    Committed,
    /// Roster hash mismatched authoritative roster.
    RosterHashMismatch,
    /// RBC chunk digest mismatched expected hash.
    ChunkDigestMismatch,
    /// Chunk root mismatch detected.
    ChunkRootMismatch,
    /// Pending stash session cap reached.
    StashSessionLimit,
    /// Pending stash payload cap reached.
    StashCap,
    /// RBC DELIVER deferred awaiting READY quorum.
    ReadyQuorumMissing,
    /// RBC DELIVER deferred awaiting missing chunks.
    ChunksMissing,
    /// RBC DELIVER deferred while INIT is missing.
    InitMissing,
    /// RBC DELIVER deferred while roster is missing.
    RosterMissingDeferred,
    /// RBC DELIVER deferred while roster hash mismatches.
    RosterHashMismatchDeferred,
    /// RBC DELIVER deferred while roster is unverified.
    RosterUnverifiedDeferred,
    /// Consensus message ignored due to mismatched mode/context.
    ModeMismatch,
    /// Requested data not found locally.
    NotFound,
}

impl ConsensusMessageReason {
    /// Stable label for telemetry and status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ConsensusMessageReason::FutureWindow => "future_window",
            ConsensusMessageReason::StaleHeight => "stale_height",
            ConsensusMessageReason::StaleView => "stale_view",
            ConsensusMessageReason::Duplicate => "duplicate",
            ConsensusMessageReason::ConflictingVote => "conflicting_vote",
            ConsensusMessageReason::LockedQc => "locked_qc",
            ConsensusMessageReason::MissingHighestQc => "missing_highest_qc",
            ConsensusMessageReason::HighestQcMismatch => "highest_qc_mismatch",
            ConsensusMessageReason::HintMismatch => "hint_mismatch",
            ConsensusMessageReason::PayloadMismatch => "payload_mismatch",
            ConsensusMessageReason::InvalidPayload => "invalid_payload",
            ConsensusMessageReason::PayloadTooLarge => "payload_too_large",
            ConsensusMessageReason::CommitConflict => "commit_conflict",
            ConsensusMessageReason::RosterMissing => "roster_missing",
            ConsensusMessageReason::InvalidSignature => "invalid_signature",
            ConsensusMessageReason::QuorumMissing => "quorum_missing",
            ConsensusMessageReason::PayloadUnapplied => "payload_unapplied",
            ConsensusMessageReason::SignatureMismatchDeferred => "signature_mismatch_deferred",
            ConsensusMessageReason::CommitPipelineActive => "commit_pipeline_active",
            ConsensusMessageReason::AggregateVerifyDeferred => "aggregate_verify_deferred",
            ConsensusMessageReason::EnqueueFailed => "enqueue_failed",
            ConsensusMessageReason::PenalizedSender => "penalized_sender",
            ConsensusMessageReason::EpochMismatch => "epoch_mismatch",
            ConsensusMessageReason::Committed => "committed",
            ConsensusMessageReason::RosterHashMismatch => "roster_hash_mismatch",
            ConsensusMessageReason::ChunkDigestMismatch => "chunk_digest_mismatch",
            ConsensusMessageReason::ChunkRootMismatch => "chunk_root_mismatch",
            ConsensusMessageReason::StashSessionLimit => "stash_session_limit",
            ConsensusMessageReason::StashCap => "stash_cap",
            ConsensusMessageReason::ReadyQuorumMissing => "ready_quorum_missing",
            ConsensusMessageReason::ChunksMissing => "chunks_missing",
            ConsensusMessageReason::InitMissing => "init_missing",
            ConsensusMessageReason::RosterMissingDeferred => "roster_missing_deferred",
            ConsensusMessageReason::RosterHashMismatchDeferred => "roster_hash_mismatch_deferred",
            ConsensusMessageReason::RosterUnverifiedDeferred => "roster_unverified_deferred",
            ConsensusMessageReason::ModeMismatch => "mode_mismatch",
            ConsensusMessageReason::NotFound => "not_found",
        }
    }
}

/// Single consensus-message handling counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsensusMessageHandlingEntry {
    /// Message kind.
    pub kind: ConsensusMessageKind,
    /// Handling outcome (dropped or deferred).
    pub outcome: ConsensusMessageOutcome,
    /// Reason label for the drop/deferral.
    pub reason: ConsensusMessageReason,
    /// Total count observed.
    pub total: u64,
}

/// Snapshot of consensus-message handling counters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConsensusMessageHandlingSnapshot {
    /// Per-kind drop/deferral counters with reason labels.
    pub entries: Vec<ConsensusMessageHandlingEntry>,
}

/// Reasons a consensus vote can be dropped during validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VoteValidationDropReason {
    /// Height is at or below the committed tip.
    StaleHeight,
    /// View is older than the local view.
    StaleView,
    /// Epoch mismatch on the vote payload.
    EpochMismatch,
    /// Sender is currently penalized.
    PenalizedSender,
    /// Commit roster could not be resolved for the vote.
    RosterMissing,
    /// Locked QC gate rejected the vote.
    LockedQc,
    /// Duplicate vote already recorded.
    Duplicate,
    /// Signer index cannot be represented as usize.
    SignerIndexOverflow,
    /// Signer index exceeds the active roster length.
    SignerOutOfRange,
    /// Signature payload fails cryptographic validation.
    SignatureInvalid,
    /// `NEW_VIEW` vote missing the highest QC reference.
    MissingHighestQc,
    /// `NEW_VIEW` highest QC mismatch.
    HighestQcMismatch,
    /// Conflicting vote already recorded for the signer.
    ConflictingVote,
}

impl VoteValidationDropReason {
    /// Stable label for telemetry and status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            VoteValidationDropReason::StaleHeight => "stale_height",
            VoteValidationDropReason::StaleView => "stale_view",
            VoteValidationDropReason::EpochMismatch => "epoch_mismatch",
            VoteValidationDropReason::PenalizedSender => "penalized_sender",
            VoteValidationDropReason::RosterMissing => "roster_missing",
            VoteValidationDropReason::LockedQc => "locked_qc",
            VoteValidationDropReason::Duplicate => "duplicate",
            VoteValidationDropReason::SignerIndexOverflow => "signer_index_overflow",
            VoteValidationDropReason::SignerOutOfRange => "signer_out_of_range",
            VoteValidationDropReason::SignatureInvalid => "invalid_signature",
            VoteValidationDropReason::MissingHighestQc => "missing_highest_qc",
            VoteValidationDropReason::HighestQcMismatch => "highest_qc_mismatch",
            VoteValidationDropReason::ConflictingVote => "conflicting_vote",
        }
    }
}

/// Input record for a vote validation drop event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoteValidationDropRecord {
    /// Drop reason label.
    pub reason: VoteValidationDropReason,
    /// Vote height.
    pub height: u64,
    /// Vote view.
    pub view: u64,
    /// Vote epoch.
    pub epoch: u64,
    /// Signer index from the vote payload.
    pub signer_index: u32,
    /// Peer ID resolved from the validation roster (if any).
    pub peer_id: Option<PeerId>,
    /// Validator roster hash used for validation (if any).
    pub roster_hash: Option<HashOf<Vec<PeerId>>>,
    /// Validator roster length used for validation (if known).
    pub roster_len: u32,
    /// Block hash referenced by the vote.
    pub block_hash: HashOf<BlockHeader>,
}

/// Recorded vote validation drop entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoteValidationDropEntry {
    /// Drop reason label.
    pub reason: VoteValidationDropReason,
    /// Vote height.
    pub height: u64,
    /// Vote view.
    pub view: u64,
    /// Vote epoch.
    pub epoch: u64,
    /// Signer index from the vote payload.
    pub signer_index: u32,
    /// Peer ID resolved from the validation roster (if any).
    pub peer_id: Option<PeerId>,
    /// Validator roster hash used for validation (if any).
    pub roster_hash: Option<HashOf<Vec<PeerId>>>,
    /// Validator roster length used for validation (if known).
    pub roster_len: u32,
    /// Block hash referenced by the vote.
    pub block_hash: HashOf<BlockHeader>,
    /// Milliseconds since UNIX epoch when the drop was recorded.
    pub timestamp_ms: u64,
}

/// Aggregated count for a vote-validation drop reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoteValidationDropReasonCount {
    /// Drop reason label.
    pub reason: VoteValidationDropReason,
    /// Total drops recorded for the reason.
    pub total: u64,
}

/// Aggregated vote validation drops for a peer/roster hash pairing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoteValidationDropPeerEntry {
    /// Peer associated with the drop counts.
    pub peer_id: PeerId,
    /// Validator roster hash used for validation (if any).
    pub roster_hash: Option<HashOf<Vec<PeerId>>>,
    /// Validator roster length used for validation (if known).
    pub roster_len: u32,
    /// Total drops recorded for this peer/roster pairing.
    pub total: u64,
    /// Per-reason drop counters.
    pub reasons: Vec<VoteValidationDropReasonCount>,
    /// Height associated with the last drop.
    pub last_height: u64,
    /// View associated with the last drop.
    pub last_view: u64,
    /// Epoch associated with the last drop.
    pub last_epoch: u64,
    /// Milliseconds since UNIX epoch when the last drop was recorded.
    pub last_timestamp_ms: u64,
}

/// Snapshot of recent vote validation drops.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoteValidationDropSnapshot {
    /// Total vote validation drops recorded.
    pub total: u64,
    /// Recent drop entries (newest-first, bounded).
    pub entries: Vec<VoteValidationDropEntry>,
    /// Aggregated drop counters per peer/roster pairing.
    pub peer_entries: Vec<VoteValidationDropPeerEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ConsensusMessageHandlingKey {
    kind: ConsensusMessageKind,
    outcome: ConsensusMessageOutcome,
    reason: ConsensusMessageReason,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct VoteValidationDropPeerKey {
    peer_id: PeerId,
    roster_hash: Option<HashOf<Vec<PeerId>>>,
}

#[derive(Clone, Debug, Default)]
struct VoteValidationDropPeerState {
    roster_len: u32,
    total: u64,
    reasons: BTreeMap<VoteValidationDropReason, u64>,
    last_height: u64,
    last_view: u64,
    last_epoch: u64,
    last_timestamp_ms: u64,
}

/// Snapshot of view-change causes and timing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewChangeCauseSnapshot {
    /// Total view changes triggered after commit failures (with QC quorum).
    pub commit_failure_total: u64,
    /// Total view changes triggered after quorum timeouts/missing commits.
    pub quorum_timeout_total: u64,
    /// Total view changes triggered after stake-quorum timeouts (`NPoS` only).
    pub stake_quorum_timeout_total: u64,
    /// Total view changes triggered after DA availability aborts (unused when DA is advisory).
    pub da_gate_total: u64,
    /// Total view changes triggered after censorship evidence reaches quorum.
    pub censorship_evidence_total: u64,
    /// Total view changes triggered after missing payloads exceeded dwell.
    pub missing_payload_total: u64,
    /// Total view changes triggered after missing or stale QCs.
    pub missing_qc_total: u64,
    /// Total view changes triggered after validation rejects before voting.
    pub validation_reject_total: u64,
    /// Last recorded view-change cause label (if any).
    pub last_cause: Option<String>,
    /// Milliseconds since UNIX epoch when the last cause was recorded.
    pub last_cause_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a commit-failure cause was last recorded.
    pub last_commit_failure_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a quorum-timeout cause was last recorded.
    pub last_quorum_timeout_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a stake-quorum-timeout cause was last recorded.
    pub last_stake_quorum_timeout_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a DA-gate cause was last recorded.
    pub last_da_gate_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a censorship-evidence cause was last recorded.
    pub last_censorship_evidence_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a missing-payload cause was last recorded.
    pub last_missing_payload_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a missing-QC cause was last recorded.
    pub last_missing_qc_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a validation-reject cause was last recorded.
    pub last_validation_reject_timestamp_ms: u64,
}

/// Snapshot of validation gate rejects before voting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ValidationRejectSnapshot {
    /// Total rejects recorded.
    pub total: u64,
    /// Stateless validation rejects (header, timestamps, genesis checks).
    pub stateless_total: u64,
    /// Execution/stateful validation rejects (transaction execution, DA availability checks).
    pub execution_total: u64,
    /// Prev-block hash mismatch rejects.
    pub prev_hash_total: u64,
    /// Prev-block height mismatch rejects.
    pub prev_height_total: u64,
    /// Topology/roster mismatch rejects.
    pub topology_total: u64,
    /// Last recorded reason label (best-effort).
    pub last_reason: Option<&'static str>,
    /// Last rejected block height (best-effort).
    pub last_height: Option<u64>,
    /// Last rejected block view (best-effort).
    pub last_view: Option<u64>,
    /// Last rejected block hash (best-effort).
    pub last_block: Option<HashOf<BlockHeader>>,
    /// Milliseconds since UNIX epoch when the last reject was recorded.
    pub last_timestamp_ms: u64,
}

/// Reasons a peer-consensus-key admission can be rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerKeyPolicyRejectReason {
    /// Required HSM binding missing.
    MissingHsm,
    /// Public-key algorithm not allowed by policy.
    DisallowedAlgorithm,
    /// HSM provider not allowed by policy.
    DisallowedProvider,
    /// Activation height violates lead-time policy.
    LeadTimeViolation,
    /// Activation height is in the past.
    ActivationInPast,
    /// Expiry occurs before activation.
    ExpiryBeforeActivation,
    /// Consensus-key identifier collides with an existing id for the same public key.
    IdentifierCollision,
}

impl PeerKeyPolicyRejectReason {
    /// Return a stable label for telemetry and status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            PeerKeyPolicyRejectReason::MissingHsm => "missing_hsm",
            PeerKeyPolicyRejectReason::DisallowedAlgorithm => "disallowed_algorithm",
            PeerKeyPolicyRejectReason::DisallowedProvider => "disallowed_provider",
            PeerKeyPolicyRejectReason::LeadTimeViolation => "lead_time_violation",
            PeerKeyPolicyRejectReason::ActivationInPast => "activation_in_past",
            PeerKeyPolicyRejectReason::ExpiryBeforeActivation => "expiry_before_activation",
            PeerKeyPolicyRejectReason::IdentifierCollision => "identifier_collision",
        }
    }
}

/// Snapshot of peer consensus-key policy rejects recorded during admission.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PeerKeyPolicySnapshot {
    /// Total rejects recorded.
    pub total: u64,
    /// Rejects due to missing HSM binding when required.
    pub missing_hsm_total: u64,
    /// Rejects due to disallowed public-key algorithm.
    pub disallowed_algorithm_total: u64,
    /// Rejects due to disallowed HSM provider.
    pub disallowed_provider_total: u64,
    /// Rejects due to activation failing lead-time checks.
    pub lead_time_violation_total: u64,
    /// Rejects due to activation height being in the past.
    pub activation_in_past_total: u64,
    /// Rejects due to expiry occurring before activation.
    pub expiry_before_activation_total: u64,
    /// Rejects due to identifier collisions for the same public key.
    pub identifier_collision_total: u64,
    /// Last recorded reject reason (best-effort).
    pub last_reason: Option<&'static str>,
    /// Milliseconds since UNIX epoch when the last reject was recorded.
    pub last_timestamp_ms: u64,
}

/// Stages within the Sumeragi worker loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WorkerLoopStage {
    /// Worker loop is idle or sleeping.
    #[default]
    Idle,
    /// Draining vote-related messages.
    DrainVotes,
    /// Draining RBC chunk messages.
    DrainRbcChunks,
    /// Draining block payload messages.
    DrainBlockPayloads,
    /// Draining block messages.
    DrainBlocks,
    /// Executing the consensus tick.
    Tick,
    /// Draining consensus control-flow messages.
    DrainConsensus,
    /// Draining lane relay envelopes.
    DrainLaneRelay,
    /// Draining background post requests.
    DrainBackground,
}

impl WorkerLoopStage {
    const fn as_id(self) -> u64 {
        match self {
            WorkerLoopStage::Idle => 0,
            WorkerLoopStage::DrainVotes => 1,
            WorkerLoopStage::DrainRbcChunks => 2,
            WorkerLoopStage::DrainBlockPayloads => 3,
            WorkerLoopStage::DrainBlocks => 4,
            WorkerLoopStage::Tick => 5,
            WorkerLoopStage::DrainConsensus => 6,
            WorkerLoopStage::DrainLaneRelay => 7,
            WorkerLoopStage::DrainBackground => 8,
        }
    }

    fn from_id(id: u64) -> Self {
        match id {
            1 => WorkerLoopStage::DrainVotes,
            2 => WorkerLoopStage::DrainRbcChunks,
            3 => WorkerLoopStage::DrainBlockPayloads,
            4 => WorkerLoopStage::DrainBlocks,
            5 => WorkerLoopStage::Tick,
            6 => WorkerLoopStage::DrainConsensus,
            7 => WorkerLoopStage::DrainLaneRelay,
            8 => WorkerLoopStage::DrainBackground,
            _ => WorkerLoopStage::Idle,
        }
    }

    /// Return a stable label for status exports.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            WorkerLoopStage::Idle => "idle",
            WorkerLoopStage::DrainVotes => "drain_votes",
            WorkerLoopStage::DrainRbcChunks => "drain_rbc_chunks",
            WorkerLoopStage::DrainBlockPayloads => "drain_block_payloads",
            WorkerLoopStage::DrainBlocks => "drain_blocks",
            WorkerLoopStage::Tick => "tick",
            WorkerLoopStage::DrainConsensus => "drain_consensus",
            WorkerLoopStage::DrainLaneRelay => "drain_lane_relay",
            WorkerLoopStage::DrainBackground => "drain_background",
        }
    }
}

/// Worker-loop queue identifiers used for depth tracking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerQueueKind {
    /// Vote-related messages.
    Votes,
    /// Block payload messages.
    BlockPayload,
    /// RBC chunk messages.
    RbcChunks,
    /// Block messages.
    Blocks,
    /// Consensus control-flow messages.
    Consensus,
    /// Lane relay envelopes.
    LaneRelay,
    /// Background post requests.
    Background,
}

/// Tracked queue depths for the Sumeragi worker loop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkerQueueDepthSnapshot {
    /// Queue depth for vote-related messages.
    pub vote_rx: u64,
    /// Queue depth for block payload messages.
    pub block_payload_rx: u64,
    /// Queue depth for RBC chunk messages.
    pub rbc_chunk_rx: u64,
    /// Queue depth for block messages.
    pub block_rx: u64,
    /// Queue depth for consensus control-flow messages.
    pub consensus_rx: u64,
    /// Queue depth for lane relay envelopes.
    pub lane_relay_rx: u64,
    /// Queue depth for background post requests.
    pub background_rx: u64,
}

/// Per-queue totals for enqueue diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkerQueueTotalsSnapshot {
    /// Total for vote-related messages.
    pub vote_rx: u64,
    /// Total for block payload messages.
    pub block_payload_rx: u64,
    /// Total for RBC chunk messages.
    pub rbc_chunk_rx: u64,
    /// Total for block messages.
    pub block_rx: u64,
    /// Total for consensus control-flow messages.
    pub consensus_rx: u64,
    /// Total for lane relay envelopes.
    pub lane_relay_rx: u64,
    /// Total for background post requests.
    pub background_rx: u64,
}

/// Diagnostics for worker-loop enqueue behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkerQueueDiagnosticsSnapshot {
    /// Total count of blocking enqueues per queue.
    pub blocked_total: WorkerQueueTotalsSnapshot,
    /// Total time (ms) spent blocked on enqueues per queue.
    pub blocked_ms_total: WorkerQueueTotalsSnapshot,
    /// Max observed block duration (ms) per queue.
    pub blocked_max_ms: WorkerQueueTotalsSnapshot,
    /// Total count of dropped enqueues per queue.
    pub dropped_total: WorkerQueueTotalsSnapshot,
}

/// Snapshot of the Sumeragi worker loop state for diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkerLoopSnapshot {
    /// Last recorded worker-loop stage.
    pub stage: WorkerLoopStage,
    /// Milliseconds since UNIX epoch when the stage was last updated.
    pub stage_started_ms: u64,
    /// Duration of the most recent worker iteration in milliseconds.
    pub last_iteration_ms: u64,
    /// Latest observed queue depths.
    pub queue_depths: WorkerQueueDepthSnapshot,
    /// Queue enqueue diagnostics (drops/blocking).
    pub queue_diagnostics: WorkerQueueDiagnosticsSnapshot,
}

/// Commit inflight snapshot used to detect stalled finalization.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitInflightSnapshot {
    /// Whether a commit job is currently in flight.
    pub active: bool,
    /// Inflight commit id (best-effort).
    pub id: u64,
    /// Block height associated with the inflight commit.
    pub height: u64,
    /// View associated with the inflight commit.
    pub view: u64,
    /// Block hash associated with the inflight commit.
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Timestamp (ms since UNIX epoch) when the inflight commit was enqueued.
    pub started_ms: u64,
    /// Milliseconds elapsed since the inflight commit started (best-effort).
    pub elapsed_ms: u64,
    /// Configured inflight timeout in milliseconds.
    pub timeout_ms: u64,
    /// Total inflight timeouts observed.
    pub timeout_total: u64,
    /// Timestamp (ms since UNIX epoch) of the last inflight timeout.
    pub last_timeout_timestamp_ms: u64,
    /// Duration (ms) of the last inflight timeout.
    pub last_timeout_elapsed_ms: u64,
    /// Height associated with the last inflight timeout.
    pub last_timeout_height: u64,
    /// View associated with the last inflight timeout.
    pub last_timeout_view: u64,
    /// Block hash associated with the last inflight timeout.
    pub last_timeout_block_hash: Option<HashOf<BlockHeader>>,
    /// Total number of pacemaker pauses caused by inflight commits.
    pub pause_total: u64,
    /// Total number of pacemaker resumes following inflight completion.
    pub resume_total: u64,
    /// Timestamp (ms since UNIX epoch) when the current pause began.
    pub paused_since_ms: u64,
    /// Queue depth snapshot recorded when the inflight pause started.
    pub pause_queue_depths: WorkerQueueDepthSnapshot,
    /// Queue depth snapshot recorded when the inflight pause ended.
    pub resume_queue_depths: WorkerQueueDepthSnapshot,
}

/// Snapshot of the latest commit-quorum signature tally.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommitQuorumSnapshot {
    /// Block height associated with the tally.
    pub height: u64,
    /// View associated with the tally.
    pub view: u64,
    /// Block hash associated with the tally.
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Total signatures present on the block.
    pub signatures_present: u64,
    /// Signatures counted toward the commit quorum.
    pub signatures_counted: u64,
    /// Signatures contributed by set-B validators.
    pub signatures_set_b: u64,
    /// Required commit quorum size for the roster.
    pub signatures_required: u64,
    /// Timestamp (ms since UNIX epoch) when the tally was recorded.
    pub last_updated_ms: u64,
}

/// Snapshot of the most recent commit certificate (summary only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QcSnapshot {
    /// Block height certified by the latest commit certificate.
    pub height: u64,
    /// View associated with the commit certificate.
    pub view: u64,
    /// Epoch associated with the commit certificate.
    pub epoch: u64,
    /// Block hash certified by the commit certificate.
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Stable hash of the validator set that produced the certificate.
    pub validator_set_hash: Option<HashOf<Vec<PeerId>>>,
    /// Number of validators in the recorded set.
    pub validator_set_len: u64,
    /// Total signatures attached to the certificate.
    pub signatures_total: u64,
}

/// Snapshot of dedup cache evictions for inbound consensus traffic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DedupEvictionSnapshot {
    /// Vote dedup evictions due to capacity.
    pub vote_capacity_total: u64,
    /// Vote dedup evictions due to TTL expiry.
    pub vote_expired_total: u64,
    /// `BlockCreated` dedup evictions due to capacity.
    pub block_created_capacity_total: u64,
    /// `BlockCreated` dedup evictions due to TTL expiry.
    pub block_created_expired_total: u64,
    /// Proposal dedup evictions due to capacity.
    pub proposal_capacity_total: u64,
    /// Proposal dedup evictions due to TTL expiry.
    pub proposal_expired_total: u64,
    /// RBC INIT dedup evictions due to capacity.
    pub rbc_init_capacity_total: u64,
    /// RBC INIT dedup evictions due to TTL expiry.
    pub rbc_init_expired_total: u64,
    /// Block sync update dedup evictions due to capacity.
    pub block_sync_update_capacity_total: u64,
    /// Block sync update dedup evictions due to TTL expiry.
    pub block_sync_update_expired_total: u64,
    /// Fetch-pending-block dedup evictions due to capacity.
    pub fetch_pending_block_capacity_total: u64,
    /// Fetch-pending-block dedup evictions due to TTL expiry.
    pub fetch_pending_block_expired_total: u64,
    /// RBC READY dedup evictions due to capacity.
    pub rbc_ready_capacity_total: u64,
    /// RBC READY dedup evictions due to TTL expiry.
    pub rbc_ready_expired_total: u64,
    /// RBC DELIVER dedup evictions due to capacity.
    pub rbc_deliver_capacity_total: u64,
    /// RBC DELIVER dedup evictions due to TTL expiry.
    pub rbc_deliver_expired_total: u64,
    /// RBC chunk dedup evictions due to capacity.
    pub rbc_chunk_capacity_total: u64,
    /// RBC chunk dedup evictions due to TTL expiry.
    pub rbc_chunk_expired_total: u64,
}

/// Compact snapshot of consensus status for operator APIs.
#[derive(Clone, Debug, Default)]
pub struct StatusSnapshot {
    /// Current runtime mode tag.
    pub mode_tag: String,
    /// Staged mode tag if activation is pending.
    pub staged_mode_tag: Option<String>,
    /// Activation height for staged mode (if any).
    pub staged_mode_activation_height: Option<u64>,
    /// Consensus handshake configuration caps derived from runtime parameters (if computed).
    pub consensus_caps: Option<iroha_p2p::ConsensusConfigCaps>,
    /// Blocks elapsed since activation height passed without applying the staged mode (if any).
    pub mode_activation_lag_blocks: Option<u64>,
    /// Whether runtime mode flips are currently allowed by configuration.
    pub mode_flip_kill_switch: bool,
    /// Whether the most recent flip attempt was blocked (e.g., by kill switch).
    pub mode_flip_blocked: bool,
    /// Total successful mode flips applied at runtime.
    pub mode_flip_success_total: u64,
    /// Total failed mode flip attempts.
    pub mode_flip_fail_total: u64,
    /// Total flip attempts blocked by configuration.
    pub mode_flip_blocked_total: u64,
    /// Timestamp (ms since UNIX epoch) of the last attempted flip, if any.
    pub last_mode_flip_timestamp_ms: Option<u64>,
    /// Last recorded flip error (if any).
    pub last_mode_flip_error: Option<String>,
    /// Current leader index (topology position).
    pub leader_index: u64,
    /// Latest view-change index observed (best-effort).
    pub view_change_index: u64,
    /// Total view-change proofs accepted (advanced the proof chain).
    pub view_change_proof_accepted_total: u64,
    /// Total view-change proofs ignored as stale/outdated.
    pub view_change_proof_stale_total: u64,
    /// Total view-change proofs rejected as invalid.
    pub view_change_proof_rejected_total: u64,
    /// Total local view-change suggestions emitted.
    pub view_change_suggest_total: u64,
    /// Total view changes installed (proof advanced locally).
    pub view_change_install_total: u64,
    /// View-change cause counters and last occurrence.
    pub view_change_causes: ViewChangeCauseSnapshot,
    /// `HighestQC` height.
    pub highest_qc_height: u64,
    /// `HighestQC` view.
    pub highest_qc_view: u64,
    /// `LockedQC` height.
    pub locked_qc_height: u64,
    /// `LockedQC` view.
    pub locked_qc_view: u64,
    /// Optional `HighestQC` subject block hash (best-effort).
    pub highest_qc_subject: Option<HashOf<BlockHeader>>,
    /// Optional `LockedQC` subject block hash (best-effort).
    pub locked_qc_subject: Option<HashOf<BlockHeader>>,
    /// Latest commit certificate summary (best-effort).
    pub commit_qc: QcSnapshot,
    /// Latest commit quorum signature tally (best-effort).
    pub commit_quorum: CommitQuorumSnapshot,
    /// Settlement telemetry snapshot (DvP/PvP).
    pub settlement: SettlementStatusSnapshot,
    /// Total gossip fallback invocations (collectors exhausted).
    pub gossip_fallback_total: u64,
    /// Dedup eviction counters for inbound consensus traffic.
    pub dedup_evictions: DedupEvictionSnapshot,
    /// Consensus message drop/deferral counters with reason labels.
    pub consensus_message_handling: ConsensusMessageHandlingSnapshot,
    /// Recent vote validation drops with roster context.
    pub vote_validation_drops: VoteValidationDropSnapshot,
    /// Total background Post drops when the worker is unavailable.
    pub bg_post_drop_post_total: u64,
    /// Total background Broadcast drops when the worker is unavailable.
    pub bg_post_drop_broadcast_total: u64,
    /// Total `BlockCreated` drops due to locked QC gate.
    pub block_created_dropped_by_lock_total: u64,
    /// Total `BlockCreated` drops due to hint mismatches.
    pub block_created_hint_mismatch_total: u64,
    /// Total `BlockCreated` drops due to proposal mismatches.
    pub block_created_proposal_mismatch_total: u64,
    /// Total block-sync batches dropped due to invalid signatures.
    pub block_sync_drop_invalid_signatures_total: u64,
    /// Total block-sync QCs replaced after aggregate/signature mismatch.
    pub block_sync_qc_replaced_total: u64,
    /// Total block-sync QCs dropped because a local aggregate could not be derived.
    pub block_sync_qc_derive_failed_total: u64,
    /// Total blocks rejected by the validation gate before voting.
    pub validation_reject_total: u64,
    /// Last validation-reject reason label (best-effort).
    pub validation_reject_reason: Option<&'static str>,
    /// Validation gate reject breakdown and last occurrence details.
    pub validation_rejects: ValidationRejectSnapshot,
    /// Peer consensus-key policy reject breakdown and last occurrence.
    pub peer_key_policy: PeerKeyPolicySnapshot,
    /// Block-sync roster selection/drop counters.
    pub block_sync_roster: BlockSyncRosterSnapshot,
    /// Kura persistence failure snapshot.
    pub kura_store: KuraStoreSnapshot,
    /// Total missing-block fetch evaluations after QC-first arrival (including backoff/no-target cases).
    pub missing_block_fetch_total: u64,
    /// Target count on the most recent missing-block fetch attempt.
    pub missing_block_fetch_last_targets: u64,
    /// Dwell time in milliseconds observed before the most recent missing-block fetch attempt.
    pub missing_block_fetch_last_dwell_ms: u64,
    /// Data-availability gate snapshot and counters.
    pub da_gate: DaGateSnapshot,
    /// Total times pacemaker deferred proposal assembly due to proposal backpressure
    /// (queue saturation, relay/RBC backpressure, or blocking pending blocks).
    pub pacemaker_backpressure_deferrals_total: u64,
    /// Total times the commit pipeline executed from the pacemaker tick loop.
    pub commit_pipeline_tick_total: u64,
    /// Total prevote-quorum timeouts that triggered rebroadcast + view change.
    pub prevote_timeout_total: u64,
    /// Total DA deadline reschedules that pushed transactions into later slots.
    pub da_reschedule_total: u64,
    /// Total RBC DELIVER deferrals due to missing READY quorum.
    pub rbc_deliver_defer_ready_total: u64,
    /// Total RBC DELIVER deferrals due to missing chunks.
    pub rbc_deliver_defer_chunks_total: u64,
    /// Current number of persisted RBC sessions on disk.
    pub rbc_store_sessions: u64,
    /// Current persisted RBC payload bytes on disk.
    pub rbc_store_bytes: u64,
    /// Current RBC store pressure level (0 = normal, 1 = soft limit, 2 = hard limit).
    pub rbc_store_pressure_level: u8,
    /// Total number of times proposal assembly was deferred due to RBC store pressure.
    pub rbc_store_backpressure_deferrals_total: u64,
    /// Total number of RBC persist requests dropped due to full async queues.
    pub rbc_store_persist_drops_total: u64,
    /// Total number of RBC sessions evicted due to TTL or capacity enforcement.
    pub rbc_store_evictions_total: u64,
    /// RBC abort counters and last occurrence.
    pub rbc_abort: RbcAbortSnapshot,
    /// Per-peer RBC payload mismatch counters.
    pub rbc_mismatch: RbcMismatchSnapshot,
    /// Most recent RBC sessions evicted due to TTL or capacity enforcement (bounded list).
    pub rbc_store_recent_evictions: Vec<RbcEvictedSession>,
    /// Snapshot of pending (pre-INIT) RBC stashes.
    pub pending_rbc: PendingRbcSnapshot,
    /// Number of collectors targeted so far for the current voting block.
    pub collectors_targeted_current: u64,
    /// Number of collectors targeted for the most recently committed block.
    pub collectors_targeted_last_per_block: u64,
    /// Total redundant-send fan-out events observed locally.
    pub redundant_sends_total: u64,
    /// Current number of transactions observed in the local queue.
    pub tx_queue_depth: u64,
    /// Configured queue capacity on this peer.
    pub tx_queue_capacity: u64,
    /// Whether the local transaction queue is saturated.
    pub tx_queue_saturated: bool,
    /// Worker-loop stage and queue depth snapshot.
    pub worker_loop: WorkerLoopSnapshot,
    /// Commit inflight status for stall detection.
    pub commit_inflight: CommitInflightSnapshot,
    /// Transaction gossip telemetry snapshot.
    pub tx_gossip: TxGossipSnapshot,
    /// Total QC rebuild attempts triggered locally.
    pub qc_rebuild_attempts_total: u64,
    /// Total QC rebuild successes (QC cached after rebuild).
    pub qc_rebuild_successes_total: u64,
    /// Total QCs accepted using the signer bitmap when local votes were missing.
    pub qc_missing_votes_accepted_total: u64,
    /// Total times a quorum of votes was observed locally without a cached QC due to missing payloads.
    pub qc_quorum_without_qc_total: u64,
    /// Epoch length in blocks (`NPoS` mode; zero when not applicable).
    pub epoch_length_blocks: u64,
    /// Commit window deadline offset from epoch start (blocks; zero when not applicable).
    pub epoch_commit_deadline_offset: u64,
    /// Reveal window deadline offset from epoch start (blocks; zero when not applicable).
    pub epoch_reveal_deadline_offset: u64,
    /// PRF epoch seed used for deterministic leader/collector selection (`NPoS` mode).
    pub prf_epoch_seed: Option<[u8; 32]>,
    /// Height associated with the recorded PRF context.
    pub prf_height: u64,
    /// View associated with the recorded PRF context.
    pub prf_view: u64,
    /// Height associated with the latest membership view hash snapshot.
    pub membership_height: u64,
    /// View associated with the latest membership view hash snapshot.
    pub membership_view: u64,
    /// Epoch associated with the latest membership view hash snapshot.
    pub membership_epoch: u64,
    /// Deterministic membership view hash (present once computed).
    pub membership_view_hash: Option<[u8; 32]>,
    /// Membership mismatch snapshot.
    pub membership_mismatch: MembershipMismatchSnapshot,
    /// Latest epoch index for which VRF penalties were recorded.
    pub vrf_penalty_epoch: u64,
    /// Count of validators that committed without revealing in the latest epoch snapshot.
    pub vrf_non_reveal_total: u64,
    /// Count of validators that neither committed nor revealed in the latest epoch snapshot.
    pub vrf_no_participation_total: u64,
    /// Count of validators that submitted late reveals in the latest epoch snapshot.
    pub vrf_late_reveals_total: u64,
    /// Total consensus penalties applied due to evidence (slashing/jailing).
    pub consensus_penalties_applied_total: u64,
    /// Number of consensus evidence records waiting for activation before applying penalties.
    pub consensus_penalties_pending: u64,
    /// Total VRF penalties applied.
    pub vrf_penalties_applied_total: u64,
    /// Number of VRF penalty snapshots waiting for activation.
    pub vrf_penalties_pending: u64,
    /// Summary of access-set sources used for IVM transactions in the latest block.
    pub access_set_sources: AccessSetSourceSummary,
    /// Conflict rate in basis points for the latest validated block.
    pub pipeline_conflict_rate_bps: u64,
    /// Latest per-lane execution snapshot for the block pipeline.
    pub lane_activity: Vec<LaneActivitySnapshot>,
    /// Latest per-dataspace execution snapshot for the block pipeline.
    pub dataspace_activity: Vec<DataspaceActivitySnapshot>,
    /// Aggregated RBC backlog per lane across active sessions.
    pub rbc_lane_backlog: Vec<LaneRbcSnapshot>,
    /// Aggregated RBC backlog per dataspace across active sessions.
    pub rbc_dataspace_backlog: Vec<DataspaceRbcSnapshot>,
    /// Aggregated lane commitment summaries for recently committed blocks.
    pub lane_commitments: Vec<LaneCommitmentSnapshot>,
    /// Aggregated dataspace commitment summaries for recently committed blocks.
    pub dataspace_commitments: Vec<DataspaceCommitmentSnapshot>,
    /// Aggregated lane settlement commitments for recently committed blocks.
    pub lane_settlement_commitments: Vec<LaneBlockCommitment>,
    /// Latest lane relay envelopes captured during block sealing.
    pub lane_relay_envelopes: Vec<LaneRelayEnvelope>,
    /// Number of lanes that remain sealed awaiting governance manifests.
    pub lane_governance_sealed_total: u32,
    /// Aliases of lanes that remain sealed awaiting governance manifests.
    pub lane_governance_sealed_aliases: Vec<String>,
    /// Governance manifest readiness per lane.
    pub lane_governance: Vec<LaneGovernanceSnapshot>,
    /// Aggregated Nexus fee debit outcomes.
    pub nexus_fee: NexusFeeSnapshot,
    /// Aggregated Nexus public-lane staking status.
    pub nexus_staking: NexusStakingSnapshot,
    /// Latest validator election outcome (`NPoS`).
    pub npos_election: Option<ValidatorElectionOutcome>,
}

impl StatusSnapshot {
    /// Drop lane/dataspace snapshots when Nexus is disabled.
    #[must_use]
    pub fn strip_lane_details(mut self) -> Self {
        self.lane_activity.clear();
        self.dataspace_activity.clear();
        self.rbc_lane_backlog.clear();
        self.rbc_dataspace_backlog.clear();
        self.lane_commitments.clear();
        self.dataspace_commitments.clear();
        self.lane_settlement_commitments.clear();
        self.lane_relay_envelopes.clear();
        self.lane_governance_sealed_total = 0;
        self.lane_governance_sealed_aliases.clear();
        self.lane_governance.clear();
        self.nexus_fee = NexusFeeSnapshot::default();
        self.nexus_staking = NexusStakingSnapshot::default();
        self.npos_election = None;
        self
    }
}

fn checkpoint_history_slot() -> &'static Mutex<VecDeque<ValidatorSetCheckpoint>> {
    VALIDATOR_CHECKPOINT_HISTORY.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn key_history_slot() -> &'static Mutex<VecDeque<ConsensusKeyRecord>> {
    KEY_LIFECYCLE_HISTORY.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn commit_cert_history_slot() -> &'static Mutex<VecDeque<Qc>> {
    COMMIT_CERT_HISTORY.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Snapshot of precommit quorum signers for a committed block.
#[derive(Clone, Debug)]
pub struct PrecommitSignerRecord {
    /// Block hash certified by the precommit QC.
    pub block_hash: HashOf<BlockHeader>,
    /// Height of the certified block.
    pub height: u64,
    /// View in which the QC was formed.
    pub view: u64,
    /// Epoch (permissioned mode uses zero).
    pub epoch: u64,
    /// Parent state root bound into commit vote preimages.
    pub parent_state_root: Hash,
    /// Post-state root bound into commit vote preimages.
    pub post_state_root: Hash,
    /// Signers that participated in the precommit QC (canonical validator-set order).
    pub signers: BTreeSet<ValidatorIndex>,
    /// Aggregate BLS signature covering the precommit vote preimage.
    pub bls_aggregate_signature: Vec<u8>,
    /// Roster length used when forming the QC.
    pub roster_len: usize,
    /// Mode tag used when forming the QC.
    pub mode_tag: String,
    /// Ordered validator set used when forming the QC.
    pub validator_set: Vec<PeerId>,
    /// Stake snapshot aligned to the validator set (`NPoS` only).
    pub stake_snapshot: Option<CommitStakeSnapshot>,
}

fn precommit_signer_history_slot() -> &'static Mutex<VecDeque<PrecommitSignerRecord>> {
    PRECOMMIT_SIGNER_HISTORY.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Record a validator-set checkpoint, retaining a bounded history (newest-last order).
pub fn record_validator_checkpoint(checkpoint: ValidatorSetCheckpoint) {
    #[cfg(test)]
    let _guard = commit_history_test_guard();
    let mut guard = checkpoint_history_slot()
        .lock()
        .expect("validator checkpoint history mutex poisoned");
    guard.push_back(checkpoint);
    while guard.len() > VALIDATOR_CHECKPOINT_HISTORY_CAP {
        guard.pop_front();
    }
}

/// Return validator-set checkpoints in newest-first order.
#[must_use]
pub fn validator_checkpoint_history() -> Vec<ValidatorSetCheckpoint> {
    #[cfg(test)]
    let _guard = commit_history_test_guard();
    checkpoint_history_slot()
        .lock()
        .expect("validator checkpoint history mutex poisoned")
        .iter()
        .rev()
        .cloned()
        .collect()
}

/// Record a commit certificate, retaining a bounded history (newest-last order).
pub fn record_commit_qc(cert: Qc) {
    #[cfg(test)]
    let _guard = commit_history_test_guard();
    let mut guard = commit_cert_history_slot()
        .lock()
        .expect("commit certificate history mutex poisoned");
    // Keep the latest certificate per (height, block_hash) to avoid stale duplicates while
    // preserving alternate views for other hashes at the same height.
    guard.retain(|entry| {
        !(entry.height == cert.height
            && entry.subject_block_hash == cert.subject_block_hash
            && entry.view <= cert.view)
    });
    guard.push_back(cert);
    while guard.len() > commit_cert_history_cap() {
        guard.pop_front();
    }
}

/// Return commit certificates in newest-first order.
#[must_use]
pub fn commit_qc_history() -> Vec<Qc> {
    #[cfg(test)]
    let _guard = commit_history_test_guard();
    let mut entries: Vec<_> = commit_cert_history_slot()
        .lock()
        .expect("commit certificate history mutex poisoned")
        .iter()
        .cloned()
        .collect();
    entries.sort_by(|a, b| b.height.cmp(&a.height).then_with(|| b.view.cmp(&a.view)));
    entries
}

/// Record precommit signer set for a block (best-effort), retaining a bounded history.
pub fn record_precommit_signers(record: PrecommitSignerRecord) {
    #[cfg(test)]
    let _guard = commit_history_test_guard();
    let mut guard = precommit_signer_history_slot()
        .lock()
        .expect("precommit signer history mutex poisoned");
    guard.retain(|entry| {
        !(entry.height == record.height
            && entry.block_hash == record.block_hash
            && entry.view <= record.view)
    });
    guard.push_back(record);
    while guard.len() > commit_cert_history_cap() {
        guard.pop_front();
    }
}

/// Return precommit signer records in newest-first order.
#[must_use]
pub fn precommit_signer_history() -> Vec<PrecommitSignerRecord> {
    #[cfg(test)]
    let _guard = commit_history_test_guard();
    let mut entries: Vec<_> = precommit_signer_history_slot()
        .lock()
        .expect("precommit signer history mutex poisoned")
        .iter()
        .cloned()
        .collect();
    entries.sort_by(|a, b| b.height.cmp(&a.height).then_with(|| b.view.cmp(&a.view)));
    entries
}

/// Fetch the latest precommit signer record for the given block hash, if any.
#[must_use]
pub fn precommit_signers_for(block_hash: HashOf<BlockHeader>) -> Option<PrecommitSignerRecord> {
    precommit_signer_history()
        .into_iter()
        .find(|entry| entry.block_hash == block_hash)
}

fn npos_election_history_slot() -> &'static Mutex<VecDeque<ValidatorElectionOutcome>> {
    NPOS_ELECTION_HISTORY.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Record a validator election outcome, retaining a bounded history (newest-last order).
pub fn record_npos_election(outcome: ValidatorElectionOutcome) {
    let mut guard = npos_election_history_slot()
        .lock()
        .expect("npos election history mutex poisoned");
    guard.push_back(outcome);
    while guard.len() > NPOS_ELECTION_HISTORY_CAP {
        guard.pop_front();
    }
}

/// Return the most recent validator election outcome, if present.
#[must_use]
pub fn latest_npos_election() -> Option<ValidatorElectionOutcome> {
    npos_election_history_slot()
        .lock()
        .expect("npos election history mutex poisoned")
        .iter()
        .next_back()
        .cloned()
}

/// Record a consensus key lifecycle entry, retaining a bounded history (newest-last order).
pub fn record_consensus_key(record: ConsensusKeyRecord) {
    let mut guard = key_history_slot()
        .lock()
        .expect("key lifecycle history mutex poisoned");
    guard.retain(|existing| existing.id != record.id);
    guard.push_back(record);
    while guard.len() > KEY_LIFECYCLE_HISTORY_CAP {
        guard.pop_front();
    }
}

/// Return recorded consensus key lifecycle entries in newest-first order.
#[must_use]
pub fn consensus_key_history() -> Vec<ConsensusKeyRecord> {
    key_history_slot()
        .lock()
        .expect("key lifecycle history mutex poisoned")
        .iter()
        .rev()
        .cloned()
        .collect()
}

#[cfg(test)]
/// Clear validator checkpoint history (test-only helper).
pub fn reset_validator_checkpoints_for_tests() {
    let _guard = commit_history_test_guard();
    let mut guard = checkpoint_history_slot()
        .lock()
        .expect("validator checkpoint history mutex poisoned");
    guard.clear();
}

/// Clear commit certificate history (test-only helper).
pub fn reset_commit_certs_for_tests() {
    #[cfg(test)]
    let _guard = commit_history_test_guard();
    let mut guard = commit_cert_history_slot()
        .lock()
        .expect("commit certificate history mutex poisoned");
    guard.clear();
}

#[cfg(test)]
/// Clear commit quorum snapshot counters (test-only helper).
pub fn reset_commit_quorum_for_tests() {
    COMMIT_QUORUM_HEIGHT.store(0, Ordering::Relaxed);
    COMMIT_QUORUM_VIEW.store(0, Ordering::Relaxed);
    COMMIT_QUORUM_PRESENT.store(0, Ordering::Relaxed);
    COMMIT_QUORUM_COUNTED.store(0, Ordering::Relaxed);
    COMMIT_QUORUM_SET_B.store(0, Ordering::Relaxed);
    COMMIT_QUORUM_REQUIRED.store(0, Ordering::Relaxed);
    COMMIT_QUORUM_LAST_UPDATED_MS.store(0, Ordering::Relaxed);
    if let Ok(mut guard) = commit_quorum_hash_slot().lock() {
        *guard = None;
    }
}

#[cfg(test)]
/// Clear `NPoS` election history (test-only helper).
pub fn reset_npos_elections_for_tests() {
    let mut guard = npos_election_history_slot()
        .lock()
        .expect("npos election history mutex poisoned");
    guard.clear();
}

#[cfg(test)]
/// Clear key lifecycle history (test-only helper).
pub fn reset_consensus_keys_for_tests() {
    let mut guard = key_history_slot()
        .lock()
        .expect("key lifecycle history mutex poisoned");
    guard.clear();
}

fn recent_rbc_evictions() -> Vec<RbcEvictedSession> {
    RBC_STORE_RECENT_EVICTIONS
        .get()
        .map(|slot| {
            let guard = slot
                .lock()
                .expect("RBC store eviction history mutex poisoned");
            guard.iter().rev().map(Clone::clone).collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn lane_governance_sealed_summary() -> (u32, Vec<String>, Vec<LaneGovernanceSnapshot>) {
    let lane_governance_entries = lane_governance_snapshot();
    let lane_governance_sealed_aliases: Vec<String> = lane_governance_entries
        .iter()
        .filter(|entry| entry.manifest_required && !entry.manifest_ready)
        .map(|entry| entry.alias.clone())
        .collect();
    let lane_governance_sealed_total =
        u32::try_from(lane_governance_sealed_aliases.len()).unwrap_or(u32::MAX);
    (
        lane_governance_sealed_total,
        lane_governance_sealed_aliases,
        lane_governance_entries,
    )
}

fn penalty_totals() -> (u64, u64, u64, u64) {
    penalty_counters()
}

fn da_gate_snapshot() -> DaGateSnapshot {
    DaGateSnapshot {
        reason: DaGateReasonSnapshot::from_code(DA_GATE_LAST_REASON.load(Ordering::Relaxed)),
        last_satisfied: DaGateSatisfactionSnapshot::from_code(
            DA_GATE_LAST_SATISFIED.load(Ordering::Relaxed),
        ),
        missing_local_data_total: DA_GATE_MISSING_LOCAL_DATA_TOTAL.load(Ordering::Relaxed),
        manifest_guard_total: DA_GATE_MANIFEST_GUARD_TOTAL.load(Ordering::Relaxed),
    }
}

fn block_sync_roster_snapshot() -> BlockSyncRosterSnapshot {
    BlockSyncRosterSnapshot {
        commit_qc_hint_total: BLOCK_SYNC_ROSTER_COMMIT_CERT_HINT_TOTAL.load(Ordering::Relaxed),
        checkpoint_hint_total: BLOCK_SYNC_ROSTER_CHECKPOINT_HINT_TOTAL.load(Ordering::Relaxed),
        commit_qc_history_total: BLOCK_SYNC_ROSTER_COMMIT_CERT_HISTORY_TOTAL
            .load(Ordering::Relaxed),
        checkpoint_history_total: BLOCK_SYNC_ROSTER_CHECKPOINT_HISTORY_TOTAL
            .load(Ordering::Relaxed),
        roster_sidecar_total: BLOCK_SYNC_ROSTER_ROSTER_SIDECAR_TOTAL.load(Ordering::Relaxed),
        commit_roster_journal_total: BLOCK_SYNC_ROSTER_COMMIT_ROSTER_JOURNAL_TOTAL
            .load(Ordering::Relaxed),
        drop_missing_total: BLOCK_SYNC_ROSTER_DROP_MISSING_TOTAL.load(Ordering::Relaxed),
        drop_unsolicited_share_blocks_total: BLOCK_SYNC_ROSTER_DROP_UNSOLICITED_SHARE_BLOCKS_TOTAL
            .load(Ordering::Relaxed),
    }
}

fn view_change_cause_snapshot() -> ViewChangeCauseSnapshot {
    let last_ts = VIEW_CHANGE_CAUSE_LAST_TS_MS.load(Ordering::Relaxed);
    let last_cause = VIEW_CHANGE_CAUSE_LAST_LABEL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("view change cause mutex poisoned")
        .clone();
    ViewChangeCauseSnapshot {
        commit_failure_total: VIEW_CHANGE_CAUSE_COMMIT_FAILURE_TOTAL.load(Ordering::Relaxed),
        quorum_timeout_total: VIEW_CHANGE_CAUSE_QUORUM_TIMEOUT_TOTAL.load(Ordering::Relaxed),
        stake_quorum_timeout_total: VIEW_CHANGE_CAUSE_STAKE_QUORUM_TIMEOUT_TOTAL
            .load(Ordering::Relaxed),
        da_gate_total: VIEW_CHANGE_CAUSE_DA_GATE_TOTAL.load(Ordering::Relaxed),
        censorship_evidence_total: VIEW_CHANGE_CAUSE_CENSORSHIP_EVIDENCE_TOTAL
            .load(Ordering::Relaxed),
        missing_payload_total: VIEW_CHANGE_CAUSE_MISSING_PAYLOAD_TOTAL.load(Ordering::Relaxed),
        missing_qc_total: VIEW_CHANGE_CAUSE_MISSING_QC_TOTAL.load(Ordering::Relaxed),
        validation_reject_total: VIEW_CHANGE_CAUSE_VALIDATION_REJECT_TOTAL.load(Ordering::Relaxed),
        last_cause,
        last_cause_timestamp_ms: last_ts,
        last_commit_failure_timestamp_ms: VIEW_CHANGE_CAUSE_LAST_COMMIT_FAILURE_TS_MS
            .load(Ordering::Relaxed),
        last_quorum_timeout_timestamp_ms: VIEW_CHANGE_CAUSE_LAST_QUORUM_TIMEOUT_TS_MS
            .load(Ordering::Relaxed),
        last_stake_quorum_timeout_timestamp_ms: VIEW_CHANGE_CAUSE_LAST_STAKE_QUORUM_TIMEOUT_TS_MS
            .load(Ordering::Relaxed),
        last_da_gate_timestamp_ms: VIEW_CHANGE_CAUSE_LAST_DA_GATE_TS_MS.load(Ordering::Relaxed),
        last_censorship_evidence_timestamp_ms: VIEW_CHANGE_CAUSE_LAST_CENSORSHIP_EVIDENCE_TS_MS
            .load(Ordering::Relaxed),
        last_missing_payload_timestamp_ms: VIEW_CHANGE_CAUSE_LAST_MISSING_PAYLOAD_TS_MS
            .load(Ordering::Relaxed),
        last_missing_qc_timestamp_ms: VIEW_CHANGE_CAUSE_LAST_MISSING_QC_TS_MS
            .load(Ordering::Relaxed),
        last_validation_reject_timestamp_ms: VIEW_CHANGE_CAUSE_LAST_VALIDATION_REJECT_TS_MS
            .load(Ordering::Relaxed),
    }
}

fn validation_reject_snapshot() -> ValidationRejectSnapshot {
    let last_reason = VALIDATION_REJECT_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("validation reject reason mutex poisoned");
    let last_height = VALIDATION_REJECT_LAST_HEIGHT
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("validation reject height mutex poisoned");
    let last_view = VALIDATION_REJECT_LAST_VIEW
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("validation reject view mutex poisoned");
    let last_block = VALIDATION_REJECT_LAST_BLOCK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("validation reject block mutex poisoned");

    ValidationRejectSnapshot {
        total: VALIDATION_REJECT_TOTAL.load(Ordering::Relaxed),
        stateless_total: VALIDATION_REJECT_STATELESS_TOTAL.load(Ordering::Relaxed),
        execution_total: VALIDATION_REJECT_EXECUTION_TOTAL.load(Ordering::Relaxed),
        prev_hash_total: VALIDATION_REJECT_PREV_HASH_TOTAL.load(Ordering::Relaxed),
        prev_height_total: VALIDATION_REJECT_PREV_HEIGHT_TOTAL.load(Ordering::Relaxed),
        topology_total: VALIDATION_REJECT_TOPOLOGY_TOTAL.load(Ordering::Relaxed),
        last_reason: *last_reason,
        last_height: *last_height,
        last_view: *last_view,
        last_block: last_block
            .as_ref()
            .map(|hash| HashOf::from_untyped_unchecked(*hash)),
        last_timestamp_ms: VALIDATION_REJECT_LAST_TS_MS.load(Ordering::Relaxed),
    }
}

fn peer_key_policy_snapshot() -> PeerKeyPolicySnapshot {
    let last_reason = PEER_KEY_POLICY_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("peer key policy reason mutex poisoned");

    PeerKeyPolicySnapshot {
        total: PEER_KEY_POLICY_REJECT_TOTAL.load(Ordering::Relaxed),
        missing_hsm_total: PEER_KEY_POLICY_MISSING_HSM_TOTAL.load(Ordering::Relaxed),
        disallowed_algorithm_total: PEER_KEY_POLICY_ALGO_TOTAL.load(Ordering::Relaxed),
        disallowed_provider_total: PEER_KEY_POLICY_PROVIDER_TOTAL.load(Ordering::Relaxed),
        lead_time_violation_total: PEER_KEY_POLICY_LEAD_TIME_TOTAL.load(Ordering::Relaxed),
        activation_in_past_total: PEER_KEY_POLICY_ACTIVATION_PAST_TOTAL.load(Ordering::Relaxed),
        expiry_before_activation_total: PEER_KEY_POLICY_EXPIRY_TOTAL.load(Ordering::Relaxed),
        identifier_collision_total: PEER_KEY_POLICY_ID_COLLISION_TOTAL.load(Ordering::Relaxed),
        last_reason: *last_reason,
        last_timestamp_ms: PEER_KEY_POLICY_LAST_TS_MS.load(Ordering::Relaxed),
    }
}

fn dedup_evictions_snapshot() -> DedupEvictionSnapshot {
    DedupEvictionSnapshot {
        vote_capacity_total: DEDUP_VOTE_EVICT_CAPACITY_TOTAL.load(Ordering::Relaxed),
        vote_expired_total: DEDUP_VOTE_EVICT_EXPIRED_TOTAL.load(Ordering::Relaxed),
        block_created_capacity_total: DEDUP_BLOCK_CREATED_EVICT_CAPACITY_TOTAL
            .load(Ordering::Relaxed),
        block_created_expired_total: DEDUP_BLOCK_CREATED_EVICT_EXPIRED_TOTAL
            .load(Ordering::Relaxed),
        proposal_capacity_total: DEDUP_PROPOSAL_EVICT_CAPACITY_TOTAL.load(Ordering::Relaxed),
        proposal_expired_total: DEDUP_PROPOSAL_EVICT_EXPIRED_TOTAL.load(Ordering::Relaxed),
        rbc_init_capacity_total: DEDUP_RBC_INIT_EVICT_CAPACITY_TOTAL.load(Ordering::Relaxed),
        rbc_init_expired_total: DEDUP_RBC_INIT_EVICT_EXPIRED_TOTAL.load(Ordering::Relaxed),
        block_sync_update_capacity_total: DEDUP_BLOCK_SYNC_UPDATE_EVICT_CAPACITY_TOTAL
            .load(Ordering::Relaxed),
        block_sync_update_expired_total: DEDUP_BLOCK_SYNC_UPDATE_EVICT_EXPIRED_TOTAL
            .load(Ordering::Relaxed),
        fetch_pending_block_capacity_total: DEDUP_FETCH_PENDING_BLOCK_EVICT_CAPACITY_TOTAL
            .load(Ordering::Relaxed),
        fetch_pending_block_expired_total: DEDUP_FETCH_PENDING_BLOCK_EVICT_EXPIRED_TOTAL
            .load(Ordering::Relaxed),
        rbc_ready_capacity_total: DEDUP_RBC_READY_EVICT_CAPACITY_TOTAL.load(Ordering::Relaxed),
        rbc_ready_expired_total: DEDUP_RBC_READY_EVICT_EXPIRED_TOTAL.load(Ordering::Relaxed),
        rbc_deliver_capacity_total: DEDUP_RBC_DELIVER_EVICT_CAPACITY_TOTAL.load(Ordering::Relaxed),
        rbc_deliver_expired_total: DEDUP_RBC_DELIVER_EVICT_EXPIRED_TOTAL.load(Ordering::Relaxed),
        rbc_chunk_capacity_total: DEDUP_RBC_CHUNK_EVICT_CAPACITY_TOTAL.load(Ordering::Relaxed),
        rbc_chunk_expired_total: DEDUP_RBC_CHUNK_EVICT_EXPIRED_TOTAL.load(Ordering::Relaxed),
    }
}

fn consensus_message_handling_snapshot() -> ConsensusMessageHandlingSnapshot {
    let Some(slot) = MESSAGE_HANDLING_TOTALS.get() else {
        return ConsensusMessageHandlingSnapshot::default();
    };
    let guard = slot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entries = guard
        .iter()
        .map(|(key, total)| ConsensusMessageHandlingEntry {
            kind: key.kind,
            outcome: key.outcome,
            reason: key.reason,
            total: *total,
        })
        .collect();
    ConsensusMessageHandlingSnapshot { entries }
}

fn vote_validation_drop_peer_registry() -> Option<
    std::sync::MutexGuard<
        'static,
        BTreeMap<VoteValidationDropPeerKey, VoteValidationDropPeerState>,
    >,
> {
    VOTE_VALIDATION_DROPS_BY_PEER
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .ok()
}

fn vote_validation_drop_peer_entries() -> Vec<VoteValidationDropPeerEntry> {
    let Some(registry) = vote_validation_drop_peer_registry() else {
        return Vec::new();
    };
    registry
        .iter()
        .map(|(key, entry)| VoteValidationDropPeerEntry {
            peer_id: key.peer_id.clone(),
            roster_hash: key.roster_hash,
            roster_len: entry.roster_len,
            total: entry.total,
            reasons: entry
                .reasons
                .iter()
                .map(|(reason, total)| VoteValidationDropReasonCount {
                    reason: *reason,
                    total: *total,
                })
                .collect(),
            last_height: entry.last_height,
            last_view: entry.last_view,
            last_epoch: entry.last_epoch,
            last_timestamp_ms: entry.last_timestamp_ms,
        })
        .collect()
}

fn vote_validation_drop_snapshot() -> VoteValidationDropSnapshot {
    let entries = VOTE_VALIDATION_DROPS
        .get()
        .map(|slot| {
            let guard = slot
                .lock()
                .expect("vote validation drop history mutex poisoned");
            guard.iter().rev().cloned().collect::<Vec<_>>()
        })
        .unwrap_or_default();
    VoteValidationDropSnapshot {
        total: VOTE_VALIDATION_DROPS_TOTAL.load(Ordering::Relaxed),
        entries,
        peer_entries: vote_validation_drop_peer_entries(),
    }
}

/// Snapshot the current status: leader index, Highest/Locked QC, and drop counters.
#[allow(clippy::too_many_lines)]
pub fn snapshot() -> StatusSnapshot {
    let (tx_queue_depth, tx_queue_capacity, tx_queue_saturated) = tx_queue_backpressure();
    let worker_loop = worker_loop_snapshot();
    let commit_inflight = commit_inflight_snapshot();
    let (prf_seed, prf_height, prf_view) = prf_context();
    let (vrf_epoch, vrf_non_reveal, vrf_no_participation, vrf_late_reveals) =
        vrf_penalty_snapshot();
    let (mode_tag, staged_mode_tag, staged_mode_activation_height, mode_activation_lag_blocks) =
        mode_tags();
    let epoch_length_blocks = EPOCH_LENGTH_BLOCKS.load(Ordering::Relaxed);
    let epoch_commit_deadline_offset = EPOCH_COMMIT_DEADLINE_OFFSET.load(Ordering::Relaxed);
    let epoch_reveal_deadline_offset = EPOCH_REVEAL_DEADLINE_OFFSET.load(Ordering::Relaxed);
    let recent_evictions = recent_rbc_evictions();
    let pending_rbc = pending_rbc_snapshot();
    let (lane_governance_sealed_total, lane_governance_sealed_aliases, lane_governance_entries) =
        lane_governance_sealed_summary();
    let lane_relay_envelopes = lane_relay_envelopes_snapshot();
    let kura_last_hash = (*KURA_STORE_LAST_HASH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura store failure hash mutex poisoned"))
    .map(HashOf::from_untyped_unchecked);
    let kura_stage_last_hash = (*KURA_STAGE_LAST_HASH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura stage hash mutex poisoned"))
    .map(HashOf::from_untyped_unchecked);
    let kura_stage_rollback_last_hash = (*KURA_STAGE_ROLLBACK_LAST_HASH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura stage rollback hash mutex poisoned"))
    .map(HashOf::from_untyped_unchecked);
    let kura_stage_rollback_last_reason = *KURA_STAGE_ROLLBACK_LAST_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura stage rollback reason mutex poisoned");
    let kura_lock_reset_last_hash = (*KURA_LOCK_RESET_LAST_HASH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura lock reset hash mutex poisoned"))
    .map(HashOf::from_untyped_unchecked);
    let kura_lock_reset_last_reason = *KURA_LOCK_RESET_LAST_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura lock reset reason mutex poisoned");
    let (
        consensus_penalties_applied_total,
        consensus_penalties_pending,
        vrf_penalties_applied_total,
        vrf_penalties_pending,
    ) = penalty_totals();
    let validation_rejects = validation_reject_snapshot();
    let peer_key_policy = peer_key_policy_snapshot();
    let consensus_caps = consensus_caps();
    let mode_flip_kill_switch = MODE_FLIP_KILL_SWITCH.load(Ordering::Relaxed);
    let mode_flip_blocked = MODE_FLIP_BLOCKED.load(Ordering::Relaxed);
    let last_flip_timestamp_ms = MODE_LAST_FLIP_TS_SET
        .load(Ordering::Relaxed)
        .then(|| MODE_LAST_FLIP_TS_MS.load(Ordering::Relaxed));
    let last_flip_error = MODE_LAST_FLIP_ERROR
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|err| err.clone()));

    StatusSnapshot {
        mode_tag,
        staged_mode_tag,
        staged_mode_activation_height,
        consensus_caps,
        mode_activation_lag_blocks,
        mode_flip_kill_switch,
        mode_flip_blocked,
        mode_flip_success_total: MODE_FLIP_SUCCESS_TOTAL.load(Ordering::Relaxed),
        mode_flip_fail_total: MODE_FLIP_FAIL_TOTAL.load(Ordering::Relaxed),
        mode_flip_blocked_total: MODE_FLIP_BLOCKED_TOTAL.load(Ordering::Relaxed),
        last_mode_flip_timestamp_ms: last_flip_timestamp_ms,
        last_mode_flip_error: last_flip_error,
        leader_index: LEADER_INDEX.load(Ordering::Relaxed),
        view_change_index: VIEW_CHANGE_INDEX.load(Ordering::Relaxed),
        view_change_proof_accepted_total: VIEW_CHANGE_PROOF_ACCEPTED_TOTAL.load(Ordering::Relaxed),
        view_change_proof_stale_total: VIEW_CHANGE_PROOF_STALE_TOTAL.load(Ordering::Relaxed),
        view_change_proof_rejected_total: VIEW_CHANGE_PROOF_REJECTED_TOTAL.load(Ordering::Relaxed),
        view_change_suggest_total: VIEW_CHANGE_SUGGEST_TOTAL.load(Ordering::Relaxed),
        view_change_install_total: VIEW_CHANGE_INSTALL_TOTAL.load(Ordering::Relaxed),
        view_change_causes: view_change_cause_snapshot(),
        highest_qc_height: HIGHEST_QC_HEIGHT.load(Ordering::Relaxed),
        highest_qc_view: HIGHEST_QC_VIEW.load(Ordering::Relaxed),
        locked_qc_height: LOCKED_QC_HEIGHT.load(Ordering::Relaxed),
        locked_qc_view: LOCKED_QC_VIEW.load(Ordering::Relaxed),
        highest_qc_subject: highest_qc_hash(),
        locked_qc_subject: locked_qc_hash(),
        commit_qc: commit_qc_snapshot(),
        commit_quorum: commit_quorum_snapshot(),
        settlement: settlement_snapshot(),
        gossip_fallback_total: GOSSIP_FALLBACK_TOTAL.load(Ordering::Relaxed),
        dedup_evictions: dedup_evictions_snapshot(),
        consensus_message_handling: consensus_message_handling_snapshot(),
        vote_validation_drops: vote_validation_drop_snapshot(),
        bg_post_drop_post_total: BG_POST_DROP_POST_TOTAL.load(Ordering::Relaxed),
        bg_post_drop_broadcast_total: BG_POST_DROP_BROADCAST_TOTAL.load(Ordering::Relaxed),
        block_created_dropped_by_lock_total: BLOCK_CREATED_DROPPED_BY_LOCK_TOTAL
            .load(Ordering::Relaxed),
        block_created_hint_mismatch_total: BLOCK_CREATED_HINT_MISMATCH_TOTAL
            .load(Ordering::Relaxed),
        block_created_proposal_mismatch_total: BLOCK_CREATED_PROPOSAL_MISMATCH_TOTAL
            .load(Ordering::Relaxed),
        block_sync_drop_invalid_signatures_total: BLOCK_SYNC_DROP_INVALID_SIGNATURES_TOTAL
            .load(Ordering::Relaxed),
        block_sync_qc_replaced_total: BLOCK_SYNC_QC_REPLACED_TOTAL.load(Ordering::Relaxed),
        block_sync_qc_derive_failed_total: BLOCK_SYNC_QC_DERIVE_FAILED_TOTAL
            .load(Ordering::Relaxed),
        validation_reject_total: validation_rejects.total,
        validation_reject_reason: validation_rejects.last_reason,
        validation_rejects,
        peer_key_policy,
        block_sync_roster: block_sync_roster_snapshot(),
        kura_store: KuraStoreSnapshot {
            failures_total: KURA_STORE_FAILURE_TOTAL.load(Ordering::Relaxed),
            abort_total: KURA_STORE_ABORT_TOTAL.load(Ordering::Relaxed),
            stage_total: KURA_STAGE_TOTAL.load(Ordering::Relaxed),
            rollback_total: KURA_STAGE_ROLLBACK_TOTAL.load(Ordering::Relaxed),
            stage_last_height: KURA_STAGE_LAST_HEIGHT.load(Ordering::Relaxed),
            stage_last_view: KURA_STAGE_LAST_VIEW.load(Ordering::Relaxed),
            stage_last_hash: kura_stage_last_hash,
            rollback_last_height: KURA_STAGE_ROLLBACK_LAST_HEIGHT.load(Ordering::Relaxed),
            rollback_last_view: KURA_STAGE_ROLLBACK_LAST_VIEW.load(Ordering::Relaxed),
            rollback_last_hash: kura_stage_rollback_last_hash,
            rollback_last_reason: kura_stage_rollback_last_reason,
            lock_reset_total: KURA_LOCK_RESET_TOTAL.load(Ordering::Relaxed),
            lock_reset_last_height: KURA_LOCK_RESET_LAST_HEIGHT.load(Ordering::Relaxed),
            lock_reset_last_view: KURA_LOCK_RESET_LAST_VIEW.load(Ordering::Relaxed),
            lock_reset_last_hash: kura_lock_reset_last_hash,
            lock_reset_last_reason: kura_lock_reset_last_reason,
            last_retry_attempt: KURA_STORE_LAST_RETRY_ATTEMPT.load(Ordering::Relaxed),
            last_retry_backoff_ms: KURA_STORE_LAST_RETRY_BACKOFF_MS.load(Ordering::Relaxed),
            last_height: KURA_STORE_LAST_HEIGHT.load(Ordering::Relaxed),
            last_view: KURA_STORE_LAST_VIEW.load(Ordering::Relaxed),
            last_hash: kura_last_hash,
        },
        missing_block_fetch_total: MISSING_BLOCK_FETCH_TOTAL.load(Ordering::Relaxed),
        missing_block_fetch_last_targets: MISSING_BLOCK_FETCH_LAST_TARGETS.load(Ordering::Relaxed),
        missing_block_fetch_last_dwell_ms: MISSING_BLOCK_FETCH_LAST_DWELL_MS
            .load(Ordering::Relaxed),
        da_gate: da_gate_snapshot(),
        pacemaker_backpressure_deferrals_total: PACEMAKER_BACKPRESSURE_DEFERRALS_TOTAL
            .load(Ordering::Relaxed),
        commit_pipeline_tick_total: COMMIT_PIPELINE_TICK_TOTAL.load(Ordering::Relaxed),
        prevote_timeout_total: PREVOTE_TIMEOUT_TOTAL.load(Ordering::Relaxed),
        da_reschedule_total: da_reschedule_total(),
        rbc_deliver_defer_ready_total: RBC_DELIVER_DEFER_READY_TOTAL.load(Ordering::Relaxed),
        rbc_deliver_defer_chunks_total: RBC_DELIVER_DEFER_CHUNKS_TOTAL.load(Ordering::Relaxed),
        rbc_store_sessions: RBC_STORE_SESSIONS.load(Ordering::Relaxed),
        rbc_store_bytes: RBC_STORE_BYTES.load(Ordering::Relaxed),
        rbc_store_pressure_level: RBC_STORE_PRESSURE_LEVEL.load(Ordering::Relaxed),
        rbc_store_backpressure_deferrals_total: RBC_STORE_BACKPRESSURE_DEFERRALS_TOTAL
            .load(Ordering::Relaxed),
        rbc_store_persist_drops_total: RBC_STORE_PERSIST_DROPS_TOTAL.load(Ordering::Relaxed),
        rbc_store_evictions_total: RBC_STORE_EVICTIONS_TOTAL.load(Ordering::Relaxed),
        rbc_abort: rbc_abort_snapshot(),
        rbc_mismatch: rbc_mismatch_snapshot(),
        rbc_store_recent_evictions: recent_evictions,
        pending_rbc,
        collectors_targeted_current: COLLECTORS_TARGETED_CURRENT.load(Ordering::Relaxed),
        collectors_targeted_last_per_block: COLLECTORS_TARGETED_LAST_COMMIT.load(Ordering::Relaxed),
        redundant_sends_total: REDUNDANT_SEND_TOTAL.load(Ordering::Relaxed),
        tx_queue_depth,
        tx_queue_capacity,
        tx_queue_saturated,
        worker_loop,
        commit_inflight,
        tx_gossip: TxGossipSnapshot::default(),
        qc_rebuild_attempts_total: QC_REBUILD_ATTEMPTS_TOTAL.load(Ordering::Relaxed),
        qc_rebuild_successes_total: QC_REBUILD_SUCCESSES_TOTAL.load(Ordering::Relaxed),
        qc_missing_votes_accepted_total: QC_MISSING_VOTES_ACCEPTED_TOTAL.load(Ordering::Relaxed),
        qc_quorum_without_qc_total: QC_QUORUM_WITHOUT_QC_TOTAL.load(Ordering::Relaxed),
        epoch_length_blocks,
        epoch_commit_deadline_offset,
        epoch_reveal_deadline_offset,
        prf_epoch_seed: prf_seed,
        prf_height,
        prf_view,
        membership_height: MEMBERSHIP_HEIGHT.load(Ordering::Relaxed),
        membership_view: MEMBERSHIP_VIEW.load(Ordering::Relaxed),
        membership_epoch: MEMBERSHIP_EPOCH.load(Ordering::Relaxed),
        membership_view_hash: membership_view_hash(),
        membership_mismatch: membership_mismatch_snapshot(),
        vrf_penalty_epoch: vrf_epoch,
        vrf_non_reveal_total: vrf_non_reveal,
        vrf_no_participation_total: vrf_no_participation,
        vrf_late_reveals_total: vrf_late_reveals,
        consensus_penalties_applied_total,
        consensus_penalties_pending,
        vrf_penalties_applied_total,
        vrf_penalties_pending,
        access_set_sources: access_set_source_snapshot(),
        pipeline_conflict_rate_bps: PIPELINE_CONFLICT_RATE_BPS.load(Ordering::Relaxed),
        lane_activity: lane_activity_snapshot(),
        dataspace_activity: dataspace_activity_snapshot(),
        rbc_lane_backlog: rbc_lane_backlog_snapshot(),
        rbc_dataspace_backlog: rbc_dataspace_backlog_snapshot(),
        lane_commitments: lane_commitments_snapshot(),
        dataspace_commitments: dataspace_commitments_snapshot(),
        lane_settlement_commitments: lane_settlement_commitments_snapshot(),
        lane_relay_envelopes,
        lane_governance_sealed_total,
        lane_governance_sealed_aliases,
        lane_governance: lane_governance_entries,
        nexus_fee: nexus_fee_snapshot(),
        nexus_staking: nexus_staking_snapshot(),
        npos_election: latest_npos_election(),
    }
}

/// Set last observed latency for the `propose` phase (ms).
pub fn set_phase_propose_ms(ms: u64) {
    LAST_PROPOSE_MS.store(ms, Ordering::Relaxed);
}
/// Set last observed latency for the data-availability collection phase (ms).
pub fn set_phase_collect_da_ms(ms: u64) {
    LAST_COLLECT_DA_MS.store(ms, Ordering::Relaxed);
}
/// Set last observed latency for the prevote collection phase (ms).
pub fn set_phase_collect_prevote_ms(ms: u64) {
    LAST_COLLECT_PREVOTE_MS.store(ms, Ordering::Relaxed);
}
/// Set last observed latency for the precommit collection phase (ms).
pub fn set_phase_collect_precommit_ms(ms: u64) {
    LAST_COLLECT_PRECOMMIT_MS.store(ms, Ordering::Relaxed);
}
/// Set last observed latency for redundant collector fan-out (ms).
pub fn set_phase_collect_aggregator_ms(ms: u64) {
    LAST_COLLECT_AGG_MS.store(ms, Ordering::Relaxed);
}
/// Set last observed latency for the commit phase (ms).
pub fn set_phase_commit_ms(ms: u64) {
    LAST_COMMIT_MS.store(ms, Ordering::Relaxed);
}

/// Set EMA latency for the propose phase (ms).
pub fn set_phase_propose_ema_ms(ms: u64) {
    LAST_PROPOSE_EMA_MS.store(ms, Ordering::Relaxed);
}
/// Set EMA latency for the data-availability phase (ms).
pub fn set_phase_collect_da_ema_ms(ms: u64) {
    LAST_COLLECT_DA_EMA_MS.store(ms, Ordering::Relaxed);
}
/// Set EMA latency for the prevote phase (ms).
pub fn set_phase_collect_prevote_ema_ms(ms: u64) {
    LAST_COLLECT_PREVOTE_EMA_MS.store(ms, Ordering::Relaxed);
}
/// Set EMA latency for the precommit phase (ms).
pub fn set_phase_collect_precommit_ema_ms(ms: u64) {
    LAST_COLLECT_PRECOMMIT_EMA_MS.store(ms, Ordering::Relaxed);
}
/// Set EMA latency for redundant collector fan-out (ms).
pub fn set_phase_collect_aggregator_ema_ms(ms: u64) {
    LAST_COLLECT_AGG_EMA_MS.store(ms, Ordering::Relaxed);
}
/// Set EMA latency for the commit phase (ms).
pub fn set_phase_commit_ema_ms(ms: u64) {
    LAST_COMMIT_EMA_MS.store(ms, Ordering::Relaxed);
}
/// Set EMA latency for the aggregate pipeline (ms).
pub fn set_phase_pipeline_total_ema_ms(ms: u64) {
    LAST_PIPELINE_TOTAL_EMA_MS.store(ms, Ordering::Relaxed);
}

/// Increment gossip fallback counter (collectors exhausted).
pub fn inc_gossip_fallback() {
    GOSSIP_FALLBACK_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Dedup cache kinds for eviction accounting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DedupEvictionKind {
    /// Vote cache evictions.
    Vote,
    /// `BlockCreated` payload cache evictions.
    BlockCreated,
    /// Proposal payload cache evictions.
    Proposal,
    /// RBC INIT payload cache evictions.
    RbcInit,
    /// Block sync update payload cache evictions.
    BlockSyncUpdate,
    /// Fetch-pending-block cache evictions.
    FetchPendingBlock,
    /// RBC READY payload cache evictions.
    RbcReady,
    /// RBC DELIVER payload cache evictions.
    RbcDeliver,
    /// RBC chunk payload cache evictions.
    RbcChunk,
}

/// Record dedup evictions for the given cache kind.
pub(crate) fn record_dedup_evictions(kind: DedupEvictionKind, capacity: usize, expired: usize) {
    let capacity = u64::try_from(capacity).unwrap_or(u64::MAX);
    let expired = u64::try_from(expired).unwrap_or(u64::MAX);
    if capacity == 0 && expired == 0 {
        return;
    }
    match kind {
        DedupEvictionKind::Vote => {
            if capacity > 0 {
                DEDUP_VOTE_EVICT_CAPACITY_TOTAL.fetch_add(capacity, Ordering::Relaxed);
            }
            if expired > 0 {
                DEDUP_VOTE_EVICT_EXPIRED_TOTAL.fetch_add(expired, Ordering::Relaxed);
            }
        }
        DedupEvictionKind::BlockCreated => {
            if capacity > 0 {
                DEDUP_BLOCK_CREATED_EVICT_CAPACITY_TOTAL.fetch_add(capacity, Ordering::Relaxed);
            }
            if expired > 0 {
                DEDUP_BLOCK_CREATED_EVICT_EXPIRED_TOTAL.fetch_add(expired, Ordering::Relaxed);
            }
        }
        DedupEvictionKind::Proposal => {
            if capacity > 0 {
                DEDUP_PROPOSAL_EVICT_CAPACITY_TOTAL.fetch_add(capacity, Ordering::Relaxed);
            }
            if expired > 0 {
                DEDUP_PROPOSAL_EVICT_EXPIRED_TOTAL.fetch_add(expired, Ordering::Relaxed);
            }
        }
        DedupEvictionKind::RbcInit => {
            if capacity > 0 {
                DEDUP_RBC_INIT_EVICT_CAPACITY_TOTAL.fetch_add(capacity, Ordering::Relaxed);
            }
            if expired > 0 {
                DEDUP_RBC_INIT_EVICT_EXPIRED_TOTAL.fetch_add(expired, Ordering::Relaxed);
            }
        }
        DedupEvictionKind::BlockSyncUpdate => {
            if capacity > 0 {
                DEDUP_BLOCK_SYNC_UPDATE_EVICT_CAPACITY_TOTAL.fetch_add(capacity, Ordering::Relaxed);
            }
            if expired > 0 {
                DEDUP_BLOCK_SYNC_UPDATE_EVICT_EXPIRED_TOTAL.fetch_add(expired, Ordering::Relaxed);
            }
        }
        DedupEvictionKind::FetchPendingBlock => {
            if capacity > 0 {
                DEDUP_FETCH_PENDING_BLOCK_EVICT_CAPACITY_TOTAL
                    .fetch_add(capacity, Ordering::Relaxed);
            }
            if expired > 0 {
                DEDUP_FETCH_PENDING_BLOCK_EVICT_EXPIRED_TOTAL.fetch_add(expired, Ordering::Relaxed);
            }
        }
        DedupEvictionKind::RbcReady => {
            if capacity > 0 {
                DEDUP_RBC_READY_EVICT_CAPACITY_TOTAL.fetch_add(capacity, Ordering::Relaxed);
            }
            if expired > 0 {
                DEDUP_RBC_READY_EVICT_EXPIRED_TOTAL.fetch_add(expired, Ordering::Relaxed);
            }
        }
        DedupEvictionKind::RbcDeliver => {
            if capacity > 0 {
                DEDUP_RBC_DELIVER_EVICT_CAPACITY_TOTAL.fetch_add(capacity, Ordering::Relaxed);
            }
            if expired > 0 {
                DEDUP_RBC_DELIVER_EVICT_EXPIRED_TOTAL.fetch_add(expired, Ordering::Relaxed);
            }
        }
        DedupEvictionKind::RbcChunk => {
            if capacity > 0 {
                DEDUP_RBC_CHUNK_EVICT_CAPACITY_TOTAL.fetch_add(capacity, Ordering::Relaxed);
            }
            if expired > 0 {
                DEDUP_RBC_CHUNK_EVICT_EXPIRED_TOTAL.fetch_add(expired, Ordering::Relaxed);
            }
        }
    }
}

/// Increment background post drop counter when no worker is available.
pub fn record_bg_post_drop(kind: &'static str) {
    match kind {
        "Post" | "PostControlFlow" => {
            BG_POST_DROP_POST_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        "Broadcast" | "BroadcastControlFlow" => {
            BG_POST_DROP_BROADCAST_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        other => {
            iroha_logger::warn!(
                kind = other,
                "unknown background request kind for drop accounting"
            );
        }
    }
}

/// Record a consensus message drop or deferral with a reason label.
pub fn record_consensus_message_handling(
    kind: ConsensusMessageKind,
    outcome: ConsensusMessageOutcome,
    reason: ConsensusMessageReason,
) {
    #[cfg(test)]
    let _guard = message_handling_test_guard();
    let slot = MESSAGE_HANDLING_TOTALS.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = slot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let key = ConsensusMessageHandlingKey {
        kind,
        outcome,
        reason,
    };
    let entry = guard.entry(key).or_insert(0);
    *entry = entry.saturating_add(1);
}

fn should_log_vote_drop_count(count: u64) -> bool {
    if count == 1 {
        return true;
    }
    if count < 10 || !count.is_multiple_of(10) {
        return false;
    }
    let mut value = count;
    while value.is_multiple_of(10) {
        value /= 10;
    }
    value == 1
}

/// Record a vote-validation drop with roster context.
pub fn record_vote_validation_drop(record: VoteValidationDropRecord) {
    #[cfg(test)]
    let _guard = vote_validation_drops_test_guard();
    let now_ms = now_timestamp_ms();
    let peer_id = record.peer_id.clone();
    let entry = VoteValidationDropEntry {
        reason: record.reason,
        height: record.height,
        view: record.view,
        epoch: record.epoch,
        signer_index: record.signer_index,
        peer_id: record.peer_id,
        roster_hash: record.roster_hash,
        roster_len: record.roster_len,
        block_hash: record.block_hash,
        timestamp_ms: now_ms,
    };
    let slot = VOTE_VALIDATION_DROPS.get_or_init(|| Mutex::new(VecDeque::new()));
    let mut guard = slot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.push_back(entry);
    while guard.len() > VOTE_VALIDATION_DROPS_CAP {
        guard.pop_front();
    }
    VOTE_VALIDATION_DROPS_TOTAL.fetch_add(1, Ordering::Relaxed);
    let Some(peer_id) = peer_id else {
        return;
    };
    let roster_hash = record.roster_hash;
    let Some(mut registry) = vote_validation_drop_peer_registry() else {
        return;
    };
    let key = VoteValidationDropPeerKey {
        peer_id: peer_id.clone(),
        roster_hash,
    };
    let entry = registry.entry(key).or_default();
    entry.total = entry.total.saturating_add(1);
    entry.roster_len = record.roster_len;
    let reason_total = entry.reasons.entry(record.reason).or_insert(0);
    *reason_total = reason_total.saturating_add(1);
    entry.last_height = record.height;
    entry.last_view = record.view;
    entry.last_epoch = record.epoch;
    entry.last_timestamp_ms = now_ms;
    if should_log_vote_drop_count(entry.total) || should_log_vote_drop_count(*reason_total) {
        iroha_logger::info!(
            peer = ?peer_id,
            roster_hash = ?roster_hash,
            roster_len = entry.roster_len,
            reason = record.reason.as_str(),
            total = entry.total,
            reason_total = *reason_total,
            height = record.height,
            view = record.view,
            epoch = record.epoch,
            "vote validation drop telemetry"
        );
    }
}

/// Increment `BlockCreated` drop counter when locked QC gate rejects a proposal.
pub fn inc_block_created_dropped_by_lock() {
    BLOCK_CREATED_DROPPED_BY_LOCK_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment counter for `BlockCreated` hint mismatches (height/view/parent).
pub fn inc_block_created_hint_mismatch() {
    BLOCK_CREATED_HINT_MISMATCH_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment counter for `BlockCreated` proposal mismatches (header/payload).
pub fn inc_block_created_proposal_mismatch() {
    BLOCK_CREATED_PROPOSAL_MISMATCH_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment counter when block sync drops a batch due to invalid signatures.
pub fn inc_block_sync_drop_invalid_signatures() {
    #[cfg(test)]
    let _guard = block_sync_test_guard();
    BLOCK_SYNC_DROP_INVALID_SIGNATURES_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment counter when block sync replaces an incoming QC with a locally derived one.
pub fn inc_block_sync_qc_replaced() {
    #[cfg(test)]
    let _guard = block_sync_test_guard();
    BLOCK_SYNC_QC_REPLACED_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment counter when block sync cannot derive a QC for an incoming batch.
pub fn inc_block_sync_qc_derive_failed() {
    #[cfg(test)]
    let _guard = block_sync_test_guard();
    BLOCK_SYNC_QC_DERIVE_FAILED_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment counter when a block is rejected by the validation gate before voting.
pub fn record_validation_reject(
    reason: &'static str,
    height: u64,
    view: u64,
    block_hash: HashOf<BlockHeader>,
) {
    #[cfg(test)]
    let _guard = validation_reject_test_guard();
    VALIDATION_REJECT_TOTAL.fetch_add(1, Ordering::Relaxed);
    match reason {
        "stateless" => {
            VALIDATION_REJECT_STATELESS_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        "execution" => {
            VALIDATION_REJECT_EXECUTION_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        "prev_hash" => {
            VALIDATION_REJECT_PREV_HASH_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        "prev_height" => {
            VALIDATION_REJECT_PREV_HEIGHT_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        "topology" => {
            VALIDATION_REJECT_TOPOLOGY_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }

    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|dur| u64::try_from(dur.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    VALIDATION_REJECT_LAST_TS_MS.store(now_ms, Ordering::Relaxed);

    if let Ok(mut guard) = VALIDATION_REJECT_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *guard = Some(reason);
    }
    if let Ok(mut guard) = VALIDATION_REJECT_LAST_HEIGHT
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *guard = Some(height);
    }
    if let Ok(mut guard) = VALIDATION_REJECT_LAST_VIEW
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *guard = Some(view);
    }
    if let Ok(mut guard) = VALIDATION_REJECT_LAST_BLOCK
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *guard = Some(Hash::from(block_hash));
    }
}

/// Record an RBC abort occurrence for telemetry.
pub fn record_rbc_abort(height: u64, view: u64) {
    RBC_ABORT_TOTAL.fetch_add(1, Ordering::Relaxed);
    RBC_ABORT_LAST_HEIGHT.store(height, Ordering::Relaxed);
    RBC_ABORT_LAST_VIEW.store(view, Ordering::Relaxed);
}

/// Snapshot of RBC abort counters.
#[must_use]
pub fn rbc_abort_snapshot() -> RbcAbortSnapshot {
    RbcAbortSnapshot {
        total: RBC_ABORT_TOTAL.load(Ordering::Relaxed),
        last_height: RBC_ABORT_LAST_HEIGHT.load(Ordering::Relaxed),
        last_view: RBC_ABORT_LAST_VIEW.load(Ordering::Relaxed),
    }
}

/// Record a peer consensus-key policy reject reason with counters and timestamp.
pub fn record_peer_key_policy_reject(reason: PeerKeyPolicyRejectReason) {
    PEER_KEY_POLICY_REJECT_TOTAL.fetch_add(1, Ordering::Relaxed);
    match reason {
        PeerKeyPolicyRejectReason::MissingHsm => {
            PEER_KEY_POLICY_MISSING_HSM_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        PeerKeyPolicyRejectReason::DisallowedAlgorithm => {
            PEER_KEY_POLICY_ALGO_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        PeerKeyPolicyRejectReason::DisallowedProvider => {
            PEER_KEY_POLICY_PROVIDER_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        PeerKeyPolicyRejectReason::LeadTimeViolation => {
            PEER_KEY_POLICY_LEAD_TIME_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        PeerKeyPolicyRejectReason::ActivationInPast => {
            PEER_KEY_POLICY_ACTIVATION_PAST_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        PeerKeyPolicyRejectReason::ExpiryBeforeActivation => {
            PEER_KEY_POLICY_EXPIRY_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        PeerKeyPolicyRejectReason::IdentifierCollision => {
            PEER_KEY_POLICY_ID_COLLISION_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
    }

    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|dur| u64::try_from(dur.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    PEER_KEY_POLICY_LAST_TS_MS.store(now_ms, Ordering::Relaxed);

    if let Ok(mut guard) = PEER_KEY_POLICY_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *guard = Some(reason.as_str());
    }
}

/// Record which roster source was used during block sync.
pub fn inc_block_sync_roster_source(source: &str) {
    #[cfg(test)]
    let _guard = block_sync_test_guard();
    match source {
        "commit_checkpoint_pair_hint" => {
            BLOCK_SYNC_ROSTER_COMMIT_CERT_HINT_TOTAL.fetch_add(1, Ordering::Relaxed);
            BLOCK_SYNC_ROSTER_CHECKPOINT_HINT_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        "commit_qc_hint" => {
            BLOCK_SYNC_ROSTER_COMMIT_CERT_HINT_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        "validator_checkpoint_hint" => {
            BLOCK_SYNC_ROSTER_CHECKPOINT_HINT_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        "commit_qc_history" => {
            BLOCK_SYNC_ROSTER_COMMIT_CERT_HISTORY_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        "validator_checkpoint_history" => {
            BLOCK_SYNC_ROSTER_CHECKPOINT_HISTORY_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        "roster_sidecar" => {
            BLOCK_SYNC_ROSTER_ROSTER_SIDECAR_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        "commit_roster_journal" => {
            BLOCK_SYNC_ROSTER_COMMIT_ROSTER_JOURNAL_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }
}

/// Increment counter when block sync drops an update with no verifiable roster.
pub fn inc_block_sync_roster_drop_missing() {
    #[cfg(test)]
    let _guard = block_sync_test_guard();
    BLOCK_SYNC_ROSTER_DROP_MISSING_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment counter when block sync drops `ShareBlocks` without a matching request.
pub fn inc_block_sync_roster_drop_unsolicited_share_blocks() {
    #[cfg(test)]
    let _guard = block_sync_test_guard();
    BLOCK_SYNC_ROSTER_DROP_UNSOLICITED_SHARE_BLOCKS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Record the cause of a view change and timestamp it.
pub fn record_view_change_cause(cause: &str) {
    #[cfg(test)]
    let _guard = view_change_cause_test_guard();
    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    match cause {
        "commit_failure" => {
            VIEW_CHANGE_CAUSE_COMMIT_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);
            VIEW_CHANGE_CAUSE_LAST_COMMIT_FAILURE_TS_MS.store(now_ms, Ordering::Relaxed);
        }
        "quorum_timeout" => {
            VIEW_CHANGE_CAUSE_QUORUM_TIMEOUT_TOTAL.fetch_add(1, Ordering::Relaxed);
            VIEW_CHANGE_CAUSE_LAST_QUORUM_TIMEOUT_TS_MS.store(now_ms, Ordering::Relaxed);
        }
        "stake_quorum_timeout" => {
            VIEW_CHANGE_CAUSE_STAKE_QUORUM_TIMEOUT_TOTAL.fetch_add(1, Ordering::Relaxed);
            VIEW_CHANGE_CAUSE_LAST_STAKE_QUORUM_TIMEOUT_TS_MS.store(now_ms, Ordering::Relaxed);
        }
        "da_gate" => {
            VIEW_CHANGE_CAUSE_DA_GATE_TOTAL.fetch_add(1, Ordering::Relaxed);
            VIEW_CHANGE_CAUSE_LAST_DA_GATE_TS_MS.store(now_ms, Ordering::Relaxed);
        }
        "censorship_evidence" => {
            VIEW_CHANGE_CAUSE_CENSORSHIP_EVIDENCE_TOTAL.fetch_add(1, Ordering::Relaxed);
            VIEW_CHANGE_CAUSE_LAST_CENSORSHIP_EVIDENCE_TS_MS.store(now_ms, Ordering::Relaxed);
        }
        "missing_payload" => {
            VIEW_CHANGE_CAUSE_MISSING_PAYLOAD_TOTAL.fetch_add(1, Ordering::Relaxed);
            VIEW_CHANGE_CAUSE_LAST_MISSING_PAYLOAD_TS_MS.store(now_ms, Ordering::Relaxed);
        }
        "missing_qc" => {
            VIEW_CHANGE_CAUSE_MISSING_QC_TOTAL.fetch_add(1, Ordering::Relaxed);
            VIEW_CHANGE_CAUSE_LAST_MISSING_QC_TS_MS.store(now_ms, Ordering::Relaxed);
        }
        "validation_reject" => {
            VIEW_CHANGE_CAUSE_VALIDATION_REJECT_TOTAL.fetch_add(1, Ordering::Relaxed);
            VIEW_CHANGE_CAUSE_LAST_VALIDATION_REJECT_TS_MS.store(now_ms, Ordering::Relaxed);
        }
        _ => {}
    }
    VIEW_CHANGE_CAUSE_LAST_TS_MS.store(now_ms, Ordering::Relaxed);
    let slot = VIEW_CHANGE_CAUSE_LAST_LABEL.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(cause.to_string());
    }
}

/// Record a kura persistence failure for a block.
pub fn record_kura_store_failure(height: u64, view: u64, block_hash: HashOf<BlockHeader>) {
    #[cfg(test)]
    let _guard = kura_store_test_guard();
    KURA_STORE_FAILURE_TOTAL.fetch_add(1, Ordering::Relaxed);
    KURA_STORE_LAST_HEIGHT.store(height, Ordering::Relaxed);
    KURA_STORE_LAST_VIEW.store(view, Ordering::Relaxed);
    let mut guard = KURA_STORE_LAST_HASH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura store failure hash mutex poisoned");
    *guard = Some(UntypedHash::from(block_hash));
}

/// Record the latest kura persistence retry attempt and backoff.
pub fn record_kura_store_retry(attempt: u32, backoff_ms: u32) {
    #[cfg(test)]
    let _guard = kura_store_test_guard();
    KURA_STORE_LAST_RETRY_ATTEMPT.store(u64::from(attempt), Ordering::Relaxed);
    KURA_STORE_LAST_RETRY_BACKOFF_MS.store(u64::from(backoff_ms), Ordering::Relaxed);
}

/// Record that a block reached the staging phase before kura persistence.
pub fn record_kura_stage(height: u64, view: u64, block_hash: HashOf<BlockHeader>) {
    #[cfg(test)]
    let _guard = kura_store_test_guard();
    KURA_STAGE_TOTAL.fetch_add(1, Ordering::Relaxed);
    KURA_STAGE_LAST_HEIGHT.store(height, Ordering::Relaxed);
    KURA_STAGE_LAST_VIEW.store(view, Ordering::Relaxed);
    let mut guard = KURA_STAGE_LAST_HASH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura stage hash mutex poisoned");
    *guard = Some(UntypedHash::from(block_hash));
}

/// Record that a staged commit was rolled back before WSV application.
pub fn record_kura_stage_rollback(
    height: u64,
    view: u64,
    block_hash: HashOf<BlockHeader>,
    reason: &'static str,
) {
    #[cfg(test)]
    let _guard = kura_store_test_guard();
    KURA_STAGE_ROLLBACK_TOTAL.fetch_add(1, Ordering::Relaxed);
    KURA_STAGE_ROLLBACK_LAST_HEIGHT.store(height, Ordering::Relaxed);
    KURA_STAGE_ROLLBACK_LAST_VIEW.store(view, Ordering::Relaxed);
    let mut hash_guard = KURA_STAGE_ROLLBACK_LAST_HASH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura stage rollback hash mutex poisoned");
    *hash_guard = Some(UntypedHash::from(block_hash));
    let mut reason_guard = KURA_STAGE_ROLLBACK_LAST_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura stage rollback reason mutex poisoned");
    *reason_guard = Some(reason);
}

/// Record that Highest/Locked QC were reset after a kura abort.
pub fn record_kura_lock_reset(
    height: u64,
    view: u64,
    block_hash: Option<HashOf<BlockHeader>>,
    reason: &'static str,
) {
    #[cfg(test)]
    let _guard = kura_store_test_guard();
    KURA_LOCK_RESET_TOTAL.fetch_add(1, Ordering::Relaxed);
    KURA_LOCK_RESET_LAST_HEIGHT.store(height, Ordering::Relaxed);
    KURA_LOCK_RESET_LAST_VIEW.store(view, Ordering::Relaxed);
    let mut hash_guard = KURA_LOCK_RESET_LAST_HASH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura lock reset hash mutex poisoned");
    *hash_guard = block_hash.map(UntypedHash::from);
    let mut reason_guard = KURA_LOCK_RESET_LAST_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("kura lock reset reason mutex poisoned");
    *reason_guard = Some(reason);
}

/// Increment counter when kura persistence retries are exhausted for a block.
pub fn inc_kura_store_abort() {
    #[cfg(test)]
    let _guard = kura_store_test_guard();
    KURA_STORE_ABORT_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Record a missing-block fetch attempt triggered by a QC-first arrival.
pub fn record_missing_block_fetch(targets: usize, dwell_ms: u64) {
    #[cfg(test)]
    let _guard = missing_block_fetch_test_guard();
    MISSING_BLOCK_FETCH_TOTAL.fetch_add(1, Ordering::Relaxed);
    MISSING_BLOCK_FETCH_LAST_TARGETS.store(targets as u64, Ordering::Relaxed);
    MISSING_BLOCK_FETCH_LAST_DWELL_MS.store(dwell_ms, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) fn reset_missing_block_fetch_counters_for_tests() {
    MISSING_BLOCK_FETCH_TOTAL.store(0, Ordering::Relaxed);
    MISSING_BLOCK_FETCH_LAST_TARGETS.store(0, Ordering::Relaxed);
    MISSING_BLOCK_FETCH_LAST_DWELL_MS.store(0, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) fn reset_message_handling_for_tests() {
    let _guard = message_handling_test_guard();
    if let Some(slot) = MESSAGE_HANDLING_TOTALS.get() {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

#[cfg(test)]
pub(crate) fn reset_block_sync_counters_for_tests() {
    let _guard = block_sync_test_guard();
    BLOCK_SYNC_DROP_INVALID_SIGNATURES_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_SYNC_QC_REPLACED_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_SYNC_QC_DERIVE_FAILED_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_SYNC_ROSTER_COMMIT_CERT_HINT_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_SYNC_ROSTER_CHECKPOINT_HINT_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_SYNC_ROSTER_COMMIT_CERT_HISTORY_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_SYNC_ROSTER_CHECKPOINT_HISTORY_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_SYNC_ROSTER_ROSTER_SIDECAR_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_SYNC_ROSTER_COMMIT_ROSTER_JOURNAL_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_SYNC_ROSTER_DROP_MISSING_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_SYNC_ROSTER_DROP_UNSOLICITED_SHARE_BLOCKS_TOTAL.store(0, Ordering::Relaxed);
}

/// Reset the cached precommit signer history for unit tests.
#[cfg(test)]
pub(crate) fn reset_precommit_signer_history_for_tests() {
    let _guard = commit_history_test_guard();
    let mut guard = precommit_signer_history_slot()
        .lock()
        .expect("precommit signer history mutex poisoned");
    guard.clear();
}

#[cfg(test)]
pub(crate) fn reset_view_change_cause_counters_for_tests() {
    let _guard = view_change_cause_test_guard();
    VIEW_CHANGE_CAUSE_COMMIT_FAILURE_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_QUORUM_TIMEOUT_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_STAKE_QUORUM_TIMEOUT_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_DA_GATE_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_CENSORSHIP_EVIDENCE_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_MISSING_PAYLOAD_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_MISSING_QC_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_VALIDATION_REJECT_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_LAST_TS_MS.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_LAST_COMMIT_FAILURE_TS_MS.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_LAST_QUORUM_TIMEOUT_TS_MS.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_LAST_STAKE_QUORUM_TIMEOUT_TS_MS.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_LAST_DA_GATE_TS_MS.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_LAST_CENSORSHIP_EVIDENCE_TS_MS.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_LAST_MISSING_PAYLOAD_TS_MS.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_LAST_MISSING_QC_TS_MS.store(0, Ordering::Relaxed);
    VIEW_CHANGE_CAUSE_LAST_VALIDATION_REJECT_TS_MS.store(0, Ordering::Relaxed);
    if let Some(slot) = VIEW_CHANGE_CAUSE_LAST_LABEL.get() {
        if let Ok(mut guard) = slot.lock() {
            guard.take();
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn reset_validation_reject_counters_for_tests() {
    let _guard = validation_reject_test_guard();
    VALIDATION_REJECT_TOTAL.store(0, Ordering::Relaxed);
    VALIDATION_REJECT_STATELESS_TOTAL.store(0, Ordering::Relaxed);
    VALIDATION_REJECT_EXECUTION_TOTAL.store(0, Ordering::Relaxed);
    VALIDATION_REJECT_PREV_HASH_TOTAL.store(0, Ordering::Relaxed);
    VALIDATION_REJECT_PREV_HEIGHT_TOTAL.store(0, Ordering::Relaxed);
    VALIDATION_REJECT_TOPOLOGY_TOTAL.store(0, Ordering::Relaxed);
    VALIDATION_REJECT_LAST_TS_MS.store(0, Ordering::Relaxed);
    if let Some(slot) = VALIDATION_REJECT_REASON.get() {
        if let Ok(mut guard) = slot.lock() {
            guard.take();
        }
    }
    if let Some(slot) = VALIDATION_REJECT_LAST_HEIGHT.get() {
        if let Ok(mut guard) = slot.lock() {
            guard.take();
        }
    }
    if let Some(slot) = VALIDATION_REJECT_LAST_VIEW.get() {
        if let Ok(mut guard) = slot.lock() {
            guard.take();
        }
    }
    if let Some(slot) = VALIDATION_REJECT_LAST_BLOCK.get() {
        if let Ok(mut guard) = slot.lock() {
            guard.take();
        }
    }
}

#[cfg(test)]
pub(crate) fn reset_peer_key_policy_counters_for_tests() {
    PEER_KEY_POLICY_REJECT_TOTAL.store(0, Ordering::Relaxed);
    PEER_KEY_POLICY_MISSING_HSM_TOTAL.store(0, Ordering::Relaxed);
    PEER_KEY_POLICY_ALGO_TOTAL.store(0, Ordering::Relaxed);
    PEER_KEY_POLICY_PROVIDER_TOTAL.store(0, Ordering::Relaxed);
    PEER_KEY_POLICY_LEAD_TIME_TOTAL.store(0, Ordering::Relaxed);
    PEER_KEY_POLICY_ACTIVATION_PAST_TOTAL.store(0, Ordering::Relaxed);
    PEER_KEY_POLICY_EXPIRY_TOTAL.store(0, Ordering::Relaxed);
    PEER_KEY_POLICY_ID_COLLISION_TOTAL.store(0, Ordering::Relaxed);
    PEER_KEY_POLICY_LAST_TS_MS.store(0, Ordering::Relaxed);
    if let Some(slot) = PEER_KEY_POLICY_REASON.get() {
        if let Ok(mut guard) = slot.lock() {
            guard.take();
        }
    }
}

#[cfg(test)]
pub(crate) fn reset_rbc_abort_counters_for_tests() {
    RBC_ABORT_TOTAL.store(0, Ordering::Relaxed);
    RBC_ABORT_LAST_HEIGHT.store(0, Ordering::Relaxed);
    RBC_ABORT_LAST_VIEW.store(0, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) fn reset_kura_store_counters_for_tests() {
    let _guard = kura_store_test_guard();
    KURA_STORE_FAILURE_TOTAL.store(0, Ordering::Relaxed);
    KURA_STORE_ABORT_TOTAL.store(0, Ordering::Relaxed);
    KURA_STAGE_TOTAL.store(0, Ordering::Relaxed);
    KURA_STAGE_ROLLBACK_TOTAL.store(0, Ordering::Relaxed);
    KURA_STAGE_LAST_HEIGHT.store(0, Ordering::Relaxed);
    KURA_STAGE_LAST_VIEW.store(0, Ordering::Relaxed);
    KURA_STAGE_ROLLBACK_LAST_HEIGHT.store(0, Ordering::Relaxed);
    KURA_STAGE_ROLLBACK_LAST_VIEW.store(0, Ordering::Relaxed);
    KURA_LOCK_RESET_TOTAL.store(0, Ordering::Relaxed);
    KURA_LOCK_RESET_LAST_HEIGHT.store(0, Ordering::Relaxed);
    KURA_LOCK_RESET_LAST_VIEW.store(0, Ordering::Relaxed);
    KURA_STORE_LAST_HEIGHT.store(0, Ordering::Relaxed);
    KURA_STORE_LAST_VIEW.store(0, Ordering::Relaxed);
    KURA_STORE_LAST_RETRY_ATTEMPT.store(0, Ordering::Relaxed);
    KURA_STORE_LAST_RETRY_BACKOFF_MS.store(0, Ordering::Relaxed);
    if let Some(slot) = KURA_STORE_LAST_HASH.get() {
        let mut guard = slot.lock().expect("kura store hash mutex poisoned");
        *guard = None;
    }
    if let Some(slot) = KURA_STAGE_LAST_HASH.get() {
        let mut guard = slot.lock().expect("kura stage hash mutex poisoned");
        *guard = None;
    }
    if let Some(slot) = KURA_STAGE_ROLLBACK_LAST_HASH.get() {
        let mut guard = slot
            .lock()
            .expect("kura stage rollback hash mutex poisoned");
        *guard = None;
    }
    if let Some(slot) = KURA_STAGE_ROLLBACK_LAST_REASON.get() {
        let mut guard = slot
            .lock()
            .expect("kura stage rollback reason mutex poisoned");
        guard.take();
    }
    if let Some(slot) = KURA_LOCK_RESET_LAST_HASH.get() {
        let mut guard = slot.lock().expect("kura lock reset hash mutex poisoned");
        *guard = None;
    }
    if let Some(slot) = KURA_LOCK_RESET_LAST_REASON.get() {
        let mut guard = slot.lock().expect("kura lock reset reason mutex poisoned");
        guard.take();
    }
}

/// Record the current DA availability reason and the last satisfied condition, if any.
pub fn record_da_gate_transition(previous: Option<GateReason>, current: Option<GateReason>) {
    #[cfg(test)]
    let _guard = da_gate_test_guard();
    if let Some(reason) = current {
        match reason {
            GateReason::MissingLocalData => {
                DA_GATE_MISSING_LOCAL_DATA_TOTAL.fetch_add(1, Ordering::Relaxed);
            }
            GateReason::ManifestGuard { .. } => {
                DA_GATE_MANIFEST_GUARD_TOTAL.fetch_add(1, Ordering::Relaxed);
            }
        }
        DA_GATE_LAST_REASON.store(
            DaGateReasonSnapshot::from(reason).as_code(),
            Ordering::Relaxed,
        );
    } else {
        DA_GATE_LAST_REASON.store(DaGateReasonSnapshot::None.as_code(), Ordering::Relaxed);
    }

    if let Some(satisfied) = super::da::gate_satisfaction(previous, current) {
        DA_GATE_LAST_SATISFIED.store(
            DaGateSatisfactionSnapshot::from(satisfied).as_code(),
            Ordering::Relaxed,
        );
    }
}

#[cfg(test)]
pub(crate) fn reset_da_gate_counters_for_tests() {
    let _guard = da_gate_test_guard();
    DA_GATE_MISSING_LOCAL_DATA_TOTAL.store(0, Ordering::Relaxed);
    DA_GATE_MANIFEST_GUARD_TOTAL.store(0, Ordering::Relaxed);
    DA_GATE_LAST_REASON.store(DaGateReasonSnapshot::None.as_code(), Ordering::Relaxed);
    DA_GATE_LAST_SATISFIED.store(
        DaGateSatisfactionSnapshot::None.as_code(),
        Ordering::Relaxed,
    );
}

/// Increment counter when the pacemaker skips proposal assembly due to proposal backpressure
/// (queue saturation, relay/RBC backpressure, or blocking pending blocks).
pub fn inc_pacemaker_backpressure_deferrals() {
    PACEMAKER_BACKPRESSURE_DEFERRALS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) fn reset_pacemaker_backpressure_deferrals_for_test() {
    PACEMAKER_BACKPRESSURE_DEFERRALS_TOTAL.store(0, Ordering::Relaxed);
}
/// Snapshot latest per-phase latencies (ms).
pub fn phase_latencies_snapshot() -> PhaseLatenciesSnapshot {
    let propose_ms = LAST_PROPOSE_MS.load(Ordering::Relaxed);
    let collect_da_ms = LAST_COLLECT_DA_MS.load(Ordering::Relaxed);
    let collect_prevote_ms = LAST_COLLECT_PREVOTE_MS.load(Ordering::Relaxed);
    let collect_precommit_ms = LAST_COLLECT_PRECOMMIT_MS.load(Ordering::Relaxed);
    let collect_aggregator_ms = LAST_COLLECT_AGG_MS.load(Ordering::Relaxed);
    let commit_ms = LAST_COMMIT_MS.load(Ordering::Relaxed);
    let propose_ema_ms = LAST_PROPOSE_EMA_MS.load(Ordering::Relaxed);
    let collect_da_ema_ms = LAST_COLLECT_DA_EMA_MS.load(Ordering::Relaxed);
    let collect_prevote_ema_ms = LAST_COLLECT_PREVOTE_EMA_MS.load(Ordering::Relaxed);
    let collect_precommit_ema_ms = LAST_COLLECT_PRECOMMIT_EMA_MS.load(Ordering::Relaxed);
    let collect_aggregator_ema_ms = LAST_COLLECT_AGG_EMA_MS.load(Ordering::Relaxed);
    let commit_ema_ms = LAST_COMMIT_EMA_MS.load(Ordering::Relaxed);

    let pipeline_total_acc = (u128::from(propose_ms)
        + u128::from(collect_da_ms)
        + u128::from(collect_prevote_ms)
        + u128::from(collect_precommit_ms)
        + u128::from(commit_ms))
    .min(u128::from(u64::MAX));
    let pipeline_total_ms = u64::try_from(pipeline_total_acc).unwrap_or(u64::MAX);
    PhaseLatenciesSnapshot {
        propose_ms,
        collect_da_ms,
        collect_prevote_ms,
        collect_precommit_ms,
        collect_aggregator_ms,
        commit_ms,
        propose_ema_ms,
        collect_da_ema_ms,
        collect_prevote_ema_ms,
        collect_precommit_ema_ms,
        collect_aggregator_ema_ms,
        commit_ema_ms,
        pipeline_total_ms,
        pipeline_total_ema_ms: LAST_PIPELINE_TOTAL_EMA_MS.load(Ordering::Relaxed),
        gossip_fallback_total: GOSSIP_FALLBACK_TOTAL.load(Ordering::Relaxed),
        block_created_dropped_by_lock_total: BLOCK_CREATED_DROPPED_BY_LOCK_TOTAL
            .load(Ordering::Relaxed),
        block_created_hint_mismatch_total: BLOCK_CREATED_HINT_MISMATCH_TOTAL
            .load(Ordering::Relaxed),
        block_created_proposal_mismatch_total: BLOCK_CREATED_PROPOSAL_MISMATCH_TOTAL
            .load(Ordering::Relaxed),
    }
}

/// Record an availability vote ingestion for the local collector.
pub fn record_availability_vote(collector_idx: u64, peer: &PeerId) {
    let mut stats = availability_slot().lock().unwrap();
    stats.total_votes = stats.total_votes.saturating_add(1);
    let entry = stats
        .per_peer
        .entry(peer.clone())
        .or_insert_with(|| CollectorEntry {
            idx: collector_idx,
            votes: 0,
        });
    entry.idx = collector_idx;
    entry.votes = entry.votes.saturating_add(1);
}

/// Snapshot availability vote ingestion counters for `/v1/sumeragi/telemetry`.
pub fn availability_snapshot() -> AvailabilitySnapshot {
    let stats = availability_slot().lock().unwrap();
    let mut collectors: Vec<AvailabilityCollectorSnapshot> = stats
        .per_peer
        .iter()
        .map(|(peer, entry)| AvailabilityCollectorSnapshot {
            collector_idx: entry.idx,
            peer: peer.clone(),
            votes_ingested: entry.votes,
        })
        .collect();
    collectors.sort_by_key(|entry| entry.collector_idx);
    AvailabilitySnapshot {
        total: stats.total_votes,
        collectors,
    }
}

/// Record the last observed QC assembly latency for the given kind (e.g., `availability`).
pub fn record_qc_latency(kind: &'static str, ms: u64) {
    let mut slot = qc_latency_slot().lock().unwrap();
    slot.insert(kind, ms);
}

/// Snapshot QC assembly latencies for `/v1/sumeragi/telemetry`.
pub fn qc_latency_snapshot() -> Vec<(String, u64)> {
    let slot = qc_latency_slot().lock().unwrap();
    let mut out: Vec<(String, u64)> = slot.iter().map(|(k, v)| ((*k).to_string(), *v)).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Update the aggregated RBC backlog snapshot for telemetry consumers.
pub fn set_rbc_backlog_snapshot(total_missing: u64, max_missing: u64, pending_sessions: u64) {
    let mut slot = rbc_backlog_slot().lock().unwrap();
    *slot = RbcBacklogSnapshot {
        total_missing_chunks: total_missing,
        max_missing_chunks: max_missing,
        pending_sessions,
    };
}

/// Snapshot RBC backlog aggregation for `/v1/sumeragi/telemetry`.
pub fn rbc_backlog_snapshot() -> RbcBacklogSnapshot {
    *rbc_backlog_slot().lock().unwrap()
}

/// Replace the pending-RBC snapshot used by `/v1/sumeragi/status`.
pub fn set_pending_rbc_snapshot(snapshot: PendingRbcSnapshot) {
    *pending_rbc_slot().lock().unwrap() = snapshot;
}

/// Snapshot pending-RBC aggregation for `/v1/sumeragi/status`.
pub fn pending_rbc_snapshot() -> PendingRbcSnapshot {
    let mut snapshot = pending_rbc_slot().lock().unwrap().clone();
    snapshot.drops_total = PENDING_RBC_DROPS_TOTAL.load(Ordering::Relaxed);
    snapshot.drops_cap_total = PENDING_RBC_DROPS_CAP_TOTAL.load(Ordering::Relaxed);
    snapshot.drops_cap_bytes_total = PENDING_RBC_DROPS_CAP_BYTES_TOTAL.load(Ordering::Relaxed);
    snapshot.drops_ttl_total = PENDING_RBC_DROPS_TTL_TOTAL.load(Ordering::Relaxed);
    snapshot.drops_ttl_bytes_total = PENDING_RBC_DROPS_TTL_BYTES_TOTAL.load(Ordering::Relaxed);
    snapshot.drops_bytes_total = PENDING_RBC_DROPPED_BYTES_TOTAL.load(Ordering::Relaxed);
    snapshot.evicted_total = PENDING_RBC_EVICTED_TOTAL.load(Ordering::Relaxed);
    snapshot.stash_ready_total = PENDING_RBC_STASH_READY_TOTAL.load(Ordering::Relaxed);
    snapshot.stash_ready_init_missing_total =
        PENDING_RBC_STASH_READY_INIT_MISSING_TOTAL.load(Ordering::Relaxed);
    snapshot.stash_ready_roster_missing_total =
        PENDING_RBC_STASH_READY_ROSTER_MISSING_TOTAL.load(Ordering::Relaxed);
    snapshot.stash_ready_roster_hash_mismatch_total =
        PENDING_RBC_STASH_READY_ROSTER_HASH_MISMATCH_TOTAL.load(Ordering::Relaxed);
    snapshot.stash_ready_roster_unverified_total =
        PENDING_RBC_STASH_READY_ROSTER_UNVERIFIED_TOTAL.load(Ordering::Relaxed);
    snapshot.stash_deliver_total = PENDING_RBC_STASH_DELIVER_TOTAL.load(Ordering::Relaxed);
    snapshot.stash_deliver_init_missing_total =
        PENDING_RBC_STASH_DELIVER_INIT_MISSING_TOTAL.load(Ordering::Relaxed);
    snapshot.stash_deliver_roster_missing_total =
        PENDING_RBC_STASH_DELIVER_ROSTER_MISSING_TOTAL.load(Ordering::Relaxed);
    snapshot.stash_deliver_roster_hash_mismatch_total =
        PENDING_RBC_STASH_DELIVER_ROSTER_HASH_MISMATCH_TOTAL.load(Ordering::Relaxed);
    snapshot.stash_deliver_roster_unverified_total =
        PENDING_RBC_STASH_DELIVER_ROSTER_UNVERIFIED_TOTAL.load(Ordering::Relaxed);
    snapshot.stash_chunk_total = PENDING_RBC_STASH_CHUNK_TOTAL.load(Ordering::Relaxed);
    snapshot
}

/// Record pending-RBC stash counts by message kind and reason.
pub fn inc_pending_rbc_stash(kind: PendingRbcStashKind, reason: Option<PendingRbcStashReason>) {
    match kind {
        PendingRbcStashKind::Chunk => {
            PENDING_RBC_STASH_CHUNK_TOTAL.fetch_add(1, Ordering::Relaxed);
        }
        PendingRbcStashKind::Ready => {
            PENDING_RBC_STASH_READY_TOTAL.fetch_add(1, Ordering::Relaxed);
            if let Some(reason) = reason {
                match reason {
                    PendingRbcStashReason::InitMissing => {
                        PENDING_RBC_STASH_READY_INIT_MISSING_TOTAL.fetch_add(1, Ordering::Relaxed);
                    }
                    PendingRbcStashReason::RosterMissing => {
                        PENDING_RBC_STASH_READY_ROSTER_MISSING_TOTAL
                            .fetch_add(1, Ordering::Relaxed);
                    }
                    PendingRbcStashReason::RosterHashMismatch => {
                        PENDING_RBC_STASH_READY_ROSTER_HASH_MISMATCH_TOTAL
                            .fetch_add(1, Ordering::Relaxed);
                    }
                    PendingRbcStashReason::RosterUnverified => {
                        PENDING_RBC_STASH_READY_ROSTER_UNVERIFIED_TOTAL
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
        PendingRbcStashKind::Deliver => {
            PENDING_RBC_STASH_DELIVER_TOTAL.fetch_add(1, Ordering::Relaxed);
            if let Some(reason) = reason {
                match reason {
                    PendingRbcStashReason::InitMissing => {
                        PENDING_RBC_STASH_DELIVER_INIT_MISSING_TOTAL
                            .fetch_add(1, Ordering::Relaxed);
                    }
                    PendingRbcStashReason::RosterMissing => {
                        PENDING_RBC_STASH_DELIVER_ROSTER_MISSING_TOTAL
                            .fetch_add(1, Ordering::Relaxed);
                    }
                    PendingRbcStashReason::RosterHashMismatch => {
                        PENDING_RBC_STASH_DELIVER_ROSTER_HASH_MISMATCH_TOTAL
                            .fetch_add(1, Ordering::Relaxed);
                    }
                    PendingRbcStashReason::RosterUnverified => {
                        PENDING_RBC_STASH_DELIVER_ROSTER_UNVERIFIED_TOTAL
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }
}

/// Record dropped pending RBC frames (cap or TTL enforcement).
pub fn inc_rbc_pending_drop(reason: &str, frames: u64, bytes: u64) {
    let frames = frames.max(1);
    match reason {
        "cap" | "session_cap" => {
            PENDING_RBC_DROPS_CAP_TOTAL.fetch_add(frames, Ordering::Relaxed);
            PENDING_RBC_DROPS_CAP_BYTES_TOTAL.fetch_add(bytes, Ordering::Relaxed);
        }
        "ttl" => {
            PENDING_RBC_DROPS_TTL_TOTAL.fetch_add(frames, Ordering::Relaxed);
            PENDING_RBC_DROPS_TTL_BYTES_TOTAL.fetch_add(bytes, Ordering::Relaxed);
        }
        _ => {}
    }
    PENDING_RBC_DROPS_TOTAL.fetch_add(frames, Ordering::Relaxed);
    PENDING_RBC_DROPPED_BYTES_TOTAL.fetch_add(bytes, Ordering::Relaxed);
}

/// Record pending RBC session evictions (TTL expiry or stash-cap eviction).
pub fn inc_pending_rbc_evicted(count: u64) {
    PENDING_RBC_EVICTED_TOTAL.fetch_add(count, Ordering::Relaxed);
}

fn lane_activity_slot() -> &'static Mutex<Vec<LaneActivitySnapshot>> {
    LANE_ACTIVITY.get_or_init(|| Mutex::new(Vec::new()))
}

fn access_set_source_slot() -> &'static Mutex<AccessSetSourceSummary> {
    ACCESS_SET_SOURCES.get_or_init(|| Mutex::new(AccessSetSourceSummary::default()))
}

fn dataspace_activity_slot() -> &'static Mutex<Vec<DataspaceActivitySnapshot>> {
    DATASPACE_ACTIVITY.get_or_init(|| Mutex::new(Vec::new()))
}

/// Replace the lane-activity snapshot used by `/v1/sumeragi/status`.
pub fn set_lane_activity_snapshot(entries: Vec<LaneActivitySnapshot>) {
    *lane_activity_slot().lock().unwrap() = entries;
}

/// Replace the access-set source summary used by `/v1/sumeragi/status`.
pub fn set_access_set_source_summary(summary: AccessSetSourceSummary) {
    *access_set_source_slot().lock().unwrap() = summary;
}

/// Record the latest conflict rate (basis points) for the pipeline DAG.
pub fn set_pipeline_conflict_rate_bps(bps: u64) {
    PIPELINE_CONFLICT_RATE_BPS.store(bps, Ordering::Relaxed);
}

/// Replace the dataspace-activity snapshot used by `/v1/sumeragi/status`.
pub fn set_dataspace_activity_snapshot(entries: Vec<DataspaceActivitySnapshot>) {
    *dataspace_activity_slot().lock().unwrap() = entries;
}

fn lane_activity_snapshot() -> Vec<LaneActivitySnapshot> {
    lane_activity_slot().lock().unwrap().clone()
}

fn access_set_source_snapshot() -> AccessSetSourceSummary {
    *access_set_source_slot().lock().unwrap()
}

fn dataspace_activity_snapshot() -> Vec<DataspaceActivitySnapshot> {
    dataspace_activity_slot().lock().unwrap().clone()
}

fn rbc_lane_backlog_slot() -> &'static Mutex<Vec<LaneRbcSnapshot>> {
    RBC_LANE_BACKLOG.get_or_init(|| Mutex::new(Vec::new()))
}

fn rbc_dataspace_backlog_slot() -> &'static Mutex<Vec<DataspaceRbcSnapshot>> {
    RBC_DATASPACE_BACKLOG.get_or_init(|| Mutex::new(Vec::new()))
}

/// Replace the aggregated RBC lane backlog snapshot used by `/v1/sumeragi/status`.
pub fn set_rbc_lane_backlog(entries: Vec<LaneRbcSnapshot>) {
    *rbc_lane_backlog_slot().lock().unwrap() = entries;
}

/// Replace the aggregated RBC dataspace backlog snapshot used by `/v1/sumeragi/status`.
pub fn set_rbc_dataspace_backlog(entries: Vec<DataspaceRbcSnapshot>) {
    *rbc_dataspace_backlog_slot().lock().unwrap() = entries;
}

fn rbc_lane_backlog_snapshot() -> Vec<LaneRbcSnapshot> {
    rbc_lane_backlog_slot().lock().unwrap().clone()
}

fn rbc_dataspace_backlog_snapshot() -> Vec<DataspaceRbcSnapshot> {
    rbc_dataspace_backlog_slot().lock().unwrap().clone()
}

fn lane_commitments_slot() -> &'static Mutex<Vec<LaneCommitmentSnapshot>> {
    LANE_COMMITMENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn dataspace_commitments_slot() -> &'static Mutex<Vec<DataspaceCommitmentSnapshot>> {
    DATASPACE_COMMITMENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn lane_settlement_commitments_slot() -> &'static Mutex<Vec<LaneBlockCommitment>> {
    LANE_SETTLEMENT_COMMITMENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn lane_relay_envelopes_slot() -> &'static Mutex<Vec<LaneRelayEnvelope>> {
    LANE_RELAY_ENVELOPES.get_or_init(|| Mutex::new(Vec::new()))
}

fn lane_relay_key(
    envelope: &LaneRelayEnvelope,
) -> (
    iroha_data_model::nexus::LaneId,
    iroha_data_model::nexus::DataSpaceId,
    u64,
    HashOf<LaneBlockCommitment>,
) {
    (
        envelope.lane_id,
        envelope.dataspace_id,
        envelope.block_height,
        envelope.settlement_hash,
    )
}

fn record_relay_error(err: &LaneRelayError) {
    if let Some(metrics) = metrics::global() {
        metrics
            .lane_relay_invalid_total
            .with_label_values(&[err.as_label()])
            .inc();
    }
}

fn upsert_lane_relay_envelope(storage: &mut Vec<LaneRelayEnvelope>, envelope: LaneRelayEnvelope) {
    match envelope.verify() {
        Ok(()) => {}
        Err(err) => {
            record_relay_error(&err);
            iroha_logger::warn!(
                lane_id = %envelope.lane_id,
                dataspace_id = %envelope.dataspace_id,
                block_height = envelope.block_height,
                error_kind = err.as_label(),
                error = %err,
                "dropping lane relay envelope with failed verification"
            );
            return;
        }
    }

    let key = lane_relay_key(&envelope);
    if let Some(existing) = storage
        .iter()
        .position(|candidate| lane_relay_key(candidate) == key)
    {
        storage[existing] = envelope;
    } else {
        storage.push(envelope);
        if storage.len() > LANE_RELAY_ENVELOPES_CAP {
            let drain = storage.len() - LANE_RELAY_ENVELOPES_CAP;
            storage.drain(0..drain);
        }
    }
}

/// Replace the aggregated lane/dataspace commitment snapshots used by `/v1/sumeragi/status`.
pub fn set_lane_commitments(
    lane_entries: Vec<LaneCommitmentSnapshot>,
    dataspace_entries: Vec<DataspaceCommitmentSnapshot>,
) {
    {
        let mut guard = lane_commitments_slot()
            .lock()
            .expect("lane commitments lock poisoned");
        *guard = lane_entries;
    }
    {
        let mut guard = dataspace_commitments_slot()
            .lock()
            .expect("dataspace commitments lock poisoned");
        *guard = dataspace_entries;
    }
}

/// Replace the aggregated lane settlement commitments used by `/v1/sumeragi/status`.
pub fn set_lane_settlement_commitments(entries: Vec<LaneBlockCommitment>) {
    let mut guard = lane_settlement_commitments_slot()
        .lock()
        .expect("lane settlement commitments lock poisoned");
    *guard = entries;
}

/// Replace the stored lane relay envelopes captured during block sealing.
pub fn set_lane_relay_envelopes(entries: Vec<LaneRelayEnvelope>) {
    let mut guard = lane_relay_envelopes_slot()
        .lock()
        .expect("lane relay envelopes lock poisoned");
    guard.clear();
    for envelope in entries {
        upsert_lane_relay_envelope(&mut guard, envelope);
    }
}

/// Append a single validated lane relay envelope to the cached snapshot.
pub fn push_lane_relay_envelope(envelope: LaneRelayEnvelope) {
    let mut guard = lane_relay_envelopes_slot()
        .lock()
        .expect("lane relay envelopes lock poisoned");
    upsert_lane_relay_envelope(&mut guard, envelope);
}

fn lane_commitments_snapshot() -> Vec<LaneCommitmentSnapshot> {
    lane_commitments_slot()
        .lock()
        .expect("lane commitments lock poisoned")
        .clone()
}

fn dataspace_commitments_snapshot() -> Vec<DataspaceCommitmentSnapshot> {
    dataspace_commitments_slot()
        .lock()
        .expect("dataspace commitments lock poisoned")
        .clone()
}

fn lane_settlement_commitments_snapshot() -> Vec<LaneBlockCommitment> {
    lane_settlement_commitments_slot()
        .lock()
        .expect("lane settlement commitments lock poisoned")
        .clone()
}

#[allow(dead_code)]
/// Returns the cached lane relay envelopes snapshot used by Sumeragi status endpoints.
pub fn lane_relay_envelopes_snapshot() -> Vec<LaneRelayEnvelope> {
    lane_relay_envelopes_slot()
        .lock()
        .expect("lane relay envelopes lock poisoned")
        .clone()
}

fn lane_governance_slot() -> &'static Mutex<Vec<LaneGovernanceSnapshot>> {
    LANE_GOVERNANCE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Replace the governance manifest snapshot used by `/v1/sumeragi/status`.
pub fn set_lane_governance_snapshot(entries: Vec<LaneGovernanceSnapshot>) {
    *lane_governance_slot()
        .lock()
        .expect("lane governance lock poisoned") = entries;
}

#[cfg_attr(not(any(test, feature = "telemetry")), allow(dead_code))]
/// Returns the cached governance manifest snapshot for Sumeragi status endpoints.
pub fn lane_governance_snapshot() -> Vec<LaneGovernanceSnapshot> {
    lane_governance_slot()
        .lock()
        .expect("lane governance lock poisoned")
        .clone()
}

fn runtime_upgrade_hook_snapshot(hook: &RuntimeUpgradeHook) -> LaneRuntimeUpgradeHookSnapshot {
    LaneRuntimeUpgradeHookSnapshot {
        allow: hook.allow,
        require_metadata: hook.require_metadata,
        metadata_key: hook
            .metadata_key
            .as_ref()
            .map(std::string::ToString::to_string),
        allowed_ids: hook
            .allowed_ids
            .as_ref()
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default(),
    }
}

fn governance_rules_snapshot(
    rules: &GovernanceRules,
) -> (
    Vec<String>,
    Option<u32>,
    Vec<String>,
    Option<LaneRuntimeUpgradeHookSnapshot>,
) {
    let validators = rules
        .validators
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let quorum = rules.quorum;
    let protected_namespaces = rules
        .protected_namespaces
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let runtime_upgrade = rules
        .hooks
        .runtime_upgrade
        .as_ref()
        .map(runtime_upgrade_hook_snapshot);
    (validators, quorum, protected_namespaces, runtime_upgrade)
}

/// Update governance manifest snapshots from the provided registry statuses.
pub fn update_lane_governance_from_statuses(statuses: &[LaneManifestStatus]) {
    let snapshots = statuses
        .iter()
        .map(|status| {
            let manifest_required = status.governance.is_some();
            let manifest_ready = manifest_required && status.manifest_path.is_some();
            let manifest_path = status
                .manifest_path
                .as_ref()
                .map(|path| path.display().to_string());
            let mut snapshot = LaneGovernanceSnapshot {
                lane_id: status.lane.as_u32(),
                alias: status.alias.clone(),
                dataspace_id: status.dataspace.as_u64(),
                visibility: status.visibility.as_str().to_string(),
                storage_profile: status.storage.as_str().to_string(),
                governance: status.governance.clone(),
                manifest_required,
                manifest_ready,
                manifest_path,
                ..LaneGovernanceSnapshot::default()
            };
            if let Some(rules) = status.governance_rules.as_ref() {
                let (validators, quorum, namespaces, runtime_upgrade) =
                    governance_rules_snapshot(rules);
                snapshot.validator_ids = validators;
                snapshot.quorum = quorum;
                snapshot.protected_namespaces = namespaces;
                snapshot.runtime_upgrade = runtime_upgrade;
            }
            snapshot.privacy_commitments = status
                .privacy_commitments
                .iter()
                .map(LanePrivacyCommitmentSnapshot::from)
                .collect();
            snapshot
        })
        .collect();
    set_lane_governance_snapshot(snapshots);
}

fn rbc_store_pressure_label(level: u8) -> &'static str {
    match level {
        0 => "normal",
        1 => "soft",
        2 => "hard",
        _ => "unknown",
    }
}

fn should_log_rbc_store_pressure(
    level: u8,
    prev_level: u8,
    last_log_secs: u64,
    now_secs: u64,
) -> bool {
    if level != prev_level {
        return true;
    }
    if level == 0 {
        return false;
    }
    now_secs.saturating_sub(last_log_secs) >= RBC_STORE_PRESSURE_LOG_INTERVAL_SECS
}

fn maybe_log_rbc_store_pressure(sessions: u64, bytes: u64, level: u8) {
    // TODO: Temporary logging for soak diagnostics; remove or formalize once stable.
    let prev_level = RBC_STORE_PRESSURE_LOG_LEVEL.load(Ordering::Relaxed);
    let last_log_secs = RBC_STORE_PRESSURE_LOG_LAST_SECS.load(Ordering::Relaxed);
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    if !should_log_rbc_store_pressure(level, prev_level, last_log_secs, now_secs) {
        return;
    }
    RBC_STORE_PRESSURE_LOG_LEVEL.store(level, Ordering::Relaxed);
    RBC_STORE_PRESSURE_LOG_LAST_SECS.store(now_secs, Ordering::Relaxed);

    let level_label = rbc_store_pressure_label(level);
    let prev_label = rbc_store_pressure_label(prev_level);
    if level == 0 {
        iroha_logger::info!(
            sessions,
            bytes,
            previous = prev_label,
            "RBC store pressure back to normal"
        );
    } else {
        iroha_logger::warn!(
            sessions,
            bytes,
            level = level_label,
            previous = prev_label,
            "RBC store pressure elevated"
        );
    }
}

/// Update the persisted RBC store pressure snapshot (sessions, bytes, pressure level).
pub fn set_rbc_store_pressure(sessions: u64, bytes: u64, level: u8) {
    RBC_STORE_SESSIONS.store(sessions, Ordering::Relaxed);
    RBC_STORE_BYTES.store(bytes, Ordering::Relaxed);
    RBC_STORE_PRESSURE_LEVEL.store(level, Ordering::Relaxed);
    maybe_log_rbc_store_pressure(sessions, bytes, level);
}

/// Record detailed information about RBC sessions evicted due to TTL or capacity enforcement.
pub fn record_rbc_store_evictions(keys: &[super::rbc_store::SessionKey]) {
    if keys.is_empty() {
        return;
    }

    {
        let mut history = rbc_store_evictions_slot()
            .lock()
            .expect("RBC store eviction history mutex poisoned");
        for &(block_hash, height, view) in keys {
            let hash_bytes: [u8; 32] = UntypedHash::from(block_hash).into();
            history.push_back(RbcEvictedSession {
                block_hash: hash_bytes,
                height,
                view,
            });
            if history.len() > RBC_STORE_RECENT_EVICTIONS_CAP {
                history.pop_front();
            }
        }
    }

    inc_rbc_store_evictions(keys.len() as u64);
}

/// Increment the deferral counter when RBC store pressure prevents proposal assembly.
pub fn inc_rbc_store_backpressure_deferrals() {
    RBC_STORE_BACKPRESSURE_DEFERRALS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment the counter tracking persist requests dropped due to a full async queue.
pub fn inc_rbc_store_persist_drops() {
    RBC_STORE_PERSIST_DROPS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Record the number of RBC sessions evicted while enforcing store limits.
pub fn inc_rbc_store_evictions(count: u64) {
    if count == 0 {
        return;
    }
    RBC_STORE_EVICTIONS_TOTAL.fetch_add(count, Ordering::Relaxed);
}

/// Increment the counter tracking RBC DELIVER deferrals waiting on READY quorum.
pub fn inc_rbc_deliver_defer_ready() {
    RBC_DELIVER_DEFER_READY_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment the counter tracking RBC DELIVER deferrals waiting on missing chunks.
pub fn inc_rbc_deliver_defer_chunks() {
    RBC_DELIVER_DEFER_CHUNKS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment the counter tracking DA deadline reschedules.
pub fn inc_da_reschedule(mode: ConsensusMode) {
    let _ = mode;
    DA_RESCHEDULE_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Record a commit-pipeline execution triggered by the pacemaker tick loop.
pub fn note_commit_pipeline_tick(_mode: ConsensusMode, has_pending: bool) {
    if has_pending {
        COMMIT_PIPELINE_TICK_TOTAL.fetch_add(1, Ordering::Relaxed);
    }
}

fn now_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|dur| u64::try_from(dur.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Set the commit inflight timeout in milliseconds.
pub fn set_commit_inflight_timeout(timeout: Duration) {
    let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
    COMMIT_INFLIGHT_TIMEOUT_MS.store(timeout_ms, Ordering::Relaxed);
}

/// Record the start of a commit inflight window (best-effort).
pub fn record_commit_inflight_start(
    id: u64,
    height: u64,
    view: u64,
    block_hash: HashOf<BlockHeader>,
) {
    let now_ms = now_timestamp_ms();
    COMMIT_INFLIGHT_ACTIVE.store(true, Ordering::Relaxed);
    COMMIT_INFLIGHT_ID.store(id, Ordering::Relaxed);
    COMMIT_INFLIGHT_HEIGHT.store(height, Ordering::Relaxed);
    COMMIT_INFLIGHT_VIEW.store(view, Ordering::Relaxed);
    COMMIT_INFLIGHT_STARTED_MS.store(now_ms, Ordering::Relaxed);
    COMMIT_PAUSE_TOTAL.fetch_add(1, Ordering::Relaxed);
    COMMIT_PAUSE_STARTED_MS.store(now_ms, Ordering::Relaxed);
    if let Ok(mut guard) = commit_inflight_hash_slot().lock() {
        *guard = Some(block_hash.into());
    }
    if let Ok(mut guard) = commit_pause_queue_depths_slot().lock() {
        *guard = worker_queue_depth_snapshot();
    }
}

/// Record completion of the current inflight commit (best-effort).
pub fn record_commit_inflight_finish(id: u64) {
    if !COMMIT_INFLIGHT_ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    if COMMIT_INFLIGHT_ID.load(Ordering::Relaxed) != id {
        return;
    }
    COMMIT_INFLIGHT_ACTIVE.store(false, Ordering::Relaxed);
    COMMIT_INFLIGHT_ID.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_HEIGHT.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_VIEW.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_STARTED_MS.store(0, Ordering::Relaxed);
    COMMIT_PAUSE_STARTED_MS.store(0, Ordering::Relaxed);
    COMMIT_RESUME_TOTAL.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut guard) = commit_inflight_hash_slot().lock() {
        *guard = None;
    }
    if let Ok(mut guard) = commit_resume_queue_depths_slot().lock() {
        *guard = worker_queue_depth_snapshot();
    }
}

/// Record an inflight commit timeout event (best-effort).
pub fn record_commit_inflight_timeout(
    height: u64,
    view: u64,
    block_hash: HashOf<BlockHeader>,
    elapsed: Duration,
) {
    let elapsed_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    let now_ms = now_timestamp_ms();
    COMMIT_INFLIGHT_TIMEOUT_TOTAL.fetch_add(1, Ordering::Relaxed);
    COMMIT_INFLIGHT_LAST_TIMEOUT_MS.store(elapsed_ms, Ordering::Relaxed);
    COMMIT_INFLIGHT_LAST_TIMEOUT_HEIGHT.store(height, Ordering::Relaxed);
    COMMIT_INFLIGHT_LAST_TIMEOUT_VIEW.store(view, Ordering::Relaxed);
    COMMIT_INFLIGHT_LAST_TIMEOUT_TS_MS.store(now_ms, Ordering::Relaxed);
    if let Ok(mut guard) = commit_inflight_timeout_hash_slot().lock() {
        *guard = Some(block_hash.into());
    }
}

/// Record the latest commit quorum signature tally (best-effort).
pub fn record_commit_quorum_snapshot(
    height: u64,
    view: u64,
    block_hash: HashOf<BlockHeader>,
    present: u64,
    counted: u64,
    set_b_signatures: u64,
    required: u64,
) {
    COMMIT_QUORUM_HEIGHT.store(height, Ordering::Relaxed);
    COMMIT_QUORUM_VIEW.store(view, Ordering::Relaxed);
    COMMIT_QUORUM_PRESENT.store(present, Ordering::Relaxed);
    COMMIT_QUORUM_COUNTED.store(counted, Ordering::Relaxed);
    COMMIT_QUORUM_SET_B.store(set_b_signatures, Ordering::Relaxed);
    COMMIT_QUORUM_REQUIRED.store(required, Ordering::Relaxed);
    COMMIT_QUORUM_LAST_UPDATED_MS.store(now_timestamp_ms(), Ordering::Relaxed);
    if let Ok(mut guard) = commit_quorum_hash_slot().lock() {
        *guard = Some(block_hash.into());
    }
}

/// Increment the counter tracking prevote-quorum timeouts that forced a view change.
pub fn inc_prevote_timeout() {
    PREVOTE_TIMEOUT_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Set the number of collectors targeted so far for the current voting block.
pub fn set_collectors_targeted_current(targeted: u64) {
    COLLECTORS_TARGETED_CURRENT.store(targeted, Ordering::Relaxed);
}

/// Record how many collectors were targeted for the most recently committed block.
pub fn observe_collectors_targeted_per_block(targeted: u64) {
    COLLECTORS_TARGETED_LAST_COMMIT.store(targeted, Ordering::Relaxed);
}

/// Increment the redundant-send counter.
pub fn inc_redundant_sends() {
    REDUNDANT_SEND_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Return the cumulative RBC DELIVER deferral counters (READY quorum, missing chunks).
pub fn rbc_deliver_defer_counters() -> (u64, u64) {
    (
        RBC_DELIVER_DEFER_READY_TOTAL.load(Ordering::Relaxed),
        RBC_DELIVER_DEFER_CHUNKS_TOTAL.load(Ordering::Relaxed),
    )
}

/// Increment the counter tracking QC rebuild attempts.
pub fn inc_qc_rebuild_attempts() {
    QC_REBUILD_ATTEMPTS_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment the counter tracking successful QC rebuilds.
pub fn inc_qc_rebuild_successes() {
    QC_REBUILD_SUCCESSES_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment the counter tracking accepted QCs when some votes were missing locally.
pub fn inc_qc_missing_votes_accepted() {
    QC_MISSING_VOTES_ACCEPTED_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment the counter tracking observed quorums without a cached QC payload.
pub fn inc_qc_quorum_without_qc() {
    QC_QUORUM_WITHOUT_QC_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Return the cumulative number of missing-availability transitions observed.
pub fn da_gate_missing_local_data_total() -> u64 {
    DA_GATE_MISSING_LOCAL_DATA_TOTAL.load(Ordering::Relaxed)
}

/// Return the cumulative number of DA reschedules observed locally.
pub fn da_reschedule_total() -> u64 {
    DA_RESCHEDULE_TOTAL.load(Ordering::Relaxed)
}

/// Return the cumulative number of commit pipeline executions triggered by the pacemaker tick loop.
pub fn commit_pipeline_tick_total() -> u64 {
    COMMIT_PIPELINE_TICK_TOTAL.load(Ordering::Relaxed)
}

/// Return the cumulative number of prevote-quorum timeouts.
pub fn prevote_timeout_total() -> u64 {
    PREVOTE_TIMEOUT_TOTAL.load(Ordering::Relaxed)
}

/// Reset the prevote timeout counter (tests only).
#[cfg(test)]
pub(crate) fn reset_prevote_timeout_for_tests() {
    PREVOTE_TIMEOUT_TOTAL.store(0, Ordering::Relaxed);
}

/// Record the latest transaction-queue backpressure snapshot for operator queries.
pub fn set_tx_queue_backpressure(state: BackpressureState) {
    match state {
        BackpressureState::Healthy { queued, capacity } => {
            TX_QUEUE_DEPTH.store(queued as u64, Ordering::Relaxed);
            TX_QUEUE_CAPACITY.store(capacity.get() as u64, Ordering::Relaxed);
            TX_QUEUE_SATURATED.store(false, Ordering::Relaxed);
        }
        BackpressureState::Saturated { queued, capacity } => {
            TX_QUEUE_DEPTH.store(queued as u64, Ordering::Relaxed);
            TX_QUEUE_CAPACITY.store(capacity.get() as u64, Ordering::Relaxed);
            TX_QUEUE_SATURATED.store(true, Ordering::Relaxed);
        }
    }
}

/// Snapshot the recorded transaction-queue backpressure state.
pub fn tx_queue_backpressure() -> (u64, u64, bool) {
    (
        TX_QUEUE_DEPTH.load(Ordering::Relaxed),
        TX_QUEUE_CAPACITY.load(Ordering::Relaxed),
        TX_QUEUE_SATURATED.load(Ordering::Relaxed),
    )
}

fn worker_queue_counter(kind: WorkerQueueKind) -> &'static AtomicU64 {
    match kind {
        WorkerQueueKind::Votes => &WORKER_QUEUE_VOTE_DEPTH,
        WorkerQueueKind::BlockPayload => &WORKER_QUEUE_BLOCK_PAYLOAD_DEPTH,
        WorkerQueueKind::RbcChunks => &WORKER_QUEUE_RBC_CHUNK_DEPTH,
        WorkerQueueKind::Blocks => &WORKER_QUEUE_BLOCK_DEPTH,
        WorkerQueueKind::Consensus => &WORKER_QUEUE_CONSENSUS_DEPTH,
        WorkerQueueKind::LaneRelay => &WORKER_QUEUE_LANE_RELAY_DEPTH,
        WorkerQueueKind::Background => &WORKER_QUEUE_BACKGROUND_DEPTH,
    }
}

fn worker_queue_blocked_total_counter(kind: WorkerQueueKind) -> &'static AtomicU64 {
    match kind {
        WorkerQueueKind::Votes => &WORKER_QUEUE_VOTE_BLOCKED_TOTAL,
        WorkerQueueKind::BlockPayload => &WORKER_QUEUE_BLOCK_PAYLOAD_BLOCKED_TOTAL,
        WorkerQueueKind::RbcChunks => &WORKER_QUEUE_RBC_CHUNK_BLOCKED_TOTAL,
        WorkerQueueKind::Blocks => &WORKER_QUEUE_BLOCK_BLOCKED_TOTAL,
        WorkerQueueKind::Consensus => &WORKER_QUEUE_CONSENSUS_BLOCKED_TOTAL,
        WorkerQueueKind::LaneRelay => &WORKER_QUEUE_LANE_RELAY_BLOCKED_TOTAL,
        WorkerQueueKind::Background => &WORKER_QUEUE_BACKGROUND_BLOCKED_TOTAL,
    }
}

fn worker_queue_blocked_ms_total_counter(kind: WorkerQueueKind) -> &'static AtomicU64 {
    match kind {
        WorkerQueueKind::Votes => &WORKER_QUEUE_VOTE_BLOCKED_MS_TOTAL,
        WorkerQueueKind::BlockPayload => &WORKER_QUEUE_BLOCK_PAYLOAD_BLOCKED_MS_TOTAL,
        WorkerQueueKind::RbcChunks => &WORKER_QUEUE_RBC_CHUNK_BLOCKED_MS_TOTAL,
        WorkerQueueKind::Blocks => &WORKER_QUEUE_BLOCK_BLOCKED_MS_TOTAL,
        WorkerQueueKind::Consensus => &WORKER_QUEUE_CONSENSUS_BLOCKED_MS_TOTAL,
        WorkerQueueKind::LaneRelay => &WORKER_QUEUE_LANE_RELAY_BLOCKED_MS_TOTAL,
        WorkerQueueKind::Background => &WORKER_QUEUE_BACKGROUND_BLOCKED_MS_TOTAL,
    }
}

fn worker_queue_blocked_max_ms_counter(kind: WorkerQueueKind) -> &'static AtomicU64 {
    match kind {
        WorkerQueueKind::Votes => &WORKER_QUEUE_VOTE_BLOCKED_MAX_MS,
        WorkerQueueKind::BlockPayload => &WORKER_QUEUE_BLOCK_PAYLOAD_BLOCKED_MAX_MS,
        WorkerQueueKind::RbcChunks => &WORKER_QUEUE_RBC_CHUNK_BLOCKED_MAX_MS,
        WorkerQueueKind::Blocks => &WORKER_QUEUE_BLOCK_BLOCKED_MAX_MS,
        WorkerQueueKind::Consensus => &WORKER_QUEUE_CONSENSUS_BLOCKED_MAX_MS,
        WorkerQueueKind::LaneRelay => &WORKER_QUEUE_LANE_RELAY_BLOCKED_MAX_MS,
        WorkerQueueKind::Background => &WORKER_QUEUE_BACKGROUND_BLOCKED_MAX_MS,
    }
}

fn worker_queue_dropped_total_counter(kind: WorkerQueueKind) -> &'static AtomicU64 {
    match kind {
        WorkerQueueKind::Votes => &WORKER_QUEUE_VOTE_DROPPED_TOTAL,
        WorkerQueueKind::BlockPayload => &WORKER_QUEUE_BLOCK_PAYLOAD_DROPPED_TOTAL,
        WorkerQueueKind::RbcChunks => &WORKER_QUEUE_RBC_CHUNK_DROPPED_TOTAL,
        WorkerQueueKind::Blocks => &WORKER_QUEUE_BLOCK_DROPPED_TOTAL,
        WorkerQueueKind::Consensus => &WORKER_QUEUE_CONSENSUS_DROPPED_TOTAL,
        WorkerQueueKind::LaneRelay => &WORKER_QUEUE_LANE_RELAY_DROPPED_TOTAL,
        WorkerQueueKind::Background => &WORKER_QUEUE_BACKGROUND_DROPPED_TOTAL,
    }
}

fn worker_queue_totals_snapshot(
    counter: fn(WorkerQueueKind) -> &'static AtomicU64,
) -> WorkerQueueTotalsSnapshot {
    WorkerQueueTotalsSnapshot {
        vote_rx: counter(WorkerQueueKind::Votes).load(Ordering::Relaxed),
        block_payload_rx: counter(WorkerQueueKind::BlockPayload).load(Ordering::Relaxed),
        rbc_chunk_rx: counter(WorkerQueueKind::RbcChunks).load(Ordering::Relaxed),
        block_rx: counter(WorkerQueueKind::Blocks).load(Ordering::Relaxed),
        consensus_rx: counter(WorkerQueueKind::Consensus).load(Ordering::Relaxed),
        lane_relay_rx: counter(WorkerQueueKind::LaneRelay).load(Ordering::Relaxed),
        background_rx: counter(WorkerQueueKind::Background).load(Ordering::Relaxed),
    }
}

/// Record an enqueue for the given worker-loop queue.
pub fn record_worker_queue_enqueue(kind: WorkerQueueKind) {
    worker_queue_counter(kind).fetch_add(1, Ordering::Relaxed);
}

/// Record a blocking enqueue duration for the given worker-loop queue.
pub fn record_worker_queue_blocked(kind: WorkerQueueKind, blocked: Duration) {
    let blocked_ms = u64::try_from(blocked.as_millis()).unwrap_or(u64::MAX);
    if blocked_ms == 0 {
        return;
    }
    worker_queue_blocked_total_counter(kind).fetch_add(1, Ordering::Relaxed);
    worker_queue_blocked_ms_total_counter(kind).fetch_add(blocked_ms, Ordering::Relaxed);
    let max_counter = worker_queue_blocked_max_ms_counter(kind);
    let _ = max_counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.max(blocked_ms))
    });
}

/// Record a dropped enqueue for the given worker-loop queue.
pub fn record_worker_queue_drop(kind: WorkerQueueKind) {
    worker_queue_dropped_total_counter(kind).fetch_add(1, Ordering::Relaxed);
}

/// Record a drain of one or more entries from the given worker-loop queue.
pub fn record_worker_queue_drain(kind: WorkerQueueKind, drained: usize) {
    if drained == 0 {
        return;
    }
    let delta = u64::try_from(drained).unwrap_or(u64::MAX);
    let counter = worker_queue_counter(kind);
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_sub(delta))
    });
}

/// Update the worker-loop stage marker for diagnostics.
pub fn set_worker_stage(stage: WorkerLoopStage) {
    WORKER_STAGE_ID.store(stage.as_id(), Ordering::Relaxed);
    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|dur| u64::try_from(dur.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    WORKER_STAGE_STARTED_MS.store(now_ms, Ordering::Relaxed);
}

/// Record the duration of the most recent worker-loop iteration (ms).
pub fn record_worker_iteration(iteration_ms: u64) {
    WORKER_LAST_ITERATION_MS.store(iteration_ms, Ordering::Relaxed);
}

pub(crate) fn worker_queue_depth_snapshot() -> WorkerQueueDepthSnapshot {
    WorkerQueueDepthSnapshot {
        vote_rx: WORKER_QUEUE_VOTE_DEPTH.load(Ordering::Relaxed),
        block_payload_rx: WORKER_QUEUE_BLOCK_PAYLOAD_DEPTH.load(Ordering::Relaxed),
        rbc_chunk_rx: WORKER_QUEUE_RBC_CHUNK_DEPTH.load(Ordering::Relaxed),
        block_rx: WORKER_QUEUE_BLOCK_DEPTH.load(Ordering::Relaxed),
        consensus_rx: WORKER_QUEUE_CONSENSUS_DEPTH.load(Ordering::Relaxed),
        lane_relay_rx: WORKER_QUEUE_LANE_RELAY_DEPTH.load(Ordering::Relaxed),
        background_rx: WORKER_QUEUE_BACKGROUND_DEPTH.load(Ordering::Relaxed),
    }
}

fn commit_inflight_hash_slot() -> &'static Mutex<Option<UntypedHash>> {
    COMMIT_INFLIGHT_HASH.get_or_init(|| Mutex::new(None))
}

fn commit_inflight_timeout_hash_slot() -> &'static Mutex<Option<UntypedHash>> {
    COMMIT_INFLIGHT_LAST_TIMEOUT_HASH.get_or_init(|| Mutex::new(None))
}

fn commit_quorum_hash_slot() -> &'static Mutex<Option<UntypedHash>> {
    COMMIT_QUORUM_HASH.get_or_init(|| Mutex::new(None))
}

fn commit_pause_queue_depths_slot() -> &'static Mutex<WorkerQueueDepthSnapshot> {
    COMMIT_PAUSE_QUEUE_DEPTHS.get_or_init(|| Mutex::new(WorkerQueueDepthSnapshot::default()))
}

fn commit_resume_queue_depths_slot() -> &'static Mutex<WorkerQueueDepthSnapshot> {
    COMMIT_RESUME_QUEUE_DEPTHS.get_or_init(|| Mutex::new(WorkerQueueDepthSnapshot::default()))
}

fn commit_inflight_snapshot() -> CommitInflightSnapshot {
    let active = COMMIT_INFLIGHT_ACTIVE.load(Ordering::Relaxed);
    let started_ms = COMMIT_INFLIGHT_STARTED_MS.load(Ordering::Relaxed);
    let now_ms = now_timestamp_ms();
    let elapsed_ms = if active && started_ms > 0 {
        now_ms.saturating_sub(started_ms)
    } else {
        0
    };
    let block_hash = commit_inflight_hash_slot()
        .lock()
        .ok()
        .and_then(|hash| hash.map(HashOf::from_untyped_unchecked));
    let last_timeout_hash = commit_inflight_timeout_hash_slot()
        .lock()
        .ok()
        .and_then(|hash| hash.map(HashOf::from_untyped_unchecked));
    let pause_queue_depths = commit_pause_queue_depths_slot()
        .lock()
        .map(|guard| *guard)
        .unwrap_or_default();
    let resume_queue_depths = commit_resume_queue_depths_slot()
        .lock()
        .map(|guard| *guard)
        .unwrap_or_default();

    CommitInflightSnapshot {
        active,
        id: COMMIT_INFLIGHT_ID.load(Ordering::Relaxed),
        height: COMMIT_INFLIGHT_HEIGHT.load(Ordering::Relaxed),
        view: COMMIT_INFLIGHT_VIEW.load(Ordering::Relaxed),
        block_hash,
        started_ms,
        elapsed_ms,
        timeout_ms: COMMIT_INFLIGHT_TIMEOUT_MS.load(Ordering::Relaxed),
        timeout_total: COMMIT_INFLIGHT_TIMEOUT_TOTAL.load(Ordering::Relaxed),
        last_timeout_timestamp_ms: COMMIT_INFLIGHT_LAST_TIMEOUT_TS_MS.load(Ordering::Relaxed),
        last_timeout_elapsed_ms: COMMIT_INFLIGHT_LAST_TIMEOUT_MS.load(Ordering::Relaxed),
        last_timeout_height: COMMIT_INFLIGHT_LAST_TIMEOUT_HEIGHT.load(Ordering::Relaxed),
        last_timeout_view: COMMIT_INFLIGHT_LAST_TIMEOUT_VIEW.load(Ordering::Relaxed),
        last_timeout_block_hash: last_timeout_hash,
        pause_total: COMMIT_PAUSE_TOTAL.load(Ordering::Relaxed),
        resume_total: COMMIT_RESUME_TOTAL.load(Ordering::Relaxed),
        paused_since_ms: COMMIT_PAUSE_STARTED_MS.load(Ordering::Relaxed),
        pause_queue_depths,
        resume_queue_depths,
    }
}

fn commit_quorum_snapshot() -> CommitQuorumSnapshot {
    let block_hash = commit_quorum_hash_slot()
        .lock()
        .ok()
        .and_then(|hash| hash.map(HashOf::from_untyped_unchecked));
    CommitQuorumSnapshot {
        height: COMMIT_QUORUM_HEIGHT.load(Ordering::Relaxed),
        view: COMMIT_QUORUM_VIEW.load(Ordering::Relaxed),
        block_hash,
        signatures_present: COMMIT_QUORUM_PRESENT.load(Ordering::Relaxed),
        signatures_counted: COMMIT_QUORUM_COUNTED.load(Ordering::Relaxed),
        signatures_set_b: COMMIT_QUORUM_SET_B.load(Ordering::Relaxed),
        signatures_required: COMMIT_QUORUM_REQUIRED.load(Ordering::Relaxed),
        last_updated_ms: COMMIT_QUORUM_LAST_UPDATED_MS.load(Ordering::Relaxed),
    }
}

fn commit_qc_snapshot() -> QcSnapshot {
    let latest = commit_qc_history().into_iter().next();
    if let Some(cert) = latest {
        QcSnapshot {
            height: cert.height,
            view: cert.view,
            epoch: cert.epoch,
            block_hash: Some(cert.subject_block_hash),
            validator_set_hash: Some(cert.validator_set_hash),
            validator_set_len: u64::try_from(cert.validator_set.len()).unwrap_or(u64::MAX),
            signatures_total: u64::try_from(crate::sumeragi::consensus::qc_signer_count(&cert))
                .unwrap_or(u64::MAX),
        }
    } else {
        QcSnapshot::default()
    }
}

fn worker_queue_diagnostics_snapshot() -> WorkerQueueDiagnosticsSnapshot {
    WorkerQueueDiagnosticsSnapshot {
        blocked_total: worker_queue_totals_snapshot(worker_queue_blocked_total_counter),
        blocked_ms_total: worker_queue_totals_snapshot(worker_queue_blocked_ms_total_counter),
        blocked_max_ms: worker_queue_totals_snapshot(worker_queue_blocked_max_ms_counter),
        dropped_total: worker_queue_totals_snapshot(worker_queue_dropped_total_counter),
    }
}

fn worker_loop_snapshot() -> WorkerLoopSnapshot {
    WorkerLoopSnapshot {
        stage: WorkerLoopStage::from_id(WORKER_STAGE_ID.load(Ordering::Relaxed)),
        stage_started_ms: WORKER_STAGE_STARTED_MS.load(Ordering::Relaxed),
        last_iteration_ms: WORKER_LAST_ITERATION_MS.load(Ordering::Relaxed),
        queue_depths: worker_queue_depth_snapshot(),
        queue_diagnostics: worker_queue_diagnostics_snapshot(),
    }
}

#[cfg(test)]
pub(crate) fn reset_worker_loop_snapshot_for_tests() {
    WORKER_STAGE_ID.store(WorkerLoopStage::Idle.as_id(), Ordering::Relaxed);
    WORKER_STAGE_STARTED_MS.store(0, Ordering::Relaxed);
    WORKER_LAST_ITERATION_MS.store(0, Ordering::Relaxed);
    WORKER_QUEUE_VOTE_DEPTH.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BLOCK_PAYLOAD_DEPTH.store(0, Ordering::Relaxed);
    WORKER_QUEUE_RBC_CHUNK_DEPTH.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BLOCK_DEPTH.store(0, Ordering::Relaxed);
    WORKER_QUEUE_CONSENSUS_DEPTH.store(0, Ordering::Relaxed);
    WORKER_QUEUE_LANE_RELAY_DEPTH.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BACKGROUND_DEPTH.store(0, Ordering::Relaxed);
    WORKER_QUEUE_VOTE_BLOCKED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BLOCK_PAYLOAD_BLOCKED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_RBC_CHUNK_BLOCKED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BLOCK_BLOCKED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_CONSENSUS_BLOCKED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_LANE_RELAY_BLOCKED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BACKGROUND_BLOCKED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_VOTE_BLOCKED_MS_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BLOCK_PAYLOAD_BLOCKED_MS_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_RBC_CHUNK_BLOCKED_MS_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BLOCK_BLOCKED_MS_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_CONSENSUS_BLOCKED_MS_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_LANE_RELAY_BLOCKED_MS_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BACKGROUND_BLOCKED_MS_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_VOTE_BLOCKED_MAX_MS.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BLOCK_PAYLOAD_BLOCKED_MAX_MS.store(0, Ordering::Relaxed);
    WORKER_QUEUE_RBC_CHUNK_BLOCKED_MAX_MS.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BLOCK_BLOCKED_MAX_MS.store(0, Ordering::Relaxed);
    WORKER_QUEUE_CONSENSUS_BLOCKED_MAX_MS.store(0, Ordering::Relaxed);
    WORKER_QUEUE_LANE_RELAY_BLOCKED_MAX_MS.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BACKGROUND_BLOCKED_MAX_MS.store(0, Ordering::Relaxed);
    WORKER_QUEUE_VOTE_DROPPED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BLOCK_PAYLOAD_DROPPED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_RBC_CHUNK_DROPPED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BLOCK_DROPPED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_CONSENSUS_DROPPED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_LANE_RELAY_DROPPED_TOTAL.store(0, Ordering::Relaxed);
    WORKER_QUEUE_BACKGROUND_DROPPED_TOTAL.store(0, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) fn reset_commit_inflight_for_tests() {
    COMMIT_INFLIGHT_ACTIVE.store(false, Ordering::Relaxed);
    COMMIT_INFLIGHT_ID.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_HEIGHT.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_VIEW.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_STARTED_MS.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_TIMEOUT_MS.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_TIMEOUT_TOTAL.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_LAST_TIMEOUT_MS.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_LAST_TIMEOUT_HEIGHT.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_LAST_TIMEOUT_VIEW.store(0, Ordering::Relaxed);
    COMMIT_INFLIGHT_LAST_TIMEOUT_TS_MS.store(0, Ordering::Relaxed);
    COMMIT_PAUSE_TOTAL.store(0, Ordering::Relaxed);
    COMMIT_RESUME_TOTAL.store(0, Ordering::Relaxed);
    COMMIT_PAUSE_STARTED_MS.store(0, Ordering::Relaxed);
    if let Ok(mut guard) = commit_inflight_hash_slot().lock() {
        *guard = None;
    }
    if let Ok(mut guard) = commit_inflight_timeout_hash_slot().lock() {
        *guard = None;
    }
    if let Ok(mut guard) = commit_pause_queue_depths_slot().lock() {
        *guard = WorkerQueueDepthSnapshot::default();
    }
    if let Ok(mut guard) = commit_resume_queue_depths_slot().lock() {
        *guard = WorkerQueueDepthSnapshot::default();
    }
}

#[cfg(test)]
pub(crate) fn reset_availability_stats_for_tests() {
    let mut stats = availability_slot().lock().unwrap();
    stats.total_votes = 0;
    stats.per_peer.clear();
}

#[cfg(test)]
pub(crate) fn reset_qc_latency_stats_for_tests() {
    qc_latency_slot().lock().unwrap().clear();
}

#[cfg(test)]
pub(crate) fn reset_rbc_backlog_stats_for_tests() {
    *rbc_backlog_slot().lock().unwrap() = RbcBacklogSnapshot::default();
    lane_activity_slot().lock().unwrap().clear();
    *access_set_source_slot().lock().unwrap() = AccessSetSourceSummary::default();
    PIPELINE_CONFLICT_RATE_BPS.store(0, Ordering::Relaxed);
    dataspace_activity_slot().lock().unwrap().clear();
    rbc_lane_backlog_slot().lock().unwrap().clear();
    rbc_dataspace_backlog_slot().lock().unwrap().clear();
}

#[cfg(test)]
pub(crate) fn reset_pending_rbc_for_tests() {
    PENDING_RBC_DROPS_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_DROPS_CAP_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_DROPS_CAP_BYTES_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_DROPS_TTL_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_DROPS_TTL_BYTES_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_DROPPED_BYTES_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_EVICTED_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_STASH_READY_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_STASH_READY_INIT_MISSING_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_STASH_READY_ROSTER_MISSING_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_STASH_READY_ROSTER_HASH_MISMATCH_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_STASH_READY_ROSTER_UNVERIFIED_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_STASH_DELIVER_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_STASH_DELIVER_INIT_MISSING_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_STASH_DELIVER_ROSTER_MISSING_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_STASH_DELIVER_ROSTER_HASH_MISMATCH_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_STASH_DELIVER_ROSTER_UNVERIFIED_TOTAL.store(0, Ordering::Relaxed);
    PENDING_RBC_STASH_CHUNK_TOTAL.store(0, Ordering::Relaxed);
    *pending_rbc_slot().lock().unwrap() = PendingRbcSnapshot::default();
}

#[cfg(test)]
pub(crate) fn reset_qc_rebuild_counters_for_tests() {
    QC_REBUILD_ATTEMPTS_TOTAL.store(0, Ordering::Relaxed);
    QC_REBUILD_SUCCESSES_TOTAL.store(0, Ordering::Relaxed);
    QC_MISSING_VOTES_ACCEPTED_TOTAL.store(0, Ordering::Relaxed);
    QC_QUORUM_WITHOUT_QC_TOTAL.store(0, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) fn reset_collectors_targeting_for_tests() {
    COLLECTORS_TARGETED_CURRENT.store(0, Ordering::Relaxed);
    COLLECTORS_TARGETED_LAST_COMMIT.store(0, Ordering::Relaxed);
    REDUNDANT_SEND_TOTAL.store(0, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) fn reset_view_change_proof_counters_for_tests() {
    VIEW_CHANGE_PROOF_ACCEPTED_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_PROOF_STALE_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_PROOF_REJECTED_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_SUGGEST_TOTAL.store(0, Ordering::Relaxed);
    VIEW_CHANGE_INSTALL_TOTAL.store(0, Ordering::Relaxed);
}

#[cfg(test)]
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestLockOwner {
    Task(tokio::task::Id),
    Thread(std::thread::ThreadId),
}

#[cfg(test)]
impl TestLockOwner {
    fn current() -> Self {
        // Use task IDs so the guard remains reentrant even when async tests hop threads.
        tokio::task::try_id().map_or_else(|| Self::Thread(std::thread::current().id()), Self::Task)
    }
}

#[cfg(test)]
#[derive(Default)]
struct TestLockState {
    owner: Option<TestLockOwner>,
    depth: usize,
}

#[cfg(test)]
#[derive(Default)]
struct TestLock {
    state: Mutex<TestLockState>,
    cvar: Condvar,
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) struct TestLockGuard {
    lock: &'static TestLock,
    owner: TestLockOwner,
}

#[cfg(test)]
impl Drop for TestLockGuard {
    fn drop(&mut self) {
        let mut state = self
            .lock
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.owner == Some(self.owner) {
            state.depth = state.depth.saturating_sub(1);
            if state.depth == 0 {
                state.owner = None;
                self.lock.cvar.notify_one();
            }
        }
    }
}

#[cfg(test)]
fn reentrant_test_guard(lock: &'static OnceLock<TestLock>) -> TestLockGuard {
    let owner = TestLockOwner::current();
    let lock = lock.get_or_init(TestLock::default);
    let mut state = lock
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    loop {
        match state.owner {
            None => {
                state.owner = Some(owner);
                state.depth = 1;
                break;
            }
            Some(current) if current == owner => {
                state.depth = state.depth.saturating_add(1);
                break;
            }
            _ => {
                state = lock
                    .cvar
                    .wait(state)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
    }
    TestLockGuard { lock, owner }
}

#[cfg(test)]
fn test_lock_depth(lock: &'static OnceLock<TestLock>) -> usize {
    let owner = TestLockOwner::current();
    let lock = lock.get_or_init(TestLock::default);
    let state = lock
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.owner == Some(owner) {
        state.depth
    } else {
        0
    }
}

#[cfg(test)]
pub(crate) fn view_change_proof_test_guard() -> std::sync::MutexGuard<'static, ()> {
    VIEW_CHANGE_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("view change test lock poisoned")
}

#[cfg(test)]
pub(crate) fn lane_relay_test_guard() -> std::sync::MutexGuard<'static, ()> {
    LANE_RELAY_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("lane relay test lock poisoned")
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(crate) fn message_handling_test_guard() -> TestLockGuard {
    reentrant_test_guard(&MESSAGE_HANDLING_TEST_LOCK)
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(crate) fn vote_validation_drops_test_guard() -> TestLockGuard {
    reentrant_test_guard(&VOTE_VALIDATION_DROPS_TEST_LOCK)
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(crate) fn missing_block_fetch_test_guard() -> TestLockGuard {
    reentrant_test_guard(&MISSING_BLOCK_FETCH_TEST_LOCK)
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(crate) fn block_sync_test_guard() -> TestLockGuard {
    reentrant_test_guard(&BLOCK_SYNC_TEST_LOCK)
}

#[cfg(test)]
pub(crate) fn prevote_timeout_test_guard() -> std::sync::MutexGuard<'static, ()> {
    PREVOTE_TIMEOUT_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("prevote-timeout test lock poisoned")
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(crate) fn view_change_cause_test_guard() -> TestLockGuard {
    reentrant_test_guard(&VIEW_CHANGE_CAUSE_TEST_LOCK)
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(crate) fn validation_reject_test_guard() -> TestLockGuard {
    reentrant_test_guard(&VALIDATION_REJECT_TEST_LOCK)
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(crate) fn commit_history_test_guard() -> TestLockGuard {
    reentrant_test_guard(&COMMIT_HISTORY_TEST_LOCK)
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(crate) fn kura_store_test_guard() -> TestLockGuard {
    reentrant_test_guard(&KURA_STORE_TEST_LOCK)
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(crate) fn da_gate_test_guard() -> TestLockGuard {
    reentrant_test_guard(&DA_GATE_TEST_LOCK)
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(crate) fn qc_status_test_guard() -> TestLockGuard {
    reentrant_test_guard(&QC_STATUS_TEST_LOCK)
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(crate) fn rbc_status_test_guard() -> TestLockGuard {
    reentrant_test_guard(&RBC_STATUS_TEST_LOCK)
}

#[cfg(test)]
pub(crate) fn reset_rbc_store_evictions_for_tests() {
    RBC_STORE_SESSIONS.store(0, Ordering::Relaxed);
    RBC_STORE_BYTES.store(0, Ordering::Relaxed);
    RBC_STORE_PRESSURE_LEVEL.store(0, Ordering::Relaxed);
    RBC_STORE_BACKPRESSURE_DEFERRALS_TOTAL.store(0, Ordering::Relaxed);
    RBC_STORE_PERSIST_DROPS_TOTAL.store(0, Ordering::Relaxed);
    RBC_STORE_EVICTIONS_TOTAL.store(0, Ordering::Relaxed);
    if let Some(slot) = RBC_STORE_RECENT_EVICTIONS.get() {
        slot.lock()
            .expect("RBC store eviction history mutex poisoned")
            .clear();
    }
}

#[cfg(test)]
/// Reset the logged RBC pressure state for unit tests.
pub(crate) fn reset_rbc_store_pressure_log_state_for_tests() {
    RBC_STORE_PRESSURE_LOG_LEVEL.store(0, Ordering::Relaxed);
    RBC_STORE_PRESSURE_LOG_LAST_SECS.store(0, Ordering::Relaxed);
}

#[cfg(test)]
/// Snapshot the logged RBC pressure state for unit tests.
pub(crate) fn rbc_store_pressure_log_state_for_tests() -> (u8, u64) {
    (
        RBC_STORE_PRESSURE_LOG_LEVEL.load(Ordering::Relaxed),
        RBC_STORE_PRESSURE_LOG_LAST_SECS.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
pub(crate) fn reset_gossip_fallback_for_tests() {
    GOSSIP_FALLBACK_TOTAL.store(0, Ordering::Relaxed);
    BG_POST_DROP_POST_TOTAL.store(0, Ordering::Relaxed);
    BG_POST_DROP_BROADCAST_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_CREATED_DROPPED_BY_LOCK_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_CREATED_HINT_MISMATCH_TOTAL.store(0, Ordering::Relaxed);
    BLOCK_CREATED_PROPOSAL_MISMATCH_TOTAL.store(0, Ordering::Relaxed);
    LAST_PIPELINE_TOTAL_EMA_MS.store(0, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) fn reset_dedup_evictions_for_tests() {
    DEDUP_VOTE_EVICT_CAPACITY_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_VOTE_EVICT_EXPIRED_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_BLOCK_CREATED_EVICT_CAPACITY_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_BLOCK_CREATED_EVICT_EXPIRED_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_PROPOSAL_EVICT_CAPACITY_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_PROPOSAL_EVICT_EXPIRED_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_RBC_INIT_EVICT_CAPACITY_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_RBC_INIT_EVICT_EXPIRED_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_BLOCK_SYNC_UPDATE_EVICT_CAPACITY_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_BLOCK_SYNC_UPDATE_EVICT_EXPIRED_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_FETCH_PENDING_BLOCK_EVICT_CAPACITY_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_FETCH_PENDING_BLOCK_EVICT_EXPIRED_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_RBC_READY_EVICT_CAPACITY_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_RBC_READY_EVICT_EXPIRED_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_RBC_DELIVER_EVICT_CAPACITY_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_RBC_DELIVER_EVICT_EXPIRED_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_RBC_CHUNK_EVICT_CAPACITY_TOTAL.store(0, Ordering::Relaxed);
    DEDUP_RBC_CHUNK_EVICT_EXPIRED_TOTAL.store(0, Ordering::Relaxed);
}

/// Simple struct for phase-latencies snapshot.
#[derive(Clone, Copy, Debug)]
pub struct PhaseLatenciesSnapshot {
    /// Last observed latency for the propose phase (ms).
    pub propose_ms: u64,
    /// Last observed latency for data-availability QC collection (ms).
    pub collect_da_ms: u64,
    /// Last observed latency for prevote QC collection (ms).
    pub collect_prevote_ms: u64,
    /// Last observed latency for precommit QC collection (ms).
    pub collect_precommit_ms: u64,
    /// Last observed latency for redundant collector fan-out (ms).
    pub collect_aggregator_ms: u64,
    /// Last observed latency for commit phase (ms).
    pub commit_ms: u64,
    /// EMA latency for the propose phase (ms).
    pub propose_ema_ms: u64,
    /// EMA latency for data-availability QC collection (ms).
    pub collect_da_ema_ms: u64,
    /// EMA latency for prevote collection (ms).
    pub collect_prevote_ema_ms: u64,
    /// EMA latency for precommit collection (ms).
    pub collect_precommit_ema_ms: u64,
    /// EMA latency for redundant collector fan-out (ms).
    pub collect_aggregator_ema_ms: u64,
    /// EMA latency for commit (ms).
    pub commit_ema_ms: u64,
    /// Aggregate pipeline latency across propose/DA/prevote/precommit/commit (ms).
    pub pipeline_total_ms: u64,
    /// Aggregate pipeline EMA latency across propose/DA/prevote/precommit/commit (ms).
    pub pipeline_total_ema_ms: u64,
    /// Gossip fallback invocations (collectors exhausted).
    pub gossip_fallback_total: u64,
    /// `BlockCreated` messages dropped because the locked QC gate rejected them.
    pub block_created_dropped_by_lock_total: u64,
    /// `BlockCreated` messages rejected due to hint mismatch (height/view/parent).
    pub block_created_hint_mismatch_total: u64,
    /// `BlockCreated` messages rejected due to proposal mismatch (header/payload).
    pub block_created_proposal_mismatch_total: u64,
}

#[cfg(test)]
/// Reset settlement telemetry counters for use in tests.
pub fn settlement_status_reset_for_tests() {
    if let Ok(mut guard) = settlement_status_slot().lock() {
        *guard = SettlementStatusState::default();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::{NonZeroU64, NonZeroUsize},
        str::FromStr,
        sync::atomic::Ordering,
        time::Duration,
    };

    use iroha_config::parameters::actual::ConsensusMode;
    use iroha_crypto::{
        Hash as UntypedHash, HashOf, KeyPair,
        privacy::{
            LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment, SnarkCircuit, SnarkCircuitId,
        },
    };
    use iroha_data_model::{
        block::{
            BlockHeader,
            consensus::{LaneBlockCommitment, LaneSettlementReceipt},
        },
        consensus::{
            ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus, Qc,
            ValidatorSetCheckpoint,
        },
        name::Name,
        nexus::{DataSpaceId, LaneId, LaneRelayEnvelope, LaneStorageProfile, LaneVisibility},
        peer::PeerId,
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_schema::Ident;
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    use super::{
        AccessSetSourceSummary, BackpressureState, GateReason, LanePrivacyCommitmentSchemeSnapshot,
        ManifestGateKind, PrecommitSignerRecord, WorkerLoopStage, WorkerQueueKind,
    };
    use crate::governance::manifest::{GovernanceHooks, GovernanceRules, RuntimeUpgradeHook};
    use crate::sumeragi::consensus::{PERMISSIONED_TAG, Phase, QcAggregate};

    #[test]
    fn locked_qc_updates_monotonically() {
        let _guard = super::qc_status_test_guard();
        super::set_locked_qc(0, 0, None);
        let hash_1 = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [1; UntypedHash::LENGTH],
        ));
        let hash_2 = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [2; UntypedHash::LENGTH],
        ));
        super::set_locked_qc(10, 2, Some(hash_1));
        super::set_locked_qc(9, 5, None); // older by height, ignored
        super::set_locked_qc(10, 1, None); // older by view, ignored
        super::set_locked_qc(12, 0, Some(hash_2)); // higher height, accepted
        let snap = super::snapshot();
        assert_eq!(snap.locked_qc_height, 12);
        assert_eq!(snap.locked_qc_view, 0);
        assert_eq!(snap.locked_qc_subject, Some(hash_2));
    }

    #[test]
    fn locked_qc_subject_updates_for_same_height_view() {
        let _guard = super::qc_status_test_guard();
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [3; UntypedHash::LENGTH],
        ));
        let hash_2 = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [4; UntypedHash::LENGTH],
        ));
        super::set_locked_qc(0, 0, None);
        super::set_locked_qc(10, 2, None);
        super::set_locked_qc(10, 2, Some(hash));
        super::set_locked_qc(10, 2, Some(hash_2));
        let snap = super::snapshot();
        assert_eq!(snap.locked_qc_height, 10);
        assert_eq!(snap.locked_qc_view, 2);
        assert_eq!(snap.locked_qc_subject, Some(hash_2));
    }

    #[test]
    fn access_set_source_and_conflict_rate_snapshot_updates() {
        super::reset_rbc_backlog_stats_for_tests();
        let summary = AccessSetSourceSummary {
            manifest_hints: 2,
            entrypoint_hints: 3,
            prepass_merge: 4,
            conservative_fallback: 5,
        };
        super::set_access_set_source_summary(summary);
        super::set_pipeline_conflict_rate_bps(250);

        let snap = super::snapshot();
        assert_eq!(
            snap.access_set_sources.manifest_hints,
            summary.manifest_hints
        );
        assert_eq!(
            snap.access_set_sources.entrypoint_hints,
            summary.entrypoint_hints
        );
        assert_eq!(snap.access_set_sources.prepass_merge, summary.prepass_merge);
        assert_eq!(
            snap.access_set_sources.conservative_fallback,
            summary.conservative_fallback
        );
        assert_eq!(snap.pipeline_conflict_rate_bps, 250);

        super::reset_rbc_backlog_stats_for_tests();
    }

    #[test]
    fn membership_snapshot_tracks_view_hash() {
        super::reset_membership_snapshot_for_tests();
        assert!(super::membership_snapshot().is_none());

        let hash = [0xAB; 32];
        super::set_membership_view_hash(hash, 42, 7, 3);
        let snap = super::membership_snapshot().expect("membership snapshot");
        assert_eq!(snap.height, 42);
        assert_eq!(snap.view, 7);
        assert_eq!(snap.epoch, 3);
        assert_eq!(snap.view_hash, Some(hash));

        super::reset_membership_snapshot_for_tests();
    }

    #[test]
    fn membership_mismatch_tracks_active_peers_and_clears() {
        super::reset_membership_mismatch_for_tests();
        let peer_a = PeerId::new(KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(KeyPair::random().public_key().clone());
        let local_hash = [0x11; 32];
        let remote_hash = [0x22; 32];

        let count =
            super::record_membership_mismatch(peer_a.clone(), 10, 1, 2, local_hash, remote_hash);
        assert_eq!(count, 1);
        assert_eq!(super::membership_mismatch_consecutive(&peer_a), 1);

        let count =
            super::record_membership_mismatch(peer_a.clone(), 11, 2, 2, local_hash, remote_hash);
        assert_eq!(count, 2);
        assert_eq!(super::membership_mismatch_consecutive(&peer_a), 2);

        let count =
            super::record_membership_mismatch(peer_b.clone(), 12, 3, 2, local_hash, remote_hash);
        assert_eq!(count, 1);

        let snap = super::membership_mismatch_snapshot();
        let active: BTreeSet<_> = snap.active_peers.iter().cloned().collect();
        assert_eq!(active.len(), 2);
        assert!(active.contains(&peer_a));
        assert!(active.contains(&peer_b));
        assert_eq!(snap.last_peer, Some(peer_b.clone()));
        assert_eq!(snap.last_height, 12);
        assert_eq!(snap.last_view, 3);
        assert_eq!(snap.last_epoch, 2);
        assert_eq!(snap.last_local_hash, Some(local_hash));
        assert_eq!(snap.last_remote_hash, Some(remote_hash));
        assert!(snap.last_timestamp_ms > 0);

        super::clear_membership_mismatch(&peer_a);
        let snap = super::membership_mismatch_snapshot();
        assert!(!snap.active_peers.iter().any(|peer| peer == &peer_a));
        assert_eq!(super::membership_mismatch_consecutive(&peer_a), 0);

        super::clear_membership_mismatch(&peer_b);
        super::reset_membership_mismatch_for_tests();
    }

    #[test]
    fn rbc_mismatch_snapshot_tracks_counts_per_peer() {
        super::reset_rbc_mismatch_for_tests();
        let peer_a = PeerId::new(KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(KeyPair::random().public_key().clone());

        super::record_rbc_mismatch(&peer_a, super::RbcMismatchKind::PayloadHash);
        super::record_rbc_mismatch(&peer_a, super::RbcMismatchKind::ChunkDigest);
        super::record_rbc_mismatch(&peer_a, super::RbcMismatchKind::ChunkDigest);
        super::record_rbc_mismatch(&peer_b, super::RbcMismatchKind::ChunkRoot);

        let snap = super::rbc_mismatch_snapshot();
        assert_eq!(snap.entries.len(), 2);
        let entry_a = snap
            .entries
            .iter()
            .find(|entry| entry.peer_id == peer_a)
            .expect("peer_a entry");
        assert_eq!(entry_a.payload_hash_mismatch_total, 1);
        assert_eq!(entry_a.chunk_digest_mismatch_total, 2);
        assert_eq!(entry_a.chunk_root_mismatch_total, 0);
        assert!(entry_a.last_timestamp_ms > 0);

        let entry_b = snap
            .entries
            .iter()
            .find(|entry| entry.peer_id == peer_b)
            .expect("peer_b entry");
        assert_eq!(entry_b.payload_hash_mismatch_total, 0);
        assert_eq!(entry_b.chunk_digest_mismatch_total, 0);
        assert_eq!(entry_b.chunk_root_mismatch_total, 1);
        assert!(entry_b.last_timestamp_ms > 0);

        super::reset_rbc_mismatch_for_tests();
    }

    #[test]
    fn vote_validation_drop_snapshot_tracks_entries() {
        super::reset_vote_validation_drops_for_tests();
        let peer = PeerId::new(KeyPair::random().public_key().clone());
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xAB; UntypedHash::LENGTH],
        ));
        let roster_hash = HashOf::new(&vec![peer.clone()]);

        super::record_vote_validation_drop(super::VoteValidationDropRecord {
            reason: super::VoteValidationDropReason::SignatureInvalid,
            height: 3,
            view: 1,
            epoch: 0,
            signer_index: 0,
            peer_id: Some(peer.clone()),
            roster_hash: Some(roster_hash),
            roster_len: 1,
            block_hash,
        });

        let snap = super::snapshot().vote_validation_drops;
        assert_eq!(snap.total, 1);
        assert_eq!(snap.entries.len(), 1);
        let entry = &snap.entries[0];
        assert_eq!(
            entry.reason,
            super::VoteValidationDropReason::SignatureInvalid
        );
        assert_eq!(entry.peer_id, Some(peer));
        assert_eq!(entry.roster_hash, Some(roster_hash));
        assert_eq!(entry.block_hash, block_hash);
        assert_eq!(entry.roster_len, 1);
        assert!(entry.timestamp_ms > 0);
    }

    #[test]
    fn vote_validation_drop_snapshot_tracks_peer_entries() {
        super::reset_vote_validation_drops_for_tests();
        let peer = PeerId::new(KeyPair::random().public_key().clone());
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xBC; UntypedHash::LENGTH],
        ));
        let roster_hash = HashOf::new(&vec![peer.clone()]);

        super::record_vote_validation_drop(super::VoteValidationDropRecord {
            reason: super::VoteValidationDropReason::SignatureInvalid,
            height: 3,
            view: 1,
            epoch: 0,
            signer_index: 0,
            peer_id: Some(peer.clone()),
            roster_hash: Some(roster_hash),
            roster_len: 1,
            block_hash,
        });
        super::record_vote_validation_drop(super::VoteValidationDropRecord {
            reason: super::VoteValidationDropReason::Duplicate,
            height: 4,
            view: 2,
            epoch: 1,
            signer_index: 0,
            peer_id: Some(peer.clone()),
            roster_hash: Some(roster_hash),
            roster_len: 1,
            block_hash,
        });

        let snap = super::snapshot().vote_validation_drops;
        let entry = snap
            .peer_entries
            .iter()
            .find(|entry| entry.peer_id == peer)
            .expect("peer entry");
        let reason_counts: BTreeMap<_, _> = entry
            .reasons
            .iter()
            .map(|entry| (entry.reason, entry.total))
            .collect();
        assert_eq!(entry.total, 2);
        assert_eq!(entry.roster_hash, Some(roster_hash));
        assert_eq!(entry.roster_len, 1);
        assert_eq!(
            reason_counts.get(&super::VoteValidationDropReason::SignatureInvalid),
            Some(&1)
        );
        assert_eq!(
            reason_counts.get(&super::VoteValidationDropReason::Duplicate),
            Some(&1)
        );
        assert_eq!(entry.last_height, 4);
        assert_eq!(entry.last_view, 2);
        assert_eq!(entry.last_epoch, 1);
        assert!(entry.last_timestamp_ms > 0);
    }

    #[test]
    fn vote_validation_drop_log_thresholds_are_decadic() {
        assert!(super::should_log_vote_drop_count(1));
        assert!(super::should_log_vote_drop_count(10));
        assert!(super::should_log_vote_drop_count(100));
        assert!(super::should_log_vote_drop_count(1_000));
        assert!(!super::should_log_vote_drop_count(0));
        assert!(!super::should_log_vote_drop_count(2));
        assert!(!super::should_log_vote_drop_count(11));
        assert!(!super::should_log_vote_drop_count(20));
    }

    #[test]
    fn commit_history_test_guard_is_reentrant() {
        let outer = super::commit_history_test_guard();
        assert_eq!(super::test_lock_depth(&super::COMMIT_HISTORY_TEST_LOCK), 1);

        {
            let _inner = super::commit_history_test_guard();
            assert_eq!(super::test_lock_depth(&super::COMMIT_HISTORY_TEST_LOCK), 2);
        }

        assert_eq!(super::test_lock_depth(&super::COMMIT_HISTORY_TEST_LOCK), 1);
        drop(outer);
        assert_eq!(super::test_lock_depth(&super::COMMIT_HISTORY_TEST_LOCK), 0);
    }

    #[test]
    fn validator_checkpoint_history_is_capped_and_ordered() {
        let _guard = super::commit_history_test_guard();
        super::reset_validator_checkpoints_for_tests();
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xAB; UntypedHash::LENGTH],
        ));
        let peer_a = PeerId::new(KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(KeyPair::random().public_key().clone());
        for height in 0..(super::VALIDATOR_CHECKPOINT_HISTORY_CAP as u64 + 5) {
            let checkpoint = ValidatorSetCheckpoint::new(
                height,
                0,
                block_hash,
                UntypedHash::prehashed([0u8; UntypedHash::LENGTH]),
                UntypedHash::prehashed([0u8; UntypedHash::LENGTH]),
                vec![peer_a.clone(), peer_b.clone()],
                Vec::new(),
                Vec::new(),
                iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                None,
            );
            super::record_validator_checkpoint(checkpoint);
        }

        let history = super::validator_checkpoint_history();
        assert_eq!(history.len(), super::VALIDATOR_CHECKPOINT_HISTORY_CAP);
        let newest = history.first().expect("history not empty");
        assert_eq!(
            newest.height,
            super::VALIDATOR_CHECKPOINT_HISTORY_CAP as u64 + 4
        );
        let oldest = history.last().expect("history has tail");
        assert_eq!(oldest.height, 5);
    }

    #[test]
    fn commit_qc_history_is_capped_and_ordered() {
        let _guard = super::commit_history_test_guard();
        super::reset_commit_certs_for_tests();
        let old_cap = super::commit_cert_history_cap();
        super::set_commit_cert_history_cap(8);
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xCD; UntypedHash::LENGTH],
        ));
        let peer_a = PeerId::new(KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(KeyPair::random().public_key().clone());
        let validator_set = vec![peer_a.clone(), peer_b.clone()];
        let validator_set_hash = HashOf::new(&validator_set);
        let cap = super::commit_cert_history_cap();
        for height in 0..(cap as u64 + 5) {
            let cert = Qc {
                phase: Phase::Commit,
                subject_block_hash: block_hash,
                parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
                post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
                height,
                view: height * 2,
                epoch: 0,
                mode_tag: PERMISSIONED_TAG.to_string(),
                highest_qc: None,
                validator_set_hash,
                validator_set_hash_version:
                    iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                validator_set: validator_set.clone(),
                aggregate: QcAggregate {
                    signers_bitmap: Vec::new(),
                    bls_aggregate_signature: Vec::new(),
                },
            };
            super::record_commit_qc(cert);
        }

        let history = super::commit_qc_history();
        assert_eq!(history.len(), cap);
        let newest = history.first().expect("history not empty");
        assert_eq!(newest.height, cap as u64 + 4);
        let oldest = history.last().expect("history has tail");
        assert_eq!(oldest.height, 5);
        assert_eq!(oldest.view, 10);
        super::set_commit_cert_history_cap(old_cap);
    }

    #[test]
    fn commit_quorum_snapshot_records_tally() {
        super::reset_commit_quorum_for_tests();
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xDA; UntypedHash::LENGTH],
        ));

        super::record_commit_quorum_snapshot(12, 3, block_hash, 6, 5, 1, 5);

        let snapshot = super::snapshot().commit_quorum;
        assert_eq!(snapshot.height, 12);
        assert_eq!(snapshot.view, 3);
        assert_eq!(snapshot.block_hash, Some(block_hash));
        assert_eq!(snapshot.signatures_present, 6);
        assert_eq!(snapshot.signatures_counted, 5);
        assert_eq!(snapshot.signatures_set_b, 1);
        assert_eq!(snapshot.signatures_required, 5);
        assert!(snapshot.last_updated_ms > 0);
    }

    #[test]
    fn commit_qc_snapshot_tracks_latest() {
        let _guard = super::commit_history_test_guard();
        super::reset_commit_certs_for_tests();
        let block_hash_a = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xC1; UntypedHash::LENGTH],
        ));
        let block_hash_b = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xC2; UntypedHash::LENGTH],
        ));
        let peer_a = PeerId::new(KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(KeyPair::random().public_key().clone());
        let validator_set = vec![peer_a, peer_b];
        let validator_set_hash = HashOf::new(&validator_set);

        let first = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash_a,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 1,
            view: 1,
            epoch: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash,
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: validator_set.clone(),
            aggregate: QcAggregate {
                signers_bitmap: Vec::new(),
                bls_aggregate_signature: Vec::new(),
            },
        };
        let second = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash_b,
            parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
            height: 2,
            view: 4,
            epoch: 1,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash,
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: Vec::new(),
                bls_aggregate_signature: Vec::new(),
            },
        };

        super::record_commit_qc(first);
        super::record_commit_qc(second.clone());

        let snapshot = super::snapshot().commit_qc;
        assert_eq!(snapshot.height, second.height);
        assert_eq!(snapshot.view, second.view);
        assert_eq!(snapshot.epoch, second.epoch);
        assert_eq!(snapshot.block_hash, Some(second.subject_block_hash));
        assert_eq!(snapshot.validator_set_hash, Some(second.validator_set_hash));
        assert_eq!(snapshot.validator_set_len, 2);
        assert_eq!(snapshot.signatures_total, 0);
    }

    #[test]
    fn consensus_key_history_is_capped_and_ordered() {
        super::reset_consensus_keys_for_tests();
        for idx in 0..(super::KEY_LIFECYCLE_HISTORY_CAP as u64 + 5) {
            let ident =
                Ident::from_str(&format!("validator-{idx}")).expect("static ident must parse");
            let record = ConsensusKeyRecord {
                id: ConsensusKeyId::new(ConsensusKeyRole::Validator, ident),
                public_key: KeyPair::random().public_key().clone(),
                pop: None,
                activation_height: idx,
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            super::record_consensus_key(record);
        }

        let history = super::consensus_key_history();
        assert_eq!(history.len(), super::KEY_LIFECYCLE_HISTORY_CAP);
        let newest = history.first().expect("history not empty");
        assert_eq!(
            newest.activation_height,
            super::KEY_LIFECYCLE_HISTORY_CAP as u64 + 4
        );
        let oldest = history.last().expect("history has tail");
        assert_eq!(oldest.activation_height, 5);
        assert_eq!(oldest.status, ConsensusKeyStatus::Active);
    }

    #[test]
    fn phase_snapshot_exposes_phase_ema() {
        super::reset_gossip_fallback_for_tests();
        super::set_phase_propose_ms(5);
        super::set_phase_collect_da_ms(6);
        super::set_phase_collect_prevote_ms(7);
        super::set_phase_collect_precommit_ms(8);
        super::set_phase_commit_ms(9);
        super::set_phase_propose_ema_ms(4);
        super::set_phase_collect_da_ema_ms(5);
        super::set_phase_collect_prevote_ema_ms(6);
        super::set_phase_collect_precommit_ema_ms(7);
        super::set_phase_commit_ema_ms(8);
        super::set_phase_pipeline_total_ema_ms(30);
        super::set_phase_collect_aggregator_ms(11);
        super::set_phase_collect_aggregator_ema_ms(31);
        super::inc_gossip_fallback();
        super::inc_gossip_fallback();
        let snapshot = super::phase_latencies_snapshot();
        assert_eq!(snapshot.collect_aggregator_ms, 11);
        assert_eq!(snapshot.collect_aggregator_ema_ms, 31);
        assert_eq!(snapshot.pipeline_total_ms, 35);
        assert_eq!(snapshot.pipeline_total_ema_ms, 30);
        assert_eq!(snapshot.gossip_fallback_total, 2);
        assert_eq!(snapshot.block_created_dropped_by_lock_total, 0);
    }

    #[test]
    fn availability_vote_tracking_records_counts() {
        super::reset_availability_stats_for_tests();
        let peer = iroha_data_model::peer::PeerId::new(KeyPair::random().public_key().clone());
        super::record_availability_vote(3, &peer);
        super::record_availability_vote(3, &peer);
        let snapshot = super::availability_snapshot();
        assert_eq!(snapshot.total, 2);
        assert_eq!(snapshot.collectors.len(), 1);
        let entry = &snapshot.collectors[0];
        assert_eq!(entry.collector_idx, 3);
        assert_eq!(entry.votes_ingested, 2);
    }

    #[test]
    fn qc_latency_tracking_overwrites_previous_value() {
        super::reset_qc_latency_stats_for_tests();
        super::record_qc_latency("availability", 50);
        super::record_qc_latency("availability", 75);
        let entries = super::qc_latency_snapshot();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "availability");
        assert_eq!(entries[0].1, 75);
    }

    #[test]
    fn rbc_backlog_snapshot_updates() {
        let _guard = super::rbc_status_test_guard();
        super::reset_rbc_backlog_stats_for_tests();
        super::set_rbc_backlog_snapshot(10, 4, 2);
        super::set_rbc_lane_backlog(vec![super::LaneRbcSnapshot {
            lane_id: 7,
            tx_count: 3,
            total_chunks: 5,
            pending_chunks: 2,
            rbc_bytes_total: 1_200,
        }]);
        super::set_rbc_dataspace_backlog(vec![super::DataspaceRbcSnapshot {
            lane_id: 7,
            dataspace_id: 42,
            tx_count: 3,
            total_chunks: 5,
            pending_chunks: 2,
            rbc_bytes_total: 1_200,
        }]);
        let snapshot = super::rbc_backlog_snapshot();
        assert_eq!(snapshot.total_missing_chunks, 10);
        assert_eq!(snapshot.max_missing_chunks, 4);
        assert_eq!(snapshot.pending_sessions, 2);
        let status = super::snapshot();
        assert_eq!(status.rbc_lane_backlog.len(), 1);
        assert_eq!(status.rbc_lane_backlog[0].lane_id, 7);
        assert_eq!(status.rbc_dataspace_backlog.len(), 1);
        assert_eq!(status.rbc_dataspace_backlog[0].dataspace_id, 42);
    }

    #[test]
    fn pending_rbc_drop_counters_track_reasons() {
        super::reset_pending_rbc_for_tests();
        super::inc_rbc_pending_drop("cap", 2, 10);
        super::inc_rbc_pending_drop("session_cap", 1, 4);
        super::inc_rbc_pending_drop("ttl", 3, 6);
        super::inc_pending_rbc_evicted(2);

        let snapshot = super::pending_rbc_snapshot();
        assert_eq!(snapshot.drops_total, 6);
        assert_eq!(snapshot.drops_cap_total, 3);
        assert_eq!(snapshot.drops_cap_bytes_total, 14);
        assert_eq!(snapshot.drops_ttl_total, 3);
        assert_eq!(snapshot.drops_ttl_bytes_total, 6);
        assert_eq!(snapshot.drops_bytes_total, 20);
        assert_eq!(snapshot.evicted_total, 2);

        super::reset_pending_rbc_for_tests();
    }

    #[test]
    fn pending_rbc_stash_counters_track_reasons() {
        super::reset_pending_rbc_for_tests();
        super::inc_pending_rbc_stash(super::PendingRbcStashKind::Chunk, None);
        super::inc_pending_rbc_stash(
            super::PendingRbcStashKind::Ready,
            Some(super::PendingRbcStashReason::InitMissing),
        );
        super::inc_pending_rbc_stash(
            super::PendingRbcStashKind::Ready,
            Some(super::PendingRbcStashReason::RosterMissing),
        );
        super::inc_pending_rbc_stash(
            super::PendingRbcStashKind::Ready,
            Some(super::PendingRbcStashReason::RosterHashMismatch),
        );
        super::inc_pending_rbc_stash(
            super::PendingRbcStashKind::Ready,
            Some(super::PendingRbcStashReason::RosterUnverified),
        );
        super::inc_pending_rbc_stash(
            super::PendingRbcStashKind::Deliver,
            Some(super::PendingRbcStashReason::InitMissing),
        );
        super::inc_pending_rbc_stash(
            super::PendingRbcStashKind::Deliver,
            Some(super::PendingRbcStashReason::RosterMissing),
        );
        super::inc_pending_rbc_stash(
            super::PendingRbcStashKind::Deliver,
            Some(super::PendingRbcStashReason::RosterHashMismatch),
        );
        super::inc_pending_rbc_stash(
            super::PendingRbcStashKind::Deliver,
            Some(super::PendingRbcStashReason::RosterUnverified),
        );

        let snapshot = super::pending_rbc_snapshot();
        assert_eq!(snapshot.stash_chunk_total, 1);
        assert_eq!(snapshot.stash_ready_total, 4);
        assert_eq!(snapshot.stash_ready_init_missing_total, 1);
        assert_eq!(snapshot.stash_ready_roster_missing_total, 1);
        assert_eq!(snapshot.stash_ready_roster_hash_mismatch_total, 1);
        assert_eq!(snapshot.stash_ready_roster_unverified_total, 1);
        assert_eq!(snapshot.stash_deliver_total, 4);
        assert_eq!(snapshot.stash_deliver_init_missing_total, 1);
        assert_eq!(snapshot.stash_deliver_roster_missing_total, 1);
        assert_eq!(snapshot.stash_deliver_roster_hash_mismatch_total, 1);
        assert_eq!(snapshot.stash_deliver_roster_unverified_total, 1);

        super::reset_pending_rbc_for_tests();
    }

    #[test]
    fn qc_rebuild_counters_surface_in_snapshot() {
        super::reset_qc_rebuild_counters_for_tests();
        super::inc_qc_rebuild_attempts();
        super::inc_qc_rebuild_attempts();
        super::inc_qc_rebuild_successes();

        let snapshot = super::snapshot();
        assert_eq!(snapshot.qc_rebuild_attempts_total, 2);
        assert_eq!(snapshot.qc_rebuild_successes_total, 1);
    }

    #[test]
    fn missing_block_fetch_counters_surface_in_snapshot() {
        let _guard = super::missing_block_fetch_test_guard();
        super::MISSING_BLOCK_FETCH_TOTAL.store(0, std::sync::atomic::Ordering::Relaxed);
        super::MISSING_BLOCK_FETCH_LAST_TARGETS.store(0, std::sync::atomic::Ordering::Relaxed);
        super::MISSING_BLOCK_FETCH_LAST_DWELL_MS.store(0, std::sync::atomic::Ordering::Relaxed);

        super::record_missing_block_fetch(3, 42);
        let snapshot = super::snapshot();
        assert_eq!(snapshot.missing_block_fetch_total, 1);
        assert_eq!(snapshot.missing_block_fetch_last_targets, 3);
        assert_eq!(snapshot.missing_block_fetch_last_dwell_ms, 42);

        super::record_missing_block_fetch(1, 7);
        let snapshot = super::snapshot();
        assert_eq!(snapshot.missing_block_fetch_total, 2);
        assert_eq!(snapshot.missing_block_fetch_last_targets, 1);
        assert_eq!(snapshot.missing_block_fetch_last_dwell_ms, 7);
    }

    #[test]
    fn consensus_message_handling_counters_surface_in_snapshot() {
        super::reset_message_handling_for_tests();
        super::record_consensus_message_handling(
            super::ConsensusMessageKind::BlockCreated,
            super::ConsensusMessageOutcome::Dropped,
            super::ConsensusMessageReason::FutureWindow,
        );
        super::record_consensus_message_handling(
            super::ConsensusMessageKind::BlockCreated,
            super::ConsensusMessageOutcome::Dropped,
            super::ConsensusMessageReason::FutureWindow,
        );
        super::record_consensus_message_handling(
            super::ConsensusMessageKind::BlockSyncUpdate,
            super::ConsensusMessageOutcome::Deferred,
            super::ConsensusMessageReason::SignatureMismatchDeferred,
        );

        let snapshot = super::snapshot();
        let entries = snapshot.consensus_message_handling.entries;
        let block_created = entries
            .iter()
            .find(|entry| {
                entry.kind == super::ConsensusMessageKind::BlockCreated
                    && entry.outcome == super::ConsensusMessageOutcome::Dropped
                    && entry.reason == super::ConsensusMessageReason::FutureWindow
            })
            .expect("block created drop entry");
        assert_eq!(block_created.total, 2);
        let deferred = entries
            .iter()
            .find(|entry| {
                entry.kind == super::ConsensusMessageKind::BlockSyncUpdate
                    && entry.outcome == super::ConsensusMessageOutcome::Deferred
                    && entry.reason == super::ConsensusMessageReason::SignatureMismatchDeferred
            })
            .expect("block sync deferred entry");
        assert_eq!(deferred.total, 1);
        super::reset_message_handling_for_tests();
    }

    #[test]
    fn consensus_message_handling_labels_include_new_variants() {
        assert_eq!(
            super::ConsensusMessageKind::ProposalHint.as_str(),
            "proposal_hint"
        );
        assert_eq!(super::ConsensusMessageKind::Proposal.as_str(), "proposal");
        assert_eq!(super::ConsensusMessageKind::QcVote.as_str(), "qc_vote");
        assert_eq!(super::ConsensusMessageKind::Qc.as_str(), "qc");
        assert_eq!(
            super::ConsensusMessageKind::VrfCommit.as_str(),
            "vrf_commit"
        );
        assert_eq!(super::ConsensusMessageKind::RbcInit.as_str(), "rbc_init");
        assert_eq!(
            super::ConsensusMessageKind::FetchPendingBlock.as_str(),
            "fetch_pending_block"
        );
        assert_eq!(super::ConsensusMessageKind::Evidence.as_str(), "evidence");

        assert_eq!(
            super::ConsensusMessageReason::MissingHighestQc.as_str(),
            "missing_highest_qc"
        );
        assert_eq!(
            super::ConsensusMessageReason::HighestQcMismatch.as_str(),
            "highest_qc_mismatch"
        );
        assert_eq!(
            super::ConsensusMessageReason::ConflictingVote.as_str(),
            "conflicting_vote"
        );
        assert_eq!(
            super::ConsensusMessageReason::PayloadTooLarge.as_str(),
            "payload_too_large"
        );
        assert_eq!(
            super::ConsensusMessageReason::ChunkDigestMismatch.as_str(),
            "chunk_digest_mismatch"
        );
        assert_eq!(
            super::ConsensusMessageReason::InvalidPayload.as_str(),
            "invalid_payload"
        );
        assert_eq!(
            super::ConsensusMessageReason::ModeMismatch.as_str(),
            "mode_mismatch"
        );
        assert_eq!(
            super::ConsensusMessageReason::NotFound.as_str(),
            "not_found"
        );
        assert_eq!(
            super::ConsensusMessageReason::AggregateVerifyDeferred.as_str(),
            "aggregate_verify_deferred"
        );
    }

    #[test]
    fn block_sync_counters_surface_in_snapshot() {
        let _guard = super::block_sync_test_guard();
        super::reset_block_sync_counters_for_tests();
        super::inc_block_sync_drop_invalid_signatures();
        super::inc_block_sync_qc_replaced();
        super::inc_block_sync_qc_derive_failed();
        super::inc_block_sync_roster_source("commit_checkpoint_pair_hint");
        super::inc_block_sync_roster_source("commit_roster_journal");
        super::inc_block_sync_roster_drop_missing();
        super::inc_block_sync_roster_drop_unsolicited_share_blocks();

        let snapshot = super::snapshot();
        assert_eq!(snapshot.block_sync_drop_invalid_signatures_total, 1);
        assert_eq!(snapshot.block_sync_qc_replaced_total, 1);
        assert_eq!(snapshot.block_sync_qc_derive_failed_total, 1);
        assert_eq!(snapshot.block_sync_roster.commit_qc_hint_total, 1);
        assert_eq!(snapshot.block_sync_roster.checkpoint_hint_total, 1);
        assert_eq!(snapshot.block_sync_roster.commit_roster_journal_total, 1);
        assert_eq!(snapshot.block_sync_roster.drop_missing_total, 1);
        assert_eq!(
            snapshot
                .block_sync_roster
                .drop_unsolicited_share_blocks_total,
            1
        );
        super::reset_block_sync_counters_for_tests();
    }

    #[test]
    fn precommit_signer_history_reset_clears_records() {
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [9; UntypedHash::LENGTH],
        ));
        let mut signers = BTreeSet::new();
        signers.insert(0);
        let validator_set = vec![PeerId::new(KeyPair::random().public_key().clone())];
        let zero_root = UntypedHash::prehashed([0u8; UntypedHash::LENGTH]);
        super::record_precommit_signers(PrecommitSignerRecord {
            block_hash,
            height: 1,
            view: 0,
            epoch: 0,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            signers,
            roster_len: 1,
            bls_aggregate_signature: vec![1],
            mode_tag: PERMISSIONED_TAG.to_string(),
            validator_set,
            stake_snapshot: None,
        });
        assert!(!super::precommit_signer_history().is_empty());

        super::reset_precommit_signer_history_for_tests();

        assert!(super::precommit_signer_history().is_empty());
    }

    #[test]
    fn view_change_cause_counters_surface_in_snapshot() {
        let _guard = super::view_change_cause_test_guard();
        super::reset_view_change_cause_counters_for_tests();
        super::record_view_change_cause("commit_failure");
        super::record_view_change_cause("quorum_timeout");
        super::record_view_change_cause("stake_quorum_timeout");
        super::record_view_change_cause("da_gate");
        super::record_view_change_cause("censorship_evidence");
        super::record_view_change_cause("missing_payload");
        super::record_view_change_cause("missing_qc");
        super::record_view_change_cause("validation_reject");

        let snapshot = super::snapshot();
        assert_eq!(snapshot.view_change_causes.commit_failure_total, 1);
        assert_eq!(snapshot.view_change_causes.quorum_timeout_total, 1);
        assert_eq!(snapshot.view_change_causes.stake_quorum_timeout_total, 1);
        assert_eq!(snapshot.view_change_causes.da_gate_total, 1);
        assert_eq!(snapshot.view_change_causes.censorship_evidence_total, 1);
        assert_eq!(snapshot.view_change_causes.missing_payload_total, 1);
        assert_eq!(snapshot.view_change_causes.missing_qc_total, 1);
        assert_eq!(snapshot.view_change_causes.validation_reject_total, 1);
        assert!(snapshot.view_change_causes.last_cause.is_some());
        super::reset_view_change_cause_counters_for_tests();
    }

    #[test]
    fn view_change_cause_timestamps_surface_in_snapshot() {
        let _guard = super::view_change_cause_test_guard();
        super::reset_view_change_cause_counters_for_tests();
        super::record_view_change_cause("missing_payload");

        let snapshot = super::snapshot().view_change_causes;
        assert!(
            snapshot.last_missing_payload_timestamp_ms > 0,
            "timestamp for missing_payload should be recorded"
        );
        assert!(
            snapshot.last_cause_timestamp_ms >= snapshot.last_missing_payload_timestamp_ms,
            "last_cause timestamp should reflect the recorded cause"
        );

        super::reset_view_change_cause_counters_for_tests();
    }

    #[test]
    fn validation_reject_snapshot_tracks_last_reject() {
        let _guard = super::validation_reject_test_guard();
        super::reset_validation_reject_counters_for_tests();
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xA1; UntypedHash::LENGTH],
        ));
        super::record_validation_reject("stateless", 4, 1, block_hash);
        super::record_validation_reject("prev_hash", 5, 2, block_hash);

        let snapshot = super::snapshot();
        assert_eq!(snapshot.validation_rejects.total, 2);
        assert_eq!(snapshot.validation_rejects.stateless_total, 1);
        assert_eq!(snapshot.validation_rejects.prev_hash_total, 1);
        assert_eq!(snapshot.validation_rejects.last_reason, Some("prev_hash"));
        assert_eq!(snapshot.validation_rejects.last_height, Some(5));
        assert_eq!(snapshot.validation_rejects.last_view, Some(2));
        assert_eq!(snapshot.validation_rejects.last_block, Some(block_hash));
        assert!(snapshot.validation_rejects.last_timestamp_ms > 0);

        super::reset_validation_reject_counters_for_tests();
    }

    #[test]
    fn peer_key_policy_snapshot_tracks_last_reject() {
        super::reset_peer_key_policy_counters_for_tests();
        super::record_peer_key_policy_reject(super::PeerKeyPolicyRejectReason::MissingHsm);
        super::record_peer_key_policy_reject(super::PeerKeyPolicyRejectReason::LeadTimeViolation);

        let snapshot = super::snapshot().peer_key_policy;
        assert_eq!(snapshot.total, 2);
        assert_eq!(snapshot.missing_hsm_total, 1);
        assert_eq!(snapshot.lead_time_violation_total, 1);
        assert!(snapshot.last_reason.is_some());
        assert!(snapshot.last_timestamp_ms > 0);

        super::reset_peer_key_policy_counters_for_tests();
    }

    #[test]
    fn validation_reject_snapshot_counts_all_buckets() {
        let _guard = super::validation_reject_test_guard();
        super::reset_validation_reject_counters_for_tests();
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xB2; UntypedHash::LENGTH],
        ));

        super::record_validation_reject("stateless", 1, 1, block_hash);
        super::record_validation_reject("execution", 2, 2, block_hash);
        super::record_validation_reject("prev_hash", 3, 3, block_hash);
        super::record_validation_reject("prev_height", 4, 4, block_hash);
        super::record_validation_reject("topology", 5, 5, block_hash);

        let snapshot = super::snapshot();
        assert_eq!(snapshot.validation_rejects.total, 5);
        assert_eq!(snapshot.validation_rejects.stateless_total, 1);
        assert_eq!(snapshot.validation_rejects.execution_total, 1);
        assert_eq!(snapshot.validation_rejects.prev_hash_total, 1);
        assert_eq!(snapshot.validation_rejects.prev_height_total, 1);
        assert_eq!(snapshot.validation_rejects.topology_total, 1);
        assert_eq!(
            snapshot.validation_rejects.last_reason,
            Some("topology"),
            "last reject reason should reflect the most recent label"
        );
        assert_eq!(snapshot.validation_rejects.last_height, Some(5));
        assert_eq!(snapshot.validation_rejects.last_view, Some(5));
        assert_eq!(snapshot.validation_rejects.last_block, Some(block_hash));
        assert!(snapshot.validation_rejects.last_timestamp_ms > 0);

        super::reset_validation_reject_counters_for_tests();
    }

    #[test]
    fn rbc_abort_snapshot_tracks_latest_height_and_view() {
        super::reset_rbc_abort_counters_for_tests();
        super::record_rbc_abort(7, 2);
        super::record_rbc_abort(9, 4);

        let snapshot = super::snapshot().rbc_abort;
        assert_eq!(snapshot.total, 2);
        assert_eq!(snapshot.last_height, 9);
        assert_eq!(snapshot.last_view, 4);

        super::reset_rbc_abort_counters_for_tests();
    }

    #[test]
    fn kura_store_counters_surface_in_snapshot() {
        let _guard = super::kura_store_test_guard();
        super::reset_kura_store_counters_for_tests();
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xAA; UntypedHash::LENGTH],
        ));

        super::record_kura_store_failure(3, 4, hash);
        super::record_kura_store_retry(2, 15);
        super::inc_kura_store_abort();

        let snapshot = super::snapshot();
        assert_eq!(snapshot.kura_store.failures_total, 1);
        assert_eq!(snapshot.kura_store.abort_total, 1);
        assert_eq!(snapshot.kura_store.stage_total, 0);
        assert_eq!(snapshot.kura_store.rollback_total, 0);
        assert_eq!(snapshot.kura_store.lock_reset_total, 0);
        assert_eq!(snapshot.kura_store.last_height, 3);
        assert_eq!(snapshot.kura_store.last_view, 4);
        assert_eq!(snapshot.kura_store.last_hash, Some(hash));
        assert_eq!(snapshot.kura_store.last_retry_attempt, 2);
        assert_eq!(snapshot.kura_store.last_retry_backoff_ms, 15);
    }

    #[test]
    fn kura_stage_counters_track_stage_and_rollback() {
        let _guard = super::kura_store_test_guard();
        super::reset_kura_store_counters_for_tests();
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xA2; UntypedHash::LENGTH],
        ));
        super::record_kura_stage(7, 3, hash);
        super::record_kura_stage_rollback(7, 3, hash, "store_failure");
        super::record_kura_lock_reset(7, 3, Some(hash), "kura_abort");

        let snapshot = super::snapshot();
        assert_eq!(snapshot.kura_store.stage_total, 1);
        assert_eq!(snapshot.kura_store.stage_last_height, 7);
        assert_eq!(snapshot.kura_store.stage_last_view, 3);
        assert_eq!(snapshot.kura_store.stage_last_hash, Some(hash));
        assert_eq!(snapshot.kura_store.rollback_total, 1);
        assert_eq!(snapshot.kura_store.rollback_last_height, 7);
        assert_eq!(snapshot.kura_store.rollback_last_view, 3);
        assert_eq!(snapshot.kura_store.rollback_last_hash, Some(hash));
        assert_eq!(
            snapshot.kura_store.rollback_last_reason,
            Some("store_failure")
        );
        assert_eq!(snapshot.kura_store.lock_reset_total, 1);
        assert_eq!(snapshot.kura_store.lock_reset_last_height, 7);
        assert_eq!(snapshot.kura_store.lock_reset_last_view, 3);
        assert_eq!(snapshot.kura_store.lock_reset_last_hash, Some(hash));
        assert_eq!(
            snapshot.kura_store.lock_reset_last_reason,
            Some("kura_abort")
        );
        super::reset_kura_store_counters_for_tests();
    }

    #[test]
    fn da_gate_transition_counters_surface_in_snapshot() {
        super::reset_da_gate_counters_for_tests();

        super::record_da_gate_transition(
            None,
            Some(GateReason::ManifestGuard {
                lane: LaneId::new(1),
                epoch: 2,
                sequence: 3,
                kind: ManifestGateKind::Missing,
            }),
        );
        let snapshot = super::snapshot();
        assert_eq!(
            snapshot.da_gate.reason,
            super::DaGateReasonSnapshot::ManifestMissing
        );
        assert_eq!(snapshot.da_gate.missing_local_data_total, 0);
        assert_eq!(snapshot.da_gate.manifest_guard_total, 1);
        assert_eq!(super::da_gate_missing_local_data_total(), 0);
        assert_eq!(
            snapshot.da_gate.last_satisfied,
            super::DaGateSatisfactionSnapshot::None
        );

        super::record_da_gate_transition(
            Some(GateReason::ManifestGuard {
                lane: LaneId::new(1),
                epoch: 2,
                sequence: 3,
                kind: ManifestGateKind::Missing,
            }),
            None,
        );
        let snapshot = super::snapshot();
        assert_eq!(snapshot.da_gate.reason, super::DaGateReasonSnapshot::None);
        assert_eq!(
            snapshot.da_gate.last_satisfied,
            super::DaGateSatisfactionSnapshot::None
        );
    }

    #[test]
    fn snapshot_reports_view_change_proof_counters() {
        let _guard = super::view_change_proof_test_guard();
        super::reset_view_change_proof_counters_for_tests();
        super::inc_view_change_proof_accepted();
        super::inc_view_change_proof_accepted();
        super::inc_view_change_proof_stale();
        super::inc_view_change_proof_rejected();
        super::inc_view_change_proof_rejected();
        super::inc_view_change_suggest();
        super::inc_view_change_install();
        let snap = super::snapshot();
        assert_eq!(snap.view_change_proof_accepted_total, 2);
        assert_eq!(snap.view_change_proof_stale_total, 1);
        assert_eq!(snap.view_change_proof_rejected_total, 2);
        assert_eq!(snap.view_change_suggest_total, 1);
        assert_eq!(snap.view_change_install_total, 1);
    }

    #[test]
    fn rbc_store_pressure_snapshot_updates() {
        let _guard = super::rbc_status_test_guard();
        super::reset_rbc_store_evictions_for_tests();
        super::set_rbc_store_pressure(0, 0, 0);
        super::set_rbc_store_pressure(3, 4_096, 1);
        super::inc_rbc_store_backpressure_deferrals();
        super::inc_rbc_store_persist_drops();
        let key_a = (
            HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
                [0xA1; UntypedHash::LENGTH],
            )),
            42,
            3,
        );
        let key_b = (
            HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
                [0xA2; UntypedHash::LENGTH],
            )),
            43,
            0,
        );
        super::record_rbc_store_evictions(&[key_a, key_b]);
        let snapshot = super::snapshot();
        assert_eq!(snapshot.rbc_store_sessions, 3);
        assert_eq!(snapshot.rbc_store_bytes, 4_096);
        assert_eq!(snapshot.rbc_store_pressure_level, 1);
        assert_eq!(snapshot.rbc_store_backpressure_deferrals_total, 1);
        assert_eq!(snapshot.rbc_store_persist_drops_total, 1);
        assert_eq!(snapshot.rbc_store_evictions_total, 2);
        assert_eq!(snapshot.rbc_store_recent_evictions.len(), 2);
        let latest = &snapshot.rbc_store_recent_evictions[0];
        let expected_latest: [u8; UntypedHash::LENGTH] = UntypedHash::from(key_b.0).into();
        assert_eq!(latest.block_hash, expected_latest);
        assert_eq!(latest.height, key_b.1);
        assert_eq!(latest.view, key_b.2);
        let oldest = &snapshot.rbc_store_recent_evictions[1];
        let expected_oldest: [u8; UntypedHash::LENGTH] = UntypedHash::from(key_a.0).into();
        assert_eq!(oldest.block_hash, expected_oldest);
        assert_eq!(oldest.height, key_a.1);
        assert_eq!(oldest.view, key_a.2);
    }

    #[test]
    fn rbc_store_pressure_log_state_updates_on_transition() {
        let _guard = super::rbc_status_test_guard();
        super::reset_rbc_store_pressure_log_state_for_tests();
        super::set_rbc_store_pressure(2, 8_192, 2);
        let (level, _stamp) = super::rbc_store_pressure_log_state_for_tests();
        assert_eq!(level, 2);

        super::set_rbc_store_pressure(0, 0, 0);
        let (level, _stamp) = super::rbc_store_pressure_log_state_for_tests();
        assert_eq!(level, 0);
    }

    #[test]
    fn rbc_store_pressure_log_interval_respects_state() {
        let interval = super::RBC_STORE_PRESSURE_LOG_INTERVAL_SECS;
        assert!(super::should_log_rbc_store_pressure(1, 0, 0, 0));
        assert!(super::should_log_rbc_store_pressure(0, 1, 10, 10));
        assert!(!super::should_log_rbc_store_pressure(0, 0, 0, interval));
        assert!(!super::should_log_rbc_store_pressure(
            1,
            1,
            0,
            interval.saturating_sub(1)
        ));
        assert!(super::should_log_rbc_store_pressure(1, 1, 0, interval));
    }

    #[test]
    fn collectors_targeting_counters_update() {
        super::reset_collectors_targeting_for_tests();
        super::set_collectors_targeted_current(2);
        super::observe_collectors_targeted_per_block(3);
        super::inc_redundant_sends();
        let snapshot = super::snapshot();
        assert_eq!(snapshot.collectors_targeted_current, 2);
        assert_eq!(snapshot.collectors_targeted_last_per_block, 3);
        assert_eq!(snapshot.redundant_sends_total, 1);
    }

    #[test]
    fn snapshot_tracks_background_drops() {
        super::reset_gossip_fallback_for_tests();
        super::record_bg_post_drop("Post");
        super::record_bg_post_drop("Broadcast");
        super::record_bg_post_drop("Post");
        let snapshot = super::snapshot();
        assert_eq!(snapshot.bg_post_drop_post_total, 2);
        assert_eq!(snapshot.bg_post_drop_broadcast_total, 1);
    }

    #[test]
    fn snapshot_tracks_dedup_evictions() {
        super::reset_dedup_evictions_for_tests();
        super::record_dedup_evictions(super::DedupEvictionKind::Vote, 2, 1);
        super::record_dedup_evictions(super::DedupEvictionKind::Proposal, 0, 3);
        let snapshot = super::snapshot();
        assert_eq!(snapshot.dedup_evictions.vote_capacity_total, 2);
        assert_eq!(snapshot.dedup_evictions.vote_expired_total, 1);
        assert_eq!(snapshot.dedup_evictions.proposal_expired_total, 3);
    }

    #[test]
    fn tx_queue_backpressure_snapshot_tracks_state() {
        let capacity = NonZeroUsize::new(16).expect("non-zero");
        super::set_tx_queue_backpressure(BackpressureState::Healthy {
            queued: 3,
            capacity,
        });
        let (depth, cap, saturated) = super::tx_queue_backpressure();
        assert_eq!(depth, 3);
        assert_eq!(cap, 16);
        assert!(!saturated);

        super::set_tx_queue_backpressure(BackpressureState::Saturated {
            queued: 16,
            capacity,
        });
        let (depth, cap, saturated) = super::tx_queue_backpressure();
        assert_eq!(depth, 16);
        assert_eq!(cap, 16);
        assert!(saturated);

        let snap = super::snapshot();
        assert_eq!(snap.tx_queue_depth, 16);
        assert_eq!(snap.tx_queue_capacity, 16);
        assert!(snap.tx_queue_saturated);

        super::set_tx_queue_backpressure(BackpressureState::Healthy {
            queued: 0,
            capacity: NonZeroUsize::new(1).expect("non-zero"),
        });
    }

    #[test]
    fn worker_loop_snapshot_tracks_stage_and_queue_depths() {
        super::reset_worker_loop_snapshot_for_tests();
        super::record_worker_queue_enqueue(WorkerQueueKind::Votes);
        super::record_worker_queue_enqueue(WorkerQueueKind::Background);
        super::record_worker_queue_drain(WorkerQueueKind::Votes, 1);
        super::record_worker_queue_blocked(WorkerQueueKind::Votes, Duration::from_millis(7));
        super::record_worker_queue_drop(WorkerQueueKind::Background);
        super::set_worker_stage(WorkerLoopStage::DrainVotes);
        super::record_worker_iteration(42);

        let snapshot = super::snapshot();
        assert_eq!(snapshot.worker_loop.stage, WorkerLoopStage::DrainVotes);
        assert!(snapshot.worker_loop.stage_started_ms > 0);
        assert_eq!(snapshot.worker_loop.last_iteration_ms, 42);
        assert_eq!(snapshot.worker_loop.queue_depths.vote_rx, 0);
        assert_eq!(snapshot.worker_loop.queue_depths.background_rx, 1);
        assert_eq!(
            snapshot.worker_loop.queue_diagnostics.blocked_total.vote_rx,
            1
        );
        assert_eq!(
            snapshot
                .worker_loop
                .queue_diagnostics
                .blocked_ms_total
                .vote_rx,
            7
        );
        assert_eq!(
            snapshot
                .worker_loop
                .queue_diagnostics
                .blocked_max_ms
                .vote_rx,
            7
        );
        assert_eq!(
            snapshot
                .worker_loop
                .queue_diagnostics
                .dropped_total
                .background_rx,
            1
        );
    }

    #[test]
    fn commit_inflight_snapshot_tracks_start_and_timeout() {
        super::reset_commit_inflight_for_tests();
        super::set_commit_inflight_timeout(Duration::from_millis(5_000));
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(UntypedHash::prehashed(
            [0xBA; UntypedHash::LENGTH],
        ));
        super::record_commit_inflight_start(7, 10, 2, hash);
        let snapshot = super::snapshot();
        assert!(snapshot.commit_inflight.active);
        assert_eq!(snapshot.commit_inflight.id, 7);
        assert_eq!(snapshot.commit_inflight.height, 10);
        assert_eq!(snapshot.commit_inflight.view, 2);
        assert_eq!(snapshot.commit_inflight.block_hash, Some(hash));
        assert_eq!(snapshot.commit_inflight.timeout_ms, 5_000);
        assert_eq!(snapshot.commit_inflight.pause_total, 1);

        super::record_commit_inflight_timeout(10, 2, hash, Duration::from_millis(123));
        let snapshot = super::snapshot();
        assert_eq!(snapshot.commit_inflight.timeout_total, 1);
        assert_eq!(snapshot.commit_inflight.last_timeout_elapsed_ms, 123);
        assert_eq!(snapshot.commit_inflight.last_timeout_height, 10);
        assert_eq!(snapshot.commit_inflight.last_timeout_view, 2);
        assert_eq!(snapshot.commit_inflight.last_timeout_block_hash, Some(hash));

        super::record_commit_inflight_finish(7);
        let snapshot = super::snapshot();
        assert!(!snapshot.commit_inflight.active);
        assert_eq!(snapshot.commit_inflight.resume_total, 1);
        assert_eq!(snapshot.commit_inflight.block_hash, None);
    }

    #[test]
    fn pacemaker_backpressure_deferral_counter_tracks_increments() {
        super::reset_pacemaker_backpressure_deferrals_for_test();
        let initial = super::snapshot().pacemaker_backpressure_deferrals_total;
        assert_eq!(initial, 0);

        super::inc_pacemaker_backpressure_deferrals();
        let snapshot = super::snapshot();
        assert_eq!(snapshot.pacemaker_backpressure_deferrals_total, 1);
    }

    #[test]
    fn set_vrf_penalties_updates_snapshot() {
        super::set_vrf_penalties(5, 2, 3, 1);
        let snap = super::snapshot();
        assert_eq!(snap.vrf_penalty_epoch, 5);
        assert_eq!(snap.vrf_non_reveal_total, 2);
        assert_eq!(snap.vrf_no_participation_total, 3);
        assert_eq!(snap.vrf_late_reveals_total, 1);
        super::set_vrf_penalties(0, 0, 0, 0);
    }

    #[test]
    fn set_vrf_late_reveals_total_updates_snapshot() {
        super::set_vrf_late_reveals_total(4);
        assert_eq!(super::snapshot().vrf_late_reveals_total, 4);
        super::set_vrf_late_reveals_total(0);
    }

    #[test]
    fn nexus_fee_snapshot_tracks_events() {
        let _guard = super::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        super::reset_nexus_economics_for_tests();
        super::record_nexus_fee_event(super::NexusFeeEvent::Charged {
            payer_kind: super::NexusFeePayer::Payer,
            payer_id: "alice@test".to_owned(),
            amount: 10,
            asset_id: "xor#sora".to_owned(),
        });
        super::record_nexus_fee_event(super::NexusFeeEvent::SponsorDisabled {
            payer_id: "sponsor@test".to_owned(),
        });
        super::record_nexus_fee_event(super::NexusFeeEvent::SponsorUnauthorized {
            sponsor_id: "sponsor@test".to_owned(),
            authority_id: "payer@test".to_owned(),
        });
        let snap = super::nexus_fee_snapshot();
        assert_eq!(snap.charged_total, 1);
        assert_eq!(snap.charged_via_payer_total, 1);
        assert_eq!(snap.sponsor_disabled_total, 1);
        assert_eq!(snap.sponsor_unauthorized_total, 1);
        assert_eq!(snap.last_payer, Some(super::NexusFeePayer::Sponsor));
        assert_eq!(
            snap.last_error.as_deref(),
            Some("sponsor not authorized for authority payer@test")
        );
    }

    #[test]
    fn nexus_staking_snapshot_collects_lane_totals() {
        let _guard = super::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        super::reset_nexus_economics_for_tests();
        let lane = LaneId::new(11);
        let bonded = Numeric::new(500, 0);
        let pending = Numeric::new(125, 0);
        super::record_public_lane_bonded_delta(lane, &bonded, true);
        super::record_public_lane_pending_unbond_delta(lane, &pending, true);
        super::record_public_lane_slash(lane);

        let snap = super::nexus_staking_snapshot();
        let lane_entry = snap.lanes.first().expect("lane snapshot present");
        assert_eq!(lane_entry.lane_id, lane);
        assert_eq!(lane_entry.bonded, bonded);
        assert_eq!(lane_entry.pending_unbond, pending);
        assert_eq!(lane_entry.slash_total, 1);
    }

    #[test]
    #[allow(clippy::too_many_lines)] // scenario coverage requires full manifest lifecycle
    fn lane_governance_snapshot_updates_from_statuses() {
        super::set_lane_governance_snapshot(Vec::new());
        let governance_rules = GovernanceRules {
            version: 1,
            validators: vec![ALICE_ID.clone(), BOB_ID.clone()],
            quorum: Some(2),
            protected_namespaces: ["governance", "treasury"]
                .into_iter()
                .map(|ns| Name::from_str(ns).expect("namespace"))
                .collect(),
            hooks: GovernanceHooks {
                runtime_upgrade: Some(RuntimeUpgradeHook {
                    allow: false,
                    require_metadata: true,
                    metadata_key: Some(Name::from_str("upgrade_id").expect("metadata key")),
                    allowed_ids: Some(
                        ["upgrade-1".to_string(), "upgrade-2".to_string()]
                            .into_iter()
                            .collect::<BTreeSet<_>>(),
                    ),
                }),
                unknown: BTreeMap::new(),
            },
        };
        let privacy_commitments = vec![
            LanePrivacyCommitment::merkle(
                LaneCommitmentId::new(7),
                MerkleCommitment::from_root_bytes([0xAB; 32], 16),
            ),
            LanePrivacyCommitment::snark(
                LaneCommitmentId::new(9),
                SnarkCircuit::new(SnarkCircuitId::new(5), [0xCD; 32], [0xEE; 32], [0x12; 32]),
            ),
        ];
        let status = super::LaneManifestStatus {
            lane: LaneId::new(3),
            alias: "governance".to_string(),
            dataspace: DataSpaceId::GLOBAL,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_string()),
            manifest_path: Some(std::path::PathBuf::from("/manifests/governance.json")),
            governance_rules: Some(governance_rules),
            privacy_commitments,
        };

        super::update_lane_governance_from_statuses(&[status]);
        let snapshot = super::lane_governance_snapshot();
        assert_eq!(snapshot.len(), 1);
        let entry = &snapshot[0];
        assert_eq!(entry.lane_id, 3);
        assert_eq!(entry.dataspace_id, DataSpaceId::GLOBAL.as_u64());
        assert_eq!(entry.visibility, LaneVisibility::Public.as_str());
        assert_eq!(
            entry.storage_profile,
            LaneStorageProfile::FullReplica.as_str()
        );
        assert!(entry.manifest_required);
        assert!(entry.manifest_ready);
        assert_eq!(
            entry.manifest_path.as_deref(),
            Some("/manifests/governance.json")
        );
        assert_eq!(
            entry.validator_ids,
            vec![ALICE_ID.to_string(), BOB_ID.to_string()]
        );
        assert_eq!(entry.quorum, Some(2));
        assert_eq!(
            entry.protected_namespaces,
            vec!["governance".to_string(), "treasury".to_string()]
        );
        let hook = entry
            .runtime_upgrade
            .as_ref()
            .expect("runtime upgrade hook should be present");
        assert!(!hook.allow);
        assert!(hook.require_metadata);
        assert_eq!(hook.metadata_key.as_deref(), Some("upgrade_id"));
        assert_eq!(
            hook.allowed_ids,
            vec!["upgrade-1".to_string(), "upgrade-2".to_string()]
        );
        assert_eq!(entry.privacy_commitments.len(), 2);
        assert_eq!(entry.privacy_commitments[0].id, 7);
        match &entry.privacy_commitments[0].scheme {
            LanePrivacyCommitmentSchemeSnapshot::Merkle { root, max_depth } => {
                assert_eq!(*max_depth, 16);
                assert_eq!(root, &[0xAB; 32]);
            }
            other => panic!("expected merkle snapshot, got {other:?}"),
        }
        assert_eq!(entry.privacy_commitments[1].id, 9);
        match &entry.privacy_commitments[1].scheme {
            LanePrivacyCommitmentSchemeSnapshot::Snark {
                circuit_id,
                verifying_key_digest,
                statement_hash,
                proof_hash,
            } => {
                assert_eq!(*circuit_id, 5);
                assert_eq!(verifying_key_digest, &[0xCD; 32]);
                assert_eq!(statement_hash, &[0xEE; 32]);
                assert_eq!(proof_hash, &[0x12; 32]);
            }
            other => panic!("expected snark snapshot, got {other:?}"),
        }
    }

    fn sample_lane_commitment(
        block_height: u64,
        lane_id: u32,
        dataspace_id: u64,
    ) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height,
            lane_id: LaneId::new(lane_id),
            dataspace_id: DataSpaceId::new(dataspace_id),
            tx_count: 1,
            total_local_micro: 10,
            total_xor_due_micro: 20,
            total_xor_after_haircut_micro: 15,
            total_xor_variance_micro: 5,
            swap_metadata: None,
            receipts: vec![LaneSettlementReceipt {
                source_id: [0xAA; 32],
                local_amount_micro: 10,
                xor_due_micro: 20,
                xor_after_haircut_micro: 15,
                xor_variance_micro: 5,
                timestamp_ms: 1_700_000_000_000,
            }],
        }
    }

    fn lane_relay_envelope(block_height: u64, lane_id: u32) -> LaneRelayEnvelope {
        let header = BlockHeader::new(
            NonZeroU64::new(block_height).expect("non-zero height"),
            None,
            None,
            None,
            1_700_000_000_000,
            0,
        );
        let settlement = sample_lane_commitment(block_height, lane_id, u64::from(lane_id) + 10);
        LaneRelayEnvelope::new(header, None, None, settlement, 0).expect("valid envelope")
    }

    #[test]
    fn lane_relay_envelopes_are_deduplicated_and_bounded() {
        let _guard = super::lane_relay_test_guard();
        super::set_lane_relay_envelopes(Vec::new());

        let envelope = lane_relay_envelope(1, 1);
        super::set_lane_relay_envelopes(vec![envelope.clone()]);
        assert_eq!(super::lane_relay_envelopes_snapshot().len(), 1);

        // Duplicate insertions replace the existing entry without growing the snapshot.
        super::push_lane_relay_envelope(envelope.clone());
        assert_eq!(super::lane_relay_envelopes_snapshot().len(), 1);

        // Push enough distinct envelopes to exceed the cap and ensure the snapshot stays bounded.
        for idx in 0..(super::LANE_RELAY_ENVELOPES_CAP + 5) {
            let lane_id = u32::try_from(idx + 2).expect("lane index fits in u32");
            let height = u64::try_from(idx + 2).expect("block height fits in u64");
            super::push_lane_relay_envelope(lane_relay_envelope(height, lane_id));
        }

        let snapshot = super::lane_relay_envelopes_snapshot();
        assert_eq!(snapshot.len(), super::LANE_RELAY_ENVELOPES_CAP);
        assert!(
            snapshot.iter().all(|entry| entry.block_height >= 3),
            "oldest entries should be evicted when the snapshot is capped"
        );
        assert!(
            snapshot
                .iter()
                .any(|entry| entry.block_height == (super::LANE_RELAY_ENVELOPES_CAP + 2) as u64),
            "latest envelope should be retained after eviction"
        );
    }

    #[test]
    fn lane_relay_envelopes_reject_bad_digests() {
        let _guard = super::lane_relay_test_guard();
        super::set_lane_relay_envelopes(Vec::new());
        let mut envelope = lane_relay_envelope(5, 9);
        // Corrupt the DA commitment hash so verification fails.
        envelope.da_commitment_hash = Some(HashOf::from_untyped_unchecked(UntypedHash::prehashed(
            [0xFF; UntypedHash::LENGTH],
        )));
        super::push_lane_relay_envelope(envelope);
        // No entries should have been stored because verification failed.
        assert!(super::lane_relay_envelopes_snapshot().is_empty());
    }

    #[test]
    fn commit_pipeline_tick_counter_tracks_pending_flag() {
        super::COMMIT_PIPELINE_TICK_TOTAL.store(0, std::sync::atomic::Ordering::Relaxed);
        super::note_commit_pipeline_tick(ConsensusMode::Permissioned, false);
        assert_eq!(super::commit_pipeline_tick_total(), 0);
        super::note_commit_pipeline_tick(ConsensusMode::Permissioned, true);
        assert_eq!(super::commit_pipeline_tick_total(), 1);
    }

    #[test]
    fn prevote_timeout_counter_increments() {
        let _guard = super::prevote_timeout_test_guard();
        super::reset_prevote_timeout_for_tests();
        super::inc_prevote_timeout();
        super::inc_prevote_timeout();
        assert_eq!(super::prevote_timeout_total(), 2);
    }

    #[test]
    fn mode_tags_snapshot_updates() {
        super::set_mode_tags("Permissioned", Some("Npos"), Some(10));
        let (mode, staged, activation, lag) = super::mode_tags();
        assert_eq!(mode, "Permissioned");
        assert_eq!(staged, Some("Npos".to_string()));
        assert_eq!(activation, Some(10));
        assert!(lag.is_none());
    }

    #[test]
    fn mode_activation_lag_snapshot_reflects_setter() {
        super::set_mode_tags("Permissioned", Some("Npos"), Some(5));
        super::set_mode_activation_lag(Some(3));
        let snap = super::snapshot();
        assert_eq!(snap.mode_activation_lag_blocks, Some(3));
        super::set_mode_activation_lag(None);
    }

    #[test]
    fn mode_flip_counters_and_errors_update() {
        super::MODE_FLIP_SUCCESS_TOTAL.store(0, Ordering::Relaxed);
        super::MODE_FLIP_FAIL_TOTAL.store(0, Ordering::Relaxed);
        super::MODE_FLIP_BLOCKED_TOTAL.store(0, Ordering::Relaxed);
        super::MODE_LAST_FLIP_TS_MS.store(0, Ordering::Relaxed);
        super::MODE_LAST_FLIP_TS_SET.store(false, Ordering::Relaxed);
        super::MODE_FLIP_BLOCKED.store(false, Ordering::Relaxed);
        if let Some(slot) = super::MODE_LAST_FLIP_ERROR.get() {
            *slot.lock().unwrap() = None;
        }

        super::set_mode_flip_kill_switch(false);
        super::note_mode_flip_blocked("blocked", 1);
        let snap = super::snapshot();
        assert!(!snap.mode_flip_kill_switch);
        assert!(snap.mode_flip_blocked);
        assert_eq!(snap.mode_flip_blocked_total, 1);
        assert_eq!(snap.last_mode_flip_timestamp_ms, Some(1));
        assert_eq!(snap.last_mode_flip_error.as_deref(), Some("blocked"));

        super::note_mode_flip_failure("failed", 2);
        let snap = super::snapshot();
        assert!(!snap.mode_flip_blocked);
        assert_eq!(snap.mode_flip_fail_total, 1);
        assert_eq!(snap.last_mode_flip_timestamp_ms, Some(2));
        assert_eq!(snap.last_mode_flip_error.as_deref(), Some("failed"));

        super::note_mode_flip_success(3);
        let snap = super::snapshot();
        assert_eq!(snap.mode_flip_success_total, 1);
        assert_eq!(snap.last_mode_flip_timestamp_ms, Some(3));
        assert!(snap.last_mode_flip_error.is_none());
    }
}
