//! Consensus-owned SBD conversion accounting and reserved Nexus validator rewards.

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId, oracle::Observation};
use crate::{oracle::ObservationOutcome, validation_fee::ValidationFeeTreasuryPayoutBindingV1};
use iroha_crypto::Hash;
use iroha_model_base::state_path::StatePath;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::collections::{BTreeMap, BTreeSet};

/// Durable per-custody balances; ordinary treasury deposits never enter this ledger.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::validation_fee_rewards::ValidationFeeRewardsState")]
pub struct ValidationFeeRewardsState {
    /// Total unconverted authenticated SBD cents. Per-month credits, historical
    /// service, and individual claims live in separate protected records so this
    /// mandatory per-block custody checkpoint has constant size.
    pub pending_sbd_total: u128,
    /// Total XOR minor units reserved exclusively for these claims.
    pub reserved_xor: u128,
    /// Next allocation sequence; never reused after conversion or claims.
    pub next_allocation: u64,
    /// Next immutable claim receipt sequence.
    pub next_claim: u64,
    /// Last successfully converted block time.
    pub last_conversion_ms: Option<u64>,
    /// Last scheduled conversion attempt, including failed pool execution.
    pub last_attempt_ms: Option<u64>,
    /// Block that owns the current attempt; retries in later blocks are rate limited.
    pub last_attempt_height: u64,
    /// Honiara day containing the current conversion limit counter.
    pub conversion_day: u64,
    /// Converted SBD cents in that day.
    pub converted_today_sbd: u64,
    /// Last finalized block incorporated into the service record.
    pub service_height: u64,
}

/// Canonical protected custody key shared by execution and independent evidence.
///
/// # Errors
/// Returns an error if custody coordinates cannot be canonically encoded or their state path cannot be represented.
pub fn validation_fee_reward_state_key(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    leaf: &str,
) -> Result<StatePath, String> {
    let digest = hex::encode(Hash::new(binding.contract_address.to_string().as_bytes()).as_ref());
    let custody = (
        binding.treasury_account_id.clone(),
        binding.ds_asset_id.clone(),
        binding.xor_asset_id.clone(),
        binding.reward_pool_account_id.clone(),
    );
    let bytes = norito::to_bytes(&custody).map_err(|e| e.to_string())?;
    let scope = hex::encode(Hash::new(bytes).as_ref());
    format!("sc/{digest}/ValidationFeeRewards/{scope}/{leaf}")
        .parse()
        .map_err(|e| format!("invalid rewards state key: {e}"))
}

/// Historical service read from its protected original-month key. The fee
/// commitment authenticates this source snapshot independently of an allocation.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::validation_fee_rewards::ValidationFeeServiceSnapshot")]
pub struct ValidationFeeServiceSnapshot {
    /// Original Honiara earning-month start.
    pub earning_period_start_ms: u64,
    /// Native finalized service weights, retained after the period closes.
    pub service_blocks: BTreeMap<AccountId, u64>,
}

/// Original provider-signed observation and its consensus admission coordinates.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::validation_fee_rewards::ValidationFeeReferenceObservation"
)]
pub struct ValidationFeeReferenceObservation {
    /// Canonical signed native oracle report, including original source timestamp.
    pub observation: Observation,
    /// Block height that admitted and authenticated the signature.
    pub admitted_height: u64,
    /// Consensus timestamp of admission.
    pub admitted_at_ms: u64,
}

/// Immutable allocation receipt, kept separately from mutable accrued claims.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::validation_fee_rewards::ValidationFeeRewardAllocation")]
pub struct ValidationFeeRewardAllocation {
    /// Monotonic allocation identity scoped to the immutable custody identity.
    pub sequence: u64,
    /// Exact Parliament-enacted conversion policy used by this allocation.
    pub lifecycle_seal: [u8; 32],
    /// Original earning month, preserved when conversion is delayed.
    pub earning_period_start_ms: u64,
    /// Authenticated SBD cents consumed.
    pub sbd_minor: u64,
    /// Actual XOR minor units atomically received from the pool.
    pub xor_minor: u128,
    /// Consensus block height at conversion.
    pub converted_at_height: u64,
    /// Exact consensus conversion time.
    pub converted_at_ms: u64,
    /// Reference-derived minimum output, with execution loss applied before rounding.
    pub min_xor_minor: u128,
    /// Original signed reference reports retained with this conversion forever.
    pub reference_observations: Vec<ValidationFeeReferenceObservation>,
    /// Actual historical service counts used for this allocation.
    pub service_blocks: BTreeMap<AccountId, u64>,
    /// Exact reserved XOR shares; canonical account order resolves rounding ties.
    pub shares: BTreeMap<AccountId, u128>,
    /// Stable beneficiary identity for each unchanged historical share account.
    pub beneficiaries: BTreeMap<AccountId, AccountId>,
}

/// Immutable funded claim receipt; repeated claims cannot reuse this sequence.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::validation_fee_rewards::ValidationFeeRewardClaim")]
pub struct ValidationFeeRewardClaim {
    /// Stable original beneficiary whose reserved credit is claimed.
    pub beneficiary_id: AccountId,
    /// Immutable authorized owner revision used at claim execution.
    pub beneficiary_revision: u64,
    /// Monotonic identity in the protected custody ledger.
    pub sequence: u64,
    /// Exact claimant authenticated by the native claim instruction.
    pub account_id: AccountId,
    /// Actually transferred XOR minor units.
    pub xor_minor: u128,
    /// Native finalization height.
    pub claimed_at_height: u64,
    /// Native finalization time.
    pub claimed_at_ms: u64,
    /// Parliament lifecycle governing the claim.
    pub lifecycle_seal: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn durable_reward_state_norito_roundtrip() {
        let state = ValidationFeeRewardsState {
            pending_sbd_total: 100,
            reserved_xor: 43,
            ..Default::default()
        };
        let bytes = norito::to_bytes(&state).expect("encode");
        let decoded: ValidationFeeRewardsState = norito::decode_from_bytes(&bytes).expect("decode");
        assert_eq!(state, decoded);
    }
}

/// Immutable scheduled conversion attempt. Absence of a same-height allocation
/// proves that the attempted conversion retained its SBD credit.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::validation_fee_rewards::ValidationFeeConversionAttempt")]
pub struct ValidationFeeConversionAttempt {
    /// Exact block that published this native attempt.
    pub attempted_at_height: u64,
    /// Consensus timestamp used by source freshness and rate limits.
    pub attempted_at_ms: u64,
    /// Original completed earning month.
    pub earning_period_start_ms: u64,
    /// Authenticated SBD cents offered for conversion.
    pub sbd_minor: u64,
    /// Exact reference-based minimum XOR output.
    pub min_xor_minor: u128,
    /// Exact independently governed conversion policy identity.
    pub lifecycle_seal: [u8; 32],
    /// Original native signed provider reports retained at scheduling time.
    pub reference_observations: Vec<ValidationFeeReferenceObservation>,
}

/// Canonical exact reference-price minimum used by consensus and offline proof verification.
///
/// # Errors
/// Returns an error for invalid conversion settings, unsupported precision, a nonpositive amount, or arithmetic overflow.
pub fn reference_minimum(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    records: &[ValidationFeeReferenceObservation],
    now: u64,
    height: u64,
    sbd_minor: u64,
    xor_scale: u32,
) -> Result<Option<u128>, String> {
    if let Some(reason) = binding.invariant_error() {
        return Err(reason.to_owned());
    }
    if xor_scale > 18 || sbd_minor == 0 {
        return Err("reference minimum requires positive exact supported-scale input".into());
    }
    let mut groups: BTreeMap<(u64, Hash), Vec<u128>> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for record in records {
        let body = &record.observation.body;
        if body.feed_id != binding.reference_feed_id
            || body.feed_config_version.0 != binding.reference_feed_config_version
            || !binding
                .reference_provider_accounts
                .contains(&body.provider_id)
            || !seen.insert(body.provider_id.clone())
            || record.admitted_height >= height
        {
            continue;
        }
        let Some(source) = body.timestamp_ms else {
            continue;
        };
        if source > record.admitted_at_ms
            || record.admitted_at_ms > now
            || source > now
            || now - source > binding.max_source_age_ms
        {
            continue;
        }
        let ObservationOutcome::Value(value) = &body.outcome else {
            continue;
        };
        if value.mantissa <= 0 || value.scale > 18 || xor_scale > 18 {
            continue;
        }
        let numerator = u128::try_from(value.mantissa)
            .map_err(|_| String::from("negative reference price"))?
            .checked_mul(u128::from(sbd_minor))
            .and_then(|v| v.checked_mul(10u128.pow(xor_scale)))
            .ok_or_else(|| String::from("reference price overflow"))?;
        let denominator = 100u128
            .checked_mul(10u128.pow(value.scale))
            .ok_or_else(|| String::from("reference scale overflow"))?;
        let loss_adjusted = numerator
            .checked_mul(u128::from(10_000 - binding.max_slippage_bps))
            .ok_or_else(|| String::from("minimum conversion overflow"))?;
        let denominator = denominator
            .checked_mul(10_000)
            .ok_or_else(|| String::from("minimum denominator overflow"))?;
        // Apply the execution-loss limit to the exact reference output before
        // rounding upward. Flooring the reference first could permit >1% loss.
        let minimum = loss_adjusted / denominator + u128::from(loss_adjusted % denominator != 0);
        groups
            .entry((body.slot, body.request_hash))
            .or_default()
            .push(minimum);
    }
    let Some((_, mut values)) = groups.into_iter().rev().find(|(_, v)| v.len() >= 3) else {
        return Ok(None);
    };
    values.sort_unstable();
    let minimum = values[values.len() / 2];
    Ok((minimum > 0).then_some(minimum))
}

/// Canonical funded largest-remainder allocation used by consensus and proof verification.
///
/// # Errors
/// Returns an error for arithmetic overflow, zero total weight, or no authenticated service.
pub fn allocate(
    amount: u128,
    weights: &BTreeMap<AccountId, u64>,
) -> Result<BTreeMap<AccountId, u128>, String> {
    let total = weights
        .values()
        .try_fold(0u128, |a, b| a.checked_add(u128::from(*b)))
        .ok_or_else(|| String::from("service weight overflow"))?;
    if total == 0 {
        return Err(String::from("no authenticated validator service"));
    }
    let mut shares = BTreeMap::new();
    let mut remainders = Vec::new();
    let mut allocated = 0u128;
    for (account, weight) in weights.iter().filter(|(_, weight)| **weight > 0) {
        let numerator = amount
            .checked_mul(u128::from(*weight))
            .ok_or_else(|| String::from("reward allocation overflow"))?;
        let share = numerator / total;
        allocated = allocated
            .checked_add(share)
            .ok_or_else(|| String::from("reward sum overflow"))?;
        shares.insert(account.clone(), share);
        remainders.push((numerator % total, account.clone()));
    }
    remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let remaining = usize::try_from(amount - allocated)
        .map_err(|_| String::from("reward rounding overflow"))?;
    for (_, account) in remainders.into_iter().take(remaining) {
        if let Some(share) = shares.get_mut(&account) {
            *share += 1;
        }
    }
    Ok(shares)
}

/// Immutable alias from an account identity to its original reward beneficiary.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::validation_fee_rewards::ValidationFeeRewardBeneficiaryAlias"
)]
pub struct ValidationFeeRewardBeneficiaryAlias {
    /// Account whose historical service or claims use this beneficiary.
    pub account_id: AccountId,
    /// Original identity; never changes during recovery.
    pub beneficiary_id: AccountId,
}
/// Immutable authenticated owner revision, retained for claims before later recovery.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::validation_fee_rewards::ValidationFeeRewardBeneficiaryRevision"
)]
pub struct ValidationFeeRewardBeneficiaryRevision {
    /// Original reward beneficiary.
    pub beneficiary_id: AccountId,
    /// Monotonic owner revision; zero initializes the original owner.
    pub revision: u64,
    /// Account entitled to claim at this revision.
    pub account_id: AccountId,
    /// Previous authorized owner, absent only for initial revision zero.
    pub previous_account_id: Option<AccountId>,
    /// Block that authorized this owner revision.
    pub authorized_at_height: u64,
}
/// Canonical immutable account alias source key.
///
/// # Errors
/// Returns an error if the custody key cannot be derived or the beneficiary state path cannot be represented.
pub fn validation_fee_beneficiary_alias_key(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    account: &AccountId,
) -> Result<StatePath, String> {
    validation_fee_reward_state_key(
        binding,
        &format!(
            "BeneficiaryAlias/{}",
            hex::encode(Hash::new(account.to_string().as_bytes()).as_ref())
        ),
    )
}
/// Canonical immutable owner revision source key.
///
/// # Errors
/// Returns an error if the custody key cannot be derived or the revision state path cannot be represented.
pub fn validation_fee_beneficiary_revision_key(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    beneficiary: &AccountId,
    revision: u64,
) -> Result<StatePath, String> {
    validation_fee_reward_state_key(
        binding,
        &format!(
            "BeneficiaryHistory/{}/{revision:020}",
            hex::encode(Hash::new(beneficiary.to_string().as_bytes()).as_ref())
        ),
    )
}
