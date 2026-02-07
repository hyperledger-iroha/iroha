---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : runbooks-index
titre : Indice des runbooks des opérateurs
sidebar_label : Indice des runbooks
description : Pont d'entrée canonique pour les runbooks des opérateurs migrés SoraFS.
---

> Reflete o registre de responsáveis que fica em `docs/source/sorafs/runbooks/`.
> Chaque nouvelle guide d'opérations du SoraFS doit être vinculée ici comme étant publiée non
> créer un portail do.

Utilisez cette page pour vérifier quels runbooks concluent la migration de l'archive de documents
alternative pour le portail. Chaque entrée dans la liste des responsabilités, ou chemin d'origine canonique
et une copie sur le portail pour que les réviseurs puissent les diriger directement vers le guide souhaité pendant la version bêta précédente.

## Hôte de la version bêta précédente

Aujourd'hui, DocOps a déjà promu un hôte bêta approuvé pour les réviser
`https://docs.iroha.tech/`. Pour diriger les opérateurs ou les réviseurs pour la migration du runbook,
Faites référence au nom d'hôte pour utiliser l'instantané du portail protégé par la somme de contrôle.
Les procédures de publication/rollback sont en cours
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Dossier d'exécution | Responsável(est) | Copie sans portail | Fonte |
|---------|-----------------|-------|-------|
| Coup d'envoi de la passerelle et du DNS | Mise en réseau TL, automatisation des opérations, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Playbook des opérations du SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Réconciliation des capacités | Trésorerie / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Opérations d'enregistrement des broches | GT Outillage | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Liste de contrôle des opérations du nó | Équipe Stockage, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook des litiges et des commentaires | Conseil de gouvernance | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Playbook de manifeste de mise en scène | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilité de l'ancienne Taikai | WG Plateforme Média / Programme DA / Réseautage TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Checklist de vérification

- [x] Une construction du portail aponta para este índice (entrada na barra lateral).
- [x] Chaque runbook migré liste le chemin d'origine canonique pour gérer les réviseurs
  alinhados durante as revisões de documentação.
- [x] Le pipeline de prévia do DocOps bloque la fusion lorsqu'un runbook est répertorié
  ausente da saída do portail.

Les migrations futures (par exemple, de nouvelles simulations de chaos ou des annexes de gouvernance) se développent
Ajouter une ligne à un tableau précis et mettre à jour une liste de contrôle DocOps intégrée dans
`docs/examples/docs_preview_request_template.md`.