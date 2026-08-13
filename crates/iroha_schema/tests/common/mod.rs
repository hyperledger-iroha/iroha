//! Strict schema-golden assertions shared by integration tests.
#![allow(dead_code)]
use std::any::TypeId;
use iroha_schema::{IntoSchema, MetaMap, MetaMapEntry};
use norito::json::Value;
pub struct ExpectedEntry(TypeId, &'static str, &'static str);
pub fn entry<T: 'static>(id: &'static str) -> ExpectedEntry {
    named_entry::<T>(id, id)
}
pub fn named_entry<T: 'static>(id: &'static str, name: &'static str) -> ExpectedEntry {
    ExpectedEntry(TypeId::of::<T>(), id, name)
}
fn fixture(case: &str, schema: &MetaMap) {
    let golden: Value =
        norito::json::from_str(include_str!("../fixtures/schema_semantic_golden.json"))
            .expect("valid schema semantic golden");
    let expected = golden.get(case).expect("schema golden case");
    let actual = norito::json::to_value(schema).expect("serialize schema");
    assert_eq!(&actual, expected, "schema golden `{case}`");
}
fn lookup(schema: &MetaMap, rust_type: TypeId) -> Option<&MetaMapEntry> {
    schema
        .iter()
        .find_map(|(key, value)| (*key == rust_type).then_some(value))
}
fn identities(case: &str, schema: &MetaMap, expected: &[ExpectedEntry]) {
    assert_eq!(schema.iter().count(), expected.len(), "{case} count");
    for (index, ExpectedEntry(rust_type, id, name)) in expected.iter().enumerate() {
        assert!(
            !expected[..index].iter().any(|entry| entry.0 == *rust_type),
            "duplicate expected TypeId `{case}`"
        );
        let actual = lookup(schema, *rust_type)
            .unwrap_or_else(|| panic!("missing expected TypeId `{case}`"));
        assert_eq!(actual.type_id, *id, "type id `{case}`");
        assert_eq!(actual.type_name, *name, "type name `{case}`");
    }
}
pub fn assert_schema<T: IntoSchema>(case: &str, expected: &[ExpectedEntry]) {
    let schema = T::schema();
    identities(case, &schema, expected);
    fixture(case, &schema);
}
pub fn assert_schema_map(case: &str, schema: &MetaMap, expected: &[ExpectedEntry]) {
    identities(case, schema, expected);
    fixture(case, schema);
}
pub fn assert_root_schema<T: IntoSchema>(case: &str, id: &str, name: &str) {
    let schema = T::schema();
    let root = lookup(&schema, TypeId::of::<T>())
        .unwrap_or_else(|| panic!("missing root TypeId `{case}`"));
    assert_eq!(root.type_id, id, "root type id `{case}`");
    assert_eq!(root.type_name, name, "root type name `{case}`");
    fixture(case, &schema);
}
