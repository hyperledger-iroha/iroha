//! Exact signed staking effects exposed to validation-fee admission.

use super::*;
use iroha_data_model::{
    isi::staking::{
        BondPublicLaneStake, ClaimPublicLaneRewards, FinalizePublicLaneUnbond,
        RecordPublicLaneRewards, RegisterPublicLaneCandidate, RegisterPublicLaneValidator,
        SlashPublicLaneValidator,
    },
    nexus::{PublicLaneMonetaryPlanV1, PublicLaneMonetaryPreconditionV1},
};

pub(super) fn monetary_staking_wire_id(instruction: &InstructionBox) -> Option<&'static str> {
    macro_rules! classify {
        ($($ty:ty),+ $(,)?) => {$(
            if instruction.as_any().downcast_ref::<$ty>().is_some() {
                return Some(<$ty>::WIRE_ID);
            }
        )+};
    }
    classify!(
        RegisterPublicLaneCandidate,
        RegisterPublicLaneValidator,
        BondPublicLaneStake,
        FinalizePublicLaneUnbond,
        SlashPublicLaneValidator,
        RecordPublicLaneRewards,
        ClaimPublicLaneRewards,
    );
    None
}

fn registration_plan(registration: &RegisterPublicLaneValidator) -> Option<&PublicLaneMonetaryPlanV1> {
    let plan = &registration.monetary_plan;
    (plan.has_canonical_shape()
        && plan.source_asset.account() == &registration.stake_account
        && plan.amount == registration.initial_stake
        && matches!(plan.precondition, PublicLaneMonetaryPreconditionV1::Registration(..)))
    .then_some(plan)
}

fn transfer_plan(instruction: &InstructionBox) -> Option<&PublicLaneMonetaryPlanV1> {
    if let Some(candidate) = instruction.as_any().downcast_ref::<RegisterPublicLaneCandidate>() {
        let plan = registration_plan(&candidate.registration)?;
        return matches!(&plan.precondition, PublicLaneMonetaryPreconditionV1::Registration(precondition) if precondition.activation_height == candidate.activation_height).then_some(plan);
    }
    if let Some(registration) = instruction.as_any().downcast_ref::<RegisterPublicLaneValidator>() {
        return registration_plan(registration);
    }
    if let Some(bond) = instruction.as_any().downcast_ref::<BondPublicLaneStake>() {
        let plan = &bond.monetary_plan;
        return (plan.has_canonical_shape()
            && plan.source_asset.account() == &bond.staker
            && plan.amount == bond.amount
            && matches!(plan.precondition, PublicLaneMonetaryPreconditionV1::Bond(..)))
        .then_some(plan);
    }
    if let Some(unbond) = instruction.as_any().downcast_ref::<FinalizePublicLaneUnbond>() {
        let plan = &unbond.monetary_plan;
        return (plan.has_canonical_shape()
            && plan.destination_asset.account() == &unbond.staker
            && matches!(plan.precondition, PublicLaneMonetaryPreconditionV1::Unbond(..)))
        .then_some(plan);
    }
    if let Some(slash) = instruction.as_any().downcast_ref::<SlashPublicLaneValidator>() {
        let plan = &slash.monetary_plan;
        return (plan.has_canonical_shape()
            && plan.amount == slash.amount
            && matches!(plan.precondition, PublicLaneMonetaryPreconditionV1::Slash(..)))
        .then_some(plan);
    }
    None
}

/// Collect only canonical signed principal transfers; native principal can never pay its own fee.
pub(super) fn collect_signed_staking_effects(
    instruction: &InstructionBox,
    context_index: usize,
    instruction_index: usize,
    collection: &mut TransferCollection,
) -> Result<(), ValidationFeeAdmissionError> {
    let instruction_wire_id = monetary_staking_wire_id(instruction)
        .expect("monetary staking disposition identifies a known instruction");
    let invalid = || ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
        context_index,
        instruction_index,
        instruction_wire_id,
    };
    let mut append = |entry_index: usize, source: &AssetId, destination: &AssetId, amount: &Quantity| {
        collection.transfers.push(AssetTransferSummary {
            context_index,
            instruction_index,
            entry_index: Some(entry_index),
            asset_definition_id: source.definition().clone(),
            source_account_id: source.account().clone(),
            destination_account_id: destination.account().clone(),
            amount: amount.clone(),
            explicit_fee_eligible: false,
        });
    };
    if let Some(rewards) = instruction.as_any().downcast_ref::<RecordPublicLaneRewards>() {
        let total = rewards.shares.iter().try_fold(Quantity::zero(), |sum, share| {
            if share.amount.is_zero() { return None; }
            sum.checked_add(&share.amount).ok()
        });
        if rewards.total_reward.is_zero() || total.as_ref() != Some(&rewards.total_reward) {
            return Err(invalid());
        }
        // Recording signed entitlements encumbers existing funds. It is not a
        // transfer principal under PerQualifyingTransferInstruction.
        return Ok(());
    }
    if let Some(claim) = instruction.as_any().downcast_ref::<ClaimPublicLaneRewards>() {
        if !claim.claim_plan.has_canonical_shape(&claim.account) {
            return Err(invalid());
        }
        for (entry, source) in claim.claim_plan.sources.iter().enumerate() {
            if !source.payout.is_zero() {
                append(entry, &source.source_asset, &source.destination_asset, &source.payout);
            }
        }
        return Ok(());
    }
    let plan = transfer_plan(instruction).ok_or_else(invalid)?;
    append(0, &plan.source_asset, &plan.destination_asset, &plan.amount);
    Ok(())
}

/// Opaque code cannot create signed staking authority by manufacturing a plan at runtime.
/// This also covers reserve-only reward recording and zero-payout accrual processing.
pub(super) fn opaque_staking_policy_asset_effect(
    instruction: &InstructionBox,
    fee_asset: &AssetDefinitionId,
) -> Option<&'static str> {
    let wire_id = monetary_staking_wire_id(instruction)?;
    let touches = if let Some(rewards) = instruction.as_any().downcast_ref::<RecordPublicLaneRewards>() {
        rewards.reward_asset.definition() == fee_asset
    } else if let Some(claim) = instruction.as_any().downcast_ref::<ClaimPublicLaneRewards>() {
        claim.claim_plan.sources.iter().any(|source| source.source_asset.definition() == fee_asset)
    } else {
        transfer_plan(instruction).is_some_and(|plan| plan.source_asset.definition() == fee_asset)
    };
    touches.then_some(wire_id)
}
