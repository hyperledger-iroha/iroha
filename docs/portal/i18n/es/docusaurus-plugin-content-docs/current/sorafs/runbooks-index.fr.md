---
lang: es
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
descripción: Punto de entrada canónico para los operadores de runbooks SoraFS migratorios.
---

> Reflète le registre des responsables qui se trouve dans `docs/source/sorafs/runbooks/`.
> Guía de explotación Chaque nouveau SoraFS hazlo aquí para saber lo que está publicado en
> la construcción del portal.

Utilice esta página para verificar los runbooks al finalizar la migración de la arborescencia.
de docs heritée vers le portail. Cada plato principal indica la responsabilidad, le chemin source
Canonique et la copie portail afin que les relecteurs puissent accéder directement au Guide
souhaité colgante l'aperçu bêta.

## Hôte d'aperçu bêta

La vaga DocOps se ha desormado porque el host de apertura debe ser aprobado por los lectores
`https://docs.iroha.tech/`. Lorsque vous dirigez des opérateurs ou des relecteurs vers un
runbook migré, referencia ce nom d'hôte afin qu'ils consultor l'instantané du portail
protegido por suma de comprobación. Los procedimientos de publicación/reversión se encuentran en
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).| Libro de ejecución | Responsable(s) | Copiar portal | Fuente |
|---------|----------------|-------------|--------|
| Puerta de enlace Lancement y DNS | TL de redes, automatización de operaciones, documentos/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Libro de jugadas de explotación SoraFS | Documentos/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Reconciliación de capacidades | Tesorería/SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operaciones de registro de pines | Grupo de Trabajo sobre Herramientas | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Lista de control de explotación de nuevos habitantes | Equipo de Almacenamiento, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook litigios y revocaciones | Consejo de Gobierno | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Manual de manifiesto de puesta en escena | Documentos/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilidad del ancre Taikai | Media Platform WG / Programa DA / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Lista de verificación de verificación

- [x] La build du portail renvoie vers cet index (entrada de la barra lateral).
- [x] Chaque runbook migré liste le chemin source canonique pour garder les lecteurs
  alignés lors des revues de documentación.
- [x] La tubería de acceso DocOps bloquea las fusiones cuando un runbook está listo en la
  Salida del portal.Les migraciones futuras (p. ej. nuevos ejercicios de caos o anexos de gobierno)
Asegúrese de agregar una línea a la tabla ci-dessus y agregar cada día la lista de verificación DocOps integrada en
`docs/examples/docs_preview_request_template.md`.