---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-routed-trace-audit-2026q1
Название: Relatorio de Auditorium Routed-Trace 2026 Q1 (B1)
описание: Espelho de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, объединяет триместры результатов пересмотров телеметрии.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::примечание Fonte canonica
Эта страница отражает `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Мантенья, как дуас копиас алинхадас, ел то, что переводил до конца.
:::

№ Аудиторского отчета Routed-Trace, 1 квартал 2026 г. (B1)

Пункт «Дорожная карта» **B1 — Аудит маршрутизированной трассировки и базовые показатели телеметрии** требует триместральной проверки программы маршрутизированной трассировки для Nexus. Это документальный документ, посвященный первому кварталу 2026 года (Жанейро-Марко), чтобы совет управления мог утвердить положение телеметрии до начала второго квартала.

## Эскопо и хронограмма

| Идентификатор трассировки | Джанела (UTC) | Цель |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Проверяйте гистограммы допуска к полосе движения, сплетни о файлах и потоки предупреждений перед использованием нескольких полос. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Проверяйте воспроизведение OTLP, сравнивайте боты и получайте данные телеметрии из SDK перед марками AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Подтвердите изменения `iroha_config`, подтверждающие управление и запуск отката перед кортом RC1. |

Когда вы создаете топологию самостоятельно и производите с помощью инструмента маршрутизированной трассировки (телеметрия `nexus.audit.outcome` + контакты Prometheus), проверяйте в Alertmanager запросы и экспортируйте доказательства для `docs/examples/`.

## Методология

1. **Сбор телеметрии.** Все исходящие данные или события, созданные `nexus.audit.outcome` и как ассоциированные метрики (`nexus_audit_outcome_total*`). Помощник `scripts/telemetry/check_nexus_audit_outcome.py` позволяет записывать журнал JSON, проверять статус события и архивировать полезную нагрузку в `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Проверка предупреждений.** `dashboards/alerts/nexus_audit_rules.yml` и это гарантия того, что границы руида и шаблоны обеспечивают постоянную согласованность полезной нагрузки. O CI executa `dashboards/alerts/tests/nexus_audit_rules.test.yml` каждый раз; как mesmas regras foram exercitadas manualmente durante cada janela.
3. **Захват информационных панелей.** Операции экспорта данных с маршрутизированной трассировкой `dashboards/grafana/soranet_sn16_handshake.json` (запись рукопожатия) и общие информационные панели телеметрии для корреляции с файлами с результатами аудитории.
4. **Notas de revisao.** Секретарь правительственной регистрации инициирует исправления, решения и билеты по смягчению изменений в [Nexus примечания к переходу] (./nexus-transition-notes) и не отслеживает изменения конфигурации (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Ахадос| Идентификатор трассировки | Результат | Эвиденсия | Заметки |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Пройти | Запись оповещения о пожаре/восстановлении (внутренняя ссылка) + воспроизведение `dashboards/alerts/tests/soranet_lane_rules.test.yml`; зарегистрированные различия телеметрии в [Nexus примечания к переходу] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 постоянный доступ к файлу через 612 мс (всего <= 750 мс). Последующее наблюдение за Сэмом. |
| `TRACE-TELEMETRY-BRIDGE` | Пройти | Полезная нагрузка архивируется `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json`, а затем хэш повтора OTLP, зарегистрированный в `status.md`. | Редактирование в SDK с базой Rust; o diff-бот сообщает о нулевых дельтах. |
| `TRACE-CONFIG-DELTA` | Пройден (устранение последствий закрыто) | Ввод без отслеживания управления (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + манифест Perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + манифест пакета телеметрии (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Повторный запуск Q2 должен подтвердить одобрение TLS и подтвердить отсутствие отстающих; o манифест регистрации телеметрии или интервал слотов 912–936 и o начальное значение рабочей нагрузки `NEXUS-REH-2026Q2`. |

Все следы, производимые в ходе события `nexus.audit.outcome`, в день их уборки, выполняются в Alertmanager (`NexusAuditOutcomeFailure` постоянно в течение триместра).

## Последующие действия

- В приложении маршрутизированная трассировка, настроенная с помощью хэша TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; чтобы уменьшить `NEXUS-421`, чтобы добавить примечания к переходу.
- Продолжайте повторять OTLP-грубия и артефакты различий до Torii в архиве для восстановления доказательств честности для пересмотра Android AND4/AND7.
- Подтвердите, что в ходе проксимальных репетиций `TRACE-MULTILANE-CANARY` повторно используется помощник телеметрии для завершения Q2, когда бенефициар проверяет рабочий процесс.

## Индекс артефатоса

| Ативо | Местный |
|-------|----------|
| Проверка телеметрии | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Проверка и тесты оповещения | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Полезная нагрузка результата примера | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Трекер изменений конфигурации | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Хронограмма и заметки маршрутизированной трассировки | [Nexus примечания к переходу](./nexus-transition-notes) |

В этой связи, наши артефакты и экспорт оповещений/телеметрии должны быть добавлены к журналу решений управления для отчета или B1 в триместре.