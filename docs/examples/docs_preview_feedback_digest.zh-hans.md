---
lang: zh-hans
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-12-29T18:16:35.070397+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 文档门户预览反馈摘要（模板）

在总结治理、发布的预览波时使用此模板
评论，或 `status.md`。将 Markdown 复制到跟踪单中，替换
包含真实数据的占位符，并附加通过导出的 JSON 摘要
`npm run --prefix docs/portal preview:log -- --summary --summary-json`。的
`preview:digest` 帮助程序 (`npm run --prefix docs/portal preview:digest -- --wave <label>`)
生成如下所示的指标部分，因此您只需填写
突出显示/操作/工件行。

```markdown
## Wave preview-<tag> feedback digest (YYYY-MM-DD)
- Invite window: <start → end>
- Reviewers invited: <count> (open: <count>)
- Feedback submissions: <count>
- Issues opened: <count>
- Latest event timestamp: <ISO8601 from summary.json>

| Category | Details | Owner / Follow-up |
| --- | --- | --- |
| Highlights | <e.g., “ISO builder walkthrough landed well”> | <owner + due date> |
| Blocking findings | <list issue IDs or tracker links> | <owner> |
| Minor polish items | <group cosmetic or copy edits> | <owner> |
| Telemetry anomalies | <link to dashboard snapshot / probe log> | <owner> |

## Actions
1. <Action item + link + ETA>
2. <Optional second action>

## Artefacts
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Wave summary: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Dashboard snapshot: `<link or path>`

```

将每个摘要与邀请跟踪票一起保存，以便审阅者和治理人员可以
无需挖掘 CI 日志即可重放证据线索。