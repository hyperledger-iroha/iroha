---
lang: es
direction: ltr
source: docs/source/merge_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f1c681730f1c94d9d00e8f829a0134374ce6cb29f21727a27685e096f0da40
source_last_modified: "2026-01-18T05:31:56.955438+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Fusionar diseño de libro mayor: finalidad del carril y reducción global

Esta nota finaliza el diseño del libro mayor de fusión para Milestone 5. Explica el
política de bloques no vacíos, semántica de fusión de control de calidad entre carriles y flujo de trabajo de finalidad
que vincula la ejecución a nivel de carril con el compromiso del estado mundial global.

El diseño amplía la arquitectura Nexus descrita en `nexus.md`. Términos como
"bloque de carril", "control de calidad de carril", "sugerencia de combinación" y "libro mayor de combinación" heredan sus
definición de ese documento; Esta nota se centra en las reglas de comportamiento y
Guía de implementación que debe ser aplicada por el tiempo de ejecución, el almacenamiento y el WSV.
capas.

## 1. Política de bloqueo no vacío

**Regla (DEBE):** Un proponente de carril emite un bloque solo cuando el bloque contiene al menos
al menos un fragmento de transacción ejecutado, disparador basado en tiempo o determinista
actualización de artefactos (por ejemplo, acumulación de artefactos DA). Están prohibidos los bloques vacíos.

**Implicaciones:**

- Slot keep-alive: cuando ninguna transacción alcanza su ventana de confirmación determinista,
el carril no emite ningún bloqueo y simplemente avanza al siguiente espacio. El libro de fusiones
permanece en el consejo anterior para ese carril.
- Disparador por lotes: activadores en segundo plano que no producen transición de estado (p. ej.,
cron que reafirma invariantes) se consideran vacíos y DEBEN omitirse o
junto con otro trabajo antes de producir un bloque.
- Telemetría: `pipeline_detached_merged` y tratamiento de métricas de seguimiento omitidas
ranuras explícitamente: los operadores pueden distinguir "sin trabajo" de "tubería estancada".
- Reproducción: el almacenamiento en bloque no inserta marcadores de posición vacíos sintéticos. la kura
El bucle de reproducción simplemente observa el mismo hash principal para espacios consecutivos si no
Se emitió el bloque.

**Verificación canónica:** Durante la propuesta y validación del bloque, `ValidBlock::commit`
afirma que el `StateBlock` asociado lleva al menos una superposición comprometida
(delta, artefacto, disparador). Esto se alinea con el protector `StateBlock::is_empty`.
eso ya garantiza que se eliminen las escrituras no operativas. La aplicación de la ley ocurre antes
Se solicitan firmas para que los comités nunca voten sobre cargas útiles vacías.

## 2. Semántica de fusión de control de calidad entre carriles

Cada bloque de carril `B_i` finalizado por su comité produce:

- `lane_state_root_i`: Se tocó el compromiso de Poseidon2-SMT sobre las raíces estatales por DS
en el bloque.
- `merge_hint_root_i`: candidato rodante para el libro mayor de fusión (`tag =
"iroha:merge:candidato:v1\0"`).
- `lane_qc_i`: firmas agregadas del comité de carril sobre el
  preimagen de votación de ejecución (hash de bloque, `parent_state_root`,
  `post_state_root`, altura/vista/época, chain_id y etiqueta de modo).

Los nodos de fusión recopilan los últimos consejos `{(B_i, lane_qc_i, merge_hint_root_i)}` para
todos los carriles `i ∈ [0, K)`.

**Fusionar entrada (DEBE):**

```
MergeLedgerEntry {
    epoch_id: u64,
    lane_tips: [Hash32; K],
    merge_hint_root: [Hash32; K],
    global_state_root: Hash32,
    merge_qc: QuorumCertificate,
}
```- `lane_tips[i]` es el hash del bloque de carril que los sellos de entrada de fusión para el carril
  `i`. Si un carril no emitió ningún bloque desde la entrada de fusión anterior, este valor es
  repetido.
- `merge_hint_root[i]` es el `merge_hint_root` del carril correspondiente
  bloque. Se repite cuando se repite `lane_tips[i]`.
- `global_state_root` es igual a `ReduceMergeHints(merge_hint_root[0..K-1])`, un
  Poseidon2 pliegue con etiqueta de separación de dominio
  `"iroha:merge:reduce:v1\0"`. La reducción es determinista y DEBE
  reconstruir el mismo valor entre pares.
- `merge_qc` es un certificado de quórum BFT del comité de fusión sobre el
  entrada serializada.

**Fusionar carga útil de control de calidad (DEBE):**

Los miembros del comité de fusión firman un resumen determinista:

```
merge_qc_digest = blake2b32(
    "iroha:merge:qc:v1\0" ||
    chain_id ||
    norito(MergeLedgerSignPayload {
        view,
        epoch_id,
        lane_tips,
        merge_hint_roots,
        global_state_root,
    })
)
```

- `view` es la vista del comité de fusión derivada de las sugerencias de carril (máx.
  `view_change_index` a través de los encabezados de carril sellados por la entrada).
- `chain_id` es la cadena de identificador de cadena configurada (UTF-8 bytes).
- La carga útil utiliza la codificación Norito con el orden de los campos que se muestra arriba.

El resumen resultante se almacena en `merge_qc.message_digest` y es el mensaje
verificado por firmas BLS.

**Fusionar construcción de control de calidad (DEBE):**

- La lista del comité de fusión es el conjunto actual de validadores de topología de compromiso.
- Quórum requerido = `commit_quorum_from_len(roster_len)`.
- `merge_qc.signers_bitmap` codifica los índices de validación participantes (LSB-first)
  en orden de topología de compromiso.
- `merge_qc.aggregate_signature` es el agregado normal BLS para el resumen
  arriba.

**Validación (DEBE):**

1. Verifique cada `lane_qc_i` con `lane_tips[i]` y confirme los encabezados del bloque.
   Incluya el `merge_hint_root_i` correspondiente.
2. Asegúrese de que ningún `lane_qc_i` apunte a un `Invalid` o a un bloque no ejecutado. el
   La política no vacía anterior garantiza que el encabezado incluya superposiciones de estado.
3. Vuelva a calcular `ReduceMergeHints` y compárelo con `global_state_root`.
4. Vuelva a calcular el resumen de control de calidad de la combinación y verifique el mapa de bits del firmante, el umbral de quórum y
   y firma agregada contra la lista de topología de confirmación.

**Observabilidad:** Los nodos de fusión emiten contadores Prometheus para
`merge_entry_lane_repeats_total{i}` para resaltar los carriles que se saltaron espacios para
visibilidad operativa.

## 3. Flujo de trabajo de finalidad

### 3.1 Finalidad a nivel de carril

1. Las transacciones se programan por carril en espacios deterministas.
2. El ejecutor aplica superposiciones en `StateBlock`, produciendo deltas y
artefactos.
3. Tras la validación, el comité de carril firma la preimagen de ejecución-voto que
   vincula el hash del bloque, las raíces del estado y la altura/vista/época. la tupla
   `(block_hash, lane_qc_i, merge_hint_root_i)` se considera final de carril.
4. Los clientes ligeros PUEDEN tratar la sugerencia de carril como final para las pruebas limitadas por DS, pero
debe registrar el `merge_hint_root` asociado para conciliarlo con el libro mayor de fusión
más tarde.Los comités de carril son por espacio de datos y no reemplazan el compromiso global.
topología. El tamaño del comité se fija en `3f+1`, donde `f` proviene del
catálogo de espacio de datos (`fault_tolerance`). El grupo de validadores es el espacio de datos.
validadores (manifiestos de gobernanza de carriles para carriles administrados por administradores o carriles públicos
registros de apuestas para carriles elegidos por apuestas). La membresía del comité es
muestreado deterministamente una vez por época utilizando la semilla de época VRF unida con
`dataspace_id` y `lane_id`. Si el grupo es más pequeño que `3f+1`, la finalidad del carril
se detiene hasta que se restablece el quórum (la recuperación de emergencia se maneja por separado).

### 3.2 Finalidad del libro mayor de fusión

1. El comité de fusión recopila los últimos consejos sobre carriles, verifica cada `lane_qc_i` y
construye el `MergeLedgerEntry` como se definió anteriormente.
2. Después de verificar la reducción determinista, el comité de fusión firma el
entrada (`merge_qc`).
3. Los nodos agregan la entrada al registro del libro mayor de fusión y la conservan junto con el
referencias de bloque de carril.
4. `global_state_root` se convierte en el compromiso estatal mundial autorizado para el
época/ranura. Los nodos completos actualizan sus metadatos de puntos de control WSV para reflejar esto
valor; la repetición determinista debe reproducir la misma reducción.

### 3.3 Integración de almacenamiento y WSV

- `State::commit_merge_entry` registra las raíces del estado por carril y el
  `global_state_root` final, puente entre la ejecución del carril y la suma de comprobación global.
- Kura persiste `MergeLedgerEntry` adyacente a los artefactos del bloque de carril, por lo que
  La repetición puede reconstruir secuencias de finalidad tanto a nivel de carril como globales.
- Cuando un carril se salta una ranura, el almacenamiento simplemente conserva la punta anterior; no
  Las entradas de combinación de marcadores de posición se crean hasta que al menos un carril produzca un nuevo
  bloque.
- Las superficies API (Torii, telemetría) exponen ambas puntas de carril y la última fusión
  entrada para que los operadores y clientes puedan conciliar vistas por carril y globales.

## 4. Notas de implementación- `crates/iroha_core/src/state.rs`: `State::commit_merge_entry` valida el
  reducción y conecta los metadatos del carril/global al estado mundial para que las consultas
  y los observadores pueden acceder a las sugerencias de fusión y al hash global autorizado.
- `crates/iroha_core/src/kura.rs`: `Kura::store_block_with_merge_entry` se pone en cola
  el bloque y persiste la entrada de fusión asociada en un solo paso, retrocediendo
  el bloque en memoria cuando falla la adición para que el almacenamiento nunca registre un bloque
  sin sus metadatos de sellado. El registro del libro mayor de fusión se elimina en pasos cerrados
  con la altura del bloque validado durante la recuperación del inicio y almacenado en caché en la memoria
  con una ventana delimitada (`kura.merge_ledger_cache_capacity`, por defecto 256) para
  Evite el crecimiento ilimitado en nodos de larga duración. La recuperación se trunca parcial o
  entradas de cola de gran tamaño del libro mayor de fusión y anexar entradas rechazadas por encima del
  protección del tamaño máximo de carga útil para limitar las asignaciones.
- `crates/iroha_core/src/block.rs`: la validación de bloques rechaza los bloques sin
  puntos de entrada (transacciones externas o desencadenadores de tiempo) y sin deterministas
  artefactos como paquetes DA (`BlockValidationError::EmptyBlock`), asegurando
  la política de no vacío se aplica antes de que se soliciten y lleven las firmas
  en el libro mayor de fusión.
- El ayudante de reducción determinista vive en el servicio de fusión: `reduce_merge_hint_roots`
  (`crates/iroha_core/src/merge.rs`) implementa el pliegue Poseidon2 descrito anteriormente.
  Los ganchos de aceleración de hardware siguen siendo un trabajo futuro, pero la ruta escalar ahora impone
  la reducción canónica de manera determinista.
- Integración de telemetría: exposición de repeticiones de fusión por carril y la
  El medidor `global_state_root` permanece rastreado en el trabajo pendiente de observabilidad, por lo que el
  el trabajo del panel de control puede enviarse junto con la implementación del servicio de fusión.
- Pruebas entre componentes: la cobertura de repetición dorada para la reducción de fusión es
  seguimiento con el trabajo pendiente de las pruebas de integración para garantizar cambios futuros en
  `reduce_merge_hint_roots` mantiene estables las raíces registradas.