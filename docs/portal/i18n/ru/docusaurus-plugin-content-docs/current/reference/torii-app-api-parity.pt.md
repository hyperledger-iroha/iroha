---
lang: ru
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: torii-app-api-parity
название: Аудитория paridade da API de app do Torii
описание: Выполнена пересмотренная версия TORII-APP-1 для того, чтобы оборудование SDK и платформа подтвердили публичное сообщение.
---

Статус: Заключение 21 марта 2026 г.  
Ответил: Torii Platform, руководитель программы SDK.  
Ссылка на дорожную карту: TORII-APP-1 - Audiria de Paridade `app_api`

Эта страница написана во внутренней аудитории `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) для тех, кто читает форумы для моно-репозитория, где есть поверхностное описание `/v1/*`, это подключенные, тестовые и документальные материалы. Аудитория сопровождает ротацию реэкспорта через `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` и `add_connect_routes`.

## Поиск и метод

Аудитория проверяет реэкспортные публикации в `crates/iroha_torii/src/lib.rs:256-522` и строители ротаций с функцией стробирования. Для каждого суперфиция `/v1/*` проверьте дорожную карту:

— Реализован обработчик и определенный DTO в `crates/iroha_torii/src/routing.rs`.
- Зарегистрируйте маршрутизаторы под номерами групп функций `app_api` или `connect`.
- Существующие интеграционные/унитарные тесты и средства, отвечающие за защиту долгого времени.

В качестве списков активных/транзакционных сообщений и заголовков активных параметров указаны параметры консультации `asset_id` для предварительной фильтрации, но существуют ограничения по страницам/противодавлению.

## Аутентикакао и каноническая ассинатура

- Конечные точки GET/POST изменяют заголовки приложений с опциями канонических требований (`X-Iroha-Account`, `X-Iroha-Signature`), созданными `METHOD\n/path\nsorted_query\nsha256(body)`; o Torii включает `QueryRequestWithAuthority` до подтверждения исполнителя для выполнения `/query`.
- Помощники SDK существуют во всех основных клиентах:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` от `canonicalRequest.js`.
  - Свифт: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Примеры:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "<katakana-i105-account-id>", method: "get", path: "/v1/accounts/<katakana-i105-account-id>/assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/<katakana-i105-account-id>/assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "<katakana-i105-account-id>",
                                                  method: "get",
                                                  path: "/v1/accounts/<katakana-i105-account-id>/assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("<katakana-i105-account-id>", "get", "/v1/accounts/<katakana-i105-account-id>/assets", "limit=5", ByteArray(0), signer)
```

## Инвентарь конечных точек

### Разрешения на контакт (`/v1/accounts/{id}/permissions`) — Коберто
- Обработчик: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Привязка роутера: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесты: `crates/iroha_torii/tests/accounts_endpoints.rs:126` и `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Владелец: Платформа Torii.
- Примечания: ответ на тело JSON Norito с `items`/`total`, добавленный в качестве помощников по страницам SDK.

### Доступ к псевдониму OPRF (`POST /v1/aliases/voprf/evaluate`) — Коберто
- Обработчик: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`.
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Привязка роутера: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Тесты: встроенный обработчик тестов (`crates/iroha_torii/src/lib.rs:9945-9986`) больше, чем SDK.
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Владелец: Платформа Torii.
- Примечания: поверхностный ответ на шестнадцатеричный детерминированный код и идентификаторы серверной части; Используемые SDK или DTO.### События доказательства SSE (`GET /v1/events/sse`) — Коберто
- Обработчик: `handle_v1_events_sse` с поддержкой фильтров (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) без подключения фильтра проверки.
- Привязка роутера: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесты: наборы специальных доказательств SSE (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) и тест дыма SSE do конвейер
  (`integration_tests/tests/events/sse_smoke.rs`).
- Владелец: Torii Платформа (среда выполнения), WG Integration Tests (фиксации).
- Примечания: Os caminhos de filtro deproof foram validados сквозной; документ, указанный в `docs/source/zk_app_api.md`.

### Ciclo de vida de contratos (`/v1/contracts/*`) - Коберто
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
- Владелец: Smart Contract WG com Torii Платформа.
- Примечания: конечные точки удаляют файлы транзакций и повторно используют метрики сравнения телеметрии (`handle_transaction_with_metrics`).

### Цикл жизни чавесов де верификакао (`/v1/zk/vk/*`) - Коберто
- Обработчики: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) и `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`.
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Привязка роутера: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Тесты: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  И18НИ00000126Х,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Владелец: Рабочая группа ZK при поддержке платформы Torii.
- Примечания: Os DTOs используются в схемах Norito, ссылающихся на SDK; Ограничение скорости доступно через приложение `limits.rs`.

### Nexus Connect (`/v1/connect/*`) — Коберто (функция `connect`)
- Обработчики: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  И18НИ00000134Х (И18НИ00000135Х).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  И18НИ00000139Х (И18НИ00000140Х).
- Привязка роутера: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Тесты: `crates/iroha_torii/tests/connect_gating.rs` (функция стробирования, цикличность просмотра, квитирование WS) e
  список функций маршрутизатора (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Владелец: Nexus Connect WG.
- Примечания: устанавливает ограничение скорости для рассрочки через `limits::rate_limit_key`; contadores de telemetria alimentam как метрики `connect.*`.

### Телеметрия реле Кайги - Коберто
- Обработчики: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  И18НИ00000149Х, И18НИ00000150Х
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  И18НИ00000154Х, И18НИ00000155Х,
  И18НИ00000156Х (И18НИ00000157Х).
- Привязка роутера: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Тесты: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Примечания: повторное использование потока SSE или глобального канала широковещательной передачи в приложении или пропуск для доступа к телеметрии; Наши схемы ответа являются документированными в `docs/source/torii/kaigi_telemetry_api.md`.## Резюме о кобертуре яичек

- Тесты дыма на маршрутизаторе (`crates/iroha_torii/tests/router_feature_matrix.rs`) гарантируют, что все функции будут зарегистрированы как ротации и что OpenAPI будет синхронизирован.
- Специальные наборы конечных точек содержат запросы, циклы договоров, проверки ZK, фильтры подтверждения SSE и протоколы Nexus Connect.
- Использование параллельного SDK (JavaScript, Swift, Python) с псевдонимом VOPRF и конечными точками SSE; nao ha trabalho adicional.

## Manter este espelho atualizado

Откройте эту страницу и шрифт аудитории (`docs/source/torii/app_api_parity_audit.md`) при использовании API приложения Torii, который будет использоваться владельцами SDK и внешними пользователями.