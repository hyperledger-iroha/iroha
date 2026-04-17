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
