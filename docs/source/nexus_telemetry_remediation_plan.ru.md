---
lang: ru
direction: ltr
source: docs/source/nexus_telemetry_remediation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19d46f99e2ba79c56cbc3af65b47f5fb6997fa66f8ee951806b21696418a1d7b
source_last_modified: "2025-11-27T14:13:33.645951+00:00"
translation_last_reviewed: 2026-01-01
---

% План устранения телеметрических пробелов Nexus (фаза B2)

# Обзор

Пункт дорожной карты **B2 - telemetry gap ownership** требует опубликовать план, который связывает
каждый оставшийся телеметрический пробел Nexus с сигналом, guardrail оповещения, владельцем,
крайним сроком и артефактом проверки до начала окон аудита Q1 2026. Этот документ централизует
матрицу, чтобы release engineering, telemetry ops и владельцы SDK могли подтвердить покрытие перед
репетициями routed-trace и `TRACE-TELEMETRY-BRIDGE`.

# Матрица пробелов

| Gap ID | Сигнал и guardrail оповещения | Owner / Escalation | Срок (UTC) | Доказательства и проверка |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Гистограмма `torii_lane_admission_latency_seconds{lane_id,endpoint}` с алертом **`SoranetLaneAdmissionLatencyDegraded`**, который срабатывает, когда `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` в течение 5 минут (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (сигнал) + `@telemetry-ops` (алерт) - эскалация через on-call Nexus routed-trace. | 2026-02-23 | Алерт-тесты в `dashboards/alerts/tests/soranet_lane_rules.test.yml` и запись репетиции `TRACE-LANE-ROUTING`, показывающая срабатывание/восстановление алерта, плюс scrape Torii `/metrics`, архивированный в `docs/source/nexus_transition_notes.md`. |
| `GAP-TELEM-002` | Счетчик `nexus_config_diff_total{knob,profile}` с guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0`, блокирующим деплои (`docs/source/telemetry.md`). | `@nexus-core` (instrumentation) -> `@telemetry-ops` (алерт) - duty officer по governance уведомляется при неожиданном росте счетчика. | 2026-02-26 | Выводы governance dry-run хранятся рядом с `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; чеклист релиза включает скриншот запроса Prometheus и выдержку логов, подтверждающую, что `StateTelemetry::record_nexus_config_diff` отправил diff. |
| `GAP-TELEM-003` | Событие `TelemetryEvent::AuditOutcome` (метрика `nexus.audit.outcome`) с алертом **`NexusAuditOutcomeFailure`**, когда сбои или пропущенные результаты держатся >30 минут (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) с эскалацией на `@sec-observability`. | 2026-02-27 | CI gate `scripts/telemetry/check_nexus_audit_outcome.py` архивирует NDJSON payloads и падает, когда TRACE окно не содержит события успеха; скриншоты алертов прикладываются к отчету routed-trace. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` под наблюдением guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` (описано в `docs/source/telemetry.md`) и подпитывает checklist дежурного SRE. | `@telemetry-ops` (gauge/export) с эскалацией на `@nexus-core`, когда узлы сообщают несовпадающие размеры каталога. | 2026-02-28 | Тест телеметрии scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` подтверждает эмиссию; операторы прикладывают diff Prometheus и выдержку лога `StateTelemetry::set_nexus_catalogs` к пакету TRACE. |

# Бюджет экспорта и лимиты OTLP

- **Решение (2026-02-11):** ограничить OTLP экспортеры до **5 MiB/min на узел** или
  **25,000 spans/min**, что меньше, с размером батча 256 spans и таймаутом экспорта 10 секунд.
  Экспорт выше 80% лимита запускает алерт `NexusOtelExporterSaturated` в
  `dashboards/alerts/nexus_telemetry_rules.yml` и создает событие
  `telemetry_export_budget_saturation` для аудиторских логов.
- **Принудительное применение:** правила Prometheus читают счетчики `iroha.telemetry.export.bytes_total`
  и `iroha.telemetry.export.spans_total`; профиль OTLP collector поставляется с теми же лимитами
  по умолчанию, и конфиги на узлах не должны повышать их без waiver от governance.
- **Доказательства:** тестовые векторы алертов и утвержденные лимиты архивируются в
  `docs/source/nexus_transition_notes.md` вместе с аудиторскими артефактами routed-trace. Принятие
  B2 теперь считает бюджет экспорта закрытым.

# Операционный процесс

1. **Еженедельный triage.** Владельцы сообщают о прогрессе на Nexus readiness созвоне; блокеры и
   артефакты алерт-тестов фиксируются в `status.md`.
2. **Dry-run алертов.** Каждое правило алерта поставляется вместе с записью в
   `dashboards/alerts/tests/*.test.yml`, чтобы CI выполнял `promtool test rules` при изменении guardrail.
3. **Аудиторские доказательства.** Во время репетиций `TRACE-LANE-ROUTING` и `TRACE-TELEMETRY-BRIDGE`
   дежурный фиксирует результаты запросов Prometheus, историю алертов и выводы релевантных скриптов
   (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py`
   для коррелированных сигналов) и сохраняет их с артефактами routed-trace.
4. **Эскалация.** Если guardrail срабатывает вне репетиционного окна, ответственная команда заводит
   тикет инцидента Nexus со ссылкой на этот план, включая snapshot метрик и шаги по смягчению перед
   возобновлением аудитов.

С публикацией этой матрицы и ссылками на нее в `roadmap.md` и `status.md` пункт дорожной карты
**B2** теперь соответствует критериям приемки "ответственность, срок, алерт, проверка".
