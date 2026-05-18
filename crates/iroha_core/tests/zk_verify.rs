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

#[path = "common/world_fixture.rs"]
mod test_world;

const TINY_ADD_CIRCUIT_ID: &str = "halo2/ipa:tiny-add";

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

fn signed_empty_tx_with_attachments(
    attachments: iroha_data_model::proof::ProofAttachmentList,
) -> SignedTransaction {
    let chain: ChainId = "test-chain".parse().expect("test chain");
    TransactionBuilder::new(chain, ALICE_ID.clone())
        .with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ))
        .with_attachments(attachments)
        .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
}

fn grant_vk_management(
    exec: &Executor,
    stx: &mut iroha_core::state::StateTransaction<'_, '_>,
    authority: &AccountId,
) {
    let permission = iroha_data_model::permission::Permission::new(
        "CanManageVerifyingKeys".parse().expect("permission name"),
        iroha_primitives::json::Json::new(()),
    );
    let grant: InstructionBox = Grant::account_permission(permission, authority.clone()).into();
    exec.execute_instruction(stx, authority, grant)
        .expect("grant vk management");
}

#[test]
fn duplicate_proof_in_same_block_is_rejected() {
    // Minimal node state and block context
    let world = test_world::world_with_test_accounts();
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
    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_preverify");
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_record = build_vk_record("vk_preverify", vk_box, fixture.schema_hash);
    let exec = Executor::default();
    {
        let mut reg_stx = block.transaction();
        let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
            id: vk_id.clone(),
            record: vk_record,
        }
        .into();
        grant_vk_management(&exec, &mut reg_stx, &authority);
        exec.execute_instruction(&mut reg_stx, &authority, reg_vk)
            .expect("register vk");
        reg_stx.apply();
    }
    let attachments = iroha_data_model::proof::ProofAttachmentList(vec![
        iroha_data_model::proof::ProofAttachment::new_ref(
            "halo2/ipa".into(),
            fixture.proof_box("halo2/ipa"),
            vk_id,
        ),
    ]);

    let tx1: SignedTransaction = TransactionBuilder::new(chain.clone(), authority.clone())
        .with_executable(Executable::Instructions(
            Vec::<InstructionBox>::new().into(),
        ))
        .with_attachments(attachments.clone())
        .sign(&private_key);

    let mut stx1 = block.transaction();
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
    let world = test_world::world_with_test_accounts();
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
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_rec = build_vk_record("vk_main", vk_box, fixture.schema_hash);
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_rec,
    }
    .into();
    grant_vk_management(&exec, &mut stx, &ALICE_ID.clone());
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
    let world = test_world::world_with_test_accounts();
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
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_rec = build_vk_record("vk_main", vk_box, fixture.schema_hash);
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_rec,
    }
    .into();
    grant_vk_management(&exec, &mut stx, &ALICE_ID.clone());
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_vk)
        .expect("register vk");

    let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        fixture.proof_box("halo2/ipa"),
        vk_id.clone(),
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
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world, kura, query_handle);

    let mut zk_cfg = state.zk.clone();
    zk_cfg.max_confidential_ops_per_block = 1;
    zk_cfg.max_verify_calls_per_block = 1;
    zk_cfg.max_verify_calls_per_tx = 2;
    zk_cfg.max_proof_bytes_block = 10_000_000;
    zk_cfg.max_proof_size_bytes = 1_000_000;
    state.set_zk(zk_cfg);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_main");
    let fixture1 = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_box = fixture1.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_rec = build_vk_record("vk_main", vk_box, fixture1.schema_hash);
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_rec,
    }
    .into();
    grant_vk_management(&exec, &mut stx, &ALICE_ID.clone());
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_vk)
        .expect("register vk");

    let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        fixture1.proof_box("halo2/ipa"),
        vk_id.clone(),
    );
    let verify: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), verify)
        .expect("first verify should succeed");

    let fixture2 = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let attachment2 = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        fixture2.proof_box("halo2/ipa"),
        vk_id.clone(),
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
    let world = test_world::world_with_test_accounts();
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
fn preverify_rejects_proof_backend_mismatch_before_lookup() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let exec = Executor::default();

    let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        iroha_data_model::proof::ProofBox::new("stark/fri".into(), vec![1, 2, 3]),
        iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_mismatch"),
    );
    let tx = signed_empty_tx_with_attachments(iroha_data_model::proof::ProofAttachmentList(vec![
        attachment,
    ]));

    let mut stx = block.transaction();
    let err = exec
        .execute_transaction(&mut stx, &ALICE_ID.clone(), tx, &mut ivm_cache)
        .expect_err("proof backend mismatch should be rejected before registry lookup");
    assert!(
        matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("proof backend mismatch"))
    );
}

#[test]
fn preverify_rejects_vk_ref_backend_mismatch_before_lookup() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let exec = Executor::default();

    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        fixture.proof_box("halo2/ipa"),
        iroha_data_model::proof::VerifyingKeyId::new("stark/fri", "vk_mismatch"),
    );
    let tx = signed_empty_tx_with_attachments(iroha_data_model::proof::ProofAttachmentList(vec![
        attachment,
    ]));

    let mut stx = block.transaction();
    let err = exec
        .execute_transaction(&mut stx, &ALICE_ID.clone(), tx, &mut ivm_cache)
        .expect_err("vk_ref backend mismatch should be rejected before registry lookup");
    assert!(
        matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("verifying key backend mismatch"))
    );
}

#[test]
fn preverify_rejects_commitment_only_missing_vk_reference() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let exec = Executor::default();

    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let mut attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        fixture.proof_box("halo2/ipa"),
        iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "missing_but_committed"),
    );
    attachment.vk_commitment = Some([0xAB; 32]);
    let tx = signed_empty_tx_with_attachments(iroha_data_model::proof::ProofAttachmentList(vec![
        attachment,
    ]));

    let mut stx = block.transaction();
    let err = exec
        .execute_transaction(&mut stx, &ALICE_ID.clone(), tx, &mut ivm_cache)
        .expect_err("vk_commitment must not bypass the registry reference requirement");
    assert!(
        matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("verifying key inactive"))
    );
}

#[test]
fn preverify_rejects_inactive_registered_vk_even_with_matching_commitment() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut ivm_cache = iroha_core::smartcontracts::ivm::cache::IvmCache::new();
    let exec = Executor::default();
    let authority = ALICE_ID.clone();

    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_proposed");
    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let mut vk_record = build_vk_record("vk_proposed", vk_box, fixture.schema_hash);
    vk_record.status = ConfidentialStatus::Proposed;
    let expected_commitment = vk_record.commitment;
    {
        let mut reg_stx = block.transaction();
        let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
            id: vk_id.clone(),
            record: vk_record,
        }
        .into();
        grant_vk_management(&exec, &mut reg_stx, &authority);
        exec.execute_instruction(&mut reg_stx, &authority, reg_vk)
            .expect("register proposed vk");
        reg_stx.apply();
    }

    let mut attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        fixture.proof_box("halo2/ipa"),
        vk_id,
    );
    attachment.vk_commitment = Some(expected_commitment);
    let tx = signed_empty_tx_with_attachments(iroha_data_model::proof::ProofAttachmentList(vec![
        attachment,
    ]));

    let mut stx = block.transaction();
    let err = exec
        .execute_transaction(&mut stx, &authority, tx, &mut ivm_cache)
        .expect_err("inactive registered vk must not preverify");
    assert!(
        matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("verifying key inactive"))
    );
}

#[test]
fn verifyproof_requires_registered_verifying_key() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        fixture.proof_box("halo2/ipa"),
        iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "missing_vk"),
    );
    let verify: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), verify)
        .expect_err("verifyproof should require a registered verifying key");
    assert!(format!("{err:?}").contains("VerifyingKeyMissing"));
}

#[test]
fn verifyproof_rejects_proof_backend_mismatch_before_lookup() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        iroha_data_model::proof::ProofBox::new("stark/fri".into(), vec![1, 2, 3]),
        iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_not_consulted"),
    );
    let verify: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), verify)
        .expect_err("proof backend mismatch must fail before registry lookup");
    assert!(format!("{err:?}").contains("proof backend mismatch"));
}

#[test]
fn verifyproof_rejects_vk_ref_backend_mismatch_before_lookup() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        fixture.proof_box("halo2/ipa"),
        iroha_data_model::proof::VerifyingKeyId::new("stark/fri", "vk_not_consulted"),
    );
    let verify: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), verify)
        .expect_err("vk_ref backend mismatch must fail before registry lookup");
    assert!(format!("{err:?}").contains("verifying key backend mismatch"));
}

#[test]
fn verifyproof_rejects_inactive_registered_verifying_key() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_inactive");
    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let mut vk_rec = build_vk_record("vk_inactive", vk_box, fixture.schema_hash);
    vk_rec.status = ConfidentialStatus::Proposed;
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_rec,
    }
    .into();
    grant_vk_management(&exec, &mut stx, &ALICE_ID.clone());
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_vk)
        .expect("register inactive vk");

    let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        fixture.proof_box("halo2/ipa"),
        vk_id,
    );
    let verify: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), verify)
        .expect_err("verifyproof must reject inactive verifying keys");
    assert!(format!("{err:?}").contains("verifying key is not active"));
}

#[test]
fn verifyproof_rejects_envelope_vk_hash_mismatch() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_tamper_hash");
    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_rec = build_vk_record("vk_tamper_hash", vk_box, fixture.schema_hash);
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_rec,
    }
    .into();
    grant_vk_management(&exec, &mut stx, &ALICE_ID.clone());
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_vk)
        .expect("register vk");

    let mut proof_box = fixture.proof_box("halo2/ipa");
    let mut envelope: iroha_data_model::zk::OpenVerifyEnvelope =
        norito::decode_from_bytes(&proof_box.bytes).expect("decode OpenVerifyEnvelope");
    envelope.vk_hash[0] ^= 0x80;
    proof_box.bytes = norito::to_bytes(&envelope).expect("encode tampered OpenVerifyEnvelope");

    let attachment =
        iroha_data_model::proof::ProofAttachment::new_ref("halo2/ipa".into(), proof_box, vk_id);
    let verify: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), verify)
        .expect_err("tampered envelope vk_hash must not verify");
    assert!(format!("{err:?}").contains("verifying key commitment mismatch"));
}

#[test]
fn verifyproof_rejects_duplicate_proof_record() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_duplicate");
    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_rec = build_vk_record("vk_duplicate", vk_box, fixture.schema_hash);
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_rec,
    }
    .into();
    grant_vk_management(&exec, &mut stx, &ALICE_ID.clone());
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_vk)
        .expect("register vk");

    let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        fixture.proof_box("halo2/ipa"),
        vk_id,
    );
    let first: InstructionBox =
        iroha_data_model::isi::zk::VerifyProof::new(attachment.clone()).into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), first)
        .expect("first proof verify records proof");

    let duplicate: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), duplicate)
        .expect_err("duplicate proof record must be rejected");
    assert!(format!("{err:?}").contains("Repetition"));
}

#[test]
fn preverify_rejects_empty_proof_as_malformed() {
    let world = test_world::world_with_test_accounts();
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

    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_empty_proof");
    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_record = build_vk_record("vk_empty_proof", vk_box, fixture.schema_hash);
    {
        let mut reg_stx = block.transaction();
        let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
            id: vk_id.clone(),
            record: vk_record,
        }
        .into();
        grant_vk_management(&exec, &mut reg_stx, &authority);
        exec.execute_instruction(&mut reg_stx, &authority, reg_vk)
            .expect("register vk");
        reg_stx.apply();
    }
    let attachments = iroha_data_model::proof::ProofAttachmentList(vec![
        iroha_data_model::proof::ProofAttachment::new_ref(
            "halo2/ipa".into(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![]),
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
        .expect_err("empty proof should be rejected as malformed");
    assert!(matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("malformed proof")));
}

#[test]
fn preverify_rejects_proof_too_big() {
    let world = test_world::world_with_test_accounts();
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
    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_big_proof");
    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_record = build_vk_record("vk_big_proof", vk_box, fixture.schema_hash);
    {
        let mut reg_stx = block.transaction();
        let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
            id: vk_id.clone(),
            record: vk_record,
        }
        .into();
        grant_vk_management(&exec, &mut reg_stx, &authority);
        exec.execute_instruction(&mut reg_stx, &authority, reg_vk)
            .expect("register vk");
        reg_stx.apply();
    }
    let attachments = iroha_data_model::proof::ProofAttachmentList(vec![
        iroha_data_model::proof::ProofAttachment::new_ref(
            "halo2/ipa".into(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), big),
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
        .expect_err("oversized proof should be rejected");
    assert!(matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("proof too big")));
}

#[test]
fn verifyproof_records_rejected_malformed_halo2_envelope() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_bad_proof");
    let fixture = halo2_fixture_envelope(TINY_ADD_CIRCUIT_ID, [0u8; 32]);
    let vk_box = fixture.vk_box("halo2/ipa").expect("fixture verifying key");
    let vk_record = build_vk_record("vk_bad_proof", vk_box, fixture.schema_hash);
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_record,
    }
    .into();
    grant_vk_management(&exec, &mut stx, &ALICE_ID.clone());
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_vk)
        .expect("register vk");

    let mut proof_box = fixture.proof_box("halo2/ipa");
    let mut envelope: iroha_data_model::zk::OpenVerifyEnvelope =
        norito::decode_from_bytes(&proof_box.bytes).expect("decode OpenVerifyEnvelope");
    envelope.proof_bytes[0] ^= 0x01;
    proof_box.bytes = norito::to_bytes(&envelope).expect("encode tampered OpenVerifyEnvelope");

    // Registered VK reference and malformed proof on a supported backend should record rejection.
    let attachment =
        iroha_data_model::proof::ProofAttachment::new_ref("halo2/ipa".into(), proof_box, vk_id);
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
