# Torii first-release API contract

Torii publishes one application API under `/v1`. Because this is the first
release, clients do not negotiate minor protocol versions and the server does
not carry compatibility routes or DTOs for earlier pre-release designs.

The former `x-iroha-api-version` request header and the API-version response
headers are not part of the contract. Torii ignores unknown request headers in
the usual HTTP manner. `/v1/api/versions` is unmounted, and proof endpoints are
available according to their authentication and rate-limit policies without a
separate minimum-version gate.

`GET /v1/api/version` remains a node-information endpoint. It reports the
active block-header/genesis version; it does not select an HTTP API variant.

## Request-target normalization

Route matching is strict and case-sensitive. Torii does not redirect or alias
trailing slashes. A non-root path ending in `/` is a typed `404
route_not_found`, including a wildcard route with an empty tail. Duplicate
slashes, raw backslashes, percent-encoded `/` or `\` (`%2f` or `%5c`, in any
hexadecimal case), and literal or percent-encoded dot segments are rejected as
`400 request_path_invalid` before a selected handler runs. These rules prevent
reverse proxies and application routers from assigning different meanings to
the same request target.

## Representation negotiation

Representation negotiation is independent of API evolution. Typed endpoints
support `application/json` and `application/x-norito`; JSON suffix media types
such as `application/problem+json` also select the JSON mapping. Media-type
parameters do not change the selected representation, so
`application/json; charset=utf-8` is JSON.

Torii evaluates each supported representation against the most-specific
matching `Accept` range: an exact type takes precedence over `application/*`,
which takes precedence over `*/*`. That range's `q` value is the effective
quality, including `q=0`; a wildcard cannot re-enable a representation that an
exact range forbids. Torii then compares effective quality, followed by
specificity. An equal explicit preference selects Norito. A wildcard-only tie
uses the endpoint default; typed Torii endpoints default to Norito. Missing
`Accept` also uses the endpoint default. Invalid or out-of-range quality values,
duplicate `q` parameters, quality values with more than three fractional
digits, or a request that permits neither representation return
`406 Not Acceptable`. Repeated `Accept` field lines are combined in wire order
as one comma-separated field before negotiation; the first equally specific
range determines a representation's effective quality. Unsupported media
ranges are ignored unless no supported representation remains.
Because no representation was successfully negotiated, the `406` itself uses
the shared typed error envelope in deterministic `application/json` form.
Negotiated responses include `Vary: Accept`.

This typed negotiation layer does not pre-empt protocol-native safe requests.
For `GET`, `HEAD`, and other safe methods, Torii lets the selected handler
advertise its actual response media type. If that type is JSON or Norito, Torii
still enforces the representation rules above before returning the response.
For SSE, Prometheus, raw content, and other native HTTP media, Torii validates
the actual response `Content-Type` against exact, type-wildcard, and global
wildcard ranges with the same specificity and `q=0` rules. WebSocket handlers
retain their upgrade and subprotocol rules because a successful upgrade has no
ordinary response `Content-Type`.
Unsafe commands are rejected with `406` before their handler runs, so an
unacceptable response preference can never produce a hidden state change.

The following examples are normative for typed endpoints whose default is
Norito:

| `Accept` | Result |
| --- | --- |
| omitted | `application/x-norito` |
| `application/json, application/x-norito` | `application/x-norito` (equal explicit preference) |
| `application/json;q=0.8, */*;q=0.9` | `application/x-norito` (JSON's exact range fixes its quality at 0.8) |
| `application/x-norito;q=0, */*;q=1` | `application/json` (the exact zero forbids Norito) |
| `application/*;q=0.7` | `application/x-norito` (wildcard-only tie uses the endpoint default) |
| `image/png` | `406` with a JSON `ErrorEnvelope` whose code is `response_not_acceptable` |

Typed request bodies require `Content-Type`. JSON and Norito use the media types
above; unsupported or missing types return `415 Unsupported Media Type`, while
an empty, malformed, or schema-invalid body under a supported type returns
`400 Bad Request`. Because `Content-Type` is a singleton field, repeated or
non-ASCII values return `400 request_content_type_invalid` before a command
handler runs. Request decoding never sniffs bytes and never falls back to
a whole-payload base64 wrapper. Negotiation failures, typed extractor
rejections, and handler errors use the shared typed error envelope whenever a
response representation can be selected. Router path and method misses are
typed application errors; protocol streams remain documented transport
exceptions after stream establishment.

## Errors and correlation

Application errors use the closed `ErrorEnvelope { code, message, details? }`
DTO in the selected JSON or Norito representation. `code` is the stable value
for SDK logic. It is 1–64 lowercase ASCII letters, digits, or underscores,
starts with a letter or digit, and is never a namespaced free-form string.
`message` is exact, non-empty human-readable text with no surrounding
whitespace or control characters; it may be reworded between builds. `details` is the
documented `ErrorDetails` record, not an arbitrary JSON object; it contains only
the reusable typed fields declared by the shared Torii model. Clients must
ignore detail fields they do not understand and must handle an unknown `code`
as a non-success without matching `message` text.

`details` is absent when an error has no structured context. When present, it
is a closed record whose optional members have these meanings:

| Member | Meaning |
| --- | --- |
| `layer` | bounded public layer that produced the error |
| `reject_code` | protocol or domain rejection identifier |
| `queue` | typed queue-depth/capacity snapshot |
| `retry_after_seconds` | whole-second retry delay, matching `Retry-After` when that header is present |
| `endpoint` | endpoint path or bounded endpoint group associated with the rejection; never a query string or credential |
| `field`, `expected`, `actual` | field-validation context; values must not contain secrets |
| `profile`, `chain_discriminant` | selected network-profile context |
| `tx_hash`, `last_status` | transaction/finality context |
| `hint` | human-readable remediation hint, not a stable SDK discriminator |
| `axt` | typed AXT rejection record |

No error code may place an undocumented object, array, or scalar behind
`details`. Adding a new detail member after `/v1` publication follows the DTO
evolution rules below; it is not an escape hatch for dynamic payloads.

One finite response-boundary rule enforces this contract for every ordinary
public, operator, diagnostic, router, authentication, rate-limit, timeout, and
handler `4xx` or `5xx` response. A response that already declares JSON or
Norito is preserved only when its body decodes as `ErrorEnvelope`; a bare
status, empty body, ad-hoc JSON object, malformed typed body, or error body
larger than 256 KiB is replaced with the generic envelope for its HTTP status.
The rejected body is not copied into the replacement message or details. An
invalid public `code` is likewise replaced with the generic status code; the
rejected identifier may be retained only in the typed `reject_code` member.
An otherwise decodable envelope with an invalid `message` retains its valid
stable code but receives the generic human-readable message for that HTTP
status.
Canonical error responses remove incompatible content encodings, advertise
their exact `Content-Length`, and carry `Vary: Accept`. A `HEAD` error advertises
the length of the corresponding typed representation but emits no body.

Only a catalogued protocol-surface response that already declares a non-JSON,
non-Norito media type bypasses body normalization. Its protocol-native error
body is preserved and telemetry records `protocol_native_error`. This exception
covers such contracts as an SSE establishment error; errors after a stream has
started follow that stream's terminal framing instead of the finite HTTP
envelope.

Offline readiness blocker messages follow the same exact human-text grammar as
error messages. Their blocker `code`, not their message, is the stable SDK
identifier.

Every HTTP response carries `X-Request-Id`. A client may supply an identifier
containing 1–128 ASCII letters, digits, `-`, `_`, `.`, or `:`; Torii echoes it.
Invalid or absent identifiers are replaced with a unique 64-character
lowercase hexadecimal identifier. Operators use this header to correlate
router, authentication, rate-limit, timeout, panic-boundary, and handler logs.
It is correlation metadata, not an authorization or idempotency token.

Framework failures use the same contract: `404` uses `route_not_found`, `405`
uses `method_not_allowed` and includes `Allow`, oversized typed bodies use
`413 request_payload_too_large`, timeouts use `408 request_timeout`, and an
unexpected panic is contained as `500 internal_server_error` without exposing
panic text. Authentication failures use `401 api_token_required` with
`WWW-Authenticate`; a node configured to require API tokens but containing no
tokens reports `503 api_token_unavailable`. Every canonical finite `429` or
`503` response includes both `Retry-After` and a matching typed retry hint.

## Reviewed protocol exceptions

These endpoints do not imply SDK or MCP generation. They are explicit media or
transport exceptions to the ordinary typed HTTP operation model. “Restricted
on the public listener” describes the current deployment topology: Torii has a
single listener today, and operator/diagnostic boundaries are enforced by
authentication and network policy rather than by claiming a separate socket.

| Method/path or family | Surface and exposure | Media/protocol | Authentication | Reason |
| --- | --- | --- | --- | --- |
| `GET /health` | protocol, public | framework health response | deployment policy | load-balancer probe |
| `GET /status`, `GET /status/{*tail}` | diagnostic, restricted on the public listener | diagnostic JSON/text | CIDR/API-token and network policy | infrastructure status namespace; never an SDK or MCP tool |
| `GET /metrics` | diagnostic, restricted on the public listener | Prometheus text | CIDR/API-token policy | scraper protocol; never an SDK or MCP tool |
| `GET /debug/pprof/profile` | diagnostic, restricted on the public listener | profiler bytes | CIDR/API-token policy | diagnostic artifact |
| `GET /openapi`, `GET /openapi.json`, `GET /v1/schema` | protocol documentation | JSON document | deployment policy | schema/document endpoints are JSON-only |
| `GET /v1/mcp`, `POST /v1/mcp` | protocol | capability document / MCP JSON-RPC | MCP/auth policy | tool transport, not ordinary generated REST operations |
| `POST /v1/operator/auth/{registration,login}/{options,verify}` | operator, OpenAPI only | WebAuthn JSON | mTLS plus handler-enforced bootstrap/session, rate-limit, lockout, and WebAuthn challenge policy | credential exchange cannot require an already-established operator request signature; it never enters SDK or MCP projections |
| `GET /v1/content/{bundle}/{*path}` and hosted-site reads | protocol | manifest-selected content type, ranges | content policy | raw/static content delivery; an empty wildcard is not a bundle-root alias |
| any method on `/soradns/{fqdn}`, `/soradns/{fqdn}/{*path}`, `/api`, or `/api/{*tail}` | protocol | proxied SoraCloud HTTP runtime | protocol handshake and gateway policy | reviewed public-runtime gateways; not OpenAPI, SDK, or MCP operations |
| query-projection, attachment, and SoraFS export reads documented as binary | operator/protocol | `application/octet-stream` or declared artifact media | route policy | exact binary artifacts |
| `/v1/events/sse`, `/v1/contracts/events/sse`, the documented `*/events/stream` and explorer/governance SSE routes | protocol stream | `text/event-stream` | route policy | long-lived SSE cannot switch to an HTTP error envelope after establishment |
| `/v1/events/ws`, `/v1/blocks/stream`, `/p2p`, `/v1/connect/ws`, and documented `*/events/ws` routes | protocol stream | WebSocket upgrade/subprotocol | route policy | bidirectional framed protocol |
| Norito-RPC ingress | protocol | Norito-RPC framing | staged ingress policy | RPC transport has its own gate errors and retry contract |

The canonical event/block handshake, ordering, heartbeat, lag, reconnect, and
close-code rules are specified in [the Torii streaming contract](streaming.md).

## Typed JSON mapping

The JSON representation is the Norito JSON mapping of the same public DTO, not
an independently evolving compatibility schema. Struct members use their
declared snake-case names. Adjacently tagged enums contain the declared tag and
content members; a unit variant has a `null` content value. Byte representation
is field-level schema, not a runtime guess. The unannotated Norito mapping of a
fixed `[u8; N]` is a JSON string containing exactly `2 * N` uppercase
hexadecimal digits; its decoder accepts uppercase or lowercase hexadecimal of
exactly that length. A field that explicitly selects the `fixed_bytes` helper
instead emits exactly `N` integer bytes, and an ordinary `Vec<u8>` is likewise
an integer-byte array unless its field selects a documented textual helper.
This field-level encoding is distinct from the forbidden whole-payload base64
wrapper. Hashes, keys, signatures, identifiers, and numeric values use the
mapping declared by their data-model type.

A map is a JSON object. Its public key type must declare an injective canonical
JSON-string mapping and the inverse map-key decoder. Decoding rejects two
different lexical keys that normalize to the same typed key. A key type without
such a mapping is not permitted in a public DTO; the DTO must instead use a
documented array of typed key/value entries or an explicit field helper. JSON
object ordering has no semantic meaning, although canonical Torii output is
deterministic for ordered maps.

Unsigned 64-bit and 128-bit values are emitted as unquoted, lossless decimal
JSON integers. Clients must use an integer representation that preserves their
full range and must not round them through an IEEE-754 `double`. Optional fields
encode as either their typed value or `null` unless the DTO explicitly declares
that `None` is omitted. Decoders reject duplicate declared members, unknown
enum discriminator values, out-of-range integers, and non-finite
numbers. They ignore unknown object members so an independently produced JSON
object can carry unrelated metadata without changing the typed value. For a
unit enum variant, decoders accept either an explicit `null` content member or
an omitted content member; canonical Torii output always includes the explicit
`null`. Signatures and payload digests cover canonical typed Norito bytes; the
lexical spelling, member order, or ignored members of input JSON are never
signed.

## Route catalog projections

The catalog is the canonical route superset, but consumers receive explicit
projections rather than identical sets. Each descriptor identifies its public,
operator, diagnostic, or protocol surface; listener; feature gate; OpenAPI and
SDK exposure; and explicit MCP allowlisting. Metrics, diagnostics, streams, and
privileged commands therefore do not become SDK operations or MCP tools merely
because they are mounted. The mounted projection is evaluated for the active
build, while the SDK projection is the canonical supported-build superset.
OpenAPI-derived MCP generation is fail-closed: an operation is emitted only
when its exact HTTP method/path pair is enabled in the catalog's MCP projection
for the compiled feature set. An uncatalogued operation, a catalogued operation
without the MCP bit, or a feature-disabled operation is not a tool. Purpose-built
`iroha.*` tools are a separately reviewed, explicit allowlist and remain subject
to read-only/writer/operator policy. `tools/call` accepts only exact names from
`tools/list`; OpenAPI `operationId` values and retired convenience spellings are
not hidden aliases. Listener metadata records the intended exposure boundary,
while the current single-listener implementation continues to enforce operator
and diagnostic restrictions through authentication and ingress policy.

## HTTP observability

Every matched request carries the catalog's stable route ID, Axum route
template, API surface, listener, and tooling projections in request extensions.
The same metadata is copied to the response for outer middleware. Neither logs
nor metric labels fall back to the raw request URI; concrete identifiers,
cursor values, and query strings therefore cannot create label cardinality or
leak through route labels. Unmatched requests use `http.route_not_found` and
the fixed template `unmatched`.

HTTP metrics include stable route ID/template, surface, bounded representation,
stable error code, method, status, latency, and exact request/response body
sizes when known. JSON and Norito retain their established content-type labels;
all other response content types are reduced to a bounded media category.
Typed `ErrorEnvelope` responses copy their validated code into an internal
response extension. Reviewed non-typed protocol errors use
`error_code="protocol_native_error"`. An ordinary failed response can be
reported as `error_code="unclassified"` only if code outside the assembled
Torii router bypasses the normalization boundary; that is an actionable
contract violation, not a supported response form.

## `/v1` evolution

Until the first release is published, incompatible API changes replace the
pre-release contract directly; there are no aliases or migration DTOs. After
publication, the typed Norito layouts and their JSON mappings are frozen for
the lifetime of `/v1`. Reordering, adding, removing, or retyping a field, adding
a variant to a closed enum, changing requiredness, or changing a field's JSON
mapping requires a new major path unless the type already declares an explicit
extensibility mechanism. Adding an independent route or a new stable error-code
string is additive. Clients must handle unknown error codes without assuming
that human-readable messages are stable. Adding a required request field is
never compatible within `/v1`.

Release tests pin both the public schema-name header hash and the canonical
structural-schema hash for proof-bearing request DTOs. A refactor that changes
field order, type identity, nesting, or representation therefore fails before
an incompatible binary layout can be published under an unchanged `/v1`
schema name.

Internal wire and consensus types may keep implementation version suffixes.
Those names do not create nested route versions and do not provide a second
public response schema.

## Offline operation locality

Both Offline command routes use the configured Torii `max_content_len` request
limit, the same operator-controlled ceiling used for transaction ingress. They
do not inherit Axum's smaller framework default: a semantic-lineage redemption
that is within the configured limit reaches typed decoding. A streamed or
declared body that exceeds the configured limit fails with typed `413` code
`request_payload_too_large` before command admission.

Offline command idempotency is globally bound by the signed `operation_id`, but
pre-commit coordination is deliberately instance-local in this first release.
The in-flight coordinator, admission cache, and transaction queue belong to the
Torii instance that accepted the command. A load balancer must therefore keep a
client on that instance from submission until the operation commits. A pending
lookup sent to a different instance can return `404`; that response never
authorizes the client to recycle or change the operation id.

Every Torii replica allowed to accept Offline commands for one deployment must
use the same Offline issuer identity and behaviorally identical issuer policy.
Given the same signed request, those replicas consequently construct the same
signed transaction. A cross-instance race can still admit the same candidate
more than once into independent local queues, so the consensus/on-chain
`operation_id` uniqueness rule is the final guard that permits at most one
economic effect. This is not a distributed idempotency-cache guarantee.

After commit, synchronized replicas recover the terminal result through Kura's
operation-id index while the indexed block body is retained. An index still
being reconstructed returns typed `503`, and a replica that does not retain the
indexed block returns the documented history-unavailable `503`. Deployments
that cannot provide pre-commit affinity must not expose Offline command routes
until shared admission coordination exists.

## Sharp cutover and release gates

External atomicity means the release exposes only the canonical route tree.
Implementation may be staged internally, but no compatibility router, alias,
redirect, fallback decoder, migration DTO, or dual-write path is mounted in the
release. Because no earlier public release exists, there is no old-to-new
migration manifest or supported migration period. A reviewed negative-path
inventory exists only to prove that known pre-release spellings cannot resolve
through redirects, normalization, wildcard capture, or another method.

The following are mandatory release gates; a failure blocks publication:

1. The catalog validates, and the mounted method/path set equals the enabled
   mounted projection including reviewed implicit `HEAD` and CORS `OPTIONS`
   behavior. Each mounted authentication policy is exercised against the
   middleware/handler that enforces it.
2. OpenAPI equals the enabled OpenAPI projection, MCP contains only its enabled
   explicit allowlist, and generated SDK operations equal the canonical SDK
   projection. Generated artifacts must be reproducible and clean.
3. Adversarial route tests cover known retired spellings, trailing and duplicate
   slashes, case changes, percent-encoded separators and dot segments, wildcard
   capture, wrong methods, and framework redirects.
4. JSON/Norito golden vectors and structural-schema guards, negotiation and
   typed-error tests, offline idempotency/lifecycle tests, cursor
   snapshot/authorization/lifetime tests, and streaming establishment/lag/
   backpressure tests all pass.
5. Every supported first-party SDK passes its contract, fixture, and generated
   surface suites against the same release artifacts.
6. The full Cargo workspace build, test, format, strict lint, serialization
   guard, and generated-artifact suites pass. Focused suites may run earlier,
   but they never replace the full release matrix.

Rollback selects an earlier unpublished build or deployment artifact; it does
not reactivate retired routes in the release binary.
