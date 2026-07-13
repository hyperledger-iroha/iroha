#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Cross-crate regression tests for the canonical Sumeragi v2 leader schedule.

use iroha_core::sumeragi::network_topology::Topology;
use iroha_crypto::KeyPair;
use iroha_data_model::prelude::PeerId;

fn sample_peers(n: usize) -> Vec<PeerId> {
    (0..n)
        .map(|_| {
            PeerId::new(
                KeyPair::try_random()
                    .expect("generate checked collector-plan peer keypair")
                    .public_key()
                    .clone(),
            )
        })
        .collect()
}

#[test]
fn v2_prf_leader_schedule_is_deterministic_and_cycles_the_roster() {
    let topology = Topology::new(sample_peers(4));
    let seed = [0x5A; 32];
    let height = 7;

    let first_cycle = (0..4)
        .map(|view| topology.leader_index_prf(seed, height, view))
        .collect::<Vec<_>>();
    let repeated_cycle = (0..4)
        .map(|view| topology.leader_index_prf(seed, height, view))
        .collect::<Vec<_>>();

    assert_eq!(first_cycle, repeated_cycle);
    let mut sorted = first_cycle;
    sorted.sort_unstable();
    assert_eq!(sorted, vec![0, 1, 2, 3]);
}

#[test]
fn v2_quorums_match_the_four_validator_bft_roster() {
    let topology = Topology::new(sample_peers(4));

    assert_eq!(topology.min_votes_for_commit(), 3);
    assert_eq!(topology.min_votes_for_view_change(), 2);
}
