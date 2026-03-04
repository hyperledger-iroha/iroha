---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-routed-trace-audit-2026q1
title: تقرير تدقيقrouted-trace للربع Q1 2026 (B1)
описание: Создан для `docs/source/nexus_routed_trace_audit_report_2026q1.md` и предназначен для подключения к сети.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::примечание
Был установлен `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Он был убит в 2007 году.
:::

# تقرير تدقيق Routed-Trace, первый квартал 2026 г. (B1)

Выполните проверку **B1 — Аудит маршрутизированной трассировки и базовый уровень телеметрии** Выполните настройку маршрутизированной трассировки для Nexus. В 2026 году в 1 квартале 2026 года (Нигерия-Канада) будет объявлено о начале работы над проектом. Будет проведено исследование в рамках Q2.

## النطاق والجدول الزمني

| Идентификатор трассировки | США (UTC) | الهدف |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | В районе Хай-Лейн-лейн, в городе Сплетни, в городе-центре с несколькими полосами движения. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Для работы с OTLP, ботом diff и поддержкой SDK для AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Установите `iroha_config` в соответствии со стандартом RC1. |

Он был отправлен в США в рамках Routed-trace (приложение `nexus.audit.outcome`). + عدادات Prometheus) В Alertmanager вы можете установить `docs/examples/`.

## عرض المنهجية

1. **Отключено.** Установите флажок `nexus.audit.outcome`. (`nexus_audit_outcome_total*`). Создайте файл `scripts/telemetry/check_nexus_audit_outcome.py` для создания файла JSON для создания файловой системы. Это `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Запустите его.** Установите `dashboards/alerts/nexus_audit_rules.yml` и установите флажок Он сказал, что хочет сделать это. Код CI `dashboards/alerts/tests/nexus_audit_rules.test.yml`, установленный в приложении. Он был убит в 2017 году в Нью-Йорке.
3. **Запустите трассировку маршрутизации.** Для этого необходимо выполнить маршрутизированную трассировку в `dashboards/grafana/soranet_sn16_handshake.json` (протокол). (см.) التدقيق.
4. **Всё в порядке.** Создан в [Nexus примечания к переходу](./nexus-transition-notes) и добавлен в раздел (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## النتائج

| Идентификатор трассировки | Новости | دليل | الملاحظات |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Пройти | لقطات تنبيه fire/recover (رابط داخلي) + اعادة تشغيل `dashboards/alerts/tests/soranet_lane_rules.test.yml`; Нажмите на [Nexus примечания к переходу] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | Для P95 время задержки составляет 612 мс (время <= 750 мс). لا يوجد متابعة مطلوبة. |
| `TRACE-TELEMETRY-BRIDGE` | Пройти | Создан `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` для повторного воспроизведения OTLP для `status.md`. | Добавление солей Использование SDK и Rust الاساس؛ Создан бот для сравнения различий. |
| `TRACE-CONFIG-DELTA` | Пройден (устранение последствий закрыто) | Создайте файл манифеста (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + манифест в TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + файл манифеста для проверки (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | В 2-м квартале будет проведено тестирование TLS, которое будет завершено в ближайшее время. В манифесте 912-936 используется код `NEXUS-REH-2026Q2`. |Он прослеживает следы على الاقل حدثا واحدا `nexus.audit.outcome`, созданного Нэнси, Беном Уилсоном. Alertmanager (`NexusAuditOutcomeFailure`, установленный в приложении).

## المتابعات

- используется трассировка маршрутизации TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; В разделе `NEXUS-421` — примечания к переходу.
- Запустите программу OTLP для проверки различий в файле Torii. Доступно для Android AND4/AND7.
- Зарегистрировано в `TRACE-MULTILANE-CANARY` для получения дополнительной информации о том, как это сделать. Он был проведен во втором квартале на стадионе «Спорт-Сити».

## فهرس артефакты

| الاصل | الموقع |
|-------|----------|
| مدقق التليمتري | `scripts/telemetry/check_nexus_audit_outcome.py` |
| قواعد وتنبيهات الاختبار | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| مثال حمولة результат | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| متتبع فروقات الاعدادات | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Маршрутизация маршрутизированной трассировки | [Nexus примечания к переходу](./nexus-transition-notes) |

Он был назначен президентом и главой государства в 1999 году. Установите флажок B1 в нижней части экрана.