//! Bridge proof submission and retention tests.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    executor::Executor,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, WorldReadOnly},
    telemetry::StateTelemetry,
};
use iroha_data_model::{
    prelude::*,
    proof::{ProofId, ProofStatus},
};
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
use sha2::{Digest as _, Sha256};

fn bridge_proof_id(proof: &BridgeProof) -> ProofId {
    let encoded = norito::to_bytes(proof).expect("encode bridge proof");
    let backend = proof.backend_label();
    let mut h = Sha256::new();
    h.update(backend.as_bytes());
    h.update(&encoded);
    ProofId {
        backend,
        proof_hash: h.finalize().into(),
    }
}

fn make_ics_proof(leaf_fill: u8, range: (u64, u64), pinned: bool) -> BridgeProof {
    let leaves = vec![[leaf_fill; 32], [leaf_fill.wrapping_add(1); 32]];
    let tree = iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.clone());
    let root_bytes: [u8; 32] = *tree.root().expect("root").as_ref();
    let proof = tree.get_proof(0).expect("proof");

    BridgeProof {
        range: BridgeProofRange {
            start_height: range.0,
            end_height: range.1,
        },
        manifest_hash: [0xAA; 32],
        payload: BridgeProofPayload::Ics(BridgeIcsProof {
            state_root: root_bytes,
            leaf_hash: leaves[0],
            proof,
            hash_function: BridgeHashFunction::Sha256,
        }),
        pinned,
    }
}

#[test]
fn submit_bridge_proof_records_metadata() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let proof = make_ics_proof(0x11, (1, 1), false);
    let expected_id = bridge_proof_id(&proof);
    let encoded_len = u32::try_from(norito::to_bytes(&proof).expect("encode proof").len())
        .expect("bridge proof length fits in u32");

    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect("bridge proof accepted");

    let rec = stx
        .world
        .proofs()
        .get(&expected_id)
        .expect("proof recorded");
    assert_eq!(rec.status, ProofStatus::Verified);
    let bridge = rec.bridge.as_ref().expect("bridge metadata stored");
    assert_eq!(bridge.commitment, expected_id.proof_hash);
    assert_eq!(bridge.size_bytes, encoded_len);
    assert_eq!(bridge.proof.range.start_height, 1);
    assert_eq!(bridge.proof.range.end_height, 1);
}

#[test]
fn bridge_retention_prunes_oldest_unpinned() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);

    state.zk.proof_history_cap = 1;
    state.zk.proof_retention_grace_blocks = 0;
    state.zk.proof_prune_batch = 10;

    let exec = Executor::default();

    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();
    let proof1 = make_ics_proof(0x21, (1, 1), false);
    let id1 = bridge_proof_id(&proof1);
    let submit1: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof1).into();
    exec.execute_instruction(&mut stx1, &ALICE_ID.clone(), submit1)
        .expect("first proof accepted");
    stx1.apply();
    block1
        .commit()
        .expect("commit first bridge-proof block snapshot");

    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let proof2 = make_ics_proof(0x33, (2, 2), false);
    let id2 = bridge_proof_id(&proof2);
    let submit2: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof2).into();
    exec.execute_instruction(&mut stx2, &ALICE_ID.clone(), submit2)
        .expect("second proof accepted");

    assert!(stx2.world.proofs().get(&id2).is_some());
    assert!(
        stx2.world.proofs().get(&id1).is_none(),
        "older unpinned proof should be pruned when cap is hit"
    );
}

#[test]
fn bridge_range_length_cap_enforced() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);

    state.zk.bridge_proof_max_range_len = 2;

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_ics_proof(0x44, (5, 10), false);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("range cap should reject long bridge proofs");
    assert!(
        format!("{err:?}").contains("range too large"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn bridge_height_window_respected() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();

    state.zk.bridge_proof_max_future_drift_blocks = 1;
    let header_future =
        iroha_data_model::block::BlockHeader::new(nonzero!(5_u64), None, None, None, 0, 0);
    let mut block_future = state.block(header_future);
    let mut stx_future = block_future.transaction();
    let future_proof = make_ics_proof(0x55, (7, 7), false);
    let submit_future: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(future_proof).into();
    let err = exec
        .execute_instruction(&mut stx_future, &ALICE_ID.clone(), submit_future)
        .expect_err("future drift guard should reject proof ahead of window");
    assert!(
        format!("{err:?}").contains("future window"),
        "unexpected error for future drift: {err:?}"
    );
    drop(stx_future);
    drop(block_future);

    state.zk.bridge_proof_max_future_drift_blocks = 10;
    state.zk.bridge_proof_max_past_age_blocks = 2;
    let header_past =
        iroha_data_model::block::BlockHeader::new(nonzero!(10_u64), None, None, None, 0, 0);
    let mut block_past = state.block(header_past);
    let mut stx_past = block_past.transaction();
    let stale_proof = make_ics_proof(0x66, (1, 7), false);
    let submit_past: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(stale_proof).into();
    let err = exec
        .execute_instruction(&mut stx_past, &ALICE_ID.clone(), submit_past)
        .expect_err("past window should reject stale proof");
    assert!(
        format!("{err:?}").contains("past window"),
        "unexpected error for stale proof: {err:?}"
    );
}

#[test]
fn bridge_overlapping_ranges_are_rejected() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();

    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();
    let proof1 = make_ics_proof(0x71, (10, 12), false);
    let submit1: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof1).into();
    exec.execute_instruction(&mut stx1, &ALICE_ID.clone(), submit1)
        .expect("first proof accepted");
    stx1.apply();
    block1
        .commit()
        .expect("commit first bridge-proof block snapshot");

    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let proof2 = make_ics_proof(0x72, (11, 13), false);
    let submit2: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof2).into();
    let err = exec
        .execute_instruction(&mut stx2, &ALICE_ID.clone(), submit2)
        .expect_err("overlapping bridge proof must be rejected");
    assert!(
        format!("{err:?}").contains("overlaps existing proof"),
        "unexpected error for overlap: {err:?}"
    );
}
