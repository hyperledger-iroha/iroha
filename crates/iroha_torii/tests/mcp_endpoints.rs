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
    cfg.torii.mcp.rate_per_minute = Some(NonZeroU32::new(10_000).expect("nonzero rate"));
    cfg.torii.mcp.burst = Some(NonZeroU32::new(10_000).expect("nonzero burst"));

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
    assert_eq!(page2_tools.len(), 2);
    let connect_ticket_listed = list_all_tool_names(&app)
        .await
        .iter()
        .any(|name| name == "connect.ws.ticket");
    assert!(
        connect_ticket_listed,
        "connect.ws.ticket should be discoverable across paginated tools/list responses"
    );

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
async fn mcp_jsonrpc_tools_call_agent_alias_node_operational_endpoints_dispatch() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    for (id, tool_name) in [
        (1031, "iroha.health"),
        (1032, "iroha.status"),
        (1033, "iroha.parameters.get"),
        (1034, "iroha.node.capabilities"),
        (1035, "iroha.time.now"),
        (1036, "iroha.time.status"),
        (1037, "iroha.api.versions"),
    ] {
        let (status, call) = post_mcp(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": tool_name
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let structured = structured_content(&call);
        let http_status = structured.get("status").and_then(Value::as_u64);
        if tool_name == "iroha.status" {
            assert!(
                http_status.is_some(),
                "operational alias `{tool_name}` should return an HTTP status"
            );
            if tool_is_error(&call) {
                assert!(
                    http_status.is_some_and(|status| status >= 400),
                    "profile-restricted status alias should report an error HTTP status"
                );
            } else {
                assert_eq!(
                    http_status,
                    Some(200),
                    "status alias should return HTTP 200 when telemetry profile allows it"
                );
            }
        } else {
            assert!(
                !tool_is_error(&call),
                "operational alias `{tool_name}` should not be an MCP tool error"
            );
            assert_eq!(
                http_status,
                Some(200),
                "operational alias `{tool_name}` should return HTTP 200 in test harness"
            );
        }
    }
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_contract_post_endpoints_dispatch() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    for (id, tool_name) in [
        (10401, "iroha.contracts.code.register"),
        (10402, "iroha.contracts.deploy"),
        (10403, "iroha.contracts.instance.create"),
        (10404, "iroha.contracts.instance.activate"),
        (10405, "iroha.contracts.call"),
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
                        "body": {}
                    }
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let structured = structured_content(&call);
        assert!(
            structured.get("status").and_then(Value::as_u64).is_some(),
            "contract alias `{tool_name}` should dispatch and return an HTTP status"
        );
    }
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_contract_call_and_wait_surfaces_submit_error() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10408,
            "method": "tools/call",
            "params": {
                "name": "iroha.contracts.call_and_wait",
                "arguments": {
                    "body": {},
                    "timeout_ms": 1000,
                    "poll_interval_ms": 100
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid contract call payload should be surfaced as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected contract call-and-wait alias to surface submit HTTP error"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_contracts_code_get_accepts_hash_shortcut() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10406,
            "method": "tools/call",
            "params": {
                "name": "iroha.contracts.code.get",
                "arguments": {
                    "hash": "not-a-code-hash"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid code hash should be marked as MCP tool error for contract code detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid code hash to be rejected by contract code detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_contracts_state_get_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10407,
            "method": "tools/call",
            "params": {
                "name": "iroha.contracts.state.get",
                "arguments": {
                    "path": "k"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let structured = structured_content(&call);
    assert!(
        structured.get("status").and_then(Value::as_u64).is_some(),
        "contract state alias should dispatch and return an HTTP status"
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
        names.iter().any(|name| name == "iroha.health"),
        "expected agent-friendly node health MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.status"),
        "expected agent-friendly node status MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.parameters.get"),
        "expected agent-friendly parameters MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.node.capabilities"),
        "expected agent-friendly node capabilities MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.time.now"),
        "expected agent-friendly time-now MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.time.status"),
        "expected agent-friendly time-status MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.api.versions"),
        "expected agent-friendly api-versions MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.aliases.resolve"),
        "expected agent-friendly alias-resolve MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.aliases.resolve_index"),
        "expected agent-friendly alias-resolve-index MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.contracts.code.register"),
        "expected agent-friendly contract code registration MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.contracts.code.get"),
        "expected agent-friendly contract code detail MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.contracts.deploy"),
        "expected agent-friendly contract deploy MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.contracts.instance.create"),
        "expected agent-friendly contract instance create MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.contracts.instance.activate"),
        "expected agent-friendly contract instance activate MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.contracts.call"),
        "expected agent-friendly contract call MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.contracts.call_and_wait"),
        "expected agent-friendly contract call-and-wait MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.contracts.state.get"),
        "expected agent-friendly contract state MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.accounts.list"),
        "expected agent-friendly account listing MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.accounts.get"),
        "expected agent-friendly account detail MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.accounts.qr"),
        "expected agent-friendly account QR MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.accounts.query"),
        "expected agent-friendly account query MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.accounts.onboard"),
        "expected agent-friendly account onboarding MCP tool"
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
        names.iter().any(|name| name == "iroha.accounts.portfolio"),
        "expected agent-friendly account portfolio MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.domains.list"),
        "expected agent-friendly domains list MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.domains.get"),
        "expected agent-friendly domains detail MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.domains.query"),
        "expected agent-friendly domains query MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.subscriptions.plans.list"),
        "expected agent-friendly subscription plans list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.subscriptions.plans.create"),
        "expected agent-friendly subscription plans create MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.subscriptions.list"),
        "expected agent-friendly subscriptions list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.subscriptions.create"),
        "expected agent-friendly subscription create MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.subscriptions.get"),
        "expected agent-friendly subscription detail MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.subscriptions.cancel"),
        "expected agent-friendly subscription cancel MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.subscriptions.pause"),
        "expected agent-friendly subscription pause MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.subscriptions.resume"),
        "expected agent-friendly subscription resume MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.subscriptions.keep"),
        "expected agent-friendly subscription keep MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.subscriptions.usage"),
        "expected agent-friendly subscription usage MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.subscriptions.charge_now"),
        "expected agent-friendly subscription charge-now MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.assets.definitions"),
        "expected agent-friendly asset definitions MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.assets.definitions.get"),
        "expected agent-friendly asset definitions detail MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.assets.definitions.query"),
        "expected agent-friendly asset definitions query MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.assets.holders"),
        "expected agent-friendly asset holders MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.assets.holders.query"),
        "expected agent-friendly asset holders query MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.assets.list"),
        "expected agent-friendly asset list MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.assets.get"),
        "expected agent-friendly asset detail MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.nfts.list"),
        "expected agent-friendly nft list MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.nfts.get"),
        "expected agent-friendly nft detail MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.nfts.query"),
        "expected agent-friendly nft query MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.transfers.list"),
        "expected agent-friendly offline transfer list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.transfers.get"),
        "expected agent-friendly offline transfer detail MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.transfers.query"),
        "expected agent-friendly offline transfer query MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.settlements.list"),
        "expected agent-friendly offline settlement list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.settlements.get"),
        "expected agent-friendly offline settlement detail MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.settlements.query"),
        "expected agent-friendly offline settlement query MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.settlements.submit"),
        "expected agent-friendly offline settlement submit MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.certificates.list"),
        "expected agent-friendly offline certificate list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.certificates.get"),
        "expected agent-friendly offline certificate detail MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.certificates.query"),
        "expected agent-friendly offline certificate query MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.certificates.issue"),
        "expected agent-friendly offline certificate issue MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.certificates.renew"),
        "expected agent-friendly offline certificate renew MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.certificates.renew_issue"),
        "expected agent-friendly offline certificate renew-issue MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.certificates.revoke"),
        "expected agent-friendly offline certificate revoke MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.allowances.get"),
        "expected agent-friendly offline allowance detail MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.allowances.issue"),
        "expected agent-friendly offline allowance issue MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.allowances.renew"),
        "expected agent-friendly offline allowance renew MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.allowances.list"),
        "expected agent-friendly offline allowance list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.allowances.query"),
        "expected agent-friendly offline allowance query MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.receipts.list"),
        "expected agent-friendly offline receipt list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.receipts.query"),
        "expected agent-friendly offline receipt query MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.revocations.list"),
        "expected agent-friendly offline revocations list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.revocations.query"),
        "expected agent-friendly offline revocations query MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.transfers.proof"),
        "expected agent-friendly offline transfer proof MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.spend_receipts.submit"),
        "expected agent-friendly offline spend receipts submit MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.offline.state"),
        "expected agent-friendly offline state MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.bundle.proof_status"),
        "expected agent-friendly offline bundle proof status MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.rejections.list"),
        "expected agent-friendly offline rejections MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.summaries.list"),
        "expected agent-friendly offline summaries list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.offline.summaries.query"),
        "expected agent-friendly offline summaries query MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.queries.submit"),
        "expected agent-friendly signed query submit MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.transactions.submit"),
        "expected agent-friendly transaction submit MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.transactions.submit_and_wait"),
        "expected agent-friendly transaction submit-and-wait MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.transactions.wait"),
        "expected agent-friendly transaction wait MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.transactions.status"),
        "expected agent-friendly transaction status MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.transactions.list"),
        "expected agent-friendly transaction list MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.transactions.get"),
        "expected agent-friendly transaction detail MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.instructions.list"),
        "expected agent-friendly instruction list MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.instructions.get"),
        "expected agent-friendly instruction detail MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.blocks.list"),
        "expected agent-friendly block list MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.blocks.get"),
        "expected agent-friendly block detail MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.connect.session.create"),
        "expected agent-friendly connect session create MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.connect.session.create_and_ticket"),
        "expected agent-friendly connect session create-and-ticket MCP tool"
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
async fn mcp_jsonrpc_tools_call_agent_alias_accounts_get_accepts_flat_account_id() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1051,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.get",
                "arguments": {
                    "account_id": "not-an-account-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid account id should be marked as MCP tool error for account detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid account id to be rejected by explorer account detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_accounts_qr_accepts_flat_account_id() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1052,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.qr",
                "arguments": {
                    "account_id": "not-an-account-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid account id should be marked as MCP tool error for account QR alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid account id to be rejected by explorer account QR alias"
    );
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
async fn mcp_jsonrpc_tools_call_agent_alias_transaction_wait_accepts_flat_hash() {
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
                "name": "iroha.transactions.wait",
                "arguments": {
                    "hash": "not-a-hash",
                    "timeout_ms": 1000,
                    "poll_interval_ms": 100
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid flat hash should be marked as MCP tool error for transaction wait alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid flat hash to be rejected by transaction wait alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_transactions_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10611,
            "method": "tools/call",
            "params": {
                "name": "iroha.transactions.list",
                "arguments": {
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "transactions list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_transactions_get_accepts_flat_hash() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10612,
            "method": "tools/call",
            "params": {
                "name": "iroha.transactions.get",
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
        "invalid hash should be marked as MCP tool error for transaction detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid transaction hash to be rejected by explorer detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_instructions_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10613,
            "method": "tools/call",
            "params": {
                "name": "iroha.instructions.list",
                "arguments": {
                    "page": 1
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "instructions list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_instructions_get_accepts_flat_hash_and_index() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10614,
            "method": "tools/call",
            "params": {
                "name": "iroha.instructions.get",
                "arguments": {
                    "hash": "not-a-hash",
                    "index": 0
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid hash should be marked as MCP tool error for instruction detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid instruction hash to be rejected by explorer detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_instructions_get_accepts_alias_shortcuts() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10615,
            "method": "tools/call",
            "params": {
                "name": "iroha.instructions.get",
                "arguments": {
                    "transaction_hash": "not-a-hash",
                    "instruction_index": 1
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid hash should be marked as MCP tool error for instruction alias shortcuts"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid transaction hash alias to be rejected by explorer detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_assets_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106151,
            "method": "tools/call",
            "params": {
                "name": "iroha.assets.list",
                "arguments": {
                    "page": 1
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "assets list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_assets_get_accepts_flat_asset_id() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106152,
            "method": "tools/call",
            "params": {
                "name": "iroha.assets.get",
                "arguments": {
                    "asset_id": "not-an-asset-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid asset id should be marked as MCP tool error for asset detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid asset id to be rejected by explorer asset detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_nfts_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106153,
            "method": "tools/call",
            "params": {
                "name": "iroha.nfts.list",
                "arguments": {
                    "page": 1
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "nfts list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_nfts_get_accepts_flat_nft_id() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106154,
            "method": "tools/call",
            "params": {
                "name": "iroha.nfts.get",
                "arguments": {
                    "nft_id": "not-an-nft-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid nft id should be marked as MCP tool error for nft detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid nft id to be rejected by explorer nft detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_nfts_query_accepts_flat_envelope_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106155,
            "method": "tools/call",
            "params": {
                "name": "iroha.nfts.query",
                "arguments": {
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "nfts query alias with flat envelope fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_transfers_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106171,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.transfers.list",
                "arguments": {
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline transfer list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_transfers_get_accepts_bundle_shortcut() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106172,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.transfers.get",
                "arguments": {
                    "bundle_id": "not-a-hex-bundle-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid bundle id should be marked as MCP tool error for offline transfer detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid offline bundle id to be rejected by transfer detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_transfers_query_accepts_flat_envelope_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106173,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.transfers.query",
                "arguments": {
                    "query": "FindAll",
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline transfer query alias with flat envelope fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_settlements_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106174,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.settlements.list",
                "arguments": {
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline settlement list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_settlements_get_accepts_bundle_shortcut() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106175,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.settlements.get",
                "arguments": {
                    "bundle_id": "not-a-hex-bundle-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid bundle id should be marked as MCP tool error for offline settlement detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid offline settlement bundle id to be rejected by detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_settlements_query_accepts_flat_envelope_fields()
{
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106176,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.settlements.query",
                "arguments": {
                    "query": "FindAll",
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline settlement query alias with flat envelope fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_settlements_submit_accepts_flat_body_shortcuts()
{
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106177,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.settlements.submit",
                "arguments": {
                    "authority": "alice@wonderland"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid settlement submit payload should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected settlement submit alias to surface HTTP validation errors"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_certificates_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106185,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.certificates.list",
                "arguments": {
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline certificate list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_certificates_get_accepts_certificate_shortcut()
{
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106186,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.certificates.get",
                "arguments": {
                    "certificate_id": "not-a-certificate-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid certificate id should be marked as MCP tool error for offline certificate detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid certificate id to be rejected by offline certificate detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_certificates_query_accepts_flat_envelope_fields()
 {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106187,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.certificates.query",
                "arguments": {
                    "query": "FindAll",
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline certificate query alias with flat envelope fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_certificates_issue_accepts_flat_body_shortcuts()
{
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106188,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.certificates.issue",
                "arguments": {
                    "authority": "alice@wonderland"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid certificate issue payload should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected certificate issue alias to surface HTTP validation errors"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_certificates_renew_accepts_shortcuts() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106189,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.certificates.renew",
                "arguments": {
                    "certificate_id": "not-a-certificate-id",
                    "authority": "alice@wonderland"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid certificate renew payload should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected certificate renew alias to surface HTTP validation errors"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_certificates_renew_issue_accepts_shortcuts() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106190,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.certificates.renew_issue",
                "arguments": {
                    "certificate_id": "not-a-certificate-id",
                    "authority": "alice@wonderland"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid certificate renew-issue payload should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected certificate renew-issue alias to surface HTTP validation errors"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_certificates_revoke_accepts_flat_body_shortcuts()
 {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106191,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.certificates.revoke",
                "arguments": {
                    "certificate_id": "not-a-certificate-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid certificate revoke payload should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected certificate revoke alias to surface HTTP validation errors"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_allowances_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106178,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.allowances.list",
                "arguments": {
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline allowance list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_allowances_get_accepts_certificate_shortcut() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106182,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.allowances.get",
                "arguments": {
                    "certificate_id": "not-a-certificate-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid certificate id should be marked as MCP tool error for offline allowance detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid certificate id to be rejected by offline allowance detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_allowances_issue_accepts_flat_body_shortcuts() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106183,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.allowances.issue",
                "arguments": {
                    "authority": "alice@wonderland"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid allowance issue payload should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected allowance issue alias to surface HTTP validation errors"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_allowances_renew_accepts_shortcuts() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106184,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.allowances.renew",
                "arguments": {
                    "certificate_id": "not-a-certificate-id",
                    "authority": "alice@wonderland"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid allowance renew payload should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected allowance renew alias to surface HTTP validation errors"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_allowances_query_accepts_flat_envelope_fields()
{
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106179,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.allowances.query",
                "arguments": {
                    "query": "FindAll",
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline allowance query alias with flat envelope fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_receipts_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106180,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.receipts.list",
                "arguments": {
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline receipt list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_receipts_query_accepts_flat_envelope_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106181,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.receipts.query",
                "arguments": {
                    "query": "FindAll",
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline receipt query alias with flat envelope fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_revocations_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106192,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.revocations.list",
                "arguments": {
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline revocations list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_revocations_query_accepts_flat_envelope_fields()
{
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106193,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.revocations.query",
                "arguments": {
                    "query": "FindAll",
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline revocations query alias with flat envelope fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_transfers_proof_accepts_flat_body_shortcuts() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106194,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.transfers.proof",
                "arguments": {
                    "kind": "sum"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid transfer proof payload should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected transfer proof alias to surface HTTP validation errors"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_spend_receipts_submit_accepts_flat_body_shortcuts()
 {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106195,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.spend_receipts.submit",
                "arguments": {
                    "receipts": "not-an-array"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid spend receipts payload should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected spend receipts alias to surface HTTP validation errors"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_state_dispatches_route() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106196,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.state"
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline state alias should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_bundle_proof_status_accepts_bundle_shortcut() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106197,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.bundle.proof_status",
                "arguments": {
                    "bundle_id": "not-a-bundle-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid bundle id should be marked as MCP tool error for bundle proof status alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid bundle id to be rejected by bundle proof status alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_rejections_list_dispatches_route() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106198,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.rejections.list"
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let structured = structured_content(&call);
    let http_status = structured.get("status").and_then(Value::as_u64);
    assert!(
        http_status.is_some(),
        "offline rejections alias should return an HTTP status"
    );
    if tool_is_error(&call) {
        assert!(
            http_status.is_some_and(|status| status >= 400),
            "offline rejections alias should report an error HTTP status when unavailable"
        );
    } else {
        assert_eq!(
            http_status,
            Some(200),
            "offline rejections alias should return HTTP 200 when available"
        );
    }
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_summaries_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106199,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.summaries.list",
                "arguments": {
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline summaries list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_offline_summaries_query_accepts_flat_envelope_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106200,
            "method": "tools/call",
            "params": {
                "name": "iroha.offline.summaries.query",
                "arguments": {
                    "query": "FindAll",
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "offline summaries query alias with flat envelope fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_blocks_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10616,
            "method": "tools/call",
            "params": {
                "name": "iroha.blocks.list",
                "arguments": {
                    "page": 1
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "blocks list alias with flat query fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_blocks_get_accepts_height_alias() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10617,
            "method": "tools/call",
            "params": {
                "name": "iroha.blocks.get",
                "arguments": {
                    "block_height": 0
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid block height should be marked as MCP tool error for block detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid block-height alias to be rejected by explorer block detail alias"
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
async fn mcp_jsonrpc_tools_call_agent_alias_accounts_onboard_accepts_shortcuts() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106201,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.onboard",
                "arguments": {
                    "alias": "agent-alice",
                    "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "onboarding alias should produce MCP tool error when onboarding is unavailable"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected onboarding alias dispatch to return a route error status"
    );
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
async fn mcp_jsonrpc_tools_call_agent_alias_account_portfolio_accepts_flat_uaid() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106221,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.portfolio",
                "arguments": {
                    "uaid": "uaid:00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "portfolio alias with flat uaid should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_domains_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106222,
            "method": "tools/call",
            "params": {
                "name": "iroha.domains.list",
                "arguments": {
                    "limit": 0
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid flat domain-list limit should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid flat domain-list limit to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_domains_get_accepts_flat_domain_id() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1062221,
            "method": "tools/call",
            "params": {
                "name": "iroha.domains.get",
                "arguments": {
                    "domain_id": "not-a-domain-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid domain id should be marked as MCP tool error for domain detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid domain id to be rejected by explorer domain detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_domains_query_accepts_flat_envelope_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106223,
            "method": "tools/call",
            "params": {
                "name": "iroha.domains.query",
                "arguments": {
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "domains query alias with flat envelope fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_subscriptions_plans_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1062231,
            "method": "tools/call",
            "params": {
                "name": "iroha.subscriptions.plans.list",
                "arguments": {
                    "limit": 1
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let structured = structured_content(&call);
    assert!(
        structured.get("status").and_then(Value::as_u64).is_some(),
        "subscriptions plans list alias should dispatch and return a status code"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_subscriptions_plans_create_accepts_body() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10622315,
            "method": "tools/call",
            "params": {
                "name": "iroha.subscriptions.plans.create",
                "arguments": {
                    "body": {}
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let structured = structured_content(&call);
    assert!(
        structured.get("status").and_then(Value::as_u64).is_some(),
        "subscriptions plans create alias should dispatch and return a status code"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_subscriptions_list_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1062232,
            "method": "tools/call",
            "params": {
                "name": "iroha.subscriptions.list",
                "arguments": {
                    "limit": 1
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let structured = structured_content(&call);
    assert!(
        structured.get("status").and_then(Value::as_u64).is_some(),
        "subscriptions list alias should dispatch and return a status code"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_subscriptions_create_accepts_body() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10622325,
            "method": "tools/call",
            "params": {
                "name": "iroha.subscriptions.create",
                "arguments": {
                    "body": {}
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let structured = structured_content(&call);
    assert!(
        structured.get("status").and_then(Value::as_u64).is_some(),
        "subscriptions create alias should dispatch and return a status code"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_subscriptions_get_accepts_flat_subscription_id() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1062233,
            "method": "tools/call",
            "params": {
                "name": "iroha.subscriptions.get",
                "arguments": {
                    "subscription_id": "not-a-subscription-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid subscription id should be marked as MCP tool error for subscription detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid subscription id to be rejected by subscription detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_subscriptions_cancel_accepts_flat_subscription_id() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10622335,
            "method": "tools/call",
            "params": {
                "name": "iroha.subscriptions.cancel",
                "arguments": {
                    "subscription_id": "not-a-subscription-id",
                    "body": {}
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid subscription id should be marked as MCP tool error for subscription cancel alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid subscription id to be rejected by subscription cancel alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_subscription_actions_accept_flat_subscription_id() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    for (id, tool_name) in [
        (10622341, "iroha.subscriptions.pause"),
        (10622342, "iroha.subscriptions.resume"),
        (10622343, "iroha.subscriptions.keep"),
        (10622344, "iroha.subscriptions.usage"),
        (10622345, "iroha.subscriptions.charge_now"),
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
                        "subscription_id": "not-a-subscription-id",
                        "body": {}
                    }
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            tool_is_error(&call),
            "invalid subscription id should be marked as MCP tool error for `{tool_name}`"
        );
        let structured = structured_content(&call);
        assert!(
            structured
                .get("status")
                .and_then(Value::as_u64)
                .is_some_and(|status| status >= 400),
            "expected invalid subscription id to be rejected by `{tool_name}`"
        );
    }
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_asset_definitions_accepts_flat_query_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106224,
            "method": "tools/call",
            "params": {
                "name": "iroha.assets.definitions",
                "arguments": {
                    "limit": 0
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid flat asset-definitions limit should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid flat asset-definitions limit to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_asset_definitions_get_accepts_flat_definition_id() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1062241,
            "method": "tools/call",
            "params": {
                "name": "iroha.assets.definitions.get",
                "arguments": {
                    "definition_id": "not-a-definition-id"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid definition id should be marked as MCP tool error for definition detail alias"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid definition id to be rejected by explorer definition detail alias"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_asset_definitions_query_accepts_flat_envelope_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106225,
            "method": "tools/call",
            "params": {
                "name": "iroha.assets.definitions.query",
                "arguments": {
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "asset definitions query alias with flat envelope fields should dispatch successfully"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_asset_holders_accepts_flat_arguments() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106226,
            "method": "tools/call",
            "params": {
                "name": "iroha.assets.holders",
                "arguments": {
                    "definition_id": "rose#wonderland",
                    "limit": 0
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid flat asset-holders limit should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid flat asset-holders limit to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_asset_holders_query_accepts_flat_arguments() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106227,
            "method": "tools/call",
            "params": {
                "name": "iroha.assets.holders.query",
                "arguments": {
                    "definition_id": "rose#wonderland",
                    "limit": 2
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "asset holders query alias with flat arguments should dispatch successfully"
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
async fn mcp_jsonrpc_tools_call_agent_aliases_resolve_accepts_flat_alias_shortcut() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10631,
            "method": "tools/call",
            "params": {
                "name": "iroha.aliases.resolve",
                "arguments": {
                    "alias": "missing-alias"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let structured = structured_content(&call);
    let http_status = structured.get("status").and_then(Value::as_u64);
    assert!(
        http_status.is_some(),
        "alias resolve alias should return an HTTP status"
    );
    if tool_is_error(&call) {
        assert!(
            http_status.is_some_and(|status| status >= 400),
            "alias resolve alias should surface non-2xx errors when alias is missing/unavailable"
        );
    } else {
        assert_eq!(
            http_status,
            Some(200),
            "alias resolve alias should return HTTP 200 when alias lookup succeeds"
        );
    }
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_aliases_resolve_index_accepts_flat_index_shortcut() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10632,
            "method": "tools/call",
            "params": {
                "name": "iroha.aliases.resolve_index",
                "arguments": {
                    "index": 0
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let structured = structured_content(&call);
    let http_status = structured.get("status").and_then(Value::as_u64);
    assert!(
        http_status.is_some(),
        "alias resolve-index alias should return an HTTP status"
    );
    if tool_is_error(&call) {
        assert!(
            http_status.is_some_and(|status| status >= 400),
            "alias resolve-index alias should surface non-2xx errors when index is missing/unavailable"
        );
    } else {
        assert_eq!(
            http_status,
            Some(200),
            "alias resolve-index alias should return HTTP 200 when alias index lookup succeeds"
        );
    }
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
async fn mcp_jsonrpc_tools_call_agent_alias_query_submit_accepts_base64_shortcut() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1133,
            "method": "tools/call",
            "params": {
                "name": "iroha.queries.submit",
                "arguments": {
                    "query_base64": "AQIDBA=="
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid signed query bytes should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid Norito query bytes to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_query_submit_accepts_hex_shortcut() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1134,
            "method": "tools/call",
            "params": {
                "name": "iroha.queries.submit",
                "arguments": {
                    "signed_query_hex": "01020304"
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid signed query bytes from hex shortcut should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid Norito query bytes via hex shortcut to be rejected"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_submit_and_wait_surfaces_submit_error() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;

    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1132,
            "method": "tools/call",
            "params": {
                "name": "iroha.transactions.submit_and_wait",
                "arguments": {
                    "signed_tx_hex": "01020304",
                    "timeout_ms": 1000,
                    "poll_interval_ms": 100
                }
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        tool_is_error(&call),
        "invalid signed transaction bytes should be surfaced as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected submit+wait alias to surface submit HTTP error status"
    );
}

#[tokio::test]
async fn mcp_jsonrpc_connect_session_create_and_ticket_dispatches_routes() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.connect.enabled = true;

    let app = build_router(cfg);
    for (id, tool_name, sid_bytes, role, token_key) in [
        (
            1078,
            "connect.session.create_and_ticket",
            [0x77u8; 32],
            "app",
            "token_app",
        ),
        (
            1079,
            "iroha.connect.session.create_and_ticket",
            [0x78u8; 32],
            "wallet",
            "token_wallet",
        ),
    ] {
        let sid = B64.encode(sid_bytes);
        let (status, call) = post_mcp(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": {
                        "sid": sid,
                        "role": role,
                        "node_url": "https://node.example"
                    }
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            !tool_is_error(&call),
            "create-and-ticket alias `{tool_name}` should not be an MCP tool error"
        );
        let structured = structured_content(&call);
        assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
        assert_eq!(
            structured.get("sid").and_then(Value::as_str),
            Some(sid.as_str())
        );
        assert_eq!(structured.get("role").and_then(Value::as_str), Some(role));

        let create = structured
            .get("create")
            .and_then(Value::as_object)
            .expect("create response");
        assert_eq!(create.get("status").and_then(Value::as_u64), Some(200));
        let create_body = create
            .get("body")
            .and_then(Value::as_object)
            .expect("create response body");
        let token = create_body
            .get(token_key)
            .and_then(Value::as_str)
            .expect("role token in create response");

        let ticket = structured
            .get("ticket")
            .and_then(Value::as_object)
            .expect("ticket payload");
        assert_eq!(
            ticket.get("ws_url").and_then(Value::as_str),
            Some(format!("wss://node.example/v1/connect/ws?sid={sid}&role={role}").as_str())
        );
        assert_eq!(
            ticket.get("authorization_header").and_then(Value::as_str),
            Some(format!("Bearer {token}").as_str())
        );
    }
}

#[tokio::test]
async fn mcp_jsonrpc_connect_session_create_generates_sid_when_omitted() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.connect.enabled = true;

    let app = build_router(cfg);
    for (id, tool_name) in [
        (2080, "connect.session.create"),
        (2081, "iroha.connect.session.create"),
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
                        "node_url": "https://node.example"
                    }
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            !tool_is_error(&call),
            "create alias `{tool_name}` should auto-generate sid"
        );
        let structured = structured_content(&call);
        assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
        let body = structured
            .get("body")
            .and_then(Value::as_object)
            .expect("create response body");
        let sid = body
            .get("sid")
            .and_then(Value::as_str)
            .expect("generated sid");
        let sid_bytes = B64.decode(sid).expect("base64url sid");
        assert_eq!(sid_bytes.len(), 32, "sid should be 32 bytes");
    }
}

#[tokio::test]
async fn mcp_jsonrpc_connect_session_create_and_ticket_generates_sid_when_omitted() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.connect.enabled = true;

    let app = build_router(cfg);
    for (id, tool_name, role) in [
        (2082, "connect.session.create_and_ticket", "app"),
        (2083, "iroha.connect.session.create_and_ticket", "wallet"),
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
                        "role": role,
                        "node_url": "https://node.example"
                    }
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            !tool_is_error(&call),
            "create-and-ticket alias `{tool_name}` should auto-generate sid"
        );
        let structured = structured_content(&call);
        let sid = structured
            .get("sid")
            .and_then(Value::as_str)
            .expect("generated sid");
        assert_eq!(B64.decode(sid).expect("base64url sid").len(), 32);
        assert_eq!(structured.get("role").and_then(Value::as_str), Some(role));
        let ticket = structured
            .get("ticket")
            .and_then(Value::as_object)
            .expect("ticket payload");
        assert_eq!(
            ticket.get("ws_url").and_then(Value::as_str),
            Some(format!("wss://node.example/v1/connect/ws?sid={sid}&role={role}").as_str())
        );
    }
}

#[tokio::test]
async fn mcp_jsonrpc_connect_session_create_and_ticket_surfaces_create_error() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.connect.enabled = true;

    let app = build_router(cfg);
    for (id, tool_name) in [
        (1080, "connect.session.create_and_ticket"),
        (1081, "iroha.connect.session.create_and_ticket"),
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
                        "sid": "not_base64url",
                        "role": "app",
                        "node_url": "https://node.example"
                    }
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(
            tool_is_error(&call),
            "create-and-ticket alias `{tool_name}` should surface create errors"
        );
        let structured = structured_content(&call);
        assert!(
            structured
                .get("status")
                .and_then(Value::as_u64)
                .is_some_and(|status| status >= 400),
            "expected create error status to be forwarded unchanged"
        );
        assert!(
            structured.get("ticket").is_none(),
            "error response should be raw create response without ticket payload"
        );
    }
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
