# Torii MCP API

Torii exposes Model Context Protocol directly at its existing `POST /v1/mcp`
route. It lets MCP clients discover tools and resources and call reviewed
Torii/Connect endpoints through JSON-RPC.

Torii itself is the only MCP server, process, and listener involved.
Route-backed tool and resource dispatch re-enters Torii's existing
authoritative router in-process and remains subject to the selected route's
normal admission policy. Explicit Torii-local helpers execute inside the same
MCP handler without fabricating another route. This design does not start or
plan a gateway, sidecar, proxy server, second listener, or separate MCP
deployment unit.

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
- `max_inflight_dispatches`: maximum number of tool or resource-route
  dispatches executing at once. Calls beyond the configured capacity fail
  closed with a retryable `dispatch_capacity_exhausted` JSON-RPC error.
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
- `DELETE /v1/mcp`: not supported; native MCP has no protocol session to end.

The route remains registered when `torii.mcp.enabled` is `false`; after
listener-wide admission and authentication, POST returns `503 Service
Unavailable` with `mcp_disabled`.

## Security And Header Forwarding
MCP does not bypass Torii authentication. `POST /v1/mcp` is a nested-route
adapter: it can reach mutations, but every call is dispatched through the
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
- Each native POST accepts exactly one JSON-RPC request object. Native client
  notifications and JSON-RPC responses are rejected. The compatibility adapter
  additionally accepts its documented legacy notifications and client response
  objects. JSON-RPC array batches, including empty arrays, are rejected in both
  eras as `invalid_request`.
- On the legacy adapter, a request or response `id` must be a non-null string
  or JSON number. Numeric IDs, including floating-point values, are echoed as
  parsed JSON values. Native ID rules are stricter, as described below.
- Native requests use stateless MCP `2026-07-28`. Every request carries object
  `params._meta` with `io.modelcontextprotocol/protocolVersion` and
  `io.modelcontextprotocol/clientCapabilities`; client identity under
  `io.modelcontextprotocol/clientInfo` is recommended. When supplied, client
  information must contain string `name` and `version` fields.
- Every native HTTP POST carries exactly one matching
  `MCP-Protocol-Version` and `Mcp-Method` header. `tools/call` additionally
  carries `Mcp-Name` matching `params.name`; `resources/read` carries
  `Mcp-Name` matching `params.uri`. Methods with no corresponding name or URI
  reject `Mcp-Name`. `prompts/get` is not implemented; if it is added later it
  will mirror `params.name` through the same header.
- A mirrored method is ordinary header-safe ASCII. A name or URI is sent as
  plain header-safe ASCII when possible; otherwise encode its UTF-8 bytes as
  canonical padded standard Base64 and wrap them as
  `=?base64?<encoded>?=`. Sentinel-looking literal text uses the encoded form
  too. The server rejects non-canonical or ambiguous encodings.
- A header/body mismatch or missing mirrored routing header returns HTTP 400
  and JSON-RPC `-32020`. An unsupported protocol returns HTTP 400 and
  `-32022` with `supported` and `requested` values. Missing required modern
  `_meta` fields returns HTTP 400 and `-32602`.
- Modern request IDs are non-null strings or integers. An invalid or unreadable
  ID is not echoed as `null` in a modern error response.
- Successful modern results carry `resultType = "complete"` and server identity
  in `_meta["io.modelcontextprotocol/serverInfo"]`. Discovery, tool-list, and
  resource results also carry reviewed `ttlMs` and `cacheScope` hints.
- The HTTP envelope remains `Cache-Control: private, no-store` so a generic
  intermediary never caches different JSON-RPC POST bodies under the shared
  `/v1/mcp` URL. MCP-aware clients may cache the parsed result only under the
  protocol cache key (method plus result-affecting params) and, for Torii's
  `private` scope, the exact same authorization context.
- Torii retains initialization-based `2025-06-18` behavior on the same endpoint
  as a compatibility adapter. Only legacy `initialize` may omit the version
  header; later legacy requests carry `MCP-Protocol-Version: 2025-06-18`.
- The advertised `tools/call_batch` extension may represent at most 64 tool
  dispatches. A native request must independently declare
  `clientCapabilities.extensions["org.hyperledger.iroha/tools"]` as an object;
  discovery from an earlier request is not session state and does not satisfy
  this per-request negotiation. Missing extension support returns HTTP 400 and
  JSON-RPC `-32021` with `requiredCapabilities`. Every requested call is
  charged separately against the rate limiter; it is not an outer JSON-RPC
  batch.
- Request bodies and nested-route response bodies have a 10-second collection
  deadline. Nested routes advertised as streaming operations are not eligible
  MCP tools.
- Tool execution is bounded by the 10-minute-15-second listener deadline and by
  `max_inflight_dispatches`.
- Long-polling transaction and contract wait helpers use a smaller derived
  quota (at most eight and always below the global limit), reserving capacity
  for bounded tools.
- On the legacy compatibility path, when API-token authentication is required,
  `notifications/cancelled` can stop
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
- Native HTTP cancellation uses the request-scoped transport: closing an active
  response stream cancels that request. The legacy cancellation notification is
  not accepted as a native `2026-07-28` request.
- Unknown method is `method_not_found`.
- Missing `params` is treated as `{}` where the method permits it; non-object
  `params` and non-object `tools/call.arguments` are rejected as
  `invalid_params`.

### HTTP Status Behavior

- `200 OK`: successful JSON-RPC responses and ordinary method-level JSON-RPC
  errors.
- `202 Accepted`: accepted legacy MCP notifications and legacy client response
  messages (no response body).
- `204 No Content`: the original authenticated request was cancelled before it
  produced a JSON-RPC response. Cancellation does not roll back a transaction
  already submitted to the ledger pipeline.
- `400 Bad Request`: invalid JSON, an outer JSON-RPC array, malformed modern
  metadata, mirrored-header mismatch, an unsupported protocol version, or a
  prohibited native notification/client-response body.
- `408 Request Timeout`: request body did not complete within the collection deadline.
- `401 Unauthorized`: API-token middleware rejected a missing or invalid
  token.
- `403 Forbidden`: a supplied `Origin` did not exactly match the allowlist.
- `404 Not Found`: a native request names an unimplemented JSON-RPC method.
- `413 Payload Too Large`: request exceeds `max_request_bytes`.
- `429 Too Many Requests`: MCP rate-limited.
- `405 Method Not Allowed`: any HTTP method other than POST, including GET.
- `503 Service Unavailable`: MCP disabled (`torii.mcp.enabled = false`) after
  outer admission, or API tokens are required but none are configured.

## Supported JSON-RPC Methods

Native `2026-07-28`:

- `server/discover`
- `tools/list`
- `tools/call`
- `tools/call_batch` (advertised Iroha extension)
- `resources/list`
- `resources/read`

Legacy `2025-06-18` compatibility adds:

- `initialize`
- `notifications/initialized` (accepted with `202 Accepted` and an empty body)
- `notifications/cancelled` (best-effort exact authenticated cancellation)
- `ping`

## Method Reference

### `server/discover`

This mandatory server method returns the versions served on the existing Torii
endpoint, request-independent capabilities, server identity, safety
instructions, and private cache hints. Calling it is optional for clients: a
native client may call `tools/list` directly and handle a typed unsupported
version error.

Result shape:

- `resultType = "complete"`
- `supportedVersions = ["2026-07-28", "2025-06-18"]`
- `capabilities.tools.listChanged`
- `capabilities.resources.listChanged = false`
- `capabilities.extensions["org.hyperledger.iroha/tools"]`, including the
  current toolset hash and bounded `tools/call_batch` contract
- `_meta["io.modelcontextprotocol/serverInfo"]`
- `instructions`, `ttlMs`, and `cacheScope = "private"`

### `initialize` (legacy compatibility)

Only the `2025-06-18` adapter accepts this method. It requires
`params.protocolVersion`, `params.capabilities`, and
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

### `notifications/initialized` (legacy compatibility)
Carries the standard client-ready lifecycle signal after a successful
`initialize` response.

Torii's legacy MCP HTTP adapter does not mint or retain an MCP
session identifier. The notification therefore does not unlock server-side
session state: negotiation is enforced per request. `initialize` may omit the
protocol header, while every other POST must carry the exact supported
`MCP-Protocol-Version`. This sequence exists only for compatibility; native
clients do not send it.

HTTP behavior:

- `202 Accepted`
- empty response body

Torii accepts the notification when:
- `method == "notifications/initialized"`
- `id` is omitted
- `jsonrpc == "2.0"`

### `notifications/cancelled` (legacy compatibility)

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

### `ping` (legacy compatibility)

Returns an empty result object for clients using the initialization-based
`2025-06-18` adapter. Native clients do not send lifecycle pings.

### `tools/list`

Returns paginated tool descriptors.

Params:

- `cursor` (optional numeric-string offset)
- `toolset_version` or `toolsetVersion` (optional client version hash)

Result:

- `tools`: array of descriptors (`name`, `description`, `inputSchema`,
  `outputSchema`, `annotations`, and optional `_meta`)
- `nextCursor`: string when another page exists; omitted on the final page
- `_meta.iroha.listChanged`: `true` when the client toolset hash differs
- `_meta.iroha.toolsetVersion`: current server toolset hash
- Native responses also include `resultType = "complete"`, `ttlMs`,
  `cacheScope = "private"`, and
  `_meta["io.modelcontextprotocol/serverInfo"]`.

Catalog-backed tool descriptors also publish `_meta["iroha/routeAuth"]` with
the exact route-catalog contract:

```json
{
  "schemaVersion": 1,
  "stableRouteId": "application.accounts_by_account_id_history_get",
  "authentication": "optional_canonical_account_signature",
  "admission": "dataspace_visible"
}
```

`stableRouteId` is the route's stable catalog identifier. `authentication` and
`admission` are the canonical V1 policy labels, not client hints: callers must
satisfy both policies when invoking the tool. Consumers should reject unknown
`schemaVersion` values rather than guessing their meaning. Tools that do not
map to a catalog route omit `iroha/routeAuth`.

Notes:

- A non-string, non-numeric, or out-of-range `cursor` is rejected as
  `invalid_cursor`.
- Effective page size is `max(1, torii.mcp.max_tools_per_list)`.
- `inputSchema` is sanitized before publication so clients always see a
  top-level object schema. Security-relevant `anyOf`, `oneOf`, `allOf`, and
  `not` constraints plus `if`/`then`/`else` branches are preserved and enforced
  before dispatch. OpenAPI references are recursively inlined; registry
  construction fails closed if an unresolved `$ref` remains.
- Every descriptor publishes standard `readOnlyHint`, `destructiveHint`,
  `idempotentHint`, and `openWorldHint` annotations. These are derived from the
  orthogonal semantic dimensions below, not from one combined effect label.
  Clients must still treat annotations as hints and obtain explicit approval
  for mutations.

Every descriptor also publishes versioned `_meta["iroha/semantics"]`:

```json
{
  "schemaVersion": 1,
  "operation": "mutate",
  "authority": "signed_body",
  "mutation": "may_reduce_remove_or_overwrite",
  "retry": "exact_identity_deduplicated",
  "world": "iroha_network",
  "sensitivity": "sensitive",
  "requiresExternalSignature": true
}
```

The independent fields have these values:

- `operation`: `observe`, `construct`, or `mutate`.
- `authority`: `public`, `listener_credential`, `dataspace_visible`, `account`,
  `signed_body`, `protocol_principal`, or `operator`.
- `mutation`: `none`, `additive_only`, or
  `may_reduce_remove_or_overwrite`.
- `retry`: `safe`, `exact_identity_deduplicated`, or `unsafe`.
- `world`: `torii_local`, `iroha_network`, or `external`.
- `sensitivity`: `normal` or `sensitive`.
- `requiresExternalSignature`: whether the caller must obtain a signature
  outside MCP.

Construct-only tools leave Torii and ledger state unchanged even when the
artifact they return is intended for a later signed mutation. A signed
transaction submission can be destructive while still being identity-
deduplicated. `openWorldHint` is true only when the tool can cross beyond the
configured Iroha network. This metadata is descriptive. For route-backed
tools, the route catalog and `iroha/routeAuth` remain the authoritative,
fail-closed admission contract. Explicit in-process capabilities do not
publish fabricated route-auth metadata; their registered backing, schema, and
handler policy are authoritative.

### `tools/call`

Executes one tool.

Params:

- `name` (required string)
- `arguments` (optional object)

### `tools/call_batch`

Executes multiple tool calls inside one JSON-RPC request. This is an Iroha
extension advertised by native `server/discover` and legacy `initialize`; it
is not JSON-RPC array batching.

On the native path, the caller must declare the extension on that same request
by using this object as
`params._meta["io.modelcontextprotocol/clientCapabilities"]`:

```json
{
  "extensions": {
    "org.hyperledger.iroha/tools": {}
  }
}
```

The legacy adapter retains its established batch behavior because it predates
formal per-request extension negotiation.

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

### `resources/list`

Returns Torii's complete fixed resource registry. The method accepts only the
standard native `_meta`; a `cursor` is rejected because the reviewed registry
does not paginate. `server/discover` advertises
`capabilities.resources.listChanged = false`, so no resource-list notification
or subscription lifecycle is implied.

The current registry contains:

- `iroha://node/health`
- `iroha://node/api-version`
- `iroha://chain/head`
- `iroha://chain/parameters` when Torii is compiled with `app_api`
- `iroha://runtime/abi/hash`

Each entry has `uri`, `name`, `title`, `description`, and
`mimeType = "application/json"`. The result is complete, has
`cacheScope = "private"`, and has a 30-second `ttlMs`. Clients should still use
the returned registry rather than hardcoding this list.

### `resources/read`

Reads one exact URI returned by `resources/list`. Params contain `uri` plus the
standard native `_meta`; the HTTP request also carries `Mcp-Name` mirroring the
URI exactly. An unknown URI is `invalid_params` rather than an arbitrary URL
fetch.

Resource reads dispatch a reviewed GET through Torii's existing in-process
router and share its ordinary outer admission, target-route authentication,
response-size bounds, and MCP dispatch semaphore. They perform no external
fetch and create no server, listener, sidecar, gateway, proxy, or network hop.
The complete result contains one JSON text entry under `contents` and private
per-resource cache hints. Health and chain-head reads have a zero TTL; other
resource TTLs are bounded by the registry.

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

### Canonical transaction artifacts

Two purpose-built capabilities make the external-signing boundary explicit:

- `iroha.transactions.prepare` accepts one non-empty canonical padded
  standard-Base64 `TransactionPayload` archive in
  `transaction_payload_base64`. It verifies ordinary construction rules,
  exact canonical re-encoding, and the serving Torii network id, then returns
  the unchanged canonical payload, its exact signing message and payload hash,
  and a bounded structural summary. It does not construct a semantic request,
  quote or rewrite fees, inspect current ledger state, simulate, sign, or
  submit.
- `iroha.transactions.inspect` accepts exactly one of
  `transaction_payload_base64` or `signed_transaction_base64`. It performs the
  same canonical and network checks; fixed-V1 signed wires additionally report
  canonical transaction identities, signature count, and a closed signature-
  verification result. Signature validity is not transaction admission or
  execution simulation.

Both capabilities execute as pure bounded helpers inside the existing Torii
MCP handler. They are explicitly marked as in-process catalogue entries, do
not pretend to be HTTP routes, do not re-enter the router, and create no new
server, listener, process, deployment, or network hop. Their inputs and
outputs never include a private key or detached signing secret.

The safe workflow is:

1. Obtain a canonical unsigned payload from a reviewed instruction/transaction
   builder and call `iroha.transactions.prepare`.
2. Review the returned summary and, when useful, call
   `iroha.transactions.inspect` on the same exact payload.
3. Sign the unchanged signing message outside MCP and assemble the canonical
   fixed-V1 `SignedTransaction` with an Iroha SDK or other reviewed signer.
4. Call `iroha.transactions.inspect` on that complete signed wire and require
   a valid signature result.
5. Submit the same wire through `iroha.transactions.submit_and_wait`.

Generic state-aware transaction simulation is not implemented by these tools.
It will require a reviewed, side-effect-free Core scratch-execution API; Torii
must not approximate or label structural inspection as simulation.

Streaming/internal paths are intentionally excluded from MCP tool generation
(for example SSE/WS stream routes and `/v1/mcp` itself). In addition to the
route-catalog decision, generated tools reject OpenAPI operations whose
response contract advertises `text/event-stream` or HTTP `101`, so streaming
exclusion does not depend on a path suffix.

Do not hardcode the full tool catalog in clients. Native clients may call
`server/discover` first or proceed directly to `tools/list`; compatibility
clients use `initialize` followed by `tools/list`.

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
Protocol and validation failures are returned in the top-level JSON-RPC
`error`. Standard modern failures are identified by their reserved top-level
code; Iroha application errors also carry a stable `error.data.error_code`.

Protocol-level JSON-RPC codes used by both eras:

- `-32700` -> `parse_error`
- `-32600` -> `invalid_request`
- `-32601` -> `method_not_found`
- `-32602` -> `invalid_params`

Native `2026-07-28` transport and application codes:

- `-32020` -> `header_mismatch`
- `-32021` -> `missing_required_client_capability`
- `-32022` -> `unsupported_protocol_version`
- `1001` -> tool lookup or admission failure
- `1002` -> `response_too_large`
- `1003` -> `request_timeout`
- `1004` -> `dispatch_capacity_exhausted`
- `1029` -> `rate_limited`

Legacy `2025-06-18` compatibility codes retain their established values,
including `-32004` for dispatch capacity and `-32029` for rate limiting.

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
- `invalid_resource_cursor`
- `resource_registry_invalid`
- `resource_route_failed`
- `resource_read_failed`

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

1. Choose native protocol `2026-07-28`; optionally call `server/discover`.
2. Call `tools/list` with native `_meta` and matching routing headers, then
   cache `_meta.iroha.toolsetVersion` for the advertised private TTL.
3. Call tools with `tools/call`, or use the advertised `tools/call_batch`
   extension when its per-dispatch charging is acceptable and declare the
   Iroha tools extension in that request's client capabilities.
4. When stable node context is useful, call `resources/list`, then
   `resources/read` for one returned URI and honor its private TTL.
5. Re-run `tools/list` when `_meta.iroha.listChanged` becomes `true` or its
   cache TTL expires.

Clients explicitly configured for a known `2025-06-18` endpoint may use the
compatibility sequence: `initialize`, `notifications/initialized`, then
versioned tool requests. Do not infer a legacy downgrade from a generic modern
transport, authentication, or protocol failure.

## Codex Plugin Workflow

This repo ships a Codex plugin bundle under `plugins/iroha/` that assumes the
target Torii host already exposes native MCP at `/v1/mcp`.

- Built-in preset: `https://taira.sora.org/v1/mcp`
- Custom deployments: add a user-local Codex MCP connection pointing to the
  existing Torii endpoint, for example
  `codex mcp add iroha-custom --url https://<torii>/v1/mcp`
- Standalone skill: this repo also ships `skills/sora-taira-testnet/` for the
  Codex Skills surface. Install it from a GitHub checkout of this repo with
  your local skill installer and restart Codex so it appears in the Skills tab.

The plugin does not parameterize `.mcp.json` and does not persist secrets. Keep
endpoint-specific bearer authentication in the user's local Codex MCP
configuration. Keep signing keys outside MCP and submit locally signed
transactions.

## Examples

Each native example below is sent with:

```text
Accept: application/json, text/event-stream
MCP-Protocol-Version: 2026-07-28
Mcp-Method: <the JSON-RPC method>
```

For `tools/call`, also send `Mcp-Name` with the exact tool name. For
`resources/read`, send it with the exact resource URI. Encode non-ASCII or
otherwise header-unsafe names and URIs with the canonical Base64 sentinel
described under Protocol Behavior.

### Discover Server

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "server/discover",
  "params": {
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientCapabilities": {},
      "io.modelcontextprotocol/clientInfo": {
        "name": "example-client",
        "version": "1.0.0"
      }
    }
  }
}
```

### List Tools (paged)
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list",
  "params": {
    "cursor": "0",
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientCapabilities": {},
      "io.modelcontextprotocol/clientInfo": {
        "name": "example-client",
        "version": "1.0.0"
      }
    }
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
    "arguments": {},
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientCapabilities": {}
    }
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
    },
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientCapabilities": {}
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
    ],
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientCapabilities": {
        "extensions": {
          "org.hyperledger.iroha/tools": {}
        }
      }
    }
  }
}
```

For this extension request, send `Mcp-Method: tools/call_batch`; `Mcp-Name` is
not used because the body names multiple tools. Omitting the matching client
extension declaration returns `-32021` rather than dispatching any item.

### List Resources

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "resources/list",
  "params": {
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientCapabilities": {}
    }
  }
}
```

### Read Resource

Send `Mcp-Name: iroha://runtime/abi/hash` with this request.

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "resources/read",
  "params": {
    "uri": "iroha://runtime/abi/hash",
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientCapabilities": {}
    }
  }
}
```

### Legacy Initialize

Clients explicitly configured for a known compatibility endpoint may initialize
with `protocolVersion = "2025-06-18"`. The initial request may omit the
protocol header; every later compatibility POST includes
`MCP-Protocol-Version: 2025-06-18`.
