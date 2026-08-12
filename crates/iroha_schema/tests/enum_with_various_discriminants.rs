//! Enum discriminant schema tests.

mod common;

use common::{assert_schema, entry};
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
    assert_schema::<Foo>("enum_various.discriminant", &[entry::<Foo>("Foo")]);
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
