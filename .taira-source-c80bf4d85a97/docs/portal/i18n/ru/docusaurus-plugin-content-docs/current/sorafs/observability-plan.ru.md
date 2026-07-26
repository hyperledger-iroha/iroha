---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: план наблюдения
title: План наблюдения и SLO для SoraFS
Sidebar_label: Наблюдаемость и SLO
описание: Схема телеметрии, дашборды и политики бюджетных ошибок для шлюзов SoraFS, узлов и мульти-источникового оркестратора.
---

:::note Канонический источник
На этой странице отражен план, пострадавший в `docs/source/sorafs_observability_plan.md`. Держите копии синхронизированными, пока старый набор Sphinx полностью не мигрирован.
:::

## Цели
- Определить метрики и структурные события для шлюзов, узлов и мультиисточникового оркестратора.
- Предоставить дашборды Grafana, пороги алертов и хуки проверки.
- Зафиксировать цели SLO вместе с политикой, бюджетными ошибками и хаос-дриллами.

## Каталог метрик

### Поверхности шлюза

| Метрика | Тип | Метки | Примечания |
|---------|-----|-------|------------|
| `sorafs_gateway_active` | Датчик (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Эмитируется через `SorafsGatewayOtel`; отслеживает HTTP-операции в полете по определенной конечной точке/методу. |
| `sorafs_gateway_responses_total` | Счетчик | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | каждая завершенная шлюз-запрос операции увеличивает счетчик на один раз; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Гистограмма | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Латентность времени до первого байта для шлюза ответов; экспортируется как Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Счетчик | `profile_version`, `result`, `error_code` | Результаты проверки доказательств фиксируются в момент запроса (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Гистограмма | `profile_version`, `result`, `error_code` | Распределение латентности проверок для PoR-реплика. |
| `telemetry::sorafs.gateway.request` | Структурированное событие | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Структурированный журнал, эмитируемый при завершении каждого запроса для проверки Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Счетчик | Унаследованные наборы методов | Метрики Prometheus, сохраненные для исторических дашбордов; эмитируются вместе с новой серией OTLP. |

События `telemetry::sorafs.gateway.request` отражают OTEL-счетчики со структурированными полезными нагрузками, цикл `endpoint`, `method`, `variant`, `status`, `error_code` и `duration_ms` для в Loki/Tempo, тогда как дашборды используют серию OTLP для идентификации SLO.

### Телеметрия доказательство здоровья| Метрика | Тип | Метки | Примечания |
|---------|-----|-------|------------|
| `torii_sorafs_proof_health_alerts_total` | Счетчик | `provider_id`, `trigger`, `penalty` | Инкремент делается каждый раз, когда `RecordCapacityTelemetry` эмитирует `SorafsProofHealthAlert`. `trigger` проявляется вместе с PDP/PoTR/Both, а `penalty` показывает, был ли коллатерал реально написан или подавлен кулдаун. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Калибр | `provider_id` | Последние значения PDP/PoTR в проблемном окне телеметрии, чтобы команда могла оценить, сегодня провайдеры превысили политику. |
| `torii_sorafs_proof_health_penalty_nano` | Калибр | `provider_id` | Сумма Nano-XOR, написанная в последнем оповещении (ноль, если время восстановления подавил приложение). |
| `torii_sorafs_proof_health_cooldown` | Калибр | `provider_id` | Булевый датчик (`1` = алерт задержки перезарядки), чтобы показать, когда сигналы алерты временно приглушены. |
| `torii_sorafs_proof_health_window_end_epoch` | Калибр | `provider_id` | Эпоха оконной телеметрии, связанной с оповещением, чтобы операторы могли коррелировать с документами Norito. |

Эти фиды теперь питают текст доказательства здоровья в дашборде просмотра Taikai.
(`dashboards/grafana/taikai_viewer.json`), вспомогательным операторам CDN живую видимость
объемы оповещений, микса триггеров PDP/PoTR, штрафы и время восстановления состояния
провайдерам.

Эти же метрики теперь применяются к двум правилам оповещения Taikai Viewer:
`SorafsProofHealthPenalty` реализуется, когда
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` растет
за последние 15 минут `SorafsProofHealthCooldown` поднимает предупреждение, если
провайдер остается в режиме восстановления пять минут. Оба оповещения проживают в
`dashboards/alerts/taikai_viewer_rules.yml`, чтобы SRE применить контекст
сразу при эскалации принуждения PoR/PoTR.

### Поверхности оркестратора| Метрика / Событие | Тип | Метки | Производитель | Примечания |
|-------------------|-----|-------|---------------|------------|
| `sorafs_orchestrator_active_fetches` | Калибр | `manifest_id`, `region` | `FetchMetricsCtx` | Сессии, сокровища в полете. |
| `sorafs_orchestrator_fetch_duration_ms` | Гистограмма | `manifest_id`, `region` | `FetchMetricsCtx` | Гистограмма длительности в миллисекундах; сегменты от 1 мс до 30 с. |
| `sorafs_orchestrator_fetch_failures_total` | Счетчик | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Причины: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | определяет причину ретраев (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Фиксирует отключения и счетчик отказов на уровне сессий. |
| `sorafs_orchestrator_chunk_latency_ms` | Гистограмма | И18НИ00000135Х, И18НИ00000136Х | `FetchMetricsCtx` | Распределение латентности выборки по чанкам (мс) для анализа пропускной способности/SLO. |
| `sorafs_orchestrator_bytes_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Байты, представленные манифесту/поставщику; пропускная способность увеличивается через `rate()` в PromQL. |
| `sorafs_orchestrator_stalls_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Считает куски, превосходящие `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Структурированное событие | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Отражает жизненный цикл задания (начало/завершение) с полезной нагрузкой JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Структурированное событие | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Эмитируется на каждую серию ретраев провайдера; `attempts` считает инкрементальные ретроспективы (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Структурированное событие | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Публикуется, когда провайдер пересекает порог отказов. |
| `telemetry::sorafs.fetch.error` | Структурированное событие | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Финальная запись об отказе, которую удобно проглотить в Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Структурированное событие | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Выдается, когда латентность chunk увеличивает настроенный лимит (отражает ларек-счетчики). |

### Поверхности узлов / репликации| Метрика | Тип | Метки | Примечания |
|---------|-----|-------|------------|
| `sorafs_node_capacity_utilisation_pct` | Гистограмма | `provider_id` | OTEL-гистограмма процентного использования хранилища (экспортируется как `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Счетчик | `provider_id` | Монотонный счетчик результатов PoR-выборок, полученных из планировщика снимков. |
| `sorafs_node_por_failure_total` | Счетчик | `provider_id` | Монотонный счетчик неуспешных PoR-выборок. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Калибр | `provider` | Существующие датчик Prometheus для использованных байтов, внешняя очередь, бортовые счетчики PoR. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Калибр | `provider` | Данные об успехах провайдера по емкости/время безотказной работы отображаются на приборной панели емкости. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Калибр | `provider`, `manifest` | Глубина отставания плюс накопленные счетчики ошибок, доступные для каждого опроса `/v1/sorafs/por/ingestion/{manifest}`, питают панель/предупреждение «PoR Stalles». |

### Доказательство своевременного извлечения (PoTR) и SLA по чанкам

| Метрика | Тип | Метки | Производитель | Примечания |
|---------|-----|-------|---------------|------------|
| `sorafs_potr_deadline_ms` | Гистограмма | `tier`, `provider` | координатор ПОТР | Запас по дедлайну в миллисекундах (положительный = выполнено). |
| `sorafs_potr_failures_total` | Счетчик | `tier`, `provider`, `reason` | координатор ПОТР | Причины: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Счетчик | `provider`, `manifest_id`, `reason` | SLA-монитор | С осуществлением, когда доставка кусков не укладывается в SLO (латентность, успешность). |
| `sorafs_chunk_sla_violation_active` | Калибр | `provider`, `manifest_id` | SLA-монитор | Булевый датчик (0/1), переключаемый во время активного нарушения окна. |

## Цели SLO

- Шлюз бездоверительной доступности: **99,9%** (ответы HTTP 2xx/304).
- Trustless TTFB P95: горячий уровень ≤ 120 мс, теплый уровень ≤ 300 мс.
- Успешность доказательства: ≥ 99,5% в день.
- Успех оркестратора (завершение чанков): ≥ 99%.

## Дашборды и оповещения

1. **Наблюдаемость шлюза** (`dashboards/grafana/sorafs_gateway_observability.json`) — отслеживает Trustless-доступность, TTFB P95, разбиение отказов и сбои PoR/PoTR через метрики OTEL.
2. **Здоровье оркестратора** (`dashboards/grafana/sorafs_fetch_observability.json`) — защита от нескольких источников, отказов от провайдеров и всплески киосков.
3. **Метрики приватности SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — графики анонимизированных реле-ведёрок, окон подавления и коллектора здоровья через `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` и `soranet_privacy_poll_errors_total{provider}`.

Пакеты оповещений:- `dashboards/alerts/sorafs_gateway_rules.yml` — шлюз доступности, TTFB, доказательства отказов всплески.
- `dashboards/alerts/sorafs_fetch_rules.yml` — отказы/ретраи/остановки оркестратора; валидируется через `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` и `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — всплески деградации приватности, срабатывания сигнализации, обнаружения холостого коллектора и оповещения отключенного коллектора (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — аварийное отключение анонимного сигнала, соответствующие требованиям `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — сигналы тревоги дрейфа/глотания/CEK запаздывают Taikai Viewer плюс новые оповещения штраф/перезарядка, доказательства здоровья SoraFS, основанные на `torii_sorafs_proof_health_*`.

## Стратегия трассировки

- Принять OpenTelemetry сквозное:
  — Шлюзы эмитируют OTLP-промежутки (HTTP) с аннотациями идентификаторов запросов, дайджестов манифестов и хэшей токенов.
  - Оркестратор использует `tracing` + `opentelemetry` для экспорта интервалов выборки.
  - Встроенные SoraFS-узлы экспортируют промежутки для PoR-челленджей и Storage-операций. Все компоненты делят общий идентификатор трассировки, проверенный через `x-sorafs-trace`.
- `SorafsFetchOtel` переносит метрики оркестратора в OTLP-гистограммы, а события `telemetry::sorafs.fetch.*` дают легкие полезные данные JSON для лог-ориентированных бэкэндов.
- Коллекторы: запускайте коллекторы OTEL рядом с Prometheus/Loki/Tempo (Tempo предпочтителен). Экспортеры Jaeger-совместимые альтернативными опционами.
- Операции с высокой кардинальностью следует сэмплировать (10% для успешных способов, 100% для отказов).

## Координация телеметрии TLS (SF-5b)

- Выравнивание метрики:
  - Автоматизация TLS публикует `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` и `sorafs_gateway_tls_ech_enabled`.
  - Включите эти датчики на панели мониторинга «Обзор шлюза» на панели TLS/Certificates.
- Связь оповещений:
  - Когда сработают алерты истечения TLS (осталось ≤ 14 дней), коррелируйте с ненадежной доступностью SLO.
  - Отключение ECH эмитирует вторичный оповещение, ссылающийся и на TLS, и на панель доступности.
- Конвейер: задание автоматизации TLS экспортирует в тот же стек Prometheus, что и шлюз метрик; координация с SF-5b приводит к дедупликации инструментов.

## характеристики имен и меток- Имена метрик, следующих за существующими префиксами `torii_sorafs_*` или `sorafs_*`, используемым Torii и шлюзом.
- Наборы стандартных меток:
  - `result` → Результат HTTP (`success`, `refused`, `failed`).
  - `reason` → код отказа/ошибки (`unsupported_chunker`, `timeout` и т. д.).
  - `provider` → идентификатор провайдера в шестнадцатеричном коде.
  - `manifest` → манифест канонического дайджеста (обрезается при высокой кардинальности).
  - `tier` → декларативные метки уровня (`hot`, `warm`, `archive`).
- Точки касания телеметрии:
  - Шлюз Метрики живет под `torii_sorafs_*` и использует сеть из `crates/iroha_core/src/telemetry.rs`.
  - Оркестратор эмитирует метрики `sorafs_orchestrator_*` и события `telemetry::sorafs.fetch.*` (жизненный цикл, повтор, сбой поставщика, ошибка, остановка), помеченные дайджест-манифест, идентификатор задания, регион и идентификаторы провайдера.
  - Узлы публикуют `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` и `torii_sorafs_por_*`.
- Координируйтесь с Observability, чтобы зарегистрировать каталог метрик в общем документе по именованию Prometheus, включая ожидания по кардинальности меток (поставщик/манифесты верхних границ).

## Конвейер данных

- Коллекторы разворачиваются рядом с каждым компонентом, экспортируя OTLP в Prometheus (метрики) и Loki/Tempo (логи/трассы).
- Опциональный eBPF (Tetragon) обогащает низкоуровневую трассировку для шлюзов/узлов.
- Используйте `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` для Torii и встроенных узлов; оркестратор продолжения отделения `install_sorafs_fetch_otlp_exporter`.

## Хуки проверки

- Запускайте `scripts/telemetry/test_sorafs_fetch_alerts.sh` в CI, чтобы правила оповещений Prometheus оставались синхронными с метриками киосков и проверками подавления приватности.
- Держите дашборды Grafana под контролем контроля (`dashboards/grafana/`) и обновляйте скриншоты/ссылки при изменениях панелей.
- Хаос-тренировки регистрируют результаты через `scripts/telemetry/log_sorafs_drill.sh`; валидация использует `scripts/telemetry/validate_drill_log.sh` (см. [Playbook Operations](operations-playbook.md)).