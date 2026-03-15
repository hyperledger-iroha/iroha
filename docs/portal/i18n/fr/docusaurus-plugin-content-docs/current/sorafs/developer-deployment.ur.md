---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement par le développeur
titre : Notes de déploiement SoraFS
sidebar_label : notes de déploiement
description : CI pour la production et le pipeline SoraFS pour promouvoir la liste de contrôle
---

:::note مستند ماخذ
:::

# Notes de déploiement

SoraFS Déterminisme du flux de travail d'emballage pour les garde-corps opérationnels de production et de CI Il s'agit de passerelles et de fournisseurs de stockage et de déploiement d'une liste de contrôle pour les fournisseurs de stockage.

## Pré-vol

- **Alignement du registre** — Les profils de chunker se manifestent par un tuple `namespace.name@semver` et se réfèrent à un tuple (`docs/source/sorafs/chunker_registry.md`).
- **Politique d'admission** — `manifest submit` pour les annonces de fournisseurs signées et les preuves d'alias pour les annonces (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook de registre Pin** — scénarios de récupération (rotation d'alias, échecs de réplication)

##Configuration de l'environnement

- Les passerelles et le point de terminaison de streaming de preuve (`POST /v2/sorafs/proof/stream`) permettent aux résumés de télémétrie CLI d'émettre des messages instantanés.
- Politique `sorafs_alias_cache` et valeurs par défaut `iroha_config` et assistant CLI (`sorafs_cli manifest submit --alias-*`) pour configurer la configuration
- Jetons de flux (identifiants Torii) et gestionnaire de secrets pour le gestionnaire de secrets
- Les exportateurs de télémétrie (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) permettent d'expédier la pile Prometheus/OTel pour expédier des fichiers.

## Stratégie de déploiement1. **Manifestes bleu/vert**
   - Déploiement des archives des réponses pour `manifest submit --summary-out` Mise à jour
   - `torii_sorafs_gateway_refusals_total` pour les incohérences de capacité de la version actuelle
2. **Validation de la preuve**
   - `sorafs_cli proof stream` échecs et bloqueurs de déploiement pics de latence, limitation du fournisseur et niveaux mal configurés
   - Test de fumée post-pin avec `proof verify` pour les fournisseurs de CAR hébergés et le résumé du manifeste et la correspondance avec les fournisseurs
3. **Tableaux de bord de télémétrie**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` et Grafana pour importer des fichiers
   - État du registre des broches (`docs/source/sorafs/runbooks/pin_registry_ops.md`) et statistiques de plage de blocs pour les panneaux et les panneaux
4. **Activation multi-source**
   - Orchestrator est en charge du `docs/source/sorafs/runbooks/multi_source_rollout.md` et des étapes de déploiement par étapes pour les audits et les archives du tableau de bord/des artefacts de télémétrie.

## Gestion des incidents

- `docs/source/sorafs/runbooks/` pour les chemins d'escalade suivants :
  - Pannes de passerelle `sorafs_gateway_operator_playbook.md` et épuisement du jeton de flux
  - `dispute_revocation_runbook.md` pour les litiges de réplication
  - Maintenance au niveau du nœud `sorafs_node_ops.md`
  - Remplacements de l'orchestrateur `multi_source_rollout.md`, liste noire des pairs et déploiements par étapes
- Échecs de preuve et anomalies de latence et GovernanceLog et les API de suivi PoR enregistrent et évaluent les performances du fournisseur de gouvernance.

## Étapes suivantes- L'orchestrateur de récupération multi-sources (SF-6b) et l'automatisation de l'orchestrateur (`sorafs_car::multi_fetch`) intègrent des fonctionnalités
- Mises à niveau PDP/PoTR pour SF-13/SF-14 pour les pistes de suivi Les preuves se stabilisent dans la CLI et les délais des documents et la surface de sélection des niveaux sont également disponibles.

Notes de déploiement, démarrage rapide, recettes CI, expérimentations locales, pipelines SoraFS de qualité production, processus reproductibles et observables. ہیں۔