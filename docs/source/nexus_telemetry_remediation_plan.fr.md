---
lang: fr
direction: ltr
source: docs/source/nexus_telemetry_remediation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19d46f99e2ba79c56cbc3af65b47f5fb6997fa66f8ee951806b21696418a1d7b
source_last_modified: "2025-11-27T14:13:33.645951+00:00"
translation_last_reviewed: 2026-01-01
---

% Plan de remediation de la telemetrie Nexus (Phase B2)

# Vue generale

Le point de roadmap **B2 - telemetry gap ownership** exige un plan publie qui relie chaque gap de
telemetrie Nexus restant a un signal, un guardrail d'alerte, un owner, une echeance et un artefact
de verification avant le debut des fenetres d'audit Q1 2026. Ce document centralise cette matrice
pour que release engineering, telemetry ops et owners SDK confirment la couverture avant les
rehearsals routed-trace et `TRACE-TELEMETRY-BRIDGE`.

# Matrice des gaps

| Gap ID | Signal et guardrail d'alerte | Owner / Escalation | Echeance (UTC) | Evidence et verification |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histogramme `torii_lane_admission_latency_seconds{lane_id,endpoint}` avec alerte **`SoranetLaneAdmissionLatencyDegraded`** qui se declenche quand `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` pendant 5 minutes (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (signal) + `@telemetry-ops` (alerte) - escalader via l'astreinte Nexus routed-trace. | 2026-02-23 | Tests d'alerte sous `dashboards/alerts/tests/soranet_lane_rules.test.yml` et capture du rehearsal `TRACE-LANE-ROUTING` montrant l'alerte declenchee/retablie, plus le scrape Torii `/metrics` archive dans `docs/source/nexus_transition_notes.md`. |
| `GAP-TELEM-002` | Compteur `nexus_config_diff_total{knob,profile}` avec guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` qui bloque les deploys (`docs/source/telemetry.md`). | `@nexus-core` (instrumentation) -> `@telemetry-ops` (alerte) - le duty officer de gouvernance est notifie quand le compteur augmente de facon inattendue. | 2026-02-26 | Sorties de dry-run de gouvernance stockees pres de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; la checklist de release inclut la capture de la requete Prometheus plus l'extrait de logs prouvant que `StateTelemetry::record_nexus_config_diff` a emis le diff. |
| `GAP-TELEM-003` | Evenement `TelemetryEvent::AuditOutcome` (metrique `nexus.audit.outcome`) avec alerte **`NexusAuditOutcomeFailure`** quand des echecs ou des issues manquantes persistent >30 minutes (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) avec escalation vers `@sec-observability`. | 2026-02-27 | Le gate CI `scripts/telemetry/check_nexus_audit_outcome.py` archive des payloads NDJSON et echoue quand une fenetre TRACE manque d'un evenement de succes; captures d'alerte jointes au rapport routed-trace. |
| `GAP-TELEM-004` | Jauge `nexus_lane_configured_total` surveillee avec guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` (documente dans `docs/source/telemetry.md`) alimentant la checklist d'astreinte SRE. | `@telemetry-ops` (gauge/export) avec escalation vers `@nexus-core` quand des noeuds rapportent des tailles de catalogue incoherentes. | 2026-02-28 | Le test de telemetrie du scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` prouve l'emission; les operateurs joignent un diff Prometheus plus l'extrait de log `StateTelemetry::set_nexus_catalogs` au package TRACE. |

# Budget d'export et limites OTLP

- **Decision (2026-02-11):** plafonner les exporteurs OTLP a **5 MiB/min par noeud** ou
  **25,000 spans/min**, selon le plus bas, avec une taille de lot de 256 spans et un timeout
  d'export de 10 secondes. Les exports au-dela de 80% du plafond declenchent l'alerte
  `NexusOtelExporterSaturated` dans `dashboards/alerts/nexus_telemetry_rules.yml` et emettent
  l'evenement `telemetry_export_budget_saturation` pour les logs d'audit.
- **Enforcement:** les regles Prometheus lisent les compteurs `iroha.telemetry.export.bytes_total`
  et `iroha.telemetry.export.spans_total`; le profil du collecteur OTLP fournit les memes limites
  par defaut et les configs sur noeud ne doivent pas les relever sans derogation de gouvernance.
- **Evidence:** les vecteurs de test d'alerte et les limites approuvees sont archives sous
  `docs/source/nexus_transition_notes.md` aux cotes des artefacts d'audit routed-trace. L'acceptation
  B2 considere maintenant le budget d'export comme clos.

# Flux operationnel

1. **Triage hebdomadaire.** Les owners rapportent l'avancement lors de l'appel de readiness Nexus;
   les blocages et artefacts de tests d'alerte sont consignes dans `status.md`.
2. **Dry-runs d'alertes.** Chaque regle d'alerte est livree avec une entree
   `dashboards/alerts/tests/*.test.yml` pour que CI execute `promtool test rules` a chaque changement
   de guardrail.
3. **Evidence d'audit.** Pendant les rehearsals `TRACE-LANE-ROUTING` et `TRACE-TELEMETRY-BRIDGE`,
   l'astreinte capture les resultats des requetes Prometheus, l'historique d'alertes et les sorties
   des scripts pertinents (`scripts/telemetry/check_nexus_audit_outcome.py`,
   `scripts/telemetry/check_redaction_status.py` pour signaux correles) et les stocke avec les
   artefacts routed-trace.
4. **Escalation.** Si un guardrail se declenche hors d'une fenetre de rehearsal, l'equipe proprietaire
   ouvre un ticket d'incident Nexus referencant ce plan, avec le snapshot de metriques et les
   etapes de mitigation avant de reprendre les audits.

Avec cette matrice publiee et referencee depuis `roadmap.md` et `status.md`, l'item de roadmap
**B2** satisfait maintenant les criteres d'acceptation "responsabilite, echeance, alerte,
verification".
