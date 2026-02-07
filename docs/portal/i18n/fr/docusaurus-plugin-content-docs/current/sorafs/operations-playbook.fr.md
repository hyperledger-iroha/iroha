---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : manuel d'opérations
titre : Playbook d’exploitation SoraFS
sidebar_label : Playbook d'exploitation
description : Guides de réponse aux incidents et procédures de drills de chaos pour les opérateurs SoraFS.
---

:::note Source canonique
Cette page reflète le runbook maintenu dans `docs/source/sorafs_ops_playbook.md`. Gardez les deux copies synchronisées jusqu’à ce que la documentation Sphinx soit totalement migrée.
:::

## Références clés

- Actifs d’observabilité : consultez les tableaux de bord Grafana sous `dashboards/grafana/` et les règles d’alerte Prometheus dans `dashboards/alerts/`.
- Catalogue de métriques : `docs/source/sorafs_observability_plan.md`.
- Surfaces de télémétrie de l'orchestrateur : `docs/source/sorafs_orchestrator_plan.md`.

## Matrice d'escalade| Priorité | Exemples de déclenchement | Directeur de garde | Sauvegarde | Remarques |
|----------|---------------------------|------------------|--------|-------|
| P1 | Panne globale gateway, taux d’échec PoR > 5% (15 min), backlog de réplication doublant toutes les 10 min | Stockage SRE | Observabilité TL | Engager le conseil de gouvernance si l’impact dépasse 30 min. |
| P2 | Violation du SLO de latence gateway régionale, pic de tentatives orchestrateur sans impact SLA | Observabilité TL | Stockage SRE | Continuer le déploiement mais bloquer les nouveaux manifestes. |
| P3 | Alertes non critiques (obsolescence des manifestes, capacité 80–90%) | Triage d'admission | Guilde des opérations | À traiter dans le prochain jour ouvré. |

## Passerelle Panne / disponibilité dégradée

**Détection**

- Alertes : `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Tableau de bord : `dashboards/grafana/sorafs_gateway_overview.json`.

**Actions immédiates**

1. Confirmer la portée (fournisseur unique vs flotte) via le panneau de taux de requêtes.
2. Basculez le routage Torii vers des fournisseurs sains (si multi-fournisseur) en basculant `sorafs_gateway_route_weights` dans la config ops (`docs/source/sorafs_gateway_self_cert.md`).
3. Si tous les fournisseurs sont impactés, activez le repli « direct fetch » pour les clients CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triage**- Vérifiez l’utilisation des stream tokens par rapport à `sorafs_gateway_stream_token_limit`.
- Inspectez les logs gateway pour des erreurs TLS ou d’admission.
- Exécutez `scripts/telemetry/run_schema_diff.sh` pour vérifier que le schéma exporté par la passerelle correspond à la version attendue.

**Options de remédiation**

- Redémarrez uniquement le processus gateway affecté ; évitez de recycler tout le cluster sauf si plusieurs fournisseurs échouent.
- Augmentez temporairement la limite de stream tokens de 10 à 15 % si une saturation est confirmée.
- Relancez le self-cert (`scripts/sorafs_gateway_self_cert.sh`) après stabilisation.

**Après l'incident**

- Rédigez un post-mortem P1 avec `docs/source/sorafs/postmortem_template.md`.
- Planifiez un exercice de chaos de suivi si la remédiation nécessite des interventions manuelles.

## Pic d’échecs de preuve (PoR / PoTR)

**Détection**

- Alertes : `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Tableau de bord : `dashboards/grafana/sorafs_proof_integrity.json`.
- Télémétrie : `torii_sorafs_proof_stream_events_total` et événements `sorafs.fetch.error` avec `provider_reason=corrupt_proof`.

**Actions immédiates**

1. Figer les nouvelles admissions de manifestes en marquant le registre de manifestes (`docs/source/sorafs/manifest_pipeline.md`).
2. Notifier la gouvernance pour maintenir les incitations des fournisseurs impactés.

**Triage**- Vérifier la profondeur de la fiche des challenges PoR face à `sorafs_node_replication_backlog_total`.
- Validez le pipeline de vérification des preuves (`crates/sorafs_node/src/potr.rs`) pour les déploiements récents.
- Comparez les versions de firmware des fournisseurs avec le registre des opérateurs.

**Options de remédiation**

- Déclenchez les replays PoR via `sorafs_cli proof stream` avec le dernier manifeste.
- Si les preuves échouent de manière cohérente, retirer le fournisseur de l’ensemble actif en mettant à jour le registre de gouvernance et en forçant un rafraîchissement des tableaux de bord de l’orchestrateur.

**Après l'incident**

- Lancez le scénario de drill de chaos PoR avant le prochain déploiement en production.
- Consignez les enseignements dans le template de postmortem et mettez à jour la checklist de qualification des fournisseurs.

## Retard de réplication / croissance du backlog

**Détection**

- Alertes : `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importez
  `dashboards/alerts/sorafs_capacity_rules.yml` et exécutez
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  avant promotion pour qu’Alertmanager reflète les seuils documentés.
- Tableau de bord : `dashboards/grafana/sorafs_capacity_health.json`.
- Métriques : `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Actions immédiates**

1. Vérifiez la portée du backlog (fournisseur unique ou flotte) et mettez en pause les tâches de réplication non essentielles.
2. Si le backlog est isolé, réaffectez temporairement les nouvelles commandes des fournisseurs alternatifs via le planificateur de réplication.**Triage**

- Inspectez la télémétrie orchestrateur pour des rafales de tentatives qui pourraient faire exploser le backlog.
- Confirmez que les cibles de stockage ont suffisamment de marge (`sorafs_node_capacity_utilisation_percent`).
- Passer en revue les changements récents de configuration (mises à jour de chunk profile, cadence des preuves).

**Options de remédiation**

- Exécutez `sorafs_cli` avec l’option `--rebalance` pour redistribuer le contenu.
- Scalez horizontalement les ouvriers de réplication pour le fournisseur impacté.
- Déclenchez un rafraîchissement des manifestes pour réaligner les fenêtres TTL.

**Après l'incident**

- Planifiez un forage de capacité ciblant les échecs de saturation du fournisseur.
- Mettre à jour la documentation SLA de réplication dans `docs/source/sorafs_node_client_protocol.md`.

## Cadence des exercices de chaos

- **Trimestriel** : simulation combinée de panne gateway + tempête de tentatives orchestrateur.
- **Semestriel** : injection d’échecs PoR/PoTR sur deux fournisseurs avec recovery.
- **Spot-check mensuel** : scénario de retard de réplication avec manifestes de staging.
- Suivez les exercices dans le runbook log partagé (`ops/drill-log.md`) via :

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- Validez le log avant les commits avec :

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```- Utilisez `--status scheduled` pour les exercices à venir, `pass`/`fail` pour les exécutions terminées, et `follow-up` lorsque les actions restent ouvertes.
- Remplacez la destination par `--log` pour les essais à sec ou la vérification automatisée ; sans cela, le script continue de mettre à jour `ops/drill-log.md`.

## Modèle d'autopsie

Utilisez `docs/source/sorafs/postmortem_template.md` pour chaque incident P1/P2 et pour les rétrospectives de drills de chaos. Le modèle couvre la chronologie, la quantification d’impact, les facteurs contributifs, les actions correctives et les tâches de vérification de suivi.