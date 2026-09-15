//! Canonical, state-independent SNS policy pricing shared by consensus and clients.
//!
//! These functions calculate rent only. Consensus separately checks ownership, reservations,
//! live namespace state, quote guards, payment authorization and normal transaction fees.

use super::{NameSelectorV1, PriceTierV1, SuffixPolicyV1, SuffixStatus};
use crate::asset::{AssetDefinitionId, AssetId};
use iroha_primitives::numeric::{Numeric, Quantity};
use regex::Regex;
use thiserror::Error;

/// Invalid requested terms or inconsistent advertised policy state.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PricingError {
    /// The requested label, pricing class or duration is not offered by the policy.
    #[error("{0}")]
    BadRequest(String),
    /// The policy is inactive, malformed or has unrepresentable rent.
    #[error("{0}")]
    Conflict(String),
}

/// Require an active SNS policy before quoting or mutating a lease.
///
/// # Errors
/// Returns a typed policy or requested-term error when validation fails.
pub fn enforce_policy_active(policy: &SuffixPolicyV1) -> Result<(), PricingError> {
    match policy.status {
        SuffixStatus::Active => Ok(()),
        SuffixStatus::Paused => Err(PricingError::Conflict(format!(
            "suffix `{}` is paused",
            policy.suffix_key()
        ))),
        SuffixStatus::Revoked => Err(PricingError::Conflict(format!(
            "suffix `{}` is revoked",
            policy.suffix_key()
        ))),
    }
}

fn tier_regex(tier: &PriceTierV1) -> Result<Regex, PricingError> {
    Regex::new(&tier.label_regex).map_err(|err| {
        PricingError::Conflict(format!(
            "pricing tier {} has invalid label regex: {err}",
            tier.tier_id
        ))
    })
}

/// Match one canonical label against a policy tier with the consensus regex engine.
///
/// # Errors
/// Rejects an invalid policy regex instead of selecting a different tier.
pub fn label_matches_tier(tier: &PriceTierV1, label: &str) -> Result<bool, PricingError> {
    Ok(tier_regex(tier)?.is_match(label))
}

/// Select the first matching policy tier, or verify the requested exact pricing class.
///
/// # Errors
/// Returns a typed policy or requested-term error when validation fails.
pub fn pick_pricing_tier(
    policy: &SuffixPolicyV1,
    selector: &NameSelectorV1,
    pricing_class_hint: Option<u8>,
) -> Result<PriceTierV1, PricingError> {
    let label = selector.normalized_label();
    if let Some(hint) = pricing_class_hint {
        let tier = policy
            .pricing
            .iter()
            .find(|tier| tier.tier_id == hint)
            .ok_or_else(|| {
                PricingError::BadRequest(format!(
                    "pricing class {hint} is not offered for suffix `{}`",
                    policy.suffix_key()
                ))
            })?;
        if !label_matches_tier(tier, label)? {
            return Err(PricingError::BadRequest(format!(
                "label `{label}` does not satisfy pricing class {hint}"
            )));
        }
        return Ok(tier.clone());
    }
    for tier in &policy.pricing {
        if label_matches_tier(tier, label)? {
            return Ok(tier.clone());
        }
    }
    Err(PricingError::BadRequest(format!(
        "label `{label}` does not match any pricing tier for suffix `{}`",
        policy.suffix_key()
    )))
}

/// Verify the retained pricing class against the current policy and canonical selector.
///
/// # Errors
/// Returns a typed policy or requested-term error when validation fails.
pub fn tier_by_pricing_class(
    policy: &SuffixPolicyV1,
    selector: &NameSelectorV1,
    pricing_class: u8,
) -> Result<PriceTierV1, PricingError> {
    let label = selector.normalized_label();
    let tier = policy
        .pricing
        .iter()
        .find(|tier| tier.tier_id == pricing_class)
        .ok_or_else(|| {
            PricingError::BadRequest(format!(
                "pricing class {pricing_class} is not offered for suffix `{}`",
                policy.suffix_key()
            ))
        })?;
    if !label_matches_tier(tier, label)? {
        return Err(PricingError::BadRequest(format!(
            "label `{label}` no longer satisfies pricing class {pricing_class}"
        )));
    }
    Ok(tier.clone())
}

/// Validate lease duration against the intersection of policy and selected tier bounds.
///
/// # Errors
/// Returns a typed policy or requested-term error when validation fails.
pub fn validate_term_bounds(
    policy: &SuffixPolicyV1,
    tier: &PriceTierV1,
    term_years: u8,
) -> Result<(), PricingError> {
    let min_years = policy.min_term_years.max(tier.min_duration_years);
    let max_years = policy.max_term_years.min(tier.max_duration_years);
    if min_years > max_years {
        return Err(PricingError::Conflict(format!(
            "suffix `{}` has incompatible policy/tier term bounds",
            policy.suffix_key()
        )));
    }
    if term_years < min_years || term_years > max_years {
        return Err(PricingError::BadRequest(format!(
            "term_years must be between {min_years} and {max_years} (got {term_years})"
        )));
    }
    Ok(())
}

/// Calculate exact rent for a whole-year lease using deterministic decimal arithmetic.
///
/// # Errors
/// Returns a typed policy or requested-term error when validation fails.
pub fn required_payment_amount(
    tier: &PriceTierV1,
    term_years: u8,
) -> Result<Quantity, PricingError> {
    tier.base_price
        .amount
        .try_mul_decimal(&Numeric::from(u32::from(term_years)))
        .map_err(|_| {
            PricingError::Conflict(format!(
                "required payment overflowed for pricing class {}",
                tier.tier_id
            ))
        })
}

/// Resolve the canonical payment asset definition from the policy settlement selector.
///
/// # Errors
/// Returns a typed policy or requested-term error when validation fails.
pub fn payment_asset_definition_id(
    policy: &SuffixPolicyV1,
) -> Result<AssetDefinitionId, PricingError> {
    if let Ok(asset_id) = AssetId::parse_literal(&policy.payment_asset_id) {
        return Ok(asset_id.definition().clone());
    }
    AssetDefinitionId::parse_address_literal(&policy.payment_asset_id).map_err(|err| {
        PricingError::Conflict(format!(
            "suffix `{}` has invalid payment asset `{}`: {err}",
            policy.suffix_key(),
            policy.payment_asset_id
        ))
    })
}

/// Exact policy rent for one canonical name and lease term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeasePrice {
    /// Selected policy pricing class.
    pub pricing_class: u8,
    /// Exact rent for the requested duration.
    pub amount: Quantity,
    /// Canonical asset definition in which rent must be paid.
    pub payment_asset: AssetDefinitionId,
}

/// Quote exact rent from one retained policy image using the same rules as consensus.
///
/// # Errors
/// Rejects wrong namespace, inactive policy, unavailable tier, invalid duration or rent overflow.
pub fn quote_lease_price(
    policy: &SuffixPolicyV1,
    selector: &NameSelectorV1,
    term_years: u8,
    pricing_class_hint: Option<u8>,
) -> Result<LeasePrice, PricingError> {
    if selector.suffix_id != policy.suffix_id {
        return Err(PricingError::BadRequest(
            "selector and policy suffix ids differ".to_owned(),
        ));
    }
    enforce_policy_active(policy)?;
    let tier = pick_pricing_tier(policy, selector, pricing_class_hint)?;
    validate_term_bounds(policy, &tier, term_years)?;
    Ok(LeasePrice {
        pricing_class: tier.tier_id,
        amount: required_payment_amount(&tier, term_years)?,
        payment_asset: payment_asset_definition_id(policy)?,
    })
}

#[cfg(test)]
mod tests;
