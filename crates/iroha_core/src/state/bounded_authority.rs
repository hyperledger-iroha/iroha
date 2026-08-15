//! Allocation-bounded helpers for deterministic lane-authority selection.
use super::{WorldReadOnly, peer_has_live_consensus_key, public_lane_validator_record_matches_key};
use crate::governance::manifest::{LANE_MANIFEST_MAX_VALIDATORS_V1, ManifestValidatorBinding};
use iroha_config::parameters::actual::LaneValidatorMode;
use iroha_data_model::{
    account::AccountId,
    consensus::MAX_LANE_CONSENSUS_VALIDATORS,
    nexus::{DataSpaceId, LaneId, PublicLaneValidatorStatus},
    peer::PeerId,
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;
pub(super) struct LaneAuthorityInputs {
    pub(super) dataspace_id: DataSpaceId,
    pub(super) autoscale_validator_set: Option<Vec<PeerId>>,
    pub(super) validator_mode: LaneValidatorMode,
    pub(super) minimum_stake: Quantity,
    pub(super) max_validators: u32,
}
pub(super) fn staking_validator_limit_from(configured: u32) -> Option<usize> {
    let limit = usize::try_from(configured).ok()?;
    (limit <= MAX_LANE_CONSENSUS_VALIDATORS).then_some(limit)
}
pub(super) fn insert_unique_by<T: Copy>(
    selected: &mut Vec<T>,
    candidate: T,
    limit: usize,
    same: impl Fn(T, T) -> bool,
    rank: impl Fn(T, T) -> std::cmp::Ordering,
) {
    if limit == 0 {
        return;
    }
    if let Some(index) = selected
        .iter()
        .position(|selected| same(candidate, *selected))
    {
        if !rank(candidate, selected[index]).is_lt() {
            return;
        }
        selected.remove(index);
    } else if selected.len() == limit {
        if !rank(
            candidate,
            *selected.last().expect("non-zero bounded selection"),
        )
        .is_lt()
        {
            return;
        }
        selected.pop();
    }
    let index = selected
        .binary_search_by(|selected| rank(*selected, candidate))
        .unwrap_or_else(|index| index);
    selected.insert(index, candidate);
}
pub(super) fn live_manifest_validator_bindings(
    world: &impl WorldReadOnly,
    bindings: &[ManifestValidatorBinding],
    declared_validators: &[AccountId],
    block_height: u64,
) -> Vec<ManifestValidatorBinding> {
    if declared_validators.len() > LANE_MANIFEST_MAX_VALIDATORS_V1
        || bindings.len() > LANE_MANIFEST_MAX_VALIDATORS_V1
        || declared_validators
            .iter()
            .enumerate()
            .any(|(index, validator)| declared_validators[..index].contains(validator))
        || bindings.iter().enumerate().any(|(index, binding)| {
            !declared_validators.contains(&binding.validator)
                || bindings[..index].iter().any(|seen| {
                    seen.validator == binding.validator || seen.peer_id == binding.peer_id
                })
        })
    {
        return Vec::new();
    }
    let mut live: Vec<_> = bindings
        .iter()
        .filter(|binding| {
            world.peers().iter().any(|peer| peer == &binding.peer_id)
                && peer_has_live_consensus_key(world, &binding.peer_id, block_height)
        })
        .cloned()
        .collect();
    live.sort_by(|lhs, rhs| {
        lhs.validator
            .cmp(&rhs.validator)
            .then_with(|| lhs.peer_id.cmp(&rhs.peer_id))
    });
    live
}
pub(super) fn live_manifest_validator_account_peers(
    world: &impl WorldReadOnly,
    validators: &[AccountId],
    block_height: u64,
) -> Vec<(AccountId, PeerId)> {
    if validators.len() > LANE_MANIFEST_MAX_VALIDATORS_V1
        || validators
            .iter()
            .enumerate()
            .any(|(index, validator)| validators[..index].contains(validator))
    {
        return Vec::new();
    }
    let mut live: Vec<_> = validators
        .iter()
        .filter_map(|validator| {
            let peer = PeerId::new(validator.try_signatory()?.clone());
            (world.peers().iter().any(|present| present == &peer)
                && peer_has_live_consensus_key(world, &peer, block_height))
            .then(|| (validator.clone(), peer))
        })
        .collect();
    live.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    live
}
pub(super) fn stake_elected_validator_accounts(
    world: &impl WorldReadOnly,
    lane_id: LaneId,
    minimum_stake: &Quantity,
    configured_limit: u32,
    block_height: u64,
) -> Vec<AccountId> {
    let Some(limit) = staking_validator_limit_from(configured_limit) else {
        return Vec::new();
    };
    let mut selected = Vec::with_capacity(limit);
    for (key, record) in world.public_lane_validators().iter() {
        if !public_lane_validator_record_matches_key(key, record)
            || key.0 != lane_id
            || !matches!(record.status, PublicLaneValidatorStatus::Active)
            || !crate::smartcontracts::isi::staking::meets_min_stake(
                &record.self_stake,
                minimum_stake,
            )
            .unwrap_or(false)
            || !world.peers().iter().any(|peer| peer == &record.peer_id)
            || !peer_has_live_consensus_key(world, &record.peer_id, block_height)
        {
            continue;
        }
        insert_unique_by(
            &mut selected,
            record,
            limit,
            |lhs, rhs| lhs.validator == rhs.validator,
            |lhs, rhs| {
                rhs.total_stake
                    .cmp(&lhs.total_stake)
                    .then_with(|| lhs.validator.cmp(&rhs.validator))
            },
        );
    }
    selected
        .into_iter()
        .map(|record| record.validator.clone())
        .collect()
}
pub(super) fn stake_elected_peer_ids(
    world: &impl WorldReadOnly,
    lane_id: LaneId,
    minimum_stake: &Quantity,
    configured_limit: u32,
    block_height: u64,
) -> Vec<PeerId> {
    let Some(limit) = staking_validator_limit_from(configured_limit) else {
        return Vec::new();
    };
    let mut selected = Vec::with_capacity(limit);
    for (key, record) in world.public_lane_validators().iter() {
        if !public_lane_validator_record_matches_key(key, record)
            || key.0 != lane_id
            || !matches!(record.status, PublicLaneValidatorStatus::Active)
            || !crate::smartcontracts::isi::staking::meets_min_stake(
                &record.self_stake,
                minimum_stake,
            )
            .unwrap_or(false)
            || !world.peers().iter().any(|peer| peer == &record.peer_id)
            || !peer_has_live_consensus_key(world, &record.peer_id, block_height)
        {
            continue;
        }
        insert_unique_by(
            &mut selected,
            record,
            limit,
            |lhs, rhs| lhs.peer_id == rhs.peer_id,
            |lhs, rhs| {
                rhs.total_stake
                    .cmp(&lhs.total_stake)
                    .then_with(|| lhs.validator.cmp(&rhs.validator))
                    .then_with(|| lhs.peer_id.cmp(&rhs.peer_id))
            },
        );
    }
    selected
        .into_iter()
        .map(|record| record.peer_id.clone())
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Candidate {
        identity: u16,
        stake: u64,
    }
    fn rank(lhs: &Candidate, rhs: &Candidate) -> std::cmp::Ordering {
        rhs.stake
            .cmp(&lhs.stake)
            .then_with(|| lhs.identity.cmp(&rhs.identity))
    }
    #[test]
    fn bounded_selection_matches_full_reference_and_never_grows() {
        const LIMIT: usize = 128;
        let input: Vec<_> = (0_u16..4096)
            .map(|index| Candidate {
                identity: index % 997,
                stake: u64::from(index.wrapping_mul(7919) % 4093),
            })
            .collect();
        let mut bounded = Vec::with_capacity(LIMIT);
        let initial_capacity = bounded.capacity();
        for candidate in &input {
            insert_unique_by(
                &mut bounded,
                candidate,
                LIMIT,
                |lhs, rhs| lhs.identity == rhs.identity,
                rank,
            );
            assert!(bounded.len() <= LIMIT);
            assert_eq!(bounded.capacity(), initial_capacity);
        }
        let mut reference: Vec<_> = input.iter().collect();
        reference.sort_by(|lhs, rhs| rank(lhs, rhs));
        reference.dedup_by_key(|candidate| candidate.identity);
        reference.truncate(LIMIT);
        assert_eq!(bounded, reference);
    }
}
