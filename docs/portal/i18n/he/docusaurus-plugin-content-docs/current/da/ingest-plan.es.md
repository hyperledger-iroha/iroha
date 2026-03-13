---
lang: he
direction: rtl
source: docs/portal/docs/da/ingest-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שימו לב פואנטה קנוניקה
Refleja `docs/source/da/ingest_plan.md`. Mantenga ambas versiones en
:::

# Plan de ingesta de Data Availability de Sora Nexus

_Redactado: 2026-02-20 - אחראי: Core Protocol WG / Storage Team / DA WG_

זרם העבודה DA-2 extiende Torii עם ה-API de ingesta de blobs que emite
metadatos Norito y siembra la replicacion de SoraFS. Este documento captura el
esquema propuesto, la superficie de API y el flujo de validacion para que la
implementacion avance sin bloquearse por simulaciones pendientes (seguimientos
DA-1). כל הפורמטים של מטען DEBEN משתמש קודקים Norito; אין לראות רשות
fallbacks serde/JSON.

## אובייקטיביות

- Aceptar blobs grandes (segmentos Taikai, cars sidecars de lane, artefactos de
  gobernanza) de forma determinista דרך Torii.
- מפיק מניפסט Norito canonicos que describan el blob, parametros de
  codec, פרופיל מחיקה ופוליטיקה של שימור.
- Persistir metadata de chunks en almacenamiento hot de SoraFS y encolar jobs de
  העתקה.
- כוונות פומביות של סיכה + תגיות פוליטיקה אל הרישום SoraFS y observadores
  דה גוברננסה.
- Exponer recibos de admision para que los clientes recuperen prueba determinista
  דה פרסום.

## Superficie API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

מטען המטען es un `DaIngestRequest` codificado en Norito. Las respuestas usan
`application/norito+v1` y devuelven `DaIngestReceipt`.

| תשובה | Significado |
| --- | --- |
| 202 מקובל | Blob en cola para chunking / replicacion; se devuelve el recibo. |
| 400 בקשה רעה | Violacion de esquema/tamano (ver validaciones). |
| 401 לא מורשה | Token API ausente/invalido. |
| 409 קונפליקט | Duplicado `client_blob_id` עם מטא נתונים לא מקרי. |
| 413 מטען גדול מדי | חריגה מהתצורה הגבולית של האורך של הכתם. |
| 429 יותר מדי בקשות | Se alcanzo el מגבלת תעריף. |
| 500 שגיאה פנימית | Fallo inesperado (יומן + התראה). |

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

> Nota de implementacion: las representaciones canonicas en Rust para estos
> מטענים ahora viven bajo `iroha_data_model::da::types`, con wrappers de
> בקשה/קבלה en `iroha_data_model::da::ingest` y la estructura de manifest
> en `iroha_data_model::da::manifest`.

El campo `compression` declara como los callers prepararon el payload. Torii
acepta `identity`, `gzip`, `deflate`, y `zstd`, descomprimiendo los bytes antes de
hashear, chunking y verificar manifestes optiones.

### רשימת אימות1. אימות הכותרת Norito de la solicitud עולה בקנה אחד עם `DaIngestRequest`.
2. Fallar si `total_size` הבדל של לארגו קנוניקו של מטען (descomprimido)
   o excede el maximo configurado.
3. Forzar alineacion de `chunk_size` (פוטנציה דה דוס, <= 2 MiB).
4. Asegurar `data_shards + parity_shards` <= maximo y global parity >= 2.
5. `retention_policy.required_replica_count` debe respetar la linea base de
   גוברננסה.
6. Verificacion de firma contra hash canonico (חתימה אקסקלוyendo el campo).
7. Rechazar duplicado `client_blob_id` salvo que el hash del payload y la
   metadata sean identicos.
8. Cuando se provee `norito_manifest`, אימות esquema + hash coincide con el
   manifest recalculado tras el chunking; de lo contrario el nodo genera el
   manifest y lo almacena.
9. Forzar la politica de replicacion configurada: Torii reescribe el
   `RetentionPolicy` enviado con `torii.da_ingest.replication_policy` (ver
   `replication-policy.md`) y rechaza manifests preconstruidos cuya metadata de
   החזרה אינה עולה בקנה אחד con el perfil impuesto.

### Flujo de chunking y replicacion

1. Trocear el payload en `chunk_size`, calcular BLAKE3 por chunk + raiz Merkle.
2. Construir Norito `DaManifestV1` (struct nueva) capturando compromisos de
   chunk (role/group_id), layout de erasure (conteos de paridad de filas y
   columnas mas `ipa_commitment`), שימור פוליטיקה ומטא נתונים.
3. Encolar los bytes del manifest canonico bajo
   `config.da_ingest.manifest_store_dir` (Torii escribe archivos
   `manifest.encoded` por lane/epoch/sequence/ticket/printed finger) para que la
   orquestacion de SoraFS los ingiera y vincule el כרטיס אחסון עם נתונים
   persistidos.
4. כוונות פרסום באמצעות `sorafs_car::PinIntent` עם תג דה גוברננזה y
   פוליטיקה.
5. Emitir evento Norito `DaIngestPublished` para notificar observadores (clientes)
   ligeros, gobernanza, analitica).
6. Devolver `DaIngestReceipt` אל המתקשר (firmado por la clave de servicio DA de
   Torii) y emitir el header `Sora-PDP-Commitment` para que los SDKs capturen el
   מחויבות codificado de inmediato. El recibo ahora incluye `rent_quote`
   (un Norito `DaRentQuote`) y `stripe_layout`, permitiendo a los remitentes
   mostrar la renta base, la reserva, las expectativas de bonus PDP/PoTR y el
   layout de erasure 2D Junto al כרטיס אחסון antes de comprometer fondos.

## אקטואליזציוני תקינה/רישום

- Extender `sorafs_manifest` עם `DaManifestV1`, habilitando parseo
  דטרמיניסטה.
- Agregar nuevo stream de registry `da.pin_intent` עם גרסה מטען נעילה
  referencia hash de manifest + מזהה כרטיס.
- צינורות אקטואליזציה של התבוננות עבור סגירת זמן לאחור,
  תפוקה של chunking, backlog de replicacion ו-conteos de fallos.

## אסטרטגיה דה פרובאס- בדיקות unitarios para validacion de esquema, checks de firma y deteccion de
  כפילויות.
- בודקת קידוד זהב verificando el Norito de `DaIngestRequest`, manifest y
  קבלה.
- רתום de integracion levantando SoraFS + סימולדות רישום, validando
  flujos de chunk + סיכה.
- מבחנים של פרופיידאס cubriendo perfiles de erasure y combinaciones de
  שימור aleatorias.
- Fuzzing de payloads Norito עבור מגן נגד מטא-נתונים.

## Tooling de CLI & SDK (DA-8)- `iroha app da submit` (נקודת כניסה של CLI) ahora envuelve el Builder/מוציא לאור
  compartido de ingesta para que operadores puedan ingerir blobs arbitrarios
  צרור טאיקאי פורה דל פלוג'ו. El comando vive en
  `crates/iroha_cli/src/commands/da.rs:1` יצרוך unloadload, perfil de
  מחיקה/שמירה וארכיון אופציונליות של מטא נתונים/מניפסט antes de firmar
  el `DaIngestRequest` canonico con la clave de config de la CLI. לאס ejecuciones
  exitosas persisten `da_request.{norito,json}` y `da_receipt.{norito,json}` באחו
  `artifacts/da/submission_<timestamp>/` (עקיפה דרך `--artifact-dir`)
  los artefactos de release registren los bytes Norito exactos usados durante la
  ingesta.
- El comando usa por defecto `client_blob_id = blake3(payload)` pero acepta
  עוקף דרך `--client-blob-id`, מפה חוזרת של JSON de metadata
  (`--metadata-json`) y manifests pre-generados (`--manifest`), y soporta
  `--no-submit` להכנה לא מקוונת mas `--endpoint` למארחים Torii
  התאמה אישית. El receipt JSON se imprime en stdout ademas de escribirse a
  disco, cerrando el requisito de tooling "submit_blob" de DA-8 y desbloqueando
  el trabajo de paridad de SDK.
- `iroha app da get` agrega un alias enfocado en DA para el orquestador multi-source
  que ya potencia `iroha app sorafs fetch`. Los operadores pueden apuntarlo a
  artefactos de manifest + תוכנית נתח (`--manifest`, `--plan`, `--manifest-id`)
  **o** pasar un כרטיס אחסון de Torii דרך `--storage-ticket`. קואנדו סה ארה"ב
  el path del ticket, la CLI baja el manifest desde `/v2/da/manifests/<ticket>`,
  persiste el bundle bajo `artifacts/da/fetch_<timestamp>/` (עקוף קו
  `--manifest-cache-dir`), deriva el hash del blob para `--manifest-id`, y luego
  ejecuta el orquestador con la list `--gateway-provider` suministrada. Todos
  los knobs avanzados del fetcher de SoraFS permanecen intactos (מניפסט
  מעטפות, תוויות לקוח, מטמוני שמירה, עוקפים את ה-transporte anonimo,
  ייצא את לוח התוצאות y paths `--output`), y el point end de manifest puede
  כתוב דרך `--manifest-endpoint` עבור מארח Torii התאמה אישית, אסי
  que los בודק את הזמינות מקצה לקצה viven totalmente bajo el namespace
  `da` sin duplicar logica del orquestador.
- `iroha app da get-blob` baja manifests canonicos directo desde Torii via
  `GET /v2/da/manifests/{storage_ticket}`. El comando escribe
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` y
  `chunk_plan_{ticket}.json` bajo `artifacts/da/fetch_<timestamp>/` (o un
  `--output-dir` provisto por el usuario) mientras imprime el comando exacto de
  `iroha app da get` (כולל `--manifest-id`) דרישה עבור אחזור דל
  orquestador. Esto mantiene a los operadores fura de los directorios spool de
  manifests y garantiza que el fetcher siempre להשתמש los artefactos firmados
  emitidos por Torii. אל לקוחות Torii de JavaScript refleja el mismo flujo via
  `ToriiClient.getDaManifest(storageTicketHex)`, devolviendo los bytes Norito
  decodificados, manifest JSON y chunk plan para que los callers de SDK hidraten
  sesiones del orquestador sin usar la CLI. El SDK de Swift ahora expone las
  משטחי מיסמס (`ToriiClient.getDaManifestBundle(...)` mas`fetchDaPayloadViaGateway(...)`), חבילות canalizando al wrapper nativo del
  orquestador SoraFS להורדת מניפסטים ל-iOS
  מביא ריבוי מקורות y capturar pruebas sin invocar la CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcula rentas deterministas y desglose de incentivos
  para un tamano de storage y ventana de retencion suministrados. אל עוזר
  צרוך את `DaRentPolicyV1` פעיל (JSON או בתים Norito) או אינטגראד ברירת מחדל,
  valida la politica e imprime un resumen JSON (`gib`, `months`, metadata de
  politica y campos de `DaRentQuote`) para que auditores citen cargos XOR exactos
  en actas de gobernanza sin scripts אד הוק. El comando tambien emite un resume
  en una linea `rent_quote ...` אנטס של מטען JSON למידע קריא
  logs de consola durante drills de incidentes. Empareje
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` con
  `--policy-label "governance ticket #..."` עבור פריטים מתמשכים
  citen el voto o bundle de config exactos; la CLI recorta el label personalizado
  y rechaza strings vacios para que los valores `policy_source` se mantengan
  מכשירי עזר ללוחות מחוונים. Ver
  `crates/iroha_cli/src/commands/da.rs` para el subcomando y
  `docs/source/da/rent_policy.md` para el esquema de politica.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadena todo lo anterior: toma un storage
  כרטיס, הורד את הצרור canonico de manifest, ejecuta el orquestador
  ריבוי מקורות (`iroha app sorafs fetch`) בניגוד לרשימה `--gateway-provider`
  suministrada, persiste el payload descargado + bajo לוח תוצאות
  `artifacts/da/prove_availability_<timestamp>/`, e invoca de inmediato el helper
  PoR existente (`iroha app da prove`) usando los bytes traidos. לוס מפעילים
  pueden ajustar los knobs del orquestador (`--max-peers`, `--scoreboard-out`,
  עוקף את נקודת הקצה של המניפסט) y el sampler de proof (`--sample-count`,
  `--leaf-index`, `--sample-seed`) מיינטרס או סולו קומנדו להפיק לוס
  artefactos esperados por auditorias DA-5/DA-9: copia del payload, evidencia de
  לוח תוצאות וקורות חיים של prueba JSON.