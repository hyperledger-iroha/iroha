---
lang: ur
direction: rtl
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: torii-app-api-parity
title: Torii ایپ API برابری کا آڈٹ
description: TORII-APP-1 جائزے کی نقل تاکہ SDK اور پلیٹ فارم ٹیمیں عوامی کوریج کی تصدیق کر سکیں۔
---

حیثیت: مکمل 2026-03-21  
مالکان: Torii Platform، SDK Program Lead  
روڈمیپ حوالہ: TORII-APP-1 — `app_api` برابری آڈٹ

یہ صفحہ اندرونی `TORII-APP-1` آڈٹ (`docs/source/torii/app_api_parity_audit.md`) کی عکاسی کرتا ہے تاکہ مونو-ریپو سے باہر کے قارئین دیکھ سکیں کہ کون سی `/v2/*` سطحیں وائرڈ، ٹیسٹ شدہ اور دستاویزی ہیں۔ یہ آڈٹ ان راستوں کو ٹریک کرتا ہے جو `Torii::add_app_api_routes`، `add_contracts_and_vk_routes` اور `add_connect_routes` کے ذریعے دوبارہ ایکسپورٹ ہوتے ہیں۔

## دائرہ کار اور طریقہ

آڈٹ `crates/iroha_torii/src/lib.rs:256-522` میں عوامی ری ایکسپورٹس اور feature gating والے روٹ بلڈرز کا جائزہ لیتا ہے۔ روڈمیپ میں ہر `/v2/*` سطح کے لئے ہم نے درج ذیل تصدیق کی:

- `crates/iroha_torii/src/routing.rs` میں handler کی نفاذ اور DTO تعریفات۔
- `app_api` یا `connect` فیچر گروپس کے تحت روٹر رجسٹریشن۔
- موجودہ انٹیگریشن/یونٹ ٹیسٹس اور طویل مدتی کوریج کے ذمہ دار ٹیم۔

اکاؤنٹ اثاثہ/ٹرانزیکشن فہرستیں اور اثاثہ ہولڈر لسٹنگز موجودہ pagination/backpressure حدود کے علاوہ پری فلٹرنگ کے لئے اختیاری `asset_id` query پیرامیٹرز قبول کرتی ہیں۔

## تصدیق اور کینونیکل دستخط

- ایپ کے لئے GET/POST اینڈپوائنٹس اختیاری کینونیکل ریکوئسٹ ہیڈرز (`X-Iroha-Account`, `X-Iroha-Signature`) قبول کرتے ہیں جو `METHOD\n/path\nsorted_query\nsha256(body)` سے بنائے جاتے ہیں؛ Torii انہیں executor ویلیڈیشن سے پہلے `QueryRequestWithAuthority` میں لپیٹتا ہے تاکہ یہ `/query` کی عکاسی کریں۔
- SDK ہیلپرز تمام بنیادی کلائنٹس میں دستیاب ہیں:
  - JS/TS: `canonicalRequest.js` سے `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`.
  - Swift: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`.
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`.
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

## اینڈپوائنٹ فہرست

### اکاؤنٹ اجازتیں (`/v2/accounts/{id}/permissions`) — کورڈ
- Handler: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTOs: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: `crates/iroha_torii/tests/accounts_endpoints.rs:126` اور `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`.
- Owner: Torii Platform.
- Notes: جواب Norito JSON body ہے جس میں `items`/`total` ہے، جو SDK pagination helpers سے مطابقت رکھتا ہے۔

### Alias OPRF تشخیص (`POST /v2/aliases/voprf/evaluate`) — کورڈ
- Handler: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTOs: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- Router binding: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- Tests: handler کے inline tests (`crates/iroha_torii/src/lib.rs:9945-9986`) کے ساتھ SDK کوریج
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- Owner: Torii Platform.
- Notes: جواب کی سطح deterministic hex اور backend identifiers نافذ کرتی ہے؛ SDKs اس DTO کو استعمال کرتے ہیں۔

### Proof SSE ایونٹس (`GET /v2/events/sse`) — کورڈ
- Handler: `handle_v1_events_sse` فلٹر سپورٹ کے ساتھ (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTOs: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) اور proof filter wiring.
- Router binding: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- Tests: proof مخصوص SSE suites (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) اور pipeline SSE smoke test
  (`integration_tests/tests/events/sse_smoke.rs`).
- Owner: Torii Platform (runtime), Integration Tests WG (fixtures).
- Notes: proof filter راستے end-to-end توثیق شدہ ہیں؛ دستاویزات `docs/source/zk_app_api.md` میں ہیں۔

### کنٹریکٹ لائف سائیکل (`/v2/contracts/*`) — کورڈ
- Handlers: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTOs: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: router/integration suites `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`.
- Owner: Smart Contract WG کے ساتھ Torii Platform.
- Notes: اینڈپوائنٹس سائن شدہ ٹرانزیکشنز کو قطار میں ڈالते ہیں اور مشترکہ ٹیلیمیٹری میٹرکس (`handle_transaction_with_metrics`) کو دوبارہ استعمال کرتے ہیں۔

### ویریفائنگ کی لائف سائیکل (`/v2/zk/vk/*`) — کورڈ
- Handlers: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) اور `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTOs: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- Router binding: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- Tests: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`.
- Owner: ZK Working Group کے ساتھ Torii Platform سپورٹ۔
- Notes: DTOs Norito schemas کے مطابق ہیں جنہیں SDKs ریفرنس کرتے ہیں؛ rate limiting `limits.rs` کے ذریعے نافذ ہے۔

### Nexus Connect (`/v2/connect/*`) — کورڈ (feature `connect`)
- Handlers: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTOs: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- Router binding: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- Tests: `crates/iroha_torii/tests/connect_gating.rs` (feature gating, سیشن لائف سائیکل، WS handshake) اور
  router feature matrix کوریج (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- Owner: Nexus Connect WG.
- Notes: rate limit keys `limits::rate_limit_key` کے ذریعے ٹریک ہوتے ہیں؛ ٹیلیمیٹری counters `connect.*` میٹرکس کو فیڈ کرتے ہیں۔

### Kaigi relay ٹیلیمیٹری — کورڈ
- Handlers: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTOs: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- Router binding: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- Tests: `crates/iroha_torii/tests/kaigi_endpoints.rs`.
- Notes: SSE stream عالمی broadcast چینل کو دوبارہ استعمال کرتا ہے جبکہ ٹیلیمیٹری پروفائل gating نافذ کرتا ہے؛ response schemas `docs/source/torii/kaigi_telemetry_api.md` میں دستاویزی ہیں۔

## ٹیسٹ کوریج خلاصہ

- Router smoke tests (`crates/iroha_torii/tests/router_feature_matrix.rs`) یقینی بناتے ہیں کہ feature combinations ہر route رجسٹر کریں اور OpenAPI generation sync میں رہے۔
- Endpoint مخصوص suites اکاؤنٹ queries، contract lifecycle، ZK verifying keys، proof SSE filters اور Nexus Connect رویوں کو کور کرتے ہیں۔
- SDK parity harnesses (JavaScript, Swift, Python) پہلے سے Alias VOPRF اور SSE endpoints استعمال کرتے ہیں؛ اضافی کام درکار نہیں۔

## اس مرآۃ کو اپ ٹو ڈیٹ رکھنا

جب Torii app API کا رویہ بدلے تو اس صفحے اور سورس آڈٹ (`docs/source/torii/app_api_parity_audit.md`) کو اپ ڈیٹ کریں تاکہ SDK مالکان اور بیرونی قارئین ہم آہنگ رہیں۔
