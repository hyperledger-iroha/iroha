---
lang: pt
direction: ltr
source: docs/portal/docs/da/ingest-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota História Canônica
Esta página contém `docs/source/da/ingest_plan.md`. Держите обе версии
:::

# Planeje a ingestão de disponibilidade de dados Sora Nexus

_Novo: 20/02/2026 — Atualizado: Core Protocol WG / Storage Team / DA WG_

Рабочий поток DA-2 расширяет Torii API ingest para blob, который выпускает
Norito-метаданные e запускает репликацию SoraFS. Documento de descrição preliminar
схему, API-область e поток валидации, чтобы реализация шла без блокировки на
ожидающих симуляциях (acompanhamento DA-1). Quais são os formatos de carga útil ДОЛЖНЫ использовать
código Norito; fallback para serde/JSON não foi implementado.

##Céli

- Принимать большие blob (сегменты Taikai, sidecar lane, артефакты управления)
  Determinação do valor Torii.
- Создавать канонические Norito manifestos, descrição de blob, parâmetros de codec,
  apagamento de perfil e retenção política.
- Сохранять метаданные chunk no armazenamento quente SoraFS e ставить задачи репликации в
  ok.
- Publicar intenções de pin + tags de política no arquivo SoraFS e instalar
  atualizado.
- Выдавать recibos de admissão, чтобы клиенты получали детерминированное
  подтверждение публикации.

## Superfície de API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Carga útil — este `DaIngestRequest`, definido como Norito. Ответы используют
`application/norito+v1` e substitua `DaIngestReceipt`.

| Exibição | Agradecimentos |
| --- | --- |
| 202 Aceito | Blob colocado em chunking/replication; recibo enviado. |
| 400 Solicitação incorreta | Use esquema/размера (como prova). |
| 401 Não autorizado | Abra o API-token. |
| 409 Conflito | Dublado `client_blob_id` com metadado não especificado. |
| 413 Carga útil muito grande | Limite de limite de blob. |
| 429 Muitas solicitações | Limite de taxa definido. |
| 500 Erro interno | Неожиданная ошибка (лог + алерт). |

## Предлагаемая Norito esquema

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

> Dicas para a realização: канонические Rust-представления этих payload теперь
> colocado em `iroha_data_model::da::types`, com embalagens de solicitação/recibo em
> `iroha_data_model::da::ingest` e estrutura do manifesto em
>`iroha_data_model::da::manifest`.

O `compression` descreve como o chamador transfere a carga útil. Torii versão
`identity`, `gzip`, `deflate` e `zstd`, распаковывая байты перед hashing,
chunking e comprovados manifestos opcionais.

### Verifique a validação

1. Verifique se o Norito é compatível com `DaIngestRequest`.
2. Ошибка, если `total_size` отличается от канонической длины carga útil
   (после распаковки) ou превышает настроенный максимум.
3. Selecione `chunk_size` (número de dados, <= 2 MiB).
4. Use, что `data_shards + parity_shards` <= máximo global e
   paridade >= 2.
5. `retention_policy.required_replica_count` должен соблюдать linha de base de governança.
6. Проверка подписи по каноническому hash (não é possível).
7. Abra o `client_blob_id`, sem carga útil de hash e sem metadados
   идентичны.
8. Use `norito_manifest` para fornecer esquema + hash para suporte
   manifesto, пересчитанным после chunking; иначе узел генерирует manifesto e
   сохраняет его.
9. Применять настроенную политику репликации: Torii переписывает отправленный
   `RetentionPolicy` é `torii.da_ingest.replication_policy` (см.
   `replication-policy.md`) e отклоняет заранее созданные manifestos, если их
   метаданные retenção não совпадают с perfil forçado.

### Поток chunking e репликации1. Selecione a carga útil para `chunk_size`, use BLAKE3 para o pedaço + Merkle
   raiz.
2. Сформировать Norito `DaManifestV1` (nova estrutura), pedaço de compromisso de física
   (role/group_id), layout de apagamento (числа паритета строк и столбцов плюс
   `ipa_commitment`), retenção de política e metadação.
3. Poste o manifesto de bytes canônicos no pod anterior
   `config.da_ingest.manifest_store_dir` (Torii opção `manifest.encoded` para
   pista/época/sequência/ticket/impressão digital), чтобы оркестрация SoraFS могла
   поглотить их и связать ticket de armazenamento com сохраненными данными.
4. Intenções de pinos públicos para `sorafs_car::PinIntent` com a mesma atualização e
   político.
5. Emita a solução Norito `DaIngestPublished` para instalação
   (clientes leves, governança, análises).
6. Возвращать `DaIngestReceipt` caller (подписан ключом Torii DA) e отправлять
   заголовок `Sora-PDP-Commitment`, чтобы SDK сразу получили compromisso. Recibo
   теперь включает `rent_quote` (Norito `DaRentQuote`) e `stripe_layout`, чтобы
   отправители могли показывать базовую аренду, долю резерва, ожидания бонусов
   PDP/PoTR e layout de apagamento 2D são adequados para que o tíquete de armazenamento seja фиксации средств.

## Armazenamento / registro de segurança

- Расширить `sorafs_manifest` novo `DaManifestV1`, обеспечив детерминированный
  análise.
- Добавить новый fluxo de registro `da.pin_intent` com carga útil de versão,
  который ссылается на manifest hash + ticket id.
- Обновить observabilidade-пайплайны para monitorar задержки ingestão, taxa de transferência
  chunking, backlog de replicação e счетчиков ошибок.

## Teste de estratégia

- Testes de unidade para esquema de validação, verificação de dados, detecção de дубликатов.
- Golden-тесты для проверки Norito codificação `DaIngestRequest`, manifesto e recibo.
- Chicote de integração, simulação de simulação SoraFS + registro e verificação
  потоки pedaço + alfinete.
- Propriedades para apagamento de perfil profissional e retenção combinada.
- Carga útil Fuzzing Norito para proteger metadados de rede.

## Ferramentas CLI e SDK (DA-8)- `iroha app da submit` (ponto de entrada CLI novo) criado pelo construtor de ingestão/
  editor, чтобы операторы могли ingest-ить произвольные blobs вне потока
  Pacote Taikai. O comando foi instalado em `crates/iroha_cli/src/commands/da.rs:1` e
  Carga útil útil, eliminação/retenção de perfil e configurações opcionais
  metadados/manifesto перед подписью канонического `DaIngestRequest` ключом
  CLI de configuração. Use a chave `da_request.{norito,json}` e
  `da_receipt.{norito,json}` por `artifacts/da/submission_<timestamp>/`
  (override через `--artifact-dir`), чтобы релизные artefatos фиксировали точные
  Norito bytes, usados por ingestão.
- Comando para instalação de controle `client_blob_id = blake3(payload)`, não
  поддерживает substitui o через `--client-blob-id`, metadados dos cartões JSON
  (`--metadata-json`) e manifestos pré-gerados (`--manifest`), etc.
  `--no-submit` para dispositivos off-line e `--endpoint` para modelos Torii.
  O Receipt JSON é enviado para stdout e digitado no disco, executando o DA-8
  "submit_blob" e a paridade do SDK definida.
- `iroha app da get` добавляет DA-ориентированный alias para orquestrador multi-fonte,
  O arquivo é `iroha app sorafs fetch`. Os operadores podem usar artefatos
  manifesto + chunk-plan (`--manifest`, `--plan`, `--manifest-id`) **или** передать
  Torii ticket de armazenamento é `--storage-ticket`. Para usar o ticket CLI
  Abra o manifesto de `/v1/da/manifests/<ticket>`, instale o pacote em
  `artifacts/da/fetch_<timestamp>/` (substituir por `--manifest-cache-dir`), necessário
  hash de blob para `--manifest-id` e orquestrador de inicialização com controle remoto
  `--gateway-provider` списком. Quais botões são produzidos no buscador SoraFS
  сохраняются (envelopes manifestos, etiquetas de clientes, caches de segurança, anônimos
  substituições de transporte, placar de transporte, `--output` пути), um endpoint de manifesto
  Você pode usar o `--manifest-endpoint` para o modelo Torii,
  Aqui está a disponibilidade ponta a ponta fornecida no namespace `da` não
  дублирования orquestrador логики.
- `iroha app da get-blob` забирает канонические manifestos напрямую из Torii через
  `GET /v1/da/manifests/{storage_ticket}`. Команда пишет
  `manifest_{ticket}.norito`, `manifest_{ticket}.json` e `chunk_plan_{ticket}.json`
  em `artifacts/da/fetch_<timestamp>/` (ou usado `--output-dir`), por
  Este é o comando `iroha app da get` (que `--manifest-id`), novo
  para buscar o orquestrador. Este é o operador de trabalho com
  manifest spool директориями и гаrantирует, что fetcher всегда использует
  artefatos подписанные Torii. Cliente JavaScript Torii fornece este valor
  `ToriiClient.getDaManifest(storageTicketHex)`, decodificador Norito
  bytes, manifesto JSON e plano de bloco, quais chamadores do SDK podem ser usados
  orquestrador é uma seção sem CLI. O Swift SDK é compatível com o que você pode usar
  (`ToriiClient.getDaManifestBundle(...)` e `fetchDaPayloadViaGateway(...)`),
  Proximidade do pacote no wrapper nativo SoraFS orquestrador, clientes iOS disponíveis
  скачивать manifestos, выполнять busca multi-fonte e собирать доказательства без
  nossa CLI.
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` вычисляет детерминированную aluguel e incentivo de parcelamento para
  заданного размера armazenamento e окна retenção. Ajudante ativo
  `DaRentPolicyV1` (JSON ou Norito bytes) é um padrão padrão, validado
  política e configuração JSON-сводку (`gib`, `months`, metadados de política e política
  `DaRentQuote`), estes auditores podem ter taxas de XOR no protocolo
  não há scripts ad hoc. Команда также печатает однострочный
  `rent_quote ...` fornece carga útil JSON para obter logs no início dos exercícios.
  Свяжите `--quote-out artifacts/da/rent_quotes/<stamp>.json` com
  `--policy-label "governance ticket #..."`, чтобы сохранить аккуратные artefatos
  с точной ссылкой на голосование ou config bundle; CLI обрезает пользовательскую
  метку и отвергает пустые строки, чтобы `policy_source` оставался пригодным для
  дашбордов. Sim. `crates/iroha_cli/src/commands/da.rs` para подкоманды e
  `docs/source/da/rent_policy.md` para quadros políticos.
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` объединяет все выше: bilhete de armazenamento берет,
  скачивает канонический pacote de manifesto, запускает orquestrador multi-fonte
  (`iroha app sorafs fetch`) против списка `--gateway-provider`, сохраняет
  carga útil + placar скачанный em `artifacts/da/prove_availability_<timestamp>/`,
  e você deve usar o PoR helper (`iroha app da prove`) com o utilitário
  bytes. Операторы могут настраивать botões orquestradores (`--max-peers`,
  `--scoreboard-out`, substituições de endpoint de manifesto) e amostrador de prova(`--sample-count`, `--leaf-index`, `--sample-seed`), por este comando
  выпускает artefatos, требуемые аудитами DA-5/DA-9: copia carga útil, доказательство
  placar e prova de JSON.