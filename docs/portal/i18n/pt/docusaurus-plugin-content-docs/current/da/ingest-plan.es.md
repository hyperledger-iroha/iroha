---
lang: pt
direction: ltr
source: docs/portal/docs/da/ingest-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canonica
Refleja `docs/source/da/ingest_plan.md`. Mantenga ambas versiones en
:::

# Plan de ingesta de Data Availability de Sora Nexus

_Redactado: 2026-02-20 - Responsable: Core Protocol WG / Storage Team / DA WG_

El workstream DA-2 extiende Torii con un API de ingesta de blobs que emite
metadatos Norito y siembra la replicacion de SoraFS. Este documento captura el
esquema propuesto, la superficie de API y el flujo de validacion para que la
implementacion avance sin bloquearse por simulaciones pendientes (seguimientos
DA-1). Todos los formatos de payload DEBEN usar codecs Norito; no se permiten
fallbacks serde/JSON.

## Objetivos

- Aceptar blobs grandes (segmentos Taikai, sidecars de lane, artefactos de
  gobernanza) de forma determinista via Torii.
- Producir manifests Norito canonicos que describan el blob, parametros de
  codec, perfil de erasure y politica de retencion.
- Persistir metadata de chunks en almacenamiento hot de SoraFS y encolar jobs de
  replicacion.
- Publicar intents de pin + tags de politica al registry SoraFS y observadores
  de gobernanza.
- Exponer recibos de admision para que los clientes recuperen prueba determinista
  de publicacion.

## Superficie API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

El payload es un `DaIngestRequest` codificado en Norito. Las respuestas usan
`application/norito+v1` y devuelven `DaIngestReceipt`.

| Respuesta | Significado |
| --- | --- |
| 202 Accepted | Blob en cola para chunking/replicacion; se devuelve el recibo. |
| 400 Bad Request | Violacion de esquema/tamano (ver validaciones). |
| 401 Unauthorized | Token API ausente/invalido. |
| 409 Conflict | Duplicado `client_blob_id` con metadata no coincidente. |
| 413 Payload Too Large | Excede el limite configurado de longitud del blob. |
| 429 Too Many Requests | Se alcanzo el rate limit. |
| 500 Internal Error | Fallo inesperado (log + alerta). |

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
> payloads ahora viven bajo `iroha_data_model::da::types`, con wrappers de
> request/receipt en `iroha_data_model::da::ingest` y la estructura de manifest
> en `iroha_data_model::da::manifest`.

El campo `compression` declara como los callers prepararon el payload. Torii
acepta `identity`, `gzip`, `deflate`, y `zstd`, descomprimiendo los bytes antes de
hashear, chunking y verificar manifests opcionales.

### Checklist de validacion

1. Verificar que el header Norito de la solicitud coincide con `DaIngestRequest`.
2. Fallar si `total_size` difiere del largo canonico del payload (descomprimido)
   o excede el maximo configurado.
3. Forzar alineacion de `chunk_size` (potencia de dos, <= 2 MiB).
4. Asegurar `data_shards + parity_shards` <= maximo global y parity >= 2.
5. `retention_policy.required_replica_count` debe respetar la linea base de
   gobernanza.
6. Verificacion de firma contra hash canonico (excluyendo el campo signature).
7. Rechazar duplicado `client_blob_id` salvo que el hash del payload y la
   metadata sean identicos.
8. Cuando se provee `norito_manifest`, verificar esquema + hash coincide con el
   manifest recalculado tras el chunking; de lo contrario el nodo genera el
   manifest y lo almacena.
9. Forzar la politica de replicacion configurada: Torii reescribe el
   `RetentionPolicy` enviado con `torii.da_ingest.replication_policy` (ver
   `replication-policy.md`) y rechaza manifests preconstruidos cuya metadata de
   retencion no coincide con el perfil impuesto.

### Flujo de chunking y replicacion

1. Trocear el payload en `chunk_size`, calcular BLAKE3 por chunk + raiz Merkle.
2. Construir Norito `DaManifestV1` (struct nueva) capturando compromisos de
   chunk (role/group_id), layout de erasure (conteos de paridad de filas y
   columnas mas `ipa_commitment`), politica de retencion y metadata.
3. Encolar los bytes del manifest canonico bajo
   `config.da_ingest.manifest_store_dir` (Torii escribe archivos
   `manifest.encoded` por lane/epoch/sequence/ticket/fingerprint) para que la
   orquestacion de SoraFS los ingiera y vincule el storage ticket con datos
   persistidos.
4. Publicar intents de pin via `sorafs_car::PinIntent` con tag de gobernanza y
   politica.
5. Emitir evento Norito `DaIngestPublished` para notificar observadores (clientes
   ligeros, gobernanza, analitica).
6. Devolver `DaIngestReceipt` al caller (firmado por la clave de servicio DA de
   Torii) y emitir el header `Sora-PDP-Commitment` para que los SDKs capturen el
   commitment codificado de inmediato. El recibo ahora incluye `rent_quote`
   (un Norito `DaRentQuote`) y `stripe_layout`, permitiendo a los remitentes
   mostrar la renta base, la reserva, las expectativas de bonus PDP/PoTR y el
   layout de erasure 2D junto al storage ticket antes de comprometer fondos.

## Actualizaciones de almacenamiento/registry

- Extender `sorafs_manifest` con `DaManifestV1`, habilitando parseo
  determinista.
- Agregar nuevo stream de registry `da.pin_intent` con payload versionado que
  referencia hash de manifest + ticket id.
- Actualizar pipelines de observabilidad para seguir latencia de ingesta,
  throughput de chunking, backlog de replicacion y conteos de fallos.

## Estrategia de pruebas

- Tests unitarios para validacion de esquema, checks de firma y deteccion de
  duplicados.
- Tests golden verificando el encoding Norito de `DaIngestRequest`, manifest y
  receipt.
- Harness de integracion levantando SoraFS + registry simulados, validando
  flujos de chunk + pin.
- Tests de propiedades cubriendo perfiles de erasure y combinaciones de
  retencion aleatorias.
- Fuzzing de payloads Norito para proteger contra metadata malformada.

## Tooling de CLI & SDK (DA-8)

- `iroha app da submit` (nuevo entrypoint de CLI) ahora envuelve el builder/publisher
  compartido de ingesta para que operadores puedan ingerir blobs arbitrarios
  fuera del flujo Taikai bundle. El comando vive en
  `crates/iroha_cli/src/commands/da.rs:1` y consume un payload, perfil de
  erasure/retencion y archivos opcionales de metadata/manifest antes de firmar
  el `DaIngestRequest` canonico con la clave de config de la CLI. Las ejecuciones
  exitosas persisten `da_request.{norito,json}` y `da_receipt.{norito,json}` bajo
  `artifacts/da/submission_<timestamp>/` (override via `--artifact-dir`) para que
  los artefactos de release registren los bytes Norito exactos usados durante la
  ingesta.
- El comando usa por defecto `client_blob_id = blake3(payload)` pero acepta
  overrides via `--client-blob-id`, respeta mapas JSON de metadata
  (`--metadata-json`) y manifests pre-generados (`--manifest`), y soporta
  `--no-submit` para preparacion offline mas `--endpoint` para hosts Torii
  personalizados. El receipt JSON se imprime en stdout ademas de escribirse a
  disco, cerrando el requisito de tooling "submit_blob" de DA-8 y desbloqueando
  el trabajo de paridad de SDK.
- `iroha app da get` agrega un alias enfocado en DA para el orquestador multi-source
  que ya potencia `iroha app sorafs fetch`. Los operadores pueden apuntarlo a
  artefactos de manifest + chunk-plan (`--manifest`, `--plan`, `--manifest-id`)
  **o** pasar un storage ticket de Torii via `--storage-ticket`. Cuando se usa
  el path del ticket, la CLI baja el manifest desde `/v1/da/manifests/<ticket>`,
  persiste el bundle bajo `artifacts/da/fetch_<timestamp>/` (override con
  `--manifest-cache-dir`), deriva el hash del blob para `--manifest-id`, y luego
  ejecuta el orquestador con la lista `--gateway-provider` suministrada. Todos
  los knobs avanzados del fetcher de SoraFS permanecen intactos (manifest
  envelopes, labels de cliente, guard caches, overrides de transporte anonimo,
  export de scoreboard y paths `--output`), y el endpoint de manifest puede
  sobrescribirse via `--manifest-endpoint` para hosts Torii personalizados, asi
  que los checks end-to-end de availability viven totalmente bajo el namespace
  `da` sin duplicar logica del orquestador.
- `iroha app da get-blob` baja manifests canonicos directo desde Torii via
  `GET /v1/da/manifests/{storage_ticket}`. El comando escribe
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` y
  `chunk_plan_{ticket}.json` bajo `artifacts/da/fetch_<timestamp>/` (o un
  `--output-dir` provisto por el usuario) mientras imprime el comando exacto de
  `iroha app da get` (incluyendo `--manifest-id`) requerido para el fetch del
  orquestador. Esto mantiene a los operadores fuera de los directorios spool de
  manifests y garantiza que el fetcher siempre use los artefactos firmados
  emitidos por Torii. El cliente Torii de JavaScript refleja el mismo flujo via
  `ToriiClient.getDaManifest(storageTicketHex)`, devolviendo los bytes Norito
  decodificados, manifest JSON y chunk plan para que los callers de SDK hidraten
  sesiones del orquestador sin usar la CLI. El SDK de Swift ahora expone las
  mismas superficies (`ToriiClient.getDaManifestBundle(...)` mas
  `fetchDaPayloadViaGateway(...)`), canalizando bundles al wrapper nativo del
  orquestador SoraFS para que clientes iOS puedan descargar manifests, ejecutar
  fetches multi-source y capturar pruebas sin invocar la CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcula rentas deterministas y desglose de incentivos
  para un tamano de storage y ventana de retencion suministrados. El helper
  consume el `DaRentPolicyV1` activo (JSON o bytes Norito) o el default integrado,
  valida la politica e imprime un resumen JSON (`gib`, `months`, metadata de
  politica y campos de `DaRentQuote`) para que auditores citen cargos XOR exactos
  en actas de gobernanza sin scripts ad hoc. El comando tambien emite un resumen
  en una linea `rent_quote ...` antes del payload JSON para mantener legibles los
  logs de consola durante drills de incidentes. Empareje
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` con
  `--policy-label "governance ticket #..."` para persistir artefactos bonitos que
  citen el voto o bundle de config exactos; la CLI recorta el label personalizado
  y rechaza strings vacios para que los valores `policy_source` se mantengan
  accionables en dashboards de tesoreria. Ver
  `crates/iroha_cli/src/commands/da.rs` para el subcomando y
  `docs/source/da/rent_policy.md` para el esquema de politica.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadena todo lo anterior: toma un storage
  ticket, descarga el bundle canonico de manifest, ejecuta el orquestador
  multi-source (`iroha app sorafs fetch`) contra la lista `--gateway-provider`
  suministrada, persiste el payload descargado + scoreboard bajo
  `artifacts/da/prove_availability_<timestamp>/`, e invoca de inmediato el helper
  PoR existente (`iroha app da prove`) usando los bytes traidos. Los operadores
  pueden ajustar los knobs del orquestador (`--max-peers`, `--scoreboard-out`,
  overrides de endpoint de manifest) y el sampler de proof (`--sample-count`,
  `--leaf-index`, `--sample-seed`) mientras un solo comando produce los
  artefactos esperados por auditorias DA-5/DA-9: copia del payload, evidencia de
  scoreboard y resumenes de prueba JSON.
