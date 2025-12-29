//! Repro tests for bare codec encode/decode roundtrips.
//!
//! These exercise dynamic fields (String/Vec) and mixed fixed+dynamic layouts
//! to ensure `codec::Decode` can correctly decode payloads produced by
//! `codec::Encode` without a Norito header.

use norito::codec::{DecodeAll as _, Encode as _};

#[test]
fn bare_roundtrip_u64_pair() {
    let value = (10u64, 20u64);
    let bytes = value.encode();
    let got: (u64, u64) = <_>::decode_all(&mut &bytes[..]).expect("decode_all");
    assert_eq!(got, value);
}

#[test]
fn bare_roundtrip_string_cases() {
    for s in ["", " ", "abc", "hello world"] {
        let value = s.to_string();
        let bytes = value.encode();
        let got: String = <_>::decode_all(&mut &bytes[..]).expect("decode_all");
        assert_eq!(got, value, "mismatch for input {s:?}");
    }
}

#[test]
fn bare_roundtrip_tuple_string_vec() {
    let value = ("backend/xyz".to_string(), vec![1u8, 2, 3, 9]);
    let bytes = value.encode();
    println!(
        "tuple bytes chunks={:?}",
        bytes
            .chunks(8)
            .map(|chunk| chunk.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
    let got: (String, Vec<u8>) = <_>::decode_all(&mut &bytes[..]).expect("decode_all");
    assert_eq!(got, value);
}

#[derive(Debug, Clone, PartialEq, iroha_schema::IntoSchema, norito::Encode, norito::Decode)]
struct Mixed {
    a: u64,
    s: String,
    b: Vec<u8>,
}

#[test]
fn bare_roundtrip_mixed_struct() {
    let value = Mixed {
        a: 42,
        s: " data ".into(),
        b: vec![0xAA, 0xBB, 0xCC],
    };
    let bytes = value.encode();
    let got: Mixed = <_>::decode_all(&mut &bytes[..]).expect("decode_all");
    assert_eq!(got, value);
}
