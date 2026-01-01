---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76a4303fa2657476a3f983f1aa5597c9ddb478f670d233b0a7cf4e3791419a72
source_last_modified: "2025-11-20T12:45:46.606949+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: preview-feedback-w3-summary
title: סיכום משוב וסטטוס W3 בטא
sidebar_label: סיכום W3
description: תקציר חי לגל הבטא 2026 (finance, observability, SDK, ecosystem).
---

| פריט | פרטים |
| --- | --- |
| גל | W3 - קוהורטים בטא (finance + ops + partner SDK + ecosystem advocate) |
| חלון הזמנה | 2026-02-18 -> 2026-02-28 |
| תג ארטיפקט | `preview-20260218` |
| כרטיס מעקב | `DOCS-SORA-Preview-W3` |
| משתתפים | finance-beta-01, observability-ops-02, partner-sdk-03, ecosystem-advocate-04 |

## נקודות בולטות

1. **Pipeline ראיות end-to-end.** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` מייצר סיכום לכל גל (`artifacts/docs_portal_preview/preview-20260218-summary.json`), digest (`preview-20260218-digest.md`), ומרענן `docs/portal/src/data/previewFeedbackSummary.json` כדי שמבקרי governance יוכלו להסתמך על פקודה אחת.
2. **כיסוי טלמטריה + governance.** כל ארבעת ה-reviewers אישרו גישה עם checksum, שלחו משוב ובוטלו בזמן; ה-digest מפנה לבעיות משוב (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) לצד ריצות Grafana שנאספו במהלך הגל.
3. **הצגה בפורטל.** הטבלה המעודכנת בפורטל מציגה כעת את גל W3 הסגור עם מדדי latency ו-response rate, והדף החדש של הלוג למטה משקף את ציר הזמן עבור auditors שלא מושכים את לוג ה-JSON הגולמי.

## פריטי פעולה

| ID | תיאור | בעלים | סטטוס |
| --- | --- | --- | --- |
| W3-A1 | ללכוד את ה-preview digest ולצרף ל-tracker. | Docs/DevRel lead | ✅ הושלם (2026-02-28). |
| W3-A2 | לשקף ראיות הזמנה/digest בפורטל + roadmap/status. | Docs/DevRel lead | ✅ הושלם (2026-02-28). |

## סיכום יציאה (2026-02-28)

- ההזמנות נשלחו ב-2026-02-18 וה-acknowledgements נרשמו דקות לאחר מכן; גישת preview בוטלה ב-2026-02-28 לאחר בדיקת טלמטריה אחרונה שעברה.
- Digest + סיכום נשמרו תחת `artifacts/docs_portal_preview/`, עם לוג גולמי מעוגן ב-`artifacts/docs_portal_preview/feedback_log.json` לצורך replay.
- מעקבי issues נפתחו תחת `docs-preview/20260218` עם tracker ה-governance `DOCS-SORA-Preview-20260218`; הערות CSP/Try it נותבו לבעלי observability/finance וקושרו מתוך ה-digest.
- שורת ה-tracker עודכנה ל-🈴 Completed וטבלת המשוב בפורטל משקפת את סגירת הגל, ומשלימה את משימת הבטא האחרונה של DOCS-SORA.
