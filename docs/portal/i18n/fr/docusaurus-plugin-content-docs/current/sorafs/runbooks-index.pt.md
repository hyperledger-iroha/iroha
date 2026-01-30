---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: runbooks-index
title: Índice de runbooks de operadores
sidebar_label: Índice de runbooks
description: Ponto de entrada canônico para os runbooks de operadores do SoraFS migrados.
---

> Reflete o registro de responsáveis que fica em `docs/source/sorafs/runbooks/`.
> Cada novo guia de operações do SoraFS deve ser vinculado aqui assim que for publicado no
> build do portal.

Use esta página para verificar quais runbooks concluíram a migração da árvore de docs
alternativa para o portal. Cada entrada lista a responsabilidade, o caminho de origem canônico
e a cópia no portal para que revisores possam ir direto ao guia desejado durante a prévia beta.

## Host de prévia beta

A onda DocOps já promoveu o host de prévia beta aprovado pelos revisores em
`https://docs.iroha.tech/`. Ao direcionar operadores ou revisores para um runbook migrado,
referencie esse hostname para que usem o snapshot do portal protegido por checksum.
Os procedimentos de publicação/rollback estão em
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Runbook | Responsável(is) | Cópia no portal | Fonte |
|---------|-----------------|-----------------|-------|
| Kickoff de gateway e DNS | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Playbook de operações do SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Reconciliação de capacidade | Treasury / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operações do registro de pins | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Checklist de operações de nó | Storage Team, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook de disputas e revogações | Governance Council | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de manifesto de staging | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilidade da âncora Taikai | Media Platform WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Checklist de verificação

- [x] A build do portal aponta para este índice (entrada na barra lateral).
- [x] Cada runbook migrado lista o caminho de origem canônico para manter os revisores
  alinhados durante as revisões de documentação.
- [x] O pipeline de prévia do DocOps bloqueia merges quando um runbook listado estiver
  ausente da saída do portal.

Migrações futuras (por exemplo, novas simulações de caos ou apêndices de governança) devem
adicionar uma linha à tabela acima e atualizar a checklist do DocOps embutida em
`docs/examples/docs_preview_request_template.md`.
