---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w0-resumo
título: Resumo de feedback do meio do W0
sidebar_label: Feedback W0 (meio)
description: Checkpoints, achados e ações de meio de onda para a onda de visualização de mantenedores core.
---

| Artigo | Detalhes |
| --- | --- |
| Onda | W0 - Núcleo de Mantenedores |
| Dados do resumo | 27/03/2025 |
| Janela de revisão | 25/03/2025 -> 08/04/2025 |
| Participantes | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilidade-01 |
| Etiqueta de arte | `preview-2025-03-24` |

##Destaques

1. **Fluxo de checksum** - Todos os revisores confirmaram que `scripts/preview_verify.sh`
   teve sucesso contra o par descritor/arquivo compartilhado. Nenhuma substituição manual foi
   necessário.
2. **Feedback de navegação** - Dois problemas menores de ordenação da barra lateral foram
   registrados (`docs-preview/w0 #1-#2`). Ambos foram encaminhados para Docs/DevRel e não
   bloqueie a onda.
3. **Paridade de runbooks SoraFS** - sorafs-ops-01 pediu links cruzados mais claros entre
   `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. Emissão de acompanhamento aberto;
   tratar antes de W1.
4. **Revisão de telemetria** - observability-01 confirmado que `docs.preview.integrity`,
   `TryItProxyErrors` e os logs do proxy Try-it ficaram verdes; nenhum alerta disparou.

## Itens de ação

| ID | Descrição | Responsável | Estado |
| --- | --- | --- | --- |
| W0-A1 | Reordenar entradas da barra lateral do devportal para destacar documentos focados em revisores (`preview-invite-*` agrupados). | Documentos-núcleo-01 | Concluído - a barra lateral agora lista os documentos de revisores de forma contígua (`docs/portal/sidebars.js`). |
| W0-A2 | Adicione link cruzado explícito entre `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Concluido - cada runbook agora aponta para o outro para que os operadores vejam ambos os guias durante os lançamentos. |
| W0-A3 | Compartilhe snapshots de telemetria + pacote de consultas com o tracker de governança. | Observabilidade-01 | Concluído - pacote anexado ao `DOCS-SORA-Preview-W0`. |

## Resumo de encerramento (2025-04-08)

- Todos os cinco revisores confirmaram a conclusão, limparam builds locais e saíram da janela
  de visualização; as revogações de acesso ficaram registradas em `DOCS-SORA-Preview-W0`.
- Nenhum incidente ou alerta ocorreu durante a onda; os dashboards de telemetria ficaram
  verdes durante todo o período.
- As ações de navegação + links cruzados (W0-A1/A2) estao inovadores e refletidos nos docs
  acima; a evidência de telemetria (W0-A3) está anexada ao rastreador.
- Pacote de evidências arquivado: capturas de tela de telemetria, confirmações de convite e este
  resumo estao linkados no issue do tracker.

## Próximos passos

- Implementar os itens de ação do W0 antes de abrir o W1.
- Obter aprovação legal e um slot de staging para o proxy, depois seguir os passos de comprovação
  da onda de parceiros descritos no [fluxo de convite de visualização](../../preview-invite-flow.md).

_Este resumo está linkado a partir do [preview Invitation Tracker](../../preview-invite-tracker.md) para
manter o roteiro DOCS-SORA rastreavel._