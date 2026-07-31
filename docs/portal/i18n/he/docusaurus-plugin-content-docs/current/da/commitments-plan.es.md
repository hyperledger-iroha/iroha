---
lang: he
direction: rtl
source: docs/portal/docs/da/commitments-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שימו לב פואנטה קנוניקה
Refleja `docs/source/da/commitments_plan.md`. Mantenga ambas versiones en
:::

# Plan de compromisos de Data Availability de Sora Nexus (DA-3)

_Redactado: 2026-03-25 -- אחראים: Core Protocol WG / צוות חוזה חכם / צוות אחסון_

DA-3 extiende el formato de bloque de Nexus לרישום קוד ליין
deterministas que describen los blobs aceptados por DA-2. Esta not captura las
estructuras de datas canonicas, los hooks del pipeline de bloques, las pruebas de
cliente ligero y las superficies Torii/RPC que deben aterrizar antes de que los
validadores puedan confiar en compromisos DA משך הקבלה או chequeos de
גוברננסה. Todos los payloads estan codificados en Norito; sin retired codec ni ad JSON
hoc.

## אובייקטיביות

- Llevar compromisos por blob (שורש נתח + חשיש מניפסט + התחייבות KZG
  אופציונלי) dentro de cada bloque Nexus para que los peers puedan reconstruir el
  estado de available sin consultar almacenamiento fura del book.
- Proeer pruebas de membresia deterministas para que clientes ligeros verifiquen
  que un manifest hash fue finalizado en un bloque dado.
- Exponer consultas Torii (`/v1/da/commitments/*`) y pruebas que permitan a
  ממסרים, SDKs y automatizacion de gobernanza זמינות אודיטור חטא משחזר
  קאדה בלוק.
- Mantener el envelope `SignedBlockWire` canonico al enhebrar las nuevas
  estructuras a traves del header de metadata Norito y la derivacion del hash de
  בלוק.

## פנורמה דה אלקנס

1. **Adiciones al modelo de datas** en `iroha_data_model::da::commitment` mas
   cambios de header de bloque en `iroha_data_model::block`.
2. **Hooks del executor** para que `iroha_core` ingeste receipts DA emitidos por
   Torii (`crates/iroha_core/src/queue.rs` y `crates/iroha_core/src/block.rs`).
3. **Persistencia/indexes** para que el WSV responda consultas de compromisos
   rapido (`iroha_core/src/wsv/mod.rs`).
4. **Adiciones RPC en Torii** עבור נקודות קצה של lista/consulta/prueba bajo
   `/v1/da/commitments`.
5. **בדיקות אינטגרציה + מתקנים** validando el wire layout y el flujo de
   הוכחה en `integration_tests/tests/da/commitments.rs`.

## 1. מידע על מודלים של נתונים

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // V1 lane policy (merkle_sha256 only)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- **V1 protocol invariant:** V1 contains only the `merkle_sha256` proof scheme. It has no KZG variant, commitment field, setup, generation, or verification path; unsupported configuration is rejected before node startup.
- A future KZG design requires a separately reviewed protocol version and an explicit wire-layout change. Hash expansion is not an elliptic-curve commitment.
- The verification endpoint checks commitment consistency against a caller-authenticated canonical block header and committed policy sidecar; it does not independently authenticate block signatures or finality.

### 1.2 הרחבה של הכותרת הראשית

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```El hash del bundle entra tanto en el hash del bloque como en la metadata de
`SignedBlockWire`. Cuando un bloque no lleva datos DA el campo permanece `None`

הערה ליישום: `BlockPayload` y el transparente `BlockBuilder` ahora
מגדירי/ג'טרים של exponen `da_commitments` (ver `BlockBuilder::set_da_commitments`
y `SignedBlock::set_da_commitments`), asi que los hosts pueden adjuntar un bundle
preconstruido antes de sellar un bloque. Todos los constructores helper dejan el
campo en `None` hasta que Torii enhebre bundles reales.

### 1.3 קידוד תיל

- `SignedBlockWire::canonical_wire()` agrega el header Norito para
  `DaCommitmentBundle` inmediatamente despues de la list de transacciones
  existente. El byte de version es `0x01`.
- `SignedBlockWire::decode_wire()` rechaza חבילות cuyo `version` es desconocido,
  siguiendo la politica Norito תיאור en `norito.md`.
- Las actualizaciones de derivacion de hash viven solo en `block::Hasher`; לוס
  clientes ligeros que decodifican el wire format existente ganan el nuevo campo
  Automaticamente porque el header Norito הודעה על ההצגה.

## 2. Flujo de produccion de bloques

1. La ingesta DA de Torii finaliza un `DaIngestReceipt` y lo publica en la cola
   פנימי (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` recopila todos los קבלות cuyo `lane_id` עולות בקנה אחד עם
   bloque en construccion, deduplicando por `(lane_id, client_blob_id,
   manifest_hash)`.
3. Justo antes de sellar, el Builder ordena los compromisos por `(lane_id,
   epoch, sequence)` para mantener el hash determinista, codifica el bundle con
   el codec Norito, y actualiza `da_commitments_hash`.
4. El bundle completo se almacena en el WSV y se emite junto al bloque dentro de
   `SignedBlockWire`.

Si la creacion del bloque falla, los קבלות permanecen en la cola para que el
suuiente intento los tome; registra el Builder el ultimo `sequence` כולל
por lane para evitar ataques de replay.

## 3. Superficie RPC y de consulta

Torii נקודות קצה של expone tres:

| רוטה | Metodo | מטען | Notas |
|------|--------|--------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (פילטר פור rango de lane/epoch/sequence, pagecion) | Devuelve `DaCommitmentPage` בסך הכל, פשרות ו-hash de bloque. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (מסלול + מניפסט hash o tupla `(epoch, sequence)`). | תגובה עם `DaCommitmentProof` (תקליט + רוטה מרקל + hash de bloque). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | עוזר חסר אזרחות que reejecuta el calculo del hash de bloque y valida הכללה; usado por SDKs que no pueden enlazar directo a `iroha_crypto`. |

Todos los payloads viven bajo `iroha_data_model::da::commitment`. לוס נתבים de
Torii montan los handlers junto a los endpoints de ingesta DA existentes para
שימוש חוזר בפוליטיקה של טוקן/mTLS.

## 4. Pruebas de inclusion y clientes ligeros- יוצר הבלוקים יוצר את המבנה של הרשימה
  serializada de `DaCommitmentRecord`. La raiz alimenta `da_commitments_hash`.
- `DaCommitmentProof` empaqueta el record objetivo mas un vector de
  `(sibling_hash, position)` para que los verificadores reconstruyan la raiz. לאס
  pruebas tambien incluyen el hash de bloque y el header firmado para que
  clientes ligeros לאמת את הסופיות.
- Helpers de CLI (`iroha_cli app da prove-commitment`) envuelven el ciclo de
  solicitud/verificacion de pruebas y exponen salidas Norito/hex para
  אופרדורים.

## 5. אחסון e indexacion

El WSV almacena compromisos en una family family dedicada con clave
`manifest_hash`. Los indexes secundarios cubren `(lane_id, epoch)` y
`(lane_id, sequence)` para que las consultas eviten escanear bundles completos.
Cada record rastrea la altura del bloque que lo sello, permitiendo a nodos en
catch-up reconstruir el indice rapidamente desde el block log.

## 6. Telemetria y Observabilidad

- `torii_da_commitments_total` אינקרמנטה cuando un bloque sella al menos un
  להקליט.
- `torii_da_commitment_queue_depth` קבלות רסטרה esperando ser empaquetados
  (שביל פור).
- לוח המחוונים Grafana `dashboards/grafana/da_commitments.json` visualiza la
  inclusion en bloques, profundidad de cola y throughput de pruebas para que
  los gates de release de DA-3 puedan auditar el comportamiento.

## 7. Estrategia de pruebas

1. **Future KZG protocol** — KZG is not part of V1. A separately reviewed later protocol version must specify polynomial encoding, setup provenance, commitment/opening algorithms, consensus verification, deterministic test vectors, and a versioned wire-layout change. V1 has no enable toggle or reserved accepted value.
2. **פערים ברצף** - Permitimos lanes fuera de orden? El plan rechaza בפועל
   gaps salvo que gobernanza active `allow_sequence_skips` להצגה חוזרת
   חירום.
3. **מטמון קל לקוח** - El equipo de SDK pidio un cache SQLite liviano para
   הוכחות; seguimiento pendiente bajo DA-8.

Responder estas preguntas en PRs de implementacion mueve DA-3 de BORRADOR (este
documento) a EN PROGRESO cuando el trabajo de codigo comience.