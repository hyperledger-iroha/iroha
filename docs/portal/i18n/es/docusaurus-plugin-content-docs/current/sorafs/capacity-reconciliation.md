---
id: capacity-reconciliation
lang: es
direction: ltr
source: docs/portal/docs/sorafs/capacity-reconciliation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

El item del roadmap **SF-2c** exige que tesoreria demuestre que el libro de tarifas de capacidad coincide con las transferencias XOR ejecutadas cada noche. Usa el helper `scripts/telemetry/capacity_reconcile.py` para comparar el snapshot `/v1/sorafs/capacity/state` contra el lote de transferencias ejecutado y emitir metricas de texto de Prometheus para Alertmanager.

## Prerrequisitos
- Snapshot del estado de capacidad (entradas `fee_ledger`) exportado desde Torii.
- Exportacion del ledger para la misma ventana (JSON o NDJSON con `provider_id_hex`,
  `kind` = settlement/penalty, y `amount_nano`).
- Ruta al textfile collector de node_exporter si quieres alertas.

## Runbook
```bash
python3 scripts/telemetry/capacity_reconcile.py \
  --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
  --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
  --label nightly-capacity \
  --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
  --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
```

- Codigos de salida: `0` si coincide, `1` cuando faltan settlements/penalties o hay sobrepago, `2` con entradas invalidas.
- Adjunta el resumen JSON + hashes al paquete de tesoreria en
  `docs/examples/sorafs_capacity_marketplace_validation/`.
- Cuando el archivo `.prom` llega al textfile collector, la alerta
  `SoraFSCapacityReconciliationMismatch` (ver
  `dashboards/alerts/sorafs_capacity_rules.yml`) se dispara cuando se detectan transferencias faltantes, sobrepagadas o inesperadas.

## Salidas
- Estados por proveedor con diffs para settlements y penalties.
- Totales exportados como gauges:
  - `sorafs_capacity_reconciliation_missing_total{kind}`
  - `sorafs_capacity_reconciliation_overpaid_total{kind}`
  - `sorafs_capacity_reconciliation_unexpected_transfers_total`
  - `sorafs_capacity_reconciliation_expected_nano{kind}`
  - `sorafs_capacity_reconciliation_actual_nano{kind}`

## Rangos esperados y tolerancias
- La reconciliacion es exacta: los nanos esperados vs reales de settlement/penalty deben coincidir con tolerancia cero. Cualquier diferencia distinta de cero debe paginar a operadores.
- CI fija un digest de soak de 30 dias para el ledger de tarifas de capacidad (test `capacity_fee_ledger_30_day_soak_deterministic`) en `71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`. Actualiza el digest solo cuando cambien la politica de precios o las semanticas de cooldown.
- En el perfil de soak (`penalty_bond_bps=0`, `strike_threshold=u32::MAX`) los penalties se mantienen en cero; en produccion solo deben emitirse penalties cuando se incumplan los umbrales de utilizacion/uptime/PoR y se respete el cooldown configurado antes de slashes sucesivos.
