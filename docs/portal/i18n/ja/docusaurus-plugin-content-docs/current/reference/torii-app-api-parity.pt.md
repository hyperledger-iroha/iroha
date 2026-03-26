---
lang: ja
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: torii-app-api-parity
タイトル: API の監査 Torii
説明: SDK およびプラットフォームの装備として TORII-APP-1 パラメータを改訂し、cobertura パブリックを確認します。
---

ステータス: 結論 2026-03-21  
回答: Torii プラットフォーム、SDK プログラム リード  
ロードマップの参照: TORII-APP-1 - Auditoria de paridade `app_api`

内部 `TORII-APP-1` (`docs/source/torii/app_api_parity_audit.md`) は、モノリポジトリを作成するための重要な情報を表示し、`/v1/*` 接続、検証、文書化を行います。 `Torii::add_app_api_routes`、`add_contracts_and_vk_routes`、`add_connect_routes` を介して回転再輸出としての聴衆。

## エスコポとメソッド

再輸出としての監査は、`crates/iroha_torii/src/lib.rs:256-522` 機能ゲート機能の構造を公開しています。 `/v1/*` はロードマップの検証を行います:

- ハンドラーと定義された DTO em `crates/iroha_torii/src/routing.rs` を実装します。
- ルーターの機能グループ `app_api` または `connect` を登録します。
- Testes de integracao/unitariosexistentes は、適切な応答を提供するための準備を整えています。

活動/トランザクションのリストとして、`asset_id` の事前濾過に関するオプションを参照し、ページ/バックプレッシャーの存在を制限します。

## Autenticacao e assinatura canonica

- エンドポイントの GET/POST ボルタドとアプリの認証ヘッダー オプション (`X-Iroha-Account`、`X-Iroha-Signature`) の `METHOD\n/path\nsorted_query\nsha256(body)` の構成。 o Torii OS は、`QueryRequestWithAuthority` を実行し、`/query` を実行します。
- SDK が存在する todos OS クライアントの主なヘルパー:
  - JS/TS: `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })` から `canonicalRequest.js`。
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

### 管理権限 (`/v1/accounts/{id}/permissions`) - コベルト
- ハンドラー: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`)。
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`)。
- ルーター バインディング: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- テスト: `crates/iroha_torii/tests/accounts_endpoints.rs:126` および `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`。
- 所有者: Torii プラットフォーム。
- 注: ボディ JSON Norito com `items`/`total`、SDK のページの aos ヘルパーに関するレスポスタ。

### Avaliacao OPRF デ エイリアス (`POST /v1/aliases/voprf/evaluate`) - コベルト
- ハンドラー: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`)。
- DTO: `AliasVoprfEvaluateRequestDto`、`AliasVoprfEvaluateResponseDto`、`AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`)。
- ルーター バインディング: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`)。
- テスト: testes インライン do ハンドラー (`crates/iroha_torii/src/lib.rs:9945-9986`) SDK の内容
  (`javascript/iroha_js/test/toriiClient.test.js:72`)。
- 所有者: Torii プラットフォーム。
- 注意: バックエンドの 16 進決定性識別子を再定義するための管理機能。 OS SDK コンソメムまたは DTO。

### イベント証明 SSE (`GET /v1/events/sse`) - Coberto
- ハンドラー: `handle_v1_events_sse` は filtros (`crates/iroha_torii/src/routing.rs:14008-14133`) をサポートします。
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) は配線とフィルターを保護します。
- ルーター バインディング: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`)。
- テスト: SSE 固有の証明スイート (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`、
  `sse_proof_callhash.rs`、`sse_proof_verified_fields.rs`、`sse_proof_rejected_fields.rs`) テスト煙 SSE 実行パイプライン
  (`integration_tests/tests/events/sse_smoke.rs`)。
- 所有者: Torii プラットフォーム (ランタイム)、統合テスト WG (フィクスチャ)。
- 注: エンドツーエンドの検証フォーマットのフィルタリングを実行します。 `docs/source/zk_app_api.md` のドキュメント。

### シクロ デ ヴィーダ デ コントラトス (`/v1/contracts/*`) - コベルト
- ハンドラー: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`)、
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`)、
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`)、
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`)、
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`)。
- DTO: `DeployContractDto`、`DeployAndActivateInstanceDto`、`ActivateInstanceDto`、`ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`)。
- ルーター バインディング: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- テスト: スイートルーター/integracao `contracts_deploy_integration.rs`、`contracts_activate_integration.rs`、
  `contracts_instance_activate_integration.rs`、`contracts_call_integration.rs`、
  `contracts_instances_list_router.rs`。
- 所有者: スマート コントラクト WG com Torii プラットフォーム。
- 注意: OS エンドポイントは、テレメトリコンパートメントのトランザクション管理および再利用メトリクスを管理します (`handle_transaction_with_metrics`)。### シクロ デ ヴィーダ デ チャベス デ ベリフィカオ (`/v1/zk/vk/*`) - コベルト
- ハンドラー: `handle_post_vk_register`、`handle_post_vk_update`、`handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) e `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`)。
- DTO: `ZkVkRegisterDto`、`ZkVkUpdateDto`、`ZkVkDeprecateDto`、`VkListQuery`、`ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`)。
- ルーター バインディング: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`)。
- テスト: `crates/iroha_torii/tests/zk_vk_get_integration.rs`、
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`、
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`。
- 所有者: ZK Working Group com は Torii プラットフォームをサポートします。
- 注: OS DTO は、alinham aos スキーマ Norito を参照し、ペロス SDK を参照します。 `limits.rs` を介してレート制限を適用します。

### Nexus 接続 (`/v1/connect/*`) - コベルト (機能 `connect`)
- ハンドラー: `handle_connect_session`、`handler_connect_session_delete`、`handle_connect_ws`、
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`)。
- DTO: `ConnectSessionRequest`、`ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`)、
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`)。
- ルーター バインディング: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`)。
- テスト: `crates/iroha_torii/tests/connect_gating.rs` (機能ゲート、シクロ デ ヴィーダ デ セッサオ、ハンドシェイク WS)
  ルーター機能を備えた製品 (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`)。
- 所有者: Nexus WG に接続します。
- 注: `limits::rate_limit_key` を介してレート制限を変更し、rastreadas を監視します。メトリカス `connect.*` としてのテレメトリア アリメンタムのコンタドール。

### テレメトリア・デ・リレー会議 - コベルト
- ハンドラー: `handle_v1_kaigi_relays`、`handle_v1_kaigi_relay_detail`、
  `handle_v1_kaigi_relays_health`、`handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`)。
- DTO: `KaigiRelaySummaryDto`、`KaigiRelaySummaryListDto`、
  `KaigiRelayDetailDto`、`KaigiRelayDomainMetricsDto`、
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`)。
- ルーターバインディング: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`)。
- テスト: `crates/iroha_torii/tests/kaigi_endpoints.rs`。
- 注: O ストリーム SSE 再利用、グローバル ブロードキャスト エンカント アプリケーション、テレメトリの実行の実行。 OS スキーマ デ レスポスタ ドキュメントの `docs/source/torii/kaigi_telemetry_api.md`。

## 精巣の回復

- テスト煙ルーター (`crates/iroha_torii/tests/router_feature_matrix.rs`) は、OpenAPI の問題を解決するために、組み合わせ機能を保証します。
- Nexus Connect のエンドポイント固有のクエリ、コントラトスのビデオ、ZK 検証、証明 SSE のフィルタなどのスイート。
- SDK (JavaScript、Swift、Python)、コンソーム、エイリアス VOPRF、エンドポイント SSE のハーネス。ナオ・ハ・トラバリョ・アディシオナル。

## マンター エステ エスペリョ アトゥアリザド

Torii のアプリケーション API を含むページ (`docs/source/torii/app_api_parity_audit.md`) を、SDK の外部ファイルの所有者が確認できるようにします。