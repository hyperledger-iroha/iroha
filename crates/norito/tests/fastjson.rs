//! Minimal FastJson/FastJsonWrite derive demo over TapeWalker.
#![cfg(feature = "json")]

use norito::json::{FastFromJson, FastJsonWrite, JsonDeserialize as _, TapeWalker};

#[derive(Debug, Clone, PartialEq, Eq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Demo {
    id: u64,
    name: String,
    tags: Vec<String>,
    opt: Option<u64>,
}

#[derive(Debug, norito::derive::FastJson)]
struct RecursiveKnownField {
    children: Vec<RecursiveKnownField>,
}

#[derive(Debug, PartialEq, Eq, norito::derive::JsonDeserialize)]
struct GenericDepthProbe {
    id: u64,
}

#[derive(Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(no_fast_from_json)]
struct FlattenedDepthFields {
    value: u64,
}

#[derive(Debug, PartialEq, Eq, norito::derive::FastJson)]
struct FlattenedDepthProbe {
    #[norito(flatten)]
    fields: FlattenedDepthFields,
}

#[derive(Debug, PartialEq, Eq, norito::derive::FastJson)]
#[norito(tag = "kind", content = "payload", rename_all = "snake_case")]
enum EnumDepthProbe {
    Unit,
}

fn assert_json_depth_exceeded<T>(result: Result<T, norito::json::Error>) {
    assert!(matches!(
        result,
        Err(norito::json::Error::NestingDepthExceeded {
            depth,
            limit: norito::json::MAX_JSON_VALUE_NESTING_DEPTH,
            context: "JSON value",
        }) if depth == norito::json::MAX_JSON_VALUE_NESTING_DEPTH + 1
    ));
}

fn assert_fast_depth_exceeded<T>(result: Result<T, norito::Error>) {
    assert!(matches!(
        result,
        Err(norito::Error::Json(
            norito::json::Error::NestingDepthExceeded {
                depth,
                limit: norito::json::MAX_JSON_VALUE_NESTING_DEPTH,
                context: "JSON value",
            }
        )) if depth == norito::json::MAX_JSON_VALUE_NESTING_DEPTH + 1
    ));
}

#[test]
fn fastjson_roundtrip() {
    let d = Demo {
        id: 7,
        name: "alice".to_string(),
        tags: vec!["a".into(), "b".into()],
        opt: Some(9),
    };
    let mut out = String::new();
    d.write_json(&mut out);
    let mut w = TapeWalker::new(&out);
    let mut arena = norito::json::Arena::new();
    let got = <Demo as FastFromJson>::parse(&mut w, &mut arena).expect("parse");
    assert_eq!(d, got);
}

#[test]
fn fastjson_unknown_fields_use_strict_bounded_skip() {
    let globally_at_limit = format!(
        "{}null{}",
        "[".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 2),
        "]".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 2)
    );
    let input =
        format!(r#"{{"id":7,"name":"alice","tags":[],"opt":null,"unknown":{globally_at_limit}}}"#);
    let mut walker = TapeWalker::new(&input);
    let mut arena = norito::json::Arena::new();
    <Demo as FastFromJson>::parse(&mut walker, &mut arena)
        .expect("unknown field at the complete-document depth limit must pass");

    let locally_at_limit = format!(
        "{}null{}",
        "[".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 1),
        "]".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 1)
    );
    let input =
        format!(r#"{{"id":7,"name":"alice","tags":[],"opt":null,"unknown":{locally_at_limit}}}"#);
    let mut walker = TapeWalker::new(&input);
    let mut arena = norito::json::Arena::new();
    assert!(matches!(
        <Demo as FastFromJson>::parse(&mut walker, &mut arena),
        Err(norito::Error::Json(
            norito::json::Error::NestingDepthExceeded {
                depth,
                limit: norito::json::MAX_JSON_VALUE_NESTING_DEPTH,
                context: "JSON value",
            }
        )) if depth == norito::json::MAX_JSON_VALUE_NESTING_DEPTH + 1
    ));

    let over_limit = format!(
        "{}null{}",
        "[".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH),
        "]".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH)
    );
    let input = format!(r#"{{"id":7,"name":"alice","tags":[],"opt":null,"unknown":{over_limit}}}"#);
    let mut walker = TapeWalker::new(&input);
    let mut arena = norito::json::Arena::new();
    assert!(matches!(
        <Demo as FastFromJson>::parse(&mut walker, &mut arena),
        Err(norito::Error::Json(
            norito::json::Error::NestingDepthExceeded { .. }
        ))
    ));

    let malformed = r#"{"id":7,"name":"alice","tags":[],"opt":null,"unknown":[}}"#;
    let mut walker = TapeWalker::new(malformed);
    let mut arena = norito::json::Arena::new();
    assert!(<Demo as FastFromJson>::parse(&mut walker, &mut arena).is_err());
}

#[test]
fn fastjson_recursive_known_fields_obey_the_global_document_depth_limit() {
    let boundary_levels = (norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 3) / 2;
    let input = format!(
        "{}{{\"children\":[],\"unknown\":[null]}}{}",
        "{\"children\":[".repeat(boundary_levels),
        "]}".repeat(boundary_levels)
    );
    let mut walker = TapeWalker::new(&input);
    let mut arena = norito::json::Arena::new();
    let parsed = <RecursiveKnownField as FastFromJson>::parse(&mut walker, &mut arena)
        .expect("recursive known fields at the exact document-depth limit must pass");
    let mut node = &parsed;
    let mut parsed_levels = 1_usize;
    while let Some(child) = node.children.first() {
        assert_eq!(node.children.len(), 1, "fixture is a single-child chain");
        node = child;
        parsed_levels += 1;
    }
    assert_eq!(parsed_levels, boundary_levels + 1);

    let over_limit_levels = norito::json::MAX_JSON_VALUE_NESTING_DEPTH / 2;
    let input = format!(
        "{}{{\"children\":[]}}{}",
        "{\"children\":[".repeat(over_limit_levels),
        "]}".repeat(over_limit_levels)
    );
    let mut walker = TapeWalker::new(&input);
    let mut arena = norito::json::Arena::new();
    assert!(matches!(
        <RecursiveKnownField as FastFromJson>::parse(&mut walker, &mut arena),
        Err(norito::Error::Json(
            norito::json::Error::NestingDepthExceeded {
                depth,
                limit: norito::json::MAX_JSON_VALUE_NESTING_DEPTH,
                context: "JSON value",
            }
        )) if depth == norito::json::MAX_JSON_VALUE_NESTING_DEPTH + 1
    ));
}

#[test]
fn generic_typed_json_obeys_the_global_document_depth_limit() {
    let at_limit = format!(
        "{}null{}",
        "[".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 2),
        "]".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 2)
    );
    let input = format!(r#"{{"id":7,"unknown":{at_limit}}}"#);
    assert_eq!(
        norito::json::from_json::<GenericDepthProbe>(&input)
            .expect("generic typed JSON at the complete-document limit must pass")
            .id,
        7
    );

    let over_limit = format!(
        "{}null{}",
        "[".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 1),
        "]".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 1)
    );
    let input = format!(r#"{{"id":7,"unknown":{over_limit}}}"#);
    assert_json_depth_exceeded(norito::json::from_json::<GenericDepthProbe>(&input));
    assert_json_depth_exceeded(norito::json::from_str::<GenericDepthProbe>(&input));
    assert_json_depth_exceeded(norito::json::from_slice::<GenericDepthProbe>(
        input.as_bytes(),
    ));
}

#[test]
fn alternate_fastjson_emitters_obey_the_enclosing_document_depth_limit() {
    let mut walker = TapeWalker::new(r#"{"value":9}"#);
    let mut arena = norito::json::Arena::new();
    let flattened = <FlattenedDepthProbe as FastFromJson>::parse(&mut walker, &mut arena)
        .expect("flattened FastFromJson emitter must decode its ordinary shape");
    assert_eq!(flattened.fields.value, 9);

    let mut walker = TapeWalker::new(r#"{"kind":"unit","payload":null}"#);
    let mut arena = norito::json::Arena::new();
    assert_eq!(
        <EnumDepthProbe as FastFromJson>::parse(&mut walker, &mut arena)
            .expect("enum FastFromJson emitter must decode its ordinary shape"),
        EnumDepthProbe::Unit
    );

    let over_limit = format!(
        "{}null{}",
        "[".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 1),
        "]".repeat(norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 1)
    );

    let input = format!(r#"{{"target":{{"id":7}},"unknown":{over_limit}}}"#);
    let target = input.find(r#"{"id":7}"#).expect("fallback target");
    let mut walker = TapeWalker::new(&input);
    walker.sync_to_raw(target);
    let mut arena = norito::json::Arena::new();
    assert_fast_depth_exceeded(<GenericDepthProbe as FastFromJson>::parse(
        &mut walker,
        &mut arena,
    ));

    let input = format!(r#"{{"target":{{"value":9}},"unknown":{over_limit}}}"#);
    let target = input.find(r#"{"value":9}"#).expect("flatten target");
    let mut walker = TapeWalker::new(&input);
    walker.sync_to_raw(target);
    let mut arena = norito::json::Arena::new();
    assert_fast_depth_exceeded(<FlattenedDepthProbe as FastFromJson>::parse(
        &mut walker,
        &mut arena,
    ));

    let input = format!(r#"{{"target":{{"kind":"unit","payload":null}},"unknown":{over_limit}}}"#);
    let target = input
        .find(r#"{"kind":"unit","payload":null}"#)
        .expect("enum target");
    let mut walker = TapeWalker::new(&input);
    walker.sync_to_raw(target);
    let mut arena = norito::json::Arena::new();
    assert_fast_depth_exceeded(<EnumDepthProbe as FastFromJson>::parse(
        &mut walker,
        &mut arena,
    ));
}
