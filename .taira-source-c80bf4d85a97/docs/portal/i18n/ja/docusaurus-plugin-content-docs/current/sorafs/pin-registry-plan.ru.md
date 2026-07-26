---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
タイトル: План реализации Pin レジストリ SoraFS
Sidebar_label: План ピン レジストリ
説明: SF-4、охватывающий мазоину состояний レジストリ、фасад Torii、ツールと наблюдаемость。
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/pin_registry_plan.md`. Держите обе копии синхронизированными, пока наследственная документация остается активной.
:::

# ピン レジストリ SoraFS (SF-4)

SF-4 は、ピン レジストリとピン レジストリを表示します。
マニフェスト、ピン、API、Torii、
Поркестраторов 。 Этот документ раслидации конкретными план валидации конкретными
задачами реализации, охватывая オンチェーン логику, сервисы на стороне хоста,
備品と операционные требования。

## Область

1. **レジストリ**: Norito マニフェスト、エイリアス、
   цепочек преемственности、эпох хранения и метаданных управления。
2. **Реализация контракта**: детерминированные CRUD-операции для жизненного
   цикла ピン (`ReplicationOrder`、`Precommit`、`Completion`、エビクション)。
3. **説明**: gRPC/REST エンドポイント、レジストリおよび
   Torii と SDK が必要です。
4. **ツールとフィクスチャ**: CLI ヘルパー、ツールとツール
   マニフェスト、エイリアス、ガバナンス エンベロープ。
5. **操作**: レジストリ、操作、Runbook のレジストリ。

## Модель данных

### Основные записи (Norito)

| Структура | Описание | Поля |
|----------|----------|------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` |エイリアス -> CID マニフェストを選択します。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |プロバイダーのマニフェストを確認します。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` | Подтверждение провайдера。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` | Снимок политики управления。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

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

- フィクスチャ: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` スナップショット マニフェスト/エイリアス/順序、`cargo run -p iroha_core --example gen_pin_snapshot` のスナップショット。
- CI: `ci/check_sorafs_fixtures.sh` スナップショットと差分、フィクスチャ CI が表示されます。
- Интеграционные тесты (`crates/iroha_core/tests/pin_registry.rs`) ハッピー パス плюс отказ при дублировании エイリアス、ガード одобрения/хранения エイリアス、 несовпадающие はチャンカーを処理し、ガードを保持します。 (неизвестные/предодобренные/выведенные/самоссылки);最低。 кейсы `register_manifest_rejects_*` 日、今日は。
- Юнит-тесты теперь покрывают валидацию エイリアス、ガード хранения и проверки премника в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение многоственности появится, когда заработает мазина состояний.
- Golden JSON は、最新の機能を備えています。

## Телеметрия и наблюдаемость

Метрики (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- プロバイダー (`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`) をエンドツーエンドで接続します。

例:
- Структурированный поток событий Norito для аудиторов управления (подписанные?)

Алерты:
- SLA を使用してください。
- Истечение срока 別名 ниже порога。
- Наруления хранения (マニフェスト не продлен до истечения)。

説明:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` 合計のマニフェスト、エイリアス、バックログ、SLA 比率、オーバーレイのレイテンシとスラックの比較オンコール中です。

## Runbook と документация

- Обновить `docs/source/sorafs/migration_ledger.md`、レジストリを確認してください。
- Руководство оператора: `docs/source/sorafs/runbooks/pin_registry_ops.md` (уже опубликовано) с метриками、алертингом、развертыванием、backup およびПосстановлением。
- Руководство по управлению: описать параметры политики、ワークフロー одобрения、обработку споров.
- API エンドポイントの説明 (Docusaurus ドキュメント)。

## Зависимости и последовательность

1. マニフェスト検証 (ManifestValidator) を実行します。
2. Norito + デフォルト値を入力します。
3. Реализовать контракт + сервис, подключить телеметрию.
4. フィクスチャ、スイートを備えています。
5. ドキュメント/Runbook およびロードマップを参照してください。

SF-4 のテストが完了しました。
REST はエンドポイントを維持します:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` と `GET /v1/sorafs/replication` は、
  別名とバックログ заказов репликации с консистентной пагинацией и
  Єильтрами статуса。

CLI によるアクセス (`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`)、レジストリを取得する
API を使用します。