---
lang: es
direction: ltr
source: docs/source/metal_neon_acceleration_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 628eb2c7776bf818a310dd4bae51e3fc655f92e885d0cd9da7ff487fd9128102
source_last_modified: "2026-01-03T18:08:01.870300+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Plan de aceleración de metales y NEON (Swift & Rust)

Este documento captura el plan compartido para habilitar hardware determinista.
aceleración (Metal GPU + NEON/Accelerate SIMD + integración StrongBox) en
el espacio de trabajo de Rust y el SDK de Swift. Aborda los elementos de la hoja de ruta rastreados.
en **Hardware Acceleration Workstream (macOS/iOS)** y proporciona una transferencia
artefacto para el equipo Rust IVM, propietarios de puentes Swift y herramientas de telemetría.

> Última actualización: 2026-01-12  
> Propietarios: IVM Performance TL, líder de Swift SDK

## Metas

1. Reutilice los núcleos de GPU Rust (Poseidon/BN254/CRC64) en hardware de Apple a través de Metal
   Computa sombreadores con paridad determinista contra rutas de CPU.
2. Exponer los conmutadores de aceleración (`AccelerationConfig`) de un extremo a otro para que las aplicaciones Swift
   puede optar por Metal/NEON/StrongBox conservando las garantías de paridad/ABI.
3. Instrumentar CI + paneles de control para mostrar datos de paridad/comparación y marcar
   regresiones entre rutas de CPU frente a GPU/SIMD.
4. Comparta lecciones de StrongBox/enclave seguro entre Android (AND2) y Swift
   (IOS4) para mantener los flujos de firmas alineados de manera determinista.

**Actualización (CRC64 + actualización de etapa 1):** Los ayudantes de GPU CRC64 ahora están conectados a `norito::core::hardware_crc64` con un límite predeterminado de 192 KB (anulación a través de `NORITO_GPU_CRC64_MIN_BYTES` o la ruta de ayuda explícita `NORITO_CRC64_GPU_LIB`) mientras se conservan los respaldos SIMD y escalares. Se volvieron a comparar los cortes JSON Stage-1 (`examples/stage1_cutover` → `benchmarks/norito_stage1/cutover.csv`), manteniendo el corte escalar en 4 KB y alineando el valor predeterminado de la GPU Stage-1 a 192 KB (`NORITO_STAGE1_GPU_MIN_BYTES`), de modo que los documentos pequeños permanezcan en la CPU y las cargas útiles grandes amorticen los costos de lanzamiento de la GPU.

## Entregables y propietarios

| Hito | Entregable | Propietario(s) | Objetivo |
|-----------|-------------|----------|--------|
| Óxido WP2-A/B | Interfaces de sombreado metálico que reflejan los núcleos CUDA | IVM Rendimiento TL | febrero de 2026 |
| Óxido WP2-C | Pruebas de paridad de metal BN254 y carril CI | IVM Rendimiento TL | Segundo trimestre de 2026 |
| IOS6 rápido | Puente alterna cableado (`connect_norito_set_acceleration_config`) + SDK API + muestras | Propietarios del puente Swift | Hecho (enero de 2026) |
| IOS5 rápido | Aplicaciones/documentos de muestra que demuestran el uso de la configuración | Swift DX TL | Segundo trimestre de 2026 |
| Telemetría | Feeds del panel con paridad de aceleración + métricas de referencia | Programa Swift PM / Telemetría | Datos piloto Q2 2026 |
| CI | Arnés de humo XCFramework que ejercita CPU frente a Metal/NEON en el grupo de dispositivos | Líder de control de calidad rápido | Segundo trimestre de 2026 |
| Caja fuerte | Pruebas de paridad de firmas respaldadas por hardware (vectores compartidos) | Android Crypto TL / Seguridad Swift | Tercer trimestre de 2026 |

## Interfaces y contratos API### Óxido (`ivm::AccelerationConfig`)
- Mantener los campos existentes (`enable_simd`, `enable_metal`, `enable_cuda`, `max_gpus`, umbrales).
- Agregue un calentamiento explícito de Metal para evitar la latencia del primer uso (Rust #15875).
- Proporcionar API de paridad que devuelvan estados/diagnósticos para paneles:
  - p.ej. `ivm::vector::metal_status()` -> {habilitado, paridad, último_error}.
- Métricas de evaluación comparativa de salida (tiempos del árbol Merkle, rendimiento de CRC) a través de
  Ganchos de telemetría para `ci/xcode-swift-parity`.
- Metal Host ahora carga el `fastpq.metallib` compilado y envía FFT/IFFT/LDE
  y Poseidon, y recurre a la implementación de la CPU siempre que el
  metallib o la cola de dispositivos no están disponibles.

### C FFI (`connect_norito_bridge`)
- Nueva estructura `connect_norito_acceleration_config` (completada).
- La cobertura del captador ahora incluye `connect_norito_get_acceleration_config` (solo configuración) e `connect_norito_get_acceleration_state` (config + paridad) para reflejar el configurador.
- Diseño de estructura de documento en comentarios de encabezado para consumidores de SPM/CocoaPods.

### Rápido (`AccelerationSettings`)
- Valores predeterminados: Metal habilitado, CUDA deshabilitado, umbrales nulos (heredar).
- Se ignoran los valores negativos; `apply()` invocado automáticamente por `IrohaSDK`.
- `AccelerationSettings.runtimeState()` ahora aparece el `connect_norito_get_acceleration_state`
  carga útil (configuración + estado de paridad Metal/CUDA) para que los paneles de Swift emitan la misma telemetría
  como óxido (`supported/configured/available/parity`). El ayudante devuelve `nil` cuando el
  El puente está ausente para mantener las pruebas portátiles.
- `AccelerationBackendStatus.lastError` copia el motivo de deshabilitación/error de
  `connect_norito_get_acceleration_state` y libera el buffer nativo una vez que la cadena es
  materializado para que los paneles de paridad móvil puedan anotar por qué Metal/CUDA se deshabilitaron en
  cada anfitrión.
- `AccelerationSettingsLoader` (`IrohaSwift/Sources/IrohaSwift/AccelerationSettingsLoader.swift`,
  pruebas bajo `IrohaSwift/Tests/IrohaSwiftTests/AccelerationSettingsLoaderTests.swift`) ahora
  resuelve los manifiestos del operador en el mismo orden de prioridad que la demostración Norito: honor
  `NORITO_ACCEL_CONFIG_PATH`, búsqueda incluida `acceleration.{json,toml}` / `client.{json,toml}`,
  registre la fuente elegida y vuelva a los valores predeterminados. Las aplicaciones ya no necesitan cargadores personalizados para
  Refleje la superficie Rust `iroha_config`.
- Actualice aplicaciones de muestra y README para mostrar alternancias e integración de telemetría.

### Telemetría (Paneles + Exportadores)
- Feed de paridad (mobile_parity.json):
  - `acceleration.metal/neon/strongbox` -> {habilitado, paridad, perf_delta_pct}.
  - Acepte la comparación básica entre CPU y GPU `perf_delta_pct`.
  - `acceleration.metal.disable_reason` espejos `AccelerationBackendStatus.lastError`
    para que la automatización Swift pueda marcar las GPU deshabilitadas con la misma fidelidad que Rust
    tableros de instrumentos.
- Fuente de CI (mobile_ci.json):
  - `acceleration_bench.metal_vs_cpu_merkle_ms` -> {cpu, metal}
  - `acceleration_bench.neon_crc64_throughput_mb_s` -> Doble.
- Los exportadores deben obtener métricas de los puntos de referencia de Rust o ejecuciones de CI (por ejemplo, ejecutar
  Microbanco de metal/CPU como parte de `ci/xcode-swift-parity`).### Perillas de configuración y valores predeterminados (WP6-C)
- Valores predeterminados de `AccelerationConfig`: `enable_metal = true` en compilaciones de macOS, `enable_cuda = true` cuando se compila la función CUDA, `max_gpus = None` (sin límite). El contenedor Swift `AccelerationSettings` hereda los mismos valores predeterminados a través de `connect_norito_set_acceleration_config`.
- Heurística de Merkle Norito (GPU vs CPU): `merkle_min_leaves_gpu = 8192` habilita el hash de GPU para árboles con ≥8192 hojas; Las anulaciones de backend (`merkle_min_leaves_metal`, `merkle_min_leaves_cuda`) tienen de forma predeterminada el mismo umbral a menos que se establezcan explícitamente.
- Heurística de preferencia de CPU (SHA2 ISA presente): tanto en AArch64 (ARMv8 SHA2) como en x86/x86_64 (SHA-NI), la ruta de la CPU sigue siendo preferida hasta las hojas `prefer_cpu_sha2_max_leaves_* = 32_768`; por encima de eso se aplica el umbral de GPU. Estos valores se pueden configurar a través de `AccelerationConfig` y deben ajustarse únicamente con evidencia de referencia.

## Estrategia de prueba

1. **Pruebas de paridad unitaria (Rust)**: asegúrese de que los núcleos metálicos coincidan con las salidas de la CPU para
   vectores deterministas; ejecutar bajo `cargo test -p ivm --features metal`.
   `crates/fastpq_prover/src/metal.rs` ahora incluye pruebas de paridad solo para macOS que
   ejercite FFT/IFFT/LDE y Poseidon contra la referencia escalar.
2. **Arnés de humo rápido**: ampliar el ejecutor de pruebas de IOS6 para ejecutar CPU frente a Metal
   codificación (Merkle/CRC64) tanto en emuladores como en dispositivos StrongBox; comparar
   resultados y estado de paridad de registros.
3. **CI**: actualice `norito_bridge_ios.yml` (ya llama a `make swift-ci`) para enviar
   métricas de aceleración de artefactos; asegúrese de que la ejecución confirme Buildkite
   Metadatos `ci/xcframework-smoke:<lane>:device_tag` antes de publicar cambios de arnés,
   y fallar en el carril por paridad/derivación del punto de referencia.
4. **Paneles**: los nuevos campos ahora se muestran en la salida CLI. Garantizar que los exportadores produzcan
   datos antes de voltear los paneles en vivo.

## Plan de sombreado metálico WP2-A (tuberías Poseidon)

El primer hito del WP2 cubre el trabajo de planificación de los núcleos de Poseidon Metal
que reflejan la implementación de CUDA. El plan divide el esfuerzo en núcleos,
programación del host y puesta en escena constante compartida para que el trabajo posterior pueda centrarse exclusivamente en
implementación y pruebas.

### Alcance del núcleo

1. `poseidon_permute`: permuta estados independientes de `state_count`. cada hilo
   posee un `STATE_CHUNK` (4 estados) y ejecuta todas las iteraciones `TOTAL_ROUNDS` usando
   Constantes redondas compartidas por grupos de subprocesos preparadas en el momento del envío.
2. `poseidon_hash_columns`: lee el escaso catálogo `PoseidonColumnSlice` y
   realiza un hash compatible con Merkle de cada columna (que coincide con la CPU)
   Diseño `PoseidonColumnBatch`). Utiliza el mismo buffer constante de grupo de subprocesos.
   como el kernel permutado pero recorre `(states_per_lane * block_count)`
   salidas para que el kernel pueda amortizar los envíos de cola.
3. `poseidon_trace_fused`: calcula los resúmenes principal/hoja para la tabla de seguimiento
   en una sola pasada. El kernel fusionado consume `PoseidonFusedArgs` por lo que el host
   puede describir regiones no contiguas y un `leaf_offset`/`parent_offset`, y
   comparte todas las tablas redondas/MDS con los otros núcleos.

### Programación de comandos y contratos de host- Cada envío del kernel pasa por `MetalPipelines::command_queue`, que
  aplica el programador adaptativo (objetivo ~2 ms) y los controles de distribución de la cola
  expuesto a través de `FASTPQ_METAL_QUEUE_FANOUT` y
  `FASTPQ_METAL_COLUMN_THRESHOLD`. La ruta de calentamiento en `with_metal_state`
  compila los tres núcleos de Poseidon por adelantado para que el primer envío no
  pagar una penalización por creación de oleoductos.
- El tamaño del grupo de subprocesos refleja los valores predeterminados existentes de Metal FFT/LDE: el objetivo es
  8192 subprocesos por envío con un límite estricto de 256 subprocesos por grupo. el
  El host puede reducir el multiplicador `states_per_lane` para dispositivos de bajo consumo al
  marcando las anulaciones del entorno (`FASTPQ_METAL_POSEIDON_STATES_PER_BATCH`
  que se agregará en WP2-B) sin modificar la lógica del sombreador.
- La preparación de columnas sigue el mismo grupo de doble buffer ya utilizado por la FFT
  tuberías. Los núcleos de Poseidon aceptan punteros sin formato en ese búfer de preparación.
  y nunca tocar las asignaciones globales del montón, lo que mantiene el determinismo de la memoria.
  alineado con el host CUDA.

### Constantes compartidas

- El manifiesto `PoseidonSnapshot` descrito en
  `docs/source/fastpq/poseidon_metal_shared_constants.md` ahora es el canónico
  fuente para las constantes redondas y la matriz MDS. Ambos metálicos (`poseidon2.metal`)
  y los kernels CUDA (`fastpq_cuda.cu`) deben regenerarse siempre que el manifiesto
  cambios.
- WP2-B agregará un pequeño cargador de host que lee el manifiesto en tiempo de ejecución y
  emite el SHA-256 en telemetría (`acceleration.poseidon_constants_sha`) para que
  Los paneles de paridad pueden afirmar que las constantes del sombreador coinciden con las publicadas.
  instantánea.
- Durante el calentamiento copiaremos las constantes `TOTAL_ROUNDS x STATE_WIDTH` en un
  `MTLBuffer` y cárguelo una vez por dispositivo. Luego, cada núcleo copia los datos.
  en la memoria del grupo de subprocesos antes de procesar su fragmento, lo que garantiza un carácter determinista.
  realizar pedidos incluso cuando se ejecutan varios buffers de comando en vuelo.

### Ganchos de validación

- Las pruebas unitarias (`cargo test -p fastpq_prover --features fastpq-gpu`) crecerán un
  afirmación que codifica las constantes del sombreador incrustado y las compara con
  el SHA del manifiesto antes de ejecutar el conjunto de dispositivos GPU.
- Las estadísticas del kernel existentes alternan (`FASTPQ_METAL_TRACE_DISPATCH`,
  `FASTPQ_METAL_QUEUE_FANOUT`, telemetría de profundidad de cola) se convierten en evidencia requerida
  para la salida de WP2: cada ejecución de prueba debe demostrar que el planificador nunca viola el
  distribución configurada y que el núcleo de seguimiento fusionado mantiene la cola por debajo del
  ventana adaptativa.
- Comenzarán el arnés de humo Swift XCFramework y los corredores de referencia Rust
  exportando `acceleration.poseidon.permute_p90_ms{cpu,metal}` para que WP2-D pueda trazar
  Deltas de metal versus CPU sin reinventar nuevas fuentes de telemetría.

## Cargador de manifiesto Poseidon WP2-B y paridad de autoprueba- `fastpq_prover::poseidon_manifest()` ahora incorpora y analiza
  `artifacts/offline_poseidon/constants.ron`, calcula su SHA-256
  (`poseidon_manifest_sha256()`) y valida la instantánea con la CPU
  tablas Poseidon antes de ejecutar cualquier trabajo de GPU. `build_metal_context()` registra el
  resumir durante el calentamiento para que los exportadores de telemetría puedan publicar
  `acceleration.poseidon_constants_sha`.
- El analizador de manifiesto rechaza tuplas de ancho/tasa/recuento redondo que no coinciden y
  asegura que la matriz MDS manifiesta sea igual a la implementación escalar, evitando
  deriva silenciosa cuando se regeneran las tablas canónicas.
- Se agregó `crates/fastpq_prover/tests/poseidon_manifest_consistency.rs`, que
  analiza las tablas de Poseidon incrustadas en `poseidon2.metal` y
  `fastpq_cuda.cu` y afirma que ambos núcleos serializan exactamente igual
  constantes como el manifiesto. CI ahora falla si alguien edita el sombreador/CUDA
  archivos sin regenerar el manifiesto canónico.
- Los ganchos de paridad futuros (WP2-C/D) pueden reutilizar `poseidon_manifest()` para organizar el
  redondear constantes en buffers de GPU y exponer el resumen a través de Norito
  transmisiones de telemetría.

## WP2-C BN254 Tuberías metálicas y pruebas de paridad- **Alcance y brecha:** Los despachadores de host, los arneses de paridad y `bn254_status()` están activos, y `crates/fastpq_prover/metal/kernels/bn254.metal` ahora implementa las primitivas de Montgomery más bucles FFT/LDE sincronizados con grupos de subprocesos. Cada envío ejecuta una columna completa dentro de un único grupo de subprocesos con barreras por etapa, por lo que los núcleos ejercitan los manifiestos por etapas en paralelo. La telemetría ahora está conectada y se respetan las anulaciones del programador para que podamos controlar la implementación predeterminada con la misma evidencia que usamos para los núcleos Goldilocks.
- **Requisitos del kernel:** ✅ reutilizar los manifiestos twiddle/coset por etapas, convertir entradas/salidas una vez y ejecutar todas las etapas radix-2 dentro del grupo de subprocesos por columna para que no necesitemos sincronización de envío múltiple. Los ayudantes de Montgomery siguen siendo compartidos entre FFT/LDE, por lo que solo cambió la geometría del bucle.
- **Cableado del host:** ✅ `crates/fastpq_prover/src/metal.rs` organiza los miembros canónicos, llena con ceros el búfer LDE, selecciona un único grupo de subprocesos por columna y expone `bn254_status()` para la activación. No se requieren cambios de host adicionales para la telemetría.
- **Build guards:** el `fastpq.metallib` incluye los núcleos en mosaico, por lo que CI aún falla rápidamente si el sombreador se desvía. Cualquier optimización futura permanece detrás de las puertas de telemetría/funciones en lugar de los interruptores en tiempo de compilación.
- **Dispositivos de paridad:** ✅ Las pruebas `bn254_parity` continúan comparando las salidas FFT/LDE de GPU con dispositivos de CPU y ahora se ejecutan en vivo en hardware metálico; tenga en cuenta las pruebas de manifiesto manipulado si aparecen nuevas rutas de código del kernel.
- **Telemetría y puntos de referencia:** `fastpq_metal_bench` ahora emite:
  - un bloque `bn254_dispatch` que resume los anchos de los grupos de subprocesos por envío, los recuentos de subprocesos lógicos y los límites de canalización para lotes de una sola columna FFT/LDE; y
  - un bloque `bn254_metrics` que registra `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` para la línea base de la CPU más cualquier backend de GPU que se haya ejecutado.
  El contenedor de referencia copia ambos mapas en cada artefacto empaquetado para que los paneles de WP2-D ingieran latencias/geometría etiquetadas sin realizar ingeniería inversa en la matriz de operaciones sin procesar. `FASTPQ_METAL_THREADGROUP` ahora también se aplica a los despachos BN254 FFT/LDE, lo que hace que la perilla se pueda utilizar para rendimiento. barridos.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_benchmark.py:1037】

## Preguntas abiertas (resueltas en mayo de 2027)1. **Limpieza de recursos metálicos:** `warm_up_metal()` reutiliza el subproceso local
   `OnceCell` y ahora tiene pruebas de idempotencia/regresión
   (`crates/ivm/src/vector.rs::warm_up_metal_reuses_cached_state` /
   `warm_up_metal_is_noop_on_non_metal_targets`), por lo que las transiciones del ciclo de vida de la aplicación
   Puede llamar con seguridad a la ruta de calentamiento sin fugas ni inicialización doble.
2. **Líneas de referencia de referencia:** Los carriles metálicos deben permanecer dentro del 20% de la CPU
   línea de base para FFT/IFFT/LDE y dentro del 15 % para los ayudantes de Poseidon CRC/Merkle;
   la alerta debería activarse cuando `acceleration.*_perf_delta_pct > 0.20` (o falta)
   en el feed de paridad móvil. Regresiones IFFT observadas en el paquete de trazas de 20k
   ahora están controlados por la solución de anulación de cola indicada en WP2-D.
3. **Respaldo de StrongBox:** Swift sigue el manual de respaldo de Android al
   registrar errores de atestación en el runbook de soporte
   (`docs/source/sdk/swift/support_playbook.md`) y cambiando automáticamente a
   la ruta de software documentada respaldada por HKDF con registro de auditoría; Vectores de paridad
   permanezca compartido a través de los dispositivos OA existentes.
4. **Almacenamiento de telemetría:** Las capturas de aceleración y las pruebas del grupo de dispositivos son
   archivado bajo `configs/swift/` (por ejemplo,
   `configs/swift/xcframework_device_pool_snapshot.json`), y exportadores
   debe reflejar el mismo diseño (`artifacts/swift/telemetry/acceleration/*.json`
   o `.prom`) para que las anotaciones de Buildkite y los paneles del portal puedan ingerir el
   se alimenta sin raspado ad hoc.

## Próximos pasos (febrero de 2026)

- [x] Rust: integración de host de land Metal (`crates/fastpq_prover/src/metal.rs`) y
      exponer la interfaz del kernel para Swift; traspaso de documentos rastreado junto al
      Notas rápidas del puente.
- [x] Swift: expone la configuración de aceleración a nivel de SDK (realizado en enero de 2026).
- [x] Telemetría: `scripts/acceleration/export_prometheus.py` ahora convierte
      Salida `cargo xtask acceleration-state --format json` en un Prometheus
      Archivo de texto (con etiqueta `--instance` opcional) para que las ejecuciones de CI puedan adjuntar GPU/CPU
      habilitación, umbrales y motivos de paridad/deshabilitación directamente al archivo de texto
      Colectores sin raspado a medida.
- [x] Swift QA: `scripts/acceleration/acceleration_matrix.py` agrega múltiples
      capturas de estado de aceleración en tablas JSON o Markdown codificadas por dispositivo
      etiqueta, que le da al arnés de humo una matriz determinista "CPU vs Metal/CUDA"
      para cargar junto con los cigarrillos de la aplicación de muestra. La salida de Markdown refleja la
      Formato de evidencia Buildkite para que los paneles puedan ingerir el mismo artefacto.
- [x] Actualice status.md ahora que `irohad` envía los exportadores de cola/relleno cero y
      las pruebas de validación de env/config cubren las anulaciones de la cola de Metal, por lo que WP2-D
      telemetría + enlaces tienen evidencia en vivo adjunta.【crates/irohad/src/main.rs:2664】【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【status.md:1546】

Comandos auxiliares de telemetría/exportación:

```bash
# Prometheus textfile from a single capture
cargo xtask acceleration-state --format json > artifacts/acceleration_state_macos_m4.json
python3 scripts/acceleration/export_prometheus.py \
  --input artifacts/acceleration_state_macos_m4.json \
  --output artifacts/acceleration_state_macos_m4.prom \
  --instance macos-m4

# Aggregate multiple captures into a Markdown matrix
python3 scripts/acceleration/acceleration_matrix.py \
  --state macos-m4=artifacts/acceleration_state_macos_m4.json \
  --state sim-m3=artifacts/acceleration_state_sim_m3.json \
  --format markdown \
  --output artifacts/acceleration_matrix.md
```

## Comparativa de lanzamiento y notas vinculantes de WP2-D- **Captura de lanzamiento de 20.000 filas:** Se registró una nueva prueba comparativa entre Metal y CPU en macOS14
  (arm64, parámetros de carril balanceado, seguimiento rellenado de 32,768 filas, lotes de dos columnas) y
  Verificó el paquete JSON en `fastpq_metal_bench_20k_release_macos14_arm64.json`.
  El punto de referencia exporta tiempos por operación más evidencia del microbanco Poseidon, por lo que
  WP2-D tiene un artefacto de calidad GA vinculado a la nueva heurística de cola de Metal. Titular
  deltas (la tabla completa se encuentra en `docs/source/benchmarks.md`):

  | Operación | Media de CPU (ms) | Media del metal (ms) | Aceleración |
  |-----------|---------------|-----------------|---------|
  | FFT (32.768 entradas) | 12.741 | 10.963 | 1,16 × |
  | IFFT (32.768 entradas) | 17.499 | 25.688 | 0,68× *(regresión: distribución de la cola acelerada para mantener el determinismo; necesita ajustes de seguimiento)* |
  | LDE (262.144 entradas) | 68.389 | 65.701 | 1,04× |
  | Columnas hash de Poseidón (524.288 entradas) | 1.728,835 | 1.447,076 | 1,19× |

  Cada captura registra tiempos `zero_fill` (9,651 ms para 33.554.432 bytes) y
  Entradas `poseidon_microbench` (carril predeterminado 596.229 ms vs escalar 656.251 ms,
  1,10 × aceleración) para que los consumidores del panel puedan diferenciar la presión de la cola junto con el
  operaciones principales.
- **Enlace cruzado de enlaces/docs:** `docs/source/benchmarks.md` ahora hace referencia al
  suelte el comando JSON y el reproductor, se validan las anulaciones de la cola de Metal
  a través de pruebas de env/manifest `iroha_config`, y publicaciones `irohad` en vivo
  `fastpq_metal_queue_*` mide para que los paneles marquen las regresiones IFFT sin
  raspado de troncos ad hoc. El `AccelerationSettings.runtimeState` de Swift expone el
  La misma carga útil de telemetría enviada en el paquete JSON, cerrando el WP2-D.
  brecha de enlace/doc con una línea base de aceptación reproducible.【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【crates/irohad/src/main.rs:2664】
- **Corrección de cola IFFT:** Los lotes FFT inversos ahora omiten el envío de colas múltiples cuando sea
  la carga de trabajo apenas alcanza el umbral de distribución (16 columnas en el carril equilibrado
  perfil), eliminando la regresión Metal-vs-CPU mencionada anteriormente mientras se mantiene
  cargas de trabajo de columnas grandes en la ruta de múltiples colas para FFT/LDE/Poseidon.