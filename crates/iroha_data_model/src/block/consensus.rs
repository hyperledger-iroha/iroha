//! Norito-encoded consensus message types shared across Sumeragi implementations.
//!
//! These types cover QC voting (prepare/commit/new-view), evidence, VRF
//! commit/reveal envelopes, and optional reliable broadcast
//! helpers. They are split out of `iroha_core` so that other crates (e.g.,
//! Torii, genesis tooling, or test harnesses) can construct and inspect
//! consensus payloads without depending on the core runtime crate.

use core::fmt;
use std::{string::String, vec::Vec};

use iroha_crypto::{Hash, HashOf};
use iroha_schema::{EnumMeta, EnumVariant, Ident, IntoSchema, MetaMap, Metadata, TypeId};
use norito::codec::{Decode, DecodeAll, Encode};

use super::Header as BlockHeader;
use crate::{
    fastpq::{FastpqTransitionBatch, TransferTranscriptBundle},
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    peer::PeerId,
    transaction::TransactionSubmissionReceipt,
};

/// Wire protocol version for consensus messages defined here.
pub const PROTO_VERSION: u32 = 1;

/// Mode tag for classic permissioned Sumeragi used in handshakes and hashing domains.
pub const PERMISSIONED_TAG: &str = "iroha2-consensus::permissioned-sumeragi@v1";
/// Mode tag for `NPoS` Sumeragi used in handshakes and hashing domains.
pub const NPOS_TAG: &str = "iroha2-consensus::npos-sumeragi@v1";

/// Height alias for consensus.
pub type Height = u64;
/// View/round number alias.
pub type View = u64;
/// Validator index within the active set.
pub type ValidatorIndex = u32;

/// Canonical consensus parameters included in the genesis fingerprint.
///
/// These parameters are encoded with Norito (binary) in a fixed order to
/// guarantee determinism across peers and platforms.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ConsensusGenesisParams {
    /// Maximal amount of time a leader waits before proposing (ms).
    pub block_time_ms: u64,
    /// Maximal amount of time to reach commit (ms).
    pub commit_time_ms: u64,
    /// Allowed clock drift (ms) for transaction admission.
    pub max_clock_drift_ms: u64,
    /// Number of aggregators (collectors) targeted per block.
    pub collectors_k: u16,
    /// Redundant send fanout per validator (distinct collectors).
    pub redundant_send_r: u8,
    /// Block sizing: max transactions per block.
    pub block_max_transactions: u64,
    /// Data availability enabled (RBC transport; consensus does not gate on DA).
    pub da_enabled: bool,
    /// Epoch length in blocks (`NPoS` mode; 0 in permissioned).
    pub epoch_length_blocks: u64,
    /// BLS domain separation string used for QC-vote signatures.
    pub bls_domain: String,
    /// Optional NPoS-specific configuration captured at genesis.
    #[norito(default)]
    pub npos: Option<NposGenesisParams>,
}

/// `NPoS`-specific consensus parameters hashed into the genesis fingerprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NposGenesisParams {
    /// Target block time for `NPoS` mode (ms).
    pub block_time_ms: u64,
    /// Proposal timeout window (ms).
    pub timeout_propose_ms: u64,
    /// Prevote aggregation timeout (ms).
    pub timeout_prevote_ms: u64,
    /// Precommit aggregation timeout (ms).
    pub timeout_precommit_ms: u64,
    /// Commit finalization timeout (ms).
    pub timeout_commit_ms: u64,
    /// Data-availability timeout (ms).
    pub timeout_da_ms: u64,
    /// Aggregator fallback timeout (ms).
    pub timeout_aggregator_ms: u64,
    /// Number of aggregators (K) per round.
    pub k_aggregators: u16,
    /// Redundant send fanout (distinct aggregators contacted over time).
    pub redundant_send_r: u8,
    /// Deterministic epoch seed for PRF-based leader and collector selection.
    pub epoch_seed: [u8; 32],
    /// VRF commit window length in blocks.
    pub vrf_commit_window_blocks: u64,
    /// VRF reveal window length in blocks.
    pub vrf_reveal_window_blocks: u64,
    /// Maximum validators to elect for the next epoch (0 = unlimited).
    pub max_validators: u32,
    /// Minimum self-bond required for validator eligibility.
    pub min_self_bond: u64,
    /// Minimum nomination bond required for delegators.
    pub min_nomination_bond: u64,
    /// Maximum nominator concentration percentage.
    pub max_nominator_concentration_pct: u8,
    /// Seat allocation variance band percentage.
    pub seat_band_pct: u8,
    /// Maximum correlation percentage across validator entities.
    pub max_entity_correlation_pct: u8,
    /// Finality margin in blocks before activating a newly elected set.
    pub finality_margin_blocks: u64,
    /// Evidence retention horizon in blocks.
    pub evidence_horizon_blocks: u64,
    /// Activation lag in blocks for newly scheduled validator sets.
    pub activation_lag_blocks: u64,
}

/// Consensus certificate phases (BLS-only).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "phase", content = "detail", rename_all = "snake_case")]
pub enum CertPhase {
    /// Prepare/lock certificate for a proposal.
    Prepare = 1,
    /// Commit QC for finalization.
    Commit = 2,
    /// New-view certificate for view change.
    NewView = 3,
}

impl TypeId for CertPhase {
    fn id() -> Ident {
        "CertPhase".to_owned()
    }
}

impl IntoSchema for CertPhase {
    fn type_name() -> Ident {
        "CertPhase".to_owned()
    }

    fn update_schema_map(metamap: &mut MetaMap) {
        let variants = vec![
            EnumVariant {
                tag: "Prepare".to_owned(),
                discriminant: CertPhase::Prepare as u8,
                ty: None,
            },
            EnumVariant {
                tag: "Commit".to_owned(),
                discriminant: CertPhase::Commit as u8,
                ty: None,
            },
            EnumVariant {
                tag: "NewView".to_owned(),
                discriminant: CertPhase::NewView as u8,
                ty: None,
            },
        ];
        metamap.insert::<Self>(Metadata::Enum(EnumMeta { variants }));
    }
}

/// Reference to an existing QC header for embedding in proposals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct QcRef {
    /// Height of the certified block.
    pub height: Height,
    /// View in which the certificate was formed.
    pub view: View,
    /// Epoch index (0 in permissioned mode).
    pub epoch: u64,
    /// Block hash certified by the certificate.
    pub subject_block_hash: HashOf<BlockHeader>,
    /// Phase certified by the certificate.
    pub phase: CertPhase,
}

/// Block header fields essential for consensus (proposal header subset).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct ConsensusBlockHeader {
    /// Parent block hash.
    pub parent_hash: HashOf<BlockHeader>,
    /// Merkle root of included transactions.
    pub tx_root: Hash,
    /// State commitment after executing the block.
    pub state_root: Hash,
    /// Proposer index within the active validator set.
    pub proposer: ValidatorIndex,
    /// Block height.
    pub height: Height,
    /// Consensus view/round number.
    pub view: View,
    /// Epoch index for `NPoS`. Zero in permissioned builds.
    pub epoch: u64,
    /// Embedded reference to the highest QC known to the proposer.
    pub highest_qc: QcRef,
}

/// Proposal message with payload commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct Proposal {
    /// Proposal header (consensus-relevant subset).
    pub header: ConsensusBlockHeader,
    /// Hash of the full block payload (DA). Used for availability tracking.
    pub payload_hash: Hash,
}

/// QC vote over a specific block and phase (BLS-only).
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct QcVote {
    /// Target phase (`Prepare`, `Commit`, `NewView`).
    pub phase: CertPhase,
    /// Hash of the block being voted on.
    pub block_hash: HashOf<BlockHeader>,
    /// Parent state root bound into the QC vote.
    pub parent_state_root: Hash,
    /// Post-state root bound into the QC vote.
    pub post_state_root: Hash,
    /// Block height of the subject.
    pub height: Height,
    /// View number of the vote.
    pub view: View,
    /// Epoch index for `NPoS`; 0 in permissioned.
    pub epoch: u64,
    /// Highest known QC for `NewView` votes (advisory; not signed).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_qc: Option<QcRef>,
    /// Signer index within the active validator set.
    pub signer: ValidatorIndex,
    /// BLS signature over the canonical QC-vote preimage.
    pub bls_sig: Vec<u8>,
}

/// BLS aggregate signature envelope with signer bitmap for constant-size certificates.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct QcAggregate {
    /// Compact signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate signature bytes (compressed).
    pub bls_aggregate_signature: Vec<u8>,
}

/// QC certifying a phase for a block.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct Qc {
    /// Phase certified by this certificate.
    pub phase: CertPhase,
    /// Block hash certified by the certificate.
    pub subject_block_hash: HashOf<BlockHeader>,
    /// Parent state root bound into the QC.
    pub parent_state_root: Hash,
    /// Post-state root bound into the QC.
    pub post_state_root: Hash,
    /// Height of the subject block.
    pub height: Height,
    /// View in which the certificate was formed.
    pub view: View,
    /// Epoch index.
    pub epoch: u64,
    /// Consensus mode tag used to domain-separate signatures.
    pub mode_tag: String,
    /// Highest known QC that justifies a `NewView` QC (advisory; not signed).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_qc: Option<QcRef>,
    /// Stable hash of the validator set that produced the certificate.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Ordered validator set used when assembling the certificate.
    pub validator_set: Vec<PeerId>,
    /// Aggregate signature and signer bitmap.
    pub aggregate: QcAggregate,
}

/// Evidence kinds for slashing or governance penalties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub enum EvidenceKind {
    /// Same (height, view) prepare vote on different blocks
    DoublePrepare = 0,
    /// Same (height, view) commit vote on different blocks
    DoubleCommit = 1,
    /// Invalid QC
    InvalidQc = 2,
    /// Invalid proposal
    InvalidProposal = 3,
    /// Transaction censorship proof (submission receipts).
    Censorship = 4,
}

/// Evidence payloads.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub enum EvidencePayload {
    /// Two votes by the same signer for the same (phase, height, view, epoch)
    /// on different block hashes.
    DoubleVote {
        /// First observed vote.
        v1: QcVote,
        /// Second observed vote.
        v2: QcVote,
    },
    /// A QC considered invalid by local checks.
    InvalidQc {
        /// The certificate flagged as invalid.
        certificate: Qc,
        /// Human-readable reason describing the invalidity.
        reason: String,
    },
    /// A proposal considered invalid by local checks.
    InvalidProposal {
        /// The proposal flagged as invalid.
        proposal: Proposal,
        /// Human-readable reason describing the invalidity.
        reason: String,
    },
    /// Evidence that a transaction was submitted but not proposed/committed.
    Censorship {
        /// Transaction hash referenced by the receipts.
        tx_hash: HashOf<crate::transaction::SignedTransaction>,
        /// Signed submission receipts from validators.
        receipts: Vec<TransactionSubmissionReceipt>,
    },
}

/// Evidence wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct Evidence {
    /// High-level classification of the evidence.
    pub kind: EvidenceKind,
    /// Detailed payload carrying the offending material.
    pub payload: EvidencePayload,
}

/// Persisted evidence entry annotated with commit metadata.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct EvidenceRecord {
    /// Slashing material captured for governance processing.
    pub evidence: Evidence,
    /// Block height at which this evidence record was appended to WSV.
    pub recorded_at_height: Height,
    /// Consensus view (round) of the block carrying the record.
    pub recorded_at_view: View,
    /// Block creation timestamp in milliseconds since UNIX epoch.
    pub recorded_at_ms: u64,
    /// Whether a penalty was already applied for this evidence record.
    #[norito(default)]
    pub penalty_applied: bool,
    /// Block height at which the penalty was applied, if any.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub penalty_applied_at_height: Option<Height>,
}

/// Membership snapshot exported through `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiMembershipStatus {
    /// Height associated with the snapshot.
    #[norito(default)]
    pub height: u64,
    /// View associated with the snapshot.
    #[norito(default)]
    pub view: u64,
    /// Epoch associated with the snapshot.
    #[norito(default)]
    pub epoch: u64,
    /// Deterministic roster hash for `(height, view, epoch)`.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub view_hash: Option<[u8; 32]>,
}

/// Membership mismatch snapshot exported through `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiMembershipMismatchStatus {
    /// Peers currently flagged for membership mismatches.
    #[norito(default)]
    pub active_peers: Vec<PeerId>,
    /// Last peer observed with a mismatch (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_peer: Option<PeerId>,
    /// Height associated with the last mismatch (best-effort).
    #[norito(default)]
    pub last_height: u64,
    /// View associated with the last mismatch (best-effort).
    #[norito(default)]
    pub last_view: u64,
    /// Epoch associated with the last mismatch (best-effort).
    #[norito(default)]
    pub last_epoch: u64,
    /// Local membership hash observed during the last mismatch (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_local_hash: Option<[u8; 32]>,
    /// Remote membership hash observed during the last mismatch (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_remote_hash: Option<[u8; 32]>,
    /// Milliseconds since UNIX epoch when the last mismatch was recorded.
    #[norito(default)]
    pub last_timestamp_ms: u64,
}

/// Aggregated per-lane commitment summary reported by Sumeragi status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiLaneCommitment {
    /// Block height associated with the commitment.
    pub block_height: u64,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Number of transactions attributed to the lane.
    pub tx_count: u64,
    /// Total RBC chunks allocated to the lane.
    pub total_chunks: u64,
    /// Total RBC payload bytes allocated to the lane.
    pub rbc_bytes_total: u64,
    /// Total TEU allocated to the lane.
    pub teu_total: u64,
    /// Block hash anchoring the commitment.
    pub block_hash: HashOf<BlockHeader>,
}

/// Aggregated per-dataspace commitment summary reported by Sumeragi status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiDataspaceCommitment {
    /// Block height associated with the commitment.
    pub block_height: u64,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Number of transactions attributed to the dataspace.
    pub tx_count: u64,
    /// Total RBC chunks allocated to the dataspace.
    pub total_chunks: u64,
    /// Total RBC payload bytes allocated to the dataspace.
    pub rbc_bytes_total: u64,
    /// Total TEU allocated to the dataspace.
    pub teu_total: u64,
    /// Block hash anchoring the commitment.
    pub block_hash: HashOf<BlockHeader>,
}

/// Deterministic settlement receipt emitted for audit and reconciliation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneSettlementReceipt {
    /// Caller-specified identifier linking the receipt to the originating transaction.
    pub source_id: [u8; 32],
    /// Local gas-token amount debited from the payer (micro units).
    pub local_amount_micro: u128,
    /// XOR amount booked immediately after inclusion (micro units).
    pub xor_due_micro: u128,
    /// XOR amount expected post-haircut (micro units).
    pub xor_after_haircut_micro: u128,
    /// Safety margin consumed by this receipt (`xor_due_micro - xor_after_haircut_micro`).
    pub xor_variance_micro: u128,
    /// UTC timestamp in milliseconds when the receipt was generated.
    pub timestamp_ms: u64,
}

/// Liquidity profile applied when computing XOR conversions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "profile", content = "state")]
pub enum LaneLiquidityProfile {
    /// Deep pools with negligible slippage.
    Tier1,
    /// Medium depth pools with moderate slippage.
    Tier2,
    /// Thin pools or credit-constrained venues.
    Tier3,
}

/// Volatility bucket applied when computing the safety margin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "bucket", content = "state")]
pub enum LaneVolatilityClass {
    /// Normal operating conditions.
    #[default]
    Stable,
    /// Elevated but healthy volatility.
    Elevated,
    /// Dislocated markets requiring maximal margin.
    Dislocated,
}

/// Swap metadata describing the deterministic conversion parameters.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneSwapMetadata {
    /// Basis-point safety margin applied on top of the TWAP.
    pub epsilon_bps: u16,
    /// TWAP window length in seconds.
    pub twap_window_seconds: u32,
    /// Liquidity profile guiding haircut selection.
    pub liquidity_profile: LaneLiquidityProfile,
    /// Human-readable TWAP value (`local_token / XOR`) captured as a decimal string.
    pub twap_local_per_xor: String,
    /// Volatility bucket recorded when applying the epsilon.
    #[norito(default)]
    pub volatility_class: LaneVolatilityClass,
}

/// Aggregated per-lane settlement commitment captured within a block.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneBlockCommitment {
    /// Block height associated with the commitment.
    pub block_height: u64,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Number of transactions contributing settlement receipts.
    pub tx_count: u64,
    /// Total local gas-token amount recorded in the block (micro units).
    pub total_local_micro: u128,
    /// Total XOR due immediately after inclusion (micro units).
    pub total_xor_due_micro: u128,
    /// Total XOR expected after applying liquidity haircuts (micro units).
    pub total_xor_after_haircut_micro: u128,
    /// Aggregate difference between the XOR debited and the post-haircut expectation (micro units).
    pub total_xor_variance_micro: u128,
    /// Deterministic metadata describing the conversion parameters.
    #[norito(default)]
    pub swap_metadata: Option<LaneSwapMetadata>,
    /// Deterministic receipts contributing to the commitment.
    #[norito(default)]
    pub receipts: Vec<LaneSettlementReceipt>,
}

impl<'a> norito::core::DecodeFromSlice<'a> for LaneSwapMetadata {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_from_slice_canonical(bytes)
    }
}

/// Runtime-upgrade governance hook snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiRuntimeUpgradeHook {
    /// Whether runtime-upgrade instructions are allowed.
    pub allow: bool,
    /// Whether runtime-upgrade instructions must include metadata.
    pub require_metadata: bool,
    /// Metadata key enforced by the manifest, if specified.
    #[norito(default)]
    pub metadata_key: Option<String>,
    /// Allowed metadata values when an allowlist is configured.
    #[norito(default)]
    pub allowed_ids: Vec<String>,
}

/// Governance manifest readiness snapshot for a lane.
#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiLaneGovernance {
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Human-readable lane alias.
    pub alias: String,
    /// Governance module configured for the lane, if any.
    #[norito(default)]
    pub governance: Option<String>,
    /// Whether the lane requires a governance manifest.
    pub manifest_required: bool,
    /// Whether a manifest has been loaded and validated.
    pub manifest_ready: bool,
    /// Path of the loaded manifest (best-effort; operator visibility).
    #[norito(default)]
    pub manifest_path: Option<String>,
    /// Validator identifiers derived from the manifest.
    #[norito(default)]
    pub validator_ids: Vec<String>,
    /// Quorum threshold configured by the manifest.
    #[norito(default)]
    pub quorum: Option<u32>,
    /// Protected namespaces enforced by the manifest.
    #[norito(default)]
    pub protected_namespaces: Vec<String>,
    /// Runtime-upgrade governance hook configuration.
    #[norito(default)]
    pub runtime_upgrade: Option<SumeragiRuntimeUpgradeHook>,
}

/// DA availability reason reported by `/v1/sumeragi/status`.
#[allow(missing_copy_implementations)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum SumeragiDaGateReason {
    /// No gate currently blocking commit/finalize.
    #[default]
    None,
    /// Missing local data required to validate the pending block.
    MissingLocalData,
    /// Manifest is missing for the pending commitment.
    ManifestMissing,
    /// Manifest hash mismatched the commitment.
    ManifestHashMismatch,
    /// Manifest could not be read from disk.
    ManifestReadFailed,
    /// Manifest spool could not be scanned.
    ManifestSpoolScan,
}

/// Which DA availability condition was satisfied most recently.
#[allow(missing_copy_implementations)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum SumeragiDaGateSatisfaction {
    /// No condition has been satisfied yet.
    #[default]
    None,
    /// Missing local data was recovered.
    MissingDataRecovered,
}

/// Snapshot of DA availability tracking counters for `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiDaGateStatus {
    /// Most recent reason that reported missing availability evidence.
    pub reason: SumeragiDaGateReason,
    /// Most recent condition that satisfied availability tracking.
    pub last_satisfied: SumeragiDaGateSatisfaction,
    /// Count of times local data was missing.
    pub missing_local_data_total: u64,
    /// Count of times the manifest guard reported missing/invalid manifests.
    #[norito(default)]
    pub manifest_guard_total: u64,
}

/// Snapshot of missing-block fetch attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiMissingBlockFetchStatus {
    /// Total fetch evaluations after QC-first arrival (including backoff/no-target cases).
    pub total: u64,
    /// Target count on the most recent fetch attempt.
    pub last_targets: u64,
    /// Dwell time in milliseconds observed before the most recent fetch attempt.
    pub last_dwell_ms: u64,
}

/// Snapshot of kura persistence failures and retries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiKuraStoreStatus {
    /// Total times a block failed to enqueue for persistence.
    pub failures_total: u64,
    /// Total times kura persistence retries were exhausted for a block.
    pub abort_total: u64,
    /// Total times a block reached the staging phase before persistence.
    #[norito(default)]
    pub stage_total: u64,
    /// Total times a staged commit was rolled back before WSV application.
    #[norito(default)]
    pub rollback_total: u64,
    /// Height of the last staged block (best-effort).
    #[norito(default)]
    pub stage_last_height: u64,
    /// View of the last staged block (best-effort).
    #[norito(default)]
    pub stage_last_view: u64,
    /// Hash of the last staged block (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub stage_last_hash: Option<HashOf<BlockHeader>>,
    /// Height of the last staged commit rolled back (best-effort).
    #[norito(default)]
    pub rollback_last_height: u64,
    /// View of the last staged commit rolled back (best-effort).
    #[norito(default)]
    pub rollback_last_view: u64,
    /// Hash of the last staged commit rolled back (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rollback_last_hash: Option<HashOf<BlockHeader>>,
    /// Reason label for the last rollback (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub rollback_last_reason: Option<String>,
    /// Total times Highest/Locked QC were reset after a kura abort.
    #[norito(default)]
    pub lock_reset_total: u64,
    /// Height associated with the last lock reset (best-effort).
    #[norito(default)]
    pub lock_reset_last_height: u64,
    /// View associated with the last lock reset (best-effort).
    #[norito(default)]
    pub lock_reset_last_view: u64,
    /// Hash associated with the last lock reset (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub lock_reset_last_hash: Option<HashOf<BlockHeader>>,
    /// Reason label for the last lock reset (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub lock_reset_last_reason: Option<String>,
    /// Last observed retry attempt count.
    pub last_retry_attempt: u64,
    /// Last observed retry backoff in milliseconds.
    pub last_retry_backoff_ms: u64,
    /// Height of the last block that failed to persist (best-effort).
    pub last_height: u64,
    /// View of the last block that failed to persist (best-effort).
    pub last_view: u64,
    /// Hash of the last block that failed to persist (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_hash: Option<HashOf<BlockHeader>>,
}

/// Session evicted from the RBC store due to TTL or capacity enforcement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiRbcEvictedSession {
    /// Block hash associated with the evicted session.
    pub block_hash: HashOf<BlockHeader>,
    /// Block height for the evicted session.
    pub height: u64,
    /// View index for the evicted session.
    pub view: u64,
}

impl Default for SumeragiRbcEvictedSession {
    fn default() -> Self {
        Self {
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH])),
            height: 0,
            view: 0,
        }
    }
}

/// Snapshot of the RBC on-disk store state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiRbcStoreStatus {
    /// Current number of persisted RBC sessions on disk.
    pub sessions: u64,
    /// Current persisted RBC payload bytes on disk.
    pub bytes: u64,
    /// Current RBC store pressure level (0 = normal, 1 = soft limit, 2 = hard limit).
    pub pressure_level: u8,
    /// Total number of times proposal assembly was deferred due to RBC store pressure.
    pub backpressure_deferrals_total: u64,
    /// Total number of RBC sessions evicted due to TTL or capacity enforcement.
    pub evictions_total: u64,
    /// Most recent RBC sessions evicted due to TTL or capacity enforcement (bounded list).
    #[norito(default)]
    pub recent_evictions: Vec<SumeragiRbcEvictedSession>,
}

/// Snapshot of pending (pre-INIT) RBC stashes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiPendingRbcEntry {
    /// Block hash associated with the pending session.
    pub block_hash: HashOf<BlockHeader>,
    /// Block height for the pending session.
    pub height: u64,
    /// View index for the pending session.
    pub view: u64,
    /// Number of chunk frames currently buffered.
    pub chunks: u64,
    /// Total chunk payload bytes buffered.
    pub bytes: u64,
    /// Number of READY frames buffered.
    #[norito(default)]
    pub ready: u64,
    /// Number of DELIVER frames buffered.
    #[norito(default)]
    pub deliver: u64,
    /// Chunk frames dropped for this session due to caps.
    #[norito(default)]
    pub dropped_chunks: u64,
    /// Chunk payload bytes dropped for this session due to caps.
    #[norito(default)]
    pub dropped_bytes: u64,
    /// READY frames dropped for this session due to caps.
    #[norito(default)]
    pub dropped_ready: u64,
    /// DELIVER frames dropped for this session due to caps.
    #[norito(default)]
    pub dropped_deliver: u64,
    /// Approximate age (ms) since the first pending message was recorded.
    #[norito(default)]
    pub age_ms: u64,
}

impl Default for SumeragiPendingRbcEntry {
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

/// Aggregated pending RBC stash telemetry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiPendingRbcStatus {
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
    /// Total pending frames dropped across all reasons.
    #[norito(default)]
    pub drops_total: u64,
    /// Total pending frames dropped due to cap/session-cap enforcement.
    #[norito(default)]
    pub drops_cap_total: u64,
    /// Aggregate payload/signature bytes dropped due to caps.
    #[norito(default)]
    pub drops_cap_bytes_total: u64,
    /// Total pending frames dropped due to TTL expiry.
    #[norito(default)]
    pub drops_ttl_total: u64,
    /// Aggregate payload/signature bytes dropped due to TTL expiry.
    #[norito(default)]
    pub drops_ttl_bytes_total: u64,
    /// Total pending bytes dropped across all reasons.
    #[norito(default)]
    pub drops_bytes_total: u64,
    /// Total pending sessions evicted (TTL expiry or stash-cap eviction).
    #[norito(default)]
    pub evicted_total: u64,
    /// Total READY frames stashed before processing.
    #[norito(default)]
    pub stash_ready_total: u64,
    /// READY frames stashed because INIT has not arrived yet.
    #[norito(default)]
    pub stash_ready_init_missing_total: u64,
    /// READY frames stashed because the commit roster is missing.
    #[norito(default)]
    pub stash_ready_roster_missing_total: u64,
    /// READY frames stashed because the commit roster hash mismatched.
    #[norito(default)]
    pub stash_ready_roster_hash_mismatch_total: u64,
    /// READY frames stashed while the commit roster is unverified.
    #[norito(default)]
    pub stash_ready_roster_unverified_total: u64,
    /// Total DELIVER frames stashed before processing.
    #[norito(default)]
    pub stash_deliver_total: u64,
    /// DELIVER frames stashed because INIT has not arrived yet.
    #[norito(default)]
    pub stash_deliver_init_missing_total: u64,
    /// DELIVER frames stashed because the commit roster is missing.
    #[norito(default)]
    pub stash_deliver_roster_missing_total: u64,
    /// DELIVER frames stashed because the commit roster hash mismatched.
    #[norito(default)]
    pub stash_deliver_roster_hash_mismatch_total: u64,
    /// DELIVER frames stashed while the commit roster is unverified.
    #[norito(default)]
    pub stash_deliver_roster_unverified_total: u64,
    /// Chunk frames stashed before INIT arrives.
    #[norito(default)]
    pub stash_chunk_total: u64,
    /// Pending sessions with per-session drop counters.
    #[norito(default)]
    pub entries: Vec<SumeragiPendingRbcEntry>,
}

/// Block-sync roster selection counters exposed via Sumeragi status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiBlockSyncRosterStatus {
    /// Total times a commit certificate hint was used.
    #[norito(default)]
    pub commit_qc_hint_total: u64,
    /// Total times a validator-checkpoint hint was used.
    #[norito(default)]
    pub checkpoint_hint_total: u64,
    /// Total times commit-certificate history was used.
    #[norito(default)]
    pub commit_qc_history_total: u64,
    /// Total times validator-checkpoint history was used.
    #[norito(default)]
    pub checkpoint_history_total: u64,
    /// Total times a roster sidecar was used.
    #[norito(default)]
    pub roster_sidecar_total: u64,
    /// Total times a commit-roster journal snapshot was used.
    #[norito(default)]
    pub commit_roster_journal_total: u64,
    /// Block-sync drops due to missing/invalid roster proofs.
    #[norito(default)]
    pub drop_missing_total: u64,
    /// Block-sync `ShareBlocks` drops without a matching request.
    #[norito(default)]
    pub drop_unsolicited_share_blocks_total: u64,
}

/// View-change cause counters surfaced via `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiViewChangeCauseStatus {
    /// Total view changes triggered after commit failures (with QC quorum).
    #[norito(default)]
    pub commit_failure_total: u64,
    /// Total view changes triggered after quorum timeouts/missing commits.
    #[norito(default)]
    pub quorum_timeout_total: u64,
    /// Total view changes triggered after DA availability aborts (unused when DA is advisory).
    #[norito(default)]
    pub da_gate_total: u64,
    /// Total view changes triggered after censorship evidence reaches quorum.
    #[norito(default)]
    pub censorship_evidence_total: u64,
    /// Total view changes triggered after missing payloads exceeded dwell.
    #[norito(default)]
    pub missing_payload_total: u64,
    /// Total view changes triggered after missing or stale QCs.
    #[norito(default)]
    pub missing_qc_total: u64,
    /// Total view changes triggered after validation rejects before voting.
    #[norito(default)]
    pub validation_reject_total: u64,
    /// Last recorded view-change cause label (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_cause: Option<String>,
    /// Milliseconds since UNIX epoch when the last cause was recorded.
    #[norito(default)]
    pub last_cause_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a commit-failure cause was last recorded.
    #[norito(default)]
    pub last_commit_failure_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a quorum-timeout cause was last recorded.
    #[norito(default)]
    pub last_quorum_timeout_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a DA-gate cause was last recorded.
    #[norito(default)]
    pub last_da_gate_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a censorship-evidence cause was last recorded.
    #[norito(default)]
    pub last_censorship_evidence_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a missing-payload cause was last recorded.
    #[norito(default)]
    pub last_missing_payload_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a missing-QC cause was last recorded.
    #[norito(default)]
    pub last_missing_qc_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a validation-reject cause was last recorded.
    #[norito(default)]
    pub last_validation_reject_timestamp_ms: u64,
}

/// Validation-gate reject counters and last-occurrence snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiValidationRejectStatus {
    /// Total rejects recorded before voting.
    #[norito(default)]
    pub total: u64,
    /// Stateless validation rejects (header, timestamps, genesis checks).
    #[norito(default)]
    pub stateless_total: u64,
    /// Execution/stateful validation rejects (transaction execution, DA availability checks).
    #[norito(default)]
    pub execution_total: u64,
    /// Prev-block hash mismatch rejects.
    #[norito(default)]
    pub prev_hash_total: u64,
    /// Prev-block height mismatch rejects.
    #[norito(default)]
    pub prev_height_total: u64,
    /// Topology/roster mismatch rejects.
    #[norito(default)]
    pub topology_total: u64,
    /// Last recorded reason label (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_reason: Option<String>,
    /// Last rejected block height (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_height: Option<u64>,
    /// Last rejected block view (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_view: Option<u64>,
    /// Last rejected block hash (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_block: Option<HashOf<BlockHeader>>,
    /// Milliseconds since UNIX epoch when the last reject was recorded.
    #[norito(default)]
    pub last_timestamp_ms: u64,
}

/// Peer consensus-key policy reject counters and last-occurrence snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiPeerKeyPolicyStatus {
    /// Total peer-key policy rejects recorded.
    #[norito(default)]
    pub total: u64,
    /// Rejects due to missing HSM binding when required.
    #[norito(default)]
    pub missing_hsm_total: u64,
    /// Rejects due to disallowed public-key algorithm.
    #[norito(default)]
    pub disallowed_algorithm_total: u64,
    /// Rejects due to disallowed HSM provider.
    #[norito(default)]
    pub disallowed_provider_total: u64,
    /// Rejects due to activation height violating lead-time policy.
    #[norito(default)]
    pub lead_time_violation_total: u64,
    /// Rejects due to activation height being in the past.
    #[norito(default)]
    pub activation_in_past_total: u64,
    /// Rejects due to expiry occurring before activation.
    #[norito(default)]
    pub expiry_before_activation_total: u64,
    /// Rejects due to identifier collisions for the same public key.
    #[norito(default)]
    pub identifier_collision_total: u64,
    /// Last recorded reject reason (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_reason: Option<String>,
    /// Milliseconds since UNIX epoch when the last reject was recorded.
    #[norito(default)]
    pub last_timestamp_ms: u64,
}

/// Deterministic consensus configuration caps captured alongside status snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[allow(clippy::struct_excessive_bools)] // Independent capability toggles are kept explicit for the status surface.
pub struct SumeragiConsensusCapsStatus {
    /// Number of collectors (K).
    pub collectors_k: u16,
    /// Redundant send fanout (r).
    pub redundant_send_r: u8,
    /// Data availability enabled (RBC + availability QC gating).
    pub da_enabled: bool,
    /// Maximum RBC chunk size in bytes.
    pub rbc_chunk_max_bytes: u64,
    /// RBC session TTL in milliseconds.
    pub rbc_session_ttl_ms: u64,
    /// Hard cap on persisted RBC sessions.
    pub rbc_store_max_sessions: u32,
    /// Soft cap on persisted RBC sessions.
    pub rbc_store_soft_sessions: u32,
    /// Hard cap on persisted RBC payload bytes.
    pub rbc_store_max_bytes: u64,
    /// Soft cap on persisted RBC payload bytes.
    pub rbc_store_soft_bytes: u64,
}

/// Queue depth snapshot for Sumeragi worker-loop channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiWorkerQueueDepths {
    /// Vote channel depth.
    #[norito(default)]
    pub vote_rx: u64,
    /// Block payload channel depth.
    #[norito(default)]
    pub block_payload_rx: u64,
    /// RBC chunk channel depth.
    #[norito(default)]
    pub rbc_chunk_rx: u64,
    /// Block channel depth.
    #[norito(default)]
    pub block_rx: u64,
    /// Consensus control channel depth.
    #[norito(default)]
    pub consensus_rx: u64,
    /// Lane relay channel depth.
    #[norito(default)]
    pub lane_relay_rx: u64,
    /// Background post channel depth.
    #[norito(default)]
    pub background_rx: u64,
}

/// Per-queue totals for worker-loop diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiWorkerQueueTotals {
    /// Vote channel total.
    #[norito(default)]
    pub vote_rx: u64,
    /// Block payload channel total.
    #[norito(default)]
    pub block_payload_rx: u64,
    /// RBC chunk channel total.
    #[norito(default)]
    pub rbc_chunk_rx: u64,
    /// Block channel total.
    #[norito(default)]
    pub block_rx: u64,
    /// Consensus control channel total.
    #[norito(default)]
    pub consensus_rx: u64,
    /// Lane relay channel total.
    #[norito(default)]
    pub lane_relay_rx: u64,
    /// Background post channel total.
    #[norito(default)]
    pub background_rx: u64,
}

/// Worker-loop queue diagnostics (drops/blocking).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiWorkerQueueDiagnostics {
    /// Total count of blocking enqueues per queue.
    #[norito(default)]
    pub blocked_total: SumeragiWorkerQueueTotals,
    /// Total time spent blocked (ms) per queue.
    #[norito(default)]
    pub blocked_ms_total: SumeragiWorkerQueueTotals,
    /// Maximum block duration (ms) per queue.
    #[norito(default)]
    pub blocked_max_ms: SumeragiWorkerQueueTotals,
    /// Total count of dropped enqueues per queue.
    #[norito(default)]
    pub dropped_total: SumeragiWorkerQueueTotals,
}

/// Worker-loop diagnostics exposed by `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiWorkerLoopStatus {
    /// Last observed worker-loop stage label.
    #[norito(default)]
    pub stage: String,
    /// Timestamp (ms since UNIX epoch) when the stage was last updated.
    #[norito(default)]
    pub stage_started_ms: u64,
    /// Duration of the most recent worker iteration in milliseconds.
    #[norito(default)]
    pub last_iteration_ms: u64,
    /// Queue depth snapshot for worker-loop channels.
    #[norito(default)]
    pub queue_depths: SumeragiWorkerQueueDepths,
    /// Queue enqueue diagnostics (drops/blocking).
    #[norito(default)]
    pub queue_diagnostics: SumeragiWorkerQueueDiagnostics,
}

/// Commit inflight diagnostics exposed by `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiCommitInflightStatus {
    /// Whether a commit job is currently in flight.
    #[norito(default)]
    pub active: bool,
    /// Inflight commit id (best-effort).
    #[norito(default)]
    pub id: u64,
    /// Block height associated with the inflight commit.
    #[norito(default)]
    pub height: u64,
    /// View associated with the inflight commit.
    #[norito(default)]
    pub view: u64,
    /// Block hash associated with the inflight commit.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Timestamp (ms since UNIX epoch) when the inflight commit was enqueued.
    #[norito(default)]
    pub started_ms: u64,
    /// Milliseconds elapsed since the inflight commit started (best-effort).
    #[norito(default)]
    pub elapsed_ms: u64,
    /// Configured inflight timeout in milliseconds.
    #[norito(default)]
    pub timeout_ms: u64,
    /// Total inflight timeouts observed.
    #[norito(default)]
    pub timeout_total: u64,
    /// Timestamp (ms since UNIX epoch) of the last inflight timeout.
    #[norito(default)]
    pub last_timeout_timestamp_ms: u64,
    /// Duration (ms) of the last inflight timeout.
    #[norito(default)]
    pub last_timeout_elapsed_ms: u64,
    /// Height associated with the last inflight timeout.
    #[norito(default)]
    pub last_timeout_height: u64,
    /// View associated with the last inflight timeout.
    #[norito(default)]
    pub last_timeout_view: u64,
    /// Block hash associated with the last inflight timeout.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_timeout_block_hash: Option<HashOf<BlockHeader>>,
    /// Total number of pacemaker pauses caused by inflight commits.
    #[norito(default)]
    pub pause_total: u64,
    /// Total number of pacemaker resumes following inflight completion.
    #[norito(default)]
    pub resume_total: u64,
    /// Timestamp (ms since UNIX epoch) when the current pause began.
    #[norito(default)]
    pub paused_since_ms: u64,
    /// Queue depth snapshot recorded when the inflight pause started.
    #[norito(default)]
    pub pause_queue_depths: SumeragiWorkerQueueDepths,
    /// Queue depth snapshot recorded when the inflight pause ended.
    #[norito(default)]
    pub resume_queue_depths: SumeragiWorkerQueueDepths,
}

/// Latest commit-quorum signature tally exposed by `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiCommitQuorumStatus {
    /// Block height associated with the tally.
    #[norito(default)]
    pub height: u64,
    /// View associated with the tally.
    #[norito(default)]
    pub view: u64,
    /// Block hash associated with the tally.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Total signatures present on the block.
    #[norito(default)]
    pub signatures_present: u64,
    /// Signatures counted toward the commit quorum.
    #[norito(default)]
    pub signatures_counted: u64,
    /// Signatures contributed by set-B validators.
    #[norito(default)]
    pub signatures_set_b: u64,
    /// Required commit quorum size.
    #[norito(default)]
    pub signatures_required: u64,
    /// Timestamp (ms since UNIX epoch) when the tally was recorded.
    #[norito(default)]
    pub last_updated_ms: u64,
}

/// Latest commit QC summary exposed by `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiQcStatus {
    /// Block height certified by the commit QC.
    #[norito(default)]
    pub height: u64,
    /// View associated with the commit QC.
    #[norito(default)]
    pub view: u64,
    /// Epoch associated with the commit QC.
    #[norito(default)]
    pub epoch: u64,
    /// Block hash certified by the commit QC.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Stable hash of the validator set that produced the QC.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub validator_set_hash: Option<HashOf<Vec<PeerId>>>,
    /// Number of validators in the recorded set.
    #[norito(default)]
    pub validator_set_len: u64,
    /// Total signatures attached to the QC.
    #[norito(default)]
    pub signatures_total: u64,
}

/// Compact Norito payload returned by Torii for `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiStatusWire {
    /// Current runtime mode tag.
    #[norito(default)]
    pub mode_tag: String,
    /// Staged mode tag if activation is pending.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub staged_mode_tag: Option<String>,
    /// Activation height for staged mode (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub staged_mode_activation_height: Option<u64>,
    /// Blocks elapsed since activation height passed without applying the staged mode (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub mode_activation_lag_blocks: Option<u64>,
    /// Whether runtime mode flips are currently allowed by configuration.
    #[norito(default)]
    pub mode_flip_kill_switch: bool,
    /// Whether the last flip attempt was blocked (e.g., by kill switch).
    #[norito(default)]
    pub mode_flip_blocked: bool,
    /// Total successful runtime mode flips.
    #[norito(default)]
    pub mode_flip_success_total: u64,
    /// Total failed mode flip attempts.
    #[norito(default)]
    pub mode_flip_fail_total: u64,
    /// Total mode flip attempts blocked by configuration.
    #[norito(default)]
    pub mode_flip_blocked_total: u64,
    /// Timestamp (ms since UNIX epoch) of the last attempted flip, if any.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_mode_flip_timestamp_ms: Option<u64>,
    /// Last recorded flip error (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_mode_flip_error: Option<String>,
    /// Consensus handshake caps derived from runtime configuration (if computed).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub consensus_caps: Option<SumeragiConsensusCapsStatus>,
    /// Current leader index (topology position).
    pub leader_index: u64,
    /// `HighestQC` height.
    pub highest_qc_height: u64,
    /// `HighestQC` view.
    pub highest_qc_view: u64,
    /// `HighestQC` subject block hash when available.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_qc_subject: Option<HashOf<BlockHeader>>,
    /// `LockedQC` height.
    pub locked_qc_height: u64,
    /// `LockedQC` view.
    pub locked_qc_view: u64,
    /// `LockedQC` subject block hash when available.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub locked_qc_subject: Option<HashOf<BlockHeader>>,
    /// Latest commit QC summary (best-effort).
    #[norito(default)]
    pub commit_qc: SumeragiQcStatus,
    /// Latest commit quorum signature tally (best-effort).
    #[norito(default)]
    pub commit_quorum: SumeragiCommitQuorumStatus,
    /// Total view-change proofs accepted (advanced the proof chain).
    #[norito(default)]
    pub view_change_proof_accepted_total: u64,
    /// Total view-change proofs ignored as stale/outdated.
    #[norito(default)]
    pub view_change_proof_stale_total: u64,
    /// Total view-change proofs rejected due to validation failures.
    #[norito(default)]
    pub view_change_proof_rejected_total: u64,
    /// Total local view-change suggestions emitted.
    #[norito(default)]
    pub view_change_suggest_total: u64,
    /// Total view changes installed locally (proof advanced).
    #[norito(default)]
    pub view_change_install_total: u64,
    /// View-change cause counters and last occurrence (best-effort).
    #[norito(default)]
    pub view_change_causes: SumeragiViewChangeCauseStatus,
    /// Total gossip fallback invocations.
    pub gossip_fallback_total: u64,
    /// Total proposals dropped due to locked QC gate.
    pub block_created_dropped_by_lock_total: u64,
    /// Total proposals rejected due to hint mismatches.
    pub block_created_hint_mismatch_total: u64,
    /// Total proposals rejected due to payload/header mismatches.
    pub block_created_proposal_mismatch_total: u64,
    /// Total blocks rejected by the validation gate before voting.
    #[norito(default)]
    pub validation_reject_total: u64,
    /// Last validation-reject reason label (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub validation_reject_reason: Option<String>,
    /// Validation gate reject breakdown and last-occurrence snapshot.
    #[norito(default)]
    pub validation_rejects: SumeragiValidationRejectStatus,
    /// Peer consensus-key policy rejects and last-occurrence snapshot.
    #[norito(default)]
    pub peer_key_policy: SumeragiPeerKeyPolicyStatus,
    /// Block-sync roster selection counters.
    #[norito(default)]
    pub block_sync_roster: SumeragiBlockSyncRosterStatus,
    /// Total pacemaker proposal attempts deferred due to transaction queue backpressure.
    pub pacemaker_backpressure_deferrals_total: u64,
    /// Total commit-pipeline executions triggered by the pacemaker tick loop.
    #[norito(default)]
    pub commit_pipeline_tick_total: u64,
    /// Total DA deadline reschedules that moved transactions into later slots.
    #[norito(default)]
    pub da_reschedule_total: u64,
    /// Missing-block fetch counters after QC-first arrivals.
    #[norito(default)]
    pub missing_block_fetch: SumeragiMissingBlockFetchStatus,
    /// DA availability telemetry and last-satisfied snapshot.
    #[norito(default)]
    pub da_gate: SumeragiDaGateStatus,
    /// Kura persistence snapshot.
    #[norito(default)]
    pub kura_store: SumeragiKuraStoreStatus,
    /// RBC store snapshot.
    #[norito(default)]
    pub rbc_store: SumeragiRbcStoreStatus,
    /// Pending RBC stash snapshot.
    #[norito(default)]
    pub pending_rbc: SumeragiPendingRbcStatus,
    /// Current transaction queue depth.
    pub tx_queue_depth: u64,
    /// Configured transaction queue capacity.
    pub tx_queue_capacity: u64,
    /// Whether the transaction queue is saturated.
    pub tx_queue_saturated: bool,
    /// Epoch length in blocks (`NPoS` mode; zero when not applicable).
    #[norito(default)]
    pub epoch_length_blocks: u64,
    /// Commit window deadline offset from epoch start (blocks; zero when not applicable).
    #[norito(default)]
    pub epoch_commit_deadline_offset: u64,
    /// Reveal window deadline offset from epoch start (blocks; zero when not applicable).
    #[norito(default)]
    pub epoch_reveal_deadline_offset: u64,
    /// PRF epoch seed used for deterministic leader/collector selection.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub prf_epoch_seed: Option<[u8; 32]>,
    /// Height associated with the recorded PRF context.
    pub prf_height: u64,
    /// View associated with the recorded PRF context.
    pub prf_view: u64,
    /// Latest epoch index for which VRF penalties were recorded.
    pub vrf_penalty_epoch: u64,
    /// Number of validators that committed without revealing in the latest epoch snapshot.
    pub vrf_committed_no_reveal_total: u64,
    /// Number of validators that neither committed nor revealed in the latest epoch snapshot.
    pub vrf_no_participation_total: u64,
    /// Number of validators that revealed after the reveal window in the latest epoch snapshot.
    pub vrf_late_reveals_total: u64,
    /// Total consensus penalties applied (evidence-driven).
    #[norito(default)]
    pub consensus_penalties_applied_total: u64,
    /// Consensus evidence records pending activation before penalties apply.
    #[norito(default)]
    pub consensus_penalties_pending: u64,
    /// Total VRF penalties applied.
    #[norito(default)]
    pub vrf_penalties_applied_total: u64,
    /// VRF penalty snapshots pending activation.
    #[norito(default)]
    pub vrf_penalties_pending: u64,
    /// Deterministic membership snapshot.
    #[norito(default)]
    pub membership: SumeragiMembershipStatus,
    /// Membership mismatch snapshot.
    #[norito(default)]
    pub membership_mismatch: SumeragiMembershipMismatchStatus,
    /// Aggregated lane-level commitment snapshots.
    #[norito(default)]
    pub lane_commitments: Vec<SumeragiLaneCommitment>,
    /// Aggregated dataspace-level commitment snapshots.
    #[norito(default)]
    pub dataspace_commitments: Vec<SumeragiDataspaceCommitment>,
    /// Aggregated lane-level settlement commitments.
    #[norito(default)]
    pub lane_settlement_commitments: Vec<LaneBlockCommitment>,
    /// Relay envelopes capturing lane block headers, QCs, DA digests, and settlement proofs.
    #[norito(default)]
    pub lane_relay_envelopes: Vec<LaneRelayEnvelope>,
    /// Count of lanes that still require a governance manifest.
    #[norito(default)]
    pub lane_governance_sealed_total: u32,
    /// Aliases of lanes that remain sealed (manifest missing).
    #[norito(default)]
    pub lane_governance_sealed_aliases: Vec<String>,
    /// Governance manifest readiness per lane.
    #[norito(default)]
    pub lane_governance: Vec<SumeragiLaneGovernance>,
    /// Worker-loop stage and queue depth snapshot.
    #[norito(default)]
    pub worker_loop: SumeragiWorkerLoopStatus,
    /// Commit inflight diagnostics snapshot.
    #[norito(default)]
    pub commit_inflight: SumeragiCommitInflightStatus,
}

/// Entry describing a QC snapshot used by `/v1/sumeragi/qc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiQcEntry {
    /// Certified block height.
    pub height: Height,
    /// View in which the QC was formed.
    pub view: View,
    /// Subject block hash if known.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub subject_block_hash: Option<HashOf<BlockHeader>>,
}

/// Norito payload returned by Torii for `/v1/sumeragi/qc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SumeragiQcSnapshot {
    /// `HighestQC` snapshot.
    pub highest_qc: SumeragiQcEntry,
    /// `LockedQC` snapshot.
    pub locked_qc: SumeragiQcEntry,
}

/// Minimal execution witness KV pair for SBV-AM prototypes.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct ExecKv {
    /// Raw key bytes.
    pub key: Vec<u8>,
    /// Raw value bytes.
    pub value: Vec<u8>,
}

/// Execution witness containing reads and writes for SMT recomputation.
#[derive(Clone, Debug, PartialEq, Eq, Default, Decode, Encode, IntoSchema)]
pub struct ExecWitness {
    /// Witnessed reads during execution (key,value).
    pub reads: Vec<ExecKv>,
    /// Writes performed during execution (key,value). Overrides reads on conflict.
    pub writes: Vec<ExecKv>,
    /// FASTPQ transfer transcripts grouped per entry hash.
    pub fastpq_transcripts: Vec<TransferTranscriptBundle>,
    /// FASTPQ transition batches prepared for prover ingestion.
    pub fastpq_batches: Vec<FastpqTransitionBatch>,
}

/// Execution witness message bound to a specific block and round. Used on-wire.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct ExecWitnessMsg {
    /// Hash of the block the witness applies to.
    pub block_hash: HashOf<BlockHeader>,
    /// Height of the block.
    pub height: Height,
    /// View/round for which the witness applies.
    pub view: View,
    /// Epoch index (0 in permissioned mode).
    pub epoch: u64,
    /// The execution witness payload.
    pub witness: ExecWitness,
}

/// VRF commit (`NPoS` only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct VrfCommit {
    /// Epoch index to which the commit applies.
    pub epoch: u64,
    /// Hiding commitment to the reveal.
    pub commitment: [u8; 32],
    /// Signer index within the validator set.
    pub signer: ValidatorIndex,
}

/// VRF reveal (`NPoS` only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct VrfReveal {
    /// Epoch index to which the reveal applies.
    pub epoch: u64,
    /// Revealed preimage value.
    pub reveal: [u8; 32],
    /// Signer index within the validator set.
    pub signer: ValidatorIndex,
}

/// Reconfiguration payload (permissioned governance path).
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct Reconfig {
    /// New validator roster (ordered deterministically).
    pub new_roster: Vec<PeerId>,
    /// First height at which the new set becomes active.
    pub activation_height: Height,
}

/// RBC init message for payload distribution scaffolding.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcInit {
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Height.
    pub height: Height,
    /// View.
    pub view: View,
    /// Epoch.
    pub epoch: u64,
    /// Commit roster snapshot for this RBC session.
    pub roster: Vec<PeerId>,
    /// Hash of the Norito-encoded roster snapshot.
    pub roster_hash: Hash,
    /// Total chunk count for the payload.
    pub total_chunks: u32,
    /// SHA-256 digests for each chunk (indexed by chunk position).
    pub chunk_digests: Vec<[u8; 32]>,
    /// Payload hash commitment (optional, when leader is also proposer).
    pub payload_hash: Hash,
    /// Merkle root of chunk digests for integrity proofs.
    pub chunk_root: Hash,
}

/// RBC payload chunk.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcChunk {
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Height.
    pub height: Height,
    /// View.
    pub view: View,
    /// Epoch.
    pub epoch: u64,
    /// Chunk index (0-based).
    pub idx: u32,
    /// Chunk bytes.
    pub bytes: Vec<u8>,
}

/// RBC READY signal.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcReady {
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Height.
    pub height: Height,
    /// View.
    pub view: View,
    /// Epoch.
    pub epoch: u64,
    /// Hash of the roster snapshot used to validate READY signatures.
    pub roster_hash: Hash,
    /// Merkle root of chunk digests for integrity proofs.
    pub chunk_root: Hash,
    /// Sender index within the active set.
    pub sender: ValidatorIndex,
    /// Signature authenticating the sender for this READY.
    pub signature: Vec<u8>,
}

/// READY signature included with RBC DELIVER to seed quorum recovery.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcReadySignature {
    /// Sender index within the active set.
    pub sender: ValidatorIndex,
    /// Signature authenticating the sender for this READY.
    pub signature: Vec<u8>,
}

/// RBC DELIVER notification.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct RbcDeliver {
    /// Subject block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Height.
    pub height: Height,
    /// View.
    pub view: View,
    /// Epoch.
    pub epoch: u64,
    /// Hash of the roster snapshot used to validate DELIVER signatures.
    pub roster_hash: Hash,
    /// Merkle root of chunk digests for integrity proofs.
    pub chunk_root: Hash,
    /// Sender index within the active set.
    pub sender: ValidatorIndex,
    /// Signature authenticating the sender for this DELIVER.
    pub signature: Vec<u8>,
    /// READY signatures observed by the sender for this session.
    pub ready_signatures: Vec<RbcReadySignature>,
}

#[cfg(feature = "sumeragi-multiproof")]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct BlockMultiproof {
    pub reads: Vec<ReadNode>,
    pub read_keys: Vec<Vec<u8>>, // canonical bytes for keys
    pub writes: Vec<WriteEntry>,
    pub per_tx_read_index: Option<Vec<TxReadSpan>>,
    pub proof_aux: Vec<u8>,
}

#[cfg(feature = "sumeragi-multiproof")]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct ReadNode {
    /// Opaque encoding of a Verkle/SMT node needed for verification
    pub node_bytes: Vec<u8>,
}

#[cfg(feature = "sumeragi-multiproof")]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct WriteEntry {
    pub key: Vec<u8>,
    pub value_bytes: Vec<u8>,
    pub pre_version: u64,
    pub new_version: u64,
}

#[cfg(feature = "sumeragi-multiproof")]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct TxReadSpan {
    /// start index (inclusive) into `BlockMultiproof::read_keys`
    pub start: u32,
    /// end index (exclusive)
    pub end: u32,
}

// --- Helpers for Norito slice decoding bridges ---
fn decode_from_slice_canonical<T>(bytes: &[u8]) -> Result<(T, usize), norito::core::Error>
where
    T: DecodeAll + Encode,
{
    let mut slice: &[u8] = bytes;
    let value = T::decode_all(&mut slice)
        .map_err(|e| norito::core::Error::Message(format!("codec decode error: {e}")))?;
    let canonical = value.encode();
    if bytes.len() < canonical.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    if bytes[..canonical.len()] != canonical {
        return Err(norito::core::Error::Message("payload mismatch".into()));
    }
    Ok((value, canonical.len()))
}

macro_rules! impl_decode_from_slice_via_codec {
    ($t:ty) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $t {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                decode_from_slice_canonical(bytes)
            }
        }
    };
}

impl_decode_from_slice_via_codec!(QcRef);
impl_decode_from_slice_via_codec!(ConsensusBlockHeader);
impl_decode_from_slice_via_codec!(Proposal);
impl_decode_from_slice_via_codec!(QcVote);
impl_decode_from_slice_via_codec!(QcAggregate);
impl_decode_from_slice_via_codec!(Qc);
impl_decode_from_slice_via_codec!(ExecKv);
impl_decode_from_slice_via_codec!(ExecWitness);
impl_decode_from_slice_via_codec!(Evidence);
impl_decode_from_slice_via_codec!(EvidencePayload);
impl_decode_from_slice_via_codec!(EvidenceKind);
impl_decode_from_slice_via_codec!(ExecWitnessMsg);
impl_decode_from_slice_via_codec!(VrfCommit);
impl_decode_from_slice_via_codec!(VrfReveal);
impl_decode_from_slice_via_codec!(Reconfig);
impl_decode_from_slice_via_codec!(RbcInit);
impl_decode_from_slice_via_codec!(RbcChunk);
impl_decode_from_slice_via_codec!(RbcReady);
impl_decode_from_slice_via_codec!(RbcReadySignature);
impl_decode_from_slice_via_codec!(RbcDeliver);
#[cfg(feature = "sumeragi-multiproof")]
impl_decode_from_slice_via_codec!(BlockMultiproof);
#[cfg(feature = "sumeragi-multiproof")]
impl_decode_from_slice_via_codec!(ReadNode);
#[cfg(feature = "sumeragi-multiproof")]
impl_decode_from_slice_via_codec!(WriteEntry);
#[cfg(feature = "sumeragi-multiproof")]
impl_decode_from_slice_via_codec!(TxReadSpan);
impl_decode_from_slice_via_codec!(ConsensusGenesisParams);
impl_decode_from_slice_via_codec!(NposGenesisParams);
impl_decode_from_slice_via_codec!(SumeragiMembershipStatus);
impl_decode_from_slice_via_codec!(SumeragiLaneCommitment);
impl_decode_from_slice_via_codec!(SumeragiDataspaceCommitment);
impl_decode_from_slice_via_codec!(SumeragiRuntimeUpgradeHook);
impl_decode_from_slice_via_codec!(SumeragiLaneGovernance);
impl_decode_from_slice_via_codec!(SumeragiStatusWire);

// Provide nicer `Debug` rendering for validator indices in test snapshots.
impl fmt::Display for CertPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CertPhase::Prepare => "Prepare",
            CertPhase::Commit => "Commit",
            CertPhase::NewView => "NewView",
        };
        f.write_str(s)
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for LaneSettlementReceipt {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut input = bytes;
        let value = <Self as DecodeAll>::decode_all(&mut input)?;
        let consumed = bytes.len() - input.len();
        Ok((value, consumed))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, MerkleTree};
    use norito::core::DecodeFromSlice;

    use super::*;

    fn dummy_hash() -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([0u8; 32]))
    }

    fn sample_roster() -> Vec<PeerId> {
        (0..3)
            .map(|_| {
                PeerId::new(
                    KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                        .public_key()
                        .clone(),
                )
            })
            .collect()
    }

    fn roster_hash(roster: &[PeerId]) -> Hash {
        Hash::new(roster.to_vec().encode())
    }

    fn sample_qc_ref() -> QcRef {
        QcRef {
            height: 4,
            view: 1,
            epoch: 1,
            subject_block_hash: dummy_hash(),
            phase: CertPhase::Prepare,
        }
    }

    fn sample_consensus_header() -> ConsensusBlockHeader {
        ConsensusBlockHeader {
            parent_hash: dummy_hash(),
            tx_root: Hash::new(b"tx_root"),
            state_root: Hash::new(b"state_root"),
            proposer: 1,
            height: 6,
            view: 3,
            epoch: 1,
            highest_qc: sample_qc_ref(),
        }
    }

    fn sample_proposal() -> Proposal {
        Proposal {
            header: sample_consensus_header(),
            payload_hash: Hash::new(b"payload"),
        }
    }

    fn sample_reconfig() -> Reconfig {
        let peers = (0..2)
            .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
            .collect();
        Reconfig {
            new_roster: peers,
            activation_height: 42,
        }
    }

    fn sample_rbc_init() -> RbcInit {
        let roster = sample_roster();
        let roster_hash = roster_hash(&roster);
        let chunk_digests = vec![[0x11; 32], [0x22; 32], [0x33; 32]];
        let chunk_root = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(chunk_digests.clone())
            .root()
            .map(Hash::from)
            .expect("chunk root");
        RbcInit {
            block_hash: dummy_hash(),
            height: 6,
            view: 3,
            epoch: 1,
            roster,
            roster_hash,
            total_chunks: 3,
            chunk_digests,
            payload_hash: Hash::new(b"payload_hash"),
            chunk_root,
        }
    }

    fn sample_rbc_chunk() -> RbcChunk {
        RbcChunk {
            block_hash: dummy_hash(),
            height: 6,
            view: 3,
            epoch: 1,
            idx: 1,
            bytes: vec![1, 2, 3, 4],
        }
    }

    fn sample_rbc_ready() -> RbcReady {
        let roster = sample_roster();
        RbcReady {
            block_hash: dummy_hash(),
            height: 6,
            view: 3,
            epoch: 1,
            roster_hash: roster_hash(&roster),
            chunk_root: Hash::prehashed([0xAA; Hash::LENGTH]),
            sender: 2,
            signature: vec![0x10, 0x11],
        }
    }

    fn sample_rbc_deliver() -> RbcDeliver {
        let roster = sample_roster();
        RbcDeliver {
            block_hash: dummy_hash(),
            height: 6,
            view: 3,
            epoch: 1,
            roster_hash: roster_hash(&roster),
            chunk_root: Hash::prehashed([0xAA; Hash::LENGTH]),
            sender: 2,
            signature: vec![0x21, 0x22],
            ready_signatures: vec![RbcReadySignature {
                sender: 1,
                signature: vec![0x31, 0x32],
            }],
        }
    }

    fn sample_vrf_commit() -> VrfCommit {
        VrfCommit {
            epoch: 7,
            commitment: [0xAB; 32],
            signer: 5,
        }
    }

    fn sample_vrf_reveal() -> VrfReveal {
        VrfReveal {
            epoch: 7,
            reveal: [0xCD; 32],
            signer: 5,
        }
    }

    #[test]
    fn qc_roundtrip_encode_decode() {
        let roster = sample_roster();
        let highest = sample_qc_ref();
        let cert = Qc {
            phase: CertPhase::NewView,
            subject_block_hash: highest.subject_block_hash,
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height: highest.height,
            view: 7,
            epoch: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: Some(highest),
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: 1,
            validator_set: roster,
            aggregate: QcAggregate {
                signers_bitmap: vec![0xAA, 0x01],
                bls_aggregate_signature: vec![1, 2, 3],
            },
        };
        let bytes = cert.encode();
        let dec = Qc::decode(&mut &bytes[..]).expect("decode certificate");
        assert_eq!(cert, dec);
    }

    #[test]
    fn exec_witness_roundtrip_codec() {
        let w = ExecWitness {
            reads: vec![ExecKv {
                key: b"key:read".to_vec(),
                value: b"value-pre".to_vec(),
            }],
            writes: vec![ExecKv {
                key: b"key:write".to_vec(),
                value: b"value-post".to_vec(),
            }],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let bytes = w.encode();
        let dec = ExecWitness::decode(&mut &bytes[..]).expect("decode witness");
        assert_eq!(w, dec);
    }

    #[test]
    fn evidence_roundtrip_codec() {
        let roster = sample_roster();
        let ev = Evidence {
            kind: EvidenceKind::InvalidQc,
            payload: EvidencePayload::InvalidQc {
                certificate: Qc {
                    phase: CertPhase::Commit,
                    subject_block_hash: dummy_hash(),
                    parent_state_root: Hash::new(b"parent_root"),
                    post_state_root: Hash::new(b"post_root"),
                    height: 12,
                    view: 3,
                    epoch: 0,
                    mode_tag: PERMISSIONED_TAG.to_string(),
                    highest_qc: None,
                    validator_set_hash: HashOf::new(&roster),
                    validator_set_hash_version: 1,
                    validator_set: roster,
                    aggregate: QcAggregate {
                        signers_bitmap: vec![0xFF],
                        bls_aggregate_signature: vec![4, 5, 6],
                    },
                },
                reason: "test".to_string(),
            },
        };
        let bytes = ev.encode();
        let dec = Evidence::decode(&mut &bytes[..]).expect("decode evidence");
        assert_eq!(ev, dec);
    }

    #[test]
    fn censorship_evidence_roundtrip_codec() {
        let key_pair = KeyPair::random();
        let payload = crate::transaction::TransactionSubmissionReceiptPayload {
            tx_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
            submitted_at_ms: 10,
            submitted_at_height: 2,
            signer: key_pair.public_key().clone(),
        };
        let receipt = crate::transaction::TransactionSubmissionReceipt::sign(payload, &key_pair);
        let tx_hash = receipt.payload.tx_hash;
        let ev = Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![receipt],
            },
        };
        let bytes = ev.encode();
        let dec = Evidence::decode(&mut &bytes[..]).expect("decode censorship evidence");
        assert_eq!(ev, dec);
    }

    #[test]
    fn evidence_record_roundtrip() {
        let ev = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: QcVote {
                    phase: CertPhase::Prepare,
                    block_hash: dummy_hash(),
                    parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                    post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                    height: 10,
                    view: 1,
                    epoch: 0,
                    highest_qc: None,
                    signer: 2,
                    bls_sig: vec![],
                },
                v2: QcVote {
                    phase: CertPhase::Prepare,
                    block_hash: dummy_hash(),
                    parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                    post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
                    height: 10,
                    view: 1,
                    epoch: 0,
                    highest_qc: None,
                    signer: 2,
                    bls_sig: vec![],
                },
            },
        };
        let rec = EvidenceRecord {
            evidence: ev,
            recorded_at_height: 11,
            recorded_at_view: 2,
            recorded_at_ms: 1_689_000,
            penalty_applied: false,
            penalty_applied_at_height: None,
        };
        let bytes = rec.encode();
        let dec = EvidenceRecord::decode(&mut &bytes[..]).expect("decode evidence record");
        assert_eq!(rec, dec);
    }

    #[test]
    fn rbc_ready_decode_from_slice_matches_encode() {
        let ready = RbcReady {
            block_hash: dummy_hash(),
            height: 5,
            view: 1,
            epoch: 0,
            roster_hash: Hash::prehashed([0xAA; Hash::LENGTH]),
            chunk_root: Hash::prehashed([0u8; Hash::LENGTH]),
            sender: 2,
            signature: vec![9, 9, 9],
        };
        let canonical = ready.encode();
        let (decoded, used) =
            RbcReady::decode_from_slice(&canonical).expect("decode_from_slice ready");
        assert_eq!(ready, decoded);
        assert_eq!(used, canonical.len());
    }

    #[test]
    fn proposal_roundtrip_codec() {
        let prop = sample_proposal();
        let bytes = prop.encode();
        let dec = Proposal::decode(&mut &bytes[..]).expect("decode proposal");
        assert_eq!(prop, dec);
    }

    #[test]
    fn qc_vote_roundtrip_codec_and_decode_from_slice() {
        let vote = QcVote {
            phase: CertPhase::Commit,
            block_hash: dummy_hash(),
            parent_state_root: Hash::new(b"parent_root"),
            post_state_root: Hash::new(b"post_root"),
            height: 7,
            view: 2,
            epoch: 0,
            highest_qc: None,
            signer: 3,
            bls_sig: vec![0x01, 0x02],
        };
        let bytes = vote.encode();
        let dec = QcVote::decode(&mut &bytes[..]).expect("decode qc vote");
        assert_eq!(vote, dec);
        let (slice_dec, used) =
            QcVote::decode_from_slice(&bytes).expect("decode_from_slice qc vote");
        assert_eq!(vote, slice_dec);
        assert_eq!(used, bytes.len());
    }

    #[test]
    fn vrf_commit_roundtrip_codec() {
        let commit = sample_vrf_commit();
        let bytes = commit.encode();
        let dec = VrfCommit::decode(&mut &bytes[..]).expect("decode vrf commit");
        assert_eq!(commit, dec);
    }

    #[test]
    fn vrf_reveal_roundtrip_codec() {
        let reveal = sample_vrf_reveal();
        let bytes = reveal.encode();
        let dec = VrfReveal::decode(&mut &bytes[..]).expect("decode vrf reveal");
        assert_eq!(reveal, dec);
    }

    #[test]
    fn reconfig_roundtrip_codec() {
        let reconfig = sample_reconfig();
        let bytes = reconfig.encode();
        let dec = Reconfig::decode(&mut &bytes[..]).expect("decode reconfig");
        assert_eq!(reconfig, dec);
    }

    #[test]
    fn rbc_init_roundtrip_codec() {
        let init = sample_rbc_init();
        let bytes = init.encode();
        let dec = RbcInit::decode(&mut &bytes[..]).expect("decode rbc init");
        assert_eq!(init, dec);
    }

    #[test]
    fn rbc_chunk_roundtrip_codec() {
        let chunk = sample_rbc_chunk();
        let bytes = chunk.encode();
        let dec = RbcChunk::decode(&mut &bytes[..]).expect("decode rbc chunk");
        assert_eq!(chunk, dec);
    }

    #[test]
    fn rbc_ready_roundtrip_codec() {
        let ready = sample_rbc_ready();
        let bytes = ready.encode();
        let dec = RbcReady::decode(&mut &bytes[..]).expect("decode rbc ready");
        assert_eq!(ready, dec);
    }

    #[test]
    fn rbc_deliver_roundtrip_codec() {
        let deliver = sample_rbc_deliver();
        let bytes = deliver.encode();
        let dec = RbcDeliver::decode(&mut &bytes[..]).expect("decode rbc deliver");
        assert_eq!(deliver, dec);
    }
}
