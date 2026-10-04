//! Bounded automatic economics from the original generated plan and one authenticated native cut.
//! Derived quantities are an immutable intent, not funding, eligibility or service authority.

use super::{
    Result,
    native_operation::{encode, invalid},
};
use crate::localnet::service_authorities::RetainedProviderServicePlan;
use iroha_data_model::sorafs::{
    pricing::{PricingScheduleRecord, ProviderCreditRecord},
    reserve::{
        ReserveAuthorityPolicyV1, ReserveProviderAccountV1, ReserveProviderTermsV1,
        account_proof::VerifiedReserveAccountStateV1, history::validate_provider_record,
    },
};
use iroha_primitives::numeric::{Quantity, XorQuantity};

pub(super) const MAX_POLICY_BYTES: usize = 32 * 1024;
pub(super) const MAX_PARTITION_BYTES: usize = 32 * 1024;
pub(super) const MAX_CREDIT_BYTES: usize = 64 * 1024;
pub(super) const MAX_PRICING_BYTES: usize = 64 * 1024;

/// Exact derived amounts at the retained original observation. Decoding this value grants no
/// authority: the original inputs must be retained, bounded and recomputed before use.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::provider_economics::EconomicAmounts")]
pub(super) struct EconomicAmounts {
    pub observed_epoch: u64,
    pub onboarding_epoch: u64,
    pub price_bond: Quantity,
    pub expected_settlement: Quantity,
    /// Explicitly one storage settlement window, not money or unbounded egress credit.
    pub available_credit: Quantity,
    pub reserve_requirement: XorQuantity,
    pub target_reserve: XorQuantity,
    /// Actual provider principal needed; no reserve borrowing is derived.
    pub top_up: XorQuantity,
}

/// Derive automatic setup amounts from an opaque original profile and verified same-cut rows.
/// The caller retains the complete original inputs and this result before creating paid work.
pub(super) fn derive(
    plan: &RetainedProviderServicePlan,
    state: &VerifiedReserveAccountStateV1,
) -> Result<EconomicAmounts> {
    if plan.network_id() != state.network_id()
        || plan.provider_id() != state.provider_id()
        || &plan.reserve_terms().provider_account != state.owner()
    {
        return Err(invalid(
            "economic observation differs from original generated provider",
        ));
    }
    let partition = state
        .current()
        .ok_or_else(|| invalid("economic observation has no reserve partition"))?;
    derive_retained(
        plan,
        &state.policy().policy,
        partition,
        state.credit(),
        state.pricing(),
        state.block_time_ms(),
    )
}

/// Recompute retained claims without upgrading those claims to independent current evidence.
pub(super) fn derive_retained(
    plan: &RetainedProviderServicePlan,
    policy: &ReserveAuthorityPolicyV1,
    partition: &ReserveProviderAccountV1,
    credit: Option<&ProviderCreditRecord>,
    pricing: &PricingScheduleRecord,
    block_time_ms: u64,
) -> Result<EconomicAmounts> {
    if pricing != plan.pricing() || &partition.terms != plan.reserve_terms() {
        return Err(invalid(
            "original economic pricing or underwriting selection changed",
        ));
    }
    calculate(
        plan.reserve_terms(),
        &plan.declaration().stake.stake_amount,
        policy,
        partition,
        credit,
        pricing,
        block_time_ms,
    )
}

// Sole arithmetic implementation. Direct unit inputs are explicitly claims, never proof fixtures.
fn calculate(
    terms: &ReserveProviderTermsV1,
    admitted_stake: &XorQuantity,
    policy: &ReserveAuthorityPolicyV1,
    partition: &ReserveProviderAccountV1,
    credit: Option<&ProviderCreditRecord>,
    pricing: &PricingScheduleRecord,
    block_time_ms: u64,
) -> Result<EconomicAmounts> {
    encode(policy, MAX_POLICY_BYTES)?;
    encode(partition, MAX_PARTITION_BYTES)?;
    encode(pricing, MAX_PRICING_BYTES)?;
    encode(admitted_stake, 4096)?;
    if let Some(credit) = credit {
        encode(credit, MAX_CREDIT_BYTES)?;
    }
    policy
        .validate()
        .map_err(|_| invalid("invalid economic reserve policy"))?;
    validate_provider_record(partition, terms.provider_id)
        .map_err(|_| invalid("invalid economic reserve partition"))?;
    pricing
        .validate()
        .map_err(|_| invalid("invalid economic pricing schedule"))?;
    if &partition.terms != terms || admitted_stake.is_zero() || block_time_ms < 1000 {
        return Err(invalid(
            "economic plan requires exact terms, positive admitted stake and native time",
        ));
    }
    let observed_epoch = block_time_ms / 1000;
    let onboarding_epoch = credit.map_or(observed_epoch, |record| record.onboarding_epoch);
    let slashed = match credit {
        Some(record) => {
            if record.provider_id != terms.provider_id {
                return Err(invalid("economic credit belongs to another provider"));
            }
            XorQuantity::try_from_quantity(record.slashed.clone()).map_err(std::io::Error::other)?
        }
        None => XorQuantity::zero(),
    };
    let price_bond = pricing
        .required_collateral(
            terms.storage_class,
            terms.capacity_gib,
            onboarding_epoch,
            observed_epoch,
        )
        .map_err(std::io::Error::other)?;
    let expected_settlement = pricing
        .expected_settlement_storage_charge(terms.storage_class, terms.capacity_gib)
        .map_err(std::io::Error::other)?;
    let bond = XorQuantity::try_from_quantity(price_bond.clone()).map_err(std::io::Error::other)?;
    let required_backing = partition
        .debt_principal
        .checked_add(&slashed)
        .and_then(|amount| amount.checked_add(&maximum(&bond, admitted_stake)))
        .map_err(std::io::Error::other)?;
    let quote = policy
        .economics
        .quote(
            terms.storage_class,
            terms.capacity_gib,
            terms.duration,
            terms.tier,
            partition.reserve_balance.clone(),
        )
        .map_err(std::io::Error::other)?;
    let target_reserve = maximum(&quote.reserve_requirement, &required_backing);
    let top_up = if target_reserve > partition.reserve_balance {
        target_reserve
            .checked_sub(&partition.reserve_balance)
            .map_err(std::io::Error::other)?
    } else {
        XorQuantity::zero()
    };
    Ok(EconomicAmounts {
        observed_epoch,
        onboarding_epoch,
        price_bond,
        available_credit: expected_settlement.clone(),
        expected_settlement,
        reserve_requirement: quote.reserve_requirement,
        target_reserve,
        top_up,
    })
}

fn maximum(left: &XorQuantity, right: &XorQuantity) -> XorQuantity {
    if left >= right {
        left.clone()
    } else {
        right.clone()
    }
}

#[cfg(test)]
#[path = "provider_economics/tests.rs"]
mod tests;
