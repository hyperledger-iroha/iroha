//! Stake snapshot helpers for commit-roster validation.
#[cfg(test)]
use crate::state::StateView;
use crate::state::{WorldReadOnly, public_lane_validator_record_matches_key};
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::consensus_v2,
    nexus::{LaneId, PublicLaneValidatorStatus},
    peer::PeerId,
};
use iroha_primitives::numeric::Quantity;
#[cfg(test)]
use iroha_primitives::numeric::{Numeric, RoundingMode};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};
use std::collections::{BTreeMap, BTreeSet};
/// Stake snapshot entry for a single validator.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct CommitStakeSnapshotEntry {
    /// Peer identifier for the validator.
    pub peer_id: PeerId,
    /// Total stake attributed to the validator.
    pub stake: Quantity,
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
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StakeQuorumError {
    DuplicateRoster,
    MissingStake,
    ZeroStake,
    SignerOutOfRoster,
    Overflow,
    ZeroTotal,
    SnapshotMismatch,
}
impl CommitStakeSnapshot {
    /// Build an exact positive stake snapshot for the provided roster.
    ///
    /// Returns `None` for an empty or duplicate roster, or when any roster
    /// member lacks a positive active validator stake. Consensus must never
    /// invent voting power for missing state.
    #[must_use]
    pub fn from_roster(world: &impl WorldReadOnly, roster: &[PeerId]) -> Option<Self> {
        Self::from_roster_with_active_lanes(world, roster, None)
    }
    /// Build an exact stake snapshot using only records from active Nexus lanes.
    #[must_use]
    pub fn from_roster_with_active_lanes(
        world: &impl WorldReadOnly,
        roster: &[PeerId],
        active_lane_ids: Option<&BTreeSet<LaneId>>,
    ) -> Option<Self> {
        let stake_map = stake_map_from_world_with_active_lanes(world, active_lane_ids);
        commit_stake_snapshot_from_exact_map(roster, &stake_map)
    }
    /// Return true only when the snapshot is an exact ordered, positive-stake projection of the
    /// provided duplicate-free roster.
    #[must_use]
    pub fn matches_roster(&self, roster: &[PeerId]) -> bool {
        if self.validator_set_hash != HashOf::new(&roster.to_vec())
            || self.entries.len() != roster.len()
        {
            return false;
        }
        let mut seen = BTreeSet::new();
        self.entries.iter().zip(roster).all(|(entry, peer)| {
            entry.peer_id == *peer && !entry.stake.is_zero() && seen.insert(entry.peer_id.clone())
        })
    }
}
/// Determine whether strict >2/3 stake quorum is reached for the provided signers.
#[cfg(test)]
pub fn stake_quorum_reached_for_world(
    world: &impl WorldReadOnly,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
) -> Result<bool, StakeQuorumError> {
    stake_quorum_reached_for_world_with_active_lanes(world, roster, signers, None)
}
/// Determine whether strict >2/3 stake quorum is reached using only active Nexus lanes.
#[cfg(test)]
pub fn stake_quorum_reached_for_world_with_active_lanes(
    world: &impl WorldReadOnly,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
    active_lane_ids: Option<&BTreeSet<LaneId>>,
) -> Result<bool, StakeQuorumError> {
    let stake_map = stake_map_for_roster(world, roster, active_lane_ids)?;
    let total = total_stake_for_roster(roster, &stake_map)?;
    if total.is_zero() {
        return Err(StakeQuorumError::ZeroTotal);
    }
    let signed = selected_stake_for_roster(roster, signers, &stake_map)?;
    Ok(signed.cmp_mul_u64(3, &total, 2).is_gt())
}
/// Return selected signer stake coverage in basis points for the provided roster.
#[cfg(test)]
pub fn stake_coverage_bps_for_world(
    world: &impl WorldReadOnly,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
) -> Result<u16, StakeQuorumError> {
    stake_coverage_bps_for_world_with_active_lanes(world, roster, signers, None)
}
/// Return selected signer stake coverage using only active Nexus lanes.
#[cfg(test)]
pub fn stake_coverage_bps_for_world_with_active_lanes(
    world: &impl WorldReadOnly,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
    active_lane_ids: Option<&BTreeSet<LaneId>>,
) -> Result<u16, StakeQuorumError> {
    let stake_map = stake_map_for_roster(world, roster, active_lane_ids)?;
    let total = total_stake_for_roster(roster, &stake_map)?;
    if total.is_zero() {
        return Err(StakeQuorumError::ZeroTotal);
    }
    let selected = selected_stake_for_roster(roster, signers, &stake_map)?;
    let bps = selected
        .as_numeric()
        .try_decimal_mul_div_round(
            &Numeric::from(10_000_u64),
            total.as_numeric(),
            0,
            RoundingMode::TowardZero,
        )
        .ok()
        .and_then(|numeric| numeric.try_mantissa_u128())
        .ok_or(StakeQuorumError::Overflow)?;
    Ok(bps.min(10_000) as u16)
}
/// Return the selected signer stake for a roster using only active Nexus lanes.
#[cfg(test)]
pub(super) fn signed_stake_for_world_with_active_lanes(
    world: &impl WorldReadOnly,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
    active_lane_ids: Option<&BTreeSet<LaneId>>,
) -> Result<Quantity, StakeQuorumError> {
    let stake_map = stake_map_for_roster(world, roster, active_lane_ids)?;
    selected_stake_for_roster(roster, signers, &stake_map)
}
#[cfg(test)]
fn stake_map_for_roster(
    world: &impl WorldReadOnly,
    roster: &[PeerId],
    active_lane_ids: Option<&BTreeSet<LaneId>>,
) -> Result<BTreeMap<PeerId, Quantity>, StakeQuorumError> {
    if roster.iter().collect::<BTreeSet<_>>().len() != roster.len() {
        return Err(StakeQuorumError::DuplicateRoster);
    }
    let stake_map = stake_map_from_world_with_active_lanes(world, active_lane_ids);
    for peer in roster {
        let stake = stake_map.get(peer).ok_or(StakeQuorumError::MissingStake)?;
        if stake.is_zero() {
            return Err(StakeQuorumError::ZeroStake);
        }
    }
    Ok(stake_map)
}
#[cfg(test)]
fn total_stake_for_roster(
    roster: &[PeerId],
    stake_map: &BTreeMap<PeerId, Quantity>,
) -> Result<Quantity, StakeQuorumError> {
    let mut total = Quantity::zero();
    for peer in roster {
        let Some(stake) = stake_map.get(peer) else {
            return Err(StakeQuorumError::MissingStake);
        };
        total = total
            .checked_add(stake)
            .map_err(|_| StakeQuorumError::Overflow)?;
    }
    Ok(total)
}
#[cfg(test)]
fn selected_stake_for_roster(
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
    stake_map: &BTreeMap<PeerId, Quantity>,
) -> Result<Quantity, StakeQuorumError> {
    let roster_set: BTreeSet<_> = roster.iter().cloned().collect();
    let mut signed = Quantity::zero();
    for peer in signers {
        if !roster_set.contains(peer) {
            return Err(StakeQuorumError::SignerOutOfRoster);
        }
        let Some(stake) = stake_map.get(peer) else {
            return Err(StakeQuorumError::MissingStake);
        };
        signed = signed
            .checked_add(stake)
            .map_err(|_| StakeQuorumError::Overflow)?;
    }
    Ok(signed)
}
/// Determine whether strict >2/3 stake quorum is reached for the provided signers.
#[cfg(test)]
pub fn stake_quorum_reached_for_peers(
    view: &StateView<'_>,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
) -> Result<bool, StakeQuorumError> {
    stake_quorum_reached_for_world(view.world(), roster, signers)
}
/// Determine whether strict >2/3 stake quorum is reached for the provided signers and snapshot.
#[cfg(test)]
pub fn stake_quorum_reached_for_snapshot(
    snapshot: &CommitStakeSnapshot,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
) -> Result<bool, StakeQuorumError> {
    if !snapshot.matches_roster(roster) {
        return Err(StakeQuorumError::SnapshotMismatch);
    }
    let roster_set: BTreeSet<_> = roster.iter().cloned().collect();
    let mut total = Quantity::zero();
    for entry in &snapshot.entries {
        total = total
            .checked_add(&entry.stake)
            .map_err(|_| StakeQuorumError::Overflow)?;
    }
    let mut signed = Quantity::zero();
    if signers.iter().any(|peer| !roster_set.contains(peer)) {
        return Err(StakeQuorumError::SignerOutOfRoster);
    }
    for entry in &snapshot.entries {
        if signers.contains(&entry.peer_id) {
            signed = signed
                .checked_add(&entry.stake)
                .map_err(|_| StakeQuorumError::Overflow)?;
        }
    }
    Ok(signed.cmp_mul_u64(3, &total, 2).is_gt())
}
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
fn commit_stake_snapshot_from_exact_map(
    roster: &[PeerId],
    stake_map: &BTreeMap<PeerId, Quantity>,
) -> Option<CommitStakeSnapshot> {
    if roster.is_empty() || roster.iter().collect::<BTreeSet<_>>().len() != roster.len() {
        return None;
    }
    let mut entries = Vec::with_capacity(roster.len());
    for peer in roster {
        let stake = stake_map.get(peer)?.clone();
        if stake.is_zero() {
            return None;
        }
        entries.push(CommitStakeSnapshotEntry {
            peer_id: peer.clone(),
            stake,
        });
    }
    Some(CommitStakeSnapshot {
        validator_set_hash: HashOf::new(&roster.to_vec()),
        entries,
    })
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
    use iroha_primitives::numeric::{Numeric, Quantity};
    use std::collections::BTreeSet;
    #[derive(Encode)]
    struct ForgedCommitStakeSnapshotEntry {
        peer_id: PeerId,
        stake: Numeric,
    }
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("stake snapshot fixture key generation should succeed")
    }
    fn checked_random_peer_id() -> PeerId {
        PeerId::new(checked_random_keypair().public_key().clone())
    }
    #[test]
    fn negative_numeric_payload_cannot_decode_as_durable_commit_stake() {
        let forged = ForgedCommitStakeSnapshotEntry {
            peer_id: checked_random_peer_id(),
            stake: Numeric::new(-1_i32, 0),
        };
        let encoded = forged.encode();
        assert!(
            CommitStakeSnapshotEntry::decode(&mut encoded.as_slice()).is_err(),
            "a negative signed payload must not decode as a durable commit stake"
        );
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
    #[test]
    fn stake_snapshot_from_roster_preserves_order_and_max_stake() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let keypair_a = checked_random_keypair();
        let keypair_b = checked_random_keypair();
        let keypair_mismatched = checked_random_keypair();
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let account_a = AccountId::new(keypair_a.public_key().clone());
        let account_b = AccountId::new(keypair_b.public_key().clone());
        let account_mismatched = AccountId::new(keypair_mismatched.public_key().clone());
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), account_a.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account_a.clone(),
                    peer_id: peer_a.clone(),
                    stake_account: account_a.clone(),
                    total_stake: Quantity::from(10_u64),
                    self_stake: Quantity::from(10_u64),
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
                    peer_id: peer_a.clone(),
                    stake_account: account_a.clone(),
                    total_stake: Quantity::from(25_u64),
                    self_stake: Quantity::from(25_u64),
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
                    peer_id: peer_b.clone(),
                    stake_account: account_b.clone(),
                    total_stake: Quantity::from(15_u64),
                    self_stake: Quantity::from(15_u64),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.insert(
                (LaneId::new(4), account_a.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(5),
                    validator: account_a.clone(),
                    peer_id: peer_a.clone(),
                    stake_account: account_a.clone(),
                    total_stake: Quantity::from(9_000_u64),
                    self_stake: Quantity::from(9_000_u64),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.insert(
                (LaneId::new(6), account_mismatched.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(6),
                    validator: account_a.clone(),
                    peer_id: peer_a.clone(),
                    stake_account: account_mismatched,
                    total_stake: Quantity::from(8_000_u64),
                    self_stake: Quantity::from(8_000_u64),
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
        assert_eq!(stake_map.get(&peer_a), Some(&Quantity::from(25_u32)));
        assert_ne!(
            stake_map.get(&peer_a),
            Some(&Quantity::from(9_000_u32)),
            "mismatched key/record lane rows must not inflate stake"
        );
        assert_eq!(stake_map.get(&peer_b), Some(&Quantity::from(15_u32)));
        let roster = vec![peer_b.clone(), peer_a.clone()];
        let snapshot = CommitStakeSnapshot::from_roster(view.world(), &roster).expect("snapshot");
        assert!(snapshot.matches_roster(&roster));
        assert!(!snapshot.matches_roster(&[peer_a.clone(), peer_b.clone()]));
        assert_eq!(snapshot.entries[0].peer_id, peer_b);
        assert_eq!(snapshot.entries[1].peer_id, peer_a);
        assert_eq!(snapshot.entries[1].stake, Quantity::from(25_u32));
    }
    #[test]
    fn stake_snapshot_active_lane_filter_ignores_higher_stale_unknown_lane_stake() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let keypair = checked_random_keypair();
        let peer = PeerId::new(keypair.public_key().clone());
        let account = AccountId::new(keypair.public_key().clone());
        let stale_lane = LaneId::new(42);
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::SINGLE, account.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::SINGLE,
                    validator: account.clone(),
                    peer_id: peer.clone(),
                    stake_account: account.clone(),
                    total_stake: Quantity::from(10_u64),
                    self_stake: Quantity::from(10_u64),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.insert(
                (stale_lane, account.clone()),
                PublicLaneValidatorRecord {
                    lane_id: stale_lane,
                    validator: account.clone(),
                    peer_id: peer.clone(),
                    stake_account: account,
                    total_stake: Quantity::from(10_000_u64),
                    self_stake: Quantity::from(10_000_u64),
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
        let active_lane_ids = BTreeSet::from([LaneId::SINGLE]);
        let unfiltered = stake_map_from_world(view.world());
        let filtered = stake_map_from_world_with_active_lanes(view.world(), Some(&active_lane_ids));
        assert_eq!(unfiltered.get(&peer), Some(&Quantity::from(10_000_u32)));
        assert_eq!(filtered.get(&peer), Some(&Quantity::from(10_u32)));
        let snapshot = CommitStakeSnapshot::from_roster_with_active_lanes(
            view.world(),
            std::slice::from_ref(&peer),
            Some(&active_lane_ids),
        )
        .expect("filtered stake snapshot");
        assert_eq!(snapshot.entries[0].peer_id, peer);
        assert_eq!(snapshot.entries[0].stake, Quantity::from(10_u32));
    }
    #[test]
    fn strict_v2_roster_uses_equal_votes_and_canonical_identity_order() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
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
        let view = state.view();
        let powers = strict_v2_voting_roster(
            view.world(),
            &[peer_b.clone(), peer_a.clone()],
            Some(&BTreeSet::from([LaneId::SINGLE])),
        )
        .expect("strict frozen powers");
        assert!(
            powers
                .windows(2)
                .all(|pair| pair[0].validator < pair[1].validator)
        );
        assert_eq!(
            powers
                .iter()
                .find(|entry| entry.validator == peer_a)
                .map(|entry| entry.power),
            Some(1)
        );
        assert_eq!(
            powers
                .iter()
                .find(|entry| entry.validator == peer_b)
                .map(|entry| entry.power),
            Some(1)
        );
    }
    #[test]
    fn strict_v2_roster_rejects_missing_duplicate_and_zero_stake() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let keypair = checked_random_keypair();
        let peer = PeerId::new(keypair.public_key().clone());
        let account = AccountId::new(keypair.public_key().clone());
        let missing = checked_random_peer_id();
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
            let mut record = active_validator_record(LaneId::SINGLE, &account, &peer, 1);
            record.total_stake = "1.5".parse().expect("non-negative stake");
            block.insert((LaneId::SINGLE, account.clone()), record);
            block.commit();
        }
        assert_eq!(
            strict_v2_voting_roster(state.view().world(), std::slice::from_ref(&peer), None)
                .expect("fractional eligibility stake does not weight consensus")[0]
                .power,
            1
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
    fn stake_quorum_coverage_and_signed_stake_active_lane_filter_unknown_lane_stake() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let keypair_a = checked_random_keypair();
        let keypair_b = checked_random_keypair();
        let keypair_c = checked_random_keypair();
        let account_a = AccountId::new(keypair_a.public_key().clone());
        let account_b = AccountId::new(keypair_b.public_key().clone());
        let account_c = AccountId::new(keypair_c.public_key().clone());
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let peer_c = PeerId::new(keypair_c.public_key().clone());
        let stale_lane = LaneId::new(42);
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::SINGLE, account_a.clone()),
                active_validator_record(LaneId::SINGLE, &account_a, &peer_a, 1),
            );
            block.insert(
                (LaneId::SINGLE, account_b.clone()),
                active_validator_record(LaneId::SINGLE, &account_b, &peer_b, 1),
            );
            block.insert(
                (LaneId::SINGLE, account_c.clone()),
                active_validator_record(LaneId::SINGLE, &account_c, &peer_c, 1),
            );
            block.insert(
                (stale_lane, account_a.clone()),
                active_validator_record(stale_lane, &account_a, &peer_a, 10_000),
            );
            block.commit();
        }
        let view = state.view();
        let roster = vec![peer_a.clone(), peer_b, peer_c];
        let signers = BTreeSet::from([peer_a]);
        let active_lane_ids = BTreeSet::from([LaneId::SINGLE]);
        assert_eq!(
            stake_quorum_reached_for_world(view.world(), &roster, &signers),
            Ok(true)
        );
        assert_eq!(
            stake_quorum_reached_for_world_with_active_lanes(
                view.world(),
                &roster,
                &signers,
                Some(&active_lane_ids),
            ),
            Ok(false)
        );
        assert_eq!(
            stake_coverage_bps_for_world(view.world(), &roster, &signers),
            Ok(9_998)
        );
        assert_eq!(
            stake_coverage_bps_for_world_with_active_lanes(
                view.world(),
                &roster,
                &signers,
                Some(&active_lane_ids),
            ),
            Ok(3_333)
        );
        assert_eq!(
            signed_stake_for_world_with_active_lanes(
                view.world(),
                &roster,
                &signers,
                Some(&active_lane_ids),
            ),
            Ok(Quantity::from(1_u32))
        );
    }
    #[test]
    fn stake_map_ignores_inactive_validators() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let keypair_active = checked_random_keypair();
        let keypair_pending = checked_random_keypair();
        let account_active = AccountId::new(keypair_active.public_key().clone());
        let account_pending = AccountId::new(keypair_pending.public_key().clone());
        let peer_active = PeerId::new(keypair_active.public_key().clone());
        let peer_pending = PeerId::new(keypair_pending.public_key().clone());
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), account_active.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account_active.clone(),
                    peer_id: peer_active.clone(),
                    stake_account: account_active,
                    total_stake: Quantity::from(5_u64),
                    self_stake: Quantity::from(5_u64),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.insert(
                (LaneId::new(2), account_pending.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(2),
                    validator: account_pending.clone(),
                    peer_id: peer_pending.clone(),
                    stake_account: account_pending,
                    total_stake: Quantity::from(9_u64),
                    self_stake: Quantity::from(9_u64),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::PendingActivation(0),
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.commit();
        }
        let view = state.view();
        let stake_map = stake_map_from_world(view.world());
        assert_eq!(stake_map.get(&peer_active), Some(&Quantity::from(5_u32)));
        assert!(!stake_map.contains_key(&peer_pending));
    }
    #[test]
    fn exact_stake_snapshot_rejects_missing_zero_and_duplicate_entries() {
        let keypair_a = checked_random_keypair();
        let keypair_b = checked_random_keypair();
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let roster = vec![peer_a.clone(), peer_b.clone()];
        let mut stake_map = BTreeMap::new();
        stake_map.insert(peer_a.clone(), Quantity::from(10_u64));
        assert!(commit_stake_snapshot_from_exact_map(&roster, &stake_map).is_none());
        stake_map.insert(peer_b.clone(), Quantity::zero());
        assert!(commit_stake_snapshot_from_exact_map(&roster, &stake_map).is_none());
        stake_map.insert(peer_b, Quantity::from(3_u64));
        let snapshot = commit_stake_snapshot_from_exact_map(&roster, &stake_map)
            .expect("complete positive stake map");
        assert!(snapshot.matches_roster(&roster));
        assert!(
            commit_stake_snapshot_from_exact_map(&[peer_a.clone(), peer_a], &stake_map).is_none(),
            "duplicate roster identities must never duplicate voting power"
        );
    }
    #[test]
    fn stake_snapshot_from_roster_rejects_missing_peers() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let keypair = checked_random_keypair();
        let account = AccountId::new(keypair.public_key().clone());
        let peer = PeerId::new(keypair.public_key().clone());
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), account.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account.clone(),
                    peer_id: peer.clone(),
                    stake_account: account,
                    total_stake: Quantity::from(10_u64),
                    self_stake: Quantity::from(10_u64),
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
        let missing_peer = checked_random_peer_id();
        assert!(
            CommitStakeSnapshot::from_roster(view.world(), std::slice::from_ref(&missing_peer))
                .is_none()
        );
        let snapshot = CommitStakeSnapshot::from_roster(view.world(), &[peer]).expect("snapshot");
        assert_eq!(snapshot.entries[0].stake, Quantity::from(10_u32));
    }
    #[test]
    fn stake_snapshot_from_roster_rejects_absent_stake_authority() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let keypair_a = checked_random_keypair();
        let keypair_b = checked_random_keypair();
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let roster = vec![peer_a.clone(), peer_b.clone()];
        let view = state.view();
        assert!(CommitStakeSnapshot::from_roster(view.world(), &roster).is_none());
    }
    #[test]
    fn stake_quorum_reached_for_snapshot_enforces_strict_two_thirds() {
        let peer_a = checked_random_peer_id();
        let peer_b = checked_random_peer_id();
        let peer_c = checked_random_peer_id();
        let roster = vec![peer_a.clone(), peer_b.clone(), peer_c.clone()];
        let snapshot = CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&roster),
            entries: roster
                .iter()
                .cloned()
                .map(|peer_id| CommitStakeSnapshotEntry {
                    peer_id,
                    stake: Quantity::from(1_u64),
                })
                .collect(),
        };
        let mut signers = BTreeSet::new();
        signers.insert(peer_a.clone());
        signers.insert(peer_b.clone());
        assert_eq!(
            stake_quorum_reached_for_snapshot(&snapshot, &roster, &signers),
            Ok(false)
        );
        signers.insert(peer_c.clone());
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
    fn stake_snapshot_rejects_reordered_duplicate_missing_extra_and_zero_entries() {
        let peer_a = checked_random_peer_id();
        let peer_b = checked_random_peer_id();
        let peer_c = checked_random_peer_id();
        let roster = vec![peer_a.clone(), peer_b.clone(), peer_c.clone()];
        let snapshot = CommitStakeSnapshot {
            validator_set_hash: HashOf::new(&roster),
            entries: roster
                .iter()
                .cloned()
                .map(|peer_id| CommitStakeSnapshotEntry {
                    peer_id,
                    stake: Quantity::from(1_u64),
                })
                .collect(),
        };
        let signers = BTreeSet::from([peer_a.clone(), peer_b.clone(), peer_c.clone()]);
        let mut reordered = snapshot.clone();
        reordered.entries.swap(0, 1);
        let mut duplicate_inflated = snapshot.clone();
        duplicate_inflated.entries[1] = CommitStakeSnapshotEntry {
            peer_id: peer_a,
            stake: Quantity::from(1_000_000_u64),
        };
        let mut missing = snapshot.clone();
        missing.entries.pop();
        let mut extra = snapshot.clone();
        extra.entries.push(CommitStakeSnapshotEntry {
            peer_id: checked_random_peer_id(),
            stake: Quantity::from(1_u64),
        });
        let mut zero = snapshot;
        zero.entries[0].stake = Quantity::zero();
        for malformed in [reordered, duplicate_inflated, missing, extra, zero] {
            assert!(!malformed.matches_roster(&roster));
            assert_eq!(
                stake_quorum_reached_for_snapshot(&malformed, &roster, &signers),
                Err(StakeQuorumError::SnapshotMismatch)
            );
        }
    }
    #[test]
    fn stake_quorum_rejects_roster_without_stake_authority() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let view = state.view();
        let peer_a = checked_random_peer_id();
        let peer_b = checked_random_peer_id();
        let peer_c = checked_random_peer_id();
        let roster = vec![peer_a.clone(), peer_b.clone(), peer_c.clone()];
        let mut signers = BTreeSet::new();
        signers.insert(peer_a.clone());
        signers.insert(peer_b.clone());
        assert_eq!(
            stake_quorum_reached_for_world(view.world(), &roster, &signers),
            Err(StakeQuorumError::MissingStake)
        );
        assert_eq!(
            stake_quorum_reached_for_peers(&view, &roster, &signers),
            Err(StakeQuorumError::MissingStake)
        );
        signers.insert(peer_c.clone());
        assert_eq!(
            stake_quorum_reached_for_world(view.world(), &roster, &signers),
            Err(StakeQuorumError::MissingStake)
        );
        assert_eq!(
            stake_quorum_reached_for_peers(&view, &roster, &signers),
            Err(StakeQuorumError::MissingStake)
        );
        let mut partial = BTreeSet::new();
        partial.insert(peer_a);
        assert_eq!(
            stake_quorum_reached_for_world(view.world(), &roster, &partial),
            Err(StakeQuorumError::MissingStake)
        );
        assert_eq!(
            stake_quorum_reached_for_peers(&view, &roster, &partial),
            Err(StakeQuorumError::MissingStake)
        );
    }
    #[test]
    fn stake_quorum_rejects_duplicate_roster_and_zero_power() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let keypair = checked_random_keypair();
        let peer = PeerId::new(keypair.public_key().clone());
        let account = AccountId::new(keypair.public_key().clone());
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::SINGLE, account.clone()),
                active_validator_record(LaneId::SINGLE, &account, &peer, 0),
            );
            block.commit();
        }
        let view = state.view();
        let signers = BTreeSet::from([peer.clone()]);
        assert_eq!(
            stake_quorum_reached_for_world(view.world(), &[peer.clone(), peer.clone()], &signers),
            Err(StakeQuorumError::DuplicateRoster)
        );
        assert_eq!(
            stake_quorum_reached_for_world(view.world(), &[peer], &signers),
            Err(StakeQuorumError::ZeroStake)
        );
    }
    #[test]
    fn stake_coverage_rejects_roster_without_stake_authority() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let view = state.view();
        let peer_a = checked_random_peer_id();
        let peer_b = checked_random_peer_id();
        let peer_c = checked_random_peer_id();
        let peer_d = checked_random_peer_id();
        let roster = vec![peer_a.clone(), peer_b.clone(), peer_c.clone(), peer_d];
        let mut selected = BTreeSet::new();
        selected.insert(peer_a);
        selected.insert(peer_b);
        selected.insert(peer_c);
        assert_eq!(
            stake_coverage_bps_for_world(view.world(), &roster, &selected),
            Err(StakeQuorumError::MissingStake)
        );
    }
    #[test]
    fn stake_quorum_and_snapshot_reject_partially_missing_stakes() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let keypair_a = checked_random_keypair();
        let keypair_b = checked_random_keypair();
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let roster = vec![peer_a.clone(), peer_b.clone()];
        {
            let account_id = AccountId::new(keypair_a.public_key().clone());
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), account_id.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account_id.clone(),
                    peer_id: peer_a.clone(),
                    stake_account: account_id,
                    total_stake: Quantity::from(10_u64),
                    self_stake: Quantity::from(10_u64),
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
        assert!(CommitStakeSnapshot::from_roster(view.world(), &roster).is_none());
        assert_eq!(
            stake_quorum_reached_for_peers(&view, &roster, &signers),
            Err(StakeQuorumError::MissingStake)
        );
    }
}
