---
lang: pt
direction: ltr
source: docs/portal/docs/da/ingest-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
ہونے تک دونوں ورژنز کو sincronização رکھیں۔
:::

# Plano de ingestão de disponibilidade de dados Sora Nexus

_مسودہ: 2026-02-20 -- Endereço: Core Protocol WG / Storage Team / DA WG_

DA-2 é uma API Torii que contém uma API de ingestão de blob que é usada para Norito.
جاری کرتی ہے اور SoraFS ریپلیکیشن کو semente کرتی ہے۔ یہ دستاویز مجوزہ esquema,
Superfície de API, fluxo de validação e implementação de implementação
simulações (acompanhamentos DA-1) Principais formatos de carga útil
Codecs Norito استعمال کرنا لازمی ہے؛ substituto serde/JSON

##

- بڑے blobs (segmentos Taikai, carros laterais de pista, artefatos de governança) کو Torii کے
  ذریعے deterministicamente قبول کرنا۔
- blob, parâmetros de codec, perfil de eliminação, e política de retenção کو بیان کرنے
  والے manifestos canônicos Norito تیار کرنا۔
- chunk metadata کو SoraFS hot storage میں محفوظ کرنا اور trabalhos de replicação کو
  enfileirar کرنا۔
- intenções de pinos + tags de política کو registro SoraFS اور observadores de governança تک
  publicar کرنا۔
- recibos de admissão فراہم کرنا تاکہ clientes کو prova determinística de
  publicação

## Superfície de API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

Carga útil ایک Norito codificada `DaIngestRequest` ہے۔ Respostas `application/norito+v1`
O cartão de crédito é `DaIngestReceipt` e o cartão de crédito

| Resposta | مطلب |
| --- | --- |
| 202 Aceito | Blob e chunking/replicação e fila recibo |
| 400 Solicitação incorreta | Violação de esquema/tamanho (verificações de validação دیکھیں)۔ |
| 401 Não autorizado | Token API |
| 409 Conflito | `client_blob_id` ڈپلیکیٹ ہے اور metadados مختلف ہے۔ |
| 413 Carga útil muito grande | Limite de comprimento de blob configurado |
| 429 Muitas solicitações | Limite de taxa atingido۔ |
| 500 Erro interno | غیر متوقع falha (log + alerta)۔ |

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

> Nota de implementação: cargas úteis e representações canônicas do Rust
> `iroha_data_model::da::types` کے تحت ہیں، wrappers de solicitação/recebimento
> `iroha_data_model::da::ingest` میں، اور estrutura de manifesto
> `iroha_data_model::da::manifest` میں ہے۔

Campo `compression` بتاتا ہے کہ chamadores نے carga útil کیسے تیار کیا۔ Torii
`identity`, `gzip`, `deflate`, e `zstd` são métodos de hashing, chunking e
manifestos opcionais verificam کرنے سے پہلے bytes کو descompactar کرتا ہے۔

### Lista de verificação de validação

1. Verifique کریں کہ solicitação کا Norito cabeçalho `DaIngestRequest` سے correspondência کرتا ہے۔
2. Comprimento da carga útil canônica (descompactada) `total_size` سے مختلف ہو یا
   máximo configurado سے زیادہ ہو تو falhar کریں۔
3. Alinhamento `chunk_size` impor کریں (potência de dois, <= 2 MiB)۔
4. `data_shards + parity_shards` <= paridade máxima global >= 2 یقینی بنائیں۔
5. `retention_policy.required_replica_count` کو respeito da linha de base de governança کرنا ہوگا۔
6. Hash canônico کے خلاف verificação de assinatura (campo de assinatura کے بغیر)۔
7. `client_blob_id` duplicatas rejeitadas کریں جب تک hash de carga útil + metadados idênticos نہ ہوں۔
8. اگر `norito_manifest` دیا گیا ہو تو esquema + hash کو chunking کے بعد
   manifesto recalculado سے correspondência کریں؛ ورنہ manifesto do nó gerar کر کے armazenar کرے۔
9. A política de replicação configurada impõe کریں: Torii `RetentionPolicy` کو
   `torii.da_ingest.replication_policy` سے reescrever کرتا ہے ( `replication-policy.md`
   دیکھیں ) اور manifestos pré-construídos کو rejeitar کرتا ہے اگر metadados de retenção
   perfil forçado سے correspondência نہ کرے۔

### Fluxo de fragmentação e replicação1. Payload کو `chunk_size` میں تقسیم کریں, ہر chunk پر BLAKE3 اور Merkle root حساب کریں۔
2. Norito `DaManifestV1` (estrutura) por meio de compromissos de bloco (role/group_id),
   layout de eliminação (contagens de paridade de linha/coluna + `ipa_commitment`), política de retenção,
   Os metadados e a captura de dados
3. Bytes de manifesto canônicos کو `config.da_ingest.manifest_store_dir` کے تحت fila کریں
   (Torii `manifest.encoded` arquivos faixa/época/sequência/ticket/impressão digital کے لحاظ سے لکھتا ہے)
   تاکہ SoraFS orquestração انہیں ingest کر کے ticket de armazenamento کو dados persistentes سے link کرے۔
4. `sorafs_car::PinIntent` کے ذریعے tag de governança + política کے ساتھ pin intents publicar کریں۔
5. Evento Norito `DaIngestPublished` emite observadores کریں تاکہ (clientes leves, governança, análise)
   کو اطلاع ملے۔
6. Chamador `DaIngestReceipt` کو واپس دیں (Torii chave de serviço DA سے assinada) اور
   `Sora-PDP-Commitment` cabeçalho emit کریں تاکہ SDKs فوری طور پر compromisso captura کر سکیں۔
   Recibo de recibo de `rent_quote` (Norito `DaRentQuote`) ou `stripe_layout` de recibo de pagamento
   Aluguel base do remetente, participação de reserva, expectativas de bônus PDP / PoTR e 2D
   layout de apagamento کو tíquete de armazenamento کے ساتھ commit de fundos کرنے سے پہلے دکھا سکتے ہیں۔

## Atualizações de armazenamento/registro

- `sorafs_manifest` کو `DaManifestV1` کے ساتھ estender کریں تاکہ análise determinística ہو سکے۔
- O fluxo de registro `da.pin_intent` é uma carga útil versionada
  hash de manifesto + ID do ticket e referência کرتا ہے۔
- Pipelines de observabilidade – latência de ingestão, redução da taxa de transferência, backlog de replicação
  اور contagens de falhas ٹریک کرنے کیلئے اپ ڈیٹ کریں۔

## Estratégia de teste

- Validação de esquema, verificações de assinatura, detecção de duplicatas e testes de unidade
- Codificação Norito کے testes dourados (`DaIngestRequest`, manifesto, recibo)۔
- Chicote de integração جو mock SoraFS + registro چلاتا ہے اور pedaço + fluxos de pinos verificar کرتا ہے۔
- Testes de propriedade, perfis de eliminação aleatórios e combinações de retenção cobrem کرتے ہیں۔
- Fuzzing de carga útil Norito تاکہ metadados malformados سے بچاؤ ہو۔

## Ferramentas CLI e SDK (DA-8)- `iroha app da submit` (ponto de entrada CLI) para construtor/editor de ingestão compartilhada e wrapper.
  operadores Fluxo de pacote Taikai کے باہر ingestão de blobs arbitrários کر سکیں۔ یہ کمانڈ
  `crates/iroha_cli/src/commands/da.rs:1` میں ہے اور payload, perfil de apagamento/retenção اور opcional
  arquivos de metadados/manifestos لے کر chave de configuração CLI کے ساتھ sinal canônico `DaIngestRequest` کرتی ہے۔
  O arquivo executa `da_request.{norito,json}` e `da_receipt.{norito,json}`.
  `artifacts/da/submission_<timestamp>/` کے تحت محفوظ کرتے ہیں (substituir via `--artifact-dir`) تاکہ
  liberar artefatos ingerir میں استعمال ہونے والے registro exato de bytes Norito کریں۔
- کمانڈ padrão میں `client_blob_id = blake3(payload)` استعمال کرتی ہے مگر `--client-blob-id` substituições,
  mapas JSON de metadados (`--metadata-json`) e manifestos pré-gerados (`--manifest`) e aceitar کرتی ہے،
  اور `--no-submit` (preparação offline) اور `--endpoint` (hosts Torii personalizados) سپورٹ کرتی ہے۔ Recibo JSON
  stdout پر print ہوتا ہے اور disk پر بھی لکھا جاتا ہے, جس سے DA-8 کا "submit_blob" requisito پورا ہوتا ہے
  Desbloquear trabalho de paridade SDK ہوتا ہے۔
- `iroha app da get` Alias focado em DA فراہم کرتا ہے جو orquestrador multi-fonte کو usar کرتا ہے جو پہلے ہی
  `iroha app sorafs fetch` کو چلاتا ہے۔ Manifesto de operadores + artefatos de plano de bloco (`--manifest`, `--plan`, `--manifest-id`)
  **یا** Bilhete de armazenamento Torii via `--storage-ticket` دے سکتے ہیں۔ Caminho do ticket پر CLI `/v2/da/manifests/<ticket>` سے
  download do manifesto کرتی ہے، pacote کو `artifacts/da/fetch_<timestamp>/` میں محفوظ کرتی ہے (substituir via
  `--manifest-cache-dir`), `--manifest-id` کیلئے blob hash deriva کرتی ہے, اور فراہم کردہ Lista `--gateway-provider`
  کے ساتھ execução do orquestrador کرتی ہے۔ SoraFS buscador کے botões avançados برقرار رہتے ہیں (envelopes manifestos,
  rótulos de cliente, caches de proteção, substituições de transporte de anonimato, exportação de placar, caminhos `--output`) ou
  `--manifest-endpoint` کے ذریعے substituição de endpoint de manifesto کیا جا سکتا ہے, لہذا verificações de disponibilidade ponta a ponta
  مکمل طور پر `da` namespace میں رہتے ہیں بغیر duplicata da lógica do orquestrador کئے۔
- `iroha app da get-blob` Torii سے `GET /v2/da/manifests/{storage_ticket}` کے ذریعے manifestos canônicos کھینچتا ہے۔
  Escolha `manifest_{ticket}.norito`, `manifest_{ticket}.json`, ou `chunk_plan_{ticket}.json`
  `artifacts/da/fetch_<timestamp>/` میں لکھتی ہے (یا `--output-dir` fornecido pelo usuário) ou `iroha app da get`
  invocação (بشمول `--manifest-id`) echo کرتی ہے جو orquestrador de acompanhamento buscar کیلئے درکار ہے۔ Operadores de اس سے
  diretórios de spool de manifesto سے دور رہتے ہیں اور fetcher ہمیشہ Torii کے artefatos assinados استعمال کرتا ہے۔
  Cliente JavaScript Torii یہی fluxo `ToriiClient.getDaManifest(storageTicketHex)` کے ذریعے دیتا ہے, اور decodificado
  Norito bytes, manifest JSON e chunk plan واپس کرتا ہے تاکہ SDK callers CLI کے بغیر sessões do orquestrador hidratar
  کر سکیں۔ Swift SDK بھی وہی superfícies expõem کرتا ہے (`ToriiClient.getDaManifestBundle(...)` اور
  `fetchDaPayloadViaGateway(...)`), pacotes کو wrapper de orquestrador SoraFS nativo میں pipe کر کے clientes iOS کو
  download de manifestos کرنے، buscas de múltiplas fontes چلانے اور captura de provas کرنے دیتا ہے بغیر CLI کے۔
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` دیے گئے tamanho de armazenamento اور janela de retenção کیلئے aluguel determinístico اور detalhamento de incentivo
  computar کرتا ہے۔ یہ auxiliar ativo `DaRentPolicyV1` (JSON یا Norito bytes) یا padrão integrado استعمال کرتا ہے،
  validação de política کرتا ہے، اور Resumo JSON (`gib`, `months`, metadados de política, campos `DaRentQuote`) imprimir کرتا ہے
  تاکہ minutos de governança de auditores میں cobranças XOR exatas citar کر سکیں بغیر scripts ad hoc کے۔ Carga útil JSON
  سے پہلے ایک لائن `rent_quote ...` resumo بھی دیتی ہے تاکہ logs do console legíveis رہیں۔
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` e `--policy-label "governance ticket #..."` کے ساتھ
  جوڑیں تاکہ artefatos embelezados محفوظ ہوں جو درست voto de política یا pacote de configuração citar کریں؛ Etiqueta personalizada CLI e acabamento
  کرتی ہے اور خالی strings rejeitadas کرتی ہے تاکہ `policy_source` Painel de controle میں acionável رہیں۔ دیکھیں
  `crates/iroha_cli/src/commands/da.rs` (subcomando) e `docs/source/da/rent_policy.md` (esquema de política)۔
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` اوپر کی سب چیزیں cadeia کرتا ہے: یہ ticket de armazenamento لیتا ہے, pacote de manifesto canônico
  baixar کرتا ہے، orquestrador multi-fonte (`iroha app sorafs fetch`) کو فراہم کردہ lista `--gateway-provider` کے خلاف
  چلاتا ہے، carga útil baixada + placar کو `artifacts/da/prove_availability_<timestamp>/` میں محفوظ کرتا ہے، اور
  فوری طور پر موجودہ Auxiliar PoR (`iroha app da prove`) کو bytes buscados کے ساتھ invocar کرتا ہے۔ Botões orquestradores de operadores(`--max-peers`, `--scoreboard-out`, substituições de endpoint de manifesto) e amostrador de prova (`--sample-count`, `--leaf-index`,
  `--sample-seed`) ajuste کر سکتے ہیں جبکہ ایک ہی auditorias de comando DA-5/DA-9 کیلئے متوقع artefatos پیدا کرتی ہے:
  cópia de carga útil, evidência de placar e resumos de prova JSON۔