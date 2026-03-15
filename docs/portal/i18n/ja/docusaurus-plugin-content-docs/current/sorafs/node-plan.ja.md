---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3b64cd60d892db1241f6091716407374578939f5d9feddeaae9e558ad654a208
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: node-plan
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note 正規ソース
このページは `docs/source/sorafs/sorafs_node_plan.md` を反映しています。レガシーの Sphinx ドキュメントが退役するまで両方を同期してください。
:::

SF-3 は、Iroha/Torii プロセスを SoraFS ストレージプロバイダに変える最初の実行可能な `sorafs-node` クレートを提供します。本計画は、[ノードストレージガイド](node-storage.md)、[プロバイダ入場ポリシー](provider-admission-policy.md)、[ストレージ容量マーケットプレイスのロードマップ](storage-capacity-marketplace.md) と併せて進行順を決めてください。

## 目標スコープ (マイルストーン M1)

1. **チャンクストア統合。** `sorafs_car::ChunkStore` を永続バックエンドで包み、チャンクバイト、manifest、PoR ツリーを設定済みデータディレクトリに保存します。
2. **ゲートウェイエンドポイント。** Torii プロセス内で、pin 提出、チャンク取得、PoR サンプリング、ストレージテレメトリの Norito HTTP エンドポイントを公開します。
3. **設定配線。** `SoraFsStorage` の config 構造体 (有効化フラグ、容量、ディレクトリ、並列制限) を追加し、`iroha_config`、`iroha_core`、`iroha_torii` に配線します。
4. **クォータ/スケジューリング。** オペレータ定義のディスク/並列制限を適用し、バックプレッシャ付きで要求をキューイングします。
5. **テレメトリ。** pin 成功、チャンク取得レイテンシ、容量利用率、PoR サンプリング結果のメトリクス/ログを発行します。

## 作業内訳

### A. クレート/モジュール構成

| タスク | 担当 | 備考 |
|------|------|------|
| `crates/sorafs_node` を `config`, `store`, `gateway`, `scheduler`, `telemetry` で構成する。 | Storage Team | Torii 統合向けに再利用可能な型を再公開する。 |
| `SoraFsStorage` から `StorageConfig` をマッピング (user → actual → defaults)。 | Storage Team / Config WG | Norito/`iroha_config` の層を決定的に保つ。 |
| Torii が pin/fetch を送るための `NodeHandle` ファサードを提供する。 | Storage Team | ストレージ内部と async 配線をカプセル化する。 |

### B. 永続チャンクストア

| タスク | 担当 | 備考 |
|------|------|------|
| `sorafs_car::ChunkStore` をラップするディスクバックエンドを構築し、オンディスクの manifest インデックス (`sled`/`sqlite`) を持たせる。 | Storage Team | 決定的レイアウト: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| `ChunkStore::sample_leaves` を使って PoR メタデータ (64 KiB/4 KiB ツリー) を維持する。 | Storage Team | 再起動後のリプレイをサポートし、破損時は即失敗。 |
| 起動時の整合性リプレイを実装 (manifest 再ハッシュ、未完了 pin の削除)。 | Storage Team | リプレイ完了まで Torii 起動をブロック。 |

### C. ゲートウェイエンドポイント

| エンドポイント | 挙動 | タスク |
|---------------|------|------|
| `POST /sorafs/pin` | `PinProposalV1` を受け入れ、manifest を検証し、取り込みをキューし、manifest CID を返す。 | チャンクプロファイル検証、クォータ適用、chunk store へのストリーミング。 |
| `GET /sorafs/chunks/{cid}` + range クエリ | `Content-Chunker` ヘッダ付きでチャンクバイトを返し、range 能力仕様を尊重。 | scheduler + ストリーム予算を使用 (SF-2d の range 能力と連動)。 |
| `POST /sorafs/por/sample` | manifest の PoR サンプリングを実行し、証明バンドルを返す。 | chunk store のサンプリングを再利用し、Norito JSON で応答。 |
| `GET /sorafs/telemetry` | 収容力、PoR 成功、fetch エラーカウントの概要。 | ダッシュボード/運用向けにデータ提供。 |

ランタイム配線は PoR のやり取りを `sorafs_node::por` 経由に通します。トラッカーが `PorChallengeV1`、`PorProofV1`、`AuditVerdictV1` を記録し、`CapacityMeter` のメトリクスが Torii 固有のロジックなしでガバナンス判定を反映します。【crates/sorafs_node/src/scheduler.rs#L147】

実装メモ:

- Torii の Axum スタックを `norito::json` ペイロードで使用する。
- レスポンス用の Norito スキーマ (`PinResultV1`, `FetchErrorV1`, テレメトリ構造体) を追加する。

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` はバックログ深度、最古のエポック/期限、プロバイダごとの最新成功/失敗タイムスタンプを公開するようになりました。`sorafs_node::NodeHandle::por_ingestion_status` によって提供され、Torii はダッシュボード向けに `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` のゲージを記録します。【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. スケジューラとクォータ適用

| タスク | 詳細 |
|------|------|
| ディスククォータ | ディスク上のバイト数を追跡し、`max_capacity_bytes` 超過時に新規 pin を拒否。将来のポリシー向けに削除フックを提供。 |
| fetch 並列性 | グローバルセマフォ (`max_parallel_fetches`) と、SF-2d range 上限由来のプロバイダ別予算。 |
| pin キュー | 取り込みジョブの待ち行列を制限し、キュー深度を Norito ステータスエンドポイントで公開。 |
| PoR 間隔 | `por_sample_interval_secs` で駆動されるバックグラウンドワーカー。 |

### E. テレメトリとログ

メトリクス (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (`result` ラベル付きヒストグラム)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

ログ/イベント:

- ガバナンス取り込み向けの構造化 Norito テレメトリ (`StorageTelemetryV1`)。
- 利用率が 90% 超、または PoR 失敗連続が閾値超えの場合に警告。

### F. テスト戦略

1. **ユニットテスト。** チャンクストア永続化、クォータ計算、スケジューラ不変条件 ( `crates/sorafs_node/src/scheduler.rs` を参照)。
2. **統合テスト** (`crates/sorafs_node/tests`)。Pin → fetch 往復、再起動復旧、クォータ拒否、PoR サンプリング証明の検証。
3. **Torii 統合テスト。** ストレージ有効で Torii を起動し、`assert_cmd` で HTTP エンドポイントを実行。
4. **カオスロードマップ。** 将来のドリルでディスク枯渇、遅い IO、プロバイダ削除をシミュレート。

## 依存関係

- SF-2b 入場ポリシー — ノードが広告前に admission envelope を検証すること。
- SF-2c 容量マーケットプレイス — テレメトリを容量宣言へ紐付ける。
- SF-2d advert 拡張 — range 能力 + ストリーム予算が利用可能になったら消費する。

## マイルストーン終了条件

- `cargo run -p sorafs_node --example pin_fetch` がローカル fixtures で動作。
- Torii が `--features sorafs-storage` でビルドされ、統合テストに合格。
- ドキュメント ([ノードストレージガイド](node-storage.md)) が設定デフォルト + CLI 例で更新され、運用 runbook が利用可能。
- ステージングダッシュボードでテレメトリが可視化され、容量飽和と PoR 失敗のアラートが構成済み。

## ドキュメント/運用の成果物

- [ノードストレージ参照](node-storage.md) を設定デフォルト、CLI 使用方法、トラブルシューティング手順で更新する。
- [ノード運用 runbook](node-operations.md) を SF-3 の進化に合わせて実装と整合させる。
- `/sorafs/*` エンドポイントの API 参照を開発者ポータルに公開し、Torii ハンドラが入ったら OpenAPI マニフェストへ接続する。
