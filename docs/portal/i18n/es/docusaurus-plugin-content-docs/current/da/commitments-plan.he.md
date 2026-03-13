---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/da/commitments-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 00dbce8345397c064bba02d51de83b24f6d703ab697b175e1b0efbbb7c1d23e9
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: es
direction: ltr
source: docs/portal/docs/da/commitments-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canonica
Refleja `docs/source/da/commitments_plan.md`. Mantenga ambas versiones en
:::

# Plan de compromisos de Data Availability de Sora Nexus (DA-3)

_Redactado: 2026-03-25 -- Responsables: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 extiende el formato de bloque de Nexus para que cada lane incruste registros
deterministas que describen los blobs aceptados por DA-2. Esta nota captura las
estructuras de datos canonicas, los hooks del pipeline de bloques, las pruebas de
cliente ligero y las superficies Torii/RPC que deben aterrizar antes de que los
validadores puedan confiar en compromisos DA durante admision o chequeos de
gobernanza. Todos los payloads estan codificados en Norito; sin SCALE ni JSON ad
hoc.

## Objetivos

- Llevar compromisos por blob (chunk root + manifest hash + commitment KZG
  opcional) dentro de cada bloque Nexus para que los peers puedan reconstruir el
  estado de availability sin consultar almacenamiento fuera del ledger.
- Proveer pruebas de membresia deterministas para que clientes ligeros verifiquen
  que un manifest hash fue finalizado en un bloque dado.
- Exponer consultas Torii (`/v2/da/commitments/*`) y pruebas que permitan a
  relays, SDKs y automatizacion de gobernanza auditar availability sin reproducir
  cada bloque.
- Mantener el envelope `SignedBlockWire` canonico al enhebrar las nuevas
  estructuras a traves del header de metadata Norito y la derivacion del hash de
  bloque.

## Panorama de alcance

1. **Adiciones al modelo de datos** en `iroha_data_model::da::commitment` mas
   cambios de header de bloque en `iroha_data_model::block`.
2. **Hooks del executor** para que `iroha_core` ingeste receipts DA emitidos por
   Torii (`crates/iroha_core/src/queue.rs` y `crates/iroha_core/src/block.rs`).
3. **Persistencia/indexes** para que el WSV responda consultas de compromisos
   rapido (`iroha_core/src/wsv/mod.rs`).
4. **Adiciones RPC en Torii** para endpoints de lista/consulta/prueba bajo
   `/v2/da/commitments`.
5. **Tests de integracion + fixtures** validando el wire layout y el flujo de
   proof en `integration_tests/tests/da/commitments.rs`.

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
```

- `KzgCommitment` reutiliza el punto de 48 bytes usado en `iroha_crypto::kzg`.
  Cuando esta ausente, se vuelve a Merkle proofs solamente.
- `proof_scheme` se deriva del catalogo de lanes; las lanes Merkle rechazan
  payloads KZG mientras que las lanes `kzg_bls12_381` requieren commitments KZG
  no cero. Torii actualmente solo produce compromisos Merkle y rechaza lanes
  configuradas con KZG.
- `KzgCommitment` reutiliza el punto de 48 bytes usado en `iroha_crypto::kzg`.
  Cuando esta ausente en lanes Merkle se vuelve a Merkle proofs solamente.
- `proof_digest` anticipa la integracion DA-5 PDP/PoTR para que el mismo record
  enumere el schedule de sampling usado para mantener blobs vivos.

### 1.2 Extension del header de bloque

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

El hash del bundle entra tanto en el hash del bloque como en la metadata de
`SignedBlockWire`. Cuando un bloque no lleva datos DA el campo permanece `None`

Nota de implementacion: `BlockPayload` y el transparente `BlockBuilder` ahora
exponen setters/getters `da_commitments` (ver `BlockBuilder::set_da_commitments`
y `SignedBlock::set_da_commitments`), asi que los hosts pueden adjuntar un bundle
preconstruido antes de sellar un bloque. Todos los constructores helper dejan el
campo en `None` hasta que Torii enhebre bundles reales.

### 1.3 Encoding de wire

- `SignedBlockWire::canonical_wire()` agrega el header Norito para
  `DaCommitmentBundle` inmediatamente despues de la lista de transacciones
  existente. El byte de version es `0x01`.
- `SignedBlockWire::decode_wire()` rechaza bundles cuyo `version` es desconocido,
  siguiendo la politica Norito descrita en `norito.md`.
- Las actualizaciones de derivacion de hash viven solo en `block::Hasher`; los
  clientes ligeros que decodifican el wire format existente ganan el nuevo campo
  automaticamente porque el header Norito anuncia su presencia.

## 2. Flujo de produccion de bloques

1. La ingesta DA de Torii finaliza un `DaIngestReceipt` y lo publica en la cola
   interna (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` recopila todos los receipts cuyo `lane_id` coincide con el
   bloque en construccion, deduplicando por `(lane_id, client_blob_id,
   manifest_hash)`.
3. Justo antes de sellar, el builder ordena los compromisos por `(lane_id,
   epoch, sequence)` para mantener el hash determinista, codifica el bundle con
   el codec Norito, y actualiza `da_commitments_hash`.
4. El bundle completo se almacena en el WSV y se emite junto al bloque dentro de
   `SignedBlockWire`.

Si la creacion del bloque falla, los receipts permanecen en la cola para que el
siguiente intento los tome; el builder registra el ultimo `sequence` incluido
por lane para evitar ataques de replay.

## 3. Superficie RPC y de consulta

Torii expone tres endpoints:

| Ruta | Metodo | Payload | Notas |
|------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (filtro por rango de lane/epoch/sequence, paginacion) | Devuelve `DaCommitmentPage` con total, compromisos y hash de bloque. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (lane + manifest hash o tupla `(epoch, sequence)`). | Responde con `DaCommitmentProof` (record + ruta Merkle + hash de bloque). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless que reejecuta el calculo del hash de bloque y valida inclusion; usado por SDKs que no pueden enlazar directo a `iroha_crypto`. |

Todos los payloads viven bajo `iroha_data_model::da::commitment`. Los routers de
Torii montan los handlers junto a los endpoints de ingesta DA existentes para
reutilizar politicas de token/mTLS.

## 4. Pruebas de inclusion y clientes ligeros

- El productor de bloques construye un arbol Merkle binario sobre la lista
  serializada de `DaCommitmentRecord`. La raiz alimenta `da_commitments_hash`.
- `DaCommitmentProof` empaqueta el record objetivo mas un vector de
  `(sibling_hash, position)` para que los verificadores reconstruyan la raiz. Las
  pruebas tambien incluyen el hash de bloque y el header firmado para que
  clientes ligeros verifiquen finality.
- Helpers de CLI (`iroha_cli app da prove-commitment`) envuelven el ciclo de
  solicitud/verificacion de pruebas y exponen salidas Norito/hex para
  operadores.

## 5. Storage e indexacion

El WSV almacena compromisos en una column family dedicada con clave
`manifest_hash`. Los indexes secundarios cubren `(lane_id, epoch)` y
`(lane_id, sequence)` para que las consultas eviten escanear bundles completos.
Cada record rastrea la altura del bloque que lo sello, permitiendo a nodos en
catch-up reconstruir el indice rapidamente desde el block log.

## 6. Telemetria y observabilidad

- `torii_da_commitments_total` incrementa cuando un bloque sella al menos un
  record.
- `torii_da_commitment_queue_depth` rastrea receipts esperando ser empaquetados
  (por lane).
- El dashboard Grafana `dashboards/grafana/da_commitments.json` visualiza la
  inclusion en bloques, profundidad de cola y throughput de pruebas para que
  los gates de release de DA-3 puedan auditar el comportamiento.

## 7. Estrategia de pruebas

1. **Tests unitarios** para encoding/decoding de `DaCommitmentBundle` y
   actualizaciones de derivacion del hash de bloque.
2. **Fixtures golden** bajo `fixtures/da/commitments/` que capturan bytes
   canonicos del bundle y pruebas Merkle.
3. **Tests de integracion** levantando dos validadores, ingiriendo blobs de
   muestra y verificando que ambos nodos concuerdan en el contenido del bundle y
   las respuestas de consulta/prueba.
4. **Tests de cliente ligero** en `integration_tests/tests/da/commitments.rs`
   (Rust) que llaman `/prove` y verifican la prueba sin hablar con Torii.
5. **Smoke de CLI** con `scripts/da/check_commitments.sh` para mantener tooling
   de operadores reproducible.

## 8. Plan de rollout

| Fase | Descripcion | Criterio de salida |
|------|-------------|--------------------|
| P0 - Merge de modelo de datos | Integrar `DaCommitmentRecord`, actualizaciones de header de bloque y codecs Norito. | `cargo test -p iroha_data_model` en verde con nuevas fixtures. |
| P1 - Cableado Core/WSV | Enhebrar logica de cola + block builder, persistir indexes y exponer handlers RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` pasan con assertions de bundle proof. |
| P2 - Tooling de operadores | Lanzar helpers de CLI, dashboard Grafana y actualizaciones de docs de verificacion de proof. | `iroha_cli app da prove-commitment` funciona contra devnet; el dashboard muestra datos en vivo. |
| P3 - Gate de gobernanza | Habilitar el validador de bloques que requiere compromisos DA en las lanes marcadas en `iroha_config::nexus`. | Entrada de status + update de roadmap marcan DA-3 como COMPLETADO. |

## Preguntas abiertas

1. **KZG vs Merkle defaults** - Debemos omitir compromisos KZG en blobs pequenos
   para reducir el tamano del bloque? Propuesta: mantener `kzg_commitment`
   opcional y gatear via `iroha_config::da.enable_kzg`.
2. **Sequence gaps** - Permitimos lanes fuera de orden? El plan actual rechaza
   gaps salvo que gobernanza active `allow_sequence_skips` para replay de
   emergencia.
3. **Light-client cache** - El equipo de SDK pidio un cache SQLite liviano para
   proofs; seguimiento pendiente bajo DA-8.

Responder estas preguntas en PRs de implementacion mueve DA-3 de BORRADOR (este
documento) a EN PROGRESO cuando el trabajo de codigo comience.
