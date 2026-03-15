<!-- Japanese translation of docs/source/zk/prover_runbook.md -->

---
lang: ja
direction: ltr
source: docs/source/zk/prover_runbook.md
status: complete
translator: manual
---

# Torii ZK 添付ファイルとプローバー運用手順書

本ランブックは、Torii の添付ファイルサービスとバックグラウンド ZK プローバーワーカーを監視・アラート・トリアージするための手引きです。Prometheus メトリクスを公開するテレメトリプロファイル（`telemetry_profile = "extended"` または `"full"`）が有効で、ダッシュボードが `/metrics` を取得していることを前提とします。

## コンポーネント
- **添付 API** — `POST /v1/zk/attachments` で証明やトランスクリプトなどのペイロードを保存。ワーカー有効時に順次スキャン。`iroha_cli app zk attachments *` で操作可能。
- **バックグラウンドプローバー** — `torii.zk_prover_enabled=true` で有効化。添付キューを処理し `ProofAttachment` を検証、JSON レポート（`/v1/zk/prover/reports`）を生成。リソース上限は `torii.zk_prover_max_inflight`, `torii.zk_prover_max_scan_bytes`, `torii.zk_prover_max_scan_millis`。スコープは `torii.zk_prover_allowed_backends`/`torii.zk_prover_allowed_circuits` で制御。インライン VK がない場合は `torii.zk_prover_keys_dir` から `<backend>__<name>.vk` を読み込む。
- **テレメトリ** — `crates/iroha_telemetry::metrics` で登録され `/metrics` に露出。

## クイックチェックリスト
| シナリオ | 対応 |
| --- | --- |
| `torii_zk_prover_budget_exhausted_total` が増加 | 理由（`bytes`／`time`）を確認。設定で上限を引き上げるか添付数を抑制。 |
| `torii_zk_prover_pending` が 10 分以上 0 を超過 | キュー滞留：スキャンメトリクスを確認し、inflight 上限や添付削減を検討。 |
| 添付ファイルがスキャンされない | ワーカー設定を確認、`zk_prover` ログのエラーと予算設定を要チェック。 |
| CLI/SDK アップロード失敗 | Torii ログの HTTP エラーとディスク容量を確認。 |
| レポートが欠落 | TTL (`torii.zk_prover_reports_ttl_secs`) と GC ログを確認。 |

## メトリクスと PromQL
| メトリクス | 説明 | PromQL／アラート例 |
| --- | --- | --- |
| `torii_zk_prover_inflight` | 処理中の添付数 | `torii_zk_prover_inflight` が上限張り付きで >10 分ならアラート |
| `torii_zk_prover_pending` | 待機キュー長 | `avg_over_time(torii_zk_prover_pending[5m]) > 0` で滞留検知 |
| `torii_zk_prover_last_scan_bytes` / `..._ms` | 直近スキャンのサイズ／時間 | `increase` を観測しリグレッション兆候を掴む |
| `torii_zk_prover_budget_exhausted_total{reason}` | 予算超過回数 | `rate(...[5m]) > 0` で調査開始 |
| `torii_zk_prover_attachment_bytes_bucket` | 添付サイズ分布（`content_type` ラベル） | 例: `histogram_quantile(0.95, sum(rate(...[5m])) by (le, content_type))` |
| `torii_zk_prover_latency_ms_bucket` | 添付ごとのワーカーレイテンシ | p95 が `torii.zk_prover_max_scan_millis` を超えたらアラート |
| `zk_verify_proof_bytes_bucket` / `_latency_ms_bucket` | 検証経路のメトリクス | Ledger 検証負荷とプローバー取込みの相関を確認 |
| `torii_zk_prover_gc_total` | TTL による削除件数 | 突然のスパイクは TTL 誤設定の可能性 |

## Grafana ダッシュボード例
`grafana_zk_prover.json` をインポートすると以下のパネルが利用可能:
1. キュー深さ (`pending` と `inflight`)。
2. 添付スループット（サイズのヒストグラム）。
3. ワーカーレイテンシ（設定予算との比較）。
4. 予算超過率（`increase(...[15m])`）。
5. 検証レイテンシオーバーレイ（`zk_verify_latency_ms` との比較）。

インポート後、データソース UID を環境に合わせて調整。

## ログと可観測性
- 添付 API は `torii::zk_attachments` ターゲットで構造化ログを出力。失敗は明示的な HTTP ステータスが Torii ログに記録。
- バックグラウンドプローバーは `torii::zk_prover`。各スキャンで `scheduled`, `processed_bytes`, `elapsed_ms`, `budget` を出力。
- 詳細調査時は一時的に `RUST_LOG=torii::zk_prover=debug` を設定（本番では常用しない）。
- テレメトリワーカーは予算超過やキューリセットを `torii::zk_prover::metrics` で記録。

## インシデント対応例

### 1. 予算枯渇が頻発
1. `torii_zk_prover_budget_exhausted_total{reason}` を確認。
2. ログで `bytes`／`time` どちらが原因か調査。
3. 設定値（`max_scan_bytes`, `max_scan_millis`, `max_inflight`）を確認。
4. クライアントが大きな添付を送っていないかチェック。必要に応じ上限を引き上げるか upstream で分割。
5. 調整後は `torii_zk_prover_pending` を監視し滞留が解消されるか確認。

### 2. 添付バックログの増大
1. `pending` と `inflight` を観測。
2. `last_scan_ms` が上限近い場合、`max_scan_millis` に阻まれている可能性。
3. ワーカー稼働状況（`scan_completed` ログ）を確認。
4. `max_inflight` を増やす、またはスキャン間隔を短縮。
5. 古い添付が不要なら API で削除。

### 3. レポート欠落／遅延
1. `torii.zk_prover_reports_ttl_secs` を確認。
2. `torii_zk_prover_gc_total` の増加有無を確認。
3. クライアントが TTL 以内に `/reports` を取得しているか確認。
4. 必要なら添付を手動取得しオフライン処理。

## CLI／SDK との連携
- `iroha_cli app zk attachments` は Torii エンドポイントをラップ。ノードと同じコミットで構築し DTO を同期する。
- Swift/Python SDK の `ToriiClient` は `upload_attachment`, `list_attachment`, `get_attachment`, `delete_attachment` を提供。ランブックのメトリクスで SDK 経路を検証。

## 変更管理
- 設定変更は ops ノートへ記録（日時・担当者・旧値・新値・理由）。
- 予算やスキャン周期を変更したら Grafana とアラート閾値も更新。
- 新ビルドを適用する前に `cargo test -p iroha_torii zk_prover` を実行しユニットテスト回帰を確認。

## 参考資料
- [ZK App API](../zk_app_api.md)
- [ZK ライフサイクル](lifecycle.md)
- [Torii CLI スモークテスト](../../../crates/iroha_cli/tests/cli_smoke.rs)
- [テレメトリ概要](../telemetry.md)
- [ロードマップ Milestone 4](../../../roadmap.md)
