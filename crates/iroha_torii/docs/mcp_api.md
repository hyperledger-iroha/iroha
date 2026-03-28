# Torii MCP API

Torii exposes a native Model Context Protocol bridge at `/v1/mcp`.
It lets MCP clients discover tools and call Torii/Connect endpoints through JSON-RPC.

## Enable And Configure
MCP is disabled by default. Enable it under `torii.mcp`.

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

### Public Deployed-Network Profile

For public SORA/Torii deployments intended to be used from Codex, the
recommended policy is a curated writer profile that exposes only the stable
`iroha.*` aliases:

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "profile": "writer",
      "expose_operator_routes": false,
      "allow_tool_prefixes": ["iroha."]
    }
  }
}
```

This keeps the public tool catalog small and task-oriented while hiding the
full raw `torii.*` OpenAPI-derived namespace and all operator routes.

### Configuration Fields
- `enabled`: master switch for `/v1/mcp`.
- `max_request_bytes`: POST body limit for MCP JSON-RPC.
- `max_tools_per_list`: pagination size for `tools/list`.
- `profile`: `read_only`, `writer`, or `operator`.
- `expose_operator_routes`: include operator routes even when profile is not `operator`.
- `allow_tool_prefixes`: if non-empty, only matching tool-name prefixes are allowed.
- `deny_tool_prefixes`: blocked tool-name prefixes (applied before allow-list).
- `rate_per_minute` / `burst`: MCP endpoint token-bucket limits.
- `async_job_ttl_secs` / `async_job_max_entries`: in-memory retention policy for `tools/call_async` jobs.

Profile behavior:
- `read_only`: GET/HEAD/OPTIONS and read-style aliases only.
- `writer`: includes mutating non-operator tools.
- `operator`: includes operator tools as well.

When `allow_tool_prefixes` is set, the profile still applies first and the
prefix allow-list is applied second. Public networks can therefore use
`profile = "writer"` together with `allow_tool_prefixes = ["iroha."]` to keep
mutating app-development helpers available without publishing the broader raw
surface.

## Endpoints
- `GET /v1/mcp`: capabilities payload (not JSON-RPC wrapped).
- `POST /v1/mcp`: JSON-RPC 2.0 execution endpoint.

If `torii.mcp.enabled` is `false`, these routes are not exposed.

## Security And Header Forwarding
MCP does not bypass Torii auth.
Tool dispatch executes the same handlers as regular HTTP routes, so normal endpoint auth rules still apply.

`/v1/mcp` is also covered by Torii’s API-token middleware. If `torii.require_api_token` is enabled and
the inbound token is missing/invalid, Torii rejects before JSON-RPC dispatch.

For route dispatch, MCP forwards inbound auth headers automatically:
- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

Per-call additional headers can also be passed via `arguments.headers`.
`content-length`, `host`, and `connection` from `arguments.headers` are ignored.

For public writer-profile deployments, treat user-supplied `authority` /
`private_key` fields and forwarded auth headers as runtime-only inputs. Do not
store deployment credentials in repo config, plugin manifests, or
documentation examples tied to real secrets.

## Protocol Behavior
- `jsonrpc` is recommended as `"2.0"`.
- If `jsonrpc` is present as a string and is not `"2.0"`, request is rejected as `invalid_request`.
- If `jsonrpc` is omitted, request is accepted for compatibility.
- POST accepts either a single request object or a non-empty request array (batch).
- Empty batch is rejected as `invalid_request`.
- Unknown method is `method_not_found`.
- Missing/non-object `params` is treated as `{}`.

### HTTP Status Behavior
- `200 OK`: JSON-RPC responses (including JSON-RPC-level errors).
- `400 Bad Request`: invalid JSON payload.
- `403 Forbidden`: API-token middleware rejected request before JSON-RPC handling.
- `413 Payload Too Large`: request exceeds `max_request_bytes`.
- `429 Too Many Requests`: MCP rate-limited.
- `404 Not Found`: MCP disabled (`torii.mcp.enabled = false`).
- `405 Method Not Allowed`: method other than GET/POST on `/v1/mcp`.

## Supported JSON-RPC Methods
- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

## Method Reference

### `initialize`
Returns MCP protocol metadata and capabilities for visible tools.

Result shape:
- `protocolVersion` (currently `2025-06-18`)
- `serverInfo` (`name`, `version`)
- `capabilities.tools` (`count`, `listChanged`, `toolsetVersion`)

### `tools/list`
Returns paginated tool descriptors.

Params:
- `cursor` (optional numeric-string offset)
- `toolset_version` or `toolsetVersion` (optional client version hash)

Result:
- `tools`: array of descriptors (`name`, `description`, `inputSchema`, `outputSchema`)
- `nextCursor`: string or `null`
- `listChanged`: `true` when client toolset hash differs
- `toolsetVersion`: current server toolset hash

Notes:
- Non-numeric `cursor` falls back to `0`.
- Effective page size is `max(1, torii.mcp.max_tools_per_list)`.

### `tools/call`
Executes one tool.

Params:
- `name` (required string)
- `arguments` (optional object)

### `tools/call_batch`
Executes multiple tool calls in one request.

Params:
- `calls` (required array of `{ "name": string, "arguments": object? }`)

Result:
- `results`: array where each entry has either `result` or `error`.

Batch execution is best-effort per item. One failing call does not fail sibling calls.

### `tools/call_async`
Starts one background tool call and returns a job handle.

Params:
- `name` (required string)
- `arguments` (optional object)

Immediate result:
- `job_id`
- `status` = `pending`

`tools/call_async` only validates envelope shape up front.
Tool lookup/policy/runtime failures are reflected later in `tools/jobs/get` state.

### `tools/jobs/get`
Fetches job state by id.

Params:
- `job_id` or `jobId` (required string)

Result:
- `job_id`
- `state`

Possible `state.status` values:
- `pending`
- `completed` (with `result`)
- `failed` (with `error`)

Async job records are in-memory only and are pruned by TTL/capacity.
Pruning is performed lazily on async job inserts and `tools/jobs/get` reads.

## Tool Names And Discovery
Tool names are stable and generated from HTTP method + path for OpenAPI-derived routes:
- format: `torii.<method>_<path...>`
- example: `torii.get_v1_accounts`

Additional curated aliases are provided under `connect.*` and `iroha.*`.

For public Codex-facing deployments, prefer publishing only `iroha.*` aliases.
Those names are curated for live account, asset, contract, governance, and
transaction workflows and are substantially easier for an agent to use than the
full raw `torii.*` catalog.

Streaming/internal paths are intentionally excluded from MCP tool generation (for example SSE/WS stream routes and `/v1/mcp` itself).

Do not hardcode the full tool catalog in clients.
Use `initialize` + `tools/list` for runtime discovery.

## Tool Arguments
For OpenAPI-derived tools, pass structured arguments under:
- `path`: path-template variables
- `query`: query parameters
- `body`: request payload (JSON or textual)
- `body_base64`: binary request body
- `content_type`: request content type override
- `headers`: extra headers
- `accept`: Accept header override
- `project`: optional body-key projection of structured response

Body/headers behavior:
- `body_base64` takes precedence over `body`.
- When `body` is used and `content_type` is omitted, Torii sends `application/json`.
- When `body_base64` is used and `content_type` is omitted, Torii defaults to Norito MIME.
- `arguments.headers` entries for `content-length`, `host`, and `connection` are ignored.

Many `iroha.*` alias tools also accept flat shortcut keys (for example `account_id`, `hash`, `definition_id`, `limit`, `offset`).
Rely on each tool’s `inputSchema` for authoritative accepted fields.

The live-network write-oriented aliases intentionally support the existing
Torii JSON request bodies used by deployed app endpoints, so Codex can work
with runtime-supplied `authority` / `private_key` JSON fields on supported
routes such as contract, governance, onboarding, faucet, subscription, and
submit-and-wait flows.

## Tool Result Contract
`tools/call` returns a JSON-RPC `result` object with MCP tool semantics:
- `content`: text summary array
- `isError`: boolean
- `structuredContent`: structured payload

For route-dispatched tools, `structuredContent` typically contains:
- `status`: HTTP status code
- `headers`: response headers map
- `content_type`: response content type or `null`
- `body`: decoded response body

Body decoding rules:
- JSON content-types are decoded as JSON.
- UTF-8 bodies are returned as strings.
- Non-UTF8 bodies are base64-encoded strings.

If `structuredContent.status >= 400`, `isError` is set to `true` and `structuredContent.error_code` is added.
Route-dispatched HTTP status mapping:
- `400` -> `bad_request`
- `401` -> `unauthorized`
- `403` -> `forbidden`
- `404` -> `not_found`
- `405` -> `method_not_allowed`
- `409` -> `conflict`
- `413` -> `payload_too_large`
- `415` -> `unsupported_media_type`
- `422` -> `unprocessable_entity`
- `429` -> `rate_limited`
- `5xx` -> `server_error`

## JSON-RPC Error Contract
Protocol/validation errors are returned in top-level `error` with stable `error.data.error_code`.

Primary top-level JSON-RPC codes:
- `-32700` -> `parse_error`
- `-32600` -> `invalid_request`
- `-32601` -> `method_not_found`
- `-32602` -> `invalid_params`
- `-32029` -> `rate_limited`

Additional MCP-specific `error_code` values may appear in `error.data`:
- `tool_not_found`
- `tool_not_allowed`
- `job_not_found`

Notes:
- Tool runtime failures are returned as MCP tool results (`result.isError = true`) with
  `result.structuredContent.error_code = "tool_execution_error"`.
- `-32603/internal_error` is used for malformed internal batch-item handling fallbacks.

## Minimal Usage Flow
1. `GET /v1/mcp` (optional) or JSON-RPC `initialize`.
2. `tools/list` and cache `toolsetVersion`.
3. Call tools with `tools/call`.
4. For long-running work, use `tools/call_async` + `tools/jobs/get` polling.
5. Re-run `tools/list` when `listChanged` becomes `true`.

## Codex Plugin Workflow

This repo ships a Codex plugin bundle under `plugins/iroha/` that assumes the
target Torii host already exposes native MCP at `/v1/mcp`.

- Built-in preset: `https://taira.sora.org/v1/mcp`
- Custom deployments: add a user-local MCP server, for example
  `codex mcp add iroha-custom --url https://<torii>/v1/mcp`

The plugin does not parameterize `.mcp.json` and does not persist secrets. For
custom networks, keep endpoint-specific auth and any signing material in the
user's local Codex MCP configuration or pass them as explicit runtime inputs to
supported `iroha.*` tools.

## Examples

### Initialize
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {}
}
```

### List Tools (paged)
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list",
  "params": {
    "cursor": "0"
  }
}
```

### Call OpenAPI-Derived Tool
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "torii.get_v1_accounts_account_id_transactions",
    "arguments": {
      "path": {
        "account_id": "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE"
      },
      "query": {
        "limit": 20,
        "offset": 0
      }
    }
  }
}
```

### Call Alias Tool With Flat Arguments
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.transactions",
    "arguments": {
      "account_id": "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE",
      "limit": 20,
      "offset": 0
    }
  }
}
```

### Batch Calls
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "tools/call_batch",
  "params": {
    "calls": [
      { "name": "iroha.health" },
      { "name": "iroha.status" }
    ]
  }
}
```

### Async Call + Poll
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "tools/call_async",
  "params": {
    "name": "iroha.transactions.wait",
    "arguments": {
      "hash": "5f6d..."
    }
  }
}
```

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "tools/jobs/get",
  "params": {
    "job_id": "<job-id-from-call-async>"
  }
}
```
