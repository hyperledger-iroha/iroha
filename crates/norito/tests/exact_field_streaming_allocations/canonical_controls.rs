//! Physical original-pool counter construction through canonical admission.

use iroha_allocation::{AllocationBudget, PrepaidSharedError};
use norito::{
    CanonicalDecodeControlError, PreparedCanonicalDecode, canonical_decode_limits,
    core::{PreparedDecodeScopeError, PreparedDecodeWorkspace},
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

#[test]
fn reusable_canonical_controls_allocate_once_and_decode_without_replacement_allocations() {
    let scalar = encode_canonical(&19_u64).unwrap();
    let fixed = encode_canonical(&[3_u8; 8]).unwrap();
    // Warm ordinary schema/panic-hook state before counting actual control custody.
    assert_eq!(norito::decode_canonical::<u64>(&scalar).unwrap(), 19);
    assert_eq!(norito::decode_canonical::<[u8; 8]>(&fixed).unwrap(), [3; 8]);
    let pool = AllocationBudget::new(demand());
    let mut reservation = pool
        .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
        .unwrap();
    let mut workspace = None;
    let allocations = super::allocations_during(|| {
        workspace = Some(PreparedDecodeWorkspace::from_reservation(
            &pool,
            &mut reservation,
        ));
    });
    let mut workspace = workspace.unwrap().unwrap();
    assert_eq!(
        allocations, 2,
        "both original controls must physically exist"
    );
    assert_eq!(pool.reserved_bytes(), demand());
    pool.set_limit_bytes(0);
    let mut result = None;
    let allocations = super::allocations_during(|| {
        // Refuse every replacement control request while counting all allocations.
        result = Some(refusing(
            PreparedDecodeWorkspace::allocation_layouts()[0].size(),
            0,
            || {
                for _ in 0..16 {
                    if workspace
                        .decode_canonical::<u64>(&scalar, canonical_decode_limits(scalar.len()))?
                        != 19
                    {
                        panic!("same original scalar");
                    }
                    if workspace
                        .decode_canonical::<[u8; 8]>(&fixed, canonical_decode_limits(fixed.len()))?
                        != [3; 8]
                    {
                        panic!("same original fixed payload");
                    }
                }
                Ok::<(), CanonicalDecodeControlError>(())
            },
        ));
    });
    result.unwrap().unwrap();
    assert_eq!(
        allocations, 0,
        "reuse must allocate no replacement controls or scope backing"
    );
    assert_eq!(pool.reserved_bytes(), demand());
    drop((workspace, reservation));
    assert_eq!(pool.reserved_bytes(), 0);
}

fn reusable_control_allocation_failure(successful_controls: usize) {
    let frame = encode_canonical(&31_u64).unwrap();
    let pool = AllocationBudget::new(demand() + 17);
    let sibling = pool.try_reserve_bytes(17).unwrap();
    let layouts = PreparedDecodeWorkspace::allocation_layouts();
    let mut reservation = pool.try_reserve_layouts(layouts).unwrap();
    let mut result = None;
    let allocations = super::allocations_during(|| {
        result = Some(refusing(layouts[0].size(), successful_controls, || {
            PreparedDecodeWorkspace::from_reservation(&pool, &mut reservation)
        }));
    });
    assert!(matches!(
        result.unwrap(),
        Err(PreparedDecodeScopeError::Allocation(PrepaidSharedError::Allocator { requested_bytes }))
            if requested_bytes == layouts[0].size()
    ));
    assert_eq!(allocations, successful_controls);
    assert_eq!(
        reservation.remaining_bytes(),
        demand() - (successful_controls + 1) * layouts[0].size()
    );
    drop(reservation);
    assert_eq!(
        pool.reserved_bytes(),
        17,
        "unrelated original sibling stays admitted"
    );
    let mut reservation = pool.try_reserve_layouts(layouts).unwrap();
    let mut workspace = PreparedDecodeWorkspace::from_reservation(&pool, &mut reservation).unwrap();
    assert_eq!(
        workspace
            .decode_canonical::<u64>(&frame, canonical_decode_limits(frame.len()))
            .unwrap(),
        31
    );
    assert_eq!(pool.reserved_bytes(), demand() + 17);
    drop((workspace, reservation));
    assert_eq!(pool.reserved_bytes(), 17);
    drop(sibling);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn reusable_canonical_first_control_physical_failure_preserves_original_cause_and_sibling() {
    reusable_control_allocation_failure(0);
}

#[test]
fn reusable_canonical_second_control_physical_failure_preserves_original_cause_and_sibling() {
    reusable_control_allocation_failure(1);
}
