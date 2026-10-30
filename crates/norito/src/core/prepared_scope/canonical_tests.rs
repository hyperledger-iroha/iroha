//! Reusable ordinary canonical decoding with original controls and attempt provenance.

use super::*;
use crate::{CanonicalDecodeControlError, schema::SamplePayload};

fn demand() -> usize {
    PreparedDecodeWorkspace::allocation_layouts()
        .iter()
        .map(Layout::size)
        .sum()
}

fn prepared(pool: &AllocationBudget) -> PreparedDecodeWorkspace {
    let mut reservation = pool
        .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
        .unwrap();
    PreparedDecodeWorkspace::from_reservation(pool, &mut reservation).unwrap()
}

fn decode_error<T>(result: Result<T, CanonicalDecodeControlError>) -> DecodeAttemptError {
    match result {
        Err(CanonicalDecodeControlError::Decode(error)) => error,
        Err(error) => panic!("expected original canonical failure, got {error}"),
        Ok(_) => panic!("expected original canonical failure"),
    }
}

fn allocation_limit(bytes: usize) -> DecodeLimits {
    DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
}

#[test]
fn reusable_canonical_controls_preserve_golden_frames_types_and_original_backing() {
    let pool = AllocationBudget::new(demand());
    let foreign = AllocationBudget::new(demand());
    let mut workspace = prepared(&pool);
    let derived = workspace.derived.clone();
    let explicit = workspace.explicit.clone();
    pool.set_limit_bytes(0);
    let golden = include_bytes!("../../../tests/fixtures/sample_payload_frame.norito");
    let expected = crate::decode_canonical::<SamplePayload>(golden).unwrap();
    for _ in 0..3 {
        let actual = workspace
            .decode_canonical::<SamplePayload>(golden, crate::canonical_decode_limits(golden.len()))
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(crate::encode_canonical(&actual).unwrap(), golden.as_slice());
        let scalar = crate::encode_canonical(&17_u64).unwrap();
        assert_eq!(
            workspace
                .decode_canonical::<u64>(&scalar, allocation_limit(0))
                .unwrap(),
            17
        );
        let larger = vec![vec![1_u64, 2, 3], vec![], vec![7; 17]];
        let bytes = crate::encode_canonical(&larger).unwrap();
        assert_eq!(
            workspace
                .decode_canonical::<Vec<Vec<u64>>>(
                    &bytes,
                    crate::canonical_decode_limits(bytes.len()),
                )
                .unwrap(),
            larger
        );
        assert!(ChargedShared::ptr_eq(&derived, &workspace.derived));
        assert!(ChargedShared::ptr_eq(&explicit, &workspace.explicit));
        assert!(workspace.belongs_to(&pool));
        assert!(!workspace.belongs_to(&foreign));
        assert_eq!(pool.reserved_bytes(), demand());
        assert_eq!(foreign.reserved_bytes(), 0);
    }
    assert_eq!(workspace.attempt, 9);
    drop((workspace, derived, explicit));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn reusable_canonical_header_precedes_attempt_exhaustion_without_changing_counters() {
    let pool = AllocationBudget::new(demand());
    let mut workspace = prepared(&pool);
    let frame = crate::encode_canonical(&7_u64).unwrap();
    let limits = crate::canonical_decode_limits(frame.len());
    workspace.decode_canonical::<u64>(&frame, limits).unwrap();
    workspace.attempt = u64::MAX;
    pool.set_limit_bytes(0);
    let before = workspace.derived.attempt.load(Ordering::Relaxed);
    for bytes in [&[][..], &frame[..Header::SIZE - 1]] {
        let ordinary = crate::decode_canonical_for_admission::<u64>(bytes, limits).unwrap_err();
        let actual = decode_error(workspace.decode_canonical::<u64>(bytes, limits));
        assert_eq!(actual.kind(), ordinary.kind());
        assert_eq!(actual.error().to_string(), ordinary.error().to_string());
        assert_eq!(workspace.derived.attempt.load(Ordering::Relaxed), before);
        assert_eq!(workspace.explicit.attempt.load(Ordering::Relaxed), before);
    }
    assert!(matches!(
        workspace.decode_canonical::<u64>(&frame, limits),
        Err(CanonicalDecodeControlError::Scope(
            PreparedDecodeScopeError::AttemptExhausted
        ))
    ));
    assert_eq!(workspace.derived.attempt.load(Ordering::Relaxed), before);
    assert_eq!(workspace.explicit.attempt.load(Ordering::Relaxed), before);
    assert_eq!(pool.reserved_bytes(), demand());
    drop(workspace);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn reusable_canonical_rejections_match_ordinary_and_restore_ambient_context() {
    let pool = AllocationBudget::new(demand());
    let mut workspace = prepared(&pool);
    let frame = crate::encode_canonical(&vec![1_u64, 2, 3]).unwrap();
    let mut schema = frame.clone();
    schema[6] ^= 1;
    let mut checksum = frame.clone();
    checksum[31] ^= 1;
    let mut trailing = frame.clone();
    trailing.push(0);
    let alternate = {
        let _flags = DecodeFlagsGuard::enter(0);
        crate::core::to_bytes(&vec![1_u64, 2, 3]).unwrap()
    };
    assert_ne!(alternate, frame);
    let ambient_flags = crate::core::default_encode_flags() ^ header_flags::COMPACT_LEN;
    let _flags = DecodeFlagsGuard::enter(ambient_flags);
    let _payload = PayloadCtxGuard::enter(b"original caller bytes");
    let before = payload_ctx();
    for bytes in [
        schema.as_slice(),
        checksum.as_slice(),
        trailing.as_slice(),
        alternate.as_slice(),
        &frame[..frame.len() - 1],
    ] {
        let limits = crate::canonical_decode_limits(bytes.len());
        let ordinary =
            crate::decode_canonical_for_admission::<Vec<u64>>(bytes, limits).unwrap_err();
        let actual = decode_error(workspace.decode_canonical::<Vec<u64>>(bytes, limits));
        assert_eq!(actual.kind(), ordinary.kind());
        assert_eq!(actual.error().to_string(), ordinary.error().to_string());
        assert_eq!(payload_ctx(), before);
        assert_eq!(effective_decode_flags(), Some(ambient_flags));
        assert_eq!(DECODE_NESTING_DEPTH.with(Cell::get), 0);
    }
    assert_eq!(pool.reserved_bytes(), demand());
    drop(workspace);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn reusable_canonical_retains_old_refusal_without_granting_later_attempt_provenance() {
    let pool = AllocationBudget::new(demand());
    let mut workspace = prepared(&pool);
    let frame = crate::encode_canonical(&vec![1_u64, 2]).unwrap();
    let limits = crate::canonical_decode_limits(frame.len());
    let outer_pool = AllocationBudget::new(DecodeBudgetContext::allocation_layout().size());
    let outer = DecodeBudgetContext::try_new_owned(allocation_limit(0), &outer_pool).unwrap();
    let original =
        outer.with(|| decode_error(workspace.decode_canonical::<Vec<u64>>(&frame, limits)));
    assert_eq!(original.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    let retained = original.local_refusal().unwrap();
    let sibling = retained.clone();
    let cause = retained.resource_error();
    let original_attempt = workspace.attempt;
    drop(outer);
    assert_eq!(outer_pool.reserved_bytes(), 0);

    for _ in 0..3 {
        assert_eq!(
            workspace
                .decode_canonical::<Vec<u64>>(&frame, limits)
                .unwrap(),
            vec![1, 2]
        );
        let strict = DecodeLimits::new(1, 4096, 4096, 4096, 100);
        let invalid = decode_error(with_decode_limits_scope(allocation_limit(0), || {
            workspace.decode_canonical::<Vec<u64>>(&frame, strict)
        }));
        assert_eq!(invalid.kind(), DecodeAttemptErrorKind::Invalid);
        assert!(invalid.local_refusal().is_none());
        assert!(matches!(
            invalid.error().decode_resource_error(),
            Some(DecodeResourceError::SequenceLengthExceeded {
                length: 2,
                limit: 1
            })
        ));
        drop(invalid);
        let replayed = classify_decode_attempt(|| {
            workspace
                .decode_canonical::<Vec<u64>>(&frame, limits)
                .map_err(|error| match error {
                    CanonicalDecodeControlError::Decode(error) => error.into_error(),
                    _ => panic!("prepared original controls are reusable"),
                })?;
            Err::<(), _>(retained.clone().into_error())
        })
        .unwrap_err();
        assert_eq!(replayed.kind(), DecodeAttemptErrorKind::Invalid);
        assert!(replayed.local_refusal().is_none());
        assert_eq!(replayed.error().decode_resource_error(), Some(cause));
        assert_eq!(original.kind(), DecodeAttemptErrorKind::EnclosingLimit);
        assert_eq!(original.local_refusal().unwrap(), retained);
        assert_eq!(retained, sibling);
        assert_eq!(retained.resource_error(), cause);
        assert!(workspace.attempt > original_attempt);
        assert_eq!(pool.reserved_bytes(), demand());
    }
    drop(workspace);
    let one_control = PreparedDecodeWorkspace::allocation_layouts()[0].size();
    assert_eq!(pool.reserved_bytes(), one_control);
    drop((original, retained));
    assert_eq!(
        pool.reserved_bytes(),
        one_control,
        "last original refusal still owns its family"
    );
    drop(sibling);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn reusable_canonical_nested_observer_preserves_original_enclosing_refusal() {
    let pool = AllocationBudget::new(demand());
    let mut workspace = prepared(&pool);
    let frame = crate::encode_canonical(&vec![1_u64, 2]).unwrap();
    let limits = crate::canonical_decode_limits(frame.len());
    let retained = std::cell::RefCell::new(None);
    let error = with_decode_limits_scope(allocation_limit(0), || {
        classify_decode_attempt(|| {
            let error = decode_error(workspace.decode_canonical::<Vec<u64>>(&frame, limits));
            *retained.borrow_mut() = Some(error.local_refusal().unwrap());
            Err::<(), _>(error.into_error())
        })
    })
    .unwrap_err();
    assert_eq!(error.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert_eq!(error.local_refusal(), *retained.borrow());
    drop((error, retained, workspace));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[derive(crate::NoritoSerialize)]
struct VerificationBody(u64);

#[derive(Debug, PartialEq, crate::NoritoDeserialize, crate::NoritoSchema)]
#[norito_schema(name = "norito::tests::ReusableCanonicalVerificationCharge")]
struct VerificationCharge(u64);

impl SerializePayload for VerificationCharge {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        reserve_decode_allocation(1)?;
        SerializePayload::serialize(&VerificationBody(self.0), writer)
    }
}

#[test]
fn reusable_canonical_schema_scope_retires_before_streaming_verification() {
    let frame = crate::encode_canonical(&VerificationCharge(29)).unwrap();
    let strict = allocation_limit(0);
    let ordinary =
        crate::decode_canonical_with_limits::<VerificationCharge>(&frame, strict).unwrap();
    let pool = AllocationBudget::new(demand());
    let mut workspace = prepared(&pool);
    assert_eq!(
        workspace
            .decode_canonical::<VerificationCharge>(&frame, strict)
            .unwrap(),
        ordinary
    );
    assert_eq!(
        workspace
            .explicit
            .total_allocated_bytes
            .load(Ordering::Relaxed),
        0
    );
    assert!(
        workspace
            .derived
            .total_allocated_bytes
            .load(Ordering::Relaxed)
            > 0
    );
    drop(workspace);
    assert_eq!(pool.reserved_bytes(), 0);
}
