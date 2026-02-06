#![doc = "ZK attachment pre-verify wiring tests (dedup and basic sanity)."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
//! ZK attachment pre-verify wiring tests (dedup and basic sanity).
#![cfg(feature = "zk-preverify")]

use iroha_core::{
    executor::Executor,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, WorldReadOnly},
    zk::test_utils::halo2_fixture_envelope,
};
use iroha_data_model::{
    ValidationFail,
    confidential::ConfidentialStatus,
    isi::error::{InstructionExecutionError, InvalidParameterError},
    prelude::*,
    zk::BackendTag,
};
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

const TINY_ADD_CIRCUIT_ID: &str = "halo2/ipa:tiny-add-v1";

fn build_vk_record(
    _name: &str,
    vk_box: iroha_data_model::proof::VerifyingKeyBox,
    schema_hash: [u8; 32],
) -> iroha_data_model::proof::VerifyingKeyRecord {
    let commitment = iroha_core::zk::hash_vk(&vk_box);
    let mut record = iroha_data_model::proof::VerifyingKeyRecord::new_with_owner(
        1,
        TINY_ADD_CIRCUIT_ID,
        None,
        "core",
        BackendTag::Halo2IpaPasta,
        "pallas",
        schema_hash,
        commitment,
    );
    record.vk_len = vk_box.bytes.len() as u32;
    record.status = ConfidentialStatus::Active;
    record.key = Some(vk_box);
    record.gas_schedule_id = Some("halo2_default".into());
    record
}

#[test]
fn duplicate_proof_in_same_block_is_rejected() {
    // Minimal node state and block context
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();

    // Build a transaction with proofs carrying a single inline attachment and no instructions
    let chain: ChainId = "test-chain".parse().unwrap();
    let authority = ALICE_ID.clone();
    let private_key = iroha_test_samples::ALICE_KEYPAIR.private_key().clone();

    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let attachments = iroha_data_model::proof::ProofAttachmentList(vec![
        iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            fixture.proof_box("halo2/ipa"),
            vk_box,
        ),
    ]);

    let tx1: SignedTransaction = TransactionBuilder::new(chain.clone(), authority.clone())
        .with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ))
        .with_attachments(attachments.clone())
        .sign(&private_key);

    let mut stx1 = block.transaction();
    let exec = Executor::default();
    exec.execute_transaction(&mut stx1, &authority, tx1, &mut ivm_cache)
        .expect("first tx should pass pre-verify");
    drop(stx1);

    // Second identical transaction in the same block should hit dedup
    let tx2: SignedTransaction = TransactionBuilder::new(chain, authority.clone())
        .with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ))
        .with_attachments(attachments)
        .sign(&private_key);

    let mut stx2 = block.transaction();
    let err = exec
        .execute_transaction(&mut stx2, &authority, tx2, &mut ivm_cache)
        .expect_err("duplicate proof should be rejected");
    match err {
        ValidationFail::NotPermitted(msg) => {
            assert!(msg.contains("duplicate proof"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn verifyproof_isi_records_proof() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    // Register a verifying key record
    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_main");
    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_inline = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let commitment = iroha_core::zk::hash_vk(&vk_inline);
    let vk_rec = build_vk_record("vk_main", vk_inline.clone(), fixture.schema_hash);
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_rec,
    }
    .into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_vk)
        .expect("register vk");

    // Verify a proof using VK reference
    let proof_box = fixture.proof_box("halo2/ipa");
    let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        proof_box.clone(),
        vk_id.clone(),
    );
    let verify: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), verify)
        .expect("verify proof");

    // Apply transaction and ensure a proof record exists
    let pid = iroha_data_model::proof::ProofId {
        backend: "halo2/ipa".into(),
        proof_hash: iroha_core::zk::hash_proof(&proof_box),
    };
    assert!(stx.world.proofs().get(&pid).is_some());
}

#[test]
fn verifyproof_rejects_when_exceeding_size_cap() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world, kura, query_handle);

    let mut zk_cfg = state.zk.clone();
    zk_cfg.max_proof_size_bytes = 3;
    state.set_zk(zk_cfg);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_main");
    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_inline = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_rec = build_vk_record("vk_main", vk_inline.clone(), fixture.schema_hash);
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_rec,
    }
    .into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_vk)
        .expect("register vk");

    let attachment = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/ipa".into(),
        fixture.proof_box("halo2/ipa"),
        vk_inline,
    );
    let verify: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), verify)
        .expect_err("proof should exceed size cap");
    assert!(matches!(
        err,
        ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(msg)
        )) if msg.contains("max_proof_size_bytes")
    ));
}

#[test]
fn verifyproof_rejects_when_block_cap_hit() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world, kura, query_handle);

    let mut zk_cfg = state.zk.clone();
    zk_cfg.max_confidential_ops_per_block = 1;
    zk_cfg.max_verify_calls_per_block = 1;
    zk_cfg.max_verify_calls_per_tx = 2;
    zk_cfg.max_proof_bytes_block = 100;
    zk_cfg.max_proof_size_bytes = 16;
    state.set_zk(zk_cfg);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_main");
    let fixture1 = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_inline = fixture1.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_rec = build_vk_record("vk_main", vk_inline.clone(), fixture1.schema_hash);
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_rec,
    }
    .into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_vk)
        .expect("register vk");

    let attachment = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/ipa".into(),
        fixture1.proof_box("halo2/ipa"),
        vk_inline.clone(),
    );
    let verify: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), verify)
        .expect("first verify should succeed");

    let fixture2 = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let attachment2 = iroha_data_model::proof::ProofAttachment::new_inline(
        "halo2/ipa".into(),
        fixture2.proof_box("halo2/ipa"),
        vk_inline,
    );
    let verify2: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment2).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), verify2)
        .expect_err("second verify should hit block cap");
    assert!(matches!(
        err,
        ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(msg)
        )) if msg.contains("per block exceeded")
    ));
}

#[test]
fn preverify_rejects_missing_vk_reference() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let exec = Executor::default();

    // Build tx with attachment referencing a non-existent VK id
    let chain: ChainId = "test-chain".parse().unwrap();
    let authority = ALICE_ID.clone();
    let private_key = iroha_test_samples::ALICE_KEYPAIR.private_key().clone();
    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_missing");
    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let attachments = iroha_data_model::proof::ProofAttachmentList(vec![
        iroha_data_model::proof::ProofAttachment::new_ref(
            "halo2/ipa".into(),
            fixture.proof_box("halo2/ipa"),
            vk_id,
        ),
    ]);
    let tx: SignedTransaction = TransactionBuilder::new(chain, authority.clone())
        .with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ))
        .with_attachments(attachments)
        .sign(&private_key);

    let mut stx = block.transaction();
    let err = exec
        .execute_transaction(&mut stx, &authority, tx, &mut ivm_cache)
        .expect_err("missing vk_ref should be rejected");
    assert!(matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("verifying key")));
}

#[test]
fn preverify_rejects_when_no_verifying_key_present() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let exec = Executor::default();

    let chain: ChainId = "test-chain".parse().unwrap();
    let authority = ALICE_ID.clone();
    let private_key = iroha_test_samples::ALICE_KEYPAIR.private_key().clone();

    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let attachments = iroha_data_model::proof::ProofAttachmentList(vec![
        iroha_data_model::proof::ProofAttachment {
            backend: "halo2/ipa".into(),
            proof: fixture.proof_box("halo2/ipa"),
            vk_ref: None,
            vk_inline: None,
            vk_commitment: None,
            envelope_hash: None,
            lane_privacy: None,
        },
    ]);

    let tx: SignedTransaction = TransactionBuilder::new(chain, authority.clone())
        .with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ))
        .with_attachments(attachments)
        .sign(&private_key);

    let mut stx = block.transaction();
    let err = exec
        .execute_transaction(&mut stx, &authority, tx, &mut ivm_cache)
        .expect_err("attachments without a verifying key should be rejected");
    match err {
        ValidationFail::NotPermitted(msg) => {
            assert!(msg.contains("verifying key"), "msg={msg}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn verifyproof_requires_verifying_key() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let attachment = iroha_data_model::proof::ProofAttachment {
        backend: "halo2/ipa".into(),
        proof: fixture.proof_box("halo2/ipa"),
        vk_ref: None,
        vk_inline: None,
        vk_commitment: None,
        envelope_hash: None,
        lane_privacy: None,
    };
    let verify: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), verify)
        .expect_err("verifyproof should require a verifying key");
    assert!(matches!(
        err,
        ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(msg)
        )) if msg.contains("verifying key")
    ));
}

#[test]
fn preverify_rejects_empty_proof_as_malformed() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let exec = Executor::default();

    let chain: ChainId = "test-chain".parse().unwrap();
    let authority = ALICE_ID.clone();
    let private_key = iroha_test_samples::ALICE_KEYPAIR.private_key().clone();

    let attachments = iroha_data_model::proof::ProofAttachmentList(vec![
        iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![]),
            iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2, 3]),
        ),
    ]);

    let tx: SignedTransaction = TransactionBuilder::new(chain, authority.clone())
        .with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ))
        .with_attachments(attachments)
        .sign(&private_key);

    let mut stx = block.transaction();
    let err = exec
        .execute_transaction(&mut stx, &authority, tx, &mut ivm_cache)
        .expect_err("empty proof should be rejected as malformed");
    assert!(matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("malformed proof")));
}

#[test]
fn preverify_rejects_proof_too_big() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let exec = Executor::default();

    let chain: ChainId = "test-chain".parse().unwrap();
    let authority = ALICE_ID.clone();
    let private_key = iroha_test_samples::ALICE_KEYPAIR.private_key().clone();

    // Build a proof larger than the current preverify cap (1 MiB)
    let big = vec![0u8; 1_200_000];
    let attachments = iroha_data_model::proof::ProofAttachmentList(vec![
        iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), big),
            iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2, 3]),
        ),
    ]);

    let tx: SignedTransaction = TransactionBuilder::new(chain, authority.clone())
        .with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ))
        .with_attachments(attachments)
        .sign(&private_key);

    let mut stx = block.transaction();
    let err = exec
        .execute_transaction(&mut stx, &authority, tx, &mut ivm_cache)
        .expect_err("oversized proof should be rejected");
    assert!(matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("proof too big")));
}

#[test]
fn verifyproof_rejects_via_debug_backend() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    // Inline VK and proof on a debug backend that forces rejection
    let attachment = iroha_data_model::proof::ProofAttachment::new_inline(
        "debug/reject".into(),
        iroha_data_model::proof::ProofBox::new("debug/reject".into(), vec![0xaa]),
        iroha_data_model::proof::VerifyingKeyBox::new("debug/reject".into(), vec![0xbb]),
    );
    let verify: InstructionBox =
        iroha_data_model::isi::zk::VerifyProof::new(attachment.clone()).into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), verify)
        .expect("verify should record even if rejected");

    let pid = iroha_data_model::proof::ProofId {
        backend: attachment.backend,
        proof_hash: iroha_core::zk::hash_proof(&attachment.proof),
    };
    let rec = stx.world.proofs().get(&pid).expect("proof record exists");
    assert!(matches!(
        rec.status,
        iroha_data_model::proof::ProofStatus::Rejected
    ));
}
