---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Nexus-Telemetry-Remediation
заголовок: План исправления телеметрии до Nexus (B2)
описание: Espelho de `docs/source/nexus_telemetry_remediation_plan.md`, документация матрицы пробелов телеметрии и рабочих потоков.
---

# Визао Герал

В пункте дорожной карты **B2 – владение пробелами в телеметрии** содержится план публикации, который поможет найти пробелы в телеметрии в Nexus на уровне, на ограждении оповещения, на ответе, на рабочем месте и в искусстве проверки перед началом работы с аудиторией в первом квартале. 2026. Эта страница выделена `docs/source/nexus_telemetry_remediation_plan.md` для разработки выпуска, операций телеметрии и владельцев SDK, подтверждающих cobertura antes dos ensaios Routed-Trace и `TRACE-TELEMETRY-BRIDGE`.

# Матрица лакун

| ID с пробелами | Синал и ограждение оповещения | Ответственность/эскалонаменто | Празо (UTC) | Доказательства и проверки |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Гистограмма `torii_lane_admission_latency_seconds{lane_id,endpoint}` с предупреждением **`SoranetLaneAdmissionLatencyDegraded`** исчезла, когда `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` за 5 минут (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (синал) + `@telemetry-ops` (алерта); Эскалонаменто через маршрутизированную трассировку по вызову до Nexus. | 2026-02-23 | Тесты оповещений в `dashboards/alerts/tests/soranet_lane_rules.test.yml` больше всего в регистре `TRACE-LANE-ROUTING` большинство предупреждений об удалении/восстановлении и очистке Torii `/metrics` архивируются в [Nexus переход примечания](./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` com ограждение `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` развертывается bloqueando (`docs/source/telemetry.md`). | `@nexus-core` (инструмент) -> `@telemetry-ops` (предупреждение); Официальное правительство и страница, когда или контадор увеличивает непреодолимую форму. | 26 февраля 2026 г. | Указания о пробном запуске правительственных армейских вооружений на уровне `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; контрольный список выпуска, включающий запись консультации Prometheus, в основном или третье место в журнале, подтверждающее, что `StateTelemetry::record_nexus_config_diff` излучает или различается. |
| `GAP-TELEM-003` | Даже `TelemetryEvent::AuditOutcome` (метрика `nexus.audit.outcome`) с предупреждением **`NexusAuditOutcomeFailure`**, когда ошибка или результат сохраняется в течение >30 минут (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (конвейер) с расширением для `@sec-observability`. | 2026-02-27 | Ворота CI `scripts/telemetry/check_nexus_audit_outcome.py` архивируют полезные данные NDJSON и отправляют сообщение TRACE в успешное событие; захваты оповещений, анексада или связанная трассировка маршрутизации. |
| `GAP-TELEM-004` | Манометр `nexus_lane_configured_total` с ограждением `nexus_lane_configured_total != EXPECTED_LANE_COUNT` включает в себя контрольный список по вызову SRE. | `@telemetry-ops` (датчик/экспорт) с расширением для `@nexus-core`, когда мы получаем несоответствующие отчеты в каталоге. | 28 февраля 2026 г. | Тест телеметрии планировщика `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` скомпрометирован; Операции анализа различий Prometheus + запись журнала `StateTelemetry::set_nexus_catalogs` в пакете TRACE. |

# Fluxo в рабочем состоянии1. **Триагем семанал.** Владельцы сообщают о ходе выполнения задания о готовности Nexus; блокировщики и артефакты тест-сигналов тревоги, зарегистрированные под `status.md`.
2. **Указания по оповещениям.** При включении оповещения и вступлении в контакт с входом `dashboards/alerts/tests/*.test.yml` для того, чтобы CI выполнил `promtool test rules`, когда оно было установлено на ограждении.
3. **Свидетельства аудитории.** Во время получения сообщений `TRACE-LANE-ROUTING` и `TRACE-TELEMETRY-BRIDGE` или записи результатов консультаций по вызову Prometheus, истории предупреждений и соответствующих сценариев (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` для корреляции) и как арразена с артефатос маршрутизированной трассировкой.
4. **Эскалонаменто.** Если ограждение несопоставимо с местом, где находится девушка, человек должен отвечать за билет об инциденте Nexus, ссылающийся на этот план, включая снимок метрики и проходы по смягчению последствий перед повторным просмотром в качестве аудиторий.

Как опубликованная матрица - и ссылки на `roadmap.md` и `status.md` - или пункт дорожной карты **B2**, агора соответствует критериям соблюдения "ответственности, ответственности, оповещения, проверки".