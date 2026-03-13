---
lang: ar
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: torii-app-api-parity
العنوان: Audit de parite de l'API d'application Torii
الوصف: شاهد مراجعة TORII-APP-1 لتتمكن أجهزة SDK واللوحة من تأكيد الغطاء العام.
---

القانون: تيرمين 21-03-2026  
المسؤولون: منصة Torii، قائد برنامج SDK  
مرجع خريطة الطريق: TORII-APP-1 - مراجعة التكافؤ `app_api`

تعكس هذه الصفحة التدقيق الداخلي `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) حتى يتمكن المحاضرون في قواعد المستودع الأحادي من رؤية أسطح `/v2/*` ككابلين ومختبرين وموثقين. تتناسب المراجعة مع المسارات المعاد تصديرها عبر `Torii::add_app_api_routes` و`add_contracts_and_vk_routes` و`add_connect_routes`.

## بورتيه وطريقة

قم بفحص عمليات إعادة التصدير العامة في `crates/iroha_torii/src/lib.rs:256-522` ومنشئي الطرق الموجودة في ميزة البوابات. من أجل سطح شاك `/v2/*` من خريطة الطريق، علينا التحقق من ذلك:

- تنفيذ معالج وتعريفات DTO في `crates/iroha_torii/src/routing.rs`.
- تسجيل المسار ضمن مجموعات الميزات `app_api` أو `connect`.
- اختبارات التكامل/الوحدات الموجودة والفريق المسؤول عن التغطية على المدى الطويل.

قوائم الأنشطة/معاملات الحساب وقوائم ضوابط الأنشطة المقبولة لمعلمات الطلب `asset_id` الاختيارية للتصفية المسبقة، بالإضافة إلى حدود ترقيم الصفحات/الضغط العكسي الموجودة.

## المصادقة والتوقيع الكنسي- تعرض نقاط النهاية GET/POST التطبيقات المقبولة لرؤوس خيارات الطلب الكنسي (`X-Iroha-Account`، `X-Iroha-Signature`) من `METHOD\n/path\nsorted_query\nsha256(body)`؛ Torii المغلف في `QueryRequestWithAuthority` قبل التحقق من صحة المنفذ للانعكاس `/query`.
- يتم توفير أدوات المساعدة SDK في جميع العملاء الأساسيين:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` من `canonicalRequest.js`.
  - سويفت: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - أندرويد (كوتلين/جافا): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- أمثلة:
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

## اختراع نقاط النهاية

### أذونات الحساب (`/v2/accounts/{id}/permissions`) - Couvert
- المعالج: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
-DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- ربط جهاز التوجيه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات: `crates/iroha_torii/tests/accounts_endpoints.rs:126` و`crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- المالك: منصة Torii.
- ملاحظات: الاستجابة هي هيئة JSON Norito مع `items`/`total`، متوافقة مع مساعدي ترقيم الصفحات في SDK.### تقييم OPRF d'alias (`POST /v2/aliases/voprf/evaluate`) - Couvert
- المعالج: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
-DTOs: `AliasVoprfEvaluateRequestDto`، `AliasVoprfEvaluateResponseDto`، `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- ربط جهاز التوجيه: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- الاختبارات: الاختبارات المضمنة du Handler (`crates/iroha_torii/src/lib.rs:9945-9986`) بالإضافة إلى couverture SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- المالك: منصة Torii.
- ملاحظات: يفرض سطح الاستجابة محددًا سداسيًا ومعرفات الواجهة الخلفية؛ تستخدم SDK DTO.

### أحداث إثبات SSE (`GET /v2/events/sse`) - Couvert
- المعالج: `handle_v1_events_sse` مع دعم المرشحات (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) بالإضافة إلى إثبات الأسلاك الخاصة بالفلتر.
- ربط جهاز التوجيه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات: مجموعات إثبات SSE المحددة (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`،
  `sse_proof_callhash.rs`، `sse_proof_verified_fields.rs`، `sse_proof_rejected_fields.rs`) واختبار الدخان SSE لخط الأنابيب
  (`integration_tests/tests/events/sse_smoke.rs`).
- المالك: منصة Torii (وقت التشغيل)، فريق عمل اختبارات التكامل (التركيبات).
- ملاحظات: حلقات مقاومة الفلتر صالحة للاستخدام مرة واحدة؛ تم العثور على الوثائق في `docs/source/zk_app_api.md`.### دورة حياة العقود (`/v2/contracts/*`) - كوفر
- المعالجات: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)،
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`)،
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`)،
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`)،
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`، `DeployAndActivateInstanceDto`، `ActivateInstanceDto`، `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- ربط جهاز التوجيه: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- الاختبارات: مجموعات جهاز التوجيه/التكامل `contracts_deploy_integration.rs`، `contracts_activate_integration.rs`،
  `contracts_instance_activate_integration.rs`، `contracts_call_integration.rs`،
  `contracts_instances_list_router.rs`.
- المالك: منصة Smart Contract WG avec Torii.
- ملاحظات: نقاط النهاية موجودة في ملف المعاملات الموقعة وإعادة استخدام مقاييس القياس عن بعد (`handle_transaction_with_metrics`).

### Cycle de vie des cles de التحقق (`/v2/zk/vk/*`) - Couvert
- المعالجات: `handle_post_vk_register`، `handle_post_vk_update`، `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) و`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`، `ZkVkUpdateDto`، `ZkVkDeprecateDto`، `VkListQuery`، `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- ربط جهاز التوجيه: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- الاختبارات: `crates/iroha_torii/tests/zk_vk_get_integration.rs`،
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- المالك: مجموعة عمل ZK avec تدعم منصة Torii.
- ملاحظات: تتم محاذاة DTOs مع المخططات Norito وفقًا لمراجع SDK؛ يتم فرض الحد من المعدل عبر `limits.rs`.### Nexus Connect (`/v2/connect/*`) - Couvert (الميزة `connect`)
- المعالجات: `handle_connect_session`، `handler_connect_session_delete`، `handle_connect_ws`،
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
-DTOs: `ConnectSessionRequest`، `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)،
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- ربط جهاز التوجيه: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- الاختبارات: `crates/iroha_torii/tests/connect_gating.rs` (ميزة البوابات، دورة حياة الجلسة، المصافحة WS) وما إلى ذلك
  غطاء مصفوفة ميزات جهاز التوجيه (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- المالك: Nexus Connect WG.
- ملاحظات: يتم تحديد أسعار الفائدة عبر `limits::rate_limit_key`؛ تعمل حواسيب القياس عن بعد على تشغيل المقاييس `connect.*`.

### قياس التتابع عن بعد Kaigi - Couvert
- المعالجات: `handle_v1_kaigi_relays`، `handle_v1_kaigi_relay_detail`،
  `handle_v1_kaigi_relays_health`، `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
-DTOs: `KaigiRelaySummaryDto`، `KaigiRelaySummaryListDto`،
  `KaigiRelayDetailDto`، `KaigiRelayDomainMetricsDto`،
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- ربط جهاز التوجيه: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- الاختبارات: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- ملاحظات: يقوم Flux SSE بإعادة استخدام قناة البث العالمية عبر تطبيق ملف تعريف القياس عن بعد؛ مخططات الاستجابة هي مستندات في `docs/source/torii/kaigi_telemetry_api.md`.

## استئناف غطاء الاختبارات- تضمن اختبارات دخان جهاز التوجيه (`crates/iroha_torii/tests/router_feature_matrix.rs`) أن مجموعات الميزات تسجل كل مسار وأن الجيل OpenAPI يظل متزامنًا.
- تشمل مجموعات نقاط النهاية طلبات الحسابات ودورة حياة العقود ومفاتيح التحقق من ZK ومرشحات إثبات SSE والسلوكيات Nexus Connect.
- تستخدم الأدوات المماثلة SDK (JavaScript وSwift وPython) الاسم المستعار VOPRF ونقاط النهاية SSE؛ يتطلب العمل الإضافي.

## Garder ce miroir a jour

قم بالاطلاع على هذه الصفحة ومصدر التدقيق (`docs/source/torii/app_api_parity_audit.md`) عند تغيير سلوك التطبيق API Torii حتى يتمكن مالكو SDK والقراء الخارجيون من المحاذاة.