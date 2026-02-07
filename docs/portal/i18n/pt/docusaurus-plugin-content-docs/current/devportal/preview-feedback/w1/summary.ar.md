---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-resumo
título: ملخص ملاحظات وخروج W1
sidebar_label: Nome W1
description: Descrição do arquivo وادلة الخروج لموجة معاينة الشركاء ومتكاملي Torii.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W1 - Conjunto de ferramentas Torii |
| نافذة الدعوة | 12/04/2025 -> 26/04/2025 |
| وسم الاثر | `preview-2025-04-12` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W1` |
| المشاركون | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## ابرز النقاط

1. **سير عمل checksum** - تحقق جميع المراجعين من descritor/archive عبر `scripts/preview_verify.sh`; Isso é o que você precisa para fazer isso.
2. **القياس** - بقيت لوحات `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` خضراء طوال الموجة؛ Não há necessidade de fazer isso e de fazer isso.
3. **ملاحظات الوثائق (`docs-preview/w1`)** - تم تسجيل ملاحظتين بسيطتين:
   - `docs-preview/w1 #1`: توضيح صياغة التنقل في قسم Try it (تم الحل).
   - `docs-preview/w1 #2`: تحديث لقطة Try it (تم الحل).
4. **runbook de execução** - Use o SoraFS e o `orchestrator-ops` e `multi-source-rollout` عالجت ملاحظات W0.

## بنود العمل

| المعرف | الوصف | المالك | الحالة |
| --- | --- | --- | --- |
| W1-A1 | تحديث صياغة تنقل Experimente حسب `docs-preview/w1 #1`. | Documentos-núcleo-02 | ✅ مكتمل (18/04/2025). |
| W1-A2 | Tente tentar em `docs-preview/w1 #2`. | Documentos-núcleo-03 | ✅ مكتمل (19/04/2025). |
| W1-A3 | Selecione o mapa e o status do mapa. | Líder do Documentos/DevRel | ✅ مكتمل (راجع المتتبع e status.md). |

## ملخص الخروج (2025/04/26)

- اكد جميع المراجعين الثمانية الاكتمال خلال ساعات المكتب الاخيرة, ونظفوا الاثار المحلية, وتم سحب صلاحياتهم.
- بقيت القياسات خضراء حتى الخروج؛ O nome do produto é `DOCS-SORA-Preview-W1`.
- تم تحديث سجل الدعوات باقرارات الخروج؛ حول المتتبع W1 الى 🈴 واضاف نقاط التحقق.
- حزمة الادلة (descritor, سجل checksum, مخرجات probe, نص وكيل Try it, لقطات القياس, ملخص الملاحظات) ارشفت تحت `artifacts/docs_preview/W1/`.

## الخطوات التالية

- تجهيز خطة entrada المجتمعية لـ W2 (موافقة الحوكمة + تعديلات قالب الطلب).
- تحديث وسم اثر المعاينة لموجة W2 واعادة تشغيل سكربت comprovação بمجرد تثبيت التواريخ.
- نقل النتائج المناسبة من W1 الى roadmap/status حتى تحصل الموجة المجتمعية على اخر التوجيهات.