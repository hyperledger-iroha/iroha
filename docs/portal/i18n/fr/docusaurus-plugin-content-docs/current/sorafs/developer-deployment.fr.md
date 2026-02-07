---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement par le développeur
titre : Notes de déploiement SoraFS
sidebar_label : Notes de déploiement
description : Checklist pour promouvoir le pipeline SoraFS de la CI vers la production.
---

:::note Source canonique
:::

# Notes de déploiement

Le workflow de packaging SoraFS renforce le déterminisme, donc passer de la CI à la production nécessite surtout des garde-fous opérationnels. Utilisez cette liste de contrôle lors du déploiement de l'outillage sur des passerelles et fournisseurs de stockage réels.

## Pré-vol

- **Alignement du registre** — confirmez que les profils de chunker et les manifestes référencent le même tuple `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Politique d'admission** — revoyez les publicités de fournisseurs signés et les alias proofs nécessaires pour `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook du pin Registry** — gardez `docs/source/sorafs/runbooks/pin_registry_ops.md` à portée pour les scénarios de reprise (rotation d'alias, échecs de réplication).

## Configuration de l'environnement- Les passerelles doivent activer l'endpoint de proof streaming (`POST /v1/sorafs/proof/stream`) pour que la CLI puisse émettre des résumés de télémétrie.
- Configurez la stratégie `sorafs_alias_cache` en utilisant les valeurs par défaut de `iroha_config` ou le helper CLI (`sorafs_cli manifest submit --alias-*`).
- Fournissez les stream tokens (ou identifiants Torii) via un gestionnaire de secrets sécurisé.
- Activez les exportateurs de télémétrie (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) et envoyez-les vers votre stack Prometheus/OTel.

## Stratégie de déploiement1. **Manifeste bleu/vert**
   - Utilisez `manifest submit --summary-out` pour archiver les réponses de chaque déploiement.
   - Surveillez `torii_sorafs_gateway_refusals_total` pour détecter tôt les écarts de capacité.
2. **Validation des preuves**
   - Traitez les échecs de `sorafs_cli proof stream` comme des bloqueurs de déploiement ; les images de latence indiquent souvent un fournisseur de limitation ou des niveaux mal configurés.
   - `proof verify` doit faire partie du smoke test post-pin pour s'assurer que le CAR hébergé par les fournisseurs correspond toujours au digest du manifeste.
3. **Tableaux de bord de télémétrie**
   - Importez `docs/examples/sorafs_proof_streaming_dashboard.json` dans Grafana.
   - Ajoutez des panneaux pour la santé du pin Registry (`docs/source/sorafs/runbooks/pin_registry_ops.md`) et les stats de chunk range.
4. **Activation multi-source**
   - Suivez les étapes de déploiement progressif dans `docs/source/sorafs/runbooks/multi_source_rollout.md` lors de l'activation de l'orchestrateur, et archivez les artefacts scoreboard/télémétrie pour les audits.

##Gestion des incidents- Suivez les chemins d'escalade dans `docs/source/sorafs/runbooks/` :
  - `sorafs_gateway_operator_playbook.md` pour les pannes de gateway et l'épuisement des stream tokens.
  - `dispute_revocation_runbook.md` lors de litiges de réplication.
  - `sorafs_node_ops.md` pour la maintenance au niveau des nœuds.
  - `multi_source_rollout.md` pour les overrides d'orchestrateur, le blacklisting des pairs et les rollouts par étapes.
- enregistrer les échecs de preuves et les anomalies de latence dans GovernanceLog via les API de PoR tracker existantes afin que la gouvernance puisse évaluer les performances des fournisseurs.

## Prochaines étapes

- Intégrez l'automatisation de l'orchestrateur (`sorafs_car::multi_fetch`) lorsque l'orchestrateur multi-source fetch (SF-6b) sera disponible.
- Suivez les mises à jour PDP/PoTR sous SF-13/SF-14 ; le CLI et les docs évolueront pour exposer les délais et la sélection de niveaux une fois ces épreuves stabilisées.

En combinant ces notes de déploiement avec le quickstart et les recettes CI, les équipes peuvent passer des expérimentations locales à des pipelines SoraFS en production avec un processus répétable et observable.