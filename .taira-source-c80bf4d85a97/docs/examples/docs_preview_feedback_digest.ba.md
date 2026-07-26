---
lang: ba
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-12-29T18:16:35.070397+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Докс порталы алдан ҡарау кире бәйләнеш дайджест (Ҡалып)

Был шаблон ҡулланыу ҡасан дөйөмләштереү өсөн алдан ҡарау тулҡыны идара итеү, релиз
тикшерелгән, йәки `status.md`. Күсермә Маркдаун күҙәтеү билет, алмаштырыу
ысын мәғлүмәттәр менән урын хужалары, һәм JSON йомғаҡлау аша беркетелгән экспорт аша
`npm run --prefix docs/portal preview:log -- --summary --summary-json`. 1990 й.
I18NI000000003X ярҙамсыһы (`npm run --prefix docs/portal preview:digest -- --wave <label>`)
генерациялай метрика бүлеге түбәндә күрһәтелгән, шулай итеп, һеҙгә тик тултырырға кәрәк .
айырыу/ғәмәлдәр/артафакттар рәттәре.

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

Һаҡлау һәр distest менән саҡырыу-күҙәтеү билет шулай рецензенттар һәм идара итеү мөмкин
реплей дәлилдәр эҙҙәре аша ҡаҙылмай CI журналдар.