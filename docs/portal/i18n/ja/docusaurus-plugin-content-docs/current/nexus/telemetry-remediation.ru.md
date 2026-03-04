---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-telemetry-remediation
タイトル: План устранения пробелов телеметрии Nexus (B2)
説明: Зеркало `docs/source/nexus_telemetry_remediation_plan.md`、документирующее матрицу пробелов телеметрии и операционный рабочий процесс。
---

# Обзор

Пункт ロードマップ **B2 - владение пробелами телеметрии** требует опубликованного плана, который привязывает каждый оставлийся пробел телеметрии Nexus к сигналу, защитному порогу оповещений, владельцу, дедлайну и артефакту проверки 2026 年第 1 四半期にリリースされます。 `docs/source/nexus_telemetry_remediation_plan.md`、リリース エンジニアリング、テレメトリ運用、SDK の説明。ルートトレースと `TRACE-TELEMETRY-BRIDGE` が必要です。

# Матрица пробелов

|ギャップ ID | Сигнал и защитный порог оповещения | Владелец / эскалация | Срок (UTC) | Доказательства и проверка |
|----------|----------------------|----------------------|-----------|----------------------|
| `GAP-TELEM-001` | `torii_lane_admission_latency_seconds{lane_id,endpoint}` は **`SoranetLaneAdmissionLatencyDegraded`**、5 分以内に `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` が表示されます(`dashboards/alerts/soranet_lane_rules.yml`)。 | `@torii-sdk` (標準) + `@telemetry-ops` (標準);オンコール ルーテッド トレース Nexus。 | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` を使用して、`TRACE-LANE-ROUTING` を使用してスクレープを実行します。 Torii `/metrics` × [Nexus 移行メモ](./nexus-transition-notes)。 |
| `GAP-TELEM-002` | Счетчик `nexus_config_diff_total{knob,profile}` с ガードレール `increase(nexus_config_diff_total{profile="active"}[5m]) > 0`, блокирующим деплой (`docs/source/telemetry.md`) | `@nexus-core` (инструментирование) -> `@telemetry-ops` (алерт);ガバナンスを維持する必要があります。 | 2026-02-26 |ガバナンスの予行演習 сохраняются рядом с `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; чеклист релиза включает скринSoupот запроса Prometheus и отрывок логов, подтверждающий, что `StateTelemetry::record_nexus_config_diff`差分を確認します。 |
| `GAP-TELEM-003` | `TelemetryEvent::AuditOutcome` (`nexus.audit.outcome`) は **`NexusAuditOutcomeFailure`** を表示します。 30 分以内に到着します (`dashboards/alerts/nexus_audit_rules.yml`)。 | `@telemetry-ops` (パイプライン) と `@sec-observability`。 | 2026-02-27 | CI-гейт `scripts/telemetry/check_nexus_audit_outcome.py` архивирует NDJSON ペイロード и падает, когда окно TRACE не содержит события успеха;これは、routed-trace の機能です。 |
| `GAP-TELEM-004` |ゲージ `nexus_lane_configured_total` とガードレール `nexus_lane_configured_total != EXPECTED_LANE_COUNT`、オンコール SRE です。 | `@telemetry-ops` (ゲージ/エクスポート) с эскалацией в `@nexus-core`, когда узлы сообщают о несовпадающих размерах каталога. | 2026-02-28 | Тест телеметрии планировщика `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` подтверждает эмиссию;差分 Prometheus + отрывок лога `StateTelemetry::set_nexus_catalogs` と TRACE を比較します。 |

# Операционный рабочий процесс

1. **Еженедельный триаж.** Владельцы отчитываются о прогрессе на Nexus readiness созвоне; `status.md` を参照してください。
2. **予行演習です** Каждое правило алерта поставляется вместе с записью `dashboards/alerts/tests/*.test.yml`, чтобы CI запускал `promtool test rules` ガードレール。
3. **Доказательства для аудита.** Во время репетиций `TRACE-LANE-ROUTING` и `TRACE-TELEMETRY-BRIDGE` дежурный собирает результаты запросов Prometheus, историю алертов и релевантные выводы скриптов (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` 日)これは、routed-trace の機能です。
4. **Эскалация.** ガードレール срабатывает вне репетиционного окна, команда-владелец открывает Nexus инцидент,スナップショットを取得すると、スナップショットが表示されます。

С опубликованной матрицей - и сылками из `roadmap.md` и `status.md` - ロードマップ **B2** を表示するприемки「ответственность、срок、алерт、проверка」。