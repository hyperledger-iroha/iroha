---
lang: es
direction: ltr
source: docs/portal/docs/da/commitments-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
ہونے تک دونوں ورژنز کو sincronización رکھیں۔
:::

# Sora Nexus Plan de compromisos de disponibilidad de datos (DA-3)

_مسودہ: 2026-03-25 -- مالکان: Core Protocol WG / Equipo de contrato inteligente / Equipo de almacenamiento_

DA-3 Nexus بلاک فارمیٹ کو وسعت دیتا ہے تاکہ ہر carril میں ایسے determinista
registros شامل ہوں جو DA-2 کے ذریعے قبول شدہ blobs کو بیان کریں۔ یہ نوٹ
estructuras de datos canónicas, bloquear ganchos de canalización, pruebas de cliente ligero, etc.
Torii/RPC superficies کو بیان کرتی ہے جو validadores کے admisión یا gobernanza
comprueba los compromisos de DA پر بھروسہ کرنے سے پہلے لازمی ہیں۔ cargas útiles
Norito codificado en ہیں؛ ESCALA یا ad-hoc JSON نہیں۔

## مقاصد

- Nexus incluye múltiples compromisos por blob (raíz de fragmento + hash de manifiesto + opcional
  Compromiso de KZG) شامل کرنا تاکہ pares almacenamiento fuera del libro mayor دیکھے بغیر
  estado de disponibilidad دوبارہ بنا سکیں۔
- pruebas de membresía deterministas فراہم کرنا تاکہ clientes ligeros verifican کر سکیں
  کہ hash de manifiesto کسی مخصوص بلاک میں finalizado ہوا تھا۔
- Consultas Torii (`/v1/da/commitments/*`) y pruebas de relés.
  SDK, automatización de gobernanza, reproducción, disponibilidad
  auditoría کر سکیں۔
- `SignedBlockWire` envolvente کو canonical رکھنا، نئی estructuras کو Norito
  encabezado de metadatos اور derivación de hash de bloque سے hilo کرتے ہوئے۔

## Descripción general del alcance1. **Adiciones al modelo de datos** `iroha_data_model::da::commitment` Bloque میں اور
   cambios de encabezado `iroha_data_model::block` میں۔
2. **Ganchos de ejecutor** تاکہ `iroha_core` Torii سے emitir شدہ DA recibos ingerir
   کرے (`crates/iroha_core/src/queue.rs` y `crates/iroha_core/src/block.rs`).
3. **Persistencia/índices** تاکہ Consultas de compromisos de WSV تیزی سے manejar کرے
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii Adiciones de RPC** enumerar/consultar/probar puntos finales کیلئے
   `/v1/da/commitments` کے تحت۔
5. **Pruebas de integración + accesorios**, diseño de cables, flujo de prueba y validación
   Número de modelo `integration_tests/tests/da/commitments.rs` Número de modelo

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

- `KzgCommitment` Memoria de 48 bytes de reutilización de memoria de 48 bytes para `iroha_crypto::kzg`
  میں ہے۔ جب ausente ہو تو صرف Pruebas de Merkle استعمال ہوتے ہیں۔
- `proof_scheme` catálogo de carriles سے deriva ہوتا ہے؛ Cargas útiles KZG de Merkle Lanes
  rechazar کرتی ہیں جبکہ `kzg_bls12_381` carriles distintos de cero compromisos KZG requieren
  کرتی ہیں۔ Torii فی الحال صرف Compromisos de Merkle بناتا ہے اور configurado con KZG
  carriles کو rechazar کرتا ہے۔
- `KzgCommitment` Memoria de 48 bytes de reutilización de memoria de 48 bytes y `iroha_crypto::kzg`
  میں ہے۔ جب Merkle carriles پر ausente ہو تو صرف Pruebas de Merkle استعمال ہوتے ہیں۔
- `proof_digest` Integración DA-5 PDP/PoTR کیلئے پیشگی جگہ دیتا ہے تاکہ اسی
  registrar el programa de muestreo میں درج ہو جو blobs کو زندہ رکھنے کیلئے استعمال
  ہوتا ہے۔

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
```Paquete hash بلاک hash اور `SignedBlockWire` metadatos دونوں میں شامل ہوتا ہے۔ جب
بچیں۔

Nota de implementación: `BlockPayload` اور transparente `BlockBuilder` اب
Los establecedores/captadores `da_commitments` exponen کرتے ہیں (دیکھیں
`BlockBuilder::set_da_commitments` o `SignedBlock::set_da_commitments`), respectivamente
hosts بلاک seal ہونے سے پہلے paquete prediseñado adjuntar کر سکیں۔ ayudante
campo de constructores کو `None` رکھتے ہیں جب تک Torii حقیقی paquetes de hilos نہ کرے۔

### 1.3 Codificación de cables

- `SignedBlockWire::canonical_wire()` موجودہ lista de transacciones کے فوراً بعد
  `DaCommitmentBundle` کیلئے Norito encabezado adjunto کرتا ہے۔ byte de versión `0x01` ہے۔
- `SignedBlockWire::decode_wire()` desconocido `version` y paquetes کو rechazar کرتا
  ہے، جیسا کہ `norito.md` میں Norito política بیان ہے۔
- Actualizaciones de derivación de hash صرف `block::Hasher` میں ہیں؛ formato de cable existente
  decodificar کرنے والے clientes ligeros خود بخود نیا campo حاصل کرتے ہیں کیونکہ
  Encabezado Norito اس کی موجودگی بتاتا ہے۔

## 2. Flujo de producción de bloques1. Torii DA ingesta ایک `DaIngestReceipt` finalizar کرتا ہے اور اسے cola interna
   پر publicar کرتا ہے (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` تمام recibos جمع کرتا ہے جن کا `lane_id` زیر تعمیر بلاک سے
   coincidir con کرتا ہے، اور `(lane_id, client_blob_id, manifest_hash)` پر deduplicar
   کرتا ہے۔
3. Sello کرنے سے پہلے compromisos del constructor کو `(lane_id, epoch, sequence)` سے
   ordenar کرتا ہے تاکہ hash determinista رہے، paquete کو Norito codec سے codificar
   کرتا ہے، اور `da_commitments_hash` actualización کرتا ہے۔
4. Paquete مکمل WSV میں store ہوتا ہے اور `SignedBlockWire` کے ساتھ بلاک میں
   emitir ہوتا ہے۔

اگر بلاک بنانا falla ہو تو cola de recibos میں رہتے ہیں تاکہ اگلی کوشش انہیں لے
سکے؛ constructor ہر carril کیلئے آخری شامل شدہ `sequence` ریکارڈ کرتا ہے تاکہ repetición
ataques سے بچا جا سکے۔

## 3. RPC y superficie de consulta

Torii تین puntos finales فراہم کرتا ہے:| Ruta | Método | Carga útil | Notas |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtro de rango de carril/época/secuencia, paginación) | `DaCommitmentPage` incluye un recuento total de compromisos y un hash de bloque. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (carril + hash de manifiesto یا tupla `(epoch, sequence)`) ۔ | `DaCommitmentProof` واپس کرتا ہے (registro + ruta Merkle + hash de bloque) ۔ |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Ayudante sin estado جو cálculo de hash de bloque دوبارہ کرتا ہے اور inclusión validar کرتا ہے؛ ایسے SDK کیلئے جو `iroha_crypto` سے براہ راست link نہیں کر سکتے۔ |

Cargas útiles de تمام `iroha_data_model::da::commitment` کے تحت ہیں۔ Enrutadores Torii
controladores کو موجودہ DA ingest endpoints کے ساتھ mount کرتے ہیں تاکہ token/mTLS
políticas de reutilización ہوں۔

## 4. Pruebas de inclusión y clientes ligeros

- Productor serializado `DaCommitmentRecord` lista de árbol binario Merkle
  ہے۔ اس کی raíz `da_commitments_hash` کو feed کرتی ہے۔
- `DaCommitmentProof` registro de destino کے ساتھ `(sibling_hash, position)` کا vector
  پیک کرتا ہے تاکہ verificadores raíz دوبارہ بنا سکیں۔ pruebas میں bloque hash اور
  encabezado firmado بھی شامل ہیں تاکہ clientes ligeros finalidad verificar کر سکیں۔
- CLI helpers (`iroha_cli app da prove-commitment`) solicitud de prueba/ciclo de verificación کو
  wrap کرتے ہیں اور operadores کیلئے Norito/salidas hexadecimales دیتے ہیں۔

## 5. Almacenamiento e indexaciónCompromisos de WSV کو familia de columnas dedicadas میں `manifest_hash` key کے ساتھ
tienda کرتا ہے۔ Índices secundarios `(lane_id, epoch)` y `(lane_id, sequence)`
کو portada کرتے ہیں تاکہ consultas پورے escaneo de paquetes نہ کریں۔ ہر registro اس bloque
altura کو pista کرتا ہے جس نے اسے sello کیا، جس سے nodos de recuperación bloquear registro سے
índice تیزی سے reconstruir کر سکتے ہیں۔

## 6. Telemetría y observabilidad

- `torii_da_commitments_total` تب incremento ہوتا ہے جب کوئی بلاک کم از کم ایک
  sello de registro کرے۔
- Paquete `torii_da_commitment_queue_depth` ہونے کے انتظار میں recibos کو
  pista کرتا ہے (por carril)۔
- Grafana tablero `dashboards/grafana/da_commitments.json` inclusión de bloque,
  profundidad de cola اور rendimiento de prueba دکھاتا ہے تاکہ Puertas de liberación DA-3
  auditoría de comportamiento کر سکیں۔

## 7. Estrategia de prueba

1. `DaCommitmentBundle` codificación/decodificación y actualizaciones de derivación de hash de bloque کیلئے
   **pruebas unitarias**۔
2. `fixtures/da/commitments/` میں **accesorios dorados** y bytes de paquete canónico
   اور Merkle pruebas capturan کرتے ہیں۔
3. **Pruebas de integración** Los validadores inician la ingesta de blobs de muestra
   کرتے ہیں، اور contenido del paquete اور respuestas de consulta/prueba پر اتفاق verificar کرتے
   ہیں۔
4. **Pruebas de cliente ligero** `integration_tests/tests/da/commitments.rs` (óxido)
   میں جو `/prove` call کر کے Torii سے بات کئے بغیر prueba verificar کرتے ہیں۔
5. **CLI smoke** script `scripts/da/check_commitments.sh` Herramientas del operador
   reproducible رہے۔

## 8. Plan de implementación| Fase | Descripción | Criterios de salida |
|-------|-------------|---------------|
| P0: fusión del modelo de datos | `DaCommitmentRecord`, actualizaciones de encabezado de bloque y códecs Norito aterrizan en کریں۔ | `cargo test -p iroha_data_model` نئی accesorios کے ساتھ verde۔ |
| P1 — Cableado central/WSV | cola + hilo lógico del generador de bloques los índices persisten y los controladores RPC exponen los índices | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` afirmaciones de prueba de paquete کے ساتھ pass۔ |
| P2 — Herramientas del operador | Ayudantes de CLI, panel de control Grafana, documentos de verificación de prueba enviados کریں۔ | `iroha_cli app da prove-commitment` devnet پر چلتا ہو؛ datos en vivo del panel de control دکھائے۔ |
| P3 — Puerta de gobernanza | `iroha_config::nexus` میں carriles marcados کیلئے Los compromisos DA requieren کرنے y el validador de bloque habilitado کریں۔ | entrada de estado + actualización de la hoja de ruta DA-3 کو marca completa کریں۔ |

## Preguntas abiertas

1. **Valores predeterminados de KZG frente a Merkle**: کیا چھوٹے blobs کیلئے compromisos de KZG ہمیشہ
   چھوڑ دئے جائیں تاکہ tamaño de bloque کم ہو؟ Versión: `kzg_commitment` opcional رکھیں
   اور `iroha_config::da.enable_kzg` کے ذریعے puerta کریں۔
2. **Brechos de secuencia** — کیا carriles fuera de servicio کی اجازت ہو؟ موجودہ پلان lagunas کو
   rechazar کرتا ہے جب تک gobernanza `allow_sequence_skips` کو repetición de emergencia کیلئے
   habilitar نہ کرے۔
3. **Caché de cliente ligero** — SDK con pruebas de caché SQLite
   ہے؛ DA-8 میں seguimiento باقی ہے۔ان سوالات کے جوابات implementación PRs میں دینے سے DA-3 اس مسودے سے نکل کر
"en progreso" کی حالت میں جائے گا جب código de trabajo شروع ہوگا۔