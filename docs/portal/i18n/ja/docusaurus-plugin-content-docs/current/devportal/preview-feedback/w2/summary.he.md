---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w2/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e01793585a67709236811c76392b5df2ac62f5371cefa9471c8d23da2cccc51f
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w2-summary
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| 項目 | 詳細 |
| --- | --- |
| 波 | W2 - コミュニティレビュアー |
| 招待期間 | 2025-06-15 -> 2025-06-29 |
| アーティファクトタグ | `preview-2025-06-15` |
| トラッカーIssue | `DOCS-SORA-Preview-W2` |
| 参加者 | comm-vol-01...comm-vol-08 |

## ハイライト

1. **ガバナンスとツール** - コミュニティ intake ポリシーは 2025-05-20 に全会一致で承認; 動機/タイムゾーン欄を追加した申請テンプレートは `docs/examples/docs_preview_request_template.md` に配置。
2. **プレフライト証跡** - Try it proxy 変更 `OPS-TRYIT-188` を 2025-06-09 に実行し、Grafana ダッシュボードを取得、`preview-2025-06-15` の descriptor/checksum/probe 出力を `artifacts/docs_preview/W2/` に保存。
3. **招待波** - コミュニティレビュアー 8 名を 2025-06-15 に招待し、acknowledgements をトラッカーの招待表に記録; 全員が閲覧前に checksum 検証を完了。
4. **フィードバック** - `docs-preview/w2 #1` (tooltip 文言) と `#2` (ローカライゼーションのサイドバー順) は 2025-06-18 に起票し、2025-06-21 までに解決 (Docs-core-04/05); 期間中のインシデントはなし。

## アクションアイテム

| ID | 内容 | 担当 | ステータス |
| --- | --- | --- | --- |
| W2-A1 | `docs-preview/w2 #1` (tooltip 文言) を対応。 | Docs-core-04 | ✅ 完了 (2025-06-21)。 |
| W2-A2 | `docs-preview/w2 #2` (ローカライゼーションのサイドバー) を対応。 | Docs-core-05 | ✅ 完了 (2025-06-21)。 |
| W2-A3 | 退出証跡をアーカイブし roadmap/status を更新。 | Docs/DevRel lead | ✅ 完了 (2025-06-29)。 |

## 終了サマリー (2025-06-29)

- コミュニティレビュアー8名全員が完了を確認し、preview アクセスは取り消し; acknowledgements はトラッカーの招待ログに記録。
- 最終テレメトリーのスナップショット (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) はグリーン維持; ログと Try it proxy transcript を `DOCS-SORA-Preview-W2` に添付。
- 証跡バンドル (descriptor, checksum log, probe output, link report, Grafana screenshots, invite acknowledgements) を `artifacts/docs_preview/W2/preview-2025-06-15/` に保存。
- トラッカーの W2 checkpoint ログを退出まで更新し、W3 計画開始前に roadmap が監査可能な記録を保持。
