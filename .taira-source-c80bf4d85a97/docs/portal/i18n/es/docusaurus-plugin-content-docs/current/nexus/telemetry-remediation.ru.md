---
lang: es
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: reparación-telemetría-nexus
título: План устранения пробелов телеметрии Nexus (B2)
descripción: Зеркало `docs/source/nexus_telemetry_remediation_plan.md`, documentación de la matriz de prueba de televisores y procesos operativos.
---

#Obzor

Hoja de ruta de Punkt **B2 - владение пробелами телеметрии** требует опубликованного плана, который привязывает каждый оставшийся пробел Telemetros Nexus ke señal, защитному порогу оповещений, владельцу, дедлайну и артефакту проверки до начала окон Auditoría del primer trimestre de 2026. Esta página contiene `docs/source/nexus_telemetry_remediation_plan.md`, ingeniería de versiones, operaciones de telemetría y SDK que pueden mejorarse durante el período. Repeticiones de seguimiento enrutado y `TRACE-TELEMETRY-BRIDGE`.

# Матрица пробелов| ID de brecha | Señales y señales de tráfico | Владелец / эскалация | Срок (UTC) | Доказательства y проверка |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | El sistema `torii_lane_admission_latency_seconds{lane_id,endpoint}` con la alerta **`SoranetLaneAdmissionLatencyDegraded`**, se conecta a `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` durante 5 minutos (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (signal) + `@telemetry-ops` (alerta); эскалация через rastreo enrutado de guardia Nexus. | 2026-02-23 | Pruebe la alerta en `dashboards/alerts/tests/soranet_lane_rules.test.yml` además de borrar las repeticiones de `TRACE-LANE-ROUTING` con alerta y descarga y raspado de archivos Torii `/metrics` в [Notas de transición Nexus](./nexus-transition-notes). |
| `GAP-TELEM-002` | Счетчик `nexus_config_diff_total{knob,profile}` с barandilla `increase(nexus_config_diff_total{profile="active"}[5m]) > 0`, блокирующим деплой (`docs/source/telemetry.md`). | `@nexus-core` (instrumento) -> `@telemetry-ops` (alerta); дежурный по gobernancia пейджится при неожиданном росте счетчика. | 2026-02-26 | Выходы gobierno simulacro сохраняются рядом с `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; Lista de verificación de la pantalla de inicio Prometheus y otros logotipos, incluidos `StateTelemetry::record_nexus_config_diff` diferencial diferencial. || `GAP-TELEM-003` | Событие `TelemetryEvent::AuditOutcome` (métrica `nexus.audit.outcome`) con alerta **`NexusAuditOutcomeFailure`** de alarmas o apagadores resultados de aproximadamente 30 minutos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (tubería) с эскалацией в `@sec-observability`. | 2026-02-27 | CI-гейт `scripts/telemetry/check_nexus_audit_outcome.py` архивирует NDJSON payloads and падает, когда окно TRACE не содержит события успеха; скриншоты алертов прикладываются к отчету de seguimiento enrutado. |
| `GAP-TELEM-004` | Calibre `nexus_lane_configured_total` con barandilla `nexus_lane_configured_total != EXPECTED_LANE_COUNT`, который питает de guardia чеклист SRE. | `@telemetry-ops` (medidor/exportación) с эскалацией в `@nexus-core`, когда узлы сообщают о несовпадающих размерах каталога. | 2026-02-28 | La planificación del televisor de prueba `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` permite emitir emisiones; Los operadores utilizan la diferencia Prometheus + el registro `StateTelemetry::set_nexus_catalogs` en paquetes repetitivos TRACE. |

# Операционный рабочий процесс1. **Еженедельный триаж.** Владельцы отчитываются о прогрессе на Nexus readiness созвоне; Bloqueadores y artefactos prueban alertas de fallas en `status.md`.
2. **Alertas de funcionamiento en seco.** La alerta de funcionamiento se realiza con el cierre `dashboards/alerts/tests/*.test.yml`, o con el CI cerrado `promtool test rules` изменении barandilla.
3. **Доказательства для аудита.** Во время репетиций `TRACE-LANE-ROUTING` and `TRACE-TELEMETRY-BRIDGE` дежурный собирает результаты запросов Prometheus, historias de alertas y scripts de textos relevantes (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` para señales de correlación) y сохраняет их вместе с артефактами de seguimiento enrutado.
4. **Эскалация.** Si la barandilla se retira de forma repetitiva, el comando-владелец открывает Nexus incidente, ссылаясь En este plan, se utilizan métricas de instantáneas y se detectan riesgos de detección antes de realizar auditorías.

С опубликованной матрицей - и ссылками из `roadmap.md` и `status.md` - hoja de ruta del punto **B2** теперь соответствует критериям приемки "ответственность, срок, алерт, проверка".