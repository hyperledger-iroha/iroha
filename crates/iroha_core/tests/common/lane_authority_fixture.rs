//! Exact four-validator lane authority for block admission fixtures.

use iroha_core::{
    governance::manifest::{
        GovernanceRules, LaneManifestRegistry, LaneManifestStatus, ManifestValidatorBinding,
    },
    state::{State, World},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{AccountId, PeerId, nexus::LaneId};
use std::{collections::BTreeMap, sync::Arc};

fn keypairs() -> Vec<KeyPair> {
    let mut keys = (0xA0_u8..0xA4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS lane authority")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    keys
}

/// Return the active signer at the first position of the canonical roster.
pub(crate) fn leader() -> KeyPair {
    keypairs().remove(0)
}

/// Seed registered peers and their exact active consensus-key proofs.
pub(crate) fn seed_world(world: &mut World) {
    let keys = keypairs();
    for key in &keys {
        world.register_validator_pop_for_testing(
            key.public_key().clone(),
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("validator proof of possession"),
        );
    }
    let mut block = world.block();
    let mut peers = block.peers_mut_for_testing().transaction();
    peers.extend(keys.iter().map(|key| PeerId::new(key.public_key().clone())));
    peers.apply();
    block.commit();
}

/// Install the same exact validator bindings on the configured primary lane.
pub(crate) fn install_manifest(state: &State) {
    let nexus = state.nexus_snapshot();
    let lane = nexus
        .lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == LaneId::SINGLE)
        .expect("physical primary lane");
    let bindings = peers()
        .into_iter()
        .map(|peer_id| ManifestValidatorBinding {
            validator: AccountId::of(peer_id.public_key().clone()),
            peer_id,
            torii_url: None,
        })
        .collect::<Vec<_>>();
    let rules = GovernanceRules {
        validators: bindings
            .iter()
            .map(|binding| binding.validator.clone())
            .collect(),
        validator_bindings: bindings,
        ..GovernanceRules::default()
    };
    state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(
        BTreeMap::from([(
            lane.id,
            LaneManifestStatus {
                lane: lane.id,
                alias: lane.alias.clone(),
                dataspace: lane.dataspace_id,
                visibility: lane.visibility,
                storage: lane.storage,
                governance: Some("parliament".to_owned()),
                manifest_path: Some("/test-fixtures/lane-authority.json".into()),
                governance_rules: Some(rules),
                privacy_commitments: Vec::new(),
            },
        )]),
    )));
}

/// Return the canonical roster bound into each lane payload ownership.
pub(crate) fn peers() -> Vec<PeerId> {
    keypairs()
        .into_iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect()
}
