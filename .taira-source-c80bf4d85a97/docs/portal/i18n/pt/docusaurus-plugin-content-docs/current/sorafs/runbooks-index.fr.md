---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice de runbooks
título: Índice de operadores de runbooks
sidebar_label: Índice de runbooks
descrição: Ponto de entrada canônico para os operadores de runbooks SoraFS migrados.
---

> Reflita sobre o registro dos responsáveis que você encontrou em `docs/source/sorafs/runbooks/`.
> Cada novo guia de exploração SoraFS deve ser lido aqui do que foi publicado em
> la build du portal.

Use esta página para verificar se os runbooks terminaram a migração da árvore
dos documentos herdados para o portal. Chaque entrée indique la responsabilidade, le chemin source
canônico e a cópia do portal para que os leitores possam acessar diretamente o guia
souhaité pendente l’aperçu bêta.

## Hôte d’aperçu bêta

O vago DocOps foi desormado promu l’hôte d’aperçu beta aprovado pelos relecteurs para
`https://docs.iroha.tech/`. Quando você dirige operadores ou revisores contra um
runbook migrado, consulte este nome de hotel para quem consulta o instante do portal
protegido por soma de verificação. Os procedimentos de publicação/reversão foram encontrados em
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Caderno | Responsáveis(es) | Copiar portal | Fonte |
|--------|----------------|-------------|--------|
| Gateway de lançamento e DNS | TL de rede, automação de operações, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Manual de exploração SoraFS | Documentos/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Reconciliação de capacidade | Tesouraria / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operações de registro de pinos | GT Ferramentaria | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Checklist d’exploitation des noeuds | Equipe de armazenamento, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook litígios e revogações | Conselho de Governança | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Manual de manifesto de encenação | Documentos/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilidade do ano Taikai | GT Plataforma de Mídia / Programa DA / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Checklist de verificação

- [x] A construção do portal reenviado para este índice (entrada da barra lateral).
- [x] Cada runbook migrou para a lista de fontes canônicas para proteger os leitores
  alinhados pelas revistas de documentação.
- [x] O pipeline de abertura do DocOps bloqueia as mesclagens em um runbook listado anteriormente
  surtida do portal.

Les migrações futuras (p. ex. novos exercícios de caos ou anexos de governo)
Devo adicionar uma linha ao quadro ci-dessus e adicionar hoje a lista de verificação DocOps integrada em
`docs/examples/docs_preview_request_template.md`.