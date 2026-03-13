---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan de observabilidad
título: План наблюдаемости и SLO для SoraFS
sidebar_label: Наблюдаемость y SLO
descripción: Схема телеметрии, дашборды and политика бюджета ошибок для gateways SoraFS, узлов и мульти-источникового оркестратора.
---

:::nota Канонический источник
Esta página está reemplazada por un plan que se puede utilizar en `docs/source/sorafs_observability_plan.md`. Deje copias sincronizadas de la estrella Sphinx que no es migratoria.
:::

## Цели
- Ajuste de parámetros y estructuras de las puertas de enlace, dispositivos y operadores de múltiples sistemas.
- Paneles anteriores Grafana, alertas y ganchos de validación.
- Зафиксировать цели SLO вместе с политиками бюджета ошибок и хаос-дрилами.

## Métrica del catálogo

### Puerta de enlace de seguridad| Métrica | Consejo | Metálicas | Примечания |
|---------|-----|-------|------------|
| `sorafs_gateway_active` | Medidor (ContadorArribaAbajo) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Эмитируется через `SorafsGatewayOtel`; Se utilizan operaciones HTTP en combinaciones de punto final/método. |
| `sorafs_gateway_responses_total` | Mostrador | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Каждая завершенная gateway-запросом операция увеличивает счетчик один раз; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histograma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Латентность tiempo hasta el primer byte para la puerta de enlace anterior; Експортируется как Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Mostrador | `profile_version`, `result`, `error_code` | Результаты проверки доказательств фиксируются в момент запроса (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histograma | `profile_version`, `result`, `error_code` | Распределение латентности проверки для PoR-replik. || `telemetry::sorafs.gateway.request` | Структурированное событие | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Registro estructural, emitido por archivos guardados según la correlación de Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Mostrador | Otros métodos de trabajo | Métricas Prometheus, compatibles con tableros históricos; Емитируются вместе с новой серией OTLP. |

События `telemetry::sorafs.gateway.request` отражают OTEL-счетчики со структурированными payloads, показывая `endpoint`, `method`, `variant`, `status`, `error_code` y `duration_ms` para correlacionar con Loki/Tempo, también como tablero de instrumentos utiliza la serie OTLP para отслеживания SLO.

### Телеметрия здоровья доказательств| Métrica | Consejo | Metálicas | Примечания |
|---------|-----|-------|------------|
| `torii_sorafs_proof_health_alerts_total` | Mostrador | `provider_id`, `trigger`, `penalty` | Инкрементируется каждый раз, когда `RecordCapacityTelemetry` эмитирует `SorafsProofHealthAlert`. `trigger` funciona con PDP/PoTR/Both, y `penalty` activa, con una cola real y un tiempo de reutilización más rápido. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Calibre | `provider_id` | Después de que PDP/PoTR tenga problemas con los televisores, los comandos que pueden mostrarse y los proveedores de políticas anteriores. |
| `torii_sorafs_proof_health_penalty_nano` | Calibre | `provider_id` | Además de Nano-XOR, aparece una alerta posterior (no así, el tiempo de reutilización se activará primero). |
| `torii_sorafs_proof_health_cooldown` | Calibre | `provider_id` | Indicador de volumen (`1` = alerta de enfriamiento de tiempo de reutilización), que muestra, como resultado de alertas recientes. |
| `torii_sorafs_proof_health_window_end_epoch` | Calibre | `provider_id` | Cuando se activan los televisores, se activa la alarma y los operadores pueden correlacionarse con los artefactos Norito. |

Эти фиды теперь питают строкуproof-health en el visor Taikai
(`dashboards/grafana/taikai_viewer.json`), el operador CDN muestra el vídeo
Alertas de advertencia, activadores de PDP/PoTR, penalizaciones y tiempos de reutilización
провайдерам.Estas métricas pueden variar según el espectador Taikai:
`SorafsProofHealthPenalty` срабатывает, когда
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` растет
Después de 15 minutos, aparece una advertencia `SorafsProofHealthCooldown`, o
провайдер остается в cooldown пять minут. Оба алерта живут в
`dashboards/alerts/taikai_viewer_rules.yml`, чтобы SRes получали контекст
немедленно при эскалации принуждения PoR/PoTR.

### Поверхности оркестратора| Métrica / Событие | Consejo | Metálicas | Производитель | Примечания |
|-------------------|-----|-------|---------------|------------|
| `sorafs_orchestrator_active_fetches` | Calibre | `manifest_id`, `region` | `FetchMetricsCtx` | Сессии, находящиеся в полете. |
| `sorafs_orchestrator_fetch_duration_ms` | Histograma | `manifest_id`, `region` | `FetchMetricsCtx` | Гистограмма длительности в миллисекундах; cubos de 1 ms a 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Mostrador | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Principales: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Mostrador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Различает причины ретраев (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Mostrador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Фиксирует отключения и счетчики отказов на уровне сессий. |
| `sorafs_orchestrator_chunk_latency_ms` | Histograma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Распределение латентности buscar fragmento (ms) para analizar el rendimiento/SLO. |
| `sorafs_orchestrator_bytes_total` | Mostrador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Байты, доставленные на manifiesto/proveedor; El rendimiento está determinado por `rate()` en PromQL. |
| `sorafs_orchestrator_stalls_total` | Mostrador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Считает trozos, anterior `ScoreboardConfig::latency_cap_ms`. || `telemetry::sorafs.fetch.lifecycle` | Структурированное событие | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Haga un trabajo de ciclo corto (inicio/completo) con la carga útil JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Структурированное событие | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Эмитируется на каждую серию ретраев провайдера; `attempts` считает инкрементальные ретраи (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Структурированное событие | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Публикуется, когда провайдер пересекает порог отказов. |
| `telemetry::sorafs.fetch.error` | Структурированное событие | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | El primer paso final para ingerirlo en Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Структурированное событие | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Выдается, когда латентность trozo превышает настроенный лимит (отражает puesto-счетчики). |

### Поверхности узлов / репликации| Métrica | Consejo | Metálicas | Примечания |
|---------|-----|-------|------------|
| `sorafs_node_capacity_utilisation_pct` | Histograma | `provider_id` | OTEL-гистограмма процента использования хранилища (экспортируется как `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Mostrador | `provider_id` | El programador de instantáneas utilizado en el programador de instantáneas es muy popular. |
| `sorafs_node_por_failure_total` | Mostrador | `provider_id` | Монотонный счетчик неуспешных PoR-выборок. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Calibre | `provider` | Medidor de calibre Prometheus para байтов, глубины очереди, PoR en vuelo. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Calibre | `provider` | Debido a la falta de capacidad/tiempo de actividad, se eliminan las emisiones del tablero. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Calibre | `provider`, `manifest` | La cartera de trabajos pendientes de Глубина más накопленные счетчики ошибок, экспортируемые при каждом опросе `/v2/sorafs/por/ingestion/{manifest}`, питают панель/алерт "PoR Puestos". |

### Prueba de recuperación oportuna (PoTR) y SLA por fragmentos| Métrica | Consejo | Metálicas | Производитель | Примечания |
|---------|-----|-------|---------------|------------|
| `sorafs_potr_deadline_ms` | Histograma | `tier`, `provider` | Coordinador del PoTR | Запас по дедлайну в миллисекундах (положительный = выполнен). |
| `sorafs_potr_failures_total` | Mostrador | `tier`, `provider`, `reason` | Coordinador del PoTR | Principales: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Mostrador | `provider`, `manifest_id`, `reason` | Monitor de SLA | Срабатывает, когда доставка fragments не укладывается в SLO (латентность, tasa de éxito). |
| `sorafs_chunk_sla_violation_active` | Calibre | `provider`, `manifest_id` | Monitor de SLA | Булевый calibre (0/1), переключаемый во время активного окна нарушения. |

## Цели SLO

- Puerta de enlace sin confianza: **99,9%** (HTTP 2xx/304 ответы).
- Trustless TTFB P95: nivel activo ≤ 120 ms, nivel activo ≤ 300 ms.
- Успешность доказательств: ≥ 99,5% en el día.
- Успех оркестратора (завершение trozos): ≥ 99%.

## Ciudades y alertas1. **Наблюдаемость gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) — отслеживает trustless-доступность, TTFB P95, разбиение отказов and сбои PoR/PoTR через метрики OTEL.
2. **Здоровье оркестратора** (`dashboards/grafana/sorafs_fetch_observability.json`) — покрывает нагрузку puestos de múltiples fuentes, reproductores, отказы провайдеров и всплески.
3. **Метрики приватности SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — gráficos anónimos de cubos de retransmisión, окон подавления y здоровья recopilador через `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` y `soranet_privacy_poll_errors_total{provider}`.

Paquetes de alertas:

- `dashboards/alerts/sorafs_gateway_rules.yml` — puerta de enlace de descarga, TTFB, всплески отказов доказательств.
- `dashboards/alerts/sorafs_fetch_rules.yml` — отказы/ретраи/puestos оркестратора; Los valores válidos son `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` y `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — всплески деградации приватности, alarmas подавления, детекция indle-collector y alertas отключенного colector (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — alarmas de caída de tensión anonima, proporcionadas por `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml`: alarmas de recepción/ingesta/retraso CEK del visor Taikai, además de nuevas alertas de penalización/enfriamiento para dispositivos SoraFS, disponibles en `torii_sorafs_proof_health_*`.

## Стратегия трассировки- Implementar OpenTelemetry de extremo a extremo:
  - Las puertas de enlace emiten intervalos OTLP (HTTP) con ID de solicitud annotadas, resúmenes de manifiesto y hashes de tokens.
  - El operador utiliza `tracing` + `opentelemetry` para exportar tramos de recuperación de datos.
  - El espacio de almacenamiento SoraFS se extiende por PoR-челленджей y la operación de almacenamiento. Estos componentes eliminan el ID de seguimiento, primero que el `x-sorafs-trace`.
- `SorafsFetchOtel` organiza la mayoría de las métricas en los registros OTLP, y `telemetry::sorafs.fetch.*` contiene cargas útiles JSON legítimas para backends de registro.
- Coleccionistas: запускайте OTEL coleccionistas рядом с Prometheus/Loki/Tempo (Tempo предпочтителен). Los exportadores Jaeger están disponibles de forma opcional.
- Операции с высокой кардинальностью следует сэмплировать (10% для успешных путей, 100% для отказов).

## Telemetro de coordinacion TLS (SF-5b)- Medición de medición:
  - Automatización TLS publicadas `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` y `sorafs_gateway_tls_ech_enabled`.
  - Abra estos medidores en el panel Gateway Overview en los paneles TLS/Certificados.
- Enviar alertas:
  - Когда срабатывают алерты истечения TLS (≤ 14 дней осталось), коррелируйте с trustless disponibilidad SLO.
  - ECH emite alertas automáticas, actualizadas en TLS y en paneles de disponibilidad.
- Pipeline: el trabajo de automatización TLS se exporta a la pila Prometheus, a la puerta de enlace y a las medidas; координация с SF-5b garantiza la deduplicación del instrumento.

## Конвенции именования и меток- El parámetro métrico establece las configuraciones de configuración `torii_sorafs_*` o `sorafs_*`, utiliza Torii y gateway.
- Наборы меток стандартизированы:
  - `result` → Resultado HTTP (`success`, `refused`, `failed`).
  - `reason` → код отказа/ошибки (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → probador de identificador con codificación hexadecimal.
  - `manifest` → manifiesto de resumen canónico (обрезается при высокой кардинальности).
  - `tier` → декларативные метки tier (`hot`, `warm`, `archive`).
- Точки эмиссии телеметрии:
  - La puerta de enlace métrica se conecta a `torii_sorafs_*` y se adapta a las convenciones de `crates/iroha_core/src/telemetry.rs`.
  - Emisor de parámetros `sorafs_orchestrator_*` y `telemetry::sorafs.fetch.*` (ciclo de vida, reintento, falla del proveedor, error, bloqueo), manifiesto de resumen detallado, ID de trabajo, región e identificadores провайдера.
  - Las publicaciones `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` y `torii_sorafs_por_*`.
- Coordinado con Observabilidad, cómo registrar la métrica del catálogo en el documento con el nombre Prometheus, con el nombre Prometheus. кардинальности меток (proveedor/manifiestos de gran tamaño).

## Canalización de datos- Los coleccionistas cuentan con componentes de cámara, exportan OTLP en Prometheus (métricas) y Loki/Tempo (líneas/transmisiones).
- La opción eBPF (Tetragon) proporciona conexiones no deseadas para puertas de enlace/puertas de enlace.
- Utilice `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` para Torii y dispositivos externos; El operador deberá utilizar `install_sorafs_fetch_otlp_exporter`.

## Ganchos de validación

- Introduzca `scripts/telemetry/test_sorafs_fetch_alerts.sh` en CI, que proporciona alertas Prometheus que se sincronizan con paradas métricas y supresión de dispositivos. приватности.
- Coloque el tablero de instrumentos Grafana en la versión de control (`dashboards/grafana/`) y cambie las pantallas/dispositivos de configuración панелей.
- Ejercicios de caos логируют результаты через `scripts/telemetry/log_sorafs_drill.sh`; валидация использует `scripts/telemetry/validate_drill_log.sh` (см. [Playbook операций](operations-playbook.md)).