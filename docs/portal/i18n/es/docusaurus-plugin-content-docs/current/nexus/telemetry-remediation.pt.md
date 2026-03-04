---
lang: es
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: reparación-telemetría-nexus
título: Plano de reparación de telemetría do Nexus (B2)
descripción: Espelho de `docs/source/nexus_telemetry_remediation_plan.md`, documentando a matriz de lagunas de telemetria e o fluxo operacional.
---

# Visao general

El elemento de la hoja de ruta **B2 - propiedad de lagunas de telemetría** exige un plano publicado que vincule cada laguna de telemetría pendiente de Nexus a una señal, una barandilla de alerta, una respuesta, un prazo y un artefato de verificación antes del inicio de las janelas de auditoría del primer trimestre de 2026. Esta página espelha `docs/source/nexus_telemetry_remediation_plan.md` para que la ingeniería de lanzamiento, las operaciones de telemetría y los propietarios del SDK confirmen una cobertura antes de los dos ensayos routed-trace e `TRACE-TELEMETRY-BRIDGE`.

# Matriz de lagunas| ID de la laguna | Señal y barandilla de alerta | Responsavel / escalada | Prazo (UTC) | Pruebas y verificação |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Histograma `torii_lane_admission_latency_seconds{lane_id,endpoint}` con alerta **`SoranetLaneAdmissionLatencyDegraded`** disparando cuando `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` por 5 minutos (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (sinal) + `@telemetry-ops` (alerta); escalamiento a través del rastreo enrutado de guardia de Nexus. | 2026-02-23 | Testes de alerta em `dashboards/alerts/tests/soranet_lane_rules.test.yml` mais o registro do ensaio `TRACE-LANE-ROUTING` mostrando alerta disparado/recuperado e o scrape Torii `/metrics` arquivado em [Nexus notas de transición](./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` con guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` bloqueando despliegues (`docs/source/telemetry.md`). | `@nexus-core` (instrumento) -> `@telemetry-ops` (alerta); oficial de gobierno y paginado cuando el contador incrementa de forma inesperada. | 2026-02-26 | Saidas de dry-run degobernanza armazenadas al lado de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; a checklist de release incluye una captura de consulta Prometheus mais o trecho de log provando que `StateTelemetry::record_nexus_config_diff` emitiu o diff. || `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (métrica `nexus.audit.outcome`) con alerta **`NexusAuditOutcomeFailure`** cuando faltan o resultados ausentes persisten durante >30 minutos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (tubería) con escalamiento para `@sec-observability`. | 2026-02-27 | La puerta de CI `scripts/telemetry/check_nexus_audit_outcome.py` carga útil de archivo NDJSON y falta cuando una janela TRACE nao tem evento de éxito; capturas de alerta anexadas al relatorio routed-trace. |
| `GAP-TELEM-004` | Calibre `nexus_lane_configured_total` com guardarrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` alimentando una lista de verificación de guardia de SRE. | `@telemetry-ops` (gauge/export) con escalamiento para `@nexus-core` cuando los informes de tamaños de catálogo son inconsistentes. | 2026-02-28 | La prueba de telemetría del planificador `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` comprova a emissao; operadores anexam diff de Prometheus + trecho de log `StateTelemetry::set_nexus_catalogs` ao pacote do ensaio TRACE. |

# flujo operacional1. **Triagem semanal.** Owners reportam Progresso na chamada de readiness do Nexus; blockers e artefactos de testículos de alerta sao registrados en `status.md`.
2. **Ensaios de alertas.** Cada regra de alerta e entregue junto con una entrada `dashboards/alerts/tests/*.test.yml` para que el CI ejecute `promtool test rules` cuando o guardrail mudar.
3. **Evidencia de auditoría.** Durante los ensayos `TRACE-LANE-ROUTING` e `TRACE-TELEMETRY-BRIDGE` o captura de resultados de consultas Prometheus, histórico de alertas y dichas relevantes de scripts (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` para sinais correlacionados) e como armazena con los artefatos routed-trace.
4. **Escalonamento.** Se algum guardrail disparar fora de uma janela ensaiada, a equipe responsavel abre um ticket de incidente Nexus referenciando este plano, incluyendo o snapshot da métrica y os passos de mitigacao antes de retomar as auditorias.

Como esta matriz publicada - y referenciada en `roadmap.md` e `status.md` - el elemento de hoja de ruta **B2** ahora atiende a los criterios de aceitacao "responsabilidade, prazo, alerta, verificacao".