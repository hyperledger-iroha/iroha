---
lang: az
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-12-29T18:16:35.070397+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sənəd Portalına Ön Baxış Rəy Həzmi (Şablon)

İdarəetmə, buraxılış üçün ilkin baxış dalğasını ümumiləşdirərkən bu şablondan istifadə edin
rəylər və ya `status.md`. Markdown-u izləmə biletinə köçürün, dəyişdirin
real data ilə yer tutucular və vasitəsilə ixrac edilən JSON xülasəsini əlavə edin
`npm run --prefix docs/portal preview:log -- --summary --summary-json`. The
`preview:digest` köməkçi (`npm run --prefix docs/portal preview:digest -- --wave <label>`)
aşağıda göstərilən ölçülər bölməsini yaradır, ona görə də yalnız doldurmağınız lazımdır
vurğulamalar/fəaliyyətlər/artefaktlar sıraları.

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

Rəyçilər və idarəçilik edə bilsin ki, hər bir həzmi dəvət izləmə bileti ilə saxlayın
CI qeydlərini qazmadan sübut izini təkrarlayın.