---
lang: ja
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-parity
title: Torii ایپ API برابری کا آڈٹ
説明: TORII-APP-1 جائزے کی نقل تاکہ SDK اور پلیٹ فارم ٹیمیں عوامی کوریج کی تصدیق کر سکیں۔
---

ニュース: مکمل 2026-03-21  
担当者: Torii プラットフォーム SDK プログラム リード  
テスト: TORII-APP-1 — `app_api` テスト

یہ صفحہ اندرونی `TORII-APP-1` آڈٹ (`docs/source/torii/app_api_parity_audit.md`) کی عکاسی کرتا ہے تاکہ مونو-ریپو سے باہر کے قارئین دیکھ سکیں کہ کون سی `/v1/*` سطحیں وائرڈ، ٹیسٹ شدہ اور دستاویزیやあیہ آڈٹ انراستوں کو ٹریک کرتا ہے جو `Torii::add_app_api_routes`، `add_contracts_and_vk_routes` اور `add_connect_routes` کے ذریعے دوبارہ ایکسپورٹ ہوتے ہیں۔

## دائرہ کار اور طریقہ

آڈٹ `crates/iroha_torii/src/lib.rs:256-522` میں عوامی ری ایکسپورٹس اور 機能ゲート والے روٹ بلڈرز کا جائزہ لیتا ہے۔ روڈمیپ میں ہر `/v1/*` سطح کے لئے ہم نے درج ذیل تصدیق کی:

- `crates/iroha_torii/src/routing.rs` ハンドラー فاذ اور DTO تعریفات۔
- `app_api` یا `connect` فیچر گروپس کے تحت روٹر رجسٹریشن۔
- موجودہ انٹیگریشن/یونٹ ٹیسٹس اور طویل مدتی کوریج کے ذمہ دار ٹیم۔

自動ページネーション/バックプレッシャーページネーション/バックプレッシャーページネーション/バックプレッシャپری فلٹرنگ کے لئے اختیاری `asset_id` クエリ پیرامیٹرز قبول کرتی ہیں۔

## صدیق اور کینونیکل دستخط

- 取得/投稿 取得/投稿 取得/投稿 取得/投稿 取得/投稿 取得/投稿قبول کرتے ہیں جو `METHOD\n/path\nsorted_query\nsha256(body)` سے بنائے جاتے ہیں؛ Torii انہیں executor ویلیڈیشن سے پہلے `QueryRequestWithAuthority` میں لپیٹتا ہے تاکہ یہ `/query` کی और देखें
- SDK のバージョン:
  - JS/TS: `canonicalRequest.js` と `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`。
  - スイフト: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`。
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`。
- 意味:
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

## ありがとうございます

### اکاؤنٹ اجازتیں (`/v1/accounts/{id}/permissions`) — کورڈ
- ハンドラー: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)。
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)。
- ルーター バインディング: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- テスト: `crates/iroha_torii/tests/accounts_endpoints.rs:126` と `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`。
- 所有者: Torii プラットフォーム。
- 注: Norito JSON ボディ ہے جس میں `items`/`total` ہے، جو SDK ページネーション ヘルパー سے مطابقت رکھتا ہے۔

### エイリアス OPRF 評価 (`POST /v1/aliases/voprf/evaluate`) — کورڈ
- ハンドラー: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)。
- DTO: `AliasVoprfEvaluateRequestDto`、`AliasVoprfEvaluateResponseDto`、`AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)。
- ルーター バインディング: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)。
- テスト: ハンドラー インライン テスト (`crates/iroha_torii/src/lib.rs:9945-9986`) SDK のテスト
  (`javascript/iroha_js/test/toriiClient.test.js:72`)。
- 所有者: Torii プラットフォーム。
- 注: 決定論的な 16 進数、バックエンド識別子、およびバックエンド識別子。 SDK と DTO の組み合わせ

### 証明 SSE ایونٹس (`GET /v1/events/sse`) — کورڈ
- ハンドラー: `handle_v1_events_sse` فلٹر سپورٹ کے ساتھ (`crates/iroha_torii/src/routing.rs:14008-14133`)。
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) 安全フィルター配線。
- ルーター バインディング: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- テスト: SSE スイートの証明 (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`、
  `sse_proof_callhash.rs`、`sse_proof_verified_fields.rs`、`sse_proof_rejected_fields.rs`) パイプライン SSE 煙テスト
  (`integration_tests/tests/events/sse_smoke.rs`)。
- 所有者: Torii プラットフォーム (ランタイム)、統合テスト WG (フィクスチャ)。
- 注: プルーフ フィルターのエンドツーエンドのテスト。 دستاویزات `docs/source/zk_app_api.md` میں ہیں۔

### کنٹریکٹ لائف سائیکل (`/v1/contracts/*`) — کورڈ
- ハンドラー: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)、
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`)、
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`)、
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`)、
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)。
- DTO: `DeployContractDto`、`DeployAndActivateInstanceDto`、`ActivateInstanceDto`、`ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`)。
- ルーター バインディング: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- テスト: ルーター/統合スイート `contracts_deploy_integration.rs`、`contracts_activate_integration.rs`、
  `contracts_instance_activate_integration.rs`、`contracts_call_integration.rs`、
  `contracts_instances_list_router.rs`。
- 所有者: スマート コントラクト WG Torii プラットフォーム。
- メモ: اینڈپوائنٹس سائن شدہ ٹرانزیکشنز کو قطار میں ڈالते ہیں اور مشترکہ ٹیلیمیٹری میٹرکس (`handle_transaction_with_metrics`) کو دوبارہ استعمال کرتے ہیں۔

### ویریفائنگ کی لائف سائیکل (`/v1/zk/vk/*`) — کورڈ
- ハンドラー: `handle_post_vk_register`、`handle_post_vk_update`、`handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)。
- DTO: `ZkVkRegisterDto`、`ZkVkUpdateDto`、`ZkVkDeprecateDto`、`VkListQuery`、`ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)。
- ルーター バインディング: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- テスト: `crates/iroha_torii/tests/zk_vk_get_integration.rs`、
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`、
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`。
- 所有者: ZK ワーキング グループ Torii プラットフォーム
- 注: DTO Norito スキーマ مطابق ہیں جنہیں SDK ریفرنس کرتے ہیں؛レート制限 `limits.rs` ذریعے نافذ ہے۔### Nexus 接続 (`/v1/connect/*`) — کورڈ (機能 `connect`)
- ハンドラー: `handle_connect_session`、`handler_connect_session_delete`、`handle_connect_ws`、
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)。
- DTO: `ConnectSessionRequest`、`ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)、
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)。
- ルーター バインディング: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)。
- テスト: `crates/iroha_torii/tests/connect_gating.rs` (機能ゲーティング、WS ハンドシェイク)
  ルーター機能マトリックス (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`)。
- 所有者: Nexus WG に接続します。
- 注: レート制限キー `limits::rate_limit_key` کے ذریعے ٹریک ہوتے ہیں؛ ٹیلیمیٹری カウンター `connect.*` میٹرکس کو فیڈ کرتے ہیں۔

### カイギリレー ٹیلیمیٹری — کورڈ
- ハンドラー: `handle_v1_kaigi_relays`、`handle_v1_kaigi_relay_detail`、
  `handle_v1_kaigi_relays_health`、`handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)。
- DTO: `KaigiRelaySummaryDto`、`KaigiRelaySummaryListDto`、
  `KaigiRelayDetailDto`、`KaigiRelayDomainMetricsDto`、
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)。
- ルーターバインディング: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)。
- テスト: `crates/iroha_torii/tests/kaigi_endpoints.rs`。
- 注: SSE ストリーム ブロードキャスト چینل کو دوبارہ استعمال کرتا ہے جبکہ ٹیلیمیٹری پروفائل gating نافذ کرتا ہے؛応答スキーマ `docs/source/torii/kaigi_telemetry_api.md` میں دستاویزی ہیں۔

## ٹیسٹ کوریج خلاصہ

- ルータースモークテスト (`crates/iroha_torii/tests/router_feature_matrix.rs`) 機能の組み合わせ ルート OpenAPI 世代同期 بناتے ہیں کہ 機能の組み合わせ
- エンドポイント スイート、クエリ、契約ライフサイクル、ZK キー検証、証明 SSE フィルター、Nexus 接続、接続
- SDK パリティ ハーネス (JavaScript、Swift、Python) エイリアス VOPRF および SSE エンドポイントضافی کام درکار نہیں۔

## اس مرآۃ کو اپ ٹو ڈیٹ رکھنا

Torii アプリ API を使用する (`docs/source/torii/app_api_parity_audit.md`) を使用する (`docs/source/torii/app_api_parity_audit.md`) を使用するSDK の開発、開発、開発、開発、開発、開発、開発