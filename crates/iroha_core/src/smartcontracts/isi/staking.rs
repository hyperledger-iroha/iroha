//! Public lane staking instruction handlers (NX-9).

use std::{collections::BTreeMap, time::Duration};

use iroha_data_model::{
    asset::{Asset, AssetDefinitionId, AssetId},
    isi::{
        error::{InstructionExecutionError as Error, InvalidParameterError, MathError},
        staking::{
            BondPublicLaneStake, CancelConsensusEvidencePenalty, ClaimPublicLaneRewards,
            FinalizePublicLaneUnbond, RecordPublicLaneRewards, RegisterPublicLaneValidator,
            SchedulePublicLaneUnbond, SlashPublicLaneValidator,
        },
    },
    metadata::Metadata,
    nexus::{
        LaneId, PublicLaneRewardRecord, PublicLaneRewardRole, PublicLaneRewardShare,
        PublicLaneStakeShare, PublicLaneUnbonding, PublicLaneValidatorRecord,
        PublicLaneValidatorStatus,
    },
    peer::PeerId,
    prelude::AccountId,
};
use iroha_primitives::{BigInt, numeric::Numeric};

use super::prelude::*;
use crate::{
    smartcontracts::isi::asset::isi::assert_numeric_spec_with,
    state::{ConsensusKeyGate, WorldReadOnly, WorldTransaction, peer_consensus_key_gate},
    sumeragi::status as sumeragi_status,
    telemetry::StateTelemetry,
};

use crate::sumeragi::evidence::evidence_key;

fn current_epoch(block_height: u64, epoch_length_blocks: u64) -> Result<u64, Error> {
    if epoch_length_blocks == 0 {
        return Err(Error::InvariantViolation(
            "epoch_length_blocks must be greater than zero".into(),
        ));
    }
    Ok(block_height.saturating_sub(1) / epoch_length_blocks)
}

fn ensure_lane_allows_staking(
    state_transaction: &StateTransaction<'_, '_>,
    lane_id: LaneId,
    context: &str,
) -> Result<(), Error> {
    if !matches!(
        state_transaction.lane_validator_mode(lane_id),
        iroha_config::parameters::actual::LaneValidatorMode::StakeElected
    ) {
        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_public_lane_validator_reject("lane_mode_admin");
        return Err(Error::InvalidParameter(
            InvalidParameterError::SmartContract(format!(
                "{context} rejected: lane {lane_id} is admin-managed (validator_mode={})",
                state_transaction.lane_validator_mode(lane_id).as_str()
            )),
        ));
    }

    Ok(())
}

impl Execute for RegisterPublicLaneValidator {
    #[iroha_logger::log(
        name = "register_public_lane_validator",
        skip_all,
        fields(lane_id = %self.lane_id, validator = %self.validator)
    )]
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "register_public_lane_validator",
        )?;
        finalize_validator_lifecycle(state_transaction)?;
        ensure_validator_peer_registered(state_transaction, self.lane_id, &self.validator)?;

        if self.initial_stake.is_zero() {
            return Err(Error::InvariantViolation(
                "initial stake must be greater than zero".into(),
            ));
        }
        let meets_min = meets_min_stake(
            &self.initial_stake,
            state_transaction.nexus.staking.min_validator_stake,
        )?;
        if !meets_min {
            return Err(Error::InvariantViolation(
                "initial stake below minimum configured amount".into(),
            ));
        }
        let stake_ctx = stake_context(
            &state_transaction.world,
            &state_transaction.nexus.staking,
            &self.stake_account,
            None,
        )?;
        assert_stake_amount_matches_spec(
            state_transaction,
            &stake_ctx.asset_definition,
            &self.initial_stake,
        )?;
        let existing = state_transaction
            .world
            .public_lane_validators
            .iter()
            .filter(|((lane, _), record)| {
                *lane == self.lane_id && !matches!(record.status, PublicLaneValidatorStatus::Exited)
            })
            .count();
        let max_validators = usize::try_from(state_transaction.nexus.staking.max_validators.get())
            .unwrap_or(usize::MAX);
        if existing >= max_validators {
            return Err(Error::InvariantViolation(
                "lane reached maximum validator capacity".into(),
            ));
        }

        let validator_key = validator_storage_key(self.lane_id, &self.validator);
        if let Some(existing) = state_transaction
            .world
            .public_lane_validators
            .get(&validator_key)
        {
            if !matches!(existing.status, PublicLaneValidatorStatus::Exited) {
                return Err(Error::InvariantViolation(
                    "validator already registered for lane".into(),
                ));
            }
            remove_all_shares_for_validator(state_transaction, self.lane_id, &self.validator);
            let removal_key = validator_key.clone();
            state_transaction
                .world
                .public_lane_validators
                .remove(removal_key);
        }

        let initial_stake = self.initial_stake.clone();
        state_transaction
            .world
            .withdraw_numeric_asset(&stake_ctx.staker_asset, &initial_stake)?;
        state_transaction
            .world
            .deposit_numeric_asset(&stake_ctx.escrow_asset, &initial_stake)?;

        let block_height = state_transaction.block_height();
        let epoch_length = state_transaction
            .world
            .sumeragi_npos_parameters()
            .map_or(
                iroha_config::parameters::defaults::sumeragi::EPOCH_LENGTH_BLOCKS,
                |params| params.epoch_length_blocks,
            )
            .max(1);
        let current_epoch = current_epoch(block_height, epoch_length)?;
        let pending_activation_epoch = current_epoch.saturating_add(1);
        let pending_status = PublicLaneValidatorStatus::PendingActivation(pending_activation_epoch);
        let record = PublicLaneValidatorRecord {
            lane_id: self.lane_id,
            validator: self.validator.clone(),
            stake_account: self.stake_account.clone(),
            total_stake: initial_stake.clone(),
            self_stake: initial_stake.clone(),
            metadata: self.metadata.clone(),
            status: pending_status.clone(),
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        };
        state_transaction
            .world
            .public_lane_validators
            .insert(validator_key, record);

        let share = PublicLaneStakeShare {
            lane_id: self.lane_id,
            validator: self.validator.clone(),
            staker: self.stake_account.clone(),
            bonded: initial_stake.clone(),
            pending_unbonds: BTreeMap::new(),
            metadata: Metadata::default(),
        };
        state_transaction.world.public_lane_stake_shares.insert(
            stake_key(self.lane_id, &self.validator, &self.stake_account),
            share,
        );

        sumeragi_status::record_public_lane_bonded_delta(self.lane_id, &initial_stake, true);

        #[cfg(feature = "telemetry")]
        {
            state_transaction
                .telemetry
                .record_public_lane_validator_status(self.lane_id, None, &pending_status);
            state_transaction
                .telemetry
                .increase_public_lane_bonded(self.lane_id, &initial_stake);
        }

        Ok(())
    }
}

impl Execute for ActivatePublicLaneValidator {
    #[iroha_logger::log(
        name = "activate_public_lane_validator",
        skip_all,
        fields(lane_id = %self.lane_id, validator = %self.validator)
    )]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "activate_public_lane_validator",
        )?;
        finalize_validator_lifecycle(state_transaction)?;
        ensure_validator_peer_registered(state_transaction, self.lane_id, &self.validator)?;
        let block_height = state_transaction.block_height();
        let epoch_length = state_transaction
            .world
            .sumeragi_npos_parameters()
            .map_or(
                iroha_config::parameters::defaults::sumeragi::EPOCH_LENGTH_BLOCKS,
                |params| params.epoch_length_blocks,
            )
            .max(1);
        let current_epoch = current_epoch(block_height, epoch_length)?;
        let validator_key = validator_storage_key(self.lane_id, &self.validator);
        let validator = state_transaction
            .world
            .public_lane_validators
            .get_mut(&validator_key)
            .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;
        let previous_status = validator.status.clone();
        match previous_status {
            PublicLaneValidatorStatus::PendingActivation(pending_epoch) => {
                if current_epoch < pending_epoch {
                    return Err(Error::InvariantViolation(
                        "validator activation epoch not reached".into(),
                    ));
                }
                if let Some(prev_epoch) = validator.activation_epoch {
                    if current_epoch < prev_epoch {
                        return Err(Error::InvariantViolation(
                            "activation epoch would regress".into(),
                        ));
                    }
                }
                let activation_epoch = current_epoch.max(pending_epoch);
                validator.status = PublicLaneValidatorStatus::Active;
                validator.activation_epoch = Some(activation_epoch);
                validator.activation_height = Some(block_height);
            }
            PublicLaneValidatorStatus::Active => return Ok(()),
            _ => {
                return Err(Error::InvariantViolation(
                    "validator not pending activation".into(),
                ));
            }
        }

        #[cfg(feature = "telemetry")]
        {
            state_transaction
                .telemetry
                .record_public_lane_validator_status(
                    self.lane_id,
                    Some(&previous_status),
                    &validator.status,
                );
            state_transaction
                .telemetry
                .record_public_lane_validator_activation(self.lane_id, current_epoch);
        }

        Ok(())
    }
}

impl Execute for ExitPublicLaneValidator {
    #[iroha_logger::log(
        name = "exit_public_lane_validator",
        skip_all,
        fields(lane_id = %self.lane_id, validator = %self.validator, release_at_ms = self.release_at_ms)
    )]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "exit_public_lane_validator",
        )?;
        let now_ms = state_transaction.block_unix_timestamp_ms();
        if self.release_at_ms < now_ms {
            return Err(Error::InvariantViolation(
                "release_at_ms must not precede current block".into(),
            ));
        }
        let validator_key = validator_storage_key(self.lane_id, &self.validator);
        let mut record = state_transaction
            .world
            .public_lane_validators
            .get(&validator_key)
            .cloned()
            .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;
        #[cfg(feature = "telemetry")]
        let previous_status = record.status.clone();
        match record.status {
            PublicLaneValidatorStatus::PendingActivation(_)
            | PublicLaneValidatorStatus::Active
            | PublicLaneValidatorStatus::Jailed(_)
            | PublicLaneValidatorStatus::Slashed(_) => {
                if self.release_at_ms == now_ms {
                    record.status = PublicLaneValidatorStatus::Exited;
                } else {
                    record.status = PublicLaneValidatorStatus::Exiting(self.release_at_ms);
                }
            }
            PublicLaneValidatorStatus::Exiting(at) => {
                if now_ms >= at {
                    record.status = PublicLaneValidatorStatus::Exited;
                } else {
                    return Err(Error::InvariantViolation(
                        "validator already exiting with a future release time".into(),
                    ));
                }
            }
            PublicLaneValidatorStatus::Exited => return Ok(()),
        }

        state_transaction
            .world
            .public_lane_validators
            .insert(validator_key, record.clone());

        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_public_lane_validator_status(
                self.lane_id,
                Some(&previous_status),
                &record.status,
            );

        Ok(())
    }
}

impl Execute for BondPublicLaneStake {
    #[iroha_logger::log(
        name = "bond_public_lane_stake",
        skip_all,
        fields(lane_id = %self.lane_id, validator = %self.validator, staker = %self.staker)
    )]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(state_transaction, self.lane_id, "bond_public_lane_stake")?;
        finalize_validator_lifecycle(state_transaction)?;
        ensure_positive_amount(&self.amount, "stake amount")?;

        let stake_ctx = stake_context(
            &state_transaction.world,
            &state_transaction.nexus.staking,
            &self.staker,
            None,
        )?;
        assert_stake_amount_matches_spec(
            state_transaction,
            &stake_ctx.asset_definition,
            &self.amount,
        )?;

        let validator_key = validator_storage_key(self.lane_id, &self.validator);
        if state_transaction
            .world
            .public_lane_validators
            .get(&validator_key)
            .is_none()
        {
            return Err(Error::InvariantViolation("validator not registered".into()));
        }

        let amount = self.amount.clone();
        let available = state_transaction
            .world
            .assets
            .get(&stake_ctx.staker_asset)
            .map_or_else(Numeric::zero, |balance| balance.as_ref().clone());
        if available < amount {
            return Err(Error::Math(MathError::NotEnoughQuantity));
        }
        state_transaction
            .world
            .withdraw_numeric_asset(&stake_ctx.staker_asset, &amount)?;
        state_transaction
            .world
            .deposit_numeric_asset(&stake_ctx.escrow_asset, &amount)?;

        {
            let validator = state_transaction
                .world
                .public_lane_validators
                .get_mut(&validator_key)
                .expect("validated above");
            validator.total_stake = numeric_add(validator.total_stake.clone(), amount.clone())?;
            if self.staker == self.validator {
                validator.self_stake = numeric_add(validator.self_stake.clone(), amount.clone())?;
            }
        }

        let share_key = stake_key(self.lane_id, &self.validator, &self.staker);
        let mut share = state_transaction
            .world
            .public_lane_stake_shares
            .get(&share_key)
            .cloned()
            .unwrap_or_else(|| PublicLaneStakeShare {
                lane_id: self.lane_id,
                validator: self.validator.clone(),
                staker: self.staker.clone(),
                bonded: Numeric::zero(),
                pending_unbonds: BTreeMap::new(),
                metadata: Metadata::default(),
            });
        share.metadata = self.metadata.clone();
        share.bonded = numeric_add(share.bonded, amount.clone())?;
        persist_share(state_transaction, share_key, share);

        sumeragi_status::record_public_lane_bonded_delta(self.lane_id, &amount, true);

        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .increase_public_lane_bonded(self.lane_id, &amount);

        Ok(())
    }
}

impl Execute for SchedulePublicLaneUnbond {
    #[iroha_logger::log(
        name = "schedule_public_lane_unbond",
        skip_all,
        fields(lane_id = %self.lane_id, validator = %self.validator, staker = %self.staker)
    )]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "schedule_public_lane_unbond",
        )?;
        finalize_validator_lifecycle(state_transaction)?;
        ensure_positive_amount(&self.amount, "unbond amount")?;
        let block_timestamp_ms = state_transaction.block_unix_timestamp_ms();
        let unbonding_delay_ms = duration_millis(state_transaction.nexus.staking.unbonding_delay);
        if self.release_at_ms.saturating_sub(block_timestamp_ms) < unbonding_delay_ms {
            return Err(Error::InvariantViolation(
                "release_at_ms must respect the configured unbonding delay".into(),
            ));
        }
        if self.release_at_ms < block_timestamp_ms {
            return Err(Error::InvariantViolation(
                "release_at_ms must be in the future or equal to the current block".into(),
            ));
        }
        let stake_ctx = stake_context(
            &state_transaction.world,
            &state_transaction.nexus.staking,
            &self.staker,
            None,
        )?;
        assert_stake_amount_matches_spec(
            state_transaction,
            &stake_ctx.asset_definition,
            &self.amount,
        )?;

        let validator_key = validator_storage_key(self.lane_id, &self.validator);
        let validator = state_transaction
            .world
            .public_lane_validators
            .get_mut(&validator_key)
            .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;

        let amount = self.amount.clone();
        if validator.total_stake < amount {
            return Err(Error::InvariantViolation(
                "unbond exceeds validator total stake".into(),
            ));
        }

        validator.total_stake = numeric_sub(validator.total_stake.clone(), amount.clone())?;
        if self.staker == self.validator {
            validator.self_stake = numeric_sub(validator.self_stake.clone(), amount.clone())?;
        }

        let share_key = stake_key(self.lane_id, &self.validator, &self.staker);
        let mut share = state_transaction
            .world
            .public_lane_stake_shares
            .get(&share_key)
            .cloned()
            .ok_or_else(|| Error::InvariantViolation("stake position not found".into()))?;
        if share.pending_unbonds.contains_key(&self.request_id) {
            return Err(Error::InvariantViolation(
                "unbond request already scheduled".into(),
            ));
        }
        if share.bonded < amount {
            return Err(Error::InvariantViolation(
                "unbond exceeds bonded amount".into(),
            ));
        }

        share.bonded = numeric_sub(share.bonded, amount.clone())?;
        share.pending_unbonds.insert(
            self.request_id,
            PublicLaneUnbonding {
                request_id: self.request_id,
                amount: amount.clone(),
                release_at_ms: self.release_at_ms,
            },
        );
        persist_share(state_transaction, share_key, share);

        sumeragi_status::record_public_lane_bonded_delta(self.lane_id, &amount, false);
        sumeragi_status::record_public_lane_pending_unbond_delta(self.lane_id, &amount, true);

        #[cfg(feature = "telemetry")]
        {
            state_transaction
                .telemetry
                .decrease_public_lane_bonded(self.lane_id, &self.amount);
            state_transaction
                .telemetry
                .increase_public_lane_pending_unbond(self.lane_id, &self.amount);
        }

        Ok(())
    }
}

impl Execute for FinalizePublicLaneUnbond {
    #[iroha_logger::log(
        name = "finalize_public_lane_unbond",
        skip_all,
        fields(lane_id = %self.lane_id, validator = %self.validator, staker = %self.staker)
    )]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "finalize_public_lane_unbond",
        )?;
        finalize_validator_lifecycle(state_transaction)?;
        let block_timestamp_ms = state_transaction.block_unix_timestamp_ms();
        let stake_ctx = stake_context(
            &state_transaction.world,
            &state_transaction.nexus.staking,
            &self.staker,
            None,
        )?;
        let share_key = stake_key(self.lane_id, &self.validator, &self.staker);
        let mut share = state_transaction
            .world
            .public_lane_stake_shares
            .get(&share_key)
            .cloned()
            .ok_or_else(|| Error::InvariantViolation("stake position not found".into()))?;
        let pending = share
            .pending_unbonds
            .remove(&self.request_id)
            .ok_or_else(|| Error::InvariantViolation("unbond request not found".into()))?;
        if pending.release_at_ms > block_timestamp_ms {
            return Err(Error::InvariantViolation(
                "unbond request not yet releasable".into(),
            ));
        }
        let grace_ms = duration_millis(state_transaction.nexus.staking.withdraw_grace);
        let withdraw_deadline = pending.release_at_ms.saturating_add(grace_ms);
        if block_timestamp_ms > withdraw_deadline {
            return Err(Error::InvariantViolation(
                "unbond request exceeded withdraw grace window".into(),
            ));
        }
        assert_stake_amount_matches_spec(
            state_transaction,
            &stake_ctx.asset_definition,
            &pending.amount,
        )?;
        state_transaction
            .world
            .withdraw_numeric_asset(&stake_ctx.escrow_asset, &pending.amount)?;
        state_transaction
            .world
            .deposit_numeric_asset(&stake_ctx.staker_asset, &pending.amount)?;
        persist_share(state_transaction, share_key, share);

        sumeragi_status::record_public_lane_pending_unbond_delta(
            self.lane_id,
            &pending.amount,
            false,
        );

        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .decrease_public_lane_pending_unbond(self.lane_id, &pending.amount);

        Ok(())
    }
}

impl Execute for SlashPublicLaneValidator {
    #[iroha_logger::log(
        name = "slash_public_lane_validator",
        skip_all,
        fields(lane_id = %self.lane_id, validator = %self.validator, slash_id = %self.slash_id)
    )]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "slash_public_lane_validator",
        )?;
        finalize_validator_lifecycle(state_transaction)?;
        ensure_positive_amount(&self.amount, "slash amount")?;
        apply_slash_to_validator(
            &mut state_transaction.world,
            &state_transaction.nexus.staking,
            self.lane_id,
            &self.validator,
            self.slash_id,
            &self.amount,
            #[cfg(feature = "telemetry")]
            Some(state_transaction.telemetry),
            #[cfg(not(feature = "telemetry"))]
            None,
        )
    }
}

impl Execute for CancelConsensusEvidencePenalty {
    #[iroha_logger::log(name = "cancel_consensus_evidence_penalty", skip_all)]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let key = evidence_key(&self.evidence);
        let mut record = state_transaction
            .world
            .consensus_evidence
            .get(&key)
            .cloned()
            .ok_or_else(|| Error::InvariantViolation("consensus evidence not found".into()))?;
        if record.penalty_applied {
            return Err(Error::InvariantViolation(
                "consensus evidence penalty already applied".into(),
            ));
        }
        if record.penalty_cancelled {
            return Ok(());
        }
        record.penalty_cancelled = true;
        record.penalty_cancelled_at_height = Some(state_transaction.block_height());
        state_transaction
            .world
            .consensus_evidence
            .insert(key, record);
        Ok(())
    }
}

impl Execute for RecordPublicLaneRewards {
    #[iroha_logger::log(
        name = "record_public_lane_rewards",
        skip_all,
        fields(lane_id = %self.lane_id, epoch = self.epoch)
    )]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !state_transaction.nexus.enabled {
            return Err(Error::InvariantViolation(
                "Nexus rewards are disabled on this build".into(),
            ));
        }
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "record_public_lane_rewards",
        )?;
        ensure_positive_amount(&self.total_reward, "total_reward")?;
        finalize_validator_lifecycle(state_transaction)?;
        ensure_reward_targets_active(state_transaction, self.lane_id, &self.shares)?;

        let record_key = (self.lane_id, self.epoch);
        ensure_reward_epoch_fresh(state_transaction, self.lane_id, self.epoch, record_key)?;
        validate_reward_amounts(
            &self.total_reward,
            &self.shares,
            self.reward_asset.definition(),
            state_transaction,
        )?;
        if state_transaction.nexus.enabled {
            validate_reward_sink(&self.reward_asset, &self.total_reward, state_transaction)?;
        }

        let record = PublicLaneRewardRecord {
            lane_id: self.lane_id,
            epoch: self.epoch,
            asset: self.reward_asset.clone(),
            total_reward: self.total_reward.clone(),
            shares: self.shares.clone(),
            metadata: self.metadata.clone(),
        };
        state_transaction
            .world
            .public_lane_rewards
            .insert(record_key, record);
        update_validator_rewards(state_transaction, self.lane_id, self.epoch, &self.shares);

        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_public_lane_reward(self.lane_id, &self.total_reward);

        Ok(())
    }
}

impl Execute for ClaimPublicLaneRewards {
    #[iroha_logger::log(
        name = "claim_public_lane_rewards",
        skip_all,
        fields(lane_id = %self.lane_id, account = %self.account)
    )]
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !state_transaction.nexus.enabled {
            return Err(Error::InvariantViolation(
                "Nexus rewards are disabled on this build".into(),
            ));
        }
        ensure_lane_allows_staking(state_transaction, self.lane_id, "claim_public_lane_rewards")?;
        finalize_validator_lifecycle(state_transaction)?;
        if &self.account != authority {
            return Err(Error::InvariantViolation(
                "reward claims must be submitted by the recipient account".into(),
            ));
        }

        let upto_epoch = self.upto_epoch.unwrap_or(u64::MAX);
        let mut claim_totals: BTreeMap<AssetId, (Numeric, u64)> = BTreeMap::new();

        // Preload last-claimed epochs so we can skip already settled rewards.
        let mut last_claimed: BTreeMap<AssetId, u64> = BTreeMap::new();
        for ((lane, account, asset_id), epoch) in
            state_transaction.world.public_lane_reward_claims.iter()
        {
            if *lane != self.lane_id || account != &self.account {
                continue;
            }
            last_claimed.insert(asset_id.clone(), *epoch);
        }

        for ((lane, epoch), record) in state_transaction.world.public_lane_rewards.iter() {
            if *lane != self.lane_id {
                continue;
            }
            if *epoch > upto_epoch {
                break;
            }
            let last_seen = *last_claimed.get(&record.asset).unwrap_or(&0);
            if *epoch <= last_seen {
                continue;
            }
            for share in record.shares.iter().filter(|s| s.account == self.account) {
                let entry = claim_totals
                    .entry(record.asset.clone())
                    .or_insert_with(|| (Numeric::zero(), last_seen));
                entry.0 = numeric_add(entry.0.clone(), share.amount.clone())?;
                entry.1 = entry.1.max(*epoch);
            }
        }

        if claim_totals.is_empty() {
            return Ok(());
        }

        let sink_account = crate::block::parse_account_literal_with_world(
            &state_transaction.world,
            &state_transaction.nexus.fees.fee_sink_account_id,
        )
        .or_else(|| {
            state_transaction
                .nexus
                .fees
                .fee_sink_account_id
                .parse()
                .ok()
        })
        .ok_or_else(|| {
            Error::InvariantViolation(
                "invalid nexus.fees.fee_sink_account_id; expected account identifier".into(),
            )
        })?;
        let fee_asset: AssetDefinitionId = state_transaction
            .nexus
            .fees
            .fee_asset_id
            .parse()
            .map_err(|_| {
                Error::InvariantViolation(
                    "invalid nexus.fees.fee_asset_id; expected `name#domain`".into(),
                )
            })?;
        let dust_threshold = state_transaction.nexus.staking.reward_dust_threshold;
        let dust_numeric = Numeric::new(u128::from(dust_threshold), 0);

        for (asset_id, (amount, max_epoch)) in claim_totals {
            if amount.is_zero() {
                continue;
            }
            if asset_id.account() != &sink_account {
                return Err(Error::InvariantViolation(
                    "reward asset owner must match the configured fee sink account".into(),
                ));
            }
            if asset_id.definition() != &fee_asset {
                return Err(Error::InvariantViolation(
                    "reward asset definition must match the configured fee asset".into(),
                ));
            }
            if dust_threshold > 0 && amount < dust_numeric {
                state_transaction
                    .world
                    .public_lane_reward_claims
                    .insert((self.lane_id, self.account.clone(), asset_id), max_epoch);
                continue;
            }

            let transfer = iroha_data_model::isi::Transfer::<
                Asset,
                Numeric,
                iroha_data_model::account::Account,
            >::asset_numeric(
                asset_id.clone(), amount, self.account.clone()
            );
            transfer.execute(&sink_account, state_transaction)?;
            state_transaction
                .world
                .public_lane_reward_claims
                .insert((self.lane_id, self.account.clone(), asset_id), max_epoch);
        }

        Ok(())
    }
}

fn validator_storage_key(lane_id: LaneId, validator: &AccountId) -> (LaneId, AccountId) {
    (lane_id, validator.clone())
}

fn finalize_validator_lifecycle(
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    finalize_pending_activations(state_transaction)?;
    finalize_released_exits(state_transaction);
    Ok(())
}

#[cfg_attr(not(feature = "telemetry"), allow(unused_variables))]
pub(crate) fn promote_pending_validator(
    record: &mut PublicLaneValidatorRecord,
    lane_id: LaneId,
    block_height: u64,
    current_epoch: u64,
    previous_status: &PublicLaneValidatorStatus,
    telemetry: Option<&StateTelemetry>,
) -> Result<(), Error> {
    let pending_epoch = match record.status {
        PublicLaneValidatorStatus::PendingActivation(epoch) => epoch,
        _ => current_epoch,
    };

    if let Some(existing_epoch) = record.activation_epoch {
        if current_epoch < existing_epoch {
            return Err(Error::InvariantViolation(
                "activation epoch cannot regress".into(),
            ));
        }
    }
    if let Some(existing_height) = record.activation_height {
        if block_height < existing_height {
            return Err(Error::InvariantViolation(
                "activation height cannot regress".into(),
            ));
        }
    }

    let activation_epoch = record
        .activation_epoch
        .unwrap_or_else(|| current_epoch.max(pending_epoch))
        .max(current_epoch.max(pending_epoch));
    record.activation_epoch = Some(activation_epoch);
    record.activation_height = Some(record.activation_height.unwrap_or(block_height));
    record.status = PublicLaneValidatorStatus::Active;

    #[cfg(not(feature = "telemetry"))]
    let _ = telemetry;

    #[cfg(feature = "telemetry")]
    if let Some(tel) = telemetry {
        tel.record_public_lane_validator_status(lane_id, Some(previous_status), &record.status);
        tel.record_public_lane_validator_activation(lane_id, activation_epoch);
    }

    Ok(())
}

fn finalize_pending_activations(
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let block_height = state_transaction.block_height();
    let epoch_length = state_transaction
        .world
        .sumeragi_npos_parameters()
        .map_or(
            iroha_config::parameters::defaults::sumeragi::EPOCH_LENGTH_BLOCKS,
            |params| params.epoch_length_blocks,
        )
        .max(1);
    let current_epoch = current_epoch(block_height, epoch_length)?;

    let to_activate: Vec<_> = state_transaction
        .world
        .public_lane_validators
        .iter()
        .filter(|(_, record)| {
            matches!(
                record.status,
                PublicLaneValidatorStatus::PendingActivation(pending) if pending <= current_epoch
            )
        })
        .map(|(key, record)| (key.clone(), record.status.clone()))
        .collect();

    for (key, previous_status) in to_activate {
        let Some(record) = state_transaction.world.public_lane_validators.get_mut(&key) else {
            continue;
        };
        let telemetry = {
            #[cfg(feature = "telemetry")]
            {
                Some(state_transaction.telemetry)
            }
            #[cfg(not(feature = "telemetry"))]
            {
                None
            }
        };
        promote_pending_validator(
            record,
            key.0,
            block_height,
            current_epoch,
            &previous_status,
            telemetry,
        )?;
    }

    Ok(())
}

fn finalize_released_exits(state_transaction: &mut StateTransaction<'_, '_>) {
    let now_ms = state_transaction.block_unix_timestamp_ms();
    let exiting: Vec<_> = state_transaction
        .world
        .public_lane_validators
        .iter()
        .filter(|(_, record)| {
            matches!(
                record.status,
                PublicLaneValidatorStatus::Exiting(releases_at) if releases_at <= now_ms
            )
        })
        .map(|(key, record)| (key.clone(), record.status.clone()))
        .collect();

    for (key, previous_status) in exiting {
        if let Some(record) = state_transaction.world.public_lane_validators.get_mut(&key) {
            record.status = PublicLaneValidatorStatus::Exited;
        }
        remove_all_shares_for_validator(state_transaction, key.0, &key.1);
        #[cfg(not(feature = "telemetry"))]
        let _ = previous_status;
        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_public_lane_validator_status(
                key.0,
                Some(&previous_status),
                &PublicLaneValidatorStatus::Exited,
            );
    }
}

fn ensure_reward_targets_active(
    state_transaction: &StateTransaction<'_, '_>,
    lane_id: LaneId,
    shares: &[PublicLaneRewardShare],
) -> Result<(), Error> {
    for share in shares {
        if !matches!(share.role, PublicLaneRewardRole::Validator) {
            continue;
        }
        let Some(record) = state_transaction
            .world
            .public_lane_validators
            .get(&validator_storage_key(lane_id, &share.account))
        else {
            return Err(Error::InvariantViolation(
                "reward share references unknown validator".into(),
            ));
        };
        if !matches!(record.status, PublicLaneValidatorStatus::Active) {
            return Err(Error::InvariantViolation(
                "reward share validator is not active".into(),
            ));
        }
    }
    Ok(())
}

fn stake_key(
    lane_id: LaneId,
    validator: &AccountId,
    staker: &AccountId,
) -> (LaneId, AccountId, AccountId) {
    (lane_id, validator.clone(), staker.clone())
}

fn ensure_positive_amount(amount: &Numeric, label: &str) -> Result<(), Error> {
    if amount.is_zero() {
        return Err(Error::InvariantViolation(
            format!("{label} must be greater than zero").into(),
        ));
    }
    Ok(())
}

fn numeric_add(lhs: Numeric, rhs: Numeric) -> Result<Numeric, Error> {
    lhs.checked_add(rhs)
        .ok_or_else(|| Error::Math(MathError::Overflow))
}

fn ensure_reward_epoch_fresh(
    state_transaction: &StateTransaction<'_, '_>,
    lane_id: LaneId,
    epoch: u64,
    record_key: (LaneId, u64),
) -> Result<(), Error> {
    if state_transaction
        .world
        .public_lane_rewards
        .get(&record_key)
        .is_some()
    {
        return Err(Error::InvariantViolation(
            "reward entry already recorded for epoch".into(),
        ));
    }
    if let Some(latest_epoch) = state_transaction
        .world
        .public_lane_rewards
        .iter()
        .filter(|((lane, _), _)| *lane == lane_id)
        .map(|((_, existing_epoch), _)| *existing_epoch)
        .max()
    {
        if epoch <= latest_epoch {
            return Err(Error::InvariantViolation(
                "reward epoch must be greater than the last recorded epoch for the lane".into(),
            ));
        }
    }
    Ok(())
}

fn validate_reward_amounts(
    total_reward: &Numeric,
    shares: &[PublicLaneRewardShare],
    reward_definition: &AssetDefinitionId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let computed_total = shares.iter().try_fold(Numeric::zero(), |acc, share| {
        numeric_add(acc, share.amount.clone())
    })?;
    if computed_total != *total_reward {
        return Err(Error::InvariantViolation(
            "reward shares must sum to total_reward".into(),
        ));
    }
    let reward_spec = state_transaction
        .numeric_spec_for(reward_definition)
        .map_err(Error::from)?;
    assert_numeric_spec_with(total_reward, reward_spec)?;
    for share in shares {
        ensure_positive_amount(&share.amount, "reward share amount")?;
        assert_numeric_spec_with(&share.amount, reward_spec)?;
    }
    Ok(())
}

fn validate_reward_sink(
    reward_asset: &AssetId,
    total_reward: &Numeric,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let sink_account = crate::block::parse_account_literal_with_world(
        &state_transaction.world,
        &state_transaction.nexus.fees.fee_sink_account_id,
    )
    .or_else(|| {
        state_transaction
            .nexus
            .fees
            .fee_sink_account_id
            .parse()
            .ok()
    })
    .ok_or_else(|| {
        Error::InvariantViolation(
            "invalid nexus.fees.fee_sink_account_id; expected account identifier".into(),
        )
    })?;
    let fee_asset: AssetDefinitionId =
        state_transaction
            .nexus
            .fees
            .fee_asset_id
            .parse()
            .map_err(|_| {
                Error::InvariantViolation(
                    "invalid nexus.fees.fee_asset_id; expected `name#domain`".into(),
                )
            })?;
    if reward_asset.account() != &sink_account {
        return Err(Error::InvariantViolation(
            "reward asset owner must match the configured fee sink account".into(),
        ));
    }
    if reward_asset.definition() != &fee_asset {
        return Err(Error::InvariantViolation(
            "reward asset definition must match the configured fee asset".into(),
        ));
    }
    let sink_balance = state_transaction
        .world
        .assets
        .get(reward_asset)
        .cloned()
        .ok_or_else(|| {
            Error::InvariantViolation(
                "reward asset must exist in the configured fee sink account".into(),
            )
        })?;
    if sink_balance.as_ref() < total_reward {
        return Err(Error::InvariantViolation(
            "insufficient balance in reward fee sink for recorded payout".into(),
        ));
    }
    Ok(())
}

fn update_validator_rewards(
    state_transaction: &mut StateTransaction<'_, '_>,
    lane_id: LaneId,
    epoch: u64,
    shares: &[PublicLaneRewardShare],
) {
    for share in shares {
        if !matches!(share.role, PublicLaneRewardRole::Validator) {
            continue;
        }
        if let Some(validator) = state_transaction
            .world
            .public_lane_validators
            .get_mut(&validator_storage_key(lane_id, &share.account))
        {
            validator.last_reward_epoch = Some(epoch);
        }
    }
}

fn numeric_sub(lhs: Numeric, rhs: Numeric) -> Result<Numeric, Error> {
    lhs.checked_sub(rhs)
        .ok_or_else(|| Error::Math(MathError::Overflow))
}

fn min_numeric(lhs: Numeric, rhs: Numeric) -> Numeric {
    if lhs <= rhs { lhs } else { rhs }
}

fn persist_share(
    state_transaction: &mut StateTransaction<'_, '_>,
    key: (LaneId, AccountId, AccountId),
    share: PublicLaneStakeShare,
) {
    if share.bonded.is_zero() && share.pending_unbonds.is_empty() {
        state_transaction.world.public_lane_stake_shares.remove(key);
        return;
    }
    state_transaction
        .world
        .public_lane_stake_shares
        .insert(key, share);
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn meets_min_stake(amount: &Numeric, min_units: u64) -> Result<bool, Error> {
    let scale = amount.scale();
    let multiplier = BigInt::pow10(scale).ok_or(Error::Math(MathError::Overflow))?;
    let threshold = BigInt::from(i128::from(min_units))
        .checked_mul(&multiplier)
        .map_err(|_| Error::Math(MathError::Overflow))?;
    Ok(amount.mantissa() >= &threshold)
}

fn slash_within_limit(amount: &Numeric, total: &Numeric, max_bps: u16) -> Result<bool, Error> {
    let target_scale = amount.scale().max(total.scale());
    let amount_scaled = scale_amount_to(amount, target_scale)?;
    let total_scaled = scale_amount_to(total, target_scale)?;
    let lhs = amount_scaled
        .checked_mul(&BigInt::from(10_000i32))
        .map_err(|_| Error::Math(MathError::Overflow))?;
    let rhs = total_scaled
        .checked_mul(&BigInt::from(i32::from(max_bps)))
        .map_err(|_| Error::Math(MathError::Overflow))?;
    Ok(lhs <= rhs)
}

fn scale_amount_to(amount: &Numeric, target_scale: u32) -> Result<BigInt, Error> {
    let current_scale = amount.scale();
    if target_scale < current_scale {
        return Err(Error::Math(MathError::Overflow));
    }
    let factor = BigInt::pow10(target_scale.saturating_sub(current_scale))
        .ok_or(Error::Math(MathError::Overflow))?;
    amount
        .mantissa()
        .checked_mul(&factor)
        .map_err(|_| Error::Math(MathError::Overflow))
}

/// Compute the maximum slashable amount given a total bonded stake and max ratio.
pub(crate) fn max_slash_amount(total: &Numeric, max_bps: u16) -> Result<Numeric, Error> {
    if total.is_zero() || max_bps == 0 {
        return Ok(Numeric::zero());
    }
    if max_bps >= 10_000 {
        return Ok(total.clone());
    }
    let scaled = total
        .mantissa()
        .checked_mul(&BigInt::from(i32::from(max_bps)))
        .map_err(|_| Error::Math(MathError::Overflow))?;
    let (mut mantissa, _) = scaled
        .checked_div_rem(&BigInt::from(10_000i32))
        .map_err(|_| Error::Math(MathError::Overflow))?;
    if mantissa.is_zero() {
        mantissa = BigInt::from(1i32);
    }
    Numeric::try_new(mantissa, total.scale()).map_err(|_| Error::Math(MathError::Overflow))
}

fn ensure_validator_peer_registered(
    state_transaction: &StateTransaction<'_, '_>,
    lane_id: LaneId,
    validator: &AccountId,
) -> Result<(), Error> {
    let validator_peer = validator
        .try_signatory()
        .map(|pk| PeerId::from(pk.clone()))
        .ok_or_else(|| {
            Error::InvariantViolation(
                "validator must be single-signatory to register for public lanes".into(),
            )
        })?;
    if !state_transaction
        .world
        .peers
        .iter()
        .any(|peer| peer == &validator_peer)
    {
        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_public_lane_validator_reject("missing_peer");
        iroha_logger::warn!(
            lane_id = %lane_id,
            validator = %validator,
            peer = %validator_peer,
            "public-lane validator action rejected: peer not registered in topology"
        );
        return Err(Error::InvariantViolation(
            "validator peer must be registered before staking".into(),
        ));
    }

    let block_height = state_transaction.block_height();
    let gate = peer_consensus_key_gate(&state_transaction.world, &validator_peer, block_height);
    if gate != ConsensusKeyGate::Live {
        let (telemetry_reason, message) = match gate {
            ConsensusKeyGate::Live => ("consensus_key_live", "unexpected live consensus key"),
            ConsensusKeyGate::Missing => (
                "consensus_key_missing",
                "validator peer must register a consensus key before staking",
            ),
            ConsensusKeyGate::NotYetActive => (
                "consensus_key_pending",
                "validator peer consensus key is not yet active for this block",
            ),
            ConsensusKeyGate::Expired => (
                "consensus_key_expired",
                "validator peer consensus key expired before this block",
            ),
            ConsensusKeyGate::Disabled => (
                "consensus_key_disabled",
                "validator peer consensus key is disabled",
            ),
        };
        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_public_lane_validator_reject(telemetry_reason);
        iroha_logger::warn!(
            lane_id = %lane_id,
            validator = %validator,
            peer = %validator_peer,
            height = block_height,
            reason = telemetry_reason,
            "public-lane validator action rejected: {message}"
        );
        return Err(Error::InvariantViolation(message.into()));
    }

    let commit_topology: Vec<_> = state_transaction.commit_topology.iter().cloned().collect();
    if !commit_topology.is_empty()
        && commit_topology
            .iter()
            .all(|peer_in_topology| peer_in_topology != &validator_peer)
    {
        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_public_lane_validator_reject("missing_peer_topology");
        iroha_logger::warn!(
            lane_id = %lane_id,
            validator = %validator,
            peer = %validator_peer,
            topology = ?commit_topology,
            "public-lane validator action rejected: peer not present in commit topology"
        );
        return Err(Error::InvariantViolation(
            "validator peer must be present in the commit topology (with a reachable address) before staking"
                .into(),
        ));
    }

    Ok(())
}

fn remove_all_shares_for_validator(
    state_transaction: &mut StateTransaction<'_, '_>,
    lane_id: LaneId,
    validator: &AccountId,
) {
    let share_keys: Vec<_> = state_transaction
        .world
        .public_lane_stake_shares
        .iter()
        .filter(|((lane, val, _), _)| *lane == lane_id && val == validator)
        .map(|(key, _)| key.clone())
        .collect();
    for key in share_keys {
        state_transaction.world.public_lane_stake_shares.remove(key);
    }
}

/// Apply a slash to a validator using a prebuilt world transaction.
pub(crate) fn apply_slash_to_validator(
    world: &mut WorldTransaction<'_, '_>,
    staking_cfg: &iroha_config::parameters::actual::NexusStaking,
    lane_id: LaneId,
    validator: &AccountId,
    slash_id: Hash,
    amount: &Numeric,
    #[cfg(feature = "telemetry")] telemetry: Option<&crate::telemetry::StateTelemetry>,
    #[cfg(not(feature = "telemetry"))] _telemetry: Option<&crate::telemetry::StateTelemetry>,
) -> Result<(), Error> {
    let validator_key = validator_storage_key(lane_id, validator);
    let stake_account = world
        .public_lane_validators
        .get(&validator_key)
        .map(|record| record.stake_account.clone())
        .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;
    let stake_ctx = stake_context(world, staking_cfg, &stake_account, None)?;
    let spec = world
        .asset_definitions
        .get(&stake_ctx.asset_definition)
        .ok_or_else(|| Error::InvariantViolation("stake asset definition missing".into()))?
        .spec();
    assert_numeric_spec_with(amount, spec)?;
    let mut remaining = amount.clone();
    let slashed_status = PublicLaneValidatorStatus::Slashed(slash_id);
    #[cfg(feature = "telemetry")]
    let previous_status: Option<PublicLaneValidatorStatus>;
    {
        let validator = world
            .public_lane_validators
            .get_mut(&validator_key)
            .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;
        let allowed =
            slash_within_limit(amount, &validator.total_stake, staking_cfg.max_slash_bps)?;
        if !allowed {
            return Err(Error::InvariantViolation(
                "slash exceeds configured maximum ratio".into(),
            ));
        }

        #[cfg(feature = "telemetry")]
        {
            previous_status = Some(validator.status.clone());
        }
        if validator.total_stake < amount.clone() {
            return Err(Error::InvariantViolation(
                "slash exceeds total stake".into(),
            ));
        }

        validator.total_stake = numeric_sub(validator.total_stake.clone(), amount.clone())?;
        let self_slash = min_numeric(validator.self_stake.clone(), remaining.clone());
        if !self_slash.is_zero() {
            validator.self_stake = numeric_sub(validator.self_stake.clone(), self_slash.clone())?;
            remaining = numeric_sub(remaining, self_slash)?;
        }
        validator.status = slashed_status.clone();
    }

    if !remaining.is_zero() {
        let share_snapshots: Vec<_> = world
            .public_lane_stake_shares
            .iter()
            .filter(|((lane, validator_id, _), _)| *lane == lane_id && validator_id == validator)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();

        for (key, mut share) in share_snapshots {
            if remaining.is_zero() {
                break;
            }
            if share.bonded.is_zero() {
                continue;
            }
            let slash_part = min_numeric(share.bonded.clone(), remaining.clone());
            if slash_part.is_zero() {
                continue;
            }
            share.bonded = numeric_sub(share.bonded, slash_part.clone())?;
            if share.bonded.is_zero() && share.pending_unbonds.is_empty() {
                world.public_lane_stake_shares.remove(key);
            } else {
                world.public_lane_stake_shares.insert(key, share);
            }
            remaining = numeric_sub(remaining, slash_part)?;
        }
    }

    if !remaining.is_zero() {
        return Err(Error::InvariantViolation(
            "slash could not be satisfied by stake shares".into(),
        ));
    }

    world.withdraw_numeric_asset(&stake_ctx.escrow_asset, amount)?;
    world.deposit_numeric_asset(&stake_ctx.slash_sink_asset, amount)?;

    sumeragi_status::record_public_lane_bonded_delta(lane_id, amount, false);
    sumeragi_status::record_public_lane_slash(lane_id);

    #[cfg(feature = "telemetry")]
    if let Some(t) = telemetry {
        t.record_public_lane_validator_status(lane_id, previous_status.as_ref(), &slashed_status);
        t.decrease_public_lane_bonded(lane_id, amount);
        t.record_public_lane_slash(lane_id);
    }

    Ok(())
}

#[derive(Debug)]
struct StakeEscrowContext {
    asset_definition: AssetDefinitionId,
    staker_asset: AssetId,
    escrow_asset: AssetId,
    slash_sink_asset: AssetId,
}

fn stake_context(
    world: &impl WorldReadOnly,
    staking_cfg: &iroha_config::parameters::actual::NexusStaking,
    staker: &AccountId,
    slash_sink_override: Option<&AccountId>,
) -> Result<StakeEscrowContext, Error> {
    let asset_definition: AssetDefinitionId = staking_cfg.stake_asset_id.parse().map_err(|_| {
        Error::InvariantViolation(
            "invalid nexus.staking.stake_asset_id; expected `name#domain`".into(),
        )
    })?;
    let escrow_account = parse_staking_account_literal(
        world,
        &staking_cfg.stake_escrow_account_id,
        "stake_escrow_account_id",
    )?;
    let slash_sink_account: AccountId = if let Some(account) = slash_sink_override {
        account.clone()
    } else {
        parse_staking_account_literal(
            world,
            &staking_cfg.slash_sink_account_id,
            "slash_sink_account_id",
        )?
    };

    Ok(StakeEscrowContext {
        asset_definition: asset_definition.clone(),
        staker_asset: AssetId::new(asset_definition.clone(), staker.clone()),
        escrow_asset: AssetId::new(asset_definition.clone(), escrow_account),
        slash_sink_asset: AssetId::new(asset_definition, slash_sink_account),
    })
}

fn parse_staking_account_literal(
    world: &impl WorldReadOnly,
    literal: &str,
    field: &'static str,
) -> Result<AccountId, Error> {
    crate::block::parse_account_literal_with_world(world, literal)
        .or_else(|| literal.parse().ok())
        .ok_or_else(|| {
            Error::InvariantViolation(
                format!("invalid nexus.staking.{field}; expected account identifier").into(),
            )
        })
}

fn assert_stake_amount_matches_spec(
    state_transaction: &mut StateTransaction<'_, '_>,
    asset_definition: &AssetDefinitionId,
    amount: &Numeric,
) -> Result<(), Error> {
    let spec = state_transaction
        .numeric_spec_for(asset_definition)
        .map_err(Error::from)?;
    assert_numeric_spec_with(amount, spec)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;
    use std::time::Duration;

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        account::{Account, MultisigMember, MultisigPolicy},
        asset::{AssetDefinition, AssetDefinitionId},
        block::consensus::{
            CertPhase, ConsensusBlockHeader, Evidence, EvidenceKind, EvidencePayload,
            EvidenceRecord, Proposal, QcRef,
        },
        consensus::{ConsensusKeyRecord, ConsensusKeyStatus},
        domain::Domain,
        isi::error::InvalidParameterError,
        nexus::{DataSpaceId, LaneCatalog, LaneConfig, LaneVisibility, PublicLaneRewardShare},
        parameter::{Parameter, system::SumeragiNposParameters},
        peer::Peer,
        prelude::*,
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, gen_account_in};
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        block::ValidBlock,
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, StateBlock, StateTransaction, World},
    };

    fn new_block() -> crate::block::CommittedBlock {
        let (_leader_public_key, leader_private_key) = iroha_crypto::KeyPair::random().into_parts();
        ValidBlock::new_dummy_and_modify_header(&leader_private_key, |h| {
            h.set_height(NonZeroU64::new(1).unwrap());
        })
        .commit_unchecked()
        .unpack(|_| {})
    }

    fn block_header_with_height(height: u64) -> iroha_data_model::block::BlockHeader {
        let mut header = new_block().as_ref().header();
        header.set_height(NonZeroU64::new(height).expect("non-zero height"));
        header
    }

    fn new_block_with_height(height: u64) -> crate::block::CommittedBlock {
        let (_leader_public_key, leader_private_key) = iroha_crypto::KeyPair::random().into_parts();
        ValidBlock::new_dummy_and_modify_header(&leader_private_key, |h| {
            h.set_height(NonZeroU64::new(height).expect("non-zero height"));
        })
        .commit_unchecked()
        .unpack(|_| {})
    }

    fn new_block_with_height_and_time(
        height: u64,
        creation_time_ms: u64,
    ) -> crate::block::CommittedBlock {
        let (_leader_public_key, leader_private_key) = iroha_crypto::KeyPair::random().into_parts();
        ValidBlock::new_dummy_and_modify_header(&leader_private_key, |h| {
            h.set_height(NonZeroU64::new(height).expect("non-zero height"));
            h.creation_time_ms = creation_time_ms;
        })
        .commit_unchecked()
        .unpack(|_| {})
    }

    fn record_block_commit(state_block: &mut StateBlock<'_>, block: &crate::block::CommittedBlock) {
        let topology = state_block.commit_topology.get().clone();
        let _ = state_block.apply_without_execution(block, topology);
    }

    fn setup_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        State::new(World::default(), kura, query_handle)
    }

    fn register_peer_for_account(
        stx: &mut StateTransaction<'_, '_>,
        account: &AccountId,
    ) -> crate::PeerId {
        let peer = crate::PeerId::from(
            account
                .try_signatory()
                .expect("test accounts are single-signatory")
                .clone(),
        );
        let _ = stx.world.peers.push(peer.clone());
        seed_validator_consensus_key(stx, &peer, ConsensusKeyStatus::Active);
        peer
    }

    fn seed_validator_consensus_key(
        stx: &mut StateTransaction<'_, '_>,
        peer: &crate::PeerId,
        status: ConsensusKeyStatus,
    ) {
        let ident = crate::state::derive_validator_key_id(peer.public_key());
        let mut record = ConsensusKeyRecord {
            id: ident,
            public_key: peer.public_key().clone(),
            pop: None,
            activation_height: stx.block_height(),
            expiry_height: None,
            hsm: None,
            replaces: None,
            status,
        };
        if matches!(record.status, ConsensusKeyStatus::Disabled) {
            record.expiry_height = Some(stx.block_height());
        }
        stx.world
            .consensus_keys
            .insert(record.id.clone(), record.clone());
        let pk = record.public_key.to_string();
        let mut by_pk = stx
            .world
            .consensus_keys_by_pk
            .get(&pk)
            .cloned()
            .unwrap_or_default();
        if !by_pk.contains(&record.id) {
            by_pk.push(record.id.clone());
            stx.world.consensus_keys_by_pk.insert(pk, by_pk);
        }
    }

    fn seed_validator_consensus_key_with_heights(
        stx: &mut StateTransaction<'_, '_>,
        peer: &crate::PeerId,
        status: ConsensusKeyStatus,
        activation_height: u64,
        expiry_height: Option<u64>,
    ) {
        let ident = crate::state::derive_validator_key_id(peer.public_key());
        let mut record = ConsensusKeyRecord {
            id: ident,
            public_key: peer.public_key().clone(),
            pop: None,
            activation_height,
            expiry_height,
            hsm: None,
            replaces: None,
            status,
        };
        if matches!(record.status, ConsensusKeyStatus::Disabled) {
            record.expiry_height = Some(record.expiry_height.unwrap_or(activation_height));
        }
        stx.world
            .consensus_keys
            .insert(record.id.clone(), record.clone());
        let key_label = record.public_key.to_string();
        let mut by_pk = stx
            .world
            .consensus_keys_by_pk
            .get(&key_label)
            .cloned()
            .unwrap_or_default();
        if !by_pk.contains(&record.id) {
            by_pk.push(record.id.clone());
            stx.world.consensus_keys_by_pk.insert(key_label, by_pk);
        }
    }

    fn set_epoch_length(state: &mut State, epoch_length_blocks: u64) {
        let mut wb = state.world.block();
        {
            let params = wb.parameters.get_mut();
            params.set_parameter(Parameter::Custom(
                SumeragiNposParameters {
                    epoch_length_blocks,
                    ..SumeragiNposParameters::default()
                }
                .into_custom_parameter(),
            ));
        }
        wb.commit();
    }

    fn configure_reward_fixture(
        stx: &mut StateTransaction<'_, '_>,
        lane_id: LaneId,
        mint_amount: u32,
    ) -> (AccountId, AccountId, AssetId, AssetDefinitionId) {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, stx)
            .unwrap();
        let (sink, _) = gen_account_in("wonderland");
        let (validator, _) = gen_account_in("wonderland");
        Register::account(Account::new(sink.clone()))
            .execute(&ALICE_ID, stx)
            .unwrap();
        Register::account(Account::new(validator.clone()))
            .execute(&ALICE_ID, stx)
            .unwrap();

        let asset_def_id: AssetDefinitionId =
            "xor#wonderland".parse().expect("asset definition id");
        Register::asset_definition(AssetDefinition::numeric(asset_def_id.clone()))
            .execute(&ALICE_ID, stx)
            .unwrap();
        let reward_asset = AssetId::new(asset_def_id.clone(), sink.clone());
        let initial_stake = Numeric::new(u64::from(mint_amount.max(1)), 0);
        Mint::asset_numeric(mint_amount, reward_asset.clone())
            .execute(&ALICE_ID, stx)
            .unwrap();
        let validator_asset = AssetId::new(asset_def_id.clone(), validator.clone());
        Mint::asset_numeric(mint_amount, validator_asset.clone())
            .execute(&ALICE_ID, stx)
            .unwrap();

        stx.nexus.enabled = true;
        stx.nexus.fees.fee_sink_account_id = sink.to_string();
        stx.nexus.fees.fee_asset_id = asset_def_id.to_string();
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(64_u32),
            vec![LaneConfig {
                id: lane_id,
                alias: "lane-9".to_string(),
                dataspace_id: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Public,
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
        stx.nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&stx.nexus.lane_catalog);
        stx.nexus.staking.public_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected;
        stx.nexus.staking.stake_asset_id = asset_def_id.to_string();
        stx.nexus.staking.stake_escrow_account_id = sink.to_string();
        stx.nexus.staking.slash_sink_account_id = sink.to_string();
        let peer = register_peer_for_account(stx, &validator);
        stx.commit_topology.get_mut().push(peer);
        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: initial_stake.clone(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, stx)
        .expect("register validator for rewards");

        (sink, validator, reward_asset, asset_def_id)
    }

    fn prepare_accounts(
        stx: &mut StateTransaction<'_, '_>,
    ) -> (AccountId, AccountId, AccountId, AssetDefinitionId) {
        let domain_id: DomainId = "nexus".parse().expect("domain id");
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, stx)
            .unwrap();
        // Ensure the authority account exists in the test ledger so subsequent instructions
        // can execute under Alice's identity.
        Register::domain(Domain::new(ALICE_ID.domain().clone()))
            .execute(&ALICE_ID, stx)
            .unwrap();
        Register::account(Account::new(ALICE_ID.clone()))
            .execute(&ALICE_ID, stx)
            .unwrap();
        let (validator, _kp) = gen_account_in("nexus");
        let (delegator, _kp) = gen_account_in("nexus");
        let (escrow, _kp) = gen_account_in("nexus");
        Register::account(Account::new(validator.clone()))
            .execute(&ALICE_ID, stx)
            .unwrap();
        Register::account(Account::new(delegator.clone()))
            .execute(&ALICE_ID, stx)
            .unwrap();
        Register::account(Account::new(escrow.clone()))
            .execute(&ALICE_ID, stx)
            .unwrap();
        register_peer_for_account(stx, &validator);
        register_peer_for_account(stx, &delegator);
        register_peer_for_account(stx, &escrow);

        let asset_def_id: AssetDefinitionId = "xor#nexus".parse().expect("asset definition id");
        Register::asset_definition(AssetDefinition::numeric(asset_def_id.clone()))
            .execute(&ALICE_ID, stx)
            .unwrap();
        let validator_asset = AssetId::new(asset_def_id.clone(), validator.clone());
        let delegator_asset = AssetId::new(asset_def_id.clone(), delegator.clone());
        Mint::asset_numeric(10_000u32, validator_asset)
            .execute(&ALICE_ID, stx)
            .unwrap();
        Mint::asset_numeric(10_000u32, delegator_asset)
            .execute(&ALICE_ID, stx)
            .unwrap();

        stx.nexus.staking.stake_asset_id = asset_def_id.to_string();
        stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        stx.nexus.staking.slash_sink_account_id = escrow.to_string();
        stx.commit_topology.get_mut().clear();
        stx.commit_topology
            .get_mut()
            .extend(stx.world.peers.iter().cloned());

        (validator, delegator, escrow, asset_def_id)
    }

    #[test]
    fn stake_context_accepts_ih58_account_literals() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        stx.nexus.staking.stake_asset_id = asset_def_id.to_string();
        stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        stx.nexus.staking.slash_sink_account_id = escrow.to_string();

        let stake_ctx = stake_context(&stx.world, &stx.nexus.staking, &validator, None)
            .expect("stake context should accept IH58 literals");

        assert_eq!(
            stake_ctx.escrow_asset.account(),
            &escrow,
            "escrow account should resolve from literal"
        );
        assert_eq!(
            stake_ctx.slash_sink_asset.account(),
            &escrow,
            "slash sink account should resolve from literal"
        );
    }

    #[test]
    fn register_stores_validator_and_share() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(1),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("register validator");

        assert!(
            stx.world
                .public_lane_validators
                .get(&(LaneId::new(1), validator.clone()))
                .is_some(),
            "validator not stored before apply"
        );

        stx.apply();
        assert_eq!(
            block.as_ref().header().height().get(),
            1,
            "first block height must start at 1 for snapshot"
        );
        record_block_commit(&mut state_block, &block);
        state_block.commit().unwrap();

        let view = state.view();
        let record = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(1), validator.clone()))
            .expect("validator record");
        assert_eq!(record.total_stake, Numeric::new(1_000, 0));
        assert_eq!(record.self_stake, Numeric::new(1_000, 0));

        let share = view
            .world
            .public_lane_stake_shares()
            .get(&(LaneId::new(1), validator.clone(), validator.clone()))
            .expect("share record");
        assert_eq!(share.bonded, Numeric::new(1_000, 0));

        let stake_asset = AssetId::new(asset_def_id.clone(), validator.clone());
        let escrow_asset = AssetId::new(asset_def_id, escrow.clone());
        let stake_balance = view
            .world
            .assets()
            .get(&stake_asset)
            .expect("validator stake balance");
        let escrow_balance = view
            .world
            .assets()
            .get(&escrow_asset)
            .expect("escrow balance");
        assert_eq!(stake_balance.as_ref(), &Numeric::new(9_000, 0));
        assert_eq!(escrow_balance.as_ref(), &Numeric::new(1_000, 0));
    }

    #[test]
    fn register_rejects_without_live_consensus_key() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let validator_peer = crate::PeerId::from(
            validator
                .try_signatory()
                .expect("validator is single-signatory")
                .clone(),
        );
        seed_validator_consensus_key(&mut stx, &validator_peer, ConsensusKeyStatus::Disabled);

        let result = RegisterPublicLaneValidator {
            lane_id: LaneId::new(14),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(
            result.is_err(),
            "validator registration should be rejected without an active consensus key"
        );

        let escrow_asset = AssetId::new(asset_def_id, escrow);
        assert!(
            stx.world.assets.get(&escrow_asset).is_none(),
            "rejected registration must not touch escrow"
        );
    }

    #[test]
    fn register_rejects_when_consensus_key_pending_activation() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let validator_peer = crate::PeerId::from(
            validator
                .try_signatory()
                .expect("validator is single-signatory")
                .clone(),
        );
        let activation_height = stx.block_height().saturating_add(5);
        seed_validator_consensus_key_with_heights(
            &mut stx,
            &validator_peer,
            ConsensusKeyStatus::Pending,
            activation_height,
            None,
        );

        let result = RegisterPublicLaneValidator {
            lane_id: LaneId::new(22),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(
            matches!(&result, Err(Error::InvariantViolation(msg)) if msg.contains("not yet active")),
            "unexpected result: {result:?}"
        );

        let escrow_asset = AssetId::new(asset_def_id, escrow);
        assert!(
            stx.world.assets.get(&escrow_asset).is_none(),
            "rejected registration must not touch escrow"
        );
    }

    #[test]
    fn register_rejects_when_consensus_key_expired() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let validator_peer = crate::PeerId::from(
            validator
                .try_signatory()
                .expect("validator is single-signatory")
                .clone(),
        );
        let expiry_height = stx.block_height();
        seed_validator_consensus_key_with_heights(
            &mut stx,
            &validator_peer,
            ConsensusKeyStatus::Active,
            0,
            Some(expiry_height),
        );

        let result = RegisterPublicLaneValidator {
            lane_id: LaneId::new(23),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(
            matches!(&result, Err(Error::InvariantViolation(msg)) if msg.contains("expired")),
            "unexpected result: {result:?}"
        );

        let escrow_asset = AssetId::new(asset_def_id, escrow);
        assert!(
            stx.world.assets.get(&escrow_asset).is_none(),
            "rejected registration must not touch escrow"
        );
    }

    #[test]
    fn register_rejects_on_admin_managed_lane() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let lane_id = LaneId::new(7);
        stx.nexus.enabled = true;
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(8_u32),
            vec![LaneConfig {
                id: lane_id,
                alias: "restricted".to_string(),
                dataspace_id: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Restricted,
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
        stx.nexus.staking.public_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected;
        stx.nexus.staking.restricted_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;

        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let result = RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator,
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("admin-managed lanes must reject staking");

        assert!(matches!(
            result,
            Error::InvalidParameter(InvalidParameterError::SmartContract(msg))
                if msg.contains("admin-managed")
        ));
        assert!(
            stx.world
                .public_lane_validators
                .iter()
                .all(|((lane, _), _)| lane != &lane_id)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn mixed_mode_respects_lane_validator_modes() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 1);
        let block = new_block_with_height(1);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        stx.nexus.enabled = true;
        let stake_lane = LaneId::new(0);
        let admin_lane = LaneId::new(1);
        stx.nexus.enabled = true;
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(2_u32),
            vec![
                LaneConfig {
                    id: stake_lane,
                    alias: "public-stake".to_string(),
                    dataspace_id: DataSpaceId::GLOBAL,
                    visibility: LaneVisibility::Public,
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: admin_lane,
                    alias: "restricted-admin".to_string(),
                    dataspace_id: DataSpaceId::GLOBAL,
                    visibility: LaneVisibility::Restricted,
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
        stx.nexus.staking.public_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected;
        stx.nexus.staking.restricted_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;

        let (validator, delegator, _, _) = prepare_accounts(&mut stx);
        let stake_peer = crate::PeerId::from(
            validator
                .try_signatory()
                .expect("validator is single-signatory")
                .clone(),
        );
        RegisterPublicLaneValidator {
            lane_id: stake_lane,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("stake-elected lane should accept staking");

        let err = RegisterPublicLaneValidator {
            lane_id: admin_lane,
            validator: delegator.clone(),
            stake_account: delegator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("admin-managed lane must reject staking");
        assert!(matches!(
            err,
            Error::InvalidParameter(InvalidParameterError::SmartContract(msg))
                if msg.contains("admin-managed")
        ));

        stx.apply();
        assert_eq!(
            block.as_ref().header().height().get(),
            1,
            "first block height must start at 1 for snapshot"
        );
        record_block_commit(&mut state_block, &block);
        state_block.commit().unwrap();

        let block = new_block_with_height(2);
        let mut activation_block = state.block(block.as_ref().header());
        let mut activation_tx = activation_block.transaction();
        ActivatePublicLaneValidator {
            lane_id: stake_lane,
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut activation_tx)
        .unwrap();
        activation_tx.apply();
        record_block_commit(&mut activation_block, &block);
        activation_block.commit().unwrap();

        let view = state.view();
        let roster =
            <crate::state::StateView as crate::state::StakeSnapshot>::epoch_validator_peer_ids(
                &view, 0,
            )
            .expect("stake-elected roster should be present");
        assert_eq!(
            roster,
            vec![stake_peer],
            "admin-managed lanes must not alter stake-elected roster"
        );
        assert!(
            view.world
                .public_lane_validators()
                .get(&(stake_lane, validator.clone()))
                .is_some(),
            "stake-elected lane should retain the registered validator"
        );
        assert!(
            view.world
                .public_lane_validators()
                .get(&(admin_lane, delegator.clone()))
                .is_none(),
            "admin-managed lane must remain free of staking records"
        );
    }

    #[test]
    fn admin_multisig_register_peer_allowed_in_permissioned_mode() {
        let mut state = setup_state();
        let mut pipeline = state.view().pipeline().clone();
        pipeline.signature_batch_max_bls = 4;
        state.set_pipeline(pipeline);

        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        stx.nexus.enabled = true;
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(2_u32),
            vec![LaneConfig {
                id: LaneId::new(1),
                alias: "restricted".to_string(),
                dataspace_id: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Restricted,
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
        stx.nexus.staking.restricted_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;

        let domain_id: DomainId = "council".parse().expect("domain id");
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register domain");

        let member_a = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let member_b = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(member_a.public_key().clone(), 1).expect("member_a"),
                MultisigMember::new(member_b.public_key().clone(), 1).expect("member_b"),
            ],
        )
        .expect("policy");
        let admin_id = AccountId::new_multisig(domain_id.clone(), policy);
        Register::account(Account::new(admin_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register multisig admin");

        let bls = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = crate::PeerId::new(bls.public_key().clone());
        let pop = iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("pop");

        iroha_data_model::isi::register::RegisterPeerWithPop::new(peer_id.clone(), pop)
            .execute(&admin_id, &mut stx)
            .expect("admin multisig should register peer");
        assert!(stx.world.peers().iter().any(|peer| peer == &peer_id));

        Unregister::<Peer>::peer(peer_id.clone())
            .execute(&admin_id, &mut stx)
            .expect("admin multisig should unregister peer");
        assert!(stx.world.peers().iter().all(|peer| peer != &peer_id));
    }

    #[test]
    fn register_rejects_when_peer_missing_from_topology() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let validator_peer = crate::PeerId::from(
            validator
                .try_signatory()
                .expect("validator is single-signatory")
                .clone(),
        );

        let foreign_peer = crate::PeerId::from(KeyPair::random().public_key().clone());
        stx.commit_topology.get_mut().clear();
        stx.commit_topology.get_mut().push(foreign_peer);

        let result = RegisterPublicLaneValidator {
            lane_id: LaneId::new(42),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(
            result.is_err(),
            "validator registration should fail when commit topology lacks the peer"
        );

        let escrow_asset = AssetId::new(asset_def_id, escrow);
        assert!(
            stx.world.assets.get(&escrow_asset).is_none(),
            "rejected registration must not move stake into escrow"
        );
        assert!(
            !stx.commit_topology
                .iter()
                .any(|peer| peer == &validator_peer),
            "validator peer should remain absent from topology in rejection path"
        );
    }

    #[test]
    fn register_rejects_when_peer_unregistered() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let validator_peer = crate::PeerId::from(
            validator
                .try_signatory()
                .expect("validator is single-signatory")
                .clone(),
        );
        // Drop the validator peer from the WSV to emulate an unregistered topology entry.
        if let Some(pos) = stx
            .world
            .peers
            .iter()
            .position(|peer| peer == &validator_peer)
        {
            stx.world.peers.remove(pos);
        }
        stx.commit_topology
            .get_mut()
            .retain(|peer| peer != &validator_peer);

        let result = RegisterPublicLaneValidator {
            lane_id: LaneId::new(99),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(
            matches!(&result, Err(Error::InvariantViolation(msg)) if msg.contains("peer must be registered")),
            "unregistered peer should be rejected: {result:?}"
        );

        let escrow_asset = AssetId::new(asset_def_id, escrow);
        assert!(
            stx.world.assets.get(&escrow_asset).is_none(),
            "rejected registration must not move stake into escrow"
        );
    }

    #[test]
    fn pending_activation_advances_on_epoch_boundary() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 2);

        let mut state_block = state.block(block_header_with_height(1));
        let mut stx = state_block.transaction();

        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(1),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("register validator");
        let pending = stx
            .world
            .public_lane_validators
            .get(&(LaneId::new(1), validator.clone()))
            .expect("pending record");
        assert!(matches!(
            pending.status,
            PublicLaneValidatorStatus::PendingActivation(1)
        ));
        stx.apply();
        state_block.commit().unwrap();

        let mut activate_block = state.block(block_header_with_height(2));
        let mut activate_stx = activate_block.transaction();
        let err = ActivatePublicLaneValidator {
            lane_id: LaneId::new(1),
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut activate_stx)
        .expect_err("activation should wait for the next epoch");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg) if msg.contains("activation epoch not reached")
        ));
        activate_stx.apply();
        activate_block.commit().unwrap();

        let mut activate_block = state.block(block_header_with_height(3));
        let mut activate_stx = activate_block.transaction();
        ActivatePublicLaneValidator {
            lane_id: LaneId::new(1),
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut activate_stx)
        .expect("activate once pending epoch reached");
        let record = activate_stx
            .world
            .public_lane_validators
            .get(&(LaneId::new(1), validator.clone()))
            .cloned()
            .expect("record present");
        assert!(matches!(record.status, PublicLaneValidatorStatus::Active));
        assert_eq!(record.activation_epoch, Some(1));
        assert_eq!(record.activation_height, Some(3));
        activate_stx.apply();
        activate_block.commit().unwrap();

        let view = state.view();
        let stored = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(1), validator.clone()))
            .expect("stored record");
        assert_eq!(stored.activation_epoch, Some(1));
        assert_eq!(stored.activation_height, Some(3));

        let escrow_asset = AssetId::new(asset_def_id, escrow.clone());
        assert!(
            view.world.assets().get(&escrow_asset).is_some(),
            "staking assets must remain bonded"
        );
    }

    #[test]
    fn rewards_reject_non_active_validator() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.enabled = true;
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(2_u32),
            vec![LaneConfig {
                id: LaneId::new(1),
                alias: "rewards-lane".to_string(),
                dataspace_id: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Public,
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
        stx.nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&stx.nexus.lane_catalog);
        stx.nexus.staking.public_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected;

        let (validator, _delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        stx.nexus.fees.fee_sink_account_id = escrow.to_string();
        stx.nexus.fees.fee_asset_id = asset_def_id.to_string();
        let reward_asset = AssetId::new(asset_def_id.clone(), escrow.clone());
        Mint::asset_numeric(1_000u32, reward_asset.clone())
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(1),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("register validator");
        if let Some(record) = stx
            .world
            .public_lane_validators
            .get_mut(&(LaneId::new(1), validator.clone()))
        {
            record.status = PublicLaneValidatorStatus::PendingActivation(1);
            record.activation_epoch = None;
            record.activation_height = None;
        }

        let err = RecordPublicLaneRewards {
            lane_id: LaneId::new(1),
            epoch: 0,
            reward_asset,
            total_reward: Numeric::new(10, 0),
            shares: vec![PublicLaneRewardShare {
                account: validator,
                role: PublicLaneRewardRole::Validator,
                amount: Numeric::new(10, 0),
            }],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("inactive validator should not receive rewards");

        assert!(
            matches!(&err, Error::InvariantViolation(msg) if msg.contains("not active")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn pending_activation_auto_promotes_at_epoch_boundary() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 2);

        let block1 = new_block_with_height(1);
        let mut state_block = state.block(block1.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(1),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("register validator");
        stx.apply();
        state_block.commit().unwrap();

        let block2 = new_block_with_height(2);
        let mut state_block2 = state.block(block2.as_ref().header());
        let stx2 = state_block2.transaction();
        stx2.apply();
        state_block2.commit().unwrap();

        let block3 = new_block_with_height(3);
        let view = state.view();
        let pending_record = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(1), validator.clone()))
            .cloned()
            .expect("record present after scheduling");
        assert!(
            matches!(
                pending_record.status,
                PublicLaneValidatorStatus::PendingActivation(1)
            ),
            "validator should remain pending until the next epoch"
        );
        drop(view);

        let state_block3 = state.block(block3.as_ref().header());
        let record = state_block3
            .world
            .public_lane_validators
            .get(&(LaneId::new(1), validator.clone()))
            .cloned()
            .expect("record present after scheduling");
        assert!(
            matches!(record.status, PublicLaneValidatorStatus::Active),
            "validator should auto-activate at the next epoch boundary"
        );
        assert_eq!(record.activation_epoch, Some(1));
        assert_eq!(record.activation_height, Some(3));
        state_block3.commit().unwrap();

        let view = state.view();
        let stored = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(1), validator.clone()))
            .expect("stored record");
        assert!(
            matches!(stored.status, PublicLaneValidatorStatus::Active),
            "persistent record should remain active"
        );
        assert_eq!(stored.activation_epoch, Some(1));
        assert_eq!(stored.activation_height, Some(3));

        let escrow_asset = AssetId::new(asset_def_id, escrow);
        assert!(
            view.world.assets().get(&escrow_asset).is_some(),
            "staking assets must remain bonded"
        );
    }

    #[test]
    fn pending_activation_rejects_regressing_activation_height() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 2);

        let block1 = new_block_with_height(1);
        let mut state_block = state.block(block1.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, _, _) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(11),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("register validator");
        let key = (LaneId::new(11), validator.clone());
        stx.world
            .public_lane_validators
            .get_mut(&key)
            .expect("pending record")
            .activation_height = Some(99);
        stx.apply();
        state_block.commit().unwrap();

        // Insert an intermediate block to keep heights monotonic before the activation block.
        let block2 = new_block_with_height(2);
        let mut mid_block = state.block(block2.as_ref().header());
        let mid_tx = mid_block.transaction();
        mid_tx.apply();
        mid_block.commit().unwrap();

        let block3 = new_block_with_height(3);
        let mut activation_block = state.block(block3.as_ref().header());
        let mut stx3 = activation_block.transaction();
        let err = finalize_pending_activations(&mut stx3)
            .expect_err("regressing activation height must be rejected");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg) if msg.contains("activation height cannot regress")
        ));
    }

    #[test]
    fn unregister_peer_frees_validator_capacity() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);

        let (validator, _, _escrow, asset_def_id) = prepare_accounts(&mut stx);
        let (replacement, _kp) = gen_account_in("nexus");
        Register::account(Account::new(replacement.clone()))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        register_peer_for_account(&mut stx, &replacement);
        stx.commit_topology.get_mut().push(crate::PeerId::from(
            replacement
                .try_signatory()
                .expect("replacement is single-signatory")
                .clone(),
        ));
        Mint::asset_numeric(
            10_000u32,
            AssetId::new(asset_def_id.clone(), replacement.clone()),
        )
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        RegisterPublicLaneValidator {
            lane_id: LaneId::new(7),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("first validator should register");

        let second_attempt = RegisterPublicLaneValidator {
            lane_id: LaneId::new(7),
            validator: replacement.clone(),
            stake_account: replacement.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(
            matches!(&second_attempt, Err(Error::InvariantViolation(msg)) if msg.contains("maximum validator capacity")),
            "capacity guard should reject second validator: {second_attempt:?}"
        );

        Unregister::<Peer>::peer(crate::PeerId::from(
            validator
                .try_signatory()
                .expect("validator is single-signatory")
                .clone(),
        ))
        .execute(&ALICE_ID, &mut stx)
        .expect("unregister peer");

        RegisterPublicLaneValidator {
            lane_id: LaneId::new(7),
            validator: replacement.clone(),
            stake_account: replacement.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("second validator should register after removal");

        let former = stx
            .world
            .public_lane_validators
            .get(&(LaneId::new(7), validator.clone()))
            .expect("former validator record");
        assert!(
            matches!(former.status, PublicLaneValidatorStatus::Exited),
            "unregistered peer should mark validator as exited"
        );
        let remaining_shares: Vec<_> = stx
            .world
            .public_lane_stake_shares
            .iter()
            .filter(|((lane, val, _), _)| *lane == LaneId::new(7) && val == &validator)
            .collect();
        assert!(
            remaining_shares.is_empty(),
            "stake shares for the removed peer must be cleared"
        );
    }

    #[test]
    fn exiting_validators_finalize_before_capacity_check() {
        let state = setup_state();
        let block = new_block_with_height_and_time(1, 0);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);

        let (validator, _delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(0);
        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("register validator");

        ExitPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            release_at_ms: stx.block_unix_timestamp_ms().saturating_add(5),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("mark exiting validator");

        stx.apply();
        state_block.commit().unwrap();

        let block2 = new_block_with_height_and_time(2, 10);
        let mut state_block = state.block(block2.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);

        let (replacement, _kp) = gen_account_in("nexus");
        Register::account(Account::new(replacement.clone()))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        let replacement_peer = register_peer_for_account(&mut stx, &replacement);
        stx.commit_topology.get_mut().push(replacement_peer);
        stx.nexus.staking.stake_asset_id = asset_def_id.to_string();
        stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        stx.nexus.staking.slash_sink_account_id = escrow.to_string();
        finalize_validator_lifecycle(&mut stx).expect("finalize lifecycle before replacement");
        Mint::asset_numeric(
            10_000u32,
            AssetId::new(asset_def_id.clone(), replacement.clone()),
        )
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        RegisterPublicLaneValidator {
            lane_id,
            validator: replacement.clone(),
            stake_account: replacement.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("replacement should register after exit release");

        let stale_record = stx
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()));
        assert!(
            stale_record.is_none()
                || matches!(stale_record, Some(record) if matches!(record.status, PublicLaneValidatorStatus::Exited)),
            "exited validator should be pruned before replacement insert"
        );
        let remaining_shares: Vec<_> = stx
            .world
            .public_lane_stake_shares
            .iter()
            .filter(|((lane, val, _), _)| *lane == lane_id && val == &validator)
            .collect();
        assert!(
            remaining_shares.is_empty(),
            "exited validator stake shares must be cleared"
        );
    }

    #[test]
    fn slashed_validator_can_exit_and_reregister() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 1);

        let block = new_block_with_height_and_time(1, 0);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);

        let (validator, _delegator, escrow, _asset_def_id) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(12);
        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("register validator");
        stx.apply();
        state_block.commit().unwrap();

        let block2 = new_block_with_height_and_time(2, 5);
        let mut state_block = state.block(block2.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);
        stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        stx.nexus.staking.slash_sink_account_id = escrow.to_string();

        SlashPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            slash_id: Hash::new("slash-reenter"),
            amount: Numeric::new(100, 0),
            reason_code: "violation".to_string(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("slash validator");

        ExitPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            release_at_ms: stx.block_unix_timestamp_ms(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("exit slashed validator");
        finalize_validator_lifecycle(&mut stx).expect("finalize exit lifecycle");
        stx.apply();
        state_block.commit().unwrap();

        let exited = state
            .view()
            .world
            .public_lane_validators()
            .get(&(lane_id, validator.clone()))
            .cloned()
            .expect("exited record retained");
        assert!(
            matches!(exited.status, PublicLaneValidatorStatus::Exited),
            "validator should be marked exited after exit finalizer"
        );

        let block3 = new_block_with_height_and_time(3, 10);
        let mut state_block = state.block(block3.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);
        stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        stx.nexus.staking.slash_sink_account_id = escrow.to_string();

        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("re-register exited validator after slash");

        let record = stx
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()))
            .expect("re-registered record");
        assert!(
            matches!(
                record.status,
                PublicLaneValidatorStatus::PendingActivation(_)
            ),
            "re-registration should restart at pending activation"
        );
        let shares: Vec<_> = stx
            .world
            .public_lane_stake_shares
            .iter()
            .filter(|((lane, val, _), _)| *lane == lane_id && val == &validator)
            .collect();
        assert_eq!(
            shares.len(),
            1,
            "stake shares must be rebuilt for the re-registered validator"
        );

        stx.apply();
        state_block.commit().unwrap();
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn exiting_validator_blocks_capacity_until_release() {
        let state = setup_state();
        let block = new_block_with_height_and_time(1, 0);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);

        let (validator, _delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(10);
        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("register validator");

        let release_at_ms = stx.block_unix_timestamp_ms().saturating_add(50);
        ExitPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            release_at_ms,
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("mark validator exiting");

        stx.apply();
        state_block.commit().unwrap();

        // Prepare a replacement validator with bonded stake and a registered peer.
        let block2 = new_block_with_height_and_time(2, 10);
        let mut state_block = state.block(block2.as_ref().header());
        let replacement = {
            let mut stx = state_block.transaction();
            let (replacement, _kp) = gen_account_in("nexus");
            Register::account(Account::new(replacement.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            let replacement_peer = register_peer_for_account(&mut stx, &replacement);
            stx.commit_topology.get_mut().push(replacement_peer.clone());
            stx.nexus.staking.stake_asset_id = asset_def_id.to_string();
            stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
            stx.nexus.staking.slash_sink_account_id = escrow.to_string();
            Mint::asset_numeric(
                10_000u32,
                AssetId::new(asset_def_id.clone(), replacement.clone()),
            )
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
            stx.apply();
            replacement
        };
        state_block.commit().unwrap();

        // Capacity is still consumed while the original validator is exiting.
        let block3 = new_block_with_height_and_time(3, 10);
        let mut state_block = state.block(block3.as_ref().header());
        {
            let mut stx = state_block.transaction();
            stx.nexus.staking.max_validators = nonzero!(1u32);
            stx.nexus.staking.stake_asset_id = asset_def_id.to_string();
            stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
            stx.nexus.staking.slash_sink_account_id = escrow.to_string();
            let err = RegisterPublicLaneValidator {
                lane_id,
                validator: replacement.clone(),
                stake_account: replacement.clone(),
                initial_stake: Numeric::new(1_000, 0),
                metadata: Metadata::default(),
            }
            .execute(&ALICE_ID, &mut stx)
            .expect_err("capacity should remain consumed until exit finalizes");
            assert!(matches!(
                err,
                Error::InvariantViolation(msg) if msg.contains("maximum validator capacity")
            ));
            drop(stx);
        }
        state_block.commit().unwrap();

        // After the release timestamp elapses, exit finalization frees capacity.
        let block4 = new_block_with_height_and_time(4, 60);
        let mut state_block = state.block(block4.as_ref().header());
        {
            let mut stx = state_block.transaction();
            stx.nexus.staking.max_validators = nonzero!(1u32);
            stx.nexus.staking.stake_asset_id = asset_def_id.to_string();
            stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
            stx.nexus.staking.slash_sink_account_id = escrow.to_string();
            RegisterPublicLaneValidator {
                lane_id,
                validator: replacement.clone(),
                stake_account: replacement.clone(),
                initial_stake: Numeric::new(1_000, 0),
                metadata: Metadata::default(),
            }
            .execute(&ALICE_ID, &mut stx)
            .expect("capacity should free after exit release");
            stx.apply();
        }
        state_block.commit().unwrap();

        let view = state.view();
        let roster = view
            .world
            .public_lane_validators()
            .iter()
            .filter(|((lane, _), _)| *lane == lane_id)
            .map(|((_, id), record)| (id.clone(), record.status.clone()))
            .collect::<Vec<_>>();
        assert!(
            roster.iter().any(|(id, status)| id == &replacement
                && matches!(status, PublicLaneValidatorStatus::PendingActivation(_))),
            "replacement should be admitted after exit finalizes"
        );
        assert!(
            roster.iter().any(|(id, status)| id == &validator
                && matches!(status, PublicLaneValidatorStatus::Exited)),
            "exiting validator should finalize to Exited"
        );
    }

    #[test]
    fn stake_snapshot_ignores_disabled_peer() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        stx.nexus.enabled = true;
        let lane_id = LaneId::new(15);
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(16_u32),
            vec![LaneConfig {
                id: lane_id,
                alias: "stake-snapshot-15".to_string(),
                dataspace_id: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Public,
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
        stx.nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&stx.nexus.lane_catalog);
        stx.nexus.staking.public_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected;

        let (validator, _, _escrow, _asset_def_id) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        ActivatePublicLaneValidator {
            lane_id,
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        stx.apply();
        record_block_commit(&mut state_block, &block);
        state_block.commit().unwrap();

        let validator_peer = crate::PeerId::from(
            validator
                .try_signatory()
                .expect("validator is single-signatory")
                .clone(),
        );
        {
            let view = state.view();
            let roster = crate::state::StakeSnapshot::epoch_validator_peer_ids(&view, 0)
                .expect("active validator should appear before key disable");
            assert!(
                roster.contains(&validator_peer),
                "active validator should be present before disable"
            );
        }

        let block = new_block_with_height(2);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        seed_validator_consensus_key(&mut stx, &validator_peer, ConsensusKeyStatus::Disabled);
        stx.apply();
        record_block_commit(&mut state_block, &block);
        state_block.commit().unwrap();

        let view = state.view();
        let roster =
            crate::state::StakeSnapshot::epoch_validator_peer_ids(&view, 0).unwrap_or_default();
        assert!(
            !roster.contains(&validator_peer),
            "disabled consensus key should remove validator from snapshot"
        );
    }

    #[test]
    fn stake_snapshot_widens_missing_validator_from_topology() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        stx.nexus.enabled = true;
        let lane_id = LaneId::new(16);
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(17_u32),
            vec![LaneConfig {
                id: lane_id,
                alias: "stake-snapshot-16".to_string(),
                dataspace_id: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Public,
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
        stx.nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&stx.nexus.lane_catalog);
        stx.nexus.staking.public_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected;

        let (validator, _, _, _asset_def_id) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        ActivatePublicLaneValidator {
            lane_id,
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        stx.apply();
        record_block_commit(&mut state_block, &block);
        state_block.commit().unwrap();

        let validator_peer = crate::PeerId::from(
            validator
                .try_signatory()
                .expect("validator is single-signatory")
                .clone(),
        );
        {
            let view = state.view();
            let roster = crate::state::StakeSnapshot::epoch_validator_peer_ids(&view, 0)
                .expect("validator should appear before topology change");
            assert!(
                roster.contains(&validator_peer),
                "validator should be present before topology change"
            );
        }

        let block = new_block_with_height(2);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let foreign_peer = crate::PeerId::from(KeyPair::random().public_key().clone());
        let _ = stx.world.peers.push(foreign_peer.clone());
        stx.commit_topology.get_mut().clear();
        stx.commit_topology.get_mut().push(foreign_peer);
        stx.apply();
        record_block_commit(&mut state_block, &block);
        state_block.commit().unwrap();

        let view = state.view();
        let roster =
            crate::state::StakeSnapshot::epoch_validator_peer_ids(&view, 0).unwrap_or_default();
        assert!(
            roster.contains(&validator_peer),
            "active validator should remain in the snapshot even if commit topology omits it"
        );
    }

    #[test]
    fn stake_snapshot_drops_validator_when_peer_removed() {
        let state = setup_state();
        let block = new_block_with_height(1);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        stx.nexus.enabled = true;
        let lane_id = LaneId::new(17);
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(18_u32),
            vec![LaneConfig {
                id: lane_id,
                alias: "stake-snapshot-17".to_string(),
                dataspace_id: DataSpaceId::GLOBAL,
                visibility: LaneVisibility::Public,
                ..LaneConfig::default()
            }],
        )
        .expect("lane catalog");
        stx.nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&stx.nexus.lane_catalog);
        stx.nexus.staking.public_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected;

        let (validator, _, _, _asset_def_id) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        ActivatePublicLaneValidator {
            lane_id,
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        stx.apply();
        record_block_commit(&mut state_block, &block);
        state_block.commit().unwrap();

        let validator_peer = crate::PeerId::from(
            validator
                .try_signatory()
                .expect("validator is single-signatory")
                .clone(),
        );
        {
            let view = state.view();
            let roster = crate::state::StakeSnapshot::epoch_validator_peer_ids(&view, 0)
                .expect("validator should appear before peer removal");
            assert!(
                roster.contains(&validator_peer),
                "validator should be present before peer removal"
            );
        }

        let block = new_block_with_height(2);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        if let Some(pos) = stx
            .world
            .peers
            .iter()
            .position(|peer| peer == &validator_peer)
        {
            stx.world.peers.remove(pos);
        }
        stx.apply();
        record_block_commit(&mut state_block, &block);
        state_block.commit().unwrap();

        let view = state.view();
        let roster =
            crate::state::StakeSnapshot::epoch_validator_peer_ids(&view, 0).unwrap_or_default();
        assert!(
            !roster.contains(&validator_peer),
            "validator should be dropped when its peer is removed from WSV"
        );
        let record = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(17), validator.clone()))
            .expect("validator record persists");
        assert!(
            matches!(
                record.status,
                PublicLaneValidatorStatus::Active | PublicLaneValidatorStatus::PendingActivation(_)
            ),
            "peer removal gating should not mutate validator status"
        );
    }

    #[test]
    fn bond_and_unbond_updates_totals() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(7),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(500, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        BondPublicLaneStake {
            lane_id: LaneId::new(7),
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Numeric::new(250, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        let release_at_ms = stx.block_unix_timestamp_ms();
        SchedulePublicLaneUnbond {
            lane_id: LaneId::new(7),
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("req"),
            amount: Numeric::new(100, 0),
            release_at_ms,
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        FinalizePublicLaneUnbond {
            lane_id: LaneId::new(7),
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("req"),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        assert!(
            stx.world
                .public_lane_validators
                .get(&(LaneId::new(7), validator.clone()))
                .is_some(),
            "validator not stored before apply"
        );

        stx.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let record = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(7), validator.clone()))
            .unwrap();
        assert_eq!(record.total_stake, Numeric::new(650, 0));

        let share = view
            .world
            .public_lane_stake_shares()
            .get(&(LaneId::new(7), validator.clone(), delegator.clone()))
            .unwrap();
        assert_eq!(share.bonded, Numeric::new(150, 0));
        assert!(share.pending_unbonds.is_empty());

        let escrow_asset = AssetId::new(asset_def_id.clone(), escrow.clone());
        let validator_stake = AssetId::new(asset_def_id.clone(), validator.clone());
        let delegator_stake = AssetId::new(asset_def_id, delegator.clone());
        let escrow_balance = view
            .world
            .assets()
            .get(&escrow_asset)
            .expect("escrow balance after unbond");
        assert_eq!(escrow_balance.as_ref(), &Numeric::new(650, 0));
        let validator_balance = view
            .world
            .assets()
            .get(&validator_stake)
            .expect("validator free stake");
        let delegator_balance = view
            .world
            .assets()
            .get(&delegator_stake)
            .expect("delegator free stake");
        assert_eq!(validator_balance.as_ref(), &Numeric::new(9_500, 0));
        assert_eq!(delegator_balance.as_ref(), &Numeric::new(9_850, 0));
    }

    #[test]
    fn register_requires_peer() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, _, _) = prepare_accounts(&mut stx);
        stx.world.peers.clear();
        let res = RegisterPublicLaneValidator {
            lane_id: LaneId::new(42),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(res.is_err(), "expected register to fail when peer missing");
    }

    #[test]
    fn activate_transitions_pending_to_active() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, _, _) = prepare_accounts(&mut stx);
        // Register matching peer so validator admission passes.
        let peer_id = crate::PeerId::from(validator.signatory().clone());
        let _ = stx.world.peers.push(peer_id);

        RegisterPublicLaneValidator {
            lane_id: LaneId::new(2),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        ActivatePublicLaneValidator {
            lane_id: LaneId::new(2),
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        stx.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let record = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(2), validator.clone()))
            .unwrap();
        assert!(matches!(record.status, PublicLaneValidatorStatus::Active));
    }

    #[test]
    fn exit_sets_exited_and_allows_reregister() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let peer_id = crate::PeerId::from(validator.signatory().clone());
        let _ = stx.world.peers.push(peer_id);

        RegisterPublicLaneValidator {
            lane_id: LaneId::new(3),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        let now_ms = stx.block_unix_timestamp_ms();
        ExitPublicLaneValidator {
            lane_id: LaneId::new(3),
            validator: validator.clone(),
            release_at_ms: now_ms,
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        // Re-register should succeed once exited.
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(3),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(500, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("re-register after exit");
    }

    #[test]
    fn slashed_validator_must_exit_before_reregistering() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let lane_id = LaneId::new(34);
        let (validator, _, _, asset_def_id) = prepare_accounts(&mut stx);
        let peer_id = crate::PeerId::from(validator.signatory().clone());
        let _ = stx.world.peers.push(peer_id);

        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        SlashPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            slash_id: Hash::new("slash-reason"),
            amount: Numeric::new(100, 0),
            reason_code: "evidence".to_string(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("slash succeeds");

        let err = RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(250, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("slashed validator should remain registered until exited");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg) if msg.contains("already registered")
        ));

        let now_ms = stx.block_unix_timestamp_ms();
        ExitPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            release_at_ms: now_ms,
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("exit slashed validator");

        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(250, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("re-register after exit");

        stx.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let record = view
            .world
            .public_lane_validators()
            .get(&(lane_id, validator.clone()))
            .expect("validator record");
        assert!(
            matches!(
                record.status,
                PublicLaneValidatorStatus::PendingActivation(_)
            ),
            "re-registration should reset lifecycle after exit"
        );
        let stake_balance = view
            .world
            .assets()
            .get(&AssetId::new(asset_def_id, validator.clone()))
            .expect("stake balance tracked");
        assert!(
            stake_balance.as_ref() < &Numeric::new(10_000, 0),
            "stake should have been withdrawn for re-registration"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn slashed_exit_with_release_timer_frees_capacity() {
        let state = setup_state();
        let block = new_block_with_height_and_time(1, 1_000);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);

        let lane_id = LaneId::new(35);
        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);

        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        SlashPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            slash_id: Hash::new("slash-release-gate"),
            amount: Numeric::new(200, 0),
            reason_code: "evidence".to_string(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        let release_at_ms = stx.block_unix_timestamp_ms().saturating_add(5_000);
        ExitPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            release_at_ms,
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        stx.apply();
        state_block.commit().unwrap();

        // Prepare replacement validator in a dedicated setup block.
        let setup_block = new_block_with_height_and_time(2, release_at_ms.saturating_sub(2_000));
        let mut setup_state_block = state.block(setup_block.as_ref().header());
        let mut setup_tx = setup_state_block.transaction();
        setup_tx.nexus.staking.max_validators = nonzero!(1u32);
        setup_tx.nexus.staking.stake_asset_id = asset_def_id.to_string();
        setup_tx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        setup_tx.nexus.staking.slash_sink_account_id = escrow.to_string();
        let (replacement, _replacement_kp) = gen_account_in("nexus");
        Register::account(Account::new(replacement.clone()))
            .execute(&ALICE_ID, &mut setup_tx)
            .unwrap();
        let replacement_peer = register_peer_for_account(&mut setup_tx, &replacement);
        setup_tx
            .commit_topology
            .get_mut()
            .push(replacement_peer.clone());
        Mint::asset_numeric(
            10_000u32,
            AssetId::new(asset_def_id.clone(), replacement.clone()),
        )
        .execute(&ALICE_ID, &mut setup_tx)
        .unwrap();
        setup_tx.apply();
        setup_state_block.commit().unwrap();

        let prerelease_block = new_block_with_height_and_time(3, release_at_ms.saturating_sub(1));
        let mut prerelease_state_block = state.block(prerelease_block.as_ref().header());
        {
            let mut prerelease_tx = prerelease_state_block.transaction();
            prerelease_tx.nexus.staking.max_validators = nonzero!(1u32);
            prerelease_tx.nexus.staking.stake_asset_id = asset_def_id.to_string();
            prerelease_tx.nexus.staking.stake_escrow_account_id = escrow.to_string();
            prerelease_tx.nexus.staking.slash_sink_account_id = escrow.to_string();
            let err = RegisterPublicLaneValidator {
                lane_id,
                validator: replacement.clone(),
                stake_account: replacement.clone(),
                initial_stake: Numeric::new(1_000, 0),
                metadata: Metadata::default(),
            }
            .execute(&ALICE_ID, &mut prerelease_tx)
            .expect_err("capacity should remain full until release time");
            assert!(
                matches!(
                    err,
                    Error::InvariantViolation(ref msg) if msg.contains("maximum validator capacity")
                ),
                "unexpected prerelease error: {err:?}"
            );
            prerelease_tx.apply();
        }
        prerelease_state_block.commit().unwrap();

        let post_block = new_block_with_height_and_time(4, release_at_ms.saturating_add(1));
        let mut post_state_block = state.block(post_block.as_ref().header());
        let mut post_tx = post_state_block.transaction();
        post_tx.nexus.staking.max_validators = nonzero!(1u32);
        post_tx.nexus.staking.stake_asset_id = asset_def_id.to_string();
        post_tx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        post_tx.nexus.staking.slash_sink_account_id = escrow.to_string();
        if !post_tx.commit_topology.get().contains(&replacement_peer) {
            post_tx
                .commit_topology
                .get_mut()
                .push(replacement_peer.clone());
        }
        RegisterPublicLaneValidator {
            lane_id,
            validator: replacement.clone(),
            stake_account: replacement.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut post_tx)
        .expect("capacity should free after exit release");

        let previous = post_tx
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()))
            .expect("slashed validator record still present");
        assert!(
            matches!(previous.status, PublicLaneValidatorStatus::Exited),
            "release should finalize exit before replacement registers"
        );

        post_tx.apply();
        post_state_block.commit().unwrap();

        let view = state.view();
        assert!(
            view.world
                .public_lane_validators()
                .get(&(lane_id, replacement))
                .is_some(),
            "replacement should register once exit release matures"
        );
    }

    #[test]
    fn unregister_peer_exits_validator_and_drops_roster() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let peer_id = crate::PeerId::from(validator.signatory().clone());

        RegisterPublicLaneValidator {
            lane_id: LaneId::new(31),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        ActivatePublicLaneValidator {
            lane_id: LaneId::new(31),
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        {
            use iroha_data_model::isi::register::Unregister;
            Unregister::<Peer>::peer(peer_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
        }

        stx.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let record = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(31), validator.clone()))
            .expect("validator record after peer removal");
        assert!(matches!(record.status, PublicLaneValidatorStatus::Exited));

        let roster =
            <crate::state::StateView as crate::state::StakeSnapshot>::epoch_validator_peer_ids(
                &view, 0,
            );
        assert!(
            roster
                .as_ref()
                .is_none_or(|peers| !peers.contains(&peer_id)),
            "peer should not appear in roster after unregistration"
        );
    }

    #[test]
    fn unregister_peer_removes_stake_shares() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let lane_id = LaneId::new(32);
        let (validator, delegator, _, _) = prepare_accounts(&mut stx);
        let peer_id = crate::PeerId::from(validator.signatory().clone());

        RegisterPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        ActivatePublicLaneValidator {
            lane_id,
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        BondPublicLaneStake {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Numeric::new(250, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        // Ensure stake shares exist before unregistration.
        let pre_exit_shares = stx
            .world
            .public_lane_stake_shares
            .iter()
            .filter(|((lane, val, _), _)| *lane == lane_id && val == &validator)
            .count();
        assert!(
            pre_exit_shares > 0,
            "expected bonded stake shares prior to unregister"
        );

        {
            use iroha_data_model::isi::register::Unregister;
            Unregister::<Peer>::peer(peer_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
        }

        stx.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let remaining_shares = view
            .world
            .public_lane_stake_shares()
            .iter()
            .filter(|((lane, val, _), _)| *lane == lane_id && val == &validator)
            .count();
        assert_eq!(
            remaining_shares, 0,
            "stake shares for exited validator must be pruned"
        );

        let record = view
            .world
            .public_lane_validators()
            .get(&(lane_id, validator.clone()))
            .expect("validator record after removal");
        assert!(matches!(record.status, PublicLaneValidatorStatus::Exited));
    }

    #[test]
    fn bond_rejects_without_funds() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, delegator, _, asset_def_id) = prepare_accounts(&mut stx);
        let delegator_asset = AssetId::new(asset_def_id.clone(), delegator.clone());
        stx.world
            .withdraw_numeric_asset(&delegator_asset, &Numeric::new(10_000, 0))
            .expect("drain delegator funds");

        RegisterPublicLaneValidator {
            lane_id: LaneId::new(12),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(500, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        let res = BondPublicLaneStake {
            lane_id: LaneId::new(12),
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Numeric::new(500, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(
            matches!(res, Err(Error::Math(MathError::NotEnoughQuantity))),
            "expected bond to fail when staker lacks funds"
        );
    }

    #[test]
    fn register_rejects_below_min_stake() {
        let mut state = setup_state();
        state.nexus.get_mut().staking.min_validator_stake = 2_000;
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let res = RegisterPublicLaneValidator {
            lane_id: LaneId::new(3),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(res.is_err(), "expected min stake rejection");
    }

    #[test]
    fn register_rejects_when_peer_missing() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let missing_peer = crate::PeerId::from(
            validator
                .try_signatory()
                .expect("validator has signatory")
                .clone(),
        );
        if let Some(index) = stx
            .world
            .peers
            .iter()
            .position(|peer| peer == &missing_peer)
        {
            stx.world.peers.remove(index);
        }

        let res = RegisterPublicLaneValidator {
            lane_id: LaneId::new(42),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(
            matches!(res, Err(Error::InvariantViolation(ref msg)) if msg.contains("peer")),
            "expected missing peer rejection, got {res:?}"
        );
    }

    #[test]
    fn unbond_respects_delay() {
        let mut state = setup_state();
        state.nexus.get_mut().staking.unbonding_delay = Duration::from_secs(10);
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, delegator, _, _) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(4),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        let release_at_ms = stx.block_unix_timestamp_ms();
        let res = SchedulePublicLaneUnbond {
            lane_id: LaneId::new(4),
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("req-delay"),
            amount: Numeric::new(100, 0),
            release_at_ms,
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(res.is_err(), "expected unbond delay enforcement");
    }

    #[test]
    fn slash_transfers_funds_to_sink() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        // Route slashes to the delegator account to ensure the transfer is observable.
        stx.nexus.staking.slash_sink_account_id = delegator.to_string();

        RegisterPublicLaneValidator {
            lane_id: LaneId::new(13),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        BondPublicLaneStake {
            lane_id: LaneId::new(13),
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Numeric::new(500, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        SlashPublicLaneValidator {
            lane_id: LaneId::new(13),
            validator: validator.clone(),
            slash_id: Hash::new("slash-escrow"),
            amount: Numeric::new(400, 0),
            reason_code: "evidence".to_string(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        stx.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let escrow_asset = AssetId::new(asset_def_id.clone(), escrow.clone());
        let sink_asset = AssetId::new(asset_def_id.clone(), delegator.clone());
        let escrow_balance = view
            .world
            .assets()
            .get(&escrow_asset)
            .expect("escrow asset after slash");
        let sink_balance = view
            .world
            .assets()
            .get(&sink_asset)
            .expect("slash sink asset");
        assert_eq!(escrow_balance.as_ref(), &Numeric::new(1_100, 0));
        assert_eq!(sink_balance.as_ref(), &Numeric::new(9_900, 0));
    }

    #[test]
    fn claim_rewards_transfers_and_marks_epoch() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (_sink, validator, reward_asset, asset_def_id) =
            configure_reward_fixture(&mut stx, LaneId::new(0), 1_000);
        stx.nexus.staking.reward_dust_threshold = 0;

        let share = PublicLaneRewardShare {
            account: validator.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: Numeric::new(150, 0),
        };
        RecordPublicLaneRewards {
            lane_id: LaneId::new(0),
            epoch: 1,
            reward_asset: reward_asset.clone(),
            total_reward: Numeric::new(150, 0),
            shares: vec![share],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        ClaimPublicLaneRewards {
            lane_id: LaneId::new(0),
            account: validator.clone(),
            upto_epoch: Some(1),
        }
        .execute(&validator, &mut stx)
        .unwrap();

        stx.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let claimed = view
            .world
            .public_lane_reward_claims()
            .get(&(LaneId::new(0), validator.clone(), reward_asset.clone()))
            .copied()
            .expect("claim marker");
        assert_eq!(claimed, 1);

        let validator_asset = AssetId::new(asset_def_id.clone(), validator.clone());
        let balance = view
            .world
            .assets()
            .get(&validator_asset)
            .expect("validator reward asset");
        assert_eq!(balance.as_ref(), &Numeric::new(150, 0));
    }

    #[test]
    fn claim_rewards_skips_dust() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (_sink, validator, reward_asset, asset_def_id) =
            configure_reward_fixture(&mut stx, LaneId::new(11), 500);
        stx.nexus.staking.reward_dust_threshold = 100;

        let share = PublicLaneRewardShare {
            account: validator.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: Numeric::new(50, 0),
        };
        RecordPublicLaneRewards {
            lane_id: LaneId::new(11),
            epoch: 1,
            reward_asset: reward_asset.clone(),
            total_reward: Numeric::new(50, 0),
            shares: vec![share],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        ClaimPublicLaneRewards {
            lane_id: LaneId::new(11),
            account: validator.clone(),
            upto_epoch: Some(1),
        }
        .execute(&validator, &mut stx)
        .unwrap();

        stx.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let claimed = view
            .world
            .public_lane_reward_claims()
            .get(&(LaneId::new(11), validator.clone(), reward_asset.clone()))
            .copied()
            .expect("claim marker");
        assert_eq!(claimed, 1);

        let validator_asset = AssetId::new(asset_def_id.clone(), validator.clone());
        assert!(
            view.world.assets().get(&validator_asset).is_none(),
            "dust claim should not transfer funds"
        );
    }

    #[test]
    fn claim_rewards_accepts_ih58_fee_sink() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        iroha_data_model::account::clear_account_domain_selector_resolver();

        let (_sink, validator, reward_asset, _asset_def_id) =
            configure_reward_fixture(&mut stx, LaneId::new(12), 200);
        stx.nexus.staking.reward_dust_threshold = 0;

        let share = PublicLaneRewardShare {
            account: validator.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: Numeric::new(10, 0),
        };
        RecordPublicLaneRewards {
            lane_id: LaneId::new(12),
            epoch: 1,
            reward_asset: reward_asset.clone(),
            total_reward: Numeric::new(10, 0),
            shares: vec![share],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        ClaimPublicLaneRewards {
            lane_id: LaneId::new(12),
            account: validator.clone(),
            upto_epoch: Some(1),
        }
        .execute(&validator, &mut stx)
        .unwrap();

        stx.apply();
        state_block.commit().unwrap();

        let view = state.view();
        let claimed = view
            .world
            .public_lane_reward_claims()
            .get(&(LaneId::new(12), validator.clone(), reward_asset.clone()))
            .copied()
            .expect("claim marker");
        assert_eq!(claimed, 1);
    }

    #[test]
    fn slash_rejects_above_max_ratio() {
        let mut state = setup_state();
        state.nexus.get_mut().staking.max_slash_bps = 1_000; // 10%
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, _, _) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(5),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Numeric::new(1_000, 0),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();

        let res = SlashPublicLaneValidator {
            lane_id: LaneId::new(5),
            validator: validator.clone(),
            slash_id: Hash::new("slash"),
            amount: Numeric::new(200, 0),
            reason_code: "double_sign".to_string(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(res.is_err(), "expected slash ratio guard to reject");
    }

    #[test]
    fn record_rewards_rejects_underfunded_sink() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (_sink, validator, reward_asset, _) =
            configure_reward_fixture(&mut stx, LaneId::new(11), 50);

        let share = PublicLaneRewardShare {
            account: validator.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: Numeric::new(100, 0),
        };
        let res = RecordPublicLaneRewards {
            lane_id: LaneId::new(7),
            epoch: 1,
            reward_asset,
            total_reward: Numeric::new(100, 0),
            shares: vec![share],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(
            res.is_err(),
            "expected underfunded reward record to be rejected"
        );
    }

    #[test]
    fn record_rewards_rejects_stale_epoch() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (_sink, validator, reward_asset, _) =
            configure_reward_fixture(&mut stx, LaneId::new(8), 500);

        let share = PublicLaneRewardShare {
            account: validator.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: Numeric::new(100, 0),
        };
        RecordPublicLaneRewards {
            lane_id: LaneId::new(8),
            epoch: 2,
            reward_asset: reward_asset.clone(),
            total_reward: Numeric::new(100, 0),
            shares: vec![share.clone()],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("initial record");

        let stale_record = RecordPublicLaneRewards {
            lane_id: LaneId::new(8),
            epoch: 1,
            reward_asset,
            total_reward: Numeric::new(100, 0),
            shares: vec![share],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(stale_record.is_err(), "expected stale epoch rejection");
    }

    #[test]
    fn record_rewards_rejects_zero_share_amounts() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (_sink, validator, reward_asset, _) =
            configure_reward_fixture(&mut stx, LaneId::new(0), 100);

        let share = PublicLaneRewardShare {
            account: validator.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: Numeric::zero(),
        };
        let res = RecordPublicLaneRewards {
            lane_id: LaneId::new(10),
            epoch: 1,
            reward_asset,
            total_reward: Numeric::new(50, 0),
            shares: vec![share],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);

        assert!(res.is_err(), "expected zero-share reward to be rejected");
    }

    #[test]
    fn cancel_consensus_evidence_penalty_marks_record() {
        let state = setup_state();
        let evidence = Evidence {
            kind: EvidenceKind::InvalidProposal,
            payload: EvidencePayload::InvalidProposal {
                proposal: Proposal {
                    header: ConsensusBlockHeader {
                        parent_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; 32])),
                        tx_root: Hash::prehashed([0xBB; 32]),
                        state_root: Hash::prehashed([0xCC; 32]),
                        proposer: 0,
                        height: 1,
                        view: 0,
                        epoch: 0,
                        highest_qc: QcRef {
                            height: 0,
                            view: 0,
                            epoch: 0,
                            subject_block_hash: HashOf::from_untyped_unchecked(Hash::prehashed(
                                [0xDD; 32],
                            )),
                            phase: CertPhase::Prepare,
                        },
                    },
                    payload_hash: Hash::prehashed([0xEE; 32]),
                },
                reason: "governance-cancel".to_string(),
            },
        };
        let record = EvidenceRecord {
            evidence: evidence.clone(),
            recorded_at_height: 1,
            recorded_at_view: 0,
            recorded_at_ms: 0,
            penalty_applied: false,
            penalty_cancelled: false,
            penalty_cancelled_at_height: None,
            penalty_applied_at_height: None,
        };
        let key = evidence_key(&record.evidence);
        {
            let mut block = state.world.consensus_evidence.block();
            block.insert(key.clone(), record);
            block.commit();
        }

        let block = new_block_with_height(2);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        CancelConsensusEvidencePenalty { evidence }
            .execute(&ALICE_ID, &mut stx)
            .expect("cancel evidence penalty");
        stx.apply();
        state_block.commit().unwrap();

        let view = state.world.consensus_evidence.view();
        let updated = view.get(&key).expect("evidence record");
        assert!(updated.penalty_cancelled);
        assert!(!updated.penalty_applied);
        assert_eq!(updated.penalty_cancelled_at_height, Some(2));
    }
}
