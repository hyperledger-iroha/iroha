//! Exact slice decoding tests for the Norito codec facade.

#[derive(Debug, PartialEq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
#[norito(decode_from_slice)]
struct ExactPayload {
    label: String,
    data: Vec<u8>,
    count: u32,
}

#[test]
fn decode_exact_from_slice_roundtrips() {
    let payload = ExactPayload {
        label: "chain-payload".to_owned(),
        data: vec![1, 2, 3, 5, 8, 13],
        count: 6,
    };
    let bytes = norito::codec::encode_adaptive(&payload);

    let decoded =
        norito::codec::decode_exact_from_slice::<ExactPayload>(&bytes).expect("decode exact");

    assert_eq!(decoded, payload);
}

#[test]
fn decode_exact_from_slice_rejects_trailing_bytes() {
    let payload = ExactPayload {
        label: "chain-payload".to_owned(),
        data: vec![1, 2, 3, 5, 8, 13],
        count: 6,
    };
    let mut bytes = norito::codec::encode_adaptive(&payload);
    bytes.push(0);

    let err = norito::codec::decode_exact_from_slice::<ExactPayload>(&bytes)
        .expect_err("trailing bytes must be rejected");

    assert!(matches!(err, norito::Error::LengthMismatch));
}

#[test]
fn decode_exact_from_slice_rejects_forged_sequence_length_before_allocation() {
    let bytes = u64::MAX.to_le_bytes();

    let err = norito::codec::decode_exact_from_slice::<Vec<u64>>(&bytes)
        .expect_err("a forged sequence count must fail before allocation");

    assert!(matches!(
        err,
        norito::Error::SequenceLengthExceeded { .. }
            | norito::Error::TotalElementsExceeded { .. }
            | norito::Error::TotalAllocationExceeded { .. }
    ));
}

#[test]
fn decode_exact_from_slice_with_limits_enforces_schema_count() {
    let value = vec![1_u64, 2, 3, 4];
    let bytes = norito::codec::encode_adaptive(&value);
    let limits = norito::DecodeLimits::new(3, bytes.len(), 3, bytes.len() * 8, 8);

    let err = norito::codec::decode_exact_from_slice_with_limits::<Vec<u64>>(&bytes, limits)
        .expect_err("schema-specific vector bound must be enforced");

    assert!(matches!(
        err,
        norito::Error::SequenceLengthExceeded {
            length: 4,
            limit: 3
        } | norito::Error::TotalElementsExceeded {
            attempted: 4,
            limit: 3
        }
    ));
}

#[test]
fn explicit_limits_cannot_weaken_payload_derived_defaults() {
    let bytes = u64::MAX.to_le_bytes();
    let permissive =
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX);

    let err = norito::codec::decode_exact_from_slice_with_limits::<Vec<u64>>(&bytes, permissive)
        .expect_err("caller limits must not disable payload-derived allocation protection");

    assert!(matches!(
        err,
        norito::Error::SequenceLengthExceeded { .. }
            | norito::Error::TotalElementsExceeded { .. }
            | norito::Error::TotalAllocationExceeded { .. }
    ));
}

#[test]
fn nested_exact_decode_cannot_weaken_an_ambient_budget() {
    let value = vec![1_u64, 2, 3, 4];
    let bytes = norito::codec::encode_adaptive(&value);
    let ambient = norito::DecodeLimits::new(3, bytes.len(), 3, bytes.len() * 8, 8);
    let inner = norito::DecodeLimits::new(10, bytes.len(), 10, bytes.len() * 16, 16);

    let err = norito::with_decode_limits(ambient, || {
        norito::codec::decode_exact_from_slice_with_limits::<Vec<u64>>(&bytes, inner)
    })
    .expect_err("an inner exact decoder must inherit the stricter caller budget");

    assert!(matches!(
        err,
        norito::Error::SequenceLengthExceeded {
            length: 4,
            limit: 3
        } | norito::Error::TotalElementsExceeded {
            attempted: 4,
            limit: 3
        }
    ));
}

#[test]
fn failed_limited_exact_decode_does_not_leak_budget_state() {
    let value = vec![1_u64, 2, 3, 4];
    let bytes = norito::codec::encode_adaptive(&value);
    let strict = norito::DecodeLimits::new(3, bytes.len(), 3, bytes.len() * 8, 8);
    norito::codec::decode_exact_from_slice_with_limits::<Vec<u64>>(&bytes, strict)
        .expect_err("strict decode must reject four elements");

    assert_eq!(
        norito::codec::decode_exact_from_slice::<Vec<u64>>(&bytes)
            .expect("default budget must be restored"),
        value
    );
}
