---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/norito/ledger-walkthrough.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8cdfd010c75d54cf80ef9ed67bae6565fdbcd24c8b7d15ba361762f92a58c29b
source_last_modified: "2026-01-22T06:58:49+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ru
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Пошаговый разбор реестра
description: Воспроизведите детерминированный поток register -> mint -> transfer с CLI `iroha` и проверьте итоговое состояние реестра.
slug: /norito/ledger-walkthrough
---

Этот walkthrough дополняет [Norito quickstart](./quickstart.md), показывая, как менять и проверять состояние реестра с помощью CLI `iroha`. Вы зарегистрируете новую дефиницию актива, заминтите единицы на дефолтный операторский аккаунт, переведете часть баланса на другой аккаунт и проверите итоговые транзакции и владения. Каждый шаг отражает потоки, покрытые в quickstart SDK Rust/Python/JavaScript, чтобы вы могли подтвердить паритет между CLI и поведением SDK.

## Требования

- Следуйте [quickstart](./quickstart.md), чтобы запустить одноузловую сеть через
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Убедитесь, что `iroha` (CLI) собран или скачан, и что вы можете достучаться до
  peer через `defaults/client.toml`.
- Опциональные помощники: `jq` (форматирование JSON ответов) и POSIX shell для
  сниппетов с переменными окружения ниже.

По всей инструкции заменяйте `$ADMIN_ACCOUNT` и `$RECEIVER_ACCOUNT` на нужные вам
ID аккаунтов. В дефолтном bundle уже есть два аккаунта, полученных из demo-ключей:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

Подтвердите значения, выведя первые аккаунты:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Осмотрите состояние genesis

Начните с изучения реестра, на который нацелен CLI:

```sh
# Domains, зарегистрированные в genesis
iroha --config defaults/client.toml domain list all --table

# Accounts внутри wonderland (увеличьте --limit при необходимости)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland.universal"}' \
  --limit 10 --table

# Asset definitions, которые уже существуют
iroha --config defaults/client.toml asset definition list all --table
```

Эти команды опираются на Norito-ответы, поэтому фильтрация и пагинация
детерминированы и совпадают с тем, что получают SDK.

## 2. Зарегистрируйте дефиницию актива

Создайте новый бесконечно mintable актив `coffee` в домене `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI выведет хэш отправленной транзакции (например, `0x5f…`). Сохраните его, чтобы
позже проверить статус.

## 3. Замитьте единицы на операторский аккаунт

Количество актива живет под парой `(asset definition, account)`. Замитьте 250
единиц `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` на `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Снова сохраните хэш транзакции (`$MINT_HASH`) из вывода CLI. Чтобы проверить баланс,
выполните:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

или, чтобы получить только новый актив:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Переведите часть баланса на другой аккаунт

Переведите 50 единиц с операторского аккаунта на `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Сохраните хэш транзакции как `$TRANSFER_HASH`. Запросите holdings на обоих аккаунтах,
чтобы проверить новые балансы:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Проверьте доказательства реестра

Используйте сохраненные хэши, чтобы подтвердить коммит обеих транзакций:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Вы также можете стримить последние блоки, чтобы увидеть, какой блок включил перевод:

```sh
# Стрим от последнего блока и остановка через ~5 секунд
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Все команды выше используют те же Norito payloads, что и SDK. Если вы повторите
этот поток в коде (см. quickstarts SDK ниже), хэши и балансы совпадут при условии,
что вы нацелены на ту же сеть и те же defaults.

## Ссылки на паритет SDK

- [Rust SDK quickstart](../sdks/rust) — демонстрирует регистрацию инструкций,
  отправку транзакций и polling статуса из Rust.
- [Python SDK quickstart](../sdks/python) — показывает те же операции register/mint
  с Norito-backed JSON helpers.
- [JavaScript SDK quickstart](../sdks/javascript) — покрывает Torii запросы,
  governance helpers и typed query wrappers.

Сначала выполните walkthrough в CLI, затем повторите сценарий с предпочитаемым SDK,
чтобы убедиться, что обе поверхности согласуются по хэшам транзакций, балансам и
результатам запросов.
