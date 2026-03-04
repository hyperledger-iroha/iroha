---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76a4303fa2657476a3f983f1aa5597c9ddb478f670d233b0a7cf4e3791419a72
source_last_modified: "2025-11-20T12:45:46.606949+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: preview-feedback-w3-summary
title: W3 ベータフィードバックとステータス
sidebar_label: W3 サマリー
description: 2026 ベータプレビュー波 (ファイナンス、オブザーバビリティ、SDK、エコシステム) のライブダイジェスト。
---

| 項目 | 詳細 |
| --- | --- |
| 波 | W3 - ベータコホート (finance + ops + SDK パートナー + ecosystem advocate) |
| 招待期間 | 2026-02-18 -> 2026-02-28 |
| アーティファクトタグ | `preview-20260218` |
| トラッカーIssue | `DOCS-SORA-Preview-W3` |
| 参加者 | finance-beta-01, observability-ops-02, partner-sdk-03, ecosystem-advocate-04 |

## ハイライト

1. **エンドツーエンド証跡パイプライン。** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` は波ごとのサマリー (`artifacts/docs_portal_preview/preview-20260218-summary.json`)、digest (`preview-20260218-digest.md`) を生成し、`docs/portal/src/data/previewFeedbackSummary.json` を更新するため、ガバナンスレビュアーは1つのコマンドに頼れる。
2. **テレメトリー + ガバナンスのカバレッジ。** 4名のレビュアー全員が checksum 制御のアクセスを確認し、フィードバックを提出し、期限通りにアクセスが解除された; digest はフィードバック課題 (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) と波中に取得した Grafana の実行ログを参照。
3. **ポータル掲載。** 更新されたポータル表はレイテンシ/レスポンス率メトリクス付きで W3 をクローズ済みとして表示し、下段の新しいログページは JSON 生ログを取得しない監査者向けにタイムラインを反映。

## アクションアイテム

| ID | 内容 | 担当 | ステータス |
| --- | --- | --- | --- |
| W3-A1 | preview digest を収集してトラッカーに添付。 | Docs/DevRel lead | ✅ 完了 (2026-02-28)。 |
| W3-A2 | 招待/ダイジェスト証跡をポータル + roadmap/status に反映。 | Docs/DevRel lead | ✅ 完了 (2026-02-28)。 |

## 終了サマリー (2026-02-28)

- 2026-02-18 に招待送信、数分後に acknowledgements を記録; 最終テレメトリー確認後の 2026-02-28 に preview アクセスを解除。
- Digest + サマリーは `artifacts/docs_portal_preview/` に保存し、再現性のため生ログ `artifacts/docs_portal_preview/feedback_log.json` をアンカー。
- フォローアップ課題は `docs-preview/20260218` とガバナンストラッカー `DOCS-SORA-Preview-20260218` に起票; CSP/Try it のメモは observability/finance 担当へ送付し digest からリンク。
- トラッカー行は 🈴 Completed に更新され、ポータルのフィードバック表が波の完了を反映して、DOCS-SORA のベータ準備タスクを完了。
