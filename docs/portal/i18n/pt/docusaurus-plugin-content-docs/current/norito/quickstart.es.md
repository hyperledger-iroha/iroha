---
lang: pt
direction: ltr
source: docs/portal/docs/norito/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Início rápido de Norito
description: Crie, valide e despligue um contrato Kotodama com as ferramentas de lançamento e a rede predeterminada de um único par.
slug: /norito/início rápido
---

Este retorno reflete o fluxo que esperamos seguir os desenvolvedores para aprender Norito e Kotodama pela primeira vez: arrancar um determinista vermelho de um par solo, compilar um contrato, fazer um dry-run local e depois enviá-lo por Torii com a CLI de referência.

O contrato de exemplo descreve uma chave/valor na conta do chamador para que você possa verificar o efeito lateral imediato com `iroha_cli`.

## Requisitos anteriores

- [Docker](https://docs.docker.com/engine/install/) com Compose V2 habilitado (é usado para iniciar o par de exibição definido em `defaults/docker-compose.single.yml`).
- Toolchain de Rust (1.76+) para construir os binários auxiliares se não forem baixados publicados.
- Binários `koto_compile`, `ivm_run` e `iroha_cli`. Você pode construí-los a partir do checkout do espaço de trabalho, como mostrado abaixo ou baixar os artefatos da versão correspondente:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Os binários anteriores são seguros de instalar junto com o resto do espaço de trabalho.
> Nunca ligue com `serde`/`serde_json`; Os codecs Norito são aplicados de ponta a ponta.

## 1. Inicia uma rede de desenvolvimento de um par solo

O repositório inclui um pacote de Docker Compose gerado por `kagami swarm` (`defaults/docker-compose.single.yml`). Conecte a gênese por defeito, a configuração do cliente e as sondas de saúde para que Torii seja acessível em `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deixe o conteúdo funcionando (no primeiro plano ou desacoplado). Todas as chamadas posteriores do CLI foram apontadas para este peer por meio de `defaults/client.toml`.

## 2. Redigir o contrato

Crie um diretório de trabalho e guarde o exemplo mínimo de Kotodama:

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

> Prefira manter as fontes de Kotodama no controle de versões. Os exemplos alojados no portal também estão disponíveis na [galeria de exemplos Norito](./examples/) se você quiser um ponto de partida mais completo.

## 3. Compilação e teste de perigo com IVM

Compile o contrato com o bytecode IVM/Norito (`.to`) e execute-o localmente para confirmar se as syscalls do host funcionam antes de tocar a rede:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

O executor imprime o log `info("Hello from Kotodama")` e executa o syscall `SET_ACCOUNT_DETAIL` contra o host simulado. Se o binário opcional `ivm_tool` estiver disponível, `ivm_tool inspect target/quickstart/hello.to` mostra o ABI encabeçado, os bits de recursos e os pontos de entrada exportados.

## 4. Envia o bytecode via Torii

Com o nodo aun corriendo, envie o bytecode compilado para Torii usando a CLI. A identidade de desenvolvimento por defeito é derivada da chave publicada em `defaults/client.toml`, por isso o ID da conta é
```
soraカタカナ...
```

Use o arquivo de configuração para fornecer a URL de Torii, o ID da cadeia e a chave de empresa:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```A CLI codifica a transação com Norito, firma com a chave de desenvolvimento e envia ao peer em execução. Observe os logs de Docker para o syscall `set_account_detail` ou monitore a saída da CLI para o hash da transação comprometida.

## 5. Verifique a mudança de estado

Use o mesmo perfil da CLI para obter os detalhes da conta que estão escritos no contrato:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

Deveria ver o payload JSON respaldado por Norito:

```json
{
  "hello": "world"
}
```

Se faltar o valor, confirme que o serviço de Docker compõe-se em execução e que o hash da transação reportado por `iroha` vai para o estado `Committed`.

## Seguintes passos

- Explore a [galeria de exemplos](./examples/) gerada automaticamente para ver
  como fragmentos Kotodama mas avançados são mapeados para syscalls Norito.
- Leia o [guia de início de Norito](./getting-started) para uma explicação
  mas profundo nas ferramentas do compilador/executor, no desenvolvimento de manifestos e nos metadados de IVM.
- Quando iterar em seus próprios contratos, use `npm run sync-norito-snippets` no
  espaço de trabalho para regenerar snippets descartáveis de modo que os documentos do portal e os artefatos
  será mantido sincronizado com as fontes em `crates/ivm/docs/examples/`.