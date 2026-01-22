---
lang: ru
direction: ltr
source: docs/portal/docs/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5cae8fa9d9a69d506d0fc49903e801382041d29f2e9a052321224bd3cb7a72d1
source_last_modified: "2025-11-02T04:40:39.595528+00:00"
translation_last_reviewed: 2025-12-30
---

# Начало работы с Norito

Это краткое руководство показывает минимальный рабочий процесс компиляции контракта Kotodama, проверки сгенерированного байткода Norito, локального запуска и деплоя на узел Iroha.

## Требования

1. Установите Rust toolchain (1.76 или новее) и клонируйте этот репозиторий.
2. Соберите или скачайте вспомогательные бинарники:
   - `koto_compile` - компилятор Kotodama, который генерирует байткод IVM/Norito
   - `ivm_run` и `ivm_tool` - утилиты локального запуска и инспекции
   - `iroha_cli` - используется для деплоя контрактов через Torii

   Makefile репозитория ожидает эти бинарники в `PATH`. Вы можете скачать готовые артефакты или собрать их из исходников. Если вы компилируете toolchain локально, укажите Makefile хелперам путь к бинарникам:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Убедитесь, что узел Iroha запущен к моменту шага деплоя. Примеры ниже предполагают, что Torii доступен по URL из профиля `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Скомпилировать контракт Kotodama

В репозитории есть минимальный контракт "hello world" в `examples/hello/hello.ko`. Скомпилируйте его в байткод Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Ключевые флаги:

- `--abi 1` фиксирует контракт на ABI версии 1 (единственная поддерживаемая версия на момент написания).
- `--max-cycles 0` запрашивает неограниченное выполнение; установите положительное число, чтобы ограничить padding циклов для zero-knowledge доказательств.

## 2. Проверить артефакт Norito (опционально)

Используйте `ivm_tool`, чтобы проверить заголовок и встроенные метаданные:

```sh
ivm_tool inspect target/examples/hello.to
```

Вы увидите версию ABI, включенные флаги и экспортированные entrypoints. Это быстрый sanity check перед деплоем.

## 3. Запустить контракт локально

Запустите байткод через `ivm_run`, чтобы подтвердить поведение без обращения к узлу:

```sh
ivm_run target/examples/hello.to --args '{}'
```

Пример `hello` пишет приветствие в лог и вызывает syscall `SET_ACCOUNT_DETAIL`. Локальный запуск полезен при итерации логики контракта до публикации on-chain.

## 4. Деплой через `iroha_cli`

Когда контракт вас устраивает, задеплойте его на узел через CLI. Укажите аккаунт-авторитет, его ключ подписи и либо файл `.to`, либо Base64 payload:

```sh
iroha_cli app contracts deploy \
  --authority alice@wonderland \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

Команда отправляет bundle манифеста Norito + байткода через Torii и печатает статус транзакции. После коммита показанный в ответе хэш кода можно использовать для получения манифестов или списка инстансов:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Запуск через Torii

После регистрации байткода вы можете вызывать его, отправляя инструкцию, которая ссылается на сохраненный код (например, через `iroha_cli ledger transaction submit` или ваш клиент приложения). Убедитесь, что права аккаунта разрешают нужные syscalls (`set_account_detail`, `transfer_asset` и т.д.).

## Советы и устранение проблем

- Используйте `make examples-run`, чтобы собрать и выполнить примеры одним запуском. Переопределите переменные окружения `KOTO`/`IVM`, если бинарники не находятся в `PATH`.
- Если `koto_compile` отклоняет ABI версию, проверьте, что компилятор и узел нацелены на ABI v1 (запустите `koto_compile --abi` без аргументов, чтобы увидеть поддержку).
- CLI принимает ключи подписи в hex или Base64. Для тестов можно использовать ключи, выданные `iroha_cli tools crypto keypair`.
- При отладке Norito payloads полезна команда `ivm_tool disassemble`, которая помогает сопоставлять инструкции с исходниками Kotodama.

Этот поток отражает шаги, используемые в CI и интеграционных тестах. Для более глубокого изучения грамматики Kotodama, маппинга syscalls и внутренностей Norito см.:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
