---
lang: pt
direction: ltr
source: docs/portal/docs/norito/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Início rápido do Norito
description: Construa, valide e faça deploy de um contrato Kotodama com o tooling de release e a rede padrão de um único peer.
slug: /norito/início rápido
---

Este passo a passo reflete o fluxo que esperamos que os desenvolvedores sigam ao aprender Norito e Kotodama pela primeira vez: iniciar uma rede determinística de um único peer, compilar um contrato, fazer dry-run localmente e depois enviar via Torii com o CLI de referência.

O contrato de exemplo grava um par chave/valor na conta do chamador para que você possa verificar o efeito colateral imediatamente com `iroha_cli`.

## Pré-requisitos

- [Docker](https://docs.docker.com/engine/install/) com Compose V2 habilitado (usado para iniciar o par de exemplo definido em `defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) para compilar os binários auxiliares se você não baixar os publicados.
- Binários `koto_compile`, `ivm_run` e `iroha_cli`. Você pode compilar os itens a partir do checkout do workspace como mostrado abaixo ou baixar os artefatos de lançamento correspondentes:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Os binários acima são seguros para instalação junto com o resto do espaço de trabalho.
> Eles nunca fazem link com `serde`/`serde_json`; os codecs Norito são aplicados de ponta a ponta.

## 1. Inicie uma rede de desenvolvimento de um único par

O repositório inclui um pacote Docker Compose gerado por `kagami swarm` (`defaults/docker-compose.single.yml`). Ele conecta o padrão genesis, a configuração do cliente e as sondas de saúde para que Torii fique acessível em `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deixe o container rodando (em primeiro plano ou desanexado). Todas as chamadas de CLI posteriores apontam para esse peer via `defaults/client.toml`.

## 2. Escreva o contrato

Crie um diretório de trabalho e salve o exemplo mínimo de Kotodama:

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

> Prefira manter as fontes Kotodama em controle de versão. Exemplos hospedados no portal também estão disponíveis na [galeria de exemplos Norito](./examples/) se você quiser um ponto de partida mais rico.

## 3. Compile e faca dry-run com IVM

Compile o contrato para bytecode IVM/Norito (`.to`) e execute-o localmente para confirmar se os syscalls do host funcionam antes de tocar a rede:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

O runner imprime o log `info("Hello from Kotodama")` e executa o syscall `SET_ACCOUNT_DETAIL` contra o host mockado. Se o binário opcional `ivm_tool` estiver disponível, `ivm_tool inspect target/quickstart/hello.to` mostra o cabeçalho ABI, os feature bits e os pontos de entrada exportados.

## 4. Envie o bytecode via Torii

Com o nodo ainda rodando, envie o bytecode compilado para Torii usando o CLI. A identidade de desenvolvimento padrão e derivada da chave pública em `defaults/client.toml`, portanto o ID de conta e
```
ih58...
```

Use o arquivo de configuração para fornecer a URL de Torii, o ID da cadeia e a chave de assinatura:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```O CLI codifica a transação com Norito, assina com a chave de dev e envia ao peer em execução. Observe os logs do Docker para o syscall `set_account_detail` ou monitore a saida do CLI para o hash da transação confirmada.

## 5. Verifique a mudança de estado

Use o mesmo perfil da CLI para buscar os detalhes da conta que o contrato gravou:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

Você deve ver o payload JSON suportado por Norito:

```json
{
  "hello": "world"
}
```

Se o valor estiver ausente, confirme que o serviço Docker compõe ainda esta rodando e que o hash da transação reportado por `iroha` chegou ao estado `Committed`.

## Próximos passos

- Explore a [galeria de exemplos](./examples/) gerada automaticamente para ver
  como snippets Kotodama mais avançados se mapeiam para syscalls Norito.
- Leia o [guia de início do Norito](./getting-started) para uma explicação
  mais profundo do tooling de compilador/runner, do deploy de manifests e dos metadados IVM.
- Ao iterar nos seus contratos, use `npm run sync-norito-snippets` no workspace para
  regenerar trechos baixos e manter os documentos do portal e artefatos sincronizados com as fontes em `crates/ivm/docs/examples/`.