---
lang: es
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2026-01-03T18:07:57.759135+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Líneas base de calibración de gas confidenciales

Este libro de contabilidad rastrea los resultados validados de la calibración de gas confidencial.
puntos de referencia. Cada fila documenta un conjunto de mediciones de calidad de liberación capturado con
el procedimiento descrito en `docs/source/confidential_assets.md#calibration-baselines--acceptance-gates`.

| Fecha (UTC) | Comprometerse | Perfil | `ns/op` | `gas/op` | `ns/gas` | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | línea de base-neón | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (información del host); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | línea de base-neón-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | Darwin 25.0.0 arm64 (`rustc 1.91.0`). Comando: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; inicie sesión en `docs/source/confidential_assets_calibration_neon_20260428.log`. Las ejecuciones de paridad x86_64 (SIMD-neutral + AVX2) están programadas para el espacio del laboratorio de Zurich del 2026-03-19; Los artefactos aterrizarán en `artifacts/confidential_assets_calibration/2026-03-x86/` con comandos coincidentes y se fusionarán en la tabla de referencia una vez capturados. |
| 2026-04-28 | — | línea base-simd-neutral | — | — | — | **Exento** en Apple Silicon: `ring` aplica NEON para la plataforma ABI, por lo que `RUSTFLAGS="-C target-feature=-neon"` falla antes de que se pueda ejecutar el banco (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). Los datos neutrales permanecen cerrados en el host CI `bench-x86-neon0`. |
| 2026-04-28 | — | línea de base-avx2 | — | — | — | **Diferido** hasta que haya un corredor x86_64 disponible. `arch -x86_64` no puede generar archivos binarios en esta máquina (“Tipo de CPU incorrecto en el ejecutable”; consulte `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`). El host de CI `bench-x86-avx2a` sigue siendo la fuente de registro. |

`ns/op` agrega el reloj de pared mediano por instrucción medida por Criterion;
`gas/op` es la media aritmética de los costos de programación correspondientes de
`iroha_core::gas::meter_instruction`; `ns/gas` divide los nanosegundos sumados por
el gas sumado en el conjunto de muestra de nueve instrucciones.

*Nota.* El host arm64 actual no emite resúmenes del Criterio `raw.csv` fuera de
la caja; Vuelva a ejecutar con `CRITERION_OUTPUT_TO=csv` o una solución ascendente antes de etiquetar un
liberación por lo que se adjuntan los artefactos requeridos por la lista de verificación de aceptación.
Si todavía falta `target/criterion/` después de `--save-baseline`, recopile la ejecución
en un host Linux o serializar la salida de la consola en el paquete de lanzamiento como un
solución temporal. Como referencia, el registro de la consola arm64 de la última ejecución
vive en `docs/source/confidential_assets_calibration_neon_20251018.log`.

Medianas por instrucción de la misma ejecución (`cargo bench -p iroha_core --bench isi_gas_calibration`):

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
| Transferir activo | 3.68e5 | 180 | 2.04e3 |

### 2026-04-28 (Apple Silicon, NEON habilitado)

Latencias medias para la actualización del 28 de abril de 2026 (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):| Instrucción | mediana `ns/op` | horario `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegistrarDominio | 8.58e6 | 200 | 4.29e4 |
| RegistrarseCuenta | 4.40e6 | 200 | 2.20e4 |
| RegistrarAssetDef | 4.23e6 | 200 | 2.12e4 |
| Establecer cuentaKV_small | 3.79e6 | 67 | 5.66e4 |
| Función de cuenta de concesión | 3.60e6 | 96 | 3.75e4 |
| Revocar función de cuenta | 3.76e6 | 96 | 3.92e4 |
| EjecutarTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| Activo de menta | 3.92e6 | 150 | 2.61e4 |
| Transferir activo | 3.59e6 | 180 | 1.99e4 |

Los agregados `ns/op` e `ns/gas` en la tabla anterior se derivan de la suma de
estas medianas (total `3.85717e7`ns en el conjunto de nueve instrucciones y 1,413
unidades de gas).

La columna de programación la aplica `gas::tests::calibration_bench_gas_snapshot`
(un total de 1.413 gases en el conjunto de nueve instrucciones) y se disparará si se actualizan futuros parches.
cambiar la medición sin actualizar los accesorios de calibración.

## Evidencia de telemetría del árbol de compromiso (M2.2)

Por tarea de hoja de ruta **M2.2**, cada ejecución de calibración debe capturar el nuevo
medidores de árbol de compromiso y contadores de desalojo para demostrar que la frontera de Merkle permanece
dentro de los límites configurados:

- `iroha_confidential_tree_commitments{asset_id}`
-`iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

Registre los valores inmediatamente antes y después de la carga de trabajo de calibración. un
un solo comando por activo es suficiente; ejemplo para `xor#wonderland`:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

Adjunte la salida sin procesar (o la instantánea Prometheus) al ticket de calibración para que el
El revisor de gobernanza puede confirmar que los límites del historial de raíces y los intervalos de los puntos de control son
honrado. La guía de telemetría en `docs/source/telemetry.md#confidential-tree-telemetry-m22`
amplía las expectativas de alerta y los paneles Grafana asociados.

Incluya los contadores de caché del verificador en el mismo borrador para que los revisores puedan confirmar
el índice de errores se mantuvo por debajo del umbral de advertencia del 40 %:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

Documente la relación derivada (`miss / (hit + miss)`) dentro de la nota de calibración.
para mostrar que los ejercicios de modelado de costos neutrales SIMD reutilizaron cachés calientes en lugar de
destruyendo el registro del verificador de Halo2.

## Neutral y exención AVX2

El Consejo SDK otorgó una exención temporal para la puerta PhaseC que requiere
Medidas `baseline-simd-neutral` e `baseline-avx2`:

- **SIMD-neutral:** En Apple Silicon, el backend criptográfico `ring` aplica NEON para
  Corrección del ABI. Deshabilitar la función (`RUSTFLAGS="-C target-feature=-neon"`)
  cancela la compilación antes de que se produzca el binario del banco (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`).
- **AVX2:** La cadena de herramientas local no puede generar archivos binarios x86_64 (`arch -x86_64 rustc -V`
  → “Tipo de CPU incorrecto en el ejecutable”; ver
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`).

Hasta que los hosts CI `bench-x86-neon0` e `bench-x86-avx2a` estén en línea, la ejecución NEON
anterior más la evidencia de telemetría satisfacen los criterios de aceptación de PhaseC.
La exención está registrada en `status.md` y se revisará una vez que el hardware x86 esté disponible.
disponible.