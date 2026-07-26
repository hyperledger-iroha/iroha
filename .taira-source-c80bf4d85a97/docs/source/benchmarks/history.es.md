---
lang: es
direction: ltr
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2026-01-03T18:08:00.425813+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Historial de captura de referencia de GPU (FASTPQ WP5-B)

Este archivo es generado por `python3 scripts/fastpq/update_benchmark_history.py`.
Cumple con el entregable FASTPQ Stage 7 WP5-B al rastrear cada GPU empaquetada
artefacto de referencia, el manifiesto del microbanco Poseidón y barridos auxiliares bajo
`benchmarks/`. Actualice las capturas subyacentes y vuelva a ejecutar el script cada vez que aparezca un nuevo
Las tierras agrupadas o la telemetría necesitan nueva evidencia.

## Alcance y proceso de actualización

- Producir o empaquetar nuevas capturas de GPU (a través de `scripts/fastpq/wrap_benchmark.py`),
  añádalos a la matriz de captura y vuelva a ejecutar este generador para actualizar el
  mesas.
- Cuando los datos del microbench Poseidon estén presentes, expórtelos con
  `scripts/fastpq/export_poseidon_microbench.py` y reconstruir el manifiesto usando
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- Registre los barridos de umbral de Merkle almacenando sus salidas JSON en
  `benchmarks/merkle_threshold/`; este generador enumera los archivos conocidos para que las auditorías
  puede hacer una referencia cruzada entre la disponibilidad de CPU y GPU.

## Puntos de referencia de GPU FASTPQ Etapa 7

| Paquete | Servidor | Modo | Parte trasera de la GPU | GPU disponible | Clase de dispositivo | GPU | LDE ms (CPU/GPU/SU) | Poseidón ms (CPU/GPU/SU) |
|-------|---------|------|-------------|---------------|--------------|-----|----------------------|---------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | GPU | cuda-sm80 | si | xeon-rtx | NVIDIA RTX 6000 Ada | 1512,9/880,7/1,72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | metales | GPU | ninguno | si | manzana-m4 | GPU Apple de 40 núcleos | 785,6/735,6/1,07 | 1803,8/1897,5/0,95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | metales | GPU | metales | si | manzana-m2-ultra | Apple M2 Ultra | 1581,1/1604,5/0,98 | 3589,9/3697,3/0,97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | metales | GPU | metales | si | manzana-m2-ultra | Apple M2 Ultra | 1804,5/1666,4/1,08 | 3939,5/4083,3/0,96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | metales | GPU | metales | si | manzana-m2-ultra | Apple M2 Ultra | 1804,5/1666,4/1,08 | 3939,5/4083,3/0,96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | abrircl | GPU | abrircl | si | neoverse-mi300 | AMD Instinto MI300A | 4518,5/688,9/6,56 | 2780,4/905,6/3,07 |

> Columnas: `Backend` se deriva del nombre del paquete; `Mode`/`GPU backend`/`GPU available`
> se copian del bloque `benchmarks` empaquetado para exponer fallas de CPU o GPU faltantes
> descubrimiento (por ejemplo, `gpu_backend=none` a pesar de `Mode=gpu`). SU = relación de aceleración (CPU/GPU).

## Instantáneas del microbanco Poseidon

`benchmarks/poseidon/manifest.json` agrega el Poseidón predeterminado versus escalar
Ejecuciones de microbench exportadas desde cada paquete de Metal. La siguiente tabla se actualiza con
el script del generador, para que las revisiones de CI y gobernanza puedan diferenciar las aceleraciones históricas
sin desempaquetar los informes FASTPQ empaquetados.

| Resumen | Paquete | Marca de tiempo | Ms predeterminado | Escalar ms | Aceleración |
|---------|--------|-----------|------------|-----------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167,7 | 2152.2 | 0,99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990,5 | 1994,5 | 1,00 |

## Barridos de umbral de MerkleCapturas de referencia recopiladas a través de
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
vivir bajo `benchmarks/merkle_threshold/`. Las entradas de la lista muestran si el host
Dispositivos metálicos expuestos cuando se realizó el barrido; Las capturas habilitadas para GPU deberían informar
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

La captura de Apple Silicon (`takemiyacStudio.lan_25.0.0_arm64`) es la base de GPU canónica utilizada en `docs/source/benchmarks.md`; las entradas de macOS 14 permanecen como líneas base solo de CPU para entornos que no pueden exponer dispositivos Metal.

## Instantáneas de uso de filas

Las decodificaciones de testigos capturadas a través de `scripts/fastpq/check_row_usage.py` prueban la transferencia
eficiencia de fila del dispositivo. Mantenga los artefactos JSON en `artifacts/fastpq_benchmarks/`
y este generador resumirá los índices de transferencia registrados para los auditores.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — lotes=2, ratio_transferencia promedio=0,629 (mín=0,625, máximo=0,633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — lotes=2, ratio_transferencia promedio=0,619 (mín=0,613, máximo=0,625)