//! Stake snapshot helpers for commit-roster validation.

use std::collections::BTreeMap;

use iroha_crypto::HashOf;
use iroha_data_model::peer::PeerId;
use iroha_primitives::numeric::Numeric;
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};

use crate::state::WorldReadOnly;

/// Stake snapshot entry for a single validator.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct CommitStakeSnapshotEntry {
    /// Peer identifier for the validator.
    pub peer_id: PeerId,
    /// Total stake attributed to the validator.
    pub stake: Numeric,
}

/// Stake snapshot aligned to a commit roster.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct CommitStakeSnapshot {
    /// Hash of the validator set this snapshot applies to.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Stake entries aligned to the validator set order.
    pub entries: Vec<CommitStakeSnapshotEntry>,
}

impl CommitStakeSnapshot {
    /// Build a stake snapshot for the provided roster using the supplied world view.
    #[must_use]
    pub fn from_roster(world: &impl WorldReadOnly, roster: &[PeerId]) -> Option<Self> {
        if roster.is_empty() {
            return None;
        }
        let stake_map = stake_map_from_world(world);
        if stake_map.is_empty() {
            return Some(Self {
                validator_set_hash: HashOf::new(&roster.to_vec()),
                entries: roster
                    .iter()
                    .map(|peer| CommitStakeSnapshotEntry {
                        peer_id: peer.clone(),
                        stake: Numeric::from(1_u64),
                    })
                    .collect(),
            });
        }
        let mut entries = Vec::with_capacity(roster.len());
        for peer in roster {
            let stake = stake_map.get(peer)?.clone();
            entries.push(CommitStakeSnapshotEntry {
                peer_id: peer.clone(),
                stake,
            });
        }
        Some(Self {
            validator_set_hash: HashOf::new(&roster.to_vec()),
            entries,
        })
    }

    /// Return true if the snapshot hash matches the provided roster.
    #[must_use]
    pub fn matches_roster(&self, roster: &[PeerId]) -> bool {
        self.validator_set_hash == HashOf::new(&roster.to_vec())
    }
}

/// Build a stake map keyed by peer id using the largest stake seen per peer.
#[must_use]
pub(super) fn stake_map_from_world(world: &impl WorldReadOnly) -> BTreeMap<PeerId, Numeric> {
    let mut stake_map: BTreeMap<PeerId, Numeric> = BTreeMap::new();
    for ((_lane_id, validator_id), record) in world.public_lane_validators().iter() {
        let Some(pk) = validator_id.try_signatory() else {
            continue;
        };
        let peer_id = PeerId::from(pk.clone());
        let entry = stake_map
            .entry(peer_id)
            .or_insert_with(|| record.total_stake.clone());
        if record.total_stake > *entry {
            *entry = record.total_stake.clone();
        }
    }
    stake_map
}

#[cfg(test)]
mod tests {
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::AccountId,
        metadata::Metadata,
        nexus::{LaneId, PublicLaneValidatorRecord, PublicLaneValidatorStatus},
        prelude::{DomainId, PeerId},
    };
    use iroha_primitives::numeric::Numeric;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    #[test]
    fn stake_snapshot_from_roster_preserves_order_and_max_stake() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);

        let domain: DomainId = "validators".parse().expect("domain id");
        let keypair_a = KeyPair::random();
        let keypair_b = KeyPair::random();
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let account_a = AccountId::new(domain.clone(), keypair_a.public_key().clone());
        let account_b = AccountId::new(domain, keypair_b.public_key().clone());

        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), account_a.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account_a.clone(),
                    stake_account: account_a.clone(),
                    total_stake: Numeric::new(10, 0),
                    self_stake: Numeric::new(10, 0),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.insert(
                (LaneId::new(2), account_a.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(2),
                    validator: account_a.clone(),
                    stake_account: account_a.clone(),
                    total_stake: Numeric::new(25, 0),
                    self_stake: Numeric::new(25, 0),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.insert(
                (LaneId::new(3), account_b.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(3),
                    validator: account_b.clone(),
                    stake_account: account_b.clone(),
                    total_stake: Numeric::new(15, 0),
                    self_stake: Numeric::new(15, 0),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.commit();
        }

        let view = state.view();
        let stake_map = stake_map_from_world(view.world());
        assert_eq!(stake_map.get(&peer_a), Some(&Numeric::new(25, 0)));
        assert_eq!(stake_map.get(&peer_b), Some(&Numeric::new(15, 0)));

        let roster = vec![peer_b.clone(), peer_a.clone()];
        let snapshot = CommitStakeSnapshot::from_roster(view.world(), &roster).expect("snapshot");
        assert!(snapshot.matches_roster(&roster));
        assert!(!snapshot.matches_roster(&[peer_a.clone(), peer_b.clone()]));
        assert_eq!(snapshot.entries[0].peer_id, peer_b);
        assert_eq!(snapshot.entries[1].peer_id, peer_a);
        assert_eq!(snapshot.entries[1].stake, Numeric::new(25, 0));
    }

    #[test]
    fn stake_snapshot_from_roster_requires_all_peers() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);

        let domain: DomainId = "validators".parse().expect("domain id");
        let keypair = KeyPair::random();
        let account = AccountId::new(domain, keypair.public_key().clone());
        let peer = PeerId::new(keypair.public_key().clone());

        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), account.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account.clone(),
                    stake_account: account,
                    total_stake: Numeric::new(10, 0),
                    self_stake: Numeric::new(10, 0),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.commit();
        }

        let view = state.view();
        let missing_peer = PeerId::new(KeyPair::random().public_key().clone());
        assert!(CommitStakeSnapshot::from_roster(view.world(), &[missing_peer]).is_none());
        assert!(CommitStakeSnapshot::from_roster(view.world(), &[peer]).is_some());
    }

    #[test]
    fn stake_snapshot_from_roster_falls_back_to_equal_weights_without_stake_records() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);

        let keypair_a = KeyPair::random();
        let keypair_b = KeyPair::random();
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let roster = vec![peer_a.clone(), peer_b.clone()];

        let view = state.view();
        let snapshot = CommitStakeSnapshot::from_roster(view.world(), &roster).expect("snapshot");
        assert!(snapshot.matches_roster(&roster));
        assert_eq!(snapshot.entries.len(), roster.len());
        assert_eq!(snapshot.entries[0].peer_id, peer_a);
        assert_eq!(snapshot.entries[1].peer_id, peer_b);
        assert_eq!(snapshot.entries[0].stake, Numeric::from(1_u64));
        assert_eq!(snapshot.entries[1].stake, Numeric::from(1_u64));
    }
}
