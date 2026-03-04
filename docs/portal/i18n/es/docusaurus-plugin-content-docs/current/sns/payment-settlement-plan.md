---
id: payment-settlement-plan
lang: es
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Plan de pago y liquidacion SNS

> Fuente canonica: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

La tarea del roadmap **SN-5 -- Payment & Settlement Service** introduce una capa
de pago determinista para el Sora Name Service. Cada registro, renovacion o
reembolso debe emitir un payload Norito estructurado para que tesoreria,
stewards y gobernanza puedan reproducir los flujos financieros sin hojas de
calculo. Esta pagina destila la especificacion para audiencias del portal.

## Modelo de ingresos

- La tarifa base (`gross_fee`) deriva de la matriz de precios del registrar.
- Tesoreria recibe `gross_fee x 0.70`, los stewards reciben el resto menos
  bonos de referral (tope 10 %).
- Holdbacks opcionales permiten a la gobernanza pausar pagos a stewards durante
  disputas.
- Los bundles de settlement exponen un bloque `ledger_projection` con ISIs
  `Transfer` concretos para que la automatizacion publique movimientos XOR
  directo en Torii.

## Servicios y automatizacion

| Componente | Proposito | Evidencia |
|------------|-----------|-----------|
| `sns_settlementd` | Aplica politica, firma bundles, expone `/v1/sns/settlements`. | Bundle JSON + hash. |
| Settlement queue & writer | Cola idempotente + submitter del ledger impulsado por `iroha_cli app sns settlement ledger`. | Manifiesto de bundle hash <-> tx hash. |
| Reconciliation job | Diff diario + estado mensual bajo `docs/source/sns/reports/`. | Markdown + JSON digest. |
| Refund desk | Reembolsos aprobados por gobernanza via `/settlements/{id}/refund`. | `RefundRecordV1` + ticket. |

Los helpers de CI reflejan estos flujos:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Observabilidad y reporting

- Dashboards: `dashboards/grafana/sns_payment_settlement.json` para totales de
  tesoreria vs stewards, pagos de referral, profundidad de cola y latencia de
  reembolso.
- Alertas: `dashboards/alerts/sns_payment_settlement_rules.yml` monitorea edad
  pendiente, fallos de reconciliacion y deriva del ledger.
- Estados: los digest diarios (`settlement_YYYYMMDD.{json,md}`) se acumulan en
  reportes mensuales (`settlement_YYYYMM.md`) que se suben a Git y al almacen de
  objetos de gobernanza (`s3://sora-governance/sns/settlements/<period>/`).
- Los paquetes de gobernanza agrupan dashboards, logs de CLI y aprobaciones antes
  de la aprobacion del consejo.

## Checklist de rollout

1. Prototipar helpers de quote + ledger y capturar un bundle de staging.
2. Lanzar `sns_settlementd` con queue + writer, cablear dashboards y ejecutar
   pruebas de alertas (`promtool test rules ...`).
3. Entregar el helper de reembolsos mas la plantilla de estado mensual; reflejar
   artefactos en `docs/portal/docs/sns/reports/`.
4. Ejecutar un ensayo con partners (mes completo de settlements) y capturar el
   voto de gobernanza que marca SN-5 como completo.

Volver al documento fuente para las definiciones exactas del esquema, preguntas
abiertas y futuras enmiendas.
