//! Stake snapshot helpers for commit-roster validation.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::HashOf;
use iroha_data_model::{
    block::consensus_v2,
    nexus::{LaneId, PublicLaneValidatorStatus},
    peer::PeerId,
};
use iroha_logger::prelude::*;
use iroha_primitives::numeric::{Numeric, NumericSpec, Quantity};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};

#[cfg(test)]
use crate::state::StateView;
use crate::state::{WorldReadOnly, public_lane_validator_record_matches_key};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StakeQuorumError {
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
        Self::from_roster_with_active_lanes(world, roster, None)
    }

    /// Build a stake snapshot using only public-validator records from active Nexus lanes.
    ///
    /// Missing stakes fall back to the chain-configured minimum self-bond (or 1 when unset).
    #[must_use]
    pub fn from_roster_with_active_lanes(
        world: &impl WorldReadOnly,
        roster: &[PeerId],
        active_lane_ids: Option<&BTreeSet<LaneId>>,
    ) -> Option<Self> {
        if roster.is_empty() {
            return None;
        }
        let fallback_stake = fallback_stake_for_world(world);
        let stake_map = stake_map_from_world_with_active_lanes(world, active_lane_ids);
        commit_stake_snapshot_from_map(roster, &stake_map, &fallback_stake)
    }

    /// Return true if the snapshot hash matches the provided roster.
    #[must_use]
    pub fn matches_roster(&self, roster: &[PeerId]) -> bool {
        self.validator_set_hash == HashOf::new(&roster.to_vec())
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
pub fn stake_quorum_reached_for_world_with_active_lanes(
    world: &impl WorldReadOnly,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
    active_lane_ids: Option<&BTreeSet<LaneId>>,
) -> Result<bool, StakeQuorumError> {
    let stake_map = stake_map_for_roster(world, roster, active_lane_ids);
    let total = total_stake_for_roster(roster, &stake_map)?;
    if total.is_zero() {
        return Err(StakeQuorumError::ZeroTotal);
    }
    let signed = selected_stake_for_roster(roster, signers, &stake_map)?;

    let signed_scaled = signed
        .as_numeric()
        .clone()
        .checked_mul(Numeric::from(3_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    let total_scaled = total
        .as_numeric()
        .clone()
        .checked_mul(Numeric::from(2_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    Ok(signed_scaled > total_scaled)
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
pub fn stake_coverage_bps_for_world_with_active_lanes(
    world: &impl WorldReadOnly,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
    active_lane_ids: Option<&BTreeSet<LaneId>>,
) -> Result<u16, StakeQuorumError> {
    let stake_map = stake_map_for_roster(world, roster, active_lane_ids);
    let total = total_stake_for_roster(roster, &stake_map)?;
    if total.is_zero() {
        return Err(StakeQuorumError::ZeroTotal);
    }
    let selected = selected_stake_for_roster(roster, signers, &stake_map)?;

    let selected_scaled = selected
        .as_numeric()
        .clone()
        .checked_mul(Numeric::from(10_000_u64), NumericSpec::unconstrained())
        .ok_or(StakeQuorumError::Overflow)?;
    let bps = selected_scaled
        .checked_div(total.as_numeric().clone(), NumericSpec::integer())
        .and_then(|numeric| numeric.try_mantissa_u128())
        .ok_or(StakeQuorumError::Overflow)?;
    Ok(bps.min(10_000) as u16)
}

/// Return the selected signer stake for a roster using only active Nexus lanes.
pub(super) fn signed_stake_for_world_with_active_lanes(
    world: &impl WorldReadOnly,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
    active_lane_ids: Option<&BTreeSet<LaneId>>,
) -> Result<Quantity, StakeQuorumError> {
    let stake_map = stake_map_for_roster(world, roster, active_lane_ids);
    selected_stake_for_roster(roster, signers, &stake_map)
}

fn stake_map_for_roster(
    world: &impl WorldReadOnly,
    roster: &[PeerId],
    active_lane_ids: Option<&BTreeSet<LaneId>>,
) -> BTreeMap<PeerId, Quantity> {
    let fallback_stake = fallback_stake_for_world(world);
    let mut stake_map = stake_map_from_world_with_active_lanes(world, active_lane_ids);
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
    stake_map
}

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
pub fn stake_quorum_reached_for_snapshot(
    snapshot: &CommitStakeSnapshot,
    roster: &[PeerId],
    signers: &BTreeSet<PeerId>,
) -> Result<bool, StakeQuorumError> {
    if !snapshot.matches_roster(roster) {
        return Err(StakeQuorumError::SnapshotMismatch);
    }
    let mut stake_map: BTreeMap<PeerId, Quantity> = BTreeMap::new();
    for entry in &snapshot.entries {
        let entry_stake = stake_map
            .entry(entry.peer_id.clone())
            .or_insert_with(|| entry.stake.clone());
        if entry.stake > *entry_stake {
            *entry_stake = entry.stake.clone();
        }
    }

    let roster_set: BTreeSet<_> = roster.iter().cloned().collect();
    let mut total = Quantity::zero();
    for peer in roster {
        let Some(stake) = stake_map.get(peer) else {
            return Err(StakeQuorumError::MissingStake);
        };
        total = total
            .checked_add(stake)
            .map_err(|_| StakeQuorumError::Overflow)?;
    }
    if total.is_zero() {
        return Err(StakeQuorumError::ZeroTotal);
    }

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

    let signed_scaled = signed
        .as_numeric()
        .clone()
        .checked_mul(Numeric::from(3_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    let total_scaled = total
        .as_numeric()
        .clone()
        .checked_mul(Numeric::from(2_u64), NumericSpec::default())
        .ok_or(StakeQuorumError::Overflow)?;
    Ok(signed_scaled > total_scaled)
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

/// Convert a staged or finalized NPoS world snapshot into the exact voting
/// powers frozen by a Sumeragi v2 height context.
///
/// Unlike the legacy commit-certificate helper, this function never invents a
/// fallback stake. Every elected validator must have one active, integral,
/// non-zero stake value representable by the v2 wire format. The returned
/// roster is sorted by validator identity, matching the canonical context
/// order used for validator indices and deterministic leader rotation.
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
            .ok_or(StrictV2StakeSnapshotError::MissingStake)?
            .as_numeric();
        if stake.scale() != 0 {
            return Err(StrictV2StakeSnapshotError::FractionalStake);
        }
        let power = stake
            .try_mantissa_u128()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(StrictV2StakeSnapshotError::PowerOutOfRange)?;
        if power == 0 {
            return Err(StrictV2StakeSnapshotError::ZeroStake);
        }
        roster.push(consensus_v2::ValidatorPower {
            validator: validator.clone(),
            power,
        });
    }
    roster.sort_by(|left, right| left.validator.cmp(&right.validator));
    Ok(roster)
}

/// Failure to freeze an exact NPoS voting-power snapshot for Sumeragi v2.
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
    /// Voting power cannot contain fractional stake units.
    #[error("Sumeragi v2 NPoS voting power must be an integer")]
    FractionalStake,
    /// Voting power is required to be strictly positive.
    #[error("Sumeragi v2 NPoS voting power must be non-zero")]
    ZeroStake,
    /// Voting power does not fit the canonical unsigned 64-bit wire field.
    #[error("Sumeragi v2 NPoS voting power is outside the u64 wire range")]
    PowerOutOfRange,
}

pub(super) fn fallback_stake_for_world(world: &impl WorldReadOnly) -> Quantity {
    let min_self_bond = world
        .sumeragi_npos_parameters()
        .map_or(1, |params| params.min_self_bond);
    Quantity::from(min_self_bond.max(1))
}

pub(super) fn commit_stake_snapshot_from_map(
    roster: &[PeerId],
    stake_map: &BTreeMap<PeerId, Quantity>,
    fallback_stake: &Quantity,
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
        let stake = stake_map.get(peer).map_or_else(
            || {
                missing = missing.saturating_add(1);
                fallback_stake.clone()
            },
            Clone::clone,
        );
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
        prelude::PeerId,
    };
    use iroha_primitives::numeric::{Numeric, Quantity};

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

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
    fn strict_v2_roster_uses_exact_stake_and_canonical_identity_order() {
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
            Some(7)
        );
        assert_eq!(
            powers
                .iter()
                .find(|entry| entry.validator == peer_b)
                .map(|entry| entry.power),
            Some(3)
        );
    }

    #[test]
    fn strict_v2_roster_rejects_missing_duplicate_fractional_and_zero_stake() {
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
            record.total_stake =
                Quantity::try_from_numeric(Numeric::new(15_u32, 1)).expect("non-negative stake");
            block.insert((LaneId::SINGLE, account.clone()), record);
            block.commit();
        }
        assert_eq!(
            strict_v2_voting_roster(state.view().world(), std::slice::from_ref(&peer), None),
            Err(StrictV2StakeSnapshotError::FractionalStake)
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
    fn stake_snapshot_from_map_uses_fallback_for_missing_entries() {
        let keypair_a = checked_random_keypair();
        let keypair_b = checked_random_keypair();
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let roster = vec![peer_a.clone(), peer_b.clone()];
        let mut stake_map = BTreeMap::new();
        stake_map.insert(peer_a.clone(), Quantity::from(10_u64));
        let fallback = Quantity::from(3_u64);

        let snapshot =
            commit_stake_snapshot_from_map(&roster, &stake_map, &fallback).expect("snapshot");

        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.entries[0].peer_id, peer_a);
        assert_eq!(snapshot.entries[0].stake, Quantity::from(10_u64));
        assert_eq!(snapshot.entries[1].peer_id, peer_b);
        assert_eq!(snapshot.entries[1].stake, fallback);
    }

    #[test]
    fn stake_snapshot_from_roster_falls_back_for_missing_peers() {
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
        let fallback = fallback_stake_for_world(view.world());
        let snapshot =
            CommitStakeSnapshot::from_roster(view.world(), std::slice::from_ref(&missing_peer))
                .expect("snapshot");
        assert_eq!(snapshot.entries[0].peer_id, missing_peer);
        assert_eq!(snapshot.entries[0].stake, fallback);

        let snapshot = CommitStakeSnapshot::from_roster(view.world(), &[peer]).expect("snapshot");
        assert_eq!(snapshot.entries[0].stake, Quantity::from(10_u32));
    }

    #[test]
    fn stake_snapshot_from_roster_falls_back_to_equal_weights_without_stake_records() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);

        let keypair_a = checked_random_keypair();
        let keypair_b = checked_random_keypair();
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());
        let roster = vec![peer_a.clone(), peer_b.clone()];

        let view = state.view();
        let snapshot = CommitStakeSnapshot::from_roster(view.world(), &roster).expect("snapshot");
        assert!(snapshot.matches_roster(&roster));
        assert_eq!(snapshot.entries.len(), roster.len());
        assert_eq!(snapshot.entries[0].peer_id, peer_a);
        assert_eq!(snapshot.entries[1].peer_id, peer_b);
        assert_eq!(snapshot.entries[0].stake, Quantity::from(1_u64));
        assert_eq!(snapshot.entries[1].stake, Quantity::from(1_u64));
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
    fn stake_quorum_reached_for_peers_falls_back_to_equal_weights() {
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
            Ok(false)
        );
        assert_eq!(
            stake_quorum_reached_for_peers(&view, &roster, &signers),
            Ok(false)
        );
        signers.insert(peer_c.clone());
        assert_eq!(
            stake_quorum_reached_for_world(view.world(), &roster, &signers),
            Ok(true)
        );
        assert_eq!(
            stake_quorum_reached_for_peers(&view, &roster, &signers),
            Ok(true)
        );

        let mut partial = BTreeSet::new();
        partial.insert(peer_a);
        assert_eq!(
            stake_quorum_reached_for_world(view.world(), &roster, &partial),
            Ok(false)
        );
        assert_eq!(
            stake_quorum_reached_for_peers(&view, &roster, &partial),
            Ok(false)
        );
    }

    #[test]
    fn stake_coverage_bps_for_world_reports_selected_coverage() {
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
            Ok(7_500)
        );
    }

    #[test]
    fn stake_quorum_reached_for_peers_falls_back_for_missing_stakes() {
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
        let snapshot =
            CommitStakeSnapshot::from_roster(view.world(), &roster).expect("stake snapshot");
        let direct = stake_quorum_reached_for_peers(&view, &roster, &signers)
            .expect("stake quorum computed");
        let via_snapshot = stake_quorum_reached_for_snapshot(&snapshot, &roster, &signers)
            .expect("stake quorum computed");
        assert_eq!(direct, via_snapshot);
    }
}
