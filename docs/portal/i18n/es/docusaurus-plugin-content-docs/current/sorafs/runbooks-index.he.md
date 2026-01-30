---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/runbooks-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 661a6fab22ec1049d6bca1b8405169e77246b2e2043a77453a825cb18d6a6ddb
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
id: runbooks-index
lang: es
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Índice de runbooks de operadores
sidebar_label: Índice de runbooks
description: Punto de entrada canónico para los runbooks de operadores de SoraFS migrados.
---

> Refleja el registro de responsables que vive en `docs/source/sorafs/runbooks/`.
> Cada nueva guía de operaciones de SoraFS debe enlazarse aquí una vez que se publique en
> el build del portal.

Usa esta página para verificar qué runbooks han completado la migración desde el
árbol de documentación heredado al portal. Cada entrada enumera la titularidad, la
ruta de origen canónica y la copia en el portal para que los revisores puedan
saltar directamente a la guía deseada durante la vista previa beta.

## Host de vista previa beta

La oleada DocOps ya promovió el host de vista previa beta aprobado por los revisores en
`https://docs.iroha.tech/`. Al dirigir a operadores o revisores a un runbook migrado,
referencia ese hostname para que usen la instantánea del portal protegida por checksum.
Los procedimientos de publicación/rollback viven en
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Runbook | Propietario(s) | Copia en el portal | Fuente |
|---------|----------------|-------------------|--------|
| Arranque de gateway y DNS | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Playbook de operaciones de SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Conciliación de capacidad | Treasury / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operaciones del registro de pins | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Checklist de operaciones de nodo | Storage Team, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook de disputas y revocaciones | Governance Council | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de manifiestos en staging | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilidad del ancla Taikai | Media Platform WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Checklist de verificación

- [x] El build del portal enlaza este índice (entrada en la barra lateral).
- [x] Cada runbook migrado enumera la ruta de origen canónica para mantener a los revisores
  alineados durante las revisiones de documentación.
- [x] El pipeline de vista previa de DocOps bloquea los merges cuando falta un runbook
  listado en la salida del portal.

Las migraciones futuras (p. ej., nuevos simulacros de caos o apéndices de gobernanza)
deben añadir una fila a la tabla anterior y actualizar la checklist de DocOps incorporada en
`docs/examples/docs_preview_request_template.md`.
