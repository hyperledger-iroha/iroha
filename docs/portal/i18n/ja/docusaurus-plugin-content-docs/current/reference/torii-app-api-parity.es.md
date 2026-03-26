---
lang: ja
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-parity
title: Torii アプリの API を使用した監査
説明: TORII-APP-1 の改訂版は、SDK とプラットフォームの公開を確認するために必要です。
---

エスタード: コンプリート 2026-03-21  
責任者: Torii プラットフォーム、SDK プログラム リード  
ロードマップの参照: TORII-APP-1 - Auditoria de paridad de `app_api`

`TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) のページは、モノレポのページで、最高の情報を提供します `/v1/*` ケーブル、プロバダス、および文書を表示します。 `Torii::add_app_api_routes`、`add_contracts_and_vk_routes` および `add_connect_routes` のオーディオを再エクスポートします。

## アルカンスと方法

`crates/iroha_torii/src/lib.rs:256-522` では、機能ゲーティングのコンストラクターが公開されており、監査検査が行われています。ロードマップの検証情報 `/v1/*` の詳細:

- `crates/iroha_torii/src/routing.rs` の DTO 定義のハンドラーを実装します。
- ルーター機能の登録 `app_api` または `connect`。
- Pruebas de integracion/unitariasexistentes y el equipo 責任者 de la cobertura a largo plazo。

アクティベーション/バックプレッシャーのリストとトランザクションのリストは、`asset_id` プレフィルトラドのオプション、ページ/バックプレッシャーの存在を制限します。

## 認証と正規認証

- Los endpoints GET/POST orientados a apps aceptan headers opcionales de solicitud canonica (`X-Iroha-Account`, `X-Iroha-Signature`) construidos desde `METHOD\n/path\nsorted_query\nsha256(body)`; Torii は `QueryRequestWithAuthority` を参照し、実行者は `/query` を検証します。
- SDK のヘルパーとクライアント プリンシパルの管理:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` デスデ `canonicalRequest.js`。
  - スイフト: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`。
  - Android (Kotlin/Java): `CanonicalRequestSigner.signingHeaders(accountId, method, path, query, body, signer)`。
- 例:
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

## エンドポイントの在庫

### Permisos de cuenta (`/v1/accounts/{id}/permissions`) - クビエルト
- ハンドラー: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)。
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)。
- ルーター バインディング: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- テスト: `crates/iroha_torii/tests/accounts_endpoints.rs:126` および `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`。
- 所有者: Torii プラットフォーム。
- 注意: 本文 JSON Norito と `items`/`total` は、SDK のページヘルパーと一致します。

### 別名 OPRF の評価 (`POST /v1/aliases/voprf/evaluate`) - Cubierto
- ハンドラー: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)。
- DTO: `AliasVoprfEvaluateRequestDto`、`AliasVoprfEvaluateResponseDto`、`AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)。
- ルーター バインディング: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)。
- テスト: SDK のプルエバス インライン デル ハンドラー (`crates/iroha_torii/src/lib.rs:9945-9986`)
  (`javascript/iroha_js/test/toriiClient.test.js:72`)。
- 所有者: Torii プラットフォーム。
- 注意事項: バックエンドの 16 進決定性識別子を保護するための管理権限。 SDK コンシューマー DTO を失います。

### イベント証明 SSE (`GET /v1/events/sse`) - Cubierto
- ハンドラー: `handle_v1_events_sse` フィルター デ コン ポート (`crates/iroha_torii/src/routing.rs:14008-14133`)。
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) 配線フィルターの保護。
- ルーター バインディング: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- テスト: SSE 固有の証明スイート (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`、
  `sse_proof_callhash.rs`、`sse_proof_verified_fields.rs`、`sse_proof_rejected_fields.rs`) および SSE デル パイプラインのプルエバ スモーク
  (`integration_tests/tests/events/sse_smoke.rs`)。
- 所有者: Torii プラットフォーム (ランタイム)、統合テスト WG (フィクスチャ)。
- 注: エンドツーエンドで有効な証明のフィルタの実行。 `docs/source/zk_app_api.md` で生きているドキュメント。

### シクロ デ ヴィーダ デ コントラトス (`/v1/contracts/*`) - クビエルト
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
- 所有者: Torii プラットフォームのスマート コントラクト WG。
- 注意: エンドポイントのエンコラン トランザクション ファームダと再利用メトリクス コンパルティダ デ テレメトリ (`handle_transaction_with_metrics`)。### シクロ デ ヴィーダ デ クラーベス デ ベリフィカシオン (`/v1/zk/vk/*`) - Cubierto
- ハンドラー: `handle_post_vk_register`、`handle_post_vk_update`、`handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) y `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)。
- DTO: `ZkVkRegisterDto`、`ZkVkUpdateDto`、`ZkVkDeprecateDto`、`VkListQuery`、`ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)。
- ルーター バインディング: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- テスト: `crates/iroha_torii/tests/zk_vk_get_integration.rs`、
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`、
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`。
- 所有者: Torii プラットフォームをサポートする ZK ワーキング グループ。
- 注意: Los DTOs se alinean con los esquemas Norito Referenciados por los SDK; `limits.rs` を介してレート制限を実行します。

### Nexus 接続 (`/v1/connect/*`) - Cubierto (機能 `connect`)
- ハンドラー: `handle_connect_session`、`handler_connect_session_delete`、`handle_connect_ws`、
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)。
- DTO: `ConnectSessionRequest`、`ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)、
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)。
- ルーター バインディング: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)。
- テスト: `crates/iroha_torii/tests/connect_gating.rs` (機能ゲート、シクロ デ ヴィーダ セッション、ハンドシェイク WS)
  ルーター機能の詳細 (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`)。
- 所有者: Nexus WG に接続します。
- 注: `limits::rate_limit_key` 経由のレート制限のラス クラベス セキュリティ。ロス コンタドール デ テレメトリア アリメンタン ラス メトリカス `connect.*`。

### テレメトリア・デ・リレー会議 - クビエルト
- ハンドラー: `handle_v1_kaigi_relays`、`handle_v1_kaigi_relay_detail`、
  `handle_v1_kaigi_relays_health`、`handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)。
- DTO: `KaigiRelaySummaryDto`、`KaigiRelaySummaryListDto`、
  `KaigiRelayDetailDto`、`KaigiRelayDomainMetricsDto`、
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)。
- ルーターバインディング: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)。
- テスト: `crates/iroha_torii/tests/kaigi_endpoints.rs`。
- 注意: ストリーム SSE 再利用、グローバル ブロードキャスト ミエントラ アプリケーション、ゲートおよびテレメトリのパーフィル。 los esquemas de respuesta se documentan en `docs/source/torii/kaigi_telemetry_api.md`。

## プルエバスの履歴書

- ルータ (`crates/iroha_torii/tests/router_feature_matrix.rs`) の煙のルータ (`crates/iroha_torii/tests/router_feature_matrix.rs`) は、OpenAPI と同期の機能を登録するための組み合わせを保証します。
- エンドポイント固有のクエリ、コントラトスの検証、ZK のクラベス、証明 SSE および Nexus 接続のフィルター。
- SDK (JavaScript、Swift、Python) を利用し、Alias VOPRF とエンドポイント SSE を使用します。いいえ、トラバホ・アディシオナルは必要ありません。

## マンテナー エステ エスペホ アクチュアリザド

実際のページとオーディオ フエンテ (`docs/source/torii/app_api_parity_audit.md`) は、Torii の SDK 所有者と外部のスピーカーからアプリ API を提供します。