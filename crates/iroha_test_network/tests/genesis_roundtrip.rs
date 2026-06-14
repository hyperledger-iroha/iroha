use core::iter::FromIterator;
use std::collections::HashSet;

use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{block::decode_framed_signed_block, peer::PeerId};
use iroha_primitives::unique_vec::UniqueVec;
use iroha_test_network::{genesis_factory, init_instruction_registry};

fn checked_bls_fixture_keypair() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
        .expect("checked genesis roundtrip BLS fixture key generation")
}

#[test]
fn genesis_roundtrip_fixture_uses_checked_bls_key_generation() {
    let bls = checked_bls_fixture_keypair();

    assert_eq!(
        bls.public_key()
            .try_algorithm()
            .expect("checked BLS fixture public-key algorithm"),
        Algorithm::BlsNormal,
    );
}

#[test]
fn genesis_roundtrip_decode() {
    init_instruction_registry();
    let bls = checked_bls_fixture_keypair();
    let peer = PeerId::new(bls.public_key().clone());
    let topology = UniqueVec::from_iter([peer]);
    let entry = iroha_genesis::GenesisTopologyEntry::new(
        PeerId::new(bls.public_key().clone()),
        iroha_crypto::bls_normal_pop_prove(bls.private_key()).expect("BLS PoP generation"),
    );
    let genesis = genesis_factory(Vec::new(), topology, vec![entry]);
    let wire = genesis
        .0
        .encode_wire()
        .unwrap_or_else(|err| panic!("encode genesis wire: {err:?}"));
    decode_framed_signed_block(&wire).unwrap_or_else(|err| panic!("decode genesis: {err:?}"));
}

#[test]
fn genesis_transactions_are_unique() {
    init_instruction_registry();
    let genesis = genesis_factory(Vec::new(), UniqueVec::new(), Vec::new());
    let mut seen = HashSet::new();
    for tx in genesis.0.external_transactions() {
        let hash = tx.hash();
        assert!(
            seen.insert(hash),
            "duplicate transaction detected in default genesis: {hash:?}"
        );
    }
}
