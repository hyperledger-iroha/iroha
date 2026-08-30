# Torii MCP API

Torii exposes a native Model Context Protocol bridge at `/v1/mcp`.
It lets MCP clients discover tools and call Torii/Connect endpoints through JSON-RPC.

## Enable And Configure
MCP defaults to the bounded `read_only` profile with operator routes hidden.
Set `torii.mcp.enabled = false` to disable it, or configure a broader profile
only when the deployment policy requires one.

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "max_inflight_dispatches": 32,
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
- `max_request_bytes`: byte limit for the accepted POST body, each collected
  nested-route response body, and the complete emitted JSON-RPC response. The
  shared limit prevents a small tool request from amplifying into an unbounded
  in-memory response.
- `max_tools_per_list`: pagination size for `tools/list`.
- `max_inflight_dispatches`: maximum number of tool dispatches executing at
  once. Calls beyond the configured capacity fail closed with a retryable
  `dispatch_capacity_exhausted` JSON-RPC error.
- `profile`: `read_only`, `writer`, or `operator`.
- `expose_operator_routes`: second, explicit gate for operator tools. Operator
  tools are published only when this is `true` **and** `profile` is `operator`.
- `allow_tool_prefixes`: if non-empty, only matching tool-name prefixes are allowed.
- `deny_tool_prefixes`: blocked tool-name prefixes (applied before allow-list).
- `rate_per_minute` / `burst`: MCP dispatch token-bucket limits. An ordinary
  request costs one token; `tools/call_batch` costs one token per requested
  tool call.

Profile behavior:

- `read_only`: read-only and instruction-builder tools only.
- `writer`: includes mutating non-operator tools.
- `operator`: makes operator tools eligible only when
  `expose_operator_routes = true` as well.

When `allow_tool_prefixes` is set, the profile still applies first and the
prefix allow-list is applied second. Public networks can therefore use
`profile = "writer"` together with `allow_tool_prefixes = ["iroha."]` to keep
mutating app-development helpers available without publishing the broader raw
surface.

## Endpoints

- `POST /v1/mcp`: JSON-RPC 2.0 execution endpoint.
- `GET /v1/mcp`: not supported; returns `405 Method Not Allowed` because Torii
  does not provide an SSE stream on this endpoint.

The route remains registered when `torii.mcp.enabled` is `false`; after
listener-wide admission and authentication, POST returns `503 Service
Unavailable` with `mcp_disabled`.

## Security And Header Forwarding
MCP does not bypass Torii authentication. `POST /v1/mcp` is a nested-route
gateway: it can reach mutations, but every call is dispatched through the
authoritative router and the exact selected route admits its own principal
before any target effect.

`/v1/mcp` is also covered by Torii’s API-token middleware. If
`torii.require_api_token` is enabled and the inbound token is missing or
invalid, Torii returns `401 Unauthorized` before JSON-RPC dispatch. If tokens
are required but none are configured, the listener returns `503 Service
Unavailable` with `api_token_unavailable`.

Browser-style requests are also subject to an exact Origin check. Requests
without `Origin` remain valid for non-browser MCP clients. If `Origin` is
present, exactly one value must byte-for-byte match an explicit
`torii.cors.allowed_origins` entry; duplicate values, wildcard origins, and
unlisted origins are rejected. When CORS is disabled there is no browser-origin
allowlist, so requests carrying `Origin` are rejected.

For ordinary route dispatch, MCP automatically forwards only these
transport-scoped credentials:

- `Authorization`
- `x-api-token`

The exact onboarding routes also forward the outer
`x-iroha-onboarding-token`; other targets cannot receive it.

Canonical account and operator signatures are inner-route proofs, not proofs
over the outer MCP envelope. A catalog target that requires canonical account
authentication therefore requires the complete account/signature/timestamp/
nonce tuple (or an exclusive witness) in `arguments.headers`. Operator targets
similarly require their complete four-header tuple. These authentication
headers are accepted only for the matching catalog policy; attempts to inject
them into another target are rejected or stripped. `content-length`, `host`,
and `connection` from `arguments.headers` are ignored.

Never pass raw private keys, seed phrases, or signing key files through MCP.
Build with instruction-builder tools where available, sign locally, and submit
a pre-signed transaction envelope. Bearer tokens and forwarded authentication
headers are runtime-only inputs and must not be stored in repo config, plugin
manifests, logs, or documentation examples.

Route output is untrusted data. Torii reparses JSON route bodies into a Norito
JSON value before placing them under `structuredContent`; malformed JSON and
non-JSON UTF-8 bodies are represented as escaped JSON strings. Success
`content[].text` is derived only from the HTTP status and never includes route
body, metadata, role, trigger, or permission text. MCP clients must likewise
treat `structuredContent` as data rather than instructions.

## Protocol Behavior

- `jsonrpc` is required and must be the string `"2.0"`.
- Missing, non-string, or different `jsonrpc` values are rejected as `invalid_request`.
- Each POST accepts exactly one JSON-RPC request, notification, or response
  object. JSON-RPC array batches, including empty arrays, are rejected as
  `invalid_request`.
- A request or response `id` must be a non-null string or JSON number. Numeric
  IDs, including floating-point values, are echoed as parsed JSON values.
- The initialization request may omit `MCP-Protocol-Version`. Every later POST
  must carry exactly one `MCP-Protocol-Version` header containing the negotiated
  version (`2025-06-18`); missing, ambiguous, or unsupported values are rejected.
- `initialize.params` must include `protocolVersion`, `capabilities`, and
  `clientInfo`. `protocolVersion` is a non-empty string, `capabilities` is an
  object, and `clientInfo` contains non-empty string `name` and `version`
  fields.
- The advertised `tools/call_batch` extension may represent at most 64 tool
  dispatches. Every requested call is charged separately against the rate
  limiter; it is not an outer JSON-RPC batch.
- Request bodies and nested-route response bodies have a 10-second collection
  deadline. Nested routes advertised as streaming operations are not eligible
  MCP tools.
- Tool execution is bounded by the 10-minute-15-second listener deadline and by
  `max_inflight_dispatches`.
- Long-polling transaction and contract wait helpers use a smaller derived
  quota (at most eight and always below the global limit), reserving capacity
  for bounded tools.
- When API-token authentication is required, `notifications/cancelled` can stop
  the currently registered live `tools/call` or `tools/call_batch` owned by the
  same validated token principal and carrying the exact request ID plus
  per-call cancellation nonce. Put a canonical unpadded base64url encoding of
  32 random bytes in
  `params._meta["iroha/cancellationNonce"]` on the call and echo it in the
  cancellation notification. This Iroha extension prevents a delayed
  cancellation from targeting a later call that reuses the same JSON-RPC ID.
  String and losslessly parsed signed/unsigned integer IDs are type-tagged so
  numerically similar representations cannot alias.
  Floating-point-form or out-of-range numeric IDs still receive normal JSON-RPC
  responses but are
  deliberately not remotely cancellable because JSON parsing cannot preserve
  their exact wire identity.
- Anonymous MCP calls remain usable but are not remotely cancellable: source IP
  is intentionally not treated as cancellation authentication.
- Unknown method is `method_not_found`.
- Missing `params` is treated as `{}` where the method permits it; non-object
  `params` and non-object `tools/call.arguments` are rejected as
  `invalid_params`.

### HTTP Status Behavior

- `200 OK`: JSON-RPC responses (including JSON-RPC-level errors).
- `202 Accepted`: accepted MCP notifications and client response messages (no
  response body).
- `204 No Content`: the original authenticated request was cancelled before it
  produced a JSON-RPC response. Cancellation does not roll back a transaction
  already submitted to the ledger pipeline.
- `400 Bad Request`: invalid JSON, an outer JSON-RPC array, or an unsupported or
  ambiguous protocol-version header.
- `408 Request Timeout`: request body did not complete within the collection deadline.
- `401 Unauthorized`: API-token middleware rejected a missing or invalid
  token.
- `403 Forbidden`: a supplied `Origin` did not exactly match the allowlist.
- `413 Payload Too Large`: request exceeds `max_request_bytes`.
- `429 Too Many Requests`: MCP rate-limited.
- `405 Method Not Allowed`: any HTTP method other than POST, including GET.
- `503 Service Unavailable`: MCP disabled (`torii.mcp.enabled = false`) after
  outer admission, or API tokens are required but none are configured.

## Supported JSON-RPC Methods

- `initialize`
- `notifications/initialized` (accepted as a notification; returns `202 Accepted` with an empty body)
- `notifications/cancelled` (best-effort exact authenticated cancellation; returns `202 Accepted`)
- `ping`
- `tools/list`
- `tools/call`
- `tools/call_batch` (advertised Iroha extension)

## Method Reference

### `initialize`

Requires `params.protocolVersion`, `params.capabilities`, and
`params.clientInfo`. It returns MCP protocol metadata, server instructions, and
capabilities for visible tools. Use the returned `protocolVersion` as the exact
`MCP-Protocol-Version` header on subsequent POST requests.

Result shape:

- `protocolVersion` (currently `2025-06-18`)
- `serverInfo` (`name`, the real Torii crate `version`)
- `capabilities.tools` (`listChanged`)
- `capabilities.experimental.iroha.tools` (`count`, `toolsetVersion`, and
  `callBatch.maxDispatches`)
- `instructions` (server-wide safety and discovery guidance)

### `notifications/initialized`
Carries the standard client-ready lifecycle signal after a successful
`initialize` response.

Torii's MCP HTTP transport is stateless and does not mint or retain an MCP
session identifier. The notification therefore does not unlock server-side
session state: negotiation is enforced per request. `initialize` may omit the
protocol header, while every other POST must carry the exact supported
`MCP-Protocol-Version`. Clients should still follow the standard initialize,
initialized-notification, then tool-discovery sequence.

HTTP behavior:

- `202 Accepted`
- empty response body

Torii accepts the notification when:
- `method == "notifications/initialized"`
- `id` is omitted
- `jsonrpc == "2.0"`

### `notifications/cancelled`

Accepts the standard best-effort cancellation shape with
`params.requestId` and an optional string `params.reason`, extended with
`params._meta["iroha/cancellationNonce"]`. The original call and notification
must carry the same canonical unpadded base64url encoding of exactly 32 bytes.
Cancellation is enabled only for requests admitted with one exact configured
API token and is bound to the token fingerprint, exact string or losslessly
parsed signed/unsigned integer JSON-RPC ID, and cancellation nonce. Calls that
omit the nonce remain ordinary non-cancellable calls; malformed nonces on an
authenticated call are rejected as `invalid_cancellation_nonce`.
Floating-point-form and out-of-range numeric IDs are accepted for ordinary
requests but are not entered in the cancellation registry.
Unknown, completed, malformed, anonymous, and cross-principal cancellations
are deliberately indistinguishable `202 Accepted` responses. A simultaneous
duplicate live cancellable ID for the same authenticated principal is rejected
as `request_id_in_use`; after completion, the ID can be reused safely with a
fresh random cancellation nonce.

### `ping`
Returns an empty result object so MCP clients can use the standard lifecycle
ping before or after initialization.

### `tools/list`

Returns paginated tool descriptors.

Params:

- `cursor` (optional numeric-string offset)
- `toolset_version` or `toolsetVersion` (optional client version hash)

Result:

- `tools`: array of descriptors (`name`, `description`, `inputSchema`,
  `outputSchema`, `annotations`)
- `nextCursor`: string when another page exists; omitted on the final page
- `_meta.iroha.listChanged`: `true` when the client toolset hash differs
- `_meta.iroha.toolsetVersion`: current server toolset hash

Notes:

- A non-string, non-numeric, or out-of-range `cursor` is rejected as
  `invalid_cursor`.
- Effective page size is `max(1, torii.mcp.max_tools_per_list)`.
- `inputSchema` is sanitized before publication so clients always see a
  top-level object schema. Security-relevant `anyOf`, `oneOf`, `allOf`, and
  `not` constraints plus `if`/`then`/`else` branches are preserved and enforced
  before dispatch. OpenAPI references are recursively inlined; registry
  construction fails closed if an unresolved `$ref` remains.
- `annotations.readOnlyHint`, `destructiveHint`, and `idempotentHint` describe
  the registered tool effect. Clients must still treat annotations as hints and
  obtain explicit approval for mutations.

### `tools/call`

Executes one tool.

Params:

- `name` (required string)
- `arguments` (optional object)

### `tools/call_batch`

Executes multiple tool calls inside one JSON-RPC request. This is an Iroha
extension advertised by `initialize`; it is not JSON-RPC array batching.

Params:

- `calls` (required array of `{ "name": string, "arguments": object? }`)

Result:

- `results`: array where each entry has either `result` or `error`.

Batch execution is best-effort per item. One failing call does not fail sibling calls.
The `calls` array is subject to the 64-dispatch ceiling, each item consumes one
rate-limit token, and each active item must acquire an in-flight dispatch slot.
If retained batch results or the final response exceed `max_request_bytes`, Torii stops
retaining further results and returns the typed `response_too_large` JSON-RPC
error instead of allocating an oversized response.

## Tool Names And Discovery
Tool names are stable and generated from HTTP method + path for
OpenAPI-derived routes, but only when the catalog explicitly enables that
exact method/path pair in its MCP projection for the compiled feature set:
- format: `torii.<method>_<path...>`
- example: `torii.get_health`

Additional purpose-built tools under `iroha.*` form a separate, code-defined
allowlist. A purpose-built alias for a catalogued route is retained
only while that route's compiled feature gate is enabled; an uncatalogued target
can be reachable only through an alias that is itself explicitly registered.
OpenAPI presence alone never publishes a tool. Generated tool candidates,
feature-disabled aliases, streaming operations, and other non-projected
operations fail closed. `tools/call` accepts only exact names returned by
`tools/list`; Torii does not resolve `operationId`, bare `connect.*` aliases, or
other retired convenience names.

The canonical Connect tools are `iroha.connect.ws.ticket`,
`iroha.connect.session.create`, `iroha.connect.session.delete`, and
`iroha.connect.session.status`. `iroha.connect.ws.ticket` requires an explicit
trusted `node_url`; Torii never constructs a credential-bearing destination
from `Host` or forwarding headers. The former composite
`iroha.connect.session.create_and_ticket` is retired: create the session first,
then call the ticket helper with the selected role token and trusted URL.
Role tokens must be the canonical unpadded base64url encoding of exactly 32
bytes; invalid or header-unsafe token text is rejected.

The curated `iroha.accounts.faucet.prepare` and
`iroha.accounts.faucet.submit` tools expose the exact two-step faucet protocol.
They are omitted from initialization metadata and `tools/list` when this node
has no `torii.faucet` runtime configuration.
Prepared faucet transactions carry a signature-bound marker version and
semantic claim hash. Core derives an authority-scoped key and consumes it in
the same state overlay as successful transaction execution; a failed transfer
does not burn the claim, while distinct bindings, peers, generic transaction
ingress, and restarts cannot make the same faucet authority consume it twice.
Both tools are classified as writes. Submit only the unmodified envelope
returned by prepare and keep all runtime authentication material outside tool
arguments. The marker is mandatory for every prepared faucet transaction;
Core rejects prepared-faucet metadata without the complete marker instead of
admitting an untracked claim.

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
- `project`: optional body-key projection of structured response, limited to 64
  unique keys of at most 128 characters each

Body/headers behavior:

- `body` and `body_base64` are mutually exclusive; supplying both is rejected.
- When `body` is used and `content_type` is omitted, Torii sends `application/json`.
- When `body_base64` is used and `content_type` is omitted, Torii defaults to Norito MIME.
- `arguments.headers` entries for `content-length`, `host`, and `connection` are ignored.

Many purpose-built `iroha.*` tools also accept flat shortcut keys (for example `account_id`, `hash`, `definition_id`, `limit`, `offset`).
Rely on each tool’s `inputSchema` for authoritative accepted fields.

Do not place a raw private key in `arguments`, `body`, or forwarded headers.
Use unsigned instruction builders plus local signing, or submit an already
signed transaction envelope with `iroha.transactions.submit_and_wait`.

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
- `-32004` -> `dispatch_capacity_exhausted`
- `-32029` -> `rate_limited`

Additional MCP-specific `error_code` values may appear in `error.data`:

- `tool_not_found`
- `tool_not_allowed`
- `tool_unavailable`
- `long_poll_capacity_exhausted`
- `request_id_in_use`
- `invalid_cancellation_nonce`
- `cancellation_registry_capacity_exhausted`
- `origin_forbidden`
- `request_body_read_failed`
- `request_payload_too_large`
- `request_timeout`
- `tool_schema_validation_failed`
- `unsupported_protocol_version`

Notes:
- Arguments that violate the tool's advertised input schema are rejected before
  dispatch as top-level JSON-RPC `-32602` errors with
  `error.data.error_code = "tool_schema_validation_failed"`.
- Schema-valid local builder or execution failures that do not produce an inner
  HTTP response are returned as MCP tool results (`result.isError = true`) with the canonical error envelope
  `result.structuredContent.code = "tool_execution_error"`. The envelope also
  contains `message` and may contain `details`. This is distinct from the
  status-derived `error_code` on route-dispatched HTTP failures described above.
- `-32603/internal_error` is used for malformed internal batch-item handling fallbacks.

## Minimal Usage Flow

1. POST a valid JSON-RPC `initialize` request with the required client fields.
2. Send `notifications/initialized` and include the negotiated
   `MCP-Protocol-Version` header on subsequent POSTs.
3. Call `tools/list` and cache `_meta.iroha.toolsetVersion`.
4. Call tools with `tools/call`, or use the advertised `tools/call_batch`
   extension when its per-dispatch charging is acceptable.
5. Re-run `tools/list` when `_meta.iroha.listChanged` becomes `true`.

## Codex Plugin Workflow

This repo ships a Codex plugin bundle under `plugins/iroha/` that assumes the
target Torii host already exposes native MCP at `/v1/mcp`.

- Built-in preset: `https://taira.sora.org/v1/mcp`
- Custom deployments: add a user-local MCP server, for example
  `codex mcp add iroha-custom --url https://<torii>/v1/mcp`
- Standalone skill: this repo also ships `skills/sora-taira-testnet/` for the
  Codex Skills surface. Install it from a GitHub checkout of this repo with
  your local skill installer and restart Codex so it appears in the Skills tab.

The plugin does not parameterize `.mcp.json` and does not persist secrets. Keep
endpoint-specific bearer authentication in the user's local Codex MCP
configuration. Keep signing keys outside MCP and submit locally signed
transactions.

## Examples

### Initialize

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-06-18",
    "capabilities": {},
    "clientInfo": {
      "name": "example-client",
      "version": "1.0.0"
    }
  }
}
```

After initialization, send `MCP-Protocol-Version: 2025-06-18` on each POST.

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
      { "name": "iroha.parameters.get" }
    ]
  }
}
```
