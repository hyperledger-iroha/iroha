#![doc = "`DeprecateVerifyingKey` lifecycle tests."]
#![cfg(feature = "zk-tests")]
//! Ensure `DeprecateVerifyingKey` flips status to Deprecated and clears inline bytes.
#![allow(clippy::items_after_statements)]

use iroha_core::{
    executor::Executor,
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, WorldReadOnly},
};
use iroha_data_model::{confidential::ConfidentialStatus, prelude::*, zk::BackendTag};
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;

#[test]
fn vk_deprecate_marks_status_and_clears_key() {
    // Minimal node state and block context
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);

    // Grant CanManageVerifyingKeys to ALICE
    let mut stx = block.transaction();
    use iroha_data_model::permission::Permission;
    let perm = Permission::new(
        "CanManageVerifyingKeys".parse().unwrap(),
        iroha_primitives::json::Json::new(()),
    );
    iroha_data_model::prelude::Grant::account_permission(perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant vk manage");

    // Register a VK (with inline bytes)
    let id = iroha_data_model::proof::VerifyingKeyId::new("halo2/ipa", "vk_test");
    let vk_inline =
        iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![1, 2, 3]);
    let commitment = iroha_core::zk::hash_vk(&vk_inline);
    let mut rec = iroha_data_model::proof::VerifyingKeyRecord::new(
        1,
        "vk_test",
        BackendTag::Halo2IpaPasta,
        "pallas",
        [0x58; 32],
        commitment,
    );
    rec.vk_len = 3;
    rec.key = Some(vk_inline);
    rec.status = ConfidentialStatus::Active;
    rec.gas_schedule_id = Some("halo2_default".into());
    let reg_insn: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: id.clone(),
        record: rec,
    }
    .into();
    let exec = Executor::default();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_insn)
        .expect("register vk");

    // Deprecate
    let dep: InstructionBox =
        iroha_data_model::isi::verifying_keys::DeprecateVerifyingKey { id: id.clone() }.into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), dep)
        .expect("deprecate vk");

    // Apply
    stx.apply();

    // Assert record remains with Deprecated status and no inline key
    let view = state.view();
    use mv::storage::StorageReadOnly;
    let rec_stored = view
        .world
        .verifying_keys()
        .get(&id)
        .cloned()
        .expect("vk should remain present after deprecate");
    assert!(matches!(rec_stored.status, ConfidentialStatus::Deprecated));
    assert!(
        rec_stored.key.is_none(),
        "inline key should be cleared on deprecate"
    );
}
