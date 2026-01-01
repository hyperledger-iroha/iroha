---
id: preview-feedback-w2-summary
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| פריט | פרטים |
| --- | --- |
| גל | W2 - reviewers קהילתיים |
| חלון הזמנה | 2025-06-15 -> 2025-06-29 |
| תג ארטיפקט | `preview-2025-06-15` |
| כרטיס מעקב | `DOCS-SORA-Preview-W2` |
| משתתפים | comm-vol-01...comm-vol-08 |

## נקודות בולטות

1. **Governance & tooling** - מדיניות intake קהילתית אושרה פה אחד ב-2025-05-20; template הבקשה המעודכן עם שדות motivation/timezone נמצא תחת `docs/examples/docs_preview_request_template.md`.
2. **Preflight evidence** - שינוי Try it proxy `OPS-TRYIT-188` רץ ב-2025-06-09, dashboards Grafana נתפסו, ופלטי descriptor/checksum/probe של `preview-2025-06-15` נשמרו תחת `artifacts/docs_preview/W2/`.
3. **גל הזמנות** - שמונה reviewers קהילתיים הוזמנו ב-2025-06-15, acknowledgements נרשמו בטבלת ההזמנות של ה-tracker; כולם השלימו אימות checksum לפני גלישה.
4. **משוב** - `docs-preview/w2 #1` (ניסוח tooltip) ו-`#2` (סדר sidebar לוקליזציה) נפתחו ב-2025-06-18 ונסגרו עד 2025-06-21 (Docs-core-04/05); לא היו incidents במהלך הגל.

## פריטי פעולה

| ID | תיאור | בעלים | סטטוס |
| --- | --- | --- | --- |
| W2-A1 | לטפל ב-`docs-preview/w2 #1` (ניסוח tooltip). | Docs-core-04 | ✅ הושלם (2025-06-21). |
| W2-A2 | לטפל ב-`docs-preview/w2 #2` (sidebar לוקליזציה). | Docs-core-05 | ✅ הושלם (2025-06-21). |
| W2-A3 | לארכב ראיות יציאה + לעדכן roadmap/status. | Docs/DevRel lead | ✅ הושלם (2025-06-29). |

## סיכום יציאה (2025-06-29)

- כל שמונת reviewers הקהילתיים אישרו סיום וגישת ה-preview בוטלה; acknowledgements נרשמו בלוג ההזמנות של ה-tracker.
- ה-snapshots הסופיים של הטלמטריה (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) נשארו ירוקים; לוגים ו-transcripts של Try it proxy צורפו ל-`DOCS-SORA-Preview-W2`.
- חבילת ראיות (descriptor, checksum log, probe output, link report, Grafana screenshots, invite acknowledgements) נשמרה תחת `artifacts/docs_preview/W2/preview-2025-06-15/`.
- לוג checkpoints W2 ב-tracker עודכן עד היציאה, כדי לוודא שה-roadmap שומר רישום בר-ביקורת לפני תחילת תכנון W3.
