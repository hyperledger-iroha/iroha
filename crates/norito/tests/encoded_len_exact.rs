//! Tests for encoded_len_exact to ensure exact sizing and buffer preallocation.
use norito::{NoritoDeserialize, NoritoSerialize, to_bytes};
use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};
#[test]
fn primitive_exact_len() {
    norito::core::reset_decode_state();
    let v: u32 = 123;
    let exact = v.encoded_len_exact().expect("exact len");
    assert_eq!(exact, 4);
    let bytes = to_bytes(&v).expect("encode");
    assert_eq!(bytes.len(), norito::core::Header::SIZE + exact);
    norito::core::reset_decode_state();
}
#[test]
fn string_exact_len_matches() {
    norito::core::reset_decode_state();
    let s = String::from("hello");
    let exact = match s.encoded_len_exact() {
        Some(len) => len,
        None => return,
    };
    let expected = norito::core::len_prefix_len(s.len()) + s.len();
    assert_eq!(exact, expected);
    let bytes = to_bytes(&s).expect("encode");
    assert_eq!(bytes.len(), norito::core::Header::SIZE + expected);
    norito::core::reset_decode_state();
}
#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct S {
    a: u32,
    b: String,
}
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(NoritoSerialize)]
struct NamedByteArrays {
    tag: u8,
    digest: [u8; 32],
    suffix: [u8; 7],
}
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(NoritoSerialize)]
struct TupleByteArrays(u8, [u8; 32], [u8; 7]);
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::TypeId))]
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
enum EnumByteArrays {
    Tuple([u8; 32], u16),
    Named { digest: [u8; 32], suffix: [u8; 7] },
}
#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct EnumByteArraysTupleSchema([u8; 32], u16);
#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct EnumByteArraysNamedSchema {
    digest: [u8; 32],
    suffix: [u8; 7],
}
#[cfg(feature = "schema-structural")]
impl iroha_schema::IntoSchema for EnumByteArrays {
    fn type_name() -> String {
        "EnumByteArrays".to_owned()
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants: vec![
                iroha_schema::EnumVariant {
                    tag: "Tuple".to_owned(),
                    discriminant: 0,
                    ty: Some(core::any::TypeId::of::<EnumByteArraysTupleSchema>()),
                },
                iroha_schema::EnumVariant {
                    tag: "Named".to_owned(),
                    discriminant: 1,
                    ty: Some(core::any::TypeId::of::<EnumByteArraysNamedSchema>()),
                },
            ],
        }));
        <EnumByteArraysTupleSchema as iroha_schema::IntoSchema>::update_schema_map(map);
        <EnumByteArraysNamedSchema as iroha_schema::IntoSchema>::update_schema_map(map);
    }
}
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct NestedEnumByteArrays {
    prefix: u8,
    value: EnumByteArrays,
    suffix: u8,
}
fn assert_lengths_match_payload_for_supported_layouts<T: NoritoSerialize>(
    value: &T,
    emits_field_bitset: bool,
) {
    use norito::core::header_flags::{COMPACT_LEN, FIELD_BITSET, PACKED_SEQ, PACKED_STRUCT};
    for flags in [
        0,
        COMPACT_LEN,
        PACKED_SEQ,
        PACKED_SEQ | COMPACT_LEN,
        PACKED_STRUCT,
        PACKED_SEQ | PACKED_STRUCT,
        PACKED_STRUCT | COMPACT_LEN,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN,
        PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
    ] {
        norito::core::reset_decode_state();
        let _guard = norito::core::DecodeFlagsGuard::enter_with_hint(flags, flags);
        let mut payload = Vec::new();
        norito::core::serialize_to_buffer(value, &mut payload)
            .expect("serialize derived byte-array payload");
        assert_eq!(
            value.encoded_len_hint(),
            Some(payload.len()),
            "derived hint differs from named/tuple byte-array bytes for flags 0x{flags:02x}"
        );
        assert_eq!(
            value.encoded_len_exact(),
            Some(payload.len()),
            "derived exact length differs from named/tuple byte-array bytes for flags 0x{flags:02x}"
        );
        let frame = norito::to_bytes(value)
            .expect("frame derived byte-array payload under every supported layout");
        let advertised_flags = frame[norito::core::Header::SIZE - 1];
        norito::core::validate_header_flags(advertised_flags)
            .expect("encoder must advertise a supported layout combination");
        if flags & FIELD_BITSET != 0 && emits_field_bitset {
            let required = FIELD_BITSET | PACKED_STRUCT | COMPACT_LEN;
            assert_eq!(
                advertised_flags & required,
                required,
                "field-bitset frame dropped a required dependency for flags 0x{flags:02x}"
            );
        } else if flags & FIELD_BITSET != 0 {
            assert_eq!(
                advertised_flags & FIELD_BITSET,
                0,
                "frame advertised a field bitset that the payload did not emit for flags 0x{flags:02x}"
            );
        }
    }
    norito::core::reset_decode_state();
}
#[test]
fn derive_named_byte_array_lengths_match_every_supported_layout() {
    assert_lengths_match_payload_for_supported_layouts(
        &NamedByteArrays {
            tag: 9,
            digest: [0xA5; 32],
            suffix: [0x5A; 7],
        },
        true,
    );
}
#[test]
fn derive_tuple_byte_array_lengths_match_every_supported_layout() {
    assert_lengths_match_payload_for_supported_layouts(
        &TupleByteArrays(9, [0xA5; 32], [0x5A; 7]),
        true,
    );
}
#[test]
fn derive_enum_byte_array_lengths_and_nested_roundtrip_match_every_supported_layout() {
    for value in [
        EnumByteArrays::Tuple([0xA5; 32], 17),
        EnumByteArrays::Named {
            digest: [0x5A; 32],
            suffix: [0x3C; 7],
        },
    ] {
        assert_lengths_match_payload_for_supported_layouts(&value, false);
        let nested = NestedEnumByteArrays {
            prefix: 9,
            value,
            suffix: 11,
        };
        let bytes = to_bytes(&nested).expect("encode nested enum byte arrays");
        let decoded: NestedEnumByteArrays =
            norito::decode_from_bytes(&bytes).expect("decode nested enum byte arrays");
        assert_eq!(decoded, nested);
    }
}
#[test]
fn nested_builtin_tuple_lengths_match_every_supported_layout() {
    assert_lengths_match_payload_for_supported_layouts(
        &(7_u32, vec![Some(11_u32), None, Some(13_u32)]),
        false,
    );
}
#[test]
fn derive_struct_exact_len() {
    norito::core::reset_decode_state();
    let s = S {
        a: 7,
        b: "xyz".into(),
    };
    let inner_a = 4usize;
    let inner_b = norito::core::len_prefix_len(3) + 3;
    let expected = norito::core::len_prefix_len(inner_a)
        + inner_a
        + norito::core::len_prefix_len(inner_b)
        + inner_b;
    let exact = match s.encoded_len_exact() {
        Some(len) => len,
        None => return,
    };
    assert_eq!(exact, expected);
    let bytes = to_bytes(&s).expect("encode");
    assert_eq!(bytes.len(), norito::core::Header::SIZE + expected);
    norito::core::reset_decode_state();
}
#[test]
fn tuple_exact_len_matches_encoded_payload() {
    norito::core::reset_decode_state();
    let tuple = (String::from("id"), vec![1_u8, 2, 3]);
    let exact = tuple.encoded_len_exact().expect("tuple exact len");
    let bytes = to_bytes(&tuple).expect("encode");
    assert_eq!(bytes.len(), norito::core::Header::SIZE + exact);
    norito::core::reset_decode_state();
}
#[test]
fn option_exact_len() {
    norito::core::reset_decode_state();
    let some = Some(5u32);
    let some_exact = some
        .encoded_len_exact()
        .expect("Some should have exact len when its value does");
    assert_eq!(
        some_exact,
        1 + norito::core::len_prefix_len(4) + 4,
        "Some encodes as discriminator, length prefix, and payload"
    );
    let none: Option<u32> = None;
    let exact = none
        .encoded_len_exact()
        .expect("None should have exact len");
    assert_eq!(exact, 1, "None encodes as only the discriminator tag");
    norito::core::reset_decode_state();
}
#[test]
fn nonzero_exact_len_matches_primitive_width() {
    norito::core::reset_decode_state();
    assert_eq!(
        NonZeroU16::new(1)
            .expect("nonzero")
            .encoded_len_exact()
            .expect("exact len"),
        2
    );
    assert_eq!(
        NonZeroU32::new(1)
            .expect("nonzero")
            .encoded_len_exact()
            .expect("exact len"),
        4
    );
    assert_eq!(
        NonZeroU64::new(1)
            .expect("nonzero")
            .encoded_len_exact()
            .expect("exact len"),
        8
    );
    norito::core::reset_decode_state();
}
#[test]
fn vec_sequential_exact_len() {
    norito::core::reset_decode_state();
    let v: Vec<u32> = vec![1, 2, 3, 4];
    let exact = match v.encoded_len_exact() {
        Some(len) => len,
        None => return,
    };
    let expected =
        norito::core::seq_len_prefix_len(v.len()) + v.len() * (norito::core::len_prefix_len(4) + 4);
    assert_eq!(exact, expected);
    let bytes = to_bytes(&v).expect("encode");
    assert_eq!(bytes.len(), norito::core::Header::SIZE + expected);
    norito::core::reset_decode_state();
}
