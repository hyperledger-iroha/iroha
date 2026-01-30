---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/nexus/telemetry-remediation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f46222cc1e6f7f50d232eb520e5cf65fdcc07b2fe9196577fdf234e7fb247c94
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-telemetry-remediation
title: Plan de remediation de telemetrie Nexus (B2)
description: Miroir de `docs/source/nexus_telemetry_remediation_plan.md`, documentant la matrice des ecarts de telemetrie et le flux operationnel.
---

# Vue d'ensemble

L'element de roadmap **B2 - ownership des ecarts de telemetrie** exige un plan publie reliant chaque ecart de telemetrie restant de Nexus a un signal, un guardrail d'alerte, un proprietaire, une date limite et un artefact de verification avant le debut des fenetres d'audit de Q1 2026. Cette page reflete `docs/source/nexus_telemetry_remediation_plan.md` afin que release engineering, telemetry ops et les owners SDK puissent confirmer la couverture avant les repetitions routed-trace et `TRACE-TELEMETRY-BRIDGE`.

# Matrice des ecarts

| ID d'ecart | Signal et guardrail d'alerte | Proprietaire / escalade | Echeance (UTC) | Preuves et verification |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histogramme `torii_lane_admission_latency_seconds{lane_id,endpoint}` avec l'alerte **`SoranetLaneAdmissionLatencyDegraded`** declenchee quand `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` pendant 5 minutes (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (signal) + `@telemetry-ops` (alerte); escalade via l'on-call routed-trace Nexus. | 2026-02-23 | Tests d'alerte sous `dashboards/alerts/tests/soranet_lane_rules.test.yml` plus la capture de la repetition `TRACE-LANE-ROUTING` montrant l'alerte declenchee/retablie et le scrape Torii `/metrics` archive dans [Nexus transition notes](./nexus-transition-notes). |
| `GAP-TELEM-002` | Compteur `nexus_config_diff_total{knob,profile}` avec guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` bloquant les deploiements (`docs/source/telemetry.md`). | `@nexus-core` (instrumentation) -> `@telemetry-ops` (alerte); l'officier de garde gouvernance est page lorsque le compteur augmente de maniere inattendue. | 2026-02-26 | Sorties de dry-run de gouvernance stockees a cote de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; la checklist de release inclut la capture de la requete Prometheus plus l'extrait de logs prouvant que `StateTelemetry::record_nexus_config_diff` a emis le diff. |
| `GAP-TELEM-003` | Evenement `TelemetryEvent::AuditOutcome` (metrique `nexus.audit.outcome`) avec l'alerte **`NexusAuditOutcomeFailure`** lorsque des echecs ou des outcomes manquants persistent >30 minutes (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) avec escalade vers `@sec-observability`. | 2026-02-27 | La gate CI `scripts/telemetry/check_nexus_audit_outcome.py` archive des payloads NDJSON et echoue lorsqu'une fenetre TRACE ne contient pas d'evenement de succes; captures d'alertes jointes au rapport routed-trace. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` avec guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` alimentant la checklist SRE on-call. | `@telemetry-ops` (gauge/export) avec escalade vers `@nexus-core` lorsque les noeuds signalent des tailles de catalogue incoherentes. | 2026-02-28 | Le test de telemetrie du scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` prouve l'emission; les operateurs joignent un diff Prometheus + extrait de log `StateTelemetry::set_nexus_catalogs` au package de repetition TRACE. |

# Flux operationnel

1. **Triage hebdomadaire.** Les owners rapportent l'avancement lors de l'appel de readiness Nexus; les blocages et artefacts de tests d'alerte sont consignes dans `status.md`.
2. **Tests d'alerte.** Chaque regle d'alerte est livree avec une entree `dashboards/alerts/tests/*.test.yml` afin que CI execute `promtool test rules` quand le guardrail evolue.
3. **Preuves d'audit.** Pendant les repetitions `TRACE-LANE-ROUTING` et `TRACE-TELEMETRY-BRIDGE`, l'on-call capture les resultats de requetes Prometheus, l'historique des alertes et les sorties de scripts pertinentes (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` pour les signaux correles) et les stocke avec les artefacts routed-trace.
4. **Escalade.** Si un guardrail se declenche hors d'une fenetre repetee, l'equipe proprietaire ouvre un ticket d'incident Nexus en reference a ce plan, incluant le snapshot de la metrique et les etapes de mitigation avant de reprendre les audits.

Avec cette matrice publiee - et referencee depuis `roadmap.md` et `status.md` - l'item de roadmap **B2** satisfait maintenant aux criteres d'acceptation "responsabilite, echeance, alerte, verification".
