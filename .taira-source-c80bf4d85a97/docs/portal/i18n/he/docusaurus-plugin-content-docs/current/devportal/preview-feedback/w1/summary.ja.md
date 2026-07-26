---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b395cb6e35ff07d840f181822f4c347a01ae377c2459208c8b1949ebb283d47
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w1-summary
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| פריט | פרטים |
| --- | --- |
| גל | W1 - שותפים ואינטגרטורים Torii |
| חלון הזמנה | 2025-04-12 -> 2025-04-26 |
| תג ארטיפקט | `preview-2025-04-12` |
| כרטיס מעקב | `DOCS-SORA-Preview-W1` |
| משתתפים | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## נקודות בולטות

1. **Workflow checksum** - כל reviewers אימתו descriptor/archive באמצעות `scripts/preview_verify.sh`; הלוגים נשמרו לצד אישורי ההזמנה.
2. **טלמטריה** - לוחות `docs.preview.integrity`, `TryItProxyErrors` ו-`DocsPortal/GatewayRefusals` נשארו ירוקים לאורך כל הגל; לא היו incidents או alert pages.
3. **משוב מסמכים (`docs-preview/w1`)** - שני נטים קטנים דווחו:
   - `docs-preview/w1 #1`: לחדד את ניסוח הניווט בסעיף Try it (נפתר).
   - `docs-preview/w1 #2`: לעדכן צילום Try it (נפתר).
4. **פריטי runbook** - מפעילי SoraFS אישרו שה-cross-links החדשים בין `orchestrator-ops` ו-`multi-source-rollout` פתרו את נקודות W0.

## פריטי פעולה

| ID | תיאור | בעלים | סטטוס |
| --- | --- | --- | --- |
| W1-A1 | לעדכן ניסוח ניווט Try it לפי `docs-preview/w1 #1`. | Docs-core-02 | ✅ הושלם (2025-04-18). |
| W1-A2 | לרענן צילום Try it לפי `docs-preview/w1 #2`. | Docs-core-03 | ✅ הושלם (2025-04-19). |
| W1-A3 | לסכם ממצאי שותפים וראיות טלמטריה ב-roadmap/status. | Docs/DevRel lead | ✅ הושלם (ראו tracker + status.md). |

## סיכום יציאה (2025-04-26)

- כל שמונת reviewers אישרו סיום במהלך ה-office hours האחרונות, ניקו ארטיפקטים מקומיים והגישה בוטלה.
- הטלמטריה נשארה ירוקה עד היציאה; snapshots סופיים מצורפים ל-`DOCS-SORA-Preview-W1`.
- לוג ההזמנות עודכן באישורי יציאה; ה-tracker סימן את W1 כ-🈴 והוסיף checkpoints.
- חבילת ראיות (descriptor, checksum log, probe output, transcript של Try it proxy, screenshots של טלמטריה, feedback digest) נשמרה תחת `artifacts/docs_preview/W1/`.

## הצעדים הבאים

- להכין את תוכנית intake הקהילתית W2 (אישור governance + התאמות template בקשות).
- לרענן את תג ה-preview artefact עבור גל W2 ולהריץ שוב את סקריפט ה-preflight לאחר סגירת התאריכים.
- להעביר ממצאים רלוונטיים מ-W1 אל roadmap/status כדי שלגל הקהילתי יהיו ההנחיות העדכניות.
