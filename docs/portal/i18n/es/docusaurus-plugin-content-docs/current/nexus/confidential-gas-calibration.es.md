---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Libro mayor de calibracion de gas confidencial
descripción: Mediciones de calidad de liberación que respaldan el calendario de gas confidencial.
babosa: /nexus/calibracion-de-gas-confidencial
---

# Lineas base de calibracion de gas confidencial

Este registro rastrea los resultados validados de los benchmarks de calibración de gas confidencial. Cada fila documenta un conjunto de mediciones de calidad de liberación capturada con el procedimiento descrito en [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| Fecha (UTC) | Comprometerse | Perfil | `ns/op` | `gas/op` | `ns/gas` | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | línea de base-neón | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (información del host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pendiente | línea base-simd-neutral | - | - | - | Ejecucion neutral x86_64 programada en el host CI `bench-x86-neon0`; ver billete GAS-214. Los resultados se agregarán cuando termine la ventana de bench (la checklist pre-merge apunta al lanzamiento 2.1). |
| 2026-04-13 | pendiente | línea de base-avx2 | - | - | - | Calibracion AVX2 posterior usando el mismo commit/build que la corrida neutral; requiere el host `bench-x86-avx2a`. GAS-214 cubre ambas corridas con comparación de delta contra `baseline-neon`. |`ns/op` agregue la mediana de tiempo de pared según instrucciones medida por Criterion; `gas/op` es la media aritmética de los costos de programación correspondientes de `iroha_core::gas::meter_instruction`; `ns/gas` divide los nanosegundos sumados entre el gas sumado en el conjunto de nueve instrucciones.

*Nota.* El host arm64 actual no emite resúmenes `raw.csv` de Criterion por defecto; Vuelve a ejecutar con `CRITERION_OUTPUT_TO=csv` o una corrección upstream antes de etiquetar un lanzamiento para que los artefactos requeridos por la lista de verificación de aceptación queden adjuntos. Si `target/criterion/` sigue faltando después de `--save-baseline`, recolecta la corrida en un host Linux o serializa la salida de consola en el paquete del lanzamiento como stopgap temporal. Como referencia, el log de consola arm64 de la ultima corrida vive en `docs/source/confidential_assets_calibration_neon_20251018.log`.

Medianas por instrucción de la misma corrida (`cargo bench -p iroha_core --bench isi_gas_calibration`):| Instrucción | mediana `ns/op` | horario `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegistrarDominio | 3.46e5 | 200 | 1.73e3 |
| RegistrarseCuenta | 3.15e5 | 200 | 1.58e3 |
| RegistrarAssetDef | 3.41e5 | 200 | 1.71e3 |
| Establecer cuentaKV_small | 3.28e5 | 67 | 4.90e3 |
| Función de cuenta de concesión | 3.33e5 | 96 | 3.47e3 |
| Revocar función de cuenta | 3.12e5 | 96 | 3.25e3 |
| EjecutarTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| Activo de menta | 1.56e5 | 150 | 1.04e3 |
| Transferir activo | 3.68e5 | 180 | 2.04e3 |

La columna de cronograma esta impuesta por `gas::tests::calibration_bench_gas_snapshot` (total 1,413 gas en el conjunto de nueve instrucciones) y fallara si parches futuros cambian la medición sin actualizar los accesorios de calibración.