---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ノードプラン
title: SoraFS ノード実装計画
Sidebar_label: ノード実装計画
説明: SF-3 ストレージ ロードマップ、マイルストーン、タスク、テスト カバレッジ、実用的なエンジニアリング作業、重要な作業
---

:::note メモ
:::

SF-3 実行可能 `sorafs-node` クレート Iroha/Torii プロセス SoraFS ストレージ プロバイダー❁❁❁❁ [ノード ストレージ ガイド](node-storage.md) [プロバイダー アドミッション ポリシー](provider-admission-policy.md) [ストレージ容量市場のロードマップ](storage-capacity-marketplace.md)成果物の順序

## 対象範囲 (マイルストーン M1)

1. **チャンク ストアの統合。** `sorafs_car::ChunkStore` 永続バックエンド ラップ 構成されたデータ ディレクトリ チャンク バイト マニフェスト PoR ツリー
2. **ゲートウェイ エンドポイント** Torii プロセス、ピンの送信、チャンクのフェッチ、PoR サンプリング、ストレージ テレメトリ、Norito HTTP エンドポイントのエクスポーズ
3. **構成の配管** `SoraFsStorage` 構成構造体 (フラグ、容量、ディレクトリ、同時実行制限の有効化) `iroha_config`、`iroha_core`、`iroha_torii`ワイヤー ੩ریں۔
4. **クォータ/スケジューリング** オペレータ定義のディスク/並列処理の制限により、リクエスト、バックプレッシャー、キューが強制されます。
5. **テレメトリ** ピンの成功、チャンクフェッチのレイテンシ、容量使用率、PoR サンプリング結果、メトリクス/ログの出力、

## 作業の内訳

### A. クレートとモジュールの構造

|タスク |所有者 |メモ |
|------|----------|------|
| `crates/sorafs_node` モジュール `config`、`store`、`gateway`、`scheduler`、`telemetry` モジュール|ストレージチーム | Torii 統合 再利用可能なタイプの再エクスポート|
| `StorageConfig` 実装 `SoraFsStorage` マッピング (ユーザー → 実際 → デフォルト) |ストレージ チーム / 構成 WG | Norito/`iroha_config` レイヤーの決定性|
| Torii ピン/フェッチ `NodeHandle` ファサード فراہم کریں۔ |ストレージチーム |ストレージの内部構造、非同期配管、カプセル化|

### B. 永続的なチャンク ストア

|タスク |所有者 |メモ |
|------|----------|------|
| `sorafs_car::ChunkStore` ディスク上のマニフェスト インデックス (`sled`/`sqlite`) ディスク バックエンドのラップ|ストレージチーム |確定的レイアウト: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`。 |
| `ChunkStore::sample_leaves` PoR メタデータ (64 KiB/4 KiB ツリー) が維持する|ストレージチーム |再起動とリプレイのサポート汚職は早く失敗する|
|スタートアップの完全性リプレイの実装 (マニフェストの再ハッシュ、不完全なピンのプルーン) |ストレージチーム |リプレイ مکمل ہونے تک Torii 開始ブロック کریں۔ |

### C. ゲートウェイエンドポイント

|エンドポイント |行動 |タスク |
|----------|-----------|----------|
| `POST /sorafs/pin` | `PinProposalV1` マニフェストは取り込みキューを検証します マニフェスト CID واپس دیں۔ |チャンク プロファイルの検証 割り当て割り当ての強制 チャンク ストア データ ストリーム|
| `GET /sorafs/chunks/{cid}` + 範囲クエリ | `Content-Chunker` ヘッダーはチャンク バイトを提供します範囲能力スペック尊重|スケジューラ + ストリーム バジェット (SF-2d 範囲機能と関連付け) |
| `POST /sorafs/por/sample` |マニフェスト、PoR サンプリング、プルーフ バンドル、PoR サンプリング|チャンク ストアのサンプリングの再利用 Norito JSON ペイロードの応答|
| `GET /sorafs/telemetry` |概要: 容量、PoR 成功、フェッチ エラー数。 |ダッシュボード/オペレーター データの管理|

ランタイム配管 `sorafs_node::por` スレッド PoR インタラクション: トラッカー `PorChallengeV1`、`PorProofV1`、`AuditVerdictV1` レコード`CapacityMeter` メトリクス ガバナンス評決 Torii 固有のロジックを反映する [crates/sorafs_node/src/scheduler.rs#L147]

実装メモ:

- Torii Axum スタック `norito::json` ペイロード数
- 応答 Norito スキーマ (`PinResultV1`、`FetchErrorV1`、テレメトリ構造体)

- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` バックログの深さ 最も古いエポック/期限 プロバイダー 最近の成功/失敗のタイムスタンプ `sorafs_node::NodeHandle::por_ingestion_status` パワード ہے، Torii ダッシュボード `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` ゲージ レコードہے۔【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. スケジューラとクォータの適用|タスク |詳細 |
|-----|----------|
|ディスク クォータ |ディスクバイトトラック`max_capacity_bytes` ピンが拒否されましたセキュリティ ポリシー セキュリティ 立ち退きフック セキュリティ ポリシー|
|フェッチの同時実行性 |グローバル セマフォ (`max_parallel_fetches`) プロバイダーごとの予算 SF-2d 範囲の上限|
|ピンキュー |優れた取り込みジョブキューの深さ Norito ステータス エンドポイントが公開する|
| PoR の頻度 | `por_sample_interval_secs` バックグラウンド ワーカー|

### E. テレメトリとロギング

メトリック (Prometheus):

- `sorafs_pin_success_total`、`sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (`result` ラベル付きのヒストグラム)
- `torii_sorafs_storage_bytes_used`、`torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`、`torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`、`torii_sorafs_storage_por_samples_failed_total`

ログ/イベント:

- ガバナンス取り込み構造化 Norito テレメトリ (`StorageTelemetryV1`)。
- 使用率 > 90% PoR 連続失敗しきい値とアラート数

### F. テスト戦略

1. **単体テスト** チャンク ストアの永続性、クォータ計算、スケジューラの不変条件 (`crates/sorafs_node/src/scheduler.rs` を参照)。
2. **統合テスト** (`crates/sorafs_node/tests`)。 Pin → フェッチラウンドトリップ、リスタートリカバリ、クォータ拒否、PoR サンプリングプルーフ検証
3. **Torii 統合テスト。** Torii ストレージ有効化 HTTP エンドポイント `assert_cmd` 演習 演習
4. **カオス ロードマップ。** ディスク枯渇を引き起こす、IO が遅い、プロバイダーの削除をシミュレートする

## 依存関係

- SF-2b 入場ポリシー — ノードが広告を公開し、入場封筒が確認する
- SF-2c 容量マーケットプレイス — テレメトリと容量宣言の関係
- SF-2D 広告拡張機能 — 範囲機能 + ストリーム予算を大幅に消費します

## マイルストーンの終了基準

- `cargo run -p sorafs_node --example pin_fetch` ローカル備品 کے خلاف کام کرے۔
- Torii `--features sorafs-storage` ビルド 統合テスト 統合テスト
- ドキュメント ([ノード ストレージ ガイド](node-storage.md)) 構成のデフォルト + CLI の例が更新されました。オペレーター ランブック دستیاب ہو۔
- テレメトリ ステージング ダッシュボード容量の飽和、PoR 障害、アラート、構成

## ドキュメントと運用成果物

- [ノード ストレージ参照](node-storage.md) 構成のデフォルト、CLI の使用法、トラブルシューティングの手順、更新プログラム
- [ノード操作 Runbook](node-operations.md) 実装、調整、SF-3 進化、調整
- `/sorafs/*` エンドポイント API リファレンス 開発者ポータル 公開 Torii ハンドラー OpenAPI マニフェスト ワイヤー