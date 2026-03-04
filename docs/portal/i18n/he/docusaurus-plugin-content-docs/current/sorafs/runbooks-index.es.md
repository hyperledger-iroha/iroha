---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/runbooks-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: runbooks-index
כותרת: Índice de runbooks de operadores
sidebar_label: Índice de runbooks
תיאור: Punto de entrada canónico para los runbooks de operadores de SoraFS migrados.
---

> Refleja el registro de responsables que vive en `docs/source/sorafs/runbooks/`.
> Cada nueva guía de operaciones de SoraFS debe enlazarse aquí una vez que se publique en
> בניית הפורטל.

Usa esta página para verificar qué runbooks han completado la migración desde el
árbol de documentación heredado al portal. Cada entrada enumera la titularidad, la
ruta de origen canónica y la copia en el portal para que los revisores puedan
saltar directamente a la guía deseada durante la vista previa beta.

## Host de vista previa בטא

La oleada DocOps ya promovió el host de vista previa beta aprobado por los revisores en
`https://docs.iroha.tech/`. מנהלים או מבקרים ב-Runbook Migrado,
referencia ese שם מארח para que usen la instantánea del portal protegida por checksum.
Los procedimientos de publicación/rollback viven en
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Runbook | Propietario(s) | Copia en el portal | פואנטה |
|--------|----------------|----------------|--------|
| ערכת שער ו-DNS | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Playbook de operaciones de SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Conciliación de capacidad | האוצר / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operaciones del registro de pins | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| רשימת רשימת פעולות דה נודו | צוות אחסון, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook de disputas y revocaciones | מועצת ממשל | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de manifiestos in staging | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilidad del ancla Taikai | פלטפורמת מדיה WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## רשימת בדיקה לאימות

- [x] El build del portal enlaza este índice (entrada en la barra lateral).
- [x] Cada runbook migrado enumera la ruta de origen canónica para mantener a los revisores
  alineados durante las revisiones de documentación.
- [x] El pipeline de vista previa de DocOps bloquea los מיזוג cuando falta un runbook
  listdo en la salida del portal.

Las migraciones futuras (עמוד ej., nuevos simulacros de caos o apéndices de gobernanza)
deben añadir una fila a la tabla anterior y actualizar la checklist de DocOps incorporada en
`docs/examples/docs_preview_request_template.md`.