---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56c8920a7b09e9536b6777807520958c064ad6bb2890d40af964719565ebc2ed
source_last_modified: "2026-01-22T15:55:01+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: pt
direction: ltr
source: docs/portal/docs/norito/getting-started.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Primeiros passos com Norito

Este guia rapido mostra o fluxo minimo para compilar um contrato Kotodama, inspecionar o bytecode Norito gerado, executa-lo localmente e faze-lo deploy em um nodo Iroha.

## Pre-requisitos

1. Instale a toolchain Rust (1.76 ou mais recente) e faca checkout deste repositorio.
2. Compile ou baixe os binarios de suporte:
   - `koto_compile` - compilador Kotodama que emite bytecode IVM/Norito
   - `ivm_run` e `ivm_tool` - utilitarios de execucao local e inspecao
   - `iroha_cli` - usado para deploy de contratos via Torii

   O Makefile do repositorio espera esses binarios no `PATH`. Voce pode baixar artefatos precompilados ou compilar a partir do codigo fonte. Se compilar a toolchain localmente, aponte os helpers do Makefile para os binarios:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Garanta que um nodo Iroha esteja rodando quando chegar na etapa de deploy. Os exemplos abaixo assumem que Torii esta acessivel na URL configurada no perfil `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compile um contrato Kotodama

O repositorio inclui um contrato minimo "hello world" em `examples/hello/hello.ko`. Compile-o para bytecode Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Flags chave:

- `--abi 1` fixa o contrato na versao ABI 1 (a unica suportada no momento).
- `--max-cycles 0` solicita execucao sem limite; defina um numero positivo para limitar o padding de ciclos para provas de conhecimento zero.

## 2. Inspecione o artefato Norito (opcional)

Use `ivm_tool` para verificar o cabecalho e os metadados embutidos:

```sh
ivm_tool inspect target/examples/hello.to
```

Voce deve ver a versao ABI, os flags habilitados e os entry points exportados. E uma checagem rapida antes do deploy.

## 3. Execute o contrato localmente

Execute o bytecode com `ivm_run` para confirmar o comportamento sem tocar um nodo:

```sh
ivm_run target/examples/hello.to --args '{}'
```

O exemplo `hello` registra uma saudacao e emite um syscall `SET_ACCOUNT_DETAIL`. Executar localmente e util enquanto voce itera na logica do contrato antes de publica-lo on-chain.

## 4. Faca deploy via `iroha_cli`

Quando estiver satisfeito com o contrato, faca o deploy em um nodo usando o CLI. Forneca uma conta de autoridade, sua chave de assinatura e um arquivo `.to` ou payload Base64:

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

O comando envia um bundle de manifest Norito + bytecode via Torii e imprime o status da transacao resultante. Depois da transacao ser confirmada, o hash do codigo mostrado na resposta pode ser usado para recuperar manifests ou listar instances:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Execute contra Torii

Com o bytecode registrado, voce pode invoca-lo submetendo uma instrucao que referencia o codigo armazenado (por exemplo, via `iroha_cli ledger transaction submit` ou pelo cliente da sua aplicacao). Garanta que as permissoes da conta permitam os syscalls desejados (`set_account_detail`, `transfer_asset`, etc.).

## Dicas e solucao de problemas

- Use `make examples-run` para compilar e executar os exemplos de uma vez. Substitua as variaveis de ambiente `KOTO`/`IVM` se os binarios nao estiverem no `PATH`.
- Se `koto_compile` rejeitar a versao ABI, verifique se o compilador e o nodo miram ABI v1 (rode `koto_compile --abi` sem argumentos para listar o suporte).
- O CLI aceita chaves de assinatura em hex ou Base64. Para testes, voce pode usar chaves emitidas por `iroha_cli tools crypto keypair`.
- Ao depurar payloads Norito, o subcomando `ivm_tool disassemble` ajuda a correlacionar instrucoes com o codigo fonte Kotodama.

Este fluxo espelha os passos usados em CI e testes de integracao. Para um mergulho mais profundo na gramatica Kotodama, nos mapeamentos de syscalls e nos internals de Norito, veja:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
