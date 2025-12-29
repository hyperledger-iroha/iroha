#![doc = "Verifying-key retention cap tests for ZK backends"]
#![cfg(feature = "zk-tests")]
//! Ensure deprecated verifying-key retention cap per backend is enforced.

use iroha_core::smartcontracts::Execute; // for .execute on ISI/Grant
use iroha_core::{
    executor::Executor,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, WorldReadOnly},
};
use iroha_data_model::{confidential::ConfidentialStatus, prelude::*, zk::BackendTag};
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
use std::num::NonZeroU64;

fn prepare_state_with_vk_permission() -> State {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query_handle);

    {
        use iroha_data_model::permission::Permission;
        use iroha_data_model::prelude::Grant;
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let perm = Permission::new(
            "CanManageVerifyingKeys".parse().unwrap(),
            iroha_primitives::json::Json::new(()),
        );
        Grant::account_permission(perm, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("grant vk manage");
        stx.apply();
    }

    state
}

#[test]
fn deprecated_vk_pruned_to_cap_per_backend() {
    let mut state = prepare_state_with_vk_permission();

    // Set small deprecated cap per backend
    let mut zk_cfg = state.zk.clone();
    zk_cfg.vk_deprecated_cap_per_backend = 3;
    state.set_zk(zk_cfg);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let exec = Executor::default();
    let backend = "halo2/ipa".to_string();

    // Register and deprecate 5 different VK names under same backend
    for i in 0..5u8 {
        let mut stx = block.transaction();
        let name = format!("vk_{i}");
        let id = iroha_data_model::proof::VerifyingKeyId::new(backend.clone(), name);
        let vk_inline =
            iroha_data_model::proof::VerifyingKeyBox::new(backend.clone(), vec![1, 2, 3, i]);
        let commitment = iroha_core::zk::hash_vk(&vk_inline);
        let mut rec = iroha_data_model::proof::VerifyingKeyRecord::new(
            1,
            format!("vk_{i}"),
            BackendTag::Halo2IpaPasta,
            "pallas",
            [0x59; 32],
            commitment,
        );
        rec.vk_len = 4;
        rec.key = Some(vk_inline);
        rec.status = ConfidentialStatus::Active;
        rec.gas_schedule_id = Some("halo2_default".into());
        let reg_instr: InstructionBox =
            iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
                id: id.clone(),
                record: rec,
            }
            .into();
        exec.execute_instruction(&mut stx, &ALICE_ID.clone(), reg_instr)
            .expect("register vk");
        let dep: InstructionBox =
            iroha_data_model::isi::verifying_keys::DeprecateVerifyingKey { id: id.clone() }.into();
        exec.execute_instruction(&mut stx, &ALICE_ID.clone(), dep)
            .expect("deprecate vk");
        stx.apply();
    }

    // Count deprecated VKs for backend
    let view = state.view();
    let count_depr = view
        .world()
        .verifying_keys()
        .iter()
        .filter(|(id, rec)| {
            id.backend.as_ref() == backend && matches!(rec.status, ConfidentialStatus::Deprecated)
        })
        .count();
    assert!(count_depr <= 3, "deprecated count {count_depr} exceeds cap");
}

#[test]
fn deprecated_vk_pruning_prefers_oldest_height() {
    let mut state = prepare_state_with_vk_permission();

    let mut zk_cfg = state.zk.clone();
    zk_cfg.vk_deprecated_cap_per_backend = 2;
    state.set_zk(zk_cfg);

    let exec = Executor::default();
    let backend = "halo2/ipa".to_string();
    let scenarios = [(1_u64, "zz_old"), (2_u64, "aa_new"), (3_u64, "mm_new")];

    for (height, name) in scenarios {
        let header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(height).expect("height must be non-zero"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let id = iroha_data_model::proof::VerifyingKeyId::new(backend.clone(), name);
        let vk_inline = iroha_data_model::proof::VerifyingKeyBox::new(
            backend.clone(),
            vec![1, 2, 3, u8::try_from(height).expect("height fits in u8")],
        );
        let commitment = iroha_core::zk::hash_vk(&vk_inline);
        let mut rec = iroha_data_model::proof::VerifyingKeyRecord::new(
            1,
            name.to_string(),
            BackendTag::Halo2IpaPasta,
            "pallas",
            [0x42; 32],
            commitment,
        );
        rec.vk_len = u32::try_from(vk_inline.bytes.len()).expect("vk len fits in u32");
        rec.key = Some(vk_inline);
        rec.status = ConfidentialStatus::Active;
        rec.gas_schedule_id = Some("halo2_default".into());

        let register: InstructionBox =
            iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
                id: id.clone(),
                record: rec,
            }
            .into();
        exec.execute_instruction(&mut stx, &ALICE_ID.clone(), register)
            .expect("register vk");
        let deprecate: InstructionBox =
            iroha_data_model::isi::verifying_keys::DeprecateVerifyingKey { id: id.clone() }.into();
        exec.execute_instruction(&mut stx, &ALICE_ID.clone(), deprecate)
            .expect("deprecate vk");
        stx.apply();
        block.commit().expect("commit block");
    }

    let view = state.view();
    let mut retained: Vec<(String, Option<u64>)> = view
        .world()
        .verifying_keys()
        .iter()
        .filter(|(id, rec)| {
            id.backend.as_ref() == backend && matches!(rec.status, ConfidentialStatus::Deprecated)
        })
        .map(|(id, rec)| (id.name.clone(), rec.deprecation_height))
        .collect();
    retained.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(retained.len(), 2, "cap should retain exactly two records");
    let names: Vec<&str> = retained.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        vec!["aa_new", "mm_new"],
        "oldest record should be pruned"
    );
    let heights: Vec<Option<u64>> = retained.iter().map(|(_, h)| *h).collect();
    assert_eq!(
        heights,
        vec![Some(2), Some(3)],
        "heights must reflect pruning order"
    );
    assert!(
        view.world()
            .verifying_keys()
            .get(&iroha_data_model::proof::VerifyingKeyId::new(
                backend.clone(),
                "zz_old"
            ))
            .is_none(),
        "zz_old should have been pruned due to oldest deprecation height"
    );
}
