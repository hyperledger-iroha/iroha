---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/operations-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d67ab604b3e59a3b8a94e83e775bb8f5fd7083fafdc29d351046e1ed6c76f8e4
source_last_modified: "2026-01-03T18:08:01+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: operations-playbook
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Cette page reflète le runbook maintenu dans `docs/source/sorafs_ops_playbook.md`. Gardez les deux copies synchronisées jusqu’à ce que la documentation Sphinx soit totalement migrée.
:::

## Références clés

- Actifs d’observabilité : consultez les dashboards Grafana sous `dashboards/grafana/` et les règles d’alerte Prometheus dans `dashboards/alerts/`.
- Catalogue de métriques : `docs/source/sorafs_observability_plan.md`.
- Surfaces de télémétrie de l’orchestrateur : `docs/source/sorafs_orchestrator_plan.md`.

## Matrice d’escalade

| Priorité | Exemples de déclenchement | On-call principal | Backup | Notes |
|----------|---------------------------|------------------|--------|-------|
| P1 | Panne globale gateway, taux d’échec PoR > 5% (15 min), backlog de réplication doublant toutes les 10 min | Storage SRE | Observability TL | Engager le conseil de gouvernance si l’impact dépasse 30 min. |
| P2 | Violation du SLO de latence gateway régionale, pic de retries orchestrateur sans impact SLA | Observability TL | Storage SRE | Continuer le rollout mais bloquer les nouveaux manifests. |
| P3 | Alertes non critiques (staleness de manifests, capacité 80–90%) | Intake triage | Ops guild | À traiter dans le prochain jour ouvré. |

## Panne gateway / disponibilité dégradée

**Détection**

- Alertes : `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Dashboard : `dashboards/grafana/sorafs_gateway_overview.json`.

**Actions immédiates**

1. Confirmer la portée (fournisseur unique vs flotte) via le panneau de taux de requêtes.
2. Basculez le routage Torii vers des fournisseurs sains (si multi-fournisseur) en basculant `sorafs_gateway_route_weights` dans la config ops (`docs/source/sorafs_gateway_self_cert.md`).
3. Si tous les fournisseurs sont impactés, activez le fallback “direct fetch” pour les clients CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triage**

- Vérifiez l’utilisation des stream tokens par rapport à `sorafs_gateway_stream_token_limit`.
- Inspectez les logs gateway pour des erreurs TLS ou d’admission.
- Exécutez `scripts/telemetry/run_schema_diff.sh` pour vérifier que le schéma exporté par le gateway correspond à la version attendue.

**Options de remédiation**

- Redémarrez uniquement le processus gateway affecté ; évitez de recycler tout le cluster sauf si plusieurs fournisseurs échouent.
- Augmentez temporairement la limite de stream tokens de 10–15% si une saturation est confirmée.
- Relancez le self-cert (`scripts/sorafs_gateway_self_cert.sh`) après stabilisation.

**Post-incident**

- Rédigez un postmortem P1 avec `docs/source/sorafs/postmortem_template.md`.
- Planifiez un drill de chaos de suivi si la remédiation a nécessité des interventions manuelles.

## Pic d’échecs de preuve (PoR / PoTR)

**Détection**

- Alertes : `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Dashboard : `dashboards/grafana/sorafs_proof_integrity.json`.
- Télémétrie : `torii_sorafs_proof_stream_events_total` et événements `sorafs.fetch.error` avec `provider_reason=corrupt_proof`.

**Actions immédiates**

1. Figer les nouvelles admissions de manifests en marquant le registre de manifests (`docs/source/sorafs/manifest_pipeline.md`).
2. Notifier la gouvernance pour suspendre les incitations des fournisseurs impactés.

**Triage**

- Vérifiez la profondeur de la file des challenges PoR face à `sorafs_node_replication_backlog_total`.
- Validez le pipeline de vérification des preuves (`crates/sorafs_node/src/potr.rs`) pour les déploiements récents.
- Comparez les versions de firmware des fournisseurs avec le registre des opérateurs.

**Options de remédiation**

- Déclenchez des replays PoR via `sorafs_cli proof stream` avec le dernier manifest.
- Si les preuves échouent de manière cohérente, retirez le fournisseur de l’ensemble actif en mettant à jour le registre de gouvernance et en forçant un refresh des scoreboards de l’orchestrateur.

**Post-incident**

- Lancez le scénario de drill de chaos PoR avant le prochain déploiement en production.
- Consignez les enseignements dans le template de postmortem et mettez à jour la checklist de qualification des fournisseurs.

## Retard de réplication / croissance du backlog

**Détection**

- Alertes : `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importez
  `dashboards/alerts/sorafs_capacity_rules.yml` et exécutez
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  avant promotion pour qu’Alertmanager reflète les seuils documentés.
- Dashboard : `dashboards/grafana/sorafs_capacity_health.json`.
- Métriques : `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Actions immédiates**

1. Vérifiez la portée du backlog (fournisseur unique ou flotte) et mettez en pause les tâches de réplication non essentielles.
2. Si le backlog est isolé, réaffectez temporairement les nouvelles commandes à des fournisseurs alternatifs via le scheduler de réplication.

**Triage**

- Inspectez la télémétrie orchestrateur pour des rafales de retries qui pourraient faire exploser le backlog.
- Confirmez que les cibles de stockage ont suffisamment de headroom (`sorafs_node_capacity_utilisation_percent`).
- Passez en revue les changements récents de configuration (mises à jour de chunk profile, cadence des proofs).

**Options de remédiation**

- Exécutez `sorafs_cli` avec l’option `--rebalance` pour redistribuer le contenu.
- Scalez horizontalement les workers de réplication pour le fournisseur impacté.
- Déclenchez un refresh des manifests pour réaligner les fenêtres TTL.

**Post-incident**

- Planifiez un drill de capacité ciblant les échecs de saturation fournisseur.
- Mettez à jour la documentation SLA de réplication dans `docs/source/sorafs_node_client_protocol.md`.

## Cadence des drills de chaos

- **Trimestriel** : simulation combinée de panne gateway + tempête de retries orchestrateur.
- **Semestriel** : injection d’échecs PoR/PoTR sur deux fournisseurs avec recovery.
- **Spot-check mensuel** : scénario de retard de réplication avec manifests de staging.
- Suivez les drills dans le runbook log partagé (`ops/drill-log.md`) via :

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
  ```

- Utilisez `--status scheduled` pour les drills à venir, `pass`/`fail` pour les runs terminés, et `follow-up` quand des actions restent ouvertes.
- Remplacez la destination avec `--log` pour les dry-runs ou la vérification automatisée ; sans cela, le script continue de mettre à jour `ops/drill-log.md`.

## Template de postmortem

Utilisez `docs/source/sorafs/postmortem_template.md` pour chaque incident P1/P2 et pour les rétrospectives de drills de chaos. Le template couvre la chronologie, la quantification d’impact, les facteurs contributifs, les actions correctives et les tâches de vérification de suivi.
