---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: nexus-routed-trace-audit-2026q1
название: 2026 Q1 Routed-Trace آڈٹ رپورٹ (B1)
описание: `docs/source/nexus_routed_trace_audit_report_2026q1.md` کا آئینہ، جو سہ ماہی ٹیلیمیٹری ریہرسل کے نتائج کا احاطہ کرتا ہے۔
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::примечание
یہ صفحہ `docs/source/nexus_routed_trace_audit_report_2026q1.md` کی عکاسی کرتا ہے۔ باقی تراجم آنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# 2026 Q1 Routed-Trace آڈٹ رپورٹ (B1)

**B1 — Аудит маршрутизированной трассировки и базовый уровень телеметрии** Nexus Routed-Trace کرتا ہے۔ یہ رپورٹ Q1 2026 (جنوری-مارچ) کی کی گورننس В 2-м квартале будет готово 2-е место, где будет проходить обучение.

## دائرہ کار اور ٹائم لائن

| Идентификатор трассировки | Нью-Йорк (UTC) | مقصد |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | многополосное управление, гистограммы доступа к полосам, очередь сплетен и поток оповещений. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Этапы AND4/AND7, воспроизведение OTLP, проверка четности ботов и прием телеметрии SDK. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | RC1 вырезал سے پہلے, одобренный правительством `iroha_config` deltas и готовность к откату کی تصدیق۔ |

Топология, подобная производственной, и инструментарий маршрутизированной трассировки, необходимые функции (телеметрия `nexus.audit.outcome` + счетчики Prometheus), правила Alertmanager. Найдите доказательства `docs/examples/` میں ایکسپورٹ ہوا۔

## طریقہ کار

1. ** ٹیلیمیٹری کلیکشن۔** تمام نوڈز نے структурированные метрики `nexus.audit.outcome` ایونٹ اور متعلقہ (`nexus_audit_outcome_total*`) излучает کیں۔ helper `scripts/telemetry/check_nexus_audit_outcome.py` и JSON log Tail для создания полезной нагрузки для `docs/examples/nexus_audit_outcomes/` آرکائیو کیا۔ [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Обозначение** `dashboards/alerts/nexus_audit_rules.yml` Используется для проверки тестового оборудования, определения пороговых значений шума предупреждений и шаблонов полезной нагрузки. مسلسل رہیں۔ CI ہر تبدیلی پر `dashboards/alerts/tests/nexus_audit_rules.test.yml` چلاتا ہے؛ یہی رولز ہر ونڈو میں دستی طور پر بھی چلائے گئے۔
3. **Дополнительно** Используйте `dashboards/grafana/soranet_sn16_handshake.json` (состояние рукопожатия), панели маршрутизированной трассировки и информационные панели обзора телеметрии. Проверьте состояние очереди и результаты аудита. Проверьте корреляцию.
4. **ریویو نوٹس۔** گورننس سیکرٹری نے инициалы рецензента, فیصلہ اور mitigation Tickets کو [Nexus переход примечания](./nexus-transition-notes) Конфигурация дельта-трекера (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`)

## نتائج| Идентификатор трассировки | نتیجہ | ثبوت | نوٹس |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Пройти | Оповещение о пожаре/восстановлении اسکرین شاٹس (اندرونی لنک) + повтор `dashboards/alerts/tests/soranet_lane_rules.test.yml`; различия телеметрии [Nexus примечания к переходу](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) میں ریکارڈ۔ | Вход в очередь P95 612 мс в течение (ہدف <=750 мс)۔ فالو اپ درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | Пройти | Полезная нагрузка архивированного результата `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` и хэш воспроизведения OTLP `status.md` میں ریکارڈ۔ | Соли редактирования SDK Базовая версия Rust سے match تھے؛ diff bot и нулевая дельта |
| `TRACE-CONFIG-DELTA` | Пройден (устранение последствий закрыто) | Запись отслеживания управления (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + манифест профиля TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + манифест пакета телеметрии (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Повторный запуск Q2 в случае хэш-профиля TLS с нулевым отставанием или отсутствием отстающих. Манифест телеметрии, слоты 912-936 и начальное значение рабочей нагрузки `NEXUS-REH-2026Q2`. |

Следы трассировок могут быть найдены в `nexus.audit.outcome`, где можно найти Ограждения Alertmanager پورے ہوئے (`NexusAuditOutcomeFailure` پورے کوارٹر میں گرین رہا)۔

## Последующие действия

- Приложение трассировки маршрутизации с хэшем TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`, который может быть установлен или отключен. смягчение последствий `NEXUS-421` примечания к переходу
- Android AND4/AND7 проверяет наличие доказательств четности и необработанных OTLP-повторов, а также артефакты различий Torii и другие возможности. منسلک کرتے رہیں۔
- تصدیق کریں کہ آنے والی `TRACE-MULTILANE-CANARY` репетиции и помощник по телеметрии دوبارہ استعمال کریں تاکہ Подтверждение второго квартала Рабочий процесс سے فائدہ ٹھائے۔

## Артефакт انڈیکس

| Актив | مقام |
|-------|----------|
| Валидатор телеметрии | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Правила оповещений и тесты | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Пример полезной нагрузки результата | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Конфигурация дельта-трекера | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Расписание маршрутизации и примечания | [Nexus примечания к переходу](./nexus-transition-notes) |

Можно создавать артефакты, экспортировать оповещения/телеметрию и журнал решений управления, а также создавать и использовать эти артефакты. تاکہ اس کوارٹر کے لئے B1 بند ہو جائے۔