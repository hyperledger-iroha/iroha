//! Pricing manifests and probabilistic micropayment policies for SoraFS.

use std::num::{NonZeroU32, NonZeroU64};

use iroha_crypto::numeric::RoundingMode;
use norito::{
    derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize},
    json::{Map, Value},
};
use thiserror::Error;

use crate::deal::{DealAmountError, XorQuantity};

pub mod signed;

/// SoraFS pricing manifest schema version.
pub const PRICING_MANIFEST_VERSION_V1: u8 = 1;
/// Maximum number of tiers carried by one pricing manifest.
pub const MAX_PRICING_TIERS: usize = 256;
/// Maximum canonical pricing-tier identifier length.
pub const MAX_PRICING_TIER_ID_LEN: usize = 64;
/// Maximum UTF-8 byte length of an optional human-readable note.
pub const MAX_PRICING_NOTES_LEN: usize = 1_024;
/// Maximum nonce samples accepted by pricing diagnostic JSON helpers.
pub const MAX_PRICING_NONCE_SAMPLES: usize = 65_536;

/// Number of seconds in one hour.
const SECONDS_PER_HOUR: u128 = 60 * 60;
/// Number of bytes in a gibibyte.
const BYTES_PER_GIB: u128 = 1024 * 1024 * 1024;
/// Basis points denominator.
const BASIS_POINTS_SCALE: u128 = 10_000;
/// Maximum collateral ratio (basis points) supported by pricing tiers (10×).
const MAX_COLLATERAL_RATIO_BPS: u32 = 100_000;

/// Pricing manifest describing storage/egress tiers and settlement policy.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PricingManifestV1 {
    /// Manifest schema version (`PRICING_MANIFEST_VERSION_V1`).
    pub version: u8,
    /// Currency denomination (three to six ASCII lowercase characters/digits).
    pub currency: String,
    /// Unix timestamp (seconds) when the manifest becomes effective.
    pub effective_from_unix: u64,
    /// Pricing tiers available to consumers.
    pub tiers: Vec<PricingTierV1>,
    /// Credit settlement policy.
    pub credit_policy: CreditPolicyV1,
    /// Collateral/bond policy for providers.
    pub bond_policy: BondPolicyV1,
    /// Probabilistic micropayment policy applied to retrieval vouchers.
    #[norito(default)]
    pub micropayment_policy: Option<PricingMicropaymentPolicyV1>,
}

impl PricingManifestV1 {
    /// Validate the pricing manifest, returning an error describing the first invalid field.
    pub fn validate(&self) -> Result<(), PricingManifestError> {
        if self.version != PRICING_MANIFEST_VERSION_V1 {
            return Err(PricingManifestError::UnsupportedVersion {
                version: self.version,
            });
        }

        validate_currency(&self.currency)
            .map_err(|reason| PricingManifestError::CurrencyInvalid { reason })?;

        if self.effective_from_unix == 0 {
            return Err(PricingManifestError::MissingEffectiveTime);
        }

        if self.tiers.is_empty() {
            return Err(PricingManifestError::NoTiers);
        }
        if self.tiers.len() > MAX_PRICING_TIERS {
            return Err(PricingManifestError::TooManyTiers {
                count: self.tiers.len(),
                max: MAX_PRICING_TIERS,
            });
        }

        let mut previous_tier_id: Option<&str> = None;
        for tier in &self.tiers {
            tier.validate()
                .map_err(|source| PricingManifestError::TierInvalid {
                    tier_id: tier.tier_id.clone(),
                    source,
                })?;
            if let Some(previous) = previous_tier_id {
                if previous == tier.tier_id {
                    return Err(PricingManifestError::DuplicateTier {
                        tier_id: tier.tier_id.clone(),
                    });
                }
                if previous > tier.tier_id.as_str() {
                    return Err(PricingManifestError::TiersNotSorted);
                }
            }
            previous_tier_id = Some(&tier.tier_id);
        }

        self.credit_policy
            .validate()
            .map_err(PricingManifestError::CreditPolicyInvalid)?;

        self.bond_policy
            .validate()
            .map_err(PricingManifestError::BondPolicyInvalid)?;

        if let Some(policy) = &self.micropayment_policy {
            policy
                .validate()
                .map_err(PricingManifestError::MicropaymentPolicyInvalid)?;
        }

        Ok(())
    }

    /// Lookup the specified pricing tier.
    #[must_use]
    pub fn tier(&self, tier_id: &str) -> Option<&PricingTierV1> {
        self.tiers.iter().find(|tier| tier.tier_id == tier_id)
    }

    /// Serialise the pricing manifest into a JSON value.
    pub fn to_json(&self) -> Result<Value, PricingManifestError> {
        norito::json::to_value(self).map_err(|err| PricingManifestError::Json(err.to_string()))
    }

    /// Attempt to parse a pricing manifest from a JSON value.
    pub fn from_json(value: &Value) -> Result<Self, PricingManifestError> {
        norito::json::from_value::<PricingManifestV1>(value.clone())
            .map_err(|err| PricingManifestError::Json(err.to_string()))
            .and_then(|manifest| {
                manifest.validate()?;
                Ok(manifest)
            })
    }
}

/// Pricing tier describing storage and egress fees.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PricingTierV1 {
    /// Tier identifier (`[a-z0-9_-]+`).
    pub tier_id: String,
    /// Exact storage price per GiB·hour in the manifest currency.
    pub storage_price_per_gib_hour: XorQuantity,
    /// Exact egress price per GiB transferred in the manifest currency.
    pub egress_price_per_gib: XorQuantity,
    /// Minimum collateral ratio expressed in basis points.
    #[norito(default)]
    pub min_collateral_ratio_bps: Option<u32>,
    /// Optional human-readable notes.
    #[norito(default)]
    pub notes: Option<String>,
}

impl PricingTierV1 {
    /// Ensure the tier adheres to validation rules.
    pub fn validate(&self) -> Result<(), PricingTierError> {
        validate_tier_id(&self.tier_id)?;
        if self.storage_price_per_gib_hour.is_zero() && self.egress_price_per_gib.is_zero() {
            return Err(PricingTierError::ZeroPricing);
        }
        if let Some(bps) = self.min_collateral_ratio_bps {
            NonZeroU32::new(bps).ok_or(PricingTierError::CollateralBasisPointsZero)?;
            if bps > MAX_COLLATERAL_RATIO_BPS {
                return Err(PricingTierError::CollateralBasisPointsTooHigh { value: bps });
            }
        }
        if let Some(notes) = &self.notes {
            validate_notes(notes).map_err(|error| match error {
                NotesValidationError::Invalid => PricingTierError::InvalidNotes,
                NotesValidationError::TooLong { len } => PricingTierError::NotesTooLong {
                    len,
                    max: MAX_PRICING_NOTES_LEN,
                },
            })?;
        }
        Ok(())
    }

    /// Calculate the exact storage fee for the supplied GiB·seconds duration.
    ///
    /// The V1 settlement rule rounds fractional results upward at nine decimal
    /// places so a non-zero usage is never silently free.
    pub fn storage_fee_for_gib_seconds(
        &self,
        gib_seconds: u128,
    ) -> Result<XorQuantity, DealAmountError> {
        if self.storage_price_per_gib_hour.is_zero() || gib_seconds == 0 {
            return Ok(XorQuantity::zero());
        }
        self.storage_price_per_gib_hour
            .checked_mul_u128(gib_seconds)?
            .checked_div_u64_round(
                NonZeroU64::new(SECONDS_PER_HOUR as u64).expect("hour is non-zero"),
                9,
                RoundingMode::Ceil,
            )
    }

    /// Calculate the exact egress fee for the supplied byte length.
    ///
    /// The V1 settlement rule rounds fractional results upward at nine decimal
    /// places so a non-zero transfer is never silently free.
    pub fn egress_fee_for_bytes(&self, bytes: u64) -> Result<XorQuantity, DealAmountError> {
        if self.egress_price_per_gib.is_zero() || bytes == 0 {
            return Ok(XorQuantity::zero());
        }
        self.egress_price_per_gib
            .checked_mul_u64(bytes)?
            .checked_div_u64_round(
                NonZeroU64::new(BYTES_PER_GIB as u64).expect("GiB is non-zero"),
                9,
                RoundingMode::Ceil,
            )
    }
}

/// Settlement and credit policy for buyers.
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
pub struct CreditPolicyV1 {
    /// Settlement window in seconds (minimum 3600).
    pub settlement_window_secs: u32,
    /// Threshold (basis points of expected weekly spend) that triggers a top-up alert.
    #[norito(default)]
    pub auto_top_up_threshold_bps: u16,
}

impl CreditPolicyV1 {
    /// Validate the credit policy.
    pub fn validate(&self) -> Result<(), CreditPolicyError> {
        if self.settlement_window_secs < 3600 {
            return Err(CreditPolicyError::SettlementWindowTooShort {
                seconds: self.settlement_window_secs,
            });
        }
        if self.auto_top_up_threshold_bps > 10_000 {
            return Err(CreditPolicyError::TopUpThresholdTooHigh {
                value: self.auto_top_up_threshold_bps,
            });
        }
        Ok(())
    }
}

/// Bond and collateral requirements applied to providers.
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
pub struct BondPolicyV1 {
    /// Collateral multiplier expressed in basis points (e.g., 30_000 = 3×).
    pub collateral_ratio_bps: u32,
    /// Grace period for newly-admitted providers (days).
    #[norito(default)]
    pub new_provider_grace_days: u16,
}

impl BondPolicyV1 {
    /// Validate the bond policy.
    pub fn validate(&self) -> Result<(), BondPolicyError> {
        if self.collateral_ratio_bps < 10_000 {
            return Err(BondPolicyError::CollateralRatioTooLow {
                value: self.collateral_ratio_bps,
            });
        }
        if self.collateral_ratio_bps > MAX_COLLATERAL_RATIO_BPS {
            return Err(BondPolicyError::CollateralRatioTooHigh {
                value: self.collateral_ratio_bps,
            });
        }
        Ok(())
    }
}

/// Probabilistic micropayment configuration for retrieval vouchers.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct PricingMicropaymentPolicyV1 {
    /// Probability (basis points) that a voucher pays out.
    pub payout_probability_bps: u16,
    /// Exact ceiling for a single voucher payout.
    pub max_voucher_value: XorQuantity,
    /// Optional human-readable notes.
    #[norito(default)]
    pub notes: Option<String>,
}

impl PricingMicropaymentPolicyV1 {
    /// Validate micropayment policy invariants.
    pub fn validate(&self) -> Result<(), PricingMicropaymentPolicyError> {
        if self.payout_probability_bps == 0 || self.payout_probability_bps > 10_000 {
            return Err(PricingMicropaymentPolicyError::InvalidProbability {
                value: self.payout_probability_bps,
            });
        }
        if self.max_voucher_value.is_zero() {
            return Err(PricingMicropaymentPolicyError::ZeroVoucherCap);
        }
        if let Some(notes) = &self.notes {
            validate_notes(notes).map_err(|error| match error {
                NotesValidationError::Invalid => PricingMicropaymentPolicyError::InvalidNotes,
                NotesValidationError::TooLong { len } => {
                    PricingMicropaymentPolicyError::NotesTooLong {
                        len,
                        max: MAX_PRICING_NOTES_LEN,
                    }
                }
            })?;
        }
        Ok(())
    }

    /// Evaluate a micropayment voucher for the supplied deterministic nonce.
    ///
    /// The `nonce` must be uniformly distributed in `[0, 10_000)` to honour the basis-point
    /// probability scaling. When `nonce < payout_probability_bps`, the payout amount is returned;
    /// otherwise the voucher carries no payout for this round.
    pub fn evaluate(
        &self,
        nonce: u16,
        fee: &XorQuantity,
    ) -> Result<MicropaymentDecision, PricingMicropaymentEvaluationError> {
        self.validate()?;
        if nonce >= BASIS_POINTS_SCALE as u16 {
            return Err(PricingMicropaymentEvaluationError::NonceOutOfRange { nonce });
        }

        if fee.is_zero() || nonce >= self.payout_probability_bps {
            return Ok(MicropaymentDecision::skip(
                fee.clone(),
                self.payout_probability_bps,
            ));
        }

        let capped = if fee >= &self.max_voucher_value {
            self.max_voucher_value.clone()
        } else {
            let probability = NonZeroU64::new(u64::from(self.payout_probability_bps))
                .expect("validated payout probability is non-zero");
            match fee.checked_mul_ratio_round(
                BASIS_POINTS_SCALE as u64,
                probability,
                9,
                RoundingMode::Ceil,
            ) {
                Ok(payout) => XorQuantity::min(&payout, &self.max_voucher_value),
                // A positive payout outside the bounded XOR domain necessarily
                // exceeds every representable voucher cap.
                Err(DealAmountError::Overflow) => self.max_voucher_value.clone(),
                Err(error) => return Err(error.into()),
            }
        };

        Ok(MicropaymentDecision::pay(
            fee.clone(),
            capped,
            self.payout_probability_bps,
        ))
    }
}

/// Outcome of evaluating a probabilistic micropayment voucher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicropaymentDecision {
    /// Whether the voucher should be paid out.
    pub should_pay: bool,
    /// Exact value of the payout.
    pub payout: XorQuantity,
    /// Probability (basis points) used for the decision.
    pub probability_bps: u16,
    /// Exact expected fee value associated with the voucher.
    pub expected_fee: XorQuantity,
}

impl MicropaymentDecision {
    fn pay(expected_fee: XorQuantity, payout: XorQuantity, probability_bps: u16) -> Self {
        Self {
            should_pay: true,
            payout,
            probability_bps,
            expected_fee,
        }
    }

    fn skip(expected_fee: XorQuantity, probability_bps: u16) -> Self {
        Self {
            should_pay: false,
            payout: XorQuantity::zero(),
            probability_bps,
            expected_fee,
        }
    }
}

/// Errors encountered while validating a pricing manifest.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PricingManifestError {
    /// Unsupported manifest version encountered.
    #[error("unsupported pricing manifest version {version}")]
    UnsupportedVersion { version: u8 },
    /// Invalid currency string.
    #[error("invalid currency string: {reason}")]
    CurrencyInvalid { reason: String },
    /// Effective timestamp is the invalid Unix epoch sentinel.
    #[error("pricing manifest effective_from_unix must be non-zero")]
    MissingEffectiveTime,
    /// No tiers provided by the manifest.
    #[error("pricing manifest must contain at least one tier")]
    NoTiers,
    /// Tier count exceeds the V1 resource bound.
    #[error("pricing manifest tier count {count} exceeds maximum {max}")]
    TooManyTiers { count: usize, max: usize },
    /// Duplicate tier identifier encountered.
    #[error("duplicate pricing tier \"{tier_id}\"")]
    DuplicateTier { tier_id: String },
    /// Tiers are not in canonical identifier order.
    #[error("pricing tiers must be sorted by tier identifier")]
    TiersNotSorted,
    /// Individual tier failed validation.
    #[error("tier \"{tier_id}\" invalid: {source}")]
    TierInvalid {
        tier_id: String,
        #[source]
        source: PricingTierError,
    },
    /// Credit policy validation error.
    #[error("credit policy invalid: {0}")]
    CreditPolicyInvalid(#[source] CreditPolicyError),
    /// Bond policy validation error.
    #[error("bond policy invalid: {0}")]
    BondPolicyInvalid(#[source] BondPolicyError),
    /// Micropayment policy validation error.
    #[error("micropayment policy invalid: {0}")]
    MicropaymentPolicyInvalid(#[source] PricingMicropaymentPolicyError),
    /// JSON serialisation/deserialisation failure.
    #[error("pricing manifest JSON error: {0}")]
    Json(String),
}

/// Errors raised while validating a pricing tier.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PricingTierError {
    /// Tier identifier missing.
    #[error("tier identifier must not be empty")]
    EmptyTierId,
    /// Tier identifier did not match the accepted pattern.
    #[error("tier identifier \"{tier_id}\" must match [a-z0-9_-]+")]
    InvalidTierId { tier_id: String },
    /// Tier identifier exceeds the V1 byte bound.
    #[error("tier identifier length {len} exceeds maximum {max}")]
    TierIdTooLong { len: usize, max: usize },
    /// Pricing values cannot both be zero.
    #[error("storage and egress pricing cannot both be zero")]
    ZeroPricing,
    /// Collateral basis points must be non-zero.
    #[error("collateral ratio must be greater than zero")]
    CollateralBasisPointsZero,
    /// Collateral basis points cannot exceed 100%.
    #[error("collateral ratio basis points {value} exceed 100_000 limit")]
    CollateralBasisPointsTooHigh { value: u32 },
    /// Notes must not be blank when present.
    #[error("tier notes must not be blank")]
    InvalidNotes,
    /// Tier notes exceed the V1 byte bound.
    #[error("tier notes length {len} exceeds maximum {max}")]
    NotesTooLong { len: usize, max: usize },
}

/// Errors raised while validating the credit policy.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CreditPolicyError {
    /// Settlement window shorter than one hour.
    #[error("settlement window {seconds}s shorter than 1 hour")]
    SettlementWindowTooShort { seconds: u32 },
    /// Top-up threshold outside the 0–10_000 range.
    #[error("top-up threshold {value} basis points exceeds 10_000")]
    TopUpThresholdTooHigh { value: u16 },
}

/// Errors raised while validating a bond policy.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BondPolicyError {
    /// Collateral ratio must be at least 1× (10_000 bps).
    #[error("collateral ratio {value} bps below 1.0× minimum")]
    CollateralRatioTooLow { value: u32 },
    /// Collateral ratio must not exceed 10× (100_000 bps).
    #[error("collateral ratio {value} bps exceeds 10.0× maximum")]
    CollateralRatioTooHigh { value: u32 },
}

/// Errors encountered while validating a micropayment policy.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PricingMicropaymentPolicyError {
    /// Probability outside the 1–10_000 bounds.
    #[error("micropayment probability {value} basis points invalid")]
    InvalidProbability { value: u16 },
    /// Voucher cap cannot be zero.
    #[error("micropayment voucher cap must be non-zero")]
    ZeroVoucherCap,
    /// Notes must not be blank when present.
    #[error("micropayment notes must not be blank")]
    InvalidNotes,
    /// Notes exceed the V1 byte bound.
    #[error("micropayment notes length {len} exceeds maximum {max}")]
    NotesTooLong { len: usize, max: usize },
}

/// Checked pricing calculation failures.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PricingCalculationError {
    /// Exact fixed-point arithmetic exceeded `u128`.
    #[error("pricing arithmetic overflow during {operation}")]
    ArithmeticOverflow { operation: &'static str },
}

/// Micropayment evaluation failures.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PricingMicropaymentEvaluationError {
    /// The policy itself is invalid.
    #[error(transparent)]
    InvalidPolicy(#[from] PricingMicropaymentPolicyError),
    /// Deterministic nonce is outside the basis-point sample space.
    #[error("micropayment nonce {nonce} is outside 0..10_000")]
    NonceOutOfRange { nonce: u16 },
    /// Exact payout arithmetic failed.
    #[error(transparent)]
    Arithmetic(#[from] DealAmountError),
}

/// Strict JSON nonce-sample decoding failures.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PricingNonceJsonError {
    /// The root value is not an array.
    #[error("pricing nonce samples must be a JSON array")]
    NotArray,
    /// Sample count exceeds the V1 resource bound.
    #[error("pricing nonce sample count {count} exceeds maximum {max}")]
    TooManySamples { count: usize, max: usize },
    /// An array element is not an unsigned integer.
    #[error("pricing nonce sample at index {index} must be an unsigned integer")]
    InvalidSample { index: usize },
    /// An array element is outside the exact basis-point sample space.
    #[error("pricing nonce sample {value} at index {index} is outside 0..10_000")]
    SampleOutOfRange { index: usize, value: u64 },
    /// A bounded output allocation failed.
    #[error("pricing nonce sample allocation failed")]
    AllocationFailed,
}

fn validate_currency(currency: &str) -> Result<(), String> {
    if currency.len() < 3 || currency.len() > 6 {
        return Err("currency code must be 3–6 characters".into());
    }
    if !currency
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err("currency code must consist of lowercase ASCII alphanumerics".into());
    }
    Ok(())
}

fn validate_tier_id(tier_id: &str) -> Result<(), PricingTierError> {
    if tier_id.is_empty() {
        return Err(PricingTierError::EmptyTierId);
    }
    if tier_id.len() > MAX_PRICING_TIER_ID_LEN {
        return Err(PricingTierError::TierIdTooLong {
            len: tier_id.len(),
            max: MAX_PRICING_TIER_ID_LEN,
        });
    }
    if !tier_id.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
    }) {
        return Err(PricingTierError::InvalidTierId {
            tier_id: tier_id.to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotesValidationError {
    Invalid,
    TooLong { len: usize },
}

fn validate_notes(notes: &str) -> Result<(), NotesValidationError> {
    if notes.len() > MAX_PRICING_NOTES_LEN {
        return Err(NotesValidationError::TooLong { len: notes.len() });
    }
    if notes.is_empty() || notes != notes.trim() || notes.chars().any(char::is_control) {
        return Err(NotesValidationError::Invalid);
    }
    Ok(())
}

/// Helper to construct a deterministic nonce map from a JSON array of integers.
pub fn load_nonces_from_json(array: &Value) -> Result<Vec<u16>, PricingNonceJsonError> {
    let values = array.as_array().ok_or(PricingNonceJsonError::NotArray)?;
    if values.len() > MAX_PRICING_NONCE_SAMPLES {
        return Err(PricingNonceJsonError::TooManySamples {
            count: values.len(),
            max: MAX_PRICING_NONCE_SAMPLES,
        });
    }
    let mut nonces = Vec::new();
    nonces
        .try_reserve_exact(values.len())
        .map_err(|_| PricingNonceJsonError::AllocationFailed)?;
    for (index, value) in values.iter().enumerate() {
        let number = value
            .as_u64()
            .ok_or(PricingNonceJsonError::InvalidSample { index })?;
        if number >= BASIS_POINTS_SCALE as u64 {
            return Err(PricingNonceJsonError::SampleOutOfRange {
                index,
                value: number,
            });
        }
        nonces.push(u16::try_from(number).map_err(|_| {
            PricingNonceJsonError::SampleOutOfRange {
                index,
                value: number,
            }
        })?);
    }
    Ok(nonces)
}

/// Persist nonce samples into a JSON map for diagnostics.
pub fn nonce_samples_to_json(samples: &[u16]) -> Result<Value, PricingNonceJsonError> {
    if samples.len() > MAX_PRICING_NONCE_SAMPLES {
        return Err(PricingNonceJsonError::TooManySamples {
            count: samples.len(),
            max: MAX_PRICING_NONCE_SAMPLES,
        });
    }
    let mut root = Map::new();
    let mut values = Vec::new();
    values
        .try_reserve_exact(samples.len())
        .map_err(|_| PricingNonceJsonError::AllocationFailed)?;
    for (index, value) in samples.iter().copied().enumerate() {
        if value >= BASIS_POINTS_SCALE as u16 {
            return Err(PricingNonceJsonError::SampleOutOfRange {
                index,
                value: u64::from(value),
            });
        }
        values.push(Value::from(u64::from(value)));
    }
    root.insert("nonces".into(), Value::Array(values));
    Ok(Value::Object(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }

    #[test]
    fn validates_pricing_manifest_roundtrip() {
        let manifest = PricingManifestV1 {
            version: PRICING_MANIFEST_VERSION_V1,
            currency: "xor".into(),
            effective_from_unix: 1_704_361_600,
            tiers: vec![
                PricingTierV1 {
                    tier_id: "hot".into(),
                    storage_price_per_gib_hour: xor("0.5"),
                    egress_price_per_gib: xor("0.05"),
                    min_collateral_ratio_bps: Some(15_000),
                    notes: Some("Low latency targets".into()),
                },
                PricingTierV1 {
                    tier_id: "warm".into(),
                    storage_price_per_gib_hour: xor("0.2"),
                    egress_price_per_gib: xor("0.02"),
                    min_collateral_ratio_bps: None,
                    notes: None,
                },
            ],
            credit_policy: CreditPolicyV1 {
                settlement_window_secs: 86_400,
                auto_top_up_threshold_bps: 2_000,
            },
            bond_policy: BondPolicyV1 {
                collateral_ratio_bps: 30_000,
                new_provider_grace_days: 30,
            },
            micropayment_policy: Some(PricingMicropaymentPolicyV1 {
                payout_probability_bps: 100,
                max_voucher_value: xor("5"),
                notes: Some("Probabilistic micropayments".into()),
            }),
        };

        manifest.validate().expect("valid manifest");
        let json = manifest.to_json().expect("manifest to json");
        let decoded = PricingManifestV1::from_json(&json).expect("json manifest");
        assert_eq!(manifest, decoded);
    }

    #[test]
    fn storage_fee_rounding_matches_expectation() {
        let tier = PricingTierV1 {
            tier_id: "test".into(),
            storage_price_per_gib_hour: xor("0.5"),
            egress_price_per_gib: XorQuantity::zero(),
            min_collateral_ratio_bps: None,
            notes: None,
        };
        tier.validate().expect("tier valid");

        // One GiB reserved for an hour should cost exactly 0.5 XOR.
        let fee = tier
            .storage_fee_for_gib_seconds(SECONDS_PER_HOUR)
            .expect("fee arithmetic");
        assert_eq!(fee, xor("0.5"));

        // Half-hour should round up to the nearest milli-unit.
        let half_fee = tier
            .storage_fee_for_gib_seconds(SECONDS_PER_HOUR / 2)
            .expect("fee arithmetic");
        assert_eq!(half_fee, xor("0.25"));
    }

    #[test]
    fn egress_fee_rounding_matches_expectation() {
        let tier = PricingTierV1 {
            tier_id: "egress".into(),
            storage_price_per_gib_hour: XorQuantity::zero(),
            egress_price_per_gib: xor("0.01"),
            min_collateral_ratio_bps: None,
            notes: None,
        };
        tier.validate().expect("tier valid");

        let one_gib = tier
            .egress_fee_for_bytes(BYTES_PER_GIB as u64)
            .expect("fee arithmetic");
        assert_eq!(one_gib, xor("0.01"));

        let half_gib = tier
            .egress_fee_for_bytes((BYTES_PER_GIB / 2) as u64)
            .expect("fee arithmetic");
        assert_eq!(half_gib, xor("0.005"));
    }

    #[test]
    fn micropayment_decision_respects_probability() {
        let fee = xor("0.5");
        let policy = PricingMicropaymentPolicyV1 {
            payout_probability_bps: 5_000,
            max_voucher_value: xor("2"),
            notes: None,
        };
        policy.validate().expect("policy valid");

        let payout = policy.evaluate(50, &fee).expect("payout arithmetic");
        assert!(payout.should_pay);
        assert!(payout.payout >= fee);

        let skip = policy.evaluate(6_000, &fee).expect("payout arithmetic");
        assert!(!skip.should_pay);
        assert_eq!(skip.payout, XorQuantity::zero());
    }

    #[test]
    fn duplicate_tier_detected() {
        let manifest = PricingManifestV1 {
            version: PRICING_MANIFEST_VERSION_V1,
            currency: "xor".into(),
            effective_from_unix: 1,
            tiers: vec![
                PricingTierV1 {
                    tier_id: "same".into(),
                    storage_price_per_gib_hour: xor("0.001"),
                    egress_price_per_gib: XorQuantity::zero(),
                    min_collateral_ratio_bps: None,
                    notes: None,
                },
                PricingTierV1 {
                    tier_id: "same".into(),
                    storage_price_per_gib_hour: xor("0.001"),
                    egress_price_per_gib: XorQuantity::zero(),
                    min_collateral_ratio_bps: None,
                    notes: None,
                },
            ],
            credit_policy: CreditPolicyV1 {
                settlement_window_secs: 3_600,
                auto_top_up_threshold_bps: 0,
            },
            bond_policy: BondPolicyV1 {
                collateral_ratio_bps: 10_000,
                new_provider_grace_days: 0,
            },
            micropayment_policy: None,
        };

        let err = manifest.validate().expect_err("duplicate tier invalid");
        assert!(matches!(err, PricingManifestError::DuplicateTier { .. }));
    }

    #[test]
    fn manifest_rejects_zero_time_unsorted_tiers_and_excessive_cardinality() {
        let mut manifest = PricingManifestV1 {
            version: PRICING_MANIFEST_VERSION_V1,
            currency: "xor".into(),
            effective_from_unix: 0,
            tiers: vec![PricingTierV1 {
                tier_id: "hot".into(),
                storage_price_per_gib_hour: xor("0.001"),
                egress_price_per_gib: XorQuantity::zero(),
                min_collateral_ratio_bps: None,
                notes: None,
            }],
            credit_policy: CreditPolicyV1 {
                settlement_window_secs: 3_600,
                auto_top_up_threshold_bps: 0,
            },
            bond_policy: BondPolicyV1 {
                collateral_ratio_bps: 10_000,
                new_provider_grace_days: 0,
            },
            micropayment_policy: None,
        };
        assert_eq!(
            manifest.validate(),
            Err(PricingManifestError::MissingEffectiveTime)
        );

        manifest.effective_from_unix = 1;
        manifest.tiers.insert(
            0,
            PricingTierV1 {
                tier_id: "warm".into(),
                ..manifest.tiers[0].clone()
            },
        );
        assert_eq!(
            manifest.validate(),
            Err(PricingManifestError::TiersNotSorted)
        );

        manifest.tiers = (0..=MAX_PRICING_TIERS)
            .map(|index| PricingTierV1 {
                tier_id: format!("tier-{index:03}"),
                storage_price_per_gib_hour: xor("0.001"),
                egress_price_per_gib: XorQuantity::zero(),
                min_collateral_ratio_bps: None,
                notes: None,
            })
            .collect();
        assert_eq!(
            manifest.validate(),
            Err(PricingManifestError::TooManyTiers {
                count: MAX_PRICING_TIERS + 1,
                max: MAX_PRICING_TIERS,
            })
        );
    }

    #[test]
    fn canonical_text_fields_reject_padding_controls_and_oversize() {
        for currency in [" xor", "xor ", "XOR", "xør"] {
            assert!(
                validate_currency(currency).is_err(),
                "currency {currency:?}"
            );
        }
        for tier_id in [" hot", "hot ", "HOT", "hot.tier"] {
            assert!(validate_tier_id(tier_id).is_err(), "tier {tier_id:?}");
        }
        assert!(matches!(
            validate_tier_id(&"a".repeat(MAX_PRICING_TIER_ID_LEN + 1)),
            Err(PricingTierError::TierIdTooLong { .. })
        ));
        for notes in ["", " padded", "padded ", "line\nbreak", "control\0byte"] {
            assert_eq!(validate_notes(notes), Err(NotesValidationError::Invalid));
        }
        assert!(matches!(
            validate_notes(&"a".repeat(MAX_PRICING_NOTES_LEN + 1)),
            Err(NotesValidationError::TooLong { .. })
        ));
    }

    #[test]
    fn fee_calculations_are_exact_and_reject_overflow_instead_of_saturating() {
        let maximum = xor(
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047",
        );
        let storage = PricingTierV1 {
            tier_id: "storage".into(),
            storage_price_per_gib_hour: maximum,
            egress_price_per_gib: XorQuantity::zero(),
            min_collateral_ratio_bps: None,
            notes: None,
        };
        assert!(matches!(
            storage.storage_fee_for_gib_seconds(2),
            Err(DealAmountError::Overflow)
        ));

        let egress = PricingTierV1 {
            tier_id: "egress".into(),
            storage_price_per_gib_hour: XorQuantity::zero(),
            egress_price_per_gib: xor("18446744073709551615"),
            min_collateral_ratio_bps: None,
            notes: None,
        };
        assert_eq!(
            egress
                .egress_fee_for_bytes(BYTES_PER_GIB as u64)
                .expect("one GiB preserves the exact rate"),
            xor("18446744073709551615")
        );
    }

    #[test]
    fn micropayment_rejects_out_of_range_nonce_and_caps_extreme_fee_exactly() {
        let policy = PricingMicropaymentPolicyV1 {
            payout_probability_bps: 1,
            max_voucher_value: xor("0.000000123"),
            notes: None,
        };
        assert_eq!(
            policy.evaluate(10_000, &xor("1")),
            Err(PricingMicropaymentEvaluationError::NonceOutOfRange { nonce: 10_000 })
        );
        let maximum = xor(
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047",
        );
        let extreme = policy
            .evaluate(0, &maximum)
            .expect("extreme expected fee is safely capped");
        assert!(extreme.should_pay);
        assert_eq!(extreme.payout, xor("0.000000123"));
        let zero = policy.evaluate(0, &XorQuantity::zero()).expect("zero fee");
        assert!(!zero.should_pay);
        assert_eq!(zero.payout, XorQuantity::zero());
    }

    #[test]
    fn nonce_json_helpers_are_strict_bounded_and_lossless() {
        let valid = Value::Array(vec![Value::from(0_u64), Value::from(9_999_u64)]);
        assert_eq!(
            load_nonces_from_json(&valid).expect("valid nonce samples"),
            vec![0, 9_999]
        );
        assert_eq!(
            load_nonces_from_json(&Value::from(1_u64)),
            Err(PricingNonceJsonError::NotArray)
        );
        assert!(matches!(
            load_nonces_from_json(&Value::Array(vec![Value::from(-1_i64)])),
            Err(PricingNonceJsonError::InvalidSample { index: 0 })
        ));
        assert_eq!(
            load_nonces_from_json(&Value::Array(vec![Value::from(10_000_u64)])),
            Err(PricingNonceJsonError::SampleOutOfRange {
                index: 0,
                value: 10_000,
            })
        );
        assert_eq!(
            nonce_samples_to_json(&[10_000]),
            Err(PricingNonceJsonError::SampleOutOfRange {
                index: 0,
                value: 10_000,
            })
        );

        let oversized = Value::Array(vec![Value::from(0_u64); MAX_PRICING_NONCE_SAMPLES + 1]);
        assert_eq!(
            load_nonces_from_json(&oversized),
            Err(PricingNonceJsonError::TooManySamples {
                count: MAX_PRICING_NONCE_SAMPLES + 1,
                max: MAX_PRICING_NONCE_SAMPLES,
            })
        );
    }
}
