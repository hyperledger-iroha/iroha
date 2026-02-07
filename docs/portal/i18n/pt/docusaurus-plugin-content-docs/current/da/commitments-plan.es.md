---
lang: pt
direction: ltr
source: docs/portal/docs/da/commitments-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Reflexo `docs/source/da/commitments_plan.md`. Mantenha ambas as versões em
:::

# Plano de compromissos de disponibilidade de dados do Sora Nexus (DA-3)

_Redatado: 2026-03-25 -- Responsáveis: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 estende o formato do bloco Nexus para que cada pista incruste registros
deterministas que descrevem os blobs aceitos pelo DA-2. Esta nota captura as
estruturas de dados canônicas, ganchos de pipeline de blocos, testes de
cliente leve e as superfícies Torii/RPC que devem ser aterradas antes de que as
validadores podem confiar em compromissos DA durante a admissão ou cheques de
governança. Todas as cargas úteis estão codificadas em Norito; sin SCALE e anúncio JSON
hoc.

## Objetivos

- Levar compromissos por blob (chunk root + manifest hash + commit KZG
  opcional) dentro de cada bloco Nexus para que os pares possam reconstruir o
  estado de disponibilidade sem consultar o armazenamento fora do razão.
- Fornecer testes de membresia deterministas para que clientes leves verifiquem
  que um hash de manifesto foi finalizado em um bloco fornecido.
- Expor consultas Torii (`/v1/da/commitments/*`) e verificar se você pode permitir
  relés, SDKs e automatização de governança auditar disponibilidade sem reprodução
  cada bloco.
- Mantenha o envelope `SignedBlockWire` canônico para limpar as novas
  estruturas através do cabeçalho de metadados Norito e a derivação do hash de
  bloco.

## Panorama de alcance

1. **Adições ao modelo de dados** em `iroha_data_model::da::commitment` mas
   mudanças de cabeçalho de bloco em `iroha_data_model::block`.
2. **Ganchos do executor** para que `iroha_core` receba recibos DA emitidos por
   Torii (`crates/iroha_core/src/queue.rs` e `crates/iroha_core/src/block.rs`).
3. **Persistência/índices** para que o WSV responda consultas de compromissos
   rápido (`iroha_core/src/wsv/mod.rs`).
4. **Adições RPC em Torii** para endpoints da lista/consulta/verifique abaixo
   `/v1/da/commitments`.
5. **Testes de integração + acessórios** validando o layout do fio e o fluxo de
   prova em `integration_tests/tests/da/commitments.rs`.

## 1. Adicionados ao modelo de dados

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
  Quando você estiver ausente, você precisará apenas de provas Merkle.
- `proof_scheme` é derivado do catálogo de pistas; las lanes Merkle rechazan
  cargas úteis KZG enquanto as pistas `kzg_bls12_381` exigem compromissos KZG
  não, zero. Torii atualmente produz apenas compromissos Merkle e rechaza lanes
  configurações com KZG.
- `KzgCommitment` reutiliza o ponto de 48 bytes usado em `iroha_crypto::kzg`.
  Quando você está ausente nas pistas Merkle, ele se volta apenas para as provas Merkle.
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

O hash do pacote entra tanto no hash do bloco como nos metadados de
`SignedBlockWire`. Quando um bloco não leva dados DA o campo permanece `None`

Nota de implementação: `BlockPayload` e o transparente `BlockBuilder` agora
exponen setters/getters `da_commitments` (ver `BlockBuilder::set_da_commitments`
e `SignedBlock::set_da_commitments`), assim como os hosts podem adicionar um pacote
pré-construído antes de vender um bloco. Todos os construtores helper dejan el
campo em `None` até que Torii enhebre pacotes reais.

### 1.3 Codificação de fio

- `SignedBlockWire::canonical_wire()` adiciona o cabeçalho Norito para
  `DaCommitmentBundle` imediatamente após a lista de transações
  existente. O byte da versão é `0x01`.
- `SignedBlockWire::decode_wire()` rechaza bundles cuyo `version` é desconhecido,
  seguindo a política Norito descrita em `norito.md`.
- As atualizações de derivação de hash vivem apenas em `block::Hasher`; eles
  clientes leves que decodificam o formato de fio existente ganham o novo campo
  automaticamente porque o cabeçalho Norito anuncia sua presença.

## 2. Fluxo de produção de blocos1. A ingestão DA de Torii finaliza um `DaIngestReceipt` e o publica na cola
   interno (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` recopila todos os recibos cujo `lane_id` coincide com o
   bloco em construção, desduplicando por `(lane_id, client_blob_id,
   manifest_hash)`.
3. Justo antes de vender, o construtor ordena os compromissos por `(lane_id,
   época, sequência)` para manter o hash determinista, codificar o pacote com
   o codec Norito, e atualiza `da_commitments_hash`.
4. O pacote completo é armazenado no WSV e emitido junto com o bloco dentro do
   `SignedBlockWire`.

Se a criação do bloco falhar, os recibos permanecerão na cola para que o
siga a intenção do tomo; el construtor registra o último `sequence` incluído
por lane para evitar ataques de repetição.

## 3. Superfície RPC e consulta

Torii expõe três pontos de extremidade:

| Rota | Método | Carga útil | Notas |
|------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtro por faixa de faixa/época/sequência, paginação) | Devolva `DaCommitmentPage` com total, compromissos e hash de bloco. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (pista + hash de manifesto ou tupla `(epoch, sequence)`). | Responder com `DaCommitmentProof` (registro + ruta Merkle + hash de bloco). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Helper stateless que reejecuta o cálculo do hash de bloco e valida a inclusão; usado por SDKs que não podem ser enviados diretamente para `iroha_crypto`. |

Todas as cargas úteis vivem abaixo de `iroha_data_model::da::commitment`. Os roteadores de
Torii monta os manipuladores junto com os endpoints de ingestão DA existentes para
reutilizar políticas de token/mTLS.

## 4. Testes de inclusão e clientes leves

- O produtor de blocos constrói um binário Merkle de árvore na lista
  serializada de `DaCommitmentRecord`. A raiz alimentar `da_commitments_hash`.
- `DaCommitmentProof` empaque o registro objetivo mas um vetor de
  `(sibling_hash, position)` para que os verificadores reconstruam a raiz. Las
  as tentativas também incluem o hash de bloco e o cabeçalho firmado para que
  clientes leves verifiquem a finalidade.
- Helpers de CLI (`iroha_cli app da prove-commitment`) envolvem o ciclo de
  solicitação/verificação de testes e exposições de saídas Norito/hex para
  operadores.

## 5. Armazenamento e indexação

El WSV almacena compromete-se em uma família de colunas dedicada com chave
`manifest_hash`. Os índices secundários cubren `(lane_id, epoch)` y
`(lane_id, sequence)` para que as consultas evitem verificar pacotes completos.
Cada registro rastreia a altura do bloco que você vende, permitindo a nós em
catch-up reconstruir o índice rapidamente a partir do log do bloco.

## 6. Telemetria e observabilidade

- `torii_da_commitments_total` incrementa quando um bloco sela pelo menos um
  registro.
- `torii_da_commitment_queue_depth` rastrea recibos esperando ser empaquetados
  (por pista).
- El painel Grafana `dashboards/grafana/da_commitments.json` visualizar la
  inclusão em blocos, profundidade de cola e rendimento de testes para que
  Os portões de liberação do DA-3 podem auditar o comportamento.

## 7. Estratégia de teste

1. **Testes unitários** para codificação/decodificação de `DaCommitmentBundle` e
   atualizações de derivação de hash de bloco.
2. **Fixtures golden** abaixo de `fixtures/da/commitments/` que captura bytes
   canônicos do pacote e testes de Merkle.
3. **Testes de integração** levantando dos validadores, ingiriendo blobs de
   mostre e verifique se ambos os nós coincidem no conteúdo do pacote e
   as respostas de consulta/teste.
4. **Testes de cliente leve** em `integration_tests/tests/da/commitments.rs`
   (Rust) que chama `/prove` e verifica o teste sem falar com Torii.
5. **Smoke de CLI** com `scripts/da/check_commitments.sh` para manter ferramentas
   de operadores reproduzíveis.

## 8. Plano de implementação| Fase | Descrição | Critério de saída |
|------|-------------|--------------------|
| P0 - Mesclar modelo de dados | Integra `DaCommitmentRecord`, atualizações de cabeçalho de bloco e codecs Norito. | `cargo test -p iroha_data_model` em verde com novas luminárias. |
| P1 - Núcleo Cableado/WSV | Aprenda lógica de cola + construtor de blocos, persista índices e manipuladores de expoentes RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` passam com afirmações de prova de pacote. |
| P2 - Ferramentas de operadores | Auxiliares Lanzar de CLI, painel Grafana e atualizações de documentos de verificação de prova. | `iroha_cli app da prove-commitment` funciona contra devnet; o painel mostra dados ao vivo. |
| P3 - Portão de governo | Habilite o validador de blocos que exigem compromissos DA nas pistas marcadas em `iroha_config::nexus`. | Entrada de status + atualização de roadmap marcan DA-3 como COMPLETADO. |

## Perguntas abertas

1. **KZG vs Merkle defaults** - Devemos omitir compromissos KZG em pequenos blobs
   para reduzir o tamanho do bloco? Proposta: mantenedor `kzg_commitment`
   opcionalmente e gate via `iroha_config::da.enable_kzg`.
2. **Lacunas na sequência** - Permitimos lanes fuera de orden? El plano real rechaza
   lacunas salvo que governo ativo `allow_sequence_skips` para replay de
   emergência.
3. **Light-client cache** - O equipamento SDK pidio un cache SQLite liviano para
   provas; seguimento pendente baixo DA-8.

Responder a estas perguntas em PRs de implementação mueve DA-3 de BORRADOR (este
documento) a EN PROGRESO quando o trabalho de código começa.