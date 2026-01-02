---
lang: ja
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-11-15T09:17:21.371048+00:00"
translation_last_reviewed: 2026-01-01
---

# SNS トレーニングスライド テンプレート

この Markdown アウトラインは、ファシリテーターが言語別コホート向けに調整すべきスライドを反映している。これらのセクションを Keynote/PowerPoint/Google Slides にコピーし、箇条書き、スクリーンショット、図を必要に応じてローカライズする。

## タイトルスライド
- プログラム: "Sora Name Service onboarding"
- サブタイトル: サフィックス + サイクルを指定 (例: `.sora - 2026-03`)
- 登壇者 + 所属

## KPI オリエンテーション
- `docs/portal/docs/sns/kpi-dashboard.md` のスクリーンショットまたは埋め込み
- サフィックスフィルタ、ARPU テーブル、freeze トラッカーの説明リスト
- PDF/CSV エクスポートの案内

## Manifest ライフサイクル
- 図: registrar -> Torii -> governance -> DNS/gateway
- `docs/source/sns/registry_schema.md` を参照する手順
- 注釈付き manifest 抜粋例

## 争議と freeze のドリル
- guardian 介入のフローチャート
- `docs/source/sns/governance_playbook.md` を参照するチェックリスト
- freeze チケットのタイムライン例

## Annex 収集
- `cargo xtask sns-annex ... --portal-entry ...` のコマンド抜粋
- Grafana JSON を `artifacts/sns/regulatory/<suffix>/<cycle>/` にアーカイブすること
- `docs/source/sns/reports/.<suffix>/<cycle>.md` へのリンク

## 次のステップ
- トレーニングのフィードバックリンク ( `docs/examples/sns_training_eval_template.md` を参照)
- Slack/Matrix チャンネルの連絡先
- 次のマイルストーン日程
