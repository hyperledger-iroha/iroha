---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-plan
título: خطة entrada المجتمعية W2
sidebar_label: Nome W2
description: القبول والموافقات وقائمة ادلة لمجموعة معاينة المجتمع.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W2 - مراجعة المجتمع |
| نافذة الهدف | الربع الثالث 2025 الاسبوع 1 (مبدئي) |
| وسم الاثر (مخطط) | `preview-2025-06-15` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W2` |

## الاهداف

1. Verifique a ingestão de alimentos e a ingestão de alimentos.
2. الحصول على موافقة الحوكمة على القائمة المقترحة وملحق الاستخدام المقبول.
3. Verifique a soma de verificação e verifique a soma de verificação.
4. Experimente e experimente.

## تفصيل المهام

| المعرف | المهمة | المالك | الاستحقاق | الحالة | Produtos |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Ingestão de consumo (CoC) e CoC | Líder do Documentos/DevRel | 15/05/2025 | ✅ مكتمل | A ingestão de água foi tomada em `DOCS-SORA-Preview-W2` e foi lançada em 20/05/2025. |
| W2-P2 | Produtos de higiene pessoal (serviços de limpeza) | Documentos-núcleo-01 | 18/05/2025 | ✅ مكتمل | ملف `docs/examples/docs_preview_request_template.md` يتضمن الان قسم Community, ومشار اليه في نموذج ingestão. |
| W2-P3 | تأمين موافقة الحوكمة على خطة ingestão (تصويت اجتماع + محاضر مسجلة) | Ligação para governação | 22/05/2025 | ✅ مكتمل | Ele foi lançado em 20/05/2025; Você pode usar o produto em `DOCS-SORA-Preview-W2`. |
| W2-P4 | جدولة staging لوكيل Try it والتقاط القياس لنافذة W2 (`preview-2025-06-15`) | Documentos/DevRel + Operações | 05/06/2025 | ✅ مكتمل | A atualização `OPS-TRYIT-188` foi concluída em 2025-06-09 02:00-04:00 UTC؛ O Grafana está danificado. |
| W2-P5 | بناء/التحقق من وسم اثر المعاينة الجديد (`preview-2025-06-15`) وارشفة سجلات descritor/checksum/probe | PortalTL | 07/06/2025 | ✅ مكتمل | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` foi removido em 10/06/2025; O número de telefone é `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | تجميع قائمة دعوات المجتمع (<=25 مراجع, دفعات مرحلية) بمعلومات اتصال معتمدة من الحوكمة | Gerente de comunidade | 10/06/2025 | ✅ مكتمل | تمت الموافقة على الدفعة الاولى من 8 مراجعين مجتمعيين؛ A chave `DOCS-SORA-Preview-REQ-C01...C08` está disponível no site. |

## قائمة ادلة الاثبات

- [x] سجل موافقة الحوكمة (محاضر الاجتماع + رابط التصويت) مرفق بـ `DOCS-SORA-Preview-W2`.
- [x] قالب الطلب المحدث مثبت تحت `docs/examples/`.
- [x] descritor لاصدار `preview-2025-06-15` e checksum ومخرجات probe وتقرير الروابط ونص وكيل Try it محفوظة تحت `artifacts/docs_preview/W2/`.
- [x] لقطات Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) ملتقطة لنافذة preflight em W2.
- [x] جدول lista للدعوات مع معرفات المراجعين وتذاكر الطلب وتواريخ الموافقة معبأة قبل الارسال (راجع no W2 no local).

حافظ على تحديث هذه الخطة؛ Verifique se o DOCS-SORA é compatível com o W2.