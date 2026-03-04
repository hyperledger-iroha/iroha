---
lang: pt
direction: ltr
source: docs/portal/docs/da/commitments-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota História Canônica
Esta página contém `docs/source/da/commitments_plan.md`. Держите обе версии
:::

# Planeje a disponibilidade de dados Sora Nexus (DA-3)

_Atualizado: 25/03/2026 — Desenvolvido: Grupo de trabalho de protocolo principal/Equipe de contrato inteligente/Equipe de armazenamento_

DA-3 расширяет формат блока Nexus também, чтобы каждая lane встраивала
детерминированные записи, описывающие blobs, принятые DA-2. Neste documento
зафиксированы канонические структуры данных, хуки блокового пайплайна,
лайт-клиентские доказательства e поверхности Torii/RPC, которые должны появиться
então, como validar o pedido de admissão no DA-коммитменты при admissão ou
verificação. Essa carga útil é Norito-кодированы; sem SCALE e JSON ad-hoc.

##Céli

- Нести коммитменты на blob (chunk root + manifest hash + опциональный KZG
  compromisso) внутри каждого блока Nexus, чтобы пиры могли реконструировать
  A disponibilidade não é garantida pelo armazenamento off-ledger.
- Дать детерминированные provas de adesão, чтобы clientes leves могли проверить,
  Este hash de manifesto é finalizado no bloco de concreto.
- Teste de exportação Torii (`/v1/da/commitments/*`) e provas, verificação
  relés, SDKs e governança de automação fornecem disponibilidade sem problemas
  blocos.
- Сохранить канонический `SignedBlockWire` envelope, пропуская новые структуры
  é o cabeçalho de metadados Norito e o hash do bloco de derivação.

## Обзор области работ

1. **Modelo de dados definido** em `iroha_data_model::da::commitment` mais informações
   cabeçalho do bloco em `iroha_data_model::block`.
2. **Ganchos do executor** чтобы `iroha_core` ingerir-e recibos DA, эмитированные
   Torii (`crates/iroha_core/src/queue.rs` e `crates/iroha_core/src/block.rs`).
3. **Persistência/índices** WSV foi criado para consultas de compromisso
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii Adições de RPC** para listar/consultar/comprovar endpoints
   `/v1/da/commitments`.
5. **Testes de integração + acessórios** para testar layout de fios e fluxo de prova em
   `integration_tests/tests/da/commitments.rs`.

## 1. Modelo de dados de desenvolvimento

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

- `KzgCommitment` переиспользует 48 dígitos de `iroha_crypto::kzg`.
  При отсутствии используем только Provas Merkle.
- `proof_scheme` берется из каталога pistas; Merkle Lanes отклоняют cargas úteis KZG,
  As pistas `kzg_bls12_381` требуют ненулевых compromissos KZG. Torii seção
  производит только Merkle compromissos e отклоняет pistas com конфигурацией KZG.
- `KzgCommitment` verifique 48 pontos do `iroha_crypto::kzg`.
  При отсутствии на Merkle Lanes используем только Provas Merkle.
- `proof_digest` integra DA-5 PDP/PoTR, integração de dados
  amostragem de amostragem, utilização de blobs.

### 1.2 Cabeçalho do bloco Расширение

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

Este pacote está no bloco de hash, assim como nos metadados `SignedBlockWire`. Когда
накладных расходов.

Nota de implementação: `BlockPayload` e прозрачный `BlockBuilder` теперь имеют
setters/getters `da_commitments` (см. `BlockBuilder::set_da_commitments` e
`SignedBlock::set_da_commitments`), então esses hosts podem ser registrados
предварительно собранный pacote para fechar o bloco. Seus ajudantes-condutores
O post `None` e o Torii não oferecem pacotes reais.

### 1.3 Codificação de fio

- `SignedBlockWire::canonical_wire()` fornece cabeçalho Norito para
  `DaCommitmentBundle` é uma opção de transação de transação. Byte de versão `0x01`.
- `SignedBlockWire::decode_wire()` отклоняет pacotes com неизвестной `version`,
  na solução Norito, política de `norito.md`.
- Derivação de hash обновляется только в `block::Hasher`; clientes leves, clientes
  декодируют существующий formato de fio, автоматически получают novo pólo, потому
  Este cabeçalho Norito foi criado por mim.

## 2. Поток выпуска блоков

1. Torii DA ingest финализирует `DaIngestReceipt` e публикует его во внутреннюю
   очередь (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` coleta seus recibos com suporte `lane_id` para bloco em
   построении, дедуплицируя по `(lane_id, client_blob_id, manifest_hash)`.
3. Use os compromissos do construtor сортирует em `(lane_id, epoch,
   sequência)` para determinar o hash, codificar o codec do pacote Norito e
   обновляет `da_commitments_hash`.
4. O pacote mais recente é fornecido no WSV e é adicionado ao bloco no
   `SignedBlockWire`.

Se o bloco de prova for aprovado, os recibos serão transferidos para o final do dia
попытки; construtor записывает последний включенный `sequence` по каждой lane,
чтобы предотвратить replay атаки.

## 3. RPC e superfície de consulta

Torii fornece três endpoints:| Rota | Método | Carga útil | Notas |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (range-фильтр по lane/época/sequência, paginação) | Verifique `DaCommitmentPage` com contagem total, compromissos e bloco de hash. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (pista + hash de manifesto ou curto `(epoch, sequence)`). | Отвечает `DaCommitmentProof` (registro + caminho Merkle + hash блока). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Ajudante apátrida, пересчитывающий hash блока и проверяющий inclusão; A opção para SDKs não foi fornecida para `iroha_crypto`. |

Suas cargas úteis são colocadas em `iroha_data_model::da::commitment`. Roteador Torii
монтируют handlers рядом с существующими DA ingest endpoints, чтобы переиспользовать
token político/mTLS.

## 4. Provas de inclusão e clientes leves

- Производитель блока строит бинарное Merkle дерево по сериализованному списку
  `DaCommitmentRecord`. A cor é `da_commitments_hash`.
- `DaCommitmentProof` упаковывает целевой registro e vetor `(sibling_hash,
  posição)` чтобы верификаторы смогли восстановить корень. Hash de prova включает
  No cabeçalho do bloco e no cabeçalho, os clientes leves podem fornecer finalidade.
- Ajudantes CLI (`iroha_cli app da prove-commitment`) оборачивают цикл solicitação de prova/
  verifique e coloque Norito/hex para o operador.

## 5. Armazenamento e índice

WSV хранит compromissos na família de colunas отдельной no grupo `manifest_hash`.
Os índices de referência são `(lane_id, epoch)` e `(lane_id, sequence)`, чтобы
запросы не сканировали полные pacotes. Каждый registro хранит высоту блока, в
котором он был запечатан, что позволяет catch-up узлам быстро восстанавливать
índice do log de bloco.

## 6. Telemetria e Observabilidade

- `torii_da_commitments_total` instalado, o tamanho do bloco é mínimo
  registro anterior.
- `torii_da_commitment_queue_depth` отслеживает recibos, ожидающие agrupamento
  (na pista).
- Painel Grafana `dashboards/grafana/da_commitments.json` visualizado
  включение в блок, глубину очереди e taxa de transferência de prova para liberação de auditoria DA-3
  portão.

## 7. Estratégia de teste

1. **Testes de unidade** para codificação/decodificação `DaCommitmentBundle` e обновлений
   derivação de hash de bloco.
2. **Acessórios dourados** em `fixtures/da/commitments/` com pacote de bytes canônicos
   и Provas de Merkle.
3. **Testes de integração** são validados, ingerir blobs de amostra e provar
   согласованности pacote e consulta/prova ответов.
4. **Testes de cliente leve** em `integration_tests/tests/da/commitments.rs` (Rust),
   Verifique `/prove` e prove a prova de que foi testada por Torii.
5. **CLI smoke** script `scripts/da/check_commitments.sh` para instalação
   ferramentas operacionais.

## 8. Implementação do plano

| Fase | Descrição | Critérios de saída |
|-------|------------|---------------|
| P0 - Mesclagem de modelos de dados | Use `DaCommitmentRecord`, cabeçalho de bloco padrão e codecs Norito. | `cargo test -p iroha_data_model` зеленый с новыми fixtures. |
| P1 - Fiação Core/WSV | Protянуть queue + block builder логику, сохранить индексы e открыть manipuladores RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` são fornecidos com prova de pacote. |
| P2 - Ferramental do operador | Adicione ajudantes CLI, painel Grafana e documentos atualizados para verificação de prova. | `iroha_cli app da prove-commitment` funciona no devnet; painel de controle показывает live данные. |
| P3 - Porta de governança | Ative o validador de bloco, требующий compromissos DA em pistas, отмеченных em `iroha_config::nexus`. | Entrada de status + atualização do roteiro помечают DA-3 как завершенный. |

## Открытые вопросы

1. **Padrões KZG vs Merkle** — Нужно ли для маленьких blobs всегда пропускать
   Compromissos KZG, чтобы уменьшить размер блока? Sugestões: começar
   `kzg_commitment` opcional e portão é `iroha_config::da.enable_kzg`.
2. **Lacunas de sequência** — Разрешать ли разрывы последовательности? Plano de tecnologia
   отклоняет lacunas, если governança não включит `allow_sequence_skips` для
   repetição экстренного.
3. **Cache do cliente leve** — O SDK do comando armazena o cache SQLite legítimo para provas;
   дальнейшая работа под DA-8.

Ответы на эти вопросы в implementação PRs переведут DA-3 из статуса "rascunho"
(este documento) em "em andamento" após o código de início do trabalho.