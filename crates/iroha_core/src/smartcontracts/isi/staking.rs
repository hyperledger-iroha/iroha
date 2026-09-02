//! Public lane staking instruction handlers (NX-9).
use super::prelude::*;
use crate::sumeragi::evidence::evidence_key;
use crate::{
    smartcontracts::isi::asset::isi::assert_numeric_spec_with,
    state::{
        ConsensusKeyGate, WorldReadOnly, peer_consensus_key_gate,
        public_lane_reward_record_matches_key, public_lane_stake_share_matches_key,
        public_lane_validator_record_matches_key,
    },
    sumeragi::status as sumeragi_status,
    telemetry::StateTelemetry,
};
use iroha_data_model::{
    asset::{Asset, AssetDefinitionId, AssetId},
    block::consensus::EvidencePenaltyStatus,
    consensus::ConsensusKeyRole,
    isi::{
        error::{InstructionExecutionError as Error, InvalidParameterError, MathError},
        staking::{
            BondPublicLaneStake, CancelConsensusEvidencePenalty, ClaimPublicLaneRewards,
            FinalizePublicLaneUnbond, RebindPublicLaneValidatorPeer, RecordPublicLaneRewards,
            RegisterPublicLaneValidator, SchedulePublicLaneUnbond, SlashPublicLaneValidator,
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
use iroha_primitives::numeric::{Numeric, Quantity, RoundingMode};
use std::{collections::BTreeMap, time::Duration};
/// Canonical storage key for one public-lane stake share.
pub(crate) type PublicLaneStakeShareKey = (LaneId, AccountId, AccountId);

struct IndexedValidatorStake {
    share_keys: Vec<PublicLaneStakeShareKey>,
    bonded: Quantity,
    self_bonded: Quantity,
    pending_unbonds: Quantity,
}

impl Default for IndexedValidatorStake {
    fn default() -> Self {
        Self {
            share_keys: Vec::new(),
            bonded: Quantity::zero(),
            self_bonded: Quantity::zero(),
            pending_unbonds: Quantity::zero(),
        }
    }
}

/// One-pass, bounded-work index over the complete public-lane stake-share table.
///
/// The index proves that every share belongs to exactly one canonical validator
/// and that the configured per-validator/per-share bounds hold. Consensus uses
/// the resulting key slices for point reads instead of rescanning the global
/// share table once per penalty.
pub(crate) struct PublicLaneStakeIndex {
    by_validator: BTreeMap<(LaneId, AccountId), IndexedValidatorStake>,
    #[cfg(test)]
    rows_scanned: usize,
}

impl PublicLaneStakeIndex {
    /// Build and validate an index from one exact world overlay.
    pub(crate) fn from_world(
        world: &impl WorldReadOnly,
        max_stake_shares_per_validator: u32,
        max_pending_unbonds_per_share: u32,
    ) -> Result<Self, Error> {
        let max_shares = usize::try_from(max_stake_shares_per_validator)
            .expect("u32 stake-share cap fits usize on supported targets");
        let max_pending = usize::try_from(max_pending_unbonds_per_share)
            .expect("u32 pending-unbond cap fits usize on supported targets");
        let mut by_validator = BTreeMap::<_, IndexedValidatorStake>::new();
        #[cfg(test)]
        let mut rows_scanned = 0_usize;

        for (key, share) in world.public_lane_stake_shares().iter() {
            #[cfg(test)]
            {
                rows_scanned = rows_scanned.saturating_add(1);
            }
            if !public_lane_stake_share_matches_key(key, share) {
                return Err(Error::InvariantViolation(
                    "public-lane stake share does not match its storage key".into(),
                ));
            }
            let validator_key = (key.0, key.1.clone());
            let record = world
                .public_lane_validators()
                .get(&validator_key)
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        "public-lane stake-share aggregate has no validator record".into(),
                    )
                })?;
            ensure_public_lane_validator_record_matches_key(&validator_key, record)?;
            if record.stake_account != record.validator {
                return Err(Error::InvariantViolation(
                    "public-lane validator stake account must match the validator account".into(),
                ));
            }
            if share.pending_unbonds.len() > max_pending {
                return Err(Error::InvariantViolation(
                    "public-lane stake share exceeds pending-unbond capacity".into(),
                ));
            }
            let indexed = by_validator.entry(validator_key).or_default();
            if indexed.share_keys.len() >= max_shares {
                return Err(Error::InvariantViolation(
                    "public-lane validator exceeds stake-share capacity".into(),
                ));
            }
            indexed.share_keys.push(key.clone());
            indexed.bonded = quantity_add(indexed.bonded.clone(), share.bonded.clone())?;
            if key.2 == record.stake_account {
                indexed.self_bonded =
                    quantity_add(indexed.self_bonded.clone(), share.bonded.clone())?;
            }
            for (request_id, pending) in &share.pending_unbonds {
                ensure_canonical_pending_unbond(request_id, pending)?;
                indexed.pending_unbonds =
                    quantity_add(indexed.pending_unbonds.clone(), pending.amount.clone())?;
            }
        }

        for (key, record) in world.public_lane_validators().iter() {
            ensure_public_lane_validator_record_matches_key(key, record)?;
            if record.stake_account != record.validator {
                return Err(Error::InvariantViolation(
                    "public-lane validator stake account must match the validator account".into(),
                ));
            }
            let indexed = by_validator.get(key);
            let bonded = indexed.map_or_else(Quantity::zero, |entry| entry.bonded.clone());
            let self_bonded =
                indexed.map_or_else(Quantity::zero, |entry| entry.self_bonded.clone());
            if bonded != record.total_stake || self_bonded != record.self_stake {
                return Err(Error::InvariantViolation(
                    "public-lane validator totals do not match canonical stake shares".into(),
                ));
            }
        }

        Ok(Self {
            by_validator,
            #[cfg(test)]
            rows_scanned,
        })
    }

    /// Return the complete canonical share-key slice for one validator.
    pub(crate) fn share_keys(
        &self,
        lane_id: LaneId,
        validator: &AccountId,
    ) -> &[PublicLaneStakeShareKey] {
        self.by_validator
            .get(&(lane_id, validator.clone()))
            .map_or(&[], |entry| entry.share_keys.as_slice())
    }

    /// Return total bonded and pending-unbond custody for one validator.
    pub(crate) fn total_exposure(
        &self,
        lane_id: LaneId,
        validator: &AccountId,
    ) -> Result<Quantity, Error> {
        let Some(indexed) = self.by_validator.get(&(lane_id, validator.clone())) else {
            return Ok(Quantity::zero());
        };
        quantity_add(indexed.bonded.clone(), indexed.pending_unbonds.clone())
    }

    #[cfg(test)]
    pub(crate) fn rows_scanned(&self) -> usize {
        self.rows_scanned
    }
}

/// One-shot retained-state proof for an exact public-lane staking slash.
pub(in crate::smartcontracts::isi) struct VerifiedStakingSlashDebit {
    lane_id: LaneId,
    validator: AccountId,
    slash_id: Hash,
    source_id: AssetId,
    destination_id: AssetId,
    amount: Quantity,
    slashable_exposure: Quantity,
}
impl VerifiedStakingSlashDebit {
    fn new(
        lane_id: LaneId,
        validator: AccountId,
        slash_id: Hash,
        source_id: AssetId,
        destination_id: AssetId,
        amount: Quantity,
        slashable_exposure: Quantity,
    ) -> Self {
        Self {
            lane_id,
            validator,
            slash_id,
            source_id,
            destination_id,
            amount,
            slashable_exposure,
        }
    }
    pub(in crate::smartcontracts::isi) fn into_parts(
        self,
    ) -> (
        LaneId,
        AccountId,
        Hash,
        AssetId,
        AssetId,
        Quantity,
        Quantity,
    ) {
        (
            self.lane_id,
            self.validator,
            self.slash_id,
            self.source_id,
            self.destination_id,
            self.amount,
            self.slashable_exposure,
        )
    }
}
fn current_epoch(block_height: u64, epoch_length_blocks: u64) -> Result<u64, Error> {
    if epoch_length_blocks == 0 {
        return Err(Error::InvariantViolation(
            "epoch_length_blocks must be greater than zero".into(),
        ));
    }
    Ok(block_height.saturating_sub(1) / epoch_length_blocks)
}

fn next_unfrozen_election_height(
    block_height: u64,
    epoch_length_blocks: u64,
) -> Result<u64, Error> {
    if epoch_length_blocks == 0 {
        return Err(Error::InvariantViolation(
            "epoch_length_blocks must be greater than zero".into(),
        ));
    }
    let epoch = block_height.saturating_sub(1) / epoch_length_blocks;
    let current_epoch_end = epoch
        .checked_add(1)
        .and_then(|value| value.checked_mul(epoch_length_blocks))
        .ok_or_else(|| Error::InvariantViolation("validator epoch boundary overflowed".into()))?;
    let next_epoch_start = current_epoch_end
        .checked_add(1)
        .ok_or_else(|| Error::InvariantViolation("validator epoch boundary overflowed".into()))?;
    if block_height < current_epoch_end {
        return Ok(next_epoch_start);
    }
    next_epoch_start
        .checked_add(epoch_length_blocks)
        .ok_or_else(|| Error::InvariantViolation("validator epoch boundary overflowed".into()))
}

fn scheduled_validator_eligibility_height(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<u64, Error> {
    let block_height = state_transaction.block_height();
    if state_transaction._curr_block.is_genesis() {
        return Ok(block_height);
    }
    let epoch_length = state_transaction
        .world
        .sumeragi_npos_parameters()
        .map_or(
            iroha_config::parameters::defaults::sumeragi::npos::EPOCH_LENGTH_BLOCKS,
            |params| params.epoch_length_blocks.get(),
        )
        .max(1);
    next_unfrozen_election_height(block_height, epoch_length)
}

fn scheduled_validator_deactivation_height(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<u64, Error> {
    let epoch_length = state_transaction
        .world
        .sumeragi_npos_parameters()
        .map_or(
            iroha_config::parameters::defaults::sumeragi::npos::EPOCH_LENGTH_BLOCKS,
            |params| params.epoch_length_blocks.get(),
        )
        .max(1);
    next_unfrozen_election_height(state_transaction.block_height(), epoch_length)
}

fn schedule_validator_deactivation(
    record: &mut PublicLaneValidatorRecord,
    boundary: u64,
) -> Result<(), Error> {
    let activation = record.activation_height;
    if let Some(existing) = record.deactivation_height {
        if existing < activation {
            return Err(Error::InvariantViolation(
                "validator deactivation boundary precedes activation".into(),
            ));
        }
        // The first terminal transition fixes the historical half-open tenure.
        // Later lifecycle labels must never rewrite that consensus fact.
        return Ok(());
    }
    let boundary = if matches!(
        record.status,
        PublicLaneValidatorStatus::PendingActivation(_)
    ) {
        // Cancelling a future activation creates an empty retained tenure;
        // its exclusive end cannot precede its scheduled inclusive start.
        boundary.max(activation)
    } else {
        boundary
    };
    if boundary < activation {
        return Err(Error::InvariantViolation(
            "validator deactivation boundary precedes activation".into(),
        ));
    }
    record.deactivation_height = Some(boundary);
    Ok(())
}

/// Check whether an offence height falls inside one exact retained tenure.
pub(crate) fn validator_tenure_contains_height(
    record: &PublicLaneValidatorRecord,
    height: u64,
) -> Result<bool, Error> {
    let activation = record.activation_height;
    if record
        .deactivation_height
        .is_some_and(|deactivation| deactivation < activation)
    {
        return Err(Error::InvariantViolation(
            "validator deactivation boundary precedes activation".into(),
        ));
    }
    if let PublicLaneValidatorStatus::PendingActivation(pending_height) = record.status
        && pending_height != activation
    {
        return Err(Error::InvariantViolation(
            "pending validator status disagrees with activation boundary".into(),
        ));
    }
    Ok(activation <= height
        && record
            .deactivation_height
            .is_none_or(|deactivation| height < deactivation))
}

/// Check whether a record may participate in an election for an exact height.
pub(crate) fn validator_election_eligible_at_height(
    record: &PublicLaneValidatorRecord,
    height: u64,
) -> bool {
    // Lifecycle labels describe requested/observed state, but cannot rewrite a
    // committee that was already frozen. The retained half-open tenure is the
    // sole authority for exact-height election and validation.
    validator_tenure_contains_height(record, height).unwrap_or(false)
}
fn ensure_validator_deactivation_reached(
    record: &PublicLaneValidatorRecord,
    block_height: u64,
    operation: &str,
) -> Result<(), Error> {
    let deactivation = record.deactivation_height.ok_or_else(|| {
        Error::InvariantViolation(
            format!("{operation} requires a canonical validator deactivation height").into(),
        )
    })?;
    if deactivation < record.activation_height {
        return Err(Error::InvariantViolation(
            "validator deactivation boundary precedes activation".into(),
        ));
    }
    if block_height < deactivation {
        return Err(Error::InvariantViolation(
            format!(
                "{operation} cannot replace or prune a validator before deactivation height {deactivation}"
            )
            .into(),
        ));
    }
    Ok(())
}
fn ensure_lane_allows_staking(
    state_transaction: &StateTransaction<'_, '_>,
    lane_id: LaneId,
    context: &str,
) -> Result<(), Error> {
    if !state_transaction.is_lane_active_for_authority(lane_id) {
        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_public_lane_validator_reject("lane_inactive");
        return Err(Error::InvalidParameter(
            InvalidParameterError::SmartContract(format!(
                "{context} rejected: lane {lane_id} is not active at block height {}",
                state_transaction.block_height()
            )),
        ));
    }
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
fn ensure_canonical_staking_owner(
    state_transaction: &StateTransaction<'_, '_>,
    lane_id: LaneId,
    context: &str,
) -> Result<(), Error> {
    let Some(owner_lane) = state_transaction.staking_authority_lane(lane_id) else {
        return Err(Error::InvalidParameter(
            InvalidParameterError::SmartContract(format!(
                "{context} rejected: lane {lane_id} has no active staking owner"
            )),
        ));
    };
    if owner_lane != lane_id {
        return Err(Error::InvalidParameter(
            InvalidParameterError::SmartContract(format!(
                "{context} rejected: lane {lane_id} shares a physical dataspace whose staking owner is lane {owner_lane}"
            )),
        ));
    }
    Ok(())
}
fn ensure_validator_authority(
    authority: &AccountId,
    validator: &AccountId,
    context: &str,
) -> Result<(), Error> {
    if authority != validator {
        return Err(Error::InvariantViolation(
            format!("{context} rejected: authority must match validator account").into(),
        ));
    }
    Ok(())
}
fn ensure_staker_authority(
    authority: &AccountId,
    staker: &AccountId,
    context: &str,
) -> Result<(), Error> {
    if authority != staker {
        return Err(Error::InvariantViolation(
            format!("{context} rejected: authority must match staker account").into(),
        ));
    }
    Ok(())
}
fn ensure_registration_uses_validator_stake(
    validator: &AccountId,
    stake_account: &AccountId,
    context: &str,
) -> Result<(), Error> {
    if validator != stake_account {
        return Err(Error::InvariantViolation(
            format!("{context} rejected: stake account must match validator account").into(),
        ));
    }
    Ok(())
}
fn ensure_public_lane_validator_record_matches_key(
    key: &(LaneId, AccountId),
    record: &PublicLaneValidatorRecord,
) -> Result<(), Error> {
    if !public_lane_validator_record_matches_key(key, record) {
        return Err(Error::InvariantViolation(
            "public-lane validator record does not match its storage key".into(),
        ));
    }
    if record.stake_account != record.validator {
        return Err(Error::InvariantViolation(
            "public-lane validator stake account must match the validator account".into(),
        ));
    }
    Ok(())
}
fn ensure_public_lane_stake_share_matches_key(
    key: &(LaneId, AccountId, AccountId),
    share: &PublicLaneStakeShare,
) -> Result<(), Error> {
    if public_lane_stake_share_matches_key(key, share) {
        Ok(())
    } else {
        Err(Error::InvariantViolation(
            "public-lane stake share does not match its storage key".into(),
        ))
    }
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
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "register_public_lane_validator",
        )?;
        ensure_canonical_staking_owner(
            state_transaction,
            self.lane_id,
            "register_public_lane_validator",
        )?;
        let is_genesis_bootstrap =
            state_transaction._curr_block.is_genesis() && state_transaction.block_hashes.is_empty();
        if !is_genesis_bootstrap {
            ensure_validator_authority(
                authority,
                &self.validator,
                "register_public_lane_validator",
            )?;
        }
        ensure_registration_uses_validator_stake(
            &self.validator,
            &self.stake_account,
            "register_public_lane_validator",
        )?;
        finalize_validator_lifecycle(state_transaction)?;
        // Resolve the exact election boundary before validating the peer or
        // moving funds. An open-ended validator tenure requires a validator
        // key that remains live from this boundary onward.
        let activation_height = scheduled_validator_eligibility_height(state_transaction)?;
        ensure_validator_peer_registered(
            state_transaction,
            self.lane_id,
            &self.validator,
            &self.peer_id,
            activation_height,
        )?;
        ensure_positive_amount(&self.initial_stake, "initial stake")?;
        let meets_min = meets_min_stake(
            &self.initial_stake,
            &state_transaction.nexus.staking.min_validator_stake,
        )?;
        if !meets_min {
            return Err(Error::InvariantViolation(
                "initial stake below minimum configured amount".into(),
            ));
        }
        let stake_ctx = stake_context(
            &state_transaction.world,
            &state_transaction.nexus.dataspace_catalog,
            &state_transaction.nexus.staking,
            &self.stake_account,
            None,
            state_transaction.block_unix_timestamp_ms(),
        )?;
        assert_stake_amount_matches_spec(
            state_transaction,
            &stake_ctx.asset_definition,
            &self.initial_stake,
        )?;
        let validator_key = validator_storage_key(self.lane_id, &self.validator);
        let replacing_exited = if let Some(existing) = state_transaction
            .world
            .public_lane_validators
            .get(&validator_key)
        {
            ensure_public_lane_validator_record_matches_key(&validator_key, existing)?;
            if !matches!(existing.status, PublicLaneValidatorStatus::Exited) {
                return Err(Error::InvariantViolation(
                    "validator already registered for lane".into(),
                ));
            }
            ensure_validator_deactivation_reached(
                existing,
                state_transaction.block_height(),
                "register_public_lane_validator",
            )?;
            ensure_no_pending_evidence_for_validator(
                state_transaction,
                existing,
                "register_public_lane_validator",
            )?;
            if !existing.total_stake.is_zero()
                || !existing.self_stake.is_zero()
                || state_transaction.world.public_lane_stake_shares.iter().any(
                    |((lane, validator, _), _)| {
                        *lane == self.lane_id && validator == &self.validator
                    },
                )
            {
                return Err(Error::InvariantViolation(
                    "exited validator retains slashable stake custody; finalize every unbond before re-registration"
                        .into(),
                ));
            }
            true
        } else {
            if state_transaction.world.public_lane_stake_shares.iter().any(
                |((lane, validator, _), _)| *lane == self.lane_id && validator == &self.validator,
            ) {
                return Err(Error::InvariantViolation(
                    "validator has orphaned public-lane stake shares".into(),
                ));
            }
            false
        };
        let existing = state_transaction
            .world
            .public_lane_validators
            .iter()
            .filter(|(key, record)| {
                public_lane_validator_record_matches_key(key, record)
                    && key.0 == self.lane_id
                    && (!replacing_exited || key.1 != self.validator)
            })
            .count();
        let max_validators = usize::try_from(state_transaction.nexus.staking.max_validators.get())
            .unwrap_or(usize::MAX);
        if existing >= max_validators {
            return Err(Error::InvariantViolation(
                "lane reached maximum validator capacity".into(),
            ));
        }
        if state_transaction
            .world
            .public_lane_validators
            .iter()
            .any(|(key, record)| {
                public_lane_validator_record_matches_key(key, record)
                    && key.0 == self.lane_id
                    && key.1 != self.validator
                    && record.peer_id == self.peer_id
            })
        {
            return Err(Error::InvariantViolation(
                "validator peer is already retained for lane".into(),
            ));
        }
        let initial_stake = self.initial_stake.clone();
        crate::smartcontracts::isi::asset::isi::execute_staking_bond_transfer(
            state_transaction,
            authority,
            self.lane_id,
            &self.validator,
            &self.stake_account,
            is_genesis_bootstrap,
            stake_ctx.staker_asset.clone(),
            stake_ctx.escrow_asset.clone(),
            initial_stake.clone(),
        )?;
        if replacing_exited {
            let removal_key = validator_key.clone();
            state_transaction
                .world
                .public_lane_validators
                .remove(removal_key);
        }
        // The exact height is assigned while scheduling, not when promotion
        // happens. Boundary-block transactions cannot alter the already-frozen
        // successor roster, so they target the following election instead.
        let pending_status = PublicLaneValidatorStatus::PendingActivation(activation_height);
        let record = PublicLaneValidatorRecord {
            lane_id: self.lane_id,
            validator: self.validator.clone(),
            peer_id: self.peer_id.clone(),
            stake_account: self.stake_account.clone(),
            total_stake: initial_stake.clone(),
            self_stake: initial_stake.clone(),
            metadata: self.metadata.clone(),
            status: pending_status.clone(),
            activation_height,
            deactivation_height: None,
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
        ensure_canonical_staking_owner(
            state_transaction,
            self.lane_id,
            "activate_public_lane_validator",
        )?;
        finalize_validator_lifecycle(state_transaction)?;
        let validator_key = validator_storage_key(self.lane_id, &self.validator);
        let validator_record = state_transaction
            .world
            .public_lane_validators
            .get(&validator_key)
            .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;
        ensure_public_lane_validator_record_matches_key(&validator_key, validator_record)?;
        let validator_peer = validator_record.peer_id.clone();
        let activation_height = validator_record.activation_height;
        ensure_validator_peer_registered(
            state_transaction,
            self.lane_id,
            &self.validator,
            &validator_peer,
            activation_height,
        )?;
        let block_height = state_transaction.block_height();
        let epoch_length = state_transaction
            .world
            .sumeragi_npos_parameters()
            .map_or(
                iroha_config::parameters::defaults::sumeragi::npos::EPOCH_LENGTH_BLOCKS,
                |params| params.epoch_length_blocks.get(),
            )
            .max(1);
        let current_epoch = current_epoch(block_height, epoch_length)?;
        let validator = state_transaction
            .world
            .public_lane_validators
            .get_mut(&validator_key)
            .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;
        let previous_status = validator.status.clone();
        match previous_status {
            PublicLaneValidatorStatus::PendingActivation(pending_height) => {
                if block_height < pending_height {
                    return Err(Error::InvariantViolation(
                        "validator activation height not reached".into(),
                    ));
                }
                if validator.activation_height != pending_height {
                    return Err(Error::InvariantViolation(
                        "pending validator activation height is not canonical".into(),
                    ));
                }
                if validator.deactivation_height.is_some() {
                    return Err(Error::InvariantViolation(
                        "pending validator already has a deactivation boundary".into(),
                    ));
                }
                validator.status = PublicLaneValidatorStatus::Active;
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
impl Execute for RebindPublicLaneValidatorPeer {
    #[iroha_logger::log(
        name = "rebind_public_lane_validator_peer",
        skip_all,
        fields(lane_id = %self.lane_id, validator = %self.validator, peer_id = %self.peer_id)
    )]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "rebind_public_lane_validator_peer",
        )?;
        ensure_canonical_staking_owner(
            state_transaction,
            self.lane_id,
            "rebind_public_lane_validator_peer",
        )?;
        ensure_validator_authority(
            authority,
            &self.validator,
            "rebind_public_lane_validator_peer",
        )?;
        finalize_validator_lifecycle(state_transaction)?;
        let validator_key = validator_storage_key(self.lane_id, &self.validator);
        let record = state_transaction
            .world
            .public_lane_validators
            .get(&validator_key)
            .cloned()
            .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;
        ensure_public_lane_validator_record_matches_key(&validator_key, &record)?;
        if record.peer_id == self.peer_id {
            return Ok(());
        }
        if !state_transaction._curr_block.is_genesis()
            && state_transaction.block_height().saturating_add(1) >= record.activation_height
        {
            return Err(Error::InvariantViolation(
                "validator peer binding is already frozen for its activation height".into(),
            ));
        }
        ensure_no_pending_evidence_for_validator(
            state_transaction,
            &record,
            "rebind_public_lane_validator_peer",
        )?;
        match record.status {
            PublicLaneValidatorStatus::PendingActivation(_) => {}
            PublicLaneValidatorStatus::Active
            | PublicLaneValidatorStatus::Exiting(_)
            | PublicLaneValidatorStatus::Exited
            | PublicLaneValidatorStatus::Slashed(_) => {
                return Err(Error::InvariantViolation(
                    "peer rebinding is allowed only before validator activation".into(),
                ));
            }
        }
        ensure_validator_peer_registered(
            state_transaction,
            self.lane_id,
            &self.validator,
            &self.peer_id,
            record.activation_height,
        )?;
        if state_transaction
            .world
            .public_lane_validators
            .iter()
            .any(|(key, record)| {
                public_lane_validator_record_matches_key(key, record)
                    && key.0 == self.lane_id
                    && key.1 != self.validator
                    && record.peer_id == self.peer_id
            })
        {
            return Err(Error::InvariantViolation(
                "validator peer is already registered for lane".into(),
            ));
        }
        let record = state_transaction
            .world
            .public_lane_validators
            .get_mut(&validator_key)
            .expect("validated above");
        record.peer_id = self.peer_id;
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
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "exit_public_lane_validator",
        )?;
        ensure_canonical_staking_owner(
            state_transaction,
            self.lane_id,
            "exit_public_lane_validator",
        )?;
        ensure_validator_authority(authority, &self.validator, "exit_public_lane_validator")?;
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
        ensure_public_lane_validator_record_matches_key(&validator_key, &record)?;
        ensure_no_pending_evidence_for_validator(
            state_transaction,
            &record,
            "exit_public_lane_validator",
        )?;
        let deactivation_height = scheduled_validator_deactivation_height(state_transaction)?;
        #[cfg(feature = "telemetry")]
        let previous_status = record.status.clone();
        match record.status {
            PublicLaneValidatorStatus::PendingActivation(_)
            | PublicLaneValidatorStatus::Active
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
        schedule_validator_deactivation(&mut record, deactivation_height)?;
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
        prune_zero_custody_exited_validators(state_transaction);
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
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(state_transaction, self.lane_id, "bond_public_lane_stake")?;
        ensure_canonical_staking_owner(state_transaction, self.lane_id, "bond_public_lane_stake")?;
        ensure_staker_authority(authority, &self.staker, "bond_public_lane_stake")?;
        finalize_validator_lifecycle(state_transaction)?;
        ensure_positive_amount(&self.amount, "stake amount")?;
        let stake_ctx = stake_context(
            &state_transaction.world,
            &state_transaction.nexus.dataspace_catalog,
            &state_transaction.nexus.staking,
            &self.staker,
            None,
            state_transaction.block_unix_timestamp_ms(),
        )?;
        assert_stake_amount_matches_spec(
            state_transaction,
            &stake_ctx.asset_definition,
            &self.amount,
        )?;
        let validator_key = validator_storage_key(self.lane_id, &self.validator);
        let mut validator_record = state_transaction
            .world
            .public_lane_validators
            .get(&validator_key)
            .cloned()
            .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;
        ensure_public_lane_validator_record_matches_key(&validator_key, &validator_record)?;
        if !matches!(
            validator_record.status,
            PublicLaneValidatorStatus::PendingActivation(_) | PublicLaneValidatorStatus::Active
        ) {
            return Err(Error::InvariantViolation(
                "validator status does not accept new stake".into(),
            ));
        }
        ensure_no_pending_evidence_for_validator(
            state_transaction,
            &validator_record,
            "bond_public_lane_stake",
        )?;
        let amount = self.amount.clone();
        let available = state_transaction
            .world
            .assets
            .get(&stake_ctx.staker_asset)
            .map_or_else(Quantity::zero, |balance| balance.as_ref().clone());
        if available < amount {
            return Err(Error::Math(MathError::NotEnoughQuantity));
        }
        validator_record.total_stake =
            quantity_add(validator_record.total_stake.clone(), amount.clone())?;
        if is_self_stake_share_staker(
            &self.staker,
            &self.validator,
            &validator_record.stake_account,
        ) {
            validator_record.self_stake =
                quantity_add(validator_record.self_stake.clone(), amount.clone())?;
        }
        let share_key = stake_key(self.lane_id, &self.validator, &self.staker);
        let mut share = if let Some(share) = state_transaction
            .world
            .public_lane_stake_shares
            .get(&share_key)
            .cloned()
        {
            ensure_public_lane_stake_share_matches_key(&share_key, &share)?;
            share
        } else {
            let share_count = state_transaction
                .world
                .public_lane_stake_shares
                .iter()
                .filter(|(key, share)| {
                    (key.0 == self.lane_id && key.1 == self.validator)
                        || (share.lane_id == self.lane_id && share.validator == self.validator)
                })
                .count();
            let max_shares = usize::try_from(
                state_transaction
                    .nexus
                    .staking
                    .max_stake_shares_per_validator
                    .get(),
            )
            .expect("u32 stake-share cap fits usize on supported targets");
            if share_count >= max_shares {
                return Err(Error::InvariantViolation(
                    "validator reached maximum stake-share capacity".into(),
                ));
            }
            PublicLaneStakeShare {
                lane_id: self.lane_id,
                validator: self.validator.clone(),
                staker: self.staker.clone(),
                bonded: Quantity::zero(),
                pending_unbonds: BTreeMap::new(),
                metadata: Metadata::default(),
            }
        };
        share.metadata = self.metadata.clone();
        share.bonded = quantity_add(share.bonded.clone(), amount.clone())?;
        crate::smartcontracts::isi::asset::isi::execute_staking_bond_transfer(
            state_transaction,
            authority,
            self.lane_id,
            &self.validator,
            &self.staker,
            false,
            stake_ctx.staker_asset.clone(),
            stake_ctx.escrow_asset.clone(),
            amount.clone(),
        )?;
        state_transaction
            .world
            .public_lane_validators
            .insert(validator_key, validator_record);
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
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "schedule_public_lane_unbond",
        )?;
        ensure_canonical_staking_owner(
            state_transaction,
            self.lane_id,
            "schedule_public_lane_unbond",
        )?;
        ensure_staker_authority(authority, &self.staker, "schedule_public_lane_unbond")?;
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
            &state_transaction.nexus.dataspace_catalog,
            &state_transaction.nexus.staking,
            &self.staker,
            None,
            block_timestamp_ms,
        )?;
        assert_stake_amount_matches_spec(
            state_transaction,
            &stake_ctx.asset_definition,
            &self.amount,
        )?;
        let validator_key = validator_storage_key(self.lane_id, &self.validator);
        let mut validator_snapshot = state_transaction
            .world
            .public_lane_validators
            .get(&validator_key)
            .cloned()
            .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;
        ensure_public_lane_validator_record_matches_key(&validator_key, &validator_snapshot)?;
        ensure_no_pending_evidence_for_validator(
            state_transaction,
            &validator_snapshot,
            "schedule_public_lane_unbond",
        )?;
        let slashable_through_height = scheduled_validator_deactivation_height(state_transaction)?
            .checked_sub(1)
            .ok_or_else(|| {
                Error::InvariantViolation("public-lane unbond slashable height underflowed".into())
            })?;
        let liability_release_height =
            unbond_liability_release_height(state_transaction, slashable_through_height)?;
        let amount = self.amount.clone();
        let share_key = stake_key(self.lane_id, &self.validator, &self.staker);
        let mut share = state_transaction
            .world
            .public_lane_stake_shares
            .get(&share_key)
            .cloned()
            .ok_or_else(|| Error::InvariantViolation("stake position not found".into()))?;
        ensure_public_lane_stake_share_matches_key(&share_key, &share)?;
        if share.pending_unbonds.contains_key(&self.request_id) {
            return Err(Error::InvariantViolation(
                "unbond request already scheduled".into(),
            ));
        }
        let max_pending = usize::try_from(
            state_transaction
                .nexus
                .staking
                .max_pending_unbonds_per_share
                .get(),
        )
        .expect("u32 pending-unbond cap fits usize on supported targets");
        if share.pending_unbonds.len() >= max_pending {
            return Err(Error::InvariantViolation(
                "stake share reached maximum pending-unbond capacity".into(),
            ));
        }
        if share.bonded < amount {
            return Err(Error::InvariantViolation(
                "unbond exceeds bonded amount".into(),
            ));
        }
        if validator_snapshot.total_stake < amount {
            return Err(Error::InvariantViolation(
                "unbond exceeds validator total stake".into(),
            ));
        }
        validator_snapshot.total_stake =
            quantity_sub(validator_snapshot.total_stake.clone(), amount.clone())?;
        if is_self_stake_share_staker(
            &self.staker,
            &self.validator,
            &validator_snapshot.stake_account,
        ) {
            validator_snapshot.self_stake =
                quantity_sub(validator_snapshot.self_stake.clone(), amount.clone())?;
        }
        share.bonded = quantity_sub(share.bonded.clone(), amount.clone())?;
        share.pending_unbonds.insert(
            self.request_id,
            PublicLaneUnbonding {
                request_id: self.request_id,
                amount: amount.clone(),
                release_at_ms: self.release_at_ms,
                slashable_through_height,
                liability_release_height,
            },
        );
        state_transaction
            .world
            .public_lane_validators
            .insert(validator_key, validator_snapshot);
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
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "finalize_public_lane_unbond",
        )?;
        ensure_canonical_staking_owner(
            state_transaction,
            self.lane_id,
            "finalize_public_lane_unbond",
        )?;
        ensure_staker_authority(authority, &self.staker, "finalize_public_lane_unbond")?;
        finalize_validator_lifecycle(state_transaction)?;
        let block_timestamp_ms = state_transaction.block_unix_timestamp_ms();
        let stake_ctx = stake_context(
            &state_transaction.world,
            &state_transaction.nexus.dataspace_catalog,
            &state_transaction.nexus.staking,
            &self.staker,
            None,
            block_timestamp_ms,
        )?;
        let validator_key = validator_storage_key(self.lane_id, &self.validator);
        let validator_record = state_transaction
            .world
            .public_lane_validators
            .get(&validator_key)
            .cloned()
            .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;
        ensure_public_lane_validator_record_matches_key(&validator_key, &validator_record)?;
        ensure_no_pending_evidence_for_validator(
            state_transaction,
            &validator_record,
            "finalize_public_lane_unbond",
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
        ensure_canonical_pending_unbond(&self.request_id, &pending)?;
        let current_height = state_transaction.block_height();
        let current_policy_release_height =
            unbond_liability_release_height(state_transaction, pending.slashable_through_height)?;
        let liability_release_height = pending
            .liability_release_height
            .max(current_policy_release_height);
        if current_height < liability_release_height {
            return Err(Error::InvariantViolation(
                format!(
                    "unbond request remains slashable through the pre-transaction effects at block height {liability_release_height}"
                )
                .into(),
            ));
        }
        if pending.release_at_ms > block_timestamp_ms {
            return Err(Error::InvariantViolation(
                "unbond request not yet releasable".into(),
            ));
        }
        assert_stake_amount_matches_spec(
            state_transaction,
            &stake_ctx.asset_definition,
            &pending.amount,
        )?;
        crate::smartcontracts::isi::asset::isi::execute_staking_unbond_transfer(
            state_transaction,
            authority,
            self.lane_id,
            &self.validator,
            &self.staker,
            self.request_id,
            stake_ctx.escrow_asset,
            stake_ctx.staker_asset,
            pending.amount.clone(),
        )?;
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
        prune_zero_custody_exited_validators(state_transaction);
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
        ensure_canonical_staking_owner(
            state_transaction,
            self.lane_id,
            "slash_public_lane_validator",
        )?;
        finalize_validator_lifecycle(state_transaction)?;
        ensure_positive_amount(&self.amount, "slash amount")?;
        let current_height = state_transaction.block_height();
        if self.offence_height == 0 || self.offence_height > current_height {
            return Err(Error::InvariantViolation(
                "slash offence height must identify a non-zero height no later than the current block"
                    .into(),
            ));
        }
        let recorded_at_ms = state_transaction.block_unix_timestamp_ms();
        apply_slash_to_validator(
            state_transaction,
            self.lane_id,
            &self.validator,
            self.offence_height,
            self.slash_id,
            &self.amount,
            recorded_at_ms,
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
        let current_height = state_transaction.block_height();
        if record.recorded_at_height >= current_height {
            return Err(Error::InvariantViolation(
                "consensus evidence must be admitted by a prior committed block before cancellation"
                    .into(),
            ));
        }
        match record.penalty_status {
            EvidencePenaltyStatus::Pending => {}
            EvidencePenaltyStatus::Applied { .. } => {
                return Err(Error::InvariantViolation(
                    "consensus evidence penalty already applied".into(),
                ));
            }
            EvidencePenaltyStatus::Cancelled { .. } => return Ok(()),
        }
        record.penalty_status = EvidencePenaltyStatus::Cancelled {
            height: current_height,
        };
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
        ensure_lane_allows_staking(
            state_transaction,
            self.lane_id,
            "record_public_lane_rewards",
        )?;
        let validator_lane_id = state_transaction
            .staking_authority_lane(self.lane_id)
            .ok_or_else(|| {
                Error::InvariantViolation(
                    format!(
                        "record_public_lane_rewards rejected: lane {} has no active staking owner",
                        self.lane_id
                    )
                    .into(),
                )
            })?;
        ensure_positive_amount(&self.total_reward, "total_reward")?;
        finalize_validator_lifecycle(state_transaction)?;
        ensure_reward_targets_active(state_transaction, validator_lane_id, &self.shares)?;
        let record_key = (self.lane_id, self.epoch);
        ensure_reward_epoch_fresh(state_transaction, self.lane_id, self.epoch, record_key)?;
        validate_reward_amounts(
            &self.total_reward,
            &self.shares,
            self.reward_asset.definition(),
            state_transaction,
        )?;
        validate_reward_sink(&self.reward_asset, &self.total_reward, state_transaction)?;
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
        update_validator_rewards(
            state_transaction,
            validator_lane_id,
            self.epoch,
            &self.shares,
        );
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
        ensure_lane_allows_staking(state_transaction, self.lane_id, "claim_public_lane_rewards")?;
        finalize_validator_lifecycle(state_transaction)?;
        if &self.account != authority {
            return Err(Error::InvariantViolation(
                "reward claims must be submitted by the recipient account".into(),
            ));
        }
        let upto_epoch = self.upto_epoch.unwrap_or(u64::MAX);
        let mut claim_totals: BTreeMap<AssetId, (Quantity, u64)> = BTreeMap::new();
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
        for (key, record) in state_transaction.world.public_lane_rewards.iter() {
            let (lane, epoch) = key;
            if *lane != self.lane_id {
                continue;
            }
            if *epoch > upto_epoch {
                break;
            }
            if !public_lane_reward_record_matches_key(key, record) {
                continue;
            }
            let last_seen = *last_claimed.get(&record.asset).unwrap_or(&0);
            if *epoch <= last_seen {
                continue;
            }
            for share in record.shares.iter().filter(|s| s.account == self.account) {
                let entry = claim_totals
                    .entry(record.asset.clone())
                    .or_insert_with(|| (Quantity::zero(), last_seen));
                entry.0 = quantity_add(entry.0.clone(), share.amount.clone())?;
                entry.1 = entry.1.max(*epoch);
            }
        }
        if claim_totals.is_empty() {
            return Ok(());
        }
        let sink_account = crate::block::parse_account_literal_with_world(
            &state_transaction.world,
            &state_transaction.nexus.dataspace_catalog,
            &state_transaction.nexus.fees.fee_sink_account_id,
            state_transaction.block_unix_timestamp_ms(),
        )
        .map_err(|error| Error::InvariantViolation(error.to_string().into()))?
        .ok_or_else(|| {
            Error::InvariantViolation(
                "invalid nexus.fees.fee_sink_account_id; expected canonical I105 account id or on-chain alias"
                    .into(),
            )
        })?;
        let fee_asset = resolve_nexus_fee_asset_definition(state_transaction)?;
        let dust_threshold = state_transaction
            .nexus
            .staking
            .reward_dust_threshold
            .clone();
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
            if !dust_threshold.is_zero() && amount < dust_threshold {
                state_transaction
                    .world
                    .public_lane_reward_claims
                    .insert((self.lane_id, self.account.clone(), asset_id), max_epoch);
                continue;
            }
            let transfer =
                iroha_data_model::isi::Transfer::<
                    Asset,
                    Quantity,
                    iroha_data_model::account::Account,
                >::asset_quantity(asset_id.clone(), amount, self.account.clone());
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
    prune_zero_custody_exited_validators(state_transaction);
    Ok(())
}

fn prune_zero_custody_exited_validators(state_transaction: &mut StateTransaction<'_, '_>) {
    let removable =
        state_transaction
            .world
            .public_lane_validators
            .iter()
            .filter(|(key, record)| {
                public_lane_validator_record_matches_key(key, record)
                    && matches!(record.status, PublicLaneValidatorStatus::Exited)
                    && ensure_validator_deactivation_reached(
                        record,
                        state_transaction.block_height(),
                        "validator pruning",
                    )
                    .is_ok()
                    && record.total_stake.is_zero()
                    && record.self_stake.is_zero()
                    && !state_transaction.world.public_lane_stake_shares.iter().any(
                        |((lane_id, validator, _), _)| *lane_id == key.0 && validator == &key.1,
                    )
                    && !crate::sumeragi::evidence::has_pending_v2_evidence_for_validator_tenure(
                        &state_transaction.world,
                        record,
                    )
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
    for key in removable {
        state_transaction.world.public_lane_validators.remove(key);
    }
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
    let pending_height = match record.status {
        PublicLaneValidatorStatus::PendingActivation(height) => height,
        _ => {
            return Err(Error::InvariantViolation(
                "only a pending validator can be promoted".into(),
            ));
        }
    };
    if block_height < pending_height {
        return Err(Error::InvariantViolation(
            "validator activation height has not been reached".into(),
        ));
    }
    if record.activation_height != pending_height {
        return Err(Error::InvariantViolation(
            "pending validator activation height is not canonical".into(),
        ));
    }
    if record.deactivation_height.is_some() {
        return Err(Error::InvariantViolation(
            "pending validator already has a deactivation boundary".into(),
        ));
    }
    record.status = PublicLaneValidatorStatus::Active;
    #[cfg(not(feature = "telemetry"))]
    let _ = telemetry;
    #[cfg(feature = "telemetry")]
    if let Some(tel) = telemetry {
        tel.record_public_lane_validator_status(lane_id, Some(previous_status), &record.status);
        tel.record_public_lane_validator_activation(lane_id, current_epoch);
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
            iroha_config::parameters::defaults::sumeragi::npos::EPOCH_LENGTH_BLOCKS,
            |params| params.epoch_length_blocks.get(),
        )
        .max(1);
    let current_epoch = current_epoch(block_height, epoch_length)?;
    let to_activate: Vec<_> = state_transaction
        .world
        .public_lane_validators
        .iter()
        .filter(|(key, record)| {
            public_lane_validator_record_matches_key(key, record)
                && state_transaction.staking_authority_lane(key.0) == Some(key.0)
                && matches!(
                    record.status,
                    PublicLaneValidatorStatus::PendingActivation(pending) if pending <= block_height
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
        .filter(|(key, record)| {
            public_lane_validator_record_matches_key(key, record)
                && matches!(
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
        let key = validator_storage_key(lane_id, &share.account);
        let Some(record) = state_transaction.world.public_lane_validators.get(&key) else {
            return Err(Error::InvariantViolation(
                "reward share references unknown validator".into(),
            ));
        };
        if !public_lane_validator_record_matches_key(&key, record)
            || !validator_election_eligible_at_height(record, state_transaction.block_height())
        {
            return Err(Error::InvariantViolation(
                "reward share validator is outside its active tenure at this block height".into(),
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
fn ensure_positive_amount(amount: &Quantity, label: &str) -> Result<(), Error> {
    if amount.is_zero() {
        return Err(Error::InvariantViolation(
            format!("{label} must be greater than zero").into(),
        ));
    }
    Ok(())
}
fn quantity_add(lhs: Quantity, rhs: Quantity) -> Result<Quantity, Error> {
    lhs.checked_add(&rhs)
        .map_err(|_| Error::Math(MathError::Overflow))
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
    total_reward: &Quantity,
    shares: &[PublicLaneRewardShare],
    reward_definition: &AssetDefinitionId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let computed_total = shares.iter().try_fold(Quantity::zero(), |acc, share| {
        quantity_add(acc, share.amount.clone())
    })?;
    if computed_total != *total_reward {
        return Err(Error::InvariantViolation(
            "reward shares must sum to total_reward".into(),
        ));
    }
    let reward_spec = state_transaction
        .numeric_spec_for(reward_definition)
        .map_err(Error::from)?;
    assert_numeric_spec_with(total_reward.as_numeric(), reward_spec)?;
    for share in shares {
        ensure_positive_amount(&share.amount, "reward share amount")?;
        assert_numeric_spec_with(share.amount.as_numeric(), reward_spec)?;
    }
    Ok(())
}
fn validate_reward_sink(
    reward_asset: &AssetId,
    total_reward: &Quantity,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let sink_account = crate::block::parse_account_literal_with_world(
        &state_transaction.world,
        &state_transaction.nexus.dataspace_catalog,
        &state_transaction.nexus.fees.fee_sink_account_id,
        state_transaction.block_unix_timestamp_ms(),
    )
    .map_err(|error| Error::InvariantViolation(error.to_string().into()))?
    .ok_or_else(|| {
        Error::InvariantViolation(
            "invalid nexus.fees.fee_sink_account_id; expected canonical I105 account id or on-chain alias"
                .into(),
        )
    })?;
    let fee_asset = resolve_nexus_fee_asset_definition(state_transaction)?;
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
        let key = validator_storage_key(lane_id, &share.account);
        if let Some(validator) = state_transaction.world.public_lane_validators.get_mut(&key)
            && public_lane_validator_record_matches_key(&key, validator)
        {
            validator.last_reward_epoch = Some(
                validator
                    .last_reward_epoch
                    .map_or(epoch, |last_epoch| last_epoch.max(epoch)),
            );
        }
    }
}
fn quantity_sub(lhs: Quantity, rhs: Quantity) -> Result<Quantity, Error> {
    lhs.checked_sub(&rhs)
        .map_err(|_| Error::Math(MathError::Overflow))
}
/// Return every slashable unit still held by staking custody for one validator.
pub(crate) fn slashable_validator_exposure(
    world: &impl WorldReadOnly,
    lane_id: LaneId,
    validator: &AccountId,
    record: &PublicLaneValidatorRecord,
) -> Result<Quantity, Error> {
    let shares = validator_share_updates(world, lane_id, validator, None)?;
    slashable_exposure_from_shares(record, &shares, None)
}

/// Return offence-height-eligible custody using a complete indexed key slice.
pub(crate) fn indexed_slashable_validator_exposure(
    world: &impl WorldReadOnly,
    lane_id: LaneId,
    validator: &AccountId,
    record: &PublicLaneValidatorRecord,
    offence_height: u64,
    share_keys: &[PublicLaneStakeShareKey],
) -> Result<Quantity, Error> {
    let shares = validator_share_updates(world, lane_id, validator, Some(share_keys))?;
    slashable_exposure_from_shares(record, &shares, Some(offence_height))
}

fn validator_share_updates(
    world: &impl WorldReadOnly,
    lane_id: LaneId,
    validator: &AccountId,
    indexed_share_keys: Option<&[PublicLaneStakeShareKey]>,
) -> Result<Vec<(PublicLaneStakeShareKey, PublicLaneStakeShare)>, Error> {
    let mut shares = Vec::new();
    if let Some(keys) = indexed_share_keys {
        if !keys.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(Error::InvariantViolation(
                "indexed public-lane stake-share keys are not canonical".into(),
            ));
        }
        for key in keys {
            if key.0 != lane_id || &key.1 != validator {
                return Err(Error::InvariantViolation(
                    "indexed public-lane stake-share key belongs to another validator".into(),
                ));
            }
            let Some(share) = world.public_lane_stake_shares().get(key) else {
                // An earlier slash in the same ordered consensus bundle may
                // have consumed and removed this exact indexed share.
                continue;
            };
            if !public_lane_stake_share_matches_key(key, share) {
                return Err(Error::InvariantViolation(
                    "public-lane stake share does not match its storage key".into(),
                ));
            }
            shares.push((key.clone(), share.clone()));
        }
        return Ok(shares);
    }

    for (key, share) in world.public_lane_stake_shares().iter() {
        if key.0 != lane_id || &key.1 != validator {
            continue;
        }
        if !public_lane_stake_share_matches_key(key, share) {
            return Err(Error::InvariantViolation(
                "public-lane stake share does not match its storage key".into(),
            ));
        }
        shares.push((key.clone(), share.clone()));
    }
    Ok(shares)
}

fn pending_unbond_is_slashable_at(
    pending: &PublicLaneUnbonding,
    offence_height: Option<u64>,
) -> bool {
    offence_height.is_none_or(|height| height <= pending.slashable_through_height)
}

fn slashable_exposure_from_shares(
    record: &PublicLaneValidatorRecord,
    shares: &[(PublicLaneStakeShareKey, PublicLaneStakeShare)],
    offence_height: Option<u64>,
) -> Result<Quantity, Error> {
    if record.stake_account != record.validator {
        return Err(Error::InvariantViolation(
            "public-lane validator stake account must match the validator account".into(),
        ));
    }
    let mut bonded = Quantity::zero();
    let mut self_bonded = Quantity::zero();
    let mut pending_unbonds = Quantity::zero();
    for (key, share) in shares {
        bonded = quantity_add(bonded, share.bonded.clone())?;
        if key.2 == record.stake_account {
            self_bonded = quantity_add(self_bonded, share.bonded.clone())?;
        }
        for (request_id, pending) in &share.pending_unbonds {
            ensure_canonical_pending_unbond(request_id, pending)?;
            if pending_unbond_is_slashable_at(pending, offence_height) {
                pending_unbonds = quantity_add(pending_unbonds, pending.amount.clone())?;
            }
        }
    }
    if bonded != record.total_stake || self_bonded != record.self_stake {
        return Err(Error::InvariantViolation(
            "public-lane validator totals do not match canonical stake shares".into(),
        ));
    }
    quantity_add(bonded, pending_unbonds)
}
fn min_quantity(lhs: Quantity, rhs: Quantity) -> Quantity {
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
fn unbond_liability_release_height(
    state_transaction: &StateTransaction<'_, '_>,
    slashable_through_height: u64,
) -> Result<u64, Error> {
    let parameters = state_transaction
        .world
        .sumeragi_npos_parameters()
        .ok_or_else(|| {
            Error::InvariantViolation(
                "public-lane unbonding requires signed NPoS parameters".into(),
            )
        })?;
    slashable_through_height
        .checked_add(parameters.evidence_horizon_blocks())
        .and_then(|height| height.checked_add(parameters.slashing_delay_blocks()))
        .ok_or_else(|| {
            Error::InvariantViolation("public-lane unbond liability height overflows u64".into())
        })
}
fn ensure_canonical_pending_unbond(
    request_id: &Hash,
    pending: &PublicLaneUnbonding,
) -> Result<(), Error> {
    if request_id != &pending.request_id
        || pending.amount.is_zero()
        || pending.slashable_through_height == 0
        || pending.liability_release_height < pending.slashable_through_height
    {
        return Err(Error::InvariantViolation(
            "public-lane pending unbond is non-canonical".into(),
        ));
    }
    Ok(())
}
pub(crate) fn meets_min_stake(amount: &Quantity, minimum: &Quantity) -> Result<bool, Error> {
    Ok(amount >= minimum)
}
fn slash_within_limit(amount: &Quantity, total: &Quantity, max_bps: u16) -> Result<bool, Error> {
    Ok(!amount
        .cmp_mul_u64(10_000, total, u64::from(max_bps))
        .is_gt())
}
fn is_self_stake_share_staker(
    staker: &AccountId,
    validator: &AccountId,
    stake_account: &AccountId,
) -> bool {
    staker == validator || staker == stake_account
}
/// Compute the maximum slashable amount given a total bonded stake and max ratio.
pub(crate) fn max_slash_amount(total: &Quantity, max_bps: u16) -> Result<Quantity, Error> {
    if total.is_zero() || max_bps == 0 {
        return Ok(Quantity::zero());
    }
    if max_bps >= 10_000 {
        return Ok(total.clone());
    }
    let amount = total
        .try_mul_div_decimal_round(
            &Numeric::from(u64::from(max_bps)),
            &Numeric::from(10_000_u64),
            total.scale(),
            RoundingMode::TowardZero,
        )
        .map_err(|_| Error::Math(MathError::Overflow))?;
    Ok(amount)
}
fn ensure_validator_peer_registered(
    state_transaction: &StateTransaction<'_, '_>,
    lane_id: LaneId,
    validator: &AccountId,
    validator_peer: &PeerId,
    required_live_height: u64,
) -> Result<(), Error> {
    if !state_transaction
        .world
        .peers
        .iter()
        .any(|peer| peer == validator_peer)
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
    let gate = peer_consensus_key_gate(&state_transaction.world, validator_peer, block_height);
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
    let peer_public_key = validator_peer.public_key();
    let has_unbounded_validator_key = state_transaction
        .world
        .consensus_keys_by_pk()
        .get(&peer_public_key.to_string())
        .is_some_and(|ids| {
            ids.iter().any(|id| {
                id.role == ConsensusKeyRole::Validator
                    && state_transaction
                        .world
                        .consensus_keys()
                        .get(id)
                        .is_some_and(|record| {
                            record.id == *id
                                && record.public_key == *peer_public_key
                                && record.expiry_height.is_none()
                                && record.is_live_at(required_live_height, 0, 0)
                        })
            })
        });
    if !has_unbounded_validator_key {
        #[cfg(feature = "telemetry")]
        state_transaction
            .telemetry
            .record_public_lane_validator_reject("consensus_key_not_unbounded");
        iroha_logger::warn!(
            lane_id = %lane_id,
            validator = %validator,
            peer = %validator_peer,
            required_live_height,
            "public-lane validator action rejected: no unbounded validator consensus key covers the scheduled activation height"
        );
        return Err(Error::InvariantViolation(
            "validator peer requires an unbounded validator consensus key live at its scheduled activation height"
                .into(),
        ));
    }
    let commit_topology: Vec<_> = state_transaction.commit_topology.iter().cloned().collect();
    if !commit_topology.is_empty()
        && commit_topology
            .iter()
            .all(|peer_in_topology| peer_in_topology != validator_peer)
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
fn ensure_no_pending_evidence_for_validator(
    state_transaction: &StateTransaction<'_, '_>,
    record: &PublicLaneValidatorRecord,
    operation: &str,
) -> Result<(), Error> {
    if crate::sumeragi::evidence::has_pending_v2_evidence_for_validator_tenure(
        &state_transaction.world,
        record,
    ) {
        return Err(Error::InvariantViolation(
            format!(
                "{operation} rejected while unresolved consensus evidence liens this validator"
            )
            .into(),
        ));
    }
    Ok(())
}
fn slash_bonded_share_group(
    shares: &mut [((LaneId, AccountId, AccountId), PublicLaneStakeShare)],
    validator: &AccountId,
    stake_account: &AccountId,
    self_stake_group: bool,
    amount: &Quantity,
) -> Result<(), Error> {
    let mut remaining = amount.clone();
    for ((_, _, staker), share) in shares {
        if remaining.is_zero() {
            break;
        }
        if is_self_stake_share_staker(staker, validator, stake_account) != self_stake_group
            || share.bonded.is_zero()
        {
            continue;
        }
        let slash_part = min_quantity(share.bonded.clone(), remaining.clone());
        share.bonded = quantity_sub(share.bonded.clone(), slash_part.clone())?;
        remaining = quantity_sub(remaining, slash_part)?;
    }
    if !remaining.is_zero() {
        return Err(Error::InvariantViolation(
            "slash could not be satisfied by bonded stake shares".into(),
        ));
    }
    Ok(())
}
fn slash_pending_unbond_group(
    shares: &mut [((LaneId, AccountId, AccountId), PublicLaneStakeShare)],
    validator: &AccountId,
    stake_account: &AccountId,
    self_stake_group: bool,
    amount: &Quantity,
    offence_height: Option<u64>,
) -> Result<(), Error> {
    let mut remaining = amount.clone();
    for ((_, _, staker), share) in shares {
        if remaining.is_zero() {
            break;
        }
        if is_self_stake_share_staker(staker, validator, stake_account) != self_stake_group {
            continue;
        }
        let request_ids = share.pending_unbonds.keys().copied().collect::<Vec<_>>();
        for request_id in request_ids {
            if remaining.is_zero() {
                break;
            }
            let pending = share
                .pending_unbonds
                .get_mut(&request_id)
                .expect("request id was collected from this exact share");
            ensure_canonical_pending_unbond(&request_id, pending)?;
            if !pending_unbond_is_slashable_at(pending, offence_height) {
                continue;
            }
            let slash_part = min_quantity(pending.amount.clone(), remaining.clone());
            pending.amount = quantity_sub(pending.amount.clone(), slash_part.clone())?;
            remaining = quantity_sub(remaining, slash_part)?;
            if pending.amount.is_zero() {
                share.pending_unbonds.remove(&request_id);
            }
        }
    }
    if !remaining.is_zero() {
        return Err(Error::InvariantViolation(
            "slash could not be satisfied by pending unbond custody".into(),
        ));
    }
    Ok(())
}
fn pending_unbond_group_total(
    shares: &[((LaneId, AccountId, AccountId), PublicLaneStakeShare)],
    validator: &AccountId,
    stake_account: &AccountId,
    self_stake_group: bool,
    offence_height: Option<u64>,
) -> Result<Quantity, Error> {
    let mut total = Quantity::zero();
    for ((_, _, staker), share) in shares {
        if is_self_stake_share_staker(staker, validator, stake_account) != self_stake_group {
            continue;
        }
        for (request_id, pending) in &share.pending_unbonds {
            ensure_canonical_pending_unbond(request_id, pending)?;
            if pending_unbond_is_slashable_at(pending, offence_height) {
                total = quantity_add(total, pending.amount.clone())?;
            }
        }
    }
    Ok(total)
}
/// Apply a slash to a validator through the central retained-movement path.
pub(crate) fn apply_slash_to_validator(
    state_transaction: &mut StateTransaction<'_, '_>,
    lane_id: LaneId,
    validator: &AccountId,
    offence_height: u64,
    slash_id: Hash,
    amount: &Quantity,
    now_ms: u64,
) -> Result<(), Error> {
    apply_slash_to_validator_inner(
        state_transaction,
        lane_id,
        validator,
        slash_id,
        amount,
        now_ms,
        Some(offence_height),
        None,
        true,
        true,
    )
}
/// Apply a finality-owned slash with commit-boundary metrics but no transaction evidence.
pub(crate) fn apply_consensus_slash_to_validator(
    state_transaction: &mut StateTransaction<'_, '_>,
    lane_id: LaneId,
    validator: &AccountId,
    slash_id: Hash,
    amount: &Quantity,
    now_ms: u64,
) -> Result<(), Error> {
    apply_slash_to_validator_inner(
        state_transaction,
        lane_id,
        validator,
        slash_id,
        amount,
        now_ms,
        None,
        None,
        false,
        true,
    )
}
/// Apply a slash in a disposable validation transaction without external observability effects.
pub(crate) fn apply_slash_to_validator_without_observability(
    state_transaction: &mut StateTransaction<'_, '_>,
    lane_id: LaneId,
    validator: &AccountId,
    slash_id: Hash,
    amount: &Quantity,
    now_ms: u64,
) -> Result<(), Error> {
    apply_slash_to_validator_inner(
        state_transaction,
        lane_id,
        validator,
        slash_id,
        amount,
        now_ms,
        None,
        None,
        false,
        false,
    )
}
/// Apply a finality-owned slash using a complete key slice from one indexed overlay.
pub(crate) fn apply_indexed_consensus_slash_to_validator(
    state_transaction: &mut StateTransaction<'_, '_>,
    lane_id: LaneId,
    validator: &AccountId,
    slash_id: Hash,
    amount: &Quantity,
    now_ms: u64,
    offence_height: u64,
    share_keys: &[PublicLaneStakeShareKey],
) -> Result<(), Error> {
    apply_slash_to_validator_inner(
        state_transaction,
        lane_id,
        validator,
        slash_id,
        amount,
        now_ms,
        Some(offence_height),
        Some(share_keys),
        false,
        true,
    )
}
/// Validate a finality-owned slash using a complete key slice without external effects.
pub(crate) fn apply_indexed_slash_to_validator_without_observability(
    state_transaction: &mut StateTransaction<'_, '_>,
    lane_id: LaneId,
    validator: &AccountId,
    slash_id: Hash,
    amount: &Quantity,
    now_ms: u64,
    offence_height: u64,
    share_keys: &[PublicLaneStakeShareKey],
) -> Result<(), Error> {
    apply_slash_to_validator_inner(
        state_transaction,
        lane_id,
        validator,
        slash_id,
        amount,
        now_ms,
        Some(offence_height),
        Some(share_keys),
        false,
        false,
    )
}
#[allow(clippy::too_many_arguments)]
fn apply_slash_to_validator_inner(
    state_transaction: &mut StateTransaction<'_, '_>,
    lane_id: LaneId,
    validator: &AccountId,
    slash_id: Hash,
    amount: &Quantity,
    now_ms: u64,
    offence_height: Option<u64>,
    indexed_share_keys: Option<&[PublicLaneStakeShareKey]>,
    record_execution_evidence: bool,
    record_operational_observability: bool,
) -> Result<(), Error> {
    ensure_canonical_staking_owner(state_transaction, lane_id, "apply_slash_to_validator")?;
    let deactivation_height = scheduled_validator_deactivation_height(state_transaction)?;
    let dataspace_catalog = state_transaction.nexus.dataspace_catalog.clone();
    let staking_cfg = state_transaction.nexus.staking.clone();
    let world = &state_transaction.world;
    let validator_key = validator_storage_key(lane_id, validator);
    let validator_snapshot = world
        .public_lane_validators
        .get(&validator_key)
        .map(|record| {
            ensure_public_lane_validator_record_matches_key(&validator_key, record)?;
            Ok::<PublicLaneValidatorRecord, Error>(record.clone())
        })
        .transpose()?
        .ok_or_else(|| Error::InvariantViolation("validator not registered".into()))?;
    if let Some(offence_height) = offence_height
        && !validator_tenure_contains_height(&validator_snapshot, offence_height)?
    {
        return Err(Error::InvariantViolation(
            "consensus slash offence falls outside the validator tenure".into(),
        ));
    }
    let stake_account = validator_snapshot.stake_account.clone();
    let stake_ctx = stake_context(
        world,
        &dataspace_catalog,
        &staking_cfg,
        &stake_account,
        None,
        now_ms,
    )?;
    let spec = world
        .asset_definitions
        .get(&stake_ctx.asset_definition)
        .ok_or_else(|| Error::InvariantViolation("stake asset definition missing".into()))?
        .spec();
    assert_numeric_spec_with(amount.as_numeric(), spec)?;
    let slashed_status = PublicLaneValidatorStatus::Slashed(slash_id);
    let previous_status = validator_snapshot.status.clone();
    if indexed_share_keys.is_some_and(|keys| {
        keys.len()
            > usize::try_from(staking_cfg.max_stake_shares_per_validator.get())
                .expect("u32 stake-share cap fits usize on supported targets")
    }) {
        return Err(Error::InvariantViolation(
            "indexed public-lane stake-share slice exceeds configured capacity".into(),
        ));
    }
    let mut share_updates = validator_share_updates(world, lane_id, validator, indexed_share_keys)?;
    let slashable_exposure =
        slashable_exposure_from_shares(&validator_snapshot, &share_updates, offence_height)?;
    let allowed = slash_within_limit(amount, &slashable_exposure, staking_cfg.max_slash_bps)?;
    if !allowed {
        return Err(Error::InvariantViolation(
            "slash exceeds configured maximum ratio".into(),
        ));
    }
    if slashable_exposure < amount.clone() {
        return Err(Error::InvariantViolation(
            "slash exceeds stake still held in protocol custody".into(),
        ));
    }
    let self_pending = pending_unbond_group_total(
        &share_updates,
        validator,
        &validator_snapshot.stake_account,
        true,
        offence_height,
    )?;
    let self_exposure = quantity_add(validator_snapshot.self_stake.clone(), self_pending)?;
    let self_slash = min_quantity(self_exposure, amount.clone());
    let self_bonded_slash = min_quantity(validator_snapshot.self_stake.clone(), self_slash.clone());
    let self_pending_slash = quantity_sub(self_slash.clone(), self_bonded_slash.clone())?;
    let delegated_slash = quantity_sub(amount.clone(), self_slash)?;
    let delegated_bonded = quantity_sub(
        validator_snapshot.total_stake.clone(),
        validator_snapshot.self_stake.clone(),
    )?;
    let delegated_bonded_slash = min_quantity(delegated_bonded, delegated_slash.clone());
    let delegated_pending_slash = quantity_sub(delegated_slash, delegated_bonded_slash.clone())?;
    let bonded_slash = quantity_add(self_bonded_slash.clone(), delegated_bonded_slash.clone())?;
    let pending_slash_amount =
        quantity_add(self_pending_slash.clone(), delegated_pending_slash.clone())?;
    let new_total_stake =
        quantity_sub(validator_snapshot.total_stake.clone(), bonded_slash.clone())?;
    let new_self_stake = quantity_sub(
        validator_snapshot.self_stake.clone(),
        self_bonded_slash.clone(),
    )?;
    let mut validator_update = validator_snapshot.clone();
    validator_update.total_stake = new_total_stake;
    validator_update.self_stake = new_self_stake;
    schedule_validator_deactivation(&mut validator_update, deactivation_height)?;
    validator_update.status = slashed_status.clone();
    slash_bonded_share_group(
        &mut share_updates,
        validator,
        &validator_snapshot.stake_account,
        true,
        &self_bonded_slash,
    )?;
    slash_pending_unbond_group(
        &mut share_updates,
        validator,
        &validator_snapshot.stake_account,
        true,
        &self_pending_slash,
        offence_height,
    )?;
    slash_bonded_share_group(
        &mut share_updates,
        validator,
        &validator_snapshot.stake_account,
        false,
        &delegated_bonded_slash,
    )?;
    slash_pending_unbond_group(
        &mut share_updates,
        validator,
        &validator_snapshot.stake_account,
        false,
        &delegated_pending_slash,
        offence_height,
    )?;
    let movement = VerifiedStakingSlashDebit::new(
        lane_id,
        validator.clone(),
        slash_id,
        stake_ctx.escrow_asset.clone(),
        stake_ctx.slash_sink_asset.clone(),
        amount.clone(),
        slashable_exposure,
    );
    // A consensus slash is a finality effect rather than a transaction
    // entrypoint. Its balance movement must not enter the transaction FASTPQ
    // transcript or process-global execution witness. Commit mode publishes
    // only the slash-specific status and telemetry below.
    if record_execution_evidence {
        crate::smartcontracts::isi::asset::isi::execute_verified_staking_slash_transfer(
            state_transaction,
            movement,
        )?;
    } else {
        crate::smartcontracts::isi::asset::isi::execute_verified_consensus_staking_slash_transfer(
            state_transaction,
            movement,
        )?;
    }
    let world = &mut state_transaction.world;
    world
        .public_lane_validators
        .insert(validator_key, validator_update);
    for (key, share) in share_updates {
        if !share.bonded.is_zero() || !share.pending_unbonds.is_empty() {
            world.public_lane_stake_shares.insert(key, share);
        } else {
            world.public_lane_stake_shares.remove(key);
        }
    }
    if record_operational_observability {
        state_transaction.stage_public_lane_slash_observability(
            lane_id,
            previous_status,
            slashed_status,
            bonded_slash,
            pending_slash_amount,
        );
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
/// Check that an unbond movement uses the configured stake definition and escrow.
pub(in crate::smartcontracts::isi) fn is_configured_staking_unbond_movement(
    state_transaction: &StateTransaction<'_, '_>,
    staker: &AccountId,
    source_id: &AssetId,
    destination_id: &AssetId,
) -> Result<bool, Error> {
    let context = stake_context(
        &state_transaction.world,
        &state_transaction.nexus.dataspace_catalog,
        &state_transaction.nexus.staking,
        staker,
        None,
        state_transaction.block_unix_timestamp_ms(),
    )?;
    Ok(source_id == &context.escrow_asset && destination_id == &context.staker_asset)
}
/// Check that a slash movement uses the configured stake escrow and slash sink.
pub(in crate::smartcontracts::isi) fn is_configured_staking_slash_movement(
    state_transaction: &StateTransaction<'_, '_>,
    stake_account: &AccountId,
    source_id: &AssetId,
    destination_id: &AssetId,
) -> Result<bool, Error> {
    let context = stake_context(
        &state_transaction.world,
        &state_transaction.nexus.dataspace_catalog,
        &state_transaction.nexus.staking,
        stake_account,
        None,
        state_transaction.block_unix_timestamp_ms(),
    )?;
    Ok(source_id == &context.escrow_asset && destination_id == &context.slash_sink_asset)
}
fn stake_context(
    world: &impl WorldReadOnly,
    dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
    staking_cfg: &iroha_config::parameters::actual::NexusStaking,
    staker: &AccountId,
    slash_sink_override: Option<&AccountId>,
    now_ms: u64,
) -> Result<StakeEscrowContext, Error> {
    let asset_definition = resolve_configured_asset_definition(
        world,
        &staking_cfg.stake_asset_id,
        "nexus.staking.stake_asset_id",
        now_ms,
    )?;
    let escrow_account = parse_staking_account_literal(
        world,
        dataspace_catalog,
        &staking_cfg.stake_escrow_account_id,
        "stake_escrow_account_id",
        now_ms,
    )?;
    let slash_sink_account: AccountId = if let Some(account) = slash_sink_override {
        account.clone()
    } else {
        parse_staking_account_literal(
            world,
            dataspace_catalog,
            &staking_cfg.slash_sink_account_id,
            "slash_sink_account_id",
            now_ms,
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
    dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
    literal: &str,
    field: &'static str,
    now_ms: u64,
) -> Result<AccountId, Error> {
    if let Some(account) =
        crate::block::parse_account_literal_with_world(world, dataspace_catalog, literal, now_ms)
            .map_err(|error| Error::InvariantViolation(error.to_string().into()))?
    {
        return Ok(account);
    }
    let reason = match AccountId::parse_encoded(literal) {
        Ok(_) => "literal resolved to no matching account in world state".to_owned(),
        Err(err) => format!("decode failed: {err}"),
    };
    Err(Error::InvariantViolation(
        format!(
            "invalid nexus.staking.{field}; expected canonical I105 account id or on-chain alias ({reason})"
        )
        .into(),
    ))
}
fn resolve_configured_asset_definition(
    world: &impl WorldReadOnly,
    literal: &str,
    field: &'static str,
    now_ms: u64,
) -> Result<AssetDefinitionId, Error> {
    crate::block::parse_asset_definition_literal_with_world(world, literal, now_ms).ok_or_else(
        || {
            Error::InvariantViolation(
                format!(
                    "invalid {field}; expected canonical Base58 asset definition id or active asset alias"
                )
                .into(),
            )
        },
    )
}
fn resolve_nexus_fee_asset_definition(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<AssetDefinitionId, Error> {
    resolve_configured_asset_definition(
        &state_transaction.world,
        &state_transaction.nexus.fees.fee_asset_id,
        "nexus.fees.fee_asset_id",
        state_transaction.block_unix_timestamp_ms(),
    )
}
fn assert_stake_amount_matches_spec(
    state_transaction: &mut StateTransaction<'_, '_>,
    asset_definition: &AssetDefinitionId,
    amount: &Quantity,
) -> Result<(), Error> {
    let spec = state_transaction
        .numeric_spec_for(asset_definition)
        .map_err(Error::from)?;
    assert_numeric_spec_with(amount.as_numeric(), spec)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        block::ValidBlock,
        query::store::LiveQueryStore,
        state::{State, StateBlock, StateTransaction, World},
    };
    use core::num::{NonZeroU32, NonZeroU64};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        account::{Account, MultisigMember, MultisigPolicy},
        asset::{AssetDefinition, AssetDefinitionId},
        block::{
            consensus::{
                Evidence, EvidencePenaltyStatus, EvidenceRecord, SumeragiV2EquivocationEvidence,
            },
            consensus_v2::{
                BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
                ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
                SumeragiV2Equivocation, ValidatorPower, Vote,
            },
        },
        consensus::{ConsensusKeyRecord, ConsensusKeyStatus},
        domain::Domain,
        isi::error::InvalidParameterError,
        nexus::{
            AUTOSCALE_META_CREATED_HEIGHT, AUTOSCALE_META_MANAGED, DataSpaceId, LaneCatalog,
            LaneConfig, LaneVisibility, PublicLaneRewardShare,
        },
        parameter::{Parameter, system::SumeragiNposParameters},
        peer::Peer,
        prelude::*,
    };
    use iroha_primitives::numeric::{Numeric, Quantity};
    use iroha_test_samples::{ALICE_ID, gen_account_in};
    use nonzero_ext::nonzero;
    use std::time::Duration;
    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("staking fixture key generation should succeed")
    }
    fn checked_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm)
            .expect("staking algorithm-specific fixture key generation should succeed")
    }
    fn checked_peer_id() -> crate::PeerId {
        crate::PeerId::from(checked_keypair().public_key().clone())
    }
    include!("staking_core_tests.rs");
    #[test]
    fn stake_context_accepts_i105_account_literals() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        stx.nexus.staking.stake_asset_id = asset_def_id.to_string();
        stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        stx.nexus.staking.slash_sink_account_id = escrow.to_string();
        let stake_ctx = stake_context(
            &stx.world,
            &stx.nexus.dataspace_catalog,
            &stx.nexus.staking,
            &validator,
            None,
            stx.block_unix_timestamp_ms(),
        )
        .expect("stake context should accept i105 literals");
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
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
        assert_eq!(record.total_stake, Quantity::from(1_000_u64));
        assert_eq!(record.self_stake, Quantity::from(1_000_u64));
        let share = view
            .world
            .public_lane_stake_shares()
            .get(&(LaneId::new(1), validator.clone(), validator.clone()))
            .expect("share record");
        assert_eq!(share.bonded, Quantity::from(1_000_u64));
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
        assert_eq!(stake_balance.as_ref(), &Quantity::from(9_000_u64));
        assert_eq!(escrow_balance.as_ref(), &Quantity::from(1_000_u64));
    }
    #[test]
    fn register_allows_genesis_bootstrap_authority() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(81),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("genesis authority may bootstrap validator registration");
        assert!(
            stx.world
                .public_lane_validators
                .get(&(LaneId::new(81), validator))
                .is_some(),
            "genesis bootstrap registration must create a validator record"
        );
    }
    #[test]
    fn register_rejects_non_validator_authority() {
        let state = setup_state();
        let block = new_block_with_height(2);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let err = RegisterPublicLaneValidator {
            lane_id: LaneId::new(81),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("non-validator authority must not register validator stake");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg)
                if msg.contains("register_public_lane_validator rejected: authority must match validator account")
        ));
        assert!(
            stx.world
                .public_lane_validators
                .get(&(LaneId::new(81), validator))
                .is_none(),
            "rejected registration must not create a validator record"
        );
        assert!(
            stx.world
                .assets
                .get(&AssetId::new(asset_def_id, escrow))
                .is_none(),
            "rejected registration must not touch escrow"
        );
    }
    #[test]
    fn register_rejects_non_validator_stake_account() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let err = RegisterPublicLaneValidator {
            lane_id: LaneId::new(82),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: delegator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect_err("registration must not pull initial self-stake from another account");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg)
                if msg.contains("register_public_lane_validator rejected: stake account must match validator account")
        ));
        assert!(
            stx.world
                .public_lane_validators
                .get(&(LaneId::new(82), validator))
                .is_none(),
            "rejected registration must not create a validator record"
        );
        assert!(
            stx.world
                .assets
                .get(&AssetId::new(asset_def_id.clone(), escrow))
                .is_none(),
            "rejected registration must not touch escrow"
        );
        let delegator_stake = stx
            .world
            .assets
            .get(&AssetId::new(asset_def_id, delegator))
            .expect("delegator stake balance remains present");
        assert_eq!(delegator_stake.as_ref(), &Quantity::from(10_000_u64));
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx);
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx);
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx);
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
    fn register_rejects_finite_consensus_key_for_open_ended_tenure() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let mut state_block = state.block(block_header_with_height(4));
        let mut stx = state_block.transaction();
        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let validator_peer = validator_peer_id(&validator);
        seed_validator_consensus_key_with_heights(
            &mut stx,
            &validator_peer,
            ConsensusKeyStatus::Active,
            1,
            Some(8),
        );

        let result = RegisterPublicLaneValidator {
            lane_id: LaneId::new(24),
            peer_id: validator_peer,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx);

        assert!(
            matches!(&result, Err(Error::InvariantViolation(msg)) if msg.contains("unbounded validator consensus key")),
            "unexpected result: {result:?}"
        );
        assert!(
            stx.world
                .assets
                .get(&AssetId::new(asset_def_id, escrow))
                .is_none(),
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
        set_transaction_lane_catalog(
            &mut stx,
            LaneCatalog::new(
                nonzero!(8_u32),
                vec![LaneConfig {
                    id: lane_id,
                    alias: "restricted".to_string(),
                    dataspace_id: DataSpaceId::UNIVERSAL,
                    visibility: LaneVisibility::Restricted,
                    ..LaneConfig::default()
                }],
            )
            .expect("lane catalog"),
        );
        stx.nexus.staking.public_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected;
        stx.nexus.staking.restricted_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let result = RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
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
    fn staking_rejects_future_created_autoscale_lane_before_creation_height() {
        let state = setup_state();
        let block = new_block_with_height(6);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let future_lane = LaneId::new(1);
        let mut autoscale_lane = LaneConfig {
            id: future_lane,
            alias: "elastic-lane-1".to_string(),
            dataspace_id: DataSpaceId::UNIVERSAL,
            visibility: LaneVisibility::Public,
            ..LaneConfig::default()
        };
        autoscale_lane
            .metadata
            .insert(AUTOSCALE_META_MANAGED.to_owned(), "true".to_owned());
        autoscale_lane
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.to_owned(), "7".to_owned());
        crate::state::attach_synthetic_autoscale_committee_for_test(&mut autoscale_lane);
        stx.nexus.autoscale.enabled = true;
        stx.nexus.autoscale.min_lane_id = nonzero!(1_u32);
        stx.nexus.autoscale.max_lane_id_exclusive = nonzero!(2_u32);
        set_transaction_lane_catalog(
            &mut stx,
            LaneCatalog::new(nonzero!(2_u32), vec![LaneConfig::default(), autoscale_lane])
                .expect("future-created autoscale catalog"),
        );
        stx.nexus.staking.public_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::StakeElected;
        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let err = RegisterPublicLaneValidator {
            lane_id: future_lane,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect_err("future-created autoscale lane must reject staking before creation height");
        assert!(matches!(
            err,
            Error::InvalidParameter(InvalidParameterError::SmartContract(msg))
                if msg.contains("not active at block height 6")
        ));
        assert!(
            stx.world
                .public_lane_validators
                .iter()
                .all(|((lane, _), _)| lane != &future_lane),
            "rejected future-created autoscale lane must not create validator records"
        );
        let escrow_asset = AssetId::new(asset_def_id, escrow);
        assert!(
            stx.world.assets.get(&escrow_asset).is_none(),
            "rejected future-created autoscale lane must not touch escrow"
        );
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn mixed_mode_respects_lane_validator_modes() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let block = new_block_with_height(1);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let stake_lane = LaneId::new(0);
        let admin_lane = LaneId::new(1);
        set_transaction_lane_catalog(
            &mut stx,
            LaneCatalog::new(
                nonzero!(2_u32),
                vec![
                    LaneConfig {
                        id: stake_lane,
                        alias: "public-stake".to_string(),
                        dataspace_id: DataSpaceId::UNIVERSAL,
                        visibility: LaneVisibility::Public,
                        ..LaneConfig::default()
                    },
                    LaneConfig {
                        id: admin_lane,
                        alias: "restricted-admin".to_string(),
                        dataspace_id: DataSpaceId::UNIVERSAL,
                        visibility: LaneVisibility::Restricted,
                        ..LaneConfig::default()
                    },
                ],
            )
            .expect("lane catalog"),
        );
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("stake-elected lane should accept staking");
        let err = RegisterPublicLaneValidator {
            lane_id: admin_lane,
            peer_id: validator_peer_id(&delegator),
            validator: delegator.clone(),
            stake_account: delegator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&delegator, &mut stx)
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
        let roster = view
            .epoch_validator_peer_ids_for_testing(0)
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
        set_transaction_lane_catalog(
            &mut stx,
            LaneCatalog::new(
                nonzero!(2_u32),
                vec![LaneConfig {
                    id: LaneId::new(1),
                    alias: "restricted".to_string(),
                    dataspace_id: DataSpaceId::UNIVERSAL,
                    visibility: LaneVisibility::Restricted,
                    ..LaneConfig::default()
                }],
            )
            .expect("lane catalog"),
        );
        stx.nexus.staking.restricted_validator_mode =
            iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;
        let domain_id: DomainId = DomainId::try_new("council", "universal").expect("domain id");
        let selector = crate::sns::selector_for_domain(&domain_id).expect("selector");
        let address =
            iroha_data_model::account::AccountAddress::from_account_id(&ALICE_ID).expect("address");
        let record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            ALICE_ID.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        stx.world.smart_contract_state.insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register domain");
        let member_a = checked_keypair_with_algorithm(Algorithm::Ed25519);
        let member_b = checked_keypair_with_algorithm(Algorithm::Ed25519);
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(member_a.public_key().clone(), 1).expect("member_a"),
                MultisigMember::new(member_b.public_key().clone(), 1).expect("member_b"),
            ],
        )
        .expect("policy");
        let admin_id = AccountId::new_multisig(policy);
        Register::account(Account::new(admin_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register multisig admin");
        let bls = checked_keypair_with_algorithm(Algorithm::BlsNormal);
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
        let foreign_peer = checked_peer_id();
        stx.commit_topology.get_mut().clear();
        stx.commit_topology.get_mut().push(foreign_peer);
        let result = RegisterPublicLaneValidator {
            lane_id: LaneId::new(42),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx);
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
    fn register_accepts_distinct_validator_peer_binding() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let foreign_peer = checked_peer_id();
        let _ = stx.world.peers.push(foreign_peer.clone());
        seed_validator_consensus_key(&mut stx, &foreign_peer, ConsensusKeyStatus::Active);
        stx.commit_topology.get_mut().clear();
        stx.commit_topology.get_mut().push(foreign_peer.clone());
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(43),
            peer_id: foreign_peer.clone(),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("registration should accept a distinct peer binding");
        let record = stx
            .world
            .public_lane_validators()
            .get(&(LaneId::new(43), validator.clone()))
            .expect("validator record should be stored");
        assert_eq!(record.peer_id, foreign_peer);
        assert_ne!(record.peer_id, validator_peer_id(&validator));
    }
    #[test]
    fn register_rejects_peer_binding_retained_by_exited_custody() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, replacement, _, _) = prepare_accounts(&mut stx);
        let shared_peer = validator_peer_id(&validator);
        stx.commit_topology.get_mut().clear();
        stx.commit_topology.get_mut().push(shared_peer.clone());
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(44),
            peer_id: shared_peer.clone(),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("first validator should bind the shared peer");
        stx.world
            .public_lane_validators
            .get_mut(&(LaneId::new(44), validator.clone()))
            .expect("first validator record")
            .status = PublicLaneValidatorStatus::Exited;
        let err = RegisterPublicLaneValidator {
            lane_id: LaneId::new(44),
            peer_id: shared_peer,
            validator: replacement.clone(),
            stake_account: replacement.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&replacement, &mut stx)
        .expect_err("exited custody must continue reserving its peer binding");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg) if msg.contains("validator peer is already retained for lane")
        ));
    }
    #[test]
    fn register_ignores_mismatched_public_lane_validator_rows_for_capacity_and_peer_binding() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, stale_validator, _, _) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(144);
        let shared_peer = validator_peer_id(&validator);
        stx.nexus.staking.max_validators = nonzero!(1u32);
        stx.world.public_lane_validators.insert(
            (lane_id, stale_validator.clone()),
            PublicLaneValidatorRecord {
                lane_id: LaneId::new(145),
                validator: stale_validator.clone(),
                peer_id: shared_peer.clone(),
                stake_account: stale_validator,
                total_stake: Quantity::from(10_000_u64),
                self_stake: Quantity::from(10_000_u64),
                metadata: Metadata::default(),
                status: PublicLaneValidatorStatus::Active,
                activation_height: 1,
                deactivation_height: None,
                last_reward_epoch: None,
            },
        );
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: shared_peer,
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("mismatched stale row must not consume capacity or peer binding");
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("valid validator should be registered");
        assert_eq!(record.lane_id, lane_id);
        assert!(matches!(
            record.status,
            PublicLaneValidatorStatus::PendingActivation(_)
        ));
    }
    #[test]
    fn rebind_updates_pending_validator_peer_and_preserves_record_fields() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let block = new_block_with_height(4);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let replacement_peer = checked_peer_id();
        let _ = stx.world.peers.push(replacement_peer.clone());
        seed_validator_consensus_key(&mut stx, &replacement_peer, ConsensusKeyStatus::Active);
        stx.commit_topology.get_mut().push(replacement_peer.clone());
        let lane_id = LaneId::new(45);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        let original = stx
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()))
            .expect("validator record")
            .clone();
        RebindPublicLaneValidatorPeer::new(lane_id, validator.clone(), replacement_peer.clone())
            .execute(&validator, &mut stx)
            .expect("rebind should succeed before validator activation");
        let updated = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator.clone()))
            .expect("updated validator record");
        assert_eq!(updated.peer_id, replacement_peer);
        assert_eq!(updated.validator, original.validator);
        assert_eq!(updated.stake_account, original.stake_account);
        assert_eq!(updated.total_stake, original.total_stake);
        assert_eq!(updated.self_stake, original.self_stake);
        assert_eq!(updated.status, original.status);
        assert_eq!(updated.activation_height, original.activation_height);
        assert_eq!(updated.deactivation_height, original.deactivation_height);
        assert_eq!(updated.last_reward_epoch, original.last_reward_epoch);
    }
    #[test]
    fn rebind_is_allowed_before_but_not_during_activation_roster_freeze() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let mut registration_block = state.block(block_header_with_height(4));
        let mut registration_stx = registration_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut registration_stx);
        let first_replacement = checked_peer_id();
        let second_replacement = checked_peer_id();
        for peer in [&first_replacement, &second_replacement] {
            let _ = registration_stx.world.peers.push(peer.clone());
            seed_validator_consensus_key(&mut registration_stx, peer, ConsensusKeyStatus::Active);
        }
        let lane_id = LaneId::new(145);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut registration_stx)
        .expect("register validator for height seven");
        registration_stx.apply();
        registration_block
            .commit_world_overlay_for_testing()
            .unwrap();

        let mut safe_block = state.block(block_header_with_height(5));
        let mut safe_stx = safe_block.transaction();
        safe_stx
            .commit_topology
            .get_mut()
            .push(first_replacement.clone());
        RebindPublicLaneValidatorPeer::new(lane_id, validator.clone(), first_replacement.clone())
            .execute(&validator, &mut safe_stx)
            .expect("height five is before the height-seven roster freeze");
        safe_stx.apply();
        safe_block.commit_world_overlay_for_testing().unwrap();

        let mut frozen_block = state.block(block_header_with_height(6));
        let mut frozen_stx = frozen_block.transaction();
        frozen_stx
            .commit_topology
            .get_mut()
            .push(second_replacement.clone());
        let err =
            RebindPublicLaneValidatorPeer::new(lane_id, validator.clone(), second_replacement)
                .execute(&validator, &mut frozen_stx)
                .expect_err("height six has already frozen the height-seven roster");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg) if msg.contains("already frozen")
        ));
        let record = frozen_stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("validator record remains present");
        assert_eq!(record.peer_id, first_replacement);
    }
    #[test]
    fn rebind_same_peer_is_idempotent_without_revalidation() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(46);
        let peer_id = validator_peer_id(&validator);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: peer_id.clone(),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        stx.commit_topology.get_mut().clear();
        if let Some(index) = stx.world.peers.iter().position(|peer| peer == &peer_id) {
            let _ = stx.world.peers.remove(index);
        }
        RebindPublicLaneValidatorPeer::new(lane_id, validator.clone(), peer_id.clone())
            .execute(&validator, &mut stx)
            .expect("same-peer rebind should be idempotent");
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("validator record remains present");
        assert_eq!(record.peer_id, peer_id);
    }
    #[test]
    fn rebind_rejects_duplicate_peer_binding_on_same_lane() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let block = new_block_with_height(4);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, replacement, _, _) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(47);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register primary validator");
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&replacement),
            validator: replacement.clone(),
            stake_account: replacement.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&replacement, &mut stx)
        .expect("register replacement validator");
        let err = RebindPublicLaneValidatorPeer::new(
            lane_id,
            validator.clone(),
            validator_peer_id(&replacement),
        )
        .execute(&validator, &mut stx)
        .expect_err("duplicate binding should be rejected");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg) if msg.contains("validator peer is already registered for lane")
        ));
    }
    #[test]
    fn rebind_ignores_mismatched_public_lane_validator_rows_for_peer_binding() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let block = new_block_with_height(4);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, stale_validator, _, _) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(147);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register primary validator");
        let replacement_peer = checked_peer_id();
        let _ = stx.world.peers.push(replacement_peer.clone());
        seed_validator_consensus_key(&mut stx, &replacement_peer, ConsensusKeyStatus::Active);
        stx.commit_topology.get_mut().push(replacement_peer.clone());
        stx.world.public_lane_validators.insert(
            (lane_id, stale_validator.clone()),
            PublicLaneValidatorRecord {
                lane_id: LaneId::new(148),
                validator: stale_validator.clone(),
                peer_id: replacement_peer.clone(),
                stake_account: stale_validator,
                total_stake: Quantity::from(10_000_u64),
                self_stake: Quantity::from(10_000_u64),
                metadata: Metadata::default(),
                status: PublicLaneValidatorStatus::Active,
                activation_height: 1,
                deactivation_height: None,
                last_reward_epoch: None,
            },
        );
        RebindPublicLaneValidatorPeer::new(lane_id, validator.clone(), replacement_peer.clone())
            .execute(&validator, &mut stx)
            .expect("mismatched stale row must not reserve the replacement peer");
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("validator record remains present");
        assert_eq!(record.peer_id, replacement_peer);
    }
    #[test]
    fn rebind_rejects_mismatched_public_lane_validator_row() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(152);
        let replacement_peer = checked_peer_id();
        let _ = stx.world.peers.push(replacement_peer.clone());
        seed_validator_consensus_key(&mut stx, &replacement_peer, ConsensusKeyStatus::Active);
        stx.commit_topology.get_mut().push(replacement_peer.clone());
        insert_validator_record_for_key(
            &mut stx,
            lane_id,
            LaneId::new(153),
            &validator,
            PublicLaneValidatorStatus::Active,
            Quantity::from(1_000_u64),
        );
        let err = RebindPublicLaneValidatorPeer::new(lane_id, validator.clone(), replacement_peer)
            .execute(&validator, &mut stx)
            .expect_err("mismatched validator row must reject peer rebinding");
        assert!(
            matches!(err, Error::InvariantViolation(msg) if msg.contains("does not match its storage key"))
        );
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("mismatched record remains present");
        assert_eq!(record.lane_id, LaneId::new(153));
        assert_eq!(record.peer_id, validator_peer_id(&record.validator));
    }
    #[test]
    fn rebind_requires_validator_authority() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let replacement_peer = checked_peer_id();
        let _ = stx.world.peers.push(replacement_peer.clone());
        seed_validator_consensus_key(&mut stx, &replacement_peer, ConsensusKeyStatus::Active);
        stx.commit_topology.get_mut().push(replacement_peer.clone());
        let lane_id = LaneId::new(48);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        let err = RebindPublicLaneValidatorPeer::new(lane_id, validator.clone(), replacement_peer)
            .execute(&ALICE_ID, &mut stx)
            .expect_err("non-validator authority should be rejected");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg) if msg.contains("authority must match validator account")
        ));
    }
    #[test]
    fn rebind_rejects_disallowed_validator_statuses() {
        let statuses = [
            PublicLaneValidatorStatus::Active,
            PublicLaneValidatorStatus::Exiting(5),
            PublicLaneValidatorStatus::Exited,
            PublicLaneValidatorStatus::Slashed(Hash::new("slash-rebind")),
        ];
        for (index, status) in statuses.into_iter().enumerate() {
            let state = setup_state();
            let block = new_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            let (validator, _, _, _) = prepare_accounts(&mut stx);
            let replacement_peer = checked_peer_id();
            let _ = stx.world.peers.push(replacement_peer.clone());
            seed_validator_consensus_key(&mut stx, &replacement_peer, ConsensusKeyStatus::Active);
            stx.commit_topology.get_mut().push(replacement_peer.clone());
            let lane_id = LaneId::new(49 + u32::try_from(index).expect("index fits in u32"));
            RegisterPublicLaneValidator {
                lane_id,
                peer_id: validator_peer_id(&validator),
                validator: validator.clone(),
                stake_account: validator.clone(),
                initial_stake: Quantity::from(1_000_u64),
                metadata: Metadata::default(),
            }
            .execute(&validator, &mut stx)
            .expect("register validator");
            stx.world
                .public_lane_validators
                .get_mut(&(lane_id, validator.clone()))
                .expect("validator record")
                .status = status;
            let err =
                RebindPublicLaneValidatorPeer::new(lane_id, validator.clone(), replacement_peer)
                    .execute(&validator, &mut stx)
                    .expect_err("disallowed status should reject peer rebinding");
            assert!(matches!(
                err,
                Error::InvariantViolation(msg) if msg.contains("only before validator activation")
            ));
        }
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx);
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
    fn finalize_pending_activations_ignores_mismatched_public_lane_validator_rows() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let mut state_block = state.block(block_header_with_height(7));
        let mut stx = state_block.transaction();
        let (validator, mismatched_validator, _, _) = prepare_accounts(&mut stx);
        let valid_lane = LaneId::new(149);
        let mismatched_key_lane = LaneId::new(150);
        let mismatched_record_lane = LaneId::new(151);
        stx.world.public_lane_validators.insert(
            (valid_lane, validator.clone()),
            PublicLaneValidatorRecord {
                lane_id: valid_lane,
                validator: validator.clone(),
                peer_id: validator_peer_id(&validator),
                stake_account: validator.clone(),
                total_stake: Quantity::from(1_000_u64),
                self_stake: Quantity::from(1_000_u64),
                metadata: Metadata::default(),
                status: PublicLaneValidatorStatus::PendingActivation(7),
                activation_height: 7,
                deactivation_height: None,
                last_reward_epoch: None,
            },
        );
        stx.world.public_lane_validators.insert(
            (mismatched_key_lane, mismatched_validator.clone()),
            PublicLaneValidatorRecord {
                lane_id: mismatched_record_lane,
                validator: mismatched_validator.clone(),
                peer_id: validator_peer_id(&mismatched_validator),
                stake_account: mismatched_validator.clone(),
                total_stake: Quantity::from(9_000_u64),
                self_stake: Quantity::from(9_000_u64),
                metadata: Metadata::default(),
                status: PublicLaneValidatorStatus::PendingActivation(7),
                activation_height: 7,
                deactivation_height: None,
                last_reward_epoch: None,
            },
        );
        finalize_pending_activations(&mut stx).expect("activation pass should complete");
        let valid = stx
            .world
            .public_lane_validators()
            .get(&(valid_lane, validator))
            .expect("valid pending validator remains present");
        assert!(matches!(valid.status, PublicLaneValidatorStatus::Active));
        assert_eq!(valid.activation_height, 7);
        assert_eq!(valid.deactivation_height, None);
        let mismatched = stx
            .world
            .public_lane_validators()
            .get(&(mismatched_key_lane, mismatched_validator))
            .expect("mismatched pending validator remains present");
        assert!(
            matches!(
                mismatched.status,
                PublicLaneValidatorStatus::PendingActivation(7)
            ),
            "mismatched key/record rows must not auto-promote to Active"
        );
        assert_eq!(mismatched.activation_height, 7);
        assert_eq!(mismatched.deactivation_height, None);
    }
    #[test]
    fn finalize_pending_activations_skips_non_owner_same_dataspace_rows() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);

        let mut state_block = state.block(block_header_with_height(7));
        let mut stx = state_block.transaction();
        let (owner_validator, sibling_validator, _, _) = prepare_accounts(&mut stx);
        let owner_lane = LaneId::SINGLE;
        let sibling_lane = LaneId::new(1);
        set_transaction_lane_catalog(
            &mut stx,
            LaneCatalog::new(
                nonzero!(2_u32),
                vec![
                    LaneConfig::default(),
                    LaneConfig {
                        id: sibling_lane,
                        alias: "pending-shared-staking-sibling".to_owned(),
                        dataspace_id: DataSpaceId::UNIVERSAL,
                        visibility: LaneVisibility::Public,
                        ..LaneConfig::default()
                    },
                ],
            )
            .expect("shared-dataspace lane catalog"),
        );
        insert_validator_record_for_key(
            &mut stx,
            owner_lane,
            owner_lane,
            &owner_validator,
            PublicLaneValidatorStatus::PendingActivation(7),
            Quantity::from(1_000_u64),
        );
        insert_validator_record_for_key(
            &mut stx,
            sibling_lane,
            sibling_lane,
            &sibling_validator,
            PublicLaneValidatorStatus::PendingActivation(7),
            Quantity::from(1_000_u64),
        );

        finalize_pending_activations(&mut stx).expect("activation pass should complete");

        let owner = stx
            .world
            .public_lane_validators()
            .get(&(owner_lane, owner_validator))
            .expect("canonical owner validator remains present");
        assert!(matches!(owner.status, PublicLaneValidatorStatus::Active));
        let sibling = stx
            .world
            .public_lane_validators()
            .get(&(sibling_lane, sibling_validator))
            .expect("non-owner sibling validator remains present");
        assert!(matches!(
            sibling.status,
            PublicLaneValidatorStatus::PendingActivation(7)
        ));
        assert_eq!(sibling.activation_height, 7);
        assert_eq!(sibling.deactivation_height, None);
    }

    #[test]
    fn activate_public_lane_validator_rejects_mismatched_public_lane_validator_row() {
        let state = setup_state();
        let mut state_block = state.block(block_header_with_height(1));
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(154);
        insert_validator_record_for_key(
            &mut stx,
            lane_id,
            LaneId::new(155),
            &validator,
            PublicLaneValidatorStatus::PendingActivation(0),
            Quantity::from(1_000_u64),
        );
        let err = ActivatePublicLaneValidator {
            lane_id,
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("mismatched validator row must reject explicit activation");
        assert!(
            matches!(err, Error::InvariantViolation(msg) if msg.contains("does not match its storage key"))
        );
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("mismatched record remains present");
        assert!(matches!(
            record.status,
            PublicLaneValidatorStatus::PendingActivation(0)
        ));
        assert_eq!(record.activation_height, 0);
        assert_eq!(record.deactivation_height, None);
    }
    #[test]
    fn finalize_released_exits_ignores_mismatched_public_lane_validator_rows() {
        let state = setup_state();
        let block = new_block_with_height_and_time(2, 1_000);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, mismatched_validator, _, _) = prepare_accounts(&mut stx);
        let valid_lane = LaneId::new(156);
        let mismatched_key_lane = LaneId::new(157);
        insert_validator_record_for_key(
            &mut stx,
            valid_lane,
            valid_lane,
            &validator,
            PublicLaneValidatorStatus::Exiting(999),
            Quantity::from(1_000_u64),
        );
        insert_validator_record_for_key(
            &mut stx,
            mismatched_key_lane,
            LaneId::new(158),
            &mismatched_validator,
            PublicLaneValidatorStatus::Exiting(999),
            Quantity::from(1_000_u64),
        );
        finalize_released_exits(&mut stx);
        let valid = stx
            .world
            .public_lane_validators()
            .get(&(valid_lane, validator))
            .expect("valid exiting validator remains present");
        assert!(matches!(valid.status, PublicLaneValidatorStatus::Exited));
        let mismatched = stx
            .world
            .public_lane_validators()
            .get(&(mismatched_key_lane, mismatched_validator))
            .expect("mismatched exiting validator remains present");
        assert!(matches!(
            mismatched.status,
            PublicLaneValidatorStatus::Exiting(999)
        ));
    }
    #[test]
    fn pending_activation_advances_on_epoch_boundary() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        // Start in epoch 1 to avoid genesis, which allows immediate activation.
        let mut state_block = state.block(block_header_with_height(4));
        let mut stx = state_block.transaction();
        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(1),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        let pending = stx
            .world
            .public_lane_validators
            .get(&(LaneId::new(1), validator.clone()))
            .expect("pending record");
        assert!(matches!(
            pending.status,
            PublicLaneValidatorStatus::PendingActivation(7)
        ));
        assert_eq!(pending.activation_height, 7);
        assert_eq!(pending.deactivation_height, None);
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
        let mut activate_block = state.block(block_header_with_height(5));
        let mut activate_stx = activate_block.transaction();
        let err = ActivatePublicLaneValidator {
            lane_id: LaneId::new(1),
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut activate_stx)
        .expect_err("activation should wait for the next epoch");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg) if msg.contains("activation height not reached")
        ));
        activate_stx.apply();
        activate_block.commit_world_overlay_for_testing().unwrap();
        let mut intermediate_block = state.block(block_header_with_height(6));
        let intermediate_stx = intermediate_block.transaction();
        intermediate_stx.apply();
        intermediate_block
            .commit_world_overlay_for_testing()
            .unwrap();
        let mut activate_block = state.block(block_header_with_height(7));
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
        assert_eq!(record.activation_height, 7);
        assert_eq!(record.deactivation_height, None);
        activate_stx.apply();
        activate_block.commit_world_overlay_for_testing().unwrap();
        let view = state.view();
        let stored = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(1), validator.clone()))
            .expect("stored record");
        assert_eq!(stored.activation_height, 7);
        assert_eq!(stored.deactivation_height, None);
        let escrow_asset = AssetId::new(asset_def_id, escrow.clone());
        assert!(
            view.world.assets().get(&escrow_asset).is_some(),
            "staking assets must remain bonded"
        );
    }
    #[test]
    fn boundary_block_registration_targets_the_following_election() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let mut state_block = state.block(block_header_with_height(6));
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);

        RegisterPublicLaneValidator {
            lane_id: LaneId::new(1),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator in the epoch-ending block");

        let record = stx
            .world
            .public_lane_validators
            .get(&(LaneId::new(1), validator))
            .expect("pending validator record");
        assert_eq!(
            record.status,
            PublicLaneValidatorStatus::PendingActivation(10),
            "height seven's roster is already frozen before height-six transactions execute"
        );
        assert_eq!(record.activation_height, 10);
        assert_eq!(record.deactivation_height, None);
    }
    #[test]
    fn genesis_activation_allows_same_block() {
        let state = setup_state();
        let mut state_block = state.block(block_header_with_height(1));
        let mut stx = state_block.transaction();
        let (validator, _delegator, _escrow, _asset_def_id) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator in genesis");
        let pending = stx
            .world
            .public_lane_validators
            .get(&(LaneId::SINGLE, validator.clone()))
            .expect("pending record");
        assert!(matches!(
            pending.status,
            PublicLaneValidatorStatus::PendingActivation(1)
        ));
        assert_eq!(pending.activation_height, 1);
        assert_eq!(pending.deactivation_height, None);
        ActivatePublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("activate validator in genesis");
        let record = stx
            .world
            .public_lane_validators
            .get(&(LaneId::SINGLE, validator))
            .cloned()
            .expect("record present");
        assert!(matches!(record.status, PublicLaneValidatorStatus::Active));
        assert_eq!(record.activation_height, 1);
        assert_eq!(record.deactivation_height, None);
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
    }
    #[test]
    fn rewards_reject_non_active_validator() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(2_u32),
            vec![LaneConfig {
                id: LaneId::new(1),
                alias: "rewards-lane".to_string(),
                dataspace_id: DataSpaceId::UNIVERSAL,
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
        Mint::asset_quantity(1_000u32, reward_asset.clone())
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(1),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        if let Some(record) = stx
            .world
            .public_lane_validators
            .get_mut(&(LaneId::new(1), validator.clone()))
        {
            record.status = PublicLaneValidatorStatus::PendingActivation(2);
            record.activation_height = 2;
            record.deactivation_height = None;
        }
        let err = RecordPublicLaneRewards {
            lane_id: LaneId::new(1),
            epoch: 0,
            reward_asset,
            total_reward: Quantity::from(10_u64),
            shares: vec![PublicLaneRewardShare {
                account: validator.clone(),
                role: PublicLaneRewardRole::Validator,
                amount: Quantity::from(10_u64),
            }],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("inactive validator should not receive rewards");
        assert!(
            matches!(&err, Error::InvariantViolation(msg) if msg.contains("outside its active tenure")),
            "unexpected error: {err:?}"
        );
        let record = stx
            .world
            .public_lane_validators
            .get_mut(&(LaneId::new(1), validator.clone()))
            .expect("validator record remains available");
        record.status = PublicLaneValidatorStatus::Exiting(10);
        record.activation_height = 1;
        record.deactivation_height = Some(2);
        ensure_reward_targets_active(
            &stx,
            LaneId::new(1),
            &[PublicLaneRewardShare {
                account: validator,
                role: PublicLaneRewardRole::Validator,
                amount: Quantity::from(10_u64),
            }],
        )
        .expect("a terminal lifecycle label cannot revoke a still-frozen validator tenure");
    }
    #[test]
    fn record_rewards_rejects_mismatched_public_lane_validator_row() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let lane_id = LaneId::new(59);
        let (_sink, validator, reward_asset, _) = configure_reward_fixture(&mut stx, lane_id, 500);
        let record = stx
            .world
            .public_lane_validators
            .get_mut(&(lane_id, validator.clone()))
            .expect("validator record");
        record.lane_id = LaneId::new(60);
        record.status = PublicLaneValidatorStatus::Active;
        let err = RecordPublicLaneRewards {
            lane_id,
            epoch: 1,
            reward_asset,
            total_reward: Quantity::from(25_u64),
            shares: vec![PublicLaneRewardShare {
                account: validator.clone(),
                role: PublicLaneRewardRole::Validator,
                amount: Quantity::from(25_u64),
            }],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("mismatched validator row must not receive rewards");
        assert!(
            matches!(err, Error::InvariantViolation(ref msg) if msg.contains("outside its active tenure")),
            "unexpected error: {err:?}"
        );
        assert!(
            stx.world.public_lane_rewards.get(&(lane_id, 1)).is_none(),
            "rejected reward record must not be persisted"
        );
    }
    #[test]
    fn out_of_order_sibling_lane_rewards_preserve_canonical_owner_reward_epoch() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let owner_lane = LaneId::SINGLE;
        let serviced_lane = LaneId::new(1);
        let (_sink, validator, reward_asset, _) =
            configure_reward_fixture(&mut stx, owner_lane, 500);
        stx.world
            .public_lane_validators
            .get_mut(&(owner_lane, validator.clone()))
            .expect("canonical owner validator")
            .status = PublicLaneValidatorStatus::Active;
        set_transaction_lane_catalog(
            &mut stx,
            LaneCatalog::new(
                nonzero!(2_u32),
                vec![
                    LaneConfig::default(),
                    LaneConfig {
                        id: serviced_lane,
                        alias: "reward-serviced-sibling".to_owned(),
                        dataspace_id: DataSpaceId::UNIVERSAL,
                        visibility: LaneVisibility::Public,
                        ..LaneConfig::default()
                    },
                ],
            )
            .expect("shared-dataspace reward lane catalog"),
        );

        let validator_share = PublicLaneRewardShare {
            account: validator.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: Quantity::from(25_u64),
        };
        RecordPublicLaneRewards {
            lane_id: owner_lane,
            epoch: 10,
            reward_asset: reward_asset.clone(),
            total_reward: Quantity::from(25_u64),
            shares: vec![validator_share.clone()],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("the canonical lane may first record its later epoch");

        RecordPublicLaneRewards {
            lane_id: serviced_lane,
            epoch: 5,
            reward_asset,
            total_reward: Quantity::from(25_u64),
            shares: vec![validator_share],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("a serviced sibling may record rewards for the shared validator cohort");

        assert!(
            stx.world
                .public_lane_rewards
                .get(&(serviced_lane, 5))
                .is_some(),
            "the reward ledger remains keyed to the serviced lane"
        );
        assert_eq!(
            stx.world
                .public_lane_validators
                .get(&(owner_lane, validator.clone()))
                .expect("canonical owner validator after reward")
                .last_reward_epoch,
            Some(10),
            "an older sibling-lane epoch must not regress the shared validator marker"
        );
        assert!(
            stx.world
                .public_lane_validators
                .get(&(serviced_lane, validator))
                .is_none(),
            "reward recording must not create a sibling stake projection"
        );
    }

    #[test]
    fn update_validator_rewards_ignores_mismatched_public_lane_validator_rows() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(161);
        insert_validator_record_for_key(
            &mut stx,
            lane_id,
            LaneId::new(162),
            &validator,
            PublicLaneValidatorStatus::Active,
            Quantity::from(1_000_u64),
        );
        update_validator_rewards(
            &mut stx,
            lane_id,
            7,
            &[PublicLaneRewardShare {
                account: validator.clone(),
                role: PublicLaneRewardRole::Validator,
                amount: Quantity::from(10_u64),
            }],
        );
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("mismatched validator row remains present");
        assert_eq!(record.last_reward_epoch, None);
    }
    #[test]
    fn pending_activation_auto_promotes_at_epoch_boundary() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        // Start in epoch 1 to avoid genesis, which allows immediate activation.
        let block1 = new_block_with_height(4);
        let mut state_block = state.block(block1.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(1),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
        let block2 = new_block_with_height(5);
        let mut state_block2 = state.block(block2.as_ref().header());
        let stx2 = state_block2.transaction();
        stx2.apply();
        state_block2.commit_world_overlay_for_testing().unwrap();
        let block3 = new_block_with_height(6);
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
                PublicLaneValidatorStatus::PendingActivation(7)
            ),
            "validator should remain pending until the next epoch"
        );
        drop(view);
        let mut state_block3 = state.block(block3.as_ref().header());
        let stx3 = state_block3.transaction();
        stx3.apply();
        state_block3.commit_world_overlay_for_testing().unwrap();
        let block4 = new_block_with_height(7);
        let state_block4 = state.block(block4.as_ref().header());
        let record = state_block4
            .world
            .public_lane_validators
            .get(&(LaneId::new(1), validator.clone()))
            .cloned()
            .expect("record present after scheduling");
        assert!(
            matches!(record.status, PublicLaneValidatorStatus::Active),
            "validator should auto-activate at the next epoch boundary"
        );
        assert_eq!(record.activation_height, 7);
        assert_eq!(record.deactivation_height, None);
        state_block4.commit_world_overlay_for_testing().unwrap();
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
        assert_eq!(stored.activation_height, 7);
        assert_eq!(stored.deactivation_height, None);
        let escrow_asset = AssetId::new(asset_def_id, escrow);
        assert!(
            view.world.assets().get(&escrow_asset).is_some(),
            "staking assets must remain bonded"
        );
    }
    #[test]
    fn pending_activation_rejects_regressing_activation_height() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let block1 = new_block_with_height(1);
        let mut state_block = state.block(block1.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(11),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        let key = (LaneId::new(11), validator.clone());
        stx.world
            .public_lane_validators
            .get_mut(&key)
            .expect("pending record")
            .activation_height = 99;
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
        // Insert an intermediate block to keep heights monotonic before the activation block.
        let block2 = new_block_with_height(2);
        let mut mid_block = state.block(block2.as_ref().header());
        let mid_tx = mid_block.transaction();
        mid_tx.apply();
        mid_block.commit_world_overlay_for_testing().unwrap();
        let block3 = new_block_with_height(3);
        let mut activation_block = state.block(block3.as_ref().header());
        let mut stx3 = activation_block.transaction();
        let err = finalize_pending_activations(&mut stx3)
            .expect_err("regressing activation height must be rejected");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg) if msg.contains("activation height is not canonical")
        ));
    }
    #[test]
    fn unregister_peer_retains_validator_capacity_while_custody_exists() {
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
        Mint::asset_quantity(
            10_000u32,
            AssetId::new(asset_def_id.clone(), replacement.clone()),
        )
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(7),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("first validator should register");
        let second_attempt = RegisterPublicLaneValidator {
            lane_id: LaneId::new(7),
            peer_id: validator_peer_id(&replacement),
            validator: replacement.clone(),
            stake_account: replacement.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&replacement, &mut stx);
        assert!(
            matches!(&second_attempt, Err(Error::InvariantViolation(msg)) if msg.contains("maximum validator capacity")),
            "capacity guard should reject second validator: {second_attempt:?}"
        );
        let validator_peer = validator_peer_id(&validator);
        stx.commit_topology
            .get_mut()
            .retain(|peer| peer != &validator_peer);
        let unregister_error = Unregister::<Peer>::peer(validator_peer)
            .execute(&ALICE_ID, &mut stx)
            .expect_err("retained validator tenure must reject peer unregistration");
        assert!(
            unregister_error
                .to_string()
                .contains("wait for its deactivation height"),
            "unexpected unregistration rejection: {unregister_error}"
        );
        let error = RegisterPublicLaneValidator {
            lane_id: LaneId::new(7),
            peer_id: validator_peer_id(&replacement),
            validator: replacement.clone(),
            stake_account: replacement.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&replacement, &mut stx)
        .expect_err("retained custody must continue to consume validator capacity");
        assert!(matches!(
            error,
            Error::InvariantViolation(message)
                if message.contains("maximum validator capacity")
        ));
        let former = stx
            .world
            .public_lane_validators
            .get(&(LaneId::new(7), validator.clone()))
            .expect("former validator record");
        assert!(
            matches!(former.status, PublicLaneValidatorStatus::Active),
            "rejected peer removal must leave the validator active"
        );
        let remaining_shares: Vec<_> = stx
            .world
            .public_lane_stake_shares
            .iter()
            .filter(|((lane, val, _), _)| *lane == LaneId::new(7) && val == &validator)
            .collect();
        assert_eq!(
            remaining_shares.len(),
            1,
            "peer removal must preserve slashable stake custody"
        );
    }
    #[test]
    fn exited_validator_retains_capacity_while_custody_exists() {
        let state = setup_state();
        let block = new_block_with_height_and_time(1, 0);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);
        let (validator, _delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(0);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        ExitPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            release_at_ms: stx.block_unix_timestamp_ms().saturating_add(5),
        }
        .execute(&validator, &mut stx)
        .expect("mark exiting validator");
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
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
        Mint::asset_quantity(
            10_000u32,
            AssetId::new(asset_def_id.clone(), replacement.clone()),
        )
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        let error = RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&replacement),
            validator: replacement.clone(),
            stake_account: replacement.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&replacement, &mut stx)
        .expect_err("released exit with retained custody must consume capacity");
        assert!(matches!(
            error,
            Error::InvariantViolation(message)
                if message.contains("maximum validator capacity")
        ));
        let stale_record = stx
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()));
        assert!(
            matches!(stale_record, Some(record) if matches!(record.status, PublicLaneValidatorStatus::Exited)),
            "exited validator custody record must remain after replacement admission"
        );
        let remaining_shares: Vec<_> = stx
            .world
            .public_lane_stake_shares
            .iter()
            .filter(|((lane, val, _), _)| *lane == lane_id && val == &validator)
            .collect();
        assert_eq!(
            remaining_shares.len(),
            1,
            "exit finalization must preserve slashable stake custody"
        );
    }
    #[test]
    fn final_unbond_prunes_exited_record_and_frees_validator_capacity() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let block = new_block_with_height_and_time(1, 0);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);
        let (validator, _, escrow, asset_definition) = prepare_accounts(&mut stx);
        set_test_npos_penalty_windows(&mut stx, 1, 1);
        let (replacement, _) = gen_account_in("nexus");
        Register::account(Account::new(replacement.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register replacement account");
        let replacement_peer = register_peer_for_account(&mut stx, &replacement);
        stx.commit_topology.get_mut().push(replacement_peer);
        Mint::asset_quantity(
            10_000_u32,
            AssetId::new(asset_definition.clone(), replacement.clone()),
        )
        .execute(&ALICE_ID, &mut stx)
        .expect("fund replacement stake account");
        let lane_id = LaneId::new(13);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(500_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register original validator");
        let request_id = Hash::new("final-exit-unbond");
        SchedulePublicLaneUnbond {
            lane_id,
            validator: validator.clone(),
            staker: validator.clone(),
            request_id,
            amount: Quantity::from(500_u64),
            release_at_ms: stx.block_unix_timestamp_ms(),
        }
        .execute(&validator, &mut stx)
        .expect("schedule complete self-unbond");
        ExitPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            release_at_ms: stx.block_unix_timestamp_ms(),
        }
        .execute(&validator, &mut stx)
        .expect("exit zero-bonded validator while pending custody remains");
        assert!(
            stx.world
                .public_lane_validators
                .get(&(lane_id, validator.clone()))
                .is_some(),
            "pending unbond custody must retain the exited validator record"
        );
        stx.apply();
        state_block
            .commit_world_overlay_for_testing()
            .expect("commit exited validator custody");

        let block = new_block_with_height_and_time(5, 0);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);
        stx.nexus.staking.stake_asset_id = asset_definition.to_string();
        stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        stx.nexus.staking.slash_sink_account_id = escrow.to_string();
        FinalizePublicLaneUnbond {
            lane_id,
            validator: validator.clone(),
            staker: validator.clone(),
            request_id,
        }
        .execute(&validator, &mut stx)
        .expect("final liability height releases complete custody");
        assert!(
            stx.world
                .public_lane_validators
                .get(&(lane_id, validator))
                .is_none(),
            "zero-custody exited records must be pruned at the deactivation boundary"
        );
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&replacement),
            validator: replacement.clone(),
            stake_account: replacement.clone(),
            initial_stake: Quantity::from(500_u64),
            metadata: Metadata::default(),
        }
        .execute(&replacement, &mut stx)
        .expect("released terminal custody must free validator capacity");
    }
    #[test]
    fn slashed_validator_cannot_reregister_until_custody_is_released() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let block = new_block_with_height_and_time(1, 0);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);
        let (validator, _delegator, escrow, _asset_def_id) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(12);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
        let block2 = new_block_with_height_and_time(2, 5);
        let mut state_block = state.block(block2.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);
        stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        stx.nexus.staking.slash_sink_account_id = escrow.to_string();
        SlashPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            offence_height: 1,
            slash_id: Hash::new("slash-reenter"),
            amount: Quantity::from(100_u64),
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
        .execute(&validator, &mut stx)
        .expect("exit slashed validator");
        finalize_validator_lifecycle(&mut stx).expect("finalize exit lifecycle");
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
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
        let err = RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect_err("exited validator must not overwrite retained slashable custody");
        assert!(matches!(
            err,
            Error::InvariantViolation(message)
                if message.contains("retains slashable stake custody")
        ));
        let record = stx
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()))
            .expect("exited record remains");
        assert!(
            matches!(record.status, PublicLaneValidatorStatus::Exited),
            "rejected re-registration must retain the exited lifecycle state"
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
            "rejected re-registration must preserve the original stake share"
        );
        assert_eq!(shares[0].1.bonded, Quantity::from(900_u64));
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn exiting_and_exited_validator_block_capacity_while_custody_exists() {
        let state = setup_state();
        let block = new_block_with_height_and_time(1, 0);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);
        let (validator, _delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(10);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        let release_at_ms = stx.block_unix_timestamp_ms().saturating_add(50);
        ExitPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            release_at_ms,
        }
        .execute(&validator, &mut stx)
        .expect("mark validator exiting");
        stx.apply();
        state_block.commit_empty_block_for_testing().unwrap();
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
            Mint::asset_quantity(
                10_000u32,
                AssetId::new(asset_def_id.clone(), replacement.clone()),
            )
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
            stx.apply();
            replacement
        };
        state_block.commit_empty_block_for_testing().unwrap();
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
                peer_id: validator_peer_id(&replacement),
                validator: replacement.clone(),
                stake_account: replacement.clone(),
                initial_stake: Quantity::from(1_000_u64),
                metadata: Metadata::default(),
            }
            .execute(&replacement, &mut stx)
            .expect_err("capacity should remain consumed until exit finalizes");
            assert!(matches!(
                err,
                Error::InvariantViolation(msg) if msg.contains("maximum validator capacity")
            ));
            drop(stx);
        }
        state_block.commit_empty_block_for_testing().unwrap();
        // The release timestamp only finalizes lifecycle status; bonded custody
        // continues to consume the bounded validator slot.
        let block4 = new_block_with_height_and_time(4, 60);
        let mut state_block = state.block(block4.as_ref().header());
        {
            let mut stx = state_block.transaction();
            stx.nexus.staking.max_validators = nonzero!(1u32);
            stx.nexus.staking.stake_asset_id = asset_def_id.to_string();
            stx.nexus.staking.stake_escrow_account_id = escrow.to_string();
            stx.nexus.staking.slash_sink_account_id = escrow.to_string();
            let error = RegisterPublicLaneValidator {
                lane_id,
                peer_id: validator_peer_id(&replacement),
                validator: replacement.clone(),
                stake_account: replacement.clone(),
                initial_stake: Quantity::from(1_000_u64),
                metadata: Metadata::default(),
            }
            .execute(&replacement, &mut stx)
            .expect_err("exited custody must retain validator capacity");
            assert!(matches!(
                error,
                Error::InvariantViolation(message)
                    if message.contains("maximum validator capacity")
            ));
        }
        state_block.commit_empty_block_for_testing().unwrap();
        let view = state.view();
        let roster = view
            .world
            .public_lane_validators()
            .iter()
            .filter(|((lane, _), _)| *lane == lane_id)
            .map(|((_, id), record)| (id.clone(), record.status.clone()))
            .collect::<Vec<_>>();
        assert!(
            roster.iter().all(|(id, _)| id != &replacement),
            "replacement must remain excluded while exited custody is retained"
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
        let lane_id = LaneId::new(15);
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(16_u32),
            vec![LaneConfig {
                id: lane_id,
                alias: "stake-snapshot-15".to_string(),
                dataspace_id: DataSpaceId::UNIVERSAL,
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
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
            let roster = view
                .epoch_validator_peer_ids_for_testing(0)
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
        let roster = view
            .epoch_validator_peer_ids_for_testing(0)
            .unwrap_or_default();
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
        let lane_id = LaneId::new(16);
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(17_u32),
            vec![LaneConfig {
                id: lane_id,
                alias: "stake-snapshot-16".to_string(),
                dataspace_id: DataSpaceId::UNIVERSAL,
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
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
            let roster = view
                .epoch_validator_peer_ids_for_testing(0)
                .expect("validator should appear before topology change");
            assert!(
                roster.contains(&validator_peer),
                "validator should be present before topology change"
            );
        }
        let block = new_block_with_height(2);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let foreign_peer = checked_peer_id();
        let _ = stx.world.peers.push(foreign_peer.clone());
        stx.commit_topology.get_mut().clear();
        stx.commit_topology.get_mut().push(foreign_peer);
        stx.apply();
        record_block_commit(&mut state_block, &block);
        state_block.commit().unwrap();
        let view = state.view();
        let roster = view
            .epoch_validator_peer_ids_for_testing(0)
            .unwrap_or_default();
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
        let lane_id = LaneId::new(17);
        stx.nexus.lane_catalog = LaneCatalog::new(
            nonzero!(18_u32),
            vec![LaneConfig {
                id: lane_id,
                alias: "stake-snapshot-17".to_string(),
                dataspace_id: DataSpaceId::UNIVERSAL,
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
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
            let roster = view
                .epoch_validator_peer_ids_for_testing(0)
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
        let roster = view
            .epoch_validator_peer_ids_for_testing(0)
            .unwrap_or_default();
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
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        set_test_npos_penalty_windows(&mut stx, 1, 1);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(7),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(500_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        BondPublicLaneStake {
            lane_id: LaneId::new(7),
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Quantity::from(250_u64),
            metadata: Metadata::default(),
        }
        .execute(&delegator, &mut stx)
        .unwrap();
        let release_at_ms = stx.block_unix_timestamp_ms();
        SchedulePublicLaneUnbond {
            lane_id: LaneId::new(7),
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("req"),
            amount: Quantity::from(100_u64),
            release_at_ms,
        }
        .execute(&delegator, &mut stx)
        .unwrap();
        let pending = &stx
            .world
            .public_lane_stake_shares
            .get(&(LaneId::new(7), validator.clone(), delegator.clone()))
            .expect("scheduled delegator unbond")
            .pending_unbonds[&Hash::new("req")];
        assert_eq!(pending.slashable_through_height, 3);
        assert_eq!(pending.liability_release_height, 5);
        assert!(
            stx.world
                .public_lane_validators
                .get(&(LaneId::new(7), validator.clone()))
                .is_some(),
            "validator not stored before apply"
        );
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
        let early_block = new_block_with_height_and_time(2, release_at_ms);
        let mut early_state_block = state.block(early_block.as_ref().header());
        let mut early_tx = early_state_block.transaction();
        early_tx.nexus.staking.stake_asset_id = asset_def_id.to_string();
        early_tx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        early_tx.nexus.staking.slash_sink_account_id = escrow.to_string();
        let error = FinalizePublicLaneUnbond {
            lane_id: LaneId::new(7),
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("req"),
        }
        .execute(&delegator, &mut early_tx)
        .expect_err("stake must remain in custody throughout the evidence liability window");
        assert!(
            matches!(&error, Error::InvariantViolation(message) if message.contains("remains slashable")),
            "unexpected early-finalization error: {error:?}"
        );
        drop(early_tx);
        drop(early_state_block);
        let finalize_block = new_block_with_height_and_time(5, release_at_ms);
        let mut finalize_state_block = state.block(finalize_block.as_ref().header());
        let mut finalize_tx = finalize_state_block.transaction();
        finalize_tx.nexus.staking.stake_asset_id = asset_def_id.to_string();
        finalize_tx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        finalize_tx.nexus.staking.slash_sink_account_id = escrow.to_string();
        FinalizePublicLaneUnbond {
            lane_id: LaneId::new(7),
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("req"),
        }
        .execute(&delegator, &mut finalize_tx)
        .expect("unbond becomes releasable after its complete evidence liability window");
        finalize_tx.apply();
        finalize_state_block
            .commit_world_overlay_for_testing()
            .unwrap();
        let view = state.view();
        let record = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(7), validator.clone()))
            .unwrap();
        assert_eq!(record.total_stake, Quantity::from(650_u64));
        let share = view
            .world
            .public_lane_stake_shares()
            .get(&(LaneId::new(7), validator.clone(), delegator.clone()))
            .unwrap();
        assert_eq!(share.bonded, Quantity::from(150_u64));
        assert!(share.pending_unbonds.is_empty());
        let escrow_asset = AssetId::new(asset_def_id.clone(), escrow.clone());
        let validator_stake = AssetId::new(asset_def_id.clone(), validator.clone());
        let delegator_stake = AssetId::new(asset_def_id, delegator.clone());
        let escrow_balance = view
            .world
            .assets()
            .get(&escrow_asset)
            .expect("escrow balance after unbond");
        assert_eq!(escrow_balance.as_ref(), &Quantity::from(650_u64));
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
        assert_eq!(validator_balance.as_ref(), &Quantity::from(9_500_u64));
        assert_eq!(delegator_balance.as_ref(), &Quantity::from(9_850_u64));
    }
    #[test]
    fn boundary_block_unbond_covers_the_successor_frozen_epoch() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let block = new_block_with_height_and_time(3, 0);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        set_test_npos_penalty_windows(&mut stx, 1, 1);
        let lane_id = LaneId::new(174);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(500_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register boundary-block validator");
        let request_id = Hash::new("boundary-frozen-epoch-unbond");
        SchedulePublicLaneUnbond {
            lane_id,
            validator: validator.clone(),
            staker: validator.clone(),
            request_id,
            amount: Quantity::from(100_u64),
            release_at_ms: stx.block_unix_timestamp_ms(),
        }
        .execute(&validator, &mut stx)
        .expect("schedule boundary-block unbond");

        let pending = &stx
            .world
            .public_lane_stake_shares
            .get(&stake_key(lane_id, &validator, &validator))
            .expect("boundary-block stake share")
            .pending_unbonds[&request_id];
        assert_eq!(pending.slashable_through_height, 6);
        assert_eq!(pending.liability_release_height, 8);
        assert!(pending_unbond_is_slashable_at(pending, Some(6)));
        assert!(!pending_unbond_is_slashable_at(pending, Some(7)));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn bond_schedule_and_finalize_require_staker_authority() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        set_test_npos_penalty_windows(&mut stx, 1, 1);
        let lane_id = LaneId::new(83);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(500_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        let err = BondPublicLaneStake {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Quantity::from(250_u64),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("non-staker authority must not bond delegator funds");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg)
                if msg.contains("bond_public_lane_stake rejected: authority must match staker account")
        ));
        assert!(
            stx.world
                .public_lane_stake_shares
                .get(&(lane_id, validator.clone(), delegator.clone()))
                .is_none(),
            "rejected bond must not create delegator stake"
        );
        BondPublicLaneStake {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Quantity::from(250_u64),
            metadata: Metadata::default(),
        }
        .execute(&delegator, &mut stx)
        .expect("delegator can bond own funds");
        let release_at_ms = stx.block_unix_timestamp_ms();
        let err = SchedulePublicLaneUnbond {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("authority-unbond"),
            amount: Quantity::from(100_u64),
            release_at_ms,
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("non-staker authority must not schedule delegator unbond");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg)
                if msg.contains("schedule_public_lane_unbond rejected: authority must match staker account")
        ));
        let share = stx
            .world
            .public_lane_stake_shares
            .get(&(lane_id, validator.clone(), delegator.clone()))
            .expect("delegator share remains present");
        assert_eq!(share.bonded, Quantity::from(250_u64));
        assert!(share.pending_unbonds.is_empty());
        SchedulePublicLaneUnbond {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("authority-unbond"),
            amount: Quantity::from(100_u64),
            release_at_ms,
        }
        .execute(&delegator, &mut stx)
        .expect("delegator can schedule own unbond");
        let err = FinalizePublicLaneUnbond {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("authority-unbond"),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("non-staker authority must not finalize delegator unbond");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg)
                if msg.contains("finalize_public_lane_unbond rejected: authority must match staker account")
        ));
        let share = stx
            .world
            .public_lane_stake_shares
            .get(&(lane_id, validator.clone(), delegator.clone()))
            .expect("delegator share remains present after rejected finalize");
        assert!(
            share
                .pending_unbonds
                .contains_key(&Hash::new("authority-unbond")),
            "rejected finalize must leave pending unbond intact"
        );
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
        let finalize_block = new_block_with_height_and_time(5, release_at_ms);
        let mut finalize_state_block = state.block(finalize_block.as_ref().header());
        let mut finalize_tx = finalize_state_block.transaction();
        finalize_tx.nexus.staking.stake_asset_id = asset_def_id.to_string();
        finalize_tx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        finalize_tx.nexus.staking.slash_sink_account_id = escrow.to_string();
        FinalizePublicLaneUnbond {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("authority-unbond"),
        }
        .execute(&delegator, &mut finalize_tx)
        .expect("delegator can finalize own unbond after the liability window");
        let share = finalize_tx
            .world
            .public_lane_stake_shares
            .get(&(lane_id, validator.clone(), delegator.clone()))
            .expect("delegator share remains after finalize");
        assert_eq!(share.bonded, Quantity::from(150_u64));
        assert!(share.pending_unbonds.is_empty());
        let escrow_balance = finalize_tx
            .world
            .assets
            .get(&AssetId::new(asset_def_id.clone(), escrow))
            .expect("escrow balance after authorized finalize");
        assert_eq!(escrow_balance.as_ref(), &Quantity::from(650_u64));
        let delegator_balance = finalize_tx
            .world
            .assets
            .get(&AssetId::new(asset_def_id, delegator))
            .expect("delegator balance after authorized finalize");
        assert_eq!(delegator_balance.as_ref(), &Quantity::from(9_850_u64));
        finalize_tx.apply();
        finalize_state_block
            .commit_world_overlay_for_testing()
            .unwrap();
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx);
        assert!(res.is_err(), "expected register to fail when peer missing");
    }
    #[test]
    fn register_rejects_non_owner_same_dataspace_lane_before_stake_moves() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();

        let (validator, _, escrow, asset_definition) = prepare_accounts(&mut stx);
        let owner_lane = LaneId::SINGLE;
        let sibling_lane = LaneId::new(1);
        set_transaction_lane_catalog(
            &mut stx,
            LaneCatalog::new(
                nonzero!(2_u32),
                vec![
                    LaneConfig::default(),
                    LaneConfig {
                        id: sibling_lane,
                        alias: "shared-staking-sibling".to_owned(),
                        dataspace_id: DataSpaceId::UNIVERSAL,
                        visibility: LaneVisibility::Public,
                        ..LaneConfig::default()
                    },
                ],
            )
            .expect("two public lanes may share the universal dataspace"),
        );
        assert_eq!(stx.staking_authority_lane(sibling_lane), Some(owner_lane));

        let validator_asset = AssetId::new(asset_definition.clone(), validator.clone());
        let escrow_asset = AssetId::new(asset_definition, escrow);
        let validator_balance_before = stx
            .world
            .assets
            .get(&validator_asset)
            .expect("validator stake balance")
            .as_ref()
            .clone();
        let escrow_balance_before = stx
            .world
            .assets
            .get(&escrow_asset)
            .map(|asset| asset.as_ref().clone());

        let err = RegisterPublicLaneValidator {
            lane_id: sibling_lane,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect_err("a non-owner sibling must not create a second stake projection");

        assert!(matches!(
            err,
            Error::InvalidParameter(InvalidParameterError::SmartContract(message))
                if message.contains("staking owner is lane")
        ));
        assert_eq!(
            stx.world
                .assets
                .get(&validator_asset)
                .expect("validator stake balance after rejection")
                .as_ref(),
            &validator_balance_before
        );
        assert_eq!(
            stx.world
                .assets
                .get(&escrow_asset)
                .map(|asset| asset.as_ref().clone()),
            escrow_balance_before
        );
        assert!(
            stx.world
                .public_lane_validators
                .get(&(sibling_lane, validator.clone()))
                .is_none()
        );
        assert!(
            stx.world
                .public_lane_stake_shares
                .get(&(sibling_lane, validator.clone(), validator))
                .is_none()
        );
    }

    #[test]
    fn activate_transitions_pending_to_active() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        // Register matching peer so validator admission passes.
        let peer_id = crate::PeerId::from(validator.expect_single_signatory().clone());
        let _ = stx.world.peers.push(peer_id);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(2),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        ActivatePublicLaneValidator {
            lane_id: LaneId::new(2),
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
        let view = state.view();
        let record = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(2), validator.clone()))
            .unwrap();
        assert!(matches!(record.status, PublicLaneValidatorStatus::Active));
    }
    #[test]
    fn exit_preserves_custody_and_blocks_reregister() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let peer_id = crate::PeerId::from(validator.expect_single_signatory().clone());
        let _ = stx.world.peers.push(peer_id);
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(3),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        let now_ms = stx.block_unix_timestamp_ms();
        ExitPublicLaneValidator {
            lane_id: LaneId::new(3),
            validator: validator.clone(),
            release_at_ms: now_ms,
        }
        .execute(&validator, &mut stx)
        .unwrap();
        let err = RegisterPublicLaneValidator {
            lane_id: LaneId::new(3),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(500_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect_err("exit alone must not discard slashable custody");
        assert!(matches!(
            err,
            Error::InvariantViolation(message)
                if message.contains("retains slashable stake custody")
        ));
        let record = stx
            .world
            .public_lane_validators
            .get(&(LaneId::new(3), validator.clone()))
            .expect("exited validator record remains");
        assert!(matches!(record.status, PublicLaneValidatorStatus::Exited));
        let share = stx
            .world
            .public_lane_stake_shares
            .get(&(LaneId::new(3), validator.clone(), validator))
            .expect("bonded custody remains");
        assert_eq!(share.bonded, Quantity::from(1_000_u64));
    }
    #[test]
    fn exit_rejects_non_validator_authority() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(84);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        let err = ExitPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            release_at_ms: stx.block_unix_timestamp_ms(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("non-validator authority must not exit another validator");
        assert!(matches!(
            err,
            Error::InvariantViolation(msg)
                if msg.contains("exit_public_lane_validator rejected: authority must match validator account")
        ));
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("validator record remains present");
        assert!(
            matches!(
                record.status,
                PublicLaneValidatorStatus::PendingActivation(_)
            ),
            "rejected exit must not mutate validator lifecycle state"
        );
    }
    #[test]
    fn exit_public_lane_validator_rejects_mismatched_public_lane_validator_row() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(163);
        insert_validator_record_for_key(
            &mut stx,
            lane_id,
            LaneId::new(164),
            &validator,
            PublicLaneValidatorStatus::Active,
            Quantity::from(1_000_u64),
        );
        let err = ExitPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            release_at_ms: stx.block_unix_timestamp_ms(),
        }
        .execute(&validator, &mut stx)
        .expect_err("mismatched validator row must reject exit");
        assert!(
            matches!(err, Error::InvariantViolation(msg) if msg.contains("does not match its storage key"))
        );
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("mismatched record remains present");
        assert!(matches!(record.status, PublicLaneValidatorStatus::Active));
    }
    #[test]
    fn slashed_validator_exit_does_not_bypass_retained_custody() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let lane_id = LaneId::new(34);
        let (validator, _, _, _asset_def_id) = prepare_accounts(&mut stx);
        let peer_id = crate::PeerId::from(validator.expect_single_signatory().clone());
        let _ = stx.world.peers.push(peer_id);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        SlashPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            offence_height: stx.block_height(),
            slash_id: Hash::new("slash-reason"),
            amount: Quantity::from(100_u64),
            reason_code: "evidence".to_string(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("slash succeeds");
        let err = RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(250_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
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
        .execute(&validator, &mut stx)
        .expect("exit slashed validator");
        let err = RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(250_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect_err("exit must not erase retained slashable custody");
        assert!(matches!(
            err,
            Error::InvariantViolation(message)
                if message.contains("retains slashable stake custody")
        ));
        let record = stx
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()))
            .expect("validator record");
        assert!(
            matches!(record.status, PublicLaneValidatorStatus::Exited),
            "rejected re-registration must preserve the exited record"
        );
        let share = stx
            .world
            .public_lane_stake_shares
            .get(&(lane_id, validator.clone(), validator))
            .expect("slashed bonded custody remains");
        assert_eq!(share.bonded, Quantity::from(900_u64));
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn slashed_exit_with_release_timer_retains_capacity_while_custody_exists() {
        let state = setup_state();
        let block = new_block_with_height_and_time(1, 1_000);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_validators = nonzero!(1u32);
        let lane_id = LaneId::new(35);
        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        SlashPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            offence_height: stx.block_height(),
            slash_id: Hash::new("slash-release-gate"),
            amount: Quantity::from(200_u64),
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
        .execute(&validator, &mut stx)
        .unwrap();
        stx.apply();
        state_block.commit_empty_block_for_testing().unwrap();
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
        Mint::asset_quantity(
            10_000u32,
            AssetId::new(asset_def_id.clone(), replacement.clone()),
        )
        .execute(&ALICE_ID, &mut setup_tx)
        .unwrap();
        setup_tx.apply();
        setup_state_block.commit_empty_block_for_testing().unwrap();
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
                peer_id: validator_peer_id(&replacement),
                validator: replacement.clone(),
                stake_account: replacement.clone(),
                initial_stake: Quantity::from(1_000_u64),
                metadata: Metadata::default(),
            }
            .execute(&replacement, &mut prerelease_tx)
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
        prerelease_state_block
            .commit_empty_block_for_testing()
            .unwrap();
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
        let error = RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&replacement),
            validator: replacement.clone(),
            stake_account: replacement.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&replacement, &mut post_tx)
        .expect_err("release time must not free capacity while slashable custody remains");
        assert!(matches!(
            error,
            Error::InvariantViolation(message)
                if message.contains("maximum validator capacity")
        ));
        let previous = post_tx
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()))
            .expect("slashed validator record still present");
        assert!(
            matches!(previous.status, PublicLaneValidatorStatus::Exited),
            "release should finalize exit before replacement registers"
        );
        assert!(
            post_tx.world.public_lane_stake_shares.iter().any(
                |((lane, retained_validator, _), _)| {
                    *lane == lane_id && retained_validator == &validator
                }
            ),
            "slashed exit must retain custody until its stake is explicitly unbonded"
        );
    }
    #[test]
    fn unregister_peer_rejects_active_validator_tenure() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let peer_id = crate::PeerId::from(validator.expect_single_signatory().clone());
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(31),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        ActivatePublicLaneValidator {
            lane_id: LaneId::new(31),
            validator: validator.clone(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        stx.commit_topology
            .get_mut()
            .retain(|peer| peer != &peer_id);
        use iroha_data_model::isi::register::Unregister;
        let error = Unregister::<Peer>::peer(peer_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect_err("active validator tenure must reject peer unregistration");
        assert!(
            error
                .to_string()
                .contains("wait for its deactivation height"),
            "unexpected unregistration rejection: {error}"
        );
        assert!(stx.world.peers().iter().any(|peer| peer == &peer_id));
        let record = stx
            .world
            .public_lane_validators()
            .get(&(LaneId::new(31), validator.clone()))
            .expect("validator record after rejected peer removal");
        assert!(matches!(record.status, PublicLaneValidatorStatus::Active));
    }
    #[test]
    fn unregister_peer_preserves_stake_shares() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let lane_id = LaneId::new(32);
        let (validator, delegator, _, _) = prepare_accounts(&mut stx);
        let peer_id = crate::PeerId::from(validator.expect_single_signatory().clone());
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
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
            amount: Quantity::from(250_u64),
            metadata: Metadata::default(),
        }
        .execute(&delegator, &mut stx)
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
        stx.commit_topology
            .get_mut()
            .retain(|peer| peer != &peer_id);
        use iroha_data_model::isi::register::Unregister;
        let error = Unregister::<Peer>::peer(peer_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect_err("retained validator tenure must reject peer unregistration");
        assert!(
            error
                .to_string()
                .contains("wait for its deactivation height"),
            "unexpected unregistration rejection: {error}"
        );
        assert!(stx.world.peers().iter().any(|peer| peer == &peer_id));
        let remaining_shares = stx
            .world
            .public_lane_stake_shares()
            .iter()
            .filter(|((lane, val, _), _)| *lane == lane_id && val == &validator)
            .count();
        assert_eq!(
            remaining_shares, pre_exit_shares,
            "rejected peer removal must preserve every bonded stake share"
        );
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator.clone()))
            .expect("validator record after rejected removal");
        assert!(matches!(record.status, PublicLaneValidatorStatus::Active));
    }
    #[test]
    fn bond_rejects_without_funds() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, delegator, _, asset_def_id) = prepare_accounts(&mut stx);
        let delegator_asset = AssetId::new(asset_def_id.clone(), delegator.clone());
        let delegator_balance = Quantity::from(10_000_u64);
        crate::smartcontracts::isi::asset::isi::debit_numeric_asset_balance_for_test(
            &mut stx.world,
            &stx.network_id,
            &delegator_asset,
            &delegator_balance,
        )
        .expect("drain delegator funds");
        RegisterPublicLaneValidator {
            lane_id: LaneId::new(12),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(500_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        let res = BondPublicLaneStake {
            lane_id: LaneId::new(12),
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Quantity::from(500_u64),
            metadata: Metadata::default(),
        }
        .execute(&delegator, &mut stx);
        assert!(
            matches!(res, Err(Error::Math(MathError::NotEnoughQuantity))),
            "expected bond to fail when staker lacks funds"
        );
    }
    #[test]
    fn bond_rejects_terminal_validator_statuses_without_moving_funds() {
        let statuses = [
            PublicLaneValidatorStatus::Exiting(0),
            PublicLaneValidatorStatus::Exited,
            PublicLaneValidatorStatus::Slashed(Hash::new("terminal-bond")),
        ];
        for (index, status) in statuses.into_iter().enumerate() {
            let state = setup_state();
            let block = new_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            let (validator, delegator, escrow, asset_definition) = prepare_accounts(&mut stx);
            let lane_id = LaneId::new(210 + u32::try_from(index).expect("small status index"));
            RegisterPublicLaneValidator {
                lane_id,
                peer_id: validator_peer_id(&validator),
                validator: validator.clone(),
                stake_account: validator.clone(),
                initial_stake: Quantity::from(500_u64),
                metadata: Metadata::default(),
            }
            .execute(&validator, &mut stx)
            .expect("register terminal-status fixture");
            stx.world
                .public_lane_validators
                .get_mut(&(lane_id, validator.clone()))
                .expect("validator fixture")
                .status = status;
            let delegator_asset = AssetId::new(asset_definition.clone(), delegator.clone());
            let escrow_asset = AssetId::new(asset_definition, escrow);
            let delegator_before = stx
                .world
                .assets
                .get(&delegator_asset)
                .expect("delegator balance")
                .as_ref()
                .clone();
            let escrow_before = stx
                .world
                .assets
                .get(&escrow_asset)
                .expect("escrow balance")
                .as_ref()
                .clone();

            let error = BondPublicLaneStake {
                lane_id,
                validator: validator.clone(),
                staker: delegator.clone(),
                amount: Quantity::from(100_u64),
                metadata: Metadata::default(),
            }
            .execute(&delegator, &mut stx)
            .expect_err("terminal validator status must reject new stake");

            assert!(matches!(
                error,
                Error::InvariantViolation(message)
                    if message.contains("does not accept new stake")
            ));
            assert_eq!(
                stx.world
                    .public_lane_validators
                    .get(&(lane_id, validator.clone()))
                    .expect("validator remains present")
                    .total_stake,
                Quantity::from(500_u64)
            );
            assert!(
                stx.world
                    .public_lane_stake_shares
                    .get(&(lane_id, validator, delegator))
                    .is_none(),
                "rejected bond must not create a delegator share"
            );
            assert_eq!(
                stx.world
                    .assets
                    .get(&delegator_asset)
                    .expect("delegator balance remains")
                    .as_ref(),
                &delegator_before
            );
            assert_eq!(
                stx.world
                    .assets
                    .get(&escrow_asset)
                    .expect("escrow balance remains")
                    .as_ref(),
                &escrow_before
            );
        }
    }
    #[test]
    fn bond_public_lane_stake_rejects_mismatched_public_lane_validator_row() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, delegator, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(165);
        let delegator_asset = AssetId::new(asset_def_id.clone(), delegator.clone());
        let escrow_asset = AssetId::new(asset_def_id, escrow);
        let delegator_before = stx
            .world
            .assets
            .get(&delegator_asset)
            .expect("delegator stake asset")
            .as_ref()
            .clone();
        let escrow_before = stx
            .world
            .assets
            .get(&escrow_asset)
            .map_or_else(Quantity::zero, |asset| asset.as_ref().clone());
        insert_validator_record_for_key(
            &mut stx,
            lane_id,
            LaneId::new(166),
            &validator,
            PublicLaneValidatorStatus::Active,
            Quantity::from(1_000_u64),
        );
        let err = BondPublicLaneStake {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Quantity::from(100_u64),
            metadata: Metadata::default(),
        }
        .execute(&delegator, &mut stx)
        .expect_err("mismatched validator row must reject bonding");
        assert!(
            matches!(err, Error::InvariantViolation(msg) if msg.contains("does not match its storage key"))
        );
        assert_eq!(
            stx.world
                .assets
                .get(&delegator_asset)
                .expect("delegator stake asset after rejection")
                .as_ref(),
            &delegator_before
        );
        assert_eq!(
            stx.world
                .assets
                .get(&escrow_asset)
                .map_or_else(Quantity::zero, |asset| asset.as_ref().clone()),
            escrow_before
        );
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("mismatched record remains present");
        assert_eq!(record.total_stake, Quantity::from(1_000_u64));
    }
    #[test]
    fn register_rejects_below_min_stake() {
        let mut state = setup_state();
        state.nexus.get_mut().staking.min_validator_stake = 2_000_u64.into();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, _, _) = prepare_accounts(&mut stx);
        let res = RegisterPublicLaneValidator {
            lane_id: LaneId::new(3),
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx);
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx);
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        let release_at_ms = stx.block_unix_timestamp_ms();
        let res = SchedulePublicLaneUnbond {
            lane_id: LaneId::new(4),
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("req-delay"),
            amount: Quantity::from(100_u64),
            release_at_ms,
        }
        .execute(&delegator, &mut stx);
        assert!(res.is_err(), "expected unbond delay enforcement");
    }
    #[test]
    fn bond_rejects_new_share_at_validator_cap_without_moving_funds() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_stake_shares_per_validator = nonzero!(1_u32);
        let (validator, delegator, _escrow, asset_definition) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(171);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("validator self share occupies the configured capacity");
        let validator_before = stx
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()))
            .expect("validator record")
            .clone();
        let delegator_asset = AssetId::new(asset_definition, delegator.clone());
        let balance_before = stx
            .world
            .assets
            .get(&delegator_asset)
            .expect("delegator stake balance")
            .as_ref()
            .clone();

        let err = BondPublicLaneStake {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Quantity::from(100_u64),
            metadata: Metadata::default(),
        }
        .execute(&delegator, &mut stx)
        .expect_err("a second share must exceed the configured cap");
        assert!(matches!(
            err,
            Error::InvariantViolation(message)
                if message.contains("maximum stake-share capacity")
        ));
        assert_eq!(
            stx.world
                .public_lane_validators
                .get(&(lane_id, validator.clone())),
            Some(&validator_before),
            "rejected admission must not mutate validator aggregates"
        );
        assert_eq!(
            stx.world
                .assets
                .get(&delegator_asset)
                .expect("delegator stake balance")
                .as_ref(),
            &balance_before,
            "rejected admission must not move stake into escrow"
        );
        assert!(
            stx.world
                .public_lane_stake_shares
                .get(&(lane_id, validator, delegator))
                .is_none(),
            "rejected admission must not create a share"
        );
    }
    #[test]
    fn schedule_unbond_rejects_request_at_share_cap_atomically() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus.staking.max_pending_unbonds_per_share = nonzero!(2_u32);
        set_test_npos_penalty_windows(&mut stx, 1, 1);
        let (validator, _delegator, _escrow, _asset_definition) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(172);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        let release_at_ms = stx.block_unix_timestamp_ms();
        for request in [Hash::new("bounded-unbond-1"), Hash::new("bounded-unbond-2")] {
            SchedulePublicLaneUnbond {
                lane_id,
                validator: validator.clone(),
                staker: validator.clone(),
                request_id: request,
                amount: Quantity::from(100_u64),
                release_at_ms,
            }
            .execute(&validator, &mut stx)
            .expect("request up to the configured cap must succeed");
        }
        let validator_before = stx
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()))
            .expect("validator record")
            .clone();
        let share_key = (lane_id, validator.clone(), validator.clone());
        let share_before = stx
            .world
            .public_lane_stake_shares
            .get(&share_key)
            .expect("validator self share")
            .clone();

        let err = SchedulePublicLaneUnbond {
            lane_id,
            validator: validator.clone(),
            staker: validator.clone(),
            request_id: Hash::new("bounded-unbond-3"),
            amount: Quantity::from(100_u64),
            release_at_ms,
        }
        .execute(&validator, &mut stx)
        .expect_err("request beyond the configured cap must fail");
        assert!(matches!(
            err,
            Error::InvariantViolation(message)
                if message.contains("maximum pending-unbond capacity")
        ));
        assert_eq!(
            stx.world
                .public_lane_validators
                .get(&(lane_id, validator.clone())),
            Some(&validator_before),
            "rejected request must not mutate validator aggregates"
        );
        assert_eq!(
            stx.world.public_lane_stake_shares.get(&share_key),
            Some(&share_before),
            "rejected request must not mutate the share or pending queue"
        );
    }
    #[test]
    fn schedule_public_lane_unbond_rejects_mismatched_public_lane_validator_row() {
        let mut state = setup_state();
        state.nexus.get_mut().staking.unbonding_delay = Duration::from_millis(0);
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, delegator, _, _) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(167);
        insert_validator_record_for_key(
            &mut stx,
            lane_id,
            LaneId::new(168),
            &validator,
            PublicLaneValidatorStatus::Active,
            Quantity::from(1_000_u64),
        );
        let err = SchedulePublicLaneUnbond {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            request_id: Hash::new("req-mismatched"),
            amount: Quantity::from(100_u64),
            release_at_ms: stx.block_unix_timestamp_ms(),
        }
        .execute(&delegator, &mut stx)
        .expect_err("mismatched validator row must reject unbond scheduling");
        assert!(
            matches!(err, Error::InvariantViolation(msg) if msg.contains("does not match its storage key"))
        );
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("mismatched record remains present");
        assert_eq!(record.total_stake, Quantity::from(1_000_u64));
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        BondPublicLaneStake {
            lane_id: LaneId::new(13),
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Quantity::from(500_u64),
            metadata: Metadata::default(),
        }
        .execute(&delegator, &mut stx)
        .unwrap();
        SlashPublicLaneValidator {
            lane_id: LaneId::new(13),
            validator: validator.clone(),
            offence_height: stx.block_height(),
            slash_id: Hash::new("slash-escrow"),
            amount: Quantity::from(400_u64),
            reason_code: "evidence".to_string(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
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
        assert_eq!(escrow_balance.as_ref(), &Quantity::from(1_100_u64));
        assert_eq!(sink_balance.as_ref(), &Quantity::from(9_900_u64));
        let validator_record = view
            .world
            .public_lane_validators()
            .get(&(LaneId::new(13), validator.clone()))
            .expect("validator after slash");
        assert_eq!(validator_record.total_stake, Quantity::from(1_100_u64));
        assert_eq!(validator_record.self_stake, Quantity::from(600_u64));
        let self_share = view
            .world
            .public_lane_stake_shares()
            .get(&(LaneId::new(13), validator.clone(), validator.clone()))
            .expect("self stake share after slash");
        assert_eq!(self_share.bonded, Quantity::from(600_u64));
        let delegator_share = view
            .world
            .public_lane_stake_shares()
            .get(&(LaneId::new(13), validator, delegator))
            .expect("delegator stake share after slash");
        assert_eq!(delegator_share.bonded, Quantity::from(500_u64));
    }
    #[test]
    fn consensus_slash_after_unbond_schedule_consumes_offender_pending_custody() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, delegator, _escrow, _asset_def_id) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(173);
        stx.nexus.staking.slash_sink_account_id = delegator.to_string();
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register slash-order fixture validator");
        BondPublicLaneStake {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Quantity::from(500_u64),
            metadata: Metadata::default(),
        }
        .execute(&delegator, &mut stx)
        .expect("bond nominator stake");
        let request_id = Hash::new("self-pending-before-slash");
        SchedulePublicLaneUnbond {
            lane_id,
            validator: validator.clone(),
            staker: validator.clone(),
            request_id,
            amount: Quantity::from(900_u64),
            release_at_ms: stx.block_unix_timestamp_ms(),
        }
        .execute(&validator, &mut stx)
        .expect("move offender self stake into slashable pending custody");

        let mut share_keys = vec![
            stake_key(lane_id, &validator, &validator),
            stake_key(lane_id, &validator, &delegator),
        ];
        share_keys.sort();
        apply_indexed_consensus_slash_to_validator(
            &mut stx,
            lane_id,
            &validator,
            Hash::new("self-pending-priority"),
            &Quantity::from(400_u64),
            0,
            2,
            &share_keys,
        )
        .expect("an H+1 offence remains slashable from frozen pending custody");

        let record = stx
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()))
            .expect("slashed validator record");
        assert_eq!(record.total_stake, Quantity::from(500_u64));
        assert_eq!(record.self_stake, Quantity::zero());
        let self_share = stx
            .world
            .public_lane_stake_shares
            .get(&(lane_id, validator.clone(), validator.clone()))
            .expect("offender pending share remains");
        assert_eq!(self_share.bonded, Quantity::zero());
        assert_eq!(
            self_share.pending_unbonds[&request_id].amount,
            Quantity::from(600_u64)
        );
        let nominator_share = stx
            .world
            .public_lane_stake_shares
            .get(&(lane_id, validator, delegator))
            .expect("nominator share remains");
        assert_eq!(nominator_share.bonded, Quantity::from(500_u64));
        assert!(nominator_share.pending_unbonds.is_empty());
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn manual_slash_enforces_offence_tenure_and_pending_custody_cutoff() {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let block = new_block_with_height_and_time(1, 0);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _delegator, escrow, asset_definition) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(175);
        set_test_npos_penalty_windows(&mut stx, 1, 1);
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register manual-slash fixture validator");
        let request_id = Hash::new("manual-slash-pending-cutoff");
        SchedulePublicLaneUnbond {
            lane_id,
            validator: validator.clone(),
            staker: validator.clone(),
            request_id,
            amount: Quantity::from(900_u64),
            release_at_ms: stx.block_unix_timestamp_ms(),
        }
        .execute(&validator, &mut stx)
        .expect("schedule pending custody before the manual slash");
        let pending = &stx
            .world
            .public_lane_stake_shares
            .get(&stake_key(lane_id, &validator, &validator))
            .expect("manual-slash fixture share")
            .pending_unbonds[&request_id];
        assert_eq!(pending.slashable_through_height, 3);
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();

        let slash_block = new_block_with_height_and_time(4, 1);
        let mut slash_state_block = state.block(slash_block.as_ref().header());
        let mut slash_tx = slash_state_block.transaction();
        slash_tx.nexus.staking.stake_asset_id = asset_definition.to_string();
        slash_tx.nexus.staking.stake_escrow_account_id = escrow.to_string();
        slash_tx.nexus.staking.slash_sink_account_id = escrow.to_string();
        slash_tx.nexus.staking.max_slash_bps = 10_000;
        finalize_validator_lifecycle(&mut slash_tx).expect("activate fixture validator");

        for (offence_height, expected_message) in [
            (
                0,
                "slash offence height must identify a non-zero height no later than the current block",
            ),
            (
                5,
                "slash offence height must identify a non-zero height no later than the current block",
            ),
        ] {
            let error = SlashPublicLaneValidator {
                lane_id,
                validator: validator.clone(),
                offence_height,
                slash_id: Hash::new(offence_height.to_be_bytes()),
                amount: Quantity::from(1_u64),
                reason_code: "invalid offence height".to_owned(),
                metadata: Metadata::default(),
            }
            .execute(&ALICE_ID, &mut slash_tx)
            .expect_err("zero and future offence heights must fail closed");
            assert!(
                matches!(error, Error::InvariantViolation(message) if message.contains(expected_message))
            );
        }

        slash_tx
            .world
            .public_lane_validators
            .get_mut(&(lane_id, validator.clone()))
            .expect("manual-slash fixture validator")
            .deactivation_height = Some(4);
        let error = SlashPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            offence_height: 4,
            slash_id: Hash::new("manual-slash-outside-tenure"),
            amount: Quantity::from(1_u64),
            reason_code: "outside retained tenure".to_owned(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut slash_tx)
        .expect_err("the exclusive deactivation boundary must reject the offence height");
        assert!(
            matches!(error, Error::InvariantViolation(message) if message.contains("outside the validator tenure"))
        );
        slash_tx
            .world
            .public_lane_validators
            .get_mut(&(lane_id, validator.clone()))
            .expect("manual-slash fixture validator")
            .deactivation_height = None;

        let error = SlashPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            offence_height: 4,
            slash_id: Hash::new("manual-slash-after-pending-cutoff"),
            amount: Quantity::from(101_u64),
            reason_code: "after pending custody cutoff".to_owned(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut slash_tx)
        .expect_err("post-cutoff pending custody must not expand slashable exposure");
        assert!(
            matches!(error, Error::InvariantViolation(message) if message.contains("exceeds configured maximum ratio"))
        );
        let share = slash_tx
            .world
            .public_lane_stake_shares
            .get(&stake_key(lane_id, &validator, &validator))
            .expect("rejected slash preserves the fixture share");
        assert_eq!(share.bonded, Quantity::from(100_u64));
        assert_eq!(
            share.pending_unbonds[&request_id].amount,
            Quantity::from(900_u64)
        );

        SlashPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            offence_height: 4,
            slash_id: Hash::new("manual-slash-bonded-only"),
            amount: Quantity::from(100_u64),
            reason_code: "bonded custody only".to_owned(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut slash_tx)
        .expect("the bonded portion remains slashable for an in-tenure offence");
        let share = slash_tx
            .world
            .public_lane_stake_shares
            .get(&stake_key(lane_id, &validator, &validator))
            .expect("pending custody remains after the bonded slash");
        assert_eq!(share.bonded, Quantity::zero());
        assert_eq!(
            share.pending_unbonds[&request_id].amount,
            Quantity::from(900_u64)
        );
    }
    #[test]
    fn validation_only_consensus_slash_rolls_back_every_world_write() {
        let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let lane_id = LaneId::new(13);
        let (validator, escrow_asset, sink_asset, staking_config) = {
            let mut transaction = state_block.transaction();
            let (validator, delegator, escrow, asset_definition_id) =
                prepare_accounts(&mut transaction);
            transaction.nexus.staking.slash_sink_account_id = delegator.to_string();
            RegisterPublicLaneValidator {
                lane_id,
                peer_id: validator_peer_id(&validator),
                validator: validator.clone(),
                stake_account: validator.clone(),
                initial_stake: Quantity::from(1_000_u64),
                metadata: Metadata::default(),
            }
            .execute(&validator, &mut transaction)
            .expect("register validation-only slash target");
            let escrow_asset = AssetId::new(asset_definition_id.clone(), escrow.clone());
            let sink_asset = AssetId::new(asset_definition_id, delegator);
            let staking_config = transaction.nexus.staking.clone();
            transaction.apply();
            (validator, escrow_asset, sink_asset, staking_config)
        };
        // `prepare_accounts` configures the transaction snapshot. Preserve that
        // exact custody policy for the independent consensus-validation scope.
        state_block.nexus.staking = staking_config;
        state_block.world.take_external_events();
        let status_before = crate::sumeragi::status::lane_scoped_status_fingerprint_for_tests();
        let record_before = state_block
            .world
            .public_lane_validators
            .get(&(lane_id, validator.clone()))
            .cloned()
            .expect("registered validator remains in the block overlay");
        let escrow_before = state_block
            .world
            .assets
            .get(&escrow_asset)
            .map_or_else(Quantity::zero, |asset| asset.as_ref().clone());
        let sink_before = state_block
            .world
            .assets
            .get(&sink_asset)
            .map_or_else(Quantity::zero, |asset| asset.as_ref().clone());
        let share_key = (lane_id, validator.clone(), validator.clone());
        let share_before = state_block
            .world
            .public_lane_stake_shares
            .get(&share_key)
            .cloned()
            .expect("registered validator retains a self-stake share");

        {
            let mut validation = state_block.consensus_effects_transaction();
            let now_ms = validation.block_unix_timestamp_ms();
            apply_slash_to_validator_without_observability(
                &mut validation,
                lane_id,
                &validator,
                Hash::new("validation-only-consensus-slash"),
                &Quantity::from(100_u64),
                now_ms,
            )
            .expect("a valid slash must be applicable in the disposable overlay");
            let validation_record = validation
                .world
                .public_lane_validators
                .get(&(lane_id, validator.clone()))
                .expect("validation overlay retains the slashed validator");
            assert_eq!(validation_record.total_stake, Quantity::from(900_u64));
            assert_eq!(
                validation
                    .world
                    .public_lane_stake_shares
                    .get(&share_key)
                    .expect("validation overlay retains the reduced self-stake share")
                    .bonded,
                Quantity::from(900_u64)
            );
            assert_eq!(
                validation
                    .world
                    .assets
                    .get(&escrow_asset)
                    .expect("validation overlay retains escrow")
                    .as_ref(),
                &Quantity::from(900_u64)
            );
            assert_eq!(
                validation
                    .world
                    .assets
                    .get(&sink_asset)
                    .expect("validation overlay credits the slash sink")
                    .as_ref(),
                &Quantity::from(10_100_u64)
            );
        }

        assert_eq!(
            state_block
                .world
                .public_lane_validators
                .get(&(lane_id, validator))
                .expect("discarded validation leaves the validator intact"),
            &record_before
        );
        assert_eq!(
            state_block
                .world
                .assets
                .get(&escrow_asset)
                .map_or_else(Quantity::zero, |asset| asset.as_ref().clone()),
            escrow_before
        );
        assert_eq!(
            state_block
                .world
                .assets
                .get(&sink_asset)
                .map_or_else(Quantity::zero, |asset| asset.as_ref().clone()),
            sink_before
        );
        assert_eq!(
            state_block
                .world
                .public_lane_stake_shares
                .get(&share_key)
                .expect("discarded validation leaves the self-stake share intact"),
            &share_before
        );
        assert!(
            state_block.world.take_external_events().is_empty(),
            "discarded consensus validation must not publish world events"
        );
        assert_eq!(
            crate::sumeragi::status::lane_scoped_status_fingerprint_for_tests(),
            status_before,
            "discarded consensus validation must not publish process-global status"
        );
        crate::sumeragi::status::reset_nexus_economics_for_tests();
    }
    #[test]
    fn slash_public_lane_validator_rejects_mismatched_public_lane_validator_row() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, escrow, asset_def_id) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(169);
        let escrow_asset = AssetId::new(asset_def_id, escrow);
        let escrow_before = stx
            .world
            .assets
            .get(&escrow_asset)
            .map_or_else(Quantity::zero, |asset| asset.as_ref().clone());
        insert_validator_record_for_key(
            &mut stx,
            lane_id,
            LaneId::new(170),
            &validator,
            PublicLaneValidatorStatus::Active,
            Quantity::from(1_000_u64),
        );
        let err = SlashPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            offence_height: stx.block_height(),
            slash_id: Hash::new("slash-mismatched-row"),
            amount: Quantity::from(100_u64),
            reason_code: "evidence".to_string(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("mismatched validator row must reject slashing");
        assert!(
            matches!(err, Error::InvariantViolation(msg) if msg.contains("does not match its storage key"))
        );
        assert_eq!(
            stx.world
                .assets
                .get(&escrow_asset)
                .map_or_else(Quantity::zero, |asset| asset.as_ref().clone()),
            escrow_before
        );
        let record = stx
            .world
            .public_lane_validators()
            .get(&(lane_id, validator))
            .expect("mismatched record remains present");
        assert!(matches!(record.status, PublicLaneValidatorStatus::Active));
        assert_eq!(record.total_stake, Quantity::from(1_000_u64));
    }
    #[test]
    fn slash_ignores_mismatched_public_lane_stake_share_rows() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, delegator, _escrow, _asset_def_id) = prepare_accounts(&mut stx);
        let lane_id = LaneId::new(171);
        stx.nexus.staking.max_slash_bps = 10_000;
        RegisterPublicLaneValidator {
            lane_id,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .expect("register validator");
        BondPublicLaneStake {
            lane_id,
            validator: validator.clone(),
            staker: delegator.clone(),
            amount: Quantity::from(100_u64),
            metadata: Metadata::default(),
        }
        .execute(&delegator, &mut stx)
        .expect("bond delegator stake");
        let validator_key = (lane_id, validator.clone());
        let share_key = (lane_id, validator.clone(), delegator.clone());
        let validator_before = stx
            .world
            .public_lane_validators
            .get(&validator_key)
            .expect("validator before slash")
            .clone();
        assert_eq!(validator_before.total_stake, Quantity::from(1_100_u64));
        let mut malformed_share = stx
            .world
            .public_lane_stake_shares
            .get(&share_key)
            .expect("delegator share before corruption")
            .clone();
        malformed_share.lane_id = LaneId::new(172);
        stx.world
            .public_lane_stake_shares
            .insert(share_key.clone(), malformed_share);
        let err = SlashPublicLaneValidator {
            lane_id,
            validator: validator.clone(),
            offence_height: stx.block_height(),
            slash_id: Hash::new("slash-mismatched-share-row"),
            amount: Quantity::from(1_100_u64),
            reason_code: "evidence".to_string(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect_err("mismatched stake-share row must not satisfy slash");
        assert!(
            matches!(err, Error::InvariantViolation(msg) if msg.contains("could not be satisfied by stake shares"))
        );
        let validator_after = stx
            .world
            .public_lane_validators
            .get(&validator_key)
            .expect("validator after rejected slash");
        assert_eq!(
            validator_after.total_stake, validator_before.total_stake,
            "rejected slash must not reduce validator total stake"
        );
        assert_eq!(
            validator_after.self_stake, validator_before.self_stake,
            "rejected slash must not reduce validator self stake"
        );
        assert!(
            !matches!(
                validator_after.status,
                PublicLaneValidatorStatus::Slashed(_)
            ),
            "rejected slash must not mark validator as slashed"
        );
        let share_after = stx
            .world
            .public_lane_stake_shares
            .get(&share_key)
            .expect("malformed share remains as stored");
        assert_eq!(share_after.lane_id, LaneId::new(172));
        assert_eq!(share_after.bonded, Quantity::from(100_u64));
    }
    #[test]
    fn claim_rewards_transfers_and_marks_epoch() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        seed_test_call_hash(&mut stx, 0xD1);
        let (_sink, validator, reward_asset, asset_def_id) =
            configure_reward_fixture(&mut stx, LaneId::new(0), 1_000);
        stx.nexus.staking.reward_dust_threshold = Quantity::zero();
        let share = PublicLaneRewardShare {
            account: validator.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: Quantity::from(150_u64),
        };
        RecordPublicLaneRewards {
            lane_id: LaneId::new(0),
            epoch: 1,
            reward_asset: reward_asset.clone(),
            total_reward: Quantity::from(150_u64),
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
        state_block.commit_world_overlay_for_testing().unwrap();
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
        assert_eq!(balance.as_ref(), &Quantity::from(150_u64));
    }
    #[test]
    fn claim_rewards_skips_dust() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.tx_call_hash = Some(Hash::prehashed([0xD2; Hash::LENGTH]));
        let (_sink, validator, reward_asset, asset_def_id) =
            configure_reward_fixture(&mut stx, LaneId::new(11), 500);
        stx.nexus.staking.reward_dust_threshold = 100_u64.into();
        let share = PublicLaneRewardShare {
            account: validator.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: Quantity::from(50_u64),
        };
        RecordPublicLaneRewards {
            lane_id: LaneId::new(11),
            epoch: 1,
            reward_asset: reward_asset.clone(),
            total_reward: Quantity::from(50_u64),
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
        state_block.commit_world_overlay_for_testing().unwrap();
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
    fn claim_rewards_ignores_mismatched_reward_record_rows() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        seed_test_call_hash(&mut stx, 0xD9);
        let lane_id = LaneId::new(13);
        let (_sink, validator, reward_asset, asset_def_id) =
            configure_reward_fixture(&mut stx, lane_id, 50);
        stx.nexus.staking.reward_dust_threshold = Quantity::zero();
        stx.world.public_lane_rewards.insert(
            (lane_id, 1),
            PublicLaneRewardRecord {
                lane_id: LaneId::new(14),
                epoch: 1,
                asset: reward_asset.clone(),
                total_reward: Quantity::from(25_u64),
                shares: vec![PublicLaneRewardShare {
                    account: validator.clone(),
                    role: PublicLaneRewardRole::Validator,
                    amount: Quantity::from(25_u64),
                }],
                metadata: Metadata::default(),
            },
        );
        ClaimPublicLaneRewards {
            lane_id,
            account: validator.clone(),
            upto_epoch: Some(1),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
        let view = state.view();
        assert!(
            view.world
                .public_lane_reward_claims()
                .get(&(lane_id, validator.clone(), reward_asset))
                .is_none(),
            "mismatched reward row must not advance claim cursor"
        );
        let validator_asset = AssetId::new(asset_def_id, validator);
        assert!(
            view.world.assets().get(&validator_asset).is_none(),
            "mismatched reward row must not transfer rewards"
        );
    }
    #[test]
    fn claim_rewards_accepts_i105_fee_sink() {
        let state = setup_state();
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        seed_test_call_hash(&mut stx, 0xD3);
        let (_sink, validator, reward_asset, _asset_def_id) =
            configure_reward_fixture(&mut stx, LaneId::new(12), 200);
        stx.nexus.staking.reward_dust_threshold = Quantity::zero();
        let share = PublicLaneRewardShare {
            account: validator.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: Quantity::from(10_u64),
        };
        RecordPublicLaneRewards {
            lane_id: LaneId::new(12),
            epoch: 1,
            reward_asset: reward_asset.clone(),
            total_reward: Quantity::from(10_u64),
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
        state_block.commit_world_overlay_for_testing().unwrap();
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
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(1_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        let res = SlashPublicLaneValidator {
            lane_id: LaneId::new(5),
            validator: validator.clone(),
            offence_height: stx.block_height(),
            slash_id: Hash::new("slash"),
            amount: Quantity::from(200_u64),
            reason_code: "double_sign".to_string(),
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(res.is_err(), "expected slash ratio guard to reject");
    }
    #[test]
    fn slash_ratio_arithmetic_accepts_representable_results_at_512_bit_boundary() {
        let maximum: Quantity = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
            .parse()
            .expect("signed maximum quantity");
        assert!(slash_within_limit(&maximum, &maximum, 10_000).expect("exact comparison"));
        assert!(!slash_within_limit(&maximum, &maximum, 9_999).expect("exact comparison"));
        let capped = max_slash_amount(&maximum, 9_999)
            .expect("wide product is divided before the final domain check");
        assert!(capped < maximum);
        assert!(slash_within_limit(&capped, &maximum, 9_999).expect("capped amount is legal"));
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
            amount: Quantity::from(100_u64),
        };
        let res = RecordPublicLaneRewards {
            lane_id: LaneId::new(7),
            epoch: 1,
            reward_asset,
            total_reward: Quantity::from(100_u64),
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
            amount: Quantity::from(100_u64),
        };
        RecordPublicLaneRewards {
            lane_id: LaneId::new(8),
            epoch: 2,
            reward_asset: reward_asset.clone(),
            total_reward: Quantity::from(100_u64),
            shares: vec![share.clone()],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("initial record");
        let stale_record = RecordPublicLaneRewards {
            lane_id: LaneId::new(8),
            epoch: 1,
            reward_asset,
            total_reward: Quantity::from(100_u64),
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
            amount: Quantity::zero(),
        };
        let res = RecordPublicLaneRewards {
            lane_id: LaneId::new(10),
            epoch: 1,
            reward_asset,
            total_reward: Quantity::from(50_u64),
            shares: vec![share],
            metadata: Metadata::default(),
        }
        .execute(&ALICE_ID, &mut stx);
        assert!(res.is_err(), "expected zero-share reward to be rejected");
    }
    #[test]
    fn cancel_consensus_evidence_penalty_marks_record() {
        let state = setup_state();
        let roster = vec![ValidatorPower {
            validator: checked_peer_id(),
            power: 1,
        }];
        let context = HeightContext {
            network_id: *state.network_id_ref(),
            protocol_version: PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 1,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"staking cancellation nexus context"),
            execution_policy_hash: Hash::new(b"staking cancellation execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 4,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 1024,
                max_chunk_count: 512,
            },
            leader_seed: [0xA1; Hash::LENGTH],
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"staking cancellation parent state"),
            Hash::new(b"staking cancellation post state"),
            Hash::new(b"staking cancellation ordinary writes"),
            1,
            Hash::new(b"staking cancellation block"),
        );
        let vote = |seed: u8| Vote {
            round,
            proposal_round: round,
            phase: GlobalPhase::Prepare,
            subject: BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([seed; Hash::LENGTH])),
                payload_hash: Hash::new([seed]),
            },
            execution_commitment,
            signer: 0,
            signature: vec![seed; 96],
        };
        let evidence = Evidence {
            equivocation: SumeragiV2EquivocationEvidence {
                context,
                proofs_of_possession: vec![vec![0xD0; 96]],
                conflict: SumeragiV2Equivocation::PhaseVote {
                    first: vote(0xAA),
                    second: vote(0xBB),
                },
            },
        };
        let record = EvidenceRecord {
            evidence: evidence.clone(),
            recorded_at_height: 1,
            recorded_at_view: 0,
            recorded_at_ms: 0,
            penalty_status: EvidencePenaltyStatus::Pending,
        };
        let key = evidence_key(&record.evidence);
        {
            let same_block = new_block_with_height(1);
            let mut same_state_block = state.block(same_block.as_ref().header());
            let mut same_transaction = same_state_block.transaction();
            same_transaction
                .world
                .consensus_evidence
                .insert(key.clone(), record.clone());
            let error = CancelConsensusEvidencePenalty {
                evidence: evidence.clone(),
            }
            .execute(&ALICE_ID, &mut same_transaction)
            .expect_err("same-block evidence admission must not be cancellable");
            assert!(
                matches!(&error, Error::InvariantViolation(message) if message.contains("prior committed block")),
                "unexpected same-block cancellation error: {error:?}"
            );
        }
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
        state_block.commit_world_overlay_for_testing().unwrap();
        let view = state.world.consensus_evidence.view();
        let updated = view.get(&key).expect("evidence record");
        assert_eq!(
            updated.penalty_status,
            EvidencePenaltyStatus::Cancelled { height: 2 }
        );
    }
}
