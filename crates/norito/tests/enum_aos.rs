//! Targeted AoS enum roundtrip tests for Norito derives.
#![allow(clippy::size_of_ref)]
use iroha_schema::IntoSchema;
use norito::{NoritoDeserialize, from_bytes, to_bytes};
#[derive(
    IntoSchema, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq,
)]
struct TuplePayload {
    value: u64,
    text: String,
}
#[derive(
    IntoSchema, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq,
)]
struct StructPayload {
    name: String,
    data: Vec<u8>,
    tag: [u8; 4],
}
#[derive(
    IntoSchema, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq,
)]
enum AoSEnum {
    Unit,
    Tuple(TuplePayload),
    Struct(StructPayload),
}
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::TypeId))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq)]
#[norito(decode_from_slice)]
enum AoSNamedEnum {
    StructLike {
        label: String,
        data: Vec<u8>,
        code: u16,
    },
    Unit,
}
#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct AoSNamedEnumStructLikeSchema {
    label: String,
    data: Vec<u8>,
    code: u16,
}
#[cfg(feature = "schema-structural")]
impl iroha_schema::IntoSchema for AoSNamedEnum {
    fn type_name() -> String {
        "AoSNamedEnum".to_owned()
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants: vec![
                iroha_schema::EnumVariant {
                    tag: "StructLike".to_owned(),
                    discriminant: 0,
                    ty: Some(core::any::TypeId::of::<AoSNamedEnumStructLikeSchema>()),
                },
                iroha_schema::EnumVariant {
                    tag: "Unit".to_owned(),
                    discriminant: 1,
                    ty: None,
                },
            ],
        }));
        <AoSNamedEnumStructLikeSchema as iroha_schema::IntoSchema>::update_schema_map(map);
    }
}
#[derive(
    IntoSchema, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq,
)]
enum AoSU8ArrayEnum {
    Unit,
    Bytes([u8; 12]),
}
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::TypeId))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq)]
enum NamedArrayEnum {
    Raw { prefix: i32, bytes: [u8; 32] },
}
#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct NamedArrayEnumRawSchema {
    prefix: i32,
    bytes: [u8; 32],
}
#[cfg(feature = "schema-structural")]
impl iroha_schema::IntoSchema for NamedArrayEnum {
    fn type_name() -> String {
        "NamedArrayEnum".to_owned()
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants: vec![iroha_schema::EnumVariant {
                tag: "Raw".to_owned(),
                discriminant: 0,
                ty: Some(core::any::TypeId::of::<NamedArrayEnumRawSchema>()),
            }],
        }));
        <NamedArrayEnumRawSchema as iroha_schema::IntoSchema>::update_schema_map(map);
    }
}
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq)]
struct NestedNamedArray {
    first: String,
    value: NamedArrayEnum,
    last: String,
}
#[test]
fn aos_enum_roundtrip_unit() {
    let v = AoSEnum::Unit;
    let bytes = to_bytes(&v).unwrap();
    let arch = from_bytes::<AoSEnum>(&bytes).unwrap();
    let back = <AoSEnum as NoritoDeserialize>::deserialize(arch);
    assert_eq!(v, back);
}
#[test]
fn aos_enum_roundtrip_tuple() {
    let v = AoSEnum::Tuple(TuplePayload {
        value: 42,
        text: "hello".to_string(),
    });
    let bytes = to_bytes(&v).unwrap();
    let arch = from_bytes::<AoSEnum>(&bytes).unwrap();
    let back = <AoSEnum as NoritoDeserialize>::deserialize(arch);
    assert_eq!(v, back);
}
#[test]
fn aos_enum_roundtrip_struct() {
    let v = AoSEnum::Struct(StructPayload {
        name: "abc".to_string(),
        data: vec![1, 2, 3, 4, 5],
        tag: *b"TAG!",
    });
    let bytes = to_bytes(&v).unwrap();
    let arch = from_bytes::<AoSEnum>(&bytes).unwrap();
    let back = <AoSEnum as NoritoDeserialize>::deserialize(arch);
    assert_eq!(v, back);
}
#[test]
fn aos_enum_roundtrip_named_variant() {
    let v = AoSNamedEnum::StructLike {
        label: "named".to_string(),
        data: vec![1, 2, 3, 4],
        code: 7,
    };
    let bytes = to_bytes(&v).unwrap();
    let view = norito::core::from_bytes_view(&bytes).unwrap();
    let back: AoSNamedEnum = view.decode().expect("decode named enum");
    assert_eq!(v, back);
}
#[test]
fn aos_enum_roundtrip_u8_array_unpacked() {
    let _guard = norito::core::DecodeFlagsGuard::enter(0);
    let v = AoSU8ArrayEnum::Bytes([0xAB; 12]);
    let bytes = to_bytes(&v).unwrap();
    let arch = from_bytes::<AoSU8ArrayEnum>(&bytes).unwrap();
    let back = <AoSU8ArrayEnum as NoritoDeserialize>::deserialize(arch);
    assert_eq!(v, back);
}
#[test]
fn aos_nested_named_variant_with_u8_array_roundtrips() {
    let v = NestedNamedArray {
        first: "before".to_owned(),
        value: NamedArrayEnum::Raw {
            prefix: -1,
            bytes: [0xAB; 32],
        },
        last: "after".to_owned(),
    };
    let bytes = to_bytes(&v).unwrap();
    let payload = norito::core::from_bytes_view(&bytes).unwrap();
    assert_eq!(
        norito::core::NoritoSerialize::encoded_len_exact(&v),
        Some(payload.as_bytes().len()),
        "named enum byte arrays must report the raw-byte wire length"
    );
    let back: NestedNamedArray = norito::decode_from_bytes(&bytes).unwrap();
    assert_eq!(v, back);
}
