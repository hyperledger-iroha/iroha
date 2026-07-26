---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement par le développeur
titre : Notes de déploiement par SoraFS
sidebar_label : Notes de déploiement
description : Liste de contrôle pour promouvoir le pipeline de SoraFS de CI pour la production.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/developer/deployment.md`. Mantenha ambas comme copies synchronisées.
:::

# Notes de déploiement

Le flux de travail de mise en œuvre du SoraFS renforce le déterminisme, entre autres pour le passage de CI pour la production, qui nécessite principalement des garde-corps opérationnels. Utilisez cette liste de contrôle pour trouver les ferraments pour les passerelles et les fournisseurs d'armement réel.

## Pré-vol

- **Alinhamento do registro** - confirmez que les performances du chunker et se manifestent en référence à mon tupla `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Politica d'admission** - réviser les annonces des fournisseurs d'os assassinés et les preuves d'alias nécessaires pour `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook do pin registre** - gère `docs/source/sorafs/runbooks/pin_registry_ops.md` pour les scénarios de récupération (rotation d'alias, fausses répliques).

## Configuration de l'ambiance- Les passerelles développent le streaming de preuve de point final (`POST /v1/sorafs/proof/stream`) pour que la CLI émette des résumés de télémétrie.
- Configurez une politique `sorafs_alias_cache` en utilisant vos pilotes dans `iroha_config` ou l'assistant de la CLI (`sorafs_cli manifest submit --alias-*`).
- Jetons de flux Forneca (ou identifiants Torii) via un gestionnaire de secrets sécurisé.
- Habilite exportateurs de télémétrie (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) et envie de votre pile Prometheus/OTel.

## Stratégie de déploiement

1. **Manifeste bleu/vert**
   - Utilisez `manifest submit --summary-out` pour le déploiement des réponses à chaque fois.
   - Observer `torii_sorafs_gateway_refusals_total` para captar mismatches de capacidade cedo.
2. **Validacao de proofs**
   - Traiter les erreurs dans `sorafs_cli proof stream` comme bloqueurs de déploiement ; des pics de latence personnalisés indiquent la limitation du fournisseur ou des niveaux mal configurés.
   - `proof verify` deve fazer parte do smoke test pos-pin para garantir que o CAR hospedado pelos provenores ainda corresponde ao digest do manifest.
3. **Tableaux de bord de télémétrie**
   - Importé `docs/examples/sorafs_proof_streaming_dashboard.json` n° Grafana.
   - Ajout de paineis pour la sauvegarde du registre des broches (`docs/source/sorafs/runbooks/pin_registry_ops.md`) et des statistiques de plage de morceaux.
4. **Habilitacao multi-source**
   - Suivez les étapes de déploiement dans les étapes `docs/source/sorafs/runbooks/multi_source_rollout.md` pour activer l'orquestrador et archiver les artéfacts de tableau de bord/télémétrie pour les auditoires.

## Traitement des incidents- Vérifiez les chemins d'escalade dans `docs/source/sorafs/runbooks/` :
  - `sorafs_gateway_operator_playbook.md` pour les passerelles et l'échange de jetons de flux.
  - `dispute_revocation_runbook.md` lorsque des litiges de réplication surviennent.
  - `sorafs_node_ops.md` pour la maintenance au niveau du nœud.
  - `multi_source_rollout.md` pour les remplacements de l'explorateur, la liste noire des pairs et les déploiements par étapes.
- Enregistrez les erreurs de preuves et les anomalies de latence dans GovernanceLog via les API de PoR tracker existantes pour que la gouvernance avalie ou emploie les fournisseurs.

## Passons à proximité

- Intégrer un explorateur automatique (`sorafs_car::multi_fetch`) lorsque l'explorateur de récupération multi-source (SF-6b) est sélectionné.
- Accompagne les mises à niveau du PDP/PoTR sur SF-13/SF-14 ; o CLI et une documentation qui va évoluer pour exporter les prazos et sélectionner les niveaux lorsque ces preuves sont stabilisées.

En combinant ces notes de déploiement avec le démarrage rapide et les recettes de CI, les équipes peuvent passer des expériences locales pour les pipelines SoraFS dans la production d'un processus répétitif et d'observation.