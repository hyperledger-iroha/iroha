---
lang: es
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
descripción: Punto de entrada canónico para los runbooks de operadores do SoraFS migrados.
---

> Rellene el registro de respuestas que fica en `docs/source/sorafs/runbooks/`.
> Cada nueva guía de operaciones de SoraFS debe estar vinculada aquí así que para publicado no
> construir el portal.

Utilice esta página para verificar qué runbooks y concluir la migración al archivo de documentos.
alternativa para el portal. Cada entrada lista a responsabilidad, o camino de origen canónico
e a cópia no portal para que revisores possam ir directamente ao guia desejado durante la versión beta anterior.

## Host de la versión beta anterior

A onda DocOps já promoveu o host de prévia beta aprovado pelos revisores em
`https://docs.iroha.tech/`. Para dirigir operadores o revisores para un runbook migrado,
Haga referencia a ese nombre de host para utilizar la instantánea en el portal protegido por suma de comprobación.
Los procedimientos de publicación/rollback están en
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).| Libro de ejecución | Responsável(es) | Copia en el portal | Fuente |
|---------|-----------------|-----------------|-------|
| Lanzamiento de gateway y DNS | TL de redes, automatización de operaciones, documentos/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Manual de operaciones de SoraFS | Documentos/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Reconciliación de capacidad | Tesorería/SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operaciones de registro de pines | Grupo de Trabajo sobre Herramientas | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Lista de verificación de operaciones de nó | Equipo de Almacenamiento, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook de disputas y revogações | Consejo de Gobierno | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de manifiesto de puesta en escena | Documentos/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilidades de la áncora Taikai | Media Platform WG / Programa DA / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Lista de verificación de verificación

- [x] A build do portal aponta para este índice (entrada na barra lateral).
- [x] Cada runbook migrado lista o camino de origen canónico para manter os revisores
  alinhados durante as revisões de documentação.
- [x] La canalización previa del bloque DocOps se fusiona cuando se publica un runbook listado
  ausente da saída do portal.Migrações futuras (por ejemplo, nuevas simulações de caos o apéndices de gobierno) devem
Agregar una línea a la tabla acima y actualizar la lista de verificación de DocOps embutida en
`docs/examples/docs_preview_request_template.md`.