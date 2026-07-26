---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w2-resumo
título: ملخص ملاحظات وحالة W2
sidebar_label: Nome W2
descrição: ملخص حي لموجة المعاينة المجتمعية (W2).
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W2 - Nome do usuário |
| نافذة الدعوة | 15/06/2025 -> 29/06/2025 |
| وسم الاثر | `preview-2025-06-15` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W2` |
| المشاركون | comm-vol-01...comm-vol-08 |

## ابرز النقاط

1. **الحوكمة والادوات** - تمت الموافقة بالاجماع على سياسة ingestão de alimentos em 20/05/2025; قالب الطلب المحدث مع حقول الدافع/المنطقة الزمنية موجود تحت `docs/examples/docs_preview_request_template.md`.
2. **Preflight** - تم تنفيذ تغيير وكيل Try it `OPS-TRYIT-188` em 2025-06-09, تم التقاط لوحات Grafana, Use o descritor/checksum/sonda para `preview-2025-06-15` em vez de `artifacts/docs_preview/W2/`.
3. **موجة الدعوات** - تمت دعوة ثمانية مراجعين مجتمعيين في 2025-06-15, مع تسجيل الاقرارات في جدول الدعوات بالمتتبع; A soma de verificação é definida como uma soma de verificação.
4. **الملاحظات** - تم تسجيل `docs-preview/w2 #1` (dica de ferramenta) e `#2` (ترتيب الشريط الجانبي للترجمة) aqui 2025-06-18 e بحلول 2025-06-21 (Docs-core-04/05)؛ Não importa o que aconteça.

## بنود العمل

| المعرف | الوصف | المالك | الحالة |
| --- | --- | --- | --- |
| W2-A1 | Use `docs-preview/w2 #1` (dica de ferramenta). | Documentos-núcleo-04 | ✅ مكتمل (2025-06-21). |
| W2-A2 | Selecione `docs-preview/w2 #2` (é necessário usar o recurso). | Documentos-núcleo-05 | ✅ مكتمل (2025-06-21). |
| W2-A3 | ارشفة ادلة الخروج + تحديث roteiro/status. | Líder do Documentos/DevRel | ✅ مكتمل (29/06/2025). |

## ملخص الخروج (29/06/2025)

- اكد جميع المراجعين المجتمعيين الثمانية الاكتمال وتم سحب صلاحيات المعاينة; Não desmonte o produto em qualquer lugar do mundo.
- بقيت لقطات القياس النهائية (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) خضراء; Tente isso e tente isso em `DOCS-SORA-Preview-W2`.
- حزمة الادلة (descritor, log de soma de verificação, saída de sonda, relatório de link, لقطات Grafana, اقرارات الدعوة) ارشفت تحت `artifacts/docs_preview/W2/preview-2025-06-15/`.
- تم تحديث سجل نقاط التحقق W2 في المتتبع حتى الخروج, لضمان سجل قابل للتدقيق قبل بدء تخطيط W3.