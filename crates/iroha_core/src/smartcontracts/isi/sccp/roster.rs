//! Bridge roster generations (`specs/sccp.md` §4.3). Owner: ws30.
//!
//! A generation is created at genesis, at every boundary whose `(peer, address)` members
//! change, on forced rotation and on the heartbeat. The generation that signs height `h` is the
//! one with the greatest `activation_height ≤ h`. A generation with fewer than `t` nonzero
//! members is inert.
//!
//! Validators come and go at any epoch boundary: there is no seat-change batching, no minimum
//! generation interval, no handoff bond and no staking hook.

use super::{bridge_keys, height::SccpHeightInputsV1, store};
use crate::{
    block::BlockValidationError,
    state::{StateTransaction, WorldReadOnly},
};
use iroha_data_model::sccp::{
    events::{
        SccpEvent, SccpRosterDerivationFailedV1, SccpRosterDerivationFailureV1,
        SccpRosterGenerationCreatedV1,
    },
    params::SccpParametersV1,
    roster::{SccpBridgeRosterV1, SccpRosterMemberV1},
};
use iroha_model_base::peer::PeerId;
use iroha_sccp::v1::{
    constants::{MAX_ROSTER_MEMBERS, MIN_ROSTER_MEMBERS},
    roster::{RosterV1, threshold},
};

/// Return the generation that signs `height`: the greatest `activation_height ≤ height`.
#[must_use]
pub fn generation_for_height(world: &(impl WorldReadOnly + ?Sized), height: u64) -> Option<u64> {
    // Generations are numbered in activation order, so the newest qualifying one is found by
    // walking back from the newest generation.
    store::rosters::iter(world)
        .rev()
        .find(|(_, roster)| roster.activation_height <= height)
        .map(|(generation, _)| *generation)
}

/// Return the current generation and its roster, if any generation exists (§4.3.1).
#[must_use]
pub fn current(world: &(impl WorldReadOnly + ?Sized)) -> Option<(u64, &SccpBridgeRosterV1)> {
    let generation = *store::roster_current::get(world);
    store::rosters::get(world, &generation).map(|roster| (generation, roster))
}

/// Return whether `roster` is inert: fewer than `threshold` nonzero members (§4.3.2).
#[must_use]
pub fn is_inert(roster: &SccpBridgeRosterV1) -> bool {
    roster.is_inert()
}

/// Return the bridge-key address `peer` signs with in `epoch`, or zero: its active key when that
/// key is active from `epoch` or earlier and not faulted (§4.3.2).
#[must_use]
pub fn member_address(
    world: &(impl WorldReadOnly + ?Sized),
    peer: &PeerId,
    epoch: u64,
) -> [u8; 20] {
    store::bridge_keys::get(world, peer)
        .and_then(|state| state.active.as_ref())
        .filter(|key| key.activation_epoch <= epoch && !key.faulted)
        .map_or([0; 20], |key| key.address)
}

/// Return `members(E)` of `peers` (§4.3.2): each distinct peer mapped to its epoch-`E`
/// address, ordered per §3.7 (zero slots first, ordered by peer; then nonzero addresses
/// strictly ascending).
#[must_use]
pub fn members(
    world: &(impl WorldReadOnly + ?Sized),
    peers: &[PeerId],
    epoch: u64,
) -> Vec<SccpRosterMemberV1> {
    let mut zero: Vec<PeerId> = Vec::new();
    let mut keyed: Vec<([u8; 20], PeerId)> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for peer in peers {
        if !seen.insert(peer.clone()) {
            continue;
        }
        let address = member_address(world, peer, epoch);
        if address == [0; 20] {
            zero.push(peer.clone());
        } else {
            keyed.push((address, peer.clone()));
        }
    }
    zero.sort();
    keyed.sort();
    zero.into_iter()
        .map(|peer| SccpRosterMemberV1 {
            address: [0; 20],
            peer: Some(peer),
        })
        .chain(keyed.into_iter().map(|(address, peer)| SccpRosterMemberV1 {
            address,
            peer: Some(peer),
        }))
        .collect()
}

/// Outcome of the roster step of one block (§4.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RosterStepV1 {
    /// Whether the block is an epoch boundary.
    pub boundary: bool,
    /// Digest of the generation created at this height, if any (the rotation subject's
    /// `next_roster_digest`).
    pub created_digest: Option<[u8; 32]>,
}

/// Return whether the heartbeat of the current generation is due at block time `now_ms`.
#[must_use]
pub fn heartbeat_due(world: &(impl WorldReadOnly + ?Sized), now_ms: u64) -> Option<u64> {
    let params = store::parameters::get(world).as_ref()?;
    let (generation, roster) = current(world)?;
    (now_ms.saturating_sub(roster.valid_from_ms) >= params.roster_max_age_ms
        && now_ms >= roster.valid_from_ms)
        .then_some(generation)
}

/// Build the stored generation `generation` over `members`, signing from `activation_height`.
fn build_generation(
    network_id: &[u8; 32],
    params: &SccpParametersV1,
    generation: u64,
    members: Vec<SccpRosterMemberV1>,
    valid_from_ms: u64,
    activation_height: u64,
) -> Result<SccpBridgeRosterV1, SccpRosterDerivationFailureV1> {
    let n = members.len();
    if !(MIN_ROSTER_MEMBERS..=MAX_ROSTER_MEMBERS).contains(&n) {
        return Err(SccpRosterDerivationFailureV1::RosterSizeOutOfRange);
    }
    let valid_until_ms = valid_from_ms.saturating_add(params.roster_validity_ms);
    let digest = RosterV1 {
        generation,
        valid_from_ms,
        valid_until_ms,
        members: members.iter().map(|member| member.address).collect(),
    }
    .digest(network_id)
    .map_err(|_| SccpRosterDerivationFailureV1::RosterSizeOutOfRange)?;
    Ok(SccpBridgeRosterV1 {
        generation,
        valid_from_ms,
        valid_until_ms,
        activation_height,
        handoff_height: None,
        members,
        threshold: u8::try_from(threshold(n)).expect("n is at most 31"),
        digest,
    })
}

fn storage(error: impl core::fmt::Display) -> BlockValidationError {
    BlockValidationError::ExecutionContextInvalid(format!("SCCP roster: {error}"))
}

fn emit_failed(
    state_transaction: &mut StateTransaction<'_, '_>,
    height: u64,
    generation: u64,
    reason: SccpRosterDerivationFailureV1,
    roster_size: u32,
) {
    state_transaction
        .world
        .emit_events(Some(SccpEvent::RosterDerivationFailed(
            SccpRosterDerivationFailedV1 {
                height,
                generation,
                reason,
                roster_size,
            },
        )));
}

/// Install `roster` as the new current generation and emit `RosterGenerationCreated`.
fn install(
    state_transaction: &mut StateTransaction<'_, '_>,
    roster: SccpBridgeRosterV1,
) -> Result<[u8; 32], BlockValidationError> {
    let generation = roster.generation;
    let digest = roster.digest;
    let event = SccpRosterGenerationCreatedV1 {
        generation,
        digest,
        activation_height: roster.activation_height,
        valid_until_ms: roster.valid_until_ms,
    };
    store::rosters::insert(state_transaction, generation, roster).map_err(storage)?;
    store::roster_current::set(state_transaction, generation);
    state_transaction
        .world
        .emit_events(Some(SccpEvent::RosterGenerationCreated(event)));
    Ok(digest)
}

/// Apply the roster rule of block `height` created at `now_ms` (§4.3.2).
///
/// Creates generation 1 in the first block that has consensus inputs (the genesis block);
/// afterwards, at rotation heights (epoch boundaries, fault blocks and heartbeat blocks),
/// promotes pending keys at a boundary and creates `g + 1` when the forced, membership-change
/// or heartbeat rule holds. A derivation that fails (missing next roster at a boundary, roster
/// size outside `4..=31`) keeps `g`, emits `SccpRosterDerivationFailed` and does not fail the
/// block.
///
/// # Errors
///
/// Fails the block only on a storage invariant violation or a failing key promotion.
pub fn apply_roster_rule(
    state_transaction: &mut StateTransaction<'_, '_>,
    network_id: &[u8; 32],
    params: &SccpParametersV1,
    height: u64,
    now_ms: u64,
    inputs: &SccpHeightInputsV1,
) -> Result<RosterStepV1, BlockValidationError> {
    let boundary = inputs.is_boundary();
    let Some((generation, roster)) =
        current(&*state_transaction.world).map(|(generation, roster)| (generation, roster.clone()))
    else {
        // Generation 1 from `peers(E)` of the first block with consensus inputs.
        let members = members(&*state_transaction.world, &inputs.roster, inputs.epoch);
        let size = u32::try_from(members.len()).unwrap_or(u32::MAX);
        return match build_generation(network_id, params, 1, members, now_ms, height) {
            Ok(first) => {
                install(state_transaction, first)?;
                Ok(RosterStepV1 {
                    boundary,
                    created_digest: None,
                })
            }
            Err(reason) => {
                emit_failed(state_transaction, height, 0, reason, size);
                Ok(RosterStepV1 {
                    boundary,
                    created_digest: None,
                })
            }
        };
    };

    let fault_block = store::attestation_faults::iter(&*state_transaction.world)
        .any(|(_, fault)| fault.reported_at_height == height);
    let heartbeat = now_ms.saturating_sub(roster.valid_from_ms) >= params.roster_max_age_ms
        && now_ms >= roster.valid_from_ms;
    if !(boundary || fault_block || heartbeat) {
        return Ok(RosterStepV1::default());
    }

    let (next_epoch, next_peers) = if boundary {
        let Some(next) = inputs.next_roster.as_ref() else {
            emit_failed(
                state_transaction,
                height,
                generation,
                SccpRosterDerivationFailureV1::MissingNextEpochSnapshot,
                0,
            );
            return Ok(RosterStepV1 {
                boundary,
                created_digest: None,
            });
        };
        let next_epoch = inputs.epoch.saturating_add(1);
        bridge_keys::promote_pending_for_epoch(state_transaction, next_epoch).map_err(storage)?;
        (next_epoch, next.clone())
    } else {
        (inputs.epoch, inputs.roster.clone())
    };

    let next_members = members(&*state_transaction.world, &next_peers, next_epoch);
    let size = next_members.len();
    if !(MIN_ROSTER_MEMBERS..=MAX_ROSTER_MEMBERS).contains(&size) {
        emit_failed(
            state_transaction,
            height,
            generation,
            SccpRosterDerivationFailureV1::RosterSizeOutOfRange,
            u32::try_from(size).unwrap_or(u32::MAX),
        );
        return Ok(RosterStepV1 {
            boundary,
            created_digest: None,
        });
    }
    if !rotation_required(
        &*state_transaction.world,
        &roster,
        &next_members,
        &next_peers,
        next_epoch,
        boundary,
        heartbeat,
    ) {
        return Ok(RosterStepV1 {
            boundary,
            created_digest: None,
        });
    }
    let successor = match build_generation(
        network_id,
        params,
        generation.saturating_add(1),
        next_members,
        now_ms,
        height.saturating_add(1),
    ) {
        Ok(successor) => successor,
        Err(reason) => {
            let size = u32::try_from(next_peers.len()).unwrap_or(u32::MAX);
            emit_failed(state_transaction, height, generation, reason, size);
            return Ok(RosterStepV1 {
                boundary,
                created_digest: None,
            });
        }
    };
    let mut outgoing = roster;
    outgoing.handoff_height = Some(height);
    store::rosters::insert(state_transaction, generation, outgoing).map_err(storage)?;
    let digest = install(state_transaction, successor)?;
    Ok(RosterStepV1 {
        boundary,
        created_digest: Some(digest),
    })
}

/// Return whether rule (a) forced, (b) membership change or (c) heartbeat requires `g + 1`
/// (§4.3.2 step 3).
fn rotation_required(
    world: &(impl WorldReadOnly + ?Sized),
    roster: &SccpBridgeRosterV1,
    next_members: &[SccpRosterMemberV1],
    next_peers: &[PeerId],
    next_epoch: u64,
    boundary: bool,
    heartbeat: bool,
) -> bool {
    // (a) forced: a nonzero address is no longer its peer's active non-faulted key.
    let key_changed = roster.members.iter().any(|member| {
        member.is_nonzero()
            && member
                .peer
                .as_ref()
                .is_none_or(|peer| member_address(world, peer, next_epoch) != member.address)
    });
    // (a) forced: too many unusable slots (zero, or peer outside the next roster).
    let unusable = roster
        .members
        .iter()
        .filter(|member| {
            !member.is_nonzero()
                || member
                    .peer
                    .as_ref()
                    .is_none_or(|peer| !next_peers.contains(peer))
        })
        .count();
    let n = roster.members.len();
    let t = usize::from(roster.threshold);
    let too_many_unusable = unusable > n.saturating_sub(t);
    // (b) membership change at a boundary, comparing `(peer, address)` pairs.
    let membership_changed = boundary && roster.members.as_slice() != next_members;
    key_changed || too_many_unusable || membership_changed || heartbeat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{
        blank_state, header, peer, sample_roster,
    };
    use iroha_data_model::{
        parameter::system::ConsensusMode,
        sccp::keys::{SccpBridgeKeyStateV1, SccpBridgeKeyV1},
    };

    const NETWORK: [u8; 32] = [7; 32];

    fn params() -> SccpParametersV1 {
        SccpParametersV1::taira_default()
    }

    fn key(state: &mut StateTransaction<'_, '_>, seed: u8, address_seed: u8, epoch: u64) {
        let key = SccpBridgeKeyV1 {
            public_key: [2; 33],
            address: [address_seed; 20],
            activation_epoch: epoch,
            registered_at_height: 1,
            faulted: false,
        };
        let entry = SccpBridgeKeyStateV1 {
            active: Some(key),
            ..SccpBridgeKeyStateV1::default()
        };
        store::bridge_keys::insert(state, peer(seed), entry).expect("key state");
    }

    fn inputs(
        height: u64,
        epoch_end: u64,
        roster: &[u8],
        next: Option<&[u8]>,
    ) -> SccpHeightInputsV1 {
        SccpHeightInputsV1 {
            mode: ConsensusMode::Npos,
            height,
            epoch: 0,
            epoch_end_height: epoch_end,
            roster: roster.iter().map(|seed| peer(*seed)).collect(),
            next_roster: next.map(|seeds| seeds.iter().map(|seed| peer(*seed)).collect()),
        }
    }

    #[test]
    fn the_signing_generation_is_the_latest_activated_one() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        assert_eq!(generation_for_height(&*stx.world, 1), None);
        for (generation, activation_height) in [(1, 1), (2, 101), (3, 250)] {
            store::rosters::insert(
                &mut stx,
                generation,
                sample_roster(generation, activation_height),
            )
            .expect("a roster under its own generation");
        }
        assert_eq!(generation_for_height(&*stx.world, 1), Some(1));
        assert_eq!(generation_for_height(&*stx.world, 100), Some(1));
        assert_eq!(generation_for_height(&*stx.world, 101), Some(2));
        assert_eq!(generation_for_height(&*stx.world, 249), Some(2));
        assert_eq!(generation_for_height(&*stx.world, 10_000), Some(3));
    }

    #[test]
    fn inertness_follows_the_nonzero_member_count() {
        let mut roster = sample_roster(1, 1);
        assert!(!is_inert(&roster));
        for member in &mut roster.members {
            member.address = [0; 20];
        }
        assert!(is_inert(&roster));
    }

    #[test]
    fn members_order_zero_slots_first_then_ascending_addresses() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        key(&mut stx, 1, 0x30, 0);
        key(&mut stx, 2, 0x10, 0);
        key(&mut stx, 3, 0x20, 5); // not yet active in epoch 0
        let peers = [peer(1), peer(2), peer(3), peer(4), peer(1)];
        let members = members(&*stx.world, &peers, 0);
        assert_eq!(members.len(), 4, "duplicate peers collapse");
        assert_eq!(members[0].address, [0; 20]);
        assert_eq!(members[1].address, [0; 20]);
        assert!(
            members[0].peer < members[1].peer,
            "zero slots ordered by peer"
        );
        assert_eq!(members[2].address, [0x10; 20]);
        assert_eq!(members[3].address, [0x30; 20]);
    }

    #[test]
    fn genesis_creates_generation_one_then_boundaries_follow_membership() {
        let state = blank_state();
        let mut block = state.block(header(1));
        let mut stx = block.transaction();
        for seed in 1..=5 {
            key(&mut stx, seed, seed * 16, 0);
        }
        let step = apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            1,
            4_000,
            &inputs(1, 10, &[1, 2, 3, 4], None),
        )
        .expect("genesis roster");
        assert_eq!(step.created_digest, None);
        let (generation, first) = current(&*stx.world).expect("generation 1");
        assert_eq!(generation, 1);
        assert_eq!(first.activation_height, 1);
        assert_eq!(first.threshold, 3);
        assert!(!is_inert(first));

        // A non-boundary block without faults or heartbeat changes nothing.
        let step = apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            5,
            20_000,
            &inputs(5, 10, &[1, 2, 3, 4], None),
        )
        .expect("quiet block");
        assert_eq!(step, RosterStepV1::default());

        // An unchanged boundary keeps the generation.
        let step = apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            10,
            40_000,
            &inputs(10, 10, &[1, 2, 3, 4], Some(&[1, 2, 3, 4])),
        )
        .expect("unchanged boundary");
        assert!(step.boundary);
        assert_eq!(step.created_digest, None);
        assert_eq!(current(&*stx.world).map(|(g, _)| g), Some(1));

        // A validator swap at the next boundary starts generation 2 immediately.
        let step = apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            20,
            80_000,
            &inputs(20, 20, &[1, 2, 3, 4], Some(&[1, 2, 3, 5])),
        )
        .expect("membership change");
        let digest = step.created_digest.expect("generation 2");
        let (generation, second) = current(&*stx.world).expect("generation 2");
        assert_eq!(generation, 2);
        assert_eq!(second.digest, digest);
        assert_eq!(second.activation_height, 21);
        assert_eq!(second.valid_from_ms, 80_000);
        assert_eq!(
            store::rosters::get(&*stx.world, &1).and_then(|g| g.handoff_height),
            Some(20)
        );
    }

    #[test]
    fn swapping_keyless_validators_is_a_membership_change() {
        let state = blank_state();
        let mut block = state.block(header(1));
        let mut stx = block.transaction();
        for seed in 1..=3 {
            key(&mut stx, seed, seed * 16, 0);
        }
        apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            1,
            4_000,
            &inputs(1, 10, &[1, 2, 3, 4], None),
        )
        .expect("genesis");
        let step = apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            10,
            40_000,
            &inputs(10, 10, &[1, 2, 3, 4], Some(&[1, 2, 3, 6])),
        )
        .expect("keyless swap");
        assert!(
            step.created_digest.is_some(),
            "peer 4 → peer 6, both keyless"
        );
    }

    #[test]
    fn a_revoked_key_forces_rotation_at_a_heartbeat_free_fault_block() {
        let state = blank_state();
        let mut block = state.block(header(1));
        let mut stx = block.transaction();
        for seed in 1..=4 {
            key(&mut stx, seed, seed * 16, 0);
        }
        apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            1,
            4_000,
            &inputs(1, 100, &[1, 2, 3, 4], None),
        )
        .expect("genesis");
        // Peer 2's key becomes faulted and a fault is recorded at height 7.
        let mut entry = store::bridge_keys::get(&*stx.world, &peer(2))
            .cloned()
            .expect("key");
        entry.active.as_mut().expect("active").faulted = true;
        store::bridge_keys::insert(&mut stx, peer(2), entry).expect("faulted key");
        store::attestation_faults::insert(
            &mut stx,
            ([32; 20], 5),
            iroha_data_model::sccp::keys::SccpAttestationFaultRecordV1 {
                peer: peer(2),
                statement_hash: [1; 32],
                reported_at_height: 7,
            },
        )
        .expect("fault");
        let step = apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            7,
            28_000,
            &inputs(7, 100, &[1, 2, 3, 4], None),
        )
        .expect("fault block");
        assert!(
            step.created_digest.is_some(),
            "rule (a) forces generation 2"
        );
        let (_, second) = current(&*stx.world).expect("generation 2");
        assert_eq!(
            second.nonzero_member_count(),
            3,
            "the faulted slot becomes zero"
        );
    }

    #[test]
    fn the_heartbeat_rotates_even_without_changes() {
        let state = blank_state();
        let mut block = state.block(header(1));
        let mut stx = block.transaction();
        for seed in 1..=4 {
            key(&mut stx, seed, seed * 16, 0);
        }
        apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            1,
            4_000,
            &inputs(1, 1_000_000, &[1, 2, 3, 4], None),
        )
        .expect("genesis");
        let max_age = params().roster_max_age_ms;
        assert_eq!(heartbeat_due(&*stx.world, 4_000 + max_age - 1), None);
        assert_eq!(heartbeat_due(&*stx.world, 4_000 + max_age), Some(1));
        let step = apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            50,
            4_000 + max_age,
            &inputs(50, 1_000_000, &[1, 2, 3, 4], None),
        )
        .expect("heartbeat");
        assert!(step.created_digest.is_some());
        assert_eq!(
            heartbeat_due(&*stx.world, 4_000 + max_age),
            None,
            "reset by the new generation"
        );
    }

    #[test]
    fn derivation_fails_closed_without_the_next_roster_or_with_a_bad_size() {
        let state = blank_state();
        let mut block = state.block(header(1));
        let mut stx = block.transaction();
        for seed in 1..=4 {
            key(&mut stx, seed, seed * 16, 0);
        }
        apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            1,
            4_000,
            &inputs(1, 10, &[1, 2, 3, 4], None),
        )
        .expect("genesis");
        let step = apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            10,
            40_000,
            &inputs(10, 10, &[1, 2, 3, 4], None),
        )
        .expect("missing next roster does not fail the block");
        assert_eq!(step.created_digest, None);
        let step = apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            20,
            80_000,
            &inputs(20, 20, &[1, 2, 3, 4], Some(&[1, 2, 3])),
        )
        .expect("three validators do not fail the block");
        assert_eq!(step.created_digest, None);
        assert_eq!(
            current(&*stx.world).map(|(g, _)| g),
            Some(1),
            "generation kept"
        );
    }

    #[test]
    fn inert_generations_are_recorded() {
        let state = blank_state();
        let mut block = state.block(header(1));
        let mut stx = block.transaction();
        apply_roster_rule(
            &mut stx,
            &NETWORK,
            &params(),
            1,
            4_000,
            &inputs(1, 10, &[1, 2, 3, 4], None),
        )
        .expect("keyless genesis");
        let (_, first) = current(&*stx.world).expect("generation 1");
        assert!(is_inert(first), "no keys: every slot is zero");
    }
}
