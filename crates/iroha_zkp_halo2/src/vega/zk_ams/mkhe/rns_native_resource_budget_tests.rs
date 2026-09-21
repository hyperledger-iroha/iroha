//! Original resource-owner identity, refusal, lifetime and poison controls.
use super::super::{
    rns_native_proof_hash::RnsNativeProofHashContextV1,
    rns_native_qpcs_leaf::RnsNativeOracleV1,
    rns_native_qpcs_tree::{RnsNativeQpcsTreeV1, RnsNativeTreeErrorV1},
};
use super::*;

fn parameter() -> [u8; 32] {
    RnsNativeProofHashContextV1::canonical()
        .unwrap()
        .parameter_digest()
}

#[test]
fn work_and_live_bytes_are_admitted_atomically_and_only_bytes_are_released() {
    let mut budget = RnsNativeProofResourceBudgetV1::default();
    let reservation = budget
        .admit(7, ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1 - 1, 1)
        .unwrap();
    assert_eq!(budget.consumed().expect("healthy resource ledger"), 7);
    assert_eq!(
        budget.live_bytes().expect("healthy resource ledger"),
        ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1
    );
    assert_eq!(
        budget.peak_bytes().expect("healthy resource ledger"),
        budget.live_bytes().expect("healthy resource ledger")
    );
    assert!(matches!(
        budget.admit(3, 1, 0),
        Err(RnsNativeResourceErrorV1::WorkspaceLimit)
    ));
    assert!(matches!(
        budget.admit(0, u64::MAX, 1),
        Err(RnsNativeResourceErrorV1::ArithmeticOverflow)
    ));
    assert!(matches!(
        budget.admit(u64::MAX, 0, 0),
        Err(RnsNativeResourceErrorV1::ArithmeticOverflow)
    ));
    assert_eq!(budget.consumed().expect("healthy resource ledger"), 7);
    assert_eq!(
        budget.live_bytes().expect("healthy resource ledger"),
        ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1
    );
    drop(reservation);
    assert_eq!(budget.live_bytes().expect("healthy resource ledger"), 0);
    assert_eq!(budget.consumed().expect("healthy resource ledger"), 7);
    budget
        .charge(ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1 - 7)
        .unwrap();
    assert!(matches!(
        budget.admit(1, 1, 0),
        Err(RnsNativeResourceErrorV1::WorkLimit)
    ));
    assert_eq!(budget.live_bytes().expect("healthy resource ledger"), 0);
    assert_eq!(
        budget.consumed().expect("healthy resource ledger"),
        ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1
    );
    assert_eq!(
        budget.peak_bytes().expect("healthy resource ledger"),
        ZK_AMS_MKHE_RNS_NATIVE_WORKSPACE_MAX_BYTES_V1
    );
}

#[test]
fn poisoned_ledger_rejects_new_admission_but_releases_existing_bytes() {
    let mut budget = RnsNativeProofResourceBudgetV1::default();
    let mut reservation = budget.admit(7, 20, 11).expect("healthy admission");
    assert_eq!(budget.consumed(), Ok(7));
    assert_eq!(budget.live_bytes(), Ok(31));
    let shared = Arc::clone(&budget.usage);
    let unwind = std::panic::catch_unwind(|| {
        let _guard = shared.lock().expect("healthy lock");
        panic!("deliberately poison the accounting lock");
    });
    assert!(unwind.is_err());
    assert!(shared.is_poisoned());
    for observed in [budget.consumed(), budget.live_bytes(), budget.peak_bytes()] {
        assert_eq!(observed, Err(RnsNativeResourceErrorV1::LedgerPoisoned));
    }
    assert_eq!(
        budget.charge(1),
        Err(RnsNativeResourceErrorV1::LedgerPoisoned)
    );
    assert!(matches!(
        budget.admit(1, 1, 1),
        Err(RnsNativeResourceErrorV1::LedgerPoisoned)
    ));
    let attempted = RnsNativeQpcsTreeV1::build(
        parameter(),
        RnsNativeOracleV1::Fri { layer: 17 },
        &mut budget,
        |_, _| panic!("poison rejection must precede source access"),
    );
    assert!(matches!(
        attempted,
        Err(RnsNativeTreeErrorV1::Resource(
            RnsNativeResourceErrorV1::LedgerPoisoned
        ))
    ));
    // Test-only inspection of the poisoned state proves rejected admissions
    // did not mutate any counter; production observations remain fallible.
    {
        let observed = shared.lock().unwrap_err().into_inner();
        assert_eq!(
            (
                observed.consumed_work,
                observed.live_bytes,
                observed.peak_bytes
            ),
            (7, 31, 31)
        );
    }
    reservation.release_scratch();
    reservation.release_scratch();
    {
        let observed = shared.lock().unwrap_err().into_inner();
        assert_eq!(
            (
                observed.consumed_work,
                observed.live_bytes,
                observed.peak_bytes
            ),
            (7, 20, 31)
        );
    }
    drop(reservation);
    {
        let observed = shared.lock().unwrap_err().into_inner();
        assert_eq!(
            (
                observed.consumed_work,
                observed.live_bytes,
                observed.peak_bytes
            ),
            (7, 0, 31)
        );
    }
    assert!(shared.is_poisoned());
    assert_eq!(
        budget.charge(0),
        Err(RnsNativeResourceErrorV1::LedgerPoisoned)
    );
}

#[test]
fn resource_child_refusal_preserves_original_identity_counters_and_exact_capacity() {
    let mut original = RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1(32);
    let unrelated = RnsNativeProofResourceBudgetV1::default();
    original.charge(19).unwrap();
    let mut parent = original.reserve_workspace_v1(10, 10).unwrap();
    assert!(parent.belongs_to_v1(&original));
    assert!(!parent.belongs_to_v1(&unrelated));
    let child = parent.reserve_child_workspace_v1(12, 0).unwrap();
    assert!(child.belongs_to_v1(&original));
    assert!(!child.belongs_to_v1(&unrelated));
    assert_eq!(original.live_bytes(), Ok(32));
    assert!(matches!(
        parent.reserve_child_workspace_v1(1, 0),
        Err(RnsNativeResourceErrorV1::WorkspaceLimit)
    ));
    assert!(matches!(
        child.reserve_child_workspace_v1(u64::MAX, 1),
        Err(RnsNativeResourceErrorV1::ArithmeticOverflow)
    ));
    assert_eq!(original.consumed(), Ok(19));
    assert_eq!(original.live_bytes(), Ok(32));
    assert_eq!(original.peak_bytes(), Ok(32));
    assert_eq!(unrelated.live_bytes(), Ok(0));
    parent.release_scratch();
    parent.release_scratch();
    assert_eq!(original.live_bytes(), Ok(22));
    let retry = child.reserve_child_workspace_v1(10, 0).unwrap();
    assert!(retry.belongs_to_v1(&original));
    assert_eq!(original.live_bytes(), Ok(32));
    drop(parent);
    assert_eq!(original.live_bytes(), Ok(22));
    drop(child);
    assert_eq!(original.live_bytes(), Ok(10));
    drop(retry);
    assert_eq!(original.live_bytes(), Ok(0));
    assert_eq!(original.consumed(), Ok(19));
    assert_eq!(original.peak_bytes(), Ok(32));
}

#[test]
fn resource_child_outlives_issuer_and_parent_without_reset_or_double_refund() {
    let mut original = RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1(32);
    original.charge(7).unwrap();
    let parent = original.reserve_workspace_v1(10, 0).unwrap();
    let child = parent.reserve_child_workspace_v1(12, 0).unwrap();
    let observed = Arc::clone(&original.usage);
    drop(original);
    drop(parent);
    assert_eq!(observed.lock().unwrap().live_bytes, 12);
    let grandchild = child.reserve_child_workspace_v1(20, 0).unwrap();
    assert!(Arc::ptr_eq(&child.usage, &grandchild.usage));
    assert!(matches!(
        grandchild.reserve_child_workspace_v1(1, 0),
        Err(RnsNativeResourceErrorV1::WorkspaceLimit)
    ));
    drop(child);
    {
        let state = observed.lock().unwrap();
        assert_eq!(
            (state.consumed_work, state.live_bytes, state.peak_bytes),
            (7, 20, 32)
        );
    }
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _owned = grandchild;
        panic!("drop retained child after original issuer is gone");
    }));
    assert!(unwind.is_err());
    assert!(!observed.is_poisoned());
    let state = observed.lock().unwrap();
    assert_eq!(
        (state.consumed_work, state.live_bytes, state.peak_bytes),
        (7, 0, 32)
    );
}
