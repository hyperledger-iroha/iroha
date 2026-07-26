//! Sanity checks for schema hash helpers.

use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{
        NoritoDeserialize as DeriveNoritoDeserialize, NoritoSerialize as DeriveNoritoSerialize,
    },
};

const STABLE_STRUCT_SCHEMA_NAME: &str = "example.public.v1.stable_struct";
const STABLE_ENUM_SCHEMA_NAME: &str = "example.public.v1.stable_enum";

#[derive(
    Debug, PartialEq, Eq, iroha_schema::IntoSchema, DeriveNoritoSerialize, DeriveNoritoDeserialize,
)]
#[norito(schema_name = "example.public.v1.stable_struct")]
struct StableStruct {
    value: u32,
}

#[derive(
    Debug, PartialEq, Eq, iroha_schema::IntoSchema, DeriveNoritoSerialize, DeriveNoritoDeserialize,
)]
#[norito(schema_name = "example.public.v1.stable_enum")]
enum StableEnum {
    Unit,
    Value(u32),
}

#[derive(
    Debug, PartialEq, Eq, iroha_schema::IntoSchema, DeriveNoritoSerialize, DeriveNoritoDeserialize,
)]
struct DefaultSchemaStruct {
    value: u32,
}

#[test]
fn type_name_and_string_based_schema_hash_agree() {
    let via_type = norito::core::type_name_schema_hash::<String>();
    let name = core::any::type_name::<String>();
    let via_name = norito::core::schema_hash_for_name(name);
    assert_eq!(via_type, via_name);

    // Different types should not collide in this test set
    let h_u32 = norito::core::type_name_schema_hash::<u32>();
    assert_ne!(via_type, h_u32);
}

#[test]
fn schema_hash_deterministic_across_calls() {
    let a1 = norito::core::type_name_schema_hash::<(u8, bool)>();
    let a2 = norito::core::type_name_schema_hash::<(u8, bool)>();
    assert_eq!(a1, a2);
}

#[test]
fn schema_name_overrides_struct_encode_decode_and_header_schema() {
    let expected = norito::core::schema_hash_for_name(STABLE_STRUCT_SCHEMA_NAME);
    assert_eq!(<StableStruct as NoritoSerialize>::schema_hash(), expected);
    assert_eq!(
        <StableStruct as NoritoDeserialize<'static>>::schema_hash(),
        expected
    );

    let value = StableStruct { value: 7 };
    let bytes = norito::to_bytes(&value).expect("encode stable-schema struct");
    assert_eq!(&bytes[6..22], expected.as_slice());
    let decoded: StableStruct =
        norito::decode_from_bytes(&bytes).expect("decode stable-schema struct");
    assert_eq!(decoded, value);
}

#[test]
fn schema_name_overrides_enum_encode_decode_and_header_schema() {
    let expected = norito::core::schema_hash_for_name(STABLE_ENUM_SCHEMA_NAME);
    assert_eq!(<StableEnum as NoritoSerialize>::schema_hash(), expected);
    assert_eq!(
        <StableEnum as NoritoDeserialize<'static>>::schema_hash(),
        expected
    );

    for value in [StableEnum::Unit, StableEnum::Value(9)] {
        let bytes = norito::to_bytes(&value).expect("encode stable-schema enum");
        assert_eq!(&bytes[6..22], expected.as_slice());
        let decoded: StableEnum =
            norito::decode_from_bytes(&bytes).expect("decode stable-schema enum");
        assert_eq!(decoded, value);
    }
}

#[cfg(not(feature = "schema-structural"))]
#[test]
fn derive_without_schema_name_keeps_type_name_schema() {
    let expected = norito::core::type_name_schema_hash::<DefaultSchemaStruct>();
    assert_eq!(
        <DefaultSchemaStruct as NoritoSerialize>::schema_hash(),
        expected
    );
    assert_eq!(
        <DefaultSchemaStruct as NoritoDeserialize<'static>>::schema_hash(),
        expected
    );
    let bytes =
        norito::to_bytes(&DefaultSchemaStruct { value: 11 }).expect("encode default-schema struct");
    assert_eq!(&bytes[6..22], expected.as_slice());
}

#[cfg(feature = "schema-structural")]
#[test]
fn structural_schema_hash_matches_reference() {
    let structural = norito::json!({
        "Sample": {"Struct": [
            {"name": "id", "type": "u64"},
            {"name": "name", "type": "String"},
            {"name": "flag", "type": "bool"},
        ]},
        "String": "String",
        "bool": "bool",
        "u64": {"Int": "FixedWidth"},
    });

    let expected = [
        0x3A, 0xE1, 0x59, 0x17, 0x41, 0xF6, 0x66, 0x46, 0x2F, 0xB7, 0x66, 0x57, 0x20, 0xDD, 0xDE,
        0x6C,
    ];

    let value_hash = norito::core::schema_hash_structural_value(&structural);
    assert_eq!(value_hash, expected);

    let json = norito::json::to_json(&structural).expect("serialize structural value");
    let from_str =
        norito::core::schema_hash_structural_from_json_str(&json).expect("hash from str");
    assert_eq!(from_str, expected);

    let from_bytes = norito::core::schema_hash_structural_from_json_bytes(json.as_bytes())
        .expect("hash from bytes");
    assert_eq!(from_bytes, expected);
}
