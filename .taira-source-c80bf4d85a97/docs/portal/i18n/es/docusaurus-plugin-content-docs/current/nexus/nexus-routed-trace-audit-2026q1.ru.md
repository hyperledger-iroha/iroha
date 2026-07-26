---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
título: Auditoría de seguimiento enrutado para el primer trimestre de 2026 (B1)
descripción: Зеркало `docs/source/nexus_routed_trace_audit_report_2026q1.md`, охватывающее итоги квартальных репетиций телеметрии.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::nota Канонический источник
Esta página está escrita `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Deje copias sincronizadas, pero no hay archivos disponibles durante mucho tiempo.
:::

# Отчет аудита Routed-Trace para el primer trimestre de 2026 (B1)

Hoja de ruta de Punk **B1 - Línea base de telemetría y auditorías de seguimiento de ruta** Este es un programa de seguimiento de ruta Nexus. Esta es una ficción para la auditoría del primer trimestre de 2026 (marzo de enero), que necesita mejorar la configuración del televisor перед репетициями запуска Q2.

## Область и таймлайн

| ID de seguimiento | Ok (UTC) | Celo |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Проверить гистограммы допуска в включением multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Valide la reproducción OTLP, comparta el bot de diferenciación y el SDK de los televisores con AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Puede controlar los deltas `iroha_config` y realizar la reversión según RC1. |Каждая репетиция проходила на прод-подобной топологии с включенной routed-trace instrumental (telemetro `nexus.audit.outcome` + счетчики Prometheus), descargas de alertamanager y puertos deportivos en `docs/examples/`.

## Metodología

1. **Сбор телеметрии.** Все узлы эмитировали структурированное событие `nexus.audit.outcome` and сопутствующие метрики (`nexus_audit_outcome_total*`). El asistente `scripts/telemetry/check_nexus_audit_outcome.py` implementó el registro JSON, el estado de validación y la carga útil del archivo en `docs/examples/nexus_audit_outcomes/`. [scripts/telemetría/check_nexus_audit_outcome.py:1]
2. **Проверка алертов.** `dashboards/alerts/nexus_audit_rules.yml` y el arnés de seguridad garantiza la estabilidad de la carga útil y de la carga útil. CI запускает `dashboards/alerts/tests/nexus_audit_rules.test.yml` при каждом изменении; те же правила вручную прогонялись в каждом окне.
3. **Съемка дашбордов.** Los operadores de paneles exportadores de ruta de seguimiento en `dashboards/grafana/soranet_sn16_handshake.json` (apretón de manos cerrado) y обзорные дашборды телеметрии, чтобы связать здоровье очередей с результатами аудита.
4. **Заметки ревьюеров.** Секретарь по управлению записал инициалы ревьюеров, решение и тикеты по mitigaciones en [Nexus transición notas](./nexus-transition-notes) y трекер конфигурационных дельт (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Respuestas| ID de seguimiento | Este | Доказательства | Примечания |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pase | Скриншоты fuego/recuperación алертов (внутренняя ссылка) + repetición `dashboards/alerts/tests/soranet_lane_rules.test.yml`; телеметрийные дифы зафиксированы в [Nexus notas de transición](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 dura 612 ms (цель <=750 ms). Дальнейших действий не требуется. |
| `TRACE-TELEMETRY-BRIDGE` | Pase | Carga útil incorporada `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` más hash de reproducción OTLP, incluida en `status.md`. | Sales de redacción del SDK compatibles con Rust baseline; diff bot сообщил ноль дельт. |
| `TRACE-CONFIG-DELTA` | Pasa (mitigación cerrada) | Primer bloque de gobernanza (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifiesto de perfil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifiesto de paquete de telemetría (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Vuelva a ejecutar Q2 захешировал одобренный TLS профиль и подтвердил отсутствие отстающих; manifiesto de telemetría фиксирует диапазон слотов 912-936 y semilla de carga de trabajo `NEXUS-REH-2026Q2`. |

Все rastros выдали хотя бы одно событие `nexus.audit.outcome` en рамках окон, что удовлетворило guardrails Alertmanager (`NexusAuditOutcomeFailure` оставался зеленым весь квартал).

## Seguimientos- Дополнение routed-trace обновлено TLS хэшем `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; mitigación `NEXUS-421` закрыт в notas de transición.
- Продолжать прикладывать сырые OTLP replays and артефакты Torii diff в архив, чтобы усилить паритетные доказательства для Proverok Android AND4/AND7.
- Tenga en cuenta que las repeticiones previas `TRACE-MULTILANE-CANARY` utilizan el asistente de telemetría y la aprobación del segundo trimestre. flujo de trabajo проверенный.

## Индекс артефактов

| Activo | Ubicación |
|-------|----------|
| Telemetros validadores | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Правила и тесты алертов | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Primera carga útil de resultados | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Трекер конфиг-дельт | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Gráficos y datos de seguimiento de rutas | [Notas de transición Nexus](./nexus-transition-notes) |

Esto es lo que se muestra, los artefactos y los deportes, alertas/telemetros para la gobernanza diaria, cómo закрыть B1 за квартал.