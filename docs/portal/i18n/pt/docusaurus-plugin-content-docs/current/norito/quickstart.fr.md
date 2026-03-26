---
lang: pt
direction: ltr
source: docs/portal/docs/norito/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Demarrage Norito
description: Construa, valide e implemente um contrato Kotodama com a liberação da liberação e o saldo monopar por padrão.
slug: /norito/início rápido
---

Este passo a passo representa o fluxo de trabalho que atendemos aos desenvolvedores quando descobrimos Norito e Kotodama para a estreia: demarrer um recurso determinado mono-par, compilar um contrato, executar a localização e enviá-lo via Torii com a CLI de referência.

O contrato de exemplo ecrit une paire cle/valeur na conta do recorrente para que você possa verificar o efeito de borda imediatamente com `iroha_cli`.

## Pré-requisito

- [Docker](https://docs.docker.com/engine/install/) com Compose V2 ativo (utilize para demarcar o par de exemplo definido em `defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) para construir binários auxiliares se você não precisar descarregá-los publicamente.
- Binários `koto_compile`, `ivm_run` e `iroha_cli`. Você pode construir a partir do checkout do espaço de trabalho como ci-dessous ou baixar os artefatos de lançamento correspondentes:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Os binários ci-dessus são sem risco de instalação com o resto do espaço de trabalho.
> Ils ne lient jamais `serde`/`serde_json` ; Os codecs Norito são aplicados em uma partida.

## 1. Demarrer un reseau dev mono-pair

O depósito inclui um pacote Docker Compose gerado por `kagami swarm` (`defaults/docker-compose.single.yml`). Ele conectou a gênese por padrão, a configuração do cliente e as sondas de integridade até Torii, portanto, podem ser conectadas a `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deixe o conteneur tourner (no primeiro plano ou em separado). Todos os comandos CLI seguintes fornecem este par via `defaults/client.toml`.

## 2. Escrever o contrato

Crie um repertório de trabalho e registre o exemplo Kotodama mínimo:

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

> Prefira conservar as fontes Kotodama no controle de versão. Os exemplos listados no portal também estão disponíveis na [galerie de exemplos Norito](./examples/) se você quiser um ponto de partida mais rico.

## 3. Compilador e simulação com IVM

Compile o contrato no bytecode IVM/Norito (`.to`) e execute o local para confirmar que as syscalls do host foram reativadas antes de tocar na rede:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

O executor imprimiu o log `info("Hello from Kotodama")` e efetuou o syscall `SET_ACCOUNT_DETAIL` contra a simulação do host. Se a opção binária `ivm_tool` estiver disponível, `ivm_tool inspect target/quickstart/hello.to` exibirá o ABI completo, os bits de recursos e os pontos de entrada exportados.

## 4. Insira o bytecode via Torii

O novo durante todo o processo de execução, envia o bytecode compilado para Torii com a CLI. A identidade de desenvolvimento por padrão é derivada do código público em `defaults/client.toml`, já que a ID da conta é
```
soraカタカナ...
```

Use o arquivo de configuração para fornecer o URL Torii, o ID da cadeia e a chave de assinatura:```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

A CLI codifica a transação com Norito, a assinatura com a chave de desenvolvimento e o envio au pair durante a execução. Monitore os logs Docker para o syscall `set_account_detail` ou execute a saída da CLI para o hash da transação confirmada.

## 5. Verificador da mudança de estado

Use a CLI do perfil do meme para recuperar os detalhes da conta que você contratou por escrito:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

Você deve ver o payload JSON adicionado a Norito :

```json
{
  "hello": "world"
}
```

Se o valor estiver ausente, verifique se o serviço Docker compõe sempre e que o hash de transação sinalizado por `iroha` e atinja o estado `Committed`.

## Etapas seguintes

- Explore a [galerie d'exemples](./examples/) gerada automaticamente para ver
  comente os snippets Kotodama e avance para o syscalls Norito.
- Leia o [guia Norito primeiros passos](./getting-started) para uma explicação
  além de aprovar ferramentas de compilação/execução, implantação de manifestos e metadonnees IVM.
- Ao navegar pelos seus próprios contratos, use `npm run sync-norito-snippets` no
  espaço de trabalho para regenerar fragmentos telecarregáveis para que os documentos do portal e os artefatos permaneçam
  sincroniza com fontes sob `crates/ivm/docs/examples/`.