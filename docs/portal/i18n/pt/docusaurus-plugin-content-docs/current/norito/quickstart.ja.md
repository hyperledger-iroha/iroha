---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/norito/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c065b3bb375c51ac5953f47e817fc6dfa93a6c4803e749bc06c4ea44abce0193
source_last_modified: "2025-11-14T04:43:20.901903+00:00"
translation_last_reviewed: 2026-01-30
---

Este passo a passo espelha o fluxo que esperamos que desenvolvedores sigam ao aprender Norito e Kotodama pela primeira vez: iniciar uma rede deterministica de um unico peer, compilar um contrato, fazer dry-run localmente e depois enviar via Torii com o CLI de referencia.

O contrato de exemplo grava um par chave/valor na conta do chamador para que voce possa verificar o efeito colateral imediatamente com `iroha_cli`.

## Pre-requisitos

- [Docker](https://docs.docker.com/engine/install/) com Compose V2 habilitado (usado para iniciar o peer de exemplo definido em `defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) para compilar os binarios auxiliares se voce nao baixar os publicados.
- Binarios `koto_compile`, `ivm_run` e `iroha_cli`. Voce pode compila-los a partir do checkout do workspace como mostrado abaixo ou baixar os artifacts de release correspondentes:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Os binarios acima sao seguros para instalar junto com o resto do workspace.
> Eles nunca fazem link com `serde`/`serde_json`; os codecs Norito sao aplicados de ponta a ponta.

## 1. Inicie uma rede dev de um unico peer

O repositorio inclui um bundle Docker Compose gerado por `kagami swarm` (`defaults/docker-compose.single.yml`). Ele conecta a genesis padrao, a configuracao do cliente e os health probes para que Torii fique acessivel em `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deixe o container rodando (em primeiro plano ou detached). Todas as chamadas de CLI posteriores apontam para esse peer via `defaults/client.toml`.

## 2. Escreva o contrato

Crie um diretorio de trabalho e salve o exemplo minimo de Kotodama:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> Prefira manter os fontes Kotodama em controle de versao. Exemplos hospedados no portal tambem estao disponiveis na [galeria de exemplos Norito](./examples/) se voce quiser um ponto de partida mais rico.

## 3. Compile e faca dry-run com IVM

Compile o contrato para bytecode IVM/Norito (`.to`) e execute-o localmente para confirmar que os syscalls do host funcionam antes de tocar a rede:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

O runner imprime o log `info("Hello from Kotodama")` e executa o syscall `SET_ACCOUNT_DETAIL` contra o host mockado. Se o binario opcional `ivm_tool` estiver disponivel, `ivm_tool inspect target/quickstart/hello.to` mostra o ABI header, os feature bits e os entrypoints exportados.

## 4. Envie o bytecode via Torii

Com o nodo ainda rodando, envie o bytecode compilado para Torii usando o CLI. A identidade de desenvolvimento padrao e derivada da chave publica em `defaults/client.toml`, portanto o ID de conta e
```
ih58...
```

Use o arquivo de configuracao para fornecer a URL de Torii, o chain ID e a chave de assinatura:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

O CLI codifica a transacao com Norito, assina com a chave de dev e envia ao peer em execucao. Observe os logs do Docker para o syscall `set_account_detail` ou monitore a saida do CLI para o hash da transacao committed.

## 5. Verifique a mudanca de estado

Use o mesmo perfil do CLI para buscar o account detail que o contrato gravou:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

Voce deve ver o payload JSON suportado por Norito:

```json
{
  "hello": "world"
}
```

Se o valor estiver ausente, confirme que o servico Docker compose ainda esta rodando e que o hash da transacao reportado por `iroha` chegou ao estado `Committed`.

## Proximos passos

- Explore a [galeria de exemplos](./examples/) gerada automaticamente para ver
  como snippets Kotodama mais avancados se mapeiam para syscalls Norito.
- Leia o [guia de inicio do Norito](./getting-started) para uma explicacao
  mais profunda do tooling de compilador/runner, do deploy de manifests e dos metadados IVM.
- Ao iterar nos seus contratos, use `npm run sync-norito-snippets` no workspace para
  regenerar snippets baixaveis e manter os docs do portal e artifacts sincronizados com as fontes em `crates/ivm/docs/examples/`.
