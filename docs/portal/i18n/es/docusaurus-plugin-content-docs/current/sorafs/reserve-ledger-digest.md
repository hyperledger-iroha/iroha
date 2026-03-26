---
id: reserve-ledger-digest
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


La política Reserve+Rent (ítem del roadmap **SFM-6**) ahora incluye los helpers de CLI `sorafs reserve`
y el traductor `scripts/telemetry/reserve_ledger_digest.py` para que las ejecuciones de tesorería emitan
transferencias deterministas de rent/reserva. Esta página refleja el flujo definido en
`docs/source/sorafs_reserve_rent_plan.md` y explica cómo conectar el nuevo feed de transferencias en
Grafana + Alertmanager para que los revisores de economía y gobernanza auditen cada ciclo de facturación.

## Flujo de extremo a extremo

1. **Cotización + proyección del ledger**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

   sorafs reserve ledger \
     --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
     --provider-account soraカタカナ... \
     --treasury-account soraカタカナ... \
     --reserve-account soraカタカナ... \
     --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
     --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   El helper del ledger adjunta un bloque `ledger_projection` (renta debida, déficit de reserva,
   delta de top-up, booleanos de underwriting) más los ISIs Norito `Transfer` necesarios para
   mover XOR entre las cuentas de tesorería y reserva.

2. **Generar el digest + salidas Prometheus/NDJSON**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   El helper de digest normaliza los totales de micro-XOR a XOR, registra si la proyección cumple
   el underwriting y emite las métricas del feed de transferencias `sorafs_reserve_ledger_transfer_xor`
   y `sorafs_reserve_ledger_instruction_total`. Cuando hay que procesar múltiples ledgers (por
   ejemplo, un lote de proveedores), repite los pares `--ledger`/`--label` y el helper escribe un
   único archivo NDJSON/Prometheus que contiene cada digest para que los dashboards ingieran el
   ciclo completo sin glue a medida. El archivo `--out-prom` apunta a un textfile collector de
   node-exporter: deja el `.prom` en el directorio observado por el exporter o súbelo al bucket de
   telemetría consumido por el job del dashboard Reserve, mientras que `--ndjson-out` alimenta los
   mismos payloads en pipelines de datos.

3. **Publicar artefactos + evidencias**
   - Guarda los digests en `artifacts/sorafs_reserve/ledger/<provider>/` y enlaza el resumen
     Markdown desde tu informe semanal de economía.
   - Adjunta el digest JSON al burn-down de renta (para que los auditores puedan recalcular la
     matemática) e incluye el checksum dentro del paquete de evidencia de gobernanza.
   - Si el digest indica un top-up o una brecha de underwriting, referencia los IDs de alerta
     (`SoraFSReserveLedgerTopUpRequired`, `SoraFSReserveLedgerUnderwritingBreach`) y anota qué
     ISIs de transferencia se aplicaron.

## Métricas → dashboards → alertas

| Métrica fuente | Panel de Grafana | Alerta / gancho de política | Notas |
|---------------|------------------|-----------------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | “DA Rent Distribution (XOR/hour)” en `dashboards/grafana/sorafs_capacity_health.json` | Alimenta el digest semanal de tesorería; los picos en el flujo de reserva se propagan a `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`). |
| `torii_da_rent_gib_months_total` | “Capacity Usage (GiB-months)” (mismo dashboard) | Empareja con el digest del ledger para demostrar que el almacenamiento facturado coincide con las transferencias XOR. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | “Reserve Snapshot (XOR)” + tarjetas de estado en `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerTopUpRequired` se dispara cuando `requires_top_up=1`; `SoraFSReserveLedgerUnderwritingBreach` se dispara cuando `meets_underwriting=0`. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | “Transfers by Kind”, “Latest Transfer Breakdown” y las tarjetas de cobertura en `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing` y `SoraFSReserveLedgerTopUpTransferMissing` avisan cuando el feed de transferencias está ausente o en cero aunque se requiere rent/top-up; las tarjetas de cobertura caen a 0% en los mismos casos. |

Cuando se completa un ciclo de renta, refresca los snapshots Prometheus/NDJSON, confirma que los
paneles de Grafana detectan el nuevo `label`, y adjunta capturas + IDs de Alertmanager al paquete
de gobernanza de renta. Esto demuestra que la proyección CLI, la telemetría y los artefactos de
gobernanza provienen del **mismo** feed de transferencias y mantiene los dashboards económicos del
roadmap alineados con la automatización Reserve+Rent. Las tarjetas de cobertura deberían marcar
100% (o 1.0) y las nuevas alertas deben despejarse una vez que las transferencias de renta y top-up
estén presentes en el digest.
