#![cfg(feature = "json")]

use norito::json::native::Value;

#[test]
fn value_kind_detectors_work() {
    let null = Value::Null;
    assert!(null.is_null());
    let boolean = Value::from(true);
    assert!(boolean.is_bool());
    let number = Value::from(42u64);
    assert!(number.is_number());
    let string = Value::from("str");
    assert!(string.is_string());
    let array = norito::json!([1, 2, 3]);
    assert!(array.is_array());
    let object = norito::json!({ "k": 1 });
    assert!(object.is_object());
}

#[test]
fn value_get_helpers() {
    let value = norito::json!({ "numbers": [10, 20, 30], "flag": true });
    assert_eq!(value.get("flag").and_then(Value::as_bool), Some(true));
    assert_eq!(value.get("missing"), None);
    assert_eq!(
        value
            .get("numbers")
            .and_then(Value::as_array)
            .map(|a| a.len()),
        Some(3)
    );
    let key = String::from("flag");
    assert_eq!(value.get(key).and_then(Value::as_bool), Some(true));
    assert_eq!(
        value
            .get("numbers")
            .and_then(|v| v.get(1))
            .and_then(Value::as_u64),
        Some(20)
    );
}

#[test]
fn value_get_mut_helpers() {
    let mut value = norito::json!({ "numbers": [1, 2, 3] });
    if let Some(elem) = value.get_mut("numbers").and_then(|v| v.get_mut(0)) {
        *elem = Value::from(9u64);
    }
    assert_eq!(value.pointer("/numbers/0").and_then(Value::as_u64), Some(9));
}

#[test]
fn value_pointer_navigation() {
    let value = norito::json!({
        "users": [
            { "id": 1, "name": "alice" },
            { "id": 2, "name": "bob" }
        ],
        "meta": { "count": 2 }
    });
    assert_eq!(value.pointer("").unwrap().as_object().unwrap().len(), 2);
    assert_eq!(
        value.pointer("/users/1/name").and_then(Value::as_str),
        Some("bob")
    );
    assert_eq!(
        value.pointer("/meta/count").and_then(Value::as_u64),
        Some(2)
    );
    assert!(value.pointer("/users/idx").is_none());

    let escaped = norito::json!({ "a/b": { "til~de": 1u64 } });
    assert_eq!(
        escaped.pointer("/a~1b/til~0de").and_then(Value::as_u64),
        Some(1)
    );
}

#[test]
fn value_pointer_mut_allows_updates() {
    let mut value = norito::json!({ "scores": [3, 5, 7] });
    if let Some(slot) = value.pointer_mut("/scores/2") {
        *slot = Value::from(11u64);
    }
    assert_eq!(value.pointer("/scores/2").and_then(Value::as_u64), Some(11));
}

#[test]
fn value_take_resets_to_null() {
    let mut value = norito::json!({ "v": 123 });
    let slot = value.pointer_mut("/v").unwrap();
    let taken = slot.take();
    assert_eq!(taken.as_u64(), Some(123));
    assert!(slot.is_null());
}

#[test]
fn value_roundtrips_via_json_traits() {
    let value = norito::json!({
        "numbers": [1, 2, 3],
        "flag": true,
        "object": { "name": "alice" }
    });
    let rendered = norito::json::to_json(&value).expect("serialize value");
    let decoded = norito::json::from_json::<Value>(&rendered).expect("deserialize value");
    assert_eq!(decoded, value);
}

#[test]
fn map_roundtrips_via_json_traits() {
    let mut map = norito::json::Map::new();
    map.insert("a".to_owned(), Value::from(1u64));
    map.insert("b".to_owned(), Value::String("text".to_owned()));
    let rendered = norito::json::to_json(&map).expect("serialize map");
    let decoded = norito::json::from_json::<norito::json::Map>(&rendered).expect("deserialize map");
    assert_eq!(decoded, map);
}

#[test]
fn value_as_f64_covering_integer_inputs() {
    let int = Value::from(-4i64);
    assert_eq!(int.as_f64(), Some(-4.0));
    let unsigned = Value::from(7u64);
    assert_eq!(unsigned.as_f64(), Some(7.0));
    let float = Value::from(1.25f64);
    assert_eq!(float.as_f64(), Some(1.25));
}
