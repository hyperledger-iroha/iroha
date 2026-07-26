#![allow(clippy::manual_div_ceil)]
//! Ensure [u8; N] fields in packed-structs roundtrip and avoid per-element overhead.
use norito::{NoritoDeserialize, NoritoSerialize, decode_from_bytes, decode_from_reader, to_bytes};

#[derive(Debug, Clone, PartialEq, iroha_schema::IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct BlobPair {
    left: [u8; 8],
    right: [u8; 16],
}

#[test]
fn array_u8_roundtrip() {
    let mut l = [0u8; 8];
    let mut r = [0u8; 16];
    for (i, b) in l.iter_mut().enumerate() {
        *b = i as u8;
    }
    for (i, b) in r.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(3);
    }
    let bp = BlobPair { left: l, right: r };
    let bytes = to_bytes(&bp).expect("encode");
    let got: BlobPair = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(bp, got);
}

#[test]
fn option_array_u8_roundtrip() {
    let mut payload = [0u8; 12];
    for (idx, slot) in payload.iter_mut().enumerate() {
        *slot = idx as u8 ^ 0xAB;
    }
    let value = Some(payload);
    let bytes = to_bytes(&value).expect("encode option");
    let decoded: Option<[u8; 12]> = decode_from_bytes(&bytes).expect("decode option");
    assert_eq!(decoded, value);

    let none_bytes = to_bytes(&Option::<[u8; 12]>::None).expect("encode none");
    let decoded_none: Option<[u8; 12]> = decode_from_bytes(&none_bytes).expect("decode none");
    assert_eq!(decoded_none, None);

    let decoded_none_reader: Option<[u8; 12]> =
        decode_from_reader(std::io::Cursor::new(&none_bytes)).expect("decode none reader");
    assert_eq!(decoded_none_reader, None);
}

#[test]
fn large_array_roundtrip() {
    let arr = [5u8; 32];
    let bytes = to_bytes(&arr).expect("encode large array");
    let decoded: [u8; 32] = decode_from_bytes(&bytes).expect("decode large array");
    assert_eq!(decoded, arr);
}
