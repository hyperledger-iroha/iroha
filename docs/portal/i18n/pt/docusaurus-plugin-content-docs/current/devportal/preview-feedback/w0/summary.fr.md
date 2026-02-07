---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w0-resumo
título: Digest des retours mi-parcours W0
sidebar_label: Retornos W0 (mi-parcours)
descrição: Pontos de controle, estatísticas e ações de meus caminhos para a vaga visualização do núcleo dos mantenedores.
---

| Elemento | Detalhes |
| --- | --- |
| Vago | W0 - Núcleo de mantenedores |
| Data do resumo | 27/03/2025 |
| Janela de revisão | 25/03/2025 -> 08/04/2025 |
| Participantes | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilidade-01 |
| Etiqueta do artefato | `preview-2025-03-24` |

## Pontos marinheiros

1. **Workflow de checksum** - Todos os revisores confirmaram que `scripts/preview_verify.sh`
   reussi contre le par descritor/archive partage. Aucun substituir manuel requis.
2. **Retours de navigation** - Deux problemes mineurs d'ordre du sidebar ont ete signales
   (`docs-preview/w0 #1-#2`). Os dois são rotas para Docs/DevRel e não são bloqueados
   vago.
3. **Parite des runbooks SoraFS** - sorafs-ops-01 a demande des liens croises plus clairs
   entre `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. Issue de suivi ouverte;
   um traidor avant W1.
4. **Revue de telemetrie** - observability-01 e confirma que `docs.preview.integrity`,
   `TryItProxyErrors` e os logs do proxy Try-it estão em repouso; nenhum alerta n'a
   ete declinchee.

## Ações

| ID | Descrição | Responsável | Estatuto |
| --- | --- | --- | --- |
| W0-A1 | Reordene as entradas da barra lateral do devportal para enviar antes dos documentos para revisores (`preview-invite-*` reagrupar). | Documentos-núcleo-01 | Termine - a barra lateral lista os revisores de documentos do facão adjacentes (`docs/portal/sidebars.js`). |
| W0-A2 | Adiciona uma garantia cruzada explícita entre `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Termine - cada runbook apontado para outro lado para que os operadores mostrem os dois guias durante as implementações. |
| W0-A3 | Compartilhe snapshots de telemetria + pacotes de solicitações com o rastreador de governança. | Observabilidade-01 | Termine - pacote anexado a `DOCS-SORA-Preview-W0`. |

## Currículo da surtida (08/04/2025)

- Os revisores do cinq não confirmaram o fim, limparam as construções locais e fecharam a janela de
  visualização; as revogações de acesso são registradas em `DOCS-SORA-Preview-W0`.
- Aucun incidente ni alerte pendente la vago; os painéis de telemetria são verdes
  pendente toute la periode.
- Les actions de navigation + liens croises (W0-A1/A2) são implementadas e refletidas em
  os documentos ci-dessus; A telemetria prévia (W0-A3) é anexada ao rastreador.
- Pacote de arquivo preliminar: capturas de tela de telemetria, acusações de convite e resumo
  sont lie depuis l'issue du tracker.

## Étapes de Prochaines

- Implemente as ações W0 antes de abrir W1.
- Obtenha a aprovação legal e um slot de teste para o proxy, depois siga as etapas de
  comprovação dos dados vagos detalhados no [fluxo de convite de visualização](../../preview-invite-flow.md).

_Ce digest est lie depuis le [rastreador de convite de pré-visualização](../../preview-invite-tracker.md) para
Garder le roadmap DOCS-SORA rastreável._