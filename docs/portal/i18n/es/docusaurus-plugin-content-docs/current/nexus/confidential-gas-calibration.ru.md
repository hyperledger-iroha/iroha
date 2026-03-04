---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Реестр калибровки конфиденциального газа
descripción: Измерения уровня релиза, подтверждающие график конфиденциального газа.
babosa: /nexus/calibracion-de-gas-confidencial
---

# Базовые линии калибровки конфиденциального газа

Este restablecimiento elimina los resultados de los calibres de gas confidenciales. Каждая строка документирует набор измерений уровня релиза, собранный по процедуре из [Activos confidenciales y transferencias ZK] (./confidential-assets#calibration-baselines--acceptance-gates).

| Datos (UTC) | Comprometerse | Perfil | `ns/op` | `gas/op` | `ns/gas` | Примечания |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | línea de base-neón | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (información del host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pendiente | línea base-simd-neutral | - | - | - | Запланированный нейтральный прогон x86_64 на CI-хосте `bench-x86-neon0`; см. billete GAS-214. Las respuestas se pueden eliminar después de guardarlas en un banco (lista de verificación previa a la fusión orientada a la versión 2.1). |
| 2026-04-13 | pendiente | línea de base-avx2 | - | - | - | Después de calibrar AVX2 con el comando commit/build, que es un programa neutro; требуется хост `bench-x86-avx2a`. El GAS-214 está conectado a otros dispositivos originales `baseline-neon`. |`ns/op` агрегирует медиану wall-clock на инструкцию, измеренную Criterion; `gas/op` - esta arifímética se cree que está diseñada para ser precisa en `iroha_core::gas::meter_instruction`; `ns/gas` Para realizar un resumen de las instrucciones de uso, deje un resumen de las instrucciones.

*Примечание.* Текущий arm64 хост по умолчанию не выводит сводки Criterio `raw.csv`; Conecte con `CRITERION_OUTPUT_TO=csv` o inicie una operación de upstream antes de realizar una resolución de temperatura, qué artefactos y archivos adjuntos приемки, были приложены. Si `target/criterion/` y cualquier otra versión después de `--save-baseline`, utilice un programa en el host de Linux o una consola de serie en релизный бандл как временный recurso provisional. Para ello, el registro de la consola arm64 se introdujo en `docs/source/confidential_assets_calibration_neon_20251018.log`.

Los medios de instalación son los siguientes (`cargo bench -p iroha_core --bench isi_gas_calibration`):

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
| Transferir activo | 3.68e5 | 180 | 2.04e3 |Колонка обеспечивается `gas::tests::calibration_bench_gas_snapshot` (всего 1,413 gas по набору из девяти инструкций) и вызовет ошибку, если будущие La medición se puede realizar sin el ajuste del calibre.