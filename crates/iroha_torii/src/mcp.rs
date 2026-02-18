//! Native MCP endpoint support for Torii.
//!
//! This module exposes a lightweight JSON-RPC bridge that maps MCP tool calls to
//! existing Torii HTTP routes. Tool definitions are derived from Torii's OpenAPI
//! document so the MCP surface tracks the documented API.

use std::{fmt::Write as _, time::Duration};

use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode, header},
    response::Response,
};
use base64::Engine as _;
use http_body_util::BodyExt as _;
use norito::json::{self, Map, Value};
use tower::ServiceExt as _;

use crate::{SharedAppState, limits, openapi};

const JSONRPC_VERSION: &str = "2.0";
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

const JSONRPC_PARSE_ERROR: i64 = -32700;
const JSONRPC_INVALID_REQUEST: i64 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
const JSONRPC_INVALID_PARAMS: i64 = -32602;
const JSONRPC_INTERNAL_ERROR: i64 = -32603;
const MCP_TOOL_EXECUTION_ERROR: i64 = -32001;
const MCP_RATE_LIMITED: i64 = -32029;

const HEADER_X_API_TOKEN: &str = "x-api-token";
const HEADER_X_IROHA_ACCOUNT: &str = "x-iroha-account";
const HEADER_X_IROHA_SIGNATURE: &str = "x-iroha-signature";
const HEADER_X_IROHA_API_VERSION: &str = "x-iroha-api-version";
const HEADER_X_FORWARDED_PROTO: &str = "x-forwarded-proto";
const DEFAULT_TX_SUBMIT_WAIT_TIMEOUT_MS: u64 = 30_000;
const MAX_TX_SUBMIT_WAIT_TIMEOUT_MS: u64 = 600_000;
const DEFAULT_TX_SUBMIT_WAIT_POLL_INTERVAL_MS: u64 = 500;
const MIN_TX_SUBMIT_WAIT_POLL_INTERVAL_MS: u64 = 50;
const DEFAULT_TX_SUBMIT_WAIT_TERMINAL_STATUSES: &[&str] =
    &["Committed", "Applied", "Rejected", "Expired"];
const SUPPORTED_PIPELINE_STATUS_KINDS: &[&str] = &[
    "Queued",
    "Approved",
    "Committed",
    "Applied",
    "Rejected",
    "Expired",
];

/// OpenAPI-derived tool metadata used for MCP dispatch.
#[derive(Debug, Clone)]
pub(crate) struct ToolSpec {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) method: Method,
    pub(crate) path_template: String,
    pub(crate) input_schema: Value,
}

impl ToolSpec {
    pub(crate) fn descriptor(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("name".into(), Value::String(self.name.clone()));
        obj.insert(
            "description".into(),
            Value::String(self.description.clone()),
        );
        obj.insert("inputSchema".into(), self.input_schema.clone());
        Value::Object(obj)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParameterInfo {
    name: String,
    location: String,
    required: bool,
}

/// Build the MCP tool registry from OpenAPI operations.
pub(crate) fn build_tool_specs(cfg: &iroha_config::parameters::actual::ToriiMcp) -> Vec<ToolSpec> {
    let mut tools = Vec::new();
    let spec = openapi::generate_spec();
    let Some(paths) = spec.get("paths").and_then(Value::as_object) else {
        return tools;
    };

    for (path, path_item) in paths {
        let Some(path_map) = path_item.as_object() else {
            continue;
        };

        let path_parameters = parse_parameters(path_map.get("parameters"));

        for method_key in ["get", "post", "put", "patch", "delete", "head", "options"] {
            let Some(operation) = path_map.get(method_key).and_then(Value::as_object) else {
                continue;
            };
            let Some(method) = method_from_key(method_key) else {
                continue;
            };
            if should_skip_operation(path, operation, cfg.expose_operator_routes) {
                continue;
            }

            let operation_id = operation
                .get("operationId")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| fallback_operation_id(method_key, path));
            let description = operation
                .get("summary")
                .and_then(Value::as_str)
                .or_else(|| operation.get("description").and_then(Value::as_str))
                .unwrap_or("Torii API operation")
                .to_owned();

            let mut parameters = path_parameters.clone();
            parameters.extend(parse_parameters(operation.get("parameters")));
            let input_schema =
                build_input_schema(path, &parameters, operation.get("requestBody").is_some());

            tools.push(ToolSpec {
                name: format!("torii.{operation_id}"),
                description,
                method,
                path_template: path.clone(),
                input_schema,
            });
        }
    }

    tools.push(connect_ws_ticket_tool());
    tools.push(connect_session_create_tool());
    tools.push(connect_session_create_and_ticket_tool());
    tools.push(connect_session_delete_tool());
    tools.push(connect_status_tool());
    tools.push(iroha_connect_ws_ticket_tool());
    tools.push(iroha_connect_session_create_tool());
    tools.push(iroha_connect_session_create_and_ticket_tool());
    tools.push(iroha_connect_session_delete_tool());
    tools.push(iroha_connect_status_tool());
    tools.push(iroha_health_tool());
    tools.push(iroha_status_tool());
    tools.push(iroha_parameters_get_tool());
    tools.push(iroha_node_capabilities_tool());
    tools.push(iroha_contracts_code_register_tool());
    tools.push(iroha_contracts_code_get_tool());
    tools.push(iroha_contracts_deploy_tool());
    tools.push(iroha_contracts_instance_create_tool());
    tools.push(iroha_contracts_instance_activate_tool());
    tools.push(iroha_contracts_call_tool());
    tools.push(iroha_contracts_call_and_wait_tool());
    tools.push(iroha_contracts_state_get_tool());
    tools.push(iroha_accounts_list_tool());
    tools.push(iroha_accounts_get_tool());
    tools.push(iroha_accounts_qr_tool());
    tools.push(iroha_accounts_query_tool());
    tools.push(iroha_accounts_resolve_tool());
    tools.push(iroha_accounts_onboard_tool());
    tools.push(iroha_account_transactions_tool());
    tools.push(iroha_account_transactions_query_tool());
    tools.push(iroha_account_assets_tool());
    tools.push(iroha_account_assets_query_tool());
    tools.push(iroha_account_permissions_tool());
    tools.push(iroha_account_portfolio_tool());
    tools.push(iroha_domains_list_tool());
    tools.push(iroha_domains_get_tool());
    tools.push(iroha_domains_query_tool());
    tools.push(iroha_subscriptions_plans_list_tool());
    tools.push(iroha_subscriptions_plans_create_tool());
    tools.push(iroha_subscriptions_list_tool());
    tools.push(iroha_subscriptions_create_tool());
    tools.push(iroha_subscriptions_get_tool());
    tools.push(iroha_subscriptions_pause_tool());
    tools.push(iroha_subscriptions_resume_tool());
    tools.push(iroha_subscriptions_cancel_tool());
    tools.push(iroha_subscriptions_keep_tool());
    tools.push(iroha_subscriptions_usage_tool());
    tools.push(iroha_subscriptions_charge_now_tool());
    tools.push(iroha_asset_definitions_tool());
    tools.push(iroha_asset_definitions_get_tool());
    tools.push(iroha_asset_definitions_query_tool());
    tools.push(iroha_asset_holders_tool());
    tools.push(iroha_asset_holders_query_tool());
    tools.push(iroha_assets_list_tool());
    tools.push(iroha_assets_get_tool());
    tools.push(iroha_nfts_list_tool());
    tools.push(iroha_nfts_get_tool());
    tools.push(iroha_nfts_query_tool());
    tools.push(iroha_transactions_list_tool());
    tools.push(iroha_transactions_get_tool());
    tools.push(iroha_instructions_list_tool());
    tools.push(iroha_instructions_get_tool());
    tools.push(iroha_blocks_list_tool());
    tools.push(iroha_blocks_get_tool());
    tools.push(iroha_transactions_submit_tool());
    tools.push(iroha_transactions_submit_and_wait_tool());
    tools.push(iroha_transactions_wait_tool());
    tools.push(iroha_transactions_status_tool());

    tools.sort_by(|a, b| a.name.cmp(&b.name));
    tools
}

pub(crate) fn capabilities_payload(tool_count: usize) -> Value {
    let mut server_info = Map::new();
    server_info.insert("name".into(), Value::String("iroha-torii-mcp".to_owned()));
    server_info.insert("version".into(), Value::String("0.0.0-dev".to_owned()));

    let mut tools = Map::new();
    tools.insert("listChanged".into(), Value::Bool(false));
    tools.insert("count".into(), Value::from(tool_count as u64));

    let mut capabilities = Map::new();
    capabilities.insert("tools".into(), Value::Object(tools));

    let mut out = Map::new();
    out.insert(
        "protocolVersion".into(),
        Value::String(MCP_PROTOCOL_VERSION.to_owned()),
    );
    out.insert("serverInfo".into(), Value::Object(server_info));
    out.insert("capabilities".into(), Value::Object(capabilities));
    Value::Object(out)
}

pub(crate) fn jsonrpc_invalid_request(message: &str) -> Value {
    jsonrpc_error_response(None, JSONRPC_INVALID_REQUEST, message, None)
}

pub(crate) fn jsonrpc_parse_error(message: &str) -> Value {
    jsonrpc_error_response(None, JSONRPC_PARSE_ERROR, message, None)
}

pub(crate) fn jsonrpc_rate_limited() -> Value {
    jsonrpc_error_response(
        None,
        MCP_RATE_LIMITED,
        "mcp request rate limited",
        Some(norito::json!({
            "error": "rate_limited"
        })),
    )
}

/// Execute one MCP JSON-RPC request value.
pub(crate) async fn handle_jsonrpc_request(
    app: SharedAppState,
    inbound_headers: &HeaderMap,
    request: Value,
) -> Value {
    let Some(req_obj) = request.as_object() else {
        return jsonrpc_invalid_request("request must be an object");
    };
    if req_obj
        .get("jsonrpc")
        .and_then(Value::as_str)
        .is_some_and(|v| v != JSONRPC_VERSION)
    {
        return jsonrpc_error_response(
            req_obj.get("id").cloned(),
            JSONRPC_INVALID_REQUEST,
            "jsonrpc must be \"2.0\"",
            None,
        );
    }

    let id = req_obj.get("id").cloned();
    let Some(method) = req_obj.get("method").and_then(Value::as_str) else {
        return jsonrpc_error_response(
            id,
            JSONRPC_INVALID_REQUEST,
            "method must be a string",
            None,
        );
    };
    let params = req_obj
        .get("params")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    match method {
        "initialize" => jsonrpc_result_response(id, capabilities_payload(app.mcp_tools.len())),
        "tools/list" => handle_tools_list(id, &app, &params),
        "tools/call" => handle_tools_call(id, app, inbound_headers, &params).await,
        _ => jsonrpc_error_response(
            id,
            JSONRPC_METHOD_NOT_FOUND,
            "method not found",
            Some(norito::json!({ "method": method })),
        ),
    }
}

fn handle_tools_list(id: Option<Value>, app: &SharedAppState, params: &Map) -> Value {
    let start = params
        .get("cursor")
        .and_then(Value::as_str)
        .and_then(|cursor| cursor.parse::<usize>().ok())
        .unwrap_or(0);
    let page_size = app.mcp.max_tools_per_list.max(1);
    let end = start.saturating_add(page_size).min(app.mcp_tools.len());

    let tools = app.mcp_tools[start..end]
        .iter()
        .map(ToolSpec::descriptor)
        .collect::<Vec<_>>();
    let next_cursor = if end < app.mcp_tools.len() {
        Value::String(end.to_string())
    } else {
        Value::Null
    };

    jsonrpc_result_response(
        id,
        norito::json!({
            "tools": tools,
            "nextCursor": next_cursor
        }),
    )
}

async fn handle_tools_call(
    id: Option<Value>,
    app: SharedAppState,
    inbound_headers: &HeaderMap,
    params: &Map,
) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return jsonrpc_error_response(
            id,
            JSONRPC_INVALID_PARAMS,
            "tools/call params.name must be a string",
            None,
        );
    };
    let arguments = params
        .get("arguments")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let tool_result = match name {
        "connect.ws.ticket" | "iroha.connect.ws.ticket" => {
            build_connect_ws_ticket(&arguments, inbound_headers)
                .map(mcp_tool_success)
                .unwrap_or_else(mcp_tool_error)
        }
        "connect.session.create" | "iroha.connect.session.create" => {
            match dispatch_connect_session_create(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "connect.session.create_and_ticket" | "iroha.connect.session.create_and_ticket" => {
            match dispatch_connect_session_create_and_ticket(&app, inbound_headers, &arguments)
                .await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "connect.session.delete" | "iroha.connect.session.delete" => {
            match dispatch_connect_session_delete(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "connect.status" | "iroha.connect.status" => {
            match dispatch_connect_status(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.health" => match dispatch_iroha_health(&app, inbound_headers, &arguments).await {
            Ok(result) => mcp_tool_success(result),
            Err(err) => mcp_tool_error(err),
        },
        "iroha.status" => match dispatch_iroha_status(&app, inbound_headers, &arguments).await {
            Ok(result) => mcp_tool_success(result),
            Err(err) => mcp_tool_error(err),
        },
        "iroha.parameters.get" => {
            match dispatch_iroha_parameters_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.node.capabilities" => {
            match dispatch_iroha_node_capabilities(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.contracts.code.register" => {
            match dispatch_iroha_contracts_code_register(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.contracts.code.get" => {
            match dispatch_iroha_contracts_code_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.contracts.deploy" => {
            match dispatch_iroha_contracts_deploy(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.contracts.instance.create" => {
            match dispatch_iroha_contracts_instance_create(&app, inbound_headers, &arguments).await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.contracts.instance.activate" => {
            match dispatch_iroha_contracts_instance_activate(&app, inbound_headers, &arguments)
                .await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.contracts.call" => {
            match dispatch_iroha_contracts_call(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.contracts.call_and_wait" => {
            match dispatch_iroha_contracts_call_and_wait(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.contracts.state.get" => {
            match dispatch_iroha_contracts_state_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.list" => {
            match dispatch_iroha_accounts_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.get" => {
            match dispatch_iroha_accounts_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.qr" => {
            match dispatch_iroha_accounts_qr(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.query" => {
            match dispatch_iroha_accounts_query(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.resolve" => {
            match dispatch_iroha_accounts_resolve(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.onboard" => {
            match dispatch_iroha_accounts_onboard(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.transactions" => {
            match dispatch_iroha_account_transactions(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.transactions.query" => {
            match dispatch_iroha_account_transactions_query(&app, inbound_headers, &arguments).await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.assets" => {
            match dispatch_iroha_account_assets(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.assets.query" => {
            match dispatch_iroha_account_assets_query(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.permissions" => {
            match dispatch_iroha_account_permissions(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.portfolio" => {
            match dispatch_iroha_account_portfolio(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.domains.list" => {
            match dispatch_iroha_domains_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.domains.get" => {
            match dispatch_iroha_domains_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.domains.query" => {
            match dispatch_iroha_domains_query(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.subscriptions.plans.list" => {
            match dispatch_iroha_subscriptions_plans_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.subscriptions.plans.create" => {
            match dispatch_iroha_subscriptions_plans_create(&app, inbound_headers, &arguments).await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.subscriptions.list" => {
            match dispatch_iroha_subscriptions_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.subscriptions.create" => {
            match dispatch_iroha_subscriptions_create(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.subscriptions.get" => {
            match dispatch_iroha_subscriptions_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.subscriptions.pause" => {
            match dispatch_iroha_subscriptions_pause(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.subscriptions.resume" => {
            match dispatch_iroha_subscriptions_resume(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.subscriptions.cancel" => {
            match dispatch_iroha_subscriptions_cancel(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.subscriptions.keep" => {
            match dispatch_iroha_subscriptions_keep(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.subscriptions.usage" => {
            match dispatch_iroha_subscriptions_usage(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.subscriptions.charge_now" => {
            match dispatch_iroha_subscriptions_charge_now(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.assets.definitions" => {
            match dispatch_iroha_asset_definitions(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.assets.definitions.get" => {
            match dispatch_iroha_asset_definitions_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.assets.definitions.query" => {
            match dispatch_iroha_asset_definitions_query(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.assets.holders" => {
            match dispatch_iroha_asset_holders(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.assets.holders.query" => {
            match dispatch_iroha_asset_holders_query(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.assets.list" => {
            match dispatch_iroha_assets_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.assets.get" => {
            match dispatch_iroha_assets_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.nfts.list" => {
            match dispatch_iroha_nfts_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.nfts.get" => {
            match dispatch_iroha_nfts_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.nfts.query" => {
            match dispatch_iroha_nfts_query(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.transactions.list" => {
            match dispatch_iroha_transactions_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.transactions.get" => {
            match dispatch_iroha_transactions_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.instructions.list" => {
            match dispatch_iroha_instructions_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.instructions.get" => {
            match dispatch_iroha_instructions_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.blocks.list" => {
            match dispatch_iroha_blocks_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.blocks.get" => {
            match dispatch_iroha_blocks_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.transactions.submit" => {
            match dispatch_iroha_transactions_submit(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.transactions.submit_and_wait" => {
            match dispatch_iroha_transactions_submit_and_wait(&app, inbound_headers, &arguments)
                .await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.transactions.wait" => {
            match dispatch_iroha_transactions_wait(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.transactions.status" => {
            match dispatch_iroha_transactions_status(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        _ => match app.mcp_tools.iter().find(|tool| tool.name == name) {
            Some(tool) => {
                match dispatch_openapi_tool(&app, inbound_headers, tool, &arguments).await {
                    Ok(result) => mcp_tool_success(result),
                    Err(err) => mcp_tool_error(err),
                }
            }
            None => {
                return jsonrpc_error_response(
                    id,
                    JSONRPC_INVALID_PARAMS,
                    "tool not found",
                    Some(norito::json!({ "name": name })),
                );
            }
        },
    };

    jsonrpc_result_response(id, tool_result)
}

fn mcp_tool_success(structured: Value) -> Value {
    let status = structured.get("status").and_then(Value::as_u64);
    let is_http_error = status.is_some_and(|code| code >= 400);
    let text = match status {
        Some(code) if is_http_error => format!("http error {code}"),
        Some(code) => format!("http {code}"),
        None => "ok".to_owned(),
    };
    norito::json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "isError": is_http_error,
        "structuredContent": structured
    })
}

fn mcp_tool_error(message: String) -> Value {
    norito::json!({
        "content": [
            {
                "type": "text",
                "text": message
            }
        ],
        "isError": true
    })
}

fn jsonrpc_result_response(id: Option<Value>, result: Value) -> Value {
    let mut obj = Map::new();
    obj.insert("jsonrpc".into(), Value::String(JSONRPC_VERSION.to_owned()));
    obj.insert("id".into(), id.unwrap_or(Value::Null));
    obj.insert("result".into(), result);
    Value::Object(obj)
}

fn jsonrpc_error_response(
    id: Option<Value>,
    code: i64,
    message: &str,
    data: Option<Value>,
) -> Value {
    let mut err = Map::new();
    err.insert("code".into(), Value::from(code));
    err.insert("message".into(), Value::String(message.to_owned()));
    if let Some(data) = data {
        err.insert("data".into(), data);
    }
    let mut obj = Map::new();
    obj.insert("jsonrpc".into(), Value::String(JSONRPC_VERSION.to_owned()));
    obj.insert("id".into(), id.unwrap_or(Value::Null));
    obj.insert("error".into(), Value::Object(err));
    Value::Object(obj)
}

fn parse_parameters(value: Option<&Value>) -> Vec<ParameterInfo> {
    let Some(array) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    array
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|param| {
            let name = param.get("name").and_then(Value::as_str)?;
            let location = param.get("in").and_then(Value::as_str)?;
            let required = param
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Some(ParameterInfo {
                name: name.to_owned(),
                location: location.to_owned(),
                required,
            })
        })
        .collect()
}

fn build_input_schema(path: &str, parameters: &[ParameterInfo], has_request_body: bool) -> Value {
    let mut path_props = Map::new();
    let mut path_required = Vec::new();
    let mut query_props = Map::new();
    let mut header_props = Map::new();

    for param in parameters {
        match param.location.as_str() {
            "path" => {
                path_props.insert(param.name.clone(), string_schema());
                if param.required || path.contains(&format!("{{{}}}", param.name)) {
                    path_required.push(Value::String(param.name.clone()));
                }
            }
            "query" => {
                query_props.insert(param.name.clone(), string_schema());
            }
            "header" => {
                header_props.insert(param.name.clone(), string_schema());
            }
            _ => {}
        }
    }

    let mut properties = Map::new();
    let mut required = Vec::new();

    if !path_props.is_empty() {
        let mut path_schema = Map::new();
        path_schema.insert("type".into(), Value::String("object".to_owned()));
        path_schema.insert("properties".into(), Value::Object(path_props));
        path_schema.insert("additionalProperties".into(), Value::Bool(false));
        if !path_required.is_empty() {
            path_schema.insert("required".into(), Value::Array(path_required));
        }
        properties.insert("path".into(), Value::Object(path_schema));
        required.push(Value::String("path".to_owned()));
    }

    if !query_props.is_empty() {
        let mut query_schema = Map::new();
        query_schema.insert("type".into(), Value::String("object".to_owned()));
        query_schema.insert("properties".into(), Value::Object(query_props));
        query_schema.insert("additionalProperties".into(), Value::Bool(false));
        properties.insert("query".into(), Value::Object(query_schema));
    }

    if !header_props.is_empty() {
        let mut headers_schema = Map::new();
        headers_schema.insert("type".into(), Value::String("object".to_owned()));
        headers_schema.insert("properties".into(), Value::Object(header_props));
        headers_schema.insert("additionalProperties".into(), Value::Bool(true));
        properties.insert("headers".into(), Value::Object(headers_schema));
    } else {
        properties.insert(
            "headers".into(),
            norito::json!({
                "type": "object",
                "additionalProperties": { "type": "string" }
            }),
        );
    }

    if has_request_body {
        properties.insert(
            "body".into(),
            norito::json!({
                "description": "Request body payload. JSON values are encoded as application/json unless `content_type` overrides it."
            }),
        );
        properties.insert(
            "body_base64".into(),
            norito::json!({
                "type": "string",
                "description": "Base64-encoded request body payload for binary formats."
            }),
        );
    }

    properties.insert("content_type".into(), string_schema());
    properties.insert("accept".into(), string_schema());

    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("object".to_owned()));
    schema.insert("properties".into(), Value::Object(properties));
    schema.insert("additionalProperties".into(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert("required".into(), Value::Array(required));
    }
    Value::Object(schema)
}

fn string_schema() -> Value {
    norito::json!({ "type": "string" })
}

fn method_from_key(key: &str) -> Option<Method> {
    match key {
        "get" => Some(Method::GET),
        "post" => Some(Method::POST),
        "put" => Some(Method::PUT),
        "patch" => Some(Method::PATCH),
        "delete" => Some(Method::DELETE),
        "head" => Some(Method::HEAD),
        "options" => Some(Method::OPTIONS),
        _ => None,
    }
}

fn should_skip_operation(path: &str, operation: &Map, expose_operator_routes: bool) -> bool {
    if matches!(
        path,
        "/events" | "/block/stream" | "/p2p" | "/v1/connect/ws" | "/v1/mcp"
    ) {
        return true;
    }
    if path.ends_with("/sse") {
        return true;
    }
    if path.starts_with("/openapi") {
        return true;
    }
    if !expose_operator_routes {
        let has_operator_tag =
            operation
                .get("tags")
                .and_then(Value::as_array)
                .is_some_and(|tags| {
                    tags.iter()
                        .filter_map(Value::as_str)
                        .any(|tag| tag == "OperatorAuth")
                });
        if has_operator_tag || path.starts_with("/v1/operator/") {
            return true;
        }
    }
    false
}

fn fallback_operation_id(method: &str, path: &str) -> String {
    let mut out = String::new();
    out.push_str(method);
    out.push('_');
    for c in path.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_owned()
}

async fn dispatch_openapi_tool(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    tool: &ToolSpec,
    arguments: &Map,
) -> Result<Value, String> {
    let route = fill_path_template(&tool.path_template, arguments.get("path"))?;
    let route = append_query(route, arguments.get("query"))?;
    let (body, content_type) = build_request_body(arguments)?;
    let accept = arguments
        .get("accept")
        .and_then(Value::as_str)
        .map(str::to_owned);

    dispatch_route(
        app,
        inbound_headers,
        tool.method.clone(),
        route.as_str(),
        arguments.get("headers"),
        body,
        content_type,
        accept,
    )
    .await
}

async fn dispatch_connect_session_create(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = arguments.get("body").cloned().unwrap_or_else(|| {
        let mut payload = Map::new();
        if let Some(sid) = arguments.get("sid").and_then(Value::as_str) {
            payload.insert("sid".into(), Value::String(sid.to_owned()));
        }
        let node = arguments
            .get("node")
            .or_else(|| arguments.get("node_url"))
            .and_then(Value::as_str);
        if let Some(node) = node {
            payload.insert("node".into(), Value::String(node.to_owned()));
        }
        Value::Object(payload)
    });
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/connect/session",
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_connect_session_create_and_ticket(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let role = arguments
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| "`role` is required".to_owned())?;
    if role != "app" && role != "wallet" {
        return Err("`role` must be `app` or `wallet`".to_owned());
    }

    let create = dispatch_connect_session_create(app, inbound_headers, arguments).await?;
    let create_status = create.get("status").and_then(Value::as_u64).unwrap_or(0);
    if !(200..300).contains(&create_status) {
        return Ok(create);
    }

    let (sid, token, create_node) = {
        let create_body = create
            .get("body")
            .and_then(Value::as_object)
            .ok_or_else(|| "connect session create response is missing `body` object".to_owned())?;
        let sid = create_body
            .get("sid")
            .and_then(Value::as_str)
            .or_else(|| arguments.get("sid").and_then(Value::as_str))
            .ok_or_else(|| "connect session create response is missing `body.sid`".to_owned())?
            .to_owned();
        let token_key = if role == "app" {
            "token_app"
        } else {
            "token_wallet"
        };
        let token = create_body
            .get(token_key)
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!("connect session create response is missing `body.{token_key}`")
            })?
            .to_owned();
        let create_node = create_body
            .get("node")
            .and_then(Value::as_str)
            .map(str::to_owned);
        (sid, token, create_node)
    };

    let mut ticket_arguments = Map::new();
    ticket_arguments.insert("sid".into(), Value::String(sid.clone()));
    ticket_arguments.insert("role".into(), Value::String(role.to_owned()));
    ticket_arguments.insert("token".into(), Value::String(token.clone()));
    if let Some(node_url) = arguments
        .get("ticket_node_url")
        .or_else(|| arguments.get("node_url"))
        .or_else(|| arguments.get("node"))
        .and_then(Value::as_str)
        .or(create_node.as_deref())
    {
        ticket_arguments.insert("node_url".into(), Value::String(node_url.to_owned()));
    }

    let ticket = build_connect_ws_ticket(&ticket_arguments, inbound_headers)?;
    Ok(norito::json!({
        "status": create_status,
        "sid": sid,
        "role": role,
        "create": create,
        "ticket": ticket
    }))
}

async fn dispatch_connect_session_delete(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let sid = arguments
        .get("sid")
        .and_then(Value::as_str)
        .ok_or_else(|| "`sid` is required".to_owned())?;
    let mut path = String::from("/v1/connect/session/");
    path.push_str(&urlencoding::encode(sid));
    dispatch_route(
        app,
        inbound_headers,
        Method::DELETE,
        path.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_connect_status(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/connect/status",
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_health(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/health",
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_status(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/status",
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_parameters_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/parameters",
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_node_capabilities(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/node/capabilities",
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_contracts_code_register(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_contracts_post(app, inbound_headers, arguments, "/v1/contracts/code").await
}

async fn dispatch_iroha_contracts_code_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let code_hash = extract_code_hash_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("code_hash".into(), Value::String(code_hash));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/contracts/code/{code_hash}", Some(&path_value))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_contracts_deploy(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_contracts_post(app, inbound_headers, arguments, "/v1/contracts/deploy").await
}

async fn dispatch_iroha_contracts_instance_create(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_contracts_post(app, inbound_headers, arguments, "/v1/contracts/instance").await
}

async fn dispatch_iroha_contracts_instance_activate(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_contracts_post(
        app,
        inbound_headers,
        arguments,
        "/v1/contracts/instance/activate",
    )
    .await
}

async fn dispatch_iroha_contracts_call(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_contracts_post(app, inbound_headers, arguments, "/v1/contracts/call").await
}

async fn dispatch_iroha_contracts_call_and_wait(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let timeout_ms = resolve_submit_wait_timeout_ms(arguments)?;
    let poll_interval_ms = resolve_submit_wait_poll_interval_ms(arguments)?;
    let terminal_statuses = resolve_submit_wait_terminal_statuses(arguments)?;

    let submit = dispatch_iroha_contracts_call(app, inbound_headers, arguments).await?;
    let submit_status = submit.get("status").and_then(Value::as_u64).unwrap_or(0);
    if !(200..300).contains(&submit_status) {
        return Ok(submit);
    }

    let tx_hash = extract_optional_transaction_hash_argument(arguments)
        .or_else(|| extract_transaction_hash_from_submit_result(&submit).ok())
        .ok_or_else(|| {
            "could not resolve transaction hash; provide `hash`/`transaction_hash` explicitly"
                .to_owned()
        })?;

    wait_for_terminal_transaction_status(
        app,
        inbound_headers,
        arguments,
        tx_hash,
        Some(submit),
        timeout_ms,
        poll_interval_ms,
        terminal_statuses,
    )
    .await
}

async fn dispatch_iroha_contracts_state_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/contracts/state".to_owned(), query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_contracts_post(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
    route: &str,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        route,
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_accounts_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/accounts".to_owned(), query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_accounts_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let account_id = extract_account_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("account_id".into(), Value::String(account_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/explorer/accounts/{account_id}", Some(&path_value))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_accounts_qr(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let account_id = extract_account_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("account_id".into(), Value::String(account_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/explorer/accounts/{account_id}/qr", Some(&path_value))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_accounts_query(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_query_envelope_body(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/accounts/query",
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_accounts_resolve(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = if let Some(body) = arguments.get("body") {
        body.clone()
    } else {
        let literal = arguments
            .get("literal")
            .or_else(|| arguments.get("account_literal"))
            .or_else(|| arguments.get("account_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                "`literal` is required (or provide `body.literal`) for iroha.accounts.resolve"
                    .to_owned()
            })?;
        norito::json!({ "literal": literal })
    };
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/accounts/resolve",
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_accounts_onboard(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_accounts_onboard_body(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/accounts/onboard",
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_account_transactions(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let account_id = extract_account_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("account_id".into(), Value::String(account_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/accounts/{account_id}/transactions", Some(&path_value))?;
    let query = collect_query_arguments(
        arguments,
        &["path", "account_id", "query", "headers", "accept"],
    )?;
    let route = append_query(route, query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_account_transactions_query(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let account_id = extract_account_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("account_id".into(), Value::String(account_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template(
        "/v1/accounts/{account_id}/transactions/query",
        Some(&path_value),
    )?;
    let body = build_query_envelope_body(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        route.as_str(),
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_account_assets(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let account_id = extract_account_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("account_id".into(), Value::String(account_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/accounts/{account_id}/assets", Some(&path_value))?;
    let query = collect_query_arguments(
        arguments,
        &["path", "account_id", "query", "headers", "accept"],
    )?;
    let route = append_query(route, query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_account_assets_query(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let account_id = extract_account_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("account_id".into(), Value::String(account_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/accounts/{account_id}/assets/query", Some(&path_value))?;
    let body = build_query_envelope_body(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        route.as_str(),
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_account_permissions(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let account_id = extract_account_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("account_id".into(), Value::String(account_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/accounts/{account_id}/permissions", Some(&path_value))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_account_portfolio(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let uaid = extract_uaid_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("uaid".into(), Value::String(uaid));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/accounts/{uaid}/portfolio", Some(&path_value))?;
    let query =
        collect_query_arguments(arguments, &["path", "uaid", "query", "headers", "accept"])?;
    let route = append_query(route, query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_domains_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/domains".to_owned(), query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_domains_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let domain_id = extract_domain_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("domain_id".into(), Value::String(domain_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/explorer/domains/{domain_id}", Some(&path_value))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_domains_query(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_query_envelope_body(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/domains/query",
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_subscriptions_plans_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/subscriptions/plans".to_owned(), query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_subscriptions_plans_create(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_default(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/subscriptions/plans",
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_subscriptions_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/subscriptions".to_owned(), query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_subscriptions_create(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_default(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/subscriptions",
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_subscriptions_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let subscription_id = extract_subscription_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("subscription_id".into(), Value::String(subscription_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/subscriptions/{subscription_id}", Some(&path_value))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_subscriptions_cancel(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_subscription_action(app, inbound_headers, arguments, "cancel").await
}

async fn dispatch_iroha_subscriptions_pause(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_subscription_action(app, inbound_headers, arguments, "pause").await
}

async fn dispatch_iroha_subscriptions_resume(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_subscription_action(app, inbound_headers, arguments, "resume").await
}

async fn dispatch_iroha_subscriptions_keep(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_subscription_action(app, inbound_headers, arguments, "keep").await
}

async fn dispatch_iroha_subscriptions_usage(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_subscription_action(app, inbound_headers, arguments, "usage").await
}

async fn dispatch_iroha_subscriptions_charge_now(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_subscription_action(app, inbound_headers, arguments, "charge-now").await
}

async fn dispatch_iroha_subscription_action(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
    action: &str,
) -> Result<Value, String> {
    let subscription_id = extract_subscription_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("subscription_id".into(), Value::String(subscription_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template(
        format!("/v1/subscriptions/{{subscription_id}}/{action}").as_str(),
        Some(&path_value),
    )?;
    let body = build_object_body_or_default(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        route.as_str(),
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_asset_definitions(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/assets/definitions".to_owned(), query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_asset_definitions_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let definition_id = extract_definition_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("definition_id".into(), Value::String(definition_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template(
        "/v1/explorer/asset-definitions/{definition_id}",
        Some(&path_value),
    )?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_asset_definitions_query(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_query_envelope_body(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/assets/definitions/query",
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_asset_holders(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let definition_id = extract_definition_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("definition_id".into(), Value::String(definition_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/assets/{definition_id}/holders", Some(&path_value))?;
    let query = collect_query_arguments(
        arguments,
        &["path", "definition_id", "query", "headers", "accept"],
    )?;
    let route = append_query(route, query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_asset_holders_query(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let definition_id = extract_definition_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("definition_id".into(), Value::String(definition_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template(
        "/v1/assets/{definition_id}/holders/query",
        Some(&path_value),
    )?;
    let body = build_query_envelope_body(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        route.as_str(),
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_assets_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/explorer/assets".to_owned(), query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_assets_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let asset_id = extract_asset_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("asset_id".into(), Value::String(asset_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/explorer/assets/{asset_id}", Some(&path_value))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_nfts_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/explorer/nfts".to_owned(), query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_nfts_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let nft_id = extract_nft_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("nft_id".into(), Value::String(nft_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/explorer/nfts/{nft_id}", Some(&path_value))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_nfts_query(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_query_envelope_body(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/nfts/query",
        arguments.get("headers"),
        body_bytes,
        Some("application/json".to_owned()),
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_transactions_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/explorer/transactions".to_owned(), query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_transactions_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let hash = extract_transaction_hash_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("hash".into(), Value::String(hash));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/explorer/transactions/{hash}", Some(&path_value))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_instructions_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/explorer/instructions".to_owned(), query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_instructions_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let hash = extract_transaction_hash_argument(arguments)?;
    let index = extract_instruction_index_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("hash".into(), Value::String(hash));
    path_args.insert("index".into(), Value::String(index));
    let path_value = Value::Object(path_args);
    let route = fill_path_template(
        "/v1/explorer/instructions/{hash}/{index}",
        Some(&path_value),
    )?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_blocks_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/explorer/blocks".to_owned(), query.as_ref())?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_blocks_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let identifier = extract_block_identifier_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("identifier".into(), Value::String(identifier));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/explorer/blocks/{identifier}", Some(&path_value))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_transactions_submit(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let mut adapted = arguments.clone();
    if !adapted.contains_key("body_base64") && !adapted.contains_key("body") {
        if let Some(encoded) = arguments
            .get("signed_tx_base64")
            .or_else(|| arguments.get("tx_base64"))
            .and_then(Value::as_str)
        {
            adapted.insert("body_base64".into(), Value::String(encoded.to_owned()));
        } else if let Some(encoded_hex) = arguments
            .get("body_hex")
            .or_else(|| arguments.get("signed_tx_hex"))
            .or_else(|| arguments.get("tx_hex"))
            .and_then(Value::as_str)
        {
            let bytes = hex::decode(encoded_hex)
                .map_err(|err| format!("transaction hex payload must be valid hex: {err}"))?;
            adapted.insert(
                "body_base64".into(),
                Value::String(base64::engine::general_purpose::STANDARD.encode(bytes)),
            );
        }
    }

    if !adapted.contains_key("body_base64") && !adapted.contains_key("body") {
        return Err("one of `body_base64`, `signed_tx_base64`, `tx_base64`, `body_hex`, `signed_tx_hex`, `tx_hex`, or `body` is required".to_owned());
    }

    let (body, content_type) = build_request_body(&adapted)?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/transaction",
        adapted.get("headers"),
        body,
        content_type,
        adapted
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_transactions_submit_and_wait(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let timeout_ms = resolve_submit_wait_timeout_ms(arguments)?;
    let poll_interval_ms = resolve_submit_wait_poll_interval_ms(arguments)?;
    let terminal_statuses = resolve_submit_wait_terminal_statuses(arguments)?;

    let submit = dispatch_iroha_transactions_submit(app, inbound_headers, arguments).await?;
    let submit_status = submit.get("status").and_then(Value::as_u64).unwrap_or(0);
    if !(200..300).contains(&submit_status) {
        return Ok(submit);
    }

    let tx_hash = extract_optional_transaction_hash_argument(arguments)
        .or_else(|| extract_transaction_hash_from_submit_result(&submit).ok())
        .ok_or_else(|| {
            "could not resolve transaction hash; provide `hash`/`transaction_hash` explicitly"
                .to_owned()
        })?;

    wait_for_terminal_transaction_status(
        app,
        inbound_headers,
        arguments,
        tx_hash,
        Some(submit),
        timeout_ms,
        poll_interval_ms,
        terminal_statuses,
    )
    .await
}

async fn dispatch_iroha_transactions_wait(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let timeout_ms = resolve_submit_wait_timeout_ms(arguments)?;
    let poll_interval_ms = resolve_submit_wait_poll_interval_ms(arguments)?;
    let terminal_statuses = resolve_submit_wait_terminal_statuses(arguments)?;
    let tx_hash = extract_optional_transaction_hash_argument(arguments).ok_or_else(|| {
        "`hash` is required (provide `hash`, `transaction_hash`, or `query.hash`)".to_owned()
    })?;

    wait_for_terminal_transaction_status(
        app,
        inbound_headers,
        arguments,
        tx_hash,
        None,
        timeout_ms,
        poll_interval_ms,
        terminal_statuses,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn wait_for_terminal_transaction_status(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
    tx_hash: String,
    submit: Option<Value>,
    timeout_ms: u64,
    poll_interval_ms: u64,
    terminal_statuses: Vec<String>,
) -> Result<Value, String> {
    let start = tokio::time::Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let poll_interval = Duration::from_millis(poll_interval_ms);
    let mut attempts = 0_u64;
    let mut last_kind: Option<String> = None;
    let status_accept = arguments
        .get("status_accept")
        .or_else(|| arguments.get("accept"))
        .and_then(Value::as_str)
        .unwrap_or("application/json")
        .to_owned();

    loop {
        attempts = attempts.saturating_add(1);

        let mut status_arguments = Map::new();
        status_arguments.insert("hash".into(), Value::String(tx_hash.clone()));
        status_arguments.insert("accept".into(), Value::String(status_accept.clone()));
        if let Some(headers) = arguments.get("headers") {
            status_arguments.insert("headers".into(), headers.clone());
        }

        let status_result =
            dispatch_iroha_transactions_status(app, inbound_headers, &status_arguments).await?;
        let status_code = status_result
            .get("status")
            .and_then(Value::as_u64)
            .unwrap_or(0);

        if (200..300).contains(&status_code) {
            let kind = extract_pipeline_status_kind(&status_result).ok_or_else(|| {
                "status polling response is missing `body.content.status.kind`".to_owned()
            })?;
            last_kind = Some(kind.to_owned());
            if is_terminal_pipeline_status(kind, &terminal_statuses) {
                let mut out = Map::new();
                out.insert("status".into(), Value::from(status_code));
                out.insert("hash".into(), Value::String(tx_hash.clone()));
                out.insert("terminal_kind".into(), Value::String(kind.to_owned()));
                out.insert(
                    "terminal_statuses".into(),
                    Value::Array(
                        terminal_statuses
                            .iter()
                            .cloned()
                            .map(Value::String)
                            .collect(),
                    ),
                );
                out.insert("attempts".into(), Value::from(attempts));
                out.insert(
                    "elapsed_ms".into(),
                    Value::from(
                        start
                            .elapsed()
                            .as_millis()
                            .min(u128::from(u64::MAX))
                            .try_into()
                            .unwrap_or(u64::MAX),
                    ),
                );
                if let Some(submit) = submit.as_ref() {
                    out.insert("submit".into(), submit.clone());
                }
                out.insert("final".into(), status_result);
                return Ok(Value::Object(out));
            }
        } else {
            let retryable_http = status_code == 404 || status_code == 429 || status_code >= 500;
            if let Some(kind) = extract_pipeline_status_kind(&status_result) {
                last_kind = Some(kind.to_owned());
            }
            if !retryable_http {
                return Ok(status_result);
            }
        }

        if start.elapsed() >= timeout {
            break;
        }
        let remaining = timeout.saturating_sub(start.elapsed());
        tokio::time::sleep(poll_interval.min(remaining)).await;
    }

    let last_kind = last_kind
        .map(|kind| format!(" (last status kind: `{kind}`)"))
        .unwrap_or_default();
    Err(format!(
        "timed out waiting for terminal transaction status after {timeout_ms}ms for `{tx_hash}`{last_kind}"
    ))
}

async fn dispatch_iroha_transactions_status(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let mut query = collect_query_map(arguments, &["query", "headers", "accept", "hash"])?;
    if !query
        .get("hash")
        .and_then(Value::as_str)
        .is_some_and(|hash| !hash.is_empty())
    {
        if let Some(hash) = arguments.get("hash").and_then(Value::as_str) {
            query.insert("hash".into(), Value::String(hash.to_owned()));
        }
    }
    if !query
        .get("hash")
        .and_then(Value::as_str)
        .is_some_and(|hash| !hash.is_empty())
    {
        return Err("`hash` is required (provide `hash` or `query.hash`)".to_owned());
    }

    let query_value = Value::Object(query);
    let route = append_query(
        "/v1/pipeline/transactions/status".to_owned(),
        Some(&query_value),
    )?;
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        route.as_str(),
        arguments.get("headers"),
        Vec::new(),
        None,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

fn resolve_submit_wait_timeout_ms(arguments: &Map) -> Result<u64, String> {
    let timeout_ms = match arguments.get("timeout_ms") {
        Some(value) => value
            .as_u64()
            .ok_or_else(|| "`timeout_ms` must be an unsigned integer".to_owned())?,
        None => DEFAULT_TX_SUBMIT_WAIT_TIMEOUT_MS,
    };
    if timeout_ms == 0 {
        return Err("`timeout_ms` must be greater than zero".to_owned());
    }
    if timeout_ms > MAX_TX_SUBMIT_WAIT_TIMEOUT_MS {
        return Err(format!(
            "`timeout_ms` must be <= {MAX_TX_SUBMIT_WAIT_TIMEOUT_MS}"
        ));
    }
    Ok(timeout_ms)
}

fn resolve_submit_wait_poll_interval_ms(arguments: &Map) -> Result<u64, String> {
    let poll_interval_ms = match arguments.get("poll_interval_ms") {
        Some(value) => value
            .as_u64()
            .ok_or_else(|| "`poll_interval_ms` must be an unsigned integer".to_owned())?,
        None => DEFAULT_TX_SUBMIT_WAIT_POLL_INTERVAL_MS,
    };
    if poll_interval_ms < MIN_TX_SUBMIT_WAIT_POLL_INTERVAL_MS {
        return Err(format!(
            "`poll_interval_ms` must be >= {MIN_TX_SUBMIT_WAIT_POLL_INTERVAL_MS}"
        ));
    }
    Ok(poll_interval_ms)
}

fn resolve_submit_wait_terminal_statuses(arguments: &Map) -> Result<Vec<String>, String> {
    let statuses = match arguments.get("terminal_statuses") {
        Some(value) => {
            let array = value
                .as_array()
                .ok_or_else(|| "`terminal_statuses` must be an array of strings".to_owned())?;
            if array.is_empty() {
                return Err("`terminal_statuses` must not be empty".to_owned());
            }
            let mut out = Vec::with_capacity(array.len());
            for value in array {
                let status = value
                    .as_str()
                    .ok_or_else(|| "`terminal_statuses` must be an array of strings".to_owned())?;
                if !SUPPORTED_PIPELINE_STATUS_KINDS
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(status))
                {
                    return Err(format!(
                        "unsupported terminal status `{status}` (supported: {})",
                        SUPPORTED_PIPELINE_STATUS_KINDS.join(", ")
                    ));
                }
                out.push(status.to_owned());
            }
            out
        }
        None => DEFAULT_TX_SUBMIT_WAIT_TERMINAL_STATUSES
            .iter()
            .map(|status| (*status).to_owned())
            .collect(),
    };
    Ok(statuses)
}

fn extract_optional_transaction_hash_argument(arguments: &Map) -> Option<String> {
    if let Some(query) = arguments.get("query").and_then(Value::as_object)
        && let Some(hash) = query.get("hash").and_then(Value::as_str)
        && !hash.is_empty()
    {
        return Some(hash.to_owned());
    }

    arguments
        .get("hash")
        .or_else(|| arguments.get("transaction_hash"))
        .and_then(Value::as_str)
        .filter(|hash| !hash.is_empty())
        .map(str::to_owned)
}

fn extract_transaction_hash_from_submit_result(submit_result: &Value) -> Result<String, String> {
    let status = submit_result
        .get("status")
        .and_then(Value::as_u64)
        .ok_or_else(|| "submit response missing HTTP status".to_owned())?;
    if !(200..300).contains(&status) {
        return Err(format!(
            "submit response status `{status}` is not successful"
        ));
    }

    let body = submit_result
        .get("body")
        .ok_or_else(|| "submit response missing `body`".to_owned())?;

    if let Some(hash) = body
        .get("payload")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("tx_hash"))
        .and_then(Value::as_str)
        .or_else(|| body.get("tx_hash_hex").and_then(Value::as_str))
        .or_else(|| body.get("tx_hash").and_then(Value::as_str))
        .or_else(|| body.get("transaction_hash").and_then(Value::as_str))
        .or_else(|| body.get("hash").and_then(Value::as_str))
        .filter(|hash| !hash.is_empty())
    {
        return Ok(hash.to_owned());
    }

    if let Some(encoded_receipt) = body.as_str() {
        if let Some(hash) = decode_transaction_hash_from_receipt_base64(encoded_receipt) {
            return Ok(hash);
        }
    }

    Err("unable to extract transaction hash from submission response".to_owned())
}

fn decode_transaction_hash_from_receipt_base64(encoded_receipt: &str) -> Option<String> {
    let bytes = decode_base64_any(encoded_receipt)?;
    let receipt: iroha_data_model::transaction::TransactionSubmissionReceipt =
        norito::decode_from_bytes(&bytes).ok()?;
    Some(receipt.payload.tx_hash.to_string())
}

fn extract_pipeline_status_kind(status_result: &Value) -> Option<&str> {
    let body = status_result.get("body")?;
    body.get("content")
        .and_then(Value::as_object)
        .and_then(|content| content.get("status"))
        .and_then(Value::as_object)
        .and_then(|status| status.get("kind"))
        .and_then(Value::as_str)
        .or_else(|| {
            body.get("status")
                .and_then(Value::as_object)
                .and_then(|status| status.get("kind"))
                .and_then(Value::as_str)
        })
}

fn is_terminal_pipeline_status(status: &str, terminal_statuses: &[String]) -> bool {
    terminal_statuses
        .iter()
        .any(|terminal| terminal.eq_ignore_ascii_case(status))
}

fn extract_account_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(account_id) = path.get("account_id").and_then(Value::as_str) {
            return Ok(account_id.to_owned());
        }
    }
    arguments
        .get("account_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`account_id` is required (provide `account_id` or `path.account_id`)".to_owned()
        })
}

fn extract_uaid_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(uaid) = path.get("uaid").and_then(Value::as_str) {
            return Ok(uaid.to_owned());
        }
    }
    arguments
        .get("uaid")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "`uaid` is required (provide `uaid` or `path.uaid`)".to_owned())
}

fn extract_domain_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(domain_id) = path.get("domain_id").and_then(Value::as_str) {
            return Ok(domain_id.to_owned());
        }
    }
    arguments
        .get("domain_id")
        .or_else(|| arguments.get("domain"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`domain_id` is required (provide `domain_id`, `domain`, or `path.domain_id`)"
                .to_owned()
        })
}

fn extract_subscription_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(subscription_id) = path.get("subscription_id").and_then(Value::as_str) {
            return Ok(subscription_id.to_owned());
        }
    }
    arguments
        .get("subscription_id")
        .or_else(|| arguments.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`subscription_id` is required (provide `subscription_id`, `id`, or `path.subscription_id`)".to_owned()
        })
}

fn extract_definition_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(definition_id) = path.get("definition_id").and_then(Value::as_str) {
            return Ok(definition_id.to_owned());
        }
    }
    arguments
        .get("definition_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`definition_id` is required (provide `definition_id` or `path.definition_id`)"
                .to_owned()
        })
}

fn extract_asset_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(asset_id) = path.get("asset_id").and_then(Value::as_str) {
            return Ok(asset_id.to_owned());
        }
    }
    arguments
        .get("asset_id")
        .or_else(|| arguments.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`asset_id` is required (provide `asset_id`, `id`, or `path.asset_id`)".to_owned()
        })
}

fn extract_nft_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(nft_id) = path.get("nft_id").and_then(Value::as_str) {
            return Ok(nft_id.to_owned());
        }
    }
    arguments
        .get("nft_id")
        .or_else(|| arguments.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "`nft_id` is required (provide `nft_id`, `id`, or `path.nft_id`)".to_owned())
}

fn extract_transaction_hash_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(hash) = path.get("hash").and_then(Value::as_str) {
            return Ok(hash.to_owned());
        }
    }
    arguments
        .get("hash")
        .or_else(|| arguments.get("transaction_hash"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`hash` is required (provide `hash`, `transaction_hash`, or `path.hash`)".to_owned()
        })
}

fn extract_code_hash_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(code_hash) = path.get("code_hash").and_then(Value::as_str) {
            return Ok(code_hash.to_owned());
        }
    }
    arguments
        .get("code_hash")
        .or_else(|| arguments.get("hash"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`code_hash` is required (provide `code_hash`, `hash`, or `path.code_hash`)".to_owned()
        })
}

fn extract_instruction_index_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(index) = path.get("index").and_then(value_to_string) {
            return Ok(index);
        }
    }
    arguments
        .get("index")
        .or_else(|| arguments.get("instruction_index"))
        .and_then(value_to_string)
        .ok_or_else(|| {
            "`index` is required (provide `index`, `instruction_index`, or `path.index`)".to_owned()
        })
}

fn extract_block_identifier_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(identifier) = path.get("identifier").and_then(value_to_string) {
            return Ok(identifier);
        }
    }
    arguments
        .get("identifier")
        .or_else(|| arguments.get("block_identifier"))
        .or_else(|| arguments.get("block_height"))
        .or_else(|| arguments.get("block_hash"))
        .and_then(value_to_string)
        .ok_or_else(|| {
            "`identifier` is required (provide `identifier`, `block_identifier`, `block_height`, `block_hash`, or `path.identifier`)".to_owned()
        })
}

fn build_query_envelope_body(arguments: &Map) -> Result<Value, String> {
    if let Some(body) = arguments.get("body") {
        return body
            .as_object()
            .map(|_| body.clone())
            .ok_or_else(|| "`body` must be an object".to_owned());
    }

    let mut env = Map::new();
    for key in [
        "query",
        "filter",
        "select",
        "sort",
        "fetch_size",
        "address_format",
    ] {
        if let Some(value) = arguments.get(key) {
            env.insert(key.to_owned(), value.clone());
        }
    }

    if let Some(pagination) = arguments.get("pagination") {
        let pagination_obj = pagination
            .as_object()
            .ok_or_else(|| "`pagination` must be an object".to_owned())?;
        env.insert(
            "pagination".to_owned(),
            Value::Object(pagination_obj.clone()),
        );
    } else {
        let mut pagination = Map::new();
        if let Some(limit) = arguments.get("limit") {
            pagination.insert("limit".to_owned(), limit.clone());
        }
        if let Some(offset) = arguments.get("offset") {
            pagination.insert("offset".to_owned(), offset.clone());
        }
        if !pagination.is_empty() {
            env.insert("pagination".to_owned(), Value::Object(pagination));
        }
    }

    Ok(Value::Object(env))
}

fn build_object_body_or_default(arguments: &Map) -> Result<Value, String> {
    if let Some(body) = arguments.get("body") {
        return body
            .as_object()
            .map(|_| body.clone())
            .ok_or_else(|| "`body` must be an object".to_owned());
    }
    Ok(Value::Object(Map::new()))
}

fn build_object_body_or_flat_shortcuts(
    arguments: &Map,
    ignored_keys: &[&str],
) -> Result<Value, String> {
    if let Some(body) = arguments.get("body") {
        return body
            .as_object()
            .map(|_| body.clone())
            .ok_or_else(|| "`body` must be an object".to_owned());
    }

    let mut payload = Map::new();
    for (key, value) in arguments {
        if ignored_keys.iter().any(|ignored| key == ignored) {
            continue;
        }
        if value.is_null() {
            continue;
        }
        payload.insert(key.clone(), value.clone());
    }

    if payload.is_empty() {
        return Err("`body` is required (or provide flat top-level fields)".to_owned());
    }

    Ok(Value::Object(payload))
}

fn build_accounts_onboard_body(arguments: &Map) -> Result<Value, String> {
    if let Some(body) = arguments.get("body") {
        return body
            .as_object()
            .map(|_| body.clone())
            .ok_or_else(|| "`body` must be an object".to_owned());
    }

    let alias = arguments
        .get("alias")
        .and_then(Value::as_str)
        .ok_or_else(|| "`alias` is required (or provide `body.alias`)".to_owned())?;
    let account_id = arguments
        .get("account_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "`account_id` is required (or provide `body.account_id`)".to_owned())?;

    let mut payload = Map::new();
    payload.insert("alias".to_owned(), Value::String(alias.to_owned()));
    payload.insert(
        "account_id".to_owned(),
        Value::String(account_id.to_owned()),
    );

    if let Some(identity) = arguments.get("identity") {
        let identity_obj = identity
            .as_object()
            .ok_or_else(|| "`identity` must be an object when provided".to_owned())?;
        payload.insert("identity".to_owned(), Value::Object(identity_obj.clone()));
    }
    if let Some(uaid) = arguments.get("uaid").and_then(Value::as_str) {
        payload.insert("uaid".to_owned(), Value::String(uaid.to_owned()));
    }

    Ok(Value::Object(payload))
}

fn collect_query_arguments(
    arguments: &Map,
    ignored_keys: &[&str],
) -> Result<Option<Value>, String> {
    let query = collect_query_map(arguments, ignored_keys)?;
    if query.is_empty() {
        return Ok(None);
    }
    Ok(Some(Value::Object(query)))
}

fn collect_query_map(arguments: &Map, ignored_keys: &[&str]) -> Result<Map, String> {
    if let Some(query) = arguments.get("query") {
        return query
            .as_object()
            .cloned()
            .ok_or_else(|| "`query` must be an object".to_owned());
    }

    let mut query = Map::new();
    for (key, value) in arguments {
        if ignored_keys.iter().any(|ignored| key == ignored) {
            continue;
        }
        if value.is_null() {
            continue;
        }
        query.insert(key.clone(), value.clone());
    }
    Ok(query)
}

#[allow(clippy::too_many_arguments)]
async fn dispatch_route(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    method: Method,
    path_and_query: &str,
    extra_headers: Option<&Value>,
    body: Vec<u8>,
    content_type: Option<String>,
    accept: Option<String>,
) -> Result<Value, String> {
    let mut request = Request::builder()
        .method(method)
        .uri(path_and_query)
        .body(Body::from(body))
        .map_err(|err| format!("build request: {err}"))?;

    {
        let headers = request.headers_mut();
        forward_auth_headers(headers, inbound_headers);
        apply_extra_headers(headers, extra_headers)?;
        if let Some(accept_value) = accept {
            let value = HeaderValue::from_str(&accept_value)
                .map_err(|err| format!("invalid accept header: {err}"))?;
            headers.insert(header::ACCEPT, value);
        }
        if let Some(content_type_value) = content_type {
            let value = HeaderValue::from_str(&content_type_value)
                .map_err(|err| format!("invalid content_type header: {err}"))?;
            headers.insert(header::CONTENT_TYPE, value);
        }
        headers.insert(
            HeaderName::from_static(limits::REMOTE_ADDR_HEADER),
            HeaderValue::from_static("127.0.0.1"),
        );
    }

    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            0,
        ))));

    let router = {
        let guard = app
            .mcp_dispatch_router
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard
            .clone()
            .ok_or_else(|| "mcp router unavailable".to_owned())?
    };

    let response = router
        .with_state(app.clone())
        .oneshot(request)
        .await
        .map_err(|err| format!("dispatch failed: {err}"))?;
    response_to_value(response).await
}

fn build_request_body(arguments: &Map) -> Result<(Vec<u8>, Option<String>), String> {
    if let Some(encoded) = arguments.get("body_base64").and_then(Value::as_str) {
        let bytes = decode_base64_any(encoded)
            .ok_or_else(|| "body_base64 must be valid base64/base64url".to_owned())?;
        let content_type = arguments
            .get("content_type")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| Some(crate::utils::NORITO_MIME_TYPE.to_owned()));
        return Ok((bytes, content_type));
    }

    if let Some(body_value) = arguments.get("body") {
        let bytes = json::to_vec(body_value).map_err(|err| format!("encode body: {err}"))?;
        let content_type = arguments
            .get("content_type")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| Some("application/json".to_owned()));
        return Ok((bytes, content_type));
    }

    Ok((Vec::new(), None))
}

fn decode_base64_any(input: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .ok()
        .or_else(|| base64::engine::general_purpose::URL_SAFE.decode(input).ok())
        .or_else(|| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(input)
                .ok()
        })
}

fn fill_path_template(path_template: &str, path_args: Option<&Value>) -> Result<String, String> {
    let args = path_args
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut out = String::with_capacity(path_template.len() + 16);
    let mut chars = path_template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '{' {
            out.push(ch);
            continue;
        }
        let mut key = String::new();
        while let Some(next) = chars.next() {
            if next == '}' {
                break;
            }
            key.push(next);
        }
        if key.is_empty() {
            return Err("invalid path template placeholder".to_owned());
        }
        let value = args
            .get(&key)
            .and_then(value_to_string)
            .ok_or_else(|| format!("missing required path argument `{key}`"))?;
        out.push_str(&urlencoding::encode(&value));
    }

    Ok(out)
}

fn append_query(path: String, query: Option<&Value>) -> Result<String, String> {
    let Some(map) = query.and_then(Value::as_object) else {
        return Ok(path);
    };
    if map.is_empty() {
        return Ok(path);
    }
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in map {
        if value.is_null() {
            continue;
        }
        let value =
            value_to_string(value).ok_or_else(|| format!("invalid query value for `{key}`"))?;
        serializer.append_pair(key, &value);
    }
    let encoded = serializer.finish();
    if encoded.is_empty() {
        return Ok(path);
    }
    Ok(format!("{path}?{encoded}"))
}

fn value_to_string(value: &Value) -> Option<String> {
    if value.is_null() {
        return None;
    }
    if let Some(s) = value.as_str() {
        return Some(s.to_owned());
    }
    if let Some(i) = value.as_i64() {
        return Some(i.to_string());
    }
    if let Some(u) = value.as_u64() {
        return Some(u.to_string());
    }
    if let Some(f) = value.as_f64() {
        return Some(f.to_string());
    }
    if let Some(b) = value.as_bool() {
        return Some(b.to_string());
    }
    json::to_string(value).ok()
}

fn forward_auth_headers(out: &mut HeaderMap, inbound: &HeaderMap) {
    for header_name in [
        header::AUTHORIZATION,
        HeaderName::from_static(HEADER_X_API_TOKEN),
        HeaderName::from_static(HEADER_X_IROHA_ACCOUNT),
        HeaderName::from_static(HEADER_X_IROHA_SIGNATURE),
        HeaderName::from_static(HEADER_X_IROHA_API_VERSION),
    ] {
        if let Some(value) = inbound.get(&header_name) {
            out.insert(header_name, value.clone());
        }
    }
}

fn apply_extra_headers(out: &mut HeaderMap, value: Option<&Value>) -> Result<(), String> {
    let Some(headers_obj) = value.and_then(Value::as_object) else {
        return Ok(());
    };

    for (raw_name, raw_value) in headers_obj {
        let lowered = raw_name.to_ascii_lowercase();
        if lowered == "content-length" || lowered == "host" || lowered == "connection" {
            continue;
        }
        let header_name: HeaderName = raw_name
            .parse()
            .map_err(|err| format!("invalid header name `{raw_name}`: {err}"))?;
        let header_value = value_to_string(raw_value)
            .ok_or_else(|| format!("invalid header value for `{raw_name}`"))?;
        let header_value = HeaderValue::from_str(&header_value)
            .map_err(|err| format!("invalid header value for `{raw_name}`: {err}"))?;
        out.insert(header_name, header_value);
    }
    Ok(())
}

async fn response_to_value(response: Response) -> Result<Value, String> {
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|err| format!("read response body: {err}"))?
        .to_bytes();

    let headers_value = headers_to_value(&headers);
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body_value = decode_response_body(&body_bytes, content_type.as_deref());

    let mut structured = Map::new();
    structured.insert("status".into(), Value::from(u64::from(status.as_u16())));
    structured.insert("headers".into(), headers_value);
    structured.insert(
        "content_type".into(),
        content_type.map(Value::String).unwrap_or(Value::Null),
    );
    structured.insert("body".into(), body_value);

    Ok(Value::Object(structured))
}

fn headers_to_value(headers: &HeaderMap) -> Value {
    let mut out = Map::new();
    for (name, value) in headers {
        if let Ok(as_str) = value.to_str() {
            out.insert(name.as_str().to_owned(), Value::String(as_str.to_owned()));
        }
    }
    Value::Object(out)
}

fn decode_response_body(bytes: &[u8], content_type: Option<&str>) -> Value {
    if bytes.is_empty() {
        return Value::Null;
    }
    if content_type.is_some_and(|ct| ct.to_ascii_lowercase().contains("json"))
        && let Ok(value) = json::from_slice::<Value>(bytes)
    {
        return value;
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Value::String(text.to_owned());
    }
    Value::String(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn build_connect_ws_ticket(arguments: &Map, inbound_headers: &HeaderMap) -> Result<Value, String> {
    let sid = arguments
        .get("sid")
        .and_then(Value::as_str)
        .ok_or_else(|| "`sid` is required".to_owned())?;
    let role = arguments
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| "`role` is required".to_owned())?;
    if role != "app" && role != "wallet" {
        return Err("`role` must be `app` or `wallet`".to_owned());
    }
    let token = arguments
        .get("token")
        .and_then(Value::as_str)
        .or_else(|| match role {
            "app" => arguments.get("token_app").and_then(Value::as_str),
            "wallet" => arguments.get("token_wallet").and_then(Value::as_str),
            _ => None,
        })
        .ok_or_else(|| {
            "`token` is required (or provide `token_app`/`token_wallet` matching `role`)".to_owned()
        })?;
    let node = arguments
        .get("node_url")
        .or_else(|| arguments.get("node"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| infer_node_url(inbound_headers));

    let mut url = parse_node_url(&node)?;
    url.set_path("/v1/connect/ws");
    {
        let mut query = url.query_pairs_mut();
        query.clear();
        query.append_pair("sid", sid);
        query.append_pair("role", role);
    }

    let protocol_token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token.as_bytes());

    Ok(norito::json!({
        "ws_url": (url.to_string()),
        "authorization_header": (format!("Bearer {token}")),
        "sec_websocket_protocol": (format!("iroha-connect.token.v1.{protocol_token}"))
    }))
}

fn infer_node_url(inbound_headers: &HeaderMap) -> String {
    let host = inbound_headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("127.0.0.1:8080");
    let proto = inbound_headers
        .get(HEADER_X_FORWARDED_PROTO)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("http");
    format!("{proto}://{host}")
}

fn parse_node_url(raw: &str) -> Result<url::Url, String> {
    let parsed = if raw.contains("://") {
        url::Url::parse(raw)
    } else {
        let mut with_scheme = String::from("http://");
        with_scheme.push_str(raw);
        url::Url::parse(&with_scheme)
    }
    .map_err(|err| format!("invalid node url `{raw}`: {err}"))?;

    let mut url = parsed;
    match url.scheme() {
        "http" => {
            url.set_scheme("ws")
                .map_err(|_| "failed to convert http->ws".to_owned())?;
        }
        "https" => {
            url.set_scheme("wss")
                .map_err(|_| "failed to convert https->wss".to_owned())?;
        }
        "ws" | "wss" => {}
        other => {
            return Err(format!(
                "unsupported node URL scheme `{other}`; expected http/https/ws/wss"
            ));
        }
    }
    Ok(url)
}

fn connect_ws_ticket_tool() -> ToolSpec {
    ToolSpec {
        name: "connect.ws.ticket".to_owned(),
        description: "Build Connect WebSocket join metadata (URL + auth headers/protocol token)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/connect/ws".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["sid", "role"],
            "properties": {
                "sid": { "type": "string" },
                "role": { "type": "string", "enum": ["app", "wallet"] },
                "token": {
                    "type": "string",
                    "description": "Explicit token for the selected role."
                },
                "token_app": {
                    "type": "string",
                    "description": "Token alias used when `role=app` and `token` is omitted."
                },
                "token_wallet": {
                    "type": "string",
                    "description": "Token alias used when `role=wallet` and `token` is omitted."
                },
                "node_url": { "type": "string", "description": "Optional node URL; defaults to Host/X-Forwarded-Proto from the MCP request." }
            }
        }),
    }
}

fn connect_session_create_tool() -> ToolSpec {
    ToolSpec {
        name: "connect.session.create".to_owned(),
        description: "Create an Iroha Connect session and return app/wallet tokens.".to_owned(),
        method: Method::POST,
        path_template: "/v1/connect/session".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "sid": {
                    "type": "string",
                    "description": "Convenience shortcut for `body.sid` (base64url session id)."
                },
                "node": {
                    "type": "string",
                    "description": "Convenience shortcut for `body.node`."
                },
                "node_url": {
                    "type": "string",
                    "description": "Alias for `node` convenience shortcut."
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw Connect session request body. If provided, it takes precedence over `sid`/`node` shortcuts."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn connect_session_create_and_ticket_tool() -> ToolSpec {
    ToolSpec {
        name: "connect.session.create_and_ticket".to_owned(),
        description: "Create an Iroha Connect session and immediately build WebSocket join metadata for a selected role.".to_owned(),
        method: Method::POST,
        path_template: "/v1/connect/session".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["role"],
            "properties": {
                "role": {
                    "type": "string",
                    "enum": ["app", "wallet"],
                    "description": "Role used to select `token_app` or `token_wallet` for ticket generation."
                },
                "sid": {
                    "type": "string",
                    "description": "Convenience shortcut for `body.sid` (base64url session id)."
                },
                "node": {
                    "type": "string",
                    "description": "Convenience shortcut for `body.node`."
                },
                "node_url": {
                    "type": "string",
                    "description": "Alias for `node`; also used as ticket URL base when `ticket_node_url` is omitted."
                },
                "ticket_node_url": {
                    "type": "string",
                    "description": "Optional node URL override used only for ticket generation."
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw Connect session request body. If provided, it takes precedence over `sid`/`node` shortcuts."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn connect_session_delete_tool() -> ToolSpec {
    ToolSpec {
        name: "connect.session.delete".to_owned(),
        description: "Delete/purge an Iroha Connect session by SID.".to_owned(),
        method: Method::DELETE,
        path_template: "/v1/connect/session/{sid}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["sid"],
            "properties": {
                "sid": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn connect_status_tool() -> ToolSpec {
    ToolSpec {
        name: "connect.status".to_owned(),
        description: "Get Iroha Connect relay/session status.".to_owned(),
        method: Method::GET,
        path_template: "/v1/connect/status".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_connect_ws_ticket_tool() -> ToolSpec {
    let mut tool = connect_ws_ticket_tool();
    tool.name = "iroha.connect.ws.ticket".to_owned();
    tool.description = "Alias for connect.ws.ticket.".to_owned();
    tool
}

fn iroha_connect_session_create_tool() -> ToolSpec {
    let mut tool = connect_session_create_tool();
    tool.name = "iroha.connect.session.create".to_owned();
    tool.description = "Alias for connect.session.create.".to_owned();
    tool
}

fn iroha_connect_session_create_and_ticket_tool() -> ToolSpec {
    let mut tool = connect_session_create_and_ticket_tool();
    tool.name = "iroha.connect.session.create_and_ticket".to_owned();
    tool.description = "Alias for connect.session.create_and_ticket.".to_owned();
    tool
}

fn iroha_connect_session_delete_tool() -> ToolSpec {
    let mut tool = connect_session_delete_tool();
    tool.name = "iroha.connect.session.delete".to_owned();
    tool.description = "Alias for connect.session.delete.".to_owned();
    tool
}

fn iroha_connect_status_tool() -> ToolSpec {
    let mut tool = connect_status_tool();
    tool.name = "iroha.connect.status".to_owned();
    tool.description = "Alias for connect.status.".to_owned();
    tool
}

fn iroha_health_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.health".to_owned(),
        description: "Get node liveness status (`/health`).".to_owned(),
        method: Method::GET,
        path_template: "/health".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_status_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.status".to_owned(),
        description: "Get node status snapshot (`/status`).".to_owned(),
        method: Method::GET,
        path_template: "/status".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_parameters_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.parameters.get".to_owned(),
        description: "Get node parameters snapshot (`/v1/parameters`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/parameters".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_node_capabilities_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.node.capabilities".to_owned(),
        description: "Get node capability metadata (`/v1/node/capabilities`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/node/capabilities".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_contracts_post_tool(name: &str, description: &str, path_template: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_owned(),
        description: description.to_owned(),
        method: Method::POST,
        path_template: path_template.to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw request payload. If omitted, flat top-level fields are forwarded as the request body."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_contracts_code_register_tool() -> ToolSpec {
    iroha_contracts_post_tool(
        "iroha.contracts.code.register",
        "Register contract code/manifest (`/v1/contracts/code`).",
        "/v1/contracts/code",
    )
}

fn iroha_contracts_code_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.contracts.code.get".to_owned(),
        description: "Fetch contract code metadata (`code_hash` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/contracts/code/{code_hash}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "code_hash": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.code_hash`."
                },
                "hash": {
                    "type": "string",
                    "description": "Alias for `code_hash`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["code_hash"],
                    "properties": {
                        "code_hash": { "type": "string" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_contracts_deploy_tool() -> ToolSpec {
    iroha_contracts_post_tool(
        "iroha.contracts.deploy",
        "Deploy contract code (`/v1/contracts/deploy`).",
        "/v1/contracts/deploy",
    )
}

fn iroha_contracts_instance_create_tool() -> ToolSpec {
    iroha_contracts_post_tool(
        "iroha.contracts.instance.create",
        "Deploy and activate a contract instance (`/v1/contracts/instance`).",
        "/v1/contracts/instance",
    )
}

fn iroha_contracts_instance_activate_tool() -> ToolSpec {
    iroha_contracts_post_tool(
        "iroha.contracts.instance.activate",
        "Activate a contract instance (`/v1/contracts/instance/activate`).",
        "/v1/contracts/instance/activate",
    )
}

fn iroha_contracts_call_tool() -> ToolSpec {
    iroha_contracts_post_tool(
        "iroha.contracts.call",
        "Call a deployed contract instance (`/v1/contracts/call`).",
        "/v1/contracts/call",
    )
}

fn iroha_contracts_call_and_wait_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.contracts.call_and_wait".to_owned(),
        description:
            "Call a deployed contract instance and poll pipeline status until a terminal state."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/contracts/call".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw request payload. If omitted, flat top-level fields are forwarded as the request body."
                },
                "hash": {
                    "type": "string",
                    "description": "Optional known transaction hash override."
                },
                "transaction_hash": {
                    "type": "string",
                    "description": "Alias for `hash`."
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Polling timeout in milliseconds (default 30000, max 600000)."
                },
                "poll_interval_ms": {
                    "type": "integer",
                    "description": "Polling interval in milliseconds (default 500, minimum 50)."
                },
                "terminal_statuses": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional terminal status override (default: Committed, Applied, Rejected, Expired)."
                },
                "status_accept": {
                    "type": "string",
                    "description": "Optional Accept header for status polling calls (defaults to application/json)."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_contracts_state_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.contracts.state.get".to_owned(),
        description:
            "Read contract state (`/v1/contracts/state`) using path/paths/prefix query modes."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/contracts/state".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "path": { "type": "string" },
                "paths": { "type": "string" },
                "prefix": { "type": "string" },
                "include_value": { "type": "boolean" },
                "offset": { "type": "integer" },
                "limit": { "type": "integer" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_accounts_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.list".to_owned(),
        description:
            "List accounts with optional query filters/pagination (supports flat top-level query args)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/accounts".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "asset_id": { "type": "string" },
                "address_format": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_accounts_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.get".to_owned(),
        description: "Fetch explorer account detail (`account_id` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/accounts/{account_id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.account_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["account_id"],
                    "properties": {
                        "account_id": { "type": "string" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_accounts_qr_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.qr".to_owned(),
        description: "Fetch explorer account QR code (`account_id` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/accounts/{account_id}/qr".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.account_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["account_id"],
                    "properties": {
                        "account_id": { "type": "string" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_accounts_query_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.query".to_owned(),
        description:
            "Query accounts with filter/select/sort/pagination envelope (flat shortcuts supported)."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/accounts/query".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw QueryEnvelope payload. If provided, it takes precedence over shortcut fields."
                },
                "query": { "type": "string" },
                "filter": { "type": "object", "additionalProperties": true },
                "select": {},
                "sort": { "type": "array", "items": {} },
                "pagination": { "type": "object", "additionalProperties": true },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "fetch_size": { "type": "integer" },
                "address_format": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_accounts_resolve_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.resolve".to_owned(),
        description:
            "Resolve account literals into canonical IH58 account identifiers (`literal` shortcut supported)."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/accounts/resolve".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "literal": {
                    "type": "string",
                    "description": "Convenience shortcut for `body.literal`."
                },
                "account_literal": {
                    "type": "string",
                    "description": "Alias for `literal`."
                },
                "account_id": {
                    "type": "string",
                    "description": "Alias for `literal` when the input is already an account identifier."
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_accounts_onboard_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.onboard".to_owned(),
        description:
            "Onboard an account (`alias` + `account_id` shortcuts supported when `body` is omitted)."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/accounts/onboard".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "alias": {
                    "type": "string",
                    "description": "Convenience shortcut for `body.alias`."
                },
                "account_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `body.account_id`."
                },
                "identity": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Optional onboarding identity metadata."
                },
                "uaid": {
                    "type": "string",
                    "description": "Optional UAID literal."
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw onboarding request body. If provided, it takes precedence over shortcuts."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_account_transactions_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.transactions".to_owned(),
        description:
            "List transactions authored by a specific account (`account_id` shortcut supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/accounts/{account_id}/transactions".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.account_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["account_id"],
                    "properties": {
                        "account_id": { "type": "string" }
                    }
                },
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_account_transactions_query_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.transactions.query".to_owned(),
        description: "Query transactions authored by a specific account (flat `account_id` + QueryEnvelope shortcuts supported).".to_owned(),
        method: Method::POST,
        path_template: "/v1/accounts/{account_id}/transactions/query".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.account_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["account_id"],
                    "properties": {
                        "account_id": { "type": "string" }
                    }
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw QueryEnvelope payload. If provided, it takes precedence over shortcut fields."
                },
                "query": { "type": "string" },
                "filter": { "type": "object", "additionalProperties": true },
                "select": {},
                "sort": { "type": "array", "items": {} },
                "pagination": { "type": "object", "additionalProperties": true },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "fetch_size": { "type": "integer" },
                "address_format": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_account_assets_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.assets".to_owned(),
        description: "List assets held by a specific account (`account_id` shortcut supported)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/accounts/{account_id}/assets".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.account_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["account_id"],
                    "properties": {
                        "account_id": { "type": "string" }
                    }
                },
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "asset_id": { "type": "string" },
                "address_format": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_account_assets_query_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.assets.query".to_owned(),
        description: "Query assets held by a specific account (flat `account_id` + QueryEnvelope shortcuts supported).".to_owned(),
        method: Method::POST,
        path_template: "/v1/accounts/{account_id}/assets/query".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.account_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["account_id"],
                    "properties": {
                        "account_id": { "type": "string" }
                    }
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw QueryEnvelope payload. If provided, it takes precedence over shortcut fields."
                },
                "query": { "type": "string" },
                "filter": { "type": "object", "additionalProperties": true },
                "select": {},
                "sort": { "type": "array", "items": {} },
                "pagination": { "type": "object", "additionalProperties": true },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "fetch_size": { "type": "integer" },
                "address_format": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_account_permissions_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.permissions".to_owned(),
        description:
            "List permissions granted to a specific account (`account_id` shortcut supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/accounts/{account_id}/permissions".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.account_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["account_id"],
                    "properties": {
                        "account_id": { "type": "string" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_account_portfolio_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.portfolio".to_owned(),
        description: "Fetch a UAID portfolio snapshot (`uaid` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/accounts/{uaid}/portfolio".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "uaid": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.uaid`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["uaid"],
                    "properties": {
                        "uaid": { "type": "string" }
                    }
                },
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "asset_id": {
                    "type": "string",
                    "description": "Optional asset-id filter."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_domains_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.domains.list".to_owned(),
        description: "List domains with optional flat pagination/query fields.".to_owned(),
        method: Method::GET,
        path_template: "/v1/domains".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_domains_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.domains.get".to_owned(),
        description: "Fetch explorer domain detail (`domain_id` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/domains/{domain_id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "domain_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.domain_id`."
                },
                "domain": {
                    "type": "string",
                    "description": "Alias for `domain_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["domain_id"],
                    "properties": {
                        "domain_id": { "type": "string" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_domains_query_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.domains.query".to_owned(),
        description:
            "Query domains with filter/select/sort/pagination envelope (flat shortcuts supported)."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/domains/query".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw QueryEnvelope payload. If provided, it takes precedence over shortcut fields."
                },
                "query": { "type": "string" },
                "filter": { "type": "object", "additionalProperties": true },
                "select": {},
                "sort": { "type": "array", "items": {} },
                "pagination": { "type": "object", "additionalProperties": true },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "fetch_size": { "type": "integer" },
                "address_format": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_subscriptions_plans_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.subscriptions.plans.list".to_owned(),
        description: "List subscription plans with optional flat query filters.".to_owned(),
        method: Method::GET,
        path_template: "/v1/subscriptions/plans".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "provider": { "type": "string" },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_subscriptions_plans_create_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.subscriptions.plans.create".to_owned(),
        description: "Create a subscription plan (`body` payload).".to_owned(),
        method: Method::POST,
        path_template: "/v1/subscriptions/plans".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Subscription plan payload. When omitted, `{}` is submitted."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_subscriptions_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.subscriptions.list".to_owned(),
        description: "List subscriptions with optional flat query filters.".to_owned(),
        method: Method::GET,
        path_template: "/v1/subscriptions".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "owned_by": { "type": "string" },
                "provider": { "type": "string" },
                "status": { "type": "string" },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_subscriptions_create_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.subscriptions.create".to_owned(),
        description: "Create a subscription (`body` payload).".to_owned(),
        method: Method::POST,
        path_template: "/v1/subscriptions".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Subscription create payload. When omitted, `{}` is submitted."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_subscriptions_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.subscriptions.get".to_owned(),
        description: "Fetch subscription detail (`subscription_id`/`id` shortcut supported)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/subscriptions/{subscription_id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "subscription_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.subscription_id`."
                },
                "id": {
                    "type": "string",
                    "description": "Alias for `subscription_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["subscription_id"],
                    "properties": {
                        "subscription_id": { "type": "string" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_subscriptions_cancel_tool() -> ToolSpec {
    iroha_subscription_action_tool(
        "iroha.subscriptions.cancel",
        "Cancel a subscription (`subscription_id` shortcut supported).",
        "cancel",
        "Optional cancellation payload. When omitted, `{}` is submitted.",
    )
}

fn iroha_subscriptions_pause_tool() -> ToolSpec {
    iroha_subscription_action_tool(
        "iroha.subscriptions.pause",
        "Pause a subscription (`subscription_id` shortcut supported).",
        "pause",
        "Optional pause payload. When omitted, `{}` is submitted.",
    )
}

fn iroha_subscriptions_resume_tool() -> ToolSpec {
    iroha_subscription_action_tool(
        "iroha.subscriptions.resume",
        "Resume a subscription (`subscription_id` shortcut supported).",
        "resume",
        "Optional resume payload. When omitted, `{}` is submitted.",
    )
}

fn iroha_subscriptions_keep_tool() -> ToolSpec {
    iroha_subscription_action_tool(
        "iroha.subscriptions.keep",
        "Keep a subscription active (`subscription_id` shortcut supported).",
        "keep",
        "Optional keep payload. When omitted, `{}` is submitted.",
    )
}

fn iroha_subscriptions_usage_tool() -> ToolSpec {
    iroha_subscription_action_tool(
        "iroha.subscriptions.usage",
        "Record subscription usage (`subscription_id` shortcut supported).",
        "usage",
        "Optional usage payload. When omitted, `{}` is submitted.",
    )
}

fn iroha_subscriptions_charge_now_tool() -> ToolSpec {
    iroha_subscription_action_tool(
        "iroha.subscriptions.charge_now",
        "Trigger immediate billing for a subscription (`subscription_id` shortcut supported).",
        "charge-now",
        "Optional charge-now payload. When omitted, `{}` is submitted.",
    )
}

fn iroha_subscription_action_tool(
    name: &str,
    description: &str,
    action: &str,
    body_description: &str,
) -> ToolSpec {
    ToolSpec {
        name: name.to_owned(),
        description: description.to_owned(),
        method: Method::POST,
        path_template: format!("/v1/subscriptions/{{subscription_id}}/{action}"),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "subscription_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.subscription_id`."
                },
                "id": {
                    "type": "string",
                    "description": "Alias for `subscription_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["subscription_id"],
                    "properties": {
                        "subscription_id": { "type": "string" }
                    }
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": body_description
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_asset_definitions_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.assets.definitions".to_owned(),
        description:
            "List asset definitions with optional flat pagination/sort/filter query fields."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/assets/definitions".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "sort": { "type": "string" },
                "filter": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_asset_definitions_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.assets.definitions.get".to_owned(),
        description: "Fetch explorer asset definition detail (`definition_id` shortcut supported)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/asset-definitions/{definition_id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "definition_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.definition_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["definition_id"],
                    "properties": {
                        "definition_id": { "type": "string" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_asset_definitions_query_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.assets.definitions.query".to_owned(),
        description: "Query asset definitions with QueryEnvelope shortcuts when `body` is omitted."
            .to_owned(),
        method: Method::POST,
        path_template: "/v1/assets/definitions/query".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw QueryEnvelope payload. If provided, it takes precedence over shortcut fields."
                },
                "query": { "type": "string" },
                "filter": { "type": "object", "additionalProperties": true },
                "select": {},
                "sort": { "type": "array", "items": {} },
                "pagination": { "type": "object", "additionalProperties": true },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "fetch_size": { "type": "integer" },
                "address_format": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_asset_holders_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.assets.holders".to_owned(),
        description: "List asset holders for one definition (`definition_id` shortcut supported)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/assets/{definition_id}/holders".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "definition_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.definition_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["definition_id"],
                    "properties": {
                        "definition_id": { "type": "string" }
                    }
                },
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "asset_id": { "type": "string" },
                "address_format": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_asset_holders_query_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.assets.holders.query".to_owned(),
        description: "Query asset holders for one definition (flat `definition_id` + QueryEnvelope shortcuts supported).".to_owned(),
        method: Method::POST,
        path_template: "/v1/assets/{definition_id}/holders/query".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "definition_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.definition_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["definition_id"],
                    "properties": {
                        "definition_id": { "type": "string" }
                    }
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw QueryEnvelope payload. If provided, it takes precedence over shortcut fields."
                },
                "query": { "type": "string" },
                "filter": { "type": "object", "additionalProperties": true },
                "select": {},
                "sort": { "type": "array", "items": {} },
                "pagination": { "type": "object", "additionalProperties": true },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "fetch_size": { "type": "integer" },
                "address_format": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_assets_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.assets.list".to_owned(),
        description: "List explorer assets with optional flat query filters.".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/assets".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "page": { "type": "integer" },
                "per_page": { "type": "integer" },
                "owned_by": { "type": "string" },
                "definition": { "type": "string" },
                "asset_id": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_assets_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.assets.get".to_owned(),
        description: "Fetch explorer asset detail (`asset_id` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/assets/{asset_id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "asset_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.asset_id`."
                },
                "id": {
                    "type": "string",
                    "description": "Alias for `asset_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["asset_id"],
                    "properties": {
                        "asset_id": { "type": "string" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_nfts_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.nfts.list".to_owned(),
        description: "List explorer NFTs with optional flat query filters.".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/nfts".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "page": { "type": "integer" },
                "per_page": { "type": "integer" },
                "owned_by": { "type": "string" },
                "domain": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_nfts_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.nfts.get".to_owned(),
        description: "Fetch explorer NFT detail (`nft_id` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/nfts/{nft_id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "nft_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.nft_id`."
                },
                "id": {
                    "type": "string",
                    "description": "Alias for `nft_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["nft_id"],
                    "properties": {
                        "nft_id": { "type": "string" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_nfts_query_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.nfts.query".to_owned(),
        description:
            "Query NFTs with filter/select/sort/pagination envelope (flat shortcuts supported)."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/nfts/query".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw QueryEnvelope payload. If provided, it takes precedence over shortcut fields."
                },
                "query": { "type": "string" },
                "filter": { "type": "object", "additionalProperties": true },
                "select": {},
                "sort": { "type": "array", "items": {} },
                "pagination": { "type": "object", "additionalProperties": true },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "fetch_size": { "type": "integer" },
                "address_format": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_transactions_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.transactions.list".to_owned(),
        description: "List explorer transactions with optional flat query filters.".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/transactions".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "limit": { "type": "integer" },
                "offset": { "type": "integer" },
                "authority": { "type": "string" },
                "block": { "type": "integer" },
                "status": { "type": "string" },
                "asset_id": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_transactions_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.transactions.get".to_owned(),
        description: "Fetch explorer transaction detail (`hash` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/transactions/{hash}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "hash": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.hash`."
                },
                "transaction_hash": {
                    "type": "string",
                    "description": "Alias for `hash`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["hash"],
                    "properties": {
                        "hash": { "type": "string" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_instructions_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.instructions.list".to_owned(),
        description: "List explorer instructions with optional flat query filters.".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/instructions".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "page": { "type": "integer" },
                "per_page": { "type": "integer" },
                "authority": { "type": "string" },
                "account": { "type": "string" },
                "transaction_hash": { "type": "string" },
                "transaction_status": { "type": "string" },
                "block": { "type": "integer" },
                "kind": { "type": "string" },
                "asset_id": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_instructions_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.instructions.get".to_owned(),
        description: "Fetch explorer instruction detail (`hash` + `index` shortcuts supported)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/instructions/{hash}/{index}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "hash": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.hash`."
                },
                "transaction_hash": {
                    "type": "string",
                    "description": "Alias for `hash`."
                },
                "index": {
                    "type": "integer",
                    "description": "Convenience shortcut for `path.index`."
                },
                "instruction_index": {
                    "type": "integer",
                    "description": "Alias for `index`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["hash", "index"],
                    "properties": {
                        "hash": { "type": "string" },
                        "index": { "type": "integer" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_blocks_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.blocks.list".to_owned(),
        description: "List explorer blocks with optional flat query filters.".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/blocks".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "page": { "type": "integer" },
                "per_page": { "type": "integer" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_blocks_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.blocks.get".to_owned(),
        description:
            "Fetch explorer block detail (`identifier` shortcut and block aliases supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/blocks/{identifier}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "identifier": {
                    "description": "Convenience shortcut for `path.identifier` (hash or height)."
                },
                "block_identifier": {
                    "description": "Alias for `identifier`."
                },
                "block_height": {
                    "type": "integer",
                    "description": "Alias for `identifier` using numeric block height."
                },
                "block_hash": {
                    "type": "string",
                    "description": "Alias for `identifier` using a block hash."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["identifier"],
                    "properties": {
                        "identifier": {}
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_transactions_submit_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.transactions.submit".to_owned(),
        description: "Submit a signed transaction encoded as Norito bytes (`signed_tx_base64`/`tx_base64`/hex shortcuts supported).".to_owned(),
        method: Method::POST,
        path_template: "/transaction".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body_base64": {
                    "type": "string",
                    "description": "Base64/base64url encoded SignedTransaction bytes."
                },
                "signed_tx_base64": {
                    "type": "string",
                    "description": "Alias for `body_base64`."
                },
                "tx_base64": {
                    "type": "string",
                    "description": "Alias for `body_base64`."
                },
                "body_hex": {
                    "type": "string",
                    "description": "Hex-encoded SignedTransaction bytes."
                },
                "signed_tx_hex": {
                    "type": "string",
                    "description": "Alias for `body_hex`."
                },
                "tx_hex": {
                    "type": "string",
                    "description": "Alias for `body_hex`."
                },
                "body": {
                    "description": "Optional JSON request body; use only when submitting JSON transaction envelopes."
                },
                "content_type": {
                    "type": "string",
                    "description": "Optional content type override (defaults to application/x-norito)."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_transactions_submit_and_wait_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.transactions.submit_and_wait".to_owned(),
        description: "Submit a signed transaction and poll pipeline status until a terminal state (`Committed`/`Applied`/`Rejected`/`Expired` by default).".to_owned(),
        method: Method::POST,
        path_template: "/transaction".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body_base64": {
                    "type": "string",
                    "description": "Base64/base64url encoded SignedTransaction bytes."
                },
                "signed_tx_base64": {
                    "type": "string",
                    "description": "Alias for `body_base64`."
                },
                "tx_base64": {
                    "type": "string",
                    "description": "Alias for `body_base64`."
                },
                "body_hex": {
                    "type": "string",
                    "description": "Hex-encoded SignedTransaction bytes."
                },
                "signed_tx_hex": {
                    "type": "string",
                    "description": "Alias for `body_hex`."
                },
                "tx_hex": {
                    "type": "string",
                    "description": "Alias for `body_hex`."
                },
                "body": {
                    "description": "Optional JSON request body; use only when submitting JSON transaction envelopes."
                },
                "content_type": {
                    "type": "string",
                    "description": "Optional content type override (defaults to application/x-norito)."
                },
                "hash": {
                    "type": "string",
                    "description": "Optional known transaction hash; if omitted the tool attempts to decode it from the submission receipt."
                },
                "transaction_hash": {
                    "type": "string",
                    "description": "Alias for `hash`."
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Polling timeout in milliseconds (default 30000, max 600000)."
                },
                "poll_interval_ms": {
                    "type": "integer",
                    "description": "Polling interval in milliseconds (default 500, minimum 50)."
                },
                "terminal_statuses": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional terminal status override (default: Committed, Applied, Rejected, Expired)."
                },
                "status_accept": {
                    "type": "string",
                    "description": "Optional Accept header for status polling calls (defaults to application/json)."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_transactions_wait_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.transactions.wait".to_owned(),
        description:
            "Poll pipeline status for an existing transaction hash until a terminal state."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/pipeline/transactions/status".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "hash": {
                    "type": "string",
                    "description": "Convenience shortcut for `query.hash`."
                },
                "transaction_hash": {
                    "type": "string",
                    "description": "Alias for `hash`."
                },
                "query": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["hash"],
                    "properties": {
                        "hash": { "type": "string" }
                    }
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Polling timeout in milliseconds (default 30000, max 600000)."
                },
                "poll_interval_ms": {
                    "type": "integer",
                    "description": "Polling interval in milliseconds (default 500, minimum 50)."
                },
                "terminal_statuses": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional terminal status override (default: Committed, Applied, Rejected, Expired)."
                },
                "status_accept": {
                    "type": "string",
                    "description": "Optional Accept header for status polling calls (defaults to application/json)."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_transactions_status_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.transactions.status".to_owned(),
        description:
            "Get latest pipeline status for a submitted transaction hash (`hash` shortcut supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/pipeline/transactions/status".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "hash": {
                    "type": "string",
                    "description": "Convenience shortcut for `query.hash`."
                },
                "query": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["hash"],
                    "properties": {
                        "hash": { "type": "string" }
                    }
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

/// Build the HTTP status + JSON-RPC error payload for oversized requests.
pub(crate) fn oversized_payload_response(max_request_bytes: usize) -> (StatusCode, Value) {
    (
        StatusCode::PAYLOAD_TOO_LARGE,
        jsonrpc_error_response(
            None,
            JSONRPC_INVALID_REQUEST,
            "mcp request body exceeds configured size limit",
            Some(norito::json!({
                "max_request_bytes": max_request_bytes
            })),
        ),
    )
}

/// Build the JSON-RPC payload for internal dispatch failures.
pub(crate) fn internal_error_payload(message: &str) -> Value {
    jsonrpc_error_response(
        None,
        MCP_TOOL_EXECUTION_ERROR,
        message,
        Some(norito::json!({
            "kind": "dispatch_error"
        })),
    )
}

pub(crate) fn method_not_allowed_payload() -> Value {
    jsonrpc_error_response(
        None,
        JSONRPC_METHOD_NOT_FOUND,
        "only JSON-RPC over POST is supported",
        None,
    )
}

pub(crate) fn invalid_json_payload(err: &json::Error) -> Value {
    let mut msg = String::from("invalid json payload: ");
    let _ = write!(msg, "{err}");
    jsonrpc_error_response(None, JSONRPC_PARSE_ERROR, &msg, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_registry_skips_ws_and_sse_routes() {
        let cfg = iroha_config::parameters::actual::ToriiMcp::default();
        let tools = build_tool_specs(&cfg);
        assert!(!tools.is_empty(), "tool registry must not be empty");
        assert!(
            tools.iter().all(
                |tool| tool.path_template != "/events" && !tool.path_template.ends_with("/sse")
            )
        );
        assert!(tools.iter().any(|tool| tool.name == "connect.ws.ticket"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "connect.session.create_and_ticket")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.connect.session.create")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.connect.session.create_and_ticket")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.health"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.status"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.parameters.get"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.node.capabilities")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.contracts.code.register")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.contracts.code.get")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.contracts.deploy")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.contracts.instance.create")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.contracts.instance.activate")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.contracts.call"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.contracts.call_and_wait")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.contracts.state.get")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.accounts.list"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.accounts.get"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.accounts.qr"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.accounts.query"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.accounts.onboard")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.transactions.submit")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.transactions.submit_and_wait")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.transactions.wait")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.accounts.assets")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.accounts.permissions")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.accounts.transactions.query")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.accounts.assets.query")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.accounts.portfolio")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.domains.list"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.domains.get"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.domains.query"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.subscriptions.plans.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.subscriptions.plans.create")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.subscriptions.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.subscriptions.create")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.subscriptions.get")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.subscriptions.pause")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.subscriptions.resume")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.subscriptions.cancel")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.subscriptions.keep")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.subscriptions.usage")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.subscriptions.charge_now")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.assets.definitions")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.assets.definitions.get")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.assets.definitions.query")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.assets.holders"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.assets.holders.query")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.assets.list"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.assets.get"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.nfts.list"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.nfts.get"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.nfts.query"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.transactions.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.transactions.get")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.instructions.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.instructions.get")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.blocks.list"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.blocks.get"));
    }

    #[test]
    fn fill_path_template_substitutes_required_values() {
        let args = norito::json!({
            "sid": "abc",
            "role": "wallet"
        });
        let path =
            fill_path_template("/v1/connect/session/{sid}/{role}", Some(&args)).expect("filled");
        assert_eq!(path, "/v1/connect/session/abc/wallet");
    }

    #[test]
    fn ws_ticket_uses_ws_url_and_protocol_token() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("node.example"));
        let args = norito::json!({
            "sid": "Z2Fr",
            "role": "app",
            "token": "my-token"
        });
        let ticket =
            build_connect_ws_ticket(args.as_object().expect("object"), &headers).expect("ticket");
        let ws_url = ticket
            .get("ws_url")
            .and_then(Value::as_str)
            .expect("ws url");
        assert!(ws_url.starts_with("ws://node.example/v1/connect/ws?"));
        assert_eq!(
            ticket
                .get("sec_websocket_protocol")
                .and_then(Value::as_str)
                .expect("protocol"),
            "iroha-connect.token.v1.bXktdG9rZW4"
        );
    }

    #[test]
    fn ws_ticket_accepts_role_specific_token_aliases() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("node.example"));
        let args = norito::json!({
            "sid": "YWJj",
            "role": "wallet",
            "token_wallet": "wallet-token"
        });
        let ticket =
            build_connect_ws_ticket(args.as_object().expect("object"), &headers).expect("ticket");
        assert_eq!(
            ticket
                .get("authorization_header")
                .and_then(Value::as_str)
                .expect("authorization"),
            "Bearer wallet-token"
        );
    }

    #[test]
    fn collect_query_map_accepts_flat_query_fields_when_query_absent() {
        let args = norito::json!({
            "account_id": "alice@wonderland",
            "limit": 20,
            "offset": 0,
            "headers": {"x": "1"}
        });
        let map = collect_query_map(
            args.as_object().expect("object"),
            &["account_id", "headers", "accept", "query"],
        )
        .expect("query map");
        assert_eq!(map.get("limit").and_then(Value::as_u64), Some(20));
        assert_eq!(map.get("offset").and_then(Value::as_u64), Some(0));
        assert!(!map.contains_key("account_id"));
        assert!(!map.contains_key("headers"));
    }

    #[test]
    fn collect_query_map_rejects_non_object_query() {
        let args = norito::json!({
            "query": "not-an-object"
        });
        let err =
            collect_query_map(args.as_object().expect("object"), &["query"]).expect_err("error");
        assert!(err.contains("`query` must be an object"));
    }

    #[test]
    fn extract_account_id_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "account_id": "alice@wonderland"
        });
        let account_id =
            extract_account_id_argument(args.as_object().expect("object")).expect("account id");
        assert_eq!(account_id, "alice@wonderland");
    }

    #[test]
    fn extract_uaid_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "uaid": "uaid:00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
        });
        let uaid = extract_uaid_argument(args.as_object().expect("object")).expect("uaid");
        assert_eq!(
            uaid,
            "uaid:00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
        );
    }

    #[test]
    fn extract_domain_id_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "domain_id": "wonderland"
        });
        let domain_id =
            extract_domain_id_argument(args.as_object().expect("object")).expect("domain");
        assert_eq!(domain_id, "wonderland");
    }

    #[test]
    fn extract_subscription_id_argument_accepts_alias_shortcut() {
        let args = norito::json!({
            "id": "sub-001"
        });
        let subscription_id = extract_subscription_id_argument(args.as_object().expect("object"))
            .expect("subscription");
        assert_eq!(subscription_id, "sub-001");
    }

    #[test]
    fn extract_definition_id_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "definition_id": "rose#wonderland"
        });
        let definition_id =
            extract_definition_id_argument(args.as_object().expect("object")).expect("definition");
        assert_eq!(definition_id, "rose#wonderland");
    }

    #[test]
    fn extract_asset_id_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "asset_id": "rose##ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
        });
        let asset_id =
            extract_asset_id_argument(args.as_object().expect("object")).expect("asset id");
        assert_eq!(
            asset_id,
            "rose##ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
        );
    }

    #[test]
    fn extract_nft_id_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "nft_id": "nft-001"
        });
        let nft_id = extract_nft_id_argument(args.as_object().expect("object")).expect("nft id");
        assert_eq!(nft_id, "nft-001");
    }

    #[test]
    fn extract_transaction_hash_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "hash": "deadbeef"
        });
        let hash =
            extract_transaction_hash_argument(args.as_object().expect("object")).expect("hash");
        assert_eq!(hash, "deadbeef");
    }

    #[test]
    fn extract_optional_transaction_hash_argument_accepts_alias_shortcut() {
        let args = norito::json!({
            "transaction_hash": "deadbeef"
        });
        let hash = extract_optional_transaction_hash_argument(args.as_object().expect("object"))
            .expect("hash");
        assert_eq!(hash, "deadbeef");
    }

    #[test]
    fn extract_transaction_hash_from_submit_result_decodes_submission_receipt() {
        let key_pair = iroha_crypto::KeyPair::random();
        let tx_hash =
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0xAB; 32]));
        let payload = iroha_data_model::transaction::TransactionSubmissionReceiptPayload {
            tx_hash: tx_hash.clone(),
            submitted_at_ms: 1,
            submitted_at_height: 1,
            signer: key_pair.public_key().clone(),
        };
        let receipt =
            iroha_data_model::transaction::TransactionSubmissionReceipt::sign(payload, &key_pair);
        let receipt_bytes = norito::to_bytes(&receipt).expect("receipt bytes");
        let encoded =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, receipt_bytes);
        let submit_result = norito::json!({
            "status": 202,
            "body": encoded
        });

        let hash = extract_transaction_hash_from_submit_result(&submit_result).expect("hash");
        assert_eq!(hash, tx_hash.to_string());
    }

    #[test]
    fn extract_transaction_hash_from_submit_result_accepts_tx_hash_hex_field() {
        let submit_result = norito::json!({
            "status": 202,
            "body": {
                "ok": true,
                "tx_hash_hex": "deadbeef"
            }
        });
        let hash = extract_transaction_hash_from_submit_result(&submit_result).expect("hash");
        assert_eq!(hash, "deadbeef");
    }

    #[test]
    fn extract_pipeline_status_kind_reads_pipeline_envelope() {
        let status_result = norito::json!({
            "status": 200,
            "body": {
                "kind": "Transaction",
                "content": {
                    "hash": "deadbeef",
                    "status": {
                        "kind": "Committed"
                    }
                }
            }
        });
        assert_eq!(
            extract_pipeline_status_kind(&status_result),
            Some("Committed")
        );
    }

    #[test]
    fn resolve_submit_wait_terminal_statuses_accepts_custom_values() {
        let args = norito::json!({
            "terminal_statuses": ["Applied", "Rejected"]
        });
        let statuses =
            resolve_submit_wait_terminal_statuses(args.as_object().expect("object")).expect("ok");
        assert_eq!(statuses, vec!["Applied", "Rejected"]);
    }

    #[test]
    fn resolve_submit_wait_terminal_statuses_rejects_unsupported_values() {
        let args = norito::json!({
            "terminal_statuses": ["Unknown"]
        });
        let err = resolve_submit_wait_terminal_statuses(args.as_object().expect("object"))
            .expect_err("unsupported terminal status should fail");
        assert!(err.contains("unsupported terminal status"));
    }

    #[test]
    fn extract_code_hash_argument_accepts_alias_shortcut() {
        let args = norito::json!({
            "hash": "cafebabe"
        });
        let hash = extract_code_hash_argument(args.as_object().expect("object")).expect("hash");
        assert_eq!(hash, "cafebabe");
    }

    #[test]
    fn extract_instruction_index_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "index": 3
        });
        let index =
            extract_instruction_index_argument(args.as_object().expect("object")).expect("index");
        assert_eq!(index, "3");
    }

    #[test]
    fn extract_instruction_index_argument_accepts_alias_shortcut() {
        let args = norito::json!({
            "instruction_index": 7
        });
        let index =
            extract_instruction_index_argument(args.as_object().expect("object")).expect("index");
        assert_eq!(index, "7");
    }

    #[test]
    fn extract_block_identifier_argument_accepts_height_alias() {
        let args = norito::json!({
            "block_height": 7
        });
        let identifier =
            extract_block_identifier_argument(args.as_object().expect("object")).expect("id");
        assert_eq!(identifier, "7");
    }

    #[test]
    fn build_query_envelope_body_collects_shortcut_fields() {
        let args = norito::json!({
            "filter": { "op": "eq", "args": ["authority", "alice@wonderland"] },
            "limit": 25,
            "offset": 5,
            "fetch_size": 10
        });
        let body = build_query_envelope_body(args.as_object().expect("object")).expect("body");
        let body = body.as_object().expect("body object");
        assert!(body.contains_key("filter"));
        let pagination = body
            .get("pagination")
            .and_then(Value::as_object)
            .expect("pagination");
        assert_eq!(pagination.get("limit").and_then(Value::as_u64), Some(25));
        assert_eq!(pagination.get("offset").and_then(Value::as_u64), Some(5));
        assert_eq!(body.get("fetch_size").and_then(Value::as_u64), Some(10));
    }

    #[test]
    fn build_query_envelope_body_rejects_non_object_body() {
        let args = norito::json!({
            "body": "invalid"
        });
        let err = build_query_envelope_body(args.as_object().expect("object")).expect_err("error");
        assert!(err.contains("`body` must be an object"));
    }

    #[test]
    fn build_accounts_onboard_body_collects_shortcut_fields() {
        let args = norito::json!({
            "alias": "alice",
            "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
            "identity": { "tier": "gold" },
            "uaid": "uaid:00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
        });
        let body = build_accounts_onboard_body(args.as_object().expect("object")).expect("body");
        let body = body.as_object().expect("object");
        assert_eq!(body.get("alias").and_then(Value::as_str), Some("alice"));
        assert!(body.get("identity").is_some_and(Value::is_object));
        assert!(body.get("uaid").is_some_and(Value::is_string));
    }

    #[test]
    fn build_accounts_onboard_body_rejects_missing_required_shortcuts() {
        let args = norito::json!({
            "identity": {}
        });
        let err =
            build_accounts_onboard_body(args.as_object().expect("object")).expect_err("error");
        assert!(err.contains("`alias` is required"));
    }

    #[test]
    fn build_object_body_or_default_uses_empty_object_when_missing() {
        let args = norito::json!({
            "headers": { "x-test": "1" }
        });
        let body = build_object_body_or_default(args.as_object().expect("object")).expect("body");
        assert_eq!(
            body,
            Value::Object(Map::new()),
            "missing body should default to empty object"
        );
    }

    #[test]
    fn build_object_body_or_default_rejects_non_object_body() {
        let args = norito::json!({
            "body": "invalid"
        });
        let err = build_object_body_or_default(args.as_object().expect("object"))
            .expect_err("should reject non-object body");
        assert!(err.contains("`body` must be an object"));
    }

    #[test]
    fn build_object_body_or_flat_shortcuts_collects_top_level_fields() {
        let args = norito::json!({
            "authority": "alice@wonderland",
            "namespace": "nexus",
            "headers": { "x-test": "1" }
        });
        let body = build_object_body_or_flat_shortcuts(
            args.as_object().expect("object"),
            &["body", "headers", "accept"],
        )
        .expect("body");
        let body = body.as_object().expect("object");
        assert_eq!(
            body.get("authority").and_then(Value::as_str),
            Some("alice@wonderland")
        );
        assert_eq!(body.get("namespace").and_then(Value::as_str), Some("nexus"));
        assert!(body.get("headers").is_none());
    }

    #[test]
    fn build_object_body_or_flat_shortcuts_rejects_missing_body_and_shortcuts() {
        let args = norito::json!({
            "headers": { "x-test": "1" }
        });
        let err = build_object_body_or_flat_shortcuts(
            args.as_object().expect("object"),
            &["body", "headers", "accept"],
        )
        .expect_err("should reject empty payload");
        assert!(err.contains("`body` is required"));
    }
}
