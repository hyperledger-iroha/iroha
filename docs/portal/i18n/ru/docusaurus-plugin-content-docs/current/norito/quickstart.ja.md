---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/norito/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 516e5f965bba1da289240c6899f9ade348d4de888a024db1a4e25f70c172837a
source_last_modified: "2026-01-22T06:58:49+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ru
direction: ltr
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Быстрый старт Norito
description: Соберите, проверьте и задеплойте контракт Kotodama с релизным инструментарием и дефолтной одноузловой сетью.
slug: /norito/quickstart
---

Этот walkthrough отражает процесс, которому мы ожидаем следовать разработчиков при первом знакомстве с Norito и Kotodama: поднять детерминированную одноузловую сеть, скомпилировать контракт, сделать локальный dry-run, затем отправить его через Torii с эталонным CLI.

Пример контракта записывает пару ключ/значение в аккаунт вызывающего, чтобы вы могли сразу проверить сайд-эффект с помощью `iroha_cli`.

## Требования

- [Docker](https://docs.docker.com/engine/install/) с включенным Compose V2 (используется для старта sample peer, определенного в `defaults/docker-compose.single.yml`).
- Rust toolchain (1.76+) для сборки вспомогательных бинарников, если вы не скачиваете опубликованные.
- Бинарники `koto_compile`, `ivm_run` и `iroha_cli`. Их можно собрать из checkout workspace, как показано ниже, или скачать соответствующие release artifacts:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Эти бинарники безопасно устанавливать рядом с остальными из workspace.
> Они никогда не линкуются с `serde`/`serde_json`; кодеки Norito применяются end-to-end.

## 1. Запустите одноузловую dev сеть

В репозитории есть Docker Compose bundle, сгенерированный `kagami swarm` (`defaults/docker-compose.single.yml`). Он подключает дефолтную genesis, конфигурацию клиента и health probes, чтобы Torii был доступен на `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Оставьте контейнер запущенным (в фоне или в foreground). Все последующие вызовы CLI будут нацелены на этот peer через `defaults/client.toml`.

## 2. Напишите контракт

Создайте рабочую директорию и сохраните минимальный пример Kotodama:

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

> Предпочитайте хранить исходники Kotodama в системе контроля версий. Примеры, размещенные на портале, также доступны в [галерее примеров Norito](./examples/), если нужен более богатый стартовый набор.

## 3. Компиляция и dry-run с IVM

Скомпилируйте контракт в байткод IVM/Norito (`.to`) и запустите его локально, чтобы убедиться, что syscalls хоста проходят до обращения к сети:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Runner выводит лог `info("Hello from Kotodama")` и выполняет syscall `SET_ACCOUNT_DETAIL` против смоделированного хоста. Если доступен опциональный бинарник `ivm_tool`, команда `ivm_tool inspect target/quickstart/hello.to` показывает ABI header, feature bits и экспортированные entrypoints.

## 4. Отправьте байткод через Torii

Пока узел работает, отправьте скомпилированный байткод в Torii через CLI. Дефолтная dev-идентичность выводится из публичного ключа в `defaults/client.toml`, поэтому ID аккаунта:
```
i105...
```

Используйте конфигурационный файл, чтобы задать URL Torii, chain ID и ключ подписи:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI кодирует транзакцию Norito, подписывает ее dev-ключом и отправляет на работающий peer. Следите за логами Docker для syscall `set_account_detail` или мониторьте вывод CLI для хэша committed транзакции.

## 5. Проверьте изменение состояния

Используйте тот же профиль CLI, чтобы получить account detail, который записал контракт:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

Вы должны увидеть JSON payload на базе Norito:

```json
{
  "hello": "world"
}
```

Если значение отсутствует, убедитесь, что сервис Docker compose все еще работает и что хэш транзакции, который вывел `iroha`, достиг состояния `Committed`.

## Следующие шаги

- Изучите авто-сгенерированную [галерею примеров](./examples/), чтобы увидеть,
  как более продвинутые сниппеты Kotodama маппятся на syscalls Norito.
- Прочитайте [Norito getting started guide](./getting-started) для более глубокого
  объяснения инструментов компилятора/раннера, деплоя manifests и метаданных IVM.
- При работе над своими контрактами используйте `npm run sync-norito-snippets` в
  workspace, чтобы регенерировать скачиваемые сниппеты и держать документы портала и артефакты
  синхронизированными с исходниками в `crates/ivm/docs/examples/`.
