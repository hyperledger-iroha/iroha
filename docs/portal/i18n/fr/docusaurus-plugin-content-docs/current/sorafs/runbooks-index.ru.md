---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : runbooks-index
titre : Индекс операторских ранбуков
sidebar_label : Index des catégories
description: Каноническая точка входа для мигрированных операторских ранбуков SoraFS.
---

> Ouvrez le dossier de recherche qui se trouve dans `docs/source/sorafs/runbooks/`.
> Le nouveau produit pour l'exploitation SoraFS doit être acheté après la publication dans
> сборке портала.

Utilisez cette étape pour vérifier que les jeux de hasard ont réussi à migrer vers votre entreprise
Vous avez des documents sur le portail. Quand il s'agit de la vérité, l'histoire canonique
et copiez-les sur le portail, ce que vous pouvez voir avant le prochain lancement de la version bêta.

## Хост бета‑превью

DocOps a publié des versions bêta de l'hôte `https://docs.iroha.tech/`.
Lorsque vous souhaitez que les opérateurs ou les rapports sur le réseau de migration, utilisez-le ici
хоста, чтобы они работали со снимком портала, защищённым контрольной суммой. Procédures
publications/informations sur le sujet
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Ranbouk | Владельцы | Copie sur le portail | Источник |
|--------|-----------|-----------------|--------------|
| Ajouter une passerelle et DNS | Mise en réseau TL, automatisation des opérations, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Плейбук операций SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Сверка ёмкости | Trésorerie / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Exploitation de restaurants | GT Outillage | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Чекlist операций узла | Équipe Stockage, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Ранбук споров и отзывов | Conseil de gouvernance | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Плейбук mise en scène-manifestations | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Наблюдаемость якоря Taikai | WG Plateforme Média / Programme DA / Réseautage TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Liste des preuves

- [x] Le portail est connecté à cet index (élément sous les panneaux).
- [x] Le réseau migratoire ouvre la voie canonique à l'histoire, ce qui se passe
  les rapports sur les documents relatifs à la révision des documents.
- [x] Le projet précédent le projet DocOps bloque la session, mais il est déjà terminé.
  Le service est disponible sur votre portail.

Les migrations planifiées (par exemple, les nouvelles tâches ou les applications à mettre en œuvre) doivent être effectuées
cliquez sur le tableau et ouvrez la liste DocOps, basée sur
`docs/examples/docs_preview_request_template.md`.