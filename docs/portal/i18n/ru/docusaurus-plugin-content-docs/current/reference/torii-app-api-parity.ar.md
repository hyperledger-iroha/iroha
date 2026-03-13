---
lang: ru
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: torii-app-api-parity
Название: تدقيق تكافؤ واجهة تطبيق Torii
описание: Установленное приложение TORII-APP-1, созданное в SDK и установленное в приложении. العامة.
---

Сообщение: Дата: 21 марта 2026 г.  
Место: Torii Platform, руководитель программы SDK.  
Приложение создано: TORII-APP-1 — добавлено `app_api`.

Установите флажок `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`). Пожалуйста, проверьте, как работает `/v2/*`. Для этого используйте `Torii::add_app_api_routes` и `add_contracts_and_vk_routes` и `add_connect_routes`.

## النطاق والمنهج

Для получения дополнительной информации обратитесь к `crates/iroha_torii/src/lib.rs:256-522`. المحمية بالميزات. На странице `/v2/*` в разделе "Программы" написано:

- Выполнено обновление DTO для `crates/iroha_torii/src/routing.rs`.
- Установите флажок `app_api` или `connect`.
- اختبارات التكامل/الوحدة الموجودة والفريق المسؤول عن التغطية طويلة الاجل.

قوائم أصول/معاملات الحساب وقوائم حاملي الأصول تقبل معاملات استعلام `asset_id` Он был создан для того, чтобы сделать это, а также для того, чтобы сделать это.

## المصادقة والتوقيع القياسي

- Отправьте запрос GET/POST для отправки запроса на отправку сообщения. (`X-Iroha-Account`, `X-Iroha-Signature`) Добавлено `METHOD\n/path\nsorted_query\nsha256(body)`; Для Torii используется `QueryRequestWithAuthority` для запуска исполнителя `/query`.
- Добавлено в SDK в разделе "Служба безопасности":
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` от `canonicalRequest.js`.
  - Свифт: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- Ответ:
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

## جرد نقاط النهاية

### اذونات الحساب (`/v2/accounts/{id}/permissions`) — مغطى
- Код: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Код: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Код: `crates/iroha_torii/tests/accounts_endpoints.rs:126` и `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Сообщение: Torii Платформа.
- Код: Загрузка в формате JSON Norito и `items`/`total`, а также в формате JSON Norito. Доступно изменение в SDK.

### تقييم OPRF للاسماء المستعارة (`POST /v2/aliases/voprf/evaluate`) — مغطى
- Код: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`.
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Код: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Обновление: Встроенный встроенный файл (`crates/iroha_torii/src/lib.rs:9945-9986`) Добавление встроенного SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Сообщение: Платформа Torii.
- Значение: шестнадцатеричное числовое значение для серверной части; Используйте SDK для DTO.

### احداث доказательство عبر SSE (`GET /v2/events/sse`) — مغطى
- Код: `handle_v1_events_sse` в случае необходимости (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) для проверки подлинности.
- Код: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Ошибка: SSE خاصة بالproof (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) واختبار Smoke لSSE في خط الانابيب
  (`integration_tests/tests/events/sse_smoke.rs`).
- Добавлено: Torii Платформа (среда выполнения), WG Integration Tests (фикстуры).
- Сообщение: تم التحقق من مسارات فلتر доказательство طرفا لطرف؛ Создан для `docs/source/zk_app_api.md`.### دورة حياة العقود (`/v2/contracts/*`) — مغطى
- Код: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  И18НИ00000089Х (И18НИ00000090Х),
  И18НИ00000091Х (И18НИ00000092Х),
  И18НИ00000093Х (И18НИ00000094Х),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`.
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Код: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Дополнительно: маршрутизатор/интеграция `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  И18НИ00000106Х, И18НИ00000107Х,
  `contracts_instances_list_router.rs`.
- Сообщение: Рабочая группа по смарт-контрактам на платформе Torii.
- Видео: Вспомните, что произошло в 2017 году. Установите флажок для проверки подлинности (`handle_transaction_with_metrics`).

### دورة حياة مفاتيح التحقق (`/v2/zk/vk/*`) — مغطى
- Коды: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`.
  (`crates/iroha_torii/src/routing.rs:4282-4382`) و`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`.
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Код: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Код: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  И18НИ00000126Х,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Сообщение: Рабочая группа ZK создала платформу Torii.
- Добавлено: добавление DTO в Norito для установки SDK; Установлено ограничение скорости `limits.rs`.

### Nexus Connect (`/v2/connect/*`) — مغطى (функция `connect`)
- Коды: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  И18НИ00000134Х (И18НИ00000135Х).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  И18НИ00000139Х (И18НИ00000140Х).
- Код: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Поддержка: `crates/iroha_torii/tests/connect_gating.rs` (функция стробирования, функция синхронизации, рукопожатие WS) и
  Установите флажок для подключения (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Сообщение: Nexus Connect WG.
- Добавлено: установлен предел скорости `limits::rate_limit_key`; Установите флажок `connect.*`.

### تليمترية مرحلات Kaigi — مغطى
- Код: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  И18НИ00000149Х, И18НИ00000150Х
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  И18НИ00000154Х, И18НИ00000155Х,
  И18НИ00000156Х (И18НИ00000157Х).
- Код: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Код: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Место: Стокгольмская школа SSE, 2017 год Создан для `docs/source/torii/kaigi_telemetry_api.md`.

## ملخص تغطية الاختبارات

- اختبارات Smoke للراوتر (`crates/iroha_torii/tests/router_feature_matrix.rs`) Код OpenAPI установлен.
- تغطي الحزم الخاصة بنقاط النهاية استعلامات الحسابات ودورة حياة العقود Проверьте ZK, проверьте SSE и Nexus Connect.
- Поддержка SDK (JavaScript, Swift, Python) Поддержка псевдонима VOPRF и SSE; Уилла Уилла.

## الحفاظ على تحديث هذه المرآة

Он был создан в 2017 году (`docs/source/torii/app_api_parity_audit.md`). Torii был добавлен в SDK и был установлен в приложении.