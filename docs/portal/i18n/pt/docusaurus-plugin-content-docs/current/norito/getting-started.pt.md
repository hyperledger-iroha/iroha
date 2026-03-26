---
lang: pt
direction: ltr
source: docs/portal/docs/norito/getting-started.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Primeiros passos com Norito

Este guia rápido mostra o fluxo mínimo para compilar um contrato Kotodama, executar o bytecode Norito gerado, executá-lo localmente e fazer deploy em um nodo Iroha.

## Pré-requisitos

1. Instale o toolchain Rust (1.76 ou mais recente) e faça checkout deste repositório.
2. Compile ou baixe os binários de suporte:
   - `koto_compile` - compilador Kotodama que emite bytecode IVM/Norito
   - `ivm_run` e `ivm_tool` - utilitários de execução local e inspeção
   - `iroha_cli` - usado para implantar contratos via Torii

   O Makefile do repositório espera esses binários no `PATH`. Você pode baixar artigos pré-compilados ou compilados a partir do código fonte. Se compilar um conjunto de ferramentas localmente, aponte os helpers do Makefile para os binários:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Garanta que um nodo Iroha esteja rodando quando chegar na etapa de implantação. Os exemplos abaixo assumem que Torii está acessível na URL definida no perfil `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compilar um contrato Kotodama

O repositório inclui um contrato mínimo "hello world" em `examples/hello/hello.ko`. Compilar o bytecode Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Chave das bandeiras:

- `--abi 1` fixa o contrato na versão ABI 1 (única suportada no momento).
- `--max-cycles 0` solicitação de execução sem limite; defina um número positivo para limitar o preenchimento de ciclos para provas de conhecimento zero.

## 2. Inspeção de artistas Norito (opcional)

Use `ivm_tool` para verificar o cabecalho e os metadados embutidos:

```sh
ivm_tool inspect target/examples/hello.to
```

Você deve ver a versão ABI, os flags habilitados e os pontos de entrada exportados. É uma verificação rápida antes do deploy.

## 3. Execute o contrato localmente

Execute o bytecode com `ivm_run` para confirmar o comportamento sem tocar um nó:

```sh
ivm_run target/examples/hello.to --args '{}'
```

O exemplo `hello` registra uma saudação e emite um syscall `SET_ACCOUNT_DETAIL`. Executar localmente e ser útil enquanto você itera na lógica do contrato antes de publicá-lo on-chain.

## 4. Implantação Faca via `iroha_cli`

Quando você estiver satisfeito com o contrato, faça o deploy em um nó usando a CLI. Forneça uma conta de autoridade, sua chave de assinatura e um arquivo `.to` ou payload Base64:

```sh
iroha_cli app contracts deploy \
  --authority soraカタカナ... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

O comando envia um pacote de manifesto Norito + bytecode via Torii e imprime o status da transação resultante. Depois que a transação for confirmada, o hash do código exibido na resposta pode ser usado para recuperar manifestos ou listar instâncias:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Execute contra Torii

Com o bytecode registrado, você pode invocá-lo enviando uma instrução que referencie o código armazenado (por exemplo, via `iroha_cli ledger transaction submit` ou pelo cliente de sua aplicação). Garanta que as permissões da conta permitam os syscalls desejados (`set_account_detail`, `transfer_asset`, etc.).

## Dicas e solução de problemas- Use `make examples-run` para compilar e executar os exemplos de uma vez. Substitua as variáveis ​​de ambiente `KOTO`/`IVM` se os binários não estiverem no `PATH`.
- Se `koto_compile` rejeitar a versão ABI, verifique se o compilador e o nodo miram ABI v1 (rode `koto_compile --abi` sem argumentos para listar o suporte).
- O CLI aceita chaves de assinatura em hexadecimal ou Base64. Para testes, você pode usar chaves emitidas por `iroha_cli tools crypto keypair`.
- Ao depurar payloads Norito, o subcomando `ivm_tool disassemble` ajuda a correlacionar instruções com o código fonte Kotodama.

Este fluxo reflete os passos usados ​​em CI e testes de integração. Para um mergulho mais profundo na gramatica Kotodama, nos mapeamentos de syscalls e nos internos de Norito, veja:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`