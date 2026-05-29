#![doc = "Backend tag acceptance tests for ZK attachments (pre-verify path)."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "zk-tests")]
#![cfg(feature = "zk-preverify")]
//! Backend tag acceptance tests for ZK attachments (pre-verify path).
//! - Trusted-setup families (e.g., `groth16/*`) are rejected at VK admission.
//! - Halo2 curve mismatch is rejected at VK admission.

use iroha_core::{
    executor::Executor, kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute,
    state::State, zk::test_utils::halo2_fixture_envelope,
};
use iroha_data_model::prelude::*;
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

fn vk_record(
    circuit_id: &str,
    backend_tag: iroha_data_model::zk::BackendTag,
    curve: &str,
    vk_box: iroha_data_model::proof::VerifyingKeyBox,
    schema_hash: [u8; 32],
) -> iroha_data_model::proof::VerifyingKeyRecord {
    let mut record = iroha_data_model::proof::VerifyingKeyRecord::new(
        1,
        circuit_id,
        backend_tag,
        curve,
        schema_hash,
        iroha_core::zk::hash_vk(&vk_box),
    );
    record.vk_len = vk_box.bytes.len() as u32;
    record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    record.key = Some(vk_box);
    record.gas_schedule_id = Some("halo2_default".into());
    record
}

#[test]
fn groth16_backend_tag_is_unsupported() {
    let (state, header) = new_block_ctx();
    let mut block = state.block(header);
    let exec = Executor::default();

    let mut stx = block.transaction();
    let authority = ALICE_ID.clone();
    let perm = Permission::new(
        "CanManageVerifyingKeys".parse().unwrap(),
        iroha_primitives::json::Json::new(()),
    );
    Grant::account_permission(perm, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant manage vk");

    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("groth16/bn254", "vk_groth16");
    let vk_box =
        iroha_data_model::proof::VerifyingKeyBox::new("groth16/bn254".into(), vec![7, 7, 7]);
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_record(
            "groth16/bn254:unsupported",
            iroha_data_model::zk::BackendTag::Groth16,
            "bn254",
            vk_box,
            [0u8; 32],
        ),
    }
    .into();
    let err = exec
        .execute_instruction(&mut stx, &authority, reg_vk)
        .expect_err("trusted-setup VK backend should be rejected at admission");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("backend must be Halo2IpaPasta or Stark"),
        "unexpected error: {msg}"
    );
}

#[test]
fn halo2_curve_mismatch_rejected_at_vk_admission() {
    let (state, header) = new_block_ctx();
    let mut block = state.block(header);
    let exec = Executor::default();

    let authority = ALICE_ID.clone();
    let halo2_fixture = halo2_fixture_envelope("halo2/pasta/ipa/tiny-add", [0u8; 32]);
    let vk_box = halo2_fixture
        .vk_box("halo2/pasta/ipa")
        .expect("fixture verifying key");
    let vk_id = iroha_data_model::proof::VerifyingKeyId::new("halo2/pasta/ipa", "vk_curve");
    let mut stx = block.transaction();
    let perm = Permission::new(
        "CanManageVerifyingKeys".parse().unwrap(),
        iroha_primitives::json::Json::new(()),
    );
    Grant::account_permission(perm, authority.clone())
        .execute(&authority, &mut stx)
        .expect("grant manage vk");
    let reg_vk: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_record(
            "halo2/pasta/ipa/tiny-add",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            vk_box,
            halo2_fixture.schema_hash,
        ),
    }
    .into();
    let err = exec
        .execute_instruction(&mut stx, &authority, reg_vk)
        .expect_err("curve mismatch should be rejected at VK admission");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("verifying key curve must be \\\"pallas\\\""),
        "unexpected error: {msg}"
    );
}
