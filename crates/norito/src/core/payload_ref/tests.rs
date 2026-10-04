//! Exact borrowed payload, length, and error propagation controls.

use super::*;
use crate::core::{DecodeFlagsGuard, DecodeFromSlice, default_encode_flags, serialize_to_buffer};

// Deliberately non-Clone: encoding must observe this original value.
struct Leaf(u32);

impl SerializePayload for Leaf {
    fn serialize(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.serialize(encoder)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(8)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(4)
    }
}

#[test]
fn borrowed_payload_preserves_golden_bytes_lengths_and_original_value() {
    let leaf = Leaf(0x1234_5678);
    let borrowed = PayloadRef(&leaf);
    let erased = PayloadRef(&leaf as &dyn SerializePayload);
    let mut erased_bytes = Vec::new();
    serialize_to_buffer(&erased, &mut erased_bytes).unwrap();
    assert_eq!(erased_bytes, [0x78, 0x56, 0x34, 0x12]);
    assert!(std::ptr::eq(&*borrowed, &leaf));
    assert_eq!(borrowed.encoded_len_hint(), Some(8));
    assert_eq!(borrowed.encoded_len_exact(), Some(4));
    for flags in [0, default_encode_flags()] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let mut bytes = Vec::new();
        serialize_to_buffer(&borrowed, &mut bytes).unwrap();
        assert_eq!(bytes, [0x78, 0x56, 0x34, 0x12]);
        let (decoded, used) = u32::decode_from_slice(&bytes).unwrap();
        assert_eq!((decoded, used), (leaf.0, bytes.len()));
    }
}

#[test]
fn borrowed_unsized_payload_and_json_match_the_owned_field() {
    let text = "quoted \"payload\"";
    let owned = text.to_owned();
    let borrowed = PayloadRef(&text);
    for flags in [0, default_encode_flags()] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let mut bytes = Vec::new();
        let mut expected = Vec::new();
        serialize_to_buffer(&borrowed, &mut bytes).unwrap();
        serialize_to_buffer(&owned, &mut expected).unwrap();
        assert_eq!(bytes, expected);
        assert_eq!(borrowed.encoded_len_hint(), text.encoded_len_hint());
        assert_eq!(borrowed.encoded_len_exact(), text.encoded_len_exact());
        let (decoded, used) = String::decode_from_slice(&bytes).unwrap();
        assert_eq!((decoded.as_str(), used), (text, bytes.len()));
    }
    let mut json = String::new();
    borrowed.json_serialize(&mut json);
    assert_eq!(json, "\"quoted \\\"payload\\\"\"");
    assert_eq!(
        crate::json::to_json_bounded(&borrowed, json.len()).unwrap(),
        json
    );
    assert_eq!(
        crate::json::to_json_bounded(&borrowed, json.len() - 1),
        Err(BoundedJsonError::BodyTooLarge)
    );
}

#[test]
fn borrowed_payload_preserves_serialization_refusals() {
    struct Refused;
    impl SerializePayload for Refused {
        fn serialize(&self, _: &mut Encoder<'_>) -> Result<(), Error> {
            Err(Error::LengthMismatch)
        }
    }
    impl JsonSerialize for Refused {
        fn json_serialize(&self, _: &mut String) {
            panic!("bounded encoding must not call the unbounded writer");
        }
        fn json_serialize_to(&self, _: &mut dyn JsonWriteSink) -> Result<(), BoundedJsonError> {
            Err(BoundedJsonError::Unsupported)
        }
    }
    let borrowed = PayloadRef(&Refused);
    assert_eq!(borrowed.encoded_len_hint(), None);
    assert_eq!(borrowed.encoded_len_exact(), None);
    assert!(matches!(
        serialize_to_buffer(&borrowed, &mut Vec::new()),
        Err(Error::LengthMismatch)
    ));
    assert_eq!(
        crate::json::to_json_bounded(&borrowed, 100),
        Err(BoundedJsonError::Unsupported)
    );
}
