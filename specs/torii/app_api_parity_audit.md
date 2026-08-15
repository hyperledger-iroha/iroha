## Torii App API Parity Audit (TORII-APP-1)

Status: Route inventory complete; typed canonical-witness construction remains an explicit SDK residual (updated 2026-08-14)
Owners: Torii Platform, SDK Program Lead  
Roadmap reference: TORII-APP-1 — `app_api` parity audit

### Scope & Method

The audit inspects the public re-exports in `crates/iroha_torii/src/lib.rs:256-522` and the
feature-gated route builders (`add_app_api_routes`, `add_contracts_and_vk_routes`,
`add_connect_routes`). For every `/v1/*` surface mentioned in the roadmap we verified:

- Handler implementation and DTO definitions in `crates/iroha_torii/src/routing.rs`.
- Router registration under the `app_api` or `connect` feature groups.
- Existing integration/unit tests and the owning team responsible for long-term coverage.

### Configuration

- SSE/webhook buffers are now driven by `torii.events_buffer_capacity` (broadcast channel depth) and
  `torii.webhook.{queue_capacity,max_attempts,backoff_initial_ms,backoff_max_ms,connect_timeout_ms,write_timeout_ms,read_timeout_ms}`
  for the delivery worker. These defaults remain conservative but can be tuned per deployment.
- App API pagination/backpressure honours `torii.app_api.{default_list_limit,max_list_limit,max_fetch_size,rate_limit_cost_per_row}`; account assets/transactions and asset-holder listings clamp `limit`/`fetch_size` and scale rate-limit cost by requested rows. The same endpoints accept an optional `asset_id` query parameter for pre-filtering.
- The push registration bridge is guarded by `torii.push.*` (feature `push`), enforcing `max_topics_per_device` and requiring FCM/APNS credentials before accepting device tokens.

### Auth & canonical signing

- Catalog-protected app-facing GET/POST endpoints require a complete canonical
  signature tuple (`X-Iroha-Account`, `X-Iroha-Signature`,
  `X-Iroha-Timestamp-Ms`, `X-Iroha-Nonce`) or its exclusive bounded witness
  alternative. The signature preimage is
  `iroha.app.request.network.v1\0 || exact_network_id_bytes || METHOD\n/path\nsorted_query\nsha256(body)\n<timestamp_ms>\n<nonce>`;
  Torii validates exact-network binding, freshness, and replay resistance
  before wrapping the request into `QueryRequestWithAuthority`. Public catalog
  routes do not require this proof.
- SDK builders keep canonical I105 as the semantic account spelling in paths
  and bodies, but transport that identity in `X-Iroha-Account` as lowercase
  canonical-address hex; an exact active canonical ASCII alias may be carried
  unchanged. Canonical form queries ignore empty `&` segments, preserve empty
  names and values, apply byte-precise lossy UTF-8 decoding, sort by UTF-8
  bytes, and use the application/x-www-form-urlencoded safe set.
- First-release network binding accepts only the exact genesis-derived
  `hash:<64 uppercase hex digits>#<4 uppercase CRC-16 digits>` `NetworkId`
  literal whose decoded 32-byte value carries the V1 marker bit. Canonical
  nonces contain 1--256 visible ASCII bytes (`0x21..0x7e`). Methods are
  non-empty ASCII HTTP tokens of at most 32 bytes, and SDK URI signers require
  the exact root-relative ASCII wire path of at most 64 KiB.
- SDK helpers ship in all primary clients:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, networkId, method, path, query, body, privateKey, timestampMs?, nonce? })` from `canonicalRequest.js`.
  - Swift: `CanonicalRequest.signingHeaders(accountId:networkId:method:path:query:body:signer:timestampMs:nonce:)` and `ToriiCanonicalRequest.buildHeaders(..., networkId: ...)`.
  - Android (Kotlin): `CanonicalRequestSigner.buildHeaders(networkId, method, uri, body, accountId, privateKey, timestampMs, nonce)`.
  - Android (Java): `CanonicalRequestSigner.buildHeaders(networkId, method, uri, body, canonicalAuth, timestampMs, nonce)`, where `canonicalAuth` is a `ToriiCanonicalRequestAuth` backed by a caller-owned signing callback.
- Residual witness surface: no client SDK yet exposes end-to-end typed witness
  construction. Rust can bounded-encode a completed witness; JavaScript and the
  `iroha_python` SoraFS reputation helpers can validate and forward an
  externally produced bounded canonical witness. The standalone
  `iroha_torii_client` and the typed mobile/C# helpers are intentionally
  signer-only.
  Kotlin and Java direct multisig writes to canonical signed transactions or
  closed typed signed intents. This is not malformed single-key V1 wire, but
  it remains an explicit API-parity gap until every affected SDK either ships
  typed bounded witness construction or documents a route-complete alternative.
- Example snippets use one syntactically valid fixture `NetworkId`; production
  callers must replace it with the exact genesis-derived identity of their
  deployment:
```ts
import { buildCanonicalRequestHeaders, NetworkId } from "@iroha/iroha-js";
const networkId = NetworkId.parse("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
const headers = buildCanonicalRequestHeaders({ accountId, networkId, method: "get", path: "/v1/node/capabilities", query: "", body: "", privateKey });
await fetch(`${torii}/v1/node/capabilities`, { headers });
```
```swift
let networkId = try NetworkId(literal: "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0")
let headers = try CanonicalRequest.signingHeaders(accountId: accountId,
                                                  networkId: networkId,
                                                  method: "get",
                                                  path: "/v1/node/capabilities",
                                                  query: "",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val networkId = NetworkId.parse(
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
)
val headers = CanonicalRequestSigner.buildHeaders(
    networkId,
    "get",
    URI.create("https://torii.example/v1/node/capabilities"),
    ByteArray(0),
    accountId,
    privateKey
)
```
```java
NetworkId networkId = NetworkId.parse(
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
ToriiCanonicalRequestAuth canonicalAuth =
    new ToriiCanonicalRequestAuth(accountId, signatureProvider);
long timestampMs = System.currentTimeMillis();
Map<String, String> headers = CanonicalRequestSigner.buildHeaders(
    networkId,
    "get",
    URI.create("https://torii.example/v1/node/capabilities"),
    new byte[0],
    canonicalAuth,
    timestampMs,
    "request-20260814-0001");
```
The Java `signatureProvider` is supplied by the application keystore or HSM and
must return a non-empty, non-zero detached signature no larger than the V1
3,309-byte ceiling.

### Endpoint Inventory

#### Account permissions (`/v1/accounts/{id}/permissions`) — Covered
- Handler: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: `crates/iroha_torii/tests/accounts_endpoints.rs:126` and `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Owner: Torii Platform.
- Notes: Response is a Norito JSON body with `items`/`total`, matching SDK pagination helpers.

- Notes: Response surface already enforces deterministic hex and backend identifiers; SDKs consume the DTO.

#### Proof events SSE (`GET /v1/events/sse`) — Covered
- Handler: `handle_v1_events_sse` with filter support (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) and proof filter wiring within the handler.
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: proof-specific SSE suites (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) and pipeline SSE smoke test
  (`integration_tests/tests/events/sse_smoke.rs`).
- Owner: Torii Platform (runtime), Integration Tests WG (fixtures).
- Notes: Proof filter paths validated end-to-end; documentation updated under `specs/zk_app_api.md`.

#### Contract lifecycle (`/v1/contracts/*`) — Covered
- Handlers: `handle_post_contract_call`,
  `handle_post_contract_call_multisig_propose`,
  `handle_post_contract_call_multisig_approve`,
  `handle_post_contract_view`,
  `handle_get_contract_code_bytes`.
- DTOs: `ContractCallDto`, `MultisigContractCallProposeDto`,
  `MultisigContractCallApproveDto`, and `ContractViewDto`.
- Router binding: `Torii::add_contracts_and_vk_routes`.
- Tests: `contracts_call_integration.rs` and related unit coverage for signed
  artifact reads, multisig, and view handling.
- Owner: Smart Contract WG with Torii Platform.
- Notes: Deployment is locally signed and enters through the standard native
  transaction pipeline. Runtime calls are by-reference `ContractCall`
  executions that accept exactly one of `contract_address` or
  `contract_alias`; Torii exposes no server-side deployment or activation
  shortcut. Contract-call and multisig DTOs accept either a detached public-key
  signature or return an unsigned preparation response; they never accept a
  private key. Alias writes return a canonical `AppApiTransactionDraftDto`
  payload for local signing.

#### Local signing boundary for app mutations — Covered
- Unsigned transaction drafts: contract aliases, verifying-key registration
  and updates, subscription plan and usage writes, and space-directory manifest
  publication and revocation.
- Signed-query envelopes: `POST /v1/proofs/query` accepts only canonical
  versioned `SignedQuery` bytes constrained to `FindProofRecordById`.
- All corresponding request DTOs use `deny_unknown_fields`, so a retired
  `private_key` member is rejected during extraction rather than ignored.
- Draft responses set `submitted: false` and return canonical padded-base64
  `TransactionPayload` bytes plus the exact payload-hash signing message. The
  caller validates, signs, constructs a `SignedTransaction`, and submits through
  the ordinary transaction endpoint.

#### Verifying key lifecycle (`/v1/zk/vk/*`) — Covered
- Handlers: `handle_post_vk_register`, `handle_post_vk_update`, and `handle_get_vk`.
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `AppApiTransactionDraftDto`, `VkListQuery`,
  and `ProofFindByIdQueryDto`.
- Router binding: `Torii::add_contracts_and_vk_routes`.
- Tests: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_vk_post_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Owner: ZK Working Group with Torii Platform support.
- Notes: The strict POST DTOs reject signing secrets and unknown fields. Torii returns a canonical
  unsigned transaction draft bound to the request and never signs or submits it; SDKs validate,
  sign, and submit locally. Rate limiting is enforced via `limits.rs`.

#### Nexus Connect (`/v1/connect/*`) — Covered (feature `connect`)
- Handlers: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Router binding: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Tests: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, session lifecycle, WS handshake) and
  router feature matrix coverage (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Owner: Nexus Connect WG.
- Notes: Rate limit keys tracked via `limits::rate_limit_key`; telemetry counters surfaced through connect metrics.

#### Push registration (`POST /v1/notify/devices`) — Covered (feature `push`)
- Handler: `handler_push_register_device` (`crates/iroha_torii/src/lib.rs:1528-1577`).
- DTOs: `RegisterDeviceRequest` (`crates/iroha_torii/src/push.rs:17-27`).
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:10518-10547`, feature `push`).
- Tests: unit coverage in `crates/iroha_torii/src/push.rs:68-120` and integration tests in `crates/iroha_torii/tests/push_bridge.rs`.
- Owner: Torii Platform.
- Notes: Returns `503` when the bridge is disabled or credentials are missing; applies per-account rate limiting and enforces `max_topics_per_device`.

#### Kaigi relay telemetry — Covered
- Handlers: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Router binding: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Tests: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notes: SSE stream reuses the global broadcast channel while enforcing
  telemetry profile gating; response schemas documented in
  `specs/torii/kaigi_telemetry_api.md:1`.

### Test Coverage Summary
- Router smoke tests (`crates/iroha_torii/tests/router_feature_matrix.rs`) ensure feature combinations register every
  route and that OpenAPI generation stays in sync.
- Endpoint-specific suites cover account queries, contract lifecycle, ZK verifying keys, SSE proof filters, and Nexus
  Connect behaviours.
- SDK parity harnesses (JavaScript, Swift, Python) already consume the supported alias and SSE endpoints; no additional work
  required.

### Open Actions
- Public, in-depth API guidance is maintained in `iroha-docs`; coordinate route
  changes there while keeping this repository-local audit source accurate.
