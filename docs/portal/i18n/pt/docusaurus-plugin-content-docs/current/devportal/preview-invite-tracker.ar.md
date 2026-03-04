---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# متعقب دعوات المعاينة

Faça o download do arquivo do produto DOCS-SORA e الحوكمة من رؤية اي مجموعة نشطة, من وافق على الدعوات, واي اثار ما زالت بحاجة الى متابعة. Você pode fazer isso sem parar e sem usar o produto.

## حالة الموجات

| الموجة | المجموعة | تذكرة المتابعة | الموافقون | الحالة | النافذة المستهدفة | Produtos |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principais** | Mantenedores do Docs + SDK não usam checksum | `DOCS-SORA-Preview-W0` (GitHub/ops) | Documentos principais/DevRel + Portal TL | مكتمل | 2º trimestre de 2025 1-2 | الدعوات ارسلت 2025-03-25, القياس عن بعد بقي اخضر, ملخص الخروج نشر 2025-04-08. |
| **W1 - Parceiros** | O documento SoraFS e o Torii são NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + رابط الحوكمة | مكتمل | 2º trimestre de 2025 Dia 3 | الدعوات 2025-04-12 -> 2025-04-26 مع تأكيد جميع الشركاء الثمانية؛ Você pode usar o [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) e usar o [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comunidade** | قائمة انتظار مجتمعية منتقاة (<=25 por mês) | `DOCS-SORA-Preview-W2` | Líder de Documentos/DevRel + gerente de comunidade | مكتمل | 3º trimestre de 2025 Dia 1 (Mesmo) | الدعوات 2025-06-15 -> 2025-06-29 مع قياس عن بعد اخضر طوال الفترة؛ A opção é [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Coortes beta** | Versão Beta / Software + SDK + Software de configuração | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + رابط الحوكمة | مكتمل | 1º trimestre de 2026 8 | دعوات 2026-02-18 -> 2026-02-28؛ Você pode usar o `preview-20260218` (`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> ملاحظة: اربط كل تذكرة في المتعقب بطلبات المعاينة وارشفها تحت مشروع `docs-portal-preview` حتى Verifique se há algum problema com isso.

## المهام النشطة (W0)

- تحديث اثار preflight (GitHub Actions `docs-portal-preview` بتاريخ 2025-03-24, وتحقق descritor عبر `scripts/preview_verify.sh` باستخدام (exemplo `preview-2025-03-24`).
- Você pode usar o dispositivo de segurança (`docs.preview.integrity`, e usar o `TryItProxyErrors` no W0).
- تثبيت نص التواصل باستخدام [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) com a visualização `preview-2025-03-24`.
- Verifique os mantenedores do sistema (`DOCS-SORA-Preview-REQ-01` ... `-05`).
- ارسال اول خمس دعوات 2025-03-25 10:00-10:20 UTC بعد سبعة ايام متتالية من القياس الاخضر؛ Você pode usar o `DOCS-SORA-Preview-W0`.
- متابعة القياس عن بعد + horário de expediente للمضيف (فحوصات يومية حتى 2025-03-31, سجل checkpoints بالاسفل).
- جمع ملاحظات منتصف الموجة / القضايا ووضع الوسم `docs-preview/w0` (انظر [W0 digest](./preview-feedback/w0/summary.md)).
- نشر ملخص الموجة + تأكيدات الخروج (حزمة خروج بتاريخ 2025-04-08؛ انظر [W0 digest](./preview-feedback/w0/summary.md)).
- Baixar W3 beta; Você pode fazer isso com mais frequência.

## ملخص موجة الشركاء W1

- الموافقات القانونية والحوكمة. تم توقيع ملحق الشركاء 2025-04-05؛ Verifique o valor do `DOCS-SORA-Preview-W1`.
- القياس عن بعد + Experimente a preparação. Você pode usar o `OPS-TRYIT-147` em 2025-04-06 para obter o Grafana do `docs.preview.integrity` e `TryItProxyErrors` e `DocsPortal/GatewayRefusals`.
- Artefato تجهيز + soma de verificação. تم التحقق من حزمة `preview-2025-04-12`; Aqui, descriptor/checksum/probe é `artifacts/docs_preview/W1/preview-2025-04-12/`.
- قائمة الدعوات + الارسال. تمت الموافقة على ثماني طلبات شركاء (`DOCS-SORA-Preview-REQ-P01...P08`), ارسلت الدعوات 2025-04-12 15:00-15:21 UTC مع تسجيل الايصالات لكل مراجع.
- ادوات جمع الملاحظات. تم تسجيل horário de expediente اليومية + pontos de verificação القياس؛ Selecione [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) para isso.
- القائمة النهائية / سجل الخروج. يسجل [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) Você pode exportar produtos/serviços, exportar produtos ومؤشرات artefatos حتى 2025-04-26 ليتمكن فريق الحوكمة من اعادة بناء الموجة.

## سجل الدعوات - Mantenedores principais do W0

| معرف المراجع | الدور | تذكرة الطلب | Horário de Brasília (UTC) | Hora do Dia (UTC) | الحالة | Produtos |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Mantenedor do portal | `DOCS-SORA-Preview-REQ-01` | 25/03/2025 10:05 | 08/04/2025 10:00 | Não | Como usar a soma de verificação; يركز على مراجعة nav/sidebar. |
| sdk-rust-01 | Líder do Rust SDK | `DOCS-SORA-Preview-REQ-02` | 25/03/2025 10:08 | 08/04/2025 10:00 | Não | Use o SDK + guias de início rápido Norito. |
| sdk-js-01 | Mantenedor JS SDK | `DOCS-SORA-Preview-REQ-03` | 25/03/2025 10:12 | 08/04/2025 10:00 | Não | يراجع وحدة Try it + تدفقات ISO. |
| sorafs-ops-01 | Contato com o operador SoraFS | `DOCS-SORA-Preview-REQ-04` | 25/03/2025 10:15 | 08/04/2025 10:00 | Não | Use runbooks SoraFS + orquestração. |
| observabilidade-01 | Observabilidade TL | `DOCS-SORA-Preview-REQ-05` | 25/03/2025 10:18 | 08/04/2025 10:00 | Não | يراجع ملاحق القياس/الحوادث؛ مسؤول عن تغطية Alertmanager. |Encontre o artefato `docs-portal-preview` (em 2025-03-24, e `preview-2025-03-24`) e verifique o item `DOCS-SORA-Preview-W0`. Verifique se há um/para o produto no local e se você está usando o software, você pode usá-lo.

## Pontos de verificação - W0

| التاريخ (UTC) | النشاط | Produtos |
| --- | --- | --- |
| 26/03/2025 | مراجعة linha de base القياس + horário de expediente | Os itens `docs.preview.integrity` e `TryItProxyErrors` estão disponíveis O horário comercial é o checksum. |
| 27/03/2025 | Confira o feedback do usuário | Você pode usar o [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md) تم حفظ الملخص Se você usar o navegador nav, use `docs-preview/w0`. |
| 31/03/2025 | فحص قياس الاسبوع الاخير | Horário de expediente اكد المراجعون تقدم المهام, دون تنبيهات. |
| 08/04/2025 | ملخص الخروج + اغلاق الدعوات | Você pode usar o recurso de configuração do dispositivo em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08), Ative o W1. |

## سجل الدعوات - Parceiros W1

| معرف المراجع | الدور | تذكرة الطلب | Horário de Brasília (UTC) | Hora do Dia (UTC) | الحالة | Produtos |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 12/04/2025 15:00 | 26/04/2025 15:00 | مكتمل | تم تسليم ملاحظات operações em 2025/04/20; ack até às 15:05 UTC. |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 12/04/2025 15:03 | 26/04/2025 15:00 | مكتمل | Execute a implementação em `docs-preview/w1`; retorno às 15:10 UTC. |
| sorafs-op-03 | Operador SoraFS (EUA) | `DOCS-SORA-Preview-REQ-P03` | 12/04/2025 15:06 | 26/04/2025 15:00 | مكتمل | تم تسجيل تعديلات disputa/lista negra; retorno às 15:12 UTC. |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 12/04/2025 15:09 | 26/04/2025 15:00 | مكتمل | قبول passo a passo Experimente auth؛ retorno às 15h14 UTC. |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 12/04/2025 15:12 | 26/04/2025 15:00 | مكتمل | Como usar RPC/OAuth; retorno às 15h16 UTC. |
| sdk-parceiro-01 | Parceiro SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 12/04/2025 15:15 | 26/04/2025 15:00 | مكتمل | دمج ملاحظات سلامة المعاينة؛ retorno às 15h18 UTC. |
| sdk-parceiro-02 | Parceiro SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 12/04/2025 15:18 | 26/04/2025 15:00 | مكتمل | مراجعة telemetria/redação اكتملت؛ retorno às 15:22 UTC. |
| gateway-ops-01 | Operador de gateway | `DOCS-SORA-Preview-REQ-P08` | 12/04/2025 15:21 | 26/04/2025 15:00 | مكتمل | Como configurar o gateway DNS do runbook; retorno às 15h24 UTC. |

## Pontos de verificação - W1

| التاريخ (UTC) | النشاط | Produtos |
| --- | --- | --- |
| 12/04/2025 | Artefactos de arte + arte | artefactos | تم ارسال البريد لكل الشركاء مع descritor/arquivo `preview-2025-04-12`; Você pode fazer isso no site. |
| 13/04/2025 | Linha de base de cálculo | `docs.preview.integrity` e `TryItProxyErrors` e `DocsPortal/GatewayRefusals` são usados horário de expediente اكدت اكتمال تحقق checksum. |
| 18/04/2025 | horário de expediente | Para `docs.preview.integrity` A documentação do documento é `docs-preview/w1` (nav + لقطة Experimente). |
| 22/04/2025 | فحص قياس نهائي | الوكيل ولوحات القياس سليمة؛ Não há nada que você possa fazer no site da empresa. |
| 2025/04/26 | ملخص الخروج + اغلاق الدعوات | Para fazer isso, você pode usar o recurso de configuração em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## ملخص coorte beta W3

- ارسلت الدعوات 2026-02-18 مع تحقق checksum وتسجيل الايصالات في نفس اليوم.
- O feedback do `docs-preview/20260218` é o mesmo do `DOCS-SORA-Preview-20260218`; Você pode digerir + ملخص عبر `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- تم سحب الوصول 2026-02-28 بعد فحص القياس النهائي؛ Verifique a configuração do W3.

## سجل الدعوات - Comunidade W2| معرف المراجع | الدور | تذكرة الطلب | Horário de Brasília (UTC) | Hora do Dia (UTC) | الحالة | Produtos |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Revisor da comunidade (SDK) | `DOCS-SORA-Preview-REQ-C01` | 15/06/2025 16:00 | 29/06/2025 16:00 | مكتمل | retorno às 16h06 UTC؛ Acesse o SDK de início rápido; تم تأكيد الخروج 2025-06-29. |
| comm-vol-02 | Revisor comunitário (Governação) | `REQ-C02` | 15/06/2025 16:03 | 29/06/2025 16:00 | مكتمل | مراجعة الحوكمة/SNS اكتملت؛ تم تأكيد الخروج 2025-06-29. |
| comm-vol-03 | Revisor da comunidade (Norito) | `REQ-C03` | 15/06/2025 16:06 | 29/06/2025 16:00 | مكتمل | Passo a passo do passo a passo Norito; confirmação 29/06/2025. |
| comm-vol-04 | Revisor da comunidade (SoraFS) | `REQ-C04` | 15/06/2025 16:09 | 29/06/2025 16:00 | مكتمل | Executando runbooks SoraFS confirmação 29/06/2025. |
| comm-vol-05 | Revisor da comunidade (Acessibilidade) | `REQ-C05` | 15/06/2025 16:12 | 29/06/2025 16:00 | مكتمل | مشاركة ملاحظات acessibilidade/UX; confirmação 29/06/2025. |
| comm-vol-06 | Revisor da comunidade (Localização) | `REQ-C06` | 15/06/2025 16:15 | 29/06/2025 16:00 | مكتمل | تسجيل ملاحظات localização; confirmação 29/06/2025. |
| comm-vol-07 | Revisor da comunidade (móvel) | `REQ-C07` | 15/06/2025 16:18 | 29/06/2025 16:00 | مكتمل | تم تسليم مراجعات docs SDK móvel; confirmação 29/06/2025. |
| comm-vol-08 | Revisor comunitário (Observabilidade) | `REQ-C08` | 15/06/2025 16:21 | 29/06/2025 16:00 | مكتمل | مراجعة ملحق observabilidade اكتملت؛ confirmação 29/06/2025. |

## سجل pontos de verificação - W2

| التاريخ (UTC) | النشاط | Produtos |
| --- | --- | --- |
| 15/06/2025 | Artefactos de arte + arte | artefactos | Usando o descritor/arquivo `preview-2025-06-15` em 8 meses Você pode fazer isso no site. |
| 16/06/2025 | Linha de base de cálculo | `docs.preview.integrity` e `TryItProxyErrors` e `DocsPortal/GatewayRefusals` سجلات الوكيل Experimente تظهر رموز المجتمع نشطة. |
| 18/06/2025 | horário de expediente | اقتراحان (`docs-preview/w2 #1` صياغة dica de ferramenta, `#2` شريط localização) - كلاهما اسند الى Docs. |
| 21/06/2025 | فحص قياس + اصلاحات docs | Para `docs-preview/w2 #1/#2`; Não há nada melhor do que isso. |
| 24/06/2025 | horário de expediente | اكد المراجعون ما تبقى من الملاحظات؛ Não é verdade. |
| 29/06/2025 | ملخص الخروج + اغلاق الدعوات | Para criar snapshots + artefatos (`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 15/04/2025 | horário de expediente | A solução de problemas de segurança é `docs-preview/w1`; Não há nada que você possa fazer. |

## روابط التقارير

- كل يوم اربعاء, حدّث الجدول اعلاه وتذكرة الدعوات النشطة بملاحظة قصيرة (الدعوات المرسلة), المراجعين النشطين, الحوادث).
- Você pode usar um dispositivo de armazenamento de dados (`docs/portal/docs/devportal/preview-feedback/w0/summary.md`) e `status.md`.
- اذا تم تفعيل اي معيار توقف في [visualizar fluxo de convite](./preview-invite-flow.md), اضف خطوات المعالجة هنا قبل استئناف Primeiro.