---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Informe de remojo de acumulación de capacidad SF-2c

Fecha: 2026-03-21

## Alcance

Este informe registra las pruebas determinísticas de remojo de acumulación de capacidad SoraFS y pagos.
solicitadas bajo la hoja de ruta SF-2c.

- **Remojo multiproveedor de 30 días:** Ejecutado por
  `capacity_fee_ledger_30_day_soak_deterministic` es
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  El arnés cinco instancias proveedores, abarca 30 ventanas de asentamiento y
  valida que los totales del libro mayor coinciden con una proyección de referencia
  calculada de forma independiente. La prueba emite un resumen Blake3
  (`capacity_soak_digest=...`) para que CI pueda capturar y comparar la instantánea
  canónico.
- **Penalizaciones por subentrega:** Impuestas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (mismo archivo). La prueba confirma que los umbrales de strikes, cooldowns,
  Las barras diagonales de garantía y contadores del libro mayor permanecen determinísticos.

## Ejecución

Ejecuta las validaciones de remojo localmente con:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Las pruebas se completan en menos de un segundo en un portátil estándar y no requieren
accesorios externos.

## Observabilidad

Torii ahora exponen instantáneas de crédito de proveedores junto a libros de contabilidad de tarifas para que los paneles
puedan gatear sobre saldos bajos y strikes de penalización:- REST: `GET /v1/sorafs/capacity/state` devuelve entradas `credit_ledger[*]` que
  reflejando los campos del libro mayor verificados en la prueba de remojo. Ver
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importación de Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` gráfica los
  contadores de huelgas exportados, totales de penalizaciones y colaterales en garantía para que el
  equipo on-call pueda comparar las líneas base de remojo con entornos en vivo.

##Seguimiento

- Programar ejecuciones de puerta semanales en CI para reejecutar la prueba de remojo (smoke-tier).
- Extender el tablero de Grafana con objetivos de scrape de Torii cuando las exportaciones de
  telemetría de producción estén disponibles.