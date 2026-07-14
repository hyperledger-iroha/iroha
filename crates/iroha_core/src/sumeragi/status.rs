//! Process-local operator diagnostics for Sumeragi v2 and Nexus lanes.
//!
//! Consensus state itself is published exclusively as the exact reducer-owned
//! [`SumeragiV2Status`]. The remaining snapshots in this module are
//! non-consensus Nexus economics, settlement, lane, and adapter diagnostics.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(test)]
use std::sync::Condvar;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Mutex, MutexGuard, OnceLock},
};

use iroha_crypto::{
    Hash, Hash as UntypedHash, HashOf,
    privacy::{CommitmentScheme, LanePrivacyCommitment},
};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{
            COMMITTED_LANE_STATUS_APPLICATION_RECEIPT_CONFLICTS_WITH_PREFLIGHT,
            COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD,
            COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION,
            COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR,
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION,
            COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION, LaneBlockCommitment,
            LaneBlockProposalV1, LaneBlockQcV1, SumeragiLaneBlockSessionStatus,
            SumeragiLanePayloadOwnership,
        },
        consensus_v2::SumeragiV2Status,
    },
    consensus::{ConsensusKeyRecord, Qc, ValidatorSetCheckpoint},
    da::commitment::DaCommitmentBundle,
    isi::settlement::{SettlementAtomicity, SettlementExecutionOrder},
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope, LaneRelayError},
    peer::PeerId,
};
use iroha_primitives::numeric::Quantity;
use iroha_telemetry::metrics;
use norito::codec::{Decode, Encode};

#[cfg(test)]
use crate::commit_roster_journal::CommitRosterSnapshot;
use crate::{
    governance::manifest::{GovernanceRules, LaneManifestStatus, RuntimeUpgradeHook},
    queue::{BackpressureState, QueuePressureSnapshot},
};

static SUMERAGI_V2_STATUS: OnceLock<Mutex<Option<SumeragiV2Status>>> = OnceLock::new();
// Serializes destructive Kura transitions with consensus decisions that may
// concurrently advance the same canonical chain boundary.
static CONSENSUS_TRANSITION_GATE: OnceLock<Mutex<()>> = OnceLock::new();
static MODE_TAG: OnceLock<Mutex<String>> = OnceLock::new();
static STAGED_MODE_TAG: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static STAGED_MODE_ACTIVATION_HEIGHT: OnceLock<Mutex<Option<u64>>> = OnceLock::new();
static MODE_ACTIVATION_LAG_BLOCKS: OnceLock<Mutex<Option<u64>>> = OnceLock::new();
static VALIDATOR_CHECKPOINT_HISTORY: OnceLock<Mutex<VecDeque<ValidatorSetCheckpoint>>> =
    OnceLock::new();
static COMMIT_CERT_HISTORY: OnceLock<Mutex<VecDeque<Qc>>> = OnceLock::new();
static LAST_PROPOSE_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_DA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_PREVOTE_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_PRECOMMIT_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_AGG_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COMMIT_MS: AtomicU64 = AtomicU64::new(0);
static MAX_PROPOSE_MS: AtomicU64 = AtomicU64::new(0);
static MAX_COLLECT_DA_MS: AtomicU64 = AtomicU64::new(0);
static MAX_COLLECT_PREVOTE_MS: AtomicU64 = AtomicU64::new(0);
static MAX_COLLECT_PRECOMMIT_MS: AtomicU64 = AtomicU64::new(0);
static MAX_COLLECT_AGG_MS: AtomicU64 = AtomicU64::new(0);
static MAX_COMMIT_MS: AtomicU64 = AtomicU64::new(0);
static LAST_PROPOSE_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_DA_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_PREVOTE_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_PRECOMMIT_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COLLECT_AGG_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_COMMIT_EMA_MS: AtomicU64 = AtomicU64::new(0);
static LAST_PIPELINE_TOTAL_EMA_MS: AtomicU64 = AtomicU64::new(0);
static GOSSIP_FALLBACK_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_CREATED_DROPPED_BY_LOCK_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_CREATED_HINT_MISMATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static BLOCK_CREATED_PROPOSAL_MISMATCH_TOTAL: AtomicU64 = AtomicU64::new(0);
static AVAILABILITY_STATS: OnceLock<Mutex<AvailabilityStats>> = OnceLock::new();
static QC_LATENCY_MS: OnceLock<Mutex<BTreeMap<&'static str, u64>>> = OnceLock::new();
static RBC_BACKLOG: OnceLock<Mutex<RbcBacklogSnapshot>> = OnceLock::new();
static PENDING_RBC_STATE: OnceLock<Mutex<PendingRbcSnapshot>> = OnceLock::new();

const VALIDATOR_CHECKPOINT_HISTORY_CAP: usize = 64;
const COMMIT_CERT_HISTORY_CAP: usize = 512;

/// Guard serializing destructive canonical-chain transitions.
pub(crate) struct ConsensusTransitionGuard {
    _guard: MutexGuard<'static, ()>,
}

/// Serialize a Kura canonical-chain mutation with other consensus transitions.
pub(crate) fn consensus_transition_guard() -> ConsensusTransitionGuard {
    let guard = CONSENSUS_TRANSITION_GATE
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|_| fail_closed_after_consensus_transition_poison());
    ConsensusTransitionGuard { _guard: guard }
}

fn fail_closed_after_consensus_transition_poison() -> ! {
    iroha_logger::error!("consensus transition gate was poisoned; refusing canonical mutation");
    #[cfg(not(test))]
    std::process::abort();
    #[cfg(test)]
    panic!("consensus transition gate poisoned; refusing canonical mutation");
}

/// Opaque test-only view of one authenticated legacy commit-roster snapshot.
///
/// Sumeragi v2 carries finality in its exact Kura-owned v2 artifact and does
/// not mint this capability. Unit tests use the type to prove that exact
/// recovery-metadata fixtures cannot promote raw journal fields independently.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct AuthenticatedCommitRoster(CommitRosterSnapshot);

#[cfg(test)]
impl AuthenticatedCommitRoster {
    /// Return the authenticated commit certificate.
    #[must_use]
    pub(crate) fn commit_qc(&self) -> &crate::sumeragi::consensus::Qc {
        &self.0.commit_qc
    }

    /// Return the validator checkpoint bound to the certificate.
    #[must_use]
    pub(crate) fn validator_checkpoint(
        &self,
    ) -> &iroha_data_model::consensus::ValidatorSetCheckpoint {
        &self.0.validator_checkpoint
    }

    /// Return the optional stake authority bound to the validator roster.
    #[must_use]
    pub(crate) fn stake_snapshot(
        &self,
    ) -> Option<&crate::sumeragi::stake_snapshot::CommitStakeSnapshot> {
        self.0.stake_snapshot.as_ref()
    }

    /// Construct a capability from an internally authenticated fixture.
    ///
    /// This seam is deliberately test-only: production v2 code must never
    /// promote decoded legacy journal metadata into finality authority.
    pub(crate) fn from_snapshot_for_tests(snapshot: CommitRosterSnapshot) -> Option<Self> {
        let qc = &snapshot.commit_qc;
        let checkpoint = &snapshot.validator_checkpoint;
        let exact_checkpoint = checkpoint.height == qc.height
            && checkpoint.view == qc.view
            && checkpoint.block_hash == qc.subject_block_hash
            && checkpoint.parent_state_root == qc.parent_state_root
            && checkpoint.post_state_root == qc.post_state_root
            && checkpoint.chain_order_hash == qc.chain_order_hash
            && checkpoint.rechain_seq == qc.rechain_seq
            && checkpoint.validator_set_hash == qc.validator_set_hash
            && checkpoint.validator_set_hash_version == qc.validator_set_hash_version
            && checkpoint.validator_set == qc.validator_set
            && checkpoint.signers_bitmap == qc.aggregate.signers_bitmap
            && checkpoint.bls_aggregate_signature == qc.aggregate.bls_aggregate_signature
            && checkpoint.expires_at_height.is_none();
        let exact_stake = snapshot
            .stake_snapshot
            .as_ref()
            .is_none_or(|stake| stake.matches_roster(&qc.validator_set));
        (exact_checkpoint && exact_stake).then_some(Self(snapshot))
    }
}

#[cfg(test)]
mod archival_status_tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        block::{BlockHeader, consensus::QcAggregate},
        consensus::{Qc, VALIDATOR_SET_HASH_VERSION_V1, ValidatorSetCheckpoint},
        peer::PeerId,
    };

    use crate::commit_roster_journal::CommitRosterSnapshot;
    use crate::sumeragi::consensus::{PERMISSIONED_TAG, Phase};

    use super::AuthenticatedCommitRoster;

    fn fixture() -> CommitRosterSnapshot {
        let key_pair = KeyPair::try_from_seed(
            b"authenticated-commit-roster-status-test".to_vec(),
            Algorithm::BlsNormal,
        )
        .expect("derive validator fixture");
        let validator_set = vec![PeerId::new(key_pair.public_key().clone())];
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block"));
        let parent_state_root = Hash::new(b"parent-state");
        let post_state_root = Hash::new(b"post-state");
        let chain_order_hash = Hash::new(b"chain-order");
        let signers_bitmap = vec![1];
        let aggregate_signature = vec![0xA5; 96];
        let qc = Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root,
            post_state_root,
            height: 7,
            view: 2,
            epoch: 1,
            chain_order_hash,
            rechain_seq: 3,
            mode_tag: PERMISSIONED_TAG.to_owned(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: validator_set.clone(),
            aggregate: QcAggregate {
                signers_bitmap: signers_bitmap.clone(),
                bls_aggregate_signature: aggregate_signature.clone(),
            },
        };
        let validator_checkpoint = ValidatorSetCheckpoint::new_with_chain_order(
            qc.height,
            qc.view,
            block_hash,
            chain_order_hash,
            qc.rechain_seq,
            parent_state_root,
            post_state_root,
            validator_set,
            signers_bitmap,
            aggregate_signature,
            VALIDATOR_SET_HASH_VERSION_V1,
            None,
        );
        CommitRosterSnapshot {
            commit_qc: qc,
            validator_checkpoint,
            stake_snapshot: None,
        }
    }

    #[test]
    fn capability_exposes_only_an_exact_roster_tuple() {
        let snapshot = fixture();
        let capability = AuthenticatedCommitRoster::from_snapshot_for_tests(snapshot.clone())
            .expect("exact snapshot should mint a test capability");
        assert_eq!(capability.commit_qc(), &snapshot.commit_qc);
        assert_eq!(
            capability.validator_checkpoint(),
            &snapshot.validator_checkpoint
        );
        assert_eq!(capability.stake_snapshot(), None);

        let mut mismatched = snapshot;
        mismatched.validator_checkpoint.view += 1;
        assert!(AuthenticatedCommitRoster::from_snapshot_for_tests(mismatched).is_none());
    }

    #[test]
    fn archival_mode_tags_roundtrip_without_changing_v2_status() {
        let _guard = super::mode_tags_test_guard();
        super::clear_v2_status();
        super::set_mode_tags(PERMISSIONED_TAG, Some("staged"), Some(9));

        assert_eq!(
            super::mode_tags(),
            (
                PERMISSIONED_TAG.to_owned(),
                Some("staged".to_owned()),
                Some(9),
                None,
            )
        );
        assert_eq!(super::v2_status(), None);

        super::set_mode_tags("", None, None);
    }

    #[test]
    fn archival_commit_histories_are_newest_first_and_resettable() {
        let _guard = super::commit_history_test_guard();
        super::reset_commit_certs_for_tests();
        super::reset_validator_checkpoints_for_tests();

        let first = fixture();
        let mut second = first.clone();
        second.commit_qc.height += 1;
        second.validator_checkpoint.height += 1;
        super::record_commit_qc(first.commit_qc.clone());
        super::record_validator_checkpoint(first.validator_checkpoint.clone());
        super::record_commit_qc(second.commit_qc.clone());
        super::record_validator_checkpoint(second.validator_checkpoint.clone());

        assert_eq!(
            super::commit_qc_history()
                .first()
                .map(|certificate| certificate.height),
            Some(second.commit_qc.height)
        );
        assert_eq!(
            super::validator_checkpoint_history()
                .first()
                .map(|checkpoint| checkpoint.height),
            Some(second.validator_checkpoint.height)
        );

        super::reset_commit_certs_for_tests();
        super::reset_validator_checkpoints_for_tests();
        assert!(super::commit_qc_history().is_empty());
        assert!(super::validator_checkpoint_history().is_empty());
    }

    #[test]
    fn lane_rbc_reset_clears_surviving_adapter_diagnostics() {
        let _guard = super::rbc_status_test_guard();
        super::lock_operator_status_slot(super::lane_activity_slot(), "lane activity test").push(
            super::LaneActivitySnapshot {
                lane_id: 7,
                ..super::LaneActivitySnapshot::default()
            },
        );
        super::lock_operator_status_slot(
            super::dataspace_activity_slot(),
            "dataspace activity test",
        )
        .push(super::DataspaceActivitySnapshot {
            lane_id: 7,
            dataspace_id: 9,
            tx_served: 1,
        });
        super::lock_operator_status_slot(
            super::pipeline_execution_slot(),
            "pipeline execution test",
        )
        .rbc_chunks_total = 3;

        super::reset_rbc_backlog_stats_for_tests();

        assert!(
            super::lock_operator_status_slot(super::lane_activity_slot(), "lane activity test")
                .is_empty()
        );
        assert!(
            super::lock_operator_status_slot(
                super::dataspace_activity_slot(),
                "dataspace activity test",
            )
            .is_empty()
        );
        assert_eq!(
            super::lock_operator_status_slot(
                super::pipeline_execution_slot(),
                "pipeline execution test",
            )
            .rbc_chunks_total,
            0
        );
    }
}

/// Publish the exact protocol-v2 reducer snapshot served by Torii.
pub fn set_v2_status(status: SumeragiV2Status) {
    *SUMERAGI_V2_STATUS
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(status);
}

/// Return the latest protocol-v2 reducer snapshot, if v2 has started.
#[must_use]
pub fn v2_status() -> Option<SumeragiV2Status> {
    SUMERAGI_V2_STATUS.get().and_then(|slot| {
        slot.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    })
}

/// Return the latest exact reducer snapshot with process-wide fail-stop state
/// overlaid at read time.
///
/// Kura or snapshot persistence can activate the shared output guard after the
/// reducer's last status publication. Applying the monotonic flag while serving
/// prevents a stale `restart_required = false` observation in that interval.
#[must_use]
pub fn v2_status_with_restart_required(restart_required: bool) -> Option<SumeragiV2Status> {
    v2_status().map(|mut status| {
        status.restart_required |= restart_required;
        status
    })
}

/// Clear protocol-v2 status during shutdown and isolated tests.
pub fn clear_v2_status() {
    if let Some(slot) = SUMERAGI_V2_STATUS.get() {
        *slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
}

/// Record archival consensus-mode labels used by retained evidence validation.
///
/// The labels are process-local diagnostics only. Protocol-v2 consensus mode
/// remains owned by the immutable height context.
pub fn set_mode_tags(
    mode_tag: &str,
    staged_mode_tag: Option<&str>,
    staged_mode_activation_height: Option<u64>,
) {
    *lock_operator_status_slot(
        MODE_TAG.get_or_init(|| Mutex::new(String::new())),
        "mode tag",
    ) = mode_tag.to_owned();
    *lock_operator_status_slot(
        STAGED_MODE_TAG.get_or_init(|| Mutex::new(None)),
        "staged mode tag",
    ) = staged_mode_tag.map(ToOwned::to_owned);
    *lock_operator_status_slot(
        STAGED_MODE_ACTIVATION_HEIGHT.get_or_init(|| Mutex::new(None)),
        "staged mode activation height",
    ) = staged_mode_activation_height;
}

/// Return archival consensus-mode labels used by retained operator routes.
#[must_use]
pub fn mode_tags() -> (String, Option<String>, Option<u64>, Option<u64>) {
    let mode = MODE_TAG
        .get()
        .map(|slot| lock_operator_status_slot(slot, "mode tag").clone())
        .unwrap_or_default();
    let staged = STAGED_MODE_TAG
        .get()
        .map(|slot| lock_operator_status_slot(slot, "staged mode tag").clone())
        .unwrap_or_default();
    let activation = STAGED_MODE_ACTIVATION_HEIGHT
        .get()
        .map(|slot| *lock_operator_status_slot(slot, "staged mode activation height"))
        .unwrap_or_default();
    let lag = MODE_ACTIVATION_LAG_BLOCKS
        .get()
        .map(|slot| *lock_operator_status_slot(slot, "mode activation lag"))
        .unwrap_or_default();
    (mode, staged, activation, lag)
}

/// Legacy lane-RBC mismatch labels retained only by lane-local telemetry.
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
    /// Stable telemetry label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ChunkDigest => "chunk_digest",
            Self::PayloadHash => "payload_hash",
            Self::ChunkRoot => "chunk_root",
        }
    }
}

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

/// Snapshot entry describing availability votes ingested by a collector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailabilityCollectorSnapshot {
    /// Collector topology index.
    pub collector_idx: u64,
    /// Collector peer identifier.
    pub peer: PeerId,
    /// Number of availability votes ingested by this collector.
    pub votes_ingested: u64,
}

/// Aggregated availability vote ingestion snapshot for the telemetry route.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AvailabilitySnapshot {
    /// Total availability votes ingested by this node.
    pub total: u64,
    /// Per-collector vote counts keyed by topology index and peer id.
    pub collectors: Vec<AvailabilityCollectorSnapshot>,
}

/// Aggregated RBC backlog metrics snapshot for the telemetry route.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RbcBacklogSnapshot {
    /// Total missing chunks across active sessions.
    pub total_missing_chunks: u64,
    /// Maximum missing chunks within any single session.
    pub max_missing_chunks: u64,
    /// Number of sessions whose local chunk delivery is still incomplete.
    pub pending_sessions: u64,
}

/// Pending pre-INIT RBC stash entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingRbcSnapshot {
    /// Current pending sessions awaiting INIT.
    pub sessions: u64,
    /// Maximum pending sessions retained.
    pub session_cap: u64,
    /// Aggregate pending chunk frames across sessions.
    pub chunks: u64,
    /// Aggregate pending chunk payload bytes across sessions.
    pub bytes: u64,
    /// Configured per-session chunk cap.
    pub max_chunks_per_session: u64,
    /// Configured per-session byte cap.
    pub max_bytes_per_session: u64,
    /// Configured TTL in milliseconds before pending entries expire.
    pub ttl_ms: u64,
    /// Total pending frames dropped across all reasons.
    pub drops_total: u64,
    /// Total pending frames dropped due to cap enforcement.
    pub drops_cap_total: u64,
    /// Aggregate payload or signature bytes dropped due to caps.
    pub drops_cap_bytes_total: u64,
    /// Total pending frames dropped due to TTL expiry.
    pub drops_ttl_total: u64,
    /// Aggregate payload or signature bytes dropped due to TTL expiry.
    pub drops_ttl_bytes_total: u64,
    /// Total pending bytes dropped across all reasons.
    pub drops_bytes_total: u64,
    /// Total pending sessions evicted.
    pub evicted_total: u64,
    /// Total READY frames stashed before processing.
    pub stash_ready_total: u64,
    /// READY frames stashed because INIT has not arrived.
    pub stash_ready_init_missing_total: u64,
    /// READY frames stashed because the commit roster is missing.
    pub stash_ready_roster_missing_total: u64,
    /// READY frames stashed because the commit roster hash mismatched.
    pub stash_ready_roster_hash_mismatch_total: u64,
    /// READY frames stashed while the commit roster is unverified.
    pub stash_ready_roster_unverified_total: u64,
    /// Total DELIVER frames stashed before processing.
    pub stash_deliver_total: u64,
    /// DELIVER frames stashed because INIT has not arrived.
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

/// Process-local phase-latency and retained compatibility-counter snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhaseLatenciesSnapshot {
    /// Last observed latency for the propose phase in milliseconds.
    pub propose_ms: u64,
    /// Last observed latency for data-availability collection in milliseconds.
    pub collect_da_ms: u64,
    /// Last observed latency for prevote collection in milliseconds.
    pub collect_prevote_ms: u64,
    /// Last observed latency for precommit collection in milliseconds.
    pub collect_precommit_ms: u64,
    /// Last observed latency for redundant collector fan-out in milliseconds.
    pub collect_aggregator_ms: u64,
    /// Last observed latency for the commit phase in milliseconds.
    pub commit_ms: u64,
    /// Maximum propose latency observed since process start.
    pub propose_max_ms: u64,
    /// Maximum data-availability collection latency observed since process start.
    pub collect_da_max_ms: u64,
    /// Maximum prevote collection latency observed since process start.
    pub collect_prevote_max_ms: u64,
    /// Maximum precommit collection latency observed since process start.
    pub collect_precommit_max_ms: u64,
    /// Maximum redundant collector fan-out latency observed since process start.
    pub collect_aggregator_max_ms: u64,
    /// Maximum commit latency observed since process start.
    pub commit_max_ms: u64,
    /// EMA propose latency in milliseconds.
    pub propose_ema_ms: u64,
    /// EMA data-availability collection latency in milliseconds.
    pub collect_da_ema_ms: u64,
    /// EMA prevote collection latency in milliseconds.
    pub collect_prevote_ema_ms: u64,
    /// EMA precommit collection latency in milliseconds.
    pub collect_precommit_ema_ms: u64,
    /// EMA redundant collector fan-out latency in milliseconds.
    pub collect_aggregator_ema_ms: u64,
    /// EMA commit latency in milliseconds.
    pub commit_ema_ms: u64,
    /// Sum of current propose, DA, prevote, precommit, and commit latencies.
    pub pipeline_total_ms: u64,
    /// Saturating sum of the maxima for the pipeline phases.
    pub pipeline_total_max_ms: u64,
    /// EMA latency for the aggregate pipeline in milliseconds.
    pub pipeline_total_ema_ms: u64,
    /// Gossip fallback invocations after collectors were exhausted.
    pub gossip_fallback_total: u64,
    /// Block-created messages dropped by the locked-QC gate.
    pub block_created_dropped_by_lock_total: u64,
    /// Block-created messages rejected due to hint mismatch.
    pub block_created_hint_mismatch_total: u64,
    /// Block-created messages rejected due to proposal mismatch.
    pub block_created_proposal_mismatch_total: u64,
}

fn lock_operator_status_slot<T>(
    slot: &'static Mutex<T>,
    label: &'static str,
) -> MutexGuard<'static, T> {
    match slot.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            iroha_logger::warn!(
                "Sumeragi {label} mutex was poisoned; recovering operator status snapshot"
            );
            poisoned.into_inner()
        }
    }
}

static SETTLEMENT_STATUS: OnceLock<Mutex<SettlementStatusState>> = OnceLock::new();
static LANE_ACTIVITY: OnceLock<Mutex<Vec<LaneActivitySnapshot>>> = OnceLock::new();
static PIPELINE_EXECUTION: OnceLock<Mutex<PipelineExecutionSnapshot>> = OnceLock::new();
static ACCESS_SET_SOURCES: OnceLock<Mutex<AccessSetSourceSummary>> = OnceLock::new();
static DATASPACE_ACTIVITY: OnceLock<Mutex<Vec<DataspaceActivitySnapshot>>> = OnceLock::new();
static LANE_COMMITMENTS: OnceLock<Mutex<Vec<LaneCommitmentSnapshot>>> = OnceLock::new();
static DATASPACE_COMMITMENTS: OnceLock<Mutex<Vec<DataspaceCommitmentSnapshot>>> = OnceLock::new();
static LANE_SETTLEMENT_COMMITMENTS: OnceLock<Mutex<Vec<LaneBlockCommitment>>> = OnceLock::new();
static LANE_RELAY_ENVELOPES: OnceLock<Mutex<Vec<LaneRelayEnvelope>>> = OnceLock::new();
static LANE_PAYLOAD_OWNERSHIPS: OnceLock<Mutex<Vec<SumeragiLanePayloadOwnership>>> =
    OnceLock::new();
static COMMITTED_LANE_BLOCKS: OnceLock<Mutex<Vec<CommittedLaneBlockSnapshot>>> = OnceLock::new();
static LANE_BLOCK_SESSIONS: OnceLock<Mutex<Vec<SumeragiLaneBlockSessionStatus>>> = OnceLock::new();
static LANE_GOVERNANCE: OnceLock<Mutex<Vec<LaneGovernanceSnapshot>>> = OnceLock::new();
static NEXUS_FEE_STATUS: OnceLock<Mutex<NexusFeeSnapshot>> = OnceLock::new();
static NEXUS_STAKING_STATUS: OnceLock<Mutex<BTreeMap<LaneId, NexusStakingLaneSnapshot>>> =
    OnceLock::new();
static PIPELINE_CONFLICT_RATE_BPS: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_DEPTH: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_CAPACITY: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_RETAINED_BYTES: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_MAX_RETAINED_BYTES: AtomicU64 = AtomicU64::new(0);
static TX_QUEUE_SATURATED: AtomicBool = AtomicBool::new(false);
static TX_QUEUE_SATURATED_BY_COUNT: AtomicBool = AtomicBool::new(false);
static TX_QUEUE_SATURATED_BY_BYTES: AtomicBool = AtomicBool::new(false);
static TX_QUEUE_SATURATED_BY_AGE: AtomicBool = AtomicBool::new(false);
static TX_QUEUE_OLDEST_QUEUED_AGE_MS: AtomicU64 = AtomicU64::new(0);

const LANE_RELAY_ENVELOPES_CAP: usize = 64;
const LANE_PAYLOAD_OWNERSHIPS_CAP: usize = 128;
const COMMITTED_LANE_BLOCKS_CAP: usize = 128;

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
    /// Failures while executing the fee debit.
    pub transfer_failures_total: u64,
    /// Last attempted fee amount if available.
    pub last_amount: Option<Quantity>,
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
        /// Amount charged.
        amount: Quantity,
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
        /// Maximum allowed fee.
        max_fee: Quantity,
        /// Attempted fee.
        attempted_fee: Quantity,
    },
    /// Fee debit failed to apply.
    TransferFailed {
        /// Payer classification.
        payer_kind: NexusFeePayer,
        /// Account that attempted to pay.
        payer_id: String,
        /// Amount attempted.
        amount: Quantity,
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
    pub bonded: Quantity,
    /// Total pending-unbond stake recorded.
    pub pending_unbond: Quantity,
    /// Total slashes applied.
    pub slash_total: u64,
}

impl Default for NexusStakingLaneSnapshot {
    fn default() -> Self {
        Self {
            lane_id: LaneId::new(0),
            bonded: Quantity::zero(),
            pending_unbond: Quantity::zero(),
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
    #[cfg(test)]
    let _guard = local_removed_test_guard();
    LOCAL_REMOVED_FROM_WORLD.store(removed, Ordering::Relaxed);
}

/// Check if the local peer has been removed from the world state.
pub fn local_peer_removed() -> bool {
    #[cfg(test)]
    let _guard = local_removed_test_guard();
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
    let mut guard = lock_operator_status_slot(settlement_status_slot(), "settlement status");
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
    let mut guard = lock_operator_status_slot(settlement_status_slot(), "settlement status");
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
    let guard = lock_operator_status_slot(settlement_status_slot(), "settlement status");
    SettlementStatusSnapshot {
        dvp: guard.dvp.clone(),
        pvp: guard.pvp.clone(),
    }
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
    /// Transactions prepared for detached overlay execution.
    pub detached_prepared: u64,
    /// Detached transaction deltas merged without sequential fallback.
    pub detached_merged: u64,
    /// Detached transaction deltas that fell back to sequential execution.
    pub detached_fallback: u64,
    /// Sequential fallbacks caused by fee postprocessing requirements.
    pub detached_fallback_fee_postprocessing: u64,
    /// Sequential fallbacks caused by a user-provided executor.
    pub detached_fallback_user_executor: u64,
    /// Sequential fallbacks caused by durable smart-contract state changes.
    pub detached_fallback_durable_state: u64,
    /// Sequential fallbacks caused by unsupported detached instructions.
    pub detached_fallback_unsupported_instruction: u64,
    /// Sequential fallbacks caused by rejected detached evaluation.
    pub detached_fallback_rejected_eval: u64,
    /// Sequential fallbacks caused by overlay build errors.
    pub detached_fallback_overlay_error: u64,
    /// Quarantine transactions executed in the sequential quarantine lane.
    pub quarantine_executed: u64,
}

/// Aggregate execution summary for the latest block pipeline run.
#[derive(Clone, Copy, Debug, Default)]
pub struct PipelineExecutionSnapshot {
    /// Total transaction vertices across all lanes.
    pub tx_vertices_total: u64,
    /// Total conflict edges across all lanes.
    pub tx_edges_total: u64,
    /// Total overlay fragments executed across all lanes.
    pub overlay_count_total: u64,
    /// Total overlay instructions executed across all lanes.
    pub overlay_instr_total: u64,
    /// Total overlay bytes executed across all lanes.
    pub overlay_bytes_total: u64,
    /// Total RBC chunks attributed across all lanes.
    pub rbc_chunks_total: u64,
    /// Total RBC payload bytes attributed across all lanes.
    pub rbc_bytes_total: u64,
    /// Transactions prepared for detached overlay execution.
    pub detached_prepared_total: u64,
    /// Detached transaction deltas merged without sequential fallback.
    pub detached_merged_total: u64,
    /// Detached transaction deltas that fell back to sequential execution.
    pub detached_fallback_total: u64,
    /// Sequential fallbacks caused by fee postprocessing requirements.
    pub detached_fallback_fee_postprocessing_total: u64,
    /// Sequential fallbacks caused by a user-provided executor.
    pub detached_fallback_user_executor_total: u64,
    /// Sequential fallbacks caused by durable smart-contract state changes.
    pub detached_fallback_durable_state_total: u64,
    /// Sequential fallbacks caused by unsupported detached instructions.
    pub detached_fallback_unsupported_instruction_total: u64,
    /// Sequential fallbacks caused by rejected detached evaluation.
    pub detached_fallback_rejected_eval_total: u64,
    /// Sequential fallbacks caused by overlay build errors.
    pub detached_fallback_overlay_error_total: u64,
    /// Quarantine transactions executed in the sequential quarantine lane.
    pub quarantine_executed_total: u64,
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

/// Execution readiness for a certified standalone lane-local block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommittedLaneBlockExecutionStatus {
    /// The block has proposal/prepare/commit certificates, but no executable lane payload yet.
    AwaitingExecutablePayload,
    /// Accepted entrypoints are locally recoverable, but standalone execution is not wired yet.
    PayloadAvailableAwaitingExecutor,
    /// Accepted entrypoints have been durably recovered for standalone state application.
    PayloadRecoveredAwaitingStateApplication,
    /// Recovered entrypoints passed direct-execution preflight at the current local state tip.
    PayloadPreflightedAwaitingStateApplication,
    /// Recovered entrypoints produced at least one rejection during direct-execution preflight.
    PayloadPreflightRejectedAwaitingStateApplication,
    /// Canonical application receipt disagrees with durable direct-execution preflight results.
    ApplicationReceiptConflictsWithPreflight,
    /// This lane block cannot execute until its certified predecessor is applied.
    AwaitingPredecessorApplication,
    /// Accepted entrypoints already have canonical committed results recorded locally.
    StateAppliedByCanonicalBlock,
    /// Accepted entrypoints were directly applied to local WSV without a canonical block append.
    StateAppliedByDirectExecution,
}

impl CommittedLaneBlockExecutionStatus {
    /// Stable operator-facing label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AwaitingExecutablePayload => COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD,
            Self::PayloadAvailableAwaitingExecutor => {
                COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR
            }
            Self::PayloadRecoveredAwaitingStateApplication => {
                COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION
            }
            Self::PayloadPreflightedAwaitingStateApplication => {
                COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION
            }
            Self::PayloadPreflightRejectedAwaitingStateApplication => {
                COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION
            }
            Self::ApplicationReceiptConflictsWithPreflight => {
                COMMITTED_LANE_STATUS_APPLICATION_RECEIPT_CONFLICTS_WITH_PREFLIGHT
            }
            Self::AwaitingPredecessorApplication => {
                COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION
            }
            Self::StateAppliedByCanonicalBlock => {
                COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK
            }
            Self::StateAppliedByDirectExecution => {
                COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION
            }
        }
    }

    /// Whether the committed lane block can be handed to a standalone executor.
    #[must_use]
    pub const fn executable_payload_available(self) -> bool {
        match self {
            Self::AwaitingExecutablePayload => false,
            Self::PayloadAvailableAwaitingExecutor
            | Self::PayloadRecoveredAwaitingStateApplication
            | Self::PayloadPreflightedAwaitingStateApplication
            | Self::StateAppliedByCanonicalBlock
            | Self::StateAppliedByDirectExecution => true,
            Self::ApplicationReceiptConflictsWithPreflight
            | Self::PayloadPreflightRejectedAwaitingStateApplication
            | Self::AwaitingPredecessorApplication => false,
        }
    }
}

/// Standalone lane-local block that has proposal, prepare QC, and commit QC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommittedLaneBlockSnapshot {
    /// Lane whose local block is committed.
    pub lane_id: LaneId,
    /// Dataspace bound to the committed lane-local block.
    pub dataspace_id: DataSpaceId,
    /// Lane-local block height.
    pub lane_block_height: u64,
    /// Lane-local consensus view.
    pub lane_block_view: u64,
    /// Stable hash of the standalone lane block descriptor.
    pub descriptor_hash: Hash,
    /// Stable hash of the standalone lane block proposal.
    pub proposal_hash: Hash,
    /// Execution readiness of the certified standalone lane-local block.
    pub execution_status: CommittedLaneBlockExecutionStatus,
    /// Proposal artifact committed by the QCs.
    pub proposal: LaneBlockProposalV1,
    /// Prepare QC for the proposal.
    pub prepare_qc: LaneBlockQcV1,
    /// Commit QC for the proposal.
    pub commit_qc: LaneBlockQcV1,
}

impl CommittedLaneBlockSnapshot {
    /// Whether the committed lane block has enough payload material for execution.
    #[must_use]
    pub const fn executable_payload_available(&self) -> bool {
        self.execution_status.executable_payload_available()
    }
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

fn nexus_fee_slot() -> &'static Mutex<NexusFeeSnapshot> {
    NEXUS_FEE_STATUS.get_or_init(|| Mutex::new(NexusFeeSnapshot::default()))
}

fn nexus_staking_slot() -> &'static Mutex<BTreeMap<LaneId, NexusStakingLaneSnapshot>> {
    NEXUS_STAKING_STATUS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Record a Nexus fee debit outcome for later status/telemetry surfacing.
pub fn record_nexus_fee_event(event: NexusFeeEvent) {
    #[cfg(test)]
    let Some(_guard) = try_reentrant_test_guard(&RBC_STATUS_TEST_LOCK) else {
        return;
    };
    let mut guard = lock_operator_status_slot(nexus_fee_slot(), "nexus fee status");
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
    #[cfg(test)]
    let Some(_guard) = try_reentrant_test_guard(&RBC_STATUS_TEST_LOCK) else {
        return;
    };
    let mut guard = lock_operator_status_slot(nexus_staking_slot(), "nexus staking status");
    let entry = guard
        .entry(lane_id)
        .or_insert_with(|| NexusStakingLaneSnapshot {
            lane_id,
            ..NexusStakingLaneSnapshot::default()
        });
    update(entry);
}

fn adjust_quantity_value(current: Quantity, delta: &Quantity, increase: bool) -> Quantity {
    if delta.is_zero() {
        return current;
    }
    if increase {
        let base = current.clone();
        current.checked_add(delta).unwrap_or_else(|_| {
            iroha_logger::warn!(
                %base,
                %delta,
                "nexus staking accumulator overflowed; clamping to Quantity::zero()"
            );
            Quantity::zero()
        })
    } else {
        let base = current.clone();
        current.checked_sub(delta).unwrap_or_else(|_| {
            iroha_logger::warn!(
                %base,
                %delta,
                "nexus staking accumulator underflowed; clamping to Quantity::zero()"
            );
            Quantity::zero()
        })
    }
}

/// Record a bonded stake delta for a Nexus lane.
pub fn record_public_lane_bonded_delta(lane_id: LaneId, amount: &Quantity, increase: bool) {
    update_staking_lane(lane_id, |snapshot| {
        snapshot.bonded = adjust_quantity_value(snapshot.bonded.clone(), amount, increase);
    });
}

/// Record a pending-unbond delta for a Nexus lane.
pub fn record_public_lane_pending_unbond_delta(lane_id: LaneId, amount: &Quantity, increase: bool) {
    update_staking_lane(lane_id, |snapshot| {
        snapshot.pending_unbond =
            adjust_quantity_value(snapshot.pending_unbond.clone(), amount, increase);
    });
}

/// Record a slash event for a Nexus lane.
pub fn record_public_lane_slash(lane_id: LaneId) {
    update_staking_lane(lane_id, |snapshot| {
        snapshot.slash_total = snapshot.slash_total.saturating_add(1);
    });
}

/// Remove accumulated Nexus public-lane staking status for reset lanes.
pub fn reset_public_lane_staking_lanes(lanes_to_reset: &BTreeSet<LaneId>) {
    if lanes_to_reset.is_empty() {
        return;
    }
    #[cfg(test)]
    let Some(_guard) = try_reentrant_test_guard(&RBC_STATUS_TEST_LOCK) else {
        return;
    };

    let mut guard = lock_operator_status_slot(nexus_staking_slot(), "nexus staking status");
    for lane_id in lanes_to_reset {
        guard.remove(lane_id);
    }
}

/// Latest aggregated Nexus fee snapshot.
pub fn nexus_fee_snapshot() -> NexusFeeSnapshot {
    lock_operator_status_slot(nexus_fee_slot(), "nexus fee status").clone()
}

/// Latest aggregated Nexus staking snapshot.
pub fn nexus_staking_snapshot() -> NexusStakingSnapshot {
    let guard = lock_operator_status_slot(nexus_staking_slot(), "nexus staking status");
    let mut lanes: Vec<_> = guard.values().cloned().collect();
    lanes.sort_by_key(|lane| lane.lane_id.as_u32());
    NexusStakingSnapshot { lanes }
}

/// Shared lock for tests that mutate global Nexus fee state.
#[cfg(not(test))]
pub fn nexus_fee_test_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Shared lock for tests that mutate global Nexus fee state.
#[cfg(test)]
pub(crate) fn nexus_fee_test_lock() -> &'static NexusFeeTestLock {
    static LOCK: NexusFeeTestLock = NexusFeeTestLock;
    &LOCK
}

/// Clear Nexus economics snapshots (test-only helper).
pub fn reset_nexus_economics_for_tests() {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    {
        let mut guard = lock_operator_status_slot(nexus_fee_slot(), "nexus fee status");
        *guard = NexusFeeSnapshot::default();
    }
    {
        let mut guard = lock_operator_status_slot(nexus_staking_slot(), "nexus staking status");
        guard.clear();
    }
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
    /// Return a stable label for telemetry.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingHsm => "missing_hsm",
            Self::DisallowedAlgorithm => "disallowed_algorithm",
            Self::DisallowedProvider => "disallowed_provider",
            Self::LeadTimeViolation => "lead_time_violation",
            Self::ActivationInPast => "activation_in_past",
            Self::ExpiryBeforeActivation => "expiry_before_activation",
            Self::IdentifierCollision => "identifier_collision",
        }
    }
}

static PEER_KEY_POLICY_REJECT_TOTAL: AtomicU64 = AtomicU64::new(0);
static PEER_KEY_POLICY_LAST_REASON: OnceLock<Mutex<Option<&'static str>>> = OnceLock::new();

/// Record a peer consensus-key policy rejection.
pub fn record_peer_key_policy_reject(reason: PeerKeyPolicyRejectReason) {
    #[cfg(test)]
    let Some(_guard) = try_reentrant_test_guard(&PEER_KEY_POLICY_TEST_LOCK) else {
        return;
    };
    PEER_KEY_POLICY_REJECT_TOTAL.fetch_add(1, Ordering::Relaxed);
    *lock_operator_status_slot(
        PEER_KEY_POLICY_LAST_REASON.get_or_init(|| Mutex::new(None)),
        "peer key policy reason",
    ) = Some(reason.as_str());
}

/// Reset peer-key policy diagnostics in isolated tests.
#[cfg(test)]
pub(crate) fn reset_peer_key_policy_counters_for_tests() {
    let _guard = peer_key_policy_test_guard();
    PEER_KEY_POLICY_REJECT_TOTAL.store(0, Ordering::Relaxed);
    *lock_operator_status_slot(
        PEER_KEY_POLICY_LAST_REASON.get_or_init(|| Mutex::new(None)),
        "peer key policy reason",
    ) = None;
}

/// Read the compact peer-key rejection diagnostic in isolated unit tests.
#[cfg(test)]
pub(crate) fn peer_key_policy_reject_snapshot_for_tests() -> (u64, Option<&'static str>) {
    let total = PEER_KEY_POLICY_REJECT_TOTAL.load(Ordering::Relaxed);
    let last_reason = *lock_operator_status_slot(
        PEER_KEY_POLICY_LAST_REASON.get_or_init(|| Mutex::new(None)),
        "peer key policy reason",
    );
    (total, last_reason)
}

const KEY_LIFECYCLE_HISTORY_CAP: usize = 128;
static KEY_LIFECYCLE_HISTORY: OnceLock<Mutex<VecDeque<ConsensusKeyRecord>>> = OnceLock::new();

fn key_history_slot() -> &'static Mutex<VecDeque<ConsensusKeyRecord>> {
    KEY_LIFECYCLE_HISTORY.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn checkpoint_history_slot() -> &'static Mutex<VecDeque<ValidatorSetCheckpoint>> {
    VALIDATOR_CHECKPOINT_HISTORY.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn commit_cert_history_slot() -> &'static Mutex<VecDeque<Qc>> {
    COMMIT_CERT_HISTORY.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Record a validator checkpoint for retained archival query routes.
pub fn record_validator_checkpoint(checkpoint: ValidatorSetCheckpoint) {
    let mut history =
        lock_operator_status_slot(checkpoint_history_slot(), "validator checkpoint history");
    history.push_back(checkpoint);
    while history.len() > VALIDATOR_CHECKPOINT_HISTORY_CAP {
        history.pop_front();
    }
}

/// Return retained validator checkpoints newest first.
#[must_use]
pub fn validator_checkpoint_history() -> Vec<ValidatorSetCheckpoint> {
    lock_operator_status_slot(checkpoint_history_slot(), "validator checkpoint history")
        .iter()
        .rev()
        .cloned()
        .collect()
}

/// Record a legacy commit certificate for archival query and fixture consumers.
///
/// Protocol-v2 finality remains represented exclusively by its typed finality
/// artifact; this cache does not participate in v2 consensus decisions.
pub fn record_commit_qc(cert: Qc) {
    let mut history =
        lock_operator_status_slot(commit_cert_history_slot(), "commit certificate history");
    history.retain(|entry| {
        !(entry.height == cert.height
            && entry.subject_block_hash == cert.subject_block_hash
            && entry.view <= cert.view)
    });
    history.push_back(cert);
    while history.len() > COMMIT_CERT_HISTORY_CAP {
        history.pop_front();
    }
}

/// Return retained legacy commit certificates newest first.
#[must_use]
pub fn commit_qc_history() -> Vec<Qc> {
    let mut entries: Vec<_> =
        lock_operator_status_slot(commit_cert_history_slot(), "commit certificate history")
            .iter()
            .cloned()
            .collect();
    entries.sort_by(|left, right| {
        right
            .height
            .cmp(&left.height)
            .then_with(|| right.view.cmp(&left.view))
    });
    entries
}

/// Raw finality fixture hook for dependent-crate tests.
#[cfg(all(feature = "iroha-core-tests", feature = "finality-test-fixtures"))]
#[doc(hidden)]
pub fn record_commit_qc_for_tests(cert: Qc) {
    record_commit_qc(cert);
}

/// Record a consensus-key lifecycle entry for the remaining legacy Torii endpoint.
pub fn record_consensus_key(record: ConsensusKeyRecord) {
    let mut history = lock_operator_status_slot(key_history_slot(), "key lifecycle history");
    history.retain(|existing| existing.id != record.id);
    history.push_back(record);
    while history.len() > KEY_LIFECYCLE_HISTORY_CAP {
        history.pop_front();
    }
}

/// Return consensus-key lifecycle entries newest first.
#[must_use]
pub fn consensus_key_history() -> Vec<ConsensusKeyRecord> {
    lock_operator_status_slot(key_history_slot(), "key lifecycle history")
        .iter()
        .rev()
        .cloned()
        .collect()
}

/// Clear consensus-key lifecycle history in tests.
#[cfg(test)]
pub fn reset_consensus_keys_for_tests() {
    lock_operator_status_slot(key_history_slot(), "key lifecycle history").clear();
}

/// Clear validator checkpoint history in isolated tests.
#[cfg(test)]
pub fn reset_validator_checkpoints_for_tests() {
    lock_operator_status_slot(checkpoint_history_slot(), "validator checkpoint history").clear();
}

/// Clear legacy commit-certificate history in isolated tests.
#[cfg(test)]
pub fn reset_commit_certs_for_tests() {
    lock_operator_status_slot(commit_cert_history_slot(), "commit certificate history").clear();
}

static VRF_PENALTY_EPOCH: AtomicU64 = AtomicU64::new(0);
static VRF_NON_REVEAL_TOTAL: AtomicU64 = AtomicU64::new(0);
static VRF_NO_PARTICIPATION_TOTAL: AtomicU64 = AtomicU64::new(0);
static VRF_LATE_REVEALS_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Return the legacy VRF penalty counters still consumed by one Torii route.
#[must_use]
pub fn vrf_penalty_snapshot() -> (u64, u64, u64, u64) {
    (
        VRF_PENALTY_EPOCH.load(Ordering::Relaxed),
        VRF_NON_REVEAL_TOTAL.load(Ordering::Relaxed),
        VRF_NO_PARTICIPATION_TOTAL.load(Ordering::Relaxed),
        VRF_LATE_REVEALS_TOTAL.load(Ordering::Relaxed),
    )
}

/// Worker-loop queue identifiers used by the remaining async adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerQueueKind {
    /// Vote-related messages.
    Votes,
    /// Block payload messages.
    BlockPayload,
    /// Legacy lane-RBC chunk transport.
    RbcChunks,
    /// Fallback block/control messages.
    Blocks,
    /// Consensus control-flow messages.
    Consensus,
    /// Lane relay envelopes.
    LaneRelay,
    /// Background post requests.
    Background,
}

static WORKER_QUEUE_DEPTHS: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];
static WORKER_QUEUE_DROPS: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];

const fn worker_queue_index(kind: WorkerQueueKind) -> usize {
    match kind {
        WorkerQueueKind::Votes => 0,
        WorkerQueueKind::BlockPayload => 1,
        WorkerQueueKind::RbcChunks => 2,
        WorkerQueueKind::Blocks => 3,
        WorkerQueueKind::Consensus => 4,
        WorkerQueueKind::LaneRelay => 5,
        WorkerQueueKind::Background => 6,
    }
}

/// Record an enqueue for the given adapter queue.
pub fn record_worker_queue_enqueue(kind: WorkerQueueKind) {
    WORKER_QUEUE_DEPTHS[worker_queue_index(kind)].fetch_add(1, Ordering::Relaxed);
}

/// Record a dropped enqueue for the given adapter queue.
pub fn record_worker_queue_drop(kind: WorkerQueueKind) {
    WORKER_QUEUE_DROPS[worker_queue_index(kind)].fetch_add(1, Ordering::Relaxed);
}

static GOSSIP_DUPLICATE_KNOWN_SKIPPED_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Set the last observed propose-phase latency in milliseconds.
pub fn set_phase_propose_ms(ms: u64) {
    store_phase_ms(&LAST_PROPOSE_MS, &MAX_PROPOSE_MS, ms);
}

/// Set the last observed data-availability collection latency in milliseconds.
pub fn set_phase_collect_da_ms(ms: u64) {
    store_phase_ms(&LAST_COLLECT_DA_MS, &MAX_COLLECT_DA_MS, ms);
}

/// Set the last observed prevote collection latency in milliseconds.
pub fn set_phase_collect_prevote_ms(ms: u64) {
    store_phase_ms(&LAST_COLLECT_PREVOTE_MS, &MAX_COLLECT_PREVOTE_MS, ms);
}

/// Set the last observed precommit collection latency in milliseconds.
pub fn set_phase_collect_precommit_ms(ms: u64) {
    store_phase_ms(&LAST_COLLECT_PRECOMMIT_MS, &MAX_COLLECT_PRECOMMIT_MS, ms);
}

/// Set the last observed redundant collector fan-out latency in milliseconds.
pub fn set_phase_collect_aggregator_ms(ms: u64) {
    store_phase_ms(&LAST_COLLECT_AGG_MS, &MAX_COLLECT_AGG_MS, ms);
}

/// Set the last observed commit-phase latency in milliseconds.
pub fn set_phase_commit_ms(ms: u64) {
    store_phase_ms(&LAST_COMMIT_MS, &MAX_COMMIT_MS, ms);
}

fn store_phase_ms(latest: &AtomicU64, maximum: &AtomicU64, ms: u64) {
    latest.store(ms, Ordering::Relaxed);
    maximum.fetch_max(ms, Ordering::Relaxed);
}

/// Set the EMA propose-phase latency in milliseconds.
pub fn set_phase_propose_ema_ms(ms: u64) {
    LAST_PROPOSE_EMA_MS.store(ms, Ordering::Relaxed);
}

/// Set the EMA data-availability collection latency in milliseconds.
pub fn set_phase_collect_da_ema_ms(ms: u64) {
    LAST_COLLECT_DA_EMA_MS.store(ms, Ordering::Relaxed);
}

/// Set the EMA prevote collection latency in milliseconds.
pub fn set_phase_collect_prevote_ema_ms(ms: u64) {
    LAST_COLLECT_PREVOTE_EMA_MS.store(ms, Ordering::Relaxed);
}

/// Set the EMA precommit collection latency in milliseconds.
pub fn set_phase_collect_precommit_ema_ms(ms: u64) {
    LAST_COLLECT_PRECOMMIT_EMA_MS.store(ms, Ordering::Relaxed);
}

/// Set the EMA redundant collector fan-out latency in milliseconds.
pub fn set_phase_collect_aggregator_ema_ms(ms: u64) {
    LAST_COLLECT_AGG_EMA_MS.store(ms, Ordering::Relaxed);
}

/// Set the EMA commit-phase latency in milliseconds.
pub fn set_phase_commit_ema_ms(ms: u64) {
    LAST_COMMIT_EMA_MS.store(ms, Ordering::Relaxed);
}

/// Set the EMA aggregate pipeline latency in milliseconds.
pub fn set_phase_pipeline_total_ema_ms(ms: u64) {
    LAST_PIPELINE_TOTAL_EMA_MS.store(ms, Ordering::Relaxed);
}

/// Increment the collector-exhaustion gossip fallback counter.
pub fn inc_gossip_fallback() {
    GOSSIP_FALLBACK_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment the counter for block-created messages rejected by the lock gate.
pub fn inc_block_created_dropped_by_lock() {
    BLOCK_CREATED_DROPPED_BY_LOCK_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment the counter for block-created hint mismatches.
pub fn inc_block_created_hint_mismatch() {
    BLOCK_CREATED_HINT_MISMATCH_TOTAL.fetch_add(1, Ordering::Relaxed);
}

/// Increment the counter for block-created proposal mismatches.
pub fn inc_block_created_proposal_mismatch() {
    BLOCK_CREATED_PROPOSAL_MISMATCH_TOTAL.fetch_add(1, Ordering::Relaxed);
}

fn phase_pipeline_total(values: [u64; 5]) -> u64 {
    values.into_iter().fold(0_u64, u64::saturating_add)
}

/// Snapshot process-local per-phase latency diagnostics.
#[must_use]
pub fn phase_latencies_snapshot() -> PhaseLatenciesSnapshot {
    let propose_ms = LAST_PROPOSE_MS.load(Ordering::Relaxed);
    let collect_da_ms = LAST_COLLECT_DA_MS.load(Ordering::Relaxed);
    let collect_prevote_ms = LAST_COLLECT_PREVOTE_MS.load(Ordering::Relaxed);
    let collect_precommit_ms = LAST_COLLECT_PRECOMMIT_MS.load(Ordering::Relaxed);
    let collect_aggregator_ms = LAST_COLLECT_AGG_MS.load(Ordering::Relaxed);
    let commit_ms = LAST_COMMIT_MS.load(Ordering::Relaxed);
    let propose_max_ms = MAX_PROPOSE_MS.load(Ordering::Relaxed);
    let collect_da_max_ms = MAX_COLLECT_DA_MS.load(Ordering::Relaxed);
    let collect_prevote_max_ms = MAX_COLLECT_PREVOTE_MS.load(Ordering::Relaxed);
    let collect_precommit_max_ms = MAX_COLLECT_PRECOMMIT_MS.load(Ordering::Relaxed);
    let collect_aggregator_max_ms = MAX_COLLECT_AGG_MS.load(Ordering::Relaxed);
    let commit_max_ms = MAX_COMMIT_MS.load(Ordering::Relaxed);

    PhaseLatenciesSnapshot {
        propose_ms,
        collect_da_ms,
        collect_prevote_ms,
        collect_precommit_ms,
        collect_aggregator_ms,
        commit_ms,
        propose_max_ms,
        collect_da_max_ms,
        collect_prevote_max_ms,
        collect_precommit_max_ms,
        collect_aggregator_max_ms,
        commit_max_ms,
        propose_ema_ms: LAST_PROPOSE_EMA_MS.load(Ordering::Relaxed),
        collect_da_ema_ms: LAST_COLLECT_DA_EMA_MS.load(Ordering::Relaxed),
        collect_prevote_ema_ms: LAST_COLLECT_PREVOTE_EMA_MS.load(Ordering::Relaxed),
        collect_precommit_ema_ms: LAST_COLLECT_PRECOMMIT_EMA_MS.load(Ordering::Relaxed),
        collect_aggregator_ema_ms: LAST_COLLECT_AGG_EMA_MS.load(Ordering::Relaxed),
        commit_ema_ms: LAST_COMMIT_EMA_MS.load(Ordering::Relaxed),
        pipeline_total_ms: phase_pipeline_total([
            propose_ms,
            collect_da_ms,
            collect_prevote_ms,
            collect_precommit_ms,
            commit_ms,
        ]),
        pipeline_total_max_ms: phase_pipeline_total([
            propose_max_ms,
            collect_da_max_ms,
            collect_prevote_max_ms,
            collect_precommit_max_ms,
            commit_max_ms,
        ]),
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

/// Record an availability vote ingested by the local collector.
pub fn record_availability_vote(collector_idx: u64, peer: &PeerId) {
    let mut stats = lock_operator_status_slot(availability_slot(), "availability vote stats");
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

/// Snapshot process-local availability vote ingestion counters.
#[must_use]
pub fn availability_snapshot() -> AvailabilitySnapshot {
    let stats = lock_operator_status_slot(availability_slot(), "availability vote stats");
    let mut collectors: Vec<_> = stats
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

/// Record the last observed QC assembly latency for a stable kind label.
pub fn record_qc_latency(kind: &'static str, ms: u64) {
    lock_operator_status_slot(qc_latency_slot(), "QC latency stats").insert(kind, ms);
}

/// Snapshot QC assembly latencies sorted by kind label.
#[must_use]
pub fn qc_latency_snapshot() -> Vec<(String, u64)> {
    lock_operator_status_slot(qc_latency_slot(), "QC latency stats")
        .iter()
        .map(|(kind, ms)| ((*kind).to_owned(), *ms))
        .collect()
}

/// Replace the aggregated RBC backlog telemetry snapshot.
pub fn set_rbc_backlog_snapshot(
    total_missing_chunks: u64,
    max_missing_chunks: u64,
    pending_sessions: u64,
) {
    *lock_operator_status_slot(rbc_backlog_slot(), "RBC backlog snapshot") = RbcBacklogSnapshot {
        total_missing_chunks,
        max_missing_chunks,
        pending_sessions,
    };
}

/// Snapshot the aggregated RBC backlog telemetry.
#[must_use]
pub fn rbc_backlog_snapshot() -> RbcBacklogSnapshot {
    *lock_operator_status_slot(rbc_backlog_slot(), "RBC backlog snapshot")
}

/// Replace the pending-RBC compatibility snapshot.
pub fn set_pending_rbc_snapshot(snapshot: PendingRbcSnapshot) {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    *lock_operator_status_slot(pending_rbc_slot(), "pending RBC snapshot") = snapshot;
}

/// Snapshot pending-RBC compatibility diagnostics.
#[must_use]
pub fn pending_rbc_snapshot() -> PendingRbcSnapshot {
    #[cfg(test)]
    let _guard = rbc_status_test_guard();
    lock_operator_status_slot(pending_rbc_slot(), "pending RBC snapshot").clone()
}

#[cfg(test)]
mod telemetry_compatibility_tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{block::BlockHeader, peer::PeerId};

    use super::{PendingRbcEntrySnapshot, PendingRbcSnapshot, RbcBacklogSnapshot};

    #[test]
    fn phase_snapshot_tracks_current_max_ema_and_compatibility_counters() {
        let _guard = super::rbc_status_test_guard();
        super::reset_rbc_backlog_stats_for_tests();

        super::set_phase_propose_ms(10);
        super::set_phase_propose_ms(4);
        super::set_phase_collect_da_ms(2);
        super::set_phase_collect_prevote_ms(3);
        super::set_phase_collect_precommit_ms(4);
        super::set_phase_collect_aggregator_ms(50);
        super::set_phase_commit_ms(6);
        super::set_phase_propose_ema_ms(11);
        super::set_phase_collect_da_ema_ms(12);
        super::set_phase_collect_prevote_ema_ms(13);
        super::set_phase_collect_precommit_ema_ms(14);
        super::set_phase_collect_aggregator_ema_ms(15);
        super::set_phase_commit_ema_ms(16);
        super::set_phase_pipeline_total_ema_ms(17);
        super::inc_gossip_fallback();
        super::inc_block_created_dropped_by_lock();
        super::inc_block_created_hint_mismatch();
        super::inc_block_created_proposal_mismatch();

        let snapshot = super::phase_latencies_snapshot();
        assert_eq!(snapshot.propose_ms, 4);
        assert_eq!(snapshot.propose_max_ms, 10);
        assert_eq!(snapshot.collect_da_ms, 2);
        assert_eq!(snapshot.collect_prevote_ms, 3);
        assert_eq!(snapshot.collect_precommit_ms, 4);
        assert_eq!(snapshot.collect_aggregator_ms, 50);
        assert_eq!(snapshot.commit_ms, 6);
        assert_eq!(snapshot.pipeline_total_ms, 19);
        assert_eq!(snapshot.pipeline_total_max_ms, 25);
        assert_eq!(snapshot.propose_ema_ms, 11);
        assert_eq!(snapshot.collect_da_ema_ms, 12);
        assert_eq!(snapshot.collect_prevote_ema_ms, 13);
        assert_eq!(snapshot.collect_precommit_ema_ms, 14);
        assert_eq!(snapshot.collect_aggregator_ema_ms, 15);
        assert_eq!(snapshot.commit_ema_ms, 16);
        assert_eq!(snapshot.pipeline_total_ema_ms, 17);
        assert_eq!(snapshot.gossip_fallback_total, 1);
        assert_eq!(snapshot.block_created_dropped_by_lock_total, 1);
        assert_eq!(snapshot.block_created_hint_mismatch_total, 1);
        assert_eq!(snapshot.block_created_proposal_mismatch_total, 1);

        super::reset_rbc_backlog_stats_for_tests();
        assert_eq!(super::phase_latencies_snapshot(), Default::default());
    }

    #[test]
    fn phase_pipeline_totals_saturate() {
        let _guard = super::rbc_status_test_guard();
        super::reset_rbc_backlog_stats_for_tests();
        super::set_phase_propose_ms(u64::MAX);
        super::set_phase_collect_da_ms(1);

        let snapshot = super::phase_latencies_snapshot();
        assert_eq!(snapshot.pipeline_total_ms, u64::MAX);
        assert_eq!(snapshot.pipeline_total_max_ms, u64::MAX);

        super::reset_rbc_backlog_stats_for_tests();
    }

    #[test]
    fn collector_qc_and_rbc_snapshots_roundtrip_and_reset() {
        let _guard = super::rbc_status_test_guard();
        super::reset_rbc_backlog_stats_for_tests();
        let key_pair = KeyPair::try_from_seed(
            b"telemetry-compatibility-collector".to_vec(),
            Algorithm::BlsNormal,
        )
        .expect("derive collector fixture");
        let peer = PeerId::new(key_pair.public_key().clone());

        super::record_availability_vote(4, &peer);
        super::record_availability_vote(5, &peer);
        let availability = super::availability_snapshot();
        assert_eq!(availability.total, 2);
        assert_eq!(availability.collectors.len(), 1);
        assert_eq!(availability.collectors[0].collector_idx, 5);
        assert_eq!(availability.collectors[0].peer, peer);
        assert_eq!(availability.collectors[0].votes_ingested, 2);

        super::record_qc_latency("precommit", 30);
        super::record_qc_latency("availability", 10);
        super::record_qc_latency("availability", 20);
        assert_eq!(
            super::qc_latency_snapshot(),
            vec![
                ("availability".to_owned(), 20),
                ("precommit".to_owned(), 30)
            ]
        );

        super::set_rbc_backlog_snapshot(9, 4, 2);
        assert_eq!(
            super::rbc_backlog_snapshot(),
            RbcBacklogSnapshot {
                total_missing_chunks: 9,
                max_missing_chunks: 4,
                pending_sessions: 2,
            }
        );

        let pending = PendingRbcSnapshot {
            sessions: 1,
            session_cap: 8,
            chunks: 3,
            bytes: 512,
            drops_total: 2,
            entries: vec![PendingRbcEntrySnapshot {
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"pending-rbc",
                )),
                height: 7,
                view: 2,
                chunks: 3,
                bytes: 512,
                ..PendingRbcEntrySnapshot::default()
            }],
            ..PendingRbcSnapshot::default()
        };
        super::set_pending_rbc_snapshot(pending.clone());
        assert_eq!(super::pending_rbc_snapshot(), pending);

        super::reset_rbc_backlog_stats_for_tests();
        assert_eq!(super::availability_snapshot(), Default::default());
        assert!(super::qc_latency_snapshot().is_empty());
        assert_eq!(super::rbc_backlog_snapshot(), Default::default());
        assert_eq!(super::pending_rbc_snapshot(), Default::default());
    }
}

/// Count a duplicate transaction skipped by gossip.
pub fn inc_gossip_duplicate_known_skipped() {
    GOSSIP_DUPLICATE_KNOWN_SKIPPED_TOTAL.fetch_add(1, Ordering::Relaxed);
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

fn pipeline_execution_slot() -> &'static Mutex<PipelineExecutionSnapshot> {
    PIPELINE_EXECUTION.get_or_init(|| Mutex::new(PipelineExecutionSnapshot::default()))
}

/// Replace the lane-activity adapter diagnostic.
pub fn set_lane_activity_snapshot(entries: Vec<LaneActivitySnapshot>) {
    *lock_operator_status_slot(lane_activity_slot(), "lane activity snapshot") = entries;
}

/// Replace the aggregate pipeline-execution adapter diagnostic.
pub fn set_pipeline_execution_snapshot(snapshot: PipelineExecutionSnapshot) {
    #[cfg(test)]
    let Some(_guard) = try_reentrant_test_guard(&RBC_STATUS_TEST_LOCK) else {
        return;
    };
    *lock_operator_status_slot(pipeline_execution_slot(), "pipeline execution snapshot") = snapshot;
}

/// Test-only wrapper that reads only the pipeline adapter diagnostic without
/// cloning the rest of the public non-consensus status snapshot.
#[cfg(test)]
pub(crate) struct PipelineExecutionTestSnapshot {
    /// Aggregate adapter counters asserted by block-pipeline tests.
    pub(crate) pipeline_execution: PipelineExecutionSnapshot,
}

/// Read the aggregate pipeline-execution diagnostic in isolated unit tests.
#[cfg(test)]
pub(crate) fn pipeline_execution_snapshot_for_tests() -> PipelineExecutionTestSnapshot {
    PipelineExecutionTestSnapshot {
        pipeline_execution: lock_operator_status_slot(
            pipeline_execution_slot(),
            "pipeline execution snapshot",
        )
        .clone(),
    }
}

/// Replace the access-set source adapter diagnostic.
pub fn set_access_set_source_summary(summary: AccessSetSourceSummary) {
    *lock_operator_status_slot(access_set_source_slot(), "access-set source snapshot") = summary;
}

/// Record the latest conflict rate (basis points) for the pipeline DAG.
pub fn set_pipeline_conflict_rate_bps(bps: u64) {
    PIPELINE_CONFLICT_RATE_BPS.store(bps, Ordering::Relaxed);
}

/// Replace the dataspace-activity adapter diagnostic.
pub fn set_dataspace_activity_snapshot(entries: Vec<DataspaceActivitySnapshot>) {
    *lock_operator_status_slot(dataspace_activity_slot(), "dataspace activity snapshot") = entries;
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

fn lane_payload_ownerships_slot() -> &'static Mutex<Vec<SumeragiLanePayloadOwnership>> {
    LANE_PAYLOAD_OWNERSHIPS.get_or_init(|| Mutex::new(Vec::new()))
}

fn committed_lane_blocks_slot() -> &'static Mutex<Vec<CommittedLaneBlockSnapshot>> {
    COMMITTED_LANE_BLOCKS.get_or_init(|| Mutex::new(Vec::new()))
}

fn lane_block_sessions_slot() -> &'static Mutex<Vec<SumeragiLaneBlockSessionStatus>> {
    LANE_BLOCK_SESSIONS.get_or_init(|| Mutex::new(Vec::new()))
}

type LaneRelayKey = (
    iroha_data_model::nexus::LaneId,
    iroha_data_model::nexus::DataSpaceId,
    u64,
    HashOf<BlockHeader>,
    Option<HashOf<DaCommitmentBundle>>,
    Option<Hash>,
    HashOf<LaneBlockCommitment>,
    u64,
    Option<[u8; 32]>,
);

fn lane_relay_key(envelope: &LaneRelayEnvelope) -> LaneRelayKey {
    (
        envelope.lane_id,
        envelope.dataspace_id,
        envelope.block_height,
        envelope.block_header.hash(),
        envelope.da_commitment_hash,
        envelope.lane_block_descriptor_hash,
        envelope.settlement_hash,
        envelope.rbc_bytes_total,
        envelope.manifest_root,
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
    match envelope.verify().and_then(|()| {
        if envelope.fastpq_proof.is_some() {
            envelope.verify_fastpq_proof_material()
        } else {
            Ok(())
        }
    }) {
        Ok(()) => {}
        Err(err) => {
            record_relay_error(&err);
            iroha_logger::warn!(
                lane_id = %envelope.lane_id,
                dataspace_id = %envelope.dataspace_id,
                block_height = envelope.block_height,
                error_kind = err.as_label(),
                error = %err,
                "dropping lane relay envelope with failed structural verification"
            );
            return;
        }
    }

    let key = lane_relay_key(&envelope);
    if let Some(existing) = storage
        .iter()
        .position(|candidate| lane_relay_key(candidate) == key)
    {
        if storage[existing].is_merge_admissible() && !envelope.is_merge_admissible() {
            return;
        }
        storage[existing] = envelope;
    } else {
        storage.push(envelope);
        if storage.len() > LANE_RELAY_ENVELOPES_CAP {
            let drain = storage.len() - LANE_RELAY_ENVELOPES_CAP;
            storage.drain(0..drain);
        }
    }
}

/// Replace the aggregated lane/dataspace commitment snapshots used by Nexus diagnostics.
pub fn set_lane_commitments(
    lane_entries: Vec<LaneCommitmentSnapshot>,
    dataspace_entries: Vec<DataspaceCommitmentSnapshot>,
) {
    {
        let mut guard =
            lock_operator_status_slot(lane_commitments_slot(), "lane commitments snapshot");
        *guard = lane_entries;
    }
    {
        let mut guard = lock_operator_status_slot(
            dataspace_commitments_slot(),
            "dataspace commitments snapshot",
        );
        *guard = dataspace_entries;
    }
}

/// Replace the aggregated lane settlement commitments used by Nexus diagnostics.
pub fn set_lane_settlement_commitments(entries: Vec<LaneBlockCommitment>) {
    let mut guard = lock_operator_status_slot(
        lane_settlement_commitments_slot(),
        "lane settlement commitments snapshot",
    );
    *guard = entries;
}

/// Replace the stored lane relay envelopes captured during block sealing.
pub fn set_lane_relay_envelopes(entries: Vec<LaneRelayEnvelope>) {
    let mut guard =
        lock_operator_status_slot(lane_relay_envelopes_slot(), "lane relay envelopes snapshot");
    guard.clear();
    for envelope in entries {
        upsert_lane_relay_envelope(&mut guard, envelope);
    }
}

/// Append a single validated lane relay envelope to the cached snapshot.
pub fn push_lane_relay_envelope(envelope: LaneRelayEnvelope) {
    let mut guard =
        lock_operator_status_slot(lane_relay_envelopes_slot(), "lane relay envelopes snapshot");
    upsert_lane_relay_envelope(&mut guard, envelope);
}

/// Update the planned lane-local DA ownership identities used by Nexus diagnostics.
///
/// Updates are merged by `(lane_id, dataspace_id)` so a proposal for one lane
/// does not erase the latest ownership evidence for another active lane. Empty
/// updates are no-ops; use [`clear_lane_payload_ownerships`] for deliberate
/// test/shutdown cleanup.
pub fn set_lane_payload_ownerships(mut entries: Vec<SumeragiLanePayloadOwnership>) {
    entries.retain(|entry| match entry.validate_replay_material() {
        Ok(()) => true,
        Err(err) => {
            iroha_logger::warn!(
                lane_id = %entry.lane_id,
                dataspace_id = %entry.dataspace_id,
                lane_block_height = entry.lane_block_height,
                lane_block_view = entry.lane_block_view,
                error = %err,
                "dropping lane payload ownership status with invalid replay material"
            );
            false
        }
    });
    let mut guard = lock_operator_status_slot(
        lane_payload_ownerships_slot(),
        "lane payload ownership snapshot",
    );
    if entries.is_empty() {
        return;
    }
    for entry in entries {
        upsert_lane_payload_ownership(&mut guard, entry);
    }
    if guard.len() > LANE_PAYLOAD_OWNERSHIPS_CAP {
        guard.sort_by_key(lane_payload_ownership_retention_key);
        let drain = guard.len() - LANE_PAYLOAD_OWNERSHIPS_CAP;
        guard.drain(0..drain);
    }
}

/// Clear all cached lane-local DA/RBC ownership identities.
pub fn clear_lane_payload_ownerships() {
    let mut guard = lock_operator_status_slot(
        lane_payload_ownerships_slot(),
        "lane payload ownership snapshot",
    );
    guard.clear();
}

fn upsert_lane_payload_ownership(
    entries: &mut Vec<SumeragiLanePayloadOwnership>,
    entry: SumeragiLanePayloadOwnership,
) {
    if let Some(existing) = entries.iter_mut().find(|existing| {
        existing.lane_id == entry.lane_id && existing.dataspace_id == entry.dataspace_id
    }) {
        if lane_payload_ownership_retention_key(&entry)
            >= lane_payload_ownership_retention_key(existing)
        {
            *existing = entry;
        }
        return;
    }
    entries.push(entry);
}

fn lane_payload_ownership_retention_key(
    entry: &SumeragiLanePayloadOwnership,
) -> (u64, u64, u64, u64, u32, u64) {
    (
        entry.lane_block_height,
        entry.lane_block_view,
        entry.proposal_height,
        entry.proposal_view,
        entry.lane_id.as_u32(),
        entry.dataspace_id.as_u64(),
    )
}

fn validate_committed_lane_block_snapshot(
    entry: &CommittedLaneBlockSnapshot,
) -> Result<(), String> {
    let descriptor = &entry.proposal.descriptor;
    if entry.lane_id != descriptor.lane_id
        || entry.dataspace_id != descriptor.dataspace_id
        || entry.lane_block_height != descriptor.lane_block_height
        || entry.lane_block_view != descriptor.lane_block_view
        || entry.descriptor_hash != descriptor.descriptor_hash
        || entry.proposal_hash != entry.proposal.proposal_hash
    {
        return Err("summary fields do not match embedded lane-block proposal".to_owned());
    }

    let session = crate::lane_consensus::CommittedLaneBlockSession {
        proposal: entry.proposal.clone(),
        prepare_qc: entry.prepare_qc.clone(),
        commit_qc: entry.commit_qc.clone(),
    };
    crate::lane_consensus::validate_committed_lane_block_session(&session)
        .map_err(|err| err.to_string())
}

/// Replace the committed standalone lane-block snapshot used by Nexus diagnostics.
pub fn set_committed_lane_blocks(mut entries: Vec<CommittedLaneBlockSnapshot>) {
    entries.retain(
        |entry| match validate_committed_lane_block_snapshot(entry) {
            Ok(()) => true,
            Err(err) => {
                iroha_logger::warn!(
                    lane_id = %entry.lane_id,
                    dataspace_id = %entry.dataspace_id,
                    lane_block_height = entry.lane_block_height,
                    lane_block_view = entry.lane_block_view,
                    error = %err,
                    "dropping committed lane block status with invalid certified identity"
                );
                false
            }
        },
    );
    if entries.len() > COMMITTED_LANE_BLOCKS_CAP {
        let drain = entries.len() - COMMITTED_LANE_BLOCKS_CAP;
        entries.drain(0..drain);
    }
    let mut guard = lock_operator_status_slot(
        committed_lane_blocks_slot(),
        "committed lane block snapshot",
    );
    *guard = entries;
}

/// Remove lane-scoped operator status snapshots for lanes whose runtime state was reset.
pub fn prune_lane_scoped_snapshots(lanes_to_reset: &BTreeSet<LaneId>) {
    if lanes_to_reset.is_empty() {
        return;
    }
    let lane_matches = |lane_id: u32| lanes_to_reset.contains(&LaneId::new(lane_id));

    lock_operator_status_slot(lane_activity_slot(), "lane activity snapshot")
        .retain(|entry| !lane_matches(entry.lane_id));
    lock_operator_status_slot(dataspace_activity_slot(), "dataspace activity snapshot")
        .retain(|entry| !lane_matches(entry.lane_id));
    lock_operator_status_slot(lane_commitments_slot(), "lane commitments snapshot")
        .retain(|entry| !lane_matches(entry.lane_id));
    lock_operator_status_slot(
        dataspace_commitments_slot(),
        "dataspace commitments snapshot",
    )
    .retain(|entry| !lane_matches(entry.lane_id));
    lock_operator_status_slot(
        lane_settlement_commitments_slot(),
        "lane settlement commitments snapshot",
    )
    .retain(|entry| !lanes_to_reset.contains(&entry.lane_id));
    lock_operator_status_slot(lane_relay_envelopes_slot(), "lane relay envelopes snapshot")
        .retain(|entry| !lanes_to_reset.contains(&entry.lane_id));
    lock_operator_status_slot(
        lane_payload_ownerships_slot(),
        "lane payload ownership snapshot",
    )
    .retain(|entry| !lanes_to_reset.contains(&entry.lane_id));
    lock_operator_status_slot(
        committed_lane_blocks_slot(),
        "committed lane block snapshot",
    )
    .retain(|entry| !lanes_to_reset.contains(&entry.lane_id));
    lock_operator_status_slot(lane_block_sessions_slot(), "lane block sessions snapshot")
        .retain(|entry| !lanes_to_reset.contains(&entry.lane_id));
    lock_operator_status_slot(lane_governance_slot(), "lane governance snapshot")
        .retain(|entry| !lane_matches(entry.lane_id));
}

#[cfg(test)]
pub(crate) fn lane_scoped_status_fingerprint_for_tests() -> String {
    format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
        lock_operator_status_slot(lane_activity_slot(), "lane activity snapshot"),
        lock_operator_status_slot(dataspace_activity_slot(), "dataspace activity snapshot"),
        lock_operator_status_slot(lane_commitments_slot(), "lane commitments snapshot"),
        lock_operator_status_slot(
            dataspace_commitments_slot(),
            "dataspace commitments snapshot"
        ),
        lock_operator_status_slot(
            lane_settlement_commitments_slot(),
            "lane settlement commitments snapshot"
        ),
        lock_operator_status_slot(lane_relay_envelopes_slot(), "lane relay envelopes snapshot"),
        lock_operator_status_slot(
            lane_payload_ownerships_slot(),
            "lane payload ownership snapshot"
        ),
        lock_operator_status_slot(
            committed_lane_blocks_slot(),
            "committed lane block snapshot"
        ),
        lock_operator_status_slot(lane_block_sessions_slot(), "lane block sessions snapshot"),
        lock_operator_status_slot(lane_governance_slot(), "lane governance snapshot"),
        lock_operator_status_slot(nexus_staking_slot(), "nexus staking status"),
        lock_operator_status_slot(nexus_fee_slot(), "nexus fee status"),
    )
}

fn lane_commitments_snapshot() -> Vec<LaneCommitmentSnapshot> {
    lock_operator_status_slot(lane_commitments_slot(), "lane commitments snapshot").clone()
}

fn dataspace_commitments_snapshot() -> Vec<DataspaceCommitmentSnapshot> {
    lock_operator_status_slot(
        dataspace_commitments_slot(),
        "dataspace commitments snapshot",
    )
    .clone()
}

fn lane_settlement_commitments_snapshot() -> Vec<LaneBlockCommitment> {
    lock_operator_status_slot(
        lane_settlement_commitments_slot(),
        "lane settlement commitments snapshot",
    )
    .clone()
}

/// Return the cached lane relay envelopes used by Nexus diagnostics.
pub fn lane_relay_envelopes_snapshot() -> Vec<LaneRelayEnvelope> {
    lock_operator_status_slot(lane_relay_envelopes_slot(), "lane relay envelopes snapshot").clone()
}

/// Return the cached lane-local DA ownership snapshot used by Nexus diagnostics.
pub fn lane_payload_ownerships_snapshot() -> Vec<SumeragiLanePayloadOwnership> {
    lock_operator_status_slot(
        lane_payload_ownerships_slot(),
        "lane payload ownership snapshot",
    )
    .clone()
}

/// Return the cached standalone committed lane-block snapshot used by Nexus diagnostics.
pub fn committed_lane_blocks_snapshot() -> Vec<CommittedLaneBlockSnapshot> {
    lock_operator_status_slot(
        committed_lane_blocks_slot(),
        "committed lane block snapshot",
    )
    .clone()
}

/// Replace the cached standalone lane-block session snapshot used by Nexus diagnostics.
pub fn set_lane_block_sessions(entries: Vec<SumeragiLaneBlockSessionStatus>) {
    *lock_operator_status_slot(lane_block_sessions_slot(), "lane block sessions snapshot") =
        entries;
}

/// Return the cached standalone lane-block session snapshot used by Nexus diagnostics.
pub fn lane_block_sessions_snapshot() -> Vec<SumeragiLaneBlockSessionStatus> {
    lock_operator_status_slot(lane_block_sessions_slot(), "lane block sessions snapshot").clone()
}

fn lane_governance_slot() -> &'static Mutex<Vec<LaneGovernanceSnapshot>> {
    LANE_GOVERNANCE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Replace the governance manifest snapshot used by Nexus diagnostics.
pub fn set_lane_governance_snapshot(entries: Vec<LaneGovernanceSnapshot>) {
    *lock_operator_status_slot(lane_governance_slot(), "lane governance snapshot") = entries;
}

/// Return the cached governance manifest snapshot used by Nexus diagnostics.
pub fn lane_governance_snapshot() -> Vec<LaneGovernanceSnapshot> {
    lock_operator_status_slot(lane_governance_slot(), "lane governance snapshot").clone()
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

/// Lane-local Nexus diagnostics kept separate from global v2 consensus status.
#[derive(Clone, Debug, Default)]
pub struct StatusSnapshot {
    /// Aggregate block-pipeline execution diagnostics; this is adapter state,
    /// not a global consensus phase or recovery signal.
    pub pipeline_execution: PipelineExecutionSnapshot,
    /// Lane-local block commitments retained for Nexus diagnostics.
    pub lane_commitments: Vec<LaneCommitmentSnapshot>,
    /// Dataspace-local commitments retained for Nexus diagnostics.
    pub dataspace_commitments: Vec<DataspaceCommitmentSnapshot>,
    /// Lane-local settlement commitments.
    pub lane_settlement_commitments: Vec<LaneBlockCommitment>,
    /// Certified lane relay envelopes.
    pub lane_relay_envelopes: Vec<LaneRelayEnvelope>,
    /// Lane-local payload ownership commitments.
    pub lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
    /// Standalone committed lane-block state.
    pub committed_lane_blocks: Vec<CommittedLaneBlockSnapshot>,
    /// Lane-local consensus sessions.
    pub lane_block_sessions: Vec<SumeragiLaneBlockSessionStatus>,
    /// Count of governance-sealed lanes.
    pub lane_governance_sealed_total: u32,
    /// Aliases of governance-sealed lanes.
    pub lane_governance_sealed_aliases: Vec<String>,
    /// Lane governance readiness.
    pub lane_governance: Vec<LaneGovernanceSnapshot>,
}

fn lane_governance_sealed_summary() -> (u32, Vec<String>, Vec<LaneGovernanceSnapshot>) {
    let lane_governance = lane_governance_snapshot();
    let aliases: Vec<_> = lane_governance
        .iter()
        .filter(|entry| entry.manifest_required && !entry.manifest_ready)
        .map(|entry| entry.alias.clone())
        .collect();
    let total = u32::try_from(aliases.len()).unwrap_or(u32::MAX);
    (total, aliases, lane_governance)
}

/// Snapshot non-consensus Nexus lane diagnostics.
#[must_use]
pub fn snapshot() -> StatusSnapshot {
    let (lane_governance_sealed_total, lane_governance_sealed_aliases, lane_governance) =
        lane_governance_sealed_summary();
    StatusSnapshot {
        pipeline_execution: lock_operator_status_slot(
            pipeline_execution_slot(),
            "pipeline execution snapshot",
        )
        .clone(),
        lane_commitments: lane_commitments_snapshot(),
        dataspace_commitments: dataspace_commitments_snapshot(),
        lane_settlement_commitments: lane_settlement_commitments_snapshot(),
        lane_relay_envelopes: lane_relay_envelopes_snapshot(),
        lane_payload_ownerships: lane_payload_ownerships_snapshot(),
        committed_lane_blocks: committed_lane_blocks_snapshot(),
        lane_block_sessions: lane_block_sessions_snapshot(),
        lane_governance_sealed_total,
        lane_governance_sealed_aliases,
        lane_governance,
    }
}

/// Latest transaction-queue pressure published for operator queries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TxQueueBackpressureSnapshot {
    /// Number of transactions waiting in the local queue.
    pub depth: u64,
    /// Configured transaction queue capacity.
    pub capacity: u64,
    /// Estimated retained transaction queue bytes.
    pub retained_bytes: u64,
    /// Configured retained transaction queue byte budget.
    pub max_retained_bytes: u64,
    /// Whether the queue reached capacity. This mirrors the public `saturated` field.
    pub saturated: bool,
    /// Whether the queue reached capacity.
    pub saturated_by_count: bool,
    /// Whether the queue exhausted its retained-byte budget.
    pub saturated_by_bytes: bool,
    /// Whether the oldest queued transaction exceeded the latency budget.
    pub saturated_by_age: bool,
    /// Age in milliseconds of the oldest queued transaction.
    pub oldest_queued_age_ms: u64,
}

/// Record the latest transaction-queue pressure snapshot for operator queries.
pub fn set_tx_queue_pressure(snapshot: QueuePressureSnapshot) {
    let saturated_by_count = snapshot.saturated_by_count;
    let saturated_by_bytes = snapshot.saturated_by_bytes;
    let saturated = saturated_by_count || saturated_by_bytes;
    TX_QUEUE_DEPTH.store(snapshot.queued_tx_count as u64, Ordering::Relaxed);
    TX_QUEUE_CAPACITY.store(snapshot.capacity.get() as u64, Ordering::Relaxed);
    TX_QUEUE_RETAINED_BYTES.store(snapshot.retained_bytes, Ordering::Relaxed);
    TX_QUEUE_MAX_RETAINED_BYTES.store(snapshot.max_retained_bytes.get(), Ordering::Relaxed);
    TX_QUEUE_SATURATED.store(saturated, Ordering::Relaxed);
    TX_QUEUE_SATURATED_BY_COUNT.store(saturated_by_count, Ordering::Relaxed);
    TX_QUEUE_SATURATED_BY_BYTES.store(saturated_by_bytes, Ordering::Relaxed);
    TX_QUEUE_SATURATED_BY_AGE.store(snapshot.saturated_by_age, Ordering::Relaxed);
    TX_QUEUE_OLDEST_QUEUED_AGE_MS.store(snapshot.oldest_queued_tx_age_ms, Ordering::Relaxed);
}

/// Record the latest transaction-queue backpressure snapshot for operator queries.
pub fn set_tx_queue_backpressure(state: BackpressureState) {
    match state {
        BackpressureState::Healthy { queued, capacity } => {
            TX_QUEUE_DEPTH.store(queued as u64, Ordering::Relaxed);
            TX_QUEUE_CAPACITY.store(capacity.get() as u64, Ordering::Relaxed);
            TX_QUEUE_RETAINED_BYTES.store(0, Ordering::Relaxed);
            TX_QUEUE_MAX_RETAINED_BYTES.store(0, Ordering::Relaxed);
            TX_QUEUE_SATURATED.store(false, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_COUNT.store(false, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_BYTES.store(false, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_AGE.store(false, Ordering::Relaxed);
            TX_QUEUE_OLDEST_QUEUED_AGE_MS.store(0, Ordering::Relaxed);
        }
        BackpressureState::Saturated { queued, capacity } => {
            TX_QUEUE_DEPTH.store(queued as u64, Ordering::Relaxed);
            TX_QUEUE_CAPACITY.store(capacity.get() as u64, Ordering::Relaxed);
            TX_QUEUE_RETAINED_BYTES.store(0, Ordering::Relaxed);
            TX_QUEUE_MAX_RETAINED_BYTES.store(0, Ordering::Relaxed);
            TX_QUEUE_SATURATED.store(true, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_COUNT.store(true, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_BYTES.store(false, Ordering::Relaxed);
            TX_QUEUE_SATURATED_BY_AGE.store(false, Ordering::Relaxed);
            TX_QUEUE_OLDEST_QUEUED_AGE_MS.store(0, Ordering::Relaxed);
        }
    }
}

/// Snapshot the recorded transaction-queue backpressure state.
pub fn tx_queue_backpressure() -> TxQueueBackpressureSnapshot {
    TxQueueBackpressureSnapshot {
        depth: TX_QUEUE_DEPTH.load(Ordering::Relaxed),
        capacity: TX_QUEUE_CAPACITY.load(Ordering::Relaxed),
        retained_bytes: TX_QUEUE_RETAINED_BYTES.load(Ordering::Relaxed),
        max_retained_bytes: TX_QUEUE_MAX_RETAINED_BYTES.load(Ordering::Relaxed),
        saturated: TX_QUEUE_SATURATED.load(Ordering::Relaxed),
        saturated_by_count: TX_QUEUE_SATURATED_BY_COUNT.load(Ordering::Relaxed),
        saturated_by_bytes: TX_QUEUE_SATURATED_BY_BYTES.load(Ordering::Relaxed),
        saturated_by_age: TX_QUEUE_SATURATED_BY_AGE.load(Ordering::Relaxed),
        oldest_queued_age_ms: TX_QUEUE_OLDEST_QUEUED_AGE_MS.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestLockOwner {
    Task(tokio::task::Id),
    Thread(std::thread::ThreadId),
}

#[cfg(test)]
thread_local! {
    static TEST_LOCK_OWNER_OVERRIDE: std::cell::Cell<Option<TestLockOwner>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
impl TestLockOwner {
    fn current() -> Self {
        if let Some(owner) = TEST_LOCK_OWNER_OVERRIDE.with(std::cell::Cell::get) {
            return owner;
        }
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
static STATUS_TEST_GLOBAL_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static RBC_STATUS_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static COMMIT_HISTORY_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static MODE_TAGS_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static PEER_KEY_POLICY_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static LOCAL_REMOVED_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static LANE_RELAY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
fn canonical_test_lock(_: &'static OnceLock<TestLock>) -> &'static TestLock {
    STATUS_TEST_GLOBAL_LOCK.get_or_init(TestLock::default)
}

#[cfg(test)]
fn reentrant_test_guard(lock: &'static OnceLock<TestLock>) -> TestLockGuard {
    let owner = TestLockOwner::current();
    let lock = canonical_test_lock(lock);
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
            Some(_) => {
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
fn try_reentrant_test_guard(lock: &'static OnceLock<TestLock>) -> Option<TestLockGuard> {
    let owner = TestLockOwner::current();
    let lock = canonical_test_lock(lock);
    let mut state = lock
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match state.owner {
        None => {
            state.owner = Some(owner);
            state.depth = 1;
            Some(TestLockGuard { lock, owner })
        }
        Some(current) if current == owner => {
            state.depth = state.depth.saturating_add(1);
            Some(TestLockGuard { lock, owner })
        }
        Some(_) => None,
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct NexusFeeTestLock;

#[cfg(test)]
pub(crate) struct NexusFeeTestGuard(TestLockGuard);

#[cfg(test)]
impl NexusFeeTestLock {
    pub(crate) fn lock(&'static self) -> Result<NexusFeeTestGuard, std::convert::Infallible> {
        Ok(NexusFeeTestGuard(reentrant_test_guard(
            &RBC_STATUS_TEST_LOCK,
        )))
    }
}

#[cfg(test)]
pub(crate) fn rbc_status_test_guard() -> TestLockGuard {
    reentrant_test_guard(&RBC_STATUS_TEST_LOCK)
}

#[cfg(test)]
/// Serialize tests that mutate archival commit history.
pub(crate) fn commit_history_test_guard() -> TestLockGuard {
    reentrant_test_guard(&COMMIT_HISTORY_TEST_LOCK)
}

#[cfg(test)]
/// Serialize tests that mutate archival mode tags.
pub(crate) fn mode_tags_test_guard() -> TestLockGuard {
    reentrant_test_guard(&MODE_TAGS_TEST_LOCK)
}

#[cfg(test)]
pub(crate) fn peer_key_policy_test_guard() -> TestLockGuard {
    reentrant_test_guard(&PEER_KEY_POLICY_TEST_LOCK)
}

#[cfg(test)]
pub(crate) fn local_removed_test_guard() -> TestLockGuard {
    reentrant_test_guard(&LOCAL_REMOVED_TEST_LOCK)
}

#[cfg(test)]
pub(crate) fn lane_relay_test_guard() -> std::sync::MutexGuard<'static, ()> {
    LANE_RELAY_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("lane relay test lock poisoned")
}

#[cfg(test)]
/// Reset settlement telemetry counters for isolated tests.
pub fn settlement_status_reset_for_tests() {
    *lock_operator_status_slot(settlement_status_slot(), "settlement status") =
        SettlementStatusState::default();
}

#[cfg(test)]
/// Reset process-local telemetry compatibility and lane-adapter diagnostics.
pub(crate) fn reset_rbc_backlog_stats_for_tests() {
    let _guard = rbc_status_test_guard();
    for counter in [
        &LAST_PROPOSE_MS,
        &LAST_COLLECT_DA_MS,
        &LAST_COLLECT_PREVOTE_MS,
        &LAST_COLLECT_PRECOMMIT_MS,
        &LAST_COLLECT_AGG_MS,
        &LAST_COMMIT_MS,
        &MAX_PROPOSE_MS,
        &MAX_COLLECT_DA_MS,
        &MAX_COLLECT_PREVOTE_MS,
        &MAX_COLLECT_PRECOMMIT_MS,
        &MAX_COLLECT_AGG_MS,
        &MAX_COMMIT_MS,
        &LAST_PROPOSE_EMA_MS,
        &LAST_COLLECT_DA_EMA_MS,
        &LAST_COLLECT_PREVOTE_EMA_MS,
        &LAST_COLLECT_PRECOMMIT_EMA_MS,
        &LAST_COLLECT_AGG_EMA_MS,
        &LAST_COMMIT_EMA_MS,
        &LAST_PIPELINE_TOTAL_EMA_MS,
        &GOSSIP_FALLBACK_TOTAL,
        &BLOCK_CREATED_DROPPED_BY_LOCK_TOTAL,
        &BLOCK_CREATED_HINT_MISMATCH_TOTAL,
        &BLOCK_CREATED_PROPOSAL_MISMATCH_TOTAL,
    ] {
        counter.store(0, Ordering::Relaxed);
    }
    *lock_operator_status_slot(availability_slot(), "availability vote stats") =
        AvailabilityStats::default();
    lock_operator_status_slot(qc_latency_slot(), "QC latency stats").clear();
    *lock_operator_status_slot(rbc_backlog_slot(), "RBC backlog snapshot") =
        RbcBacklogSnapshot::default();
    *lock_operator_status_slot(pending_rbc_slot(), "pending RBC snapshot") =
        PendingRbcSnapshot::default();
    lock_operator_status_slot(lane_activity_slot(), "lane activity snapshot").clear();
    lock_operator_status_slot(dataspace_activity_slot(), "dataspace activity snapshot").clear();
    *lock_operator_status_slot(pipeline_execution_slot(), "pipeline execution snapshot") =
        PipelineExecutionSnapshot::default();
    *lock_operator_status_slot(access_set_source_slot(), "access-set source snapshot") =
        AccessSetSourceSummary::default();
    PIPELINE_CONFLICT_RATE_BPS.store(0, Ordering::Relaxed);
}
