//! Snapshot refusal preserves the original lane signer resource category and owner.
use super::*;

#[test]
fn restore_lane_custody_refusal_keeps_original_typed_local_error() {
    use iroha_data_model::sumeragi_lanes::{CustodySignersAdmissionError, LaneStateAdmissionError};
    let pool = iroha_allocation::AllocationBudget::new(64);
    let owner = pool.try_reserve_bytes(64).unwrap();
    let refusal = pool.try_reserve_bytes(1).unwrap_err();
    let converted = TryReadError::from(
        crate::state::deserialize::StateRestoreError::NativeLaneCustody(
            LaneStateAdmissionError::Signers(CustodySignersAdmissionError::ControlAdmission(
                refusal,
            )),
        ),
    );
    assert!(matches!(
        converted,
        TryReadError::StateNativeLaneCustody(LaneStateAdmissionError::Signers(
            CustodySignersAdmissionError::ControlAdmission(
                iroha_allocation::AllocationRefusal::Capacity { .. }
            )
        ))
    ));
    assert_eq!(pool.reserved_bytes(), 64);
    drop(owner);
    assert_eq!(pool.reserved_bytes(), 0);
}
