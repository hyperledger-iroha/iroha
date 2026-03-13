---
lang: es
direction: ltr
source: docs/portal/docs/da/ingest-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Reflejo `docs/source/da/ingest_plan.md`. Gardez les dos versiones en sincronización tant
:::

# Plan de ingesta Disponibilidad de datos Sora Nexus

_Redige: 2026-02-20 - Responsables: Core Protocol WG / Storage Team / DA WG_

El flujo de trabajo DA-2 incluye Torii con una API de ingesta de blobs que emet des
metadonnees Norito y amorce la replicación SoraFS. Archivo de captura de documentos ce
propuesta de esquema, la API de superficie y el flujo de validación afin que
La implementación avanza sin bloquear las simulaciones restantes (suivi
DA-1). Todos los formatos de carga útil DOIVENT utilizan los códecs Norito; aucún
El servidor alternativo/JSON no está permitido.

## Objetivos

- Aceptador de blobs volumineux (segmentos Taikai, sidecars de lane, artefactos de
  gobernanza) de manera determinista vía Torii.
- Produire des manifests Norito canoniques decrivant le blob, les parametres de
  codec, el perfil de borrado y la política de retención.
- Persister los metadatos de fragmentos en el almacenamiento en caliente de SoraFS y mettre en
  archivo de trabajos de replicación.
- Publicar las intenciones de pin + etiquetas políticas en el registro SoraFS y las
  observadores de gobierno.
- Exponer los recibos de admisión para que los clientes retrouvent une preuve
  determinante de publicación.

## API de superficie (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```La carga útil es una codificación `DaIngestRequest` en Norito. Las respuestas útiles
`application/norito+v1` y envíe `DaIngestReceipt`.

| Respuesta | Significado |
| --- | --- |
| 202 Aceptado | Blob y archivo para fragmentación/replicación; recibo renvoye. |
| 400 Solicitud incorrecta | Violación de esquema/talla (ver controles de validación). |
| 401 No autorizado | Cantidad de API de token/inválida. |
| 409 Conflicto | Doublon `client_blob_id` con metadatos no idénticos. |
| 413 Carga útil demasiado grande | Salga del límite de configuración de longitud del blob. |
| 429 Demasiadas solicitudes | Atención de límite de tarifa. |
| 500 Error interno | Echec inattendu (log + alerta). |

## Esquema Norito proponer

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

> Nota de implementación: representaciones canónicas de Rust para estas cargas útiles
> vivent maintenant sous `iroha_data_model::da::types`, con envoltorios
> solicitud/recibo en `iroha_data_model::da::ingest` et la estructura de
> manifiesto en `iroha_data_model::da::manifest`.

El campeón `compression` anuncia que las personas que llaman no pueden preparar la carga útil. Torii
Acepta `identity`, `gzip`, `deflate` y `zstd`, y descomprime los bytes antes
El hash, el fragmentación y las opciones de verificación de manifiestos.

### Lista de verificación de validación1. Verifique que el encabezado Norito de la solicitud corresponda a `DaIngestRequest`.
2. Echouer si `total_size` difiere de la longitud canónica de la carga útil
   (descomprimir) o desactivar la configuración máxima.
3. Forzar la alineación de `chunk_size` (potencia de dos, = 2.
5. `retention_policy.required_replica_count` haga respetar la línea base de
   gobernanza.
6. Verificación de firma contre le hash canonique (excluant le champ
   firma).
7. Rechazar un `client_blob_id` duplicado salvo el hash de la carga útil y los metadatos
   sont identiques.
8. Cuando `norito_manifest` está disponible, verificador de esquema + correspondiente a hash
   au manifiesto recalcular después de la fragmentación; sinon le noeud genere le manifest et le
   stocke.
9. Aplique la política de replicación configurada: Torii vuelva a escribir el archivo
   `RetentionPolicy` junto con `torii.da_ingest.replication_policy` (ver
   `replication-policy.md`) et rejette les manifests preconstruits dont la
   Los metadatos de retención no corresponden al perfil impuesto.

### Flujo de fragmentación y replicación1. Desacople la carga útil en `chunk_size`, calculando BLAKE3 por trozo + racine Merkle.
2. Construir Norito `DaManifestV1` (nueva estructura) capturando los compromisos
   el fragmento (role/group_id), el diseño de borrado (cuentas de partición de líneas y
   colonnes plus `ipa_commitment`), la política de retención y los metadatos.
3. Mettre en file les bytes du manifest canonique sous
   `config.da_ingest.manifest_store_dir` (Torii escrito de archivos
   Índices `manifest.encoded` por carril/época/secuencia/ticket/huella digital) afin
   que l'orchestration SoraFS les ingere et relie le storage ticket aux donnees
   persiste.
4. Publier les intents de pin via `sorafs_car::PinIntent` con etiqueta de gobierno
   y política.
5. Emettre l'evenement Norito `DaIngestPublished` para notificar a los observadores
   (clientes legers, gobernanza, analítica).
6. Renvoyer `DaIngestReceipt` au caller (signe par la cle de service DA de Torii)
   y elimine el encabezado `Sora-PDP-Commitment` para que los SDK capturen el archivo
   el compromiso codifica la inmediatez. El recibo incluye mantenimiento `rent_quote`
   (un Norito `DaRentQuote`) y `stripe_layout`, remitentes auxiliares permanentes
   Muestra el alquiler de base, la reserva, los asistentes de bonificación PDP/PoTR y
   Diseño de borrado 2D en las costas del ticket de almacenamiento antes de utilizar el fondo.

## Mises a diario almacenamiento/registro- Etendre `sorafs_manifest` con `DaManifestV1`, permitiendo un análisis
  determinista.
- Agregar una nueva secuencia de registro `da.pin_intent` con una versión de carga útil
  referente al hash de manifiesto + ID del ticket.
- Mettre a jour les pipelines d'observabilite pour suivre la latence d'ingest,
  el rendimiento de fragmentación, el trabajo pendiente de replicación y los ordenadores
  d'echecs.

## Estrategia de pruebas

- Pruebas unitarias para validación de esquema, comprobaciones de firma, detección de
  doblones.
- Pruebas de verificación dorada de la codificación Norito de `DaIngestRequest`, manifiesto y
  recibo.
- Arnés de integración solicitando un SoraFS + simulaciones de registro, archivos válidos
  flujo de trozo + pin.
- Pruebas de propiedades que cubren los perfiles de borrado y combinaciones de
  retención aleatoria.
- Fuzzing des payloads Norito para proteger contra los metadatos mal formados.

## CLI de herramientas y SDK (DA-8)- `iroha app da submit` (nuevo punto de entrada CLI) generador de mantenimiento de sobres
  d'ingest partage afin que les operatorurs puissent ingerer des blobs
  Arbitrios fuera del flujo Paquete Taikai. La comando vit dans
  `crates/iroha_cli/src/commands/da.rs:1` y consuma una carga útil, un perfil
  Borrado/retención y archivos opcionales de metadatos/manifiesto antes de
  signer le `DaIngestRequest` canonique con el botón de configuración CLI. Las carreras
  reussis persistente `da_request.{norito,json}` y `da_receipt.{norito,json}` sous
  `artifacts/da/submission_<timestamp>/` (anulación mediante `--artifact-dir`) después de que
  les artefactos de liberación registrando les bytes Norito exactas utiliza colgante
  Lo ingiero.
- El comando utiliza por defecto `client_blob_id = blake3(payload)` pero se acepta
  Anulaciones a través de `--client-blob-id`, respeta los mapas JSON de metadatos
  (`--metadata-json`) y los manifiestos pregéneres (`--manifest`), y soporte
  `--no-submit` para la preparación fuera de línea más `--endpoint` para los hosts
  Torii personaliza. El recibo JSON se imprime en la salida estándar y además
  Escribir en disco, activando la herramienta de exigencia "submit_blob" de DA-8 y
  Debloquant le travail de parite SDK.
- `iroha app da get` agregue un alias DA para el orquestador multifuente que alimenta
  deja `iroha app sorafs fetch`. Los operadores pueden marcar el puntero frente a los artefactos.
  manifiesto + plan de fragmentos (`--manifest`, `--plan`, `--manifest-id`) **ou** fournirun ticket de almacenamiento Torii vía `--storage-ticket`. Quand le chemin ticket est
  utilizar, la CLI recupera el manifiesto después de `/v2/da/manifests/<ticket>`,
  persistir el paquete bajo `artifacts/da/fetch_<timestamp>/` (anular con
  `--manifest-cache-dir`), deriva el hash del blob para `--manifest-id`, luego
  Ejecute el orquestador con la lista `--gateway-provider` fournie. Todos les
  pomos avances du fetcher SoraFS resto intactos (sobres manifiestos, etiquetas
  cliente, guardar cachés, anular el transporte anónimo, exportar marcador y
  rutas `--output`), y el manifiesto del punto final puede tener un recargo a través de
  `--manifest-endpoint` para los hosts Torii personaliza, realiza comprobaciones
  La disponibilidad de extremo a extremo vive en todo el espacio de nombres `da` sin
  Duplicar la lógica del orquestador.
- `iroha app da get-blob` recupera los manifiestos canónicos directamente después de Torii
  vía `GET /v2/da/manifests/{storage_ticket}`. La orden escrita
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` y
  `chunk_plan_{ticket}.json` en `artifacts/da/fetch_<timestamp>/` (o un
  `--output-dir` proporcionado por el usuario) para indicar el comando exacto
  `iroha app da get` (incluido `--manifest-id`) requerido para el orquestador de búsqueda.
  Cela garde les operatorurs fuera de repertorios spool de manifests et garantit
  que le fetcher utiliza siempre los artefactos firmados emitidos por Torii. el cliente
  Torii Reproducción de JavaScript con flujo a través de`ToriiClient.getDaManifest(storageTicketHex)`, reenviando bytes Norito
  decodifica, el manifiesto JSON y el plan de fragmentos según el SDK de las personas que llaman
  des sessiones d'orchestrateur sans passer par la CLI. El SDK Swift expone
  mantenimiento de las superficies memes (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`), ramificando los paquetes en el envoltorio natif
  Orquestador SoraFS para que los clientes iOS puedan telecargar
  manifiestos, ejecutador de búsquedas de múltiples fuentes y capturador de preuves sans
  invocar la CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` cálculo de las rentas deterministas y la ventilación de
  incentivos para una cola de almacenamiento y una barrera de retención.
  El ayudante contiene el `DaRentPolicyV1` activo (JSON o bytes Norito) o el archivo
  Integra por defecto, valida la política e imprime un currículum JSON (`gib`,
  `months`, metadata de politique, champs `DaRentQuote`) afin que les auditeurs
  citar los cargos XOR exactos en las actas de gobierno sin guiones ad
  hoc. El comando emet aussi un currículum en una línea `rent_quote ...` antes del
  carga útil JSON para guardar los registros consola lisibles colgar los taladros
  d'incidente. Asociación `--quote-out artifacts/da/rent_quotes/<stamp>.json` con
  `--policy-label "governance ticket #..."` para persistir artefactos soignescitando el voto o el paquete de configuración exacta; la CLI tronque le label personnalise
  et rechace les chaines vides afin que les valeurs `policy_source` restent
  accionables en los paneles de tresorerie. Ver
  `crates/iroha_cli/src/commands/da.rs` para el bajo mando y
  `docs/source/da/rent_policy.md` para el esquema político.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` cadena todo lo que precede: está buscando un almacenamiento
  billete, telecargar el paquete canónico de manifiesto, ejecutar el orquestador
  fuente múltiple (`iroha app sorafs fetch`) contra la lista `--gateway-provider`
  fournie, persiste le payload telecharge + marcador sous
  `artifacts/da/prove_availability_<timestamp>/`, e invoque inmediatamente le
  PoR auxiliar existente (`iroha app da prove`) con bytes recuperados. Los operadores
  posible ajustar los mandos del orquestador (`--max-peers`, `--scoreboard-out`,
  anula el manifiesto del punto final) y el muestreador de prueba (`--sample-count`,
  `--leaf-index`, `--sample-seed`) tandis qu'une solo comandoe produit les
  artefactos asistentes par les auditorías DA-5/DA-9: copia de la carga útil, evidencia de
  marcador y currículums de preuve JSON.