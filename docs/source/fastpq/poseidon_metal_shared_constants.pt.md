---
lang: pt
direction: ltr
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2026-01-03T18:07:57.621942+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Poseidon Metal Constantes Compartilhadas

Kernels de metal, kernels CUDA, o provador Rust e todos os equipamentos SDK devem compartilhar
exatamente os mesmos parâmetros do Poseidon2 para manter a aceleração por hardware
hash determinístico. Este documento registra o instantâneo canônico, como
regenerá-lo e como se espera que os pipelines de GPU ingiram os dados.

## Manifesto Instantâneo

Os parâmetros são publicados como um documento RON `PoseidonSnapshot`. As cópias são
mantido sob controle de versão para que os conjuntos de ferramentas de GPU e SDKs não dependam do tempo de construção
geração de código.

| Caminho | Finalidade | SHA-256 |
|------|---------|---------|
| `artifacts/offline_poseidon/constants.ron` | Instantâneo canônico gerado a partir de `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}`; fonte de verdade para compilações de GPU. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` | Espelha o instantâneo canônico para que os testes de unidade do Swift e o chicote de fumaça do XCFramework carreguem as mesmas constantes que os kernels do Metal esperam. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | Os dispositivos Android/Kotlin compartilham o manifesto idêntico para testes de paridade e serialização. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

Cada consumidor deve verificar o hash antes de conectar as constantes a uma GPU
gasoduto. Quando o manifesto muda (novo conjunto de parâmetros ou perfil), o SHA e
os espelhos downstream devem ser atualizados em passo de bloqueio.

## Regeneração

O manifesto é gerado a partir das fontes Rust executando o `xtask`
ajudante. O comando grava o arquivo canônico e os espelhos do SDK:

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

Use `--constants <path>`/`--vectors <path>` para substituir os destinos ou
`--no-sdk-mirror` ao regenerar apenas a captura instantânea canônica. O ajudante irá
espelhar os artefatos nas árvores Swift e Android quando o sinalizador for omitido,
que mantém os hashes alinhados para CI.

## Alimentando compilações Metal/CUDA

-`crates/fastpq_prover/metal/kernels/poseidon2.metal` e
  `crates/fastpq_prover/cuda/fastpq_cuda.cu` deve ser regenerado a partir do
  manifesta sempre que a tabela muda.
- Constantes arredondadas e MDS são preparadas em `MTLBuffer`/`__constant` contíguos
  segmentos que correspondem ao layout do manifesto: `round_constants[round][state_width]`
  seguido pela matriz MDS 3x3.
- `fastpq_prover::poseidon_manifest()` carrega e valida o snapshot em
  tempo de execução (durante o aquecimento do metal) para que as ferramentas de diagnóstico possam afirmar que o
  constantes de shader correspondem ao hash publicado via
  `fastpq_prover::poseidon_manifest_sha256()`.
- Leitores de acessórios SDK (Swift `PoseidonSnapshot`, Android `PoseidonSnapshot`) e
  as ferramentas off-line Norito dependem do mesmo manifesto, o que impede apenas GPU
  garfos de parâmetros.

## Validação

1. Após regenerar o manifesto, execute `cargo test -p xtask` para exercitar o
   Testes unitários de geração de dispositivos elétricos Poseidon.
2. Registre o novo SHA-256 neste documento e em qualquer painel que monitore
   Artefatos de GPU.
3. `cargo test -p fastpq_prover poseidon_manifest_consistency` analisa
   `poseidon2.metal` e `fastpq_cuda.cu` no momento da construção e afirma que seus
   constantes serializadas correspondem ao manifesto, mantendo as tabelas CUDA/Metal e
   o instantâneo canônico em lock-step.Manter o manifesto junto com as instruções de construção da GPU fornece ao Metal/CUDA
fluxos de trabalho um aperto de mão determinístico: os kernels são livres para otimizar sua memória
layout, desde que eles ingiram o blob de constantes compartilhadas e exponham o hash em
telemetria para verificações de paridade.