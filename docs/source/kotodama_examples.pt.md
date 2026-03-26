---
lang: pt
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T15:34:14.183250+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Visão geral dos exemplos Kotodama

Esta página mostra exemplos concisos de Kotodama e como eles são mapeados para syscalls IVM e argumentos de ponteiro-ABI. Veja também:
- `examples/` para fontes executáveis
- `docs/source/ivm_syscalls.md` para o syscall canônico ABI
- `kotodama_grammar.md` para a especificação completa do idioma

## Olá + Detalhes da conta

Fonte: `examples/hello/hello.ko`

```
seiyaku Hello {
  hajimari() { info("Hello from Kotodama"); }

  kotoage fn write_detail() permission(Admin) {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
```

Mapeamento (ponteiro-ABI):
- `authority()` → `SCALL 0xA4` (o host grava `&AccountId` em `r10`)
- `set_account_detail(a, k, v)` → mover `r10=&AccountId`, `r11=&Name`, `r12=&Json`, depois `SCALL 0x1A`

## Transferência de ativos

Fonte: `examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ"),
      account!("soraゴヂアニラショリャヒャャサピテヶベチュヲボヹヂギタクアニョロホドチャヘヱヤジヶハシャウンベニョャルフハケネキカ"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```

Mapeamento (ponteiro-ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`, então `SCALL 0x24`

## Criação + Transferência NFT

Fonte: `examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ");
    let recipient = account!("soraゴヂアニラショリャヒャャサピテヶベチュヲボヹヂギタクアニョロホドチャヘヱヤジヶハシャウンベニョャルフハケネキカ");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

Mapeamento (ponteiro-ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Ponteiro Norito Auxiliares

O estado durável com valor de ponteiro requer a conversão de TLVs digitados de e para o
Envelope `NoritoBytes` que os hosts persistem. Kotodama agora conecta esses ajudantes
diretamente através do compilador para que os construtores possam usar padrões de ponteiro e mapear
pesquisas sem cola FFI manual:

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("soraゴヂアニラショリャヒャャサピテヶベチュヲボヹヂギタクアニョロホドチャヘヱヤジヶハシャウンベニョャルフハケネキカ");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

Abaixando:

- Os padrões do ponteiro emitem `POINTER_TO_NORITO` após publicar o TLV digitado, então
  o host recebe uma carga canônica `NoritoBytes` para armazenamento.
- As leituras realizam a operação inversa com `POINTER_FROM_NORITO`, fornecendo o
  ID do tipo de ponteiro esperado em `r11`.
- Ambos os caminhos publicam automaticamente TLVs literais na região INPUT, permitindo
  contratos para misturar literais de string e ponteiros de tempo de execução de forma transparente.

Consulte `crates/ivm/tests/kotodama_pointer_args.rs` para uma regressão em tempo de execução que
exerce a viagem de ida e volta contra o `MockWorldStateView`.

## Iteração de mapa determinístico (design)

O mapa determinístico para cada um requer um limite. A iteração de múltiplas entradas requer um mapa de estado; o compilador aceita `.take(n)` ou um comprimento máximo declarado.

```
// design example (iteration requires bounds and state storage)
state M: Map<int, int>;

fn sum_first_two() -> int {
  let s = 0;
  for (k, v) in M.take(2) {
    s = s + v;
  }
  return s;
}
```

Semântica:
- O conjunto de iterações é um instantâneo na entrada do loop; a ordem é lexicográfica por bytes Norito da chave.
- Mutações estruturais para `M` no loop trap com `E_ITER_MUTATION`.
- Sem limite, o compilador emite `E_UNBOUNDED_ITERATION`.

## Internos do compilador/host (Rust, não fonte Kotodama)

Os trechos abaixo ficam no lado Rust do conjunto de ferramentas. Eles ilustram auxiliares de compilador e mecânica de redução de VM e **não** são fontes Kotodama `.ko` válidas.

## Atualizações de quadros fragmentados do Wide Opcode

Os amplos auxiliares de opcode do Kotodama têm como alvo o layout de operando de 8 bits usado pelo IVM
codificação ampla. Cargas e armazenamentos que movem valores de 128 bits reutilizam o terceiro operando
slot para o registro alto, então o registro base já deve conter o registro final
endereço. Ajuste a base com um `ADDI` antes de emitir a carga/armazenamento:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```As atualizações de quadros fragmentados avançam a base em etapas de 16 bytes, garantindo o registro
par confirmado por `STORE128` pousa no limite de alinhamento necessário. O mesmo
o padrão se aplica a `LOAD128`; emitindo um `ADDI` com a passada desejada antes
cada carga mantém o registro de destino alto vinculado ao slot do terceiro operando.
Captura de endereços desalinhados com `VMError::MisalignedAccess`, correspondente à VM
comportamento exercido em `crates/ivm/tests/wide_memory128.rs`.

Os programas que emitem esses auxiliares de 128 bits devem anunciar a capacidade do vetor.
O compilador Kotodama ativa o bit de modo `VECTOR` automaticamente sempre que
`LOAD128`/`STORE128` aparecem; as armadilhas da VM com
`VMError::VectorExtensionDisabled` se um programa tentar executá-los
sem esse bit definido.

## Ampla redução de ramificação condicional

Quando Kotodama reduz um `if`/`else` ou ramificação ternária para bytecode largo, ele emite um
sequência `BNE cond, zero, +2` fixa seguida por um par de instruções `JAL`:

1. O `BNE` curto mantém a ramificação condicional dentro da faixa imediata de 8 bits
   saltando sobre o `JAL`.
2. O primeiro `JAL` tem como alvo o bloco `else` (executado quando a condição é
   falso).
3. O segundo `JAL` salta para o bloco `then` (tomado quando a condição é
   verdade).

Este padrão garante que a verificação da condição nunca precise codificar deslocamentos maiores
de ± 127 palavras, embora ainda suporte corpos arbitrariamente grandes para o `then`
e blocos `else` por meio do amplo auxiliar `JAL`. Veja
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` para
o teste de regressão que bloqueia a sequência.

### Exemplo de redução

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

Compila para o seguinte esqueleto de instruções amplo (números de registro e
os deslocamentos absolutos dependem da função envolvente):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

As instruções subsequentes materializam as constantes e escrevem o valor de retorno.
Como o `BNE` salta sobre o primeiro `JAL`, o deslocamento condicional é sempre
Palavras `+2`, mantendo a ramificação dentro do alcance mesmo quando os corpos do bloco se expandem.