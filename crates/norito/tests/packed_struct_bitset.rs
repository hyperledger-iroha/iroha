//! Golden checks for hybrid packed-struct bitset sizing behavior.
use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{self as norito_core, DecodeFlagsGuard, Error, header_flags},
};
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
fn encode_bare_with_flags<T: NoritoSerialize>(value: &T, flags: u8) -> Vec<u8> {
    let _guard = DecodeFlagsGuard::enter(flags);
    let mut payload = Vec::new();
    norito::core::serialize_to_buffer(value, &mut payload).expect("serialize");
    payload
}
#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct SelfDelimExample {
    id: u32,
    maybe: Option<u64>,
    values: Vec<u8>,
    labels: BTreeMap<String, u32>,
}
#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct Nested {
    name: String,
}
#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct NeedsSize {
    id: u32,
    nested: Nested,
}
#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct Tiny(u32);
#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct NamedTiny {
    inner: Tiny,
}
#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct TupleTiny(Tiny);
#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct NamedMixed {
    fixed: u32,
    inner: Tiny,
}
#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct TupleMixed(u32, Tiny);
fn decode_bare_with_flags<T>(payload: &[u8], flags: u8) -> Result<T, Error>
where
    T: for<'de> NoritoDeserialize<'de>,
{
    let _guard = DecodeFlagsGuard::enter(flags);
    norito_core::decode_archived_field::<T>(payload)
}
fn assert_typed_error_without_unwind<T>(payload: &[u8], flags: u8)
where
    T: for<'de> NoritoDeserialize<'de>,
{
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        decode_bare_with_flags::<T>(payload, flags)
    }));
    let result = outcome.expect("malformed packed structs must not unwind");
    match result {
        Err(Error::LengthMismatch) => {}
        Err(error) => panic!("unexpected packed-struct decode error: {error:?}"),
        Ok(_) => panic!("malformed packed structs must return a typed decode error"),
    }
}
#[test]
fn packed_struct_bitset_skips_self_delimiting_fields() {
    let mut labels = BTreeMap::new();
    labels.insert("a".to_string(), 1);
    let value = SelfDelimExample {
        id: 7,
        maybe: Some(9),
        values: vec![1, 2, 3],
        labels,
    };
    let flags =
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET;
    let payload = encode_bare_with_flags(&value, flags);
    let bitset_len = 1usize;
    assert_eq!(
        payload[0], 0,
        "bitset should be empty for fixed/self-delimiting fields"
    );
    let id_bytes = value.id.to_le_bytes();
    assert_eq!(&payload[bitset_len..bitset_len + id_bytes.len()], id_bytes);
}
#[test]
fn packed_struct_bitset_emits_size_for_nested_structs() {
    let value = NeedsSize {
        id: 1,
        nested: Nested {
            name: "hi".to_string(),
        },
    };
    let flags =
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET;
    let payload = encode_bare_with_flags(&value, flags);
    assert_eq!(payload[0], 0b10, "nested field should set bit 1");
    let (nested_len, hdr_len) = {
        let _guard = DecodeFlagsGuard::enter(flags);
        norito_core::read_len_dyn_slice(&payload[1..]).expect("nested size header")
    };
    let nested_payload = encode_bare_with_flags(&value.nested, flags);
    assert_eq!(nested_len, nested_payload.len());
    let data_start = 1 + hdr_len;
    let id_bytes = value.id.to_le_bytes();
    assert_eq!(&payload[data_start..data_start + id_bytes.len()], id_bytes);
    assert_eq!(
        &payload[data_start + id_bytes.len()..data_start + id_bytes.len() + nested_payload.len()],
        nested_payload.as_slice()
    );
}
#[test]
fn packed_struct_bitset_rejects_layout_forgery_for_named_and_unnamed_structs() {
    let flags =
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET;
    let named = NamedMixed {
        fixed: 7,
        inner: Tiny(11),
    };
    let mut named_payload = encode_bare_with_flags(&named, flags);
    assert_eq!(named_payload[0], 0b10);
    assert_eq!(
        decode_bare_with_flags::<NamedMixed>(&named_payload, flags).expect("canonical named value"),
        named
    );
    named_payload[0] |= 0b01;
    assert!(matches!(
        decode_bare_with_flags::<NamedMixed>(&named_payload, flags),
        Err(Error::NonCanonicalEncoding)
    ));
    let unnamed = TupleMixed(13, Tiny(17));
    let mut unnamed_payload = encode_bare_with_flags(&unnamed, flags);
    assert_eq!(unnamed_payload[0], 0b10);
    assert_eq!(
        decode_bare_with_flags::<TupleMixed>(&unnamed_payload, flags)
            .expect("canonical unnamed value"),
        unnamed
    );
    unnamed_payload[0] |= 0b01;
    assert!(matches!(
        decode_bare_with_flags::<TupleMixed>(&unnamed_payload, flags),
        Err(Error::NonCanonicalEncoding)
    ));
}
#[test]
fn packed_struct_size_headers_reject_truncation_without_unwind() {
    let flags =
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET;
    for header_len in 0..=7 {
        let mut payload = vec![0b1];
        payload.extend(std::iter::repeat_n(0x80, header_len));
        assert_typed_error_without_unwind::<NamedTiny>(&payload, flags);
        assert_typed_error_without_unwind::<TupleTiny>(&payload, flags);
    }
}
#[test]
fn compact_zero_size_header_is_not_reinterpreted_as_fixed_u64() {
    let flags =
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET;
    for bytes_after_zero in 0..=7 {
        let mut payload = vec![0b1, 0x00];
        payload.extend(std::iter::repeat_n(0xAA, bytes_after_zero));
        assert_typed_error_without_unwind::<NamedTiny>(&payload, flags);
        assert_typed_error_without_unwind::<TupleTiny>(&payload, flags);
    }
}

#[test]
fn packed_struct_sized_field_rejects_a_nested_length_frame() {
    let flags =
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET;
    let inner = Tiny(0x1122_3344);
    let inner_payload = encode_bare_with_flags(&inner, flags);

    let mut nested_frame = Vec::new();
    let mut forged = vec![0b1];
    {
        let _guard = DecodeFlagsGuard::enter(flags);
        norito_core::write_len_header(&mut nested_frame, inner_payload.len() as u64)
            .expect("frame the retired inner encoding");
        nested_frame.extend_from_slice(&inner_payload);
        norito_core::write_len_header(&mut forged, nested_frame.len() as u64)
            .expect("write the canonical outer field size");
    }
    forged.extend_from_slice(&nested_frame);

    assert!(matches!(
        decode_bare_with_flags::<NamedTiny>(&forged, flags),
        Err(Error::NonCanonicalEncoding)
    ));
}
