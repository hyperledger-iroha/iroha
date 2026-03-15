---
lang: es
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-paridad
título: تدقيق تكافؤ واجهة تطبيق Torii
descripción: La aplicación TORII-APP-1 incluye el SDK y el software.
---

الحالة: مكتمل 2026-03-21  
المالكون: Plataforma Torii, Líder del programa SDK  
Nombre del usuario: TORII-APP-1 — تدقيق تكافؤ `app_api`

تعكس هذه الصفحة تدقيق `TORII-APP-1` الداخلي (`docs/source/torii/app_api_parity_audit.md`) حتى يتمكن القراء خارج المستودع الاحادي من معرفة Aquí está `/v1/*` موصولة ومختبرة وموثقة. يتتبع التدقيق المسارات المعاد تصديرها عبر `Torii::add_app_api_routes` و`add_contracts_and_vk_routes` و`add_connect_routes`.

## النطاق والمنهج

يفحص التدقيق عمليات اعادة التصدير العامة في `crates/iroha_torii/src/lib.rs:256-522` وبناة المسارات المحمية بالميزات. Y aquí está `/v1/*` que está conectado a:

- تنفيذ المعالج وتعريفات DTO في `crates/iroha_torii/src/routing.rs`.
- Utilice los dispositivos `app_api` e `connect`.
- اختبارات التكامل/الوحدة الموجودة والفريق المسؤول عن التغطية طويلة الاجل.

قوائم أصول/معاملات الحساب وقوائم حاملي الأصول تقبل معاملات استعلام `asset_id` اختيارية للتصفية المسبقة, بالإضافة إلى حدود الترقيم/الضغط العكسي الحالية.

## المصادقة والتوقيع القياسي- Haga clic en GET/POST para obtener información sobre el producto (`X-Iroha-Account`, `X-Iroha-Signature`) `METHOD\n/path\nsorted_query\nsha256(body)`؛ Aquí está Torii y `QueryRequestWithAuthority` para ejecutar el ejecutor `/query`.
- تتوفر مساعدات SDK في جميع العملاء الرئيسيين:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` de `canonicalRequest.js`.
  - Rápido: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- امثلة:
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

## جرد نقاط النهاية

### اذونات الحساب (`/v1/accounts/{id}/permissions`) — مغطى
- Nombre: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Nombre del producto: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Idiomas: `crates/iroha_torii/tests/accounts_endpoints.rs:126` y `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Contenido: Plataforma Torii.
- Archivos: Aquí hay JSON Norito o `items`/`total`, pero no hay archivos disponibles. Utilice el SDK.

### تقييم OPRF للاسماء المستعارة (`POST /v1/aliases/voprf/evaluate`) — مغطى
- Nombre: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Nombre del producto: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- الاختبارات: اختبارات inline للمعالج (`crates/iroha_torii/src/lib.rs:9945-9986`) بالاضافة الى تغطية SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Contenido: Plataforma Torii.
- Funciones: configuración hexadecimal y backend وتستهلك SDK الDTO.### Prueba de prueba عبر SSE (`GET /v1/events/sse`) — مغطى
- المعالج: `handle_v1_events_sse` مع دعم الفلاتر (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) مع توصيل فلتر prueba.
- Nombre del producto: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات: حزم SSE خاصة بالproof (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) y humo لSSE في خط الانابيب
  (`integration_tests/tests/events/sse_smoke.rs`).
- Contenido: Plataforma Torii (tiempo de ejecución), GT de pruebas de integración (accesorios).
- ملاحظات: تم التحقق من مسارات فلتر prueba طرفا لطرف؛ Este es el nombre de `docs/source/zk_app_api.md`.

### دورة حياة العقود (`/v1/contracts/*`) — مغطى
- Títulos: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Nombre del producto: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- الاختبارات: enrutador/integración `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Tema: Smart Contract WG de la plataforma Torii.
- ملاحظات: نقاط النهاية تضع المعاملات الموقعة في قائمة انتظار وتعيد استخدام مقاييس التليمترية المشتركة (`handle_transaction_with_metrics`).### دورة حياة مفاتيح التحقق (`/v1/zk/vk/*`) — مغطى
- Títulos: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) y `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Nombre del producto: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Idiomas: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- المالك: Grupo de trabajo ZK sobre la plataforma Torii.
- Funciones: تطابق DTOs مع مخططات Norito التي تعتمد عليها SDKs؛ Aquí está la limitación de velocidad en `limits.rs`.

### Nexus Conectar (`/v1/connect/*`) — مغطى (característica `connect`)
- Títulos: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Nombre del producto: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Contenidos: `crates/iroha_torii/tests/connect_gating.rs` (activación de funciones, función de control de enlace WS) y
  تغطية مصفوفة ميزات الموجه (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- المالك: Nexus Connect WG.
- ملاحظات: يتم تتبع مفاتيح límite de tasa عبر `limits::rate_limit_key`؛ وتغذي عدادات التليمترية مقاييس `connect.*`.### تليمترية مرحلات Kaigi — مغطى
- Títulos: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Nombre del artículo: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Idiomas: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- ملاحظات: يعيد بث SSE استخدام القناة العامة للبث مع فرض بوابة ملف تليمترية؛ وتوثق مخططات الاستجابة في `docs/source/torii/kaigi_telemetry_api.md`.

## ملخص تغطية الاختبارات

- Humo de humo (`crates/iroha_torii/tests/router_feature_matrix.rs`) تضمن ان تركيبات الميزات تسجل كل مسار وان توليد OpenAPI يبقى متزامنا.
- تغطي الحزم الخاصة بنقاط النهاية استعلامات الحسابات ودورة حياة العقود ومفاتيح التحقق ZK وفلاتر prueba SSE وسلوك Nexus Conectar.
- Aplicación SDK (JavaScript, Swift, Python) Aplicación Alias ​​VOPRF y SSE ولا يلزم عمل اضافي.

## الحفاظ على تحديث هذه المرآة

حدّث هذه الصفحة وتدقيق المصدر (`docs/source/torii/app_api_parity_audit.md`) عند تغير سلوك واجهة تطبيق Torii حتى يبقى مالكو SDK y الخارجيون على نفس الخط.