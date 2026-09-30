//! Composite enum payload metadata retains codec tags, field order and recursive ownership.

use std::any::TypeId;

use iroha_schema::{EnumVariantPayload, IntoSchema, Metadata, TypeId as SchemaTypeId};
use norito::{
    Decode, Encode,
    codec::{DecodeAll as _, Encode as _},
};

#[derive(Debug, PartialEq, Eq, IntoSchema, Encode, Decode)]
enum Report<T> {
    #[codec(index = 17)]
    Pair(T, u32),
    #[norito(rename = "invalid")]
    Invalid {
        value: T,
        reason: u8,
    },
    Children {
        children: Vec<T>,
        generation: u64,
    },
    Empty(),
    EmptyNamed {},
}

#[derive(IntoSchema)]
#[expect(
    dead_code,
    reason = "recursive fixture is used only for schema expansion"
)]
enum Recursive<T> {
    Children {
        children: Vec<Recursive<T>>,
        value: T,
    },
}

#[test]
fn composite_fields_are_closed_ordered_and_owned_by_the_enum() {
    let schema = Report::<u16>::schema();
    let Some(Metadata::Enum(report)) = schema.get::<Report<u16>>() else {
        panic!("report enum metadata");
    };
    assert_eq!(
        report
            .variants
            .iter()
            .map(|v| v.discriminant)
            .collect::<Vec<_>>(),
        [17, 1, 2, 3, 4]
    );
    assert_eq!(report.variants[1].tag, "invalid");
    assert_eq!(
        report.variants[0].ty,
        Some(TypeId::of::<EnumVariantPayload<Report<u16>, 17>>())
    );
    assert_eq!(
        report.variants[1].ty,
        Some(TypeId::of::<EnumVariantPayload<Report<u16>, 1>>())
    );
    let Some(Metadata::Tuple(pair)) = schema.get::<EnumVariantPayload<Report<u16>, 17>>() else {
        panic!("ordered tuple fields");
    };
    assert_eq!(pair.types, [TypeId::of::<u16>(), TypeId::of::<u32>()]);
    let Some(Metadata::Struct(invalid)) = schema.get::<EnumVariantPayload<Report<u16>, 1>>() else {
        panic!("named fields");
    };
    assert_eq!(
        invalid
            .declarations
            .iter()
            .map(|v| (v.name.as_str(), v.ty))
            .collect::<Vec<_>>(),
        [
            ("value", TypeId::of::<u16>()),
            ("reason", TypeId::of::<u8>())
        ]
    );
    let Some(Metadata::Struct(children)) = schema.get::<EnumVariantPayload<Report<u16>, 2>>()
    else {
        panic!("recursive fields");
    };
    assert_eq!(children.declarations[0].ty, TypeId::of::<Vec<u16>>());
    assert!(schema.contains_key::<Vec<u16>>());
    assert_eq!(
        EnumVariantPayload::<Report<u16>, 17>::id(),
        "Report<u16>::Pair"
    );
    assert_eq!(
        EnumVariantPayload::<Report<u32>, 17>::type_name(),
        "Report<u32>::Pair"
    );
    let mut repeated = schema.clone();
    EnumVariantPayload::<Report<u16>, 17>::update_schema_map(&mut repeated);
    Report::<u16>::update_schema_map(&mut repeated);
    assert_eq!(schema, repeated, "recursive insertion is idempotent");
    assert!(
        norito::json::to_value(&schema).is_ok(),
        "all field references are registered"
    );
    let recursive = Recursive::<u16>::schema();
    assert!(recursive.contains_key::<EnumVariantPayload<Recursive<u16>, 0>>());
    assert!(recursive.contains_key::<Vec<Recursive<u16>>>());
    assert!(norito::json::to_value(&recursive).is_ok());
}

#[test]
fn composite_codec_tags_and_roundtrips_match_the_schema() {
    let cases = [
        (Report::Pair(7_u16, 9), 17_u32),
        (
            Report::Invalid {
                value: 7,
                reason: 2,
            },
            1,
        ),
        (
            Report::Children {
                children: vec![3, 4],
                generation: 9,
            },
            2,
        ),
        (Report::Empty(), 3),
        (Report::EmptyNamed {}, 4),
    ];
    for (value, tag) in cases {
        let bytes = value.encode();
        assert_eq!(u32::from_le_bytes(bytes[..4].try_into().unwrap()), tag);
        assert_eq!(Report::decode_all(&mut bytes.as_slice()).unwrap(), value);
    }
}
