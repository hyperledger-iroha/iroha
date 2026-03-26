---
lang: pt
direction: ltr
source: docs/portal/docs/norito/getting-started.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Начало работы с Norito

Este é um processo de compilação de contrato mínimo Kotodama, provado Verifique a bateria Norito, feche a tampa local e instale-a no Iroha.

## Treino

1. Configure o conjunto de ferramentas Rust (1.76 ou novo) e clone este repositório.
2. Verifique ou baixe os binários:
   - `koto_compile` - compilador Kotodama, gerador de bateria IVM/Norito
   - `ivm_run` e `ivm_tool` - use verificação e inspeção local
   - `iroha_cli` - usado para a implementação do contrato de trabalho Torii

   O repositório Makefile foi instalado em `PATH`. Você pode encontrar artefatos originais ou desmontá-los. Se você compilar o conjunto de ferramentas localmente, use o Makefile para ajudá-lo com o binário:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Verifique se o Iroha foi usado durante o tempo de execução. Por exemplo, não há necessidade de usar o Torii fornecido no URL do perfil `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Contrato de compilação Kotodama

No repositório há um contrato mínimo "olá mundo" em `examples/hello/hello.ko`. Скомпилируйте его в байткод Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Bandeiras de sucesso:

- `--abi 1` contrato de atualização na versão 1 da ABI (é possível usar a versão no momento da atualização).
- `--max-cycles 0` é uma falha negativa; Use um código de segurança para usar ciclos de preenchimento para documentos de conhecimento zero.

## 2. Prover o artefato Norito (opcional)

Use `ivm_tool`, isso fornecerá a configuração e o metadado padrão:

```sh
ivm_tool inspect target/examples/hello.to
```

Você pode verificar a versão ABI, configurar pontos de entrada e exportar pontos de entrada. Esta é a verificação de sanidade antes da conclusão.

## 3. Fechar contrato local

Para abrir a bateria `ivm_run`, você pode obter o seguinte resultado:

```sh
ivm_run target/examples/hello.to --args '{}'
```

O exemplo `hello` é exibido no log e executa o syscall `SET_ACCOUNT_DETAIL`. A transação local é possível através da iteração do contrato de lógica para publicação na cadeia.

## 4. Деплой через `iroha_cli`

Para que você possa usar o contrato, instale-o na CLI. Use uma conta-autorizada, este é um arquivo de registro e um arquivo `.to`, uma carga útil Base64 gratuita:

```sh
iroha_cli app contracts deploy \
  --authority soraカタカナ... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

O comando отправляет pacote манифеста Norito + байткода через Torii e печатает статус tranзакции. После коммита показанный ответе хэш кода можно использовать для получения манифестов или списка instantes:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Verifique a configuração Torii

Ao registrar o banco de dados, você pode usá-lo também, abrindo as instruções e escolhendo a opção de proteção código (por exemplo, digite `iroha_cli ledger transaction submit` ou seu cliente cliente). Claro, esta conta está gerando novos syscalls (`set_account_detail`, `transfer_asset` e etc.).

## Problema de solução e operação- Use `make examples-run`, isso é feito e iniciado primeiro. Verifique a configuração do `KOTO`/`IVM`, exceto o binário não instalado no `PATH`.
- Если `koto_compile` отклоняет ABI версию, проверьте, что компилятор e узел нацелены на ABI v1 (запустите `koto_compile --abi` Não há argumentos que possam ser encontrados).
- CLI cria chaves em hexadecimal ou Base64. Para o teste, você pode usar um teclado, use `iroha_cli tools crypto keypair`.
- Ao usar cargas úteis Norito, use o comando `ivm_tool disassemble`, que contém instruções e isolações Kotodama.

Isso pode ser feito usando CI e testes integrados. Para melhor usar a gramática Kotodama, mapear syscalls e conectar Norito sim.:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`