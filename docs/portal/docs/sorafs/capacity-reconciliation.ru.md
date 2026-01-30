---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e26cc8232dd7d3b392d56646fdfbf809952f017532a37aafbfde3c8cc704ae0e
source_last_modified: "2025-12-07T08:57:10.640650+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: capacity-reconciliation
title: Сверка емкости SoraFS
description: Ночной workflow для сопоставления реестров сборов за емкость с экспортами XOR-переводов.
---

Пункт дорожной карты **SF-2c** требует, чтобы казначейство доказало соответствие реестра сборов за емкость XOR-переводам, выполняемым каждую ночь. Используйте `scripts/telemetry/capacity_reconcile.py`, чтобы сравнить snapshot `/v1/sorafs/capacity/state` с выполненным батчем переводов и выпустить текстовые метрики Prometheus для Alertmanager.

## Предварительные условия
- Snapshot состояния емкости (записи `fee_ledger`), экспортированный из Torii.
- Экспорт ledger за тот же период (JSON или NDJSON с `provider_id_hex`,
  `kind` = settlement/penalty, и `amount_nano`).
- Путь к textfile collector node_exporter, если нужны алерты.

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Коды выхода: `0` при полном совпадении, `1` когда settlements/penalties отсутствуют или переплачены, `2` при некорректных входных данных.
- Приложите JSON-резюме + хэши к пакету казначейства в
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- Когда файл `.prom` попадает в textfile collector, алерт
  `SoraFSCapacityReconciliationMismatch` (см.
  `dashboards/alerts/sorafs_capacity_rules.yml`) срабатывает при обнаружении отсутствующих, переплаченных или неожиданных переводов провайдеров.

## Выходные данные
- Статусы по провайдерам с diff для settlements и penalties.
- Итоги, экспортируемые как gauges:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## Ожидаемые диапазоны и допуски
- Сверка строгая: ожидаемые и фактические nanos по settlement/penalty должны совпадать с нулевой толерантностью. Любое ненулевое расхождение должно пейджить операторов.
- CI фиксирует 30-дневный soak digest для реестра сборов за емкость (тест `capacity_fee_ledger_30_day_soak_deterministic`) на `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`. Обновляйте digest только при изменении ценовой политики или семантики cooldown.
- В soak-профиле (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) penalties остаются нулевыми; в продакшене penalties должны появляться только при нарушении порогов utilisation/uptime/PoR и с соблюдением настроенного cooldown перед последовательными slashes.
