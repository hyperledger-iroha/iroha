//! Stake snapshot helpers for commit-roster validation.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::HashOf;
use iroha_data_model::peer::PeerId;
use iroha_logger::prelude::*;
use iroha_primitives::numeric::{Numeric, NumericSpec};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};

use crate::state::{StateView, WorldReadOnly};

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

/// Errors returned when checking stake quorum for a roster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StakeQuorumError {
    MissingStake,
    SignerOutOfRoster,
    Overflow,
    ZeroTotal,
    SnapshotMismatch,
}

impl CommitStakeSnapshot {
    /// Build a stake snapshot for the provided roster using the supplied world view.
    ///
    /// Missing stakes fall back to the chain-configured minimum self-bond (or 1 when unset).
    #[must_use]
    pub fn from_roster(world: &impl WorldReadOnly, roster: &[PeerId]) -> Option<Self> {
        if roster.is_empty() {
            return None;
        }
        let fallback_stake = fallback_stake_for_world(world);
        let stake_map = stake_map_from_world(world);
        commit_stake_snapshot_from_map(roster, &stake_map, &fallback_stake)
    }

    /// Return true if the snapshot hash matches the provided roster.
    #[must_use]
    pub fn matches_roster(&self, roster: &[PeerId]) -> bool {
        self.validator_set_hash == HashOf::new(&roster.to_vec())
    }
}

/// Determine whether 2/3 stake quorum is reached for the provided signers.
pub(crate) fn stake_quorum_reached_for_peers(
    view: &StateView<'_>,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
) -> Result<bool, StakeQuorumError> {
    let fallback_stake = fallback_stake_for_world(view.world());
    let mut stake_map = stake_map_from_world(view.world());
    if stake_map.is_empty() {
        for peer in roster {
            stake_map.insert(peer.clone(), fallback_stake.clone());
        }
    } else {
        let mut missing = 0usize;
        for peer in roster {
            if !stake_map.contains_key(peer) {
                missing = missing.saturating_add(1);
                stake_map.insert(peer.clone(), fallback_stake.clone());
            }
        }
        if missing > 0 {
            warn!(
                missing,
                roster_len = roster.len(),
                "stake map missing roster entries; using fallback stake"
            );
        }
    }

    let roster_set: BTreeSet<_> = roster.iter().cloned().collect();
    let mut total = Numeric::from(0_u64);
    for peer in roster {
        let Some(stake) = stake_map.get(peer) else {
            return Err(StakeQuorumError::MissingStake);
        };
        total = total
            .checked_add(stake.clone())
            .ok_or(StakeQuorumError::Overflow)?;
    }
    if total.is_zero() {
        return Err(StakeQuorumError::ZeroTotal);
    }

    let mut signed = Numeric::from(0_u64);
    for peer in signers {
        if !roster_set.contains(peer) {
            return Err(StakeQuorumError::SignerOutOfRoster);
        }
        let Some(stake) = stake_map.get(peer) else {
            return Err(StakeQuorumError::MissingStake);
        };
        signed = signed
            .checked_add(stake.clone())
            .ok_or(StakeQuorumError::Overflow)?;
    }

    let signed_scaled = signed
        .checked_mul(Numeric::from(3_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    let total_scaled = total
        .checked_mul(Numeric::from(2_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    Ok(signed_scaled >= total_scaled)
}

/// Determine whether 2/3 stake quorum is reached for the provided signers and snapshot.
pub(crate) fn stake_quorum_reached_for_snapshot(
    snapshot: &CommitStakeSnapshot,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
) -> Result<bool, StakeQuorumError> {
    if !snapshot.matches_roster(roster) {
        return Err(StakeQuorumError::SnapshotMismatch);
    }
    let mut stake_map: BTreeMap<PeerId, Numeric> = BTreeMap::new();
    for entry in &snapshot.entries {
        let entry_stake = stake_map
            .entry(entry.peer_id.clone())
            .or_insert_with(|| entry.stake.clone());
        if entry.stake > *entry_stake {
            *entry_stake = entry.stake.clone();
        }
    }

    let roster_set: BTreeSet<_> = roster.iter().cloned().collect();
    let mut total = Numeric::from(0_u64);
    for peer in roster {
        let Some(stake) = stake_map.get(peer) else {
            return Err(StakeQuorumError::MissingStake);
        };
        total = total
            .checked_add(stake.clone())
            .ok_or(StakeQuorumError::Overflow)?;
    }
    if total.is_zero() {
        return Err(StakeQuorumError::ZeroTotal);
    }

    let mut signed = Numeric::from(0_u64);
    for peer in signers {
        if !roster_set.contains(peer) {
            return Err(StakeQuorumError::SignerOutOfRoster);
        }
        let Some(stake) = stake_map.get(peer) else {
            return Err(StakeQuorumError::MissingStake);
        };
        signed = signed
            .checked_add(stake.clone())
            .ok_or(StakeQuorumError::Overflow)?;
    }

    let signed_scaled = signed
        .checked_mul(Numeric::from(3_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    let total_scaled = total
        .checked_mul(Numeric::from(2_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    Ok(signed_scaled >= total_scaled)
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

pub(super) fn fallback_stake_for_world(world: &impl WorldReadOnly) -> Numeric {
    let min_self_bond = world
        .sumeragi_npos_parameters()
        .map(|params| params.min_self_bond)
        .unwrap_or(1);
    Numeric::from(min_self_bond.max(1))
}

pub(super) fn commit_stake_snapshot_from_map(
    roster: &[PeerId],
    stake_map: &BTreeMap<PeerId, Numeric>,
    fallback_stake: &Numeric,
) -> Option<CommitStakeSnapshot> {
    if roster.is_empty() {
        return None;
    }
    if stake_map.is_empty() {
        return Some(CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&roster.to_vec()),
            entries: roster
                .iter()
                .map(|peer| CommitStakeSnapshotEntry {
                    peer_id: peer.clone(),
                    stake: fallback_stake.clone(),
                })
                .collect(),
        });
    }
    let mut entries = Vec::with_capacity(roster.len());
    let mut missing = 0usize;
    for peer in roster {
        let stake = match stake_map.get(peer) {
            Some(stake) => stake.clone(),
            None => {
                missing = missing.saturating_add(1);
                fallback_stake.clone()
            }
        };
        entries.push(CommitStakeSnapshotEntry {
            peer_id: peer.clone(),
            stake,
        });
    }
    if missing > 0 {
        warn!(
            missing,
            roster_len = roster.len(),
            "missing stake entries for roster; using fallback stake"
        );
    }
    Some(CommitStakeSnapshot {
        validator_set_hash: HashOf::new(&roster.to_vec()),
        entries,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

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
    fn stake_snapshot_from_map_uses_fallback_for_missing_entries() {
        let keypair_a = KeyPair::random();
        let keypair_b = KeyPair::random();
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let roster = vec![peer_a.clone(), peer_b.clone()];
        let mut stake_map = BTreeMap::new();
        stake_map.insert(peer_a.clone(), Numeric::from(10_u64));
        let fallback = Numeric::from(3_u64);

        let snapshot =
            commit_stake_snapshot_from_map(&roster, &stake_map, &fallback).expect("snapshot");

        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.entries[0].peer_id, peer_a);
        assert_eq!(snapshot.entries[0].stake, Numeric::from(10_u64));
        assert_eq!(snapshot.entries[1].peer_id, peer_b);
        assert_eq!(snapshot.entries[1].stake, fallback);
    }

    #[test]
    fn stake_snapshot_from_roster_falls_back_for_missing_peers() {
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
        let fallback = fallback_stake_for_world(view.world());
        let snapshot = CommitStakeSnapshot::from_roster(view.world(), &[missing_peer.clone()])
            .expect("snapshot");
        assert_eq!(snapshot.entries[0].peer_id, missing_peer);
        assert_eq!(snapshot.entries[0].stake, fallback);

        let snapshot = CommitStakeSnapshot::from_roster(view.world(), &[peer]).expect("snapshot");
        assert_eq!(snapshot.entries[0].stake, Numeric::new(10, 0));
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

    #[test]
    fn stake_quorum_reached_for_snapshot_enforces_two_thirds() {
        let peer_a = PeerId::new(KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(KeyPair::random().public_key().clone());
        let peer_c = PeerId::new(KeyPair::random().public_key().clone());
        let roster = vec![peer_a.clone(), peer_b.clone(), peer_c.clone()];
        let snapshot = CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&roster),
            entries: roster
                .iter()
                .cloned()
                .map(|peer_id| CommitStakeSnapshotEntry {
                    peer_id,
                    stake: Numeric::from(1_u64),
                })
                .collect(),
        };
        let mut signers = BTreeSet::new();
        signers.insert(peer_a.clone());
        signers.insert(peer_b.clone());
        assert_eq!(
            stake_quorum_reached_for_snapshot(&snapshot, &roster, &signers),
            Ok(true)
        );

        let mut partial = BTreeSet::new();
        partial.insert(peer_a);
        assert_eq!(
            stake_quorum_reached_for_snapshot(&snapshot, &roster, &partial),
            Ok(false)
        );
    }

    #[test]
    fn stake_quorum_reached_for_peers_falls_back_to_equal_weights() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let view = state.view();

        let peer_a = PeerId::new(KeyPair::random().public_key().clone());
        let peer_b = PeerId::new(KeyPair::random().public_key().clone());
        let peer_c = PeerId::new(KeyPair::random().public_key().clone());
        let roster = vec![peer_a.clone(), peer_b.clone(), peer_c.clone()];

        let mut signers = BTreeSet::new();
        signers.insert(peer_a.clone());
        signers.insert(peer_b.clone());
        assert_eq!(
            stake_quorum_reached_for_peers(&view, &roster, &signers),
            Ok(true)
        );

        let mut partial = BTreeSet::new();
        partial.insert(peer_a);
        assert_eq!(
            stake_quorum_reached_for_peers(&view, &roster, &partial),
            Ok(false)
        );
    }

    #[test]
    fn stake_quorum_reached_for_peers_falls_back_for_missing_stakes() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);

        let domain: DomainId = "validators".parse().expect("domain id");
        let keypair_a = KeyPair::random();
        let keypair_b = KeyPair::random();
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let roster = vec![peer_a.clone(), peer_b.clone()];

        {
            let account_id = AccountId::new(domain, keypair_a.public_key().clone());
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), account_id.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account_id.clone(),
                    stake_account: account_id,
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
        let mut signers = BTreeSet::new();
        signers.insert(peer_a);
        let snapshot =
            CommitStakeSnapshot::from_roster(view.world(), &roster).expect("stake snapshot");
        let direct = stake_quorum_reached_for_peers(&view, &roster, &signers)
            .expect("stake quorum computed");
        let via_snapshot = stake_quorum_reached_for_snapshot(&snapshot, &roster, &signers)
            .expect("stake quorum computed");
        assert_eq!(direct, via_snapshot);
    }
}
