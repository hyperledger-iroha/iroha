use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use norito::json::{self, Error, Value};

#[test]
fn from_value_vec_avoids_string_roundtrip() {
    let input = Value::Array(vec![
        Value::from(1u64),
        Value::from(2u64),
        Value::from(3u64),
    ]);
    let output: Vec<u64> = json::from_value(input).expect("vec from value");
    assert_eq!(output, vec![1, 2, 3]);
}

#[test]
fn from_value_option_handles_null() {
    let none: Option<String> = json::from_value(Value::Null).expect("option none");
    assert!(none.is_none());

    let some: Option<String> =
        json::from_value(Value::String("hello".to_owned())).expect("option some");
    assert_eq!(some.as_deref(), Some("hello"));
}

#[test]
fn from_value_hashset_reports_duplicates() {
    let dup = Value::Array(vec![Value::from(1u64), Value::from(1u64)]);
    let err = json::from_value::<HashSet<u64>>(dup).expect_err("duplicate should fail");
    match err {
        Error::Message(msg) => assert_eq!(msg, "duplicate element in set"),
        other => panic!("expected duplicate error, got {other:?}"),
    }
}

#[test]
fn from_value_hashmap_object() {
    let mut obj = Value::Object(BTreeMap::new());
    if let Value::Object(map) = &mut obj {
        map.insert("a".into(), Value::from(1u64));
        map.insert("b".into(), Value::from(2u64));
    }
    let out: HashMap<String, u64> = json::from_value(obj).expect("hashmap");
    assert_eq!(out.get("a"), Some(&1));
    assert_eq!(out.get("b"), Some(&2));
}

#[test]
fn from_value_btreemap_object() {
    let mut map = Value::Object(BTreeMap::new());
    if let Value::Object(obj) = &mut map {
        obj.insert("foo".into(), Value::from(5u64));
        obj.insert("bar".into(), Value::from(6u64));
    }
    let out: BTreeMap<String, u64> = json::from_value(map).expect("btreemap");
    assert_eq!(out.get("bar"), Some(&6));
    assert_eq!(out.get("foo"), Some(&5));
}

#[test]
fn from_value_hashmap_numeric_keys() {
    let mut obj = Value::Object(BTreeMap::new());
    if let Value::Object(map) = &mut obj {
        map.insert("1".into(), Value::from("one"));
        map.insert("2".into(), Value::from("two"));
    }
    let out: HashMap<u32, String> = json::from_value(obj).expect("numeric keys");
    assert_eq!(out.get(&1), Some(&"one".to_string()));
    assert_eq!(out.get(&2), Some(&"two".to_string()));
}

#[test]
fn from_value_hashmap_bool_keys() {
    let mut obj = Value::Object(BTreeMap::new());
    if let Value::Object(map) = &mut obj {
        map.insert("true".into(), Value::from(1u64));
        map.insert("false".into(), Value::from(0u64));
    }
    let out: HashMap<bool, u64> = json::from_value(obj).expect("bool keys");
    assert_eq!(out.get(&true), Some(&1));
    assert_eq!(out.get(&false), Some(&0));
}

#[test]
fn from_value_hex_array() {
    let raw = Value::String("0a0b0c0d".to_owned());
    let bytes: [u8; 4] = json::from_value(raw).expect("decode hex");
    assert_eq!(bytes, [0x0A, 0x0B, 0x0C, 0x0D]);
}

#[test]
fn from_value_value_clones() {
    let original = Value::Bool(true);
    let cloned: Value = json::from_value(original.clone()).expect("clone value");
    assert_eq!(cloned, original);
}

#[test]
fn from_value_btreeset_roundtrip() {
    let input = Value::Array(vec![
        Value::from(3u64),
        Value::from(1u64),
        Value::from(2u64),
    ]);
    let out: BTreeSet<u64> = json::from_value(input).expect("btreeset");
    let expected: BTreeSet<u64> = [1u64, 2u64, 3u64].into_iter().collect();
    assert_eq!(out, expected);
}
