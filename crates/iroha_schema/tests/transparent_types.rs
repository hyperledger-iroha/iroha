//! Transparent types schema tests.
mod common;
use common::{assert_schema_map, entry, named_entry};
use iroha_schema::prelude::*;
use norito::{Decode, Encode};
/// This type tests transparent type inference
#[derive(IntoSchema, Encode, Decode)]
#[schema(transparent)]
struct TransparentStruct(u32);
/// This type tests explicit transparent type (u32)
#[derive(IntoSchema, Encode, Decode)]
#[schema(transparent = "u32")]
struct TransparentStructExplicitInt {
    a: u32,
    b: i32,
}
/// This type tests explicit transparent type (String)
#[derive(IntoSchema, Encode, Decode)]
#[schema(transparent = "String")]
struct TransparentStructExplicitString {
    a: u32,
    b: i32,
}
/// This type tests transparent type being an enum
#[derive(IntoSchema, Encode, Decode)]
#[schema(transparent = "String")]
enum TransparentEnum {
    Variant1,
    Variant2,
}
#[test]
fn transparent_types() {
    let mut schema = MetaMap::new();
    TransparentStruct::update_schema_map(&mut schema);
    TransparentStructExplicitInt::update_schema_map(&mut schema);
    TransparentStructExplicitString::update_schema_map(&mut schema);
    TransparentEnum::update_schema_map(&mut schema);
    <Box<u32>>::update_schema_map(&mut schema);
    assert_schema_map(
        "transparent.transparent_types",
        &schema,
        &[
            entry::<String>("String"),
            entry::<u32>("u32"),
            named_entry::<TransparentStruct>("TransparentStruct", "u32"),
            named_entry::<TransparentStructExplicitInt>("TransparentStructExplicitInt", "u32"),
            named_entry::<TransparentStructExplicitString>(
                "TransparentStructExplicitString",
                "String",
            ),
            named_entry::<TransparentEnum>("TransparentEnum", "String"),
            named_entry::<Box<u32>>("Box<u32>", "u32"),
        ],
    );
}
