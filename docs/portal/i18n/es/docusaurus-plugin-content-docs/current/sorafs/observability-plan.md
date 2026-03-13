---
id: observability-plan
lang: es
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canónica
Esta página refleja el plan mantenido en `docs/source/sorafs_observability_plan.md`. Mantén ambas copias sincronizadas hasta que el conjunto de documentación Sphinx se migre por completo.
:::

## Objetivos
- Definir métricas y eventos estructurados para gateways, nodos y el orquestador multifuente.
- Proveer dashboards de Grafana, umbrales de alerta y hooks de validación.
- Establecer objetivos SLO junto con políticas de presupuesto de error y drills de caos.

## Catálogo de métricas

### Superficies del gateway

| Métrica | Tipo | Etiquetas | Notas |
|--------|------|-----------|-------|
| `sorafs_gateway_active` | Gauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Emitido vía `SorafsGatewayOtel`; rastrea operaciones HTTP en vuelo por combinación de endpoint/método. |
| `sorafs_gateway_responses_total` | Counter | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Cada solicitud completada del gateway incrementa una vez; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogram | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Latencia de time-to-first-byte para respuestas del gateway; exportada como Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Counter | `profile_version`, `result`, `error_code` | Resultados de verificación de pruebas capturados en el momento de la solicitud (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogram | `profile_version`, `result`, `error_code` | Distribución de latencia de verificación para recibos PoR. |
| `telemetry::sorafs.gateway.request` | Evento estructurado | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Log estructurado emitido al completar cada solicitud para correlación en Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Counter | Conjuntos de etiquetas heredados | Métricas Prometheus retenidas para dashboards históricos; emitidas junto con la nueva serie OTLP. |

Los eventos `telemetry::sorafs.gateway.request` reflejan los contadores OTEL con payloads estructurados, exponiendo `endpoint`, `method`, `variant`, `status`, `error_code` y `duration_ms` para correlación en Loki/Tempo, mientras que los dashboards consumen la serie OTLP para el seguimiento de SLO.

### Telemetría de salud de pruebas

| Métrica | Tipo | Etiquetas | Notas |
|--------|------|-----------|-------|
| `torii_sorafs_proof_health_alerts_total` | Counter | `provider_id`, `trigger`, `penalty` | Se incrementa cada vez que `RecordCapacityTelemetry` emite un `SorafsProofHealthAlert`. `trigger` distingue fallos PDP/PoTR/Ambos, mientras que `penalty` captura si el colateral se recortó realmente o se suprimió por cooldown. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Gauge | `provider_id` | Recuentos más recientes de PDP/PoTR reportados dentro de la ventana de telemetría infractora para que los equipos cuantifiquen cuánto se excedieron los proveedores de la política. |
| `torii_sorafs_proof_health_penalty_nano` | Gauge | `provider_id` | Monto Nano-XOR recortado en la última alerta (cero cuando el cooldown suprimió la aplicación). |
| `torii_sorafs_proof_health_cooldown` | Gauge | `provider_id` | Gauge booleano (`1` = alerta suprimida por cooldown) para mostrar cuándo las alertas de seguimiento están temporalmente silenciadas. |
| `torii_sorafs_proof_health_window_end_epoch` | Gauge | `provider_id` | Época registrada para la ventana de telemetría vinculada a la alerta para que los operadores correlacionen con artefactos Norito. |

Estos feeds ahora alimentan la fila de salud de pruebas del dashboard Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), dando a los operadores CDN visibilidad en tiempo real
de volúmenes de alertas, mezcla de disparadores PDP/PoTR, penalizaciones y estado de cooldown por
proveedor.

Las mismas métricas ahora respaldan dos reglas de alerta del Taikai viewer:
`SorafsProofHealthPenalty` se dispara cuando
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` aumenta en
los últimos 15 minutos, mientras que `SorafsProofHealthCooldown` lanza una advertencia si un
proveedor permanece en cooldown durante cinco minutos. Ambas alertas viven en
`dashboards/alerts/taikai_viewer_rules.yml` para que los SREs reciban contexto inmediato
cuando la aplicación PoR/PoTR se intensifica.

### Superficies del orquestador

| Métrica / Evento | Tipo | Etiquetas | Productor | Notas |
|-----------------|------|-----------|-----------|-------|
| `sorafs_orchestrator_active_fetches` | Gauge | `manifest_id`, `region` | `FetchMetricsCtx` | Sesiones actualmente en vuelo. |
| `sorafs_orchestrator_fetch_duration_ms` | Histogram | `manifest_id`, `region` | `FetchMetricsCtx` | Histograma de duración en milisegundos; buckets de 1 ms a 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Counter | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Razones: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Distingue causas de reintento (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Captura deshabilitación y conteos de fallos a nivel de sesión. |
| `sorafs_orchestrator_chunk_latency_ms` | Histogram | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Distribución de latencia de fetch por chunk (ms) para análisis de throughput/SLO. |
| `sorafs_orchestrator_bytes_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Bytes entregados por manifest/proveedor; deriva el throughput vía `rate()` en PromQL. |
| `sorafs_orchestrator_stalls_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Cuenta chunks que exceden `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Evento estructurado | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Refleja el ciclo de vida del job (inicio/completado) con payload JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Emitido por racha de reintentos por proveedor; `attempts` cuenta reintentos incrementales (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Se publica cuando un proveedor cruza el umbral de fallos. |
| `telemetry::sorafs.fetch.error` | Evento estructurado | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Registro de fallo terminal, amigable para ingestión en Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Se emite cuando la latencia de chunk supera el límite configurado (refleja contadores de stall). |

### Superficies de nodo / replicación

| Métrica | Tipo | Etiquetas | Notas |
|--------|------|-----------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histogram | `provider_id` | Histograma OTEL del porcentaje de utilización de almacenamiento (exportado como `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Counter | `provider_id` | Contador monotónico de muestras PoR exitosas, derivado de snapshots del scheduler. |
| `sorafs_node_por_failure_total` | Counter | `provider_id` | Contador monotónico de muestras PoR fallidas. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Gauge | `provider` | Gauges Prometheus existentes para bytes usados, profundidad de cola, conteos PoR en vuelo. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Gauge | `provider` | Datos de capacidad/uptime exitoso del proveedor expuestos en el dashboard de capacidad. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Gauge | `provider`, `manifest` | Profundidad del backlog más los contadores acumulados de fallos exportados cada vez que se consulta `/v2/sorafs/por/ingestion/{manifest}`, alimentando el panel/alerta "PoR Stalls". |

### Reparación y SLA

| Métrica | Tipo | Etiquetas | Notas |
|--------|------|-----------|-------|
| `sorafs_repair_tasks_total` | Counter | `status` | Contador OTEL para transiciones de tareas de reparación. |
| `sorafs_repair_latency_minutes` | Histogram | `outcome` | Histograma OTEL para la latencia del ciclo de vida de reparación. |
| `sorafs_repair_queue_depth` | Histogram | `provider` | Histograma OTEL de tareas en cola por proveedor (modo snapshot). |
| `sorafs_repair_backlog_oldest_age_seconds` | Histogram | — | Histograma OTEL de la antigüedad de la tarea en cola más antigua (segundos). |
| `sorafs_repair_lease_expired_total` | Counter | `outcome` | Contador OTEL de expiraciones de lease (`requeued`/`escalated`). |
| `sorafs_repair_slash_proposals_total` | Counter | `outcome` | Contador OTEL de transiciones de propuestas de slash. |
| `torii_sorafs_repair_tasks_total` | Counter | `status` | Contador Prometheus para transiciones de tareas. |
| `torii_sorafs_repair_latency_minutes_bucket` | Histogram | `outcome` | Histograma Prometheus para la latencia del ciclo de vida de reparación. |
| `torii_sorafs_repair_queue_depth` | Gauge | `provider` | Gauge Prometheus de tareas en cola por proveedor. |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | Gauge | — | Gauge Prometheus de la antigüedad de la tarea en cola más antigua (segundos). |
| `torii_sorafs_repair_lease_expired_total` | Counter | `outcome` | Contador Prometheus de expiraciones de lease. |
| `torii_sorafs_slash_proposals_total` | Counter | `outcome` | Contador Prometheus de transiciones de propuestas de slash. |

Los metadatos JSON de auditoría de gobernanza reflejan las etiquetas de telemetría de reparación (`status`, `ticket_id`, `manifest`, `provider` en eventos de reparación; `outcome` en propuestas de slash) para correlacionar métricas y artefactos de auditoría de forma determinística.

### Prueba de recuperación oportuna (PoTR) y SLA de chunks

| Métrica | Tipo | Etiquetas | Productor | Notas |
|--------|------|-----------|-----------|-------|
| `sorafs_potr_deadline_ms` | Histogram | `tier`, `provider` | Coordinador PoTR | Holgura del deadline en milisegundos (positivo = cumplido). |
| `sorafs_potr_failures_total` | Counter | `tier`, `provider`, `reason` | Coordinador PoTR | Razones: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Counter | `provider`, `manifest_id`, `reason` | Monitor de SLA | Se dispara cuando la entrega de chunks incumple el SLO (latencia, tasa de éxito). |
| `sorafs_chunk_sla_violation_active` | Gauge | `provider`, `manifest_id` | Monitor de SLA | Gauge booleano (0/1) alternado durante la ventana de incumplimiento activa. |

## Objetivos SLO

- Disponibilidad trustless del gateway: **99.9%** (respuestas HTTP 2xx/304).
- TTFB P95 trustless: hot tier ≤ 120 ms, warm tier ≤ 300 ms.
- Tasa de éxito de pruebas: ≥ 99.5% por día.
- Éxito del orquestador (finalización de chunks): ≥ 99%.

## Dashboards y alertas

1. **Observabilidad del gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) — rastrea disponibilidad trustless, TTFB P95, desglose de rechazos y fallos PoR/PoTR vía las métricas OTEL.
2. **Salud del orquestador** (`dashboards/grafana/sorafs_fetch_observability.json`) — cubre carga multifuente, reintentos, fallos de proveedores y ráfagas de stalls.
3. **Métricas de privacidad de SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — grafica buckets de relay anonimizados, ventanas de supresión y salud de collectors vía `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` y `soranet_privacy_poll_errors_total{provider}`.

Paquetes de alertas:

- `dashboards/alerts/sorafs_gateway_rules.yml` — disponibilidad del gateway, TTFB, picos de fallos de pruebas.
- `dashboards/alerts/sorafs_fetch_rules.yml` — fallos/reintentos/stalls del orquestador; validado vía `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` y `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — picos de degradación de privacidad, alarmas de supresión, detección de collector inactivo y alertas de collector deshabilitado (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — alarmas de brownout de anonimato conectadas a `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — alarmas de deriva/ingest/CEK lag del Taikai viewer más las nuevas alertas de penalización/cooldown de salud de pruebas SoraFS impulsadas por `torii_sorafs_proof_health_*`.

## Estrategia de trazas

- Adopta OpenTelemetry de extremo a extremo:
  - Los gateways emiten spans OTLP (HTTP) anotados con IDs de solicitud, digests de manifest y hashes de token.
  - El orquestador usa `tracing` + `opentelemetry` para exportar spans de intentos de fetch.
  - Los nodos SoraFS embebidos exportan spans para desafíos PoR y operaciones de almacenamiento. Todos los componentes comparten un trace ID común propagado vía `x-sorafs-trace`.
- `SorafsFetchOtel` conecta las métricas del orquestador a histogramas OTLP mientras que los eventos `telemetry::sorafs.fetch.*` proporcionan payloads JSON ligeros para backends centrados en logs.
- Collectors: ejecuta collectors OTEL junto con Prometheus/Loki/Tempo (Tempo preferido). Los exportadores API Jaeger siguen siendo opcionales.
- Las operaciones de alta cardinalidad deben muestrearse (10% para rutas de éxito, 100% para fallos).

## Coordinación de telemetría TLS (SF-5b)

- Alineación de métricas:
  - La automatización TLS envía `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` y `sorafs_gateway_tls_ech_enabled`.
  - Incluye estos gauges en el dashboard Gateway Overview bajo el panel TLS/Certificates.
- Vinculación de alertas:
  - Cuando se disparen alertas de expiración TLS (≤ 14 días restantes) correlaciona con el SLO de disponibilidad trustless.
  - La deshabilitación de ECH emite una alerta secundaria que referencia tanto los paneles TLS como de disponibilidad.
- Pipeline: el job de automatización TLS exporta al mismo stack Prometheus que las métricas del gateway; la coordinación con SF-5b asegura instrumentación deduplicada.

## Convenciones de nombres y etiquetas de métricas

- Los nombres de métricas siguen los prefijos existentes `torii_sorafs_*` o `sorafs_*` usados por Torii y el gateway.
- Los conjuntos de etiquetas están estandarizados:
  - `result` → resultado HTTP (`success`, `refused`, `failed`).
  - `reason` → código de rechazo/error (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → identificador de proveedor codificado en hex.
  - `manifest` → digest canónico de manifest (recortado cuando hay alta cardinalidad).
  - `tier` → etiquetas declarativas de tier (`hot`, `warm`, `archive`).
- Puntos de emisión de telemetría:
  - Las métricas del gateway viven bajo `torii_sorafs_*` y reutilizan convenciones de `crates/iroha_core/src/telemetry.rs`.
  - El orquestador emite métricas `sorafs_orchestrator_*` y eventos `telemetry::sorafs.fetch.*` (lifecycle, retry, provider failure, error, stall) etiquetados con digest de manifest, job ID, región e identificadores de proveedor.
  - Los nodos exponen `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` y `torii_sorafs_por_*`.
- Coordina con Observability para registrar el catálogo de métricas en el documento compartido de nombres Prometheus, incluyendo expectativas de cardinalidad de etiquetas (límites superiores de proveedor/manifests).

## Pipeline de datos

- Los collectors se despliegan junto a cada componente, exportando OTLP a Prometheus (métricas) y Loki/Tempo (logs/trazas).
- eBPF opcional (Tetragon) enriquece el trazado de bajo nivel para gateways/nodos.
- Usa `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` para Torii y nodos embebidos; el orquestador continúa llamando a `install_sorafs_fetch_otlp_exporter`.

## Hooks de validación

- Ejecuta `scripts/telemetry/test_sorafs_fetch_alerts.sh` durante CI para asegurar que las reglas de alerta de Prometheus permanezcan en sintonía con métricas de stalls y checks de supresión de privacidad.
- Mantén los dashboards de Grafana bajo control de versiones (`dashboards/grafana/`) y actualiza capturas/links cuando cambien los paneles.
- Los drills de caos registran resultados vía `scripts/telemetry/log_sorafs_drill.sh`; la validación usa `scripts/telemetry/validate_drill_log.sh` (consulta el [Playbook de operaciones](operations-playbook.md)).
