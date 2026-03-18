---
lang: es
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: reparación-telemetría-nexus
título: Plan de remediación de telemetría Nexus (B2)
descripción: Miroir de `docs/source/nexus_telemetry_remediation_plan.md`, documenta la matriz de datos electrónicos de telemetría y el flujo operativo.
---

# Vista del conjunto

El elemento de hoja de ruta **B2 - propiedad de los tarjetas electrónicas de telemetría** exige un plan público basado en cada tarjeta electrónica de telemetría restante de Nexus a una señal, una barandilla de alerta, un propietario, una fecha límite y un artefacto de verificación antes del debut de las ventanas de auditoría del primer trimestre de 2026. Esta página reflejada `docs/source/nexus_telemetry_remediation_plan.md` después de que la ingeniería de lanzamiento, las operaciones de telemetría y los propietarios del SDK puedan confirmar la cobertura antes de las repeticiones de ruta enrutada y `TRACE-TELEMETRY-BRIDGE`.

# Matrice des carts| ID de tarjeta electrónica | Señal y barandilla de alerta | Propietario / escalada | Écheance (UTC) | Preuves y verificación |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histograma `torii_lane_admission_latency_seconds{lane_id,endpoint}` con alerta **`SoranetLaneAdmissionLatencyDegraded`** declinado cuando `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` dura 5 minutos (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (señal) + `@telemetry-ops` (alerta); escalada a través del rastreo enrutado de guardia Nexus. | 2026-02-23 | Tests d'alerte sous `dashboards/alerts/tests/soranet_lane_rules.test.yml` plus la captura de la repetición `TRACE-LANE-ROUTING` montrant l'alerte declenchee/retablie et le scrape Torii `/metrics` archive dans [Nexus transición notas](./nexus-transition-notes). |
| `GAP-TELEM-002` | Compteur `nexus_config_diff_total{knob,profile}` con guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` bloquea las implementaciones (`docs/source/telemetry.md`). | `@nexus-core` (instrumentación) -> `@telemetry-ops` (alerta); El oficial de guardia de gobierno está en la página cuando el contador aumenta la falta de atención. | 2026-02-26 | Sorties de dry-run de gouvernance stockees a cote de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; la lista de verificación de liberación incluye la captura de la solicitud Prometheus más el extracto de registros que proporciona `StateTelemetry::record_nexus_config_diff` a emitir le diff. || `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (métrica `nexus.audit.outcome`) con la alerta **`NexusAuditOutcomeFailure`** cuando los resultados o las pruebas persisten durante >30 minutos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (canalización) con escalade versiones `@sec-observability`. | 2026-02-27 | La puerta CI `scripts/telemetry/check_nexus_audit_outcome.py` archive des payloads NDJSON y echoue lorsqu'une fenetre TRACE no contiene eventos de éxito; Captura de alertas conjuntas au rapport routed-trace. |
| `GAP-TELEM-004` | Calibre `nexus_lane_configured_total` con barandilla `nexus_lane_configured_total != EXPECTED_LANE_COUNT` alimentant la checklist SRE de guardia. | `@telemetry-ops` (medidor/exportación) con escalade vers `@nexus-core` lorsque les noeuds signalent des tailles de catalog incoherentes. | 2026-02-28 | La prueba de telemetría del programador `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` prueba la emisión; Los operadores unen un diff Prometheus + extraen el registro `StateTelemetry::set_nexus_catalogs` en el paquete de repetición TRACE. |

# Operación de flujo1. **Triage hebdomadaire.** Les propietarios informan el avance lors de l'appel de readiness Nexus; les blocages et artefactos de pruebas de alerta son consignados en `status.md`.
2. **Pruebas de alerta.** Cada regla de alerta está disponible con un entree `dashboards/alerts/tests/*.test.yml` después de que CI ejecute `promtool test rules` cuando el guardrail evolucione.
3. **Preuves d'audit.** Durante las repeticiones `TRACE-LANE-ROUTING` y `TRACE-TELEMETRY-BRIDGE`, la llamada captura los resultados de las solicitudes Prometheus, el historial de alertas y las salidas de scripts pertinentes (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` pour les signaux correles) et les stocke avec les artefactos routed-trace.
4. **Escalada.** Si una barandilla se baja entre una ventana repetida, el equipo propietario abrirá un ticket de incidente Nexus en referencia a este plan, incluyendo la instantánea de la métrica y las etapas de mitigación antes de reprender las auditorías.

Con esta matriz publicada - y referencias posteriores `roadmap.md` e `status.md` - el elemento de la hoja de ruta **B2** satisface el mantenimiento de los criterios de aceptación "responsabilidad, responsabilidad, alerta, verificación".