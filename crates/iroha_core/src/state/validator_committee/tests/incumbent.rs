//! The incumbent authority and the scheduling epochs come from committed World state.

use super::*;
use crate::{kura::Kura, query::store::LiveQueryStore, state::State};
use iroha_data_model::parameter::{
    custom::CustomParameter,
    system::{
        ConsensusFingerprint, ConsensusHandshakeMetadata, SumeragiConsensusMode, consensus_metadata,
    },
};
use iroha_primitives::json::Json;
use std::num::NonZeroU64;

fn roster(size: u8) -> Vec<ValidatorPower> {
    let mut roster = (1..=size)
        .map(|seed| ValidatorPower {
            validator: PeerId::new(
                KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .public_key()
                    .clone(),
            ),
            power: 1,
        })
        .collect::<Vec<_>>();
    roster.sort_by(|left, right| left.validator.cmp(&right.validator));
    roster
}

fn handshake_metadata(roster: &[ValidatorPower]) -> Parameter {
    let metadata = ConsensusHandshakeMetadata {
        mode: SumeragiConsensusMode::Permissioned,
        block_cadence_ms: NonZeroU64::new(1_000).unwrap(),
        wire_protocol_version: u32::from(iroha_data_model::block::consensus_v2::PROTOCOL_VERSION),
        consensus_fingerprint: ConsensusFingerprint::new([0xA5; 32]),
        kagemusha_mint_finality:
            crate::kagemusha_v1_test_fixtures::mint_finality_genesis_parameters(roster),
        sumeragi_v2: crate::kagemusha_v1_test_fixtures::genesis_context_parameters(),
    };
    metadata.validate().unwrap();
    let value = norito::json::value::to_value(&metadata).unwrap();
    Parameter::Custom(CustomParameter::new(
        consensus_metadata::handshake_meta_id(),
        Json::from_norito_value_ref(&value).unwrap(),
    ))
}

fn state_on(network: iroha_data_model::NetworkId, world: World) -> State {
    State::new_with_chain_and_network_id_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        iroha_model_base::chain::ChainId::from("validator-committee-incumbent"),
        network,
    )
}

fn network_id_of(tag: &[u8]) -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(tag)))
}

#[test]
fn incumbent_authority_is_the_signed_genesis_generation_bound_to_this_network() {
    let roster = roster(4);
    let world = World::new();
    let mut parameters = world.parameters.block();
    parameters.set_parameter(handshake_metadata(&roster));
    parameters.commit();
    let network = network_id_of(b"incumbent-network");
    let state = state_on(network, world);

    let (authority, authorization) = current_authority(&state.view()).unwrap();
    assert_eq!(authority.network_id, network);
    assert_eq!(authority.generation, 0);
    assert!(
        authority
            .validators
            .iter()
            .map(|keys| &keys.validator)
            .eq(roster.iter().map(|seat| &seat.validator)),
        "the signed roster keeps its order"
    );
    assert_eq!(
        authorization,
        KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, u64::MAX).unwrap(),
        "no certified successor exists, so the open-ended genesis authorization governs"
    );
    assert_eq!(authorization.beacon, BeaconEpochBindingV1::Bootstrap);

    // The same signed parameters on another chain bind that chain's network id.
    let other_world = World::new();
    let mut parameters = other_world.parameters.block();
    parameters.set_parameter(handshake_metadata(&roster));
    parameters.commit();
    let other = network_id_of(b"another-network");
    let (foreign, _) = current_authority(&state_on(other, other_world).view()).unwrap();
    assert_eq!(foreign.network_id, other);
    assert_ne!(
        foreign.authority_id().unwrap(),
        authority.authority_id().unwrap()
    );
}

#[test]
fn incumbent_authority_requires_the_signed_genesis_metadata() {
    let state = state_on(network_id_of(b"no-metadata"), World::new());
    let error = current_authority(&state.view()).unwrap_err();
    assert!(
        error.contains("signed genesis consensus metadata"),
        "{error}"
    );
}

#[test]
fn scheduling_epochs_partition_heights_by_the_committed_length() {
    for (height, expected) in [
        (1, (0, 1, 10)),
        (10, (0, 1, 10)),
        (11, (1, 11, 20)),
        (15, (1, 11, 20)),
        (20, (1, 11, 20)),
        (21, (2, 21, 30)),
    ] {
        let epoch = SchedulingEpoch::containing(height, 10).unwrap();
        assert_eq!(
            (epoch.epoch, epoch.first_height, epoch.last_height),
            expected,
            "height {height}"
        );
    }
    assert_eq!(
        SchedulingEpoch::containing(7, 1).unwrap(),
        SchedulingEpoch {
            epoch: 6,
            first_height: 7,
            last_height: 7
        }
    );
    assert!(SchedulingEpoch::containing(0, 10).is_err());
    assert!(SchedulingEpoch::containing(5, 0).is_err());
    assert!(SchedulingEpoch::containing(u64::MAX, 2).is_err());
}

#[test]
fn a_frozen_preparation_must_follow_the_current_scheduling_epoch() {
    let fixture = fixture(4);
    let preparation = &fixture.transition.preparation;
    let network = preparation.network_id;
    // Frozen at the end of epoch 0 for epoch 2, prepared during epoch 1 = [11, 20].
    let current = SchedulingEpoch::containing(15, 10).unwrap();
    current
        .validate_prepared_successor(network, preparation)
        .unwrap();
    assert!(
        current
            .validate_prepared_successor(network_id_of(b"foreign"), preparation)
            .is_err(),
        "a preparation of another network"
    );
    for other in [
        SchedulingEpoch::containing(5, 10).unwrap(),
        SchedulingEpoch::containing(25, 10).unwrap(),
        SchedulingEpoch::containing(15, 5).unwrap(),
    ] {
        assert!(
            other
                .validate_prepared_successor(network, preparation)
                .is_err(),
            "{other:?} is not the preparing epoch"
        );
    }
    let mut gap = preparation.clone();
    gap.first_height += 1;
    assert!(current.validate_prepared_successor(network, &gap).is_err());
}
