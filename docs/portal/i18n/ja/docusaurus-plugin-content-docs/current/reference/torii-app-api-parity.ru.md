---
lang: ja
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-parity
タイトル: Аудит паритета API приложения Torii
説明: TORII-APP-1 の команды SDK と платформы могли подтвердить публичное покрытие。
---

Статус: Завербено 2026-03-21  
プロフィール: Torii プラットフォーム、SDK プログラム リード  
バージョン: TORII-APP-1 — `app_api`

Эта страница отражает внутренний аудит `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`), чтобы читатели вне монорепозитория могли Сидеть、какие поверхности `/v1/*` подключены、протестированы и задокументированы。 `Torii::add_app_api_routes`、`add_contracts_and_vk_routes`、`add_connect_routes` を参照してください。

## Область и метод

`crates/iroha_torii/src/lib.rs:256-522` にはゲート機能が備わっています。 `/v1/*` からのメッセージ:

- ハンドラーと DTO の `crates/iroha_torii/src/routing.rs`。
- 機能 `app_api` または `connect` を確認してください。
- Наличие интеграционных/юнит-тестов и команду, ответственную за долгосрочное покрытие.

Списки активов/транзакций аккаунта и списки держателей активов принимают необязательные query‑параметры `asset_id` для предварительной фильтрации, помимо существующих лимитов пагинации/обратного давления.

## Аутентификация и каноническая подпись

- GET/POST эндпойнты для приложений принимают опциональные заголовки канонического запроса (`X-Iroha-Account`, `X-Iroha-Signature`)、`METHOD\n/path\nsorted_query\nsha256(body)`; Torii оборачивает их в `QueryRequestWithAuthority` は、実行者、`/query` を実行します。
- SDK のアップデート:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` または `canonicalRequest.js`。
  - スイフト: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`。
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`。
- 説明:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "soraカタカナ...", method: "get", path: "/v1/accounts/i105.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/i105.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "soraカタカナ...",
                                                  method: "get",
                                                  path: "/v1/accounts/i105.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("soraカタカナ...", "get", "/v1/accounts/i105.../assets", "limit=5", ByteArray(0), signer)
```

## Инвентарь эндпойнтов

### Права аккаунта (`/v1/accounts/{id}/permissions`) — Покрыто
- ハンドラー: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)。
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)。
- ルーター バインディング: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- テスト: `crates/iroha_torii/tests/accounts_endpoints.rs:126` または `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`。
- 所有者: Torii プラットフォーム。
- 注: Ответ — Norito JSON с `items`/`total`, совпадает с SDK хелперами пагинации.

### OPRF оценка エイリアス (`POST /v1/aliases/voprf/evaluate`) — Покрыто
- ハンドラー: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)。
- DTO: `AliasVoprfEvaluateRequestDto`、`AliasVoprfEvaluateResponseDto`、`AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)。
- ルーター バインディング: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)。
- テスト: インライン тесты ハンドラー (`crates/iroha_torii/src/lib.rs:9945-9986`) SDK のテスト
  (`javascript/iroha_js/test/toriiClient.test.js:72`)。
- 所有者: Torii プラットフォーム。
- 注: Поверхность ответа принуждает детерминированный 16 進数とバックエンド。 SDK は DTO に対応します。

### 証明 SSE (`GET /v1/events/sse`) — Покрыто
- ハンドラー: `handle_v1_events_sse` は、 (`crates/iroha_torii/src/routing.rs:14008-14133`) を実行します。
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) 配線の証明。
- ルーター バインディング: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- テスト:proof-специфичные SSE сьюты (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`、
  `sse_proof_callhash.rs`、`sse_proof_verified_fields.rs`、`sse_proof_rejected_fields.rs`) と煙 тест SSE пайплайна
  (`integration_tests/tests/events/sse_smoke.rs`)。
- 所有者: Torii プラットフォーム (ランタイム)、統合テスト WG (フィクスチャ)。
- 注: エンドツーエンドの証明。 `docs/source/zk_app_api.md` を参照してください。

### Жизненный цикл контрактов (`/v1/contracts/*`) — Покрыто
- ハンドラー: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)、
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`)、
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`)、
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`)、
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)。
- DTO: `DeployContractDto`、`DeployAndActivateInstanceDto`、`ActivateInstanceDto`、`ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`)。
- ルーター バインディング: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- テスト: ルーター/統合 `contracts_deploy_integration.rs`、`contracts_activate_integration.rs`、
  `contracts_instance_activate_integration.rs`、`contracts_call_integration.rs`、
  `contracts_instances_list_router.rs`。
- 所有者: スマート コントラクト WG совместно с Torii プラットフォーム。
- メモ: Эндпойнты ставят подписанные транзакции в очередь и переиспользуют общие метрики телеметрии (`handle_transaction_with_metrics`)。

### Жизненный цикл ключей проверки (`/v1/zk/vk/*`) — Покрыто
- ハンドラー: `handle_post_vk_register`、`handle_post_vk_update`、`handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) または `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)。
- DTO: `ZkVkRegisterDto`、`ZkVkUpdateDto`、`ZkVkDeprecateDto`、`VkListQuery`、`ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)。
- ルーター バインディング: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- テスト: `crates/iroha_torii/tests/zk_vk_get_integration.rs`、
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`、
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`。
- 所有者: ZK ワーキング グループ、Torii プラットフォーム。
- 注: DTO согласованы со схемами Norito, на которые ссылаются SDK;レート制限が適用されました через `limits.rs`。### Nexus 接続 (`/v1/connect/*`) — Покрыто (機能 `connect`)
- ハンドラー: `handle_connect_session`、`handler_connect_session_delete`、`handle_connect_ws`、
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)。
- DTO: `ConnectSessionRequest`、`ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)、
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)。
- ルーター バインディング: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)。
- テスト: `crates/iroha_torii/tests/connect_gating.rs` (機能ゲーティング、жизненный цикл сессии、WS ハンドシェイク)
  機能 (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`) を確認してください。
- 所有者: Nexus WG に接続します。
- 注: レート制限 отслеживаются через `limits::rate_limit_key`; телеметрические счетчики питают метрики `connect.*`。

### 会議カイギ — Покрыто
- ハンドラー: `handle_v1_kaigi_relays`、`handle_v1_kaigi_relay_detail`、
  `handle_v1_kaigi_relays_health`、`handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)。
- DTO: `KaigiRelaySummaryDto`、`KaigiRelaySummaryListDto`、
  `KaigiRelayDetailDto`、`KaigiRelayDomainMetricsDto`、
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)。
- ルーターバインディング: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)。
- テスト: `crates/iroha_torii/tests/kaigi_endpoints.rs`。
- 注: SSE は、ブロードキャスト канал и применяет ゲート профиля телеметрии; `docs/source/torii/kaigi_telemetry_api.md` を参照してください。

## Сводка покрытия тестами

- スモーク тесты роутера (`crates/iroha_torii/tests/router_feature_matrix.rs`) гарантируют, что комбинации 機能 регистрируют каждый мардырут и генерация OpenAPI остается синхронизированной.
- Эндпойнт-специфичные сьюты покрывают запросы аккаунтов、жизненный цикл контрактов、ZK ключи проверки、SSE 証明Nexus 接続してください。
- SDK パリティ ハーネス (JavaScript、Swift、Python) は、エイリアス VOPRF および SSE に準拠しています。 дополнительной работы не требуется。

## Поддержание зеркала в актуальном состоянии

Обновляйте эту страницу исходный аудит (`docs/source/torii/app_api_parity_audit.md`)、когда меняется поведение Torii アプリ API、чтобы SDK と читатели оставались согласованными。