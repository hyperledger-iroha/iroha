---
lang: ja
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-parity
タイトル: تدقيق تكافؤ واجهة تطبيق Torii
説明: TORII-APP-1 と SDK のバージョン。
---

回答: 2026-03-21  
所属: Torii プラットフォーム SDK プログラム リード  
重要: TORII-APP-1 — تدقيق تكافؤ `app_api`

تعكس هذه الصفحة تدقيق `TORII-APP-1` الداخلي (`docs/source/torii/app_api_parity_audit.md`) حتى يتمكن القراء خارج المستودع `/v1/*` موصولة ومختبرة وموثقة 。 `Torii::add_app_api_routes` و`add_contracts_and_vk_routes` و`add_connect_routes`。

## うーん

يفحص التدقيق عمليات اعادة التصدير العامة في `crates/iroha_torii/src/lib.rs:256-522` وبناة المسارات المحمية بالميزات。テスト `/v1/*` テスト:

- アメリカ合衆国 DTO 国家 `crates/iroha_torii/src/routing.rs`。
- テスト `app_api` テスト `connect`。
- ニュース/ニュース/ニュース/ニュース/ニュース/ニュース/ニュース/ニュース/ニュース。

قوائم أصول/معاملات الحساب وقوائم حاملي الأصول تقبل معاملات استعلام `asset_id` اختياريةログインしてください。

## صادقة والتوقيع القياسي

- 取得/投稿 (`X-Iroha-Account`、`X-Iroha-Signature`)重要 `METHOD\n/path\nsorted_query\nsha256(body)` يقوم Torii بتغليفها في `QueryRequestWithAuthority` قبل تحقق executor لتطابق `/query`.
- SDK のバージョン:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` 、 `canonicalRequest.js`。
  - スイフト: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`。
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`。
- 説明:
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

## और देखें

### اذونات الحساب (`/v1/accounts/{id}/permissions`) — مغطى
- 名前: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)。
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)。
- 番号: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- 回答: `crates/iroha_torii/tests/accounts_endpoints.rs:126` と `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`。
- 名前: Torii プラットフォーム。
- メッセージ: JSON Norito メッセージ `items`/`total` メッセージ メッセージSDK を使用します。

### تقييم OPRF للاسماء المستعارة (`POST /v1/aliases/voprf/evaluate`) — مغطى
- 名前: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)。
- DTO: `AliasVoprfEvaluateRequestDto`、`AliasVoprfEvaluateResponseDto`、`AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)。
- 番号: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)。
- バージョン: インライン バージョン (`crates/iroha_torii/src/lib.rs:9945-9986`) バージョン SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`)。
- 名前: Torii プラットフォーム。
- 意味: واجهة الاستجابة تفرض hex محدد وهوية backend؛ SDK と DTO。

### 証明 SSE (`GET /v1/events/sse`) — مغطى
- メッセージ: `handle_v1_events_sse` メッセージ (`crates/iroha_torii/src/routing.rs:14008-14133`)。
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) の証明。
- 番号: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- 認証: SSE 認証 (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`、
  `sse_proof_callhash.rs`、`sse_proof_verified_fields.rs`、`sse_proof_rejected_fields.rs`) 煙 لSSE في خط الانابيب
  (`integration_tests/tests/events/sse_smoke.rs`)。
- 名前: Torii プラットフォーム (ランタイム)、統合テスト WG (フィクスチャ)。
- 証明: 証明 証明 証明 証明`docs/source/zk_app_api.md`。

### دورة حياة العقود (`/v1/contracts/*`) — مغطى
- 名前: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)、
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`)、
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`)、
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`)、
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)。
- DTO: `DeployContractDto`、`DeployAndActivateInstanceDto`、`ActivateInstanceDto`、`ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`)。
- 番号: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- バージョン: ルーター/統合 `contracts_deploy_integration.rs`、`contracts_activate_integration.rs`、
  `contracts_instance_activate_integration.rs`、`contracts_call_integration.rs`、
  `contracts_instances_list_router.rs`。
- 名前: スマート コントラクト WG Torii プラットフォーム。
- 意味: نقاط النهاية تضع المعاملات الموقعة في قائمة انتظار وتعيد استخدام مقاييس (`handle_transaction_with_metrics`)。

### دورة حياة مفاتيح التحقق (`/v1/zk/vk/*`) — مغطى
- 名前: `handle_post_vk_register`、`handle_post_vk_update`、`handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) و`handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)。
- DTO: `ZkVkRegisterDto`、`ZkVkUpdateDto`、`ZkVkDeprecateDto`、`VkListQuery`、`ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)。
- 番号: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- 番号: `crates/iroha_torii/tests/zk_vk_get_integration.rs`、
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`、
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`。
- 名前: ZK ワーキング グループ Torii プラットフォーム。
- 説明: DTO の説明 Norito の説明 SDK の説明レート制限 `limits.rs`。### Nexus 接続 (`/v1/connect/*`) — مغطى (機能 `connect`)
- 名前: `handle_connect_session`、`handler_connect_session_delete`、`handle_connect_ws`、
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)。
- DTO: `ConnectSessionRequest`、`ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)、
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)。
- 番号: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)。
- バージョン: `crates/iroha_torii/tests/connect_gating.rs` (機能ゲーティング、ハンドシェイク WS)
  (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`) を参照してください。
- メッセージ: Nexus WG に接続します。
- 重要: レート制限 `limits::rate_limit_key`؛ وتغذي عدادات التليمترية مقاييس `connect.*`。

### カイギ — مغطى
- 名前: `handle_v1_kaigi_relays`、`handle_v1_kaigi_relay_detail`、
  `handle_v1_kaigi_relays_health`、`handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)。
- DTO: `KaigiRelaySummaryDto`、`KaigiRelaySummaryListDto`、
  `KaigiRelayDetailDto`、`KaigiRelayDomainMetricsDto`、
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)。
- 名前: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)。
- 番号: `crates/iroha_torii/tests/kaigi_endpoints.rs`。
- ملاحظات: يعيد بث SSE القناة العامة للبث مع فرض بوابة ملف تليمترية؛ وتوثق مخططات الاسجابة في `docs/source/torii/kaigi_telemetry_api.md`。

## ملخص تغطية الاختبارات

- 煙 للراوتر (`crates/iroha_torii/tests/router_feature_matrix.rs`) 煙のようなもの (`crates/iroha_torii/tests/router_feature_matrix.rs`) 煙のようなもの (OpenAPI يبقى)そうです。
- ニュース - ニュース - ニュース - ニュース - ニュース - ニュース - ニュース - ニュース - ニュース - ニュース - ニュース - ニュース - ニュース - ニュース - ニュース - ニュースSSE 証明 Nexus 接続します。
- SDK (JavaScript、Swift、Python) のバージョン、別名 VOPRF および SSE バージョンولا يلزم عمل اضافي。

## الحفاظ على تحديث هذه المرآة

حدّث هذه الصفحة وتدقيق المصدر (`docs/source/torii/app_api_parity_audit.md`) عند تغير سلوك واجهة تطبيق Torii حتى SDK をダウンロードしてください。