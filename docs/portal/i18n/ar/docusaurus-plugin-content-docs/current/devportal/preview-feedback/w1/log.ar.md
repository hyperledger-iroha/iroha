---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/log.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معاينة ردود الفعل W1-سجل
العنوان: سجل المذكرة والقياس W1
Sidebar_label: سجل W1
description: قائمة مجمعة، نقاط قياس، وملاحظات المراجعين لموجة تعقب تسعة الاولى.
---

يحفظ هذا السجل قائمة الدعوات ونقاط القياس وملاحظات المراجعين لموجة ** اشتراك W1**
ملاحظة لمهام مقبولة في [`preview-feedback/w1/plan.md`](./plan.md) ومدخل متتبع في الراديو
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). قصه عند ترجمة،
او يتم تسجيل لقطة قياس، او تصنيف بند للرؤية حتى تطلعو على المساهمه من جديد
المعادلة دون ملاحقة تذاكر الطيران الخارجية.

## قائمة الدفعة

| معرف الشريك | تذكرة الطلب | استلام التجمع الوطني الديمقراطي | كتابة الأحداث (UTC) | اقرار/اول الدخول (UTC) | الحالة | تعليقات |
| --- | --- | --- | --- | --- | --- | --- |
| شريك-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ مكتملة 2025-04-26 | سورافس-المرجع-01؛ ركز على المعادلة تكافؤ وثائق الأوركسترا. |
| شريك-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ مكتملة 2025-04-26 | سورافس-المرجع-02؛ تحقق من الروابط المتقاطعة بين Norito/telemetry. |
| شريك-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ مكتملة 2025-04-26 | سورافس-المرجع-03؛ ينفذ عمليات تجاوز الفشل متعددة المصادر. |
| شريك-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ مكتملة 2025-04-26 | توري-int-01; مراجعة دليل Torii `/v2/pipeline` وكتاب الطبخ Try it. |
| شريك-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ مكتملة 2025-04-26 | توري-int-02; شارك في تحديث لقطة Try it (docs-preview/w1 #2). |
| شريك-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ مكتملة 2025-04-26 | SDK-شريك-01; مراجعات كتب الطبخ لـ JS/Swift + فحوصات العقل لجسر ISO. |
| شريك-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ مكتملة 2025-04-26 | SDK-شريك-02; تم انهاء تماما 2025-04-11، ركز على أفكار Connect/telemetry. |
| شريك-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ مكتملة 2025-04-26 | بوابة العمليات-01؛ دليل عمليات البوابة + مسار الوكيل جرب المجهول. |

املأ تواريخ **ارسال الأحداث** و **الاقرار** فورإصدار البريد واتساعه.
ربط الاوقات بجدول UTC للبحث عن خطة W1.

## نقاط القياس

| الطابعة (التوقيت العالمي المنسق) | لوحات / مجسات | المالك | النتيجة | الاثر |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`، `TryItProxyErrors`، `DocsPortal/GatewayRefusals` | مستندات/DevRel + Ops | ✅ خضراء بالكامل | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | نص `npm run manage:tryit-proxy -- --stage preview-w1` | العمليات | ✅ تم التجهيز | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | لوحات اعلانية + `probe:portal` | مستندات/DevRel + Ops | ✅ لقطة قبل الأحداث، بلا بداية | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | لوحات فنية + فرق زمن جربها | مستندات/DevRel الرصاص | ✅ اجتاز اختبار التسجيل الداخلي (0 تنبيهات; زمن Try it p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | لوحات اعلانية + مسبار خروج | Docs/DevRel + اتصال الحوكمة | ✅ لقطة خروج، صفر تنبيهات متبقية | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

تلت ساعات المكتب اليومية (2025-04-13 -> 2025-04-25) مجمعة صادرات NDJSON + PNG تحت
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` مع ملفات الاسماء
`docs-preview-integrity-<date>.json` ولقطات الجوع.

## سجل المفكرة والذاكرة

استخدم هذا الجدول لتلخيص المراجعين. ربط كل بند بتذكرة GitHub/discuss
بالاضافة الى نموذج المختار المتقاطع
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| المرجع | الشدة | المالك | الحالة | تعليقات |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | منخفض | مستندات-core-02 | ✅ تم الحل 2025-04-18 | تم الاتصال بشبكة NBC Try it + مرساة الشريط الجانبي (تم تحديث `docs/source/sorafs/tryit.md` بالوسم الجديد). |
| `docs-preview/w1 #2` | منخفض | مستندات-core-03 | ✅ تم الحل 2025-04-19 | تم تحديث الصورة Try it + التسمية حسب طلب المراجع؛ تأثير `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| — | معلومات | مستندات/DevRel الرصاص | 🟢 مغلق | كانت التعليقات النهائية أسئلة/اجابات فقط؛ تم التقاطها في نموذج تعليقات كل الشركاء تحت `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## تتبع اختبار المعرفة للاستبيان

1. سجل البحث (الهدف >=90%) لكل مرشح؛ وارفق ملف CSV المصدر بعد اثار الأحداث.
2. اجمع اجابات الاستبيان النوعي عبر نموذج المذكرات وكررها تحت
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. جدول مكالمات الحاسوب المحمول لتقليل عدد وادونها في هذا الملف.

سجل جميع المراجعين >=94% في اختبار المعرفة (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). لم تكن هناك مكالمات
مطلوبة؛ صادرات الاستبيان لكل شريك محفوظة تحت
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## جرد الاثار

- معاينة حزمة الواصف/المجموع الاختباري: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- مسبار ملخص + فحص الارتباط: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- سجل تغيير الوكيل جربه: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- صادرات القياس: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- قياس حزمة يومية لساعات المكتب: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- مؤلفات الدراسة للبيان: ضع مجلدات كل باحث تحت
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- اختبار المعرفة وملخصه CSV:`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

حافظ على الجرد متزامنا مع تذكرة التتبع. ارفق الهاشات عند نسخة الاثار الى تذكرة التورم
حتى يستمر المنتهى في التحقق من الملفات دون وصول للصدفة.