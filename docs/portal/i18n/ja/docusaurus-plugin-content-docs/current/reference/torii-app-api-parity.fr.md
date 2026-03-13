---
lang: ja
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-parity
タイトル: API アプリケーションの監査 Torii
説明: Miroir de la revue TORII-APP-1 は、SDK と couverture public のプレート形式を確認します。
---

法規: 終了 2026-03-21  
責任者: Torii プラットフォーム、SDK プログラム リード  
ロードマップのリファレンス: TORII-APP-1 - パリテ監査 `app_api`

インターネット監査 `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) のページは、ケーブル、試験対象者、および文書対象者を対象としたモノレポートの表面 `/v2/*` を参照します。 `Torii::add_app_api_routes`、`add_contracts_and_vk_routes`、および `add_connect_routes` 経由で再輸出先のルートを監査します。

## ポートと方法

`crates/iroha_torii/src/lib.rs:256-522` およびルートの機能ゲートの再輸出を検査します。チャック表面を注ぐ `/v2/*` ロードマップ、ヌース エイボンは次のことを確認します。

- `crates/iroha_torii/src/routing.rs` による DTO ハンドラーおよび定義の実装。
- 機能登録 `app_api` または `connect`。
- 既存の組織の統合/統合をテストし、長期にわたる責任を負わせる。

処理手順/トランザクションのリストと、要求されたパラメータの受け入れリスト `asset_id` 機能は、事前濾過の機能、およびページネーション/バックプレッシャーの存在制限を示します。

## 認証と署名の正規化

- エンドポイントの GET/POST は、ヘッダーのオプションを受け入れ、正規要求 (`X-Iroha-Account`、`X-Iroha-Signature`) が `METHOD\n/path\nsorted_query\nsha256(body)` の一部を構成する補助アプリを公開します。 Torii は `QueryRequestWithAuthority` の封筒を作成し、リフレクター `/query` の実行者による検証を行っています。
- クライアントのプリンシポーをサポートするヘルパー SDK:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` デピュイ `canonicalRequest.js`。
  - スイフト: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`。
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`。
- 例:
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

## エンドポイントの発明

### コンパイル権限 (`/v2/accounts/{id}/permissions`) - Couvert
- ハンドラー: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)。
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)。
- ルーター バインディング: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- テスト: `crates/iroha_torii/tests/accounts_endpoints.rs:126` および `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`。
- 所有者: Torii プラットフォーム。
- 注: 本体 JSON Norito の応答は `items`/`total`、SDK のページネーションの補助ヘルパーに準拠します。

### 評価 OPRF d'alias (`POST /v2/aliases/voprf/evaluate`) - Couvert
- ハンドラー: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)。
- DTO: `AliasVoprfEvaluateRequestDto`、`AliasVoprfEvaluateResponseDto`、`AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)。
- ルーター バインディング: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)。
- テスト: インライン デュ ハンドラー テスト (`crates/iroha_torii/src/lib.rs:9945-9986`) とクーベルチュール SDK
  (`javascript/iroha_js/test/toriiClient.test.js:72`)。
- 所有者: Torii プラットフォーム。
- 注: 応答の表面には、バックエンドの識別子と 16 進数の決定が課せられます。 SDK は DTO に対応しています。

### Evenements deproof SSE (`GET /v2/events/sse`) - Couvert
- ハンドラー: `handle_v1_events_sse` フィルタのサポート (`crates/iroha_torii/src/routing.rs:14008-14133`)。
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) およびフィルタ耐性のある配線。
- ルーター バインディング: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- テスト: スイート SSE 固有の証明 (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`、
  `sse_proof_callhash.rs`、`sse_proof_verified_fields.rs`、`sse_proof_rejected_fields.rs`) およびパイプラインの SSE 煙のテスト
  (`integration_tests/tests/events/sse_smoke.rs`)。
- 所有者: Torii プラットフォーム (ランタイム)、統合テスト WG (フィクスチャ)。
- 注: 試合中の有効性を証明するための証拠。 `docs/source/zk_app_api.md` のドキュメントを参照してください。

### Cycle de vie des contrats (`/v2/contracts/*`) - Couvert
- ハンドラー: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)、
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`)、
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`)、
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`)、
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)。
- DTO: `DeployContractDto`、`DeployAndActivateInstanceDto`、`ActivateInstanceDto`、`ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`)。
- ルーター バインディング: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- テスト: スイート ルーター/統合 `contracts_deploy_integration.rs`、`contracts_activate_integration.rs`、
  `contracts_instance_activate_integration.rs`、`contracts_call_integration.rs`、
  `contracts_instances_list_router.rs`。
- 所有者: スマート コントラクト WG avec Torii プラットフォーム。
- 注: トランザクションの署名者およびテレメトリー参加者の再利用および再利用のファイルのエンドポイントの管理 (`handle_transaction_with_metrics`)。### 検証サイクル (`/v2/zk/vk/*`) - Couvert
- ハンドラー: `handle_post_vk_register`、`handle_post_vk_update`、`handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) および `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)。
- DTO: `ZkVkRegisterDto`、`ZkVkUpdateDto`、`ZkVkDeprecateDto`、`VkListQuery`、`ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)。
- ルーター バインディング: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- テスト: `crates/iroha_torii/tests/zk_vk_get_integration.rs`、
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`、
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`。
- 所有者: ZK ワーキング グループ avec サポート Torii プラットフォーム。
- 注: DTO の整列スキーマ Norito は SDK を参照します。ファイルのレート制限は `limits.rs` 経由で適用されます。

### Nexus 接続 (`/v2/connect/*`) - Couvert (機能 `connect`)
- ハンドラー: `handle_connect_session`、`handler_connect_session_delete`、`handle_connect_ws`、
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)。
- DTO: `ConnectSessionRequest`、`ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)、
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)。
- ルーター バインディング: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)。
- テスト: `crates/iroha_torii/tests/connect_gating.rs` (機能ゲーティング、セッション デバイド サイクル、ハンドシェイク WS) など
  ルートゥール機能のクーベルチュール マトリス (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`)。
- 所有者: Nexus WG に接続します。
- 注: `limits::rate_limit_key` 経由でレート制限を解決するための説明。 `connect.*` のテレメトリ測定基準の計算。

### テレメトリー・ド・リレー会議 - Couvvert
- ハンドラー: `handle_v1_kaigi_relays`、`handle_v1_kaigi_relay_detail`、
  `handle_v1_kaigi_relays_health`、`handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)。
- DTO: `KaigiRelaySummaryDto`、`KaigiRelaySummaryListDto`、
  `KaigiRelayDetailDto`、`KaigiRelayDomainMetricsDto`、
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)。
- ルーターバインディング: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)。
- テスト: `crates/iroha_torii/tests/kaigi_endpoints.rs`。
- 注: フラックス SSE は、テレメトリのプロファイル ゲートを適用したグローバルな運河ブロードキャストを再利用します。 `docs/source/torii/kaigi_telemetry_api.md` の応答ソント ドキュメントのスキーマ。

## テストの再開

- ルート上の煙のテスト (`crates/iroha_torii/tests/router_feature_matrix.rs`) は、登録されたチャック ルートと世代 OpenAPI の組み合わせを保証します。
- エンドポイントの要件を確認、コントラクトのサイクル、検証 ZK、フィルタの証明 SSE およびコンポーネント Nexus に接続します。
- SDK (JavaScript、Swift、Python) と Alias VOPRF およびエンドポイント SSE を組み合わせたハーネス。 Aucun travail のサプリメントが必要です。

## ガーダー・セ・ミロワール・ア・ジュール

情報ページと監査ソース (`docs/source/torii/app_api_parity_audit.md`) を参照して、アプリ API Torii を変更し、所有者 SDK と外部講師を変更します。