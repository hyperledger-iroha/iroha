---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement multi-source
titre : Runbook de déploiement multi-source et de mise en liste noire
sidebar_label : Runbook déployé multi-source
description : Checklist opérationnelle pour des déploiements multi-sources par étapes et la mise en liste noire d'urgence des fournisseurs.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/runbooks/multi_source_rollout.md`. Conservez les deux copies synchronisées jusqu'à ce que la documentation soit retirée.
:::

## Objectif

Ce runbook guide les SRE et les ingénieurs d'astreinte à travers deux critiques de workflows :

1. Déployer l'orchestrateur multi-source par vagues contrôlées.
2. Mettre en liste noire ou déprioriser les fournisseurs défaillants sans déstabiliser les sessions existantes.

Il suppose que la pile d'orchestration livrée sous SF-6 est déjà déployée (`sorafs_orchestrator`, API de plage de chunks du gateway, exportateurs de télémétrie).

> **Voir aussi :** Le [Runbook d'exploitation de l'orchestrateur](./orchestrator-ops.md) approfondit les procédures par exécution (capture de scoreboard, bascules de déploiement par étapes, rollback). Utilisez les deux références ensemble lors des changements en production.

## 1. Validation pré-déploiement1. **Confirmer les entrées de gouvernance.**
   - Tous les fournisseurs doivent publier des enveloppes `ProviderAdvertV1` avec des charges utiles de capacité de plage et des budgets de flux. Validez via `/v1/sorafs/providers` et comparez aux champs de capacité attendus.
   - Les instantanés de télémétrie fournissant les taux de latence/échec doivent dater de moins de 15 minutes avant chaque exécution canary.
2. **Préparer la configuration.**
   - Persistez la configuration JSON de l'orchestrateur dans l'arborescence `iroha_config` en couches :

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Mettez à jour le JSON avec les limites spécifiques au déploiement (`max_providers`, budgets de nouvelle tentative). Utilisez le même fichier en staging/production pour garder des différences minimales.
3. **Exercer les luminaires canoniques.**
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
     ```Les variables d'environnement doivent contenir le résumé du payload du manifest (hex) et les tokens de flux encodés en base64 pour chaque fournisseur participant au canary.
   - Comparez `artifacts/canary.scoreboard.json` au release précédent. Tout nouveau fournisseur inéligible ou un décalage de poids >10% nécessite une revue.
4. **Vérifier que la télémétrie est câblée.**
   - Ouvrez l'export Grafana dans `docs/examples/sorafs_fetch_dashboard.json`. Assurez-vous que les mesures `sorafs_orchestrator_*` apparaissent en scène avant de continuer.

## 2. Mise en liste noire d'urgence des fournisseurs

Suivez cette procédure lorsqu'un fournisseur sert des morceaux corrompus, subit des délais d'attente persistants ou échoue aux contrôles de conformité.1. **Capturer les preuves.**
   - Exportez le dernier résumé de fetch (sortie de `--json-out`). Enregistrez les index de chunks en échec, les alias de fournisseurs et les discordances de digest.
   - Sauvegardez les extraits de logs pertinents des cibles `telemetry::sorafs.fetch.*`.
2. **Appliquer un remplacement immédiat.**
   - Marquez le fournisseur comme pénalisé dans l'instantané de télémétrie distribué à l'orchestrateur (définissez `penalty=true` ou forcez `token_health` à `0`). Le prochain tableau de bord exclura automatiquement le fournisseur.
   - Pour des tests de fumée ad hoc, passer `--deny-provider gw-alpha` à `sorafs_cli fetch` afin d'exercer le chemin de défaillance sans attendre la propagation de la télémétrie.
   - Redéployez le bundle télémétrie/configuration mis à jour dans l'environnement touché (stage → canary → production). Documentez le changement dans le journal d'incident.
3. **Valider l'override.**
   - Relancez le fetch de la luminaire canonique. Confirmez que le scoreboard marque le fournisseur comme inéligible avec la raison `policy_denied`.
   - Inspectez `sorafs_orchestrator_provider_failures_total` pour vérifier que le compteur cesse d'augmenter pour le fournisseur refusé.
4. **Escalader les interdictions longue durée.**- Si le fournisseur reste bloqué >24 h, ouvrez un ticket de gouvernance pour faire pivoter ou conserver son annonce. Jusqu'au vote, conservez la liste de refus et rafraîchissez les instantanés de télémétrie pour éviter qu'il ne réintègre le tableau de bord.
5. **Protocole de restauration.**
   - Pour rétablir le fournisseur, retirer-le de la liste de refus, redéployez et capturez un nouveau snapshot de scoreboard. Attachez le changement au post-mortem d'incident.

## 3. Plan de déploiement par étapes| Phases | Portée | Signaux requis | Critères Go/No-Go |
|-------|--------|----------------|---------|
| **Labo** | Cluster d'intégration dédié | Fetch CLI manuel contre les charges utiles de luminaires | Tous les morceaux réussissent, les compteurs d'échec fournis restent à 0, taux de retry < 5%. |
| **Mise en scène** | Staging du plan de contrôle complet | Tableau de bord Grafana connecté ; règles d'alerte en mode warn-only | `sorafs_orchestrator_active_fetches` revient à zéro après chaque exécution de test ; aucune alerte `warn/critical`. |
| **Canari** | ≤10% du trafic de production | Pager muet mais télémétrie surveillée en temps réel | Ratio de retry < 10%, échecs fournisseurs limités aux paires bruyantes connues, histogramme de latence conforme à la baseline staging ±20%. |
| **Disponibilité générale** | 100% du déploiement | Règles du pager actif | Zéro erreur `NoHealthyProviders` pendant 24 h, ratio de réessai stable, panneaux SLA du tableau de bord au vert. |

Versez chaque phase :

1. Mettez à jour le JSON de l'orchestrateur avec les `max_providers` et les budgets de nouvelle tentative prévus.
2. Lancez `sorafs_cli fetch` ou la suite de tests d'intégration SDK contre la luminaire canonique et un manifeste représentatif de l'environnement.
3. Capturez les artefacts de scoreboard + summary et attachez-les au registre de release.
4. Revoyez les tableaux de bord de télémétrie avec l'ingénieur d'astreinte avant de promouvoir à la phase suivante.## 4. Observabilité et crochets d'incident

- **Métriques :** Assurez-vous qu'Alertmanager surveille `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` et `sorafs_orchestrator_retries_total`. Un pic soudain signifie généralement qu'un fournisseur se dégrade sous charge.
- **Logs :** Acheminez les cibles `telemetry::sorafs.fetch.*` vers l'agrégateur de logs partagés. Créez des recherches enregistrées pour `event=complete status=failed` afin d'accélérer le triage.
- **Scoreboards :** Persistez chaque artefact de scoreboard en stockage à long terme. Le JSON sert aussi de piste d'audit pour les revues de conformité et les rollbacks par étapes.
- **Dashboards :** Clonez le tableau Grafana canonique (`docs/examples/sorafs_fetch_dashboard.json`) dans le dossier production avec les règles d'alerte de `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Communication et documentation

- Journalisez chaque changement de deny/boost dans le changelog d'exploitation avec horodatage, opérateur, raison et incident associé.
- Informez les équipes SDK lorsque les poids de fournisseurs ou les budgets de réessayer changent pour aligner les attentes côté client.
- Une fois la GA terminée, mettez à jour `status.md` avec le résumé du déploiement et archivez cette référence de runbook dans les notes de version.