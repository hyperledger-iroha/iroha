//! Regression tests for derived-record decoding of self-delimiting fields.
use norito::{
    codec::{Decode, Encode, decode_adaptive, encode_adaptive, encode_with_header_flags},
    core::{DecodeFlagsGuard, frame_bare_with_header_flags, header_flags},
    decode_from_bytes,
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, PartialEq, Eq, Encode, Decode)]
struct NamedSelfDelimiting {
    domains: BTreeSet<String>,
    alias: Option<String>,
    metadata: BTreeMap<String, String>,
}
#[derive(Debug, PartialEq, Eq, Encode, Decode)]
struct TupleSelfDelimiting(BTreeSet<String>, Option<String>, Vec<String>);
#[derive(Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "norito.test.struct_self_delimiting.SelfDelimitingEnum")]
enum SelfDelimitingEnum {
    Named {
        domains: BTreeSet<String>,
        alias: Option<String>,
        metadata: BTreeMap<String, String>,
    },
    Tuple(BTreeSet<String>, Option<String>, Vec<String>),
}
fn assert_roundtrips_in_every_layout<T>(value: &T)
where
    T: core::fmt::Debug
        + PartialEq
        + Eq
        + norito::NoritoSerialize
        + Decode
        + for<'de> norito::NoritoDeserialize<'de>,
{
    for requested in [0, header_flags::COMPACT_LEN] {
        let _guard = DecodeFlagsGuard::enter(requested);
        let (payload, flags) = encode_with_header_flags(value);
        assert_eq!(flags, requested, "advertised layout changed");
        let bytes = frame_bare_with_header_flags::<T>(&payload, flags).expect("frame payload");
        let decoded: T = decode_from_bytes(&bytes).expect("decode payload");
        assert_eq!(&decoded, value, "roundtrip changed flags {flags:#04x}");
    }
}
#[test]
fn named_struct_roundtrips_non_empty_self_delimiting_fields() {
    let value = NamedSelfDelimiting {
        domains: BTreeSet::from([String::from("wonderland")]),
        alias: Some(String::from("alice")),
        metadata: BTreeMap::from([(String::from("title"), String::from("queen"))]),
    };
    let bytes = encode_adaptive(&value);
    let decoded: NamedSelfDelimiting =
        decode_adaptive(&bytes).expect("decode named self-delimiting fields");
    assert_eq!(decoded, value);
}
#[test]
fn enum_named_variant_roundtrips_non_empty_self_delimiting_fields() {
    assert_roundtrips_in_every_layout(&SelfDelimitingEnum::Named {
        domains: BTreeSet::from([String::from("wonderland")]),
        alias: Some(String::from("alice")),
        metadata: BTreeMap::from([(String::from("title"), String::from("queen"))]),
    });
}
#[test]
fn enum_tuple_variant_roundtrips_non_empty_self_delimiting_fields() {
    assert_roundtrips_in_every_layout(&SelfDelimitingEnum::Tuple(
        BTreeSet::from([String::from("wonderland")]),
        Some(String::from("alice")),
        vec![String::from("alpha"), String::from("beta")],
    ));
}
#[test]
fn tuple_struct_roundtrips_non_empty_self_delimiting_fields() {
    let value = TupleSelfDelimiting(
        BTreeSet::from([String::from("wonderland")]),
        Some(String::from("alice")),
        vec![String::from("alpha"), String::from("beta")],
    );
    let bytes = encode_adaptive(&value);
    let decoded: TupleSelfDelimiting =
        decode_adaptive(&bytes).expect("decode tuple self-delimiting fields");
    assert_eq!(decoded, value);
}
