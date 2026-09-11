fn expand_advertised_schema_refs(schema: &Value) -> Value {
    fn expand(value: &Value, root: &Value, active: &mut BTreeSet<String>) -> Value {
        match value {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                    assert_eq!(
                        object.len(),
                        1,
                        "factored references have no sibling keywords"
                    );
                    assert!(
                        reference.starts_with("#/$defs/"),
                        "external reference: {reference}"
                    );
                    assert!(
                        active.insert(reference.to_owned()),
                        "cyclic reference: {reference}"
                    );
                    let target = root
                        .pointer(&reference[1..])
                        .expect("self-contained reference");
                    let expanded = expand(target, root, active);
                    active.remove(reference);
                    return expanded;
                }
                Value::Object(
                    object
                        .iter()
                        .map(|(key, value)| {
                            let value = match key.as_str() {
                                "properties" | "patternProperties" | "dependentSchemas"
                                | "$defs" | "definitions" | "dependencies" => Value::Object(
                                    value
                                        .as_object()
                                        .expect("schema map")
                                        .iter()
                                        .map(|(name, child)| {
                                            (name.clone(), expand(child, root, active))
                                        })
                                        .collect(),
                                ),
                                "allOf"
                                | "anyOf"
                                | "oneOf"
                                | "prefixItems"
                                | "additionalProperties"
                                | "additionalItems"
                                | "items"
                                | "contains"
                                | "propertyNames"
                                | "not"
                                | "if"
                                | "then"
                                | "else"
                                | "unevaluatedProperties"
                                | "unevaluatedItems"
                                | "contentSchema" => expand(value, root, active),
                                _ => value.clone(),
                            };
                            (key.clone(), value)
                        })
                        .collect(),
                )
            }
            Value::Array(values) => Value::Array(
                values
                    .iter()
                    .map(|value| expand(value, root, active))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }
    let mut expanded = expand(schema, schema, &mut BTreeSet::new());
    expanded
        .as_object_mut()
        .expect("input schema object")
        .remove("$defs");
    expanded
}

async fn modern_doctor_tools_page(app: &SharedAppState, cursor: Option<&str>) -> Value {
    use http_body_util::BodyExt as _;
    use iroha_torii_shared::mcp as wire;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::ACCEPT,
        HeaderValue::from_static("application/json, text/event-stream"),
    );
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        wire::HEADER_PROTOCOL_VERSION,
        HeaderValue::from_static(wire::MODERN_PROTOCOL_VERSION),
    );
    headers.insert(wire::HEADER_METHOD, HeaderValue::from_static("tools/list"));
    let mut meta = Map::new();
    meta.insert(
        wire::META_PROTOCOL_VERSION.into(),
        Value::from(wire::MODERN_PROTOCOL_VERSION),
    );
    meta.insert(
        wire::META_CLIENT_CAPABILITIES.into(),
        Value::Object(Map::new()),
    );
    meta.insert(
        wire::META_CLIENT_INFO.into(),
        norito::json!({ "name": "iroha-taira-doctor", "version": "1" }),
    );
    let mut params = norito::json!({ "_meta": (Value::Object(meta)) });
    if let Some(cursor) = cursor {
        params
            .as_object_mut()
            .expect("params")
            .insert("cursor".into(), Value::from(cursor));
    }
    let request =
        norito::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": params });
    let validated =
        validate_protocol_request(&headers, &request).expect("actual doctor request metadata");
    let JsonRpcRequestOutcome::Response(payload) =
        handle_validated_jsonrpc_request(app.clone(), &headers, request, &validated).await
    else {
        panic!("tools/list must respond")
    };
    let response = bounded_modern_jsonrpc_http_response(payload, app.mcp.max_request_bytes);
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("bounded HTTP body")
        .to_bytes();
    assert!(bytes.len() <= app.mcp.max_request_bytes);
    let payload: Value =
        json::from_slice(&bytes).expect("actual serialized MCP response must decode");
    assert!(
        payload.get("error").is_none(),
        "catalog page became a transport error: {payload:?}"
    );
    assert_eq!(payload.get("id").and_then(Value::as_u64), Some(2));
    assert_eq!(
        payload
            .pointer("/result/resultType")
            .and_then(Value::as_str),
        Some("complete")
    );
    assert_eq!(
        payload
            .pointer("/result/cacheScope")
            .and_then(Value::as_str),
        Some("private")
    );
    assert!(
        payload
            .pointer("/result/ttlMs")
            .and_then(Value::as_u64)
            .is_some_and(|ttl| ttl > 0)
    );
    assert!(
        payload
            .pointer("/result/_meta")
            .and_then(|meta| meta.get(wire::META_SERVER_INFO))
            .is_some()
    );
    payload
}

#[tokio::test]
async fn tools_list_writer_catalog_roundtrips_through_modern_http_byte_limit() {
    for prefixes in [Vec::new(), vec!["iroha.".to_owned()]] {
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("exclusive fixture");
            state.mcp = iroha_config::parameters::actual::ToriiMcp::default();
            state.mcp.profile = ToriiMcpProfile::Writer;
            state.mcp.allow_tool_prefixes = prefixes;
            state.mcp_tools = Arc::new(build_tool_specs(&state.mcp));
        }
        assert_eq!(app.mcp.max_request_bytes, 1_048_576);
        assert_eq!(app.mcp.max_tools_per_list, 500);
        let expected = visible_tools_for_app(&app)
            .iter()
            .map(|tool| tool.name.clone())
            .collect::<Vec<_>>();
        assert!(!expected.is_empty());
        let expected_schemas = visible_tools_for_app(&app)
            .iter()
            .map(|tool| {
                (
                    tool.name.clone(),
                    sanitize_tool_input_schema(&tool.input_schema),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut names = Vec::new();
        let mut cursor = None;
        let mut pages = 0;
        loop {
            assert!(pages < 64, "catalog exceeds the doctor page bound");
            let payload = modern_doctor_tools_page(&app, cursor.as_deref()).await;
            pages += 1;
            let tools = payload
                .pointer("/result/tools")
                .and_then(Value::as_array)
                .expect("tools array");
            assert!(!tools.is_empty(), "an advertised cursor must make progress");
            assert!(tools.len() <= app.mcp.max_tools_per_list);
            names.extend(tools.iter().map(|tool| {
                let name = tool.get("name").and_then(Value::as_str).expect("name");
                let advertised = tool.get("inputSchema").expect("advertised schema");
                assert_eq!(
                    &expand_advertised_schema_refs(advertised),
                    expected_schemas.get(name).expect("visible tool"),
                    "advertisement must preserve the complete validation schema for {name}"
                );
                name.to_owned()
            }));
            let Some(next) = payload.pointer("/result/nextCursor") else {
                break;
            };
            let next = next.as_str().expect("decimal cursor");
            assert_eq!(next, names.len().to_string());
            cursor = Some(next.to_owned());
        }
        assert_eq!(
            names, expected,
            "pagination must neither omit nor duplicate actual registry tools"
        );
        eprintln!(
            "Writer MCP catalog {:?}: {} tools, {pages} bounded modern HTTP pages",
            app.mcp.allow_tool_prefixes,
            names.len()
        );
        for required in [
            "iroha.health",
            "iroha.accounts.get",
            "iroha.accounts.assets",
            "iroha.assets.definitions.get",
            "iroha.transactions.submit",
            "iroha.transactions.submit_and_wait",
        ] {
            assert!(
                names.iter().any(|name| name == required),
                "missing {required}"
            );
        }
    }
}

#[test]
fn advertised_schema_factoring_preserves_subschemas_and_literal_values() {
    let literal = norito::json!({
        "$ref": "this is literal data, not a schema reference",
        "properties": { "items": { "const": { "type": "object" } } }
    });
    let leaf = norito::json!({
        "type": "object", "const": (literal.clone()), "enum": [(literal.clone())],
        "default": (literal.clone()), "examples": [(literal.clone())],
        "x-opaque": (literal.clone())
    });
    let mut nested = leaf;
    for _ in 0..18 {
        nested = Value::Object(Map::from([
            ("type".into(), Value::from("object")),
            (
                "properties".into(),
                Value::Object(Map::from([("child".into(), nested)])),
            ),
        ]));
    }
    let mut schema = norito::json!({ "type": "object", "properties": {} });
    let root = schema.as_object_mut().expect("schema");
    for keyword in [
        "properties",
        "patternProperties",
        "dependentSchemas",
        "definitions",
        "dependencies",
    ] {
        root.insert(
            keyword.into(),
            Value::Object(Map::from([("child".into(), nested.clone())])),
        );
    }
    for keyword in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        root.insert(keyword.into(), Value::Array(vec![nested.clone()]));
    }
    for keyword in [
        "additionalProperties",
        "additionalItems",
        "items",
        "contains",
        "propertyNames",
        "not",
        "if",
        "then",
        "else",
        "unevaluatedProperties",
        "unevaluatedItems",
        "contentSchema",
    ] {
        root.insert(keyword.into(), nested.clone());
    }
    root.insert(
        "$defs".into(),
        norito::json!({ "mcp_schema_0": { "type": "string" } }),
    );
    root.get_mut("dependencies")
        .and_then(Value::as_object_mut)
        .expect("dependencies")
        .insert("names".into(), norito::json!(["other"]));
    let sanitized = sanitize_tool_input_schema(&schema);
    let advertised = advertised_tool_input_schema(&schema);
    assert_eq!(
        advertised,
        advertised_tool_input_schema(&schema),
        "deterministic definitions"
    );
    assert_eq!(
        advertised.pointer("/$defs/mcp_schema_0"),
        sanitized.pointer("/$defs/mcp_schema_0"),
        "existing definitions are not overwritten"
    );
    let mut expected = sanitized;
    expected.as_object_mut().expect("schema").remove("$defs");
    assert_eq!(expand_advertised_schema_refs(&advertised), expected);
    let envelope = norito::json!({ "jsonrpc": "2.0", "id": 2, "result": { "tools": [{ "inputSchema": advertised }] } });
    assert!(json::to_json_bounded_boxed(&envelope, 1_048_576).is_ok());

    // Tuple-form items is a schema array; dependency-name arrays remain data.
    let tuple = Value::Object(Map::from([
        ("type".into(), Value::from("object")),
        (
            "properties".into(),
            Value::Object(Map::from([(
                "body".into(),
                Value::Object(Map::from([("items".into(), Value::Array(vec![nested]))])),
            )])),
        ),
    ]));
    assert_eq!(
        expand_advertised_schema_refs(&advertised_tool_input_schema(&tuple)),
        sanitize_tool_input_schema(&tuple)
    );
    // Moving schemas across a resource identifier would alter reference scope.
    let mut scoped = schema;
    scoped
        .as_object_mut()
        .expect("schema")
        .insert("$id".into(), Value::from("https://example.invalid/schema"));
    assert_eq!(
        advertised_tool_input_schema(&scoped),
        sanitize_tool_input_schema(&scoped)
    );
}

#[test]
fn tools_list_byte_budget_includes_envelope_and_rejects_oversized_single_tool() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("exclusive fixture");
        state.mcp_tools = Arc::new(vec![iroha_health_tool(), iroha_parameters_get_tool()]);
        state.mcp.max_tools_per_list = 500;
    }
    let id = Some(Value::from("quoted request \" id"));
    let visible = visible_tools_for_app(&app);
    let version = compute_toolset_version(&visible);
    let expected = tools_list_page_response(
        id.clone(),
        vec![visible[0].descriptor()],
        Some(1),
        &version,
        false,
        ProtocolEra::Modern,
    );
    let exact = bounded_json_value_len(&expected, usize::MAX).expect("one decorated tool");
    Arc::get_mut(&mut app)
        .expect("exclusive fixture")
        .mcp
        .max_request_bytes = exact;
    let page = handle_tools_list(id.clone(), &app, &Map::new(), ProtocolEra::Modern);
    assert_eq!(
        page, expected,
        "an exact-boundary descriptor must be admitted with its cursor"
    );
    assert_eq!(
        json::to_json_bounded_boxed(&page, exact)
            .expect("actual serializer")
            .len(),
        exact
    );
    Arc::get_mut(&mut app)
        .expect("exclusive fixture")
        .mcp
        .max_request_bytes = exact - 1;
    let rejected = handle_tools_list(id, &app, &Map::new(), ProtocolEra::Modern);
    assert_eq!(
        rejected
            .pointer("/error/data/error_code")
            .and_then(Value::as_str),
        Some(MCP_RESPONSE_TOO_LARGE_CODE)
    );
    assert!(
        rejected.get("result").is_none(),
        "one oversized tool cannot become an empty success page"
    );
}

#[test]
fn tool_batch_rate_cost_matches_nested_dispatch_count() {
    let calls = (0..MAX_JSONRPC_BATCH_DISPATCHES)
        .map(|index| norito::json!({ "name": "iroha.health", "arguments": { "index": index } }))
        .collect::<Vec<_>>();
    let at_limit = norito::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call_batch",
        "params": { "calls": (calls.clone()) }
    });
    assert_eq!(
        jsonrpc_dispatch_cost(&at_limit),
        MAX_JSONRPC_BATCH_DISPATCHES
    );
    assert_eq!(
        jsonrpc_dispatch_cost(&norito::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "ping"
        })),
        1
    );
}

#[test]
fn native_early_transport_errors_omit_an_unreadable_request_id() {
    let mut headers = HeaderMap::new();
    headers.insert(
        protocol::HEADER_PROTOCOL_VERSION,
        HeaderValue::from_static(protocol::MODERN_PROTOCOL_VERSION),
    );
    let mut native = jsonrpc_request_timeout();
    adapt_transport_error_for_headers(&headers, &mut native);
    assert!(native.get("id").is_none());
    assert_eq!(
        native.pointer("/error/code").and_then(Value::as_i64),
        Some(MODERN_IROHA_REQUEST_TIMEOUT)
    );

    let mut legacy = jsonrpc_request_timeout();
    adapt_transport_error_for_headers(&HeaderMap::new(), &mut legacy);
    assert!(legacy.get("id").is_some_and(Value::is_null));
    assert_eq!(
        legacy.pointer("/error/code").and_then(Value::as_i64),
        Some(MCP_REQUEST_TIMEOUT)
    );
}

#[test]
fn native_batch_results_remap_legacy_application_error_types() {
    let mut response = norito::json!({
        "jsonrpc": "2.0",
        "id": "batch",
        "result": {
            "results": [
                {
                    "error": {
                        "code": (MCP_DISPATCH_CAPACITY_EXHAUSTED),
                        "message": "capacity exhausted"
                    }
                },
                {
                    "error": {
                        "code": (MCP_RESPONSE_TOO_LARGE),
                        "message": "response too large"
                    }
                }
            ]
        }
    });

    remap_modern_application_error(&mut response);

    assert_eq!(
        response
            .pointer("/result/results/0/error/code")
            .and_then(Value::as_i64),
        Some(MODERN_IROHA_DISPATCH_CAPACITY_EXHAUSTED)
    );
    assert_eq!(
        response
            .pointer("/result/results/1/error/code")
            .and_then(Value::as_i64),
        Some(MODERN_IROHA_RESPONSE_TOO_LARGE)
    );
}

#[test]
fn bounded_json_array_rejects_before_retaining_an_over_budget_value() {
    let mut values = BoundedJsonArray::new(2, 9).expect("array envelope fits");
    values
        .try_push(Value::String("abc".to_owned()))
        .expect("first value fits");
    assert_eq!(
        values.try_push(Value::String("def".to_owned())),
        Err(BoundedJsonError::BodyTooLarge)
    );
    assert_eq!(values.into_values(), vec![Value::String("abc".to_owned())]);
}

#[tokio::test]
async fn bounded_jsonrpc_response_falls_back_to_typed_limit_error() {
    use http_body_util::BodyExt as _;

    let response = bounded_jsonrpc_http_response(
        jsonrpc_result_response(
            Some(Value::from(7_u64)),
            norito::json!({ "body": ("x".repeat(512)) }),
        ),
        128,
    );
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&HeaderValue::from_static("private, no-store"))
    );
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("fixed fallback body")
        .to_bytes();
    assert!(bytes.len() <= 128, "fallback exceeded configured cap");
    let payload: Value = json::from_slice(&bytes).expect("typed JSON-RPC fallback");
    assert_eq!(payload.get("id").and_then(Value::as_u64), Some(7));
    assert_eq!(
        payload
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_i64),
        Some(MCP_RESPONSE_TOO_LARGE)
    );
    assert_eq!(
        payload
            .get("error")
            .and_then(|error| error.get("data"))
            .and_then(|data| data.get("error_code"))
            .and_then(Value::as_str),
        Some(MCP_RESPONSE_TOO_LARGE_CODE)
    );
}

#[tokio::test]
async fn bounded_modern_response_uses_an_application_error_code() {
    use http_body_util::BodyExt as _;

    let response = bounded_modern_jsonrpc_http_response(
        jsonrpc_result_response(
            Some(Value::from(9_u64)),
            norito::json!({ "body": ("x".repeat(512)) }),
        ),
        128,
    );
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("fixed modern fallback body")
        .to_bytes();
    let payload: Value = json::from_slice(&bytes).expect("typed modern JSON-RPC fallback");
    assert_eq!(payload.get("id").and_then(Value::as_u64), Some(9));
    assert_eq!(
        payload.pointer("/error/code").and_then(Value::as_i64),
        Some(MODERN_IROHA_RESPONSE_TOO_LARGE)
    );
    assert_eq!(
        payload
            .pointer("/error/data/error_code")
            .and_then(Value::as_str),
        Some(MCP_RESPONSE_TOO_LARGE_CODE)
    );
}

#[tokio::test]
async fn nested_route_response_collection_has_a_hard_byte_cap() {
    let response = Response::new(Body::from(vec![0_u8; 17]));
    let error = response_to_value(response, 16)
        .await
        .expect_err("seventeenth byte exceeds cap");
    assert_eq!(error, TARGET_RESPONSE_TOO_LARGE_MESSAGE);
}
