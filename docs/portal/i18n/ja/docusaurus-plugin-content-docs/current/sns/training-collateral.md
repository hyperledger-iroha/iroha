---
lang: ja
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: training-collateral
title: SNS トレーニング資料
description: SN-8 で求められるカリキュラム、ローカライズの流れ、付属資料の証跡取得。
---

> `docs/source/sns/training_collateral.md` を反映しています。各サフィックスのローンチ前に、レジストラ、DNS、guardian、ファイナンスチームをブリーフィングする際に使用します。

## 1. カリキュラム概要

| トラック | 目的 | 事前資料 |
|-------|------------|-----------|
| レジストラ運用 | マニフェストの提出、KPI ダッシュボード監視、エラーのエスカレーション。 | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS とゲートウェイ | リゾルバ skeleton の適用、フリーズ/ロールバックのリハーサル。 | `sorafs/gateway-dns-runbook`, direct-mode policy samples. |
| ガーディアンと評議会 | 紛争対応、ガバナンス補遺の更新、付録のログ化。 | `sns/governance-playbook`, steward scorecards. |
| 財務と分析 | ARPU/bulk 指標の取得、付録バンドルの公開。 | `finance/settlement-iso-mapping`, KPI dashboard JSON. |

### モジュールフロー

1. **M1 — KPI オリエンテーション (30分):** サフィックスフィルタ、エクスポート、フリーズカウンタを確認。成果物: SHA-256 ダイジェスト付き PDF/CSV スナップショット。
2. **M2 — マニフェストライフサイクル (45分):** レジストラのマニフェストを作成・検証し、`scripts/sns_zonefile_skeleton.py` でリゾルバ skeleton を生成。成果物: skeleton + GAR 証跡を示す git diff。
3. **M3 — 紛争ドリル (40分):** guardian のフリーズ + アピールをシミュレートし、CLI ログを `artifacts/sns/training/<suffix>/<cycle>/logs/` に保存。
4. **M4 — 付録取得 (25分):** ダッシュボード JSON をエクスポートし、次を実行:

   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   成果物: 更新された付録 Markdown + 規制メモ + ポータル用ブロック。

## 2. ローカライズフロー

- 言語: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, `ur`.
- 各翻訳はソースファイルの隣に配置します (`docs/source/sns/training_collateral.<lang>.md`)。更新後は `status` と `translation_last_reviewed` を更新してください。
- 言語別のアセットは `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (slides/, workbooks/, recordings/, logs/) 配下に置きます。
- 英語ソースを編集したら `python3 scripts/sync_docs_i18n.py --lang <code>` を実行して、新しいハッシュを翻訳者に知らせます。

### 配布チェックリスト

1. ローカライズ完了後、翻訳スタブを更新 (`status: complete`)。
2. スライドを PDF にエクスポートし、言語別の `slides/` にアップロード。
3. KPI walkthrough を10分以内で録画し、言語スタブからリンク。
4. `sns-training` タグのガバナンスチケットを作成し、スライド/ワークブックの digest、録画リンク、付録証跡を添付。

## 3. トレーニング資産

- スライドアウトライン: `docs/examples/sns_training_template.md`.
- ワークブックテンプレート: `docs/examples/sns_training_workbook.md` (参加者ごとに1部).
- 招待 + リマインダー: `docs/examples/sns_training_invite_email.md`.
- 評価フォーム: `docs/examples/sns_training_eval_template.md` (回答は `artifacts/sns/training/<suffix>/<cycle>/feedback/` に保存).

## 4. スケジュールとメトリクス

| サイクル | ウィンドウ | メトリクス | 備考 |
|-------|--------|---------|-------|
| 2026‑03 | KPI レビュー後 | 出席率 %, 付録 digest を記録 | `.sora` + `.nexus` cohorts |
| 2026‑06 | `.dao` GA 前 | 財務準備度 ≥90 % | ポリシー更新を含む |
| 2026‑09 | 拡張 | 紛争ドリル <20分、付録 SLA ≤2日 | SN-7 インセンティブに整合 |

匿名フィードバックを `docs/source/sns/reports/sns_training_feedback.md` に記録し、次のコホートがローカライズとラボを改善できるようにします。

