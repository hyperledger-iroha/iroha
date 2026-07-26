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
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | alias -> manifest CID の対応。 | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | provider に manifest を pin させる命令。 | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | provider の確認応答。 | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | ガバナンスポリシーのスナップショット。 | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: the authoritative manifest lifecycle and finalized
read schemas live in `crates/iroha_data_model/src/sorafs/pin_registry.rs`.
Supporting alias, replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord` and `PinManifestFinalizedRecordV1` are the V1
  manifest-registry surface used by core, Torii, fixtures, and reference
  validators.
- Rust code generation uses Norito derives; SDK parity follows the normal guard
  lanes whenever the native schema changes.
- Architecture, manifest-pipeline, CLI, OpenAPI, status, and roadmap documents
  describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Registration carries the complete canonical manifest, resource-bounds and validates it in consensus, and derives all stored commitments. Core execution also validates aliases, council envelopes, governance permissions, canonical replication payloads, completion, and deadline-bound expiration. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage; alias uniqueness, retention, and replication issue/complete bookkeeping are covered by unit tests. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue/complete, permissions, duplicate rejection, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and runs the parity checks that keep the canonical schema
  surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The manifest-detail route returns exact native `PinManifestFinalizedRecordV1` JSON and accepts only the optional paired expected finalized height/hash precondition; pagination and filters remain on list routes. | Networking TL / Core Infra |
| Finality binding | Listing responses retain their listing attestation. A manifest-detail response carries the native `finalized_cursor` beside the authoritative `PinManifestRecord`; a stale requested cursor fails with HTTP 409. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the JavaScript, Python, Swift, and C# guard lanes mirror the manifest payload and pin-register validation surface. | SDK Teams |

Operations:
- List endpoints use attested snapshots, deterministic pagination, and the cache
  behavior documented in the alias policy where alias proofs are involved.
- `GET /v1/sorafs/pin/{digest_hex}` returns only `finalized_cursor` and the
  native `manifest`. The retired `limit`, attestation, embedded alias/order
  arrays, counts, and truncation fields are absent; callers use
  `/v1/sorafs/aliases` and `/v1/sorafs/replication` for bounded list queries.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

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

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` と `GET /v1/sorafs/replication` はアクティブな
  alias カタログとレプリケーションオーダー backlog を一貫したページネーションと
  ステータスフィルタで公開。

CLI はこれらの呼び出し (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) を包み、低レベル API を触らずに registry 監査を自動化できます。
