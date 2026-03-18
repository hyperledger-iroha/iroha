<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ru
direction: ltr
source: docs/source/nexus_routed_trace_audit_report_2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b77d8021c6e09ba132ba080f183b532b35f8f6293a13497646566d13e932306
source_last_modified: "2025-11-22T12:03:01.494516+00:00"
translation_last_reviewed: 2026-01-01
---

# Отчет аудита Routed-Trace за 2026 Q1 (B1)

Пункт дорожной карты **B1 — Routed-Trace Audits & Telemetry Baseline** требует
квартального обзора программы routed-trace в Nexus. Этот отчет фиксирует
аудиторское окно Q1 2026 (январь–март), чтобы совет по управлению мог утвердить
состояние телеметрии перед репетициями запуска Q2.

## Область и график

| Trace ID | Окно (UTC) | Цель |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | Проверить гистограммы admission по lanes, gossip очередей и поток алертов перед включением multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | Валидировать OTLP replay, паритет diff bot и ingest телеметрии SDK перед вехами AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | Подтвердить одобренные governance delta `iroha_config` и готовность rollback перед срезом RC1. |

Каждая репетиция проходила на топологии, близкой к production, с включенной
инструментацией routed-trace (телеметрия `nexus.audit.outcome` + счетчики
Prometheus), загруженными правилами Alertmanager и экспортом доказательств в
`docs/examples/`.

## Методология

1. **Сбор телеметрии.** Все узлы эмитировали структурированное событие
   `nexus.audit.outcome` и сопровождающие метрики (`nexus_audit_outcome_total*`). Хелпер
   `scripts/telemetry/check_nexus_audit_outcome.py` парсил JSON лог, валидировал статус события
   и архивировал payload в `docs/examples/nexus_audit_outcomes/`
   (`scripts/telemetry/check_nexus_audit_outcome.py:1`).
2. **Проверка алертов.** `dashboards/alerts/nexus_audit_rules.yml` и его тестовый harness
   гарантировали стабильность порогов шума и шаблонов payload. CI запускает
   `dashboards/alerts/tests/nexus_audit_rules.test.yml` на каждое изменение; те же правила
   вручную прогонялись в каждом окне.
3. **Снимки дашбордов.** Операторы экспортировали панели routed-trace из
   `dashboards/grafana/soranet_sn16_handshake.json` (health handshake) и обзорные дашборды
   телеметрии, чтобы связать здоровье очередей с результатами аудита.
4. **Заметки ревью.** Секретарь governance зафиксировал инициалы, решение и тикеты по
   mitigation в `docs/source/nexus_transition_notes.md` и в трекере config delta
   (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Результаты

| Trace ID | Итог | Доказательства | Примечания |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Скриншоты fire/recover (внутренний линк) + replay `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diff телеметрии записаны в `docs/source/nexus_transition_notes.md#quarterly-routed-trace-audit-schedule`. | Queue-admission P95 оставался 612 ms (цель <=750 ms). Follow-up не требуется. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Архивированный payload `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` плюс hash OTLP replay, записанный в `status.md`. | SDK redaction salts совпали с Rust baseline; diff bot сообщил нулевые дельты. |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | Запись в governance tracker (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS profile manifest (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + telemetry pack manifest (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Повтор Q2 захешировал одобренный TLS profile и подтвердил нулевые stragglers; telemetry manifest фиксирует slot range 912–936 и workload seed `NEXUS-REH-2026Q2`. |

Все trace окна содержали как минимум одно событие `nexus.audit.outcome`, что
удовлетворяет guardrails Alertmanager (`NexusAuditOutcomeFailure` оставался зеленым
в течение квартала).

## Дальнейшие действия

- Обновлен routed-trace appendix с TLS hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`
  (см. `nexus_transition_notes.md`); mitigation `NEXUS-421` закрыта.
- Продолжать прикладывать raw OTLP replays и Torii diff артефакты в архив, чтобы
  усилить доказательства паритета для обзоров AND4/AND7.
- Убедиться, что предстоящие репетиции `TRACE-MULTILANE-CANARY` используют тот же
  telemetry helper, чтобы Q2 sign-off опирался на проверенный workflow.

## Индекс артефактов

| Актив | Локация |
|-------|----------|
| Валидатор телеметрии | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Правила и тесты алертов | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Пример payload outcome | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Расписание и заметки routed-trace | `docs/source/nexus_transition_notes.md` |

Этот отчет, перечисленные артефакты и экспорт алертов/телеметрии должны быть
приложены к журналу решений governance, чтобы закрыть B1 за квартал.
