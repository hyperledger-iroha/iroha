//! Snapshot error transport retains the original beacon admission owner for retry.

use super::*;
use crate::{beacon::GlobalThresholdBeaconSessionError, state::deserialize::StateRestoreError};
use iroha_allocation::{AllocationBudget, AllocationRefusal, release::ReleaseRegistration};
use std::task::{Context, Poll, Waker};

#[test]
fn restore_beacon_refusal_retains_original_source_and_retry_credit() {
    let registration_bytes = ReleaseRegistration::allocation_layout().size();
    let budget = AllocationBudget::new(registration_bytes + 64);
    let mut registration = crate::unit_test_support::release_registration(&budget);
    let occupied = budget.try_reserve_bytes(64).unwrap();
    let original = budget.try_reserve_bytes(64).unwrap_err();
    let converted = TryReadError::from(StateRestoreError::BeaconSession(
        GlobalThresholdBeaconSessionError::Admission(original.clone()),
    ));
    let source = std::error::Error::source(&converted)
        .unwrap()
        .downcast_ref::<GlobalThresholdBeaconSessionError>()
        .expect("snapshot failure keeps its concrete beacon resource cause");
    let GlobalThresholdBeaconSessionError::Admission(actual) = source else {
        panic!("local memory pressure must not become invalid snapshot data");
    };
    assert_eq!(actual, &original);
    let AllocationRefusal::Capacity { release, .. } = actual else {
        panic!("the failed original pool must supply its own release observation");
    };
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(registration.poll_wait(release, &mut context), Poll::Pending);
    let foreign = AllocationBudget::new(64);
    drop(foreign.try_reserve_bytes(64).unwrap());
    assert_eq!(registration.poll_wait(release, &mut context), Poll::Pending);
    assert_eq!(budget.reserved_bytes(), registration_bytes + 64);
    drop(occupied);
    assert_eq!(
        registration.poll_wait(release, &mut context),
        Poll::Ready(())
    );
    registration.cancel();
    let retry = budget.try_reserve_bytes(64).unwrap();
    assert_eq!(budget.reserved_bytes(), registration_bytes + 64);
    drop(retry);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}
