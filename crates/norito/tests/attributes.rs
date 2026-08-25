#![allow(clippy::manual_div_ceil)]
use iroha_schema::IntoSchema;
use norito::core::*;
#[derive(IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct Rename {
    #[norito(rename = "z")]
    x: u32,
}
#[test]
fn rename_roundtrip() {
    let r = Rename { x: 42 };
    let bytes = to_bytes(&r).expect("serialize");
    let archived = from_bytes::<Rename>(&bytes).expect("deserialize");
    let decoded = <Rename as NoritoDeserialize>::deserialize(archived);
    assert_eq!(decoded.x, 42);
}
#[derive(IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct SkipDefault {
    a: u32,
    #[norito(skip)]
    b: u32,
    #[norito(default)]
    c: u32,
}

const fn custom_default() -> u16 {
    41
}

#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TupleDefaults(
    u32,
    #[norito(default)] Option<u64>,
    #[norito(default = "custom_default")] u16,
);

#[cfg_attr(feature = "schema-structural", derive(iroha_schema::TypeId))]
#[derive(Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum NamedDecision {
    Accepted {
        source: u32,
        #[norito(default)]
        program_revision: Option<u64>,
        #[norito(default = "custom_default")]
        marker: u16,
    },
}

#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct NamedDecisionAcceptedSchema {
    source: u32,
    program_revision: Option<u64>,
    marker: u16,
}

#[cfg(feature = "schema-structural")]
impl iroha_schema::IntoSchema for NamedDecision {
    fn type_name() -> String {
        "NamedDecision".to_owned()
    }

    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants: vec![iroha_schema::EnumVariant {
                tag: "Accepted".to_owned(),
                discriminant: 0,
                ty: Some(core::any::TypeId::of::<NamedDecisionAcceptedSchema>()),
            }],
        }));
        <NamedDecisionAcceptedSchema as iroha_schema::IntoSchema>::update_schema_map(map);
    }
}

#[cfg_attr(feature = "schema-structural", derive(iroha_schema::TypeId))]
#[derive(NoritoSerialize)]
enum IncompleteNamedDecision {
    Accepted { source: u32 },
}

#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct IncompleteNamedDecisionAcceptedSchema {
    source: u32,
}

#[cfg(feature = "schema-structural")]
impl iroha_schema::IntoSchema for IncompleteNamedDecision {
    fn type_name() -> String {
        "IncompleteNamedDecision".to_owned()
    }

    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants: vec![iroha_schema::EnumVariant {
                tag: "Accepted".to_owned(),
                discriminant: 0,
                ty: Some(core::any::TypeId::of::<IncompleteNamedDecisionAcceptedSchema>()),
            }],
        }));
        <IncompleteNamedDecisionAcceptedSchema as iroha_schema::IntoSchema>::update_schema_map(map);
    }
}

#[cfg_attr(feature = "schema-structural", derive(iroha_schema::TypeId))]
#[derive(Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum TupleDecision {
    Accepted(
        u32,
        #[norito(default)] Option<u64>,
        #[norito(default = "custom_default")] u16,
    ),
}

#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct TupleDecisionAcceptedSchema(u32, Option<u64>, u16);

#[cfg(feature = "schema-structural")]
impl iroha_schema::IntoSchema for TupleDecision {
    fn type_name() -> String {
        "TupleDecision".to_owned()
    }

    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants: vec![iroha_schema::EnumVariant {
                tag: "Accepted".to_owned(),
                discriminant: 0,
                ty: Some(core::any::TypeId::of::<TupleDecisionAcceptedSchema>()),
            }],
        }));
        <TupleDecisionAcceptedSchema as iroha_schema::IntoSchema>::update_schema_map(map);
    }
}

#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(NoritoSerialize)]
enum IncompleteTupleDecision {
    Accepted(u32),
}

#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(NoritoSerialize)]
struct IncompleteSkipDefault {
    a: u32,
}

#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(Debug, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum DecisionMode {
    #[default]
    Automatic,
    Manual,
}

#[cfg_attr(feature = "schema-structural", derive(iroha_schema::TypeId))]
#[derive(Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum DefaultedDecisionMode {
    Accepted {
        #[norito(default)]
        mode: DecisionMode,
    },
}

#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct DefaultedDecisionModeAcceptedSchema {
    mode: DecisionMode,
}

#[cfg(feature = "schema-structural")]
impl iroha_schema::IntoSchema for DefaultedDecisionMode {
    fn type_name() -> String {
        "DefaultedDecisionMode".to_owned()
    }

    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants: vec![iroha_schema::EnumVariant {
                tag: "Accepted".to_owned(),
                discriminant: 0,
                ty: Some(core::any::TypeId::of::<DefaultedDecisionModeAcceptedSchema>()),
            }],
        }));
        <DefaultedDecisionModeAcceptedSchema as iroha_schema::IntoSchema>::update_schema_map(map);
    }
}

fn binary_layouts() -> [u8; 3] {
    let ordinary = default_encode_flags();
    let packed = ordinary | header_flags::PACKED_STRUCT;
    [ordinary, packed, packed | header_flags::FIELD_BITSET]
}

fn encode_bare_with_flags(value: &impl NoritoSerialize, flags: u8) -> Vec<u8> {
    let _flags = DecodeFlagsGuard::enter(flags);
    let mut payload = Vec::new();
    let mut encoder = Encoder::for_buffer(&mut payload);
    value.serialize(&mut encoder).expect("encode bare value");
    payload
}

fn decode_bare_with_flags<T>(payload: &[u8], flags: u8) -> T
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let _flags = DecodeFlagsGuard::enter(flags);
    let (decoded, used) = decode_field_canonical::<T>(payload)
        .unwrap_or_else(|error| panic!("decode bare value with flags {flags:#04x}: {error:?}"));
    assert_eq!(used, payload.len());
    decoded
}

fn assert_roundtrip_in_all_layouts<T>(value: &T)
where
    T: core::fmt::Debug + PartialEq + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    for flags in binary_layouts() {
        let payload = encode_bare_with_flags(value, flags);
        let decoded = decode_bare_with_flags::<T>(&payload, flags);
        assert_eq!(&decoded, value, "roundtrip with flags {flags:#04x}");
    }
}

#[test]
fn skip_and_default() {
    let s = SkipDefault { a: 5, b: 7, c: 9 };
    let bytes = to_bytes(&s).unwrap();
    let archived = from_bytes::<SkipDefault>(&bytes).unwrap();
    let decoded = <SkipDefault as NoritoDeserialize>::deserialize(archived);
    assert_eq!(decoded.a, 5);
    assert_eq!(decoded.b, 0);
    assert_eq!(decoded.c, 9);
}

#[test]
fn default_fields_are_decoded_in_packed_layouts() {
    let value = SkipDefault { a: 5, b: 7, c: 9 };
    let packed = default_encode_flags() | header_flags::PACKED_STRUCT;
    for flags in [packed, packed | header_flags::FIELD_BITSET] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let mut payload = Vec::new();
        let mut encoder = Encoder::for_buffer(&mut payload);
        value.serialize(&mut encoder).expect("encode packed value");

        let (decoded, used) = decode_field_canonical::<SkipDefault>(&payload)
            .expect("decode packed value with an encoded default field");
        assert_eq!(used, payload.len());
        assert_eq!(decoded.a, value.a);
        assert_eq!(decoded.b, 0);
        assert_eq!(decoded.c, value.c);
    }
}

#[test]
fn tuple_struct_default_fields_roundtrip_in_all_layouts() {
    assert_roundtrip_in_all_layouts(&TupleDefaults(5, Some(7), custom_default()));
}

#[test]
fn enum_default_fields_roundtrip_in_all_layouts() {
    assert_roundtrip_in_all_layouts(&NamedDecision::Accepted {
        source: 5,
        program_revision: Some(7),
        marker: custom_default(),
    });
    assert_roundtrip_in_all_layouts(&TupleDecision::Accepted(5, Some(7), custom_default()));
}

#[test]
fn binary_default_attributes_reject_omitted_fields_in_all_layouts() {
    for flags in binary_layouts() {
        let incomplete_struct = encode_bare_with_flags(&IncompleteSkipDefault { a: 5 }, flags);
        let incomplete_named =
            encode_bare_with_flags(&IncompleteNamedDecision::Accepted { source: 5 }, flags);
        let incomplete_tuple = encode_bare_with_flags(&IncompleteTupleDecision::Accepted(5), flags);
        let _flags = DecodeFlagsGuard::enter(flags);
        assert!(
            decode_field_canonical::<SkipDefault>(&incomplete_struct).is_err(),
            "a missing struct field must be rejected with flags {flags:#04x}",
        );
        assert!(
            decode_field_canonical::<NamedDecision>(&incomplete_named).is_err(),
            "a missing named-enum field must be rejected with flags {flags:#04x}",
        );
        assert!(
            decode_field_canonical::<TupleDecision>(&incomplete_tuple).is_err(),
            "a missing tuple-enum field must be rejected with flags {flags:#04x}",
        );
    }
}

#[test]
fn malformed_present_default_field_is_rejected_in_all_layouts() {
    let value = DefaultedDecisionMode::Accepted {
        mode: DecisionMode::Manual,
    };
    for flags in binary_layouts() {
        let mut payload = encode_bare_with_flags(&value, flags);
        let nested_tag = payload
            .len()
            .checked_sub(core::mem::size_of::<u32>())
            .expect("outer and nested enum discriminants");
        assert_eq!(&payload[nested_tag..], &1_u32.to_le_bytes());
        payload[nested_tag..].copy_from_slice(&u32::MAX.to_le_bytes());

        let _flags = DecodeFlagsGuard::enter(flags);
        assert!(
            decode_field_canonical::<DefaultedDecisionMode>(&payload).is_err(),
            "malformed present default field must be rejected with flags {flags:#04x}",
        );
    }
}
