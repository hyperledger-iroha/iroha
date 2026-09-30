//! Native journal preimages, explicit absence, finite funding and frozen retry.
use super::*;
use crate::state::{StateReadOnly, World};
use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
use iroha_model_base::state_path::StatePath;
use std::sync::OnceLock;

fn key(text: &str) -> StatePath {
    text.parse().unwrap()
}
fn tip() -> NativeExecutionTip {
    static TIP: OnceLock<NativeExecutionTip> = OnceLock::new();
    *TIP.get_or_init(|| {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1000)).unwrap();
        chain.commit_at(2000, Vec::new());
        let tip = chain.state().view().native_execution_tip().unwrap();
        tip
    })
}
fn seed(world: &World, values: &[(&str, Vec<u8>)]) {
    let mut block = world.block();
    for (name, value) in values {
        block.smart_contract_state.insert(key(name), value.clone());
    }
    let captured = WorldStateAccumulator::capture(&block).unwrap();
    *block.state_accumulator.get_mut() = captured;
    block.commit();
}
fn prepare(
    capture: &JournalCapture,
    world: &mut WorldBlock<'_>,
    budget: &AllocationBudget,
) -> ChargedShared<CutCapsule> {
    world.advance_state_accumulator(false).unwrap();
    world.begin_freeze();
    world.finish_freeze();
    capture.prepare(world, tip(), 2, budget).unwrap()
}

#[test]
fn touched_at_result_uses_current_value_instead_of_block_predecessor() {
    let world = World::new();
    seed(&world, &[("app/a", vec![1])]);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    block.smart_contract_state.insert(key("app/a"), vec![2]);
    let original = JournalCapture::capture(&block, false, &budget).unwrap();
    block.smart_contract_state.insert(key("app/a"), vec![3]);
    let capsule = prepare(&original, &mut block, &budget);
    let row = capsule.changes().next().unwrap();
    assert_eq!(capsule.changes().len(), 1);
    assert_eq!(row.3, Some(hash_value(&vec![2_u8]).unwrap()));
    assert_eq!(row.4, Some(hash_value(&vec![3_u8]).unwrap()));
    assert_eq!(capsule.root, original.root);
    assert_ne!(capsule.root, capsule.applied_root);
}

#[test]
fn newly_touched_tail_insert_has_explicit_absent_result_preimage() {
    let world = World::new();
    seed(&world, &[]);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    let original = JournalCapture::capture(&block, false, &budget).unwrap();
    block.smart_contract_state.insert(key("app/tail"), vec![7]);
    let capsule = prepare(&original, &mut block, &budget);
    let row = capsule.changes().next().unwrap();
    assert_eq!(row.3, None);
    assert_eq!(row.4, Some(hash_value(&vec![7_u8]).unwrap()));
    assert_eq!(capsule.applied_entries, capsule.entries + 1);
}

#[test]
fn result_deletion_then_tail_reinsert_never_recovers_block_before_value() {
    let world = World::new();
    seed(&world, &[("app/a", vec![1])]);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    block.smart_contract_state.remove(key("app/a"));
    let original = JournalCapture::capture(&block, false, &budget).unwrap();
    assert_eq!(original.rows.as_slice()[0].after, None);
    block.smart_contract_state.insert(key("app/a"), vec![3]);
    let capsule = prepare(&original, &mut block, &budget);
    assert_eq!(capsule.changes().next().unwrap().3, None);
    assert_eq!(capsule.applied_entries, capsule.entries + 1);
}

#[test]
fn no_op_touch_at_result_is_retained_before_tail_delete() {
    let world = World::new();
    seed(&world, &[("app/a", vec![1])]);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    block.smart_contract_state.insert(key("app/a"), vec![1]);
    let original = JournalCapture::capture(&block, false, &budget).unwrap();
    assert_eq!(original.rows.as_slice().len(), 1);
    assert_eq!(original.rows.as_slice()[0].before, None);
    assert_eq!(
        original.rows.as_slice()[0].after,
        Some(hash_value(&vec![1_u8]).unwrap())
    );
    block.smart_contract_state.remove(key("app/a"));
    let capsule = prepare(&original, &mut block, &budget);
    let row = capsule.changes().next().unwrap();
    assert_eq!(row.3, Some(hash_value(&vec![1_u8]).unwrap()));
    assert_eq!(row.4, None);
    assert_eq!(capsule.entries, capsule.applied_entries + 1);
}

#[test]
fn absent_to_absent_touch_at_result_remains_explicit_before_tail_insert() {
    let world = World::new();
    seed(&world, &[]);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    block.smart_contract_state.insert(key("app/a"), vec![1]);
    block.smart_contract_state.remove(key("app/a"));
    let original = JournalCapture::capture(&block, false, &budget).unwrap();
    assert_eq!(original.rows.as_slice().len(), 1);
    assert_eq!(original.rows.as_slice()[0].before, None);
    assert_eq!(original.rows.as_slice()[0].after, None);
    assert_eq!(
        original.rows.as_slice()[0].key,
        Some(hash_value(&key("app/a")).unwrap())
    );
    block.smart_contract_state.insert(key("app/a"), vec![2]);
    let capsule = prepare(&original, &mut block, &budget);
    let row = capsule.changes().next().unwrap();
    assert_eq!(capsule.changes().len(), 1);
    assert_eq!(row.3, None);
    assert_eq!(row.4, Some(hash_value(&vec![2_u8]).unwrap()));
    assert_eq!(capsule.applied_entries, capsule.entries + 1);
}

#[test]
fn untouched_result_entry_deleted_by_tail_uses_native_journal_before() {
    let world = World::new();
    seed(&world, &[("app/a", vec![1])]);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    let original = JournalCapture::capture(&block, false, &budget).unwrap();
    assert!(original.rows.as_slice().is_empty());
    block.smart_contract_state.remove(key("app/a"));
    let capsule = prepare(&original, &mut block, &budget);
    assert_eq!(
        capsule.changes().next().unwrap().3,
        Some(hash_value(&vec![1_u8]).unwrap())
    );
    assert_eq!(capsule.changes().next().unwrap().4, None);
}

#[test]
fn canonical_cell_and_tail_no_op_use_complete_semantic_projection() {
    let world = World::new();
    seed(&world, &[]);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    *block.soradns_last_publish_ms.get_mut() = Some(4);
    let original = JournalCapture::capture(&block, false, &budget).unwrap();
    *block.soradns_last_publish_ms.get_mut() = Some(9);
    block
        .smart_contract_state
        .insert(key("app/transient"), vec![1]);
    block.smart_contract_state.remove(key("app/transient"));
    let capsule = prepare(&original, &mut block, &budget);
    assert_eq!(capsule.changes().len(), 1);
    let row = capsule.changes().next().unwrap();
    assert_eq!(row.0, "world.soradns_last_publish_ms");
    assert_eq!(row.1, WorldStateElementKindV1::Cell);
    assert_eq!(row.2, None);
    assert_eq!(row.3, Some(hash_value(&Some(4_u64)).unwrap()));
    assert_eq!(row.4, Some(hash_value(&Some(9_u64)).unwrap()));
}

#[test]
fn original_capture_budget_refusal_is_local_and_refunds_exact_pool() {
    let world = World::new();
    seed(&world, &[]);
    let block = world.block();
    let budget = AllocationBudget::new(0);
    assert!(matches!(
        JournalCapture::capture(&block, false, &budget),
        Err(CutError::Deferred(_))
    ));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn frozen_preparation_refusal_retries_same_original_and_releases_shared_charges() {
    let world = World::new();
    seed(&world, &[("app/a", vec![1])]);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    block.smart_contract_state.insert(key("app/a"), vec![2]);
    let original = JournalCapture::capture(&block, false, &budget).unwrap();
    let original_root = original.root;
    block.smart_contract_state.insert(key("app/a"), vec![3]);
    block.advance_state_accumulator(false).unwrap();
    block.begin_freeze();
    block.finish_freeze();
    let refused = AllocationBudget::new(0);
    assert!(matches!(
        original.prepare(&block, tip(), 2, &refused),
        Err(CutError::Deferred(_))
    ));
    assert_eq!(refused.reserved_bytes(), 0);
    assert_eq!(original.root, original_root);
    let capsule = original.prepare(&block, tip(), 2, &budget).unwrap();
    let retained = capsule.clone();
    drop(capsule);
    assert!(budget.reserved_bytes() > 0);
    drop(retained);
    drop(original);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn frozen_capsule_rejects_corrupt_applied_accumulator_and_count() {
    let world = World::new();
    seed(&world, &[]);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    let original = JournalCapture::capture(&block, false, &budget).unwrap();
    block.advance_state_accumulator(false).unwrap();
    block.state_accumulator.get_mut().entries += 1;
    block.begin_freeze();
    block.finish_freeze();
    assert!(matches!(
        original.prepare(&block, tip(), 2, &budget),
        Err(CutError::Invalid(_))
    ));
    drop(original);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn replacement_cut_is_derived_from_exact_reverted_world_journal() {
    let world = World::new();
    seed(&world, &[("app/a", vec![1])]);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    {
        let mut first = world.block();
        first.smart_contract_state.insert(key("app/a"), vec![2]);
        first.advance_state_accumulator(false).unwrap();
        first.commit();
    }
    let mut replacement = world.block_and_revert();
    assert_eq!(
        replacement.smart_contract_state.get(&key("app/a")),
        Some(&vec![1])
    );
    replacement
        .smart_contract_state
        .insert(key("app/replacement"), vec![5]);
    let original = JournalCapture::capture(&replacement, false, &budget).unwrap();
    replacement
        .smart_contract_state
        .insert(key("app/replacement"), vec![6]);
    let capsule = prepare(&original, &mut replacement, &budget);
    assert_eq!(capsule.changes().len(), 1);
    assert_eq!(
        capsule.changes().next().unwrap().3,
        Some(hash_value(&vec![5_u8]).unwrap())
    );
}

#[test]
fn musubi_tail_uses_canonical_semantic_anchor_and_excludes_runtime_cache_rows() {
    use iroha_data_model::musubi::{
        ArchiveId, MusubiArchiveAvailabilityV1, MusubiStorageAvailabilityV1,
    };
    let world = World::new();
    let archive = ArchiveId::new([0x41; 32]);
    let original = MusubiArchiveAvailabilityV1 {
        archive_id: archive,
        availability: MusubiStorageAvailabilityV1::Unavailable,
        healthy_replicas: 0,
        active_locations: 0,
        finalized_height: 7,
        finalized_block_hash: [0x42; 32],
        index_revision: 9,
    };
    original.validate().unwrap();
    {
        let mut seed = world.block();
        seed.musubi_archive_availability.insert(archive, original);
        let accumulator = WorldStateAccumulator::capture(&seed).unwrap();
        *seed.state_accumulator.get_mut() = accumulator;
        seed.commit();
    }
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    let cut = JournalCapture::capture(&block, false, &budget).unwrap();
    let mut derived = original;
    derived.availability = MusubiStorageAvailabilityV1::BelowQuorum;
    derived.healthy_replicas = 1;
    derived.active_locations = 1;
    derived.validate().unwrap();
    block.musubi_archive_availability.insert(archive, derived);
    let capsule = prepare(&cut, &mut block, &budget);
    assert_eq!(capsule.changes().len(), 0);
    assert_eq!(capsule.root, capsule.applied_root);
}

#[test]
fn touched_journal_capture_storage_cost_is_independent_of_untouched_world_entries() {
    let world = World::new();
    {
        let mut seed = world.block();
        for i in 0..1000 {
            seed.smart_contract_state
                .insert(key(&format!("app/k{i}")), vec![7; 16]);
        }
        let accumulator = WorldStateAccumulator::capture(&seed).unwrap();
        *seed.state_accumulator.get_mut() = accumulator;
        seed.commit();
    }
    let budget = AllocationBudget::new(8192);
    let mut block = world.block();
    block
        .smart_contract_state
        .insert(key("app/k3"), vec![8; 16]);
    let cut = JournalCapture::capture(&block, false, &budget).unwrap();
    assert_eq!(cut.rows.as_slice().len(), 1);
    assert_eq!(budget.reserved_bytes(), std::mem::size_of::<JournalRow>());
    drop(cut);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn trigger_tail_projects_authoritative_action_and_excludes_derived_indexes() {
    use crate::smartcontracts::isi::triggers::specialized::{
        SpecializedAction, SpecializedTrigger,
    };
    use iroha_data_model::{events::execute_trigger::ExecuteTriggerEventFilter, prelude::*};
    let world = World::new();
    seed(&world, &[]);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    let cut = JournalCapture::capture(&block, false, &budget).unwrap();
    {
        let mut tx = block.triggers.transaction();
        let action = SpecializedAction::new(
            Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "native cut action".into(),
                ))]
                .into(),
            ),
            Repeats::Exactly(3),
            iroha_test_samples::ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        assert!(
            tx.add_by_call_trigger(SpecializedTrigger::new(
                "native_cut".parse().unwrap(),
                action
            ))
            .unwrap()
        );
        tx.apply();
    }
    let complete = WorldStateAccumulator::capture(&block).unwrap();
    assert_eq!(complete.entries(), cut.entries + 1);
    let capsule = prepare(&cut, &mut block, &budget);
    assert_eq!(capsule.changes().len(), 1);
    let row = capsule.changes().next().unwrap();
    assert_eq!(row.0, "triggers.by_call");
    assert_eq!(row.3, None);
    assert!(row.4.is_some());
    assert_eq!(capsule.entries + 1, capsule.applied_entries);
}
