#![doc = "Ensure proof registry retention cap is enforced per backend."]
#![cfg(feature = "zk-tests")]
//! Ensure proof registry retention cap is enforced per backend.
use crate::root_scope_fixture;
use crate::test_world;
use crate::zk_components;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
use std::collections::BTreeSet;
use zk_components::execute_isi_component;
fn rejected_native_pipa_r_proof(
    circuit_id: String,
    vk_box: &iroha_data_model::proof::VerifyingKeyBox,
    public_inputs: Vec<u8>,
    seed: u8,
) -> iroha_data_model::proof::ProofBox {
    let envelope = iroha_data_model::zk::OpenVerifyEnvelope::new(
        iroha_data_model::zk::BackendTag::NativePipaRPasta,
        circuit_id,
        iroha_core_zk::hash_vk(vk_box),
        public_inputs,
        vec![seed.saturating_add(1)],
    );
    iroha_data_model::proof::ProofBox::new(
        "pipa-r/pasta".into(),
        norito::to_bytes(&envelope).expect("encode OpenVerifyEnvelope"),
    )
}
fn grant_manage_verifying_keys(stx: &mut iroha_core::state::StateTransaction<'_, '_>) {
    let perm = Permission::new(
        "CanManageVerifyingKeys".parse().unwrap(),
        iroha_primitives::json::Json::new(()),
    );
    Grant::account_permission(perm, ALICE_ID.clone())
        .execute(&ALICE_ID.clone(), stx)
        .expect("grant manage vk");
}
#[test]
fn proof_records_pruned_to_cap_per_backend() {
    const CAP: usize = 3;
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut zk = state.installed_zk().clone();
    zk.proof_history_cap = CAP;
    zk.proof_retention_grace_blocks = 0;
    zk.proof_prune_batch = 0;
    state
        .set_zk(zk)
        .expect("install isolated retention component policy");
    root_scope_fixture::commit_global_root_scope(&state);

    // Retained state from another backend must not consume the native cap or be
    // evicted by native proof insertion, even when its record is older.
    let other_id = iroha_data_model::proof::ProofId {
        backend: iroha_core_zk::ZK_BACKEND_STARK_FRI_V1.into(),
        proof_hash: [0xA5; 32],
    };
    let other_record = iroha_data_model::proof::ProofRecord {
        id: other_id.clone(),
        vk_ref: None,
        vk_commitment: None,
        status: iroha_data_model::proof::ProofStatus::Rejected,
        verified_at_height: Some(1),
        bridge: None,
    };
    iroha_core::query::insert_proof_record_for_test(
        &mut state,
        other_id.clone(),
        other_record.clone(),
    );

    let backend = iroha_core_zk::ZK_BACKEND_NATIVE_PIPA_R.to_owned();
    let vk_id = iroha_data_model::proof::VerifyingKeyId::new(backend.clone(), "retention_vk");
    let vk_record =
        iroha_core_zk::confidential_v2::confidential_transfer_v2_vk_record("retention_vk", 1)
            .expect("compiled native transfer key record");
    let vk_box = vk_record.key.clone().expect("compiled native key bytes");
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    grant_manage_verifying_keys(&mut stx);
    let register: InstructionBox = iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: vk_record,
    }
    .into();
    execute_isi_component(&mut stx, &ALICE_ID, &register).expect("register compiled native key");
    stx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit key registry component overlay");

    let mut inserted = Vec::new();
    for i in 0_u8..5 {
        // Distinct component heights make the oldest records unambiguous. The
        // real VerifyProof handler records malformed native bodies as Rejected.
        let height = std::num::NonZeroU64::new(u64::from(i) + 3).expect("nonzero height");
        let header = iroha_data_model::block::BlockHeader::new(height, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let proof = rejected_native_pipa_r_proof(
            iroha_core_zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID.into(),
            &vk_box,
            iroha_core_zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1
                .to_vec(),
            i,
        );
        let id = iroha_data_model::proof::ProofId {
            backend: backend.clone(),
            proof_hash: iroha_core_zk::hash_proof(&proof),
        };
        let attachment = iroha_data_model::proof::ProofAttachment::new_ref(
            backend.clone(),
            proof,
            vk_id.clone(),
        );
        let verify: InstructionBox = iroha_data_model::isi::zk::VerifyProof::new(attachment).into();
        execute_isi_component(&mut stx, &ALICE_ID, &verify).expect("record rejected proof");
        stx.apply();
        // Component overlays establish no authenticated genesis or block finality.
        block
            .commit_world_overlay_for_testing()
            .expect("commit proof retention component overlay");
        inserted.push(id.clone());
        let view = state.view();
        let proofs = view.world().proofs();
        let record = proofs.get(&id).expect("new rejection is retained");
        assert_eq!(
            record.status,
            iroha_data_model::proof::ProofStatus::Rejected
        );
        assert_eq!(record.verified_at_height, Some(height.get()));
        let retained: BTreeSet<_> = proofs
            .iter()
            .filter(|(id, _)| id.backend == backend)
            .map(|(id, _)| id.clone())
            .collect();
        let expected: BTreeSet<_> = inserted.iter().rev().take(CAP).cloned().collect();
        assert_eq!(
            retained, expected,
            "retain exactly the newest native records"
        );
        assert_eq!(proofs.get(&other_id), Some(&other_record));
    }
}
#[test]
fn manual_prune_instruction_applies_new_cap() {
    let world = test_world::world_with_test_accounts();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);
    // Start with a high cap so inserts do not prune.
    let mut zk = state.installed_zk().clone();
    zk.proof_history_cap = 10;
    zk.proof_retention_grace_blocks = 0;
    zk.proof_prune_batch = 10;
    state
        .set_zk(zk)
        .expect("empty state accepts retention test configuration");
    // Instruction execution requires committed root authority: the first height commits
    // the Global root scope and the fixture executes from the second height on.
    root_scope_fixture::commit_global_root_scope(&state);
    let backend = "pipa-r/pasta".to_string();
    // A proof record enters the registry when a proof of a production circuit is verified,
    // and a verifying key through the governed lifecycle. Retention does not depend on
    // either: the fixture seeds four retained records of the backend directly.
    for i in 0u8..4 {
        let vk_box = iroha_data_model::proof::VerifyingKeyBox::new(backend.clone(), vec![i; 8]);
        let proof_box = rejected_native_pipa_r_proof(
            iroha_core_zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID.to_owned(),
            &vk_box,
            vec![0x5A, i],
            i,
        );
        let id = iroha_data_model::proof::ProofId {
            backend: proof_box.backend.clone(),
            proof_hash: iroha_core_zk::hash_proof(&proof_box),
        };
        let record = iroha_data_model::proof::ProofRecord {
            id: id.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: iroha_data_model::proof::ProofStatus::Rejected,
            verified_at_height: Some(u64::from(i) + 1),
            bridge: None,
        };
        iroha_core::query::insert_proof_record_for_test(&mut state, id, record);
    }
    assert_eq!(
        state
            .view()
            .world()
            .proofs()
            .iter()
            .filter(|(id, _)| id.backend.as_str() == backend.as_str())
            .count(),
        4,
        "the fixture retains four records before the cap is tightened"
    );
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    grant_manage_verifying_keys(&mut stx);
    stx.apply();
    // The fixture height follows no certified parent, so its World overlay is committed
    // without pretending that a block was finalized.
    block
        .commit_world_overlay_for_testing()
        .expect("commit fixture overlay");
    // Tighten retention policy and prune explicitly.
    let mut zk = state.installed_zk().clone();
    zk.proof_history_cap = 1;
    zk.proof_retention_grace_blocks = 0;
    zk.proof_prune_batch = 0;
    state
        .set_zk(zk)
        .expect("empty state accepts retention test configuration");
    let prune_header =
        iroha_data_model::block::BlockHeader::new(nonzero!(10_u64), None, None, 0, 0);
    let mut prune_block = state.block(prune_header);
    let mut stx = prune_block.transaction();
    let prune: InstructionBox =
        iroha_data_model::isi::zk::PruneProofs::new(Some(backend.clone())).into();
    execute_isi_component(&mut stx, &ALICE_ID.clone(), &(prune)).expect("prune proofs");
    stx.apply();
    prune_block
        .commit_world_overlay_for_testing()
        .expect("commit prune fixture overlay");
    let view = state.view();
    let remaining = view
        .world()
        .proofs()
        .iter()
        .filter(|(id, _)| id.backend.as_str() == backend.as_str())
        .count();
    assert_eq!(
        remaining, 1,
        "manual pruning retains exactly the tightened cap"
    );
}
