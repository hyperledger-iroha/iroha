//! Header-first original-pool admission through the shared canonical kernel.

use super::*;
use crate::{
    core::{DecodeAttemptErrorKind, DecodeFlagsGuard, PayloadCtxGuard, header_flags},
    schema::SamplePayload,
};

fn control_bytes() -> usize {
    PreparedCanonicalDecode::allocation_layouts()
        .iter()
        .map(Layout::size)
        .sum()
}

#[test]
fn owned_canonical_controls_preserve_golden_frame_and_ordinary_rejections() {
    let frame = include_bytes!("../../tests/fixtures/sample_payload_frame.norito");
    let expected = SamplePayload {
        version: 7,
        enabled: true,
        label: "demo".into(),
        items: vec![1, 2, 3],
    };
    let pool = AllocationBudget::new(control_bytes());
    let limits = canonical_decode_limits(frame.len());
    let alternate_flags = crate::core::default_encode_flags() ^ header_flags::COMPACT_LEN;
    let _flags = DecodeFlagsGuard::enter(alternate_flags);
    let _payload = PayloadCtxGuard::enter(b"outer original payload");
    let outer_payload = crate::core::payload_ctx();
    let decoded: SamplePayload =
        decode_canonical_with_owned_controls(frame, limits, &pool).unwrap();
    assert_eq!(decoded, expected);
    assert_eq!(
        decoded,
        crate::decode_canonical::<SamplePayload>(frame).unwrap()
    );
    assert_eq!(crate::encode_canonical(&decoded).unwrap(), frame.as_slice());
    assert_eq!(crate::core::payload_ctx(), outer_payload);
    assert_eq!(crate::core::effective_decode_flags(), Some(alternate_flags));
    assert_eq!(pool.peak_reserved_bytes(), control_bytes());
    assert_eq!(pool.reserved_bytes(), 0);

    let mut bad_schema = frame.to_vec();
    bad_schema[6] ^= 1;
    let mut bad_checksum = frame.to_vec();
    bad_checksum[31] ^= 1;
    let mut trailing = frame.to_vec();
    trailing.push(0);
    for malformed in [
        bad_schema.as_slice(),
        bad_checksum.as_slice(),
        trailing.as_slice(),
        &frame[..frame.len() - 1],
    ] {
        let ordinary = crate::decode_canonical_for_admission::<SamplePayload>(
            malformed,
            canonical_decode_limits(malformed.len()),
        )
        .unwrap_err();
        let owned = decode_canonical_with_owned_controls::<SamplePayload>(
            malformed,
            canonical_decode_limits(malformed.len()),
            &pool,
        )
        .unwrap_err();
        let CanonicalDecodeControlError::Decode(owned) = owned else {
            panic!("body rejection must remain a canonical decode error")
        };
        assert_eq!(owned.kind(), ordinary.kind());
        assert_eq!(owned.error().to_string(), ordinary.error().to_string());
        drop(owned);
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(crate::core::payload_ctx(), outer_payload);
        assert_eq!(crate::core::effective_decode_flags(), Some(alternate_flags));
    }
}

#[test]
fn owned_canonical_header_errors_precede_exhausted_original_pool() {
    let frame = crate::encode_canonical(&7_u64).unwrap();
    let pool = AllocationBudget::new(control_bytes());
    let occupied = pool.try_reserve_bytes(control_bytes()).unwrap();
    let mut invalid = vec![Vec::new(), frame[..Header::SIZE - 1].to_vec()];
    // Magic, major/minor versions, unknown/compressed formats and unsupported layout.
    for (offset, value) in [(0, 0), (4, 255), (5, 255), (22, 255), (22, 1), (39, 128)] {
        let mut bytes = frame.clone();
        bytes[offset] = value;
        invalid.push(bytes);
    }
    for bytes in invalid {
        let ordinary = crate::decode_canonical_for_admission::<u64>(
            &bytes,
            canonical_decode_limits(bytes.len()),
        )
        .unwrap_err();
        let owned = decode_canonical_with_owned_controls::<u64>(
            &bytes,
            canonical_decode_limits(bytes.len()),
            &pool,
        )
        .unwrap_err();
        let CanonicalDecodeControlError::Decode(owned) = owned else {
            panic!("the ordinary header error must precede control admission")
        };
        assert_eq!(owned.kind(), DecodeAttemptErrorKind::Invalid);
        assert_eq!(owned.kind(), ordinary.kind());
        assert_eq!(owned.error().to_string(), ordinary.error().to_string());
        assert_eq!(pool.reserved_bytes(), control_bytes());
    }
    assert!(matches!(
        decode_canonical_with_owned_controls::<u64>(
            &frame,
            canonical_decode_limits(frame.len()),
            &pool,
        ),
        Err(CanonicalDecodeControlError::Admission(
            AllocationRefusal::Capacity { .. }
        ))
    ));
    drop(occupied);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn owned_canonical_admission_keeps_original_refusal_and_retries_only_after_its_release() {
    let frame = crate::encode_canonical(&7_u64).unwrap();
    let layouts = PreparedCanonicalDecode::allocation_layouts();
    let pool = AllocationBudget::new(control_bytes());
    let occupied = pool.try_reserve_bytes(1).unwrap();
    let expected = pool.try_reserve_layouts(layouts).unwrap_err();
    let foreign = AllocationBudget::new(control_bytes());
    let foreign_occupied = foreign.try_reserve_bytes(1).unwrap();
    let foreign_refusal = foreign.try_reserve_layouts(layouts).unwrap_err();
    assert_ne!(expected, foreign_refusal);

    for _ in 0..2 {
        let error = decode_canonical_with_owned_controls::<u64>(
            &frame,
            canonical_decode_limits(frame.len()),
            &pool,
        )
        .unwrap_err();
        let CanonicalDecodeControlError::Admission(original) = error else {
            panic!("expected original pool capacity refusal")
        };
        assert_eq!(original, expected, "retain original release observation");
        assert_ne!(original, foreign_refusal);
        assert_eq!(pool.reserved_bytes(), 1);
        drop(foreign.try_reserve_bytes(1).unwrap());
    }
    drop(foreign_occupied);
    assert!(matches!(
        decode_canonical_with_owned_controls::<u64>(
            &frame,
            canonical_decode_limits(frame.len()),
            &pool,
        ),
        Err(CanonicalDecodeControlError::Admission(_))
    ));
    drop(occupied);
    assert_eq!(
        decode_canonical_with_owned_controls::<u64>(
            &frame,
            canonical_decode_limits(frame.len()),
            &pool,
        )
        .unwrap(),
        7
    );
    assert_eq!(pool.peak_reserved_bytes(), control_bytes());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn owned_canonical_controls_preserve_outer_refusal_and_protocol_limit_classification() {
    let frame = crate::encode_canonical(&vec![1_u64, 2]).unwrap();
    let pool = AllocationBudget::new(control_bytes());
    let outer_pool = AllocationBudget::new(DecodeBudgetContext::allocation_layout().size());
    let physical_limits = DecodeLimits::new(1000, 1000, 1000, 0, 100);
    let physical = DecodeBudgetContext::try_new_owned(physical_limits, &outer_pool).unwrap();
    let error = physical
        .with(|| {
            decode_canonical_with_owned_controls::<Vec<u64>>(
                &frame,
                canonical_decode_limits(frame.len()),
                &pool,
            )
        })
        .unwrap_err();
    let CanonicalDecodeControlError::Decode(original) = error else {
        panic!("outer decode refusal must retain its original decoder cause")
    };
    assert_eq!(original.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    let retained = original.local_refusal().unwrap();
    assert_eq!(original.into_local_refusal().unwrap(), retained);
    drop(physical);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(
        outer_pool.reserved_bytes(),
        DecodeBudgetContext::allocation_layout().size()
    );

    let foreign_pool = AllocationBudget::new(DecodeBudgetContext::allocation_layout().size());
    let foreign = DecodeBudgetContext::try_new_owned(physical_limits, &foreign_pool).unwrap();
    let error = foreign
        .with(|| {
            decode_canonical_with_owned_controls::<Vec<u64>>(
                &frame,
                canonical_decode_limits(frame.len()),
                &pool,
            )
        })
        .unwrap_err();
    let CanonicalDecodeControlError::Decode(error) = error else {
        panic!("foreign decode scope must still classify its own failure")
    };
    let foreign_refusal = error.into_local_refusal().unwrap();
    assert_eq!(retained.resource_error(), foreign_refusal.resource_error());
    assert_ne!(retained, foreign_refusal);
    drop((foreign_refusal, foreign));
    assert_eq!(foreign_pool.reserved_bytes(), 0);
    assert_eq!(
        outer_pool.reserved_bytes(),
        DecodeBudgetContext::allocation_layout().size()
    );
    drop(retained);
    assert_eq!(outer_pool.reserved_bytes(), 0);

    let strict = DecodeLimits::new(1, 1000, 1000, 1000, 100);
    let invalid =
        decode_canonical_with_owned_controls::<Vec<u64>>(&frame, strict, &pool).unwrap_err();
    let CanonicalDecodeControlError::Decode(invalid) = invalid else {
        panic!("schema limit must remain an invalid canonical decode")
    };
    assert_eq!(invalid.kind(), DecodeAttemptErrorKind::Invalid);
    assert!(invalid.local_refusal().is_none());
    let ordinary = crate::decode_canonical_with_limits::<Vec<u64>>(&frame, strict).unwrap_err();
    assert_eq!(invalid.error().to_string(), ordinary.to_string());
    drop(invalid);
    assert_eq!(pool.reserved_bytes(), 0);
}
