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
fn native_batch_results_remap_legacy_application_error_codes() {
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
