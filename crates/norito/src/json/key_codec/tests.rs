//! Encoding and rejection controls for the sole persisted-map key contract.

use super::*;
use core::fmt::Debug;

fn roundtrip<T: JsonKeyCodec + Debug + PartialEq>(key: T) -> String {
    let mut encoded = String::new();
    key.encode_json_key(&mut encoded);
    let raw: String = json::from_str(&encoded).expect("quoted JSON key");
    assert_eq!(T::decode_json_key(&raw).expect("typed key"), key);
    encoded
}

#[test]
fn primitive_key_spellings_and_roundtrips() {
    assert_eq!(roundtrip("quote\"\\\n".to_owned()), r#""quote\"\\\n""#);
    assert_eq!(roundtrip(u64::MAX), "\"18446744073709551615\"");
    assert_eq!(roundtrip([0xab, 0x00, 0xff]), "\"AB00FF\"");
    assert_eq!(
        <[u8; 3]>::decode_json_key("ab00ff").unwrap(),
        [0xab, 0, 0xff]
    );
}

#[test]
fn tuple_key_spellings_and_roundtrips() {
    let encoded = roundtrip(("left".to_owned(), "right".to_owned()));
    assert_eq!(
        json::from_str::<String>(&encoded).unwrap(),
        "left\u{1f}right"
    );
    roundtrip(("one".to_owned(), "two".to_owned(), "three".to_owned()));
    roundtrip(("circuit".to_owned(), u32::MAX));
    roundtrip(("service".to_owned(), "version".to_owned(), u16::MAX));
    let encoded = roundtrip(("a\u{1f}\"b\\c".to_owned(), u64::MAX, 0));
    assert_eq!(
        json::from_str::<String>(&encoded).unwrap(),
        "\"a\\u001f\\\"b\\\\c\"\u{1f}18446744073709551615\u{1f}0"
    );
}

#[test]
fn malformed_scalar_keys_are_rejected() {
    for invalid in ["", "-1", "18446744073709551616", "x"] {
        assert!(u64::decode_json_key(invalid).is_err(), "{invalid:?}");
    }
    for invalid in ["", "ABC", "00000", "0000000", "GG0000"] {
        assert!(<[u8; 3]>::decode_json_key(invalid).is_err(), "{invalid:?}");
    }
}

#[test]
fn malformed_tuple_keys_are_rejected() {
    for invalid in [
        "",
        "a",
        "a\u{1f}1\u{1f}2",
        "\"a\"\u{1f}18446744073709551616\u{1f}0",
        "\"a\"\u{1f}1\u{1f}0\u{1f}3",
    ] {
        assert!(
            <(String, u64, u64)>::decode_json_key(invalid).is_err(),
            "{invalid:?}"
        );
    }
    assert!(<(String, String)>::decode_json_key("one").is_err());
    assert!(<(String, String, String)>::decode_json_key("one\u{1f}two").is_err());
    assert!(<(String, u32)>::decode_json_key("one\u{1f}4294967296").is_err());
    assert!(<(String, String, u16)>::decode_json_key("one\u{1f}two\u{1f}65536").is_err());
}
