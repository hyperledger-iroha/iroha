---
lang: ru
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
название: Прохождение книги
описание: Воспроизведение детерминированного потока регистрации -> mint -> передача через CLI `iroha` и проверка состояния бухгалтерской книги.
слизень: /norito/ledger-walkthrough
---

Это пошаговое руководство дополняет [Norito краткое руководство] (./quickstart.md) для проверки состояния бухгалтерской книги с помощью CLI `iroha`. Голос регистратора по новому определенному активу, единый договор с оператором-отправителем, передача части дела для выполнения других операций и проверки результатов транзакций и холдингов. Давайте рассмотрим флюксы и ознакомимся с краткими руководствами по SDK Rust/Python/JavaScript для подтверждения совместимости между CLI и SDK.

## Предварительные требования

- Нажмите или [быстрый старт](./quickstart.md), чтобы запустить Rede de um Unico Peer через
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Гарантия, что `iroha` (или CLI) будет скомпилирован или загружен и что вы скажете
  доступ к пиру с использованием `defaults/client.toml`.
- Дополнительные помощники: `jq` (формат ответов JSON) и оболочка POSIX для
  фрагменты различных вариантов окружающей среды, используемые в любом месте.

В ближайшее время замените `$ADMIN_ACCOUNT` и `$RECEIVER_ACCOUNT` идентификаторами людей.
Conta que voce planeja usar. O Bundle Padrao ja Inclui Duas Contas Derivadas Das
демо-версия:

```sh
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

Подтвердите список значений в качестве первых сведений:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Инспекция о статусе происхождения

Приступайте к исследованию реестра, в котором находится CLI:

```sh
# Domains registrados no genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dentro de wonderland (substitua --limit por um numero maior se necessario)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions que ja existem
iroha --config defaults/client.toml asset definition list all --table
```

Эти команды зависят от ответов, полученных от Norito, включая фильтры и страницы, которые определены и соответствуют полученным SDK.

## 2. Регистрация определенного актива

Crie um novo ativo infinitamente mintable chamado `coffee` dentro do dominio
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI выводит хэш отправленной транзакции (например, `0x5f...`). Guarde-o для
консультант или статус более поздно.

## 3. Минте единые инструкции по работе с оператором

Как количество активных жизней по номиналу `(asset definition, account)`. Минта 250
Объединения `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` с `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Снова захватите хэш транзакции (`$MINT_HASH`) с помощью CLI. Пара
Подтверждаю сальдо, ехал:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

или, для того, чтобы увидеть новое начало:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Трансферная часть до конца, чтобы выйти за рамки

Нажмите 50 раз, чтобы связаться с оператором для `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Защитите хэш транзакции как `$TRANSFER_HASH`. Проконсультируйтесь с холдингами в посольствах
в качестве информации для проверки новых сальдо:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Verifique как доказательство в бухгалтерской книге

Используйте хеш-пакеты для подтверждения того, что транзакции зафиксированы в формате:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Голос там может быть недавним потоком блоков для того, чтобы включить в него блок
трансференсия:

```sh
# Stream a partir do ultimo bloco e pare apos ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Cada comando acima usa os mesmos payloads Norito que os SDK. Повторить свой голос
это Fluxo через Codigo (есть краткие руководства по SDK abaixo), хэши и файлы
Вао Алинхар, где вы говорите, что это один и тот же код и настройки по умолчанию.

## Дополнительные ссылки на SDK- [Краткий старт Rust SDK](../sdks/rust) - демонстрационные инструкции для регистратора,
  субметровые транзакции и статус консультанта по ржавчине.
- [Краткий старт Python SDK](../sdks/python)
  com helpers JSON, измененный на Norito.
- [Краткий старт JavaScript SDK](../sdks/javascript) - запросы cobre Torii,
  помощники в управлении и обертки для типичных запросов.

Сначала пройдите пошаговое руководство по CLI, затем повторите сценарий с помощью SDK для клиента.
Предпочтение для гарантии того, что Duas Superficies Concordem Em Hashes de
транзакции, продажи и результаты запроса.