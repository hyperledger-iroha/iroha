---
lang: pt
direction: ltr
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T10:01:27.059307+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Design de gadget de transferência FastPQ

# Visão geral

O planejador FASTPQ atual registra cada operação primitiva envolvida em uma instrução `TransferAsset`, o que significa que cada transferência paga pela aritmética de saldo, rodadas de hash e atualizações SMT separadamente. Para reduzir linhas de rastreamento por transferência, introduzimos um gadget dedicado que verifica apenas as verificações mínimas de aritmética/comprometimento enquanto o host continua a executar a transição de estado canônico.

- **Escopo**: transferências únicas e pequenos lotes emitidos por meio da superfície de syscall Kotodama/IVM `TransferAsset` existente.
- **Objetivo**: reduzir o espaço ocupado pelas colunas FFT/LDE para transferências de alto volume, compartilhando tabelas de pesquisa e reduzindo a aritmética por transferência em um bloco de restrição compacto.

# Arquitetura

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## Formato da transcrição

O host emite um `TransferTranscript` por invocação de syscall:

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` vincula a transcrição ao hash do ponto de entrada da transação para proteção de reprodução.
- `authority_digest` é o hash do host sobre assinantes/dados de quorum classificados; o gadget verifica a igualdade, mas não refaz a verificação da assinatura. Concretamente, o host Norito codifica o `AccountId` (que já incorpora o controlador multisig canônico) e faz o hash de `b"iroha:fastpq:v1:authority|" || encoded_account` com Blake2b-256, armazenando o `Hash` resultante.
- `poseidon_preimage_digest` = Poseidon(conta_de || conta_para || ativo || valor || batch_hash); garante que o gadget recalcule o mesmo resumo que o host. Os bytes de pré-imagem são construídos como `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` usando a codificação Norito simples antes de passá-los pelo auxiliar Poseidon2 compartilhado. Este resumo está presente para transcrições de delta único e omitido para lotes multi-delta.

Todos os campos são serializados via Norito para que as garantias de determinismo existentes sejam mantidas.
Tanto `from_path` quanto `to_path` são emitidos como blobs Norito usando o
Esquema `TransferMerkleProofV1`: `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`.
Versões futuras podem estender o esquema enquanto o provador impõe a tag de versão
antes da decodificação. Os metadados `TransitionBatch` incorporam a transcrição codificada Norito
vetor sob a chave `transfer_transcripts` para que o provador possa decodificar a testemunha
sem realizar consultas fora de banda. Entradas públicas (`dsid`, `slot`, raízes,
`perm_root`, `tx_set_hash`) são transportados em `FastpqTransitionBatch.public_inputs`,
deixando metadados para contabilidade de contagem de hash/transcrição de entrada. Até o encanamento do host
terras, o provador deriva sinteticamente provas dos pares chave/equilíbrio, de modo que as linhas
sempre inclua um caminho SMT determinístico mesmo quando a transcrição omitir os campos opcionais.

## Layout do gadget

1. **Bloco Aritmético de Equilíbrio**
   - Entradas: `from_balance_before`, `amount`, `to_balance_before`.
   - Verificações:
     - `from_balance_before >= amount` (gadget de alcance com decomposição RNS compartilhada).
     -`from_balance_after = from_balance_before - amount`.
     -`to_balance_after = to_balance_before + amount`.
   - Embalado em uma porta personalizada para que todas as três equações consumam um grupo de linhas.2. **Bloco de Compromisso Poseidon**
   - Recalcula `poseidon_preimage_digest` usando a tabela de pesquisa Poseidon compartilhada já usada em outros gadgets. Nenhuma rodada de Poseidon por transferência no rastreamento.

3. **Bloco de Caminho Merkle**
   - Estende o gadget Kaigi SMT existente com um modo de "atualização emparelhada". Duas folhas (remetente, destinatário) compartilham a mesma coluna para hashes irmãos, reduzindo linhas duplicadas.

4. **Verificação do resumo da autoridade**
   - Restrição de igualdade simples entre o resumo fornecido pelo host e o valor da testemunha. As assinaturas permanecem em seu gadget dedicado.

5. **Loop em lote**
   - Os programas chamam `transfer_v1_batch_begin()` antes de um loop de construtores `transfer_asset` e `transfer_v1_batch_end()` depois. Enquanto o escopo está ativo, o host armazena em buffer cada transferência e as reproduz como um único `TransferAssetBatch`, reutilizando o contexto Poseidon/SMT uma vez por lote. Cada delta adicional adiciona apenas a aritmética e duas verificações de folhas. O decodificador de transcrição agora aceita lotes multi-delta e os apresenta como `TransferGadgetInput::deltas` para que o planejador possa dobrar as testemunhas sem reler Norito. Contratos que já possuem uma carga útil Norito útil (por exemplo, CLI/SDKs) podem pular totalmente o escopo chamando `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)`, que entrega ao host um lote totalmente codificado em uma syscall.

# Mudanças no Host e no Provador| Camada | Mudanças |
|-------|---------|
| `ivm::syscalls` | Adicione `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) para que os programas possam agrupar vários syscalls `transfer_v1` sem emitir ISIs intermediários, além de `transfer_v1_batch_apply` (`0x2B`) para lotes pré-codificados. |
| `ivm::host` e testes | Os hosts principais/padrão tratam `transfer_v1` como um acréscimo em lote enquanto o escopo está ativo, superfície `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` e o host WSV simulado armazena em buffer as entradas antes de confirmar, para que os testes de regressão possam afirmar o equilíbrio determinístico atualizações.【crates/ivm/src/core_host.rs:1001】【crates/ivm/src/host.rs:451】【crates/ivm/src/mock_wsv.rs :3713】【crates/ivm/tests/wsv_host_pointer_tlv.rs:219】【crates/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | Emita `TransferTranscript` após a transição de estado, crie registros `FastpqTransitionBatch` com `public_inputs` explícito durante `StateBlock::capture_exec_witness` e execute a pista do provador FASTPQ para que as ferramentas Torii/CLI e o back-end Stage6 recebam canônico Entradas `TransitionBatch`. `TransferAssetBatch` agrupa transferências sequenciais em uma única transcrição, omitindo o resumo poseidon para lotes multi-delta para que o gadget possa iterar entre as entradas de forma determinística. |
| `fastpq_prover` | `gadgets::transfer` agora valida transcrições multi-delta (aritmética de equilíbrio + resumo Poseidon) e apresenta testemunhas estruturadas (incluindo blobs SMT emparelhados com espaço reservado) para o planejador (`crates/fastpq_prover/src/gadgets/transfer.rs`). `trace::build_trace` decodifica essas transcrições de metadados de lote, rejeita lotes de transferência sem a carga útil `transfer_transcripts`, anexa as testemunhas validadas a `Trace::transfer_witnesses` e `TracePolynomialData::transfer_plan()` mantém o plano agregado ativo até que o planejador consuma o gadget (`crates/fastpq_prover/src/trace.rs`). O chicote de regressão de contagem de linhas agora é fornecido via `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`), cobrindo cenários de até 65.536 linhas preenchidas, enquanto a fiação SMT emparelhada permanece atrás do marco auxiliar de lote TF-3 (os espaços reservados mantêm o layout de rastreamento estável até que a troca seja concluída). |
| Kotodama | Reduz o auxiliar `transfer_batch((from,to,asset,amount), …)` em `transfer_v1_batch_begin`, chamadas `transfer_asset` sequenciais e `transfer_v1_batch_end`. Cada argumento da tupla deve seguir o formato `(AccountId, AccountId, AssetDefinitionId, int)`; transferências únicas mantêm o construtor existente. |

Exemplo de uso de Kotodama:

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` executa a mesma permissão e verificações aritméticas que chamadas `Transfer::asset_numeric` individuais, mas registra todos os deltas dentro de um único `TransferTranscript`. As transcrições multi-delta eliminam o resumo do poseidon até que os compromissos por delta cheguem a um acompanhamento. O construtor Kotodama agora emite syscalls de início/fim automaticamente, para que os contratos possam implementar transferências em lote sem codificação manual de cargas úteis Norito.

## Chicote de regressão de contagem de linhas

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) sintetiza lotes de transição FASTPQ com contagens de seletores configuráveis e relata o resumo `row_usage` resultante (`total_rows`, contagens por seletor, proporção) junto com o comprimento/log₂ preenchido. Capture benchmarks para o teto de 65.536 linhas com:

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```O JSON emitido espelha os artefatos de lote FASTPQ que `iroha_cli audit witness` agora emite por padrão (passe `--no-fastpq-batches` para suprimi-los), portanto, `scripts/fastpq/check_row_usage.py` e a porta CI podem comparar as execuções sintéticas com instantâneos anteriores ao validar alterações do planejador.

# Plano de implementação

1. **TF-1 (encanamento de transcrição)**: ✅ `StateTransaction::record_transfer_transcripts` agora emite transcrições Norito para cada `TransferAsset`/lote, `sumeragi::witness::record_fastpq_transcript` as armazena dentro da testemunha global e `StateBlock::capture_exec_witness` compila `fastpq_batches` com `public_inputs` explícito para operadores e a pista do provador (use `--no-fastpq-batches` se precisar de um mais fino saída).【crates/iroha_core/src/state.rs:8801】【crates/iroha_core/src/sumeragi/witness.rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】【crates/iroha_cli/src/audit.rs:185】
2. **TF-2 (implementação de gadget)**: ✅ `gadgets::transfer` agora valida transcrições multi-delta (aritmética de equilíbrio + resumo Poseidon), sintetiza provas SMT emparelhadas quando os hosts as omitem, expõe testemunhas estruturadas via `TransferGadgetPlan` e `trace::build_trace` encadeia essas testemunhas em `Trace::transfer_witnesses` enquanto preenchendo colunas SMT a partir das provas. `fastpq_row_bench` captura o chicote de regressão de 65536 linhas para que os planejadores rastreiem o uso de linhas sem repetir Norito cargas úteis.【crates/fastpq_prover/src/gadgets/transfer.rs:1】【crates/fastpq_prover/src/trace.rs:1】【crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1】
3. **TF-3 (Batch helper)**: Habilite o syscall em lote + construtor Kotodama, incluindo aplicativo sequencial em nível de host e loop de gadget.
4. **TF-4 (Telemetria e documentos)**: Atualize `fastpq_plan.md`, `fastpq_migration_guide.md` e esquemas de painel para exibir a alocação de linhas de transferência em comparação com outros gadgets.

# Perguntas abertas

- **Limites de domínio**: o planejador FFT atual entra em pânico por rastreamentos além de 2¹⁴ linhas. O TF-2 deve aumentar o tamanho do domínio ou documentar uma meta de benchmark reduzida.
- **Lotes de vários ativos**: o gadget inicial assume o mesmo ID de recurso por delta. Se precisarmos de lotes heterogêneos, devemos garantir que a testemunha Poseidon inclua o ativo todas as vezes para evitar a reprodução entre ativos.
- **Reutilização do resumo de autoridade**: a longo prazo, podemos reutilizar o mesmo resumo para outras operações autorizadas para evitar o recálculo das listas de assinantes por syscall.


Este documento acompanha as decisões de design; mantenha-o sincronizado com as entradas do roteiro quando os marcos chegarem.