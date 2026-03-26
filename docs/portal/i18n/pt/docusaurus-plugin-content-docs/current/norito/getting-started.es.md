---
lang: pt
direction: ltr
source: docs/portal/docs/norito/getting-started.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Primeiros passos com Norito

Este guia rápido mostra o fluxo mínimo para compilar um contrato Kotodama, inspecionar o bytecode Norito gerado, executá-lo localmente e desinstalá-lo em um nó de Iroha.

## Requisitos anteriores

1. Instale o conjunto de ferramentas Rust (1.76 ou mais recente) e clone este repositório.
2. Construa ou baixe os binários de suporte:
   - `koto_compile` - compilador Kotodama que emite bytecode IVM/Norito
   - `ivm_run` e `ivm_tool` - russálias de execução local e inspeção
   - `iroha_cli` - é usado para a solução de contratos via Torii

   O Makefile do repositório espera esses binários em `PATH`. Você pode baixar artefatos pré-compilados ou compilados a partir do código fonte. Se você compilar o conjunto de ferramentas localmente, use os auxiliares do Makefile para os binários:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Certifique-se de que um nó de Iroha esteja em execução quando você sair do passo de despliegue. Os exemplos abaixo presumem que Torii está acessível no URL definido em seu perfil de `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compilar um contrato Kotodama

O repositório inclui um contrato mínimo "hello world" em `examples/hello/hello.ko`. Compile o bytecode Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Opções chave:

- `--abi 1` fixa o contrato na versão ABI 1 (a única suportada no momento da escrita).
- `--max-cycles 0` solicitação de ejecução sem limites; estabeleça um número positivo para acotar o preenchimento de ciclos para testes de conhecimento zero.

## 2. Inspecione o artefato Norito (opcional)

Use `ivm_tool` para verificar a cabeça e os metadados incrustados:

```sh
ivm_tool inspect target/examples/hello.to
```

Deveria ver a versão ABI, os sinalizadores habilitados e os pontos de entrada exportados. É uma verificação rápida antes do despliegue.

## 3. Execute o contrato localmente

Execute o bytecode com `ivm_run` para confirmar o comportamento sem tocar um nó:

```sh
ivm_run target/examples/hello.to --args '{}'
```

O exemplo `hello` registra uma saudação e emite um syscall `SET_ACCOUNT_DETAIL`. Executá-lo localmente é útil durante iteras sobre a lógica do contrato antes de publicá-lo na rede.

## 4. Despliega via `iroha_cli`

Quando você estiver satisfeito com o contrato, desdobre um nó usando a CLI. Proporciona uma conta de autoridade, sua chave de firma e um arquivo `.to` ou payload Base64:

```sh
iroha_cli app contracts deploy \
  --authority soraカタカナ... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

O comando envia um pacote de manifesto Norito + bytecode por Torii e mostra o estado da transação resultante. Uma vez confirmado, o hash do código exibido na resposta pode ser usado para recuperar manifestos ou listar instâncias:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Execução contra ToriiCom o bytecode registrado, você pode invocar o envio de uma instrução que deve ser referenciada ao código armazenado (p. ej., por meio de `iroha_cli ledger transaction submit` ou seu cliente de aplicação). Certifique-se de que as permissões da conta permitem os syscalls desejados (`set_account_detail`, `transfer_asset`, etc.).

## Conselhos e solução de problemas

- Use `make examples-run` para compilar e executar os exemplos em uma única etapa. Descreva as variáveis ​​de ambiente `KOTO`/`IVM` se os binários não estiverem em `PATH`.
- Se `koto_compile` rechaza a versão ABI, verifique se o compilador e o nodo apontam para ABI v1 (executa `koto_compile --abi` sem argumentos para listar suporte).
- O CLI aceita chaves de firma em hexadecimal ou Base64. Para fins de verificação, você pode usar as chaves emitidas por `iroha_cli tools crypto keypair`.
- Ao limpar cargas úteis Norito, o subcomando `ivm_tool disassemble` ajuda a instruções correlacionadas com o código fonte Kotodama.

Este fluxo reflete as etapas usadas na CI e nas tentativas de integração. Para uma análise mais profunda da gramatica de Kotodama, os mapas de syscalls e os internos de Norito, consulte:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`