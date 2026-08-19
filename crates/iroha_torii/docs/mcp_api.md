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
      "burst": 120
    }
  }
}
```

### Public Deployed-Network Profile

For public SORA/Torii deployments intended to be used from Codex, the
recommended policy is a curated writer profile that exposes only the stable
purpose-built `iroha.*` tools:

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
catalog-projected `torii.*` namespace and all operator routes.

### Configuration Fields
- `enabled`: master switch for `/v1/mcp`.
- `max_request_bytes`: POST body limit for MCP JSON-RPC.
- `max_tools_per_list`: pagination size for `tools/list`.
- `profile`: `read_only`, `writer`, or `operator`.
- `expose_operator_routes`: include operator routes even when profile is not `operator`.
- `allow_tool_prefixes`: if non-empty, only matching tool-name prefixes are allowed.
- `deny_tool_prefixes`: blocked tool-name prefixes (applied before allow-list).
- `rate_per_minute` / `burst`: MCP endpoint token-bucket limits.

Profile behavior:
- `read_only`: read-only and instruction-builder tools only.
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
MCP does not bypass Torii authentication. `POST /v1/mcp` is a nested-route
gateway: it can reach mutations, but every call is dispatched through the
authoritative router and the exact selected route admits its own principal
before any target effect.

`/v1/mcp` is also covered by Torii’s API-token middleware. If `torii.require_api_token` is enabled and
the inbound token is missing/invalid, Torii rejects before JSON-RPC dispatch.

For route dispatch, MCP forwards only transport-scoped credentials automatically:
- `Authorization`
- `x-api-token`

Canonical account and operator signatures are inner-route proofs, not proofs
over the outer MCP envelope. A catalog target that requires canonical account
authentication therefore requires the complete account/signature/timestamp/
nonce tuple (or an exclusive witness) in `arguments.headers`. Operator targets
similarly require their complete four-header tuple. These authentication
headers are accepted only for the matching catalog policy; attempts to inject
them into another target are rejected or stripped. `content-length`, `host`,
and `connection` from `arguments.headers` are ignored.

For public writer-profile deployments, treat user-supplied `authority` /
`private_key` fields and forwarded auth headers as runtime-only inputs. Do not
store deployment credentials in repo config, plugin manifests, or
documentation examples tied to real secrets.

Route output is untrusted data. Torii reparses JSON route bodies into a Norito
JSON value before placing them under `structuredContent`; malformed JSON and
non-JSON UTF-8 bodies are represented as escaped JSON strings. Success
`content[].text` is derived only from the HTTP status and never includes route
body, metadata, role, trigger, or permission text. MCP clients must likewise
treat `structuredContent` as data rather than instructions.

## Protocol Behavior
- `jsonrpc` is required and must be the string `"2.0"`.
- Missing, non-string, or different `jsonrpc` values are rejected as `invalid_request`.
- POST accepts either a single request object or a non-empty request array (batch).
- Empty batch is rejected as `invalid_request`.
- Unknown method is `method_not_found`.
- Missing/non-object `params` is treated as `{}`.

### HTTP Status Behavior
- `200 OK`: JSON-RPC responses (including JSON-RPC-level errors).
- `202 Accepted`: accepted MCP notifications such as `notifications/initialized` (no response body).
- `400 Bad Request`: invalid JSON payload.
- `403 Forbidden`: API-token middleware rejected request before JSON-RPC handling.
- `413 Payload Too Large`: request exceeds `max_request_bytes`.
- `429 Too Many Requests`: MCP rate-limited.
- `404 Not Found`: MCP disabled (`torii.mcp.enabled = false`).
- `405 Method Not Allowed`: method other than GET/POST on `/v1/mcp`.

## Supported JSON-RPC Methods
- `initialize`
- `notifications/initialized` (accepted as a notification; returns `202 Accepted` with an empty body)
- `ping`
- `tools/list`
- `tools/call`
- `tools/call_batch`

## Method Reference

### `initialize`
Returns MCP protocol metadata and capabilities for visible tools.

Result shape:
- `protocolVersion` (currently `2025-06-18`)
- `serverInfo` (`name`, `version`)
- `capabilities.tools` (`count`, `listChanged`, `toolsetVersion`)

### `notifications/initialized`
Marks the client ready for normal MCP operations after a successful `initialize`
response.

HTTP behavior:
- `202 Accepted`
- empty response body

Torii accepts the notification when:
- `method == "notifications/initialized"`
- `id` is omitted
- `jsonrpc == "2.0"`

### `ping`
Returns an empty result object so MCP clients can use the standard lifecycle
ping before or after initialization.

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
- `inputSchema` is sanitized before publication so OpenAI-compatible MCP clients
  always see a top-level object schema and never a top-level
  `anyOf`/`oneOf`/`allOf`/`enum`/`not` keyword.

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

## Tool Names And Discovery
Tool names are stable and generated from HTTP method + path for
OpenAPI-derived routes, but only when the catalog explicitly enables that
exact method/path pair in its MCP projection for the compiled feature set:
- format: `torii.<method>_<path...>`
- example: `torii.get_health`

Additional purpose-built tools under `connect.*` and `iroha.*` form a separate,
code-defined allowlist. A purpose-built alias for a catalogued route is retained
only while that route's compiled feature gate is enabled; an uncatalogued target
can be reachable only through an alias that is itself explicitly registered.
The diagnostic and ledger/proof mirrors (`iroha.status`, `iroha.time.now`,
`iroha.time.status`, `iroha.ledger.headers`, `iroha.ledger.state_root`,
`iroha.ledger.state_proof`, `iroha.ledger.block_proof`, `iroha.proofs.get`, and
`iroha.proofs.retention`) additionally require the exact route-catalog MCP
projection. OpenAPI presence alone never publishes a tool. Generated tool
candidates, projected mirrors, feature-disabled aliases, streaming operations,
and other non-projected operations fail closed. `tools/call` accepts only exact
names returned by `tools/list`; Torii does not resolve `operationId` or retired
convenience-name aliases.

For public Codex-facing deployments, prefer publishing only `iroha.*` tools.
Those names are curated for live account, asset, contract, governance, and
transaction workflows and are substantially easier for an agent to use than the
lower-level catalog-projected `torii.*` tools.

Streaming/internal paths are intentionally excluded from MCP tool generation
(for example SSE/WS stream routes and `/v1/mcp` itself). In addition to the
route-catalog decision, generated tools reject OpenAPI operations whose
response contract advertises `text/event-stream` or HTTP `101`, so streaming
exclusion does not depend on a path suffix.

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

Many purpose-built `iroha.*` tools also accept flat shortcut keys (for example `account_id`, `hash`, `definition_id`, `limit`, `offset`).
Rely on each tool’s `inputSchema` for authoritative accepted fields.

The live-network write-oriented aliases intentionally support the existing
Torii JSON request bodies used by deployed app endpoints, so Codex can work
with runtime-supplied `authority` / `private_key` JSON fields on supported
routes such as contract, governance, onboarding, faucet, subscription, and
submit-and-wait flows.

### Musubi Package Registry Tools

Musubi V1 is exposed through curated `iroha.musubi.*` tools and typed `POST`
routes under `/v1/musubi/queries/*` and `/v1/musubi/instructions/*`. Every tool
accepts one `body` object containing the exact bounded Norito V1 request or
instruction. Flat query shortcuts and the pre-release string package envelopes
are not accepted.

Read tools:

- `iroha.musubi.queries.exact_package`
- `iroha.musubi.queries.exact_release`
- `iroha.musubi.queries.provider_bundle_attestation`
- `iroha.musubi.queries.resolver_index`
- `iroha.musubi.queries.versions`
- `iroha.musubi.queries.maintainers`
- `iroha.musubi.queries.archive_locations`
- `iroha.musubi.queries.archive_retention`
- `iroha.musubi.queries.alias`
- `iroha.musubi.queries.alias_history`
- `iroha.musubi.queries.ordered_prefix`
- `iroha.musubi.queries.search`

Instruction-builder tools:

- `iroha.musubi.instructions.namespace_binding_register`
- `iroha.musubi.instructions.archive_register`
- `iroha.musubi.instructions.provider_bundle_attestation_register`
- `iroha.musubi.instructions.archive_location_add`
- `iroha.musubi.instructions.archive_location_retire`
- `iroha.musubi.instructions.release_publish`
- `iroha.musubi.instructions.release_yank_set`
- `iroha.musubi.instructions.package_metadata_set`
- `iroha.musubi.instructions.package_member_invite`
- `iroha.musubi.instructions.package_member_accept`
- `iroha.musubi.instructions.package_member_invitation_revoke`
- `iroha.musubi.instructions.package_member_set_role`
- `iroha.musubi.instructions.package_member_remove`
- `iroha.musubi.instructions.alias_register`
- `iroha.musubi.instructions.package_recover`
- `iroha.musubi.instructions.alias_retarget`
- `iroha.musubi.instructions.artifact_takedown`
- `iroha.musubi.instructions.registry_policy_set`
- `iroha.musubi.instructions.release_digest_assert`

The Musubi instruction builders are pre-signing helpers only. They never accept
`authority`, `private_key`, or bearer-token material. Each returns `wire_id`,
`instruction_base64`, `instruction_hex`, and an `instruction_json` preview in a
`musubi-instruction-envelope` version-1 document. The caller must assemble and
sign a transaction locally, then submit it with `iroha.transactions.submit_and_wait`.
Manifests and lockfiles normalize public namespace or permanent-alias input to
the structural `MusubiPackageIdV1`. Exact Torii registry queries use that
structural identity. `PublishMusubiReleaseV1` additionally carries the exact
canonical namespace whose immutable binding authorizes a first package claim;
the registry never infers it by scanning structural package identifiers.

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
- JSON content-types are reparsed into a typed Norito JSON value.
- Malformed JSON and other UTF-8 bodies are returned as escaped strings.
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

Notes:
- Tool validation or execution failures that do not produce an inner HTTP
  response are returned as MCP tool results (`result.isError = true`) with the
  canonical error envelope
  `result.structuredContent.code = "tool_execution_error"`. The envelope also
  contains `message` and may contain `details`. This is distinct from the
  status-derived `error_code` on route-dispatched HTTP failures described above.
- `-32603/internal_error` is used for malformed internal batch-item handling fallbacks.

## Minimal Usage Flow
1. `GET /v1/mcp` (optional) or JSON-RPC `initialize`.
2. `tools/list` and cache `toolsetVersion`.
3. Call tools with `tools/call`.
4. Re-run `tools/list` when `listChanged` becomes `true`.

## Codex Plugin Workflow

This repo ships a Codex plugin bundle under `plugins/iroha/` that assumes the
target Torii host already exposes native MCP at `/v1/mcp`.

- Built-in preset: `https://taira.sora.org/v1/mcp`
- Custom deployments: add a user-local MCP server, for example
  `codex mcp add iroha-custom --url https://<torii>/v1/mcp`
- Standalone skill: this repo also ships `skills/sora-taira-testnet/` for the
  Codex Skills surface. Install it from a GitHub checkout of this repo with
  your local skill installer and restart Codex so it appears in the Skills tab.

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
    "name": "torii.get_health",
    "arguments": {}
  }
}
```

### Call Purpose-Built Tool With Flat Arguments
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.transactions",
    "arguments": {
      "account_id": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
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
