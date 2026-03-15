---
lang: pt
direction: ltr
source: docs/portal/docs/da/commitments-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4404d6ce2d1d937e702d17d79a8c75d9e487a146d947ec76a9d7148d04627776
source_last_modified: "2025-12-09T14:01:56.955866+00:00"
translation_last_reviewed: 2026-01-01
---

:::note Fonte canonica
Espelha `docs/source/da/commitments_plan.md`. Mantenha as duas versoes em
:::

# Plano de compromissos de Data Availability da Sora Nexus (DA-3)

_Redigido: 2026-03-25 -- Responsaveis: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 estende o formato de bloco da Nexus para que cada lane incorpore registros
deterministicos que descrevem os blobs aceitos pelo DA-2. Esta nota captura as
estruturas de dados canonicas, os hooks do pipeline de blocos, as provas de
cliente leve e as superficies Torii/RPC que precisam chegar antes que
validadores possam confiar nos compromissos DA durante admissao ou checks de
governanca. Todos os payloads sao codificados em Norito; sem SCALE ou JSON ad
hoc.

## Objetivos

- Carregar compromissos por blob (chunk root + manifest hash + commitment KZG
  opcional) dentro de cada bloco Nexus para que peers possam reconstruir o estado
  de availability sem consultar storage fora do ledger.
- Fornecer provas de membership deterministicas para que clientes leves
  verifiquem que um manifest hash foi finalizado em um bloco.
- Expor consultas Torii (`/v1/da/commitments/*`) e provas que permitam a relays,
  SDKs e automacao de governanca auditar availability sem reproduzir cada bloco.
- Manter o envelope `SignedBlockWire` canonico ao enfiar as novas estruturas
  pelo header de metadata Norito e a derivacao do hash de bloco.

## Panorama de escopo

1. **Adicoes ao data model** em `iroha_data_model::da::commitment` mais alteracoes
   de header de bloco em `iroha_data_model::block`.
2. **Hooks do executor** para que `iroha_core` ingeste receipts DA emitidos por
   Torii (`crates/iroha_core/src/queue.rs` e `crates/iroha_core/src/block.rs`).
3. **Persistencia/indexes** para que o WSV responda consultas de compromissos
   rapidamente (`iroha_core/src/wsv/mod.rs`).
4. **Adicoes RPC em Torii** para endpoints de lista/consulta/prova sob
   `/v1/da/commitments`.
5. **Tests de integracao + fixtures** validando o wire layout e o fluxo de proof
   em `integration_tests/tests/da/commitments.rs`.

## 1. Adicoes ao data model

### 1.1 `DaCommitmentRecord`

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
- `proof_scheme` deriva do catalogo de lanes; lanes Merkle rejeitam payloads KZG
  enquanto lanes `kzg_bls12_381` exigem commitments KZG nao zero. Torii atualmente
  so produz compromissos Merkle e rejeita lanes configuradas com KZG.
- `KzgCommitment` reutiliza o ponto de 48 bytes usado em `iroha_crypto::kzg`.
  Quando ausente em lanes Merkle, cai para provas Merkle apenas.
- `proof_digest` antecipa a integracao DA-5 PDP/PoTR para que o mesmo record
  enumere o schedule de sampling usado para manter blobs vivos.

### 1.2 Extensao do header de bloco

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

O hash do bundle entra tanto no hash do bloco quanto na metadata de
`SignedBlockWire`. Quando um bloco nao carrega dados DA, o campo fica `None` para

Nota de implementacao: `BlockPayload` e o `BlockBuilder` transparente agora
expoem setters/getters `da_commitments` (ver `BlockBuilder::set_da_commitments`
e `SignedBlock::set_da_commitments`), entao hosts podem anexar um bundle
preconstruido antes de selar um bloco. Todos os helpers deixam o campo em `None`
ate que Torii encadeie bundles reais.

### 1.3 Encoding de wire

- `SignedBlockWire::canonical_wire()` adiciona o header Norito para
  `DaCommitmentBundle` imediatamente apos a lista de transacoes existente. O
  byte de versao e `0x01`.
- `SignedBlockWire::decode_wire()` rejeita bundles cujo `version` seja
  desconhecido, alinhado a politica Norito descrita em `norito.md`.
- Atualizacoes de derivacao de hash existem apenas em `block::Hasher`; clientes
  leves que decodificam o wire format existente ganham o novo campo
  automaticamente porque o header Norito anuncia sua presenca.

## 2. Fluxo de producao de blocos

1. A ingestao DA de Torii finaliza um `DaIngestReceipt` e o publica na fila
   interna (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` coleta todos os receipts cujo `lane_id` corresponde ao bloco
   em construcao, deduplicando por `(lane_id, client_blob_id, manifest_hash)`.
3. Pouco antes de selar, o builder ordena os compromissos por `(lane_id, epoch,
   sequence)` para manter o hash deterministico, codifica o bundle com o codec
   Norito, e atualiza `da_commitments_hash`.
4. O bundle completo e armazenado no WSV e emitido junto com o bloco dentro de
   `SignedBlockWire`.

Se a criacao do bloco falhar, os receipts permanecem na fila para que a proxima

tentativa os capture; o builder registra o ultimo `sequence` incluido por lane
para evitar ataques de replay.

## 3. Superficie RPC e consultas

Torii expoe tres endpoints:

| Rota | Metodo | Payload | Notas |
|------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtro de range por lane/epoch/sequence, paginacao) | Retorna `DaCommitmentPage` com total, compromissos e hash de bloco. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (lane + manifest hash ou tupla `(epoch, sequence)`). | Responde com `DaCommitmentProof` (record + caminho Merkle + hash de bloco). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless que refaz o calculo do hash de bloco e valida a inclusao; usado por SDKs que nao podem linkar direto em `iroha_crypto`. |

Todos os payloads vivem sob `iroha_data_model::da::commitment`. Os routers de
Torii montam os handlers ao lado dos endpoints de ingestao DA existentes para
reutilizar politicas de token/mTLS.

## 4. Provas de inclusao e clientes leves

- O produtor de blocos constroi uma arvore Merkle binaria sobre a lista
  serializada de `DaCommitmentRecord`. A raiz alimenta `da_commitments_hash`.
- `DaCommitmentProof` empacota o record alvo mais um vetor de
  `(sibling_hash, position)` para que verificadores reconstruam a raiz. As
  provas tambem incluem o hash do bloco e o header assinado para que clientes
  leves validem a finality.
- Helpers de CLI (`iroha_cli app da prove-commitment`) envolvem o ciclo de
  request/verify e expoem saidas Norito/hex para operadores.

## 5. Storage e indexacao

O WSV armazena compromissos em uma column family dedicada com chave
`manifest_hash`. Indexes secundarios cobrem `(lane_id, epoch)` e
`(lane_id, sequence)` para que consultas evitem varrer bundles completos. Cada
record rastreia a altura do bloco que o selou, permitindo que nos em catch-up
reconstruam o indice rapidamente a partir do block log.

## 6. Telemetria e observabilidade

- `torii_da_commitments_total` incrementa quando um bloco sela ao menos um
  record.
- `torii_da_commitment_queue_depth` rastreia receipts aguardando bundle (por
  lane).
- O dashboard Grafana `dashboards/grafana/da_commitments.json` visualiza a
  inclusao em blocos, profundidade da fila e throughput de provas para que os
  gates de release da DA-3 possam auditar o comportamento.

## 7. Estrategia de testes

1. **Tests unitarios** para encoding/decoding de `DaCommitmentBundle` e
   atualizacoes de derivacao do hash de bloco.
2. **Fixtures golden** sob `fixtures/da/commitments/` capturando bytes canonicos
   do bundle e provas Merkle.
3. **Tests de integracao** com dois validadores, ingerindo blobs de exemplo e
   verificando que ambos os nos concordam no conteudo do bundle e nas respostas
   de query/proof.
4. **Tests de cliente leve** em `integration_tests/tests/da/commitments.rs`
   (Rust) que chamam `/prove` e verificam a prova sem falar com Torii.
5. **Smoke CLI** com `scripts/da/check_commitments.sh` para manter tooling de
   operadores reproduzivel.

## 8. Plano de rollout

| Fase | Descricao | Criterio de saida |
|------|-----------|-------------------|
| P0 - Merge do data model | Integrar `DaCommitmentRecord`, atualizacoes de header de bloco e codecs Norito. | `cargo test -p iroha_data_model` verde com novas fixtures. |
| P1 - Wiring Core/WSV | Encadear logica de fila + block builder, persistir indexes e expor handlers RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` passam com assertions de bundle proof. |
| P2 - Tooling de operadores | Entregar helpers CLI, dashboard Grafana e updates de docs de verificacao de proof. | `iroha_cli app da prove-commitment` funciona contra devnet; o dashboard mostra dados ao vivo. |
| P3 - Gate de governanca | Habilitar o validador de blocos que requer compromissos DA nas lanes marcadas em `iroha_config::nexus`. | Entrada de status + update de roadmap marcam DA-3 como COMPLETADO. |

## Perguntas abertas

1. **KZG vs Merkle defaults** - Devemos sempre pular commitments KZG em blobs
   pequenos para reduzir o tamanho do bloco? Proposta: manter `kzg_commitment`
   opcional e gatear via `iroha_config::da.enable_kzg`.
2. **Sequence gaps** - Permitimos lanes fora de ordem? O plano atual rejeita gaps
   salvo se a governanca ativar `allow_sequence_skips` para replay de emergencia.
3. **Light-client cache** - O time de SDK pediu um cache SQLite leve para proofs;
   pendente em DA-8.

Responder estas perguntas em PRs de implementacao move DA-3 de RASCUNHO (este
documento) para EM ANDAMENTO quando o trabalho de codigo comecar.
