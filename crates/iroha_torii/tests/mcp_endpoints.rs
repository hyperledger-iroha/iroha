#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration coverage for Torii MCP endpoints.

use std::{num::NonZeroU32, sync::Arc};

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use http_body_util::BodyExt as _;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, test_utils};
use norito::json::Value;
use tower::ServiceExt as _;

fn build_router(cfg: iroha_config::parameters::actual::Root) -> axum::Router {
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = Arc::new(State::new_for_testing(
        World::default(),
        kura.clone(),
        query,
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let _ = peers_tx;

    let torii = Torii::new_with_handle(
        cfg.common.chain.clone(),
        kiso,
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        kura,
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );

    torii.api_router_for_tests()
}

async fn read_json_body(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    norito::json::from_slice(&bytes).expect("valid json body")
}

async fn post_mcp(app: &axum::Router, payload: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/v1/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            norito::json::to_vec(&payload).expect("serialize payload"),
        ))
        .expect("valid request");

    let response = app.clone().oneshot(request).await.expect("mcp response");
    let status = response.status();
    let body = read_json_body(response).await;
    (status, body)
}

async fn post_mcp_with_headers(
    app: &axum::Router,
    payload: Value,
    headers: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/v1/mcp")
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let request = builder
        .body(Body::from(
            norito::json::to_vec(&payload).expect("serialize payload"),
        ))
        .expect("valid request");

    let response = app.clone().oneshot(request).await.expect("mcp response");
    let status = response.status();
    let body = read_json_body(response).await;
    (status, body)
}

async fn list_all_tool_names(app: &axum::Router) -> Vec<String> {
    let mut cursor: Option<String> = None;
    let mut names = Vec::new();

    loop {
        let payload = if let Some(cursor_value) = cursor.clone() {
            norito::json!({
                "jsonrpc": "2.0",
                "id": "tools-page",
                "method": "tools/list",
                "params": {
                    "cursor": cursor_value
                }
            })
        } else {
            norito::json!({
                "jsonrpc": "2.0",
                "id": "tools-page",
                "method": "tools/list"
            })
        };

        let (status, body) = post_mcp(app, payload).await;
        assert_eq!(status, StatusCode::OK);
        let result = body.get("result").expect("tools/list result");
        let page = result
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array");
        for tool in page {
            let name = tool.get("name").and_then(Value::as_str).expect("tool name");
            names.push(name.to_owned());
        }

        cursor = result
            .get("nextCursor")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }

    names
}

fn structured_content(response: &Value) -> &norito::json::Map {
    response
        .get("result")
        .and_then(|value| value.get("structuredContent"))
        .and_then(Value::as_object)
        .expect("structured content")
}

fn tool_is_error(response: &Value) -> bool {
    response
        .get("result")
        .and_then(|value| value.get("isError"))
        .and_then(Value::as_bool)
        .expect("tool isError flag")
}

#[tokio::test]
async fn mcp_capabilities_endpoint_exposes_server_metadata() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

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
        .expect("mcp capability response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json_body(response).await;
    assert_eq!(
        body.get("protocolVersion").and_then(Value::as_str),
        Some("2025-06-18")
    );
    assert_eq!(
        body.get("serverInfo")
            .and_then(|value| value.get("name"))
            .and_then(Value::as_str),
        Some("iroha-torii-mcp")
    );
    assert!(
        body.get("capabilities")
            .and_then(|value| value.get("tools"))
            .and_then(|value| value.get("count"))
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0),
        "tool count should be present and positive"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_initialize_list_and_call_connect_ticket() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.max_tools_per_list = 2;

    let app = build_router(cfg);

    let (status, initialize) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        initialize
            .get("result")
            .and_then(|value| value.get("protocolVersion"))
            .and_then(Value::as_str),
        Some("2025-06-18")
    );

    let (status, page1) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let page1_tools = page1
        .get("result")
        .and_then(|value| value.get("tools"))
        .and_then(Value::as_array)
        .expect("tools list");
    assert_eq!(page1_tools.len(), 2);
    assert_eq!(
        page1
            .get("result")
            .and_then(|value| value.get("nextCursor"))
            .and_then(Value::as_str),
        Some("2")
    );

    let (status, page2) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/list",
            "params": {
                "cursor": "2"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let page2_tools = page2
        .get("result")
        .and_then(|value| value.get("tools"))
        .and_then(Value::as_array)
        .expect("tools list page2");
    assert!(page2_tools.iter().any(|tool| {
        tool.get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| name == "connect.ws.ticket")
    }));

    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": "call-1",
            "method": "tools/call",
            "params": {
                "name": "connect.ws.ticket",
                "arguments": {
                    "sid": "sid-1",
                    "role": "app",
                    "token": "secret-token",
                    "node_url": "https://node.example"
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let structured = call
        .get("result")
        .and_then(|value| value.get("structuredContent"))
        .and_then(Value::as_object)
        .expect("structured content");
    assert_eq!(
        structured.get("ws_url").and_then(Value::as_str),
        Some("wss://node.example/v1/connect/ws?sid=sid-1&role=app")
    );
    assert_eq!(
        structured
            .get("authorization_header")
            .and_then(Value::as_str),
        Some("Bearer secret-token")
    );
    assert_eq!(
        structured
            .get("sec_websocket_protocol")
            .and_then(Value::as_str),
        Some("iroha-connect.token.v1.c2VjcmV0LXRva2Vu")
    );
}

#[tokio::test]
async fn mcp_jsonrpc_rejects_invalid_json_payload() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"jsonrpc\": \"2.0\", bad"))
                .expect("valid request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = read_json_body(response).await;
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_i64),
        Some(-32700)
    );
}

#[tokio::test]
async fn mcp_jsonrpc_rejects_non_object_request() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("1"))
                .expect("valid request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json_body(response).await;
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_i64),
        Some(-32600)
    );
}

#[tokio::test]
async fn mcp_jsonrpc_rejects_wrong_jsonrpc_version() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, body) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "1.0",
            "id": 7,
            "method": "initialize"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_i64),
        Some(-32600)
    );
}

#[tokio::test]
async fn mcp_jsonrpc_enforces_rate_limit() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.rate_per_minute = Some(NonZeroU32::new(1).expect("nonzero rate"));
    cfg.torii.mcp.burst = Some(NonZeroU32::new(1).expect("nonzero burst"));

    let app = build_router(cfg);
    let request = norito::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize"
    });

    let (status, _) = post_mcp(&app, request.clone()).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = post_mcp(&app, request).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_i64),
        Some(-32029)
    );
}

#[tokio::test]
async fn mcp_jsonrpc_rejects_oversized_payload() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.max_request_bytes = 32;

    let app = build_router(cfg);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}",
                ))
                .expect("valid request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body = read_json_body(response).await;
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_i64),
        Some(-32600)
    );
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.get("max_request_bytes"))
            .and_then(Value::as_u64),
        Some(32)
    );
}

#[tokio::test]
async fn mcp_jsonrpc_rejects_empty_batch() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("[]"))
                .expect("valid request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json_body(response).await;
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_i64),
        Some(-32600)
    );
}

#[tokio::test]
async fn mcp_jsonrpc_batch_returns_per_call_results() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"[{"jsonrpc":"2.0","id":1,"method":"initialize"},{"jsonrpc":"2.0","id":2,"method":"missing.method"}]"#,
                ))
                .expect("valid request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json_body(response).await;
    let batch = body.as_array().expect("batch response");
    assert_eq!(batch.len(), 2);
    assert_eq!(
        batch[0]
            .get("result")
            .and_then(|value| value.get("protocolVersion"))
            .and_then(Value::as_str),
        Some("2025-06-18")
    );
    assert_eq!(
        batch[1]
            .get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_i64),
        Some(-32601)
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_unknown_tool_returns_invalid_params() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, body) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 99,
            "method": "tools/call",
            "params": {
                "name": "unknown.tool"
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_i64),
        Some(-32602)
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_openapi_healthcheck_dispatches_route() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 100,
            "method": "tools/call",
            "params": {
                "name": "torii.healthCheck"
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "health check should not be an MCP error"
    );
    let structured = call
        .get("result")
        .and_then(|value| value.get("structuredContent"))
        .and_then(Value::as_object)
        .expect("structured content");
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
    assert_eq!(
        structured.get("body").and_then(Value::as_str),
        Some("Healthy")
    );
    assert!(
        structured
            .get("content_type")
            .and_then(Value::as_str)
            .is_some_and(|content_type| content_type.contains("text/plain")),
        "expected text/plain content type"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_openapi_healthcheck_requires_token_when_enabled() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.require_api_token = true;
    cfg.torii.api_tokens = vec!["mcp-token".to_owned()];

    let app = build_router(cfg);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    norito::json::to_vec(&norito::json!({
                        "jsonrpc": "2.0",
                        "id": 101,
                        "method": "tools/call",
                        "params": {
                            "name": "torii.healthCheck"
                        }
                    }))
                    .expect("serialize payload"),
                ))
                .expect("valid request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_openapi_healthcheck_respects_forwarded_headers() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 102,
            "method": "tools/call",
            "params": {
                "name": "torii.healthCheck",
                "arguments": {
                    "headers": {
                        "x-iroha-api-version": "2.0"
                    }
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let structured = call
        .get("result")
        .and_then(|value| value.get("structuredContent"))
        .and_then(Value::as_object)
        .expect("structured content");
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(400));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_openapi_healthcheck_accepts_token_from_mcp_request_headers() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.require_api_token = true;
    cfg.torii.api_tokens = vec!["mcp-token".to_owned()];

    let app = build_router(cfg);
    let (status, call) = post_mcp_with_headers(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 103,
            "method": "tools/call",
            "params": {
                "name": "torii.healthCheck"
            }
        }),
        &[("x-api-token", "mcp-token")],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let structured = call
        .get("result")
        .and_then(|value| value.get("structuredContent"))
        .and_then(Value::as_object)
        .expect("structured content");
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
    assert_eq!(
        structured.get("body").and_then(Value::as_str),
        Some("Healthy")
    );
}

#[tokio::test]
async fn mcp_tools_list_exposes_account_and_transaction_interfaces() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.max_tools_per_list = 5;

    let app = build_router(cfg);
    let names = list_all_tool_names(&app).await;

    assert!(
        names.iter().any(|name| name == "torii.get_v1_accounts"),
        "expected account listing MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "torii.get_v1_accounts_account_id_transactions"),
        "expected account transaction MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "torii.post_transaction"),
        "expected transaction submission MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.accounts.list"),
        "expected agent-friendly account listing MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.accounts.query"),
        "expected agent-friendly account query MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.accounts.transactions"),
        "expected agent-friendly account transactions MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.accounts.transactions.query"),
        "expected agent-friendly account transactions query MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.accounts.assets"),
        "expected agent-friendly account assets MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.accounts.assets.query"),
        "expected agent-friendly account assets query MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.accounts.permissions"),
        "expected agent-friendly account permissions MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.transactions.submit"),
        "expected agent-friendly transaction submit MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.transactions.status"),
        "expected agent-friendly transaction status MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.connect.session.create"),
        "expected agent-friendly connect session create MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.connect.ws.ticket"),
        "expected agent-friendly connect ws ticket MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.connect.status"),
        "expected agent-friendly connect status MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.connect.session.delete"),
        "expected agent-friendly connect session delete MCP tool"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_account_transactions_uses_path_and_query_arguments() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 104,
            "method": "tools/call",
            "params": {
                "name": "torii.get_v1_accounts_account_id_transactions",
                "arguments": {
                    "path": {
                        "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    },
                    "query": {
                        "limit": 0
                    }
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid query should be marked as MCP tool error"
    );
    let structured = call
        .get("result")
        .and_then(|value| value.get("structuredContent"))
        .and_then(Value::as_object)
        .expect("structured content");
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid `limit=0` query argument to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_accounts_list_dispatches_route() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 105,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.list"
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_transaction_status_validates_query() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106,
            "method": "tools/call",
            "params": {
                "name": "iroha.transactions.status",
                "arguments": {
                    "query": {
                        "hash": "not-a-hash"
                    }
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid hash should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid transaction hash query to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_transaction_status_accepts_flat_hash() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1061,
            "method": "tools/call",
            "params": {
                "name": "iroha.transactions.status",
                "arguments": {
                    "hash": "not-a-hash"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid flat hash should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid flat hash to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_account_transactions_accepts_flat_arguments() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1062,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.transactions",
                "arguments": {
                    "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
                    "limit": 0
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid flat query should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid flat limit to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_accounts_query_accepts_flat_envelope_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10620,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.query",
                "arguments": {
                    "limit": 2,
                    "offset": 0
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "accounts query alias with flat envelope fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_account_transactions_query_accepts_flat_arguments() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10623,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.transactions.query",
                "arguments": {
                    "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "transactions query alias with flat arguments should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_account_assets_accepts_flat_arguments() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10621,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.assets",
                "arguments": {
                    "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
                    "limit": 0
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid flat asset query should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid flat asset limit to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_account_assets_query_accepts_flat_arguments() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10624,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.assets.query",
                "arguments": {
                    "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "assets query alias with flat arguments should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_account_permissions_accepts_flat_account_id() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10622,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.permissions",
                "arguments": {
                    "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "permissions alias with flat account id should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_accounts_resolve_accepts_literal_shortcut() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1063,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.resolve",
                "arguments": {
                    "literal": "not-an-account-literal"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid literal shortcut should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid literal to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_post_transaction_dispatches_binary_payload() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    for (id, tool_name) in [
        (107, "torii.post_transaction"),
        (112, "iroha.transactions.submit"),
    ] {
        let (status, call) = post_mcp(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": {
                        "body_base64": "AQIDBA"
                    }
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            tool_is_error(&call),
            "invalid signed transaction bytes should be marked as MCP tool error for `{tool_name}`"
        );
        let structured = structured_content(&call);
        assert!(
            structured
                .get("status")
                .and_then(Value::as_u64)
                .is_some_and(|status| status >= 400),
            "expected invalid Norito transaction bytes to be rejected for `{tool_name}`"
        );
    }
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_post_transaction_accepts_hex_shortcut() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1131,
            "method": "tools/call",
            "params": {
                "name": "iroha.transactions.submit",
                "arguments": {
                    "signed_tx_hex": "01020304"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid signed transaction bytes from hex shortcut should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid Norito bytes via hex shortcut to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_connect_session_lifecycle_dispatches_routes() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.connect.enabled = true;

    let app = build_router(cfg);
    let sid = B64.encode([0x55u8; 32]);

    let (status, create_call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 108,
            "method": "tools/call",
            "params": {
                "name": "connect.session.create",
                "arguments": {
                    "sid": sid
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&create_call),
        "session creation should not be an MCP tool error"
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
    assert_eq!(
        create_body.get("sid").and_then(Value::as_str),
        Some(sid.as_str())
    );
    assert!(
        create_body
            .get("token_app")
            .and_then(Value::as_str)
            .is_some_and(|token| !token.is_empty()),
        "session create response should contain token_app"
    );
    assert!(
        create_body
            .get("token_wallet")
            .and_then(Value::as_str)
            .is_some_and(|token| !token.is_empty()),
        "session create response should contain token_wallet"
    );
    let token_app = create_body
        .get("token_app")
        .and_then(Value::as_str)
        .expect("token_app present")
        .to_owned();
    let token_wallet = create_body
        .get("token_wallet")
        .and_then(Value::as_str)
        .expect("token_wallet present")
        .to_owned();

    let (status, ws_ticket_app_call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 109,
            "method": "tools/call",
            "params": {
                "name": "connect.ws.ticket",
                "arguments": {
                    "sid": sid,
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
        !tool_is_error(&ws_ticket_app_call),
        "ticket generation should not be an MCP tool error"
    );
    let ws_ticket_app = structured_content(&ws_ticket_app_call);
    assert_eq!(
        ws_ticket_app.get("ws_url").and_then(Value::as_str),
        Some(
            "wss://node.example/v1/connect/ws?sid=VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVU&role=app"
        )
    );
    assert_eq!(
        ws_ticket_app
            .get("authorization_header")
            .and_then(Value::as_str),
        Some(format!("Bearer {token_app}").as_str())
    );
    assert_eq!(
        ws_ticket_app
            .get("sec_websocket_protocol")
            .and_then(Value::as_str),
        Some(
            format!(
                "iroha-connect.token.v1.{}",
                B64.encode(token_app.as_bytes())
            )
            .as_str()
        )
    );

    let (status, ws_ticket_wallet_call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 110,
            "method": "tools/call",
            "params": {
                "name": "connect.ws.ticket",
                "arguments": {
                    "sid": sid,
                    "role": "wallet",
                    "token_wallet": token_wallet,
                    "node_url": "https://node.example"
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&ws_ticket_wallet_call),
        "wallet ticket generation should not be an MCP tool error"
    );
    let ws_ticket_wallet = structured_content(&ws_ticket_wallet_call);
    assert_eq!(
        ws_ticket_wallet
            .get("authorization_header")
            .and_then(Value::as_str),
        Some(format!("Bearer {token_wallet}").as_str())
    );

    let (status, status_call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 111,
            "method": "tools/call",
            "params": {
                "name": "connect.status"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&status_call),
        "connect status should not be an MCP tool error"
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
            "id": 112,
            "method": "tools/call",
            "params": {
                "name": "connect.session.delete",
                "arguments": {
                    "sid": sid
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&delete_call),
        "successful delete should not be an MCP tool error"
    );
    let delete_structured = structured_content(&delete_call);
    assert_eq!(
        delete_structured.get("status").and_then(Value::as_u64),
        Some(204)
    );

    let (status, delete_again_call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 113,
            "method": "tools/call",
            "params": {
                "name": "connect.session.delete",
                "arguments": {
                    "sid": sid
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&delete_again_call),
        "404 delete should be marked as MCP tool error"
    );
    let delete_again_structured = structured_content(&delete_again_call);
    assert_eq!(
        delete_again_structured
            .get("status")
            .and_then(Value::as_u64),
        Some(404)
    );
}

#[tokio::test]
async fn mcp_jsonrpc_connect_alias_lifecycle_dispatches_routes() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
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
                    "sid": sid
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

    let (status, ticket_call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 115,
            "method": "tools/call",
            "params": {
                "name": "iroha.connect.ws.ticket",
                "arguments": {
                    "sid": sid,
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
                    "sid": sid
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
async fn mcp_routes_are_not_registered_when_disabled() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = false;

    let app = build_router(cfg);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/mcp")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
