---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プライバシー-メトリクス-パイプライン
タイトル: Конвейер метрик приватности SoraNet (SNNet-8)
サイドバーラベル: Конвейер метрик приватности
説明: Сбор телеметрии с сохранением приватности для リレーとオーケストレーター SoraNet。
---

:::note Канонический источник
`docs/source/soranet/privacy_metrics_pipeline.md` です。あなたのことを忘れないでください。
:::

# Конвейер метрик приватности SoraNet

SNNet-8 はランタイム リレーを実行します。ハンドシェイク、回路、バケット、экспортирует только грубые счетчики Prometheus, оставляя をリレーに接続します。 отдельные 回路 несвязываемыми и давая операторам практическую видимость。

## Обзор агрегатора

- ランタイム - `tools/soranet-relay/src/privacy.rs` と `PrivacyAggregator` の組み合わせ。
- バケット индексируются по минутам настенного времени (`bucket_secs`, по умолчанию 60 секунд) и хранятся в ограниченном кольце (`max_completed_buckets`、по умолчанию 120)。コレクターの株式 имеют собственный ограниченный バックログ (`max_share_lag_buckets`, по умолчанию 12)、чтобы устаревлие окна Prio сбрасывались как が抑制されましたバケット、コレクター向けのアイテムです。
- `RelayConfig::privacy` напрямую сопоставляется с `PrivacyConfig`, открывая настройки (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`、`max_completed_buckets`、`max_share_lag_buckets`、`expected_shares`)。プロダクション ランタイムは、SNNet-8a からの要求に応じて実行されます。
- ランタイムのヘルパー: `record_circuit_accepted`、`record_circuit_rejected`、`record_throttle`、`record_throttle_cooldown`、 `record_capacity_reject`、`record_active_sample`、`record_verified_bytes`、`record_gar_category`。

## Админ-эндпоинт リレー

管理者リスナー リレーが `GET /privacy/events` にアクセスしています。 JSON を使用して (`application/x-ndjson`)、ペイロード `SoranetPrivacyEventV1`、отраженные из внутреннего `PrivacyEventBuffer`。 Буфер хранит самые новые события до `privacy.event_buffer_capacity` записей (по умолчанию 4096) и очищается при чтении, поэтому最高のパフォーマンスを見せてください。ハンドシェイク、スロットル、検証済み帯域幅、アクティブ回路および GAR、Prometheus、ダウンストリーム コレクターの詳細ワークフローのブレッドクラムとワークフローを確認します。

## Конфигурация リレー

リレー リレー через секцию `privacy`:

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

SNNet-8 との接続:| Поле | Описание | По умолчанию |
|------|----------|--------------|
| `bucket_secs` | Ширина каждого окна агрегации (секунды)。 | `60` |
| `min_handshakes` | Минимальное число участников перед выпуском счетчиков из バケット。 | `12` |
| `flush_delay_buckets` |バケットはフラッシュされます。 | `1` |
| `force_flush_buckets` |抑制されたバケット。 | `6` |
| `max_completed_buckets` |バックログ バケット (バックログ バケット)。 | `120` |
| `max_share_lag_buckets` |コレクターは株式を保有しています。 | `12` |
| `expected_shares` | Prio コレクターは株式を保有しています。 | `2` |
| `event_buffer_capacity` |バックログ NDJSON は管理者権限を持っています。 | `4096` |

Установка `force_flush_buckets` ниже `flush_delay_buckets`, обнуление порогов или отключение guard удержания теперь проваливает валидацию、чтобы избежать развертываний、которые бы утекали телеметрией на уровне リレー。

Лимит `event_buffer_capacity` также ограничивает `/admin/privacy/events`, гарантируя, что скрейперы не смогут бесконечно отставать.

## Prio コレクター株

SNNet-8a はコレクター、バケット Prio を備えています。オーケストレーターは、NDJSON を使用して `/privacy/events` と `SoranetPrivacyEventV1` を共有し、`SoranetPrivacyPrioShareV1` を共有し、`SoranetSecureAggregator::ingest_prio_share` を共有します。バケット выпускаются、когда приходит `PrivacyBucketConfig::expected_shares` вкладов、отражая поведение リレー。バケットと `SoranetPrivacyBucketMetricsV1` を共有します。 Если итоговое число ハンドシェイク падает ниже `min_contributors`、バケット экспортируется как `suppressed`、повторяя поведение агрегатора внутри リレー。 Подавленные окна теперь выпускают метку `suppression_reason`, чтобы операторы могли отличать `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` および `forced_flush_window_elapsed` は、これを実行します。 Причина `collector_window_elapsed` также срабатывает, когда Prio 株 задерживаются дользе `max_share_lag_buckets`, делая завислие コレクター видимымиあなたのことを思い出してください。

## Эндпоинты приема Torii

Torii теперь публикует два телеметрических HTTP-эндпоинта、чтобы リレー、コレクターのメッセージが表示されます。必要な情報:- `POST /v2/soranet/privacy/event` ペイロード `RecordSoranetPrivacyEventDto`。 Тело оборачивает `SoranetPrivacyEventV1` плюс необязательную метку `source`. Torii HTTP `202 Accepted` は、HTTP `202 Accepted` Norito JSON-конвертом、содержащим вычисленное окно (`bucket_start_unix`, `bucket_duration_secs`) および режим リレー。
- `POST /v2/soranet/privacy/share` ペイロード `RecordSoranetPrivacyShareDto`。 `SoranetPrivacyPrioShareV1` と `forwarded_by` のコレクション、コレクター向けのコレクションです。 HTTP `202 Accepted` と Norito JSON-конвертом、суммирующим コレクター、окно バケットと подсказку और देखें олибки валидации сопоставляются с телеметрическим ответом `Conversion`, чтобы сохранить детерминированнуюコレクター。 Цикл событий オーケストレーターが、リレー、リレー、バケットを共有します。

Оба эндпоинта учитывают профиль телеметрии: они возвращают `503 Service Unavailable`, когда метрики отключены. Norito バイナリ (`application/x.norito`) または Norito JSON (`application/x.norito+json`) を使用します。 Torii エクストラクターを使用してください。

## Метрики Prometheus

バケット `mode` (`entry`、`middle`、`exit`) および `bucket_start`。 Выпускаются следующие семейства метрик:

|メトリック |説明 |
|----------|---------------|
| `soranet_privacy_circuit_events_total{kind}` |ハンドシェイクは `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` です。 |
| `soranet_privacy_throttles_total{scope}` | Счетчики スロットル с `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`。 |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` |クールダウン、スロットル ハンドシェイクの停止。 |
| `soranet_privacy_verified_bytes_total` | Проверенная пропускная способность от слепых доказательств измерений. |
| `soranet_privacy_active_circuits_{avg,max}` |バケット内の回路。 |
| `soranet_privacy_rtt_millis{percentile}` | RTT パーセンタイル (`p50`、`p90`、`p99`)。 |
| `soranet_privacy_gar_reports_total{category_hash}` |ガバナンス行動報告書、ダイジェスト版を参照してください。 |
| `soranet_privacy_bucket_suppressed` |バケットは、すべてのデータを保存します。 |
| `soranet_privacy_pending_collectors{mode}` | Аккумуляторы コレクターは、リレーを共有します。 |
| `soranet_privacy_suppression_total{reason}` |抑制されたバケット с `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` は、バケットを削除するために使用されます。 |
| `soranet_privacy_snapshot_suppression_ratio` |抑圧/ドレイン (0-1) での勝利。 |
| `soranet_privacy_last_poll_unixtime` | UNIX の場合は、コレクターアイドル状態です。 |
| `soranet_privacy_collector_enabled` |ゲージ、`0` のプライバシー コレクター (コレクターが無効になっている)。 |
| `soranet_privacy_poll_errors_total{provider}` | Олибки опроса по алиасу リレー (увеличивается при олибках декодирования、HTTP-сбоях или неожиданных статус-кодах)。 |

バケットは、すべてのバケットを保持します。

## Операционные рекомендации1. **Даборды** - строьте метрики выbolе, группируя по `mode` и `window_start`.コレクターとリレーの両方を備えています。 Используйте `soranet_privacy_suppression_total{reason}` для различения недостатка участников и 抑制、вызванной コレクター、при триаже пробелов。 В Grafana теперь есть панель **「抑制の理由 (5m)」** は、統計 **「抑制されたバケット %」**, вычисляющий `sum(soranet_privacy_bucket_suppressed) / count(...)` は、今日の状況を確認します。 **「コレクター共有バックログ」** (`soranet_privacy_pending_collectors`) および統計 **「スナップショット抑制率」** は、コレクターとの統合を確認します。 автоматизированных прогонов。
2. **Алертинг** - 説明: PoW 拒否、クールダウン、RTT および容量拒否。バケットを作成し、バケットを作成します。
3. **Инцидент-ответ** - Инчала полагайтесь на агрегированные данные.リレーとスナップショット バケットの両方を実行し、リレーとスナップショットを実行します。 Сместо сбора сырых логов трафика。
4. **Удержание** - скрейпьте достаточно часто, чтобы не превысить `max_completed_buckets`. Экспортеры должны считать вывод Prometheus каноническим источником и удалять локальные バケツが отправки にあります。

## Аналитика 抑制 и автоматизированные прогоны

SNNet-8 のサポート、コレクターのサポート、抑制機能のサポートпределах политики (≤10% バケット на リレー за любое 30-минутное окно)。 Нужные инструменты теперь поставляются вместе с репозиторием; жераторы должны встроить их в еженедельные ритуалы. Grafana による PromQL-выражения ниже による抑制の解除、 давая дежурным командам живую видимость до того、 как они прибегнут к ручным запросам.

### PromQL による抑制

PromQL-хелперы を使用してください。 Grafana (`dashboards/grafana/soranet_privacy_metrics.json`) およびアラート マネージャー:

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

統計情報 **「抑制されたバケット %」** を表示します。アラートマネージャーのメッセージが表示されます。

### CLI のバケット

ワークスペースは `cargo xtask soranet-privacy-report` で NDJSON-выгрузок です。 Укажите один или несколько admin-экспортов リレー:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

`SoranetSecureAggregator` を使用して、標準出力を抑制し、標準出力を無効にします。 JSON-отчет через `--json-out <path|->`。 Он учитывает те же настройки, что и ライブコレクター (`--bucket-secs`, `--min-contributors`, `--expected-shares` и т. д.), позволяя операторам問題を解決するには、次のことを試してください。 JSON が Grafana をサポートし、SNNet-8 が抑制します。### Чеклист первой автоматизированной сессии

Говернанс по-прежнему требует доказать, что первая автоматизированная сессия уложилась в бюджет 抑制。 Хелпер теперь принимает `--max-suppression-ratio <0-1>`, чтобы CI или операторы могли быстро заверперь принимает `--max-suppression-ratio <0-1>`, когда 抑制されたバケット превывают допустимое окно (по умолчанию 10%) или когда バケツ еще отсутствуют. Рекомендуемый поток:

1. NDJSON と管理エンドポイント リレーとオーケストレーター `/v2/soranet/privacy/event|share` と `artifacts/sorafs_privacy/<relay>.ndjson` を組み合わせます。
2. 必要な情報:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Команда печатает наблюдаемое и заверbolиает работу с ненулевым кодом, когда бюджет превыленили** когда バケット еще не готовы、сигнализируя、что телеметрия еще не была произведена для прогона. Live-метрики должны показывать, что `soranet_privacy_pending_collectors` сливается к нулю, а `soranet_privacy_snapshot_suppression_ratio` остается ниже того же бюджета Свремя прогона.
3. JSON-вывод и лог CLI вместе с пакетом доказательств SNNet-8 до переключения транспорта по умолчанию, Їтобы ревьюеры могли воспроизвести точные артефакты.

## Следующие заги (SNNet-8a)

- Prio コレクター、ランタイム共有、リレー、コレクターのペイロード `SoranetPrivacyBucketMetricsV1` を確認します。 *(Готово — см. `ingest_privacy_payload` в `crates/sorafs_orchestrator/src/lib.rs` и сопутствующие тесты.)*
- Prometheus ダッシュボード JSON と、サプレッション ギャップ、コレクター、および анонимность を確認します。 *(Готово — см. `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` и проверки.)*
- Подготовить артефакты калибровки дифференциальной приватности, описанные в `privacy_metrics_dp.md`, включаяガバナンスのダイジェストをご覧ください。 *(Готово — ノートブック и артефакты генерируются `scripts/telemetry/run_privacy_dp.py`; CI ラッパー `scripts/telemetry/run_privacy_dp_notebook.sh` исполняет ノートブック через ワークフロー `.github/workflows/release-pipeline.yml`; ダイジェスト сохранен в `docs/source/status/soranet_privacy_dp_digest.md`.)*

SNNet-8 からのメッセージ: детерминированную、приватно-безопасную телеметрию、которая напрямую Prometheus を確認してください。 Артефакты калибровки дифференциальной приватности на месте、ワークフロー пайплайна поддерживает актуальность вывода ノートブック、а оставbolаяся работа сосредоточена на мониторинге первого автоматизированного прогона и расзирении аналитики抑圧-алертов。