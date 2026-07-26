---
lang: es
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-04T10:50:53.613193+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! Guía de migración de producción FASTPQ

Este runbook describe cómo validar el probador FASTPQ de producción de Stage6.
El backend de marcador de posición determinista se eliminó como parte de este plan de migración.
Complementa el plan por etapas en `docs/source/fastpq_plan.md` y supone que ya realiza un seguimiento
estado del espacio de trabajo en `status.md`.

## Audiencia y alcance
- Operadores validadores implementando el probador de producción en entornos de prueba o de red principal.
- Ingenieros de lanzamiento que crean binarios o contenedores que se enviarán con el backend de producción.
- Equipos de SRE/observabilidad cableando nuevas señales de telemetría y alertas.

Fuera del alcance: creación de contratos Kotodama y cambios de ABI IVM (consulte `docs/source/nexus.md` para obtener más información).
modelo de ejecución).

## Matriz de características
| Camino | Funciones de carga para habilitar | Resultado | Cuándo utilizar |
| ---- | ----------------------- | ------ | ----------- |
| Probador de producción (predeterminado) | _ninguno_ | Backend Stage6 FASTPQ con planificador FFT/LDE y canalización DEEP-FRI.【crates/fastpq_prover/src/backend.rs:1144】 | Valor predeterminado para todos los binarios de producción. |
| Aceleración de GPU opcional | `fastpq_prover/fastpq-gpu` | Habilita kernels CUDA/Metal con respaldo automático de CPU.【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | Hosts con aceleradores compatibles. |

## Procedimiento de construcción
1. **Compilación solo de CPU**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   El backend de producción se compila de forma predeterminada; no se requieren funciones adicionales.

2. **Compilación habilitada para GPU (opcional)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   La compatibilidad con GPU requiere un kit de herramientas CUDA SM80+ con `nvcc` disponible durante la compilación.【crates/fastpq_prover/Cargo.toml:11】

3. **Autopruebas**
   ```bash
   cargo test -p fastpq_prover
   ```
   Ejecute esto una vez por versión de lanzamiento para confirmar la ruta de Stage6 antes del empaquetado.

### Preparación de la cadena de herramientas de metal (macOS)
1. Instale las herramientas de línea de comandos de Metal antes de compilar: `xcode-select --install` (si faltan las herramientas CLI) e `xcodebuild -downloadComponent MetalToolchain` para recuperar la cadena de herramientas de GPU. El script de compilación invoca `xcrun metal`/`xcrun metallib` directamente y fallará rápidamente si los binarios están ausentes. 【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. Para validar la canalización antes de CI, puede reflejar el script de compilación localmente:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   Cuando esto tiene éxito, la compilación emite `FASTPQ_METAL_LIB=<path>`; el tiempo de ejecución lee ese valor para cargar metallib de forma determinista. 【crates/fastpq_prover/build.rs:188】 【crates/fastpq_prover/src/metal.rs:43】
3. Configure `FASTPQ_SKIP_GPU_BUILD=1` cuando realice una compilación cruzada sin la cadena de herramientas Metal; la compilación imprime una advertencia y el planificador permanece en la ruta de la CPU.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Los nodos recurren automáticamente a la CPU si Metal no está disponible (falta el marco, GPU no compatible o `FASTPQ_METAL_LIB` vacío); el script de compilación borra la var env y el planificador registra la degradación. 【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】### Lista de verificación de liberación (Etapa 6)
Mantenga bloqueado el boleto de lanzamiento de FASTPQ hasta que todos los elementos a continuación estén completos y adjuntos.

1. **Métricas de prueba de menos de un segundo**: inspeccione el `fastpq_metal_bench_*.json` recién capturado y
   confirme la entrada `benchmarks.operations` donde `operation = "lde"` (y el espejo
   Muestra `report.operations`) informa `gpu_mean_ms ≤ 950` para la carga de trabajo de 20000 filas (32768 rellenos).
   filas). Las capturas fuera del límite máximo requieren repeticiones antes de que se pueda firmar la lista de verificación.
2. **Manifiesto firmado** — Ejecutar
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   por lo que el boleto de liberación lleva tanto el manifiesto como su firma separada
   (`artifacts/fastpq_bench_manifest.sig`). Los revisores verifican el par resumen/firma antes
   promocionando un lanzamiento. 【xtask/src/fastpq.rs:128】 【xtask/src/main.rs:845】 El manifiesto de matriz (construido
   a través de `scripts/fastpq/capture_matrix.sh`) ya codifica el piso de fila de 20k y
   depurar una regresión.
3. **Adjuntos de evidencia**: cargue el JSON de referencia de Metal, el registro de salida estándar (o el seguimiento de instrumentos).
   Salidas del manifiesto CUDA/Metal y la firma separada del ticket de lanzamiento. La entrada de la lista de verificación
   debe vincularse a todos los artefactos más la huella digital de la clave pública utilizada para firmar, de modo que las auditorías posteriores
   Puede reproducir el paso de verificación. 【artifacts/fastpq_benchmarks/README.md:65】### Flujo de trabajo de validación de metales
1. Después de una compilación habilitada para GPU, confirme los puntos `FASTPQ_METAL_LIB` en un `.metallib` (`echo $FASTPQ_METAL_LIB`) para que el tiempo de ejecución pueda cargarlo de manera determinista.【crates/fastpq_prover/build.rs:188】
2. Ejecute el paquete de paridad con los carriles de GPU activados:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. El backend ejercitará los núcleos metálicos y registrará un respaldo determinista de la CPU si falla la detección.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. Capture una muestra de referencia para paneles:\
   localice la biblioteca Metal compilada (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   expórtelo a través de `FASTPQ_METAL_LIB` y ejecútelo\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  El perfil canónico `fastpq-lane-balanced` ahora rellena cada captura a 32,768 filas (2¹⁵), por lo que el JSON lleva tanto `rows` como `padded_rows` junto con la latencia Metal LDE; Vuelva a ejecutar la captura si `zero_fill` o la configuración de la cola empujan el LDE de la GPU más allá del objetivo de 950 ms (<1 s) en los hosts de la serie AppleM. Archive el JSON/registro resultante junto con otras pruebas de publicación; el flujo de trabajo nocturno de macOS realiza la misma ejecución y carga sus artefactos para comparar. 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  Cuando necesite telemetría exclusiva de Poseidon (por ejemplo, para registrar un seguimiento de Instrumentos), agregue `--operation poseidon_hash_columns` al comando anterior; el banco seguirá respetando `FASTPQ_GPU=gpu`, emitirá `metal_dispatch_queue.poseidon` e incluirá el nuevo bloque `poseidon_profiles` para que el paquete de lanzamiento documente explícitamente el cuello de botella de Poseidón.
  La evidencia ahora incluye `zero_fill.{bytes,ms,queue_delta}` más `kernel_profiles` (por kernel
  ocupación, GB/s estimados y estadísticas de duración) para que la eficiencia de la GPU se pueda graficar sin
  reprocesamiento de trazas sin procesar y un bloque `twiddle_cache` (aciertos/fallos + `before_ms`/`after_ms`) que
  prueba que las cargas de twiddle en caché están vigentes. `--trace-dir` relanza el arnés debajo
  `xcrun xctrace record` y
  almacena un archivo `.trace` con marca de tiempo junto con el JSON; todavía puedes proporcionar uno personalizado
  `--trace-output` (con `--trace-template` / `--trace-seconds` opcional) al capturar a un
  ubicación/plantilla personalizada. El JSON registra `metal_trace_{template,seconds,output}` para auditoría.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】Después de cada captura, ejecute `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` para que la publicación lleve metadatos del host (que ahora incluye `metadata.metal_trace`) para el paquete de alertas/placa Grafana (`dashboards/grafana/fastpq_acceleration.json`, `dashboards/alerts/fastpq_acceleration_rules.yml`). El informe ahora incluye un objeto `speedup` por operación (`speedup.ratio`, `speedup.delta_ms`), el contenedor eleva `zero_fill_hotspots` (bytes, latencia, GB/s derivados y los contadores delta de cola de metal), aplana `kernel_profiles` en `benchmarks.kernel_summary`, mantiene intacto el bloque `twiddle_cache`, copia el nuevo bloque/resumen `post_tile_dispatches` para que los revisores puedan probar que el kernel multipaso se ejecutó durante la captura y ahora resume la evidencia del microbench Poseidon en `benchmarks.poseidon_microbench` para que los paneles puedan citar la latencia escalar frente a la predeterminada sin volver a analizar el informe en bruto. La puerta de manifiesto lee el mismo bloque y rechaza los paquetes de evidencia de GPU que lo omiten, lo que obliga a los operadores a actualizar las capturas cada vez que se omite o se omite la ruta posterior al mosaico. mal configurado.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq.rs:280】
  El núcleo Poseidon2 Metal comparte las mismas perillas: `FASTPQ_METAL_POSEIDON_LANES` (32–256, potencias de dos) e `FASTPQ_METAL_POSEIDON_BATCH` (1–32 estados por carril) le permiten fijar el ancho de lanzamiento y el trabajo por carril sin reconstruir; el host pasa esos valores a través de `PoseidonArgs` antes de cada envío. De forma predeterminada, el tiempo de ejecución inspecciona `MTLDevice::{is_low_power,is_headless,location}` para desviar las GPU discretas hacia lanzamientos por niveles de VRAM (`256×24` cuando se informa ≥48 GiB, `256×20` a 32 GiB, `256×16` en caso contrario) mientras los SoC de bajo consumo permanecen encendidos. `256×8` (y las piezas más antiguas de 128/64 carriles se adhieren a 8/6 estados por carril), por lo que la mayoría de los operadores nunca necesitan configurar las variables de entorno manualmente. bajo `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` y emite un bloque `poseidon_microbench` que registra ambos perfiles de lanzamiento más la velocidad medida versus el carril escalar para que los paquetes de lanzamiento puedan demostrar que el nuevo kernel en realidad reduce `poseidon_hash_columns`, e incluye el bloque `poseidon_pipeline` para que la evidencia de Stage7 capture las perillas de profundidad/superposición de fragmentos junto con los nuevos niveles de ocupación. Deje el entorno sin configurar para ejecuciones normales; el arnés administra la reejecución automáticamente, registra fallas si la captura secundaria no se puede ejecutar y sale inmediatamente cuando se configura `FASTPQ_GPU=gpu` pero no hay backend de GPU disponible, por lo que los respaldos silenciosos de la CPU nunca se cuelan en el rendimiento. artefactos.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】El contenedor rechaza las capturas de Poseidon a las que les falta el delta `metal_dispatch_queue.poseidon`, los contadores `column_staging` compartidos o los bloques de evidencia `poseidon_profiles`/`poseidon_microbench`, por lo que los operadores deben actualizar cualquier captura que no pueda demostrar la superposición de la puesta en escena o el escalar versus el valor predeterminado. speedup.【scripts/fastpq/wrap_benchmark.py:732】 Cuando necesite un JSON independiente para paneles o deltas de CI, ejecute `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`; el asistente acepta tanto artefactos empaquetados como capturas `fastpq_metal_bench*.json` sin procesar, emitiendo `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` con los tiempos predeterminados/escalares, metadatos de ajuste y aceleración registrada.【scripts/fastpq/export_poseidon_microbench.py:1】
  Termine la ejecución ejecutando `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` para que la lista de verificación de lanzamiento de Stage6 aplique el límite máximo de LDE `<1 s` y emita un paquete de manifiesto/resumen firmado que se envía con el ticket de lanzamiento.
4. Verifique la telemetría antes de la implementación: enrolle el punto final Prometheus (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) e inspeccione los registros `telemetry::fastpq.execution_mode` en busca de `resolved="cpu"` inesperados. entradas.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. Documente la ruta alternativa de la CPU forzándola intencionalmente (`FASTPQ_GPU=cpu` o `zk.fastpq.execution_mode = "cpu"`) para que los libros de jugadas de SRE permanezcan alineados con el comportamiento determinista.6. Ajuste opcional: de forma predeterminada, el host selecciona 16 carriles para trazas cortas, 32 para medias y 64/128 una vez `log_len ≥ 10/14`, aterrizando en 256 cuando `log_len ≥ 18`, y ahora mantiene el mosaico de memoria compartida en cinco etapas para trazas pequeñas, cuatro una vez `log_len ≥ 12` y 14/12/16. etapas para `log_len ≥ 18/20/22` antes de enviar el trabajo al kernel posterior al mosaico. Exporte `FASTPQ_METAL_FFT_LANES` (potencia de dos entre 8 y 256) y/o `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) antes de ejecutar los pasos anteriores para anular esas heurísticas. Los tamaños de lote de columnas FFT/IFFT y LDE se derivan del ancho del grupo de subprocesos resuelto (≈2048 subprocesos lógicos por envío, con un límite de 32 columnas y ahora disminuyendo hasta 32→16→8→4→2→1 a medida que crece el dominio), mientras que la ruta LDE aún aplica sus límites de dominio; configure `FASTPQ_METAL_FFT_COLUMNS` (1–32) para fijar un tamaño de lote FFT determinista y `FASTPQ_METAL_LDE_COLUMNS` (1–32) para aplicar la misma anulación al despachador LDE cuando necesite comparaciones bit por bit entre hosts. La profundidad del mosaico LDE también refleja la heurística FFT: los seguimientos con `log₂ ≥ 18/20/22` solo ejecutan 10/12/8 etapas de memoria compartida antes de entregar las mariposas anchas al kernel posterior al mosaico, y puede anular ese límite a través de `FASTPQ_METAL_LDE_TILE_STAGES` (1–32). El tiempo de ejecución pasa todos los valores a través de los argumentos del kernel de Metal, fija las anulaciones no admitidas y registra los valores resueltos para que los experimentos sigan siendo reproducibles sin reconstruir metallib; el JSON de referencia muestra tanto el ajuste resuelto como el presupuesto de llenado cero del host (`zero_fill.{bytes,ms,queue_delta}`) capturado a través de estadísticas LDE para que los deltas de cola estén vinculados directamente a cada captura, y ahora agrega un bloque `column_staging` (lotes aplanados, flatten_ms, wait_ms, wait_ratio) para que los revisores puedan verificar la superposición de host/dispositivo introducida por la canalización de doble búfer. Cuando la GPU se niega a informar la telemetría de llenado cero, el arnés ahora sintetiza una temporización determinista de los borrados del búfer del lado del host y la inyecta en el bloque `zero_fill` para que la evidencia de liberación nunca se envíe sin la campo.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. El envío de colas múltiples es automático en Mac discretas: cuando `Device::is_low_power()` devuelve falso o el dispositivo metálico informa una ubicación de ranura/externa, el host crea una instancia de dos `MTLCommandQueue`, solo se distribuye una vez que la carga de trabajo lleva ≥16 columnas (escaladas según la distribución) y realiza operaciones por turnos en los lotes de columnas en las colas, de modo que los seguimientos largos mantienen ambos carriles de GPU ocupados sin comprometer determinismo. Anule la política con `FASTPQ_METAL_QUEUE_FANOUT` (1 a 4 colas) e `FASTPQ_METAL_COLUMN_THRESHOLD` (total mínimo de columnas antes de la distribución) siempre que necesite capturas reproducibles en todas las máquinas; las pruebas de paridad fuerzan esas anulaciones para que las Mac con varias GPU permanezcan cubiertas y el despliegue/umbral resuelto se registre junto a la telemetría de profundidad de la cola.### Evidencia para archivar
| Artefacto | Captura | Notas |
|----------|---------|-------|
| Paquete `.metallib` | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` e `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` seguidos de `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` e `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | Demuestra que Metal CLI/toolchain se instaló y produjo una biblioteca determinista para esta confirmación.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| Instantánea del entorno | `echo $FASTPQ_METAL_LIB` después de la compilación; mantenga el camino absoluto con su boleto de liberación. | La salida vacía significa que el metal estaba deshabilitado; registrar los documentos de valor que los carriles de GPU permanecen disponibles en el artefacto de envío.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| Registro de paridad de GPU | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` y archive el fragmento que contiene `backend="metal"` o la advertencia de degradación. | Demuestra que los kernels se ejecutan (o retroceden de manera determinista) antes de promocionar la compilación.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| Producción de referencia | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; envolver y firmar a través de `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`. | Los registros JSON envueltos `speedup.ratio`, `speedup.delta_ms`, ajuste FFT, filas rellenas (32,768), `zero_fill`/`kernel_profiles` enriquecido, el `kernel_summary` aplanado, el verificado Bloques `metal_dispatch_queue.poseidon`/`poseidon_profiles` (cuando se usa `--operation poseidon_hash_columns`), y los metadatos de seguimiento para que la media LDE de GPU permanezca ≤950 ms y Poseidon permanezca <1 s; mantenga tanto el paquete como la firma `.json.asc` generada con el ticket de lanzamiento para que los paneles y los auditores puedan verificar el artefacto sin volver a ejecutar las cargas de trabajo. |
| Manifiesto del banco | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | Valida ambos artefactos de GPU, falla si la media LDE supera el límite `<1 s`, registra resúmenes de BLAKE3/SHA-256 y emite un manifiesto firmado para que la lista de verificación de lanzamiento no pueda avanzar sin métricas verificables.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| Paquete CUDA | Ejecute `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` en el host de laboratorio SM80, ajuste/firme el JSON en `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` (use `--label device_class=xeon-rtx-sm80` para que los paneles seleccionen la clase correcta), agregue la ruta a `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` y mantenga el par `.json`/`.asc` con el artefacto de metal antes de regenerar el manifiesto. El `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` registrado ilustra el formato de paquete exacto que esperan los auditores. 【scripts/fastpq/wrap_benchmark.py:714】 【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】 |
| Prueba de telemetría | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` más el registro `telemetry::fastpq.execution_mode` emitido al inicio. | Confirma que Prometheus/OTEL expone `device_class="<matrix>", backend="metal"` (o un registro de degradación) antes de habilitar el tráfico.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】 || Taladro de CPU forzado | Ejecute un lote corto con `FASTPQ_GPU=cpu` o `zk.fastpq.execution_mode = "cpu"` y capture el registro de degradación. | Mantiene los runbooks de SRE alineados con la ruta de retorno determinista en caso de que sea necesaria una reversión a mitad del lanzamiento. 【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| Captura de seguimiento (opcional) | Repita una prueba de paridad con `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` y guarde el seguimiento de envío emitido. | Conserva la evidencia de ocupación/grupo de subprocesos para revisiones posteriores de perfiles sin volver a ejecutar los puntos de referencia.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

Los archivos multilingües `fastpq_plan.*` hacen referencia a esta lista de verificación para que los operadores de puesta en escena y producción sigan el mismo rastro de evidencia.【docs/source/fastpq_plan.md:1】

## Construcciones reproducibles
Utilice el flujo de trabajo del contenedor fijado para producir artefactos Stage6 reproducibles:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

El script auxiliar crea la imagen de la cadena de herramientas `rust:1.88.0-slim-bookworm` (e `nvidia/cuda:12.2.2-devel-ubuntu22.04` para GPU), ejecuta la compilación dentro del contenedor y escribe `manifest.json`, `sha256s.txt` y los archivos binarios compilados en la salida de destino. directorio.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

Anulaciones del entorno:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN`: fije una base/etiqueta Rust explícita.
- `FASTPQ_CUDA_IMAGE`: cambie la base CUDA al producir artefactos de GPU.
- `FASTPQ_CONTAINER_RUNTIME` – fuerza un tiempo de ejecución específico; predeterminado `auto` intenta `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`: orden de preferencia separado por comas para la detección automática en tiempo de ejecución (el valor predeterminado es `docker,podman,nerdctl`).

## Actualizaciones de configuración
1. Configure el modo de ejecución en tiempo de ejecución en su TOML:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   El valor se analiza a través de `FastpqExecutionMode` y se introduce en el backend al inicio. 【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. Anular en el lanzamiento si es necesario:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   Las anulaciones de CLI mutan la configuración resuelta antes de que se inicie el nodo.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Los desarrolladores pueden forzar temporalmente la detección sin tocar las configuraciones exportando
   `FASTPQ_GPU={auto,cpu,gpu}` antes de lanzar el binario; se registra la anulación y la canalización
   todavía aparece el modo resuelto.【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## Lista de verificación de verificación
1. **Registros de inicio**
   - Espere `FASTPQ execution mode resolved` del objetivo `telemetry::fastpq.execution_mode` con
     Etiquetas `requested`, `resolved` e `backend`.【crates/fastpq_prover/src/backend.rs:208】
   - En la detección automática de GPU, un registro secundario de `fastpq::planner` informa el carril final.
   - Los hosts metálicos salen a la superficie `backend="metal"` cuando metallib se carga correctamente; Si la compilación o la carga fallan, el script de compilación emite una advertencia, borra `FASTPQ_METAL_LIB` y el planificador registra `GPU acceleration unavailable` antes de continuar. CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/src/metal.rs:43】2. **Métricas Prometheus**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   El contador se incrementa a través de `record_fastpq_execution_mode` (ahora etiquetado por
   `{device_class,chip_family,gpu_kind}`) cada vez que un nodo resuelve su ejecución
   modo.【crates/iroha_telemetry/src/metrics.rs:8887】
   - Para cobertura de Metal confirmar
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     incrementos junto con sus paneles de implementación. 【crates/iroha_telemetry/src/metrics.rs:5397】
   - Los nodos macOS compilados con `irohad --features fastpq-gpu` también exponen
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     y
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` para que los paneles de Stage7
     puede rastrear el ciclo de trabajo y el margen de cola de los scrapes Prometheus en vivo. 【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **Exportación de telemetría**
   - Las construcciones OTEL emiten `fastpq.execution_mode_resolutions_total` con las mismas etiquetas; asegura tu
     Los paneles o alertas detectan `resolved="cpu"` inesperados cuando las GPU deberían estar activas.

4. **Prueba/verifica la cordura**
   - Ejecute un lote pequeño a través de `iroha_cli` o un arnés de integración y confirme las pruebas, verifique en un
     peer compilado con los mismos parámetros.

## Solución de problemas
- **El modo resuelto mantiene la CPU en los hosts de GPU**: verifique que el binario se haya creado con
  `fastpq_prover/fastpq-gpu`, las bibliotecas CUDA están en la ruta del cargador y `FASTPQ_GPU` no está forzando
  `cpu`.
- **Metal no disponible en Apple Silicon**: verifique que las herramientas CLI estén instaladas (`xcode-select --install`), vuelva a ejecutar `xcodebuild -downloadComponent MetalToolchain` y asegúrese de que la compilación genere una ruta `FASTPQ_METAL_LIB` que no esté vacía; un valor vacío o faltante desactiva el backend por diseño.【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **Errores `Unknown parameter`**: asegúrese de que tanto el probador como el verificador utilicen el mismo catálogo canónico
  emitido por `fastpq_isi`; no coincide con la superficie como `Error::UnknownParameter`.【crates/fastpq_prover/src/proof.rs:133】
- **Retroceso inesperado de la CPU**: inspeccione `cargo tree -p fastpq_prover --features` y
  confirme que `fastpq_prover/fastpq-gpu` esté presente en las compilaciones de GPU; verifique que las bibliotecas `nvcc`/CUDA estén en la ruta de búsqueda.
- **Falta el contador de telemetría**: verifique que el nodo se inició con `--features telemetry` (predeterminado)
  y que la exportación OTEL (si está habilitada) incluye la tubería métrica.【crates/iroha_telemetry/src/metrics.rs:8887】

## Procedimiento alternativo
Se ha eliminado el backend determinista del marcador de posición. Si una regresión requiere reversión,
Vuelva a implementar los artefactos de lanzamiento previamente conocidos e investigue antes de volver a publicar Stage6.
binarios. Documente la decisión de gestión de cambios y asegúrese de que el avance se complete solo después de la
Se entiende la regresión.

3. Supervise la telemetría para garantizar que `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` refleje lo esperado
   ejecución del marcador de posición.

## Línea base de hardware
| Perfil | CPU | GPU | Notas |
| ------- | --- | --- | ----- |
| Referencia (Etapa 6) | AMD EPYC7B12 (32 núcleos), 256 GiB de RAM | NVIDIA A10040GB (CUDA12.2) | Los lotes sintéticos de 20000 filas deben completarse ≤1000 ms.【docs/source/fastpq_plan.md:131】 |
| Sólo CPU | ≥32 núcleos físicos, AVX2 | – | Espere entre 0,9 y 1,2 segundos para 20 000 filas; mantenga `execution_mode = "cpu"` para determinismo. |## Pruebas de regresión
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (en hosts GPU)
- Control de accesorios dorados opcional:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

Documente cualquier desviación de esta lista de verificación en su runbook de operaciones y actualice `status.md` después de la
Se completa la ventana de migración.