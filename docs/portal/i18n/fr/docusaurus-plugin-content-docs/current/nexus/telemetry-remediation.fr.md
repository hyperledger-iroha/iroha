---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : lien-télémétrie-remédiation
titre : Plan de remédiation de télémétrie Nexus (B2)
description : Miroir de `docs/source/nexus_telemetry_remediation_plan.md`, documentant la matrice des ecarts de télémétrie et le flux opérationnel.
---

# Vue d'ensemble

L'élément de roadmap **B2 -ownership des ecarts de telemetrie** exige un plan public dépendant de chaque ecart de telemetrie restant de Nexus a un signal, un garde-corps d'alerte, un propriétaire, une date limite et un artefact de vérification avant le début des fenêtres d'audit de Q1 2026. Cette page reflète `docs/source/nexus_telemetry_remediation_plan.md` afin que release Engineering, telemetry ops et les propriétaires SDK peuvent confirmer la couverture avant les répétitions routed-trace et `TRACE-TELEMETRY-BRIDGE`.

# Matrice des écarts| ID du panier | Signal et garde-corps d'alerte | Propriétaire / escalade | Écheance (UTC) | Preuves et vérification |
|--------|-------------------------|----------|---------------|-------------------------|
| `GAP-TELEM-001` | Histogramme `torii_lane_admission_latency_seconds{lane_id,endpoint}` avec l'alerte **`SoranetLaneAdmissionLatencyDegraded`** declenchée lorsque `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` pendant 5 minutes (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (signal) + `@telemetry-ops` (alerte) ; escalade via l'on-call routé-trace Nexus. | 2026-02-23 | Tests d'alerte sous `dashboards/alerts/tests/soranet_lane_rules.test.yml` plus la capture de la répétition `TRACE-LANE-ROUTING` affichant l'alerte déclenchée/rétablie et le scrape Torii `/metrics` archive dans [Nexus transition notes](./nexus-transition-notes). |
| `GAP-TELEM-002` | Compteur `nexus_config_diff_total{knob,profile}` avec garde-corps `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` bloquant les déploiements (`docs/source/telemetry.md`). | `@nexus-core` (instrumentation) -> `@telemetry-ops` (alerte) ; l'officier de garde gouvernance est page lorsque le compteur augmente de manière inattendue. | 2026-02-26 | Sorties de dry-run de gouvernance stockées a cote de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; la checklist de release inclut la capture de la requête Prometheus plus l'extrait de logs prouvant que `StateTelemetry::record_nexus_config_diff` a émis le diff. || `GAP-TELEM-003` | Événement `TelemetryEvent::AuditOutcome` (métrique `nexus.audit.outcome`) avec l'alerte **`NexusAuditOutcomeFailure`** lorsque des échecs ou des résultats manquants persistants >30 minutes (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) avec escalade vers `@sec-observability`. | 2026-02-27 | La porte CI `scripts/telemetry/check_nexus_audit_outcome.py` archive des payloads NDJSON et échoue lorsqu'une fenêtre TRACE ne contient pas d'événement de succès; capture d'alertes jointes au rapport routé-trace. |
| `GAP-TELEM-004` | Jauge `nexus_lane_configured_total` avec garde-corps `nexus_lane_configured_total != EXPECTED_LANE_COUNT` alimentant la liste de contrôle SRE de garde. | `@telemetry-ops` (jauge/export) avec escalade vers `@nexus-core` lorsque les noeuds signalent des tailles de catalogue incohérentes. | 2026-02-28 | Le test de télémétrie du planificateur `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` prouve l'émission; les opérateurs joignent un diff Prometheus + extrait de log `StateTelemetry::set_nexus_catalogs` au package de répétition TRACE. |

# Flux opérationnel1. **Triage hebdomadaire.** Les propriétaires rapportent l'avancement lors de l'appel de préparation Nexus; les blocages et artefacts de tests d'alerte sont consignés dans `status.md`.
2. **Tests d'alerte.** Chaque règle d'alerte est livrée avec une entrée `dashboards/alerts/tests/*.test.yml` afin que CI exécute `promtool test rules` lorsque le garde-corps évolue.
3. **Preuves d'audit.** Pendant les répétitions `TRACE-LANE-ROUTING` et `TRACE-TELEMETRY-BRIDGE`, l'on-call capture les résultats de requêtes Prometheus, l'historique des alertes et les sorties de scripts pertinents (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` pour les signaux correles) et les stocke avec les artefacts routé-trace.
4. **Escalade.** Si un garde-corps se declenche hors d'une fenêtre répétée, l'équipe propriétaire ouvre un ticket d'incident Nexus en référence à ce plan, incluant le snapshot de la métrique et les étapes d'atténuation avant de reprendre les audits.

Avec cette matrice publiée - et référencee depuis `roadmap.md` et `status.md` - l'item de roadmap **B2** satisfait maintenant aux critères d'acceptation "responsabilité, équité, alerte, vérification".