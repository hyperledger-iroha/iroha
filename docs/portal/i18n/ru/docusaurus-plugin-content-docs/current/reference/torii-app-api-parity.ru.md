---
lang: ru
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: torii-app-api-parity
заголовок: Аудит паритета API приложения Torii
описание: Зеркало обзора TORII-APP-1, для команды SDK и платформы с надежным открытым покрытием.
---

Статус: Завершено 21.03.2026  
Владельцы: Платформа Torii, Руководитель программы SDK  
Ссылка в дорожную карту: TORII-APP-1 — аудит паритета `app_api`

На этой странице отражен внешний аудит `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`), чтобы читатели вне монорепозитория могли видеть, какие поверхности `/v1/*` подключены, протестированы и задокументированы. Аудит отслеживает маршруты, переэкспортируемые через `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` и `add_connect_routes`.

## Область и метод

Аудит впоследствии публичных переэкспортов в `crates/iroha_torii/src/lib.rs:256-522` и построителей маршрутов с функцией стробирования. Для каждой поверхности `/v1/*` в дорожной карте мы проверили:

- Реализация обработчика и определение DTO в `crates/iroha_torii/src/routing.rs`.
- Регистрация роутера в группах с функцией `app_api` или `connect`.
- Наличие интеграционных/юнит-тестов и команды, ответственной за долгосрочное покрытие.

Списки активов/транзакций аккаунта и состоятельных держателей активов принимают необязательные параметры запроса `asset_id` для предварительной фильтрации, за исключением существующих лимитов пагинации/обратного давления.

## Аутентификация и каноническая подпись

- GET/POST эндпойнты для приложений принимают опциональные заголовки стандартного запроса (`X-Iroha-Account`, `X-Iroha-Signature`), построенные из `METHOD\n/path\nsorted_query\nsha256(body)`; Torii оборачивает их в `QueryRequestWithAuthority` перед валидацией исполнителя, чтобы они назвали `/query`.
- Хелперы SDK доступны всем основным клиентам:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` из `canonicalRequest.js`.
  - Свифт: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Примеры:
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

## Инвентарь эндпойнтов

### Права аккаунта (`/v1/accounts/{id}/permissions`) — Открыто
- Обработчик: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Привязка роутера: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесты: `crates/iroha_torii/tests/accounts_endpoints.rs:126` и `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Владелец: Платформа Torii.
- Примечания: Ответ — Norito JSON с `items`/`total`, соответствует SDK хелперами пагинации.

### Псевдоним кредитоспособности OPRF (`POST /v1/aliases/voprf/evaluate`) — Открыто
- Обработчик: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Привязка роутера: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Тесты: встроенный обработчик тестов (`crates/iroha_torii/src/lib.rs:9945-9986`) плюс покрытие SDK.
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Владелец: Платформа Torii.
- Примечания: поверхность ответа принуждает определить шестнадцатеричный и идентификационный коды бэкэнда; SDK потребляет DTO.### Доказательство событий SSE (`GET /v1/events/sse`) — Открыто
- Обработчик: `handle_v1_events_sse` с фильтрами поддержки (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) плюс защита от проводного фильтра.
- Привязка роутера: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесты: доказательство-специфичные SSE сьюты (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) и курите тест SSE пайплайна
  (`integration_tests/tests/events/sse_smoke.rs`).
- Владелец: Torii Платформа (среда выполнения), WG Integration Tests (фиксации).
- Примечания: Маршруты доказательства фильтра проверены сквозным способом; документация в `docs/source/zk_app_api.md`.

### Жизненный цикл контрактов (`/v1/contracts/*`) — Открыто
- Обработчики: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  И18НИ00000089Х (И18НИ00000090Х),
  И18НИ00000091Х (И18НИ00000092Х),
  И18НИ00000093Х (И18НИ00000094Х),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Привязка роутера: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Тесты: маршрутизатор/интеграционные сьюты `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  И18НИ00000106Х, И18НИ00000107Х,
  `contracts_instances_list_router.rs`.
- Владелец: Smart Contract WG совместно с платформой Torii.
- Примечания: Эндпойнты поочередно передают подписанные транзакции и переиспользуют общие метрики телеметрии (`handle_transaction_with_metrics`).

### Жизненный цикл проверки ключей (`/v1/zk/vk/*`) — Открыто
- Обработчики: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) и `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Привязка роутера: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Тесты: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  И18НИ00000126Х,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Владелец: Рабочая группа ZK при поддержке платформы Torii.
- Примечания: соглашения DTO со схемами Norito, на которые ссылаются SDK; ограничение скорости применяется через `limits.rs`.

### Nexus Connect (`/v1/connect/*`) — Открыто (функция `connect`)
- Обработчики: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  И18НИ00000134Х (И18НИ00000135Х).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  И18НИ00000139Х (И18НИ00000140Х).
- Привязка роутера: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Тесты: `crates/iroha_torii/tests/connect_gating.rs` (функция стробирования, жизненный цикл сессии, WS-рукопожатие) и
  Функция покрытия матрицы маршрутизатора (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Владелец: Nexus Connect WG.
- Примечания: Ключи лимита скорости отслеживаются через `limits::rate_limit_key`; телеметрические счетчики питают метрики `connect.*`.

### Телеметрия реле Кайги — Покрыто
- Обработчики: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  И18НИ00000149Х, И18НИ00000150Х
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  И18НИ00000154Х, И18НИ00000155Х,
  И18НИ00000156Х (И18НИ00000157Х).
- Привязка роутера: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Тесты: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Примечания: поток SSE переиспользует глобальный широковещательный канал и применяет стробирующий профиль телеметрии; Схема ответов описана в `docs/source/torii/kaigi_telemetry_api.md`.

## Сводка покрытий тестами- Дымовые тесты маршрутизатора (`crates/iroha_torii/tests/router_feature_matrix.rs`) определяют, что особенность регистрирует каждый маршрут и генерацию OpenAPI остается синхронизированной.
- Эндпойнт-специфичные сьюты раскрывают запросы аккаунтов, жизненные циклические контракты, проверки ключей ZK, фильтры проверки SSE и поведение Nexus Connect.
- SDK-жгуты четности (JavaScript, Swift, Python) уже потребляют Alias ​​VOPRF и SSE эндпойнты; дополнительных работ не требуется.

##поддержание зеркала в актуальном состоянии

Обновляйте эту страницу и исходный аудит (`docs/source/torii/app_api_parity_audit.md`), когда изменится поведение API приложения Torii, чтобы держать SDK и внешние читатели после согласованных действий.