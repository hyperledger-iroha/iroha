---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-plan
título: W1 شراکت داروں کے لئے پری فلائٹ پلان
sidebar_label: W1
description: پارٹنر preview کوہوٹ کے لئے ٹاسکس, مالکان, اور ثبوت چیک لسٹ۔
---

| آئٹم | تفصیل |
| --- | --- |
| Para | W1 - Integradores e integradores Torii |
| ہدف ونڈو | 2º trimestre de 2025 3º trimestre |
| آرٹیفیکٹ ٹیگ (منصوبہ) | `preview-2025-04-12` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W1` |

## مقاصد

1. Pré-visualização de پارٹنر شرائط کے لئے قانونی اور گورننس منظوری حاصل کرنا۔
2. دعوتی بنڈل میں استعمال ہونے والے Experimente proxy ou instantâneos de telemetria تیار کرنا۔
3. checksum سے verificar شدہ visualizar artefato اور probe نتائج تازہ کرنا۔
4. دعوتیں بھیجنے سے پہلے پارٹنر lista e modelos de solicitação حتمی کرنا۔

## ٹاسک بریک ڈاؤن

| ID | ٹاسک | Mal | مقررہ تاریخ | اسٹیٹس | Não |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | pré-visualização dos termos adendo کے لئے قانونی منظوری حاصل کرنا | Líder do Docs/DevRel -> Jurídico | 05/04/2025 | ✅ مکمل | قانونی ٹکٹ `DOCS-SORA-Preview-W1-Legal` 2025-04-05 کو منظور ہوا؛ PDF ٹریکر کے ساتھ منسلک ہے۔ |
| W1-P2 | Experimente proxy کا staging ونڈو (2025-04-10) محفوظ کرنا اور proxy health کی تصدیق | Documentos/DevRel + Operações | 06/04/2025 | ✅ مکمل | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` 2025-04-06 کو چلایا گیا؛ Transcrição CLI de `.env.tryit-proxy.bak` محفوظ کر دیے گئے۔ |
| W1-P3 | preview artefato (`preview-2025-04-12`) بنانا، `scripts/preview_verify.sh` + `npm run probe:portal` چلانا، descritor/checksums محفوظ کرنا | PortalTL | 08/04/2025 | ✅ مکمل | artefato e logs de verificação `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ ہیں؛ saída da sonda ٹریکر کے ساتھ منسلک ہے۔ |
| W1-P4 | پارٹنر formulários de admissão (`DOCS-SORA-Preview-REQ-P01...P08`) کا جائزہ, contatos اور NDAs کی تصدیق | Ligação para governação | 07/04/2025 | ✅ مکمل | تمام آٹھ درخواستیں منظور ہوئیں (آخری دو 2025-04-11 کو منظور ہوئیں)؛ aprovações ٹریکر میں لنک ہیں۔ |
| W1-P5 | Você pode usar um cartão de crédito (`docs/examples/docs_preview_invite_template.md` پر مبنی), ہر پارٹنر کے لئے `<preview_tag>` ou `<request_ticket>` سیٹ کرنا | Líder do Documentos/DevRel | 08/04/2025 | ✅ مکمل | دعوت کا مسودہ 2025-04-12 15:00 UTC کو artefato لنکس کے ساتھ بھیجا گیا۔ |

## Preflight چیک لسٹ

> ٹِپ: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` چلائیں تاکہ مراحل 1-5 خودکار طور پر چل جائیں (construir, verificação de soma de verificação, sonda de portal, verificador de link, ou tentar atualização de proxy). O JSON é o que você precisa para usar o JSON

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` کے ساتھ) تاکہ `build/checksums.sha256` ou `build/release.json` دوبارہ بنیں۔
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` e `build/link-report.json` کو descritor کے ساتھ arquivo کریں۔
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (alvo alvo `--tryit-target` سے دیں)؛ اپ ڈیٹ شدہ `.env.tryit-proxy` کو commit کریں اور rollback کے لئے `.bak` رکھیں۔
6. W1 ٹریکر ایشو کو caminhos de log کے ساتھ اپ ڈیٹ کریں (soma de verificação do descritor, saída da sonda, Experimente proxy تبدیلی, اور instantâneos Grafana)۔

## ثبوت چیک لسٹ

- [x] دستخط شدہ قانونی منظوری (PDF یا ٹکٹ لنک) `DOCS-SORA-Preview-W1` کے ساتھ منسلک ہے۔
- [x] `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` کے Grafana capturas de tela۔
- [x] Descritor `preview-2025-04-12` e log de soma de verificação `artifacts/docs_preview/W1/` کے تحت محفوظ ہیں۔
- [x] دعوت tabela de lista میں `invite_sent_at` timestamps مکمل ہیں (ٹریکر W1 log دیکھیں)۔
- [x] artefatos de feedback [`preview-feedback/w1/log.md`](./log.md) میں نظر آتے ہیں, ہر پارٹنر کے لئے ایک row (2025-04-26 کو escalação/telemetria/questões ڈیٹا کے ساتھ اپ ڈیٹ)۔

جوں جوں کام آگے بڑھے یہ پلان اپ ڈیٹ کریں؛ ٹریکر اسے roadmap کی auditabilidade

## فیڈبیک ورک فلو

1. Revisor ہر کے لئے
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) modelo de modelo کاپی کریں،
   میٹا ڈیٹا بھریں اور مکمل کاپی
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` میں رکھیں۔
2. convites, pontos de verificação de telemetria e problemas em aberto
   [`preview-feedback/w1/log.md`](./log.md) کے log ao vivo میں خلاصہ کریں تاکہ revisores de governança پوری لہر کو
   repositório کے اندر ہی repetição کر سکیں۔
3. جب verificação de conhecimento یا exportações de pesquisa آئیں تو انہیں log میں درج caminho do artefato پر anexar کریں
   Problema no rastreador سے link کریں۔