---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: план наблюдения
заголовок: План наблюдения и SLO де SoraFS
Sidebar_label: Наблюдение и SLO
описание: Программа телеметрии, информационные панели и политика предположений об ошибках для шлюзов SoraFS, узлы и многофункциональный оператор.
---

:::примечание Фуэнте каноника
На этой странице отображается план обслуживания в `docs/source/sorafs_observability_plan.md`. Многие синхронизировали несколько копий того, что комплект документации Sphinx был полностью перенесен.
:::

## Объективос
- Определите метрики и структуры событий для шлюзов, узлов и многофункционального оркеста.
- Проверьте панели мониторинга Grafana, зоны оповещения и крючки проверки.
- Создание объединения SLO с политическими предпосылками ошибок и учениями по хаосу.

## Каталог метрик

### Особенности шлюза

| Метрика | Типо | Этикет | Заметки |
|--------|------|-----------|-------|
| `sorafs_gateway_active` | Датчик (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Выпуск через `SorafsGatewayOtel`; выполнять операции HTTP в режиме комбинации конечной точки/метода. |
| `sorafs_gateway_responses_total` | Счетчик | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Cada запросил завершение увеличения шлюза; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Гистограмма | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Задержка времени до первого байта для ответа шлюза; экспортируется как Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Счетчик | `profile_version`, `result`, `error_code` | Результаты проверки заявок зафиксированы в момент запроса (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Гистограмма | `profile_version`, `result`, `error_code` | Распространение задержек верификации для получения PoR. |
| `telemetry::sorafs.gateway.request` | Эструктура | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Лог эструктурирован по всем запросам для корреляции в Локи/Темп. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Счетчик | Правила этикета наследников | Метрики Prometheus сохранены для исторических информационных панелей; Emitidas Junto с новой серией OTLP. |

События `telemetry::sorafs.gateway.request` отражают контакты OTEL с построенными полезными нагрузками, экспонирование `endpoint`, `method`, `variant`, `status`, `error_code` y `duration_ms` для корреляции в Loki/Tempo, в то время, когда информационные панели используются в серии OTLP для отслеживания SLO.

### Телеметрия спасения де Прюбас| Метрика | Типо | Этикет | Заметки |
|--------|------|-----------|-------|
| `torii_sorafs_proof_health_alerts_total` | Счетчик | `provider_id`, `trigger`, `penalty` | Если `RecordCapacityTelemetry` испускает `SorafsProofHealthAlert`, то увеличивается каждый раз. `trigger` различает PDP/PoTR/Ambos, в то время, когда `penalty` захватывает, если залоговая часть записывается в реальном времени или превосходит время восстановления. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Калибр | `provider_id` | Получено больше сообщений PDP/PoTR о ветре телеметрии, которые могут привести к тому, что оборудование будет превосходить политические доказательства. |
| `torii_sorafs_proof_health_penalty_nano` | Калибр | `provider_id` | Monto Nano-XOR записывает последнее предупреждение (когда время восстановления отображается в приложении). |
| `torii_sorafs_proof_health_cooldown` | Калибр | `provider_id` | Логический датчик (`1` = предупреждение о повышении времени восстановления) для запуска, когда оповещения о слежении остаются временно отключенными. |
| `torii_sorafs_proof_health_window_end_epoch` | Калибр | `provider_id` | Эпоха регистрации для получения сигнала телеметрии и оповещения о корреляции операторов с артефактами Norito. |

Estos кормит ahora alimentan la fila de salud de pruebas del приборная панель Taikai Viewer
(`dashboards/grafana/taikai_viewer.json`), затем работайте с CDN, видимым в реальном времени.
объемы оповещений, набор разногласий PDP/PoTR, штрафы и время восстановления для
провидор.

Неправильные показатели теперь будут изменены в соответствии с правилами просмотра Taikai:
`SorafsProofHealthPenalty` выглядит не так, как хотелось бы
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` увеличение ru
последние 15 минут, в течение которых `SorafsProofHealthCooldown` была рекламой, если она есть
обеспечивает постоянство и время восстановления в течение пяти минут. Ambas alertas viven ru
`dashboards/alerts/taikai_viewer_rules.yml` для немедленного получения SRE в контексте
Когда приложение PoR/PoTR становится более интенсивным.

### Особенности работы orquestador| Метрика / Эвенто | Типо | Этикет | Производитель | Заметки |
|-----------------|------|-----------|-----------|-------|
| `sorafs_orchestrator_active_fetches` | Калибр | `manifest_id`, `region` | `FetchMetricsCtx` | Текущие сеансы на улице. |
| `sorafs_orchestrator_fetch_duration_ms` | Гистограмма | `manifest_id`, `region` | `FetchMetricsCtx` | Гистограмма продолжительности жизни в миллисекундах; сегменты по 1 мс по 30 с. |
| `sorafs_orchestrator_fetch_failures_total` | Счетчик | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Разоны: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Различайте причины повторного намерения (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Captura deshabilitación y conteos de Fallos на уровне сессии. |
| `sorafs_orchestrator_chunk_latency_ms` | Гистограмма | И18НИ00000135Х, И18НИ00000136Х | `FetchMetricsCtx` | Распределение задержки выборки по фрагментам (мс) для анализа пропускной способности/SLO. |
| `sorafs_orchestrator_bytes_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Байты, переданные манифесту/поставщику; получить пропускную способность через `rate()` в PromQL. |
| `sorafs_orchestrator_stalls_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Куэнты, превосходящие `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Эструктура | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Отображение цикла просмотра задания (начало/завершение) с полезной нагрузкой JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Эструктура | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Emitido por racha de reintentos por provedor; `attempts` количество повторных инкрементных изменений (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Эструктура | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Se publica cuando unprovedor cruza el umbral de Fallos. |
| `telemetry::sorafs.fetch.error` | Эструктура | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Терминал регистратуры, удобный для приема внутрь в Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Эструктура | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Если вы выберете задержку фрагмента, превышающую лимит конфигурации (отображается сообщение о задержке). |

### Поверхности узла / репликация| Метрика | Типо | Этикет | Заметки |
|--------|------|-----------|-------|
| `sorafs_node_capacity_utilisation_pct` | Гистограмма | `provider_id` | Гистограмма OTEL по использованию холодильника (экспортируется как `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Счетчик | `provider_id` | Монотонный контроль выходов PoR, получение снимков планировщика. |
| `sorafs_node_por_failure_total` | Счетчик | `provider_id` | Contador monotónico de muestras PoR Fallidas. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Калибр | `provider` | Датчики Prometheus существуют для используемых байтов, глубокого хранения данных, PoR и vuelo. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Калибр | `provider` | Данные о емкости/время безотказной работы выходят из проверки или расходуются на приборной панели емкости. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Калибр | `provider`, `manifest` | Глубокий объем накопившихся задолженностей по экспорту каждый раз, когда вы проконсультируетесь с `/v2/sorafs/por/ingestion/{manifest}`, подадите на панель/предупреждение «PoR Stalles». |

### Возможности восстановления (PoTR) и соглашения об уровне обслуживания фрагментов

| Метрика | Типо | Этикет | Производитель | Заметки |
|--------|------|-----------|-----------|-------|
| `sorafs_potr_deadline_ms` | Гистограмма | `tier`, `provider` | Координатор ПОТР | Holgura del period en milisegundos (positivo = cumplido). |
| `sorafs_potr_failures_total` | Счетчик | `tier`, `provider`, `reason` | Координатор ПОТР | Разоны: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Счетчик | `provider`, `manifest_id`, `reason` | Монитор SLA | Если входящие фрагменты включают SLO (задержка, выходной сигнал), это не соответствует действительности. |
| `sorafs_chunk_sla_violation_active` | Калибр | `provider`, `manifest_id` | Монитор SLA | Логический датчик (0/1) чередуется во время активного вентилирования. |

## Объективос СЛО

- Отключение доверенного шлюза: **99,9%** (ответы HTTP 2xx/304).
- TTFB P95 без доверия: горячий уровень ≤ 120 мс, теплый уровень ≤ 300 мс.
- Выход из пруда: ≥ 99,5% в день.
- Исходный результат (завершение фрагментов): ≥ 99%.

## Панели мониторинга и оповещения

1. **Наблюдение за шлюзом** (`dashboards/grafana/sorafs_gateway_observability.json`) — доступ к доверенным соединениям, TTFB P95, удаление сообщений и падение PoR/PoTR через метрики OTEL.
2. **Salud del orquestador** (`dashboards/grafana/sorafs_fetch_observability.json`) — cubre carga multifuente, reintentos, Fallos de Providedores y Ráfagas de Stalles.
3. **Метрики конфиденциальности SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — графические сегменты анонимных реле, каналов подавления и вызова коллекторов через `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` и `soranet_privacy_poll_errors_total{provider}`.

Пакеты предупреждений:- `dashboards/alerts/sorafs_gateway_rules.yml` — доступ к шлюзу, TTFB, пиктограммам ошибок.
- `dashboards/alerts/sorafs_fetch_rules.yml` — Fallos/reintentos/stalls del orquestador; действительно через `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` и `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — изображения ухудшения конфиденциальности, аварийных сигналов подавления, обнаружения неактивности коллектора и оповещений о выходе коллектора из строя (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — сигналы тревоги при отключении анонимного сигнала при подключении к `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — сигналы тревоги по получению/поглощению/задержке CEK в просмотре Тайкай новых предупреждений о штрафах/перезарядке салютов SoraFS, импульсы от `torii_sorafs_proof_health_*`.

## Стратегия торговли

- Принять OpenTelemetry в крайнем случае:
  - Шлюзы излучают аннотации OTLP (HTTP) с запрашиваемыми идентификаторами, дайджестами манифеста и хэшами токенов.
  - Оркестр из США `tracing` + `opentelemetry` для экспорта в диапазон намерений получения.
  - Узлы SoraFS экспортируются для обеспечения безопасности и операций по резервному копированию. Все компоненты объединяются с идентификатором трассировки, который распространяется через `x-sorafs-trace`.
- `SorafsFetchOtel` подключает метрики ордера к гистограммам OTLP в течение всего времени, когда происходят события. `telemetry::sorafs.fetch.*` пропорциональны полезным нагрузкам JSON, доступным для центральных бэкэндов и журналов.
- Коллекционеры: коллекционеры ejecuta OTEL junto con Prometheus/Loki/Tempo (Tempo предпочтительно). Экспортируемые API Jaeger могут быть добавлены дополнительно.
- Las Operationaciones de Alta Cardinalidad deben muestrearse (10% для рутинных операций, 100% для аварий).

## Координация телеметрии TLS (SF-5b)

- Метрическая метрика:
  - Автоматизация TLS осуществляется через `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` и `sorafs_gateway_tls_ech_enabled`.
  - Включите эти датчики на приборную панель Обзор шлюза на панели TLS/сертификаты.
- Информация о оповещениях:
  - Когда возникают несоответствующие оповещения об истечении срока действия TLS (≤ 14 дней до остывания), корреляция с SLO-де-ненадежным.
  - Отключение ECH выдает временное предупреждение о том, что ссылки на панели TLS являются недоступными.
- Конвейер: задание автоматизации TLS экспортируется в стек Prometheus, который содержит метрики шлюза; координация с дедупликационным оборудованием SF-5b.

## Соглашения о номерах и этикетках метрик- Метричные номера соответствуют существующим префиксам `torii_sorafs_*` или `sorafs_*`, используемым для Torii и шлюзу.
- Правила этикета остались стандартными:
  - `result` → результат HTTP (`success`, `refused`, `failed`).
  - `reason` → код ошибки/ошибки (`unsupported_chunker`, `timeout` и т. д.).
  - `provider` → идентификатор подтверждения или кодированный в шестнадцатеричном формате.
  - `manifest` → дайджест канонического манифеста (recortado cuando hay alta cardinalidad).
  - `tier` → этикетки уровня (`hot`, `warm`, `archive`).
- Точки излучения телеметрии:
  - Метрики шлюза активизируются с помощью `torii_sorafs_*` и повторно используются символы `crates/iroha_core/src/telemetry.rs`.
  - Организатор выдает метрики `sorafs_orchestrator_*` и события `telemetry::sorafs.fetch.*` (жизненный цикл, повторная попытка, сбой поставщика, ошибка, остановка) этикетки с дайджестом манифеста, идентификатором задания, регионом и идентификаторами поставщика.
  - Лос-ноды показаны `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` и `torii_sorafs_por_*`.
- Координация с наблюдаемостью для регистратора метрического каталога в документе, содержащем номера Prometheus, включая ожидаемые кардинальные значения этикеток (пределы вышестоящих доказательств/манифестов).

## Конвейер данных

- Коллекторы объединяются в каждый компонент, экспортируя OTLP в Prometheus (метрики) и Loki/Tempo (логи/тразы).
- Дополнительный eBPF (Tetragon) позволяет использовать нижний уровень для шлюзов/узлов.
- США `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` для Torii и узлов вставки; Оркестр продолжает звонить по `install_sorafs_fetch_otlp_exporter`.

## Перехватчики проверки

- Вызовите `scripts/telemetry/test_sorafs_fetch_alerts.sh` во время CI, чтобы гарантировать, что правила оповещения Prometheus будут постоянными в синтонии с метриками киосков и проверками подавления конфиденциальности.
- Поднимите панели мониторинга Grafana для управления версиями (`dashboards/grafana/`) и актуализируйте захваты/ссылки, когда панели исчезнут.
- Результаты регистрации событий через `scripts/telemetry/log_sorafs_drill.sh`; la validación USA `scripts/telemetry/validate_drill_log.sh` (см. [Playbook deoperaciones](operations-playbook.md)).