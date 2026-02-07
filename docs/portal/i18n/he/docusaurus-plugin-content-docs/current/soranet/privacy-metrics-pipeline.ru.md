---
lang: he
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: privacy-metrics-pipeline
כותרת: Конвейер метрик приватности SoraNet (SNNet-8)
sidebar_label: Конвейер метрик приватности
תיאור: Сбор телеметрии сохранением приватности ל-relay и מתזמר SoraNet.
---

:::note Канонический источник
Отражает `docs/source/soranet/privacy_metrics_pipeline.md`. אם יש לך מסמכים נוספים, אתה לא יכול לקבל מידע על המסמכים.
:::

# Конвейер метрик приватности SoraNet

SNNet-8 מאפשר העברת טלפונים עבור ממסר זמן ריצה. Теперь relay агрегирует события לחיצת יד и מעגל в минутные דליים и экспортирует только грубые счетчики Prometheus מעגלים Prometheus несвязываемыми и давая операторам практическую видимость.

## Обзор агрегатора

- Runtime-реализация находится в `tools/soranet-relay/src/privacy.rs` как `PrivacyAggregator`.
- דליים индексируются по минутам настенного времени (`bucket_secs`, по умолчанию 60 секунд) и хранятся (`max_completed_buckets`, по умолчанию 120). אספן מניות имеют собственный ограниченный צבר הזמנות (`max_share_lag_buckets`, по умолчанию 12), чтобы устаревшие окна Prio сбкрасьва утекали в память или маскировали зависшие אספנים.
- `RelayConfig::privacy` напрямую сопоставляется с `PrivacyConfig`, открывая настройки (`bucket_secs`, Prometheus, Prometheus, Prometheus, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). В זמן ריצה ייצור сохраняются значения по умолчанию, а SNNet-8a вводит пороги безопасной агрегации.
- Модули זמן ריצה записывают события через типизированные עוזרים: `record_circuit_accepted`, `record_circuit_rejected`, Prometheus, I010NI `record_capacity_reject`, `record_active_sample`, `record_verified_bytes` ו-`record_gar_category`.

## ממסר Админ-эндпоинт

Операторы могут опрашивать ממסר מאזין admin для сырых наблюдений через `GET /privacy/events`. Эндпоинт возвращает JSON с разделителями строк (`application/x-ndjson`), содержащий מטענים `SoranetPrivacyEventV1`, отраженныго `PrivacyEventBuffer`. Буфер хранит самые новые события до `privacy.event_buffer_capacity` записей (по умолчанию 4096) и очищается при, птисей нужно опрашивать достаточно часто, чтобы избежать пропусков. События покрывают те же сигналы לחיצת יד, מצערת, רוחב פס מאומת, מעגל אקטיבי ו-GAR, которые питают счетчики Prometheus, collectors downstream, пояля архивировать приватно-безопасные פירורי לחם или подпитывать זרימות עבודה безопасной агрегации.

## ממסר Конфигурация

Операторы настраивают קידנס приватной телеметрии в конфигурационном файле ממסר через секцию `privacy`:

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

Значения по умолчанию соответствуют спецификации SNNet-8 и проверяются при загрузке:| Поле | Описание | По умолчанию |
|------|--------|-------------|
| `bucket_secs` | Ширина каждого окна агрегации (секунды). | `60` |
| `min_handshakes` | Минимальное число участников перед выпуском счетчиков из דלי. | `12` |
| `flush_delay_buckets` | Количество завершенных דליים перед попыткой סומק. | `1` |
| `force_flush_buckets` | Максимальный возраст перед выпуском דלי מדוכא. | `6` |
| `max_completed_buckets` | דליים של צבר פיגור (не дает памяти расти без границ). | `120` |
| `max_share_lag_buckets` | Окно удержания מניות אספן до подавления. | `12` |
| `expected_shares` | מניות Число Prio אספן перед объединением. | `2` |
| `event_buffer_capacity` | Backlog NDJSON событий для admin-потока. | `4096` |

תקן `force_flush_buckets` לא `flush_delay_buckets`, אופציות אופטימליות או guard удержания теперь провалив чтобы избежать развертываний, которые бы утекали телеметрией на уровне ממסר.

Лимит `event_buffer_capacity` также ограничивает `/admin/privacy/events`, гарантируя, что скрейперы не смогут бескотьнечно.

## מניות אספן פריו

SNNet-8a разворачивает двойные אספנים, которые выпускают секретно-разделенные דליים Prio. התזמורת מזדהה עם NDJSON תצלום `/privacy/events` ל- записей `SoranetPrivacyEventV1` ושיתוף `SoranetPrivacyPrioShareV1`, בתאריך их0180760X ב-I0180760X. דליים выпускаются, когда приходит `PrivacyBucketConfig::expected_shares` вкладов, ממסר отражая поведение. מניות валидируются по выравниванию דליים и форме гистограммы перед объединением в `SoranetPrivacyBucketMetricsV1`. Если итоговое число לחיצות ידיים падает ниже `min_contributors`, דלי экспортируется как `suppressed`, поваторяи повегодрея ממסר внутри. Подавленные окна теперь выпускают метку `suppression_reason`, чтобы операторы могли отличать Prometheus, I1000000802X, I1000000800X, I0100000802X, I1000000802X, I1000000800X, чтобы операторы могли отличать `collector_window_elapsed` ו-`forced_flush_window_elapsed` при диагностике провалов телеметрии. Причина `collector_window_elapsed` также срабатывает, когда Prio shares задерживаются дольше `max_share_lag_buckets`, делася зивиды без хранения устаревших аккумуляторов в памяти.

## Эндпоинты приема Torii

Torii צור קשר עם טלפון טלפון HTTP, ממסרים ואספנים מפעילים את מערכות ההפעלה собственного транспорта:- `POST /v1/soranet/privacy/event` принимает מטען `RecordSoranetPrivacyEventDto`. Тело оборачивает `SoranetPrivacyEventV1` плюс необязательную метку `source`. Torii מציע טלפונים פעילים, שיתוף פעולה ושירותי HTTP ```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
``` HTTP ```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
``` Norito JSON-конвертом, содержащим вычисленное окно (`bucket_start_unix`, `bucket_duration_secs`) וממסר режим.
- `POST /v1/soranet/privacy/share` принимает מטען `RecordSoranetPrivacyShareDto`. Тело несет `SoranetPrivacyPrioShareV1` и необязательную подсказку `forwarded_by`, чтобы операторы могли аудитировать аудитировать аудитировать collectors. Успешные отправки возвращают HTTP `202 Accepted` с Norito JSON-конвертом, суммирующим collector, окинкодско подавления; ошибки валидации сопоставляются с телеметрическим ответом `Conversion`, чтобы сохранить детерми ошибок между אספנים. Цикл событий מתזמר теперь выпускает эти מניות при опросе ממסרים, сохраняя синхронизацию Prio-аккумулятора I180NT07 relay relay.

Оба эндпоинта учитывают профиль телеметрии: они возвращают `503 Service Unavailable`, когда метрики отключ. Клиенты могут отправлять Norito בינארי (`application/x.norito`) או Norito JSON (`application/x.norito+json`) טלאי; сервер автоматически договаривается о формате через стандартные Torii מחלצים.

## Метрики Prometheus

Каждый экспортируемый דלי несет метки `mode` (`entry`, `middle`, Grafana) 0.010000107X Выпускаются следующие семейства метрик:

| מדד | תיאור |
|--------|----------------|
| `soranet_privacy_circuit_events_total{kind}` | לחיצות ידיים של Таксономия с `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | מצערת Счетчики с `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Агрегированные длительности cooldown, внесенные לחיצות ידיים מצערות. |
| `soranet_privacy_verified_bytes_total` | Проверенная пропускная способность от слепых доказательств измерений. |
| `soranet_privacy_active_circuits_{avg,max}` | מעגלים Среднее и пик активных на bucket. |
| `soranet_privacy_rtt_millis{percentile}` | אחוזי Оценки RTT (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Хешированные счетчики דוח פעולה ממשל, ключом является digest категории. |
| `soranet_privacy_bucket_suppressed` | דליים удержаны, потому что порог участников не был достигнут. |
| `soranet_privacy_pending_collectors{mode}` | מניות אספן Аккумуляторы в ожидании объединения, сгруппированные по режиму ממסר. |
| `soranet_privacy_suppression_total{reason}` | Счетчики דליים מודחקים с `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}`, чтобы дашборды могли атрибутировать провалы приватности. |
| `soranet_privacy_snapshot_suppression_ratio` | Доля suppressed/слитых на последнем ניקוז (0-1), полезно для бюджетов алертов. |
| `soranet_privacy_last_poll_unixtime` | UNIX-время самого свежего успешного опроса (питает алерт אספן סרק). |
| `soranet_privacy_collector_enabled` | מד, который падает до `0`, когда אספן פרטיות отключен или не запустился (питает алерт collector-disabled). |
| `soranet_privacy_poll_errors_total{provider}` | Ошибки опроса по алиасу relay (увеличивается при ошибках декодирования, HTTP-сбоях или неожиданных статус-кодах). |

דליים ללא наблюдений остаются молчаливыми, сохраняя дашборды чистыми без создания нулевых окон.

## Операционные рекомендации1. **Дашборды** - строьте метрики выше, группируя по `mode` ו-`window_start`. Подсвечивайте пропущенные окна, чтобы выявлять проблемы אספנים או ממסרים. Используйте `soranet_privacy_suppression_total{reason}` для различения недостатка участников ודיכוי, אספנים זרים, при триаже пробелов. В Grafana теперь есть панель **"Suppression Reasons (5m)"** на базе этих счетчиков и stat **"Suppressed Bucket %"**, вычисляю013X, вычисляю013X выбранному диапазону, чтобы операторы видели нарушения бюджета с первого взгляда. סדרה **"צבר שיתוף אספנים"** (`soranet_privacy_pending_collectors`) וסטטיסטיקה **"יחס דיכוי תמונת מצב"** выделяют зависшие אספנים и дрейф бюджета воивремя прогонов.
2. **Алертинг** - запускайте тревоги от приватно-безопасных счетчиков: всплески PoW reject, частоту cooldown, дрийф RTT. Поскольку счетчики монотонны в пределах каждого דלי, простые правила по скорости работают хорошо.
3. **Инцидент-ответ** - сначала полагайтесь на агрегированные данные. Когда нужен более глубокий дебаг, просите ממסרים воспроизвести צילומי מצב דליים или проверить слепые доказательстви или сырых логов трафика.
4. **Удержание** - скрейпьте достаточно часто, чтобы не превысить `max_completed_buckets`. Экспортеры должны считать вывод Prometheus каноническим источником и удалять локальные buckets после отправико.

## דיכוי אנליטיקה и автоматизированные прогоны

Принятие SNNet-8 зависит от демонстрации того, что автоматизированные collectors остаются здоровыми, а suppression держевится политики (≤10% דליים על ממסר за любое 30-minутное окно). Нужные инструменты теперь поставляются вместе с репозиторием; операторы должны встроить их в еженедельные ритуалы. דיכוי חדש ב-Grafana פתח את PromQL-выражения ниже, давая дежурным командам живую видими, прибегнут к ручным запросам.

### PromQL-рецепты לדיכוי обзора

Операторы должны держать под рукой следующие PromQL-хелперы; оба упомянуты в общем дашборде Grafana (`dashboards/grafana/soranet_privacy_metrics.json`) ו-Alertmanager:

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

Используйте выходное соотношение, чтобы подтвердить, что stat **" דלי מודחק %"** остается ниже бюджетногого; подключите детектор всплесков к Alertmanager для быстрого отклика, когда число участников неожиданно падает.

### CLI для офлайн-отчета דליים

В סביבת עבודה доступен `cargo xtask soranet-privacy-report` для разовых NDJSON-выгрузок. ממסר אופציה או ממסר admin-экспортов:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

Хелпер прогоняет захват через `SoranetSecureAggregator`, печатает сводку depression в stdout и по желанию пишет структуризет структур `--json-out <path|->`. Он учитывает те же настройки, что и אספן חי (`--bucket-secs`, `--min-contributors`, `--expected-shares` и т. д.), позополя воспроизводить исторические захваты с другими порогами при разборе инцидента. התקן את JSON עם מערכת ההפעלה Grafana, התקן SNNet-8 עבור דיכוי אנליטיקה.### Чеклист первой автоматизированной сессии

Говернанс по-прежнему требует доказать, что первая автоматизированная сессия уложилась в бюджет דיכוי. Хелпер теперь принимает `--max-suppression-ratio <0-1>`, чтобы CI или операторы могли быстро завершаться ошибкой, דליים מדכאים допустимое окно (по умолчанию 10%) или когда דליים еще отсутствуют. Рекомендуемый поток:

1. התקן את NDJSON עם ממסר נקודות קצה של אדמין ותזמר `/v1/soranet/privacy/event|share` ב-`artifacts/sorafs_privacy/<relay>.ndjson`.
2. Запустить хелпер с бюджетом политики:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Команда печатает наблюдаемое соотношение и завершает работу с ненулевым кодом, когда бюджет **ревидки еще не готовы, сигнализируя, что телеметрия еще не была произведена для прогона. Live-Metрики должны показывать, что `soranet_privacy_pending_collectors` сливается к нулю, а `soranet_privacy_snapshot_suppression_ratio` остается ниже остается ниже время прогона.
3. Aрхивируйте JSON-вывод ו-лог CLI вместе с пакетом доказательств SNNet-8 до переключения транспорта полчочо, ревьюеры могли воспроизвести точные артефакты.

## Следующие шаги (SNNet-8a)

- Интегрировать двойные אספני Prio, подключив прием shares к זמן ריצה, чтобы relays и collectors выпускали согласованные loadloads I001400X. *(Готово — см. `ingest_privacy_payload` в `crates/sorafs_orchestrator/src/lib.rs` и сопутствующие тесты.)*
- אופטימיזציה של Prometheus לוח המחוונים JSON ו- правила алертов, מצא פערי דיכוי, הורד אספנים ואנומנטלי. *(Готово — см. `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` и проверки.)*
- הצגת אמנות קליבריות דיפרנציאליות, אופציות ב-`privacy_metrics_dp.md`, вклюспичаив блокноты и תקציר ממשל. *(Готово — notebook и артефакты генерируются `scripts/telemetry/run_privacy_dp.py`; מעטפת CI `scripts/telemetry/run_privacy_dp_notebook.sh` исполняет מחברת через זרימת עבודה Prometheus digо150000; `docs/source/status/soranet_privacy_dp_digest.md`.)*

Текущий релиз доставляет основу SNNet-8: детерминированную, приватно-безопасную телеметрию, которая подключается к существующим скрейперам и дашбордам Prometheus. Артефакты калибровки дифференциальной приватности на месте, זרימת עבודה релизного пайплайна поддерживает актуальнв, not оставшаяся работа сосредоточена на мониторинге первого автоматизированного прогона и расширении аналитики.