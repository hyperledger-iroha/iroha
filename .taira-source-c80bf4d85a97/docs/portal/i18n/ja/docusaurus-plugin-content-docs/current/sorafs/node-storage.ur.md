---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-storage.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ノードストレージ
title: SoraFS ノード ストレージ設計
Sidebar_label: ノードのストレージ設計
説明: SoraFS データ ホスト Torii ノード ストレージ アーキテクチャ クォータ ライフサイクル フック
---

:::note メモ
:::

## SoraFS ノードのストレージ設計 (ドラフト)

Iroha (Torii) ノード SoraFS データ可用性レイヤーのオプトインローカル ディスク、チャンク、チャンク、チャンク、およびチャンクの数。 `sorafs_node_client_protocol.md` ディスカバリ仕様 SF-1b フィクスチャ ストレージ側アーキテクチャ リソース制御 構成配管 ノード ゲートウェイ コード パスمیں شامل ہونا ضروری ہے۔訓練
[ノード操作ランブック](./node-operations) میں موجود ہیں۔

### 目標

- 補助 Iroha プロセス スペア ディスク SoraFS プロバイダーの公開 コア元帳の管理੩یے۔
- ストレージ モジュール、決定論的 Norito 駆動型、マニフェスト、チャンク プラン、Proof-of-Retrievability (PoR) ルート、プロバイダー広告、真実のソース、
- オペレータ定義のクォータにより、ノードのピン/フェッチ リクエストが強制され、リソースが制限されます。
- ヘルス/テレメトリ (PoR サンプリング、チャンク フェッチ レイテンシー、ディスク プレッシャー)、ガバナンス、クライアントの管理

### 高レベルのアーキテクチャ

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

主要なモジュール:

- **ゲートウェイ**: Norito HTTP エンドポイントは、ピン プロポーザル、チャンク フェッチ リクエスト、PoR サンプリング、テレメトリを公開します。 Norito ペイロードは、リクエストを検証し、チャンク ストアとマーシャルを検証します。デーモン デーモン Torii HTTP スタックの再利用 ہے۔
- **ピン レジストリ**: マニフェスト ピンの状態 جو `iroha_data_model::sorafs` اور `iroha_core` میں track ہوتی ہے۔マニフェスト受け入れ レジストリ マニフェスト ダイジェスト チャンク プラン ダイジェスト PoR ルート プロバイダー機能フラグ レコード
- **チャンク ストレージ**: ディスク バックアップの `ChunkStore` 実装、署名されたマニフェストの取り込み、`ChunkProfile::DEFAULT` およびチャンク プランの具体化、チャンク、決定論的レイアウト、永続化ありがとうございますチャンク コンテンツ フィンガープリント PoR メタデータ アソシエイト サンプリング サンプリング 再検証 再検証
- **クォータ/スケジューラ**: オペレータが設定した制限 (最大ディスクバイト数、最大未処理ピン数、最大並列フェッチ数、チャンク TTL) により、IO 座標、ノード、台帳義務が強制されます。スケジューラ PoR 証明、サンプリング リクエスト、制限付き CPU の提供、サービスの提供

### 構成

`iroha_config` セクション番号:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # optional human friendly tag
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: 参加切り替えfalse ゲートウェイ ストレージ エンドポイント 503 ノード検出 広告 宣伝 広告
- `data_dir`: チャンク データ、PoR ツリー、フェッチ テレメトリ、ルート ディレクトリデフォルト `<iroha.data_dir>/sorafs` ہے۔
- `max_capacity_bytes`: ピン留めされたチャンク データ (ハード リミット)バックグラウンド タスクの制限 پر پہنچنے پر نئے ピンの拒否 ہے۔
- `max_parallel_fetches`: スケジューラの同時実行上限、帯域幅/ディスク IO、バリデータのワークロード、バランスの調整
- `max_pins`: マニフェスト ピンの最大数とノード数の最大数と、エビクション/バック プレッシャーの適用数。
- `por_sample_interval_secs`: 自動 PoR サンプリング ジョブの頻度ジョブ `N` はサンプルを残します (マニフェストごとに構成可能) テレメトリ イベントが出力しますガバナンス `profile.sample_multiplier` メタデータ キー (整数 `1-4`) `N` 決定論的なスケール値 単一の数値/文字列 プロファイルごとのオーバーライド オブジェクト オブジェクト `{"default":2,"sorafs.sf2@1.0.0":3}`۔
- `adverts`: 構造体プロバイダー広告ジェネレーター `ProviderAdvertV1` フィールド (ステーク ポインター、QoS ヒント、トピック)ノード ガバナンス レジストリのデフォルトを省略します。

構成配管:

- `[sorafs.storage]` `iroha_config` میں `SorafsStorage` کے طور پر 定義 ہے اور ノード構成ファイル سے 読み込み ہوتا ہے۔
- `iroha_core` `iroha_torii` ストレージ構成 スタートアップ ゲートウェイ ビルダー チャンク ストア スレッド ہیں۔
- 開発/テスト環境のオーバーライド (`SORAFS_STORAGE_*`、`SORAFS_STORAGE_PIN_*`)、運用環境のデプロイメント、構成ファイルのオーバーライド

### CLI ユーティリティ

Torii HTTP サーフェス ワイヤー `sorafs_node` クレート Thin CLI の演算子 永続的なバックエンドの演算子取り込み/エクスポート ドリル スクリプト [crates/sorafs_node/src/bin/sorafs-node.rs:1]

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```- `ingest` Norito でエンコードされたマニフェスト `.to` 一致するペイロード バイトが期待されます。マニフェスト チャンク プロファイル チャンク プランの再構築 ダイジェスト パリティの強制 チャンク ファイルの永続化 オプションで `chunk_fetch_specs` JSON BLOB の発行下流のツーリングレイアウトの健全性チェック
- `export` マニフェスト ID 保存されたマニフェスト/ペイロード ディスク (オプションのプラン JSON イメージ) フィクスチャ環境 再現可能な環境

コマンド stdout پر Norito JSON 概要 پرنٹ کرتے ہیں، جسے スクリプト میں パイプ کرنا آسان ہے۔ CLI 統合テストの対象範囲 マニフェスト/ペイロードの往復のテスト Torii API のテスト [crates/sorafs_node/tests/cli.rs:1]

> HTTPパリティ
>
> Torii ゲートウェイ読み取り専用ヘルパーは、`NodeHandle` ベースのデータを公開します。
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — 保存された Norito マニフェスト (base64) ダイジェスト/メタデータ کے ساتھ واپس کرتا ہے۔【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — 決定論的チャンク プラン JSON (`chunk_fetch_specs`) ダウンストリーム ツール
>
> エンドポイント CLI 出力 ミラーリング パイプライン ローカル スクリプト HTTP プローブ パーサー パーサーسکیں۔【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### ノードのライフサイクル

1. **起動**:
   - ストレージの有効化 ノードの構成済みディレクトリの容量 チャンク ストアの初期化PoR マニフェスト データベースの検証/作成 固定マニフェストの再生 ウォーム キャッシュ ウォーム キャッシュ
   - SoraFS ゲートウェイ ルート レジスタ (ピン、フェッチ、PoR サンプル、テレメトリ用の Norito JSON POST/GET エンドポイント)
   - PoR サンプリング ワーカーのクォータ モニターのスポーン
2. **発見/広告**:
3. **ピンのワークフロー**:
   - ゲートウェイ署名マニフェスト (チャンク計画、PoR ルート、評議会署名) (チャンク計画、PoR ルート、評議会署名)エイリアス リストの検証 (`sorafs.sf1@1.0.0` が必要) チャンク プランの確認 マニフェスト メタデータの一致の確認
   - 割り当てのチェック容量/ピン制限がポリシー エラー (構造化 Norito) を超えています。
   - チャンク データ `ChunkStore` ストリーム 取り込み ダイジェスト検証PoR ツリーの更新 マニフェスト メタデータ レジストリ ストアの保存
4. **フェッチワークフロー**:
   - ディスクのチャンク範囲リクエストを処理しますスケジューラ `max_parallel_fetches` は飽和状態を強制します `429` واپس کرتا ہے۔
   - 構造化テレメトリ (Norito JSON) は、レイテンシ、処理されたバイト数、エラー数、ダウンストリーム監視を出力します。
5. **PoR サンプリング**:
   - ワーカー マニフェストの重み (保存されたバイト数) 選択、チャンク ストア、PoR ツリー、決定論的サンプリング
   - 結果、ガバナンス監査、継続、プロバイダーの広告/テレメトリ エンドポイント、概要の確認
6. **エビクション/クォータの強制**:
   - キャパシティ ノードのデフォルト ピンの拒否オプションのオペレーターエビクションポリシー (TTLLRU) ガバナンスモデルの構成厳格なクォータの設計 オペレータによる固定解除操作の設計

### キャパシティ宣言とスケジュールの統合- Torii `/v1/sorafs/capacity/declare` `CapacityDeclarationRecord` 埋め込まれた更新 `CapacityManager` リレー ノード コミットされたチャンカー/レーン割り当てインメモリビューマネージャー テレメトリ 読み取り専用スナップショット (`GET /v1/sorafs/capacity/state`) の公開 注文 プロファイルごと レーンごとの予約の強制ہے۔【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- `/v1/sorafs/capacity/schedule` エンドポイント ガバナンスによって発行された `ReplicationOrderV1` ペイロードローカルプロバイダーの注文 ターゲットの管理 マネージャーの重複スケジューリング チャンカー/レーンの容量の確認 スライスリザーブの確認 `ReplicationPlan` の確認残りの容量、オーケストレーション ツールの取り込み、および残りの容量プロバイダー 注文 `ignored` 応答 承認 承認 マルチオペレーター ワークフロー [crates/iroha_torii/src/routing.rs:4845]
- 完了フック (مثلاً ingestion کامیاب ہونے کے بعد) `POST /v1/sorafs/capacity/complete` کو hit کرتے ہیں تاکہ `CapacityManager::complete_order` کے ذریعے 予約リリースوں۔応答 `ReplicationRelease` スナップショット (残りの合計、チャンカー/レーンの残差) を表示 オーケストレーション ツールのポーリングを表示 注文キューを表示یہ بعد میں チャンク ストア パイプライン کے ساتھ ワイヤー ہوگا جب インジェスト ロジック ランド ہو جائے۔【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- 埋め込み `TelemetryAccumulator` `NodeHandle::update_telemetry` 突然変異、バックグラウンド ワーカー、PoR/稼働時間サンプルの記録正規の `CapacityTelemetryV1` ペイロードは、スケジューラーの内部構造を派生します。 چھیڑے۔【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### 統合と今後の取り組み

- **ガバナンス**: `sorafs_pin_registry_tracker.md` ストレージ テレメトリ (PoR 成功率、ディスク使用率) 拡張機能アドミッション ポリシーの広告を受け入れます。 最小キャパシティ 最小 PoR 成功率が必要です。
- **クライアント SDK**: ストレージ構成 (ディスク制限、エイリアス) を公開し、管理ツールをプログラムでノードのブートストラップを公開します。
- **テレメトリ**: メトリクス スタック (Prometheus / OpenTelemetry) ストレージ メトリクスの可観測性ダッシュボードの統合
- **セキュリティ**: ストレージ モジュール、専用の非同期タスク プール、バック プレッシャー、チャンクの読み取り、io_uring、境界のある tokio プール、サンドボックス、バック プレッシャー悪意のあるクライアントのリソースが枯渇する

デザイン ストレージ モジュール オプションの決定論的演算子 SoraFS データ可用性レイヤー ノブありがとうございます`iroha_config`、`iroha_core`、`iroha_torii`、Norito ゲートウェイ プロバイダー広告ツール تبدیلیاں مانگتا ہے۔