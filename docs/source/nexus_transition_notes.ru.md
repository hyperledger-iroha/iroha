<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ru
direction: ltr
source: docs/source/nexus_transition_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: baf4e91fdb2c447c453711170e7df58a9a4831b15957052818736e0e7914e8a3
source_last_modified: "2025-12-13T06:33:05.401788+00:00"
translation_last_reviewed: 2026-01-01
---

# Переходные заметки Nexus

Этот журнал отслеживает оставшиеся работы **Фаза B - основы перехода Nexus**
до завершения чек-листа запуска multi-lane. Он дополняет записи в `roadmap.md`
и собирает доказательства, упомянутые в B1-B4, в одном месте, чтобы команды
governance, SRE и SDK имели единый источник истины.

## Область и периодичность

- Охватывает аудиты routed-trace и guardrails телеметрии (B1/B2), набор
  утвержденных governance дельт конфигурации (B3) и follow-up по репетициям
  multi-lane запуска (B4).
- Заменяет временную заметку о периодичности; с аудита 2026 Q1 подробный отчет
  находится в `docs/source/nexus_routed_trace_audit_report_2026q1.md`, а эта
  страница хранит актуальный график и реестр мер по снижению рисков.
- Обновляйте таблицы после каждого окна routed-trace, голосования governance
  или репетиции запуска. Когда артефакты перемещаются, отражайте новый адрес
  на этой странице, чтобы downstream-документация (status, dashboards, SDK порталы)
  ссылалась на стабильную опору.

## Снимок доказательств (2026 Q1-Q2)

| Поток работ | Доказательства | Владельцы | Статус | Примечания |
|------------|----------------|-----------|--------|------------|
| **B1 - Routed-trace аудиты** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | Завершено (Q1 2026) | Зафиксированы три окна аудита; задержка TLS из `TRACE-CONFIG-DELTA` закрыта в повторе Q2. |
| **B2 - Ремедиация телеметрии и guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Завершено | Пакет алертов, политика diff bot и размер батча OTLP (`nexus.scheduler.headroom` log + панель headroom в Grafana) доставлены; открытых исключений нет. |
| **B3 - Утверждения config delta** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Завершено | Голосование GOV-2026-03-19 зафиксировано; подписанный bundle подпитывает telemetria pack ниже. |
| **B4 - Репетиция multi-lane запуска** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Завершено (Q2 2026) | Q2 canary rerun закрыл TLS lag mitigation; manifest валидаторов + `.sha256` фиксируют диапазон слотов 912-936, seed `NEXUS-REH-2026Q2` и hash TLS профиля из повтора. |

## Квартальный график routed-trace аудитов

| Trace ID | Окно (UTC) | Итог | Примечания |
|----------|------------|------|------------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Пройдено | Queue-admission P95 оказался значительно ниже цели <=750 ms. Действия не требуются. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Пройдено | OTLP replay hashes приложены к `status.md`; SDK diff bot подтвердил нулевой drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Закрыто | TLS лаг закрыт в повторе Q2; telemetria pack для `NEXUS-REH-2026Q2` фиксирует hash TLS профиля `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (см. `artifacts/nexus/tls_profile_rollout_2026q2/`) и отсутствие отстающих. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Пройдено | Seed нагрузки `NEXUS-REH-2026Q2`; telemetria pack + manifest/digest в `artifacts/nexus/rehearsals/2026q1/` (диапазон слотов 912-936) с agenda в `artifacts/nexus/rehearsals/2026q2/`. |

В следующих кварталах добавляйте новые строки и переносите завершенные записи
в приложение, когда таблица перерастет текущий квартал. Ссылайтесь на этот раздел
из routed-trace отчетов или протоколов governance, используя якорь
`#quarterly-routed-trace-audit-schedule`.

## Митигирования и backlog

| Пункт | Описание | Владелец | Срок | Статус / Примечания |
|-------|----------|---------|------|---------------------|
| `NEXUS-421` | Завершить распространение TLS профиля, отставшего во время `TRACE-CONFIG-DELTA`, зафиксировать доказательства повтора и закрыть запись mitigation. | @release-eng, @sre-core | Окно routed-trace Q2 2026 | Закрыто - hash TLS профиля `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` зафиксирован в `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; повтор подтвердил отсутствие отстающих. |
| Подготовка `TRACE-MULTILANE-CANARY` | Запланировать Q2 репетицию, приложить fixtures к telemetria pack и убедиться, что SDK harness повторно использует валидированный helper. | @telemetry-ops, SDK Program | Планировочный созвон 2026-04-30 | Завершено - agenda сохранена в `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` с метаданными слотов/нагрузки; повторное использование harness отмечено в tracker. |
| Ротация digests telemetria pack | Запускать `scripts/telemetry/validate_nexus_telemetry_pack.py` перед каждым rehearsal/release и писать digests рядом с tracker config delta. | @telemetry-ops | На каждый release candidate | Завершено - `telemetry_manifest.json` + `.sha256` выпущены в `artifacts/nexus/rehearsals/2026q1/` (диапазон слотов `912-936`, seed `NEXUS-REH-2026Q2`); digests скопированы в tracker и индекс доказательств. |

## Интеграция config delta bundle

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` остается
  каноническим summary diff. Когда появляются новые `defaults/nexus/*.toml`
  или изменения genesis, сначала обновляйте tracker, затем отражайте основные
  пункты здесь.
- Подписанные config bundles подпитывают rehearsal telemetria pack. Пакет,
  проверяемый `scripts/telemetry/validate_nexus_telemetry_pack.py`, должен быть
  опубликован рядом с доказательствами config delta, чтобы операторы могли
  воспроизвести точные артефакты B4.
- Bundles Iroha 2 остаются без lanes: configs с `nexus.enabled = false` теперь
  отклоняют overrides lane/dataspace/routing, если профиль Nexus не включен
  (`--sora`), поэтому удаляйте секции `nexus.*` из single-lane шаблонов.
- Держите log голосования governance (GOV-2026-03-19) связанным с tracker и
  этой заметкой, чтобы будущие голосования копировали формат без повторного
  поиска процедуры одобрения.

## Follow-up по репетиции запуска

- `docs/source/runbooks/nexus_multilane_rehearsal.md` фиксирует canary план,
  список участников и шаги rollback; обновляйте runbook при изменении топологии
  lanes или экспортеров телеметрии.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` перечисляет каждый
  артефакт, проверенный на репетиции 9 апреля, и теперь содержит Q2 prep notes/agenda.
  Добавляйте будущие репетиции в тот же tracker вместо отдельных документов, чтобы
  сохранять монотонность доказательств.
- Публикуйте OTLP collector snippets и Grafana exports (см. `docs/source/telemetry.md`)
  при изменении guidance по batching; обновление Q1 увеличило batch size до 256
  сэмплов, чтобы избежать headroom alerts.
- Доказательства CI/test multi-lane теперь находятся в
  `integration_tests/tests/nexus/multilane_pipeline.rs` и выполняются в workflow
  `Nexus Multilane Pipeline`
  (`.github/workflows/integration_tests_multilane.yml`), заменяя устаревший
  `pytests/nexus/test_multilane_pipeline.py`; держите hash для
  `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) синхронизированным
  с tracker при обновлении rehearsal bundles.

## Жизненный цикл lane во время runtime

- Планы runtime lane lifecycle теперь валидируют bindings dataspaces и прерывают
  выполнение при ошибках reconciliation Kura/tiers storage, оставляя каталог неизменным.
  Helpers удаляют кэшированные lane relays для retired lanes, чтобы merge-ledger
  synthesis не использовал устаревшие proofs.
- Применяйте планы через helpers Nexus config/lifecycle (`State::apply_lane_lifecycle`,
  `Queue::apply_lane_lifecycle`) для добавления/удаления lanes без рестарта; routing,
  TEU snapshots и manifest registries перезагружаются автоматически после успешного плана.
- Рекомендация операторам: при сбое плана проверьте отсутствующие dataspaces или roots
  storage, которые нельзя создать (tiered cold root / Kura lane directories). Исправьте
  базовые пути и повторите; успешные планы переизлучают lane/dataspace telemetria diff,
  чтобы dashboards отражали новую топологию.

## Доказательства телеметрии и backpressure NPoS

Итоги Phase B rehearsal retro запросили детерминированные телеметрические замеры,
подтверждающие, что NPoS pacemaker и gossip уровни остаются в пределах backpressure.
Интеграционный harness `integration_tests/tests/sumeragi_npos_performance.rs`
воспроизводит эти сценарии и печатает JSON summaries
(`sumeragi_baseline_summary::<scenario>::...`) при появлении новых метрик.
Запуск локально:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Используйте `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` или
`SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` для более тяжелых топологий; значения по умолчанию
соответствуют профилю 1 s/`k=3`, использованному в B4.

| Сценарий / тест | Покрытие | Ключевая телеметрия |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Блокирует 12 раундов с rehearsal block time, чтобы записать EMA latency envelopes, глубину очередей и redundant-send gauges перед сериализацией evidence bundle. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Перегружает очередь транзакций, чтобы детерминированно включить admission deferrals и экспортировать счетчики capacity/saturation. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Измеряет pacemaker jitter и view timeouts до подтверждения, что соблюдается полоса +/-125 permille. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Подает крупные RBC payloads до soft/hard лимитов store, чтобы показать рост, откат и стабилизацию счетчиков без переполнения store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Форсирует retransmits, чтобы обновить redundant-send gauges и collectors-on-target counters, подтверждая end-to-end телеметрию. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Детерминированно теряет chunks, чтобы backlog monitors поднимали faults вместо бесшумного дренажа payloads. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Прикладывайте JSON строки из harness вместе со scrape Prometheus, снятым во время
прогона, когда governance запрашивает подтверждение соответствия backpressure alarms
топологии репетиции.

## Чек-лист обновлений

1. Добавляйте новые routed-trace окна и выводите старые по мере смены кварталов.
2. Обновляйте таблицу mitigations после каждого Alertmanager follow-up, даже если
   действие - закрыть тикет.
3. При изменении config delta обновляйте tracker, эту заметку и список telemetria pack
   digests в одном pull request.
4. Линкуйте новые rehearsal/telemetry артефакты здесь, чтобы будущие status updates
   ссылались на один документ вместо разрозненных ad-hoc заметок.

## Индекс доказательств

| Актив | Расположение | Примечания |
|-------|--------------|------------|
| Routed-trace audit report (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Канонический источник доказательств Phase B1; отражен в портале под `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Содержит TRACE-CONFIG-DELTA summaries, инициалы ревьюеров и log голосования GOV-2026-03-19. |
| Telemetry remediation plan | `docs/source/nexus_telemetry_remediation_plan.md` | Описывает alert pack, OTLP batch sizing и export budget guardrails для B2. |
| Multi-lane rehearsal tracker | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Список артефактов репетиции 9 апреля, validator manifest/digest, Q2 prep notes/agenda и rollback evidence. |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Фиксирует диапазон слотов 912-936, seed `NEXUS-REH-2026Q2` и hashes артефактов для governance bundles. |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash утвержденного TLS профиля, зафиксированный в повторе Q2; цитировать в routed-trace приложениях. |
| TRACE-MULTILANE-CANARY agenda | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Планировочные заметки для репетиции Q2 (окно, диапазон слотов, seed нагрузки, владельцы действий). |
| Launch rehearsal runbook | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Операционный checklist для staging -> execution -> rollback; обновлять при изменении топологии lane или guidance exporters. |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI из B4 retro; архивировать digests рядом с tracker при изменениях пакета. |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Доказывает `nexus.enabled = true` для multi-lane configs, сохраняет hashes каталога Sora и создает локальные Kura/merge-log paths (`blocks/lane_{id:03}_{slug}`) через `ConfigLaneRouter` перед публикацией artifact digests. |
