---
lang: es
direction: ltr
source: docs/portal/docs/da/commitments-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Reflejo `docs/source/da/commitments_plan.md`. Gardez les deux versiones sincronizadas
:::

# Plan de compromisos Disponibilidad de datos Sora Nexus (DA-3)

_Redige: 2026-03-25 -- Responsables: Core Protocol WG / Equipo de Contrato Inteligente / Equipo de Almacenamiento_

DA-3 amplía el formato de bloque Nexus para que cada carril integre los registros
Determina la descripción de los blobs aceptados por DA-2. Estos archivos de captura de notas
estructuras de mujeres canónicas, les ganchos del tubo de bloques, les preuves de
client leger, et les marks Torii/RPC qui doivent Arrivalr avant que les
validadores puissent s'appuyer sur les engagements DA lors des checks
d'admission ou de gouvernance. Todas las cargas útiles están codificadas en Norito; paso de
ESCALA ni de JSON ad hoc.

## Objetivos- Porter des engagements par blob (raíz de fragmento + hash de manifiesto + compromiso KZG
  optionnel) en cada bloque Nexus para que los pares puedan reconstruire
  El estado de disponibilidad sin consultar el almacenamiento fuera del libro mayor.
- Fournir des preuves de member deterministes afin que les client legers
  Verifient qu'un manifest hash a ete finalize dans un bloc donne.
- Exposer des requetes Torii (`/v1/da/commitments/*`) et des preuves permettant
  Relés auxiliares, SDK y automatizaciones de gobierno de auditoría de disponibilidad.
  sans rejouer chaque bloc.
- Conservar el sobre `SignedBlockWire` canonique en transmettant les nouvelles
  estructuras a través del encabezado de metadatos Norito y la derivación del hash de bloque.

## Vista del conjunto del alcance

1. **Ajustado al modelo de datos** en `iroha_data_model::da::commitment` plus
   modificaciones del encabezado de bloque en `iroha_data_model::block`.
2. **Hooks d'executor** para que `iroha_core` ingeste los recibos DA emis par
   Torii (`crates/iroha_core/src/queue.rs` y `crates/iroha_core/src/block.rs`).
3. **Persistencia/índices** después de que el WSV responda rápidamente a las solicitudes de
   compromisos (`iroha_core/src/wsv/mod.rs`).
4. **Ajouts RPC Torii** para los puntos finales de lista/lectura/prueba bajo
   `/v1/da/commitments`.
5. **Pruebas de integración + accesorios** validan el diseño del cable y el flujo de prueba
   en `integration_tests/tests/da/commitments.rs`.

## 1. Ampliar el modelo de datos

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
```- `KzgCommitment` reutiliza el punto 48 octetos utilizados en `iroha_crypto::kzg`.
  Quand il est ausente, on retombe sur des preuves Merkle Uniquement.
- `proof_scheme` deriva del catálogo de carriles; les lanes Merkle rejettent les
  cargas útiles KZG tandis que les lanes `kzg_bls12_381` exigentes des compromisos KZG
  no nulos. Torii ne produit actuellement que des engagements Merkle et rejette
  les carriles configurados en KZG.
- `KzgCommitment` reutiliza el punto 48 octetos utilizados en `iroha_crypto::kzg`.
  Cuando está ausente en las calles Merkle on retombe sur des preuves Merkle
  singularidad.
- `proof_digest` anticipe l'integration DA-5 PDP/PoTR afin que le meme record
  Enumere el programa de muestreo utilizado para mantener los blobs en vida.

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

Le hash du bundle entre a la fois dans le hash de bloc et dans la metadata de
`SignedBlockWire`. Quand un bloc ne transporte pas de donnees DA, le champ reste

Nota de implementación: `BlockPayload` y el expuestor transparente `BlockBuilder`
mantenimiento de setters/getters `da_commitments` (ver
`BlockBuilder::set_da_commitments` y `SignedBlock::set_da_commitments`), donc
Los hosts pueden adjuntar un paquete preconstruido antes de crear un bloque. Todos
les helpers laissent le champ a `None` tant que Torii ne fournit pas de bundles
carretes.

### 1.3 Cable de codificación- `SignedBlockWire::canonical_wire()` agregue el encabezado Norito para
  `DaCommitmentBundle` inmediatamente después de la lista de transacciones existentes.
  El byte de versión es `0x01`.
- `SignedBlockWire::decode_wire()` rechaza los paquetes que no `version` est
  inconnue, en ligne avec la politique Norito decrite dans `norito.md`.
- Les mises a jour de derivation du hash vivent only dans `block::Hasher`;
  les client legers qui decodent le wire format existente gagnent le nouveau
  Champ automatiquement car le header Norito anuncia su presencia.

## 2. Flujo de producción de bloques

1. La ingesta DA Torii finaliza un `DaIngestReceipt` y se publica en la cola
   interno (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` recopila todos los recibos que `lane_id` no corresponden en bloque
   en construcción, en dedupliquant par `(lane_id, client_blob_id,
   manifest_hash)`.
3. Justo antes del almacenamiento, el constructor prueba los compromisos par `(lane_id, epoch,
   secuencia)` para guardar el hash determinante, codificar el paquete con el códec
   Norito, y conocí un día `da_commitments_hash`.
4. Le bundle complet est stocke dans le WSV et emis avec le bloc dans
   `SignedBlockWire`.

Si la creación del bloque hace eco, los recibos reposan en la cola para que la
prochaine tentative les reprenne; el constructor registra el último `sequence`
Incluye par carril para evitar los ataques de repetición.## 3. RPC de superficie y solicitudes

Torii expone tres puntos finales:

| Ruta | Método | Carga útil | Notas |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtro de rango carril/época/secuencia, paginación) | Renvoie `DaCommitmentPage` con total, compromisos y hash de bloque. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (carril + hash de manifiesto o tupla `(epoch, sequence)`). | Responda con `DaCommitmentProof` (registro + camino Merkle + hash de bloque). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless qui rejoue le calcul du hash de bloc et valide l'inclusion; Utilice los SDK que no pueden utilizarse directamente `iroha_crypto`. |

Todas las cargas útiles viven en `iroha_data_model::da::commitment`. Los enrutadores
Torii monte los manejadores en la base de puntos finales de ingesta DA existentes para
Reutilizar tokens políticos/mTLS.

## 4. Preuves d'inclusion et client legers- El productor de bloques construye un árbol Merkle binaire sur la liste
  serialisee des `DaCommitmentRecord`. La racine alimente `da_commitments_hash`.
- `DaCommitmentProof` emballe le record cible plus un vecteur de
  `(sibling_hash, position)` afin que les verificateurs puissent reconstruire la
  racina. Las opciones incluyen además el hash de bloque y el encabezado firmado para que
  les client legers puissent verifier la finalite.
- Les helpers CLI (`iroha_cli app da prove-commitment`) ciclo envolvente
  request/verify et exponent des sorties Norito/hex para los operadores.

## 5. Almacenamiento e indexación

Le WSV almacena los compromisos en una columna familiar dediee, cle par
`manifest_hash`. Des indexaires secondaires couvrent `(lane_id, epoch)` et
`(lane_id, sequence)` afin que les requetes eviten de scanner des bundles
completa. Chaque record traje la hauteur de bloc qui l'a scelle, permettant aux
Noeuds en rattrapage de reconstruire el índice rápidamente a partir del registro de bloques.

## 6. Telemetría y observabilidad

- `torii_da_commitments_total` incrementa el bloque inicial al menor
  registro.
- `torii_da_commitment_queue_depth` se adapta a los recibos en atención al paquete
  (par carril).
- Le tablero Grafana `dashboards/grafana/da_commitments.json` visualizar
  La inclusión de bloques, la profundidad de la cola y el rendimiento previo para
  que les gates de release DA-3 puissent auditer le comportement.

## 7. Estrategia de pruebas1. **Pruebas unitarias** para la codificación/decodificación de `DaCommitmentBundle` y los
   pierde un día de derivación de hash de bloc.
2. **Accesorios dorados** con `fixtures/da/commitments/` capturando los bytes
   Canoniques du bundle et les preuves Merkle.
3. **Pruebas de integración** demarrant dos validadores, ingestant des blobs de
   test, et verifiant que les dos noeuds concordent sur le contenu du bundle et
   les reponses de query/proof.
4. **Prueba el cliente ligero** en `integration_tests/tests/da/commitments.rs`
   (Rust) qui apelante `/prove` et verifient la preuve sans parler a Torii.
5. **Smoke CLI** con `scripts/da/check_commitments.sh` para guardar el consumo
   Operador reproducible.

## 8. Plan de implementación| Fase | Descripción | Criterios de salida |
|-------|-------------|---------------|
| P0 - Fusionar el modelo de datos | Integrer `DaCommitmentRecord`, pierde un día de encabezado de bloque y códecs Norito. | `cargo test -p iroha_data_model` vert con nuevas luminarias. |
| P1 - Núcleo de cableado/WSV | Cableado logístico de cola + generador de bloques, persiste los índices y expone los controladores RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` pasan con afirmaciones de prueba de paquete. |
| P2 - Operador de herramientas | Libere la CLI de ayuda, el panel Grafana y pierda un día de documentos de verificación de prueba. | `iroha_cli app da prove-commitment` funciona en devnet; El cartel del panel de control en vivo. |
| P3 - Puerta de gobierno | Active el validador de bloques que requieren compromisos DA en las vías señalizadas en `iroha_config::nexus`. | Entrada de estado + actualización de hoja de ruta marca DA-3 como TERMINAR. |

## Preguntas abiertas

1. **KZG vs Merkle defaults** - Doit-on siempre ignora los compromisos de KZG para
   les petits blobs afin de reduire la taille des blocs? Proposición: jardinero
   `kzg_commitment` opcional y gater a través de `iroha_config::da.enable_kzg`.
2. **Brechos en la secuencia** - ¿Autorise-t-on des lanes hors ordre? El plan actual rechazado
   les gaps sauf si la gouvernance active `allow_sequence_skips` para una repetición
   de urgencia.
3. **Caché de cliente ligero**: el SDK equipado requiere un caché SQLite legible para archivos
   pruebas; suivi en attente sous DA-8.Responder a estas preguntas en las relaciones públicas de implementación fera passer DA-3 de
BROUILLON (documento ce) a EN COURS des que le travail de code start.