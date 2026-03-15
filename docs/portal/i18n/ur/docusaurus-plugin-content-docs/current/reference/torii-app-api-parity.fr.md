---
lang: ur
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: torii-app-api-parity
عنوان: درخواست API پیریٹی آڈٹ Torii
تفصیل: عوامی کوریج کی تصدیق کے ل S SDK اور پلیٹ فارم ٹیموں کے لئے Torii-App-1 جائزہ آئینہ۔
---

حیثیت: مکمل 2026-03-21  
دیکھ بھال کرنے والے: Torii پلیٹ فارم ، SDK پروگرام لیڈ  
روڈ میپ حوالہ: torii-app-1-پیریٹی آڈٹ `app_api`

یہ صفحہ داخلی آڈٹ `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) کی عکاسی کرتا ہے تاکہ مونو ریپو سے باہر کے قارئین دیکھ سکیں کہ کون سی سطح `/v2/*` وائرڈ ، ٹیسٹ اور دستاویزی دستاویزات ہیں۔ آڈٹ `Torii::add_app_api_routes` ، `add_contracts_and_vk_routes` ، اور `add_connect_routes` کے ذریعے دوبارہ برآمد ہونے والے راستوں کی پیروی کرتا ہے۔

## دائرہ کار اور طریقہ

آڈٹ `crates/iroha_torii/src/lib.rs:256-522` میں عوامی دوبارہ برآمدات کا معائنہ کرتا ہے اور روڈ بلڈروں کو فیچر گیٹنگ سے مشروط کیا جاتا ہے۔ روڈ میپ کی ہر سطح `/v2/*` کے لئے ، ہم نے چیک کیا:

- `crates/iroha_torii/src/routing.rs` میں ہینڈلر اور ڈی ٹی او تعریفوں کا نفاذ۔
- فیچر گروپس `app_api` یا `connect` کے تحت روٹر کی رجسٹریشن۔
- موجودہ انضمام/یونٹ ٹیسٹ اور طویل مدتی کوریج کے لئے ذمہ دار ٹیم۔

اثاثہ کی فہرستیں/اکاؤنٹ کے لین دین اور اثاثہ مالک کی فہرستیں موجودہ پیجنگ/بیک پریسچر کی حدود کے علاوہ ، پہلے سے فلٹرنگ کے لئے اختیاری `asset_id` استفسار پیرامیٹرز کو قبول کرتی ہیں۔

## توثیق اور کیننیکل دستخط

- ایپس کے سامنے آنے والے/پوسٹ اختتامی مقامات کو اختیاری کیننیکل درخواست ہیڈرز (`X-Iroha-Account` ، `X-Iroha-Signature`) کو `METHOD\n/path\nsorted_query\nsha256(body)` سے تعمیر کیا گیا ہے۔ Torii ان کو `QueryRequestWithAuthority` میں لپیٹ کر `/query` کی عکاسی کرنے کے لئے ایگزیکٹر کی توثیق سے پہلے۔
- تمام بڑے کلائنٹوں میں ایس ڈی کے مددگار مہیا کیے گئے ہیں:
  - JS/TS: `canonicalRequest.js` سے `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`۔
  - سوئفٹ: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`۔
  - Android (کوٹلن/جاوا): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`۔
- مثالیں:
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

## اختتامی انوینٹری

### اکاؤنٹ کی اجازت (`/v2/accounts/{id}/permissions`) - احاطہ کرتا ہے
- ہینڈلر: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)۔
- DTOS: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)۔
- روٹر بائنڈنگ: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)۔
- ٹیسٹ: `crates/iroha_torii/tests/accounts_endpoints.rs:126` اور `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`۔
- مالک: Torii پلیٹ فارم۔
۔

### عرف او پی آر ایف تشخیص (`POST /v2/aliases/voprf/evaluate`) - احاطہ کرتا ہے
- ہینڈلر: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)۔
- DTOS: `AliasVoprfEvaluateRequestDto` ، `AliasVoprfEvaluateResponseDto` ، `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)۔
- روٹر بائنڈنگ: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)۔
- ٹیسٹ: ہینڈلر کے ان لائن ٹیسٹ (`crates/iroha_torii/src/lib.rs:9945-9986`) پلس SDK کوریج
  (`javascript/iroha_js/test/toriiClient.test.js:72`)۔
- مالک: Torii پلیٹ فارم۔
- نوٹ: ردعمل کی سطح ایک عین مطابق ہیکس اور پسدید شناخت کاروں کو مسلط کرتی ہے۔ ایس ڈی کے ڈی ٹی او کا استعمال کرتے ہیں۔### ایس ایس ای پروف ایونٹس (`GET /v2/events/sse`) - احاطہ کرتا ہے
- ہینڈلر: `handle_v1_events_sse` فلٹر سپورٹ (`crates/iroha_torii/src/routing.rs:14008-14133`) کے ساتھ۔
- DTOS: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) پلس پروف فلٹر وائرنگ۔
- روٹر بائنڈنگ: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)۔
- ٹیسٹ: مخصوص ایس ایس ای پروف سویٹس (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs` ،
  `sse_proof_callhash.rs` ، `sse_proof_verified_fields.rs` ، `sse_proof_rejected_fields.rs`) اور پائپ لائن ایس ایس ای دھواں ٹیسٹ
  (`integration_tests/tests/events/sse_smoke.rs`)۔
- مالک: Torii پلیٹ فارم (رن ٹائم) ، انضمام ٹیسٹ WG (فکسچر)۔
-نوٹ: پروف فلٹر کے راستے درست اختتام سے آخر تک ہیں۔ دستاویزات `docs/source/zk_app_api.md` میں مل سکتی ہیں۔

### معاہدہ لائف سائیکل (`/v2/contracts/*`) - احاطہ کرتا ہے
- ہینڈلرز: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`) ،
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`) ،
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`) ،
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`) ،
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)۔
- DTOS: `DeployContractDto` ، `DeployAndActivateInstanceDto` ، `ActivateInstanceDto` ، `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`)۔
- روٹر بائنڈنگ: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)۔
- ٹیسٹ: روٹر/انضمام سوٹ `contracts_deploy_integration.rs` ، `contracts_activate_integration.rs` ،
  `contracts_instance_activate_integration.rs` ، `contracts_call_integration.rs` ،
  `contracts_instances_list_router.rs`۔
- مالک: Torii پلیٹ فارم کے ساتھ سمارٹ معاہدہ WG۔
- نوٹ: اختتامی نقطہ قطار پر دستخط شدہ لین دین اور مشترکہ ٹیلی میٹری میٹرکس (`handle_transaction_with_metrics`) کو دوبارہ استعمال کریں۔

### توثیق کلیدی لائف سائیکل (`/v2/zk/vk/*`) - احاطہ کرتا ہے
- ہینڈلرز: `handle_post_vk_register` ، `handle_post_vk_update` ، `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) اور `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)۔
- DTOS: `ZkVkRegisterDto` ، `ZkVkUpdateDto` ، `ZkVkDeprecateDto` ، `VkListQuery` ، `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)۔
- روٹر بائنڈنگ: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)۔
- ٹیسٹ: `crates/iroha_torii/tests/zk_vk_get_integration.rs` ،
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs` ،
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`۔
- مالک: ZK ورکنگ گروپ جس کی حمایت Torii پلیٹ فارم ہے۔
- نوٹ: DTOS SDKs کے ذریعہ حوالہ کردہ اسکیماس Norito کے ساتھ سیدھ میں ہے۔ شرح کو محدود کرنا `limits.rs` کے ذریعے نافذ کیا گیا ہے۔

### Nexus کنیکٹ (`/v2/connect/*`) - احاطہ (خصوصیت `connect`)
- ہینڈلرز: `handle_connect_session` ، `handler_connect_session_delete` ، `handle_connect_ws` ،
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)۔
- DTOS: `ConnectSessionRequest` ، `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`) ،
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)۔
- روٹر بائنڈنگ: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)۔
- ٹیسٹ: `crates/iroha_torii/tests/connect_gating.rs` (فیچر گیٹنگ ، سیشن لائف سائیکل ، ڈبلیو ایس ہینڈ شیک) اور
  روٹر کی خصوصیت میٹرکس کوریج (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`)۔
- مالک: Nexus WG سے رابطہ کریں۔
- نوٹ: شرح کی حد کی چابیاں `limits::rate_limit_key` کے ذریعے ٹریک کی جاتی ہیں۔ ٹیلی میٹری کاؤنٹرز `connect.*` میٹرکس کو کھانا کھاتے ہیں۔### کیگی ریلے ٹیلی میٹری - احاطہ کرتا ہے
- ہینڈلرز: `handle_v1_kaigi_relays` ، `handle_v1_kaigi_relay_detail` ،
  `handle_v1_kaigi_relays_health` ، `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)۔
- DTOS: `KaigiRelaySummaryDto` ، `KaigiRelaySummaryListDto` ،
  `KaigiRelayDetailDto` ، `KaigiRelayDomainMetricsDto` ،
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)۔
- روٹر بائنڈنگ: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)۔
- ٹیسٹ: `crates/iroha_torii/tests/kaigi_endpoints.rs`۔
- نوٹ: ایس ایس ای اسٹریم ٹیلی میٹری پروفائل گیٹنگ کا اطلاق کرتے ہوئے عالمی نشریاتی چینل کو دوبارہ استعمال کرتا ہے۔ ردعمل کے نمونوں کو `docs/source/torii/kaigi_telemetry_api.md` میں دستاویزی کیا گیا ہے۔

## ٹیسٹ کوریج کا خلاصہ

- راؤٹر سگریٹ نوشی کی جانچ (`crates/iroha_torii/tests/router_feature_matrix.rs`) اس بات کو یقینی بناتا ہے کہ خصوصیت کے امتزاج ہر راستے میں رجسٹر ہوں اور یہ کہ OpenAPI نسل ہم آہنگی میں ہے۔
- اختتامی نقطہ سوٹ اکاؤنٹ کے سوالات ، معاہدہ لائف سائیکل ، زیڈ کے توثیق کی چابیاں ، ایس ایس ای پروف فلٹرز اور Nexus سے منسلک طرز عمل کا احاطہ کرتا ہے۔
- ایس ڈی کے پیریٹی ہارنس (جاوا اسکرپٹ ، سوئفٹ ، ازگر) پہلے ہی عرف ووپرف اور ایس ایس ای کے اختتامی مقامات کا استعمال کرتے ہیں۔ کسی اضافی کام کی ضرورت نہیں ہے۔

## اس آئینے کو تازہ ترین رکھیں

اس صفحے اور آڈٹ ماخذ (`docs/source/torii/app_api_parity_audit.md`) کو اپ ڈیٹ کریں جب API APP Torii کا طرز عمل تبدیل ہوجاتا ہے تاکہ SDK مالکان اور بیرونی کھلاڑی منسلک رہیں۔