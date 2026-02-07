---
lang: es
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
descripción: Punto de entrada canónico para los runbooks de operadores de SoraFS migrados.
---

> Refleja el registro de responsables que vive en `docs/source/sorafs/runbooks/`.
> Cada nueva guía de operaciones de SoraFS debe enlazarse aquí una vez que se publique en
> la construcción del portal.

Usa esta página para verificar qué runbooks han completado la migración desde el
árbol de documentación heredado al portal. Cada entrada enumera la titularidad, la
ruta de origen canónica y la copia en el portal para que los revisores puedan
saltar directamente a la guía deseada durante la vista previa beta.

## Anfitrión de vista previa beta

La oleada DocOps ya promovió el host de vista previa beta aprobado por los revisores en
`https://docs.iroha.tech/`. Al dirigir a operadores o revisores a un runbook migrado,
referencia ese nombre de host para que use la instantánea del portal protegido por checksum.
Los procedimientos de publicación/rollback viven en
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).| Libro de ejecución | Propietario(s) | Copia en el portal | Fuente |
|---------|----------------|-------------------|--------|
| Configuración de puerta de enlace y DNS | TL de redes, automatización de operaciones, documentos/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Manual de operaciones de SoraFS | Documentos/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Conciliación de capacidad | Tesorería/SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operaciones del registro de pines | Grupo de Trabajo sobre Herramientas | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Lista de verificación de operaciones de nodo | Equipo de Almacenamiento, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook de disputas y revocaciones | Consejo de Gobierno | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de manifiestos en puesta en escena | Documentos/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilidad del ancla Taikai | Media Platform WG / Programa DA / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Lista de verificación de verificación

- [x] El build del portal enlaza este índice (entrada en la barra lateral).
- [x] Cada runbook migrado enumera la ruta de origen canónica para mantener a los revisores
  alineados durante las revisiones de documentación.
- [x] El pipeline de vista previa de DocOps bloquea los merges cuando falta un runbook
  listado en la salida del portal.Las migraciones futuras (p. ej., nuevos simulacros de caos o apéndices de gobernanza)
deben agregar una fila a la tabla anterior y actualizar la lista de verificación de DocOps incorporada en
`docs/examples/docs_preview_request_template.md`.