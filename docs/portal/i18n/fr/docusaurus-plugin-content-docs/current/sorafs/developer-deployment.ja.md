---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/developer-deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 172f4a5b594d5059967e82d42a41847eb6c765b23100cdacea73f0e432aaacba
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
id: developer-deployment
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Source canonique
:::

# Notes de déploiement

Le workflow de packaging SoraFS renforce le déterminisme, donc passer de la CI à la production nécessite surtout des garde-fous opérationnels. Utilisez cette checklist lors du déploiement de l'outillage sur des gateways et fournisseurs de stockage réels.

## Pré-vol

- **Alignement du registre** — confirmez que les profils de chunker et les manifests référencent le même tuple `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Politique d'admission** — revoyez les adverts de fournisseurs signés et les alias proofs nécessaires pour `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook du pin registry** — gardez `docs/source/sorafs/runbooks/pin_registry_ops.md` à portée pour les scénarios de reprise (rotation d'alias, échecs de réplication).

## Configuration de l'environnement

- Les gateways doivent activer l'endpoint de proof streaming (`POST /v1/sorafs/proof/stream`) pour que le CLI puisse émettre des résumés de télémétrie.
- Configurez la policy `sorafs_alias_cache` en utilisant les valeurs par défaut de `iroha_config` ou le helper CLI (`sorafs_cli manifest submit --alias-*`).
- Fournissez les stream tokens (ou identifiants Torii) via un gestionnaire de secrets sécurisé.
- Activez les exporters de télémétrie (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) et envoyez-les vers votre stack Prometheus/OTel.

## Stratégie de rollout

1. **Manifests blue/green**
   - Utilisez `manifest submit --summary-out` pour archiver les réponses de chaque rollout.
   - Surveillez `torii_sorafs_gateway_refusals_total` pour détecter tôt les mismatches de capacité.
2. **Validation des proofs**
   - Traitez les échecs de `sorafs_cli proof stream` comme des bloqueurs de déploiement ; les pics de latence indiquent souvent un throttling fournisseur ou des tiers mal configurés.
   - `proof verify` doit faire partie du smoke test post-pin pour s'assurer que le CAR hébergé par les fournisseurs correspond toujours au digest du manifest.
3. **Dashboards de télémétrie**
   - Importez `docs/examples/sorafs_proof_streaming_dashboard.json` dans Grafana.
   - Ajoutez des panneaux pour la santé du pin registry (`docs/source/sorafs/runbooks/pin_registry_ops.md`) et les stats de chunk range.
4. **Activation multi-source**
   - Suivez les étapes de rollout progressif dans `docs/source/sorafs/runbooks/multi_source_rollout.md` lors de l'activation de l'orchestrateur, et archivez les artefacts scoreboard/télémétrie pour les audits.

## Gestion des incidents

- Suivez les chemins d'escalade dans `docs/source/sorafs/runbooks/` :
  - `sorafs_gateway_operator_playbook.md` pour les pannes de gateway et l'épuisement des stream tokens.
  - `dispute_revocation_runbook.md` lors de litiges de réplication.
  - `sorafs_node_ops.md` pour la maintenance au niveau des nœuds.
  - `multi_source_rollout.md` pour les overrides d'orchestrateur, le blacklisting des peers et les rollouts par étapes.
- Enregistrez les échecs de proofs et les anomalies de latence dans GovernanceLog via les API de PoR tracker existantes afin que la gouvernance puisse évaluer les performances des fournisseurs.

## Prochaines étapes

- Intégrez l'automatisation de l'orchestrateur (`sorafs_car::multi_fetch`) lorsque l'orchestrateur multi-source fetch (SF-6b) sera disponible.
- Suivez les mises à jour PDP/PoTR sous SF-13/SF-14 ; le CLI et les docs évolueront pour exposer les deadlines et la sélection de tiers une fois ces proofs stabilisés.

En combinant ces notes de déploiement avec le quickstart et les recettes CI, les équipes peuvent passer des expérimentations locales à des pipelines SoraFS en production avec un processus répétable et observable.
