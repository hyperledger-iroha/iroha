---
lang: ur
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: torii-app-api-parity
عنوان: درخواست API پیریٹی آڈٹ Torii
تفصیل: Torii-App-1 کا جائزہ آئینہ تاکہ SDK اور پلیٹ فارم ٹیمیں عوامی کوریج کی تصدیق کرسکیں۔
---

حیثیت: مکمل 2026-03-21  
مالکان: Torii پلیٹ فارم ، SDK پروگرام لیڈ  
روڈ میپ لنک: torii-app-1-پیریٹی آڈٹ `app_api`

یہ صفحہ `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) کے داخلی آڈٹ کی عکاسی کرتا ہے تاکہ مونورپوزٹری سے باہر کے قارئین دیکھ سکیں کہ کون سے `/v1/*` کی سطحیں منسلک ، جانچ اور دستاویزی دستاویزات ہیں۔ آڈٹ `Torii::add_app_api_routes` ، `add_contracts_and_vk_routes` ، اور `add_connect_routes` کے ذریعے دوبارہ برآمد کردہ راستوں پر نظر رکھتا ہے۔

## دائرہ کار اور طریقہ

آڈٹ `crates/iroha_torii/src/lib.rs:256-522` میں عوامی دوبارہ برآمدات کی جانچ پڑتال کرتا ہے اور فیچر گیٹنگ کے ساتھ روٹ بلڈرز۔ روڈ میپ میں ہر سطح `/v1/*` کے لئے ہم نے چیک کیا:

- `crates/iroha_torii/src/routing.rs` میں ہینڈلر اور ڈی ٹی او تعریفوں کا نفاذ۔
- گروپس میں روٹر کی رجسٹریشن میں `app_api` یا `connect` کی خصوصیت ہے۔
- انضمام/یونٹ ٹیسٹوں کی دستیابی اور طویل مدتی کوریج کے لئے ذمہ دار ٹیم۔

اکاؤنٹ اثاثہ/ٹرانزیکشن کی فہرستیں اور اثاثہ ہولڈر کی فہرستیں اختیاری استفسار پیرامیٹرز کو قبول کرتی ہیں `asset_id` پری فلٹرنگ کے لئے ، موجودہ صفحہ بندی/بیک پریسور کی حدود کے علاوہ۔

## توثیق اور کیننیکل دستخط

- ایپلی کیشنز کے ل Get/پوسٹ اختتامی مقامات اختیاری کیننیکل درخواست ہیڈرز (`X-Iroha-Account` ، `X-Iroha-Signature`) کو قبول کریں ، `METHOD\n/path\nsorted_query\nsha256(body)` سے بنایا گیا ہے۔ Torii ان کو `QueryRequestWithAuthority` میں لپیٹتا ہے اس سے پہلے کہ `/query` سے ملنے کے لئے ایگزیکٹر کی توثیق کریں۔
- ایس ڈی کے مددگار تمام بڑے گاہکوں میں دستیاب ہیں:
  - JS/TS: `canonicalRequest.js` سے `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`۔
  - سوئفٹ: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`۔
  - Android (کوٹلن/جاوا): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`۔
- مثالیں:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "<katakana-i105-account-id>", method: "get", path: "/v1/accounts/<katakana-i105-account-id>/assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/<katakana-i105-account-id>/assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "<katakana-i105-account-id>",
                                                  method: "get",
                                                  path: "/v1/accounts/<katakana-i105-account-id>/assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("<katakana-i105-account-id>", "get", "/v1/accounts/<katakana-i105-account-id>/assets", "limit=5", ByteArray(0), signer)
```

## اختتامی انوینٹری

### اکاؤنٹ کی اجازت (`/v1/accounts/{id}/permissions`) - احاطہ کرتا ہے
- ہینڈلر: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)۔
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)۔
- روٹر بائنڈنگ: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)۔
- ٹیسٹ: `crates/iroha_torii/tests/accounts_endpoints.rs:126` اور `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`۔
- مالک: Torii پلیٹ فارم۔
- نوٹ: اس کا جواب Norito JSON کے ساتھ `items`/`total` ہے ، جو SDK صفحہ بندی کے مددگاروں کی طرح ہے۔

### OPRF تشخیص عرف (`POST /v1/aliases/voprf/evaluate`) - احاطہ کرتا ہے
- ہینڈلر: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)۔
- DTO: `AliasVoprfEvaluateRequestDto` ، `AliasVoprfEvaluateResponseDto` ، `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)۔
- روٹر بائنڈنگ: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)۔
- ٹیسٹ: ان لائن ٹیسٹ ہینڈلر (`crates/iroha_torii/src/lib.rs:9945-9986`) پلس SDK کوریج
  (`javascript/iroha_js/test/toriiClient.test.js:72`)۔
- مالک: Torii پلیٹ فارم۔
- نوٹ: ردعمل کی سطح عصبی ہیکس اور بیک اینڈ شناخت کاروں کو نافذ کرتی ہے۔ SDKs DTOS استعمال کرتے ہیں۔### پروف ایس ایس ای واقعات (`GET /v1/events/sse`) - احاطہ کرتا ہے
- ہینڈلر: `handle_v1_events_sse` فلٹر سپورٹ (`crates/iroha_torii/src/routing.rs:14008-14133`) کے ساتھ۔
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) پلس فلٹر وائرنگ۔
- روٹر بائنڈنگ: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)۔
- ٹیسٹ: پروف مخصوص ایس ایس ای سویٹس (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs` ،
  `sse_proof_callhash.rs` ، `sse_proof_verified_fields.rs` ، `sse_proof_rejected_fields.rs`) اور SSE پائپ لائن کا دھواں ٹیسٹ
  (`integration_tests/tests/events/sse_smoke.rs`)۔
- مالک: Torii پلیٹ فارم (رن ٹائم) ، انضمام ٹیسٹ WG (فکسچر)۔
-نوٹ: پروف فلٹر روٹس کا اختتام اختتام سے آخر تک کیا جاتا ہے۔ `docs/source/zk_app_api.md` میں دستاویزات۔

### معاہدہ لائف سائیکل (`/v1/contracts/*`) - احاطہ کرتا ہے
- ہینڈلرز: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`) ،
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`) ،
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`) ،
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`) ،
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)۔
- DTO: `DeployContractDto` ، `DeployAndActivateInstanceDto` ، Torii ، `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`)۔
- روٹر بائنڈنگ: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)۔
- ٹیسٹ: روٹر/انضمام سوٹ `contracts_deploy_integration.rs` ، `contracts_activate_integration.rs` ،
  `contracts_instance_activate_integration.rs` ، `contracts_call_integration.rs` ،
  `contracts_instances_list_router.rs`۔
- مالک: سمارٹ معاہدہ Wg ایک ساتھ Torii پلیٹ فارم کے ساتھ۔
- نوٹ: اختتامی نقطہ قطار پر دستخط شدہ لین دین اور عام ٹیلی میٹری میٹرکس (`handle_transaction_with_metrics`) کا دوبارہ استعمال کریں۔

### توثیق کلیدی لائف سائیکل (`/v1/zk/vk/*`) - احاطہ کرتا ہے
- ہینڈلرز: `handle_post_vk_register` ، `handle_post_vk_update` ، `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) اور `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)۔
- DTO: `ZkVkRegisterDto` ، `ZkVkUpdateDto` ، `ZkVkDeprecateDto` ، `VkListQuery` ، `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)۔
- روٹر بائنڈنگ: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)۔
- ٹیسٹ: `crates/iroha_torii/tests/zk_vk_get_integration.rs` ،
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs` ،
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`۔
- مالک: Torii پلیٹ فارم کی حمایت کے ساتھ ZK ورکنگ گروپ۔
- نوٹ: ڈی ٹی او ایس ایس ڈی کے میں حوالہ کردہ Norito اسکیموں کے مطابق ہیں۔ `limits.rs` کے ذریعے نافذ کردہ شرح کو محدود کرنا۔

### Nexus کنیکٹ (`/v1/connect/*`) - احاطہ (خصوصیت `connect`)
- ہینڈلرز: `handle_connect_session` ، `handler_connect_session_delete` ، `handle_connect_ws` ،
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)۔
- DTO: `ConnectSessionRequest` ، `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`) ،
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)۔
- روٹر بائنڈنگ: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)۔
- ٹیسٹ: `crates/iroha_torii/tests/connect_gating.rs` (فیچر گیٹنگ ، سیشن لائف سائیکل ، ڈبلیو ایس ہینڈ شیک) اور
  روٹر کی خصوصیت میٹرکس (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`) کی کوریج۔
- مالک: Nexus WG سے رابطہ کریں۔
- نوٹ: شرح کی حد کی چابیاں `limits::rate_limit_key` کے ذریعے ٹریک کی جاتی ہیں۔ ٹیلی میٹری میٹرز فیڈ میٹرکس `connect.*`۔

### کیگی ریلے ٹیلی میٹری - احاطہ کرتا ہے
- ہینڈلرز: `handle_v1_kaigi_relays` ، `handle_v1_kaigi_relay_detail` ،
  `handle_v1_kaigi_relays_health` ، `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)۔
- DTO: `KaigiRelaySummaryDto` ، `KaigiRelaySummaryListDto` ،
  `KaigiRelayDetailDto` ، `KaigiRelayDomainMetricsDto` ،
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)۔
- روٹر بائنڈنگ: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)۔
- ٹیسٹ: `crates/iroha_torii/tests/kaigi_endpoints.rs`۔
- نوٹ: ایس ایس ای اسٹریم عالمی نشریاتی چینل کو دوبارہ استعمال کرتا ہے اور ٹیلی میٹری پروفائل گیٹنگ کا اطلاق کرتا ہے۔ ردعمل کے نمونوں کو `docs/source/torii/kaigi_telemetry_api.md` میں بیان کیا گیا ہے۔

## ٹیسٹ کوریج کا خلاصہ- روٹر (`crates/iroha_torii/tests/router_feature_matrix.rs`) کے تمباکو نوشی کے ٹیسٹ اس بات کو یقینی بناتے ہیں کہ خصوصیت کے امتزاج ہر راستے میں رجسٹر ہوں اور OpenAPI ایکس کی نسل ہم آہنگ رہے۔
- اختتامی نقطہ سے متعلق سوٹ اکاؤنٹ کی درخواستوں ، معاہدہ لائف سائیکل ، زیڈ کے توثیق کی چابیاں ، ایس ایس ای پروف فلٹرز اور Nexus کنیکٹ سلوک کا احاطہ کرتا ہے۔
- ایس ڈی کے پیریٹی ہارنس (جاوا اسکرپٹ ، سوئفٹ ، ازگر) پہلے ہی عرف ووپرف اور ایس ایس ای کے اختتامی مقامات کا استعمال کرتے ہیں۔ کسی اضافی کام کی ضرورت نہیں ہے۔

## آئینہ کو تازہ ترین رکھنا

براہ کرم اس صفحے اور اصل آڈٹ (`docs/source/torii/app_api_parity_audit.md`) کو اپ ڈیٹ کریں جب SDK مالکان اور بیرونی قارئین کے مستقل رہنے کو یقینی بنانے کے لئے Torii APP API میں تبدیلی کا سلوک تبدیل ہوتا ہے۔