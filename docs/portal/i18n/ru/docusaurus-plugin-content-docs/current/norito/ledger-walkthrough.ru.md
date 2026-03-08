---
lang: ru
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: Пошаговый реестр разбора
описание: Воспроизвести Регистр детерминированного потока -> mint -> передать с CLI `iroha` и проверить итоговое состояние реестра.
слизень: /norito/ledger-walkthrough
---

Это пошаговое руководство выполняет [Norito faststart](./quickstart.md), период, как менять и проверять состояние реестра с помощью CLI `iroha`. Вы регистрируете новое определение актива, заминтируете значение на дефолтном операторском аккаунте, переводите часть баланса на другой аккаунт и проверяете итоговые транзакции и владение. Каждый шаг отражает потоки, открытые в SDK быстрого запуска Rust/Python/JavaScript, чтобы обеспечить надежный паритет между CLI и поведением SDK.

## Требования

- Следуйте [quickstart](./quickstart.md), чтобы запустить одноузловую сеть через
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Убедитесь, что `iroha` (CLI) собран или скачан, и что вы можете достучаться до
  пир через `defaults/client.toml`.
- Дополнительные помощники: `jq` (форматирование ответов JSON) и оболочка POSIX для
  фрагменты с переменными окружениями ниже.

По всей инструкции замените `$ADMIN_ACCOUNT` и `$RECEIVER_ACCOUNT` на нужные вам.
ID аккаунтов. В дефолтном комплекте уже есть два аккаунта, полученные из демо-ключей:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

Подтвердите значения, выведя первые аккаунты:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Осмотрите состояние генезиса

Перейдем к изучению реестра, на котором ориентирован CLI:

```sh
# Domains, зарегистрированные в genesis
iroha --config defaults/client.toml domain list all --table

# Accounts внутри wonderland (увеличьте --limit при необходимости)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions, которые уже существуют
iroha --config defaults/client.toml asset definition list all --table
```

Эти команды основаны на Norito-ответах, поэтому фильтрация и пагинация.
определено и согласовано с тем, что получен SDK.

## 2. Зарегистрируйте определение активации

Создайте новый бесконечный актив mintable `coffee` в домене `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI выводит хэш-доставку (например, `0x5f…`). Сохраните его, чтобы
позже проверю статус.

## 3. Замитьте значение на операторском аккаунте

Количество активности под парой `(asset definition, account)`. Замитьте 250
единица `coffee#wonderland` на `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Снова сохранить хэш-транзакцию (`$MINT_HASH`) из результатов CLI. Чтобы проверить баланс,
выполнить:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

или, чтобы получить только новые активы:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Переведите часть баланса на другой аккаунт

50 единиц с операторского аккаунта на `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Сохраните хэш-транзакцию как `$TRANSFER_HASH`. Запросите холдинги на обоих аккаунтах,
для проверки новых балансов:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Проверка документов реестра

Используйте сохраненные хэши, чтобы обеспечить коммит текущих транзакций:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Вы также можете стримить последние блоки, чтобы увидеть, какой блок включает перевод:

```sh
# Стрим от последнего блока и остановка через ~5 секунд
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Все команды выше используют те же полезные нагрузки Norito, что и SDK. Если вы повторите
этот поток в коде (см. Quickstarts SDK ниже), хэши и балансы совпадут при фундаментальной,
что вы ориентируетесь на ту же самую сеть и те же настройки по умолчанию.

## Ссылки на паритет SDK- [Rust SDK Quickstart](../sdks/rust) — бесплатные инструкции по настройке,
  отправку транзакций и опрос результатов из Rust.
- [Краткий старт Python SDK](../sdks/python) — показывает те же операции
  с Norito-помощники JSON.
- [Краткий старт JavaScript SDK](../sdks/javascript) — раскрывает запрос Torii,
  помощники управления и оболочки типизированных запросов.

Сначала выполните пошаговое руководство в CLI, затем повторите сценарий с предпочитаемым SDK,
чтобы убедиться, что обе стороны согласуются по хэшам транзакций, балансу и
результаты запроса.