//! Actual control admission, reuse and cleanup at the native policy decoder boundary.
use super::*;
use iroha_allocation::{AllocationRefusal, release::ReleaseRegistration};
use std::task::{Context, Poll, Waker};

fn demand() -> usize {
    ChargedShared::<Mutex<PreparedDecodeWorkspace>>::allocation_layout().size()
        + PreparedDecodeWorkspace::allocation_layouts()
            .iter()
            .map(std::alloc::Layout::size)
            .sum::<usize>()
}

#[test]
fn native_policy_controls_admit_one_atomic_original_demand_without_self_refund() {
    let pool = AllocationBudget::new(demand());
    let external = pool.try_reserve_bytes(1).unwrap();
    let refusal = PolicyDecodeControls::try_new(&pool)
        .err()
        .expect("original atomic demand cannot fit");
    let Some(AllocationRefusal::Capacity {
        requested_bytes,
        release,
        ..
    }) = refusal.allocation_refusal()
    else {
        panic!("retain the original capacity source");
    };
    assert_eq!(*requested_bytes, demand());
    assert_eq!(pool.reserved_bytes(), 1);
    let watch = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
    let mut credit = watch
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut credit).unwrap();
    registration.bind_thread(std::thread::current());
    assert_eq!(
        registration.poll_wait(release, &mut Context::from_waker(Waker::noop())),
        Poll::Pending
    );
    drop(external);
    assert_eq!(
        registration.poll_wait(release, &mut Context::from_waker(Waker::noop())),
        Poll::Ready(())
    );
    let original = PolicyDecodeControls::try_new(&pool).unwrap();
    assert_eq!(pool.reserved_bytes(), demand());
    drop(original);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn native_policy_controls_reuse_original_backing_at_full_capacity_and_reject_foreign_pool() {
    let pool = AllocationBudget::new(demand());
    let original = PolicyDecodeControls::try_new(&pool).unwrap();
    let clone = original.clone();
    assert!(ChargedShared::ptr_eq(&original.0, &clone.0));
    for value in [3_u64, 99, u64::MAX] {
        let bytes = norito::encode_canonical(&value).unwrap();
        for control in PreparedDecodeWorkspace::allocation_layouts() {
            let (result, replaced) =
                crate::test_allocations::refuse_layout_occurrence_during(control, 0, || {
                    clone.decode::<u64>(&pool, &bytes, norito::canonical_decode_limits(bytes.len()))
                });
            assert_eq!(result.unwrap(), value);
            assert!(
                !replaced,
                "native policy reads must reuse their original physical controls"
            );
            assert_eq!(pool.reserved_bytes(), demand());
        }
        let foreign = AllocationBudget::new(demand());
        let error = original
            .decode::<u64>(
                &foreign,
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
            )
            .unwrap_err();
        assert!(
            matches!(canonical_decode_control_attempt_error(error, |error| error), ExecutionAttemptError::Deferred(reason) if reason.reason() == ExecutionDeferral::LocalInvariantViolation)
        );
        assert_eq!(foreign.reserved_bytes(), 0);
        assert!(
            matches!(
                original.decode::<u64>(
                    &foreign,
                    &bytes[..4],
                    norito::canonical_decode_limits(bytes.len())
                ),
                Err(norito::CanonicalDecodeControlError::Decode(_))
            ),
            "the ordinary malformed header precedes source-control admission"
        );
    }
    drop(original);
    assert_eq!(pool.reserved_bytes(), demand());
    drop(clone);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn native_policy_control_construction_preserves_each_physical_allocator_refusal() {
    let layouts = PreparedDecodeWorkspace::allocation_layouts();
    for (layout, occurrence) in [
        (layouts[0], 0),
        (layouts[1], 1),
        (
            ChargedShared::<Mutex<PreparedDecodeWorkspace>>::allocation_layout(),
            0,
        ),
    ] {
        let pool = AllocationBudget::new(demand());
        let (result, refused) =
            crate::test_allocations::refuse_layout_occurrence_during(layout, occurrence, || {
                PolicyDecodeControls::try_new(&pool)
            });
        assert!(refused);
        let original = result
            .err()
            .expect("actual policy backing allocation must refuse");
        assert!(
            matches!(original.allocator_refusal().unwrap().shared_error(), Some(PrepaidSharedError::Allocator { requested_bytes }) if *requested_bytes == layout.size())
        );
        assert_eq!(
            pool.reserved_bytes(),
            0,
            "all actual partial controls retire before refund"
        );
    }
}
