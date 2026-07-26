---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w2-plan
título: خطة ingesta المجتمعية W2
sidebar_label: خطة W2
descripción: القبول والموافقات وقائمة ادلة لمجموعة معاينة المجتمع.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W2 - مراجعة المجتمع |
| نافذة الهدف | الربع الثالث 2025 الاسبوع 1 (مبدئي) |
| وسم الاثر (مخطط) | `preview-2025-06-15` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W2` |

## الاهداف

1. تعريف معايير ingesta المجتمعية وسير عمل التقييم.
2. الحصول على موافقة الحوكمة على القائمة المقترحة وملحق الاستخدام المقبول.
3. تحديث اثر المعاينة المتحقق بالـ checksum وحزمة القياس للنافذة الجديدة.
4. تجهيز وكيل Pruébelo واللوحات قبل ارسال الدعوات.

## تفصيل المهام| المعرف | المهمة | المالك | الاستحقاق | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- |
| G2-P1 | صياغة معايير ingesta المجتمعية (الاهلية، الحد الاقصى، متطلبات CoC) y توزيعها على الحوكمة | Líder de Docs/DevRel | 2025-05-15 | ✅ مكتمل | تم دمج سياسة admisión في `DOCS-SORA-Preview-W2` واعتمادها في اجتماع المجلس 2025-05-20. |
| W2-P2 | تحديث قالب الطلب باسئلة خاصة بالمجتمع (الدوافع، الاتاحة، احتياجات التوطين) | Documentos-core-01 | 2025-05-18 | ✅ مكتمل | ملف `docs/examples/docs_preview_request_template.md` يتضمن الان قسم Community, ومشار اليه في نموذج ingesta. |
| W2-P3 | تأمين موافقة الحوكمة على خطة ingesta (تصويت اجتماع + محاضر مسجلة) | Enlace de gobernanza | 2025-05-22 | ✅ مكتمل | تم تمرير التصويت بالاجماع في 2025-05-20؛ La configuración y la configuración se realizan según `DOCS-SORA-Preview-W2`. |
| W2-P4 | جدولة puesta en escena لوكيل Pruébalo والتقاط القياس لنافذة W2 (`preview-2025-06-15`) | Documentos/DevRel + Operaciones | 2025-06-05 | ✅ مكتمل | تذكرة التغيير `OPS-TRYIT-188` تمت الموافقة عليها وتنفيذها في 2025-06-09 02:00-04:00 UTC؛ لقطات Grafana ارشفت مع التذكرة. |
| W2-P5 | بناء/التحقق من وسم اثر المعاينة الجديد (`preview-2025-06-15`) وارشفة سجلات descriptor/suma de comprobación/sonda | Portal TL | 2025-06-07 | ✅ مكتمل | Actualizado `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` el 2025-06-10 La configuración del equipo es `artifacts/docs_preview/W2/preview-2025-06-15/`. || W2-P6 | تجميع قائمة دعوات المجتمع (<=25 مراجع، دفعات مرحلية) بمعلومات اتصال معتمدة من الحوكمة | Responsable de la comunidad | 2025-06-10 | ✅ مكتمل | تمت الموافقة على الدفعة الاولى من 8 مراجعين مجتمعيين؛ معرفات الطلب `DOCS-SORA-Preview-REQ-C01...C08` مسجلة في المتتبع. |

## قائمة ادلة الاثبات

- [x] سجل موافقة الحوكمة (محاضر الاجتماع + رابط التصويت) مرفق بـ `DOCS-SORA-Preview-W2`.
- [x] قالب الطلب المحدث مثبت تحت `docs/examples/`.
- [x] descriptor لاصدار `preview-2025-06-15` وسجل checksum ومخرجات sonda وتقرير الروابط ونص وكيل Pruébelo محفوظة تحت `artifacts/docs_preview/W2/`.
- [x] Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) Verificación previa de la verificación previa de W2.
- [x] جدول roster للدعوات مع معرفات المراجعين وتذاكر الطلب وتواريخ الموافقة معبأة قبل الارسال (راجع قسم W2 في المتتبع).

حافظ على تحديث هذه الخطة؛ Esta es la versión DOCS-SORA del software W2.