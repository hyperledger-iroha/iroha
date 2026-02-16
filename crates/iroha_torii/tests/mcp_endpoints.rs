#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration coverage for Torii MCP endpoints.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
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
