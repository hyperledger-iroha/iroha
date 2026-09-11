//! Captured schema identities and complete frames for generated instruction records.

use std::{collections::BTreeMap, fmt::Debug, sync::OnceLock};

use norito::{
    NoritoDeserialize, NoritoSchema, NoritoSerialize,
    json::{self, Value},
};

#[path = "generated_record_values.rs"]
mod values;

#[path = "generated_record_inventory.rs"]
mod inventory;

#[path = "kaigi_record_identity_tests.rs"]
mod kaigi_records;

use crate::fixture_json;

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for byte in bytes {
        write!(out, "{byte:02x}").expect("String formatting");
    }
    out
}

fn frame<T>(value: &T) -> String
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + Debug + PartialEq,
{
    let bytes = norito::to_bytes(value).expect("encode instruction record frame");
    let decoded: T = norito::decode_from_bytes(&bytes)
        .unwrap_or_else(|error| panic!("decode {} frame: {error}", core::any::type_name::<T>()));
    assert_eq!(&decoded, value);
    assert_eq!(norito::to_bytes(&decoded).expect("reencode record"), bytes);
    hex(&bytes)
}

/// Render the declared identity and complete root and container frames for comparison.
pub fn capture<T>(value: T) -> Value
where
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de> + Clone + Debug + PartialEq,
{
    let identity = norito::schema::identity::frame_hash::<T>();
    assert_eq!(identity, norito::schema::identity::frame_hash::<T>());
    assert_eq!(identity, norito::schema::identity::frame_hash::<T>());
    json::object([
        ("nominal", Value::String(T::nominal_name())),
        (
            "serialize_hash",
            Value::String(hex(&norito::schema::identity::frame_hash::<T>())),
        ),
        (
            "deserialize_hash",
            Value::String(hex(&norito::schema::identity::frame_hash::<T>())),
        ),
        ("frame", Value::String(frame(&value))),
        ("vector_frame", Value::String(frame(&vec![value.clone()]))),
        ("option_frame", Value::String(frame(&Some(value.clone())))),
        (
            "map_frame",
            Value::String(frame(&BTreeMap::from([(7_u8, value)]))),
        ),
    ])
    .expect("generated instruction record")
}

fn missing_record_values() -> Vec<Value> {
    let mut records = values::values();
    records.extend(super::musubi::generated_identity_values::values());
    records.extend(super::kagemusha_v1::generated_identity_values::values());
    records.extend(super::private_settlement::generated_identity_values::values());
    records.push(capture(
        super::privacy::RegisterPrivacyExact12QualificationV1::new(
            crate::privacy::tests::generated_instruction_qualification(),
        ),
    ));
    assert_eq!(
        records.len(),
        51,
        "complete missing record fixture inventory"
    );
    let names: std::collections::BTreeSet<_> = records
        .iter()
        .map(|row| row.get("nominal").and_then(Value::as_str).expect("nominal"))
        .collect();
    assert_eq!(names.len(), 51, "one populated value per missing record");
    records.sort_by(|a, b| {
        a.get("nominal")
            .and_then(Value::as_str)
            .cmp(&b.get("nominal").and_then(Value::as_str))
    });
    records
}

fn captured(nominal: &str) -> &'static Value {
    static CAPTURE: OnceLock<Value> = OnceLock::new();
    let rows = CAPTURE
        .get_or_init(|| {
            use sha2::{Digest as _, Sha256};

            let source = include_str!(
                "../../tests/fixtures/instruction_record_generated_identity_frames.json"
            );
            assert_eq!(
                hex(&Sha256::digest(source.as_bytes())),
                "7e69371c0072539ff3d85952169da3e4185aa66c66580967a33ce697112d95ac",
                "instruction record capture digest drift"
            );
            let capture: Value =
                json::from_str(source).expect("immutable instruction record capture");
            let rows = capture.as_array().expect("captured type rows");
            assert_eq!(rows.len(), 322, "complete instantiated record inventory");
            let mut previous = None;
            let mut case_count = 0;
            for row in rows {
                let nominal = row
                    .get("nominal")
                    .and_then(Value::as_str)
                    .expect("captured nominal");
                if let Some(previous) = previous {
                    assert!(previous < nominal, "captured nominals are strictly sorted");
                }
                previous = Some(nominal);
                case_count += row
                    .get("cases")
                    .and_then(Value::as_array)
                    .expect("captured cases")
                    .len();
            }
            assert_eq!(case_count, 357, "complete populated record case inventory");
            capture
        })
        .as_array()
        .expect("captured type rows");
    assert_eq!(rows.len(), 322, "complete instantiated record inventory");
    let mut matches = rows
        .iter()
        .filter(|row| row.get("nominal").and_then(Value::as_str) == Some(nominal));
    let row = matches.next().expect("captured type");
    assert!(matches.next().is_none(), "exactly one captured type row");
    row
}

fn frame_fields(row: &Value) -> Value {
    json::object(
        ["frame", "vector_frame", "option_frame", "map_frame"].map(|key| {
            (
                key,
                row.get(key).expect("complete captured frame set").clone(),
            )
        }),
    )
    .expect("frame case")
}

fn unhex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("fixture hex UTF8"), 16)
                .expect("fixture hex byte")
        })
        .collect()
}

fn check<T>(nominal: &str)
where
    T: NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de> + Clone + Debug + PartialEq,
{
    assert_eq!(T::nominal_name(), nominal, "captured compiler identity");
    assert_eq!(T::frame_name(), nominal, "captured frame identity");
    let row = captured(nominal);
    assert_eq!(
        row.get("serialize_hash").and_then(Value::as_str),
        Some(hex(&norito::schema::identity::frame_hash::<T>()).as_str())
    );
    assert_eq!(
        row.get("deserialize_hash").and_then(Value::as_str),
        Some(hex(&norito::schema::identity::frame_hash::<T>()).as_str())
    );
    let cases = row
        .get("cases")
        .and_then(Value::as_array)
        .expect("captured cases");
    assert!(!cases.is_empty(), "every type has a populated frame");
    for expected in cases {
        let bytes = unhex(
            expected
                .get("frame")
                .and_then(Value::as_str)
                .expect("root frame"),
        );
        let value: T = norito::decode_from_bytes(&bytes)
            .unwrap_or_else(|error| panic!("decode captured {nominal}: {error}"));
        let actual = frame_fields(&capture(value));
        fixture_json::assert_json_matches(expected, &actual, nominal);
    }
}

#[test]
fn populated_record_values_preserve_the_original_capture() {
    for actual in missing_record_values() {
        let nominal = actual
            .get("nominal")
            .and_then(Value::as_str)
            .expect("nominal");
        let expected = captured(nominal);
        for key in ["nominal", "serialize_hash", "deserialize_hash"] {
            fixture_json::assert_json_matches(
                expected.get(key).expect("identity"),
                actual.get(key).expect("actual identity"),
                nominal,
            );
        }
        let cases = expected
            .get("cases")
            .and_then(Value::as_array)
            .expect("cases");
        assert_eq!(cases.len(), 1, "one original populated value for each gap");
        fixture_json::assert_json_matches(&cases[0], &frame_fields(&actual), nominal);
    }
}
