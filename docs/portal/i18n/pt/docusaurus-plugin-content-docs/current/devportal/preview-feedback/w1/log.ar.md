---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-log
título: سجل الملاحظات والقياس W1
sidebar_label: W1
description: قائمة مجمعة, نقاط قياس, وملاحظات المراجعين لموجة معاينة الشركاء الاولى.
---

يحفظ هذا السجل قائمة الدعوات ونقاط القياس وملاحظات المراجعين لموجة **معاينة الشركاء W1**
Faça o download do arquivo [`preview-feedback/w1/plan.md`](./plan.md) e faça o download do arquivo no site
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). حدثه عند ارسال دعوة,
او تسجيل لقطة قياس, او تصنيف بند ملاحظات حتى يتمكن مراجعو الحوكمة من اعادة تشغيل
Não há nada que você possa fazer.

## قائمة الدفعة

| معرف الشريك | تذكرة الطلب | NDA | Horário de Brasília (UTC) | Hora/Hora (UTC) | الحالة | Produtos |
| --- | --- | --- | --- | --- | --- | --- |
| parceiro-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 03/04/2025 | 12/04/2025 15:00 | 12/04/2025 15:11 | ✅ مكتمل 2025/04/26 | sorafs-op-01; ركز على ادلة تكافؤ وثائق orquestrador. |
| parceiro-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 03/04/2025 | 12/04/2025 15:03 | 12/04/2025 15:15 | ✅ مكتمل 2025/04/26 | sorafs-op-02; Você pode usar o Norito/telemetria. |
| parceiro-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 04/04/2025 | 12/04/2025 15:06 | 12/04/2025 15:18 | ✅ مكتمل 2025/04/26 | sorafs-op-03; Não há failover como resultado. |
| parceiro-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 04/04/2025 | 12/04/2025 15:09 | 12/04/2025 15:21 | ✅ مكتمل 2025/04/26 | torii-int-01; مراجعة دليل Torii `/v1/pipeline` e livro de receitas Experimente. |
| parceiro-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 05/04/2025 | 12/04/2025 15:12 | 12/04/2025 15:23 | ✅ مكتمل 2025/04/26 | torii-int-02; شارك في تحديث لقطة Experimente (docs-preview/w1 #2). |
| parceiro-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 05/04/2025 | 12/04/2025 15:15 | 12/04/2025 15:26 | ✅ مكتمل 2025/04/26 | SDK-parceiro-01; Baixar livros de receitas para JS/Swift + sanity para ISO. |
| parceiro-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 11/04/2025 | 12/04/2025 15:18 | 12/04/2025 15:29 | ✅ مكتمل 2025/04/26 | SDK-parceiro-02; تم انهاء الامتثال 2025-04-11, ركز على ملاحظات Conectar/telemetria. |
| parceiro-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 11/04/2025 | 12/04/2025 15:21 | 12/04/2025 15:33 | ✅ مكتمل 2025/04/26 | gateway-ops-01; دقق دليل عمليات gateway + proxy proxy Experimente. |

املأ تواريخ **ارسال الدعوة** e **الاقرار** é uma opção de compra.
A partida foi realizada no UTC no dia W1.

## نقاط القياس

| الطابع الزمني (UTC) | Sondas / sondas | المالك | النتيجة | الاثر |
| --- | --- | --- | --- | --- |
| 06/04/2025 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operações | ✅ كلها خضراء | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06/04/2025 18:20 | Para `npm run manage:tryit-proxy -- --stage preview-w1` | Operações | ✅ تم التجهيز | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 12/04/2025 14:45 | Chave de fenda + `probe:portal` | Documentos/DevRel + Operações | ✅ لقطة قبل الدعوة, بلا تراجعات | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | اللوحات اعلاه + فرق زمن Experimente | Líder do Documentos/DevRel | ✅ اجتاز فحص منتصف الموجة (0 تنبيهات; زمن Experimente p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | Sonda + sonda Sonda | Contato do Docs/DevRel + Governança | ✅ لقطة خروج, صفر تنبيهات متبقية | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

عينات ساعات المكتب اليومية (2025-04-13 -> 2025-04-25) مجمعة كصادرات NDJSON + PNG تحت
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` é um dispositivo de armazenamento
`docs-preview-integrity-<date>.json` é um problema.

## سجل الملاحظات والتذاكر

Você pode fazer isso com antecedência. اربط كل بند بتذكرة GitHub/discuss
بالاضافة الى النموذج المهيكل الملتقط عبر
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| المرجع | الشدة | المالك | الحالة | Produtos |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Baixo | Documentos-núcleo-02 | ✅ تم الحل 2025-04-18 | تم توضيح صياغة تنقل Try it + مرساة الشريط الجانبي (تم تحديث `docs/source/sorafs/tryit.md` بالوسم الجديد). |
| `docs-preview/w1 #2` | Baixo | Documentos-núcleo-03 | ✅ تم الحل 2025-04-19 | تم تحديث لقطة Experimente + التسمية حسب طلب المراجع؛ Use `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| — | Informações | Líder do Documentos/DevRel | 🟢 مغلق | كانت التعليقات المتبقية اسئلة/اجابات فقط؛ Verifique se o produto está no `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## تتبع اختبار المعرفة والاستبيان

1. Faça uma pausa (>=90%) para obter mais informações. E o arquivo CSV está disponível para download.
2. اجمع اجابات الاستبيان النوعية الملتقطة عبر نموذج الملاحظات وكررها تحت
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Verifique o valor do produto no final do processo.

سجل جميع المراجعين الثمانية >=94% em اختبار المعرفة (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). لم تكن هناك مكالمات
معالجة مطلوبة؛ صادرات الاستبيان لكل شريك محفوظة تحت
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## جرد الاثار- Descritor de visualização/soma de verificação: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
Sonda de teste + verificação de link: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- سجل تغيير proxy Experimente: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Nome do usuário: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- حزمة قياس يومية لساعات المكتب: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- صادرات الملاحظات والاستبيان: ضع مجلدات كل مراجع تحت
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Arquivo CSV e arquivo: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Não deixe de usar o produto. ارفق الهاشات عند نسخ الاثار الى تذكرة الحوكمة
Você pode fazer isso com o dinheiro e o dinheiro.