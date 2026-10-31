//! Actual admission, physical failure and retained-reader controls for privacy custody.

use super::super::tests::{merkle_status_with_witness, status_with_commitments};
use super::*;
use iroha_crypto::privacy::MerkleCommitment;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

fn statuses() -> [LaneManifestStatus; 2] {
    std::array::from_fn(|index| {
        let count = if index == 0 { 3 } else { 7 };
        status_with_commitments(
            LaneId::new(index as u32 + 1),
            DataSpaceId::new(12),
            (0..count)
                .rev()
                .map(|id| {
                    LanePrivacyCommitment::merkle(
                        LaneCommitmentId::new(id),
                        MerkleCommitment::from_root_bytes([id as u8; 32], 8),
                    )
                })
                .collect(),
        )
    })
}

#[test]
fn privacy_owner_charges_exact_graph_and_retains_it_until_last_reader() {
    let pool = AllocationBudget::new(1024 * 1024);
    let foreign = AllocationBudget::new(1024 * 1024);
    let source = statuses();
    let owner = LanePrivacyRegistry::try_from_statuses(&source, &pool).unwrap();
    let expected = ChargedShared::<PrivacyEntries>::allocation_layout().size()
        + Layout::array::<LanePrivacyLane>(2).unwrap().size()
        + Layout::array::<LanePrivacyCommitment>(3).unwrap().size()
        + Layout::array::<LanePrivacyCommitment>(7).unwrap().size();
    assert_eq!(
        pool.reserved_bytes(),
        expected,
        "scratch is already refunded"
    );
    assert!(owner.belongs_to(&pool));
    assert!(!owner.belongs_to(&foreign));
    let mut reader = None;
    let allocations = crate::test_allocations::allocations_during(|| reader = Some(owner.clone()));
    assert_eq!(allocations, 0);
    let reader = reader.unwrap();
    assert!(LanePrivacyRegistry::ptr_eq(&owner, &reader));
    drop(source);
    drop(owner);
    assert_eq!(pool.reserved_bytes(), expected);
    let entry = reader.lane(LaneId::new(2)).unwrap();
    assert_eq!(
        entry
            .commitments()
            .map(|value| value.id())
            .collect::<Vec<_>>(),
        (0..7).map(LaneCommitmentId::new).collect::<Vec<_>>()
    );
    drop(reader);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(foreign.reserved_bytes(), 0);
}

#[test]
fn privacy_owner_each_actual_allocator_failure_refunds_partial_graph_and_retries() {
    let pool = AllocationBudget::new(1024 * 1024);
    let source = statuses();
    // The exact physical allocation order, including the transient borrowed pointers.
    let layouts = [
        Layout::array::<(usize, &LaneManifestStatus)>(2).unwrap(),
        ChargedShared::<PrivacyEntries>::allocation_layout(),
        Layout::array::<LanePrivacyLane>(2).unwrap(),
        Layout::array::<LanePrivacyCommitment>(3).unwrap(),
        Layout::array::<LanePrivacyCommitment>(7).unwrap(),
    ];
    for (index, layout) in layouts.into_iter().enumerate() {
        let occurrence = layouts[..index]
            .iter()
            .filter(|earlier| **earlier == layout)
            .count();
        let (result, observed) =
            crate::test_allocations::refuse_layout_occurrence_during(layout, occurrence, || {
                LanePrivacyRegistry::try_from_statuses(&source, &pool)
            });
        assert!(observed, "must reach original allocation {index}");
        let original = result.unwrap_err();
        assert!(
            original.allocation_refusal().is_none(),
            "allocator failure has no invented capacity wake"
        );
        let allocator = original
            .allocator_refusal()
            .expect("original physical allocator cause");
        if index == 1 {
            assert!(
                matches!(allocator.shared_error(), Some(PrepaidSharedError::Allocator { requested_bytes }) if *requested_bytes == layout.size())
            );
        } else {
            assert!(
                matches!(allocator.buffer_error(), Some(ChargedBufferError::Allocator { requested_bytes }) if *requested_bytes == layout.size())
            );
        }
        assert_eq!(
            pool.reserved_bytes(),
            0,
            "every earlier physical allocation released"
        );
        let retry = LanePrivacyRegistry::try_from_statuses(&source, &pool).unwrap();
        assert_eq!(retry.lane(LaneId::new(2)).unwrap().commitments().count(), 7);
        drop(retry);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn privacy_owner_original_pool_refusal_preserves_input_and_same_input_retry() {
    let source = statuses();
    let pool = AllocationBudget::new(1024 * 1024);
    let scratch = Layout::array::<(usize, &LaneManifestStatus)>(2)
        .unwrap()
        .size();
    let graph = ChargedShared::<PrivacyEntries>::allocation_layout().size()
        + Layout::array::<LanePrivacyLane>(2).unwrap().size()
        + Layout::array::<LanePrivacyCommitment>(3).unwrap().size()
        + Layout::array::<LanePrivacyCommitment>(7).unwrap().size();
    pool.set_limit_bytes(graph + scratch);
    let competing = pool.try_reserve_bytes(1).unwrap();
    let original = LanePrivacyRegistry::try_from_statuses(&source, &pool).unwrap_err();
    assert!(
        matches!(original.allocation_refusal(), Some(AllocationRefusal::Capacity {
        requested_bytes, reserved_bytes, limit_bytes, ..
    }) if *requested_bytes == graph && *reserved_bytes == scratch + 1 && *limit_bytes == graph + scratch)
    );
    assert!(original.allocator_refusal().is_none());
    assert_eq!(source[1].privacy_commitments.len(), 7);
    assert_eq!(pool.reserved_bytes(), 1);
    drop(competing);
    let retry = LanePrivacyRegistry::try_from_statuses(&source, &pool).unwrap();
    assert!(retry.belongs_to(&pool));
    drop(retry);
    assert_eq!(pool.reserved_bytes(), 0);
    pool.set_limit_bytes(0);
    let no_capacity = LanePrivacyRegistry::try_from_statuses(&source, &pool).unwrap_err();
    assert!(matches!(
        no_capacity.allocation_refusal(),
        Some(AllocationRefusal::ExceedsLimit { limit_bytes: 0, .. })
    ));
    let empty =
        LanePrivacyRegistry::try_from_statuses(std::iter::empty::<&LaneManifestStatus>(), &pool)
            .unwrap();
    assert!(empty.is_empty());
    assert_eq!(pool.reserved_bytes(), 0);
}

struct AfterUnlock {
    lock: Arc<Mutex<()>>,
    woke: AtomicBool,
}
impl std::task::Wake for AfterUnlock {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        assert!(
            self.lock.try_lock().is_ok(),
            "refund wake must follow original physical unlock"
        );
        self.woke.store(true, Ordering::SeqCst);
    }
}

#[test]
fn privacy_last_reader_refund_waits_for_enclosing_physical_guard_and_retains_valid_proof() {
    let pool = AllocationBudget::new(1024 * 1024);
    let id = LaneCommitmentId::new(5);
    let (status, witness) = merkle_status_with_witness(LaneId::new(7), DataSpaceId::new(99), id);
    let owner = LanePrivacyRegistry::try_from_statuses([&status], &pool).unwrap();
    owner
        .verify(status.lane, id, PrivacyWitness::Merkle(witness))
        .unwrap();
    let reader = owner.clone();
    drop(owner);
    let charge = pool.reserved_bytes();
    let wait = match pool.try_reserve_bytes(pool.limit_bytes()).unwrap_err() {
        AllocationRefusal::Capacity { release, .. } => release,
        other => panic!("expected original occupied pool: {other:?}"),
    };
    let waiter_pool = AllocationBudget::new(4096);
    let mut prepaid = waiter_pool
        .try_reserve(iroha_allocation::release::ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration =
        iroha_allocation::release::ReleaseRegistration::from_reservation(&mut prepaid).unwrap();
    let lock = Arc::new(Mutex::new(()));
    let observer = Arc::new(AfterUnlock {
        lock: lock.clone(),
        woke: AtomicBool::new(false),
    });
    let waker = std::task::Waker::from(observer.clone());
    assert!(
        registration
            .poll_wait(&wait, &mut std::task::Context::from_waker(&waker))
            .is_pending()
    );
    pool.with_deferred_refund_notifications(|_| {
        let _physical_guard = lock.lock().unwrap();
        assert_eq!(pool.reserved_bytes(), charge);
        drop(reader);
        assert_eq!(pool.reserved_bytes(), 0);
        assert!(!observer.woke.load(Ordering::SeqCst));
    });
    assert!(observer.woke.load(Ordering::SeqCst));
}
