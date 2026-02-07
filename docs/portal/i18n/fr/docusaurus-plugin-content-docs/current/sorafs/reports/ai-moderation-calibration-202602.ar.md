---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : تقرير معايرة إشراف الذكاء الاصطناعي (2026-02)
résumé: مجموعة بيانات معايرة أساسية وعتبات ولوحة نتائج لأول إصدار حوكمة MINFO-1.
---

# تقرير معايرة إشراف الذكاء الاصطناعي - فبراير 2026

يجمع هذا التقرير قطع معايرة البداية لـ **MINFO-1**. تم إنتاج ensemble de données وmanifeste وscoreboard
Le 2026-02-05, et le 2026-02-10, et le DAG الحوكمة
على الارتفاع `912044`.

## Manifest مجموعة البيانات

- **Référence de l'ensemble de données :** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Limace :** `ai-moderation-calibration-202602`
- **Entrées :** manifeste 480, bloc 12 800, métadonnées 920, audio 160
- **Mélange d'étiquettes :** sûr 68 %, suspect 19 %, escalade 13 %
- **Résumé d'artefacts :** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution :** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

يتوفر manifeste الكامل في `docs/examples/ai_moderation_calibration_manifest_202602.json`
Il s'agit d'un hachage pour coureur et d'un coureur.

## ملخص tableau de bord

تم تشغيل المعايرات باستخدام opset 17 ومسار البذور الحتمي. يسجل JSON الكامل للـ tableau de bord
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) hachages et résumés de la télémétrie
الجدول أدناه يبرز أهم المقاييس.

| النموذج (العائلة) | Brier | CEE | AUROC | Précision@Quarantaine | Rappel@Escalade |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Sécurité (vision) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Sécurité (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Ensemble perceptuel (perceptuel) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Nom du produit : `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. كان توزيع
Le taux de réussite est de 91,2 %, la mise en quarantaine de 6,8 % et l'escalade de 2,0 %, ainsi que la mise en quarantaine.
لتوقعات السياسة المسجلة في ملخص manifeste. ظل backlog للإيجابيات الكاذبة عند الصفر،
Le score de dérive (7,1%) est de 20%.

## العتبات والمصادقة

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Motion de gouvernance : `MINFO-2026-02-07`
- Signé par `ministry-council-seat-03` à `2026-02-10T11:33:12Z`

خزنت CI الحزمة الموقعة في `artifacts/ministry/ai_moderation/2026-02/` مع ثنائيات
coureur de modération. يجب الرجوع إلى digest الخاص بالـ manifeste وhashes الخاصة بالـ tableau de bord
أعلاه أثناء عمليات التدقيق والاستئناف.

## لوحات المتابعة والتنبيهات

على SREs الخاصة بالموديريشن استيراد لوحة Grafana من
`dashboards/grafana/ministry_moderation_overview.json` et Prometheus pour
`dashboards/alerts/ministry_moderation_rules.yml` (transfert d'informations sur le produit
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`). تصدر هذه
القطع تنبيهات لتوقفات ingestion وارتفاعات drift ونمو طابور quarantaine, وتلبي متطلبات
المراقبة المشار إليها في
[Spécification du coureur de modération AI] (../../ministry/ai-moderation-runner.md).