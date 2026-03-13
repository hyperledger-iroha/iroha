---
lang: pt
direction: ltr
source: docs/portal/docs/da/ingest-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Espelha `docs/source/da/ingest_plan.md`. Mantenha as duas versoes em
:::

# Plano de ingestão de Data Availability da Sora Nexus

_Redigido: 2026-02-20 - Responsáveis: Core Protocol WG / Storage Team / DA WG_

O workstream DA-2 estende Torii com uma API de ingestão de blobs que emite
metadados Norito e semeia a replicação SoraFS. Este documento captura o esquema
proposta, a superfície de API e o fluxo de validação para que a implementação
avance sem bloqueio em simulações pendentes (seguimentos DA-1). Todos os
formatos de payload DEVEM usar codecs Norito; não são permitidos fallbacks
serde/JSON.

## Objetivos

- Aceitar blobs grandes (segmentos Taikai, sidecars de lane, artefatos de
  governança) de forma determinística via Torii.
- Produzir manifestos Norito canônicos que descrevam o blob, parâmetros de codec,
  perfil de apagamento e política de retenção.
- Persistir metadados de chunks no storage hot de SoraFS e arquivar jobs de
  replicação.
- Publicar intenções de pin + tags de política no registro SoraFS e observadores
  de governança.
- Expor recibos de admissão para que os clientes se recuperem com prova determinística
  de publicação.

## API de superfície (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

O payload é um `DaIngestRequest` codificado em Norito. Como as respostas usam
`application/norito+v1` e retornam `DaIngestReceipt`.

| Resposta | Significado |
| --- | --- |
| 202 Aceito | Blob enfileirado para chunking/replicação; recibo retornado. |
| 400 Solicitação incorreta | Violação de esquema/tamanho (veja validações). |
| 401 Não autorizado | API de token ausente/inválido. |
| 409 Conflito | Duplicado `client_blob_id` com metadados divergentes. |
| 413 Carga útil muito grande | Excede o limite configurado de tamanho do blob. |
| 429 Muitas solicitações | Limite de taxa atingido. |
| 500 Erro interno | Falha inesperada (log + alerta). |

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

> Nota de implementação: as representações canônicas em Rust para esses payloads
> agora vivem em `iroha_data_model::da::types`, com wrappers de solicitação/recibo
> em `iroha_data_model::da::ingest` e a estrutura de manifesto em
> `iroha_data_model::da::manifest`.

O campo `compression` informa como os chamadores prepararam o payload. Torii aceita
`identity`, `gzip`, `deflate`, e `zstd`, descomprimindo os bytes antes do hash,
chunking e verificação de manifestos indiretamente.

### Checklist de validação

1. Verifique se o cabeçalho Norito da requisição corresponde a `DaIngestRequest`.
2. Falhar se `total_size` diferir do tamanho canônico do payload (descomprimido)
   ou exceda o máximo configurado.
3. Forcar alinhamento de `chunk_size` (potência de dois, <= 2 MiB).
4. Garantir `data_shards + parity_shards` <= máximo global e paridade >= 2.
5. `retention_policy.required_replica_count` deve respeitar a linha de base de
   governança.
6. Verificação de assinatura contra hash canônico (excluindo o campo de assinatura).
7. Rejeitar `client_blob_id` duplicado, exceto se o hash do payload e os metadados
   antes idênticos.
8. Quando `norito_manifest` for fornecido, verifique esquema + hash coincidem com
   o manifesto recalculado após o chunking; caso contrário o nó gera o manifesto
   e o armazenamos.
9. Forcar a política de replicação configurada: Torii reescrever o
   `RetentionPolicy` enviado com `torii.da_ingest.replication_policy` (veja
   `replication-policy.md`) e rejeita manifestos pré-construídos cujos metadados de
   retencao nao coincide com o perfil imposto.

### Fluxo de chunking e replicação1. Fatiar o payload em `chunk_size`, calcular BLAKE3 por chunk + raiz Merkle.
2. Construir Norito `DaManifestV1` (struct nova) capturando compromissos de
   chunk (role/group_id), layout de apagamento (contagens de paridade de linhas e
   colunas mais `ipa_commitment`), política de retenção e metadados.
3. Enfileirar os bytes do manifesto canônico sob
   `config.da_ingest.manifest_store_dir` (Torii escreve arquivos
   `manifest.encoded` por lane/época/sequência/ticket/impressão digital) para que a
   orquestracao SoraFS a ingira e ligação o storage ticket aos dados
   persistidos.
4. Publicar intenções de pin via `sorafs_car::PinIntent` com tag de governança e
   política.
5. Emitir evento Norito `DaIngestPublished` para notificar observadores
   (clientes leves, governança, analítica).
6. Retornar `DaIngestReceipt` ao chamador (assinado pela chave do serviço DA de
   Torii) e emitir o cabeçalho `Sora-PDP-Commitment` para que os SDKs capturem o
   compromisso codificado de imediato. O recibo agora inclui `rent_quote`
   (um Norito `DaRentQuote`) e `stripe_layout`, permitindo que os remetentes
   exibam a renda base, a reserva, expectativas de bônus PDP/PoTR e o layout de
   apagamento 2D ao lado do ticket de armazenamento antes de comprometer fundos.

## Atualizações de armazenamento/registro

- Estender `sorafs_manifest` com `DaManifestV1`, habilitando análise
  determinístico.
- Adicionar novo stream de registro `da.pin_intent` com payload versionado
  referenciando hash de manifesto + id do ticket.
- Atualizar pipelines de observabilidade para rastrear latência de ingestão,
  throughput de chunking, backlog de replicação e contágio de falhas.

## Estratégia de testes

- Testes unitários para validação de esquema, verificações de assinatura, detecção de
  duplicados.
- Testa golden verificando a codificação Norito de `DaIngestRequest`, manifesto e
  recibo.
- Harness de integração subindo mock SoraFS + registro, validando fluxos de
  pedaço + alfinete.
- Testes de propriedades cobrindo perfis de apagamento e combinações de retenção
  aleatórios.
- Fuzzing de cargas úteis Norito para proteção contra metadados malformados.

## Ferramentas de CLI e SDK (DA-8)- `iroha app da submit` (novo entrypoint CLI) agora envolve o construtor/editor de
  ingestão compartilhada para que os operadores possam ingerir blobs arbitrários
  fora do fluxo de Taikai bundle. O comando vive em
  `crates/iroha_cli/src/commands/da.rs:1` e consome uma carga útil, perfil de
  apagamento/retenção e arquivos compilados de metadata/manifest antes de revisar o
  `DaIngestRequest` canônico com chave de configuração da CLI. Execuções bem-sucedidas
  persistem `da_request.{norito,json}` e `da_receipt.{norito,json}` sob
  `artifacts/da/submission_<timestamp>/` (substituir via `--artifact-dir`) para que
  os artistas de liberação registram os bytes Norito exatos usados durante um
  ingestão.
- O comando usa por padrão `client_blob_id = blake3(payload)` mas aceita
  substituições via `--client-blob-id`, honra mapas JSON de metadados (`--metadata-json`)
  e manifestos pré-gerados (`--manifest`), e suporte `--no-submit` para preparação
  offline mais `--endpoint` para hosts Torii personalizados. O recibo JSON e
  impresso no stdout além de ser escrito no disco, fechando o requisito de
  tooling "submit_blob" do DA-8 e destravando o trabalho de paridade do SDK.
- `iroha app da get` adiciona um alias focado em DA para o orquestrador multi-source
  que ja alimenta `iroha app sorafs fetch`. Operadores podem apontar para artefatos
  de manifesto + chunk-plan (`--manifest`, `--plan`, `--manifest-id`) **ou**
  passar um ticket de armazenamento Torii via `--storage-ticket`. Quando o caminho do
  ticket e usado, a CLI baixa o manifesto de `/v2/da/manifests/<ticket>`,
  persistir o pacote sob `artifacts/da/fetch_<timestamp>/` (override com
  `--manifest-cache-dir`), deriva o hash do blob para `--manifest-id`, e então
  executa o orquestrador com a lista `--gateway-provider` fornecida. Todos os
  botões avançados do fetcher SoraFS permanecem intactos (envelopes manifestos,
  etiquetas de cliente, guarda caches, substituições de transporte anônimo, exportação de
  scoreboard e paths `--output`), e o endpoint do manifesto pode ser sobrescrito
  via `--manifest-endpoint` para hosts Torii personalizados, então os cheques
  end-to-end de disponibilidade vivem totalmente sob o namespace `da` sem duplicar
  lógica do orquestrador.
- `iroha app da get-blob` baixa manifestos canônicos direto de Torii via
  `GET /v2/da/manifests/{storage_ticket}`. O comando escreve
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` e
  `chunk_plan_{ticket}.json` sob `artifacts/da/fetch_<timestamp>/` (ou um
  `--output-dir` fornecido pelo usuário) enquanto imprime o comando exato de
  `iroha app da get` (incluindo `--manifest-id`) necessário para buscar
  orquestrador. Isso mantem operadores fora dos diretórios spool de manifestos e
  garante que o buscador sempre utilize os contratos assinados emitidos por Torii. Ó
  cliente Torii JavaScript reflete o mesmo fluxo via
  `ToriiClient.getDaManifest(storageTicketHex)`, retornando os bytes Norito
  decodificados, manifest JSON e chunk plan para que os chamadores do SDK hidratem
  sessões de orquestrador sem usar a CLI. O SDK Swift agora expõe as mesmas
  superfícies (`ToriiClient.getDaManifestBundle(...)` mais
  `fetchDaPayloadViaGateway(...)`), canalizando bundles para o wrapper nativo do
  orquestrador SoraFS para que clientes iOS possam baixar manifestos, executar
  busca múltiplas fontes e captura tentativas sem invocar uma CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcula aluguel determinístico e o detalhamento de
  incentivos para um tamanho de armazenamento e janela de retenção fornecida. Ó ajudante
  consome a `DaRentPolicyV1` ativa (JSON ou bytes Norito) ou o padrão embutido,
  valida a política e imprime um resumo JSON (`gib`, `months`, metadados de
  política e campos de `DaRentQuote`) para que os auditores citem cobranças XOR exatas
  em atas de governança sem scripts ad hoc. O comando também emite um resumo em
  uma linha `rent_quote ...` antes do payload JSON para manter logs do console
  legíveis durante exercícios de incidente. Emparelhe
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` com
  `--policy-label "governance ticket #..."` para persistir artistas bonitos que
  citar o voto ou pacote de configurações exatas; a CLI corta o rótulo personalizado e
  recusa strings vazias para que valores `policy_source` permanentecam acionaveis
  nos painéis de tesouraria. Veja
  `crates/iroha_cli/src/commands/da.rs` para o subcomando e
  `docs/source/da/rent_policy.md` para o esquema político.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadeia tudo acima: recebe um ticket de armazenamento,
  baixa o pacote canônico de manifesto, executado o orquestrador multi-source
  (`iroha app sorafs fetch`) contra a lista `--gateway-provider` fornecida, persistao payload baixado + placar sob `artifacts/da/prove_availability_<timestamp>/`,
  e invocar imediatamente o helper PoR existente (`iroha app da prove`) usando os
  bytes baixados. Operadores podem ajustar os botões do orquestrador
  (`--max-peers`, `--scoreboard-out`, substitui o endpoint do manifesto) e o
  amostrador de prova (`--sample-count`, `--leaf-index`, `--sample-seed`) enquanto um
  único comando produz os artefatos esperados por auditorias DA-5/DA-9: copia do
  payload, evidência de placar e resumos de prova JSON.