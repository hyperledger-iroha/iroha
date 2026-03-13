---
lang: pt
direction: ltr
source: docs/portal/docs/da/ingest-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Reflexo `docs/source/da/ingest_plan.md`. Mantenha ambas as versões em
:::

# Plano de ingestão de disponibilidade de dados de Sora Nexus

_Redatado: 2026-02-20 - Responsável: Core Protocol WG / Storage Team / DA WG_

O fluxo de trabalho DA-2 estende Torii com uma API de ingestão de blobs que emite
metadados Norito e sempre a replicação de SoraFS. Este documento captura o
esquema proposto, a superfície da API e o fluxo de validação para que
implementação avançada sem bloqueio de simulações pendentes (seguimentos
DA-1). Todos os formatos de carga útil DEBEN usam codecs Norito; não se permita
substitutos serde/JSON.

## Objetivos

- Aceitar blobs grandes (segmentos Taikai, sidecars de lane, artefatos de
  governo) de forma determinista via Torii.
- Produzir manifestos Norito canônicos que descrevem o blob, parâmetros de
  codec, perfil de apagamento e política de retenção.
- Persistir metadados de pedaços no armazenamento quente de SoraFS e colar trabalhos de
  replicação.
- Publicar intenções de pin + tags políticas no registro SoraFS e observadores
  de governança.
- Expor recibos de admissão para que os clientes recuperem a prova determinista
  de publicação.

## API de superfície (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

A carga útil é um `DaIngestRequest` codificado em Norito. As respostas usan
`application/norito+v1` e desenvolvido `DaIngestReceipt`.

| Resposta | Significado |
| --- | --- |
| 202 Aceito | Blob en cola para fragmentação/replicação; devolva o recibo. |
| 400 Solicitação incorreta | Violação de esquema/tamano (ver validações). |
| 401 Não autorizado | API de token ausente/inválido. |
| 409 Conflito | `client_blob_id` duplicado com metadados não coincidentes. |
| 413 Carga útil muito grande | Excede o limite configurado de longitude do blob. |
| 429 Muitas solicitações | Você pode alterar o limite de taxa. |
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

> Nota de implementação: as representações canônicas em Rust para estes
> cargas úteis agora vivem em baixo `iroha_data_model::da::types`, com wrappers de
> solicitação/recebimento em `iroha_data_model::da::ingest` e a estrutura do manifesto
> en `iroha_data_model::da::manifest`.

O campo `compression` declara como os chamadores preparam a carga útil. Torii
aceite `identity`, `gzip`, `deflate`, e `zstd`, descomprimindo os bytes antes de
hashear, chunking e verificar manifestos opcionais.

### Checklist de validação

1. Verifique se o cabeçalho Norito da solicitação coincide com `DaIngestRequest`.
2. Fallar si `total_size` difere do comprimento canônico da carga útil (descomprimido)
   ou exceda o máximo configurado.
3. Forzar alinhamento de `chunk_size` (potência de dos, <= 2 MiB).
4. Asegurar `data_shards + parity_shards` <= máximo global e paridade >= 2.
5. `retention_policy.required_replica_count` deve respeitar a linha base de
   governança.
6. Verificação de firma contra hash canônico (excluindo a assinatura do campo).
7. Rechazar duplicado `client_blob_id` salva o hash da carga útil e la
   metadados são idênticos.
8. Quando você provar `norito_manifest`, verifique o esquema + hash coincide com o
   manifesto recalculado após o chunking; de lo contrario el nodo gera el
   manifeste-se e guarde-o.
9. Forzar a política de replicação definida: Torii reescribe el
   `RetentionPolicy` enviado com `torii.da_ingest.replication_policy` (ver
   `replication-policy.md`) e rechaza manifesta pré-construídos cuya metadados de
   a retenção não coincide com o perfil imposto.

### Fluxo de fragmentação e replicação1. Troque o payload em `chunk_size`, calcule BLAKE3 por chunk + raiz Merkle.
2. Construir Norito `DaManifestV1` (struct nova) capturando comprometimentos de
   chunk (role/group_id), layout de apagamento (conteúdos de paridade de filas e
   colunas mas `ipa_commitment`), política de retenção e metadados.
3. Encolher os bytes do manifesto canônico abaixo
   `config.da_ingest.manifest_store_dir` (Torii escrever arquivos
   `manifest.encoded` por lane/época/sequência/ticket/impressão digital) para que la
   solicitação de SoraFS a entrada e vinculação do ticket de armazenamento com dados
   persistidos.
4. Publicar intenções de pin via `sorafs_car::PinIntent` com tag de governo e
   política.
5. Emitir evento Norito `DaIngestPublished` para notificar observadores (clientes
   ligeros, gobernanza, analítica).
6. Devolver `DaIngestReceipt` ao chamador (firmado pela chave de serviço DA de
   Torii) e emitir o cabeçalho `Sora-PDP-Commitment` para que os SDKs capturem o
   compromisso codificado de imediato. O recibo agora inclui `rent_quote`
   (um Norito `DaRentQuote`) e `stripe_layout`, permitindo os envios
   mostrar a base de aluguel, a reserva, as expectativas de bônus PDP/PoTR e o
   layout de apagamento 2D junto com o ticket de armazenamento antes de comprometer fundos.

## Atualizações de armazenamento/registro

- Extender `sorafs_manifest` com `DaManifestV1`, habilitando parseo
  determinista.
- Agregar novo fluxo de registro `da.pin_intent` com payload versionado que
  hash de referência do manifesto + id do ticket.
- Atualizar pipelines de observação para seguir a latência de ingestão,
  taxa de transferência de chunking, backlog de replicação e conteúdo de falhas.

## Estratégia de teste

- Testes unitários para validação de esquema, verificações de firma e detecção de
  duplicados.
- Testes dourados verificando a codificação Norito de `DaIngestRequest`, manifesto e
  recibo.
- Chicote de integração levantando SoraFS + registro simulado, validando
  flujos de chunk + pin.
- Testes de propriedades cubriendo perfis de apagamento e combinações de
  retenção aleatória.
- Fuzzing de cargas úteis Norito para proteção contra metadados malformados.

## Ferramentas de CLI e SDK (DA-8)- `iroha app da submit` (novo ponto de entrada da CLI) agora envie o construtor/editor
  compartimento de ingestão para que os operadores possam gerar blobs arbitrários
  pacote Taikai fuera del flujo. O comando vive em
  `crates/iroha_cli/src/commands/da.rs:1` e consome uma carga útil, perfil de
  apagamento/retenção e arquivos opcionais de metadados/manifestos antes de firmar
  o `DaIngestRequest` canônico com a chave de configuração da CLI. As execuções
  saídas persistem `da_request.{norito,json}` e `da_receipt.{norito,json}` baixo
  `artifacts/da/submission_<timestamp>/` (substituir via `--artifact-dir`) para que
  os artefatos de liberação registram os bytes Norito exatos usados durante o
  ingestão.
- O comando usa por defeito `client_blob_id = blake3(payload)`, mas aceita
  substituições via `--client-blob-id`, respeta mapas JSON de metadados
  (`--metadata-json`) e manifestos pré-gerados (`--manifest`), e suporte
  `--no-submit` para preparação offline, mas `--endpoint` para hosts Torii
  personalizados. O recibo JSON é impresso em stdout além de ser escrito em
  disco, cerrando o requisito de ferramenta "submit_blob" de DA-8 e desbloqueando
  o trabalho de paridade do SDK.
- `iroha app da get` adiciona um alias enfocado no DA para o orquestrador multi-source
  que você tem potência `iroha app sorafs fetch`. Os operadores podem apontá-lo para
  artefatos de manifesto + chunk-plan (`--manifest`, `--plan`, `--manifest-id`)
  **o** passe um tíquete de armazenamento de Torii via `--storage-ticket`. Quando você usa
  o caminho do ticket, a CLI abaixo do manifesto de `/v2/da/manifests/<ticket>`,
  persista o pacote abaixo `artifacts/da/fetch_<timestamp>/` (override con
  `--manifest-cache-dir`), deriva o hash do blob para `--manifest-id`, e depois
  executa o orquestrador com a lista `--gateway-provider` suministrada. Todos
  os botões avançados do buscador de SoraFS permanecem intactos (manifesto
  envelopes, etiquetas de cliente, guarda caches, cancelamentos de transporte anônimo,
  exportar placar e caminhos `--output`), e o endpoint do manifesto pode
  sobrescrever via `--manifest-endpoint` para hosts Torii personalizados, asi
  que los checks end-to-end de disponibilidade viven totalmente bajo el namespace
  `da` não duplica a lógica do orquestrador.
- `iroha app da get-blob` baja manifesta canonicos direto de Torii via
  `GET /v2/da/manifests/{storage_ticket}`. O comando escribe
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` e
  `chunk_plan_{ticket}.json` abaixo `artifacts/da/fetch_<timestamp>/` (ou um
  `--output-dir` fornecido pelo usuário) enquanto imprime o comando exato de
  `iroha app da get` (incluindo `--manifest-id`) necessário para buscar
  orquestrador. Isso mantém os operadores fora dos diretórios spool de
  manifesta e garante que o buscador sempre use os artefatos firmados
  emitido por Torii. O cliente Torii de JavaScript reflete o mesmo fluxo via
  `ToriiClient.getDaManifest(storageTicketHex)`, devolvendo os bytes Norito
  decodificados, manifesto JSON e plano de bloco para hidratar os chamadores do SDK
  sessões do orquestrador sem usar a CLI. O SDK do Swift agora expõe isso
  superfícies mismas (`ToriiClient.getDaManifestBundle(...)` mas
  `fetchDaPayloadViaGateway(...)`), canalizando pacotes para o wrapper nativo do
  orquestrador SoraFS para que clientes iOS possam baixar manifestos, executar
  busca várias fontes e captura testes sem invocar a CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcula rendas deterministas e desglose de incentivos
  para um tamanho de armazenamento e ventilação de retenção fornecidos. El ajudante
  consumir o `DaRentPolicyV1` ativo (JSON ou bytes Norito) ou o padrão integrado,
  validar a política e imprimir um currículo JSON (`gib`, `months`, metadados de
  política e campos de `DaRentQuote`) para que os auditores citem cargas XOR exatas
  em atos de governança sem scripts ad hoc. O comando também emite um currículo
  em uma linha `rent_quote ...` antes do payload JSON para manter legíveis os
  registros de console durante exercícios de incidentes. Empareje
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` com
  `--policy-label "governance ticket #..."` para persistir artefatos bonitos que
  citar o voto ou pacote de configuração exata; a CLI registra o rótulo personalizado
  e rechaza strings vacios para que os valores `policy_source` sejam mantidos
  acionáveis ​​em painéis de tesouraria. Ver
  `crates/iroha_cli/src/commands/da.rs` para o subcomando y
  `docs/source/da/rent_policy.md` para o esquema político.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadena todo o anterior: toma um armazenamentoticket, baixar o pacote canônico de manifesto, executar o orquestrador
  multi-fonte (`iroha app sorafs fetch`) contra a lista `--gateway-provider`
  suministrada, persiste el payload baixado + scoreboard bajo
  `artifacts/da/prove_availability_<timestamp>/`, e invoca imediatamente o auxiliar
  PoR existente (`iroha app da prove`) usando os bytes traídos. Os operadores
  você pode ajustar os botões do orquestrador (`--max-peers`, `--scoreboard-out`,
  substitui o endpoint do manifesto) e o amostrador de prova (`--sample-count`,
  `--leaf-index`, `--sample-seed`) enquanto um comando solo produz los
  artefatos esperados para auditorias DA-5/DA-9: cópia da carga útil, evidência de
  placar e resumos de teste JSON.