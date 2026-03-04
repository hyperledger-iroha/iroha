---
lang: ja
direction: ltr
source: docs/source/sns/reports/steward_scorecard_2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 31ff1c34929cbcf68fa08ef8041bf47589c1c742aea36bf3a2f5bf5e91f2fd9a
source_last_modified: "2026-01-03T18:07:57.614745+00:00"
translation_last_reviewed: 2026-01-22
title: SNS スチュワード スコアカード
summary: スチュワード監視用の自動 KPI レポート。
---

<!-- 日本語訳: docs/source/sns/reports/steward_scorecard_2026q1.md -->

# SNS スチュワード スコアカード — 2026-Q1

_生成日時 2026-04-05T18:45:00Z（バージョン 1）_

| サフィックス | スチュワード | 更新 | サポート SLA | 異議 | ローテーション |
|--------|---------|---------|-------------|---------|----------|
| .sora | Sora Foundation Steward Desk | 合格 (91%) | 合格 (98%) | 合格 (96h) | なし |
| .dao | DAO Launchpad Guild | 警告 (53%) | 違反 (81%) | 警告 (220h) | 交代 |

## .sora (Sora Foundation Steward Desk)

- 更新維持率: 91.2% (目標 55%) — 合格
- サポート SLA: 97.6% (420 チケット、P1 違反 1 件) — 合格
- 異議解決ターンアラウンド: 中央値 96 h (36 件解決) — 合格
- ローテーション姿勢: なし (未処理アクションなし)
  - メモ:
    - 期中に紹介リベートを開始。テレメトリは安定。

## .dao (DAO Launchpad Guild)

- 更新維持率: 53.3% (目標 55%) — 警告
- サポート SLA: 80.6% (310 チケット、P1 違反 6 件) — 違反
- 異議解決ターンアラウンド: 中央値 220 h (17 件解決) — 警告
- ローテーション姿勢: 交代 (単一 KPI 違反を記録; guardian freeze(s) 1 件を開始)
  - アラート:
    - 更新維持率 53.3% vs 目標 55%
    - サポート SLA 80.6% vs 目標 95% (P1 違反 6 件)
    - 異議解決ターンアラウンド 220 h vs 予算 168 h
    - 1 件の guardian freeze(s) が四半期中に有効
  - メモ:
    - Guardian freeze (2026-02-10) は是正待ち。人員増強を進行中。
