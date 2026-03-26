---
lang: he
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: torii-app-api-parity
title: تدقيق تكافؤ واجهة تطبيق Torii
description: نسخة مرآة لمراجعة TORII-APP-1 حتى تتمكن فرق SDK والمنصة من تأكيد التغطية العامة.
---

الحالة: مكتمل 2026-03-21  
מקור: Torii Platform, מוביל תוכנית SDK  
مرجع خارطة الطريق: TORII-APP-1 — تدقيق تكافؤ `app_api`

تعكس هذه الصفحة تدقيق `TORII-APP-1` الداخلي (`docs/source/torii/app_api_parity_audit.md`) حتى يتمكن القراء خارج المستودع الاحادي من معرفة اي اسطح `/v1/*` موصولة ومختبرة وموثقة. يتتبع التدقيق المسارات المعاد تصديرها عبر `Torii::add_app_api_routes` و`add_contracts_and_vk_routes` و`add_connect_routes`.

## النطاق والمنهج

يفحص التدقيق عمليات اعادة التصدير العامة في `crates/iroha_torii/src/lib.rs:256-522` وبناة المسارات المحمية بالميزات. ولكل سطح `/v1/*` في خارطة الطريق تحققنا من:

- تنفيذ المعالج وتعريفات DTO في `crates/iroha_torii/src/routing.rs`.
- تسجيل الموجه ضمن مجموعات الميزات `app_api` او `connect`.
- اختبارات التكامل/الوحدة الموجودة والفريق المسؤول عن التغطية طويلة الاجل.

טלפון סלולרי/תקשורת טלפון ותקשורת טלפון סלולרית للتصفية المسبقة، بالإضافة إلى حدود الترقيم/الضغط العكسي الحالية.

## المصادقة والتوقيع القياسي

- نقاط النهاية GET/POST الموجهة للتطبيقات تقبل رؤوس طلب قياسية اختيارية (`X-Iroha-Account`, `X-Iroha-Signature`) مبنية من `METHOD\n/path\nsorted_query\nsha256(body)`؛ يقوم Torii بتغليفها في `QueryRequestWithAuthority` قبل تحقق executor لتطابق `/query`.
- تتوفر مساعدات SDK في جميع العملاء الرئيسيين:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` ומין `canonicalRequest.js`.
  - סוויפט: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - אנדרואיד (קוטלין/ג'אווה): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
- מידע:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "soraカタカナ...", method: "get", path: "/v1/accounts/soraカタカナ.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/soraカタカナ.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "soraカタカナ...",
                                                  method: "get",
                                                  path: "/v1/accounts/soraカタカナ.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("soraカタカナ...", "get", "/v1/accounts/soraカタカナ.../assets", "limit=5", ByteArray(0), signer)
```

## جرد نقاط النهاية

### اذونات الحساب (`/v1/accounts/{id}/permissions`) — مغطى
- מידע: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- ערך מקור: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- מוצרים: `crates/iroha_torii/tests/accounts_endpoints.rs:126` ו-`crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- פלטפורמה: Torii.
- ملاحظات: الاستجابة هي جسم JSON Norito مع `items`/`total`، بما يتطابق مع مساعدات ترقيم الصفحات في SDK.

### تقييم OPRF للاسماء المستعارة (`POST /v1/aliases/voprf/evaluate`) — مغطى
- מידע: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- ערך מקור: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- الاختبارات: اختبارات inline للمعالج (`crates/iroha_torii/src/lib.rs:9945-9986`) بالاضافة الى تغطية SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- פלטפורמה: Torii.
- ملاحظات: واجهة الاستجابة تفرض hex محدد وهوية backend؛ وتستهلك SDK الDTO.

### احداث proof عبر SSE (`GET /v1/events/sse`) — مغطى
- المعالج: `handle_v1_events_sse` مع دعم الفلاتر (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) مع توصيل فلتر proof.
- ערך מקור: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- الاختبارات: حزم SSE خاصة بالproof (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) واختبار smoke لSSE في خط الانابيب
  (`integration_tests/tests/events/sse_smoke.rs`).
- תוכן: Torii Platform (זמן ריצה), בדיקות אינטגרציה WG (אביזרי).
- ملاحظات: تم التحقق من مسارات فلتر proof طرفا لطرف؛ والتوثيق موجود في `docs/source/zk_app_api.md`.### دورة حياة العقود (`/v1/contracts/*`) — مغطى
- מחיר: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- ערך מקור: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- الاختبارات: حزم router/integration `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- المالك: Smart Contract WG مع Torii Platform.
- ملاحظات: نقاط النهاية تضع المعاملات الموقعة في قائمة انتظار وتعيد استخدام مقاييس التليمترية المشتركة (`handle_transaction_with_metrics`).

### دورة حياة مفاتيح التحقق (`/v1/zk/vk/*`) — مغطى
- المعالجات: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) ו`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- ערך מקור: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- מוצר: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- المالك: ZK Working Group مع دعم Torii Platform.
- ملاحظات: تتطابق DTOs مع مخططات Norito التي تعتمد عليها SDKs؛ מגבלת תעריף של `limits.rs`.

### Nexus Connect (`/v1/connect/*`) — مغطى (feature `connect`)
- المعالجات: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- ערך מקור: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- الاختبارات: `crates/iroha_torii/tests/connect_gating.rs` (feature gating، دورة حياة الجلسة، handshake WS) و
  تغطية مصفوفة ميزات الموجه (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- מדינה: Nexus Connect WG.
- ملاحظات: يتم تتبع مفاتيح rate limit عبر `limits::rate_limit_key`؛ وتغذي عدادات التليمترية مقاييس `connect.*`.

### تليمترية مرحلات Kaigi — مغطى
- المعالجات: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- ערך מקור: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- מוצר: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- ملاحظات: يعيد بث SSE استخدام القناة العامة للبث مع فرض بوابة ملف تليمترية؛ وتوثق مخططات الاستجابة في `docs/source/torii/kaigi_telemetry_api.md`.

## ملخص تغطية الاختبارات

- اختبارات smoke للراوتر (`crates/iroha_torii/tests/router_feature_matrix.rs`) تضمن ان تركيبات الميزات تسجل كل مسار وان توليد OpenAPI يبقى متزامنا.
- تغطي الحزم الخاصة بنقاط النهاية استعلامات الحسابات ودورة حياة العقود ومفاتيح التحقق ZK وفلاتر proof SSE وسلوك Nexus Connect.
- ادوات تكافؤ SDK (JavaScript, Swift, Python) تستهلك بالفعل Alias ​​VOPRF ونقاط SSE؛ ولا يلزم عمل اضافي.

## الحفاظ على تحديث هذه المرآة

حدّث هذه الصفحة وتدقيق المصدر (`docs/source/torii/app_api_parity_audit.md`) عند تغير سلوك واجهة تطبيق Torii حتى يبقى مالكو SDK والقراء الخارجيون على نفس الخط.