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

struct CheckedKey<'a, T>(&'a T);
impl<T: JsonKeyCodec> json::JsonSerialize for CheckedKey<'_, T> {
    fn json_serialize(&self, output: &mut String) {
        self.0.encode_json_key(output);
    }
    fn json_serialize_to(
        &self,
        output: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.0.encode_json_key_to(output)
    }
}
fn checked_parity<T: JsonKeyCodec + Debug + PartialEq>(key: T) {
    let value = CheckedKey(&key);
    let ordinary = json::to_json(&value).unwrap();
    assert_eq!(
        json::to_json_bounded(&value, ordinary.len()),
        Ok(ordinary.clone())
    );
    for cap in 0..ordinary.len() {
        assert_eq!(
            json::to_json_bounded(&value, cap),
            Err(json::BoundedJsonError::BodyTooLarge)
        );
    }
    let parsed: String = json::from_str(&ordinary).unwrap();
    assert_eq!(T::decode_json_key(&parsed).unwrap(), key);
}
#[test]
fn checked_keys_match_ordinary_uppercase_hex_and_every_tuple_shape() {
    checked_parity(String::new());
    checked_parity("quote\"\\\n\u{1f}😀".to_owned());
    checked_parity(0_u64);
    checked_parity(u64::MAX);
    checked_parity([0_u8; 0]);
    checked_parity([0xab, 0x00, 0xff]);
    checked_parity(("left".to_owned(), "right".to_owned()));
    checked_parity(("one".to_owned(), "two\n".to_owned(), "three".to_owned()));
    checked_parity(("circuit".to_owned(), u32::MAX));
    checked_parity(("service".to_owned(), "version".to_owned(), u16::MAX));
    checked_parity(("a\u{1f}\"b\\c😀".to_owned(), u64::MAX, 0));
    checked_parity(([0xff, 0], 0, u64::MAX));
    checked_parity((("left".to_owned(), "right".to_owned()), 1, 2));
}
struct UnmigratedKey;
impl JsonKeyCodec for UnmigratedKey {
    fn encode_json_key(&self, _: &mut String) {
        panic!("unmigrated checked key may not allocate an ordinary String");
    }
    fn decode_json_key(_: &str) -> Result<Self, json::Error> {
        Err(json::Error::Message("unused decoder".into()))
    }
}
#[test]
fn checked_key_default_and_nested_tuple_refuse_before_ordinary_fallback() {
    assert_eq!(
        json::to_json_bounded(&CheckedKey(&UnmigratedKey), 1024),
        Err(json::BoundedJsonError::Unsupported)
    );
    assert_eq!(
        json::to_json_bounded(&CheckedKey(&(UnmigratedKey, 1, 2)), 1024),
        Err(json::BoundedJsonError::Unsupported)
    );
}
