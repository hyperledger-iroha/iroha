//! Hijiri reputation system data structures.
//!
//! This module defines the portable data types used by the Hijiri peer and account reputation
//! pipelines. The initial focus is on account-level positive attestations: observer registries
//! describe which external parties may issue them, and incentives determine how much an attestation
//! can boost the `S_attestation` component of the global risk score as well as how registries are
//! compensated.
use crate::{account::AccountId, metadata::Metadata, name::Name};
use derive_more::{AsRef, Deref};
use iroha_primitives::numeric::{Numeric, NumericOperationError, Quantity};
use norito::codec::{Decode, Encode};
use std::{convert::TryFrom, fmt};
use thiserror::Error;
/// Unsigned Q16.16 fixed-point representation backed by `u32`.
///
/// Hijiri scoring relies on Q16.16 arithmetic. This lightweight wrapper keeps
/// the encoding explicit while providing saturating helpers for common math.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, AsRef, Deref, Default,
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
        Self::new(
            wire.algorithm,
            wire.payload_commitment,
            wire.redacted_fields,
        )
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
        // Ensure field paths are unique and deterministically ordered.
        redacted_fields.sort_by(|a, b| a.field_path.cmp(&b.field_path));
        let mut last_path: Option<&[String]> = None;
        for field in &redacted_fields {
            validate_evidence_field_path(&field.field_path)?;
            if last_path.is_some_and(|prev| prev == &field.field_path[..]) {
                return Err(EvidenceHashError::DuplicateFieldPath);
            }
            last_path = Some(&field.field_path);
        }
        Ok(Self {
            algorithm,
            payload_commitment,
            redacted_fields,
        })
    }
}
fn validate_evidence_field_path(field_path: &[String]) -> Result<(), EvidenceHashError> {
    if field_path.is_empty() {
        return Err(EvidenceHashError::EmptyFieldPath);
    }
    if field_path.iter().any(String::is_empty) {
        return Err(EvidenceHashError::EmptyFieldSegment);
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
}
/// Band describing the fee multiplier applied to a given Hijiri risk range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
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
        if bands.is_empty() {
            return Err(FeePolicyError::NoBands);
        }
        if penalty_cap.0 < Q16::ONE.0 {
            return Err(FeePolicyError::PenaltyCapBelowOne);
        }
        bands.sort_by(|a, b| a.max_risk.cmp(&b.max_risk));
        let mut prev = Q16::ZERO;
        for band in &bands {
            band.validate()?;
            if band.max_risk.0 <= prev.0 {
                return Err(FeePolicyError::DescendingBounds);
            }
            prev = band.max_risk;
        }
        if bands
            .last()
            .is_none_or(|band| band.max_risk.0 != Q16::ONE.0)
        {
            return Err(FeePolicyError::TerminalBandBelowOne);
        }
        Ok(Self { bands, penalty_cap })
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Metadata;
    use iroha_crypto::KeyPair;
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("test fixture random key generation should succeed")
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
}
