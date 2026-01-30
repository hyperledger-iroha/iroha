---
lang: he
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-telemetry-remediation
title: План устранения пробелов телеметрии Nexus (B2)
description: Зеркало `docs/source/nexus_telemetry_remediation_plan.md`, документирующее матрицу пробелов телеметрии и операционный рабочий процесс.
---

# Обзор

Пункт roadmap **B2 - владение пробелами телеметрии** требует опубликованного плана, который привязывает каждый оставшийся пробел телеметрии Nexus к сигналу, защитному порогу оповещений, владельцу, дедлайну и артефакту проверки до начала окон аудита Q1 2026. Эта страница отражает `docs/source/nexus_telemetry_remediation_plan.md`, чтобы release engineering, telemetry ops и владельцы SDK могли подтвердить покрытие перед репетициями routed-trace и `TRACE-TELEMETRY-BRIDGE`.

# Матрица пробелов

| Gap ID | Сигнал и защитный порог оповещения | Владелец / эскалация | Срок (UTC) | Доказательства и проверка |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Гистограмма `torii_lane_admission_latency_seconds{lane_id,endpoint}` с алертом **`SoranetLaneAdmissionLatencyDegraded`**, срабатывающим когда `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` в течение 5 минут (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (сигнал) + `@telemetry-ops` (алерт); эскалация через on-call routed-trace Nexus. | 2026-02-23 | Тесты алерта в `dashboards/alerts/tests/soranet_lane_rules.test.yml` плюс запись репетиции `TRACE-LANE-ROUTING` с алертом и восстановлением и архивированный scrape Torii `/metrics` в [Nexus transition notes](./nexus-transition-notes). |
| `GAP-TELEM-002` | Счетчик `nexus_config_diff_total{knob,profile}` с guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0`, блокирующим деплой (`docs/source/telemetry.md`). | `@nexus-core` (инструментирование) -> `@telemetry-ops` (алерт); дежурный по governance пейджится при неожиданном росте счетчика. | 2026-02-26 | Выходы governance dry-run сохраняются рядом с `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; чеклист релиза включает скриншот запроса Prometheus и отрывок логов, подтверждающий, что `StateTelemetry::record_nexus_config_diff` сгенерировал diff. |
| `GAP-TELEM-003` | Событие `TelemetryEvent::AuditOutcome` (метрика `nexus.audit.outcome`) с алертом **`NexusAuditOutcomeFailure`** при сохранении ошибок или отсутствующих результатов более 30 минут (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) с эскалацией в `@sec-observability`. | 2026-02-27 | CI-гейт `scripts/telemetry/check_nexus_audit_outcome.py` архивирует NDJSON payloads и падает, когда окно TRACE не содержит события успеха; скриншоты алертов прикладываются к routed-trace отчету. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` с guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT`, который питает on-call чеклист SRE. | `@telemetry-ops` (gauge/export) с эскалацией в `@nexus-core`, когда узлы сообщают о несовпадающих размерах каталога. | 2026-02-28 | Тест телеметрии планировщика `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` подтверждает эмиссию; операторы прикладывают diff Prometheus + отрывок лога `StateTelemetry::set_nexus_catalogs` в пакет репетиции TRACE. |

# Операционный рабочий процесс

1. **Еженедельный триаж.** Владельцы отчитываются о прогрессе на Nexus readiness созвоне; блокеры и артефакты тестов алертов фиксируются в `status.md`.
2. **Dry-run алертов.** Каждое правило алерта поставляется вместе с записью `dashboards/alerts/tests/*.test.yml`, чтобы CI запускал `promtool test rules` при изменении guardrail.
3. **Доказательства для аудита.** Во время репетиций `TRACE-LANE-ROUTING` и `TRACE-TELEMETRY-BRIDGE` дежурный собирает результаты запросов Prometheus, историю алертов и релевантные выводы скриптов (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` для коррелированных сигналов) и сохраняет их вместе с routed-trace артефактами.
4. **Эскалация.** Если guardrail срабатывает вне репетиционного окна, команда-владелец открывает Nexus инцидент, ссылаясь на этот план, и прикладывает snapshot метрики и шаги по снижению риска перед возобновлением аудитов.

С опубликованной матрицей - и ссылками из `roadmap.md` и `status.md` - пункт roadmap **B2** теперь соответствует критериям приемки "ответственность, срок, алерт, проверка".
