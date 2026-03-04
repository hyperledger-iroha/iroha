---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w1-resumen
título: ملخص ملاحظات وخروج W1
sidebar_label: ملخص W1
descripción: النتائج، الاجراءات، وادلة الخروج لموجة معاينة الشركاء ومتكاملي Torii.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W1 - Contenido y contenido Torii |
| نافذة الدعوة | 2025-04-12 -> 2025-04-26 |
| وسم الاثر | `preview-2025-04-12` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W1` |
| المشاركون | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## ابرز النقاط

1. **سير عمل checksum** - تحقق جميع المراجعين من descriptor/archive عبر `scripts/preview_verify.sh`; تم حفظ السجلات بجانب اقرارات الدعوة.
2. **القياس** - Los códigos `docs.preview.integrity`, `TryItProxyErrors` y `DocsPortal/GatewayRefusals` son compatibles con el sistema. لم تقع حوادث او صفحات تنبيه.
3. **ملاحظات الوثائق (`docs-preview/w1`)** - تم تسجيل ملاحظتين بسيطتين:
   - `docs-preview/w1 #1`: توضيح صياغة التنقل في قسم Pruébalo (تم الحل).
   - `docs-preview/w1 #2`: تحديث لقطة Pruébalo (تم الحل).
4. **تكافؤ runbook** - Utilice SoraFS y utilice `orchestrator-ops` e `multi-source-rollout`. ملاحظات W0.

## بنود العمل| المعرف | الوصف | المالك | الحالة |
| --- | --- | --- | --- |
| W1-A1 | تحديث صياغة تنقل Pruébalo حسب `docs-preview/w1 #1`. | Documentos-core-02 | ✅ مكتمل (2025-04-18). |
| W1-A2 | تحديث لقطة Pruébelo حسب `docs-preview/w1 #2`. | Documentos-core-03 | ✅ مكتمل (2025-04-19). |
| W1-A3 | تلخيص نتائج الشركاء وادلة القياس في hoja de ruta/estado. | Líder de Docs/DevRel | ✅ مكتمل (راجع المتتبع y status.md). |

## ملخص الخروج (2025-04-26)

- اكد جميع المراجعين الثمانية الاكتمال خلال ساعات المكتب الاخيرة, ونظفوا الاثار المحلية, وتم سحب صلاحياتهم.
- بقيت القياسات خضراء حتى الخروج؛ La configuración del equipo es `DOCS-SORA-Preview-W1`.
- تم تحديث سجل الدعوات باقرارات الخروج؛ حول المتتبع W1 الى 🈴 واضاف نقاط التحقق.
- حزمة الادلة (descriptor, سجل checksum, مخرجات probe, نص وكيل Pruébelo, لقطات القياس، ملخص الملاحظات) ارشفت تحت `artifacts/docs_preview/W1/`.

## الخطوات التالية

- تجهيز خطة ingesta المجتمعية لـ W2 (موافقة الحوكمة + تعديلات قالب الطلب).
- تحديث وسم اثر المعاينة لموجة W2 واعادة تشغيل سكربت preflight بمجرد تثبيت التواريخ.
- نقل النتائج المناسبة من W1 الى roadmap/status حتى تحصل الموجة المجتمعية على اخر التوجيهات.