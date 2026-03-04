---
lang: ar
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: torii-app-api-parity
العنوان: Torii IP API
الوصف: TORII-APP-1 هو عبارة عن أداة نقل SDK ومنصة متعددة الاستخدامات تعمل على تحديث اللعبة.
---

حیثیت: مكمل 2026-03-21  
مالكان: منصة Torii، قائد برنامج SDK  
نظام الحوالة: TORII-APP-1 — `app_api` للتفعيل

تحتوي إحدى صفحات الاندرويد `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) على بطاقة مونو ريبو التي توفر سكيًا عالي الجودة `/v1/*` سطح الأرض، موجود ومثبت. لقد تم إصدار هذه البطاقة الجديدة `Torii::add_app_api_routes` و`add_contracts_and_vk_routes` و`add_connect_routes` والتي تم إصدارها مرة أخرى من Microsoft.

## دائره وطريقه

يحتوي `crates/iroha_torii/src/lib.rs:256-522` على عوائد للرياضات الإلكترونية ويتميز بميزة البوابات والمنافذ التي تفوز بالجائزة الكبرى. لا يوجد درج لتصفح سطح المكتب `/v1/*`:

- `crates/iroha_torii/src/routing.rs` معالج نفاذ وتعريف DTO.
- `app_api` أو `connect` مجموعة أدوات تحت جهاز التوجيه.
- متوفر في الإنترنت/يونتن ٹيسٹس وملف كامل ومتنوع.

توفر الأثاث/الضغط الخلفي والأثاث المتاح نطاق الصفحات/الضغط الخلفي نطاقًا إضافيًا لاختلاف المتصفح `asset_id` استعلام برمائي ہیں۔

## تصدیق وکينونیكل دستخط- خيار الحصول على/POST خرائط مختلفة لسجلات الإنترنت (`X-Iroha-Account`, `X-Iroha-Signature`) يوافق على استخدام `METHOD\n/path\nsorted_query\nsha256(body)` جاتے ہيں؛ Torii هو المنفذ ويليشن الذي تم تنفيذه `QueryRequestWithAuthority` وهو عبارة عن جهاز كمبيوتر محمول أو `/query`.
- SDK يلبرز جميع أجهزة الكمبيوتر المحمولة:
  - JS/TS: `canonicalRequest.js` سے `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`.
  - سويفت: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - أندرويد (كوتلين/جافا): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
-مثال:
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

## اينڈپوائنٹ فيرست

### اجازة (`/v1/accounts/{id}/permissions`) — كورڈ
- المعالج: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
-DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- ربط جهاز التوجيه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات: `crates/iroha_torii/tests/accounts_endpoints.rs:126` و`crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- المالك: منصة Torii.
- ملاحظات: جواب Norito JSON body ہے جس میں `items`/`total` ہے، مساعدو ترقيم الصفحات من جو SDK يتوافقون مع رکھتا ہے.

### الاسم المستعار OPRF تشخیص (`POST /v1/aliases/voprf/evaluate`) — کورڈ
- المعالج: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
-DTOs: `AliasVoprfEvaluateRequestDto`، `AliasVoprfEvaluateResponseDto`، `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- ربط جهاز التوجيه: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- الاختبارات: المعالج والاختبارات المضمنة (`crates/iroha_torii/src/lib.rs:9945-9986`) ومرفق SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- المالك: منصة Torii.
- ملاحظات: الإجابة عبارة عن معرّفات سداسية حتمية وخلفية نافعة؛ يتم استخدام SDKs كـ DTO.### إثبات SSE ايونٹس (`GET /v1/events/sse`) — كورڈ
- المعالج: `handle_v1_events_sse` فلير سبورت کے ساتھ (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) وأسلاك مرشح الإثبات.
- ربط جهاز التوجيه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات : إثبات مخصوص أجنحة SSE (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`، `sse_proof_verified_fields.rs`، `sse_proof_rejected_fields.rs`) واختبار الدخان SSE لخط الأنابيب
  (`integration_tests/tests/events/sse_smoke.rs`).
- المالك: منصة Torii (وقت التشغيل)، فريق عمل اختبارات التكامل (التركيبات).
- ملاحظات: تم التحقق من صحة مرشح الإثبات من طرف إلى طرف؛ تم إنشاء دستاويزات `docs/source/zk_app_api.md`.

### انترنت للذواقة (`/v1/contracts/*`) — كورڈ
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
- المالك: Smart Contract WG کے ساتھ Torii Platform.
- ملاحظات: يمكن استخدام القطارات الفضائية وقطارات الأقمار الصناعية (`handle_transaction_with_metrics`) مرة أخرى.### فيرونا للأجنحة الفندقية (`/v1/zk/vk/*`) — كورڈ
- المعالجات: `handle_post_vk_register`، `handle_post_vk_update`، `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) و`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`، `ZkVkUpdateDto`، `ZkVkDeprecateDto`، `VkListQuery`، `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- ربط جهاز التوجيه: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- الاختبارات: `crates/iroha_torii/tests/zk_vk_get_integration.rs`،
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- المالك: ZK Working Group کے ساتھ Torii منصة سپورٹ۔
- ملاحظات: مخططات DTOs Norito متوافقة مع إصدار SDKs؛ الحد الأقصى للمعدل `limits.rs` هو ذريعة نافعة.

### Nexus Connect (`/v1/connect/*`) — كورڈ (الميزة `connect`)
- المعالجات: `handle_connect_session`، `handler_connect_session_delete`، `handle_connect_ws`،
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
-DTOs: `ConnectSessionRequest`، `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)،
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- ربط جهاز التوجيه: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- الاختبارات: `crates/iroha_torii/tests/connect_gating.rs` (ميزة البوابات، سيشن لعشاق سائیکل، مصافحة WS) و
  مصفوفة ميزة جهاز التوجيه کوریج (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- المالك: Nexus Connect WG.
- ملاحظات: مفاتيح حد المعدل `limits::rate_limit_key` تم إصدارها؛ ٹيليمتري عدادات `connect.*` عدادات بطاقة الائتمان.### Kaigi Relay ٹيليمتري — كورڈ
- المعالجات: `handle_v1_kaigi_relays`، `handle_v1_kaigi_relay_detail`،
  `handle_v1_kaigi_relays_health`، `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
-DTOs: `KaigiRelaySummaryDto`، `KaigiRelaySummaryListDto`،
  `KaigiRelayDetailDto`، `KaigiRelayDomainMetricsDto`،
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- ربط جهاز التوجيه: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- الاختبارات: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- ملاحظات: البث العالمي لبث SSE يستخدم ككرتا من خلال بوابات البوابات الذكية؛ مخططات الاستجابة `docs/source/torii/kaigi_telemetry_api.md`

## ٹیستٹ کوريج خلاصةہ

- اختبارات دخان جهاز التوجيه (`crates/iroha_torii/tests/router_feature_matrix.rs`) تحتوي على مجموعات من الميزات مثل تسجيل المسار ومزامنة الجيل OpenAPI.
- مجموعات مخصوص لنقطة النهاية، الاستعلامات، ودورة حياة العقد، ومفاتيح التحقق من ZK، ومرشحات إثبات SSE، وبطاقة Nexus Connect.
- أدوات تكافؤ SDK (JavaScript وSwift وPython) تستخدم الاسم المستعار VOPRF ونقاط النهاية SSE؛ لا يوجد أي إضافة إضافية.

## هذا أمر جيد جدًا

يتم تحديث واجهة برمجة التطبيقات Torii app API التي تدعم صفحتك والإصدار (`docs/source/torii/app_api_parity_audit.md`) من خلال تطبيق SDK مالك وقارئ البيانات.