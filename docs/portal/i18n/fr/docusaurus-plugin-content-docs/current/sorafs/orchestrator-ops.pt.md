---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : orchestrator-ops
titre : Runbook des opérations de l'orchestre SoraFS
sidebar_label : Runbook de l'orchestre
la description : Guide opérationnel passo a passo para implantar, monitorar e reverter o orquestrador multi-origem.
---

:::note Fonte canônica
Cette page s'affiche `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Mantenha ambas comme copies synchronisées.
:::

Ce runbook oriente les SRE vers la préparation, le déploiement et l'exploitation de l'explorateur de récupération multi-origes. Il complète le guide de développement avec les procédures ajustées pour les déploiements en production, y compris l'habilitation aux phases et le blocage des pairs.

> **Voir aussi :** Le [Runbook de déploiement multi-origem](./multi-source-rollout.md) se concentre sur les vagues de déploiement aujourd'hui à l'avant et sur la négative émergente des fournisseurs. Consultez-le pour la coordination de la gouvernance/mise en scène lorsque vous utilisez ce document pour les opérations quotidiennes de l'orchestre.

## 1. Checklist pré-voo

1. **Coletar insumos de provenores**
   - Dernières annonces de fournisseurs (`ProviderAdvertV1`) et un instantané de télémétrie à partir d'avant.
   - Plan de charge utile (`plan.json`) dérivé du manifeste sur le test.
2. **Gérer un tableau de bord déterministe**

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

   - Valide selon la liste `artifacts/scoreboard.json` de chaque fournisseur de production comme `eligible`.
   - Archiver le CV JSON avec le tableau de bord ; Les auditeurs dépendent des contadores pour réessayer les morceaux pour certifier la sollicitation de modification.
3. **Dry-run avec les appareils** — Exécutez cette commande contre les appareils publics dans `docs/examples/sorafs_ci_sample/` pour garantir que le binaire de l'explorateur correspond à la version attendue avant de charger les charges utiles de production.

## 2. Procédure de déploiement par phases

1. **Fase canário (≤2 fournisseurs)**
   - Enregistrez le tableau de bord et exécutez `--max-peers=2` pour restreindre l'orchestre à un petit sous-ensemble.
   - Surveillance :
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - Prossiga quando as taxas de retry permanecerem abaixo de 1% para um fetch complete do manifesto e nenhum provideor acumular falhas.
2. **Fase de rampa (50% dos proveneurs)**
   - Augmentez `--max-peers` et exécutez récemment un instantané de télémétrie récent.
   - Continuer chaque exécution avec `--provider-metrics-out` et `--chunk-receipts-out`. Conserver les artéfacts pendant ≥7 jours.
3. **Déploiement terminé**
   - Supprimer `--max-peers` (ou défini pour un contagem total de elegíveis).
   - Active le mode d'organisation de nos déploiements de clients : distribuez le tableau de bord persistant et le JSON de configuration via votre système de gestion de configuration.
   - Actualiser les tableaux de bord pour afficher `sorafs_orchestrator_fetch_duration_ms` p95/p99 et les histogrammes de nouvelle tentative par région.

## 3. Blocage et renforcement des pairs

Utilisez les dérogations politiques de pont de la CLI pour trier les fournisseurs qui ne sont pas sûrs d'espérer les mises à jour de la gouvernance.

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
```- `--deny-provider` supprimer l'alias indiqué pour la session actuelle.
- `--boost-provider=<alias>=<weight>` augmente le peso doprovedor no agendador. Les valeurs sont somados au peso normalisé du tableau de bord et sont appliquées uniquement à l'exécution locale.
- Enregistrez les remplacements sans ticket d'incident et annexe comme indiqué JSON pour que l'équipe responsable puisse concilier l'état quand le problème sous-jacent est résolu.

Pour les changements permanents, ajustez la télémétrie d'origine (marque ou infracteur comme pénalisé) ou actualisez l'annonce avec les réglages de flux actualisés avant de nettoyer les remplacements de CLI.

## 4. Triage des erreurs

Quand aller chercher la Falha :

1. Capturez les étapes suivantes avant d'exécuter nouvellement :
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. Inspectez `session.summary.json` pour une chaîne d'erreurs légales :
   - `no providers were supplied` → vérifiez les chemins des fournisseurs et les annonces.
   - `retry budget exhausted ...` → augmenter `--retry-budget` ou supprimer les pairs instáveis.
   - `no compatible providers available ...` → auditer les métadonnées de capacité de la façade du fournisseur d'infracteur.
3. Corrélation du nom du fournisseur avec `sorafs_orchestrator_provider_failures_total` et ouverture d'un ticket d'accompagnement à une métrique différente.
4. Reproduisez ou récupérez hors ligne avec `--scoreboard-json` et une télémétrie capturée pour reproduire sous une forme déterminée.

## 5. Restauration

Pour inverser le déploiement de l'orchestre :

1. Distribuez une configuration qui définit `--max-peers=1` (désactivée efficacement ou agenda multi-origem) ou retournez les clients vers le chemin de récupération de la police unique.
2. Supprimez dès que possible les remplacements `--boost-provider` pour que le tableau de bord retourne à une réflexion neutre.
3. Continuez à collecter les paramètres de l'explorateur jusqu'à un minimum d'un jour pour confirmer que vous n'avez pas récupéré les résidus dans l'opération.

Maintenir la capture disciplinée des objets et les déploiements en phases garantissent que l'explorateur multi-origes pourra être utilisé avec sécurité sur les surfaces hétérogènes des fournisseurs, en respectant les conditions d'observation et les salles intactes.