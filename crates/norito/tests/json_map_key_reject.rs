//! Tests for typed JSON object-key encoding, decoding, and duplicate rejection.
use norito::json::{self, JsonDeserialize};
use std::collections::HashMap;
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
#[test]
fn bool_map_key_rejects_non_bool_inputs() {
    for key in ["", "truthy", "False", "0", "yes", " true "] {
        let mut map = norito::json::Map::new();
        map.insert(key.to_owned(), norito::json!(1));
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
}
#[test]
fn hashset_rejects_duplicate_elements_for_deterministic_values() {
    for value in [0_u32, 1, 42, u32::MAX] {
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
}
#[test]
fn btreeset_rejects_duplicate_elements_for_deterministic_values() {
    for value in [0_u32, 1, 42, u32::MAX] {
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
}
#[test]
fn numeric_duplicate_strings_canonicalise_for_deterministic_values() {
    for (value, pad) in [(0_u64, 1_usize), (1, 2), (42, 3), (u64::MAX, 4)] {
        let canonical = value.to_string();
        let padded = format!("{:0width$}", value, width = canonical.len() + pad);
        assert_ne!(canonical, padded);
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
}
#[test]
fn numeric_map_keys_roundtrip_for_deterministic_values() {
    for values in [
        Vec::new(),
        vec![0_u64],
        vec![1, 1, 2, 3, 3],
        vec![u64::MAX, 0, 42, u64::MAX],
    ] {
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

#[test]
fn object_key_scalars_roundtrip_with_canonical_quoted_text() {
    use norito::json::{JsonObjectKey, JsonObjectKeyOwned};
    use std::{
        fmt::Debug,
        num::{NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128, NonZeroUsize},
    };

    fn check<K>(key: K, expected_key: &str)
    where
        K: JsonObjectKey + JsonObjectKeyOwned + Clone + Debug + Eq + Ord,
    {
        let map = std::collections::BTreeMap::from([(key.clone(), 7_u8)]);
        let expected = format!(r#"{{"{expected_key}":7}}"#);
        let encoded = json::to_json(&map).expect("serialize typed object key");
        assert_eq!(encoded, expected);
        assert_eq!(
            json::from_json::<std::collections::BTreeMap<K, u8>>(&encoded)
                .expect("decode typed object key"),
            map
        );
    }

    check(false, "false");
    check(true, "true");
    check(0_u8, "0");
    check(u8::MAX, "255");
    check(u16::MAX, "65535");
    check(u32::MAX, "4294967295");
    check(u64::MAX, "18446744073709551615");
    check(u128::MAX, "340282366920938463463374607431768211455");
    check(usize::MAX, &usize::MAX.to_string());
    check(i8::MIN, "-128");
    check(i16::MIN, "-32768");
    check(i32::MIN, "-2147483648");
    check(i64::MIN, "-9223372036854775808");
    check(isize::MIN, &isize::MIN.to_string());
    check(NonZeroU16::new(1).unwrap(), "1");
    check(NonZeroU32::new(2).unwrap(), "2");
    check(NonZeroU64::new(3).unwrap(), "3");
    check(
        NonZeroU128::new(u128::MAX).unwrap(),
        "340282366920938463463374607431768211455",
    );
    check(
        NonZeroUsize::new(usize::MAX).unwrap(),
        &usize::MAX.to_string(),
    );
}

#[derive(Debug, PartialEq, Eq, norito::JsonSerialize, norito::JsonDeserialize)]
struct NestedNumericMap {
    values: std::collections::BTreeMap<u16, u8>,
}

#[test]
fn nested_numeric_map_uses_the_same_bounded_key_writer() {
    let value = NestedNumericMap {
        values: std::collections::BTreeMap::from([(3, 4), (17, 18)]),
    };
    let expected = r#"{"values":{"3":4,"17":18}}"#;
    assert_eq!(
        json::to_json(&value).expect("ordinary nested map"),
        expected
    );
    assert_eq!(
        json::to_json_bounded(&value, expected.len()).expect("exact bounded nested map"),
        expected
    );
    assert_eq!(
        json::to_json_bounded(&value, expected.len() - 1),
        Err(json::BoundedJsonError::BodyTooLarge)
    );
    assert_eq!(
        json::from_json::<NestedNumericMap>(expected).unwrap(),
        value
    );
}

#[test]
fn string_object_keys_preserve_canonical_escaping_and_bytes() {
    let key = "quote\" slash\\ line\ncontrol\u{0008} snowman☃".to_owned();
    let map = std::collections::BTreeMap::from([(key.clone(), 1_u8)]);
    let encoded = json::to_json(&map).expect("serialize escaped key");
    assert_eq!(
        encoded,
        "{\"quote\\\" slash\\\\ line\\ncontrol\\b snowman☃\":1}"
    );
    assert_eq!(
        json::to_json_bounded(&map, encoded.len()).expect("bounded escaped key"),
        encoded
    );
    assert_eq!(
        json::from_json::<std::collections::BTreeMap<String, u8>>(&encoded).unwrap(),
        map
    );
}

#[test]
fn byte_array_object_key_streams_uppercase_hex_at_exact_bound() {
    let map = std::collections::BTreeMap::from([([0x00_u8, 0xab, 0xff], 9_u8)]);
    let expected = r#"{"00ABFF":9}"#;
    assert_eq!(json::to_json(&map).unwrap(), expected);
    assert_eq!(
        json::to_json_bounded(&map, expected.len()).unwrap(),
        expected
    );
    assert_eq!(
        json::to_json_bounded(&map, expected.len() - 1),
        Err(json::BoundedJsonError::BodyTooLarge)
    );
    assert_eq!(
        json::from_json::<std::collections::BTreeMap<[u8; 3], u8>>(expected).unwrap(),
        map
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CheckedRejectingKey;

impl json::JsonObjectKey for CheckedRejectingKey {
    fn visit_json_key_text<E>(
        &self,
        mut visitor: impl FnMut(&str) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor("ordinary")
    }

    fn visit_json_key_text_checked(
        &self,
        _visitor: impl FnMut(&str) -> Result<(), json::BoundedJsonError>,
    ) -> Result<(), json::BoundedJsonError> {
        Err(json::BoundedJsonError::Unsupported)
    }
}

#[test]
fn object_key_checked_conversion_errors_propagate() {
    let map = std::collections::BTreeMap::from([(CheckedRejectingKey, 1_u8)]);
    assert_eq!(json::to_json(&map).unwrap(), r#"{"ordinary":1}"#);
    assert_eq!(
        json::to_json_bounded(&map, usize::MAX),
        Err(json::BoundedJsonError::Unsupported)
    );

    let key = CheckedRejectingKey;
    let borrowed = std::collections::BTreeMap::from([(&key, 1_u8)]);
    assert_eq!(
        json::to_json_bounded(&borrowed, usize::MAX),
        Err(json::BoundedJsonError::Unsupported),
        "borrowing a key must retain its checked conversion"
    );
}

#[test]
fn borrowed_string_object_keys_use_canonical_escaping() {
    let map = std::collections::BTreeMap::from([("borrowed\nkey", 1_u8)]);
    let expected = r#"{"borrowed\nkey":1}"#;
    assert_eq!(json::to_json(&map).unwrap(), expected);
    assert_eq!(
        json::to_json_bounded(&map, expected.len()).unwrap(),
        expected
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ChunkedEscapedKey;

impl json::JsonObjectKey for ChunkedEscapedKey {
    fn visit_json_key_text<E>(
        &self,
        mut visitor: impl FnMut(&str) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor("quote\"")?;
        visitor(" slash\\")?;
        visitor(" line\n")?;
        visitor(" snowman☃")
    }
}

#[test]
fn chunked_object_key_text_uses_one_canonical_escape_writer() {
    let map = std::collections::BTreeMap::from([(ChunkedEscapedKey, 1_u8)]);
    let expected = "{\"quote\\\" slash\\\\ line\\n snowman☃\":1}";
    assert_eq!(json::to_json(&map).unwrap(), expected);
    assert_eq!(
        json::to_json_bounded(&map, expected.len()).unwrap(),
        expected
    );
}

#[test]
fn object_key_parser_rejects_invalid_string_grammar_before_typed_decode() {
    for input in [r#"{"\uD800":1}"#, r#"{"\uDC00":1}"#, r#"{"key"x:1}"#] {
        assert!(
            json::from_json::<std::collections::BTreeMap<String, u8>>(input).is_err(),
            "invalid object key must be rejected: {input}"
        );
    }
}

#[test]
fn object_key_escaping_matches_string_codec_for_all_control_characters() {
    let mut key: String = ('\u{0000}'..='\u{001f}').collect();
    key.push_str("quote\" slash\\ snowman☃ musical𝄞");
    let expected = format!("{{{}:1}}", json::to_json(&key).unwrap());
    let map = std::collections::BTreeMap::from([(key, 1_u8)]);
    assert_eq!(json::to_json(&map).unwrap(), expected);
    assert_eq!(
        json::to_json_bounded(&map, expected.len()).unwrap(),
        expected
    );
}

#[test]
fn display_key_text_preserves_raw_chunks_and_the_first_visitor_error() {
    struct DisplayChunks;
    impl std::fmt::Display for DisplayChunks {
        fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            out.write_str("quote\"")?;
            out.write_str("line\n")
        }
    }
    let mut text = String::new();
    json::visit_json_display_text(&DisplayChunks, |chunk| {
        text.push_str(chunk);
        Ok(())
    })
    .unwrap();
    assert_eq!(text, "quote\"line\n");

    // Even a formatter that ignores write failures must not resume the visitor.
    struct IgnoresWriteError;
    impl std::fmt::Display for IgnoresWriteError {
        fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let _ = out.write_str("first");
            let _ = out.write_str("second");
            Ok(())
        }
    }
    let mut visits = 0;
    let result = json::visit_json_display_text(&IgnoresWriteError, |_| {
        visits += 1;
        Err(json::BoundedJsonError::BodyTooLarge)
    });
    assert_eq!(result, Err(json::BoundedJsonError::BodyTooLarge));
    assert_eq!(visits, 1);

    struct InvalidDisplay;
    impl std::fmt::Display for InvalidDisplay {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            Err(std::fmt::Error)
        }
    }
    assert_eq!(
        json::visit_json_display_text(&InvalidDisplay, |_| Ok(())),
        Err(json::BoundedJsonError::Unsupported)
    );
}
