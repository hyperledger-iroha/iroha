---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-transition-notes
title: Заметки о переходе Nexus
description: Зеркало `docs/source/nexus_transition_notes.md`, охватывающее доказательства перехода Phase B, график аудита и митигации.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Заметки о переходе Nexus

Этот лог отслеживает оставшуюся работу **Phase B - Nexus Transition Foundations** до завершения checklist многоlane запуска. Он дополняет milestone записи в `roadmap.md` и держит доказательства, на которые ссылаются B1-B4, в одном месте, чтобы governance, SRE и лиды SDK могли делиться единым источником истины.

## Область и cadence

- Покрывает routed-trace аудиты и telemetry guardrails (B1/B2), набор config delta, одобренный governance (B3), и follow-ups по multi-lane launch rehearsal (B4).
- Заменяет временную cadence note, которая была здесь раньше; с аудита Q1 2026 подробный отчет хранится в `docs/source/nexus_routed_trace_audit_report_2026q1.md`, а эта страница ведет рабочий график и реестр митигаций.
- Обновляйте таблицы после каждого routed-trace окна, голосования governance или launch rehearsal. Когда артефакты перемещаются, отражайте новый путь здесь, чтобы downstream docs (status, dashboards, SDK portals) могли ссылаться на стабильный якорь.

## Снимок доказательств (2026 Q1-Q2)

| Поток | Доказательства | Owner(s) | Статус | Примечания |
|------------|----------|----------|--------|-------|
| **B1 - Routed-trace audits** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | Завершено (Q1 2026) | Зафиксированы три окна аудита; TLS лаг `TRACE-CONFIG-DELTA` закрыт в Q2 rerun. |
| **B2 - Telemetry remediation & guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Завершено | Alert pack, политика diff bot и размер OTLP batch (`nexus.scheduler.headroom` log + панель headroom Grafana) поставлены; открытых waiver нет. |
| **B3 - Config delta approvals** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Завершено | Голосование GOV-2026-03-19 зафиксировано; подписанный bundle питает telemetry pack ниже. |
| **B4 - Multi-lane launch rehearsal** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Завершено (Q2 2026) | Q2 canary rerun закрыл митигацию TLS лага; validator manifest + `.sha256` фиксирует слот-диапазон 912-936, workload seed `NEXUS-REH-2026Q2` и hash профиля TLS из rerun. |

## Квартальный график routed-trace аудита

| Trace ID | Окно (UTC) | Результат | Примечания |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Пройдено | Queue-admission P95 оставался значительно ниже цели <=750 ms. Действий не требуется. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Пройдено | OTLP replay hashes приложены к `status.md`; parity SDK diff bot подтвердил нулевой drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Решено | TLS лаг закрыт в Q2 rerun; telemetry pack для `NEXUS-REH-2026Q2` фиксирует hash профиля TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (см. `artifacts/nexus/tls_profile_rollout_2026q2/`) и ноль отстающих. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Пройдено | Workload seed `NEXUS-REH-2026Q2`; telemetry pack + manifest/digest в `artifacts/nexus/rehearsals/2026q1/` (slot range 912-936) с agenda в `artifacts/nexus/rehearsals/2026q2/`. |

Будущие кварталы должны добавлять новые строки и переносить завершенные записи в приложение, когда таблица перерастет текущий квартал. Ссылайтесь на этот раздел из routed-trace отчетов или governance minutes через якорь `#quarterly-routed-trace-audit-schedule`.

## Митигации и backlog

| Item | Описание | Owner | Цель | Статус / Примечания |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Завершить распространение TLS профиля, который отставал в `TRACE-CONFIG-DELTA`, захватить evidence rerun и закрыть журнал митигации. | @release-eng, @sre-core | Q2 2026 routed-trace окно | Закрыто - hash TLS профиля `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` зафиксирован в `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; rerun подтвердил отсутствие отстающих. |
| `TRACE-MULTILANE-CANARY` prep | Запланировать Q2 rehearsal, приложить fixtures к telemetry pack и убедиться, что SDK harnesses переиспользуют валидированный helper. | @telemetry-ops, SDK Program | Планерка 2026-04-30 | Завершено - agenda хранится в `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` с metadata slot/workload; reuse harness отмечен в tracker. |
| Telemetry pack digest rotation | Запускать `scripts/telemetry/validate_nexus_telemetry_pack.py` перед каждым rehearsal/release и фиксировать digests рядом с tracker config delta. | @telemetry-ops | На каждый release candidate | Завершено - `telemetry_manifest.json` + `.sha256` выпущены в `artifacts/nexus/rehearsals/2026q1/` (slot range `912-936`, seed `NEXUS-REH-2026Q2`); digests скопированы в tracker и индекс доказательств. |

## Интеграция config delta bundle

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` остается каноническим summary diff. Когда приходят новые `defaults/nexus/*.toml` или изменения genesis, сначала обновляйте tracker, затем отражайте ключевые пункты здесь.
- Подписанные config bundles питают rehearsal telemetry pack. Pack, валидированный `scripts/telemetry/validate_nexus_telemetry_pack.py`, должен публиковаться вместе с доказательствами config delta, чтобы операторы могли воспроизвести точные артефакты, использованные в B4.
- Bundles Iroha 2 остаются без lanes: configs с `nexus.enabled = false` теперь отклоняют overrides lane/dataspace/routing, если не включен Nexus профиль (`--sora`), поэтому удаляйте секции `nexus.*` из single-lane шаблонов.
- Держите лог голосования governance (GOV-2026-03-19) связанным и с tracker, и с этой заметкой, чтобы будущие голосования могли копировать формат без повторного поиска ритуала одобрения.

## Follow-ups по launch rehearsal

- `docs/source/runbooks/nexus_multilane_rehearsal.md` фиксирует canary план, roster участников и шаги rollback; обновляйте runbook при изменениях топологии lanes или exporter телеметрии.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` перечисляет все артефакты, проверенные на репетиции 9 апреля, и теперь включает Q2 prep notes/agenda. Добавляйте будущие репетиции в тот же tracker вместо одноразовых tracker, чтобы доказательства были монотонными.
- Публикуйте OTLP collector snippets и Grafana exports (см. `docs/source/telemetry.md`) при изменении batching guidance exporter; обновление Q1 подняло batch size до 256 samples, чтобы предотвратить headroom alerts.
- Доказательства CI/tests multi-lane теперь живут в `integration_tests/tests/nexus/multilane_pipeline.rs` и запускаются через workflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), заменяя устаревшую ссылку `pytests/nexus/test_multilane_pipeline.py`; держите hash `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) синхронизированным с tracker при обновлении rehearsal bundles.

## Runtime lane lifecycle

- Планы runtime lane lifecycle теперь валидируют dataspace bindings и прерываются при сбое reconciliation Kura/tiered storage, оставляя каталог без изменений. Helpers очищают кэшированные lane relays для retired lanes, чтобы merge-ledger synthesis не переиспользовал устаревшие proofs.
- Применяйте планы через Nexus config/lifecycle helpers (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) для добавления/вывода lanes без рестарта; routing, TEU snapshots и manifest registries перезагружаются автоматически после успешного плана.
- Руководство оператора: при сбое плана проверьте отсутствие dataspaces или storage roots, которые не удается создать (tiered cold root/Kura lane directories). Исправьте базовые пути и повторите; успешные планы повторно эмитят telemetry diff lane/dataspace, чтобы dashboards отражали новую топологию.

## NPoS телеметрия и доказательства backpressure

Ретро Phase B launch rehearsal запросило детерминированные телеметрийные captures, доказывающие, что pacemaker NPoS и gossip слои остаются в пределах backpressure. Интеграционный harness в `integration_tests/tests/sumeragi_npos_performance.rs` прогоняет эти сценарии и выводит JSON summaries (`sumeragi_baseline_summary::<scenario>::...`) при появлении новых метрик. Запуск локально:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Установите `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` или `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R`, чтобы исследовать более стрессовые топологии; значения по умолчанию отражают профиль 1 s/`k=3`, использованный в B4.

| Сценарий / test | Покрытие | Ключевая телеметрия |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Блокирует 12 раундов с rehearsal block time, чтобы записать EMA latency envelopes, глубины очередей и redundant-send gauges перед сериализацией evidence bundle. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Переполняет очередь транзакций, чтобы гарантировать детерминированный запуск admission deferrals и экспорт счетчиков capacity/saturation. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Семплирует pacemaker jitter и view timeouts, пока не докажет соблюдение полосы +/-125 permille. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Проталкивает крупные RBC payloads до soft/hard лимитов store, чтобы показать рост, откат и стабилизацию session/byte счетчиков без переполнения store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Форсирует ретрансмиты, чтобы gauges redundant-send ratio и counters collectors-on-target продвигались, доказывая end-to-end связность требуемой телеметрии. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Детерминированно дропает chunks, чтобы backlog monitors поднимали faults вместо тихого дренажа payloads. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Приложите JSON линии, которые выводит harness, вместе с Prometheus scrape, захваченным во время запуска, всякий раз когда governance просит доказательства, что backpressure alarms соответствуют rehearsal топологии.

## Чеклист обновления

1. Добавляйте новые routed-trace окна и переносите старые по мере смены кварталов.
2. Обновляйте таблицу митигаций после каждого follow-up Alertmanager, даже если действие - закрыть тикет.
3. Когда config deltas меняются, обновляйте tracker, эту заметку и список digests telemetry pack в одном pull request.
4. Ссылайтесь здесь на новые rehearsal/telemetry артефакты, чтобы будущие обновления roadmap ссылались на единый документ, а не разрозненные ad-hoc заметки.

## Индекс доказательств

| Актив | Локация | Примечания |
|-------|----------|-------|
| Routed-trace audit report (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Канонический источник доказательств Phase B1; зеркалируется в портал через `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Содержит TRACE-CONFIG-DELTA diff summaries, инициалы reviewers и лог голосования GOV-2026-03-19. |
| Telemetry remediation plan | `docs/source/nexus_telemetry_remediation_plan.md` | Документирует alert pack, OTLP batch size и export budget guardrails, связанные с B2. |
| Multi-lane rehearsal tracker | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Список артефактов репетиции 9 апреля, validator manifest/digest, Q2 notes/agenda и rollback evidence. |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Фиксирует slot range 912-936, seed `NEXUS-REH-2026Q2` и hashes артефактов governance bundles. |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash утвержденного TLS профиля, захваченный в Q2 rerun; ссылайтесь в routed-trace appendices. |
| TRACE-MULTILANE-CANARY agenda | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Плановые заметки для Q2 rehearsal (window, slot range, workload seed, action owners). |
| Launch rehearsal runbook | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Операционный чеклист staging -> execution -> rollback; обновлять при изменении топологии lanes или guidance exporters. |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI, упомянутый в B4 ретро; архивируйте digests вместе с tracker при любом изменении pack. |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Проверяет `nexus.enabled = true` для multi-lane configs, сохраняет Sora catalog hashes и провижнит lane-local Kura/merge-log paths (`blocks/lane_{id:03}_{slug}`) через `ConfigLaneRouter` перед публикацией artefact digests. |
