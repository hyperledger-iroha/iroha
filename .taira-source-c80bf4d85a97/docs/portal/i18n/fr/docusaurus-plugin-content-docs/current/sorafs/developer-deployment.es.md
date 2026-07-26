---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement par le développeur
titre : Notes de despliegue de SoraFS
sidebar_label : Notes de despliegue
description : Liste de vérification pour promouvoir le pipeline de SoraFS de CI a production.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/developer/deployment.md`. Gardez les versions synchronisées jusqu'à ce que les documents hérités soient retirés.
:::

# Notes de despligue

Le flux d'emballage de SoraFS répond à la détermination, car le passage de CI à la production nécessite principalement des agents de sécurité. Utilisez cette liste lorsque vous déployez l'outil sur les passerelles et les fournisseurs de stockage réel.

## Préparation préalable

- **Alineación del registro** — confirme que les profils de chunker et les manifestes font référence à la même tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Política de admissionión** — réviser les annonces du fournisseur firmados et les preuves d'alias nécessaires pour `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook de registre PIN** — mantén `docs/source/sorafs/runbooks/pin_registry_ops.md` à la main pour les scénarios de récupération (rotation d'alias, erreurs de réplication).

## Configuration de l'arrivée- Les passerelles doivent permettre au point final de streaming des preuves (`POST /v1/sorafs/proof/stream`) pour que la CLI émette des résultats de télémétrie.
- Configurez la politique `sorafs_alias_cache` en utilisant les valeurs prédéfinies de `iroha_config` ou l'assistant de CLI (`sorafs_cli manifest submit --alias-*`).
- Proposer des jetons de flux (ou identifiants Torii) à l'aide d'un gestionnaire de secrets sécurisés.
- Habilita les exportateurs de télémétrie (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) et les envoie à votre pile Prometheus/OTel.

## Stratégie de despligue1. **Manifeste bleu/vert**
   - Utilisez `manifest submit --summary-out` pour archiver les réponses de chaque envoi.
   - Vigila `torii_sorafs_gateway_refusals_total` pour détecter le désajustement des capacités temprano.
2. **Validation des preuves**
   - Trata los fallos en `sorafs_cli proof stream` comme bloqueadores del despliegue; Les pics de latence suelen indiquent la limitation du fournisseur ou des niveaux mal configurés.
   - `proof verify` doit former une partie du test de fumée postérieur à la broche pour garantir que le CAR alojado por los fournisseurs sigue coïncident avec le résumé du manifeste.
3. **Tableaux de bord de télémétrie**
   - Importé `docs/examples/sorafs_proof_streaming_dashboard.json` et Grafana.
   - Ajoutez des panneaux supplémentaires pour la santé du registre des broches (`docs/source/sorafs/runbooks/pin_registry_ops.md`) et des statistiques de plage de morceaux.
4. **Habilité multi-source**
   - Suivez les étapes de déroulement des étapes en `docs/source/sorafs/runbooks/multi_source_rollout.md` pour activer l'orchestre et archiver les artefacts de tableau de bord/télémétrie pour les auditeurs.

## Gestion des incidents- Suivez les itinéraires d'escalade en `docs/source/sorafs/runbooks/` :
  - `sorafs_gateway_operator_playbook.md` pour les caídas de gateway et l'agotamiento de stream-token.
  - `dispute_revocation_runbook.md` lorsqu'il y a des litiges de réplication.
  - `sorafs_node_ops.md` pour maintenir un niveau de noeud.
  - `multi_source_rollout.md` pour remplacer l'orquestador, listas negras de peers y despliegues por etapas.
- Enregistrer les erreurs de preuves et les anomalies de latence dans GovernanceLog à l'aide des API de PoR tracker existantes pour que l'administration puisse évaluer le rendu du fournisseur.

## Nous passons bientôt

- Intégrer l'automatisation de l'explorateur (`sorafs_car::multi_fetch`) lorsque l'explorateur effectue une récupération multi-source (SF-6b).
- Suivez les mises à jour de PDP/PoTR sous SF-13/SF-14 ; la CLI et les documents évoluent pour les zones d'exposition et la sélection des niveaux lorsque ces preuves sont établies.

En combinant ces notes de déploiement avec le démarrage rapide et les recettes de CI, les équipes peuvent effectuer des expériences locales sur les pipelines SoraFS en production avec un processus répétitif et observable.