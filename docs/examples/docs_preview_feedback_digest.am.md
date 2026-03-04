---
lang: am
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-12-29T18:16:35.070397+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የሰነዶች ፖርታል ቅድመ-ዕይታ ግብረ-መልስ (አብነት)

ለአስተዳደር ቅድመ እይታ ሞገድ ሲያጠቃልሉ ይህን አብነት ይጠቀሙ፣ ይልቀቁ
ግምገማዎች, ወይም `status.md`. ማርክዳውን ወደ መከታተያ ትኬቱ ይቅዱ፣ ይተኩ
ቦታ ያዢዎች በእውነተኛ ውሂብ፣ እና በ በኩል ወደ ውጭ የተላከውን የJSON ማጠቃለያ ያያይዙ
I18NI0000002X. የ
`preview:digest` አጋዥ (I18NI0000004X)
ከታች የሚታየውን የመለኪያ ክፍል ያመነጫል ስለዚህ መሙላት ብቻ ያስፈልግዎታል
ድምቀቶች / ድርጊቶች / artefacts ረድፎች.

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

ገምጋሚዎች እና አስተዳደር እንዲችሉ እያንዳንዱን የምግብ አሰራር በግብዣ መከታተያ ትኬት ያቆዩት።
በ CI ምዝግብ ማስታወሻዎች ውስጥ ሳይቆፍሩ የማስረጃውን መንገድ እንደገና ያጫውቱ።