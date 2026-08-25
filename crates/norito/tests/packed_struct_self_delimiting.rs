//! Regression tests for packed-struct decoding of self-delimiting fields.
use norito::{
    codec::{Decode, Encode, decode_adaptive, encode_adaptive, encode_with_header_flags},
    core::{DecodeFlagsGuard, frame_bare_with_header_flags, header_flags},
    decode_from_bytes,
};
use std::collections::{BTreeMap, BTreeSet};
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(Debug, PartialEq, Eq, Encode, Decode)]
struct NamedPackedSelfDelimiting {
    domains: BTreeSet<String>,
    alias: Option<String>,
    metadata: BTreeMap<String, String>,
}
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(Debug, PartialEq, Eq, Encode, Decode)]
struct TuplePackedSelfDelimiting(BTreeSet<String>, Option<String>, Vec<String>);
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::TypeId))]
#[derive(Debug, PartialEq, Eq, Encode, Decode)]
enum PackedSelfDelimitingEnum {
    Named {
        domains: BTreeSet<String>,
        alias: Option<String>,
        metadata: BTreeMap<String, String>,
    },
    Tuple(BTreeSet<String>, Option<String>, Vec<String>),
}
#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct PackedSelfDelimitingEnumNamedSchema {
    domains: BTreeSet<String>,
    alias: Option<String>,
    metadata: BTreeMap<String, String>,
}
#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct PackedSelfDelimitingEnumTupleSchema(BTreeSet<String>, Option<String>, Vec<String>);
#[cfg(feature = "schema-structural")]
impl iroha_schema::IntoSchema for PackedSelfDelimitingEnum {
    fn type_name() -> String {
        "PackedSelfDelimitingEnum".to_owned()
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants: vec![
                iroha_schema::EnumVariant {
                    tag: "Named".to_owned(),
                    discriminant: 0,
                    ty: Some(core::any::TypeId::of::<PackedSelfDelimitingEnumNamedSchema>()),
                },
                iroha_schema::EnumVariant {
                    tag: "Tuple".to_owned(),
                    discriminant: 1,
                    ty: Some(core::any::TypeId::of::<PackedSelfDelimitingEnumTupleSchema>()),
                },
            ],
        }));
        <PackedSelfDelimitingEnumNamedSchema as iroha_schema::IntoSchema>::update_schema_map(map);
        <PackedSelfDelimitingEnumTupleSchema as iroha_schema::IntoSchema>::update_schema_map(map);
    }
}
fn packed_struct_roundtrip<T>(value: &T) -> T
where
    T: core::fmt::Debug + PartialEq + Eq + Encode + Decode,
{
    let requested = header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN;
    let _guard = DecodeFlagsGuard::enter(requested);
    let (payload, flags) = encode_with_header_flags(value);
    assert_ne!(
        flags & header_flags::PACKED_STRUCT,
        0,
        "packed-struct flag missing"
    );
    assert_ne!(
        flags & header_flags::COMPACT_LEN,
        0,
        "compact-len flag missing"
    );
    let bytes = frame_bare_with_header_flags::<T>(&payload, flags).expect("frame packed payload");
    decode_from_bytes(&bytes).expect("decode packed payload")
}
#[test]
fn named_struct_roundtrips_non_empty_self_delimiting_fields() {
    let value = NamedPackedSelfDelimiting {
        domains: BTreeSet::from([String::from("wonderland")]),
        alias: Some(String::from("alice")),
        metadata: BTreeMap::from([(String::from("title"), String::from("queen"))]),
    };
    let bytes = encode_adaptive(&value);
    let decoded: NamedPackedSelfDelimiting =
        decode_adaptive(&bytes).expect("decode named packed self-delimiting fields");
    assert_eq!(decoded, value);
}
#[test]
fn packed_enum_named_variant_roundtrips_non_empty_self_delimiting_fields() {
    let value = PackedSelfDelimitingEnum::Named {
        domains: BTreeSet::from([String::from("wonderland")]),
        alias: Some(String::from("alice")),
        metadata: BTreeMap::from([(String::from("title"), String::from("queen"))]),
    };
    let decoded = packed_struct_roundtrip(&value);
    assert_eq!(decoded, value);
}
#[test]
fn packed_enum_tuple_variant_roundtrips_non_empty_self_delimiting_fields() {
    let value = PackedSelfDelimitingEnum::Tuple(
        BTreeSet::from([String::from("wonderland")]),
        Some(String::from("alice")),
        vec![String::from("alpha"), String::from("beta")],
    );
    let decoded = packed_struct_roundtrip(&value);
    assert_eq!(decoded, value);
}
#[test]
fn tuple_struct_roundtrips_non_empty_self_delimiting_fields() {
    let value = TuplePackedSelfDelimiting(
        BTreeSet::from([String::from("wonderland")]),
        Some(String::from("alice")),
        vec![String::from("alpha"), String::from("beta")],
    );
    let bytes = encode_adaptive(&value);
    let decoded: TuplePackedSelfDelimiting =
        decode_adaptive(&bytes).expect("decode tuple packed self-delimiting fields");
    assert_eq!(decoded, value);
}
