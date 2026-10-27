//! Authored controls only; this detached candidate has not been compiled/run.
use super::*;
fn owned(pool: &AllocationBudget) -> RetainedActionFrameBytesV1 {
    // Deliberately not a semantic Action. Physical producer framing cannot grant validation.
    RetainedActionFrameBytesV1::try_encode_unvalidated_payload(
        &1_u32,
        pool,
        norito::core::EncodeWorkLimits {
            max_frame_bytes: 4096,
            max_byte_visits: 100_000,
            max_operations: 100_000,
            max_depth: 32,
        },
    )
    .unwrap()
}
fn scanner(pool: &AllocationBudget) -> BoundedActionFrameScannerV1<'_> {
    BoundedActionFrameScannerV1::try_new(
        pool,
        ActionScannerLimitsV1 {
            max_frame_bytes: 4096,
            max_total_frame_bytes: 8192,
            max_frames: 2,
            max_work: 100_000,
            max_fields: 100,
            max_sequence_elements: 1024,
            max_depth: 32,
        },
        DecodeLimits::new(1000, 8192, 1000, 8192, 32),
    )
    .unwrap()
}
#[test]
fn owned_scan_failure_preserves_same_bytes_address_charge_and_first_refusal() {
    let pool = AllocationBudget::new(1_048_576);
    let frame = owned(&pool);
    let address = frame.as_bytes().as_ptr();
    let length = frame.backing_bytes();
    let mut scanner = scanner(&pool);
    let failure = scanner.scan_owned(frame).unwrap_err();
    let first = std::ptr::from_ref(failure.failure());
    let frame = failure.into_frame().unwrap();
    assert_eq!(frame.as_bytes().as_ptr(), address);
    let failure = scanner.scan_owned(frame).unwrap_err();
    assert_eq!(std::ptr::from_ref(failure.failure()), first);
    let frame = failure.into_frame().unwrap();
    assert_eq!(frame.as_bytes().as_ptr(), address);
    drop(scanner);
    assert_eq!(pool.reserved_bytes(), length);
    drop(frame);
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn foreign_pool_owner_is_retained_but_never_admitted_to_scan_range() {
    let original = AllocationBudget::new(1_048_576);
    let foreign = AllocationBudget::new(1_048_576);
    let frame = owned(&original);
    let address = frame.as_bytes().as_ptr();
    let length = frame.backing_bytes();
    let mut scanner = scanner(&foreign);
    let failure = scanner.scan_owned(frame).unwrap_err();
    assert!(matches!(
        failure.failure(),
        ActionFrameScanFailureV1::Structure("Action frame belongs to a different original pool")
    ));
    let frame = failure.into_frame().unwrap();
    assert_eq!(frame.as_bytes().as_ptr(), address);
    assert_eq!(original.reserved_bytes(), length);
    drop(scanner);
    assert_eq!(foreign.reserved_bytes(), 0);
    drop(frame);
    assert_eq!(original.reserved_bytes(), 0);
}
