---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan de observabilidad
título: Plan d'observabilité et de SLO de SoraFS
sidebar_label: Observabilidad y SLO
descripción: Esquema de télémétrie, paneles de control y política de presupuesto de error para les gateways SoraFS, les nœuds et l'orchestrateur multi-source.
---

:::nota Fuente canónica
Esta página refleja el plan de mantenimiento en `docs/source/sorafs_observability_plan.md`. Guarde las dos copias sincronizadas junto con la migración completa del antiguo conjunto Sphinx.
:::

## Objetivos
- Definir las métricas y los eventos estructurados para las puertas de enlace, los nudos y el orquestador multifuente.
- Coloque los paneles Grafana, las señales de alerta y los ganchos de validación.
- Établir des objectifs SLO avec des politiques de Budget d'erreur et de Drills de Chaos.

## Catálogo de medidas

### Superficies de la puerta de enlace| Métrico | Tipo | Etiquetas | Notas |
|---------|------|------------|---------------|
| `sorafs_gateway_active` | Medidor (ContadorArribaAbajo) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Émis vía `SorafsGatewayOtel`; Se adapta a las operaciones HTTP en volumen mediante una combinación de punto final/método. |
| `sorafs_gateway_responses_total` | Mostrador | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Cada vez que solicite una puerta de enlace terminada cada vez más; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histograma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Tiempo de latencia hasta el primer byte para la puerta de enlace de respuesta; exportée en Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Mostrador | `profile_version`, `result`, `error_code` | Résultats de verification des preuves capturés au moment de la requête (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histograma | `profile_version`, `result`, `error_code` | Distribución de latencia de verificación para los recursos PoR. || `telemetry::sorafs.gateway.request` | Evento estructurado | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | El registro estructurado se emite cada vez que se solicita para la correlación Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Mostrador | Juegos de etiquetas heredadas | Métriques Prometheus conservados para los paneles históricos; Se emite en paralelo a la nueva serie OTLP. |

Los eventos `telemetry::sorafs.gateway.request` reflejan los ordenadores OTEL con cargas útiles estructuradas, exponentes `endpoint`, `method`, `variant`, `status`, `error_code` y `duration_ms` para la correlación Loki/Tempo, así como los paneles de control consumen la serie OTLP para seguir el SLO.

### Telemétrie de santé des preuves| Métrico | Tipo | Etiquetas | Notas |
|---------|------|------------|---------------|
| `torii_sorafs_proof_health_alerts_total` | Mostrador | `provider_id`, `trigger`, `penalty` | Cada vez más, `RecordCapacityTelemetry` aparece un `SorafsProofHealthAlert`. `trigger` distingue los controles PDP/PoTR/Both, así como que `penalty` captura si la garantía es amputada o suprimida por el tiempo de reutilización. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Calibre | `provider_id` | Los últimos recuentos del PDP/PoTR reportados en la ventana de télémétrie fautive para que los equipos cuantifiquen el dépassement de politique par les fournisseurs. |
| `torii_sorafs_proof_health_penalty_nano` | Calibre | `provider_id` | Montant Nano-XOR amputé sur la última alerta (cero cuando el enfriamiento se suprime la aplicación). |
| `torii_sorafs_proof_health_cooldown` | Calibre | `provider_id` | Indicador booleano (`1` = alerta actualizada por enfriamiento) para señalar cuando las alertas siguientes se silencian temporalmente. |
| `torii_sorafs_proof_health_window_end_epoch` | Calibre | `provider_id` | Época registrada para la ventana de télémétrie situada en la alerta después de que los operadores puedan correlacionar con los artefactos Norito. |

Este flujo alimenta la línea Proof-Health del tablero Taikai Viewer
(`dashboards/grafana/taikai_viewer.json`), ofrece a los operadores CDN una visibilidad directa
en los volúmenes de alertas, la combinación de activadores PDP/PoTR, las penalidades y el estado de enfriamiento por
proveedor.Las mismas medidas necesarias para mantener dos reglas de alerta Taikai Viewer:
`SorafsProofHealthPenalty` se abre cuando lo hace
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` aumentar
En los últimos 15 minutos, entonces `SorafsProofHealthCooldown` emitió una advertencia si un
El proveedor retiene el tiempo de reutilización durante cinco minutos. Les dos alertes vivent dans
`dashboards/alerts/taikai_viewer_rules.yml` después de que los SRE dispongan de un contexto inmediato
A medida que se intensifica la aplicación PoR/PoTR.

### Superficies del orquestador| Métrique / Événement | Tipo | Etiquetas | Productor | Notas |
|---------------------|------|------------|------------|-------|
| `sorafs_orchestrator_active_fetches` | Calibre | `manifest_id`, `region` | `FetchMetricsCtx` | Sesiones actuales en vol. |
| `sorafs_orchestrator_fetch_duration_ms` | Histograma | `manifest_id`, `region` | `FetchMetricsCtx` | Histograma de duración en milisegundos; cubos 1 ms a 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Mostrador | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Razones: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Mostrador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Distinga las causas del reintento (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Mostrador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Capture las desactivaciones y las cuentas de control del nivel de sesión. |
| `sorafs_orchestrator_chunk_latency_ms` | Histograma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Distribución de latencia, recuperación por fragmento (ms) para analizar el rendimiento/SLO. |
| `sorafs_orchestrator_bytes_total` | Mostrador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Octetos libros por manifiesto/fournisseur; Deduisez el rendimiento a través de `rate()` en PromQL. || `sorafs_orchestrator_stalls_total` | Mostrador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Cuente los trozos que pasaron `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Evento estructurado | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Refleje el ciclo de vida del trabajo (inicio/completo) con la carga útil JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Émis por racha de reintento por proveedor; `attempts` cuenta los reintentos incrementales (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Publié lorsqu'un fournisseur franchit le seuil d'échecs. |
| `telemetry::sorafs.fetch.error` | Evento estructurado | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Registro de terminal fallido adaptado a la ingesta de Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Evento estructurado | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Se producirá cuando la latencia supere el límite configurado (reflète les compteurs de stop). |### Superficies nœud / réplica

| Métrico | Tipo | Etiquetas | Notas |
|---------|------|------------|---------------|
| `sorafs_node_capacity_utilisation_pct` | Histograma | `provider_id` | Histograma OTEL du pourcentage d'utilisation du stockage (exportado en `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Mostrador | `provider_id` | El ordenador monotone de échantillons PoR réussis, deriva des instantáneas del planificador. |
| `sorafs_node_por_failure_total` | Mostrador | `provider_id` | Compteur monotone des échantillons PoR échoués. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Calibre | `provider` | Calibres Prometheus existentes para octetos utilizados, profundidad de archivo, cuentas PoR en vol. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Calibre | `provider` | Données de capacité/uptime réussies du fournisseur exposées dans le tablero de capacidad. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Calibre | `provider`, `manifest` | Profondeur du backlog plus compteurs cumulés d'échecs exportés à cada interrogation de `/v1/sorafs/por/ingestion/{manifest}`, alimentant le panel/alerta "PoR Stalls". |

### Preuve de récupération en temps utile (PoTR) y SLA de fragmentos| Métrico | Tipo | Etiquetas | Productor | Notas |
|---------|------|------------|------------|-------|
| `sorafs_potr_deadline_ms` | Histograma | `tier`, `provider` | Coordinador PoTR | Marge de date date en milisegundos (positif = respecté). |
| `sorafs_potr_failures_total` | Mostrador | `tier`, `provider`, `reason` | Coordinador PoTR | Razones: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Mostrador | `provider`, `manifest_id`, `reason` | Monitor SLA | Déclenché quand la livraison de chunks rate le SLO (latencia, taux de succès). |
| `sorafs_chunk_sla_violation_active` | Calibre | `provider`, `manifest_id` | Monitor SLA | Calibre booleano (0/1) activado durante la ventana de infracción activa. |

## Objetivos SLO

- Disponibilidad de confianza del gateway: **99,9%** (respuestas HTTP 2xx/304).
- TTFB P95 sin confianza: nivel activo ≤ 120 ms, nivel activo ≤ 300 ms.
- Taux de succès des preuves: ≥ 99,5% por día.
- Succès de l'orchestrateur (finalización de fragmentos): ≥ 99%.

## Paneles y alertas1. **Observabilité gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) — adecuado a la disponibilidad de confianza, TTFB P95, la répartition des refus et les échecs PoR/PoTR via les métriques OTEL.
2. **Santé de l'orchestrateur** (`dashboards/grafana/sorafs_fetch_observability.json`) — cubra la carga multifuente, los reintentos, los échecs fournisseurs y los rafales de puestos.
3. **Métriques de confidencialité SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`): rastrea los cubos de relaciones anónimas, las ventanas de supresión y la salud de los coleccionistas a través de `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` y `soranet_privacy_poll_errors_total{provider}`.

Paquetes de alertas:

- `dashboards/alerts/sorafs_gateway_rules.yml` — gateway de disponibilidad, TTFB, fotografías de cheques previos.
- `dashboards/alerts/sorafs_fetch_rules.yml` — échecs/retrys/stalls de l'orchestrateur ; validado a través de `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` y `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml`: imágenes de degradación de confidencialidad, alarmas de supresión, detección de colector inactivo y alertas de colector desactivado (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml`: alarmas de caída de tensión de cables anónimos en `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml`: alarmas de deriva/ingesta/CEK lag Taikai visor más las nuevas alertas de penalidad/enfriamiento de salud de las preuves SoraFS alimentadas por `torii_sorafs_proof_health_*`.

## Estrategia de rastreo- Adoptador OpenTelemetry de combate y combate:
  - Las puertas de enlace activadas por tramos OTLP (HTTP) anotadas con los ID de solicitud, resúmenes de manifiesto y hashes de token.
  - El orquestador utiliza `tracing` + `opentelemetry` para exportar los intervalos de tentativas de búsqueda.
  - Les nœuds SoraFS embarqués exportent des spans pour les défis PoR et les opérations de stockage. Todos los componentes comparten un ID de seguimiento compartido a través de `x-sorafs-trace`.
- `SorafsFetchOtel` confía en las métricas orquestadas por los histogramas OTLP y los eventos `telemetry::sorafs.fetch.*` proporcionan cargas útiles JSON legibles para los registros centrales del backend.
- Coleccionistas: exécutez des coleccionistas OTEL à côté de Prometheus/Loki/Tempo (Tempo préféré). Los exportadores API Jaeger retienen opciones.
- Les opérations à haute cardinalité doivent être échantillonnées (10% pour les chemins de succès, 100% pour les échecs).

## Coordinación de la televisión TLS (SF-5b)- Alineación de medidas :
  - La automatización TLS publica `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` e `sorafs_gateway_tls_ech_enabled`.
  - Incluye estos indicadores en el panel Gateway Overview en el panel TLS/Certificados.
- Enlace de alertas:
  - Cuando las alertas de vencimiento TLS se apaguen (≤ 14 días restantes), corrélez avec le SLO de disponibilité trustless.
  - La desactivación del ECH genera una alerta secundaria relacionada con las hojas de los paneles TLS y disponibles.
- Pipeline: exporta el trabajo de automatización TLS a la misma pila Prometheus que las puertas de enlace métricas; La coordinación con SF-5b asegura una instrumentación duplicada.

## Convenciones de nommage et d'étiquetage des métriques- Los nombres de parámetros siguientes a los prefijos existentes `torii_sorafs_*` o `sorafs_*` utilizados por Torii y la puerta de enlace.
- Los conjuntos de etiquetas son estandarizados:
  - `result` → resultado HTTP (`success`, `refused`, `failed`).
  - `reason` → código de rechazo/error (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → identificador del proveedor codificado en hexadecimal.
  - `manifest` → digest canonique de manifest (tronqué quand la cardinalité est élevée).
  - `tier` → etiquetas de nivel declarado (`hot`, `warm`, `archive`).
- Puntos de emisión de televisión:
  - Las puertas de enlace métricas viven bajo `torii_sorafs_*` y utilizan las convenciones de `crates/iroha_core/src/telemetry.rs`.
  - El orquestador emite las métricas `sorafs_orchestrator_*` y los eventos `telemetry::sorafs.fetch.*` (ciclo de vida, reintento, falla del proveedor, error, bloqueo) etiquetas con resumen de manifiesto, ID de trabajo, región e identificadores del proveedor.
  - Los nuevos expuestos `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` y `torii_sorafs_por_*`.
- Coordonnez avec Observability pour registrar le catalog des métriques dans le document de nommage Prometheus partagé, y comprende les attentes de cardinalité des label (bornes supérieures fournisseur/manifests).

## Tubería de données- Los recopiladores se implementan en la base de cada componente, exportando OTLP versiones Prometheus (métricas) y Loki/Tempo (registros/trazas).
- La opción eBPF (Tetragon) enriquece el seguimiento del nivel bajo para puertas de enlace/nœuds.
- Utilice `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` para Torii y los nuevos embarques; El orquestador continúa tocando `install_sorafs_fetch_otlp_exporter`.

## Ganchos de validación

- Ejecute `scripts/telemetry/test_sorafs_fetch_alerts.sh` en CI para garantizar que las reglas de alerta Prometheus estén alineadas con las métricas de bloqueo y las verificaciones de supresión de confidencialidad.
- Guarde los paneles Grafana bajo control de versión (`dashboards/grafana/`) y actualice diariamente las capturas/enlaces cuando los paneles cambien.
- Les Drills de Chaos publican los resultados a través de `scripts/telemetry/log_sorafs_drill.sh`; La validación se aplica en `scripts/telemetry/validate_drill_log.sh` (ver el [Playbook de explotación](operations-playbook.md)).