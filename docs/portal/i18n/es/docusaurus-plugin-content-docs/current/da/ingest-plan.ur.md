---
lang: es
direction: ltr
source: docs/portal/docs/da/ingest-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
ہونے تک دونوں ورژنز کو sincronización رکھیں۔
:::

# Sora Nexus Plan de ingesta de disponibilidad de datos

_مسودہ: 2026-02-20 -- مالک: Core Protocol WG / Equipo de almacenamiento / DA WG_

DA-2 ورک اسٹریم Torii میں ایک blob ingest API شامل کرتا ہے جو Norito میٹاڈیٹا
جاری کرتی ہے اور SoraFS ریپلیکیشن کو semilla کرتی ہے۔ یہ دستاویز مجوزہ esquema،
Superficie API, flujo de validación y implementación
simulaciones (seguimientos de DA-1) پر رکے بغیر آگے بڑھے۔ تمام formatos de carga útil کو
Códecs Norito استعمال کرنا لازمی ہے؛ reserva de serde/JSON کی اجازت نہیں۔

## اہداف

- بڑے blobs (segmentos Taikai, sidecars de carril, artefactos de gobernanza) کو Torii کے
  ذریعے deterministamente قبول کرنا۔
- blob, parámetros de códec, perfil de borrado, política de retención کو بیان کرنے
  Y manifiestos canónicos Norito تیار کرنا۔
- fragmentos de metadatos کو SoraFS almacenamiento en caliente میں محفوظ کرنا اور trabajos de replicación کو
  poner en cola کرنا۔
- Intenciones de pin + etiquetas de políticas, registro SoraFS y observadores de gobernanza.
  publicar کرنا۔
- recibos de admisión فراہم کرنا تاکہ clientes کو prueba determinista de
  publicación مل سکے۔

## Superficie API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Carga útil ایک Norito codificado `DaIngestRequest` ہے۔ Respuestas `application/norito+v1`
استعمال کرتی ہیں اور `DaIngestReceipt` واپس کرتی ہیں۔| Respuesta | مطلب |
| --- | --- |
| 202 Aceptado | Blob کو fragmentación/replicación کیلئے cola کیا گیا؛ recibo واپس۔ |
| 400 Solicitud incorrecta | Violación de esquema/tamaño (verificaciones de validación دیکھیں)۔ |
| 401 No autorizado | Token API موجود نہیں/غلط۔ |
| 409 Conflicto | `client_blob_id` ڈپلیکیٹ ہے اور metadatos مختلف ہے۔ |
| 413 Carga útil demasiado grande | Límite de longitud del blob configurado سے تجاوز۔ |
| 429 Demasiadas solicitudes | Límite de tasa alcanzado۔ |
| 500 Error interno | غیر متوقع falla (registro + alerta) ۔ |

## Esquema Norito propuesto

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> Nota de implementación: cargas útiles y representaciones canónicas de Rust.
> `iroha_data_model::da::types` کے تحت ہیں، envoltorios de solicitud/recibo
> `iroha_data_model::da::ingest` میں، اور estructura manifiesta
> `iroha_data_model::da::manifest` میں ہے۔

Campo `compression` بتاتا ہے کہ llamantes نے carga útil کیسے تیار کیا۔ Torii
`identity`, `gzip`, `deflate`, y `zstd` son una combinación de hash, fragmentación y
manifiestos opcionales verificar کرنے سے پہلے bytes کو descomprimir کرتا ہے۔

### Lista de verificación de validación1. Verifique la solicitud کریں کہ کا Norito encabezado `DaIngestRequest` سے coincide کرتا ہے۔
2. Longitud de carga útil canónica (descomprimida) `total_size` سے مختلف ہو یا
   configurado máximo سے زیادہ ہو تو fallar کریں۔
3. La alineación `chunk_size` aplica کریں (potencia de dos, = 2 یقینی بنائیں۔
5. `retention_policy.required_replica_count` Respeto de línea base de gobernanza کرنا ہوگا۔
6. Hash canónico کے خلاف verificación de firma (campo de firma کے بغیر)۔
7. Los duplicados `client_blob_id` rechazan el hash de carga útil + metadatos idénticos a کریں جب تک
8. اگر `norito_manifest` دیا گیا ہو تو esquema + hash کو fragmentación کے بعد
   manifiesto recalculado سے coincidencia کریں؛ ورنہ manifiesto de nodo generar کر کے almacenar کرے۔
9. Aplicar la política de replicación configurada کریں: Torii `RetentionPolicy` کو
   `torii.da_ingest.replication_policy` سے reescribir کرتا ہے ( `replication-policy.md`
   دیکھیں ) اور manifiestos prediseñados کو rechazar کرتا ہے اگر metadatos de retención
   perfil forzado سے coincidencia نہ کرے۔

### Flujo de fragmentación y replicación1. Carga útil `chunk_size` میں تقسیم کریں، ہر chunk پر BLAKE3 اور Merkle root حساب کریں۔
2. Norito `DaManifestV1` (estructura) para compromisos de fragmentos (rol/group_id) ,
   diseño de borrado (recuentos de paridad de filas/columnas + `ipa_commitment`), política de retención,
   اور metadatos کو captura کرے۔
3. Bytes de manifiesto canónico کو `config.da_ingest.manifest_store_dir` کے تحت cola کریں
   (Torii `manifest.encoded` archivos carril/época/secuencia/ticket/huella digital کے لحاظ سے لکھتا ہے)
   تاکہ SoraFS orquestación انہیں ingesta کر کے ticket de almacenamiento کو datos persistentes سے enlace کرے۔
4. `sorafs_car::PinIntent` کے ذریعے etiqueta de gobernanza + política کے ساتھ pin intents publicar کریں۔
5. El evento Norito `DaIngestPublished` emite observadores کریں تاکہ (clientes ligeros, gobernanza, análisis)
   کو اطلاع ملے۔
6. Llamante `DaIngestReceipt` کو واپس دیں (Torii Clave de servicio DA سے firmada) اور
   El encabezado `Sora-PDP-Commitment` emite SDK de compromiso de captura de compromiso
   Recibo میں اب `rent_quote` (Norito `DaRentQuote`) اور `stripe_layout` شامل ہیں،
   جس سے alquiler base de los remitentes, participación de reserva, expectativas de bonificación PDP/PoTR اور 2D
   diseño de borrado کو ticket de almacenamiento کے ساتھ compromiso de fondos کرنے سے پہلے دکھا سکتے ہیں۔

## Actualizaciones de almacenamiento/registro- `sorafs_manifest` کو `DaManifestV1` کے ساتھ extender کریں تاکہ análisis determinista ہو سکے۔
- Flujo de registro actual `da.pin_intent` شامل کریں جس کا carga útil versionada
  hash de manifiesto + ID del ticket کو referencia کرتا ہے۔
- Canalizaciones de observabilidad, latencia de ingesta, rendimiento fragmentado, acumulación de replicación
  اور el fracaso cuenta ٹریک کرنے کیلئے اپ ڈیٹ کریں۔

## Estrategia de prueba

- Validación de esquemas, comprobaciones de firmas, detección de duplicados y pruebas unitarias.
- Codificación Norito کے pruebas doradas (`DaIngestRequest`, manifiesto, recibo) ۔
- Arnés de integración, simulacro SoraFS + registro چلاتا ہے اور trozo + verificación de flujos de pines چلاتا ہے۔
- Pruebas de propiedad, perfiles de borrado aleatorios y combinaciones de retención cubren کرتے ہیں۔
- Fuzzing de carga útil Norito Metadatos mal formados سے بچاؤ ہو۔

## Herramientas CLI y SDK (DA-8)- `iroha app da submit` (punto de entrada CLI) es un editor/generador de ingesta compartida کو wrap کرتا ہے تاکہ
  operadores Flujo de paquete Taikai کے باہر ingesta de blobs arbitrarios کر سکیں۔ یہ کمانڈ
  `crates/iroha_cli/src/commands/da.rs:1` میں ہے اور payload, borrado/perfil de retención اور opcional
  archivos de metadatos/manifiesto لے کر Clave de configuración CLI کے ساتھ signo canónico `DaIngestRequest` کرتی ہے۔
  El sistema ejecuta `da_request.{norito,json}` y `da_receipt.{norito,json}`.
  `artifacts/da/submission_<timestamp>/` کے تحت محفوظ کرتے ہیں (anulación mediante `--artifact-dir`) تاکہ
  liberar artefactos ingerir میں استعمال ہونے والے registro exacto de bytes Norito کریں۔
- کمانڈ predeterminado میں `client_blob_id = blake3(payload)` استعمال کرتی ہے مگر `--client-blob-id` anula,
  Mapas JSON de metadatos (`--metadata-json`) y manifiestos pregenerados (`--manifest`) y aceptar archivos.
  اور `--no-submit` (preparación sin conexión) اور `--endpoint` (hosts Torii personalizados) سپورٹ کرتی ہے۔ Recibo JSON
  stdout پر print ہوتا ہے اور disk پر بھی لکھا جاتا ہے، جس سے DA-8 کا requisito "submit_blob" پورا ہوتا ہے
  اور Desbloqueo del trabajo de paridad del SDK ہوتا ہے۔
- `iroha app da get` Alias centrado en DA فراہم کرتا ہے جو multi-source Orchestrator کو use کرتا ہے جو پہلے ہی
  `iroha app sorafs fetch` کو چلاتا ہے۔ Manifiesto de operadores + artefactos de plan de fragmentos (`--manifest`, `--plan`, `--manifest-id`)
  **یا** Ticket de almacenamiento Torii a través de `--storage-ticket` دے سکتے ہیں۔ Ruta del billete پر CLI `/v1/da/manifests/<ticket>` سےdescarga de manifiesto کرتی ہے، paquete کو `artifacts/da/fetch_<timestamp>/` میں محفوظ کرتی ہے (anulación mediante
  `--manifest-cache-dir`), `--manifest-id` کیلئے blob hash derivar کرتی ہے، اور فراہم کردہ Lista `--gateway-provider`
  کے ساتھ ejecución del orquestador کرتی ہے۔ SoraFS buscador کے perillas avanzadas برقرار رہتے ہیں (sobres manifiestos,
  etiquetas de cliente, cachés de protección, anulaciones de transporte de anonimato, exportación de marcadores (o rutas `--output`) o
  `--manifest-endpoint` Anulación del punto final del manifiesto کیا جا سکتا ہے، لہذا verificaciones de disponibilidad de un extremo a otro
  مکمل طور پر `da` espacio de nombres میں رہتے ہیں بغیر lógica del orquestador duplicado کئے۔
- `iroha app da get-blob` Torii سے `GET /v1/da/manifests/{storage_ticket}` کے ذریعے manifiestos canónicos کھینچتا ہے۔
  Número de modelo `manifest_{ticket}.norito`, `manifest_{ticket}.json`, y número `chunk_plan_{ticket}.json`
  `artifacts/da/fetch_<timestamp>/` میں لکھتی ہے (یا `--output-dir` proporcionado por el usuario) اور عین `iroha app da get`
  invocación (بشمول `--manifest-id`) echo کرتی ہے جو seguimiento orquestador buscar کیلئے درکار ہے۔ اس سے operadores
  directorios de carrete de manifiesto سے دور رہتے ہیں اور fetcher ہمیشہ Torii کے artefactos firmados استعمال کرتا ہے۔
  JavaScript Torii cliente یہی flujo `ToriiClient.getDaManifest(storageTicketHex)` کے ذریعے دیتا ہے، اور decodificado
  Norito bytes, manifiesto JSON, plan de fragmentos, y llamadas de SDK, CLI y sesiones de orquestador hidratadas
  کر سکیں۔ Las superficies Swift SDK بھی وہی exponen کرتا ہے (`ToriiClient.getDaManifestBundle(...)` اور`fetchDaPayloadViaGateway(...)`), paquetes y envoltorio de orquestador nativo SoraFS, y tuberías y clientes iOS.
  descarga de manifiestos کرنے، recuperaciones de múltiples fuentes چلانے اور captura de pruebas کرنے دیتا ہے بغیر CLI کے۔
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` دیے گئے tamaño de almacenamiento اور ventana de retención کیلئے alquiler determinista اور desglose de incentivos
  calcular کرتا ہے۔ یہ ayudante activo `DaRentPolicyV1` (JSON یا Norito bytes) یا valor predeterminado incorporado استعمال کرتا ہے،
  validar política کرتا ہے، اور Resumen JSON (`gib`, `months`, metadatos de política, campos `DaRentQuote`) imprimir کرتا ہے
  تاکہ actas de gobierno de los auditores میں cargos XOR exactos citan کر سکیں بغیر scripts ad hoc کے۔ Carga útil JSON
  سے پہلے ایک لائن `rent_quote ...` resumen بھی دیتی ہے تاکہ registros de consola legibles رہیں۔
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` کو `--policy-label "governance ticket #..."` کے ساتھ
  جوڑیں تاکہ artefactos embellecidos محفوظ ہوں جو درست votación de políticas یا paquete de configuración citar کریں؛ Etiqueta personalizada CLI کو recortar
  کرتی ہے اور خالی cadenas rechazadas کرتی ہے تاکہ `policy_source` اقدار panel میں procesable رہیں۔ دیکھیں
  `crates/iroha_cli/src/commands/da.rs` (subcomando) o `docs/source/da/rent_policy.md` (esquema de política) ۔
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` اوپر کی سب چیزیں cadena کرتا ہے: یہ ticket de almacenamiento لیتا ہے، paquete de manifiesto canónicodescargar کرتا ہے، orquestador de fuentes múltiples (`iroha app sorafs fetch`) کو فراہم کردہ `--gateway-provider` lista کے خلاف
  چلاتا ہے، carga útil descargada + marcador کو `artifacts/da/prove_availability_<timestamp>/` میں محفوظ کرتا ہے، اور
  فوری طور پر موجودہ PoR helper (`iroha app da prove`) کو bytes recuperados کے ساتھ invocar کرتا ہے۔ Perillas del orquestador de operadores
  (`--max-peers`, `--scoreboard-out`, anulaciones de punto final de manifiesto) y muestra de prueba (`--sample-count`, `--leaf-index`,
  `--sample-seed`) ajuste کر سکتے ہیں جبکہ ایک ہی comando DA-5/DA-9 auditorías کیلئے متوقع artefactos پیدا کرتی ہے:
  copia de carga útil, evidencia del marcador, o resúmenes de prueba JSON۔