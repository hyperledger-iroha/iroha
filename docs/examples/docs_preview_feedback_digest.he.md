---
lang: he
direction: rtl
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-11-19T07:56:11.635822+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/docs_preview_feedback_digest.md -->

# דיג'סט פידבק לתצוגת preview של פורטל docs (תבנית)

השתמשו בתבנית זו בעת סיכום גל preview לצרכי governance, סקירות release או `status.md`.
העתיקו את ה-Markdown לתוך כרטיס המעקב, החליפו placeholders בנתונים אמיתיים, וצרפו
את סיכום ה-JSON שיוצא באמצעות `npm run --prefix docs/portal preview:log -- --summary --summary-json`.
העוזר `preview:digest` (`npm run --prefix docs/portal preview:digest -- --wave <label>`) מייצר
את סעיף המדדים למטה כך שתצטרכו למלא רק את שורות highlights/actions/artefacts.

```markdown
## דיג'סט פידבק לגל preview-<tag> (YYYY-MM-DD)
- חלון הזמנות: <start -> end>
- מספר מוזמנים: <count> (open: <count>)
- שליחות פידבק: <count>
- Issues שנפתחו: <count>
- timestamp אחרון של אירוע: <ISO8601 from summary.json>

| קטגוריה | פרטים | Owner / Follow-up |
| --- | --- | --- |
| Highlights | <דוגמה: "ISO builder walkthrough landed well"> | <owner + תאריך יעד> |
| ממצאים חוסמים | <רשימת issue IDs או קישורי tracker> | <owner> |
| פוליש מינורי | <לקבץ עריכות קוסמטיות או copy> | <owner> |
| חריגות טלמטריה | <קישור ל-snapshot של dashboard / log probe> | <owner> |

## Actions
1. <פעולה + קישור + ETA>
2. <פעולה שניה אופציונלית>

## Artefacts
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Wave summary: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Dashboard snapshot: `<link or path>`

```

שמרו כל דיג'סט עם כרטיס המעקב של ההזמנה כדי שמבקרים ו-governance יוכלו לשחזר את
שרשרת הראיות בלי לחפש בלוגי CI.

</div>
