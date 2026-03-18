---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice de runbooks
título: Índice de runbooks de operadores
sidebar_label: Índice de runbooks
description: Ponto de entrada canônico para runbooks de operadores de SoraFS migrados.
---

> Reflita sobre o registro de responsáveis que vivem em `docs/source/sorafs/runbooks/`.
> Cada novo guia de operações de SoraFS deve ser lido aqui uma vez que for publicado em
> a construção do portal.

Use esta página para verificar quais runbooks completaram a migração do
árvore de documentação herdada do portal. Cada entrada enumera la titularidad, la
rota de origem canônica e a cópia no portal para que os revisores possam
saltar diretamente para o guia desejado durante a vista anterior beta.

## Host da vista anterior beta

O oleado DocOps promoveu o host da vista anterior beta aprovado pelos revisores em
`https://docs.iroha.tech/`. Ao dirigir operadores ou revisores de um runbook migrado,
refere-se a esse nome de host para usar o instante do portal protegido por checksum.
Os procedimentos de publicação/reversão ocorrem em
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Caderno | Proprietário(s) | Copiar no portal | Fonte |
|---------|----------------|-------------------|--------|
| Configuração de gateway e DNS | TL de rede, automação de operações, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Manual de operações de SoraFS | Documentos/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Conciliação de capacidade | Tesouraria / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operações de registro de pins | GT Ferramentaria | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Checklist de operações de nó | Equipe de armazenamento, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook de disputas e revogações | Conselho de Governança | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Manual de manifestação em encenação | Documentos/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilidade do anel Taikai | GT Plataforma de Mídia / Programa DA / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Checklist de verificação

- [x] A construção do portal enlaza é este índice (entrada na barra lateral).
- [x] Cada runbook migrado enumera a rota de origem canônica para manter os revisores
  alinhados durante as revisões da documentação.
- [x] O pipeline de vista anterior do DocOps bloqueia as mesclagens quando falta um runbook
  listado na saída do portal.

Las migraciones futuras (p. ej., novos simulacros de caos ou apêndices de governo)
você deve adicionar um arquivo à tabela anterior e atualizar a lista de verificação de DocOps incluída em
`docs/examples/docs_preview_request_template.md`.