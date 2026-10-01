//! Preserve original local sample refusal through the snapshot error adapter.
use super::*;

#[test]
fn restore_lane_sample_refusal_keeps_original_typed_local_error() {
    use iroha_data_model::sumeragi_lanes::{LaneSamplesAdmissionError, LaneStateAdmissionError};
    let pool = iroha_allocation::AllocationBudget::new(64);
    let original = pool.try_reserve_bytes(64).unwrap();
    let refusal = pool.try_reserve_bytes(1).unwrap_err();
    assert!(matches!(
        TryReadError::from(
            crate::state::deserialize::StateRestoreError::NativeLaneCustody(
                LaneStateAdmissionError::Samples(LaneSamplesAdmissionError::Admission(refusal))
            )
        ),
        TryReadError::StateNativeLaneCustody(LaneStateAdmissionError::Samples(
            LaneSamplesAdmissionError::Admission(
                iroha_allocation::AllocationRefusal::Capacity { .. }
            )
        ))
    ));
    assert_eq!(pool.reserved_bytes(), 64);
    drop(original);
    assert_eq!(pool.reserved_bytes(), 0);
}
