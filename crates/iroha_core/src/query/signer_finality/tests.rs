//! Native State/Kura association over a real certified chain: a four-validator committee with
//! real BLS signatures certifies every block (`sumeragi::test_chain`).

use std::sync::Arc;

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{NetworkId, block::BlockHeader};

use super::{SignerFinalityErrorV1, verify_signer_finality_v1};
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
    sumeragi::{
        certified_chain::committed_block,
        test_chain::{CertifiedTestChain, Signers, TestChainConfig},
    },
};

fn chain() -> CertifiedTestChain {
    let mut chain =
        CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).expect("chain");
    chain.commit_at(2_000, Vec::new());
    chain.commit_at(3_000, Vec::new());
    chain
}

fn hash(chain: &CertifiedTestChain, height: u64) -> [u8; 32] {
    *chain.committed(height).block_hash().as_ref()
}

#[test]
fn invalid_or_absent_coordinates_fail_closed() {
    let chain = chain();
    let view = chain.state().view();
    for (height, hash) in [
        (0, hash(&chain, 1)),
        (2, [0; 32]),
        (2, [1; 32]),
        (4, hash(&chain, 3)),
        (u64::MAX, [1; 32]),
    ] {
        assert_eq!(
            verify_signer_finality_v1(&view, height, hash),
            Err(SignerFinalityErrorV1)
        );
    }
    assert_eq!(
        SignerFinalityErrorV1.to_string(),
        "signer finality is unavailable for the exact native block"
    );
}

#[test]
fn every_certified_height_is_final_with_its_certified_block_id() {
    let chain = chain();
    let view = chain.state().view();
    for height in 1..=3 {
        let verified = verify_signer_finality_v1(&view, height, hash(&chain, height))
            .expect("certified block");
        assert_eq!(verified.height(), height);
        assert_eq!(verified.block_hash(), hash(&chain, height));
        assert_eq!(verified.context_id(), chain.committed(height).id());
    }
    // Another height's hash is not this height's block.
    assert_eq!(
        verify_signer_finality_v1(&view, 2, hash(&chain, 3)),
        Err(SignerFinalityErrorV1)
    );
}

#[test]
fn native_hash_cache_without_a_durable_block_is_not_finality() {
    let chain = chain();
    let mut state = State::new_with_chain_and_network_id_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        "sumeragi-certified-test-chain".parse().unwrap(),
        chain.network_id(),
    );
    state.push_block_hash_for_testing(chain.genesis().hash());
    assert_eq!(
        verify_signer_finality_v1(&state.view(), 1, hash(&chain, 1)),
        Err(SignerFinalityErrorV1)
    );
}

#[test]
fn durable_certificate_cannot_substitute_for_the_same_state_history() {
    let chain = chain();
    // The same Kura behind a State whose journal names another block at height 2.
    let mut other = State::new_with_chain_and_network_id_for_testing(
        World::default(),
        Arc::clone(chain.kura()),
        LiveQueryStore::start_test(),
        "sumeragi-certified-test-chain".parse().unwrap(),
        chain.network_id(),
    );
    other.push_block_hash_for_testing(chain.genesis().hash());
    let foreign = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"other native view"));
    other.push_block_hash_for_testing(foreign);
    verify_signer_finality_v1(&other.view(), 1, hash(&chain, 1)).expect("shared genesis");
    assert_eq!(
        verify_signer_finality_v1(&other.view(), 2, hash(&chain, 2)),
        Err(SignerFinalityErrorV1)
    );
    assert_eq!(
        verify_signer_finality_v1(&other.view(), 2, *foreign.as_ref()),
        Err(SignerFinalityErrorV1)
    );
}

#[test]
fn identical_blocks_and_certificates_cannot_authorize_a_foreign_state_network() {
    let chain = chain();
    let foreign_network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"different State network with identical retained blocks and certificates",
    )));
    // The same frames, certificates included, in the Kura of a State of another network.
    let kura = Kura::blank_kura_for_testing();
    let mut foreign = State::new_with_chain_and_network_id_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        "sumeragi-certified-test-chain".parse().unwrap(),
        foreign_network,
    );
    for height in 1..=3 {
        let block = chain.committed(height).block().clone();
        assert!(block.commit_certificate().is_some());
        kura.store_block(Arc::clone(&block)).unwrap();
        foreign.push_block_hash_for_testing(block.hash());
    }
    assert!(
        foreign
            .view()
            .block_hashes()
            .iter()
            .eq(chain.state().view().block_hashes().iter()),
        "both States retain the same ordered block history"
    );
    for height in 1..=3 {
        assert_eq!(
            verify_signer_finality_v1(&foreign.view(), height, hash(&chain, height)),
            Err(SignerFinalityErrorV1)
        );
    }
}

/// A block whose local `CommitQC` does not verify (two of four signers) is committed, but not
/// signer-final; the consensus-visible read of the same block is unaffected by the certificate.
#[test]
fn an_invalid_local_certificate_is_not_signer_finality() {
    let mut chain = chain();
    chain.commit_with(Some(4_000), Vec::new(), Signers::BelowQuorum);
    let view = chain.state().view();
    assert_eq!(
        verify_signer_finality_v1(&view, 4, hash(&chain, 4)),
        Err(SignerFinalityErrorV1)
    );
    verify_signer_finality_v1(&view, 3, hash(&chain, 3)).expect("earlier heights stay final");
    let committed = committed_block(&view, 4).expect("committed");
    assert_eq!(*committed.block_hash().as_ref(), hash(&chain, 4));
}
