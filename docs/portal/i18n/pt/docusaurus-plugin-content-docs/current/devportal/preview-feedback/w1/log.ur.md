---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-log
título: W1 فيڈبیک اور ٹیلیمیٹری لاگ
sidebar_label: W1 Palavra
descrição: پہلی پارٹنر onda de visualização کے لئے مجموعی escalação, ٹیلیمیٹری pontos de verificação, اور revisores نوٹس۔
---

یہ لاگ **Visualização do W1 پارٹنر** کے لئے lista de convites, ٹیلیمیٹری pontos de verificação, e feedback do revisor محفوظ کرتا ہے
جو [`preview-feedback/w1/plan.md`](./plan.md) کی tarefas de aceitação e rastreador de onda اندراج
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md) کے ساتھ وابستہ ہیں۔ جب دعوت ارسال ہو،
ٹیلیمیٹری snapshot ریکارڈ ہو، یا triagem de item de feedback ہو تو اسے اپ ڈیٹ کریں تاکہ revisores de governança
بغیر بیرونی ٹکٹس کے پیچھے گئے ثبوت replay کر سکیں۔

## Lista da coorte

| ID do parceiro | Solicitar ingresso | NDA Método | Convite enviado (UTC) | Confirmação/primeiro login (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| parceiro-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 03/04/2025 | 12/04/2025 15:00 | 12/04/2025 15:11 | ✅ Data 2025/04/26 | sorafs-op-01; orquestrador documenta evidência de paridade |
| parceiro-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 03/04/2025 | 12/04/2025 15:03 | 12/04/2025 15:15 | ✅ Data 2025/04/26 | sorafs-op-02; Norito/ligações cruzadas de telemetria کی توثیق۔ |
| parceiro-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 04/04/2025 | 12/04/2025 15:06 | 12/04/2025 15:18 | ✅ Data 2025/04/26 | sorafs-op-03; exercícios de failover de várias fontes |
| parceiro-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 04/04/2025 | 12/04/2025 15:09 | 12/04/2025 15:21 | ✅ Data 2025/04/26 | torii-int-01; Torii `/v2/pipeline` + Experimente a revisão do livro de receitas۔ |
| parceiro-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 05/04/2025 | 12/04/2025 15:12 | 12/04/2025 15:23 | ✅ Data 2025/04/26 | torii-int-02; Experimente a atualização da captura de tela میں ساتھ دیا (docs-preview/w1 #2). |
| parceiro-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 05/04/2025 | 12/04/2025 15:15 | 12/04/2025 15:26 | ✅ Data 2025/04/26 | SDK-parceiro-01; Feedback do livro de receitas JS/Swift + verificações de integridade da ponte ISO۔ |
| parceiro-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 11/04/2025 | 12/04/2025 15:18 | 12/04/2025 15:29 | ✅ Data 2025/04/26 | SDK-parceiro-02; conformidade 2025-04-11 کو clear، Notas de conexão/telemetria پر فوکس۔ |
| parceiro-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 11/04/2025 | 12/04/2025 15:21 | 12/04/2025 15:33 | ✅ Data 2025/04/26 | gateway-ops-01; guia de operações de gateway auditoria + anonimizado Experimente fluxo de proxy۔ |

**Convite enviado** اور **Ack** کے timestamps فوری طور پر درج کریں جب e-mail de saída جاری ہو۔
وقت کو W1 پلان میں دی گئی Horário UTC کے مطابق رکھیں۔

## Pontos de verificação de telemetria

| Carimbo de data/hora (UTC) | Painéis/sondas | Proprietário | Resultado | Artefato |
| --- | --- | --- | --- | --- |
| 06/04/2025 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operações | ✅ Tudo verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06/04/2025 18:20 | Transcrição `npm run manage:tryit-proxy -- --stage preview-w1` | Operações | ✅ Encenado | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 12/04/2025 14:45 | Painéis de controle + `probe:portal` | Documentos/DevRel + Operações | ✅ Instantâneo pré-convite, sem regressões | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | Dashboards اوپر + Experimente diferença de latência de proxy | Líder do Documentos/DevRel | ✅ Verificação de ponto médio aprovada (0 alertas; latência de teste p95 = 410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | Painéis de controle + sonda de saída | Contato do Docs/DevRel + Governança | ✅ Instantâneo de saída, zero alertas pendentes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Amostras de horário comercial (2025-04-13 -> 2025-04-25) Exportações NDJSON + PNG کے طور پر
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` میں موجود ہیں, فائل نام
`docs-preview-integrity-<date>.json` اور متعلقہ screenshots کے ساتھ۔

## Feedback e registro de problemas

As descobertas do revisor são importantes para você ہر entrada کو GitHub/discuss
ticket اور اس formulário estruturado سے جوڑیں جو
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) کے ذریعے بھرا گیا۔

| Referência | Gravidade | Proprietário | Estado | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Baixo | Documentos-núcleo-02 | ✅ Resolvido 18/04/2025 | Experimente o texto de navegação + âncora da barra lateral واضح کیا (`docs/source/sorafs/tryit.md` نئے rótulo کے ساتھ اپ ڈیٹ). |
| `docs-preview/w1 #2` | Baixo | Documentos-núcleo-03 | ✅ Resolvido 19/04/2025 | Experimente a captura de tela + revisor de legenda کی درخواست پر اپ ڈیٹ؛ artefato `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Informações | Líder do Documentos/DevRel | 🟢 Fechado | Perguntas e respostas sobre perguntas e respostas ہر پارٹنر کے formulário de feedback میں محفوظ ہیں `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Verificação de conhecimento e rastreamento de pesquisa

1. ہر revisor کے pontuações do questionário ریکارڈ کریں (meta >=90%); CSV exportado کو artefatos de convite کے ساتھ anexar کریں۔
2. formulário de feedback سے حاصل respostas de pesquisas qualitativas جمع کریں اور انہیں
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/` کے تحت espelho کریں۔
3. limite سے نیچے والوں کے لئے agendamento de chamadas de remediação کریں اور انہیں اس فائل میں log کریں۔تمام آٹھ revisores نے verificação de conhecimento میں >=94% اسکور کیا (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). کوئی chamadas de remediação درکار نہیں ہوئیں؛
ہر پارٹنر کے pesquisas exportam یہاں موجود ہیں:
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventário de artefatos

- Pacote de descritor/soma de verificação de visualização: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Resumo da sonda + verificação de link: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Experimente o log de alterações do proxy: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exportações de telemetria: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
Pacote diário de telemetria no horário comercial: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exportações de feedback + pesquisa: pastas específicas do revisor رکھیں
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` کے تحت
- Verificação de conhecimento CSV ou resumo: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Problema de inventário e rastreador کے ساتھ sincronização رکھیں۔ جب artefatos کو ticket de governança میں کاپی کریں تو hashes anexar کریں
تاکہ auditores فائلیں بغیر acesso shell کے verificar کر سکیں۔