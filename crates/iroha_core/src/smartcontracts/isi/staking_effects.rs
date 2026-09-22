//! Signed exact monetary effects for native public-lane staking instructions.

use super::*;
use iroha_data_model::nexus::{
    PublicLaneMonetaryPlanV1, PublicLaneMonetaryPreconditionV1, PublicLaneMonetaryScopeV1,
    PublicLaneRewardClaimPlanV1, PublicLaneRewardClaimStateV1,
    public_lane_reward_record_commitment,
};

/// Restrict a signed plan to this network and one governed epoch of validity.
pub(super) fn validate_plan_context(
    state_transaction: &StateTransaction<'_, '_>,
    network_scope: &PublicLaneMonetaryScopeV1,
    valid_until_height: u64,
) -> Result<(), Error> {
    let scope_matches = match network_scope {
        PublicLaneMonetaryScopeV1::Genesis => {
            state_transaction._curr_block.is_genesis() && state_transaction.block_hashes.is_empty()
        }
        PublicLaneMonetaryScopeV1::Network(network_id) => network_id == state_transaction.network_id(),
    };
    if !scope_matches {
        return Err(Error::InvariantViolation(
            "staking monetary plan does not match this authenticated genesis or network scope".into(),
        ));
    }
    let parameters = state_transaction
        .world
        .sumeragi_npos_parameters()
        .ok_or_else(|| {
            Error::InvariantViolation(
                "staking monetary plans require committed NPoS epoch parameters".into(),
            )
        })?;
    parameters.validate().map_err(|error| {
        Error::InvariantViolation(format!("invalid staking monetary plan epoch parameters: {error}").into())
    })?;
    let height = state_transaction.block_height();
    let latest = height
        .checked_add(parameters.epoch_length_blocks.get())
        .ok_or_else(|| {
            Error::InvariantViolation("staking monetary plan validity height overflow".into())
        })?;
    if valid_until_height < height || valid_until_height > latest {
        return Err(Error::InvariantViolation(
            "staking monetary plan must expire within the current committed epoch-length window"
                .into(),
        ));
    }
    Ok(())
}

/// Compare every signed movement and custody direction with its freshly resolved operation.
pub(super) fn verify_transfer_plan(
    state_transaction: &StateTransaction<'_, '_>,
    plan: &PublicLaneMonetaryPlanV1,
    source_asset: &AssetId,
    destination_asset: &AssetId,
    amount: &Quantity,
    precondition: &PublicLaneMonetaryPreconditionV1,
) -> Result<(), Error> {
    validate_plan_context(state_transaction, &plan.network_scope, plan.valid_until_height)?;
    if !plan.has_canonical_shape()
        || &plan.source_asset != source_asset
        || &plan.destination_asset != destination_asset
        || &plan.amount != amount
        || &plan.precondition != precondition
    {
        return Err(Error::InvariantViolation(
            "staking monetary plan does not match its exact current transfer and custody state"
                .into(),
        ));
    }
    Ok(())
}

/// Fully checked point updates and exact payouts for one bounded claim.
pub(super) struct PreparedRewardClaim {
    /// Cursor after processing the exact signed prefix.
    pub(super) state_after: Option<PublicLaneRewardClaimStateV1>,
    /// Remaining unpaid accrual for each touched exact source; zero removes its row.
    pub(super) accrued_updates: Vec<(AssetId, Quantity)>,
    /// Positive actual transfers, already in canonical exact-source order.
    pub(super) payouts: Vec<(AssetId, AssetId, Quantity)>,
}

/// Recompute the complete bounded claim against its exact retained state.
///
/// There is one range lookup followed by at most 64 record reads and 64 source
/// point reads. Existing accrual sources outside this signed plan are untouched.
pub(super) fn prepare_reward_claim(
    state_transaction: &StateTransaction<'_, '_>,
    lane_id: LaneId,
    recipient: &AccountId,
    plan: &PublicLaneRewardClaimPlanV1,
) -> Result<PreparedRewardClaim, Error> {
    validate_plan_context(state_transaction, &plan.network_scope, plan.valid_until_height)?;
    if !plan.has_canonical_shape(recipient) {
        return Err(Error::InvariantViolation(
            "reward claim plan is not bounded and canonical".into(),
        ));
    }
    let world = &state_transaction.world;
    let claim_key = (lane_id, recipient.clone());
    if world.public_lane_reward_claims.get(&claim_key) != plan.expected_state.as_ref() {
        return Err(Error::InvariantViolation(
            "reward claim processing cursor changed after signing".into(),
        ));
    }
    let mut accrued = BTreeMap::new();
    for source in &plan.sources {
        let key = (lane_id, recipient.clone(), source.source_asset.clone());
        if world.public_lane_reward_accruals.get(&key) != source.expected_accrued.as_ref() {
            return Err(Error::InvariantViolation(
                "reward claim source accrual changed after signing".into(),
            ));
        }
        accrued.insert(
            source.source_asset.clone(),
            source.expected_accrued.clone().unwrap_or_else(Quantity::zero),
        );
    }
    let lower = plan
        .expected_state
        .as_ref()
        .and_then(|state| state.through_epoch)
        .map_or(std::ops::Bound::Included((lane_id, 0)), |epoch| {
            std::ops::Bound::Excluded((lane_id, epoch))
        });
    let mut records = world.public_lane_rewards.range((
        lower,
        std::ops::Bound::Included((lane_id, u64::MAX)),
    ));
    let mut touched_sources = std::collections::BTreeSet::new();
    let mut state_after = plan.expected_state.clone();
    for reference in &plan.records {
        let (key, record) = records.next().ok_or_else(|| {
            Error::InvariantViolation("reward claim references a missing reward record".into())
        })?;
        if !public_lane_reward_record_matches_key(key, record) {
            return Err(Error::InvariantViolation(
                "reward claim contains a non-canonical reward record".into(),
            ));
        }
        let record_hash = public_lane_reward_record_commitment(record).map_err(|error| {
            Error::InvariantViolation(format!("reward record commitment failed: {error}").into())
        })?;
        if key.1 != reference.epoch || record_hash != reference.record_hash {
            return Err(Error::InvariantViolation(
                "reward claim must bind the exact consecutive reward records after its cursor".into(),
            ));
        }
        touched_sources.insert(record.asset.clone());
        let source_accrued = accrued.get_mut(&record.asset).ok_or_else(|| {
            Error::InvariantViolation("reward claim omits a processed record's custody source".into())
        })?;
        for share in record.shares.iter().filter(|share| &share.account == recipient) {
            *source_accrued = quantity_add(source_accrued.clone(), share.amount.clone())?;
        }
        state_after = Some(PublicLaneRewardClaimStateV1 {
            through_epoch: Some(key.1),
        });
    }
    let dust_threshold = &state_transaction.nexus.staking.reward_dust_threshold;
    let mut accrued_updates = Vec::with_capacity(plan.sources.len());
    let mut payouts = Vec::with_capacity(plan.sources.len());
    for source in &plan.sources {
        if !touched_sources.contains(&source.source_asset) && source.expected_accrued.is_none() {
            return Err(Error::InvariantViolation(
                "reward claim includes a source with neither a processed record nor retained accrual".into(),
            ));
        }
        let available = accrued.remove(&source.source_asset).expect("every signed source was seeded");
        let expected_payout = if &available >= dust_threshold {
            available.clone()
        } else {
            Quantity::zero()
        };
        if source.payout != expected_payout {
            return Err(Error::InvariantViolation(
                "reward claim payout does not match its exact accrued entitlement and dust threshold".into(),
            ));
        }
        accrued_updates.push((
            source.source_asset.clone(),
            quantity_sub(available, expected_payout.clone())?,
        ));
        if !expected_payout.is_zero() {
            payouts.push((
                source.source_asset.clone(),
                source.destination_asset.clone(),
                expected_payout,
            ));
        }
    }
    Ok(PreparedRewardClaim { state_after, accrued_updates, payouts })
}

/// One-shot payout capability minted only after exact signed claim recomputation.
pub(in crate::smartcontracts::isi) struct VerifiedStakingRewardPayouts {
    recipient: AccountId,
    binding: Vec<u8>,
    payouts: Vec<(AssetId, AssetId, Quantity)>,
}

impl VerifiedStakingRewardPayouts {
    pub(super) fn new(
        recipient: AccountId,
        binding: Vec<u8>,
        payouts: Vec<(AssetId, AssetId, Quantity)>,
    ) -> Self {
        Self { recipient, binding, payouts }
    }

    /// Consume the exact authorized payout set without exposing a reusable constructor.
    pub(in crate::smartcontracts::isi) fn into_parts(
        self,
    ) -> (AccountId, Vec<u8>, Vec<(AssetId, AssetId, Quantity)>) {
        (self.recipient, self.binding, self.payouts)
    }
}

/// Apply a verified claim with one atomic asset batch and exact reserve rollback.
pub(super) fn execute_reward_claim(
    instruction: ClaimPublicLaneRewards,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    if &instruction.account != authority {
        return Err(Error::InvariantViolation(
            "reward claims must be submitted by the recipient account".into(),
        ));
    }
    let prepared = prepare_reward_claim(
        state_transaction,
        instruction.lane_id,
        &instruction.account,
        &instruction.claim_plan,
    )?;
    let binding = norito::encode_canonical(&instruction).map_err(|error| {
        Error::InvariantViolation(format!("reward claim monetary binding failed: {error}").into())
    })?;
    let mut reserve_changes = Vec::with_capacity(prepared.payouts.len());
    for (source, _, amount) in &prepared.payouts {
        let before = state_transaction
            .world
            .public_lane_reward_reserves
            .get(source)
            .cloned()
            .ok_or_else(|| {
                Error::InvariantViolation("reward claim is missing its retained reserve".into())
            })?;
        let after = quantity_sub(before.clone(), amount.clone())?;
        reserve_changes.push((source.clone(), before, after));
    }
    // Only these exact signed obligations are released for the central debit
    // preflight. Every other reward and stake liability remains reserved.
    for (source, _, after) in &reserve_changes {
        if after.is_zero() {
            state_transaction.world.public_lane_reward_reserves.remove(source.clone());
        } else {
            state_transaction.world.public_lane_reward_reserves.insert(source.clone(), after.clone());
        }
    }
    let capability = VerifiedStakingRewardPayouts::new(
        instruction.account.clone(),
        binding,
        prepared.payouts,
    );
    if let Err(error) = crate::smartcontracts::isi::asset::isi::execute_verified_staking_reward_payouts(
        state_transaction,
        capability,
    ) {
        for (source, before, _) in reserve_changes {
            state_transaction.world.public_lane_reward_reserves.insert(source, before);
        }
        return Err(error);
    }
    if let Some(after) = prepared.state_after {
        state_transaction.world.public_lane_reward_claims.insert(
            (instruction.lane_id, instruction.account.clone()),
            after,
        );
    }
    for (source, after) in prepared.accrued_updates {
        let key = (instruction.lane_id, instruction.account.clone(), source);
        if after.is_zero() {
            state_transaction.world.public_lane_reward_accruals.remove(key);
        } else {
            state_transaction.world.public_lane_reward_accruals.insert(key, after);
        }
    }
    Ok(())
}
