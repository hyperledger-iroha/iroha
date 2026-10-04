//! Collection query arguments shared by the Torii MCP collection tools.
//!
//! Every Torii collection endpoint reads one query language
//! (`specs/torii/collection_queries.md`, [`ListQuery`]). Collection tools take
//! its controls as flat arguments — `filter`, `sort`, `select`, `limit`,
//! `cursor`, `include_total` and, on `POST …/query` tools, `aggregate` — or as
//! one complete query in `body` on `POST` tools. The arguments are parsed into
//! a [`ListQuery`] before dispatch, so malformed controls fail with the
//! server's own error text; `GET` tools then send its URL parameters and
//! `POST` tools its canonical JSON body. Pages answer
//! `{"items": [...], "next_cursor": ...}`, and callers continue by passing
//! `next_cursor` back as `cursor` until it is `null`.

use super::{
    Map, Method, ToolSpec, Value, encode_mcp_json_body, manual_tool_effect_from_name,
    try_append_form_pair, try_begin_form_query,
};
use iroha_torii_shared::list_query::{
    CURSOR_MAX_BYTES, FilterExpr, LIST_QUERY_MEMBERS, ListQuery, ListQueryError, SELECT_MAX_FIELDS,
    SORT_MAX_KEYS, parse_sort,
};

/// Input-schema extension marking a static descriptor as a collection tool.
///
/// Its value names a [`CollectionQueryShape`]. The descriptor loader replaces
/// the marker with [`collection_query_properties`], so every collection tool
/// advertises the same, single definition of the query controls.
pub(super) const COLLECTION_QUERY_SCHEMA_EXTENSION: &str = "x-iroha-mcp-collection-query";

/// Arguments that address the route or the transport rather than the query.
const TRANSPORT_ARGUMENTS: [&str; 3] = ["path", "headers", "accept"];

/// Arguments of the retired offset-paged query envelope.
const RETIRED_ARGUMENTS: [&str; 5] = ["pagination", "offset", "fetch_size", "count_mode", "query"];

const FILTER_DESCRIPTION: &str = "Rows to keep, as text or as the JSON form. Text reads like a SQL WHERE clause: `owned_by = \"sorau…\" and quantity >= 10`. Operators: `=`, `!=`, `<`, `<=`, `>`, `>=`, `in [..]`, `not in [..]`, `exists(field)`, `is null`, `is not null`, combined with `and`, `or`, `not` and parentheses; strings are quoted, and decimals are exact (`10.5` or \"10.5\"). JSON form: {\"op\": \"and\", \"args\": [{\"op\": \"eq\", \"args\": [\"owned_by\", \"sorau…\"]}, {\"op\": \"gte\", \"args\": [\"quantity\", 10]}]}.";
const SORT_DESCRIPTION: &str = "Sort keys, most significant first; prefix `-` for descending. A comma-separated string such as \"-quantity,id\" or an array such as [\"-quantity\", \"id\"]. The collection's default order applies when omitted.";
const SELECT_DESCRIPTION: &str = "Fields returned per item, as a comma-separated string such as \"id,quantity\" or an array such as [\"id\", \"quantity\"]; nested fields keep their paths (`alias_binding.status`). Full rows are returned when omitted.";
const LIMIT_DESCRIPTION: &str =
    "Rows per page; the collection's documented default and server maximum apply.";
const CURSOR_DESCRIPTION: &str = "The previous page's `next_cursor`, passed unchanged with the same filter, sort and aggregate to read the next page. Stop when `next_cursor` is null.";
const INCLUDE_TOTAL_DESCRIPTION: &str = "Add the exact number of matching rows as `total`. Counting scans every match, so request it only when the count is needed.";
const AGGREGATE_DESCRIPTION: &str = "Grouped metrics instead of rows (POST only; not combinable with `select`). Example: {\"group_by\": [\"asset\"], \"metrics\": [{\"alias\": \"holders\", \"fn\": \"count\"}, {\"alias\": \"supply\", \"fn\": \"sum\", \"field\": \"quantity\"}], \"having\": \"holders >= 10\"}. `sort` may name group fields and metric aliases.";
const BODY_DESCRIPTION: &str = "One complete query sent as the request body: `filter`, `sort` and `select` (arrays), `aggregate`, `limit`, `cursor` and `include_total`. Use it instead of the flat arguments, never together with them.";
const HISTORY_BODY_DESCRIPTION: &str = "One complete query sent as the request body: `filter`, `select` (array), `limit` and `cursor`. Use it instead of the flat arguments, never together with them.";

/// The query controls a collection tool accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CollectionQueryShape {
    /// `GET` collection: the controls travel as URL parameters.
    Get,
    /// `POST …/query` collection: the controls form the JSON body, which also
    /// takes `aggregate`.
    Post,
    /// `GET` transaction history: rows are newest first, so `sort` and
    /// `include_total` are not offered.
    HistoryGet,
    /// `POST …/query` transaction history: no `sort`, `include_total` or
    /// `aggregate`.
    HistoryPost,
}

impl CollectionQueryShape {
    /// Parse a [`COLLECTION_QUERY_SCHEMA_EXTENSION`] value.
    fn from_marker(marker: &str) -> Option<Self> {
        match marker {
            "get" => Some(Self::Get),
            "post" => Some(Self::Post),
            "history_get" => Some(Self::HistoryGet),
            "history_post" => Some(Self::HistoryPost),
            _ => None,
        }
    }

    /// Whether the query is sent as a `POST` body.
    const fn is_post(self) -> bool {
        matches!(self, Self::Post | Self::HistoryPost)
    }

    /// Whether rows come in the fixed history order (newest first).
    const fn is_history(self) -> bool {
        matches!(self, Self::HistoryGet | Self::HistoryPost)
    }
}

/// Query-control properties advertised by collection tools of `shape`.
pub(super) fn collection_query_properties(shape: CollectionQueryShape) -> Map {
    let mut properties = Map::new();
    properties.insert("filter".into(), filter_schema(FILTER_DESCRIPTION));
    if !shape.is_history() {
        properties.insert(
            "sort".into(),
            norito::json!({
                "type": ["string", "array"],
                "items": { "type": "string" },
                "maxItems": SORT_MAX_KEYS,
                "description": SORT_DESCRIPTION
            }),
        );
    }
    properties.insert(
        "select".into(),
        norito::json!({
            "type": ["string", "array"],
            "items": { "type": "string" },
            "minItems": 1,
            "maxItems": SELECT_MAX_FIELDS,
            "description": SELECT_DESCRIPTION
        }),
    );
    if shape == CollectionQueryShape::Post {
        properties.insert("aggregate".into(), aggregate_schema());
    }
    properties.insert(
        "limit".into(),
        norito::json!({
            "type": "integer",
            "minimum": 1,
            "description": LIMIT_DESCRIPTION
        }),
    );
    properties.insert(
        "cursor".into(),
        norito::json!({
            "type": "string",
            "minLength": 1,
            "maxLength": CURSOR_MAX_BYTES,
            "pattern": "^[A-Za-z0-9_-]+$",
            "description": CURSOR_DESCRIPTION
        }),
    );
    if !shape.is_history() {
        properties.insert(
            "include_total".into(),
            norito::json!({
                "type": "boolean",
                "description": INCLUDE_TOTAL_DESCRIPTION
            }),
        );
    }
    if shape.is_post() {
        let description = if shape.is_history() {
            HISTORY_BODY_DESCRIPTION
        } else {
            BODY_DESCRIPTION
        };
        properties.insert(
            "body".into(),
            norito::json!({
                "type": "object",
                "description": description
            }),
        );
    }
    properties
}

/// Schema of a filter: text in the filter grammar, or the root node of the
/// JSON form, which has exactly the members `op` and `args`.
fn filter_schema(description: &str) -> Value {
    norito::json!({
        "type": ["string", "object"],
        "required": ["op", "args"],
        "properties": {
            "op": {
                "type": "string",
                "enum": [
                    "and", "or", "not", "eq", "ne", "lt", "lte", "gt", "gte", "in", "nin",
                    "exists", "is_null"
                ]
            },
            "args": { "type": "array" }
        },
        "description": description
    })
}

/// Schema of the `aggregate` control.
fn aggregate_schema() -> Value {
    norito::json!({
        "type": "object",
        "required": ["metrics"],
        "properties": {
            "group_by": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Fields whose values form the groups."
            },
            "metrics": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["alias", "fn"],
                    "properties": {
                        "alias": { "type": "string", "minLength": 1 },
                        "fn": {
                            "type": "string",
                            "enum": ["count", "sum", "min", "max", "avg", "distinct_count"]
                        },
                        "field": { "type": "string" }
                    }
                },
                "description": "Metrics per group: `count` takes no field; `sum`, `min`, `max` and `avg` take a numeric field; `distinct_count` takes a scalar field."
            },
            "having": (filter_schema(
                "Filter over group fields and metric aliases, in the `filter` syntax."
            ))
        },
        "description": AGGREGATE_DESCRIPTION
    })
}

/// Purpose-built collection tool without path parameters whose input schema
/// is [`collection_query_input_schema`] for `shape`.
pub(super) fn collection_query_tool(
    name: &str,
    description: &str,
    path_template: &str,
    shape: CollectionQueryShape,
) -> ToolSpec {
    let method = if shape.is_post() {
        Method::POST
    } else {
        Method::GET
    };
    ToolSpec::route(
        name.to_owned(),
        description.to_owned(),
        manual_tool_effect_from_name(name),
        method,
        path_template.to_owned(),
        collection_query_input_schema(shape),
    )
}

/// Complete input schema of a collection tool without path parameters.
pub(super) fn collection_query_input_schema(shape: CollectionQueryShape) -> Value {
    let mut properties = collection_query_properties(shape);
    properties.insert(
        "headers".into(),
        norito::json!({
            "type": "object",
            "additionalProperties": { "type": "string" }
        }),
    );
    properties.insert("accept".into(), norito::json!({ "type": "string" }));
    let mut schema = Map::new();
    schema.insert("type".into(), Value::from("object"));
    schema.insert("additionalProperties".into(), Value::Bool(false));
    schema.insert("properties".into(), Value::Object(properties));
    Value::Object(schema)
}

/// Replace a static descriptor's [`COLLECTION_QUERY_SCHEMA_EXTENSION`] marker
/// with the shared query-control properties. Schemas without the marker are
/// left unchanged.
///
/// # Errors
/// The marker names no [`CollectionQueryShape`], the schema has no
/// `properties` object, or it already declares one of the query controls.
pub(super) fn expand_collection_query_schema(schema: &mut Value) -> Result<(), String> {
    let Some(object) = schema.as_object_mut() else {
        return Ok(());
    };
    let Some(marker) = object.remove(COLLECTION_QUERY_SCHEMA_EXTENSION) else {
        return Ok(());
    };
    let shape = marker
        .as_str()
        .and_then(CollectionQueryShape::from_marker)
        .ok_or_else(|| {
            format!(
                "`{COLLECTION_QUERY_SCHEMA_EXTENSION}` must be one of get, post, history_get, history_post"
            )
        })?;
    let properties = object
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "collection tool schema lacks a `properties` object".to_owned())?;
    for (name, property) in collection_query_properties(shape) {
        if properties.contains_key(&name) {
            return Err(format!(
                "collection tool schema must not declare the shared query control `{name}`"
            ));
        }
        properties.insert(name, property);
    }
    Ok(())
}

/// Parse the collection query carried by tool `arguments`.
///
/// The query is either one complete object in `body` or the flat controls of
/// [`LIST_QUERY_MEMBERS`]. Flat `sort` and `select` may also use the
/// comma-separated `GET` spellings (`"-quantity,id"`, `"id,quantity"`).
/// `path`, `headers` and `accept` address the route and the transport and are
/// skipped.
///
/// # Errors
/// Names the offending argument: an unknown or retired argument, flat
/// controls next to `body`, a non-object `body`, or the [`ListQueryError`] of
/// an invalid control.
pub(super) fn list_query_from_tool_arguments(arguments: &Map) -> Result<ListQuery, String> {
    let mut controls = Map::new();
    for (name, value) in arguments {
        let name = name.as_str();
        if name == "body" || TRANSPORT_ARGUMENTS.contains(&name) {
            continue;
        }
        if RETIRED_ARGUMENTS.contains(&name) {
            return Err(retired_argument_error(name));
        }
        if !LIST_QUERY_MEMBERS.contains(&name) {
            return Err(format!(
                "unknown collection query argument `{name}`; expected one of: {}, body",
                LIST_QUERY_MEMBERS.join(", ")
            ));
        }
        controls.insert(name.to_owned(), flat_control_value(name, value)?);
    }
    let query = match arguments.get("body") {
        None => Value::Object(controls),
        Some(body) => {
            if let Some(name) = controls.keys().next() {
                return Err(format!(
                    "`body` already holds the complete query; move `{name}` into `body` or omit `body`"
                ));
            }
            if !body.is_object() {
                return Err(
                    "`body` must be a query object such as {\"filter\": \"…\", \"limit\": 50}"
                        .to_owned(),
                );
            }
            body.clone()
        }
    };
    ListQuery::from_json_value(query).map_err(|error| error.to_string())
}

/// Explain how to replace a retired paging or envelope argument.
fn retired_argument_error(name: &str) -> String {
    let replacement = match name {
        "count_mode" => " Set `include_total` to true to receive the match count as `total`.",
        "query" => {
            " Pass `filter`, `sort`, `select`, `limit`, `cursor` and `include_total` as top-level arguments."
        }
        _ => "",
    };
    format!(
        "`{name}` is retired: collections page by cursor, not by offset. Set `limit` and pass the previous page's `next_cursor` as `cursor` until it is null.{replacement}"
    )
}

/// Normalize the comma-separated `GET` spellings of flat `sort` and `select`
/// to their JSON arrays; every other control is passed through unchanged.
fn flat_control_value(name: &str, value: &Value) -> Result<Value, String> {
    match (name, value) {
        ("sort", Value::String(text)) => {
            let keys = parse_sort(text)
                .map_err(|error| ListQueryError::new("sort", error.to_string()).to_string())?;
            Ok(Value::Array(
                keys.iter()
                    .map(|key| Value::String(key.to_string()))
                    .collect(),
            ))
        }
        ("select", Value::String(text)) => Ok(Value::Array(
            text.split(',')
                .map(|field| Value::String(field.trim().to_owned()))
                .collect(),
        )),
        _ => Ok(value.clone()),
    }
}

/// Append `query` to a `GET` collection `route` as form-encoded URL
/// parameters; an empty query leaves the route unchanged.
///
/// # Errors
/// `aggregate` and filters with object or array literals have no URL form,
/// or the route could not be reserved.
pub(super) fn append_list_query_parameters(
    mut route: String,
    query: &ListQuery,
) -> Result<String, String> {
    let parameters = query.to_query_pairs().map_err(|error| error.to_string())?;
    if let (Some(filter), Some((_, text))) = (
        &query.filter,
        parameters.iter().find(|(name, _)| *name == "filter"),
    ) && !matches!(FilterExpr::parse(text), Ok(parsed) if parsed == *filter)
    {
        // Only filters without object or array literals have a text form.
        return Err(ListQueryError::new(
            "filter",
            "object and array literals exist only in the JSON form, which a `GET` collection read cannot carry; use the collection's `POST …/query` tool",
        )
        .to_string());
    }
    if parameters.is_empty() {
        return Ok(route);
    }
    let query_start = try_begin_form_query(&mut route)?;
    for (name, value) in &parameters {
        try_append_form_pair(&mut route, query_start, name, value)?;
    }
    Ok(route)
}

/// The `GET` route of a collection read: `route` with the query in
/// `arguments` appended as URL parameters.
///
/// The query is built synchronously, so dispatch futures hold only the route.
///
/// # Errors
/// The arguments do not form a collection query, or the query has no URL form.
pub(super) fn collection_get_route(route: String, arguments: &Map) -> Result<String, String> {
    let query = list_query_from_tool_arguments(arguments)?;
    append_list_query_parameters(route, &query)
}

/// The canonical JSON body of a `POST …/query` collection read built from the
/// query in `arguments`.
///
/// # Errors
/// The arguments do not form a collection query, or the body cannot be encoded.
pub(super) fn collection_post_body(arguments: &Map) -> Result<Vec<u8>, String> {
    let query = list_query_from_tool_arguments(arguments)?;
    encode_mcp_json_body(&query.to_json_value(), "encode collection query body")
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_torii_shared::list_query::{SortKey, field};

    fn arguments(value: Value) -> Map {
        match value {
            Value::Object(map) => map,
            other => panic!("tool arguments must be an object, got {other:?}"),
        }
    }

    fn expected_page_query() -> ListQuery {
        ListQuery::new()
            .filter(field("owned_by").eq("alice") & field("quantity").gte(10))
            .sort_by(SortKey::desc("quantity"))
            .sort_by(SortKey::asc("id"))
            .select(["id", "quantity"])
            .limit(25)
            .cursor("c1_A-z")
            .include_total()
    }

    #[test]
    fn flat_controls_accept_get_spellings_and_skip_transport_arguments() {
        let query = list_query_from_tool_arguments(&arguments(norito::json!({
            "path": { "account_id": "alice" },
            "filter": "owned_by = \"alice\" and quantity >= 10",
            "sort": "-quantity, id",
            "select": "id, quantity",
            "limit": 25,
            "cursor": "c1_A-z",
            "include_total": true,
            "headers": { "x-test": "1" },
            "accept": "application/json"
        })))
        .expect("flat GET spellings");
        assert_eq!(query, expected_page_query());
        assert_eq!(
            query.to_json_value(),
            norito::json!({
                "filter": (expected_page_query().filter.expect("filter").to_json_value()),
                "sort": ["-quantity", "id"],
                "select": ["id", "quantity"],
                "limit": 25,
                "cursor": "c1_A-z",
                "include_total": true
            })
        );
    }

    #[test]
    fn flat_controls_accept_arrays_and_the_json_filter_form() {
        let query = list_query_from_tool_arguments(&arguments(norito::json!({
            "filter": {
                "op": "and",
                "args": [
                    { "op": "eq", "args": ["owned_by", "alice"] },
                    { "op": "gte", "args": ["quantity", 10] }
                ]
            },
            "sort": ["-quantity", "id"],
            "select": ["id", "quantity"],
            "limit": 25,
            "cursor": "c1_A-z",
            "include_total": true
        })))
        .expect("flat JSON spellings");
        assert_eq!(query, expected_page_query());
        assert_eq!(
            list_query_from_tool_arguments(&Map::new()).expect("empty query"),
            ListQuery::new()
        );
    }

    #[test]
    fn body_is_one_complete_query_exclusive_with_flat_controls() {
        let body = norito::json!({
            "filter": "metadata.tier in [1, 2]",
            "aggregate": {
                "group_by": ["asset"],
                "metrics": [{ "alias": "holders", "fn": "count" }]
            },
            "sort": ["-holders"],
            "limit": 5
        });
        let query = list_query_from_tool_arguments(&arguments(norito::json!({
            "body": (body.clone()),
            "path": { "definition_id": "x" },
            "headers": {}
        })))
        .expect("complete body");
        assert_eq!(query.to_json_value()["sort"], norito::json!(["-holders"]));
        assert_eq!(query.limit, Some(5));
        assert_eq!(
            query.filter,
            Some(FilterExpr::parse("metadata.tier in [1, 2]").expect("filter"))
        );
        assert!(query.aggregate.is_some());

        let error = list_query_from_tool_arguments(&arguments(norito::json!({
            "body": (body),
            "limit": 2
        })))
        .expect_err("flat controls next to body");
        assert!(
            error.contains("`limit`") && error.contains("`body`"),
            "{error}"
        );

        for body in [norito::json!("limit=2"), norito::json!([])] {
            let error =
                list_query_from_tool_arguments(&arguments(norito::json!({ "body": (body) })))
                    .expect_err("non-object body");
            assert!(error.contains("query object"), "{error}");
        }
        let error = list_query_from_tool_arguments(&arguments(norito::json!({
            "body": { "pagination": { "limit": 1 } }
        })))
        .expect_err("unknown body member");
        assert!(error.contains("unknown member `pagination`"), "{error}");
    }

    #[test]
    fn retired_arguments_explain_cursor_paging() {
        for (name, value) in [
            ("pagination", norito::json!({ "limit": 10, "offset": 20 })),
            ("offset", norito::json!(20)),
            ("fetch_size", norito::json!(10)),
            ("count_mode", norito::json!("exact")),
            ("query", norito::json!({ "limit": 10 })),
        ] {
            let mut map = Map::new();
            map.insert(name.to_owned(), value);
            let error = list_query_from_tool_arguments(&map).expect_err(name);
            assert!(
                error.starts_with(&format!("`{name}` is retired")),
                "{error}"
            );
            assert!(
                error.contains("`limit`")
                    && error.contains("`next_cursor` as `cursor`")
                    && error.contains("until it is null"),
                "{error}"
            );
        }
        assert!(retired_argument_error("count_mode").contains("`include_total`"));
        assert!(retired_argument_error("query").contains("top-level arguments"));
    }

    #[test]
    fn unknown_arguments_name_the_accepted_controls() {
        let error = list_query_from_tool_arguments(&arguments(norito::json!({ "page": 1 })))
            .expect_err("unknown argument");
        assert!(
            error.contains("unknown collection query argument `page`"),
            "{error}"
        );
        assert!(
            error.contains("filter, sort, select, aggregate, limit, cursor, include_total, body"),
            "{error}"
        );
    }

    #[test]
    fn invalid_controls_surface_the_list_query_error() {
        for (arguments_value, prefix, needle) in [
            (
                norito::json!({ "filter": "a = = 1" }),
                "invalid `filter`",
                "literal",
            ),
            (norito::json!({ "filter": 5 }), "invalid `filter`", ""),
            (
                norito::json!({ "sort": "id:desc" }),
                "invalid `sort`",
                "`-field`",
            ),
            (
                norito::json!({ "sort": "id,-id" }),
                "invalid `sort`",
                "more than once",
            ),
            (norito::json!({ "sort": 7 }), "invalid `sort`", "array"),
            (
                norito::json!({ "select": "id,,name" }),
                "invalid `select`",
                "empty",
            ),
            (
                norito::json!({ "select": [] }),
                "invalid `select`",
                "at least one",
            ),
            (
                norito::json!({ "limit": 0 }),
                "invalid `limit`",
                "at least 1",
            ),
            (
                norito::json!({ "limit": "10" }),
                "invalid `limit`",
                "positive integer",
            ),
            (
                norito::json!({ "cursor": "has space" }),
                "invalid `cursor`",
                "next_cursor",
            ),
            (
                norito::json!({ "include_total": "yes" }),
                "invalid `include_total`",
                "true or false",
            ),
            (
                norito::json!({
                    "select": ["id"],
                    "aggregate": { "metrics": [{ "alias": "n", "fn": "count" }] }
                }),
                "invalid `select`",
                "cannot be combined",
            ),
        ] {
            let error = list_query_from_tool_arguments(&arguments(arguments_value.clone()))
                .expect_err("invalid control");
            assert!(error.starts_with(prefix), "{arguments_value:?}: {error}");
            assert!(error.contains(needle), "{arguments_value:?}: {error}");
        }
    }

    #[test]
    fn get_parameters_use_the_canonical_form_encoded_url_spelling() {
        let query = ListQuery::new()
            .filter(FilterExpr::parse("metadata.tier >= 2").expect("filter"))
            .sort_by(SortKey::desc("id"))
            .select(["id", "label"])
            .limit(20)
            .cursor("c1")
            .include_total();
        let route =
            append_list_query_parameters("/v1/accounts".to_owned(), &query).expect("GET route");
        assert_eq!(
            route,
            "/v1/accounts?filter=metadata.tier+%3E%3D+2&sort=-id&select=id%2Clabel&limit=20&cursor=c1&include_total=true"
        );
        let (_, raw_query) = route.split_once('?').expect("query string");
        let decoded =
            ListQuery::from_query_pairs(url::form_urlencoded::parse(raw_query.as_bytes()))
                .expect("server-side decoding");
        assert_eq!(decoded, query);

        let quoted = list_query_from_tool_arguments(&arguments(norito::json!({
            "filter": { "op": "eq", "args": ["owned_by", "sorau a&b=\"c\""] }
        })))
        .expect("JSON filter");
        let route =
            append_list_query_parameters("/v1/nfts".to_owned(), &quoted).expect("quoted GET route");
        let (_, raw_query) = route.split_once('?').expect("query string");
        assert_eq!(
            ListQuery::from_query_pairs(url::form_urlencoded::parse(raw_query.as_bytes()))
                .expect("quoted decoding"),
            quoted
        );
    }

    #[test]
    fn get_parameters_leave_empty_queries_alone_and_reject_aggregates() {
        assert_eq!(
            append_list_query_parameters("/v1/domains".to_owned(), &ListQuery::new())
                .expect("empty query"),
            "/v1/domains"
        );
        let aggregate = list_query_from_tool_arguments(&arguments(norito::json!({
            "aggregate": { "metrics": [{ "alias": "n", "fn": "count" }] }
        })))
        .expect("aggregate query");
        let error = append_list_query_parameters("/v1/domains".to_owned(), &aggregate)
            .expect_err("aggregate has no URL form");
        assert!(
            error.contains("invalid `aggregate`") && error.contains("POST /query"),
            "{error}"
        );
        let structured = list_query_from_tool_arguments(&arguments(norito::json!({
            "filter": { "op": "eq", "args": ["metadata.profile", { "tier": 1 }] }
        })))
        .expect("JSON-form metadata object literal");
        let error = append_list_query_parameters("/v1/domains".to_owned(), &structured)
            .expect_err("object literals have no text form");
        assert!(
            error.starts_with("invalid `filter`") && error.contains("JSON form"),
            "{error}"
        );
    }

    #[test]
    fn request_builders_carry_the_parsed_query() {
        let page = arguments(norito::json!({
            "path": { "account_id": "alice" },
            "filter": "quantity > 0",
            "limit": 2,
            "headers": {}
        }));
        assert_eq!(
            collection_get_route("/v1/accounts/alice/assets".to_owned(), &page).expect("GET route"),
            "/v1/accounts/alice/assets?filter=quantity+%3E+0&limit=2"
        );
        let body = collection_post_body(&page).expect("POST body");
        let body = norito::json::from_slice::<Value>(&body).expect("JSON body");
        assert_eq!(
            ListQuery::from_json_value(body).expect("server decode"),
            list_query_from_tool_arguments(&page).expect("collection query")
        );
        let retired = arguments(norito::json!({ "limit": 2, "offset": 4 }));
        for error in [
            collection_get_route("/v1/domains".to_owned(), &retired).expect_err("GET"),
            collection_post_body(&retired).expect_err("POST"),
        ] {
            assert!(error.starts_with("`offset` is retired"), "{error}");
        }
    }

    #[test]
    fn shapes_advertise_exactly_their_controls() {
        for (shape, expected) in [
            (
                CollectionQueryShape::Get,
                &[
                    "cursor",
                    "filter",
                    "include_total",
                    "limit",
                    "select",
                    "sort",
                ][..],
            ),
            (
                CollectionQueryShape::Post,
                &[
                    "aggregate",
                    "body",
                    "cursor",
                    "filter",
                    "include_total",
                    "limit",
                    "select",
                    "sort",
                ][..],
            ),
            (
                CollectionQueryShape::HistoryGet,
                &["cursor", "filter", "limit", "select"][..],
            ),
            (
                CollectionQueryShape::HistoryPost,
                &["body", "cursor", "filter", "limit", "select"][..],
            ),
        ] {
            let properties = collection_query_properties(shape);
            assert_eq!(
                properties.keys().map(String::as_str).collect::<Vec<_>>(),
                expected,
                "{shape:?}"
            );
            for (name, property) in &properties {
                assert!(
                    property
                        .get("description")
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.is_empty()),
                    "{shape:?} `{name}` must be documented"
                );
            }
            let schema = collection_query_input_schema(shape);
            assert_eq!(schema["additionalProperties"], Value::Bool(false));
            assert!(schema["properties"].get("headers").is_some());
            assert!(schema["properties"].get("accept").is_some());
            super::super::validate_advertised_schema_patterns(&schema, "collection schema")
                .expect("advertised patterns compile");
        }
        let history = collection_query_properties(CollectionQueryShape::HistoryPost);
        assert!(
            history["body"]["description"]
                .as_str()
                .is_some_and(|text| !text.contains("sort"))
        );
    }

    #[test]
    fn schema_validation_accepts_documented_forms_and_rejects_others() {
        let validate = |shape: CollectionQueryShape, value: Value| {
            let schema =
                super::super::sanitize_tool_input_schema(&collection_query_input_schema(shape));
            super::super::validate_json_schema_value(&schema, &value, "arguments")
        };
        for value in [
            norito::json!({}),
            norito::json!({
                "filter": "quantity > 0",
                "sort": "-quantity,id",
                "select": "id,quantity",
                "limit": 1,
                "cursor": "c1_A-z",
                "include_total": false
            }),
            norito::json!({
                "filter": { "op": "gt", "args": ["quantity", 0] },
                "sort": ["-supply"],
                "aggregate": {
                    "group_by": ["asset"],
                    "metrics": [
                        { "alias": "n", "fn": "count" },
                        { "alias": "supply", "fn": "sum", "field": "quantity" }
                    ],
                    "having": "n >= 2"
                }
            }),
            norito::json!({ "body": { "filter": "quantity > 0", "limit": 5 } }),
        ] {
            validate(CollectionQueryShape::Post, value.clone())
                .unwrap_or_else(|error| panic!("{value:?}: {error}"));
        }
        for (shape, value) in [
            (CollectionQueryShape::Post, norito::json!({ "offset": 0 })),
            (
                CollectionQueryShape::Post,
                norito::json!({ "pagination": {} }),
            ),
            (CollectionQueryShape::Post, norito::json!({ "limit": 0 })),
            (CollectionQueryShape::Post, norito::json!({ "filter": 1 })),
            (
                CollectionQueryShape::Post,
                norito::json!({ "filter": { "op": "between", "args": [] } }),
            ),
            (
                CollectionQueryShape::Post,
                norito::json!({ "filter": { "op": "eq" } }),
            ),
            (
                CollectionQueryShape::Post,
                norito::json!({ "filter": { "op": "eq", "args": [], "field": "id" } }),
            ),
            (
                CollectionQueryShape::Post,
                norito::json!({
                    "aggregate": {
                        "metrics": [{ "alias": "n", "fn": "count" }],
                        "having": { "op": "gt" }
                    }
                }),
            ),
            (CollectionQueryShape::Post, norito::json!({ "sort": [1] })),
            (CollectionQueryShape::Post, norito::json!({ "cursor": "" })),
            (
                CollectionQueryShape::Post,
                norito::json!({ "cursor": "a b" }),
            ),
            (
                CollectionQueryShape::Post,
                norito::json!({ "aggregate": { "metrics": [{ "alias": "n", "fn": "median" }] } }),
            ),
            (
                CollectionQueryShape::Post,
                norito::json!({ "aggregate": { "metrics": [], "extra": 1 } }),
            ),
            (CollectionQueryShape::Get, norito::json!({ "body": {} })),
            (
                CollectionQueryShape::Get,
                norito::json!({ "aggregate": { "metrics": [{ "alias": "n", "fn": "count" }] } }),
            ),
            (
                CollectionQueryShape::HistoryGet,
                norito::json!({ "sort": "id" }),
            ),
            (
                CollectionQueryShape::HistoryPost,
                norito::json!({ "include_total": true }),
            ),
        ] {
            assert!(
                validate(shape, value.clone()).is_err(),
                "{shape:?} {value:?}"
            );
        }
    }

    #[test]
    fn descriptor_markers_expand_into_the_shared_controls() {
        let mut schema = norito::json!({
            "type": "object",
            "additionalProperties": false,
            "x-iroha-mcp-collection-query": "history_get",
            "required": ["path"],
            "properties": {
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["account_id"],
                    "properties": { "account_id": { "type": "string" } }
                },
                "headers": { "type": "object", "additionalProperties": { "type": "string" } },
                "accept": { "type": "string" }
            }
        });
        expand_collection_query_schema(&mut schema).expect("marker expands");
        assert!(schema.get(COLLECTION_QUERY_SCHEMA_EXTENSION).is_none());
        let properties = schema["properties"].as_object().expect("properties");
        assert_eq!(
            properties.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "accept", "cursor", "filter", "headers", "limit", "path", "select"
            ]
        );
        for (name, property) in collection_query_properties(CollectionQueryShape::HistoryGet) {
            assert_eq!(properties[&name], property, "{name}");
        }

        let mut unmarked = norito::json!({ "type": "object", "properties": {} });
        let before = unmarked.clone();
        expand_collection_query_schema(&mut unmarked).expect("unmarked schema");
        assert_eq!(unmarked, before);

        for invalid in [
            norito::json!({ "x-iroha-mcp-collection-query": "patch", "properties": {} }),
            norito::json!({ "x-iroha-mcp-collection-query": 1, "properties": {} }),
            norito::json!({ "x-iroha-mcp-collection-query": "get" }),
            norito::json!({
                "x-iroha-mcp-collection-query": "get",
                "properties": { "limit": { "type": "integer" } }
            }),
        ] {
            let mut invalid = invalid;
            expand_collection_query_schema(&mut invalid).expect_err("invalid marker");
        }
    }
}
