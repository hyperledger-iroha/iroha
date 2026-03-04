---
lang: ja
direction: ltr
source: docs/source/sorafs_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 43dfc53b7a951ec0b036a56941335a4fd05657da91c7a3f727e3846b8c647d23
source_last_modified: "2025-11-08T02:24:51.340739+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/sorafs_chaos_plan.md -->

# SoraFS カオスドリル & インシデント対応プレイブック計画（ドラフト）

## カオスシナリオ（初期）

1. **ゲートウェイ停止**
   - リージョン単位のゲートウェイ障害をシミュレーションする。
   - 期待される反応: オーケストレーターが迂回し、アラートが発火し、インシデントを宣言。
2. **証明失敗の急増**
   - ゲートウェイ側で破損した証明を注入する。
   - 期待される反応: 証明失敗のアラート、自動隔離、インシデント対応。
3. **レプリケーション遅延**
   - チャンク応答を遅らせ、遅いプロバイダを再現する。
   - オーケストレーターの再試行、容量利用率アラームを監視。
4. **アドミッション不一致**
   - アドミッションエンベロープを削除／失効させ、拒否テレメトリの発火を確認する。

## プレイブックの構成要素

- **インシデント・チェックリスト**（誰に通知するか、コミュニケーション経路）。
- **シナリオ別ランブック**（封じ込め／復旧手順）。
- **ポストモーテム・テンプレート**（SLO 影響、是正措置）。
- **ランログ**（タイムライン、メトリクススナップショット、意思決定ポイント）。

## ドリル頻度 & トラッキング

| 頻度 | シナリオ束 | 参加者 | メモ |
|------|------------|--------|------|
| 四半期 | ゲートウェイ停止 + オーケストレーター再試行ストーム | Storage SRE、Observability TL、ガバナンス連絡役 | まずステージングで実施し、ステージング成功後に本番リハーサルを許可。 |
| 半年 | 証明失敗急増（PoR + PoTR） | Storage SRE、Crypto WG、Tooling | 証明リプレイツールとガバナンス通知を検証。 |
| 毎月 | レプリケーション遅延のスポットチェック | Storage SRE ローテーション | ステージングのマニフェストを使い、バックログ回復時間を記録。 |

各演習の後、`scripts/telemetry/log_sorafs_drill.sh` を使って結果を `ops/drill-log.md` に追記し、
ポストモーテムテンプレートにフォローアップのアクションリストを作成する。レビュー時には
`scripts/telemetry/validate_drill_log.sh` を実行し、ログが正しく整形されていることを確認する。
予定ドリルは `--status scheduled`、リハーサル完了後は `pass`/`fail`、追加アクションが残る場合は
`follow-up` を使う。ヘルパは `--log <path>` も受け付けるため、ドライランや自動テストで
`ops/drill-log.md` を触らずに実行できる。

TLS/ECH のドリルでは `cargo xtask sorafs-gateway-probe` を `--drill-*`、`--summary-json`、
`--pagerduty-*` フラグ付きで実行し、ゲートウェイプローブが実行記録を自動保存し、
`artifacts/sorafs_gateway_probe/` に JSON 証跡を出力し、（必要に応じて）ヘッダや GAR ポリシーの
ドリフトを検知した時点で PagerDuty の訓練インシデントを起票する。
テレメトリヘルパ `scripts/telemetry/run_sorafs_gateway_probe.sh` はこれらのフラグを
そのまま転送するため、既存ランブックは追加のシェル処理なしで
ネイティブのロギング／ページングフックを利用できる。

## ツール

- 失敗注入のためのハーネスコントローラ（モックプロバイダ計画より）。
- ドリルで利用する観測ダッシュボード（メトリクス計画）。
- 応答時間と完全性を評価する自動スコアリング。

## コミュニケーション・ワークフロー

- **インシデントチャネル**
  - 主系: `#sorafs-incident` Slack（`inc-bot` がインシデントごとに自動作成）。
  - エスカレーション: PagerDuty サービス “SoraFS On-Call”。重大アラートは L1 を自動呼び出し、
    5 分以内に ACK がない場合は L2 リードへ昇格。
  - ステータス更新: `#sorafs-status` Slack。テンプレートはボットが生成。
- **役割**
  - Incident Commander（IC）: PagerDuty を ACK する当番エンジニア。
  - Communications Lead（CL）: 社内外更新を担当する PM/DevRel。
  - Scribe: インシデントごとに交代し、`runbooks/incident_log.md` にタイムラインを記録。
- **チェックリスト（インシデントごと）**
  1. PagerDuty アラート発火 → IC が 5 分以内に ACK。
  2. IC がインシデント用 Slack チャンネルを作成（`/incident create`）。
  3. CL が 10 分以内に `#sorafs-status` へ初報を投稿。
  4. Scribe が Notion にタイムライン文書（`Incident YYYY-MM-DD`）を開始。週次でリポジトリに同期。
  5. 30 分ごとに IC が更新（状況／対応／次のステップ）を投稿。解決まで継続。
  6. 解決後、3 営業日以内にポストモーテム会議を設定。
- **ドリル通知**
  - カオスドリルは事前告知し、混乱防止のためボットが “DRILL – No customer impact” を投稿。
  - PagerDuty は “training” モードを使い、意図的な場合を除き実当番を呼び出さない。

## ガバナンス報告との整合

- **GAR 連携**
  - 各カオスシナリオは GAR（Governance Accountability Report）のカテゴリに対応する。
    例: ゲートウェイ停止は GAR の可用性メトリクスに紐づく。
  - ドリル／インシデント中に IC は GAR のしきい値超過を記録し、超過時は
    `GarIncidentReportV1` Norito ペイロードを生成する:
    - `incident_id`, `metric`, `duration`, `impact`
  - レポートをガバナンス DAG に追記し、月次コンプライアンスレビューに備える。
- **透明性ダッシュボード**
  - ドリル結果は透明性計画に流し込み、メトリクス
    `sorafs_chaos_drill_duration_seconds` と `sorafs_gar_incidents_total` を出力する。
  - ガバナンスニュースレターでドリル要約を共有し、準備状況と未解決のアクションを示す。

## TLS/ECH インシデント手順の統合

- **参照計画** `sorafs_gateway_tls_automation.md`（SF-5b）。カオスドリルには TLS/ECH 失敗シナリオを含める:
  - 期限切れ証明書、TLS 自動化の誤発行、ECH 設定ドリフトをシミュレーション。
  - ランブック手順:
    1. `tls_cert_expiry` アラートまたはハンドシェイク失敗メトリクスで検知。
    2. 自動更新パイプライン（`scripts/tls/renew_all.sh`）を起動するか、手動で再発行する。
    3. `sorafs tls verify` CLI で検証する。
    4. ECH 設定のローテーションを記録し、DNS レコード更新を確認する。
- **連携**
  - セキュリティチームは TLS/ECH ドリルに参加する。
  - 結果はコンプライアンス計画に反映し、証明書管理遵守を示す。
  - ポストモーテムでは、自動化改善（例: 合成プローブ追加）がアクション項目として追跡されていることを確認する。

これらの追加により、カオスドリル計画のコミュニケーション、ガバナンス、TLS/ECH 統合の作業が完了する。
