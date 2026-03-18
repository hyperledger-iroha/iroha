---
lang: ar
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: torii-app-api-parity
العنوان: تدقيق تطبيق واجهة برمجة التطبيقات (API) Torii
الوصف: نظرة عامة على TORII-APP-1، حيث يمكن لأوامر SDK والأنظمة الأساسية مراقبة النشر العام.
---

الحالة: جديد 2026-03-21  
المشجعون: منصة Torii، قائد برنامج SDK  
خيار البطاقة الأخيرة: TORII-APP-1 — تدقيق الطرف `app_api`

يعرض هذا الجزء التدقيق الداخلي `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`)، بحيث يمكن للقراء مشاهدته كما هو الحال تم إغلاق `/v1/*` والبروتستيروفان والتوثيق. تراقب التدقيق البيانات الرئيسية، وإعادة التصدير من خلال `Torii::add_app_api_routes`، و`add_contracts_and_vk_routes`، و`add_connect_routes`.

## الكتلة والطريقة

تقوم المراجعة بفحص الرحلات العامة في `crates/iroha_torii/src/lib.rs:256-522` وتركيب المخططات ذات ميزة البوابات. للاختبار الشامل `/v1/*` في البطاقة التالية:

- تحقيق المعالج والاقتراح DTO في `crates/iroha_torii/src/routing.rs`.
- ميزة تسجيل جهاز التوجيه في المجموعة هي `app_api` أو `connect`.
- تحقيق التكامل/الاختبارات والأوامر للتكامل الشامل.

قائمة الأنشطة/حساب المعاملات وقائمة الوكلاء النشطين تبدأ بمعلمات الاستعلام غير الضرورية `asset_id` التصفية المسبقة، مع وجود حدود للصفحات/التنقل العام.

## التصديق والسجل القانوني- الحصول على/نشر نقاط التطبيق الرئيسية اختياري للتأمين القانوني (`X-Iroha-Account`، `X-Iroha-Signature`)، التعزيز من `METHOD\n/path\nsorted_query\nsha256(body)`; Torii يتم إعادته إلى `QueryRequestWithAuthority` قبل التحقق من صحة المنفذ، وهو ما يعني `/query`.
- المساعدة في توفير SDK لجميع العملاء الأساسيين:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` من `canonicalRequest.js`.
  - سويفت: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - أندرويد (كوتلين/جافا): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- الأمثلة:
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

## إنشاء نقاط

### الحساب الصحيح (`/v1/accounts/{id}/permissions`) — مفتوح
- المعالج: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- دي تي أو: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- ربط جهاز التوجيه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات: `crates/iroha_torii/tests/accounts_endpoints.rs:126` و`crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- المالك: منصة Torii.
- ملاحظات: أرسل — Norito JSON مع `items`/`total`، متضمن مع مساعدي صفحات SDK.

### الاسم المستعار لـ OPRF (`POST /v1/aliases/voprf/evaluate`) — مفتوح
- المعالج: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- دي تي أو: `AliasVoprfEvaluateRequestDto`، `AliasVoprfEvaluateResponseDto`، `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- ربط جهاز التوجيه: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- الاختبارات: معالج الاختبارات المضمنة (`crates/iroha_torii/src/lib.rs:9945-9986`) بالإضافة إلى إنشاء SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- المالك: منصة Torii.
- ملاحظات: قم بالإجابة على السؤال التالي: تحديد الواجهة الخلفية السداسية والمعرفات؛ يستخدم SDK DTO.### إثبات الاشتراك SSE (`GET /v1/events/sse`) — تمام
- المعالج: `handle_v1_events_sse` с поддерой фильтров (`crates/iroha_torii/src/routing.rs:14008-14133`).
-DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) بالإضافة إلى إثبات مرشح الأسلاك.
- ربط جهاز التوجيه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات: إثباتات محددة لعناصر SSE (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`، `sse_proof_verified_fields.rs`، `sse_proof_rejected_fields.rs`) واختبار الدخان SSE العادي
  (`integration_tests/tests/events/sse_smoke.rs`).
- المالك: منصة Torii (وقت التشغيل)، فريق عمل اختبارات التكامل (التركيبات).
- ملاحظات: يتم التحقق من التصفية من النهاية إلى النهاية؛ التوثيق في `docs/source/zk_app_api.md`.

### العقود التجارية الخاصة (`/v1/contracts/*`) — الانتهاء
- المعالجات: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)،
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`)،
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`)،
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`)،
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- دي تي أو: `DeployContractDto`، `DeployAndActivateInstanceDto`، `ActivateInstanceDto`، `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- ربط جهاز التوجيه: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- الاختبارات: جهاز التوجيه/التكامل сьюты `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`، `contracts_call_integration.rs`،
  `contracts_instances_list_router.rs`.
- المالك: Smart Contract WG مع منصة Torii.
- ملاحظات: تقوم النقاط بوضع المعاملات التفصيلية في المراقبة والتبادل باستخدام أجهزة القياس عن بعد (`handle_transaction_with_metrics`).### اختبارات المفاتيح الذكية (`/v1/zk/vk/*`) — إخلاء المسؤولية
- المعالجات: `handle_post_vk_register`، `handle_post_vk_update`، `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) و`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`، `ZkVkUpdateDto`، `ZkVkDeprecateDto`، `VkListQuery`، `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- ربط جهاز التوجيه: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- الاختبارات: `crates/iroha_torii/tests/zk_vk_get_integration.rs`،
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- المالك: ZK Working Group pri поддераке Torii Platform.
- ملاحظات: تعليمات DTO ذات المخططات Norito، التي تتوافق مع SDK؛ تم فرض تحديد المعدل من خلال `limits.rs`.

### Nexus Connect (`/v1/connect/*`) — Поклыто (الميزة `connect`)
- المعالجات: `handle_connect_session`، `handler_connect_session_delete`، `handle_connect_ws`،
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- دي تي أو: `ConnectSessionRequest`، `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)،
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- ربط جهاز التوجيه: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- الاختبارات: `crates/iroha_torii/tests/connect_gating.rs` (ميزة البوابات، دورة الحياة، مصافحة WS) و
  تتميز المواد العازلة بجهاز التوجيه (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- المالك: Nexus Connect WG.
- ملاحظات: يتم تحديد حد معدل المفاتيح من خلال `limits::rate_limit_key`؛ قياس المسافة عن بعد متر `connect.*`.### جهاز القياس عن بعد Kaigi — بوكريتو
- المعالجات: `handle_v1_kaigi_relays`، `handle_v1_kaigi_relay_detail`،
  `handle_v1_kaigi_relays_health`، `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- دي تي أو: `KaigiRelaySummaryDto`، `KaigiRelaySummaryListDto`،
  `KaigiRelayDetailDto`، `KaigiRelayDomainMetricsDto`،
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- ربط جهاز التوجيه: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- الاختبارات: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- ملاحظات: تنقل SSE عبر قناة البث العالمية وتدمج ملف التعريف عن بعد؛ تم توضيح المخططات في `docs/source/torii/kaigi_telemetry_api.md`.

## شهادات صب الماء

- يضمن جهاز توجيه اختبار الدخان (`crates/iroha_torii/tests/router_feature_matrix.rs`) أن المجموعات تتميز بتسجيل كل من الخريطة والجيل OpenAPI متزامن.
- معلومات محددة عن نقطة معينة لفحص حسابات الحسابات، وعقود التأمين على الحياة، وأزرار ZK، ومرشحات إثبات SSE، و متابعة Nexus الاتصال.
- يمكن لأدوات تكافؤ SDK (JavaScript وSwift وPython) استخدام نقاط Alias ​​VOPRF وSSE؛ لا حاجة إلى أعمال إضافية.

## تعزيز الحالة في الحالة الفعلية

قم بتحديث هذا الجزء والتدقيق الخارجي (`docs/source/torii/app_api_parity_audit.md`)، عند الإشارة إلى Torii app API لأعضاء SDK و القراء الجدد يتخلصون من الإدمان.