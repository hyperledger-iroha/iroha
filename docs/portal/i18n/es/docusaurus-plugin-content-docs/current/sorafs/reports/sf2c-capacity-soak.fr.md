---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Informe de remojo de acumulación de capacidad SF-2c

Fecha: 2026-03-21

## Portée

Este informe consigna las pruebas determinantes de remojo de acumulación y pago de capacidad SoraFS demandadas.
dans la feuille de route SF-2c.

- **Remojar multiproveedor cada 30 días:** Ejecutado por
  `capacity_fee_ledger_30_day_soak_deterministic` en
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  El arnés instala cinco proveedores, cubre 30 ventanas de asentamiento y
  valide que les totaux du ledger corresponden a una proyección de referencia
  calculado indépendamment. La prueba hizo un resumen de Blake3 (`capacity_soak_digest=...`)
  Afin que CI puede capturar y comparar la instantánea canónica.
- **Pénalités de sous-livraison:** Aplicaciones por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (même archivo). La prueba confirma las siguientes etapas de ataques, tiempos de reutilización,
  barras diagonales de garantía y contadores del libro mayor restent déterministes.

## Ejecución

Relance las validaciones de ubicación de remojo con:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Las pruebas se determinan en menos de un segundo en una computadora portátil estándar y no
nécessitent aucun accesorio externo.

## Observabilidad

Torii expone el mantenimiento de instantáneas de proveedores de crédito en los libros de contabilidad de tarifas junto con los paneles de control
puissent gate sur les faibles soldes et penales strikes:- RESTO: `GET /v2/sorafs/capacity/state` reenvío de platos principales `credit_ledger[*]` aquí
  refleja los campos del libro mayor verificados en la prueba de remojo. Ver
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importar archivos de seguimiento Grafana: `dashboards/grafana/sorafs_capacity_penalties.json`
  compteurs de strikes exportés, les totaux de pénalités et le colateral engagé afin que
  El equipo de guardia puede comparar las líneas base de remojo con los entornos en vivo.

## Suivi

- Planificador de ejecuciones de hebdomadaires de gate en CI para reactivar la prueba de remojo (nivel de humo).
- Étendre le tableau Grafana con les cibles de scrape Torii una vez que les exports de telemetry de production
  seront en ligne.