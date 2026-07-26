---
lang: es
direction: ltr
source: docs/portal/docs/da/ingest-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Refleja `docs/source/da/ingest_plan.md`. Mantenga ambas versiones en
:::

# Plan de ingesta de Data Availability de Sora Nexus

_Redactado: 2026-02-20 - Responsable: Core Protocol WG / Storage Team / DA WG_

El flujo de trabajo DA-2 extiende Torii con una API de ingesta de blobs que emite
metadatos Norito y siembra la replicacion de SoraFS. Este documento captura el
esquema propuesto, la superficie de API y el flujo de validación para que la
implementación avance sin bloquearse por simulaciones pendientes (seguimientos
DA-1). Todos los formatos de carga útil DEBEN usar codecs Norito; no se permiten
reservas serde/JSON.

## Objetivos

- Aceptar blobs grandes (segmentos Taikai, sidecars de lane, artefactos de
  gobernanza) de forma determinista vía Torii.
- Producir manifiestos Norito canónicos que describen el blob, parámetros de
  codec, perfil de borrado y política de retención.
- Persistir metadatos de chunks en almacenamiento hot de SoraFS y encolar jobs de
  replicación.
- Publicar intents de pin + tags de politica al registro SoraFS y observadores
  de gobernanza.
- Exponer recibos de admisión para que los clientes recuperen prueba determinista
  de publicación.

## Superficie API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```La carga útil es un `DaIngestRequest` codificado en Norito. Las respuestas usan
`application/norito+v1` y devuelven `DaIngestReceipt`.

| Respuesta | SIGNIFICADO |
| --- | --- |
| 202 Aceptado | Blob en cola para fragmentación/replicación; se devuelve el recibo. |
| 400 Solicitud incorrecta | Violación de esquema/tamano (ver validaciones). |
| 401 No autorizado | API de token ausente/inválido. |
| 409 Conflicto | Duplicado `client_blob_id` con metadatos no coincidentes. |
| 413 Carga útil demasiado grande | Exceda el límite configurado de longitud del blob. |
| 429 Demasiadas solicitudes | Se alcanzo el límite de tasa. |
| 500 Error interno | Fallo inesperado (log + alerta). |

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

> Nota de implementación: las representaciones canónicas en Rust para estos
> payloads ahora viven bajo `iroha_data_model::da::types`, con wrappers de
> solicitud/recibo en `iroha_data_model::da::ingest` y la estructura de manifiesto
> es `iroha_data_model::da::manifest`.

El campo `compression` declara como los llamadores prepararon la carga útil. Torii
acepta `identity`, `gzip`, `deflate`, y `zstd`, descomprimiendo los bytes antes de
hashear, fragmentación y verificar manifiestos opcionales.

### Lista de verificación de validación1. Verifique que el encabezado Norito de la solicitud coincida con `DaIngestRequest`.
2. Fallar si `total_size` difiere del largo canonico del payload (descomprimido)
   o excede el máximo configurado.
3. Forzar alineacion de `chunk_size` (potencia de dos, = 2.
5. `retention_policy.required_replica_count` debe respetar la línea base de
   gobierno.
6. Verificación de firma contra hash canonico (excluyendo la firma del campo).
7. Rechazar duplicado `client_blob_id` salvo que el hash del payload y la
   Los metadatos son idénticos.
8. Cuando se proporciona `norito_manifest`, verificar esquema + hash coincide con el
   manifiesto recalculado tras el fragmentación; de lo contrario el nodo genera el
   manifest y lo almacena.
9. Forzar la política de replicación configurada: Torii reescribe el
   `RetentionPolicy` enviado con `torii.da_ingest.replication_policy` (ver
   `replication-policy.md`) y rechaza manifests preconstruidos cuyos metadata de
   la retención no coincide con el perfil impuesto.

### Flujo de fragmentación y replicación1. Trocear la carga útil en `chunk_size`, calcular BLAKE3 por chunk + raiz Merkle.
2. Construir Norito `DaManifestV1` (struct nueva) capturando compromisos de
   chunk (role/group_id), diseño de borrado (conteos de paridad de filas y
   columnas mas `ipa_commitment`), política de retención y metadatos.
3. Encolar los bytes del manifiesto canonico bajo
   `config.da_ingest.manifest_store_dir` (Torii escribe archivos
   `manifest.encoded` por lane/epoch/sequence/ticket/fingerprint) para que la
   orquestacion de SoraFS los ingiera y vincule el ticket de almacenamiento con datos
   persistidos.
4. Publicar intents de pin via `sorafs_car::PinIntent` con tag de gobernanza y
   política.
5. Emitir evento Norito `DaIngestPublished` para notificar observadores (clientes
   ligeros, gobernanza, analítica).
6. Devolver `DaIngestReceipt` al llamante (firmado por la clave de servicio DA de
   Torii) y emite el encabezado `Sora-PDP-Commitment` para que los SDK capturen el
   compromiso codificado de inmediato. El recibo ahora incluye `rent_quote`
   (un Norito `DaRentQuote`) y `stripe_layout`, permitiendo a los remitentes
   mostrar la base de alquiler, la reserva, las expectativas de bono PDP/PoTR y el
   diseño de borrado 2D junto al ticket de almacenamiento antes de comprometer fondos.

## Actualizaciones de almacenamiento/registro- Extender `sorafs_manifest` con `DaManifestV1`, habilitando parseo
  determinista.
- Agregar nuevo flujo de registro `da.pin_intent` con carga útil versionado que
  referencia hash de manifiesto + ID del ticket.
- Actualizar pipelines de observabilidad para seguir latencia de ingesta,
  throughput de fragmentación, backlog de replicacion y recuentos de fallos.

## Estrategia de pruebas

- Pruebas unitarias para validación de esquema, comprobaciones de firma y detección de
  duplicados.
- Pruebas golden verificando el encoding Norito de `DaIngestRequest`, manifest y
  recibo.
- Arnés de integracion levantando SoraFS + registro simulados, validando
  flujos de trozo + pin.
- Tests de propiedades cubriendo perfiles de borrado y combinaciones de
  retención aleatoria.
- Fuzzing de payloads Norito para proteger contra metadatos malformados.

## Herramientas de CLI y SDK (DA-8)- `iroha app da submit` (nuevo punto de entrada de CLI) ahora envuelve el builder/publisher
  compartido de ingesta para que los operadores puedan ingerir blobs arbitrarios
  fuera del flujo Paquete Taikai. El comando vive en
  `crates/iroha_cli/src/commands/da.rs:1` y consume una carga útil, perfil de
  borrado/retencion y archivos opcionales de metadata/manifiesto antes de firmar
  el `DaIngestRequest` canónico con la clave de configuración de la CLI. Las ejecuciones
  exitosas persisten `da_request.{norito,json}` y `da_receipt.{norito,json}` bajo
  `artifacts/da/submission_<timestamp>/` (anulación mediante `--artifact-dir`) para que
  los artefactos de liberación registran los bytes Norito exactos usados durante la
  ingesta.
- El comando usa por defecto `client_blob_id = blake3(payload)` pero acepta
  anula a través de `--client-blob-id`, respeta mapas JSON de metadatos
  (`--metadata-json`) y manifests pregenerados (`--manifest`), y soporta
  `--no-submit` para preparación fuera de línea más `--endpoint` para hosts Torii
  personalizados. El recibo JSON se imprime en stdout además de escribirse a
  disco, cerrando el requisito de herramienta "submit_blob" de DA-8 y desbloqueando
  el trabajo de paridad de SDK.
- `iroha app da get` agrega un alias enfocado en DA para el orquestador multi-source
  que ya potencia `iroha app sorafs fetch`. Los operadores pueden apuntarlo a
  artefactos de manifiesto + plan de fragmentos (`--manifest`, `--plan`, `--manifest-id`)**o** pasar un ticket de almacenamiento de Torii vía `--storage-ticket`. Cuando se usa
  el path del ticket, la CLI baja el manifest desde `/v1/da/manifests/<ticket>`,
  persistir el paquete bajo `artifacts/da/fetch_<timestamp>/` (anular con
  `--manifest-cache-dir`), deriva el hash del blob para `--manifest-id`, y luego
  ejecuta el orquestador con la lista `--gateway-provider` suministrada. Todos
  los botones avanzados del fetcher de SoraFS permanecen intactos (manifiesto
  sobres, etiquetas de cliente, cachés de guardia, anulaciones de transporte anónimo,
  export de scoreboard y paths `--output`), y el endpoint de manifest puede
  sobrescribirse vía `--manifest-endpoint` para hosts Torii personalizados, así
  que los cheques de disponibilidad de extremo a extremo viven totalmente bajo el espacio de nombres
  `da` sin duplicar lógica del orquestador.
- `iroha app da get-blob` baja manifiesta canonicos directo desde Torii vía
  `GET /v1/da/manifests/{storage_ticket}`. El comando escribe
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` y
  `chunk_plan_{ticket}.json` bajo `artifacts/da/fetch_<timestamp>/` (o un
  `--output-dir` provisto por el usuario) mientras imprime el comando exacto de
  `iroha app da get` (incluido `--manifest-id`) requerido para el fetch del
  orquestador. Esto mantiene a los operadores fuera de los directorios spool de
  manifiesta y garantiza que el fetcher siempre utiliza los artefactos firmadosemitidos por Torii. El cliente Torii de JavaScript refleja el mismo flujo vía
  `ToriiClient.getDaManifest(storageTicketHex)`, devolviendo los bytes Norito
  decodificados, manifest JSON y fragment plan para que los llamadores de SDK se hidraten
  sesiones del orquestador sin usar la CLI. El SDK de Swift ahora exponen las
  mismas superficies (`ToriiClient.getDaManifestBundle(...)` mas
  `fetchDaPayloadViaGateway(...)`), canalizando paquetes al wrapper nativo del
  orquestador SoraFS para que clientes iOS puedan descargar manifests, ejecutar
  recupera múltiples fuentes y captura pruebas sin invocar la CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcula rentas deterministas y desglose de incentivos
  para un tamaño de almacenamiento y ventana de retención suministrados. El ayudante
  consume el `DaRentPolicyV1` activo (JSON o bytes Norito) o el predeterminado integrado,
  valida la política e imprime un resumen JSON (`gib`, `months`, metadata de
  politica y campos de `DaRentQuote`) para que auditores citen cargos XOR exactos
  en actas de gobernanza sin guiones ad hoc. El comando también emite un resumen.
  en una linea `rent_quote ...` antes del payload JSON para mantener legibles los
  logs de consola durante simulacros de incidentes. empareje
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` estafa
  `--policy-label "governance ticket #..."` para persistir artefactos bonitos queciten el voto o paquete de configuraciones exactas; la CLI recorta la etiqueta personalizada
  y rechaza strings vacios para que los valores `policy_source` se mantengan
  accionables en tableros de tesoreria. Ver
  `crates/iroha_cli/src/commands/da.rs` para el subcomando y
  `docs/source/da/rent_policy.md` para el esquema de política.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadena todo lo anterior: toma un almacenamiento
  ticket, descarga el paquete canónico de manifest, ejecuta el orquestador
  fuente múltiple (`iroha app sorafs fetch`) contra la lista `--gateway-provider`
  suministrada, persiste el payload descargado + marcador bajo
  `artifacts/da/prove_availability_<timestamp>/`, e invoca de inmediato el ayudante
  PoR existente (`iroha app da prove`) usando los bytes traidos. Los operadores
  pueden ajustar las perillas del orquestador (`--max-peers`, `--scoreboard-out`,
  anula el punto final de manifiesto) y el muestreador de prueba (`--sample-count`,
  `--leaf-index`, `--sample-seed`) mientras un solo comando produce los
  artefactos esperados por auditorias DA-5/DA-9: copia del payload, evidencia de
  marcador y resúmenes de prueba JSON.