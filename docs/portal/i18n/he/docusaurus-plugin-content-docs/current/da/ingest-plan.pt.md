---
lang: he
direction: rtl
source: docs/portal/docs/da/ingest-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canonica
Espelha `docs/source/da/ingest_plan.md`. Mantenha as duas versoes em
:::

# Plano de ingestao de Data Availability da Sora Nexus

_Redigido: 2026-02-20 - Responsaveis: Core Protocol WG / Storage Team / DA WG_

O workstream DA-2 estende Torii com uma API de ingestao de blobs que emite
metadados Norito e semeia a replicacao SoraFS. Este documento captura o esquema
proposto, a superficie de API e o fluxo de validacao para que a implementacao
avance sem bloquear em simulacoes pendentes (seguimentos DA-1). Todos os
formatos de payload DEVEM usar codecs Norito; nao sao permitidos fallbacks
serde/JSON.

## Objetivos

- Aceitar blobs grandes (segmentos Taikai, sidecars de lane, artefatos de
  governanca) de forma deterministica via Torii.
- Produzir manifests Norito canonicos que descrevam o blob, parametros de codec,
  perfil de erasure e politica de retencao.
- Persistir metadata de chunks no storage hot de SoraFS e enfileirar jobs de
  replicacao.
- Publicar intents de pin + tags de politica no registry SoraFS e observadores
  de governanca.
- Expor receipts de admissao para que os clientes recuperem prova deterministica
  de publicacao.

## Superficie API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

O payload e um `DaIngestRequest` codificado em Norito. As respostas usam
`application/norito+v1` e retornam `DaIngestReceipt`.

| Resposta | Significado |
| --- | --- |
| 202 Accepted | Blob enfileirado para chunking/replicacao; receipt retornado. |
| 400 Bad Request | Violacao de esquema/tamanho (veja validacoes). |
| 401 Unauthorized | Token API ausente/invalido. |
| 409 Conflict | Duplicado `client_blob_id` com metadata divergente. |
| 413 Payload Too Large | Excede o limite configurado de tamanho do blob. |
| 429 Too Many Requests | Rate limit atingido. |
| 500 Internal Error | Falha inesperada (log + alerta). |

## Esquema Norito proposto

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

> Nota de implementacao: as representacoes canonicas em Rust para esses payloads
> agora vivem em `iroha_data_model::da::types`, com wrappers de request/receipt
> em `iroha_data_model::da::ingest` e a estrutura de manifest em
> `iroha_data_model::da::manifest`.

O campo `compression` informa como os callers prepararam o payload. Torii aceita
`identity`, `gzip`, `deflate`, e `zstd`, descomprimindo os bytes antes de hash,
chunking e verificacao de manifests opcionais.

### Checklist de validacao

1. Verificar se o header Norito da requisicao corresponde a `DaIngestRequest`.
2. Falhar se `total_size` diferir do tamanho canonico do payload (descomprimido)
   ou exceder o maximo configurado.
3. Forcar alinhamento de `chunk_size` (potencia de dois, <= 2 MiB).
4. Garantir `data_shards + parity_shards` <= maximo global e parity >= 2.
5. `retention_policy.required_replica_count` deve respeitar a baseline de
   governanca.
6. Verificacao de assinatura contra hash canonico (excluindo o campo signature).
7. Rejeitar `client_blob_id` duplicado, exceto se o hash do payload e a metadata
   forem identicos.
8. Quando `norito_manifest` for fornecido, verificar schema + hash coincide com
   o manifest recalculado apos o chunking; caso contrario o node gera o manifest
   e o armazena.
9. Forcar a politica de replicacao configurada: Torii reescreve o
   `RetentionPolicy` enviado com `torii.da_ingest.replication_policy` (veja
   `replication-policy.md`) e rejeita manifests preconstruidos cuja metadata de
   retencao nao coincide com o perfil imposto.

### Fluxo de chunking e replicacao

1. Fatiar o payload em `chunk_size`, calcular BLAKE3 por chunk + raiz Merkle.
2. Construir Norito `DaManifestV1` (struct nova) capturando compromissos de
   chunk (role/group_id), layout de erasure (contagens de paridade de linhas e
   colunas mais `ipa_commitment`), politica de retencao e metadata.
3. Enfileirar os bytes do manifest canonico sob
   `config.da_ingest.manifest_store_dir` (Torii escreve arquivos
   `manifest.encoded` por lane/epoch/sequence/ticket/fingerprint) para que a
   orquestracao SoraFS os ingira e vincule o storage ticket aos dados
   persistidos.
4. Publicar intents de pin via `sorafs_car::PinIntent` com tag de governanca e
   politica.
5. Emitir evento Norito `DaIngestPublished` para notificar observadores
   (clientes leves, governanca, analitica).
6. Retornar `DaIngestReceipt` ao caller (assinado pela chave de servico DA de
   Torii) e emitir o header `Sora-PDP-Commitment` para que os SDKs capturem o
   commitment codificado de imediato. O receipt agora inclui `rent_quote`
   (um Norito `DaRentQuote`) e `stripe_layout`, permitindo que os remetentes
   exibam a renda base, a reserva, expectativas de bonus PDP/PoTR e o layout de
   erasure 2D ao lado do storage ticket antes de comprometer fundos.

## Atualizacoes de storage/registry

- Estender `sorafs_manifest` com `DaManifestV1`, habilitando parsing
  deterministico.
- Adicionar novo stream de registry `da.pin_intent` com payload versionado
  referenciando hash de manifest + ticket id.
- Atualizar pipelines de observabilidade para rastrear latencia de ingestao,
  throughput de chunking, backlog de replicacao e contagens de falhas.

## Estrategia de testes

- Tests unitarios para validacao de schema, checks de assinatura, deteccao de
  duplicados.
- Tests golden verificando encoding Norito de `DaIngestRequest`, manifest e
  receipt.
- Harness de integracao subindo mock SoraFS + registry, validando fluxos de
  chunk + pin.
- Tests de propriedades cobrindo perfis de erasure e combinacoes de retencao
  aleatorias.
- Fuzzing de payloads Norito para proteger contra metadata malformada.

## Tooling de CLI & SDK (DA-8)

- `iroha app da submit` (novo entrypoint CLI) agora envolve o builder/publisher de
  ingestao compartilhado para que operadores possam ingerir blobs arbitrarios
  fora do fluxo de Taikai bundle. O comando vive em
  `crates/iroha_cli/src/commands/da.rs:1` e consome um payload, perfil de
  erasure/retencao e arquivos opcionais de metadata/manifest antes de assinar o
  `DaIngestRequest` canonico com a chave de config da CLI. Execucoes bem-sucedidas
  persistem `da_request.{norito,json}` e `da_receipt.{norito,json}` sob
  `artifacts/da/submission_<timestamp>/` (override via `--artifact-dir`) para que
  os artefatos de release registrem os bytes Norito exatos usados durante a
  ingestao.
- O comando usa por padrao `client_blob_id = blake3(payload)` mas aceita
  overrides via `--client-blob-id`, honra mapas JSON de metadata (`--metadata-json`)
  e manifests pre-gerados (`--manifest`), e suporta `--no-submit` para preparacao
  offline mais `--endpoint` para hosts Torii personalizados. O receipt JSON e
  impresso no stdout alem de ser escrito no disco, fechando o requisito de
  tooling "submit_blob" da DA-8 e destravando o trabalho de paridade do SDK.
- `iroha app da get` adiciona um alias focado em DA para o orquestrador multi-source
  que ja alimenta `iroha app sorafs fetch`. Operadores podem apontar para artefatos
  de manifest + chunk-plan (`--manifest`, `--plan`, `--manifest-id`) **ou**
  passar um storage ticket Torii via `--storage-ticket`. Quando o caminho do
  ticket e usado, a CLI baixa o manifest de `/v1/da/manifests/<ticket>`,
  persiste o bundle sob `artifacts/da/fetch_<timestamp>/` (override com
  `--manifest-cache-dir`), deriva o hash do blob para `--manifest-id`, e entao
  executa o orquestrador com a lista `--gateway-provider` fornecida. Todos os
  knobs avancados do fetcher SoraFS permanecem intactos (manifest envelopes,
  labels de cliente, guard caches, overrides de transporte anonimo, export de
  scoreboard e paths `--output`), e o endpoint de manifest pode ser sobrescrito
  via `--manifest-endpoint` para hosts Torii personalizados, entao os checks
  end-to-end de availability vivem totalmente sob o namespace `da` sem duplicar
  logica do orquestrador.
- `iroha app da get-blob` baixa manifests canonicos direto de Torii via
  `GET /v1/da/manifests/{storage_ticket}`. O comando escreve
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` e
  `chunk_plan_{ticket}.json` sob `artifacts/da/fetch_<timestamp>/` (ou um
  `--output-dir` fornecido pelo usuario) enquanto imprime o comando exato de
  `iroha app da get` (incluindo `--manifest-id`) necessario para o fetch do
  orquestrador. Isso mantem operadores fora dos diretorios spool de manifests e
  garante que o fetcher sempre use os artefatos assinados emitidos por Torii. O
  cliente Torii JavaScript espelha o mesmo fluxo via
  `ToriiClient.getDaManifest(storageTicketHex)`, retornando os bytes Norito
  decodificados, manifest JSON e chunk plan para que callers de SDK hidratem
  sessoes de orquestrador sem usar a CLI. O SDK Swift agora expoe as mesmas
  superficies (`ToriiClient.getDaManifestBundle(...)` mais
  `fetchDaPayloadViaGateway(...)`), canalizando bundles para o wrapper nativo do
  orquestrador SoraFS para que clientes iOS possam baixar manifests, executar
  fetches multi-source e capturar provas sem invocar a CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcula rentas deterministicas e o detalhamento de
  incentivos para um tamanho de storage e janela de retencao fornecidos. O helper
  consome a `DaRentPolicyV1` ativa (JSON ou bytes Norito) ou o default embutido,
  valida a politica e imprime um resumo JSON (`gib`, `months`, metadata de
  politica e campos de `DaRentQuote`) para que auditores citem charges XOR exatas
  em atas de governanca sem scripts ad hoc. O comando tambem emite um resumo em
  uma linha `rent_quote ...` antes do payload JSON para manter logs de console
  legiveis durante drills de incidente. Emparelhe
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` com
  `--policy-label "governance ticket #..."` para persistir artefatos bonitos que
  citem o voto ou bundle de config exatos; a CLI corta o label personalizado e
  recusa strings vazias para que valores `policy_source` permanecam acionaveis
  nos dashboards de tesouraria. Veja
  `crates/iroha_cli/src/commands/da.rs` para o subcomando e
  `docs/source/da/rent_policy.md` para o schema de politica.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadeia tudo acima: recebe um storage ticket,
  baixa o bundle canonico de manifest, executa o orquestrador multi-source
  (`iroha app sorafs fetch`) contra a lista `--gateway-provider` fornecida, persiste
  o payload baixado + scoreboard sob `artifacts/da/prove_availability_<timestamp>/`,
  e invoca imediatamente o helper PoR existente (`iroha app da prove`) usando os
  bytes baixados. Operadores podem ajustar os knobs do orquestrador
  (`--max-peers`, `--scoreboard-out`, overrides de endpoint de manifest) e o
  sampler de proof (`--sample-count`, `--leaf-index`, `--sample-seed`) enquanto um
  unico comando produz os artefatos esperados por auditorias DA-5/DA-9: copia do
  payload, evidencia de scoreboard e resumos de prova JSON.
