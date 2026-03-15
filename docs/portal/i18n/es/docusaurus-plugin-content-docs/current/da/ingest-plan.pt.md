---
lang: es
direction: ltr
source: docs/portal/docs/da/ingest-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
España `docs/source/da/ingest_plan.md`. Mantenha como dos versos em
:::

# Plano de ingesta de Data Availability de Sora Nexus

_Redigido: 2026-02-20 - Responsaveis: Core Protocol WG / Storage Team / DA WG_

El flujo de trabajo DA-2 incluye Torii con una API de ingesta de blobs que emite
metadados Norito y algunas réplicas SoraFS. Este documento captura o esquema
propuesta, una superficie de API y el flujo de validación para que a implementar
avance sin bloquear em simulacoes pendentes (seguimentos DA-1). Todos os
formatos de carga útil DEVEM usar códecs Norito; nao sao permitidos retrocesos
serde/JSON.

## Objetivos

- Aceitar blobs grandes (segmentos Taikai, sidecars de lane, artefatos de
  Gobernanza) de forma determinística vía Torii.
- Produzir manifiesta Norito canónicos que descrevam o blob, parámetros de codec,
  perfil de erasure e politica de retencao.
- Persistir metadatos de fragmentos sin almacenamiento en caliente de SoraFS y enfileirar trabajos de
  replicacao.
- Publicar intents de pin + etiquetas de política no registro SoraFS e observadores
  de gobernancia.
- Exportación de recibos de admisión para que os clientes recuperem prova determinística
  de publicacao.

## Superficie API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```La carga útil es un `DaIngestRequest` codificado en Norito. Como respuestas usamos
`application/norito+v1` y regrese `DaIngestReceipt`.

| Respuesta | SIGNIFICADO |
| --- | --- |
| 202 Aceptado | Blob almacenado para fragmentación/replicación; recibo retornado. |
| 400 Solicitud incorrecta | Violacao de esquema/tamanho (veja validacoes). |
| 401 No autorizado | API de token ausente/inválido. |
| 409 Conflicto | Duplicado `client_blob_id` con metadatos divergentes. |
| 413 Carga útil demasiado grande | Exceda el límite configurado de tamaño del blob. |
| 429 Demasiadas solicitudes | Límite de tarifa atingido. |
| 500 Error interno | Falha inesperada (log + alerta). |

## Esquema Norito propuesta

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

> Nota de implementación: como representantes canónicos en Rust para esses payloads
> agora vivem em `iroha_data_model::da::types`, com envoltorios de solicitud/recibo
> em `iroha_data_model::da::ingest` y una estructura de manifiesto em
> `iroha_data_model::da::manifest`.

El campo `compression` informa cómo las personas que llaman preparan la carga útil. Torii aceita
`identity`, `gzip`, `deflate`, e `zstd`, descomprimiendo los bytes antes de hash,
fragmentación y verificación de manifiestos opcionales.

### Lista de verificación de validación1. Verifique si el encabezado Norito da requisicao corresponde a `DaIngestRequest`.
2. Falhar se `total_size` difiere del tamaño canónico de la carga útil (descomprimido)
   o exceder el máximo configurado.
3. Forcar alinhamento de `chunk_size` (potencia de dos, = 2.
5. `retention_policy.required_replica_count` debe respetar una línea base de
   gobernancia.
6. Verificacao de assinatura contra hash canonico (excluyendo la firma de campo).
7. Rejeitar `client_blob_id` duplicado, excepto el hash de la carga útil y los metadatos
   anteriores idénticos.
8. Cuando `norito_manifest` para fornecido, verificar esquema + hash coinciden com
   o manifiesto recalculado apos o fragmentación; caso contrario o nodo gera o manifiesto
   e o armazena.
9. Forcar a política de replicación configurada: Torii reescreve o
   `RetentionPolicy` enviado con `torii.da_ingest.replication_policy` (veja
   `replication-policy.md`) e rejeita manifiesta preconstruidos cuja metadata de
   retencao nao coincide con el perfil imposto.

### Flujo de fragmentación y replicación1. Fatiar la carga útil en `chunk_size`, calcular BLAKE3 por chunk + raíz Merkle.
2. Construir Norito `DaManifestV1` (struct nova) capturando compromisos de
   trozo (role/group_id), diseño de borrado (contagens de paridade de linhas e
   columnas más `ipa_commitment`), política de retención y metadatos.
3. Enfileirar os bytes do manifest canonico sollozo
   `config.da_ingest.manifest_store_dir` (Torii escreve archivos
   `manifest.encoded` por carril/época/secuencia/ticket/huella digital) para que a
   orquestracao SoraFS os ingira e vincule o ticket de almacenamiento aos dados
   persistidos.
4. Publicar intents de pin via `sorafs_car::PinIntent` com tag degobernanca e
   política.
5. Emitir evento Norito `DaIngestPublished` para notificar observadores
   (clientes leves, gobernadora, analítica).
6. Retornar `DaIngestReceipt` ao caller (assinado pela chave de servico DA de
   Torii) y emite el encabezado `Sora-PDP-Commitment` para que los SDK capturen
   compromiso codificado de inmediato. O recibo ágora incluido `rent_quote`
   (um Norito `DaRentQuote`) e `stripe_layout`, permitiendo que los remetentes
   exibam a renda base, a reserva, expectativas de bonificación PDP/PoTR e o diseño de
   borrado 2D al lado del ticket de almacenamiento antes de comprometer fondos.

## Actualizaciones de almacenamiento/registro- Estender `sorafs_manifest` con `DaManifestV1`, habilitando parsing
  determinista.
- Agregar nueva secuencia de registro `da.pin_intent` con carga útil versionada
  referenciando hash de manifiesto + ID del ticket.
- Actualizar tuberías de observabilidad para rastrear la latencia de ingesta,
  rendimiento de fragmentación, trabajo pendiente de replicación y contagios de errores.

## Estrategia de testículos

- Pruebas unitarias para validación de esquema, comprobaciones de assinatura, detección de
  duplicados.
- Pruebas golden verificando codificación Norito de `DaIngestRequest`, manifiesto e
  recibo.
- Arnés de integracao subindo simulado SoraFS + registro, validando flujos de
  trozo + alfiler.
- Tests de propiedades cobrindo perfis de erasure e combinacoes de retencao
  aleatorios.
- Fuzzing de payloads Norito para proteger contra metadatos malformados.

## Herramientas de CLI y SDK (DA-8)- `iroha app da submit` (nuevo punto de entrada CLI) ahora involucra al constructor/editor de
  ingestao compartilhado para que los operadores possam ingerir blobs arbitrarios
  fora do flujo de paquete Taikai. O comando vive em
  `crates/iroha_cli/src/commands/da.rs:1` y consume una carga útil, perfil de
  erasure/retencao e arquivos opcionais de metadata/manifest antes de assinar o
  `DaIngestRequest` canonico con una chave de config da CLI. Ejecucoes bem-sucedidas
  persiste `da_request.{norito,json}` e `da_receipt.{norito,json}` sollozo
  `artifacts/da/submission_<timestamp>/` (anulación mediante `--artifact-dir`) para que
  Los artefactos de liberación registran los bytes Norito exatos usados durante un
  ingesta.
- O comando usa por padrao `client_blob_id = blake3(payload)` mas aceita
  anulaciones a través de `--client-blob-id`, honra mapas JSON de metadatos (`--metadata-json`)
  e manifests pre-gerados (`--manifest`), e suporta `--no-submit` para preparacao
  Sin conexión más `--endpoint` para hosts Torii personalizados. O recibo JSON e
  impresso no stdout alem de ser escrito no disco, fechando o requisito de
  herramienta "submit_blob" del DA-8 y destravando el trabajo de paridad del SDK.
- `iroha app da get` agrega un alias enfocado en DA para el orquestador multifuente
  que ja alimenta `iroha app sorafs fetch`. Operadores podem apontar para artefatos
  manifiesto + plan de fragmentos (`--manifest`, `--plan`, `--manifest-id`) **ou**
  pasar un ticket de almacenamiento Torii a través de `--storage-ticket`. Cuando el camino lo hagoticket e usado, a CLI baixa o manifest de `/v2/da/manifests/<ticket>`,
  persistir o paquete sollozo `artifacts/da/fetch_<timestamp>/` (anular com
  `--manifest-cache-dir`), deriva del hash del blob para `--manifest-id`, y entao
  ejecuta el orquestador con la lista `--gateway-provider` fornecida. Todos os
  perillas avancados do fetcher SoraFS permanecem intactos (sobres manifiestos,
  etiquetas de cliente, cachés de guardia, anulaciones de transporte anónimo, exportación de
  marcador y rutas `--output`), y o punto final de manifiesto pode ser sobrescrito
  vía `--manifest-endpoint` para hosts Torii personalizados, entao os checks
  La disponibilidad de extremo a extremo vive totalmente sob o namespace `da` sin duplicar
  lógica del orquestador.
- `iroha app da get-blob` baixa manifests canonicos direto de Torii via
  `GET /v2/da/manifests/{storage_ticket}`. O comando escreve
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` y
  `chunk_plan_{ticket}.json` sollozo `artifacts/da/fetch_<timestamp>/` (ou um
  `--output-dir` fornecido pelo usuario) enquanto imprime o comando exato de
  `iroha app da get` (incluido `--manifest-id`) necesario para buscar
  orquestador. Isso mantem operadores fora dos diretorios spool de manifests e
  garante que o fetcher siempre use os artefatos assinados emitidos por Torii. oh
  cliente Torii JavaScript escribe o mesmo fluxo vía
  `ToriiClient.getDaManifest(storageTicketHex)`, retornando los bytes Norito
  decodificados, manifiesto JSON y plan de fragmentos para que las personas que llaman del SDK hidratenLas sesiones de orquestador sem usan CLI. El SDK Swift ahora expone como mesmas
  superficies (`ToriiClient.getDaManifestBundle(...)` más
  `fetchDaPayloadViaGateway(...)`), canalizando paquetes para el wrapper nativo
  orquestador SoraFS para que los clientes iOS puedan descargar manifiestos, ejecutar
  recupera múltiples fuentes y captura pruebas sin invocar una CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcula las rentas determinísticas y el detalle de
  incentivos para un tamaño de almacenamiento y janela de retencao fornecidos. Oh ayudante
  consome una `DaRentPolicyV1` activa (JSON o bytes Norito) o embutido por defecto,
  valida a política e imprime un resumen JSON (`gib`, `months`, metadatos de
  politica e campos de `DaRentQuote`) para que auditores citan cargos XOR exatas
  em atas degobernanca sem scripts ad hoc. El comando también emite un resumen en
  Una línea `rent_quote ...` antes de hacer la carga útil JSON para mantener los registros de la consola.
  legiveis durante simulacros de incidente. empareje
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` con
  `--policy-label "governance ticket #..."` para persistir artefatos bonitos que
  citar el voto o paquete de configuración exatos; a CLI corta o etiqueta personalizada e
  recusa strings vazias para que valores `policy_source` permanecam acionaveis
  nos tableros de tesouraria. Veja
  `crates/iroha_cli/src/commands/da.rs` para el subcomando e`docs/source/da/rent_policy.md` para el esquema político.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadeia todo acima: recibir un ticket de almacenamiento,
  baixa o bundle canonico de manifest, ejecuta o orquestador multi-source
  (`iroha app sorafs fetch`) contra la lista `--gateway-provider` fornecida, persiste
  o carga útil baixado + marcador sollozo `artifacts/da/prove_availability_<timestamp>/`,
  e invoca inmediatamente el helper PoR existente (`iroha app da prove`) usando el sistema operativo
  bytes bajos. Operadores podem ajustar os perillas do orquestador
  (`--max-peers`, `--scoreboard-out`, anula el punto final de manifiesto) e o
  muestra de prueba (`--sample-count`, `--leaf-index`, `--sample-seed`) en cuanto
  unico comando produz os artefatos esperados por auditorias DA-5/DA-9: copia do
  carga útil, evidencia de marcador y resúmenes de prueba JSON.