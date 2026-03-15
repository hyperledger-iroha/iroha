---
lang: pt
direction: ltr
source: docs/portal/docs/da/commitments-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Espelha `docs/source/da/commitments_plan.md`. Mantenha as duas versoes em
:::

# Plano de compromissos de Disponibilidade de Dados da Sora Nexus (DA-3)

_Redigido: 2026-03-25 -- Responsáveis: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 estende o formato do bloco Nexus para que cada pista incorpore registros
determinísticos que descrevem os blobs aceitos pelo DA-2. Esta nota captura as
estruturas de dados canônicas, os ganchos do pipeline de blocos, as provas de
cliente leve e as superfícies Torii/RPC que precisam chegar antes que
validadores podem confiar nos compromissos DA durante a admissão ou verificações de
governança. Todos os payloads são codificados em Norito; sem SCALE ou anúncio JSON
hoc.

## Objetivos

- Carregar compromissos por blob (chunk root + manifest hash + comprometimento KZG
  opcional) dentro de cada bloco Nexus para que os pares possam reconstruir o estado
  de disponibilidade sem consultar armazenamento fora do ledger.
- Fornecer provas de adesão determinísticas para que os clientes levem
  verifique se um hash de manifesto foi finalizado em um bloco.
- Exporte consultas Torii (`/v2/da/commitments/*`) e tente permitir relés,
  SDKs e automação de governança auditam a disponibilidade sem reproduzir cada bloco.
- Manter o envelope `SignedBlockWire` canônico ao enfiar as novas estruturas
  pelo cabeçalho de metadados Norito e a derivação do hash do bloco.

## Panorama de escopo

1. **Adições ao modelo de dados** em `iroha_data_model::da::commitment` mais alterações
   do cabeçalho do bloco em `iroha_data_model::block`.
2. **Hooks do executor** para que `iroha_core` ingeste recibos DA emitidos por
   Torii (`crates/iroha_core/src/queue.rs` e `crates/iroha_core/src/block.rs`).
3. **Persistência/índices** para que o WSV responda consultas de compromissos
   rapidamente (`iroha_core/src/wsv/mod.rs`).
4. **Adições RPC em Torii** para endpoints da lista/consulta/prova sob
   `/v2/da/commitments`.
5. **Testes de integração + luminárias** validando o layout da fiação e o fluxo de prova
   em `integration_tests/tests/da/commitments.rs`.

## 1. Adicoes ao modelo de dados

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

- `KzgCommitment` reutiliza o ponto de 48 bytes usado em `iroha_crypto::kzg`.
  Quando ausente, cai para provas Merkle apenas.
- `proof_scheme` derivado do catálogo de pistas; lanes Merkle rejeitam cargas úteis KZG
  enquanto lanes `kzg_bls12_381` desativar compromissos KZG não zero. Torii atualmente
  assim produz compromissos Merkle e rejeita faixas definidas com KZG.
- `KzgCommitment` reutiliza o ponto de 48 bytes usado em `iroha_crypto::kzg`.
  Quando ausente em pistas Merkle, cai para provas Merkle apenas.
- `proof_digest` antecipa a integração DA-5 PDP/PoTR para que o mesmo registro
  enumere o cronograma de amostragem usado para manter blobs vivos.

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

O hash do pacote entra tanto no hash do bloco quanto na metadados de
`SignedBlockWire`. Quando um bloco não carrega dados DA, o campo fica `None` para

Nota de implementação: `BlockPayload` e o `BlockBuilder` transparente agora
setters/getters expoem `da_commitments` (ver `BlockBuilder::set_da_commitments`
e `SignedBlock::set_da_commitments`), então hosts podem anexar um pacote
pré-construído antes de selar um bloco. Todos os ajudantes deixam o campo em `None`
ate que Torii encadeie pacotes reais.

### 1.3 Codificação de fio

- `SignedBlockWire::canonical_wire()` adiciona o cabeçalho Norito para
  `DaCommitmentBundle` imediatamente após uma lista de transações existentes. Ó
  byte de versão e `0x01`.
- `SignedBlockWire::decode_wire()` pacotes rejeitados cujo `version` seja
  desconhecido, alinhado a política Norito descrita em `norito.md`.
- Atualizações de derivação de hash existem apenas em `block::Hasher`; clientes
  folhas que decodificam o formato wire existente ganham o novo campo
  automaticamente porque o cabeçalho Norito anuncia sua presença.

## 2. Fluxo de produção de blocos

1. A ingestão DA de Torii finaliza um `DaIngestReceipt` e o público na fila
   interno (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` coleta todos os recibos cujo `lane_id` corresponde ao bloco
   em construção, desduplicando por `(lane_id, client_blob_id, manifest_hash)`.
3. Pouco antes de selar, o construtor ordena os compromissos por `(lane_id, epoch,
   sequência)` para manter o hash determinístico, codificar o bundle com o codec
   Norito, e atualiza `da_commitments_hash`.
4. O pacote completo e armazenado no WSV e emitido junto com o bloco dentro de
   `SignedBlockWire`.

Se a criação do bloco falhar, as receitas permanecem na fila para que a próximatentativa de captura; o construtor registra o último `sequence` incluído por lane
para evitar ataques de repetição.

## 3. Superfície RPC e consultas

Torii expõe três endpoints:

| Rota | Método | Carga útil | Notas |
|------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (filtro de intervalo por faixa/época/sequência, paginação) | Retorno `DaCommitmentPage` com total, compromissos e hash de bloco. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (pista + hash de manifesto ou tupla `(epoch, sequence)`). | Responde com `DaCommitmentProof` (record + caminho Merkle + hash de bloco). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless que refaz o cálculo do hash de bloco e valida a inclusão; usado por SDKs que não podem linkar direto em `iroha_crypto`. |

Todas as cargas úteis vivem sob `iroha_data_model::da::commitment`. Os roteadores de
Torii montam os handlers ao lado dos endpoints de ingestão DA existentes para
reutilizar políticas de token/mTLS.

## 4. Provas de inclusão e níveis de clientes

- O produtor de blocos constrói uma árvore Merkle binária sobre a lista
  serializada de `DaCommitmentRecord`. A raiz alimentar `da_commitments_hash`.
- `DaCommitmentProof` embalagem o registro alvo mais um vetor de
  `(sibling_hash, position)` para que verificadores reconstruam a raiz. Como
  Os testes também incluem o hash do bloco e o cabeçalho assinado para que clientes
  níveis validem uma finalidade.
- Helpers de CLI (`iroha_cli app da prove-commitment`) envolvendo o ciclo de
  request/verify e expoem saidas Norito/hex para operadores.

## 5. Armazenamento e indexação

O WSV armazena compromissos em uma coluna familiar dedicada com chave
`manifest_hash`. Índices secundários cobrem `(lane_id, epoch)` e
`(lane_id, sequence)` para que consultas evitem varrer bundles completos. Cada
gravar rastreia a altura do bloco que o selou, permitindo que nos em catch-up
reconstruímos o índice rapidamente a partir do log do bloco.

## 6. Telemetria e observabilidade

- `torii_da_commitments_total` incrementa quando um bloco sela ao menos um
  registro.
- `torii_da_commitment_queue_depth` rastreia recibos aguardando pacote (por
  pista).
- O painel Grafana `dashboards/grafana/da_commitments.json` visualiza um
  inclusão em blocos, profundidade da fila e rendimento de provas para que os
  gates de release do DA-3 podem auditar o comportamento.

## 7. Estratégia de testes

1. **Testes unitários** para codificação/decodificação de `DaCommitmentBundle` e
   atualizações de derivação do hash de bloco.
2. **Fixtures golden** sob `fixtures/da/commitments/` capturando bytes canônicos
   do bundle e provas Merkle.
3. **Testes de integração** com dois validadores, inserindo blobs de exemplo e
   verificando que ambos os nossos concordam no conteúdo do pacote e nas respostas
   de consulta/prova.
4. **Testes de nível de cliente** em `integration_tests/tests/da/commitments.rs`
   (Rust) que chamam `/prove` e verificamos a prova sem falar com Torii.
5. **Smoke CLI** com `scripts/da/check_commitments.sh` para manter as ferramentas de
   Operador reproduzível.

## 8. Plano de implementação

| Fase | Descrição | Critério de disseda |
|------|-----------|-------------------|
| P0 - Mesclagem do modelo de dados | Integrar `DaCommitmentRecord`, atualizações de cabeçalho de bloco e codecs Norito. | `cargo test -p iroha_data_model` verde com novas luminárias. |
| P1 - Núcleo de Fiação/WSV | Encadear lógica de fila + construtor de blocos, persistir índices e exportar manipuladores RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` passam com assertions de bundle proof. |
| P2 - Ferramentas de operadores | Entregar helpers CLI, dashboard Grafana e atualizações de documentos de verificação de prova. | `iroha_cli app da prove-commitment` funciona contra devnet; o painel mostra dados ao vivo. |
| P3 - Portão de governança | Habilitar o validador de blocos que requer compromissos DA nas pistas marcadas em `iroha_config::nexus`. | Entrada de status + atualização de roadmap marcam DA-3 como COMPLETADO. |

## Perguntas abertas

1. **KZG vs Merkle defaults** - devemos sempre pular compromissos KZG em blobs
   pequenos para reduzir o tamanho do bloco? Proposta: manter `kzg_commitment`
   opcional e gateway via `iroha_config::da.enable_kzg`.
2. **Lacunas de sequência** - Permitimos pistas fora de ordem? O plano atual rejeita lacunas
   salvo se a governança ativar `allow_sequence_skips` para repetição de emergência.
3. **Light-client cache** - O time do SDK pediu um cache SQLite leve para provas;
   pendente em DA-8.

Responder estas perguntas em PRs de implementação da mudança DA-3 de RASCUNHO (este
documento) para EM ANDAMENTO quando o trabalho de código comecar.