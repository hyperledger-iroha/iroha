//! Fixed chain-label storage and exact codec refusal classification.

use super::*;

#[test]
fn inline_chain_label_preserves_exact_grammar_and_boundary() {
    for text in [
        "chain",
        "a.b_c:d-1",
        &"x".repeat(iroha_primitives::chain_id::MAX_CHAIN_ID_BYTES),
    ] {
        assert_eq!(BoundChainId::new(text).unwrap().as_bytes(), text.as_bytes());
    }
    for text in [
        "",
        "a/b",
        "é",
        &"x".repeat(iroha_primitives::chain_id::MAX_CHAIN_ID_BYTES + 1),
    ] {
        assert!(matches!(BoundChainId::new(text), Err(Error::Transaction)));
    }
}

#[test]
fn codec_errors_keep_original_fields_and_separate_local_from_terminal() {
    let allocator: NativeCheckBindingErrorV1<Error> =
        norito::Error::AllocationFailed { bytes: 73 }.into();
    assert!(allocator.is_retryable());
    assert!(matches!(
        allocator,
        NativeCheckBindingErrorV1::Codec {
            original: norito::Error::AllocationFailed { bytes: 73 },
            local: Some(ExecutionDeferral::AllocationUnavailable)
        }
    ));
    let malformed: NativeCheckBindingErrorV1<Error> = norito::Error::LengthMismatch.into();
    assert!(!malformed.is_retryable());
    assert!(matches!(
        malformed,
        NativeCheckBindingErrorV1::Codec {
            original: norito::Error::LengthMismatch,
            local: None
        }
    ));
}

#[test]
fn bounded_frame_destination_refuses_growth_and_retains_funding() {
    use std::io::Write as _;
    let budget = iroha_allocation::AllocationBudget::new(3);
    let mut writer = FrameWriter(ChargedBuffer::new(3, &budget).unwrap());
    writer.write_all(&[1, 2, 3]).unwrap();
    assert!(writer.write_all(&[4]).is_err());
    writer.flush().unwrap();
    assert_eq!(writer.0.as_slice(), &[1, 2, 3]);
    assert_eq!(budget.reserved_bytes(), 3);
    drop(writer);
    assert_eq!(budget.reserved_bytes(), 0);
}

/// A component-only serializer requests real caller scratch admission before writing.
/// This does not impersonate State admission or a production Check serializer.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_core.test.signer_check.AdmittedMeasurement")]
struct AdmittedMeasurement;
impl norito::SerializePayload for AdmittedMeasurement {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        norito::core::reserve_decode_allocation(73)?;
        writer.write_all(&[1])?;
        Ok(())
    }
}

#[test]
fn early_canonical_measurement_retains_exact_active_scope_refusal_and_semantic_bound() {
    let zero = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
    let failure = norito::with_decode_limits_scope(zero, || {
        check_canonical_frame_bound::<_, Error>(&AdmittedMeasurement, usize::MAX)
    })
    .unwrap_err();
    assert!(matches!(
        failure,
        NativeCheckBindingErrorV1::Codec {
            original: norito::Error::TotalAllocationExceeded {
                attempted: 73,
                limit: 0
            },
            local: Some(ExecutionDeferral::ActiveMemoryCapacity)
        }
    ));
    let length = norito::canonical_frame_len(&AdmittedMeasurement).unwrap();
    assert!(check_canonical_frame_bound::<_, Error>(&AdmittedMeasurement, length).is_ok());
    assert!(matches!(
        check_canonical_frame_bound::<_, Error>(&AdmittedMeasurement, length - 1),
        Err(NativeCheckBindingErrorV1::Rejected(Error::Transaction))
    ));
}
