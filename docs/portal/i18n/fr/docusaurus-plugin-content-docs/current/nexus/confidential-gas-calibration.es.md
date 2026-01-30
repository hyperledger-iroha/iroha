---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Libro mayor de calibracion de gas confidencial
description: Mediciones de calidad de release que respaldan el calendario de gas confidencial.
slug: /nexus/confidential-gas-calibration
---

# Lineas base de calibracion de gas confidencial

Este registro rastrea los resultados validados de los benchmarks de calibracion de gas confidencial. Cada fila documenta un conjunto de mediciones de calidad de release capturado con el procedimiento descrito en [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| Fecha (UTC) | Commit | Perfil | `ns/op` | `gas/op` | `ns/gas` | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pending | baseline-simd-neutral | - | - | - | Ejecucion neutral x86_64 programada en el host CI `bench-x86-neon0`; ver ticket GAS-214. Los resultados se agregaran cuando termine la ventana de bench (la checklist pre-merge apunta al release 2.1). |
| 2026-04-13 | pending | baseline-avx2 | - | - | - | Calibracion AVX2 posterior usando el mismo commit/build que la corrida neutral; requiere el host `bench-x86-avx2a`. GAS-214 cubre ambas corridas con comparacion de delta contra `baseline-neon`. |

`ns/op` agrega la mediana de tiempo de pared por instruccion medida por Criterion; `gas/op` es la media aritmetica de los costos de schedule correspondientes de `iroha_core::gas::meter_instruction`; `ns/gas` divide los nanosegundos sumados entre el gas sumado en el conjunto de nueve instrucciones.

*Nota.* El host arm64 actual no emite resumenes `raw.csv` de Criterion por defecto; vuelve a ejecutar con `CRITERION_OUTPUT_TO=csv` o una correccion upstream antes de etiquetar un release para que los artefactos requeridos por la checklist de aceptacion queden adjuntos. Si `target/criterion/` sigue faltando despues de `--save-baseline`, recolecta la corrida en un host Linux o serializa la salida de consola en el bundle del release como stopgap temporal. Como referencia, el log de consola arm64 de la ultima corrida vive en `docs/source/confidential_assets_calibration_neon_20251018.log`.

Medianas por instruccion de la misma corrida (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instruccion | mediana `ns/op` | schedule `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| RegisterAccount | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

La columna de schedule esta impuesta por `gas::tests::calibration_bench_gas_snapshot` (total 1,413 gas en el conjunto de nueve instrucciones) y fallara si parches futuros cambian el metering sin actualizar los fixtures de calibracion.
