//! Tests covering `json_from_map_key` fast-path validation and duplicate detection.

use std::collections::HashMap;

use norito::json::{self, JsonDeserialize};
use proptest::{prelude::*, prop_assume};

#[test]
fn json_map_key_fast_path_rejects_invalid_keys() {
    let invalid = norito::json!({ "truthy": 1 });
    let err = HashMap::<bool, u32>::json_from_value(&invalid).expect_err("expected bool rejection");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "expected bool"),
        other => panic!("expected message error, got {other:?}"),
    }

    let err = json::from_json::<HashMap<bool, u32>>(r#"{"truthy":1}"#)
        .expect_err("parser path should reject invalid bool keys");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "expected bool"),
        other => panic!("expected message error, got {other:?}"),
    }
    let dup = norito::json!({ "1": 0, "01": 1 });
    let err = HashMap::<u64, u32>::json_from_value(&dup)
        .expect_err("duplicate numeric keys should be rejected");
    match err {
        json::Error::DuplicateField { field } => {
            assert!(
                field == "1" || field == "01",
                "unexpected duplicate key: {field}"
            );
        }
        other => panic!("expected duplicate-field error, got {other:?}"),
    }

    let err = json::from_json::<HashMap<u64, u32>>(r#"{"1":0,"01":1}"#)
        .expect_err("parser path should detect duplicate numeric keys");
    match err {
        json::Error::DuplicateField { field } => assert_eq!(field, "01"),
        other => panic!("expected duplicate-field error, got {other:?}"),
    }
}

#[test]
fn json_map_key_rejects_invalid_numeric_keys() {
    let invalid = norito::json!({ "not_an_int": 1 });
    let err = HashMap::<u64, u32>::json_from_value(&invalid)
        .expect_err("value path should reject non-numeric map keys");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "expected u64"),
        other => panic!("expected message error, got {other:?}"),
    }

    let err = json::from_json::<HashMap<u64, u32>>(r#"{"not_an_int":1}"#)
        .expect_err("parser path should reject non-numeric map keys");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "expected u64"),
        other => panic!("expected message error, got {other:?}"),
    }
}

#[test]
fn json_map_key_rejects_negative_numbers() {
    let invalid = norito::json!({ "-1": 1 });
    let err = HashMap::<u64, u32>::json_from_value(&invalid)
        .expect_err("value path should reject negative keys for unsigned map");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "expected u64"),
        other => panic!("expected message error, got {other:?}"),
    }

    let err = json::from_json::<HashMap<u64, u32>>(r#"{"-1":1}"#)
        .expect_err("parser path should reject negative keys for unsigned map");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "expected u64"),
        other => panic!("expected message error, got {other:?}"),
    }
}

#[test]
fn json_set_fast_path_rejects_duplicates() {
    let dup = norito::json!([1, 1]);
    let err = std::collections::HashSet::<u32>::json_from_value(&dup)
        .expect_err("value path should reject duplicate set elements");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "duplicate element in set"),
        other => panic!("expected duplicate-element error, got {other:?}"),
    }

    let err = json::from_json::<std::collections::HashSet<u32>>(r#"[1,1]"#)
        .expect_err("parser path should reject duplicate set elements");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "duplicate element in set"),
        other => panic!("expected duplicate-element error, got {other:?}"),
    }
}

#[test]
fn json_btreeset_fast_path_rejects_duplicates() {
    let dup = norito::json!([1, 1]);
    let err = std::collections::BTreeSet::<u32>::json_from_value(&dup)
        .expect_err("value path should reject duplicate BTreeSet elements");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "duplicate element in set"),
        other => panic!("expected duplicate-element error, got {other:?}"),
    }

    let err = json::from_json::<std::collections::BTreeSet<u32>>(r#"[1,1]"#)
        .expect_err("parser path should reject duplicate BTreeSet elements");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "duplicate element in set"),
        other => panic!("expected duplicate-element error, got {other:?}"),
    }
}

#[test]
fn json_map_bool_duplicate_detection() {
    let err = json::from_json::<HashMap<bool, u32>>(r#"{"true":0,"true":1}"#)
        .expect_err("parser path should detect duplicate bool keys");
    match err {
        json::Error::DuplicateField { field } => assert_eq!(field, "true"),
        other => panic!("expected duplicate-field error, got {other:?}"),
    }
}

#[test]
fn json_btreemap_duplicate_detection() {
    let dup = norito::json!({ "1": 0, "01": 1 });
    let err = std::collections::BTreeMap::<u64, u32>::json_from_value(&dup)
        .expect_err("value path should reject duplicate numeric keys");
    match err {
        json::Error::DuplicateField { field } => assert!(field == "1" || field == "01"),
        other => panic!("expected duplicate-field error, got {other:?}"),
    }

    let err = json::from_json::<std::collections::BTreeMap<u64, u32>>(r#"{"1":0,"01":1}"#)
        .expect_err("parser path should detect duplicate numeric keys");
    match err {
        json::Error::DuplicateField { field } => assert_eq!(field, "01"),
        other => panic!("expected duplicate-field error, got {other:?}"),
    }
}

#[test]
fn json_map_key_rejects_numeric_overflow() {
    let overflow = norito::json!({ "300": 1 });
    let err = HashMap::<u8, u32>::json_from_value(&overflow)
        .expect_err("value path should reject u8 overflow");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "u8 overflow"),
        other => panic!("expected overflow error, got {other:?}"),
    }

    let err = json::from_json::<HashMap<u8, u32>>(r#"{"300":1}"#)
        .expect_err("parser path should reject u8 overflow");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "u8 overflow"),
        other => panic!("expected overflow error, got {other:?}"),
    }
}

#[test]
fn json_map_key_rejects_nonzero_zero_key() {
    let zeroish = norito::json!({ "0": 1 });
    let err = HashMap::<core::num::NonZeroU32, u32>::json_from_value(&zeroish)
        .expect_err("value path should reject zero for NonZero keys");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "expected non-zero u32"),
        other => panic!("expected non-zero error, got {other:?}"),
    }

    let err = json::from_json::<HashMap<core::num::NonZeroU32, u32>>(r#"{"0":1}"#)
        .expect_err("parser path should reject zero for NonZero keys");
    match err {
        json::Error::Message(msg) => assert_eq!(msg, "expected non-zero u32"),
        other => panic!("expected non-zero error, got {other:?}"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        failure_persistence: None,
        .. ProptestConfig::default()
    })]
    #[test]
    fn bool_map_key_rejects_non_bool_inputs(key in any::<String>().prop_filter("reject canonical bool strings", |s| s != "true" && s != "false")) {
        let mut map = norito::json::Map::new();
        map.insert(key.clone(), norito::json!(1));
        let value = norito::json::Value::Object(map);

        let err = HashMap::<bool, u8>::json_from_value(&value)
            .expect_err("value path should reject non-bool keys");
        match err {
            json::Error::Message(msg) => assert_eq!(msg, "expected bool"),
            other => panic!("expected bool error, got {other:?}"),
        }

        let json_text = norito::json::to_json(&value).expect("serialize test map");
        let err = json::from_json::<HashMap<bool, u8>>(&json_text)
            .expect_err("parser path should reject non-bool keys");
        match err {
            json::Error::Message(msg) => assert_eq!(msg, "expected bool"),
            other => panic!("expected bool error, got {other:?}"),
        }
    }

    #[test]
    fn hashset_rejects_duplicate_elements(value in any::<u32>()) {
        let arr = norito::json!([value, value]);
        let err = std::collections::HashSet::<u32>::json_from_value(&arr)
            .expect_err("value path should reject duplicate set elements");
        match err {
            json::Error::Message(msg) => assert_eq!(msg, "duplicate element in set"),
            other => panic!("expected duplicate-element error, got {other:?}"),
        }

        let json_text = norito::json::to_json(&arr).expect("serialize duplicate set");
        let err = json::from_json::<std::collections::HashSet<u32>>(&json_text)
            .expect_err("parser path should reject duplicate set elements");
        match err {
            json::Error::Message(msg) => assert_eq!(msg, "duplicate element in set"),
            other => panic!("expected duplicate-element error, got {other:?}"),
        }
    }

    #[test]
    fn btreeset_rejects_duplicate_elements(value in any::<u32>()) {
        let arr = norito::json!([value, value]);
        let err = std::collections::BTreeSet::<u32>::json_from_value(&arr)
            .expect_err("value path should reject duplicate BTreeSet elements");
        match err {
            json::Error::Message(msg) => assert_eq!(msg, "duplicate element in set"),
            other => panic!("expected duplicate-element error, got {other:?}"),
        }

        let json_text = norito::json::to_json(&arr).expect("serialize duplicate set");
        let err = json::from_json::<std::collections::BTreeSet<u32>>(&json_text)
            .expect_err("parser path should reject duplicate BTreeSet elements");
        match err {
            json::Error::Message(msg) => assert_eq!(msg, "duplicate element in set"),
            other => panic!("expected duplicate-element error, got {other:?}"),
        }
    }

    #[test]
    fn numeric_duplicate_strings_canonicalise(value in any::<u64>(), pad in 1usize..5) {
        let canonical = value.to_string();
        let padded = format!("{:0width$}", value, width = canonical.len() + pad);
        prop_assume!(canonical != padded);

        let mut map = norito::json::Map::new();
        map.insert(canonical.clone(), norito::json!(0));
        map.insert(padded.clone(), norito::json!(1));
        let json_map = norito::json::Value::Object(map);

        let err = HashMap::<u64, u32>::json_from_value(&json_map)
            .expect_err("value path should reject duplicate numeric key encodings");
        match err {
            json::Error::DuplicateField { field } => {
                assert!(
                    field == canonical || field == padded,
                    "unexpected duplicate field: {field}"
                );
            }
            other => panic!("expected duplicate-field error, got {other:?}"),
        }

        let json_text = norito::json::to_json(&json_map).expect("serialize duplicate numeric map");
        let err = json::from_json::<HashMap<u64, u32>>(&json_text)
            .expect_err("parser path should reject duplicate numeric key encodings");
        match err {
            json::Error::DuplicateField { field } => {
                assert!(
                    field == canonical || field == padded,
                    "unexpected duplicate field: {field}"
                );
            }
            other => panic!("expected duplicate-field error, got {other:?}"),
        }
    }

    #[test]
    fn numeric_map_keys_roundtrip(values in prop::collection::vec(any::<u64>(), 0..20)) {
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        let mut json_map = norito::json::Map::new();
        for v in values {
            if seen.insert(v) {
                json_map.insert(v.to_string(), norito::json!(v));
            }
        }

        let value = norito::json::Value::Object(json_map.clone());
        let parsed = HashMap::<u64, u64>::json_from_value(&value)
            .expect("value path should decode canonical numeric keys");
        assert_eq!(parsed.len(), json_map.len());
        for (k, v) in &parsed {
            let expected = json_map
                .get(&k.to_string())
                .and_then(|val| val.as_u64())
                .expect("stored numeric value");
            assert_eq!(expected, *v);
        }

        let json_text = norito::json::to_json(&value).expect("serialize numeric map");
        let parsed_from_str = json::from_json::<HashMap<u64, u64>>(&json_text)
            .expect("parser path should decode canonical numeric keys");
        assert_eq!(parsed_from_str, parsed);
    }
}
