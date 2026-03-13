---
lang: es
direction: ltr
source: docs/source/da/commitments_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ea1b16b73a55e3e47dfe9d5bfc77dedce2e8fa9ff964d244856767f14931733
source_last_modified: "2026-01-22T15:38:30.660808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus Plan de compromisos de disponibilidad de datos (DA-3)

_Redactado: 2026-03-25 — Propietarios: Core Protocol WG / Equipo de contrato inteligente / Equipo de almacenamiento_

DA-3 amplía el formato de bloque Nexus para que cada carril incorpore registros deterministas
describiendo las manchas aceptadas por DA-2. Esta nota captura los datos canónicos.
estructuras, ganchos de bloques de tuberías, pruebas de cliente ligero y superficies Torii/RPC
que debe aterrizar antes de que los validadores puedan confiar en los compromisos de DA durante la admisión o
controles de gobernanza. Todas las cargas útiles están codificadas con Norito; sin ESCALA o JSON ad-hoc.

## Objetivos

- Llevar compromisos por blob (raíz de fragmento + hash de manifiesto + KZG opcional)
  compromiso) dentro de cada bloque Nexus para que los pares puedan reconstruir la disponibilidad
  estado sin consultar el almacenamiento fuera del libro mayor.
- Proporcionar pruebas de membresía deterministas para que los clientes ligeros puedan verificar que un
  El hash manifiesto se finalizó en un bloque determinado.
- Exponer consultas Torii (`/v2/da/commitments/*`) y pruebas que permiten retransmisiones,
  SDK y disponibilidad de auditoría de automatización de gobernanza sin reproducir cada
  bloque.
- Mantenga canónico el sobre `SignedBlockWire` existente enhebrando el nuevo
  estructuras a través del encabezado de metadatos Norito y la derivación de hash de bloque.

## Descripción general del alcance

1. **Adiciones al modelo de datos** en el bloque `iroha_data_model::da::commitment` plus
   cambios de encabezado en `iroha_data_model::block`.
2. **El ejecutor se engancha** para que `iroha_core` ingiera los recibos de DA emitidos por Torii
   (`crates/iroha_core/src/queue.rs` y `crates/iroha_core/src/block.rs`).
3. **Persistencia/índices** para que el WSV pueda responder consultas de compromiso rápidamente
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii Adiciones de RPC** para enumerar/consultar/probar puntos finales en
   `/v2/da/commitments`.
5. **Pruebas de integración + accesorios** validando el diseño del cable y el flujo de prueba en
   `integration_tests/tests/da/commitments.rs`.

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

- `KzgCommitment` reutiliza el punto de 48 bytes existente utilizado en
  `iroha_crypto::kzg`. Los carriles Merkle lo dejan vacío; Carriles `kzg_bls12_381` ahora
  recibir un compromiso BLAKE3-XOF determinista derivado de la raíz del fragmento y
  ticket de almacenamiento para que los hashes de bloque se mantengan estables sin un probador externo.
- `proof_scheme` se deriva del catálogo de carriles; Merkle Lanes rechaza KZG perdido
  cargas útiles, mientras que los carriles `kzg_bls12_381` requieren compromisos KZG distintos de cero.
- `proof_digest` anticipa la integración DA-5 PDP/PoTR, por lo que se obtendrá el mismo registro
  enumera el programa de muestreo utilizado para mantener vivos los blobs.

### 1.2 Extensión del encabezado del bloque

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

El hash del paquete alimenta tanto el hash del bloque como los metadatos `SignedBlockWire`.
arriba.

Nota de implementación: `BlockPayload` y el `BlockBuilder` transparente ahora exponen
Configuradores/captadores `da_commitments` (consulte `BlockBuilder::set_da_commitments` y
`SignedBlock::set_da_commitments`), para que los hosts puedan adjuntar un paquete prediseñado
antes de sellar un bloque. Todos los constructores auxiliares tienen por defecto el campo `None`.
hasta que Torii pase paquetes reales.

### 1.3 Codificación de cables- `SignedBlockWire::canonical_wire()` agrega el encabezado Norito para
  `DaCommitmentBundle` inmediatamente después de la lista de transacciones existente. el
  El byte de versión es `0x01`.
- `SignedBlockWire::decode_wire()` rechaza paquetes cuyo `version` se desconoce,
  que coincide con la política Norito descrita en `norito.md`.
- Las actualizaciones de derivación de hash existen solo en `block::Hasher`; decodificación de clientes ligeros
  El formato de cable existente obtiene automáticamente el nuevo campo porque Norito
  El encabezado anuncia su presencia.

## 2. Flujo de producción de bloques

1. Torii La ingesta de DA conserva los recibos firmados y los registros de compromiso en el
   Carrete DA (`da-receipt-*.norito` / `da-commitment-*.norito`). El duradero
   El registro de recibos genera cursores al reiniciar para que los recibos reproducidos aún estén ordenados.
   deterministamente.
2. El conjunto de bloques carga los recibos del carrete y los deja obsoletos/ya sellados.
   entradas utilizando la instantánea del cursor comprometida, y aplica la contigüidad por
   `(lane, epoch)`. Si un recibo accesible carece de un compromiso de contrapartida o el
   manifiesto hash diverge la propuesta aborta en lugar de omitirla silenciosamente.
3. Justo antes de sellar, el constructor corta el paquete de compromiso al
   conjunto basado en recibos, ordenado por `(lane_id, epoch, sequence)`, codifica el
   paquete con el códec Norito y actualizaciones `da_commitments_hash`.
4. El paquete completo se almacena en el WSV y se emite junto con el bloque interior.
   `SignedBlockWire`; los paquetes comprometidos hacen avanzar los cursores de recibo (hidratados
   de Kura al reiniciar) y podar las entradas de spool obsoletas para limitar el crecimiento del disco.

El ensamblaje del bloque y la ingesta de `BlockCreated` revalidan cada compromiso contra
el catálogo de carriles: los carriles Merkle rechazan los compromisos extraviados de KZG, los carriles KZG requieren un
compromiso KZG distinto de cero y `chunk_root` distinto de cero, y carriles desconocidos son
cayó. El punto final `/v2/da/commitments/verify` de Torii refleja la misma protección,
e ingerir ahora entrelaza el compromiso determinista de KZG en cada
Registro `kzg_bls12_381` para que los paquetes que cumplen con la política lleguen al ensamblaje del bloque.

Los accesorios manifiestos descritos en el plan de ingesta DA-2 también sirven como fuente de
verdad para el paquete de compromiso. La prueba Torii
`manifest_fixtures_cover_all_blob_classes` regenera manifiestos para cada
Variante `BlobClass` y se niega a compilar hasta que nuevas clases obtengan accesorios,
asegurando que el hash del manifiesto codificado dentro de cada `DaCommitmentRecord` coincida con el
par dorado Norito/JSON.【crates/iroha_torii/src/da/tests.rs:2902】

Si la creación del bloque falla, los recibos permanecen en la cola para que el siguiente bloque
el intento puede recogerlos; el constructor registra el último `sequence` incluido por
carril para evitar ataques de repetición.

## 3. RPC y superficie de consulta

Torii expone tres puntos finales:| Ruta | Método | Carga útil | Notas |
|-------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (filtro de rango por carril/época/secuencia, paginación) | Devuelve `DaCommitmentPage` con recuento total, compromisos y hash de bloque. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (carril + hash de manifiesto o tupla `(epoch, sequence)`). | Responde con `DaCommitmentProof` (registro + ruta Merkle + hash de bloque). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | Ayudante sin estado que reproduce el cálculo del hash del bloque y valida la inclusión; utilizado por SDK que no pueden vincularse directamente a `iroha_crypto`. |

Todas las cargas útiles se encuentran bajo `iroha_data_model::da::commitment`. Montaje de enrutadores Torii
los controladores junto a los puntos finales de ingesta de DA existentes para reutilizar token/mTLS
políticas.

## 4. Pruebas de inclusión y clientes ligeros

- El productor de bloques construye un árbol binario de Merkle sobre el serializado.
  Lista `DaCommitmentRecord`. La raíz alimenta `da_commitments_hash`.
- `DaCommitmentProof` empaqueta el registro de destino más un vector de `(sibling_hash,
  position)` entradas para que los verificadores puedan reconstruir la raíz. Las pruebas también incluyen
  el hash del bloque y el encabezado firmado para que los clientes ligeros puedan verificar la finalidad.
- Los ayudantes de CLI (`iroha_cli app da prove-commitment`) envuelven/verifican la solicitud de prueba
  ciclo y superficie Norito/salidas hex para operadores.

## 5. Almacenamiento e indexación

WSV almacena compromisos en una familia de columnas dedicada con la clave `manifest_hash`.
Los índices secundarios cubren `(lane_id, epoch)` e `(lane_id, sequence)`, por lo que las consultas
Evite escanear paquetes completos. Cada registro rastrea la altura del bloque que lo selló,
permitiendo que los nodos de recuperación reconstruyan el índice rápidamente a partir del registro de bloques.

## 6. Telemetría y observabilidad

- `torii_da_commitments_total` aumenta cada vez que un bloque sella al menos uno
  registro.
- `torii_da_commitment_queue_depth` rastrea los recibos en espera de ser agrupados (según
  carril).
- Grafana tablero `dashboards/grafana/da_commitments.json` visualiza bloque
  inclusión, profundidad de la cola y rendimiento de prueba para que las puertas de liberación DA-3 puedan auditar
  comportamiento.

## 7. Estrategia de prueba

1. **Pruebas unitarias** para codificación/decodificación `DaCommitmentBundle` y hash de bloque
   actualizaciones de derivación.
2. **Accesorios dorados** bajo `fixtures/da/commitments/` capturando canónico
   agrupar bytes y pruebas de Merkle. Cada paquete hace referencia a los bytes del manifiesto.
   de `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}`, entonces
   regenerando `cargo test -p iroha_torii regenerate_da_ingest_fixtures -- --ignored --nocapture`
   mantiene la historia Norito consistente antes de que `ci/check_da_commitments.sh` actualice el compromiso
   pruebas.【fixtures/da/ingest/README.md:1】
3. **Pruebas de integración** arrancando dos validadores, ingiriendo blobs de muestra y
   afirmando que ambos nodos están de acuerdo con el contenido del paquete y la consulta/prueba
   respuestas.
4. **Pruebas de cliente ligero** en `integration_tests/tests/da/commitments.rs`
   (Rust) que llaman a `/prove` y verifican la prueba sin hablar con Torii.
5. **CLI smoke** script `scripts/da/check_commitments.sh` para mantener al operador
   herramientas reproducibles.

## 8. Plan de implementación| Fase | Descripción | Criterios de salida |
|-------|-------------|---------------|
| P0: fusión del modelo de datos | Land `DaCommitmentRecord`, actualizaciones de encabezado de bloque y códecs Norito. | `cargo test -p iroha_data_model` verde con accesorios nuevos. |
| P1 — Cableado central/WSV | Cola de subprocesos + lógica de creación de bloques, índices persistentes y exposición de controladores RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` pasan con afirmaciones de prueba de paquete. |
| P2 — Herramientas del operador | Envíe ayudantes de CLI, panel Grafana y actualizaciones de documentos de verificación de pruebas. | `iroha_cli app da prove-commitment` funciona contra devnet; El panel muestra datos en vivo. |
| P3 — Puerta de gobernanza | Habilite el validador de bloques que requiere compromisos de DA en los carriles marcados en `iroha_config::nexus`. | Entrada de estado + actualización de la hoja de ruta marca DA-3 como 🈴. |

## Preguntas abiertas

1. **Valores predeterminados de KZG frente a Merkle**: ¿deben los pequeños blobs omitir siempre los compromisos de KZG?
   ¿Reducir el tamaño del bloque? Propuesta: mantener `kzg_commitment` opcional y puerta vía
   `iroha_config::da.enable_kzg`.
2. **Brechos en la secuencia**: ¿permitimos carriles fuera de orden? El plan actual rechaza las lagunas
   a menos que el gobierno cambie `allow_sequence_skips` para reproducción de emergencia.
3. **Caché de cliente liviano**: el equipo del SDK solicitó un caché SQLite liviano para
   pruebas; pendiente de seguimiento bajo DA-8.

Responder a estas preguntas en la implementación mueve DA-3 de 🈸 (este documento) a 🈺
una vez que comienza el trabajo del código.