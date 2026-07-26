---
lang: ja
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-11-15T09:17:29.843566+00:00"
translation_last_reviewed: 2026-01-01
---

# SNS トレーニングワークブック テンプレート

このワークブックは各トレーニングコホートの公式配布資料として使用する。配布前にプレースホルダー (`<...>`) を置き換えること。

## セッション詳細
- サフィックス: `<.sora | .nexus | .dao>`
- サイクル: `<YYYY-MM>`
- 言語: `<ar/es/fr/ja/pt/ru/ur>`
- ファシリテーター: `<name>`

## Lab 1 - KPI エクスポート
1. ポータルの KPI ダッシュボードを開く (`docs/portal/docs/sns/kpi-dashboard.md`)。
2. サフィックス `<suffix>` と期間 `<window>` でフィルタする。
3. PDF + CSV のスナップショットをエクスポートする。
4. エクスポートした JSON/PDF の SHA-256 をここに記録: `______________________`.

## Lab 2 - manifest drill
1. サンプル manifest を `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json` から取得する。
2. `cargo run --bin sns_manifest_check -- --input <file>` で検証する。
3. `scripts/sns_zonefile_skeleton.py` で resolver の skeleton を生成する。
4. diff の要約を貼り付ける:
   ```
   <git diff output>
   ```

## Lab 3 - 争議シミュレーション
1. guardian CLI で freeze を開始する (case id `<case-id>`)。
2. 争議ハッシュを記録する: `______________________`.
3. 証拠ログを `artifacts/sns/training/<suffix>/<cycle>/logs/` にアップロードする。

## Lab 4 - annex 自動化
1. Grafana ダッシュボードの JSON をエクスポートし、`artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json` にコピーする。
2. 実行:
   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. annex のパス + SHA-256 出力を貼り付ける: `________________________________`.

## フィードバックメモ
- どこが不明確だったか?
- どの Lab が時間超過だったか?
- ツールの不具合はあったか?

完了したワークブックはファシリテーターへ返送し、
`artifacts/sns/training/<suffix>/<cycle>/workbooks/` に保管する。
