---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice de runbooks
título: Índice de runbooks de operadores
sidebar_label: Índice de runbooks
description: Ponto de entrada canônico para os runbooks de operadores do SoraFS migrados.
---

> Reflete o registro de responsáveis que fica em `docs/source/sorafs/runbooks/`.
> Cada novo guia de operações do SoraFS deve ser garantido aqui assim que for publicado no
> construir o portal.

Use esta página para verificar quais runbooks concluíram a migração da árvore de documentos
alternativa para o portal. Cada entrada lista a responsabilidade, o caminho de origem canônico
e a cópia no portal para que os revisores possam ir direto ao guia desejado durante a versão beta prévia.

## Host da versão beta prévia

A onda DocOps já promoveu o host de beta prévio aprovado pelos revisores em
`https://docs.iroha.tech/`. Ao direcionar operadores ou revisores para um runbook migrado,
referencie esse hostname para que use o snapshot do portal protegido por checksum.
Os procedimentos de publicação/rollback estão em
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Caderno | Responsável(é) | Cópia no portal | Fonte |
|--------|-----------------|-----------------|-------|
| Início do gateway e DNS | TL de rede, automação de operações, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Manual de operações do SoraFS | Documentos/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Reconciliação de capacidade | Tesouraria / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operações de registro de pins | GT Ferramentaria | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Checklist de operações de nó | Equipe de armazenamento, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Manual de disputas e revogações | Conselho de Governança | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Manual de manifesto de encenação | Documentos/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilidade da Âncora Taikai | GT Plataforma de Mídia / Programa DA / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Checklist de verificação

- [x] A build do portal aponta para este índice (entrada na barra lateral).
- [x] Cada runbook migrado lista o caminho de origem canônico para manter os revisores
  entregues durante as revisões de documentação.
- [x] O pipeline de previsão do DocOps bloqueia mesclagens quando um runbook listado estiver
  ausente da saída do portal.

Migrações futuras (por exemplo, novas simulações de caos ou apêndices de governança) devem
adicione uma linha à tabela acima e atualize a checklist do DocOps incorporado em
`docs/examples/docs_preview_request_template.md`.