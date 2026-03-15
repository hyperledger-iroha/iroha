---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/storage-capacity-marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0d3c80a17440c024895046e29d6e3afe5783859997ff5ff3c0413c010462cb32
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: storage-capacity-marketplace
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
このページは `docs/source/sorafs/storage_capacity_marketplace.md` を反映しています。レガシーなドキュメントが有効な間は両方を揃えてください。
:::

# SoraFS ストレージ容量マーケットプレイス (SF-2c ドラフト)

SF-2c ロードマップ項目は、storage providers がコミットした容量を宣言し、replication orders を受け取り、提供された可用性に比例した fees を獲得する、ガバナンス付き marketplace を導入します。本ドキュメントは初回リリースに必要な deliverables をスコープし、実行可能なトラックに分解します。

## 目的

- providers の容量コミットメント (総バイト、lane ごとの上限、期限) を、governance、SoraNet transport、Torii が利用できる検証可能な形で表現します。
- 宣言済み容量、stake、policy 制約に基づき pins を providers に割り当て、決定論的な挙動を維持します。
- storage 提供 (replication 成功、uptime、整合性 proofs) を計測し、fees 配分向けのテレメトリを出力します。
- 不正な providers を罰則または除外できるよう、revocation と dispute のプロセスを用意します。

## ドメイン概念

| 概念 | 説明 | 初期 deliverable |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | provider ID、chunker profile サポート、コミット済み GiB、lane 別上限、pricing hints、staking commitment、期限を記述する Norito payload。 | `sorafs_manifest::capacity` のスキーマ + バリデータ。 |
| `ReplicationOrder` | ガバナンスが発行する指示で、manifest CID を 1 つ以上の providers に割り当て、冗長度レベルと SLA メトリクスを含む。 | Torii 共有 Norito スキーマ + スマートコントラクト API。 |
| `CapacityLedger` | アクティブな容量宣言、replication orders、パフォーマンスメトリクス、fees の累積を追跡する on-chain/off-chain レジストリ。 | スマートコントラクトモジュール、または決定論的 snapshot を持つ off-chain サービス stub。 |
| `MarketplacePolicy` | 最小 stake、監査要件、ペナルティ曲線を定義する governance ポリシー。 | `sorafs_manifest` の config struct + governance ドキュメント。 |

### 実装済みスキーマ (ステータス)

## 作業分解

### 1. スキーマ & レジストリ層

| タスク | Owner(s) | メモ |
|------|----------|-------|
| `CapacityDeclarationV1`、`ReplicationOrderV1`、`CapacityTelemetryV1` を定義する。 | Storage Team / Governance | Norito を使用し、セマンティックバージョニングと capability 参照を含める。 |
| `sorafs_manifest` に parser + validator モジュールを実装する。 | Storage Team | 単調な IDs、容量上限、stake 要件を強制する。 |
| chunker レジストリ metadata にプロファイルごとの `min_capacity_gib` を追加する。 | Tooling WG | クライアントがプロファイル別の最小ハードウェア要件を強制するのに役立つ。 |
| admission guardrails と penalty schedule をまとめた `MarketplacePolicy` ドキュメントを作成する。 | Governance Council | docs に公開し、policy defaults と並べる。 |

#### スキーマ定義 (実装済み)

- `CapacityDeclarationV1` は、provider ごとの署名済み容量コミットメントを記録し、カノニカルな chunker handles、capability 参照、任意の lane cap、pricing hints、有効期間、metadata を含みます。バリデーションは stake が 0 でないこと、カノニカル handles、重複排除した aliases、宣言総量内の lane cap、GiB の単調な会計を保証します。【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` は、manifests をガバナンス発行の assignments に紐づけ、冗長度ターゲット、SLA しきい値、assignment ごとの保証を含みます。バリデータは Torii またはレジストリが order を取り込む前に、カノニカルな chunker handles、providers の一意性、deadline 制約を強制します。【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` は epoch snapshots (宣言 GiB と使用 GiB、replication カウンタ、uptime/PoR の比率) を表現し、fees 配分に利用されます。境界チェックで利用量を宣言内に、割合を 0-100% に保ちます。【crates/sorafs_manifest/src/capacity.rs:476】
- 共有 helpers (`CapacityMetadataEntry`、`PricingScheduleV1`、lane/assignment/SLA バリデータ) は、決定論的なキー検証とエラーレポートを提供し、CI と downstream tooling が再利用できます。【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` は `/v1/sorafs/capacity/state` で on-chain snapshot を公開し、provider 宣言と fee ledger エントリを決定論的な Norito JSON の背後で統合します。【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- バリデーションカバレッジは、カノニカル handle の強制、重複検出、lane ごとの上限、replication assignment ガード、テレメトリ範囲チェックを行い、リグレッションを CI で即時に検知できるようにします。【crates/sorafs_manifest/src/capacity.rs:792】
- Operator tooling: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` は、人間可読な specs をカノニカルな Norito payloads、base64 blobs、JSON summaries に変換し、オペレータが `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry`、replication order の fixtures をローカル検証付きで準備できるようにします。【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Reference fixtures は `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) にあり、`cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` で生成します。

### 2. コントロールプレーン統合

| タスク | Owner(s) | メモ |
|------|----------|-------|
| Torii ハンドラ `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry`、`/v1/sorafs/capacity/orders` を Norito JSON payloads で追加する。 | Torii Team | バリデーションロジックをミラーし、Norito JSON helpers を再利用する。 |
| `CapacityDeclarationV1` snapshots を orchestrator の scoreboard metadata と gateway fetch plans に反映する。 | Tooling WG / Orchestrator team | `provider_metadata` に capacity 参照を追加し、multi-source scoring が lane 上限を尊重するようにする。 |
| replication orders を orchestrator/gateway clients に投入し、assignments と failover hints を駆動する。 | Networking TL / Gateway team | Scoreboard builder が governance 署名の replication orders を消費する。 |
| CLI tooling: `sorafs_cli` に `capacity declare`、`capacity telemetry`、`capacity orders import` を追加する。 | Tooling WG | 決定論的 JSON と scoreboard outputs を提供する。 |

### 3. マーケットプレイス ポリシー & ガバナンス

| タスク | Owner(s) | メモ |
|------|----------|-------|
| `MarketplacePolicy` (最小 stake、ペナルティ乗数、監査カデンス) を批准する。 | Governance Council | docs に公開し、改訂履歴を記録する。 |
| Parliament が declarations を approve/renew/revoke できるよう governance hooks を追加する。 | Governance Council / Smart Contract team | Norito events + manifest ingestion を使用する。 |
| テレメトリ化された SLA 違反に紐づくペナルティスケジュール (fee 減額、bond slashing) を実装する。 | Governance Council / Treasury | `DealEngine` の settlement outputs に整合させる。 |
| dispute プロセスとエスカレーションマトリクスを文書化する。 | Docs / Governance | dispute runbook + CLI helpers にリンクする。 |

### 4. メータリング & fees 配分

| タスク | Owner(s) | メモ |
|------|----------|-------|
| Torii の metering ingest を拡張して `CapacityTelemetryV1` を受け付ける。 | Torii Team | GiB-hour、PoR 成功、uptime を検証する。 |
| `sorafs_node` の metering pipeline を更新し、order ごとの利用量 + SLA 統計を報告する。 | Storage Team | replication orders と chunker handles に合わせる。 |
| settlement pipeline: テレメトリ + replication データを XOR 建て payouts に変換し、ガバナンス向けサマリーを生成し、ledger 状態を記録する。 | Treasury / Storage Team | Deal Engine / Treasury exports に接続する。 |
| metering の健全性 (ingestion backlog、stale telemetry) の dashboards/alerts を出力する。 | Observability | SF-6/SF-7 で参照される Grafana pack を拡張する。 |

- Torii は `/v1/sorafs/capacity/telemetry` と `/v1/sorafs/capacity/state` (JSON + Norito) を公開し、オペレータが epoch telemetry snapshots を提出し、検査者が監査または証拠パッケージ化のためにカノニカルな ledger を取得できるようにします。【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- `PinProviderRegistry` 統合により replication orders は同じ endpoint から取得でき、CLI helpers (`sorafs_cli capacity telemetry --from-file telemetry.json`) は自動化実行からのテレメトリを決定論的 hashing と alias 解決付きで検証/公開します。
- metering snapshots は `CapacityTelemetrySnapshot` エントリを `metering` snapshot に固定して生成し、Prometheus exports は `docs/source/grafana_sorafs_metering.json` の Grafana ボードに取り込める形式で出力されるため、請求チームは GiB-hour の累積、予測 nano-SORA fees、SLA 順守をリアルタイムで監視できます。【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- metering smoothing を有効にすると、snapshot に `smoothed_gib_hours` と `smoothed_por_success_bps` が含まれ、ガバナンスが payouts に使う生カウンタと EMA トレンド値を比較できます。【crates/sorafs_node/src/metering.rs:401】

### 5. Dispute & Revocation 対応

| タスク | Owner(s) | メモ |
|------|----------|-------|
| `CapacityDisputeV1` payload (申立人、evidence、対象 provider) を定義する。 | Governance Council | Norito スキーマ + バリデータ。 |
| disputes の提出と応答 (evidence attachments 付き) を行う CLI サポート。 | Tooling WG | evidence bundle の決定論的 hashing を保証する。 |
| SLA 違反の繰り返しに対する自動チェック (dispute への auto-escalate) を追加する。 | Observability | alert しきい値と governance hooks。 |
| revocation playbook (grace period、pinned data の退避) を文書化する。 | Docs / Storage Team | policy doc と operator runbook にリンクする。 |

## テスト & CI 要件

- すべての新しいスキーマバリデータのユニットテスト (`sorafs_manifest`)。
- 宣言 → replication order → metering → payout をシミュレートする統合テスト。
- サンプルの capacity declaration/telemetry を再生成し、署名の同期を保証する CI workflow ( `ci/check_sorafs_fixtures.sh` を拡張)。
- registry API の load tests (10k providers、100k orders を想定)。

## テレメトリ & ダッシュボード

- ダッシュボードのパネル:
  - provider ごとの宣言済み容量と使用量。
  - replication order backlog と平均割り当て遅延。
  - SLA 順守 (uptime %、PoR 成功率)。
  - epoch ごとの fee 累積とペナルティ。
- Alerts:
  - 最小コミット容量を下回る provider。
  - SLA を超えて滞留している replication order。
  - metering pipeline の障害。

## ドキュメント deliverables

- 容量宣言、コミット更新、利用状況監視の operator guide。
- 宣言の承認、orders 発行、disputes 対応の governance guide。
- capacity endpoints と replication order 形式の API reference。
- 開発者向け marketplace FAQ。

## GA 準備チェックリスト

ロードマップ項目 **SF-2c** は、会計、dispute 対応、オンボーディングに関する具体的な証拠が揃うまでプロダクションローンチをゲートします。以下の成果物を使い、受け入れ基準を実装と同期させてください。

### Nightly accounting & XOR reconciliation
- 同じウィンドウの capacity state snapshot と XOR ledger export を出力し、以下を実行します:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  このヘルパーは、不足/過払い settlement またはペナルティがあると非ゼロで終了し、Prometheus の textfile summary を出力します。
- `dashboards/alerts/sorafs_capacity_rules.yml` の alert `SoraFSCapacityReconciliationMismatch` は、reconciliation メトリクスが不一致を報告した際に発火します。ダッシュボードは `dashboards/grafana/sorafs_capacity_penalties.json` にあります。
- JSON summary と hashes を `docs/examples/sorafs_capacity_marketplace_validation/` にアーカイブし、governance packets と一緒に保存します。

### Dispute & slashing evidence
- `sorafs_manifest_stub capacity dispute` で disputes を提出します (tests:
  `cargo test -p sorafs_car --test capacity_cli`)。payloads がカノニカルに保たれます。
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` とペナルティ系テスト
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) を実行し、disputes と slashes が決定論的に再生されることを証明します。
- 証拠収集とエスカレーションは `docs/source/sorafs/dispute_revocation_runbook.md` に従い、承認済み strike を validation report にリンクします。

### Provider onboarding & exit smoke tests
- `sorafs_manifest_stub capacity ...` で declaration/telemetry artefacts を再生成し、提出前に CLI tests を再実行します (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Torii (`/v1/sorafs/capacity/declare`) 経由で提出し、その後 `/v1/sorafs/capacity/state` と Grafana のスクリーンショットを取得します。`docs/source/sorafs/capacity_onboarding_runbook.md` の exit flow に従ってください。
- 署名済み artefacts と reconciliation outputs を `docs/examples/sorafs_capacity_marketplace_validation/` にアーカイブします。

## 依存関係 & シーケンス

1. SF-2b (admission policy) を完了する - marketplace は vetted providers に依存します。
2. Torii 統合の前にスキーマ + registry 層 (本ドキュメント) を実装する。
3. payouts を有効化する前に metering pipeline を完了する。
4. 最終ステップ: staging で metering data を検証した後、governance 管理の fee 配分を有効化する。

進捗はこのドキュメントへの参照付きで roadmap に追跡してください。スキーマ、control plane、統合、metering、dispute 対応の主要セクションが feature complete に到達するたびに、roadmap を更新します。
