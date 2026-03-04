---
lang: pt
direction: ltr
source: docs/source/da/commitments_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ea1b16b73a55e3e47dfe9d5bfc77dedce2e8fa9ff964d244856767f14931733
source_last_modified: "2026-01-22T15:38:30.660808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Plano de compromissos de disponibilidade de dados Sora Nexus (DA-3)

_Elaborado: 25/03/2026 - Proprietários: Grupo de protocolo principal / Equipe de contrato inteligente / Equipe de armazenamento_

DA-3 estende o formato de bloco Nexus para que cada pista incorpore registros determinísticos
descrevendo os blobs aceitos pelo DA-2. Esta nota captura os dados canônicos
estruturas, ganchos de pipeline de bloco, provas de cliente leve e superfícies Torii/RPC
que deve chegar antes que os validadores possam confiar nos compromissos do DA durante a admissão ou
verificações de governança. Todas as cargas úteis são codificadas em Norito; sem ESCALA ou JSON ad hoc.

## Objetivos

- Carregar compromissos por blob (raiz de bloco + hash de manifesto + KZG opcional
  compromisso) dentro de cada bloco Nexus para que os pares possam reconstruir a disponibilidade
  estado sem consultar o armazenamento off-ledger.
- Fornecer provas de associação determinísticas para que os clientes leves possam verificar se um
  o hash do manifesto foi finalizado em um determinado bloco.
- Expor consultas Torii (`/v1/da/commitments/*`) e provas que permitem relés,
  SDKs e disponibilidade de auditoria de automação de governança sem repetir todos
  bloco.
- Mantenha o envelope `SignedBlockWire` existente canônico, rosqueando o novo
  estruturas por meio do cabeçalho de metadados Norito e derivação de hash de bloco.

## Visão geral do escopo

1. **Adições ao modelo de dados** no bloco `iroha_data_model::da::commitment` plus
   alterações de cabeçalho em `iroha_data_model::block`.
2. **Ganchos do executor** para que `iroha_core` ingira recibos DA emitidos por Torii
   (`crates/iroha_core/src/queue.rs` e `crates/iroha_core/src/block.rs`).
3. **Persistência/índices** para que o WSV possa responder rapidamente a consultas de compromisso
   (`iroha_core/src/wsv/mod.rs`).
4. **Adições de RPC Torii** para listar/consultar/comprovar endpoints em
   `/v1/da/commitments`.
5. **Testes de integração + acessórios** validando o layout da fiação e o fluxo de prova em
   `integration_tests/tests/da/commitments.rs`.

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

- `KzgCommitment` reutiliza o ponto existente de 48 bytes usado em
  `iroha_crypto::kzg`. As pistas Merkle deixam-no vazio; Pistas `kzg_bls12_381` agora
  receber um compromisso determinístico BLAKE3-XOF derivado da raiz do pedaço e
  tíquete de armazenamento para que os hashes de bloco permaneçam estáveis sem um provador externo.
- `proof_scheme` é derivado do catálogo de pistas; Pistas Merkle rejeitam KZG perdido
  cargas úteis, enquanto as pistas `kzg_bls12_381` exigem compromissos KZG diferentes de zero.
- `proof_digest` antecipa a integração DA-5 PDP/PoTR para o mesmo registro
  enumera o cronograma de amostragem usado para manter os blobs ativos.

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

O hash do pacote alimenta o hash do bloco e os metadados `SignedBlockWire`.
sobrecarga.

Nota de implementação: `BlockPayload` e o `BlockBuilder` transparente agora expõem
Setters/getters `da_commitments` (consulte `BlockBuilder::set_da_commitments` e
`SignedBlock::set_da_commitments`), para que os hosts possam anexar um pacote pré-construído
antes de selar um bloco. Todos os construtores auxiliares padronizam o campo para `None`
até que Torii encadeie pacotes reais.

### 1.3 Codificação de fio- `SignedBlockWire::canonical_wire()` anexa o cabeçalho Norito para
  `DaCommitmentBundle` imediatamente após a lista de transações existente. O
  o byte da versão é `0x01`.
- `SignedBlockWire::decode_wire()` rejeita pacotes cujo `version` é desconhecido,
  correspondendo à política Norito descrita em `norito.md`.
- As atualizações de derivação de hash existem apenas em `block::Hasher`; decodificação de clientes leves
  o formato de fio existente ganha automaticamente o novo campo porque o Norito
  cabeçalho anuncia sua presença.

## 2. Bloquear fluxo de produção

1. Torii A ingestão DA persiste recibos assinados e registros de compromisso no
   Carretel DA (`da-receipt-*.norito` / `da-commitment-*.norito`). O durável
   o registro de recebimento semeia cursores na reinicialização para que os recibos reproduzidos ainda sejam ordenados
   determinísticamente.
2. A montagem do bloco carrega os recibos do carretel, deixa cair obsoletos/já selados
   entradas usando o instantâneo do cursor confirmado e impõe contiguidade por
   `(lane, epoch)`. Se um recibo acessível não tiver um compromisso correspondente ou o
   o hash manifesto diverge, a proposta é abortada em vez de omiti-la silenciosamente.
3. Logo antes de selar, o construtor corta o pacote de compromisso ao
   conjunto orientado a recibos, classifica por `(lane_id, epoch, sequence)`, codifica o
   empacotado com o codec Norito e atualiza `da_commitments_hash`.
4. O pacote completo é armazenado no WSV e emitido junto com o bloco dentro
   `SignedBlockWire`; pacotes comprometidos avançam os cursores de recebimento (hidratados
   do Kura na reinicialização) e remover entradas de spool obsoletas para limitar o crescimento do disco.

A montagem do bloco e a ingestão `BlockCreated` revalidam cada compromisso em relação
o catálogo de pistas: as pistas Merkle rejeitam compromissos perdidos da KZG, as pistas KZG exigem um
compromisso KZG diferente de zero e `chunk_root` diferente de zero, e faixas desconhecidas são
caiu. O endpoint `/v1/da/commitments/verify` do Torii espelha a mesma proteção,
e ingerir agora envolve o compromisso determinístico da KZG em cada
Registro `kzg_bls12_381` para que pacotes configuráveis em conformidade com a política alcancem a montagem do bloco.

Os acessórios de manifesto descritos no plano de ingestão DA-2 também funcionam como fonte de
verdade para o empacotador de compromisso. O teste Torii
`manifest_fixtures_cover_all_blob_classes` regenera manifestos para cada
Variante `BlobClass` e se recusa a compilar até que novas classes ganhem equipamentos,
garantindo que o hash do manifesto codificado dentro de cada `DaCommitmentRecord` corresponda ao
par dourado Norito/JSON.【crates/iroha_torii/src/da/tests.rs:2902】

Se a criação do bloco falhar, os recibos permanecerão na fila, então o próximo bloco
tentativa pode pegá-los; o construtor registra o último `sequence` incluído por
pista para evitar ataques de repetição.

## 3. RPC e superfície de consulta

Torii expõe três pontos de extremidade:| Rota | Método | Carga útil | Notas |
|-------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (filtro de intervalo por faixa/época/sequência, paginação) | Retorna `DaCommitmentPage` com contagem total, compromissos e hash de bloco. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (pista + hash de manifesto ou tupla `(epoch, sequence)`). | Responde com `DaCommitmentProof` (registro + caminho Merkle + hash de bloco). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Auxiliar sem estado que reproduz o cálculo do hash do bloco e valida a inclusão; usado por SDKs que não podem ser vinculados diretamente ao `iroha_crypto`. |

Todas as cargas úteis residem em `iroha_data_model::da::commitment`. Montagem de roteadores Torii
os manipuladores próximos aos endpoints de ingestão de DA existentes para reutilizar token/mTLS
políticas.

## 4. Provas de inclusão e clientes leves

- O produtor do bloco constrói uma árvore Merkle binária sobre o serializado
  Lista `DaCommitmentRecord`. A raiz alimenta `da_commitments_hash`.
- `DaCommitmentProof` empacota o registro de destino mais um vetor de `(sibling_hash,
  position)` para que os verificadores possam reconstruir a raiz. As provas também incluem
  o hash do bloco e o cabeçalho assinado para que clientes leves possam verificar a finalidade.
- Auxiliares CLI (`iroha_cli app da prove-commitment`) envolvem a solicitação/verificação de prova
  saídas de ciclo e superfície Norito/hex para operadores.

## 5. Armazenamento e indexação

O WSV armazena compromissos em uma família de colunas dedicada codificada por `manifest_hash`.
Os índices secundários cobrem `(lane_id, epoch)` e `(lane_id, sequence)`, portanto, consultas
evite digitalizar pacotes completos. Cada registro rastreia a altura do bloco que o selou,
permitindo que os nós de atualização reconstruam o índice rapidamente a partir do log de bloco.

## 6. Telemetria e Observabilidade

- `torii_da_commitments_total` incrementa sempre que um bloco sela pelo menos um
  registro.
- `torii_da_commitment_queue_depth` rastreia recibos aguardando para serem agrupados (por
  pista).
- Painel Grafana `dashboards/grafana/da_commitments.json` visualiza bloco
  inclusão, profundidade da fila e rendimento de prova para que os portões de liberação DA-3 possam auditar
  comportamento.

## 7. Estratégia de teste

1. **Testes de unidade** para codificação/decodificação `DaCommitmentBundle` e hash de bloco
   atualizações de derivação.
2. **Golden fixtures** sob `fixtures/da/commitments/` capturando canônico
   agrupar bytes e provas Merkle. Cada pacote faz referência aos bytes do manifesto
   de `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}`, então
   regenerando `cargo test -p iroha_torii regenerate_da_ingest_fixtures -- --ignored --nocapture`
   mantém a história Norito consistente antes que `ci/check_da_commitments.sh` atualize o compromisso
   provas.【fixtures/da/ingest/README.md:1】
3. **Testes de integração** inicializando dois validadores, ingerindo blobs de amostra e
   afirmando que ambos os nós concordam com o conteúdo do pacote e consulta/prova
   respostas.
4. **Testes de cliente leve** em `integration_tests/tests/da/commitments.rs`
   (Rust) que liga para `/prove` e verifica a prova sem falar com Torii.
5. **CLI smoke** script `scripts/da/check_commitments.sh` para manter o operador
   ferramental reproduzível.

## 8. Plano de implementação| Fase | Descrição | Critérios de saída |
|-------|------------|---------------|
| P0 — Mesclagem de modelos de dados | Land `DaCommitmentRecord`, atualizações de cabeçalho de bloco e codecs Norito. | `cargo test -p iroha_data_model` verde com novas luminárias. |
| P1 — Fiação Core/WSV | Fila de threads + lógica do construtor de blocos, persistem índices e expõem manipuladores RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` passam com asserções à prova de pacote. |
| P2 — Ferramentas do operador | Envie ajudantes CLI, painel Grafana e atualizações de documentos de verificação de prova. | `iroha_cli app da prove-commitment` funciona contra devnet; painel exibe dados ao vivo. |
| P3 — Portão de governança | Habilite o validador de bloco que exige compromissos de DA nas pistas sinalizadas em `iroha_config::nexus`. | Entrada de status + atualização do roteiro marcam DA-3 como 🈴. |

## Perguntas abertas

1. **Padrões KZG vs Merkle** – As pequenas bolhas devem sempre ignorar os compromissos KZG para
   reduzir o tamanho do bloco? Proposta: manter `kzg_commitment` opcional e gate via
   `iroha_config::da.enable_kzg`.
2. **Lacunas na sequência** — Permitimos faixas fora de ordem? Plano atual rejeita lacunas
   a menos que a governança alterne `allow_sequence_skips` para reprodução de emergência.
3. **Cache de cliente leve** — A equipe do SDK solicitou um cache SQLite leve para
   provas; acompanhamento pendente sob DA-8.

Responder a isso nos PRs de implementação move DA-3 de 🈸 (este documento) para 🈺
assim que o trabalho do código começar.