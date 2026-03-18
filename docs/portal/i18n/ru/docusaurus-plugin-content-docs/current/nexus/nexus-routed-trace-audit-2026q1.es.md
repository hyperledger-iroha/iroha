---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-routed-trace-audit-2026q1
заголовок: Информирование аудитории о маршрутизации-трассировке, 1 квартал 2026 г. (B1)
описание: Espejo de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, que cubre los resultados de la триместральный пересмотр телеметрии.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::обратите внимание на Фуэнте каноника
Эта страница отражает `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Manten ambas copyas alineadas hasta que lleguen las las traducciones restantes.
:::

# Информационная аудитория Routed-Trace, 1 квартал 2026 г. (B1)

Для пункта дорожной карты **B1 — Базовый план аудита маршрутизации и телеметрии** требуется триместральная ревизия программы маршрутизированной трассировки Nexus. Этот информационный документ о выпуске зрительного зала за первый квартал 2026 г. (марта) для того, чтобы совет губернатора мог проверить состояние телеметрии перед съемками во втором квартале.

## Alcance и линия времени

| Идентификатор трассировки | Вентана (UTC) | Цель |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Проверьте гистограммы доступа к полосе, сплетни о колах и сигналы тревоги до того, как вы станете многополосным. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Действительное воспроизведение OTLP, сравнение ботов и получение телеметрии из SDK перед хитами AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Подтвердите изменения `iroha_config`, одобренные для управления и подготовки к откату перед кортом RC1. |

Он был создан в топологии типа производства с использованием инструментов с маршрутизированной трассировкой (телеметрия `nexus.audit.outcome` + контакты Prometheus), регистрация сообщений Alertmanager и экспортированные доказательства в `docs/examples/`.

## Методология

1. **Запись телеметрии.** Все узлы, излучающие событие, созданное `nexus.audit.outcome`, и сопутствующие метрики (`nexus_audit_outcome_total*`). Помощник `scripts/telemetry/check_nexus_audit_outcome.py` содержит хвост журнала JSON, проверяет состояние события и архивирует полезную нагрузку в `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Проверка оповещений.** `dashboards/alerts/nexus_audit_rules.yml` и ваш жгут средств защиты, обеспечивающий защиту зон от руин оповещений и шаблонов полезной нагрузки, которые соответствуют друг другу. CI выбросил `dashboards/alerts/tests/nexus_audit_rules.test.yml` в каждом камбио; las mismas reglas se ejercitaron manualmente durante cada ventana.
3. **Захват информационных панелей.** Операции экспорта панелей с маршрутизированной трассировкой `dashboards/grafana/soranet_sn16_handshake.json` (приветствие при рукопожатии) и общие информационные панели телеметрии для корреляции сигналов вызова с аудиторией.
4. **Примечания к изменениям.** Начальная регистрация изменений в секретаре правительства, решение и билеты по смягчению последствий в [Nexus примечания к переходу] (./nexus-transition-notes) и отслеживание изменений конфигурации (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Халлазгос| Идентификатор трассировки | Результат | Эвиденсия | Заметки |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Пройти | Запись оповещения о пожаре/восстановлении (внутренняя связь) + воспроизведение `dashboards/alerts/tests/soranet_lane_rules.test.yml`; зарегистрированные различия телеметрии в [Nexus примечания к переходу] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 подача колы происходит через 612 мс (объективно <=750 мс). Нет необходимости в дальнейшем. |
| `TRACE-TELEMETRY-BRIDGE` | Пройти | Полезная нагрузка архивируется `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json`, если хеш повтора OTLP зарегистрирован в `status.md`. | Соли редактирования SDK совпадают с базой Rust; el diff bot сообщает о дельтах. |
| `TRACE-CONFIG-DELTA` | Пройден (устранение последствий закрыто) | Ввод в правительственный трекер (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + манифест Perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + манифест пакета телеметрии (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Повторное выполнение Q2 было одобрено и подтверждено выполнением TLS; Манифест регистрации телеметрии для слотов 912–936 и начальное значение рабочей нагрузки `NEXUS-REH-2026Q2`. |

Все следы, продуцированные всеми событиями `nexus.audit.outcome` в день ветренных событий, удовлетворяют ограждениям Alertmanager (`NexusAuditOutcomeFailure`, если они остаются открытыми в течение триместра).

## Последующие действия

- Включите приложение Routed-Trace с хэшем TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; смягчение `NEXUS-421` находится в примечаниях к переходу.
- Продолжайте дополнительные повторы OTLP без обработки и артефактов различий Torii в архиве для восстановления доказательств паритета для версий Android AND4/AND7.
- Подтвердите, что проксимальные репетиции `TRACE-MULTILANE-CANARY` повторно используют помощник телеметрии для того, чтобы подписание Q2 было выгодным для проверки достоверности данных.

## Индекс артефактов

| Активо | Убикасьон |
|-------|----------|
| Проверка телеметрии | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Регламент и тесты оповещений | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Полезная нагрузка результата примера | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Трекер изменений конфигурации | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Расписание и заметки о маршрутизированной трассировке | [Nexus примечания к переходу](./nexus-transition-notes) |

Это информация о том, что предыдущие артефакты и экспорт оповещений/телеметрии должны быть добавлены в реестр решений губернатора для записи B1 триместра.