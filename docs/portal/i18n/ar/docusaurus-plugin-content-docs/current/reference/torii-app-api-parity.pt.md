---
lang: ar
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: torii-app-api-parity
العنوان: Auditori de paridade da API de app do Torii
الوصف: شرح مراجعة TORII-APP-1 لتأكيد معدات SDK والمنصة على التغطية العامة.
---

الحالة: النتيجة 2026-03-21  
الردود: منصة Torii، قائد برنامج SDK  
مرجع خريطة الطريق: TORII-APP-1 - قاعة الاستماع `app_api`

هذه الصفحة مخصصة للمستمعين الداخليين `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) للقراء من أجل المستودعات الأحادية مثل الأسطح `/v1/*` التي تحتوي على اتصالات واختبارات ووثائق. ترافق القاعة إعادة التصدير عبر `Torii::add_app_api_routes` و`add_contracts_and_vk_routes` و`add_connect_routes`.

## طريقة الاستخدام

يتم فحص القاعة كإعادة تصدير منشورات في `crates/iroha_torii/src/lib.rs:256-522` ومصممي الدوران مع ميزة البوابات. لكل سطح `/v1/*` للتحقق من خريطة الطريق:

- تنفيذ المعالج وتعريفات DTO في `crates/iroha_torii/src/routing.rs`.
- قم بتسجيل جهاز التوجيه ضمن مجموعات الميزات `app_api` أو `connect`.
- اختبارات التكامل/الوحدات الموجودة وفريق الاستجابة من خلال تغطية طويلة المدى.

كما تتوفر قوائم المهام/معاملات الحسابات وملكية السمات لمعايير استشارة `asset_id` الاختيارية للتصفية المسبقة، بالإضافة إلى وجود حدود للترحيل/الضغط الخلفي.

## Autenticacao e assinatura canonica- نقاط النهاية GET/POST مدعومة بالتطبيقات التي تحتوي على الرؤوس الاختيارية المطلوبة من Canon (`X-Iroha-Account`، `X-Iroha-Signature`) المبنية على `METHOD\n/path\nsorted_query\nsha256(body)`؛ o يشمل نظام التشغيل Torii `QueryRequestWithAuthority` قبل التحقق من صحة المنفذ لاستكشاف `/query`.
- مساعدو SDK موجودون في جميع العملاء الأساسيين:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` من `canonicalRequest.js`.
  - سويفت: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - أندرويد (كوتلين/جافا): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- أمثلة:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "<i105-account-id>", method: "get", path: "/v1/accounts/<i105-account-id>/assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/<i105-account-id>/assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "<i105-account-id>",
                                                  method: "get",
                                                  path: "/v1/accounts/<i105-account-id>/assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("<i105-account-id>", "get", "/v1/accounts/<i105-account-id>/assets", "limit=5", ByteArray(0), signer)
```

## مخزون نقاط النهاية

### أذونات الحساب (`/v1/accounts/{id}/permissions`) - كوبرتو
- المعالج: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
-DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- ربط جهاز التوجيه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات: `crates/iroha_torii/tests/accounts_endpoints.rs:126` و`crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- المالك: منصة Torii.
- الملاحظات: الرد على الجسم JSON Norito مع `items`/`total`، بالإضافة إلى مساعدي صفحات SDK.

### Avaliacao OPRF de alias (`POST /v1/aliases/voprf/evaluate`) - كوبرتو
- المعالج: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
-DTOs: `AliasVoprfEvaluateRequestDto`، `AliasVoprfEvaluateResponseDto`، `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- ربط جهاز التوجيه: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- الاختبارات: معالج الخصيتين المضمن (`crates/iroha_torii/src/lib.rs:9945-9986`) مع حزمة SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- المالك: منصة Torii.
- الملاحظات: سطح الرد يعمل على تحسين المحددات السداسية ومعرفات الواجهة الخلفية؛ يستهلك نظام SDKs o DTO.### أحداث إثبات SSE (`GET /v1/events/sse`) - كوبرتو
- المعالج: `handle_v1_events_sse` com supporte a filtros (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) بالإضافة إلى أسلاك مرشح التصفية.
- ربط جهاز التوجيه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات: مجموعات SSE المحددة للإثبات (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`،
  `sse_proof_callhash.rs`، `sse_proof_verified_fields.rs`، `sse_proof_rejected_fields.rs`) واختبار الدخان SSE لخط الأنابيب
  (`integration_tests/tests/events/sse_smoke.rs`).
- المالك: منصة Torii (وقت التشغيل)، فريق عمل اختبارات التكامل (التركيبات).
- الملاحظات: مرشحات التصفية للمصادقة الشاملة؛ مستند مطابق لـ `docs/source/zk_app_api.md`.

### Ciclo de vida de contratos (`/v1/contracts/*`) - كوبرتو
- المعالجات: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)،
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`)،
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`)،
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`)،
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`، `DeployAndActivateInstanceDto`، `ActivateInstanceDto`، `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- ربط جهاز التوجيه: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- الاختبارات: أجنحة جهاز التوجيه/integracao `contracts_deploy_integration.rs`، `contracts_activate_integration.rs`،
  `contracts_instance_activate_integration.rs`، `contracts_call_integration.rs`،
  `contracts_instances_list_router.rs`.
- المالك: منصة Smart Contract WG com Torii.
- الملاحظات: تقوم نقاط النهاية بحذف المعاملات وإعادة استخدام مقاييس القياس عن بعد المشتركة (`handle_transaction_with_metrics`).### Ciclo de vida de chaves de verificacao (`/v1/zk/vk/*`) - كوبرتو
- المعالجات: `handle_post_vk_register`، `handle_post_vk_update`، `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) و`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`، `ZkVkUpdateDto`، `ZkVkDeprecateDto`، `VkListQuery`، `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- ربط جهاز التوجيه: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- الاختبارات: `crates/iroha_torii/tests/zk_vk_get_integration.rs`،
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- المالك: ZK Working Group com supporte da Torii Platform.
- الملاحظات: Os DTOs se alinham aos schemas Norito rerenciados pelos SDKs؛ يتم تطبيق تحديد المعدل عبر `limits.rs`.

### Nexus Connect (`/v1/connect/*`) - كوبرتو (الميزة `connect`)
- المعالجات: `handle_connect_session`، `handler_connect_session_delete`، `handle_connect_ws`،
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
-DTOs: `ConnectSessionRequest`، `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)،
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- ربط جهاز التوجيه: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- الاختبارات: `crates/iroha_torii/tests/connect_gating.rs` (ميزة البوابات، دورة حياة الجلسة، المصافحة WS) e
  تتضمن ميزات المصفوفة جهاز التوجيه (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- المالك: Nexus Connect WG.
- الملاحظات: الحد الأقصى لسعر الفائدة sao rastreadas عبر `limits::rate_limit_key`؛ أجهزة القياس عن بعد للطعام بمقاييس `connect.*`.### قياس التتابع عن بعد كايجي - كوبرتو
- المعالجات: `handle_v1_kaigi_relays`، `handle_v1_kaigi_relay_detail`،
  `handle_v1_kaigi_relays_health`، `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
-DTOs: `KaigiRelaySummaryDto`، `KaigiRelaySummaryListDto`،
  `KaigiRelayDetailDto`، `KaigiRelayDomainMetricsDto`،
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- ربط جهاز التوجيه: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- الاختبارات: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- الملاحظات: إعادة استخدام دفق SSE أو قناة البث العالمية من خلال التطبيق أو بوابة ملف القياس عن بعد؛ تم توثيق مخططات الاستجابة على `docs/source/torii/kaigi_telemetry_api.md`.

## ملخص تغطية الخصية

- تضمن خصيتي دخان جهاز التوجيه (`crates/iroha_torii/tests/router_feature_matrix.rs`) أن تجمع الميزات تسجيل كل شيء كدوران وأن تتم مزامنة جهاز OpenAPI.
- مجموعات خاصة بنقاط النهاية تشمل استعلامات البيانات، ودائرة العقود، وأرقام التحقق ZK، ومرشحات إثبات SSE، وخصائص Nexus Connect.
- أدوات تكافؤ SDK (JavaScript وSwift وPython) واستخدام الاسم المستعار VOPRF ونقاط النهاية SSE؛ nao ha trabalho adicional.

## Manter este espelho atualizado

قم بتحديث هذه الصفحة وخط الصوت (`docs/source/torii/app_api_parity_audit.md`) عندما تعمل واجهة برمجة تطبيقات التطبيق Torii على تمكين مالكي SDK والقراء الخارجيين من النفاذ.