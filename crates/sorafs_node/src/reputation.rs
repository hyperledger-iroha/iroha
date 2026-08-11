//! Durable finalized-ledger ingest for deterministic SoraFS reputation material.
//!
//! This module is deliberately a projector, not a domain authority. Its public
//! ingest surface accepts only typed finalized event pages returned by native
//! ledger queries. It never signs a reputation snapshot and never turns a
//! process-local proof, repair, orderbook, dispute, token, or reserve outcome
//! into an authoritative event.
//!
//! PDP/PoTR, repair, orderbook, reserve, and the unified PoR/dispute/token
//! journal each retain one global contiguous cursor. The three semantic journal
//! sources intentionally share that physical cursor and finality anchor so an
//! interleaved committed journal cannot be partially projected.
//! This module defines the complete projection contract; ledger mutation,
//! storage, and native query/Torii wiring remain outside this service layer.

pub mod runtime;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use iroha_data_model::{
    NetworkId,
    events::data::sorafs::SorafsRepairLedgerEventKind,
    sorafs::{
        capacity::{CapacityDisputeOutcome, ProviderId},
        moderation_ledger::{
            REPAIR_QUERY_MAX_EVENT_PAGE_BYTES_V1, REPAIR_QUERY_MAX_ITEMS_V1,
            RepairFinalizedEventPageV1, RepairFinalizedEventV1,
        },
        orderbook::{
            ORDERBOOK_QUERY_MAX_EVENT_PAGE_BYTES_V1, ORDERBOOK_QUERY_MAX_ITEMS_V1,
            OrderbookFinalizedEventPageV1, OrderbookFinalizedEventV1,
        },
        proof_ledger::{
            PROOF_OUTCOME_QUERY_MAX_EVENT_PAGE_BYTES_V1, PROOF_OUTCOME_QUERY_MAX_ITEMS_V1,
            PdpOutcomeStatusV1, PotrOutcomeStatusV1, ProofOutcomeFinalizedEventPageV1,
            ProofOutcomeFinalizedEventV1, ProofOutcomeProjectionV1,
        },
        reputation::{
            PorTerminalStatusV1, ProviderDisputeStatusV1,
            REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1, ReputationJournalFinalizedEventPageV1,
            ReputationJournalFinalizedEventV1, ReputationJournalPayloadV1,
            ReputationJournalValidationError, StreamTokenValidationStatusV1,
        },
        reserve::{
            RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1, RESERVE_QUERY_MAX_ITEMS_V1,
            ReserveFinalizedEventPageV1, ReserveLifecycleStage, ReserveProviderAccountPageV1,
        },
    },
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::reputation::{
    REPUTATION_PROVIDER_INPUT_VERSION_V1, REPUTATION_PROVIDER_METRICS_VERSION_V1,
    ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
    ReputationSnapshotV1, ReputationWeightsV1, build_reputation_snapshot,
    signed::{
        REPUTATION_SCORING_EVIDENCE_VERSION_V1, ReputationScoringEvidenceV1,
        ReputationSnapshotTrustPolicyV1, SignedReputationSnapshotV1, snapshot_signing_digest,
    },
};
use thiserror::Error;

use crate::durable_transaction_forwarder::{AtomicCheckpointStore, CheckpointStoreError};

/// Durable reputation-ingest policy schema version.
pub const REPUTATION_INGEST_POLICY_VERSION_V1: u8 = 1;
/// Durable reputation checkpoint schema version.
pub const REPUTATION_INGEST_CHECKPOINT_VERSION_V1: u8 = 1;
/// Unsigned signing-material schema version.
pub const REPUTATION_UNSIGNED_MATERIAL_VERSION_V1: u8 = 1;
/// Unsigned-material delivery schema version.
pub const REPUTATION_UNSIGNED_MATERIAL_DELIVERY_VERSION_V1: u8 = 1;
/// Unsigned-material acknowledgement schema version.
pub const REPUTATION_UNSIGNED_MATERIAL_ACKNOWLEDGEMENT_VERSION_V1: u8 = 1;
/// Canonical checkpoint file below the configured private state root.
pub const REPUTATION_INGEST_CHECKPOINT_FILE_NAME_V1: &str = "reputation-committed-ingest-v1.to";
/// Single-writer lock file below the configured private state root.
pub const REPUTATION_INGEST_LOCK_FILE_NAME_V1: &str = "reputation-committed-ingest-v1.lock";

/// V1 provider ceiling, aligned with the canonical reputation scorer.
pub const REPUTATION_INGEST_MAX_PROVIDERS_V1: u32 = 65_536;
/// Hard cap on typed events presented in one atomic ingest batch.
pub const REPUTATION_INGEST_MAX_PENDING_EVENTS_V1: u32 = 65_536;
/// Hard cap on retained exact-replay receipts.
pub const REPUTATION_INGEST_MAX_REPLAY_RECEIPTS_V1: u32 = 262_144;
/// Hard cap on typed ledger pages accepted in one ingest call.
pub const REPUTATION_INGEST_MAX_PAGES_PER_BATCH_V1: u32 = 4_096;
/// Hard cap on the canonical durable checkpoint.
pub const REPUTATION_INGEST_MAX_CHECKPOINT_BYTES_V1: u64 = 64 * 1024 * 1024;
/// Minimum checkpoint ceiling accepted by a production policy.
pub const REPUTATION_INGEST_MIN_CHECKPOINT_BYTES_V1: u64 = 64 * 1024;
/// Hard cap on failed external delivery attempts retained for one release window.
pub const REPUTATION_UNSIGNED_MATERIAL_MAX_DELIVERY_FAILURES_V1: u32 = 64;

const CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT: usize = 16;
const CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT: usize = 16;
const CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES: usize = 4 * 1024 * 1024;
const CHECKPOINT_MAX_NESTING_DEPTH: usize = 64;

/// One canonical finalized-ledger source consumed by reputation scoring.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub enum ReputationSourceV1 {
    /// Native PDP/PoTR terminal-outcome feed.
    Proof,
    /// Native PoR terminal-outcome feed.
    Por,
    /// Native repair lifecycle feed.
    Repair,
    /// Native orderbook lifecycle feed.
    Orderbook,
    /// Native provider dispute feed.
    Dispute,
    /// Native stream-token validation feed.
    Token,
    /// Native reserve/rent lifecycle feed.
    Reserve,
}

impl ReputationSourceV1 {
    /// Stable, bounded metric label for this source.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Proof => "proof",
            Self::Por => "por",
            Self::Repair => "repair",
            Self::Orderbook => "orderbook",
            Self::Dispute => "dispute",
            Self::Token => "token",
            Self::Reserve => "reserve",
        }
    }

    const fn bit(self) -> u16 {
        match self {
            Self::Proof => 1 << 0,
            Self::Por => 1 << 1,
            Self::Repair => 1 << 2,
            Self::Orderbook => 1 << 3,
            Self::Dispute => 1 << 4,
            Self::Token => 1 << 5,
            Self::Reserve => 1 << 6,
        }
    }
}

const ALL_SOURCES: [ReputationSourceV1; 7] = [
    ReputationSourceV1::Proof,
    ReputationSourceV1::Por,
    ReputationSourceV1::Repair,
    ReputationSourceV1::Orderbook,
    ReputationSourceV1::Dispute,
    ReputationSourceV1::Token,
    ReputationSourceV1::Reserve,
];

/// Governed set of finalized feeds required for one material release.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub struct ReputationRequiredSourceMaskV1(u16);

impl ReputationRequiredSourceMaskV1 {
    /// Canonical first-release source set.
    pub const ALL_V1: Self = Self(
        ReputationSourceV1::Proof.bit()
            | ReputationSourceV1::Por.bit()
            | ReputationSourceV1::Repair.bit()
            | ReputationSourceV1::Orderbook.bit()
            | ReputationSourceV1::Dispute.bit()
            | ReputationSourceV1::Token.bit()
            | ReputationSourceV1::Reserve.bit(),
    );

    /// No V1 source is intentionally omitted from the projector model.
    pub const UNAVAILABLE_NATIVE_V1: Self = Self::EMPTY;

    /// Empty source set.
    pub const EMPTY: Self = Self(0);

    /// Whether this mask contains `source`.
    #[must_use]
    pub const fn contains(self, source: ReputationSourceV1) -> bool {
        self.0 & source.bit() != 0
    }

    /// Raw stable bit representation.
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    const fn is_empty(self) -> bool {
        self.0 == 0
    }

    const fn from_source(source: ReputationSourceV1) -> Self {
        Self(source.bit())
    }
}

/// Exact identity of one immutable finalized ledger view.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub struct ReputationFinalizedIdentityV1 {
    /// Finalized block height.
    pub height: u64,
    /// Exact finalized block hash.
    pub block_hash: [u8; 32],
}

impl ReputationFinalizedIdentityV1 {
    fn validate(self) -> Result<(), ReputationIngestError> {
        if self.height == 0 || self.block_hash == [0; 32] {
            return Err(ReputationIngestError::InvalidFinalizedIdentity);
        }
        Ok(())
    }
}

/// Exact identity of one source event in a finalized block.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub struct ReputationCommittedEventIdentityV1 {
    /// Monotonic source sequence beginning at one.
    pub sequence: u64,
    /// Finalized block height containing the event.
    pub block_height: u64,
    /// Exact finalized hash for `block_height`.
    pub block_hash: [u8; 32],
    /// Source-event index within the committing block.
    pub event_index: u32,
}

impl ReputationCommittedEventIdentityV1 {
    fn validate(self) -> Result<(), ReputationIngestError> {
        if self.sequence == 0 || self.block_height == 0 || self.block_hash == [0; 32] {
            return Err(ReputationIngestError::InvalidEventIdentity);
        }
        Ok(())
    }
}

/// Governed, deterministic ingest policy for one reputation release window.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ReputationIngestPolicyV1 {
    /// Schema version.
    pub version: u8,
    /// Exact network whose finalized views may feed this projector.
    pub network_id: NetworkId,
    /// Every source that must be finalized at the release target.
    pub required_sources: ReputationRequiredSourceMaskV1,
    /// Inclusive first block whose events affect this release.
    pub window_start_height: u64,
    /// Inclusive final block and mandatory signing-material target.
    pub window_end_height: u64,
    /// Digest of the external threshold-signer trust policy.
    pub snapshot_trust_policy_digest: [u8; 32],
    /// Governed deterministic scoring weights.
    pub weights: ReputationWeightsV1,
    /// Maximum provider accumulators retained.
    pub max_providers: u32,
    /// Maximum typed events presented in one atomic batch.
    pub max_pending_events: u32,
    /// Maximum exact-replay receipts retained.
    pub max_replay_receipts: u32,
    /// Maximum native query pages accepted in one call.
    pub max_pages_per_batch: u32,
    /// Failed external delivery attempts retained before dead-lettering material.
    pub max_material_delivery_failures: u32,
    /// Maximum canonical checkpoint length.
    pub checkpoint_max_bytes: u64,
}

impl ReputationIngestPolicyV1 {
    /// Construct the strict first-release policy for one finalized block window.
    #[must_use]
    pub fn strict_v1(
        network_id: NetworkId,
        window_start_height: u64,
        window_end_height: u64,
        snapshot_trust_policy_digest: [u8; 32],
        weights: ReputationWeightsV1,
    ) -> Self {
        Self {
            version: REPUTATION_INGEST_POLICY_VERSION_V1,
            network_id,
            required_sources: ReputationRequiredSourceMaskV1::ALL_V1,
            window_start_height,
            window_end_height,
            snapshot_trust_policy_digest,
            weights,
            max_providers: REPUTATION_INGEST_MAX_PROVIDERS_V1,
            max_pending_events: REPUTATION_INGEST_MAX_PENDING_EVENTS_V1,
            max_replay_receipts: REPUTATION_INGEST_MAX_REPLAY_RECEIPTS_V1,
            max_pages_per_batch: REPUTATION_INGEST_MAX_PAGES_PER_BATCH_V1,
            max_material_delivery_failures: REPUTATION_UNSIGNED_MATERIAL_MAX_DELIVERY_FAILURES_V1,
            checkpoint_max_bytes: REPUTATION_INGEST_MAX_CHECKPOINT_BYTES_V1,
        }
    }

    /// Validate the canonical first-release policy.
    ///
    /// # Errors
    ///
    /// Returns [`ReputationIngestError::InvalidPolicy`] for an unsupported
    /// version, incomplete V1 source mask, invalid window, inert trust-policy
    /// digest, invalid scoring weights, or a resource bound outside hard caps.
    pub fn validate(&self) -> Result<(), ReputationIngestError> {
        if self.version != REPUTATION_INGEST_POLICY_VERSION_V1
            || self.network_id.as_bytes()[31] & 1 != 1
            || self.required_sources != ReputationRequiredSourceMaskV1::ALL_V1
            || self.window_start_height == 0
            || self.window_end_height < self.window_start_height
            || self.snapshot_trust_policy_digest == [0; 32]
            || self.max_providers == 0
            || self.max_providers > REPUTATION_INGEST_MAX_PROVIDERS_V1
            || self.max_pending_events == 0
            || self.max_pending_events > REPUTATION_INGEST_MAX_PENDING_EVENTS_V1
            || self.max_replay_receipts == 0
            || self.max_replay_receipts > REPUTATION_INGEST_MAX_REPLAY_RECEIPTS_V1
            || self.max_pages_per_batch == 0
            || self.max_pages_per_batch > REPUTATION_INGEST_MAX_PAGES_PER_BATCH_V1
            || self.max_material_delivery_failures == 0
            || self.max_material_delivery_failures
                > REPUTATION_UNSIGNED_MATERIAL_MAX_DELIVERY_FAILURES_V1
            || self.checkpoint_max_bytes < REPUTATION_INGEST_MIN_CHECKPOINT_BYTES_V1
            || self.checkpoint_max_bytes > REPUTATION_INGEST_MAX_CHECKPOINT_BYTES_V1
            || self.weights.validate().is_err()
        {
            return Err(ReputationIngestError::InvalidPolicy);
        }
        Ok(())
    }

    fn canonical_digest(&self) -> Result<[u8; 32], ReputationIngestError> {
        self.validate()?;
        hash_canonical(b"sorafs-reputation-ingest-policy-v1", self)
    }
}

/// One set of typed pages read from a coherent finalized ledger view.
///
/// Pages for a source may be omitted while another source catches up. Material
/// emission remains blocked until every governed source reports complete at
/// the same final target. A completed reserve event page requires a complete,
/// same-anchor provider-account projection so lifecycle stages are never
/// guessed.
#[derive(Debug, Clone)]
pub struct ReputationFinalizedBatchV1 {
    /// Exact network from which every page and finalized block timestamp was read.
    pub network_id: NetworkId,
    /// Finalized block timestamp read from the same immutable block metadata.
    pub finalized_at_unix_ms: u64,
    /// Ordered pages from `FindSorafsProofOutcomeEvents`.
    pub proof_pages: Vec<ProofOutcomeFinalizedEventPageV1>,
    /// Ordered pages from the unified PoR/dispute/token reputation journal.
    pub journal_pages: Vec<ReputationJournalFinalizedEventPageV1>,
    /// Ordered pages from `FindSorafsRepairEvents`.
    pub repair_pages: Vec<RepairFinalizedEventPageV1>,
    /// Ordered pages from `FindSorafsOrderbookEvents`.
    pub orderbook_pages: Vec<OrderbookFinalizedEventPageV1>,
    /// Ordered pages from `FindSorafsReserveEvents`.
    pub reserve_pages: Vec<ReserveFinalizedEventPageV1>,
    /// Complete ordered pages from `FindSorafsReserveProviders` at the same anchor.
    pub reserve_provider_pages: Vec<ReserveProviderAccountPageV1>,
}

/// Finalized source position bound into unsigned signing material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ReputationSourceFinalityV1 {
    /// Native source.
    pub source: ReputationSourceV1,
    /// Exact target through which this source was completely queried.
    pub observed_through: ReputationFinalizedIdentityV1,
    /// Last committed source event, absent only when the feed is empty.
    pub last_event: Option<ReputationCommittedEventIdentityV1>,
}

/// Restart-safe query position for one physical committed ledger feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ReputationCommittedFeedCursorV1 {
    /// Physical native feed.
    pub feed: ReputationCommittedFeedV1,
    /// Last committed event durably projected, for the next query's `after` cursor.
    pub after: Option<ReputationCommittedEventIdentityV1>,
    /// Exact finalized view completely observed by this feed.
    pub observed_through: Option<ReputationFinalizedIdentityV1>,
    /// Finalized block timestamp paired with `observed_through`.
    pub observed_at_unix_ms: u64,
}

/// Canonical unsigned material ready for independent threshold signers.
///
/// The service returns the exact snapshot signing digest but contains no key,
/// signature, signer identity, or signing callback.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ReputationUnsignedSigningMaterialV1 {
    /// Schema version.
    pub version: u8,
    /// Exact network that produced every committed source event.
    pub network_id: NetworkId,
    /// Digest of the deterministic ingest policy and window.
    pub ingest_policy_digest: [u8; 32],
    /// Digest of the external threshold-signer trust policy.
    pub snapshot_trust_policy_digest: [u8; 32],
    /// Inclusive reputation window start.
    pub window_start_height: u64,
    /// Inclusive reputation window end.
    pub window_end_height: u64,
    /// Exact shared finalized target.
    pub target_finalized: ReputationFinalizedIdentityV1,
    /// Exact finalized block timestamp paired with `target_finalized`.
    pub target_finalized_at_unix_ms: u64,
    /// Required source finality in stable source order.
    pub source_finality: Vec<ReputationSourceFinalityV1>,
    /// Canonical deterministic scoring evidence.
    pub scoring_evidence: ReputationScoringEvidenceV1,
    /// Digest of `scoring_evidence`.
    pub scoring_evidence_digest: [u8; 32],
    /// Canonical deterministic snapshot.
    pub snapshot: ReputationSnapshotV1,
    /// Exact digest external threshold signers must sign.
    pub snapshot_signing_digest: [u8; 32],
}

/// Durable state of one externally delivered unsigned-material item.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub enum ReputationUnsignedMaterialDeliveryStateV1 {
    /// The external threshold-signing worker may retry this item.
    Pending,
    /// The governed retry ceiling was reached without acknowledgement.
    DeadLetter,
}

/// Canonical, payload-bounded item exposed to an external threshold-signing worker.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ReputationUnsignedMaterialDeliveryV1 {
    /// Schema version.
    pub version: u8,
    /// Stable one-based sequence within this immutable V1 release window.
    pub sequence: u64,
    /// Digest of the exact canonical `material` bytes.
    pub material_digest: [u8; 32],
    /// Deterministic unsigned snapshot material; no signing key or signature is present.
    pub material: ReputationUnsignedSigningMaterialV1,
    /// Number of distinct, durably recorded failed delivery attempts.
    pub failed_attempts: u32,
    /// Current retry/dead-letter state.
    pub state: ReputationUnsignedMaterialDeliveryStateV1,
}

/// Payload-free durable acknowledgement retained after external delivery succeeds.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub struct ReputationUnsignedMaterialAcknowledgementV1 {
    /// Schema version.
    pub version: u8,
    /// Exact acknowledged delivery sequence.
    pub sequence: u64,
    /// Digest of the exact unsigned material delivered.
    pub material_digest: [u8; 32],
    /// Canonical digest of the public trust policy used for verification.
    pub trust_policy_digest: [u8; 32],
    /// Authoritative finalized-chain admission time derived from the locked checkpoint.
    pub verified_at_unix: u64,
    /// Digest of the canonical externally signed result.
    pub signed_result_digest: [u8; 32],
}

/// Result of durably enqueuing deterministic unsigned material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationMaterialEnqueueOutcomeV1 {
    /// A new delivery item was durably enqueued.
    Enqueued {
        /// Stable delivery sequence.
        sequence: u64,
    },
    /// The exact item was already present in the outbox.
    ExactReplay {
        /// Stable delivery sequence.
        sequence: u64,
        /// Current retry/dead-letter state.
        state: ReputationUnsignedMaterialDeliveryStateV1,
    },
    /// The exact item was already acknowledged.
    AlreadyAcknowledged {
        /// Stable delivery sequence.
        sequence: u64,
    },
}

/// Result of recording one idempotent external-delivery failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationMaterialFailureOutcomeV1 {
    /// A new failed attempt was durably recorded and retries remain.
    RetryPending {
        /// Number of distinct failures now retained.
        failed_attempts: u32,
        /// Attempts remaining before the item is dead-lettered.
        remaining_attempts: u32,
    },
    /// This failure reached the governed retry ceiling.
    DeadLettered {
        /// Number of distinct failures retained.
        failed_attempts: u32,
    },
    /// The exact failure receipt was already durably recorded.
    ExactReplay {
        /// Number of distinct failures retained.
        failed_attempts: u32,
        /// Current retry/dead-letter state.
        state: ReputationUnsignedMaterialDeliveryStateV1,
    },
}

/// Result of acknowledging one externally signed material delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationMaterialAcknowledgementOutcomeV1 {
    /// The matching outbox item was durably acknowledged and removed.
    Acknowledged,
    /// The exact acknowledgement was already durable.
    ExactReplay,
}

/// Payload-free service counters with no provider or event labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReputationIngestMetricsSnapshot {
    /// Atomic batches durably applied.
    pub batches_applied: u64,
    /// New typed committed events projected.
    pub events_applied: u64,
    /// Exact retained event replays accepted.
    pub exact_replays: u64,
    /// Calls that changed only finalized source positions.
    pub finality_only_batches: u64,
    /// Reordered inputs rejected.
    pub reordered_rejections: u64,
    /// Event gaps rejected.
    pub gap_rejections: u64,
    /// Same-identity payload conflicts rejected.
    pub equivocation_rejections: u64,
    /// Finalized fork identities rejected.
    pub fork_rejections: u64,
    /// Resource-bound violations rejected.
    pub bound_rejections: u64,
    /// Durable checkpoint failures.
    pub checkpoint_failures: u64,
    /// Pending batches recovered after restart.
    pub restart_reconciliations: u64,
    /// Signing-material requests rejected for incomplete sources.
    pub incomplete_material_rejections: u64,
    /// Deterministic material items durably enqueued.
    pub material_enqueued: u64,
    /// Distinct external-delivery failures durably retained.
    pub material_delivery_failures: u64,
    /// Material items moved to the dead-letter state.
    pub material_dead_letters: u64,
    /// Material items durably acknowledged.
    pub material_acknowledgements: u64,
    /// Idempotent material enqueue/failure/acknowledgement replays.
    pub material_exact_replays: u64,
}

#[derive(Debug, Default)]
struct ReputationIngestMetrics {
    batches_applied: AtomicU64,
    events_applied: AtomicU64,
    exact_replays: AtomicU64,
    finality_only_batches: AtomicU64,
    reordered_rejections: AtomicU64,
    gap_rejections: AtomicU64,
    equivocation_rejections: AtomicU64,
    fork_rejections: AtomicU64,
    bound_rejections: AtomicU64,
    checkpoint_failures: AtomicU64,
    restart_reconciliations: AtomicU64,
    incomplete_material_rejections: AtomicU64,
    material_enqueued: AtomicU64,
    material_delivery_failures: AtomicU64,
    material_dead_letters: AtomicU64,
    material_acknowledgements: AtomicU64,
    material_exact_replays: AtomicU64,
}

impl ReputationIngestMetrics {
    fn snapshot(&self) -> ReputationIngestMetricsSnapshot {
        ReputationIngestMetricsSnapshot {
            batches_applied: self.batches_applied.load(Ordering::Relaxed),
            events_applied: self.events_applied.load(Ordering::Relaxed),
            exact_replays: self.exact_replays.load(Ordering::Relaxed),
            finality_only_batches: self.finality_only_batches.load(Ordering::Relaxed),
            reordered_rejections: self.reordered_rejections.load(Ordering::Relaxed),
            gap_rejections: self.gap_rejections.load(Ordering::Relaxed),
            equivocation_rejections: self.equivocation_rejections.load(Ordering::Relaxed),
            fork_rejections: self.fork_rejections.load(Ordering::Relaxed),
            bound_rejections: self.bound_rejections.load(Ordering::Relaxed),
            checkpoint_failures: self.checkpoint_failures.load(Ordering::Relaxed),
            restart_reconciliations: self.restart_reconciliations.load(Ordering::Relaxed),
            incomplete_material_rejections: self
                .incomplete_material_rejections
                .load(Ordering::Relaxed),
            material_enqueued: self.material_enqueued.load(Ordering::Relaxed),
            material_delivery_failures: self.material_delivery_failures.load(Ordering::Relaxed),
            material_dead_letters: self.material_dead_letters.load(Ordering::Relaxed),
            material_acknowledgements: self.material_acknowledgements.load(Ordering::Relaxed),
            material_exact_replays: self.material_exact_replays.load(Ordering::Relaxed),
        }
    }
}

/// Result of one durable finalized batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationIngestOutcomeV1 {
    /// New events and/or source finality were applied.
    Applied {
        /// Number of newly projected events.
        events: u32,
    },
    /// Every event and finalized position exactly matched durable state.
    ExactReplay,
}

/// Payload-free source completion and queue status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationIngestStatusV1 {
    /// Ingest-policy digest bound to the checkpoint.
    pub policy_digest: [u8; 32],
    /// Latest coherent target presented to the service.
    pub latest_finalized: Option<ReputationFinalizedIdentityV1>,
    /// Latest finalized timestamp paired with `latest_finalized`.
    pub latest_finalized_at_unix_ms: u64,
    /// Per-source finalized positions in stable order.
    pub source_finality: Vec<(ReputationSourceV1, Option<ReputationFinalizedIdentityV1>)>,
    /// Governed sources not complete at `latest_finalized`.
    pub missing_sources: ReputationRequiredSourceMaskV1,
    /// Number of provider accumulators retained.
    pub providers: u32,
    /// Number of events staged for restart reconciliation.
    pub pending_events: u32,
    /// Current unsigned-material outbox state, when an item awaits acknowledgement.
    pub material_outbox_state: Option<ReputationUnsignedMaterialDeliveryStateV1>,
    /// Distinct failed delivery attempts retained for the current outbox item.
    pub material_delivery_failures: u32,
    /// Whether this release window's material has a durable acknowledgement.
    pub material_acknowledged: bool,
}

/// Deterministic ingest and persistence failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ReputationIngestError {
    /// Policy fields or resource bounds are not canonical for V1.
    #[error("reputation ingest policy is invalid")]
    InvalidPolicy,
    /// The finalized batch belongs to a different network.
    #[error("reputation finalized batch belongs to a different network")]
    NetworkIdMismatch,
    /// A finalized height/hash pair is empty.
    #[error("reputation finalized identity is invalid")]
    InvalidFinalizedIdentity,
    /// A committed event has an empty sequence, height, or block hash.
    #[error("reputation committed-event identity is invalid")]
    InvalidEventIdentity,
    /// A page is malformed or its continuation metadata is inconsistent.
    #[error("reputation committed-event page is invalid")]
    InvalidPage,
    /// Pages in one batch do not share one immutable finalized view.
    #[error("reputation page finalized anchors do not match")]
    PageAnchorMismatch,
    /// A batch attempted to move the global finalized target backward.
    #[error("reputation finalized target is stale")]
    StaleFinalizedIdentity,
    /// The same finalized height was presented with a different block hash.
    #[error("reputation finalized fork was rejected")]
    FinalizedFork,
    /// Source events were not in strict sequence/block/index order.
    #[error("reputation committed events are reordered")]
    EventReordered,
    /// A new source sequence skipped at least one committed event.
    #[error("reputation committed-event sequence has a gap")]
    EventGap,
    /// A retained source sequence was replayed with different identity or bytes.
    #[error("reputation committed-event equivocation was rejected")]
    EventEquivocation,
    /// An old replay fell outside the bounded exact-replay receipt suffix.
    #[error("reputation event replay is outside retained receipts")]
    ReplayOutsideRetention,
    /// A typed event or reserve projection references an inert provider.
    #[error("reputation source references an invalid provider")]
    InvalidProvider,
    /// A complete reserve feed omitted its same-anchor provider projection.
    #[error("reputation reserve-stage projection is missing")]
    ReserveStageResolutionMissing,
    /// A provider has no chain-authoritative reserve lifecycle stage.
    #[error("reputation provider reserve stage is unavailable")]
    ReserveStageUnavailable,
    /// A required proof metric has no committed observations.
    #[error("reputation provider metric is unavailable from its native source")]
    MetricUnavailable,
    /// No provider can be scored from the committed source set.
    #[error("reputation committed source set contains no providers")]
    EmptyProviderSet,
    /// Not every governed required source is finalized at the target.
    #[error("reputation required sources are incomplete")]
    MissingRequiredSources,
    /// The common target is not the governed release-window endpoint.
    #[error("reputation release window is not finalized")]
    WindowNotFinalized,
    /// A bounded counter or timestamp arithmetic operation overflowed.
    #[error("reputation deterministic integer arithmetic overflowed")]
    ArithmeticOverflow,
    /// A provider, page, event, queue, receipt, or checkpoint bound was exceeded.
    #[error("reputation ingest resource bound was exceeded")]
    CapacityExceeded,
    /// Canonical Norito encoding or scoring validation failed.
    #[error("reputation canonical material is invalid")]
    CanonicalEncoding,
    /// The durable checkpoint is corrupt, noncanonical, or inconsistent.
    #[error("reputation ingest checkpoint is invalid")]
    InvalidCheckpoint,
    /// A durably staged batch must be reconciled before material emission.
    #[error("reputation ingest batch reconciliation is pending")]
    ReconciliationPending,
    /// A different unsigned-material item or acknowledgement occupies the V1 window.
    #[error("reputation unsigned-material outbox conflicts with durable state")]
    MaterialOutboxConflict,
    /// A failure receipt or acknowledgement does not identify the durable item.
    #[error("reputation unsigned-material delivery identity does not match")]
    MaterialDeliveryMismatch,
    /// A failure or signed-result receipt digest is empty or malformed.
    #[error("reputation unsigned-material delivery receipt is invalid")]
    InvalidDeliveryReceipt,
    /// The governed delivery retry ceiling was already reached.
    #[error("reputation unsigned-material delivery is dead-lettered")]
    MaterialDeliveryDeadLettered,
    /// The durable checkpoint path is unsafe or inaccessible.
    #[error("reputation ingest checkpoint I/O failed")]
    CheckpointIo,
    /// The durable checkpoint exceeds its policy ceiling.
    #[error("reputation ingest checkpoint exceeds its byte limit")]
    CheckpointTooLarge,
    /// Another process owns the checkpoint writer lock.
    #[error("reputation ingest checkpoint writer is busy")]
    CheckpointBusy,
    /// Another runtime changed the checkpoint.
    #[error("reputation ingest checkpoint changed concurrently")]
    CheckpointStale,
    /// Rename became visible but parent-directory durability is uncertain.
    #[error("reputation ingest checkpoint durability is uncertain")]
    CheckpointDurabilityUncertain,
    /// The in-process writer registry or service mutex was poisoned.
    #[error("reputation ingest runtime is poisoned")]
    RuntimePoisoned,
}

impl From<CheckpointStoreError> for ReputationIngestError {
    fn from(error: CheckpointStoreError) -> Self {
        match error {
            CheckpointStoreError::Io => Self::CheckpointIo,
            CheckpointStoreError::TooLarge => Self::CheckpointTooLarge,
            CheckpointStoreError::Busy => Self::CheckpointBusy,
            CheckpointStoreError::Stale => Self::CheckpointStale,
            CheckpointStoreError::DurabilityUncertain => Self::CheckpointDurabilityUncertain,
            CheckpointStoreError::RuntimePoisoned => Self::RuntimePoisoned,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct SourceProgressV1 {
    source: ReputationSourceV1,
    last_event: Option<ReputationCommittedEventIdentityV1>,
    observed_through: Option<ReputationFinalizedIdentityV1>,
    observed_at_unix_ms: u64,
    reserve_projection_digest: Option<[u8; 32]>,
}

impl SourceProgressV1 {
    fn empty(source: ReputationSourceV1) -> Self {
        Self {
            source,
            last_event: None,
            observed_through: None,
            observed_at_unix_ms: 0,
            reserve_projection_digest: None,
        }
    }
}

/// Physical committed feed whose sequence is globally contiguous.
///
/// PoR, dispute, and token entries deliberately share `Journal`; their
/// semantic source rows are three views of this one cursor.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub enum ReputationCommittedFeedV1 {
    /// Native PDP/PoTR proof-outcome feed.
    Proof,
    /// Unified native PoR/dispute/stream-token journal feed.
    Journal,
    /// Native repair lifecycle feed.
    Repair,
    /// Native orderbook lifecycle feed.
    Orderbook,
    /// Native reserve/rent lifecycle feed.
    Reserve,
}

impl ReputationCommittedFeedV1 {
    /// Stable bounded label for metrics and operational status.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Proof => "proof",
            Self::Journal => "journal",
            Self::Repair => "repair",
            Self::Orderbook => "orderbook",
            Self::Reserve => "reserve",
        }
    }
}

const ALL_COMMITTED_FEEDS: [ReputationCommittedFeedV1; 5] = [
    ReputationCommittedFeedV1::Proof,
    ReputationCommittedFeedV1::Journal,
    ReputationCommittedFeedV1::Repair,
    ReputationCommittedFeedV1::Orderbook,
    ReputationCommittedFeedV1::Reserve,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct EventReceiptV1 {
    feed: ReputationCommittedFeedV1,
    identity: ReputationCommittedEventIdentityV1,
    event_digest: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum ReputationDisputeSignalV1 {
    Opened,
    Resolved { upheld: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum ReputationSignalV1 {
    Noop,
    ProviderObserved {
        provider_id: ProviderId,
    },
    Pdp {
        provider_id: ProviderId,
        success: bool,
    },
    Por {
        provider_id: ProviderId,
        counts_for_provider: bool,
        success: bool,
    },
    Potr {
        provider_id: ProviderId,
        counts_for_provider: bool,
        success: bool,
        latency_healthy: bool,
    },
    Repair {
        provider_id: ProviderId,
        terminal: bool,
        breach: bool,
        slashing: bool,
    },
    Dispute {
        provider_id: ProviderId,
        transition: ReputationDisputeSignalV1,
    },
    Token {
        provider_id: ProviderId,
        counts_for_provider: bool,
        violation: bool,
    },
}

impl ReputationSignalV1 {
    const fn provider_id(self) -> Option<ProviderId> {
        match self {
            Self::Noop => None,
            Self::ProviderObserved { provider_id }
            | Self::Pdp { provider_id, .. }
            | Self::Por { provider_id, .. }
            | Self::Potr { provider_id, .. }
            | Self::Repair { provider_id, .. }
            | Self::Dispute { provider_id, .. }
            | Self::Token { provider_id, .. } => Some(provider_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PendingEventV1 {
    feed: ReputationCommittedFeedV1,
    identity: ReputationCommittedEventIdentityV1,
    event_digest: [u8; 32],
    occurred_at_unix_ms: u64,
    signal: ReputationSignalV1,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct SourceFinalityUpdateV1 {
    source: ReputationSourceV1,
    target: ReputationFinalizedIdentityV1,
    finalized_at_unix_ms: u64,
    last_event: Option<ReputationCommittedEventIdentityV1>,
    complete: bool,
    reserve_projection_digest: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ReserveStageRecordV1 {
    provider_id: ProviderId,
    stage: ReputationReserveStageV1,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PendingBatchV1 {
    target: ReputationFinalizedIdentityV1,
    finalized_at_unix_ms: u64,
    events: Vec<PendingEventV1>,
    finality_updates: Vec<SourceFinalityUpdateV1>,
    reserve_stages: Vec<ReserveStageRecordV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ProviderAccumulatorV1 {
    provider_id: ProviderId,
    por_successes: u64,
    por_total: u64,
    pdp_successes: u64,
    pdp_total: u64,
    potr_successes: u64,
    potr_total: u64,
    latency_healthy: u64,
    latency_total: u64,
    disputes_upheld: u64,
    disputes_resolved: u64,
    token_violations: u64,
    token_observations: u64,
    repair_breaches: u64,
    repair_terminals: u64,
    active_disputes: u64,
    slashing_event: bool,
    reserve_stage: Option<ReputationReserveStageV1>,
}

impl ProviderAccumulatorV1 {
    const fn new(provider_id: ProviderId) -> Self {
        Self {
            provider_id,
            por_successes: 0,
            por_total: 0,
            pdp_successes: 0,
            pdp_total: 0,
            potr_successes: 0,
            potr_total: 0,
            latency_healthy: 0,
            latency_total: 0,
            disputes_upheld: 0,
            disputes_resolved: 0,
            token_violations: 0,
            token_observations: 0,
            repair_breaches: 0,
            repair_terminals: 0,
            active_disputes: 0,
            slashing_event: false,
            reserve_stage: None,
        }
    }

    const fn has_active_dispute(&self) -> bool {
        self.active_disputes > 0
    }

    fn apply(&mut self, signal: ReputationSignalV1) -> Result<(), ReputationIngestError> {
        match signal {
            ReputationSignalV1::Noop | ReputationSignalV1::ProviderObserved { .. } => {}
            ReputationSignalV1::Pdp { success, .. } => {
                let total = checked_next(self.pdp_total)?;
                let successes = if success {
                    checked_next(self.pdp_successes)?
                } else {
                    self.pdp_successes
                };
                self.pdp_total = total;
                self.pdp_successes = successes;
            }
            ReputationSignalV1::Por {
                counts_for_provider,
                success,
                ..
            } => {
                if !counts_for_provider {
                    if success {
                        return Err(ReputationIngestError::InvalidCheckpoint);
                    }
                } else {
                    let total = checked_next(self.por_total)?;
                    let successes = if success {
                        checked_next(self.por_successes)?
                    } else {
                        self.por_successes
                    };
                    self.por_total = total;
                    self.por_successes = successes;
                }
            }
            ReputationSignalV1::Potr {
                counts_for_provider,
                success,
                latency_healthy,
                ..
            } => {
                if latency_healthy && !success {
                    return Err(ReputationIngestError::InvalidCheckpoint);
                }
                if !counts_for_provider {
                    if success || latency_healthy {
                        return Err(ReputationIngestError::InvalidCheckpoint);
                    }
                } else {
                    let total = checked_next(self.potr_total)?;
                    let latency_total = checked_next(self.latency_total)?;
                    let successes = if success {
                        checked_next(self.potr_successes)?
                    } else {
                        self.potr_successes
                    };
                    let latency_healthy_count = if latency_healthy {
                        checked_next(self.latency_healthy)?
                    } else {
                        self.latency_healthy
                    };
                    self.potr_total = total;
                    self.latency_total = latency_total;
                    self.potr_successes = successes;
                    self.latency_healthy = latency_healthy_count;
                }
            }
            ReputationSignalV1::Repair {
                terminal,
                breach,
                slashing,
                ..
            } => {
                if breach && !terminal {
                    return Err(ReputationIngestError::InvalidCheckpoint);
                }
                if terminal {
                    let terminals = checked_next(self.repair_terminals)?;
                    let breaches = if breach {
                        checked_next(self.repair_breaches)?
                    } else {
                        self.repair_breaches
                    };
                    self.repair_terminals = terminals;
                    self.repair_breaches = breaches;
                }
                self.slashing_event |= slashing;
            }
            ReputationSignalV1::Dispute { transition, .. } => match transition {
                ReputationDisputeSignalV1::Opened => {
                    self.active_disputes = checked_next(self.active_disputes)?;
                }
                ReputationDisputeSignalV1::Resolved { upheld } => {
                    let active_disputes = self
                        .active_disputes
                        .checked_sub(1)
                        .ok_or(ReputationIngestError::ArithmeticOverflow)?;
                    let resolved = checked_next(self.disputes_resolved)?;
                    let upheld_count = if upheld {
                        checked_next(self.disputes_upheld)?
                    } else {
                        self.disputes_upheld
                    };
                    self.active_disputes = active_disputes;
                    self.disputes_resolved = resolved;
                    self.disputes_upheld = upheld_count;
                }
            },
            ReputationSignalV1::Token {
                counts_for_provider,
                violation,
                ..
            } => {
                if !counts_for_provider {
                    if violation {
                        return Err(ReputationIngestError::InvalidCheckpoint);
                    }
                } else {
                    let observations = checked_next(self.token_observations)?;
                    let violations = if violation {
                        checked_next(self.token_violations)?
                    } else {
                        self.token_violations
                    };
                    self.token_observations = observations;
                    self.token_violations = violations;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ReputationUnsignedMaterialOutboxEntryV1 {
    version: u8,
    sequence: u64,
    material_digest: [u8; 32],
    failed_attempts: u32,
    state: ReputationUnsignedMaterialDeliveryStateV1,
    failure_receipts: Vec<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ReputationIngestCheckpointV1 {
    version: u8,
    policy_digest: [u8; 32],
    latest_finalized: Option<ReputationFinalizedIdentityV1>,
    latest_finalized_at_unix_ms: u64,
    source_progress: Vec<SourceProgressV1>,
    receipts: Vec<EventReceiptV1>,
    providers: Vec<ProviderAccumulatorV1>,
    pending: Option<PendingBatchV1>,
    material_outbox: Option<ReputationUnsignedMaterialOutboxEntryV1>,
    material_acknowledgement: Option<ReputationUnsignedMaterialAcknowledgementV1>,
}

impl ReputationIngestCheckpointV1 {
    fn empty(policy_digest: [u8; 32]) -> Self {
        Self {
            version: REPUTATION_INGEST_CHECKPOINT_VERSION_V1,
            policy_digest,
            latest_finalized: None,
            latest_finalized_at_unix_ms: 0,
            source_progress: ALL_SOURCES
                .into_iter()
                .map(SourceProgressV1::empty)
                .collect(),
            receipts: Vec::new(),
            providers: Vec::new(),
            pending: None,
            material_outbox: None,
            material_acknowledgement: None,
        }
    }

    fn progress(&self, source: ReputationSourceV1) -> &SourceProgressV1 {
        self.source_progress
            .iter()
            .find(|progress| progress.source == source)
            .expect("validated checkpoint contains every reputation source")
    }

    fn progress_mut(&mut self, source: ReputationSourceV1) -> &mut SourceProgressV1 {
        self.source_progress
            .iter_mut()
            .find(|progress| progress.source == source)
            .expect("validated checkpoint contains every reputation source")
    }
}

const fn committed_feed_for_source(source: ReputationSourceV1) -> ReputationCommittedFeedV1 {
    match source {
        ReputationSourceV1::Proof => ReputationCommittedFeedV1::Proof,
        ReputationSourceV1::Por | ReputationSourceV1::Dispute | ReputationSourceV1::Token => {
            ReputationCommittedFeedV1::Journal
        }
        ReputationSourceV1::Repair => ReputationCommittedFeedV1::Repair,
        ReputationSourceV1::Orderbook => ReputationCommittedFeedV1::Orderbook,
        ReputationSourceV1::Reserve => ReputationCommittedFeedV1::Reserve,
    }
}

const fn primary_source_for_feed(feed: ReputationCommittedFeedV1) -> ReputationSourceV1 {
    match feed {
        ReputationCommittedFeedV1::Proof => ReputationSourceV1::Proof,
        ReputationCommittedFeedV1::Journal => ReputationSourceV1::Por,
        ReputationCommittedFeedV1::Repair => ReputationSourceV1::Repair,
        ReputationCommittedFeedV1::Orderbook => ReputationSourceV1::Orderbook,
        ReputationCommittedFeedV1::Reserve => ReputationSourceV1::Reserve,
    }
}

fn checkpoint_progress_for_feed(
    checkpoint: &ReputationIngestCheckpointV1,
    feed: ReputationCommittedFeedV1,
) -> &SourceProgressV1 {
    checkpoint.progress(primary_source_for_feed(feed))
}

#[derive(Debug)]
struct RuntimeState {
    checkpoint: ReputationIngestCheckpointV1,
    fingerprint: Option<[u8; 32]>,
}

/// Durable deterministic projector for finalized reputation source events.
#[derive(Debug)]
pub struct ReputationIngestService {
    policy: ReputationIngestPolicyV1,
    policy_digest: [u8; 32],
    store: AtomicCheckpointStore,
    state: Mutex<RuntimeState>,
    durability_poisoned: AtomicBool,
    metrics: ReputationIngestMetrics,
}

impl ReputationIngestService {
    /// Open the durable projector and reconcile a crash-staged batch.
    ///
    /// # Errors
    ///
    /// Returns a policy, persistence, canonical-checkpoint, or deterministic
    /// projection error. A durable pending batch is applied before this method
    /// returns, so callers never observe an un-reconciled queue.
    pub fn open(
        root: &Path,
        policy: ReputationIngestPolicyV1,
    ) -> Result<Self, ReputationIngestError> {
        policy.validate()?;
        let policy_digest = policy.canonical_digest()?;
        let store = AtomicCheckpointStore::new(
            root,
            REPUTATION_INGEST_CHECKPOINT_FILE_NAME_V1,
            REPUTATION_INGEST_LOCK_FILE_NAME_V1,
            policy.checkpoint_max_bytes,
        )?;
        let (bytes, fingerprint) = store.load_bytes()?;
        let checkpoint = match bytes {
            Some(bytes) => decode_checkpoint(&bytes, &policy, policy_digest)?,
            None => ReputationIngestCheckpointV1::empty(policy_digest),
        };
        let service = Self {
            policy,
            policy_digest,
            store,
            state: Mutex::new(RuntimeState {
                checkpoint,
                fingerprint,
            }),
            durability_poisoned: AtomicBool::new(false),
            metrics: ReputationIngestMetrics::default(),
        };
        service.reconcile_pending_on_open()?;
        Ok(service)
    }

    /// Ingest typed pages, durably stage them, and atomically acknowledge their
    /// deterministic projection.
    ///
    /// # Errors
    ///
    /// Returns a typed failure for malformed pages, source gaps/reordering,
    /// replay equivocation, finalized forks, missing reserve-stage resolution,
    /// capacity overflow, or persistence failure.
    pub fn ingest_finalized_batch(
        &self,
        batch: ReputationFinalizedBatchV1,
    ) -> Result<ReputationIngestOutcomeV1, ReputationIngestError> {
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationIngestError::CheckpointDurabilityUncertain);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationIngestError::RuntimePoisoned)?;
        if state.checkpoint.pending.is_some() {
            self.reconcile_pending_locked(&mut state, false)?;
        }
        let prepared = prepare_pending_batch(&state.checkpoint, &batch, &self.policy)
            .inspect_err(|error| self.record_rejection(*error))?;
        let exact_replays = prepared.exact_replays;
        if prepared.pending.events.is_empty()
            && prepared.pending.finality_updates.is_empty()
            && prepared.pending.reserve_stages.is_empty()
        {
            saturating_add(&self.metrics.exact_replays, u64::from(exact_replays));
            return Ok(ReputationIngestOutcomeV1::ExactReplay);
        }
        let events = u32::try_from(prepared.pending.events.len())
            .map_err(|_| ReputationIngestError::CapacityExceeded)?;
        let mut preflight = state.checkpoint.clone();
        preflight.pending = Some(prepared.pending.clone());
        apply_pending_batch(&mut preflight, &self.policy)
            .inspect_err(|error| self.record_rejection(*error))?;
        ensure_checkpoint_size(&preflight, &self.policy)
            .inspect_err(|error| self.record_rejection(*error))?;
        let mut staged = state.checkpoint.clone();
        staged.pending = Some(prepared.pending);
        let fingerprint = self.commit_checkpoint(&staged, state.fingerprint)?;
        state.checkpoint = staged;
        state.fingerprint = Some(fingerprint);
        self.reconcile_pending_locked(&mut state, false)?;

        saturating_increment(&self.metrics.batches_applied);
        saturating_add(&self.metrics.events_applied, u64::from(events));
        saturating_add(&self.metrics.exact_replays, u64::from(exact_replays));
        if events == 0 {
            saturating_increment(&self.metrics.finality_only_batches);
        }
        Ok(ReputationIngestOutcomeV1::Applied { events })
    }

    /// Return canonical unsigned snapshot material when every required native
    /// feed is complete through the exact release target.
    ///
    /// No signature or key operation occurs in this process.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed source-completeness error while any governed feed
    /// is behind the exact release target.
    pub fn unsigned_signing_material(
        &self,
    ) -> Result<ReputationUnsignedSigningMaterialV1, ReputationIngestError> {
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationIngestError::CheckpointDurabilityUncertain);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationIngestError::RuntimePoisoned)?;
        let checkpoint = &state.checkpoint;
        if checkpoint.pending.is_some() {
            saturating_increment(&self.metrics.incomplete_material_rejections);
            return Err(ReputationIngestError::ReconciliationPending);
        }
        let target = checkpoint
            .latest_finalized
            .ok_or(ReputationIngestError::WindowNotFinalized)?;
        if target.height != self.policy.window_end_height {
            saturating_increment(&self.metrics.incomplete_material_rejections);
            return Err(ReputationIngestError::WindowNotFinalized);
        }
        let missing = missing_sources(checkpoint, self.policy.required_sources, target);
        if !missing.is_empty() {
            saturating_increment(&self.metrics.incomplete_material_rejections);
            return Err(ReputationIngestError::MissingRequiredSources);
        }
        build_signing_material(checkpoint, &self.policy, self.policy_digest)
    }

    /// Durably enqueue the exact deterministic material for external threshold signing.
    ///
    /// The V1 policy describes one immutable release window, so its outbox has
    /// exactly one stable delivery sequence. Repeated calls are idempotent.
    ///
    /// # Errors
    ///
    /// Returns a source-completeness, persistence, or durable-state conflict
    /// error. No signing key, signature, or private event payload is accepted.
    pub fn enqueue_unsigned_signing_material(
        &self,
    ) -> Result<ReputationMaterialEnqueueOutcomeV1, ReputationIngestError> {
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationIngestError::CheckpointDurabilityUncertain);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationIngestError::RuntimePoisoned)?;
        if state.checkpoint.pending.is_some() {
            self.reconcile_pending_locked(&mut state, false)?;
        }
        let material = build_signing_material(&state.checkpoint, &self.policy, self.policy_digest)?;
        let material_digest = unsigned_material_digest(&material)?;
        if let Some(acknowledgement) = state.checkpoint.material_acknowledgement {
            if acknowledgement.sequence == 1 && acknowledgement.material_digest == material_digest {
                saturating_increment(&self.metrics.material_exact_replays);
                return Ok(ReputationMaterialEnqueueOutcomeV1::AlreadyAcknowledged {
                    sequence: acknowledgement.sequence,
                });
            }
            return Err(ReputationIngestError::MaterialOutboxConflict);
        }
        if let Some(entry) = &state.checkpoint.material_outbox {
            if entry.sequence == 1 && entry.material_digest == material_digest {
                saturating_increment(&self.metrics.material_exact_replays);
                return Ok(ReputationMaterialEnqueueOutcomeV1::ExactReplay {
                    sequence: entry.sequence,
                    state: entry.state,
                });
            }
            return Err(ReputationIngestError::MaterialOutboxConflict);
        }

        let mut updated = state.checkpoint.clone();
        updated.material_outbox = Some(ReputationUnsignedMaterialOutboxEntryV1 {
            version: REPUTATION_UNSIGNED_MATERIAL_DELIVERY_VERSION_V1,
            sequence: 1,
            material_digest,
            failed_attempts: 0,
            state: ReputationUnsignedMaterialDeliveryStateV1::Pending,
            failure_receipts: Vec::new(),
        });
        ensure_checkpoint_size(&updated, &self.policy)?;
        let fingerprint = self.commit_checkpoint(&updated, state.fingerprint)?;
        state.checkpoint = updated;
        state.fingerprint = Some(fingerprint);
        saturating_increment(&self.metrics.material_enqueued);
        Ok(ReputationMaterialEnqueueOutcomeV1::Enqueued { sequence: 1 })
    }

    /// Return the durable unsigned-material delivery, including dead letters.
    ///
    /// # Errors
    ///
    /// Returns a durability or runtime-lock error.
    pub fn unsigned_material_delivery(
        &self,
    ) -> Result<Option<ReputationUnsignedMaterialDeliveryV1>, ReputationIngestError> {
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationIngestError::CheckpointDurabilityUncertain);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationIngestError::RuntimePoisoned)?;
        let Some(entry) = &state.checkpoint.material_outbox else {
            return Ok(None);
        };
        let material = build_signing_material(&state.checkpoint, &self.policy, self.policy_digest)?;
        if unsigned_material_digest(&material)? != entry.material_digest {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        Ok(Some(ReputationUnsignedMaterialDeliveryV1 {
            version: entry.version,
            sequence: entry.sequence,
            material_digest: entry.material_digest,
            material,
            failed_attempts: entry.failed_attempts,
            state: entry.state,
        }))
    }

    /// Return the payload-free durable acknowledgement, when present.
    ///
    /// # Errors
    ///
    /// Returns a durability or runtime-lock error.
    pub fn unsigned_material_acknowledgement(
        &self,
    ) -> Result<Option<ReputationUnsignedMaterialAcknowledgementV1>, ReputationIngestError> {
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationIngestError::CheckpointDurabilityUncertain);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationIngestError::RuntimePoisoned)?;
        Ok(state.checkpoint.material_acknowledgement)
    }

    /// Durably record one failed external delivery attempt.
    ///
    /// `failure_receipt` is an opaque, payload-free idempotency digest created
    /// by the external worker. Replaying it does not consume the retry budget.
    ///
    /// # Errors
    ///
    /// Returns an identity, dead-letter, persistence, or runtime error.
    pub fn record_unsigned_material_delivery_failure(
        &self,
        sequence: u64,
        material_digest: [u8; 32],
        failure_receipt: [u8; 32],
    ) -> Result<ReputationMaterialFailureOutcomeV1, ReputationIngestError> {
        if sequence == 0 || material_digest == [0; 32] || failure_receipt == [0; 32] {
            return Err(ReputationIngestError::InvalidDeliveryReceipt);
        }
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationIngestError::CheckpointDurabilityUncertain);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationIngestError::RuntimePoisoned)?;
        let entry = state
            .checkpoint
            .material_outbox
            .as_ref()
            .ok_or(ReputationIngestError::MaterialDeliveryMismatch)?;
        if entry.sequence != sequence || entry.material_digest != material_digest {
            return Err(ReputationIngestError::MaterialDeliveryMismatch);
        }
        if entry.failure_receipts.contains(&failure_receipt) {
            saturating_increment(&self.metrics.material_exact_replays);
            return Ok(ReputationMaterialFailureOutcomeV1::ExactReplay {
                failed_attempts: entry.failed_attempts,
                state: entry.state,
            });
        }
        if entry.state == ReputationUnsignedMaterialDeliveryStateV1::DeadLetter {
            return Err(ReputationIngestError::MaterialDeliveryDeadLettered);
        }

        let mut updated = state.checkpoint.clone();
        let updated_entry = updated
            .material_outbox
            .as_mut()
            .ok_or(ReputationIngestError::MaterialDeliveryMismatch)?;
        updated_entry.failure_receipts.push(failure_receipt);
        let failed_attempts = u32::try_from(updated_entry.failure_receipts.len())
            .map_err(|_| ReputationIngestError::CapacityExceeded)?;
        updated_entry.failed_attempts = failed_attempts;
        let state_after = if failed_attempts == self.policy.max_material_delivery_failures {
            updated_entry.state = ReputationUnsignedMaterialDeliveryStateV1::DeadLetter;
            ReputationMaterialFailureOutcomeV1::DeadLettered { failed_attempts }
        } else {
            ReputationMaterialFailureOutcomeV1::RetryPending {
                failed_attempts,
                remaining_attempts: self
                    .policy
                    .max_material_delivery_failures
                    .checked_sub(failed_attempts)
                    .ok_or(ReputationIngestError::ArithmeticOverflow)?,
            }
        };
        ensure_checkpoint_size(&updated, &self.policy)?;
        let fingerprint = self.commit_checkpoint(&updated, state.fingerprint)?;
        state.checkpoint = updated;
        state.fingerprint = Some(fingerprint);
        saturating_increment(&self.metrics.material_delivery_failures);
        if matches!(
            state_after,
            ReputationMaterialFailureOutcomeV1::DeadLettered { .. }
        ) {
            saturating_increment(&self.metrics.material_dead_letters);
        }
        Ok(state_after)
    }

    /// Durably acknowledge an externally signed result and remove its outbox item.
    ///
    /// The signed envelope must be structurally canonical and exactly bind the
    /// projected snapshot, scoring evidence, trust-policy digest, and signing
    /// digest. `trust_policy` contains public verification material only; its
    /// canonical digest must equal the governed ingest anchor. Full quorum,
    /// revocation, signature, freshness, and future-skew verification uses the
    /// authoritative finalized timestamp from the same locked checkpoint used
    /// for exact material binding, and succeeds before the service retains the
    /// canonical result digest.
    ///
    /// # Errors
    ///
    /// Returns an identity, receipt, persistence, or runtime error.
    pub fn acknowledge_unsigned_material(
        &self,
        sequence: u64,
        material_digest: [u8; 32],
        signed_result: &SignedReputationSnapshotV1,
        trust_policy: &ReputationSnapshotTrustPolicyV1,
    ) -> Result<ReputationMaterialAcknowledgementOutcomeV1, ReputationIngestError> {
        if sequence == 0 || material_digest == [0; 32] {
            return Err(ReputationIngestError::InvalidDeliveryReceipt);
        }
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationIngestError::CheckpointDurabilityUncertain);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationIngestError::RuntimePoisoned)?;
        let verified_at_unix =
            checked_unix_millis_to_seconds(state.checkpoint.latest_finalized_at_unix_ms)
                .ok_or(ReputationIngestError::InvalidCheckpoint)?;
        let trust_policy_digest = trust_policy
            .canonical_digest()
            .map_err(|_| ReputationIngestError::InvalidDeliveryReceipt)?;
        if trust_policy_digest != self.policy.snapshot_trust_policy_digest
            || signed_result.policy_digest != trust_policy_digest
        {
            return Err(ReputationIngestError::MaterialDeliveryMismatch);
        }
        signed_result
            .verify(trust_policy, verified_at_unix)
            .map_err(|_| ReputationIngestError::InvalidDeliveryReceipt)?;
        let material = build_signing_material(&state.checkpoint, &self.policy, self.policy_digest)?;
        if unsigned_material_digest(&material)? != material_digest
            || signed_result.policy_digest != material.snapshot_trust_policy_digest
            || signed_result.snapshot != material.snapshot
            || signed_result.scoring_evidence_digest != material.scoring_evidence_digest
            || signed_result.scoring_evidence != material.scoring_evidence
            || signed_result
                .signing_digest()
                .map_err(|_| ReputationIngestError::InvalidDeliveryReceipt)?
                != material.snapshot_signing_digest
        {
            return Err(ReputationIngestError::MaterialDeliveryMismatch);
        }
        let signed_result_digest = canonical_signed_result_digest(signed_result)?;
        let acknowledgement = ReputationUnsignedMaterialAcknowledgementV1 {
            version: REPUTATION_UNSIGNED_MATERIAL_ACKNOWLEDGEMENT_VERSION_V1,
            sequence,
            material_digest,
            trust_policy_digest,
            verified_at_unix,
            signed_result_digest,
        };
        if let Some(existing) = state.checkpoint.material_acknowledgement {
            if existing == acknowledgement {
                saturating_increment(&self.metrics.material_exact_replays);
                return Ok(ReputationMaterialAcknowledgementOutcomeV1::ExactReplay);
            }
            return Err(ReputationIngestError::MaterialDeliveryMismatch);
        }
        let entry = state
            .checkpoint
            .material_outbox
            .as_ref()
            .ok_or(ReputationIngestError::MaterialDeliveryMismatch)?;
        if entry.sequence != sequence || entry.material_digest != material_digest {
            return Err(ReputationIngestError::MaterialDeliveryMismatch);
        }

        let mut updated = state.checkpoint.clone();
        updated.material_outbox = None;
        updated.material_acknowledgement = Some(acknowledgement);
        ensure_checkpoint_size(&updated, &self.policy)?;
        let fingerprint = self.commit_checkpoint(&updated, state.fingerprint)?;
        state.checkpoint = updated;
        state.fingerprint = Some(fingerprint);
        saturating_increment(&self.metrics.material_acknowledgements);
        Ok(ReputationMaterialAcknowledgementOutcomeV1::Acknowledged)
    }

    /// Return restart-safe cursors for the five physical committed feeds.
    ///
    /// The PoR, dispute, and stream-token semantic sources intentionally share
    /// one `Journal` row so a worker cannot resume those interleaved records
    /// from divergent positions.
    ///
    /// # Errors
    ///
    /// Returns a durability or runtime-lock error.
    pub fn committed_feed_cursors(
        &self,
    ) -> Result<Vec<ReputationCommittedFeedCursorV1>, ReputationIngestError> {
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationIngestError::CheckpointDurabilityUncertain);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationIngestError::RuntimePoisoned)?;
        Ok(ALL_COMMITTED_FEEDS
            .into_iter()
            .map(|feed| {
                let progress = checkpoint_progress_for_feed(&state.checkpoint, feed);
                ReputationCommittedFeedCursorV1 {
                    feed,
                    after: progress.last_event,
                    observed_through: progress.observed_through,
                    observed_at_unix_ms: progress.observed_at_unix_ms,
                }
            })
            .collect())
    }

    /// Return payload-free service status.
    ///
    /// # Errors
    ///
    /// Returns a runtime-lock or resource-conversion error.
    pub fn status(&self) -> Result<ReputationIngestStatusV1, ReputationIngestError> {
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationIngestError::CheckpointDurabilityUncertain);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationIngestError::RuntimePoisoned)?;
        let target = state.checkpoint.latest_finalized;
        let missing = target.map_or(self.policy.required_sources, |target| {
            missing_sources(&state.checkpoint, self.policy.required_sources, target)
        });
        Ok(ReputationIngestStatusV1 {
            policy_digest: self.policy_digest,
            latest_finalized: target,
            latest_finalized_at_unix_ms: state.checkpoint.latest_finalized_at_unix_ms,
            source_finality: state
                .checkpoint
                .source_progress
                .iter()
                .map(|progress| (progress.source, progress.observed_through))
                .collect(),
            missing_sources: missing,
            providers: u32::try_from(state.checkpoint.providers.len())
                .map_err(|_| ReputationIngestError::CapacityExceeded)?,
            pending_events: state.checkpoint.pending.as_ref().map_or(Ok(0), |pending| {
                u32::try_from(pending.events.len())
                    .map_err(|_| ReputationIngestError::CapacityExceeded)
            })?,
            material_outbox_state: state
                .checkpoint
                .material_outbox
                .as_ref()
                .map(|entry| entry.state),
            material_delivery_failures: state.checkpoint.material_outbox.as_ref().map_or(
                Ok(0),
                |entry| {
                    u32::try_from(entry.failure_receipts.len())
                        .map_err(|_| ReputationIngestError::CapacityExceeded)
                },
            )?,
            material_acknowledged: state.checkpoint.material_acknowledgement.is_some(),
        })
    }

    /// Return payload-free counters.
    #[must_use]
    pub fn metrics(&self) -> ReputationIngestMetricsSnapshot {
        self.metrics.snapshot()
    }

    /// Return the exact canonical durable state bytes for replica parity checks.
    ///
    /// # Errors
    ///
    /// Returns a runtime-lock or canonical encoding error.
    pub fn canonical_checkpoint_bytes(&self) -> Result<Vec<u8>, ReputationIngestError> {
        if self.durability_poisoned.load(Ordering::Acquire) {
            return Err(ReputationIngestError::CheckpointDurabilityUncertain);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| ReputationIngestError::RuntimePoisoned)?;
        norito::to_bytes(&state.checkpoint).map_err(|_| ReputationIngestError::CanonicalEncoding)
    }

    fn reconcile_pending_on_open(&self) -> Result<(), ReputationIngestError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReputationIngestError::RuntimePoisoned)?;
        self.reconcile_pending_locked(&mut state, true)
    }

    fn reconcile_pending_locked(
        &self,
        state: &mut RuntimeState,
        opening: bool,
    ) -> Result<(), ReputationIngestError> {
        if state.checkpoint.pending.is_none() {
            return Ok(());
        }
        let mut applied = state.checkpoint.clone();
        apply_pending_batch(&mut applied, &self.policy)?;
        let fingerprint = self.commit_checkpoint(&applied, state.fingerprint)?;
        state.checkpoint = applied;
        state.fingerprint = Some(fingerprint);
        if opening {
            saturating_increment(&self.metrics.restart_reconciliations);
        }
        Ok(())
    }

    fn commit_checkpoint(
        &self,
        checkpoint: &ReputationIngestCheckpointV1,
        fingerprint: Option<[u8; 32]>,
    ) -> Result<[u8; 32], ReputationIngestError> {
        validate_checkpoint(checkpoint, &self.policy, self.policy_digest)?;
        let bytes =
            norito::to_bytes(checkpoint).map_err(|_| ReputationIngestError::CanonicalEncoding)?;
        let result = self.store.commit_bytes(&bytes, fingerprint);
        match result {
            Ok(fingerprint) => Ok(fingerprint),
            Err(error) => {
                saturating_increment(&self.metrics.checkpoint_failures);
                if matches!(
                    error,
                    CheckpointStoreError::Io
                        | CheckpointStoreError::Stale
                        | CheckpointStoreError::DurabilityUncertain
                        | CheckpointStoreError::RuntimePoisoned
                ) {
                    self.durability_poisoned.store(true, Ordering::Release);
                }
                Err(error.into())
            }
        }
    }

    fn record_rejection(&self, error: ReputationIngestError) {
        match error {
            ReputationIngestError::EventReordered => {
                saturating_increment(&self.metrics.reordered_rejections);
            }
            ReputationIngestError::EventGap => {
                saturating_increment(&self.metrics.gap_rejections);
            }
            ReputationIngestError::EventEquivocation
            | ReputationIngestError::ReplayOutsideRetention => {
                saturating_increment(&self.metrics.equivocation_rejections);
            }
            ReputationIngestError::FinalizedFork => {
                saturating_increment(&self.metrics.fork_rejections);
            }
            ReputationIngestError::CapacityExceeded | ReputationIngestError::CheckpointTooLarge => {
                saturating_increment(&self.metrics.bound_rejections);
            }
            _ => {}
        }
    }
}

#[derive(Debug)]
struct PreparedPendingBatch {
    pending: PendingBatchV1,
    exact_replays: u32,
}

struct PrepareContext<'a> {
    checkpoint: &'a ReputationIngestCheckpointV1,
    policy: &'a ReputationIngestPolicyV1,
    target: ReputationFinalizedIdentityV1,
    finalized_at_unix_ms: u64,
    working_last: BTreeMap<ReputationCommittedFeedV1, Option<ReputationCommittedEventIdentityV1>>,
    receipt_index: BTreeMap<(ReputationCommittedFeedV1, u64), EventReceiptV1>,
    block_hashes: BTreeMap<u64, [u8; 32]>,
    events: Vec<PendingEventV1>,
    presented_events: usize,
    encoded_page_bytes: usize,
    exact_replays: u32,
}

impl<'a> PrepareContext<'a> {
    fn new(
        checkpoint: &'a ReputationIngestCheckpointV1,
        policy: &'a ReputationIngestPolicyV1,
        target: ReputationFinalizedIdentityV1,
        finalized_at_unix_ms: u64,
    ) -> Self {
        let working_last = ALL_COMMITTED_FEEDS
            .into_iter()
            .map(|feed| {
                (
                    feed,
                    checkpoint_progress_for_feed(checkpoint, feed).last_event,
                )
            })
            .collect();
        let receipt_index = checkpoint
            .receipts
            .iter()
            .map(|receipt| ((receipt.feed, receipt.identity.sequence), *receipt))
            .collect();
        let mut block_hashes = BTreeMap::new();
        for identity in checkpoint
            .receipts
            .iter()
            .map(|receipt| receipt.identity)
            .chain(
                checkpoint
                    .source_progress
                    .iter()
                    .filter_map(|progress| progress.last_event),
            )
            .chain(checkpoint.source_progress.iter().filter_map(|progress| {
                progress
                    .observed_through
                    .map(finalized_identity_as_event_identity)
            }))
            .chain(
                checkpoint
                    .latest_finalized
                    .map(finalized_identity_as_event_identity),
            )
        {
            block_hashes.insert(identity.block_height, identity.block_hash);
        }
        Self {
            checkpoint,
            policy,
            target,
            finalized_at_unix_ms,
            working_last,
            receipt_index,
            block_hashes,
            events: Vec::new(),
            presented_events: 0,
            encoded_page_bytes: 0,
            exact_replays: 0,
        }
    }

    fn last(&self, feed: ReputationCommittedFeedV1) -> Option<ReputationCommittedEventIdentityV1> {
        self.working_last.get(&feed).copied().flatten()
    }

    fn accept_encoded_page<T: norito::NoritoSerialize>(
        &mut self,
        page: &T,
        max_page_bytes: usize,
    ) -> Result<(), ReputationIngestError> {
        let encoded_bytes = norito::core::encoded_frame_len(page)
            .map_err(|_| ReputationIngestError::CanonicalEncoding)?;
        if encoded_bytes > max_page_bytes {
            return Err(ReputationIngestError::CapacityExceeded);
        }
        self.encoded_page_bytes = self
            .encoded_page_bytes
            .checked_add(encoded_bytes)
            .ok_or(ReputationIngestError::CapacityExceeded)?;
        if u64::try_from(self.encoded_page_bytes).unwrap_or(u64::MAX)
            > self.policy.checkpoint_max_bytes
        {
            return Err(ReputationIngestError::CapacityExceeded);
        }
        Ok(())
    }

    fn accept_event<T: norito::NoritoSerialize>(
        &mut self,
        feed: ReputationCommittedFeedV1,
        identity: ReputationCommittedEventIdentityV1,
        occurred_at_unix_ms: u64,
        signal: ReputationSignalV1,
        event: &T,
    ) -> Result<(), ReputationIngestError> {
        identity.validate()?;
        if occurred_at_unix_ms == 0
            || occurred_at_unix_ms > self.finalized_at_unix_ms
            || identity.block_height > self.target.height
            || identity.block_height > self.policy.window_end_height
        {
            return Err(ReputationIngestError::InvalidEventIdentity);
        }
        if identity.block_height == self.target.height
            && identity.block_hash != self.target.block_hash
        {
            return Err(ReputationIngestError::FinalizedFork);
        }
        if signal
            .provider_id()
            .is_some_and(|provider| provider.as_bytes() == &[0; 32])
        {
            return Err(ReputationIngestError::InvalidProvider);
        }
        if !signal_is_well_formed(signal) {
            return Err(ReputationIngestError::InvalidPage);
        }
        let presented_limit = usize::try_from(self.policy.max_pending_events)
            .map_err(|_| ReputationIngestError::CapacityExceeded)?;
        if self.presented_events >= presented_limit {
            return Err(ReputationIngestError::CapacityExceeded);
        }
        self.presented_events = self
            .presented_events
            .checked_add(1)
            .ok_or(ReputationIngestError::CapacityExceeded)?;
        self.reject_known_block_fork(identity)?;
        let event_digest = hash_canonical(feed_event_domain(feed), event)?;
        let last = self.last(feed);

        if last.is_some_and(|cursor| identity.sequence <= cursor.sequence) {
            let retained = self
                .receipt_index
                .get(&(feed, identity.sequence))
                .ok_or(ReputationIngestError::ReplayOutsideRetention)?;
            if retained.identity != identity || retained.event_digest != event_digest {
                return Err(ReputationIngestError::EventEquivocation);
            }
            self.exact_replays = self
                .exact_replays
                .checked_add(1)
                .ok_or(ReputationIngestError::ArithmeticOverflow)?;
            return Ok(());
        }

        let expected_sequence = last.map_or(Ok(1), |cursor| {
            cursor
                .sequence
                .checked_add(1)
                .ok_or(ReputationIngestError::ArithmeticOverflow)
        })?;
        if identity.sequence != expected_sequence {
            return Err(ReputationIngestError::EventGap);
        }
        if let Some(previous) = last {
            if identity.block_height < previous.block_height
                || (identity.block_height == previous.block_height
                    && (identity.block_hash != previous.block_hash
                        || identity.event_index <= previous.event_index))
            {
                return Err(ReputationIngestError::EventReordered);
            }
        }
        let receipt = EventReceiptV1 {
            feed,
            identity,
            event_digest,
        };
        self.receipt_index
            .insert((feed, identity.sequence), receipt);
        self.block_hashes
            .insert(identity.block_height, identity.block_hash);
        self.events.push(PendingEventV1 {
            feed,
            identity,
            event_digest,
            occurred_at_unix_ms,
            signal,
        });
        self.working_last.insert(feed, Some(identity));
        Ok(())
    }

    fn reject_known_block_fork(
        &self,
        identity: ReputationCommittedEventIdentityV1,
    ) -> Result<(), ReputationIngestError> {
        if self
            .block_hashes
            .get(&identity.block_height)
            .is_some_and(|known| *known != identity.block_hash)
        {
            return Err(ReputationIngestError::FinalizedFork);
        }
        Ok(())
    }
}

fn prepare_pending_batch(
    checkpoint: &ReputationIngestCheckpointV1,
    batch: &ReputationFinalizedBatchV1,
    policy: &ReputationIngestPolicyV1,
) -> Result<PreparedPendingBatch, ReputationIngestError> {
    if batch.network_id != policy.network_id {
        return Err(ReputationIngestError::NetworkIdMismatch);
    }
    if batch.finalized_at_unix_ms == 0 {
        return Err(ReputationIngestError::InvalidFinalizedIdentity);
    }
    let page_count = batch
        .proof_pages
        .len()
        .checked_add(batch.journal_pages.len())
        .and_then(|count| count.checked_add(batch.repair_pages.len()))
        .and_then(|count| count.checked_add(batch.orderbook_pages.len()))
        .and_then(|count| count.checked_add(batch.reserve_pages.len()))
        .and_then(|count| count.checked_add(batch.reserve_provider_pages.len()))
        .ok_or(ReputationIngestError::CapacityExceeded)?;
    if page_count == 0
        || page_count
            > usize::try_from(policy.max_pages_per_batch)
                .map_err(|_| ReputationIngestError::CapacityExceeded)?
    {
        return Err(ReputationIngestError::CapacityExceeded);
    }
    let target = batch_target(batch)?;
    target.validate()?;
    if target.height > policy.window_end_height {
        return Err(ReputationIngestError::WindowNotFinalized);
    }
    validate_global_target(
        checkpoint.latest_finalized,
        checkpoint.latest_finalized_at_unix_ms,
        target,
        batch.finalized_at_unix_ms,
    )?;

    let mut context = PrepareContext::new(checkpoint, policy, target, batch.finalized_at_unix_ms);
    let mut updates = Vec::new();
    if !batch.proof_pages.is_empty() {
        updates.push(prepare_proof_pages(
            &mut context,
            &batch.proof_pages,
            batch.finalized_at_unix_ms,
        )?);
    }
    if !batch.journal_pages.is_empty() {
        updates.extend(prepare_journal_pages(
            &mut context,
            &batch.journal_pages,
            batch.finalized_at_unix_ms,
        )?);
    }
    if !batch.repair_pages.is_empty() {
        updates.push(prepare_repair_pages(
            &mut context,
            &batch.repair_pages,
            batch.finalized_at_unix_ms,
        )?);
    }
    if !batch.orderbook_pages.is_empty() {
        updates.push(prepare_orderbook_pages(
            &mut context,
            &batch.orderbook_pages,
            batch.finalized_at_unix_ms,
        )?);
    }

    let (reserve_update, reserve_stages) = if batch.reserve_pages.is_empty() {
        if !batch.reserve_provider_pages.is_empty() {
            return Err(ReputationIngestError::InvalidPage);
        }
        (None, Vec::new())
    } else {
        let update = prepare_reserve_pages(
            &mut context,
            &batch.reserve_pages,
            batch.finalized_at_unix_ms,
        )?;
        if update.complete {
            if batch.reserve_provider_pages.is_empty() {
                return Err(ReputationIngestError::ReserveStageResolutionMissing);
            }
            let (stages, digest) = prepare_reserve_projection(
                &batch.reserve_provider_pages,
                target,
                batch.finalized_at_unix_ms,
                policy,
                &mut context.encoded_page_bytes,
            )?;
            let existing = checkpoint.progress(ReputationSourceV1::Reserve);
            if existing.observed_through == Some(target)
                && existing
                    .reserve_projection_digest
                    .is_some_and(|retained| retained != digest)
            {
                return Err(ReputationIngestError::EventEquivocation);
            }
            (
                Some(SourceFinalityUpdateV1 {
                    reserve_projection_digest: Some(digest),
                    ..update
                }),
                stages,
            )
        } else {
            if !batch.reserve_provider_pages.is_empty() {
                return Err(ReputationIngestError::InvalidPage);
            }
            (Some(update), Vec::new())
        }
    };
    if let Some(update) = reserve_update {
        updates.push(update);
    }

    context.events.sort_by_key(|event| {
        (
            event.identity.block_height,
            event.identity.event_index,
            event.feed,
            event.identity.sequence,
        )
    });
    updates.sort_by_key(|update| update.source);

    updates.retain(|update| {
        let existing = checkpoint.progress(update.source);
        existing.last_event != update.last_event
            || (update.complete
                && (existing.observed_through != Some(update.target)
                    || existing.observed_at_unix_ms != update.finalized_at_unix_ms
                    || existing.reserve_projection_digest != update.reserve_projection_digest))
    });
    let reserve_stages = canonical_reserve_stage_projection(checkpoint, target, reserve_stages);
    Ok(PreparedPendingBatch {
        exact_replays: context.exact_replays,
        pending: PendingBatchV1 {
            target,
            finalized_at_unix_ms: batch.finalized_at_unix_ms,
            events: context.events,
            finality_updates: updates,
            reserve_stages,
        },
    })
}

fn batch_target(
    batch: &ReputationFinalizedBatchV1,
) -> Result<ReputationFinalizedIdentityV1, ReputationIngestError> {
    let mut target = None;
    for cursor in batch
        .proof_pages
        .iter()
        .map(|page| ReputationFinalizedIdentityV1 {
            height: page.finalized_cursor.height,
            block_hash: page.finalized_cursor.block_hash,
        })
        .chain(
            batch
                .journal_pages
                .iter()
                .map(|page| ReputationFinalizedIdentityV1 {
                    height: page.finalized_cursor.height,
                    block_hash: page.finalized_cursor.block_hash,
                }),
        )
        .chain(
            batch
                .repair_pages
                .iter()
                .map(|page| ReputationFinalizedIdentityV1 {
                    height: page.finalized_cursor.height,
                    block_hash: page.finalized_cursor.block_hash,
                }),
        )
        .chain(
            batch
                .orderbook_pages
                .iter()
                .map(|page| ReputationFinalizedIdentityV1 {
                    height: page.finalized_cursor.height,
                    block_hash: page.finalized_cursor.block_hash,
                }),
        )
        .chain(
            batch
                .reserve_pages
                .iter()
                .map(|page| ReputationFinalizedIdentityV1 {
                    height: page.finalized_cursor.height,
                    block_hash: page.finalized_cursor.block_hash,
                }),
        )
        .chain(
            batch
                .reserve_provider_pages
                .iter()
                .map(|page| ReputationFinalizedIdentityV1 {
                    height: page.finalized_cursor.height,
                    block_hash: page.finalized_cursor.block_hash,
                }),
        )
    {
        if target.is_some_and(|existing| existing != cursor) {
            return Err(ReputationIngestError::PageAnchorMismatch);
        }
        target = Some(cursor);
    }
    target.ok_or(ReputationIngestError::InvalidPage)
}

fn validate_global_target(
    previous: Option<ReputationFinalizedIdentityV1>,
    previous_at_unix_ms: u64,
    next: ReputationFinalizedIdentityV1,
    next_at_unix_ms: u64,
) -> Result<(), ReputationIngestError> {
    let Some(previous) = previous else {
        return Ok(());
    };
    if next.height < previous.height {
        return Err(ReputationIngestError::StaleFinalizedIdentity);
    }
    if next.height == previous.height {
        if next.block_hash != previous.block_hash {
            return Err(ReputationIngestError::FinalizedFork);
        }
        if next_at_unix_ms != previous_at_unix_ms {
            return Err(ReputationIngestError::EventEquivocation);
        }
    } else if next_at_unix_ms < previous_at_unix_ms {
        return Err(ReputationIngestError::EventReordered);
    }
    Ok(())
}

fn prepare_proof_pages(
    context: &mut PrepareContext<'_>,
    pages: &[ProofOutcomeFinalizedEventPageV1],
    finalized_at_unix_ms: u64,
) -> Result<SourceFinalityUpdateV1, ReputationIngestError> {
    let mut previous_page_event = None;
    validate_event_page_chain(
        pages.len(),
        pages.iter().map(|page| {
            (
                page.has_more,
                page.next_after
                    .map(|cursor| ReputationCommittedEventIdentityV1 {
                        sequence: cursor.sequence,
                        block_height: cursor.block_height,
                        block_hash: cursor.block_hash,
                        event_index: cursor.event_index,
                    }),
                page.events.last().map(proof_event_identity),
            )
        }),
    )?;
    for page in pages {
        validate_page_item_count(page.events.len(), PROOF_OUTCOME_QUERY_MAX_ITEMS_V1)?;
        context.accept_encoded_page(page, PROOF_OUTCOME_QUERY_MAX_EVENT_PAGE_BYTES_V1)?;
        assert_page_target(
            context.target,
            ReputationFinalizedIdentityV1 {
                height: page.finalized_cursor.height,
                block_hash: page.finalized_cursor.block_hash,
            },
        )?;
        for event in &page.events {
            let identity = proof_event_identity(event);
            validate_page_event_order(&mut previous_page_event, identity)?;
        }
        for event in &page.events {
            let identity = proof_event_identity(event);
            let signal = match &event.outcome.projection {
                ProofOutcomeProjectionV1::Pdp(projection) => ReputationSignalV1::Pdp {
                    provider_id: event.outcome.provider_id,
                    success: projection.status == PdpOutcomeStatusV1::Accepted,
                },
                ProofOutcomeProjectionV1::Potr(projection) => {
                    let counts_for_provider = !matches!(
                        projection.status,
                        PotrOutcomeStatusV1::GatewayError | PotrOutcomeStatusV1::ClientCancelled
                    );
                    ReputationSignalV1::Potr {
                        provider_id: event.outcome.provider_id,
                        counts_for_provider,
                        success: projection.status == PotrOutcomeStatusV1::Success,
                        latency_healthy: projection.status == PotrOutcomeStatusV1::Success
                            && projection.latency_ms <= projection.deadline_ms,
                    }
                }
            };
            context.accept_event(
                ReputationCommittedFeedV1::Proof,
                identity,
                event.outcome.committed_at_unix_ms,
                signal,
                event,
            )?;
        }
    }
    source_update(
        context,
        ReputationSourceV1::Proof,
        finalized_at_unix_ms,
        pages.last().is_some_and(|page| !page.has_more),
    )
}

fn prepare_journal_pages(
    context: &mut PrepareContext<'_>,
    pages: &[ReputationJournalFinalizedEventPageV1],
    finalized_at_unix_ms: u64,
) -> Result<[SourceFinalityUpdateV1; 3], ReputationIngestError> {
    let mut previous_page_event = None;
    validate_event_page_chain(
        pages.len(),
        pages.iter().map(|page| {
            (
                page.has_more,
                page.next_after
                    .map(|cursor| ReputationCommittedEventIdentityV1 {
                        sequence: cursor.sequence,
                        block_height: cursor.block_height,
                        block_hash: cursor.block_hash,
                        event_index: cursor.event_index,
                    }),
                page.events.last().map(journal_event_identity),
            )
        }),
    )?;
    for page in pages {
        validate_page_item_count(page.events.len(), REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1)?;
        context.accept_encoded_page(page, REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1)?;
        page.validate().map_err(map_journal_page_error)?;
        assert_page_target(
            context.target,
            ReputationFinalizedIdentityV1 {
                height: page.finalized_cursor.height,
                block_hash: page.finalized_cursor.block_hash,
            },
        )?;
        if page.finalized_cursor.finalized_at_unix_ms != finalized_at_unix_ms {
            return Err(ReputationIngestError::PageAnchorMismatch);
        }
        for event in &page.events {
            let identity = journal_event_identity(event);
            validate_page_event_order(&mut previous_page_event, identity)?;
            let signal = match &event.entry.payload {
                ReputationJournalPayloadV1::PorTerminal(outcome) => match outcome.status {
                    PorTerminalStatusV1::Excluded(_) => ReputationSignalV1::Noop,
                    status => ReputationSignalV1::Por {
                        provider_id: event.entry.provider_id,
                        counts_for_provider: status.counts_for_provider(),
                        success: status.is_success(),
                    },
                },
                ReputationJournalPayloadV1::ProviderDispute(dispute) => {
                    let transition = match &dispute.status {
                        ProviderDisputeStatusV1::Opened => ReputationDisputeSignalV1::Opened,
                        ProviderDisputeStatusV1::Resolved(resolution) => {
                            ReputationDisputeSignalV1::Resolved {
                                upheld: resolution.outcome == CapacityDisputeOutcome::Upheld,
                            }
                        }
                    };
                    ReputationSignalV1::Dispute {
                        provider_id: event.entry.provider_id,
                        transition,
                    }
                }
                ReputationJournalPayloadV1::StreamTokenValidation(outcome) => {
                    match outcome.status {
                        StreamTokenValidationStatusV1::Excluded(_) => ReputationSignalV1::Noop,
                        status => ReputationSignalV1::Token {
                            provider_id: event.entry.provider_id,
                            counts_for_provider: status.counts_for_provider(),
                            violation: status.is_violation(),
                        },
                    }
                }
            };
            context.accept_event(
                ReputationCommittedFeedV1::Journal,
                identity,
                event.recorded_at_unix_ms,
                signal,
                event,
            )?;
        }
    }
    let complete = pages.last().is_some_and(|page| !page.has_more);
    Ok([
        source_update(
            context,
            ReputationSourceV1::Por,
            finalized_at_unix_ms,
            complete,
        )?,
        source_update(
            context,
            ReputationSourceV1::Dispute,
            finalized_at_unix_ms,
            complete,
        )?,
        source_update(
            context,
            ReputationSourceV1::Token,
            finalized_at_unix_ms,
            complete,
        )?,
    ])
}

const fn map_journal_page_error(error: ReputationJournalValidationError) -> ReputationIngestError {
    match error {
        ReputationJournalValidationError::EventSequenceGap => ReputationIngestError::EventGap,
        ReputationJournalValidationError::EventSequenceReordered
        | ReputationJournalValidationError::BlockEventReordered
        | ReputationJournalValidationError::EventTimestampReordered => {
            ReputationIngestError::EventReordered
        }
        ReputationJournalValidationError::BlockHashMismatch
        | ReputationJournalValidationError::FinalizedBlockHashMismatch => {
            ReputationIngestError::FinalizedFork
        }
        ReputationJournalValidationError::EventSequenceOverflow => {
            ReputationIngestError::ArithmeticOverflow
        }
        ReputationJournalValidationError::TooManyPageItems { .. }
        | ReputationJournalValidationError::EncodedPageTooLarge { .. } => {
            ReputationIngestError::CapacityExceeded
        }
        _ => ReputationIngestError::InvalidPage,
    }
}

fn prepare_repair_pages(
    context: &mut PrepareContext<'_>,
    pages: &[RepairFinalizedEventPageV1],
    finalized_at_unix_ms: u64,
) -> Result<SourceFinalityUpdateV1, ReputationIngestError> {
    let mut previous_page_event = None;
    validate_event_page_chain(
        pages.len(),
        pages.iter().map(|page| {
            (
                page.has_more,
                page.next_after
                    .map(|cursor| ReputationCommittedEventIdentityV1 {
                        sequence: cursor.sequence,
                        block_height: cursor.block_height,
                        block_hash: cursor.block_hash,
                        event_index: cursor.event_index,
                    }),
                page.events.last().map(repair_event_identity),
            )
        }),
    )?;
    for page in pages {
        validate_page_item_count(
            page.events.len(),
            usize::try_from(REPAIR_QUERY_MAX_ITEMS_V1)
                .map_err(|_| ReputationIngestError::CapacityExceeded)?,
        )?;
        context.accept_encoded_page(page, REPAIR_QUERY_MAX_EVENT_PAGE_BYTES_V1)?;
        assert_page_target(
            context.target,
            ReputationFinalizedIdentityV1 {
                height: page.finalized_cursor.height,
                block_hash: page.finalized_cursor.block_hash,
            },
        )?;
        for event in &page.events {
            let identity = repair_event_identity(event);
            validate_page_event_order(&mut previous_page_event, identity)?;
            let (terminal, breach, slashing) = match event.event.kind() {
                SorafsRepairLedgerEventKind::Completed => (true, false, false),
                SorafsRepairLedgerEventKind::Failed => (true, true, false),
                SorafsRepairLedgerEventKind::Escalated => (true, true, true),
                SorafsRepairLedgerEventKind::TaskSubmitted
                | SorafsRepairLedgerEventKind::LeaseClaimed
                | SorafsRepairLedgerEventKind::LeaseRenewed
                | SorafsRepairLedgerEventKind::Appealed => (false, false, false),
            };
            context.accept_event(
                ReputationCommittedFeedV1::Repair,
                identity,
                *event.event.occurred_at_unix_ms(),
                ReputationSignalV1::Repair {
                    provider_id: *event.event.provider_id(),
                    terminal,
                    breach,
                    slashing,
                },
                event,
            )?;
        }
    }
    source_update(
        context,
        ReputationSourceV1::Repair,
        finalized_at_unix_ms,
        pages.last().is_some_and(|page| !page.has_more),
    )
}

fn prepare_orderbook_pages(
    context: &mut PrepareContext<'_>,
    pages: &[OrderbookFinalizedEventPageV1],
    finalized_at_unix_ms: u64,
) -> Result<SourceFinalityUpdateV1, ReputationIngestError> {
    let mut previous_page_event = None;
    validate_event_page_chain(
        pages.len(),
        pages.iter().map(|page| {
            (
                page.has_more,
                page.next_after
                    .map(|cursor| ReputationCommittedEventIdentityV1 {
                        sequence: cursor.sequence,
                        block_height: cursor.block_height,
                        block_hash: cursor.block_hash,
                        event_index: cursor.event_index,
                    }),
                page.events.last().map(orderbook_event_identity),
            )
        }),
    )?;
    for page in pages {
        validate_page_item_count(
            page.events.len(),
            usize::try_from(ORDERBOOK_QUERY_MAX_ITEMS_V1)
                .map_err(|_| ReputationIngestError::CapacityExceeded)?,
        )?;
        context.accept_encoded_page(page, ORDERBOOK_QUERY_MAX_EVENT_PAGE_BYTES_V1)?;
        assert_page_target(
            context.target,
            ReputationFinalizedIdentityV1 {
                height: page.finalized_cursor.height,
                block_hash: page.finalized_cursor.block_hash,
            },
        )?;
        for event in &page.events {
            let identity = orderbook_event_identity(event);
            validate_page_event_order(&mut previous_page_event, identity)?;
            let signal = (*event.event.provider_id())
                .map_or(ReputationSignalV1::Noop, |provider_id| {
                    ReputationSignalV1::ProviderObserved { provider_id }
                });
            context.accept_event(
                ReputationCommittedFeedV1::Orderbook,
                identity,
                *event.event.occurred_at_unix_ms(),
                signal,
                event,
            )?;
        }
    }
    source_update(
        context,
        ReputationSourceV1::Orderbook,
        finalized_at_unix_ms,
        pages.last().is_some_and(|page| !page.has_more),
    )
}

fn prepare_reserve_pages(
    context: &mut PrepareContext<'_>,
    pages: &[ReserveFinalizedEventPageV1],
    finalized_at_unix_ms: u64,
) -> Result<SourceFinalityUpdateV1, ReputationIngestError> {
    let mut previous_page_event = None;
    validate_event_page_chain(
        pages.len(),
        pages.iter().map(|page| {
            (
                page.has_more,
                page.next_after
                    .map(|cursor| ReputationCommittedEventIdentityV1 {
                        sequence: cursor.sequence,
                        block_height: cursor.block_height,
                        block_hash: cursor.block_hash,
                        event_index: cursor.event_index,
                    }),
                page.events.last().map(reserve_event_identity),
            )
        }),
    )?;
    for page in pages {
        validate_page_item_count(
            page.events.len(),
            usize::try_from(RESERVE_QUERY_MAX_ITEMS_V1)
                .map_err(|_| ReputationIngestError::CapacityExceeded)?,
        )?;
        context.accept_encoded_page(page, RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1)?;
        assert_page_target(
            context.target,
            ReputationFinalizedIdentityV1 {
                height: page.finalized_cursor.height,
                block_hash: page.finalized_cursor.block_hash,
            },
        )?;
        for event in &page.events {
            let identity = reserve_event_identity(event);
            validate_page_event_order(&mut previous_page_event, identity)?;
            let signal = (*event.event.provider_id())
                .map_or(ReputationSignalV1::Noop, |provider_id| {
                    ReputationSignalV1::ProviderObserved { provider_id }
                });
            context.accept_event(
                ReputationCommittedFeedV1::Reserve,
                identity,
                *event.event.occurred_at_unix_ms(),
                signal,
                event,
            )?;
        }
    }
    source_update(
        context,
        ReputationSourceV1::Reserve,
        finalized_at_unix_ms,
        pages.last().is_some_and(|page| !page.has_more),
    )
}

fn prepare_reserve_projection(
    pages: &[ReserveProviderAccountPageV1],
    target: ReputationFinalizedIdentityV1,
    finalized_at_unix_ms: u64,
    policy: &ReputationIngestPolicyV1,
    encoded_page_bytes: &mut usize,
) -> Result<(Vec<ReserveStageRecordV1>, [u8; 32]), ReputationIngestError> {
    if pages.is_empty()
        || pages.last().is_some_and(|page| page.has_more)
        || pages
            .iter()
            .take(pages.len().saturating_sub(1))
            .any(|page| !page.has_more)
    {
        return Err(ReputationIngestError::ReserveStageResolutionMissing);
    }
    let max_providers = usize::try_from(policy.max_providers)
        .map_err(|_| ReputationIngestError::CapacityExceeded)?;
    let mut stages = Vec::new();
    let mut previous_provider = None;
    let mut previous_promised_more = false;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs-reputation-reserve-projection-v1");
    let target_bytes =
        norito::to_bytes(&target).map_err(|_| ReputationIngestError::CanonicalEncoding)?;
    update_len_prefixed(&mut hasher, &target_bytes)?;

    for page in pages {
        if previous_promised_more && page.accounts.is_empty() {
            return Err(ReputationIngestError::InvalidPage);
        }
        validate_page_item_count(
            page.accounts.len(),
            usize::try_from(RESERVE_QUERY_MAX_ITEMS_V1)
                .map_err(|_| ReputationIngestError::CapacityExceeded)?,
        )?;
        validate_encoded_page(
            page,
            RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            encoded_page_bytes,
            policy.checkpoint_max_bytes,
        )?;
        assert_page_target(
            target,
            ReputationFinalizedIdentityV1 {
                height: page.finalized_cursor.height,
                block_hash: page.finalized_cursor.block_hash,
            },
        )?;
        let last_provider = page
            .accounts
            .last()
            .map(|account| account.terms.provider_id);
        match (page.has_more, page.next_after, last_provider) {
            (true, Some(next), Some(last)) if next == last => {}
            (false, None, _) => {}
            _ => return Err(ReputationIngestError::InvalidPage),
        }
        for account in &page.accounts {
            let provider_id = account.terms.provider_id;
            if provider_id.as_bytes() == &[0; 32]
                || account.revision == 0
                || account.policy_digest == [0; 32]
                || account.updated_at_unix == 0
                || account.updated_at_unix > finalized_at_unix_ms / 1_000
                || account.rent_charged_through_unix == 0
                || account.rent_charged_through_unix > account.updated_at_unix
                || account.interest_accrued_at_unix > account.updated_at_unix
                || previous_provider.is_some_and(|previous| previous >= provider_id)
            {
                return Err(ReputationIngestError::InvalidPage);
            }
            if stages.len() >= max_providers {
                return Err(ReputationIngestError::CapacityExceeded);
            }
            previous_provider = Some(provider_id);
            stages.push(ReserveStageRecordV1 {
                provider_id,
                stage: reserve_stage(account.lifecycle_stage),
            });
            let bytes =
                norito::to_bytes(account).map_err(|_| ReputationIngestError::CanonicalEncoding)?;
            update_len_prefixed(&mut hasher, &bytes)?;
        }
        previous_promised_more = page.has_more;
    }
    Ok((stages, *hasher.finalize().as_bytes()))
}

fn canonical_reserve_stage_projection(
    checkpoint: &ReputationIngestCheckpointV1,
    target: ReputationFinalizedIdentityV1,
    mut stages: Vec<ReserveStageRecordV1>,
) -> Vec<ReserveStageRecordV1> {
    stages.sort_by_key(|record| record.provider_id);
    if checkpoint
        .progress(ReputationSourceV1::Reserve)
        .observed_through
        == Some(target)
    {
        stages.clear();
    }
    stages
}

fn source_update(
    context: &PrepareContext<'_>,
    source: ReputationSourceV1,
    finalized_at_unix_ms: u64,
    complete: bool,
) -> Result<SourceFinalityUpdateV1, ReputationIngestError> {
    let existing = context.checkpoint.progress(source);
    let last_event = context.last(committed_feed_for_source(source));
    if existing.observed_through == Some(context.target) {
        if existing.observed_at_unix_ms != finalized_at_unix_ms || existing.last_event != last_event
        {
            return Err(ReputationIngestError::EventEquivocation);
        }
    }
    Ok(SourceFinalityUpdateV1 {
        source,
        target: context.target,
        finalized_at_unix_ms,
        last_event,
        complete,
        reserve_projection_digest: existing.reserve_projection_digest,
    })
}

fn validate_event_page_chain<I>(page_count: usize, pages: I) -> Result<(), ReputationIngestError>
where
    I: IntoIterator<
        Item = (
            bool,
            Option<ReputationCommittedEventIdentityV1>,
            Option<ReputationCommittedEventIdentityV1>,
        ),
    >,
{
    if page_count == 0 {
        return Err(ReputationIngestError::InvalidPage);
    }
    let mut previous_promised_more = false;
    for (index, (has_more, next_after, last_event)) in pages.into_iter().enumerate() {
        if previous_promised_more && last_event.is_none() {
            return Err(ReputationIngestError::InvalidPage);
        }
        if index + 1 < page_count && !has_more {
            return Err(ReputationIngestError::InvalidPage);
        }
        match (has_more, next_after, last_event) {
            (true, Some(next), Some(last)) if next == last => {}
            (false, None, _) => {}
            _ => return Err(ReputationIngestError::InvalidPage),
        }
        previous_promised_more = has_more;
    }
    Ok(())
}

fn validate_encoded_page<T: norito::NoritoSerialize>(
    page: &T,
    max_page_bytes: usize,
    encoded_page_bytes: &mut usize,
    max_batch_bytes: u64,
) -> Result<(), ReputationIngestError> {
    let encoded_bytes = norito::core::encoded_frame_len(page)
        .map_err(|_| ReputationIngestError::CanonicalEncoding)?;
    if encoded_bytes > max_page_bytes {
        return Err(ReputationIngestError::CapacityExceeded);
    }
    *encoded_page_bytes = encoded_page_bytes
        .checked_add(encoded_bytes)
        .ok_or(ReputationIngestError::CapacityExceeded)?;
    if u64::try_from(*encoded_page_bytes).unwrap_or(u64::MAX) > max_batch_bytes {
        return Err(ReputationIngestError::CapacityExceeded);
    }
    Ok(())
}

fn validate_page_item_count(observed: usize, maximum: usize) -> Result<(), ReputationIngestError> {
    if observed > maximum {
        return Err(ReputationIngestError::CapacityExceeded);
    }
    Ok(())
}

fn validate_page_event_order(
    previous: &mut Option<ReputationCommittedEventIdentityV1>,
    next: ReputationCommittedEventIdentityV1,
) -> Result<(), ReputationIngestError> {
    next.validate()?;
    if let Some(previous) = *previous {
        if next.sequence <= previous.sequence
            || next.block_height < previous.block_height
            || (next.block_height == previous.block_height
                && (next.block_hash != previous.block_hash
                    || next.event_index <= previous.event_index))
        {
            return Err(ReputationIngestError::EventReordered);
        }
        if next.sequence
            != previous
                .sequence
                .checked_add(1)
                .ok_or(ReputationIngestError::ArithmeticOverflow)?
        {
            return Err(ReputationIngestError::EventGap);
        }
    }
    *previous = Some(next);
    Ok(())
}

fn assert_page_target(
    expected: ReputationFinalizedIdentityV1,
    observed: ReputationFinalizedIdentityV1,
) -> Result<(), ReputationIngestError> {
    if expected != observed {
        return Err(ReputationIngestError::PageAnchorMismatch);
    }
    Ok(())
}

const fn proof_event_identity(
    event: &ProofOutcomeFinalizedEventV1,
) -> ReputationCommittedEventIdentityV1 {
    ReputationCommittedEventIdentityV1 {
        sequence: event.sequence,
        block_height: event.block_height,
        block_hash: event.block_hash,
        event_index: event.event_index,
    }
}

const fn journal_event_identity(
    event: &ReputationJournalFinalizedEventV1,
) -> ReputationCommittedEventIdentityV1 {
    ReputationCommittedEventIdentityV1 {
        sequence: event.sequence,
        block_height: event.block_height,
        block_hash: event.block_hash,
        event_index: event.event_index,
    }
}

const fn repair_event_identity(
    event: &RepairFinalizedEventV1,
) -> ReputationCommittedEventIdentityV1 {
    ReputationCommittedEventIdentityV1 {
        sequence: event.sequence,
        block_height: event.block_height,
        block_hash: event.block_hash,
        event_index: event.event_index,
    }
}

const fn orderbook_event_identity(
    event: &OrderbookFinalizedEventV1,
) -> ReputationCommittedEventIdentityV1 {
    ReputationCommittedEventIdentityV1 {
        sequence: event.sequence,
        block_height: event.block_height,
        block_hash: event.block_hash,
        event_index: event.event_index,
    }
}

const fn reserve_event_identity(
    event: &iroha_data_model::sorafs::reserve::ReserveFinalizedEventV1,
) -> ReputationCommittedEventIdentityV1 {
    ReputationCommittedEventIdentityV1 {
        sequence: event.sequence,
        block_height: event.block_height,
        block_hash: event.block_hash,
        event_index: event.event_index,
    }
}

const fn reserve_stage(stage: ReserveLifecycleStage) -> ReputationReserveStageV1 {
    match stage {
        ReserveLifecycleStage::Active => ReputationReserveStageV1::Active,
        ReserveLifecycleStage::Warning => ReputationReserveStageV1::Warning,
        ReserveLifecycleStage::Grace => ReputationReserveStageV1::Grace,
        ReserveLifecycleStage::Delinquent => ReputationReserveStageV1::Delinquent,
        ReserveLifecycleStage::Default => ReputationReserveStageV1::Default,
    }
}

fn apply_pending_batch(
    checkpoint: &mut ReputationIngestCheckpointV1,
    policy: &ReputationIngestPolicyV1,
) -> Result<(), ReputationIngestError> {
    let pending = checkpoint
        .pending
        .clone()
        .ok_or(ReputationIngestError::InvalidCheckpoint)?;
    validate_global_target(
        checkpoint.latest_finalized,
        checkpoint.latest_finalized_at_unix_ms,
        pending.target,
        pending.finalized_at_unix_ms,
    )?;
    let mut providers = checkpoint
        .providers
        .iter()
        .cloned()
        .map(|provider| (provider.provider_id, provider))
        .collect::<BTreeMap<_, _>>();
    for event in &pending.events {
        if (policy.window_start_height..=policy.window_end_height)
            .contains(&event.identity.block_height)
            && let Some(provider_id) = event.signal.provider_id()
        {
            if !providers.contains_key(&provider_id)
                && providers.len()
                    >= usize::try_from(policy.max_providers)
                        .map_err(|_| ReputationIngestError::CapacityExceeded)?
            {
                return Err(ReputationIngestError::CapacityExceeded);
            }
            providers
                .entry(provider_id)
                .or_insert_with(|| ProviderAccumulatorV1::new(provider_id))
                .apply(event.signal)?;
        }
    }
    let replaces_reserve_projection = pending.finality_updates.iter().any(|update| {
        update.source == ReputationSourceV1::Reserve
            && update.complete
            && update.reserve_projection_digest.is_some()
    });
    if replaces_reserve_projection {
        for provider in providers.values_mut() {
            provider.reserve_stage = None;
        }
    }
    for record in &pending.reserve_stages {
        if !providers.contains_key(&record.provider_id)
            && providers.len()
                >= usize::try_from(policy.max_providers)
                    .map_err(|_| ReputationIngestError::CapacityExceeded)?
        {
            return Err(ReputationIngestError::CapacityExceeded);
        }
        providers
            .entry(record.provider_id)
            .or_insert_with(|| ProviderAccumulatorV1::new(record.provider_id))
            .reserve_stage = Some(record.stage);
    }
    checkpoint.providers = providers.into_values().collect();

    checkpoint
        .receipts
        .extend(pending.events.iter().map(|event| EventReceiptV1 {
            feed: event.feed,
            identity: event.identity,
            event_digest: event.event_digest,
        }));
    checkpoint.receipts.sort_by_key(receipt_order_key);
    for pair in checkpoint.receipts.windows(2) {
        if pair[0].feed == pair[1].feed && pair[0].identity.sequence == pair[1].identity.sequence {
            if pair[0] != pair[1] {
                return Err(ReputationIngestError::EventEquivocation);
            }
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
    }
    let receipt_limit = usize::try_from(policy.max_replay_receipts)
        .map_err(|_| ReputationIngestError::CapacityExceeded)?;
    if checkpoint.receipts.len() > receipt_limit {
        let remove = checkpoint.receipts.len() - receipt_limit;
        checkpoint.receipts.drain(..remove);
    }

    for update in &pending.finality_updates {
        let progress = checkpoint.progress_mut(update.source);
        progress.last_event = update.last_event;
        if update.complete {
            progress.observed_through = Some(update.target);
            progress.observed_at_unix_ms = update.finalized_at_unix_ms;
            progress.reserve_projection_digest = update.reserve_projection_digest;
        }
    }
    checkpoint.latest_finalized = Some(pending.target);
    checkpoint.latest_finalized_at_unix_ms = pending.finalized_at_unix_ms;
    checkpoint.pending = None;
    validate_checkpoint(checkpoint, policy, checkpoint.policy_digest)
}

fn receipt_order_key(receipt: &EventReceiptV1) -> (u64, u32, ReputationCommittedFeedV1, u64) {
    (
        receipt.identity.block_height,
        receipt.identity.event_index,
        receipt.feed,
        receipt.identity.sequence,
    )
}

fn missing_sources(
    checkpoint: &ReputationIngestCheckpointV1,
    required: ReputationRequiredSourceMaskV1,
    target: ReputationFinalizedIdentityV1,
) -> ReputationRequiredSourceMaskV1 {
    let mut present = ReputationRequiredSourceMaskV1::EMPTY;
    for progress in &checkpoint.source_progress {
        if required.contains(progress.source) && progress.observed_through == Some(target) {
            present = present.union(ReputationRequiredSourceMaskV1::from_source(progress.source));
        }
    }
    required.difference(present)
}

fn build_signing_material(
    checkpoint: &ReputationIngestCheckpointV1,
    policy: &ReputationIngestPolicyV1,
    policy_digest: [u8; 32],
) -> Result<ReputationUnsignedSigningMaterialV1, ReputationIngestError> {
    if checkpoint.pending.is_some() {
        return Err(ReputationIngestError::ReconciliationPending);
    }
    let target = checkpoint
        .latest_finalized
        .ok_or(ReputationIngestError::WindowNotFinalized)?;
    if target.height != policy.window_end_height
        || checkpoint.latest_finalized_at_unix_ms == 0
        || !missing_sources(checkpoint, policy.required_sources, target).is_empty()
    {
        return Err(ReputationIngestError::MissingRequiredSources);
    }
    if checkpoint.providers.is_empty() {
        return Err(ReputationIngestError::EmptyProviderSet);
    }

    let mut provider_inputs = Vec::new();
    provider_inputs
        .try_reserve_exact(checkpoint.providers.len())
        .map_err(|_| ReputationIngestError::CapacityExceeded)?;
    for provider in &checkpoint.providers {
        let reserve_stage = provider
            .reserve_stage
            .ok_or(ReputationIngestError::ReserveStageUnavailable)?;
        if provider.por_total == 0
            || provider.pdp_total == 0
            || provider.potr_total == 0
            || provider.latency_total == 0
        {
            return Err(ReputationIngestError::MetricUnavailable);
        }
        provider_inputs.push(ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: hex::encode(provider.provider_id.as_bytes()),
            metrics: ReputationProviderMetricsV1 {
                version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
                por_success_bps: ratio_bps(provider.por_successes, provider.por_total)?,
                pdp_success_bps: ratio_bps(provider.pdp_successes, provider.pdp_total)?,
                potr_success_bps: ratio_bps(provider.potr_successes, provider.potr_total)?,
                latency_health_bps: ratio_bps(provider.latency_healthy, provider.latency_total)?,
                dispute_rate_bps: zero_safe_ratio_bps(
                    provider.disputes_upheld,
                    provider.disputes_resolved,
                )?,
                token_violation_rate_bps: zero_safe_ratio_bps(
                    provider.token_violations,
                    provider.token_observations,
                )?,
                repair_breach_rate_bps: zero_safe_ratio_bps(
                    provider.repair_breaches,
                    provider.repair_terminals,
                )?,
            },
            reserve_stage,
            previous_score_bps: None,
            active_dispute: provider.has_active_dispute(),
            slashing_event: provider.slashing_event,
        });
    }
    provider_inputs.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
    let scoring_evidence = ReputationScoringEvidenceV1 {
        version: REPUTATION_SCORING_EVIDENCE_VERSION_V1,
        provider_inputs,
        trust_edges: Vec::new(),
    };
    let scoring_evidence_digest = scoring_evidence
        .canonical_digest()
        .map_err(|_| ReputationIngestError::CanonicalEncoding)?;
    let source_finality = checkpoint
        .source_progress
        .iter()
        .filter(|progress| policy.required_sources.contains(progress.source))
        .map(|progress| {
            Ok(ReputationSourceFinalityV1 {
                source: progress.source,
                observed_through: progress
                    .observed_through
                    .ok_or(ReputationIngestError::MissingRequiredSources)?,
                last_event: progress.last_event,
            })
        })
        .collect::<Result<Vec<_>, ReputationIngestError>>()?;
    let snapshot_seed = ReputationSnapshotSeedV1 {
        network_id: policy.network_id,
        ingest_policy_digest: policy_digest,
        snapshot_trust_policy_digest: policy.snapshot_trust_policy_digest,
        target_finalized: target,
        target_finalized_at_unix_ms: checkpoint.latest_finalized_at_unix_ms,
        window_start_height: policy.window_start_height,
        window_end_height: policy.window_end_height,
        source_finality: source_finality.clone(),
        scoring_evidence_digest,
    };
    let seed_digest = hash_canonical(b"sorafs-reputation-snapshot-id-v1", &snapshot_seed)?;
    let mut snapshot_id = [0; 16];
    snapshot_id.copy_from_slice(&seed_digest[..16]);
    if snapshot_id == [0; 16] {
        return Err(ReputationIngestError::CanonicalEncoding);
    }
    let generated_at_unix = checked_unix_millis_to_seconds(checkpoint.latest_finalized_at_unix_ms)
        .ok_or(ReputationIngestError::InvalidFinalizedIdentity)?;
    let snapshot = build_reputation_snapshot(
        snapshot_id,
        generated_at_unix,
        policy.weights,
        &scoring_evidence.provider_inputs,
        None,
    )
    .map_err(|_| ReputationIngestError::CanonicalEncoding)?;
    scoring_evidence
        .verify_snapshot(&snapshot)
        .map_err(|_| ReputationIngestError::CanonicalEncoding)?;
    let signing_digest = snapshot_signing_digest(
        &snapshot,
        policy.snapshot_trust_policy_digest,
        scoring_evidence_digest,
    )
    .map_err(|_| ReputationIngestError::CanonicalEncoding)?;
    Ok(ReputationUnsignedSigningMaterialV1 {
        version: REPUTATION_UNSIGNED_MATERIAL_VERSION_V1,
        network_id: policy.network_id,
        ingest_policy_digest: policy_digest,
        snapshot_trust_policy_digest: policy.snapshot_trust_policy_digest,
        window_start_height: policy.window_start_height,
        window_end_height: policy.window_end_height,
        target_finalized: target,
        target_finalized_at_unix_ms: checkpoint.latest_finalized_at_unix_ms,
        source_finality,
        scoring_evidence,
        scoring_evidence_digest,
        snapshot,
        snapshot_signing_digest: signing_digest,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize)]
struct ReputationSnapshotSeedV1 {
    network_id: NetworkId,
    ingest_policy_digest: [u8; 32],
    snapshot_trust_policy_digest: [u8; 32],
    target_finalized: ReputationFinalizedIdentityV1,
    target_finalized_at_unix_ms: u64,
    window_start_height: u64,
    window_end_height: u64,
    source_finality: Vec<ReputationSourceFinalityV1>,
    scoring_evidence_digest: [u8; 32],
}

fn ratio_bps(successes: u64, total: u64) -> Result<u16, ReputationIngestError> {
    if total == 0 || successes > total {
        return Err(ReputationIngestError::MetricUnavailable);
    }
    let numerator = u128::from(successes)
        .checked_mul(10_000)
        .ok_or(ReputationIngestError::ArithmeticOverflow)?;
    u16::try_from(numerator / u128::from(total))
        .map_err(|_| ReputationIngestError::ArithmeticOverflow)
}

fn zero_safe_ratio_bps(successes: u64, total: u64) -> Result<u16, ReputationIngestError> {
    if total == 0 {
        if successes == 0 {
            return Ok(0);
        }
        return Err(ReputationIngestError::ArithmeticOverflow);
    }
    ratio_bps(successes, total)
}

fn checked_next(value: u64) -> Result<u64, ReputationIngestError> {
    value
        .checked_add(1)
        .ok_or(ReputationIngestError::ArithmeticOverflow)
}

fn decode_checkpoint(
    bytes: &[u8],
    policy: &ReputationIngestPolicyV1,
    policy_digest: [u8; 32],
) -> Result<ReputationIngestCheckpointV1, ReputationIngestError> {
    if bytes.is_empty()
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > policy.checkpoint_max_bytes
    {
        return Err(ReputationIngestError::CheckpointTooLarge);
    }
    norito::core::from_bytes_view(bytes).map_err(|_| ReputationIngestError::InvalidCheckpoint)?;
    let checkpoint = norito::decode_from_bytes_with_limits::<ReputationIngestCheckpointV1>(
        bytes,
        checkpoint_decode_limits(bytes.len())?,
    )
    .map_err(|_| ReputationIngestError::InvalidCheckpoint)?;
    if norito::to_bytes(&checkpoint).map_err(|_| ReputationIngestError::CanonicalEncoding)? != bytes
    {
        return Err(ReputationIngestError::InvalidCheckpoint);
    }
    validate_checkpoint(&checkpoint, policy, policy_digest)?;
    Ok(checkpoint)
}

fn ensure_checkpoint_size(
    checkpoint: &ReputationIngestCheckpointV1,
    policy: &ReputationIngestPolicyV1,
) -> Result<(), ReputationIngestError> {
    let bytes =
        norito::to_bytes(checkpoint).map_err(|_| ReputationIngestError::CanonicalEncoding)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > policy.checkpoint_max_bytes {
        return Err(ReputationIngestError::CheckpointTooLarge);
    }
    Ok(())
}

fn checkpoint_decode_limits(
    encoded_bytes: usize,
) -> Result<norito::DecodeLimits, ReputationIngestError> {
    if encoded_bytes == 0 {
        return Err(ReputationIngestError::InvalidCheckpoint);
    }
    let total_elements = encoded_bytes
        .checked_mul(CHECKPOINT_ELEMENT_AMPLIFICATION_LIMIT)
        .ok_or(ReputationIngestError::CheckpointTooLarge)?;
    let total_allocated_bytes = encoded_bytes
        .checked_mul(CHECKPOINT_ALLOCATION_AMPLIFICATION_LIMIT)
        .and_then(|budget| budget.checked_add(CHECKPOINT_ALLOCATION_FIXED_OVERHEAD_BYTES))
        .ok_or(ReputationIngestError::CheckpointTooLarge)?;
    Ok(norito::DecodeLimits::new(
        encoded_bytes,
        encoded_bytes,
        total_elements,
        total_allocated_bytes,
        CHECKPOINT_MAX_NESTING_DEPTH,
    ))
}

fn validate_checkpoint(
    checkpoint: &ReputationIngestCheckpointV1,
    policy: &ReputationIngestPolicyV1,
    policy_digest: [u8; 32],
) -> Result<(), ReputationIngestError> {
    policy.validate()?;
    if checkpoint.version != REPUTATION_INGEST_CHECKPOINT_VERSION_V1
        || checkpoint.policy_digest != policy_digest
        || checkpoint.source_progress.len() != ALL_SOURCES.len()
        || checkpoint.providers.len()
            > usize::try_from(policy.max_providers)
                .map_err(|_| ReputationIngestError::InvalidCheckpoint)?
        || checkpoint.receipts.len()
            > usize::try_from(policy.max_replay_receipts)
                .map_err(|_| ReputationIngestError::InvalidCheckpoint)?
    {
        return Err(ReputationIngestError::InvalidCheckpoint);
    }
    match checkpoint.latest_finalized {
        Some(identity) => {
            identity
                .validate()
                .map_err(|_| ReputationIngestError::InvalidCheckpoint)?;
            if identity.height > policy.window_end_height
                || checked_unix_millis_to_seconds(checkpoint.latest_finalized_at_unix_ms).is_none()
            {
                return Err(ReputationIngestError::InvalidCheckpoint);
            }
        }
        None if checkpoint.latest_finalized_at_unix_ms == 0 => {}
        None => return Err(ReputationIngestError::InvalidCheckpoint),
    }

    for (expected, progress) in ALL_SOURCES.iter().zip(&checkpoint.source_progress) {
        if progress.source != *expected {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        if let Some(last) = progress.last_event {
            last.validate()
                .map_err(|_| ReputationIngestError::InvalidCheckpoint)?;
            if last.block_height > policy.window_end_height
                || checkpoint
                    .latest_finalized
                    .is_none_or(|latest| last.block_height > latest.height)
            {
                return Err(ReputationIngestError::InvalidCheckpoint);
            }
        }
        match progress.observed_through {
            Some(identity) => {
                identity
                    .validate()
                    .map_err(|_| ReputationIngestError::InvalidCheckpoint)?;
                if identity.height > policy.window_end_height
                    || progress.observed_at_unix_ms == 0
                    || checkpoint
                        .latest_finalized
                        .is_none_or(|latest| identity.height > latest.height)
                    || progress.observed_at_unix_ms > checkpoint.latest_finalized_at_unix_ms
                    || checkpoint.latest_finalized.is_some_and(|latest| {
                        identity.height == latest.height
                            && (identity.block_hash != latest.block_hash
                                || progress.observed_at_unix_ms
                                    != checkpoint.latest_finalized_at_unix_ms)
                    })
                {
                    return Err(ReputationIngestError::InvalidCheckpoint);
                }
            }
            None if progress.observed_at_unix_ms == 0 => {}
            None => return Err(ReputationIngestError::InvalidCheckpoint),
        }
        if progress.source != ReputationSourceV1::Reserve
            && progress.reserve_projection_digest.is_some()
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        if progress.source == ReputationSourceV1::Reserve
            && progress.observed_through.is_some()
            && progress.reserve_projection_digest.is_none()
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
    }
    let journal_progress = checkpoint.progress(ReputationSourceV1::Por);
    for source in [ReputationSourceV1::Dispute, ReputationSourceV1::Token] {
        let progress = checkpoint.progress(source);
        if progress.last_event != journal_progress.last_event
            || progress.observed_through != journal_progress.observed_through
            || progress.observed_at_unix_ms != journal_progress.observed_at_unix_ms
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
    }

    let mut receipt_keys = BTreeSet::new();
    let mut previous_order = None;
    let mut final_feed_receipt = BTreeMap::new();
    for receipt in &checkpoint.receipts {
        receipt
            .identity
            .validate()
            .map_err(|_| ReputationIngestError::InvalidCheckpoint)?;
        if receipt.event_digest == [0; 32]
            || !receipt_keys.insert((receipt.feed, receipt.identity.sequence))
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        let order = receipt_order_key(receipt);
        if previous_order.is_some_and(|previous| previous >= order) {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        previous_order = Some(order);
        let feed_progress = checkpoint_progress_for_feed(checkpoint, receipt.feed);
        let Some(feed_last) = feed_progress.last_event else {
            return Err(ReputationIngestError::InvalidCheckpoint);
        };
        if receipt.identity.block_height > policy.window_end_height
            || checkpoint
                .latest_finalized
                .is_none_or(|latest| receipt.identity.block_height > latest.height)
            || receipt.identity.sequence > feed_last.sequence
            || (receipt.identity.sequence == feed_last.sequence && receipt.identity != feed_last)
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        if final_feed_receipt
            .insert(receipt.feed, *receipt)
            .is_some_and(|previous: EventReceiptV1| {
                previous
                    .identity
                    .sequence
                    .checked_add(1)
                    .is_none_or(|expected| expected != receipt.identity.sequence)
            })
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
    }
    for (feed, receipt) in final_feed_receipt {
        if checkpoint_progress_for_feed(checkpoint, feed).last_event != Some(receipt.identity) {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
    }
    validate_known_block_hashes(
        checkpoint
            .receipts
            .iter()
            .map(|receipt| receipt.identity)
            .chain(
                checkpoint
                    .source_progress
                    .iter()
                    .filter_map(|progress| progress.last_event),
            )
            .chain(checkpoint.source_progress.iter().filter_map(|progress| {
                progress
                    .observed_through
                    .map(finalized_identity_as_event_identity)
            }))
            .chain(
                checkpoint
                    .latest_finalized
                    .map(finalized_identity_as_event_identity),
            ),
    )?;

    let mut previous_provider = None;
    for provider in &checkpoint.providers {
        if provider.provider_id.as_bytes() == &[0; 32]
            || previous_provider.is_some_and(|previous| previous >= provider.provider_id)
            || provider.por_successes > provider.por_total
            || provider.pdp_successes > provider.pdp_total
            || provider.potr_successes > provider.potr_total
            || provider.latency_healthy > provider.latency_total
            || provider.disputes_upheld > provider.disputes_resolved
            || provider.token_violations > provider.token_observations
            || provider.repair_breaches > provider.repair_terminals
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        previous_provider = Some(provider.provider_id);
    }
    if let Some(pending) = &checkpoint.pending {
        validate_pending(pending, checkpoint, policy)?;
    }
    validate_material_delivery_state(checkpoint, policy, policy_digest)?;
    Ok(())
}

fn validate_material_delivery_state(
    checkpoint: &ReputationIngestCheckpointV1,
    policy: &ReputationIngestPolicyV1,
    policy_digest: [u8; 32],
) -> Result<(), ReputationIngestError> {
    if checkpoint.material_outbox.is_some() && checkpoint.material_acknowledgement.is_some() {
        return Err(ReputationIngestError::InvalidCheckpoint);
    }
    if checkpoint.pending.is_some()
        && (checkpoint.material_outbox.is_some() || checkpoint.material_acknowledgement.is_some())
    {
        return Err(ReputationIngestError::InvalidCheckpoint);
    }
    if checkpoint.material_outbox.is_none() && checkpoint.material_acknowledgement.is_none() {
        return Ok(());
    }

    let expected_material = build_signing_material(checkpoint, policy, policy_digest)
        .map_err(|_| ReputationIngestError::InvalidCheckpoint)?;
    let expected_digest = unsigned_material_digest(&expected_material)
        .map_err(|_| ReputationIngestError::InvalidCheckpoint)?;
    let expected_verified_at_unix =
        checked_unix_millis_to_seconds(checkpoint.latest_finalized_at_unix_ms)
            .ok_or(ReputationIngestError::InvalidCheckpoint)?;
    if let Some(entry) = &checkpoint.material_outbox {
        if entry.version != REPUTATION_UNSIGNED_MATERIAL_DELIVERY_VERSION_V1
            || entry.sequence != 1
            || entry.material_digest != expected_digest
            || entry.failed_attempts
                != u32::try_from(entry.failure_receipts.len())
                    .map_err(|_| ReputationIngestError::InvalidCheckpoint)?
            || entry.failed_attempts > policy.max_material_delivery_failures
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        let expected_state = if entry.failed_attempts == policy.max_material_delivery_failures {
            ReputationUnsignedMaterialDeliveryStateV1::DeadLetter
        } else {
            ReputationUnsignedMaterialDeliveryStateV1::Pending
        };
        if entry.state != expected_state {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        let mut unique_failure_receipts = BTreeSet::new();
        if entry
            .failure_receipts
            .iter()
            .any(|receipt| *receipt == [0; 32] || !unique_failure_receipts.insert(*receipt))
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
    }
    if let Some(acknowledgement) = checkpoint.material_acknowledgement
        && (acknowledgement.version != REPUTATION_UNSIGNED_MATERIAL_ACKNOWLEDGEMENT_VERSION_V1
            || acknowledgement.sequence != 1
            || acknowledgement.material_digest != expected_digest
            || acknowledgement.trust_policy_digest != policy.snapshot_trust_policy_digest
            || acknowledgement.verified_at_unix != expected_verified_at_unix
            || acknowledgement.signed_result_digest == [0; 32])
    {
        return Err(ReputationIngestError::InvalidCheckpoint);
    }
    Ok(())
}

fn validate_pending(
    pending: &PendingBatchV1,
    checkpoint: &ReputationIngestCheckpointV1,
    policy: &ReputationIngestPolicyV1,
) -> Result<(), ReputationIngestError> {
    pending
        .target
        .validate()
        .map_err(|_| ReputationIngestError::InvalidCheckpoint)?;
    if pending.target.height > policy.window_end_height
        || pending.finalized_at_unix_ms == 0
        || (pending.events.is_empty()
            && pending.finality_updates.is_empty()
            && pending.reserve_stages.is_empty())
        || pending.events.len()
            > usize::try_from(policy.max_pending_events)
                .map_err(|_| ReputationIngestError::InvalidCheckpoint)?
        || pending.finality_updates.len() > ALL_SOURCES.len()
        || pending.reserve_stages.len()
            > usize::try_from(policy.max_providers)
                .map_err(|_| ReputationIngestError::InvalidCheckpoint)?
    {
        return Err(ReputationIngestError::InvalidCheckpoint);
    }
    validate_global_target(
        checkpoint.latest_finalized,
        checkpoint.latest_finalized_at_unix_ms,
        pending.target,
        pending.finalized_at_unix_ms,
    )
    .map_err(|_| ReputationIngestError::InvalidCheckpoint)?;

    let mut previous_event_order = None;
    let mut event_keys = BTreeSet::new();
    let mut event_feeds = BTreeSet::new();
    for event in &pending.events {
        event
            .identity
            .validate()
            .map_err(|_| ReputationIngestError::InvalidCheckpoint)?;
        if event.event_digest == [0; 32]
            || event.occurred_at_unix_ms == 0
            || event.occurred_at_unix_ms > pending.finalized_at_unix_ms
            || event.identity.block_height > pending.target.height
            || !signal_matches_feed(event.signal, event.feed)
            || !signal_is_well_formed(event.signal)
            || !event_keys.insert((event.feed, event.identity.sequence))
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        if event
            .signal
            .provider_id()
            .is_some_and(|provider| provider.as_bytes() == &[0; 32])
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        event_feeds.insert(event.feed);
        let order = (
            event.identity.block_height,
            event.identity.event_index,
            event.feed,
            event.identity.sequence,
        );
        if previous_event_order.is_some_and(|previous| previous >= order) {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        previous_event_order = Some(order);
    }
    validate_known_block_hashes(
        checkpoint
            .receipts
            .iter()
            .map(|receipt| receipt.identity)
            .chain(
                checkpoint
                    .source_progress
                    .iter()
                    .filter_map(|progress| progress.last_event),
            )
            .chain(checkpoint.source_progress.iter().filter_map(|progress| {
                progress
                    .observed_through
                    .map(finalized_identity_as_event_identity)
            }))
            .chain(
                checkpoint
                    .latest_finalized
                    .map(finalized_identity_as_event_identity),
            )
            .chain(pending.events.iter().map(|event| event.identity))
            .chain(Some(finalized_identity_as_event_identity(pending.target))),
    )?;

    for feed in &event_feeds {
        let mut expected = checkpoint_progress_for_feed(checkpoint, *feed)
            .last_event
            .map_or(Ok(1), |identity| {
                identity
                    .sequence
                    .checked_add(1)
                    .ok_or(ReputationIngestError::InvalidCheckpoint)
            })?;
        let mut feed_events = pending
            .events
            .iter()
            .filter(|event| event.feed == *feed)
            .peekable();
        while let Some(event) = feed_events.next() {
            if event.identity.sequence != expected {
                return Err(ReputationIngestError::InvalidCheckpoint);
            }
            if feed_events.peek().is_some() {
                expected = expected
                    .checked_add(1)
                    .ok_or(ReputationIngestError::InvalidCheckpoint)?;
            }
        }
    }

    let mut previous_source = None;
    let mut update_sources = BTreeSet::new();
    for update in &pending.finality_updates {
        if previous_source.is_some_and(|previous| previous >= update.source)
            || update.target != pending.target
            || update.finalized_at_unix_ms != pending.finalized_at_unix_ms
            || update.last_event != pending_last_event(checkpoint, pending, update.source)
            || (update.source != ReputationSourceV1::Reserve
                && update.reserve_projection_digest.is_some())
            || (update.source == ReputationSourceV1::Reserve
                && update.complete
                && update.reserve_projection_digest.is_none())
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        previous_source = Some(update.source);
        update_sources.insert(update.source);
    }
    for feed in event_feeds {
        if !updates_cover_feed(&update_sources, feed) {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
    }
    let journal_updates = pending
        .finality_updates
        .iter()
        .filter(|update| {
            matches!(
                update.source,
                ReputationSourceV1::Por | ReputationSourceV1::Dispute | ReputationSourceV1::Token
            )
        })
        .collect::<Vec<_>>();
    if !journal_updates.is_empty()
        && (journal_updates.len() != 3
            || journal_updates.windows(2).any(|pair| {
                pair[0].target != pair[1].target
                    || pair[0].finalized_at_unix_ms != pair[1].finalized_at_unix_ms
                    || pair[0].last_event != pair[1].last_event
                    || pair[0].complete != pair[1].complete
                    || pair[0].reserve_projection_digest != pair[1].reserve_projection_digest
            }))
    {
        return Err(ReputationIngestError::InvalidCheckpoint);
    }
    if !pending.reserve_stages.is_empty()
        && !pending.finality_updates.iter().any(|update| {
            update.source == ReputationSourceV1::Reserve
                && update.complete
                && update.reserve_projection_digest.is_some()
        })
    {
        return Err(ReputationIngestError::InvalidCheckpoint);
    }
    let mut previous_stage_provider = None;
    for stage in &pending.reserve_stages {
        if stage.provider_id.as_bytes() == &[0; 32]
            || previous_stage_provider.is_some_and(|previous| previous >= stage.provider_id)
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
        previous_stage_provider = Some(stage.provider_id);
    }
    Ok(())
}

fn pending_last_event(
    checkpoint: &ReputationIngestCheckpointV1,
    pending: &PendingBatchV1,
    source: ReputationSourceV1,
) -> Option<ReputationCommittedEventIdentityV1> {
    let feed = committed_feed_for_source(source);
    pending
        .events
        .iter()
        .filter(|event| event.feed == feed)
        .map(|event| event.identity)
        .max_by_key(|identity| identity.sequence)
        .or_else(|| checkpoint_progress_for_feed(checkpoint, feed).last_event)
}

fn updates_cover_feed(
    sources: &BTreeSet<ReputationSourceV1>,
    feed: ReputationCommittedFeedV1,
) -> bool {
    match feed {
        ReputationCommittedFeedV1::Journal => [
            ReputationSourceV1::Por,
            ReputationSourceV1::Dispute,
            ReputationSourceV1::Token,
        ]
        .into_iter()
        .all(|source| sources.contains(&source)),
        _ => sources.contains(&primary_source_for_feed(feed)),
    }
}

const fn signal_matches_feed(signal: ReputationSignalV1, feed: ReputationCommittedFeedV1) -> bool {
    match signal {
        ReputationSignalV1::Pdp { .. } | ReputationSignalV1::Potr { .. } => {
            matches!(feed, ReputationCommittedFeedV1::Proof)
        }
        ReputationSignalV1::Por { .. }
        | ReputationSignalV1::Dispute { .. }
        | ReputationSignalV1::Token { .. } => {
            matches!(feed, ReputationCommittedFeedV1::Journal)
        }
        ReputationSignalV1::Repair { .. } => {
            matches!(feed, ReputationCommittedFeedV1::Repair)
        }
        ReputationSignalV1::ProviderObserved { .. } => {
            matches!(
                feed,
                ReputationCommittedFeedV1::Orderbook | ReputationCommittedFeedV1::Reserve
            )
        }
        ReputationSignalV1::Noop => matches!(
            feed,
            ReputationCommittedFeedV1::Journal
                | ReputationCommittedFeedV1::Orderbook
                | ReputationCommittedFeedV1::Reserve
        ),
    }
}

const fn signal_is_well_formed(signal: ReputationSignalV1) -> bool {
    match signal {
        ReputationSignalV1::Noop
        | ReputationSignalV1::ProviderObserved { .. }
        | ReputationSignalV1::Pdp { .. }
        | ReputationSignalV1::Dispute { .. } => true,
        ReputationSignalV1::Por {
            counts_for_provider,
            ..
        }
        | ReputationSignalV1::Token {
            counts_for_provider,
            ..
        } => counts_for_provider,
        ReputationSignalV1::Potr {
            counts_for_provider,
            success,
            latency_healthy,
            ..
        } => (counts_for_provider || !success) && (!latency_healthy || success),
        ReputationSignalV1::Repair {
            terminal,
            breach,
            slashing,
            ..
        } => terminal || !breach && !slashing,
    }
}

fn validate_known_block_hashes<I>(identities: I) -> Result<(), ReputationIngestError>
where
    I: IntoIterator<Item = ReputationCommittedEventIdentityV1>,
{
    let mut hashes = BTreeMap::new();
    for identity in identities {
        if hashes
            .insert(identity.block_height, identity.block_hash)
            .is_some_and(|previous| previous != identity.block_hash)
        {
            return Err(ReputationIngestError::InvalidCheckpoint);
        }
    }
    Ok(())
}

const fn finalized_identity_as_event_identity(
    identity: ReputationFinalizedIdentityV1,
) -> ReputationCommittedEventIdentityV1 {
    ReputationCommittedEventIdentityV1 {
        sequence: u64::MAX,
        block_height: identity.height,
        block_hash: identity.block_hash,
        event_index: u32::MAX,
    }
}

fn hash_canonical<T: norito::NoritoSerialize>(
    domain: &'static [u8],
    value: &T,
) -> Result<[u8; 32], ReputationIngestError> {
    let bytes = norito::to_bytes(value).map_err(|_| ReputationIngestError::CanonicalEncoding)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    update_len_prefixed(&mut hasher, &bytes)?;
    Ok(*hasher.finalize().as_bytes())
}

fn unsigned_material_digest(
    material: &ReputationUnsignedSigningMaterialV1,
) -> Result<[u8; 32], ReputationIngestError> {
    hash_canonical(b"sorafs-reputation-unsigned-material-delivery-v1", material)
}

fn canonical_signed_result_digest(
    signed_result: &SignedReputationSnapshotV1,
) -> Result<[u8; 32], ReputationIngestError> {
    let canonical = signed_result
        .canonical_bytes()
        .map_err(|_| ReputationIngestError::InvalidDeliveryReceipt)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs-reputation-signed-material-result-v1");
    update_len_prefixed(&mut hasher, &canonical)?;
    Ok(*hasher.finalize().as_bytes())
}

fn update_len_prefixed(
    hasher: &mut blake3::Hasher,
    bytes: &[u8],
) -> Result<(), ReputationIngestError> {
    let len = u64::try_from(bytes.len()).map_err(|_| ReputationIngestError::CapacityExceeded)?;
    hasher.update(&len.to_le_bytes());
    hasher.update(bytes);
    Ok(())
}

const fn feed_event_domain(feed: ReputationCommittedFeedV1) -> &'static [u8] {
    match feed {
        ReputationCommittedFeedV1::Proof => b"sorafs-reputation-proof-event-v1",
        ReputationCommittedFeedV1::Journal => b"sorafs-reputation-journal-event-v1",
        ReputationCommittedFeedV1::Repair => b"sorafs-reputation-repair-event-v1",
        ReputationCommittedFeedV1::Orderbook => b"sorafs-reputation-orderbook-event-v1",
        ReputationCommittedFeedV1::Reserve => b"sorafs-reputation-reserve-event-v1",
    }
}

fn saturating_increment(counter: &AtomicU64) {
    saturating_add(counter, 1);
}

fn saturating_add(counter: &AtomicU64, value: u64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(value))
    });
}

fn checked_unix_millis_to_seconds(unix_ms: u64) -> Option<u64> {
    unix_ms.checked_div(1_000).filter(|seconds| *seconds != 0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ed25519_dalek::{Signer, SigningKey};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        events::data::sorafs::{
            SorafsOrderbookLedgerEvent, SorafsOrderbookLedgerEventKind, SorafsRepairLedgerEvent,
        },
        sorafs::{
            capacity::{CapacityDisputeId, CapacityDisputeOutcome},
            moderation_ledger::{RepairFinalizedCursorV1, RepairFinalizedEventCursorV1},
            orderbook::{OrderbookFinalizedCursorV1, OrderbookFinalizedEventCursorV1},
            pin_registry::{ManifestDigest, StorageClass},
            proof_ledger::{
                PROOF_OUTCOME_RECORD_VERSION_V1, PdpOutcomeProjectionV1, PotrOutcomeProjectionV1,
                ProofOutcomeEd25519AttestationV1, ProofOutcomeFinalizedCursorV1,
                ProofOutcomeFinalizedEventCursorV1, ProofOutcomeRecordV1,
            },
            reputation::{
                PorTerminalOutcomeV1, ProviderDisputeEventV1, ProviderDisputeKindV1,
                ProviderDisputeResolutionV1, REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
                ReputationJournalAuthorityPolicyV1, ReputationJournalEntryV1,
                ReputationJournalFinalizedCursorV1, StreamTokenValidationBindingV1,
                StreamTokenValidationOutcomeV1,
            },
            reserve::{
                ReserveDuration, ReserveFinalizedCursorV1, ReserveProviderAccountV1,
                ReserveProviderTermsV1, ReserveTier,
            },
        },
    };
    use sorafs_manifest::deal::XorQuantity;
    use sorafs_manifest::reputation::signed::{
        REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1, REPUTATION_TRUSTED_SIGNER_VERSION_V1,
        ReputationSnapshotSignatureV1, ReputationTrustedSignerV1,
        SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
    };
    use tempfile::TempDir;

    use super::*;

    const TARGET_HEIGHT: u64 = 10;
    const TARGET_HASH: [u8; 32] = [0xA1; 32];
    const FINALIZED_AT_MS: u64 = 1_800_000_010_000;

    fn network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(
            HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(Hash::new(
                vec![seed; 32],
            )),
        )
    }

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed.max(1); 32], Algorithm::Ed25519)
            .expect("derive deterministic account key");
        AccountId::new(keypair.public_key().clone())
    }

    fn provider(seed: u8) -> ProviderId {
        ProviderId::new([seed.max(1); 32])
    }

    fn trust_policy() -> ReputationSnapshotTrustPolicyV1 {
        let signing_key = SigningKey::from_bytes(&[0x5A; 32]);
        ReputationSnapshotTrustPolicyV1 {
            version: REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1,
            policy_id: [0x91; 32],
            valid_from_unix: FINALIZED_AT_MS / 1_000 - 1_000,
            valid_until_unix: FINALIZED_AT_MS / 1_000 + 100_000,
            max_snapshot_age_secs: 3_600,
            max_future_skew_secs: 60,
            min_signatures: 1,
            signers: vec![ReputationTrustedSignerV1 {
                version: REPUTATION_TRUSTED_SIGNER_VERSION_V1,
                signer_id: "external-threshold-signer-1".to_owned(),
                public_key: signing_key.verifying_key().to_bytes(),
            }],
            revoked_signer_ids: Vec::new(),
        }
    }

    fn policy_for_trust_policy(
        trust_policy: &ReputationSnapshotTrustPolicyV1,
    ) -> ReputationIngestPolicyV1 {
        ReputationIngestPolicyV1::strict_v1(
            network_id(0x61),
            1,
            TARGET_HEIGHT,
            trust_policy
                .canonical_digest()
                .expect("canonical trust policy digest"),
            ReputationWeightsV1::default(),
        )
    }

    fn policy() -> ReputationIngestPolicyV1 {
        policy_for_trust_policy(&trust_policy())
    }

    fn proof_event(
        sequence: u64,
        block_height: u64,
        block_hash: [u8; 32],
        event_index: u32,
        provider_id: ProviderId,
        outcome_marker: u8,
    ) -> ProofOutcomeFinalizedEventV1 {
        ProofOutcomeFinalizedEventV1 {
            sequence,
            block_height,
            block_hash,
            event_index,
            outcome: ProofOutcomeRecordV1 {
                version: PROOF_OUTCOME_RECORD_VERSION_V1,
                identity_digest: [outcome_marker; 32],
                outcome_digest: [outcome_marker.wrapping_add(1); 32],
                provider_id,
                manifest_digest: ManifestDigest::new([0x41; 32]),
                admission_envelope_digest: [0x42; 32],
                submitted_by: account(5),
                committed_at_unix_ms: FINALIZED_AT_MS
                    .saturating_sub((TARGET_HEIGHT - block_height) * 1_000),
                projection: ProofOutcomeProjectionV1::Pdp(PdpOutcomeProjectionV1 {
                    source_sequence: sequence,
                    epoch_id: block_height,
                    status: PdpOutcomeStatusV1::Accepted,
                    proof_digest: Some([0x51; 32]),
                    provider_attestation: Some(ProofOutcomeEd25519AttestationV1 {
                        public_key: [0x52; 32],
                        signature: [0x53; 64],
                    }),
                    sampled_segments: 1,
                    sampled_hot_leaves: 1,
                    sampled_bytes: 1,
                    issued_at_unix: FINALIZED_AT_MS / 1_000 - 2,
                    response_deadline_unix: FINALIZED_AT_MS / 1_000 - 1,
                    decided_at_unix: FINALIZED_AT_MS / 1_000,
                }),
            },
        }
    }

    fn potr_event(
        sequence: u64,
        block_height: u64,
        block_hash: [u8; 32],
        event_index: u32,
        provider_id: ProviderId,
        outcome_marker: u8,
    ) -> ProofOutcomeFinalizedEventV1 {
        ProofOutcomeFinalizedEventV1 {
            sequence,
            block_height,
            block_hash,
            event_index,
            outcome: ProofOutcomeRecordV1 {
                version: PROOF_OUTCOME_RECORD_VERSION_V1,
                identity_digest: [outcome_marker; 32],
                outcome_digest: [outcome_marker.wrapping_add(1); 32],
                provider_id,
                manifest_digest: ManifestDigest::new([0x41; 32]),
                admission_envelope_digest: [0x42; 32],
                submitted_by: account(5),
                committed_at_unix_ms: FINALIZED_AT_MS
                    .saturating_sub((TARGET_HEIGHT - block_height) * 1_000),
                projection: ProofOutcomeProjectionV1::Potr(PotrOutcomeProjectionV1 {
                    status: PotrOutcomeStatusV1::Success,
                    deadline_ms: 250,
                    latency_ms: 100,
                    requested_at_ms: FINALIZED_AT_MS - 4_000,
                    responded_at_ms: FINALIZED_AT_MS - 3_900,
                    recorded_at_ms: FINALIZED_AT_MS - 3_500,
                    range_start: 0,
                    range_end: 63,
                    gateway_public_key: [0x54; 32],
                    governed_provider_key_digest: [0x55; 32],
                    canonical_signed_receipt: vec![0x56; 64],
                }),
            },
        }
    }

    fn journal_policy() -> ReputationJournalAuthorityPolicyV1 {
        ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: account(9),
            dispute_recorder_authority: account(10),
            token_recorder_authority: account(11),
            max_source_age_ms: 24 * 60 * 60 * 1_000,
        }
    }

    fn por_journal_entry(
        provider_id: ProviderId,
        marker: u8,
        source_time_unix_ms: u64,
    ) -> ReputationJournalEntryV1 {
        let policy = journal_policy();
        ReputationJournalEntryV1::try_new(
            provider_id,
            policy.canonical_digest().expect("journal policy digest"),
            policy.por_recorder_authority,
            source_time_unix_ms,
            None,
            ReputationJournalPayloadV1::PorTerminal(PorTerminalOutcomeV1 {
                challenge_id: [marker.max(1); 32],
                manifest_digest: [0xA2; 32],
                epoch_id: 7,
                drand_round: 11,
                forced: false,
                sample_count: 4,
                failed_samples: 0,
                issued_at_unix_ms: source_time_unix_ms - 2_000,
                deadline_at_unix_ms: source_time_unix_ms - 500,
                responded_at_unix_ms: Some(source_time_unix_ms - 750),
                decided_at_unix_ms: source_time_unix_ms,
                proof_digest: Some([0xA3; 32]),
                repair_task_id: None,
                verifier_latency_ms: Some(17),
                status: PorTerminalStatusV1::Verified,
            }),
        )
        .expect("valid PoR journal entry")
    }

    fn token_journal_entry(
        provider_id: ProviderId,
        marker: u8,
        source_time_unix_ms: u64,
    ) -> ReputationJournalEntryV1 {
        let policy = journal_policy();
        ReputationJournalEntryV1::try_new(
            provider_id,
            policy.canonical_digest().expect("journal policy digest"),
            policy.token_recorder_authority,
            source_time_unix_ms,
            None,
            ReputationJournalPayloadV1::StreamTokenValidation(StreamTokenValidationOutcomeV1 {
                binding: StreamTokenValidationBindingV1 {
                    gateway_id: [marker.max(1); 32],
                    gateway_sequence: 1,
                    request_context_digest: [0xB2; 32],
                },
                token_body_digest: Some([0xB3; 32]),
                token_key_version: Some(1),
                validated_at_unix_ms: source_time_unix_ms,
                status: StreamTokenValidationStatusV1::Accepted,
            }),
        )
        .expect("valid stream-token journal entry")
    }

    fn opened_dispute_entry(
        provider_id: ProviderId,
        marker: u8,
        source_time_unix_ms: u64,
    ) -> ReputationJournalEntryV1 {
        let policy = journal_policy();
        ReputationJournalEntryV1::try_new(
            provider_id,
            policy.canonical_digest().expect("journal policy digest"),
            policy.dispute_recorder_authority,
            source_time_unix_ms,
            None,
            ReputationJournalPayloadV1::ProviderDispute(ProviderDisputeEventV1 {
                dispute_id: CapacityDisputeId::new([marker.max(1); 32]),
                kind: ProviderDisputeKindV1::ProofFailure,
                evidence_digest: [marker.wrapping_add(1).max(1); 32],
                submitted_at_unix_ms: source_time_unix_ms,
                status: ProviderDisputeStatusV1::Opened,
            }),
        )
        .expect("valid opened dispute entry")
    }

    fn resolved_dispute_entry(
        provider_id: ProviderId,
        opened: &ReputationJournalEntryV1,
        source_time_unix_ms: u64,
        outcome: CapacityDisputeOutcome,
    ) -> ReputationJournalEntryV1 {
        let policy = journal_policy();
        let ReputationJournalPayloadV1::ProviderDispute(dispute) = &opened.payload else {
            panic!("opened dispute payload")
        };
        ReputationJournalEntryV1::try_new(
            provider_id,
            policy.canonical_digest().expect("journal policy digest"),
            policy.dispute_recorder_authority,
            source_time_unix_ms,
            Some(opened.event_id),
            ReputationJournalPayloadV1::ProviderDispute(ProviderDisputeEventV1 {
                dispute_id: dispute.dispute_id,
                kind: dispute.kind,
                evidence_digest: dispute.evidence_digest,
                submitted_at_unix_ms: dispute.submitted_at_unix_ms,
                status: ProviderDisputeStatusV1::Resolved(ProviderDisputeResolutionV1 {
                    outcome,
                    resolved_at_unix_ms: source_time_unix_ms,
                    decision_digest: [0xC3; 32],
                    rationale: None,
                }),
            }),
        )
        .expect("valid resolved dispute entry")
    }

    fn journal_event(
        sequence: u64,
        event_index: u32,
        entry: ReputationJournalEntryV1,
    ) -> ReputationJournalFinalizedEventV1 {
        let recorded_at_unix_ms = entry.source_time_unix_ms.saturating_add(100);
        ReputationJournalFinalizedEventV1 {
            sequence,
            block_height: 9,
            block_hash: [0x91; 32],
            event_index,
            recorded_at_unix_ms,
            entry,
        }
    }

    fn journal_page(
        events: Vec<ReputationJournalFinalizedEventV1>,
    ) -> ReputationJournalFinalizedEventPageV1 {
        ReputationJournalFinalizedEventPageV1 {
            finalized_cursor: ReputationJournalFinalizedCursorV1 {
                height: TARGET_HEIGHT,
                block_hash: TARGET_HASH,
                finalized_at_unix_ms: FINALIZED_AT_MS,
            },
            events,
            has_more: false,
            next_after: None,
        }
    }

    fn journal_only_batch(
        pages: Vec<ReputationJournalFinalizedEventPageV1>,
    ) -> ReputationFinalizedBatchV1 {
        ReputationFinalizedBatchV1 {
            journal_pages: pages,
            ..empty_batch()
        }
    }

    fn proof_page(
        target_hash: [u8; 32],
        events: Vec<ProofOutcomeFinalizedEventV1>,
    ) -> ProofOutcomeFinalizedEventPageV1 {
        ProofOutcomeFinalizedEventPageV1 {
            finalized_cursor: ProofOutcomeFinalizedCursorV1 {
                height: TARGET_HEIGHT,
                block_hash: target_hash,
            },
            events,
            has_more: false,
            next_after: None,
        }
    }

    fn proof_only_batch(
        target_hash: [u8; 32],
        events: Vec<ProofOutcomeFinalizedEventV1>,
    ) -> ReputationFinalizedBatchV1 {
        ReputationFinalizedBatchV1 {
            proof_pages: vec![proof_page(target_hash, events)],
            ..empty_batch()
        }
    }

    fn empty_batch() -> ReputationFinalizedBatchV1 {
        ReputationFinalizedBatchV1 {
            network_id: network_id(0x61),
            finalized_at_unix_ms: FINALIZED_AT_MS,
            proof_pages: Vec::new(),
            journal_pages: Vec::new(),
            repair_pages: Vec::new(),
            orderbook_pages: Vec::new(),
            reserve_pages: Vec::new(),
            reserve_provider_pages: Vec::new(),
        }
    }

    fn repair_page(provider_id: ProviderId) -> RepairFinalizedEventPageV1 {
        RepairFinalizedEventPageV1 {
            finalized_cursor: RepairFinalizedCursorV1 {
                height: TARGET_HEIGHT,
                block_hash: TARGET_HASH,
            },
            events: vec![RepairFinalizedEventV1 {
                sequence: 1,
                block_height: 7,
                block_hash: [0x71; 32],
                event_index: 0,
                event: SorafsRepairLedgerEvent {
                    kind: SorafsRepairLedgerEventKind::Failed,
                    ticket_id: "ticket-1".to_owned(),
                    task_id: [0x72; 32],
                    provider_id,
                    manifest_digest: ManifestDigest::new([0x73; 32]),
                    revision: 2,
                    authority: account(6),
                    occurred_at_unix_ms: FINALIZED_AT_MS - 3_000,
                },
            }],
            has_more: false,
            next_after: None,
        }
    }

    fn orderbook_page(provider_id: ProviderId) -> OrderbookFinalizedEventPageV1 {
        OrderbookFinalizedEventPageV1 {
            finalized_cursor: OrderbookFinalizedCursorV1 {
                height: TARGET_HEIGHT,
                block_hash: TARGET_HASH,
            },
            events: vec![OrderbookFinalizedEventV1 {
                sequence: 1,
                block_height: 8,
                block_hash: [0x81; 32],
                event_index: 0,
                event: SorafsOrderbookLedgerEvent {
                    kind: SorafsOrderbookLedgerEventKind::ReceiptRecorded,
                    order_id: None,
                    trade_id: Some([0x82; 32]),
                    channel_id: Some([0x83; 32]),
                    receipt_id: Some([0x84; 32]),
                    provider_id: Some(provider_id),
                    book_revision: 2,
                    authority: account(7),
                    occurred_at_unix_ms: FINALIZED_AT_MS - 2_000,
                },
            }],
            has_more: false,
            next_after: None,
        }
    }

    fn reserve_event_page() -> ReserveFinalizedEventPageV1 {
        ReserveFinalizedEventPageV1 {
            finalized_cursor: ReserveFinalizedCursorV1 {
                height: TARGET_HEIGHT,
                block_hash: TARGET_HASH,
            },
            events: Vec::new(),
            has_more: false,
            next_after: None,
        }
    }

    fn reserve_provider_page(provider_id: ProviderId) -> ReserveProviderAccountPageV1 {
        ReserveProviderAccountPageV1 {
            finalized_cursor: ReserveFinalizedCursorV1 {
                height: TARGET_HEIGHT,
                block_hash: TARGET_HASH,
            },
            accounts: vec![ReserveProviderAccountV1 {
                terms: ReserveProviderTermsV1 {
                    provider_id,
                    provider_account: account(8),
                    tier: ReserveTier::TierA,
                    storage_class: StorageClass::Hot,
                    duration: ReserveDuration::Monthly,
                    capacity_gib: 64,
                },
                policy_digest: [0x92; 32],
                revision: 1,
                reserve_balance: XorQuantity::zero(),
                debt_principal: XorQuantity::zero(),
                accrued_interest: XorQuantity::zero(),
                credit_cap: XorQuantity::zero(),
                lifecycle_stage: ReserveLifecycleStage::Active,
                days_past_due: 0,
                pending_movements: 0,
                open_appeals: 0,
                rent_charged_through_unix: FINALIZED_AT_MS / 1_000,
                interest_accrued_at_unix: FINALIZED_AT_MS / 1_000,
                updated_at_unix: FINALIZED_AT_MS / 1_000,
            }],
            has_more: false,
            next_after: None,
        }
    }

    fn all_sources_batch(provider_id: ProviderId) -> ReputationFinalizedBatchV1 {
        let opened_a = opened_dispute_entry(provider_id, 0x61, FINALIZED_AT_MS - 1_800);
        let opened_b = opened_dispute_entry(provider_id, 0x62, FINALIZED_AT_MS - 1_600);
        let resolved_a = resolved_dispute_entry(
            provider_id,
            &opened_a,
            FINALIZED_AT_MS - 1_400,
            CapacityDisputeOutcome::Upheld,
        );
        ReputationFinalizedBatchV1 {
            network_id: network_id(0x61),
            finalized_at_unix_ms: FINALIZED_AT_MS,
            proof_pages: vec![proof_page(
                TARGET_HASH,
                vec![
                    proof_event(1, 6, [0x61; 32], 0, provider_id, 0x11),
                    potr_event(2, 6, [0x61; 32], 1, provider_id, 0x12),
                ],
            )],
            journal_pages: vec![journal_page(vec![
                journal_event(
                    1,
                    0,
                    por_journal_entry(provider_id, 0x21, FINALIZED_AT_MS - 2_000),
                ),
                journal_event(2, 1, opened_a),
                journal_event(3, 2, opened_b),
                journal_event(4, 3, resolved_a),
                journal_event(
                    5,
                    4,
                    token_journal_entry(provider_id, 0x22, FINALIZED_AT_MS - 1_000),
                ),
            ])],
            repair_pages: vec![repair_page(provider_id)],
            orderbook_pages: vec![orderbook_page(provider_id)],
            reserve_pages: vec![reserve_event_page()],
            reserve_provider_pages: vec![reserve_provider_page(provider_id)],
        }
    }

    fn signed_material_result(
        material: &ReputationUnsignedSigningMaterialV1,
    ) -> SignedReputationSnapshotV1 {
        signed_material_result_with_signer(
            material,
            "external-threshold-signer-1",
            &SigningKey::from_bytes(&[0x5A; 32]),
        )
    }

    fn signed_material_result_with_signer(
        material: &ReputationUnsignedSigningMaterialV1,
        signer_id: &str,
        signing_key: &SigningKey,
    ) -> SignedReputationSnapshotV1 {
        SignedReputationSnapshotV1 {
            version: SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
            policy_digest: material.snapshot_trust_policy_digest,
            snapshot: material.snapshot.clone(),
            scoring_evidence_digest: material.scoring_evidence_digest,
            scoring_evidence: material.scoring_evidence.clone(),
            signatures: vec![ReputationSnapshotSignatureV1 {
                signer_id: signer_id.to_owned(),
                signature: signing_key
                    .sign(&material.snapshot_signing_digest)
                    .to_bytes(),
            }],
        }
    }

    fn signed_material_result_generated_at(
        material: &ReputationUnsignedSigningMaterialV1,
        generated_at_unix: u64,
    ) -> SignedReputationSnapshotV1 {
        let snapshot = sorafs_manifest::reputation::build_reputation_snapshot_with_trust_edges(
            material.snapshot.snapshot_id,
            generated_at_unix,
            material.snapshot.weights,
            &material.scoring_evidence.provider_inputs,
            &material.scoring_evidence.trust_edges,
            material.snapshot.previous_snapshot_id,
        )
        .expect("build time-adjusted snapshot");
        let mut adjusted = material.clone();
        adjusted.snapshot_signing_digest = snapshot_signing_digest(
            &snapshot,
            adjusted.snapshot_trust_policy_digest,
            adjusted.scoring_evidence_digest,
        )
        .expect("time-adjusted signing digest");
        adjusted.snapshot = snapshot;
        signed_material_result(&adjusted)
    }

    fn ready_material_service(
        verification_policy: &ReputationSnapshotTrustPolicyV1,
        provider_seed: u8,
    ) -> (
        TempDir,
        ReputationIngestService,
        ReputationUnsignedMaterialDeliveryV1,
    ) {
        let root = TempDir::new().expect("ready material root");
        let service = ReputationIngestService::open(
            root.path(),
            policy_for_trust_policy(verification_policy),
        )
        .expect("open ready material service");
        service
            .ingest_finalized_batch(all_sources_batch(provider(provider_seed)))
            .expect("complete ready material sources");
        service
            .enqueue_unsigned_signing_material()
            .expect("enqueue ready material");
        let delivery = service
            .unsigned_material_delivery()
            .expect("read ready material")
            .expect("pending ready material");
        (root, service, delivery)
    }

    #[test]
    fn exact_replay_is_idempotent_and_byte_identical() {
        let root = TempDir::new().expect("state root");
        let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
        let provider_id = provider(1);
        let batch = proof_only_batch(
            TARGET_HASH,
            vec![proof_event(1, 6, [0x61; 32], 0, provider_id, 0x11)],
        );
        assert_eq!(
            service
                .ingest_finalized_batch(batch.clone())
                .expect("apply event"),
            ReputationIngestOutcomeV1::Applied { events: 1 }
        );
        let before = service
            .canonical_checkpoint_bytes()
            .expect("canonical checkpoint");
        assert_eq!(
            service
                .ingest_finalized_batch(batch)
                .expect("accept exact replay"),
            ReputationIngestOutcomeV1::ExactReplay
        );
        assert_eq!(
            service
                .canonical_checkpoint_bytes()
                .expect("canonical replay checkpoint"),
            before
        );
        assert_eq!(service.metrics().exact_replays, 1);
    }

    #[test]
    fn unified_journal_replay_is_byte_identical_and_advances_all_semantic_sources() {
        let root = TempDir::new().expect("journal state root");
        let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
        let provider_id = provider(12);
        let batch = journal_only_batch(vec![journal_page(vec![
            journal_event(
                1,
                0,
                por_journal_entry(provider_id, 0x31, FINALIZED_AT_MS - 1_000),
            ),
            journal_event(
                2,
                1,
                token_journal_entry(provider_id, 0x32, FINALIZED_AT_MS - 500),
            ),
        ])]);
        assert_eq!(
            service
                .ingest_finalized_batch(batch.clone())
                .expect("apply journal"),
            ReputationIngestOutcomeV1::Applied { events: 2 }
        );
        let before = service
            .canonical_checkpoint_bytes()
            .expect("journal checkpoint");
        {
            let state = service.state.lock().expect("journal state");
            for source in [
                ReputationSourceV1::Por,
                ReputationSourceV1::Dispute,
                ReputationSourceV1::Token,
            ] {
                let progress = state.checkpoint.progress(source);
                assert_eq!(progress.last_event.expect("journal cursor").sequence, 2);
                assert_eq!(
                    progress.observed_through,
                    Some(ReputationFinalizedIdentityV1 {
                        height: TARGET_HEIGHT,
                        block_hash: TARGET_HASH,
                    })
                );
            }
            let accumulator = state.checkpoint.providers.first().expect("provider");
            assert_eq!(accumulator.por_total, 1);
            assert_eq!(accumulator.por_successes, 1);
            assert_eq!(accumulator.token_observations, 1);
            assert_eq!(accumulator.token_violations, 0);
        }
        let cursors = service
            .committed_feed_cursors()
            .expect("public committed-feed cursors");
        assert_eq!(cursors.len(), ALL_COMMITTED_FEEDS.len());
        let journal_cursor = cursors
            .iter()
            .find(|cursor| cursor.feed == ReputationCommittedFeedV1::Journal)
            .expect("unified journal cursor");
        assert_eq!(journal_cursor.after.expect("journal after").sequence, 2);
        assert_eq!(
            journal_cursor.observed_through,
            Some(ReputationFinalizedIdentityV1 {
                height: TARGET_HEIGHT,
                block_hash: TARGET_HASH,
            })
        );
        assert_eq!(
            service
                .ingest_finalized_batch(batch)
                .expect("exact journal replay"),
            ReputationIngestOutcomeV1::ExactReplay
        );
        assert_eq!(
            service
                .canonical_checkpoint_bytes()
                .expect("replayed journal checkpoint"),
            before
        );
        assert_eq!(service.metrics().exact_replays, 2);
    }

    #[test]
    fn journal_gap_fork_equivocation_and_cross_page_continuation_fail_closed() {
        let provider_id = provider(13);

        let gap_root = TempDir::new().expect("journal gap root");
        let gap = ReputationIngestService::open(gap_root.path(), policy()).expect("open gap");
        assert_eq!(
            gap.ingest_finalized_batch(journal_only_batch(vec![journal_page(vec![
                journal_event(
                    2,
                    0,
                    por_journal_entry(provider_id, 0x41, FINALIZED_AT_MS - 1_000),
                ),
            ])]))
            .expect_err("global journal gap rejected"),
            ReputationIngestError::EventGap
        );

        let paged_root = TempDir::new().expect("journal paged root");
        let paged = ReputationIngestService::open(paged_root.path(), policy()).expect("open paged");
        let first_event = journal_event(
            1,
            0,
            por_journal_entry(provider_id, 0x42, FINALIZED_AT_MS - 1_000),
        );
        let mut first_page = journal_page(vec![first_event.clone()]);
        first_page.has_more = true;
        first_page.next_after = Some(first_event.cursor());
        let second_page = journal_page(vec![journal_event(
            2,
            1,
            token_journal_entry(provider_id, 0x43, FINALIZED_AT_MS - 500),
        )]);
        assert_eq!(
            paged
                .ingest_finalized_batch(journal_only_batch(vec![first_page, second_page]))
                .expect("contiguous journal pages"),
            ReputationIngestOutcomeV1::Applied { events: 2 }
        );

        let conflict_root = TempDir::new().expect("journal conflict root");
        let conflict =
            ReputationIngestService::open(conflict_root.path(), policy()).expect("open conflict");
        conflict
            .ingest_finalized_batch(journal_only_batch(vec![journal_page(vec![journal_event(
                1,
                0,
                por_journal_entry(provider_id, 0x44, FINALIZED_AT_MS - 1_000),
            )])]))
            .expect("apply journal baseline");
        assert_eq!(
            conflict
                .ingest_finalized_batch(journal_only_batch(vec![journal_page(vec![
                    journal_event(
                        1,
                        0,
                        por_journal_entry(provider_id, 0x45, FINALIZED_AT_MS - 1_000),
                    ),
                ])]))
                .expect_err("same journal sequence substitution rejected"),
            ReputationIngestError::EventEquivocation
        );
        let mut fork_page = journal_page(Vec::new());
        fork_page.finalized_cursor.block_hash = [0xD1; 32];
        assert_eq!(
            conflict
                .ingest_finalized_batch(journal_only_batch(vec![fork_page]))
                .expect_err("same-height journal fork rejected"),
            ReputationIngestError::FinalizedFork
        );
    }

    #[test]
    fn multiple_disputes_preserve_active_cardinality_across_pages() {
        let root = TempDir::new().expect("dispute state root");
        let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
        let provider_id = provider(14);
        let opened_a = opened_dispute_entry(provider_id, 0x51, FINALIZED_AT_MS - 1_600);
        let opened_b = opened_dispute_entry(provider_id, 0x52, FINALIZED_AT_MS - 1_400);
        let resolved_a = resolved_dispute_entry(
            provider_id,
            &opened_a,
            FINALIZED_AT_MS - 1_200,
            CapacityDisputeOutcome::Upheld,
        );
        let resolved_b = resolved_dispute_entry(
            provider_id,
            &opened_b,
            FINALIZED_AT_MS - 1_000,
            CapacityDisputeOutcome::Dismissed,
        );
        let third = journal_event(3, 2, resolved_a);
        let mut partial = journal_page(vec![
            journal_event(1, 0, opened_a),
            journal_event(2, 1, opened_b),
            third.clone(),
        ]);
        partial.has_more = true;
        partial.next_after = Some(third.cursor());
        service
            .ingest_finalized_batch(journal_only_batch(vec![partial]))
            .expect("apply partial dispute journal");
        {
            let state = service.state.lock().expect("partial state");
            let accumulator = state.checkpoint.providers.first().expect("provider");
            assert_eq!(accumulator.active_disputes, 1);
            assert!(accumulator.has_active_dispute());
            assert_eq!(accumulator.disputes_resolved, 1);
            assert_eq!(accumulator.disputes_upheld, 1);
        }

        service
            .ingest_finalized_batch(journal_only_batch(vec![journal_page(vec![journal_event(
                4, 3, resolved_b,
            )])]))
            .expect("resolve remaining dispute");
        let state = service.state.lock().expect("resolved state");
        let accumulator = state.checkpoint.providers.first().expect("provider");
        assert_eq!(accumulator.active_disputes, 0);
        assert!(!accumulator.has_active_dispute());
        assert_eq!(accumulator.disputes_resolved, 2);
        assert_eq!(accumulator.disputes_upheld, 1);
    }

    #[test]
    fn reordered_gap_fork_and_equivocation_fail_closed() {
        let provider_id = provider(2);

        let reordered_root = TempDir::new().expect("reordered root");
        let reordered =
            ReputationIngestService::open(reordered_root.path(), policy()).expect("open service");
        let error = reordered
            .ingest_finalized_batch(proof_only_batch(
                TARGET_HASH,
                vec![
                    proof_event(2, 6, [0x61; 32], 1, provider_id, 0x12),
                    proof_event(1, 6, [0x61; 32], 0, provider_id, 0x11),
                ],
            ))
            .expect_err("reordered events rejected");
        assert_eq!(error, ReputationIngestError::EventReordered);

        let gap_root = TempDir::new().expect("gap root");
        let gap = ReputationIngestService::open(gap_root.path(), policy()).expect("open service");
        let error = gap
            .ingest_finalized_batch(proof_only_batch(
                TARGET_HASH,
                vec![proof_event(2, 6, [0x61; 32], 0, provider_id, 0x12)],
            ))
            .expect_err("gap rejected");
        assert_eq!(error, ReputationIngestError::EventGap);

        let conflict_root = TempDir::new().expect("conflict root");
        let conflict =
            ReputationIngestService::open(conflict_root.path(), policy()).expect("open service");
        let accepted = proof_event(1, 6, [0x61; 32], 0, provider_id, 0x11);
        conflict
            .ingest_finalized_batch(proof_only_batch(TARGET_HASH, vec![accepted.clone()]))
            .expect("apply baseline");
        let error = conflict
            .ingest_finalized_batch(proof_only_batch(
                [0xB1; 32],
                vec![ProofOutcomeFinalizedEventV1 {
                    block_hash: [0x61; 32],
                    ..accepted.clone()
                }],
            ))
            .expect_err("same-height finalized fork rejected");
        assert_eq!(error, ReputationIngestError::FinalizedFork);

        let mut substituted = accepted;
        substituted.outcome.outcome_digest = [0xEE; 32];
        let error = conflict
            .ingest_finalized_batch(proof_only_batch(TARGET_HASH, vec![substituted]))
            .expect_err("same sequence with substituted payload rejected");
        assert_eq!(error, ReputationIngestError::EventEquivocation);

        let error = conflict
            .ingest_finalized_batch(proof_only_batch(
                TARGET_HASH,
                vec![proof_event(2, 7, [0x71; 32], 0, provider_id, 0x12)],
            ))
            .expect_err("completed feed cannot append at the same finalized anchor");
        assert_eq!(error, ReputationIngestError::EventEquivocation);
    }

    #[test]
    fn source_omission_blocks_material_until_global_journal_is_complete() {
        let root = TempDir::new().expect("state root");
        let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
        let provider_id = provider(3);
        service
            .ingest_finalized_batch(proof_only_batch(
                TARGET_HASH,
                vec![proof_event(1, 6, [0x61; 32], 0, provider_id, 0x11)],
            ))
            .expect("apply proof source");
        assert_eq!(
            service
                .unsigned_signing_material()
                .expect_err("available source omissions block material"),
            ReputationIngestError::MissingRequiredSources
        );

        let ready_root = TempDir::new().expect("ready state root");
        let ready =
            ReputationIngestService::open(ready_root.path(), policy()).expect("open ready service");
        ready
            .ingest_finalized_batch(all_sources_batch(provider_id))
            .expect("complete every source");
        let status = ready.status().expect("ready status");
        assert_eq!(
            status.missing_sources,
            ReputationRequiredSourceMaskV1::EMPTY
        );
        let material = ready
            .unsigned_signing_material()
            .expect("all native source material is ready");
        let journal_finality = material
            .source_finality
            .iter()
            .filter(|row| {
                matches!(
                    row.source,
                    ReputationSourceV1::Por
                        | ReputationSourceV1::Dispute
                        | ReputationSourceV1::Token
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(journal_finality.len(), 3);
        assert!(journal_finality.windows(2).all(|pair| {
            pair[0].observed_through == pair[1].observed_through
                && pair[0].last_event == pair[1].last_event
        }));
        let provider_input = material
            .scoring_evidence
            .provider_inputs
            .first()
            .expect("scored provider");
        assert!(provider_input.active_dispute);
        assert_eq!(provider_input.metrics.por_success_bps, 10_000);
        assert_eq!(provider_input.metrics.dispute_rate_bps, 10_000);
        assert_eq!(provider_input.metrics.token_violation_rate_bps, 0);
    }

    #[test]
    fn reserve_completion_requires_same_anchor_stage_projection() {
        let root = TempDir::new().expect("state root");
        let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
        let error = service
            .ingest_finalized_batch(ReputationFinalizedBatchV1 {
                reserve_pages: vec![reserve_event_page()],
                ..empty_batch()
            })
            .expect_err("missing reserve provider projection rejected");
        assert_eq!(error, ReputationIngestError::ReserveStageResolutionMissing);
    }

    #[test]
    fn pagination_cannot_complete_with_an_empty_promised_tail() {
        let root = TempDir::new().expect("state root");
        let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
        let event = proof_event(1, 6, [0x61; 32], 0, provider(9), 0x11);
        let mut first = proof_page(TARGET_HASH, vec![event.clone()]);
        first.has_more = true;
        first.next_after = Some(event.cursor());
        assert_eq!(
            service
                .ingest_finalized_batch(ReputationFinalizedBatchV1 {
                    proof_pages: vec![first, proof_page(TARGET_HASH, Vec::new())],
                    ..empty_batch()
                })
                .expect_err("empty page cannot satisfy a promise of more events"),
            ReputationIngestError::InvalidPage
        );
    }

    #[test]
    fn finalized_batch_from_same_label_foreign_network_is_rejected() {
        let root = TempDir::new().expect("state root");
        let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
        let mut batch = proof_only_batch(
            TARGET_HASH,
            vec![proof_event(1, 6, [0x61; 32], 0, provider(9), 0x11)],
        );
        batch.network_id = network_id(0x62);
        assert_eq!(
            service
                .ingest_finalized_batch(batch)
                .expect_err("same-label foreign-network batch rejected"),
            ReputationIngestError::NetworkIdMismatch
        );
    }

    #[test]
    fn restart_reconciles_a_durably_staged_batch() {
        let root = TempDir::new().expect("state root");
        let provider_id = provider(4);
        {
            let service =
                ReputationIngestService::open(root.path(), policy()).expect("open service");
            let batch = proof_only_batch(
                TARGET_HASH,
                vec![proof_event(1, 6, [0x61; 32], 0, provider_id, 0x11)],
            );
            let mut state = service.state.lock().expect("runtime state");
            let prepared = prepare_pending_batch(&state.checkpoint, &batch, &service.policy)
                .expect("prepare staged batch");
            let mut staged = state.checkpoint.clone();
            staged.pending = Some(prepared.pending);
            let fingerprint = service
                .commit_checkpoint(&staged, state.fingerprint)
                .expect("persist staged batch");
            state.checkpoint = staged;
            state.fingerprint = Some(fingerprint);
        }

        let restored =
            ReputationIngestService::open(root.path(), policy()).expect("reconcile restart");
        let status = restored.status().expect("restored status");
        assert_eq!(status.pending_events, 0);
        assert_eq!(status.providers, 1);
        assert_eq!(restored.metrics().restart_reconciliations, 1);
        let proof_cursor = restored
            .committed_feed_cursors()
            .expect("restored public cursors")
            .into_iter()
            .find(|cursor| cursor.feed == ReputationCommittedFeedV1::Proof)
            .expect("proof cursor");
        assert_eq!(proof_cursor.after.expect("proof after").sequence, 1);
        assert_eq!(
            proof_cursor.observed_through,
            Some(ReputationFinalizedIdentityV1 {
                height: TARGET_HEIGHT,
                block_hash: TARGET_HASH,
            })
        );
        let state = restored.state.lock().expect("restored state");
        assert_eq!(
            state
                .checkpoint
                .progress(ReputationSourceV1::Proof)
                .last_event
                .unwrap()
                .sequence,
            1
        );
    }

    #[test]
    fn corrupt_checkpoint_and_batch_bound_are_rejected() {
        let root = TempDir::new().expect("corrupt state root");
        let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
        service
            .ingest_finalized_batch(proof_only_batch(
                TARGET_HASH,
                vec![proof_event(1, 6, [0x61; 32], 0, provider(5), 0x11)],
            ))
            .expect("create checkpoint");
        drop(service);
        fs::write(
            root.path().join(REPUTATION_INGEST_CHECKPOINT_FILE_NAME_V1),
            b"poisoned reputation checkpoint",
        )
        .expect("poison checkpoint");
        assert_eq!(
            ReputationIngestService::open(root.path(), policy())
                .expect_err("corrupt checkpoint rejected"),
            ReputationIngestError::InvalidCheckpoint
        );

        let bounded_root = TempDir::new().expect("bounded state root");
        let mut bounded_policy = policy();
        bounded_policy.max_pending_events = 1;
        let bounded = ReputationIngestService::open(bounded_root.path(), bounded_policy)
            .expect("open bounded service");
        let provider_id = provider(6);
        assert_eq!(
            bounded
                .ingest_finalized_batch(proof_only_batch(
                    TARGET_HASH,
                    vec![
                        proof_event(1, 6, [0x61; 32], 0, provider_id, 0x11),
                        proof_event(2, 6, [0x61; 32], 1, provider_id, 0x12),
                    ],
                ))
                .expect_err("pending bound enforced"),
            ReputationIngestError::CapacityExceeded
        );

        let byte_bounded_root = TempDir::new().expect("byte-bounded state root");
        let mut byte_bounded_policy = policy();
        byte_bounded_policy.checkpoint_max_bytes = REPUTATION_INGEST_MIN_CHECKPOINT_BYTES_V1;
        let byte_bounded =
            ReputationIngestService::open(byte_bounded_root.path(), byte_bounded_policy)
                .expect("open byte-bounded service");
        let mut page = repair_page(provider(6));
        page.events[0].event.ticket_id = "x".repeat(
            usize::try_from(REPUTATION_INGEST_MIN_CHECKPOINT_BYTES_V1)
                .expect("minimum byte cap fits usize"),
        );
        assert_eq!(
            byte_bounded
                .ingest_finalized_batch(ReputationFinalizedBatchV1 {
                    repair_pages: vec![page],
                    ..empty_batch()
                })
                .expect_err("aggregate encoded-page byte bound enforced"),
            ReputationIngestError::CapacityExceeded
        );
    }

    #[test]
    fn poisoned_runtime_blocks_all_authoritative_reads() {
        let root = TempDir::new().expect("state root");
        let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
        service.durability_poisoned.store(true, Ordering::Release);
        assert_eq!(
            service.status().expect_err("status fails closed"),
            ReputationIngestError::CheckpointDurabilityUncertain
        );
        assert_eq!(
            service
                .canonical_checkpoint_bytes()
                .expect_err("checkpoint read fails closed"),
            ReputationIngestError::CheckpointDurabilityUncertain
        );
        assert_eq!(
            service
                .unsigned_signing_material()
                .expect_err("material read fails closed"),
            ReputationIngestError::CheckpointDurabilityUncertain
        );
        assert_eq!(
            service
                .committed_feed_cursors()
                .expect_err("cursor read fails closed"),
            ReputationIngestError::CheckpointDurabilityUncertain
        );
        assert_eq!(
            service
                .unsigned_material_delivery()
                .expect_err("outbox read fails closed"),
            ReputationIngestError::CheckpointDurabilityUncertain
        );
        assert_eq!(
            service
                .enqueue_unsigned_signing_material()
                .expect_err("outbox mutation fails closed"),
            ReputationIngestError::CheckpointDurabilityUncertain
        );
    }

    #[test]
    fn replicas_produce_identical_checkpoint_and_unsigned_material_bytes() {
        let left_root = TempDir::new().expect("left root");
        let right_root = TempDir::new().expect("right root");
        let left = ReputationIngestService::open(left_root.path(), policy()).expect("open left");
        let right = ReputationIngestService::open(right_root.path(), policy()).expect("open right");
        let batch = all_sources_batch(provider(7));
        left.ingest_finalized_batch(batch.clone())
            .expect("ingest left");
        right.ingest_finalized_batch(batch).expect("ingest right");
        assert_eq!(
            left.canonical_checkpoint_bytes().expect("left bytes"),
            right.canonical_checkpoint_bytes().expect("right bytes")
        );

        let left_material = left
            .unsigned_signing_material()
            .expect("left unsigned material");
        let right_material = right
            .unsigned_signing_material()
            .expect("right unsigned material");
        assert_eq!(left_material, right_material);
        assert_eq!(
            norito::to_bytes(&left_material).expect("left material bytes"),
            norito::to_bytes(&right_material).expect("right material bytes")
        );
    }

    #[test]
    fn material_outbox_retry_dead_letter_and_acknowledgement_are_restart_safe() {
        let root = TempDir::new().expect("material outbox root");
        let verification_policy = trust_policy();
        let mut ingest_policy = policy();
        ingest_policy.max_material_delivery_failures = 2;
        let material_digest;
        let signed_result;
        {
            let service = ReputationIngestService::open(root.path(), ingest_policy.clone())
                .expect("open material outbox");
            service
                .ingest_finalized_batch(all_sources_batch(provider(15)))
                .expect("complete committed sources");
            assert_eq!(
                service
                    .enqueue_unsigned_signing_material()
                    .expect("enqueue deterministic material"),
                ReputationMaterialEnqueueOutcomeV1::Enqueued { sequence: 1 }
            );
            let delivery = service
                .unsigned_material_delivery()
                .expect("read delivery")
                .expect("pending delivery");
            material_digest = delivery.material_digest;
            signed_result = signed_material_result(&delivery.material);
            assert_eq!(delivery.failed_attempts, 0);
            assert_eq!(
                delivery.state,
                ReputationUnsignedMaterialDeliveryStateV1::Pending
            );
            assert_eq!(
                service
                    .record_unsigned_material_delivery_failure(1, material_digest, [0xD1; 32],)
                    .expect("record first failure"),
                ReputationMaterialFailureOutcomeV1::RetryPending {
                    failed_attempts: 1,
                    remaining_attempts: 1,
                }
            );
        }

        {
            let restored = ReputationIngestService::open(root.path(), ingest_policy.clone())
                .expect("restore pending material");
            let delivery = restored
                .unsigned_material_delivery()
                .expect("read restored delivery")
                .expect("restored pending delivery");
            assert_eq!(delivery.material_digest, material_digest);
            assert_eq!(delivery.failed_attempts, 1);
            assert_eq!(
                restored
                    .record_unsigned_material_delivery_failure(1, material_digest, [0xD1; 32],)
                    .expect("failure receipt replay"),
                ReputationMaterialFailureOutcomeV1::ExactReplay {
                    failed_attempts: 1,
                    state: ReputationUnsignedMaterialDeliveryStateV1::Pending,
                }
            );
            assert_eq!(
                restored
                    .record_unsigned_material_delivery_failure(1, material_digest, [0xD2; 32],)
                    .expect("record terminal failure"),
                ReputationMaterialFailureOutcomeV1::DeadLettered { failed_attempts: 2 }
            );
        }

        {
            let restored = ReputationIngestService::open(root.path(), ingest_policy.clone())
                .expect("restore dead letter");
            let delivery = restored
                .unsigned_material_delivery()
                .expect("read dead letter")
                .expect("dead-letter delivery");
            assert_eq!(
                delivery.state,
                ReputationUnsignedMaterialDeliveryStateV1::DeadLetter
            );
            assert_eq!(
                restored
                    .record_unsigned_material_delivery_failure(1, material_digest, [0xD3; 32],)
                    .expect_err("dead letter rejects a new retry"),
                ReputationIngestError::MaterialDeliveryDeadLettered
            );
            assert_eq!(
                restored
                    .acknowledge_unsigned_material(
                        1,
                        material_digest,
                        &signed_result,
                        &verification_policy,
                    )
                    .expect("late exact acknowledgement remains admissible"),
                ReputationMaterialAcknowledgementOutcomeV1::Acknowledged
            );
            assert!(
                restored
                    .unsigned_material_delivery()
                    .expect("read cleared outbox")
                    .is_none()
            );
        }

        let restored =
            ReputationIngestService::open(root.path(), ingest_policy).expect("restore ack");
        assert_eq!(
            restored
                .acknowledge_unsigned_material(
                    1,
                    material_digest,
                    &signed_result,
                    &verification_policy,
                )
                .expect("acknowledgement replay"),
            ReputationMaterialAcknowledgementOutcomeV1::ExactReplay
        );
        assert_eq!(
            restored
                .enqueue_unsigned_signing_material()
                .expect("enqueue replay after acknowledgement"),
            ReputationMaterialEnqueueOutcomeV1::AlreadyAcknowledged { sequence: 1 }
        );
        let acknowledgement = restored
            .unsigned_material_acknowledgement()
            .expect("read acknowledgement")
            .expect("durable acknowledgement");
        assert_eq!(
            acknowledgement.signed_result_digest,
            canonical_signed_result_digest(&signed_result).expect("signed result digest")
        );
        assert_eq!(
            acknowledgement.trust_policy_digest,
            verification_policy
                .canonical_digest()
                .expect("trust policy digest")
        );
        assert_eq!(acknowledgement.verified_at_unix, FINALIZED_AT_MS / 1_000);
    }

    #[test]
    fn material_outbox_corruption_and_substituted_receipts_fail_closed() {
        let root = TempDir::new().expect("material corruption root");
        let ingest_policy = policy();
        let mut checkpoint;
        let material_digest;
        {
            let service = ReputationIngestService::open(root.path(), ingest_policy.clone())
                .expect("open material outbox");
            service
                .ingest_finalized_batch(all_sources_batch(provider(16)))
                .expect("complete committed sources");
            service
                .enqueue_unsigned_signing_material()
                .expect("enqueue material");
            let delivery = service
                .unsigned_material_delivery()
                .expect("read delivery")
                .expect("pending delivery");
            material_digest = delivery.material_digest;
            let mut substituted_result = signed_material_result(&delivery.material);
            substituted_result.policy_digest = [0xF4; 32];
            assert_eq!(
                service
                    .record_unsigned_material_delivery_failure(1, [0xF1; 32], [0xF2; 32])
                    .expect_err("substituted material digest rejected"),
                ReputationIngestError::MaterialDeliveryMismatch
            );
            assert_eq!(
                service
                    .acknowledge_unsigned_material(
                        1,
                        material_digest,
                        &substituted_result,
                        &trust_policy(),
                    )
                    .expect_err("substituted signed result rejected"),
                ReputationIngestError::MaterialDeliveryMismatch
            );
            checkpoint = service
                .state
                .lock()
                .expect("checkpoint state")
                .checkpoint
                .clone();
        }

        checkpoint
            .material_outbox
            .as_mut()
            .expect("outbox entry")
            .material_digest = [0xF3; 32];
        fs::write(
            root.path().join(REPUTATION_INGEST_CHECKPOINT_FILE_NAME_V1),
            norito::to_bytes(&checkpoint).expect("encode corrupt checkpoint"),
        )
        .expect("write corrupt checkpoint");
        assert_eq!(
            ReputationIngestService::open(root.path(), ingest_policy)
                .expect_err("substituted outbox material rejected"),
            ReputationIngestError::InvalidCheckpoint
        );
    }

    #[test]
    fn material_acknowledgement_checkpoint_binds_authoritative_finalized_time() {
        let verification_policy = trust_policy();
        let ingest_policy = policy_for_trust_policy(&verification_policy);
        let (root, service, delivery) = ready_material_service(&verification_policy, 17);
        let signed_result = signed_material_result(&delivery.material);
        service
            .acknowledge_unsigned_material(
                1,
                delivery.material_digest,
                &signed_result,
                &verification_policy,
            )
            .expect("acknowledge exact externally signed material");
        let mut checkpoint = service
            .state
            .lock()
            .expect("checkpoint state")
            .checkpoint
            .clone();
        drop(service);

        checkpoint
            .material_acknowledgement
            .as_mut()
            .expect("material acknowledgement")
            .verified_at_unix += 1;
        fs::write(
            root.path().join(REPUTATION_INGEST_CHECKPOINT_FILE_NAME_V1),
            norito::to_bytes(&checkpoint).expect("encode timestamp-substituted checkpoint"),
        )
        .expect("write timestamp-substituted checkpoint");
        assert_eq!(
            ReputationIngestService::open(root.path(), ingest_policy)
                .expect_err("substituted acknowledgement time rejected"),
            ReputationIngestError::InvalidCheckpoint
        );
    }

    #[test]
    fn material_ack_rejects_forged_revoked_duplicate_quorum_and_time_failures() {
        let mut base_policy = trust_policy();
        base_policy.valid_from_unix =
            FINALIZED_AT_MS / 1_000 - base_policy.max_snapshot_age_secs - 100;
        let (_root, service, delivery) = ready_material_service(&base_policy, 18);

        let forged = signed_material_result_with_signer(
            &delivery.material,
            "external-threshold-signer-1",
            &SigningKey::from_bytes(&[0x5B; 32]),
        );
        assert_eq!(
            service
                .acknowledge_unsigned_material(1, delivery.material_digest, &forged, &base_policy,)
                .expect_err("forged signature rejected"),
            ReputationIngestError::InvalidDeliveryReceipt
        );

        let mut duplicate = signed_material_result(&delivery.material);
        duplicate.signatures.push(duplicate.signatures[0].clone());
        assert_eq!(
            service
                .acknowledge_unsigned_material(
                    1,
                    delivery.material_digest,
                    &duplicate,
                    &base_policy,
                )
                .expect_err("duplicate signer rejected"),
            ReputationIngestError::InvalidDeliveryReceipt
        );

        let stale = signed_material_result_generated_at(
            &delivery.material,
            FINALIZED_AT_MS / 1_000 - base_policy.max_snapshot_age_secs - 1,
        );
        assert_eq!(
            service
                .acknowledge_unsigned_material(1, delivery.material_digest, &stale, &base_policy,)
                .expect_err("stale signed result rejected"),
            ReputationIngestError::InvalidDeliveryReceipt
        );
        let future = signed_material_result_generated_at(
            &delivery.material,
            FINALIZED_AT_MS / 1_000 + base_policy.max_future_skew_secs + 1,
        );
        assert_eq!(
            service
                .acknowledge_unsigned_material(1, delivery.material_digest, &future, &base_policy,)
                .expect_err("future signed result rejected"),
            ReputationIngestError::InvalidDeliveryReceipt
        );
        assert!(
            service
                .unsigned_material_delivery()
                .expect("failed acknowledgements retain outbox")
                .is_some()
        );

        let second_signer = ReputationTrustedSignerV1 {
            version: REPUTATION_TRUSTED_SIGNER_VERSION_V1,
            signer_id: "external-threshold-signer-2".to_owned(),
            public_key: SigningKey::from_bytes(&[0x5C; 32])
                .verifying_key()
                .to_bytes(),
        };

        let mut revoked_policy = trust_policy();
        revoked_policy.signers.push(second_signer.clone());
        revoked_policy.revoked_signer_ids = vec!["external-threshold-signer-1".to_owned()];
        let (_revoked_root, revoked_service, revoked_delivery) =
            ready_material_service(&revoked_policy, 19);
        let revoked_result = signed_material_result(&revoked_delivery.material);
        assert_eq!(
            revoked_service
                .acknowledge_unsigned_material(
                    1,
                    revoked_delivery.material_digest,
                    &revoked_result,
                    &revoked_policy,
                )
                .expect_err("revoked signer rejected"),
            ReputationIngestError::InvalidDeliveryReceipt
        );

        let mut quorum_policy = trust_policy();
        quorum_policy.signers.push(second_signer);
        quorum_policy.min_signatures = 2;
        let (_quorum_root, quorum_service, quorum_delivery) =
            ready_material_service(&quorum_policy, 20);
        let insufficient_quorum = signed_material_result(&quorum_delivery.material);
        assert_eq!(
            quorum_service
                .acknowledge_unsigned_material(
                    1,
                    quorum_delivery.material_digest,
                    &insufficient_quorum,
                    &quorum_policy,
                )
                .expect_err("insufficient quorum rejected"),
            ReputationIngestError::InvalidDeliveryReceipt
        );
    }

    #[test]
    fn public_failures_are_payload_free() {
        let provider_id = provider(17);
        let root = TempDir::new().expect("payload-free error root");
        let service = ReputationIngestService::open(root.path(), policy()).expect("open service");
        let error = service
            .ingest_finalized_batch(proof_only_batch(
                TARGET_HASH,
                vec![proof_event(2, 6, [0x61; 32], 0, provider_id, 0xAB)],
            ))
            .expect_err("gap rejected");
        let display = error.to_string();
        let debug = format!("{error:?}");
        for private_marker in [
            hex::encode(provider_id.as_bytes()),
            hex::encode([0x61; 32]),
            hex::encode([0xAB; 32]),
        ] {
            assert!(!display.contains(&private_marker));
            assert!(!debug.contains(&private_marker));
        }
    }

    #[test]
    fn checked_integer_paths_reject_overflow_without_wrapping() {
        let provider_id = provider(8);
        let mut accumulator = ProviderAccumulatorV1::new(provider_id);
        accumulator.pdp_total = u64::MAX;
        assert_eq!(
            accumulator
                .apply(ReputationSignalV1::Pdp {
                    provider_id,
                    success: false,
                })
                .expect_err("counter overflow rejected"),
            ReputationIngestError::ArithmeticOverflow
        );
        assert_eq!(accumulator.pdp_total, u64::MAX);

        let mut por = ProviderAccumulatorV1::new(provider_id);
        por.por_total = u64::MAX;
        assert_eq!(
            por.apply(ReputationSignalV1::Por {
                provider_id,
                counts_for_provider: true,
                success: false,
            })
            .expect_err("PoR counter overflow rejected"),
            ReputationIngestError::ArithmeticOverflow
        );
        assert_eq!(por.por_total, u64::MAX);

        let mut token = ProviderAccumulatorV1::new(provider_id);
        token.token_observations = u64::MAX;
        assert_eq!(
            token
                .apply(ReputationSignalV1::Token {
                    provider_id,
                    counts_for_provider: true,
                    violation: false,
                })
                .expect_err("token counter overflow rejected"),
            ReputationIngestError::ArithmeticOverflow
        );
        assert_eq!(token.token_observations, u64::MAX);

        let mut dispute = ProviderAccumulatorV1::new(provider_id);
        assert_eq!(
            dispute
                .apply(ReputationSignalV1::Dispute {
                    provider_id,
                    transition: ReputationDisputeSignalV1::Resolved { upheld: true },
                })
                .expect_err("dispute cardinality underflow rejected"),
            ReputationIngestError::ArithmeticOverflow
        );
        assert_eq!(dispute.active_disputes, 0);
        assert_eq!(dispute.disputes_resolved, 0);
        dispute.active_disputes = u64::MAX;
        assert_eq!(
            dispute
                .apply(ReputationSignalV1::Dispute {
                    provider_id,
                    transition: ReputationDisputeSignalV1::Opened,
                })
                .expect_err("dispute cardinality overflow rejected"),
            ReputationIngestError::ArithmeticOverflow
        );
        assert_eq!(dispute.active_disputes, u64::MAX);
        assert_eq!(ratio_bps(u64::MAX, u64::MAX).expect("wide ratio"), 10_000);
        assert_eq!(
            zero_safe_ratio_bps(1, 0).expect_err("invalid zero denominator"),
            ReputationIngestError::ArithmeticOverflow
        );
    }

    #[test]
    fn event_cursor_constructors_remain_exact() {
        let proof = ProofOutcomeFinalizedEventCursorV1 {
            sequence: 9,
            block_height: 8,
            block_hash: [0x88; 32],
            event_index: 7,
        };
        let repair = RepairFinalizedEventCursorV1 {
            sequence: proof.sequence,
            block_height: proof.block_height,
            block_hash: proof.block_hash,
            event_index: proof.event_index,
        };
        let orderbook = OrderbookFinalizedEventCursorV1 {
            sequence: proof.sequence,
            block_height: proof.block_height,
            block_hash: proof.block_hash,
            event_index: proof.event_index,
        };
        assert_eq!(
            ReputationCommittedEventIdentityV1 {
                sequence: repair.sequence,
                block_height: repair.block_height,
                block_hash: repair.block_hash,
                event_index: repair.event_index,
            },
            ReputationCommittedEventIdentityV1 {
                sequence: orderbook.sequence,
                block_height: orderbook.block_height,
                block_hash: orderbook.block_hash,
                event_index: orderbook.event_index,
            }
        );
    }
}
