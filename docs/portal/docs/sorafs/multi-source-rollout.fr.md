---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6d90c0796a29e2ae49c0019de7df2370c822b985e8247675bc6b8de93111f928
source_last_modified: "2025-11-10T18:39:01.034083+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: multi-source-rollout
title: Runbook de déploiement multi-source et de mise en liste noire
sidebar_label: Runbook déploiement multi-source
description: Checklist opérationnelle pour des déploiements multi-source par étapes et la mise en liste noire d'urgence des fournisseurs.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/multi_source_rollout.md`. Gardez les deux copies synchronisées jusqu'à ce que la documentation héritée soit retirée.
:::

## Objectif

Ce runbook guide les SRE et les ingénieurs d'astreinte à travers deux workflows critiques :

1. Déployer l'orchestrateur multi-source par vagues contrôlées.
2. Mettre en liste noire ou déprioriser les fournisseurs défaillants sans déstabiliser les sessions existantes.

Il suppose que la pile d'orchestration livrée sous SF-6 est déjà déployée (`sorafs_orchestrator`, API de plage de chunks du gateway, exporteurs de télémétrie).

> **Voir aussi :** Le [Runbook d'exploitation de l'orchestrateur](./orchestrator-ops.md) approfondit les procédures par exécution (capture de scoreboard, bascules de déploiement par étapes, rollback). Utilisez les deux références ensemble lors des changements en production.

## 1. Validation pré-déploiement

1. **Confirmer les entrées de gouvernance.**
   - Tous les fournisseurs candidats doivent publier des enveloppes `ProviderAdvertV1` avec des payloads de capacité de plage et des budgets de flux. Validez via `/v1/sorafs/providers` et comparez aux champs de capacité attendus.
   - Les snapshots de télémétrie fournissant les taux de latence/échec doivent dater de moins de 15 minutes avant chaque exécution canary.
2. **Préparer la configuration.**
   - Persistez la configuration JSON de l'orchestrateur dans l'arborescence `iroha_config` en couches :

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Mettez à jour le JSON avec les limites spécifiques au rollout (`max_providers`, budgets de retry). Utilisez le même fichier en staging/production pour garder des diffs minimales.
3. **Exercer les fixtures canoniques.**
   - Renseignez les variables d'environnement manifest/token et lancez le fetch déterministe :

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     Les variables d'environnement doivent contenir le digest du payload du manifest (hex) et les tokens de flux encodés en base64 pour chaque fournisseur participant au canary.
   - Comparez `artifacts/canary.scoreboard.json` au release précédent. Tout nouveau fournisseur inéligible ou un décalage de poids >10% nécessite une revue.
4. **Vérifier que la télémétrie est câblée.**
   - Ouvrez l'export Grafana dans `docs/examples/sorafs_fetch_dashboard.json`. Assurez-vous que les métriques `sorafs_orchestrator_*` apparaissent en staging avant de poursuivre.

## 2. Mise en liste noire d'urgence des fournisseurs

Suivez cette procédure lorsqu'un fournisseur sert des chunks corrompus, subit des délais d'attente persistants ou échoue aux contrôles de conformité.

1. **Capturer les preuves.**
   - Exportez le dernier résumé de fetch (sortie de `--json-out`). Enregistrez les index de chunks en échec, les alias de fournisseurs et les discordances de digest.
   - Sauvegardez les extraits de logs pertinents des cibles `telemetry::sorafs.fetch.*`.
2. **Appliquer un override immédiat.**
   - Marquez le fournisseur comme pénalisé dans le snapshot de télémétrie distribué à l'orchestrateur (définissez `penalty=true` ou forcez `token_health` à `0`). Le prochain scoreboard exclura automatiquement le fournisseur.
   - Pour des smoke tests ad hoc, passez `--deny-provider gw-alpha` à `sorafs_cli fetch` afin d'exercer le chemin de défaillance sans attendre la propagation de la télémétrie.
   - Redéployez le bundle télémétrie/configuration mis à jour dans l'environnement touché (staging → canary → production). Documentez le changement dans le journal d'incident.
3. **Valider l'override.**
   - Relancez le fetch de la fixture canonique. Confirmez que le scoreboard marque le fournisseur comme inéligible avec la raison `policy_denied`.
   - Inspectez `sorafs_orchestrator_provider_failures_total` pour vérifier que le compteur cesse d'augmenter pour le fournisseur refusé.
4. **Escalader les interdictions longue durée.**
   - Si le fournisseur reste bloqué >24 h, ouvrez un ticket de gouvernance pour faire pivoter ou suspendre son advert. Jusqu'au vote, conservez la liste de refus et rafraîchissez les snapshots de télémétrie pour éviter qu'il ne réintègre le scoreboard.
5. **Protocole de rollback.**
   - Pour rétablir le fournisseur, retirez-le de la liste de refus, redéployez et capturez un nouveau snapshot de scoreboard. Attachez le changement au postmortem d'incident.

## 3. Plan de rollout par étapes

| Phase | Portée | Signaux requis | Critères Go/No-Go |
|-------|--------|----------------|-------------------|
| **Lab** | Cluster d'intégration dédié | Fetch CLI manuel contre les payloads de fixtures | Tous les chunks réussissent, les compteurs d'échec fournisseur restent à 0, taux de retry < 5%. |
| **Staging** | Staging du control-plane complet | Dashboard Grafana connecté; règles d'alerte en mode warning-only | `sorafs_orchestrator_active_fetches` revient à zéro après chaque exécution de test; aucune alerte `warn/critical`. |
| **Canary** | ≤10% du trafic de production | Pager muet mais télémétrie surveillée en temps réel | Ratio de retry < 10%, échecs fournisseurs limités aux pairs bruyants connus, histogramme de latence conforme à la baseline staging ±20%. |
| **Disponibilité générale** | 100% du rollout | Règles du pager actives | Zéro erreur `NoHealthyProviders` pendant 24 h, ratio de retry stable, panneaux SLA du dashboard au vert. |

Pour chaque phase :

1. Mettez à jour le JSON de l'orchestrateur avec les `max_providers` et budgets de retry prévus.
2. Lancez `sorafs_cli fetch` ou la suite de tests d'intégration SDK contre la fixture canonique et un manifest représentatif de l'environnement.
3. Capturez les artefacts de scoreboard + summary et attachez-les au registre de release.
4. Revoyez les dashboards de télémétrie avec l'ingénieur d'astreinte avant de promouvoir à la phase suivante.

## 4. Observabilité et hooks d'incident

- **Métriques :** Assurez-vous qu'Alertmanager surveille `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` et `sorafs_orchestrator_retries_total`. Un pic soudain signifie généralement qu'un fournisseur se dégrade sous charge.
- **Logs :** Acheminez les cibles `telemetry::sorafs.fetch.*` vers l'agrégateur de logs partagé. Créez des recherches enregistrées pour `event=complete status=failed` afin d'accélérer le triage.
- **Scoreboards :** Persistez chaque artefact de scoreboard en stockage long terme. Le JSON sert aussi de piste d'audit pour les revues de conformité et les rollbacks par étapes.
- **Dashboards :** Clonez le tableau Grafana canonique (`docs/examples/sorafs_fetch_dashboard.json`) dans le dossier production avec les règles d'alerte de `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Communication et documentation

- Journalisez chaque changement de deny/boost dans le changelog d'exploitation avec horodatage, opérateur, raison et incident associé.
- Informez les équipes SDK lorsque les poids de fournisseurs ou les budgets de retry changent pour aligner les attentes côté client.
- Une fois la GA terminée, mettez à jour `status.md` avec le résumé du rollout et archivez cette référence de runbook dans les notes de version.
