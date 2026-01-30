---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d9a4e09025f8a37b3828ed148352f512c07d5c70c3b0139f6a4a17fc6b44e1bc
source_last_modified: "2025-11-14T04:43:19.845011+00:00"
translation_last_reviewed: 2026-01-30
---

הלוג הזה שומר את roster ההזמנות, checkpoints טלמטריה ומשוב reviewers עבור
**preview השותפים W1** שמלווה את משימות הקבלה ב-[`preview-feedback/w1/plan.md`](./plan.md)
ואת רשומת ה-tracker של הגל ב-[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md).
לעדכן אותו כאשר נשלחת הזמנה, נשמר snapshot טלמטריה, או triage נעשה לפריט משוב, כדי שמבקרי
governance יוכלו לשחזר את הראיות בלי לרדוף אחרי כרטיסים חיצוניים.

## רוסטר הקבוצה

| Partner ID | כרטיס בקשה | NDA התקבל | הזמנה נשלחה (UTC) | Ack/כניסה ראשונה (UTC) | סטטוס | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ הושלם 2025-04-26 | sorafs-op-01; התמקד בראיות פריטי parity של docs ל-orchestrator. |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ הושלם 2025-04-26 | sorafs-op-02; אימת cross-links Norito/telemetry. |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ הושלם 2025-04-26 | sorafs-op-03; הריץ drills של failover multi-source. |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ הושלם 2025-04-26 | torii-int-01; סקירת cookbook של Torii `/v1/pipeline` ו-Try it. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ הושלם 2025-04-26 | torii-int-02; עזר בעדכון צילום Try it (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ הושלם 2025-04-26 | sdk-partner-01; משוב cookbooks JS/Swift + sanity checks ל-ISO bridge. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ הושלם 2025-04-26 | sdk-partner-02; compliance אושר 2025-04-11, התמקד בהערות Connect/telemetry. |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ הושלם 2025-04-26 | gateway-ops-01; בדק מדריך ops של gateway + זרימת proxy Try it אנונימית. |

למלא את חותמות הזמן של **הזמנה נשלחה** ו-**Ack** מיד עם שליחת המייל.
לעגן את הזמנים ללוח UTC המוגדר בתכנית W1.

## Checkpoints טלמטריה

| חותמת זמן (UTC) | Dashboards / probes | בעלים | תוצאה | ארטיפקט |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ הכל ירוק | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | תמלול `npm run manage:tryit-proxy -- --stage preview-w1` | Ops | ✅ הועמד | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Dashboards לעיל + `probe:portal` | Docs/DevRel + Ops | ✅ snapshot לפני ההזמנה, בלי רגרסיות | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Dashboards לעיל + diff לטנטיות Try it | Docs/DevRel lead | ✅ בדיקת אמצע עברה (0 alertים; לטנטיות Try it p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Dashboards לעיל + probe יציאה | Docs/DevRel + Governance liaison | ✅ snapshot יציאה, אפס alertים פתוחים | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

דגימות office hours יומיות (2025-04-13 -> 2025-04-25) ארוזות כ-exports NDJSON + PNG תחת
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` עם שמות קבצים
`docs-preview-integrity-<date>.json` וצילומי המסך התואמים.

## לוג משוב ותקלות

השתמשו בטבלה הזו כדי לסכם ממצאים של reviewers. קשרו כל רשומה לכרטיס GitHub/discuss
ולנוסח הטופס המובנה שנלכד דרך
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| רפרנס | חומרה | בעלים | סטטוס | הערות |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Low | Docs-core-02 | ✅ נפתר 2025-04-18 | ניסוח ניווט Try it הובהר + עוגן sidebar (`docs/source/sorafs/tryit.md` עודכן עם label חדש). |
| `docs-preview/w1 #2` | Low | Docs-core-03 | ✅ נפתר 2025-04-19 | צילום Try it והכיתוב עודכנו לבקשת reviewer; ארטיפקט `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Info | Docs/DevRel lead | 🟢 סגור | שאר ההערות היו Q&A בלבד; נתפסו בטופס המשוב של כל שותף תחת `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## מעקב knowledge check וסקרים

1. לרשום ציוני quiz (יעד >=90%) לכל reviewer; לצרף את קובץ ה-CSV המיוצא לצד ארטיפקטי ההזמנה.
2. לאסוף את תשובות הסקר האיכותיות שנתפסו בטופס המשוב ולשקף אותן תחת
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. לתזמן שיחות remediation לכל מי שמתחת לסף ולתעד אותן בקובץ זה.

כל שמונת reviewers קיבלו >=94% ב-knowledge check (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). לא נדרשו שיחות remediation;
exports של הסקר לכל שותף נמצאים תחת
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## מלאי ארטיפקטים

- Bundle של preview descriptor/checksum: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- סיכום probe + link-check: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- לוג שינוי של proxy Try it: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- exports טלמטריה: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- bundle טלמטריה יומי של office hours: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- exports feedback + survey: לשים תיקיות לפי reviewer תחת
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV knowledge check ותקציר: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

לשמור על המלאי מסונכרן עם כרטיס ה-tracker. לצרף hashes בעת העתקת ארטיפקטים לכרטיס governance
כדי שמבקרים יוכלו לאמת את הקבצים ללא גישת shell.
