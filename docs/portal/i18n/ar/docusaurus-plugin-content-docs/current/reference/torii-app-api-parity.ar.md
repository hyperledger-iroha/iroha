---
lang: ar
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: torii-app-api-parity
العنوان: تدقيق تكافؤ واجهة تطبيق Torii
description: نسخة مرآة لمراجعة TORII-APP-1 حتى الشعبية الناشئة SDK والمنصة من المؤكد التغطية العامة.
---

الحالة: مكتملة 2026-03-21  
بالباكون: Torii Platform، قائد برنامج SDK  
مرجع خريطة الطريق: TORII-APP-1 — تدقيق تكافؤ `app_api`

احترام هذه الصفحة تدقيق `TORII-APP-1` الداخلي (`docs/source/torii/app_api_parity_audit.md`) حتى يشاء القراء خارج المستودع الاحادي من معرفة اي سطح `/v2/*` موصولة ومختبرة وموثقة. يتتبع التمييز المسارات تصديرها عبر `Torii::add_app_api_routes` و`add_contracts_and_vk_routes` و`add_connect_routes`.

## النطاق والمنهج

يفحص عمليات إعادة النسخ العامة في `crates/iroha_torii/src/lib.rs:256-522` وبناة المسارات المحمية بالميزات. سطح المكتب `/v2/*` في خارطة الطريق تحققنا من:

- تنفيذ برنامج وتعريفات DTO في `crates/iroha_torii/src/routing.rs`.
- تسجيل الموجه ضمن مجموعات الميزات `app_api` او `connect`.
- تتكون/الوحدة الموجودة والفريق المسؤول عن التغطية الطويلة الاجل.

أسس أصول/معاملات الحساب وقوائم الأصول حاملي المقايضة `asset_id` اختيارية للتصفية المسبقة، بالإضافة إلى نطاق القيم/الضغط العكسي الحالي.

## المصادقة والتوقيع العلامة التجارية- نقاط النهاية GET/POST نهائيا تقبل العاصمة نهائيا اختيارية (`X-Iroha-Account`, `X-Iroha-Signature`) مبنية من `METHOD\n/path\nsorted_query\nsha256(body)`؛ يقوم Torii بتغليفها في `QueryRequestWithAuthority` قبل أن يتحقق المنفذ ليتطابق `/query`.
- اختيارات مساعدات SDK في جميع العملاء الرئيسيين:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` من `canonicalRequest.js`.
  - سويفت: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - أندرويد (كوتلين/جافا): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- امثلة:
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

## جرد النهائي نقاط

### اذونات الحساب (`/v2/accounts/{id}/permissions`) — الإرسال
-الروم: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
-DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- ربط الموجه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
-التحدي: `crates/iroha_torii/tests/accounts_endpoints.rs:126` و`crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- بالمالك: منصة Torii.
- وجهات النظر: هي كائن JSON Norito مع `items`/`total`، بما يتطابق مع مساعدات ترقيم الصفحات في SDK.

### تقييم OPRF للاسماء المستعارة (`POST /v2/aliases/voprf/evaluate`) — النادى
-الروم: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
-DTOs: `AliasVoprfEvaluateRequestDto`، `AliasVoprfEvaluateResponseDto`، `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- ربط الموجه: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- التركيز: السيولة المضمنة للمعالج (`crates/iroha_torii/src/lib.rs:9945-9986`) بالاضافة الى قطاعات SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- بالمالك: منصة Torii.
- ملفات: لغة المستعار المستعارة Hex محددة وهي الواجهة الخلفية؛ وتستهلك SDK الDTO.### احدث دليل عبر SSE (`GET /v2/events/sse`) — عبر
-Rum: `handle_v1_events_sse` مع دعم الفلاتر (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) مع دليل توصيل الفلتر.
- ربط الموجه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
-التحدي: حزم SSE خاصة بالإثبات (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) وجرب الدخان لSSE في خط الانابيب
  (`integration_tests/tests/events/sse_smoke.rs`).
- بالمالك: منصة Torii (وقت التشغيل)، مجموعة اختبارات التكامل (التركيبات).
- نصيحة: تم التحقق من التحقق من مرشح التصفية لطرف؛ والتوثيق موجود في `docs/source/zk_app_api.md`.

### دورة حياة العقود (`/v2/contracts/*`) — الاعلام
-الرومات: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)،
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`)،
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`)،
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`)،
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`، `DeployAndActivateInstanceDto`، `ActivateInstanceDto`، `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- ربط الموجه: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
-البحث: حزم جهاز التوجيه/التكامل `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`، `contracts_call_integration.rs`،
  `contracts_instances_list_router.rs`.
- بالمالك: Smart Contract WG مع منصة Torii.
- نقاط: النتيجة النهائية المعاملات الموقعة في قائمة انتظار وتعيد استخدام مقاييس التليمترية المشتركة (`handle_transaction_with_metrics`).### دورة حياة مفاتيح التحقق (`/v2/zk/vk/*`) — البريد الإلكتروني
-الرومات: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) و`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`، `ZkVkUpdateDto`، `ZkVkDeprecateDto`، `VkListQuery`، `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- ربط الموجه: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
-التحدي: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- بالمالك: ZK Working Group مع دعم منصة Torii.
- الجداول: تتطابق DTOs مع مخططات Norito التي تعتمد عليها SDKs؛ يفترض أيضًا تحديد المعدل عبر `limits.rs`.

### Nexus Connect (`/v2/connect/*`) — عبر (الميزة `connect`)
-الرومات: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
-DTOs: `ConnectSessionRequest`، `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)،
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- ربط الموجه: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
-التحدي: `crates/iroha_torii/tests/connect_gating.rs` (ميزة البوابات، دورة حياة النظر، المصافحة WS) و
  نطاقات مصفوفة خدمات الموجه (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- بالمالك: Nexus Connect WG.
- آراء: يتم تغيير مفاتيح المفاتيح عبر `limits::rate_limit_key`؛ وتغذي عدادات التليميترية معايير `connect.*`.### رحلات تليمترية Kaigi — ناطق
-الرومات: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`، `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
-DTOs: `KaigiRelaySummaryDto`، `KaigiRelaySummaryListDto`،
  `KaigiRelayDetailDto`، `KaigiRelayDomainMetricsDto`،
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- ربط الموجه: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
-التحدي: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- ملاحظات: استعادة بث SSE استخدام القناة العامة للبث مع فرض بوابة ملف تليمترية؛ ومخططات التنسيق حسب `docs/source/torii/kaigi_telemetry_api.md`.

## نطاق القطاعات

- تحسين الدخان للروتر (`crates/iroha_torii/tests/router_feature_matrix.rs`) ويشمل ان تركيبات عناصر مهمة كل مسار وسوف يستمر توليد OpenAPI متزامنا.
- تغطي الشارات الخاصة بالنقاط النهائية استشارات العلامات التجارية الخاصة بحياة العقود ومفاتيح التحقق من ZK وفلاتر إثبات SSE والسلوك Nexus Connect.
- ادوات تشارك SDK (JavaScript, Swift, Python) تستهلك بالفعل Alias ​​VOPRF ونقاط SSE؛ ولا تحتاج إلى عمل إضافي.

## صيانة على تحديث هذه الاميره

تحديث هذه الصفحة ودقيق المصدر (`docs/source/torii/app_api_parity_audit.md`) عند تغيير التحكم في تطبيق Torii حتى يبقى مالكو SDK والقراء الخارجيون على نفس الخط.