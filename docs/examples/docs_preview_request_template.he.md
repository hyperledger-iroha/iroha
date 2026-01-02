---
lang: he
direction: rtl
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-11-10T20:01:03.610024+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/docs_preview_request_template.md -->

# בקשת גישה לתצוגה מקדימה לפורטל docs (תבנית)

השתמשו בתבנית זו בעת איסוף פרטי הסוקר לפני הענקת גישה לסביבת ה-preview הציבורית.
העתיקו את ה-Markdown ל-issue או לטופס בקשה והחליפו את הערכים הממלאים.

```markdown
## סיכום הבקשה
- מבקש: <שם מלא / ארגון>
- משתמש GitHub: <username>
- קשר מועדף: <email/Matrix/Signal>
- אזור ואזור זמן: <UTC offset>
- תאריכי התחלה / סיום מוצעים: <YYYY-MM-DD -> YYYY-MM-DD>
- סוג הסוקר: <Core maintainer | Partner | Community volunteer>

## רשימת תאימות
- [ ] חתם על מדיניות שימוש מקובל ל-preview (link).
- [ ] קרא את `docs/portal/docs/devportal/security-hardening.md`.
- [ ] קרא את `docs/portal/docs/devportal/incident-runbooks.md`.
- [ ] אישר איסוף טלמטריה וניתוח אנונימי (כן/לא).
- [ ] התבקש alias ל-SoraFS (כן/לא). שם ה-alias: `<docs-preview-???>`

## צרכי גישה
- כתובת/יות preview: <https://docs-preview.sora.link/...>
- הרשאות API נדרשות: <Torii read-only | Try it sandbox | none>
- הקשר נוסף (בדיקות SDK, מיקוד סקירת תיעוד וכו'):
  <פרטים כאן>

## אישור
- סוקר (maintainer): <שם + תאריך>
- כרטיס governance / בקשת שינוי: <קישור>
```

---

## שאלות ייעודיות לקהילה (W2+)
- מניע לגישה ל-preview (משפט אחד):
- מוקד סקירה עיקרי (SDK, governance, Norito, SoraFS, other):
- התחייבות זמן שבועית וחלון זמינות (UTC):
- צרכי לוקליזציה או נגישות (כן/לא + פרטים):
- אושר קוד ההתנהגות לקהילה + נספח שימוש מקובל ל-preview (כן/לא):

</div>
