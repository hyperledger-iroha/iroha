---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: ru
direction: ltr
source: docs/source/runbooks/nexus_lane_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1d9fbcf5301bd36a0bdc8a010ae214f7b1561373237054ab3c8a45a411cf6c0e
source_last_modified: "2025-12-14T09:53:36.241505+00:00"
translation_last_reviewed: 2025-12-28
---

# Ранбук финальности линий Nexus и оракулов

**Статус:** Активен — закрывает требование NX-18 по dashboards/runbooks.  
**Аудитория:** Core Consensus WG, SRE/Telemetry, Release Engineering, дежурные лиды.  
**Область:** Описывает SLO для длительности слота, DA‑кворума, оракула и
буфера расчётов, которые обеспечивают обещание финальности 1 с. Используйте
вместе с `dashboards/grafana/nexus_lanes.json` и утилитами из `scripts/telemetry/`.

## Дашборды

- **Grafana (`dashboards/grafana/nexus_lanes.json`)** — публикует панель “Nexus Lane Finality & Oracles”. Панели отслеживают:
  - `histogram_quantile()` по `iroha_slot_duration_ms` (p50/p95/p99) и gauge последней выборки.
  - `iroha_da_quorum_ratio` и `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` для подсветки churn DA.
  - Метрики оракулов: `iroha_oracle_price_local_per_xor`, `iroha_oracle_staleness_seconds`, `iroha_oracle_twap_window_seconds`, `iroha_oracle_haircut_basis_points`.
  - Панель буфера расчётов (`iroha_settlement_buffer_xor`) с дебетами по линиям из receipt `LaneBlockCommitment`.
- **Правила алертов** — используют Slot/DA SLO из `ans3.md`. Тревожить, если:
  - p95 длительности слота > 1000 мс два окна подряд по 5 м,
  - DA‑кворум < 0,95 или `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m]) > 0`,
  - staleness оракула > 90 с или окно TWAP ≠ 60 с,
  - буфер расчётов < 25 % (soft) / 10 % (hard) после включения метрики.

## Быстрая шпаргалка по метрикам

| Метрика | Цель / Алерт | Примечания |
|--------|----------------|-------|
| `histogram_quantile(0.95, iroha_slot_duration_ms)` | ≤ 1000 мс (hard), 950 мс warning | Используйте панель в дашборде или `scripts/telemetry/check_slot_duration.py` (`--json-out artifacts/nx18/slot_summary.json`) на экспорт Prometheus из chaos‑прогонов. |
| `iroha_slot_duration_ms_latest` | Последний слот; расследовать, если > 1100 мс даже при нормальных квантилях. | Экспортируйте значение в инцидент. |
| `iroha_da_quorum_ratio` | ≥ 0,95 в окне 30 м. | Производная от reschedule DA при коммитах блоков. |
| `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` | Должно быть 0 вне chaos‑репетиций. | Любой устойчивый рост трактуйте как `missing-availability warning`. |

Каждый reschedule также пишет предупреждение в pipeline Torii с `kind = "missing-availability warning"`. Фиксируйте эти события вместе со всплеском метрики, чтобы определить затронутый заголовок блока, попытку ретрая и счётчики requeue без просмотра логов валидаторов.【crates/iroha_core/src/sumeragi/main_loop.rs:5164】
| `iroha_oracle_staleness_seconds` | ≤ 60 с. Алерт при 75 с. | Указывает на устаревшие TWAP‑фиды 60 с. |
| `iroha_oracle_twap_window_seconds` | Ровно 60 с ± 5 с. | Отклонение означает неверную настройку оракула. |
| `iroha_oracle_haircut_basis_points` | Совпадает с tier ликвидности линии (0/25/75 bps). | Эскалировать при неожиданном росте haircut. |
| `iroha_settlement_buffer_xor` | Soft 25 %, hard 10 %. При <10 % включить XOR‑only. | Панель показывает дебеты micro‑XOR по lane/dataspace; экспортируйте перед изменением политики роутера. |

## План реагирования

### Нарушение длительности слота
1. Подтвердить через дашборд + `promql` (p95/p99).  
2. Сохранить вывод `scripts/telemetry/check_slot_duration.py --json-out <path>`
   (и snapshot метрик), чтобы CXO‑ревью могли проверить gate 1 с.  
3. Проверить входы RCA: глубину очереди mempool, reschedule DA, трассы IVM.  
4. Открыть инцидент, приложить скриншот Grafana и запланировать chaos‑drill при повторении.

### Деградация DA‑кворума
1. Проверить `iroha_da_quorum_ratio` и счётчик reschedule; сопоставить с логами `missing-availability warning`.  
2. Если ratio <0,95 — зафиксировать проблемных attesters, расширить параметры семплирования или перевести профиль в XOR‑only.  
3. Запустить `scripts/telemetry/check_nexus_audit_outcome.py` во время routed‑trace rehearsal, чтобы подтвердить прохождение `nexus.audit.outcome` после исправлений.  
4. Архивировать bundles DA‑receipt вместе с тикетом инцидента.

### Staleness оракула / drift haircut
1. Панели 5–8: проверить цену, staleness, окно TWAP и haircut.  
2. При staleness >90 с: перезапустить или переключить oracle feed, затем повторить chaos‑harness.  
3. При mismatch haircut: проверить профиль ликвидности и недавние governance‑изменения; уведомить treasury при необходимости вмешательства.

### Алерты буфера расчётов
1. Используйте `iroha_settlement_buffer_xor` (и ночные receipts), чтобы подтвердить запас до изменения политики роутера.  
2. При пересечении порога:
   - **Soft breach (<25 %)**: подключить treasury, рассмотреть использование swap‑линий, зафиксировать алерт.  
   - **Hard breach (<10 %)**: включить XOR‑only, отказать subsidised‑линиям, записать в `ops/drill-log.md`.  
3. См. `docs/source/settlement_router.md` для рычагов repo/reverse‑repo.

## Доказательства и автоматизация

- **CI** — подключить `scripts/telemetry/check_slot_duration.py --json-out artifacts/nx18/slot_summary.json` и `scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` в RC acceptance workflow, чтобы каждый RC отдавал summary по длительности слота и результаты gate DA/oracle/buffer вместе со snapshot метрик. Хелпер уже вызывается из `ci/check_nexus_lane_smoke.sh`.  
- **Паритет дашбордов** — запустить `scripts/telemetry/compare_dashboards.py dashboards/grafana/nexus_lanes.json <prod-export.json>` для сверки staging/prod экспорта с опубликованным board.  
- **Trace‑артефакты** — во время TRACE‑репетиций или NX-18 chaos‑drill запускать `scripts/telemetry/check_nexus_audit_outcome.py` для архивации `nexus.audit.outcome` (`docs/examples/nexus_audit_outcomes/`). Приложить архив и скриншоты Grafana к drill‑логу.
- **Сбор evidence слота** — после генерации summary JSON выполнить `scripts/telemetry/bundle_slot_artifacts.py --metrics <prometheus.tgz-extract>/metrics.prom --summary artifacts/nx18/slot_summary.json --out-dir artifacts/nx18`, чтобы `slot_bundle_manifest.json` зафиксировал SHA‑256 digests обоих артефактов. Загружайте каталог как есть вместе с RC evidence‑пакетом. Release‑pipeline запускает это автоматически (можно пропустить через `--skip-nexus-lane-smoke`) и копирует `artifacts/nx18/` в релизный output.

## Чек‑лист обслуживания

- Держите `dashboards/grafana/nexus_lanes.json` синхронизированным с экспортами Grafana после каждого изменения схемы; фиксируйте это в commit‑сообщениях с упоминанием NX-18.  
- Обновляйте ранбук при появлении новых метрик (например, gauges буфера расчётов) или новых порогов алертов.  
- Записывайте каждую chaos‑репетицию (latency слота, jitter DA, stall оракула, истощение буфера) с `scripts/telemetry/log_sorafs_drill.sh --log ops/drill-log.md --program NX-18 --status <status>`.

Следование этому ранбуку предоставляет требуемые NX-18 доказательства “operator dashboards/runbooks” и гарантирует, что SLO финальности будет контролируем до Nexus GA.
