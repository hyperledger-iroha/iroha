#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration coverage for Torii MCP endpoints.
use axum::{
    body::{Body, Bytes},
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
use iroha_data_model::{
    isi::musubi::SetMusubiReleaseYankV1,
    musubi::{MusubiPackageIdV1, MusubiPackageScopeV1, MusubiReleaseIdV1},
    nexus::DataSpaceId,
};
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, test_utils};
use norito::json::Value;
use std::{collections::BTreeSet, net::SocketAddr, num::NonZeroU32, sync::Arc, time::Duration};
use tower::ServiceExt as _;
const TEST_ACCOUNT_I105: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";
const TOOL_LIST_PAGE_LIMIT: usize = 128;
fn build_router(cfg: iroha_config::parameters::actual::Root) -> axum::Router {
    let (kiso, _child) = KisoHandle::start(cfg.clone());
    let kura = Kura::blank_kura_for_testing();
    let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
        World::default(),
        kura.clone(),
        LiveQueryStore::start_test(),
        cfg.common.chain.clone(),
        iroha_data_model::NetworkId::from_genesis_hash(cfg.genesis.expected_hash),
    ));
    let queue_cfg = iroha_config::parameters::actual::Queue::default();
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(queue_cfg, events_sender));
    let (_peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    let torii = Torii::new_with_handle(
        cfg.common.chain.clone(),
        *state.network_id_ref(),
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
    norito::json::from_slice(&bytes).unwrap_or_else(|err| {
        panic!(
            "valid json body: {err}; raw_body={}",
            String::from_utf8_lossy(&bytes)
        )
    })
}
async fn read_body_bytes(response: axum::response::Response) -> Bytes {
    response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes()
}
async fn call_app(app: &axum::Router, request: Request<Body>) -> axum::response::Response {
    let service = app
        .clone()
        .into_make_service_with_connect_info::<SocketAddr>()
        .oneshot(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("mcp make service");
    service.oneshot(request).await.expect("mcp response")
}
async fn post_mcp(app: &axum::Router, payload: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/v1/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-06-18")
        .body(Body::from(
            norito::json::to_vec(&payload).expect("serialize payload"),
        ))
        .expect("valid request");
    let response = call_app(app, request).await;
    let status = response.status();
    let body = read_json_body(response).await;
    (status, body)
}
async fn post_mcp_bytes(app: &axum::Router, payload: Value) -> (StatusCode, Bytes) {
    let request = Request::builder()
        .method("POST")
        .uri("/v1/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-06-18")
        .body(Body::from(
            norito::json::to_vec(&payload).expect("serialize payload"),
        ))
        .expect("valid request");
    let response = call_app(app, request).await;
    let status = response.status();
    let body = read_body_bytes(response).await;
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
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream");
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("MCP-Protocol-Version"))
    {
        builder = builder.header("MCP-Protocol-Version", "2025-06-18");
    }
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let request = builder
        .body(Body::from(
            norito::json::to_vec(&payload).expect("serialize payload"),
        ))
        .expect("valid request");
    let response = call_app(app, request).await;
    let status = response.status();
    let body = read_json_body(response).await;
    (status, body)
}
fn initialize_params() -> Value {
    norito::json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": {
            "name": "iroha-torii-mcp-integration-tests",
            "version": "1.0.0"
        }
    })
}
fn initialize_request(id: u64) -> Value {
    norito::json!({
        "jsonrpc": "2.0",
        "id": (id),
        "method": "initialize",
        "params": (initialize_params())
    })
}
fn assert_single_invalid_request(body: &Value) {
    assert!(
        body.is_object(),
        "expected one JSON-RPC error, got {body:?}"
    );
    assert_eq!(body.get("id"), Some(&Value::Null));
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_i64),
        Some(-32600)
    );
}
async fn list_all_tool_names(app: &axum::Router) -> Vec<String> {
    let mut cursor: Option<String> = None;
    let mut seen_cursors = BTreeSet::new();
    let mut names = Vec::new();
    for _ in 0..TOOL_LIST_PAGE_LIMIT {
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
        let next_cursor = checked_next_tools_cursor(result, cursor.as_deref(), &mut seen_cursors);
        if next_cursor.is_none() {
            return names;
        }
        cursor = next_cursor;
    }
    panic!("tools/list pagination exceeded {TOOL_LIST_PAGE_LIMIT} pages");
}
async fn list_all_tools(app: &axum::Router) -> Vec<Value> {
    let mut cursor: Option<String> = None;
    let mut seen_cursors = BTreeSet::new();
    let mut tools = Vec::new();
    for _ in 0..TOOL_LIST_PAGE_LIMIT {
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
        tools.extend(page.iter().cloned());
        let next_cursor = checked_next_tools_cursor(result, cursor.as_deref(), &mut seen_cursors);
        if next_cursor.is_none() {
            return tools;
        }
        cursor = next_cursor;
    }
    panic!("tools/list pagination exceeded {TOOL_LIST_PAGE_LIMIT} pages");
}
async fn find_tool(app: &axum::Router, target_name: &str) -> Value {
    let mut cursor: Option<String> = None;
    let mut seen_cursors = BTreeSet::new();
    for _ in 0..TOOL_LIST_PAGE_LIMIT {
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
            if tool.get("name").and_then(Value::as_str) == Some(target_name) {
                return tool.clone();
            }
        }
        let next_cursor = checked_next_tools_cursor(result, cursor.as_deref(), &mut seen_cursors);
        if next_cursor.is_none() {
            panic!("tool {target_name} not found in tools/list");
        }
        cursor = next_cursor;
    }
    panic!(
        "tools/list pagination exceeded {TOOL_LIST_PAGE_LIMIT} pages while searching for {target_name}"
    );
}
fn checked_next_tools_cursor(
    result: &Value,
    requested_cursor: Option<&str>,
    seen_cursors: &mut BTreeSet<String>,
) -> Option<String> {
    let next_cursor = result
        .get("nextCursor")
        .and_then(Value::as_str)
        .map(str::to_owned)?;
    assert_ne!(
        requested_cursor,
        Some(next_cursor.as_str()),
        "tools/list returned an unchanged nextCursor"
    );
    assert!(
        seen_cursors.insert(next_cursor.clone()),
        "tools/list repeated cursor {next_cursor}"
    );
    Some(next_cursor)
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
fn assert_tool_error(response: &Value, context: &str) {
    let result = response
        .get("result")
        .unwrap_or_else(|| panic!("{context}: expected MCP result, got {response:?}"));
    let is_error = result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("{context}: expected MCP isError flag, got {response:?}"));
    assert!(
        is_error,
        "{context}: expected MCP tool error, got {response:?}"
    );
}
fn assert_tool_schema_error<'a>(response: &'a Value, context: &str) -> &'a str {
    assert_eq!(
        response.pointer("/error/code").and_then(Value::as_i64),
        Some(-32602),
        "{context}: expected JSON-RPC invalid params, got {response:?}"
    );
    assert_eq!(
        response
            .pointer("/error/data/error_code")
            .and_then(Value::as_str),
        Some("tool_schema_validation_failed"),
        "{context}: expected advertised-schema rejection, got {response:?}"
    );
    response
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{context}: expected schema error message, got {response:?}"))
}

#[derive(Clone, Copy)]
enum McpAliasDispatchArguments {
    InvalidAccountId,
    InvalidPathHash,
    InvalidStatusHash,
    InvalidAssetId,
    InvalidNftId,
    InvalidRwaId,
    InvalidDomainId,
    InvalidSubscriptionId,
    InvalidDefinitionId,
    LimitTwo,
    PageOne,
}
impl McpAliasDispatchArguments {
    fn into_json(self) -> Value {
        match self {
            Self::InvalidAccountId => {
                norito::json!({"path": {"account_id": "not-an-account-id"}})
            }
            Self::InvalidPathHash => norito::json!({"path": {"hash": "not-a-hash"}}),
            Self::InvalidStatusHash => norito::json!({"query": {"hash": "not-a-hash"}}),
            Self::InvalidAssetId => {
                norito::json!({"path": {"asset_id": "not-an-asset-id"}})
            }
            Self::InvalidNftId => norito::json!({"path": {"nft_id": "not-an-nft-id"}}),
            Self::InvalidRwaId => norito::json!({"path": {"rwa_id": "not-a-rwa-id"}}),
            Self::InvalidDomainId => {
                norito::json!({"path": {"domain_id": "not-a-domain-id"}})
            }
            Self::InvalidSubscriptionId => {
                norito::json!({"path": {"subscription_id": "not-a-subscription-id"}})
            }
            Self::InvalidDefinitionId => {
                norito::json!({"path": {"definition_id": "not-a-definition-id"}})
            }
            Self::LimitTwo => norito::json!({"limit": 2}),
            Self::PageOne => norito::json!({"page": 1}),
        }
    }
}
#[derive(Clone, Copy)]
enum McpAliasDispatchExpectation {
    ToolError {
        context: &'static str,
        status_context: &'static str,
    },
    Success {
        context: &'static str,
    },
    SchemaError {
        context: &'static str,
    },
}
#[derive(Clone, Copy)]
struct McpAliasDispatchCase {
    request_id: u64,
    tool_name: &'static str,
    arguments: McpAliasDispatchArguments,
    expectation: McpAliasDispatchExpectation,
}
async fn assert_mcp_alias_dispatch(case: McpAliasDispatchCase) {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    enable_writer_mcp(&mut cfg);
    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": { case.request_id },
            "method": "tools/call",
            "params": {
                "name": { case.tool_name },
                "arguments": { case.arguments.into_json() }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    match case.expectation {
        McpAliasDispatchExpectation::ToolError {
            context,
            status_context,
        } => {
            assert_tool_error(&call, context);
            let structured = structured_content(&call);
            assert!(
                structured
                    .get("status")
                    .and_then(Value::as_u64)
                    .is_some_and(|status| status >= 400)
                    || structured.get("code").and_then(Value::as_str) == Some(status_context),
                "{context}: {status_context}"
            );
        }
        McpAliasDispatchExpectation::Success { context } => {
            assert!(!tool_is_error(&call), "{context}");
            let structured = structured_content(&call);
            assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
        }
        McpAliasDispatchExpectation::SchemaError { context } => {
            assert_tool_schema_error(&call, context);
        }
    }
}
macro_rules! mcp_alias_dispatch_test {
    (
        $(#[$attribute:meta])*
        async fn $name:ident => error(
            $request_id:literal,
            $tool_name:literal,
            $arguments:ident,
            $context:literal,
            $status_context:literal $(,)?
        )
    ) => {
        $(#[$attribute])*
        async fn $name() {
            assert_mcp_alias_dispatch(McpAliasDispatchCase {
                request_id: $request_id,
                tool_name: $tool_name,
                arguments: McpAliasDispatchArguments::$arguments,
                expectation: McpAliasDispatchExpectation::ToolError {
                    context: $context,
                    status_context: $status_context,
                },
            })
            .await;
        }
    };
    (
        $(#[$attribute:meta])*
        async fn $name:ident => schema_error(
            $request_id:literal,
            $tool_name:literal,
            $arguments:ident,
            $context:literal $(,)?
        )
    ) => {
        $(#[$attribute])*
        async fn $name() {
            assert_mcp_alias_dispatch(McpAliasDispatchCase {
                request_id: $request_id,
                tool_name: $tool_name,
                arguments: McpAliasDispatchArguments::$arguments,
                expectation: McpAliasDispatchExpectation::SchemaError { context: $context },
            })
            .await;
        }
    };
    (
        $(#[$attribute:meta])*
        async fn $name:ident => success(
            $request_id:literal,
            $tool_name:literal,
            $arguments:ident,
            $context:literal $(,)?
        )
    ) => {
        $(#[$attribute])*
        async fn $name() {
            assert_mcp_alias_dispatch(McpAliasDispatchCase {
                request_id: $request_id,
                tool_name: $tool_name,
                arguments: McpAliasDispatchArguments::$arguments,
                expectation: McpAliasDispatchExpectation::Success { context: $context },
            })
            .await;
        }
    };
}
fn enable_writer_mcp(cfg: &mut iroha_config::parameters::actual::Root) {
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Writer;
}
fn assert_top_level_object_input_schema(tool_name: &str, schema: &norito::json::Map) {
    assert_eq!(
        schema.get("type").and_then(Value::as_str),
        Some("object"),
        "{tool_name} schema must be a top-level object"
    );
    assert!(!schema.contains_key("x-iroha-mcp-strict-body"));
    assert!(!schema.contains_key("x-iroha-mcp-flat-body"));
}
fn assert_schema_omits_exact_properties(schema: &Value, forbidden: &[&str], context: &str) {
    match schema {
        Value::Object(object) => {
            if let Some(properties) = object.get("properties").and_then(Value::as_object) {
                for key in forbidden {
                    assert!(
                        !properties.contains_key(*key),
                        "{context} must not publish the exact `{key}` property"
                    );
                }
            }
            for value in object.values() {
                assert_schema_omits_exact_properties(value, forbidden, context);
            }
        }
        Value::Array(values) => {
            for value in values {
                assert_schema_omits_exact_properties(value, forbidden, context);
            }
        }
        _ => {}
    }
}
#[tokio::test]
async fn mcp_get_streamable_http_endpoint_returns_method_not_allowed() {
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
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
#[tokio::test]
async fn mcp_jsonrpc_accepts_only_one_canonical_json_content_type() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.rate_per_minute = Some(NonZeroU32::new(10_000).expect("nonzero rate"));
    cfg.torii.mcp.burst = Some(NonZeroU32::new(10_000).expect("nonzero burst"));
    let app = build_router(cfg);
    let payload =
        norito::json::to_vec(&initialize_request(1)).expect("serialize initialize request");

    for content_type in [
        "application/json",
        "application/json;charset=utf-8",
        "application/json; charset=\"UTF-8\"",
    ] {
        let response = call_app(
            &app,
            Request::builder()
                .method("POST")
                .uri("/v1/mcp")
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(payload.clone()))
                .expect("canonical MCP request"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{content_type}");
    }

    for (content_type, expected_status, expected_code) in [
        (
            None,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_content_type_missing",
        ),
        (
            Some("text/plain"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_content_type_unsupported",
        ),
        (
            Some("application/problem+json"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_content_type_unsupported",
        ),
        // Native Norito is rejected by the global Norito-RPC admission gate
        // before MCP content-type validation and is covered by that gate's tests.
        (
            Some("application/json; profile=torii"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_content_type_unsupported",
        ),
        (
            Some("application/json;"),
            StatusCode::BAD_REQUEST,
            "request_content_type_invalid",
        ),
    ] {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/v1/mcp")
            .header(header::ACCEPT, "application/json");
        if let Some(content_type) = content_type {
            builder = builder.header(header::CONTENT_TYPE, content_type);
        }
        let response = call_app(
            &app,
            builder
                .body(Body::from("not valid JSON"))
                .expect("rejected MCP request"),
        )
        .await;
        assert_eq!(response.status(), expected_status, "{content_type:?}");
        let body = read_json_body(response).await;
        assert_eq!(
            body.get("code").and_then(Value::as_str),
            Some(expected_code),
            "{content_type:?}"
        );
    }

    let mut duplicate = Request::builder()
        .method("POST")
        .uri("/v1/mcp")
        .header(header::ACCEPT, "application/json")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("not valid JSON"))
        .expect("duplicate MCP request");
    duplicate.headers_mut().append(
        header::CONTENT_TYPE,
        "application/json".parse().expect("header"),
    );
    let response = call_app(&app, duplicate).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = read_json_body(response).await;
    assert_eq!(
        body.get("code").and_then(Value::as_str),
        Some("request_content_type_invalid")
    );
}
#[tokio::test]
async fn mcp_jsonrpc_rejects_hostile_origin() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.cors.enabled = true;
    cfg.torii.cors.allowed_origins = vec!["https://trusted.example".to_owned()];
    cfg.torii.cors.allowed_methods = vec!["POST".to_owned()];
    cfg.torii.cors.allowed_headers =
        vec!["content-type".to_owned(), "mcp-protocol-version".to_owned()];
    let app = build_router(cfg);
    let (status, body) = post_mcp_with_headers(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": "hostile-origin",
            "method": "ping"
        }),
        &[("Origin", "https://attacker.example")],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.get("error_code"))
            .and_then(Value::as_str),
        Some("origin_forbidden")
    );
    let (status, body) = post_mcp_with_headers(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": "trusted-origin",
            "method": "ping"
        }),
        &[("Origin", "https://trusted.example")],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("result").is_some());
}
#[tokio::test]
async fn mcp_connect_session_delete_tools_publish_openai_compatible_schema() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    cfg.torii.connect.enabled = true;
    let app = build_router(cfg);
    for name in ["iroha.connect.session.delete"] {
        let tool = find_tool(&app, name).await;
        let schema = tool
            .get("inputSchema")
            .and_then(Value::as_object)
            .expect("inputSchema object");
        assert_top_level_object_input_schema(name, schema);
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties object");
        assert!(properties.contains_key("sid"));
        assert!(!properties.contains_key("session_id"));
        assert!(properties.contains_key("token_management"));
        assert!(
            !properties.contains_key("path"),
            "{name} schema should keep delete parameters flat for OpenAI-compatible clients"
        );
        assert_eq!(
            schema
                .get("required")
                .and_then(Value::as_array)
                .map(|required| required
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()),
            Some(vec!["sid"])
        );
        assert_eq!(
            properties
                .get("sid")
                .and_then(|value| value.get("pattern"))
                .and_then(Value::as_str),
            Some("^[A-Za-z0-9_-]{43}$")
        );
    }
}
#[tokio::test]
async fn mcp_connect_ticket_tools_publish_openai_compatible_schema() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    cfg.torii.connect.enabled = true;
    let app = build_router(cfg);
    for name in ["iroha.connect.ws.ticket"] {
        let tool = find_tool(&app, name).await;
        let schema = tool
            .get("inputSchema")
            .and_then(Value::as_object)
            .expect("inputSchema object");
        assert_top_level_object_input_schema(name, schema);
        assert_eq!(
            schema
                .get("required")
                .and_then(Value::as_array)
                .map(|required| required
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()),
            Some(vec!["sid", "role", "node_url"])
        );
        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("properties object");
        assert!(!properties.contains_key("session_id"));
        assert_eq!(
            properties
                .get("sid")
                .and_then(|value| value.get("pattern"))
                .and_then(Value::as_str),
            Some("^[A-Za-z0-9_-]{43}$")
        );
        for token_field in ["token", "token_app", "token_wallet"] {
            let token_schema = properties
                .get(token_field)
                .and_then(Value::as_object)
                .expect("Connect token schema");
            assert_eq!(
                token_schema.get("pattern").and_then(Value::as_str),
                Some("^[A-Za-z0-9_-]{43}$")
            );
            assert_eq!(
                token_schema.get("minLength").and_then(Value::as_u64),
                Some(43)
            );
            assert_eq!(
                token_schema.get("maxLength").and_then(Value::as_u64),
                Some(43)
            );
        }
    }
}
#[tokio::test]
async fn mcp_vpn_session_detail_tools_publish_openai_compatible_schema() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    cfg.torii.connect.enabled = true;
    let app = build_router(cfg);
    let name = "iroha.vpn.sessions.get";
    let tool = find_tool(&app, name).await;
    let schema = tool
        .get("inputSchema")
        .and_then(Value::as_object)
        .expect("inputSchema object");
    assert_top_level_object_input_schema(name, schema);
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("properties object");
    assert!(properties.contains_key("session_id"));
    assert!(!properties.contains_key("id"));
    assert!(!properties.contains_key("path"));
}
#[tokio::test]
async fn mcp_all_published_tool_schemas_are_top_level_objects() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    cfg.torii.connect.enabled = true;
    let app = build_router(cfg);
    let tools = list_all_tools(&app).await;
    assert!(
        !tools.is_empty(),
        "tools/list should expose at least one tool"
    );
    for tool in tools {
        let name = tool.get("name").and_then(Value::as_str).expect("tool name");
        let schema = tool
            .get("inputSchema")
            .and_then(Value::as_object)
            .expect("inputSchema object");
        assert_top_level_object_input_schema(name, schema);
    }
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
    let (status, initialize) = post_mcp(&app, initialize_request(1)).await;
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
    let connect_ticket_tool = find_tool(&app, "iroha.connect.ws.ticket").await;
    assert_eq!(
        connect_ticket_tool.get("name").and_then(Value::as_str),
        Some("iroha.connect.ws.ticket"),
        "the canonical Connect ticket tool should be discoverable across pages"
    );
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": "call-1",
            "method": "tools/call",
            "params": {
                "name": "iroha.connect.ws.ticket",
                "arguments": {
                    "sid": "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE",
                    "role": "app",
                    "token": "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE",
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
        Some(
            "wss://node.example/v1/connect/ws?sid=AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE&role=app"
        )
    );
    assert_eq!(
        structured
            .get("authorization_header")
            .and_then(Value::as_str),
        Some("Bearer AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE")
    );
    assert_eq!(
        structured
            .get("sec_websocket_protocol")
            .and_then(Value::as_str),
        Some("iroha-connect.token.v1.QVFFQkFRRUJBUUVCQVFFQkFRRUJBUUVCQVFFQkFRRUJBUUVCQVFFQkFRRQ")
    );
}
#[tokio::test]
async fn mcp_jsonrpc_initialized_notification_returns_accepted_without_body() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.rate_per_minute = Some(NonZeroU32::new(10_000).expect("nonzero rate"));
    cfg.torii.mcp.burst = Some(NonZeroU32::new(10_000).expect("nonzero burst"));
    let app = build_router(cfg);
    let (status, initialize) = post_mcp(&app, initialize_request(1)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        initialize.get("result").is_some(),
        "initialize should succeed before the client sends initialized"
    );
    let (status, body) = post_mcp_bytes(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert!(
        body.is_empty(),
        "initialized notification should return 202 with no response body"
    );
}
#[tokio::test]
async fn mcp_jsonrpc_generic_notifications_return_accepted_without_body() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    for method in [
        "notifications/progress",
        "notifications/cancelled",
        "notifications/unknown",
    ] {
        let (status, body) = post_mcp_bytes(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "method": (method),
                "params": {
                    "progressToken": "test-progress",
                    "progress": 1
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED, "{method}");
        assert!(body.is_empty(), "{method} must not receive a response body");
    }
}

#[tokio::test]
async fn mcp_jsonrpc_authenticated_cancellation_stops_exact_live_call() {
    const CANCELLATION_NONCE: &str = "QkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkI";
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Writer;
    cfg.torii.mcp.rate_per_minute = Some(NonZeroU32::new(10_000).expect("nonzero rate"));
    cfg.torii.mcp.burst = Some(NonZeroU32::new(10_000).expect("nonzero burst"));
    cfg.torii.require_api_token = true;
    cfg.torii.api_tokens = vec!["cancellation-client".to_owned()];
    let app = build_router(cfg);

    let call_request = Request::builder()
        .method("POST")
        .uri("/v1/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-06-18")
        .header("x-api-token", "cancellation-client")
        .body(Body::from(
            norito::json::to_vec(&norito::json!({
                "jsonrpc": "2.0",
                "id": "cancel-me",
                "method": "tools/call",
                "params": {
                    "name": "iroha.transactions.wait",
                    "arguments": {
                        "query": { "hash": ("ab".repeat(32)) },
                        "timeout_ms": 600_000,
                        "poll_interval_ms": 100
                    },
                    "_meta": { "iroha/cancellationNonce": (CANCELLATION_NONCE) }
                }
            }))
            .expect("serialize call"),
        ))
        .expect("valid call request");
    let call_app_clone = app.clone();
    let mut live_call = tokio::spawn(async move { call_app(&call_app_clone, call_request).await });

    let cancelled = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let cancellation_request = Request::builder()
                .method("POST")
                .uri("/v1/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ACCEPT, "application/json, text/event-stream")
                .header("MCP-Protocol-Version", "2025-06-18")
                .header("x-api-token", "cancellation-client")
                .body(Body::from(
                    norito::json::to_vec(&norito::json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/cancelled",
                        "params": {
                            "requestId": "cancel-me",
                            "reason": "integration test",
                            "_meta": { "iroha/cancellationNonce": (CANCELLATION_NONCE) }
                        }
                    }))
                    .expect("serialize cancellation"),
                ))
                .expect("valid cancellation request");
            let cancellation = call_app(&app, cancellation_request).await;
            assert_eq!(cancellation.status(), StatusCode::ACCEPTED);
            assert!(read_body_bytes(cancellation).await.is_empty());

            tokio::select! {
                result = &mut live_call => return result.expect("live request task joins"),
                () = tokio::time::sleep(Duration::from_millis(10)) => {}
            }
        }
    })
    .await
    .expect("repeated cancellation reaches the registered request");
    assert_eq!(cancelled.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        cancelled
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("private, no-store")
    );
    assert!(read_body_bytes(cancelled).await.is_empty());
}
#[tokio::test]
async fn mcp_jsonrpc_response_messages_return_accepted_without_body() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    for (label, payload) in [
        (
            "success response",
            norito::json!({
                "jsonrpc": "2.0",
                "id": 1.5,
                "result": { "roots": [] }
            }),
        ),
        (
            "error response",
            norito::json!({
                "jsonrpc": "2.0",
                "id": "sampling-request",
                "error": {
                    "code": (-32603),
                    "message": "sampling failed"
                }
            }),
        ),
    ] {
        let (status, body) = post_mcp_bytes(&app, payload).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{label}");
        assert!(body.is_empty(), "{label} must not receive a response body");
    }
}
#[tokio::test]
async fn mcp_jsonrpc_ping_returns_empty_result_object() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.rate_per_minute = Some(NonZeroU32::new(10_000).expect("nonzero rate"));
    cfg.torii.mcp.burst = Some(NonZeroU32::new(10_000).expect("nonzero burst"));
    let app = build_router(cfg);
    let (status, ping) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "ping"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ping.get("id").and_then(Value::as_u64), Some(7));
    assert_eq!(ping.get("result"), Some(&norito::json!({})));
}
#[tokio::test]
async fn mcp_writer_prefix_policy_lists_only_curated_iroha_tools() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Writer;
    cfg.torii.mcp.allow_tool_prefixes = vec!["iroha.".to_owned()];
    let app = build_router(cfg);
    let names = list_all_tool_names(&app).await;
    assert!(
        !names.is_empty(),
        "writer+allowlist policy should expose tools"
    );
    assert!(
        names.iter().all(|name| name.starts_with("iroha.")),
        "expected only curated iroha.* tools, got {names:?}"
    );
    for required in [
        "iroha.accounts.query",
        "iroha.contracts.call_and_wait",
        "iroha.transactions.submit_and_wait",
    ] {
        assert!(
            names.iter().any(|name| name == required),
            "expected `{required}` in visible tool list, got {names:?}"
        );
    }
    for required in [
        "iroha.accounts.faucet.prepare",
        "iroha.accounts.faucet.submit",
    ] {
        assert!(
            names.iter().any(|name| name == required),
            "consensus consume-once faucet tool `{required}` must be exposed to writers"
        );
    }
    for operator_only in [
        "iroha.accounts.onboard.plan",
        "iroha.accounts.onboard.prepare",
        "iroha.accounts.onboard.submit",
    ] {
        assert!(
            names.iter().all(|name| name != operator_only),
            "operator-admitted onboarding tool leaked into the writer profile: {operator_only}"
        );
    }
    for retired in [
        "iroha.contracts.deploy",
        "iroha.contracts.deploy_bundle",
        "iroha.contracts.deploy_bundles.get",
    ] {
        assert!(
            names.iter().all(|name| name != retired),
            "retired server-side deployment tool leaked into MCP: {retired}"
        );
    }
    assert!(
        !names.iter().any(|name| name.starts_with("torii.")),
        "raw torii.* tools must be hidden by the public allowlist"
    );
    assert!(
        !names.iter().any(|name| name.starts_with("connect.")),
        "connect.* tools must be hidden when the allowlist only permits iroha.*"
    );
}
#[tokio::test]
async fn mcp_writer_prefix_policy_rejects_hidden_raw_tool_calls() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Writer;
    cfg.torii.mcp.allow_tool_prefixes = vec!["iroha.".to_owned()];
    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "torii.post_v1_pipeline_transactions"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        call.get("error").is_some(),
        "hidden raw tool should be rejected at the JSON-RPC layer"
    );
    assert_eq!(
        call.get("error")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.get("error_code"))
            .and_then(Value::as_str),
        Some("tool_not_allowed")
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
                .header("MCP-Protocol-Version", "2025-06-18")
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
async fn mcp_jsonrpc_rejects_invalid_request_ids() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    for (label, id) in [
        ("null", Value::Null),
        ("boolean", Value::Bool(true)),
        ("array", norito::json!([])),
        ("object", norito::json!({})),
    ] {
        let (status, body) = post_mcp(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "id": (id),
                "method": "ping"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{label}");
        assert_single_invalid_request(&body);
    }
}
#[tokio::test]
async fn mcp_jsonrpc_preserves_fractional_numeric_request_id() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let (status, body) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1.5,
            "method": "ping"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.get("id"), Some(&norito::json!(1.5)));
    assert_eq!(body.get("result"), Some(&norito::json!({})));
}
#[tokio::test]
async fn mcp_jsonrpc_requires_exact_string_version() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    for (label, request) in [
        (
            "wrong version",
            norito::json!({
                "jsonrpc": "1.0",
                "id": 7,
                "method": "initialize",
                "params": (initialize_params())
            }),
        ),
        (
            "missing version",
            norito::json!({
                "id": 8,
                "method": "initialize",
                "params": (initialize_params())
            }),
        ),
        (
            "null version",
            norito::json!({
                "jsonrpc": null,
                "id": 9,
                "method": "initialize",
                "params": (initialize_params())
            }),
        ),
        (
            "numeric version",
            norito::json!({
                "jsonrpc": 2,
                "id": 10,
                "method": "initialize",
                "params": (initialize_params())
            }),
        ),
    ] {
        let (status, body) = post_mcp(&app, request).await;
        assert_eq!(status, StatusCode::OK, "{label}");
        assert_eq!(
            body.get("error")
                .and_then(|value| value.get("code"))
                .and_then(Value::as_i64),
            Some(-32600),
            "{label}"
        );
    }
}
#[tokio::test]
async fn mcp_jsonrpc_rejects_unsupported_protocol_version_header() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let (status, body) = post_mcp_with_headers(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": "unsupported-protocol-version",
            "method": "ping"
        }),
        &[("MCP-Protocol-Version", "2024-11-05")],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.get("error_code"))
            .and_then(Value::as_str),
        Some("unsupported_protocol_version")
    );
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.get("supported_protocol_version"))
            .and_then(Value::as_str),
        Some("2025-06-18")
    );
}
#[tokio::test]
async fn mcp_jsonrpc_requires_protocol_header_after_initialize() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    for (request, expected_status) in [
        (initialize_request(1), StatusCode::OK),
        (
            norito::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "ping"
            }),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let request = Request::builder()
            .method("POST")
            .uri("/v1/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                norito::json::to_vec(&request).expect("serialize payload"),
            ))
            .expect("valid request");
        let response = call_app(&app, request).await;
        assert_eq!(response.status(), expected_status);
    }
}
#[tokio::test]
async fn mcp_jsonrpc_enforces_rate_limit() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.rate_per_minute = Some(NonZeroU32::new(1).expect("nonzero rate"));
    cfg.torii.mcp.burst = Some(NonZeroU32::new(1).expect("nonzero burst"));
    let app = build_router(cfg);
    let request = initialize_request(1);
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
async fn mcp_jsonrpc_charges_each_inner_tool_batch_dispatch() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.rate_per_minute = Some(NonZeroU32::new(1).expect("nonzero rate"));
    cfg.torii.mcp.burst = Some(NonZeroU32::new(2).expect("nonzero burst"));
    let app = build_router(cfg);
    let (status, body) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": "two-inner-dispatches",
            "method": "tools/call_batch",
            "params": {
                "calls": [
                    { "name": "iroha.health", "arguments": {} },
                    { "name": "iroha.health", "arguments": {} }
                ]
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.get("result").is_some(),
        "batch should consume its two-token budget"
    );

    let (status, body) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": "after-inner-dispatches",
            "method": "ping"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_i64),
        Some(-32029)
    );
}
#[tokio::test]
async fn mcp_one_per_minute_rate_does_not_round_up_to_one_per_second() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.rate_per_minute = Some(NonZeroU32::new(1).expect("nonzero rate"));
    cfg.torii.mcp.burst = Some(NonZeroU32::new(1).expect("nonzero burst"));
    let app = build_router(cfg);
    let request = norito::json!({
        "jsonrpc": "2.0",
        "id": "one-per-minute",
        "method": "ping"
    });
    let (status, _) = post_mcp(&app, request.clone()).await;
    assert_eq!(status, StatusCode::OK);

    tokio::time::sleep(Duration::from_millis(1_100)).await;

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
    let request_body =
        norito::json::to_vec(&initialize_request(1)).expect("serialize initialize request");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(request_body))
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
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.get("error_code"))
            .and_then(Value::as_str),
        Some("request_payload_too_large")
    );
}
#[tokio::test]
async fn mcp_jsonrpc_bounds_the_complete_response_envelope() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.max_request_bytes = 256;
    let app = build_router(cfg);
    let (status, body) = post_mcp(&app, initialize_request(1)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("code"))
            .and_then(Value::as_i64),
        Some(-32002)
    );
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.get("error_code"))
            .and_then(Value::as_str),
        Some("response_too_large")
    );
}
#[tokio::test]
async fn mcp_jsonrpc_rejects_oversized_outer_array_as_one_invalid_request() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let payload = Value::Array(
        (0_u64..65)
            .map(|id| {
                norito::json!({
                    "jsonrpc": "2.0",
                    "id": (id),
                    "method": "ping"
                })
            })
            .collect(),
    );
    let (status, body) = post_mcp(&app, payload).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_single_invalid_request(&body);
}
#[tokio::test]
async fn mcp_jsonrpc_rejects_tool_batch_above_dispatch_ceiling() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let calls = (0_u64..65)
        .map(|index| {
            norito::json!({
                "name": "iroha.health",
                "arguments": { "index": (index) }
            })
        })
        .collect::<Vec<_>>();
    let (status, body) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call_batch",
            "params": { "calls": (calls) }
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
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.get("error_code"))
            .and_then(Value::as_str),
        Some("batch_too_large")
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
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = read_json_body(response).await;
    assert_single_invalid_request(&body);
}
#[tokio::test]
async fn mcp_jsonrpc_rejects_valid_outer_array_as_one_invalid_request() {
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
                .header(header::ACCEPT, "application/json, text/event-stream")
                .header("MCP-Protocol-Version", "2025-06-18")
                .body(Body::from(
                    r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","id":2,"method":"ping"}]"#,
                ))
                .expect("valid request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = read_json_body(response).await;
    assert_single_invalid_request(&body);
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
async fn mcp_jsonrpc_rejects_every_unlisted_tool_alias() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    let app = build_router(cfg);
    let names = list_all_tool_names(&app).await;
    assert!(!names.iter().any(|name| name == "torii.post_transaction"));
    assert!(!names.iter().any(|name| name == "torii.healthCheck"));
    assert!(
        !names.iter().any(|name| name == "iroha.gov.ballots.zk"),
        "retired legacy governance ZK-ballot tool must remain absent"
    );
    for retired_name in [
        "iroha.sumeragi.evidence.submit",
        "iroha.sumeragi.vrf.commit",
        "iroha.sumeragi.vrf.reveal",
    ] {
        assert!(
            !names.iter().any(|name| name == retired_name),
            "retired Sumeragi mutation tool must remain absent: {retired_name}"
        );
    }
    assert!(names.iter().any(|name| name == "torii.get_health"));
    assert!(names.iter().any(|name| name == "iroha.transactions.submit"));
    for alias in [
        "torii.post_transaction",
        "torii.healthCheck",
        "iroha.gov.ballots.zk",
        "iroha.sumeragi.evidence.submit",
        "iroha.sumeragi.vrf.commit",
        "iroha.sumeragi.vrf.reveal",
    ] {
        let (status, body) = post_mcp(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "id": alias,
                "method": "tools/call",
                "params": {
                    "name": alias,
                    "arguments": {}
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body.get("error")
                .and_then(|value| value.get("code"))
                .and_then(Value::as_i64),
            Some(-32602),
            "unlisted alias must be rejected: {alias}"
        );
        assert_eq!(
            body.get("error")
                .and_then(|value| value.get("data"))
                .and_then(|value| value.get("error_code"))
                .and_then(Value::as_str),
            Some("tool_not_found"),
            "unlisted alias must not reach dispatch: {alias}"
        );
    }
}
#[tokio::test]
async fn mcp_jsonrpc_includes_universal_offline_operations_for_operator_profile() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    let app = build_router(cfg);
    let names = list_all_tool_names(&app).await;
    let expected = [
        "torii.get_v1_offline_readiness",
        "torii.post_v1_offline_receiver_lineage",
        "torii.post_v1_offline_top_up",
        "torii.post_v1_offline_redeem",
        "torii.get_v1_offline_operations_operation_id",
    ];
    for name in expected {
        assert!(
            names.iter().any(|candidate| candidate == name),
            "universal offline operation is missing from tools/list: {name}"
        );
    }
    assert!(names.iter().any(|name| name == "iroha.health"));
    assert!(names.iter().any(|name| name == "iroha.transactions.submit"));
}
#[tokio::test]
async fn mcp_jsonrpc_uncataloged_openapi_operation_fails_closed() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    let app = build_router(cfg);
    let unprojected_name = "torii.get_v1_accounts";
    let names = list_all_tool_names(&app).await;
    assert!(
        !names.iter().any(|name| name == unprojected_name),
        "OpenAPI presence without an exact catalog MCP projection must not publish a tool"
    );
    let (status, body) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": "uncataloged-openapi",
            "method": "tools/call",
            "params": {
                "name": unprojected_name,
                "arguments": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body.get("error")
            .and_then(|value| value.get("data"))
            .and_then(|value| value.get("error_code"))
            .and_then(Value::as_str),
        Some("tool_not_found"),
        "uncataloged OpenAPI operations must be rejected before dispatch"
    );
}
#[tokio::test]
async fn mcp_jsonrpc_rejects_manual_tools_without_catalog_projection() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    let app = build_router(cfg);
    let names = list_all_tool_names(&app).await;
    let excluded = [
        "iroha.status",
        "iroha.time.now",
        "iroha.time.status",
        "iroha.ledger.headers",
        "iroha.ledger.state_root",
        "iroha.ledger.state_proof",
        "iroha.ledger.block_proof",
        "iroha.proofs.get",
        "iroha.proofs.retention",
    ];
    for name in excluded {
        assert!(
            !names.iter().any(|candidate| candidate == name),
            "non-projected manual tool leaked into tools/list: {name}"
        );
        let (status, body) = post_mcp(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "id": name,
                "method": "tools/call",
                "params": {
                    "name": name,
                    "arguments": {}
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body.get("error")
                .and_then(|value| value.get("data"))
                .and_then(|value| value.get("error_code"))
                .and_then(Value::as_str),
            Some("tool_not_found"),
            "non-projected manual tool must not be callable: {name}"
        );
    }
    assert!(names.iter().any(|name| name == "iroha.health"));
    assert!(names.iter().any(|name| name == "iroha.queries.submit"));
    assert!(names.iter().any(|name| name == "iroha.transactions.submit"));
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
                "name": "torii.get_health"
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
                            "name": "torii.get_health"
                        }
                    }))
                    .expect("serialize payload"),
                ))
                .expect("valid request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().contains_key(header::WWW_AUTHENTICATE));
}
#[tokio::test]
async fn retired_transaction_status_alias_is_not_mounted() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let app = build_router(test_utils::mk_minimal_root_cfg());
    let response = call_app(
        &app,
        Request::builder()
            .uri("/v1/transactions/status")
            .body(Body::empty())
            .expect("retired status request"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
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
                "name": "torii.get_health"
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
async fn mcp_jsonrpc_tools_call_projected_node_operational_endpoints_dispatch() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    for (id, tool_name) in [
        (1031, "iroha.health"),
        (1033, "iroha.parameters.get"),
        (1034, "iroha.node.capabilities"),
        (1037, "torii.get_v1_api_version"),
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
        assert!(
            !tool_is_error(&call),
            "projected operational tool `{tool_name}` should not be an MCP tool error"
        );
        assert_eq!(
            http_status,
            Some(200),
            "projected operational tool `{tool_name}` should return HTTP 200 in test harness"
        );
    }
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_da_read_endpoints_dispatch() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    for (id, tool_name, arguments) in [
        (1045, "iroha.da.proof_policies", norito::json!({})),
        (1046, "iroha.da.proof_policy_snapshot", norito::json!({})),
        (
            1047,
            "iroha.da.manifests.get",
            norito::json!({
                "id": "manifest-ticket-001"
            }),
        ),
    ] {
        let (status, call) = post_mcp(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": arguments
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let structured = structured_content(&call);
        let http_status = structured.get("status").and_then(Value::as_u64);
        assert!(
            http_status.is_some(),
            "DA alias `{tool_name}` should return an HTTP status"
        );
        if tool_is_error(&call) {
            assert!(
                http_status.is_some_and(|status| status >= 400),
                "error path for `{tool_name}` should reflect HTTP error status"
            );
        } else {
            assert_eq!(
                http_status,
                Some(200),
                "successful path for `{tool_name}` should return HTTP 200"
            );
        }
    }
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_da_ingest_accepts_body() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    enable_writer_mcp(&mut cfg);
    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1048,
            "method": "tools/call",
            "params": {
                "name": "iroha.da.ingest",
                "arguments": {
                    "body": {}
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
        "DA ingest alias should return an HTTP status"
    );
    if tool_is_error(&call) {
        assert!(
            http_status.is_some_and(|status| status >= 400),
            "error path for DA ingest alias should reflect HTTP error status"
        );
    } else {
        assert_eq!(
            http_status,
            Some(200),
            "successful DA ingest alias calls should return HTTP 200"
        );
    }
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_da_commitments_endpoints_accept_body() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    enable_writer_mcp(&mut cfg);
    let app = build_router(cfg);
    for (id, tool_name) in [
        (1048, "iroha.da.commitments.list"),
        (1049, "iroha.da.commitments.prove"),
        (1050, "iroha.da.commitments.verify"),
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
        let http_status = structured.get("status").and_then(Value::as_u64);
        assert!(
            http_status.is_some(),
            "DA alias `{tool_name}` should return an HTTP status"
        );
        if tool_is_error(&call) {
            assert!(
                http_status.is_some_and(|status| status >= 400),
                "error path for `{tool_name}` should reflect HTTP error status"
            );
        } else {
            assert_eq!(
                http_status,
                Some(200),
                "successful path for `{tool_name}` should return HTTP 200"
            );
        }
    }
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_da_pin_intents_endpoints_accept_body() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    enable_writer_mcp(&mut cfg);
    let app = build_router(cfg);
    for (id, tool_name) in [
        (1058, "iroha.da.pin_intents.list"),
        (1059, "iroha.da.pin_intents.prove"),
        (1060, "iroha.da.pin_intents.verify"),
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
        let http_status = structured.get("status").and_then(Value::as_u64);
        assert!(
            http_status.is_some(),
            "DA alias `{tool_name}` should return an HTTP status"
        );
        if tool_is_error(&call) {
            assert!(
                http_status.is_some_and(|status| status >= 400),
                "error path for `{tool_name}` should reflect HTTP error status"
            );
        } else {
            assert_eq!(
                http_status,
                Some(200),
                "successful path for `{tool_name}` should return HTTP 200"
            );
        }
    }
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_runtime_endpoints_dispatch() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    for (id, tool_name) in [
        (1051, "iroha.runtime.abi.active"),
        (1052, "iroha.runtime.abi.hash"),
        (1053, "iroha.runtime.metrics"),
        (1054, "iroha.runtime.upgrades.list"),
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
        assert!(
            http_status.is_some(),
            "runtime alias `{tool_name}` should return an HTTP status"
        );
        if tool_is_error(&call) {
            assert!(
                http_status.is_some_and(|status| status >= 400),
                "error path for `{tool_name}` should reflect HTTP error status"
            );
        } else {
            assert_eq!(
                http_status,
                Some(200),
                "successful path for `{tool_name}` should return HTTP 200"
            );
        }
    }
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_runtime_upgrade_mutation_endpoints_dispatch() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    enable_writer_mcp(&mut cfg);
    let app = build_router(cfg);
    for (id, tool_name, arguments) in [
        (
            1055,
            "iroha.runtime.upgrades.propose",
            norito::json!({ "body": {} }),
        ),
        (
            1056,
            "iroha.runtime.upgrades.activate",
            norito::json!({
                "path": { "id": "upgrade-001" },
                "body": {}
            }),
        ),
        (
            1057,
            "iroha.runtime.upgrades.cancel",
            norito::json!({
                "path": { "id": "upgrade-001" },
                "body": {}
            }),
        ),
    ] {
        let (status, call) = post_mcp(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": arguments
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let structured = structured_content(&call);
        let http_status = structured.get("status").and_then(Value::as_u64);
        assert!(
            http_status.is_some(),
            "runtime upgrade alias `{tool_name}` should return an HTTP status"
        );
        if tool_is_error(&call) {
            assert!(
                http_status.is_some_and(|status| status >= 400),
                "error path for `{tool_name}` should reflect HTTP error status"
            );
        } else {
            assert_eq!(
                http_status,
                Some(200),
                "successful path for `{tool_name}` should return HTTP 200"
            );
        }
    }
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_finality_endpoints_accept_canonical_height_path() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    for (id, tool_name) in [
        (1041, "iroha.bridge.finality.proof"),
        (1042, "iroha.bridge.finality.bundle"),
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
                        "path": { "height": 1 }
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
            "finality tool `{tool_name}` should return an HTTP status"
        );
        if tool_is_error(&call) {
            assert!(
                http_status.is_some_and(|status| status >= 400),
                "error path for `{tool_name}` should still reflect HTTP error status"
            );
        } else {
            assert_eq!(
                http_status,
                Some(200),
                "successful path for `{tool_name}` should return HTTP 200"
            );
        }
    }
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_uncataloged_proof_query_dispatches() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let tool_name = "iroha.proofs.query";
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10311,
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
    let http_status = structured.get("status").and_then(Value::as_u64);
    assert!(
        http_status.is_some(),
        "proof query tool should return an HTTP status"
    );
    if tool_is_error(&call) {
        assert!(
            http_status.is_some_and(|status| status >= 400),
            "proof query error should reflect an HTTP error status"
        );
    } else {
        assert_eq!(
            http_status,
            Some(200),
            "successful proof queries should return HTTP 200"
        );
    }
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_gov_endpoints_dispatch() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    let app = build_router(cfg);
    for (id, tool_name, arguments) in [
        (
            10320,
            "iroha.gov.contract.get",
            norito::json!({
                "path": {
                    "contract_address": "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                }
            }),
        ),
        (
            10321,
            "iroha.gov.proposals.deploy_contract",
            norito::json!({
                "body": {}
            }),
        ),
        (
            10322,
            "iroha.gov.proposals.get",
            norito::json!({
                "path": { "id": ("11".repeat(32)) }
            }),
        ),
        (
            10323,
            "iroha.gov.locks.get",
            norito::json!({
                "path": { "rid": "referendum-001" }
            }),
        ),
        (
            10324,
            "iroha.gov.referenda.get",
            norito::json!({
                "path": { "id": "referendum-001" }
            }),
        ),
        (
            10325,
            "iroha.gov.tally.get",
            norito::json!({
                "path": { "id": "tally-001" }
            }),
        ),
        (
            10327,
            "iroha.gov.ballots.zk_v1",
            norito::json!({
                "body": { "election_id": "election-001" }
            }),
        ),
        (
            10328,
            "iroha.gov.ballots.zk_v1.ballot_proof",
            norito::json!({
                "body": { "election_id": "election-001" }
            }),
        ),
        (
            10329,
            "iroha.gov.ballots.plain",
            norito::json!({
                "body": { "referendum_id": "referendum-001" }
            }),
        ),
        (
            10330,
            "iroha.gov.protected_namespaces.list",
            norito::json!({}),
        ),
        (
            10331,
            "iroha.gov.protected_namespaces.update",
            norito::json!({
                "body": {}
            }),
        ),
        (10332, "iroha.gov.unlocks.stats", norito::json!({})),
        (10333, "iroha.gov.council.current", norito::json!({})),
    ] {
        let (status, call) = post_mcp(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": arguments
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let structured = call
            .get("result")
            .and_then(|value| value.get("structuredContent"))
            .and_then(Value::as_object)
            .unwrap_or_else(|| {
                panic!("governance alias `{tool_name}` should return structured content: {call:?}")
            });
        let http_status = structured.get("status").and_then(Value::as_u64);
        assert!(
            http_status.is_some(),
            "governance alias `{tool_name}` should return an HTTP status"
        );
        if tool_is_error(&call) {
            assert!(
                http_status.is_some_and(|status| status >= 400),
                "error path for `{tool_name}` should reflect HTTP error status"
            );
        } else {
            assert_eq!(
                http_status,
                Some(200),
                "successful path for `{tool_name}` should return HTTP 200"
            );
        }
    }
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_contract_call_dispatches() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    let app = build_router(cfg);
    for (id, tool_name) in [(10403, "iroha.contracts.call")] {
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
        assert!(
            call.get("result").is_some(),
            "contract alias `{tool_name}` should return a JSON-RPC result, got {call:?}"
        );
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
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
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
async fn mcp_jsonrpc_tools_call_contracts_code_get_accepts_canonical_code_hash_path() {
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
                    "path": { "code_hash": "not-a-code-hash" }
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
async fn mcp_jsonrpc_tools_call_contracts_code_bytes_get_accepts_canonical_code_hash_path() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10409,
            "method": "tools/call",
            "params": {
                "name": "iroha.contracts.code.bytes.get",
                "arguments": {
                    "path": { "code_hash": "not-a-code-hash" }
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_tool_schema_error(
        &call,
        "invalid code hash must fail contract code-bytes advertised-schema validation",
    );
}
#[tokio::test]
async fn retired_server_contract_deploy_mcp_tools_are_not_callable() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    for (id, name) in [
        (10410, "iroha.contracts.deploy"),
        (10411, "iroha.contracts.deploy_bundle"),
        (10412, "iroha.contracts.deploy_bundles.get"),
    ] {
        let (status, call) = post_mcp(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": { "name": name, "arguments": {} }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            call.pointer("/error/data/error_code")
                .and_then(Value::as_str),
            Some("tool_not_found"),
            "retired tool remained callable: {name}"
        );
    }
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
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    let app = build_router(cfg);
    let names = list_all_tool_names(&app).await;
    assert!(
        names.iter().any(|name| name == "iroha.accounts.list"),
        "expected explicitly allowlisted account listing tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.accounts.transactions"),
        "expected explicitly allowlisted account transaction tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.accounts.history"),
        "expected explicitly allowlisted account history tool"
    );
    assert!(
        names.iter().all(|name| {
            !matches!(
                name.as_str(),
                "torii.get_v1_accounts"
                    | "torii.get_v1_accounts_account_id_transactions"
                    | "torii.get_v1_accounts_account_id_history"
            )
        }),
        "uncataloged OpenAPI operations must not become tools implicitly"
    );
    assert!(
        names.iter().any(|name| name == "iroha.health"),
        "expected agent-friendly node health MCP tool"
    );
    assert!(
        !names.iter().any(|name| name == "iroha.status"),
        "diagnostic status must not be projected into MCP"
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
        !names.iter().any(|name| name == "iroha.time.now"),
        "non-projected time-now route must not be an MCP tool"
    );
    assert!(
        !names.iter().any(|name| name == "iroha.time.status"),
        "non-projected time-status route must not be an MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "torii.get_v1_api_version"),
        "expected explicitly projected API-version MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.da.ingest"),
        "expected agent-friendly DA ingest MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.da.proof_policies"),
        "expected agent-friendly DA proof-policies MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.da.proof_policy_snapshot"),
        "expected agent-friendly DA proof-policy snapshot MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.da.manifests.get"),
        "expected agent-friendly DA manifest lookup MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.da.commitments.list"),
        "expected agent-friendly DA commitment list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.da.commitments.prove"),
        "expected agent-friendly DA commitment prove MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.da.commitments.verify"),
        "expected agent-friendly DA commitment verify MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.da.pin_intents.list"),
        "expected agent-friendly DA pin intents list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.da.pin_intents.prove"),
        "expected agent-friendly DA pin intents prove MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.da.pin_intents.verify"),
        "expected agent-friendly DA pin intents verify MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.runtime.abi.active"),
        "expected agent-friendly runtime ABI-active MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.runtime.abi.hash"),
        "expected agent-friendly runtime ABI-hash MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.runtime.metrics"),
        "expected agent-friendly runtime metrics MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.runtime.upgrades.list"),
        "expected agent-friendly runtime upgrades-list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.runtime.upgrades.propose"),
        "expected agent-friendly runtime upgrades-propose MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.runtime.upgrades.activate"),
        "expected agent-friendly runtime upgrades-activate MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.runtime.upgrades.cancel"),
        "expected agent-friendly runtime upgrades-cancel MCP tool"
    );
    for retired_name in [
        "iroha.sumeragi.pacemaker",
        "iroha.sumeragi.commit_certificates",
        "iroha.sumeragi.validator_sets.list",
        "iroha.sumeragi.validator_sets.get",
        "iroha.sumeragi.params",
        "iroha.sumeragi.status",
        "iroha.sumeragi.leader",
        "iroha.sumeragi.qc",
        "iroha.sumeragi.checkpoints",
        "iroha.sumeragi.consensus_keys",
        "iroha.sumeragi.bls_keys",
        "iroha.sumeragi.telemetry",
        "iroha.sumeragi.phases",
        "iroha.sumeragi.commit_qc.get",
        "iroha.sumeragi.evidence.count",
        "iroha.sumeragi.evidence.list",
        "iroha.sumeragi.vrf.penalties",
        "iroha.sumeragi.vrf.epoch",
        "iroha.sumeragi.evidence.submit",
        "iroha.sumeragi.vrf.commit",
        "iroha.sumeragi.vrf.reveal",
    ] {
        assert!(
            !names.iter().any(|name| name == retired_name),
            "operator-only Sumeragi route must remain absent from MCP: {retired_name}"
        );
    }
    assert!(
        !names.iter().any(|name| name == "iroha.ledger.headers"),
        "non-projected ledger headers route must not be an MCP tool"
    );
    assert!(
        !names.iter().any(|name| name == "iroha.ledger.state_root"),
        "non-projected ledger state-root route must not be an MCP tool"
    );
    assert!(
        !names.iter().any(|name| name == "iroha.ledger.state_proof"),
        "non-projected ledger state-proof route must not be an MCP tool"
    );
    assert!(
        !names.iter().any(|name| name == "iroha.ledger.block_proof"),
        "non-projected ledger block-proof route must not be an MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.bridge.finality.proof"),
        "expected agent-friendly bridge finality-proof MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.bridge.finality.bundle"),
        "expected agent-friendly bridge finality-bundle MCP tool"
    );
    assert!(
        !names.iter().any(|name| name == "iroha.proofs.get"),
        "non-projected proof detail route must not be an MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.proofs.query"),
        "expected agent-friendly proof query MCP tool"
    );
    assert!(
        !names.iter().any(|name| name == "iroha.proofs.retention"),
        "non-projected proof retention route must not be an MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.gov.contract.get"),
        "expected agent-friendly governance contract-get MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.gov.proposals.deploy_contract"),
        "expected agent-friendly governance deploy-contract MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.gov.proposals.get"),
        "expected agent-friendly governance proposal detail MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.gov.locks.get"),
        "expected agent-friendly governance lock-detail MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.gov.referenda.get"),
        "expected agent-friendly governance referendum detail MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.gov.tally.get"),
        "expected agent-friendly governance tally detail MCP tool"
    );
    assert!(
        !names.iter().any(|name| name == "iroha.gov.ballots.zk"),
        "retired legacy governance ZK-ballot MCP tool must remain absent"
    );
    assert!(
        names.iter().any(|name| name == "iroha.gov.ballots.zk_v1"),
        "expected agent-friendly governance ZK-v1-ballot MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.gov.ballots.zk_v1.ballot_proof"),
        "expected agent-friendly governance ballot-proof MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.gov.ballots.plain"),
        "expected agent-friendly governance plain-ballot MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.gov.protected_namespaces.list"),
        "expected agent-friendly governance protected-namespaces list MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.gov.protected_namespaces.update"),
        "expected agent-friendly governance protected-namespaces update MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.gov.unlocks.stats"),
        "expected agent-friendly governance unlocks-stats MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.gov.council.current"),
        "expected agent-friendly governance council snapshot MCP tool"
    );
    assert!(
        !names.iter().any(|name| name == "iroha.gov.council.persist"),
        "removed governance council persist MCP tool must remain absent"
    );
    assert!(
        !names.iter().any(|name| name == "iroha.gov.council.replace"),
        "removed governance council replace MCP tool must remain absent"
    );
    assert!(
        !names.iter().any(|name| name == "iroha.gov.council.audit"),
        "removed governance council audit MCP tool must remain absent"
    );
    assert!(
        !names
            .iter()
            .any(|name| name == "iroha.gov.council.derive_vrf"),
        "removed governance council derive-vrf MCP tool must remain absent"
    );
    for retired_name in ["iroha.gov.enact", "iroha.gov.finalize"] {
        assert!(
            !names.iter().any(|name| name == retired_name),
            "proposal-backed legacy governance tool must remain absent: {retired_name}"
        );
    }
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
        names.iter().any(|name| name == "iroha.contracts.code.get"),
        "expected agent-friendly contract code detail MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.contracts.code.bytes.get"),
        "expected agent-friendly contract code-bytes MCP tool"
    );
    for retired in [
        "iroha.contracts.deploy",
        "iroha.contracts.deploy_bundle",
        "iroha.contracts.deploy_bundles.get",
    ] {
        assert!(
            names.iter().all(|name| name != retired),
            "retired server-side deployment MCP tool leaked into tool list: {retired}"
        );
    }
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
    for required in [
        "iroha.accounts.onboard.prepare",
        "iroha.accounts.onboard.submit",
        "iroha.accounts.faucet.prepare",
        "iroha.accounts.faucet.submit",
    ] {
        assert!(
            names.iter().any(|name| name == required),
            "expected explicit prepared-transaction MCP tool `{required}`"
        );
    }
    assert!(names.iter().all(|name| !matches!(
        name.as_str(),
        "iroha.accounts.onboard" | "iroha.accounts.faucet"
    )));
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.accounts.transactions"),
        "expected agent-friendly account transactions MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.accounts.history"),
        "expected agent-friendly account history MCP tool"
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
    for expected in [
        "iroha.musubi.queries.exact_package",
        "iroha.musubi.queries.exact_release",
        "iroha.musubi.queries.resolver_index",
        "iroha.musubi.queries.versions",
        "iroha.musubi.queries.maintainers",
        "iroha.musubi.queries.archive_locations",
        "iroha.musubi.queries.provider_bundle_attestation",
        "iroha.musubi.queries.archive_retention",
        "iroha.musubi.queries.alias",
        "iroha.musubi.queries.alias_history",
        "iroha.musubi.queries.ordered_prefix",
        "iroha.musubi.queries.search",
        "iroha.musubi.instructions.namespace_binding_register",
        "iroha.musubi.instructions.archive_register",
        "iroha.musubi.instructions.provider_bundle_attestation_register",
        "iroha.musubi.instructions.archive_location_add",
        "iroha.musubi.instructions.archive_location_retire",
        "iroha.musubi.instructions.release_publish",
        "iroha.musubi.instructions.release_yank_set",
        "iroha.musubi.instructions.package_metadata_set",
        "iroha.musubi.instructions.package_member_invite",
        "iroha.musubi.instructions.package_member_accept",
        "iroha.musubi.instructions.package_member_invitation_revoke",
        "iroha.musubi.instructions.package_member_set_role",
        "iroha.musubi.instructions.package_member_remove",
        "iroha.musubi.instructions.alias_register",
        "iroha.musubi.instructions.package_recover",
        "iroha.musubi.instructions.alias_retarget",
        "iroha.musubi.instructions.artifact_takedown",
        "iroha.musubi.instructions.registry_policy_set",
        "iroha.musubi.instructions.release_digest_assert",
    ] {
        assert!(
            names.iter().any(|name| name == expected),
            "expected first-release Musubi MCP tool `{expected}`"
        );
    }
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
        names.iter().any(|name| name == "iroha.nfts.chain.list"),
        "expected agent-friendly chain NFT list MCP tool"
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
        names.iter().any(|name| name == "iroha.rwas.chain.list"),
        "expected agent-friendly chain RWA list MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.rwas.list"),
        "expected agent-friendly rwa list MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.rwas.get"),
        "expected agent-friendly rwa detail MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.rwas.query"),
        "expected agent-friendly rwa query MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.iso20022.pacs008.submit"),
        "expected agent-friendly ISO pacs.008 submit MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.iso20022.pacs009.submit"),
        "expected agent-friendly ISO pacs.009 submit MCP tool"
    );
    assert!(
        names.iter().any(|name| name == "iroha.iso20022.status.get"),
        "expected agent-friendly ISO status MCP tool"
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
    for retired in [
        "iroha.sumeragi.rbc",
        "iroha.sumeragi.rbc.sessions",
        "iroha.sumeragi.rbc.delivered",
        "iroha.sumeragi.rbc.sample",
        "iroha.sumeragi.collectors",
    ] {
        assert!(
            names.iter().all(|name| name != retired),
            "retired Sumeragi MCP tool {retired} must not be advertised"
        );
    }
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
            .all(|name| name != "iroha.connect.session.create_and_ticket"),
        "credential-minting Connect composite must remain retired"
    );
    assert!(
        names.iter().any(|name| name == "iroha.connect.ws.ticket"),
        "expected agent-friendly connect ws ticket MCP tool"
    );
    assert!(
        names
            .iter()
            .any(|name| name == "iroha.connect.session.status"),
        "expected agent-friendly connect session status MCP tool"
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
                "name": "iroha.accounts.transactions",
                "arguments": {
                    "path": {
                        "account_id": TEST_ACCOUNT_I105
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
async fn mcp_jsonrpc_tools_call_account_history_uses_path_and_query_arguments() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10450,
            "method": "tools/call",
            "params": {
                "name": "iroha.accounts.history",
                "arguments": {
                    "path": {
                        "account_id": TEST_ACCOUNT_I105
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
        "invalid account history query should be marked as MCP tool error"
    );
    let structured = structured_content(&call);
    assert!(
        structured
            .get("status")
            .and_then(Value::as_u64)
            .is_some_and(|status| status >= 400),
        "expected invalid `limit=0` account history query argument to be rejected"
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
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_accounts_get_accepts_canonical_path => error(
        1051,
        "iroha.accounts.get",
        InvalidAccountId,
        "invalid account id should be marked as MCP tool error for account detail alias",
        "expected invalid account id to be rejected by explorer account detail alias",
    )
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_accounts_qr_accepts_canonical_path => error(
        1052,
        "iroha.accounts.qr",
        InvalidAccountId,
        "invalid account id should be marked as MCP tool error for account QR alias",
        "expected invalid account id to be rejected by explorer account QR alias",
    )
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
    assert_tool_schema_error(
        &call,
        "invalid transaction-status hash must fail advertised-schema validation",
    );
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_transaction_status_accepts_canonical_query_hash => schema_error(
        1061,
        "iroha.transactions.status",
        InvalidStatusHash,
        "invalid query hash must fail advertised-schema validation",
    )
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_transaction_wait_rejects_retired_flat_hash() {
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
    assert_tool_schema_error(
        &call,
        "retired flat transaction-wait hash must fail advertised-schema validation",
    );
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_transaction_wait_rejects_query_transaction_hash_alias()
{
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
                "name": "iroha.transactions.wait",
                "arguments": {
                    "query": {
                        "transaction_hash": "not-a-hash"
                    },
                    "timeout_ms": 1000,
                    "poll_interval_ms": 100
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_tool_schema_error(
        &call,
        "retired query.transaction_hash must fail advertised-schema validation",
    );
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_transactions_list_accepts_flat_query_fields => success(
        10611,
        "iroha.transactions.list",
        LimitTwo,
        "transactions list alias with flat query fields should dispatch successfully",
    )
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_transactions_get_accepts_canonical_path => error(
        10612,
        "iroha.transactions.get",
        InvalidPathHash,
        "invalid hash should be marked as MCP tool error for transaction detail alias",
        "expected invalid transaction hash to be rejected by explorer detail alias",
    )
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_transactions_get_rejects_retired_path_transaction_hash() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106121,
            "method": "tools/call",
            "params": {
                "name": "iroha.transactions.get",
                "arguments": {
                    "path": {
                        "transaction_hash": "not-a-hash"
                    }
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let message = assert_tool_schema_error(
        &call,
        "retired nested transaction_hash must fail advertised-schema validation",
    );
    assert!(
        message.contains("transaction_hash"),
        "retired nested transaction_hash must be rejected before dispatch"
    );
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_instructions_list_accepts_flat_query_fields => success(
        10613,
        "iroha.instructions.list",
        PageOne,
        "instructions list alias with flat query fields should dispatch successfully",
    )
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_instructions_get_accepts_canonical_path() {
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
                    "path": {
                        "hash": "not-a-hash",
                        "index": 0
                    }
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
async fn mcp_jsonrpc_tools_call_instructions_get_rejects_retired_flat_aliases() {
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
    let message = assert_tool_schema_error(
        &call,
        "retired flat instruction aliases must fail advertised-schema validation",
    );
    assert!(
        message.contains("transaction_hash"),
        "retired flat instruction aliases must be rejected before dispatch"
    );
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_instructions_get_rejects_retired_path_transaction_hash() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 1061511,
            "method": "tools/call",
            "params": {
                "name": "iroha.instructions.get",
                "arguments": {
                    "path": {
                        "transaction_hash": "not-a-hash",
                        "index": 0
                    }
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let message = assert_tool_schema_error(
        &call,
        "retired nested instruction hash alias must fail advertised-schema validation",
    );
    assert!(
        message.contains("transaction_hash"),
        "retired nested transaction_hash must be rejected before dispatch"
    );
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_assets_list_accepts_flat_query_fields => success(
        106151,
        "iroha.assets.list",
        PageOne,
        "assets list alias with flat query fields should dispatch successfully",
    )
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_assets_get_accepts_canonical_path => error(
        106152,
        "iroha.assets.get",
        InvalidAssetId,
        "invalid asset id should be marked as MCP tool error for asset detail alias",
        "expected invalid asset id to be rejected by explorer asset detail alias",
    )
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_nfts_chain_list_dispatches_route() {
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
                "name": "iroha.nfts.chain.list",
                "arguments": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let structured = structured_content(&call);
    let http_status = structured.get("status").and_then(Value::as_u64);
    assert!(
        http_status.is_some(),
        "nfts chain-list alias should return an HTTP status"
    );
    if tool_is_error(&call) {
        assert!(
            http_status.is_some_and(|status| status >= 400),
            "error path for nft chain-list alias should reflect HTTP error status"
        );
    } else {
        assert_eq!(
            http_status,
            Some(200),
            "successful nft chain-list alias should return HTTP 200"
        );
    }
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_nfts_list_accepts_flat_query_fields => success(
        106153,
        "iroha.nfts.list",
        PageOne,
        "nfts list alias with flat query fields should dispatch successfully",
    )
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_nfts_get_accepts_canonical_path => error(
        106154,
        "iroha.nfts.get",
        InvalidNftId,
        "invalid nft id should be marked as MCP tool error for nft detail alias",
        "expected invalid nft id to be rejected by explorer nft detail alias",
    )
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_nfts_query_accepts_flat_envelope_fields => success(
        106155,
        "iroha.nfts.query",
        LimitTwo,
        "nfts query alias with flat envelope fields should dispatch successfully",
    )
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_rwas_chain_list_dispatches_route() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106156,
            "method": "tools/call",
            "params": {
                "name": "iroha.rwas.chain.list",
                "arguments": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let structured = structured_content(&call);
    let http_status = structured.get("status").and_then(Value::as_u64);
    assert!(
        http_status.is_some(),
        "rwas chain-list alias should return an HTTP status"
    );
    if tool_is_error(&call) {
        assert!(
            http_status.is_some_and(|status| status >= 400),
            "error path for rwa chain-list alias should reflect HTTP error status"
        );
    } else {
        assert_eq!(
            http_status,
            Some(200),
            "successful rwa chain-list alias should return HTTP 200"
        );
    }
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_rwas_list_accepts_flat_query_fields => success(
        106157,
        "iroha.rwas.list",
        PageOne,
        "rwas list alias with flat query fields should dispatch successfully",
    )
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_rwas_get_accepts_canonical_path => error(
        106158,
        "iroha.rwas.get",
        InvalidRwaId,
        "invalid rwa id should be marked as MCP tool error for rwa detail alias",
        "expected invalid rwa id to be rejected by explorer rwa detail alias",
    )
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_rwas_query_accepts_flat_envelope_fields => success(
        106159,
        "iroha.rwas.query",
        LimitTwo,
        "rwas query alias with flat envelope fields should dispatch successfully",
    )
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_blocks_list_accepts_flat_query_fields => success(
        10616,
        "iroha.blocks.list",
        PageOne,
        "blocks list alias with flat query fields should dispatch successfully",
    )
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_blocks_get_accepts_canonical_identifier_path() {
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
                    "path": { "identifier": 0 }
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
async fn mcp_jsonrpc_tools_call_agent_alias_account_transactions_rejects_retired_flat_arguments() {
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
                    "account_id": TEST_ACCOUNT_I105,
                    "limit": 0
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_tool_schema_error(
        &call,
        "retired flat account-transactions arguments must fail advertised-schema validation",
    );
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_agent_alias_account_history_rejects_retired_flat_arguments() {
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
                "name": "iroha.accounts.history",
                "arguments": {
                    "account_id": TEST_ACCOUNT_I105,
                    "limit": 0
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_tool_schema_error(
        &call,
        "retired flat account-history arguments must fail advertised-schema validation",
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
async fn mcp_jsonrpc_tools_call_rejects_retired_one_shot_onboarding_tool() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    enable_writer_mcp(&mut cfg);
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
                    "account_id": TEST_ACCOUNT_I105,
                    "uaid": "uaid:00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        call.pointer("/error/data/error_code")
            .and_then(Value::as_str),
        Some("tool_not_found"),
        "retired one-shot onboarding tool must be rejected before dispatch"
    );
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_account_transactions_query_accepts_canonical_path() {
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
                    "path": { "account_id": TEST_ACCOUNT_I105 },
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
async fn mcp_jsonrpc_tools_call_account_assets_accepts_canonical_path() {
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
                    "path": { "account_id": TEST_ACCOUNT_I105 },
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
async fn mcp_jsonrpc_tools_call_account_assets_query_accepts_canonical_path() {
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
                    "path": { "account_id": TEST_ACCOUNT_I105 },
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
async fn mcp_jsonrpc_tools_call_account_permissions_accepts_canonical_path() {
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
                    "path": { "account_id": TEST_ACCOUNT_I105 }
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
async fn mcp_jsonrpc_tools_call_account_portfolio_accepts_canonical_path() {
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
                    "path": {
                        "uaid": "uaid:00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
                    }
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
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_domains_get_accepts_canonical_path => error(
        1062221,
        "iroha.domains.get",
        InvalidDomainId,
        "invalid domain id should be marked as MCP tool error for domain detail alias",
        "expected invalid domain id to be rejected by explorer domain detail alias",
    )
}
mcp_alias_dispatch_test! {
    #[tokio::test]
    async fn mcp_jsonrpc_tools_call_agent_alias_domains_query_accepts_flat_envelope_fields => success(
        106223,
        "iroha.domains.query",
        LimitTwo,
        "domains query alias with flat envelope fields should dispatch successfully",
    )
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_musubi_v1_query_requires_typed_body() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10622301,
            "method": "tools/call",
            "params": {
                "name": "iroha.musubi.queries.exact_package",
                "arguments": {}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let message = assert_tool_schema_error(
        &call,
        "Musubi V1 queries must reject a missing typed body before dispatch",
    );
    assert!(
        message.contains("body"),
        "missing typed request body should produce a focused error"
    );
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_musubi_v1_rejects_signing_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 106223011,
            "method": "tools/call",
            "params": {
                "name": "iroha.musubi.instructions.release_yank_set",
                "arguments": {
                    "body": {},
                    "private_key": "must-not-be-accepted"
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_tool_schema_error(
        &call,
        "Musubi V1 MCP tools must reject signing material before dispatch",
    );
}
#[tokio::test]
async fn mcp_jsonrpc_tools_call_musubi_v1_yank_instruction_builds_unsigned_payload() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    let app = build_router(cfg);
    let instruction = SetMusubiReleaseYankV1::new(
        MusubiReleaseIdV1::new(
            MusubiPackageIdV1::new(
                DataSpaceId::new(7),
                MusubiPackageScopeV1::DataspaceRoot,
                "swap-core".parse().expect("package name"),
            ),
            "1.2.3".parse().expect("version"),
        ),
        true,
        "bad archive".parse().expect("reason"),
        1,
    );
    let body = norito::json!({
        "release": (norito::json::to_value(&instruction.release).expect("release JSON")),
        "yanked": (instruction.yanked),
        "reason": (norito::json::to_value(&instruction.reason).expect("reason JSON")),
        "expected_yank_revision": (instruction.expected_yank_revision),
    });
    let (status, call) = post_mcp(
        &app,
        norito::json!({
            "jsonrpc": "2.0",
            "id": 10622302,
            "method": "tools/call",
            "params": {
                "name": "iroha.musubi.instructions.release_yank_set",
                "arguments": {
                    "body": body
                }
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !tool_is_error(&call),
        "Musubi yank instruction builder should return an unsigned payload"
    );
    let structured = structured_content(&call);
    assert_eq!(structured.get("status").and_then(Value::as_u64), Some(200));
    let body = structured.get("body").expect("instruction response body");
    let body_object = body.as_object().expect("instruction response body");
    assert_eq!(
        body_object.get("wire_id").and_then(Value::as_str),
        Some(SetMusubiReleaseYankV1::WIRE_ID)
    );
    assert!(
        body_object
            .get("instruction_base64")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty()),
        "instruction base64 should be present"
    );
    assert_eq!(
        body.pointer("/instruction_json/payload/yanked")
            .and_then(Value::as_bool),
        Some(true),
        "instruction preview should expose the exact typed payload"
    );
    assert_eq!(
        body.pointer("/instruction_json/payload/expected_yank_revision")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        !body_object.contains_key("private_key"),
        "instruction builders must not accept or return private keys"
    );
}
#[tokio::test]
async fn mcp_musubi_instruction_schemas_do_not_publish_private_key_fields() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    cfg.torii.mcp.enabled = true;
    cfg.torii.mcp.profile = iroha_config::parameters::actual::ToriiMcpProfile::Operator;
    cfg.torii.mcp.expose_operator_routes = true;
    let app = build_router(cfg);
    for tool_name in [
        "iroha.musubi.instructions.namespace_binding_register",
        "iroha.musubi.instructions.archive_register",
        "iroha.musubi.instructions.provider_bundle_attestation_register",
        "iroha.musubi.instructions.archive_location_add",
        "iroha.musubi.instructions.archive_location_retire",
        "iroha.musubi.instructions.release_publish",
        "iroha.musubi.instructions.release_yank_set",
        "iroha.musubi.instructions.package_metadata_set",
        "iroha.musubi.instructions.package_member_invite",
        "iroha.musubi.instructions.package_member_accept",
        "iroha.musubi.instructions.package_member_invitation_revoke",
        "iroha.musubi.instructions.package_member_set_role",
        "iroha.musubi.instructions.package_member_remove",
        "iroha.musubi.instructions.alias_register",
        "iroha.musubi.instructions.package_recover",
        "iroha.musubi.instructions.alias_retarget",
        "iroha.musubi.instructions.artifact_takedown",
        "iroha.musubi.instructions.registry_policy_set",
        "iroha.musubi.instructions.release_digest_assert",
    ] {
        let tool = find_tool(&app, tool_name).await;
        let schema = tool.get("inputSchema").expect("input schema");
        assert_schema_omits_exact_properties(schema, &["private_key", "authority"], tool_name);
    }
}
include!("mcp_endpoints/extended_tool_dispatch_tests.rs");
#[tokio::test]
async fn mcp_jsonrpc_rejects_retired_connect_aliases_without_dispatch() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    enable_writer_mcp(&mut cfg);
    cfg.torii.connect.enabled = true;
    let app = build_router(cfg);
    for (id, tool_name) in [
        (1078, "connect.session.create"),
        (1079, "connect.session.create_and_ticket"),
        (1080, "iroha.connect.session.create_and_ticket"),
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
                        "network_id": "hash:4141414141414141414141414141414141414141414141414141414141414141#7023",
                        "app_pk": "d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3c",
                        "nonce": "FxcXFxcXFxcXFxcXFxcXFw"
                    }
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            call.pointer("/error/data/error_code")
                .and_then(Value::as_str),
            Some("tool_not_found"),
            "retired tool `{tool_name}` must fail before dispatch"
        );
    }
}
#[tokio::test]
async fn mcp_jsonrpc_connect_session_create_derives_sid_from_exact_identity() {
    let _data_dir = test_utils::TestDataDirGuard::new();
    let mut cfg = test_utils::mk_minimal_root_cfg();
    enable_writer_mcp(&mut cfg);
    cfg.torii.connect.enabled = true;
    let network_id = test_utils::signed_query_network_id().to_string();
    let app = build_router(cfg);
    for (id, tool_name, app_key_byte, nonce_byte) in
        [(2081, "iroha.connect.session.create", 0x42u8, 0x22u8)]
    {
        let app_pk = B64.encode([app_key_byte; 32]);
        let nonce = B64.encode([nonce_byte; 16]);
        let (status, call) = post_mcp(
            &app,
            norito::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": {
                        "network_id": (network_id.clone()),
                        "app_pk": app_pk,
                        "nonce": nonce
                    }
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            !tool_is_error(&call),
            "create alias `{tool_name}` should derive a sid"
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
include!("mcp_endpoints/connect_session_lifecycle_test.rs");
include!("mcp_endpoints/connect_and_registration_tests.rs");
