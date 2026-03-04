---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : lien-télémétrie-remédiation
titre : Plan de réparation de télémétrie par Nexus (B2)
description : Espelho de `docs/source/nexus_telemetry_remediation_plan.md`, documentant la matrice des lacunes de télémétrie et le flux opérationnel.
---

# Visa général

L'élément de la feuille de route **B2 - Propriété des lacunes de télémétrie** exige un plan publié pour que chaque lacune de télémétrie pendante du Nexus soit un signal, un garde-corps d'alerte, une responsabilité, un prazo et un artéfact de vérification avant le début des auditoires du premier trimestre 2026. La page affiche `docs/source/nexus_telemetry_remediation_plan.md` pour que l'ingénierie des versions, les opérations de télémétrie et les propriétaires du SDK confirment la couverture avant l'enregistrement de routed-trace et `TRACE-TELEMETRY-BRIDGE`.

# Matrice de lacunes| ID de la lacune | Sinal et garde-corps d'alerte | Responsavel / escalade | Prazo (UTC) | Preuves et vérifications |
|--------|-------------------------|----------|---------------|-------------------------|
| `GAP-TELEM-001` | L'histogramme `torii_lane_admission_latency_seconds{lane_id,endpoint}` avec l'alerte **`SoranetLaneAdmissionLatencyDegraded`** disparaît lorsque `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` pendant 5 minutes (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (sinal) + `@telemetry-ops` (alerte) ; escalade via la trace acheminée sur appel de Nexus. | 2026-02-23 | Les tests d'alerte dans `dashboards/alerts/tests/soranet_lane_rules.test.yml` mais l'enregistrement de l'essai `TRACE-LANE-ROUTING` montrent l'alerte disparado/récupérée et le scrape Torii `/metrics` arquivado em [Nexus transition remarques] (./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` avec garde-corps `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` bloqueando se déploie (`docs/source/telemetry.md`). | `@nexus-core` (instrument) -> `@telemetry-ops` (alerte); officiel de gouvernance et paginado lorsque le contacteur s'incrémente de forme inattendue. | 2026-02-26 | Informations sur les armazenadas de gouvernance à sec en direction de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ; une liste de contrôle de version comprend une capture de la consultation Prometheus mais le trecho de journal indiquant que `StateTelemetry::record_nexus_config_diff` émet le diff. || `GAP-TELEM-003` | Événement `TelemetryEvent::AuditOutcome` (métrique `nexus.audit.outcome`) avec alerte **`NexusAuditOutcomeFailure`** lorsque des erreurs ou des résultats ausentés persistent pendant >30 minutes (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) avec escalade pour `@sec-observability`. | 2026-02-27 | La porte de CI `scripts/telemetry/check_nexus_audit_outcome.py` archive les charges utiles NDJSON et échoue lorsqu'une nouvelle TRACE n'a pas réussi l'événement ; captures d'alerte anexadas au rapport routed-trace. |
| `GAP-TELEM-004` | Jauge `nexus_lane_configured_total` avec garde-corps `nexus_lane_configured_total != EXPECTED_LANE_COUNT` alimentant une liste de contrôle d'astreinte de SRE. | `@telemetry-ops` (jauge/export) avec escalade pour `@nexus-core` lorsque nos rapports portent sur des catalogues incohérents. | 2026-02-28 | Le test de télémétrie du planificateur `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` comprend l'émission ; opérateurs anexam diff de Prometheus + trecho de log `StateTelemetry::set_nexus_catalogs` à pacote do ensaio TRACE. |

# Flux opérationnel1. **Triage semanal.** Les propriétaires signalent la progression de l'état de préparation du Nexus ; bloqueurs et artéfacts de testicules d'alerte sao enregistrés au `status.md`.
2. **Ensaios de alertas.** Chaque fois, regraissez l'alerte et entrez avec une entrée `dashboards/alerts/tests/*.test.yml` pour que le CI exécute `promtool test rules` lorsque le garde-corps change.
3. **Preuve de l'auditoire.** Durant les enquêtes `TRACE-LANE-ROUTING` et `TRACE-TELEMETRY-BRIDGE` ou la capture sur appel des résultats des consultations Prometheus, l'historique des alertes et des déclarations pertinentes pour les scripts (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` pour sinais correlacionados) e comme armazena com os artefatos routed-trace.
4. **Escalonamento.** Si un garde-corps disparaît pour une jeune fille ensaiada, l'équipe responsable est responsable d'un ticket d'incident Nexus en référence à ce plan, y compris l'instantané de la mesure et les étapes d'atténuation avant le retour en salle.

Avec cette matrice publiée - et référencée aux `roadmap.md` et `status.md` - l'élément de la feuille de route **B2** il y a peu d'attention aux critères d'assurance "responsabilité, pratique, alerte, vérification".