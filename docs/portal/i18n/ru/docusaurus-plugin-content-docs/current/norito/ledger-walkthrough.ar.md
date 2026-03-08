---
lang: ru
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: جولة في السجل
описание: اعادة انتاج تدفق حتمي Registration -> Mint -> Transfer باستخدام CLI `iroha` والتحقق من حالة السجل Это так.
слизень: /norito/ledger-walkthrough
---

Нажмите на [Norito краткое руководство](./quickstart.md) Загрузите CLI `iroha`. Спродюсированный Дэвидом, он был в фильме "Старый город" в Нью-Йорке. В 2013 году он был убит в 1990-х годах в Нью-Йорке. Чтобы получить доступ к кратким руководствам по работе с Rust/Python/JavaScript, нажмите здесь. Используйте CLI и SDK.

## المتطلبات المسبقة

- اتبع [быстрый старт](./quickstart.md)
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Запускается в `iroha` (CLI) и используется для подключения к одноранговому узлу. `defaults/client.toml`.
- Доступ к файлу: `jq` (отображается в формате JSON) для POSIX-файлов и может быть изменен. الاسفل.

Для этого необходимо установить `$ADMIN_ACCOUNT` и `$RECEIVER_ACCOUNT`. В комплект поставки входит:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

По словам президента США:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. فحص حالة генезис

Откройте интерфейс командной строки:

```sh
# Domains المسجلة في genesis
iroha --config defaults/client.toml domain list all --table

# Accounts داخل wonderland (استبدل --limit بعدد اكبر عند الحاجة)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions الموجودة مسبقا
iroha --config defaults/client.toml asset definition list all --table
```

Он был использован в программе Norito, а также в 2017 году. Используйте его для создания SDK.

## 2. تسجيل تعريف اصل

Для этого необходимо установить `coffee` и установить `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

Проверьте хеш CLI (например, `0x5f…`). Это произошло в 2007 году.

## 3. Сделай это в Стиве

Установите флажок `(asset definition, account)`. На 250 секунд от `coffee#wonderland` до `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Создайте хеш-код (`$MINT_HASH`) в CLI. Сообщение от автора:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

В ответ на вопрос:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. В جزء من الرصيد الى حساب اخر

Через 50 дней после запуска `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Хэш был создан `$TRANSFER_HASH`. Сообщение о том, что происходит в фильме "Лаборатория", в разделе "Обзор":

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Нажмите на кнопку «Получить»

Сообщение о том, что произошло в 2017 году:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Он сказал, что в фильме "Лидерство" говорится:

```sh
# Stream من اخر كتلة والتوقف بعد ~5 ثوان
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Для создания полезных нагрузок используйте Norito для создания SDK. Получите дополнительную информацию (краткие руководства по использованию SDK) и нажмите кнопку «Установить». Он был убит в Нью-Йорке и в Нью-Йорке.

## Открыть SDK

- [Краткий старт Rust SDK](../sdks/rust)
- [Краткое руководство по Python SDK](../sdks/python) — необходимо выполнить команду Register/mint для создания JSON-файла Norito.
- [Краткое руководство по JavaScript SDK](../sdks/javascript) Напечатал.

Для работы с CLI используйте приложение SDK, а затем нажмите кнопку «Удалить». Он был создан в 1990-х годах в Нью-Йорке.