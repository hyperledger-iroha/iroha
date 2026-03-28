//! Integration checks for BLS batching + `PoP` gating on transaction admission.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use core::time::Duration;
use std::sync::Arc;

#[cfg(feature = "telemetry")]
use iroha_core::telemetry::StateTelemetry;
use iroha_core::{
    block::{BlockValidationError, ValidBlock},
    da::proof_policy_bundle,
    kura::Kura,
    prelude::*,
    query::store::LiveQueryStore,
    state::State,
    sumeragi::network_topology::Topology,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    ChainId, Metadata, PeerId, Registrable,
    block::builder::BlockBuilder,
    prelude::{
        Account, AccountId, AssetDefinition, BlockHeader, Domain, DomainId, HashOf, Level, Log,
        SignedTransaction, TransactionBuilder,
    },
};
use iroha_primitives::time::TimeSource;
use nonzero_ext::nonzero;

fn mk_state_with_bls_batch() -> (State, ChainId, AccountId, KeyPair) {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    // Seed world with an account
    let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let domain_id: DomainId = "wonderland".parse().unwrap();
    let account_id = AccountId::of(kp.public_key().clone());
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new_in_domain(account_id.clone(), domain_id).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    #[cfg(feature = "telemetry")]
    let mut state = State::new(world, kura, query_handle, StateTelemetry::default());
    #[cfg(not(feature = "telemetry"))]
    let mut state = State::new(world, kura, query_handle);
    let mut pipeline = state.view().pipeline().clone();
    pipeline.signature_batch_max_bls = 4;
    state.set_pipeline(pipeline);
    let mut crypto_cfg = iroha_config::parameters::actual::Crypto::default();
    if !crypto_cfg.allowed_signing.contains(&Algorithm::BlsNormal) {
        crypto_cfg.allowed_signing.push(Algorithm::BlsNormal);
        crypto_cfg.allowed_signing.sort();
        crypto_cfg.allowed_signing.dedup();
    }
    state.set_crypto(crypto_cfg);
    let chain = ChainId::from("chain");
    (state, chain, account_id, kp)
}

fn seed_genesis(state: &State) -> (HashOf<BlockHeader>, KeyPair, PeerId) {
    let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let peer = PeerId::from(kp.public_key().clone());
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut builder = BlockBuilder::new(header);
    let proof_policies = proof_policy_bundle(&state.view().nexus().lane_config);
    builder.set_da_proof_policies(Some(proof_policies));
    let block = builder.build_with_signature(0, kp.private_key());
    let mut state_block = state.block(block.header());
    let valid = ValidBlock::validate_unchecked(block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let _ = state_block.apply_without_execution(&committed, vec![peer.clone()]);
    state_block
        .kura()
        .store_block(Arc::new(committed.clone().into()))
        .expect("store genesis");
    state_block.commit().expect("genesis commit");
    (committed.as_ref().hash(), kp, peer)
}

fn make_tx(
    chain: &ChainId,
    authority: &AccountId,
    kp: &KeyPair,
    with_pop: bool,
) -> SignedTransaction {
    let mut builder = TransactionBuilder::new(chain.clone(), authority.clone())
        .with_instructions([Log::new(Level::INFO, "msg".to_string())]);
    if with_pop {
        let pop = iroha_crypto::bls_normal_pop_prove(kp.private_key()).expect("pop");
        let mut meta = Metadata::default();
        meta.insert(
            "bls_pop".parse().unwrap(),
            iroha_primitives::json::Json::new(hex::encode_upper(pop)),
        );
        builder = builder.with_metadata(meta);
    }
    // Use a creation timestamp earlier than the block to satisfy future-time checks in validation.
    builder.set_creation_time(Duration::ZERO);
    builder.sign(kp.private_key())
}

#[test]
fn bls_batch_block_validates_with_pop() {
    let (state, chain, account, kp) = mk_state_with_bls_batch();
    let (genesis_hash, peer_kp, peer) = seed_genesis(&state);
    let tx = make_tx(&chain, &account, &kp, true);
    let header = BlockHeader::new(nonzero!(2_u64), Some(genesis_hash), None, None, 1, 0);
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(tx);
    let proof_policies = proof_policy_bundle(&state.view().nexus().lane_config);
    builder.set_da_proof_policies(Some(proof_policies));
    let block = builder.build_with_signature(0, peer_kp.private_key());

    let mut state_block = state.block(block.header());
    let topology = Topology::new(vec![peer]);
    ValidBlock::validate(
        block,
        &topology,
        &chain,
        &account,
        &TimeSource::new_system(),
        &mut state_block,
    )
    .unpack(|_| {})
    .expect("block validation must succeed with PoP");
}

#[test]
fn bls_batch_block_validates_without_pop_fallback() {
    let (state, chain, account, kp) = mk_state_with_bls_batch();
    let (genesis_hash, peer_kp, peer) = seed_genesis(&state);
    let tx = make_tx(&chain, &account, &kp, false);
    let header = BlockHeader::new(nonzero!(2_u64), Some(genesis_hash), None, None, 1, 0);
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(tx);
    let proof_policies = proof_policy_bundle(&state.view().nexus().lane_config);
    builder.set_da_proof_policies(Some(proof_policies));
    let block = builder.build_with_signature(0, peer_kp.private_key());

    let mut state_block = state.block(block.header());
    let topology = Topology::new(vec![peer]);
    // Should still validate via per-signature path when PoP is absent.
    ValidBlock::validate(
        block,
        &topology,
        &chain,
        &account,
        &TimeSource::new_system(),
        &mut state_block,
    )
    .unpack(|_| {})
    .expect("block validation must succeed without PoP (per-signature fallback)");
}

#[test]
fn bls_batch_block_rejects_missing_proof_policy_hash() {
    let (state, chain, account, kp) = mk_state_with_bls_batch();
    let (genesis_hash, peer_kp, peer) = seed_genesis(&state);
    let tx = make_tx(&chain, &account, &kp, true);
    let header = BlockHeader::new(nonzero!(2_u64), Some(genesis_hash), None, None, 1, 0);
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(tx);
    let block = builder.build_with_signature(0, peer_kp.private_key());

    let mut state_block = state.block(block.header());
    let topology = Topology::new(vec![peer]);
    let err = ValidBlock::validate(
        block,
        &topology,
        &chain,
        &account,
        &TimeSource::new_system(),
        &mut state_block,
    )
    .unpack(|_| {})
    .expect_err("block validation must reject missing DA proof policy hash");
    assert!(matches!(
        *err.1,
        BlockValidationError::ProofPolicyHashMismatch { .. }
    ));
}
