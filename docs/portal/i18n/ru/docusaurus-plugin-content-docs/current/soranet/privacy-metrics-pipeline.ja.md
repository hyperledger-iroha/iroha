---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/soranet/privacy-metrics-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ec9f1d09c4ec1f3f963e44fc9b12e21ac5c25d4b9ca896b8b35fc4c66b35c51
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: privacy-metrics-pipeline
lang: ru
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
Отражает `docs/source/soranet/privacy_metrics_pipeline.md`. Держите обе копии синхронизированными, пока не будет выведена из эксплуатации старая документация.
:::

# Конвейер метрик приватности SoraNet

SNNet-8 вводит ориентированную на приватность поверхность телеметрии для runtime relay. Теперь relay агрегирует события handshake и circuit в минутные buckets и экспортирует только грубые счетчики Prometheus, оставляя отдельные circuits несвязываемыми и давая операторам практическую видимость.

## Обзор агрегатора

- Runtime-реализация находится в `tools/soranet-relay/src/privacy.rs` как `PrivacyAggregator`.
- Buckets индексируются по минутам настенного времени (`bucket_secs`, по умолчанию 60 секунд) и хранятся в ограниченном кольце (`max_completed_buckets`, по умолчанию 120). Collector shares имеют собственный ограниченный backlog (`max_share_lag_buckets`, по умолчанию 12), чтобы устаревшие окна Prio сбрасывались как suppressed buckets, а не утекали в память или маскировали зависшие collectors.
- `RelayConfig::privacy` напрямую сопоставляется с `PrivacyConfig`, открывая настройки (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). В production runtime сохраняются значения по умолчанию, а SNNet-8a вводит пороги безопасной агрегации.
- Модули runtime записывают события через типизированные helpers: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes` и `record_gar_category`.

## Админ-эндпоинт relay

Операторы могут опрашивать admin listener relay для сырых наблюдений через `GET /privacy/events`. Эндпоинт возвращает JSON с разделителями строк (`application/x-ndjson`), содержащий payloads `SoranetPrivacyEventV1`, отраженные из внутреннего `PrivacyEventBuffer`. Буфер хранит самые новые события до `privacy.event_buffer_capacity` записей (по умолчанию 4096) и очищается при чтении, поэтому скрейперам нужно опрашивать достаточно часто, чтобы избежать пропусков. События покрывают те же сигналы handshake, throttle, verified bandwidth, active circuit и GAR, которые питают счетчики Prometheus, позволяя downstream collectors архивировать приватно-безопасные breadcrumbs или подпитывать workflows безопасной агрегации.

## Конфигурация relay

Операторы настраивают cadence приватной телеметрии в конфигурационном файле relay через секцию `privacy`:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Значения по умолчанию соответствуют спецификации SNNet-8 и проверяются при загрузке:

| Поле | Описание | По умолчанию |
|------|----------|--------------|
| `bucket_secs` | Ширина каждого окна агрегации (секунды). | `60` |
| `min_handshakes` | Минимальное число участников перед выпуском счетчиков из bucket. | `12` |
| `flush_delay_buckets` | Количество завершенных buckets перед попыткой flush. | `1` |
| `force_flush_buckets` | Максимальный возраст перед выпуском suppressed bucket. | `6` |
| `max_completed_buckets` | Хранимый backlog buckets (не дает памяти расти без границ). | `120` |
| `max_share_lag_buckets` | Окно удержания collector shares до подавления. | `12` |
| `expected_shares` | Число Prio collector shares перед объединением. | `2` |
| `event_buffer_capacity` | Backlog NDJSON событий для admin-потока. | `4096` |

Установка `force_flush_buckets` ниже `flush_delay_buckets`, обнуление порогов или отключение guard удержания теперь проваливает валидацию, чтобы избежать развертываний, которые бы утекали телеметрией на уровне relay.

Лимит `event_buffer_capacity` также ограничивает `/admin/privacy/events`, гарантируя, что скрейперы не смогут бесконечно отставать.

## Prio collector shares

SNNet-8a разворачивает двойные collectors, которые выпускают секретно-разделенные buckets Prio. Orchestrator теперь парсит NDJSON поток `/privacy/events` для записей `SoranetPrivacyEventV1` и share `SoranetPrivacyPrioShareV1`, направляя их в `SoranetSecureAggregator::ingest_prio_share`. Buckets выпускаются, когда приходит `PrivacyBucketConfig::expected_shares` вкладов, отражая поведение relay. Shares валидируются по выравниванию buckets и форме гистограммы перед объединением в `SoranetPrivacyBucketMetricsV1`. Если итоговое число handshakes падает ниже `min_contributors`, bucket экспортируется как `suppressed`, повторяя поведение агрегатора внутри relay. Подавленные окна теперь выпускают метку `suppression_reason`, чтобы операторы могли отличать `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` и `forced_flush_window_elapsed` при диагностике провалов телеметрии. Причина `collector_window_elapsed` также срабатывает, когда Prio shares задерживаются дольше `max_share_lag_buckets`, делая зависшие collectors видимыми без хранения устаревших аккумуляторов в памяти.

## Эндпоинты приема Torii

Torii теперь публикует два телеметрических HTTP-эндпоинта, чтобы relays и collectors могли пересылать наблюдения без встраивания собственного транспорта:

- `POST /v2/soranet/privacy/event` принимает payload `RecordSoranetPrivacyEventDto`. Тело оборачивает `SoranetPrivacyEventV1` плюс необязательную метку `source`. Torii проверяет запрос по активному профилю телеметрии, записывает событие и отвечает HTTP `202 Accepted` вместе с Norito JSON-конвертом, содержащим вычисленное окно (`bucket_start_unix`, `bucket_duration_secs`) и режим relay.
- `POST /v2/soranet/privacy/share` принимает payload `RecordSoranetPrivacyShareDto`. Тело несет `SoranetPrivacyPrioShareV1` и необязательную подсказку `forwarded_by`, чтобы операторы могли аудитировать потоки collectors. Успешные отправки возвращают HTTP `202 Accepted` с Norito JSON-конвертом, суммирующим collector, окно bucket и подсказку подавления; ошибки валидации сопоставляются с телеметрическим ответом `Conversion`, чтобы сохранить детерминированную обработку ошибок между collectors. Цикл событий orchestrator теперь выпускает эти shares при опросе relays, сохраняя синхронизацию Prio-аккумулятора Torii с buckets на relay.

Оба эндпоинта учитывают профиль телеметрии: они возвращают `503 Service Unavailable`, когда метрики отключены. Клиенты могут отправлять Norito binary (`application/x.norito`) или Norito JSON (`application/x.norito+json`) тела; сервер автоматически договаривается о формате через стандартные Torii extractors.

## Метрики Prometheus

Каждый экспортируемый bucket несет метки `mode` (`entry`, `middle`, `exit`) и `bucket_start`. Выпускаются следующие семейства метрик:

| Metric | Description |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | Таксономия handshakes с `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Счетчики throttle с `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Агрегированные длительности cooldown, внесенные throttled handshakes. |
| `soranet_privacy_verified_bytes_total` | Проверенная пропускная способность от слепых доказательств измерений. |
| `soranet_privacy_active_circuits_{avg,max}` | Среднее и пик активных circuits на bucket. |
| `soranet_privacy_rtt_millis{percentile}` | Оценки RTT percentiles (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Хешированные счетчики Governance Action Report, ключом является digest категории. |
| `soranet_privacy_bucket_suppressed` | Buckets удержаны, потому что порог участников не был достигнут. |
| `soranet_privacy_pending_collectors{mode}` | Аккумуляторы collector shares в ожидании объединения, сгруппированные по режиму relay. |
| `soranet_privacy_suppression_total{reason}` | Счетчики suppressed buckets с `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}`, чтобы дашборды могли атрибутировать провалы приватности. |
| `soranet_privacy_snapshot_suppression_ratio` | Доля suppressed/слитых на последнем drain (0-1), полезно для бюджетов алертов. |
| `soranet_privacy_last_poll_unixtime` | UNIX-время самого свежего успешного опроса (питает алерт collector-idle). |
| `soranet_privacy_collector_enabled` | Gauge, который падает до `0`, когда privacy collector отключен или не запустился (питает алерт collector-disabled). |
| `soranet_privacy_poll_errors_total{provider}` | Ошибки опроса по алиасу relay (увеличивается при ошибках декодирования, HTTP-сбоях или неожиданных статус-кодах). |

Buckets без наблюдений остаются молчаливыми, сохраняя дашборды чистыми без создания нулевых окон.

## Операционные рекомендации

1. **Дашборды** - строьте метрики выше, группируя по `mode` и `window_start`. Подсвечивайте пропущенные окна, чтобы выявлять проблемы collectors или relays. Используйте `soranet_privacy_suppression_total{reason}` для различения недостатка участников и suppression, вызванной collectors, при триаже пробелов. В Grafana теперь есть панель **"Suppression Reasons (5m)"** на базе этих счетчиков и stat **"Suppressed Bucket %"**, вычисляющий `sum(soranet_privacy_bucket_suppressed) / count(...)` по выбранному диапазону, чтобы операторы видели нарушения бюджета с первого взгляда. Серия **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) и stat **"Snapshot Suppression Ratio"** выделяют зависшие collectors и дрейф бюджета во время автоматизированных прогонов.
2. **Алертинг** - запускайте тревоги от приватно-безопасных счетчиков: всплески PoW reject, частоту cooldown, дрейф RTT и capacity rejects. Поскольку счетчики монотонны в пределах каждого bucket, простые правила по скорости работают хорошо.
3. **Инцидент-ответ** - сначала полагайтесь на агрегированные данные. Когда нужен более глубокий дебаг, просите relays воспроизвести snapshots buckets или проверить слепые доказательства измерений вместо сбора сырых логов трафика.
4. **Удержание** - скрейпьте достаточно часто, чтобы не превысить `max_completed_buckets`. Экспортеры должны считать вывод Prometheus каноническим источником и удалять локальные buckets после отправки.

## Аналитика suppression и автоматизированные прогоны

Принятие SNNet-8 зависит от демонстрации того, что автоматизированные collectors остаются здоровыми, а suppression держится в пределах политики (≤10% buckets на relay за любое 30-минутное окно). Нужные инструменты теперь поставляются вместе с репозиторием; операторы должны встроить их в еженедельные ритуалы. Новые панели suppression в Grafana повторяют PromQL-выражения ниже, давая дежурным командам живую видимость до того, как они прибегнут к ручным запросам.

### PromQL-рецепты для обзора suppression

Операторы должны держать под рукой следующие PromQL-хелперы; оба упомянуты в общем дашборде Grafana (`dashboards/grafana/soranet_privacy_metrics.json`) и правилах Alertmanager:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

Используйте выходное соотношение, чтобы подтвердить, что stat **"Suppressed Bucket %"** остается ниже бюджетного порога; подключите детектор всплесков к Alertmanager для быстрого отклика, когда число участников неожиданно падает.

### CLI для офлайн-отчета buckets

В workspace доступен `cargo xtask soranet-privacy-report` для разовых NDJSON-выгрузок. Укажите один или несколько admin-экспортов relay:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

Хелпер прогоняет захват через `SoranetSecureAggregator`, печатает сводку suppression в stdout и по желанию пишет структурированный JSON-отчет через `--json-out <path|->`. Он учитывает те же настройки, что и live collector (`--bucket-secs`, `--min-contributors`, `--expected-shares` и т. д.), позволяя операторам воспроизводить исторические захваты с другими порогами при разборе инцидента. Приложите JSON вместе со скриншотами Grafana, чтобы проход SNNet-8 по аналитике suppression оставался аудируемым.

### Чеклист первой автоматизированной сессии

Говернанс по-прежнему требует доказать, что первая автоматизированная сессия уложилась в бюджет suppression. Хелпер теперь принимает `--max-suppression-ratio <0-1>`, чтобы CI или операторы могли быстро завершаться ошибкой, когда suppressed buckets превышают допустимое окно (по умолчанию 10%) или когда buckets еще отсутствуют. Рекомендуемый поток:

1. Экспортировать NDJSON из admin-endpoints relay и потока orchestrator `/v2/soranet/privacy/event|share` в `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Запустить хелпер с бюджетом политики:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Команда печатает наблюдаемое соотношение и завершает работу с ненулевым кодом, когда бюджет превышен **или** когда buckets еще не готовы, сигнализируя, что телеметрия еще не была произведена для прогона. Live-метрики должны показывать, что `soranet_privacy_pending_collectors` сливается к нулю, а `soranet_privacy_snapshot_suppression_ratio` остается ниже того же бюджета во время прогона.
3. Архивируйте JSON-вывод и лог CLI вместе с пакетом доказательств SNNet-8 до переключения транспорта по умолчанию, чтобы ревьюеры могли воспроизвести точные артефакты.

## Следующие шаги (SNNet-8a)

- Интегрировать двойные Prio collectors, подключив прием shares к runtime, чтобы relays и collectors выпускали согласованные payloads `SoranetPrivacyBucketMetricsV1`. *(Готово — см. `ingest_privacy_payload` в `crates/sorafs_orchestrator/src/lib.rs` и сопутствующие тесты.)*
- Опубликовать общий Prometheus dashboard JSON и правила алертов, покрывающие suppression gaps, здоровье collectors и анонимность. *(Готово — см. `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` и проверки.)*
- Подготовить артефакты калибровки дифференциальной приватности, описанные в `privacy_metrics_dp.md`, включая воспроизводимые блокноты и governance digest. *(Готово — notebook и артефакты генерируются `scripts/telemetry/run_privacy_dp.py`; CI wrapper `scripts/telemetry/run_privacy_dp_notebook.sh` исполняет notebook через workflow `.github/workflows/release-pipeline.yml`; digest сохранен в `docs/source/status/soranet_privacy_dp_digest.md`.)*

Текущий релиз доставляет основу SNNet-8: детерминированную, приватно-безопасную телеметрию, которая напрямую подключается к существующим скрейперам и дашбордам Prometheus. Артефакты калибровки дифференциальной приватности на месте, workflow релизного пайплайна поддерживает актуальность вывода notebook, а оставшаяся работа сосредоточена на мониторинге первого автоматизированного прогона и расширении аналитики suppression-алертов.
