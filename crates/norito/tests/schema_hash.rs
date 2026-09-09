//! Sanity checks for schema hash helpers.
use norito::{
    DeserializePayload, NoritoSchema,
    derive::{
        NoritoDeserialize as DeriveNoritoDeserialize, NoritoSerialize as DeriveNoritoSerialize,
    },
};
const STABLE_STRUCT_SCHEMA_NAME: &str = "example.public.v1.stable_struct";
const STABLE_ENUM_SCHEMA_NAME: &str = "example.public.v1.stable_enum";
#[derive(
    Debug,
    PartialEq,
    Eq,
    iroha_schema::IntoSchema,
    NoritoSchema,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
)]
#[norito_schema(name = "example.public.v1.stable_struct")]
struct StableStruct {
    value: u32,
}
#[derive(
    Debug,
    PartialEq,
    Eq,
    iroha_schema::IntoSchema,
    NoritoSchema,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
)]
#[norito_schema(name = "example.public.v1.stable_enum")]
enum StableEnum {
    Unit,
    Value(u32),
}
#[derive(
    Debug,
    PartialEq,
    Eq,
    iroha_schema::IntoSchema,
    NoritoSchema,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
)]
#[norito_schema(name = "example.public.v1.declared_struct")]
struct DeclaredSchemaStruct {
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
fn declared_struct_identity_owns_encoding_decoding_and_header() {
    let expected = norito::core::schema_hash_for_name(STABLE_STRUCT_SCHEMA_NAME);
    assert_eq!(
        norito::schema::identity::frame_hash::<StableStruct>(),
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
fn declared_enum_identity_owns_encoding_decoding_and_header() {
    let expected = norito::core::schema_hash_for_name(STABLE_ENUM_SCHEMA_NAME);
    assert_eq!(
        norito::schema::identity::frame_hash::<StableEnum>(),
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
#[test]
fn declared_identity_is_independent_of_rust_type_name() {
    let expected = norito::core::schema_hash_for_name("example.public.v1.declared_struct");
    assert_ne!(
        expected,
        norito::core::type_name_schema_hash::<DeclaredSchemaStruct>()
    );
    assert_eq!(
        norito::schema::identity::frame_hash::<DeclaredSchemaStruct>(),
        expected
    );
    let bytes = norito::to_bytes(&DeclaredSchemaStruct { value: 11 })
        .expect("encode declared-schema struct");
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

// Every reader must use the declared frame identity, including structural builds.
#[derive(
    Debug,
    PartialEq,
    Eq,
    iroha_schema::IntoSchema,
    NoritoSchema,
    DeriveNoritoSerialize,
    DeriveNoritoDeserialize,
)]
#[norito(decode_from_slice)]
#[norito_schema(name = "example.public.v1.declared_enum")]
enum DeclaredSchemaEnum {
    Unit,
    Value(u8),
}

#[test]
fn derived_enum_schema_agrees_across_all_frame_readers() {
    let expected = norito::core::schema_hash_for_name("example.public.v1.declared_enum");
    #[cfg(feature = "schema-structural")]
    assert_ne!(
        expected,
        norito::core::schema_hash_structural::<DeclaredSchemaEnum>()
    );
    assert_eq!(
        norito::schema::identity::frame_hash::<DeclaredSchemaEnum>(),
        expected
    );
    for value in [DeclaredSchemaEnum::Unit, DeclaredSchemaEnum::Value(9)] {
        let frame = norito::to_bytes(&value).expect("encode enum frame");
        assert_eq!(&frame[6..22], expected.as_slice());
        assert_eq!(
            norito::decode_from_bytes::<DeclaredSchemaEnum>(&frame).expect("high-level decode"),
            value
        );
        assert_eq!(
            norito::deserialize_stream::<_, DeclaredSchemaEnum>(frame.as_slice())
                .expect("stream decode"),
            value
        );
        assert_eq!(
            norito::core::decode_from_bytes::<DeclaredSchemaEnum>(&frame)
                .expect("core slice decode"),
            value
        );
        let view = norito::core::from_bytes_view(&frame).expect("read archive view");
        assert_eq!(
            view.decode::<DeclaredSchemaEnum>().expect("view decode"),
            value
        );
        assert_eq!(
            view.decode_exact::<DeclaredSchemaEnum>()
                .expect("exact view decode"),
            value
        );
        let archived = norito::core::from_bytes::<DeclaredSchemaEnum>(&frame)
            .expect("validate archived enum frame");
        assert_eq!(
            DeclaredSchemaEnum::try_deserialize(archived).expect("archived enum decode"),
            value
        );

        let mut wrong_frame = frame.clone();
        #[cfg(feature = "schema-structural")]
        let wrong_schema = norito::core::schema_hash_structural::<DeclaredSchemaEnum>();
        #[cfg(not(feature = "schema-structural"))]
        let wrong_schema = {
            let mut changed = expected;
            changed[0] ^= 1;
            changed
        };
        assert_ne!(
            wrong_schema, expected,
            "negative control changes the schema"
        );
        wrong_frame[6..22].copy_from_slice(&wrong_schema);
        assert!(matches!(
            norito::decode_from_bytes::<DeclaredSchemaEnum>(&wrong_frame),
            Err(norito::Error::SchemaMismatch)
        ));
        assert!(matches!(
            norito::deserialize_stream::<_, DeclaredSchemaEnum>(wrong_frame.as_slice()),
            Err(norito::Error::SchemaMismatch)
        ));
        assert!(matches!(
            norito::core::decode_from_bytes::<DeclaredSchemaEnum>(&wrong_frame),
            Err(norito::Error::SchemaMismatch)
        ));
        assert!(matches!(
            norito::core::from_bytes::<DeclaredSchemaEnum>(&wrong_frame),
            Err(norito::Error::SchemaMismatch)
        ));
        let wrong_view = norito::core::from_bytes_view(&wrong_frame)
            .expect("a view validates bytes before choosing a typed decoder");
        assert!(matches!(
            wrong_view.decode::<DeclaredSchemaEnum>(),
            Err(norito::Error::SchemaMismatch)
        ));
        assert!(matches!(
            wrong_view.decode_exact::<DeclaredSchemaEnum>(),
            Err(norito::Error::SchemaMismatch)
        ));
    }
}
