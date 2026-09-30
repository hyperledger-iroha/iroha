//! Original World journal cuts, frozen-tail reconstruction and finite-pool refusal.

use super::*;
use crate::{
    state::{StateReadOnly, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_model_base::state_path::StatePath;

fn path(value: &str) -> StatePath {
    value.parse().expect("test state path")
}

fn native_tip() -> NativeExecutionTip {
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000))
        .expect("certified genesis");
    let tip = chain
        .state()
        .view()
        .native_execution_tip()
        .expect("original native execution tip");
    tip
}

#[test]
fn frozen_tail_reconstructs_original_cut_with_absent_and_noop_touches() {
    let world = World::new();
    {
        let mut setup = world.block();
        setup
            .smart_contract_state
            .insert(path("cut/delete"), vec![4]);
        setup.smart_contract_state.insert(path("cut/noop"), vec![3]);
        setup.advance_state_accumulator(true).unwrap();
        setup.commit();
    }
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    block
        .smart_contract_state
        .insert(path("cut/update"), vec![1]);
    block.smart_contract_state.insert(path("cut/noop"), vec![3]);
    block.smart_contract_state.remove(path("cut/absent"));
    *block.soradns_last_publish_ms.get_mut() = Some(11);
    let original = WorldStateAccumulator::capture(&block).unwrap();
    let capture = JournalCapture::capture(&block, false, &budget).unwrap();
    assert_eq!(capture.root, original.root().unwrap());
    assert_eq!(capture.entries, original.entries());
    assert!(capture.rows.as_slice().iter().any(|row| {
        row.key == Some(hash_value(&path("cut/absent")).unwrap())
            && row.before.is_none()
            && row.after.is_none()
    }));
    assert!(capture.rows.as_slice().iter().any(|row| {
        row.key == Some(hash_value(&path("cut/noop")).unwrap())
            && row.before.is_some()
            && row.before == row.after
    }));

    block
        .smart_contract_state
        .insert(path("cut/update"), vec![2]);
    block.smart_contract_state.remove(path("cut/delete"));
    block.smart_contract_state.insert(path("cut/new"), vec![5]);
    block
        .smart_contract_state
        .insert(path("cut/absent"), vec![6]);
    *block.soradns_last_publish_ms.get_mut() = Some(22);
    block.advance_state_accumulator(false).unwrap();
    let applied = WorldStateAccumulator::capture(&block).unwrap();
    let semantic_delta = block.net_state_delta().unwrap();
    let publication_delta = block.publication_state_delta().unwrap();
    let original_storage_value =
        std::ptr::from_ref(block.smart_contract_state.get(&path("cut/update")).unwrap());
    let original_cell_value = std::ptr::from_ref(block.soradns_last_publish_ms.get());
    assert_ne!(original.root(), applied.root());
    block.begin_freeze();
    block.finish_freeze();
    // Both visitors and cold capture read the same original payloads after
    // execution authority is consumed and all physical writers are released.
    assert_eq!(block.net_state_delta().unwrap(), semantic_delta);
    assert_eq!(block.publication_state_delta().unwrap(), publication_delta);
    assert_eq!(WorldStateAccumulator::capture(&block).unwrap(), applied);
    assert_eq!(
        std::ptr::from_ref(block.smart_contract_state.get(&path("cut/update")).unwrap()),
        original_storage_value
    );
    assert_eq!(
        std::ptr::from_ref(block.soradns_last_publish_ms.get()),
        original_cell_value
    );
    let tip = native_tip();
    let capsule = capture.prepare(&block, tip, 8, &budget).unwrap();
    assert_eq!(capsule.tip, tip);
    assert_eq!(capsule.generation, 8);
    assert_eq!(capsule.root, original.root().unwrap());
    assert_eq!(capsule.entries, original.entries());
    assert_eq!(capsule.applied_root, applied.root().unwrap());
    assert_eq!(capsule.applied_entries, applied.entries());
    assert_eq!(capsule.rows.as_slice().len(), 5);
    let changes: Vec<_> = capsule.changes().collect();
    assert!(
        changes
            .windows(2)
            .all(|pair| { (pair[0].0, pair[0].1, pair[0].2) < (pair[1].0, pair[1].1, pair[1].2) })
    );
    for (index, &(id, kind, key, before, after)) in changes.iter().enumerate() {
        assert_eq!(
            capsule.change_for(id, kind, key),
            Some((index, before, after))
        );
        assert_ne!(
            before, after,
            "only actual post-result changes are retained"
        );
    }
    for (name, before, after) in [
        ("cut/update", Some(vec![1_u8]), Some(vec![2_u8])),
        ("cut/delete", Some(vec![4]), None),
        ("cut/new", None, Some(vec![5])),
        ("cut/absent", None, Some(vec![6])),
    ] {
        let (_, observed_before, observed_after) = capsule
            .change_for(
                "world.smart_contract_state",
                WorldStateElementKindV1::Table,
                Some(hash_value(&path(name)).unwrap()),
            )
            .expect("exact tail change");
        assert_eq!(
            observed_before,
            before.as_ref().map(hash_value).transpose().unwrap()
        );
        assert_eq!(
            observed_after,
            after.as_ref().map(hash_value).transpose().unwrap()
        );
    }
    let (_, before, after) = capsule
        .change_for(
            "world.soradns_last_publish_ms",
            WorldStateElementKindV1::Cell,
            None,
        )
        .unwrap();
    assert_eq!(before, Some(hash_value(&Some(11_u64)).unwrap()));
    assert_eq!(after, Some(hash_value(&Some(22_u64)).unwrap()));
    assert!(
        capsule
            .change_for(
                "world.smart_contract_state",
                WorldStateElementKindV1::Table,
                Some(hash_value(&path("cut/noop")).unwrap()),
            )
            .is_none()
    );
    assert!(budget.reserved_bytes() > 0);
    drop(capsule);
    drop(capture);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn journal_cut_rejects_incomplete_registry_and_lost_original_touches() {
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let index = field_index().as_ref().unwrap();
    let mut visitor = JournalVisitor::new(index, &budget, None).unwrap();
    assert!(
        visitor
            .field("smart_contract_state", TABLE)
            .unwrap()
            .is_some()
    );
    assert!(visitor.field("smart_contract_state", TABLE).is_err());
    assert!(visitor.field("parameters", TABLE).is_err());
    assert!(visitor.field("not_registered", TABLE).is_err());
    assert_eq!(visitor.field("domains_by_owner", TABLE).unwrap(), None);
    assert!(visitor.finish().is_err());
    assert_eq!(budget.reserved_bytes(), 0);

    let world = World::new();
    let capture = {
        let mut original = world.block();
        original.smart_contract_state.remove(path("cut/absent"));
        JournalCapture::capture(&original, true, &budget).unwrap()
    };
    // A replacement overlay has the same values but has lost the original
    // absent-to-absent touch. It cannot stand in for the original execution.
    let mut replacement = world.block();
    replacement.advance_state_accumulator(true).unwrap();
    let retained = budget.reserved_bytes();
    assert!(matches!(
        capture.prepare(&replacement, native_tip(), 2, &budget),
        Err(CutError::Invalid(reason)) if reason.contains("original execution touches")
    ));
    assert_eq!(budget.reserved_bytes(), retained);
    drop(capture);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn cut_refusal_refunds_scratch_and_retries_the_original_frozen_journal() {
    let world = World::new();
    let mut block = world.block();
    block
        .smart_contract_state
        .insert(path("cut/value"), vec![1]);
    let empty_budget = AllocationBudget::new(0);
    assert!(matches!(
        JournalCapture::capture(&block, true, &empty_budget),
        Err(CutError::Deferred(_))
    ));
    assert_eq!(empty_budget.reserved_bytes(), 0);

    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut capture = JournalCapture::capture(&block, true, &budget).unwrap();
    block
        .smart_contract_state
        .insert(path("cut/value"), vec![2]);
    block.advance_state_accumulator(true).unwrap();
    block.begin_freeze();
    block.finish_freeze();
    let tip = native_tip();
    let retained = budget.reserved_bytes();
    budget.set_limit_bytes(retained);
    assert!(matches!(
        capture.prepare(&block, tip, 2, &budget),
        Err(CutError::Deferred(_))
    ));
    assert_eq!(budget.reserved_bytes(), retained);
    budget.set_limit_bytes(16 * 1024 * 1024);
    let capsule = capture.prepare(&block, tip, 2, &budget).unwrap();
    assert_eq!(capsule.root, capture.root);
    assert_eq!(capsule.rows.as_slice().len(), 1);
    drop(capsule);
    assert_eq!(budget.reserved_bytes(), retained);

    capture.entries += 1;
    assert!(matches!(
        capture.prepare(&block, tip, 2, &budget),
        Err(CutError::Invalid(reason)) if reason.contains("reconstruct original R/count")
    ));
    assert_eq!(budget.reserved_bytes(), retained);
    drop(capture);
    assert_eq!(budget.reserved_bytes(), 0);
}
