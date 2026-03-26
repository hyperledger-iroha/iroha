---
lang: pt
direction: ltr
source: docs/portal/docs/norito/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Быстрый старт Norito
description: Соберите, проверьте и задеплойте контракт Kotodama com релизным инструментарием и дефолтной одноузловой сетью.
slug: /norito/início rápido
---

Este passo a passo é um processo que nos ajuda a organizar o processo de instalação com Norito e Kotodama: é possível determinar o conjunto de configurações, configurar o contrato, executar o teste local, fechar Abra o Torii com CLI padrão.

Ao iniciar um contrato de compra, você pode obter uma conta/clique para obter mais informações сайд-эффект с помощью `iroha_cli`.

## Treino

- [Docker](https://docs.docker.com/engine/install/) no Compose V2 (explicado para o par de amostra inicial, instalado em `defaults/docker-compose.single.yml`).
- Rust toolchain (1.76+) para сборки вспомогательных бинарников, mas você não pode usá-lo.
- Binários `koto_compile`, `ivm_run` e `iroha_cli`. É possível trabalhar na área de trabalho de checkout, como é possível, ou criar artefatos de lançamento:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Este arquivo binário não pode ser usado para configurar o espaço de trabalho.
> Este único não está vinculado ao `serde`/`serde_json`; O código Norito é desenvolvido de ponta a ponta.

## 1. Запустите одноузловую dev сеть

No repositório está Docker Compose bundle, gerador `kagami swarm` (`defaults/docker-compose.single.yml`). Em uma experiência de genesis, configuração de cliente e sondas de saúde, o Torii foi fornecido para `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Coloque o conteúdo em primeiro plano (no telefone ou em primeiro plano). Você pode usar CLI para conectar-se a este peer no `defaults/client.toml`.

## 2. Fechar contrato

Selecione o diretor de trabalho e forneça o exemplo mínimo Kotodama:

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

> Verifique a configuração do Kotodama na versão de controle do sistema. Exemplos, размещенные на портале, также доступны в [галерее примеров Norito](./examples/), если нужен более богатый стартовый набор.

## 3. Compilação e simulação com IVM

Скомпилируйте контракт в айткод IVM/Norito (`.to`) и запустите его локально, чтобы убедиться, o que os syscalls fazem é obter a seguinte configuração:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

O Runner obteve o log `info("Hello from Kotodama")` e o syscall `SET_ACCOUNT_DETAIL` foi instalado. O pacote opcional `ivm_tool`, comando `ivm_tool inspect target/quickstart/hello.to` contém cabeçalho ABI, bits de recurso e pontos de entrada de exportação.

## 4. Abra a bateria do Torii

Ao usar o trabalho, abra o arquivo compactado em Torii pela CLI. Дефолтная dev-идентичность выводится из публичного ключа em `defaults/client.toml`, поэтому ID аккаунта:
```
soraカタカナ...
```

Use o arquivo de configuração, insira o URL Torii, ID da cadeia e código de identificação:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```CLI codifica a transação Norito, подписывает ее dev-ключом e отправляет на работающий peer. Confie no log Docker para o syscall `set_account_detail` ou monitore sua CLI para transações confirmadas.

## 5. Prover uma configuração de configuração

Use este perfil CLI, veja mais detalhes da conta e contrate o contrato:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

Você pode usar a carga útil JSON no Norito:

```json
{
  "hello": "world"
}
```

Se você não abrir, убедитесь, что сервис Docker compor все еще работает и что хэш транзакции, Eu tenho `iroha`, envio de `Committed`.

## Следующие шаги

- Изучите авто-сгенерированную [галерею примеров](./examples/), чтобы увидеть,
  Como é melhor produzir trechos Kotodama, mapeamos syscalls Norito.
- Прочитайте [Norito guia de primeiros passos](./getting-started) para более глубокого
  объяснения инструментов компилятора/раннера, деплоя manifestos e метаданных IVM.
- Para trabalhar no seu contrato usando `npm run sync-norito-snippets` em
  espaço de trabalho, чтобы регенерировать скачиваемые сниппеты e держать документы портала и артефакты
  sincronização com a detecção em `crates/ivm/docs/examples/`.