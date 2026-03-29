//! Native MCP endpoint support for Torii.
//!
//! This module exposes a lightweight JSON-RPC bridge that maps MCP tool calls to
//! existing Torii HTTP routes. Tool definitions are derived from Torii's OpenAPI
//! document so the MCP surface tracks the documented API.

use std::{
    fmt::Write as _,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::LazyLock,
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode, header},
    response::Response,
};
use base64::Engine as _;
use blake3::Hasher as Blake3Hasher;
use dashmap::DashMap;
use http_body_util::BodyExt as _;
use norito::json::{self, Map, Value};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
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
const MCP_TOOL_NOT_ALLOWED: &str = "tool_not_allowed";
const MCP_TOOL_NOT_FOUND: &str = "tool_not_found";
const MCP_TOOL_EXECUTION_ERROR_CODE: &str = "tool_execution_error";
const MCP_JOB_NOT_FOUND: &str = "job_not_found";

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

#[derive(Debug, Clone)]
struct AsyncJobRecord {
    state: Value,
    updated_at: Instant,
}

static MCP_ASYNC_JOBS: LazyLock<DashMap<String, AsyncJobRecord>> = LazyLock::new(DashMap::new);

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
        obj.insert("outputSchema".into(), default_tool_output_schema());
        Value::Object(obj)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParameterInfo {
    name: String,
    location: String,
    required: bool,
    schema: Value,
}

/// Build the MCP tool registry from OpenAPI operations.
pub(crate) fn build_tool_specs(cfg: &iroha_config::parameters::actual::ToriiMcp) -> Vec<ToolSpec> {
    let mut tools = Vec::new();
    let spec = openapi::generate_spec();
    let Some(paths) = spec.get("paths").and_then(Value::as_object) else {
        return tools;
    };
    let allow_operator_routes = cfg.expose_operator_routes
        || cfg.profile == iroha_config::parameters::actual::ToriiMcpProfile::Operator;

    for (path, path_item) in paths {
        let Some(path_map) = path_item.as_object() else {
            continue;
        };

        let path_parameters = parse_parameters(&spec, path_map.get("parameters"));

        for method_key in ["get", "post", "put", "patch", "delete", "head", "options"] {
            let Some(operation) = path_map.get(method_key).and_then(Value::as_object) else {
                continue;
            };
            let Some(method) = method_from_key(method_key) else {
                continue;
            };
            if should_skip_operation(path, operation, allow_operator_routes) {
                continue;
            }

            // Keep OpenAPI-derived names stable regardless of mutable `operationId` fields.
            let operation_id = fallback_operation_id(method_key, path);
            let description = operation
                .get("summary")
                .and_then(Value::as_str)
                .or_else(|| operation.get("description").and_then(Value::as_str))
                .unwrap_or("Torii API operation")
                .to_owned();

            let mut parameters = path_parameters.clone();
            parameters.extend(parse_parameters(&spec, operation.get("parameters")));
            let input_schema =
                build_input_schema(&spec, path, &parameters, operation.get("requestBody"));

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
    tools.push(iroha_vpn_profile_tool());
    tools.push(iroha_vpn_sessions_create_tool());
    tools.push(iroha_vpn_sessions_get_tool());
    tools.push(iroha_vpn_sessions_delete_tool());
    tools.push(iroha_vpn_receipts_list_tool());
    tools.push(iroha_health_tool());
    tools.push(iroha_status_tool());
    tools.push(iroha_parameters_get_tool());
    tools.push(iroha_node_capabilities_tool());
    tools.push(iroha_time_now_tool());
    tools.push(iroha_time_status_tool());
    tools.push(iroha_api_versions_tool());
    tools.push(iroha_sumeragi_commit_certificates_tool());
    tools.push(iroha_sumeragi_validator_sets_list_tool());
    tools.push(iroha_sumeragi_validator_sets_get_tool());
    tools.push(iroha_sumeragi_rbc_tool());
    tools.push(iroha_sumeragi_pacemaker_tool());
    tools.push(iroha_sumeragi_phases_tool());
    tools.push(iroha_sumeragi_params_tool());
    tools.push(iroha_sumeragi_status_tool());
    tools.push(iroha_sumeragi_leader_tool());
    tools.push(iroha_sumeragi_qc_tool());
    tools.push(iroha_sumeragi_checkpoints_tool());
    tools.push(iroha_sumeragi_consensus_keys_tool());
    tools.push(iroha_sumeragi_bls_keys_tool());
    tools.push(iroha_sumeragi_key_lifecycle_tool());
    tools.push(iroha_sumeragi_telemetry_tool());
    tools.push(iroha_sumeragi_rbc_sessions_tool());
    tools.push(iroha_sumeragi_commit_qc_get_tool());
    tools.push(iroha_sumeragi_collectors_tool());
    tools.push(iroha_sumeragi_evidence_count_tool());
    tools.push(iroha_sumeragi_evidence_list_tool());
    tools.push(iroha_sumeragi_evidence_submit_tool());
    tools.push(iroha_sumeragi_new_view_tool());
    tools.push(iroha_sumeragi_rbc_delivered_tool());
    tools.push(iroha_sumeragi_vrf_penalties_tool());
    tools.push(iroha_sumeragi_vrf_epoch_tool());
    tools.push(iroha_sumeragi_vrf_commit_tool());
    tools.push(iroha_sumeragi_vrf_reveal_tool());
    tools.push(iroha_sumeragi_rbc_sample_tool());
    tools.push(iroha_da_ingest_tool());
    tools.push(iroha_da_proof_policies_tool());
    tools.push(iroha_da_proof_policy_snapshot_tool());
    tools.push(iroha_da_manifests_get_tool());
    tools.push(iroha_da_commitments_list_tool());
    tools.push(iroha_da_commitments_prove_tool());
    tools.push(iroha_da_commitments_verify_tool());
    tools.push(iroha_da_pin_intents_list_tool());
    tools.push(iroha_da_pin_intents_prove_tool());
    tools.push(iroha_da_pin_intents_verify_tool());
    tools.push(iroha_runtime_abi_active_tool());
    tools.push(iroha_runtime_abi_hash_tool());
    tools.push(iroha_runtime_metrics_tool());
    tools.push(iroha_runtime_upgrades_list_tool());
    tools.push(iroha_runtime_upgrades_propose_tool());
    tools.push(iroha_runtime_upgrades_activate_tool());
    tools.push(iroha_runtime_upgrades_cancel_tool());
    tools.push(iroha_ledger_headers_tool());
    tools.push(iroha_ledger_state_root_tool());
    tools.push(iroha_ledger_state_proof_tool());
    tools.push(iroha_ledger_block_proof_tool());
    tools.push(iroha_bridge_finality_proof_tool());
    tools.push(iroha_bridge_finality_bundle_tool());
    tools.push(iroha_proofs_get_tool());
    tools.push(iroha_proofs_query_tool());
    tools.push(iroha_proofs_retention_tool());
    tools.push(iroha_gov_instances_list_tool());
    tools.push(iroha_gov_proposals_deploy_contract_tool());
    tools.push(iroha_gov_proposals_get_tool());
    tools.push(iroha_gov_locks_get_tool());
    tools.push(iroha_gov_referenda_get_tool());
    tools.push(iroha_gov_tally_get_tool());
    tools.push(iroha_gov_ballots_zk_tool());
    tools.push(iroha_gov_ballots_zk_v1_tool());
    tools.push(iroha_gov_ballots_zk_v1_ballot_proof_tool());
    tools.push(iroha_gov_ballots_plain_tool());
    tools.push(iroha_gov_protected_namespaces_list_tool());
    tools.push(iroha_gov_protected_namespaces_update_tool());
    tools.push(iroha_gov_unlocks_stats_tool());
    tools.push(iroha_gov_council_current_tool());
    tools.push(iroha_gov_council_persist_tool());
    tools.push(iroha_gov_council_replace_tool());
    tools.push(iroha_gov_council_audit_tool());
    tools.push(iroha_gov_council_derive_vrf_tool());
    tools.push(iroha_gov_enact_tool());
    tools.push(iroha_gov_finalize_tool());
    tools.push(iroha_aliases_resolve_tool());
    tools.push(iroha_aliases_resolve_index_tool());
    tools.push(iroha_aliases_by_account_tool());
    tools.push(iroha_contracts_code_get_tool());
    tools.push(iroha_contracts_code_bytes_get_tool());
    tools.push(iroha_contracts_deploy_tool());
    tools.push(iroha_contracts_instance_create_tool());
    tools.push(iroha_contracts_instance_activate_tool());
    tools.push(iroha_contracts_instances_list_tool());
    tools.push(iroha_contracts_call_tool());
    tools.push(iroha_contracts_call_and_wait_tool());
    tools.push(iroha_contracts_state_get_tool());
    tools.push(iroha_accounts_list_tool());
    tools.push(iroha_accounts_get_tool());
    tools.push(iroha_accounts_qr_tool());
    tools.push(iroha_accounts_query_tool());
    tools.push(iroha_accounts_onboard_tool());
    tools.push(iroha_accounts_faucet_tool());
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
    tools.push(iroha_nfts_chain_list_tool());
    tools.push(iroha_nfts_list_tool());
    tools.push(iroha_nfts_get_tool());
    tools.push(iroha_nfts_query_tool());
    tools.push(iroha_rwas_chain_list_tool());
    tools.push(iroha_rwas_list_tool());
    tools.push(iroha_rwas_get_tool());
    tools.push(iroha_rwas_query_tool());
    tools.push(iroha_offline_transfers_list_tool());
    tools.push(iroha_offline_transfers_get_tool());
    tools.push(iroha_offline_transfers_query_tool());
    tools.push(iroha_offline_cash_setup_tool());
    tools.push(iroha_offline_cash_load_tool());
    tools.push(iroha_offline_cash_refresh_tool());
    tools.push(iroha_offline_cash_sync_tool());
    tools.push(iroha_offline_cash_redeem_tool());
    tools.push(iroha_offline_revocations_list_tool());
    tools.push(iroha_offline_revocations_bundle_tool());
    tools.push(iroha_offline_revocations_register_tool());
    tools.push(iroha_iso20022_pacs008_submit_tool());
    tools.push(iroha_iso20022_pacs009_submit_tool());
    tools.push(iroha_iso20022_status_get_tool());
    tools.push(iroha_queries_submit_tool());
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

fn visible_tools_for_policy<'a>(
    cfg: &iroha_config::parameters::actual::ToriiMcp,
    tools: &'a [ToolSpec],
) -> Vec<&'a ToolSpec> {
    tools
        .iter()
        .filter(|tool| is_tool_allowed_by_policy(cfg, tool))
        .collect()
}

pub(crate) fn capabilities_payload(tools: &[&ToolSpec]) -> Value {
    let toolset_version = compute_toolset_version(tools);
    let mut server_info = Map::new();
    server_info.insert("name".into(), Value::String("iroha-torii-mcp".to_owned()));
    server_info.insert("version".into(), Value::String("0.0.0-dev".to_owned()));

    let mut tools_cap = Map::new();
    tools_cap.insert("listChanged".into(), Value::Bool(false));
    tools_cap.insert("count".into(), Value::from(tools.len() as u64));
    tools_cap.insert("toolsetVersion".into(), Value::String(toolset_version));

    let mut capabilities = Map::new();
    capabilities.insert("tools".into(), Value::Object(tools_cap));

    let mut out = Map::new();
    out.insert(
        "protocolVersion".into(),
        Value::String(MCP_PROTOCOL_VERSION.to_owned()),
    );
    out.insert("serverInfo".into(), Value::Object(server_info));
    out.insert("capabilities".into(), Value::Object(capabilities));
    Value::Object(out)
}

pub(crate) fn capabilities_payload_for_state(app: &SharedAppState) -> Value {
    let visible_tools = visible_tools_for_policy(&app.mcp, app.mcp_tools.as_slice());
    capabilities_payload(&visible_tools)
}

fn default_tool_output_schema() -> Value {
    norito::json!({
        "type": "object",
        "description": "Tool structured output payload. Route-dispatched tools include status/headers/content_type/body.",
        "properties": {
            "status": { "type": "integer", "minimum": 100, "maximum": 599 },
            "headers": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            },
            "content_type": {
                "oneOf": [{ "type": "string" }, { "type": "null" }]
            },
            "body": {}
        },
        "additionalProperties": true
    })
}

fn compute_toolset_version(tools: &[&ToolSpec]) -> String {
    let mut hasher = Blake3Hasher::new();
    for tool in tools {
        let rendered =
            norito::json::to_string(&tool.descriptor()).unwrap_or_else(|_| tool.name.clone());
        hasher.update(rendered.as_bytes());
        hasher.update(&[0x0a]);
    }
    hasher.finalize().to_hex().to_string()
}

fn is_tool_allowed_by_policy(
    cfg: &iroha_config::parameters::actual::ToriiMcp,
    tool: &ToolSpec,
) -> bool {
    use iroha_config::parameters::actual::ToriiMcpProfile;

    let profile_allowed = match cfg.profile {
        ToriiMcpProfile::Operator => true,
        ToriiMcpProfile::Writer => true,
        ToriiMcpProfile::ReadOnly => {
            matches!(tool.method, Method::GET | Method::HEAD | Method::OPTIONS)
                || tool.name.contains(".query")
                || tool.name.ends_with(".get")
                || tool.name.ends_with(".list")
                || tool.name.ends_with(".status")
                || tool.name.ends_with(".health")
                || tool.name.ends_with(".now")
                || tool.name.ends_with(".capabilities")
                || tool.name.ends_with(".leader")
                || tool.name.ends_with(".phases")
                || tool.name.ends_with(".params")
                || tool.name.ends_with(".qc")
                || tool.name.ends_with(".headers")
                || tool.name.ends_with(".proof")
        }
    };
    if !profile_allowed {
        return false;
    }

    if cfg
        .deny_tool_prefixes
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .any(|prefix| !prefix.is_empty() && tool.name.starts_with(prefix))
    {
        return false;
    }

    if cfg.allow_tool_prefixes.is_empty() {
        return true;
    }

    cfg.allow_tool_prefixes
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .any(|prefix| !prefix.is_empty() && tool.name.starts_with(prefix))
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
        "initialize" => {
            let visible_tools = visible_tools_for_policy(&app.mcp, app.mcp_tools.as_slice());
            jsonrpc_result_response(id, capabilities_payload(&visible_tools))
        }
        "tools/list" => handle_tools_list(id, &app, &params),
        "tools/call_batch" => handle_tools_call_batch(id, app, inbound_headers, &params).await,
        "tools/call_async" => handle_tools_call_async(id, app, inbound_headers, &params).await,
        "tools/jobs/get" => handle_tools_jobs_get(id, &app.mcp, &params),
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
    let visible_tools = visible_tools_for_policy(&app.mcp, app.mcp_tools.as_slice());
    let toolset_version = compute_toolset_version(&visible_tools);
    let list_changed = params
        .get("toolset_version")
        .or_else(|| params.get("toolsetVersion"))
        .and_then(Value::as_str)
        .is_some_and(|client| client != toolset_version);
    let requested_start = params
        .get("cursor")
        .and_then(Value::as_str)
        .and_then(|cursor| cursor.parse::<usize>().ok())
        .unwrap_or(0);
    let start = requested_start.min(visible_tools.len());
    let page_size = app.mcp.max_tools_per_list.max(1);
    let end = start.saturating_add(page_size).min(visible_tools.len());

    let tools = visible_tools[start..end]
        .iter()
        .map(|tool| tool.descriptor())
        .collect::<Vec<_>>();
    let next_cursor = if end < visible_tools.len() {
        Value::String(end.to_string())
    } else {
        Value::Null
    };

    jsonrpc_result_response(
        id,
        norito::json!({
            "tools": tools,
            "nextCursor": next_cursor,
            "listChanged": list_changed,
            "toolsetVersion": toolset_version
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
    let Some(tool_spec) = app.mcp_tools.iter().find(|tool| tool.name == name) else {
        return jsonrpc_error_response(
            id,
            JSONRPC_INVALID_PARAMS,
            "tool not found",
            Some(norito::json!({ "name": name, "error_code": MCP_TOOL_NOT_FOUND })),
        );
    };
    if !is_tool_allowed_by_policy(&app.mcp, tool_spec) {
        return jsonrpc_error_response(
            id,
            JSONRPC_INVALID_PARAMS,
            "tool is not enabled by MCP policy",
            Some(norito::json!({ "name": name, "error_code": MCP_TOOL_NOT_ALLOWED })),
        );
    }
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
        "iroha.vpn.profile" => {
            match dispatch_iroha_vpn_profile(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.vpn.sessions.create" => {
            match dispatch_iroha_vpn_sessions_create(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.vpn.sessions.get" => {
            match dispatch_iroha_vpn_sessions_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.vpn.sessions.delete" => {
            match dispatch_iroha_vpn_sessions_delete(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.vpn.receipts.list" => {
            match dispatch_iroha_vpn_receipts_list(&app, inbound_headers, &arguments).await {
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
        "iroha.time.now" => {
            match dispatch_iroha_time_now(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.time.status" => {
            match dispatch_iroha_time_status(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.api.versions" => {
            match dispatch_iroha_api_versions(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.commit_certificates" => {
            match dispatch_iroha_sumeragi_commit_certificates(&app, inbound_headers, &arguments)
                .await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.validator_sets.list" => {
            match dispatch_iroha_sumeragi_validator_sets_list(&app, inbound_headers, &arguments)
                .await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.validator_sets.get" => {
            match dispatch_iroha_sumeragi_validator_sets_get(&app, inbound_headers, &arguments)
                .await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.rbc" => {
            match dispatch_iroha_sumeragi_rbc(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.pacemaker" => {
            match dispatch_iroha_sumeragi_pacemaker(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.phases" => {
            match dispatch_iroha_sumeragi_phases(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.params" => {
            match dispatch_iroha_sumeragi_params(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.status" => {
            match dispatch_iroha_sumeragi_status(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.leader" => {
            match dispatch_iroha_sumeragi_leader(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.qc" => {
            match dispatch_iroha_sumeragi_qc(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.checkpoints" => {
            match dispatch_iroha_sumeragi_checkpoints(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.consensus_keys" => {
            match dispatch_iroha_sumeragi_consensus_keys(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.bls_keys" => {
            match dispatch_iroha_sumeragi_bls_keys(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.key_lifecycle" => {
            match dispatch_iroha_sumeragi_key_lifecycle(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.telemetry" => {
            match dispatch_iroha_sumeragi_telemetry(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.rbc.sessions" => {
            match dispatch_iroha_sumeragi_rbc_sessions(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.commit_qc.get" => {
            match dispatch_iroha_sumeragi_commit_qc_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.collectors" => {
            match dispatch_iroha_sumeragi_collectors(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.evidence.count" => {
            match dispatch_iroha_sumeragi_evidence_count(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.evidence.list" => {
            match dispatch_iroha_sumeragi_evidence_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.evidence.submit" => {
            match dispatch_iroha_sumeragi_evidence_submit(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.new_view" => {
            match dispatch_iroha_sumeragi_new_view(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.rbc.delivered" => {
            match dispatch_iroha_sumeragi_rbc_delivered(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.vrf.penalties" => {
            match dispatch_iroha_sumeragi_vrf_penalties(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.vrf.epoch" => {
            match dispatch_iroha_sumeragi_vrf_epoch(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.vrf.commit" => {
            match dispatch_iroha_sumeragi_vrf_commit(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.vrf.reveal" => {
            match dispatch_iroha_sumeragi_vrf_reveal(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.sumeragi.rbc.sample" => {
            match dispatch_iroha_sumeragi_rbc_sample(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.da.ingest" => {
            match dispatch_iroha_da_ingest(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.da.proof_policies" => {
            match dispatch_iroha_da_proof_policies(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.da.proof_policy_snapshot" => {
            match dispatch_iroha_da_proof_policy_snapshot(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.da.manifests.get" => {
            match dispatch_iroha_da_manifests_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.da.commitments.list" => {
            match dispatch_iroha_da_commitments_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.da.commitments.prove" => {
            match dispatch_iroha_da_commitments_prove(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.da.commitments.verify" => {
            match dispatch_iroha_da_commitments_verify(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.da.pin_intents.list" => {
            match dispatch_iroha_da_pin_intents_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.da.pin_intents.prove" => {
            match dispatch_iroha_da_pin_intents_prove(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.da.pin_intents.verify" => {
            match dispatch_iroha_da_pin_intents_verify(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.runtime.abi.active" => {
            match dispatch_iroha_runtime_abi_active(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.runtime.abi.hash" => {
            match dispatch_iroha_runtime_abi_hash(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.runtime.metrics" => {
            match dispatch_iroha_runtime_metrics(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.runtime.upgrades.list" => {
            match dispatch_iroha_runtime_upgrades_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.runtime.upgrades.propose" => {
            match dispatch_iroha_runtime_upgrades_propose(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.runtime.upgrades.activate" => {
            match dispatch_iroha_runtime_upgrades_activate(&app, inbound_headers, &arguments).await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.runtime.upgrades.cancel" => {
            match dispatch_iroha_runtime_upgrades_cancel(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.ledger.headers" => {
            match dispatch_iroha_ledger_headers(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.ledger.state_root" => {
            match dispatch_iroha_ledger_state_root(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.ledger.state_proof" => {
            match dispatch_iroha_ledger_state_proof(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.ledger.block_proof" => {
            match dispatch_iroha_ledger_block_proof(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.bridge.finality.proof" => {
            match dispatch_iroha_bridge_finality_proof(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.bridge.finality.bundle" => {
            match dispatch_iroha_bridge_finality_bundle(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.proofs.get" => {
            match dispatch_iroha_proofs_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.proofs.query" => {
            match dispatch_iroha_proofs_query(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.proofs.retention" => {
            match dispatch_iroha_proofs_retention(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.instances.list" => {
            match dispatch_iroha_gov_instances_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.proposals.deploy_contract" => {
            match dispatch_iroha_gov_proposals_deploy_contract(&app, inbound_headers, &arguments)
                .await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.proposals.get" => {
            match dispatch_iroha_gov_proposals_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.locks.get" => {
            match dispatch_iroha_gov_locks_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.referenda.get" => {
            match dispatch_iroha_gov_referenda_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.tally.get" => {
            match dispatch_iroha_gov_tally_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.ballots.zk" => {
            match dispatch_iroha_gov_ballots_zk(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.ballots.zk_v1" => {
            match dispatch_iroha_gov_ballots_zk_v1(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.ballots.zk_v1.ballot_proof" => {
            match dispatch_iroha_gov_ballots_zk_v1_ballot_proof(&app, inbound_headers, &arguments)
                .await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.ballots.plain" => {
            match dispatch_iroha_gov_ballots_plain(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.protected_namespaces.list" => {
            match dispatch_iroha_gov_protected_namespaces_list(&app, inbound_headers, &arguments)
                .await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.protected_namespaces.update" => {
            match dispatch_iroha_gov_protected_namespaces_update(&app, inbound_headers, &arguments)
                .await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.unlocks.stats" => {
            match dispatch_iroha_gov_unlocks_stats(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.council.current" => {
            match dispatch_iroha_gov_council_current(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.council.persist" => {
            match dispatch_iroha_gov_council_persist(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.council.replace" => {
            match dispatch_iroha_gov_council_replace(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.council.audit" => {
            match dispatch_iroha_gov_council_audit(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.council.derive_vrf" => {
            match dispatch_iroha_gov_council_derive_vrf(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.enact" => {
            match dispatch_iroha_gov_enact(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.gov.finalize" => {
            match dispatch_iroha_gov_finalize(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.aliases.resolve" => {
            match dispatch_iroha_aliases_resolve(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.aliases.resolve_index" => {
            match dispatch_iroha_aliases_resolve_index(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.aliases.by_account" => {
            match dispatch_iroha_aliases_by_account(&app, inbound_headers, &arguments).await {
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
        "iroha.contracts.code.bytes.get" => {
            match dispatch_iroha_contracts_code_bytes_get(&app, inbound_headers, &arguments).await {
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
        "iroha.contracts.instances.list" => {
            match dispatch_iroha_contracts_instances_list(&app, inbound_headers, &arguments).await {
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
        "iroha.accounts.onboard" => {
            match dispatch_iroha_accounts_onboard(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.accounts.faucet" => {
            match dispatch_iroha_accounts_faucet(&app, inbound_headers, &arguments).await {
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
        "iroha.nfts.chain.list" => {
            match dispatch_iroha_nfts_chain_list(&app, inbound_headers, &arguments).await {
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
        "iroha.rwas.chain.list" => {
            match dispatch_iroha_rwas_chain_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.rwas.list" => {
            match dispatch_iroha_rwas_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.rwas.get" => {
            match dispatch_iroha_rwas_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.rwas.query" => {
            match dispatch_iroha_rwas_query(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.offline.transfers.list" => {
            match dispatch_iroha_offline_transfers_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.offline.transfers.get" => {
            match dispatch_iroha_offline_transfers_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.offline.transfers.query" => {
            match dispatch_iroha_offline_transfers_query(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.offline.cash.setup" => {
            match dispatch_iroha_offline_cash_setup(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.offline.cash.load" => {
            match dispatch_iroha_offline_cash_load(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.offline.cash.refresh" => {
            match dispatch_iroha_offline_cash_refresh(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.offline.cash.sync" => {
            match dispatch_iroha_offline_cash_sync(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.offline.cash.redeem" => {
            match dispatch_iroha_offline_cash_redeem(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.offline.revocations.list" => {
            match dispatch_iroha_offline_revocations_list(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.offline.revocations.bundle" => {
            match dispatch_iroha_offline_revocations_bundle(&app, inbound_headers, &arguments).await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.offline.revocations.register" => {
            match dispatch_iroha_offline_revocations_register(&app, inbound_headers, &arguments)
                .await
            {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.iso20022.pacs008.submit" => {
            match dispatch_iroha_iso20022_pacs008_submit(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.iso20022.pacs009.submit" => {
            match dispatch_iroha_iso20022_pacs009_submit(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.iso20022.status.get" => {
            match dispatch_iroha_iso20022_status_get(&app, inbound_headers, &arguments).await {
                Ok(result) => mcp_tool_success(result),
                Err(err) => mcp_tool_error(err),
            }
        }
        "iroha.queries.submit" => {
            match dispatch_iroha_queries_submit(&app, inbound_headers, &arguments).await {
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

async fn handle_tools_call_batch(
    id: Option<Value>,
    app: SharedAppState,
    inbound_headers: &HeaderMap,
    params: &Map,
) -> Value {
    let Some(calls) = params.get("calls").and_then(Value::as_array) else {
        return jsonrpc_error_response(
            id,
            JSONRPC_INVALID_PARAMS,
            "tools/call_batch params.calls must be an array",
            None,
        );
    };

    let mut results = Vec::with_capacity(calls.len());
    for call in calls {
        let Some(call_obj) = call.as_object() else {
            results.push(norito::json!({
                "error": {
                    "code": JSONRPC_INVALID_PARAMS,
                    "message": "batch item must be an object",
                    "data": { "error_code": "invalid_params" }
                }
            }));
            continue;
        };

        let Some(name) = call_obj.get("name").and_then(Value::as_str) else {
            results.push(norito::json!({
                "error": {
                    "code": JSONRPC_INVALID_PARAMS,
                    "message": "batch item `name` must be a string",
                    "data": { "error_code": "invalid_params" }
                }
            }));
            continue;
        };

        let args = call_obj
            .get("arguments")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let mut call_params = Map::new();
        call_params.insert("name".into(), Value::String(name.to_owned()));
        call_params.insert("arguments".into(), Value::Object(args));

        let response = handle_tools_call(None, app.clone(), inbound_headers, &call_params).await;
        if let Some(result) = response.get("result") {
            let result_value = result.clone();
            results.push(norito::json!({ "result": result_value }));
        } else if let Some(error) = response.get("error") {
            let error_value = error.clone();
            results.push(norito::json!({ "error": error_value }));
        } else {
            results.push(norito::json!({
                "error": {
                    "code": JSONRPC_INTERNAL_ERROR,
                    "message": "batch item returned malformed response",
                    "data": { "error_code": "internal_error" }
                }
            }));
        }
    }

    jsonrpc_result_response(id, norito::json!({ "results": results }))
}

async fn handle_tools_call_async(
    id: Option<Value>,
    app: SharedAppState,
    inbound_headers: &HeaderMap,
    params: &Map,
) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return jsonrpc_error_response(
            id,
            JSONRPC_INVALID_PARAMS,
            "tools/call_async params.name must be a string",
            None,
        );
    };
    let arguments = params
        .get("arguments")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let job_id = generate_mcp_job_id();
    upsert_async_job_state(
        &app.mcp,
        job_id.clone(),
        norito::json!({ "status": "pending" }),
    );

    let mut call_params = Map::new();
    call_params.insert("name".into(), Value::String(name.to_owned()));
    call_params.insert("arguments".into(), Value::Object(arguments));
    let headers = inbound_headers.clone();
    let app_cloned = app.clone();
    let job_id_cloned = job_id.clone();
    tokio::spawn(async move {
        let response = handle_tools_call(None, app_cloned.clone(), &headers, &call_params).await;
        let job_state = if let Some(result) = response.get("result") {
            let result_value = result.clone();
            norito::json!({
                "status": "completed",
                "result": result_value
            })
        } else if let Some(error) = response.get("error") {
            let error_value = error.clone();
            norito::json!({
                "status": "failed",
                "error": error_value
            })
        } else {
            norito::json!({
                "status": "failed",
                "error": {
                    "code": JSONRPC_INTERNAL_ERROR,
                    "message": "asynchronous call produced malformed response",
                    "data": { "error_code": "internal_error" }
                }
            })
        };
        upsert_async_job_state(&app_cloned.mcp, job_id_cloned, job_state);
    });

    jsonrpc_result_response(
        id,
        norito::json!({
            "job_id": job_id,
            "status": "pending"
        }),
    )
}

fn handle_tools_jobs_get(
    id: Option<Value>,
    mcp_cfg: &iroha_config::parameters::actual::ToriiMcp,
    params: &Map,
) -> Value {
    let Some(job_id) = params
        .get("job_id")
        .or_else(|| params.get("jobId"))
        .and_then(Value::as_str)
    else {
        return jsonrpc_error_response(
            id,
            JSONRPC_INVALID_PARAMS,
            "tools/jobs/get params.job_id must be a string",
            None,
        );
    };

    prune_async_jobs(mcp_cfg, Instant::now());

    let Some(state) = MCP_ASYNC_JOBS.get(job_id) else {
        return jsonrpc_error_response(
            id,
            JSONRPC_INVALID_PARAMS,
            "job not found",
            Some(norito::json!({
                "error_code": MCP_JOB_NOT_FOUND,
                "job_id": job_id
            })),
        );
    };
    let state_value = state.state.clone();

    jsonrpc_result_response(
        id,
        norito::json!({
            "job_id": job_id,
            "state": state_value
        }),
    )
}

fn upsert_async_job_state(
    mcp_cfg: &iroha_config::parameters::actual::ToriiMcp,
    job_id: String,
    state: Value,
) {
    let now = Instant::now();
    MCP_ASYNC_JOBS.insert(
        job_id,
        AsyncJobRecord {
            state,
            updated_at: now,
        },
    );
    prune_async_jobs(mcp_cfg, now);
}

fn prune_async_jobs(mcp_cfg: &iroha_config::parameters::actual::ToriiMcp, now: Instant) {
    let ttl = Duration::from_secs(mcp_cfg.async_job_ttl_secs.max(1));
    MCP_ASYNC_JOBS.retain(|_, entry| now.saturating_duration_since(entry.updated_at) <= ttl);

    let max_entries = mcp_cfg.async_job_max_entries.max(1);
    let current_len = MCP_ASYNC_JOBS.len();
    if current_len <= max_entries {
        return;
    }

    let mut by_age = MCP_ASYNC_JOBS
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().updated_at))
        .collect::<Vec<_>>();
    by_age.sort_by_key(|(_, updated_at)| *updated_at);

    let remove_count = current_len.saturating_sub(max_entries);
    for (job_id, _) in by_age.into_iter().take(remove_count) {
        MCP_ASYNC_JOBS.remove(&job_id);
    }
}

fn generate_mcp_job_id() -> String {
    let mut bytes = [0_u8; 20];
    let mut rng = OsRng;
    rng.try_fill_bytes(&mut bytes)
        .expect("OS RNG should be available");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn mcp_tool_success(structured: Value) -> Value {
    let status = structured.get("status").and_then(Value::as_u64);
    let is_http_error = status.is_some_and(|code| code >= 400);
    let mut structured = structured;
    if is_http_error && let Some(map) = structured.as_object_mut() {
        let error_code = status
            .map(http_status_error_code)
            .unwrap_or("http_error")
            .to_owned();
        map.entry("error_code".into())
            .or_insert(Value::String(error_code));
    }
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
    let error_message = message.clone();
    norito::json!({
        "content": [
            {
                "type": "text",
                "text": message
            }
        ],
        "isError": true,
        "structuredContent": {
            "error_code": MCP_TOOL_EXECUTION_ERROR_CODE,
            "message": error_message
        }
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
    let mut data_object = match data {
        Some(Value::Object(map)) => map,
        Some(other) => {
            let mut map = Map::new();
            map.insert("details".into(), other);
            map
        }
        None => Map::new(),
    };
    data_object
        .entry("error_code".into())
        .or_insert(Value::String(jsonrpc_error_code_label(code).to_owned()));

    let mut err = Map::new();
    err.insert("code".into(), Value::from(code));
    err.insert("message".into(), Value::String(message.to_owned()));
    err.insert("data".into(), Value::Object(data_object));
    let mut obj = Map::new();
    obj.insert("jsonrpc".into(), Value::String(JSONRPC_VERSION.to_owned()));
    obj.insert("id".into(), id.unwrap_or(Value::Null));
    obj.insert("error".into(), Value::Object(err));
    Value::Object(obj)
}

fn jsonrpc_error_code_label(code: i64) -> &'static str {
    match code {
        JSONRPC_PARSE_ERROR => "parse_error",
        JSONRPC_INVALID_REQUEST => "invalid_request",
        JSONRPC_METHOD_NOT_FOUND => "method_not_found",
        JSONRPC_INVALID_PARAMS => "invalid_params",
        JSONRPC_INTERNAL_ERROR => "internal_error",
        MCP_TOOL_EXECUTION_ERROR => MCP_TOOL_EXECUTION_ERROR_CODE,
        MCP_RATE_LIMITED => "rate_limited",
        _ => "unknown_error",
    }
}

fn http_status_error_code(status: u64) -> &'static str {
    match status {
        400 => "bad_request",
        401 => "unauthorized",
        403 => "forbidden",
        404 => "not_found",
        405 => "method_not_allowed",
        409 => "conflict",
        413 => "payload_too_large",
        415 => "unsupported_media_type",
        422 => "unprocessable_entity",
        429 => "rate_limited",
        500..=599 => "server_error",
        _ => "http_error",
    }
}

fn parse_parameters(spec: &Value, value: Option<&Value>) -> Vec<ParameterInfo> {
    let Some(array) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    array
        .iter()
        .map(|param| deref_openapi_value(spec, param))
        .filter_map(Value::as_object)
        .filter_map(|param| {
            let name = param.get("name").and_then(Value::as_str)?;
            let location = param.get("in").and_then(Value::as_str)?;
            let required = param
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let schema = param
                .get("schema")
                .map(|schema| deref_openapi_value(spec, schema).clone())
                .unwrap_or_else(string_schema);
            Some(ParameterInfo {
                name: name.to_owned(),
                location: location.to_owned(),
                required,
                schema,
            })
        })
        .collect()
}

fn build_input_schema(
    spec: &Value,
    path: &str,
    parameters: &[ParameterInfo],
    request_body: Option<&Value>,
) -> Value {
    let mut path_props = Map::new();
    let mut path_required = Vec::new();
    let mut query_props = Map::new();
    let mut header_props = Map::new();

    for param in parameters {
        match param.location.as_str() {
            "path" => {
                path_props.insert(param.name.clone(), param.schema.clone());
                if param.required || path.contains(&format!("{{{}}}", param.name)) {
                    path_required.push(Value::String(param.name.clone()));
                }
            }
            "query" => {
                query_props.insert(param.name.clone(), param.schema.clone());
            }
            "header" => {
                header_props.insert(param.name.clone(), param.schema.clone());
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

    if let Some(request_body) = request_body {
        let body_schema = build_request_body_schema(spec, request_body);
        properties.insert(
            "body".into(),
            body_schema.unwrap_or_else(|| {
                norito::json!({
                    "description": "Request body payload. JSON values are encoded as application/json unless `content_type` overrides it."
                })
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
    properties.insert(
        "project".into(),
        norito::json!({
            "type": "array",
            "description": "Optional projection keys applied to `structuredContent.body` object items.",
            "items": { "type": "string" }
        }),
    );

    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("object".to_owned()));
    schema.insert("properties".into(), Value::Object(properties));
    schema.insert("additionalProperties".into(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert("required".into(), Value::Array(required));
    }
    Value::Object(schema)
}

fn build_request_body_schema(spec: &Value, request_body: &Value) -> Option<Value> {
    let request_body = deref_openapi_value(spec, request_body);
    let content = request_body.get("content").and_then(Value::as_object)?;
    let mut schemas = Vec::new();
    for media in content.values() {
        let Some(media_obj) = media.as_object() else {
            continue;
        };
        let Some(schema) = media_obj.get("schema") else {
            continue;
        };
        schemas.push(deref_openapi_value(spec, schema).clone());
    }
    match schemas.len() {
        0 => None,
        1 => schemas.into_iter().next(),
        _ => Some(norito::json!({ "oneOf": schemas })),
    }
}

fn deref_openapi_value<'a>(spec: &'a Value, value: &'a Value) -> &'a Value {
    let mut current = value;
    for _ in 0..8 {
        let Some(reference) = current
            .as_object()
            .and_then(|obj| obj.get("$ref"))
            .and_then(Value::as_str)
        else {
            break;
        };
        let Some(resolved) = resolve_openapi_ref(spec, reference) else {
            break;
        };
        current = resolved;
    }
    current
}

fn resolve_openapi_ref<'a>(spec: &'a Value, reference: &str) -> Option<&'a Value> {
    let path = reference.strip_prefix("#/")?;
    let mut current = spec;
    for raw in path.split('/') {
        let key = raw.replace("~1", "/").replace("~0", "~");
        current = current.get(key.as_str())?;
    }
    Some(current)
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

    let structured = dispatch_route(
        app,
        inbound_headers,
        tool.method.clone(),
        route.as_str(),
        arguments.get("headers"),
        body,
        content_type,
        accept,
    )
    .await?;

    Ok(apply_body_projection(structured, arguments.get("project")))
}

async fn dispatch_connect_session_create(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_connect_session_create_body(arguments)?;
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

fn build_connect_session_create_body(arguments: &Map) -> Result<Value, String> {
    let node = arguments
        .get("node")
        .or_else(|| arguments.get("node_url"))
        .and_then(Value::as_str);

    let mut body = arguments
        .get("body")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));

    if let Value::Object(payload) = &mut body {
        if !payload.contains_key("sid") {
            let sid = arguments
                .get("sid")
                .or_else(|| arguments.get("session_id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(generate_connect_session_sid_b64url);
            payload.insert("sid".into(), Value::String(sid));
        }
        if !payload.contains_key("node") {
            if let Some(node) = node {
                payload.insert("node".into(), Value::String(node.to_owned()));
            }
        }
    }

    Ok(body)
}

fn generate_connect_session_sid_b64url() -> String {
    let mut sid = [0_u8; 32];
    let mut rng = OsRng;
    rng.try_fill_bytes(&mut sid)
        .expect("operating-system RNG should be available");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sid)
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
            .or_else(|| arguments.get("session_id").and_then(Value::as_str))
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
    let sid = extract_connect_sid_argument(arguments)?;
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

async fn dispatch_iroha_vpn_profile(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/vpn/profile",
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

async fn dispatch_iroha_vpn_sessions_create(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/vpn/sessions",
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

async fn dispatch_iroha_vpn_sessions_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let session_id = extract_vpn_session_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("session_id".into(), Value::String(session_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/vpn/sessions/{session_id}", Some(&path_value))?;
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

async fn dispatch_iroha_vpn_sessions_delete(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let session_id = extract_vpn_session_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("session_id".into(), Value::String(session_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/vpn/sessions/{session_id}", Some(&path_value))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::DELETE,
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

async fn dispatch_iroha_vpn_receipts_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/vpn/receipts",
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

async fn dispatch_iroha_time_now(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/time/now",
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

async fn dispatch_iroha_time_status(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/time/status",
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

async fn dispatch_iroha_api_versions(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/api/versions",
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

async fn dispatch_iroha_sumeragi_commit_certificates(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query(
        "/v1/sumeragi/commit-certificates".to_owned(),
        query.as_ref(),
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

async fn dispatch_iroha_sumeragi_validator_sets_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/validator-sets",
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

async fn dispatch_iroha_sumeragi_validator_sets_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let height = extract_height_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("height".into(), Value::String(height));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/sumeragi/validator-sets/{height}", Some(&path_value))?;
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

async fn dispatch_iroha_sumeragi_rbc(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/rbc",
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

async fn dispatch_iroha_sumeragi_pacemaker(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/pacemaker",
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

async fn dispatch_iroha_sumeragi_phases(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/phases",
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

async fn dispatch_iroha_sumeragi_params(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/params",
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

async fn dispatch_iroha_sumeragi_status(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/status",
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

async fn dispatch_iroha_sumeragi_leader(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/leader",
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

async fn dispatch_iroha_sumeragi_qc(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/qc",
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

async fn dispatch_iroha_sumeragi_checkpoints(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/checkpoints",
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

async fn dispatch_iroha_sumeragi_consensus_keys(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/consensus-keys",
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

async fn dispatch_iroha_sumeragi_bls_keys(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/bls_keys",
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

async fn dispatch_iroha_da_proof_policies(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/da/proof_policies",
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

async fn dispatch_iroha_sumeragi_key_lifecycle(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/key-lifecycle",
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

async fn dispatch_iroha_sumeragi_telemetry(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/telemetry",
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

async fn dispatch_iroha_sumeragi_rbc_sessions(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/rbc/sessions",
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

async fn dispatch_iroha_sumeragi_commit_qc_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let hash = extract_transaction_hash_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("hash".into(), Value::String(hash));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/sumeragi/commit_qc/{hash}", Some(&path_value))?;
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

async fn dispatch_iroha_sumeragi_collectors(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/collectors",
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

async fn dispatch_iroha_sumeragi_evidence_count(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/evidence/count",
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

async fn dispatch_iroha_sumeragi_evidence_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/sumeragi/evidence".to_owned(), query.as_ref())?;
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

async fn dispatch_iroha_sumeragi_evidence_submit(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/sumeragi/evidence/submit",
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

async fn dispatch_iroha_sumeragi_new_view(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/new_view/json",
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

async fn dispatch_iroha_sumeragi_rbc_delivered(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let height = extract_height_argument(arguments)?;
    let view = extract_view_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("height".into(), Value::String(height));
    path_args.insert("view".into(), Value::String(view));
    let path_value = Value::Object(path_args);
    let route = fill_path_template(
        "/v1/sumeragi/rbc/delivered/{height}/{view}",
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

async fn dispatch_iroha_sumeragi_vrf_penalties(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let epoch = extract_epoch_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("epoch".into(), Value::String(epoch));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/sumeragi/vrf/penalties/{epoch}", Some(&path_value))?;
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

async fn dispatch_iroha_sumeragi_vrf_epoch(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let epoch = extract_epoch_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("epoch".into(), Value::String(epoch));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/sumeragi/vrf/epoch/{epoch}", Some(&path_value))?;
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

async fn dispatch_iroha_sumeragi_vrf_commit(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/vrf/commit",
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

async fn dispatch_iroha_sumeragi_vrf_reveal(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/sumeragi/vrf/reveal",
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

async fn dispatch_iroha_sumeragi_rbc_sample(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/sumeragi/rbc/sample".to_owned(), query.as_ref())?;
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

async fn dispatch_iroha_da_ingest(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/da/ingest",
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

async fn dispatch_iroha_da_proof_policy_snapshot(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/da/proof_policy_snapshot",
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

async fn dispatch_iroha_da_manifests_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let ticket = extract_ticket_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("ticket".into(), Value::String(ticket));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/da/manifests/{ticket}", Some(&path_value))?;
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

async fn dispatch_iroha_da_commitments_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/da/commitments",
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

async fn dispatch_iroha_da_commitments_prove(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/da/commitments/prove",
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

async fn dispatch_iroha_da_commitments_verify(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/da/commitments/verify",
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

async fn dispatch_iroha_da_pin_intents_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/da/pin_intents",
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

async fn dispatch_iroha_da_pin_intents_prove(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/da/pin_intents/prove",
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

async fn dispatch_iroha_da_pin_intents_verify(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/da/pin_intents/verify",
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

async fn dispatch_iroha_runtime_abi_active(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/runtime/abi/active",
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

async fn dispatch_iroha_runtime_abi_hash(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/runtime/abi/hash",
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

async fn dispatch_iroha_runtime_metrics(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/runtime/metrics",
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

async fn dispatch_iroha_runtime_upgrades_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/runtime/upgrades",
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

async fn dispatch_iroha_runtime_upgrades_propose(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/runtime/upgrades/propose",
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

async fn dispatch_iroha_runtime_upgrades_activate(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_runtime_upgrades_action(
        app,
        inbound_headers,
        arguments,
        "/v1/runtime/upgrades/activate/{id}",
    )
    .await
}

async fn dispatch_iroha_runtime_upgrades_cancel(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_runtime_upgrades_action(
        app,
        inbound_headers,
        arguments,
        "/v1/runtime/upgrades/cancel/{id}",
    )
    .await
}

async fn dispatch_iroha_runtime_upgrades_action(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
    route_template: &str,
) -> Result<Value, String> {
    let upgrade_id = extract_runtime_upgrade_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("id".into(), Value::String(upgrade_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template(route_template, Some(&path_value))?;
    let body = build_object_body_or_flat_shortcuts(
        arguments,
        &["body", "path", "id", "upgrade_id", "headers", "accept"],
    )?;
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

async fn dispatch_iroha_ledger_headers(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/ledger/headers".to_owned(), query.as_ref())?;
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

async fn dispatch_iroha_ledger_state_root(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let height = extract_height_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("height".into(), Value::String(height));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/ledger/state/{height}", Some(&path_value))?;
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

async fn dispatch_iroha_ledger_state_proof(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let height = extract_height_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("height".into(), Value::String(height));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/ledger/state-proof/{height}", Some(&path_value))?;
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

async fn dispatch_iroha_ledger_block_proof(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let height = extract_height_argument(arguments)?;
    let entry_hash = extract_entry_hash_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("height".into(), Value::String(height));
    path_args.insert("entry_hash".into(), Value::String(entry_hash));
    let path_value = Value::Object(path_args);
    let route = fill_path_template(
        "/v1/ledger/block/{height}/proof/{entry_hash}",
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

async fn dispatch_iroha_bridge_finality_proof(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let height = extract_height_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("height".into(), Value::String(height));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/bridge/finality/{height}", Some(&path_value))?;
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

async fn dispatch_iroha_bridge_finality_bundle(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let height = extract_height_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("height".into(), Value::String(height));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/bridge/finality/bundle/{height}", Some(&path_value))?;
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

async fn dispatch_iroha_proofs_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let proof_id = extract_proof_record_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("id".into(), Value::String(proof_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/proofs/{id}", Some(&path_value))?;
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

async fn dispatch_iroha_proofs_query(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/proofs/query",
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

async fn dispatch_iroha_proofs_retention(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/proofs/retention",
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

async fn dispatch_iroha_gov_instances_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let namespace = extract_contract_namespace_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("ns".into(), Value::String(namespace));
    let path_value = Value::Object(path_args);
    let query = collect_query_arguments(arguments, &["path", "headers", "accept"])?;
    let route = fill_path_template("/v1/gov/instances/{ns}", Some(&path_value))?;
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

async fn dispatch_iroha_gov_proposals_deploy_contract(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/gov/proposals/deploy-contract",
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

async fn dispatch_iroha_gov_proposals_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let id = extract_governance_entity_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("id".into(), Value::String(id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/gov/proposals/{id}", Some(&path_value))?;
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

async fn dispatch_iroha_gov_locks_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let rid = extract_governance_entity_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("rid".into(), Value::String(rid));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/gov/locks/{rid}", Some(&path_value))?;
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

async fn dispatch_iroha_gov_referenda_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let id = extract_governance_entity_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("id".into(), Value::String(id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/gov/referenda/{id}", Some(&path_value))?;
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

async fn dispatch_iroha_gov_tally_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let id = extract_governance_entity_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("id".into(), Value::String(id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/gov/tally/{id}", Some(&path_value))?;
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

async fn dispatch_iroha_gov_ballots_zk(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/gov/ballots/zk",
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

async fn dispatch_iroha_gov_ballots_zk_v1(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/gov/ballots/zk-v1",
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

async fn dispatch_iroha_gov_ballots_zk_v1_ballot_proof(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/gov/ballots/zk-v1/ballot-proof",
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

async fn dispatch_iroha_gov_ballots_plain(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/gov/ballots/plain",
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

async fn dispatch_iroha_gov_protected_namespaces_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/gov/protected-namespaces",
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

async fn dispatch_iroha_gov_protected_namespaces_update(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/gov/protected-namespaces",
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

async fn dispatch_iroha_gov_unlocks_stats(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/gov/unlocks/stats",
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

async fn dispatch_iroha_gov_council_current(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/gov/council/current",
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

async fn dispatch_iroha_gov_council_persist(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/gov/council/persist",
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

async fn dispatch_iroha_gov_council_replace(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/gov/council/replace",
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

async fn dispatch_iroha_gov_council_audit(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/gov/council/audit",
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

async fn dispatch_iroha_gov_council_derive_vrf(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/gov/council/derive-vrf",
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

async fn dispatch_iroha_gov_enact(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/gov/enact",
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

async fn dispatch_iroha_gov_finalize(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/gov/finalize",
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

async fn dispatch_iroha_aliases_resolve(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/aliases/resolve",
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

async fn dispatch_iroha_aliases_resolve_index(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/aliases/resolve_index",
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

async fn dispatch_iroha_aliases_by_account(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/aliases/by_account",
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

async fn dispatch_iroha_contracts_code_bytes_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let code_hash = extract_code_hash_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("code_hash".into(), Value::String(code_hash));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/contracts/code-bytes/{code_hash}", Some(&path_value))?;
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

async fn dispatch_iroha_contracts_instances_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let namespace = extract_contract_namespace_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("ns".into(), Value::String(namespace));
    let path_value = Value::Object(path_args);
    let query = collect_query_arguments(arguments, &["path", "headers", "accept"])?;
    let route = fill_path_template("/v1/contracts/instances/{ns}", Some(&path_value))?;
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

async fn dispatch_iroha_accounts_faucet(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_accounts_faucet_body(arguments)?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/accounts/faucet",
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

async fn dispatch_iroha_nfts_chain_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/nfts",
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

async fn dispatch_iroha_rwas_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/explorer/rwas".to_owned(), query.as_ref())?;
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

async fn dispatch_iroha_rwas_chain_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/rwas",
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

async fn dispatch_iroha_rwas_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let rwa_id = extract_rwa_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("rwa_id".into(), Value::String(rwa_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/explorer/rwas/{rwa_id}", Some(&path_value))?;
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

async fn dispatch_iroha_rwas_query(
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
        "/v1/rwas/query",
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

async fn dispatch_iroha_offline_transfers_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/offline/transfers".to_owned(), query.as_ref())?;
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

async fn dispatch_iroha_offline_transfers_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let bundle_id_hex = extract_bundle_id_hex_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("bundle_id_hex".into(), Value::String(bundle_id_hex));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/offline/transfers/{bundle_id_hex}", Some(&path_value))?;
    let query = collect_query_arguments(arguments, &["query", "headers", "accept", "path"])?;
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

async fn dispatch_iroha_offline_transfers_query(
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
        "/v1/offline/transfers/query",
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

async fn dispatch_iroha_offline_cash_setup(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_offline_cash_post(app, inbound_headers, arguments, "/v1/offline/cash/setup")
        .await
}

async fn dispatch_iroha_offline_cash_load(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_offline_cash_post(app, inbound_headers, arguments, "/v1/offline/cash/load").await
}

async fn dispatch_iroha_offline_cash_refresh(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_offline_cash_post(app, inbound_headers, arguments, "/v1/offline/cash/refresh")
        .await
}

async fn dispatch_iroha_offline_cash_sync(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_offline_cash_post(app, inbound_headers, arguments, "/v1/offline/cash/sync").await
}

async fn dispatch_iroha_offline_cash_redeem(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_iroha_offline_cash_post(app, inbound_headers, arguments, "/v1/offline/cash/redeem")
        .await
}

async fn dispatch_iroha_offline_cash_post(
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

async fn dispatch_iroha_offline_revocations_list(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let query = collect_query_arguments(arguments, &["query", "headers", "accept"])?;
    let route = append_query("/v1/offline/revocations".to_owned(), query.as_ref())?;
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

async fn dispatch_iroha_offline_revocations_bundle(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    dispatch_route(
        app,
        inbound_headers,
        Method::GET,
        "/v1/offline/revocations/bundle",
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

async fn dispatch_iroha_offline_revocations_register(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let body = build_object_body_or_flat_shortcuts(arguments, &["body", "headers", "accept"])?;
    let body_bytes = json::to_vec(&body).map_err(|err| format!("encode request body: {err}"))?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/offline/revocations",
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

async fn dispatch_iroha_queries_submit(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let mut adapted = arguments.clone();
    if !adapted.contains_key("body_base64") && !adapted.contains_key("body") {
        if let Some(encoded) = arguments
            .get("signed_query_base64")
            .or_else(|| arguments.get("query_base64"))
            .and_then(Value::as_str)
        {
            adapted.insert("body_base64".into(), Value::String(encoded.to_owned()));
        } else if let Some(encoded_hex) = arguments
            .get("body_hex")
            .or_else(|| arguments.get("signed_query_hex"))
            .or_else(|| arguments.get("query_hex"))
            .and_then(Value::as_str)
        {
            let bytes = hex::decode(encoded_hex)
                .map_err(|err| format!("query hex payload must be valid hex: {err}"))?;
            adapted.insert(
                "body_base64".into(),
                Value::String(base64::engine::general_purpose::STANDARD.encode(bytes)),
            );
        }
    }

    if !adapted.contains_key("body_base64") && !adapted.contains_key("body") {
        return Err("one of `body_base64`, `signed_query_base64`, `query_base64`, `body_hex`, `signed_query_hex`, `query_hex`, or `body` is required".to_owned());
    }

    let (body, content_type) = build_request_body(&adapted)?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/query",
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

fn build_iso20022_payload_body(arguments: &Map) -> Result<(Vec<u8>, Option<String>), String> {
    let mut adapted = arguments.clone();
    if !adapted.contains_key("body_base64") && !adapted.contains_key("body") {
        if let Some(xml) = arguments
            .get("message_xml")
            .or_else(|| arguments.get("xml"))
            .and_then(Value::as_str)
        {
            adapted.insert(
                "body_base64".into(),
                Value::String(base64::engine::general_purpose::STANDARD.encode(xml.as_bytes())),
            );
            if !adapted.contains_key("content_type") {
                adapted.insert(
                    "content_type".into(),
                    Value::String("application/xml".to_owned()),
                );
            }
        }
    }

    if !adapted.contains_key("body_base64") && !adapted.contains_key("body") {
        return Err("one of `body_base64`, `message_xml`, `xml`, or `body` is required".to_owned());
    }

    build_request_body(&adapted)
}

async fn dispatch_iroha_iso20022_pacs008_submit(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let (body, content_type) = build_iso20022_payload_body(arguments)?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/iso20022/pacs008",
        arguments.get("headers"),
        body,
        content_type,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_iso20022_pacs009_submit(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let (body, content_type) = build_iso20022_payload_body(arguments)?;
    dispatch_route(
        app,
        inbound_headers,
        Method::POST,
        "/v1/iso20022/pacs009",
        arguments.get("headers"),
        body,
        content_type,
        arguments
            .get("accept")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
    .await
}

async fn dispatch_iroha_iso20022_status_get(
    app: &SharedAppState,
    inbound_headers: &HeaderMap,
    arguments: &Map,
) -> Result<Value, String> {
    let msg_id = extract_iso20022_message_id_argument(arguments)?;
    let mut path_args = Map::new();
    path_args.insert("msg_id".into(), Value::String(msg_id));
    let path_value = Value::Object(path_args);
    let route = fill_path_template("/v1/iso20022/status/{msg_id}", Some(&path_value))?;
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
        "`hash` is required (provide `hash`, `transaction_hash`, `query.hash`, or `query.transaction_hash`)".to_owned()
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
    let mut query = collect_query_map(
        arguments,
        &["query", "headers", "accept", "hash", "transaction_hash"],
    )?;
    if !query
        .get("hash")
        .and_then(Value::as_str)
        .is_some_and(|hash| !hash.is_empty())
    {
        if let Some(hash) = extract_optional_transaction_hash_argument(arguments) {
            query.insert("hash".into(), Value::String(hash));
        }
    }
    // Canonicalize alias query keys before route dispatch.
    query.remove("transaction_hash");
    if !query
        .get("hash")
        .and_then(Value::as_str)
        .is_some_and(|hash| !hash.is_empty())
    {
        return Err(
            "`hash` is required (provide `hash`, `transaction_hash`, `query.hash`, or `query.transaction_hash`)"
                .to_owned(),
        );
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
    if let Some(query) = arguments.get("query").and_then(Value::as_object)
        && let Some(hash) = query.get("transaction_hash").and_then(Value::as_str)
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

fn extract_connect_sid_argument(arguments: &Map) -> Result<&str, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(sid) = path.get("sid").and_then(Value::as_str)
            && !sid.is_empty()
        {
            return Ok(sid);
        }
        if let Some(sid) = path.get("session_id").and_then(Value::as_str)
            && !sid.is_empty()
        {
            return Ok(sid);
        }
    }

    arguments
        .get("sid")
        .or_else(|| arguments.get("session_id"))
        .and_then(Value::as_str)
        .filter(|sid| !sid.is_empty())
        .ok_or_else(|| {
            "`sid` is required (provide `sid`, `session_id`, `path.sid`, or `path.session_id`)"
                .to_owned()
        })
}

fn extract_vpn_session_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(session_id) = path.get("session_id").and_then(Value::as_str)
            && !session_id.is_empty()
        {
            return Ok(session_id.to_owned());
        }
        if let Some(session_id) = path.get("id").and_then(Value::as_str)
            && !session_id.is_empty()
        {
            return Ok(session_id.to_owned());
        }
    }

    arguments
        .get("session_id")
        .or_else(|| arguments.get("id"))
        .and_then(Value::as_str)
        .filter(|session_id| !session_id.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            "`session_id` is required (provide `session_id`, `id`, `path.session_id`, or `path.id`)"
                .to_owned()
        })
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

fn extract_iso20022_message_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(msg_id) = path.get("msg_id").and_then(Value::as_str) {
            return Ok(msg_id.to_owned());
        }
    }
    arguments
        .get("msg_id")
        .or_else(|| arguments.get("message_id"))
        .or_else(|| arguments.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`msg_id` is required (provide `msg_id`, `message_id`, `id`, or `path.msg_id`)"
                .to_owned()
        })
}

fn extract_ticket_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(ticket) = path.get("ticket").and_then(Value::as_str) {
            return Ok(ticket.to_owned());
        }
    }
    arguments
        .get("ticket")
        .or_else(|| arguments.get("manifest_ticket"))
        .or_else(|| arguments.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`ticket` is required (provide `ticket`, `manifest_ticket`, `id`, or `path.ticket`)"
                .to_owned()
        })
}

fn extract_proof_record_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(proof_id) = path.get("id").and_then(Value::as_str) {
            return Ok(proof_id.to_owned());
        }
    }
    arguments
        .get("id")
        .or_else(|| arguments.get("proof_id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "`id` is required (provide `id`, `proof_id`, or `path.id`)".to_owned())
}

fn extract_governance_entity_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(id) = path.get("id").and_then(Value::as_str) {
            return Ok(id.to_owned());
        }
        if let Some(rid) = path.get("rid").and_then(Value::as_str) {
            return Ok(rid.to_owned());
        }
    }
    arguments
        .get("id")
        .or_else(|| arguments.get("rid"))
        .or_else(|| arguments.get("proposal_id"))
        .or_else(|| arguments.get("referendum_id"))
        .or_else(|| arguments.get("tally_id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`id` is required (provide `id`, `rid`, `proposal_id`, `referendum_id`, `tally_id`, `path.id`, or `path.rid`)".to_owned()
        })
}

fn extract_runtime_upgrade_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(upgrade_id) = path.get("id").and_then(Value::as_str) {
            return Ok(upgrade_id.to_owned());
        }
    }
    arguments
        .get("id")
        .or_else(|| arguments.get("upgrade_id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "`id` is required (provide `id`, `upgrade_id`, or `path.id`)".to_owned())
}

fn extract_height_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(height) = path.get("height").and_then(value_to_string) {
            return Ok(height);
        }
    }
    arguments
        .get("height")
        .or_else(|| arguments.get("block_height"))
        .and_then(value_to_string)
        .ok_or_else(|| {
            "`height` is required (provide `height`, `block_height`, or `path.height`)".to_owned()
        })
}

fn extract_view_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(view) = path.get("view").and_then(value_to_string) {
            return Ok(view);
        }
    }
    arguments
        .get("view")
        .and_then(value_to_string)
        .ok_or_else(|| "`view` is required (provide `view` or `path.view`)".to_owned())
}

fn extract_epoch_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(epoch) = path.get("epoch").and_then(value_to_string) {
            return Ok(epoch);
        }
    }
    arguments
        .get("epoch")
        .and_then(value_to_string)
        .ok_or_else(|| "`epoch` is required (provide `epoch` or `path.epoch`)".to_owned())
}

fn extract_entry_hash_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(entry_hash) = path.get("entry_hash").and_then(Value::as_str) {
            return Ok(entry_hash.to_owned());
        }
    }
    arguments
        .get("entry_hash")
        .or_else(|| arguments.get("tx_hash"))
        .or_else(|| arguments.get("hash"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`entry_hash` is required (provide `entry_hash`, `tx_hash`, `hash`, or `path.entry_hash`)".to_owned()
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

fn extract_rwa_id_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(rwa_id) = path.get("rwa_id").and_then(Value::as_str) {
            return Ok(rwa_id.to_owned());
        }
    }
    arguments
        .get("rwa_id")
        .or_else(|| arguments.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "`rwa_id` is required (provide `rwa_id`, `id`, or `path.rwa_id`)".to_owned())
}

fn extract_bundle_id_hex_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(bundle_id_hex) = path.get("bundle_id_hex").and_then(Value::as_str) {
            return Ok(bundle_id_hex.to_owned());
        }
    }
    arguments
        .get("bundle_id_hex")
        .or_else(|| arguments.get("bundle_id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`bundle_id_hex` is required (provide `bundle_id_hex`, `bundle_id`, or `path.bundle_id_hex`)".to_owned()
        })
}

fn extract_certificate_id_hex_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(certificate_id_hex) = path.get("certificate_id_hex").and_then(Value::as_str) {
            return Ok(certificate_id_hex.to_owned());
        }
    }
    arguments
        .get("certificate_id_hex")
        .or_else(|| arguments.get("certificate_id"))
        .or_else(|| arguments.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`certificate_id_hex` is required (provide `certificate_id_hex`, `certificate_id`, `id`, or `path.certificate_id_hex`)".to_owned()
        })
}

fn extract_transaction_hash_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(hash) = path.get("hash").and_then(Value::as_str) {
            return Ok(hash.to_owned());
        }
        if let Some(hash) = path.get("transaction_hash").and_then(Value::as_str) {
            return Ok(hash.to_owned());
        }
    }
    arguments
        .get("hash")
        .or_else(|| arguments.get("transaction_hash"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            "`hash` is required (provide `hash`, `transaction_hash`, `path.hash`, or `path.transaction_hash`)".to_owned()
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

fn extract_contract_namespace_argument(arguments: &Map) -> Result<String, String> {
    if let Some(path) = arguments.get("path") {
        let path = path
            .as_object()
            .ok_or_else(|| "`path` must be an object".to_owned())?;
        if let Some(namespace) = path.get("ns").and_then(Value::as_str) {
            return Ok(namespace.to_owned());
        }
    }
    arguments
        .get("ns")
        .or_else(|| arguments.get("namespace"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "`ns` is required (provide `ns`, `namespace`, or `path.ns`)".to_owned())
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
    for key in ["query", "filter", "select", "sort", "fetch_size"] {
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

fn build_accounts_faucet_body(arguments: &Map) -> Result<Value, String> {
    if let Some(body) = arguments.get("body") {
        return body
            .as_object()
            .map(|_| body.clone())
            .ok_or_else(|| "`body` must be an object".to_owned());
    }

    let account_id = arguments
        .get("account_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "`account_id` is required (or provide `body.account_id`)".to_owned())?;

    let mut payload = Map::new();
    payload.insert(
        "account_id".to_owned(),
        Value::String(account_id.to_owned()),
    );
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
    let dispatched_remote_ip = dispatched_remote_ip(inbound_headers);
    let dispatched_connect_addr = dispatched_connect_addr(dispatched_remote_ip);
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
        let remote_addr_header = HeaderName::from_static(limits::REMOTE_ADDR_HEADER);
        headers.remove(&remote_addr_header);
        if let Some(remote_ip) = dispatched_remote_ip {
            let value = HeaderValue::from_str(&remote_ip.to_string())
                .map_err(|err| format!("invalid remote addr header: {err}"))?;
            headers.insert(remote_addr_header, value);
        }
    }

    let router = {
        let guard = app
            .mcp_dispatch_router
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard
            .clone()
            .ok_or_else(|| "mcp router unavailable".to_owned())?
    };

    let service = router
        .into_make_service_with_connect_info::<SocketAddr>()
        .oneshot(dispatched_connect_addr)
        .await
        .map_err(|err| format!("dispatch connect-info failed: {err}"))?;

    let response = service
        .oneshot(request)
        .await
        .map_err(|err| format!("dispatch failed: {err}"))?;
    response_to_value(response).await
}

fn dispatched_remote_ip(inbound_headers: &HeaderMap) -> Option<IpAddr> {
    inbound_headers
        .get(limits::REMOTE_ADDR_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
}

fn dispatched_connect_addr(remote_ip: Option<IpAddr>) -> SocketAddr {
    SocketAddr::new(remote_ip.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)), 0)
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
        if lowered == "content-length"
            || lowered == "host"
            || lowered == "connection"
            || lowered == limits::REMOTE_ADDR_HEADER
            || lowered == "x-forwarded-client-cert"
        {
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

fn apply_body_projection(mut structured: Value, projection: Option<&Value>) -> Value {
    let Some(keys) = projection.and_then(parse_projection_keys) else {
        return structured;
    };
    if keys.is_empty() {
        return structured;
    }
    if let Some(body) = structured
        .as_object_mut()
        .and_then(|payload| payload.get_mut("body"))
    {
        project_value_keys(body, &keys);
    }
    structured
}

fn parse_projection_keys(value: &Value) -> Option<Vec<String>> {
    let keys = value
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    Some(keys)
}

fn project_value_keys(value: &mut Value, keys: &[String]) {
    match value {
        Value::Object(object) => {
            object.retain(|key, _| keys.iter().any(|needle| needle == key));
        }
        Value::Array(items) => {
            for item in items {
                if let Some(object) = item.as_object_mut() {
                    object.retain(|key, _| keys.iter().any(|needle| needle == key));
                }
            }
        }
        _ => {}
    }
}

fn build_connect_ws_ticket(arguments: &Map, inbound_headers: &HeaderMap) -> Result<Value, String> {
    let sid = extract_connect_sid_argument(arguments)?;
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
            "required": ["role"],
            "anyOf": [
                { "required": ["sid"] },
                { "required": ["session_id"] }
            ],
            "properties": {
                "sid": { "type": "string" },
                "session_id": {
                    "type": "string",
                    "description": "Alias for `sid`."
                },
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
                    "description": "Convenience shortcut for `body.sid` (base64url session id). When omitted, MCP generates a random 32-byte SID."
                },
                "session_id": {
                    "type": "string",
                    "description": "Alias for `sid` convenience shortcut."
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
                    "description": "Raw Connect session request body. If provided, it takes precedence over `sid`/`node` values already present in `body`."
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
                    "description": "Convenience shortcut for `body.sid` (base64url session id). When omitted, MCP generates a random 32-byte SID."
                },
                "session_id": {
                    "type": "string",
                    "description": "Alias for `sid` convenience shortcut."
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
                    "description": "Raw Connect session request body. If provided, it takes precedence over `sid`/`node` values already present in `body`."
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
            "anyOf": [
                { "required": ["sid"] },
                { "required": ["session_id"] },
                { "required": ["path"] }
            ],
            "properties": {
                "sid": { "type": "string" },
                "session_id": {
                    "type": "string",
                    "description": "Alias for `sid`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "anyOf": [
                        { "required": ["sid"] },
                        { "required": ["session_id"] }
                    ],
                    "properties": {
                        "sid": { "type": "string" },
                        "session_id": {
                            "type": "string",
                            "description": "Alias for `path.sid`."
                        }
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

fn iroha_vpn_profile_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.vpn.profile".to_owned(),
        description: "Fetch the public Sora VPN profile advertised by Torii.".to_owned(),
        method: Method::GET,
        path_template: "/v1/vpn/profile".to_owned(),
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

fn iroha_vpn_sessions_create_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.vpn.sessions.create".to_owned(),
        description: "Create a signed Sora VPN session for the active wallet account.".to_owned(),
        method: Method::POST,
        path_template: "/v1/vpn/sessions".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "exit_class": {
                    "type": "string",
                    "description": "Convenience shortcut for `body.exit_class`."
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw VPN session create request body. If provided, its own fields take precedence over flat shortcuts."
                },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" },
                    "description": "Optional request headers. Signed VPN session routes normally use canonical `X-Iroha-*` headers."
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_vpn_sessions_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.vpn.sessions.get".to_owned(),
        description: "Fetch the current status of a signed Sora VPN session.".to_owned(),
        method: Method::GET,
        path_template: "/v1/vpn/sessions/{session_id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "anyOf": [
                { "required": ["session_id"] },
                { "required": ["id"] },
                { "required": ["path"] }
            ],
            "properties": {
                "session_id": { "type": "string" },
                "id": {
                    "type": "string",
                    "description": "Alias for `session_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "anyOf": [
                        { "required": ["session_id"] },
                        { "required": ["id"] }
                    ],
                    "properties": {
                        "session_id": { "type": "string" },
                        "id": {
                            "type": "string",
                            "description": "Alias for `path.session_id`."
                        }
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

fn iroha_vpn_sessions_delete_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.vpn.sessions.delete".to_owned(),
        description: "Delete a signed Sora VPN session and return its canonical receipt."
            .to_owned(),
        method: Method::DELETE,
        path_template: "/v1/vpn/sessions/{session_id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "anyOf": [
                { "required": ["session_id"] },
                { "required": ["id"] },
                { "required": ["path"] }
            ],
            "properties": {
                "session_id": { "type": "string" },
                "id": {
                    "type": "string",
                    "description": "Alias for `session_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "anyOf": [
                        { "required": ["session_id"] },
                        { "required": ["id"] }
                    ],
                    "properties": {
                        "session_id": { "type": "string" },
                        "id": {
                            "type": "string",
                            "description": "Alias for `path.session_id`."
                        }
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

fn iroha_vpn_receipts_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.vpn.receipts.list".to_owned(),
        description: "List canonical Sora VPN receipts for the active wallet account.".to_owned(),
        method: Method::GET,
        path_template: "/v1/vpn/receipts".to_owned(),
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

fn iroha_time_now_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.time.now".to_owned(),
        description: "Get node wall-clock snapshot (`/v1/time/now`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/time/now".to_owned(),
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

fn iroha_time_status_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.time.status".to_owned(),
        description: "Get time synchronization status (`/v1/time/status`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/time/status".to_owned(),
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

fn iroha_api_versions_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.api.versions".to_owned(),
        description: "List supported Torii API versions (`/v1/api/versions`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/api/versions".to_owned(),
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

fn iroha_sumeragi_commit_certificates_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.commit_certificates".to_owned(),
        description:
            "List recent commit certificates (`/v1/sumeragi/commit-certificates`) with optional `from`/`limit` query shortcuts."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/commit-certificates".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "from": { "type": "integer" },
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

fn iroha_sumeragi_validator_sets_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.validator_sets.list".to_owned(),
        description: "List validator set snapshots (`/v1/sumeragi/validator-sets`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/validator-sets".to_owned(),
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

fn iroha_sumeragi_validator_sets_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.validator_sets.get".to_owned(),
        description: "Fetch validator set snapshot by height (`/v1/sumeragi/validator-sets/{height}`; `height`/`block_height` shortcuts supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/validator-sets/{height}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "height": {
                    "type": "integer",
                    "description": "Convenience shortcut for `path.height`."
                },
                "block_height": {
                    "type": "integer",
                    "description": "Alias for `height`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["height"],
                    "properties": {
                        "height": { "type": "integer" }
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

fn iroha_sumeragi_rbc_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.rbc".to_owned(),
        description: "Fetch RBC status (`/v1/sumeragi/rbc`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/rbc".to_owned(),
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

fn iroha_sumeragi_pacemaker_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.pacemaker".to_owned(),
        description: "Fetch pacemaker status (`/v1/sumeragi/pacemaker`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/pacemaker".to_owned(),
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

fn iroha_sumeragi_phases_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.phases".to_owned(),
        description: "Fetch phase status (`/v1/sumeragi/phases`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/phases".to_owned(),
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

fn iroha_sumeragi_params_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.params".to_owned(),
        description: "Fetch Sumeragi parameters (`/v1/sumeragi/params`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/params".to_owned(),
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

fn iroha_sumeragi_status_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.status".to_owned(),
        description: "Fetch Sumeragi status snapshot (`/v1/sumeragi/status`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/status".to_owned(),
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

fn iroha_sumeragi_leader_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.leader".to_owned(),
        description: "Fetch current Sumeragi leader info (`/v1/sumeragi/leader`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/leader".to_owned(),
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

fn iroha_sumeragi_qc_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.qc".to_owned(),
        description: "Fetch latest Sumeragi quorum-certificate summary (`/v1/sumeragi/qc`)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/qc".to_owned(),
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

fn iroha_sumeragi_checkpoints_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.checkpoints".to_owned(),
        description: "Fetch Sumeragi checkpoint summary (`/v1/sumeragi/checkpoints`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/checkpoints".to_owned(),
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

fn iroha_sumeragi_consensus_keys_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.consensus_keys".to_owned(),
        description: "Fetch active Sumeragi consensus keys (`/v1/sumeragi/consensus-keys`)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/consensus-keys".to_owned(),
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

fn iroha_sumeragi_bls_keys_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.bls_keys".to_owned(),
        description: "Fetch Sumeragi BLS key roster (`/v1/sumeragi/bls_keys`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/bls_keys".to_owned(),
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

fn iroha_sumeragi_key_lifecycle_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.key_lifecycle".to_owned(),
        description: "Fetch Sumeragi key lifecycle snapshots (`/v1/sumeragi/key-lifecycle`)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/key-lifecycle".to_owned(),
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

fn iroha_sumeragi_telemetry_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.telemetry".to_owned(),
        description: "Fetch Sumeragi telemetry snapshot (`/v1/sumeragi/telemetry`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/telemetry".to_owned(),
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

fn iroha_sumeragi_rbc_sessions_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.rbc.sessions".to_owned(),
        description: "List Sumeragi RBC sessions (`/v1/sumeragi/rbc/sessions`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/rbc/sessions".to_owned(),
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

fn iroha_sumeragi_commit_qc_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.commit_qc.get".to_owned(),
        description: "Fetch Sumeragi commit QC by block hash (`/v1/sumeragi/commit_qc/{hash}`; `hash` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/commit_qc/{hash}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "hash": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.hash`."
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

fn iroha_sumeragi_collectors_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.collectors".to_owned(),
        description: "Fetch Sumeragi collectors snapshot (`/v1/sumeragi/collectors`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/collectors".to_owned(),
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

fn iroha_sumeragi_evidence_count_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.evidence.count".to_owned(),
        description: "Fetch Sumeragi evidence count (`/v1/sumeragi/evidence/count`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/evidence/count".to_owned(),
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

fn iroha_sumeragi_evidence_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.evidence.list".to_owned(),
        description:
            "List Sumeragi evidence entries (`/v1/sumeragi/evidence`) with optional query shortcuts."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/evidence".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
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

fn iroha_sumeragi_evidence_submit_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.evidence.submit".to_owned(),
        description:
            "Submit consensus evidence (`/v1/sumeragi/evidence/submit`); accepts raw `body` or flat top-level body shortcuts."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/sumeragi/evidence/submit".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Evidence submit payload (expects `evidence_hex`)."
                },
                "evidence_hex": {
                    "type": "string",
                    "description": "Convenience shortcut copied into the request body when `body` is omitted."
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

fn iroha_sumeragi_new_view_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.new_view".to_owned(),
        description: "Fetch NEW_VIEW counters (`/v1/sumeragi/new_view/json`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/new_view/json".to_owned(),
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

fn iroha_sumeragi_rbc_delivered_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.rbc.delivered".to_owned(),
        description: "Fetch RBC delivered status (`/v1/sumeragi/rbc/delivered/{height}/{view}`; `height`/`block_height` and `view` shortcuts supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/rbc/delivered/{height}/{view}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "height": {
                    "type": "integer",
                    "description": "Convenience shortcut for `path.height`."
                },
                "block_height": {
                    "type": "integer",
                    "description": "Alias for `height`."
                },
                "view": {
                    "type": "integer",
                    "description": "Convenience shortcut for `path.view`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["height", "view"],
                    "properties": {
                        "height": { "type": "integer" },
                        "view": { "type": "integer" }
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

fn iroha_sumeragi_vrf_penalties_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.vrf.penalties".to_owned(),
        description: "Fetch VRF penalties for an epoch (`/v1/sumeragi/vrf/penalties/{epoch}`; `epoch` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/vrf/penalties/{epoch}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "epoch": {
                    "type": "integer",
                    "description": "Convenience shortcut for `path.epoch`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["epoch"],
                    "properties": {
                        "epoch": { "type": "integer" }
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

fn iroha_sumeragi_vrf_epoch_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.vrf.epoch".to_owned(),
        description: "Fetch VRF epoch snapshot (`/v1/sumeragi/vrf/epoch/{epoch}`; `epoch` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/vrf/epoch/{epoch}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "epoch": {
                    "type": "integer",
                    "description": "Convenience shortcut for `path.epoch`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["epoch"],
                    "properties": {
                        "epoch": { "type": "integer" }
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

fn iroha_sumeragi_vrf_commit_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.vrf.commit".to_owned(),
        description: "Fetch latest VRF commit snapshot (`/v1/sumeragi/vrf/commit`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/vrf/commit".to_owned(),
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

fn iroha_sumeragi_vrf_reveal_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.vrf.reveal".to_owned(),
        description: "Fetch latest VRF reveal snapshot (`/v1/sumeragi/vrf/reveal`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/vrf/reveal".to_owned(),
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

fn iroha_sumeragi_rbc_sample_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.sumeragi.rbc.sample".to_owned(),
        description:
            "Fetch RBC sampled sessions (`/v1/sumeragi/rbc/sample`) with optional query shortcuts."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/sumeragi/rbc/sample".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
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

fn iroha_da_ingest_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.da.ingest".to_owned(),
        description:
            "Ingest DA payload (`/v1/da/ingest`); accepts raw `body` or flat top-level body shortcuts."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/da/ingest".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw DA ingest request payload."
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

fn iroha_da_proof_policies_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.da.proof_policies".to_owned(),
        description: "Fetch DA proof policies (`/v1/da/proof_policies`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/da/proof_policies".to_owned(),
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

fn iroha_da_proof_policy_snapshot_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.da.proof_policy_snapshot".to_owned(),
        description: "Fetch DA proof policy snapshot (`/v1/da/proof_policy_snapshot`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/da/proof_policy_snapshot".to_owned(),
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

fn iroha_da_manifests_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.da.manifests.get".to_owned(),
        description:
            "Fetch DA manifest payload (`/v1/da/manifests/{ticket}`; `ticket`/`manifest_ticket`/`id` shortcuts supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/da/manifests/{ticket}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "ticket": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.ticket`."
                },
                "manifest_ticket": {
                    "type": "string",
                    "description": "Alias for `ticket`."
                },
                "id": {
                    "type": "string",
                    "description": "Alias for `ticket`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["ticket"],
                    "properties": {
                        "ticket": { "type": "string" }
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

fn iroha_da_commitments_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.da.commitments.list".to_owned(),
        description:
            "List DA commitments (`/v1/da/commitments`); accepts raw `body` or flat top-level body shortcuts."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/da/commitments".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw DA commitment list request payload."
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

fn iroha_da_commitments_prove_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.da.commitments.prove".to_owned(),
        description:
            "Compute DA commitment proof placeholder (`/v1/da/commitments/prove`); accepts raw `body` or flat top-level body shortcuts."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/da/commitments/prove".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw DA commitment proof request payload."
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

fn iroha_da_commitments_verify_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.da.commitments.verify".to_owned(),
        description:
            "Verify DA commitment payload (`/v1/da/commitments/verify`); accepts raw `body` or flat top-level body shortcuts."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/da/commitments/verify".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw DA commitment verification request payload."
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

fn iroha_da_pin_intents_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.da.pin_intents.list".to_owned(),
        description:
            "List DA pin intents (`/v1/da/pin_intents`); accepts raw `body` or flat top-level body shortcuts."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/da/pin_intents".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw DA pin-intents listing request payload."
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

fn iroha_da_pin_intents_prove_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.da.pin_intents.prove".to_owned(),
        description:
            "Fetch DA pin intent proof data (`/v1/da/pin_intents/prove`); accepts raw `body` or flat top-level body shortcuts."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/da/pin_intents/prove".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw DA pin-intents prove request payload."
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

fn iroha_da_pin_intents_verify_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.da.pin_intents.verify".to_owned(),
        description:
            "Verify DA pin intent proof payload (`/v1/da/pin_intents/verify`); accepts raw `body` or flat top-level body shortcuts."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/da/pin_intents/verify".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw DA pin-intents verification request payload."
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

fn iroha_runtime_abi_active_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.runtime.abi.active".to_owned(),
        description: "Fetch the active runtime ABI version (`/v1/runtime/abi/active`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/runtime/abi/active".to_owned(),
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

fn iroha_runtime_abi_hash_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.runtime.abi.hash".to_owned(),
        description: "Fetch active runtime ABI hash (`/v1/runtime/abi/hash`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/runtime/abi/hash".to_owned(),
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

fn iroha_runtime_metrics_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.runtime.metrics".to_owned(),
        description: "Fetch runtime metrics (`/v1/runtime/metrics`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/runtime/metrics".to_owned(),
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

fn iroha_runtime_upgrades_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.runtime.upgrades.list".to_owned(),
        description: "List runtime upgrades (`/v1/runtime/upgrades`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/runtime/upgrades".to_owned(),
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

fn iroha_runtime_upgrades_propose_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.runtime.upgrades.propose".to_owned(),
        description:
            "Propose a runtime upgrade (`/v1/runtime/upgrades/propose`); accepts raw `body` or flat top-level body shortcuts."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/runtime/upgrades/propose".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw runtime-upgrade proposal payload."
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

fn iroha_runtime_upgrades_activate_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.runtime.upgrades.activate".to_owned(),
        description: "Activate a runtime upgrade (`/v1/runtime/upgrades/activate/{id}`; `id`/`upgrade_id` shortcuts supported).".to_owned(),
        method: Method::POST,
        path_template: "/v1/runtime/upgrades/activate/{id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.id`."
                },
                "upgrade_id": {
                    "type": "string",
                    "description": "Alias for `id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id"],
                    "properties": {
                        "id": { "type": "string" }
                    }
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Optional activation payload. If omitted, flat top-level fields (except id/path/headers/accept) are forwarded."
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

fn iroha_runtime_upgrades_cancel_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.runtime.upgrades.cancel".to_owned(),
        description: "Cancel a runtime upgrade (`/v1/runtime/upgrades/cancel/{id}`; `id`/`upgrade_id` shortcuts supported).".to_owned(),
        method: Method::POST,
        path_template: "/v1/runtime/upgrades/cancel/{id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.id`."
                },
                "upgrade_id": {
                    "type": "string",
                    "description": "Alias for `id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id"],
                    "properties": {
                        "id": { "type": "string" }
                    }
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Optional cancellation payload. If omitted, flat top-level fields (except id/path/headers/accept) are forwarded."
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

fn iroha_ledger_headers_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.ledger.headers".to_owned(),
        description:
            "Fetch recent block headers (`/v1/ledger/headers`) with optional `from`/`limit` query shortcuts."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/ledger/headers".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "query": {
                    "type": "object",
                    "additionalProperties": true
                },
                "from": { "type": "integer" },
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

fn iroha_ledger_state_root_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.ledger.state_root".to_owned(),
        description: "Fetch execution state root by height (`/v1/ledger/state/{height}`; `height`/`block_height` shortcuts supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/ledger/state/{height}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "height": {
                    "type": "integer",
                    "description": "Convenience shortcut for `path.height`."
                },
                "block_height": {
                    "type": "integer",
                    "description": "Alias for `height`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["height"],
                    "properties": {
                        "height": { "type": "integer" }
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

fn iroha_ledger_state_proof_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.ledger.state_proof".to_owned(),
        description: "Fetch execution state proof (QC) by height (`/v1/ledger/state-proof/{height}`; `height`/`block_height` shortcuts supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/ledger/state-proof/{height}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "height": {
                    "type": "integer",
                    "description": "Convenience shortcut for `path.height`."
                },
                "block_height": {
                    "type": "integer",
                    "description": "Alias for `height`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["height"],
                    "properties": {
                        "height": { "type": "integer" }
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

fn iroha_ledger_block_proof_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.ledger.block_proof".to_owned(),
        description: "Fetch block-entry Merkle proofs (`/v1/ledger/block/{height}/proof/{entry_hash}`; `height`/`block_height` and `entry_hash`/`tx_hash`/`hash` shortcuts supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/ledger/block/{height}/proof/{entry_hash}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "height": {
                    "type": "integer",
                    "description": "Convenience shortcut for `path.height`."
                },
                "block_height": {
                    "type": "integer",
                    "description": "Alias for `height`."
                },
                "entry_hash": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.entry_hash`."
                },
                "tx_hash": {
                    "type": "string",
                    "description": "Alias for `entry_hash`."
                },
                "hash": {
                    "type": "string",
                    "description": "Alias for `entry_hash`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["height", "entry_hash"],
                    "properties": {
                        "height": { "type": "integer" },
                        "entry_hash": { "type": "string" }
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

fn iroha_bridge_finality_proof_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.bridge.finality.proof".to_owned(),
        description: "Fetch bridge finality proof by height (`/v1/bridge/finality/{height}`; `height`/`block_height` shortcuts supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/bridge/finality/{height}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "height": {
                    "type": "integer",
                    "description": "Convenience shortcut for `path.height`."
                },
                "block_height": {
                    "type": "integer",
                    "description": "Alias for `height`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["height"],
                    "properties": {
                        "height": { "type": "integer" }
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

fn iroha_bridge_finality_bundle_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.bridge.finality.bundle".to_owned(),
        description: "Fetch bridge finality commitment+justification bundle by height (`/v1/bridge/finality/bundle/{height}`; `height`/`block_height` shortcuts supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/bridge/finality/bundle/{height}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "height": {
                    "type": "integer",
                    "description": "Convenience shortcut for `path.height`."
                },
                "block_height": {
                    "type": "integer",
                    "description": "Alias for `height`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["height"],
                    "properties": {
                        "height": { "type": "integer" }
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

fn iroha_proofs_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.proofs.get".to_owned(),
        description:
            "Fetch a proof record by id (`/v1/proofs/{id}`; `id`/`proof_id` shortcuts supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/proofs/{id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.id`."
                },
                "proof_id": {
                    "type": "string",
                    "description": "Alias for `id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id"],
                    "properties": {
                        "id": { "type": "string" }
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

fn iroha_proofs_query_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.proofs.query".to_owned(),
        description:
            "Query proof records (`/v1/proofs/query`); accepts raw `body` or flat top-level body shortcuts."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/proofs/query".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw proof query payload."
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

fn iroha_proofs_retention_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.proofs.retention".to_owned(),
        description: "Fetch proof retention status (`/v1/proofs/retention`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/proofs/retention".to_owned(),
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

fn iroha_gov_post_tool(name: &str, description: &str, path_template: &str) -> ToolSpec {
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
                    "description": "Raw governance request payload. If omitted, flat top-level fields are forwarded as the request body."
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

fn iroha_gov_instances_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.gov.instances.list".to_owned(),
        description:
            "List governance instances by namespace (`/v1/gov/instances/{ns}`; `ns`/`namespace` shortcuts supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/gov/instances/{ns}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "ns": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.ns`."
                },
                "namespace": {
                    "type": "string",
                    "description": "Alias for `ns`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["ns"],
                    "properties": {
                        "ns": { "type": "string" }
                    }
                },
                "contains": { "type": "string" },
                "hash_prefix": { "type": "string" },
                "offset": { "type": "integer" },
                "limit": { "type": "integer" },
                "order": { "type": "string" },
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

fn iroha_gov_proposals_deploy_contract_tool() -> ToolSpec {
    iroha_gov_post_tool(
        "iroha.gov.proposals.deploy_contract",
        "Propose contract deployment (`/v1/gov/proposals/deploy-contract`); accepts raw `body` or flat top-level body shortcuts.",
        "/v1/gov/proposals/deploy-contract",
    )
}

fn iroha_gov_proposals_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.gov.proposals.get".to_owned(),
        description:
            "Fetch governance proposal detail (`/v1/gov/proposals/{id}`; `id`/`proposal_id` shortcuts supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/gov/proposals/{id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.id`."
                },
                "proposal_id": {
                    "type": "string",
                    "description": "Alias for `id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id"],
                    "properties": {
                        "id": { "type": "string" }
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

fn iroha_gov_locks_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.gov.locks.get".to_owned(),
        description:
            "Fetch governance lock records (`/v1/gov/locks/{rid}`; `rid`/`referendum_id` shortcuts supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/gov/locks/{rid}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "rid": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.rid`."
                },
                "id": {
                    "type": "string",
                    "description": "Alias for `rid`."
                },
                "referendum_id": {
                    "type": "string",
                    "description": "Alias for `rid`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["rid"],
                    "properties": {
                        "rid": { "type": "string" }
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

fn iroha_gov_referenda_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.gov.referenda.get".to_owned(),
        description:
            "Fetch governance referendum detail (`/v1/gov/referenda/{id}`; `id`/`referendum_id` shortcuts supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/gov/referenda/{id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.id`."
                },
                "referendum_id": {
                    "type": "string",
                    "description": "Alias for `id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id"],
                    "properties": {
                        "id": { "type": "string" }
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

fn iroha_gov_tally_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.gov.tally.get".to_owned(),
        description:
            "Fetch governance tally detail (`/v1/gov/tally/{id}`; `id`/`tally_id` shortcuts supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/gov/tally/{id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.id`."
                },
                "tally_id": {
                    "type": "string",
                    "description": "Alias for `id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id"],
                    "properties": {
                        "id": { "type": "string" }
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

fn iroha_gov_ballots_zk_tool() -> ToolSpec {
    iroha_gov_post_tool(
        "iroha.gov.ballots.zk",
        "Submit a governance ZK ballot (`/v1/gov/ballots/zk`); accepts raw `body` or flat top-level body shortcuts.",
        "/v1/gov/ballots/zk",
    )
}

fn iroha_gov_ballots_zk_v1_tool() -> ToolSpec {
    iroha_gov_post_tool(
        "iroha.gov.ballots.zk_v1",
        "Submit a governance ZK v1 ballot (`/v1/gov/ballots/zk-v1`); accepts raw `body` or flat top-level body shortcuts.",
        "/v1/gov/ballots/zk-v1",
    )
}

fn iroha_gov_ballots_zk_v1_ballot_proof_tool() -> ToolSpec {
    iroha_gov_post_tool(
        "iroha.gov.ballots.zk_v1.ballot_proof",
        "Submit a governance ZK ballot proof bundle (`/v1/gov/ballots/zk-v1/ballot-proof`); accepts raw `body` or flat top-level body shortcuts.",
        "/v1/gov/ballots/zk-v1/ballot-proof",
    )
}

fn iroha_gov_ballots_plain_tool() -> ToolSpec {
    iroha_gov_post_tool(
        "iroha.gov.ballots.plain",
        "Submit a governance plain ballot (`/v1/gov/ballots/plain`); accepts raw `body` or flat top-level body shortcuts.",
        "/v1/gov/ballots/plain",
    )
}

fn iroha_gov_protected_namespaces_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.gov.protected_namespaces.list".to_owned(),
        description: "List protected governance namespaces (`/v1/gov/protected-namespaces`)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/gov/protected-namespaces".to_owned(),
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

fn iroha_gov_protected_namespaces_update_tool() -> ToolSpec {
    iroha_gov_post_tool(
        "iroha.gov.protected_namespaces.update",
        "Update protected governance namespaces (`/v1/gov/protected-namespaces`); accepts raw `body` or flat top-level body shortcuts.",
        "/v1/gov/protected-namespaces",
    )
}

fn iroha_gov_unlocks_stats_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.gov.unlocks.stats".to_owned(),
        description: "Fetch governance unlock statistics (`/v1/gov/unlocks/stats`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/gov/unlocks/stats".to_owned(),
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

fn iroha_gov_council_current_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.gov.council.current".to_owned(),
        description: "Fetch current governance council set (`/v1/gov/council/current`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/gov/council/current".to_owned(),
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

fn iroha_gov_council_persist_tool() -> ToolSpec {
    iroha_gov_post_tool(
        "iroha.gov.council.persist",
        "Persist a governance council roster (`/v1/gov/council/persist`); accepts raw `body` or flat top-level body shortcuts.",
        "/v1/gov/council/persist",
    )
}

fn iroha_gov_council_replace_tool() -> ToolSpec {
    iroha_gov_post_tool(
        "iroha.gov.council.replace",
        "Replace a governance council member (`/v1/gov/council/replace`); accepts raw `body` or flat top-level body shortcuts.",
        "/v1/gov/council/replace",
    )
}

fn iroha_gov_council_audit_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.gov.council.audit".to_owned(),
        description: "Fetch governance council derivation audit data (`/v1/gov/council/audit`)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/gov/council/audit".to_owned(),
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

fn iroha_gov_council_derive_vrf_tool() -> ToolSpec {
    iroha_gov_post_tool(
        "iroha.gov.council.derive_vrf",
        "Derive governance council VRF inputs (`/v1/gov/council/derive-vrf`); accepts raw `body` or flat top-level body shortcuts.",
        "/v1/gov/council/derive-vrf",
    )
}

fn iroha_gov_enact_tool() -> ToolSpec {
    iroha_gov_post_tool(
        "iroha.gov.enact",
        "Enact governance proposal effects (`/v1/gov/enact`); accepts raw `body` or flat top-level body shortcuts.",
        "/v1/gov/enact",
    )
}

fn iroha_gov_finalize_tool() -> ToolSpec {
    iroha_gov_post_tool(
        "iroha.gov.finalize",
        "Finalize governance tally (`/v1/gov/finalize`); accepts raw `body` or flat top-level body shortcuts.",
        "/v1/gov/finalize",
    )
}

fn iroha_aliases_resolve_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.aliases.resolve".to_owned(),
        description: "Resolve an alias to its account binding (`/v1/aliases/resolve`).".to_owned(),
        method: Method::POST,
        path_template: "/v1/aliases/resolve".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw alias resolve payload. If omitted, flat top-level fields are forwarded as body."
                },
                "alias": {
                    "type": "string",
                    "description": "Convenience shortcut for `body.alias`."
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

fn iroha_aliases_resolve_index_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.aliases.resolve_index".to_owned(),
        description: "Resolve an alias index to its account binding (`/v1/aliases/resolve_index`)."
            .to_owned(),
        method: Method::POST,
        path_template: "/v1/aliases/resolve_index".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw alias-index resolve payload. If omitted, flat top-level fields are forwarded as body."
                },
                "index": {
                    "type": "integer",
                    "description": "Convenience shortcut for `body.index`."
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

fn iroha_aliases_by_account_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.aliases.by_account".to_owned(),
        description: "List aliases bound to an account (`/v1/aliases/by_account`).".to_owned(),
        method: Method::POST,
        path_template: "/v1/aliases/by_account".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw alias-by-account payload. If omitted, flat top-level fields are forwarded as body."
                },
                "account_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `body.account_id`."
                },
                "dataspace": {
                    "type": "string",
                    "description": "Optional convenience shortcut for `body.dataspace`."
                },
                "domain": {
                    "type": "string",
                    "description": "Optional convenience shortcut for `body.domain`."
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

fn iroha_contracts_code_bytes_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.contracts.code.bytes.get".to_owned(),
        description:
            "Fetch contract code bytes (`/v1/contracts/code-bytes/{code_hash}`; `code_hash` shortcut supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/contracts/code-bytes/{code_hash}".to_owned(),
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

fn iroha_contracts_instances_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.contracts.instances.list".to_owned(),
        description:
            "List contract instances by namespace (`/v1/contracts/instances/{ns}`; `ns`/`namespace` shortcut supported)."
                .to_owned(),
        method: Method::GET,
        path_template: "/v1/contracts/instances/{ns}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "ns": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.ns`."
                },
                "namespace": {
                    "type": "string",
                    "description": "Alias for `ns`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["ns"],
                    "properties": {
                        "ns": { "type": "string" }
                    }
                },
                "contains": { "type": "string" },
                "hash_prefix": { "type": "string" },
                "offset": { "type": "integer" },
                "limit": { "type": "integer" },
                "order": { "type": "string" },
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

fn iroha_accounts_faucet_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.accounts.faucet".to_owned(),
        description:
            "Request starter testnet funds for an existing account (`account_id` shortcut supported when `body` is omitted)."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/accounts/faucet".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "account_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `body.account_id`."
                },
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw faucet request body. If provided, it takes precedence over shortcuts."
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

fn iroha_nfts_chain_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.nfts.chain.list".to_owned(),
        description: "List NFTs from chain state (`/v1/nfts`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/nfts".to_owned(),
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
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_rwas_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.rwas.list".to_owned(),
        description: "List explorer RWA lots with optional flat query filters.".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/rwas".to_owned(),
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

fn iroha_rwas_chain_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.rwas.chain.list".to_owned(),
        description: "List RWA lots from chain state (`/v1/rwas`).".to_owned(),
        method: Method::GET,
        path_template: "/v1/rwas".to_owned(),
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

fn iroha_rwas_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.rwas.get".to_owned(),
        description: "Fetch explorer RWA detail (`rwa_id` shortcut supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/explorer/rwas/{rwa_id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "rwa_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.rwa_id`."
                },
                "id": {
                    "type": "string",
                    "description": "Alias for `rwa_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["rwa_id"],
                    "properties": {
                        "rwa_id": { "type": "string" }
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

fn iroha_rwas_query_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.rwas.query".to_owned(),
        description:
            "Query RWA lots with filter/select/sort/pagination envelope (flat shortcuts supported)."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/rwas/query".to_owned(),
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
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_offline_transfers_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.offline.transfers.list".to_owned(),
        description: "List offline transfer bundles with optional flat query filters.".to_owned(),
        method: Method::GET,
        path_template: "/v1/offline/transfers".to_owned(),
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
                "filter": { "type": "string" },
                "sort": { "type": "string" },
                "kind": { "type": "string" },
                "status": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_offline_transfers_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.offline.transfers.get".to_owned(),
        description: "Fetch one offline transfer bundle (`bundle_id_hex` shortcut supported)."
            .to_owned(),
        method: Method::GET,
        path_template: "/v1/offline/transfers/{bundle_id_hex}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "bundle_id_hex": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.bundle_id_hex`."
                },
                "bundle_id": {
                    "type": "string",
                    "description": "Alias for `bundle_id_hex`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["bundle_id_hex"],
                    "properties": {
                        "bundle_id_hex": { "type": "string" }
                    }
                },
                "query": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Optional query parameters."
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

fn iroha_offline_transfers_query_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.offline.transfers.query".to_owned(),
        description: "Query offline transfer bundles via QueryEnvelope (flat shortcuts supported)."
            .to_owned(),
        method: Method::POST,
        path_template: "/v1/offline/transfers/query".to_owned(),
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
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_offline_cash_setup_tool() -> ToolSpec {
    iroha_offline_cash_post_tool(
        "iroha.offline.cash.setup",
        "Create or fetch a device-bound offline cash lineage.",
        "/v1/offline/cash/setup",
    )
}

fn iroha_offline_cash_load_tool() -> ToolSpec {
    iroha_offline_cash_post_tool(
        "iroha.offline.cash.load",
        "Load device-bound offline cash.",
        "/v1/offline/cash/load",
    )
}

fn iroha_offline_cash_refresh_tool() -> ToolSpec {
    iroha_offline_cash_post_tool(
        "iroha.offline.cash.refresh",
        "Refresh the spend authorization for offline cash.",
        "/v1/offline/cash/refresh",
    )
}

fn iroha_offline_cash_sync_tool() -> ToolSpec {
    iroha_offline_cash_post_tool(
        "iroha.offline.cash.sync",
        "Sync pending offline cash receipts.",
        "/v1/offline/cash/sync",
    )
}

fn iroha_offline_cash_redeem_tool() -> ToolSpec {
    iroha_offline_cash_post_tool(
        "iroha.offline.cash.redeem",
        "Redeem device-bound offline cash back to the online wallet.",
        "/v1/offline/cash/redeem",
    )
}

fn iroha_offline_cash_post_tool(name: &str, description: &str, path_template: &str) -> ToolSpec {
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
                    "description": "Raw offline cash request payload. If omitted, flat top-level fields are forwarded as body."
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

fn iroha_offline_revocations_list_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.offline.revocations.list".to_owned(),
        description: "List offline verdict revocations for inspection.".to_owned(),
        method: Method::GET,
        path_template: "/v1/offline/revocations".to_owned(),
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
                "filter": { "type": "string" },
                "sort": { "type": "string" },
                "headers": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                },
                "accept": { "type": "string" }
            }
        }),
    }
}

fn iroha_offline_revocations_bundle_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.offline.revocations.bundle".to_owned(),
        description: "Fetch the signed offline revocation bundle for wallets.".to_owned(),
        method: Method::GET,
        path_template: "/v1/offline/revocations/bundle".to_owned(),
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

fn iroha_offline_revocations_register_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.offline.revocations.register".to_owned(),
        description: "Register an offline verdict revocation.".to_owned(),
        method: Method::POST,
        path_template: "/v1/offline/revocations".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {
                "body": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Raw revocation registration payload. If omitted, flat top-level fields are forwarded as body."
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

fn iroha_iso20022_pacs008_submit_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.iso20022.pacs008.submit".to_owned(),
        description:
            "Submit an ISO 20022 pacs.008 payload (`message_xml`/`xml` shortcuts supported)."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/iso20022/pacs008".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body_base64": {
                    "type": "string",
                    "description": "Base64/base64url encoded pacs.008 XML payload bytes."
                },
                "message_xml": {
                    "type": "string",
                    "description": "Raw pacs.008 XML payload shortcut (encoded to bytes internally)."
                },
                "xml": {
                    "type": "string",
                    "description": "Alias for `message_xml`."
                },
                "body": {
                    "description": "Optional raw request body payload."
                },
                "content_type": {
                    "type": "string",
                    "description": "Optional content type override (defaults to application/xml when `message_xml`/`xml` is used)."
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

fn iroha_iso20022_pacs009_submit_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.iso20022.pacs009.submit".to_owned(),
        description:
            "Submit an ISO 20022 pacs.009 payload (`message_xml`/`xml` shortcuts supported)."
                .to_owned(),
        method: Method::POST,
        path_template: "/v1/iso20022/pacs009".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body_base64": {
                    "type": "string",
                    "description": "Base64/base64url encoded pacs.009 XML payload bytes."
                },
                "message_xml": {
                    "type": "string",
                    "description": "Raw pacs.009 XML payload shortcut (encoded to bytes internally)."
                },
                "xml": {
                    "type": "string",
                    "description": "Alias for `message_xml`."
                },
                "body": {
                    "description": "Optional raw request body payload."
                },
                "content_type": {
                    "type": "string",
                    "description": "Optional content type override (defaults to application/xml when `message_xml`/`xml` is used)."
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

fn iroha_iso20022_status_get_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.iso20022.status.get".to_owned(),
        description: "Fetch ISO 20022 bridge status by message id (`msg_id`/`message_id` shortcuts supported).".to_owned(),
        method: Method::GET,
        path_template: "/v1/iso20022/status/{msg_id}".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "msg_id": {
                    "type": "string",
                    "description": "Convenience shortcut for `path.msg_id`."
                },
                "message_id": {
                    "type": "string",
                    "description": "Alias for `msg_id`."
                },
                "id": {
                    "type": "string",
                    "description": "Alias for `msg_id`."
                },
                "path": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["msg_id"],
                    "properties": {
                        "msg_id": { "type": "string" }
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

fn iroha_queries_submit_tool() -> ToolSpec {
    ToolSpec {
        name: "iroha.queries.submit".to_owned(),
        description: "Submit a signed query encoded as Norito bytes (`signed_query_base64`/`query_base64`/hex shortcuts supported).".to_owned(),
        method: Method::POST,
        path_template: "/query".to_owned(),
        input_schema: norito::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body_base64": {
                    "type": "string",
                    "description": "Base64/base64url encoded SignedQuery bytes."
                },
                "signed_query_base64": {
                    "type": "string",
                    "description": "Alias for `body_base64`."
                },
                "query_base64": {
                    "type": "string",
                    "description": "Alias for `body_base64`."
                },
                "body_hex": {
                    "type": "string",
                    "description": "Hex-encoded SignedQuery bytes."
                },
                "signed_query_hex": {
                    "type": "string",
                    "description": "Alias for `body_hex`."
                },
                "query_hex": {
                    "type": "string",
                    "description": "Alias for `body_hex`."
                },
                "body": {
                    "description": "Optional JSON request body; use only when submitting JSON query envelopes."
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
                    "anyOf": [
                        { "required": ["hash"] },
                        { "required": ["transaction_hash"] }
                    ],
                    "properties": {
                        "hash": { "type": "string" },
                        "transaction_hash": {
                            "type": "string",
                            "description": "Alias for `path.hash`."
                        }
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
                    "anyOf": [
                        { "required": ["hash", "index"] },
                        { "required": ["transaction_hash", "index"] }
                    ],
                    "properties": {
                        "hash": { "type": "string" },
                        "transaction_hash": {
                            "type": "string",
                            "description": "Alias for `path.hash`."
                        },
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
                    "properties": {
                        "hash": { "type": "string" },
                        "transaction_hash": {
                            "type": "string",
                            "description": "Alias for `query.hash`."
                        }
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
            "Get latest pipeline status for a submitted transaction hash (`hash`/`transaction_hash` shortcuts supported)."
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
                "transaction_hash": {
                    "type": "string",
                    "description": "Alias for `hash`."
                },
                "query": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "hash": { "type": "string" },
                        "transaction_hash": {
                            "type": "string",
                            "description": "Alias for `query.hash`."
                        }
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
    use crate::tests_runtime_handlers::mk_app_state_for_tests;
    use iroha_config::parameters::actual::ToriiMcpProfile;

    static MCP_ASYNC_JOBS_TEST_LOCK: LazyLock<std::sync::Mutex<()>> =
        LazyLock::new(|| std::sync::Mutex::new(()));

    const TEST_ACCOUNT_I105: &str =
        "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE";
    const TEST_ASSET_ID: &str = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";

    fn sample_tool(name: &str, method: Method) -> ToolSpec {
        ToolSpec {
            name: name.to_owned(),
            description: "sample".to_owned(),
            method,
            path_template: "/v1/sample".to_owned(),
            input_schema: norito::json!({ "type": "object" }),
        }
    }

    fn remote_addr_probe_payload(
        headers: &HeaderMap,
        remote: SocketAddr,
        allow: &[crate::limits::IpNet],
    ) -> Value {
        let header_remote = headers
            .get(crate::limits::REMOTE_ADDR_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .map(Value::String)
            .unwrap_or(Value::Null);
        let mut payload = Map::new();
        payload.insert(
            "allowed_header_only".into(),
            Value::Bool(crate::limits::is_allowed_by_cidr(headers, None, allow)),
        );
        payload.insert(
            "allowed_with_remote".into(),
            Value::Bool(crate::limits::is_allowed_by_cidr(
                headers,
                Some(remote.ip()),
                allow,
            )),
        );
        payload.insert("remote".into(), Value::String(remote.ip().to_string()));
        payload.insert("header".into(), header_remote);
        Value::Object(payload)
    }

    fn install_remote_addr_probe_router(app: &mut SharedAppState) {
        let allow = vec![crate::limits::parse_cidr("127.0.0.0/8").expect("loopback cidr")];
        let router: axum::Router = axum::Router::new().route(
            "/v1/remote-probe",
            axum::routing::get_service(tower::service_fn(move |req: Request<Body>| {
                let allow = allow.clone();
                async move {
                    let headers = req.headers().clone();
                    let remote = req
                        .extensions()
                        .get::<axum::extract::ConnectInfo<SocketAddr>>()
                        .map(|connect| connect.0)
                        .unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));
                    let payload = remote_addr_probe_payload(&headers, remote, &allow);
                    let body = Body::from(
                        norito::json::to_string(&payload).expect("encode probe payload"),
                    );
                    Ok::<_, std::convert::Infallible>(
                        Response::builder()
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(body)
                            .expect("response"),
                    )
                }
            })),
        );

        let app = std::sync::Arc::get_mut(app).expect("unique app state");
        let mut guard = app
            .mcp_dispatch_router
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(router);
    }

    #[test]
    fn capabilities_payload_includes_toolset_version() {
        let tool = sample_tool("iroha.health", Method::GET);
        let refs = vec![&tool];
        let payload = capabilities_payload(&refs);
        let toolset_version = payload
            .get("capabilities")
            .and_then(|caps| caps.get("tools"))
            .and_then(|tools| tools.get("toolsetVersion"))
            .and_then(Value::as_str)
            .expect("toolsetVersion");
        assert!(
            !toolset_version.is_empty(),
            "toolsetVersion must not be empty"
        );
    }

    #[test]
    fn jsonrpc_error_response_adds_stable_error_code() {
        let payload = jsonrpc_error_response(None, JSONRPC_INVALID_PARAMS, "bad input", None);
        let code = payload
            .get("error")
            .and_then(|err| err.get("data"))
            .and_then(|data| data.get("error_code"))
            .and_then(Value::as_str)
            .expect("error_code");
        assert_eq!(code, "invalid_params");
    }

    #[test]
    fn read_only_policy_blocks_mutating_tools() {
        let mut cfg = iroha_config::parameters::actual::ToriiMcp::default();
        cfg.profile = ToriiMcpProfile::ReadOnly;
        let read_tool = sample_tool("iroha.accounts.get", Method::GET);
        let write_tool = sample_tool("iroha.transactions.submit", Method::POST);
        assert!(is_tool_allowed_by_policy(&cfg, &read_tool));
        assert!(!is_tool_allowed_by_policy(&cfg, &write_tool));
    }

    #[test]
    fn apply_body_projection_keeps_requested_fields() {
        let structured = norito::json!({
            "status": 200,
            "body": {
                "id": 1,
                "name": "alice",
                "extra": true
            }
        });
        let projection = norito::json!(["id", "name"]);
        let projected = apply_body_projection(structured, Some(&projection));
        let body = projected
            .get("body")
            .and_then(Value::as_object)
            .expect("projected body object");
        assert!(body.contains_key("id"));
        assert!(body.contains_key("name"));
        assert!(!body.contains_key("extra"));
    }

    #[test]
    fn apply_extra_headers_blocks_reserved_internal_headers() {
        let mut out = HeaderMap::new();
        let headers = norito::json!({
            "x-test": "1",
            "x-iroha-remote-addr": "127.0.0.1",
            "x-forwarded-client-cert": "present"
        });

        apply_extra_headers(&mut out, Some(&headers)).expect("headers accepted");

        assert_eq!(
            out.get("x-test").and_then(|value| value.to_str().ok()),
            Some("1")
        );
        assert!(!out.contains_key("x-iroha-remote-addr"));
        assert!(!out.contains_key("x-forwarded-client-cert"));
    }

    #[tokio::test]
    async fn tools_call_batch_returns_per_call_errors_for_unknown_tools() {
        let app = mk_app_state_for_tests();
        let params = norito::json!({
            "calls": [
                { "name": "torii.missing.one" },
                { "name": "torii.missing.two", "arguments": { "x": 1 } }
            ]
        });
        let response = handle_tools_call_batch(
            Some(Value::from(1_u64)),
            app,
            &HeaderMap::new(),
            params.as_object().expect("params object"),
        )
        .await;
        let results = response
            .get("result")
            .and_then(|value| value.get("results"))
            .and_then(Value::as_array)
            .expect("batch results");
        assert_eq!(results.len(), 2);
        for result in results {
            let code = result
                .get("error")
                .and_then(|error| error.get("data"))
                .and_then(|data| data.get("error_code"))
                .and_then(Value::as_str)
                .expect("error code");
            assert_eq!(code, MCP_TOOL_NOT_FOUND);
        }
    }

    #[tokio::test]
    async fn tools_call_async_job_can_be_fetched_after_completion() {
        let _guard = MCP_ASYNC_JOBS_TEST_LOCK.lock().expect("async job lock");
        MCP_ASYNC_JOBS.clear();

        let app = mk_app_state_for_tests();
        let params = norito::json!({
            "name": "torii.missing.async"
        });
        let response = handle_tools_call_async(
            Some(Value::from(2_u64)),
            app.clone(),
            &HeaderMap::new(),
            params.as_object().expect("params object"),
        )
        .await;
        let job_id = response
            .get("result")
            .and_then(|value| value.get("job_id"))
            .and_then(Value::as_str)
            .expect("job id")
            .to_owned();

        let mut final_state = None;
        for _ in 0..40 {
            let mut get_params = norito::json::Map::new();
            get_params.insert("job_id".into(), Value::String(job_id.clone()));
            let jobs = handle_tools_jobs_get(None, &app.mcp, &get_params);
            let state = jobs
                .get("result")
                .and_then(|value| value.get("state"))
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
                .unwrap_or("pending");
            if state != "pending" {
                final_state = Some(state.to_owned());
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(final_state.as_deref(), Some("failed"));

        let mut get_params = norito::json::Map::new();
        get_params.insert("job_id".into(), Value::String(job_id));
        let jobs = handle_tools_jobs_get(None, &app.mcp, &get_params);
        let error_code = jobs
            .get("result")
            .and_then(|value| value.get("state"))
            .and_then(|value| value.get("error"))
            .and_then(|value| value.get("data"))
            .and_then(|value| value.get("error_code"))
            .and_then(Value::as_str)
            .expect("error code");
        assert_eq!(error_code, MCP_TOOL_NOT_FOUND);
    }

    #[tokio::test]
    async fn tools_list_list_changed_tracks_toolset_version() {
        let app = mk_app_state_for_tests();
        let visible_tools = visible_tools_for_policy(&app.mcp, app.mcp_tools.as_slice());
        let version = compute_toolset_version(&visible_tools);

        let same_version = norito::json!({ "toolsetVersion": version });
        let same_response = handle_tools_list(None, &app, same_version.as_object().expect("map"));
        assert_eq!(
            same_response
                .get("result")
                .and_then(|value| value.get("listChanged"))
                .and_then(Value::as_bool),
            Some(false)
        );

        let different_version = norito::json!({ "toolset_version": "different" });
        let different_response =
            handle_tools_list(None, &app, different_version.as_object().expect("map"));
        assert_eq!(
            different_response
                .get("result")
                .and_then(|value| value.get("listChanged"))
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn prune_async_jobs_applies_ttl_and_capacity_limits() {
        let _guard = MCP_ASYNC_JOBS_TEST_LOCK.lock().expect("async job lock");
        MCP_ASYNC_JOBS.clear();

        let mut cfg = iroha_config::parameters::actual::ToriiMcp::default();
        cfg.async_job_ttl_secs = 1;
        cfg.async_job_max_entries = 2;

        let now = Instant::now();
        MCP_ASYNC_JOBS.insert(
            "old".to_owned(),
            AsyncJobRecord {
                state: norito::json!({ "status": "completed" }),
                updated_at: now - Duration::from_secs(3),
            },
        );
        MCP_ASYNC_JOBS.insert(
            "recent-1".to_owned(),
            AsyncJobRecord {
                state: norito::json!({ "status": "completed" }),
                updated_at: now - Duration::from_millis(30),
            },
        );
        MCP_ASYNC_JOBS.insert(
            "recent-2".to_owned(),
            AsyncJobRecord {
                state: norito::json!({ "status": "completed" }),
                updated_at: now - Duration::from_millis(20),
            },
        );
        MCP_ASYNC_JOBS.insert(
            "recent-3".to_owned(),
            AsyncJobRecord {
                state: norito::json!({ "status": "completed" }),
                updated_at: now - Duration::from_millis(10),
            },
        );

        prune_async_jobs(&cfg, now);

        assert!(!MCP_ASYNC_JOBS.contains_key("old"));
        assert!(!MCP_ASYNC_JOBS.contains_key("recent-1"));
        assert!(MCP_ASYNC_JOBS.contains_key("recent-2"));
        assert!(MCP_ASYNC_JOBS.contains_key("recent-3"));
    }

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
        assert!(tools.iter().any(|tool| tool.name == "iroha.time.now"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.time.status"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.api.versions"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.commit_certificates")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.validator_sets.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.validator_sets.get")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.sumeragi.rbc"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.pacemaker")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.phases")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.params")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.status")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.leader")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.sumeragi.qc"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.checkpoints")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.consensus_keys")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.bls_keys")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.key_lifecycle")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.telemetry")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.rbc.sessions")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.commit_qc.get")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.collectors")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.evidence.count")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.evidence.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.evidence.submit")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.new_view")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.rbc.delivered")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.vrf.penalties")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.vrf.epoch")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.vrf.commit")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.vrf.reveal")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.sumeragi.rbc.sample")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.da.ingest"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.da.proof_policies")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.da.proof_policy_snapshot")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.da.manifests.get")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.da.commitments.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.da.commitments.prove")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.da.commitments.verify")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.da.pin_intents.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.da.pin_intents.prove")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.da.pin_intents.verify")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.runtime.abi.active")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.runtime.abi.hash")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.runtime.metrics")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.runtime.upgrades.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.runtime.upgrades.propose")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.runtime.upgrades.activate")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.runtime.upgrades.cancel")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.ledger.headers"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.ledger.state_root")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.ledger.state_proof")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.ledger.block_proof")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.bridge.finality.proof")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.bridge.finality.bundle")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.proofs.get"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.proofs.query"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.proofs.retention")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.instances.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.proposals.deploy_contract")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.proposals.get")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.gov.locks.get"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.referenda.get")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.gov.tally.get"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.gov.ballots.zk"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.ballots.zk_v1")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.ballots.zk_v1.ballot_proof")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.ballots.plain")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.protected_namespaces.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.protected_namespaces.update")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.unlocks.stats")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.council.current")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.council.persist")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.council.replace")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.council.audit")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.gov.council.derive_vrf")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.gov.enact"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.gov.finalize"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.aliases.resolve")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.aliases.resolve_index")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.aliases.by_account")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.contracts.code.get")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.contracts.code.bytes.get")
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
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.contracts.instances.list")
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
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.nfts.chain.list")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.nfts.list"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.nfts.get"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.nfts.query"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.rwas.chain.list")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.rwas.list"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.rwas.get"));
        assert!(tools.iter().any(|tool| tool.name == "iroha.rwas.query"));
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.offline.transfers.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.offline.transfers.get")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.offline.transfers.query")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.offline.cash.setup")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.offline.cash.load")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.offline.cash.refresh")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.offline.cash.sync")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.offline.cash.redeem")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.offline.revocations.list")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.offline.revocations.bundle")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.offline.revocations.register")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.iso20022.pacs008.submit")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.iso20022.pacs009.submit")
        );
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "iroha.iso20022.status.get")
        );
        assert!(tools.iter().any(|tool| tool.name == "iroha.queries.submit"));
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

    #[tokio::test]
    async fn dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks() {
        let mut app = mk_app_state_for_tests();
        install_remote_addr_probe_router(&mut app);

        let mut inbound_headers = HeaderMap::new();
        inbound_headers.insert(
            HeaderName::from_static(crate::limits::REMOTE_ADDR_HEADER),
            HeaderValue::from_static("198.51.100.23"),
        );

        let result = dispatch_route(
            &app,
            &inbound_headers,
            Method::GET,
            "/v1/remote-probe",
            None,
            Vec::new(),
            None,
            None,
        )
        .await
        .expect("dispatch succeeds");

        assert_eq!(result.get("status").and_then(Value::as_u64), Some(200));
        let body = result
            .get("body")
            .and_then(Value::as_object)
            .expect("response body");
        assert_eq!(
            body.get("allowed_header_only").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            body.get("allowed_with_remote").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            body.get("remote").and_then(Value::as_str),
            Some("198.51.100.23")
        );
        assert_eq!(
            body.get("header").and_then(Value::as_str),
            Some("198.51.100.23")
        );
    }

    #[tokio::test]
    async fn dispatch_route_blocks_remote_addr_spoofing_from_extra_headers() {
        let mut app = mk_app_state_for_tests();
        install_remote_addr_probe_router(&mut app);

        let mut extra_headers = Map::new();
        extra_headers.insert(
            crate::limits::REMOTE_ADDR_HEADER.to_owned(),
            Value::String("127.0.0.1".to_owned()),
        );
        let extra_headers = Value::Object(extra_headers);

        let result = dispatch_route(
            &app,
            &HeaderMap::new(),
            Method::GET,
            "/v1/remote-probe",
            Some(&extra_headers),
            Vec::new(),
            None,
            None,
        )
        .await
        .expect("dispatch succeeds");

        assert_eq!(result.get("status").and_then(Value::as_u64), Some(200));
        let body = result
            .get("body")
            .and_then(Value::as_object)
            .expect("response body");
        assert_eq!(
            body.get("allowed_header_only").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            body.get("allowed_with_remote").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(body.get("remote").and_then(Value::as_str), Some("0.0.0.0"));
        assert!(body.get("header").is_some_and(Value::is_null));
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
    fn ws_ticket_accepts_session_id_alias() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("node.example"));
        let args = norito::json!({
            "session_id": "YWJj",
            "role": "app",
            "token": "app-token"
        });
        let ticket =
            build_connect_ws_ticket(args.as_object().expect("object"), &headers).expect("ticket");
        let ws_url = ticket
            .get("ws_url")
            .and_then(Value::as_str)
            .expect("ws url");
        assert!(ws_url.contains("sid=YWJj"));
    }

    #[test]
    fn build_connect_session_create_body_generates_sid_when_missing() {
        let args = norito::json!({
            "node": "https://node.example"
        });
        let body = build_connect_session_create_body(args.as_object().expect("object"))
            .expect("create body");
        let payload = body.as_object().expect("object");
        let sid = payload
            .get("sid")
            .and_then(Value::as_str)
            .expect("generated sid");
        assert_eq!(
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(sid)
                .expect("base64url sid")
                .len(),
            32
        );
        assert_eq!(
            payload.get("node").and_then(Value::as_str),
            Some("https://node.example")
        );
    }

    #[test]
    fn build_connect_session_create_body_preserves_sid_in_body() {
        let args = norito::json!({
            "sid": "ignored-shortcut",
            "body": {
                "sid": "body-sid",
                "node": "https://in-body.example"
            }
        });
        let body = build_connect_session_create_body(args.as_object().expect("object"))
            .expect("create body");
        let payload = body.as_object().expect("object");
        assert_eq!(payload.get("sid").and_then(Value::as_str), Some("body-sid"));
        assert_eq!(
            payload.get("node").and_then(Value::as_str),
            Some("https://in-body.example")
        );
    }

    #[test]
    fn build_connect_session_create_body_accepts_session_id_shortcut() {
        let args = norito::json!({
            "session_id": "shortcut-sid"
        });
        let body = build_connect_session_create_body(args.as_object().expect("object"))
            .expect("create body");
        let payload = body.as_object().expect("object");
        assert_eq!(
            payload.get("sid").and_then(Value::as_str),
            Some("shortcut-sid")
        );
    }

    #[test]
    fn extract_connect_sid_argument_accepts_path_session_id_alias() {
        let args = norito::json!({
            "path": {
                "session_id": "nested-sid"
            }
        });
        let sid =
            extract_connect_sid_argument(args.as_object().expect("object")).expect("sid alias");
        assert_eq!(sid, "nested-sid");
    }

    #[test]
    fn vpn_tool_factories_expose_expected_names_and_routes() {
        let profile = iroha_vpn_profile_tool();
        assert_eq!(profile.name, "iroha.vpn.profile");
        assert_eq!(profile.path_template, "/v1/vpn/profile");

        let create = iroha_vpn_sessions_create_tool();
        assert_eq!(create.name, "iroha.vpn.sessions.create");
        assert_eq!(create.path_template, "/v1/vpn/sessions");

        let get = iroha_vpn_sessions_get_tool();
        assert_eq!(get.name, "iroha.vpn.sessions.get");
        assert_eq!(get.path_template, "/v1/vpn/sessions/{session_id}");

        let delete = iroha_vpn_sessions_delete_tool();
        assert_eq!(delete.name, "iroha.vpn.sessions.delete");
        assert_eq!(delete.path_template, "/v1/vpn/sessions/{session_id}");

        let receipts = iroha_vpn_receipts_list_tool();
        assert_eq!(receipts.name, "iroha.vpn.receipts.list");
        assert_eq!(receipts.path_template, "/v1/vpn/receipts");
    }

    #[test]
    fn extract_vpn_session_id_argument_accepts_path_id_alias() {
        let args = norito::json!({
            "path": {
                "id": "nested-vpn-session"
            }
        });
        let session_id = extract_vpn_session_id_argument(args.as_object().expect("object"))
            .expect("vpn session id");
        assert_eq!(session_id, "nested-vpn-session");
    }

    #[test]
    fn extract_vpn_session_id_argument_accepts_top_level_id_alias() {
        let args = norito::json!({
            "id": "top-level-vpn-session"
        });
        let session_id = extract_vpn_session_id_argument(args.as_object().expect("object"))
            .expect("vpn session id");
        assert_eq!(session_id, "top-level-vpn-session");
    }

    #[test]
    fn collect_query_map_accepts_flat_query_fields_when_query_absent() {
        let args = norito::json!({
            "account_id": TEST_ACCOUNT_I105,
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
            "account_id": TEST_ACCOUNT_I105
        });
        let account_id =
            extract_account_id_argument(args.as_object().expect("object")).expect("account id");
        assert_eq!(account_id, TEST_ACCOUNT_I105);
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
    fn extract_iso20022_message_id_argument_accepts_alias_shortcut() {
        let args = norito::json!({
            "message_id": "msg-001"
        });
        let msg_id = extract_iso20022_message_id_argument(args.as_object().expect("object"))
            .expect("message id");
        assert_eq!(msg_id, "msg-001");
    }

    #[test]
    fn extract_ticket_argument_accepts_id_alias_shortcut() {
        let args = norito::json!({
            "id": "manifest-ticket-001"
        });
        let ticket = extract_ticket_argument(args.as_object().expect("object")).expect("ticket");
        assert_eq!(ticket, "manifest-ticket-001");
    }

    #[test]
    fn extract_proof_record_id_argument_accepts_proof_id_alias_shortcut() {
        let args = norito::json!({
            "proof_id": "proof-001"
        });
        let proof_id =
            extract_proof_record_id_argument(args.as_object().expect("object")).expect("id");
        assert_eq!(proof_id, "proof-001");
    }

    #[test]
    fn extract_governance_entity_id_argument_accepts_alias_shortcuts() {
        let proposal_args = norito::json!({
            "proposal_id": "proposal-001"
        });
        let proposal_id =
            extract_governance_entity_id_argument(proposal_args.as_object().expect("object"))
                .expect("proposal id");
        assert_eq!(proposal_id, "proposal-001");

        let referendum_args = norito::json!({
            "referendum_id": "referendum-001"
        });
        let referendum_id =
            extract_governance_entity_id_argument(referendum_args.as_object().expect("object"))
                .expect("referendum id");
        assert_eq!(referendum_id, "referendum-001");

        let tally_args = norito::json!({
            "tally_id": "tally-001"
        });
        let tally_id =
            extract_governance_entity_id_argument(tally_args.as_object().expect("object"))
                .expect("tally id");
        assert_eq!(tally_id, "tally-001");

        let lock_args = norito::json!({
            "rid": "referendum-002"
        });
        let lock_id = extract_governance_entity_id_argument(lock_args.as_object().expect("object"))
            .expect("lock id");
        assert_eq!(lock_id, "referendum-002");
    }

    #[test]
    fn extract_runtime_upgrade_id_argument_accepts_upgrade_id_alias_shortcut() {
        let args = norito::json!({
            "upgrade_id": "upgrade-001"
        });
        let upgrade_id =
            extract_runtime_upgrade_id_argument(args.as_object().expect("object")).expect("id");
        assert_eq!(upgrade_id, "upgrade-001");
    }

    #[test]
    fn extract_height_argument_accepts_block_height_alias_shortcut() {
        let args = norito::json!({
            "block_height": 7
        });
        let height = extract_height_argument(args.as_object().expect("object")).expect("height");
        assert_eq!(height, "7");
    }

    #[test]
    fn extract_view_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "view": 3
        });
        let view = extract_view_argument(args.as_object().expect("object")).expect("view");
        assert_eq!(view, "3");
    }

    #[test]
    fn extract_epoch_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "epoch": 11
        });
        let epoch = extract_epoch_argument(args.as_object().expect("object")).expect("epoch");
        assert_eq!(epoch, "11");
    }

    #[test]
    fn extract_entry_hash_argument_accepts_alias_shortcut() {
        let args = norito::json!({
            "tx_hash": "abc123"
        });
        let entry_hash =
            extract_entry_hash_argument(args.as_object().expect("object")).expect("entry hash");
        assert_eq!(entry_hash, "abc123");
    }

    #[test]
    fn build_iso20022_payload_body_accepts_xml_shortcut() {
        let args = norito::json!({
            "message_xml": "<Document>ok</Document>"
        });
        let (body, content_type) =
            build_iso20022_payload_body(args.as_object().expect("object")).expect("iso body");
        assert_eq!(body, b"<Document>ok</Document>".to_vec());
        assert_eq!(content_type.as_deref(), Some("application/xml"));
    }

    #[test]
    fn extract_definition_id_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        });
        let definition_id =
            extract_definition_id_argument(args.as_object().expect("object")).expect("definition");
        assert_eq!(definition_id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    }

    #[test]
    fn extract_asset_id_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "asset_id": TEST_ASSET_ID
        });
        let asset_id =
            extract_asset_id_argument(args.as_object().expect("object")).expect("asset id");
        assert_eq!(asset_id, TEST_ASSET_ID);
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
    fn extract_rwa_id_argument_accepts_top_level_shortcut() {
        let args = norito::json!({
            "rwa_id": "rwa-001"
        });
        let rwa_id = extract_rwa_id_argument(args.as_object().expect("object")).expect("rwa id");
        assert_eq!(rwa_id, "rwa-001");
    }

    #[test]
    fn extract_bundle_id_hex_argument_accepts_alias_shortcut() {
        let args = norito::json!({
            "bundle_id": "deadbeef"
        });
        let bundle_id =
            extract_bundle_id_hex_argument(args.as_object().expect("object")).expect("bundle id");
        assert_eq!(bundle_id, "deadbeef");
    }

    #[test]
    fn extract_certificate_id_hex_argument_accepts_id_alias_shortcut() {
        let args = norito::json!({
            "id": "cafe1234"
        });
        let certificate_id = extract_certificate_id_hex_argument(args.as_object().expect("object"))
            .expect("certificate id");
        assert_eq!(certificate_id, "cafe1234");
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
    fn extract_transaction_hash_argument_accepts_path_alias_shortcut() {
        let args = norito::json!({
            "path": {
                "transaction_hash": "deadbeef"
            }
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
    fn extract_optional_transaction_hash_argument_accepts_query_alias_shortcut() {
        let args = norito::json!({
            "query": {
                "transaction_hash": "deadbeef"
            }
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
    fn extract_contract_namespace_argument_accepts_namespace_alias_shortcut() {
        let args = norito::json!({
            "namespace": "payments"
        });
        let ns = extract_contract_namespace_argument(args.as_object().expect("object"))
            .expect("namespace");
        assert_eq!(ns, "payments");
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
            "filter": { "op": "eq", "args": ["authority", TEST_ACCOUNT_I105] },
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
            "account_id": TEST_ACCOUNT_I105,
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
    fn build_accounts_faucet_body_collects_shortcut_field() {
        let args = norito::json!({
            "account_id": TEST_ACCOUNT_I105
        });
        let body = build_accounts_faucet_body(args.as_object().expect("object")).expect("body");
        let body = body.as_object().expect("object");
        assert_eq!(
            body.get("account_id").and_then(Value::as_str),
            Some(TEST_ACCOUNT_I105)
        );
    }

    #[test]
    fn build_accounts_faucet_body_rejects_missing_account_id() {
        let args = norito::json!({
            "headers": { "x-test": "1" }
        });
        let err = build_accounts_faucet_body(args.as_object().expect("object")).expect_err("error");
        assert!(err.contains("`account_id` is required"));
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
            "authority": TEST_ACCOUNT_I105,
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
            Some(TEST_ACCOUNT_I105)
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
