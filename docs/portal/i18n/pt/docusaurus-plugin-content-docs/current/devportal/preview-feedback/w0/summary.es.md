---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w0-resumo
título: Resumo de feedback da metade de W0
sidebar_label: Feedback W0 (metad)
description: Pontos de controle, hallazgos e ações de metade de ola para a ola de visualização de mantenedores core.
---

| Artigo | Detalhes |
| --- | --- |
| Olá | W0 - Núcleo de Mantenedores |
| Data do currículo | 27/03/2025 |
| Ventana de revisão | 25/03/2025 -> 08/04/2025 |
| Participantes | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilidade-01 |
| Etiqueta de artefato | `preview-2025-03-24` |

## Destacados

1. **Flujo de checksum** - Todos os revisores confirmam que `scripts/preview_verify.sh`
   sua saída contra o descritor/arquivo compartilhado. Não é necessário
   substitui manuais.
2. **Feedback de navegação** - Registrar os problemas menores na ordem da barra lateral
   (`docs-preview/w0 #1-#2`). Ambos foram atribuídos ao Docs/DevRel e não bloquearam
   ola.
3. **Paridade de runbooks de SoraFS** - sorafs-ops-01 pidio links cruzados mas claros
   entre `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. Se abra um
   emissão de acompanhamento; se atendeu antes do W1.
4. **Revisão de telemetria** - observability-01 confirmado que `docs.preview.integrity`,
   `TryItProxyErrors` e os logs do proxy Try-it são mantidos em verde; não, sim
   disparar alertas.

## Ações

| ID | Descrição | Responsável | Estado |
| --- | --- | --- | --- |
| W0-A1 | Reordenar entradas da barra lateral do devportal para destacar documentos enfocados em revisores (`preview-invite-*` agrupados). | Documentos-núcleo-01 | Completado - a barra lateral agora lista os documentos de revisores de forma contínua (`docs/portal/sidebars.js`). |
| W0-A2 | Agregar link cruzado explícito entre `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Completado - cada runbook agora colocado em outro para que os operadores vejam ambas as guias durante as implementações. |
| W0-A3 | Compartilhe snapshots de telemetria + pacote de consultas com o rastreador de governo. | Observabilidade-01 | Completado - pacote anexado a `DOCS-SORA-Preview-W0`. |

## Resumo de cierre (08/04/2025)

- Os cinco revisores confirmam a finalização, limpiaron builds locales e salieron de la
  janela de visualização; as revogações de acesso serão registradas em `DOCS-SORA-Preview-W0`.
- Não há incidentes nem alertas durante a ola; os painéis de telemetria se mantuvieron
  em verde todo o período.
- As ações de navegação + links cruzados (W0-A1/A2) foram inovadoras e reflejadas em
  os documentos de arriba; a evidência de telemetria (W0-A3) é adjunta ao rastreador.
- Pacote de evidências arquivadas: capturas de tela de telemetria, acusações de convite e este
  o currículo está enlaçado desde a edição do rastreador.

## Seguintes passos

- Implementar os itens de ação do W0 antes de abrir o W1.
- Obtenha aprovação legal e um slot de teste para o proxy, depois siga os passos de
  preflight da ola de parceiros detalhados no [visualizar fluxo de convite](../../preview-invite-flow.md).

_Este currículo está enlazado desde o [rastreador de convite de pré-visualização](../../preview-invite-tracker.md) para
manter o roteiro DOCS-SORA trazível._