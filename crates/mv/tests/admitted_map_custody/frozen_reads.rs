//! Frozen typed reads retain real prepaid nodes/payloads through retry and release.

use super::*;
use mv::{BlockMode, PublicationPreparationError};

#[test]
fn frozen_prepaid_reads_and_same_source_retry_allocate_nothing_at_full_capacity() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let budget = AllocationBudget::new(8 << 20);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x11);
    storage
        .try_with_admitted_block(|block| {
            assert!(put(block, &budget, 8, 0x18).is_none());
            Ok::<_, ()>(())
        })
        .unwrap();
    let original = storage
        .try_capture_admitted_block(BlockMode::Ordinary, |block| {
            drop(put(block, &budget, 7, 0x22));
            assert!(put(block, &budget, 10, 0x2a).is_none());
            Ok::<_, ()>(73)
        })
        .unwrap();
    let identity = original.publication_identity();
    let after = original.get(&7).unwrap().pointer();
    let retained = budget.reserved_bytes();
    let check_reads = |original: &mv::storage::Detached<
        Payload,
        Payload,
        usize,
        Prepaid<NativeStoragePolicy>,
    >| {
        assert_eq!(original.publication_identity(), identity);
        assert_eq!(original.len(), 3);
        assert!(!original.is_empty());
        marker(original.get(&7), 0x22);
        marker(original.get(&8), 0x18);
        marker(original.get(&10), 0x2a);
        assert!(original.get(&9).is_none());
        assert_eq!(original.get(&7).unwrap().pointer(), after);
        assert_eq!(original.first_key_value().unwrap().0.order, 7);
        assert_eq!(original.last_key_value().unwrap().0.order, 10);
        let mut range = original.range(7_usize..=8);
        assert_eq!(range.next().unwrap().0.order, 7);
        assert_eq!(range.next_back().unwrap().0.order, 8);
        assert!(range.next().is_none());
        let mut entries = original.iter();
        assert_eq!(entries.len(), 3);
        let key = entries.next().unwrap().0;
        marker(original.get_before_block(key), 0x11);
        let (_, value) = original.get_key_value(key).unwrap();
        assert_eq!(value.pointer(), after);
        let untouched = entries.next().unwrap().0;
        marker(original.get_before_block(untouched), 0x18);
        let new = entries.next().unwrap().0;
        assert!(original.get_before_block(new).is_none());
        assert!(entries.next().is_none());
    };
    let full = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
    without_allocations(|| {
        check_reads(&original);
        budget.with_deferred_refund_notifications(|scope| {
            let mut slot = original
                .try_publication_slot(scope, &storage)
                .unwrap_or_else(|_| panic!("original source pool"));
            slot.try_prepare().unwrap();
            let original = slot.recover_original();
            // Recovery released the acquired physical owners, not the original
            // payloads. The same frozen read surface remains valid at full pool.
            check_reads(&original);
            drop(slot);
            let mut slot = original
                .try_publication_slot(scope, &storage)
                .unwrap_or_else(|_| panic!("same original source pool"));
            slot.try_prepare().unwrap();
            assert_eq!(slot.into_prepared().publish().into_admission(), 73);
        });
    });
    assert_eq!(
        (counters.keys.load(SeqCst), counters.values.load(SeqCst)),
        copies
    );
    assert_eq!(storage.view().get(&7).unwrap().pointer(), after);
    drop(full);
    assert!(
        budget.reserved_bytes() <= retained,
        "original publication does not add payload storage"
    );
    drop(identity);
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn frozen_prepaid_reads_preserve_owner_on_foreign_pool_and_equal_source_refusal() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let budget = AllocationBudget::new(8 << 20);
    let foreign_pool = AllocationBudget::new(budget.limit_bytes());
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    let foreign = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x31);
    seed(&foreign, &budget, 0x31);
    let original = storage
        .try_capture_admitted_block(BlockMode::Ordinary, |block| {
            drop(put(block, &budget, 7, 0x32));
            Ok::<_, ()>(())
        })
        .unwrap();
    let identity = original.publication_identity();
    let pointer = original.get(&7).unwrap().pointer();
    let credit = budget.reserved_bytes();
    let full = budget
        .try_reserve_bytes(budget.limit_bytes() - credit)
        .unwrap();
    let original = without_allocations(|| {
        foreign_pool.with_deferred_refund_notifications(|scope| {
            match original.try_publication_slot(scope, &storage) {
                Err((
                    original,
                    PublicationPreparationError::Admission(AdmittedStorageError::ScopeIdentity),
                )) => original,
                _ => panic!("foreign pool cannot substitute for original custody"),
            }
        })
    });
    let original = without_allocations(|| {
        budget.with_deferred_refund_notifications(|scope| {
            let mut slot = original
                .try_publication_slot(scope, &foreign)
                .unwrap_or_else(|_| panic!("same pool permits source check only"));
            assert_eq!(
                slot.try_prepare(),
                Err(PublicationPreparationError::Changed)
            );
            let original = slot.recover_original();
            assert_eq!(original.publication_identity(), identity);
            assert_eq!(original.get(&7).unwrap().pointer(), pointer);
            marker(original.get(&7), 0x32);
            drop(slot);
            original
        })
    });
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    assert_eq!(foreign_pool.reserved_bytes(), 0);
    drop(full);
    assert_eq!(budget.reserved_bytes(), credit);
    // A real undo-only publication changes the source identity despite retaining
    // the same current value. Frozen reads must not turn this into a Busy retry.
    storage
        .try_with_admitted_block(|_| Ok::<_, ()>(()))
        .unwrap();
    let original = without_allocations(|| {
        budget.with_deferred_refund_notifications(|scope| {
            let mut slot = original
                .try_publication_slot(scope, &storage)
                .unwrap_or_else(|_| panic!("same original source pool"));
            assert_eq!(
                slot.try_prepare(),
                Err(PublicationPreparationError::Changed)
            );
            let original = slot.recover_original();
            assert_eq!(original.get(&7).unwrap().pointer(), pointer);
            assert_eq!(original.publication_identity(), identity);
            drop(slot);
            original
        })
    });
    drop(identity);
    without_allocations(|| drop((original, storage, foreign)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn raw_frozen_images_keep_exact_original_credit_at_full_pool_without_read_allocations() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let budget = AllocationBudget::new(8 << 20);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    let foreign = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x11);
    storage
        .try_with_admitted_block(|block| {
            assert!(put(block, &budget, 8, 0x18).is_none());
            assert!(put(block, &budget, 12, 0x1c).is_none());
            Ok::<_, ()>(())
        })
        .unwrap();
    let original = storage
        .try_capture_admitted_block(BlockMode::Ordinary, |block| {
            drop(put(block, &budget, 7, 0x22));
            drop(block_remove(block, &budget, 8));
            assert!(block_remove(block, &budget, 9).is_none());
            assert!(put(block, &budget, 10, 0x2a).is_none());
            drop(put(block, &budget, 12, 0x1c));
            Ok::<_, ()>(())
        })
        .unwrap();
    let identity = original.publication_identity();
    let pointer = original.get(&7).unwrap().pointer();
    let retained = budget.reserved_bytes();
    let full = budget
        .try_reserve_bytes(budget.limit_bytes() - retained)
        .unwrap();
    let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
    without_allocations(|| {
        let images = original.original_images();
        assert_eq!(images.mode(), BlockMode::Ordinary);
        assert_eq!(images.publication_identity(), identity);
        assert!(images.belongs_to(&storage));
        assert!(!images.belongs_to(&foreign));
        let mut current = images.current_entries();
        assert_eq!(current.len(), 3);
        let (key, value) = current.next().unwrap();
        assert_eq!(key.order, 7);
        assert_eq!(value.pointer(), pointer);
        marker(Some(value), 0x22);
        assert_eq!(current.next().unwrap().0.order, 10);
        assert_eq!(current.next().unwrap().0.order, 12);
        assert!(current.next().is_none());
        let mut undo = images.undo_entries();
        assert_eq!(undo.len(), 5);
        let (key, before) = undo.next().unwrap();
        assert_eq!(key.order, 7);
        marker(before.as_ref(), 0x11);
        let (key, before) = undo.next().unwrap();
        assert_eq!(key.order, 8);
        marker(before.as_ref(), 0x18);
        for expected in [9, 10] {
            let (key, before) = undo.next().unwrap();
            assert_eq!(key.order, expected);
            assert!(before.is_none());
        }
        let (key, before) = undo.next().unwrap();
        assert_eq!(key.order, 12);
        marker(before.as_ref(), 0x1c);
        assert!(undo.next().is_none());
    });
    assert_eq!(
        (counters.keys.load(SeqCst), counters.values.load(SeqCst)),
        copies
    );
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    drop(full);
    assert_eq!(budget.reserved_bytes(), retained);
    drop(identity);
    without_allocations(|| drop((original, storage, foreign)));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}
