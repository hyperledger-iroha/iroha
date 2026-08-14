//! Runtime coverage for strict Norito JSON derives.
use norito::json::JsonDeserialize as _;
use norito::json::{self, Arena, Error, FastFromJson, TapeWalker};
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
struct RequiredOptional {
    #[norito(required)]
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
struct RequiredWithFlatten {
    #[norito(required)]
    optional: Option<u32>,
    #[norito(flatten)]
    fields: FlattenedFields,
}
#[derive(Debug, PartialEq, Eq, norito_derive::FastJson)]
struct FastRequiredOptional {
    #[norito(required)]
    optional: Option<u32>,
}
#[derive(Debug, PartialEq, Eq, norito_derive::FastJson)]
struct FastRequiredWithFlatten {
    #[norito(required)]
    optional: Option<u32>,
    #[norito(flatten)]
    fields: FlattenedFields,
}
#[derive(Debug, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(tag = "kind", content = "payload", deny_unknown_fields)]
enum StrictEvent {
    Unit,
    Record { id: u32 },
}
#[derive(Debug, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(tag = "kind", content = "payload")]
enum RequiredEvent {
    Record {
        #[norito(required)]
        optional: Option<u32>,
    },
}
#[derive(Debug, PartialEq, Eq, norito_derive::FastJson)]
#[norito(tag = "kind", content = "payload")]
enum FastRequiredEvent {
    Record {
        #[norito(required)]
        optional: Option<u32>,
    },
}
fn decode_fast<'a, T: FastFromJson<'a>>(input: &'a str) -> Result<T, norito::Error> {
    let mut walker = TapeWalker::new(input);
    let mut arena = Arena::new();
    T::parse(&mut walker, &mut arena)
}
fn assert_missing_field(error: impl core::fmt::Display, field: &str) {
    let message = error.to_string();
    assert!(
        message.contains(&format!("missing field `{field}`")),
        "unexpected error: {message}"
    );
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
fn required_option_distinguishes_omission_from_explicit_null_on_normal_and_fallback_paths() {
    let expected = RequiredOptional { optional: None };
    assert_eq!(
        json::from_slice::<RequiredOptional>(br#"{"optional":null}"#)
            .expect("explicit null on normal path"),
        expected
    );
    assert_missing_field(
        json::from_slice::<RequiredOptional>(br#"{}"#).expect_err("omission must reject"),
        "optional",
    );
    assert_eq!(
        json::from_json_fast::<RequiredOptional>(r#"{"optional":null}"#)
            .expect("explicit null on fallback fast path"),
        expected
    );
    assert_missing_field(
        json::from_json_fast::<RequiredOptional>(r#"{}"#)
            .expect_err("fallback omission must reject"),
        "optional",
    );
}
#[test]
fn required_option_is_enforced_by_direct_fast_and_flatten_paths() {
    assert_eq!(
        decode_fast::<FastRequiredOptional>(r#"{"optional":null}"#)
            .expect("explicit null on direct fast path"),
        FastRequiredOptional { optional: None }
    );
    assert_missing_field(
        decode_fast::<FastRequiredOptional>(r#"{}"#).expect_err("fast omission must reject"),
        "optional",
    );
    let normal = r#"{"optional":null,"label":"known"}"#;
    assert_eq!(
        json::from_str::<RequiredWithFlatten>(normal).expect("normal flatten path"),
        RequiredWithFlatten {
            optional: None,
            fields: FlattenedFields {
                label: "known".to_owned(),
            },
        }
    );
    assert_missing_field(
        json::from_str::<RequiredWithFlatten>(r#"{"label":"known"}"#)
            .expect_err("normal flatten omission must reject"),
        "optional",
    );
    assert_eq!(
        decode_fast::<FastRequiredWithFlatten>(normal).expect("fast flatten path"),
        FastRequiredWithFlatten {
            optional: None,
            fields: FlattenedFields {
                label: "known".to_owned(),
            },
        }
    );
    assert_missing_field(
        decode_fast::<FastRequiredWithFlatten>(r#"{"label":"known"}"#)
            .expect_err("fast flatten omission must reject"),
        "optional",
    );
}
#[test]
fn required_option_is_enforced_in_tagged_enum_named_fields() {
    let explicit = r#"{"kind":"Record","payload":{"optional":null}}"#;
    assert_eq!(
        json::from_str::<RequiredEvent>(explicit).expect("normal enum explicit null"),
        RequiredEvent::Record { optional: None }
    );
    assert_missing_field(
        json::from_str::<RequiredEvent>(r#"{"kind":"Record","payload":{}}"#)
            .expect_err("normal enum omission must reject"),
        "optional",
    );
    assert_eq!(
        decode_fast::<FastRequiredEvent>(explicit).expect("fast enum explicit null"),
        FastRequiredEvent::Record { optional: None }
    );
    assert_missing_field(
        decode_fast::<FastRequiredEvent>(r#"{"kind":"Record","payload":{}}"#)
            .expect_err("fast enum omission must reject"),
        "optional",
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
