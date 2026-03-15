---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/log.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-log
title: سجل الملاحظات والقياس W1
sidebar_label: סיד W1
description: قائمة مجمعة، نقاط قياس، وملاحظات المراجعين لموجة معاينة الشركاء الاولى.
---

يحفظ هذا السجل قائمة الدعوات ونقاط القياس وملاحظات المراجعين لموجة **معاينة الشركاء W1**
المرافقة لمهام القبول في [`preview-feedback/w1/plan.md`](./plan.md) ومدخل متتبع الموجة في
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). حدثه عند ارسال دعوة،
او تسجيل لقطة قياس، او تصنيف بند ملاحظات حتى يتمكن مراجعو الحوكمة من اعادة تشغيل
الادلة دون ملاحقة تذاكر خارجية.

## قائمة الدفعة

| معرف الشريك | تذكرة الطلب | استلام NDA | ارسال الدعوة (UTC) | اقرار/اول دخول (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ חודש 26-04-2025 | soraps-op-01; ركز على ادلة تكافؤ وثائق orchestrator. |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 12-04-2025 15:03 | 12-04-2025 15:15 | ✅ חודש 26-04-2025 | soraps-op-02; تحقق من الروابط المتقاطعة بين Norito/telemetry. |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ חודש 26-04-2025 | soraps-op-03; نفذ تمارين failover متعددة المصادر. |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 12-04-2025 15:09 | 2025-04-12 15:21 | ✅ חודש 26-04-2025 | torii-int-01; مراجعة دليل Torii `/v2/pipeline` و cookbook Try it. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 12-04-2025 15:12 | 2025-04-12 15:23 | ✅ חודש 26-04-2025 | torii-int-02; شارك في تحديث لقطة Try it (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 12-04-2025 15:15 | 2025-04-12 15:26 | ✅ חודש 26-04-2025 | sdk-partner-01; ملاحظات cookbooks لـ JS/Swift + فحوصات sanity لجسر ISO. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ חודש 26-04-2025 | sdk-partner-02; تم انهاء الامتثال 2025-04-11، ركز على ملاحظات Connect/telemetry. |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ חודש 26-04-2025 | gateway-ops-01; دقق دليل عمليات gateway + مسار proxy Try it المجهول. |

املأ تواريخ **ارسال الدعوة** و **الاقرار** فور اصدار البريد الصادر.
اربط الاوقات بجدول UTC المحدد في خطة W1.

## نقاط القياس

| الطابع الزمني (UTC) | لوحات / probes | المالك | النتيجة | الاثر |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ كلها خضراء | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06-04-2025 18:20 | נה `npm run manage:tryit-proxy -- --stage preview-w1` | אופס | ✅ تم التجهيز | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | اللوحات اعلاه + `probe:portal` | Docs/DevRel + Ops | ✅ لقطة قبل الدعوة، بلا تراجعات | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | اللوحات اعلاه + فرق زمن Try it | Docs/DevRel lead | ✅ اجتاز فحص منتصف الموجة (0 تنبيهات; زمن Try it p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | اللوحات اعلاه + probe خروج | Docs/DevRel + קשר ממשל | ✅ لقطة خروج، صفر تنبيهات متبقية | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |عينات ساعات المكتب اليومية (2025-04-13 -> 2025-04-25) مجمعة كصادرات NDJSON + PNG تحت
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` مع اسماء ملفات
`docs-preview-integrity-<date>.json` واللقطات المقابلة.

## سجل الملاحظات والتذاكر

استخدم هذا الجدول لتلخيص الملاحظات المقدمة من المراجعين. اربط كل بند بتذكرة GitHub/discuss
بالاضافة الى النموذج المهيكل الملتقط عبر
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| المرجع | الشدة | المالك | الحالة | ملاحظات |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | נמוך | Docs-core-02 | ✅ تم الحل 2025-04-18 | تم توضيح صياغة تنقل Try it + مرساة الشريط الجانبي (تم تحديث `docs/source/sorafs/tryit.md` بالوسم الجديد). |
| `docs-preview/w1 #2` | נמוך | Docs-core-03 | ✅ تم الحل 2025-04-19 | تم تحديث لقطة Try it + التسمية حسب طلب المراجع؛ גודל `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| — | מידע | Docs/DevRel lead | 🟢 مغلق | كانت التعليقات المتبقية اسئلة/اجابات فقط؛ تم التقاطها في نموذج ملاحظات كل شريك تحت `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## تتبع اختبار المعرفة والاستبيان

1. سجل درجات الاختبار (الهدف >=90%) لكل مراجع؛ وارفق ملف CSV المصدر بجانب اثار الدعوة.
2. اجمع اجابات الاستبيان النوعية الملتقطة عبر نموذج الملاحظات وكررها تحت
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. جدولة مكالمات المعالجة لمن يقل عن الحد وادونها في هذا الملف.

سجل جميع المراجعين الثمانية >=94% في اختبار المعرفة (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). لم تكن هناك مكالمات
معالجة مطلوبة؛ صادرات الاستبيان لكل شريك محفوظة تحت
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## جرد الاثار

- حزمة preview descriptor/checksum: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- בדיקה של מערכת + בדיקת קישור: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- سجل تغيير proxy Try it: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- صادرات القياس: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- حزمة قياس يومية لساعات المكتب: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- صادرات الملاحظات والاستبيان: ضع مجلدات كل مراجع تحت
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV اختبار المعرفة وملخصه: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

حافظ على الجرد متزامنا مع تذكرة المتتبع. ارفق الهاشات عند نسخ الاثار الى تذكرة الحوكمة
حتى يتمكن المدققون من التحقق من الملفات دون وصول للصدفة.