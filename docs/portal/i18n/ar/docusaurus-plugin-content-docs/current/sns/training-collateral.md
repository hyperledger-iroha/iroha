---
lang: ar
direction: rtl
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: training-collateral
title: مواد تدريب SNS
description: المنهاج، سير العمل الخاص بالتوطين، وتوثيق أدلة الملاحق المطلوبة بموجب SN-8.
---

> يعكس `docs/source/sns/training_collateral.md`. استخدم هذه الصفحة عند الإحاطة بفرق المُسجّل وDNS والوصاة والمالية قبل كل إطلاق لاحقة.

## 1. لقطة المنهاج

| المسار | الأهداف | قراءات مسبقة |
|-------|------------|-----------|
| عمليات المُسجّل | إرسال المانيفستات، مراقبة لوحات KPI، تصعيد الأخطاء. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS والبوابة | تطبيق هياكل المُحلِّل، تدريب التجميد/الرجوع. | `sorafs/gateway-dns-runbook`, direct-mode policy samples. |
| الوصاة والمجلس | تنفيذ النزاعات، تحديث ملاحق الحوكمة، تسجيل الملاحق. | `sns/governance-playbook`, steward scorecards. |
| المالية والتحليلات | التقاط مقاييس ARPU/bulk، نشر حزم الملاحق. | `finance/settlement-iso-mapping`, KPI dashboard JSON. |

### تدفق الوحدات

1. **M1 — توجيه KPI (30 دقيقة):** استعراض مرشحات اللاحقة والتصديرات وعدّادات التجميد. المُخرَج: لقطات PDF/CSV مع بصمة SHA-256.
2. **M2 — دورة حياة المانيفست (45 دقيقة):** بناء والتحقق من مانيـفستات المُسجّل، توليد هياكل المُحلِّل عبر `scripts/sns_zonefile_skeleton.py`. المُخرَج: diff من git يظهر الهيكل + دليل GAR.
3. **M3 — تمارين النزاع (40 دقيقة):** محاكاة تجميد الوصي + الاستئناف، التقاط سجلات CLI تحت `artifacts/sns/training/<suffix>/<cycle>/logs/`.
4. **M4 — التقاط الملاحق (25 دقيقة):** تصدير JSON للوحة وتشغيل:

   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   المُخرَج: Markdown للملاحق محدّث + مذكرة تنظيمية + كتل البوابة.

## 2. سير عمل التوطين

- اللغات: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, `ur`.
- كل ترجمة بجوار الملف المصدر (`docs/source/sns/training_collateral.<lang>.md`). حدّث `status` + `translation_last_reviewed` بعد المراجعة.
- أصول كل لغة تحت `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (slides/, workbooks/, recordings/, logs/).
- شغّل `python3 scripts/sync_docs_i18n.py --lang <code>` بعد تعديل المصدر الإنجليزي ليظهر الهاش الجديد للمترجمين.

### قائمة التسليم

1. حدّث نموذج الترجمة (`status: complete`) بعد التعريب.
2. صدّر الشرائح إلى PDF وارفعها إلى مجلد `slides/` لكل لغة.
3. سجّل walkthrough KPI لمدة ≤10 دقائق؛ اربطه من نموذج اللغة.
4. افتح تذكرة حوكمة بعلامة `sns-training` تحتوي على بصمات الشرائح/workbook وروابط التسجيل وأدلة الملاحق.

## 3. أصول التدريب

- مخطط الشرائح: `docs/examples/sns_training_template.md`.
- قالب workbook: `docs/examples/sns_training_workbook.md` (واحد لكل مشارك).
- الدعوات + التذكيرات: `docs/examples/sns_training_invite_email.md`.
- نموذج التقييم: `docs/examples/sns_training_eval_template.md` (تُحفَظ الردود في `artifacts/sns/training/<suffix>/<cycle>/feedback/`).

## 4. الجدولة والمقاييس

| الدورة | النافذة | المقاييس | ملاحظات |
|-------|--------|---------|-------|
| 2026‑03 | بعد مراجعة KPI | نسبة الحضور %، وتسجيل بصمة الملحق | `.sora` + `.nexus` cohorts |
| 2026‑06 | قبل GA لـ `.dao` | جاهزية المالية ≥90 % | يتضمن تحديث السياسة |
| 2026‑09 | التوسع | تمرين نزاع <20 دقيقة، SLA للملحق ≤2 أيام | مواءمة مع حوافز SN-7 |

اجمع الملاحظات المجهولة في `docs/source/sns/reports/sns_training_feedback.md` حتى تحسّن الدفعات اللاحقة التوطين والتمارين العملية.

