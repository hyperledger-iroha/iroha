---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Registro de calibración de gas confidencial
descripción: Medidas de liberación de calidad que etayent le calendrier de gas confidentiel.
babosa: /nexus/calibracion-de-gas-confidencial
---

# Líneas de base de calibración del gas confidencial

Este registro se adapta a las salidas válidas de los puntos de referencia de calibración del gas confidencial. Chaque ligne documente un juego de medidas de captura de liberación de calidad con el procedimiento decrite dans [Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates).

| Fecha (UTC) | Comprometerse | Perfil | `ns/op` | `gas/op` | `ns/gas` | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | línea de base-neón | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (información del host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pendiente | línea base-simd-neutral | - | - | - | Ejecución neutra x86_64 planificada en el hotel CI `bench-x86-neon0`; Ver billete GAS-214. Los resultados deben mejorarse una vez que la ventana de banco termine (la lista de verificación previa a la fusión con la versión 2.1). |
| 2026-04-13 | pendiente | línea de base-avx2 | - | - | - | Calibración AVX2 a continuación utiliza el meme commit/build que la ejecución es neutra; requiere el hotel `bench-x86-avx2a`. GAS-214 cubre las dos carreras con comparación delta contre `baseline-neon`. |`ns/op` agregue el reloj de pared mediano según las instrucciones medidas según Criterion; `gas/op` est la moyenne arithmetique des couts de Schedule correspondientes de `iroha_core::gas::meter_instruction`; `ns/gas` divida los nanosegundos algunos por el gas somme sur el conjunto de nuevas instrucciones.

*Nota.* L'hote arm64 actuel ne produit pas les resumes `raw.csv` de Criterion par defaut; Relancez avec `CRITERION_OUTPUT_TO=csv` ou una corrección aguas arriba antes de la liberación de los artefactos requeridos por la lista de verificación de aceptación adjunta. Si `target/criterion/` sigue nuevamente después de `--save-baseline`, capture la ejecución en un sistema Linux o serialice la salida de la consola en el paquete de lanzamiento como medida provisional. Un título de referencia, le log console arm64 de la última ejecución se encuentra en `docs/source/confidential_assets_calibration_neon_20251018.log`.

Medios por instrucción de ejecución del meme (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instrucción | media `ns/op` | horario `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegistrarDominio | 3.46e5 | 200 | 1.73e3 |
| RegistrarseCuenta | 3.15e5 | 200 | 1.58e3 |
| RegistrarAssetDef | 3.41e5 | 200 | 1.71e3 |
| Establecer cuentaKV_small | 3.28e5 | 67 | 4.90e3 |
| Función de cuenta de concesión | 3.33e5 | 96 | 3.47e3 |
| Revocar función de cuenta | 3.12e5 | 96 | 3.25e3 |
| EjecutarTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| Activo de menta | 1.56e5 | 150 | 1.04e3 |
| Transferir activo | 3.68e5 | 180 | 2.04e3 |El cronograma de columnas está impuesto por `gas::tests::calibration_bench_gas_snapshot` (un total de 1,413 gases en el conjunto de nuevas instrucciones) y se hace eco de las correcciones futuras que modifican la medición sin ajustar cada día los accesorios de calibración.