---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: تقرير معايرة إشراف الذكاء الاصطناعي (2026-02)
summary: مجموعة بيانات معايرة أساسية وعتبات ولوحة نتائج لأول إصدار حوكمة MINFO-1.
---

# تقرير معايرة إشراف الذكاء الاصطناعي - فبراير 2026

يجمع هذا التقرير قطع معايرة البداية لـ **MINFO-1**. מערכת נתונים, מניפסט ולוח תוצאות
בתאריך 2026-02-05, ותקשורת בתאריך 2026-02-10, ותקשורת של DAG.
على الارتفاع `912044`.

## Manifest مجموعة البيانات

- **הפניה למערך נתונים:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **שבלול:** `ai-moderation-calibration-202602`
- **כניסות:** מניפסט 480, נתח 12,800, מטא נתונים 920, אודיו 160
- **תערובת תווית:** בטוחה 68%, חשוד 19%, הסלמה 13%
- **תמצית חפצים:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **הפצה:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

يتوفر manifest الكامل في `docs/examples/ai_moderation_calibration_manifest_202602.json`
ويتضمن توقيع الحوكمة بالإضافة إلى hash الخاص بالـ runner الملتقط وقت الإصدار.

## ملخص scoreboard

تم تشغيل المعايرات باستخدام opset 17 ومسار البذور الحتمي. يسجل JSON الكامل للـ scoreboard
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) hashes وdigests الخاصة بـ telemetry؛
الجدول أدناه يبرز أهم المقاييس.

| النموذج (العائلة) | ברייר | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 בטיחות (ראייה) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (מולטימודאלי) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| אנסמבל תפיסתי (תפיסתי) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

المقاييس المجمعة: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. كان توزيع
الأحكام عبر نافذة المعايرة هو pass 91.2% وquarantine 6.8% وescalate 2.0%، وهو مطابق
لتوقعات السياسة المسجلة في ملخص manifest. ظل backlog للإيجابيات الكاذبة عند الصفر،
ציון סחף (7.1%) זכה בשיעור של 20%.

## מידע ותקשורת

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- הצעת ממשל: `MINFO-2026-02-07`
- חתום על ידי `ministry-council-seat-03` ב-`2026-02-10T11:33:12Z`

خزنت CI الحزمة الموقعة في `artifacts/ministry/ai_moderation/2026-02/` مع ثنائيات
רץ מתינות. يجب الرجوع إلى digest الخاص بالـ manifest وhashes الخاصة بالـ scoreboard
أعلاه أثناء عمليات التدقيق والاستئناف.

## لوحات المتابعة والتنبيهات

على SREs الخاصة بالموديريشن استيراد لوحة Grafana من
`dashboards/grafana/ministry_moderation_overview.json` وقواعد تنبيهات Prometheus في
`dashboards/alerts/ministry_moderation_rules.yml` (تغطية الاختبارات موجودة في
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`). تصدر هذه
מידע על בליעה וסחף וסחף והסגר, וסגר
المراقبة المشار إليها في
[מפרט AI Moderation Runner](../../ministry/ai-moderation-runner.md).