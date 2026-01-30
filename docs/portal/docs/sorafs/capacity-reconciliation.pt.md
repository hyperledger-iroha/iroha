---
lang: pt
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
title: Conciliacao de capacidade da SoraFS
description: Fluxo noturno para comparar os ledgers de taxa de capacidade com as exportacoes de transferencias XOR.
---

O item do roadmap **SF-2c** exige que a tesouraria comprove que o ledger de taxas de capacidade corresponde as transferencias XOR executadas todas as noites. Use o helper `scripts/telemetry/capacity_reconcile.py` para comparar o snapshot `/v1/sorafs/capacity/state` com o lote de transferencias executado e emitir metricas de texto Prometheus para o Alertmanager.

## Pre-requisitos
- Snapshot do estado de capacidade (entradas `fee_ledger`) exportado do Torii.
- Exportacao do ledger para a mesma janela (JSON ou NDJSON com `provider_id_hex`,
  `kind` = settlement/penalty, e `amount_nano`).
- Caminho para o textfile collector do node_exporter se voce quiser alertas.

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Codigos de saida: `0` quando ha correspondencia, `1` quando settlements/penalties estao faltando ou pagos a mais, `2` em entradas invalidas.
- Anexe o resumo JSON + hashes ao pacote de tesouraria em
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- Quando o arquivo `.prom` chega ao textfile collector, o alerta
  `SoraFSCapacityReconciliationMismatch` (ver
  `dashboards/alerts/sorafs_capacity_rules.yml`) dispara quando transferencias faltantes, pagas a mais ou inesperadas sao detectadas.

## Saidas
- Status por provedor com diffs de settlements e penalties.
- Totais exportados como gauges:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## Faixas esperadas e tolerancias
- A reconciliacao e exata: nanos esperados vs reais de settlement/penalty devem bater com tolerancia zero. Qualquer diff diferente de zero deve paginar os operadores.
- CI fixa um digest de soak de 30 dias para o ledger de taxas de capacidade (teste `capacity_fee_ledger_30_day_soak_deterministic`) em `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`. Atualize o digest apenas quando semanticas de precos ou cooldown mudarem.
- No perfil de soak (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) penalties ficam em zero; em producao so deve haver penalties quando os limites de utilizacao/uptime/PoR forem violados e o cooldown configurado for respeitado antes de slashes sucessivos.
