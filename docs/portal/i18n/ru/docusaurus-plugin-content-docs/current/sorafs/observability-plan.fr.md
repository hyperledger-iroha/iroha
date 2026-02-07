---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: план наблюдения
заголовок: План наблюдения и SLO де SoraFS
Sidebar_label: Наблюдаемость и SLO
описание: Схема телеметрии, информационные панели и политика бюджетных ошибок для шлюзов SoraFS, узлы и оркестраторы из нескольких источников.
---

:::note Источник канонический
На этой странице отражен план обслуживания в `docs/source/sorafs_observability_plan.md`. Gardez les deux копирует синхронизированные копии полной миграции из древнего ансамбля Sphinx.
:::

## Цели
- Определите метрики и структуры для шлюзов, узлов и оркестраторов из нескольких источников.
- Четыре панели мониторинга Grafana, сигналы тревоги и средства проверки.
- Таблица целей SLO с политическими бюджетными ошибками и упражнениями в хаосе.

## Каталог метрик

### Поверхности шлюза

| Метрика | Тип | Этикет | Заметки |
|---------|------|------------|-------|
| `sorafs_gateway_active` | Датчик (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Эмис через `SorafsGatewayOtel` ; подходит для операций HTTP и для комбинации конечной точки/метода. |
| `sorafs_gateway_responses_total` | Счетчик | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Запрос шлюза завершился с увеличением ; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Гистограмма | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Время задержки до первого байта для шлюза ответов; экспортируется в Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Счетчик | `profile_version`, `result`, `error_code` | Результаты проверки предварительных снимков в момент запроса (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Гистограмма | `profile_version`, `result`, `error_code` | Распространение задержки проверки для результатов PoR. |
| `telemetry::sorafs.gateway.request` | Структура событий | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Структура журнала соответствует требованиям для корреляции Локи/Темп. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Счетчик | Наследные игры по этикету | Метрики Prometheus сохраняются для исторических панелей мониторинга; выходит параллельно с новой серией OTLP. |

События `telemetry::sorafs.gateway.request` отражают компьютеры OTEL со структурами полезной нагрузки, взрывчатыми веществами `endpoint`, `method`, `variant`, `status`, `error_code` и `duration_ms` для корреляции Loki/Tempo, а также панели мониторинга, соответствующие серии OTLP для управления SLO.

### Телеметрия здоровья де преув| Метрика | Тип | Этикет | Заметки |
|---------|------|------------|-------|
| `torii_sorafs_proof_health_alerts_total` | Счетчик | `provider_id`, `trigger`, `penalty` | Увеличьте количество, которое `RecordCapacityTelemetry` соответствует `SorafsProofHealthAlert`. `trigger` различает процессоры PDP/PoTR/Both, а затем `penalty` захватывает их в качестве залога, восстанавливает ампутацию или подавляет время восстановления. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Калибр | `provider_id` | Последние считают отчеты НДП/PoTR в рамках телеметрии ошибочными для того, чтобы количественно оценить отход от политики среди четырех специалистов. |
| `torii_sorafs_proof_health_penalty_nano` | Калибр | `provider_id` | Montant Nano-XOR ампутирует нижнюю часть тела (нулевое время восстановления при каждом применении). |
| `torii_sorafs_proof_health_cooldown` | Калибр | `provider_id` | Логический индикатор (`1` = предупреждение о превышении времени восстановления) для подачи сигнала о том, что сигналы тревоги постоянно отключаются. |
| `torii_sorafs_proof_health_window_end_epoch` | Калибр | `provider_id` | Эпоха регистрации для доступа к телеметрии находится в состоянии оповещения о том, что операторы могут эффективно коррелировать с артефактами Norito. |

Ces flux alimentent désormais la ligneproof-health du Dashboard Taikai Viewer
(`dashboards/grafana/taikai_viewer.json`), предоставляет операторам CDN прямую видимость
объемы оповещений, сочетание триггеров PDP/PoTR, штрафы и время восстановления по номиналу
фурниссер.

Метрики, поддерживающие два режима оповещения Taikai Viewer:
`SorafsProofHealthPenalty`, если ты хочешь
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` дополнение
Через 15 минут после этого `SorafsProofHealthCooldown` получил предупреждение, если
Fournisseur reste en кулдаун 5 минут. Les deux alertes vivent dans
`dashboards/alerts/taikai_viewer_rules.yml` ждет, что SRE будут немедленно удалены из контекста
или приложение PoR/PoTR усиливается.

### Поверхности оркестратора| Метрическая / Уличная | Тип | Этикет | Производитель | Заметки |
|------|------|------------|------------|-------|
| `sorafs_orchestrator_active_fetches` | Калибр | `manifest_id`, `region` | `FetchMetricsCtx` | Актуализация сессий в томе. |
| `sorafs_orchestrator_fetch_duration_ms` | Гистограмма | `manifest_id`, `region` | `FetchMetricsCtx` | Гистограмма продолжительности в миллисекундах; сегменты от 1 мс до 30 с. |
| `sorafs_orchestrator_fetch_failures_total` | Счетчик | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Причины: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Различайте причины повторной попытки (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Захват сеанса дезактиваций и проверок на новом уровне. |
| `sorafs_orchestrator_chunk_latency_ms` | Гистограмма | И18НИ00000135Х, И18НИ00000136Х | `FetchMetricsCtx` | Распределение задержки выборки номинального фрагмента (мс) для анализа пропускной способности/SLO. |
| `sorafs_orchestrator_bytes_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Octets livrés par Manifest/fournisseur ; Уменьшите пропускную способность через `rate()` в PromQL. |
| `sorafs_orchestrator_stalls_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Соберите фрагменты, которые исчезли `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | Структура событий | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Отобразите цикл жизни задания (начало/завершение) с полезной нагрузкой JSON Norito. |
| `telemetry::sorafs.fetch.retry` | Структура событий | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Эмис пар полоса повторных попыток пар фурниссер; `attempts` учитывает количество дополнительных попыток (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Структура событий | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Publié lorsqu'un fournisseur franchit le seuil d'échecs. |
| `telemetry::sorafs.fetch.error` | Структура событий | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Регистрация терминала отключения, адаптация к использованию Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | Структура событий | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Эмис Лорск la latence chunk dépasse la limite configurée (reflète les compteurs de stall). |

### Поверхности nœud / репликация| Метрика | Тип | Этикет | Заметки |
|---------|------|------------|-------|
| `sorafs_node_capacity_utilisation_pct` | Гистограмма | `provider_id` | Гистограмма OTEL du pourcentage d'utilisation du stockage (экспортирована в формате `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Счетчик | `provider_id` | Compteur monotone des échantillons PoR réussis, dérivé des snapshots du Scheduler. |
| `sorafs_node_por_failure_total` | Счетчик | `provider_id` | Compteur monotone des échantillons PoR échoues. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Калибр | `provider` | Датчики Prometheus существуют для используемых октетов, профондера файла, вычислений PoR и том. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Калибр | `provider` | Данные о емкости/время безотказной работы отображаются на приборной панели емкости. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Калибр | `provider`, `manifest` | Profondeur du backlog плюс совокупные экспортированные проверки по частям опроса `/v1/sorafs/por/ingestion/{manifest}`, включая панель/предупреждение «PoR Stalles». |

### Предварительное восстановление временных ресурсов (PoTR) и SLA для фрагментов

| Метрика | Тип | Этикет | Производитель | Заметки |
|---------|------|------------|------------|-------|
| `sorafs_potr_deadline_ms` | Гистограмма | `tier`, `provider` | Координатор ПОТР | Предельный срок в миллисекундах (positif = уважение). |
| `sorafs_potr_failures_total` | Счетчик | `tier`, `provider`, `reason` | Координатор ПОТР | Причины: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Счетчик | `provider`, `manifest_id`, `reason` | Монитор SLA | Déclenché quand la livraison de chunksrate le SLO (задержка, taux de succès). |
| `sorafs_chunk_sla_violation_active` | Калибр | `provider`, `manifest_id` | Монитор SLA | Логический датчик (0/1) активен во время активного явления нарушения. |

## Цели SLO

- Возможность бездоверительного шлюза: **99,9%** (ответы HTTP 2xx/304).
- TTFB P95 без доверия: горячий уровень ≤ 120 мс, теплый уровень ≤ 300 мс.
- Taux desucces des preuves: ≥ 99,5% в течение дня.
- Успех оркестра (завершение фрагментов): ≥ 99%.

## Панели мониторинга и оповещения

1. **Наблюдаемый шлюз** (`dashboards/grafana/sorafs_gateway_observability.json`) — подходит для возможности доверенного доступа, TTFB P95, перераспределения отказов и проверок PoR/PoTR через счетчики OTEL.
2. **Santé de l'orchestrateur** (`dashboards/grafana/sorafs_fetch_observability.json`) — позволяет заряжать несколько источников, повторять попытки, выполнять четыре операции и работать с киосками.
3. **Метрики конфиденциальности SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — отслеживание сегментов анонимных реле, окон подавления и защиты коллекторов через `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` и `soranet_privacy_poll_errors_total{provider}`.

Пакеты предупреждений:- `dashboards/alerts/sorafs_gateway_rules.yml` — доступный шлюз, TTFB, изображения предварительной проверки.
- `dashboards/alerts/sorafs_fetch_rules.yml` — проверки/повторные попытки/остановки оркестра; действительно через `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` и `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — изображения нарушения конфиденциальности, сигналов тревоги о подавлении, обнаружения неактивного коллектора и предупреждений о неактивном сборщике (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — аварийные сигналы отключения анонимных кабелей на `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — сигналы дрейфа/поглощения/задержки CEK Taikai Viewer, а также новые предупреждения о штрафах/перезарядке санте-де-преувс SoraFS, питаемые по `torii_sorafs_proof_health_*`.

## Стратегия отслеживания

- Принятие OpenTelemetry в бою:
  - Шлюзы, эмитированные в рамках OTLP (HTTP), содержат аннотации с запрошенными идентификаторами, дайджесты манифестов и хэши токенов.
  - Оркестр использует `tracing` + `opentelemetry` для экспорта интервалов пробной выборки.
  - Les nœuds SoraFS помещает экспортируемые промежутки для выполнения défis PoR и операций со складом. Все компоненты участвуют в совместном распространении идентификатора трассировки через `x-sorafs-trace`.
- `SorafsFetchOtel` опирается на оркестраторы метрик и гистограмм OTLP, а также на события `telemetry::sorafs.fetch.*`, которые содержат полезные данные JSON для записей серверных центров.
- Коллекционеры: исполнитель коллекционеров OTEL по адресу Prometheus/Loki/Tempo (предпочтительный темп). Оставшиеся опции API Jaeger для экспорта.
- Высокие кардинальные операции по повышению эффективности (10 % для успешных операций, 100 % для проверок).

## Координация телеметрии TLS (SF-5b)

- Выравнивание метрик:
  - Публикация TLS `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` и `sorafs_gateway_tls_ech_enabled`.
  - Включите эти датчики на приборную панель Обзор шлюза с использованием TLS/сертификатов.
- Связь по оповещению:
  - Если оповещения об истечении срока действия TLS отключены (≤ 14 дней ожидания), они будут исправлены с помощью SLO, не требующего доверия.
  - Деактивация ECH вызывает вторичное предупреждение о том, что TLS отключен и не доступен.
- Конвейер: задание автоматизации TLS экспортируется в стек памяти Prometheus, который используется шлюзом метрик; Координация с SF-5b обеспечивает дедупликацию приборов.

## Соглашения о номенклатуре и этикете метрик- Метрические номера, соответствующие существующим префиксам `torii_sorafs_*` или `sorafs_*`, используются для Torii и шлюза.
- Стандартные ансамбли этикета:
  - `result` → результат HTTP (`success`, `refused`, `failed`).
  - `reason` → код отказа/ошибки (`unsupported_chunker`, `timeout` и т. д.).
  - `provider` → идентификатор в шестнадцатеричном формате.
  - `manifest` → дайджест канонического манифеста (tronqué quand la cardinalité est élevée).
  - `tier` → этикетки декларативного уровня (`hot`, `warm`, `archive`).
- Точки телеметрии:
  - Шлюз метрик живет под `torii_sorafs_*` и повторно использует соглашения `crates/iroha_core/src/telemetry.rs`.
  - Оркестровый учет метрик `sorafs_orchestrator_*` и событий `telemetry::sorafs.fetch.*` (жизненный цикл, повторная попытка, сбой поставщика, ошибка, остановка) этикеток с дайджестом манифеста, идентификатором задания, регионом и идентификаторами.
  - Открытые номера `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` и `torii_sorafs_por_*`.
- Coordonnez avec Observability для регистрации каталога метрик в документе с номером Prometheus, а также включает в себя внимание к кардинальным меткам (сверху четырехзначных/манифестов).

## Трубопровод Донне

- Сборщики данных развертываются по частям, экспортируя OTLP версий Prometheus (метрики) и Loki/Tempo (журналы/трассировки).
- Опция eBPF (Tetragon) расширяет возможности отслеживания базовых узлов для шлюзов/узлов.
- Используйте `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` для Torii и посадок; l'orchestrateur continue d'appeler `install_sorafs_fetch_otlp_exporter`.

## Хуки проверки

- Выполните `scripts/telemetry/test_sorafs_fetch_alerts.sh` в CI для гарантии соблюдения правил оповещения Prometheus, оставшихся в соответствии с показателями остановки и проверками подавления конфиденциальности.
- Проверьте панели мониторинга Grafana под управлением версии (`dashboards/grafana/`) и выполняйте захваты/ссылки при изменении панелей.
- «Упражнения хаоса» публикуют результаты через `scripts/telemetry/log_sorafs_drill.sh`; la validation s'appuie sur `scripts/telemetry/validate_drill_log.sh` (voir le [Playbook d'exploitation](operations-playbook.md)).