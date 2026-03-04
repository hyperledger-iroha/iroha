---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-routed-trace-audit-2026q1
title: Отчет аудита Routed-Trace за 1 квартал 2026 г. (B1)
описание: Зеркало `docs/source/nexus_routed_trace_audit_report_2026q1.md`, охватывающее итоги квартальных повторений телеметрии.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Канонический источник
На этой странице отражено `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Держите копии синхронизированными, пока не готовы остальные переводы.
:::

# Отчет аудита Routed-Trace за 1 квартал 2026 г. (B1)

Дорожная карта пункта **B1 — Базовый план аудита маршрутизации и телеметрии** требует квартального обзора программы Routed-Trace Nexus. Этот отчет фиксирует аудит аудита за первый квартал 2026 года (январь-март), чтобы совет по управлению мог указать телеметрическую готовность окна перед повторными запусками второго квартала.

## Область и таймлайн

| Идентификатор трассировки | Окно (UTC) | Цель |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Просматривайте гистограммы, допускающие полосу, сплетни очередей и оповещения о потоке перед включением многополосной сети. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Валидировать воспроизведение OTLP, парировать различия ботов и получать SDK телеметрии на устройствах AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Подтвердить управление-одобренные дельты `iroha_config` и готовность к откату перед срезом RC1. |

Каждое повторение проходит по прод-подобной топологии с включенной инструментализацией маршрутизации-трассировки (телеметрия `nexus.audit.outcome` + счетчики Prometheus), загруженными стандартами Alertmanager и экспортом доказательств в `docs/examples/`.

## Методология

1. **Сбор телеметрии.** Все узлы эмитировали структурированное событие `nexus.audit.outcome` и сопутствующие метрики (`nexus_audit_outcome_total*`). Хелпер `scripts/telemetry/check_nexus_audit_outcome.py` отслеживал JSON-лог, валидировал статус событий и архивировал полезную нагрузку в `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Проверка оповещений.** `dashboards/alerts/nexus_audit_rules.yml` и его тестовый жгут обеспечивают стабильность порогов шума и шаблонов полезной нагрузки. CI запускает `dashboards/alerts/tests/nexus_audit_rules.test.yml` при каждой поддержке; одни и те же правила вручную прогонялись в каждом окне.
3. **Съемка дашбордов.** Операторы экспортировали панель Routed-Trace из `dashboards/grafana/soranet_sn16_handshake.json` (здоровое рукопожатие) и обзорные дашборды телеметрии, чтобы связать здоровье очередей с результатами аудита.
4. **Заметки ревьюеров.** Секретарь по управлению записал основные ревьюеры, решения и инструкции по устранению последствий в [Nexus примечания к переходу](./nexus-transition-notes) и трекер конфигурационных дельт (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Результаты| Идентификатор трассировки | Итог | Доказательства | Примечания |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Пройти | Скриншоты пожара/восстановления алертов (внутренняя ссылка) + повтор `dashboards/alerts/tests/soranet_lane_rules.test.yml`; телеметрические дифы записаны в [Nexus примечания к переходу](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 прием очереди остался 612 мс (цель <=750 мс). Дальнейших действий не требуется. |
| `TRACE-TELEMETRY-BRIDGE` | Пройти | Архивированный полезный груз `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` плюс хеш воспроизведения OTLP, записанный в `status.md`. | Соли редактирования SDK совпали с базовой версией Rust; бот diff сообщил ноль дельт. |
| `TRACE-CONFIG-DELTA` | Пройден (устранение последствий закрыто) | Запись управления-трекера (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + манифест профиля TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + манифест пакета телеметрии (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Повтор второго квартала захешировал одобренный профиль TLS и подтвердил отсутствие оставшихся; Манифест телеметрии фиксирует диапазон слотов 912-936 и начальное значение рабочей нагрузки `NEXUS-REH-2026Q2`. |

Все следы выдали, хотя бы одно событие `nexus.audit.outcome` в пределах окон, которое удовлетворило ограждения Alertmanager (`NexusAuditOutcomeFailure` - очевидная устойчивость всего квартала).

## Последующие действия

- Дополнение Routed-Trace обновлено TLS хэшем `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; Устранение последствий `NEXUS-421` закрыто в примечаниях к переходу.
- Продолжайте прикладывать сырые повторы OTLP и артефакты Torii diff в архив, чтобы использовать паритетные доказательства для проверки Android AND4/AND7.
- Убедиться, что предстоящие повторения `TRACE-MULTILANE-CANARY` используют тот же телеметрический помощник, чтобы подписание второго квартала основывалось на проверенном рабочем процессе.

## Индекс документов

| Актив | Локация |
|-------|----------|
| Валидатор телеметрии | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Правила и тесты оповещений | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Пример полезной нагрузки результата | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Трекер конфиг-дельт | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| График и заметки Routed-Trace | [Nexus примечания к переходу](./nexus-transition-notes) |

Этот отчет, материалы выше и экспортные оповещения/телеметрии должны быть внесены в журнал решений управления, чтобы закрыть B1 за кварталом.