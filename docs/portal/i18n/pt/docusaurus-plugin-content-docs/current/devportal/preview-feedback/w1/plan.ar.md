---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-plan
título: خطة التجهيز المسبق لشركاء W1
sidebar_label: Nome W1
description: مهام, مالكون, وقائمة ادلة لمجموعة معاينة الشركاء.
---

| البند | التفاصيل |
| --- | --- |
| الموجة | W1 - Conjunto de ferramentas Torii |
| نافذة الهدف | Dia 2025 Dia 3 |
| وسم الاثر (مخطط) | `preview-2025-04-12` |
| تذكرة المتتبع | `DOCS-SORA-Preview-W1` |

## الاهداف

1).
2. Experimente e experimente.
3. Verifique a soma de verificação e as sondagens.
4. Certifique-se de que o produto esteja funcionando corretamente.

## تفصيل المهام

| المعرف | المهمة | المالك | الاستحقاق | الحالة | Produtos |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Máquinas de lavar roupas para crianças | Líder do Docs/DevRel -> Jurídico | 05/04/2025 | ✅ مكتمل | A solução de problemas `DOCS-SORA-Preview-W1-Legal` foi lançada em 2025-04-05; ملف PDF مرفق بالمتتبع. |
| W1-P2 | حجز نافذة staging لوكيل Try it (2025-04-10) والتحقق من صحة الوكيل | Documentos/DevRel + Operações | 06/04/2025 | ✅ مكتمل | Retirado `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` em 2025-04-06; Você pode usar CLI e `.env.tryit-proxy.bak`. |
| W1-P3 | Usando o código (`preview-2025-04-12`), use `scripts/preview_verify.sh` + `npm run probe:portal`, e descritor/checksums | PortalTL | 08/04/2025 | ✅ مكتمل | Para obter mais informações, consulte `artifacts/docs_preview/W1/preview-2025-04-12/`; Sonda de sonda مرفقة بالمتتبع. |
| W1-P4 | Informações sobre ingestão de alimentos (`DOCS-SORA-Preview-REQ-P01...P08`), contratos de venda e NDAs | Ligação para governação | 07/04/2025 | ✅ مكتمل | تمت الموافقة على الطلبات الثمانية (اخر طلبين em 11/04/2025), O problema é o seguinte. |
| W1-P5 | Máquina de lavar roupa (مبنية على `docs/examples/docs_preview_invite_template.md`), e `<preview_tag>` e `<request_ticket>` para todos os lados | Líder do Documentos/DevRel | 08/04/2025 | ✅ مكتمل | A partida será realizada em 12/04/2025 às 15:00 UTC em 12/04/2025. |

## قائمة التحقق قبل الاطلاق

> Exemplo: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` لتنفيذ الخطوات 1-5 تلقائيا (build, تحقق checksum, probe للبوابة, link checker, وتحديث وكيل Try isso). O JSON não pode ser configurado para ser executado.

1. `npm run build` (como `DOCS_RELEASE_TAG=preview-2025-04-12`) é igual a `build/checksums.sha256` e `build/release.json`.
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` e `build/link-report.json` é o descritor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou seja, `--tryit-target`); Verifique o valor do `.env.tryit-proxy` e do `.bak`.
6. Verifique o W1 usando o descritor de soma de verificação, teste de teste, teste e experimente, e Grafana).

## قائمة ادلة الاثبات

- [x] موافقة قانونية موقعة (PDF او رابط التذكرة) مرفقة بـ `DOCS-SORA-Preview-W1`.
- [x] para Grafana para `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] descritor e soma de verificação para `preview-2025-04-12` é igual a `artifacts/docs_preview/W1/`.
- [x] A lista de escalação é baseada no `invite_sent_at` (راجع سجل W1 no site).
- [x] تم تحديثه 2025-04-26 ببيانات escalação/telemetria/edições).

حدّث هذه الخطة كلما تقدمت المهام؛ Certifique-se de que o dispositivo esteja funcionando corretamente.

## سير عمل التغذية الراجعة

1. لكل مراجع, انسخ القالب في
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)،
   املأ البيانات الوصفية, واحفظ النسخة المكتملة تحت
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. لخص الدعوات ونقاط القياس والمسائل المفتوحة داخل السجل الحي في
   [`preview-feedback/w1/log.md`](./log.md).
   Não há problema.
3. Faça o download do arquivo e do cartão de crédito no site da empresa.
   واربط تذكرة المتتبع.