---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : lien-télémétrie-remédiation
titre : Plan de remédiation de télémétrie de Nexus (B2)
description : Espejo de `docs/source/nexus_telemetry_remediation_plan.md`, documente la matrice des brins de télémétrie et le flux opérationnel.
---

# CV général

L'élément de la feuille de route **B2 - Propriété des barrières de télémétrie** nécessite un plan publié pour que chaque barrière de télémétrie pendente de Nexus avec un sénat, un garde-corps d'alerte, un responsable, une date limite et un artefact de vérification avant la mise en service des fenêtres d'auditoire du premier trimestre 2026. Cette page Réfléchissez à `docs/source/nexus_telemetry_remediation_plan.md` pour que l'ingénierie des versions, les opérations de télémétrie et les responsables du SDK confirment la couverture avant les analyses de routed-trace et `TRACE-TELEMETRY-BRIDGE`.

# Matrice de brechas| ID de brecha | Senal et garde-corps d'alerte | Responsable / escalade | Fécha (UTC) | Preuve et vérification |
|--------|-------------------------|----------|---------------|-------------------------|
| `GAP-TELEM-001` | L'histogramme `torii_lane_admission_latency_seconds{lane_id,endpoint}` contient l'alerte **`SoranetLaneAdmissionLatencyDegraded`** qui disparaît lorsque `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` dure 5 minutes (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (senal) + `@telemetry-ops` (alerte) ; escalader via l'appel de routed-trace de Nexus. | 2026-02-23 | Vérifiez l'alerte en `dashboards/alerts/tests/soranet_lane_rules.test.yml` mais la capture de l'analyse `TRACE-LANE-ROUTING` montre l'alerte disparada/récupérée et le grattage de Torii `/metrics` archivé en [transition Nexus remarques] (./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` avec garde-corps `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` qui bloquea despliegues (`docs/source/telemetry.md`). | `@nexus-core` (instrumentation) -> `@telemetry-ops` (alerte) ; se pagina al oficial de guardia de gobernanza cuando el contador incrémenta de forma inesperada. | 2026-02-26 | Salidas de dry-run de gobernanza almacenadas junto a `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ; la liste de contrôle de publication inclut la capture de la consultation de Prometheus mais l'extraction des journaux qui vérifie que `StateTelemetry::record_nexus_config_diff` émet le diff. || `GAP-TELEM-003` | Événement `TelemetryEvent::AuditOutcome` (métrique `nexus.audit.outcome`) avec alerte **`NexusAuditOutcomeFailure`** lorsque des résultats erronés persistent pendant >30 minutes (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) avec escalade vers `@sec-observability`. | 2026-02-27 | L'ordinateur de CI `scripts/telemetry/check_nexus_audit_outcome.py` archive les charges utiles NDJSON et tombe lorsqu'une fenêtre TRACE prend soin d'un événement de sortie ; captures d'alertes adjointes au rapport routed-trace. |
| `GAP-TELEM-004` | Jauge `nexus_lane_configured_total` avec garde-corps `nexus_lane_configured_total != EXPECTED_LANE_COUNT` qui alimente la liste de contrôle d'astreinte de SRE. | `@telemetry-ops` (jauge/export) avec escalade vers `@nexus-core` lorsque les nœuds signalent des tamanos de catalogo incohérents. | 2026-02-28 | L'essai de télémétrie du planificateur `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` détecte l'émission ; les opérateurs adjoints Prometheus diff + extrait du journal `StateTelemetry::set_nexus_catalogs` au paquet de l'analyse TRACE. |

# Flux opérationnel1. **Triage semanal.** Les propriétaires rapportent l'avancement de l'appel de préparation de Nexus ; les bloqueurs et les artefacts de tests d'alerte sont enregistrés en `status.md`.
2. **Ensayos de alertas.** Chaque règle d'alerte est entrée avec une entrée en `dashboards/alerts/tests/*.test.yml` pour que CI éjecte `promtool test rules` lorsque vous modifiez le garde-corps.
3. **Preuve d'audience.** Durant les analyses `TRACE-LANE-ROUTING` et `TRACE-TELEMETRY-BRIDGE`, la capture sur appel des résultats de consultation de Prometheus, l'historique des alertes et les sorties pertinentes de scripts (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` para senales correlacionadas) y los almacena con los artefactos routé-trace.
4. **Escalamiento.** Si un garde-corps disparaît hors d'une fenêtre fermée, l'équipe responsable ouvre un ticket d'incident Nexus qui fait référence à ce plan, y compris l'instantané de la mesure et les étapes d'atténuation avant de réparer les auditoires.

Avec cette matrice publiée - et référencée depuis `roadmap.md` et `status.md` - l'élément de la feuille de route **B2** regroupe maintenant les critères d'acceptation "responsabilité, limite, alerte, vérification".