---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: تقرير معايرة إشراف الذكاء الاصطناعي (2026-02)
resumen: مجموعة بيانات معايرة أساسية وعتبات ولوحة نتائج لأول إصدار حوكمة MINFO-1.
---

# تقرير معايرة إشراف الذكاء الاصطناعي - فبراير 2026

يجمع هذا التقرير قطع معايرة البداية لـ **MINFO-1**. تم إنتاج conjunto de datos, manifiesto y marcador
En 2026-02-05, y en el 2026-02-10, y en el DAG الحوكمة.
Verifique `912044`.

## Manifiesto مجموعة البيانات

- **Referencia del conjunto de datos:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Babosa:** `ai-moderation-calibration-202602`
- **Entradas:** manifiesto 480, fragmento 12.800, metadatos 920, audio 160
- **Mezcla de etiquetas:** seguro 68%, sospechoso 19%, escalado 13%
- **Resumen de artefactos:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribución:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

يتوفر manifiesto الكامل في `docs/examples/ai_moderation_calibration_manifest_202602.json`
ويتضمن توقيع الحوكمة بالإضافة إلى hash الخاص بالـ runner الملتقط وقت الإصدار.

## marcador de ملخص

تم تشغيل المعايرات باستخدام opset 17 ومسار البذور الحتمي. يسجل JSON الكامل للـ marcador
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) hashes y resúmenes de telemetría
الجدول أدناه يبرز أهم المقاييس.| النموذج (العائلة) | Zarzo | CEPE | AURÓC | Precisión@Cuarentena | Recordar@Escalar |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Seguridad (visión) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Seguridad (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Conjunto perceptual (perceptual) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Nombre del producto: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. كان توزيع
أحكام عبر نافذة المعايرة هو pase 91.2% وcuarentena 6.8% وescalada 2.0%، وهو مطابق
لتوقعات السياسة المسجلة في ملخص manifiesto. ظل trabajo pendiente للإيجابيات الكاذبة عند الصفر،
Puntuación de deriva (7,1%) أقل بكثير من عتبة التنبيه 20%.

## العتبات والمصادقة

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Moción de gobernanza: `MINFO-2026-02-07`
- Firmado por `ministry-council-seat-03` en `2026-02-10T11:33:12Z`

خزنت CI الحزمة الموقعة في `artifacts/ministry/ai_moderation/2026-02/` مع ثنائيات
corredor de moderación. يجب الرجوع إلى resumen الخاص بالـ manifiesto وhashes الخاصة بالـ marcador
أعلاه أثناء عمليات التدقيق والاستئناف.

## لوحات المتابعة والتنبيهات

Para obtener SRes, consulte el código Grafana.
`dashboards/grafana/ministry_moderation_overview.json` Y تنبيهات Prometheus في
`dashboards/alerts/ministry_moderation_rules.yml` (تغطية الاختبارات موجودة في
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`). تصدر هذه
القطع تنبيهات لتوقفات ingestión وارتفاعات deriva ونمو طابور cuarentena، وتلبي متطلبات
المراقبة المشار إليها في
[Especificación del corredor de moderación de IA](../../ministry/ai-moderation-runner.md).