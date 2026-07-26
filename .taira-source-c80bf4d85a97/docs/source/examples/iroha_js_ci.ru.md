---
lang: ru
direction: ltr
source: docs/source/examples/iroha_js_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d21afd33da5ee459b1f6ffb6ac7c42adc0852ed7929e69993f81914637b5e6b5
source_last_modified: "2026-01-03T18:08:00.440949+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
  This document provides guidance for running @iroha/iroha-js in CI systems.
-->

# Iroha Справочник JS CI

Пакет `@iroha/iroha-js` объединяет собственные привязки через `iroha_js_host`. Любой
Конвейер CI, выполняющий тесты или сборки, должен предоставлять как среду выполнения Node.js, так и среду выполнения Node.js.
и набор инструментов Rust, чтобы родной пакет можно было скомпилировать перед тестами.
беги.

## Рекомендуемые шаги

1. Используйте версию Node LTS (18 или 20) через `actions/setup-node` или ваш CI.
   эквивалент.
2. Установите набор инструментов Rust, указанный в `rust-toolchain.toml`. Мы рекомендуем
   `dtolnay/rust-toolchain@v1` в действиях GitHub.
3. Кэшируйте индексы реестра груза/git и каталог `target/`, чтобы избежать
   восстановление родного аддона в каждой работе.
4. Запустите `npm install`, затем `npm run lint:test`. Комбинированный сценарий обеспечивает
   ESLint без предупреждений, создает собственное дополнение и запускает тест Node.
   комплект, чтобы CI соответствовал рабочему процессу выпуска.
5. При необходимости запустите `node --test` в качестве быстрого шага дыма один раз `npm run build:native`.
   создал дополнение (например, предварительно отправьте полосы быстрой проверки, которые повторно используют
   кэшированные артефакты).
6. Добавьте любые дополнительные проверки или проверки форматирования от вашего потребителя.
   проект поверх `npm run lint:test`, когда требуются более строгие политики.
7. При совместном использовании конфигурации между службами загрузите `iroha_config` и передайте
   проанализированный документ до `resolveToriiClientConfig({ config })`, поэтому клиенты Node
   повторно используйте ту же политику таймаута/повторной попытки/токена, что и в остальной части развертывания (см.
   `docs/source/sdk/js/quickstart.md` для полного примера).

## Шаблон действий GitHub

```yaml
name: iroha-js-ci

on:
  push:
    branches: [ main ]
  pull_request:

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        node-version: [18, 20]
    steps:
      - uses: actions/checkout@v4

      - name: Set up Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: npm

      - name: Set up Rust toolchain
        uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable

      - name: Cache cargo artifacts
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - run: npm install
      - run: npm run lint:test
```

## Быстрая работа по дыму (необязательно)

Для запросов на включение, которые касаются только документации или определений TypeScript,
минимальное задание может повторно использовать кэшированные артефакты, пересобрать собственный модуль и запустить
Непосредственный запуск тестов Node:

```yaml
jobs:
  smoke:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable
      - run: npm ci
      - run: npm run build:native
      - run: node --test
```

Это задание выполняется быстро, при этом проверяется, что собственное дополнение компилируется.
и что набор тестов Node пройден.

> **Эталонная реализация:** репозиторий включает
> `.github/workflows/javascript-sdk.yml`, который объединяет описанные выше шаги в
> Матрица узлов 18/20 с кэшированием груза.