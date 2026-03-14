#![doc = "Backend tag acceptance tests for ZK attachments (pre-verify path)."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
#![cfg(feature = "zk-preverify")]
//! Backend tag acceptance tests for ZK attachments (pre-verify path).
//! - Unknown families (e.g., `groth16/*`) are rejected as unsupported.
//! - Halo2 curve mismatch is rejected as "curve not allowed".

use iroha_core::{
    executor::Executor, kura::Kura, query::store::LiveQueryStore, state::State,
    zk::test_utils::halo2_fixture_envelope,
};
use iroha_data_model::{ValidationFail, prelude::*, transaction::signed::TransactionBuilder};
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;

#[path = "common/world_fixture.rs"]
mod test_world;

fn new_block_ctx() -> (State, iroha_data_model::block::BlockHeader) {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    (state, header)
}

#[test]
fn groth16_backend_tag_is_unsupported() {
    let (state, header) = new_block_ctx();
    let mut block = state.block(header);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let exec = Executor::default();

    // Build a transaction with proofs carrying one inline Groth16 attachment
    let chain: ChainId = "test-chain".parse().unwrap();
    let authority = ALICE_ID.clone();
    let private_key = iroha_test_samples::ALICE_KEYPAIR.private_key().clone();
    let attach = iroha_data_model::proof::ProofAttachment::new_inline(
        "groth16/bn254".into(),
        iroha_data_model::proof::ProofBox::new("groth16/bn254".into(), vec![1, 2, 3, 4]),
        iroha_data_model::proof::VerifyingKeyBox::new("groth16/bn254".into(), vec![7, 7, 7]),
    );
    let tx: SignedTransaction = TransactionBuilder::new(chain, authority.clone())
        .with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ))
        .with_attachments(iroha_data_model::proof::ProofAttachmentList(vec![attach]))
        .sign(&private_key);

    let mut stx = block.transaction();
    let err = exec
        .execute_transaction(&mut stx, &authority, tx, &mut ivm_cache)
        .expect_err("unsupported backend should be rejected at pre-verify");
    match err {
        ValidationFail::NotPermitted(msg) => {
            assert!(msg.contains("unsupported proof backend"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn halo2_curve_mismatch_rejected_as_curve_not_allowed() {
    let (state, header) = new_block_ctx();
    let mut block = state.block(header);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let exec = Executor::default();

    // Node default config curve is Pallas (see state defaults).
    // Attach a Halo2 Pasta backend proof to trigger curve mismatch.
    let chain: ChainId = "test-chain".parse().unwrap();
    let authority = ALICE_ID.clone();
    let private_key = iroha_test_samples::ALICE_KEYPAIR.private_key().clone();
    let halo2_fixture = halo2_fixture_envelope("halo2/pasta/ipa/tiny-add-v1", [0u8; 32]);
    let vk_box = halo2_fixture
        .vk_box("halo2/pasta/ipa-v1")
        .expect("fixture verifying key");
    let attach = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/pasta/ipa-v1".into(),
        halo2_fixture.proof_box("halo2/pasta/ipa-v1"),
        vk_box,
    );
    let tx: SignedTransaction = TransactionBuilder::new(chain, authority.clone())
        .with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ))
        .with_attachments(iroha_data_model::proof::ProofAttachmentList(vec![attach]))
        .sign(&private_key);

    let mut stx = block.transaction();
    let err = exec
        .execute_transaction(&mut stx, &authority, tx, &mut ivm_cache)
        .expect_err("curve mismatch should be rejected");
    match err {
        ValidationFail::NotPermitted(msg) => {
            assert!(msg.contains("curve not allowed"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
