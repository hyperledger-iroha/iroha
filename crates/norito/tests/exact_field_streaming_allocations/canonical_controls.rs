//! Physical original-pool counter construction through canonical admission.

use iroha_allocation::{AllocationBudget, PrepaidSharedError};
use norito::{
    CanonicalDecodeControlError, PreparedCanonicalDecode, canonical_decode_limits,
    decode_canonical_with_owned_controls, encode_canonical,
};

fn demand() -> usize {
    PreparedCanonicalDecode::allocation_layouts()
        .iter()
        .map(std::alloc::Layout::size)
        .sum()
}

fn refusing<R>(size: usize, successful_controls: usize, body: impl FnOnce() -> R) -> R {
    struct Restore(usize, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            super::REFUSE_SIZE.set(self.0);
            super::MATCHES_BEFORE_REFUSAL.set(self.1);
        }
    }
    let _restore = Restore(
        super::REFUSE_SIZE.replace(size),
        super::MATCHES_BEFORE_REFUSAL.replace(successful_controls),
    );
    body()
}

#[test]
fn owned_canonical_constructs_exactly_two_original_controls_and_refunds_them() {
    let frame = encode_canonical(&7_u64).unwrap();
    let pool = AllocationBudget::new(demand());
    let limits = canonical_decode_limits(frame.len());
    // Establish schema/TLS state before measuring this decoding attempt.
    assert_eq!(
        decode_canonical_with_owned_controls::<u64>(&frame, limits, &pool).unwrap(),
        7
    );
    let mut result = None;
    let allocations = super::allocations_during(|| {
        result = Some(decode_canonical_with_owned_controls::<u64>(
            &frame, limits, &pool,
        ));
    });
    assert_eq!(result.unwrap().unwrap(), 7);
    assert_eq!(allocations, 2, "construct both actual counter controls");
    assert_eq!(pool.peak_reserved_bytes(), demand());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn owned_canonical_rejects_bad_header_without_allocating_controls() {
    let mut frame = encode_canonical(&7_u64).unwrap();
    frame[0] = 0;
    let pool = AllocationBudget::new(demand());
    let occupied = pool.try_reserve_bytes(demand()).unwrap();
    let mut result = None;
    let allocations = super::allocations_during(|| {
        result = Some(decode_canonical_with_owned_controls::<u64>(
            &frame,
            canonical_decode_limits(frame.len()),
            &pool,
        ));
    });
    let error = result.unwrap().unwrap_err();
    assert!(matches!(
        error,
        CanonicalDecodeControlError::Decode(error)
            if matches!(error.error(), norito::Error::InvalidMagic)
    ));
    assert_eq!(allocations, 0);
    assert_eq!(pool.reserved_bytes(), demand());
    drop(occupied);
    assert_eq!(pool.reserved_bytes(), 0);
}

fn control_allocation_failure(successful_controls: usize) {
    let frame = encode_canonical(&7_u64).unwrap();
    let pool = AllocationBudget::new(demand() + 17);
    let sibling = pool.try_reserve_bytes(17).unwrap();
    let layout = PreparedCanonicalDecode::allocation_layouts()[0];
    let limits = canonical_decode_limits(frame.len());
    let mut result = None;
    let allocations = super::allocations_during(|| {
        result = Some(refusing(layout.size(), successful_controls, || {
            decode_canonical_with_owned_controls::<u64>(&frame, limits, &pool)
        }));
    });
    assert!(matches!(
        result.unwrap().unwrap_err(),
        CanonicalDecodeControlError::Control(PrepaidSharedError::Allocator { requested_bytes })
            if requested_bytes == layout.size()
    ));
    assert_eq!(allocations, successful_controls);
    assert_eq!(pool.peak_reserved_bytes(), demand() + 17);
    assert_eq!(
        pool.reserved_bytes(),
        17,
        "unrelated credits must stay occupied"
    );
    assert_eq!(
        decode_canonical_with_owned_controls::<u64>(&frame, limits, &pool).unwrap(),
        7
    );
    assert_eq!(pool.reserved_bytes(), 17);
    drop(sibling);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn owned_canonical_first_control_allocator_failure_retains_original_cause() {
    control_allocation_failure(0);
}

#[test]
fn owned_canonical_second_control_allocator_failure_retires_first_control() {
    control_allocation_failure(1);
}
