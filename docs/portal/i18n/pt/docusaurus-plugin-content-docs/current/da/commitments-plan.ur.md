---
lang: pt
direction: ltr
source: docs/portal/docs/da/commitments-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
ہونے تک دونوں ورژنز کو sincronização رکھیں۔
:::

# Plano de compromissos de disponibilidade de dados Sora Nexus (DA-3)

_مسودہ: 2026-03-25 -- Fonte: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 Nexus بلاک فارمیٹ کو وسعت دیتا ہے تاکہ ہر pista میں ایسے determinístico
registros شامل ہوں جو DA-2 کے ذریعے قبول شدہ blobs کو بیان کریں۔ یہ نوٹ
estruturas de dados canônicas, bloquear ganchos de pipeline, provas de cliente leve, etc.
Superfícies Torii/RPC کو بیان کرتی ہے جو validadores کے admissão یا governança
verifica میں compromissos DA پر بھروسہ کرنے سے پہلے لازمی ہیں۔ Cargas úteis
ہیں؛ codificado em Norito SCALE e JSON ad-hoc

## مقاصد

- ہر Nexus بلاک میں compromissos por blob (raiz de bloco + hash de manifesto + opcional
  Compromisso KZG) شامل کرنا تاکہ peers armazenamento off-ledger دیکھے بغیر
  estado de disponibilidade دوبارہ بنا سکیں۔
- provas de associação determinísticas فراہم کرنا تاکہ light clients verify کر سکیں
  کہ hash de manifesto کسی مخصوص بلاک میں finalizado ہوا تھا۔
- Consultas Torii (`/v1/da/commitments/*`) e provas فراہم کرنا تاکہ relés,
  SDKs, automação de governança, repetição, disponibilidade e disponibilidade
  auditoria
- Envelope `SignedBlockWire` کو estruturas canônicas کھنا, نئی کو Norito
  cabeçalho de metadados اور derivação de hash de bloco سے thread کرتے ہوئے۔

## Visão geral do escopo

1. **Adições ao modelo de dados** Bloco `iroha_data_model::da::commitment` میں اور
   mudanças de cabeçalho `iroha_data_model::block` میں۔
2. **Ganchos do executor** تاکہ `iroha_core` Torii سے emitir ou receber recibos DA
   کرے (`crates/iroha_core/src/queue.rs` e `crates/iroha_core/src/block.rs`).
3. **Persistência/índices** تاکہ Consultas de compromissos WSV تیزی سے lidar com کرے
   (`iroha_core/src/wsv/mod.rs`).
4. **Adições de RPC Torii** listar/consultar/provar endpoints کیلئے
   `/v1/da/commitments` کے تحت۔
5. **Testes de integração + acessórios** ou layout de fiação e fluxo de prova e validação
   کریں `integration_tests/tests/da/commitments.rs` میں۔

## 1. Adições ao modelo de dados

###1.1`DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` é um arquivo de 48 bytes e pode ser reutilizado por `iroha_crypto::kzg`
  میں ہے۔ جب ausente ہو تو صرف Provas Merkle استعمال ہوتے ہیں۔
- Catálogo de pistas `proof_scheme` سے derivar ہوتا ہے؛ Cargas úteis KZG das pistas Merkle
  rejeitar کرتی ہیں جبکہ `kzg_bls12_381` faixas diferentes de zero Os compromissos KZG exigem
  کرتی ہیں۔ Torii فی الحال صرف Compromissos Merkle بناتا ہے اور KZG-configured
  pistas کو rejeitar کرتا ہے۔
- `KzgCommitment` é um arquivo de 48 bytes e pode ser reutilizado por `iroha_crypto::kzg`
  میں ہے۔ جب Merkle Lanes پر ausente ہو تو صرف Provas Merkle استعمال ہوتے ہیں۔
- `proof_digest` Integração PDP/PoTR DA-5
  registro میں cronograma de amostragem درج ہو جو blobs کو زندہ رکھنے کیلئے استعمال
  ہوتا ہے۔

### 1.2 Extensão do cabeçalho do bloco

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

Bundle hash بلاک hash اور `SignedBlockWire` metadados دونوں میں شامل ہوتا ہے۔ sim
بچیں۔

Nota de implementação: `BlockPayload` e transparente `BlockBuilder`
`da_commitments` setters/getters expõem کرتے ہیں (دیکھیں
`BlockBuilder::set_da_commitments` e `SignedBlock::set_da_commitments`) تاکہ
hosts بلاک selo ہونے سے پہلے anexo de pacote pré-construído کر سکیں۔ Ajudante
campo de construtores کو `None` رکھتے ہیں جب تک Torii حقیقی bundles thread نہ کرے۔

### 1.3 Codificação de fio

- `SignedBlockWire::canonical_wire()` Lista de transações da lista de transações کے فوراً بعد
  `DaCommitmentBundle` کیلئے Norito cabeçalho anexado کرتا ہے۔ versão byte `0x01` ہے۔
- `SignedBlockWire::decode_wire()` desconhecido `version` e pacotes configuráveis rejeitados
  ہے، جیسا کہ `norito.md` Política Norito بیان ہے۔
- Atualizações de derivação de hash صرف `block::Hasher` میں ہیں؛ formato de fio existente
  decodificar کرنے والے clientes leves خود بخود نیا campo حاصل کرتے ہیں کیونکہ
  Cabeçalho Norito

## 2. Bloquear fluxo de produção

1. Torii DA ingest ایک `DaIngestReceipt` finalize کرتا ہے اور اسے fila interna
   پر publicar کرتا ہے (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` تمام recibos جمع کرتا ہے جن کا `lane_id` زیر تعمیر بلاک سے
   match کرتا ہے، اور `(lane_id, client_blob_id, manifest_hash)` پر desduplicado
   کرتا ہے۔
3. Selo کرنے سے پہلے compromissos do construtor کو `(lane_id, epoch, sequence)` سے
   classificar کرتا ہے تاکہ hash determinístico رہے، pacote کو codec Norito سے codificar
   Atualização de atualização `da_commitments_hash`
4. Pacote WSV da loja ہوتا ہے اور `SignedBlockWire` کے ساتھ بلاک میں
   emitir ہوتا ہے۔

اگر بلاک بنانا falhar ہو تو fila de recebimentos میں رہتے ہیں تاکہ اگلی کوشش انہیں لے
سکے؛ construtor ہر lane کیلئے آخری شامل شدہ `sequence` ریکارڈ کرتا ہے تاکہ replay
ataques

## 3. RPC e superfície de consulta

Torii são endpoints de valor padrão:| Rota | Método | Carga útil | Notas |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtro de faixa/época/intervalo de sequência, paginação) | `DaCommitmentPage` é um valor de contagem total, compromissos, hash de bloco e hash de bloco. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (pista + hash de manifesto یا `(epoch, sequence)` tupla)۔ | `DaCommitmentProof` واپس کرتا ہے (registro + caminho Merkle + hash de bloco)۔ |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Ajudante sem estado جو cálculo de hash de bloco دوبارہ کرتا ہے اور validação de inclusão کرتا ہے؛ SDKs disponíveis para `iroha_crypto` com link de link para download |

Cargas úteis `iroha_data_model::da::commitment` کے تحت ہیں۔ Roteadores Torii
manipuladores کو موجودہ DA ingest endpoints کے ساتھ mount کرتے ہیں تاکہ token/mTLS
políticas de reutilização ہوں۔

## 4. Provas de inclusão e clientes leves

- بلاک produtor serializado lista `DaCommitmentRecord` پر árvore Merkle binária بناتا
  ہے۔ Como root `da_commitments_hash` e feed کرتی ہے۔
- Registro de destino `DaCommitmentProof` کے ساتھ `(sibling_hash, position)` کا vetor
  پیک کرتا ہے تاکہ raiz de verificadores دوبارہ بنا سکیں۔ provas de hash de bloco
  cabeçalho assinado بھی شامل ہیں تاکہ verificação de finalidade de clientes leves کر سکیں۔
- Ciclo de solicitação/verificação de prova de auxiliares CLI (`iroha_cli app da prove-commitment`) کو
  wrap کرتے ہیں اور operadores کیلئے Norito/saídas hexadecimais دیتے ہیں۔

## 5. Armazenamento e indexação

Compromissos WSV کو família de colunas dedicada میں Chave `manifest_hash` کے ساتھ
loja کرتا ہے۔ Índices secundários `(lane_id, epoch)` e `(lane_id, sequence)`
کو capa کرتے ہیں تاکہ consultas پورے varredura de pacotes نہ کریں۔ ہر registro ou bloco
altura کو trilha کرتا ہے جس نے اسے selo کیا، جس سے nós de recuperação bloquear log سے
índice تیزی سے reconstruir کر سکتے ہیں۔

## 6. Telemetria e observabilidade

- `torii_da_commitments_total` incremento ہوتا ہے جب کوئی بلاک کم از کم ایک
  selo de registro کرے۔
- Pacote `torii_da_commitment_queue_depth` ہونے کے انتظار میں recibos کو
  trilha کرتا ہے (por faixa)۔
- Painel Grafana Inclusão de bloco `dashboards/grafana/da_commitments.json`,
  profundidade da fila e taxa de transferência de prova دکھاتا ہے تاکہ portas de liberação DA-3
  auditoria comportamental

## 7. Estratégia de teste

1. Codificação/decodificação `DaCommitmentBundle` e atualizações de derivação de hash de bloco کیلئے
   **testes unitários**۔
2. `fixtures/da/commitments/` میں **golden fixtures** ou canonical bundle bytes
   Captura de provas Merkle کرتے ہیں۔
3. **Testes de integração** ou inicialização de validadores کرتے ہیں, ingestão de blobs de amostra
   کرتے ہیں، اور conteúdo do pacote اور consulta/respostas de prova پر اتفاق verificar کرتے
   ہیں۔
4. **Testes de cliente leve** `integration_tests/tests/da/commitments.rs` (ferrugem)
   میں جو `/prove` chamada کر کے Torii سے بات کئے بغیر verificação de prova کرتے ہیں۔
5. **CLI smoke** script `scripts/da/check_commitments.sh` Ferramentas do operador
   رہے۔ reproduzível

## 8. Plano de implementação

| Fase | Descrição | Critérios de saída |
|-------|------------|---------------|
| P0 — Mesclagem de modelos de dados | `DaCommitmentRecord`, atualizações de cabeçalho de bloco e codecs Norito pousam کریں۔ | `cargo test -p iroha_data_model` luminárias verdes کے ساتھ verde۔ |
| P1 — Fiação Core/WSV | fila + encadeamento lógico do construtor de bloco کریں, os índices persistem کریں, e os manipuladores RPC expõem کریں۔ | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` asserções à prova de pacote کے ساتھ pass۔ |
| P2 — Ferramentas do operador | Ajudantes CLI, painel Grafana, documentos de verificação de prova enviados کریں۔ | `iroha_cli app da prove-commitment` devnet پر چلتا ہو؛ dados ao vivo do painel |
| P3 — Portão de governança | `iroha_config::nexus` میں faixas sinalizadas کیلئے Os compromissos DA exigem کرنے e o validador de bloco permite کریں۔ | entrada de status + atualização do roteiro DA-3 کو marca completa کریں۔ |

## Perguntas abertas

1. **Padrões KZG vs Merkle** — کیا چھوٹے blobs کیلئے Compromissos KZG ہمیشہ
   چھوڑ دئے جائیں تاکہ tamanho do bloco کم ہو؟ Modelo: `kzg_commitment` رکھیں opcional
   Por `iroha_config::da.enable_kzg` کے ذریعے portão کریں۔
2. **Lacunas de sequência** — کیا pistas fora de ordem کی اجازت ہو؟ موجودہ پلان lacunas کو
   rejeitar کرتا ہے جب تک governança `allow_sequence_skips` کو repetição de emergência کیلئے
   ativar نہ کرے۔
3. **Cache de cliente leve** — SDK ٹیم نے provas کیلئے ایک ہلکا SQLite cache مانگا
   ہے؛ Acompanhamento DA-8 میں باقی ہے۔

ان سوالات کے جوابات PRs de implementação میں دینے سے DA-3 اس مسودے سے نکل کر
"em andamento" کی حالت میں جائے گا جب trabalho de código شروع ہوگا۔