---
lang: uz
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-12-29T18:16:35.070397+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Hujjatlar portalini oldindan ko'rish uchun fikr-mulohazalar to'plami (shablon)

Boshqarish, chiqarish uchun oldindan ko'rish to'lqinini umumlashtirganda ushbu shablondan foydalaning
sharhlar yoki `status.md`. Markdownni kuzatuv chiptasiga nusxa ko'chiring, almashtiring
haqiqiy ma'lumotlarga ega bo'lgan joy egalari va orqali eksport qilingan JSON xulosasini ilova qiling
`npm run --prefix docs/portal preview:log -- --summary --summary-json`. The
`preview:digest` yordamchi (`npm run --prefix docs/portal preview:digest -- --wave <label>`)
quyida ko'rsatilgan ko'rsatkichlar bo'limini yaratadi, shuning uchun siz faqat to'ldirishingiz kerak
diqqatga sazovor joylar/harakatlar/artefaktlar qatorlari.

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

Har bir dayjestni taklif-kuzatuv chiptasi bilan saqlang, shunda sharhlovchilar va boshqaruvchi
CI jurnallarini qazmasdan dalil izini takrorlang.