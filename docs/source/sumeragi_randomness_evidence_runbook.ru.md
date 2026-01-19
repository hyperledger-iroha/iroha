---
lang: ru
direction: ltr
source: docs/source/sumeragi_randomness_evidence_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9a7b2f030cb798b78947c0d7cb298ccbd8a94a006be2e804f1ed043dc15dabc
source_last_modified: "2025-11-15T08:00:21.780712+00:00"
translation_last_reviewed: 2026-01-01
---

# Руководство по случайности и evidence Sumeragi

Этот гайд закрывает пункт Milestone A6 в roadmap, где требовалось обновить
операторские процедуры для VRF случайности и evidence для slashing. Используйте
его вместе с {doc}`sumeragi` и {doc}`sumeragi_chaos_performance_runbook` всякий
раз, когда вы разворачиваете новую сборку валидатора или фиксируете readiness
артефакты для governance.


Note: For the v1 release, VRF penalties jail offenders after the activation lag, and consensus slashing is delayed by `sumeragi.npos.reconfig.slashing_delay_blocks` (default 259200 blocks, ~3 days at 1s) so governance can cancel with `CancelConsensusEvidencePenalty` before it applies.

## Область и предпосылки

- `iroha_cli` настроен для целевого кластера (см. `docs/source/cli.md`).
- `curl`/`jq` для вытягивания Torii `/status` payload при подготовке входных данных.
- Доступ к Prometheus (или snapshot exports) для метрик `sumeragi_vrf_*`.
- Понимание текущей эпохи и roster, чтобы сопоставить вывод CLI со staking snapshot
  или governance manifest.

## 1. Подтвердить выбор режима и контекст эпохи

1. Запустите `iroha sumeragi params --summary`, чтобы подтвердить, что бинарь
   загрузил `sumeragi.consensus_mode="npos"`, и зафиксировать `k_aggregators`,
   `redundant_send_r`, длину эпохи и VRF commit/reveal offsets.
2. Проверьте runtime view:

   ```bash
   iroha sumeragi status --summary
   iroha sumeragi collectors --summary
   iroha sumeragi rbc status --summary
   ```

   Строка `status` печатает tuple leader/view, backlog RBC, DA retries, offsets
   эпохи и pacemaker deferrals; `collectors` мапит индексы collectors на peer IDs
   чтобы показать, какие валидаторы несут duties по случайности на проверяемой высоте.
3. Зафиксируйте номер эпохи, которую собираетесь аудитировать:

   ```bash
   EPOCH=$(curl -s "$TORII/status" | jq '.sumeragi.epoch.height // 0')
   printf "auditing epoch %s\n" "$EPOCH"
   ```

   Сохраните значение (decimal или с префиксом `0x`) для VRF команд ниже.

## 2. Снимок VRF эпох и штрафов

Используйте выделенные CLI подкоманды, чтобы получить сохраненные VRF записи
с каждого валидатора:

```bash
iroha sumeragi vrf-epoch --epoch "$EPOCH" --summary
iroha sumeragi vrf-epoch --epoch "$EPOCH" > artifacts/vrf_epoch_${EPOCH}.json

iroha sumeragi vrf-penalties --epoch "$EPOCH" --summary
iroha sumeragi vrf-penalties --epoch "$EPOCH" > artifacts/vrf_penalties_${EPOCH}.json
```

Сводки показывают, финализирована ли эпоха, сколько участников отправили
commits/reveals, длину roster и derived seed. JSON содержит список участников,
статус штрафов по signer и значение `seed_hex`, используемое pacemaker. Сравните
число участников со staking roster и проверьте, что массивы штрафов отражают
алерты, вызванные chaos тестами (late reveals должны быть в `late_reveals`,
forfeited валидаторы в `no_participation`).

## 3. Мониторинг VRF телеметрии и алертов

Prometheus экспортирует счетчики, требуемые roadmap:

- `sumeragi_vrf_commits_emitted_total`
- `sumeragi_vrf_reveals_emitted_total`
- `sumeragi_vrf_reveals_late_total`
- `sumeragi_vrf_non_reveal_penalties_total`
- `sumeragi_vrf_non_reveal_by_signer{signer="peer_id"}`
- `sumeragi_vrf_no_participation_total`
- `sumeragi_vrf_no_participation_by_signer{signer="peer_id"}`
- `sumeragi_vrf_rejects_total_by_reason{reason="..."}`

Пример PromQL для недельного отчета:

```promql
increase(sumeragi_vrf_non_reveal_by_signer[1w]) > 0
```

Во время readiness drill подтвердите, что:

- `sumeragi_vrf_commits_emitted_total` и `..._reveals_emitted_total` растут
  для каждого блока в commit/reveal окнах.
- Сценарии late-reveal поднимают `sumeragi_vrf_reveals_late_total` и очищают
  соответствующую запись в JSON `vrf_penalties`.
- `sumeragi_vrf_no_participation_total` вспыхивает только при намеренном
  удержании commits во время chaos тестов.

Grafana overview (`docs/source/grafana_sumeragi_overview.json`) содержит панели
для каждого счетчика; делайте скриншоты после каждого прогона и прикладывайте
к bundle артефактов, указанному в {doc}`sumeragi_chaos_performance_runbook`.

## 4. Ingestion evidence и streaming

Evidence для slashing должна собираться на каждом валидаторе и отправляться в Torii.
Используйте CLI helpers, чтобы показать паритет с HTTP endpoints, описанными в
{doc}`torii/sumeragi_evidence_app_api`:

```bash
# Count and list persisted evidence
iroha sumeragi evidence count --summary
iroha sumeragi evidence list --summary --limit 5

# Show JSON for audits
iroha sumeragi evidence list --limit 100 > artifacts/evidence_snapshot.json
```

Проверьте, что `total` совпадает с Grafana widget, питаемым
`sumeragi_evidence_records_total`, и подтвердите, что записи старше
`sumeragi.npos.reconfig.evidence_horizon_blocks` отклоняются (CLI печатает
причину). При тестировании alerting отправьте известный корректный payload:

```bash
iroha sumeragi evidence submit --evidence-hex-file fixtures/evidence/double_prevote.hex --summary
```

Мониторьте `/v1/events/sse` с фильтрованным stream, чтобы доказать, что SDK
видят те же данные: используйте Python one-liner из
{doc}`torii/sumeragi_evidence_app_api` для построения фильтра и захвата сырых
`data:` frames. SSE payloads должны отражать kind evidence и signer, показанные
в выводе CLI.

## 5. Упаковка evidence и отчетность

Для каждого rehearsal или release candidate:

1. Сохраните JSON файлы CLI (`vrf_epoch_*.json`, `vrf_penalties_*.json`,
   `evidence_snapshot.json`) в каталоге артефактов запуска (тот же root, что
   используют chaos/performance scripts).
2. Зафиксируйте результаты Prometheus запросов или snapshot exports для
   перечисленных выше счетчиков.
3. Приложите SSE capture и подтверждения алертов к README артефактов.
4. Обновите `status.md` и
   `docs/source/project_tracker/npos_sumeragi_phase_a.md` путями к артефактам
   и номером проверенной эпохи.

Следование этому чеклисту делает доказательства VRF случайности и slashing evidence
аудитируемыми во время rollout NPoS и дает governance reviewer детерминированный
след к захваченным метрикам и CLI snapshots.

## 6. Сигналы troubleshooting

- **Mode selection mismatch** — Если `iroha sumeragi params --summary` показывает
  `consensus_mode="permissioned"` или `k_aggregators` отличается от manifest,
  удалите захваченные артефакты, исправьте `iroha_config`, перезапустите валидатор
  и повторите валидацию, описанную в {doc}`sumeragi`.
- **Missing commits or reveals** — Плоская серия `sumeragi_vrf_commits_emitted_total`
  или `sumeragi_vrf_reveals_emitted_total` означает, что Torii не пересылает VRF frames.
  Проверьте логи валидатора на `handle_vrf_*` ошибки, затем отправьте payload вручную
  через POST helpers, описанные выше.
- **Unexpected penalties** — Когда `sumeragi_vrf_no_participation_total` всплескивает,
  проверьте `vrf_penalties_<epoch>.json` для подтверждения signer ID и сравните с
  staking roster. Штрафы, не совпадающие с chaos drills, указывают на clock skew валидатора
  или Torii replay protection; исправьте проблемный peer перед повтором теста.
- **Evidence ingestion stalls** — Когда `sumeragi_evidence_records_total` замирает
  при том, что chaos тесты эмитят ошибки, выполните `iroha sumeragi evidence count`
  на нескольких валидаторах и подтвердите, что `/v1/sumeragi/evidence/count`
  совпадает с выводом CLI. Любая разница означает, что SSE/webhook consumers могут быть stale,
  поэтому повторно отправьте известный fixture и эскалируйте в Torii, если счетчик не растет.
