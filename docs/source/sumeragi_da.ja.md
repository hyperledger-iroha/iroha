<!-- Japanese translation of docs/source/sumeragi_da.md -->

---
lang: ja
direction: ltr
source: docs/source/sumeragi_da.md
status: needs-update
translator: manual
---

# Sumeragi データ可用性 & RBC シナリオ

統合テスト [`sumeragi_rbc_da_large_payload_four_peers`] と
[`sumeragi_rbc_da_large_payload_six_peers`]（`integration_tests/tests/sumeragi_da.rs`）は、`sumeragi.da.enabled = true`（DA + RBC）とした 4 ノードおよび 6 ノード構成を起動します。各実行は統合ハーネス既定の `LARGE_PAYLOAD_BYTES = 1024` を使い、RBC の配送とコミットを観測し、本来の READY クォーラム（4 ピアは 3 票以上、6 ピアは 4 票以上）を確認し、ダッシュボードや回帰ツールで取り込める構造化サマリを出力します。

RBC ペイロードをライトクライアントがサンプリングする手順については [`light_client_da.md`](light_client_da.md) を参照してください。認証付き `/v1/sumeragi/rbc/sample` エンドポイントと関連するレート制限・予算を解説しています。

### DA タイムアウトと可用性追跡

`sumeragi.da.enabled=true` の場合、コミットパイプラインはローカル payload 可用性（`BlockCreated` または RBC 配送）を DA gate に記録します。`availability evidence` や RBC `READY` クォーラムは監査・テレメトリ・決定論的リカバリのために追跡されますが、別個のコミットクォーラムではありません。`missing_local_data` が有効な間、ローカル finalize は待機し、BlockCreated または RBC/ブロック同期による回復で payload がローカルに揃うと続行します。

可用性タイムアウトは block/commit 時間と DA タイムアウト調整値から導出され、ログと再ブロードキャストの判定にのみ使われます:
- `sumeragi.advanced.da.quorum_timeout_multiplier` は DA 有効時の `block_time + 3 * commit_time` をスケールします（既定 `3`）。
- `sumeragi.advanced.da.availability_timeout_multiplier` は DA モードの可用性タイムアウトをスケールします（既定 `2`）。
- `sumeragi.advanced.da.availability_timeout_floor_ms` は可用性タイムアウトの最小値を強制します（既定 `2000`、`0` で無効）。
値はバリデータ間で揃えてください。

可用性追跡に起因する RBC の自動 resend/abort は、配信と投票の循環待ちを避けるため v1 で廃止されました。`availability evidence` または RBC `READY` クォーラムを観測したがペイロードが不足しているノードは、commit certificate 署名者を優先して決定論的に欠落ブロック取得を行い、失敗時はコミットトポロジ全体にフォールバックします。

## 収集するメトリクス

- RBC がペイロードを DELIVER とマークした時点のペイロードサイズ（バイト）と派生スループット（MiB/s）
- `/v1/sumeragi/rbc/sessions` から取得した RBC セッションスナップショット（`total_chunks`, `received_chunks`, `ready_count`, `view`, `block_hash`, `recovered`, `lane_backlog`, `dataspace_backlog`）
- 各ピアの Prometheus カウンタ `/metrics` から取得: `sumeragi_rbc_payload_bytes_delivered_total`, `sumeragi_rbc_deliver_broadcasts_total`, `sumeragi_rbc_ready_broadcasts_total`
- `/metrics` から収集できるレーン／データスペース単位のバックログゲージ:
  `sumeragi_rbc_lane_{tx_count,total_chunks,pending_chunks,bytes_total}`（`lane_id` ラベル）と
  `sumeragi_rbc_dataspace_{tx_count,total_chunks,pending_chunks,bytes_total}`（`lane_id`/`dataspace_id` ラベル）

## シナリオの実行方法

補助スクリプトが各ピアの `/metrics` を参照できるよう、テレメトリを有効化してください。実行前に `scripts/check_norito_bindings_sync.sh`（または `python3 scripts/check_norito_bindings_sync.py`）を実行し、Norito バインディングが同期しているか確認します。未同期の場合、ビルドスクリプトは再生成されるまで中断します。

```bash
cargo test -p integration_tests \
  sumeragi_rbc_da_large_payload_four_peers -- --nocapture

cargo test -p integration_tests \
  sumeragi_rbc_da_large_payload_six_peers -- --nocapture
```

各実行は `sumeragi_da_summary::<scenario>::{...}` で始まる行を出力し、オートメーションが JSON ペイロードを収集できるようにします。`SUMERAGI_DA_ARTIFACT_DIR=/path/to/dir` を設定すると、出力サマリと各ピアの生 Prometheus スナップショットを保存します。`scripts/run_sumeragi_da.py` は夜間ジョブ向けに自動的に環境変数を有効化し、収集済みアーティファクトを `cargo run -p build-support --bin sumeragi_da_report` へ渡して `sumeragi-da-report.md` を生成します。定期タスク `.github/workflows/sumeragi-da-nightly.yml` はサマリ・メトリクス・Markdown レポートを含むディレクトリ全体を GitHub Actions にアップロードし、オペレーターが結果を直接確認できます。

これらのシナリオでは `sumeragi.debug.rbc.force_deliver_quorum_one = false` のまま、プロトコル本来の READY クォーラムを検証します。デバッグ用ノブは個別診断向けに限定し、通常の DA/RBC 検証では無効のままにして、配送・スループット予算が本番クォーラムの挙動を測るようにしてください。

## 想定ベースライン

統合ハーネスの `LARGE_PAYLOAD_BYTES = 1024` とプロトコル READY クォーラムを前提に、既定の開発向けスモーク実行では次の不変条件を確認します。より大きなペイロードは、この既定チェックではなく soak/performance 作業として扱います。

注記: `sumeragi.advanced.rbc.chunk_max_bytes` は起動時にクランプされ、`network.max_frame_bytes_block_sync` から暗号化オーバーヘッドを差し引いた平文上限に収まるよう調整されます。

| シナリオ | チャンク数 | READY 閾値 | ピアごとのカウンタ | タイミング予算 |
| --- | --- | --- | --- | --- |
| 4 ピア | plain は 1 チャンク、RS16 は 4 チャンク | READY 投票 ≥3（`f=1` の 2f+1） | `payload_bytes_delivered_total ≥ 1024`、`deliver_broadcasts_total ≥ 1`、`ready_broadcasts_total ≥ 1` または同等の永続 READY 証拠 | ハーネスの配送・コミット予算 |
| 6 ピア | plain は 1 チャンク、RS16 は 6 チャンク | READY 投票 ≥4（`f=2` の 2f+1） | 上と同じ | 上と同じ |

これらのスモーク実行は主にクォーラム、配送、キュー回帰を検証します。DELIVER 遅延がハーネス予算に近づく場合、構成済みペイロードの計算済みスループット下限を下回る場合、またはピア間でカウンタが乖離する場合（コレクタのスロットリングやチャンク欠落の兆候）はアラートを推奨します。

`cargo run -p build-support --bin sumeragi_da_report [ARTIFACT_DIR]` は `.summary.json` を読み込み、レイテンシ／スループット／スナップショットをまとめた Markdown レポートを生成します。引数でアーティファクトディレクトリを指定するか、`SUMERAGI_DA_ARTIFACT_DIR` を設定してください。2025-10-05 のフィクスチャから生成されたレポートでは、RBC DELIVER の中央値が 3.12〜3.34 秒、コミットが 4 秒枠内、実効スループットが 3.1 MiB/s 以上でした。

サンドボックス環境向けメモ: `scripts/run_sumeragi_da.py` はピア起動前に `IROHA_SKIP_BIND_CHECKS=1` をエクスポートし、`integration_tests/fixtures/sumeragi_da/default/` のフィクスチャを同梱しました。macOS seatbelt が bind の事前チェックで誤って権限エラーを出す場合、この環境変数で本番 bind のリトライを許容します。それでもループバックソケットが拒否される環境では、スクリプトがフィクスチャを再生してメトリクス表示を維持します。実機でネットワークを起動できる場合は `--disable-fixture-fallback` でフィクスチャ再生を無効化してください。

## パフォーマンス予算

大容量ペイロード向け統合テストには DA・RBC・SBV-AM ゲート有効時の明示的な性能予算が設定されており、構造化サマリと `sumeragi-da-report.md`（列 `BG queue max`, `P2P drops max` など）に反映されます。実行結果が以下の上限を超えた場合は即時アラートを推奨します。

| 指標 | 予算 | 適用テスト | アラート指針 |
| --- | --- | --- | --- |
| RBC DELIVER レイテンシ | 基本 30 秒 + 4 ピア超過分ごとに 60 秒 + RS16 プレミアム 40 秒 | `sumeragi_rbc_da_large_payload_*` | 通常スモーク実行が計算済み予算に近づいたら警告、コレクタ飽和を調査 |
| コミットレイテンシ | RBC 配送予算 + 40 秒 | 同上 | コミット遅延が計算済み予算に近づいたら警告、ペースメーカー期限・ビュー変更を確認 |
| 実効スループット | min(payload/配送予算, 0.1 MiB/s) 以上 | 同上 | 2 回連続で計算済み下限を下回ったら警告 |
| Sumeragi バックグラウンド POST キュー深さ | ≤ 32 タスク | 同上 | 24 以上で警告、バックログ増大を監視 |
| P2P キューのドロップ | = 0 | 同上 | 非ゼロを検出したら即時警告、容量設定を確認 |

ナイトリー CI は同じ JSON サマリを読み込み Markdown レポートを生成し、ダッシュボードで予算順守状況を追跡できるようにしています。

.. mdinclude:: generated/sumeragi_da_report.md

## 攻撃的シナリオ

`integration_tests/tests/sumeragi_adversarial.rs` スイートはカオステスト向けに追加された RBC デバッグノブを検証します。

- `sumeragi_adversarial_chunk_drop`: `sumeragi.debug.rbc.drop_every_nth_chunk = 2` を有効化し、リーダーが隔チャンクで欠落させた場合にコミットが停止することを確認。サマリ行は `sumeragi_adversarial::chunk_drop::{...}` として出力され、RBC セッション状態を含みます。
- `sumeragi_adversarial_chunk_reorder`: `sumeragi.debug.rbc.shuffle_chunks = true` を有効化し、チャンク順序を入れ替えても DELIVER とコミットに影響しないことを実証。
- `sumeragi_adversarial_witness_corruption`: `sumeragi.debug.rbc.corrupt_witness_ack = true` を有効化し、破損した ACK がコミット高さをブロックする一方で RBC セッションが完了することを確認。
- `sumeragi_adversarial_duplicate_inits`: `sumeragi.debug.rbc.duplicate_inits = true` を用い、次ビューで重複提案ペイロードが DELIVER され、オペレーターサマリに現れることを検証。
- `sumeragi_adversarial_chunk_drop_recovery`: 2 段階フロー。まず `drop_every_nth_chunk` を有効化してコレクタ停止を確認し、次にノブを無効化してネットワークを再起動し、誠実な振る舞いでコミットが再開することを確認。

すべてのシナリオは `SUMERAGI_ADVERSARIAL_ARTIFACT_DIR` を受け付け、前述の大容量ハーネスと同様に JSON サマリを保存します。
