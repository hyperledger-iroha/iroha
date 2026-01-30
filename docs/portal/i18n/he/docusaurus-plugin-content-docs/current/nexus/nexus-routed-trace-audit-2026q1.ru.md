---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-routed-trace-audit-2026q1
title: Отчет аудита routed-trace за Q1 2026 (B1)
description: Зеркало `docs/source/nexus_routed_trace_audit_report_2026q1.md`, охватывающее итоги квартальных репетиций телеметрии.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Канонический источник
Эта страница отражает `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Держите обе копии синхронизированными, пока не готовы остальные переводы.
:::

# Отчет аудита Routed-Trace за Q1 2026 (B1)

Пункт roadmap **B1 - Routed-Trace Audits & Telemetry Baseline** требует квартального обзора программы routed-trace Nexus. Этот отчет фиксирует окно аудита Q1 2026 (январь-март), чтобы совет по управлению мог утвердить телеметрийную готовность перед репетициями запуска Q2.

## Область и таймлайн

| Trace ID | Окно (UTC) | Цель |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Проверить гистограммы допуска в lane, gossip очередей и поток алертов перед включением multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Валидировать OTLP replay, паритет diff bot и прием телеметрии SDK до вех AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Подтвердить governance-одобренные deltas `iroha_config` и готовность к rollback перед срезом RC1. |

Каждая репетиция проходила на прод-подобной топологии с включенной routed-trace инструментализацией (телеметрия `nexus.audit.outcome` + счетчики Prometheus), загруженными правилами Alertmanager и экспортом доказательств в `docs/examples/`.

## Методология

1. **Сбор телеметрии.** Все узлы эмитировали структурированное событие `nexus.audit.outcome` и сопутствующие метрики (`nexus_audit_outcome_total*`). Хелпер `scripts/telemetry/check_nexus_audit_outcome.py` отслеживал JSON-лог, валидировал статус события и архивировал payload в `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Проверка алертов.** `dashboards/alerts/nexus_audit_rules.yml` и его тестовый harness обеспечили стабильность порогов шума и шаблонов payload. CI запускает `dashboards/alerts/tests/nexus_audit_rules.test.yml` при каждом изменении; те же правила вручную прогонялись в каждом окне.
3. **Съемка дашбордов.** Операторы экспортировали панели routed-trace из `dashboards/grafana/soranet_sn16_handshake.json` (здоровье handshake) и обзорные дашборды телеметрии, чтобы связать здоровье очередей с результатами аудита.
4. **Заметки ревьюеров.** Секретарь по управлению записал инициалы ревьюеров, решение и тикеты по mitigations в [Nexus transition notes](./nexus-transition-notes) и трекер конфигурационных дельт (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Результаты

| Trace ID | Итог | Доказательства | Примечания |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Скриншоты fire/recover алертов (внутренняя ссылка) + replay `dashboards/alerts/tests/soranet_lane_rules.test.yml`; телеметрийные дифы зафиксированы в [Nexus transition notes](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 приема очереди остался 612 ms (цель <=750 ms). Дальнейших действий не требуется. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Архивированный payload `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` плюс OTLP replay hash, записанный в `status.md`. | SDK redaction salts совпали с Rust baseline; diff bot сообщил ноль дельт. |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | Запись governance-трекера (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS profile manifest (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + telemetry pack manifest (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Q2 rerun захешировал одобренный TLS профиль и подтвердил отсутствие отстающих; telemetry manifest фиксирует диапазон слотов 912-936 и workload seed `NEXUS-REH-2026Q2`. |

Все traces выдали хотя бы одно событие `nexus.audit.outcome` в рамках окон, что удовлетворило guardrails Alertmanager (`NexusAuditOutcomeFailure` оставался зеленым весь квартал).

## Follow-ups

- Дополнение routed-trace обновлено TLS хэшем `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; mitigation `NEXUS-421` закрыт в transition notes.
- Продолжать прикладывать сырые OTLP replays и артефакты Torii diff в архив, чтобы усилить паритетные доказательства для проверок Android AND4/AND7.
- Убедиться, что предстоящие репетиции `TRACE-MULTILANE-CANARY` используют тот же телеметрийный helper, чтобы Q2 sign-off опирался на проверенный workflow.

## Индекс артефактов

| Asset | Локация |
|-------|----------|
| Валидатор телеметрии | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Правила и тесты алертов | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Пример outcome payload | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Трекер конфиг-дельт | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| График и заметки routed-trace | [Nexus transition notes](./nexus-transition-notes) |

Этот отчет, артефакты выше и экспорты алертов/телеметрии должны быть приложены к журналу решений governance, чтобы закрыть B1 за квартал.
