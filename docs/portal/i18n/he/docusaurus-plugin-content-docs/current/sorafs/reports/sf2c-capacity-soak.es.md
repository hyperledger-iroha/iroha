---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# מידע על ספיגה של ספיגה SF-2c

פחה: 2026-03-21

## אלקנס

Este informe registra las pruebas determinísticas de soak de acumulación de capacidad SoraFS y pagos
solicitadas bajo la hoja de ruta SF-2c.

- **Saak multi-provider de 30 días:** Ejecutado por
  `capacity_fee_ledger_30_day_soak_deterministic` en
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  ספקי אל רתמה instancia cinco, abarca 30 ventanas de settlement y
  valida que los totals del ספר coincidan con una proyección de referencia
  calculada de forma independiente. La prueba emite un digest Blake3
  (`capacity_soak_digest=...`) para que CI pueda capturar y comparar el snapshot
  canónico.
- **Penalizaciones por subentrega:** Impuestas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (mismo archivo). La prueba confirma que los umbrales de strikes, cooldowns,
  חתכים ביטחונות וקופות חשבונות קבועות.

## פליטה

Ejecuta las validaciones de soak localmente con:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Las pruebas completan en menos de un segundo en un portátil estándar y no requieren
מתקנים חיצוניים.

## תצפית

Torii ahora expone תצלומי מצב של קרדיטו של ספקים פנקסי חשבונות עמלה עבור לוחות מחוונים
puedan gatear sobre saldos bajos y strikes penalty:

- מנוחה: `GET /v2/sorafs/capacity/state` devuelve entradas `credit_ledger[*]` que
  reflejan los campos del ledger verificados en la prueba de soak. Ver
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importación de Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` גרפיקה
  contadores de strikes exportados, totals de penalizaciones y collateral en garantía para que el
  equipo on-call pueda comparar los linelines de soak con entornos en vivo.

## Seguimiento

- Programar ejecuciones de gate semanales en CI para reejecutar la prueba de soak (מדרגת עשן).
- Extender el tablero de Grafana עם אובייקטים של גרידה דה Torii cuando las exportaciones de
  telemetry de producción estén disponibles.