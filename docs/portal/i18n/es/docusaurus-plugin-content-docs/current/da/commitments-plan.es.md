---
lang: es
direction: ltr
source: docs/portal/docs/da/commitments-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Refleja `docs/source/da/commitments_plan.md`. Mantenga ambas versiones en
:::

# Plan de compromisos de Disponibilidad de Datos de Sora Nexus (DA-3)

_Redactado: 2026-03-25 -- Responsables: Core Protocol WG / Equipo de Contrato Inteligente / Equipo de Almacenamiento_

DA-3 extiende el formato del bloque de Nexus para que cada carril incruste registros
deterministas que describen los blobs aceptados por DA-2. Esta nota captura las
estructuras de datos canónicas, los ganchos del pipeline de bloques, las pruebas de
cliente ligero y las superficies Torii/RPC que deben aterrizar antes de que los
validadores puedan confiar en compromisos DA durante la admisión o chequeos de
gobierno. Todas las cargas útiles están codificadas en Norito; anuncio sin ESCALA ni JSON
hoc.

## Objetivos- Llevar compromisos por blob (chunk root + manifest hash + compromiso KZG
  opcional) dentro de cada bloque Nexus para que los pares puedan reconstruir el
  estado de disponibilidad sin consultar almacenamiento fuera del libro mayor.
- Proveer pruebas de membresia deterministas para que clientes ligeros verifiquen
  que un hash manifiesto fue finalizado en un bloque dado.
- Exponer consultas Torii (`/v1/da/commitments/*`) y pruebas que permitieron a
  relés, SDKs y automatización de gobernanza auditar disponibilidad sin reproducir
  cada bloque.
- Mantener el sobre `SignedBlockWire` canonico al enhebrar las nuevas
  estructuras a través del header de metadata Norito y la derivacion del hash de
  bloque.

## Panorama de alcance

1. **Adiciones al modelo de datos** en `iroha_data_model::da::commitment` mas
   cambios de encabezado de bloque en `iroha_data_model::block`.
2. **Hooks del ejecutor** para que `iroha_core` ingeste recibos DA emitidos por
   Torii (`crates/iroha_core/src/queue.rs` y `crates/iroha_core/src/block.rs`).
3. **Persistencia/indexes** para que el WSV responda consultas de compromisos
   rápido (`iroha_core/src/wsv/mod.rs`).
4. **Adiciones RPC en Torii** para endpoints de lista/consulta/prueba bajo
   `/v1/da/commitments`.
5. **Pruebas de integración + accesorios** validando el cableado y el flujo de
   prueba en `integration_tests/tests/da/commitments.rs`.

## 1. Adiciones al modelo de datos

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```- `KzgCommitment` reutiliza el punto de 48 bytes usado en `iroha_crypto::kzg`.
  Cuando está ausente, se vuelve a Merkle pruebas solamente.
- `proof_scheme` se deriva del catálogo de carriles; las lanes Merkle rechazan
  payloads KZG mientras que las lanes `kzg_bls12_381` requieren compromisos KZG
  sin cero. Torii actualmente solo produce compromisos Merkle y rechaza lanes
  configuradas con KZG.
- `KzgCommitment` reutiliza el punto de 48 bytes usado en `iroha_crypto::kzg`.
  Cuando esta ausente en lanes Merkle se vuelve a Merkle pruebas solamente.
- `proof_digest` anticipa la integración DA-5 PDP/PoTR para que el mismo registro
  Enumere el cronograma de muestreo usado para mantener blobs vivos.

### 1.2 Extensión del encabezado de bloque

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

El hash del paquete entra tanto en el hash del bloque como en la metadata de
`SignedBlockWire`. Cuando un bloque no lleva datos DA el campo permanece `None`

Nota de implementación: `BlockPayload` y el transparente `BlockBuilder` ahora
Establecedores/captadores de exponentes `da_commitments` (ver `BlockBuilder::set_da_commitments`
y `SignedBlock::set_da_commitments`), asi que los hosts pueden adjuntar un paquete
preconstruido antes de sellar un bloque. Todos los constructores ayudante dejan el
campo en `None` hasta que Torii enhebre paquetes reales.

### 1.3 Codificación del cable- `SignedBlockWire::canonical_wire()` agrega el encabezado Norito para
  `DaCommitmentBundle` inmediatamente despues de la lista de transacciones
  existentes. El byte de versión es `0x01`.
- `SignedBlockWire::decode_wire()` rechaza paquetes cuyo `version` es desconocido,
  siguiendo la política Norito descrita en `norito.md`.
- Las actualizaciones de derivación de hash viven solo en `block::Hasher`; los
  clientes ligeros que decodifican el formato de alambre existente ganan el nuevo campo
  automaticamente porque el header Norito anuncia su presencia.

## 2. Flujo de producción de bloques

1. La ingesta DA de Torii finaliza un `DaIngestReceipt` y lo publica en la cola
   interno (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` recopila todos los recibos cuyos `lane_id` coinciden con el
   bloque en construcción, deduplicando por `(lane_id, client_blob_id,
   manifest_hash)`.
3. Justo antes de sellar, el constructor ordena los compromisos por `(lane_id,
   época, secuencia)` para mantener el hash determinista, codifica el paquete con
   el códec Norito, y actualiza `da_commitments_hash`.
4. El paquete completo se almacena en el WSV y se emite junto al bloque dentro de
   `SignedBlockWire`.

Si la creacion del bloque falla, los recibos permanecen en la cola para que el
siguiente intento los tome; el constructor registra el ultimo `sequence` incluido
por carril para evitar ataques de repetición.## 3. Superficie RPC y de consulta

Torii exponen tres puntos finales:

| Ruta | Método | Carga útil | Notas |
|------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtro por rango de carril/época/secuencia, paginación) | Devuelve `DaCommitmentPage` con total, compromisos y hash de bloque. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (carril + hash de manifiesto o tupla `(epoch, sequence)`). | Responde con `DaCommitmentProof` (record + ruta Merkle + hash de bloque). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless que reejecuta el cálculo del hash de bloque y valida la inclusión; usados ​​por SDK que no pueden enlazar directamente a `iroha_crypto`. |

Todas las cargas útiles viven bajo `iroha_data_model::da::commitment`. Los enrutadores de
Torii montan los handlers junto a los endpoints de ingesta DA existentes para
reutilizar políticas de token/mTLS.

## 4. Pruebas de inclusión y clientes ligeros- El productor de bloques construye un árbol Merkle binario sobre la lista
  serializada de `DaCommitmentRecord`. La raiz alimenta `da_commitments_hash`.
- `DaCommitmentProof` empaqueta el record objetivo mas un vector de
  `(sibling_hash, position)` para que los verificadores reconstruyan la raiz. Las
  Las pruebas también incluyen el hash de bloque y el encabezado firmado para que
  clientes ligeros verifiquen finalidad.
- Helpers de CLI (`iroha_cli app da prove-commitment`) envuelven el ciclo de
  solicitud/verificacion de pruebas y exponentes salidas Norito/hex para
  operadores.

## 5. Almacenamiento e indexación

El WSV almacena compromisos en una columna familiar dedicada con clave
`manifest_hash`. Los indexes secundarios cubren `(lane_id, epoch)` y
`(lane_id, sequence)` para que las consultas eviten escanear paquetes completos.
Cada record rastrea la altura del bloque que lo sello, permitiendo a nodos en
catch-up reconstruir el índice rápidamente desde el block log.

## 6. Telemetria y observabilidad

- `torii_da_commitments_total` incrementa cuando un bloque sella al menos un
  registro.
- `torii_da_commitment_queue_depth` rastrea recibos esperando ser empaquetados
  (por carril).
- El tablero Grafana `dashboards/grafana/da_commitments.json` visualiza la
  inclusión en bloques, profundidad de cola y rendimiento de pruebas para que
  las puertas de liberación de DA-3 puedan auditar el comportamiento.

## 7. Estrategia de pruebas1. **Pruebas unitarias** para codificación/decodificación de `DaCommitmentBundle` y
   Actualizaciones de derivación del hash de bloque.
2. **Fixtures golden** bajo `fixtures/da/commitments/` que capturan bytes
   canónicos del paquete y pruebas Merkle.
3. **Tests de integracion** levantando dos validadores, ingiriendo blobs de
   muestra y verificando que ambos nodos concuerdan en el contenido del paquete y
   las respuestas de consulta/prueba.
4. **Pruebas de cliente ligero** en `integration_tests/tests/da/commitments.rs`
   (Rust) que llaman `/prove` y verifican la prueba sin hablar con Torii.
5. **Smoke de CLI** con `scripts/da/check_commitments.sh` para mantener herramientas
   de operadores reproducible.

## 8. Plan de implementación| Fase | Descripción | Criterio de salida |
|------|-------------|--------------------|
| P0 - Fusionar modelo de datos | Integrar `DaCommitmentRecord`, actualizaciones de encabezado de bloque y códecs Norito. | `cargo test -p iroha_data_model` en verde con accesorios nuevos. |
| P1 - Cableado Core/WSV | Enhebrar logica de cola + block builder, persistir indexes y exponer handlers RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` pasan con afirmaciones de prueba de paquete. |
| P2 - Herramientas de operadores | Lanzar ayudantes de CLI, panel Grafana y actualizaciones de documentos de verificación de prueba. | `iroha_cli app da prove-commitment` funciona contra devnet; El tablero muestra datos en vivo. |
| P3 - Puerta de gobernanza | Habilitar el validador de bloques que requiere compromisos DA en las líneas marcadas en `iroha_config::nexus`. | Entrada de estado + actualización de hoja de ruta marcan DA-3 como COMPLETADO. |

## Preguntas abiertas

1. **KZG vs Merkle defaults** - Debemos omitir compromisos KZG en blobs pequeños
   para reducir el tamano del bloque? Propuesta: mantener `kzg_commitment`
   opcional y gatear vía `iroha_config::da.enable_kzg`.
2. **Brechos de secuencia** - ¿Permitimos carriles fuera de orden? El plan actual rechaza
   gaps salvo que gobernanza active `allow_sequence_skips` para replay de
   emergencia.
3. **Light-client cache** - El equipo de SDK pidio un cache SQLite liviano para
   pruebas; seguimiento pendiente bajo DA-8.Responder estas preguntas en PRs de implementación mueve DA-3 de BORRADOR (este
documento) a EN PROGRESO cuando el trabajo de código comienza.