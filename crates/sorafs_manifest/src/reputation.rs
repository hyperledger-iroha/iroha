#![allow(unexpected_cfgs)]
//! Deterministic SoraFS provider reputation schemas, scoring, and proofs.
use blake3::{Hash, Hasher};
use norito::{
    core::Error as NoritoError,
    derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize},
};
use std::cmp::Ordering;
use thiserror::Error;
pub mod signed;
/// Schema version for [`ReputationWeightsV1`].
pub const REPUTATION_WEIGHTS_VERSION_V1: u8 = 1;
/// Schema version for [`ReputationProviderMetricsV1`].
pub const REPUTATION_PROVIDER_METRICS_VERSION_V1: u8 = 1;
/// Schema version for [`ReputationProviderInputV1`].
pub const REPUTATION_PROVIDER_INPUT_VERSION_V1: u8 = 1;
/// Schema version for [`ReputationTrustEdgeV1`].
pub const REPUTATION_TRUST_EDGE_VERSION_V1: u8 = 1;
/// Schema version for [`ProviderReputationV1`].
pub const PROVIDER_REPUTATION_VERSION_V1: u8 = 1;
/// Schema version for [`ReputationSnapshotV1`].
pub const REPUTATION_SNAPSHOT_VERSION_V1: u8 = 1;
/// Schema version for [`ReputationSnapshotEventV1`].
pub const REPUTATION_SNAPSHOT_EVENT_VERSION_V1: u8 = 1;
/// Basis-point denominator used by reputation scores and weights.
pub const REPUTATION_BASIS_POINTS: u16 = 10_000;
/// Minimum published reputation score.
pub const MIN_REPUTATION_SCORE_BPS: u16 = 500;
/// Maximum published reputation score.
pub const MAX_REPUTATION_SCORE_BPS: u16 = 9_900;
/// Score threshold below which a provider receives a low-score flag.
pub const LOW_REPUTATION_SCORE_FLAG_BPS: u16 = 1_500;
/// Default smoothing weight applied to the current score.
pub const DEFAULT_CURRENT_SCORE_WEIGHT_BPS: u16 = 7_000;
/// Default EigenTrust alpha advertised in snapshots.
pub const DEFAULT_EIGENTRUST_ALPHA_BPS: u16 = 8_500;
/// Maximum Merkle proof length accepted by the verifier.
pub const MAX_REPUTATION_MERKLE_PROOF_LEN: usize = 64;
/// Maximum degradation flags accepted on one provider reputation record.
pub const MAX_REPUTATION_DEGRADATION_FLAGS: usize = 5;
/// Maximum provider records accepted in one reputation snapshot.
///
/// The bound applies before any provider-sized allocation. It also keeps leaf
/// indices exactly representable by the V1 proof schema on every supported
/// host.
pub const MAX_REPUTATION_PROVIDERS: usize = 65_536;
/// Maximum trust edges accepted in one scoring run.
pub const MAX_REPUTATION_TRUST_EDGES: usize = 1_048_576;
/// Maximum number of fixed-point EigenTrust iterations.
pub const REPUTATION_EIGENTRUST_MAX_ITERATIONS: usize = 100;
/// Convergence threshold for the L1 score delta, in basis points.
pub const REPUTATION_EIGENTRUST_CONVERGENCE_L1_BPS: u64 = 1;
/// Governance-controlled reputation weights expressed in basis points.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct ReputationWeightsV1 {
    /// Schema version (`REPUTATION_WEIGHTS_VERSION_V1`).
    pub version: u8,
    /// Weight for proof-of-retrievability success.
    pub por_success_bps: u16,
    /// Weight for proof-of-data-possession success.
    pub pdp_success_bps: u16,
    /// Weight for proof-of-timely-retrieval success.
    pub potr_success_bps: u16,
    /// Weight for latency health.
    pub latency_bps: u16,
    /// Penalty weight for upheld dispute rate.
    pub dispute_bps: u16,
    /// Penalty weight for token violation rate.
    pub token_violation_bps: u16,
    /// Penalty weight for unresolved repair breaches.
    pub repair_breach_bps: u16,
}
impl Default for ReputationWeightsV1 {
    fn default() -> Self {
        Self {
            version: REPUTATION_WEIGHTS_VERSION_V1,
            por_success_bps: 2_200,
            pdp_success_bps: 2_000,
            potr_success_bps: 1_800,
            latency_bps: 1_500,
            dispute_bps: 1_000,
            token_violation_bps: 500,
            repair_breach_bps: 1_000,
        }
    }
}
impl ReputationWeightsV1 {
    /// Validate that the version and aggregate weight budget are canonical.
    pub fn validate(&self) -> Result<(), ReputationValidationError> {
        if self.version != REPUTATION_WEIGHTS_VERSION_V1 {
            return Err(ReputationValidationError::UnsupportedWeightsVersion {
                found: self.version,
            });
        }
        let positive = u32::from(self.por_success_bps)
            + u32::from(self.pdp_success_bps)
            + u32::from(self.potr_success_bps)
            + u32::from(self.latency_bps);
        let negative = u32::from(self.dispute_bps)
            + u32::from(self.token_violation_bps)
            + u32::from(self.repair_breach_bps);
        let total = positive + negative;
        if total != u32::from(REPUTATION_BASIS_POINTS) {
            return Err(ReputationValidationError::InvalidWeightTotal { total_bps: total });
        }
        Ok(())
    }
}
/// Reserve+Rent lifecycle stage used as a reputation multiplier input.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
#[norito(tag = "stage", content = "value", rename_all = "snake_case")]
pub enum ReputationReserveStageV1 {
    /// Provider reserve is active and healthy.
    Active,
    /// Provider reserve is in warning.
    Warning,
    /// Provider is in grace.
    Grace,
    /// Provider is delinquent.
    Delinquent,
    /// Provider is in default.
    Default,
}
impl ReputationReserveStageV1 {
    const fn multiplier_bps(self) -> u16 {
        match self {
            Self::Active => 10_000,
            Self::Warning => 9_000,
            Self::Grace => 7_500,
            Self::Delinquent => 5_000,
            Self::Default => 2_000,
        }
    }
}
/// Degradation flags attached to a provider reputation entry.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
#[norito(tag = "flag", content = "value", rename_all = "snake_case")]
pub enum ReputationDegradationFlagV1 {
    /// Reserve is in warning.
    ReserveWarning,
    /// Reserve is in grace.
    ReserveGrace,
    /// Reserve is delinquent.
    ReserveDelinquent,
    /// Reserve is in default.
    ReserveDefault,
    /// Seven-day PoR/PDP success fell below 90%.
    ProofSuccessBelow90,
    /// Seven-day PoR/PDP success fell below 80%.
    ProofSuccessBelow80,
    /// An active dispute is open.
    ActiveDispute,
    /// A slashing event applies.
    SlashingEvent,
    /// Score is below the low-score threshold.
    LowScore,
}
/// Canonical provider metrics consumed by the reputation scorer.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct ReputationProviderMetricsV1 {
    /// Schema version (`REPUTATION_PROVIDER_METRICS_VERSION_V1`).
    pub version: u8,
    /// PoR success ratio, in basis points.
    pub por_success_bps: u16,
    /// PDP success ratio, in basis points.
    pub pdp_success_bps: u16,
    /// PoTR success ratio, in basis points.
    pub potr_success_bps: u16,
    /// Latency health ratio, in basis points.
    pub latency_health_bps: u16,
    /// Upheld disputes per reputation period, normalised to basis points.
    pub dispute_rate_bps: u16,
    /// Token violations per reputation period, normalised to basis points.
    pub token_violation_rate_bps: u16,
    /// Unresolved repair breaches per reputation period, normalised to basis points.
    pub repair_breach_rate_bps: u16,
}
impl ReputationProviderMetricsV1 {
    /// Validate all metric fields.
    pub fn validate(&self) -> Result<(), ReputationValidationError> {
        if self.version != REPUTATION_PROVIDER_METRICS_VERSION_V1 {
            return Err(ReputationValidationError::UnsupportedMetricsVersion {
                found: self.version,
            });
        }
        validate_bps("por_success_bps", self.por_success_bps)?;
        validate_bps("pdp_success_bps", self.pdp_success_bps)?;
        validate_bps("potr_success_bps", self.potr_success_bps)?;
        validate_bps("latency_health_bps", self.latency_health_bps)?;
        validate_bps("dispute_rate_bps", self.dispute_rate_bps)?;
        validate_bps("token_violation_rate_bps", self.token_violation_rate_bps)?;
        validate_bps("repair_breach_rate_bps", self.repair_breach_rate_bps)?;
        Ok(())
    }
}
/// Per-provider reputation input used for deterministic score generation.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ReputationProviderInputV1 {
    /// Schema version (`REPUTATION_PROVIDER_INPUT_VERSION_V1`).
    pub version: u8,
    /// Governance-controlled provider identifier.
    pub provider_id: String,
    /// Canonical metric window.
    pub metrics: ReputationProviderMetricsV1,
    /// Reserve+Rent lifecycle stage.
    pub reserve_stage: ReputationReserveStageV1,
    /// Previous published score, if available.
    #[norito(default)]
    pub previous_score_bps: Option<u16>,
    /// Whether an active dispute is open.
    pub active_dispute: bool,
    /// Whether a slashing event applies during the window.
    pub slashing_event: bool,
}
impl ReputationProviderInputV1 {
    /// Validate the provider input before scoring.
    pub fn validate(&self) -> Result<(), ReputationValidationError> {
        if self.version != REPUTATION_PROVIDER_INPUT_VERSION_V1 {
            return Err(ReputationValidationError::UnsupportedInputVersion {
                found: self.version,
            });
        }
        validate_provider_id(&self.provider_id)?;
        self.metrics.validate()?;
        if let Some(score) = self.previous_score_bps {
            validate_bps("previous_score_bps", score)?;
        }
        Ok(())
    }
}
/// Pairwise settlement-satisfaction trust edge used by the EigenTrust step.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ReputationTrustEdgeV1 {
    /// Schema version (`REPUTATION_TRUST_EDGE_VERSION_V1`).
    pub version: u8,
    /// Provider that emitted the trust signal.
    pub from_provider_id: String,
    /// Provider receiving the trust signal.
    pub to_provider_id: String,
    /// Trust score for this edge, in basis points.
    pub trust_bps: u16,
}
impl ReputationTrustEdgeV1 {
    /// Validate the trust edge before it enters the EigenTrust iteration.
    pub fn validate(&self) -> Result<(), ReputationValidationError> {
        if self.version != REPUTATION_TRUST_EDGE_VERSION_V1 {
            return Err(ReputationValidationError::UnsupportedTrustEdgeVersion {
                found: self.version,
            });
        }
        validate_provider_id(&self.from_provider_id)?;
        validate_provider_id(&self.to_provider_id)?;
        validate_bps("trust_bps", self.trust_bps)?;
        if self.from_provider_id == self.to_provider_id {
            return Err(ReputationValidationError::SelfTrustEdge {
                provider_id: self.from_provider_id.clone(),
            });
        }
        if self.trust_bps == 0 {
            return Err(ReputationValidationError::ZeroTrustEdge);
        }
        Ok(())
    }
}
/// Published provider reputation record.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ProviderReputationV1 {
    /// Schema version (`PROVIDER_REPUTATION_VERSION_V1`).
    pub version: u8,
    /// Governance-controlled provider identifier.
    pub provider_id: String,
    /// Final bounded score in basis points.
    pub score_bps: u16,
    /// Degradation flags in canonical sorted order.
    pub degradation_flags: Vec<ReputationDegradationFlagV1>,
    /// Raw metrics used to calculate the score.
    pub raw_metrics: ReputationProviderMetricsV1,
    /// Hash of the raw metrics payload.
    pub raw_metrics_hash: [u8; 32],
}
impl ProviderReputationV1 {
    /// Validate the provider reputation record.
    pub fn validate(&self) -> Result<(), ReputationValidationError> {
        if self.version != PROVIDER_REPUTATION_VERSION_V1 {
            return Err(ReputationValidationError::UnsupportedProviderVersion {
                found: self.version,
            });
        }
        validate_provider_id(&self.provider_id)?;
        validate_bps("score_bps", self.score_bps)?;
        if !(MIN_REPUTATION_SCORE_BPS..=MAX_REPUTATION_SCORE_BPS).contains(&self.score_bps) {
            return Err(ReputationValidationError::ScoreOutOfBounds {
                provider_id: self.provider_id.clone(),
                score_bps: self.score_bps,
            });
        }
        if self.degradation_flags.len() > MAX_REPUTATION_DEGRADATION_FLAGS {
            return Err(ReputationValidationError::TooManyDegradationFlags {
                provider_id: self.provider_id.clone(),
                count: self.degradation_flags.len(),
                max: MAX_REPUTATION_DEGRADATION_FLAGS,
            });
        }
        self.raw_metrics.validate()?;
        if self.raw_metrics_hash != hash_norito(&self.raw_metrics)? {
            return Err(ReputationValidationError::RawMetricsHashMismatch {
                provider_id: self.provider_id.clone(),
            });
        }
        ensure_sorted_unique_flags(&self.degradation_flags, &self.provider_id)?;
        Ok(())
    }
}
/// Merkle proof for one provider reputation record.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ReputationMerkleProofV1 {
    /// Provider identifier proven by this proof.
    pub provider_id: String,
    /// Zero-based leaf index after provider-id sorting.
    pub leaf_index: u32,
    /// Exact number of leaves committed by the snapshot.
    pub leaf_count: u32,
    /// Sibling hashes from leaf to root.
    pub siblings: Vec<[u8; 32]>,
}
impl ReputationMerkleProofV1 {
    /// Verify this proof for a provider record and expected root.
    pub fn verify(
        &self,
        provider: &ProviderReputationV1,
        expected_root: [u8; 32],
    ) -> Result<(), ReputationValidationError> {
        validate_provider_id(&self.provider_id)?;
        if self.provider_id != provider.provider_id {
            return Err(ReputationValidationError::ProofProviderMismatch {
                proof_provider_id: self.provider_id.clone(),
                record_provider_id: provider.provider_id.clone(),
            });
        }
        if self.siblings.len() > MAX_REPUTATION_MERKLE_PROOF_LEN {
            return Err(ReputationValidationError::ProofTooLong {
                len: self.siblings.len(),
            });
        }
        let leaf_count = usize::try_from(self.leaf_count).map_err(|_| {
            ReputationValidationError::ProofLeafCountOverflow {
                leaf_count: self.leaf_count,
            }
        })?;
        if leaf_count == 0 || leaf_count > MAX_REPUTATION_PROVIDERS {
            return Err(ReputationValidationError::InvalidProofLeafCount {
                leaf_count: self.leaf_count,
            });
        }
        provider.validate()?;
        let raw_root = verify_merkle_path(
            reputation_leaf_hash(provider)?,
            usize::try_from(self.leaf_index).map_err(|_| {
                ReputationValidationError::ProofLeafIndexOverflow {
                    leaf_index: self.leaf_index,
                }
            })?,
            leaf_count,
            &self.siblings,
        )?;
        let root = merkle_root_commitment(raw_root, leaf_count)?;
        if root != expected_root {
            return Err(ReputationValidationError::MerkleRootMismatch);
        }
        Ok(())
    }
}
/// Published reputation snapshot.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ReputationSnapshotV1 {
    /// Schema version (`REPUTATION_SNAPSHOT_VERSION_V1`).
    pub version: u8,
    /// Snapshot identifier.
    pub snapshot_id: [u8; 16],
    /// Unix timestamp (seconds) when the snapshot was generated.
    pub generated_at_unix: u64,
    /// EigenTrust alpha value advertised for this scoring run, in basis points.
    pub alpha_bps: u16,
    /// Current-score smoothing weight, in basis points.
    pub current_score_weight_bps: u16,
    /// Weights used for the scoring run.
    pub weights: ReputationWeightsV1,
    /// Provider entries sorted lexicographically by provider identifier.
    pub providers: Vec<ProviderReputationV1>,
    /// Merkle root over provider entries.
    pub merkle_root: [u8; 32],
    /// Optional previous snapshot identifier, which must be nonzero when present.
    #[norito(default)]
    pub previous_snapshot_id: Option<[u8; 16]>,
}
impl ReputationSnapshotV1 {
    /// Build and validate a snapshot from scored provider records.
    pub fn from_providers(
        snapshot_id: [u8; 16],
        generated_at_unix: u64,
        weights: ReputationWeightsV1,
        mut providers: Vec<ProviderReputationV1>,
        previous_snapshot_id: Option<[u8; 16]>,
    ) -> Result<Self, ReputationValidationError> {
        sort_provider_records(&mut providers);
        let merkle_root = compute_reputation_merkle_root(&providers)?;
        let snapshot = Self {
            version: REPUTATION_SNAPSHOT_VERSION_V1,
            snapshot_id,
            generated_at_unix,
            alpha_bps: DEFAULT_EIGENTRUST_ALPHA_BPS,
            current_score_weight_bps: DEFAULT_CURRENT_SCORE_WEIGHT_BPS,
            weights,
            providers,
            merkle_root,
            previous_snapshot_id,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
    /// Validate the snapshot and all provider records.
    pub fn validate(&self) -> Result<(), ReputationValidationError> {
        if self.version != REPUTATION_SNAPSHOT_VERSION_V1 {
            return Err(ReputationValidationError::UnsupportedSnapshotVersion {
                found: self.version,
            });
        }
        if self.snapshot_id.iter().all(|&byte| byte == 0) {
            return Err(ReputationValidationError::InvalidSnapshotId);
        }
        if self.generated_at_unix == 0 {
            return Err(ReputationValidationError::InvalidGeneratedAt);
        }
        if self
            .previous_snapshot_id
            .is_some_and(|snapshot_id| snapshot_id.iter().all(|&byte| byte == 0))
        {
            return Err(ReputationValidationError::InvalidPreviousSnapshotId);
        }
        if self.previous_snapshot_id == Some(self.snapshot_id) {
            return Err(ReputationValidationError::SelfReferentialSnapshot);
        }
        validate_bps("alpha_bps", self.alpha_bps)?;
        validate_bps("current_score_weight_bps", self.current_score_weight_bps)?;
        if self.alpha_bps != DEFAULT_EIGENTRUST_ALPHA_BPS {
            return Err(ReputationValidationError::NonCanonicalAlpha {
                found: self.alpha_bps,
            });
        }
        if self.current_score_weight_bps != DEFAULT_CURRENT_SCORE_WEIGHT_BPS {
            return Err(ReputationValidationError::NonCanonicalSmoothingWeight {
                found: self.current_score_weight_bps,
            });
        }
        self.weights.validate()?;
        if self.providers.is_empty() {
            return Err(ReputationValidationError::EmptyProviderSet);
        }
        validate_provider_count(self.providers.len())?;
        validate_sorted_providers(&self.providers)?;
        let root = compute_reputation_merkle_root(&self.providers)?;
        if self.merkle_root != root {
            return Err(ReputationValidationError::MerkleRootMismatch);
        }
        Ok(())
    }
    /// Construct a Merkle proof for a provider in this snapshot.
    pub fn merkle_proof(
        &self,
        provider_id: &str,
    ) -> Result<ReputationMerkleProofV1, ReputationValidationError> {
        self.validate()?;
        let leaf_index = self
            .providers
            .binary_search_by(|entry| entry.provider_id.as_str().cmp(provider_id))
            .map_err(|_| ReputationValidationError::ProviderNotFound {
                provider_id: provider_id.to_string(),
            })?;
        let leaves = reputation_leaf_hashes(&self.providers)?;
        let siblings = merkle_siblings(&leaves, leaf_index)?;
        let leaf_count = u32::try_from(self.providers.len()).map_err(|_| {
            ReputationValidationError::ProviderCountOverflow {
                count: self.providers.len(),
            }
        })?;
        Ok(ReputationMerkleProofV1 {
            provider_id: provider_id.to_string(),
            leaf_index: u32::try_from(leaf_index).map_err(|_| {
                ReputationValidationError::ProviderCountOverflow {
                    count: self.providers.len(),
                }
            })?,
            leaf_count,
            siblings,
        })
    }
}
/// Event emitted when a reputation snapshot is accepted for publication.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ReputationSnapshotEventV1 {
    /// Schema version (`REPUTATION_SNAPSHOT_EVENT_VERSION_V1`).
    pub version: u8,
    /// Monotonic local event sequence.
    pub sequence: u64,
    /// Snapshot identifier.
    pub snapshot_id: [u8; 16],
    /// Unix timestamp (seconds) when the snapshot was generated.
    pub generated_at_unix: u64,
    /// Merkle root over provider entries.
    pub merkle_root: [u8; 32],
    /// Number of provider entries in the snapshot.
    pub provider_count: u32,
    /// Optional previous snapshot identifier, which must be nonzero when present.
    #[norito(default)]
    pub previous_snapshot_id: Option<[u8; 16]>,
}
impl ReputationSnapshotEventV1 {
    /// Build an event from a validated snapshot and assigned sequence.
    pub fn from_snapshot(
        sequence: u64,
        snapshot: &ReputationSnapshotV1,
    ) -> Result<Self, ReputationValidationError> {
        snapshot.validate()?;
        let provider_count = u32::try_from(snapshot.providers.len()).map_err(|_| {
            ReputationValidationError::ProviderCountOverflow {
                count: snapshot.providers.len(),
            }
        })?;
        let event = Self {
            version: REPUTATION_SNAPSHOT_EVENT_VERSION_V1,
            sequence,
            snapshot_id: snapshot.snapshot_id,
            generated_at_unix: snapshot.generated_at_unix,
            merkle_root: snapshot.merkle_root,
            provider_count,
            previous_snapshot_id: snapshot.previous_snapshot_id,
        };
        event.validate()?;
        Ok(event)
    }
    /// Validate the snapshot event envelope.
    pub fn validate(&self) -> Result<(), ReputationValidationError> {
        if self.version != REPUTATION_SNAPSHOT_EVENT_VERSION_V1 {
            return Err(ReputationValidationError::UnsupportedSnapshotEventVersion {
                found: self.version,
            });
        }
        if self.sequence == 0 {
            return Err(ReputationValidationError::InvalidSnapshotEventSequence);
        }
        if self.snapshot_id.iter().all(|&byte| byte == 0) {
            return Err(ReputationValidationError::InvalidSnapshotId);
        }
        if self.generated_at_unix == 0 {
            return Err(ReputationValidationError::InvalidGeneratedAt);
        }
        if self.provider_count == 0 {
            return Err(ReputationValidationError::EmptyProviderSet);
        }
        if usize::try_from(self.provider_count)
            .ok()
            .is_none_or(|count| count > MAX_REPUTATION_PROVIDERS)
        {
            return Err(ReputationValidationError::TooManyProviders {
                count: usize::try_from(self.provider_count).unwrap_or(usize::MAX),
                max: MAX_REPUTATION_PROVIDERS,
            });
        }
        if self
            .previous_snapshot_id
            .is_some_and(|snapshot_id| snapshot_id.iter().all(|&byte| byte == 0))
        {
            return Err(ReputationValidationError::InvalidPreviousSnapshotId);
        }
        if self.previous_snapshot_id == Some(self.snapshot_id) {
            return Err(ReputationValidationError::SelfReferentialSnapshot);
        }
        Ok(())
    }
}
/// Score provider inputs and return a validated reputation snapshot.
pub fn build_reputation_snapshot(
    snapshot_id: [u8; 16],
    generated_at_unix: u64,
    weights: ReputationWeightsV1,
    provider_inputs: &[ReputationProviderInputV1],
    previous_snapshot_id: Option<[u8; 16]>,
) -> Result<ReputationSnapshotV1, ReputationValidationError> {
    build_reputation_snapshot_with_trust_edges(
        snapshot_id,
        generated_at_unix,
        weights,
        provider_inputs,
        &[],
        previous_snapshot_id,
    )
}
/// Score provider inputs, apply pairwise trust edges, and return a validated snapshot.
pub fn build_reputation_snapshot_with_trust_edges(
    snapshot_id: [u8; 16],
    generated_at_unix: u64,
    weights: ReputationWeightsV1,
    provider_inputs: &[ReputationProviderInputV1],
    trust_edges: &[ReputationTrustEdgeV1],
    previous_snapshot_id: Option<[u8; 16]>,
) -> Result<ReputationSnapshotV1, ReputationValidationError> {
    weights.validate()?;
    validate_provider_count(provider_inputs.len())?;
    validate_trust_edge_count(trust_edges.len())?;
    let mut providers = Vec::new();
    providers
        .try_reserve_exact(provider_inputs.len())
        .map_err(|_| ReputationValidationError::AllocationFailed {
            context: "provider records",
        })?;
    for input in provider_inputs {
        providers.push(score_provider_reputation(input, &weights)?);
    }
    sort_provider_records(&mut providers);
    validate_sorted_providers(&providers)?;
    apply_eigentrust_edges(&mut providers, trust_edges, DEFAULT_EIGENTRUST_ALPHA_BPS)?;
    ReputationSnapshotV1::from_providers(
        snapshot_id,
        generated_at_unix,
        weights,
        providers,
        previous_snapshot_id,
    )
}
/// Score one provider using deterministic fixed-point arithmetic.
pub fn score_provider_reputation(
    input: &ReputationProviderInputV1,
    weights: &ReputationWeightsV1,
) -> Result<ProviderReputationV1, ReputationValidationError> {
    input.validate()?;
    weights.validate()?;
    let metrics = input.metrics;
    let positive = weighted_component(metrics.por_success_bps, weights.por_success_bps)
        + weighted_component(metrics.pdp_success_bps, weights.pdp_success_bps)
        + weighted_component(metrics.potr_success_bps, weights.potr_success_bps)
        + weighted_component(metrics.latency_health_bps, weights.latency_bps);
    let negative = weighted_component(metrics.dispute_rate_bps, weights.dispute_bps)
        + weighted_component(
            metrics.token_violation_rate_bps,
            weights.token_violation_bps,
        )
        + weighted_component(metrics.repair_breach_rate_bps, weights.repair_breach_bps);
    let mut score = positive.saturating_sub(negative);
    let mut flags = Vec::new();
    apply_reserve_stage(input.reserve_stage, &mut score, &mut flags);
    apply_proof_success_penalty(metrics, &mut score, &mut flags);
    if input.active_dispute {
        flags.push(ReputationDegradationFlagV1::ActiveDispute);
        score = score.min(2_000);
    }
    if input.slashing_event {
        flags.push(ReputationDegradationFlagV1::SlashingEvent);
        score = score.min(2_000);
    }
    if let Some(previous) = input.previous_score_bps {
        let current_weight = u32::from(DEFAULT_CURRENT_SCORE_WEIGHT_BPS);
        let previous_weight = u32::from(REPUTATION_BASIS_POINTS) - current_weight;
        score = ((score * current_weight) + (u32::from(previous) * previous_weight))
            / u32::from(REPUTATION_BASIS_POINTS);
    }
    let bounded = u16::try_from(score.clamp(
        u32::from(MIN_REPUTATION_SCORE_BPS),
        u32::from(MAX_REPUTATION_SCORE_BPS),
    ))
    .map_err(|_| ReputationValidationError::ArithmeticOverflow {
        context: "bounded provider reputation score",
    })?;
    if bounded < LOW_REPUTATION_SCORE_FLAG_BPS {
        flags.push(ReputationDegradationFlagV1::LowScore);
    }
    flags.sort();
    flags.dedup();
    let raw_metrics_hash = hash_norito(&metrics)?;
    let record = ProviderReputationV1 {
        version: PROVIDER_REPUTATION_VERSION_V1,
        provider_id: input.provider_id.clone(),
        score_bps: bounded,
        degradation_flags: flags,
        raw_metrics: metrics,
        raw_metrics_hash,
    };
    record.validate()?;
    Ok(record)
}
/// Compute the Merkle root for sorted provider records.
pub fn compute_reputation_merkle_root(
    providers: &[ProviderReputationV1],
) -> Result<[u8; 32], ReputationValidationError> {
    validate_provider_count(providers.len())?;
    let mut leaves = reputation_leaf_hashes(providers)?;
    if leaves.is_empty() {
        return Ok([0_u8; 32]);
    }
    while leaves.len() > 1 {
        let parent_count = leaves.len().div_ceil(2);
        let mut parents = Vec::new();
        parents.try_reserve_exact(parent_count).map_err(|_| {
            ReputationValidationError::AllocationFailed {
                context: "reputation Merkle level",
            }
        })?;
        for pair in leaves.chunks(2) {
            let left = pair[0];
            let right = pair.get(1).copied().unwrap_or(left);
            parents.push(merkle_parent(left, right));
        }
        leaves = parents;
    }
    merkle_root_commitment(leaves[0], providers.len())
}
fn reputation_leaf_hashes(
    providers: &[ProviderReputationV1],
) -> Result<Vec<[u8; 32]>, ReputationValidationError> {
    validate_provider_count(providers.len())?;
    let mut leaves = Vec::new();
    leaves.try_reserve_exact(providers.len()).map_err(|_| {
        ReputationValidationError::AllocationFailed {
            context: "reputation Merkle leaves",
        }
    })?;
    for provider in providers {
        leaves.push(reputation_leaf_hash(provider)?);
    }
    Ok(leaves)
}
fn reputation_leaf_hash(
    provider: &ProviderReputationV1,
) -> Result<[u8; 32], ReputationValidationError> {
    provider.validate()?;
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs-reputation-leaf-v1");
    hasher.update(provider.provider_id.as_bytes());
    hasher.update(&provider.score_bps.to_le_bytes());
    hasher.update(provider.raw_metrics_hash.as_slice());
    for flag in &provider.degradation_flags {
        hasher.update(&[*flag as u8]);
    }
    Ok(hash_to_array(hasher.finalize()))
}
fn merkle_parent(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs-reputation-node-v1");
    hasher.update(&left);
    hasher.update(&right);
    hash_to_array(hasher.finalize())
}
fn merkle_root_commitment(
    raw_root: [u8; 32],
    leaf_count: usize,
) -> Result<[u8; 32], ReputationValidationError> {
    let leaf_count = u32::try_from(leaf_count)
        .map_err(|_| ReputationValidationError::ProviderCountOverflow { count: leaf_count })?;
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs-reputation-root-v1");
    hasher.update(&leaf_count.to_le_bytes());
    hasher.update(&raw_root);
    Ok(hash_to_array(hasher.finalize()))
}
fn merkle_siblings(
    leaves: &[[u8; 32]],
    leaf_index: usize,
) -> Result<Vec<[u8; 32]>, ReputationValidationError> {
    if leaves.is_empty() {
        return Ok(Vec::new());
    }
    let mut index = leaf_index;
    let mut level = Vec::new();
    level.try_reserve_exact(leaves.len()).map_err(|_| {
        ReputationValidationError::AllocationFailed {
            context: "reputation proof leaf level",
        }
    })?;
    level.extend_from_slice(leaves);
    let mut siblings = Vec::new();
    siblings
        .try_reserve_exact(merkle_depth(leaves.len()))
        .map_err(|_| ReputationValidationError::AllocationFailed {
            context: "reputation proof siblings",
        })?;
    while level.len() > 1 {
        let sibling_index = if index.is_multiple_of(2) {
            (index + 1).min(level.len() - 1)
        } else {
            index - 1
        };
        siblings.push(level[sibling_index]);
        let mut parents = Vec::new();
        parents
            .try_reserve_exact(level.len().div_ceil(2))
            .map_err(|_| ReputationValidationError::AllocationFailed {
                context: "reputation proof parent level",
            })?;
        for pair in level.chunks(2) {
            let left = pair[0];
            let right = pair.get(1).copied().unwrap_or(left);
            parents.push(merkle_parent(left, right));
        }
        level = parents;
        index /= 2;
    }
    Ok(siblings)
}
fn verify_merkle_path(
    leaf: [u8; 32],
    leaf_index: usize,
    leaf_count: usize,
    siblings: &[[u8; 32]],
) -> Result<[u8; 32], ReputationValidationError> {
    if leaf_count == 0 || leaf_index >= leaf_count {
        return Err(ReputationValidationError::ProofLeafIndexOutOfRange {
            leaf_index: u64::try_from(leaf_index).unwrap_or(u64::MAX),
            leaf_count: u64::try_from(leaf_count).unwrap_or(u64::MAX),
        });
    }
    let expected_depth = merkle_depth(leaf_count);
    if siblings.len() != expected_depth {
        return Err(ReputationValidationError::ProofDepthMismatch {
            expected: expected_depth,
            found: siblings.len(),
        });
    }
    let mut node = leaf;
    let mut index = leaf_index;
    let mut width = leaf_count;
    for sibling in siblings {
        if !width.is_multiple_of(2) && index == width - 1 && *sibling != node {
            return Err(ReputationValidationError::NonCanonicalOddLeafSibling);
        }
        node = if index.is_multiple_of(2) {
            merkle_parent(node, *sibling)
        } else {
            merkle_parent(*sibling, node)
        };
        index /= 2;
        width = width.div_ceil(2);
    }
    if width != 1 || index != 0 {
        return Err(ReputationValidationError::InvalidProofGeometry);
    }
    Ok(node)
}
fn merkle_depth(mut leaf_count: usize) -> usize {
    let mut depth = 0;
    while leaf_count > 1 {
        leaf_count = leaf_count.div_ceil(2);
        depth += 1;
    }
    depth
}
fn apply_eigentrust_edges(
    providers: &mut [ProviderReputationV1],
    trust_edges: &[ReputationTrustEdgeV1],
    alpha_bps: u16,
) -> Result<(), ReputationValidationError> {
    if trust_edges.is_empty() || providers.is_empty() {
        return Ok(());
    }
    validate_bps("alpha_bps", alpha_bps)?;
    validate_provider_count(providers.len())?;
    validate_trust_edges(trust_edges)?;
    validate_sorted_providers(providers)?;
    let len = providers.len();
    let mut row_counts = try_zeroed_vec::<usize>(len, "reputation trust row counts")?;
    for edge in trust_edges {
        let from = provider_index(providers, &edge.from_provider_id).ok_or_else(|| {
            ReputationValidationError::TrustEdgeUnknownProvider {
                provider_id: edge.from_provider_id.clone(),
            }
        })?;
        let to = provider_index(providers, &edge.to_provider_id).ok_or_else(|| {
            ReputationValidationError::TrustEdgeUnknownProvider {
                provider_id: edge.to_provider_id.clone(),
            }
        })?;
        let _ = to;
        row_counts[from] = row_counts[from].checked_add(1).ok_or(
            ReputationValidationError::ArithmeticOverflow {
                context: "reputation trust row edge count",
            },
        )?;
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(len)
        .map_err(|_| ReputationValidationError::AllocationFailed {
            context: "reputation sparse trust rows",
        })?;
    for count in row_counts {
        let mut row = Vec::new();
        row.try_reserve_exact(count)
            .map_err(|_| ReputationValidationError::AllocationFailed {
                context: "reputation sparse trust row",
            })?;
        rows.push(row);
    }
    for edge in trust_edges {
        let from = provider_index(providers, &edge.from_provider_id).ok_or_else(|| {
            ReputationValidationError::TrustEdgeUnknownProvider {
                provider_id: edge.from_provider_id.clone(),
            }
        })?;
        let to = provider_index(providers, &edge.to_provider_id).ok_or_else(|| {
            ReputationValidationError::TrustEdgeUnknownProvider {
                provider_id: edge.to_provider_id.clone(),
            }
        })?;
        rows[from].push((to, edge.trust_bps));
    }
    for row in &mut rows {
        if row.is_empty() {
            continue;
        }
        let total = row.iter().try_fold(0_u64, |total, (_, value)| {
            total.checked_add(u64::from(*value)).ok_or(
                ReputationValidationError::ArithmeticOverflow {
                    context: "reputation trust row sum",
                },
            )
        })?;
        debug_assert!(
            total > 0,
            "zero trust edges are rejected before normalization"
        );
        let mut assigned = 0_u64;
        for (_, value) in row.iter_mut() {
            let normalised = u64::from(*value)
                .checked_mul(u64::from(REPUTATION_BASIS_POINTS))
                .ok_or(ReputationValidationError::ArithmeticOverflow {
                    context: "reputation trust normalization",
                })?
                / total;
            *value = u16::try_from(normalised).map_err(|_| {
                ReputationValidationError::ArithmeticOverflow {
                    context: "normalized reputation trust edge",
                }
            })?;
            assigned = assigned.checked_add(normalised).ok_or(
                ReputationValidationError::ArithmeticOverflow {
                    context: "normalized reputation trust row sum",
                },
            )?;
        }
        let remainder = u64::from(REPUTATION_BASIS_POINTS)
            .checked_sub(assigned)
            .ok_or(ReputationValidationError::ArithmeticOverflow {
                context: "normalized reputation trust remainder",
            })?;
        let (_, last_value) = row
            .last_mut()
            .ok_or(ReputationValidationError::InvalidTrustGraph)?;
        *last_value = last_value
            .checked_add(u16::try_from(remainder).map_err(|_| {
                ReputationValidationError::ArithmeticOverflow {
                    context: "normalized reputation trust remainder",
                }
            })?)
            .ok_or(ReputationValidationError::ArithmeticOverflow {
                context: "normalized reputation trust edge remainder",
            })?;
    }
    let baseline = try_collect_scores(providers)?;
    let mut rank = try_clone_u64(&baseline, "reputation rank vector")?;
    let mut propagated = try_zeroed_vec::<u64>(len, "reputation propagated rank vector")?;
    let mut next = try_zeroed_vec::<u64>(len, "reputation next rank vector")?;
    for _ in 0..REPUTATION_EIGENTRUST_MAX_ITERATIONS {
        propagated.fill(0);
        for (from, row) in rows.iter().enumerate() {
            if row.is_empty() {
                propagated[from] = propagated[from].checked_add(rank[from]).ok_or(
                    ReputationValidationError::ArithmeticOverflow {
                        context: "reputation self-trust propagation",
                    },
                )?;
                continue;
            }
            let mut assigned = 0_u64;
            for (position, (to, trust_bps)) in row.iter().enumerate() {
                let contribution = if position + 1 == row.len() {
                    rank[from].checked_sub(assigned).ok_or(
                        ReputationValidationError::ArithmeticOverflow {
                            context: "reputation trust propagation remainder",
                        },
                    )?
                } else {
                    u64::from(*trust_bps).checked_mul(rank[from]).ok_or(
                        ReputationValidationError::ArithmeticOverflow {
                            context: "reputation trust propagation",
                        },
                    )? / u64::from(REPUTATION_BASIS_POINTS)
                };
                assigned = assigned.checked_add(contribution).ok_or(
                    ReputationValidationError::ArithmeticOverflow {
                        context: "reputation assigned trust propagation",
                    },
                )?;
                propagated[*to] = propagated[*to].checked_add(contribution).ok_or(
                    ReputationValidationError::ArithmeticOverflow {
                        context: "reputation inbound trust sum",
                    },
                )?;
            }
        }
        for to in 0..len {
            let propagated_component = u64::from(alpha_bps).checked_mul(propagated[to]).ok_or(
                ReputationValidationError::ArithmeticOverflow {
                    context: "reputation propagated score mix",
                },
            )?;
            let baseline_component = (u64::from(REPUTATION_BASIS_POINTS) - u64::from(alpha_bps))
                .checked_mul(baseline[to])
                .ok_or(ReputationValidationError::ArithmeticOverflow {
                    context: "reputation baseline score mix",
                })?;
            let mixed = propagated_component.checked_add(baseline_component).ok_or(
                ReputationValidationError::ArithmeticOverflow {
                    context: "reputation score mix",
                },
            )? / u64::from(REPUTATION_BASIS_POINTS);
            next[to] = mixed;
        }
        let delta = rank
            .iter()
            .zip(next.iter())
            .try_fold(0_u64, |delta, (left, right)| {
                delta.checked_add(left.abs_diff(*right)).ok_or(
                    ReputationValidationError::ArithmeticOverflow {
                        context: "reputation convergence delta",
                    },
                )
            })?;
        std::mem::swap(&mut rank, &mut next);
        if delta <= REPUTATION_EIGENTRUST_CONVERGENCE_L1_BPS {
            break;
        }
    }
    for (provider, eigentrust_score) in providers.iter_mut().zip(rank) {
        let bounded_eigentrust = u16::try_from(eigentrust_score.clamp(
            u64::from(MIN_REPUTATION_SCORE_BPS),
            u64::from(MAX_REPUTATION_SCORE_BPS),
        ))
        .map_err(|_| ReputationValidationError::ArithmeticOverflow {
            context: "bounded reputation trust score",
        })?;
        provider.score_bps = provider.score_bps.min(bounded_eigentrust);
        refresh_low_score_flag(provider)?;
    }
    Ok(())
}
fn try_collect_scores(
    providers: &[ProviderReputationV1],
) -> Result<Vec<u64>, ReputationValidationError> {
    let mut scores = Vec::new();
    scores.try_reserve_exact(providers.len()).map_err(|_| {
        ReputationValidationError::AllocationFailed {
            context: "reputation baseline scores",
        }
    })?;
    scores.extend(
        providers
            .iter()
            .map(|provider| u64::from(provider.score_bps)),
    );
    Ok(scores)
}
fn try_clone_u64(
    source: &[u64],
    context: &'static str,
) -> Result<Vec<u64>, ReputationValidationError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(source.len())
        .map_err(|_| ReputationValidationError::AllocationFailed { context })?;
    output.extend_from_slice(source);
    Ok(output)
}
fn try_zeroed_vec<T: Default + Clone>(
    len: usize,
    context: &'static str,
) -> Result<Vec<T>, ReputationValidationError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(len)
        .map_err(|_| ReputationValidationError::AllocationFailed { context })?;
    output.resize(len, T::default());
    Ok(output)
}
fn provider_index(providers: &[ProviderReputationV1], provider_id: &str) -> Option<usize> {
    providers
        .binary_search_by(|entry| entry.provider_id.as_str().cmp(provider_id))
        .ok()
}
fn refresh_low_score_flag(
    provider: &mut ProviderReputationV1,
) -> Result<(), ReputationValidationError> {
    provider
        .degradation_flags
        .retain(|flag| *flag != ReputationDegradationFlagV1::LowScore);
    if provider.score_bps < LOW_REPUTATION_SCORE_FLAG_BPS {
        provider
            .degradation_flags
            .push(ReputationDegradationFlagV1::LowScore);
    }
    provider.degradation_flags.sort();
    provider.degradation_flags.dedup();
    provider.validate()
}
fn weighted_component(metric_bps: u16, weight_bps: u16) -> u32 {
    (u32::from(metric_bps) * u32::from(weight_bps)) / u32::from(REPUTATION_BASIS_POINTS)
}
fn apply_reserve_stage(
    stage: ReputationReserveStageV1,
    score: &mut u32,
    flags: &mut Vec<ReputationDegradationFlagV1>,
) {
    match stage {
        ReputationReserveStageV1::Active => {}
        ReputationReserveStageV1::Warning => {
            flags.push(ReputationDegradationFlagV1::ReserveWarning)
        }
        ReputationReserveStageV1::Grace => flags.push(ReputationDegradationFlagV1::ReserveGrace),
        ReputationReserveStageV1::Delinquent => {
            flags.push(ReputationDegradationFlagV1::ReserveDelinquent);
        }
        ReputationReserveStageV1::Default => {
            flags.push(ReputationDegradationFlagV1::ReserveDefault)
        }
    }
    *score = (*score * u32::from(stage.multiplier_bps())) / u32::from(REPUTATION_BASIS_POINTS);
}
fn apply_proof_success_penalty(
    metrics: ReputationProviderMetricsV1,
    score: &mut u32,
    flags: &mut Vec<ReputationDegradationFlagV1>,
) {
    let proof_floor = metrics.por_success_bps.min(metrics.pdp_success_bps);
    if proof_floor < 8_000 {
        flags.push(ReputationDegradationFlagV1::ProofSuccessBelow80);
        *score = (*score * 6_000) / u32::from(REPUTATION_BASIS_POINTS);
    } else if proof_floor < 9_000 {
        flags.push(ReputationDegradationFlagV1::ProofSuccessBelow90);
        *score = (*score * 8_000) / u32::from(REPUTATION_BASIS_POINTS);
    }
}
fn hash_norito<T: norito::NoritoSerialize>(value: &T) -> Result<[u8; 32], NoritoError> {
    let bytes = norito::to_bytes(value)?;
    Ok(hash_to_array(blake3::hash(&bytes)))
}
fn hash_to_array(hash: Hash) -> [u8; 32] {
    let mut out = [0_u8; 32];
    out.copy_from_slice(hash.as_bytes());
    out
}
fn sort_provider_records(providers: &mut [ProviderReputationV1]) {
    providers.sort_by(
        |left, right| match left.provider_id.cmp(&right.provider_id) {
            Ordering::Equal => left.score_bps.cmp(&right.score_bps),
            ordering => ordering,
        },
    );
}
fn validate_sorted_providers(
    providers: &[ProviderReputationV1],
) -> Result<(), ReputationValidationError> {
    let mut previous: Option<&str> = None;
    for provider in providers {
        provider.validate()?;
        if let Some(prev) = previous {
            if prev == provider.provider_id {
                return Err(ReputationValidationError::DuplicateProviderId {
                    provider_id: provider.provider_id.clone(),
                });
            }
            if prev > provider.provider_id.as_str() {
                return Err(ReputationValidationError::ProvidersNotSorted);
            }
        }
        previous = Some(&provider.provider_id);
    }
    Ok(())
}
fn ensure_sorted_unique_flags(
    flags: &[ReputationDegradationFlagV1],
    provider_id: &str,
) -> Result<(), ReputationValidationError> {
    let mut previous = None;
    for flag in flags {
        if previous.is_some_and(|prev| prev == *flag) {
            return Err(ReputationValidationError::DuplicateFlag {
                provider_id: provider_id.to_string(),
                flag: *flag,
            });
        }
        if previous.is_some_and(|prev| prev > *flag) {
            return Err(ReputationValidationError::FlagsNotSorted {
                provider_id: provider_id.to_string(),
            });
        }
        previous = Some(*flag);
    }
    Ok(())
}
fn validate_provider_id(provider_id: &str) -> Result<(), ReputationValidationError> {
    if provider_id.trim().is_empty() || matches!(provider_id, "." | "..") {
        return Err(ReputationValidationError::InvalidProviderId);
    }
    if provider_id.len() > 256 {
        return Err(ReputationValidationError::ProviderIdTooLong {
            len: provider_id.len(),
        });
    }
    if !provider_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(ReputationValidationError::InvalidProviderId);
    }
    Ok(())
}
fn validate_provider_count(count: usize) -> Result<(), ReputationValidationError> {
    if count > MAX_REPUTATION_PROVIDERS {
        return Err(ReputationValidationError::TooManyProviders {
            count,
            max: MAX_REPUTATION_PROVIDERS,
        });
    }
    Ok(())
}
fn validate_trust_edge_count(count: usize) -> Result<(), ReputationValidationError> {
    if count > MAX_REPUTATION_TRUST_EDGES {
        return Err(ReputationValidationError::TooManyTrustEdges {
            count,
            max: MAX_REPUTATION_TRUST_EDGES,
        });
    }
    Ok(())
}
fn validate_trust_edges(
    trust_edges: &[ReputationTrustEdgeV1],
) -> Result<(), ReputationValidationError> {
    validate_trust_edge_count(trust_edges.len())?;
    let mut previous: Option<(&str, &str)> = None;
    for edge in trust_edges {
        edge.validate()?;
        let key = (edge.from_provider_id.as_str(), edge.to_provider_id.as_str());
        if let Some(previous_key) = previous {
            match previous_key.cmp(&key) {
                Ordering::Equal => {
                    return Err(ReputationValidationError::DuplicateTrustEdge {
                        from_provider_id: edge.from_provider_id.clone(),
                        to_provider_id: edge.to_provider_id.clone(),
                    });
                }
                Ordering::Greater => {
                    return Err(ReputationValidationError::TrustEdgesNotSorted);
                }
                Ordering::Less => {}
            }
        }
        previous = Some(key);
    }
    Ok(())
}
fn validate_bps(field: &'static str, value: u16) -> Result<(), ReputationValidationError> {
    if value > REPUTATION_BASIS_POINTS {
        return Err(ReputationValidationError::BasisPointsOutOfRange { field, value });
    }
    Ok(())
}
/// Reputation validation and proof errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReputationValidationError {
    /// Weights schema version is unsupported.
    #[error("unsupported reputation weights version {found}")]
    UnsupportedWeightsVersion {
        /// Found version.
        found: u8,
    },
    /// Provider metrics schema version is unsupported.
    #[error("unsupported reputation metrics version {found}")]
    UnsupportedMetricsVersion {
        /// Found version.
        found: u8,
    },
    /// Provider input schema version is unsupported.
    #[error("unsupported reputation provider input version {found}")]
    UnsupportedInputVersion {
        /// Found version.
        found: u8,
    },
    /// Trust-edge schema version is unsupported.
    #[error("unsupported reputation trust-edge version {found}")]
    UnsupportedTrustEdgeVersion {
        /// Found version.
        found: u8,
    },
    /// Provider reputation schema version is unsupported.
    #[error("unsupported provider reputation version {found}")]
    UnsupportedProviderVersion {
        /// Found version.
        found: u8,
    },
    /// Reputation snapshot schema version is unsupported.
    #[error("unsupported reputation snapshot version {found}")]
    UnsupportedSnapshotVersion {
        /// Found version.
        found: u8,
    },
    /// Reputation snapshot event schema version is unsupported.
    #[error("unsupported reputation snapshot event version {found}")]
    UnsupportedSnapshotEventVersion {
        /// Found version.
        found: u8,
    },
    /// Weight total is not exactly 10_000 basis points.
    #[error("reputation weights must sum to 10_000 bps, got {total_bps}")]
    InvalidWeightTotal {
        /// Observed total.
        total_bps: u32,
    },
    /// A basis-point value is outside the allowed range.
    #[error("{field}={value} is outside 0..=10_000 bps")]
    BasisPointsOutOfRange {
        /// Field name.
        field: &'static str,
        /// Observed value.
        value: u16,
    },
    /// Provider identifier is empty or malformed.
    #[error("provider identifier is empty or malformed")]
    InvalidProviderId,
    /// Provider identifier is too long.
    #[error("provider identifier length {len} exceeds 256 bytes")]
    ProviderIdTooLong {
        /// Observed length.
        len: usize,
    },
    /// Provider count exceeds the production safety limit.
    #[error("reputation provider count {count} exceeds maximum {max}")]
    TooManyProviders {
        /// Observed provider count.
        count: usize,
        /// Maximum provider count.
        max: usize,
    },
    /// Trust-edge count exceeds the production safety limit.
    #[error("reputation trust-edge count {count} exceeds maximum {max}")]
    TooManyTrustEdges {
        /// Observed trust-edge count.
        count: usize,
        /// Maximum trust-edge count.
        max: usize,
    },
    /// A trust edge refers to the same provider at both ends.
    #[error("self trust edge for provider `{provider_id}` is not allowed")]
    SelfTrustEdge {
        /// Provider identifier.
        provider_id: String,
    },
    /// Zero-value trust edges are non-canonical.
    #[error("zero-value reputation trust edge is not allowed")]
    ZeroTrustEdge,
    /// A duplicate directed trust edge was supplied.
    #[error("duplicate reputation trust edge `{from_provider_id}` -> `{to_provider_id}`")]
    DuplicateTrustEdge {
        /// Source provider identifier.
        from_provider_id: String,
        /// Destination provider identifier.
        to_provider_id: String,
    },
    /// Trust edges are not in canonical source/destination order.
    #[error("reputation trust edges must be sorted by source and destination provider id")]
    TrustEdgesNotSorted,
    /// The sparse trust graph violated an internal canonical invariant.
    #[error("reputation trust graph is invalid")]
    InvalidTrustGraph,
    /// Raw metrics hash does not match the metrics payload.
    #[error("raw metrics hash mismatch for provider `{provider_id}`")]
    RawMetricsHashMismatch {
        /// Provider identifier.
        provider_id: String,
    },
    /// Score violates published bounds.
    #[error("score {score_bps} bps for provider `{provider_id}` is outside published bounds")]
    ScoreOutOfBounds {
        /// Provider identifier.
        provider_id: String,
        /// Observed score.
        score_bps: u16,
    },
    /// Duplicate provider identifier in a snapshot.
    #[error("duplicate provider `{provider_id}` in reputation snapshot")]
    DuplicateProviderId {
        /// Provider identifier.
        provider_id: String,
    },
    /// Providers are not lexicographically sorted.
    #[error("reputation snapshot providers must be sorted by provider identifier")]
    ProvidersNotSorted,
    /// Reputation snapshots must contain at least one provider.
    #[error("reputation snapshot must contain at least one provider")]
    EmptyProviderSet,
    /// Duplicate degradation flag.
    #[error("duplicate reputation flag {flag:?} for provider `{provider_id}`")]
    DuplicateFlag {
        /// Provider identifier.
        provider_id: String,
        /// Duplicate flag.
        flag: ReputationDegradationFlagV1,
    },
    /// Degradation flags are not sorted.
    #[error("reputation flags for provider `{provider_id}` must be sorted")]
    FlagsNotSorted {
        /// Provider identifier.
        provider_id: String,
    },
    /// A provider record carries more degradation flags than V1 permits.
    #[error("reputation flag count {count} for provider `{provider_id}` exceeds maximum {max}")]
    TooManyDegradationFlags {
        /// Provider identifier.
        provider_id: String,
        /// Observed degradation-flag count.
        count: usize,
        /// Maximum degradation-flag count.
        max: usize,
    },
    /// Snapshot identifier is all zeros.
    #[error("reputation snapshot id must not be all zeros")]
    InvalidSnapshotId,
    /// A present previous snapshot identifier is all zeros.
    #[error("reputation previous snapshot id must not be all zeros")]
    InvalidPreviousSnapshotId,
    /// Snapshot generation timestamp is zero.
    #[error("reputation snapshot generated_at_unix must not be zero")]
    InvalidGeneratedAt,
    /// Snapshot points to itself as its predecessor.
    #[error("reputation snapshot must not reference itself as its predecessor")]
    SelfReferentialSnapshot,
    /// Snapshot alpha differs from the only V1 scoring parameter.
    #[error("reputation snapshot alpha {found} is not canonical for V1")]
    NonCanonicalAlpha {
        /// Observed alpha value.
        found: u16,
    },
    /// Snapshot smoothing weight differs from the only V1 scoring parameter.
    #[error("reputation snapshot smoothing weight {found} is not canonical for V1")]
    NonCanonicalSmoothingWeight {
        /// Observed smoothing value.
        found: u16,
    },
    /// Snapshot event sequence is zero.
    #[error("reputation snapshot event sequence must not be zero")]
    InvalidSnapshotEventSequence,
    /// Provider count cannot be represented in a snapshot event.
    #[error("reputation provider count {count} exceeds event limits")]
    ProviderCountOverflow {
        /// Observed provider count.
        count: usize,
    },
    /// Merkle root mismatch.
    #[error("reputation Merkle root mismatch")]
    MerkleRootMismatch,
    /// Provider was not present in the snapshot.
    #[error("provider `{provider_id}` was not found in the reputation snapshot")]
    ProviderNotFound {
        /// Provider identifier.
        provider_id: String,
    },
    /// Proof references a different provider than the record.
    #[error("proof provider `{proof_provider_id}` does not match record `{record_provider_id}`")]
    ProofProviderMismatch {
        /// Provider id in proof.
        proof_provider_id: String,
        /// Provider id in record.
        record_provider_id: String,
    },
    /// Proof length exceeds the configured maximum.
    #[error("reputation Merkle proof has {len} siblings, exceeding the maximum")]
    ProofTooLong {
        /// Observed proof length.
        len: usize,
    },
    /// Leaf index could not be represented on this host.
    #[error("reputation proof leaf index {leaf_index} cannot be represented")]
    ProofLeafIndexOverflow {
        /// Observed leaf index.
        leaf_index: u32,
    },
    /// Leaf count could not be represented on this host.
    #[error("reputation proof leaf count {leaf_count} cannot be represented")]
    ProofLeafCountOverflow {
        /// Observed leaf count.
        leaf_count: u32,
    },
    /// Leaf count is zero or exceeds the snapshot limit.
    #[error("reputation proof leaf count {leaf_count} is invalid")]
    InvalidProofLeafCount {
        /// Observed leaf count.
        leaf_count: u32,
    },
    /// Proof leaf index lies outside its committed tree.
    #[error("reputation proof leaf index {leaf_index} is outside leaf count {leaf_count}")]
    ProofLeafIndexOutOfRange {
        /// Observed leaf index.
        leaf_index: u64,
        /// Committed leaf count.
        leaf_count: u64,
    },
    /// Proof sibling count does not match the exact committed tree depth.
    #[error("reputation proof depth {found} does not match expected depth {expected}")]
    ProofDepthMismatch {
        /// Expected sibling count.
        expected: usize,
        /// Observed sibling count.
        found: usize,
    },
    /// An odd-width level did not duplicate the terminal node canonically.
    #[error("reputation proof has a non-canonical odd-leaf sibling")]
    NonCanonicalOddLeafSibling,
    /// Merkle proof geometry did not reduce to one root.
    #[error("reputation proof geometry is invalid")]
    InvalidProofGeometry,
    /// Trust edge references a provider outside the snapshot input set.
    #[error("reputation trust edge references unknown provider `{provider_id}`")]
    TrustEdgeUnknownProvider {
        /// Provider identifier.
        provider_id: String,
    },
    /// Checked fixed-point arithmetic overflowed.
    #[error("reputation arithmetic overflow while computing {context}")]
    ArithmeticOverflow {
        /// Operation context.
        context: &'static str,
    },
    /// A bounded allocation could not be reserved.
    #[error("reputation allocation failed for {context}")]
    AllocationFailed {
        /// Allocation context.
        context: &'static str,
    },
    /// Canonical Norito serialization failed.
    #[error("reputation Norito serialization failed: {0}")]
    Serialization(String),
}
impl From<NoritoError> for ReputationValidationError {
    fn from(error: NoritoError) -> Self {
        Self::Serialization(error.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn metrics() -> ReputationProviderMetricsV1 {
        ReputationProviderMetricsV1 {
            version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
            por_success_bps: 9_800,
            pdp_success_bps: 9_700,
            potr_success_bps: 9_600,
            latency_health_bps: 9_000,
            dispute_rate_bps: 100,
            token_violation_rate_bps: 50,
            repair_breach_rate_bps: 0,
        }
    }
    fn input(provider_id: &str) -> ReputationProviderInputV1 {
        ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: provider_id.to_string(),
            metrics: metrics(),
            reserve_stage: ReputationReserveStageV1::Active,
            previous_score_bps: None,
            active_dispute: false,
            slashing_event: false,
        }
    }
    #[test]
    fn default_weights_match_spec_budget() {
        ReputationWeightsV1::default()
            .validate()
            .expect("valid weights");
    }
    #[test]
    fn scoring_applies_proof_reserve_and_dispute_penalties() {
        let mut input = input("provider-a");
        input.metrics.por_success_bps = 7_900;
        input.reserve_stage = ReputationReserveStageV1::Delinquent;
        input.active_dispute = true;
        let scored = score_provider_reputation(&input, &ReputationWeightsV1::default())
            .expect("scored reputation");
        assert!(scored.score_bps <= 2_000);
        assert!(
            scored
                .degradation_flags
                .contains(&ReputationDegradationFlagV1::ReserveDelinquent)
        );
        assert!(
            scored
                .degradation_flags
                .contains(&ReputationDegradationFlagV1::ProofSuccessBelow80)
        );
        assert!(
            scored
                .degradation_flags
                .contains(&ReputationDegradationFlagV1::ActiveDispute)
        );
    }
    #[test]
    fn provider_validation_rejects_too_many_flags_before_hash_and_order_checks() {
        let mut provider =
            score_provider_reputation(&input("provider-a"), &ReputationWeightsV1::default())
                .expect("valid provider reputation");
        provider.raw_metrics_hash = [0; 32];
        provider.degradation_flags = vec![
            ReputationDegradationFlagV1::LowScore,
            ReputationDegradationFlagV1::SlashingEvent,
            ReputationDegradationFlagV1::ActiveDispute,
            ReputationDegradationFlagV1::ProofSuccessBelow80,
            ReputationDegradationFlagV1::ReserveDefault,
            ReputationDegradationFlagV1::ReserveWarning,
        ];
        assert_eq!(
            provider.validate(),
            Err(ReputationValidationError::TooManyDegradationFlags {
                provider_id: "provider-a".to_owned(),
                count: 6,
                max: MAX_REPUTATION_DEGRADATION_FLAGS,
            })
        );
    }
    #[test]
    fn provider_identifiers_reject_whole_url_dot_segments() {
        for provider_id in ["", ".", "..", " provider-a", "provider/a", "provider-a "] {
            assert_eq!(
                validate_provider_id(provider_id),
                Err(ReputationValidationError::InvalidProviderId),
                "{provider_id:?} must not be a canonical provider identifier"
            );
        }
        validate_provider_id("provider..a").expect("embedded dots remain canonical");
        validate_provider_id(&"p".repeat(256)).expect("maximum-length provider id");
        assert_eq!(
            validate_provider_id(&"p".repeat(257)),
            Err(ReputationValidationError::ProviderIdTooLong { len: 257 })
        );
    }
    #[test]
    fn scoring_smooths_against_previous_score_and_bounds_result() {
        let mut input = input("provider-a");
        input.metrics.dispute_rate_bps = 10_000;
        input.metrics.token_violation_rate_bps = 10_000;
        input.metrics.repair_breach_rate_bps = 10_000;
        input.previous_score_bps = Some(9_900);
        let scored = score_provider_reputation(&input, &ReputationWeightsV1::default())
            .expect("scored reputation");
        assert!(scored.score_bps >= MIN_REPUTATION_SCORE_BPS);
        assert!(scored.score_bps <= MAX_REPUTATION_SCORE_BPS);
        assert!(
            scored.score_bps > MIN_REPUTATION_SCORE_BPS,
            "previous score should smooth the current low score"
        );
    }
    #[test]
    fn snapshot_sorts_providers_and_verifies_merkle_proof() {
        let snapshot = build_reputation_snapshot(
            [0xAB; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[
                input("provider-b"),
                input("provider-a"),
                input("provider-c"),
            ],
            None,
        )
        .expect("snapshot");
        assert_eq!(snapshot.providers[0].provider_id, "provider-a");
        assert_eq!(snapshot.providers[1].provider_id, "provider-b");
        assert_eq!(snapshot.providers[2].provider_id, "provider-c");
        let proof = snapshot.merkle_proof("provider-b").expect("proof");
        let provider = snapshot
            .providers
            .iter()
            .find(|entry| entry.provider_id == "provider-b")
            .expect("provider");
        proof
            .verify(provider, snapshot.merkle_root)
            .expect("valid proof");
    }
    #[test]
    fn snapshot_event_derives_from_valid_snapshot() {
        let snapshot = build_reputation_snapshot(
            [0xAC; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input("provider-b"), input("provider-a")],
            Some([0xAB; 16]),
        )
        .expect("snapshot");
        let event = ReputationSnapshotEventV1::from_snapshot(7, &snapshot).expect("snapshot event");
        assert_eq!(event.version, REPUTATION_SNAPSHOT_EVENT_VERSION_V1);
        assert_eq!(event.sequence, 7);
        assert_eq!(event.snapshot_id, snapshot.snapshot_id);
        assert_eq!(event.generated_at_unix, snapshot.generated_at_unix);
        assert_eq!(event.merkle_root, snapshot.merkle_root);
        assert_eq!(event.provider_count, snapshot.providers.len() as u32);
        assert_eq!(event.previous_snapshot_id, snapshot.previous_snapshot_id);
        event.validate().expect("valid event");
    }
    #[test]
    fn snapshot_previous_id_is_absent_for_genesis_and_nonzero_for_successors() {
        let genesis = build_reputation_snapshot(
            [0xAD; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input("provider-a")],
            None,
        )
        .expect("genesis snapshot");
        assert_eq!(genesis.previous_snapshot_id, None);
        genesis.validate().expect("valid genesis snapshot");
        let predecessor_id = genesis.snapshot_id;
        let successor = build_reputation_snapshot(
            [0xAE; 16],
            1_800_000_001,
            ReputationWeightsV1::default(),
            &[input("provider-a")],
            Some(predecessor_id),
        )
        .expect("successor snapshot");
        assert_eq!(successor.previous_snapshot_id, Some(predecessor_id));
        successor.validate().expect("valid successor snapshot");
    }
    #[test]
    fn snapshot_and_event_reject_zero_previous_id() {
        assert_eq!(
            build_reputation_snapshot(
                [0xAF; 16],
                1_800_000_000,
                ReputationWeightsV1::default(),
                &[input("provider-a")],
                Some([0; 16]),
            ),
            Err(ReputationValidationError::InvalidPreviousSnapshotId)
        );
        let snapshot = build_reputation_snapshot(
            [0xB0; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input("provider-a")],
            None,
        )
        .expect("snapshot");
        let mut event =
            ReputationSnapshotEventV1::from_snapshot(1, &snapshot).expect("snapshot event");
        event.previous_snapshot_id = Some([0; 16]);
        assert_eq!(
            event.validate(),
            Err(ReputationValidationError::InvalidPreviousSnapshotId)
        );
    }
    #[test]
    fn trust_edges_apply_eigentrust_penalty_without_lifting_baseline() {
        let baseline = build_reputation_snapshot(
            [0xB1; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input("provider-a"), input("provider-b")],
            None,
        )
        .expect("baseline snapshot");
        let with_edges = build_reputation_snapshot_with_trust_edges(
            [0xB2; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input("provider-a"), input("provider-b")],
            &[ReputationTrustEdgeV1 {
                version: REPUTATION_TRUST_EDGE_VERSION_V1,
                from_provider_id: "provider-a".to_string(),
                to_provider_id: "provider-b".to_string(),
                trust_bps: 10_000,
            }],
            None,
        )
        .expect("trust-edge snapshot");
        let baseline_a = baseline
            .providers
            .iter()
            .find(|provider| provider.provider_id == "provider-a")
            .expect("baseline provider-a");
        let edged_a = with_edges
            .providers
            .iter()
            .find(|provider| provider.provider_id == "provider-a")
            .expect("trust-edge provider-a");
        let baseline_b = baseline
            .providers
            .iter()
            .find(|provider| provider.provider_id == "provider-b")
            .expect("baseline provider-b");
        let edged_b = with_edges
            .providers
            .iter()
            .find(|provider| provider.provider_id == "provider-b")
            .expect("trust-edge provider-b");
        assert!(
            edged_a.score_bps < baseline_a.score_bps,
            "provider without inbound trust should lose score"
        );
        assert_eq!(
            edged_b.score_bps, baseline_b.score_bps,
            "trust propagation must not lift above direct evidence baseline"
        );
        with_edges.validate().expect("valid trust-edge snapshot");
    }
    #[test]
    fn eigentrust_does_not_reinject_the_published_minimum_during_iteration() {
        let weights = ReputationWeightsV1::default();
        let mut providers = ["provider-a", "provider-b", "provider-c"].map(|provider_id| {
            score_provider_reputation(&input(provider_id), &weights)
                .expect("baseline provider reputation")
        });
        providers[0].score_bps = MIN_REPUTATION_SCORE_BPS;
        providers[0].degradation_flags = vec![ReputationDegradationFlagV1::LowScore];
        for provider in &mut providers[1..] {
            provider.score_bps = MAX_REPUTATION_SCORE_BPS;
            provider.degradation_flags.clear();
        }
        apply_eigentrust_edges(
            &mut providers,
            &[
                ReputationTrustEdgeV1 {
                    version: REPUTATION_TRUST_EDGE_VERSION_V1,
                    from_provider_id: "provider-a".into(),
                    to_provider_id: "provider-b".into(),
                    trust_bps: REPUTATION_BASIS_POINTS,
                },
                ReputationTrustEdgeV1 {
                    version: REPUTATION_TRUST_EDGE_VERSION_V1,
                    from_provider_id: "provider-b".into(),
                    to_provider_id: "provider-c".into(),
                    trust_bps: REPUTATION_BASIS_POINTS,
                },
            ],
            DEFAULT_EIGENTRUST_ALPHA_BPS,
        )
        .expect("apply sparse EigenTrust graph");
        assert_eq!(providers[0].score_bps, MIN_REPUTATION_SCORE_BPS);
        assert_eq!(providers[1].score_bps, 1_548);
        assert_eq!(providers[2].score_bps, MAX_REPUTATION_SCORE_BPS);
    }
    #[test]
    fn trust_edges_reject_unknown_providers() {
        let err = build_reputation_snapshot_with_trust_edges(
            [0xB3; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input("provider-a")],
            &[ReputationTrustEdgeV1 {
                version: REPUTATION_TRUST_EDGE_VERSION_V1,
                from_provider_id: "provider-a".to_string(),
                to_provider_id: "provider-missing".to_string(),
                trust_bps: 10_000,
            }],
            None,
        )
        .expect_err("unknown trust-edge provider should fail");
        assert_eq!(
            err,
            ReputationValidationError::TrustEdgeUnknownProvider {
                provider_id: "provider-missing".to_string()
            }
        );
    }
    #[test]
    fn trust_edges_reject_zero_self_duplicate_and_noncanonical_order() {
        let self_edge = ReputationTrustEdgeV1 {
            version: REPUTATION_TRUST_EDGE_VERSION_V1,
            from_provider_id: "provider-a".to_string(),
            to_provider_id: "provider-a".to_string(),
            trust_bps: 1,
        };
        assert_eq!(
            self_edge.validate(),
            Err(ReputationValidationError::SelfTrustEdge {
                provider_id: "provider-a".to_string()
            })
        );
        let zero_edge = ReputationTrustEdgeV1 {
            version: REPUTATION_TRUST_EDGE_VERSION_V1,
            from_provider_id: "provider-a".to_string(),
            to_provider_id: "provider-b".to_string(),
            trust_bps: 0,
        };
        assert_eq!(
            zero_edge.validate(),
            Err(ReputationValidationError::ZeroTrustEdge)
        );
        let edge = ReputationTrustEdgeV1 {
            version: REPUTATION_TRUST_EDGE_VERSION_V1,
            from_provider_id: "provider-a".to_string(),
            to_provider_id: "provider-b".to_string(),
            trust_bps: 1,
        };
        assert_eq!(
            validate_trust_edges(&[edge.clone(), edge]),
            Err(ReputationValidationError::DuplicateTrustEdge {
                from_provider_id: "provider-a".to_string(),
                to_provider_id: "provider-b".to_string(),
            })
        );
        assert_eq!(
            validate_trust_edges(&[
                ReputationTrustEdgeV1 {
                    version: REPUTATION_TRUST_EDGE_VERSION_V1,
                    from_provider_id: "provider-b".to_string(),
                    to_provider_id: "provider-a".to_string(),
                    trust_bps: 1,
                },
                ReputationTrustEdgeV1 {
                    version: REPUTATION_TRUST_EDGE_VERSION_V1,
                    from_provider_id: "provider-a".to_string(),
                    to_provider_id: "provider-b".to_string(),
                    trust_bps: 1,
                },
            ]),
            Err(ReputationValidationError::TrustEdgesNotSorted)
        );
    }
    #[test]
    fn sparse_trust_graph_handles_thousands_of_providers_without_dense_matrix() {
        const PROVIDER_COUNT: usize = 2_048;
        let provider_ids = (0..PROVIDER_COUNT)
            .map(|index| format!("provider-{index:05}"))
            .collect::<Vec<_>>();
        let inputs = provider_ids
            .iter()
            .map(|provider_id| input(provider_id))
            .collect::<Vec<_>>();
        let edges = provider_ids
            .iter()
            .enumerate()
            .map(|(index, provider_id)| ReputationTrustEdgeV1 {
                version: REPUTATION_TRUST_EDGE_VERSION_V1,
                from_provider_id: provider_id.clone(),
                to_provider_id: provider_ids[(index + 1) % PROVIDER_COUNT].clone(),
                trust_bps: REPUTATION_BASIS_POINTS,
            })
            .collect::<Vec<_>>();
        let snapshot = build_reputation_snapshot_with_trust_edges(
            [0xD1; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &inputs,
            &edges,
            None,
        )
        .expect("bounded sparse graph must score");
        assert_eq!(snapshot.providers.len(), PROVIDER_COUNT);
        assert!(
            snapshot
                .providers
                .windows(2)
                .all(|pair| pair[0].provider_id < pair[1].provider_id)
        );
        assert!(
            snapshot
                .providers
                .windows(2)
                .all(|pair| pair[0].score_bps == pair[1].score_bps)
        );
    }
    #[test]
    fn reputation_input_cardinality_limits_fail_before_scoring_allocations() {
        assert_eq!(
            validate_provider_count(MAX_REPUTATION_PROVIDERS + 1),
            Err(ReputationValidationError::TooManyProviders {
                count: MAX_REPUTATION_PROVIDERS + 1,
                max: MAX_REPUTATION_PROVIDERS,
            })
        );
        assert_eq!(
            validate_trust_edge_count(MAX_REPUTATION_TRUST_EDGES + 1),
            Err(ReputationValidationError::TooManyTrustEdges {
                count: MAX_REPUTATION_TRUST_EDGES + 1,
                max: MAX_REPUTATION_TRUST_EDGES,
            })
        );
    }
    #[test]
    fn merkle_proof_rejects_tampered_provider_record() {
        let snapshot = build_reputation_snapshot(
            [0xCD; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input("provider-a"), input("provider-b")],
            None,
        )
        .expect("snapshot");
        let proof = snapshot.merkle_proof("provider-a").expect("proof");
        let mut provider = snapshot.providers[0].clone();
        provider.score_bps = provider.score_bps.saturating_sub(1);
        assert_eq!(
            proof.verify(&provider, snapshot.merkle_root),
            Err(ReputationValidationError::MerkleRootMismatch)
        );
    }
    #[test]
    fn merkle_proof_binds_leaf_count_and_exact_depth() {
        let snapshot = build_reputation_snapshot(
            [0xCE; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[
                input("provider-a"),
                input("provider-b"),
                input("provider-c"),
            ],
            None,
        )
        .expect("snapshot");
        let provider = &snapshot.providers[1];
        let proof = snapshot.merkle_proof(&provider.provider_id).expect("proof");
        assert_eq!(proof.leaf_count, 3);
        let mut wrong_count = proof.clone();
        wrong_count.leaf_count = 4;
        assert_eq!(
            wrong_count.verify(provider, snapshot.merkle_root),
            Err(ReputationValidationError::MerkleRootMismatch)
        );
        let mut too_deep = proof.clone();
        too_deep.siblings.push([0xAA; 32]);
        assert_eq!(
            too_deep.verify(provider, snapshot.merkle_root),
            Err(ReputationValidationError::ProofDepthMismatch {
                expected: 2,
                found: 3,
            })
        );
        let mut too_shallow = proof;
        too_shallow.siblings.pop();
        assert_eq!(
            too_shallow.verify(provider, snapshot.merkle_root),
            Err(ReputationValidationError::ProofDepthMismatch {
                expected: 2,
                found: 1,
            })
        );
    }
    #[test]
    fn merkle_proof_rejects_bad_index_and_noncanonical_odd_duplication() {
        let snapshot = build_reputation_snapshot(
            [0xCF; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[
                input("provider-a"),
                input("provider-b"),
                input("provider-c"),
            ],
            None,
        )
        .expect("snapshot");
        let provider = &snapshot.providers[2];
        let proof = snapshot.merkle_proof(&provider.provider_id).expect("proof");
        let mut bad_index = proof.clone();
        bad_index.leaf_index = bad_index.leaf_count;
        assert_eq!(
            bad_index.verify(provider, snapshot.merkle_root),
            Err(ReputationValidationError::ProofLeafIndexOutOfRange {
                leaf_index: 3,
                leaf_count: 3,
            })
        );
        let mut bad_duplication = proof;
        bad_duplication.siblings[0][0] ^= 1;
        assert_eq!(
            bad_duplication.verify(provider, snapshot.merkle_root),
            Err(ReputationValidationError::NonCanonicalOddLeafSibling)
        );
    }
    #[test]
    fn one_leaf_merkle_proof_has_zero_depth_and_rejects_extra_sibling() {
        let snapshot = build_reputation_snapshot(
            [0xD0; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input("provider-a")],
            None,
        )
        .expect("snapshot");
        let provider = &snapshot.providers[0];
        let mut proof = snapshot.merkle_proof(&provider.provider_id).expect("proof");
        assert!(proof.siblings.is_empty());
        proof
            .verify(provider, snapshot.merkle_root)
            .expect("zero-depth proof");
        proof.siblings.push([0x11; 32]);
        assert_eq!(
            proof.verify(provider, snapshot.merkle_root),
            Err(ReputationValidationError::ProofDepthMismatch {
                expected: 0,
                found: 1,
            })
        );
    }
    #[test]
    fn snapshot_rejects_zero_time_self_link_and_parameter_drift() {
        assert_eq!(
            build_reputation_snapshot(
                [0xD2; 16],
                0,
                ReputationWeightsV1::default(),
                &[input("provider-a")],
                None,
            ),
            Err(ReputationValidationError::InvalidGeneratedAt)
        );
        let mut snapshot = build_reputation_snapshot(
            [0xD3; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input("provider-a")],
            None,
        )
        .expect("snapshot");
        snapshot.previous_snapshot_id = Some(snapshot.snapshot_id);
        assert_eq!(
            snapshot.validate(),
            Err(ReputationValidationError::SelfReferentialSnapshot)
        );
        snapshot.previous_snapshot_id = None;
        snapshot.alpha_bps -= 1;
        assert_eq!(
            snapshot.validate(),
            Err(ReputationValidationError::NonCanonicalAlpha {
                found: DEFAULT_EIGENTRUST_ALPHA_BPS - 1,
            })
        );
        snapshot.alpha_bps = DEFAULT_EIGENTRUST_ALPHA_BPS;
        snapshot.current_score_weight_bps -= 1;
        assert_eq!(
            snapshot.validate(),
            Err(ReputationValidationError::NonCanonicalSmoothingWeight {
                found: DEFAULT_CURRENT_SCORE_WEIGHT_BPS - 1,
            })
        );
    }
    #[test]
    fn snapshot_validation_rejects_duplicate_provider_ids() {
        let first =
            score_provider_reputation(&input("provider-a"), &ReputationWeightsV1::default())
                .expect("first");
        let second =
            score_provider_reputation(&input("provider-a"), &ReputationWeightsV1::default())
                .expect("second");
        assert!(matches!(
            ReputationSnapshotV1 {
                version: REPUTATION_SNAPSHOT_VERSION_V1,
                snapshot_id: [1; 16],
                generated_at_unix: 1,
                alpha_bps: DEFAULT_EIGENTRUST_ALPHA_BPS,
                current_score_weight_bps: DEFAULT_CURRENT_SCORE_WEIGHT_BPS,
                weights: ReputationWeightsV1::default(),
                providers: vec![first, second],
                merkle_root: [0; 32],
                previous_snapshot_id: None,
            }
            .validate(),
            Err(ReputationValidationError::DuplicateProviderId { .. })
        ));
    }
    #[test]
    fn snapshot_validation_rejects_empty_provider_set() {
        assert_eq!(
            ReputationSnapshotV1 {
                version: REPUTATION_SNAPSHOT_VERSION_V1,
                snapshot_id: [1; 16],
                generated_at_unix: 1,
                alpha_bps: DEFAULT_EIGENTRUST_ALPHA_BPS,
                current_score_weight_bps: DEFAULT_CURRENT_SCORE_WEIGHT_BPS,
                weights: ReputationWeightsV1::default(),
                providers: Vec::new(),
                merkle_root: [0; 32],
                previous_snapshot_id: None,
            }
            .validate(),
            Err(ReputationValidationError::EmptyProviderSet)
        );
    }
}
