---
id: torii-mcp
title: Torii MCP API
description: Reference guide for using Torii's native Model Context Protocol bridge.
---

Torii exposes a native Model Context Protocol (MCP) bridge at `/v2/mcp`.
This endpoint lets agents discover tools and invoke Torii/Connect routes through JSON-RPC.

## Endpoint shape

- `GET /v2/mcp` returns capabilities metadata (not JSON-RPC wrapped).
- `POST /v2/mcp` accepts JSON-RPC 2.0 requests.
- If `torii.mcp.enabled = false`, neither route is exposed.
- If `torii.require_api_token` is enabled, missing/invalid token is rejected before JSON-RPC dispatch.

## Configuration

Enable MCP under `torii.mcp`:

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "profile": "read_only",
      "expose_operator_routes": false,
      "allow_tool_prefixes": [],
      "deny_tool_prefixes": [],
      "rate_per_minute": 240,
      "burst": 120,
      "async_job_ttl_secs": 300,
      "async_job_max_entries": 2000
    }
  }
}
```

Key behavior:

- `profile` controls tool visibility (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` apply additional name-based policy.
- `rate_per_minute`/`burst` apply token-bucket limiting for MCP requests.
- Async job state from `tools/call_async` is retained in-memory using `async_job_ttl_secs` and `async_job_max_entries`.

## Recommended client flow

1. Call `initialize`.
2. Call `tools/list` and cache `toolsetVersion`.
3. Use `tools/call` for normal operations.
4. Use `tools/call_async` + `tools/jobs/get` for longer operations.
5. Re-run `tools/list` when `listChanged` is `true`.

Do not hardcode the full tool catalog. Discover at runtime.

## Methods and semantics

Supported JSON-RPC methods:

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

Notes:

- `tools/list` accepts both `toolset_version` and `toolsetVersion`.
- `tools/jobs/get` accepts both `job_id` and `jobId`.
- `tools/list.cursor` is a numeric string offset; invalid values fall back to `0`.
- `tools/call_batch` is best-effort per item (one failed call does not fail sibling calls).
- `tools/call_async` validates only envelope shape immediately; execution errors appear later in job state.
- `jsonrpc` should be `"2.0"`; omitted `jsonrpc` is accepted for compatibility.

## Auth and forwarding

MCP dispatch does not bypass Torii authorization. Calls execute normal route handlers and auth checks.

Torii forwards inbound auth-related headers for tool dispatch:

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

Clients can also supply additional per-call headers via `arguments.headers`.
`content-length`, `host`, and `connection` from `arguments.headers` are ignored.

## Error model

HTTP layer:

- `400` invalid JSON
- `403` API token rejected before JSON-RPC handling
- `413` payload exceeds `max_request_bytes`
- `429` rate-limited
- `200` for JSON-RPC responses (including JSON-RPC errors)

JSON-RPC layer:

- Top-level `error.data.error_code` is stable (for example `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, `rate_limited`).
- Tool failures surface as MCP tool results with `isError = true` and structured details.
- Route-dispatched tool failures map HTTP status to `structuredContent.error_code` (for example `forbidden`, `not_found`, `server_error`).

## Tool naming

OpenAPI-derived tools use stable route-based names:

- `torii.<method>_<path...>`
- Example: `torii.get_v1_accounts`

Curated aliases are also exposed under `iroha.*` and `connect.*`.

## Canonical specification

The full wire-level contract is maintained in:

- `crates/iroha_torii/docs/mcp_api.md`

When behavior changes in `crates/iroha_torii/src/mcp.rs` or `crates/iroha_torii/src/lib.rs`,
update that spec in the same change and then mirror key usage guidance here.
