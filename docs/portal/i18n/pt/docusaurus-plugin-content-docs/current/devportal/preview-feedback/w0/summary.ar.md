---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w0-resumo
título: ملخص ملاحظات منتصف W0
sidebar_label: Nome W0 (Relatório)
description: نقاط تحقق منتصف المرحلة, النتائج, وبنود العمل لموجة المعاينة لمشرفي النواة.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W0 - مشرفو النواة |
| تاريخ الملخص | 27/03/2025 |
| نافذة المراجعة | 25/03/2025 -> 08/04/2025 |
| المشاركون | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilidade-01 |
| وسم الاثر | `preview-2025-03-24` |

## ابرز النقاط

1. **سير عمل checksum** - اكد جميع المراجعين ان `scripts/preview_verify.sh`
   Não há nenhum problema/alteração. Não se preocupe ou não.
2. **ملاحظات التنقل** - تم تسجيل مشكلتين بسيطتين em ترتيب الشريط الجانبي
   (`docs-preview/w0 #1-#2`). Baixe o arquivo Docs/DevRel e instale-o
   sim.
3. **runbook de execução em SoraFS** - طلب sorafs-ops-01 روابط متقاطعة اوضح بين
   `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. تم فتح مشكلة
   متابعة؛ Desligue o W1.
4. **مراجعة القياس** - Observabilidade-01 em `docs.preview.integrity`,
   `TryItProxyErrors` Teste e Try-it Não é possível.

## بنود العمل

| المعرف | الوصف | المالك | الحالة |
| --- | --- | --- | --- |
| W0-A1 | Use o devportal para acessar o site do devportal (`preview-invite-*`). | Documentos-núcleo-01 | مكتمل - الشريط الجانبي يعرض الان وثائق المراجعين بشكل متجاور (`docs/portal/sidebars.js`). |
| W0-A2 | Você pode usar o `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. | Sorafs-ops-01 | مكتمل - كل runbook يشير الان الى الاخر حتى يرى المشغلون الدليلين اثناء عمليات rollout. |
| W0-A3 | مشاركة لقطات القياس + حزمة الاستعلامات مع متتبع الحوكمة. | Observabilidade-01 | مكتمل - الحزمة مرفقة بـ `DOCS-SORA-Preview-W0`. |

## ملخص الختام (08/04/2025)

- اكد جميع المراجعين الخمسة الاكتمال, ونظفوا البناءات المحلية, وغادروا نافذة
  المعاينة؛ Verifique o valor do arquivo em `DOCS-SORA-Preview-W0`.
- لم تقع حوادث او تنبيهات خلال الموجة؛ Verifique se há algum problema com isso.
- تم تنفيذ اجراءات التنقل + الروابط المتقاطعة (W0-A1/A2) وعكسها في الوثائق
  علاه؛ وادلة القياس (W0-A3) مرفقة بالمتتبع.
- تم ارشفة حزمة الادلة: لقطات قياس الشاشة, تاكيدات الدعوة, وهذا الملخص مرتبط
  Isso é tudo.

## الخطوات التالية

- تنفيذ بنود عمل W0 para W1.
- الحصول على موافقة قانونية وحجز slot للـ staging الخاص بالوكيل, ثم اتباع خطوات
  O fluxo de convite de pré-visualização está disponível em [visualizar fluxo de convite](../../preview-invite-flow.md).

_هذا الملخص مرتبط من [rastreador de convite de visualização](../../preview-invite-tracker.md) por aqui
O código de segurança do documento DOCS-SORA._