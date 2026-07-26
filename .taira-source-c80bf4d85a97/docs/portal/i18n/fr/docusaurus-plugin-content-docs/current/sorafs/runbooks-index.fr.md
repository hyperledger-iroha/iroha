---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : runbooks-index
titre : Index des opérateurs de runbooks
sidebar_label : Index des runbooks
description : Point d’entrée canonique pour les runbooks opérateurs SoraFS migrés.
---

> Reflète le registre des responsables qui se trouve dans `docs/source/sorafs/runbooks/`.
> Chaque nouveau guide d’exploitation SoraFS doit être lié ici dès qu’il est publié dans
> la construction du portail.

Utilisez cette page pour vérifier quels runbooks ont terminé la migration de l’arborescence
de docs héritée vers le portail. Chaque entrée indique la responsabilité, le chemin source
canonique et la copie portail afin que les rélecteurs puissent directement au guide
souhaité pendant l’aperçu bêta.

## Hôte d’aperçu bêta

La vague DocOps a désormais promu l’hôte d’aperçu bêta approuvé par les rélecteurs à
`https://docs.iroha.tech/`. Lorsque vous dirigez des opérateurs ou des rélecteurs vers un
runbook migré, référencez ce nom d’hôte afin qu’ils consultent l’instantané du portail
protégé par somme de contrôle. Les procédures de publication/rollback se trouvent dans
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Dossier d'exploitation | Responsable(s) | Copier le portail | Source |
|---------|----------------|-------------|--------|
| Passerelle Lancement & DNS | Mise en réseau TL, automatisation des opérations, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Playbook d’exploitation SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Réconciliation de capacité | Trésorerie / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Opérations du registre de pins | GT Outillage | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Checklist d’exploitation des nœuds | Équipe Stockage, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Litiges et révocations du Runbook | Conseil de gouvernance | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de manifeste de mise en scène | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilité de l’ancre Taikai | WG Plateforme Média / Programme DA / Réseautage TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Checklist de vérification

- [x] La build du portail renvoie vers cet index (entrée de la barre latérale).
- [x] Chaque runbook migré liste le chemin source canonique pour garder les rélecteurs
  alignées lors des revues de documentation.
- [x] Le pipeline d’aperçu DocOps bloque les fusions lorsqu’un runbook listé manque dans la
  sortie du portail.

Les migrations futures (p. ex. nouveaux exercices de chaos ou annexes de gouvernance)
ajouter une ligne au tableau ci-dessus et mettre à jour la checklist doit DocOps intégrée dans
`docs/examples/docs_preview_request_template.md`.