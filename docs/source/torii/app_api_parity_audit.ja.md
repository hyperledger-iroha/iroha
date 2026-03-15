---
lang: ja
direction: ltr
source: docs/source/torii/app_api_parity_audit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa393263d4ca430093f95c97f082c444ed08c90277a8537515dba04b65e7c6d3
source_last_modified: "2026-01-30T09:27:17.105195+00:00"
translation_last_reviewed: 2026-01-30
---

## Torii App API Parity Audit (TORII-APP-1)

Status: Completed 2026-03-21  
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

- App-facing GET/POST endpoints accept optional canonical request headers (`X-Iroha-Account`, `X-Iroha-Signature`) built from `METHOD\n/path\nsorted_query\nsha256(body)`; Torii wraps them into `QueryRequestWithAuthority` before executor validation so they mirror `/query`.
- SDK helpers ship in all primary clients:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` from `canonicalRequest.js`.
  - Swift: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Example snippets:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "i105...", method: "get", path: "/v1/accounts/i105.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/i105.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "i105...",
                                                  method: "get",
                                                  path: "/v1/accounts/i105.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("i105...", "get", "/v1/accounts/i105.../assets", "limit=5", ByteArray(0), signer)
```

### Endpoint Inventory

#### Account permissions (`/v1/accounts/{id}/permissions`) — Covered
- Handler: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: `crates/iroha_torii/tests/accounts_endpoints.rs:126` and `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Owner: Torii Platform.
- Notes: Response is a Norito JSON body with `items`/`total`, matching SDK pagination helpers.

#### Alias OPRF evaluate (`POST /v1/aliases/voprf/evaluate`) — Covered
- Handler: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Router binding: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Tests: inline handler tests (`crates/iroha_torii/src/lib.rs:9945-9986`) plus SDK coverage
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Owner: Torii Platform.
- Notes: Response surface already enforces deterministic hex and backend identifiers; SDKs consume the DTO.

#### Proof events SSE (`GET /v1/events/sse`) — Covered
- Handler: `handle_v1_events_sse` with filter support (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) and proof filter wiring within the handler.
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: proof-specific SSE suites (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) and pipeline SSE smoke test
  (`integration_tests/tests/events/sse_smoke.rs`).
- Owner: Torii Platform (runtime), Integration Tests WG (fixtures).
- Notes: Proof filter paths validated end-to-end; documentation updated under `docs/source/zk_app_api.md`.

#### Contract lifecycle (`/v1/contracts/*`) — Covered
- Handlers: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: router/integration suites `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Owner: Smart Contract WG with Torii Platform.
- Notes: Endpoints queue signed transactions and reuse shared telemetry metrics (`handle_transaction_with_metrics`).

#### Verifying key lifecycle (`/v1/zk/vk/*`) — Covered
- Handlers: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) and `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Owner: ZK Working Group with Torii Platform support.
- Notes: DTOs align with Norito schemas referenced by SDKs; rate limiting enforced via `limits.rs`.

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
  `docs/source/torii/kaigi_telemetry_api.md:1`.

### Test Coverage Summary
- Router smoke tests (`crates/iroha_torii/tests/router_feature_matrix.rs`) ensure feature combinations register every
  route and that OpenAPI generation stays in sync.
- Endpoint-specific suites cover account queries, contract lifecycle, ZK verifying keys, SSE proof filters, and Nexus
  Connect behaviours.
- SDK parity harnesses (JavaScript, Swift, Python) already consume Alias VOPRF and SSE endpoints; no additional work
  required.

### Open Actions
- ✅ Developer portal mirror published at `docs/portal/docs/reference/torii-app-api-parity.md`; keep both files in sync when routes change.
