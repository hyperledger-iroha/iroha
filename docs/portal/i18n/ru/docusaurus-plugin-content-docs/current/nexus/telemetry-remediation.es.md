---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Nexus-Telemetry-Remediation
заголовок: План исправления телеметрии Nexus (B2)
описание: Espejo de `docs/source/nexus_telemetry_remediation_plan.md`, документация по матрице сигналов телеметрии и оперативному управлению.
---

# Общее резюме

Пункт дорожной карты **B2 — владение устройствами телеметрии** требует опубликованного плана, который позволит избежать прерывания телеметрии в ожидании Nexus с сигналом, ограждением оповещения, ответственностью, ограничением сбора и артефактом проверки перед тем, как начать работу в зрительных залах. в первом квартале 2026 г. На этой странице указано `docs/source/nexus_telemetry_remediation_plan.md` для разработки выпуска, операций телеметрии и ответственных за SDK, подтверждающих cobertura antes de los ensayos Routed-Trace и `TRACE-TELEMETRY-BRIDGE`.

# Матрис де Бречас

| ID де Бреча | Сенал и ограждение оповещения | Ответственный / эскаламиенто | Феча (UTC) | Доказательства и проверка |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Гистограмма `torii_lane_admission_latency_seconds{lane_id,endpoint}` с предупреждением **`SoranetLaneAdmissionLatencyDegraded`**, которая исчезла с `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` в течение 5 минут (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (сенал) + `@telemetry-ops` (алерта); эскаляр через вызов маршрутизированной трассировки Nexus. | 2026-02-23 | Срабатывание оповещений в `dashboards/alerts/tests/soranet_lane_rules.test.yml` после захвата объекта `TRACE-LANE-ROUTING` большинство предупреждений об удалении/восстановлении и очистке Torii `/metrics` архивируются в [Nexus переход примечания](./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` с ограждением `increase(nexus_config_diff_total{profile="active"}[5m]) > 0`, которое блокируется (`docs/source/telemetry.md`). | `@nexus-core` (инструментарий) -> `@telemetry-ops` (предупреждение); на странице официальной гвардии губернатора, когда контадор увеличил неожиданную форму. | 26 февраля 2026 г. | Салидас де-хола-де-гобернанса альмасенадас юнто `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; контрольный список выпуска включает запись консультации по Prometheus, а также извлечение журналов, которые нужно проверить, что `StateTelemetry::record_nexus_config_diff` излучает разницу. |
| `GAP-TELEM-003` | Даже `TelemetryEvent::AuditOutcome` (метрика `nexus.audit.outcome`) с предупреждением **`NexusAuditOutcomeFailure`**, когда сбои или сбои сохраняются в течение >30 минут (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (конвейер) с переходом на `@sec-observability`. | 2026-02-27 | Компьютер CI `scripts/telemetry/check_nexus_audit_outcome.py` архивирует полезные нагрузки NDJSON и попадает в момент выхода TRACE из события выхода; дополнительные захваты оповещений и отчеты о маршрутизированной трассировке. |
| `GAP-TELEM-004` | Манометр `nexus_lane_configured_total` с ограждением `nexus_lane_configured_total != EXPECTED_LANE_COUNT`, обеспечивающим питание контрольного списка по вызову SRE. | `@telemetry-ops` (датчик/экспорт) с расширением до `@nexus-core`, когда узлы отчета содержат несоответствующие каталоги. | 28 февраля 2026 г. | Телеметрия планировщика `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` проверяет излучение; дополнительные операции Prometheus diff + извлечение журнала `StateTelemetry::set_nexus_catalogs` из пакета TRACE. |

# Оперативное общение1. **Семанальная сортировка.** Владельцы сообщают о прогрессе в сообщении о готовности Nexus; блокировщики и артефакты срабатывания оповещений, зарегистрированные в `status.md`.
2. **Уведомления о предупреждениях.** Каждый раз, когда сигнал тревоги входит в систему с входом в `dashboards/alerts/tests/*.test.yml`, чтобы CI выбросил `promtool test rules`, когда он оказался на ограждении.
3. **Свидетельства аудитории.** Во время записи `TRACE-LANE-ROUTING` и `TRACE-TELEMETRY-BRIDGE` по вызову фиксируются результаты консультаций Prometheus, история предупреждений и соответствующие сценарии (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` для корреляционной проверки и обмена с артефактами маршрутизированной трассировки.
4. **Escalamiento.** Если ограждение находится в невыгодном положении, то оборудование, ответственное за инцидент, Nexus, которое содержит ссылку на этот план, включает снимок метрики и меры по смягчению последствий перед повторным просмотром аудиторий.

Con эта матрица опубликована - и ссылается на `roadmap.md` и `status.md` - пункт дорожной карты **B2** сейчас включает в себя критерии принятия "ответственность, ограниченное выполнение, предупреждение, проверка".