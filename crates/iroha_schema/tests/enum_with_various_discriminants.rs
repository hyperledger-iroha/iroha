//! Enum discriminant schema tests.
use iroha_schema::prelude::*;
use norito::{
    Decode, Encode,
    codec::{DecodeAll as _, Encode as _},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, IntoSchema, Encode, Decode)]
enum Foo {
    #[codec(index = 10)]
    A = 10,
    B,
    C = 300,
    #[codec(index = 999)]
    D,
}

#[test]
fn discriminant() {
    use std::collections::BTreeMap;

    let expected_meta = vec![(
        core::any::TypeId::of::<Foo>(),
        MetaMapEntry {
            type_id: "Foo".to_owned(),
            type_name: "Foo".to_owned(),
            metadata: Metadata::Enum(EnumMeta {
                variants: vec![
                    EnumVariant {
                        tag: "A".to_owned(),
                        discriminant: 10,
                        ty: None,
                    },
                    EnumVariant {
                        tag: "B".to_owned(),
                        discriminant: 11,
                        ty: None,
                    },
                    EnumVariant {
                        tag: "C".to_owned(),
                        discriminant: 300,
                        ty: None,
                    },
                    EnumVariant {
                        tag: "D".to_owned(),
                        discriminant: 999,
                        ty: None,
                    },
                ],
            }),
        },
    )]
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    assert_eq!(Foo::schema(), expected_meta);
}

#[test]
fn schema_discriminants_match_encoded_u32_tags() {
    let cases = [(Foo::A, 10_u32), (Foo::B, 11), (Foo::C, 300), (Foo::D, 999)];

    for (value, expected) in cases {
        let encoded = value.encode();
        assert_eq!(
            u32::from_le_bytes(encoded[..4].try_into().expect("enum tag")),
            expected
        );
        assert_eq!(
            Foo::decode_all(&mut encoded.as_slice()).expect("roundtrip"),
            value
        );
    }
}
