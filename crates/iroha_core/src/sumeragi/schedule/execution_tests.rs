//! Reconcile independently signed genesis authority with actual executed registration rows.

use super::*;
use crate::state::{World, derive_validator_key_id};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{consensus::ConsensusKeyStatus, parameter::system::SumeragiConsensusMode};
use iroha_model_base::peer::PeerId;

fn registered_world(context: &iroha_data_model::sumeragi::epoch::ValidatorEpochContextV1) -> World {
    let mut world = World::new();
    for member in &context.committee {
        world.register_validator_pop_for_testing(
            member.validator.public_key().clone(),
            member.proof_of_possession.clone(),
        );
    }
    {
        let mut overlay = world.block();
        let mut peers = overlay.peers_mut_for_testing().transaction();
        for member in &context.committee {
            peers.push(member.validator.clone());
        }
        peers.apply();
        overlay.commit();
    }
    world
}

#[test]
fn executed_genesis_must_retain_every_exact_signed_registration() {
    let signed = crate::sumeragi::epoch::tests::genesis_fixture(
        SumeragiConsensusMode::Permissioned,
        10,
        false,
    );
    let context = crate::sumeragi::epoch::genesis_epoch(&signed).unwrap();
    let mut world = registered_world(&context);
    validate_executed_genesis(&world.view(), &context).unwrap();
    let id = derive_validator_key_id(context.committee[0].validator.public_key());
    let original = world.consensus_keys.view().get(&id).unwrap().clone();
    for mutation in 0..3 {
        let mut bad = original.clone();
        match mutation {
            0 => bad.status = ConsensusKeyStatus::Disabled,
            1 => bad.pop.as_mut().unwrap()[0] ^= 1,
            _ => bad.activation_height = 2,
        }
        world.consensus_keys.insert(id.clone(), bad);
        assert!(validate_executed_genesis(&world.view(), &context).is_err());
    }
    world.consensus_keys.insert(id, original);
    validate_executed_genesis(&world.view(), &context).unwrap();
}

#[test]
fn executed_genesis_rejects_an_extra_voting_registration() {
    let signed = crate::sumeragi::epoch::tests::genesis_fixture(
        SumeragiConsensusMode::Permissioned,
        10,
        false,
    );
    let context = crate::sumeragi::epoch::genesis_epoch(&signed).unwrap();
    let mut world = registered_world(&context);
    let pair = KeyPair::from_seed(vec![0xa7; 32], Algorithm::BlsNormal);
    world.register_validator_pop_for_testing(
        pair.public_key().clone(),
        iroha_crypto::bls_normal_pop_prove(pair.private_key()).unwrap(),
    );
    {
        let mut overlay = world.block();
        let mut peers = overlay.peers_mut_for_testing().transaction();
        peers.push(PeerId::new(pair.public_key().clone()));
        peers.apply();
        overlay.commit();
    }
    assert!(validate_executed_genesis(&world.view(), &context).is_err());
}
