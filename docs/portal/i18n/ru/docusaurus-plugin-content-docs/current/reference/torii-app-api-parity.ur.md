---
lang: ru
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: torii-app-api-parity
title: Torii API-интерфейс API
описание: TORII-APP-1 Приложение для SDK, которое может быть использовано в качестве приложения. سکیں۔
---

Дата: Дата: 21 марта 2026 г.  
Имя: Torii Platform, руководитель программы SDK.  
Приложение: TORII-APP-1 — `app_api` برابری آڈٹ

یہ صفحہ اندرونی `TORII-APP-1` آڈٹ (`docs/source/torii/app_api_parity_audit.md`) کی کاسی کرتا ہے تاکہ مونو-ریپو Если вы хотите использовать `/v1/*`, вы можете использовать `/v1/*`. اور دستاویزی ہیں۔ Если вы хотите использовать `Torii::add_app_api_routes`, `add_contracts_and_vk_routes` и `add_connect_routes`, выберите нужный вариант. ذریعے دوبارہ ایکسپورٹ ہوتے ہیں۔

## دائرہ کار اور طریقہ

`crates/iroha_torii/src/lib.rs:256-522` может помочь вам с функцией стробирования функций, которая может быть отключена. ہے۔ Если у вас есть `/v1/*`, вы можете использовать следующую информацию:

- `crates/iroha_torii/src/routing.rs` — обработчик для работы с DTO
- `app_api` یا `connect` может быть использовано в качестве защитного средства.
- موجودہ انٹیگریشن/یونٹ ٹیسٹس اور طویل مدتی کوریج کے ذمہ دار ٹیم۔

موجودہ Нумерация страниц/обратное давление. Чтобы получить информацию о запросе `asset_id`, выполните следующие действия. ہیں۔

## تصدیق اور کینونیکل دستخط

- Для получения или отправки сообщений GET/POST (`X-Iroha-Account`, `X-Iroha-Signature`) کرتے ہیں جو `METHOD\n/path\nsorted_query\nsha256(body)` سے بنائے جاتے ہیں؛ Torii انہیں executor ویلیڈیشن سے پہلے `QueryRequestWithAuthority` میں لپیٹتا ہے تاکہ یہ `/query` کی عکاسی کریں۔
- SDK позволяет использовать дополнительные возможности:
  - JS/TS: `canonicalRequest.js` سے `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`.
  - Свифт: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Сообщение:
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

## اینڈپوائنٹ فہرست

### اکاؤنٹ اجازتیں (`/v1/accounts/{id}/permissions`) — کورڈ
- Обработчик: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Привязка роутера: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесты: `crates/iroha_torii/tests/accounts_endpoints.rs:126` или `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Владелец: Платформа Torii.
- Примечания: Norito JSON body и `items`/`total`, а также помощники разбиения на страницы SDK. رکھتا ہے۔

### Псевдоним OPRF تشخیص (`POST /v1/aliases/voprf/evaluate`) — کورڈ
- Обработчик: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`.
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Привязка роутера: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Тесты: обработчик и встроенные тесты (`crates/iroha_torii/src/lib.rs:9945-9986`) и SDK.
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Владелец: Платформа Torii.
- Примечания: например, детерминированные шестнадцатеричные идентификаторы серверной части и другие. Использование SDK и DTO### Доказательство SSE ایونٹس (`GET /v1/events/sse`) — کورڈ
- Обработчик: `handle_v1_events_sse` в случае необходимости (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) для проводки защитного фильтра.
- Привязка роутера: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Тесты: проверочные пакеты SSE (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) испытание трубопровода на дым SSE
  (`integration_tests/tests/events/sse_smoke.rs`).
- Владелец: Torii Платформа (среда выполнения), WG Integration Tests (фиксации).
- Примечания: фильтр проверки сквозной توثیق شدہ ہیں؛ دستاویزات `docs/source/zk_app_api.md` میں ہیں۔

### کنٹریکٹ لائف سائیکل (`/v1/contracts/*`) — کورڈ
- Обработчики: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  И18НИ00000089Х (И18НИ00000090Х),
  И18НИ00000091Х (И18НИ00000092Х),
  И18НИ00000093Х (И18НИ00000094Х),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`.
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Привязка роутера: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Тесты: комплекты маршрутизаторов/интеграции `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  И18НИ00000106Х, И18НИ00000107Х,
  `contracts_instances_list_router.rs`.
- Владелец: Smart Contract WG Платформа Torii.
- Примечания: اینڈپوائنٹس سائن شدہ ٹرانزیکشنز کو قطار میں ڈالते ہیں اور مشترکہ ٹیلیمیٹری میٹرکس (`handle_transaction_with_metrics`) для получения дополнительной информации

### ویریفائنگ کی لائف سائیکل (`/v1/zk/vk/*`) — کورڈ
- Обработчики: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) или `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`.
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Привязка роутера: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Тесты: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  И18НИ00000126Х,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Владелец: Рабочая группа ZK کے ساتھ Torii Платформа سپورٹ۔
- Примечания: Схемы DTO Norito и другие SDK могут быть использованы. Ограничение скорости `limits.rs` کے ذریعے نافذ ہے۔

### Nexus Connect (`/v1/connect/*`) — کورڈ (функция `connect`)
- Обработчики: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  И18НИ00000134Х (И18НИ00000135Х).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  И18НИ00000139Х (И18НИ00000140Х).
- Привязка роутера: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Тесты: `crates/iroha_torii/tests/connect_gating.rs` (функция стробирования, подтверждение соединения WS).
  Матрица функций маршрутизатора کوریج (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Владелец: Nexus Connect WG.
- Примечания: клавиши ограничения скорости `limits::rate_limit_key` могут быть отключены или отключены. ٹیلیمیٹری счетчики `connect.*`

### Реле Кайги ٹیلیمیٹری — کورڈ
- Обработчики: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  И18НИ00000149Х, И18НИ00000150Х
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  И18НИ00000154Х, И18НИ00000155Х,
  И18НИ00000156Х (И18НИ00000157Х).
- Привязка роутера: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Тесты: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Примечания: трансляция SSE-потока может быть выполнена в режиме стробирования или в режиме стробирования. کرتا ہے؛ схемы ответов `docs/source/torii/kaigi_telemetry_api.md` میں دستاویزی ہیں۔

## ٹیسٹ کوریج خلاصہ- Дымовые тесты маршрутизатора (`crates/iroha_torii/tests/router_feature_matrix.rs`). Доступны комбинации функций. Маршрутизация и синхронизация поколений OpenAPI. رہے۔
- Пакеты конечных точек, запросы, жизненный цикл контракта, проверка ключей ZK, фильтры SSE для проверки подлинности и Nexus Connect.
- Средства контроля четности SDK (JavaScript, Swift, Python) и псевдоним VOPRF для конечных точек SSE. اضافی کام درکار نہیں۔

## اس مرآۃ کو اپ ٹو ڈیٹ رکھنا

API приложения Torii может быть использован для создания приложения (`docs/source/torii/app_api_parity_audit.md`). Использование SDK для создания дополнительных приложений