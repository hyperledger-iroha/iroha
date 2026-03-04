---
lang: he
direction: rtl
source: docs/examples/docs_preview_feedback_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afb7e51ddc0b7e819f2cbf3888aadf907b0e0010c676cb44af648f9f4818f8f5
source_last_modified: "2025-11-10T19:22:20.036140+00:00"
translation_last_reviewed: 2026-01-30
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/docs_preview_feedback_form.md -->

# טופס משוב ל-docs preview (גל W1 לשותפים)

השתמשו בתבנית זו בעת איסוף משוב ממבקרי W1. שכפלו אותה לכל partner, מלאו את
המטא-נתונים, ושמרו את העותק המושלם תחת
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.

## מטא-נתונים של הסוקר

- **Partner ID:** `partner-w1-XX`
- **כרטיס בקשה:** `DOCS-SORA-Preview-REQ-PXX`
- **הזמנה נשלחה (UTC):** `YYYY-MM-DD hh:mm`
- **אישור checksum (UTC):** `YYYY-MM-DD hh:mm`
- **אזורי פוקוס עיקריים:** (לדוגמה _מסמכי אורקסטרטור SoraFS_, _זרימות Torii ISO_)

## אישורי טלמטריה ו-artefacts

| פריט צ'קליסט | תוצאה | ראיה |
| --- | --- | --- |
| אימות checksum | ✅ / ⚠️ | נתיב ללוג (למשל, `build/checksums.sha256`) |
| Smoke test של Try it proxy | ✅ / ⚠️ | קטע מתוך `npm run manage:tryit-proxy …` |
| סקירת לוח Grafana | ✅ / ⚠️ | נתיב(ים) לצילום מסך |
| סקירת דוח probe של הפורטל | ✅ / ⚠️ | `artifacts/docs_preview/.../preflight-summary.json` |

הוסיפו שורות לכל SLO נוסף שהסוקר בודק.

## יומן משוב

| תחום | חומרה (info/minor/major/blocker) | תיאור | תיקון מוצע או שאלה | Issue למעקב |
| --- | --- | --- | --- | --- |
| | | | | |

ציינו בעמודה האחרונה את ה-issue ב-GitHub או כרטיס פנימי כדי ש-preview tracker יוכל
לקשר את פריטי התיקון לטופס זה.

## סיכום סקר

1. **כמה אתם בטוחים בהנחיות ה-checksum ותהליך ההזמנה?** (1–5)
2. **אילו docs היו הכי/הכי פחות שימושיים?** (תשובה קצרה)
3. **האם היו חסימות בגישה ל-Try it proxy או ללוחות הטלמטריה?**
4. **האם נדרש תוכן נוסף ללוקליזציה או לנגישות?**
5. **הערות נוספות לפני GA?**

תעדו תשובות קצרות וצרפו ייצואי סקר גולמיים אם השתמשתם בטופס חיצוני.

## בדיקת ידע

- ציון: `__/10`
- שאלות שגויות (אם יש): `[#1, #4, …]`
- פעולות מעקב (אם הציון < 9/10): נקבעה שיחת remediation? כן/לא

## חתימה

- שם הסוקר וחותמת זמן:
- שם סוקר Docs/DevRel וחותמת זמן:

שמרו את העותק החתום יחד עם ה-artefacts המשויכים כדי שמבקרים יוכלו לשחזר את גל
הבדיקה בלי הקשר נוסף.

</div>
