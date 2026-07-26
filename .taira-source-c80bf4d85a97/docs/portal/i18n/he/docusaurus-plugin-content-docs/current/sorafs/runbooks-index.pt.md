---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: runbooks-index
כותרת: Índice de runbooks de operadores
sidebar_label: Índice de runbooks
תיאור: Ponto de entrada canônico para os runbooks de operadores do SoraFS migrados.
---

> Reflete o registro de responsáveis que fica em `docs/source/sorafs/runbooks/`.
> Cada novo guia de operações do SoraFS deve ser vinculado aqui assim que for publicado no
> לבנות פורטל לעשות.

השתמש ב-esta página para verificar quais runbooks concluíram a migração da árvore de docs
אלטרנטיבה לפורטל. Cada entrada list a responsabilidade, o caminho de origem canônico
e a cópia no portal para que revisores possam ir direto ao guia desejado durante a prévia beta.

## Host de prévia בטא

A onda DocOps já promoveu o host de prévia beta aprovado pelos revisores em
`https://docs.iroha.tech/`. Ao direcionar operators ou revisores para um runbook migrado,
התייחסות לשם מארח עבור שימוש בתמונת מצב לפורטל הגנה על סכום בדיקה.
Os procedimentos de publicação/rollback estão em
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| ספר הפעלה | Responsável(is) | קופיה אין פורטל | פונטה |
|--------|----------------|----------------|-------|
| בעיטה של ​​שער ו-DNS | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Playbook de operações do SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Reconciliação de capacidade | האוצר / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operações do registro de pins | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Checklist de operações de nó | צוות אחסון, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook de disputas e revogações | מועצת ממשל | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de Manifesto de Staging | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilidade da âncora Taikai | פלטפורמת מדיה WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## רשימת אימות

- [x] A build do portal aponta para este índice (entrada na barra lateral).
- [x] Cada runbook migrado list o caminho de origem canônico para manter os revisores
  alinhados durante as revisões de documentação.
- [x] O pipeline de prévia do DocOps bloqueia ממזג quando um runbook listado estiver
  ausente da saída do portal.

Migrações futuras (לדוגמה, novas simulações de caos או apêndices de governança) devem
adicionar uma linha à tabela acima e atualizar רשימת בדיקה לעשות DocOps embutida em
`docs/examples/docs_preview_request_template.md`.