---
lang: ru
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: torii-app-api-parity
заголовок: Аудит паритетного API приложения Torii
описание: Miroir de la revue TORII-APP-1 для SDK оборудования и формы пластины, подтверждающей публичную кувертюру.
---

Статут: Прекращение действия 21 марта 2026 г.  
Ответственные: платформа Torii, руководитель программы SDK.  
Ссылка на дорожную карту: TORII-APP-1 — паритетный аудит `app_api`

На этой странице отражен l'audit interne `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) для тех, кто читает лекции в моно-репозиториях, которые могут быть использованы на поверхностях `/v2/*` для кабелей, испытуемых и документированных лиц. Аудит подходит для реэкспортируемых маршрутов через `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` и `add_connect_routes`.

## Порт и метод

L'audit проверяет публичные реэкспорты в `crates/iroha_torii/src/lib.rs:256-522` и строителей маршрутов, которые имеют функцию шлюзования. Залейте поверхность шаком `/v2/*` du roadmap, проверьте:

— Реализация обработчика и определений DTO в `crates/iroha_torii/src/routing.rs`.
- Регистрация маршрута в группах функций `app_api` или `connect`.
- Тесты на интеграцию/объединение существующих и ответственных за кувертюр на длительный срок.

Списки операций/транзакций по счетам и списки задержек действий, принятые в соответствии с запрошенными параметрами `asset_id`, позволяют выполнять предварительную фильтрацию, а также существующие ограничения нумерации страниц/противодавления.

## Аутентификация и каноническая подпись

- Конечные точки GET/POST предоставляют дополнительные приложения, принимающие опции канонических заголовков (`X-Iroha-Account`, `X-Iroha-Signature`), образующие часть `METHOD\n/path\nsorted_query\nsha256(body)`; Torii конвертируется в `QueryRequestWithAuthority` перед проверкой исполнителя перед отражателем `/query`.
- Помощники SDK используются для всех основных клиентов:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` после `canonicalRequest.js`.
  - Свифт: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Примеры:
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

## Изобретение конечных точек

### Права доступа (`/v2/accounts/{id}/permissions`) – Couvert
- Обработчик: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Привязка роутера: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесты: `crates/iroha_torii/tests/accounts_endpoints.rs:126` и `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Владелец: Платформа Torii.
- Примечания: ответ представляет собой тело JSON Norito с `items`/`total`, соответствующее дополнительным помощникам по нумерации страниц SDK.

### Оценка OPRF d'alias (`POST /v2/aliases/voprf/evaluate`) - Couvert
- Обработчик: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`.
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Привязка роутера: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Тесты: тесты встроенного обработчика (`crates/iroha_torii/src/lib.rs:9945-9986`) плюс SDK couverture.
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Владелец: Платформа Torii.
- Примечания: ответная поверхность накладывает шестнадцатеричный определитель и идентификаторы серверной части; SDK соответствует DTO.### Проверка SSE (`GET /v2/events/sse`) – Couvert
- Обработчик: `handle_v1_events_sse` с поддержкой фильтров (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) плюс проводка для защиты от фильтра.
- Привязка роутера: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесты: наборы специальных доказательств SSE (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) и тестируйте дым SSE du трубопровода
  (`integration_tests/tests/events/sse_smoke.rs`).
- Владелец: Torii Платформа (среда выполнения), WG Integration Tests (фиксации).
- Примечания: Les chemins de filterproof sont valides de bout en bout; документация найдена в `docs/source/zk_app_api.md`.

### Цикл жизни контрактов (`/v2/contracts/*`) - Couvert
- Обработчики: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  И18НИ00000089Х (И18НИ00000090Х),
  И18НИ00000091Х (И18НИ00000092Х),
  И18НИ00000093Х (И18НИ00000094Х),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`.
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Привязка роутера: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Тесты: комплекты маршрутизатора/интеграции `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  И18НИ00000106Х, И18НИ00000107Х,
  `contracts_instances_list_router.rs`.
- Владелец: Smart Contract WG с платформой Torii.
- Примечания: конечные точки находятся в файлах подписанных транзакций и повторно используются метрики участников телеметрии (`handle_transaction_with_metrics`).

### Цикл проверки ключей (`/v2/zk/vk/*`) — Couvert
- Обработчики: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) и `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`.
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Привязка роутера: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Тесты: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  И18НИ00000126Х,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Владелец: Рабочая группа ZK с поддержкой платформы Torii.
- Примечания: DTO согласованы с схемами Norito, ссылаясь на SDK; Ограничение скорости устанавливается через `limits.rs`.

### Nexus Connect (`/v2/connect/*`) - Couvert (функция `connect`)
- Обработчики: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  И18НИ00000134Х (И18НИ00000135Х).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  И18НИ00000139Х (И18НИ00000140Х).
- Привязка роутера: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Тесты: `crates/iroha_torii/tests/connect_gating.rs` (функция стробирования, цикл жизни сеанса, рукопожатие WS) и др.
  крышка матрицы функций маршрутизатора (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Владелец: Nexus Connect WG.
- Примечания: ключи ограничения скорости можно контролировать через `limits::rate_limit_key`; счетчики телеметрии, питающиеся метриками `connect.*`.### Телеметрия реле Кайги - Куверт
- Обработчики: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  И18НИ00000149Х, И18НИ00000150Х
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  И18НИ00000154Х, И18НИ00000155Х,
  И18НИ00000156Х (И18НИ00000157Х).
- Привязка роутера: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Тесты: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Примечания: Le flux SSE повторно использует глобальный канал вещания и применяет шлюзование профиля телеметрии; Схемы ответа в документах `docs/source/torii/kaigi_telemetry_api.md`.

## Резюме тестов кувертюры

- Тесты Smoke du Router (`crates/iroha_torii/tests/router_feature_matrix.rs`) гарантируют, что комбинации функций зарегистрированы в маршруте и что поколение OpenAPI остается синхронизированным.
- Наборы конечных точек соответствуют запросам счетов, циклу жизни контрактов, файлам проверки ZK, фильтрам, подтверждающим SSE, и правилам Nexus Connect.
- Использование парного SDK (JavaScript, Swift, Python) с псевдонимом VOPRF и конечными точками SSE; aucun дополнительные требования.

## Garder ce miroir a jour

Перейдите на эту страницу и источник аудита (`docs/source/torii/app_api_parity_audit.md`), чтобы изменить поддержку приложения API Torii, чтобы владельцы SDK и внешние преподаватели остались в порядке.