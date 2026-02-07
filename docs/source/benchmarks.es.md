---
lang: es
direction: ltr
source: docs/source/benchmarks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a5420a123c456aad264ceb70d744b20b09848f7dca23700b4ee1370144bb57c
source_last_modified: "2026-01-03T18:07:57.716816+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Informe de evaluación comparativa

Instantáneas detalladas por ejecución y el historial de FASTPQ WP5-B en vivo en
[`benchmarks/history.md`](benchmarks/history.md); use ese índice al adjuntar
artefactos hasta revisiones de hojas de ruta o auditorías de SRE. Regenerarlo con
`python3 scripts/fastpq/update_benchmark_history.py` cada vez que se captura una nueva GPU
o Poseidón manifiesta tierra.

## Paquete de evidencia de aceleración

Cada GPU o punto de referencia de modo mixto debe incluir la configuración de aceleración aplicada
por lo que WP6-B/WP6-C puede demostrar paridad de configuración junto con los artefactos de sincronización.

- Capture la instantánea del tiempo de ejecución antes/después de cada ejecución:
  `cargo xtask acceleration-state --format json > artifacts/acceleration_state_<stamp>.json`
  (use `--format table` para registros legibles por humanos). Esto registra `enable_{metal,cuda}`,
  Umbrales de Merkle, límites de sesgo de CPU SHA-2, bits de estado de backend detectados y cualquier
  errores de paridad persistentes o motivos de desactivación.
- Almacene el JSON junto a la salida del punto de referencia empaquetado
  (`artifacts/fastpq_benchmarks/*.json`, `benchmarks/poseidon/*.json`, barrido Merkle
  capturas, etc.) para que los revisores puedan diferenciar los tiempos y la configuración juntos.
- Las definiciones y valores predeterminados de las perillas se encuentran en `docs/source/config/acceleration.md`; cuando
  se aplican anulaciones (por ejemplo, `ACCEL_MERKLE_MIN_LEAVES_GPU`, `ACCEL_ENABLE_CUDA`),
  anótelos en los metadatos de ejecución para mantener las reejecuciones reproducibles en todos los hosts.

## Prueba comparativa de etapa 1 Norito (WP5-B/C)

- Comando: `cargo xtask stage1-bench [--size <bytes|Nk|Nm>]... [--iterations <n>]`
  emite JSON + Markdown bajo `benchmarks/norito_stage1/` con tiempos por tamaño
  para el constructor de índices estructurales escalar versus acelerado.
- Últimas ejecuciones (macOS aarch64, perfil de desarrollo) en vivo en
  `benchmarks/norito_stage1/latest.{json,md}` y el nuevo CSV de transición de
  `examples/stage1_cutover` (`benchmarks/norito_stage1/cutover.csv`) muestra SIMD
  gana desde ~6–8KiB en adelante. GPU/fase paralela 1 ahora tiene como valor predeterminado **192 KB**
  corte (`NORITO_STAGE1_GPU_MIN_BYTES=<n>` para anular) para evitar el lanzamiento
  en documentos pequeños y al mismo tiempo habilita aceleradores para cargas útiles más grandes.

## Enum vs envío de objetos de rasgo

- Tiempo de compilación (compilación de depuración): 16,58 s
- Tiempo de ejecución (Criterio, cuanto menor sea mejor):
  - `enum`: 386 ps (promedio)
  - `trait_object`: 1,56 ns (promedio)

Estas medidas provienen de un microbenchmark que compara un envío basado en enumeración con una implementación de objeto de rasgo en caja.

## Procesamiento por lotes Poseidon CUDA

El benchmark Poseidon (`crates/ivm/benches/bench_poseidon.rs`) ahora incluye cargas de trabajo que ejercitan permutaciones de hash único y los nuevos asistentes por lotes. Ejecute la suite con:

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

Criterion registrará los resultados bajo `target/criterion/poseidon*_many`. Cuando haya un trabajador de GPU disponible, exporte los resúmenes JSON (por ejemplo, copie `target/criterion/**/new/benchmark.json` en `benchmarks/poseidon/criterion_poseidon2_many_cuda.json`) (por ejemplo, copie `target/criterion/**/new/benchmark.json` en `benchmarks/poseidon/`) para que los equipos posteriores puedan comparar el rendimiento de CPU y CUDA para cada tamaño de lote. Hasta que se active el carril de GPU dedicado, el punto de referencia recurre a la implementación de SIMD/CPU y aún proporciona datos de regresión útiles para el rendimiento por lotes.

Para capturas repetibles (y para mantener la evidencia de paridad con los datos de tiempo), ejecute

```bash
cargo xtask poseidon-cuda-bench --json-out benchmarks/poseidon/poseidon_cuda_latest.json \
  --markdown-out benchmarks/poseidon/poseidon_cuda_latest.md --allow-overwrite
```que siembra lotes deterministas de Poseidon2/6, registra los motivos de salud/deshabilitación de CUDA, verifica
paridad contra el camino escalar, y emite resúmenes de operaciones/seg + aceleración junto con el Metal
estado de tiempo de ejecución (indicador de función, disponibilidad, último error). Los hosts solo de CPU todavía escriben el escalar
haga referencia y tenga en cuenta el acelerador que falta, para que CI pueda publicar artefactos incluso sin una GPU
corredor.

## Punto de referencia de metal FASTPQ (Apple Silicon)

El carril de GPU capturó una ejecución actualizada de un extremo a otro de `fastpq_metal_bench` en macOS 14 (arm64) con el conjunto de parámetros de carril equilibrado, 20 000 filas lógicas (rellenadas a 32 768) y 16 grupos de columnas. El artefacto envuelto se encuentra en `artifacts/fastpq_benchmarks/fastpq_metal_bench_20k_refresh.json`, con el rastro de metal almacenado junto con las capturas anteriores en `traces/fastpq_metal_trace_*_rows20000_iter5.trace`. Los tiempos promediados (de `benchmarks.operations[*]`) ahora dicen:

| Operación | Media de CPU (ms) | Media del metal (ms) | Aceleración (x) |
|-----------|---------------|-----------------|-------------|
| FFT (32.768 entradas) | 83,29 | 79,95 | 1.04 |
| IFFT (32.768 entradas) | 93,90 | 78,61 | 1.20 |
| LDE (262.144 entradas) | 669,54 | 657,67 | 1.02 |
| Columnas hash de Poseidón (524.288 entradas) | 29.087,53 | 30.004,90 | 0,97 |

Observaciones:

- Tanto FFT como IFFT se benefician de los núcleos BN254 actualizados (IFFT borra la regresión anterior en ~20%).
- LDE se mantiene cerca de la paridad; El relleno cero ahora registra 33.554.432 bytes rellenados con un promedio de 18,66 ms, por lo que el paquete JSON captura el impacto de la cola.
- El hash Poseidon todavía está vinculado a la CPU en este hardware; Siga comparando con los manifiestos del microbanco Poseidon hasta que la ruta Metal adopte los últimos controles de cola.
- Cada captura ahora registra `AccelerationSettings.runtimeState().metal.lastError`, lo que permite
  Los ingenieros anotan las reservas de CPU con el motivo de desactivación específico (alternancia de política,
  fallo de paridad, ningún dispositivo) directamente en el artefacto de referencia.

Para reproducir la ejecución, cree los núcleos Metal y ejecute:

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 --output fastpq_metal_bench_20k.json
```

Confirme el JSON resultante en `artifacts/fastpq_benchmarks/` junto con el rastro de Metal para que la evidencia del determinismo siga siendo reproducible.

## Automatización FASTPQ CUDA

Los hosts CUDA pueden ejecutar y ajustar el punto de referencia SM80 en un solo paso con:

```bash
cargo xtask fastpq-cuda-suite \
  --rows 20000 --iterations 5 --columns 16 \
  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \
  --label device_class=xeon-rtx --device rtx-ada
```

El ayudante invoca `fastpq_cuda_bench`, recorre etiquetas/dispositivo/notas, honra
`--require-gpu` y (de forma predeterminada) envuelve/señala a través de `scripts/fastpq/wrap_benchmark.py`.
Las salidas incluyen el JSON sin formato, el paquete empaquetado en `artifacts/fastpq_benchmarks/`,
y un `<name>_plan.json` al lado de la salida que registra los comandos/env exactos para que
Las capturas de la etapa 7 siguen siendo reproducibles en todos los ejecutores de GPU. Agregue `--sign-output` y
`--gpg-key <id>` cuando se requieren firmas; use `--dry-run` para emitir solo el
plano/caminos sin ejecutar el banco.

### Captura de versión GA (macOS 14 arm64, carril balanceado)

Para satisfacer WP2-D, también grabamos una versión de lanzamiento en el mismo host con GA-ready
heurística de cola y la publiqué como
`fastpq_metal_bench_20k_release_macos14_arm64.json`. El artefacto captura dos
lotes de columnas (equilibrados en carriles, acolchados a 32.768 filas) e incluyen Poseidón
Muestras de microbanco para consumo en tablero.| Operación | Media de CPU (ms) | Media del metal (ms) | Aceleración | Notas |
|-----------|---------------|-----------------|---------|-------|
| FFT (32.768 entradas) | 12.741 | 10.963 | 1,16 × | Los núcleos de GPU realizan un seguimiento de los umbrales de cola actualizados. |
| IFFT (32.768 entradas) | 17.499 | 25.688 | 0,68 × | La regresión se atribuye a la distribución conservadora de la cola; Sigue ajustando la heurística. |
| LDE (262.144 entradas) | 68.389 | 65.701 | 1,04× | El relleno cero registra 33.554.432 bytes en 9,651 ms para ambos lotes. |
| Columnas hash de Poseidón (524.288 entradas) | 1.728,835 | 1.447,076 | 1,19× | La GPU finalmente vence a la CPU después de los ajustes en la cola de Poseidon. |

Los valores del microbench Poseidon incrustados en el JSON muestran una aceleración de 1,10× (carril predeterminado
596,229 ms frente a 656,251 ms escalar en cinco iteraciones), por lo que los paneles ahora pueden generar gráficos
mejoras por carril junto al banco principal. Reproduzca la ejecución con:

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 \
  --output fastpq_metal_bench_20k_release_macos14_arm64.json
```

Mantenga los rastros JSON y `FASTPQ_METAL_TRACE_CHILD=1` empaquetados registrados en
`artifacts/fastpq_benchmarks/` para que las revisiones posteriores de WP2-D/WP2-E puedan diferenciar el GA
captura con ejecuciones de actualización anteriores sin volver a ejecutar la carga de trabajo.

Cada nueva captura `fastpq_metal_bench` ahora también escribe un bloque `bn254_metrics`,
que expone las entradas `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` para la CPU
línea de base y cualquier backend de GPU (Metal/CUDA) que estuviera activo, **y** un
Bloque `bn254_dispatch` que registra los anchos de grupo de subprocesos observados, subproceso lógico
recuentos y límites de canalización para los despachos BN254 FFT/LDE de una sola columna. el
El contenedor de referencia copia ambos mapas en `benchmarks.bn254_*`, por lo que los paneles y
Los exportadores de Prometheus pueden eliminar las latencias y la geometría etiquetadas sin volver a analizarlas
la matriz de operaciones sin procesar. La anulación `FASTPQ_METAL_THREADGROUP` ahora se aplica a
Kernels BN254 también, lo que hace que los barridos de grupos de subprocesos sean reproducibles desde una perilla.

Para mantener simples los paneles posteriores, ejecute `python3 scripts/benchmarks/export_csv.py`
después de capturar un paquete. El ayudante aplana `poseidon_microbench_*.json` en
hacer coincidir los archivos `.csv` para que los trabajos de automatización puedan diferenciar los carriles predeterminados y escalares sin
analizadores personalizados.

## Microbanco Poseidón (Metal)

`fastpq_metal_bench` ahora se vuelve a ejecutar en `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` y promueve los tiempos en `benchmarks.poseidon_microbench`. Exportamos las últimas capturas de Metal con `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <wrapped_json>` y las agregamos a través de `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`. Los resúmenes a continuación se encuentran bajo `benchmarks/poseidon/`:

| Resumen | Paquete envuelto | Media predeterminada (ms) | Media escalar (ms) | Aceleración vs escalar | Columnas x estados | Iteraciones |
|---------|----------------|-------------------|------------------|-------------------|------------------|------------|
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 1.990,49 | 1.994,53 | 1.002 | 64×262,144 | 5 |
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2.167,66 | 2.152,18 | 0,993 | 64×262,144 | 5 |Ambas capturas procesaron 262,144 estados por ejecución (trace log2 = 12) con una única iteración de calentamiento. El carril "predeterminado" corresponde al kernel multiestado sintonizado, mientras que "escalar" bloquea el kernel en un estado por carril para comparar.

## Barridos de umbral de Merkle

El ejemplo `merkle_threshold` (`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`) enfatiza las rutas de hash Merkle Metal-vs-CPU. La última captura de AppleSilicon (Darwin 25.0.0 arm64, `ivm::metal_available()=true`) se encuentra en `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` con una exportación CSV coincidente. Las líneas base de macOS 14 solo para CPU permanecen en `benchmarks/merkle_threshold/macos14_arm64_{cpu,metal}.json` para hosts sin Metal.

| Hojas | CPU mejor (ms) | Mejor metal (ms) | Aceleración |
|--------|---------------|-----------------|---------|
| 1.024 | 23.01 | 19,69 | 1,17× |
| 4.096 | 50,87 | 62.12 | 0,82 × |
| 8.192 | 95,77 | 96,57 | 0,99 × |
| 16.384 | 64,48 | 58,98 | 1,09 × |
| 32.768 | 109,49 | 87,68 | 1,25× |
| 65.536 | 177,72 | 137,93 | 1,29 × |

Los recuentos de hojas más grandes se benefician del Metal (1,09–1,29×); Los depósitos más pequeños aún se ejecutan más rápido en la CPU, por lo que el CSV mantiene ambas columnas para el análisis. El asistente CSV conserva el indicador `metal_available` junto a cada perfil para mantener alineados los paneles de regresión de GPU y CPU.

Pasos de reproducción:

```bash
cargo run --release -p ivm --features metal --example merkle_threshold -- --json \
  > benchmarks/merkle_threshold/<hostname>_$(uname -r)_$(uname -m).json
```

Configure `FASTPQ_METAL_LIB`/`FASTPQ_GPU` si el host requiere una habilitación de Metal explícita y mantenga marcadas las capturas de CPU y GPU para que WP1-F pueda trazar los umbrales de la política.

Cuando se ejecuta desde un shell sin cabeza, configure `IVM_DEBUG_METAL_ENUM=1` para registrar la enumeración de dispositivos y `IVM_FORCE_METAL_ENUM=1` para omitir `MTLCreateSystemDefaultDevice()`. La CLI calienta la sesión de CoreGraphics **antes** de solicitar el dispositivo Metal predeterminado y vuelve a `MTLCreateSystemDefaultDevice()` cuando `MTLCopyAllDevices()` devuelve cero; Si el host aún no informa ningún dispositivo, la captura conservará `metal_available=false` (las líneas base de CPU útiles se encuentran en `macos14_arm64_*`), mientras que los hosts de GPU deben mantener `FASTPQ_GPU=metal` habilitado para que el paquete registre el backend elegido.

`fastpq_metal_bench` expone una perilla similar a través de `FASTPQ_DEBUG_METAL_ENUM=1`, que imprime los resultados `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` antes de que el backend decida si permanecer en la ruta de la GPU. Habilítelo siempre que `FASTPQ_GPU=gpu` todavía informe `backend="none"` en el JSON empaquetado para que el paquete de captura registre exactamente cómo el host enumeró el hardware Metal; el arnés se cancela inmediatamente cuando se configura `FASTPQ_GPU=gpu` pero no se detecta ningún acelerador, apuntando a la perilla de depuración para que el paquete de lanzamiento nunca oculte un respaldo de CPU detrás de una ejecución forzada de GPU.

El asistente CSV emite tablas por perfil (por ejemplo, `macos14_arm64_*.csv` e `takemiyacStudio.lan_25.0.0_arm64.csv`), conservando el indicador `metal_available` para que los paneles de regresión puedan ingerir las mediciones de CPU y GPU sin analizadores personalizados.