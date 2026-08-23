#[test]
fn outer_batch_charges_nested_tool_calls_against_one_dispatch_limit() {
    let calls = (0..MAX_JSONRPC_BATCH_DISPATCHES)
        .map(|index| norito::json!({ "name": "iroha.health", "arguments": { "index": index } }))
        .collect::<Vec<_>>();
    let at_limit = vec![norito::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call_batch",
        "params": { "calls": (calls.clone()) }
    })];
    assert!(!jsonrpc_batch_exceeds_dispatch_limit(&at_limit));

    let over_limit = vec![
        at_limit[0].clone(),
        norito::json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }),
    ];
    assert!(jsonrpc_batch_exceeds_dispatch_limit(&over_limit));
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
    let payload: Value = json::from_slice(&bytes).expect("typed JSON-RPC fallback");
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
async fn nested_route_response_collection_has_a_hard_byte_cap() {
    let response = Response::new(Body::from(vec![0_u8; 17]));
    let error = response_to_value(response, 16)
        .await
        .expect_err("seventeenth byte exceeds cap");
    assert_eq!(error, TARGET_RESPONSE_TOO_LARGE_MESSAGE);
}
