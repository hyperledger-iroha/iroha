---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : orchestrator-ops
titre : Runbook pour les exploitants d'entreprises SoraFS
sidebar_label : gestionnaire du Runbook
description: Пошаговое операционное руководство по развёртыванию, мониторингу и откату мульти-источникового оркестратора.
---

:::note Канонический источник
Cette page correspond à `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Vous pouvez obtenir des copies de synchronisation pour pouvoir documenter Sphinx et ne pas pouvoir les transférer.
:::

Ce runbook fournit SRE pour le développement et l'exploitation de plusieurs systèmes de récupération d'infrastructures. En ajoutant des procédures de démarrage de robots, en passant par les programmes de production, y compris la validation et la configuration des pilotes dans чёрный список.

> **См. также:** [Runbook pour le déploiement multi-systèmes] (./multi-source-rollout.md) permet de modifier votre plan d'eau et votre extension. провайдеров. Il s'agit d'un document destiné à la coordination de la gouvernance/mise en scène et à l'organisation de l'exécution commerciale.

## 1. Liste de contrôle préalable

1. **Собрать входные данные от провайдеров**
   - Последние анонсы провайдеров (`ProviderAdvertV1`) et снимок телеметрии для целевого флота.
   - Plan de charge utile (`plan.json`), disponible dans le manifeste de test.
2. **Сформировать детерминированный tableau de bord**

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

   - Assurez-vous que `artifacts/scoreboard.json` correspond à votre fournisseur de produit `eligible`.
   - Архивируйте JSON-сводку вместе со tableau de bord ; Les auditeurs s'occupent des retours d'informations avant la certification en vue de l'enregistrement.
3. **Fonctionnement à sec avec les luminaires** — Consultez votre commande pour les luminaires publics à l'aide de `docs/examples/sorafs_ci_sample/`, afin de déterminer ce que l'opérateur binaire propose. Les versions actuelles, avant de pouvoir ajouter des charges utiles de production.

## 2. Déploiement du processus de livraison

1. **Канарейка (≤2 провайдера)**
   - Sélectionnez le tableau de bord et utilisez le `--max-peers=2` pour que l'opérateur ne soit pas en mesure de le faire.
   - Surveillez :
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - Le produit, alors que le remboursement ne coûte pas 1% pour récupérer le manifeste et que votre fournisseur ne capte pas votre ordinateur.
2. **Этап разгона (50% провайдеров)**
   - Utilisez le `--max-peers` et installez-vous sur votre téléphone.
   - Connectez-vous à `--provider-metrics-out` et `--chunk-receipts-out`. Храните артефакты ≥7 jours.
3. **Déploiement complet**
   - Utilisez `--max-peers` (ou choisissez-le pour être éligible).
   - Cliquez sur le gestionnaire d'entreprise pour les déploiements clients : configurez le tableau de bord social et la configuration JSON pour la configuration du système.
   - Ouvrez les frontières pour rechercher `sorafs_orchestrator_fetch_duration_ms` p95/p99 et les enregistrements d'histogramme selon la région.

## 3. Blocage et utilisation des pirates

Utilisez les remplacements des politiques de notation dans la CLI pour résoudre les problèmes des fournisseurs sans mettre en place une gouvernance approfondie.

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
```- `--deny-provider` inclut l'alias de votre utilisateur lors de la session privée.
- `--boost-provider=<alias>=<weight>` permet à votre fournisseur de planifier. Les informations relatives à la normalisation de votre tableau de bord sont également utilisées au niveau local.
- Sélectionnez les remplacements dans les fichiers d'incident et utilisez les fichiers JSON pour que la commande puisse configurer la solution après l'installation. первопричины.

Pour les travaux postaux, vous devez utiliser un système de télémétrie (vous serez pénalisé) ou vous publierez une publicité sur les futurs étudiants. потоков до очистки CLI-override.

## 4. Rasbor сбоев

Comment récupérer le pad :

1. Avant d'entrer en contact avec les objets d'art suivants :
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. Vérifiez le `session.summary.json` sur le moteur de course :
   - `no providers were supplied` → vérifier le réglage et l'entretien.
   - `retry budget exhausted ...` → sélectionnez `--retry-budget` ou activez les périphériques non installés.
   - `no compatible providers available ...` → vérifier les métadonnées du fournisseur.
3. Demandez à votre fournisseur d'utiliser `sorafs_orchestrator_provider_failures_total` et assurez-vous que le ticket est disponible selon les paramètres métriques.
4. Sélectionnez fetch of flaïn avec `--scoreboard-json` et le téléphone pour déterminer votre connexion.

## 5. Restauration

Que choisir l'opérateur de déploiement :

1. Connectez-vous à la configuration avec `--max-peers=1` (les faits ouvrent la planification multi-établissement) ou contactez les clients de l'entreprise. одноисточниковый fetch-путь.
2. Remplacez l'outil `--boost-provider` par le tableau de bord affiché de manière neutre.
3. Produisez les mesures de l'organisateur de mini-soutiens, afin de répondre à l'opération de récupération en cours.

Les déploiements de distribution d'articles d'art et d'affaires garantissent l'exclusivité d'un entrepreneur multi-industriel pour les entreprises Les flottes fournissent des services de sécurité pour les travaux d'audit et d'audit.