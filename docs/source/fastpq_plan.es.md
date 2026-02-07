---
lang: es
direction: ltr
source: docs/source/fastpq_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8324267c90cfbaf718760c4883427e85d81edcfa180dd9f64fd31a5e219749f4
source_last_modified: "2026-01-18T05:31:56.951617+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Desglose del trabajo del probador FASTPQ

Este documento captura el plan por etapas para entregar un probador FASTPQ-ISI listo para producción y conectarlo al proceso de programación del espacio de datos. Cada definición a continuación es normativa a menos que esté marcada como TODO. La solidez estimada utiliza límites DEEP-FRI al estilo de El Cairo; Las pruebas automatizadas de muestreo de rechazo en CI fallan si el límite medido cae por debajo de 128 bits.

## Etapa 0: marcador de posición hash (aterrizado)
- Codificación determinista Norito con compromiso BLAKE2b.
- El backend del marcador de posición devuelve `BackendUnavailable`.
- Tabla de parámetros canónicos proporcionada por `fastpq_isi`.

## Etapa 1: Prototipo de Trace Builder

> **Estado (09/11/2025):** `fastpq_prover` ahora expone el embalaje canónico
> ayudantes (`pack_bytes`, `PackedBytes`) y el determinista Poseidon2
> Compromiso de pedido sobre Ricitos de Oro. Las constantes están fijadas a
> `ark-poseidon2` confirma `3f2b7fe`, cerrando el seguimiento sobre el intercambio del BLAKE2 provisional
> el marcador de posición está cerrado. Luminarias doradas (`tests/fixtures/packing_roundtrip.json`,
> `tests/fixtures/ordering_hash.json`) ahora ancla el conjunto de regresión.

### Objetivos
- Implementar el generador de seguimiento FASTPQ para KV-update AIR. Cada fila debe codificar:
  - `key_limbs[i]`: miembros base-256 (7 bytes, little-endian) de la ruta de clave canónica.
  - `value_old_limbs[i]`, `value_new_limbs[i]`: mismo embalaje para valores pre/post.
  - Columnas selectoras: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`.
  - Columnas auxiliares: `delta = value_new - value_old`, `running_asset_delta`, `metadata_hash`, `supply_counter`.
  - Columnas de activos: `asset_id_limbs[i]` usando ramas de 7 bytes.
  - Columnas SMT por nivel `ℓ`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, más `neighbour_leaf` para no membresía.
  - Columnas de metadatos: `dsid`, `slot`.
- **Ordenamiento determinista.** Ordenar filas lexicográficamente por `(key_bytes, op_rank, original_index)` usando una clasificación estable. Asignación de `op_rank`: `transfer=0`, `mint=1`, `burn=2`, `role_grant=3`, `role_revoke=4`, `meta_set=5`. `original_index` es el índice basado en 0 antes de ordenar. Conserve el hash de pedido Poseidon2 resultante (etiqueta de dominio `fastpq:v1:ordering`). Codifique la preimagen hash como `[domain_len, domain_limbs…, payload_len, payload_limbs…]` donde las longitudes son elementos de campo u64 para que los bytes cero finales sigan siendo distinguibles.
- Testigo de búsqueda: produce `perm_hash = Poseidon2(role_id || permission_id || epoch_u64_le)` cuando la columna almacenada `s_perm` (OR lógico de `s_role_grant` e `s_role_revoke`) es 1. Los ID de función/permiso son cadenas LE de 32 bytes de ancho fijo; la época es LE de 8 bytes.
- Aplicar invariantes tanto antes como dentro de AIR: selectores mutuamente excluyentes, conservación por activo, constantes dsid/slot.
- `N_trace = 2^k` (`pow2_ceiling` del recuento de filas); `N_eval = N_trace * 2^b` donde `b` es el exponente de ampliación.
- Proporcionar accesorios y pruebas de propiedades:
  - Embalaje de ida y vuelta (`fastpq_prover/tests/packing.rs`, `tests/fixtures/packing_roundtrip.json`).
  - Hash de estabilidad de pedidos (`tests/fixtures/ordering_hash.json`).
  - Accesorios por lotes (`trace_transfer.json`, `trace_mint.json`, `trace_duplicate_update.json`).### Esquema de columnas AIR
| Grupo de columnas | Nombres | Descripción |
| ----------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Actividad | `s_active` | 1 para filas reales, 0 para relleno.                                                                                       |
| Principal | `key_limbs[i]`, `value_old_limbs[i]`, `value_new_limbs[i]` | Elementos Goldilocks empaquetados (little-endian, extremidades de 7 bytes).                                                             |
| Activo | `asset_id_limbs[i]` | Identificador de activo canónico empaquetado (little-endian, ramas de 7 bytes).                                                      |
| Selectores | `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm` | 0/1. Restricción: Σ selectores (incluido `s_perm`) = `s_active`; `s_perm` refleja las filas de concesión/revocación de roles.              |
| Auxiliar | `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter` | Estado utilizado para restricciones, conservación y pistas de auditoría.                                                           |
| SMT | `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` | Entradas/salidas de Poseidon2 por nivel más testigo vecino en caso de no membresía.                                         |
| Búsqueda | `perm_hash` | Hash Poseidon2 para búsqueda de permisos (restringido solo cuando `s_perm = 1`).                                            |
| Metadatos | `dsid`, `slot` | Constante en todas las filas.                                                                                                 |### Matemáticas y restricciones
- **Embalaje de campos:** los bytes se dividen en ramas de 7 bytes (little-endian). Cada miembro `limb_j = Σ_{k=0}^{6} byte_{7j+k} * 256^k`; rechazar miembros ≥ módulo Ricitos de Oro.
- **Saldo/conservación:** let `δ = value_new - value_old`. Agrupe filas por `asset_id`. Defina `r_asset_start = 1` en la primera fila de cada grupo de activos (0 en caso contrario) y restrinja
  ```
  running_asset_delta = (1 - r_asset_start) * running_asset_delta_prev + δ.
  ```
  En la última fila de cada grupo de activos afirmar
  ```
  running_asset_delta = Σ (s_mint * δ) - Σ (s_burn * δ).
  ```
  Las transferencias satisfacen la restricción automáticamente porque sus valores δ suman cero en todo el grupo. Ejemplo: si `value_old = 100` e `value_new = 120` en una fila de menta, δ = 20, la suma de menta contribuye con +20 y la verificación final se resuelve en cero cuando no se producen quemaduras.
- **Relleno:** introduce `s_active`. Multiplique todas las restricciones de fila por `s_active` y aplique un prefijo contiguo: `s_active[i] ≥ s_active[i+1]`. Las filas de relleno (`s_active=0`) deben mantener valores constantes pero, por lo demás, no tienen restricciones.
- **Hash de pedido:** hash Poseidon2 (dominio `fastpq:v1:ordering`) sobre codificaciones de filas; almacenado en Public IO para su auditabilidad.

## Etapa 2: Núcleo del probador STARK

### Objetivos
- Construir compromisos de Poseidon2 Merkle sobre vectores de evaluación de seguimiento y búsqueda. Parámetros: tasa = 2, capacidad = 1, rondas completas = 8, rondas parciales = 57, constantes fijadas a `ark-poseidon2` confirman `3f2b7fe` (v0.3.0).
- Extensión de bajo grado: evalúe cada columna en el dominio `D = { g^i | i = 0 .. N_eval-1 }`, donde `N_eval = 2^{k+b}` divide la capacidad de 2 ádicos de Ricitos de Oro. Sea `g = ω^{(p-1)/N_eval}` con `ω` la raíz primitiva fija de Ricitos de Oro y `p` su módulo; utilice el subgrupo base (sin clase lateral). Registre `g` en la transcripción (etiqueta `fastpq:v1:lde`).
- Polinomios de composición: para cada restricción `C_j`, forme `F_j(X) = C_j(X) / Z_N(X)` con los márgenes de grados que se enumeran a continuación.
- Argumento de búsqueda (permisos): muestra `γ` de la transcripción. Producto de traza `Z_0 = 1`, `Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}`. Producto de mesa `T = ∏_j (table_perm_j - γ)`. Restricción de límite: `Z_final / T = 1`.
- DEEP-FRI con aridad `r ∈ {8, 16}`: para cada capa, absorber la raíz con la etiqueta `fastpq:v1:fri_layer_ℓ`, muestra `β_ℓ` (etiqueta `fastpq:v1:beta_ℓ`) y doblar mediante `H_{ℓ+1}(i) = Σ_{k=0}^{r-1} H_ℓ(r*i + k) * β_ℓ^k`.
- Objeto de prueba (codificado con Norito):
  ```
  Proof {
      protocol_version: u16,
      params_version: u16,
      parameter_set: String,
      public_io: PublicIO,
      trace_root: [u8; 32],
      lookup_root: [u8; 32],
      fri_layers: Vec<[u8; 32]>,
      alphas: Vec<Field>,
      betas: Vec<Field>,
      queries: Vec<QueryOpening>,
  }
  ```
- Verificador refleja el probador; Ejecute el conjunto de regresión en trazas de 1k/5k/20k filas con transcripciones doradas.

### Licenciatura en Contabilidad
| Restricción | Grado antes de la división | Grado después de los selectores | Margen frente a `deg(Z_N)` |
|------------|------------------------|------------------------|----------------------|
| Transferencia/ceca/conservación quemada | ≤1 | ≤1 | `deg(Z_N) - 2` |
| Búsqueda de concesión/revocación de roles | ≤2 | ≤2 | `deg(Z_N) - 3` |
| Conjunto de metadatos | ≤1 | ≤1 | `deg(Z_N) - 2` |
| Hash SMT (por nivel) | ≤3 | ≤3 | `deg(Z_N) - 4` |
| Búsqueda de gran producto | relación de producto | N/A | Restricción de límites |
| Raíces fronterizas/totales de oferta | 0 | 0 | exacto |

Las filas de relleno se manejan a través de `s_active`; Las filas ficticias extienden el seguimiento hasta `N_trace` sin violar las restricciones.## Codificación y transcripción (global)
- **Embalaje de bytes:** base-256 (extremidades de 7 bytes, little-endian). Pruebas en `fastpq_prover/tests/packing.rs`.
- **Codificación de campo:** Ricitos de Oro canónico (extremidad little-endian de 64 bits, rechazar ≥ p); Salidas Poseidon2/raíces SMT serializadas como matrices little-endian de 32 bytes.
- **Transcripción (Fiat–Shamir):**
  1. BLAKE2b absorbe `protocol_version`, `params_version`, `parameter_set`, `public_io` y la etiqueta de confirmación Poseidon2 (`fastpq:v1:init`).
  2. Absorba `trace_root`, `lookup_root` (`fastpq:v1:roots`).
  3. Derivar el desafío de búsqueda `γ` (`fastpq:v1:gamma`).
  4. Derivar desafíos de composición `α_j` (`fastpq:v1:alpha_j`).
  5. Para cada raíz de capa FRI, absorber con `fastpq:v1:fri_layer_ℓ`, derivar `β_ℓ` (`fastpq:v1:beta_ℓ`).
  6. Derivar índices de consulta (`fastpq:v1:query_index`).

  Las etiquetas son ASCII en minúsculas; Los verificadores rechazan las discrepancias antes de las impugnaciones de muestreo. Accesorio de transcripción dorada: `tests/fixtures/transcript_v1.json`.
- **Versionado:** `protocol_version = 1`, `params_version` coincide con el conjunto de parámetros `fastpq_isi`.

## Argumento de búsqueda (permisos)
- Tabla confirmada ordenada lexicográficamente por `(role_id_bytes, permission_id_bytes, epoch_le)` y confirmada mediante el árbol Poseidon2 Merkle (`perm_root` en `PublicIO`).
- El testigo de seguimiento utiliza `perm_hash` y ​​el selector `s_perm` (O de concesión/revocación de función). La tupla está codificada como `role_id_bytes || permission_id_bytes || epoch_u64_le` con anchos fijos (32, 32, 8 bytes).
- Relación del producto:
  ```
  Z_0 = 1
  for each row i: Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}
  T = ∏_j (table_perm_j - γ)
  ```
  Aserción de límites: `Z_final / T = 1`. Consulte `examples/lookup_grand_product.md` para obtener un tutorial sobre el acumulador de concreto.

## Restricciones dispersas del árbol Merkle
- Definir `SMT_HEIGHT` (número de niveles). Las columnas `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` aparecen para todos los `ℓ ∈ [0, SMT_HEIGHT)`.
- Parámetros de Poseidon2 fijados al compromiso `ark-poseidon2` `3f2b7fe` (v0.3.0); etiqueta de dominio `fastpq:v1:poseidon_node`. Todos los nodos utilizan codificación de campo little-endian.
- Actualizar reglas por nivel:
  ```
  if path_bit_ℓ == 0:
      node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
  else:
      node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
  ```
- Juego de inserciones `(node_in_0 = 0, node_out_0 = value_new)`; elimina el conjunto `(node_in_0 = value_old, node_out_0 = 0)`.
- Las pruebas de no membresía proporcionan `neighbour_leaf` para mostrar que el intervalo consultado está vacío. Consulte `examples/smt_update.md` para ver un ejemplo resuelto y un diseño JSON.
- Restricción de límite: el hash final es igual a `old_root` para las filas previas y a `new_root` para las filas posteriores.

## Parámetros de solidez y SLO
| N_traza | explosión | FRI aridad | capas | consultas | bits est | Tamaño de prueba (≤) | RAM (≤) | Latencia P95 (≤) |
| ------- | ------ | --------- | ------ | ------- | -------- | --------------- | ------- | ---------------- |
| 2^15 | 8 | 8 | 5 | 52 | ~190 | 300 KB | 1,5 GB | 0,40 s (A100) |
| 2^16 | 8 | 8 | 6 | 58 | ~132 | 420 KB | 2,5 GB | 0,75 s (A100) |
| 2^17 | 16 | 16 | 5 | 64 | ~142 | 550 KB | 3,5 GB | 1,20 s (A100) |

Las derivaciones siguen el Apéndice A. El arnés de CI produce pruebas con formato incorrecto y falla si los bits estimados son <128.## Esquema de IO público
| Campo | Bytes | Codificación | Notas |
|-----------------|-------|---------------------------------------|--------------------------------|
| `dsid` | 16 | UUID little-endian | ID de espacio de datos para el carril de la entrada (global para el carril predeterminado), codificado con la etiqueta `fastpq:v1:dsid`. |
| `slot` | 8 | little-endian u64 | Nanosegundos desde la época.            |
| `old_root` | 32 | bytes de campo little-endian Poseidon2 | Raíz SMT antes del lote.              |
| `new_root` | 32 | bytes de campo little-endian Poseidon2 | Raíz SMT tras lote.               |
| `perm_root` | 32 | bytes de campo little-endian Poseidon2 | Raíz de la tabla de permisos para la ranura. |
| `tx_set_hash` | 32 | BLAKE2b | Identificadores de instrucciones ordenados.     |
| `parameter` | var | UTF-8 (por ejemplo, `fastpq-lane-balanced`) | Nombre del conjunto de parámetros.                 |
| `protocol_version`, `params_version` | 2 cada uno | little endian u16 | Valores de versión.                      |
| `ordering_hash` | 32 | Poseidon2 (little-endian) | Hash estable de filas ordenadas.         |

La eliminación está codificada por miembros de valor cero; Las claves ausentes utilizan hoja cero + testigo vecino.

`FastpqTransitionBatch.public_inputs` es el operador canónico para `dsid`, `slot` y compromisos raíz;
Los metadatos por lotes están reservados para la contabilidad del recuento de hash de entrada/transcripción.

## Codificación de hashes
- Hash de pedido: Poseidon2 (etiqueta `fastpq:v1:ordering`).
- Hash de artefacto por lotes: BLAKE2b sobre `PublicIO || proof.commitments` (etiqueta `fastpq:v1:artifact`).

## Definiciones de etapa de Listo (DoD)
- **Etapa 1 Departamento de Defensa**
  - Empaque de pruebas de ida y vuelta y accesorios fusionados.
  - La especificación AIR (`docs/source/fastpq_air.md`) incluye `s_active`, columnas de activos/SMT, definiciones de selector (incluido `s_perm`) y restricciones simbólicas.
  - Orden de hash registrado en PublicIO y verificado a través de accesorios.
  - Generación de testigos de búsqueda/SMT implementada con vectores de membresía y no membresía.
  - Las pruebas de conservación abarcan transferencia, acuñación, quema y lotes mixtos.
- **Etapa 2 Departamento de Defensa**
  - Especificaciones de transcripción implementadas; transcripción dorada (`tests/fixtures/transcript_v1.json`) y etiquetas de dominio verificadas.
  - El parámetro Poseidon2 confirma `3f2b7fe` fijado en el probador y el verificador con pruebas de endianidad en todas las arquitecturas.
  - Protección de solidez CI activa; Tamaño de prueba/RAM/latencia SLO registrados.
- **Etapa 3 Departamento de Defensa**
  - API del programador (`SubmitProofRequest`, `ProofResult`) documentada con claves de idempotencia.
  - Probar artefactos almacenados en contenido direccionable con reintento/retroceso.
  - Telemetría exportada para profundidad de cola, tiempo de espera de cola, latencia de ejecución del probador, recuentos de reintentos, recuentos de fallas de backend y utilización de GPU/CPU, con paneles y umbrales de alerta para cada métrica.## Etapa 5: Aceleración y optimización de GPU
- Núcleos objetivo: LDE (NTT), hash Poseidon2, construcción de árbol Merkle, plegado FRI.
- Determinismo: deshabilite las matemáticas rápidas, garantice salidas de bits idénticos en CPU, CUDA y Metal. CI debe comparar las raíces de prueba entre dispositivos.
- Conjunto de pruebas comparativas de CPU y GPU en hardware de referencia (por ejemplo, Nvidia A100, AMD MI210).
- Parte trasera metálica (Apple Silicon):
  - El script de compilación compila el conjunto del kernel (`metal/kernels/ntt_stage.metal`, `metal/kernels/poseidon2.metal`) en `fastpq.metallib` a través de `xcrun metal`/`xcrun metallib`; asegúrese de que las herramientas de desarrollador de macOS incluyan la cadena de herramientas Metal (`xcode-select --install`, luego `xcodebuild -downloadComponent MetalToolchain` si es necesario).【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:189】
  - Reconstrucción manual (espejos `build.rs`) para calentamientos de CI o empaquetamiento determinista:
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    Las compilaciones exitosas emiten `FASTPQ_METAL_LIB=<path>` para que el tiempo de ejecución pueda cargar metallib de manera determinista. 【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:42】
  - El kernel LDE ahora asume que el buffer de evaluación está inicializado en cero en el host. Mantenga la ruta de asignación `vec![0; ..]` existente o explícitamente cero buffers cuando los reutilice.【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:141】
  - La multiplicación de Coset se fusiona en la etapa final de FFT para evitar una pasada adicional; cualquier cambio en la etapa LDE debe preservar esa invariante. 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - El kernel FFT/LDE de memoria compartida ahora se detiene en la profundidad del mosaico y entrega las mariposas restantes más cualquier escala inversa a un pase `fastpq_fft_post_tiling` dedicado. El host de Rust enhebra los mismos lotes de columnas a través de ambos kernels y solo inicia el envío posterior al mosaico cuando `log_len` excede el límite del mosaico, por lo que la telemetría de profundidad de la cola, las estadísticas del kernel y el comportamiento alternativo siguen siendo deterministas mientras la GPU maneja el trabajo de etapa amplia por completo. en el dispositivo.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - Para experimentar con formas de lanzamiento, configure `FASTPQ_METAL_THREADGROUP=<width>`; la ruta de envío fija el valor al límite del dispositivo y registra la anulación para que las ejecuciones de creación de perfiles puedan barrer los tamaños de los grupos de subprocesos sin volver a compilar. 【crates/fastpq_prover/src/metal.rs:321】- Sintonice el mosaico FFT directamente: el host ahora deriva carriles de grupo de subprocesos (16 para rastros cortos, 32 una vez `log_len ≥ 6`, 64 una vez `log_len ≥ 10`, 128 una vez `log_len ≥ 14` y 256 en `log_len ≥ 18`) y la profundidad del mosaico (5 etapas para rastros pequeños, 4 cuando `log_len ≥ 12`, y una vez que el dominio alcanza `log_len ≥ 18/20/22`, el pase de memoria compartida ahora se ejecuta 14/12/16 etapas antes de entregar el control al kernel posterior al mosaico) desde el dominio solicitado más los subprocesos de ancho/máximo de ejecución del dispositivo. Anule con `FASTPQ_METAL_FFT_LANES` (potencia de dos entre 8 y 256) e `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) para fijar formas de lanzamiento específicas; ambos valores fluyen a través de `FftArgs`, se fijan en la ventana compatible y se registran para los barridos de perfiles.
- El procesamiento por lotes de columnas FFT/IFFT y LDE ahora se deriva del ancho del grupo de subprocesos resuelto: el host apunta aproximadamente a 4096 subprocesos lógicos por búfer de comando, fusiona hasta 64 columnas a la vez con la preparación de mosaicos de búfer circular y solo avanza hacia abajo a través de 64→32→16→8→4→2→1 columnas a medida que el dominio de evaluación cruza el 2¹⁶/2¹⁸/2²⁰/2²². umbrales. Esto mantiene la captura de 20.000 filas en ≥64 columnas por envío y, al mismo tiempo, garantiza que las clases laterales largas sigan finalizando de forma determinista. El programador adaptativo aún duplica el ancho de la columna hasta que los despachos se acercan al objetivo de ≈2 ms y ahora reduce a la mitad el lote automáticamente cada vez que un despacho muestreado llega ≥30 % por encima de ese objetivo, por lo que las transiciones de carril/mosaico que inflan el costo por columna retroceden sin anulaciones manuales. Las permutaciones de Poseidon comparten el mismo programador adaptativo y el bloque `metal_heuristics.batch_columns.poseidon` en `fastpq_metal_bench` ahora registra el recuento de estado resuelto, el límite, la última duración y el indicador de anulación para que la telemetría de profundidad de la cola se pueda vincular directamente al ajuste de Poseidon. Anule con `FASTPQ_METAL_FFT_COLUMNS` (1–64) para fijar un tamaño de lote de FFT determinista y use `FASTPQ_METAL_LDE_COLUMNS` (1–64) cuando necesite que el despachador LDE respete un recuento de columnas fijo; el banco de metal muestra las entradas `kernel_profiles.*.columns` resueltas en cada captura para que los experimentos de ajuste sigan siendo reproducibles.- El envío de colas múltiples ahora es automático en Mac discretas: el host inspecciona `is_low_power`, `is_headless` y la ubicación del dispositivo para decidir si activar dos colas de comandos Metal, solo se distribuye cuando la carga de trabajo lleva al menos 16 columnas (escalada según la distribución resuelta) y realiza operaciones por turnos en los lotes de columnas para que los seguimientos largos mantengan ambos carriles de GPU ocupados sin sacrificar determinismo. El semáforo del búfer de comando ahora impone un mínimo de “dos en vuelo por cola”, y la telemetría de la cola registra la ventana de medición agregada (`window_ms`) más las tasas de ocupación normalizadas (`busy_ratio`) para el semáforo global y cada entrada de la cola, de modo que los artefactos de liberación puedan demostrar que ambas colas permanecieron ≥50% ocupadas durante el mismo lapso de tiempo. Anule los valores predeterminados con `FASTPQ_METAL_QUEUE_FANOUT` (1 a 4 carriles) e `FASTPQ_METAL_COLUMN_THRESHOLD` (total mínimo de columnas antes de la distribución); Las pruebas de paridad de Metal fuerzan las anulaciones para que las Mac con múltiples GPU permanezcan cubiertas y la política resuelta se registra junto con la telemetría de profundidad de la cola y el nuevo `metal_dispatch_queue.queues[*]`. bloque.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
- La detección de metales ahora sondea `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` directamente (calentando CoreGraphics en shells sin cabeza) antes de volver a `system_profiler`, e `FASTPQ_DEBUG_METAL_ENUM` imprime los dispositivos enumerados cuando se configura de modo que las ejecuciones de CI sin cabeza pueden explicar por qué `FASTPQ_GPU=gpu` todavía se degradó a la CPU camino. Cuando la anulación se establece en `gpu` pero no se detecta ningún acelerador, `fastpq_metal_bench` ahora genera un error inmediatamente con un puntero a la perilla de depuración en lugar de continuar silenciosamente en la CPU. Esto reduce la clase de "reserva silenciosa de CPU" mencionada en WP2-E y brinda a los operadores una perilla para capturar registros de enumeración dentro de puntos de referencia envueltos.
  - Los tiempos de GPU de Poseidon ahora se niegan a tratar las reservas de CPU como datos de "GPU". `hash_columns_gpu` informa si el acelerador realmente se ejecutó, `measure_poseidon_gpu` arroja muestras (y registra una advertencia) cada vez que la canalización retrocede y el microbench secundario Poseidon sale con un error si el hash de GPU no está disponible. Como resultado, `gpu_recorded=false` cada vez que la ejecución de Metal retrocede, el resumen de la cola aún registra la ventana de envío fallida y los resúmenes del panel marcan inmediatamente la regresión. El contenedor (`scripts/fastpq/wrap_benchmark.py`) ahora falla cuando `metal_dispatch_queue.poseidon.dispatch_count == 0`, por lo que los paquetes de Stage7 no se pueden firmar sin el envío real de GPU Poseidon. evidencia.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】【scripts/fastpq/wrap_benchmark.py:912】- El hash de Poseidón ahora refleja ese contrato de puesta en escena. `PoseidonColumnBatch` produce búferes de carga útil aplanados más descriptores de desplazamiento/longitud, el host modifica la base de esos descriptores por lote y ejecuta un búfer doble `COLUMN_STAGING_PIPE_DEPTH` para que las cargas de carga útil + descriptor se superpongan con el trabajo de la GPU, y ambos núcleos Metal/CUDA consumen los descriptores directamente para que cada envío absorba todos los bloques de velocidad acolchados en el dispositivo antes de emitir los resúmenes de columnas. `hash_columns_from_coefficients` ahora transmite esos lotes a través de un subproceso de trabajo de GPU, manteniendo más de 64 columnas en funcionamiento de forma predeterminada en GPU discretas (ajustable a través de `FASTPQ_POSEIDON_PIPE_COLUMNS`/`FASTPQ_POSEIDON_PIPE_DEPTH`). El banco de metal registra la configuración de canalización resuelta + recuentos de lotes en `metal_dispatch_queue.poseidon_pipeline`, y `kernel_profiles.poseidon.bytes` incluye el tráfico del descriptor para que las capturas de Stage7 prueben la nueva ABI. de un extremo a otro.【crates/fastpq_prover/src/trace.rs:604】【crates/fastpq_prover/src/trace.rs:809】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:196 3】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2675】【crates/fastpq_prover/src/metal.rs:2290】【crates/fastpq_prover/cuda/fastpq_cuda.cu:351】
- El hash Poseidon fusionado Stage7-P2 ahora aterriza en ambos backends de GPU. El trabajador de transmisión alimenta porciones `PoseidonColumnBatch::column_window()` contiguas a `hash_columns_gpu_fused`, que las canaliza a `poseidon_hash_columns_fused` para que cada envío escriba `leaf_digests || parent_digests` con la asignación principal canónica `(⌈columns / 2⌉)`. `ColumnDigests` mantiene ambos sectores, y `merkle_root_with_first_level` consume la capa principal inmediatamente, por lo que la CPU nunca vuelve a calcular los nodos de profundidad 1 y la telemetría Stage7 puede afirmar que las capturas de GPU informan cero padres "de reserva" siempre que el núcleo fusionado tiene éxito.【crates/fastpq_prover/src/trace.rs:1070】【crates/fastpq_prover/src/gpu.rs:365】【crates/fastpq_prover/src/metal.rs:2422】【crates/fastpq_prover/cuda/fastpq_cuda.cu:631】
- `fastpq_metal_bench` ahora emite un bloque `device_profile` con el nombre del dispositivo Metal, ID de registro, indicadores `low_power`/`headless`, ubicación (integrada, ranura, externa), indicador discreto, `hw.model` y la etiqueta de SoC de Apple derivada (por ejemplo, “M3 Máximo”). Los paneles de Stage7 consumen este campo para agrupar capturas de M4/M3 frente a GPU discretas sin analizar nombres de host, y el JSON se envía junto a la cola/evidencia heurística para que cada artefacto de lanzamiento demuestre qué clase de flota produjo la ejecución.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2536】- La superposición de host/dispositivo FFT ahora utiliza una ventana de preparación con doble búfer: mientras el lote *n* finaliza dentro de `fastpq_fft_post_tiling`, el host aplana el lote *n+1* en el segundo búfer de preparación y solo se detiene cuando se debe reciclar un búfer. El backend registra cuántos lotes se aplanaron más el tiempo empleado en aplanar en comparación con esperar a que se complete la GPU, y `fastpq_metal_bench` muestra el bloque `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` agregado para que los artefactos de lanzamiento puedan demostrar la superposición en lugar de paradas silenciosas del host. El informe JSON ahora también desglosa los totales por fase en `column_staging.phases.{fft,lde,poseidon}`, lo que permite que las capturas de Stage7 demuestren si la preparación FFT/LDE/Poseidon está vinculada al host o está esperando a que se complete la GPU. Las permutaciones de Poseidon reutilizan los mismos buffers de preparación agrupados, por lo que las capturas `--operation poseidon_hash_columns` ahora emiten los deltas `column_staging` específicos de Poseidon junto con la evidencia de profundidad de la cola sin instrumentación personalizada. Las nuevas matrices `column_staging.samples.{fft,lde,poseidon}` registran las tuplas `batch/flatten_ms/wait_ms/wait_ratio` por lote, lo que hace que sea trivial demostrar que la superposición `COLUMN_STAGING_PIPE_DEPTH` se mantiene (o detectar cuándo el host comienza a esperar la GPU). terminaciones).【crates/fastpq_prover/src/metal.rs:319】【crates/fastpq_prover/src/metal.rs:330】【crates/fastpq_prover/src/metal.rs:1813】【crates/fas tpq_prover/src/metal.rs:2488】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1189】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1216】- La aceleración de Poseidon2 ahora se ejecuta como un núcleo metálico de alta ocupación: cada grupo de subprocesos copia las constantes de ronda y las filas de MDS en la memoria del grupo de subprocesos, desenrolla las rondas completas/parciales y recorre varios estados por carril para que cada envío lance al menos 4096 subprocesos lógicos. Anule la forma de lanzamiento mediante `FASTPQ_METAL_POSEIDON_LANES` (potencias de dos entre 32 y 256, sujetas al límite del dispositivo) e `FASTPQ_METAL_POSEIDON_BATCH` (1 a 32 estados por carril) para reproducir experimentos de creación de perfiles sin reconstruir `fastpq.metallib`; el host de Rust pasa el ajuste resuelto a través de `PoseidonArgs` antes de enviarlo. El host ahora toma instantáneas de `MTLDevice::{is_low_power,is_headless,location}` una vez por arranque y desvía automáticamente las GPU discretas hacia lanzamientos por niveles de VRAM (`256×24` en partes de ≥48 GiB, `256×20` en 32 GiB, `256×16` en caso contrario) mientras que los SoC de bajo consumo se apegan a `256×8` (las alternativas para hardware de 128/64 carriles continúan usando 8/6 estados por carril), por lo que los operadores obtienen la profundidad de canalización de >16 estados sin tocar las variables ambientales. `fastpq_metal_bench` se vuelve a ejecutar bajo `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` para capturar un bloque `poseidon_microbench` dedicado que compara el carril escalar con el kernel multiestado para que los artefactos de lanzamiento puedan citar una aceleración concreta. Lo mismo captura la telemetría de superficie `poseidon_pipeline` (`chunk_columns`, `pipe_depth`, `batches`, `fallbacks`), por lo que la evidencia de Stage7 demuestra la ventana de superposición en cada GPU. trace.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】【cra tes/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/trace.rs:299】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】
  - La puesta en escena de mosaicos LDE ahora refleja la heurística FFT: los rastros pesados solo ejecutan 12 etapas en el pase de memoria compartida una vez `log₂(len) ≥ 18`, bajan a 10 etapas en log₂20 y se fijan en ocho etapas en log₂22 para que las mariposas anchas se muevan hacia el kernel posterior al mosaico. Anule con `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) siempre que necesite una profundidad determinista; el host solo inicia el envío posterior al mosaico cuando la heurística se detiene antes de tiempo, de modo que la profundidad de la cola y la telemetría del kernel siguen siendo deterministas. 【crates/fastpq_prover/src/metal.rs:827】
  - Microoptimización del kernel: los mosaicos FFT/LDE de memoria compartida ahora reutilizan las zancadas de giro y coset por carril en lugar de reevaluar `pow_mod*` para cada mariposa. Cada carril precalcula `w_seed`, `w_stride` y (cuando sea necesario) la zancada lateral una vez por bloque, luego fluye a través de los desplazamientos, reduciendo las multiplicaciones escalares dentro de `apply_stage_tile`/`apply_stage_global` y reduciendo la media LDE de 20k filas a ~1,55 s con la heurística más reciente (aún por encima del objetivo de 950 ms, pero una mejora adicional de ~50 ms con respecto al ajuste de solo procesamiento por lotes).- La suite del kernel ahora tiene una referencia dedicada (`docs/source/fastpq_metal_kernels.md`) que documenta cada punto de entrada, los límites de grupo de subprocesos/mosaicos aplicados en `fastpq.metallib` y los pasos de reproducción para compilar metallib manualmente.【docs/source/fastpq_metal_kernels.md:1】
  - El informe de referencia ahora emite un objeto `post_tile_dispatches` que registra cuántos lotes FFT/IFFT/LDE se ejecutaron en el kernel posterior al mosaico dedicado (recuentos de envío por tipo más los límites de etapa/log₂). `scripts/fastpq/wrap_benchmark.py` copia el bloque en `benchmarks.post_tile_dispatches`/`benchmarks.post_tile_summary`, y la puerta de manifiesto rechaza las capturas de GPU que omiten la evidencia, por lo que cada artefacto de 20 000 filas prueba que se ejecutó el kernel de múltiples pasos. en el dispositivo.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - Configure `FASTPQ_METAL_TRACE=1` para emitir registros de depuración por envío (etiqueta de canalización, ancho de grupo de subprocesos, grupos de lanzamiento, tiempo transcurrido) para la correlación de trazas de instrumentos/metal.【crates/fastpq_prover/src/metal.rs:346】
- La cola de despacho ahora está instrumentada: `FASTPQ_METAL_MAX_IN_FLIGHT` limita los búferes de comandos metálicos concurrentes (el valor predeterminado automático se deriva del recuento de núcleos de GPU detectado a través de `system_profiler`, fijado al menos al piso de distribución de la cola con un respaldo de paralelismo del host cuando macOS se niega a informar el dispositivo). El banco permite el muestreo de profundidad de la cola para que el JSON exportado lleve un objeto `metal_dispatch_queue` con los campos `limit`, `dispatch_count`, `max_in_flight`, `busy_ms` e `overlap_ms` para evidencia de liberación, agrega un archivo anidado. Bloque `metal_dispatch_queue.poseidon` cada vez que se ejecuta una captura exclusiva de Poseidon (`--operation poseidon_hash_columns`) y emite un bloque `metal_heuristics` que describe el límite del búfer de comando resuelto más las columnas de lote FFT/LDE (incluido si las anulaciones forzaron los valores) para que los revisores puedan auditar las decisiones de programación junto con la telemetría. Los núcleos Poseidon también alimentan un bloque `poseidon_profiles` dedicado destilado de las muestras del núcleo para que se realice un seguimiento de los bytes/hilos, la ocupación y la geometría de envío en todos los artefactos. Si la ejecución principal no puede recopilar la profundidad de la cola o las estadísticas de llenado cero de LDE (por ejemplo, cuando un envío de GPU vuelve silenciosamente a la CPU), el arnés activa automáticamente un envío de sonda única para recopilar la telemetría faltante y ahora sintetiza los tiempos de llenado cero del host cuando la GPU se niega a informarlos, por lo que la evidencia publicada siempre incluye el `zero_fill`. bloque.【crates/fastpq_prover/src/metal.rs:2056】【crates/fastpq_prover/src/metal.rs:247】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - Configure `FASTPQ_SKIP_GPU_BUILD=1` al realizar una compilación cruzada sin la cadena de herramientas Metal; la advertencia registra el salto y el planificador continúa en la ruta de la CPU.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】- La detección en tiempo de ejecución utiliza `system_profiler` para confirmar la compatibilidad con Metal; Si falta el marco, el dispositivo o la biblioteca metálica, el script de compilación borra `FASTPQ_METAL_LIB` y el planificador permanece en la CPU determinista. ruta.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover/src/metal.rs:43】
  - Lista de verificación del operador (hosts metálicos):
    1. Confirme que la cadena de herramientas esté presente y que `FASTPQ_METAL_LIB` apunte a un `.metallib` compilado (`echo $FASTPQ_METAL_LIB` no debe estar vacío después de `cargo build --features fastpq-gpu`).【crates/fastpq_prover/build.rs:188】
    2. Ejecute pruebas de paridad con los carriles de GPU habilitados: `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`. Esto ejercita los núcleos de Metal y retrocede automáticamente si falla la detección.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
    3. Capture una muestra de referencia para paneles: localice la biblioteca Metal compilada
       (`fd -g 'fastpq.metallib' target/release/build | head -n1`), exportarlo vía
       `FASTPQ_METAL_LIB` y ejecute
      `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
       El conjunto canónico `fastpq-lane-balanced` ahora rellena cada captura a 32,768 filas, por lo que el
       JSON refleja tanto las 20.000 filas solicitadas como el dominio acolchado que impulsa la GPU
       granos. Cargue el JSON/log en su almacén de pruebas; el flujo de trabajo nocturno de macOS se refleja
      esta ejecución y archiva los artefactos como referencia. El informe registra
     `fft_tuning.{threadgroup_lanes,tile_stage_limit}` junto con el `speedup` de cada operación, el
     La sección LDE agrega `zero_fill.{bytes,ms,queue_delta}` para que los artefactos de liberación demuestren determinismo,
     sobrecarga de llenado cero del host y el uso incremental de la cola de GPU (límite, recuento de envío,
     pico en vuelo, tiempo ocupado/superpuesto) y el nuevo bloque `kernel_profiles` captura por kernel
     índices de ocupación, ancho de banda estimado y rangos de duración para que los paneles puedan marcar la GPU
       regresiones sin reprocesar muestras sin procesar.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
       Espere que la ruta Metal LDE se mantenga por debajo de 950 ms (objetivo `<1 s` en hardware Apple serie M);
4. Capture la telemetría de uso de filas desde un ExecWitness real para que los paneles puedan registrar el dispositivo de transferencia
   adopción. Busque un testigo de Torii
  (`iroha_cli audit witness --binary --out exec.witness`) y decodificarlo con
  `iroha_cli audit witness --decode exec.witness` (opcionalmente agregar
  `--fastpq-parameter fastpq-lane-balanced` para afirmar el conjunto de parámetros esperado; lotes FASTPQ
  emitir por defecto; pase `--no-fastpq-batches` solo si necesita recortar la salida).
   Cada entrada de lote ahora emite un objeto `row_usage` (`total_rows`, `transfer_rows`,
   `non_transfer_rows`, recuentos por selector e `transfer_ratio`). Archive ese fragmento JSON
   reprocesamiento de transcripciones sin procesar. 【crates/iroha_cli/src/audit.rs:209】 Compare la nueva captura con
   la línea de base anterior con `scripts/fastpq/check_row_usage.py`, por lo que CI falla si las relaciones de transferencia o
   el total de filas regresa:

   ```bash
   python3 scripts/fastpq/check_row_usage.py \
     --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
     --candidate fastpq_row_usage_2025-05-12.json \
     --max-transfer-ratio-increase 0.005 \
     --max-total-rows-increase 0
   ```Los blobs JSON de muestra para pruebas de humo se encuentran en `scripts/fastpq/examples/`. Localmente puedes ejecutar `make check-fastpq-row-usage`
   (envuelve `ci/check_fastpq_row_usage.sh`), y CI ejecuta el mismo script a través de `.github/workflows/fastpq-row-usage.yml` para comparar el compromiso
   Instantáneas `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json` para que el paquete de evidencia falle rápidamente siempre que
   las filas de transferencia vuelven a subir. Pase `--summary-out <path>` si desea una diferencia legible por máquina (el trabajo de CI carga `fastpq_row_usage_summary.json`).
   Cuando un ExecWitness no sea útil, sintetice una muestra de regresión con `fastpq_row_bench`
   (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`), que emite exactamente el mismo `row_usage`
   objeto para recuentos de selectores configurables (por ejemplo, una prueba de esfuerzo de 65536 filas):

   ```bash
   cargo run -p fastpq_prover --bin fastpq_row_bench -- \
     --transfer-rows 65536 \
     --mint-rows 256 \
     --burn-rows 128 \
     --pretty \
     --output artifacts/fastpq_benchmarks/fastpq_row_usage_65k.json
   ```Los paquetes de implementación Stage7-3 también deben pasar `scripts/fastpq/validate_row_usage_snapshot.py`, que
   impone que cada entrada `row_usage` contenga los recuentos del selector y que
   `transfer_ratio = transfer_rows / total_rows`; `ci/check_fastpq_rollout.sh` llama al ayudante
   automáticamente, por lo que los paquetes a los que les faltan esas invariantes fallan antes de que los carriles de GPU sean obligatorios.【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
       la puerta de manifiesto del banco aplica esto a través de `--max-operation-ms lde=950`, así que actualice el
       capturar siempre que su evidencia exceda ese límite.
      Cuando también necesite evidencia de Instrumentos, pase `--trace-dir <dir>` para que el arnés
      se reinicia a través de `xcrun xctrace record` (plantilla predeterminada “Metal System Trace”) y
      almacena un archivo `.trace` con marca de tiempo junto con el JSON; aún puedes anular la ubicación /
      plantilla manualmente con `--trace-output <path>` más `--trace-template` opcional /
      `--trace-seconds`. El JSON resultante anuncia `metal_trace_{template,seconds,output}`, por lo que
      Los paquetes de artefactos siempre identifican el rastro capturado. 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      Envuelve cada captura con
      `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output`
       (agregue `--gpg-key <fingerprint>` si necesita fijar una identidad de firma) para que el paquete falle
       rápido siempre que la GPU LDE media infrinja el objetivo de 950 ms, Poseidon supere 1 s o el
       Faltan bloques de telemetría Poseidon, incorpora un `row_usage_snapshot`
      junto al JSON, aparece el resumen del microbench Poseidon en `benchmarks.poseidon_microbench`,
      y todavía incluye metadatos para runbooks y el panel Grafana
    (`dashboards/grafana/fastpq_acceleration.json`). El JSON ahora emite `speedup.ratio` /
     `speedup.delta_ms` por operación para que la evidencia de publicación pueda probar GPU vs.
     La CPU gana sin reprocesar las muestras sin procesar, y el contenedor copia tanto las
     estadísticas de relleno cero (más `queue_delta`) en `zero_fill_hotspots` (bytes, latencia, derivados
     GB/s), registra los metadatos de los instrumentos en `metadata.metal_trace`, enhebra el opcional
     Bloque `metadata.row_usage_snapshot` cuando se suministra `--row-usage <decoded witness>` y aplana el
     contadores por kernel en `benchmarks.kernel_summary`, por lo que se rellenan los cuellos de botella y la cola de metal
     Las regresiones de utilización, ocupación del kernel y ancho de banda son visibles de un vistazo sin
     espeleología del informe sin procesar.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】【scripts/fastpq/wrap_benchmark.py:1】【artifacts/fastpq_benchmarks/fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】
     Debido a que la instantánea del uso de la fila ahora viaja con el artefacto envuelto, los boletos de implementación simplemente
     haga referencia al paquete en lugar de adjuntar un segundo fragmento JSON, y CI puede diferenciar el incrustado
    cuenta directamente al validar los envíos de Stage7. Para archivar los datos del microbench por sí solo,
    ejecutar `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json`
    y almacene el archivo resultante en `benchmarks/poseidon/`. Mantenga actualizado el manifiesto agregado con
    `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`
    para que los paneles/CI puedan diferenciar el historial completo sin tener que recorrer cada archivo manualmente.4. Valide la telemetría curvando `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` (punto final Prometheus) o buscando registros `telemetry::fastpq.execution_mode`; Las entradas inesperadas de `resolved="cpu"` indican que el host retrocedió a pesar de la intención de la GPU.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    5. Utilice `FASTPQ_GPU=cpu` (o la perilla de configuración) para forzar la ejecución de la CPU durante el mantenimiento y confirmar que los registros de respaldo aún aparecen; esto mantiene los runbooks de SRE alineados con la ruta determinista. 【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- Telemetría y respaldo:
  - Los registros del modo de ejecución (`telemetry::fastpq.execution_mode`) y los contadores (`fastpq_execution_mode_total{device_class="…", backend="metal"|…}`) exponen el modo solicitado versus el modo resuelto para que los respaldos silenciosos sean visibles en los paneles. 【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - La placa `FASTPQ Acceleration Overview` Grafana (`dashboards/grafana/fastpq_acceleration.json`) visualiza la tasa de adopción de Metal y se vincula a los artefactos de referencia, mientras que las reglas de alerta emparejadas (`dashboards/alerts/fastpq_acceleration_rules.yml`) se implementan en rebajas sostenidas.
  - Las anulaciones de `FASTPQ_GPU={auto,cpu,gpu}` siguen siendo compatibles; los valores desconocidos generan advertencias pero aún se propagan a la telemetría para su auditoría.【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - Las pruebas de paridad de GPU (`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`) deben pasar para CUDA y Metal; CI salta elegantemente cuando metallib está ausente o la detección falla.【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/backend.rs:346】
  - Evidencia de preparación del metal (archive los artefactos a continuación con cada implementación para que la auditoría de la hoja de ruta pueda demostrar el determinismo, la cobertura de telemetría y el comportamiento alternativo):| Paso | Gol | Comando/Evidencia |
    | ---- | ---- | ------------------ |
    | Construir metallib | Asegúrese de que `xcrun metal`/`xcrun metallib` estén disponibles y emita el determinista `.metallib` para esta confirmación | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"`; `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`; `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"`; exportar `FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    | Verificar var env | Confirme que Metal permanece habilitado verificando la var de entorno registrada por el script de compilación | `echo $FASTPQ_METAL_LIB` (debe devolver una ruta absoluta; vacío significa que el backend fue deshabilitado).【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
    | Conjunto de paridad de GPU | Demostrar que los núcleos se ejecutan (o emitir registros de degradación deterministas) antes del envío | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` y almacene el fragmento de registro resultante que muestra `backend="metal"` o la advertencia de respaldo.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】
    | Muestra de referencia | Capture el par JSON/log que registra `speedup.*` y el ajuste FFT para que los paneles puedan ingerir evidencia del acelerador | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; archive el JSON, la marca de tiempo `.trace` y la salida estándar junto con las notas de la versión para que la placa Grafana recoja la ejecución de Metal (el informe registra las 20 000 filas solicitadas más el dominio acolchado de 32 768 filas para que los revisores puedan confirmar el LDE `<1 s` objetivo).【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    | Informe de envoltura y firma | Fallar el lanzamiento si la media LDE de la GPU supera los 950 ms, Poseidon excede 1 s o faltan bloques de telemetría de Poseidon y se produce un paquete de artefactos firmados | `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`; envíe tanto el JSON empaquetado como la firma `.json.asc` generada para que los auditores puedan verificar las métricas de menos de un segundo sin volver a ejecutar la carga de trabajo.【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
    | Manifiesto de banco firmado | Aplicar evidencia LDE `<1 s` en paquetes Metal/CUDA y capturar resúmenes firmados para aprobación de lanzamiento | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`; adjunte el manifiesto + la firma al ticket de lanzamiento para que la automatización posterior pueda validar las métricas de prueba de menos de un segundo. 【xtask/src/fastpq.rs:1】 【artifacts/fastpq_benchmarks/README.md:65】
| Paquete CUDA | Mantenga la captura SM80 CUDA al mismo tiempo que la evidencia de Metal para que los manifiestos cubran ambas clases de GPU. | `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` en el host Xeon+RTX → `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_cuda_bench.json artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --label device_class=xeon-rtx-sm80 --sign-output`; agregue la ruta empaquetada a `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`, mantenga el par `.json`/`.asc` junto al paquete de metal y cite el `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` sembrado cuando los auditores necesiten una referencia. diseño.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】
| Comprobación de telemetría | Validar las superficies Prometheus/OTEL reflejan `device_class="<matrix>", backend="metal"` (o registrar la degradación) | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` y copie el registro `telemetry::fastpq.execution_mode` emitido al inicio.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】| Taladro de retroceso forzado | Documentar la ruta determinista de la CPU para los libros de estrategias de SRE | Ejecute una carga de trabajo breve con `FASTPQ_GPU=cpu` o `zk.fastpq.execution_mode = "cpu"` y capture el registro de degradación para que los operadores puedan ensayar el procedimiento de reversión. 【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
    | Captura de seguimiento (opcional) | Al crear perfiles, capture los seguimientos de envío para que las anulaciones de mosaicos/carriles del kernel se puedan revisar más adelante | Vuelva a ejecutar una prueba de paridad con `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` y adjunte el registro de seguimiento producido a sus artefactos de lanzamiento.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    Archive la evidencia con el ticket de lanzamiento y refleje la misma lista de verificación en `docs/source/fastpq_migration_guide.md` para que los lanzamientos de puesta en escena/producción sigan un manual idéntico.【docs/source/fastpq_migration_guide.md:1】

### Aplicación de la lista de verificación de liberación

Agregue las siguientes puertas a cada boleto de lanzamiento de FASTPQ. Los lanzamientos se bloquean hasta que todos los artículos estén
completos y adjuntos como artefactos firmados.

1. **Métricas de prueba en menos de un segundo**: la captura de referencia canónica de Metal
   (`fastpq_metal_bench_*.json`) debe demostrar que la carga de trabajo de 20000 filas (32768 filas acolchadas) termina en
   <1s. Concretamente, la entrada `benchmarks.operations` donde `operation = "lde"` y la coincidencia
   La muestra `report.operations` debe mostrar `gpu_mean_ms ≤ 950`. Los tramos que exceden el techo requieren
   investigación y una recaptura antes de que se pueda firmar la lista de verificación.
2. **Manifiesto de referencia firmado**: después de grabar paquetes nuevos de Metal + CUDA, ejecute
   `cargo xtask fastpq-bench-manifest … --signing-key <path>` para emitir
   `artifacts/fastpq_bench_manifest.json` y la firma separada
   (`artifacts/fastpq_bench_manifest.sig`). Adjunte ambos archivos más la huella digital de la clave pública al
   Libere el ticket para que los revisores puedan verificar el resumen y la firma de forma independiente. 【xtask/src/fastpq.rs:1】
3. **Adjuntos de evidencia**: almacene el JSON de referencia sin procesar, el registro de salida estándar (o el seguimiento de instrumentos, cuando
   capturado) y el par manifiesto/firma con el ticket de liberación. La lista de verificación es sólo
   se considera verde cuando el boleto se vincula a esos artefactos y el revisor de guardia confirma el
   El resumen registrado en `fastpq_bench_manifest.json` coincide con los archivos cargados. 【artifacts/fastpq_benchmarks/README.md:1】

## Etapa 6: endurecimiento y documentación
- Se retiró el backend de marcador de posición; El proceso de producción se envía de forma predeterminada sin cambios de funciones.
- Construcciones reproducibles (fijar cadenas de herramientas, imágenes de contenedores).
- Fuzzers para traza, SMT, estructuras de búsqueda.
- Las pruebas de humo a nivel de probador cubren subvenciones de votación de gobernanza y transferencias de remesas para mantener estables los partidos de Stage6 antes de la implementación completa de IVM.【crates/fastpq_prover/tests/realistic_flows.rs:1】
- Runbooks con umbrales de alerta, procedimientos de remediación, pautas de planificación de capacidad.
- Reproducción a prueba de arquitectura cruzada (x86_64, ARM64) en CI.

### Manifiesto del banco y puerta de liberación

La evidencia de publicación ahora incluye un manifiesto determinista que cubre tanto Metal como
Paquetes de referencia CUDA. Ejecutar:

```bash
cargo xtask fastpq-bench-manifest \
  --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json \
  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \
  --signing-key secrets/fastpq_bench.ed25519 \
  --out artifacts/fastpq_bench_manifest.json
```El comando valida los paquetes empaquetados, aplica umbrales de latencia/aceleración,
emite resúmenes BLAKE3 + SHA-256 y (opcionalmente) firma el manifiesto con un
Clave Ed25519 para que las herramientas de liberación puedan verificar la procedencia. Ver
`xtask/src/fastpq.rs`/`xtask/src/main.rs` para la implementación y
`artifacts/fastpq_benchmarks/README.md` para obtener orientación operativa.

> **Nota:** Los paquetes metálicos que omiten `benchmarks.poseidon_microbench` ahora causan
> la generación manifiesta de fallar. Vuelva a ejecutar `scripts/fastpq/wrap_benchmark.py`
> (y `scripts/fastpq/export_poseidon_microbench.py` si necesita uno independiente
> resumen) cada vez que falta la evidencia de Poseidón, por lo que se manifiesta la liberación
> capturar siempre la comparación escalar versus predeterminada.【xtask/src/fastpq.rs:409】

El indicador `--matrix` (predeterminado en `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
cuando está presente) carga las medianas entre dispositivos capturadas por
`scripts/fastpq/capture_matrix.sh`. El manifiesto codifica el piso de 20000 filas y
Límites de latencia/aceleración por operación para cada clase de dispositivo, personalizados
Las anulaciones de `--require-rows`/`--max-operation-ms`/`--min-operation-speedup` no son
ya no es necesario a menos que esté depurando una regresión específica.

Actualice la matriz agregando rutas de referencia ajustadas al
Listas `artifacts/fastpq_benchmarks/matrix/devices/<label>.txt` y ejecutándose
`scripts/fastpq/capture_matrix.sh`. El script captura las medianas por dispositivo,
emite el `matrix_manifest.json` consolidado e imprime la ruta relativa que
`cargo xtask fastpq-bench-manifest` consumirá. El AppleM4, Xeon+RTX y
Listas de captura Neoverse+MI300 (`devices/apple-m4-metal.txt`,
`devices/xeon-rtx-sm80.txt`, `devices/neoverse-mi300.txt`) más sus envueltos
paquetes de referencia
(`fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json`,
`fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`,
`fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json`) ahora están marcados
en, por lo que cada versión aplica las mismas medianas entre dispositivos antes del manifiesto
es firmado.【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-metal.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/x eon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【artifacts/fastpq_benchmarks/fastp q_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】【artifacts/fastpq_benchmarks/fastpq_cuda_bench_2025-11-12T09050 1Z_ubuntu24_x86_64.json:1】【artifacts/fastpq_benchmarks/fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json:1】

---

## Resumen de críticas y acciones abiertas

## Etapa 7: Adopción de flotas y evidencia de implementación

Stage7 lleva el probador de "documentado y evaluado" (Stage6) a
“preparado por defecto para flotas de producción”. La atención se centra en la ingesta de telemetría,
paridad de captura entre dispositivos y paquetes de evidencia del operador para acelerar la GPU
puede ser ordenado de manera determinista.- **Etapa 7-1: Ingestión de telemetría de flota y SLO.** Paneles de producción
  (`dashboards/grafana/fastpq_acceleration.json`) debe conectarse a corriente
  Prometheus/OTel se alimenta con cobertura de Alertmanager para paradas con mucha cola,
  regresiones de relleno cero y respaldos silenciosos de CPU. El paquete de alerta permanece bajo
  `dashboards/alerts/fastpq_acceleration_rules.yml` y alimenta la misma evidencia
  paquete requerido en Stage6. 【dashboards/grafana/fastpq_acceleration.json:1】 【dashboards/alerts/fastpq_acceleration_rules.yml:1】
  El panel ahora expone variables de plantilla para `device_class`, `chip_family`,
  e `gpu_kind`, lo que permite a los operadores orientar la adopción de metales según la matriz exacta
  etiqueta (por ejemplo, `apple-m4-max`), por familia de chips Apple o por etiquetas discretas vs.
  clases de GPU integradas sin editar las consultas.
  Los nodos macOS creados con `irohad --features fastpq-gpu` ahora emiten
  `fastpq_execution_mode_total{device_class,chip_family,gpu_kind,...}`,
  `fastpq_metal_queue_ratio{device_class,chip_family,gpu_kind,queue,metric}`
  (índices de ocupación/superposición), y
  `fastpq_metal_queue_depth{device_class,chip_family,gpu_kind,metric}`
  (límite, max_in_flight, despacho_count, ventana_segundos) para que los paneles y
  Las reglas de Alertmanager pueden leer el ciclo de trabajo/espacio libre del semáforo metálico directamente desde
  Prometheus sin esperar un paquete de referencia. Los hosts ahora exportan
  `fastpq_zero_fill_duration_ms{device_class,chip_family,gpu_kind}` y
  `fastpq_zero_fill_bandwidth_gbps{device_class,chip_family,gpu_kind}` siempre que
  el asistente LDE pone a cero los buffers de evaluación de GPU y Alertmanager obtuvo la
  `FastpqQueueHeadroomLow` (espacio libre 0,40 ms en 15 m) para que el espacio libre de la cola y
  operadores de página de regresiones de relleno cero inmediatamente en lugar de esperar a que
  siguiente punto de referencia envuelto. Una nueva alerta a nivel de página `FastpqCpuFallbackBurst` rastrea
  Solicitudes de GPU que aterrizan en el backend de la CPU durante más del 5% de la carga de trabajo,
  obligar a los operadores a capturar evidencia y causar fallas transitorias de GPU
  antes de volver a intentar el lanzamiento.【crates/irohad/src/main.rs:2345】【crates/iroha_telemetry/src/metrics.rs:4436】【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
  El SLO establecido ahora también aplica el objetivo de ciclo de trabajo del metal ≥50% a través del
  Regla `FastpqQueueDutyCycleDrop`, que promedia
  `fastpq_metal_queue_ratio{metric="busy"}` durante una ventana móvil de 15 minutos y
  advierte cuando todavía se está programando el trabajo de la GPU pero una cola no logra mantener el
  ocupación requerida. Esto mantiene el contrato de telemetría en vivo alineado con el
  evidencia comparativa antes de que se exijan los carriles de GPU. 【dashboards/alerts/fastpq_acceleration_rules.yml:1】 【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
- **Etapa 7-2: Matriz de captura entre dispositivos.** El nuevo
  `scripts/fastpq/capture_matrix.sh` compila
  `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json` desde por dispositivo
  listas de captura en `artifacts/fastpq_benchmarks/matrix/devices/`. manzanaM4,
  Las medianas Xeon+RTX y Neoverse+MI300 ahora viven en el repositorio junto con sus
  paquetes envueltos, por lo que `cargo xtask fastpq-bench-manifest` carga el manifiesto
  automáticamente, aplica el límite mínimo de 20 000 filas y se aplica por dispositivo
  Límites de latencia/aceleración sin indicadores CLI personalizados antes de que se publique un paquete de lanzamiento.aprobado.【scripts/fastpq/capture_matrix.sh:1】【artifacts/fastpq_benchmarks/matrix/matrix_manifest.json:1】【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-met al.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【xtask/src/fastpq.rs:1】
Las razones de inestabilidad agregada ahora se incluyen junto con la matriz: pasar
`--reason-summary-out` a `scripts/fastpq/geometry_matrix.py` para emitir un
Histograma JSON de causas de falla/advertencia codificadas por etiqueta de host y fuente
resumen, para que los revisores de Stage7-2 puedan ver fallas de CPU o telemetría faltante en
un vistazo sin escanear la tabla Markdown completa. El mismo ayudante ahora
acepta `--host-label chip_family:Chip` (repetir para múltiples claves) por lo que el
Las salidas de Markdown/JSON incluyen columnas de etiquetas de host seleccionadas en lugar de enterrarlas.
esos metadatos en el resumen sin procesar, lo que hace que sea trivial filtrar las compilaciones del sistema operativo o
Versiones del controlador de metal al compilar el paquete de pruebas Stage7-2.【scripts/fastpq/geometry_matrix.py:1】
Los barridos de geometría también estampan los campos ISO8601 `started_at` / `completed_at` en el
resultados de resumen, CSV y Markdown para que los paquetes de captura puedan ser la ventana para
cada host cuando las matrices Stage7-2 fusionan varias ejecuciones de laboratorio.【scripts/fastpq/launch_geometry_sweep.py:1】
`scripts/fastpq/stage7_bundle.py` ahora une la matriz de geometría con
Instantáneas `row_usage/*.json` en un único paquete Stage7 (`stage7_bundle.json`
+ `stage7_geometry.md`), validando ratios de transferencia vía
`validate_row_usage_snapshot.py` y resúmenes persistentes de host/env/reason/source
por lo que los tickets de lanzamiento pueden adjuntar un artefacto determinista en lugar de hacer malabarismos
tablas por host.【scripts/fastpq/stage7_bundle.py:1】【scripts/fastpq/validate_row_usage_snapshot.py:1】
- **Etapa 7-3: evidencia de adopción del operador y simulacros de reversión.** El nuevo
  `docs/source/fastpq_rollout_playbook.md` describe el paquete de artefactos
  (`fastpq_bench_manifest.json`, capturas envueltas de Metal/CUDA, exportación Grafana,
  Instantánea de Alertmanager, registros de reversión) que deben acompañar a cada ticket de implementación
  además del cronograma por etapas (piloto → rampa → predeterminado) y simulacros de retroceso forzado.
  `ci/check_fastpq_rollout.sh` valida estos paquetes para que CI aplique Stage7
  puerta antes de que los lanzamientos avancen. El proceso de lanzamiento ahora puede hacer lo mismo
  paquetes en `artifacts/releases/<version>/fastpq_rollouts/…` a través de
  `scripts/run_release_pipeline.py --fastpq-rollout-bundle <path>`, asegurando la
  los manifiestos firmados y la evidencia de implementación permanecen juntos. Un paquete de referencia vive
  bajo `artifacts/fastpq_rollouts/20250215T101500Z/fleet-alpha/canary/` para mantener
  el flujo de trabajo de GitHub (`.github/workflows/fastpq-rollout.yml`) es verde mientras es real
  Se revisan los envíos de implementación.

### Distribución de la cola FFT de Stage7`crates/fastpq_prover/src/metal.rs` ahora crea una instancia de `QueuePolicy` que
genera automáticamente múltiples colas de comandos de Metal cada vez que el host informa un
GPU discreta. Las GPU integradas mantienen la ruta de cola única
(`MIN_QUEUE_FANOUT = 1`), mientras que los dispositivos discretos tienen por defecto dos colas y solo
desplegar cuando una carga de trabajo cubre al menos 16 columnas. Ambas heurísticas se pueden ajustar
a través de los nuevos `FASTPQ_METAL_QUEUE_FANOUT` y `FASTPQ_METAL_COLUMN_THRESHOLD`
variables de entorno y el programador realiza round-robins por lotes FFT/LDE en todo el
colas activas antes de emitir el envío posterior al mosaico emparejado en la misma cola
para preservar las garantías de pedido.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:772】【crates/fastpq_prover/src/metal.rs:900】
Los operadores de nodos ya no necesitan exportar esas variables de entorno manualmente:
El perfil `iroha_config` expone `fastpq.metal_queue_fanout` y
`fastpq.metal_queue_column_threshold` e `irohad` los aplica a través de
`fastpq_prover::set_metal_queue_policy` antes de que se inicialice el backend de Metal.
Los perfiles de flota siguen siendo reproducibles sin envoltorios de lanzamiento personalizados. 【crates/irohad/src/main.rs:1879】【crates/fastpq_prover/src/lib.rs:60】
Los lotes de FFT inversa ahora se adhieren a una única cola siempre que la carga de trabajo sea apenas
alcanza el umbral de distribución (por ejemplo, la captura equilibrada de carriles de 16 columnas), lo que
restaura ≥1.0× paridad para WP2-D mientras deja FFT/LDE/Poseidón de columna grande
despachos en la ruta de colas múltiples. 【crates/fastpq_prover/src/metal.rs:2018】

Las pruebas auxiliares ejercitan los límites de la política de colas y la validación del analizador para que CI pueda
probar la heurística Stage7 sin requerir hardware GPU en cada constructor,
y las pruebas específicas de GPU fuerzan anulaciones de distribución para mantener la cobertura de repetición en
sincronizar con los nuevos valores predeterminados.【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

### Etapa 7-1 Etiquetas de dispositivo y contrato de alerta

`scripts/fastpq/wrap_benchmark.py` ahora prueba `system_profiler` en la captura de macOS
aloja y registra etiquetas de hardware en cada punto de referencia incluido para que la telemetría de flotas
y la matriz de captura puede girar según el dispositivo sin hojas de cálculo personalizadas. un
La captura de metales de 20000 filas ahora incluye entradas como:

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

Estas etiquetas se ingieren junto con `benchmarks.zero_fill_hotspots` y
`benchmarks.metal_dispatch_queue` entonces la instantánea Grafana, matriz de captura
(`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) y Administrador de alertas
Todas las pruebas coinciden en la clase de hardware que produjo las métricas. el
El indicador `--label` todavía permite anulaciones manuales cuando falta un host de laboratorio
`system_profiler`, pero los identificadores probados automáticamente ahora cubren AppleM1–M4 y
GPU PCIe discretas listas para usar. 【scripts/fastpq/wrap_benchmark.py:1】

Las capturas de Linux reciben el mismo tratamiento: `wrap_benchmark.py` ahora inspecciona
`/proc/cpuinfo`, `nvidia-smi`/`rocm-smi` e `lspci` para que se ejecuten CUDA y OpenCL
derivar `cpu_model`, `gpu_model` y un canónico `device_class` (`xeon-rtx-sm80`
para el host Stage7 CUDA, `neoverse-mi300` para el laboratorio MI300A). Los operadores pueden
aún anula los valores detectados automáticamente, pero los paquetes de pruebas de Stage7 ya no
Requiere ediciones manuales para etiquetar las capturas de Xeon/Neoverse con el dispositivo correcto.
metadatos.En tiempo de ejecución, cada host configura `fastpq.device_class`, `fastpq.chip_family` y
`fastpq.gpu_kind` (o las variables de entorno `FASTPQ_*` correspondientes) al
mismas etiquetas de matriz que aparecen en el paquete de captura para exportar Prometheus
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` y
El panel de Aceleración FASTPQ puede filtrar por cualquiera de los tres ejes. el
Las reglas de Alertmanager se agregan sobre el mismo conjunto de etiquetas, lo que permite a los operadores trazar
adopción, degradaciones y respaldos por perfil de hardware en lugar de un solo
relación de toda la flota.【crates/iroha_config/src/parameters/user.rs:1224】【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】

El contrato de alerta/SLO de telemetría ahora vincula las métricas capturadas con Stage7
puertas. La siguiente tabla resume las señales y los puntos de aplicación:

| Señal | Fuente | Objetivo/Disparador | Aplicación |
| ------ | ------ | ---------------- | ----------- |
| Tasa de adopción de GPU | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | ≥95% de las resoluciones por (clase_dispositivo, familia_chip, tipo_gpu) deben aterrizar en `resolved="gpu", backend="metal"`; página cuando cualquier triplete cae por debajo del 50 % en 15 m | Alerta `FastpqMetalDowngrade` (página)【dashboards/alerts/fastpq_acceleration_rules.yml:1】 |
| Brecha de backend | Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` | Debe permanecer en 0 para cada triplete; advertir después de cualquier ráfaga sostenida (>10 m) | Alerta `FastpqBackendNoneBurst` (advertencia)【dashboards/alerts/fastpq_acceleration_rules.yml:21】 |
| Relación de respaldo de CPU | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…", chip_family="…", gpu_kind="…"}` | ≤5 % de las pruebas solicitadas por la GPU pueden llegar al backend de la CPU para cualquier triplete; página cuando un triplete supera el 5% durante ≥10m | Alerta `FastpqCpuFallbackBurst` (página)【dashboards/alerts/fastpq_acceleration_rules.yml:32】 |
| Ciclo de trabajo de cola de metal | Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` | El promedio móvil de 15 m debe permanecer ≥50 % siempre que los trabajos de GPU estén en cola; advierte cuando la utilización cae por debajo del objetivo mientras persisten las solicitudes de GPU | Alerta `FastpqQueueDutyCycleDrop` (advertencia)【dashboards/alerts/fastpq_acceleration_rules.yml:98】 |
| Profundidad de cola y presupuesto de llenado cero | Bloques de referencia `metal_dispatch_queue` e `zero_fill_hotspots` envueltos | `max_in_flight` debe permanecer al menos una ranura por debajo de `limit` y la media de relleno cero de LDE debe permanecer ≤0,4 ms (≈80 GB/s) para el seguimiento canónico de 20 000 filas; cualquier regresión bloquea el paquete de implementación | Revisado a través de la salida `scripts/fastpq/wrap_benchmark.py` y adjunto al paquete de evidencia Stage7 (`docs/source/fastpq_rollout_playbook.md`). |
| Capacidad de cola en tiempo de ejecución | Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` | `limit - max_in_flight ≥ 1` para cada triplete; avisar después de 10m sin altura libre | Alerta `FastpqQueueHeadroomLow` (advertencia)【dashboards/alerts/fastpq_acceleration_rules.yml:41】 |
| Latencia de relleno cero en tiempo de ejecución | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` | La última muestra de llenado cero debe permanecer ≤0,40 ms (límite de la etapa 7) | Alerta `FastpqZeroFillRegression` (página)【dashboards/alerts/fastpq_acceleration_rules.yml:58】 |El contenedor aplica la fila de relleno cero directamente. Pase
`--require-zero-fill-max-ms 0.40` a `scripts/fastpq/wrap_benchmark.py` y
fallará cuando el banco JSON carezca de telemetría de relleno cero o cuando el más caliente
La muestra de relleno cero excede el presupuesto de Stage7, lo que impide que se implementen paquetes.
envío sin la evidencia requerida.【scripts/fastpq/wrap_benchmark.py:1008】

#### Lista de verificación de manejo de alertas de la etapa 7-1

Cada alerta enumerada anteriormente genera un simulacro de guardia específico para que los operadores recopilen la información necesaria.
Los mismos artefactos que requiere el paquete de lanzamiento:

1. **`FastpqQueueHeadroomLow` (advertencia).** Ejecute una consulta Prometheus instantánea
   para `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` y
   capturar el panel Grafana "Queue headroom" del `fastpq-acceleration`
   tablero. Registre el resultado de la consulta en
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom`
   junto con el ID de alerta para que el paquete de lanzamiento demuestre que la advertencia fue
   reconocido antes de que la cola muera de hambre. 【dashboards/grafana/fastpq_acceleration.json:1】
2. **`FastpqZeroFillRegression` (página).** Inspeccionar
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` y, si la métrica es
   ruidoso, vuelva a ejecutar `scripts/fastpq/wrap_benchmark.py` en el banco JSON más reciente
   para actualizar el bloque `zero_fill_hotspots`. Adjunte la salida de promQL,
   capturas de pantalla y archivo de banco actualizado en el directorio de implementación; esto crea
   la misma evidencia que `ci/check_fastpq_rollout.sh` espera durante el lanzamiento
   validación.【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst` (página).** Confirme que
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` supera el 5%
   piso, luego muestree los registros `irohad` para los mensajes de degradación correspondientes
   (`telemetry::fastpq.execution_mode resolved="cpu"`). Almacenar el volcado de promQL
   además de extractos de registros en `metrics_cpu_fallback.prom`/`rollback_drill.log` para que el
   El paquete demuestra tanto el impacto como el reconocimiento del operador.
4. **Empaquetado de evidencia.** Después de que desaparezca cualquier alerta, vuelva a ejecutar los pasos de la Etapa 7-3 en
   el libro de estrategias de implementación (exportación Grafana, instantánea de alerta, exploración de reversión) y
   revalidar el paquete a través de `ci/check_fastpq_rollout.sh` antes de volver a adjuntarlo
   al ticket de lanzamiento.【docs/source/fastpq_rollout_playbook.md:114】

Los operadores que prefieran la automatización pueden ejecutar
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
para consultar la API Prometheus para el espacio libre de la cola, el llenado cero y el respaldo de la CPU
métricas enumeradas anteriormente; el ayudante escribe el JSON capturado (con el prefijo
promQL original) en `metrics_headroom.prom`, `metrics_zero_fill.prom` y
`metrics_cpu_fallback.prom` en el directorio de implementación elegido para que esos archivos
se puede adjuntar al paquete sin invocaciones manuales de curl.`ci/check_fastpq_rollout.sh` ahora aplica el espacio libre en la cola y el llenado cero
presupuesto directamente. Analiza cada banco `metal` al que hace referencia
`fastpq_bench_manifest.json`, inspecciona
`benchmarks.metal_dispatch_queue.{limit,max_in_flight}` y
`benchmarks.zero_fill_hotspots[]` y falla el paquete cuando cae el espacio libre
debajo de una ranura o cuando cualquier punto de acceso LDE informa `mean_ms > 0.40`. Esto mantiene el
Guardia de telemetría Stage7 en CI, coincidiendo con la revisión manual realizada en el
Instantánea de Grafana y evidencia de publicación. 【ci/check_fastpq_rollout.sh#L1】
Como parte de la misma pasada de validación, el script ahora insiste en que cada paquete envuelto
El punto de referencia lleva las etiquetas de hardware detectadas automáticamente (`metadata.labels.device_class`
y `metadata.labels.gpu_kind`). Los paquetes a los que les faltan esas etiquetas fallan inmediatamente,
garantizando que los artefactos de lanzamiento, los manifiestos de matriz Stage7-2 y el tiempo de ejecución
Todos los paneles se refieren exactamente a los mismos nombres de clase de dispositivo.

El panel Grafana “Último punto de referencia” y el paquete de implementación asociado ahora citan el
`device_class`, presupuesto de llenado cero e instantánea de la profundidad de la cola para que los ingenieros estén de guardia
puede correlacionar la telemetría de producción con la clase de captura exacta utilizada durante la señal
apagado. Las entradas futuras de la matriz heredan las mismas etiquetas, es decir, el dispositivo Stage7-2.
Las listas y los paneles Prometheus comparten un único esquema de nomenclatura para AppleM4,
M3 Max y próximas capturas de MI300/RTX.

### Runbook de telemetría de flotas Stage7-1

Siga esta lista de verificación antes de habilitar los carriles GPU de forma predeterminada para que la telemetría de la flota
y las reglas de Alertmanager reflejan la misma evidencia capturada durante la preparación de la versión:

1. **Captura de etiquetas y hosts de tiempo de ejecución.** `python3 scripts/fastpq/wrap_benchmark.py`
   ya emite `metadata.labels.device_class`, `chip_family` e `gpu_kind`
   para cada JSON envuelto. Mantenga esas etiquetas sincronizadas con
   `fastpq.{device_class,chip_family,gpu_kind}` (o el
   `FASTPQ_{DEVICE_CLASS,CHIP_FAMILY,GPU_KIND}` variables ambientales) dentro de `iroha_config`
   para que se publiquen las métricas de tiempo de ejecución
   `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
   y los medidores `fastpq_metal_queue_*` con los mismos identificadores que aparecen
   en `artifacts/fastpq_benchmarks/matrix/devices/*.txt`. Al montar una nueva
   clase, regenerar el manifiesto de matriz a través de
   `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices`
   para que CI y los paneles comprendan la etiqueta adicional.
2. **Verifique los indicadores de cola y las métricas de adopción.** Ejecute `irohad --features fastpq-gpu`
   en los hosts de Metal y extraiga el punto final de telemetría para confirmar la cola en vivo
   Los medidores están exportando:

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```El primer comando prueba que el muestreador de semáforo está emitiendo el `busy`,
   series `overlap`, `limit` e `max_in_flight` y el segundo muestra si
   cada clase de dispositivo se resuelve en `backend="metal"` o vuelve a caer en
   `backend="cpu"`. Conecte el objetivo de raspado a través de Prometheus/OTel antes
   importando el tablero para que Grafana pueda trazar la vista de la flota inmediatamente.
3. **Instala el panel + paquete de alertas.** Importar
   `dashboards/grafana/fastpq_acceleration.json` en Grafana (conserve el
   variables de plantilla integradas de clase de dispositivo, familia de chips y tipo de GPU) y cargar
   `dashboards/alerts/fastpq_acceleration_rules.yml` en Alertmanager juntos
   con su dispositivo de prueba unitario. El paquete de reglas incluye un arnés `promtool`; correr
   `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`
   cada vez que las reglas cambian para probar `FastpqMetalDowngrade` y
   `FastpqBackendNoneBurst` todavía dispara en los umbrales documentados.
4. **La puerta se libera con el paquete de evidencia.** Mantener
   `docs/source/fastpq_rollout_playbook.md` es útil al generar una implementación
   envío para que cada paquete lleve los puntos de referencia empaquetados, exportación Grafana,
   paquete de alertas, prueba de telemetría de colas y registros de reversión. CI ya hace cumplir la
   contrato: `make check-fastpq-rollout` (o invocando
   `ci/check_fastpq_rollout.sh --bundle <path>`) valida el paquete, lo vuelve a ejecutar
   las pruebas de alerta y se niega a cerrar sesión cuando el margen de cola o el llenado cero
   los presupuestos retroceden.
5. **Vincula las alertas con la corrección.** Cuando Alertmanager busque en páginas, utilice Grafana
   placa y los contadores Prometheus sin procesar del paso 2 para confirmar si
   las degradaciones se deben a falta de cola, retrocesos de CPU o ráfagas de backend=none.
El runbook vive en
este documento más `docs/source/fastpq_rollout_playbook.md`; actualizar el
ticket de liberación con el correspondiente `fastpq_execution_mode_total`,
Extractos de `fastpq_metal_queue_ratio` e `fastpq_metal_queue_depth` juntos
con enlaces al panel Grafana y la instantánea de alerta para que los revisores puedan ver
exactamente qué SLO activó.

### WP2-E — Instantánea del perfilado metálico etapa por etapa

`scripts/fastpq/src/bin/metal_profile.rs` resume las capturas de Metal envueltas
para que el objetivo por debajo de 900 ms se pueda rastrear a lo largo del tiempo (ejecute
`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin metal_profile -- <capture.json>`).
El nuevo ayudante de Markdown
`scripts/fastpq/metal_capture_summary.py fastpq_metal_bench_20k_latest.json --label "20k snapshot (pre-override)"`
genera las tablas de escenario a continuación (imprime el Markdown junto con un texto
resumen para que los tickets del WP2-E puedan incorporar la evidencia palabra por palabra). Se rastrean dos capturas
ahora mismo:

> **Nueva instrumentación WP2-E:** `fastpq_metal_bench --gpu-probe ...` ahora emite un
> instantánea de detección (modo de ejecución solicitado/resuelto, `FASTPQ_GPU`
> anulaciones, backend detectado y los dispositivos metálicos/ID de registro enumerados)
> antes de que se ejecute cualquier kernel. Capture este registro cada vez que una GPU forzada aún se ejecute
> recurre a la ruta de la CPU para que el paquete de evidencia registre qué hosts ven
> `MTLCopyAllDevices` devuelve cero y qué anulaciones estuvieron vigentes durante el
> punto de referencia.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:603】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2616】> **Ayudante de captura de escenario:** `cargo xtask fastpq-stage-profile --trace --out-dir artifacts/fastpq_stage_profiles/<label>`
> ahora controla `fastpq_metal_bench` para FFT, LDE y Poseidon individualmente,
> almacena las salidas JSON sin procesar en directorios por etapa y emite un único
> Paquete `stage_profile_summary.json` que registra tiempos de CPU/GPU, profundidad de cola
> telemetría, estadísticas de preparación de columnas, perfiles del kernel y el seguimiento asociado
> artefactos. Pase `--stage fft --stage lde --stage poseidon` para apuntar a un subconjunto,
> `--trace-template "Metal System Trace"` para elegir una plantilla xctrace específica,
> e `--trace-dir` para enrutar paquetes `.trace` a una ubicación compartida. Adjunte el
> resumen JSON más los archivos de seguimiento generados para cada problema de WP2-E para que los revisores
> puede diferenciar la ocupación de la cola (`metal_dispatch_queue.*`), las proporciones de superposición y la
> Geometría de lanzamiento capturada en todas las carreras sin necesidad de espeleología múltiple
> Invocaciones `fastpq_metal_bench`.【xtask/src/fastpq.rs:721】【xtask/src/main.rs:3187】

> **Ayudante de cola/evidencia en preparación (2026-05-09):** `scripts/fastpq/profile_queue.py` ahora
> ingiere uno o más `fastpq_metal_bench` JSON, captura y emite una tabla Markdown y
> un resumen legible por máquina (`--markdown-out/--json-out`) para que la profundidad de la cola, las proporciones de superposición y
> La telemetría de preparación del lado del host puede acompañar a cada artefacto WP2-E. corriendo
> `python3 scripts/fastpq/profile_queue.py fastpq_metal_bench_poseidon.json fastpq_metal_bench_20k_new.json --json-out artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.json` produjo la siguiente tabla y marcó que las capturas de Metal archivadas aún informan
> `dispatch_count = 0` e `column_staging.batches = 0`—WP2-E.1 permanece abierto hasta que Metal
> la instrumentación se reconstruye con la telemetría habilitada. Los artefactos JSON/Markdown generados están activos.
> bajo `artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.{json,md}` para auditoría.
> El ayudante ahora (2026-05-19) también muestra la telemetría del oleoducto Poseidón (`pipe_depth`,
> `batches`, `chunk_columns` e `fallbacks`) dentro de la tabla Markdown y del resumen JSON,
> para que los revisores de WP2-E.4/6 puedan demostrar si la GPU permaneció en la ruta canalizada y si hubo alguna
> se produjeron retrocesos sin abrir la captura sin formato.【scripts/fastpq/profile_queue.py:1】> **Resumen de perfil de etapa (2026-05-30):** `scripts/fastpq/stage_profile_report.py` consume
> el paquete `stage_profile_summary.json` emitido por `cargo xtask fastpq-stage-profile` y
> presenta resúmenes de Markdown y JSON para que los revisores de WP2-E puedan copiar evidencia en tickets
> sin transcribir manualmente los tiempos. Invocar
> `python3 scripts/fastpq/stage_profile_report.py artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.json --label "m3-lab" --markdown-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.md --json-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.jsonl`
> producir tablas deterministas que enumeren las medias de GPU/CPU, deltas de velocidad, cobertura de seguimiento y
> brechas de telemetría por etapa. La salida JSON refleja la tabla y registra las etiquetas de problemas por etapa.
> (`trace missing`, `queue telemetry missing`, etc.) para que la automatización de la gobernanza pueda diferenciar el host
> ejecuciones a las que se hace referencia en WP2-E.1 a WP2-E.6.
> **Protección de superposición de host/dispositivo (2026-06-04):** `scripts/fastpq/profile_queue.py` ahora anota
> Proporciones de espera FFT/LDE/Poseidón junto con los totales de milisegundos de aplanamiento/espera por etapa y emite un
> problema cada vez que `--max-wait-ratio <threshold>` detecta una superposición deficiente. uso
> `python3 scripts/fastpq/profile_queue.py --max-wait-ratio 0.20 fastpq_metal_bench_20k_latest.json --markdown-out artifacts/fastpq_benchmarks/<stamp>/queue.md`
> para capturar tanto la tabla Markdown como el paquete JSON con índices de espera explícitos para que los tickets WP2-E.5
> puede mostrar si la ventana de doble buffer mantuvo alimentada la GPU. La salida de la consola en texto plano también
> enumera las proporciones por fase para facilitar las investigaciones de guardia.
> **Protección de telemetría + estado de ejecución (2026-06-09):** `fastpq_metal_bench` ahora emite un bloque `run_status`
> (etiqueta de backend, recuento de envío, motivos) y el nuevo indicador `--require-telemetry` falla en la ejecución
> cuando faltan tiempos de GPU o telemetría de cola/preparación. `profile_queue.py` representa la ejecución
> estado como una columna dedicada y muestra estados que no son `ok` en la lista de problemas, y
> `launch_geometry_sweep.py` enhebra el mismo estado en advertencias/clasificación para que las matrices no puedan
> ya no admite capturas que recurrieron silenciosamente a la CPU o se saltaron la instrumentación de la cola.
> **Ajuste automático Poseidon/LDE (2026-06-12):** `metal_config::poseidon_batch_multiplier()` ahora escala
> con las sugerencias del conjunto de trabajo de metal e `lde_tile_stage_target()` aumenta la profundidad del mosaico en GPU discretas.
> El multiplicador aplicado y el límite de mosaicos se incluyen en el bloque `metal_heuristics` de
> Salidas `fastpq_metal_bench` y renderizadas por `scripts/fastpq/metal_capture_summary.py`, por lo que WP2-E
> los paquetes registran las perillas de canalización exactas utilizadas en cada captura sin tener que buscar en JSON sin procesar.

| Etiqueta | Despacho | Ocupado | Superposición | Profundidad máxima | FFT aplanar | Espera FFT | % de espera FFT | LDE aplanar | espera LDE | % de espera LDE | Poseidón aplana | Poseidón espera | Poseidón espera % | Profundidad de la tubería | Lotes de tuberías | Respaldos de tuberías |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fastpq_metal_bench_poseidon | 0 | 0,0% | 0,0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |
| fastpq_metal_bench_20k_new | 0 | 0,0% | 0,0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |

#### Instantánea de 20k (anulación previa)

`fastpq_metal_bench_20k_latest.json`| Etapa | Columnas | Longitud de entrada | Media de GPU (ms) | Media de CPU (ms) | Compartir GPU | Aceleración | ΔCPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 130,986 ms (115,761–167,755) | 112,616 ms (95,335–132,929) | 2,4% | 0,860 × | −18,370 |
| IFFT | 16 | 32768 | 129,296 ms (111,127–142,955) | 158,144 ms (126,847–237,887) | 2,4% | 1.223× | +28.848 |
| LDE | 16 | 262144 | 1570,656 ms (1544,397–1584,502) | 1752,523 ms (1548,807–2191,930) | 29,2% | 1.116× | +181.867 |
| Poseidón | 16 | 524288 | 3548,329 ms (3519,881–3576,041) | 3642,706 ms (3539,055–3758,279) | 66,0% | 1.027× | +94.377 |

Observaciones clave:1. El total de GPU es 5,379 s, lo que supone **4,48 s más** que el objetivo de 900 ms. Poseidón
   El hash todavía domina el tiempo de ejecución (≈66%) con el kernel LDE en segundo lugar.
   lugar (≈29%), por lo que WP2-E necesita atacar tanto la profundidad del oleoducto Poseidon como
   el plan de residencia/ordenamiento en mosaico de la memoria LDE antes de que desaparezcan las reservas de la CPU.
2. La FFT sigue siendo una regresión (0,86×) aunque la IFFT es >1,22× por encima del escalar
   camino. Necesitamos un barrido de geometría de lanzamiento.
   (`FASTPQ_METAL_{FFT,LDE}_COLUMNS` + `FASTPQ_METAL_QUEUE_FANOUT`) para entender
   si la ocupación de FFT se puede salvar sin perjudicar a los que ya son mejores
   Horarios IFFT. El asistente `scripts/fastpq/launch_geometry_sweep.py` ahora conduce
   estos experimentos de un extremo a otro: pase anulaciones separadas por comas (por ejemplo,
   `--fft-columns 16,32 --queue-fanout 1,2` y
   `--poseidon-lanes auto,256`) e invocará
   `fastpq_metal_bench` para cada combinación, almacene las cargas útiles JSON en
   `artifacts/fastpq_geometry/<timestamp>/` y conservar un paquete `summary.json`
   describir las proporciones de cola de cada ejecución, las selecciones de lanzamiento de FFT/LDE, los tiempos de GPU frente a CPU,
   y los metadatos del host (nombre de host/etiqueta, plataforma triple, dispositivo detectado
   clase, proveedor/modelo de GPU), por lo que las comparaciones entre dispositivos tienen efectos deterministas.
   procedencia. El ayudante ahora también escribe `reason_summary.json` al lado del
   resumen de forma predeterminada, utilizando el mismo clasificador que la matriz de geometría para rodar
   upbacks de CPU y falta de telemetría. Utilice `--host-label staging-m3` para etiquetar
   capturas de laboratorios compartidos.
   La herramienta complementaria `scripts/fastpq/geometry_matrix.py` ahora ingiere uno o
   más paquetes resumidos (`--summary hostA/summary.json --summary hostB/summary.json`)
   y emite tablas Markdown/JSON que etiquetan cada forma de lanzamiento como *estable*
   (tiempos de GPU FFT/LDE/Poseidon capturados) o *inestable* (tiempo de espera, respaldo de CPU,
   backend no metálico o telemetría faltante) junto a las columnas del host. el
   Las tablas ahora incluyen el `execution_mode`/`gpu_backend` resuelto más un
   columna `Reason` para que los retrocesos de la CPU y los tiempos faltantes de la GPU sean obvios en
   Matrices Stage7 incluso cuando hay bloques de tiempo presentes; una línea de resumen cuenta
   las carreras estables vs totales. Pase `--operation fft|lde|poseidon_hash_columns`
   cuando el barrido necesita aislar una sola etapa (por ejemplo, para perfilar
   Poseidon por separado) y mantenga `--extra-args` libre para banderas específicas del banco.
   El ayudante acepta cualquier
   prefijo de comando (por defecto `cargo run … fastpq_metal_bench`) más opcional
   `--halt-on-error` / `--timeout-seconds` protegen para que los ingenieros de rendimiento puedan
   reproducir el barrido en diferentes máquinas mientras recopila datos comparables,
   Paquetes de pruebas multidispositivo para Stage7.
3. `metal_dispatch_queue` informó `dispatch_count = 0`, por lo que la ocupación de la cola
   Faltaba la telemetría a pesar de que se ejecutaban los núcleos de GPU. El tiempo de ejecución de Metal ahora usa
   adquirir/liberar barreras para la preparación de cola/columna se alterna para que los subprocesos de trabajo
   observe las banderas de instrumentación y el informe de la matriz de geometría indica
   Formas de lanzamiento inestables cuando los tiempos de GPU FFT/LDE/Poseidon están ausentes. mantener
   adjuntar la matriz Markdown/JSON a los tickets WP2-E para que los revisores puedan ver
   qué combinaciones siguen fallando una vez que la telemetría de cola esté disponible.El guardia `run_status` y el indicador `--require-telemetry` ahora fallan en la captura
   siempre que falten tiempos de GPU o no exista telemetría de cola/preparación, por lo que
   despacho_count=0 las ejecuciones ya no pueden pasar desapercibidas en los paquetes de WP2-E.
   `fastpq_metal_bench` ahora expone `--require-gpu` y
   `launch_geometry_sweep.py` lo habilita de forma predeterminada (optar por no participar con
   `--allow-cpu-fallback`) para que se cancelen los fallos de CPU y de detección de metales.
   inmediatamente en lugar de contaminar las matrices Stage7 con telemetría sin GPU.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs】【scripts/fastpq/launch_geometry_sweep.py】
4. Las métricas de relleno cero desaparecieron anteriormente por la misma razón; el arreglo de esgrima
   mantiene activa la instrumentación del host, por lo que la próxima captura debe incluir la
   Bloque `zero_fill` sin sincronizaciones sintéticas.

#### Instantánea de 20k con `FASTPQ_GPU=gpu`

`fastpq_metal_bench_20k_refresh.json`

| Etapa | Columnas | Longitud de entrada | Media de GPU (ms) | Media de CPU (ms) | Compartir GPU | Aceleración | ΔCPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 79,951 ms (65,645–93,193) | 83,289 ms (59,956–107,585) | 0,3% | 1.042× | +3.338 |
| IFFT | 16 | 32768 | 78,605 ms (69,986–83,726) | 93,898 ms (80,656–119,625) | 0,3% | 1.195× | +15.293 |
| LDE | 16 | 262144 | 657,673 ms (619,219–712,367) | 669,537 ms (619,716–723,285) | 2,1% | 1.018× | +11.864 |
| Poseidón | 16 | 524288 | 30004,898 ms (27284,117–32945,253) | 29087,532 ms (24969,810–33020,517) | 97,4% | 0,969 × | −917,366 |

Observaciones:

1. Incluso con `FASTPQ_GPU=gpu`, esta captura aún refleja el respaldo de la CPU:
   ~30 s por iteración con `metal_dispatch_queue` atascado en cero. cuando el
   La anulación está configurada pero el host no puede descubrir un dispositivo Metal, la CLI ahora sale
   antes de ejecutar cualquier kernel e imprime el modo solicitado/resuelto más el
   etiqueta de backend para que los ingenieros puedan saber si la detección, los derechos o el
   La búsqueda de metallib provocó la degradación. Ejecute `fastpq_metal_bench --gpu-probe
   --rows …` with `FASTPQ_DEBUG_METAL_ENUM=1` para capturar el registro de enumeración y
   solucione el problema de detección subyacente antes de volver a ejecutar el generador de perfiles. 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2636】
2. La telemetría de llenado cero ahora registra una muestra real (18,66 ms en 32 MiB), lo que demuestra
   la solución de valla funciona, pero los deltas de cola permanecen ausentes hasta que se envía la GPU
   tener éxito.
3. Debido a que el backend sigue degradándose, la puerta de telemetría Stage7 todavía está
   bloqueado: la evidencia del margen de cola y la superposición de Poseidón requieren una GPU genuina
   correr.

Estas capturas ahora anclan el trabajo pendiente de WP2-E. Próximas acciones: recopilar perfilador
diagramas de llamas y registros de cola (una vez que el backend se ejecuta en la GPU), apunte al
Cuellos de botella de Poseidon/LDE antes de volver a visitar FFT y desbloquear el respaldo del backend
entonces la telemetría Stage7 tiene datos reales de la GPU.

### Fortalezas
- Puesta en escena incremental, diseño de seguimiento primero, pila STARK transparente.### Elementos de acción de alta prioridad
1. Implementar accesorios de embalaje/orden y actualizar las especificaciones de AIR.
2. Finalice la confirmación de Poseidon2 `3f2b7fe` y publique vectores de búsqueda/SMT de ejemplo.
3. Mantenga los ejemplos trabajados (`lookup_grand_product.md`, `smt_update.md`) junto con los accesorios.
4. Agregue el Apéndice A que documente la derivación de solidez y la metodología de rechazo de CI.

### Decisiones de diseño resueltas
- ZK desactivado (sólo corrección) en P1; volver a visitarlo en una etapa futura.
- Raíz de la tabla de permisos derivada del estado de gobernanza; los lotes tratan la tabla como de solo lectura y prueban la membresía mediante búsqueda.
- En ausencia de pruebas clave se utiliza hoja cero más testigo vecino con codificación canónica.
- Eliminar semántica = valor de hoja establecido en cero dentro del espacio de claves canónico.

Utilice este documento como referencia canónica; actualícelo junto con el código fuente, accesorios y apéndices para evitar desviaciones.

## Apéndice A — Derivación de solidez

Este apéndice explica cómo se produce la tabla "Solidez y SLO" y cómo CI aplica el mínimo de ≥128 bits mencionado anteriormente.

### Notación
- `N_trace = 2^k`: longitud del rastro después de ordenar y rellenar a una potencia de dos.
- `b` — factor de explosión (`N_eval = N_trace × b`).
- `r` — FRI aridad (8 o 16 para los conjuntos canónicos).
- `ℓ`: número de reducciones del FRI (columna `layers`).
- `q`: consultas del verificador por prueba (columna `queries`).
- `ρ`: tasa de código efectiva informada por el planificador de columnas: `ρ = max_i(degree_i / domain_i)` sobre las restricciones que sobreviven a la primera ronda FRI.

El campo base Ricitos de Oro tiene `|F| = 2^64 - 2^32 + 1`, por lo que las colisiones Fiat-Shamir están limitadas por `q / 2^64`. La molienda agrega un factor ortogonal `2^{-g}`, con `g = 23` para `fastpq-lane-balanced` e `g = 21` para el perfil de latencia.【crates/fastpq_isi/src/params.rs:65】

### Encuadernado analítico

Con DEEP-FRI de tasa constante, la probabilidad estadística de falla satisface

```
p_fri ≤ Σ_{j=0}^{ℓ-1} ρ^{q} = ℓ · ρ^{q}
```

porque cada capa reduce el grado del polinomio y el ancho del dominio en el mismo factor `r`, manteniendo `ρ` constante. La columna `est bits` de la tabla informa `⌊-log₂ p_fri⌋`; Fiat-Shamir y el rectificado sirven como margen de seguridad adicional.

### Salida del planificador y cálculo trabajado

La ejecución del planificador de columnas Stage1 en lotes representativos produce:

| Conjunto de parámetros | `N_trace` | `b` | `N_eval` | `ρ` (planificador) | Grado efectivo (`ρ × N_eval`) | `ℓ` | `q` | `-log₂(ℓ · ρ^{q})` |
| ------------- | --------- | --- | -------- | ------------- | -------------------------------- | --- | --- | ------------------ |
| Lote equilibrado de 20k | `2^15` | 8 | 262144 | 0,077026 | 20192 | 5 | 52 | 190 bits |
| Rendimiento por lotes de 65k | `2^16` | 8 | 524288 | 0,200208 | 104967 | 6 | 58 | 132 bits |
| Latencia 131k por lotes | `2^17` | 16 | 2097152 | 0,209492 | 439337 | 5 | 64 | 142 bits |Ejemplo (lote equilibrado de 20k):
1. `N_trace = 2^15`, entonces `N_eval = 2^15 × 8 = 2^18`.
2. La instrumentación del planificador informa `ρ = 0.077026`, es decir, `p_fri = 5 × ρ^{52} ≈ 6.4 × 10^{-58}`.
3. `-log₂ p_fri = 190 bits`, que coincide con la entrada de la tabla.
4. Las colisiones Fiat-Shamir suman como máximo `2^{-58.3}`, y el pulido (`g = 23`) resta otro `2^{-23}`, manteniendo la solidez total cómodamente por encima de los 160 bits.

### Arnés de muestreo de rechazo de CI

Cada ejecución de CI ejecuta un arnés Monte Carlo para garantizar que las mediciones empíricas se mantengan dentro de ±0,6 bits del límite analítico:
1. Dibuje un conjunto de parámetros canónicos y sintetice un `TransitionBatch` con el recuento de filas correspondiente.
2. Construya la traza, invierta una restricción elegida al azar (por ejemplo, perturbe el gran producto de búsqueda o un hermano SMT) e intente producir una prueba.
3. Vuelva a ejecutar el verificador, volviendo a muestrear las pruebas Fiat-Shamir (pulido incluido) y registre si se rechaza la prueba de manipulación.
4. Repita el proceso para 16384 semillas por conjunto de parámetros y convierta el límite inferior de Clopper-Pearson del 99 % de la tasa de rechazo observada en bits.

El trabajo falla inmediatamente si el límite inferior medido cae por debajo de 128 bits, por lo que las regresiones en el planificador, el bucle de plegado o el cableado de transcripción se detectan antes de la fusión.

## Apéndice B: Derivación de la raíz del dominio

Stage0 fija los generadores de seguimiento y evaluación a constantes derivadas de Poseidon para que todas las implementaciones compartan los mismos subgrupos.

### Procedimiento
1. **Selección de semillas.** Absorba la etiqueta UTF-8 `fastpq:v1:domain_roots` en la esponja Poseidon2 utilizada en otras partes de FASTPQ (ancho de estado = 3, tasa = 2, cuatro rondas completas + 57 parciales). Las entradas reutilizan la codificación `[len, limbs…]` de `pack_bytes`, produciendo el generador base `g_base = 7`.【crates/fastpq_prover/src/packing.rs:44】【scripts/fastpq/src/bin/poseidon_gen.rs:1】
2. **Generador de seguimiento.** Calcule `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` y verifique `trace_root^{2^{trace_log_size}} = 1` mientras la potencia media no sea 1.
3. **Generador LDE.** Repita la misma exponenciación con `lde_log_size` para derivar `lde_root`.
4. **Selección de clase lateral.** Stage0 utiliza el subgrupo base (`omega_coset = 1`). Las clases laterales futuras pueden absorber una etiqueta adicional como `fastpq:v1:domain_roots:coset`.
5. **Tamaño de permutación.** Persista `permutation_size` explícitamente para que los programadores nunca infieran reglas de relleno a partir de potencias de dos implícitas.

### Reproducción y validación
- Herramientas: `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots` emite fragmentos de Rust o una tabla Markdown (ver `--format table`, `--seed`, `--filter`).【scripts/fastpq/src/bin/poseidon_gen.rs:1】
- Pruebas: `canonical_sets_meet_security_target` mantiene los conjuntos de parámetros canónicos alineados con las constantes publicadas (raíces distintas de cero, emparejamiento de explosión/aridad, tamaño de permutación), por lo que `cargo test -p fastpq_isi` detecta la deriva inmediatamente.【crates/fastpq_isi/src/params.rs:138】
- Fuente de la verdad: actualice la tabla Stage0 e `fastpq_isi/src/params.rs` juntas cada vez que se introduzcan nuevos paquetes de parámetros.

## Apéndice C: Detalles del proceso de compromiso### Transmisión del flujo de compromiso de Poseidón
Stage2 define el compromiso de seguimiento determinista compartido por el probador y el verificador:
1. **Normalizar transiciones.** `trace::build_trace` ordena cada lote, lo rellena con `N_trace = 2^{⌈log₂ rows⌉}` y emite vectores de columnas en el orden siguiente.【crates/fastpq_prover/src/trace.rs:123】
2. **Columnas hash.** `trace::column_hashes` transmite las columnas a través de esponjas Poseidon2 dedicadas etiquetadas como `fastpq:v1:trace:column:<name>`. Cuando la función `fastpq-prover-preview` está activa, el mismo recorrido recicla los coeficientes IFFT/LDE requeridos por el backend, por lo que no se asignan copias de matriz adicionales. 【crates/fastpq_prover/src/trace.rs:474】
3. **Levante a un árbol Merkle.** `trace::merkle_root` pliega los resúmenes de columnas con nodos de Poseidón etiquetados como `fastpq:v1:trace:node`, duplicando la última hoja cada vez que un nivel tiene una distribución impar para evitar casos especiales.【crates/fastpq_prover/src/trace.rs:656】
4. **Finalice el resumen.** `digest::trace_commitment` antepone la etiqueta de dominio (`fastpq:v1:trace_commitment`), el nombre del parámetro, las dimensiones rellenadas, los resúmenes de columnas y la raíz de Merkle utilizando la misma codificación `[len, limbs…]`, luego aplica hash a la carga útil con SHA3-256 antes de incrustarla en `Proof::trace_commitment`.【crates/fastpq_prover/src/digest.rs:25】

El verificador vuelve a calcular el mismo resumen antes de probar los desafíos Fiat-Shamir, por lo que las discrepancias abortan las pruebas antes de cualquier apertura.

### Controles alternativos de Poseidón- El probador ahora expone una anulación de canalización de Poseidon dedicada (`zk.fastpq.poseidon_mode`, env `FASTPQ_POSEIDON_MODE`, CLI `--fastpq-poseidon-mode`) para que los operadores puedan mezclar GPU FFT/LDE con hash de CPU Poseidon en dispositivos que no logran alcanzar el objetivo de Stage7 <900 ms. Los valores admitidos reflejan la perilla del modo de ejecución (`auto`, `cpu`, `gpu`), y el valor predeterminado es el modo global cuando no se especifica. El tiempo de ejecución pasa este valor a través de la configuración del carril (`FastpqPoseidonMode`) y lo propaga al probador (`Prover::canonical_with_modes`), por lo que las anulaciones son deterministas y auditables en la configuración. volcados.【crates/iroha_config/src/parameters/user.rs:1488】【crates/fastpq_prover/src/proof.rs:138】【crates/iroha_core/src/fastpq/lane.rs:123】
- La telemetría exporta el modo de canalización resuelto a través del nuevo contador `fastpq_poseidon_pipeline_total{requested,resolved,path,device_class,chip_family,gpu_kind}` (y el gemelo OTLP `fastpq.poseidon_pipeline_resolutions_total`). Por lo tanto, los paneles de operador/`sorafs` pueden confirmar cuándo una implementación ejecuta hash fusionado/canalizado de GPU versus el respaldo forzado de CPU (`path="cpu_forced"`) o degradaciones del tiempo de ejecución (`path="cpu_fallback"`). La sonda CLI se instala automáticamente en `irohad`, por lo que los paquetes de lanzamiento y la telemetría en vivo comparten el mismo flujo de evidencia.【crates/iroha_telemetry/src/metrics.rs:4780】【crates/irohad/src/main.rs:2504】
- La evidencia de modo mixto también se imprime en cada marcador a través de la puerta de adopción existente: el probador emite la etiqueta de ruta + modo resuelto para cada lote, y el contador `fastpq_poseidon_pipeline_total` se incrementa junto con el contador del modo de ejecución cada vez que llega una prueba. Esto satisface WP2-E.6 al hacer visibles las caídas de tensión y al proporcionar un cambio limpio para degradaciones deterministas mientras continúa la optimización.
- `scripts/fastpq/wrap_benchmark.py --poseidon-metrics metrics_poseidon.prom` ahora analiza los fragmentos Prometheus (Metal o CUDA) e incorpora un resumen `poseidon_metrics` dentro de cada paquete empaquetado. El asistente filtra las filas del contador por `metadata.labels.device_class`, captura las muestras `fastpq_execution_mode_total` coincidentes y falla el ajuste cuando faltan entradas `fastpq_poseidon_pipeline_total`, por lo que los paquetes WP2-E.6 siempre envían evidencia CUDA/Metal reproducible en lugar de ad-hoc. notas.【scripts/fastpq/wrap_benchmark.py:1】【scripts/fastpq/tests/test_wrap_benchmark.py:1】

#### Política determinista de modo mixto (WP2-E.6)1. **Detectar déficit de GPU.** Marque cualquier clase de dispositivo cuya captura Stage7 o instantánea Grafana en vivo muestre la latencia de Poseidon manteniendo el tiempo total de prueba >900 ms mientras FFT/LDE permanece por debajo del objetivo. Los operadores anotan la matriz de captura (`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) y avisan a la persona de guardia cuando `fastpq_poseidon_pipeline_total{device_class="<label>",path="gpu"}` se estanca mientras que `fastpq_execution_mode_total{backend="metal"}` todavía registra envíos GPU FFT/LDE. 【scripts/fastpq/wrap_benchmark.py:1】 【dashboards/grafana/fastpq_acceleration.json:1】
2. **Cambie a CPU Poseidon solo para los hosts afectados.** Configure `zk.fastpq.poseidon_mode = "cpu"` (o `FASTPQ_POSEIDON_MODE=cpu`) en la configuración local del host junto con las etiquetas de la flota, manteniendo `zk.fastpq.execution_mode = "gpu"` para que FFT/LDE continúe usando el acelerador. Registre la diferencia de configuración en el ticket de implementación y agregue la anulación por host al paquete como `poseidon_fallback.patch` para que los revisores puedan reproducir el cambio de manera determinista.
3. **Demuestre la degradación.** Raspe el contador de Poseidón inmediatamente después de reiniciar el nodo:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"'
   ```
   El volcado debe mostrar que `path="cpu_forced"` crece al mismo ritmo que el contador de ejecución de la GPU. Guarde el borrador como `metrics_poseidon.prom` junto a la instantánea `metrics_cpu_fallback.prom` existente y capture las líneas de registro `telemetry::fastpq.poseidon` coincidentes en `poseidon_fallback.log`.
4. **Monitorear y salir.** Continúe alertando en `fastpq_poseidon_pipeline_total{path="cpu_forced"}` mientras continúa el trabajo de optimización. Una vez que un parche devuelve el tiempo de ejecución por prueba a menos de 900 ms en el host de prueba, revierta la configuración a `auto`, vuelva a ejecutar el raspado (mostrando `path="gpu"` nuevamente) y adjunte las métricas de antes/después al paquete para cerrar la exploración en modo mixto.

**Contrato de Telemetría.**

| Señal | PromQL / Fuente | Propósito |
|--------|-----------------|---------|
| Contador del modo Poseidón | `fastpq_poseidon_pipeline_total{device_class="<label>",path=~"cpu_.*"}` | Confirma que el hash de la CPU es intencional y tiene como alcance la clase de dispositivo marcada. |
| Contador del modo de ejecución | `fastpq_execution_mode_total{device_class="<label>",backend="metal"}` | Demuestra que FFT/LDE aún se ejecuta en GPU incluso cuando Poseidon baja de categoría. |
| Registrar evidencia | Entradas `telemetry::fastpq.poseidon` capturadas en `poseidon_fallback.log` | Proporciona pruebas de que el host resolvió el hash de CPU con el motivo `cpu_forced`. |

El paquete de implementación ahora debe incluir `metrics_poseidon.prom`, la diferencia de configuración y el extracto del registro siempre que el modo mixto esté activo para que la gobernanza pueda auditar la política de reserva determinista junto con la telemetría FFT/LDE. `ci/check_fastpq_rollout.sh` ya aplica los límites de cola/llenado cero; la puerta de seguimiento comprobará la cordura del contador de Poseidón una vez que el modo mixto llegue a la automatización de lanzamiento.

Las herramientas de captura de Stage7 ya manejan CUDA: envuelva cada paquete `fastpq_cuda_bench` con `--poseidon-metrics` (apuntando al `metrics_poseidon.prom` raspado) y la salida ahora lleva los mismos contadores de canalización/resumen de resolución utilizados en Metal para que la gobernanza pueda verificar los respaldos de CUDA sin herramientas personalizadas.【scripts/fastpq/wrap_benchmark.py:1】### Orden de columnas
La canalización de hash consume columnas en este orden determinista:
1. Banderas selectoras: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`.
2. Columnas de ramas empaquetadas (cada una con relleno de ceros hasta la longitud del trazado): `key_limb_{i}`, `value_old_limb_{i}`, `value_new_limb_{i}`, `asset_id_limb_{i}`.
3. Escalares auxiliares: `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter`, `perm_hash`, `neighbour_leaf`, `dsid`, `slot`.
4. Testigos Merkle dispersos para cada nivel `ℓ ∈ [0, SMT_HEIGHT)`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`.

`trace::column_hashes` recorre las columnas exactamente en este orden, por lo que el backend del marcador de posición y la implementación de Stage2 STARK permanecen estables en todas las versiones. 【crates/fastpq_prover/src/trace.rs:474】

### Etiquetas de dominio de transcripción
Stage2 corrige el catálogo Fiat-Shamir a continuación para mantener la generación de desafíos determinista:

| Etiqueta | Propósito |
| --- | ------- |
| `fastpq:v1:init` | Absorba la versión del protocolo, el conjunto de parámetros y `PublicIO`. |
| `fastpq:v1:roots` | Confirme el rastreo y busque las raíces de Merkle. |
| `fastpq:v1:gamma` | Pruebe el desafío de búsqueda de grandes productos. |
| `fastpq:v1:alpha:<i>` | Muestra de desafíos de composición polinomial (`i = 0, 1`). |
| `fastpq:v1:lookup:product` | Absorba el gran producto de búsqueda evaluado. |
| `fastpq:v1:beta:<round>` | Pruebe el desafío de plegado para cada ronda del VIE. |
| `fastpq:v1:fri_layer:<round>` | Confirme la raíz de Merkle para cada capa FRI. |
| `fastpq:v1:fri:final` | Registre la capa FRI final antes de abrir consultas. |
| `fastpq:v1:query_index:0` | Derivar de manera determinista índices de consulta del verificador. |