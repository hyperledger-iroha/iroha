---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: план наблюдения
заголовок: Plano de observabilidade e SLO da SoraFS
Sidebar_label: Наблюдение и SLO
описание: Схема телеметрии, информационные панели и бюджетная политика ошибок для шлюзов SoraFS, но и для нескольких источников.
---

:::примечание Fonte canonica
Эта страница написана на плане мантидо в `docs/source/sorafs_observability_plan.md`. Мантенья представился как копиас синхронизадас.
:::

## Объективос
- Определите метрики и события, разработанные для шлюзов, а также для организаторов из нескольких источников.
- Приборные панели Fornecer Grafana, границы оповещений и крючки проверки.
- Estabelecer objetivos SLO junto com политика по бюджетным ошибкам и учения по хаосу.

## Каталог метрик

### Суперфики делают шлюз

| Метрика | Типо | Этикетки | Заметки |
|---------|------|--------|-------|
| `sorafs_gateway_active` | Датчик (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Эмитидо через `SorafsGatewayOtel`; работающие HTTP-серверы используют комбинацию конечной точки/метода. |
| `sorafs_gateway_responses_total` | Счетчик | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Cada solicitacao completada do шлюз приращения ума все; `result` в {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Гистограмма | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Задержка времени до первого байта для ответа на шлюз; экспортируется как Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Счетчик | `profile_version`, `result`, `error_code` | Результаты проверки подтверждений захвачены в момент запроса (`result` в {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Гистограмма | `profile_version`, `result`, `error_code` | Распространение латентной проверки для получения PoR. |
| `telemetry::sorafs.gateway.request` | Эструтурадо | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Журнал был создан для завершения каждого запроса для корреляции с Локи/Темп. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Счетчик | Альтернативные ярлыки | Метрики Prometheus mantidas для исторических информационных панелей; Emitidas junto в новой серии OTLP. |

События `telemetry::sorafs.gateway.request` были отправлены OTEL с установленными полезными нагрузками, пояснения `endpoint`, `method`, `variant`, `status`, `error_code` e `duration_ms` для корреляции с Loki/Tempo, а также панели мониторинга, содержащие серию OTLP для сопровождения SLO.

### Телеметрия де-сауде-дас-провас| Метрика | Типо | Этикетки | Заметки |
|---------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Счетчик | `provider_id`, `trigger`, `penalty` | Приращение всего того, что `RecordCapacityTelemetry` испускает `SorafsProofHealthAlert`. `trigger` различает PDP/PoTR/Both, а `penalty` захватывает или обеспечивает реальное кортадо или превосходит время восстановления. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Калибр | `provider_id` | Последние события PDP/PoTR связаны с информационными данными телеметрии, которые позволяют количественно или количественно проводить ультрапассары в политике. |
| `torii_sorafs_proof_health_penalty_nano` | Калибр | `provider_id` | Valor Nano-XOR кортадо не последнее предупреждение (ноль при любом времени восстановления или при использовании приложения). |
| `torii_sorafs_proof_health_cooldown` | Калибр | `provider_id` | Логический датчик (`1` = предупреждение о повышении времени восстановления) для запуска, когда оповещения о сопровождении прекращаются временно. |
| `torii_sorafs_proof_health_window_end_epoch` | Калибр | `provider_id` | Эпоха регистрации для подключения телеметрии к оповещению о том, что операторы коррелируют с артефактами Norito. |

Esses кормит агору пищей, а затем продаёт приборную панель Taikai Viewer
(`dashboards/grafana/taikai_viewer.json`), а также работу CDN, видимую в реальном темпе
большие объемы оповещений, сочетание триггеров PDP/PoTR, штрафы и время восстановления для
проведор.

Как mesmas metricas agora sustentam duas regras de alerta do Taikai Viewer:
`SorafsProofHealthPenalty`, когда это произошло
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` украшение
nos ultimos 15 минут, enquanto `SorafsProofHealthCooldown` emite um aviso se um
Проверяйте постоянство времени восстановления в течение пяти минут. Ambos os alertas vivem em
`dashboards/alerts/taikai_viewer_rules.yml` для немедленного получения контекста SRE
Когда приложение PoR/PoTR будет усилено.

### Superficies do orquestrador| Метрика / Эвенто | Типо | Этикетки | Продюсер | Заметки |
|------------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Калибр | `manifest_id`, `region` | `FetchMetricsCtx` | В конце концов, мы с вами. |
| `sorafs_orchestrator_fetch_duration_ms` | Гистограмма | `manifest_id`, `region` | `FetchMetricsCtx` | Гистограмма продолжительности жизни в милиссегундос; сегменты по 1 мс по 30 с. |
| `sorafs_orchestrator_fetch_failures_total` | Счетчик | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Разо: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Выделите причины повторной попытки (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Captura desabilitacao и contagens de falhas no level de sessao. |
| `sorafs_orchestrator_chunk_latency_ms` | Гистограмма | И18НИ00000135Х, И18НИ00000136Х | `FetchMetricsCtx` | Распределение задержки выборки порции (мс) для анализа пропускной способности/SLO. |
| `sorafs_orchestrator_bytes_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Байты, необходимые для манифеста/поставщика; получить пропускную способность через `rate()` в PromQL. |
| `sorafs_orchestrator_stalls_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Содержит фрагменты, превосходящие `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Эструтурадо | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Выполните или циклически выполните задание (начало/завершение) с полезной нагрузкой JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Эструтурадо | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Выпуск из-за серии повторных попыток; `attempts` содержит дополнительные попытки (>= 1). |
| `telemetry::sorafs.fetch.provider_failure` | Эструтурадо | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Publicado quando um Provideor Cruza o Limite de Falhas. |
| `telemetry::sorafs.fetch.error` | Эструтурадо | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Регистрационный терминал, используемый для приема Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Эструтурадо | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Выпустите, когда задержка ультра-прохода фрагмента или ограничение конфигурации (контадорес де-стойк). |

### Поверхности/реплики| Метрика | Типо | Этикетки | Заметки |
|---------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Гистограмма | `provider_id` | Гистограмма OTEL с использованием хранилища (экспортируется как `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Счетчик | `provider_id` | Contador monotono de amostras PoR bem-sucedidas, производные моментальных снимков для планировщика. |
| `sorafs_node_por_failure_total` | Счетчик | `provider_id` | Contador monotono de amostras PoR com falha. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Калибр | `provider` | Датчики Prometheus существуют для используемых байтов, глубины файла, заражения PoR em voo. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Калибр | `provider` | Даты производительности/время безотказной работы могут быть успешно проверены или отсутствуют на панели управления производительностью. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Калибр | `provider`, `manifest` | Глубокие накопления накопившихся задолженностей по экспорту постоянно происходят с `/v2/sorafs/por/ingestion/{manifest}` и консультируются, продовольствием или болью/предупреждением о «остановке PoR». |

### Доказательство своевременного извлечения (PoTR) и SLA для фрагментов

| Метрика | Типо | Этикетки | Продюсер | Заметки |
|---------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Гистограмма | `tier`, `provider` | Координатор PoTR | Folga do period em milissegundos (positivo = atendido). |
| `sorafs_potr_failures_total` | Счетчик | `tier`, `provider`, `reason` | Координатор PoTR | Разо: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Счетчик | `provider`, `manifest_id`, `reason` | Монитор SLA | Disparado quando a entrega de chunks falha no SLO (latencia, таксоны успеха). |
| `sorafs_chunk_sla_violation_active` | Калибр | `provider`, `manifest_id` | Монитор SLA | Логическое значение (0/1) изменяется во время активной активации. |

## Объективос СЛО

- Отключение доверенного шлюза: **99,9%** (ответы HTTP 2xx/304).
- TTFB P95 без доверия: горячий уровень = 99,5% в диаметре.
- Успешный оркестратор (вывод фрагментов): >= 99%.

## Панели мониторинга и оповещения

1. **Наблюдение за шлюзом** (`dashboards/grafana/sorafs_gateway_observability.json`) — компания, не требующая доверия, TTFB P95, подробные сведения о запросах и подтверждениях PoR/PoTR через метрики OTEL.
2. **Saude do orquestrador** (`dashboards/grafana/sorafs_fetch_observability.json`) — позволяет загружать данные из нескольких источников, повторять попытки, выполнять проверки и останавливаться.
3. **Метрики конфиденциальности SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — графики анонимных реле, подавления и сбора данных через `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` и `soranet_privacy_poll_errors_total{provider}`.

Пакеты предупреждений:- `dashboards/alerts/sorafs_gateway_rules.yml` - доступ к шлюзу, TTFB, изображениям проходов.
- `dashboards/alerts/sorafs_fetch_rules.yml` - ошибки/повторные попытки/остановки выполнения заказа; действительно через `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` и `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — изображения нарушения конфиденциальности, подавления сигналов тревоги, обнаружения аварийного коллектора и предупреждений о выходе из строя коллектора (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — сигналы тревоги отключения анонимных соединений на `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — сигналы дрейфа/поглощения/задержки CEK для просмотра Taikai больше, чем новые оповещения о штрафах/перезарядке, которые наблюдаются при испытаниях SoraFS, поступающих от `torii_sorafs_proof_health_*`.

## Стратегия отслеживания

- Обожаю OpenTelemetry de ponta a ponta:
  - Эмиссия шлюзов включает в себя OTLP (HTTP) аннотации, включая идентификаторы запроса, дайджесты манифестов и хэши токенов.
  - Оркестратор из США `tracing` + `opentelemetry` для экспорта интервалов предварительной выборки.
  - Экспортированные номера SoraFS охватывают нужные PoR и операции хранения. Все компоненты объединяются с идентификатором трассировки, который распространяется через `x-sorafs-trace`.
- `SorafsFetchOtel` объединяет метрики для организации гистограмм OTLP и событий `telemetry::sorafs.fetch.*` для обработки полезных данных JSON для серверных центров в центрах журналов.
- Коллекционеры: выполните сборщиков OTEL на уровне Prometheus/Loki/Tempo (предпочтительный темп). Совместимые экспортеры с постоянными опциями Jaeger.
- Высшие кардинальные операции должны быть успешными (10 % для успешных побед, 100 % для неудачных).

## Координатор телеметрии TLS (SF-5b)

- Дополнительные метрики:
  - Автоматический TLS через `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` и `sorafs_gateway_tls_ech_enabled`.
  - Включает датчики без приборной панели. Обзор шлюза, включая TLS/сертификаты.
- Винкуло де Алертас:
  - Когда оповещения об истечении срока действия TLS не совпадают (<= 14 до оставшихся), корреляция с или SLO без доверия.
  - Недостаточное сообщение ECH о вторичном ссылочном сигнале, связанном с болью в TLS, когда он отключен.
- Конвейер: автоматическое задание экспорта TLS в стек Prometheus для метрик шлюза; Координация с SF-5b гарантирует дедупликацию инструментов.

## Уведомления об именах и метках метрик- Имена метрик следуют за существующими префиксами `torii_sorafs_*` или `sorafs_*`, используемыми для Torii и шлюза.
- Связи с этикетками sao Padronizados:
  - `result` -> результат HTTP (`success`, `refused`, `failed`).
  - `reason` -> код возврата/ошибки (`unsupported_chunker`, `timeout` и т. д.).
  - `provider` -> идентификатор проверки или кодированный в шестнадцатеричном формате.
  - `manifest` -> дайджест канонического манифеста (cortado quando a cardinalidade e alta).
  - `tier` -> метки уровня (`hot`, `warm`, `archive`).
- Понтоны излучения телеметрии:
  - Метрики шлюза в реальном времени содержат `torii_sorafs_*` и повторно используются устройства `crates/iroha_core/src/telemetry.rs`.
  - Организатор выдает метрики `sorafs_orchestrator_*` и события `telemetry::sorafs.fetch.*` (жизненный цикл, повторная попытка, сбой поставщика, ошибка, остановка) этикетки с дайджестом манифеста, идентификатором задания, регионом и идентификаторами проверки.
  - Номера примеров `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` и `torii_sorafs_por_*`.
- Координация с помощью Observability для регистратора или каталога показателей без сопоставления документов с номерами Prometheus, включая ожидаемые кардинальные метки (ограничения вышестоящих доказательств/манифестов).

## Пайплайн данных

- Коллекторы, имплантированные вместе с каждым компонентом, экспортированные OTLP для Prometheus (метрики) и Loki/Tempo (логи/трассировки).
- Дополнительный eBPF (Tetragon) позволяет отслеживать нижние уровни шлюзов/номеров.
- Используйте `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` для Torii и наших вставок; o Оркестратор продолжения Chamando `install_sorafs_fetch_otlp_exporter`.

## Крючки валидации

- Выполните `scripts/telemetry/test_sorafs_fetch_alerts.sh` во время CI, чтобы гарантировать, что предупреждение Prometheus будет постоянным в синхронизации с показателями остановки и проверками подавления конфиденциальности.
- Панели мониторинга Mantenha Grafana для управления версией (`dashboards/grafana/`) и актуализации снимков экрана/ссылок, когда вы их видите.
- Результаты проверки причин регистрации через `scripts/telemetry/log_sorafs_drill.sh`; действительный США `scripts/telemetry/validate_drill_log.sh` (veja o [Playbook de Operacoes](operations-playbook.md)).