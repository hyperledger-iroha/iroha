//! Transparent types schema tests.
use std::any::TypeId;

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
    use std::collections::BTreeMap;

    use IntMode::*;
    use Metadata::*;

    let expected = [
        (
            TypeId::of::<::std::string::String>(),
            MetaMapEntry {
                type_id: "String".to_owned(),
                type_name: "String".to_owned(),
                metadata: String,
            },
        ),
        (
            TypeId::of::<u32>(),
            MetaMapEntry {
                type_id: "u32".to_owned(),
                type_name: "u32".to_owned(),
                metadata: Int(FixedWidth),
            },
        ),
        (
            TypeId::of::<TransparentStruct>(),
            MetaMapEntry {
                type_id: "TransparentStruct".to_owned(),
                type_name: "u32".to_owned(),
                metadata: Int(FixedWidth),
            },
        ),
        (
            TypeId::of::<TransparentStructExplicitInt>(),
            MetaMapEntry {
                type_id: "TransparentStructExplicitInt".to_owned(),
                type_name: "u32".to_owned(),
                metadata: Int(FixedWidth),
            },
        ),
        (
            TypeId::of::<TransparentStructExplicitString>(),
            MetaMapEntry {
                type_id: "TransparentStructExplicitString".to_owned(),
                type_name: "String".to_owned(),
                metadata: String,
            },
        ),
        (
            TypeId::of::<TransparentEnum>(),
            MetaMapEntry {
                type_id: "TransparentEnum".to_owned(),
                type_name: "String".to_owned(),
                metadata: String,
            },
        ),
        (
            TypeId::of::<Box<u32>>(),
            MetaMapEntry {
                type_id: "Box<u32>".to_owned(),
                type_name: "u32".to_owned(),
                metadata: Int(FixedWidth),
            },
        ),
    ]
    .into_iter()
    .collect::<BTreeMap<_, _>>();

    let mut schema = MetaMap::new();
    TransparentStruct::update_schema_map(&mut schema);
    TransparentStructExplicitInt::update_schema_map(&mut schema);
    TransparentStructExplicitString::update_schema_map(&mut schema);
    TransparentEnum::update_schema_map(&mut schema);
    <Box<u32>>::update_schema_map(&mut schema);

    assert_eq!(schema, expected);
}
