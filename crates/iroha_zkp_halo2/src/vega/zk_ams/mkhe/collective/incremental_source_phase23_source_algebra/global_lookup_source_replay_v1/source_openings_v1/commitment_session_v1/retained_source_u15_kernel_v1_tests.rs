//! Real retained dispatch with isolated inventory fixtures, not source authority.
use super::*;
use crate::generalized_bulletproof::secret_u15_msm_v1::{
    CanonicalU15PublicTableV1, test_controls_v1 as controls,
};
use crate::vega::bulletproof_t256::{
    ZkAmsT256BulletproofSuiteV1 as Suite, zeroizing_t256_scalar_vec_drop_count_v1,
};

fn stored_v1() -> RnsNativeStoredPlaneReplayV1<core::convert::Infallible> {
    RnsNativeSmallSignedCommitmentsV1::test_completed_signed_v1()
        .into_stored_plane_replay_v1()
        .unwrap()
}

#[test]
fn u15_retained_original_session_refuses_and_retries_after_real_competing_table_drop() {
    let mut original = stored_v1();
    let axes = original.test_u15_deny_next_entropy_v1();
    let retained = RnsNativeU15MsmTableV1::retained_bytes_v1() as u64;
    let scratch = CanonicalU15PublicTableV1::<Suite>::construction_scratch_bytes_v1() as u64;
    original.test_u15_workspace_v1(Some(retained + scratch));
    let competitor = original.admit_u15_table_v1().unwrap();
    assert_eq!(original.test_u15_workspace_v1(None), retained);
    let mut retained_owner = RetainedSourceSessionV1 {
        phase: Some(RetainedSourcePhaseV1::StoredPlaneReplay(original)),
    };
    controls::reset_v1();
    assert_eq!(
        retained_owner.begin_q_mask_kernel_v1(),
        Err(RnsNativeU15MsmErrorV1::Capacity)
    );
    assert_eq!(controls::allocations_v1(), 0);
    assert!(matches!(
        retained_owner.phase,
        Some(RetainedSourcePhaseV1::StoredPlaneReplay(_))
    ));
    drop(competitor);
    retained_owner.begin_q_mask_kernel_v1().unwrap();
    let Some(RetainedSourcePhaseV1::QMaskKernel(kernel)) = retained_owner.phase.as_mut() else {
        panic!("same original owner must advance")
    };
    assert_eq!(kernel.source.test_u15_workspace_v1(None), retained);
    assert_eq!(kernel.source.test_u15_deny_next_entropy_v1(), axes);
    // There is no second transition, extraction, old replay or renewed entropy path.
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    assert_eq!(
        retained_owner.begin_q_mask_kernel_v1(),
        Err(RnsNativeU15MsmErrorV1::Source)
    );
    assert!(retained_owner.phase.is_none());
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
}

#[test]
fn u15_retained_table_allocation_failure_consumes_original_source_without_retry() {
    let mut original = stored_v1();
    original.test_u15_deny_next_entropy_v1();
    let mut owner = RetainedSourceSessionV1 {
        phase: Some(RetainedSourcePhaseV1::StoredPlaneReplay(original)),
    };
    controls::reset_v1();
    controls::fail_table_v1();
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    assert_eq!(
        owner.begin_q_mask_kernel_v1(),
        Err(RnsNativeU15MsmErrorV1::Allocation)
    );
    assert!(owner.phase.is_none());
    assert_eq!(controls::allocations_v1(), 1);
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
    assert_eq!(
        owner.begin_q_mask_kernel_v1(),
        Err(RnsNativeU15MsmErrorV1::Source)
    );
    assert_eq!(controls::allocations_v1(), 1);
}

#[test]
fn u15_retained_kernel_rejects_earlier_phase_and_table_unwind_consumes_session() {
    let mut earlier = RetainedSourceSessionV1 {
        phase: Some(RetainedSourcePhaseV1::SmallSigned(
            RnsNativeSmallSignedCommitmentsV1::test_completed_signed_v1(),
        )),
    };
    assert_eq!(
        earlier.begin_q_mask_kernel_v1(),
        Err(RnsNativeU15MsmErrorV1::Source)
    );
    assert!(earlier.phase.is_none());
    let mut original = stored_v1();
    original.test_u15_deny_next_entropy_v1();
    let mut owner = RetainedSourceSessionV1 {
        phase: Some(RetainedSourcePhaseV1::StoredPlaneReplay(original)),
    };
    controls::reset_v1();
    controls::panic_table_v1();
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = owner.begin_q_mask_kernel_v1();
    }));
    assert!(result.is_err());
    assert!(owner.phase.is_none());
    assert_eq!(controls::allocations_v1(), 1);
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
    assert_eq!(
        owner.begin_q_mask_kernel_v1(),
        Err(RnsNativeU15MsmErrorV1::Source)
    );
    assert_eq!(controls::allocations_v1(), 1);
}
