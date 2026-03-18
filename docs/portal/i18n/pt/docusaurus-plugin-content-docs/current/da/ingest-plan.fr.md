---
lang: pt
direction: ltr
source: docs/portal/docs/da/ingest-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Reflete `docs/source/da/ingest_plan.md`. Gardez les deux versões en sync tant
:::

# Planejar a disponibilidade de dados Sora Nexus

_Redige: 2026-02-20 - Responsáveis: Core Protocol WG / Storage Team / DA WG_

O fluxo de trabalho DA-2 é estendido Torii com uma API de ingestão de blobs que emet des
metadonnees Norito e amorce a replicação SoraFS. Este arquivo de captura de documento
proposta de esquema, API de superfície e fluxo de validação para que
a implementação avança sem bloquear as simulações restantes (suivi
DA-1). Todos os formatos de carga útil DOIVENT utilizam os codecs Norito; Aucun
fallback serde/JSON não é permitido.

## Objetivos

- Aceitar blobs volumineux (segmentos Taikai, sidecars de lane, artefatos de
  governo) de maneira determinada via Torii.
- Produza os manifestos Norito canônicos descrevendo o blob, os parâmetros de
  codec, perfil de apagamento e política de retenção.
- Persistir os metadados dos pedaços no estoque quente de SoraFS e colocá-los em
  arquivar os trabalhos de replicação.
- Publique as intenções de pin + tags políticas no registro SoraFS e
  observadores de governo.
- Expositor de recibos de admissão para que os clientes revisem uma prévia
  determina a publicação.

## API de superfície (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

A carga útil é uma codificação `DaIngestRequest` em Norito. As respostas utilizadas
`application/norito+v1` e reenviado `DaIngestReceipt`.

| Resposta | Significado |
| --- | --- |
| 202 Aceito | Blob em arquivo para fragmentação/replicação; reenvio de recibo. |
| 400 Solicitação incorreta | Violação de esquema/taille (ver verificações de validação). |
| 401 Não autorizado | API de token manquant/inválida. |
| 409 Conflito | Doublon `client_blob_id` com metadados não idênticos. |
| 413 Carga útil muito grande | Ultrapasse o limite configurado de comprimento do blob. |
| 429 Muitas solicitações | Ateint de limite de taxa. |
| 500 Erro interno | Echec inattendu (log + alerta). |

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

> Nota de implementação: as representações Rust canônicas para essas cargas úteis
> vivent maintenant sous `iroha_data_model::da::types`, com des wrappers
> solicitação/recebimento em `iroha_data_model::da::ingest` e na estrutura de
> manifesto em `iroha_data_model::da::manifest`.

O campeão `compression` anuncia que os chamadores devem preparar a carga útil. Torii
aceite `identity`, `gzip`, `deflate`, e `zstd`, e descompacte os bytes antes
o hashing, o chunking e a verificação das opções de manifestos.

### Checklist de validação

1. Verifique se o cabeçalho Norito da solicitação corresponde a `DaIngestRequest`.
2. O eco `total_size` difere do comprimento canônico da carga útil
   (decompresse) ou descompacte a configuração máxima.
3. Force o alinhamento de `chunk_size` (poder de dois, <= 2 MiB).
4. Assegurador `data_shards + parity_shards` <= máximo global e paridade >= 2.
5. `retention_policy.required_replica_count` deve respeitar a linha de base de
   governança.
6. Verificação de assinatura com hash canônico (excluindo o campeão
   assinatura).
7. Rejeite uma duplicata `client_blob_id`, exceto se o hash da carga útil e os metadados
   são idênticos.
8. Quando `norito_manifest` é fornecido, verificador de esquema + hash correspondente
   au manifest recalcular apres chunking; sinon le noeud genere le manifest et le
   estoque.
9. Aplicar a política de replicação configurada: Torii reescrita
   `RetentionPolicy` soumis com `torii.da_ingest.replication_policy` (ver
   `replication-policy.md`) e rejeitar os manifestos pré-construídos não la
   metadados de retenção não correspondem ao perfil imposto.

### Fluxo de fragmentação e replicação1. Descupe a carga útil em `chunk_size`, calcule BLAKE3 par chunk + racine Merkle.
2. Construa Norito `DaManifestV1` (nova estrutura) capturando os compromissos
   o pedaço (role/group_id), o layout de apagamento (conta de par de linhas e
   colunas mais `ipa_commitment`), a política de retenção e os metadados.
3. Insira em um arquivo os bytes do manifesto canônico sous
   `config.da_ingest.manifest_store_dir` (Torii escrito de arquivos
   Índices `manifest.encoded` por faixa/época/sequência/ticket/impressão digital) afin
   que a orquestração SoraFS ingere e confia no armazenamento do ticket aux donnees
   persiste.
4. Publique as intenções de pin via `sorafs_car::PinIntent` com etiqueta de governo
   e política.
5. Emita o evento Norito `DaIngestPublished` para notificar os observadores
   (clientes legers, gouvernance, analytique).
6. Renvoyer `DaIngestReceipt` au caller (assinado pela chave de serviço DA de Torii)
   e emita o cabeçalho `Sora-PDP-Commitment` para que os SDKs capturem o
   compromisso codificar imediatamente. Le recibo inclui manutenção `rent_quote`
   (um Norito `DaRentQuote`) e `stripe_layout`, permitidos pelos remetentes
   exibir a renda de base, a reserva, as atenções de bônus PDP/PoTR e os
   layout de apagamento 2D aux cotes du ticket de armazenamento antes de engajar fundos.

## Mises a jour estoque/registro

- Crie `sorafs_manifest` com `DaManifestV1`, permitindo uma análise
  determinista.
- Adicionado um novo fluxo de registro `da.pin_intent` com uma versão de carga útil
  refere-se ao hash do manifesto + id do ticket.
- Mettre a jour les pipelines d'observabilite pour suivre la latence d'ingest,
  a taxa de transferência de chunking, o backlog de replicação e os computadores
  d'echecs.

## Estratégia de testes

- Testes unitários para validação de esquema, verificações de assinatura, detecção de
  dobrões.
- Testes de ouro verificando a codificação Norito de `DaIngestRequest`, manifesto et
  recibo.
- Harness d'integration demarrant un SoraFS + simulações de registro, arquivos válidos
  fluxo de pedaço + pino.
- Testes de propriedades cobrindo perfis de apagamento e combinações de
  aleatórios de retenção.
- Fuzzing de payloads Norito para proteger contra metadados mal formados.

## Ferramentas CLI e SDK (DA-8)- `iroha app da submit` (novo ponto de entrada CLI) envelope mantendo o construtor
  ingerir parte para que os operadores possam gerar bolhas
  Pacote Taikai de arbitrários fora do fluxo. La commande vit dans
  `crates/iroha_cli/src/commands/da.rs:1` e consome uma carga útil, um perfil
  exclusão/retenção e opções de arquivos de metadados/manifestos antes de
  assine o `DaIngestRequest` canônico com a chave de configuração CLI. As corridas
  reussis persistente `da_request.{norito,json}` e `da_receipt.{norito,json}` sous
  `artifacts/da/submission_<timestamp>/` (substituir via `--artifact-dir`) depois disso
  os artefatos de liberação registram os bytes Norito exatos utilizam pingente
  eu ingeri.
- O comando utiliza par padrão `client_blob_id = blake3(payload)` mais aceito
  das substituições via `--client-blob-id`, respeite os mapas JSON de metadados
  (`--metadata-json`) e os manifestos pré-gêneros (`--manifest`), e suporte
  `--no-submit` para preparação offline e `--endpoint` para hosts
  Torii personaliza. O recibo JSON está impresso no stdout e no ponto positivo
  escrito no disco, mantendo a exigência de ferramentas "submit_blob" do DA-8 e
  desbloqueie o trabalho de parite SDK.
- `iroha app da get` adiciona um alias DA para o orquestrador multi-fonte qui alimente
  deixe `iroha app sorafs fetch`. Os operadores podem apontar o ponteiro para os artefatos
  manifesto + chunk-plan (`--manifest`, `--plan`, `--manifest-id`) **ou** quatro
  um tíquete de armazenamento Torii via `--storage-ticket`. Quando o bilhete chemin est
  utilizar, a CLI recupera o manifesto de `/v1/da/manifests/<ticket>`,
  persista o pacote sob `artifacts/da/fetch_<timestamp>/` (substitua com
  `--manifest-cache-dir`), deriva o hash do blob para `--manifest-id`, depois
  execute o orquestrador com a lista `--gateway-provider` fournie. Todos nós
  botões avançados do buscador SoraFS permanecem intactos (envelopes manifestos, etiquetas
  cliente, guarda caches, substituições de transporte anônimo, exportar placar et
  caminhos `--output`), e o manifesto do endpoint pode ser sobrecarregado via
  `--manifest-endpoint` para hosts Torii personaliza, faz verificações
  Disponibilidade de ponta a ponta vivente totalmente no namespace `da` sem
  duplicar a lógica do orquestrador.
- `iroha app da get-blob` recupera os manifestos canônicos diretamente de Torii
  através de `GET /v1/da/manifests/{storage_ticket}`. O comando escrito
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` e outros
  `chunk_plan_{ticket}.json` sob `artifacts/da/fetch_<timestamp>/` (ou um
  `--output-dir` fornecido pelo usuário) e mostra o comando exato
  `iroha app da get` (incluindo `--manifest-id`) necessário para o orquestrador de busca.
  Cela garde les operadores fora dos repertórios spool de manifestos et garantit
  que o buscador utiliza sempre os artefatos assinados por Torii. O cliente
  Torii JavaScript reproduz esse fluxo via
  `ToriiClient.getDaManifest(storageTicketHex)`, reenviando os bytes Norito
  decodifica, o manifesto JSON e o plano de bloco para os chamadores SDK hidratados
  des sessões do orquestrador sem passar pela CLI. Le SDK Swift expõe
  manter superfícies de memes (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`), ramificando os pacotes no wrapper natif
  orquestrador SoraFS para que os clientes iOS possam baixar o aplicativo
  manifestos, executor de buscas multi-fonte e capturador de testes sem
  invocar a CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` calcula os aluguéis determinados e a ventilação dos
  incentivos para uma taxa de armazenamento e uma taxa de retenção.
  O ajudante usa o `DaRentPolicyV1` ativo (JSON ou bytes Norito) ou
  inteiro padrão, valide a política e imprima um currículo JSON (`gib`,
  `months`, metadados de política, campeões `DaRentQuote`) para os auditores
  cita as cobranças XOR exatas nos minutos de governo sem scripts de anúncio
  hoc. O comando emet também um currículo em uma linha `rent_quote ...` antes de le
  payload JSON para armazenar logs console lisibles pendentes de brocas
  incidente. Associar `--quote-out artifacts/da/rent_quotes/<stamp>.json` com
  `--policy-label "governance ticket #..."` para persistir artefatos sujos
  citant le vote ou bundle de config exact; a CLI tronque o rótulo personalizado
  e recuse as correntes vides para que os valores `policy_source` restem
  acionáveis nos painéis de controle da rede. Voir
  `crates/iroha_cli/src/commands/da.rs` para o subcomandante e o
  `docs/source/da/rent_policy.md` para o esquema político.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]- `iroha app da prove-availability` chaine tout ce qui precede: il prend un storage
  ticket, baixe o pacote canônico de manifesto, execute o orquestrador
  multi-fonte (`iroha app sorafs fetch`) com a lista `--gateway-provider`
  Fournie, persista a carga útil telecarregada + placar sous
  `artifacts/da/prove_availability_<timestamp>/`, e chame imediatamente o arquivo
  helper PoR existente (`iroha app da prove`) com bytes recuperados. Os operadores
  pode ajustar os botões do orquestrador (`--max-peers`, `--scoreboard-out`,
  substitui o manifesto do endpoint) e o amostrador de prova (`--sample-count`,
  `--leaf-index`, `--sample-seed`) tanto que um único comando produz les
  artefatos presentes nas auditorias DA-5/DA-9: cópia da carga útil, evidência de
  placar e currículos do JSON anterior.