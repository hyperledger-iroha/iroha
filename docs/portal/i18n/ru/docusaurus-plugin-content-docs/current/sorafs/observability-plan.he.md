---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/observability-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9b950177659acd4a9b77ad8e65ebbd95ec598b7ee5f98f338259feca9ddd8cf
source_last_modified: "2026-01-21T07:41:19+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
id: observability-plan
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
Эта страница отражает план, поддерживаемый в `docs/source/sorafs_observability_plan.md`. Держите обе копии синхронизированными, пока старый набор Sphinx полностью не мигрирован.
:::

## Цели
- Определить метрики и структурированные события для gateways, узлов и мульти-источникового оркестратора.
- Предоставить дашборды Grafana, пороги алертов и validation hooks.
- Зафиксировать цели SLO вместе с политиками бюджета ошибок и хаос-дриллами.

## Каталог метрик

### Поверхности gateway

| Метрика | Тип | Метки | Примечания |
|---------|-----|-------|------------|
| `sorafs_gateway_active` | Gauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Эмитируется через `SorafsGatewayOtel`; отслеживает HTTP-операции в полете по комбинации endpoint/method. |
| `sorafs_gateway_responses_total` | Counter | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Каждая завершенная gateway-запросом операция увеличивает счетчик один раз; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogram | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Латентность time-to-first-byte для ответов gateway; экспортируется как Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Counter | `profile_version`, `result`, `error_code` | Результаты проверки доказательств фиксируются в момент запроса (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogram | `profile_version`, `result`, `error_code` | Распределение латентности проверки для PoR-реплик. |
| `telemetry::sorafs.gateway.request` | Структурированное событие | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Структурированный лог, эмитируемый при завершении каждого запроса для корреляции Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Counter | Унаследованные наборы меток | Метрики Prometheus, сохраненные для исторических дашбордов; эмитируются вместе с новой серией OTLP. |

События `telemetry::sorafs.gateway.request` отражают OTEL-счетчики со структурированными payloads, показывая `endpoint`, `method`, `variant`, `status`, `error_code` и `duration_ms` для корреляции в Loki/Tempo, тогда как дашборды используют серию OTLP для отслеживания SLO.

### Телеметрия здоровья доказательств

| Метрика | Тип | Метки | Примечания |
|---------|-----|-------|------------|
| `torii_sorafs_proof_health_alerts_total` | Counter | `provider_id`, `trigger`, `penalty` | Инкрементируется каждый раз, когда `RecordCapacityTelemetry` эмитирует `SorafsProofHealthAlert`. `trigger` различает сбои PDP/PoTR/Both, а `penalty` показывает, был ли коллатерал реально списан или подавлен cooldown. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Gauge | `provider_id` | Последние значения PDP/PoTR в проблемном окне телеметрии, чтобы команды могли оценить, насколько провайдеры превысили политику. |
| `torii_sorafs_proof_health_penalty_nano` | Gauge | `provider_id` | Сумма Nano-XOR, списанная в последней алерте (ноль, если cooldown подавил применение). |
| `torii_sorafs_proof_health_cooldown` | Gauge | `provider_id` | Булевый gauge (`1` = алерт подавлен cooldown), чтобы показать, когда последующие алерты временно приглушены. |
| `torii_sorafs_proof_health_window_end_epoch` | Gauge | `provider_id` | Эпоха окна телеметрии, связанного с алертом, чтобы операторы могли коррелировать с артефактами Norito. |

Эти фиды теперь питают строку proof-health в дашборде Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), давая операторам CDN живую видимость
объемов алертов, микса триггеров PDP/PoTR, пенальти и состояния cooldown по
провайдерам.

Эти же метрики теперь поддерживают две правила алертов Taikai viewer:
`SorafsProofHealthPenalty` срабатывает, когда
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` растет
за последние 15 минут, а `SorafsProofHealthCooldown` поднимает warning, если
провайдер остается в cooldown пять минут. Оба алерта живут в
`dashboards/alerts/taikai_viewer_rules.yml`, чтобы SREs получали контекст
немедленно при эскалации принуждения PoR/PoTR.

### Поверхности оркестратора

| Метрика / Событие | Тип | Метки | Производитель | Примечания |
|-------------------|-----|-------|---------------|------------|
| `sorafs_orchestrator_active_fetches` | Gauge | `manifest_id`, `region` | `FetchMetricsCtx` | Сессии, находящиеся в полете. |
| `sorafs_orchestrator_fetch_duration_ms` | Histogram | `manifest_id`, `region` | `FetchMetricsCtx` | Гистограмма длительности в миллисекундах; buckets от 1 ms до 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Counter | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Причины: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Различает причины ретраев (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Фиксирует отключения и счетчики отказов на уровне сессий. |
| `sorafs_orchestrator_chunk_latency_ms` | Histogram | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Распределение латентности fetch по chunk (ms) для анализа throughput/SLO. |
| `sorafs_orchestrator_bytes_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Байты, доставленные на manifest/provider; throughput вычисляется через `rate()` в PromQL. |
| `sorafs_orchestrator_stalls_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Считает chunks, превышающие `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Структурированное событие | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Отражает жизненный цикл job (start/complete) с Norito JSON payload. |
| `telemetry::sorafs.fetch.retry` | Структурированное событие | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Эмитируется на каждую серию ретраев провайдера; `attempts` считает инкрементальные ретраи (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Структурированное событие | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Публикуется, когда провайдер пересекает порог отказов. |
| `telemetry::sorafs.fetch.error` | Структурированное событие | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Финальная запись об отказе, удобная для ingest в Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Структурированное событие | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Выдается, когда латентность chunk превышает настроенный лимит (отражает stall-счетчики). |

### Поверхности узлов / репликации

| Метрика | Тип | Метки | Примечания |
|---------|-----|-------|------------|
| `sorafs_node_capacity_utilisation_pct` | Histogram | `provider_id` | OTEL-гистограмма процента использования хранилища (экспортируется как `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Counter | `provider_id` | Монотонный счетчик успешных PoR-выборок, полученных из snapshot scheduler. |
| `sorafs_node_por_failure_total` | Counter | `provider_id` | Монотонный счетчик неуспешных PoR-выборок. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Gauge | `provider` | Существующие Prometheus gauge для использованных байтов, глубины очереди, PoR in-flight счетчиков. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Gauge | `provider` | Данные об успехе capacity/uptime провайдера, отображаемые в дашборде емкости. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Gauge | `provider`, `manifest` | Глубина backlog плюс накопленные счетчики ошибок, экспортируемые при каждом опросе `/v1/sorafs/por/ingestion/{manifest}`, питают панель/алерт "PoR Stalls". |

### Ремонт и SLA

| Метрика | Тип | Метки | Примечания |
|---------|-----|-------|------------|
| `sorafs_repair_tasks_total` | Counter | `status` | Счетчик OTEL для переходов задач ремонта. |
| `sorafs_repair_latency_minutes` | Histogram | `outcome` | OTEL-гистограмма для латентности жизненного цикла ремонта. |
| `sorafs_repair_queue_depth` | Histogram | `provider` | OTEL-гистограмма числа задач в очереди по провайдерам (снимок). |
| `sorafs_repair_backlog_oldest_age_seconds` | Histogram | — | OTEL-гистограмма возраста самой старой задачи в очереди (секунды). |
| `sorafs_repair_lease_expired_total` | Counter | `outcome` | Счетчик OTEL для истечений лизинга (`requeued`/`escalated`). |
| `sorafs_repair_slash_proposals_total` | Counter | `outcome` | Счетчик OTEL для переходов slash-предложений. |
| `torii_sorafs_repair_tasks_total` | Counter | `status` | Счетчик Prometheus для переходов задач. |
| `torii_sorafs_repair_latency_minutes_bucket` | Histogram | `outcome` | Гистограмма Prometheus для латентности жизненного цикла ремонта. |
| `torii_sorafs_repair_queue_depth` | Gauge | `provider` | Gauge Prometheus для задач в очереди по провайдерам. |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | Gauge | — | Gauge Prometheus для возраста самой старой задачи в очереди (секунды). |
| `torii_sorafs_repair_lease_expired_total` | Counter | `outcome` | Счетчик Prometheus для истечений лизинга. |
| `torii_sorafs_slash_proposals_total` | Counter | `outcome` | Счетчик Prometheus для переходов slash-предложений. |

JSON-метаданные аудита управления отражают метки телеметрии ремонта (`status`, `ticket_id`, `manifest`, `provider` для событий ремонта; `outcome` для slash-предложений), чтобы детерминированно коррелировать метрики и артефакты аудита.

### Proof of Timely Retrieval (PoTR) и SLA по chunks

| Метрика | Тип | Метки | Производитель | Примечания |
|---------|-----|-------|---------------|------------|
| `sorafs_potr_deadline_ms` | Histogram | `tier`, `provider` | PoTR coordinator | Запас по дедлайну в миллисекундах (положительный = выполнен). |
| `sorafs_potr_failures_total` | Counter | `tier`, `provider`, `reason` | PoTR coordinator | Причины: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Counter | `provider`, `manifest_id`, `reason` | SLA monitor | Срабатывает, когда доставка chunks не укладывается в SLO (латентность, success rate). |
| `sorafs_chunk_sla_violation_active` | Gauge | `provider`, `manifest_id` | SLA monitor | Булевый gauge (0/1), переключаемый во время активного окна нарушения. |

## Цели SLO

- Trustless-доступность gateway: **99.9%** (HTTP 2xx/304 ответы).
- Trustless TTFB P95: hot tier ≤ 120 ms, warm tier ≤ 300 ms.
- Успешность доказательств: ≥ 99.5% в день.
- Успех оркестратора (завершение chunks): ≥ 99%.

## Дашборды и алерты

1. **Наблюдаемость gateway** (`dashboards/grafana/sorafs_gateway_observability.json`) — отслеживает trustless-доступность, TTFB P95, разбиение отказов и сбои PoR/PoTR через метрики OTEL.
2. **Здоровье оркестратора** (`dashboards/grafana/sorafs_fetch_observability.json`) — покрывает нагрузку multi-source, ретраи, отказы провайдеров и всплески stalls.
3. **Метрики приватности SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — графики анонимизированных relay-buckets, окон подавления и здоровья collector через `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` и `soranet_privacy_poll_errors_total{provider}`.

Пакеты алертов:

- `dashboards/alerts/sorafs_gateway_rules.yml` — доступность gateway, TTFB, всплески отказов доказательств.
- `dashboards/alerts/sorafs_fetch_rules.yml` — отказы/ретраи/stalls оркестратора; валидируется через `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` и `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — всплески деградации приватности, alarms подавления, детекция idle-collector и алерты отключенного collector (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — alarms анонимного brownout, привязанные к `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — alarms дрейфа/ingest/CEK lag Taikai viewer плюс новые алерты penalty/cooldown здоровья доказательств SoraFS, основанные на `torii_sorafs_proof_health_*`.

## Стратегия трассировки

- Принять OpenTelemetry end-to-end:
  - Gateways эмитируют OTLP spans (HTTP) с аннотациями request IDs, manifest digests и token hashes.
  - Оркестратор использует `tracing` + `opentelemetry` для экспорта spans попыток fetch.
  - Встроенные SoraFS-узлы экспортируют spans для PoR-челленджей и storage-операций. Все компоненты делят общий trace ID, передаваемый через `x-sorafs-trace`.
- `SorafsFetchOtel` мостит метрики оркестратора в OTLP-гистограммы, а события `telemetry::sorafs.fetch.*` дают легкие JSON payloads для log-ориентированных backends.
- Collectors: запускайте OTEL collectors рядом с Prometheus/Loki/Tempo (Tempo предпочтителен). Экспортеры Jaeger-совместимые остаются опциональными.
- Операции с высокой кардинальностью следует сэмплировать (10% для успешных путей, 100% для отказов).

## Координация телеметрии TLS (SF-5b)

- Выравнивание метрик:
  - TLS automation публикует `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` и `sorafs_gateway_tls_ech_enabled`.
  - Включите эти gauges в dashboard Gateway Overview в панели TLS/Certificates.
- Связь алертов:
  - Когда срабатывают алерты истечения TLS (≤ 14 дней осталось), коррелируйте с trustless availability SLO.
  - Отключение ECH эмитирует вторичный алерт, ссылающийся и на TLS, и на availability панели.
- Pipeline: TLS automation job экспортирует в тот же Prometheus stack, что и метрики gateway; координация с SF-5b гарантирует дедупликацию инструментирования.

## Конвенции именования и меток

- Имена метрик следуют существующим префиксам `torii_sorafs_*` или `sorafs_*`, используемым Torii и gateway.
- Наборы меток стандартизированы:
  - `result` → HTTP outcome (`success`, `refused`, `failed`).
  - `reason` → код отказа/ошибки (`unsupported_chunker`, `timeout`, etc.).
  - `provider` → hex-encoded идентификатор провайдера.
  - `manifest` → канонический digest manifest (обрезается при высокой кардинальности).
  - `tier` → декларативные метки tier (`hot`, `warm`, `archive`).
- Точки эмиссии телеметрии:
  - Метрики gateway живут под `torii_sorafs_*` и переиспользуют конвенции из `crates/iroha_core/src/telemetry.rs`.
  - Оркестратор эмитирует метрики `sorafs_orchestrator_*` и события `telemetry::sorafs.fetch.*` (lifecycle, retry, provider failure, error, stall), помеченные digest manifest, job ID, region и идентификаторами провайдера.
  - Узлы публикуют `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` и `torii_sorafs_por_*`.
- Координируйтесь с Observability, чтобы зарегистрировать каталог метрик в общем документе по naming Prometheus, включая ожидания по кардинальности меток (верхние границы provider/manifests).

## Data Pipeline

- Collectors разворачиваются рядом с каждым компонентом, экспортируя OTLP в Prometheus (метрики) и Loki/Tempo (логи/трассы).
- Опциональный eBPF (Tetragon) обогащает низкоуровневую трассировку для gateways/узлов.
- Используйте `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` для Torii и встроенных узлов; оркестратор продолжает вызывать `install_sorafs_fetch_otlp_exporter`.

## Validation Hooks

- Запускайте `scripts/telemetry/test_sorafs_fetch_alerts.sh` в CI, чтобы правила алертов Prometheus оставались синхронными с метриками stalls и проверками suppression приватности.
- Держите дашборды Grafana под контролем версий (`dashboards/grafana/`) и обновляйте скриншоты/ссылки при изменениях панелей.
- Chaos drills логируют результаты через `scripts/telemetry/log_sorafs_drill.sh`; валидация использует `scripts/telemetry/validate_drill_log.sh` (см. [Playbook операций](operations-playbook.md)).
