---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
タイトル: SoraFS のピン レジストリの実装計画
サイドバー_ラベル: プラン デュ パン レジストリ
説明: SF-4 の実装計画、レジストリのマシン設計、ファサード Torii、ツールおよび観察可能性の確認。
---

:::note ソースカノニク
Cette ページは `docs/source/sorafs/pin_registry_plan.md` を参照します。 Gardez les deux は、アクティブなドキュメントの保存と同期をコピーします。
:::

# SoraFS (SF-4) のピン レジストリの実装計画

SF-4 の制御ピン レジストリと在庫管理サービス
マニフェストの約束、固定および公開の政治的なアップリケント
API à Torii、補助ゲートウェイおよび補助オーケストレーター。計画を立てて文書を作成する
具体的な実装に関する検証、論理的な検証
オンチェーン、ホテルのサービス、設備および運営の管理。

## ポルテ

1. **レジストリのマシンデータ** : マニフェスト、エイリアスを入力する登録 Norito
   継承の連鎖、維持の時代、そして統治の時代。
2. **Implementation du contrat** : CRUD 決定のサイクル ド ヴィーの操作
   ピン (`ReplicationOrder`、`Precommit`、`Completion`、エビクション)。
3. **サービスの外観** : Torii のレジストリ コンソメをサポートするエンドポイント gRPC/REST
   SDK やページネーション、認証など。
4. **ツールとフィクスチャ** : ヘルパー CLI、テストおよびドキュメントの作成ツール
   ガーダーマニフェスト、エイリアス、および統治同期のエンベロープ。
5. **Télémétrie et ops** : レジストリのメトリクス、アラート、およびランブック。

## ドネのモデル

### プリンシポー登録 (Norito)

|構造体 |説明 |チャンピオン |
|----------|---------------|----------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Mappe エイリアス -> CID のマニフェスト。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |プロバイダーのマニフェストの指示。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |プロバイダーの受信者に対する告発。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |統治の政治のスナップショット。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

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

## 備品と CI

- 備品のドシエ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` マニフェスト/エイリアス/注文登録の署名付きスナップショット (`cargo run -p iroha_core --example gen_pin_snapshot` 経由) をストックします。
- CI の作成: `ci/check_sorafs_fixtures.sh` スナップショットと相違点の確認、CI の調整結果の確認。
- 統合テスト (`crates/iroha_core/tests/pin_registry.rs`) ハッピー パスと重複の拒否、承認/保持の保護、一致しないチャンカーの処理、レプリカの検証と継承の検証 (ポイントツール) inconnus/pre-approuvés/retirés/auto-reférences) ; voir les cas `register_manifest_rejects_*` クーベルチュールの詳細を注ぎます。
- `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` の検証単位のテスト、保持および成功のチェック。後継マルチホップの検出はマシンのテストに参加します。
- JSON は、パイプラインの監視を可能にするゴールデン ユーティリティを提供します。

## 遠隔測定と観察可能性

メトリック (Prometheus) :
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- 既存のプロバイダー (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) は、ダッシュボードのスコープをエンドツーエンドで保持します。

ログ:
- Flux d'événements Norito structurés pour les Audits de gouvernance (signés ?)。

警告:
- SLA を無視して複製を行う命令。
- 有効期限 d'alias < seuil。
- 保持違反 (明示的な非更新期限切れ)。

ダッシュボード:
- JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` は、マニフェストのサイクル全体、バックログの飽和度、SLA 比率、レイテンスとスラックのオーバーレイ、およびオンコールのレビューを確認するためのテストに適しています。

## ランブックとドキュメント

- 法定登録簿 `docs/source/sorafs/migration_ledger.md` を含む。
- ガイド操作: `docs/source/sorafs/runbooks/pin_registry_ops.md` (公開情報) の特徴、警告、展開、革新性と再現性。
- 統治ガイド: 政治パラメータの決定、承認のワークフロー、訴訟の提起。
- Chaque エンドポイントの API 参照ページ (ドキュメント Docusaurus)。

## 依存性と順序付け

1. 計画の検証を終了する (ManifestValidator の統合)。
2. スキーマ Norito + 政治的デフォルトのファイナライザ。
3. コントラット + サービスの実装、テレメトリの分岐。
4. 備品を再作成し、統合を実行します。
5. 完了したロードマップの項目を、時間ごとのドキュメント/ランブックとマーケールに記載します。

チェック項目のチェックリスト SF-4 は、計画を参照し、進捗状況を登録します。
REST のエンドポイントの一覧表示の概要:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` および `GET /v1/sorafs/replication` 公開ファイル カタログ
  エイリアス アクションとバックログの順序で複製を実行し、一貫したページ番号を付けます
  法律のフィルタリング。

La CLI カプセル化の申請 (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`) 監査を自動化する操作を許可する
レジストリにはタッチャー補助 API がありません。