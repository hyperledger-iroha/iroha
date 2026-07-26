---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
タイトル: خطة تنفيذ Pin Registry في SoraFS
Sidebar_label: ピン レジストリ
説明: SF-4 レジストリ Torii 。
---

:::note ノート
評価は `docs/source/sorafs/pin_registry_plan.md` です。最高のパフォーマンスを見せてください。
:::

# 認証ピン レジストリ SoraFS (SF-4)

SF-4 の Pin レジストリとマニフェストの管理
API を Torii にピン留めしてください。
يوسّع هذا المستند خطة التحقق بمهام تنفيذية ملموسة تغطي المنطق on-chain،
試合は、試合の試合結果を表します。

## ああ

1. **レジストリ**: Norito マニフェストのエイリアス
   ログインしてください。
2. **評価: CRUD ピン (`ReplicationOrder`、`Precommit`、
   `Completion`、エビクション)。
3. **واجهة الخدمة**: gRPC/REST レジストリ تستهلكها Torii وSDKs،
   ありがとうございます。
4. **フィクスチャ**: CLI の ومتجهات اختبار ووثائق تحافظ على تزامن
   マニフェスト、エイリアス、封筒など。
5.***: مقاييس وتنبيهات وrunbooks レジストリ。

## いいえ

### السجلات الاساسية (Norito)

|ああ |ああ |ああ |
|----------|----------|----------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` |エイリアス -> CID マニフェスト。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |マニフェストを表示します。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |ありがとうございます。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |ありがとうございます。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

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

## 試合日程とCI

- フィクスチャ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` マニフェスト/エイリアス/オーダー `cargo run -p iroha_core --example gen_pin_snapshot`。
- CI: `ci/check_sorafs_fixtures.sh` 試合の試合結果CI です。
- バージョン (`crates/iroha_core/tests/pin_registry.rs`) バージョン バージョン バージョン バージョン バージョン バージョン バージョン バージョン バージョン バージョン処理チャンカーを処理します。 عليها مسبقا/مسحوبة/ذاتية الاشارة)؛ `register_manifest_rejects_*` は、`register_manifest_rejects_*` を意味します。
- エイリアス エイリアス エイリアス `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ كشف التعاقب متعدد القفزات عند وصول آلة الحالات.
- JSON は、 للاحداث المستخدمة في خطوط مراقبة الرصد 。

## いいえ

回答 (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- エンドツーエンドのテスト (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) を実行します。

説明:
- تحداث Norito منظم لتدقيقات الحوكمة (موقع؟)。

回答:
- SLA を使用します。
- انتهاء صلاحية 別名 اقل من العتبة。
- مخالفات الاحتفاظ (マニフェスト لم يجدد قبل الانتهاء)。

概要:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` ステータス、マニフェスト、エイリアス、バックログ、SLA レイテンシ緩みのないように、スラックをチェックしてください。

## ランブック

- `docs/source/sorafs/migration_ledger.md` レジストリ。
- メッセージ: `docs/source/sorafs/runbooks/pin_registry_ops.md` (منشور حاليا) يغطي المقاييس والتنبيه والنشر والنسخ الاحتياطي واستعادةああ。
- ニュース: ニュース - ニュース、ニュース、ニュース、ニュース。
- API を使用してください (وثائق Docusaurus)。

## いいえ

1. マニフェスト検証 (ManifestValidator)。
2. انهاء مخطط Norito + قيم السياسة الافتراضية。
3. تنفيذ العقد + الخدمة وربط التليمتري.
4. 試合の試合結果。
5. ランブック/ランブックをダウンロードしてください。

SF-4 を開発し、SF-4 を開発しました。
休息時間:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` と `GET /v1/sorafs/replication` のエイリアス
  あなたのことを忘れないでください。

CLI を使用する (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`) レジストリ بدون لمس
API を使用してください。