---
lang: pt
direction: ltr
source: docs/portal/docs/da/commitments-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Reflete `docs/source/da/commitments_plan.md`. Gardez les deux versões sincronizadas
:::

# Planeje compromissos Disponibilidade de dados Sora Nexus (DA-3)

_Redige: 2026-03-25 -- Responsáveis: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 possui o formato do bloco Nexus para que todas as faixas estejam inteiras de registros
determina a descrição dos blobs aceitos pelo DA-2. Estes arquivos de captura de notas
estruturas de donnees canonices, les hooks du pipeline de blocs, les preuves de
cliente leger, e as superfícies Torii/RPC que devem chegar antes que les
Validadores que podem ser aplicados em compromissos DA para cheques
d'admission ou de gouvernance. Todas as cargas úteis são codificadas em Norito; passo de
SCALE não é JSON ad hoc.

## Objetivos

- Porter des engagements par blob (chunk root + manifest hash + comprometimento KZG
  opcional) em cada bloco Nexus para que os pares possam reconstruir
  o estado de disponibilidade sem consultar o estoque fora do livro-razão.
- Fornecer recomendações de associação determinadas para que os clientes sejam legais
  verifique qual hash manifesto e finalize em um bloco feito.
- Exponha os requisitos Torii (`/v2/da/commitments/*`) e as precauções necessárias
  relés auxiliares, SDKs e automatizações de gerenciamento de auditoria de disponibilidade
  sans rejouer chaque bloc.
- Guarde o envelope `SignedBlockWire` canônico e transmita as novas
  estruturas por meio do cabeçalho de metadados Norito e da derivação do hash do bloco.

## Visualização do conjunto do escopo

1. **Adicionado ao modelo de dados** em `iroha_data_model::da::commitment` plus
   modificações no cabeçalho do bloco em `iroha_data_model::block`.
2. **Ganchos do executor** para que `iroha_core` ingira os recibos DA emis par
   Torii (`crates/iroha_core/src/queue.rs` e `crates/iroha_core/src/block.rs`).
3. **Persistência/índices** para que o WSV responda rapidamente às solicitações de
   compromissos (`iroha_core/src/wsv/mod.rs`).
4. **Adiciona RPC Torii** para os endpoints da lista/palestra/prova sob
   `/v2/da/commitments`.
5. **Testes de integração + acessórios** validando o layout do fio e o fluxo de prova
   em `integration_tests/tests/da/commitments.rs`.

## 1. Ajouts au modelo de dados

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

- `KzgCommitment` reutiliza o ponto 48 octetos utilizado em `iroha_crypto::kzg`.
  Quando ele está ausente, retombe sur des preuves Merkle uniquement.
- `proof_scheme` derivado do catálogo de pistas; les lanes Merkle rejeita os
  cargas úteis KZG tandis que les lanes `kzg_bls12_381` exigem compromissos KZG
  não nulos. Torii não é um produto atual que des compromissos Merkle et rejeitado
  as pistas configuradas em KZG.
- `KzgCommitment` reutiliza o ponto 48 octetos utilizado em `iroha_crypto::kzg`.
  Quando ele está ausente nas pistas Merkle em retombe sur des preuves Merkle
  singularidade.
- `proof_digest` antecipa a integração DA-5 PDP/PoTR após o registro do meme
  enumere o cronograma de amostragem utilizado para manter os blobs em dia.

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

O hash do pacote entre a parte inferior do hash do bloco e os metadados do
`SignedBlockWire`. Quando um bloco não é transportado para donnees DA, le champ reste

Nota de implementação: `BlockPayload` e o `BlockBuilder` exposto transparente
manutenção de setters/getters `da_commitments` (ver
`BlockBuilder::set_da_commitments` e `SignedBlock::set_da_commitments`), doc.
Os hosts podem anexar um pacote pré-construído antes de remover um bloco. Todos
os ajudantes lançaram o campeão para `None` tanto que Torii não forneceu pacotes
bobinas.

### 1.3 Fio de codificação

- `SignedBlockWire::canonical_wire()` adiciona o cabeçalho Norito para
  `DaCommitmentBundle` imediatamente após a lista de transações existentes.
  O byte da versão é `0x01`.
- `SignedBlockWire::decode_wire()` rejeita pacotes não `version` est
  inconnue, on-line com a política Norito descrita em `norito.md`.
- Les mises a jour de derivation du hash vivent only in `block::Hasher`;
  clientes legers que decodificam o formato do fio existente gagnent le nouveau
  campeão automaticamente carro le cabeçalho Norito anuncia presença.

## 2. Fluxo de produção de blocos1. A ingestão DA Torii finaliza um `DaIngestReceipt` e publica na fila
   interno (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` coletar todos os recibos não `lane_id` corresponde ao bloco
   em construção, em desduplicação por `(lane_id, client_blob_id,
   manifest_hash)`.
3. Apenas antes do scellage, o construtor tenta os compromissos par `(lane_id, epoch,
   sequência)` para armazenar o hash determinado, codificar o pacote com o codec
   Norito, e conheci um dia `da_commitments_hash`.
4. O pacote completo está armazenado no WSV e emis com o bloco no
   `SignedBlockWire`.

Se a criação do bloco ecoar, as receitas permanecem na fila para que
prochaine tentativo les reprenne; o construtor registra o último `sequence`
inclusive por pista para evitar ataques de repetição.

## 3. Surface RPC e pacotes

Torii expõe três pontos de extremidade:

| Rota | Método | Carga útil | Notas |
|-------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (filtro de faixa de faixa/época/sequência, paginação) | Reenvio `DaCommitmentPage` com total, compromissos e hash de bloco. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (pista + hash de manifesto ou tupla `(epoch, sequence)`). | Responda com `DaCommitmentProof` (registro + caminho Merkle + hash de bloco). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless que rejoue o cálculo do hash do bloco e valida a inclusão; utilize os SDKs que não podem ser direcionados para `iroha_crypto`. |

Todas as cargas úteis vivem sob `iroha_data_model::da::commitment`. Os roteadores
Torii monta os manipuladores na parte final dos endpoints de ingestão de DA existentes para
reutilizar tokens políticos/mTLS.

## 4. Preuves d'inclusion et clientes legers

- O produtor do bloco constrói uma árvore Merkle binária na lista
  serialize des `DaCommitmentRecord`. La racine alimente `da_commitments_hash`.
- `DaCommitmentProof` embale o disco de registro e um vetor de
  `(sibling_hash, position)` para que os verificadores possam reconstruir o
  racina. As preocupações incluem também o hash do bloco e o sinal do cabeçalho para que
  os clientes podem verificar o final.
- Les helpers CLI (`iroha_cli app da prove-commitment`) envolvem o ciclo
  request/verify e expõe as saídas Norito/hex para os operadores.

## 5. Armazenamento e indexação

Le WSV armazena os compromissos em uma coluna familiar dediee, cle par
`manifest_hash`. Os índices secundários cobrem `(lane_id, epoch)` et
`(lane_id, sequence)` para evitar que os pacotes sejam digitalizados
completos. Chaque record suit la hauteur de bloc qui l'a scelle, permettant aux
noeuds en rattrapage para reconstruir o índice rapidamente a partir do log do bloco.

## 6. Telemetria e observabilidade

- `torii_da_commitments_total` incrementa des qu'un bloco scelle au menos un
  registro.
- `torii_da_commitment_queue_depth` atende aos recibos e está atento ao pacote
  (pista par).
- Le painel Grafana `dashboards/grafana/da_commitments.json` visualizar
  a inclusão de blocos, o nível de profundidade da fila e a taxa de transferência de teste para
  que os portões de liberação DA-3 podem auditar o comportamento.

## 7. Estratégia de testes

1. **Testes unitários** para codificação/decodificação de `DaCommitmentBundle` e outros
   mises um dia de derivação de hash de bloco.
2. **Fixtures golden** sob `fixtures/da/commitments/` capturando os bytes
   canonices du bundle et les preuves Merkle.
3. **Testes de integração** demarrant deux validadores, ingerindo blobs de
   teste e verifique se as duas noeus concordam com o conteúdo do pacote e
   as respostas de consulta/prova.
4. **Testes light-client** em `integration_tests/tests/da/commitments.rs`
   (Rust) chamado `/prove` e verifique a prova sem falar em Torii.
5. **Smoke CLI** com `scripts/da/check_commitments.sh` para cuidar da limpeza
   operador reproduzível.

## 8. Plano de implementação| Fase | Descrição | Critérios de saída |
|-------|------------|---------------|
| P0 - Mesclagem do modelo de dados | Integre `DaCommitmentRecord`, atualiza o cabeçalho do bloco e os codecs Norito. | `cargo test -p iroha_data_model` verde com novos acessórios. |
| P1 - Núcleo de Fiação/WSV | Cablagem lógica de fila + construtor de bloco, persiste os índices e expõe os manipuladores RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` passou com asserções de prova de pacote. |
| P2 - Operador de ferramenta | Livre helpers CLI, painel Grafana e alguns documentos de verificação de prova. | `iroha_cli app da prove-commitment` funciona no devnet; o painel exibe dados ao vivo. |
| P3 - Porta de governo | Ativar o validador de blocos requer os compromissos DA nas faixas sinalizadas em `iroha_config::nexus`. | Entree de status + update de roadmap marquent DA-3 como TERMINE. |

## Perguntas abertas

1. **Padrões KZG vs Merkle** - Doit-on toujours ignore os compromissos KZG para
   os pequenos blobs para reduzir a cauda dos blocos? Proposição: jardineiro
   Opção `kzg_commitment` e gater via `iroha_config::da.enable_kzg`.
2. **Lacunas na sequência** - Autorize-t-on des lanes hors ordre? O plano atual foi rejeitado
   as lacunas salvam se o governo ativo `allow_sequence_skips` para uma repetição
   de urgência.
3. **Light-client cache** - O equipamento SDK exige um cache SQLite leger para os
   provas; siga em frente sob DA-8.

Responda a essas perguntas nos PRs de implementação fera passer DA-3 de
BROUILLON (ce document) a EN COURS des que le travail de code start.