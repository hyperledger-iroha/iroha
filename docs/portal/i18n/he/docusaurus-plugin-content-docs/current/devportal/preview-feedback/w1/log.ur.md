---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/log.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-log
title: W1 فيڈبیک اور ٹیلیمیٹری لاگ
sidebar_label: W1 לאג
description: پہلی پارٹنر preview wave کے لئے مجموعی roster، ٹیلیمیٹری checkpoints، اور reviewers نوٹس۔
---

یہ لاگ **W1 پارٹنر preview** کے لئے invite roster، ٹیلیمیٹری checkpoints، اور reviewer feedback محفوظ کرتا ہے
جو [`preview-feedback/w1/plan.md`](./plan.md) کی acceptance tasks اور wave tracker اندراج
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md) کے ساتھ وابستہ ہیں۔ جب دعوت ارسال ہو،
ٹیلیمیٹری snapshot ریکارڈ ہو، یا feedback item triage ہو تو اسے اپ ڈیٹ کریں تاکہ governance reviewers
بغیر بیرونی ٹکٹس کے پیچھے گئے ثبوت replay کر سکیں۔

## סגל קבוצה

| מזהה שותף | בקש כרטיס | NDA טלפון | ההזמנה נשלחה (UTC) | אישור/כניסה ראשונה (UTC) | סטטוס | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ חודש 2025-04-26 | soraps-op-01; orchestrator docs parity evidence پر فوکس۔ |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 12-04-2025 15:03 | 12-04-2025 15:15 | ✅ חודש 2025-04-26 | soraps-op-02; Norito/telemetry cross-links کی توثیق۔ |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ חודש 2025-04-26 | soraps-op-03; multi-source failover drills چلائے۔ |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 12-04-2025 15:09 | 2025-04-12 15:21 | ✅ חודש 2025-04-26 | torii-int-01; Torii `/v2/pipeline` + נסה זאת סקירת ספר בישול. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 12-04-2025 15:12 | 2025-04-12 15:23 | ✅ חודש 2025-04-26 | torii-int-02; Try it screenshot update میں ساتھ دیا (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 12-04-2025 15:15 | 2025-04-12 15:26 | ✅ חודש 2025-04-26 | sdk-partner-01; משוב JS/Swift ספר בישול + בדיקות גשר של ISO. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ חודש 2025-04-26 | sdk-partner-02; compliance 2025-04-11 کو clear، Connect/telemetry notes پر فوکس۔ |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ חודש 2025-04-26 | gateway-ops-01; ביקורת מדריך הפעלה של שער + אנונימי נסה את זרימת ה-proxy. |

**Invite sent** اور **Ack** کے timestamps فوری طور پر درج کریں جب outbound email جاری ہو۔
وقت کو W1 پلان میں دی گئی UTC schedule کے مطابق رکھیں۔

## מחסומי טלמטריה| חותמת זמן (UTC) | לוחות מחוונים / בדיקות | בעלים | תוצאה | חפץ |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ הכל ירוק | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06-04-2025 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` תמליל | אופס | ✅ מבוים | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Dashboards اوپر + `probe:portal` | Docs/DevRel + Ops | ✅ תמונת מצב לפני הזמנה, ללא רגרסיות | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | Dashboards اوپر + Try it proxy latency diff | Docs/DevRel lead | ✅ בדיקת נקודת אמצע עברה (0 התראות; נסה זאת חביון p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | Dashboards اوپر + exit probe | Docs/DevRel + קשר ממשל | ✅ יציאה מתמונת מצב, אפס התראות יוצאות דופן | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

روزانہ office-hour samples (2025-04-13 -> 2025-04-25) NDJSON + PNG exports کے طور پر
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` טלפון נייד
`docs-preview-integrity-<date>.json` اور متعلقہ screenshots کے ساتھ۔

## משוב או יומן בעיות

اس جدول کو reviewer findings کے خلاصے کے لئے استعمال کریں۔ ہر entry کو GitHub/discuss
ticket اور اس structured form سے جوڑیں جو
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) کے ذریعے بھرا گیا۔

| הפניה | חומרה | בעלים | סטטוס | הערות |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | נמוך | Docs-core-02 | ✅ נפתר 2025-04-18 | Try it nav wording + sidebar anchor واضح کیا (`docs/source/sorafs/tryit.md` نئے label کے ساتھ اپ ڈیٹ). |
| `docs-preview/w1 #2` | נמוך | Docs-core-03 | ✅ נפתר 2025-04-19 | Try it screenshot + caption reviewer کی درخواست پر اپ ڈیٹ؛ חפץ `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | מידע | Docs/DevRel lead | סגור | باقی تبصرے صرف Q&A تھے؛ ہر پارٹنر کے feedback form میں محفوظ ہیں `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## בדיקת ידע או מעקב אחר סקרים

1. ہر reviewer کے quiz scores ریکارڈ کریں (target >=90%); exported CSV کو invite artefacts کے ساتھ attach کریں۔
2. feedback form سے حاصل qualitative survey answers جمع کریں اور انہیں
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/` کے تحت mirror کریں۔
3. threshold سے نیچے والوں کے لئے remediation calls schedule کریں اور انہیں اس فائل میں log کریں۔

تمام آٹھ reviewers نے knowledge check میں >=94% اسکور کیا (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). کوئی remediation calls درکار نہیں ہوئیں؛
ہر پارٹنر کے survey exports یہاں موجود ہیں:
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## מלאי חפצים

- חבילת תצוגה מקדימה/מתאר: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- בדיקה + סיכום בדיקת קישור: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- נסה את יומן שינוי פרוקסי: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- ייצוא טלמטריה: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- חבילת טלמטריה יומית לשעות המשרד: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- משוב + ייצוא סקרים: תיקיות ספציפיות לסוקר
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` کے تحت
- בדיקת ידע ב-CSV סיכום: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Inventory کو tracker issue کے ساتھ sync رکھیں۔ جب artefacts کو governance ticket میں کاپی کریں تو hashes attach کریں
تاکہ auditors فائلیں بغیر shell access کے verify کر سکیں۔