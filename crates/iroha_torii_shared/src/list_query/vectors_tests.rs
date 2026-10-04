//! Golden vectors shared with every SDK (`fixtures/torii/list_query/vectors.json`).
//!
//! The cases below are the source of truth. The test fails when the checked-in
//! fixture differs from what this implementation produces; set
//! `IROHA_REGENERATE_LIST_QUERY_VECTORS=1` to rewrite the fixture.
use super::*;
use norito::json::{Map, Value};
use std::path::PathBuf;

const FILTERS: &[&str] = &[
    r#"owned_by = "alice""#,
    "owned_by = 'alice'   AND quantity>=10.5",
    "a == 1",
    "a <> -2",
    "a < 340282366920938463463374607431768211455",
    "a <= 0 or b > 1.25",
    "flag = true and other = false and gone = null",
    r#"status in ["active", "paused",]"#,
    "tier NOT IN (1, 2, 3)",
    "exists(metadata.tier)",
    "note is null",
    "note IS NOT NULL",
    "not exists(metadata.archived)",
    "a = 1 or not b = 2 and c = 3",
    "(a = 1 or b = 2) and c = 3",
    "not (a = 1 and b = 2)",
    "not not a = 1",
    "metadata.`display-name` = \"x\"",
    "`and` = 1",
    "metadata.null = 1",
    r#"text = "quote \" backslash \\ newline \n unicode é""#,
    "tx_hash = \"hash\" and tx_status in [\"Approved\", \"Rejected\"]",
];

const FILTER_ERRORS: &[&str] = &[
    "",
    "owned_by == \"x\" && quantity > 1",
    "a = 1 || b = 2",
    "!a = 1",
    "quantity >",
    "status = active",
    "5 < quantity",
    "a in [1, 2",
    "a = 1 b = 2",
    "display-name = 1",
    "a = 1e5",
    "a = 007",
    "a = .5",
    "a = \"open",
    "status:active",
    "a in []",
    "a in [1, 1]",
    "a is 1",
    "and = 1",
    "a <= null",
    "a = 1\nand b ~ 2",
];

const SORTS: &[&str] = &[
    "-quantity, id ,metadata.`ui-order`",
    "id",
    "-alias_binding.bound_at_ms",
];

const SORT_ERRORS: &[&str] = &["id:desc", "id desc", "id,-id", "", "id,"];

fn queries() -> Vec<ListQuery> {
    vec![
        ListQuery::new(),
        ListQuery::new()
            .filter(field("owned_by").eq("alice") & field("quantity").gt(1))
            .sort_by(SortKey::desc("quantity"))
            .sort_by(SortKey::asc("id"))
            .select(["id", "quantity"])
            .limit(25)
            .include_total(),
        ListQuery::new().limit(10).cursor("q1_abc-DEF"),
    ]
}

const QUERY_BODY_ERRORS: &[&str] = &[
    r#"{"pagination": {"limit": 1}}"#,
    r#"{"filter": "a ="}"#,
    r#"{"filter": {"op": "eq"}}"#,
    r#"{"sort": "id"}"#,
    r#"{"sort": ["id:desc"]}"#,
    r#"{"select": []}"#,
    r#"{"select": ["id"], "aggregate": {"metrics": [{"alias": "n", "fn": "count"}]}}"#,
    r#"{"limit": 0}"#,
    r#"{"cursor": "has space"}"#,
    r#"{"include_total": "yes"}"#,
    "[]",
];

const QUERY_PAIR_ERRORS: &[&[(&str, &str)]] = &[
    &[("offset", "10")],
    &[("limit", "ten")],
    &[("limit", "1"), ("limit", "2")],
    &[("sort", "id:asc")],
    &[("include_total", "1")],
    &[("select", "id,,name")],
];

fn object(pairs: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}

fn syntax_error_value(text: &str, err: &FilterSyntaxError) -> Value {
    object(vec![
        ("text", Value::from(text)),
        ("column", Value::from(err.column as u64)),
        ("line", Value::from(err.line as u64)),
        ("message", Value::from(err.message.clone())),
    ])
}

fn pairs_value(pairs: &[(&str, String)]) -> Value {
    Value::Array(
        pairs
            .iter()
            .map(|(key, value)| Value::Array(vec![Value::from(*key), Value::from(value.clone())]))
            .collect(),
    )
}

fn generate() -> Value {
    let filters = FILTERS
        .iter()
        .map(|text| {
            let parsed = FilterExpr::parse(text).expect("vector filter parses");
            object(vec![
                ("text", Value::from(*text)),
                ("canonical", Value::from(parsed.to_string())),
                ("json", parsed.to_json_value()),
            ])
        })
        .collect();
    let filter_errors = FILTER_ERRORS
        .iter()
        .map(|text| {
            let err = FilterExpr::parse(text).expect_err("vector filter is rejected");
            syntax_error_value(text, &err)
        })
        .collect();
    let sorts = SORTS
        .iter()
        .map(|text| {
            let keys = parse_sort(text).expect("vector sort parses");
            object(vec![
                ("text", Value::from(*text)),
                ("canonical", Value::from(sort_to_string(&keys))),
                (
                    "json",
                    Value::Array(
                        keys.iter()
                            .map(|key| Value::from(key.to_string()))
                            .collect(),
                    ),
                ),
            ])
        })
        .collect();
    let sort_errors = SORT_ERRORS
        .iter()
        .map(|text| {
            let err = parse_sort(text).expect_err("vector sort is rejected");
            syntax_error_value(text, &err)
        })
        .collect();
    let queries = queries()
        .iter()
        .map(|query| {
            let pairs = query.to_query_pairs().expect("GET form");
            object(vec![
                ("body", query.to_json_value()),
                ("query_pairs", pairs_value(&pairs)),
            ])
        })
        .collect();
    let body_errors = QUERY_BODY_ERRORS
        .iter()
        .map(|body| {
            let value = norito::json::parse_value(body).expect("vector body is JSON");
            let err =
                ListQuery::from_json_value(value.clone()).expect_err("vector body is rejected");
            object(vec![
                ("body", value),
                ("code", Value::from(err.code())),
                ("parameter", Value::from(err.parameter)),
            ])
        })
        .collect();
    let pair_errors = QUERY_PAIR_ERRORS
        .iter()
        .map(|pairs| {
            let err = ListQuery::from_query_pairs(pairs.iter().copied())
                .expect_err("vector pairs are rejected");
            let owned: Vec<(&str, String)> = pairs
                .iter()
                .map(|(key, value)| (*key, (*value).to_owned()))
                .collect();
            object(vec![
                ("query_pairs", pairs_value(&owned)),
                ("code", Value::from(err.code())),
                ("parameter", Value::from(err.parameter)),
            ])
        })
        .collect();
    let pages = [
        Page {
            items: vec![object(vec![("id", Value::from("a"))])],
            next_cursor: Some("q1_next".into()),
            total: Some(3),
        },
        Page::last(Vec::<Value>::new()),
    ]
    .iter()
    .map(|page| {
        object(vec![
            (
                "json",
                norito::json::parse_value(&norito::json::to_json(page).unwrap()).unwrap(),
            ),
            ("has_more", Value::Bool(page.has_more())),
        ])
    })
    .collect();
    object(vec![
        ("version", Value::from(1u64)),
        ("filters", Value::Array(filters)),
        ("filter_errors", Value::Array(filter_errors)),
        ("sorts", Value::Array(sorts)),
        ("sort_errors", Value::Array(sort_errors)),
        ("queries", Value::Array(queries)),
        ("query_body_errors", Value::Array(body_errors)),
        ("query_pair_errors", Value::Array(pair_errors)),
        ("pages", Value::Array(pages)),
    ])
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/torii/list_query/vectors.json")
}

#[test]
fn golden_vectors_match_the_fixture() {
    let generated = generate();
    let mut rendered = norito::json::to_string_pretty(&generated).expect("render vectors");
    rendered.push('\n');
    let path = fixture_path();
    if std::env::var_os("IROHA_REGENERATE_LIST_QUERY_VECTORS").is_some() {
        std::fs::create_dir_all(path.parent().expect("fixture directory")).expect("mkdir");
        std::fs::write(&path, &rendered).expect("write vectors");
        return;
    }
    let checked_in = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "missing {}: {err}; regenerate with IROHA_REGENERATE_LIST_QUERY_VECTORS=1",
            path.display()
        )
    });
    assert!(
        checked_in == rendered,
        "{} is stale; regenerate with IROHA_REGENERATE_LIST_QUERY_VECTORS=1",
        path.display()
    );
}

#[test]
fn vectors_roundtrip_through_both_forms() {
    for text in FILTERS {
        let parsed = FilterExpr::parse(text).unwrap();
        assert_eq!(
            FilterExpr::parse(&parsed.to_string()).unwrap(),
            parsed,
            "{text}"
        );
        assert_eq!(
            FilterExpr::from_json_value(parsed.to_json_value()).unwrap(),
            parsed,
            "{text}"
        );
    }
}
