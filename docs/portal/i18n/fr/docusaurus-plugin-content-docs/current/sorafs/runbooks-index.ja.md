---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/runbooks-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 058ea0035948df19f2effd829ad660f7101886df954439a7bb49c639f6e99798
source_last_modified: "2025-11-14T04:43:22.301201+00:00"
translation_last_reviewed: 2026-01-30
---

> Reflète le registre des responsables qui se trouve dans `docs/source/sorafs/runbooks/`.
> Chaque nouveau guide d’exploitation SoraFS doit être lié ici dès qu’il est publié dans
> la build du portail.

Utilisez cette page pour vérifier quels runbooks ont terminé la migration de l’arborescence
de docs héritée vers le portail. Chaque entrée indique la responsabilité, le chemin source
canonique et la copie portail afin que les relecteurs puissent accéder directement au guide
souhaité pendant l’aperçu bêta.

## Hôte d’aperçu bêta

La vague DocOps a désormais promu l’hôte d’aperçu bêta approuvé par les relecteurs à
`https://docs.iroha.tech/`. Lorsque vous dirigez des opérateurs ou des relecteurs vers un
runbook migré, référencez ce nom d’hôte afin qu’ils consultent l’instantané du portail
protégé par checksum. Les procédures de publication/rollback se trouvent dans
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Runbook | Responsable(s) | Copie portail | Source |
|---------|----------------|-------------|--------|
| Lancement gateway & DNS | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Playbook d’exploitation SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Réconciliation de capacité | Treasury / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Ops du registre de pins | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Checklist d’exploitation des nœuds | Storage Team, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook litiges et révocations | Governance Council | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de manifeste de staging | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilité de l’ancre Taikai | Media Platform WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Checklist de vérification

- [x] La build du portail renvoie vers cet index (entrée de la barre latérale).
- [x] Chaque runbook migré liste le chemin source canonique pour garder les relecteurs
  alignés lors des revues de documentation.
- [x] Le pipeline d’aperçu DocOps bloque les merges lorsqu’un runbook listé manque dans la
  sortie du portail.

Les migrations futures (p. ex. nouveaux exercices de chaos ou annexes de gouvernance)
doivent ajouter une ligne au tableau ci-dessus et mettre à jour la checklist DocOps intégrée dans
`docs/examples/docs_preview_request_template.md`.
