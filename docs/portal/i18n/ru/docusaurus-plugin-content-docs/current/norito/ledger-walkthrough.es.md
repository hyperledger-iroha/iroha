---
lang: ru
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: Recorrido del libro mayor
описание: Воспроизведение определенного потока регистрации -> mint -> передача с помощью CLI `iroha` и проверка состояния результата в бухгалтерской книге.
слизень: /norito/ledger-walkthrough
---

Эта запись дополняет [быстрое начало Norito](./quickstart.md) как изменение и проверка состояния реестра с помощью CLI `iroha`. Регистрируются новые активные определения, единые учетные записи в учетной записи оператора по дефекту, передача части баланса на другой счет и проверка результатов транзакций и результатов. Каждый раз вспоминайте о графических инструментах и ​​кратких руководствах по SDK для Rust/Python/JavaScript, чтобы можно было подтвердить соответствие между CLI и SDK.

## Предыдущие реквизиты

- Нажмите [быстрый старт] (./quickstart.md), чтобы начать красный одноранговый узел через
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Убедитесь, что `iroha` (CLI) является скомпилированным или удаленным и может быть удален.
  Алькансар-эль-Пэр usando `defaults/client.toml`.
- Дополнительные помощники: `jq` (формат ответов JSON) и оболочка POSIX для
  фрагменты переменных, используемые в данный момент.

На большом экране замените `$ADMIN_ACCOUNT` и `$RECEIVER_ACCOUNT` с лос
Идентификаторы того, что вы планируете использовать. Пакет из-за дефекта включает в себя дос cuentas
Демо-версия Derivadas de las claves:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

Подтвердите список значений в списке первых чисел:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Инспекциона эль-Эстадо-Генезис

Откройте реестр, используя CLI:

```sh
# Domains registrados en genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dentro de wonderland (reemplaza --limit por un numero mayor si hace falta)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland.universal"}' \
  --limit 10 --table

# Asset definitions que ya existen
iroha --config defaults/client.toml asset definition list all --table
```

Эти команды основаны на ответах, полученных от Norito, потому что фильтрация и пагинация являются детерминированными и совпадают с тем, что получен SDK.

## 2. Регистрация определения активности

Crea un nuevo activo infinitamente acuable llamado `coffee` dentro del dominio
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI отображает хеш отправленной транзакции (например, `0x5f...`). Охраняйтесь, чтобы проконсультироваться по поводу состояния здоровья.

## 3. Acuna unidades en la cuenta deloperador

Las cantidades de activos viven bajo el par `(asset definition, account)`. Акуна
250 единиц `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` и `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Новый захват хэша транзакции (`$MINT_HASH`) из CLI. Пара
проверка баланса, вывод:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

о, чтобы начать работу в новом активном режиме:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Переведите часть баланса в другую сторону.

Выберите 50 единиц управления на `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Защитите хэш транзакции как `$TRANSFER_HASH`. Посоветуйтесь с холдингами в гостях
cuentas для проверки новых балансов:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Проверка доказательств бухгалтерской книги

Используйте хеши для подтверждения того, что транзакции подтверждаются:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Также можно передать блоки, чтобы они могли включить блокировку в передачу:

```sh
# Stream desde el ultimo bloque y detente despues de ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```Передняя команда команды США использует полезные нагрузки Norito, которые используются в SDK. Си реплики
это Flujo Mediante Codigo (быстрое руководство по SDK), хэши и балансы
совпало с тем, что вы выбрали красный цвет по умолчанию.

## Связывание с SDK

- [Краткий старт Rust SDK](../sdks/rust) — инструкции для регистраторов,
  Отправьте транзакции и проконсультируйтесь по поводу Rust.
- [Краткий старт Python SDK](../sdks/python)
  Помощники JSON заменены на Norito.
- [Краткий старт JavaScript SDK](../sdks/javascript) - cubre solicitudes Torii,
  помощники правительства и обертки типовых запросов.

Сначала выведите запись CLI, а затем повторите сценарий с SDK.
предпочитаю гарантировать, чтобы были подтверждены хэши транзакций,
балансы и результаты консультаций.