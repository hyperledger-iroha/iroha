//! Reading rows from canonical text answers exactly like decoding every row first.
use super::super::specs::{ACCOUNT_PERMISSION_FIELDS, ACCOUNT_PERMISSIONS};
use super::*;
use iroha_data_model::permission::Permission;
use iroha_primitives::json::Json;
use iroha_torii_shared::list_query::SortKey;

const LIMITS: Limits = Limits {
    default_limit: 2,
    max_limit: 1_000,
    max_scanned_rows: 100_000,
    ordered_page_scan_budget: 1_000,
    max_groups: 100,
    bytes: BytePolicy {
        source_frame_bytes: 1 << 24,
        row_bytes: 1 << 24,
        retained_bytes: 1 << 24,
        scratch_bytes: 1 << 24,
        response_bytes: 1 << 24,
    },
};

const UNBOUNDED: norito::DecodeLimits = norito::DecodeLimits::new(
    usize::MAX,
    usize::MAX,
    usize::MAX,
    usize::MAX,
    norito::core::MAX_VALUE_NESTING_DEPTH,
);

/// Every kind of payload whose order or comparison differs from its spelling: numbers
/// out of text order, equal numbers spelled differently, decimal strings, escapes and
/// containers that a scalar literal can match by element.
fn payloads() -> Vec<Value> {
    vec![
        Value::Null,
        Value::from(true),
        Value::from(false),
        Value::from(0_u64),
        Value::from(1_u64),
        Value::from(9_u64),
        Value::from(10_u64),
        Value::from(-1_i64),
        Value::from(1.5_f64),
        Value::from(1.0_f64),
        Value::from(1e30_f64),
        Value::from(u128::from(u64::MAX) + 1),
        Value::from(""),
        Value::from("abc"),
        Value::from("11"),
        Value::from("9"),
        Value::from("1.50"),
        Value::from("quoted \" text"),
        Value::from("é"),
        Value::from("\u{1}"),
        norito::json!({}),
        norito::json!({"a": 1}),
        norito::json!({"a": 1.0}),
        norito::json!({"a": "x"}),
        norito::json!({"a": null}),
        norito::json!({"b": {"c": [1, 2]}, "a": null}),
        norito::json!({"quoted\"key": "é"}),
        norito::json!({"account": "sorau000"}),
        norito::json!([]),
        norito::json!([1, 2]),
        norito::json!(["abc"]),
        norito::json!(["abc", "11"]),
        norito::json!([[1]]),
        norito::json!([null]),
        norito::json!([{"a": 1}]),
        norito::json!([true]),
    ]
}

/// A source list in its stored order, as the effective permission set iterates it.
fn permissions() -> BTreeSet<Permission> {
    let mut rows = BTreeSet::new();
    for name in ["Alpha", "Beta"] {
        for payload in payloads() {
            rows.insert(Permission::new(name.to_owned(), Json::new(payload)));
        }
    }
    rows.insert(Permission::new("Gamma".to_owned(), Json::new(())));
    rows
}

fn canonical_row(permission: &Permission) -> CanonicalRow<'_> {
    CanonicalRow {
        text: permission.name(),
        json: permission.payload().get(),
    }
}

/// The row a source produced before canonical execution existed.
fn decoded_row(permission: &Permission) -> Map {
    Map::from_iter([
        ("name".to_owned(), Value::from(permission.name().to_owned())),
        (
            "payload".to_owned(),
            norito::json::from_str::<Value>(permission.payload().get()).unwrap(),
        ),
    ])
}

/// `Value` equality treats `1` and `1.0` as equal, so pages compare by their spelling.
fn spelled(page: Result<RowPage, CollectionError>) -> Result<(String, RowPage), CollectionError> {
    page.map(|page| {
        let items = page.items.iter().cloned().map(Value::Object).collect();
        let text = norito::json::to_json(&Value::Array(items)).unwrap();
        (text, page)
    })
}

/// Walk `query` to its end through both executions; every page, total and cursor must match.
fn assert_same_pages(query: &ListQuery, rows: &BTreeSet<Permission>, limits: &Limits) {
    let mut current = query.clone();
    for _ in 0..=rows.len() {
        let prepared = prepare(&ACCOUNT_PERMISSIONS, "scope", &current, limits)
            .unwrap_or_else(|error| panic!("{query:?} is not a valid fixture query: {error}"));
        let decoded = prepared
            .execute(rows.iter().map(decoded_row), limits)
            .and_then(|page| prepared.project(page));
        let canonical = prepared
            .execute_canonical(
                ACCOUNT_PERMISSION_FIELDS,
                rows.iter().map(canonical_row),
                limits,
            )
            .and_then(|page| prepared.project(page));
        let decoded = spelled(decoded);
        assert_eq!(spelled(canonical), decoded, "{current:?}");
        let Ok((_, page)) = decoded else { return };
        match page.next_cursor {
            Some(cursor) => current = query.clone().cursor(cursor),
            None => return,
        }
    }
    panic!("{query:?} did not finish");
}

const FILTERS: &[&str] = &[
    "name = \"Beta\"",
    "name != \"Beta\"",
    "name in [\"Alpha\", \"Gamma\"]",
    "name not in [\"Alpha\"]",
    "name > \"Alpha\"",
    "name <= \"Beta\"",
    "exists(name)",
    "name is null",
    "payload = \"abc\"",
    "payload = \"11\"",
    "payload = 11",
    "payload = 1",
    "payload = 1.5",
    "payload = \"1.5\"",
    "payload = true",
    "payload = null",
    "payload != \"abc\"",
    "payload != 1",
    "payload != null",
    "payload in [\"abc\", \"11\"]",
    "payload not in [\"abc\"]",
    "payload in [1, 9]",
    "payload not in [1, 9]",
    "payload > 5",
    "payload >= 1",
    "payload < 1",
    "payload <= 9",
    "payload is null",
    "payload is not null",
    "exists(payload)",
    "not exists(payload)",
    "name = \"Beta\" and payload = 1",
    "name = \"Beta\" or payload = \"abc\"",
    "not (name = \"Beta\" and payload != 1)",
    "not (payload = 1)",
    "payload = \"abc\" or payload = 1",
    "name = \"Beta\" and (payload is null or payload = \"abc\")",
    "name = \"Beta\" or name = \"Gamma\"",
    "name = \"Missing\" and payload = 1",
    "name = \"Beta\" or payload = \"never\"",
];

#[test]
fn every_query_answers_like_decoding_every_row() {
    let rows = permissions();
    let sorted = |query: ListQuery, order: usize| match order {
        0 => query,
        1 => query.sort_by(SortKey::asc("name")),
        _ => query.sort_by(SortKey::desc("name")),
    };
    for text in std::iter::once(None).chain(FILTERS.iter().map(Some)) {
        for order in 0..3 {
            let base = match text {
                Some(text) => ListQuery::new().filter(FilterExpr::parse(text).unwrap()),
                None => ListQuery::new(),
            };
            let base = sorted(base, order);
            assert_same_pages(&base.clone().limit(1), &rows, &LIMITS);
            assert_same_pages(&base.clone().limit(3).include_total(), &rows, &LIMITS);
            assert_same_pages(
                &base.clone().limit(1_000).include_total().select(["name"]),
                &rows,
                &LIMITS,
            );
            assert_same_pages(&base.limit(2).select(["payload"]), &rows, &LIMITS);
        }
    }
}

#[test]
fn aggregates_answer_like_decoding_every_row() {
    let rows = permissions();
    let by_name = AggregateSpec {
        group_by: vec!["name".into()],
        metrics: vec![
            AggregateMetric {
                alias: "rows".into(),
                r#fn: AggregateFn::Count,
                field: None,
            },
            AggregateMetric {
                alias: "payload_total".into(),
                r#fn: AggregateFn::Sum,
                field: Some("payload".into()),
            },
        ],
        having: None,
    };
    let query = ListQuery::new().aggregate(by_name).limit(1);
    assert_same_pages(&query, &rows, &LIMITS);
    let filtered = query.filter(FilterExpr::parse("payload is not null").unwrap());
    assert_same_pages(&filtered, &rows, &LIMITS);
}

#[test]
fn forged_cursor_positions_answer_like_decoding_every_row() {
    let rows = permissions();
    let query = ListQuery::new().limit(4);
    let digest = prepare(&ACCOUNT_PERMISSIONS, "scope", &query, &LIMITS)
        .unwrap()
        .digest;
    let positions = [
        [Value::Null, Value::Null],
        [Value::from(5_u64), Value::from(5_u64)],
        [Value::from("Beta"), Value::Null],
        [Value::from("Beta"), Value::from(5_u64)],
        [Value::from("Beta"), Value::from("abc")],
        [Value::from("Beta"), Value::from("5")],
        [Value::from("Beta"), norito::json!({"a": 1})],
        [Value::from("Beta"), norito::json!({"b": 1, "a": 2})],
        [Value::from("Beta"), norito::json!([1, 2])],
        [Value::from("Missing"), Value::Null],
        [norito::json!({"a": 1}), Value::Null],
    ];
    for position in positions {
        let cursor = cursor::encode(ACCOUNT_PERMISSIONS.tag, &digest, &position, 1 << 16).unwrap();
        assert_same_pages(&query.clone().cursor(cursor), &rows, &LIMITS);
    }
}

#[test]
fn the_scan_limit_answers_like_decoding_every_row() {
    let rows = permissions();
    let mut limits = LIMITS;
    limits.max_scanned_rows = 3;
    assert_same_pages(&ListQuery::new(), &rows, &limits);
    limits.max_scanned_rows = rows.len();
    assert_same_pages(&ListQuery::new().limit(1_000), &rows, &limits);
}

#[test]
fn container_keys_read_from_text_equal_keys_from_decoded_values() {
    for payload in payloads() {
        let json = Json::new(payload.clone());
        let from_text =
            SortValue::from_canonical_json(json.get(), FieldType::Json, LIMITS.bytes).unwrap();
        let from_value = SortValue::from_value(&payload, FieldType::Json, LIMITS.bytes).unwrap();
        assert_eq!(from_text, from_value, "{}", json.get());
    }
}

#[test]
fn object_decisions_never_disagree_with_the_decoded_row_and_defer_structured_literals() {
    let schema = Schema::Collection(&ACCOUNT_PERMISSIONS);
    let row = Map::from_iter([
        ("name".to_owned(), Value::from("Alpha")),
        ("payload".to_owned(), norito::json!({"a": 1})),
    ]);
    let leaves = std::cell::Cell::new(0);
    for text in FILTERS {
        let compiled = compile_filter("filter", &schema, &FilterExpr::parse(text).unwrap());
        let decided = compiled.unwrap().decide(&|leaf, path| {
            if path.full == "payload" {
                leaves.set(leaves.get() + 1);
                assert_eq!(leaf.matches_object(), Some(leaf.matches(&row)), "{text}");
            }
            None
        });
        assert_eq!(decided, None);
    }
    assert!(
        leaves.get() >= 20,
        "the corpus exercises the payload comparisons"
    );
    let path = FieldPath::new("payload");
    for literal in [norito::json!({"a": 1}), norito::json!([1])] {
        let structured = Literal::Json(literal, None);
        assert_eq!(
            Compiled::Eq(Path::new(&path), FieldType::Json, structured.clone()).matches_object(),
            None
        );
        assert_eq!(
            Compiled::Nin(
                Path::new(&path),
                FieldType::Json,
                vec![Literal::Null, structured]
            )
            .matches_object(),
            None
        );
    }
}

#[test]
fn structured_literals_on_the_payload_field_are_refused_before_execution() {
    for literal in [norito::json!({"a": 1}), norito::json!([1])] {
        let query = ListQuery::new().filter(FilterExpr::Eq(FieldPath::new("payload"), literal));
        assert!(prepare(&ACCOUNT_PERMISSIONS, "scope", &query, &LIMITS).is_err());
    }
}

fn read_grants(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| {
            let payload = norito::json!({ "account": (format!("sorau{index:060}")) });
            Json::new(payload).get().clone()
        })
        .collect()
}

#[test]
fn only_the_rows_of_the_page_spend_the_decode_budget() {
    let grants = read_grants(5_000);
    let (one, usage) = norito::core::with_decode_limits_measured(UNBOUNDED, || {
        decode_canonical_json(&grants[0]).unwrap()
    });
    let one_row = usage.total_allocated_bytes();
    drop(one);
    assert!(one_row > 0, "decoding a payload charges the active budget");
    let spent = |query: &ListQuery, count: usize| {
        let prepared = prepare(&ACCOUNT_PERMISSIONS, "scope", query, &LIMITS).unwrap();
        let rows = grants[..count].iter().map(|json| CanonicalRow {
            text: "CanReadAccountData",
            json,
        });
        let (page, usage) = norito::core::with_decode_limits_measured(UNBOUNDED, || {
            prepared.execute_canonical(ACCOUNT_PERMISSION_FIELDS, rows, &LIMITS)
        });
        assert_eq!(page.unwrap().items.len(), 10);
        usage.total_allocated_bytes()
    };
    for query in [
        ListQuery::new().limit(10),
        ListQuery::new().limit(10).include_total(),
        ListQuery::new().limit(10).sort_by(SortKey::desc("name")),
        ListQuery::new()
            .limit(10)
            .filter(FilterExpr::parse("name = \"CanReadAccountData\"").unwrap()),
        ListQuery::new()
            .limit(10)
            .filter(FilterExpr::parse("payload is not null and payload != \"x\"").unwrap()),
    ] {
        let short = spent(&query, 50);
        let long = spent(&query, 5_000);
        assert_eq!(short, long, "{query:?}");
        assert!(
            long <= 10 * one_row + 3 * CURSOR_MAX_BYTES,
            "{query:?} spent {long} bytes for ten rows of {one_row}"
        );
    }
}

#[test]
fn rows_the_filter_rejects_by_name_are_never_decoded() {
    // Text that would not parse proves the decoder was not reached.
    let rows = [
        CanonicalRow {
            text: "Rejected",
            json: "not json",
        },
        CanonicalRow {
            text: "Selected",
            json: "{}",
        },
    ];
    let query = ListQuery::new().filter(FilterExpr::parse("name = \"Selected\"").unwrap());
    let prepared = prepare(&ACCOUNT_PERMISSIONS, "scope", &query, &LIMITS).unwrap();
    let page = prepared
        .execute_canonical(ACCOUNT_PERMISSION_FIELDS, rows, &LIMITS)
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0]["name"].as_str(), Some("Selected"));
}

#[test]
fn a_refused_decode_is_an_owner_capacity_refusal() {
    let grants = read_grants(3);
    let rows = grants.iter().map(|json| CanonicalRow {
        text: "CanReadAccountData",
        json,
    });
    let query = ListQuery::new().limit(2);
    let prepared = prepare(&ACCOUNT_PERMISSIONS, "scope", &query, &LIMITS).unwrap();
    let tight = norito::DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        16,
        norito::core::MAX_VALUE_NESTING_DEPTH,
    );
    let refused = norito::core::with_decode_limits_scope(tight, || {
        prepared.execute_canonical(ACCOUNT_PERMISSION_FIELDS, rows, &LIMITS)
    })
    .unwrap_err();
    assert_eq!(refused.code, "query_capacity_exceeded");
}

#[test]
fn a_document_that_does_not_parse_is_not_a_capacity_refusal() {
    // Ledger Json cannot hold these; the refusal must still not send the read up the ladder.
    for json in ["{", "nul"] {
        let rows = [CanonicalRow { text: "Bad", json }];
        let query = ListQuery::new().limit(2);
        let prepared = prepare(&ACCOUNT_PERMISSIONS, "scope", &query, &LIMITS).unwrap();
        let refused = prepared
            .execute_canonical(ACCOUNT_PERMISSION_FIELDS, rows, &LIMITS)
            .unwrap_err();
        assert_eq!(refused.code, "internal", "{json}");
    }
}

#[test]
fn a_decoded_row_keeps_the_current_row_and_retained_ceilings() {
    let grants = read_grants(2);
    let rows = || {
        grants.iter().map(|json| CanonicalRow {
            text: "CanReadAccountData",
            json,
        })
    };
    let query = ListQuery::new().limit(2);
    let code = |limits: &Limits| {
        let prepared = prepare(&ACCOUNT_PERMISSIONS, "scope", &query, limits).unwrap();
        prepared
            .execute_canonical(ACCOUNT_PERMISSION_FIELDS, rows(), limits)
            .map(|page| page.items.len())
            .map_err(|error| error.code)
    };
    assert_eq!(code(&LIMITS), Ok(2));
    let mut tight = LIMITS;
    tight.bytes.row_bytes = 64;
    assert_eq!(code(&tight), Err("query_capacity_exceeded"));
    let mut tight = LIMITS;
    tight.bytes.retained_bytes = 1_024;
    assert_eq!(code(&tight), Err("query_capacity_exceeded"));
}
