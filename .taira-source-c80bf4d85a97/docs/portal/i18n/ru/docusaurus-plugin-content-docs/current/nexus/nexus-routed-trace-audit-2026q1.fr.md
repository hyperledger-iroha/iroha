---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-routed-trace-audit-2026q1
заголовок: Отчет об аудите маршрутизации-трассировки, 1 квартал 2026 г. (B1)
описание: Miroir de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, учитывает триместры повторений телеметрии.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Источник канонический
На этой странице указано `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Gardez les deux copys alignees jusqu'a ce que les traductions restantes прибыли.
:::

# Отчет об аудите Routed-Trace, 1 квартал 2026 г. (B1)

Пункт дорожной карты **B1 — Базовые показатели аудита маршрутизированной трассировки и телеметрии** содержит триместральный обзор программы маршрутизированной трассировки Nexus. Этот отчет документирует окончание аудита за первый квартал 2026 года (январь-март) для того, чтобы совет по управлению мог подтвердить положение телеметрии перед повторением проколов во втором квартале.

## Портье и календарь

| Идентификатор трассировки | Фенетре (UTC) | Объектиф |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Проверка гистограмм допусков к полосам, сплетен о файлах и потоков оповещений перед многополосной активацией. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Подтвердите воспроизведение OTLP, разделите различия между ботами и приемом телеметрических данных SDK перед вызовами AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Подтверждение отклонений `iroha_config` подтверждает управление и подготовку к откату перед отключением RC1. |

Повторите тур по топологии процесса производства с активной трассировкой инструментов (телеметрия `nexus.audit.outcome` + компьютеры Prometheus), правила Alertmanager, взимаемые и защищенные от экспорта в `docs/examples/`.

## Методология

1. **Сбор телеметрии.** Все данные включены в структуру `nexus.audit.outcome` и ассоциированные метрики (`nexus_audit_outcome_total*`). Помощник `scripts/telemetry/check_nexus_audit_outcome.py` поддерживает журнал JSON, проверяет статус мероприятия и архивирует полезную нагрузку с помощью `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Проверка предупреждений.** `dashboards/alerts/nexus_audit_rules.yml` и его использование для проверки, чтобы убедиться в том, что сигналы тревоги и шаблоны полезных нагрузок сохраняют когерентность. CI выполняет `dashboards/alerts/tests/nexus_audit_rules.test.yml` модификацию чака; les memes regles ont ete упражняется в кулоне с кулоном chaque fenetre.
3. **Захват информационных панелей.** Операторы экспортируют панорамы маршрутизированной трассировки от `dashboards/grafana/soranet_sn16_handshake.json` (санте-квитирование) и глобальные информационные панели телеметрии для коррелирования файлов с результатами аудита.
4. **Примечания для респондентов.** Секретарь управления передает инициалы, решения и билеты по смягчению последствий в [Nexus примечания к переходу] (./nexus-transition-notes) и в трекере изменений конфигурации (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Констатации| Идентификатор трассировки | Результат | Преве | Заметки |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Пройти | Захватывает оповещение о пожаре/восстановлении (внутреннее удержание) + воспроизведение `dashboards/alerts/tests/soranet_lane_rules.test.yml`; Различия телеметрии регистрируются в [Nexus примечания к переходу] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | Время доступа к файлу P95 составляет 612 мс (кабель <= 750 мс). Aucun suivi requis. |
| `TRACE-TELEMETRY-BRIDGE` | Пройти | Архив полезной нагрузки `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` плюс хэш воспроизведения OTLP регистрируется в `status.md`. | Корреспондент SDK Lesss de Redaction на базе Rust; le diff bot сигнализирует нулевую дельту. |
| `TRACE-CONFIG-DELTA` | Пройден (устранение последствий закрыто) | Вход в трекер управления (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + профиль TLS манифеста (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + манифест телеметрии пакета (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Повторно запустите Q2 и хеш-профиль TLS одобрит и подтвердит отсутствие задержек; Манифест телеметрии зарегистрируете пляж слотов 912-936 и начальное значение рабочей нагрузки `NEXUS-REH-2026Q2`. |

Все следы производятся на вечере `nexus.audit.outcome` в окнах, удовлетворяющих ограждениям Alertmanager (`NexusAuditOutcomeFailure` остается верным в течение триместра).

## Суивис

- Приложение Routed-trace a Ete Mis a Jour с хешем TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; Смягчение `NEXUS-421` является фермой в переходных примечаниях.
- Продолжение объединения повторов OTLP bruts и артефактов различий Torii в архиве для восстановления паритета для обзоров Android AND4/AND7.
- Подтверждение того, что прошедшие репетиции `TRACE-MULTILANE-CANARY` повторно используют помощник телеметрии мемов для проверки Q2, выгодной для рабочего процесса.

## Индекс артефактов

| Актив | Размещение |
|-------|----------|
| Валидатор телеметрии | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Правила и тесты на бдительность | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Пример результата полезной нагрузки | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Трекер изменений конфигурации | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Планирование и заметки маршрутизированной трассировки | [Nexus примечания к переходу](./nexus-transition-notes) |

В этом взаимопонимании артефакты ci-dessus и экспортные сигналы/телеметрия doivent etre прикрепляются к журналу принятия решений по управлению для завершения B1 триместра.