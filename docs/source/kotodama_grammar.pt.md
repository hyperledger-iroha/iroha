---
lang: pt
direction: ltr
source: docs/source/kotodama_grammar.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9d64b88546c924258ef054d8071b38230f3f19c8a7d920f9594b0ecb84252ce
source_last_modified: "2025-12-04T09:32:10.286919+00:00"
translation_last_reviewed: 2026-01-05
---

# Gramática e semântica da linguagem Kotodama

Este documento especifica a sintaxe da linguagem Kotodama (lexing, gramática), as regras de tipagem, a semântica determinística e como os programas são baixados para bytecode IVM (`.to`) com convenções de pointer-ABI do Norito. As fontes Kotodama usam a extensão `.ko`. O compilador emite bytecode IVM (`.to`) e pode opcionalmente devolver um manifesto.

Conteúdo
- Visão geral e objetivos
- Estrutura léxica
- Tipos e literais
- Declarações e módulos
- Contêiner de contrato e metadados
- Funções e parâmetros
- Instruções
- Expressões
- Builtins e construtores pointer-ABI
- Coleções e mapas
- Iteração determinística e limites
- Erros e diagnósticos
- Mapeamento de geração de código para IVM
- ABI, cabeçalho e manifesto
- Roteiro

## Visão geral e objetivos

- Determinístico: programas devem produzir resultados idênticos em qualquer hardware; sem ponto flutuante ou fontes não determinísticas. Todas as interações com o host ocorrem via syscalls com argumentos codificados em Norito.
- Portável: visa bytecode Iroha Virtual Machine (IVM), não uma ISA física. Codificações no estilo RISC‑V visíveis no repositório são detalhes de implementação do decodificador IVM e não devem alterar o comportamento observável.
- Auditável: semântica pequena e explícita; mapeamento claro da sintaxe para opcodes da IVM e syscalls do host.
- Limitação: loops sobre dados não limitados devem trazer limites explícitos. A iteração de mapas tem regras estritas para garantir determinismo.

## Estrutura léxica

Espaços em branco e comentários
- Espaços em branco separam tokens e são, caso contrário, insignificantes.
- Comentários de linha começam com `//` e vão até o fim da linha.
- Comentários de bloco `/* ... */` não são aninhados.

Identificadores
- Começam com `[A-Za-z_]` e continuam com `[A-Za-z0-9_]*`.
- Sensíveis a maiúsculas e minúsculas; `_` é um identificador válido, mas desencorajado.

Palavras-chave (reservadas)
- `seiyaku`, `hajimari`, `kotoage`, `kaizen`, `state`, `struct`, `fn`, `let`, `const`, `return`, `if`, `else`, `while`, `for`, `in`, `break`, `continue`, `true`, `false`, `permission`, `kotoba`.

Operadores e pontuação
- Aritméticos: `+ - * / %`
- Bit a bit: `& | ^ ~`, shifts `<< >>`
- Comparação: `== != < <= > >=`
- Lógicos: `&& || !`
- Atribuição: `= += -= *= /= %= &= |= ^= <<= >>=`
- Diversos: `: , ; . :: ->`
- Parênteses: `() [] {}`

Literais
- Inteiro: decimal (`123`), hex (`0x2A`), binário (`0b1010`). Todos os inteiros são assinados de 64 bits em tempo de execução; literais sem sufixo são tipados por inferência ou como `int` por padrão.
- String: aspas duplas com escapes `\`; UTF‑8.
- Booleano: `true`, `false`.

## Tipos e literais

Tipos escalares
- `int`: 64 bits em complemento de dois; aritmética faz wrap módulo 2^64 para soma/sub/mul; divisão tem variantes com e sem sinal definidas na IVM; o compilador escolhe a operação apropriada.
- `bool`: valor lógico; reduzido para `0`/`1`.
- `string`: string UTF‑8 imutável; representada como TLV Norito ao passar para syscalls; dentro da VM usa fatias de bytes e tamanho.
- `bytes`: payload Norito bruto; alias do tipo `Blob` do pointer-ABI para entradas de hash/cripto/prova e overlays duráveis.

Tipos compostos
- `struct Name { field: Type, ... }` tipos produto definidos pelo usuário. Construtores usam sintaxe de chamada `Name(a, b, ...)` em expressões. Acesso a campo `obj.field` é suportado e reduzido internamente a campos posicionais estilo tupla. O ABI de estado durável on-chain é codificado em Norito; o compilador emite overlays que refletem a ordem do struct e testes recentes (`crates/iroha_core/tests/kotodama_struct_overlay.rs`) mantêm o layout fixo entre versões.
- `Map<K, V>`: mapa associativo determinístico; a semântica restringe iteração e mutações durante a iteração (ver abaixo).
- `Tuple (T1, T2, ...)`: tipo produto anônimo com campos posicionais; usado para multi-retorno.

Tipos especiais pointer-ABI (voltados ao host)
- `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`, `Blob` e similares não são tipos de primeira classe em tempo de execução. São construtores que geram ponteiros tipados e imutáveis para a região INPUT (envelopes TLV Norito) e só podem ser usados como argumentos de syscall ou movidos entre variáveis sem mutação.

Inferência de tipos
- Ligações locais `let` inferem o tipo a partir do inicializador. Parâmetros de função devem ser tipados explicitamente. Tipos de retorno podem ser inferidos, salvo ambiguidade.

## Declarações e módulos

Itens de nível superior
- Contratos: `seiyaku Name { ... }` contêm funções, estado, structs e metadados.
- Múltiplos contratos por arquivo são permitidos, mas desencorajados; um `seiyaku` principal é usado como entrada padrão em manifestos.
- Declarações `struct` definem tipos de usuário dentro de um contrato.

Visibilidade
- `kotoage fn` denota um ponto de entrada público; a visibilidade afeta permissões do dispatcher, não a geração de código.

## Contêiner de contrato e metadados

Sintaxe
```
seiyaku Name {
  meta {
    abi_version: 1,
    vector_length: 0,
    max_cycles: 0,
    features: ["zk", "simd"],
  }

  state int counter;

  hajimari() { counter = 0; }

  kotoage fn inc() { counter = counter + 1; }
}
```

Semântica
- `meta { ... }` sobrescreve os padrões do compilador para o cabeçalho IVM emitido: `abi_version`, `vector_length` (0 significa não definido), `max_cycles` (0 significa padrão do compilador), `features` ativa bits de recurso do cabeçalho (tracing ZK, anúncio vetorial). Recursos não suportados são ignorados com aviso. Quando `meta {}` é omitido, o compilador emite `abi_version = 1` e usa os padrões das opções para os demais campos do cabeçalho.
- `features: ["zk", "simd"]` (aliases: `"vector"`) solicita explicitamente os bits de cabeçalho correspondentes. Strings de recursos desconhecidas agora geram erro de parser em vez de serem ignoradas.
- `state` declara variaveis de contrato duraveis. O compilador reduz os acessos a syscalls `STATE_GET/STATE_SET/STATE_DEL`, e o host os guarda em um overlay por transacao (checkpoint/restore para rollback, flush no commit para WSV). Access hints sao emitidos para caminhos literais; chaves dinamicas caem para conflitos em nivel de mapa. Para leituras/escritas explicitas do host, use `state_get/state_set/state_del` e os helpers de mapa `get_or_insert_default`; passam por TLVs Norito e mantem nomes/ordem de campos estaveis.
- Identificadores `state` são reservados; sombrear um nome `state` em parâmetros ou `let` é rejeitado (`E_STATE_SHADOWED`).
- Valores de mapas de estado não são de primeira classe: use o identificador de estado diretamente para operações de mapa e iteração. Vincular ou passar mapas de estado para funções definidas pelo usuário é rejeitado (`E_STATE_MAP_ALIAS`).
- Mapas de estado duráveis atualmente suportam apenas tipos de chave `int` e pointer-ABI; outros tipos de chave são rejeitados em tempo de compilação.
- Campos de estado durável devem ser `int`, `bool`, `Json`, `Blob`/`bytes` ou tipos pointer-ABI (incluindo structs/tuplas compostos por esses campos); `string` não é suportado para estado durável.

## Declarações de gatilhos

As declarações de gatilhos anexam metadados de agendamento aos manifestos de pontos de entrada e
são registradas automaticamente quando uma instância de contrato é ativada (removidas na
desativação). Elas são analisadas dentro de um bloco `seiyaku`.

Sintaxe
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

Notas
- `call` deve referenciar uma entrada pública `kotoage fn` no mesmo contrato; um
  `namespace::entrypoint` opcional é registrado no manifesto, mas callbacks entre contratos são
  rejeitados por enquanto (apenas callbacks locais).
- Filtros suportados: `time pre_commit` e `time schedule(start_ms, period_ms?)`, mais
  `execute trigger <name>` para gatilhos por chamada, `data any` para eventos de dados e filtros
  de pipeline (`pipeline transaction`, `pipeline block`, `pipeline merge`, `pipeline witness`).
- Valores de metadata devem ser literais JSON (`string`, `number`, `bool`, `null`) ou `json!(...)`.
- Chaves de metadata injetadas pelo runtime: `contract_namespace`, `contract_id`,
  `contract_entrypoint`, `contract_code_hash`, `contract_trigger_id`.

## Funções e parâmetros

Sintaxe
- Declaração: `fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- Pública: `kotoage fn name(...) { ... }`
- Inicializador: `hajimari() { ... }` (invocado no deploy pelo runtime, não pela VM em si).
- Gancho de upgrade: `kaizen(args...) permission(Role) { ... }`.

Parâmetros e retornos
- Argumentos são passados em registradores `r10..r22` como valores ou ponteiros INPUT (TLV Norito) conforme o ABI; argumentos adicionais derramam para a pilha.
- Funções retornam zero ou um escalar ou tupla. O valor de retorno principal fica em `r10` para escalar; tuplas são materializadas na pilha/OUTPUT por convenção.

## Instruções

- Ligações de variável: `let x = expr;`, `let mut x = expr;` (mutabilidade é verificada em compilação; mutação em runtime é permitida apenas para locais).
- Atribuição: `x = expr;` e formas compostas `x += 1;` etc. Alvos devem ser variáveis ou índices de mapa; campos de tuplas/structs são imutáveis.
- Controle: `if (cond) { ... } else { ... }`, `while (cond) { ... }`, `for (init; cond; step) { ... }` estilo C.
  - Inicializadores e passos de `for` devem ser `let name = expr` simples ou instruções de expressão; destructuring complexo é rejeitado (`E0005`, `E0006`).
  - Escopo de `for`: ligações da cláusula init são visíveis no loop e após ele; ligações criadas no corpo ou no passo não escapam do loop.
- Igualdade (`==`, `!=`) é suportada para `int`, `bool`, `string`, escalares pointer-ABI (p. ex., `AccountId`, `Name`, `Blob`/`bytes`, `Json`); tuplas, structs e mapas não são comparáveis.
- Loop de mapa: `for (k, v) in map { ... }` (determinístico; ver abaixo).
- Fluxo: `return expr;`, `break;`, `continue;`.
- Chamada: `name(args...);` ou `call name(args...);` (ambas aceitas; o compilador normaliza para instruções de chamada).
- Asserções: `assert(cond);`, `assert_eq(a, b);` mapeiam para `ASSERT*` da IVM em builds não‑ZK ou restrições ZK em modo ZK.

## Expressões

Precedência (alta → baixa)
1. Membro/índice: `a.b`, `a[b]`
2. Unário: `! ~ -`
3. Multiplicativo: `* / %`
4. Aditivo: `+ -`
5. Shifts: `<< >>`
6. Relacional: `< <= > >=`
7. Igualdade: `== !=`
8. AND/XOR/OR bit a bit: `& ^ |`
9. AND/OR lógico: `&& ||`
10. Ternário: `cond ? a : b`

Chamadas e tuplas
- Chamadas usam argumentos posicionais: `f(a, b, c)`.
- Literal de tupla: `(a, b, c)` e destructuring: `let (x, y) = pair;`.
- Destructuring de tupla requer tipos tuple/struct com aridade correspondente; incompatibilidades são rejeitadas.

Strings e bytes
- Strings são UTF‑8; funções que exigem bytes brutos aceitam ponteiros `Blob` via construtores (ver Builtins).

## Builtins e construtores pointer-ABI

Construtores de ponteiro (emitem TLV Norito no INPUT e retornam um ponteiro tipado)
- `account_id(string) -> AccountId*`
- `asset_definition(string) -> AssetDefinitionId*`
- `asset_id(string) -> AssetId*`
- `domain(string) | domain_id(string) -> DomainId*`
- `name(string) -> Name*`
- `json(string) -> Json*`
- `nft_id(string) -> NftId*`
- `blob(bytes|string) -> Blob*`
- `norito_bytes(bytes|string) -> NoritoBytes*`
- `dataspace_id(string|0xhex) -> DataSpaceId*`
- `axt_descriptor(string|0xhex) -> AxtDescriptor*`
- `asset_handle(string|0xhex) -> AssetHandle*`
- `proof_blob(string|0xhex) -> ProofBlob*`

As macros do prelúdio fornecem aliases mais curtos e validação inline para esses construtores:
- `account!("ih58...")`, `account_id!("ih58...")`
- `asset_definition!("rose#wonderland")`, `asset_id!("rose#wonderland")`
- `domain!("wonderland")`, `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` ou literais estruturados como `json!{ hello: "world" }`
- `nft_id!("dragon$demo")`, `blob!("bytes")`, `norito_bytes!("...")`

As macros expandem para os construtores acima e rejeitam literais inválidos em tempo de compilação.

Status de implementação
- Implementado: os construtores acima aceitam argumentos de string literal e são reduzidos para envelopes TLV Norito tipados colocados na região INPUT. Retornam ponteiros tipados imutáveis utilizáveis como argumentos de syscall. Expressões de string não literais são rejeitadas; use `Blob`/`bytes` para entradas dinâmicas. `blob`/`norito_bytes` também aceitam valores `bytes` em runtime sem macros.
- Formas estendidas:
  - `json(Blob[NoritoBytes]) -> Json*` via syscall `JSON_DECODE`.
  - `name(Blob[NoritoBytes]) -> Name*` via syscall `NAME_DECODE`.
  - Decodificação de ponteiros a partir de Blob/NoritoBytes: qualquer construtor de ponteiro (incluindo tipos AXT) aceita um payload `Blob`/`NoritoBytes` e reduz para `POINTER_FROM_NORITO` com o id de tipo esperado.
  - Pass-through para formas pointer: `name(Name) -> Name*`, `blob(Blob) -> Blob*`, `norito_bytes(Blob) -> Blob*`.
  - Açúcar de método é suportado: `s.name()`, `s.json()`, `b.blob()`, `b.norito_bytes()`.

Builtins de host/syscall (mapeiam para SCALL; números exatos em ivm.md)
- `mint_asset(AccountId*, AssetDefinitionId*, numeric)`
- `burn_asset(AccountId*, AssetDefinitionId*, numeric)`
- `transfer_asset(AccountId*, AccountId*, AssetDefinitionId*, numeric)`
- `set_account_detail(AccountId*, Name*, Json*)`
- `nft_mint_asset(NftId*, AccountId*)`
- `nft_transfer_asset(AccountId*, NftId*, AccountId*)`
- `nft_set_metadata(NftId*, Json*)`
- `nft_burn_asset(NftId*)`
- `authority() -> AccountId*`
- `register_domain(DomainId*)`
- `unregister_domain(DomainId*)`
- `transfer_domain(AccountId*, DomainId*, AccountId*)`
- `vrf_verify(Blob, Blob, Blob, int variant) -> Blob`
- `vrf_verify_batch(Blob) -> Blob`
- `axt_begin(AxtDescriptor*)`
- `axt_touch(DataSpaceId*, Blob[NoritoBytes]? manifest)`
- `verify_ds_proof(DataSpaceId*, ProofBlob?)`
- `use_asset_handle(AssetHandle*, Blob[NoritoBytes], ProofBlob?)`
- `axt_commit()`
- `contains(Map<K,V>, K) -> bool`

Builtins utilitários
- `info(string|int)`: emite um evento/mensagem estruturado via OUTPUT.
- `hash(blob) -> Blob*`: retorna um hash codificado em Norito como Blob.
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk) -> Blob*` e `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk) -> Blob*`: construtores ISI inline; todos os argumentos devem ser literais em tempo de compilação (literais de string ou construtores de ponteiro a partir de literais). `nullifier32` e `inputs32` devem ter exatamente 32 bytes (string crua ou hex `0x`), e `amount` deve ser não negativo.
- `schema_info(Name*) -> Json* { "id": "<hex>", "version": N }`
- `pointer_to_norito(ptr) -> NoritoBytes*`: envolve um TLV pointer-ABI existente como NoritoBytes para armazenamento ou transporte.
- `isqrt(int) -> int`: raiz quadrada inteira (`floor(sqrt(x))`) implementada como opcode IVM.
- `min(int, int) -> int`, `max(int, int) -> int`, `abs(int) -> int`, `div_ceil(int, int) -> int`, `gcd(int, int) -> int`, `mean(int, int) -> int` — helpers aritméticos fundidos apoiados por opcodes nativos da IVM (divisão teto faz trap em divisão por zero).

Notas
- Builtins são shims finos; o compilador os reduz para movimentos de registradores e um `SCALL`.
- Construtores de ponteiro são puros: a VM garante que o TLV Norito em INPUT é imutável durante a chamada.
 - Structs com campos pointer‑ABI (p. ex., `DomainId`, `AccountId`) podem ser usados para agrupar argumentos de syscall de forma ergonômica. O compilador mapeia `obj.field` para o registrador/valor correto sem alocações extras.

## Coleções e mapas

Tipo: `Map<K, V>`
- Mapas em memória (alocados no heap via `Map::new()` ou passados como parâmetros) armazenam um único par chave/valor; chaves e valores devem ser tipos de tamanho de palavra: `int`, `bool`, `string`, `Blob`, `bytes`, `Json` ou tipos de ponteiro (p. ex., `AccountId`, `Name`).
- Mapas de estado duráveis (`state Map<...>`) usam chaves/valores codificados em Norito. Chaves suportadas: `int` ou tipos de ponteiro. Valores suportados: `int`, `bool`, `Json`, `Blob`/`bytes` ou tipos de ponteiro.
- `Map::new()` aloca e zera a entrada única em memória (chave/valor = 0); para mapas não `Map<int,int>`, forneça uma anotação de tipo explícita ou tipo de retorno.
- Mapas de estado não são valores de primeira classe: você não pode reatribuí-los (p. ex., `M = Map::new()`); atualize entradas via indexação (`M[key] = value`).
- Operações:
  - Indexação: `map[key]` obtém/define valor (set é feito via syscall do host; ver mapeamento da API de runtime).
  - Existência: `contains(map, key) -> bool` (helper reduzido; pode ser um syscall intrínseco).
  - Iteração: `for (k, v) in map { ... }` com ordem determinística e regras de mutação.

Regras de iteração determinística
- O conjunto de iteração é o snapshot das chaves na entrada do loop.
- A ordem é estritamente lexicográfica ascendente dos bytes de chaves codificadas em Norito.
- Modificações estruturais (inserir/remover/limpar) no mapa iterado durante o loop causam um trap determinístico `E_ITER_MUTATION`.
- Limitação é exigida: ou um máximo declarado (`@max_len`) no mapa, um atributo explícito `#[bounded(n)]`, ou um limite explícito usando `.take(n)`/`.range(..)`; caso contrário o compilador emite `E_UNBOUNDED_ITERATION`.

Helpers de limites
- `#[bounded(n)]`: atributo opcional na expressão de mapa, p. ex. `for (k, v) in my_map #[bounded(2)] { ... }`.
- `.take(n)`: itera as primeiras `n` entradas desde o início.
- `.range(start, end)`: itera entradas no intervalo semiaberto `[start, end)`. A semântica equivale a `start` e `n = end - start`.

Notas sobre limites dinâmicos
- Limites literais: `n`, `start` e `end` como literais inteiros são totalmente suportados e compilam para um número fixo de iterações.
- Limites não literais: quando o recurso `kotodama_dynamic_bounds` está habilitado no crate `ivm`, o compilador aceita expressões dinâmicas `n`, `start` e `end` e insere asserções em runtime para segurança (não negativos, `end >= start`). O lowering emite até K iterações guardadas com `if (i < n)` para evitar execuções extras do corpo (K padrão = 2). Você pode ajustar K programaticamente via `CompilerOptions { dynamic_iter_cap, .. }`.
- Execute `koto_lint` para inspecionar avisos de lint Kotodama antes da compilação; o compilador principal sempre prossegue com o lowering após parsing e checagem de tipos.
- Códigos de erro estão documentados em [Kotodama Compiler Error Codes](./kotodama_error_codes.md); use `koto_compile --explain <code>` para explicações rápidas.

## Erros e diagnósticos

Diagnósticos em tempo de compilação (exemplos)
- `E_UNBOUNDED_ITERATION`: loop sobre mapa sem limite.
- `E_MUT_DURING_ITER`: mutação estrutural do mapa iterado no corpo do loop.
- `E_STATE_SHADOWED`: ligações locais não podem sombrear declarações `state`.
- `E_BREAK_OUTSIDE_LOOP`: `break` usado fora de um loop.
- `E_CONTINUE_OUTSIDE_LOOP`: `continue` usado fora de um loop.
- `E0005`: inicializador do for é mais complexo do que o suportado.
- `E0006`: cláusula step do for é mais complexa do que o suportado.
- `E_BAD_POINTER_USE`: uso do resultado de um construtor pointer-ABI onde é requerido um tipo de primeira classe.
- `E_UNRESOLVED_NAME`, `E_TYPE_MISMATCH`, `E_ARITY_MISMATCH`, `E_DUP_SYMBOL`.
- Ferramentas: `koto_compile` executa o lint antes de emitir bytecode; use `--no-lint` para pular ou `--deny-lint-warnings` para falhar o build ao ver output de lint.

Erros de runtime da VM (selecionados; lista completa em ivm.md)
- `E_NORITO_INVALID`, `E_OOB`, `E_UNALIGNED`, `E_SCALL_UNKNOWN`, `E_ASSERT`, `E_ASSERT_EQ`, `E_ITER_MUTATION`.

Mensagens de erro
- Diagnósticos carregam `msg_id`s estáveis que mapeiam para entradas nas tabelas de tradução `kotoba {}` quando disponíveis.

## Mapeamento de geração de código para IVM

Pipeline
1. Lexer/Parser produzem o AST.
2. A análise semântica resolve nomes, checa tipos e popula tabelas de símbolos.
3. IR lowering para uma forma simples tipo SSA.
4. Alocação de registradores para GPRs da IVM (`r10+` para args/ret por convenção); spills para a pilha.
5. Emissão de bytecode: mistura de codificações nativas da IVM e compatíveis com RV quando útil; cabeçalho de metadados emitido com `abi_version`, features, comprimento de vetor e `max_cycles`.

Destaques do mapeamento
- Aritmética e lógica mapeiam para ops ALU da IVM.
- Branching e controle mapeiam para branches condicionais e jumps; o compilador usa formas comprimidas quando vantajoso.
- Memória para locais é spillada para a pilha da VM; alinhamento é imposto.
- Builtins são reduzidos a movimentos de registradores e `SCALL` com número de 8 bits.
- Construtores de ponteiro colocam TLVs Norito na região INPUT e produzem seus endereços.
- Asserções mapeiam para `ASSERT`/`ASSERT_EQ`, que trap em execução não‑ZK e emitem restrições em builds ZK.

Restrições de determinismo
- Sem FP; sem syscalls não determinísticos.
- Aceleração SIMD/GPU é invisível ao bytecode e deve ser bit‑idêntica; o compilador não emite ops específicas de hardware.

## ABI, cabeçalho e manifesto

Campos de cabeçalho IVM definidos pelo compilador
- `version`: versão do formato de bytecode IVM (major.minor).
- `abi_version`: versão da tabela de syscalls e do esquema pointer-ABI.
- `feature_bits`: flags de recurso (p. ex., `ZK`, `VECTOR`).
- `vector_len`: comprimento lógico do vetor (0 → não definido).
- `max_cycles`: limite de admissão e dica de padding ZK.

Manifesto (sidecar opcional)
- `code_hash`, `abi_hash`, metadados do bloco `meta {}`, versão do compilador e dicas de build para reprodutibilidade.

## Roteiro

- **KD-231 (Abr 2026):** adicionar análise de faixa em tempo de compilação para limites de iteração para que loops exponham conjuntos de acesso limitados ao scheduler.
- **KD-235 (Mai 2026):** introduzir um escalar `bytes` de primeira classe distinto de `string` para construtores de ponteiro e clareza de ABI.
- **KD-242 (Jun 2026):** expandir o conjunto de opcodes builtins (hash / verificação de assinatura) atrás de flags de recurso com fallbacks determinísticos.
- **KD-247 (Jun 2026):** estabilizar `msg_id`s de erro e manter o mapeamento em tabelas `kotoba {}` para diagnósticos localizados.
### Emissão de manifesto

- A API do compilador Kotodama pode devolver um `ContractManifest` junto com o `.to` compilado via `ivm::kotodama::compiler::Compiler::compile_source_with_manifest`.
- Campos:
  - `code_hash`: hash dos bytes de código (excluindo o cabeçalho IVM e literais) computado pelo compilador para vincular o artefato.
  - `abi_hash`: digest estável da superfície de syscalls permitida para o `abi_version` do programa (ver `ivm.md` e `ivm::syscalls::compute_abi_hash`).
- `compiler_fingerprint` e `features_bitmap` opcionais são reservados para toolchains.
- `entrypoints`: lista ordenada de entrypoints exportados (públicos, `hajimari`, `kaizen`) incluindo suas strings `permission(...)` requeridas e os melhores esforços do compilador em pistas de chave de leitura/escrita para que admissão e schedulers possam raciocinar sobre o acesso esperado ao WSV.
- O manifesto é destinado a verificações de admissão e registries; veja `docs/source/new_pipeline.md` para o ciclo de vida.
