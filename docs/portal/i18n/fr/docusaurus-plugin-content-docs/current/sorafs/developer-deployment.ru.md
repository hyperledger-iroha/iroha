---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement par le développeur
titre : Demandes de mise à jour SoraFS
sidebar_label : Notes pour la mise à jour
description : Tableau de commande pour la carte de production SoraFS de CI dans le produit.
---

:::note Канонический источник
:::

# Paramètres de conversion

Le panneau de commande SoraFS utilise des mesures de sécurité avant l'utilisation de CI dans le cadre d'un travail normal de garde-corps. Utilisez cette liste pour déployer les outils de passerelles et de fournisseurs de stockage réels.

## Предварительная проверка

- **Выравнивание реестра** — vérifiez que le profil chunker et les manifestes sont sur le court-circuit `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Admission politique** — Vérifiez les annonces de fournisseurs et les preuves d'alias, non disponibles pour `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Registre des broches Runbook** : utilisez `docs/source/sorafs/runbooks/pin_registry_ops.md` pour la configuration du scénario (alias de rotation, réplication).

## Configuration de la configuration

- Les passerelles permettent de diffuser le streaming à l'épreuve des points de terminaison (`POST /v1/sorafs/proof/stream`), la CLI peut également utiliser les paramètres télémétriques.
- Enregistrez la politique `sorafs_alias_cache` en utilisant l'assistant `iroha_config` ou CLI (`sorafs_cli manifest submit --alias-*`).
- Ajoutez des jetons de flux (ou des données privées Torii) à votre gestionnaire de secrets.
- Fermez les exportateurs télémétriques (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) et ouvrez-les sur votre site Prometheus/OTel.## Déploiement de la stratégie

1. **Manifestes bleu/vert**
   - Utilisez `manifest submit --summary-out` pour le déploiement de l'archivage.
   - Placez-vous sur `torii_sorafs_gateway_refusals_total` pour que vous puissiez aimer votre travail.
2. **Preuves de vérification**
   - Vérifiez les blocs dans `sorafs_cli proof stream`; Les problèmes latents doivent limiter les fournisseurs de limitation ou les niveaux jamais définis.
   - `proof verify` doit être vérifié lors du test de fumée après la broche, afin d'être utilisé par le fournisseur de la voiture qui est également compatible avec le manuel de digestion.
3. **Tableaux de bord télémétriques**
   - Importez `docs/examples/sorafs_proof_streaming_dashboard.json` vers Grafana.
   - Créez des panneaux pour le registre de broches (`docs/source/sorafs/runbooks/pin_registry_ops.md`) et la plage de blocs statistiques.
4. **Включение multi-source**
   - Déployez le déploiement de l'étape `docs/source/sorafs/runbooks/multi_source_rollout.md` en activant Orchestrator et en archivant les éléments de tableau de bord/télémétrie pour les auditeurs.

## Обработка инцидентов

- Sélectionnez la commande `docs/source/sorafs/runbooks/` :
  - `sorafs_gateway_operator_playbook.md` pour la passerelle de panne et le jeton de flux.
  - `dispute_revocation_runbook.md`, vous trouverez ici des répliques sportives.
  - `sorafs_node_ops.md` pour la surveillance du nœud domestique.
  - `multi_source_rollout.md` pour remplacer l'opérateur, mettre les pairs sur liste noire et déployer les déploiements.
- Consultez les preuves de sécurité et les anomalies latentes dans GovernanceLog à partir de l'API de suivi PoR, pour que la gouvernance puisse identifier les fournisseurs.

## Следующие шаги- Intégrez l'orchestrateur automatique (`sorafs_car::multi_fetch`), ainsi que l'orchestrateur de récupération multi-source (SF-6b).
- Mettre à jour le PDP/PoTR dans les modules SF-13/SF-14 ; CLI et docs peuvent être résolus pour définir les délais et les niveaux, afin que ces preuves soient stabilisées.

En ce qui concerne ces instructions de démarrage rapide et de réception CI, les commandes concernent les équipements locaux de pipelines SoraFS de qualité production. повторяемым и наблюдаемым процессом.