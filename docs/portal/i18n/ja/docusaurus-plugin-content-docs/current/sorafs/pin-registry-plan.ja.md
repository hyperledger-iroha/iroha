---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 439d418cf6dbd9d4fe47b9df50d7062993b17987aecea6547ea6c51383192039
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: pin-registry-plan
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
このページは `docs/source/sorafs/pin_registry_plan.md` を反映しています。レガシー文書が有効な間は両方を同期してください。
:::

# SoraFS Pin Registry 実装計画 (SF-4)

SF-4 は Pin Registry コントラクトと、その周辺サービスを提供します。これらは
manifest のコミットメントを保持し、pin ポリシーを強制し、Torii、ゲートウェイ、
オーケストレータ向けの API を公開します。本ドキュメントは検証計画を具体的な
実装タスクに拡張し、オンチェーンロジック、ホスト側サービス、フィクスチャ、
運用要件を網羅します。

## スコープ

1. **registry の状態機械**: manifest、alias、後継チェーン、保持エポック、
   ガバナンスメタデータの Norito 定義レコード。
2. **コントラクト実装**: pin ライフサイクルの決定論的 CRUD (`ReplicationOrder`,
   `Precommit`, `Completion`, eviction)。
3. **サービスファサード**: Torii と SDK が利用する registry 由来の gRPC/REST
   エンドポイント (ページネーションとアテステーションを含む)。
4. **tooling とフィクスチャ**: CLI ヘルパー、テストベクトル、ドキュメントで
   manifest、alias、ガバナンスエンベロープを同期。
5. **テレメトリと ops**: registry 健全性のメトリクス、アラート、ランブック。

## データモデル

### コアレコード (Norito)

| Struct | 説明 | フィールド |
|--------|------|------------|
| `PinRecordV1` | 正式な manifest エントリ。 | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | alias -> manifest CID の対応。 | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | provider に manifest を pin させる命令。 | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | provider の確認応答。 | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | ガバナンスポリシーのスナップショット。 | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

実装参照: `crates/sorafs_manifest/src/pin_registry.rs` に Rust Norito スキーマと
検証ヘルパーがあり、これらのレコードを支えます。検証は manifest tooling
(chunker registry lookup、pin policy gating) と同等で、コントラクト、Torii
ファサード、CLI が同一の不変条件を共有します。

タスク:
- `crates/sorafs_manifest/src/pin_registry.rs` の Norito スキーマを確定。
- Norito マクロでコード生成 (Rust + 他 SDK)。
- スキーマ確定後に `sorafs_architecture_rfc.md` を更新。

## コントラクト実装

| タスク | 担当 | メモ |
|--------|------|------|
| registry ストレージ (sled/sqlite/off-chain) またはスマートコントラクトモジュールを実装。 | Core Infra / Smart Contract Team | 決定論的ハッシュを提供し、浮動小数点は避ける。 |
| エントリポイント: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Core Infra | 検証計画の `ManifestValidator` を活用。alias バインドは `RegisterPinManifest` (Torii DTO) を経由し、専用の `bind_alias` は後続アップデートに残る。 |
| 状態遷移: 継承関係 (manifest A -> B)、保持エポック、alias の一意性を強制。 | Governance Council / Core Infra | alias の一意性、保持制限、前任の承認/退役チェックは `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` に存在。多段継承検出と複製の台帳処理は未解決。 |
| ガバナンスパラメータ: `ManifestPolicyV1` を config/ガバナンス状態から読み込み、ガバナンスイベントで更新可能に。 | Governance Council | ポリシー更新用 CLI を提供。 |
| イベント発行: テレメトリ向け Norito イベント (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`) を発行。 | Observability | イベントスキーマ + logging を定義。 |

テスト:
- 各エントリポイントのユニットテスト (正常 + 拒否)。
- 継承チェーンのプロパティテスト (循環なし、エポック単調増加)。
- ランダム manifest (制約付き) によるバリデーション fuzz。

## サービスファサード (Torii/SDK 統合)

| コンポーネント | タスク | 担当 |
|----------------|------|------|
| Torii サービス | `/v2/sorafs/pin` (submit)、`/v2/sorafs/pin/{cid}` (lookup)、`/v2/sorafs/aliases` (list/bind)、`/v2/sorafs/replication` (orders/receipts) を公開。ページネーション + フィルタを提供。 | Networking TL / Core Infra |
| アテステーション | レスポンスに registry の高さ/ハッシュを含め、SDK が消費する Norito アテステーション構造体を追加。 | Core Infra |
| CLI | `sorafs_manifest_stub` 拡張または新 CLI `sorafs_pin` に `pin submit`, `alias bind`, `order issue`, `registry export` を追加。 | Tooling WG |
| SDK | Norito スキーマからクライアントバインディング (Rust/Go/TS) を生成し、統合テストを追加。 | SDK Teams |

運用:
- GET エンドポイントにキャッシュ/ETag 層を追加。
- Torii ポリシーに準拠した rate limiting / auth を提供。

## フィクスチャと CI

- フィクスチャディレクトリ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` に署名済み manifest/alias/order スナップショットを保存。`cargo run -p iroha_core --example gen_pin_snapshot` で再生成。
- CI ステップ: `ci/check_sorafs_fixtures.sh` がスナップショットを再生成し、差分があれば失敗して CI fixture を整合。
- 統合テスト (`crates/iroha_core/tests/pin_registry.rs`) は正常系に加え、alias 重複拒否、alias 承認/保持ガード、chunker handle の不一致、レプリカ数検証、継承ガード失敗 (未知/事前承認/退役/自己参照) を網羅。`register_manifest_rejects_*` を参照。
- ユニットテストは `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` の alias 検証、保持ガード、後継チェックをカバー。多段継承検出は状態機械導入後。
- 観測パイプラインで使うイベント用の golden JSON。

## テレメトリとオブザーバビリティ

メトリクス (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- 既存の provider テレメトリ (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) も end-to-end ダッシュボード対象。

ログ:
- ガバナンス監査向けの構造化 Norito イベントストリーム (署名付き?).

アラート:
- SLA を超過する保留中レプリケーションオーダー。
- 期限が閾値未満の alias。
- 保持違反 (manifest が期限前に更新されない)。

ダッシュボード:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` は manifest ライフサイクル合計、alias カバレッジ、backlog 飽和、SLA 比率、latency vs slack の重ね合わせ、失敗オーダー率を on-call 用に可視化。

## ランブックとドキュメント

- registry のステータス更新を含めるため `docs/source/sorafs/migration_ledger.md` を更新。
- 運用ガイド: `docs/source/sorafs/runbooks/pin_registry_ops.md` (公開済み) にメトリクス、アラート、デプロイ、バックアップ、復旧フローを記載。
- ガバナンスガイド: ポリシーパラメータ、承認ワークフロー、紛争対応を記述。
- 各エンドポイントの API リファレンス (Docusaurus docs)。

## 依存関係とシーケンス

1. 検証計画タスク完了 (ManifestValidator 統合)。
2. Norito スキーマとポリシーデフォルトの確定。
3. コントラクト + サービス実装、テレメトリ接続。
4. フィクスチャ再生成、統合スイート実行。
5. docs/runbooks 更新、ロードマップ項目を完了に。

SF-4 のチェックリスト項目は進捗時に本計画を参照すること。
REST ファサードはアテスト付きリストエンドポイントを提供済み:

- `GET /v2/sorafs/pin` と `GET /v2/sorafs/pin/{digest}` は manifest を返し、
  alias bindings、レプリケーションオーダー、最新ブロックハッシュ由来の
  アテステーションオブジェクトを含む。
- `GET /v2/sorafs/aliases` と `GET /v2/sorafs/replication` はアクティブな
  alias カタログとレプリケーションオーダー backlog を一貫したページネーションと
  ステータスフィルタで公開。

CLI はこれらの呼び出し (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) を包み、低レベル API を触らずに registry 監査を自動化できます。
