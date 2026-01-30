---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 48d5e3c40d3e6639530ce647616bbe935926423b2796732d565f233836d120ac
source_last_modified: "2025-11-14T04:43:22.176714+00:00"
translation_last_reviewed: 2026-01-30
---

# تقرير معايرة إشراف الذكاء الاصطناعي - فبراير 2026

يجمع هذا التقرير قطع معايرة البداية لـ **MINFO-1**. تم إنتاج dataset وmanifest وscoreboard
في 2026-02-05، وتمت مراجعتها من مجلس الوزارة في 2026-02-10، وتم تثبيتها في DAG الحوكمة
على الارتفاع `912044`.

## Manifest مجموعة البيانات

- **Dataset reference:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **Entries:** manifest 480, chunk 12,800, metadata 920, audio 160
- **Label mix:** safe 68%, suspect 19%, escalate 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

يتوفر manifest الكامل في `docs/examples/ai_moderation_calibration_manifest_202602.json`
ويتضمن توقيع الحوكمة بالإضافة إلى hash الخاص بالـ runner الملتقط وقت الإصدار.

## ملخص scoreboard

تم تشغيل المعايرات باستخدام opset 17 ومسار البذور الحتمي. يسجل JSON الكامل للـ scoreboard
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) hashes وdigests الخاصة بـ telemetry؛
الجدول أدناه يبرز أهم المقاييس.

| النموذج (العائلة) | Brier | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Safety (vision) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (multimodal) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| Perceptual ensemble (perceptual) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

المقاييس المجمعة: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. كان توزيع
الأحكام عبر نافذة المعايرة هو pass 91.2% وquarantine 6.8% وescalate 2.0%، وهو مطابق
لتوقعات السياسة المسجلة في ملخص manifest. ظل backlog للإيجابيات الكاذبة عند الصفر،
وبقي drift score (7.1%) أقل بكثير من عتبة التنبيه 20%.

## العتبات والمصادقة

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Governance motion: `MINFO-2026-02-07`
- Signed by `ministry-council-seat-03` at `2026-02-10T11:33:12Z`

خزنت CI الحزمة الموقعة في `artifacts/ministry/ai_moderation/2026-02/` مع ثنائيات
moderation runner. يجب الرجوع إلى digest الخاص بالـ manifest وhashes الخاصة بالـ scoreboard
أعلاه أثناء عمليات التدقيق والاستئناف.

## لوحات المتابعة والتنبيهات

على SREs الخاصة بالموديريشن استيراد لوحة Grafana من
`dashboards/grafana/ministry_moderation_overview.json` وقواعد تنبيهات Prometheus في
`dashboards/alerts/ministry_moderation_rules.yml` (تغطية الاختبارات موجودة في
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`). تصدر هذه
القطع تنبيهات لتوقفات ingestion وارتفاعات drift ونمو طابور quarantine، وتلبي متطلبات
المراقبة المشار إليها في
[AI Moderation Runner Specification](../../ministry/ai-moderation-runner.md).
