---
lang: ar
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: torii-app-api-parity
العنوان: Auditori de paridad de la API de la app de Torii
الوصف: مراجعة مراجعة TORII-APP-1 لتأكيد معدات SDK والمنصة على التغطية العامة.
---

الحالة: اكتملت بتاريخ 21-03-2026  
المسؤولون: منصة Torii، قائد برنامج SDK  
مرجع خريطة الطريق: TORII-APP-1 - قاعة الاستماع إلى `app_api`

تشير هذه الصفحة إلى المستمع الداخلي `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) حتى يتمكن القراء من المستودع الأحادي لرؤية الأسطح `/v1/*` التي يتم إرسالها والاختبارات والوثائق. تم عرض المسارات المعاد تصديرها عبر `Torii::add_app_api_routes` و`add_contracts_and_vk_routes` و`add_connect_routes`.

## الطريقة والطريقة

تقوم القاعة بفحص عمليات إعادة التصدير العامة في `crates/iroha_torii/src/lib.rs:256-522` ومنشئي المسارات المزودين ببوابات مميزة. لكل سطح `/v1/*` لخريطة الطريق التي تم التحقق منها:

- تنفيذ معالج وتعريفات DTO في `crates/iroha_torii/src/routing.rs`.
- قم بتسجيل جهاز التوجيه تحت مجموعة الميزات `app_api` أو `connect`.
- اختبار التكامل/الوحدات الموجودة والفريق المسؤول عن التغطية على المدى الطويل.

تقبل قوائم الأنشطة/معاملات الحسابات وقوائم أسماء الأنشطة مراجعة المعلمات الاختيارية `asset_id` للتصفية المسبقة، بالإضافة إلى الحدود الموجودة للصفحات/الضغط الخلفي.## Autenticacion وfirma canonica

- نقاط النهاية GET/POST موجهة إلى التطبيقات التي تقبل الرؤوس الاختيارية لطلب Canonica (`X-Iroha-Account`، `X-Iroha-Signature`) المبنية من `METHOD\n/path\nsorted_query\nsha256(body)`؛ يتم إدخال Torii في `QueryRequestWithAuthority` قبل التحقق من صحة المنفذ ليعكس `/query`.
- يتم دمج مساعدي SDK مع جميع العملاء الأساسيين:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` من `canonicalRequest.js`.
  - سويفت: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - أندرويد (كوتلين/جافا): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- الأمثلة:
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

## مخزون نقاط النهاية

### أذونات الحساب (`/v1/accounts/{id}/permissions`) - كوبيرتو
- المعالج: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
-DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- ربط جهاز التوجيه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات: `crates/iroha_torii/tests/accounts_endpoints.rs:126` و`crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- المالك: منصة Torii.
- الملاحظات: الرد عبارة عن نص JSON Norito مع `items`/`total`، والذي يتزامن مع مساعدي صفحة SDK.### تقييم OPRF للاسم المستعار (`POST /v1/aliases/voprf/evaluate`) - كوبيرتو
- المعالج: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
-DTOs: `AliasVoprfEvaluateRequestDto`، `AliasVoprfEvaluateResponseDto`، `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- ربط جهاز التوجيه: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- الاختبارات: اختبار المعالج المضمن (`crates/iroha_torii/src/lib.rs:9945-9986`) بالإضافة إلى تغطية SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- المالك: منصة Torii.
- الملاحظات: سطح الاستجابة يتطلب تحديدًا سداسيًا ومعرفات الواجهة الخلفية؛ تستهلك SDK DTO.

### أحداث إثبات SSE (`GET /v1/events/sse`) - كوبيرتو
- المعالج: `handle_v1_events_sse` مع حامل المرشحات (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) مع أسلاك مرشح التصفية.
- ربط جهاز التوجيه: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات: مجموعات SSE المحددة للإثبات (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`،
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) واختبار دخان SSE لخط الأنابيب
  (`integration_tests/tests/events/sse_smoke.rs`).
- المالك: منصة Torii (وقت التشغيل)، فريق عمل اختبارات التكامل (التركيبات).
- الملاحظات: تعتبر مسارات التصفية صالحة من البداية إلى النهاية؛ التوثيق حي في `docs/source/zk_app_api.md`.### Ciclo de vida de contratos (`/v1/contracts/*`) - كوبيرتو
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
- المالك: منصة Smart Contract WG con Torii.
- الملاحظات: تقوم نقاط النهاية بتجميع المعاملات الثابتة وإعادة استخدام مقاييس القياس عن بعد (`handle_transaction_with_metrics`).

### Ciclo de vida de claves de verificacion (`/v1/zk/vk/*`) - كوبيرتو
- المعالجات: `handle_post_vk_register`، `handle_post_vk_update`، `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) و`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`، `ZkVkUpdateDto`، `ZkVkDeprecateDto`، `VkListQuery`، `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- ربط جهاز التوجيه: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- الاختبارات: `crates/iroha_torii/tests/zk_vk_get_integration.rs`،
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- المالك: ZK Working Group con soporte de Torii Platform.
- الملاحظات: DTOs غير مرتبطة بالمسميات Norito المرجعية بواسطة SDK؛ يتم تحديد المعدل عبر `limits.rs`.### Nexus Connect (`/v1/connect/*`) - كوبيرتو (الميزة `connect`)
- المعالجات: `handle_connect_session`، `handler_connect_session_delete`، `handle_connect_ws`،
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
-DTOs: `ConnectSessionRequest`، `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)،
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- ربط جهاز التوجيه: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- الاختبارات: `crates/iroha_torii/tests/connect_gating.rs` (ميزة البوابات، دورة حياة الجلسة، المصافحة WS) ذ
  غطاء مصفوفة ميزات جهاز التوجيه (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- المالك: Nexus Connect WG.
- الملاحظات: يتم تحديد حدود المعدلات عبر `limits::rate_limit_key`؛ يتم توفير أجهزة قياس القياس عن بعد `connect.*`.

### قياس التتابع عن بعد كايجي - كوبيرتو
- المعالجات: `handle_v1_kaigi_relays`، `handle_v1_kaigi_relay_detail`،
  `handle_v1_kaigi_relays_health`، `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
-DTOs: `KaigiRelaySummaryDto`، `KaigiRelaySummaryListDto`،
  `KaigiRelayDetailDto`، `KaigiRelayDomainMetricsDto`،
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- ربط جهاز التوجيه: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- الاختبارات: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- الملاحظات: يقوم البث SSE بإعادة استخدام القناة العالمية للبث أثناء تطبيق بوابة ملف القياس عن بعد؛ يتم توثيق طلبات الإجابة على `docs/source/torii/kaigi_telemetry_api.md`.

## استئناف تغطية الاختبار- تضمن الاختبارات التجريبية لجهاز التوجيه (`crates/iroha_torii/tests/router_feature_matrix.rs`) تسجيل مجموعات الميزات كل مرة وأن جيل OpenAPI يظل متزامنًا.
- مجموعات خاصة بنقاط النهاية تشمل استعلامات الحسابات، ودائرة العقود الحيوية، وأزرار التحقق ZK، ومرشحات إثبات SSE، وميزات Nexus Connect.
- أدوات تطوير البرامج (SDK) (JavaScript وSwift وPython) وتستخدم الاسم المستعار VOPRF ونقاط النهاية SSE؛ لا يتطلب الأمر عملاً إضافيًا.

## حافظ على هذا المظهر المحدث

يتم تحديث هذه الصفحة والمستمع (`docs/source/torii/app_api_parity_audit.md`) عند تغيير إعدادات التطبيق API الخاصة بـ Torii حتى يتمكن مالكو SDK والقراء الخارجيون من الاتصال بالإنترنت.