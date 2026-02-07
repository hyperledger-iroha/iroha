---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Libro de calibración de gas confidencial
descripción: Médicos de calidad de liberación que sustentam o cronograma de gas confidencial.
babosa: /nexus/calibracion-de-gas-confidencial
---

# Líneas base de calibración de gas confidencial

Este registro acompaña los resultados validados de los benchmarks de calibración de gas confidencial. Cada línea documenta un conjunto de médicos de calidad de liberación capturada con el procedimiento descrito en [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| Datos (UTC) | Comprometerse | Perfil | `ns/op` | `gas/op` | `ns/gas` | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | línea de base-neón | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (información del host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pendiente | línea base-simd-neutral | - | - | - | Ejecución neutra x86_64 programada sin host CI `bench-x86-neon0`; ver billete GAS-214. Los resultados serán adicionados cuando a janela de bench terminar (una lista de verificación previa a la fusión mira la versión 2.1). |
| 2026-04-13 | pendiente | línea de base-avx2 | - | - | - | Calibración AVX2 de acompañamiento usando el mismo compromiso/construcción de ejecución neutra; solicitar el host `bench-x86-avx2a`. GAS-214 cobre como dos ejecutores con comparación de delta contra `baseline-neon`. |`ns/op` agrega a mediana de reloj de pared por instrucción medida según Criterion; `gas/op` e a media aritmetica dos custos de Schedule correspondientes de `iroha_core::gas::meter_instruction`; `ns/gas` divide los nanosegundos somados pelo gas somado no conjunto de nuevas instrucciones.

*Nota.* El host arm64 actualmente no emite resumos `raw.csv` do Criterion por padrao; rode novamente com `CRITERION_OUTPUT_TO=csv` ou uma correcao upstream antes de etiquetar um release para que os artefatos exigidos pela checklist de aceitacao sejam anexados. Si `target/criterion/` todavía está ausente después de `--save-baseline`, complete la ejecución en un host Linux o serialice dicha consola sin paquete de lanzamiento como solución temporal. Para referencia, el registro de la consola arm64 da la última ejecución en `docs/source/confidential_assets_calibration_neon_20251018.log`.

Medianas por instrucción de la mesma ejecutada (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instrucción | mediana `ns/op` | horario `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegistrarDominio | 3.46e5 | 200 | 1.73e3 |
| RegistrarseCuenta | 3.15e5 | 200 | 1.58e3 |
| RegistrarAssetDef | 3.41e5 | 200 | 1.71e3 |
| Establecer cuentaKV_small | 3.28e5 | 67 | 4.90e3 |
| Función de cuenta de concesión | 3.33e5 | 96 | 3.47e3 |
| Revocar función de cuenta | 3.12e5 | 96 | 3.25e3 |
| EjecutarTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| Activo de menta | 1.56e5 | 150 | 1.04e3 |
| Transferir activo | 3.68e5 | 180 | 2.04e3 |Un cronograma de columnas e impuesto por `gas::tests::calibration_bench_gas_snapshot` (total de 1,413 gas no conjunto de nuevas instrucciones) y vai falhar se parches futuros mudarem o metering sem actualizar os accesorios de calibracao.