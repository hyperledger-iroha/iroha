#![doc = "Pre-verify deduplication tests for ZK attachments."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
//! Unit tests for pre-verify dedup logic with optional `vk_commitment`.
#![cfg(feature = "zk-preverify")]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::{block::BlockHeader, proof::ProofBox};
use nonzero_ext::nonzero;

#[test]
fn dedup_allows_same_proof_with_different_commitments() {
    // Build minimal state and block context
    let state = State::new(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3, 4]);

    // No commitment: first pass accepted, second duplicate
    let r1 = stx.preverify_proof(&proof, None, 100_000, None, None, true);
    assert!(matches!(r1, iroha_core::zk::PreverifyResult::Accepted));
    let r1_dup = stx.preverify_proof(&proof, None, 100_000, None, None, true);
    assert!(matches!(r1_dup, iroha_core::zk::PreverifyResult::Duplicate));

    // Different commitment should make the same proof distinct
    let c1 = [0x11u8; 32];
    let r2 = stx.preverify_proof(&proof, None, 100_000, Some(c1), None, true);
    assert!(matches!(r2, iroha_core::zk::PreverifyResult::Accepted));
    let r2_dup = stx.preverify_proof(&proof, None, 100_000, Some(c1), None, true);
    assert!(matches!(r2_dup, iroha_core::zk::PreverifyResult::Duplicate));

    let c2 = [0x22u8; 32];
    let r3 = stx.preverify_proof(&proof, None, 100_000, Some(c2), None, true);
    assert!(matches!(r3, iroha_core::zk::PreverifyResult::Accepted));
}
