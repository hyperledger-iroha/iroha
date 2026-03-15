---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-log
título: Log de feedback e telemetria W1
sidebar_label: Registro W1
descrição: Roster agregado, checkpoints de telemetria e notas de revisores para a primeira onda de visualização de parceiros.
---

Este log mantém a lista de convites, pontos de telemetria e feedback dos revisores para o
**preview de parceiros W1** que acompanha as tarefas de aceitação em
[`preview-feedback/w1/plan.md`](./plan.md) e a entrada do tracker da onda em
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Atualizar quando um convite para enviado,
um snapshot de telemetria para registrado ou um item de feedback para triado para que revisores de governança possam
reproduzir as evidências sem perseguir ingressos externos.

## Lista da coorte

| ID do parceiro | Ticket de solicitação | NDA recebido | Convite enviado (UTC) | Ack/primeiro login (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| parceiro-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 12/04/2025 15:00 | 12/04/2025 15:11 | Concluído 2025-04-26 | sorafs-op-01; focado em evidência de paridade dos documentos do orquestrador. |
| parceiro-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 12/04/2025 15:03 | 12/04/2025 15:15 | Concluído 2025-04-26 | sorafs-op-02; validou cross-links Norito/telemetria. |
| parceiro-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 12/04/2025 15:06 | 12/04/2025 15:18 | Concluído 2025-04-26 | sorafs-op-03; executou drills de failover multi-source. |
| parceiro-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 12/04/2025 15:09 | 12/04/2025 15:21 | Concluído 2025-04-26 | torii-int-01; revisão do livro de receitas Torii `/v2/pipeline` + Experimente. |
| parceiro-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 12/04/2025 15:12 | 12/04/2025 15:23 | Concluído 2025-04-26 | torii-int-02; acompanhou a atualização do screenshot Try it (docs-preview/w1 #2). |
| parceiro-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 12/04/2025 15:15 | 12/04/2025 15:26 | Concluído 2025-04-26 | SDK-parceiro-01; feedback de livros de receitas JS/Swift + verificações de integridade da ponte ISO. |
| parceiro-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 12/04/2025 15:18 | 12/04/2025 15:29 | Concluído 2025-04-26 | SDK-parceiro-02; cumprimento aprovado em 11/04/2025, focado em notas de Connect/telemetria. |
| parceiro-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 12/04/2025 15:21 | 12/04/2025 15:33 | Concluído 2025-04-26 | gateway-ops-01; auditou o guia ops do gateway + fluxo anônimo do proxy Experimente. |

Preencha os carimbos de data e hora de **Convite enviado** e **Ack** assim como o e-mail de saida para emissão.
Ancore os horários no cronograma UTC definido no plano W1.

## Pontos de verificação de telemetria

| Carimbo de data e hora (UTC) | Painéis/sondas | Responsável | Resultado | Artefato |
| --- | --- | --- | --- | --- |
| 06/04/2025 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Documentos/DevRel + Operações | Tudo verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 06/04/2025 18:20 | Transcrição de `npm run manage:tryit-proxy -- --stage preview-w1` | Operações | Encenado | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 12/04/2025 14:45 | Painéis acima + `probe:portal` | Documentos/DevRel + Operações | Pré-convite instantâneo, sem regressão | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 19/04/2025 17:55 | Dashboards acima + diferença de latência do proxy Experimente | Líder do Documentos/DevRel | Checkpoint de meio aprovado (0 alertas; latência Experimente p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 26/04/2025 16:25 | Dashboards acima + sonda de saida | Contato do Docs/DevRel + Governança | Instantâneo de Saida, zero alertas pendentes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

As amostras diárias de horário comercial (2025-04-13 -> 2025-04-25) estão agrupadas como exports NDJSON + PNG em
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` com nomes de arquivo
`docs-preview-integrity-<date>.json` e capturas de tela correspondentes.

## Log de feedback e problemas

Utilize esta tabela para resumo de resultados enviados por revisores. Vincule cada entrada ao ticket GitHub/discuss
mais o formulário estruturado capturado via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referência | Severidade | Responsável | Estado | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Baixo | Documentos-núcleo-02 | Resolvido 2025-04-18 | Esclareceu o texto de navegação do Try it + âncora de sidebar (`docs/source/sorafs/tryit.md` atualizado com novo rótulo). |
| `docs-preview/w1 #2` | Baixo | Documentos-núcleo-03 | Resolvido 2025-04-19 | Captura de tela do Try it + legenda atualizadas conforme pedido; arte `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Informações | Líder do Documentos/DevRel | Fechado | Os comentários restantes foram apenas perguntas e respostas; capturados no formulário de feedback de cada parceiro sob `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Acompanhamento de verificação de conhecimento e pesquisas1. Cadastre-se como notas do quiz (meta >=90%) para cada revisor; anexo o CSV exportado ao lado dos artistas de convite.
2. Colete as respostas qualitativas da pesquisa capturadas no template de feedback e espelhe em
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Agende chamadas de remediação para quem estiver abaixo do limite e registre-se aqui.

Todos os oito revisores marcaram >=94% sem verificação de conhecimento (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Nenhuma chamada de remediação
foi necessário; exportações de pesquisa para cada parceiro em vivem
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventário de artefactos

- Descritor/soma de verificação de visualização do pacote: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Resumo da sonda + verificação de link: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de mudança do proxy Experimente: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exportações de telemetria: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Pacote diário de telemetria de horário comercial: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exportações de feedback + pesquisa: colocar pastas por reviewer em
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV e resumo da verificação de conhecimento: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Mantenha o inventário sincronizado com o problema do rastreador. Anexo hashes ao copiar artefatos para o ticket de governança
para que os auditores verifiquem os arquivos sem acesso ao shell.