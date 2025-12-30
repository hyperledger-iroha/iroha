//! Pricing manifests and probabilistic micropayment policies for SoraFS.

use std::{collections::HashSet, num::NonZeroU32};

use norito::{
    derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize},
    json::{Map, Value},
};
use thiserror::Error;

/// SoraFS pricing manifest schema version.
pub const PRICING_MANIFEST_VERSION_V1: u8 = 1;

/// Number of seconds in one hour.
const SECONDS_PER_HOUR: u128 = 60 * 60;
/// Number of bytes in a gibibyte.
const BYTES_PER_GIB: u128 = 1024 * 1024 * 1024;
/// Conversion factor from milli-units to nano-units (1 milli = 10⁻³ XOR).
const MILLU_TO_NANOS: u128 = 1_000_000;
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

        if self.tiers.is_empty() {
            return Err(PricingManifestError::NoTiers);
        }

        let mut seen = HashSet::with_capacity(self.tiers.len());
        for tier in &self.tiers {
            if !seen.insert(tier.tier_id.clone()) {
                return Err(PricingManifestError::DuplicateTier {
                    tier_id: tier.tier_id.clone(),
                });
            }
            tier.validate()
                .map_err(|source| PricingManifestError::TierInvalid {
                    tier_id: tier.tier_id.clone(),
                    source,
                })?;
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
    /// Storage price per GiB·hour in milli-units of the manifest currency.
    pub storage_price_milliu_per_gib_hour: u64,
    /// Egress price per GiB transferred in milli-units of the manifest currency.
    pub egress_price_milliu_per_gib: u64,
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
        if self.storage_price_milliu_per_gib_hour == 0 && self.egress_price_milliu_per_gib == 0 {
            return Err(PricingTierError::ZeroPricing);
        }
        if let Some(bps) = self.min_collateral_ratio_bps {
            NonZeroU32::new(bps).ok_or(PricingTierError::CollateralBasisPointsZero)?;
            if bps > MAX_COLLATERAL_RATIO_BPS {
                return Err(PricingTierError::CollateralBasisPointsTooHigh { value: bps });
            }
        }
        if let Some(notes) = &self.notes
            && notes.trim().is_empty()
        {
            return Err(PricingTierError::InvalidNotes);
        }
        Ok(())
    }

    /// Calculate the storage fee in nano-units for the supplied GiB·seconds duration.
    #[must_use]
    pub fn storage_fee_nanos_for_gib_seconds(&self, gib_seconds: u128) -> u128 {
        if self.storage_price_milliu_per_gib_hour == 0 || gib_seconds == 0 {
            return 0;
        }
        let price = u128::from(self.storage_price_milliu_per_gib_hour);
        let numerator = price.saturating_mul(gib_seconds);
        let fee_milliu = numerator.div_ceil(SECONDS_PER_HOUR);
        fee_milliu.saturating_mul(MILLU_TO_NANOS)
    }

    /// Calculate the egress fee in nano-units for the supplied byte length.
    #[must_use]
    pub fn egress_fee_nanos_for_bytes(&self, bytes: u64) -> u128 {
        if self.egress_price_milliu_per_gib == 0 || bytes == 0 {
            return 0;
        }
        let price = u128::from(self.egress_price_milliu_per_gib);
        let numerator = price.saturating_mul(u128::from(bytes));
        let fee_milliu = numerator.div_ceil(BYTES_PER_GIB);
        fee_milliu.saturating_mul(MILLU_TO_NANOS)
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
    /// Ceiling for a single voucher payout in nano-units.
    pub max_voucher_value_nanos: u128,
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
        if self.max_voucher_value_nanos == 0 {
            return Err(PricingMicropaymentPolicyError::ZeroVoucherCap);
        }
        if self
            .notes
            .as_ref()
            .is_some_and(|notes| notes.trim().is_empty())
        {
            return Err(PricingMicropaymentPolicyError::InvalidNotes);
        }
        Ok(())
    }

    /// Evaluate a micropayment voucher for the supplied deterministic nonce.
    ///
    /// The `nonce` must be uniformly distributed in `[0, 10_000)` to honour the basis-point
    /// probability scaling. When `nonce < payout_probability_bps`, the payout amount is returned;
    /// otherwise the voucher carries no payout for this round.
    #[must_use]
    pub fn evaluate(&self, nonce: u16, fee_nanos: u128) -> MicropaymentDecision {
        if self.payout_probability_bps == 0 {
            return MicropaymentDecision::skip(fee_nanos, self.payout_probability_bps);
        }

        if nonce >= self.payout_probability_bps {
            return MicropaymentDecision::skip(fee_nanos, self.payout_probability_bps);
        }

        let probability = u128::from(self.payout_probability_bps);
        let expected_multiplier = BASIS_POINTS_SCALE;

        let payout_raw = fee_nanos
            .saturating_mul(expected_multiplier)
            .saturating_add(probability - 1);
        let payout = payout_raw / probability;
        let capped = payout.min(self.max_voucher_value_nanos);

        MicropaymentDecision::pay(fee_nanos, capped, self.payout_probability_bps)
    }
}

/// Outcome of evaluating a probabilistic micropayment voucher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicropaymentDecision {
    /// Whether the voucher should be paid out.
    pub should_pay: bool,
    /// Value of the payout in nano-units.
    pub payout_nanos: u128,
    /// Probability (basis points) used for the decision.
    pub probability_bps: u16,
    /// Expected fee value (nano-units) associated with the voucher.
    pub expected_fee_nanos: u128,
}

impl MicropaymentDecision {
    fn pay(expected_fee_nanos: u128, payout_nanos: u128, probability_bps: u16) -> Self {
        Self {
            should_pay: true,
            payout_nanos,
            probability_bps,
            expected_fee_nanos,
        }
    }

    fn skip(expected_fee_nanos: u128, probability_bps: u16) -> Self {
        Self {
            should_pay: false,
            payout_nanos: 0,
            probability_bps,
            expected_fee_nanos,
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
    /// No tiers provided by the manifest.
    #[error("pricing manifest must contain at least one tier")]
    NoTiers,
    /// Duplicate tier identifier encountered.
    #[error("duplicate pricing tier \"{tier_id}\"")]
    DuplicateTier { tier_id: String },
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
}

fn validate_currency(currency: &str) -> Result<(), String> {
    let trimmed = currency.trim();
    if trimmed.len() < 3 || trimmed.len() > 6 {
        return Err("currency code must be 3–6 characters".into());
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return Err("currency code must consist of lowercase ASCII alphanumerics".into());
    }
    Ok(())
}

fn validate_tier_id(tier_id: &str) -> Result<(), PricingTierError> {
    let trimmed = tier_id.trim();
    if trimmed.is_empty() {
        return Err(PricingTierError::EmptyTierId);
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
    {
        return Err(PricingTierError::InvalidTierId {
            tier_id: tier_id.to_string(),
        });
    }
    Ok(())
}

/// Helper to construct a deterministic nonce map from a JSON array of integers.
#[must_use]
pub fn load_nonces_from_json(array: &Value) -> Vec<u16> {
    let mut nonces = Vec::new();
    if let Some(values) = array.as_array() {
        for value in values {
            if let Some(number) = value.as_u64() {
                nonces.push((number % BASIS_POINTS_SCALE as u64) as u16);
            }
        }
    }
    nonces
}

/// Persist nonce samples into a JSON map for diagnostics.
#[must_use]
pub fn nonce_samples_to_json(samples: &[u16]) -> Value {
    let mut root = Map::new();
    let values: Vec<Value> = samples
        .iter()
        .map(|value| Value::from(*value as u64))
        .collect();
    root.insert("nonces".into(), Value::Array(values));
    Value::Object(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_pricing_manifest_roundtrip() {
        let manifest = PricingManifestV1 {
            version: PRICING_MANIFEST_VERSION_V1,
            currency: "xor".into(),
            effective_from_unix: 1_704_361_600,
            tiers: vec![
                PricingTierV1 {
                    tier_id: "hot".into(),
                    storage_price_milliu_per_gib_hour: 500,
                    egress_price_milliu_per_gib: 50,
                    min_collateral_ratio_bps: Some(15_000),
                    notes: Some("Low latency targets".into()),
                },
                PricingTierV1 {
                    tier_id: "warm".into(),
                    storage_price_milliu_per_gib_hour: 200,
                    egress_price_milliu_per_gib: 20,
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
                max_voucher_value_nanos: 5_000_000_000,
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
            storage_price_milliu_per_gib_hour: 500,
            egress_price_milliu_per_gib: 0,
            min_collateral_ratio_bps: None,
            notes: None,
        };
        tier.validate().expect("tier valid");

        // One GiB reserved for an hour should cost exactly 0.5 XOR.
        let fee = tier.storage_fee_nanos_for_gib_seconds(SECONDS_PER_HOUR);
        assert_eq!(fee, 500 * MILLU_TO_NANOS);

        // Half-hour should round up to the nearest milli-unit.
        let half_fee = tier.storage_fee_nanos_for_gib_seconds(SECONDS_PER_HOUR / 2);
        assert_eq!(half_fee, 250 * MILLU_TO_NANOS);
    }

    #[test]
    fn egress_fee_rounding_matches_expectation() {
        let tier = PricingTierV1 {
            tier_id: "egress".into(),
            storage_price_milliu_per_gib_hour: 0,
            egress_price_milliu_per_gib: 10,
            min_collateral_ratio_bps: None,
            notes: None,
        };
        tier.validate().expect("tier valid");

        let one_gib = tier.egress_fee_nanos_for_bytes(BYTES_PER_GIB as u64);
        assert_eq!(one_gib, 10 * MILLU_TO_NANOS);

        let half_gib = tier.egress_fee_nanos_for_bytes((BYTES_PER_GIB / 2) as u64);
        assert_eq!(half_gib, 5 * MILLU_TO_NANOS);
    }

    #[test]
    fn micropayment_decision_respects_probability() {
        let fee_nanos = 500 * MILLU_TO_NANOS;
        let policy = PricingMicropaymentPolicyV1 {
            payout_probability_bps: 5_000,
            max_voucher_value_nanos: fee_nanos * 4,
            notes: None,
        };
        policy.validate().expect("policy valid");

        let payout = policy.evaluate(50, fee_nanos);
        assert!(payout.should_pay);
        assert!(payout.payout_nanos >= fee_nanos);

        let skip = policy.evaluate(6_000, fee_nanos);
        assert!(!skip.should_pay);
        assert_eq!(skip.payout_nanos, 0);
    }

    #[test]
    fn duplicate_tier_detected() {
        let manifest = PricingManifestV1 {
            version: PRICING_MANIFEST_VERSION_V1,
            currency: "xor".into(),
            effective_from_unix: 0,
            tiers: vec![
                PricingTierV1 {
                    tier_id: "same".into(),
                    storage_price_milliu_per_gib_hour: 1,
                    egress_price_milliu_per_gib: 0,
                    min_collateral_ratio_bps: None,
                    notes: None,
                },
                PricingTierV1 {
                    tier_id: "same".into(),
                    storage_price_milliu_per_gib_hour: 1,
                    egress_price_milliu_per_gib: 0,
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
}
