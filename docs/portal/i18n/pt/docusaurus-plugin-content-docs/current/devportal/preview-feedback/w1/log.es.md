---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-log
título: Log de feedback e telemetria W1
sidebar_label: Registro W1
descrição: Lista agregada, pontos de verificação de telemetria e notas de revisores para a primeira ou última visualização de parceiros.
---

Este registro mantém a lista de convites, pontos de verificação de telemetria e feedback dos revisores para o
**visualização dos parceiros W1** que acompanha as tarefas de aceitação em
[`preview-feedback/w1/plan.md`](./plan.md) e a entrada do rastreador da ola em
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Atualizando quando você deseja um convite,
registre um instantâneo de telemetria ou trie um item de feedback para que os revisores de governo possam reproduzi-lo
a evidência sem perseguir ingressos externos.

## Lista da coorte

| ID do parceiro | Ticket de solicitação | NDA recebida | Convite enviado (UTC) | Login de confirmação/primer (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| parceiro-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 12/04/2025 15:00 | 12/04/2025 15:11 | Concluído 2025-04-26 | sorafs-op-01; enfocado na evidência de paridade de documentos do orquestrador. |
| parceiro-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 12/04/2025 15:03 | 12/04/2025 15:15 | Concluído 2025-04-26 | sorafs-op-02; valido cross-links de Norito/telemetria. |
| parceiro-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 12/04/2025 15:06 | 12/04/2025 15:18 | Concluído 2025-04-26 | sorafs-op-03; ejecuto treina de failover multi-fonte. |
| parceiro-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 12/04/2025 15:09 | 12/04/2025 15:21 | Concluído 2025-04-26 | torii-int-01; revisão do livro de receitas de Torii `/v1/pipeline` + Experimente. |
| parceiro-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 12/04/2025 15:12 | 12/04/2025 15:23 | Concluído 2025-04-26 | torii-int-02; acompanha a atualização da captura de tela de Try it (docs-preview/w1 #2). |
| parceiro-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 12/04/2025 15:15 | 12/04/2025 15:26 | Concluído 2025-04-26 | SDK-parceiro-01; feedback de livros de receitas JS/Swift + verificações de sanidade da ponte ISO. |
| parceiro-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 12/04/2025 15:18 | 12/04/2025 15:29 | Concluído 2025-04-26 | SDK-parceiro-02; compliance aprovado 2025-04-11, enfocado em notas de Connect/telemetria. |
| parceiro-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 12/04/2025 15:21 | 12/04/2025 15:33 | Concluído 2025-04-26 | gateway-ops-01; auditar o guia de operações do gateway + fluxo anônimo do proxy Try it. |

Preencha os carimbos de data/hora de **Convite enviado** e **Ack** apenas se emita o e-mail relevante.
Anula os tempos do calendário UTC definido no plano W1.

## Pontos de verificação de telemetria

| Carimbo de data/hora (UTC) | Painéis/sondas | Responsável | Resultado | Artefato |
| --- | --- | --- | --- | --- |
| 06/04/2025 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operações | Tudo em verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06/04/2025 18:20 | Transcrição de `npm run manage:tryit-proxy -- --stage preview-w1` | Operações | Preparado | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 12/04/2025 14:45 | Dashboards de arriba + `probe:portal` | Documentos/DevRel + Operações | Pré-convite de instantâneo, sem regressões | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | Dashboards de subida + diferença de latência do proxy Experimente | Líder do Documentos/DevRel | Verificação de metade de ola ok (0 alertas; latência Experimente p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | Dashboards de subida + sonda de saída | Contato do Docs/DevRel + Governança | Instantâneo de saída, zero alertas pendentes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

As muestras diárias de horário comercial (2025-04-13 -> 2025-04-25) são agrupadas como exportações NDJSON + PNG abaixo
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` com nomes de arquivo
`docs-preview-integrity-<date>.json` e as capturas de tela correspondentes.

## Log de feedback e problemas

Use esta tabela para resumir hallazgos enviados por revisores. Inserir cada entrada no ticket do GitHub/discuss
mas o formulário estruturado foi capturado via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referência | Severidade | Responsável | Estado | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Baixo | Documentos-núcleo-02 | Resultado 2025-04-18 | Veja o texto de navegação de Try it + ancla de sidebar (`docs/source/sorafs/tryit.md` atualizado com novo rótulo). |
| `docs-preview/w1 #2` | Baixo | Documentos-núcleo-03 | Resultado 2025-04-19 | Veja a captura de tela atualizada de Try it + caption após o pedido do revisor; artefato `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Informações | Líder do Documentos/DevRel | Cerrado | Os comentários restantes foram apenas perguntas e respostas; capturados no formulário de feedback de cada parceiro abaixo `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Acompanhamento de verificação de conhecimento e pesquisas1. Registrar as pontuações do quiz (objetivo >=90%) para cada revisor; adicione o CSV exportado junto com os artefatos de convite.
2. Colete as respostas qualitativas da pesquisa capturada com o modelo de feedback e reflita-as abaixo
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Agenda chamada de remediação para qualquer pessoa por baixo do umbral e registrada neste arquivo.

Os outros revisores marcaram >=94% na verificação de conhecimento (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Não são necessárias chamadas de remediação;
as exportações de pesquisa para cada parceiro viven bajo
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventário de artefatos

- Pacote de descritor/soma de verificação de visualização: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Resumo da sonda + verificação de link: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de mudança do proxy Experimente: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exportações de telemetria: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Pacote diário de telemetria de horário comercial: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exportações de feedback + pesquisas: colocar pastas por revisor baixo
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV e resumo da verificação de conhecimento: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Mantenha o inventário sincronizado com a emissão do rastreador. Adiciona hashes para copiar artefatos no ticket de governo
para que os auditores verifiquem os arquivos sem acesso ao shell.