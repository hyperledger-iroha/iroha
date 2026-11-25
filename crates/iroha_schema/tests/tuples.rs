//! Native tuple schemas retain ordered recursive fields without changing codecs.

use std::any::TypeId;

use iroha_schema::{IntoSchema, Metadata, TypeId as SchemaTypeId};
use norito::codec::{DecodeAll as _, Encode as _};

#[test]
fn tuple_schema_preserves_field_order_and_recursive_closure() {
    type Key = (u64, (String, Vec<u16>), Option<[u8; 32]>);
    let schema = Key::schema();
    let Some(Metadata::Tuple(fields)) = schema.get::<Key>() else {
        panic!("ordered tuple metadata");
    };
    assert_eq!(
        fields.types,
        [
            TypeId::of::<u64>(),
            TypeId::of::<(String, Vec<u16>)>(),
            TypeId::of::<Option<[u8; 32]>>()
        ]
    );
    assert!(schema.contains_key::<Vec<u16>>());
    assert!(schema.contains_key::<u16>());
    assert!(schema.contains_key::<[u8; 32]>());
    assert!(schema.contains_key::<u8>());
    assert_ne!(<(u64, u32)>::id(), <(u32, u64)>::id());
    assert_eq!(<(u64, u32)>::type_name(), "(u64, u32)");
    let mut repeated = schema.clone();
    Key::update_schema_map(&mut repeated);
    assert_eq!(schema, repeated);
    assert!(norito::json::to_value(&schema).is_ok());

    let key: Key = (11, ("root".into(), vec![3, 5]), Some([7; 32]));
    let frame = key.encode();
    assert_eq!(Key::decode_all(&mut frame.as_slice()).unwrap(), key);
}

#[test]
fn largest_supported_tuple_registers_every_ordered_field() {
    type Key = (
        u8,
        u16,
        u32,
        u64,
        u128,
        i8,
        i16,
        i32,
        i64,
        i128,
        bool,
        String,
    );
    let schema = Key::schema();
    let Some(Metadata::Tuple(fields)) = schema.get::<Key>() else {
        panic!("ordered tuple metadata");
    };
    assert_eq!(fields.types.len(), 12);
    assert_eq!(fields.types[0], TypeId::of::<u8>());
    assert_eq!(fields.types[11], TypeId::of::<String>());
    assert!(norito::json::to_value(&schema).is_ok());
}
