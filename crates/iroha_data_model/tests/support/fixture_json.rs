//! Exact JSON fixture comparisons with bounded, path-specific diagnostics.

use norito::json::{self, Value};

fn summary(value: &Value) -> String {
    match value {
        Value::Array(values) => format!("array with {} elements", values.len()),
        Value::Object(values) => format!("object with {} keys", values.len()),
        Value::String(value) => {
            let prefix: String = value.chars().take(120).collect();
            if prefix.len() < value.len() {
                format!("{prefix:?}… ({} bytes)", value.len())
            } else {
                format!("{value:?}")
            }
        }
        other => json::to_string(other).expect("JSON scalar"),
    }
}

fn difference(actual: &Value, expected: &Value, path: &str) -> Option<String> {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            for (key, actual) in actual {
                let path = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                let Some(expected) = expected.get(key) else {
                    return Some(format!("{path}: unexpected fixture field"));
                };
                if let Some(difference) = difference(actual, expected, &path) {
                    return Some(difference);
                }
            }
            for key in expected.keys() {
                if !actual.contains_key(key) {
                    let key = key.replace('~', "~0").replace('/', "~1");
                    return Some(format!("{path}/{key}: missing fixture field"));
                }
            }
            None
        }
        (Value::Array(actual), Value::Array(expected)) => {
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                if let Some(difference) = difference(actual, expected, &format!("{path}/{index}")) {
                    return Some(difference);
                }
            }
            (actual.len() != expected.len()).then(|| {
                format!(
                    "{path}: fixture length {}, typed-owner length {}",
                    actual.len(),
                    expected.len()
                )
            })
        }
        _ => (actual != expected).then(|| {
            format!(
                "{path}: fixture {}, typed owner {}",
                summary(actual),
                summary(expected)
            )
        }),
    }
}

/// Assert exact fixture equality and report the first differing JSON pointer.
///
/// # Panics
/// Panics when a key, array position, value or JSON value type differs.
pub fn assert_json_matches(actual: &Value, expected: &Value, fixture: &str) {
    if let Some(difference) = difference(actual, expected, "") {
        panic!("{fixture}: {difference}");
    }
}

#[test]
fn exact_comparison_reports_nested_paths_without_dumping_fixture_documents() {
    let actual = norito::json!({"a/b": [("é".repeat(1_000))]});
    assert_json_matches(&actual, &actual, "equal fixture");
    let expected = norito::json!({"a/b": ["different"]});
    let difference = difference(&actual, &expected, "").expect("different scalar");
    assert!(difference.starts_with("/a~1b/0:"));
    assert!(difference.len() < 400);
}

#[test]
fn exact_comparison_rejects_missing_extra_and_reordered_values() {
    let actual = norito::json!({"value": [1, 2]});
    for expected in [
        norito::json!({"value": [2, 1]}),
        norito::json!({"value": [1]}),
        norito::json!({"value": [1, 2, 3]}),
        norito::json!({}),
        norito::json!({"value": [1, 2], "extra": null}),
    ] {
        assert!(difference(&actual, &expected, "").is_some());
    }
}
