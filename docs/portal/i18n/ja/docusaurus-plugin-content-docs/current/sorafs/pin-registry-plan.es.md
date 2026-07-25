---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
タイトル: SoraFS のピン レジストリの実装計画
Sidebar_label: プラン デル ピン レジストリ
説明: SF-4 の実装計画は、レジストリの牧草地、ファチャダ Torii、ツールと監視を実行します。
---

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/sorafs/pin_registry_plan.md`。満天のコピアス シンクロニザダス ミエントラス ラ ドキュメンタシオン ヘレダダ シガ アクティバ。
:::

# SoraFS (SF-4) のピン レジストリの実装を計画する

SF-4 entrega el contrato del Pin Registry y los servicios de soporte que almacenan
マニフェストの侵害、ピンの指数 API の不正侵入、Torii、ゲートウェイ
オルケスタドールよ。検証に関する計画に関する文書の作成
実装コンクリート、オンチェーンのロジック、ロス サービス、ロス
必要なオペラティボスの試合。

## アルカンス

1. **レジストリのマキナ**: Norito パラマニフェストの定義済みレジストリ、
   エイリアス、cadenas sucesoras、epocas de retencion y metadatos de gobernanza。
2. **コントラートの実装**: 自動制御の CRUD 決定操作
   ピン (`ReplicationOrder`、`Precommit`、`Completion`、エビクション)。
3. **サービスの機能**: エンドポイント gRPC/REST レジストリ クエリの消費
   Torii には SDK がありません。ページやアテスタシオンも含まれます。
4. **ツールとフィクスチャ**: CLI のヘルパー、管理者のプルエバとドキュメントのベクトル
   マニフェスト、エイリアス、エンベロープ デ ゴベルナンザ シンクロニザドス。
5. **テレメトリと運用**: レジストリのメトリクス、アラートおよびランブック。

## モデロ デ ダトス

### 中央レジストロス (Norito)

|構造 |説明 |カンポス |
|-----------|---------------|----------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Mapea エイリアス -> CID のマニフェスト。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |プロバイダーのマニフェストに関する指示。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |プロバイダーに対する非難。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |ゴベルナンザの政治のスナップショット。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

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

- フィクスチャのディレクトリ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` は、`cargo run -p iroha_core --example gen_pin_snapshot` のマニフェスト/エイリアス/順序再生成のスナップショット ファームを保護します。
- CI のパス: `ci/check_sorafs_fixtures.sh` スナップショットとフォールラ SI ヘイの差分、CI ラインのフィクスチャを管理します。
- 統合テスト (`crates/iroha_core/tests/pin_registry.rs`) エイリアス重複の除去、不正行為/エイリアス保持のガード、チャンカーの塩漬け処理の処理、レプリカのコンテオ検証、および継承の失敗 (パンテロス)デスコノシドス/プレアプロバドス/レティラドス/オートリファレンス);バージョン casos `register_manifest_rejects_*` パラ詳細。
- `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` のエイリアス、ガード、保持、および後継者のチェックをテストします。マルチホップの成功を検出し、マキナ デ スタドスを検出します。
- JSON ゴールデンパライベント、監視パイプライン。

## テレメトリアと観察

メトリカ (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- プロバイダー (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) がエンドツーエンドでダッシュボードに存在するテレメトリ。

ログ:
- イベント Norito のオーディオ構造 (firmados?) のストリーム。

アラート:
- SLA を超えて複製を行う手順。
- 別名有効期限。
- Violaciones de retencion (期限切れになるまでの期限を明示しません)。

ダッシュボード:
- Grafana `docs/source/grafana_sorafs_pin_registry.json` のマニフェストのラストレア合計、エイリアスのコベルチュラ、バックログの飽和率、SLA の比率、レイテンシアとスラックのオーバーレイ、オンコールでのリビジョンの調整。

## ランブックとドキュメント

- Actualizar `docs/source/sorafs/migration_ledger.md` には、レジストリの実際の情報が含まれています。
- オペラドールの操作: `docs/source/sorafs/runbooks/pin_registry_ops.md` (公開) メトリカス、アラート、デスリーグ、バックアップと回復のフルホス。
- 政治政策: 政治パラメタ、不正行為のワークフロー、論争の管理に関する記述。
- API のエンドポイントに関する参照ページ (Docusaurus ドキュメント)。

## 依存関係と優先順位

1. 検証計画の完全な領域 (ManifestValidator の統合)。
2. 最終的なエスケマ Norito + 政治的デフォルト。
3. コントラート + サービス、コネクター テレメトリアを実装します。
4. 再生フィクスチャ、統合スイートの修正。
5. ドキュメント/ランブックとロードマップのマーク項目を完全に把握します。

SF-4 のチェックリストは、エステ プランの進捗状況を参照します。
REST のアホラ エントレガ エンドポイントの一覧表示:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` y `GET /v1/sorafs/replication` 指数関数カタログ
  エイリアス アクティビティ、バックログ、レプリケーションの順序、ページの一貫性
  フィルトロス・デ・エスタド。

La CLI envuelve estas llamadas (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`) パラ ケ ロス オペラドール プエダン オートマティザール オーディトリアス デル
レジストリは、Bajo Nivel の API に登録されています。