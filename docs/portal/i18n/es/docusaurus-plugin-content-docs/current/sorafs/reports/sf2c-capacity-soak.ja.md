---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95eb94e00c79b0487a960f01460d035d9818787473e2a1dcafedc8c0de2038ad
source_last_modified: "2025-11-14T04:43:22.256497+00:00"
translation_last_reviewed: 2026-01-30
---

# Informe de soak de acumulación de capacidad SF-2c

Fecha: 2026-03-21

## Alcance

Este informe registra las pruebas determinísticas de soak de acumulación de capacidad SoraFS y pagos
solicitadas bajo la hoja de ruta SF-2c.

- **Soak multi-provider de 30 días:** Ejecutado por
  `capacity_fee_ledger_30_day_soak_deterministic` en
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  El harness instancia cinco providers, abarca 30 ventanas de settlement y
  valida que los totales del ledger coincidan con una proyección de referencia
  calculada de forma independiente. La prueba emite un digest Blake3
  (`capacity_soak_digest=...`) para que CI pueda capturar y comparar el snapshot
  canónico.
- **Penalizaciones por subentrega:** Impuestas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (mismo archivo). La prueba confirma que los umbrales de strikes, cooldowns,
  slashes de collateral y contadores del ledger permanecen determinísticos.

## Ejecución

Ejecuta las validaciones de soak localmente con:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Las pruebas completan en menos de un segundo en un portátil estándar y no requieren
fixtures externas.

## Observabilidad

Torii ahora expone snapshots de crédito de providers junto a fee ledgers para que los dashboards
puedan gatear sobre saldos bajos y penalty strikes:

- REST: `GET /v1/sorafs/capacity/state` devuelve entradas `credit_ledger[*]` que
  reflejan los campos del ledger verificados en la prueba de soak. Ver
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importación de Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` grafica los
  contadores de strikes exportados, totales de penalizaciones y collateral en garantía para que el
  equipo on-call pueda comparar los baselines de soak con entornos en vivo.

## Seguimiento

- Programar ejecuciones de gate semanales en CI para reejecutar la prueba de soak (smoke-tier).
- Extender el tablero de Grafana con objetivos de scrape de Torii cuando las exportaciones de
  telemetry de producción estén disponibles.
