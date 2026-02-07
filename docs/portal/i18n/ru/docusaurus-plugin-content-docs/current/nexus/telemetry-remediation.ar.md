---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Nexus-Telemetry-Remediation
заголовок: خطة معالجة تيليمترية Nexus (B2)
описание: Создан для `docs/source/nexus_telemetry_remediation_plan.md` и предназначен для работы в режиме реального времени.
---

# نظرة عامة

عنصر خارطة الطريق **B2 - ملكية فجوات القياس** يتطلب خطة منشورة تربط كل Он был создан для Nexus. Началось в первом квартале 2026 года. Дата публикации: `docs/source/nexus_telemetry_remediation_plan.md` Выполните настройку и использование SDK для создания маршрутизированной трассировки и трассировки. `TRACE-TELEMETRY-BRIDGE`.

# مصفوفة الفجوات

| معرف الفجوة | Информационные технологии | المالك / التصعيد | الاستحقاق (UTC) | دليل والتحقق |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Установите `torii_lane_admission_latency_seconds{lane_id,endpoint}` на **`SoranetLaneAdmissionLatencyDegraded`** на `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` на 5 дней. (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (اشارة) + `@telemetry-ops` (تنبيه)؛ Для вызова маршрутизированной трассировки вызовов используется Nexus. | 2026-02-23 | Установите флажок `dashboards/alerts/tests/soranet_lane_rules.test.yml` в исходном состоянии `TRACE-LANE-ROUTING`. Для этого выполните очистку Torii `/metrics` в [Nexus примечания к переходу](./nexus-transition-notes). |
| `GAP-TELEM-002` | Был установлен `nexus_config_diff_total{knob,profile}` на `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` с установленным модулем (`docs/source/telemetry.md`). | `@nexus-core` (Уведомление) -> `@telemetry-ops` (Уведомление); Он был убит в 2007 году в Нью-Йорке, где его пригласили в Нью-Йорк. | 26 февраля 2026 г. | Прогон вхолостую с двигателем `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; Установите флажок Prometheus, чтобы установить `StateTelemetry::record_nexus_config_diff`. اصدر الفارق. |
| `GAP-TELEM-003` | Код `TelemetryEvent::AuditOutcome` (заголовок `nexus.audit.outcome`) для **`NexusAuditOutcomeFailure`**. Включено 30 дней назад (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (конвейер) используется для `@sec-observability`. | 2026-02-27 | Загрузить CI `scripts/telemetry/check_nexus_audit_outcome.py` для полезных нагрузок NDJSON и получить информацию о TRACE для проверки. Для этого необходимо выполнить маршрутизированную трассировку. |
| `GAP-TELEM-004` | Манометр `nexus_lane_configured_total` установлен на `nexus_lane_configured_total != EXPECTED_LANE_COUNT`, и его необходимо установить в SRE. | `@telemetry-ops` (датчик/экспорт) можно загрузить с `@nexus-core`, чтобы получить дополнительную информацию о себе. متناسقة. | 28 февраля 2026 г. | Установите флажок `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size`. Для этого используйте Prometheus + введите `StateTelemetry::set_nexus_catalogs`, чтобы просмотреть TRACE. |

# سير العمل التشغيلي1. **Запись.** Создан в соответствии с кодом Nexus; Он был создан в соответствии с `status.md`.
2. **Установите код.** Установите флажок CI в `dashboards/alerts/tests/*.test.yml`. `promtool test rules` был установлен.
3. **Запустите.** Установите `TRACE-LANE-ROUTING` и `TRACE-TELEMETRY-BRIDGE`, чтобы установить их. Установите Prometheus и установите флажок `scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` للاشارات المترابطة) и выполните маршрутизированную трассировку.
4. **Удар.** Он сказал, что его отец и сын хотят, чтобы он сделал это. Установите Nexus для получения дополнительной информации о том, как это сделать. Он сказал, что это не так.

Для получения дополнительной информации о `roadmap.md` и `status.md` — дорожная карта **B2** Он написал "Старый мир", "Старый мир".