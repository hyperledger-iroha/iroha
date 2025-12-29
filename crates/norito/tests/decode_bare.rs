//! Bare payload decode helpers should roundtrip and detect trailing bytes.

use norito::core;

#[test]
fn decode_bare_roundtrip_u32() {
    let value = 0xDEADBEEFu32;
    core::reset_decode_state();
    let payload = norito::codec::encode_adaptive(&value);
    let decoded: u32 = norito::decode_bare_from_bytes(&payload).expect("decode bare u32");
    assert_eq!(decoded, value);
}

#[test]
fn decode_bare_roundtrip_vec() {
    let value = vec![1_u32, 1, 2, 3, 5, 8, 13];
    core::reset_decode_state();
    let payload = norito::codec::encode_adaptive(&value);
    let decoded: Vec<u32> = norito::decode_bare_from_bytes(&payload).expect("decode bare Vec<u32>");
    assert_eq!(decoded, value);
}

#[test]
fn decode_bare_roundtrip_map() {
    use std::collections::BTreeMap;

    let mut value = BTreeMap::new();
    value.insert("alpha".to_string(), 1_u64);
    value.insert("beta".to_string(), 2_u64);
    value.insert("gamma".to_string(), 3_u64);

    core::reset_decode_state();
    let payload = norito::codec::encode_adaptive(&value);
    let decoded: BTreeMap<String, u64> =
        norito::decode_bare_from_bytes(&payload).expect("decode bare BTreeMap<String, u64>");
    assert_eq!(decoded, value);
}

#[test]
fn decode_bare_detects_trailing_bytes() {
    let value = 0xABCDu32;
    core::reset_decode_state();
    let mut payload = norito::codec::encode_adaptive(&value);
    payload.extend_from_slice(&[0xAA, 0xBB]);
    let err = norito::decode_bare_from_bytes::<u32>(&payload)
        .expect_err("trailing bytes must be rejected");
    assert!(matches!(
        err,
        norito::core::Error::LengthMismatch
            | norito::core::Error::DecodePanic { .. }
            | norito::core::Error::Message(_)
    ));
}

#[test]
fn decode_bare_roundtrip_nested_vecs() {
    let value: Vec<Vec<u8>> = vec![vec![1, 2, 3, 4], vec![5, 6], vec![7, 8, 9, 10, 11]];
    core::reset_decode_state();
    let payload = norito::codec::encode_adaptive(&value);
    let decoded: Vec<Vec<u8>> =
        norito::decode_bare_from_bytes(&payload).expect("decode bare Vec<Vec<u8>>");
    assert_eq!(decoded, value);
}
