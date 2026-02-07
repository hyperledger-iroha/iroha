---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-routed-trace-audit-2026q1
כותרת: Отчет аудита מסלול מנותב עבור Q1 2026 (B1)
תיאור: Зеркало `docs/source/nexus_routed_trace_audit_report_2026q1.md`, охватывающее итоги квартальных репетиций телеметрии.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Канонический источник
Эта страница отражает `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Держите обе копии синхронизированными, пока не готовы остальные переводы.
:::

# Отчет аудита Routed-Trace עבור Q1 2026 (B1)

מפת הדרכים של Пункт **B1 - נתיב-מעקב אחר ביקורת וטלמטריה בסיסית** требует квартального обзора программы מנותב-עקבות Nexus. Этот отчет фиксирует окно аудита Q1 2026 (январь-март), чтобы совет по управлению мог утвердитеть телег перед репетициями запуска Q2.

## Область и таймлайн

| מזהה מעקב | Окно (UTC) | Цель |
|--------|-------------|--------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Проверить гистограммы допуска в lane, gossip очередей и поток алертов перед включением multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | שידור חוזר של OTLP, שיתוף בוט לחילופין ו-SDK של טלפונים ניידים עבור AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Подтвердить governance-одобренные deltas `iroha_config` и готовность к rollback перед срезом RC1. |

Каждая репетиция проходила на прод-подобной топологии с включенной מנותב-עקבות инструментализаци0тией (тели0етNIей6010101010101010101010101010110101010101010 счетчики Prometheus), загруженными правилами Alertmanager ו-эксPORTом доказательств в `docs/examples/`.

## Методология

1. **סביור טלמטרים.** Все узлы эмитировали структурированное событие `nexus.audit.outcome` ו сопутствующие (`nexus_audit_outcome_total*`). Хелпер `scripts/telemetry/check_nexus_audit_outcome.py` отслеживал JSON-LG, валидировал статус события архивировал מטען ב-`docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Проверка алертов.** `dashboards/alerts/nexus_audit_rules.yml` и его тестовый רתמה обеспечили стабильность порогов шума и шаблонов מטען. CI запускает `dashboards/alerts/tests/nexus_audit_rules.test.yml` при каждом изменении; те же правила вручную прогонялись в каждом окне.
3. **Sъемка дашбордов.** מסלולי אופטימיזציה של מסלולים מנותבים ב-`dashboards/grafana/soranet_sn16_handshake.json` (לחיצת יד מוזמנת) או ציוד אופטימלי, чтобы связать здоровье очередей с результатами аудита.
4. **Заметки ревьюеров.** Секретарь по управлению записал инициалы ревьюеров, решение и тикеты по108 הערות](./nexus-transition-notes) и трекер конфигурационных дельт (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Результаты| מזהה מעקב | Итог | Доказательства | Примечания |
|--------|--------|--------|-------|
| `TRACE-LANE-ROUTING` | לעבור | Скриншоты fire/recover алертов (внутренняя ссылка) + שידור חוזר `dashboards/alerts/tests/soranet_lane_rules.test.yml`; телеметрийные дифы зафиксированы в [Nexus הערות מעבר](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 приема очереди остался 612 ms (цель <=750 ms). Дальнейших действий не требуется. |
| `TRACE-TELEMETRY-BRIDGE` | לעבור | Архивированный מטען `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` плюс OTLP replay hash, записанный в `status.md`. | SDK redaction salts совпали с קו הבסיס של חלודה; diff bot сообщил ноль дельт. |
| `TRACE-CONFIG-DELTA` | מעבר (הקלה סגורה) | Запись governance-трекера (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + מניפסט פרופיל TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + מניפסט חבילת טלמטריה (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | שידור חוזר של Q2 захешировал одобренный TLS профиль ו подтвердил отсутствие отстающих; מניפסט טלמטריה фиксирует диапазон слотов 912-936 и seed עומס עבודה `NEXUS-REH-2026Q2`. |

Все traces выдали хотя бы одно событие `nexus.audit.outcome` в рамках окон, что удовлетворило מעקות בטיחות Alertmanager (I180а3вался зеленым весь квартал).

## מעקבים

- Дополнение מנותב-trace обновлено TLS хэшем `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; הקלה `NEXUS-421` закрыт в הערות מעבר.
- בדוק את השידורים החוזרים של OTLP וסרטים Torii הבדל בין ארצי, чтобы усилить паритель парит מוצר אנדרואיד AND4/AND7.
- Убедиться, что предстоящие репетиции `TRACE-MULTILANE-CANARY` используют тот же телеметрийный helper, sign-off Q2 זרימת עבודה проверенный.

## Индекс артефактов

| נכס | Локация |
|-------|--------|
| Валидатор телеметрии | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Правила и тесты алертов | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| מטען תוצאת Пример | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Трекер конфиг-дельт | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| График и заметки routed-trace | [הערות מעבר Nexus](./nexus-transition-notes) |

עשה את זה, אמנות וספורט אלטרנטים/טלמטרים שותפים לממשל השורנאלי, B1 за квартал.