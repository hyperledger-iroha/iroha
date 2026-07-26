---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
title: SoraFS Pin レジストリ
Sidebar_label: Pin レジストリ
説明: SF-4 の定義 レジストリ ステート マシン Torii ファサード ツール 可観測性 ہے۔
---

:::note メモ
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہیں دونوں نقول ہم آہنگ رکھیں۔
:::

# SoraFS ピン レジストリ نفاذی منصوبہ (SF-4)

SF-4 ピン レジストリの確認と確認の確認と約束の明示の確認
ポリシーのピン留め Torii ゲートウェイ オーケストレーター API の API の固定
検証計画 実装タスク オンチェーン ロジック
ホスト側のサービスと備品の詳細

## और देखें

1. **レジストリ ステート マシン**: Norito で定義されたレコード、マニフェスト、エイリアス、後継チェーン
   保持エポックとガバナンス メタデータ。
2. *******: ピンのライフサイクル*** 決定論的な CRUD 操作 (`ReplicationOrder`、
   `Precommit`、`Completion`、エビクション)。
3. **ファサード**: gRPC/REST エンドポイントとレジストリ、Torii、SDK、および SDK
   ページネーション 認証 شامل ہے۔
4. **ツールとフィクスチャ**: CLI ヘルパー、テスト ベクトル、ドキュメント、マニフェスト、エイリアス
   ガバナンス封筒 ہم آہنگ رہیں۔
5. **テレメトリ操作**: レジストリ、メトリクス、アラート、ランブック。

## और देखें

### بنیادی ریکارڈز (Norito)

|構造体 | | और देखしているの
|----------|----------|----------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` |エイリアス -> マニフェスト CID マッピング。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |プロバイダーはマニフェスト ピンを作成します。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |プロバイダーの承認。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |ガバナンス ポリシーのスナップショット。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

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

## 試合日程 CI

- フィクスチャ ディレクトリ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` 署名済みマニフェスト/エイリアス/注文スナップショット محفوظ ہوتے ہیں جو `cargo run -p iroha_core --example gen_pin_snapshot` سے 再生成 ہوتے ہیں۔
- CI ステップ: `ci/check_sorafs_fixtures.sh` スナップショットの再生成、差分、失敗、CI フィクスチャの位置合わせ
- 統合テスト (`crates/iroha_core/tests/pin_registry.rs`) ハッピー パス、重複エイリアスの拒否、エイリアス承認/リテンション ガード、チャンカー ハンドルの不一致、レプリカ数の検証、継承ガードの失敗 (不明/事前承認/廃止/自己ポインタ) `register_manifest_rejects_*` ケース دیکھیں۔
- 単体テスト、`crates/iroha_core/src/smartcontracts/isi/sorafs.rs` 別名検証、リテンション ガード、後続チェックマルチホップ連続検出 ステート マシン ステート マシン
- 可観測性パイプラインのゴールデン JSON イベント

## テレメトリと可観測性

メトリック (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- プロバイダー テレメトリ (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) エンドツーエンド ダッシュボード スコープ スコープ

ログ:
- ガバナンス監査構造化 Norito イベント ストリーム (署名済み?)。

アラート:
- SLA は保留中の複製命令です。
- エイリアスの有効期限のしきい値。
- 保持違反 (マニフェスト更新 پہلے نہ ہو)。

ダッシュボード:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` マニフェスト ライフサイクルの合計、エイリアス カバレッジ、バックログの飽和度、SLA 比率、レイテンシとスラック オーバーレイ、欠品率、オンコール レビュー、データ

## ランブックとドキュメント

- `docs/source/sorafs/migration_ledger.md` レジストリ ステータスの更新情報
- オペレーター ガイド: `docs/source/sorafs/runbooks/pin_registry_ops.md` (分析) メトリクス、アラート、導入、バックアップ、回復フロー
- ガバナンス ガイド: 政策パラメーター、承認ワークフロー、紛争処理、その他
- API リファレンス ページ (Docusaurus ドキュメント)。

## 依存関係とシーケンス

1. 検証計画タスク (ManifestValidator 統合)。
2. Norito スキーマ + ポリシーのデフォルト
3. 契約 + サービス テレメトリ ワイヤー
4. フィクスチャは統合スイートを再生成します
5. ドキュメント/Runbook ロードマップ項目の説明

SF-4 チェックリスト آئٹم میں پیش رفت پر اس منصوبے کا حوالہ ہونا چاہیے۔
REST ファサードの認証済みエンドポイントのリスト:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` `GET /v1/sorafs/replication` エイリアス カタログ
  レプリケーション順序バックログ 一貫性のあるページネーション ステータス フィルター

CLI は、ラップを呼び出します (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`) 演算子 API レジストリ監査 レジストリ監査