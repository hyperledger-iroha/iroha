---
lang: pt
direction: ltr
source: docs/portal/docs/norito/getting-started.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Prêmio principal de Norito

Este guia rápido apresenta o fluxo de trabalho mínimo para compilar um contrato Kotodama, inspecionar o bytecode Norito gerado, executar o local e implantar em um novo Iroha.

## Pré-requisito

1. Instale o conjunto de ferramentas Rust (1.76 ou mais) e recupere este depósito.
2. Construa ou baixe os binários de suporte:
   - `koto_compile` - compilador Kotodama que emite o bytecode IVM/Norito
   - `ivm_run` e `ivm_tool` - utilitários de local de execução e inspeção
   - `iroha_cli` - utilizado para implantação de contratos via Torii

   O Makefile do depósito atende esses binários em `PATH`. Você pode baixar artefatos pré-compilados ou construí-los a partir das fontes. Se você compilar o local do conjunto de ferramentas, indique os auxiliares do Makefile para os binários:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Certifique-se de que um novo Iroha esteja em processo de execução quando você iniciar a etapa de implantação. Os exemplos aqui supõem que Torii está acessível no URL configurado em seu perfil `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compilador de contrato Kotodama

O depósito forneceu um contrato mínimo "olá mundo" em `examples/hello/hello.ko`. Compile o bytecode Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Opções:

- `--abi 1` alterou o contrato na versão ABI 1 (somente suportado no momento da redação).
- `--max-cycles 0` exige uma execução sem limite; fixe um nome positivo para suportar o preenchimento dos ciclos para as previsões de conhecimento zero.

## 2. Inspecione o artefato Norito (opcional)

Use `ivm_tool` para verificar a integridade do corpo e das metadonas:

```sh
ivm_tool inspect target/examples/hello.to
```

Você deve ver a versão ABI, os sinalizadores ativos e os pontos de entrada de exportação. Este é um controle rápido antes da implantação.

## 3. Executar o contrato local

Execute o bytecode com `ivm_run` para confirmar o comportamento sem tocar um nud:

```sh
ivm_run target/examples/hello.to --args '{}'
```

O exemplo `hello` registra uma saudação e emite um syscall `SET_ACCOUNT_DETAIL`. O local de execução é útil enquanto você iterez na lógica do contrato antes do editor na rede.

## 4. Implantador via `iroha_cli`

Quando você estiver satisfeito com o contrato, implante-o em um nud por meio da CLI. Forneça uma conta de autoridade, insira uma assinatura e um arquivo `.to` com uma carga útil Base64 :

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

O comando foi um pacote manifestado Norito + bytecode via Torii e exibiu o estado da transação resultante. Uma vez que a transação foi válida, o hash do código exibido na resposta pode servir para recuperar manifestos ou listar instâncias:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Executor via ToriiCom o bytecode registrado, você pode invocar uma instrução que faça referência ao código armazenado (p. ex., via `iroha_cli ledger transaction submit` ou seu aplicativo cliente). Certifique-se de que as permissões de conta autorizem as chamadas de sistema desejadas (`set_account_detail`, `transfer_asset`, etc.).

## Conselhos e Depannage

- Use `make examples-run` para compilar e executar os exemplos em um único comando. Sobrecarregue as variáveis ​​de ambiente `KOTO`/`IVM` se os binários não estiverem em `PATH`.
- Se `koto_compile` recusar a versão ABI, verifique se o compilador e se ele está funcionando bem ABI v1 (execute `koto_compile --abi` sem argumentos para listar o suporte).
- A CLI aceita arquivos de assinatura em hexadecimal ou Base64. Para os testes, você pode usar as emissões do par `iroha_cli tools crypto keypair`.
- Ao depurar cargas úteis Norito, o subcomando `ivm_tool disassemble` ajuda a correlacionar as instruções com a fonte Kotodama.

Este fluxo reflete as etapas utilizadas no CI e nos testes de integração. Para uma análise mais aproximada da gramática Kotodama, dos mapeamentos de syscalls e dos internos Norito, veja:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`