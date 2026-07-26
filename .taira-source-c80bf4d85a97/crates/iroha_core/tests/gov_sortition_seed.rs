//! Deterministic sortition seed computation for governance draws.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::governance::sortition::compute_seed;
use iroha_data_model::ChainId;

#[test]
fn sortition_seed_is_deterministic_and_domain_separated() {
    let chain: ChainId = "sora-test".into();
    let beacon = [0xAB; 32];
    let epoch = 42u64;

    let seed_a = compute_seed(&chain, epoch, &beacon, b"gov:draw:v1");
    let seed_b = compute_seed(&chain, epoch, &beacon, b"gov:draw:v1");
    assert_eq!(seed_a, seed_b);

    let seed_other_domain = compute_seed(&chain, epoch, &beacon, b"gov:draw:v2");
    assert_ne!(seed_a, seed_other_domain);
}
