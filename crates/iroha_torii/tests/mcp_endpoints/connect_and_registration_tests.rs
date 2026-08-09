// MCP Connect lifecycle and disabled-route registration regressions.

#[tokio::test]
async fn mcp_jsonrpc_connect_alias_lifecycle_dispatches_routes() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    enable_writer_mcp(&mut cfg);
    cfg.torii.connect.enabled = true;

    let app = build_router(cfg);
    let sid = B64.encode([0x66u8; 32]);

    let (status, create_call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 114,
            "method": "tools/call",
            "params": {
                "name": "iroha.connect.session.create",
                "arguments": {
                    "session_id": sid
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&create_call),
        "connect alias create should not be an MCP tool error"
    );
    let create_structured = structured_content(&create_call);
    assert_eq!(
        create_structured.get("status").and_then(Value::as_u64),
        Some(200)
    );
    let create_body = create_structured
        .get("body")
        .and_then(Value::as_object)
        .expect("session create body");
    let token_app = create_body
        .get("token_app")
        .and_then(Value::as_str)
        .expect("token_app present")
        .to_owned();
    let token_management = create_body
        .get("token_management")
        .and_then(Value::as_str)
        .expect("token_management present")
        .to_owned();

    let (status, ticket_call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 115,
            "method": "tools/call",
            "params": {
                "name": "iroha.connect.ws.ticket",
                "arguments": {
                    "session_id": sid,
                    "role": "app",
                    "token_app": token_app,
                    "node_url": "https://node.example"
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&ticket_call),
        "connect alias ticket should not be an MCP tool error"
    );
    let ticket_structured = structured_content(&ticket_call);
    assert_eq!(
        ticket_structured.get("ws_url").and_then(Value::as_str),
        Some(
            "wss://node.example/v1/connect/ws?sid=ZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmY&role=app"
        )
    );

    let (status, status_call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 116,
            "method": "tools/call",
            "params": {
                "name": "iroha.connect.status"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&status_call),
        "connect alias status should not be an MCP tool error"
    );
    let status_structured = structured_content(&status_call);
    assert_eq!(
        status_structured.get("status").and_then(Value::as_u64),
        Some(200)
    );

    let (status, delete_call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 117,
            "method": "tools/call",
            "params": {
                "name": "iroha.connect.session.delete",
                "arguments": {
                    "path": {
                        "session_id": sid
                    },
                    "token_management": token_management
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&delete_call),
        "connect alias delete should not be an MCP tool error"
    );
    let delete_structured = structured_content(&delete_call);
    assert_eq!(
        delete_structured.get("status").and_then(Value::as_u64),
        Some(204)
    );
}

#[tokio::test]
async fn mcp_routes_remain_registered_and_report_disabled_state() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = false;

    let app = build_router(cfg);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/mcp")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let capabilities = read_json_body(response).await;
    assert_eq!(
        capabilities.get("enabled").and_then(Value::as_bool),
        Some(false)
    );

    let (status, error) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": "disabled",
            "method": "ping"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        error.get("code").and_then(Value::as_str),
        Some("mcp_disabled")
    );
}
