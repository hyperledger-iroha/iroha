---
lang: ur
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: torii-app-api-parity
عنوان: Torii ایپلی کیشن انٹرفیس پیریٹی چیک
تفصیل: Torii-App-1 جائزہ کا آئینہ ورژن تاکہ SDK اور پلیٹ فارم ٹیمیں مجموعی طور پر کوریج کی تصدیق کرسکیں۔
---

حیثیت: مکمل 03-21-2026  
مالکان: Torii پلیٹ فارم ، SDK پروگرام لیڈ  
روڈ میپ حوالہ: torii-app-1-`app_api` مساوات آڈٹ

یہ صفحہ داخلی `TORII-APP-1` آڈٹ (`docs/source/torii/app_api_parity_audit.md`) کی عکاسی کرتا ہے تاکہ واحد ذخیرہ سے باہر کے قارئین دیکھ سکیں کہ کون سے `/v1/*` کی سطحیں منسلک ، جانچ اور دستاویزی ہیں۔ آڈٹ `Torii::add_app_api_routes` ، `add_contracts_and_vk_routes` ، اور `add_connect_routes` کے ذریعے دوبارہ برآمد شدہ راستوں کو ٹریک کرتا ہے۔

## دائرہ کار اور نقطہ نظر

آڈٹ `crates/iroha_torii/src/lib.rs:256-522` اور خصوصیت سے محفوظ راستہ بنانے والوں میں عالمی سطح پر دوبارہ برآمدات کی جانچ کرتا ہے۔ روڈ میپ میں ہر سطح `/v1/*` کے لئے ہم نے چیک کیا:

- `crates/iroha_torii/src/routing.rs` میں پروسیسر کے نفاذ اور DTO تعریفیں۔
- روٹر کو فیچر سیٹ `app_api` یا `connect` کے تحت رجسٹر کریں۔
- موجودہ انضمام/یونٹ ٹیسٹ اور طویل مدتی کوریج کے لئے ذمہ دار ٹیم۔

اکاؤنٹ اثاثہ/ٹرانزیکشن کی فہرستیں اور اثاثہ ہولڈر کی فہرستیں اختیاری `asset_id` کو پہلے سے فلٹرنگ کے ل question ، موجودہ نمبر/ریورس کمپریشن کی حدود کے علاوہ ، پہلے سے فلٹرنگ کے ل accept قبول کرتی ہیں۔

## معیاری توثیق اور دستخط

- ایپلی کیشن پر مبنی حاصل/پوسٹ اختتامی مقامات اختیاری معیاری درخواست کے ہیڈرز کو قبول کریں (`X-Iroha-Account` ، `X-Iroha-Signature`) `METHOD\n/path\nsorted_query\nsha256(body)` سے بنایا گیا ہے۔ Torii `QueryRequestWithAuthority` میں اس سے پہلے ایگزیکٹر `/query` سے ملنے کے لئے چیک کرتا ہے۔
- ہیلپر ایس ڈی کے تمام بڑے گاہکوں میں دستیاب ہے:
  - JS/TS: `canonicalRequest.js` سے `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`۔
  - سوئفٹ: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`۔
  - Android (کوٹلن/جاوا): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`۔
- مثالیں:
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

## انوینٹری کے اختتامی مقامات

### اکاؤنٹ کی اجازت (`/v1/accounts/{id}/permissions`) - احاطہ کرتا ہے
- پروسیسر: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)۔
- DTOS: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)۔
- رابطہ روٹر: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)۔
- ٹیسٹ: `crates/iroha_torii/tests/accounts_endpoints.rs:126` اور `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`۔
- مالک: Torii پلیٹ فارم۔
۔

### او پی آر ایف کی تشخیص (`POST /v1/aliases/voprf/evaluate`) - احاطہ کرتا ہے
- پروسیسر: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)۔
- DTOS: `AliasVoprfEvaluateRequestDto` ، `AliasVoprfEvaluateResponseDto` ، `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)۔
- کنیکٹ روٹر: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)۔
- ٹیسٹ: ایس ڈی کے کوریج کے علاوہ پروسیسر (`crates/iroha_torii/src/lib.rs:9945-9986`) کے لئے ان لائن ٹیسٹ
  (`javascript/iroha_js/test/toriiClient.test.js:72`)۔
- مالک: Torii پلیٹ فارم۔
- نوٹ: رسپانس انٹرفیس ایک مخصوص ہیکس اور پسدید شناخت کو نافذ کرتا ہے۔ SDK DTO کھاتا ہے۔

### SSE (`GET /v1/events/sse`) کے ذریعے پروف واقعات - احاطہ کرتا ہے
- پروسیسر: فلٹر سپورٹ (`crates/iroha_torii/src/routing.rs:14008-14133`) کے ساتھ `handle_v1_events_sse`۔
- DTOS: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) پروف فلٹر کنکشن کے ساتھ۔
- کنیکٹ روٹر: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)۔
- ٹیسٹ: ثبوت کے لئے ایس ایس ای پیکیجز (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs` ،
  `sse_proof_callhash.rs` ، `sse_proof_verified_fields.rs` ، `sse_proof_rejected_fields.rs`) اور پائپ لائن میں ایس ایس ای کے لئے دھواں ٹیسٹ
  (`integration_tests/tests/events/sse_smoke.rs`)۔
- مالک: Torii پلیٹ فارم (رن ٹائم) ، انضمام ٹیسٹ WG (فکسچر)۔
-نوٹ: پروف فلٹر کے راستوں کی تصدیق اختتام سے آخر تک کی گئی ہے۔ دستاویزات `docs/source/zk_app_api.md` پر دستیاب ہے۔### معاہدہ لائف سائیکل (`/v1/contracts/*`) - احاطہ کرتا ہے
- پروسیسرز: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`) ،
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`) ،
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`) ،
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`) ،
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)۔
- DTOS: `DeployContractDto` ، `DeployAndActivateInstanceDto` ، `ActivateInstanceDto` ، `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`)۔
- رابطہ روٹر: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)۔
- ٹیسٹ: روٹر/انضمام پیکیجز `contracts_deploy_integration.rs` ، `contracts_activate_integration.rs` ،
  `contracts_instance_activate_integration.rs` ، `contracts_call_integration.rs` ،
  `contracts_instances_list_router.rs`۔
- مالک: Torii پلیٹ فارم کے ساتھ سمارٹ معاہدہ WG۔
- نوٹ: اختتامی نقطہ قطار پر دستخط شدہ لین دین اور مشترکہ ٹیلی میٹکس (`handle_transaction_with_metrics`) کو دوبارہ استعمال کریں۔

### توثیق کی چابیاں (`/v1/zk/vk/*`) کا لائف سائکل
- پروسیسرز: `handle_post_vk_register` ، `handle_post_vk_update` ، `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) اور `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)۔
- DTOS: `ZkVkRegisterDto` ، `ZkVkUpdateDto` ، `ZkVkDeprecateDto` ، `VkListQuery` ، `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)۔
- رابطہ روٹر: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)۔
- ٹیسٹ: `crates/iroha_torii/tests/zk_vk_get_integration.rs` ،
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs` ،
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`۔
- مالک: Torii پلیٹ فارم سپورٹ کے ساتھ ZK ورکنگ گروپ۔
- نوٹ: DTOS Norito اسکیموں سے میل کھاتا ہے جس پر SDKs پر مبنی ہے۔ شرح کی حد کو `limits.rs` کے ذریعے نافذ کیا جاتا ہے۔

### Nexus کنیکٹ (`/v1/connect/*`) - احاطہ (خصوصیت `connect`)
- پروسیسرز: `handle_connect_session` ، `handler_connect_session_delete` ، `handle_connect_ws` ،
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)۔
- DTOS: `ConnectSessionRequest` ، `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`) ،
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)۔
- رابطہ روٹر: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)۔
- ٹیسٹ: `crates/iroha_torii/tests/connect_gating.rs` (فیچر گیٹنگ ، سیشن لائف سائیکل ، ہینڈ شیک WS) اور
  روٹر کی خصوصیت میٹرکس کوریج (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`)۔
- مالک: Nexus WG سے رابطہ کریں۔
- نوٹ: شرح کی حد سوئچ کو `limits::rate_limit_key` کے ذریعے ٹریک کیا جاتا ہے۔ ٹیلی میٹر میٹر `connect.*` میٹر کو کھانا کھاتا ہے۔

### ٹیلی میٹک کیگی ریلے - احاطہ کرتا ہے
- پروسیسرز: `handle_v1_kaigi_relays` ، `handle_v1_kaigi_relay_detail` ،
  `handle_v1_kaigi_relays_health` ، `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)۔
- DTOS: `KaigiRelaySummaryDto` ، `KaigiRelaySummaryListDto` ،
  `KaigiRelayDetailDto` ، `KaigiRelayDomainMetricsDto` ،
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)۔
- رابطہ روٹر: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)۔
- ٹیسٹ: `crates/iroha_torii/tests/kaigi_endpoints.rs`۔
- نوٹ: ایس ایس ای براڈکاسٹ عوامی نشریاتی چینل کو دوبارہ استعمال کرتا ہے جبکہ ٹیلی میٹک فائل گیٹ کو نافذ کرتے ہوئے۔ رسپانس ڈایاگرام کو `docs/source/torii/kaigi_telemetry_api.md` میں دستاویزی کیا گیا ہے۔

## ٹیسٹ کوریج کا خلاصہ

روٹر (`crates/iroha_torii/tests/router_feature_matrix.rs`) کے لئے دھواں ٹیسٹ اس بات کو یقینی بناتے ہیں کہ خصوصیت کے امتزاج ہر راستے کو ریکارڈ کرتے ہیں اور یہ کہ OpenAPI نسل مطابقت پذیری میں رہتی ہے۔
- اختتامی نقطہ سے متعلق پیکیجز اکاؤنٹ کے سوالات ، معاہدہ لائف سائیکل ، زیڈ کے توثیق کی چابیاں ، ایس ایس ای پروف فلٹرز ، اور Nexus کنیکٹ سلوک کا احاطہ کرتے ہیں۔
- ایس ڈی کے پیریٹی ٹولز (جاوا اسکرپٹ ، سوئفٹ ، ازگر) پہلے ہی عرف ووپرف اور ایس ایس ای پوائنٹس کا استعمال کرتے ہیں۔ کسی اضافی کام کی ضرورت نہیں ہے۔

## اس آئینے کو اپ ڈیٹ رکھیں

اس صفحے کو اپ ڈیٹ کریں اور ماخذ (`docs/source/torii/app_api_parity_audit.md`) کو چیک کریں جب Torii API طرز عمل تبدیل ہوجاتا ہے تاکہ SDK مالکان اور بیرونی قارئین اسی صفحے پر رہیں۔