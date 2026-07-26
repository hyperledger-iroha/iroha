---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Nexus-Telemetry-Remediation
заголовок: План устранения телеметрии Nexus (B2)
описание: Miroir de `docs/source/nexus_telemetry_remediation_plan.md`, документатор матрицы электронных карт телеметрии и операций потока.
---

# ансамбль

Элемент дорожной карты **B2 – владение электронными картами телеметрии** представляет собой общедоступный план, включающий оставшуюся электронную карту телеметрии Nexus, сигнал, ограждение оповещения, собственность, ограничение по дате и артефакт проверки перед дебютом окон аудита в первом квартале 2026 года. Эта страница Отметьте `docs/source/nexus_telemetry_remediation_plan.md` для разработки выпуска, операций телеметрии и владельцев SDK, чтобы получить мощное подтверждение la couverture перед повторениями маршрутизированной трассировки и `TRACE-TELEMETRY-BRIDGE`.

# Матрица электронных карт

| удостоверение личности | Сигнал и ограждение оповещения | Собственник / эскалада | Эчеанс (UTC) | Предварительные проверки и проверка |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Гистограмма `torii_lane_admission_latency_seconds{lane_id,endpoint}` с предупреждением **`SoranetLaneAdmissionLatencyDegraded`** разжата и `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` висит в течение 5 минут (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (сигнал) + `@telemetry-ops` (предупреждение); Эскалада через маршрутизированную трассировку по вызову Nexus. | 2026-02-23 | Тесты оповещения sous `dashboards/alerts/tests/soranet_lane_rules.test.yml` плюс захват повторения `TRACE-LANE-ROUTING` montrant l'alerte declenchee/retable et le Scrape Torii `/metrics` архив в [Nexus переход примечания](./nexus-transition-notes). |
| `GAP-TELEM-002` | Compteur `nexus_config_diff_total{knob,profile}` с ограждением `increase(nexus_config_diff_total{profile="active"}[5m]) > 0`, блокирующим развертывание (`docs/source/telemetry.md`). | `@nexus-core` (приборы) -> `@telemetry-ops` (предупреждение); l'Officier de Garde Governance est page lorsque le compteur augmente de maniere unattendue. | 26 февраля 2026 г. | Выезды на пробные управленческие склады в Кот-де-`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; контрольный список выпуска, включая захват запроса Prometheus, а также экстракты журналов, которые можно получить `StateTelemetry::record_nexus_config_diff` в поле diff. |
| `GAP-TELEM-003` | Вечер `TelemetryEvent::AuditOutcome` (метрический `nexus.audit.outcome`) с предупреждением **`NexusAuditOutcomeFailure`** для получения результатов или результатов, которые сохраняются более 30 минут (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (конвейер) с расширенными версиями `@sec-observability`. | 2026-02-27 | Архив полезных нагрузок NDJSON CI `scripts/telemetry/check_nexus_audit_outcome.py` и эхо-сигнал в окне TRACE не содержат никакого успеха; захватывает соединения по маршрутизированной трассировке. |
| `GAP-TELEM-004` | Манометр `nexus_lane_configured_total` с ограждением `nexus_lane_configured_total != EXPECTED_LANE_COUNT` включает контрольный список SRE по вызову. | `@telemetry-ops` (калибровка/экспорт) с расширенными версиями `@nexus-core` с сигнальными сигналами о несвязных каталогах. | 28 февраля 2026 г. | Тест телеметрии планировщика `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` подтверждает излучение; Совместные операторы Prometheus + экстракция журнала `StateTelemetry::set_nexus_catalogs` или пакет повторения TRACE. |

# Операции с потоком1. **Triage hebdomadaire.** Доклад владельцев по запросу о готовности Nexus; Блокировки и артефакты тестов по тревоге отправляются в `status.md`.
2. **Тестирование оповещения.** Каждый контрольный сигнал находится в режиме ожидания с входом `dashboards/alerts/tests/*.test.yml` до того, как CI выполнит `promtool test rules`, когда будет развиваться ограждение.
3. **Preuves d'audit.** Отслеживание повторений `TRACE-LANE-ROUTING` и `TRACE-TELEMETRY-BRIDGE`, захват результатов запросов Prometheus по вызову, история предупреждений и соответствующих сценариев (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` для коррелирования сигналов) и запасов с артефактами маршрутизированной трассировки.
4. **Эскалада.** Если ограждение опущено после повторения отверстия, собственность владельца оборудования может получить билет на инцидент Nexus со ссылкой на этот план, включая снимок показателей и этапы смягчения последствий перед повторными проверками.

Опубликована эта матрица - и ссылки от `roadmap.md` и `status.md` - пункт дорожной карты **B2**, удовлетворяющий критериям принятия "ответственность, устранение, предупреждение, проверка".