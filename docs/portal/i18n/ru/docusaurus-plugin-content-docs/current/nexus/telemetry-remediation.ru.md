---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Nexus-Telemetry-Remediation
title: План ограничения пробелов телеметрии Nexus (B2)
описание: Зеркало `docs/source/nexus_telemetry_remediation_plan.md`, документирующая матрица пробелов телеметрии и операционный рабочий процесс.
---

# Обзор

Дорожная карта пункта **B2 – заголовок с пробелами телеметрии** опубликован требует плана, который привязывает каждый оставшийся пробел телеметрии Nexus к сигналу, защитному порогу оповещений, владельцу, дедлайну и проверкам продукции до начала окон аудита в первом квартале 2026 года. маршрутизированная трассировка и `TRACE-TELEMETRY-BRIDGE`.

# Матрица пробелов

| Идентификатор пробела | Сигнал и порог оповещения | Владелец / эскалация | Срок (UTC) | Доказательства и проверка |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Гистограмма `torii_lane_admission_latency_seconds{lane_id,endpoint}` с оповещением **`SoranetLaneAdmissionLatencyDegraded`**, разрабатываемым при `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` в течение 5 минут (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (сигнал) + `@telemetry-ops` (алерт); эскалация через маршрутизированную трассировку по вызову Nexus. | 2026-02-23 | Тесты оповещения в `dashboards/alerts/tests/soranet_lane_rules.test.yml` плюс повторы записи `TRACE-LANE-ROUTING` с оповещениями и восстановлениями и архивированный скрап Torii `/metrics` в [Nexus примечания к переходу](./nexus-transition-notes). |
| `GAP-TELEM-002` | Счетчик `nexus_config_diff_total{knob,profile}` с ограждением `increase(nexus_config_diff_total{profile="active"}[5m]) > 0`, блокирующим развертыванием (`docs/source/telemetry.md`). | `@nexus-core` (инструментирование) -> `@telemetry-ops` (предупреждение); дежурный по управлению пейджится при неожиданном росте счетчика. | 26 февраля 2026 г. | Результаты управления пробным прогоном располагаются рядом с `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; чеклист релиза включает скриншот запроса Prometheus и отрывок журналов, подтверждающий, что `StateTelemetry::record_nexus_config_diff` сгенерировал diff. |
| `GAP-TELEM-003` | Событие `TelemetryEvent::AuditOutcome` (метрика `nexus.audit.outcome`) с оповещением **`NexusAuditOutcomeFailure`** при сохранении ошибок или отсутствующих результатов более 30 минут (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (конвейер) с эскалацией в `@sec-observability`. | 2026-02-27 | CI-гейт `scripts/telemetry/check_nexus_audit_outcome.py` архивирует полезные данные NDJSON и падает, когда окно TRACE не содержит событий успеха; скриншоты оповещений применяются к отчету о маршрутизированной трассировке. |
| `GAP-TELEM-004` | Датчик `nexus_lane_configured_total` с ограждением `nexus_lane_configured_total != EXPECTED_LANE_COUNT`, который питает дежурный чеклист SRE. | `@telemetry-ops` (датчик/экспорт) с эскалацией в `@nexus-core`, когда узлы сообщают о несовпадающих размерах каталога. | 28 февраля 2026 г. | Тест телеметрии планировщика `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` подтверждает запрос; операторы приложили diff Prometheus + отрывок лога `StateTelemetry::set_nexus_catalogs` в пакет повторений TRACE. |

# Операционный рабочий процесс1. **Еженедельный триаж.** Владельцы отчитываются о прогрессе на Nexus готовности созвоне; блокеры и ссылки на тесты оповещений зафиксированы в `status.md`.
2. **Оповещения о пробном запуске.** Каждое правило оповещения применяется вместе с записью `dashboards/alerts/tests/*.test.yml`, чтобы CI запускал `promtool test rules` при наличии ограждения.
3. **Доказательства для аудита.** Во время повторов `TRACE-LANE-ROUTING` и `TRACE-TELEMETRY-BRIDGE` дежурный собирает результаты запросов Prometheus, историю оповещений и релевантные запросы скриптов (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` для коррелированных сигналов) и сохраняет их вместе с маршрутизированные-следовые документы.
4. **Эскалация.** Если ограждение реализовано вне повторяющегося окна, команда-владелец приводит к инциденту Nexus, ссылаясь на этот план, и прикладывает метрики моментальных снимков и шаги по снижению риска перед усилением аудита.

С опубликованной матрицей - и ссылками из `roadmap.md` и `status.md` - пункт дорожной карты **B2** теперь соответствует критериям приемки "ответственность, срок, алерт, проверка".