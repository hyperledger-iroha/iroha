//! Hijiri reputation system data structures.
//!
//! This module defines the portable data types used by the Hijiri peer and account reputation
//! pipelines. The initial focus is on account-level positive attestations: observer registries
//! describe which external parties may issue them, and incentives determine how much an attestation
//! can boost the `S_attestation` component of the global risk score as well as how registries are
//! compensated.
use crate::{
    account::AccountId,
    metadata::Metadata,
    name::Name,
    parameter::{CustomParameter, CustomParameterId},
};
use derive_more::{AsRef, Deref};
use iroha_crypto::Hash;
use iroha_primitives::json::Json;
use iroha_primitives::numeric::{Numeric, NumericOperationError, Quantity};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{convert::TryFrom, fmt};
use thiserror::Error;

/// Maximum number of fee bands accepted by one Hijiri policy.
pub const MAX_FEE_MULTIPLIER_BANDS: usize = 256;
/// Maximum JSON or canonical Norito size of the global on-chain Hijiri parameter.
pub const MAX_HIJIRI_PARAMETERS_ENCODED_BYTES: usize = 64 * 1024;
/// Maximum JSON or canonical Norito size of one on-chain account-risk record.
pub const MAX_HIJIRI_ACCOUNT_RISK_ENCODED_BYTES: usize = 4 * 1024;
/// Maximum number of redacted fields committed by one evidence bundle.
pub const MAX_EVIDENCE_REDACTED_FIELDS: usize = 256;
/// Maximum number of exact key segments in one evidence field path.
pub const MAX_EVIDENCE_PATH_SEGMENTS: usize = 32;
/// Maximum UTF-8 byte length of one evidence field-path segment.
pub const MAX_EVIDENCE_PATH_SEGMENT_BYTES: usize = 128;
/// Maximum aggregate UTF-8 byte length of one evidence field path.
pub const MAX_EVIDENCE_PATH_BYTES: usize = 4_096;
/// Schema version of the first on-chain Hijiri parameter.
pub const HIJIRI_PARAMETERS_VERSION_V1: u16 = 1;
/// Schema version of the first on-chain Hijiri account-risk record.
pub const HIJIRI_ACCOUNT_RISK_VERSION_V1: u16 = 1;
/// Reserved custom-parameter prefix for per-account Hijiri risk records.
pub const HIJIRI_ACCOUNT_RISK_PARAMETER_PREFIX_V1: &str = "iroha:hijiri_account_risk_v1:";
const HIJIRI_PARAMETERS_DIGEST_DOMAIN_V1: &[u8] = b"iroha:hijiri:parameters:v1\0";
const HIJIRI_ACCOUNT_RISK_ID_DOMAIN_V1: &[u8] = b"iroha:hijiri:account-risk-id:v1\0";
const HIJIRI_ACCOUNT_RISK_DIGEST_DOMAIN_V1: &[u8] = b"iroha:hijiri:account-risk:v1\0";
const HIJIRI_FEE_QUOTE_DIGEST_DOMAIN_V1: &[u8] = b"iroha:hijiri:fee-quote:v1\0";
/// Unsigned Q16.16 fixed-point representation backed by `u32`.
///
/// Hijiri scoring relies on Q16.16 arithmetic. This lightweight wrapper keeps
/// the encoding explicit while providing saturating helpers for common math.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    AsRef,
    Deref,
    Default,
    IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[repr(transparent)]
pub struct Q16(pub u32);
impl Q16 {
    /// Zero in Q16.16.
    pub const ZERO: Self = Self(0);
    /// One in Q16.16.
    pub const ONE: Self = Self(0x0001_0000);
    /// Construct a `Q16` from an integer/fraction pair.
    pub const fn from_parts(integer: u16, fraction: u16) -> Self {
        Self(((integer as u32) << 16) | fraction as u32)
    }
    /// Construct a `Q16` directly from the underlying raw value.
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
    /// Return the raw underlying value.
    pub const fn raw(self) -> u32 {
        self.0
    }
    /// Saturating addition.
    #[must_use]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }
    /// Saturating subtraction.
    #[must_use]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
    /// Saturating multiplication by an integer factor.
    #[must_use]
    pub fn saturating_mul(self, factor: u32) -> Self {
        let product = u128::from(self.0) * u128::from(factor);
        let clamped = u32::try_from(product).unwrap_or(u32::MAX);
        Self(clamped)
    }
    /// Clamp `self` so it never exceeds `cap`.
    #[must_use]
    pub const fn min(self, cap: Self) -> Self {
        if self.0 > cap.0 { cap } else { self }
    }
    /// Saturating multiplication by another Q16 value with rounding half-up.
    #[must_use]
    pub fn saturating_mul_q16(self, rhs: Self) -> Self {
        let product = u128::from(self.0) * u128::from(rhs.0);
        let with_round = product + u128::from(0x8000u32);
        let shifted = with_round >> 16;
        let clamped = u32::try_from(shifted).unwrap_or(u32::MAX);
        Self(clamped)
    }

    /// Multiply an integer by this Q16 value, rounding any fractional minor unit upward.
    ///
    /// This is the canonical conversion used for monetary fee multipliers: a positive
    /// fractional penalty can never disappear because the fee asset has a coarse scale.
    #[must_use]
    pub fn checked_mul_u64_ceil(self, value: u64) -> Option<u64> {
        let product = u128::from(value).checked_mul(u128::from(self.0))?;
        let rounded = product.checked_add(u128::from(u16::MAX))? >> 16;
        u64::try_from(rounded).ok()
    }
}
impl fmt::Display for Q16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let integer = self.0 >> 16;
        let fraction = u64::from(self.0 & 0xFFFF);
        let decimal_fraction = (fraction * 100_000 + 0x8000) >> 16;
        write!(f, "{integer}.{decimal_fraction:05}")
    }
}
/// Identifier of an observer profile approved by governance.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, AsRef, Deref, derive_more::From,
)]
pub struct ObserverProfileId(Name);
impl ObserverProfileId {
    /// Create a new profile identifier.
    pub fn new(name: Name) -> Self {
        Self(name)
    }
    /// Access the inner name.
    pub fn as_name(&self) -> &Name {
        &self.0
    }
}
/// Capability advertised by an observer profile.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum DelegatedAttestationClass {
    /// Observer may issue positive attestations that boost `S_attestation`.
    Positive(PositiveAttestationIncentive),
    /// Observer may issue negative attestations that apply penalties.
    Negative {
        /// Penalty weight applied per attestation.
        penalty_q16: Q16,
        /// Optional maximum cumulative penalty to enforce a floor.
        max_penalty_q16: Option<Q16>,
    },
}
/// Governance-approved observer profile describing capabilities and incentives.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ObserverProfile {
    /// Stable profile identifier.
    pub id: ObserverProfileId,
    /// Monotonically increasing version.
    pub version: u32,
    /// Human-readable display name surfaced in dashboards.
    pub display_name: String,
    /// Jurisdictional tag (e.g., ISO country code or regulatory perimeter).
    pub jurisdiction: String,
    /// Capabilities this profile is authorised to issue.
    pub capabilities: Vec<DelegatedAttestationClass>,
    /// Additional metadata for governance/observability.
    pub metadata: Metadata,
}
impl ObserverProfile {
    /// Helper to locate the positive attestation incentive for this profile.
    pub fn positive_incentive(&self) -> Option<&PositiveAttestationIncentive> {
        self.capabilities.iter().find_map(|capability| {
            if let DelegatedAttestationClass::Positive(incentive) = capability {
                Some(incentive)
            } else {
                None
            }
        })
    }
}
/// Registry record linking an observer account to a profile version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ObserverRegistryEntry {
    /// Observer account authorised to emit receipts.
    pub observer: AccountId,
    /// Profile identifier referenced by this observer.
    pub profile_id: ObserverProfileId,
    /// Profile version pinned for deterministic validation.
    pub profile_version: u32,
    /// Maximum positive attestations allowed per round.
    pub positive_quota_per_round: u32,
    /// Optional metadata surfaced to operators.
    pub metadata: Metadata,
}
/// Schedule describing how registries are compensated for positive attestations.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct RegistryCreditSchedule {
    /// Account receiving the reward.
    pub reward_account: AccountId,
    /// Nominal reward paid per positive attestation.
    pub reward_per_attestation: Quantity,
    /// Settlement stride expressed in Hijiri rounds.
    pub settlement_period_rounds: u32,
}
impl RegistryCreditSchedule {
    /// Compute the total credit owed for `attestations` positive receipts.
    ///
    /// # Errors
    /// Returns [`NumericOperationError::MantissaOverflow`] if the exact product
    /// falls outside the bounded quantity domain.
    pub fn total_reward(&self, attestations: u32) -> Result<Quantity, NumericOperationError> {
        self.reward_per_attestation
            .try_mul_decimal(&Numeric::from(attestations))
    }
}
/// Positive attestation incentive applied to the subject account and registry.
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
pub struct PositiveAttestationIncentive {
    /// Score boost applied per positive attestation.
    pub score_boost_per_attestation: Q16,
    /// Maximum cumulative boost contribution from this profile.
    pub max_score_boost: Q16,
    /// Registry reward schedule credited for positive attestations.
    pub registry_credit: RegistryCreditSchedule,
}
#[derive(Decode)]
struct PositiveAttestationIncentiveWire {
    score_boost_per_attestation: Q16,
    max_score_boost: Q16,
    registry_credit: RegistryCreditSchedule,
}
impl TryFrom<PositiveAttestationIncentiveWire> for PositiveAttestationIncentive {
    type Error = PositiveAttestationError;

    fn try_from(wire: PositiveAttestationIncentiveWire) -> Result<Self, Self::Error> {
        Self::new(
            wire.score_boost_per_attestation,
            wire.max_score_boost,
            wire.registry_credit,
        )
    }
}
impl<'de> norito::core::NoritoDeserialize<'de> for PositiveAttestationIncentive {
    fn schema_hash() -> [u8; 16] {
        <Self as norito::core::NoritoSerialize>::schema_hash()
    }

    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("positive attestation incentive must be valid")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let wire =
            <PositiveAttestationIncentiveWire as norito::core::NoritoDeserialize>::try_deserialize(
                archived.cast(),
            )?;
        Self::try_from(wire).map_err(|error| norito::core::Error::Message(error.to_string()))
    }
}
impl PositiveAttestationIncentive {
    /// Construct a new incentive after validating invariants.
    ///
    /// # Errors
    /// Returns [`PositiveAttestationError`] when the per-attestation boost is zero, exceeds
    /// the configured cap, or when the registry reward schedule includes zero-valued parameters.
    pub fn new(
        score_boost_per_attestation: Q16,
        max_score_boost: Q16,
        registry_credit: RegistryCreditSchedule,
    ) -> Result<Self, PositiveAttestationError> {
        if score_boost_per_attestation.0 == 0 {
            return Err(PositiveAttestationError::ZeroBoost);
        }
        if score_boost_per_attestation > max_score_boost {
            return Err(PositiveAttestationError::BoostExceedsCap {
                per_receipt: score_boost_per_attestation,
                cap: max_score_boost,
            });
        }
        if registry_credit.reward_per_attestation.is_zero() {
            return Err(PositiveAttestationError::ZeroReward);
        }
        if registry_credit.settlement_period_rounds == 0 {
            return Err(PositiveAttestationError::ZeroSettlementStride);
        }
        Ok(Self {
            score_boost_per_attestation,
            max_score_boost,
            registry_credit,
        })
    }
    /// Apply the incentive to the contribution already accumulated for this profile.
    ///
    /// Positive receipts never reduce an existing contribution, including malformed legacy
    /// values that already exceed the configured cap.
    pub fn apply_boost(&self, current_profile_boost: Q16, attestations: u32) -> Q16 {
        if attestations == 0 || current_profile_boost >= self.max_score_boost {
            return current_profile_boost;
        }
        let addition = self
            .score_boost_per_attestation
            .saturating_mul(attestations);
        let boosted = current_profile_boost.saturating_add(addition);
        boosted.min(self.max_score_boost)
    }
}
/// Validation errors encountered when constructing an incentive.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PositiveAttestationError {
    /// Per-attestation boost is zero.
    #[error("score boost per attestation must be non-zero")]
    ZeroBoost,
    /// Per-attestation boost exceeds the configured cap.
    #[error("score boost per attestation {per_receipt:?} exceeds cap {cap:?}")]
    BoostExceedsCap {
        /// Boost applied per receipt.
        per_receipt: Q16,
        /// Configured maximum boost.
        cap: Q16,
    },
    /// Registry reward equals zero.
    #[error("registry reward per attestation must be non-zero")]
    ZeroReward,
    /// Settlement period cannot be zero.
    #[error("settlement period must be at least one Hijiri round")]
    ZeroSettlementStride,
}
/// Hashing algorithm identifier for privacy-preserving evidence commitments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum EvidenceHashAlgorithm {
    /// Poseidon2 permutation over the Goldilocks field with 32-byte output.
    Poseidon2Goldilocks,
}
impl EvidenceHashAlgorithm {
    /// Output length in bytes for the selected algorithm.
    pub const fn output_len(self) -> usize {
        match self {
            Self::Poseidon2Goldilocks => 32,
        }
    }
}
/// Commitment to a redacted evidence field.
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
pub struct EvidenceFieldCommitment {
    /// Canonical JSON pointer-like path describing the redacted field.
    pub field_path: Vec<String>,
    /// Domain-separated hash over the field payload and blinding salt.
    pub commitment: [u8; 32],
    /// Blinding salt used for the commitment (Poseidon input).
    pub blinding_salt: [u8; 32],
    /// Optional salted hash of the raw payload for replay protection.
    pub value_digest: Option<[u8; 32]>,
}
#[derive(Decode)]
struct EvidenceFieldCommitmentWire {
    field_path: Vec<String>,
    commitment: [u8; 32],
    blinding_salt: [u8; 32],
    value_digest: Option<[u8; 32]>,
}
impl TryFrom<EvidenceFieldCommitmentWire> for EvidenceFieldCommitment {
    type Error = EvidenceHashError;

    fn try_from(wire: EvidenceFieldCommitmentWire) -> Result<Self, Self::Error> {
        Self::new(
            wire.field_path,
            wire.commitment,
            wire.blinding_salt,
            wire.value_digest,
        )
    }
}
impl<'de> norito::core::NoritoDeserialize<'de> for EvidenceFieldCommitment {
    fn schema_hash() -> [u8; 16] {
        <Self as norito::core::NoritoSerialize>::schema_hash()
    }

    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("evidence field commitment path must be valid")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let wire =
            <EvidenceFieldCommitmentWire as norito::core::NoritoDeserialize>::try_deserialize(
                archived.cast(),
            )?;
        Self::try_from(wire).map_err(|error| norito::core::Error::Message(error.to_string()))
    }
}
impl EvidenceFieldCommitment {
    /// Construct a new field commitment ensuring the path is well-formed.
    ///
    /// # Errors
    /// Returns [`EvidenceHashError`] when the supplied field path is empty or contains empty
    /// segments.
    pub fn new<P>(
        field_path: P,
        commitment: [u8; 32],
        blinding_salt: [u8; 32],
        value_digest: Option<[u8; 32]>,
    ) -> Result<Self, EvidenceHashError>
    where
        P: Into<Vec<String>>,
    {
        let path = field_path.into();
        validate_evidence_field_path(&path)?;
        Ok(Self {
            field_path: path,
            commitment,
            blinding_salt,
            value_digest,
        })
    }
}
/// Envelope containing the commitments for a redacted evidence payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
pub struct EvidenceHashBundle {
    /// Hashing algorithm applied to each field commitment.
    pub algorithm: EvidenceHashAlgorithm,
    /// Domain-separated hash of the full payload (pre-redaction).
    pub payload_commitment: [u8; 32],
    /// Per-field commitments for every redacted value.
    pub redacted_fields: Vec<EvidenceFieldCommitment>,
}
#[derive(Decode)]
struct EvidenceHashBundleWire {
    algorithm: EvidenceHashAlgorithm,
    payload_commitment: [u8; 32],
    redacted_fields: Vec<EvidenceFieldCommitment>,
}
impl TryFrom<EvidenceHashBundleWire> for EvidenceHashBundle {
    type Error = EvidenceHashError;

    fn try_from(wire: EvidenceHashBundleWire) -> Result<Self, Self::Error> {
        validate_canonical_evidence_fields(&wire.redacted_fields)?;
        Ok(Self {
            algorithm: wire.algorithm,
            payload_commitment: wire.payload_commitment,
            redacted_fields: wire.redacted_fields,
        })
    }
}
impl<'de> norito::core::NoritoDeserialize<'de> for EvidenceHashBundle {
    fn schema_hash() -> [u8; 16] {
        <Self as norito::core::NoritoSerialize>::schema_hash()
    }

    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("evidence hash bundle must be valid")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let wire = <EvidenceHashBundleWire as norito::core::NoritoDeserialize>::try_deserialize(
            archived.cast(),
        )?;
        Self::try_from(wire).map_err(|error| norito::core::Error::Message(error.to_string()))
    }
}
impl EvidenceHashBundle {
    /// Construct a bundle while checking invariants.
    ///
    /// # Errors
    /// Returns [`EvidenceHashError`] when a field path is malformed or duplicate paths are
    /// detected in the bundle.
    pub fn new(
        algorithm: EvidenceHashAlgorithm,
        payload_commitment: [u8; 32],
        mut redacted_fields: Vec<EvidenceFieldCommitment>,
    ) -> Result<Self, EvidenceHashError> {
        if redacted_fields.len() > MAX_EVIDENCE_REDACTED_FIELDS {
            return Err(EvidenceHashError::TooManyRedactedFields);
        }
        // Ensure field paths are unique and deterministically ordered.
        redacted_fields.sort_by(|a, b| a.field_path.cmp(&b.field_path));
        validate_canonical_evidence_fields(&redacted_fields)?;
        Ok(Self {
            algorithm,
            payload_commitment,
            redacted_fields,
        })
    }
}
fn validate_canonical_evidence_fields(
    redacted_fields: &[EvidenceFieldCommitment],
) -> Result<(), EvidenceHashError> {
    if redacted_fields.len() > MAX_EVIDENCE_REDACTED_FIELDS {
        return Err(EvidenceHashError::TooManyRedactedFields);
    }
    let mut last_path: Option<&[String]> = None;
    for field in redacted_fields {
        validate_evidence_field_path(&field.field_path)?;
        if let Some(previous) = last_path {
            match previous.cmp(&field.field_path) {
                std::cmp::Ordering::Equal => return Err(EvidenceHashError::DuplicateFieldPath),
                std::cmp::Ordering::Greater => {
                    return Err(EvidenceHashError::NonCanonicalFieldOrder);
                }
                std::cmp::Ordering::Less => {}
            }
        }
        last_path = Some(&field.field_path);
    }
    Ok(())
}
fn validate_evidence_field_path(field_path: &[String]) -> Result<(), EvidenceHashError> {
    if field_path.is_empty() {
        return Err(EvidenceHashError::EmptyFieldPath);
    }
    if field_path.len() > MAX_EVIDENCE_PATH_SEGMENTS {
        return Err(EvidenceHashError::TooManyPathSegments);
    }
    if field_path.iter().any(String::is_empty) {
        return Err(EvidenceHashError::EmptyFieldSegment);
    }
    if field_path
        .iter()
        .any(|segment| segment.len() > MAX_EVIDENCE_PATH_SEGMENT_BYTES)
    {
        return Err(EvidenceHashError::PathSegmentTooLong);
    }
    let total_bytes = field_path
        .iter()
        .try_fold(0_usize, |total, segment| total.checked_add(segment.len()));
    if total_bytes.is_none_or(|total| total > MAX_EVIDENCE_PATH_BYTES) {
        return Err(EvidenceHashError::PathTooLong);
    }
    Ok(())
}
/// Validation errors for evidence hashing bundles.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum EvidenceHashError {
    /// Field commitment path cannot be empty.
    #[error("evidence field path must contain at least one segment")]
    EmptyFieldPath,
    /// Individual path segments must be non-empty.
    #[error("evidence field path segments must be non-empty strings")]
    EmptyFieldSegment,
    /// Duplicate field path detected in the bundle.
    #[error("duplicate evidence field path in commitment bundle")]
    DuplicateFieldPath,
    /// Encoded fields were not already in canonical lexicographic path order.
    #[error("evidence fields are not in canonical path order")]
    NonCanonicalFieldOrder,
    /// One bundle exceeded the consensus-safe field count.
    #[error("evidence bundle contains too many redacted fields")]
    TooManyRedactedFields,
    /// One field path exceeded the consensus-safe segment count.
    #[error("evidence field path contains too many segments")]
    TooManyPathSegments,
    /// One exact UTF-8 path segment exceeded its byte limit.
    #[error("evidence field path segment is too long")]
    PathSegmentTooLong,
    /// One complete field path exceeded its aggregate byte limit.
    #[error("evidence field path is too long")]
    PathTooLong,
}
/// Band describing the fee multiplier applied to a given Hijiri risk range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct FeeMultiplierBand {
    /// Inclusive upper bound for the risk score covered by this band.
    pub max_risk: Q16,
    /// Fee multiplier used when the band matches.
    pub multiplier: Q16,
}
#[derive(Decode)]
struct FeeMultiplierBandWire {
    max_risk: Q16,
    multiplier: Q16,
}
impl TryFrom<FeeMultiplierBandWire> for FeeMultiplierBand {
    type Error = FeePolicyError;

    fn try_from(wire: FeeMultiplierBandWire) -> Result<Self, Self::Error> {
        Self::new(wire.max_risk, wire.multiplier)
    }
}
impl<'de> norito::core::NoritoDeserialize<'de> for FeeMultiplierBand {
    fn schema_hash() -> [u8; 16] {
        <Self as norito::core::NoritoSerialize>::schema_hash()
    }

    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("fee multiplier band must be valid")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let wire = <FeeMultiplierBandWire as norito::core::NoritoDeserialize>::try_deserialize(
            archived.cast(),
        )?;
        Self::try_from(wire).map_err(|error| norito::core::Error::Message(error.to_string()))
    }
}
impl FeeMultiplierBand {
    /// Create a band while validating bounds and multiplier.
    ///
    /// # Errors
    /// Returns [`FeePolicyError`] when the upper bound is zero or the multiplier falls below 1.
    pub fn new(max_risk: Q16, multiplier: Q16) -> Result<Self, FeePolicyError> {
        let band = Self {
            max_risk,
            multiplier,
        };
        band.validate()?;
        Ok(band)
    }
    fn validate(&self) -> Result<(), FeePolicyError> {
        if self.max_risk.0 == 0 {
            return Err(FeePolicyError::ZeroUpperBound);
        }
        if self.multiplier.0 < Q16::ONE.0 {
            return Err(FeePolicyError::MultiplierBelowOne);
        }
        Ok(())
    }
}
/// Deterministic fee policy mapping risk scores to fee multipliers.
#[derive(Clone, Debug, PartialEq, Eq, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct HijiriFeePolicy {
    /// Ordered bands covering `[0, 1]`.
    pub bands: Vec<FeeMultiplierBand>,
    /// Maximum multiplier allowed by policy.
    pub penalty_cap: Q16,
}
#[derive(Decode)]
struct HijiriFeePolicyWire {
    bands: Vec<FeeMultiplierBand>,
    penalty_cap: Q16,
}
impl TryFrom<HijiriFeePolicyWire> for HijiriFeePolicy {
    type Error = FeePolicyError;

    fn try_from(wire: HijiriFeePolicyWire) -> Result<Self, Self::Error> {
        Self::new(wire.bands, wire.penalty_cap)
    }
}
impl<'de> norito::core::NoritoDeserialize<'de> for HijiriFeePolicy {
    fn schema_hash() -> [u8; 16] {
        <Self as norito::core::NoritoSerialize>::schema_hash()
    }

    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("Hijiri fee policy must be valid")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let wire = <HijiriFeePolicyWire as norito::core::NoritoDeserialize>::try_deserialize(
            archived.cast(),
        )?;
        Self::try_from(wire).map_err(|error| norito::core::Error::Message(error.to_string()))
    }
}
impl HijiriFeePolicy {
    /// Construct a new policy, sorting bands and ensuring they cover the full range.
    ///
    /// # Errors
    /// Returns [`FeePolicyError`] when the supplied bands are empty, contain invalid or duplicate
    /// bounds, use a multiplier below one, or fail to cover the entire `[0, 1]` interval.
    /// Multipliers above `penalty_cap` are accepted and clamped when the policy is evaluated.
    pub fn new(
        mut bands: Vec<FeeMultiplierBand>,
        penalty_cap: Q16,
    ) -> Result<Self, FeePolicyError> {
        bands.sort_by(|a, b| a.max_risk.cmp(&b.max_risk));
        let policy = Self { bands, penalty_cap };
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<(), FeePolicyError> {
        if self.bands.is_empty() {
            return Err(FeePolicyError::NoBands);
        }
        if self.bands.len() > MAX_FEE_MULTIPLIER_BANDS {
            return Err(FeePolicyError::TooManyBands);
        }
        if self.penalty_cap.0 < Q16::ONE.0 {
            return Err(FeePolicyError::PenaltyCapBelowOne);
        }
        let mut prev = Q16::ZERO;
        for band in &self.bands {
            band.validate()?;
            if band.max_risk.0 <= prev.0 {
                return Err(FeePolicyError::DescendingBounds);
            }
            prev = band.max_risk;
        }
        if self
            .bands
            .last()
            .is_none_or(|band| band.max_risk.0 != Q16::ONE.0)
        {
            return Err(FeePolicyError::TerminalBandBelowOne);
        }
        Ok(())
    }
    /// Return the multiplier for a given risk score.
    pub fn multiplier_for(&self, risk: Q16) -> Q16 {
        // The public fields are retained for data-model compatibility, so evaluation of malformed
        // in-memory values fails closed even though constructors and Norito decoding reject them.
        let penalty_cap = if self.penalty_cap < Q16::ONE {
            Q16::ONE
        } else {
            self.penalty_cap
        };
        for band in &self.bands {
            if risk.0 <= band.max_risk.0 {
                let multiplier = if band.multiplier < Q16::ONE {
                    Q16::ONE
                } else {
                    band.multiplier
                };
                return multiplier.min(penalty_cap);
            }
        }
        penalty_cap
    }
    /// Apply the policy to a base fee expressed in Q16.
    pub fn apply(&self, base_fee: Q16, risk: Q16) -> Q16 {
        base_fee.saturating_mul_q16(self.multiplier_for(risk))
    }
}
/// Validation errors produced when building a fee policy.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum FeePolicyError {
    /// Policy cannot be empty.
    #[error("fee policy must contain at least one band")]
    NoBands,
    /// Policy exceeded the bounded band count.
    #[error("fee policy contains too many bands")]
    TooManyBands,
    /// Band bounds must be strictly increasing.
    #[error("fee policy bands must have strictly increasing risk bounds")]
    DescendingBounds,
    /// Band multiplier must be ≥ 1.0.
    #[error("fee multiplier must be greater than or equal to 1.0")]
    MultiplierBelowOne,
    /// Multiplier exceeds configured cap.
    #[error("fee multiplier exceeds configured penalty cap")]
    MultiplierExceedsCap,
    /// Penalty cap must be ≥ 1.0.
    #[error("penalty cap must be greater than or equal to 1.0")]
    PenaltyCapBelowOne,
    /// Final band must reach 1.0 risk.
    #[error("terminal fee band must cover risk score 1.0")]
    TerminalBandBelowOne,
    /// Upper bound cannot be zero.
    #[error("band upper bound must be non-zero")]
    ZeroUpperBound,
}

/// Versioned, bounded chain parameter that owns the global Hijiri fee policy.
///
/// Account-specific risks use independent [`HijiriAccountRiskV1`] custom parameters. Keeping the
/// global policy small prevents every fee-bearing transaction and parameter-change event from
/// decoding or copying a whole-ledger account table. Both record types form digest-linked revision
/// sequences and there is no second node-local Hijiri policy authority.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct HijiriParametersV1 {
    /// Schema version. Must equal [`HIJIRI_PARAMETERS_VERSION_V1`].
    pub version: u16,
    /// Strictly increasing registry revision, beginning at one.
    pub revision: u64,
    /// Digest of the immediately preceding parameter revision.
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::fixed_bytes::option")
    )]
    pub previous_digest: Option<[u8; 32]>,
    /// Fee multiplier policy applied by validation-fee admission.
    pub fee_policy: HijiriFeePolicy,
    /// Risk assigned to an account without an explicit [`HijiriAccountRiskV1`] record.
    pub default_account_risk: Q16,
}
impl HijiriParametersV1 {
    /// Reserved custom-parameter identifier for Hijiri V1.
    pub const PARAMETER_ID_STR: &'static str = "iroha:hijiri_parameters_v1";

    /// Build the neutral Hijiri snapshot seeded by first-release genesis manifests.
    ///
    /// The single unit-multiplier band preserves the base validation fee for every
    /// account while making Hijiri state and its signed quote binding explicit.
    #[must_use]
    pub fn first_release_genesis() -> Self {
        let fee_policy = HijiriFeePolicy::new(
            vec![
                FeeMultiplierBand::new(Q16::ONE, Q16::ONE)
                    .expect("the first-release Hijiri band is valid"),
            ],
            Q16::ONE,
        )
        .expect("the first-release Hijiri fee policy is valid");
        Self::try_new(1, None, fee_policy, Q16::ZERO)
            .expect("the first-release Hijiri genesis snapshot is valid")
    }

    /// Build a canonical global Hijiri parameter.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError`] for invalid revision lineage, fee policy, payload size,
    /// or a default risk value above one.
    pub fn try_new(
        revision: u64,
        previous_digest: Option<[u8; 32]>,
        fee_policy: HijiriFeePolicy,
        default_account_risk: Q16,
    ) -> Result<Self, HijiriParametersError> {
        let parameters = Self {
            version: HIJIRI_PARAMETERS_VERSION_V1,
            revision,
            previous_digest,
            fee_policy,
            default_account_risk,
        };
        parameters.validate()?;
        Ok(parameters)
    }

    /// Construct the chain-level custom parameter identifier.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid Hijiri parameter identifier")
    }

    /// Convert this snapshot into the custom parameter accepted by `SetParameter`.
    #[must_use]
    pub fn into_custom_parameter(self) -> CustomParameter {
        CustomParameter::new(Self::parameter_id(), Json::new(self))
    }

    /// Strictly decode a matching custom parameter.
    ///
    /// Non-matching identifiers return `Ok(None)`.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError`] for malformed JSON or invalid canonical contents.
    pub fn from_custom_parameter(
        custom: &CustomParameter,
    ) -> Result<Option<Self>, HijiriParametersError> {
        if custom.id() != &Self::parameter_id() {
            return Ok(None);
        }
        if custom.payload().get().len() > MAX_HIJIRI_PARAMETERS_ENCODED_BYTES {
            return Err(HijiriParametersError::EncodedPayloadTooLarge);
        }
        let parameters = custom
            .payload()
            .try_into_any_norito::<Self>()
            .map_err(|_| HijiriParametersError::MalformedPayload)?;
        parameters.validate()?;
        Ok(Some(parameters))
    }

    /// Validate canonical bounds and self-contained revision shape.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError`] when any invariant is violated.
    pub fn validate(&self) -> Result<(), HijiriParametersError> {
        if self.version != HIJIRI_PARAMETERS_VERSION_V1 {
            return Err(HijiriParametersError::UnsupportedVersion(self.version));
        }
        if self.revision == 0 {
            return Err(HijiriParametersError::ZeroRevision);
        }
        match (self.revision, self.previous_digest) {
            (1, Some(_)) => return Err(HijiriParametersError::InitialHasPreviousDigest),
            (2.., None) => return Err(HijiriParametersError::SuccessorMissingPreviousDigest),
            _ => {}
        }
        self.fee_policy
            .validate()
            .map_err(HijiriParametersError::InvalidFeePolicy)?;
        if self.default_account_risk > Q16::ONE {
            return Err(HijiriParametersError::RiskAboveOne);
        }
        let encoded =
            norito::encode_canonical(self).map_err(|_| HijiriParametersError::CanonicalEncoding)?;
        if encoded.len() > MAX_HIJIRI_PARAMETERS_ENCODED_BYTES {
            return Err(HijiriParametersError::EncodedPayloadTooLarge);
        }
        Ok(())
    }

    /// Validate an exact initial install or strict digest-linked successor.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError`] for skipped/replayed revisions or a predecessor mismatch.
    pub fn validate_transition(
        previous: Option<&Self>,
        next: &Self,
    ) -> Result<(), HijiriParametersError> {
        next.validate()?;
        let Some(previous) = previous else {
            if next.revision != 1 {
                return Err(HijiriParametersError::FirstRevisionNotOne(next.revision));
            }
            return Ok(());
        };
        previous.validate()?;
        let expected_revision = previous
            .revision
            .checked_add(1)
            .ok_or(HijiriParametersError::RevisionOverflow)?;
        if next.revision != expected_revision {
            return Err(HijiriParametersError::UnexpectedRevision {
                expected: expected_revision,
                found: next.revision,
            });
        }
        if next.previous_digest != Some(previous.digest()?) {
            return Err(HijiriParametersError::PreviousDigestMismatch);
        }
        Ok(())
    }

    /// Return the domain-separated digest used to link a successor revision.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError::CanonicalEncoding`] if canonical encoding fails.
    pub fn digest(&self) -> Result<[u8; 32], HijiriParametersError> {
        self.validate()?;
        let encoded =
            norito::encode_canonical(self).map_err(|_| HijiriParametersError::CanonicalEncoding)?;
        Ok(Hash::new_from_chunks(&[HIJIRI_PARAMETERS_DIGEST_DOMAIN_V1, encoded.as_slice()]).into())
    }

    /// Return the effective risk for an account and its optional explicit record.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError::FeeQuoteAccountMismatch`] when a supplied record belongs
    /// to another account.
    pub fn effective_risk(
        &self,
        account_id: &AccountId,
        account_risk: Option<&HijiriAccountRiskV1>,
    ) -> Result<Q16, HijiriParametersError> {
        match account_risk {
            Some(account_risk) if &account_risk.account_id == account_id => Ok(account_risk.risk),
            Some(_) => Err(HijiriParametersError::FeeQuoteAccountMismatch),
            None => Ok(self.default_account_risk),
        }
    }

    /// Return the active fee multiplier for an account.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError::FeeQuoteAccountMismatch`] when a supplied record belongs
    /// to another account.
    pub fn multiplier_for(
        &self,
        account_id: &AccountId,
        account_risk: Option<&HijiriAccountRiskV1>,
    ) -> Result<Q16, HijiriParametersError> {
        Ok(self
            .fee_policy
            .multiplier_for(self.effective_risk(account_id, account_risk)?))
    }

    /// Apply the account multiplier to an aggregate fee in minor units, rounding upward once.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError::FeeQuoteAccountMismatch`] when a supplied record belongs
    /// to another account.
    pub fn apply_fee_minor_units(
        &self,
        account_id: &AccountId,
        account_risk: Option<&HijiriAccountRiskV1>,
        aggregate_base: u64,
    ) -> Result<Option<u64>, HijiriParametersError> {
        Ok(self
            .multiplier_for(account_id, account_risk)?
            .checked_mul_u64_ceil(aggregate_base))
    }

    /// Commit the exact global policy and account-risk presence/value used for a fee quote.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError`] when either record is invalid, belongs to another
    /// account, or cannot be canonically encoded.
    pub fn fee_quote_hash(
        &self,
        account_id: &AccountId,
        account_risk: Option<&HijiriAccountRiskV1>,
    ) -> Result<[u8; 32], HijiriParametersError> {
        self.validate()?;
        let account_risk_digest = match account_risk {
            Some(account_risk) if &account_risk.account_id == account_id => {
                Some(account_risk.digest()?)
            }
            Some(_) => return Err(HijiriParametersError::FeeQuoteAccountMismatch),
            None => None,
        };
        hijiri_fee_quote_hash_from_digests_v1(self.digest()?, account_id, account_risk_digest)
    }
}

#[derive(Encode)]
struct HijiriFeeQuotePreimageV1 {
    version: u16,
    parameters_digest: [u8; 32],
    account_id: AccountId,
    account_risk_digest: Option<[u8; 32]>,
}

/// Derive the canonical V1 composite fee-quote hash from already authenticated record digests.
///
/// This helper lets proof and transport projections verify that their global digest, canonical
/// account identity, and explicit account-risk presence/value are bound by the advertised quote
/// hash without requiring the complete records to be transported again.
///
/// # Errors
///
/// Returns [`HijiriParametersError::CanonicalEncoding`] if the canonical preimage cannot be
/// encoded.
pub fn hijiri_fee_quote_hash_from_digests_v1(
    parameters_digest: [u8; 32],
    account_id: &AccountId,
    account_risk_digest: Option<[u8; 32]>,
) -> Result<[u8; 32], HijiriParametersError> {
    let preimage = HijiriFeeQuotePreimageV1 {
        version: HIJIRI_PARAMETERS_VERSION_V1,
        parameters_digest,
        account_id: account_id.clone(),
        account_risk_digest,
    };
    let encoded = norito::encode_canonical(&preimage)
        .map_err(|_| HijiriParametersError::CanonicalEncoding)?;
    Ok(Hash::new_from_chunks(&[HIJIRI_FEE_QUOTE_DIGEST_DOMAIN_V1, encoded.as_slice()]).into())
}

/// Versioned, bounded risk record for one canonical universal account.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct HijiriAccountRiskV1 {
    /// Schema version. Must equal [`HIJIRI_ACCOUNT_RISK_VERSION_V1`].
    pub version: u16,
    /// Canonical universal account identity governed by this record.
    pub account_id: AccountId,
    /// Strictly increasing record revision, beginning at one.
    pub revision: u64,
    /// Digest of the immediately preceding record revision.
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::fixed_bytes::option")
    )]
    pub previous_digest: Option<[u8; 32]>,
    /// Governed risk score in the inclusive Q16 range `[0, 1]`.
    pub risk: Q16,
}
impl HijiriAccountRiskV1 {
    /// Construct a canonical account-risk record.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError`] for invalid lineage, payload size, or risk above one.
    pub fn try_new(
        account_id: AccountId,
        revision: u64,
        previous_digest: Option<[u8; 32]>,
        risk: Q16,
    ) -> Result<Self, HijiriParametersError> {
        let record = Self {
            version: HIJIRI_ACCOUNT_RISK_VERSION_V1,
            account_id,
            revision,
            previous_digest,
            risk,
        };
        record.validate()?;
        Ok(record)
    }

    /// Derive the collision-resistant custom-parameter identifier for `account_id`.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError::CanonicalEncoding`] if the account cannot be encoded, or
    /// [`HijiriParametersError::ParameterIdEncoding`] if the reserved identifier is invalid.
    pub fn parameter_id_for(
        account_id: &AccountId,
    ) -> Result<CustomParameterId, HijiriParametersError> {
        let encoded = norito::encode_canonical(account_id)
            .map_err(|_| HijiriParametersError::CanonicalEncoding)?;
        let digest: [u8; 32] =
            Hash::new_from_chunks(&[HIJIRI_ACCOUNT_RISK_ID_DOMAIN_V1, encoded.as_slice()]).into();
        format!(
            "{HIJIRI_ACCOUNT_RISK_PARAMETER_PREFIX_V1}{}",
            hex::encode(digest)
        )
        .parse()
        .map_err(|_| HijiriParametersError::ParameterIdEncoding)
    }

    /// Return this record's collision-resistant custom-parameter identifier.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError`] when identifier derivation fails.
    pub fn parameter_id(&self) -> Result<CustomParameterId, HijiriParametersError> {
        Self::parameter_id_for(&self.account_id)
    }

    /// Convert this record into the custom parameter accepted by `SetParameter`.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError`] when validation or identifier derivation fails.
    pub fn into_custom_parameter(self) -> Result<CustomParameter, HijiriParametersError> {
        self.validate()?;
        Ok(CustomParameter::new(self.parameter_id()?, Json::new(self)))
    }

    /// Strictly decode a matching account-risk custom parameter.
    ///
    /// Non-matching identifiers return `Ok(None)`.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError`] for malformed JSON, invalid contents, or an identifier
    /// that does not match the embedded account.
    pub fn from_custom_parameter(
        custom: &CustomParameter,
    ) -> Result<Option<Self>, HijiriParametersError> {
        if !custom
            .id()
            .name()
            .as_ref()
            .starts_with(HIJIRI_ACCOUNT_RISK_PARAMETER_PREFIX_V1)
        {
            return Ok(None);
        }
        if custom.payload().get().len() > MAX_HIJIRI_ACCOUNT_RISK_ENCODED_BYTES {
            return Err(HijiriParametersError::AccountRiskPayloadTooLarge);
        }
        let record = custom
            .payload()
            .try_into_any_norito::<Self>()
            .map_err(|_| HijiriParametersError::MalformedAccountRiskPayload)?;
        record.validate()?;
        if custom.id() != &record.parameter_id()? {
            return Err(HijiriParametersError::AccountRiskParameterIdMismatch);
        }
        Ok(Some(record))
    }

    /// Validate bounded contents and self-contained revision shape.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError`] when an invariant is violated.
    pub fn validate(&self) -> Result<(), HijiriParametersError> {
        if self.version != HIJIRI_ACCOUNT_RISK_VERSION_V1 {
            return Err(HijiriParametersError::UnsupportedAccountRiskVersion(
                self.version,
            ));
        }
        if self.revision == 0 {
            return Err(HijiriParametersError::ZeroRevision);
        }
        match (self.revision, self.previous_digest) {
            (1, Some(_)) => return Err(HijiriParametersError::InitialHasPreviousDigest),
            (2.., None) => return Err(HijiriParametersError::SuccessorMissingPreviousDigest),
            _ => {}
        }
        if self.risk > Q16::ONE {
            return Err(HijiriParametersError::RiskAboveOne);
        }
        let encoded =
            norito::encode_canonical(self).map_err(|_| HijiriParametersError::CanonicalEncoding)?;
        if encoded.len() > MAX_HIJIRI_ACCOUNT_RISK_ENCODED_BYTES {
            return Err(HijiriParametersError::AccountRiskPayloadTooLarge);
        }
        Ok(())
    }

    /// Validate an exact initial install or strict digest-linked successor for the same account.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError`] for account changes, skipped/replayed revisions, or a
    /// predecessor mismatch.
    pub fn validate_transition(
        previous: Option<&Self>,
        next: &Self,
    ) -> Result<(), HijiriParametersError> {
        next.validate()?;
        let Some(previous) = previous else {
            if next.revision != 1 {
                return Err(HijiriParametersError::FirstRevisionNotOne(next.revision));
            }
            return Ok(());
        };
        previous.validate()?;
        if previous.account_id != next.account_id {
            return Err(HijiriParametersError::AccountRiskAccountChanged);
        }
        let expected_revision = previous
            .revision
            .checked_add(1)
            .ok_or(HijiriParametersError::RevisionOverflow)?;
        if next.revision != expected_revision {
            return Err(HijiriParametersError::UnexpectedRevision {
                expected: expected_revision,
                found: next.revision,
            });
        }
        if next.previous_digest != Some(previous.digest()?) {
            return Err(HijiriParametersError::PreviousDigestMismatch);
        }
        Ok(())
    }

    /// Return the domain-separated digest used to link a successor and bind a fee quote.
    ///
    /// # Errors
    ///
    /// Returns [`HijiriParametersError::CanonicalEncoding`] if canonical encoding fails.
    pub fn digest(&self) -> Result<[u8; 32], HijiriParametersError> {
        self.validate()?;
        let encoded =
            norito::encode_canonical(self).map_err(|_| HijiriParametersError::CanonicalEncoding)?;
        Ok(
            Hash::new_from_chunks(&[HIJIRI_ACCOUNT_RISK_DIGEST_DOMAIN_V1, encoded.as_slice()])
                .into(),
        )
    }
}

/// Return whether a custom parameter belongs to the consensus-owned Hijiri fee surface.
#[must_use]
pub fn is_hijiri_parameter_id(id: &CustomParameterId) -> bool {
    id == &HijiriParametersV1::parameter_id()
        || id
            .name()
            .as_ref()
            .starts_with(HIJIRI_ACCOUNT_RISK_PARAMETER_PREFIX_V1)
}

/// Validation failures for the canonical on-chain Hijiri parameter.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum HijiriParametersError {
    /// Matching custom-parameter JSON could not be decoded.
    #[error("Hijiri parameter payload is malformed")]
    MalformedPayload,
    /// The parameter declared an unsupported schema version.
    #[error("unsupported Hijiri parameter version {0}")]
    UnsupportedVersion(u16),
    /// Revision zero is never valid.
    #[error("Hijiri parameter revision must begin at one")]
    ZeroRevision,
    /// Revision one must not claim a predecessor.
    #[error("initial Hijiri parameter revision must not carry a previous digest")]
    InitialHasPreviousDigest,
    /// Revisions after one must bind their predecessor.
    #[error("successor Hijiri parameter revision must carry a previous digest")]
    SuccessorMissingPreviousDigest,
    /// The fee policy failed its own bounds.
    #[error("invalid Hijiri fee policy: {0}")]
    InvalidFeePolicy(FeePolicyError),
    /// The complete canonical snapshot exceeded its byte budget.
    #[error("Hijiri parameter canonical payload is too large")]
    EncodedPayloadTooLarge,
    /// Matching account-risk JSON could not be decoded.
    #[error("Hijiri account-risk payload is malformed")]
    MalformedAccountRiskPayload,
    /// The account-risk parameter exceeded its byte budget.
    #[error("Hijiri account-risk payload is too large")]
    AccountRiskPayloadTooLarge,
    /// The account-risk record declared an unsupported schema version.
    #[error("unsupported Hijiri account-risk version {0}")]
    UnsupportedAccountRiskVersion(u16),
    /// A derived custom-parameter identifier could not be represented canonically.
    #[error("Hijiri account-risk parameter identifier could not be encoded")]
    ParameterIdEncoding,
    /// The account-risk custom-parameter id did not match its embedded account.
    #[error("Hijiri account-risk parameter identifier does not match its account")]
    AccountRiskParameterIdMismatch,
    /// A successor attempted to change the account governed by an existing risk record.
    #[error("Hijiri account-risk successor cannot change its account")]
    AccountRiskAccountChanged,
    /// A fee quote was given a risk record for another account.
    #[error("Hijiri fee quote account does not match its account-risk record")]
    FeeQuoteAccountMismatch,
    /// Canonical Norito encoding failed.
    #[error("Hijiri parameter canonical encoding failed")]
    CanonicalEncoding,
    /// Account risk exceeded the inclusive unit range.
    #[error("Hijiri account risk must not exceed one")]
    RiskAboveOne,
    /// A first install attempted to skip revision one.
    #[error("first Hijiri parameter revision must be one, found {0}")]
    FirstRevisionNotOne(u64),
    /// The previous revision could not be incremented.
    #[error("Hijiri parameter revision overflow")]
    RevisionOverflow,
    /// A successor skipped or replayed a revision.
    #[error("expected Hijiri parameter revision {expected}, found {found}")]
    UnexpectedRevision {
        /// Required successor revision.
        expected: u64,
        /// Supplied revision.
        found: u64,
    },
    /// A successor did not commit to the exact active parameter.
    #[error("Hijiri parameter previous digest does not match the active revision")]
    PreviousDigestMismatch,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Metadata;
    use iroha_crypto::{Algorithm, KeyPair};
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("test fixture random key generation should succeed")
    }
    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("deterministic Hijiri test key")
                .public_key()
                .clone(),
        )
    }
    fn fee_policy() -> HijiriFeePolicy {
        HijiriFeePolicy::new(
            vec![
                FeeMultiplierBand::new(Q16::from_parts(0, 0x8000), Q16::ONE)
                    .expect("low-risk band"),
                FeeMultiplierBand::new(Q16::ONE, Q16::from_parts(2, 0)).expect("high-risk band"),
            ],
            Q16::from_parts(3, 0),
        )
        .expect("Hijiri test policy")
    }
    fn norito_roundtrip<T>(value: &T) -> T
    where
        T: Encode + Decode,
    {
        let encoded = value.encode();
        T::decode(&mut encoded.as_slice()).expect("valid Hijiri value should round-trip")
    }
    #[derive(Encode)]
    struct ForgedRegistryCreditSchedule {
        reward_account: AccountId,
        reward_per_attestation: Numeric,
        settlement_period_rounds: u32,
    }
    #[test]
    fn q16_saturating_mul_caps() {
        let value = Q16::from_parts(0, 0x8000); // 0.5
        assert_eq!(value.saturating_mul(2).raw(), Q16::ONE.raw());
        let huge = Q16::from_raw(u32::MAX);
        assert_eq!(huge.saturating_mul(2).raw(), u32::MAX);
    }
    #[test]
    fn q16_minor_unit_multiplication_rounds_penalties_up() {
        assert_eq!(Q16::ONE.checked_mul_u64_ceil(10), Some(10));
        assert_eq!(Q16::from_parts(1, 0x8000).checked_mul_u64_ceil(1), Some(2));
        assert_eq!(
            Q16::from_parts(1, 0x4000).checked_mul_u64_ceil(10),
            Some(13)
        );
        assert_eq!(Q16::from_raw(u32::MAX).checked_mul_u64_ceil(u64::MAX), None);
    }
    #[test]
    fn q16_display_handles_the_full_fraction_range() {
        assert_eq!(Q16::from_parts(0, u16::MAX).to_string(), "0.99998");
        assert_eq!(Q16::from_raw(u32::MAX).to_string(), "65535.99998");
    }
    #[test]
    fn incentive_apply_respects_cap() {
        let reward_account = {
            let kp = checked_random_keypair();
            AccountId::new(kp.public_key().clone())
        };
        let schedule = RegistryCreditSchedule {
            reward_account,
            reward_per_attestation: Quantity::from(100_u32),
            settlement_period_rounds: 4,
        };
        let incentive = PositiveAttestationIncentive::new(
            Q16::from_parts(0, 0x4000), // 0.25
            Q16::ONE,
            schedule,
        )
        .expect("valid incentive");
        // Start at zero and apply five attestations; cap should clamp to 1.0.
        let boosted = incentive.apply_boost(Q16::ZERO, 5);
        assert_eq!(boosted, Q16::ONE);
    }
    #[test]
    fn incentive_apply_never_reduces_an_existing_boost() {
        let reward_account = {
            let kp = checked_random_keypair();
            AccountId::new(kp.public_key().clone())
        };
        let incentive = PositiveAttestationIncentive::new(
            Q16::from_parts(0, 0x1000),
            Q16::from_parts(0, 0x4000),
            RegistryCreditSchedule {
                reward_account,
                reward_per_attestation: Quantity::from(1_u32),
                settlement_period_rounds: 1,
            },
        )
        .expect("valid incentive");
        let existing = Q16::from_parts(0, 0x8000);
        assert_eq!(incentive.apply_boost(existing, 0), existing);
        assert_eq!(incentive.apply_boost(existing, 1), existing);
    }
    #[test]
    fn incentive_rewards_scale_linearly() {
        let reward_account = {
            let kp = checked_random_keypair();
            AccountId::new(kp.public_key().clone())
        };
        let schedule = RegistryCreditSchedule {
            reward_account,
            reward_per_attestation: Quantity::from(500_u32),
            settlement_period_rounds: 2,
        };
        let incentive = PositiveAttestationIncentive::new(
            Q16::from_parts(0, 0x2000),
            Q16::from_parts(2, 0),
            schedule.clone(),
        )
        .expect("valid incentive");
        assert_eq!(
            schedule.total_reward(3).expect("bounded reward"),
            Quantity::from(1_500_u32)
        );
        assert_eq!(
            incentive
                .registry_credit
                .total_reward(3)
                .expect("bounded reward"),
            Quantity::from(1_500_u32)
        );
    }
    #[test]
    fn profile_positive_incentive_lookup() {
        let kp = checked_random_keypair();
        let reward_account = AccountId::new(kp.public_key().clone());
        let schedule = RegistryCreditSchedule {
            reward_account,
            reward_per_attestation: Quantity::from(250_u32),
            settlement_period_rounds: 1,
        };
        let incentive = PositiveAttestationIncentive::new(
            Q16::from_parts(0, 0x0800),
            Q16::from_parts(0, 0x4000),
            schedule,
        )
        .expect("valid incentive");
        let profile = ObserverProfile {
            id: ObserverProfileId::new("psp_compliance".parse().unwrap()),
            version: 1,
            display_name: "PSP Compliance Registry".to_string(),
            jurisdiction: "EU".to_string(),
            capabilities: vec![
                DelegatedAttestationClass::Positive(incentive.clone()),
                DelegatedAttestationClass::Negative {
                    penalty_q16: Q16::from_parts(0, 0x0800),
                    max_penalty_q16: Some(Q16::from_parts(0, 0x8000)),
                },
            ],
            metadata: Metadata::default(),
        };
        assert!(profile.positive_incentive().is_some());
    }
    #[test]
    fn registry_credit_rejects_forged_negative_reward() {
        let forged = ForgedRegistryCreditSchedule {
            reward_account: AccountId::new(checked_random_keypair().public_key().clone()),
            reward_per_attestation: Numeric::new(-1_i32, 0),
            settlement_period_rounds: 1,
        };
        let encoded = forged.encode();
        let mut input = encoded.as_slice();
        assert!(
            <RegistryCreditSchedule as Decode>::decode(&mut input).is_err(),
            "nominal reward decoding must reject a forged negative value"
        );
    }
    #[test]
    fn norito_decode_rejects_invalid_positive_incentive() {
        let forged = PositiveAttestationIncentive {
            score_boost_per_attestation: Q16::ZERO,
            max_score_boost: Q16::ZERO,
            registry_credit: RegistryCreditSchedule {
                reward_account: AccountId::new(checked_random_keypair().public_key().clone()),
                reward_per_attestation: Quantity::from(0_u32),
                settlement_period_rounds: 0,
            },
        };
        let encoded = forged.encode();
        assert!(
            PositiveAttestationIncentive::decode(&mut encoded.as_slice()).is_err(),
            "Norito decoding must enforce positive incentive invariants"
        );
    }
    #[test]
    fn norito_roundtrips_validated_hijiri_values() {
        let band = FeeMultiplierBand::new(Q16::ONE, Q16::from_parts(2, 0))
            .expect("valid fee multiplier band");
        let policy = HijiriFeePolicy::new(vec![band], Q16::from_parts(3, 0))
            .expect("valid Hijiri fee policy");
        assert_eq!(norito_roundtrip(&policy), policy);

        let field = EvidenceFieldCommitment::new(
            vec!["details".to_owned(), "case_id".to_owned()],
            [0x11; 32],
            [0x22; 32],
            Some([0x33; 32]),
        )
        .expect("valid field commitment");
        let bundle = EvidenceHashBundle::new(
            EvidenceHashAlgorithm::Poseidon2Goldilocks,
            [0x44; 32],
            vec![field],
        )
        .expect("valid evidence bundle");
        assert_eq!(norito_roundtrip(&bundle), bundle);

        let incentive = PositiveAttestationIncentive::new(
            Q16::from_parts(0, 0x1000),
            Q16::from_parts(0, 0x4000),
            RegistryCreditSchedule {
                reward_account: AccountId::new(checked_random_keypair().public_key().clone()),
                reward_per_attestation: Quantity::from(1_u32),
                settlement_period_rounds: 1,
            },
        )
        .expect("valid positive attestation incentive");
        assert_eq!(norito_roundtrip(&incentive), incentive);
    }
    #[test]
    fn evidence_field_commitment_validation() {
        let commitment = EvidenceFieldCommitment::new(
            vec!["details".to_string(), "case_id".to_string()],
            [0x11; 32],
            [0x22; 32],
            None,
        )
        .expect("valid commitment");
        assert_eq!(
            commitment.field_path,
            vec!["details".to_string(), "case_id".to_string()]
        );
    }
    #[test]
    fn evidence_bundle_rejects_duplicates() {
        let commitment =
            EvidenceFieldCommitment::new(vec!["details".to_string()], [0xAA; 32], [0xBB; 32], None)
                .expect("valid commitment");
        let duplicate =
            EvidenceFieldCommitment::new(vec!["details".to_string()], [0xCC; 32], [0xDD; 32], None)
                .expect("valid commitment");
        let err = EvidenceHashBundle::new(
            EvidenceHashAlgorithm::Poseidon2Goldilocks,
            [0xEE; 32],
            vec![commitment, duplicate],
        )
        .expect_err("duplicate paths must be rejected");
        assert_eq!(err, EvidenceHashError::DuplicateFieldPath);
    }
    #[test]
    fn evidence_bundle_revalidates_public_field_paths() {
        let empty_path = EvidenceFieldCommitment {
            field_path: Vec::new(),
            commitment: [0xAA; 32],
            blinding_salt: [0xBB; 32],
            value_digest: None,
        };
        let err = EvidenceHashBundle::new(
            EvidenceHashAlgorithm::Poseidon2Goldilocks,
            [0xCC; 32],
            vec![empty_path],
        )
        .expect_err("public fields must not bypass empty-path validation");
        assert_eq!(err, EvidenceHashError::EmptyFieldPath);

        let empty_segment = EvidenceFieldCommitment {
            field_path: vec!["details".to_owned(), String::new()],
            commitment: [0xDD; 32],
            blinding_salt: [0xEE; 32],
            value_digest: None,
        };
        let err = EvidenceHashBundle::new(
            EvidenceHashAlgorithm::Poseidon2Goldilocks,
            [0xFF; 32],
            vec![empty_segment],
        )
        .expect_err("public fields must not bypass empty-segment validation");
        assert_eq!(err, EvidenceHashError::EmptyFieldSegment);
    }
    #[test]
    fn norito_decode_rejects_duplicate_evidence_paths() {
        let field =
            EvidenceFieldCommitment::new(vec!["details".to_owned()], [0xA1; 32], [0xB1; 32], None)
                .expect("valid field commitment");
        let forged = EvidenceHashBundle {
            algorithm: EvidenceHashAlgorithm::Poseidon2Goldilocks,
            payload_commitment: [0xC1; 32],
            redacted_fields: vec![field.clone(), field],
        };
        let encoded = forged.encode();
        assert!(
            EvidenceHashBundle::decode(&mut encoded.as_slice()).is_err(),
            "Norito decoding must reject duplicate evidence paths"
        );
    }
    #[test]
    fn norito_decode_rejects_noncanonical_evidence_order() {
        let field_a =
            EvidenceFieldCommitment::new(vec!["a".to_owned()], [0xA1; 32], [0xB1; 32], None)
                .expect("valid field commitment");
        let field_b =
            EvidenceFieldCommitment::new(vec!["b".to_owned()], [0xA2; 32], [0xB2; 32], None)
                .expect("valid field commitment");
        let forged = EvidenceHashBundle {
            algorithm: EvidenceHashAlgorithm::Poseidon2Goldilocks,
            payload_commitment: [0xC1; 32],
            redacted_fields: vec![field_b, field_a],
        };
        assert!(
            EvidenceHashBundle::decode(&mut forged.encode().as_slice()).is_err(),
            "Norito decoding must reject noncanonical field order"
        );
    }
    #[test]
    fn evidence_bundle_sorts_paths() {
        let field_b = EvidenceFieldCommitment::new(
            vec!["details".to_string(), "transcript".to_string()],
            [0x02; 32],
            [0x03; 32],
            None,
        )
        .expect("valid commitment");
        let field_a = EvidenceFieldCommitment::new(
            vec!["details".to_string(), "case_id".to_string()],
            [0x04; 32],
            [0x05; 32],
            Some([0x06; 32]),
        )
        .expect("valid commitment");
        let bundle = EvidenceHashBundle::new(
            EvidenceHashAlgorithm::Poseidon2Goldilocks,
            [0xFF; 32],
            vec![field_b.clone(), field_a.clone()],
        )
        .expect("sorted bundle");
        assert_eq!(bundle.redacted_fields[0].field_path, field_a.field_path);
        assert_eq!(bundle.redacted_fields[1].field_path, field_b.field_path);
    }
    #[test]
    fn evidence_paths_and_bundle_sizes_are_bounded() {
        let too_many_segments = vec!["x".to_owned(); MAX_EVIDENCE_PATH_SEGMENTS + 1];
        assert_eq!(
            EvidenceFieldCommitment::new(too_many_segments, [1; 32], [2; 32], None),
            Err(EvidenceHashError::TooManyPathSegments)
        );
        assert_eq!(
            EvidenceFieldCommitment::new(
                vec!["x".repeat(MAX_EVIDENCE_PATH_SEGMENT_BYTES + 1)],
                [1; 32],
                [2; 32],
                None,
            ),
            Err(EvidenceHashError::PathSegmentTooLong)
        );
        let field = EvidenceFieldCommitment::new(vec!["x".to_owned()], [1; 32], [2; 32], None)
            .expect("bounded evidence field");
        assert_eq!(
            EvidenceHashBundle::new(
                EvidenceHashAlgorithm::Poseidon2Goldilocks,
                [3; 32],
                vec![field; MAX_EVIDENCE_REDACTED_FIELDS + 1],
            ),
            Err(EvidenceHashError::TooManyRedactedFields)
        );
    }
    #[test]
    fn fee_policy_enforces_ordering_and_bounds() {
        let bands = vec![
            FeeMultiplierBand::new(Q16::from_parts(0, 0x8000), Q16::from_parts(1, 0)).unwrap(),
            FeeMultiplierBand::new(Q16::ONE, Q16::from_parts(1, 0x4000)).unwrap(),
        ];
        let policy =
            HijiriFeePolicy::new(bands, Q16::from_parts(2, 0)).expect("policy should be valid");
        assert_eq!(
            policy.multiplier_for(Q16::from_parts(0, 0x1000)),
            Q16::from_parts(1, 0)
        );
        assert_eq!(
            policy.multiplier_for(Q16::from_parts(0, 0xF000)),
            Q16::from_parts(1, 0x4000)
        );
    }
    #[test]
    fn fee_policy_enforces_the_exact_band_count_limit() {
        let bands = |count: usize| {
            let count = u64::try_from(count).expect("test band count fits u64");
            (1..=count)
                .map(|position| {
                    let raw = u32::try_from(u64::from(Q16::ONE.raw()) * position / count)
                        .expect("interpolated risk fits Q16");
                    FeeMultiplierBand::new(Q16::from_raw(raw), Q16::ONE)
                        .expect("strictly increasing bounded band")
                })
                .collect::<Vec<_>>()
        };

        assert!(
            HijiriFeePolicy::new(bands(MAX_FEE_MULTIPLIER_BANDS), Q16::ONE).is_ok(),
            "the exact band limit must remain accepted"
        );
        assert_eq!(
            HijiriFeePolicy::new(bands(MAX_FEE_MULTIPLIER_BANDS + 1), Q16::ONE),
            Err(FeePolicyError::TooManyBands)
        );
    }
    #[test]
    fn fee_policy_revalidates_public_band_fields() {
        let forged_band = FeeMultiplierBand {
            max_risk: Q16::ONE,
            multiplier: Q16::ZERO,
        };
        let err = HijiriFeePolicy::new(vec![forged_band], Q16::ONE)
            .expect_err("public band fields must not bypass multiplier validation");
        assert_eq!(err, FeePolicyError::MultiplierBelowOne);
    }
    #[test]
    fn norito_decode_rejects_invalid_fee_band() {
        let forged_band = FeeMultiplierBand {
            max_risk: Q16::ONE,
            multiplier: Q16::ZERO,
        };
        let encoded = forged_band.encode();
        assert!(
            FeeMultiplierBand::decode(&mut encoded.as_slice()).is_err(),
            "Norito decoding must enforce fee-band invariants"
        );
    }
    #[test]
    fn malformed_public_fee_policy_cannot_discount_the_base_fee() {
        let policy = HijiriFeePolicy {
            bands: vec![FeeMultiplierBand {
                max_risk: Q16::ONE,
                multiplier: Q16::ZERO,
            }],
            penalty_cap: Q16::ZERO,
        };
        assert_eq!(policy.multiplier_for(Q16::ZERO), Q16::ONE);
        assert_eq!(policy.apply(Q16::ONE, Q16::ZERO), Q16::ONE);
    }
    #[test]
    fn fee_policy_applies_penalty_cap() {
        let bands = vec![
            FeeMultiplierBand::new(Q16::from_parts(0, 0x8000), Q16::from_parts(3, 0)).unwrap(),
            FeeMultiplierBand::new(Q16::ONE, Q16::from_parts(5, 0)).unwrap(),
        ];
        let policy =
            HijiriFeePolicy::new(bands, Q16::from_parts(4, 0)).expect("policy should be capped");
        assert_eq!(
            policy.multiplier_for(Q16::from_parts(0, 0xF000)),
            Q16::from_parts(4, 0)
        );
        let base_fee = Q16::from_parts(0, 0x4000);
        assert_eq!(
            policy.apply(base_fee, Q16::from_parts(0, 0xF000)),
            base_fee.saturating_mul_q16(Q16::from_parts(4, 0))
        );
    }
    #[test]
    fn hijiri_parameters_and_account_risk_roundtrip() {
        let low_risk = account(1);
        let parameters = HijiriParametersV1::try_new(1, None, fee_policy(), Q16::ONE)
            .expect("canonical Hijiri parameters");
        let risk = HijiriAccountRiskV1::try_new(low_risk.clone(), 1, None, Q16::ZERO)
            .expect("canonical risk record");
        assert_eq!(
            parameters.effective_risk(&low_risk, Some(&risk)),
            Ok(Q16::ZERO)
        );
        assert_eq!(parameters.effective_risk(&account(3), None), Ok(Q16::ONE));
        assert_eq!(
            parameters.apply_fee_minor_units(&low_risk, Some(&risk), 10),
            Ok(Some(10))
        );
        assert_eq!(
            parameters.apply_fee_minor_units(&account(3), None, 10),
            Ok(Some(20))
        );

        let custom = parameters.clone().into_custom_parameter();
        assert_eq!(
            HijiriParametersV1::from_custom_parameter(&custom).expect("decode custom parameter"),
            Some(parameters)
        );
        let risk_custom = risk
            .clone()
            .into_custom_parameter()
            .expect("risk parameter");
        assert_eq!(
            HijiriAccountRiskV1::from_custom_parameter(&risk_custom)
                .expect("decode risk parameter"),
            Some(risk)
        );
    }
    #[test]
    fn first_release_genesis_is_exact_neutral_and_roundtrips_json() {
        let parameters = HijiriParametersV1::first_release_genesis();
        assert_eq!(parameters.version, HIJIRI_PARAMETERS_VERSION_V1);
        assert_eq!(parameters.revision, 1);
        assert_eq!(parameters.previous_digest, None);
        assert_eq!(parameters.default_account_risk, Q16::ZERO);
        assert_eq!(parameters.fee_policy.penalty_cap, Q16::ONE);
        assert_eq!(
            parameters.fee_policy.bands,
            vec![FeeMultiplierBand {
                max_risk: Q16::ONE,
                multiplier: Q16::ONE,
            }]
        );
        assert_eq!(
            parameters
                .apply_fee_minor_units(&account(13), None, 10)
                .expect("neutral genesis policy must evaluate"),
            Some(10)
        );

        let json = norito::json::to_json(&parameters).expect("encode genesis Hijiri JSON");
        let actual = norito::json::parse_value(&json).expect("parse encoded genesis Hijiri JSON");
        let expected = norito::json::parse_value(
            r#"{
                "version": 1,
                "revision": 1,
                "previous_digest": null,
                "fee_policy": {
                    "bands": [{
                        "max_risk": [65536],
                        "multiplier": [65536]
                    }],
                    "penalty_cap": [65536]
                },
                "default_account_risk": [0]
            }"#,
        )
        .expect("parse expected genesis Hijiri JSON");
        assert_eq!(actual, expected);

        let decoded: HijiriParametersV1 =
            norito::json::from_json(&json).expect("decode genesis Hijiri JSON");
        assert_eq!(decoded, parameters);
    }
    #[test]
    fn hijiri_global_parameter_rejects_malformed_matching_custom_payload() {
        let custom = CustomParameter::new(
            HijiriParametersV1::parameter_id(),
            Json::from_raw_json(r#"{"version":1}"#.to_owned())
                .expect("syntactically valid malformed Hijiri fixture"),
        );

        assert_eq!(
            HijiriParametersV1::from_custom_parameter(&custom),
            Err(HijiriParametersError::MalformedPayload)
        );
    }
    #[test]
    fn hijiri_account_risk_rejects_parameter_id_mismatch() {
        let record = HijiriAccountRiskV1::try_new(account(9), 1, None, Q16::ZERO)
            .expect("valid account-risk record");
        let foreign_parameter_id = HijiriAccountRiskV1::parameter_id_for(&account(10))
            .expect("foreign account-risk parameter id");
        let custom = CustomParameter::new(foreign_parameter_id, Json::new(record));

        assert_eq!(
            HijiriAccountRiskV1::from_custom_parameter(&custom),
            Err(HijiriParametersError::AccountRiskParameterIdMismatch)
        );
    }
    #[test]
    fn hijiri_global_transition_rejects_invalid_shapes_skips_and_overflow() {
        assert_eq!(
            HijiriParametersV1::try_new(0, None, fee_policy(), Q16::ONE),
            Err(HijiriParametersError::ZeroRevision)
        );
        assert_eq!(
            HijiriParametersV1::try_new(1, Some([1; 32]), fee_policy(), Q16::ONE),
            Err(HijiriParametersError::InitialHasPreviousDigest)
        );
        assert_eq!(
            HijiriParametersV1::try_new(2, None, fee_policy(), Q16::ONE),
            Err(HijiriParametersError::SuccessorMissingPreviousDigest)
        );

        let first = HijiriParametersV1::try_new(1, None, fee_policy(), Q16::ONE)
            .expect("valid initial parameters");
        HijiriParametersV1::validate_transition(None, &first)
            .expect("revision one is the exact initial install");

        let successor = HijiriParametersV1::try_new(
            2,
            Some(first.digest().expect("initial digest")),
            fee_policy(),
            Q16::ZERO,
        )
        .expect("valid successor shape");
        HijiriParametersV1::validate_transition(Some(&first), &successor)
            .expect("revision two with the exact predecessor is valid");

        let skipped = HijiriParametersV1::try_new(
            3,
            Some(first.digest().expect("initial digest")),
            fee_policy(),
            Q16::ZERO,
        )
        .expect("self-contained skipped-revision shape");
        assert_eq!(
            HijiriParametersV1::validate_transition(Some(&first), &skipped),
            Err(HijiriParametersError::UnexpectedRevision {
                expected: 2,
                found: 3,
            })
        );

        let first_revision_skipped =
            HijiriParametersV1::try_new(2, Some([2; 32]), fee_policy(), Q16::ONE)
                .expect("self-contained successor shape");
        assert_eq!(
            HijiriParametersV1::validate_transition(None, &first_revision_skipped),
            Err(HijiriParametersError::FirstRevisionNotOne(2))
        );

        let previous_at_max = HijiriParametersV1 {
            version: HIJIRI_PARAMETERS_VERSION_V1,
            revision: u64::MAX,
            previous_digest: Some([3; 32]),
            fee_policy: fee_policy(),
            default_account_risk: Q16::ONE,
        };
        previous_at_max
            .validate()
            .expect("maximum revision remains self-contained");
        assert_eq!(
            HijiriParametersV1::validate_transition(Some(&previous_at_max), &first),
            Err(HijiriParametersError::RevisionOverflow)
        );
    }
    #[test]
    fn hijiri_account_risk_transition_rejects_account_change() {
        let first = HijiriAccountRiskV1::try_new(account(11), 1, None, Q16::ZERO)
            .expect("valid initial account-risk record");
        let changed_account = HijiriAccountRiskV1::try_new(
            account(12),
            2,
            Some(first.digest().expect("initial account-risk digest")),
            Q16::ONE,
        )
        .expect("valid successor-shaped record for another account");

        assert_eq!(
            HijiriAccountRiskV1::validate_transition(Some(&first), &changed_account),
            Err(HijiriParametersError::AccountRiskAccountChanged)
        );
    }
    #[test]
    fn hijiri_parameter_and_account_risk_enforce_lineage() {
        assert_eq!(
            HijiriAccountRiskV1::try_new(account(4), 1, None, Q16::from_raw(Q16::ONE.raw() + 1)),
            Err(HijiriParametersError::RiskAboveOne)
        );

        let first = HijiriParametersV1::try_new(1, None, fee_policy(), Q16::ONE)
            .expect("initial parameters");
        let second = HijiriParametersV1::try_new(
            2,
            Some(first.digest().expect("first revision digest")),
            fee_policy(),
            Q16::ONE,
        )
        .expect("successor parameters");
        HijiriParametersV1::validate_transition(Some(&first), &second).expect("strict successor");
        let wrong = HijiriParametersV1::try_new(2, Some([9; 32]), fee_policy(), Q16::ONE)
            .expect("self-contained successor shape");
        assert_eq!(
            HijiriParametersV1::validate_transition(Some(&first), &wrong),
            Err(HijiriParametersError::PreviousDigestMismatch)
        );

        let account_id = account(5);
        let first_risk =
            HijiriAccountRiskV1::try_new(account_id.clone(), 1, None, Q16::ZERO).unwrap();
        let second_risk = HijiriAccountRiskV1::try_new(
            account_id,
            2,
            Some(first_risk.digest().expect("first risk digest")),
            Q16::ONE,
        )
        .unwrap();
        HijiriAccountRiskV1::validate_transition(Some(&first_risk), &second_risk)
            .expect("strict account-risk successor");
    }
    #[test]
    fn fee_quote_hash_binds_record_presence_and_revision() {
        let account_id = account(6);
        let parameters = HijiriParametersV1::try_new(1, None, fee_policy(), Q16::ZERO).unwrap();
        let without_record = parameters
            .fee_quote_hash(&account_id, None)
            .expect("default quote hash");
        let first = HijiriAccountRiskV1::try_new(account_id.clone(), 1, None, Q16::ZERO).unwrap();
        let first_hash = parameters
            .fee_quote_hash(&account_id, Some(&first))
            .expect("explicit quote hash");
        let second = HijiriAccountRiskV1::try_new(
            account_id.clone(),
            2,
            Some(first.digest().unwrap()),
            Q16::ZERO,
        )
        .unwrap();
        let second_hash = parameters
            .fee_quote_hash(&account_id, Some(&second))
            .expect("successor quote hash");
        assert_eq!(
            first_hash,
            hijiri_fee_quote_hash_from_digests_v1(
                parameters.digest().unwrap(),
                &account_id,
                Some(first.digest().unwrap()),
            )
            .expect("digest-only quote hash"),
        );
        assert_ne!(without_record, first_hash);
        assert_ne!(first_hash, second_hash);
        assert_ne!(
            without_record,
            hijiri_fee_quote_hash_from_digests_v1(parameters.digest().unwrap(), &account(8), None,)
                .expect("other-account quote hash"),
        );
    }
    #[test]
    fn aggregate_fee_is_rounded_once() {
        let account_id = account(7);
        let policy = HijiriFeePolicy::new(
            vec![FeeMultiplierBand::new(Q16::ONE, Q16::from_parts(1, 0x8000)).unwrap()],
            Q16::from_parts(1, 0x8000),
        )
        .unwrap();
        let parameters = HijiriParametersV1::try_new(1, None, policy, Q16::ZERO).unwrap();
        assert_eq!(
            parameters.apply_fee_minor_units(&account_id, None, 3),
            Ok(Some(5))
        );
    }
}
