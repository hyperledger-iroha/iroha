---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
タイトル: SoraFS によるピン レジストリの実装計画
サイドバーラベル: Plano do Pin レジストリ
説明: SF-4 の実装計画、レジストリのマキナ、ファチャダ Torii、ツールの監視。
---

:::note フォンテ カノニカ
エスタページナリフレテ`docs/source/sorafs/pin_registry_plan.md`。マンテンハは、記録を記録するための記録として、永続的な活動を記録します。
:::

# SoraFS (SF-4) を使用してピン レジストリを実装する計画

O SF-4 entrega o contrato do Pin Registry e os servicos de apoio que armazenam
Torii に関する API のマニフェストでの妥協、マニフェストでの政治的不法行為、
ゲートウェイとオルケストラドール。 Este documento amplia o plano de validacao com
具体的な実装方法、オンチェーンでのロジックのコブリンド、OS サービスの実行
ホスト、OS フィクスチャ、OS の操作が必要です。

## エスコポ

1. **Maquina de estados do registry**: Norito パラ マニフェストのレジストリ定義、
   エイリアス、cadias sucessoras、epocas de retencao および metadados de Governmenta。
2. **コントラートの実装**: 自動 CRUD の決定性を実現するためのオペラ
   dos ピン (`ReplicationOrder`、`Precommit`、`Completion`、エビクション)。
3. **サービスの機能**: エンドポイント gRPC/REST sustentados pelo registry que Torii
   os SDK のコンソメムには、paginacao や atestacao が含まれます。
4. **ツールとフィクスチャ**: CLI のヘルパー、テストおよびドキュメントのパラメータの管理
   マニフェスト、エイリアス、エンベロープ デ ガバナンカ シンクロニザドス。
5. **Telemetria e ops**: レジストリに関するメトリクス、アラート、ランブック。

## モデロ デ ダドス

### レジストロス セントライス (Norito)

|構造体 |説明 |カンポス |
|----------|----------|----------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Mapeia エイリアス -> CID のマニフェスト。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |プロバイダーの修正プログラムまたはマニフェストの説明。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |プロバイダーを確認してください。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |政府の政治のスナップショット。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

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

- フィクスチャのディレクトリ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` マニフェスト/エイリアス/オーダーの再生成のスナップショット `cargo run -p iroha_core --example gen_pin_snapshot`。
- CI の Etapa: `ci/check_sorafs_fixtures.sh` スナップショットとファルハセ フーバー差分、CI アリニャドスのマンテンド OS フィクスチャを再生成します。
- Testes de integracao (`crates/iroha_core/tests/pin_registry.rs`) は、別名重複を再確認し、別名を保護/別名を保持し、チャンカーの互換性のないハンドルを処理し、レプリカの感染を検証し、安全性を確認します (ポンテイロス)。デスコンヘシドス/プレアプロバドス/レティラドス/オートリファレンス); veja casos `register_manifest_rejects_*` パラ デタルヘス デ コベルトゥーラ。
- 別名、保護者が後継者をチェックするためのテスト ユニタリオス アゴラ コブレム バリダカオ `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`。マルチホップを確実に実行し、マキナ デ スタドス チェガールを実行します。
- JSON ゴールデンパライベント米国ペロスパイプラインの監視。

## テレメトリアと観察

メトリカ (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- プロバイダー (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) は、ダッシュボードをエンドツーエンドで永続的に管理します。

ログ:
- イベント Norito のストリーム パラ オーディトリアス デ ガバナンカ (アッシナドス?)。

アラート:
- SLA を超えた複製の順序。
- 別名 abaixo do limiar を有効にしてください。
- Violacoes de retencao (マニフェスト nao renovado antes de expirar)。

ダッシュボード:
- O JSON は、Grafana `docs/source/grafana_sorafs_pin_registry.json` rastreia totais の ciclo de vida dos マニフェスト、cobertura de alias、saturacao do backlog、razao de SLA、overlays de latencia とlack、on-call での定期的な見直しを行います。

## ランブックとドキュメント

- Atualizar `docs/source/sorafs/migration_ledger.md` には、ステータスのレジストリが含まれています。
- 操作方法: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ja publicado) コブリンド メトリクス、アラート、デプロイ、バックアップ、および回復方法。
- 政府の政策: 政治パラメタ、承認ワークフロー、紛争処理。
- API のエンドポイントに関する参照ページ (ドキュメント Docusaurus)。

## 依存性とシーケンス

1. 完全なデータ検証計画 (ManifestValidator の統合)。
2. 最終的なエスケマ Norito + 政治的デフォルト。
3. contrato + servico、conectar テレメトリアを実装します。
4. 再生設備、統合されたロッドスイート。
5. Atualizar のドキュメント/Runbook とロードマップを完全に作成します。

CADA チェックリストは、SF-4 を参照して、エステ プランノ クアンド フーバー プログレスを実行します。
com atestacao でエンドポイントのリストを作成する REST の詳細:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` および `GET /v1/sorafs/replication` カタログの説明
  別名、バックログ、レプリカのコンパニオンの順序、一貫性
  ステータスのフィルター。

CLI カプセル化 essas Chamadas (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`) パラ・ケ・オペラドール・ポッサム・オートマティザー・オーディトリアス・ドゥ
レジストリとバイショニベルの API を組み合わせます。