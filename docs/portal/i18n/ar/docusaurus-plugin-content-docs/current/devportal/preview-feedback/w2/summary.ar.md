---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/summary.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w2-summary
title: ملخص ملاحظات وحالة W2
sidebar_label: ملخص W2
description: ملخص حي لموجة المعاينة المجتمعية (W2).
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W2 - المراجعون المجتمعيون |
| نافذة الدعوة | 2025-06-15 -> 2025-06-29 |
| وسم الاثر | `preview-2025-06-15` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W2` |
| المشاركون | comm-vol-01...comm-vol-08 |

## ابرز النقاط

1. **الحوكمة والادوات** - تمت الموافقة بالاجماع على سياسة intake المجتمعية في 2025-05-20; قالب الطلب المحدث مع حقول الدافع/المنطقة الزمنية موجود تحت `docs/examples/docs_preview_request_template.md`.
2. **ادلة preflight** - تم تنفيذ تغيير وكيل Try it `OPS-TRYIT-188` في 2025-06-09، تم التقاط لوحات Grafana، وارشفة مخرجات descriptor/checksum/probe لـ `preview-2025-06-15` تحت `artifacts/docs_preview/W2/`.
3. **موجة الدعوات** - تمت دعوة ثمانية مراجعين مجتمعيين في 2025-06-15، مع تسجيل الاقرارات في جدول الدعوات بالمتتبع; اكمل الجميع تحقق checksum قبل التصفح.
4. **الملاحظات** - تم تسجيل `docs-preview/w2 #1` (صياغة tooltip) و `#2` (ترتيب الشريط الجانبي للترجمة) في 2025-06-18 وحلهما بحلول 2025-06-21 (Docs-core-04/05)؛ لم تقع حوادث خلال الموجة.

## بنود العمل

| المعرف | الوصف | المالك | الحالة |
| --- | --- | --- | --- |
| W2-A1 | معالجة `docs-preview/w2 #1` (صياغة tooltip). | Docs-core-04 | ✅ مكتمل (2025-06-21). |
| W2-A2 | معالجة `docs-preview/w2 #2` (ترتيب الشريط الجانبي للترجمة). | Docs-core-05 | ✅ مكتمل (2025-06-21). |
| W2-A3 | ارشفة ادلة الخروج + تحديث roadmap/status. | Docs/DevRel lead | ✅ مكتمل (2025-06-29). |

## ملخص الخروج (2025-06-29)

- اكد جميع المراجعين المجتمعيين الثمانية الاكتمال وتم سحب صلاحيات المعاينة; تم تسجيل الاقرارات في سجل الدعوات بالمتتبع.
- بقيت لقطات القياس النهائية (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) خضراء; اللوجات ونصوص وكيل Try it مرفقة بـ `DOCS-SORA-Preview-W2`.
- حزمة الادلة (descriptor, checksum log, probe output, link report, لقطات Grafana, اقرارات الدعوة) ارشفت تحت `artifacts/docs_preview/W2/preview-2025-06-15/`.
- تم تحديث سجل نقاط التحقق W2 في المتتبع حتى الخروج، لضمان سجل قابل للتدقيق قبل بدء تخطيط W3.
