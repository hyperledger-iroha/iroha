//! Runtime coverage for strict Norito JSON derives.

use norito::json::{self, Error};
use norito_derive::{JsonDeserialize, JsonSerialize};

#[derive(Debug, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct StrictInner {
    value: u32,
}

#[derive(Debug, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct StrictOuter {
    child: StrictInner,
    children: Vec<StrictInner>,
}

#[derive(Debug, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
struct Permissive {
    value: u32,
}

#[derive(Debug, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct StrictOptional {
    optional: Option<u32>,
}

#[derive(Debug, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
struct FlattenedFields {
    label: String,
}

#[derive(Debug, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct StrictFlattened {
    id: u32,
    #[norito(flatten)]
    fields: FlattenedFields,
}

#[derive(Debug, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(tag = "kind", content = "payload", deny_unknown_fields)]
enum StrictEvent {
    Unit,
    Record { id: u32 },
}

fn assert_unknown_field(error: Error, expected: &str) {
    match error {
        Error::UnknownField { field } => assert_eq!(field, expected),
        other => panic!("expected unknown field `{expected}`, got {other:?}"),
    }
}

fn assert_duplicate_field(error: Error, expected: &str) {
    match error {
        Error::DuplicateField { field } => assert_eq!(field, expected),
        other => panic!("expected duplicate field `{expected}`, got {other:?}"),
    }
}

#[test]
fn named_struct_rejects_unknown_fields_without_changing_permissive_default() {
    let error = json::from_slice::<StrictInner>(br#"{"value":1,"future":2}"#)
        .expect_err("strict struct must reject unknown field");
    assert_unknown_field(error, "future");

    let permissive = json::from_slice::<Permissive>(br#"{"value":1,"future":2}"#)
        .expect("unannotated struct remains permissive");
    assert_eq!(permissive, Permissive { value: 1 });
}

#[test]
fn strictness_applies_to_nested_objects_and_array_elements() {
    for input in [
        br#"{"child":{"value":1,"future":2},"children":[]}"#.as_slice(),
        br#"{"child":{"value":1},"children":[{"value":2,"future":3}]}"#.as_slice(),
    ] {
        let error = json::from_slice::<StrictOuter>(input)
            .expect_err("nested strict struct must reject unknown field");
        assert_unknown_field(error, "future");
    }
}

#[test]
fn strict_struct_keeps_optional_absence_and_explicit_null_semantics() {
    assert_eq!(
        json::from_slice::<StrictOptional>(br#"{}"#).expect("omitted option"),
        StrictOptional { optional: None }
    );
    assert_eq!(
        json::from_slice::<StrictOptional>(br#"{"optional":null}"#).expect("explicit null option"),
        StrictOptional { optional: None }
    );
}

#[test]
fn strict_flatten_rejects_fields_not_consumed_by_flattened_value() {
    let decoded = json::from_slice::<StrictFlattened>(br#"{"id":7,"label":"known"}"#)
        .expect("known flattened fields");
    assert_eq!(
        decoded,
        StrictFlattened {
            id: 7,
            fields: FlattenedFields {
                label: "known".to_owned(),
            },
        }
    );

    let error =
        json::from_slice::<StrictFlattened>(br#"{"id":7,"label":"known","unclaimed":true}"#)
            .expect_err("unclaimed flattened field must reject");
    assert_unknown_field(error, "unclaimed");
}

#[test]
fn tagged_enum_rejects_unknown_envelope_and_variant_fields() {
    let direct_envelope =
        json::from_json::<StrictEvent>(r#"{"kind":"Unit","payload":null,"future":false}"#)
            .expect_err("strict enum envelope must reject unknown field on direct parser path");
    assert_unknown_field(direct_envelope, "future");

    let envelope =
        json::from_slice::<StrictEvent>(br#"{"kind":"Unit","payload":null,"future":false}"#)
            .expect_err("strict enum envelope must reject unknown field");
    assert_unknown_field(envelope, "future");

    let payload =
        json::from_slice::<StrictEvent>(br#"{"kind":"Record","payload":{"id":1,"future":false}}"#)
            .expect_err("strict enum payload must reject unknown field");
    assert_unknown_field(payload, "future");
}

#[test]
fn tagged_enum_rejects_duplicate_tag_and_content_on_direct_parser_path() {
    let duplicate_tag =
        json::from_json::<StrictEvent>(r#"{"kind":"Unit","kind":"Unit","payload":null}"#)
            .expect_err("duplicate tag must reject");
    assert_duplicate_field(duplicate_tag, "kind");

    let duplicate_content =
        json::from_json::<StrictEvent>(r#"{"kind":"Unit","payload":null,"payload":null}"#)
            .expect_err("duplicate content must reject");
    assert_duplicate_field(duplicate_content, "payload");
}

#[test]
fn value_conversion_path_enforces_unknown_fields() {
    let value: json::Value =
        json::from_slice(br#"{"value":1,"future":2}"#).expect("generic JSON value");
    let error = json::from_value::<StrictInner>(value)
        .expect_err("strict json_from_value must reject unknown field");
    assert_unknown_field(error, "future");
}

#[test]
fn preparse_rejects_escaped_duplicate_keys_recursively() {
    let error =
        json::from_slice::<StrictOuter>(br#"{"child":{"value":1,"\u0076alue":2},"children":[]}"#)
            .expect_err("escaped duplicate key must reject before typed decode");
    assert_duplicate_field(error, "value");
}
