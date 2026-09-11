//! Integration checks for BLS batching + `PoP` gating on transaction admission.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#[path = "common/lane_authority_fixture.rs"]
mod lane_authority_fixture;
use core::time::Duration;
use iroha_core::{
    block::{BlockValidationError, ValidBlock},
    da::proof_policy_bundle,
    kura::Kura,
    prelude::*,
    query::store::LiveQueryStore,
    state::{State, StateReadOnly},
    sumeragi::network_topology::Topology,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    NetworkId, Registrable,
    block::{
        BlockExecutionContextBundle, ExternalExecutionContext, builder::BlockBuilder,
        consensus::SumeragiLanePayloadOwnership,
    },
    prelude::{
        Account, AccountId, AssetDefinition, BlockHeader, Domain, HashOf, Level, Log,
        SignedTransaction, TransactionBuilder,
    },
};
use iroha_model_base::chain::ChainId;
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use iroha_primitives::time::TimeSource;
use nonzero_ext::nonzero;
use std::sync::Arc;
fn checked_random_bls_batch_keypair() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
        .expect("generate checked BLS batch keypair")
}
#[test]
fn bls_batch_fixture_uses_checked_bls_randomness() {
    let key_pair = checked_random_bls_batch_keypair();
    assert_eq!(key_pair.public_key().algorithm(), Algorithm::BlsNormal);
}
fn mk_state_with_bls_batch() -> (State, NetworkId, AccountId, KeyPair) {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    // Seed world with an account
    let kp = checked_random_bls_batch_keypair();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let account_id = AccountId::of(kp.public_key().clone());
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let mut world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    lane_authority_fixture::seed_world(&mut world);
    let mut state =
        State::new_with_chain_for_testing(world, kura, query_handle, ChainId::from("chain"));
    let network_id = *state.network_id_ref();
    lane_authority_fixture::install_manifest(&state);
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
    (state, network_id, account_id, kp)
}
fn seed_genesis(state: &State) -> (HashOf<BlockHeader>, KeyPair) {
    let kp = lane_authority_fixture::leader();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut builder = BlockBuilder::new(header);
    let proof_policies = proof_policy_bundle(&state.view().nexus().lane_config);
    builder.set_da_proof_policies(Some(proof_policies));
    let block = builder
        .build_with_signature(0, kp.private_key())
        .canonical_resultless_proposal();
    let mut state_block = state.block(block.header());
    let valid = ValidBlock::validate_unchecked(block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let _ = state_block.apply_without_execution(&committed, lane_authority_fixture::peers());
    state_block
        .kura()
        .store_block(Arc::new(committed.clone().into()))
        .expect("store genesis");
    state_block.commit().expect("genesis commit");
    (committed.as_ref().hash(), kp)
}
fn make_tx(
    network_id: &NetworkId,
    authority: &AccountId,
    kp: &KeyPair,
    with_pop: bool,
) -> SignedTransaction {
    let mut builder = TransactionBuilder::new(
        *network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
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
fn push_single_tx_with_context(
    builder: &mut BlockBuilder,
    tx: SignedTransaction,
    state: &State,
    height: std::num::NonZeroU64,
) {
    let execution_context = BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
        tx.hash_as_entrypoint(),
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    )]);
    let lane_incarnation = state
        .view()
        .lane_incarnation_at_height(LaneId::SINGLE, height.get())
        .expect("single-lane incarnation must be active at the block height");
    let mut ownership = SumeragiLanePayloadOwnership {
        proposal_height: height.get(),
        proposal_view: 0,
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
        lane_incarnation,
        lane_block_height: 1,
        lane_block_view: 0,
        subject_hash: Hash::prehashed([0; Hash::LENGTH]),
        qc_mode_tag: "permissioned:bls-batch-pop-test".to_owned(),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![Hash::from(tx.hash_as_entrypoint())],
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_descriptor_hash: Some(Hash::prehashed([0; Hash::LENGTH])),
        lane_block_descriptor_validator_set: lane_authority_fixture::peers(),
        lane_block_descriptor_validator_count: 4,
        lane_block_descriptor_min_quorum: 3,
        payload_ownership_hash: Hash::prehashed([0; Hash::LENGTH]),
        rbc_instance_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    let replay_hashes = ownership
        .compute_replay_hashes()
        .expect("BLS batch ownership replay hashes must compute");
    ownership.subject_hash = replay_hashes.subject_hash;
    ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
    ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
    builder.set_execution_context(Some(
        execution_context.with_lane_payload_ownerships(vec![ownership]),
    ));
    builder.push_transaction(tx);
}
#[test]
fn bls_batch_block_validates_with_pop() {
    let (state, network_id, account, kp) = mk_state_with_bls_batch();
    let (genesis_hash, peer_kp) = seed_genesis(&state);
    let tx = make_tx(&network_id, &account, &kp, true);
    let height = nonzero!(2_u64);
    let header = BlockHeader::new(height, Some(genesis_hash), None, None, 1, 0);
    let mut builder = BlockBuilder::new(header);
    push_single_tx_with_context(&mut builder, tx, &state, height);
    let proof_policies = proof_policy_bundle(&state.view().nexus().lane_config);
    builder.set_da_proof_policies(Some(proof_policies));
    let block = builder
        .build_with_signature(0, peer_kp.private_key())
        .canonical_resultless_proposal();
    let mut state_block = state.block(block.header());
    let topology = Topology::new(lane_authority_fixture::peers());
    ValidBlock::validate_sumeragi_v2_fixture(
        block,
        &topology,
        &account,
        &TimeSource::new_system(),
        &mut state_block,
    )
    .unpack(|_| {})
    .expect("block validation must succeed with PoP");
}
#[test]
fn bls_batch_block_validates_without_pop_fallback() {
    let (state, network_id, account, kp) = mk_state_with_bls_batch();
    let (genesis_hash, peer_kp) = seed_genesis(&state);
    let tx = make_tx(&network_id, &account, &kp, false);
    let height = nonzero!(2_u64);
    let header = BlockHeader::new(height, Some(genesis_hash), None, None, 1, 0);
    let mut builder = BlockBuilder::new(header);
    push_single_tx_with_context(&mut builder, tx, &state, height);
    let proof_policies = proof_policy_bundle(&state.view().nexus().lane_config);
    builder.set_da_proof_policies(Some(proof_policies));
    let block = builder
        .build_with_signature(0, peer_kp.private_key())
        .canonical_resultless_proposal();
    let mut state_block = state.block(block.header());
    let topology = Topology::new(lane_authority_fixture::peers());
    // Should still validate via per-signature path when PoP is absent.
    ValidBlock::validate_sumeragi_v2_fixture(
        block,
        &topology,
        &account,
        &TimeSource::new_system(),
        &mut state_block,
    )
    .unpack(|_| {})
    .expect("block validation must succeed without PoP (per-signature fallback)");
}
#[test]
fn bls_batch_block_rejects_missing_proof_policies() {
    let (state, network_id, account, kp) = mk_state_with_bls_batch();
    let (genesis_hash, peer_kp) = seed_genesis(&state);
    let tx = make_tx(&network_id, &account, &kp, true);
    let height = nonzero!(2_u64);
    let header = BlockHeader::new(height, Some(genesis_hash), None, None, 1, 0);
    let mut builder = BlockBuilder::new(header);
    push_single_tx_with_context(&mut builder, tx, &state, height);
    let block = builder
        .build_with_signature(0, peer_kp.private_key())
        .canonical_resultless_proposal();
    let mut state_block = state.block(block.header());
    let topology = Topology::new(lane_authority_fixture::peers());
    let err = ValidBlock::validate_sumeragi_v2_fixture(
        block,
        &topology,
        &account,
        &TimeSource::new_system(),
        &mut state_block,
    )
    .unpack(|_| {})
    .expect_err("block validation must reject missing mandatory DA proof policies");
    assert!(
        matches!(
            *err.1,
            BlockValidationError::DaProofPolicySidecarHashMismatch {
                expected: None,
                actual: None,
            }
        ),
        "missing mandatory DA proof policies must reject before signature batching: {:?}",
        err.1
    );
}
