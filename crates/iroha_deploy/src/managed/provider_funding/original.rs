//! Immutable economic selection before the first generated-provider funding child.
//! Retained rows are original intent; decoding them supplies no current native authority.

use super::*;
use iroha_crypto::HashOf;
use iroha_data_model::sorafs::{
    pricing::PricingScheduleRecord,
    reserve::{ReserveProviderAccountV1, history::validate_provider_record},
};
use iroha_fs::{PrivateDirectory, PublishMode};
use provider_economics::{
    EconomicAmounts, MAX_PARTITION_BYTES, MAX_POLICY_BYTES, MAX_PRICING_BYTES,
};

const MAX_ORIGINAL_BYTES: usize = MAX_CHECKPOINT_BYTES + 192 * 1024;

#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::provider_funding::Original")]
pub(super) struct Original {
    pub network_id: iroha_data_model::NetworkId,
    pub policy: ReserveAuthorityPolicyV1,
    pub partition: ReserveProviderAccountV1,
    pub pricing: PricingScheduleRecord,
    pub observed_block_time_ms: u64,
    pub economics: EconomicAmounts,
    pub movement_id: [u8; 32],
    pub fees: Fees,
    pub checkpoint: Vec<u8>,
}

impl Original {
    /// Retain only a fully selected original cut, with no preexisting credit or capacity.
    pub(super) fn select(
        plan: &RetainedProviderServicePlan,
        policy: &ReserveAuthorityPolicyV1,
        state: &VerifiedReserveAccountStateV1,
        verifier: &FinalityVerifier,
        fees: Fees,
    ) -> Result<Self> {
        let partition = state
            .current()
            .ok_or_else(|| invalid("automatic funding requires its native reserve partition"))?;
        encode(policy, MAX_POLICY_BYTES)?;
        encode(partition, MAX_PARTITION_BYTES)?;
        encode(state.pricing(), MAX_PRICING_BYTES)?;
        if state.policy().policy != *policy
            || state.network_id() != plan.network_id()
            || verifier.checkpoint().network_id() != plan.network_id()
            || state.credit().is_some()
            || state.capacity().is_some()
            || partition.pending_movements != 0
            || partition.open_appeals != 0
            || !partition.debt_principal.is_zero()
            || !partition.accrued_interest.is_zero()
        {
            return Err(invalid("automatic initial funding predecessor changed"));
        }
        let economics = provider_economics::derive(plan, state)?;
        let tip = verifier
            .verified_tip()
            .map_err(|_| invalid("automatic funding needs selected certified state"))?;
        if state.height() != tip.height()
            || state.context_id() != tip.context_id()
            || state.block_time_ms() != tip.header().creation_time_ms
        {
            return Err(invalid("economic cut differs from selected checkpoint"));
        }
        // The nonce is retained before any child. Reopen reads it instead of drawing again.
        let movement_id = rand::random();
        let original = Self {
            network_id: plan.network_id(),
            policy: policy.clone(),
            partition: partition.clone(),
            pricing: state.pricing().clone(),
            observed_block_time_ms: state.block_time_ms(),
            economics,
            movement_id,
            fees,
            checkpoint: checkpoint_bytes(verifier)?,
        };
        original.validate(plan)?;
        Ok(original)
    }

    pub(super) fn validate(&self, plan: &RetainedProviderServicePlan) -> Result<()> {
        self.fees.validate()?;
        encode(&self.policy, MAX_POLICY_BYTES)?;
        encode(&self.partition, MAX_PARTITION_BYTES)?;
        encode(&self.pricing, MAX_PRICING_BYTES)?;
        encode(&self.economics, 16 * 1024)?;
        self.policy
            .validate()
            .map_err(|_| invalid("invalid original funding policy"))?;
        validate_provider_record(&self.partition, plan.provider_id())
            .map_err(|_| invalid("invalid original funding partition"))?;
        if self.movement_id == [0; 32]
            || self.network_id != plan.network_id()
            || self.partition.pending_movements != 0
            || self.partition.open_appeals != 0
            || !self.partition.debt_principal.is_zero()
            || !self.partition.accrued_interest.is_zero()
            || self.checkpoint.is_empty()
            || self.checkpoint.len() > MAX_CHECKPOINT_BYTES
            || self.economics
                != provider_economics::derive_retained(
                    plan,
                    &self.policy,
                    &self.partition,
                    None,
                    &self.pricing,
                    self.observed_block_time_ms,
                )?
        {
            return Err(invalid("original automatic funding selection changed"));
        }
        Ok(())
    }

    pub(super) fn matches(
        &self,
        plan: &RetainedProviderServicePlan,
        policy: &ReserveAuthorityPolicyV1,
        fees: &Fees,
    ) -> Result<()> {
        self.validate(plan)?;
        encode(policy, MAX_POLICY_BYTES)?;
        if &self.policy != policy {
            return Err(invalid(
                "original automatic funding policy cannot be replaced",
            ));
        }
        fees.validate()?;
        if &self.fees != fees {
            return Err(invalid("original funding fee authorization changed"));
        }
        Ok(())
    }

    pub(super) fn top_up(&self) -> Option<ManagedReserveTopUpIntent> {
        (!self.economics.top_up.is_zero()).then(|| ManagedReserveTopUpIntent {
            policy: self.policy.clone(),
            partition: self.partition.clone(),
            expected_provider_revision: self.partition.revision,
            movement_id: self.movement_id,
            amount: self.economics.top_up.clone(),
        })
    }

    pub(super) fn digest(&self) -> Result<HashOf<Self>> {
        encode(self, MAX_ORIGINAL_BYTES)?;
        HashOf::try_new(self)
            .map_err(std::io::Error::other)
            .map_err(Into::into)
    }
}

pub(super) fn read(
    directory: &PrivateDirectory,
    plan: &RetainedProviderServicePlan,
) -> Result<Option<Original>> {
    let names = directory.entries(3)?;
    if names.iter().any(|name| {
        ![
            "economic-original.nrt",
            "approval-selection.nrt",
            "credit-selection.nrt",
        ]
        .iter()
        .any(|allowed| name == *allowed)
    }) {
        return Err(invalid("funding economics contains unknown material"));
    }
    let Some(bytes) = read_optional(directory, "economic-original.nrt", MAX_ORIGINAL_BYTES)? else {
        require_empty(directory)?;
        return Ok(None);
    };
    let original: Original = norito::decode_canonical_with_limits(
        &bytes,
        norito::DecodeLimits::new(
            MAX_CHECKPOINT_BYTES,
            MAX_ORIGINAL_BYTES,
            MAX_ORIGINAL_BYTES,
            96 * 1024 * 1024,
            40,
        ),
    )
    .map_err(std::io::Error::other)?;
    original.validate(plan)?;
    Ok(Some(original))
}

pub(super) fn publish(
    directory: &PrivateDirectory,
    plan: &RetainedProviderServicePlan,
    original: &Original,
) -> Result<()> {
    original.validate(plan)?;
    directory.write_atomic(
        "economic-original.nrt",
        &encode(original, MAX_ORIGINAL_BYTES)?,
        PublishMode::CreateNew,
    )?;
    Ok(())
}
