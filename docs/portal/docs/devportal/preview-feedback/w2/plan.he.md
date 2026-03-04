---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9a599a71cc49432334dbf323125756fc6056414a4b8f7622d4cc69edcfbd7503
source_last_modified: "2025-11-11T05:15:49.840830+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: preview-feedback-w2-plan
title: תכנית intake קהילתית W2
sidebar_label: תכנית W2
description: Intake, אישורים וצ'קליסט ראיות עבור מחזור ה-preview הקהילתי.
---

| פריט | פרטים |
| --- | --- |
| גל | W2 - reviewers קהילתיים |
| חלון יעד | Q3 2025 שבוע 1 (טיוטה) |
| תג ארטיפקט (מתוכנן) | `preview-2025-06-15` |
| כרטיס מעקב | `DOCS-SORA-Preview-W2` |

## יעדים

1. להגדיר קריטריוני intake קהילתיים וזרימת vetting.
2. לקבל אישור governance עבור roster מוצע ותוספת acceptable-use.
3. לרענן את ארטיפקט ה-preview המאומת ב-checksum ואת חבילת הטלמטריה לחלון החדש.
4. להכין את Try it proxy והדשבורדים לפני שליחת ההזמנות.

## פירוט משימות

| ID | משימה | בעלים | יעד | סטטוס | הערות |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | לנסח קריטריוני intake קהילתיים (זכאות, max slots, דרישות CoC) ולהפיץ ל-governance | Docs/DevRel lead | 2025-05-15 | ✅ הושלם | מדיניות intake מוזגה ל-`DOCS-SORA-Preview-W2` ואושרה בישיבת המועצה 2025-05-20. |
| W2-P2 | לעדכן תבנית בקשה עם שאלות קהילתיות (מוטיבציה, זמינות, צרכי לוקליזציה) | Docs-core-01 | 2025-05-18 | ✅ הושלם | `docs/examples/docs_preview_request_template.md` כולל כעת את סעיף Community ומצוין בטופס intake. |
| W2-P3 | לקבל אישור governance לתכנית intake (הצבעה בישיבה + פרוטוקולים מוקלטים) | Governance liaison | 2025-05-22 | ✅ הושלם | ההצבעה עברה פה אחד ב-2025-05-20; הפרוטוקולים ו-roll call מקושרים ב-`DOCS-SORA-Preview-W2`. |
| W2-P4 | לתזמן staging של Try it proxy + לכידת טלמטריה לחלון W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | ✅ הושלם | Ticket שינוי `OPS-TRYIT-188` אושר ובוצע 2025-06-09 02:00-04:00 UTC; צילומי Grafana נשמרו עם הטיקט. |
| W2-P5 | לבנות/לאמת tag חדש של preview artefact (`preview-2025-06-15`) ולארכב descriptor/checksum/probe logs | Portal TL | 2025-06-07 | ✅ הושלם | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` רץ ב-2025-06-10; outputs נשמרו תחת `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | להרכיב roster הזמנות קהילתיות (<=25 reviewers, בקבוצות) עם אנשי קשר מאושרים ע"י governance | Community manager | 2025-06-10 | ✅ הושלם | הקוהורט הראשון של 8 reviewers קהילתיים אושר; IDs `DOCS-SORA-Preview-REQ-C01...C08` נרשמו ב-tracker. |

## Checklist ראיות

- [x] רישום אישור governance (סיכומי פגישה + קישור הצבעה) מצורף ל-`DOCS-SORA-Preview-W2`.
- [x] תבנית בקשה מעודכנת commited תחת `docs/examples/`.
- [x] Descriptor `preview-2025-06-15`, checksum log, probe output, link report ו-transcript של Try it proxy נשמרו תחת `artifacts/docs_preview/W2/`.
- [x] צילומי Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) נאספו לחלון preflight של W2.
- [x] טבלת roster הזמנות עם IDs של reviewers, tickets של בקשה ו-timestamps של אישור מולאו לפני שליחה (ראו סעיף W2 ב-tracker).

השאירו את התכנית מעודכנת; ה-tracker מפנה אליה כדי שה-roadmap DOCS-SORA יראה מה נשאר לפני שליחת הזמנות W2.
