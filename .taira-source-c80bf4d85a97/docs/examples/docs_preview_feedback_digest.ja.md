---
lang: ja
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-11-19T07:56:11.635822+00:00"
translation_last_reviewed: 2026-01-01
---

# Docs ポータル プレビュー フィードバックダイジェスト (テンプレート)

ガバナンス、release review、`status.md` 向けにプレビュー wave を要約する際に
このテンプレートを使う。Markdown を追跡チケットに貼り付け、プレースホルダーを実データで
置き換え、`npm run --prefix docs/portal preview:log -- --summary --summary-json` で
出力した JSON サマリを添付する。`preview:digest` ヘルパー
(`npm run --prefix docs/portal preview:digest -- --wave <label>`) が下記のメトリクス
セクションを生成するため、highlights/actions/artefacts の行だけ埋めればよい。

```markdown
## Wave preview-<tag> フィードバックダイジェスト (YYYY-MM-DD)
- 招待ウィンドウ: <start -> end>
- 招待レビュアー数: <count> (open: <count>)
- フィードバック提出数: <count>
- 起票された issues: <count>
- 最新イベント timestamp: <ISO8601 from summary.json>

| カテゴリ | 詳細 | Owner / Follow-up |
| --- | --- | --- |
| Highlights | <例: "ISO builder walkthrough landed well"> | <owner + 期限> |
| ブロッカー | <issue IDs の一覧または tracker リンク> | <owner> |
| 軽微な polish | <cosmetic or copy edits をまとめる> | <owner> |
| テレメトリアノマリ | <dashboard snapshot / probe log のリンク> | <owner> |

## Actions
1. <アクション + リンク + ETA>
2. <任意の2つ目>

## Artefacts
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Wave summary: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Dashboard snapshot: `<link or path>`

```

各ダイジェストは招待トラッキングチケットと一緒に保管し、レビュー担当と governance が
CI ログを掘らずに証跡を再現できるようにする。
