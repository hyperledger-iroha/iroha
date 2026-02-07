---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو دعوت ٹریکر

یہ ٹریکر docs پورٹل کی ہر پریویو ویو ریکارڈ کرتا ہے تاکہ DOCS-SORA کے مالکان اور revisores de governança دیکھ سکیں کہ کون سی coorte فعال ہے, کس نے دعوتیں منظور کیں, اور کون سے artefatos ابھی توجہ چاہتے ہیں۔ جب بھی دعوتیں بھیجی جائیں, واپس لی جائیں یا مؤخر ہوں تو اسے اپ ڈیٹ کریں Trilha de auditoria تاکہ ریپوزٹری کے اندر رہے۔

## ویو اسٹیٹس

| ویو | coorte | ٹریکر ایشو | aprovador(es) | اسٹیٹس | ہدف ونڈو | Não |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principais** | Mantenedores do Docs + SDK e checksum e validar کرتے ہیں | `DOCS-SORA-Preview-W0` (rastreador GitHub/ops) | Líder Docs/DevRel + Portal TL | مکمل | 2º trimestre de 2025 1-2 | دعوتیں 2025-03-25 کو بھیجی گئیں, ٹیلی میٹری سبز رہی, resumo de saída 2025-04-08 کو شائع ہوا۔ |
| **W1 - Parceiros** | SoraFS آپریٹرز, Torii integradores تحت NDA | `DOCS-SORA-Preview-W1` | Líder do Docs/DevRel + contato de governança | مکمل | 2º trimestre de 2025 3º trimestre | دعوتیں 2025-04-12 -> 2025-04-26، تمام آٹھ پارٹنرز کی تصدیق؛ evidência [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) میں اور exit digest [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) میں۔ |
| **W2 - Comunidade** | Lista de espera da comunidade (<=25 ایک وقت میں) | `DOCS-SORA-Preview-W2` | Líder do Docs/DevRel + gerente de comunidade | مکمل | 3º trimestre de 2025 ہفتہ 1 (عارضی) | دعوتیں 2025-06-15 -> 2025-06-29، پوری مدت میں ٹیلی میٹری سبز؛ evidências + descobertas [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md) میں۔ |
| **W3 – Coortes beta** | beta de finanças/observabilidade + parceiro SDK + defensor do ecossistema | `DOCS-SORA-Preview-W3` | Líder do Docs/DevRel + contato de governança | مکمل | 1º trimestre de 2026 8 | دعوتیں 2026-02-18 -> 2026-02-28؛ resumo + dados do portal `preview-20260218` e سے تیار (دیکھیں [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> نوٹ: ہر ٹریکر ایشو کو متعلقہ solicitação de visualização de ingressos سے لنک کریں اور انہیں `docs-portal-preview` پروجیکٹ میں arquivo کریں Aprovações de تاکہ قابلِ دریافت رہیں۔

## فعال کام (W0)

- artefatos de comprovação ریفریش (GitHub Actions `docs-portal-preview` رن 2025-03-24, descritor `scripts/preview_verify.sh` کے ذریعے `preview-2025-03-24` ٹیگ سے verificar)۔
- ٹیلی میٹری linhas de base محفوظ (`docs.preview.integrity`, `TryItProxyErrors` instantâneo do painel W0 problema میں محفوظ)۔
- divulgação متن [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) کے ساتھ lock, tag de visualização `preview-2025-03-24`۔
- پہلے پانچ mantenedores کے لئے solicitações de entrada لاگ (ingressos `DOCS-SORA-Preview-REQ-01` ... `-05`).
- پہلی پانچ دعوتیں 2025-03-25 10:00-10:20 UTC کو بھیجیں, سات دن مسلسل سبز ٹیلی میٹری کے بعد؛ agradecimentos `DOCS-SORA-Preview-W0` میں محفوظ۔
- ٹیلی میٹری مانیٹرنگ + horário de atendimento do anfitrião (2025-03-31 تک روزانہ check-ins; registro de ponto de verificação نیچے)۔
- feedback/problemas de ponto médio
- ویو resumo شائع + convite para confirmações de saída (pacote de saída تاریخ 2025-04-08; دیکھیں [W0 digest](./preview-feedback/w0/summary.md))۔
- Onda beta W3 ٹریک; Análise da governança e avaliação da governança

## Parceiros W1 e خلاصہ

- قانونی اور aprovações de governança۔ Adendo de parceiro 2025-04-05 کو سائن؛ aprovações `DOCS-SORA-Preview-W1` میں اپ لوڈ۔
- Telemetria + Experimente a encenação۔ Alterar ticket `OPS-TRYIT-147` 2025-04-06 کو اجرا؛ `docs.preview.integrity`, `TryItProxyErrors`, اور `DocsPortal/GatewayRefusals` کے Grafana instantâneos آرکائیو۔
- Artefato + preparação de soma de verificação۔ Verificação do pacote `preview-2025-04-12`; descritor/checksum/logs de sonda `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ۔
- Lista de convites + envio۔ آٹھ solicitações de parceiros (`DOCS-SORA-Preview-REQ-P01...P08`) منظور؛ دعوتیں 2025-04-12 15:00-15:21 UTC کو بھیجیں, ہر revisor کا ack ریکارڈ۔
- Instrumentação de feedback۔ Horário de expediente + pontos de verificação de telemetria روزانہ ریکارڈ؛ digest کے لئے [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) دیکھیں۔
- Registro final de escalação/saída۔ [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) اب carimbos de data/hora de convite/confirmação, evidência de telemetria, exportações de quiz, اور ponteiros de artefato 2025-04-26 تک ریکارڈ کرتا ہے تاکہ onda de governança کو reproduzir کر سکے۔

## Registro de convites - mantenedores principais do W0| ID do revisor | Função | Solicitar ingresso | Convite enviado (UTC) | Saída prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Mantenedor do portal | `DOCS-SORA-Preview-REQ-01` | 25/03/2025 10:05 | 08/04/2025 10:00 | Ativo | verificação de soma de verificação revisão de navegação/barra lateral |
| sdk-rust-01 | Líder do Rust SDK | `DOCS-SORA-Preview-REQ-02` | 25/03/2025 10:08 | 08/04/2025 10:00 | Ativo | Receitas SDK + guias de início rápido Norito ٹیسٹ کر رہا ہے۔ |
| sdk-js-01 | Mantenedor JS SDK | `DOCS-SORA-Preview-REQ-03` | 25/03/2025 10:12 | 08/04/2025 10:00 | Ativo | Experimente console + fluxos ISO ویلیڈیٹ۔ |
| sorafs-ops-01 | Contato com o operador SoraFS | `DOCS-SORA-Preview-REQ-04` | 25/03/2025 10:15 | 08/04/2025 10:00 | Ativo | Runbooks SoraFS + documentos de orquestração |
| observabilidade-01 | Observabilidade TL | `DOCS-SORA-Preview-REQ-05` | 25/03/2025 10:18 | 08/04/2025 10:00 | Ativo | apêndices de telemetria/incidentes کا جائزہ؛ Cobertura do Alertmanager |

تمام دعوتیں ایک ہی `docs-portal-preview` artefato (executado em 2025-03-24, tag `preview-2025-03-24`) اور `DOCS-SORA-Preview-W0` میں محفوظ transcrição de verificação کو ریفرنس کرتی ہیں۔ کسی بھی اضافے/وقفے کو اگلی ویو پر جانے سے پہلے اوپر والی ٹیبل اور problema do rastreador دونوں میں لاگ کریں۔

## Registro do ponto de verificação - W0

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 26/03/2025 | Revisão da linha de base da telemetria + horário comercial | `docs.preview.integrity` + `TryItProxyErrors` سبز رہے؛ horário comercial نے verificação de soma de verificação مکمل ہونے کی تصدیق کی۔ |
| 27/03/2025 | Resumo de feedback de ponto médio postado | Resumo [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md) میں محفوظ؛ دو چھوٹے problemas de navegação `docs-preview/w0` کے تحت، کوئی incidente نہیں۔ |
| 31/03/2025 | Verificação pontual de telemetria da semana final | Horário de expediente; revisores نے باقی کام تصدیق کیا, کوئی alerta نہیں۔ |
| 08/04/2025 | Resumo de saída + fechamento de convites | avaliações مکمل، عارضی رسائی منسوخ، descobertas [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08) میں آرکائیو؛ Rastreador W1 سے پہلے اپ ڈیٹ۔ |

## Registro de convites - parceiros W1

| ID do revisor | Função | Solicitar ingresso | Convite enviado (UTC) | Saída prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 12/04/2025 15:00 | 26/04/2025 15:00 | Concluído | feedback de operações do orquestrador 20/04/2025 saída confirmada 15:05 UTC۔ |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 12/04/2025 15:03 | 26/04/2025 15:00 | Concluído | comentários de implementação `docs-preview/w1` میں لاگ؛ saída confirmada 15:10 UTC۔ |
| sorafs-op-03 | Operador SoraFS (EUA) | `DOCS-SORA-Preview-REQ-P03` | 12/04/2025 15:06 | 26/04/2025 15:00 | Concluído | edições de disputa/lista negra saída confirmada 15:12 UTC۔ |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 12/04/2025 15:09 | 26/04/2025 15:00 | Concluído | Experimente o passo a passo de autenticação saída confirmada 15:14 UTC۔ |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 12/04/2025 15:12 | 26/04/2025 15:00 | Concluído | Comentários RPC/OAuth saída confirmada 15:16 UTC۔ |
| sdk-parceiro-01 | Parceiro SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 12/04/2025 15:15 | 26/04/2025 15:00 | Concluído | visualizar mesclagem de feedback de integridade; saída confirmada 15:18 UTC۔ |
| sdk-parceiro-02 | Parceiro SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 12/04/2025 15:18 | 26/04/2025 15:00 | Concluído | revisão de telemetria/redação مکمل؛ saída confirmada 15:22 UTC۔ |
| gateway-ops-01 | Operador de gateway | `DOCS-SORA-Preview-REQ-P08` | 12/04/2025 15:21 | 26/04/2025 15:00 | Concluído | comentários do runbook DNS do gateway saída confirmada 15:24 UTC۔ |

## Registro do ponto de verificação - W1

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 12/04/2025 | Envio de convite + verificação de artefato | Descritor/arquivo `preview-2025-04-12` کے ساتھ آٹھ parceiros کو ای میل؛ rastreador de agradecimentos میں محفوظ۔ |
| 13/04/2025 | Revisão da linha de base da telemetria | `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` سبز؛ horário comercial نے verificação de soma de verificação مکمل ہونے کی تصدیق کی۔ |
| 18/04/2025 | Horário comercial da onda média | `docs.preview.integrity` سبز رہا؛ دو doc نٹس `docs-preview/w1` کے تحت لاگ (texto de navegação + captura de tela de teste)۔ |
| 22/04/2025 | Verificação final de telemetria | proxy + painéis کوئی نئی questões نہیں، sair سے پہلے rastreador میں نوٹ۔ |
| 2025/04/26 | Resumo de saída + fechamento de convites | تمام parceiros نے conclusão da revisão کی تصدیق کی، convida revogar, evidência [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26) میں آرکائیو۔ |

## Recapitulação da coorte W3 beta- 18/02/2026 کو convites بھیجی گئیں، verificação de soma de verificação + reconhecimentos اسی دن لاگ۔
- feedback `docs-preview/20260218` میں جمع، problema de governança `DOCS-SORA-Preview-20260218`؛ resumo + resumo `npm run --prefix docs/portal preview:wave -- --wave preview-20260218` سے تیار۔
- 28/02/2026 کو verificação final de telemetria کے بعد revogação de acesso؛ rastreador + tabelas de portal اپ ڈیٹ کر کے W3 مکمل دکھایا۔

## Registro de convites - comunidade W2

| ID do revisor | Função | Solicitar ingresso | Convite enviado (UTC) | Saída prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Revisor da comunidade (SDK) | `DOCS-SORA-Preview-REQ-C01` | 15/06/2025 16:00 | 29/06/2025 16:00 | Concluído | Confirmação 16:06 UTC; Guias de início rápido do SDK saída 2025-06-29 کو کنفرم۔ |
| comm-vol-02 | Revisor comunitário (Governação) | `REQ-C02` | 15/06/2025 16:03 | 29/06/2025 16:00 | Concluído | governança/revisão do SNS saída 2025-06-29 کو کنفرم۔ |
| comm-vol-03 | Revisor da comunidade (Norito) | `REQ-C03` | 15/06/2025 16:06 | 29/06/2025 16:00 | Concluído | Feedback do passo a passo Norito sair da confirmação 2025-06-29۔ |
| comm-vol-04 | Revisor da comunidade (SoraFS) | `REQ-C04` | 15/06/2025 16:09 | 29/06/2025 16:00 | Concluído | Revisão do runbook SoraFS sair da confirmação 2025-06-29۔ |
| comm-vol-05 | Revisor da comunidade (Acessibilidade) | `REQ-C05` | 15/06/2025 16:12 | 29/06/2025 16:00 | Concluído | Notas de acessibilidade/UX sair da confirmação 2025-06-29۔ |
| comm-vol-06 | Revisor da comunidade (Localização) | `REQ-C06` | 15/06/2025 16:15 | 29/06/2025 16:00 | Concluído | Feedback de localização sair da confirmação 2025-06-29۔ |
| comm-vol-07 | Revisor da comunidade (móvel) | `REQ-C07` | 15/06/2025 16:18 | 29/06/2025 16:00 | Concluído | Verificações de documentos do Mobile SDK sair da confirmação 2025-06-29۔ |
| comm-vol-08 | Revisor comunitário (Observabilidade) | `REQ-C08` | 15/06/2025 16:21 | 29/06/2025 16:00 | Concluído | Revisão do apêndice de observabilidade مکمل؛ sair da confirmação 2025-06-29۔ |

## Registro do ponto de verificação - W2

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 15/06/2025 | Envio de convite + verificação de artefato | Descritor/arquivo `preview-2025-06-15` 8 revisores da comunidade کے ساتھ شیئر؛ rastreador de agradecimentos میں محفوظ۔ |
| 16/06/2025 | Revisão da linha de base da telemetria | Painéis `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` Experimente registros de proxy com tokens da comunidade |
| 18/06/2025 | Horário comercial e triagem de problemas | دو sugestões (redação da dica de ferramenta `docs-preview/w2 #1`, barra lateral de localização `#2`) - دونوں Documentos کو roteado۔ |
| 21/06/2025 | Verificação de telemetria + correções de documentos | Documentos `docs-preview/w2 #1/#2` حل کیا؛ dashboards سبز، کوئی incidente نہیں۔ |
| 24/06/2025 | Horário de expediente da última semana | revisores نے باقی envios de feedback کنفرم کیے؛ Alerta de alerta |
| 29/06/2025 | Resumo de saída + fechamento de convites | acks ریکارڈ, acesso de visualização revogar, instantâneos de telemetria + artefatos آرکائیو (دیکھیں [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 15/04/2025 | Horário comercial e triagem de problemas | Ou sugestões de documentação `docs-preview/w1` کے تحت لاگ؛ کوئی incidentes یا alertas نہیں۔ |

## Ganchos de relatórios

- ہر بدھ، اوپر والی tabela اور problema de convite ativo کو مختصر nota de status سے اپ ڈیٹ کریں (convites enviados, revisores ativos, incidentes).
- جب کوئی ویو بند ہو، caminho de resumo de feedback شامل کریں (مثال: `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) اور اسے `status.md` سے لنک کریں۔
- اگر [visualizar fluxo de convite](./preview-invite-flow.md) کے critérios de pausa ٹرگر ہوں تو convites دوبارہ شروع کرنے سے پہلے etapas de remediação یہاں شامل کریں۔