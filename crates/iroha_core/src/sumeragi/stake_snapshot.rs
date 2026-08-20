//! Stake-backed validator eligibility helpers for Sumeragi v2.
use crate::state::{WorldReadOnly, public_lane_validator_record_matches_key};
use iroha_data_model::{
    block::consensus_v2,
    nexus::{LaneId, PublicLaneValidatorStatus},
    peer::PeerId,
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;
use std::collections::{BTreeMap, BTreeSet};
/// Build a stake map keyed by peer id using the largest stake seen per peer.
#[cfg(test)]
#[must_use]
pub(super) fn stake_map_from_world(world: &impl WorldReadOnly) -> BTreeMap<PeerId, Quantity> {
    stake_map_from_world_with_active_lanes(world, None)
}
/// Build a stake map using only active validator records from active Nexus lanes.
#[must_use]
pub(super) fn stake_map_from_world_with_active_lanes(
    world: &impl WorldReadOnly,
    active_lane_ids: Option<&BTreeSet<LaneId>>,
) -> BTreeMap<PeerId, Quantity> {
    let mut stake_map: BTreeMap<PeerId, Quantity> = BTreeMap::new();
    for (key, record) in world.public_lane_validators().iter() {
        if !public_lane_validator_record_matches_key(key, record) {
            continue;
        }
        if active_lane_ids.is_some_and(|active_lane_ids| !active_lane_ids.contains(&record.lane_id))
        {
            continue;
        }
        if !matches!(record.status, PublicLaneValidatorStatus::Active) {
            continue;
        }
        let peer_id = record.peer_id.clone();
        let total_stake = &record.total_stake;
        let entry = stake_map
            .entry(peer_id)
            .or_insert_with(|| total_stake.clone());
        if total_stake > &*entry {
            *entry = total_stake.clone();
        }
    }
    stake_map
}
/// Convert a staged or finalized NPoS world snapshot into the equal-vote
/// roster frozen by a Sumeragi v2 height context.
///
/// Unlike the legacy commit-certificate helper, this function never invents a
/// fallback stake. Every elected validator must have one active, integral,
/// non-zero stake value. Stake determines election eligibility, finalized
/// randomness selects seats, and every elected validator receives exactly one
/// consensus vote. The returned roster is sorted by validator identity,
/// matching the canonical context order used for validator indices and
/// deterministic leader rotation.
pub(crate) fn strict_v2_voting_roster(
    world: &impl WorldReadOnly,
    elected_roster: &[PeerId],
    active_lane_ids: Option<&BTreeSet<LaneId>>,
) -> Result<Vec<consensus_v2::ValidatorPower>, StrictV2StakeSnapshotError> {
    if elected_roster.is_empty() {
        return Err(StrictV2StakeSnapshotError::EmptyRoster);
    }
    let unique = elected_roster.iter().collect::<BTreeSet<_>>();
    if unique.len() != elected_roster.len() {
        return Err(StrictV2StakeSnapshotError::DuplicateValidator);
    }
    let stake_map = stake_map_from_world_with_active_lanes(world, active_lane_ids);
    let mut roster = Vec::with_capacity(elected_roster.len());
    for validator in elected_roster {
        let stake = stake_map
            .get(validator)
            .ok_or(StrictV2StakeSnapshotError::MissingStake)?;
        if stake.is_zero() {
            return Err(StrictV2StakeSnapshotError::ZeroStake);
        }
        roster.push(consensus_v2::ValidatorPower {
            validator: validator.clone(),
            power: 1,
        });
    }
    roster.sort_by(|left, right| left.validator.cmp(&right.validator));
    Ok(roster)
}
/// Failure to freeze an exact NPoS eligibility snapshot for Sumeragi v2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum StrictV2StakeSnapshotError {
    /// The finalized election selected no voting validators.
    #[error("Sumeragi v2 NPoS roster is empty")]
    EmptyRoster,
    /// The finalized election repeated one validator identity.
    #[error("Sumeragi v2 NPoS roster contains a duplicate validator")]
    DuplicateValidator,
    /// An elected validator has no active stake in the frozen lane snapshot.
    #[error("Sumeragi v2 NPoS roster is missing an elected validator stake")]
    MissingStake,
    /// Eligibility stake is required to be strictly positive.
    #[error("Sumeragi v2 NPoS eligibility stake must be non-zero")]
    ZeroStake,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::AccountId,
        metadata::Metadata,
        nexus::{LaneId, PublicLaneValidatorRecord, PublicLaneValidatorStatus},
        prelude::PeerId,
    };
    use iroha_primitives::numeric::Quantity;

    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("stake snapshot fixture key generation should succeed")
    }

    fn active_validator_record(
        lane_id: LaneId,
        account: &AccountId,
        peer: &PeerId,
        stake: u64,
    ) -> PublicLaneValidatorRecord {
        PublicLaneValidatorRecord {
            lane_id,
            validator: account.clone(),
            peer_id: peer.clone(),
            stake_account: account.clone(),
            total_stake: Quantity::from(stake),
            self_stake: Quantity::from(stake),
            metadata: Metadata::default(),
            status: PublicLaneValidatorStatus::Active,
            activation_epoch: None,
            activation_height: None,
            last_reward_epoch: None,
        }
    }

    fn test_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::new_for_testing(World::default(), kura, query)
    }

    #[test]
    fn strict_v2_roster_uses_equal_votes_and_canonical_identity_order() {
        let state = test_state();
        let keypair_a = checked_random_keypair();
        let keypair_b = checked_random_keypair();
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let account_a = AccountId::new(keypair_a.public_key().clone());
        let account_b = AccountId::new(keypair_b.public_key().clone());
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::SINGLE, account_a.clone()),
                active_validator_record(LaneId::SINGLE, &account_a, &peer_a, 7),
            );
            block.insert(
                (LaneId::SINGLE, account_b.clone()),
                active_validator_record(LaneId::SINGLE, &account_b, &peer_b, 3),
            );
            block.commit();
        }

        let powers = strict_v2_voting_roster(
            state.view().world(),
            &[peer_b.clone(), peer_a.clone()],
            Some(&BTreeSet::from([LaneId::SINGLE])),
        )
        .expect("strict frozen powers");
        assert!(
            powers
                .windows(2)
                .all(|pair| pair[0].validator < pair[1].validator)
        );
        assert!(powers.iter().all(|entry| entry.power == 1));
        assert!(powers.iter().any(|entry| entry.validator == peer_a));
        assert!(powers.iter().any(|entry| entry.validator == peer_b));
    }

    #[test]
    fn strict_v2_roster_rejects_missing_duplicate_and_zero_stake() {
        let state = test_state();
        let keypair = checked_random_keypair();
        let peer = PeerId::new(keypair.public_key().clone());
        let account = AccountId::new(keypair.public_key().clone());
        let missing = PeerId::new(checked_random_keypair().public_key().clone());

        assert_eq!(
            strict_v2_voting_roster(state.view().world(), &[], None),
            Err(StrictV2StakeSnapshotError::EmptyRoster)
        );
        assert_eq!(
            strict_v2_voting_roster(state.view().world(), &[peer.clone(), peer.clone()], None),
            Err(StrictV2StakeSnapshotError::DuplicateValidator)
        );
        assert_eq!(
            strict_v2_voting_roster(state.view().world(), std::slice::from_ref(&missing), None),
            Err(StrictV2StakeSnapshotError::MissingStake)
        );

        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::SINGLE, account.clone()),
                active_validator_record(LaneId::SINGLE, &account, &peer, 0),
            );
            block.commit();
        }
        assert_eq!(
            strict_v2_voting_roster(state.view().world(), std::slice::from_ref(&peer), None),
            Err(StrictV2StakeSnapshotError::ZeroStake)
        );
    }

    #[test]
    fn stake_map_ignores_inactive_validators() {
        let state = test_state();
        let active_key = checked_random_keypair();
        let pending_key = checked_random_keypair();
        let active_account = AccountId::new(active_key.public_key().clone());
        let pending_account = AccountId::new(pending_key.public_key().clone());
        let active_peer = PeerId::new(active_key.public_key().clone());
        let pending_peer = PeerId::new(pending_key.public_key().clone());
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), active_account.clone()),
                active_validator_record(LaneId::new(1), &active_account, &active_peer, 5),
            );
            let mut pending =
                active_validator_record(LaneId::new(2), &pending_account, &pending_peer, 9);
            pending.status = PublicLaneValidatorStatus::PendingActivation(0);
            block.insert((LaneId::new(2), pending_account), pending);
            block.commit();
        }

        let stake_map = stake_map_from_world(state.view().world());
        assert_eq!(stake_map.get(&active_peer), Some(&Quantity::from(5_u32)));
        assert!(!stake_map.contains_key(&pending_peer));
    }
}
