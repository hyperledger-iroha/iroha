---
lang: ur
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: torii-app-api-parity
عنوان: Torii ایپ API پیریٹی آڈٹ
تفصیل: عوامی کوریج کی تصدیق کے ل S SDK اور پلیٹ فارم ٹیموں کے لئے Torii-App-1 جائزہ کا آئینہ۔
---

حیثیت: مکمل 2026-03-21  
ذمہ دار: Torii پلیٹ فارم ، SDK پروگرام لیڈ  
روڈ میپ حوالہ: torii-app-1-پیریٹی آڈٹ `app_api`

یہ صفحہ `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) داخلی آڈٹ کی آئینہ دار ہے تاکہ مونو ریپو سے باہر کے قارئین دیکھ سکیں کہ کون سے `/v1/*` کی سطحیں منسلک ، جانچ اور دستاویزی ہیں۔ آڈٹ `Torii::add_app_api_routes` ، `add_contracts_and_vk_routes` اور `add_connect_routes` کے ذریعے دوبارہ برآمد کردہ راستوں کی پیروی کرتا ہے۔

## دائرہ کار اور طریقہ

آڈٹ `crates/iroha_torii/src/lib.rs:256-522` میں عوامی دوبارہ برآمدات کا معائنہ کرتا ہے اور فیچر گیٹنگ کے ساتھ روٹ بلڈروں کا معائنہ کرتا ہے۔ روڈ میپ پر ہر سطح `/v1/*` کے لئے ہم چیک کرتے ہیں:

- `crates/iroha_torii/src/routing.rs` میں ہینڈلر اور ڈی ٹی او تعریفوں کا نفاذ۔
- فیچر گروپس `app_api` یا `connect` میں روٹر رجسٹریشن۔
- موجودہ انضمام/یونٹ ٹیسٹ اور طویل مدتی کوریج کے لئے ذمہ دار ٹیم۔

اکاؤنٹ اثاثہ/ٹرانزیکشن اور اثاثہ ہولڈر کی فہرستیں اختیاری `asset_id` کی حمایت کرتے ہیں جو موجودہ پیجنگ/بیک پریسچر کی حدود کے علاوہ پری فلٹرنگ کے لئے استفسار پیرامیٹرز ہیں۔

## توثیق اور کیننیکل دستخط

- ایپس کے مقصد سے حاصل/پوسٹ اختتامی مقامات اختیاری کیننیکل درخواست ہیڈرز (`X-Iroha-Account` ، `X-Iroha-Signature`) کو `METHOD\n/path\nsorted_query\nsha256(body)` سے بنایا گیا ہے۔ Torii ان کو `QueryRequestWithAuthority` میں لپیٹتا ہے اس سے پہلے آئینہ `/query` پر ایگزیکٹر کی توثیق کرنے سے پہلے۔
- تمام بڑے گاہکوں میں ایس ڈی کے مددگار موجود ہیں:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` کا `canonicalRequest.js`۔
  - سوئفٹ: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`۔
  - Android (کوٹلن/جاوا): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`۔
- مثالیں:
```ts
import { buildCanonicalRequestHeaders } from "@iroha2/iroha-js";
const headers = buildCanonicalRequestHeaders({ accountId: "soraカタカナ...", method: "get", path: "/v1/accounts/i105.../assets", query: "limit=5", body: "", privateKey });
await fetch(`${torii}/v1/accounts/i105.../assets?limit=5`, { headers });
```
```swift
let headers = try CanonicalRequest.signingHeaders(accountId: "soraカタカナ...",
                                                  method: "get",
                                                  path: "/v1/accounts/i105.../assets",
                                                  query: "limit=5",
                                                  body: Data(),
                                                  signer: signingKey)
```
```kotlin
val signer = Ed25519Signer(privateKey, publicKey)
val headers = CanonicalRequestSigner.signingHeaders("soraカタカナ...", "get", "/v1/accounts/i105.../assets", "limit=5", ByteArray(0), signer)
```

## اختتامی انوینٹری

### اکاؤنٹ کی اجازت (`/v1/accounts/{id}/permissions`) - احاطہ کرتا ہے
- ہینڈلر: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)۔
- DTOS: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)۔
- روٹر بائنڈنگ: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)۔
- ٹیسٹ: `crates/iroha_torii/tests/accounts_endpoints.rs:126` اور `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`۔
- مالک: Torii پلیٹ فارم۔
- نوٹ: جواب ایک JSON باڈی Norito ہے جس میں `items`/`total` ہے ، SDKs کے پیجنگ مددگاروں کے ساتھ منسلک ہے۔

### عرف او پی آر ایف تشخیص (`POST /v1/aliases/voprf/evaluate`) - احاطہ کرتا ہے
- ہینڈلر: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)۔
- DTOS: `AliasVoprfEvaluateRequestDto` ، `AliasVoprfEvaluateResponseDto` ، `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)۔
- روٹر بائنڈنگ: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)۔
- ٹیسٹ: ان لائن ہینڈلر ٹیسٹ (`crates/iroha_torii/src/lib.rs:9945-9986`) پلس SDK کوریج
  (`javascript/iroha_js/test/toriiClient.test.js:72`)۔
- مالک: Torii پلیٹ فارم۔
- نوٹ: ردعمل کی سطح عصبی ہیکس اور بیک اینڈ شناخت کاروں کو نافذ کرتی ہے۔ ایس ڈی کے ڈی ٹی او کا استعمال کرتے ہیں۔### ایس ایس ای پروف ایونٹس (`GET /v1/events/sse`) - احاطہ کرتا ہے
- ہینڈلر: `handle_v1_events_sse` فلٹر سپورٹ (`crates/iroha_torii/src/routing.rs:14008-14133`) کے ساتھ۔
- DTOS: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) پلس پروف فلٹر وائرنگ۔
- روٹر بائنڈنگ: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)۔
- ٹیسٹ: پروف مخصوص ایس ایس ای سویٹس (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs` ،
  `sse_proof_callhash.rs` ، `sse_proof_verified_fields.rs` ، `sse_proof_rejected_fields.rs`) اور پائپ لائن دھواں ایس ایس ای ٹیسٹ
  (`integration_tests/tests/events/sse_smoke.rs`)۔
- مالک: Torii پلیٹ فارم (رن ٹائم) ، انضمام ٹیسٹ WG (فکسچر)۔
-نوٹ: پروف فلٹر کے راستوں کو اختتام سے آخر تک توثیق کیا گیا تھا۔ دستاویزات `docs/source/zk_app_api.md` پر ہے۔

### معاہدہ لائف سائیکل (`/v1/contracts/*`) - احاطہ کرتا ہے
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

### توثیق کلیدی لائف سائیکل (`/v1/zk/vk/*`) - احاطہ کرتا ہے
- ہینڈلرز: `handle_post_vk_register` ، `handle_post_vk_update` ، `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) اور `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)۔
- DTOS: `ZkVkRegisterDto` ، `ZkVkUpdateDto` ، `ZkVkDeprecateDto` ، `VkListQuery` ، `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)۔
- روٹر بائنڈنگ: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)۔
- ٹیسٹ: `crates/iroha_torii/tests/zk_vk_get_integration.rs` ،
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs` ،
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`۔
- مالک: Torii پلیٹ فارم کے ذریعہ ZK ورکنگ گروپ کی حمایت کی گئی۔
- نوٹ: ڈی ٹی او ایس ایس ڈی کے کے ذریعہ حوالہ کردہ Norito اسکیموں کے ساتھ سیدھ میں ہے۔ شرح کو محدود کرنے کا اطلاق `limits.rs` کے ذریعے کیا جاتا ہے۔

### Nexus کنیکٹ (`/v1/connect/*`) - احاطہ (خصوصیت `connect`)
- ہینڈلرز: `handle_connect_session` ، `handler_connect_session_delete` ، `handle_connect_ws` ،
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)۔
- DTOS: `ConnectSessionRequest` ، `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`) ،
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)۔
- روٹر بائنڈنگ: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)۔
- ٹیسٹ: `crates/iroha_torii/tests/connect_gating.rs` (فیچر گیٹنگ ، سیشن لائف سائیکل ، ڈبلیو ایس ہینڈ شیک) اور
  روٹر کی خصوصیت میٹرکس (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`) کی کوریج۔
- مالک: Nexus WG سے رابطہ کریں۔
- نوٹ: شرح کی حد کی چابیاں `limits::rate_limit_key` کے ذریعے ٹریک کی جاتی ہیں۔ ٹیلی میٹری کاؤنٹرز `connect.*` میٹرکس کو کھانا کھاتے ہیں۔

### کیگی ریلے ٹیلی میٹری - احاطہ کرتا ہے
- ہینڈلرز: `handle_v1_kaigi_relays` ، `handle_v1_kaigi_relay_detail` ،
  `handle_v1_kaigi_relays_health` ، `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)۔
- DTOS: `KaigiRelaySummaryDto` ، `KaigiRelaySummaryListDto` ،
  `KaigiRelayDetailDto` ، `KaigiRelayDomainMetricsDto` ،
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)۔
- روٹر بائنڈنگ: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)۔
- ٹیسٹ: `crates/iroha_torii/tests/kaigi_endpoints.rs`۔
- نوٹ: ایس ایس ای اسٹریم ٹیلی میٹری پروفائل کو گیٹنگ کرتے ہوئے عالمی نشریاتی چینل کو دوبارہ استعمال کرتا ہے۔ رسپانس اسکیموں کو `docs/source/torii/kaigi_telemetry_api.md` میں دستاویزی کیا گیا ہے۔## ٹیسٹ کوریج کا خلاصہ

- روٹر دھواں ٹیسٹ (`crates/iroha_torii/tests/router_feature_matrix.rs`) اس بات کو یقینی بنائیں کہ خصوصیت کے امتزاج تمام راستوں کو رجسٹر کریں اور یہ کہ OpenAPI کی نسل ہم آہنگ ہے۔
- اختتامی مخصوص سوٹ اکاؤنٹ کے سوالات ، کنٹریکٹ لائف سائیکل ، زیڈ کے توثیق کی چابیاں ، ایس ایس ای پروف فلٹرز اور Nexus سے منسلک طرز عمل کا احاطہ کرتے ہیں۔
- ایس ڈی کے پیریٹی ہارنس (جاوا اسکرپٹ ، سوئفٹ ، ازگر) پہلے ہی عرف ووپرف اور ایس ایس ای کے اختتامی مقامات کا استعمال کرتے ہیں۔ کوئی اضافی کام نہیں ہے۔

## اس آئینے کو اپ ڈیٹ رکھیں

اس صفحے اور ماخذ آڈٹ (`docs/source/torii/app_api_parity_audit.md`) کو اپ ڈیٹ کریں جب Torii API ایپ طرز عمل میں تبدیلی آتی ہے تاکہ SDK مالکان اور بیرونی قارئین منسلک ہوجائیں۔