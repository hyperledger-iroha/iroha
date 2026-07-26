---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-resumo
título: W1 فيڈبیک اور اختتامی خلاصہ
sidebar_label: W1 خلاصہ
description: Onda de visualização dos integradores پارٹنر/Torii
---

| آئٹم | تفصیل |
| --- | --- |
| Para | W1 - Integradores e integradores Torii |
| دعوتی ونڈو | 12/04/2025 -> 26/04/2025 |
| آرٹیفیکٹ ٹیگ | `preview-2025-04-12` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W1` |
| شرکا | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## نمایاں نکات

1. **Checksum ورک فلو** - تمام revisores نے `scripts/preview_verify.sh` کے ذریعے descritor/arquivo کی تصدیق کی؛ logs کو دعوتی agradecimentos کے ساتھ محفوظ کیا گیا۔
2. **ٹیلیمیٹری** - `docs.preview.integrity`, `TryItProxyErrors`, اور `DocsPortal/GatewayRefusals` painéis پوری لہر کے دوران verde رہے؛ کوئی incidentes یا páginas de alerta نہیں ہوئیں۔
3. **Docs فيڈبیک (`docs-preview/w1`)** - دو معمولی نٹس ریکارڈ ہوئیں:
   - `docs-preview/w1 #1`: Experimente سیکشن میں texto de navegação واضح کرنا (حل ہو گیا)۔
   - `docs-preview/w1 #2`: Experimente.
4. **Paridade de runbook** - Operadores SoraFS خدشات حل کیے۔

## ایکشن آئٹمز

| ID | وضاحت | Mal | اسٹیٹس |
| --- | --- | --- | --- |
| W1-A1 | `docs-preview/w1 #1` کے مطابق Experimente o texto de navegação اپ ڈیٹ کرنا۔ | Documentos-núcleo-02 | ✅ مکمل (2025-04-18). |
| W1-A2 | `docs-preview/w1 #2` کے مطابق Experimente اسکرین شاٹ اپ ڈیٹ کرنا۔ | Documentos-núcleo-03 | ✅ مکمل (2025-04-19). |
| W1-A3 | پارٹنر descobertas اور evidências de telemetria کو roteiro/status میں سمری کرنا۔ | Líder do Documentos/DevRel | ✅ مکمل (rastreador + status.md دیکھیں). |

## اختتامی خلاصہ (2025-04-26)

- تمام آٹھ revisores نے آخری horário de expediente میں تکمیل کی تصدیق کی, لوکل artefatos صاف کیے, اور ان کی رسائی واپس لی گئی۔
- ٹیلیمیٹری اختتام تک verde رہی؛ Instantâneos `DOCS-SORA-Preview-W1` کے ساتھ منسلک ہیں۔
- دعوتی log میں reconhecimentos de saída شامل کیے گئے؛ rastreador نے W1 کو 🈴 پر سیٹ کیا اور pontos de verificação شامل کیے۔
- pacote de evidências (descritor, log de soma de verificação, saída da sonda, transcrição do proxy Try it, capturas de tela de telemetria, resumo de feedback) `artifacts/docs_preview/W1/` Arquivo میں ہوا۔

## اگلے اقدامات

- Plano de admissão da comunidade W2 تیار کریں (aprovação de governança + ajustes no modelo de solicitação).
- Onda W2 کے لئے pré-visualização da tag do artefato
- W1 کے قابل اطلاق descobertas کو roteiro/status میں منتقل کریں تاکہ onda da comunidade کے پاس تازہ orientação ہو۔