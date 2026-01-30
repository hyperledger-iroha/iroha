---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/summary.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w1-summary
title: ملخص ملاحظات وخروج W1
sidebar_label: ملخص W1
description: النتائج، الاجراءات، وادلة الخروج لموجة معاينة الشركاء ومتكاملي Torii.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W1 - الشركاء ومتكاملو Torii |
| نافذة الدعوة | 2025-04-12 -> 2025-04-26 |
| وسم الاثر | `preview-2025-04-12` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W1` |
| المشاركون | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## ابرز النقاط

1. **سير عمل checksum** - تحقق جميع المراجعين من descriptor/archive عبر `scripts/preview_verify.sh`; تم حفظ السجلات بجانب اقرارات الدعوة.
2. **القياس** - بقيت لوحات `docs.preview.integrity`, `TryItProxyErrors`, و `DocsPortal/GatewayRefusals` خضراء طوال الموجة؛ لم تقع حوادث او صفحات تنبيه.
3. **ملاحظات الوثائق (`docs-preview/w1`)** - تم تسجيل ملاحظتين بسيطتين:
   - `docs-preview/w1 #1`: توضيح صياغة التنقل في قسم Try it (تم الحل).
   - `docs-preview/w1 #2`: تحديث لقطة Try it (تم الحل).
4. **تكافؤ runbook** - اكد مشغلو SoraFS ان الروابط المتقاطعة الجديدة بين `orchestrator-ops` و `multi-source-rollout` عالجت ملاحظات W0.

## بنود العمل

| المعرف | الوصف | المالك | الحالة |
| --- | --- | --- | --- |
| W1-A1 | تحديث صياغة تنقل Try it حسب `docs-preview/w1 #1`. | Docs-core-02 | ✅ مكتمل (2025-04-18). |
| W1-A2 | تحديث لقطة Try it حسب `docs-preview/w1 #2`. | Docs-core-03 | ✅ مكتمل (2025-04-19). |
| W1-A3 | تلخيص نتائج الشركاء وادلة القياس في roadmap/status. | Docs/DevRel lead | ✅ مكتمل (راجع المتتبع و status.md). |

## ملخص الخروج (2025-04-26)

- اكد جميع المراجعين الثمانية الاكتمال خلال ساعات المكتب الاخيرة، ونظفوا الاثار المحلية، وتم سحب صلاحياتهم.
- بقيت القياسات خضراء حتى الخروج؛ اللقطات النهائية مرفقة بـ `DOCS-SORA-Preview-W1`.
- تم تحديث سجل الدعوات باقرارات الخروج؛ حول المتتبع W1 الى 🈴 واضاف نقاط التحقق.
- حزمة الادلة (descriptor، سجل checksum، مخرجات probe، نص وكيل Try it، لقطات القياس، ملخص الملاحظات) ارشفت تحت `artifacts/docs_preview/W1/`.

## الخطوات التالية

- تجهيز خطة intake المجتمعية لـ W2 (موافقة الحوكمة + تعديلات قالب الطلب).
- تحديث وسم اثر المعاينة لموجة W2 واعادة تشغيل سكربت preflight بمجرد تثبيت التواريخ.
- نقل النتائج المناسبة من W1 الى roadmap/status حتى تحصل الموجة المجتمعية على اخر التوجيهات.
