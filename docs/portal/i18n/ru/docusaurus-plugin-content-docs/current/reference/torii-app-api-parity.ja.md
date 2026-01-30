---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/reference/torii-app-api-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f17e9ea4094de90d827d510af33825dcc773cb578c2f7fed4a72add9ef7d741c
source_last_modified: "2025-11-14T04:43:21.010473+00:00"
translation_last_reviewed: 2026-01-30
---

Статус: Завершено 2026-03-21  
Владельцы: Torii Platform, SDK Program Lead  
Ссылка в дорожной карте: TORII-APP-1 — аудит паритета `app_api`

Эта страница отражает внутренний аудит `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`), чтобы читатели вне монорепозитория могли видеть, какие поверхности `/v1/*` подключены, протестированы и задокументированы. Аудит отслеживает маршруты, переэкспортируемые через `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` и `add_connect_routes`.

## Область и метод

Аудит проверяет публичные переэкспорты в `crates/iroha_torii/src/lib.rs:256-522` и построители маршрутов с feature gating. Для каждой поверхности `/v1/*` в дорожной карте мы проверили:

- Реализацию handler и определения DTO в `crates/iroha_torii/src/routing.rs`.
- Регистрацию роутера в группах feature `app_api` или `connect`.
- Наличие интеграционных/юнит-тестов и команду, ответственную за долгосрочное покрытие.

Списки активов/транзакций аккаунта и списки держателей активов принимают необязательные query‑параметры `asset_id` для предварительной фильтрации, помимо существующих лимитов пагинации/обратного давления.

## Аутентификация и каноническая подпись

- GET/POST эндпойнты для приложений принимают опциональные заголовки канонического запроса (`X-Iroha-Account`, `X-Iroha-Signature`), построенные из `METHOD\n/path\nsorted_query\nsha256(body)`; Torii оборачивает их в `QueryRequestWithAuthority` перед валидацией executor, чтобы они соответствовали `/query`.
- Хелперы SDK доступны во всех основных клиентах:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` из `canonicalRequest.js`.
  - Swift: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Примеры:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "ih58...", method: "get", path: "/v1/accounts/ih58.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/ih58.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "ih58...",
                                                  method: "get",
                                                  path: "/v1/accounts/ih58.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("ih58...", "get", "/v1/accounts/ih58.../assets", "limit=5", ByteArray(0), signer)
```

## Инвентарь эндпойнтов

### Права аккаунта (`/v1/accounts/{id}/permissions`) — Покрыто
- Handler: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: `crates/iroha_torii/tests/accounts_endpoints.rs:126` и `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Owner: Torii Platform.
- Notes: Ответ — Norito JSON с `items`/`total`, совпадает с SDK хелперами пагинации.

### OPRF оценка alias (`POST /v1/aliases/voprf/evaluate`) — Покрыто
- Handler: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Router binding: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Tests: inline тесты handler (`crates/iroha_torii/src/lib.rs:9945-9986`) плюс SDK покрытие
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Owner: Torii Platform.
- Notes: Поверхность ответа принуждает детерминированный hex и идентификаторы backend; SDK потребляют DTO.

### События proof SSE (`GET /v1/events/sse`) — Покрыто
- Handler: `handle_v1_events_sse` с поддержкой фильтров (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) плюс wiring фильтра proof.
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: proof-специфичные SSE сьюты (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) и smoke тест SSE пайплайна
  (`integration_tests/tests/events/sse_smoke.rs`).
- Owner: Torii Platform (runtime), Integration Tests WG (fixtures).
- Notes: Маршруты proof фильтра проверены end-to-end; документация в `docs/source/zk_app_api.md`.

### Жизненный цикл контрактов (`/v1/contracts/*`) — Покрыто
- Handlers: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: router/integration сьюты `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Owner: Smart Contract WG совместно с Torii Platform.
- Notes: Эндпойнты ставят подписанные транзакции в очередь и переиспользуют общие метрики телеметрии (`handle_transaction_with_metrics`).

### Жизненный цикл ключей проверки (`/v1/zk/vk/*`) — Покрыто
- Handlers: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) и `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Owner: ZK Working Group при поддержке Torii Platform.
- Notes: DTO согласованы со схемами Norito, на которые ссылаются SDK; rate limiting enforced через `limits.rs`.

### Nexus Connect (`/v1/connect/*`) — Покрыто (feature `connect`)
- Handlers: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Router binding: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Tests: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, жизненный цикл сессии, WS handshake) и
  покрытие матрицы feature роутера (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Owner: Nexus Connect WG.
- Notes: Ключи rate limit отслеживаются через `limits::rate_limit_key`; телеметрические счетчики питают метрики `connect.*`.

### Телеметрия реле Kaigi — Покрыто
- Handlers: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Router binding: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Tests: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notes: SSE поток переиспользует глобальный broadcast канал и применяет gating профиля телеметрии; схемы ответов описаны в `docs/source/torii/kaigi_telemetry_api.md`.

## Сводка покрытия тестами

- Smoke тесты роутера (`crates/iroha_torii/tests/router_feature_matrix.rs`) гарантируют, что комбинации feature регистрируют каждый маршрут и генерация OpenAPI остается синхронизированной.
- Эндпойнт-специфичные сьюты покрывают запросы аккаунтов, жизненный цикл контрактов, ZK ключи проверки, SSE proof фильтры и поведение Nexus Connect.
- SDK parity harnesses (JavaScript, Swift, Python) уже потребляют Alias VOPRF и SSE эндпойнты; дополнительной работы не требуется.

## Поддержание зеркала в актуальном состоянии

Обновляйте эту страницу и исходный аудит (`docs/source/torii/app_api_parity_audit.md`), когда меняется поведение Torii app API, чтобы владельцы SDK и внешние читатели оставались согласованными.
