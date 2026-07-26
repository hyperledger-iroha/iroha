---
lang: mn
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-12-29T18:16:35.070397+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docs Portal Урьдчилан үзэх санал хүсэлтийн тойм (Загвар)

Засаглалын урьдчилсан давалгааг нэгтгэн гаргахдаа энэ загварыг ашиглана уу
сэтгэгдэл, эсвэл `status.md`. Markdown-ыг хянах тасалбар руу хуулж, солино уу
Бодит өгөгдөл бүхий орлуулагч болон экспортолсон JSON хураангуйг хавсаргана уу
`npm run --prefix docs/portal preview:log -- --summary --summary-json`. The
`preview:digest` туслах (`npm run --prefix docs/portal preview:digest -- --wave <label>`)
доор үзүүлсэн хэмжүүрийн хэсгийг үүсгэдэг тул та зөвхөн хэсгийг бөглөхөд хангалттай
онцлох зүйлс/үйлдэл/олдворуудын эгнээ.

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

Тайжист бүрийг урилга хянах тасалбартай хамт хадгалаарай, ингэснээр тоймчид болон засаглал боломжтой болно
CI бүртгэлийг ухахгүйгээр нотлох баримтыг дахин тоглуул.