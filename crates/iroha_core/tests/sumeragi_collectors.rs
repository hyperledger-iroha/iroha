//! Integration-style unit tests for deterministic collector routing.

use iroha_config::parameters::actual::ConsensusMode;
use iroha_core::sumeragi::{
    collectors::{CollectorPlan, deterministic_collectors},
    network_topology::Topology,
};
use iroha_crypto::KeyPair;
use iroha_data_model::prelude::PeerId;

fn sample_peers(n: usize) -> Vec<PeerId> {
    (0..n)
        .map(|_| PeerId::new(KeyPair::random().public_key().clone()))
        .collect()
}

#[test]
fn permissioned_collectors_rotate_by_height_and_view() {
    let peers = sample_peers(5);
    let topology = Topology::new(peers.clone());

    let round1 = deterministic_collectors(&topology, ConsensusMode::Permissioned, 2, None, 2, 0);
    let round2 = deterministic_collectors(&topology, ConsensusMode::Permissioned, 2, None, 3, 0);
    let view_bump = deterministic_collectors(&topology, ConsensusMode::Permissioned, 2, None, 2, 1);

    assert_eq!(round1, vec![peers[2].clone(), peers[3].clone()]);
    assert_eq!(round2, vec![peers[3].clone(), peers[2].clone()]);
    assert_eq!(view_bump, vec![peers[3].clone(), peers[2].clone()]);
}

#[test]
fn collector_plan_exhaustion_triggers_gossip_once() {
    let peers = sample_peers(3);
    let mut plan = CollectorPlan::new(peers.clone());

    assert_eq!(plan.next(), Some(peers[0].clone()));
    assert_eq!(plan.next(), Some(peers[1].clone()));
    assert_eq!(plan.next(), Some(peers[2].clone()));
    assert!(plan.exhausted());
    assert!(plan.next().is_none());
    assert!(plan.trigger_gossip());
    assert!(!plan.trigger_gossip());
    assert!(plan.gossip_triggered());
}

#[test]
fn npos_collectors_reproducible_for_same_seed() {
    let peers = sample_peers(6);
    let topology = Topology::new(peers);
    let seed = [0xA5; 32];

    let first = deterministic_collectors(&topology, ConsensusMode::Npos, 3, Some(seed), 4, 2);
    let second = deterministic_collectors(&topology, ConsensusMode::Npos, 3, Some(seed), 4, 2);
    assert_eq!(first, second);
}
