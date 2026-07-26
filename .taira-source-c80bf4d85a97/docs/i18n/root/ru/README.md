---
lang: ru
direction: ltr
source: README.md
status: complete
translator: manual
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:00:00+00:00"
translation_last_reviewed: 2026-02-07
---

# Hyperledger Iroha

[![Лицензия](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha — детерминированная блокчейн-платформа для permissioned- и консорциумных развертываний. Она предоставляет управление аккаунтами и активами, on-chain-права доступа и смарт-контракты через Iroha Virtual Machine (IVM).

> Состояние workspace и недавние изменения отслеживаются в [`status.md`](./status.md).

## Линии релиза

Этот репозиторий публикует две линии развертывания из одной кодовой базы:

- **Iroha 2**: self-hosted permissioned/consortium сети.
- **Iroha 3 (SORA Nexus)**: линия, ориентированная на Nexus и использующая те же базовые crates.

Обе линии используют одинаковые ключевые компоненты, включая сериализацию Norito, консенсус Sumeragi и toolchain Kotodama -> IVM.

## Структура репозитория

- [`crates/`](./crates): основные Rust crates (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `ivm`, `norito` и т.д.).
- [`integration_tests/`](./integration_tests): межкомпонентные интеграционные и сетевые тесты.
- [`IrohaSwift/`](./IrohaSwift): пакет Swift SDK.
- [`java/iroha_android/`](./java/iroha_android): пакет Android SDK.
- [`docs/`](./docs): документация для пользователей, операторов и разработчиков.

## Быстрый старт

### Предварительные требования

- [Rust stable](https://www.rust-lang.org/tools/install)
- Опционально: Docker + Docker Compose для локальных multi-peer запусков

### Сборка и тесты (workspace)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Примечания:

- Полная сборка workspace может занимать около 20 минут.
- Полные тесты workspace могут занимать несколько часов.
- Workspace нацелен на `std` (WASM/no-std сборки не поддерживаются).

### Точечные команды тестов

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### Команды тестов SDK

```bash
cd IrohaSwift
swift test
```

```bash
cd java/iroha_android
JAVA_HOME=$(/usr/libexec/java_home -v 21) \
ANDROID_HOME=~/Library/Android/sdk \
ANDROID_SDK_ROOT=~/Library/Android/sdk \
./gradlew test
```

## Запуск локальной сети

Запустите предоставленную Docker Compose сеть:

```bash
docker compose -f defaults/docker-compose.yml up
```

Используйте CLI с конфигурацией клиента по умолчанию:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

Для шагов native-развертывания демона см. [`crates/irohad/README.md`](./crates/irohad/README.md).

## API и наблюдаемость

Torii предоставляет Norito- и JSON-API. Часто используемые операторские endpoint’ы:

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

Полная справка по endpoint’ам:

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## Ключевые crates

- [`crates/iroha`](./crates/iroha): клиентская библиотека.
- [`crates/irohad`](./crates/irohad): бинарники peer-демона.
- [`crates/iroha_cli`](./crates/iroha_cli): эталонный CLI.
- [`crates/iroha_core`](./crates/iroha_core): движок исполнения и core ledger.
- [`crates/iroha_config`](./crates/iroha_config): типизированная модель конфигурации.
- [`crates/iroha_data_model`](./crates/iroha_data_model): каноническая модель данных.
- [`crates/iroha_crypto`](./crates/iroha_crypto): криптографические примитивы.
- [`crates/norito`](./crates/norito): детерминированный кодек сериализации.
- [`crates/ivm`](./crates/ivm): Iroha Virtual Machine.
- [`crates/iroha_kagami`](./crates/iroha_kagami): инструменты для ключей/genesis/config.

## Карта документации

- Главный индекс: [`docs/README.md`](./docs/README.md)
- Genesis: [`docs/genesis.md`](./docs/genesis.md)
- Консенсус (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- Транзакционный pipeline: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- Внутренности P2P: [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM syscalls: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Грамматика Kotodama: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Wire-формат Norito: [`norito.md`](./norito.md)
- Текущее отслеживание работ: [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## Переводы

Обзор на японском: [`README.ja.md`](./README.ja.md)

Другие обзоры:
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

Процесс перевода: [`docs/i18n/README.md`](./docs/i18n/README.md)

## Вклад и помощь

- Руководство по вкладу: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Каналы сообщества/поддержки: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## Лицензия

Iroha распространяется по лицензии Apache-2.0. См. [`LICENSE`](./LICENSE).

Документация распространяется по лицензии CC-BY-4.0: http://creativecommons.org/licenses/by/4.0/
