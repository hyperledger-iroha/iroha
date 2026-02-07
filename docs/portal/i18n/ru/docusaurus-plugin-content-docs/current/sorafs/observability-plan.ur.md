---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: план наблюдения
title: SoraFS Наблюдаемость اور SLO پلان
Sidebar_label: Наблюдаемость для SLO
описание: шлюзы SoraFS, узлы, оркестратор с несколькими источниками, схема телеметрии, информационные панели и политика бюджета ошибок.
---

:::примечание
یہ صفحہ `docs/source/sorafs_observability_plan.md` میں برقرار رکھے گئے منصوبے کی عکاسی کرتا ہے۔ جب تک پرانا Sphinx سیٹ مکمل طور پر منتقل نہ ہو جائے دونوں نقول کو ہم آہنگ رکھیں۔
:::

## Цели
- шлюзы, узлы, оркестратор с несколькими источниками, метрики и структурированные события.
- Панели мониторинга Grafana, пороговые значения оповещений и перехватчики проверки.
- Бюджет ошибок и политика хаоса-тренировки Целевые показатели SLO قائم کریں۔

## Каталог метрик

### Поверхности шлюза

| Метрическая | Тип | Этикетки | Заметки |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Датчик (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel` کے ذریعے испускать ہوتا ہے؛ Конечная точка/метод Как выполнять HTTP-операции в реальном времени? |
| `sorafs_gateway_responses_total` | Счетчик | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Запрос на шлюз или увеличение приращения. `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Гистограмма | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | ответы шлюза: большая задержка до первого байта; Prometheus `_bucket/_sum/_count` для экспорта. |
| `sorafs_gateway_proof_verifications_total` | Счетчик | `profile_version`, `result`, `error_code` | время запроса и результаты проверки доказательства захватывают کیے جاتے ہیں (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Гистограмма | `profile_version`, `result`, `error_code` | PoR-квитанции и распределение задержки при проверке. |
| `telemetry::sorafs.gateway.request` | Структурированное мероприятие | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Завершение запроса и создание структурированного журнала. |

События `telemetry::sorafs.gateway.request`, счетчики OTEL, структурированные полезные нагрузки, зеркало, корреляция Loki/Tempo, `endpoint`, `method`, `variant`, `status`, `error_code` или `duration_ms` Дополнительные панели мониторинга SLO-отслеживание серии OTLP استعمال کرتے ہیں۔

### Телеметрия для проверки работоспособности| Метрическая | Тип | Этикетки | Заметки |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Счетчик | `provider_id`, `trigger`, `penalty` | Если `RecordCapacityTelemetry` и `SorafsProofHealthAlert` испускают и увеличивают ہوتا ہے۔ `trigger` PDP/PoTR/Оба сбоя نے подавлять کیا۔ |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Калибр | `provider_id` | Окно телеметрии, вызывающее нарушения تجاوز کیا۔ |
| `torii_sorafs_proof_health_penalty_nano` | Калибр | `provider_id` | آخری alert پر slash ہونے и Nano-XOR مقدار (время восстановления и принудительное подавление کیا تو صفر). |
| `torii_sorafs_proof_health_cooldown` | Калибр | `provider_id` | Логический датчик (`1` = время восстановления оповещения или подавление) Дополнительные оповещения могут быть отключены или отключены |
| `torii_sorafs_proof_health_window_end_epoch` | Калибр | `provider_id` | предупреждение سے منسلک окно телеметрии کا эпоха تاکہ операторы Norito артефакты سے корреляция کر سکیں۔ |

یہ каналы اب Панель просмотра Taikai Viewer کی строка проверки здоровья کو چلاتے ہیں
(`dashboards/grafana/taikai_viewer.json`), операторы CDN, объемы оповещений, сочетание триггеров PDP/PoTR, штрафы, состояние перезарядки и провайдер, видимость в реальном времени.

В программе Taikai Viewer есть метрики и правила оповещений, а также дополнительные функции:
`SorafsProofHealthPenalty` اس وقت fire ہوتا ہے جب
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` в наличии
15 минут назад `SorafsProofHealthCooldown` предупреждение دیتا ہے اگر
Провайдер имеет время перезарядки میں رہے۔ دونوں оповещения
`dashboards/alerts/taikai_viewer_rules.yml` может быть использован для SRE или PoR/PoTR.
правоприменение بڑھنے پر فوری context ملے۔

### Поверхности оркестратора| Метрика/Событие | Тип | Этикетки | Продюсер | Заметки |
|----------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Калибр | `manifest_id`, `region` | `FetchMetricsCtx` | Проведение сеансов в полете۔ |
| `sorafs_orchestrator_fetch_duration_ms` | Гистограмма | `manifest_id`, `region` | `FetchMetricsCtx` | гистограмма длительности (миллисекунды); От 1 мс до 30 с сегментов. |
| `sorafs_orchestrator_fetch_failures_total` | Счетчик | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Причины: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | повторная попытка вызывает ошибку (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | отключение на уровне сеанса / запись сбоев کرتا ہے۔ |
| `sorafs_orchestrator_chunk_latency_ms` | Гистограмма | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Распределение задержки выборки по чанкам (мс) анализ пропускной способности/SLO کے لیے۔ |
| `sorafs_orchestrator_bytes_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Манифест/поставщик указывает доставленные байты; PromQL — `rate()` — высокая пропускная способность |
| `sorafs_orchestrator_stalls_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` سے تجاوز کرنے и куски گنتا ہے۔ |
| `telemetry::sorafs.fetch.lifecycle` | Структурированное мероприятие | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Жизненный цикл задания (начало/завершение) Наличие Norito Полезная нагрузка JSON Наличие зеркала Наличие |
| `telemetry::sorafs.fetch.retry` | Структурированное мероприятие | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | полоса повторных попыток провайдера `attempts` добавочные повторные попытки могут быть ہے (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | Структурированное мероприятие | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Пересечение порога отказа провайдера کرے تو ظاہر ہوتا ہے۔ |
| `telemetry::sorafs.fetch.error` | Структурированное мероприятие | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | запись о сбое терминала, прием Loki/Splunk и многое другое. |
| `telemetry::sorafs.fetch.stall` | Структурированное мероприятие | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Настроенное ограничение задержки фрагмента سے بڑھنے پر испускает ہوتا ہے (счетчики остановки или зеркало کرتا ہے). |

### Узел/поверхности репликации| Метрическая | Тип | Этикетки | Заметки |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Гистограмма | `provider_id` | Процент использования хранилища и гистограмма OTEL (`_bucket/_sum/_count` — экспорт). |
| `sorafs_node_por_success_total` | Счетчик | `provider_id` | снимки планировщика, полученные успешные образцы PoR, монотонный счетчик. |
| `sorafs_node_por_failure_total` | Счетчик | `provider_id` | неудачные образцы PoR کا монотонный счетчик |
| И18НИ00000198Х, И18НИ00000199Х | Калибр | `provider` | использованные байты, глубина очереди и подсчеты потоков PoR, а также Prometheus датчики. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Калибр | `provider` | данные об успешности емкости/время безотказной работы провайдера. Панель мониторинга емкости. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Калибр | `provider`, `manifest` | глубина невыполненной работы اور совокупные счетчики ошибок جو ہر `/v1/sorafs/por/ingestion/{manifest}` опрос и экспорт ہوتے ہیں، Панель/предупреждение «PoR Stalls» کو کرتے ہیں۔ |

### Доказательство своевременного извлечения (PoTR) в соответствии с SLA фрагмента

| Метрическая | Тип | Этикетки | Продюсер | Заметки |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Гистограмма | `tier`, `provider` | координатор ПОТР | Срок истекает на миллисекунды میں (положительный = соблюден). |
| `sorafs_potr_failures_total` | Счетчик | `tier`, `provider`, `reason` | координатор ПОТР | Причины: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Счетчик | `provider`, `manifest_id`, `reason` | SLA-монитор | SLO при доставке фрагментов пропускает время или огонь ہوتا ہے (задержка, вероятность успеха). |
| `sorafs_chunk_sla_violation_active` | Калибр | И18НИ00000225Х, И18НИ00000226Х | SLA-монитор | Логический индикатор (0/1) в окне активного нарушения или переключении ہوتا ہے۔ |

## Цели SLO

- Доступность без доверия шлюза: **99,9%** (ответы HTTP 2xx/304).
- Trustless TTFB P95: горячий уровень ≤ 120 мс, теплый уровень ≤ 300 мс.
- Уровень успеха доказательства: ≥ 99,5% в день.
- Успех оркестратора (завершение фрагмента): ≥ 99%.

## Панели мониторинга и оповещения

1. **Наблюдение за шлюзом** (`dashboards/grafana/sorafs_gateway_observability.json`) — доступность без доверия, TTFB P95, отказ в отказе, сбои PoR/PoTR и метрики OTEL, которые можно использовать.
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`) — загрузка из нескольких источников, повторные попытки, сбои поставщика и всплески зависаний.
3. **Метрики конфиденциальности SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — анонимные ретрансляционные корзины, окна подавления и состояние коллектора, например `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` и `soranet_privacy_poll_errors_total{provider}`. چارٹ کرتا ہے۔

Пакеты оповещений:- `dashboards/alerts/sorafs_gateway_rules.yml` — доступность шлюза, TTFB, всплески сбоев.
- `dashboards/alerts/sorafs_fetch_rules.yml` — сбои/повторные попытки оркестратора/зависания; `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml`, или `dashboards/alerts/tests/soranet_policy_rules.test.yml` کے ذریعے validate۔
- `dashboards/alerts/soranet_privacy_rules.yml` — скачки понижения конфиденциальности, сигналы подавления, обнаружение простоя коллектора и оповещения об отключении коллектора (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — сигнализация отключения анонимности и `sorafs_orchestrator_brownouts_total` в проводном режиме.
- `dashboards/alerts/taikai_viewer_rules.yml` — оповещения о дрейфе/поглощении/задержке CEK в программе Taikai Viewer SoraFS, оповещения о штрафах за работоспособность/перезарядке `torii_sorafs_proof_health_*`

## Стратегия отслеживания

- Комплексные возможности OpenTelemetry:
  - Промежутки OTLP шлюзов (HTTP) выдают идентификаторы запросов, дайджесты манифестов и хэши токенов.
  - оркестратор `tracing` + `opentelemetry` позволяет получить попытки выборки и охватывает экспорт.
  - Встроенные узлы SoraFS PoR бросают вызов операциям хранения данных, включая экспорт. Компоненты `x-sorafs-trace` могут использоваться для распространения и совместного использования общего идентификатора трассировки.
- `SorafsFetchOtel` метрики оркестратора, гистограммы OTLP и мосты, а также `telemetry::sorafs.fetch.*` серверная часть событий, ориентированная на журнал, а также облегченные полезные нагрузки JSON. کرتے ہیں۔
- Коллекционеры: коллекционеры OTEL کو Prometheus/Loki/Tempo или Tempo (предпочтителен темп). Экспортеры API Jaeger اختیاری رہتے ہیں۔
- Операции с высокой мощностью - выборка (пути успеха - 10%, неудачи - 100%).

## Координация телеметрии TLS (SF-5b)

- Метрическое выравнивание:
  - Автоматизация TLS `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}`, или `sorafs_gateway_tls_ech_enabled`.
  - Индикаторы, панель обзора шлюза, панель TLS/сертификатов и многое другое.
- Связь с оповещениями:
  - Оповещения об истечении срока действия TLS активируются в течение 14 дней (осталось менее 14 дней) и не требующая доверия доступность SLO.
  - Отключение ECH, выдача вторичного оповещения и доступность TLS, панели и справочные материалы.
- Конвейер: задание автоматизации TLS в стеке Prometheus и экспорте, а также метрики шлюза. SF-5b обеспечивает координацию и дедуплицирование приборов.

## Соглашения об именах и метках метрик- Имена метрик, например, `torii_sorafs_*` или `sorafs_*`, префиксы, следуйте инструкциям по Torii, шлюз استعمال کرتے ہیں۔
- Наборы этикеток стандартизированы.
  - `result` → Результат HTTP (`success`, `refused`, `failed`).
  - `reason` → код отказа/ошибки (`unsupported_chunker`, `timeout` и т. д.).
  - `provider` → идентификатор поставщика в шестнадцатеричном формате.
  - `manifest` → дайджест канонического манифеста (высокая мощность میں Trim کیا جاتا ہے). 
  - `tier` → метки декларативного уровня (`hot`, `warm`, `archive`).
- Точки выброса телеметрии:
  - Метрики шлюза `torii_sorafs_*` можно использовать повторно, используя соглашения `crates/iroha_core/src/telemetry.rs`.
  - Оркестратор `sorafs_orchestrator_*` метрики и события `telemetry::sorafs.fetch.*` (жизненный цикл, повторная попытка, сбой поставщика, ошибка, остановка)
  - Узлы `torii_sorafs_storage_*`, `torii_sorafs_capacity_*`, или `torii_sorafs_por_*`.
- Наблюдаемость - координаты, метрические каталоги, общий Prometheus, именующий документ, регистрация, метки, ожидаемая кардинальность меток (поставщик/демонстрирует верхние границы) شامل ہوں۔

## Конвейер данных

- Коллекторы могут развертывать компоненты, использовать OTLP или Prometheus (метрики), Loki/Tempo (логи/трассировки) и экспортировать их.
- Дополнительные шлюзы/узлы eBPF (Tetragon) для низкоуровневой трассировки и обогащения возможностей.
- `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` или Torii для встроенных узлов, которые можно использовать. Оркестратор `install_sorafs_fetch_otlp_exporter` کو کال کرتا رہتا ہے۔

## Хуки проверки

- CI کے دوران `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں تاکہ Prometheus правила оповещений стоп-метрики и проверки подавления конфиденциальности и блокировка шага блокировки
- Grafana панели мониторинга и контроль версий (`dashboards/grafana/`) или панели управления, скриншоты/ссылки. کریں۔
- Упражнения хаоса کے نتائج `scripts/telemetry/log_sorafs_drill.sh` کے ذریعے log ہوتے ہیں؛ валидация `scripts/telemetry/validate_drill_log.sh` استعمال کرتی ہے (دیکھیے [Сборник операций](operations-playbook.md)).