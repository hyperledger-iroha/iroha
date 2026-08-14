use core::iter::FromIterator;
use std::{collections::HashSet, time::Duration};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    block::decode_framed_signed_block,
    parameter::BlockParameter,
    peer::PeerId,
    prelude::{Parameter, SetParameter},
};
use iroha_primitives::unique_vec::UniqueVec;
use iroha_test_network::{NetworkBuilder, genesis_factory, init_instruction_registry};
use nonzero_ext::nonzero;
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
fn network_genesis_roundtrip_preserves_signed_header_hash() {
    init_instruction_registry();
    let network = NetworkBuilder::new()
        .with_auto_populated_trusted_peers()
        .with_peers(4)
        .with_default_block_cadence()
        .with_block_sync_gossip_period(Duration::from_millis(400))
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_base_seed("genesis-wire-feature-parity")
        .build();
    let genesis = network.genesis();
    let wire = genesis
        .0
        .encode_wire()
        .unwrap_or_else(|err| panic!("encode network genesis wire: {err:?}"));
    let decoded = decode_framed_signed_block(&wire)
        .unwrap_or_else(|err| panic!("decode network genesis: {err:?}"));
    assert_eq!(
        decoded.header(),
        genesis.0.header(),
        "canonical genesis wire must preserve the exact signed header",
    );
    assert_eq!(
        decoded.hash(),
        genesis.0.hash(),
        "canonical genesis wire must preserve the configured trust-anchor hash",
    );
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
