---
id: preview-feedback-w0-summary
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w0/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| פריט | פרטים |
| --- | --- |
| גל | W0 - מתחזקי ליבה |
| תאריך התקציר | 2025-03-27 |
| חלון סקירה | 2025-03-25 -> 2025-04-08 |
| משתתפים | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| תג ארטיפקט | `preview-2025-03-24` |

## נקודות בולטות

1. **תהליך checksum** - כל הסוקרים אישרו ש-`scripts/preview_verify.sh`
   הצליח מול זוג ה-descriptor/archive המשותף. לא נדרשו עקיפות ידניות.
2. **משוב ניווט** - דווחו שתי תקלות קטנות בסדר השדה הצדדי
   (`docs-preview/w0 #1-#2`). שתיהן הופנו ל-Docs/DevRel ואינן חוסמות את
   הגל.
3. **תאימות runbook של SoraFS** - sorafs-ops-01 ביקש קישורים צולבים ברורים יותר
   בין `sorafs/orchestrator-ops` ל-`sorafs/multi-source-rollout`. נפתחה משימת
   המשך; לטפל לפני W1.
4. **סקירת טלמטריה** - observability-01 אישר ש-`docs.preview.integrity`,
   `TryItProxyErrors` ולוגי ה-proxy של Try-it נשארו ירוקים; לא הופעלו התראות.

## פריטי פעולה

| מזהה | תיאור | בעלים | סטטוס |
| --- | --- | --- | --- |
| W0-A1 | לסדר מחדש את ערכי ה-sidebar של devportal כדי להבליט מסמכים לריוויורים (`preview-invite-*` בקבוצה אחת). | Docs-core-01 | הושלם - ה-sidebar מציג כעת את מסמכי הריוויורים ברצף (`docs/portal/sidebars.js`). |
| W0-A2 | להוסיף קישור צולב מפורש בין `sorafs/orchestrator-ops` ל-`sorafs/multi-source-rollout`. | Sorafs-ops-01 | הושלם - כל runbook מקשר כעת לשני כדי שמפעילים יראו את שני המדריכים בזמן rollouts. |
| W0-A3 | לשתף צילומי טלמטריה + חבילת שאילתות עם ה-tracker של governance. | Observability-01 | הושלם - החבילה צורפה ל-`DOCS-SORA-Preview-W0`. |

## סיכום סיום (2025-04-08)

- כל חמשת הסוקרים אישרו סיום, ניקו בניות מקומיות ויצאו מחלון ה-preview;
  ביטולי הגישה נרשמו ב-`DOCS-SORA-Preview-W0`.
- לא היו אירועים או התראות במהלך הגל; לוחות הטלמטריה נשארו ירוקים לכל התקופה.
- פעולות ניווט + קישורים צולבים (W0-A1/A2) מיושמות ומשתקפות במסמכים לעיל;
  ראיות הטלמטריה (W0-A3) מצורפות ל-tracker.
- חבילת ראיות נשמרה: צילומי מסך של טלמטריה, אישורי הזמנה ותקציר זה
  מקושרים מה-issue של ה-tracker.

## הצעדים הבאים

- להשלים את פריטי הפעולה של W0 לפני פתיחת W1.
- להשיג אישור משפטי וסלוט staging לפרוקסי, ואז לבצע את צעדי ה-preflight של
  גל השותפים המפורטים ב-[preview invite flow](../../preview-invite-flow.md).

_תקציר זה מקושר מתוך [preview invite tracker](../../preview-invite-tracker.md) כדי
לשמור על עקיבות ה-roadmap DOCS-SORA._
