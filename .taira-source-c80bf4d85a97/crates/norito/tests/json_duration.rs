use std::time::Duration;

use norito::json;

#[test]
fn duration_roundtrip() {
    let d = Duration::new(3, 42);
    let json = json::to_json(&d).expect("serialize duration");
    assert_eq!(json, "{\"secs\":3,\"nanos\":42}");

    let decoded: Duration = json::from_json(&json).expect("deserialize duration");
    assert_eq!(decoded, d);
}

#[test]
fn duration_missing_fields() {
    let err = json::from_json::<Duration>("{\"secs\":3}").unwrap_err();
    assert!(err.to_string().contains("missing field"));
}
