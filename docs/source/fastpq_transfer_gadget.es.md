---
lang: es
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T10:01:27.059307+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Diseño de gadgets de transferencia FastPQ

# Descripción general

El planificador FASTPQ actual registra cada operación primitiva involucrada en una instrucción `TransferAsset`, lo que significa que cada transferencia paga por la aritmética de saldo, las rondas hash y las actualizaciones SMT por separado. Para reducir las filas de seguimiento por transferencia, presentamos un dispositivo dedicado que verifica solo las comprobaciones aritméticas/de compromiso mínimas mientras el host continúa ejecutando la transición de estado canónico.

- **Alcance**: transferencias individuales y lotes pequeños emitidos a través de la superficie de llamada al sistema Kotodama/IVM `TransferAsset` existente.
- **Objetivo**: reducir el espacio de las columnas FFT/LDE para transferencias de gran volumen compartiendo tablas de búsqueda y colapsando la aritmética por transferencia en un bloque de restricciones compacto.

# Arquitectura

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## Formato de transcripción

El host emite un `TransferTranscript` por invocación de llamada al sistema:

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` vincula la transcripción al hash del punto de entrada de la transacción para protección de reproducción.
- `authority_digest` es el hash del host sobre los datos de quórum/firmantes ordenados; el gadget comprueba la igualdad pero no rehace la verificación de la firma. Concretamente, el host Norito codifica el `AccountId` (que ya incorpora el controlador multifirma canónico) y codifica el `b"iroha:fastpq:v1:authority|" || encoded_account` con Blake2b-256, almacenando el `Hash` resultante.
- `poseidon_preimage_digest` = Poseidon(cuenta_de || cuenta_a || activo || cantidad || lote_hash); garantiza que el dispositivo vuelva a calcular el mismo resumen que el host. Los bytes de la preimagen se construyen como `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` utilizando la codificación Norito antes de pasarlos a través del asistente compartido Poseidon2. Este resumen está presente para transcripciones de delta simple y se omite para lotes de delta múltiple.

Todos los campos se serializan a través de Norito, por lo que se mantienen las garantías de determinismo existentes.
Tanto `from_path` como `to_path` se emiten como blobs Norito utilizando el
Esquema `TransferMerkleProofV1`: `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`.
Las versiones futuras pueden ampliar el esquema mientras el proveedor aplica la etiqueta de versión.
antes de decodificar. Los metadatos `TransitionBatch` incorporan la transcripción codificada con Norito.
vector bajo la clave `transfer_transcripts` para que el probador pueda decodificar el testigo
sin realizar consultas fuera de banda. Entradas públicas (`dsid`, `slot`, raíces,
`perm_root`, `tx_set_hash`) se transportan en `FastpqTransitionBatch.public_inputs`,
dejando metadatos para la contabilidad del recuento de hash de entrada/transcripción. Hasta plomería anfitriona
tierras, el probador deriva sintéticamente pruebas de los pares clave/equilibrio, de modo que las filas
Incluya siempre una ruta SMT determinista incluso cuando la transcripción omita los campos opcionales.

## Diseño de gadget

1. **Bloque aritmético de equilibrio**
   - Entradas: `from_balance_before`, `amount`, `to_balance_before`.
   - Cheques:
     - `from_balance_before >= amount` (gadget de alcance con descomposición RNS compartida).
     - `from_balance_after = from_balance_before - amount`.
     - `to_balance_after = to_balance_before + amount`.
   - Empaquetado en una puerta personalizada para que las tres ecuaciones consuman un grupo de filas.2. **Bloque de compromiso de Poseidón**
   - Vuelve a calcular `poseidon_preimage_digest` utilizando la tabla de búsqueda compartida de Poseidon que ya se utiliza en otros dispositivos. No hay rondas de Poseidón por transferencia en el rastro.

3. **Bloque de ruta Merkle**
   - Amplía el dispositivo Kaigi SMT existente con un modo de "actualización emparejada". Dos hojas (remitente, receptor) comparten la misma columna para hashes hermanos, lo que reduce las filas duplicadas.

4. **Verificación del resumen de la autoridad**
   - Restricción de igualdad simple entre el resumen proporcionado por el host y el valor testigo. Las firmas permanecen en su dispositivo dedicado.

5. **Bucle por lotes**
   - Los programas llaman a `transfer_v1_batch_begin()` antes de un bucle de constructores `transfer_asset` y a `transfer_v1_batch_end()` después. Mientras el alcance está activo, el host almacena en buffer cada transferencia y las reproduce como un único `TransferAssetBatch`, reutilizando el contexto Poseidon/SMT una vez por lote. Cada delta adicional suma sólo la aritmética y las comprobaciones de dos hojas. El descodificador de transcripciones ahora acepta lotes multi-delta y los muestra como `TransferGadgetInput::deltas` para que el planificador pueda plegar testigos sin volver a leer Norito. Los contratos que ya tienen una carga útil Norito a mano (por ejemplo, CLI/SDK) pueden omitir el alcance por completo llamando a `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)`, que entrega al host un lote completamente codificado en una llamada al sistema.

# Cambios de anfitrión y probador| Capa | Cambios |
|-------|---------|
| `ivm::syscalls` | Agregue `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) para que los programas puedan agrupar múltiples llamadas al sistema `transfer_v1` sin emitir ISI intermedios, además de `transfer_v1_batch_apply` (`0x2B`) para lotes precodificados. |
| `ivm::host` y pruebas | Los hosts principales/predeterminados tratan a `transfer_v1` como un agregado por lotes mientras el alcance está activo, muestran `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` y el host WSV simulado almacena en buffer las entradas antes de confirmar para que las pruebas de regresión puedan afirmar un equilibrio determinista. actualizaciones.【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | Emita `TransferTranscript` después de la transición de estado, cree registros `FastpqTransitionBatch` con `public_inputs` explícito durante `StateBlock::capture_exec_witness` y ejecute el carril de prueba FASTPQ para que tanto las herramientas Torii/CLI como el backend Stage6 reciban información canónica. Entradas `TransitionBatch`. `TransferAssetBatch` agrupa transferencias secuenciales en una sola transcripción, omitiendo el resumen de Poseidón para lotes multi-delta para que el dispositivo pueda iterar entre entradas de manera determinista. |
| `fastpq_prover` | `gadgets::transfer` ahora valida transcripciones multi-delta (equilibrio aritmético + resumen de Poseidón) y muestra testigos estructurados (incluidos blobs SMT emparejados con marcadores de posición) para el planificador (`crates/fastpq_prover/src/gadgets/transfer.rs`). `trace::build_trace` decodifica esas transcripciones a partir de metadatos por lotes, rechaza los lotes de transferencia a los que les falta la carga útil `transfer_transcripts`, adjunta los testigos validados a `Trace::transfer_witnesses` e `TracePolynomialData::transfer_plan()` mantiene vivo el plan agregado hasta que el planificador consume el dispositivo (`crates/fastpq_prover/src/trace.rs`). El arnés de regresión de recuento de filas ahora se envía a través de `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`), que cubre escenarios de hasta 65536 filas acolchadas, mientras que el cableado SMT emparejado permanece detrás del hito del asistente por lotes TF-3 (los marcadores de posición mantienen estable el diseño de la traza hasta que se produce el intercambio). |
| Kotodama | Reduce el asistente `transfer_batch((from,to,asset,amount), …)` a `transfer_v1_batch_begin`, llamadas secuenciales `transfer_asset` e `transfer_v1_batch_end`. Cada argumento de tupla debe seguir la forma `(AccountId, AccountId, AssetDefinitionId, int)`; Las transferencias individuales mantienen al constructor existente. |

Ejemplo de uso de Kotodama:

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` ejecuta las mismas comprobaciones aritméticas y de permisos que las llamadas `Transfer::asset_numeric` individuales, pero registra todos los deltas dentro de un único `TransferTranscript`. Las transcripciones multi-delta eluden el resumen de Poseidón hasta que los compromisos por-delta llegan a un seguimiento. El constructor Kotodama ahora emite las llamadas al sistema de inicio/fin automáticamente, por lo que los contratos pueden implementar transferencias por lotes sin codificar manualmente las cargas útiles Norito.

## Arnés de regresión de recuento de filas

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) sintetiza lotes de transición FASTPQ con recuentos de selectores configurables e informa el resumen `row_usage` resultante (`total_rows`, recuentos por selector, relación) junto con la longitud acolchada/log₂. Capture puntos de referencia para el techo de 65536 filas con:

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```El JSON emitido refleja los artefactos por lotes FASTPQ que `iroha_cli audit witness` ahora emite de forma predeterminada (pase `--no-fastpq-batches` para suprimirlos), por lo que `scripts/fastpq/check_row_usage.py` y la puerta CI pueden diferenciar las ejecuciones sintéticas con respecto a instantáneas anteriores al validar los cambios del planificador.

# Plan de implementación

1. **TF-1 (plomería de transcripciones)**: ✅ `StateTransaction::record_transfer_transcripts` ahora emite transcripciones Norito para cada `TransferAsset`/lote, `sumeragi::witness::record_fastpq_transcript` las almacena dentro del testigo global y `StateBlock::capture_exec_witness` compila `fastpq_batches` con `public_inputs` explícito para operadores y el carril de prueba (use `--no-fastpq-batches` si necesita un carril más delgado). salida).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness.rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/iroha_cli/src/audit.rs:185】
2. **TF-2 (implementación de gadget)**: ✅ `gadgets::transfer` ahora valida transcripciones multi-delta (equilibrio aritmético + resumen de Poseidón), sintetiza pruebas SMT emparejadas cuando los hosts las omiten, expone testigos estructurados a través de `TransferGadgetPlan` e `trace::build_trace` enhebra esos testigos en `Trace::transfer_witnesses` mientras completa las columnas SMT de las pruebas. `fastpq_row_bench` captura el arnés de regresión de 65536 filas para que los planificadores realicen un seguimiento del uso de filas sin reproducir Norito cargas útiles.【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (ayudante por lotes)**: habilite el generador de llamadas al sistema por lotes + Kotodama, incluida la aplicación secuencial a nivel de host y el bucle de gadget.
4. **TF-4 (Telemetría y documentos)**: actualice `fastpq_plan.md`, `fastpq_migration_guide.md` y los esquemas del panel para mostrar la asignación de filas de transferencia frente a otros dispositivos.

# Preguntas abiertas

- **Límites de dominio**: el planificador FFT actual entra en pánico para los seguimientos más allá de 2¹⁴ filas. TF-2 debería aumentar el tamaño del dominio o documentar un objetivo de referencia reducido.
- **Lotes de múltiples activos**: el gadget inicial asume el mismo ID de activo por delta. Si necesitamos lotes heterogéneos, debemos asegurarnos de que el testigo de Poseidón incluya el activo cada vez para evitar la repetición entre activos.
- **Reutilización del resumen de autoridad**: a largo plazo podemos reutilizar el mismo resumen para otras operaciones autorizadas para evitar volver a calcular las listas de firmantes por llamada al sistema.


Este documento rastrea las decisiones de diseño; manténgalo sincronizado con las entradas de la hoja de ruta cuando lleguen los hitos.