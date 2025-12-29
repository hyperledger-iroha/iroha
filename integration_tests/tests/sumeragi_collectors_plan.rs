//! Cross-crate regression tests for Sumeragi collector routing fairness.

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
fn permissioned_primary_rotates_across_heights() {
    let peers = sample_peers(4);
    let topology = Topology::new(peers.clone());

    let plan_h2 = deterministic_collectors(&topology, ConsensusMode::Permissioned, 2, None, 2, 0);
    let plan_h3 = deterministic_collectors(&topology, ConsensusMode::Permissioned, 2, None, 3, 0);

    assert_ne!(
        plan_h2, plan_h3,
        "primary collector must rotate between heights"
    );
    assert_eq!(plan_h2[0], peers[2]);
    assert_eq!(plan_h3[0], peers[3]);
}

#[test]
fn collector_plan_enters_gossip_after_retries() {
    let peers = sample_peers(2);
    let mut plan = CollectorPlan::new(peers.clone());

    assert_eq!(plan.next(), Some(peers[0].clone()));
    assert_eq!(plan.next(), Some(peers[1].clone()));
    assert!(plan.exhausted());
    assert!(plan.trigger_gossip(), "gossip fallback must trigger once");
    assert!(
        !plan.trigger_gossip(),
        "gossip fallback must not trigger twice"
    );
}
