---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-storage.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 692af855445afd6fadf269a1b6c0c0cdb25e3b110cabf858cc59024f462d3956
source_last_modified: "2025-12-19T22:32:23.822586+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: node-storage
title: SoraFS ノードストレージ設計
sidebar_label: ノードストレージ設計
description: SoraFS データをホストする Torii ノードのストレージ構成、クォータ、ライフサイクルフック。
---

:::note 正規ソース
このページは `docs/source/sorafs/sorafs_node_storage.md` を反映しています。レガシーの Sphinx ドキュメントセットが退役するまで両方を同期してください。
:::

## SoraFS ノードストレージ設計 (ドラフト)

このノートは、Iroha (Torii) ノードが SoraFS データ可用性レイヤーに参加し、ローカルディスクの一部をチャンクの保存・提供に割り当てる方法を整理します。`sorafs_node_client_protocol.md` の discovery 仕様と SF-1b の fixture 作業を補完し、ストレージ側のアーキテクチャ、リソース制御、ノードと gateway のコードパスに必要な設定配線を示します。実践的な運用手順は [ノード運用ランブック](./node-operations) を参照してください。

### 目標

- いずれのバリデータ/補助 Iroha プロセスでも、台帳の責務に影響を与えずに余剰ディスクを SoraFS プロバイダとして公開できるようにする。
- ストレージモジュールを決定的かつ Norito 主導に保つ。manifest、チャンク計画、Proof-of-Retrievability (PoR) ルート、プロバイダ advert が真実の情報源となる。
- オペレータ定義のクォータを適用し、過剰な pin/fetch 要求でノード自身の資源を枯渇させない。
- 健全性/テレメトリ (PoR サンプリング、チャンク取得レイテンシ、ディスク圧力) をガバナンスとクライアントへ返す。

### ハイレベル構成

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

主なモジュール:

- **Gateway**: pin 提案、チャンク fetch 要求、PoR サンプリング、テレメトリの Norito HTTP エンドポイントを公開します。Norito ペイロードを検証し、要求をチャンクストアへ中継します。新しいデーモンを追加せず、既存の Torii HTTP スタックを再利用します。
- **Pin Registry**: `iroha_data_model::sorafs` と `iroha_core` で追跡される manifest の pin 状態。manifest を受け入れたとき、manifest digest、チャンク計画 digest、PoR ルート、プロバイダ能力フラグを記録します。
- **Chunk Storage**: ディスクバックの `ChunkStore` 実装で、署名済み manifest を取り込み、`ChunkProfile::DEFAULT` によりチャンク計画を具現化し、決定的なレイアウトでチャンクを保存します。各チャンクはコンテンツ指紋と PoR メタデータに紐づけられ、全ファイルを読み直さずにサンプリング検証できます。
- **Quota/Scheduler**: オペレータが設定する制限 (最大ディスクバイト、最大保留 pin、最大並列 fetch、チャンク TTL) を適用し、ノードの台帳タスクが枯渇しないよう IO を調整します。スケジューラは PoR 証明とサンプリング要求を CPU 制限付きで提供する責務も持ちます。

### 設定

`iroha_config` に新しいセクションを追加します:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # 任意の人間向けタグ
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: 参加トグル。false の場合、gateway はストレージエンドポイントに 503 を返し、ノードは discovery に広告しません。
- `data_dir`: チャンクデータ、PoR ツリー、fetch テレメトリのルートディレクトリ。デフォルトは `<iroha.data_dir>/sorafs`。
- `max_capacity_bytes`: pin されたチャンクデータのハードリミット。バックグラウンドタスクが上限到達時に新規 pin を拒否します。
- `max_parallel_fetches`: スケジューラが適用する並列上限で、帯域/ディスク IO とバリデータ負荷のバランスを取ります。
- `max_pins`: ノードが受け入れる manifest pin の上限。超過時に eviction/back pressure を適用します。
- `por_sample_interval_secs`: 自動 PoR サンプリングジョブの間隔。各ジョブは `N` 枚の葉 (manifest ごとに設定可能) をサンプルし、テレメトリイベントを発行します。ガバナンスはメタデータキー `profile.sample_multiplier` (整数 `1-4`) を設定して `N` を決定的にスケールできます。値は単一の数値/文字列またはプロファイル別オーバーライドのオブジェクト (例: `{"default":2,"sorafs.sf2@1.0.0":3}`) にできます。
- `adverts`: プロバイダ advert 生成が `ProviderAdvertV1` フィールド (stake pointer, QoS ヒント, topics) を埋めるための構造。省略時はガバナンスレジストリのデフォルトを使います。

設定配線:

- `[sorafs.storage]` は `iroha_config` で `SorafsStorage` として定義され、ノード設定ファイルから読み込まれます。
- `iroha_core` と `iroha_torii` はストレージ設定を gateway builder とチャンクストアへ起動時に渡します。
- 開発/テスト用の環境変数オーバーライド (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`) はありますが、プロダクションでは設定ファイルに依存してください。

### CLI ユーティリティ

Torii の HTTP サーフェスを配線中の間、`sorafs_node` クレートは軽量 CLI を提供し、オペレータが永続バックエンドに対する取り込み/エクスポートのドリルを自動化できます。【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` は Norito エンコードされた manifest `.to` と対応する payload バイトを受け取ります。manifest のチャンクプロファイルからチャンク計画を再構築し、digest の整合性を強制し、チャンクファイルを保存し、必要に応じて `chunk_fetch_specs` JSON ブロブを出力して下流ツールがレイアウトを検証できるようにします。
- `export` は manifest ID を受け取り、保存済み manifest/payload をディスクへ書き出します (任意で plan JSON も出力)。これにより fixtures を環境間で再現可能にします。

両コマンドは Norito JSON の要約を stdout に出力するため、スクリプトに取り込みやすくなっています。CLI は統合テストでカバーされ、Torii API が入る前に manifest/payload の往復が正しいことを保証します。【crates/sorafs_node/tests/cli.rs:1】

> HTTP パリティ
>
> Torii gateway は同じ `NodeHandle` を使う読み取り専用ヘルパーを公開しています:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — 保存済み Norito manifest (base64) と digest/メタデータを返します。【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — 決定的なチャンク計画 JSON (`chunk_fetch_specs`) を下流ツール向けに返します。【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> これらのエンドポイントは CLI 出力を反映しているため、パイプラインはローカルスクリプトから HTTP プローブへ切り替えてもパーサを変更せずに済みます。【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### ノードライフサイクル

1. **起動**:
   - ストレージが有効な場合、ノードは設定ディレクトリと容量でチャンクストアを初期化します。これには PoR manifest データベースの検証/作成と、pin 済み manifest の再生によるキャッシュのウォームアップが含まれます。
   - SoraFS gateway ルート (pin/fetch/PoR サンプル/テレメトリの Norito JSON POST/GET エンドポイント) を登録します。
   - PoR サンプリングワーカーとクォータモニタを起動します。
2. **Discovery / Adverts**:
   - 現在の容量/健全性に基づき `ProviderAdvertV1` を生成し、評議会承認キーで署名して discovery チャネルへ公開します。新しい `profile_aliases` リストで正規/レガシーのハンドルを維持します。
3. **Pin ワークフロー**:
   - ゲートウェイは署名済み manifest (チャンク計画、PoR ルート、評議会署名を含む) を受け取ります。エイリアスリスト (`sorafs.sf1@1.0.0` 必須) を検証し、チャンク計画が manifest メタデータと一致することを確認します。
   - クォータを確認し、容量/pin 制限を超える場合は Norito 構造化のポリシーエラーを返します。
   - チャンクデータを `ChunkStore` へストリームし、取り込み中に digest を検証します。PoR ツリーを更新し、manifest メタデータをレジストリに保存します。
4. **Fetch ワークフロー**:
   - ディスクからチャンクの range 要求を返します。スケジューラは `max_parallel_fetches` を適用し、飽和時は `429` を返します。
   - レイテンシ、提供バイト数、エラーカウントを Norito JSON の構造化テレメトリとして出力し、下流の監視に提供します。
5. **PoR サンプリング**:
   - ワーカーは重み (例: 保存バイト数) に比例して manifest を選び、チャンクストアの PoR ツリーで決定的サンプリングを実行します。
   - 結果をガバナンス監査のために保存し、プロバイダ advert / テレメトリエンドポイントに要約を含めます。
6. **エビクション / クォータ適用**:
   - 容量に達した場合、デフォルトで新規 pin を拒否します。ガバナンスモデルが合意されれば TTL/LRU などのエビクションポリシーを設定可能ですが、現状の設計は厳格クォータとオペレータ主導の unpin 操作を前提とします。

### 容量宣言とスケジューリング連携

- Torii は `/v1/sorafs/capacity/declare` の `CapacityDeclarationRecord` 更新を組み込み `CapacityManager` に中継するため、各ノードはチャンクャ/レーン割当のメモリ内ビューを構築します。マネージャはテレメトリ向けの read-only スナップショット (`GET /v1/sorafs/capacity/state`) を公開し、新規オーダー受理前にプロファイル/レーンごとの予約を適用します。【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- `/v1/sorafs/capacity/schedule` エンドポイントはガバナンス発行の `ReplicationOrderV1` を受け取ります。オーダーがローカルプロバイダを対象とする場合、マネージャは重複スケジュールを確認し、チャンクャ/レーン容量を検証し、スライスを予約し、残容量を示す `ReplicationPlan` を返します。その他のプロバイダ向けオーダーは `ignored` 応答で acknowledge し、マルチオペレータのワークフローを円滑にします。【crates/iroha_torii/src/routing.rs:4845】
- 完了フック (例: 取り込み成功後にトリガ) は `POST /v1/sorafs/capacity/complete` を呼び、`CapacityManager::complete_order` で予約を解放します。応答は `ReplicationRelease` スナップショット (残り総量、チャンクャ/レーン残量) を含み、オーケストレーションツールがポーリングなしで次のオーダーをキューできます。今後の作業で取り込みロジックが入ったらチャンクストアパイプラインに接続します。【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- 組み込み `TelemetryAccumulator` は `NodeHandle::update_telemetry` で更新でき、バックグラウンドワーカーが PoR/稼働サンプルを記録し、`CapacityTelemetryV1` の正規ペイロードをスケジューラ内部に触れずに導出できるようになります。【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### 統合と将来作業

- **ガバナンス**: `sorafs_pin_registry_tracker.md` をストレージテレメトリ (PoR 成功率、ディスク利用率) で拡張する。入場ポリシーは advert 受理前に最小容量や最小 PoR 成功率を要求できる。
- **クライアント SDK**: 新しいストレージ設定 (ディスク上限、エイリアス) を公開し、管理ツールがノードをプログラム的にブートストラップできるようにする。
- **テレメトリ**: 既存のメトリクススタック (Prometheus / OpenTelemetry) と統合し、ストレージメトリクスが観測ダッシュボードに表示されるようにする。
- **セキュリティ**: ストレージモジュールを専用の async タスクプールで動かし back-pressure を適用し、io_uring や tokio の制限プールでチャンク読み取りをサンドボックス化して悪意あるクライアントによる資源枯渇を防ぐ。

この設計はストレージモジュールをオプションかつ決定的に保ちつつ、オペレータが SoraFS データ可用性レイヤーに参加するためのノブを提供します。実装には `iroha_config`、`iroha_core`、`iroha_torii`、Norito gateway、およびプロバイダ advert ツールの変更が必要です。
