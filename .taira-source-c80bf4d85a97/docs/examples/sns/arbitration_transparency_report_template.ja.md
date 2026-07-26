---
lang: ja
direction: ltr
source: docs/examples/sns/arbitration_transparency_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 305a3f3b253a013825d4dd798d2282e111913ec777fe0fbf5b02a92c7172b92a
source_last_modified: "2025-11-15T07:34:14.070551+00:00"
translation_last_reviewed: 2026-01-01
---

# SNS 仲裁透明性レポート - <Month YYYY>

- **サフィックス:** `<.sora / .nexus / .dao>`
- **レポート期間:** `<ISO start>` -> `<ISO end>`
- **作成者:** `<Council liaison>`
- **ソース artefacts:** `cases.ndjson` SHA256 `<hash>`, ダッシュボード export `<filename>.json`

## 1. エグゼクティブサマリ

- 新規ケース合計: `<count>`
- 期間内にクローズしたケース: `<count>`
- SLA 遵守: `<ack %>` acknowledge / `<resolution %>` decision
- Guardian overrides 発行数: `<count>`
- 実施した移転/返金: `<count>`

## 2. ケース内訳

| 紛争タイプ | 新規ケース | クローズ | 解決中央値 (日) |
|------------|-----------|----------|----------------|
| 所有権 | 0 | 0 | 0 |
| ポリシー違反 | 0 | 0 | 0 |
| 乱用 | 0 | 0 | 0 |
| 請求 | 0 | 0 | 0 |
| その他 | 0 | 0 | 0 |

## 3. SLA パフォーマンス

| 優先度 | 受領 SLA | 達成 | 解決 SLA | 達成 | 逸脱 |
|--------|----------|------|----------|------|------|
| 緊急 | <= 2 h | 0% | <= 72 h | 0% | 0 |
| 高 | <= 8 h | 0% | <= 10 d | 0% | 0 |
| 標準 | <= 24 h | 0% | <= 21 d | 0% | 0 |
| Info | <= 3 d | 0% | <= 30 d | 0% | 0 |

逸脱があれば根本原因を記述し、修正チケットへリンクする。

## 4. ケース登録簿

| ケースID | セレクタ | 優先度 | ステータス | 結果 | ノート |
|---------|---------|--------|-----------|------|-------|
| SNS-YYYY-NNNNN | `label.suffix` | Standard | Closed | Upheld | `<summary>` |

匿名化された事実または公開投票リンクを参照する 1 行ノートを記載する。
必要に応じて封印し、適用した削除を明記する。

## 5. 対応と救済

- **凍結 / 解除:** `<counts + case ids>`
- **移転:** `<counts + assets moved>`
- **請求調整:** `<credits/debits>`
- **ポリシー follow-up:** `<tickets or RFCs opened>`

## 6. 申し立てと guardian overrides

guardian board にエスカレートされた申し立てを、timestamps と決定 (approve/deny) と共に要約する。
`sns governance appeal` 記録または council 投票へのリンクを付ける。

## 7. 未完了項目

- `<Action item>` - Owner `<name>`, ETA `<date>`
- `<Action item>` - Owner `<name>`, ETA `<date>`

このレポートで参照した NDJSON、Grafana の export、CLI ログを添付する。
