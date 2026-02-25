#![doc = "Verifying-key registry indexing by `(circuit_id, version)`."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
//! Verifying-key registry indexing by `(circuit_id, version)`.

use std::sync::Arc;

use iroha_core::{
    executor::Executor,
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, WorldReadOnly},
    zk::{hash_proof, hash_vk},
};
use iroha_crypto::Hash as CryptoHash;
use iroha_data_model::{
    ValidationFail,
    confidential::ConfidentialStatus,
    isi::{
        Grant,
        error::{InstructionExecutionError, InvalidParameterError},
        verifying_keys,
    },
    permission::Permission,
    prelude::InstructionBox,
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::{BackendTag, OpenVerifyEnvelope},
};
use iroha_primitives::json::Json;
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

#[path = "common/world_fixture.rs"]
mod test_world;

#[allow(clippy::disallowed_types)]
type PreverifiedMap = std::collections::BTreeMap<[u8; 32], bool>;

fn grant_manage_vk(block: &mut iroha_core::state::StateBlock<'_>) {
    let mut stx = block.transaction();
    let perm = Permission::new(
        "CanManageVerifyingKeys".parse().expect("permission id"),
        Json::new(()),
    );
    Grant::account_permission(perm, ALICE_ID.clone())
        .execute(&ALICE_ID.clone(), &mut stx)
        .expect("grant manage vk");
    stx.apply();
}

fn register_vk(
    exec: &Executor,
    block: &mut iroha_core::state::StateBlock<'_>,
    id: &VerifyingKeyId,
    record: &VerifyingKeyRecord,
) {
    let mut stx = block.transaction();
    let instr: InstructionBox = verifying_keys::RegisterVerifyingKey {
        id: id.clone(),
        record: record.clone(),
    }
    .into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
        .expect("register vk");
    stx.apply();
}

fn base_record(circuit: &str, version: u32) -> VerifyingKeyRecord {
    let mut rec = VerifyingKeyRecord::new(
        version,
        circuit.to_string(),
        BackendTag::Halo2IpaPasta,
        "pallas",
        [0xAA; 32],
        [0xBB; 32],
    );
    rec.status = ConfidentialStatus::Active;
    rec.gas_schedule_id = Some("halo2_default".into());
    rec.max_proof_bytes = 4096;
    rec
}

#[test]
fn duplicate_circuit_version_registration_rejected() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    grant_manage_vk(&mut block);

    let exec = Executor::default();

    let id_primary = VerifyingKeyId::new("halo2/ipa", "vk_primary");
    let rec_primary = base_record("circuit_alpha", 1);
    register_vk(&exec, &mut block, &id_primary, &rec_primary);

    let mut stx = block.transaction();
    let id_secondary = VerifyingKeyId::new("halo2/ipa", "vk_secondary");
    let rec_secondary = base_record("circuit_alpha", 1);
    let instr: InstructionBox = verifying_keys::RegisterVerifyingKey {
        id: id_secondary.clone(),
        record: rec_secondary,
    }
    .into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
        .expect_err("duplicate circuit/version must be rejected");
    match err {
        ValidationFail::InstructionFailed(InstructionExecutionError::InvariantViolation(msg)) => {
            assert!(msg.contains("circuit/version"), "unexpected message: {msg}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn update_rotates_circuit_version_index() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    grant_manage_vk(&mut block);

    let exec = Executor::default();

    let id = VerifyingKeyId::new("halo2/ipa", "vk_upgrade");
    let rec_v1 = base_record("circuit_beta", 1);
    register_vk(&exec, &mut block, &id, &rec_v1);

    // Upgrade to version 2
    let mut stx = block.transaction();
    let rec_v2 = base_record("circuit_beta", 2);
    let update: InstructionBox = verifying_keys::UpdateVerifyingKey {
        id: id.clone(),
        record: rec_v2,
    }
    .into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), update)
        .expect("update vk");
    stx.apply();

    let view = state.view();
    let map = view.world().verifying_keys_by_circuit();
    let key_old = (String::from("circuit_beta"), 1);
    let key_new = (String::from("circuit_beta"), 2);
    assert!(
        map.get(&key_new).is_some(),
        "new version missing from index"
    );
    assert!(map.get(&key_old).is_none(), "old version still indexed");
}

fn register_halo2_vk(
    block: &mut iroha_core::state::StateBlock<'_>,
    exec: &Executor,
    name: &str,
    circuit: &str,
    version: u32,
    vk_bytes: Vec<u8>,
    public_inputs_hash: [u8; 32],
) -> VerifyingKeyId {
    let id = VerifyingKeyId::new("halo2/ipa", name);
    let mut record = base_record(circuit, version);
    let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vk_bytes);
    record.key = Some(vk_box.clone());
    record.commitment = hash_vk(&vk_box);
    record.public_inputs_schema_hash = public_inputs_hash;
    record.vk_len =
        u32::try_from(vk_box.bytes.len()).expect("verifying key length must fit into u32");
    register_vk(exec, block, &id, &record);
    id
}

fn execute_verify_proof(
    block: &mut iroha_core::state::StateBlock<'_>,
    attachment: iroha_data_model::proof::ProofAttachment,
) -> Result<(), ValidationFail> {
    let proof_hash = hash_proof(&attachment.proof);
    let mut preverified = PreverifiedMap::new();
    preverified.insert(proof_hash, true);
    block.set_preverified_batch(Arc::new(preverified));
    let mut stx = block.transaction();
    let isi: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
    let res = isi
        .execute(&ALICE_ID, &mut stx)
        .map_err(ValidationFail::InstructionFailed);
    if res.is_ok() {
        stx.apply();
    }
    res
}

#[test]
fn verify_proof_rejects_circuit_mismatch() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    grant_manage_vk(&mut block);

    let exec = Executor::default();
    let public_inputs = vec![1u8, 2, 3];
    let expected_hash: [u8; 32] = CryptoHash::new(&public_inputs).into();
    let vk_id = register_halo2_vk(
        &mut block,
        &exec,
        "vk_main",
        "circuit_alpha",
        1,
        vec![7, 7, 7],
        expected_hash,
    );

    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: "circuit_beta".into(),
        vk_hash: hash_vk(&VerifyingKeyBox::new("halo2/ipa".into(), vec![7, 7, 7])),
        public_inputs: public_inputs.clone(),
        proof_bytes: vec![9, 9, 9],
        aux: Vec::new(),
    };
    let proof_bytes = norito::to_bytes(&envelope).expect("serialize envelope");
    let proof_box = iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), proof_bytes);
    let attachment =
        iroha_data_model::proof::ProofAttachment::new_ref("halo2/ipa".into(), proof_box, vk_id);

    let err = execute_verify_proof(&mut block, attachment).expect_err("must fail");
    match err {
        ValidationFail::InstructionFailed(InstructionExecutionError::InvariantViolation(msg)) => {
            assert!(msg.contains("circuit mismatch"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn verify_proof_rejects_schema_hash_mismatch() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    grant_manage_vk(&mut block);

    let exec = Executor::default();
    let public_inputs = vec![4u8, 5, 6];
    let vk_id = register_halo2_vk(
        &mut block,
        &exec,
        "vk_main",
        "circuit_alpha",
        1,
        vec![8, 8, 8],
        [0xFF; 32],
    );

    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: "circuit_alpha".into(),
        vk_hash: hash_vk(&VerifyingKeyBox::new("halo2/ipa".into(), vec![8, 8, 8])),
        public_inputs: public_inputs.clone(),
        proof_bytes: vec![10, 10, 10],
        aux: Vec::new(),
    };
    let proof_bytes = norito::to_bytes(&envelope).expect("serialize envelope");
    let proof_box = iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), proof_bytes);
    let attachment =
        iroha_data_model::proof::ProofAttachment::new_ref("halo2/ipa".into(), proof_box, vk_id);

    let err = execute_verify_proof(&mut block, attachment).expect_err("must fail");
    match err {
        ValidationFail::InstructionFailed(InstructionExecutionError::InvariantViolation(msg)) => {
            assert!(msg.contains("schema hash"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn verify_proof_accepts_matching_metadata() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    grant_manage_vk(&mut block);

    let exec = Executor::default();
    let public_inputs = vec![11u8, 12, 13];
    let hash: [u8; 32] = CryptoHash::new(&public_inputs).into();
    let vk_bytes = vec![9, 9, 9];
    let vk_id = register_halo2_vk(
        &mut block,
        &exec,
        "vk_main",
        "circuit_alpha",
        1,
        vk_bytes.clone(),
        hash,
    );

    let commitment = hash_vk(&VerifyingKeyBox::new("halo2/ipa".into(), vk_bytes));
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: "circuit_alpha".into(),
        vk_hash: commitment,
        public_inputs: public_inputs.clone(),
        proof_bytes: vec![1, 2, 3, 4],
        aux: Vec::new(),
    };
    let proof_bytes = norito::to_bytes(&envelope).expect("serialize envelope");
    let proof_box = iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), proof_bytes);
    let attachment =
        iroha_data_model::proof::ProofAttachment::new_ref("halo2/ipa".into(), proof_box, vk_id);

    execute_verify_proof(&mut block, attachment).expect("verify succeeds");
}

#[test]
fn register_requires_circuit_and_schema_hash() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    grant_manage_vk(&mut block);

    let exec = Executor::default();
    let mut rec = base_record("", 1);
    rec.circuit_id.clear();
    let id = VerifyingKeyId::new("halo2/ipa", "vk_bad");
    let mut stx = block.transaction();
    let instr: InstructionBox = verifying_keys::RegisterVerifyingKey {
        id: id.clone(),
        record: rec.clone(),
    }
    .into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), instr)
        .expect_err("empty circuit_id must be rejected");
    match err {
        ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(msg),
        )) => {
            assert!(
                msg.contains("circuit_id"),
                "unexpected message for circuit guard: {msg}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }

    drop(stx);

    rec.circuit_id = "circuit_alpha".into();
    rec.public_inputs_schema_hash = [0u8; 32];
    let mut stx2 = block.transaction();
    let instr2: InstructionBox = verifying_keys::RegisterVerifyingKey { id, record: rec }.into();
    let err2 = exec
        .execute_instruction(&mut stx2, &ALICE_ID.clone(), instr2)
        .expect_err("zero schema hash must be rejected");
    match err2 {
        ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(msg),
        )) => {
            assert!(
                msg.contains("schema hash"),
                "unexpected message for schema guard: {msg}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
