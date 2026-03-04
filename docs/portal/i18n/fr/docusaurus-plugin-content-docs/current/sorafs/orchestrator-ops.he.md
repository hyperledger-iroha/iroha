---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/orchestrator-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a36d9867ab298262223e3441a0a2f5c83867420c8703b0ac59a71bb4372b725e
source_last_modified: "2026-01-03T18:08:01+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: orchestrator-ops
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Runbook d’exploitation de l’orchestrateur SoraFS
sidebar_label: Runbook orchestrateur
description: Guide opérationnel pas à pas pour déployer, surveiller et revenir en arrière sur l’orchestrateur multi-source.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Gardez les deux copies synchronisées jusqu’à ce que l’ensemble de documentation Sphinx hérité soit entièrement migré.
:::

Ce runbook guide les SRE dans la préparation, le déploiement et l’exploitation de l’orchestrateur de fetch multi-source. Il complète le guide développeur avec des procédures adaptées aux déploiements en production, y compris l’activation par étapes et la mise en liste noire des pairs.

> **Voir aussi :** Le [Runbook de déploiement multi-source](./multi-source-rollout.md) se concentre sur les vagues de déploiement à l’échelle du parc et le refus d’urgence des fournisseurs. Référez-vous-y pour la coordination gouvernance / staging tout en utilisant ce document pour les opérations quotidiennes de l’orchestrateur.

## 1. Checklist pré-déploiement

1. **Collecter les entrées fournisseurs**
   - Dernières annonces fournisseurs (`ProviderAdvertV1`) et instantané de télémétrie pour la flotte cible.
   - Plan de payload (`plan.json`) dérivé du manifeste testé.
2. **Générer un scoreboard déterministe**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - Vérifiez que `artifacts/scoreboard.json` répertorie chaque fournisseur de production comme `eligible`.
   - Archivez le JSON de synthèse avec le scoreboard ; les auditeurs s’appuient sur les compteurs de retry de chunks lors de la certification de la demande de changement.
3. **Dry-run avec les fixtures** — Exécutez la même commande sur les fixtures publiques de `docs/examples/sorafs_ci_sample/` pour vous assurer que le binaire de l’orchestrateur correspond à la version attendue avant de toucher aux payloads de production.

## 2. Procédure de déploiement par étapes

1. **Étape canari (≤2 fournisseurs)**
   - Reconstruisez le scoreboard et exécutez avec `--max-peers=2` pour limiter l’orchestrateur à un petit sous-ensemble.
   - Surveillez:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - Poursuivez une fois que les taux de retry restent sous 1 % pour un fetch complet du manifeste et qu’aucun fournisseur n’accumule d’échecs.
2. **Étape de montée en charge (50 % des fournisseurs)**
   - Augmentez `--max-peers` et relancez avec un instantané de télémétrie récent.
   - Persistez chaque exécution avec `--provider-metrics-out` et `--chunk-receipts-out`. Conservez les artefacts pendant ≥7 jours.
3. **Déploiement complet**
   - Supprimez `--max-peers` (ou fixez-le au nombre total de fournisseurs éligibles).
   - Activez le mode orchestrateur dans les déploiements clients : distribuez le scoreboard persisté et le JSON de configuration via votre système de gestion de configuration.
   - Mettez à jour les tableaux de bord pour afficher `sorafs_orchestrator_fetch_duration_ms` p95/p99 et les histogrammes de retry par région.

## 3. Mise en liste noire et boosting des pairs

Utilisez les overrides de politique de scoring du CLI pour trier les fournisseurs défaillants sans attendre les mises à jour de gouvernance.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` retire l’alias indiqué de la session en cours.
- `--boost-provider=<alias>=<weight>` augmente le poids du fournisseur dans le planificateur. Les valeurs s’ajoutent au poids normalisé du scoreboard et ne s’appliquent qu’à l’exécution locale.
- Enregistrez les overrides dans le ticket d’incident et joignez les sorties JSON afin que l’équipe responsable puisse réconcilier l’état une fois le problème sous-jacent corrigé.

Pour des changements permanents, modifiez la télémétrie source (marquez le fautif comme pénalisé) ou mettez à jour l’annonce avec des budgets de flux révisés avant de supprimer les overrides du CLI.

## 4. Diagnostic des pannes

Lorsqu’un fetch échoue:

1. Capturez les artefacts suivants avant de relancer:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. Inspectez `session.summary.json` pour la chaîne d’erreur lisible:
   - `no providers were supplied` → vérifiez les chemins des fournisseurs et les annonces.
   - `retry budget exhausted ...` → augmentez `--retry-budget` ou supprimez des pairs instables.
   - `no compatible providers available ...` → auditez les métadonnées de capacité de plage du fournisseur fautif.
3. Corrélez le nom du fournisseur avec `sorafs_orchestrator_provider_failures_total` et créez un ticket de suivi si la métrique monte en flèche.
4. Rejouez le fetch hors ligne avec `--scoreboard-json` et la télémétrie capturée pour reproduire l’échec de manière déterministe.

## 5. Rollback

Pour revenir sur un déploiement de l’orchestrateur:

1. Distribuez une configuration qui définit `--max-peers=1` (désactive effectivement l’ordonnancement multi-source) ou revenez au chemin de fetch mono-source historique côté clients.
2. Supprimez toute override `--boost-provider` afin que le scoreboard revienne à un poids neutre.
3. Continuez à scruter les métriques de l’orchestrateur pendant au moins une journée pour confirmer qu’aucun fetch résiduel n’est en vol.

Maintenir une capture disciplinée des artefacts et des déploiements par étapes garantit que l’orchestrateur multi-source peut être opéré en toute sécurité sur des flottes hétérogènes de fournisseurs tout en respectant les exigences d’observabilité et d’audit.
