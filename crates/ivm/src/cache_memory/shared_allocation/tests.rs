//! Original admission, retained borrowers and final shared-allocation release.

use super::*;
use crate::{VMError, error::ExecutionDeferral};
use iroha_allocation::{AllocationRefusal, release::ReleaseRegistration};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Barrier},
    task::{Context, Poll, Waker},
};

fn demand<T>(len: usize) -> usize {
    std::alloc::Layout::array::<T>(len).unwrap().size()
        + ChargedShared::<Allocation<T>>::allocation_layout().size()
}

#[test]
fn funded_iterator_reserves_before_evaluating_and_keeps_the_original_pool() {
    let bytes = demand::<u64>(2);
    let budget = AllocationBudget::new(0);
    let evaluated = std::cell::Cell::new(0);
    let values = || {
        [7_u64, 8].into_iter().map(|value| {
            evaluated.set(evaluated.get() + 1);
            Ok::<_, VMError>(value)
        })
    };
    assert!(matches!(
        SharedAllocation::try_from_iter_with_memory_budget(values(), &budget),
        Err(VMError::AllocationDeferred(
            AllocationRefusal::ExceedsLimit { .. }
        ))
    ));
    assert_eq!(evaluated.get(), 0);
    assert_eq!(budget.peak_reserved_bytes(), 0);
    budget.set_limit_bytes(bytes);
    let occupied = budget.try_reserve_bytes(bytes).unwrap();
    assert!(matches!(
        SharedAllocation::try_from_iter_with_memory_budget(values(), &budget),
        Err(VMError::AllocationDeferred(
            AllocationRefusal::Capacity { .. }
        ))
    ));
    assert_eq!(evaluated.get(), 0);
    drop(occupied);
    let owner = SharedAllocation::try_from_iter_with_memory_budget(values(), &budget).unwrap();
    assert_eq!(evaluated.get(), 2);
    assert!(owner.belongs_to(&budget.clone()));
    assert!(!owner.belongs_to(&AllocationBudget::new(bytes)));
    let borrower = owner.clone();
    drop(owner);
    assert_eq!(budget.reserved_bytes(), bytes);
    assert!(borrower.belongs_to(&budget));
    drop(borrower);
    assert_eq!(budget.reserved_bytes(), 0);
    let diagnostic: SharedAllocation<u64> = vec![7, 8].into();
    assert!(!diagnostic.belongs_to(&budget));
}

#[test]
fn funded_iterator_refunds_partial_values_and_rejects_inexact_lengths() {
    struct Inexact {
        values: std::array::IntoIter<Result<u64, VMError>, 2>,
        advertised: usize,
    }
    impl Iterator for Inexact {
        type Item = Result<u64, VMError>;
        fn next(&mut self) -> Option<Self::Item> {
            self.values.next()
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.advertised, Some(self.advertised))
        }
    }
    impl ExactSizeIterator for Inexact {}

    let budget = AllocationBudget::new(demand::<u64>(3));
    let retention = MemoryBudget::new(0);
    for advertised in [1, 3] {
        let values = Inexact {
            values: [Ok(7), Ok(8)].into_iter(),
            advertised,
        };
        assert!(matches!(
            SharedAllocation::try_from_iter_with_budgets(values, &budget, &retention),
            Err(VMError::DecodeError)
        ));
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(retention.stats().measured_resident_bytes(), 0);
    }
    assert!(matches!(
        SharedAllocation::try_from_iter_with_budgets(
            [Ok(7_u64), Err(VMError::InvalidMetadata)].into_iter(),
            &budget,
            &retention,
        ),
        Err(VMError::InvalidMetadata)
    ));
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(retention.stats().measured_resident_bytes(), 0);
}

#[test]
fn funded_copy_refuses_before_allocation_and_preserves_original_retry_source() {
    let bytes = demand::<u64>(2);
    let budget = AllocationBudget::new(0);
    let retention = MemoryBudget::new(bytes);
    assert!(matches!(
        SharedAllocation::try_copy_with_budgets(&[7_u64, 8], &budget, &retention),
        Err(VMError::AllocationDeferred(AllocationRefusal::ExceedsLimit {
            requested_bytes,
            limit_bytes: 0,
        })) if requested_bytes == bytes
    ));
    assert_eq!(budget.peak_reserved_bytes(), 0);
    assert_eq!(retention.stats().peak_reserved_bytes, 0);
    let registration_bytes = ReleaseRegistration::allocation_layout().size();
    budget.set_limit_bytes(bytes + registration_bytes);
    let mut registration = ReleaseRegistration::from_reservation(
        &mut budget
            .try_reserve(ReleaseRegistration::allocation_layout())
            .unwrap(),
    )
    .unwrap();
    assert!(registration.belongs_to(&budget));
    let occupied = budget.try_reserve_bytes(bytes).unwrap();
    let Err(VMError::AllocationDeferred(AllocationRefusal::Capacity { release, .. })) =
        SharedAllocation::try_copy_with_budgets(&[7_u64, 8], &budget, &retention)
    else {
        panic!("original pool must supply capacity refusal");
    };
    let mut wait = release.wait_for_release(&mut registration);
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Pending);
    let unrelated = AllocationBudget::new(bytes);
    drop(unrelated.try_reserve_bytes(bytes).unwrap());
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Pending);
    drop(occupied);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Ready(()));
    drop(wait);
    assert_eq!(budget.reserved_bytes(), registration_bytes);
    let owner = SharedAllocation::try_copy_with_budgets(&[7_u64, 8], &budget, &retention)
        .expect("same original pool retries after release");
    assert_eq!(&*owner, &[7, 8]);
    assert_eq!(budget.reserved_bytes(), bytes + registration_bytes);
    assert_eq!(retention.stats().measured_resident_bytes(), bytes);
    assert_eq!(retention.stats().peak_reserved_bytes, bytes);
    drop(owner);
    assert_eq!(budget.reserved_bytes(), registration_bytes);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(retention.stats().measured_resident_bytes(), 0);
}

#[test]
fn funded_shared_control_and_payload_live_through_evicted_borrowers() {
    let bytes = demand::<u8>(3);
    let budget = AllocationBudget::new(bytes);
    let retention = MemoryBudget::new(bytes);
    let owner =
        SharedAllocation::try_copy_with_budgets(&[1_u8, 2, 3], &budget, &retention).unwrap();
    let first = owner.clone();
    let second = owner.clone();
    assert!(SharedAllocation::ptr_eq(&first, &second));
    let local =
        SharedAllocation::with_budget(vec![1_u8, 2, 3].into_boxed_slice(), &MemoryBudget::new(0));
    assert!(!SharedAllocation::ptr_eq(&first, &local));
    assert!(owner.try_retain());
    let cached = owner.into_cache_owner();
    assert_eq!(retention.stats().shared_borrowed_bytes, bytes);
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(cached);
    assert_eq!(retention.stats().shared_evicted_live_bytes, bytes);
    budget.set_limit_bytes(0);
    drop(first);
    assert_eq!(&*second, &[1, 2, 3]);
    assert_eq!(budget.reserved_bytes(), bytes);
    assert_eq!(retention.stats().retained_bytes, bytes);
    drop(second);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(retention.stats().measured_resident_bytes(), 0);
}

#[test]
fn zero_retention_keeps_funded_cold_values_and_zero_length_still_funds_control() {
    let bytes = demand::<u8>(0);
    let budget = AllocationBudget::new(bytes);
    let retention = MemoryBudget::new(0);
    let owner = SharedAllocation::try_copy_with_budgets(&[], &budget, &retention).unwrap();
    let _: &[u8] = &owner;
    assert!(!owner.try_retain());
    assert_eq!(budget.reserved_bytes(), bytes);
    assert_eq!(retention.stats().active_bytes, bytes);
    let borrower = owner.clone();
    drop(owner);
    assert!(borrower.is_empty());
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(borrower);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn admitted_control_and_buffer_allocation_failures_refund_before_retry() {
    let bytes = demand::<u32>(3);
    let budget = AllocationBudget::new(bytes);
    let retention = MemoryBudget::new(bytes);
    for buffer in [false, true] {
        if buffer {
            REFUSE_FUNDED_BUFFER.set(true);
        } else {
            REFUSE_NEXT_SHARED_ALLOCATION.set(true);
        }
        assert!(matches!(
            SharedAllocation::try_copy_with_budgets(&[2_u32, 3, 5], &budget, &retention),
            Err(VMError::ExecutionDeferred(
                ExecutionDeferral::AllocationUnavailable
            ))
        ));
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(budget.peak_reserved_bytes(), bytes);
        assert_eq!(retention.stats().measured_resident_bytes(), 0);
    }
    let owner =
        SharedAllocation::try_copy_with_budgets(&[2_u32, 3, 5], &budget, &retention).unwrap();
    assert_eq!(&*owner, &[2, 3, 5]);
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn concurrent_final_owners_reclaim_both_charges_once() {
    let bytes = demand::<u64>(4);
    let budget = AllocationBudget::new(bytes);
    let retention = MemoryBudget::new(bytes);
    let owner =
        SharedAllocation::try_copy_with_budgets(&[1_u64, 3, 5, 7], &budget, &retention).unwrap();
    let barrier = Arc::new(Barrier::new(9));
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let borrower = owner.clone();
            let barrier = Arc::clone(&barrier);
            scope.spawn(move || {
                barrier.wait();
                assert_eq!(&*borrower, &[1, 3, 5, 7]);
                drop(borrower);
            });
        }
        drop(owner);
        assert_eq!(budget.reserved_bytes(), bytes);
        barrier.wait();
    });
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(retention.stats().measured_resident_bytes(), 0);
}

#[test]
fn unwind_preserves_live_borrower_and_reclaims_after_its_last_reference() {
    let bytes = demand::<u16>(2);
    let budget = AllocationBudget::new(bytes);
    let retention = MemoryBudget::new(bytes);
    let owner =
        SharedAllocation::try_copy_with_budgets(&[13_u16, 21], &budget, &retention).unwrap();
    let borrower = owner.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _owner = owner;
        panic!("operation unwinds");
    }));
    assert!(result.is_err());
    assert_eq!(&*borrower, &[13, 21]);
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(borrower);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(retention.stats().measured_resident_bytes(), 0);
}
