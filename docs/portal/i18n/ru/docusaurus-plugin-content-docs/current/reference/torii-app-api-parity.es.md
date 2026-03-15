---
lang: ru
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: torii-app-api-parity
название: Auditoria de paridad de la API de la app de Torii
описание: Добавлена версия TORII-APP-1 для подтверждения оборудования SDK и платформы.
---

Стадо: завершено 21 марта 2026 г.  
Ответственные: платформа Torii, руководитель программы SDK.  
Ссылка на дорожную карту: TORII-APP-1 — паритетная аудитория `app_api`

Эта страница отражает внутреннюю аудиторию `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) для того, чтобы лекторы в моно-репозитории могли видеть, что поверхностные сведения `/v1/*` находятся в кабелях, пробах и документах. Аудитория растреала руты, реэкспортированные по дорогам `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` и `add_connect_routes`.

## Alcance и метод

Аудитория проверяет публичный реэкспорт в `crates/iroha_torii/src/lib.rs:256-522` и рутинные работы с функцией стробирования. Для каждого суперфиция `/v1/*` проверенной дорожной карты:

— Реализация обработчика и определений DTO в `crates/iroha_torii/src/routing.rs`.
- Зарегистрируйте маршрутизатор для групп функций `app_api` или `connect`.
- Пруэбас-де-интеграция/унитариас существует и ответственное оборудование на большой площади.

Списки операций/транзакций операций и списки названий операций принимают параметры консультации `asset_id`, дополнительные параметры предварительной фильтрации, учитывая существующие ограничения по страницам/обратному давлению.

## Аутентификация и каноническая фирма

- Конечные точки GET/POST ориентированы на приложения, принимающие дополнительные канонические заголовки запроса (`X-Iroha-Account`, `X-Iroha-Signature`), построенные из `METHOD\n/path\nsorted_query\nsha256(body)`; Torii пересылает в `QueryRequestWithAuthority` перед проверкой исполнителя для отражения `/query`.
- Помощники SDK входят в число основных клиентов:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` для `canonicalRequest.js`.
  - Свифт: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Пример:
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

## Инвентарь конечных точек

### Разрешение на покупку (`/v1/accounts/{id}/permissions`) — Кубьерто
- Обработчик: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Привязка роутера: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесты: `crates/iroha_torii/tests/accounts_endpoints.rs:126` и `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Владелец: Платформа Torii.
- Примечания: ответ - это тело JSON Norito с `items`/`total`, которое совпадает с помощниками по страницам SDK.

### Оценка псевдонима OPRF (`POST /v1/aliases/voprf/evaluate`) — Кубьерто
- Обработчик: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`.
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Привязка роутера: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Тесты: встроенный обработчик (`crates/iroha_torii/src/lib.rs:9945-9986`) с использованием SDK.
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Владелец: Платформа Torii.
- Примечания: La superficie de respuesta impone hex deterministico e identificadores de backend; SDK потребляет DTO.### События доказательства SSE (`GET /v1/events/sse`) — Cubierto
- Обработчик: `handle_v1_events_sse` с фильтрами (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) для подключения фильтра проверки.
- Привязка роутера: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесты: наборы специальных доказательств SSE (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) и дымовой дым SSE del трубопровода
  (`integration_tests/tests/events/sse_smoke.rs`).
- Владелец: Torii Платформа (среда выполнения), WG Integration Tests (фиксации).
- Примечания: маршруты фильтров проверены на всем протяжении; живая документация в `docs/source/zk_app_api.md`.

### Цикл жизни контрактов (`/v1/contracts/*`) - Кубьерто
- Обработчики: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  И18НИ00000089Х (И18НИ00000090Х),
  И18НИ00000091Х (И18НИ00000092Х),
  И18НИ00000093Х (И18НИ00000094Х),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`.
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Привязка роутера: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Тесты: комплекты маршрутизаторов/интеграций `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  И18НИ00000106Х, И18НИ00000107Х,
  `contracts_instances_list_router.rs`.
- Владелец: Рабочая группа по смарт-контрактам с платформой Torii.
- Примечания: потеря конечных точек в фирменных транзакциях и повторное использование метрик, связанных с телеметрией (`handle_transaction_with_metrics`).

### Цикл проверки ключей (`/v1/zk/vk/*`) - Cubierto
- Обработчики: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) и `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`.
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Привязка роутера: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Тесты: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  И18НИ00000126Х,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Владелец: Рабочая группа ZK с поддержкой платформы Torii.
- Примечания: DTOs se alinean con los esquemas Norito, ссылающиеся на SDK; Ограничение скорости устанавливается через `limits.rs`.

### Nexus Connect (`/v1/connect/*`) — Cubierto (функция `connect`)
- Обработчики: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  И18НИ00000134Х (И18НИ00000135Х).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  И18НИ00000139Х (И18НИ00000140Х).
- Привязка роутера: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Тесты: `crates/iroha_torii/tests/connect_gating.rs` (функция стробирования, цикличность просмотра сеанса, квитирование WS) y
  список функций маршрутизатора (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Владелец: Nexus Connect WG.
- Примечания: Клавиши ограничения скорости устанавливаются через `limits::rate_limit_key`; контадоры телеметрии питаются метриками `connect.*`.### Телеметрия реле Кайги - Кубьерто
- Обработчики: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  И18НИ00000149Х, И18НИ00000150Х
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  И18НИ00000154Х, И18НИ00000155Х,
  И18НИ00000156Х (И18НИ00000157Х).
- Привязка роутера: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Тесты: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Примечания: поток SSE повторно использует глобальный канал вещания, используя шлюз доступа к данным телеметрии; Вопросы спасения документированы в `docs/source/torii/kaigi_telemetry_api.md`.

## Резюме кобертуры де прюбас

- Проверка маршрутизатора (`crates/iroha_torii/tests/router_feature_matrix.rs`) гарантирует, что комбинации функций будут зарегистрированы каждый раз, а генерация OpenAPI будет подтверждена при синхронизации.
- Специальные наборы конечных точек включают запросы квитанций, циклы просмотра контрактов, клавиши проверки ZK, фильтры проверки SSE и компоненты Nexus Connect.
- Использование парного SDK (JavaScript, Swift, Python) с использованием псевдонима VOPRF и конечных точек SSE; никаких дополнительных работ не требуется.

## Это актуальное сообщение

Актуализировать эту страницу и полную аудиторию (`docs/source/torii/app_api_parity_audit.md`), когда будет использован интерфейс API приложения Torii для владельцев SDK и внешних преподавателей, которые были добавлены.