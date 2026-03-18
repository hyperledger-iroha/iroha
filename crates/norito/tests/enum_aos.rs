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

#[derive(
    IntoSchema, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq,
)]
enum AoSU8ArrayEnum {
    Unit,
    Bytes([u8; 12]),
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
