---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : runbooks-index
titre : Indice des runbooks des opérateurs
sidebar_label : Indice des runbooks
description : Point d'entrée canonique pour les runbooks des opérateurs de SoraFS migrés.
---

> Refleja el registro de responsables que vive en `docs/source/sorafs/runbooks/`.
> Ce nouveau guide d'opérations de SoraFS doit être téléchargé ici une fois qu'il est public en
> la construction du portail.

Utilisez cette page pour vérifier si les runbooks ont terminé la migration depuis le
arbre de documentation hérité du portail. Chaque fois que vous entrez dans l'énumération de la titularité, la
itinéraire d'origine canonique et copie sur le portail pour que les réviseurs puissent le faire
sauter directement au guide souhaité pendant la vue bêta précédente.

## Hôte de Vista version bêta

L'oleada DocOps a promu l'hôte de vue préliminaire bêta approuvé par les réviseurs de
`https://docs.iroha.tech/`. Pour diriger les opérateurs ou les réviseurs vers un runbook migré,
référence à ce nom d'hôte pour utiliser l'instantané du portail protégé par la somme de contrôle.
Les procédures de publication/rollback viven fr
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Dossier d'exploitation | Propriétaire(s) | Copie sur le portail | Source |
|---------|----------------|---------|--------|
| Arranque de passerelle et DNS | Mise en réseau TL, automatisation des opérations, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Playbook des opérations de SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Conciliation de capacité | Trésorerie / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Opérations du registre des épingles | GT Outillage | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Liste de contrôle des opérations de nœud | Équipe Stockage, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook des litiges et des révocations | Conseil de gouvernance | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de manifestes en mise en scène | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilité de l'année Taikai | WG Plateforme Média / Programme DA / Réseautage TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Checklist de vérification

- [x] El build del portal enlaza este índice (entrée dans la barre latérale).
- [x] Chaque runbook migré énumère la route d'origine canonique pour maintenir les réviseurs
  alignés lors des révisions de la documentation.
- [x] Le pipeline de vue préliminaire de DocOps bloque les fusions si un runbook échoue
  listé sur la sortie du portail.

Las migraciones futuresas (p. ej., nuevos simulacros de caos o appendices de gobernanza)
devez ajouter un fil à la table précédente et actualiser la liste de contrôle de DocOps incorporée dans
`docs/examples/docs_preview_request_template.md`.