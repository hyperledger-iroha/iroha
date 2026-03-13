---
lang: ja
direction: ltr
source: docs/portal/docs/reference/torii-app-api-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce36aff643cae11380048850c3e7ad09ae00c0532db8250a4b99e55377273022
source_last_modified: "2025-12-07T12:16:09.054032+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: torii-app-api-parity
title: Torii アプリ API パリティ監査
description: SDK とプラットフォームチームが公開カバレッジを確認できるように TORII-APP-1 レビューをミラーしたもの。
---

ステータス: 完了 2026-03-21  
オーナー: Torii Platform, SDK Program Lead  
ロードマップ参照: TORII-APP-1 — `app_api` パリティ監査

このページは内部監査 `TORII-APP-1`（`docs/source/torii/app_api_parity_audit.md`）を反映し、モノレポ外の読者が `/v2/*` のどの面が配線済みで、テストされ、文書化されているかを把握できるようにしています。監査は `Torii::add_app_api_routes`、`add_contracts_and_vk_routes`、`add_connect_routes` を通じて再公開されるルートを追跡します。

## 範囲と方法

監査は `crates/iroha_torii/src/lib.rs:256-522` の公開再エクスポートと feature gating されたルートビルダーを確認しました。ロードマップにある各 `/v2/*` の面について、次を検証しています:

- `crates/iroha_torii/src/routing.rs` にあるハンドラ実装と DTO 定義。
- `app_api` または `connect` の feature グループ配下へのルータ登録。
- 既存の統合/ユニットテストと長期的なカバレッジの担当チーム。

口座資産/取引の一覧とアセットホルダーの一覧は、既存のページネーション/バックプレッシャー制限に加えて、事前フィルタリング用のオプション `asset_id` クエリパラメータを受け付けます。

## 認証とカノニカル署名

- アプリ向け GET/POST エンドポイントは、`METHOD\n/path\nsorted_query\nsha256(body)` から構築されるカノニカルリクエストヘッダ（`X-Iroha-Account`, `X-Iroha-Signature`）を任意で受け付けます。Torii は executor 検証前に `QueryRequestWithAuthority` に包み、`/query` と同等にします。
- SDK ヘルパーは主要クライアントすべてに提供されています:
  - JS/TS: `canonicalRequest.js` の `buildCanonicalRequestHeaders({ accountId, method, path, query, body, privateKey })`。
  - Swift: `CanonicalRequest.signingHeaders(accountId:method:path:query:body:signer:)`。
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

## エンドポイント一覧

### アカウント権限 (`/v2/accounts/{id}/permissions`) — 対応済み
- ハンドラ: `handle_v1_account_permissions` (`crates/iroha_torii/src/routing.rs:16873`).
- DTO: `filter::Pagination` + `AccountPermissionListItem` (`crates/iroha_torii/src/routing.rs:16867`).
- ルータ登録: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- テスト: `crates/iroha_torii/tests/accounts_endpoints.rs:126` と `crates/iroha_torii/tests/account_query_subrouter_smoke.rs:146`。
- オーナー: Torii Platform。
- メモ: レスポンスは `items`/`total` を含む Norito JSON body で、SDK のページングヘルパーと一致します。

### エイリアス OPRF 評価 (`POST /v2/aliases/voprf/evaluate`) — 対応済み
- ハンドラ: `handler_alias_voprf_evaluate` (`crates/iroha_torii/src/lib.rs:5645-5660`).
- DTO: `AliasVoprfEvaluateRequestDto`, `AliasVoprfEvaluateResponseDto`, `AliasVoprfBackendDto`
  (`crates/iroha_torii/src/routing.rs:809-865`).
- ルータ登録: `Torii::add_alias_routes` (`crates/iroha_torii/src/lib.rs:6357-6380`).
- テスト: ハンドラの inline テスト (`crates/iroha_torii/src/lib.rs:9945-9986`) と SDK カバレッジ
  (`javascript/iroha_js/test/toriiClient.test.js:72`).
- オーナー: Torii Platform。
- メモ: レスポンス面は決定的な hex と backend 識別子を強制し、SDK が DTO を消費します。

### Proof イベント SSE (`GET /v2/events/sse`) — 対応済み
- ハンドラ: フィルタ対応の `handle_v1_events_sse` (`crates/iroha_torii/src/routing.rs:14008-14133`).
- DTO: `EventsSseParams` (`crates/iroha_torii/src/routing.rs:14000-14006`) と proof フィルタ配線。
- ルータ登録: `Torii::add_app_api_routes` (`crates/iroha_torii/src/lib.rs:6678-6797`).
- テスト: proof 専用 SSE スイート (`crates/iroha_torii/tests/sse_proof_envelope_hash.rs`,
  `sse_proof_callhash.rs`, `sse_proof_verified_fields.rs`, `sse_proof_rejected_fields.rs`) とパイプライン SSE の smoke テスト
  (`integration_tests/tests/events/sse_smoke.rs`).
- オーナー: Torii Platform (runtime), Integration Tests WG (fixtures).
- メモ: proof フィルタの経路は end-to-end で検証済み。ドキュメントは `docs/source/zk_app_api.md` にあります。

### コントラクトライフサイクル (`/v2/contracts/*`) — 対応済み
- ハンドラ: `handle_post_contract_deploy` (`crates/iroha_torii/src/routing.rs:5511-5566`),
  `handle_post_contract_instance` (`crates/iroha_torii/src/routing.rs:3464-3512`),
  `handle_post_contract_instance_activate` (`crates/iroha_torii/src/routing.rs:3408-3459`),
  `handle_post_contract_call` (`crates/iroha_torii/src/routing.rs:3534-3607`),
  `handle_get_contract_code_bytes` (`crates/iroha_torii/src/routing.rs:3237-3304`).
- DTO: `DeployContractDto`, `DeployAndActivateInstanceDto`, `ActivateInstanceDto`, `ContractCallDto`
  (`crates/iroha_torii/src/routing.rs:3124-3463`).
- ルータ登録: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- テスト: ルータ/統合スイート `contracts_deploy_integration.rs`, `contracts_activate_integration.rs`,
  `contracts_instance_activate_integration.rs`, `contracts_call_integration.rs`,
  `contracts_instances_list_router.rs`。
- オーナー: Smart Contract WG と Torii Platform。
- メモ: エンドポイントは署名済みトランザクションをキューに入れ、共有テレメトリ指標 (`handle_transaction_with_metrics`) を再利用します。

### 検証キーライフサイクル (`/v2/zk/vk/*`) — 対応済み
- ハンドラ: `handle_post_vk_register`, `handle_post_vk_update`, `handle_post_vk_deprecate`
  (`crates/iroha_torii/src/routing.rs:4282-4382`) と `handle_get_vk` (`crates/iroha_torii/src/routing.rs:4384-4418`).
- DTO: `ZkVkRegisterDto`, `ZkVkUpdateDto`, `ZkVkDeprecateDto`, `VkListQuery`, `ProofFindByIdQueryDto`
  (`crates/iroha_torii/src/routing.rs:3619-4279`).
- ルータ登録: `Torii::add_contracts_and_vk_routes` (`crates/iroha_torii/src/lib.rs:6456-6483`).
- テスト: `crates/iroha_torii/tests/zk_vk_get_integration.rs`,
  `crates/iroha_torii/tests/zk_verify_handler_integration.rs`,
  `crates/iroha_torii/tests/zk_vote_tally_handler.rs`。
- オーナー: ZK Working Group と Torii Platform。
- メモ: DTO は SDK が参照する Norito スキーマと整合し、レート制限は `limits.rs` で適用されます。

### Nexus Connect (`/v2/connect/*`) — 対応済み (feature `connect`)
- ハンドラ: `handle_connect_session`, `handler_connect_session_delete`, `handle_connect_ws`,
  `handle_connect_status` (`crates/iroha_torii/src/routing.rs:1562-2136`).
- DTO: `ConnectSessionRequest`, `ConnectSessionResponse` (`crates/iroha_torii/src/routing.rs:1534-1559`),
  `ConnectSessionStatusDto` (`crates/iroha_torii/src/routing.rs:2004-2035`).
- ルータ登録: `Torii::add_connect_routes` (`crates/iroha_torii/src/lib.rs:6645-6661`).
- テスト: `crates/iroha_torii/tests/connect_gating.rs` (feature gating、セッションライフサイクル、WS ハンドシェイク) と
  ルータ feature 行列のカバレッジ (`crates/iroha_torii/tests/router_feature_matrix.rs:804-876`).
- オーナー: Nexus Connect WG。
- メモ: rate limit キーは `limits::rate_limit_key` で追跡され、テレメトリカウンタが `connect.*` 指標を供給します。

### Kaigi リレー・テレメトリ — 対応済み
- ハンドラ: `handle_v1_kaigi_relays`, `handle_v1_kaigi_relay_detail`,
  `handle_v1_kaigi_relays_health`, `handle_v1_kaigi_relays_sse`
  (`crates/iroha_torii/src/routing.rs:14510-14787`).
- DTO: `KaigiRelaySummaryDto`, `KaigiRelaySummaryListDto`,
  `KaigiRelayDetailDto`, `KaigiRelayDomainMetricsDto`,
  `KaigiRelayHealthSnapshotDto` (`crates/iroha_torii/src/routing.rs:932-1046`).
- ルータ登録: `Torii::add_app_api_routes`
  (`crates/iroha_torii/src/lib.rs:6805-6840`).
- テスト: `crates/iroha_torii/tests/kaigi_endpoints.rs`。
- メモ: SSE ストリームはグローバルなブロードキャストチャネルを再利用しつつ、テレメトリプロファイルのゲートを適用します。レスポンススキーマは `docs/source/torii/kaigi_telemetry_api.md` にあります。

## テストカバレッジ概要

- ルータの smoke テスト (`crates/iroha_torii/tests/router_feature_matrix.rs`) は、feature の組み合わせが全ルートを登録し、OpenAPI 生成が同期されることを保証します。
- エンドポイント別スイートはアカウントクエリ、コントラクトライフサイクル、ZK 検証キー、proof SSE フィルタ、Nexus Connect の挙動をカバーします。
- SDK パリティのハーネス (JavaScript, Swift, Python) は既に Alias VOPRF と SSE を消費しており、追加作業は不要です。

## このミラーを最新に保つ

Torii app API の挙動が変わるたびに、このページと元監査 (`docs/source/torii/app_api_parity_audit.md`) を更新し、SDK オーナーと外部読者の整合性を保ってください。
