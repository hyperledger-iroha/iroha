---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Relatorio de calibracao de moderacao de IA (2026-02)
ملخص: قاعدة بيانات المعايرة والعتبات ولوحة النتائج للإصدار الأول لإدارة MINFO-1.
---

# Relatorio de calibracao de moderacao de IA - Fevereiro 2026

يحتوي هذا الرابط على أدوات المعايرة الأولية لـ **MINFO-1**. يا
مجموعة البيانات، أو البيان، أو لوحة النتائج، أو منتدى المنتجات في 2026-02-05، تمت مراجعتها مؤخرًا
مجلس الوزراء في 10/02/2026 ووافق على DAG للحوكمة في الارتفاع
`912044`.

## بيان مجموعة البيانات

- **مرجع مجموعة البيانات:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **السبيكة:** `ai-moderation-calibration-202602`
- **الإدخالات:** البيان 480، القطعة 12,800، البيانات الوصفية 920، الصوت 160
- **مزيج التصنيف:** آمن بنسبة 68%، ومشتبه به بنسبة 19%، ومتصاعد بنسبة 13%
- **ملخص المصنوعات:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **التوزيع:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

البيان كامل في `docs/examples/ai_moderation_calibration_manifest_202602.json`
يفكرون في اغتيال الحاكم، ولا يتم القبض على العداء في أي لحظة
الافراج.

## Resumo do لوحة النتائج

كما يتم قياس المعايرة مع 17 خط أنابيب حتمية البذور. يا
JSON لوحة النتائج الكاملة (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
تسجيل التجزئة وملخصات القياس عن بعد؛ لوحة يمكن إزالتها كمقاييس أخرى
مهم.| موديلو (فاميليا) | برير | اللجنة الاقتصادية لأوروبا | أوروك | الدقة @ الحجر الصحي | استدعاء @ التصعيد |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 السلامة (الرؤية) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B السلامة (متعدد الوسائط) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| المجموعة الإدراكية (الإدراك الحسي) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

مجموعات المقاييس: `Brier = 0.126`، `ECE = 0.034`، `AUROC = 0.982`. توزيعة
تم تمرير vereditos na janela de calibracao 91.2%، والحجر الصحي 6.8%،
تصاعد 2.0%, كما هو متوقع من السياسيين المسجلين لم يستأنفوا
واضح. تراكم الأخطاء الإيجابية الدائمة عند الصفر، ودرجة الانجراف (7.1%)
لا داعي للقلق بشأن الحد من التنبيه بنسبة 20%.

## عتبات تسجيل الخروج

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- اقتراح الحوكمة: `MINFO-2026-02-07`
- تم التوقيع عليه بواسطة `ministry-council-seat-03` في `2026-02-10T11:33:12Z`

تم تخزين الحزمة أو تدميرها في `artifacts/ministry/ai_moderation/2026-02/`
Junto com os binarios يقوم بتشغيل الاعتدال. يا هضم تفعل التجزئة e os واضح
يجب أن تكون لوحة النتائج مرجعية خلال جلسات الاستماع والمكالمات.

## لوحات المعلومات والتنبيهاتيجب استيراد SREs من لوحة القيادة إلى لوحة المعلومات Grafana em
`dashboards/grafana/ministry_moderation_overview.json` كما تفعل تعليمات التنبيه
Prometheus في `dashboards/alerts/ministry_moderation_rules.yml` (غطاء
اختبارات fica em `dashboards/alerts/tests/ministry_moderation_rules.test.yml`). جوهر
أرسل تنبيهات فنية لاستيعاب الأكشاك والمسامير المنجرفة ونمو الفيلا
الحجر الصحي، واتباع متطلبات المراقبة المشار إليها
[مواصفات مشغل الاعتدال AI](../../ministry/ai-moderation-runner.md).