---
lang: he
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: torii-app-api-parity
כותרת: Аудит паритета API приложения Torii
תיאור: Зеркало обзора TORII-APP-1, чтобы команды SDK и платформы могли подтвердить публичное покрытие.
---

סטאטוס: Завершено 2026-03-21  
סמלים: Torii Platform, מוביל תוכנית SDK  
כרטיס חדש: TORII-APP-1 — אאודיט חלק `app_api`

Эта страница отражает внутренний аудит `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`), чтобы читатели вне монорепозивить какие поверхности `/v2/*` מוצרים, מוצרים ושירותים. Аудит отслеживает маршруты, переэкспортируемые через `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` ו-`add_connect_routes`.

## Область и метод

אבטחת שירותים ב-`crates/iroha_torii/src/lib.rs:256-522` ושירותים עם תכונות שערים. Для каждой поверхности `/v2/*` в дорожной карте мы проверили:

- מטפל Реализацию и определения DTO в `crates/iroha_torii/src/routing.rs`.
- מערכת הפעלה למשחק תכונה `app_api` או `connect`.
- Наличие интеграционных/юнит-тестов и команду, ответственную за долгосрочное покрытие.

Списки активов/транзакций аккаунта и списки держателей активов принимают необязательные query‑пара1мет00ые предварительной фильтрации, помимо существующих лимитов пагинации/обратного давления.

## Аутентификация и каноническая подпись

- GET/POST эндпойнты для приложений принимают опциональные заголовки канонического запроса (`X-Iroha-Account`, `X-Iroha-Account`, I010041X, I0100000041X, I0100000041X, построенные из `METHOD\n/path\nsorted_query\nsha256(body)`; Torii оборачивает их в `QueryRequestWithAuthority` перед валидацией executor, чтобы они соответствали OpenAPI.
- Хелперы SDK доступны во всех основных клиентах:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` או `canonicalRequest.js`.
  - סוויפט: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - אנדרואיד (קוטלין/ג'אווה): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- דגמים:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "i105...", method: "get", path: "/v2/accounts/i105.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v2/accounts/i105.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "i105...",
                                                  method: "get",
                                                  path: "/v2/accounts/i105.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("i105...", "get", "/v2/accounts/i105.../assets", "limit=5", ByteArray(0), signer)
```

## Инвентарь эндпойнтов

### Права аккаунта (`/v2/accounts/{id}/permissions`) — Покрыто
- מטפל: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- כריכת נתב: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- בדיקות: `crates/iroha_torii/tests/accounts_endpoints.rs:126` ו-`crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- בעלים: Torii Platform.
- הערות: Ответ — Norito JSON с `items`/`total`, совпадает с SDK хелперами пагинации.

### כינוי OPRF оценка (`POST /v2/aliases/voprf/evaluate`) — Покрыто
- מטפל: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- כריכת נתב: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- בדיקות: מטפל מוטבע ב-тесты (`crates/iroha_torii/src/lib.rs:9945-9986`) плюс SDK покрытие
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- בעלים: Torii Platform.
- הערות: Поверхность ответа принуждает детерминированный hex и идентификаторы backend; SDK потребляют DTO.### События proof SSE (`GET /v2/events/sse`) — Покрыто
- מטפל: `handle_v1_events_sse` с поддержкой фильтров (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) плюс חיווט фильтра הוכחה.
- כריכת נתב: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- בדיקות: proof-специфичные SSE сьюты (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) ועשן тест SSE пайплайна
  (`integration_tests/tests/events/sse_smoke.rs`).
- בעלים: פלטפורמת Torii (זמן ריצה), בדיקות אינטגרציה WG (מתקנים).
- הערות: Маршруты proof фильтра проверены מקצה לקצה; документация в `docs/source/zk_app_api.md`.

### Жизненный цикл контрактов (`/v2/contracts/*`) — Покрыто
- מטפלים: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- כריכת נתב: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- בדיקות: נתב/אינטגרציה сьюты `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- בעלים: Smart Contract WG совместно с Torii Platform.
- הערות: Эндпойнты ставят подписанные транзакции в очередь и переиспользуют общие метрики телеметри0и (0101090).

### Жизненный цикл ключей проверки (`/v2/zk/vk/*`) — Покрыто
- מטפלים: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) ו-`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- כריכת נתב: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- בדיקות: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- בעלים: ZK Working Group при поддержке Torii Platform.
- הערות: DTO согласованы со схемами Norito, на которые ссылаются SDK; הגבלת שיעור נאכף через `limits.rs`.

### Nexus Connect (`/v2/connect/*`) — Покрыто (תכונה `connect`)
- מטפלים: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- כריכת נתב: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- בדיקות: `crates/iroha_torii/tests/connect_gating.rs` (שער תכונה, жизненный цикл сессии, לחיצת יד WS) וכן
  покрытие матрицы תכונה роутера (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- בעלים: Nexus Connect WG.
- הערות: מגבלת שיעור Ключи отслеживаются через `limits::rate_limit_key`; телеметрические счетчики питают метрики `connect.*`.

### Телеметрия реле Kaigi — Покрыто
- מטפלים: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- כריכת נתב: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- בדיקות: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- הערות: SSE поток переиспользует глобальный broadcast канал и применяет gating профиля телеметрии; схемы ответов описаны в `docs/source/torii/kaigi_telemetry_api.md`.

## Сводка покрытия тестами- Smoke тесты роутера (`crates/iroha_torii/tests/router_feature_matrix.rs`) гарантируют, что комбинации תכונה регистрируют каждый маршрут и гене10ш00X и гене10ш00X остается синхронизированной.
- Эндпойнт-специфичные сьюты покрывают запросы аккаунтов, жизненный цикл контрактов, ZK клюктов, прильки при поведение Nexus Connect.
- רתמות זוגיות של SDK (JavaScript, Swift, Python) уже потребляют כינוי VOPRF ו-SSE эндпойнты; дополнительной работы не требуется.

## Поддержание зеркала в актуальном состоянии

Обновляйте эту страницу и исходный аудит (`docs/source/torii/app_api_parity_audit.md`), когда меняется поведение Torii אפליקציית API, чыдальиц внешние читатели оставались согласованными.