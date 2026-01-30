---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w0-summary
title: Resumo de feedback do meio do W0
sidebar_label: Feedback W0 (meio)
description: Checkpoints, achados e acoes de meio de onda para a onda de preview de mantenedores core.
---

| Item | Detalhes |
| --- | --- |
| Onda | W0 - Mantenedores core |
| Data do resumo | 2025-03-27 |
| Janela de revisao | 2025-03-25 -> 2025-04-08 |
| Participantes | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| Tag de artefato | `preview-2025-03-24` |

## Destaques

1. **Fluxo de checksum** - Todos os reviewers confirmaram que `scripts/preview_verify.sh`
   teve sucesso contra o par descriptor/archive compartilhado. Nenhum override manual foi
   necessario.
2. **Feedback de navegacao** - Dois problemas menores de ordenacao do sidebar foram
   registrados (`docs-preview/w0 #1-#2`). Ambos foram encaminhados para Docs/DevRel e nao
   bloqueiam a onda.
3. **Paridade de runbooks SoraFS** - sorafs-ops-01 pediu links cruzados mais claros entre
   `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. Issue de acompanhamento aberta;
   tratar antes de W1.
4. **Revisao de telemetria** - observability-01 confirmou que `docs.preview.integrity`,
   `TryItProxyErrors` e os logs do proxy Try-it ficaram verdes; nenhum alerta disparou.

## Itens de acao

| ID | Descricao | Responsavel | Status |
| --- | --- | --- | --- |
| W0-A1 | Reordenar entradas do sidebar do devportal para destacar docs focados em reviewers (`preview-invite-*` agrupados). | Docs-core-01 | Concluido - o sidebar agora lista os docs de reviewers de forma contigua (`docs/portal/sidebars.js`). |
| W0-A2 | Adicionar link cruzado explicito entre `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Concluido - cada runbook agora aponta para o outro para que operadores vejam ambos os guias durante rollouts. |
| W0-A3 | Compartilhar snapshots de telemetria + bundle de queries com o tracker de governanca. | Observability-01 | Concluido - bundle anexado ao `DOCS-SORA-Preview-W0`. |

## Resumo de encerramento (2025-04-08)

- Todos os cinco reviewers confirmaram a conclusao, limparam builds locais e sairam da janela
  de preview; as revogacoes de acesso ficaram registradas em `DOCS-SORA-Preview-W0`.
- Nenhum incidente ou alerta ocorreu durante a onda; os dashboards de telemetria ficaram
  verdes durante todo o periodo.
- As acoes de navegacao + links cruzados (W0-A1/A2) estao implementadas e refletidas nos docs
  acima; a evidencia de telemetria (W0-A3) esta anexada ao tracker.
- Bundle de evidencia arquivado: screenshots de telemetria, confirmacoes de convite e este
  resumo estao linkados no issue do tracker.

## Proximos passos

- Implementar os itens de acao do W0 antes de abrir W1.
- Obter aprovacao legal e um slot de staging para o proxy, depois seguir os passos de preflight
  da onda de parceiros descritos no [preview invite flow](../../preview-invite-flow.md).

_Este resumo esta linkado a partir do [preview invite tracker](../../preview-invite-tracker.md) para
manter o roadmap DOCS-SORA rastreavel._
