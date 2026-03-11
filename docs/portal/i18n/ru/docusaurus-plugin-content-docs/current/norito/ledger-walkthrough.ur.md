---
lang: ru
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: لیجر واک تھرو
описание: `iroha` CLI Детерминированный регистр -> mint -> передача تصدیق کریں۔
слизень: /norito/ledger-walkthrough
---

Краткое руководство [Norito краткое руководство](./quickstart.md) Для использования `iroha` CLI. ساتھ لیجر اسٹیٹ کو کیسے بدلیں چیک کریں۔ آپ ایک نئی определение актива رجسٹر کریں گے، ڈیفالٹ آپریٹر اکاؤنٹ کریں کریں گے، بیلنس کا کچھ حصہ دوسرے کاؤنٹ کو ٹرانسفر کریں گے, اور نتیجے میڌ آنے Транзакции и холдинги, которые можно использовать Краткое руководство по Rust/Python/JavaScript SDK Использование и использование SDK для CLI درمیان parity کی تصدیق کر سکیں۔

## پیشگی تقاضے

- [быстрый старт](./quickstart.md)
  `docker compose -f defaults/docker-compose.single.yml up --build` کے ذریعے بوٹ کیا جا سکے۔
- Загрузите сборку `iroha` (CLI) и загрузите `defaults/client.toml` для однорангового узла. ہیں۔
- Доступно: `jq` (ответы JSON и форматирование) В оболочке POSIX можно использовать фрагменты фрагментов переменных среды. ہو سکیں۔

Если вы хотите использовать `$ADMIN_ACCOUNT` или `$RECEIVER_ACCOUNT`, вы можете указать идентификаторы учетных записей. چاہتے ہیں۔ Пакет настроек по умолчанию и демо-ключи, а также учетные записи и учетные записи:

```sh
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

Ниже приведены учетные записи, которые могут быть использованы для:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Бытие اسٹیٹ کا معائنہ

CLI позволяет получить доступ к следующим функциям:

```sh
# genesis میں رجسٹرڈ domains
iroha --config defaults/client.toml domain list all --table

# wonderland کے اندر accounts (ضرورت ہو تو --limit بڑھائیں)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# وہ asset definitions جو پہلے سے موجود ہیں
iroha --config defaults/client.toml asset definition list all --table
```

یہ کمانڈز Norito-поддерживаемые ответы для проверки фильтрации и детерминированной нумерации страниц. Использование SDK

## 2. Определение актива رجسٹر کریں

`wonderland` ڈومین کے `coffee` Для создания монетного актива:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI отправил хэш транзакции (مثلاً `0x5f…`). اسے محفوظ کریں تاکہ بعد میں status کو query کیا جا سکے۔

## 3. آپریٹر اکاؤنٹ میں unit mint کریں

количества активов `(asset definition, account)` کے جوڑے کے تحت رہتی ہیں۔ `$ADMIN_ACCOUNT` в упаковке `coffee#wonderland` — 250 штук в мятом виде:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Вывод CLI — хеш транзакции (`$MINT_HASH`). Вот как можно использовать:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

Какие активы могут быть использованы для:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. بیلنس کا کچھ حصہ دوسرے اکاؤنٹ کو ٹرانسفر کریں

Для заказа `$RECEIVER_ACCOUNT` требуется 50 единиц:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

хеш транзакции کو `$TRANSFER_HASH` کے طور پر محفوظ کریں۔ Вы можете запросить холдинги и узнать балансы, например:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Сделайте выбор в пользу

Для хэшей и операций с фиксацией, например:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

В блоках потока или потоковом режиме, а также в блоках передачи и блокировке данных:

```sh
# جدید ترین block سے stream کریں اور ~5 seconds بعد رک جائیں
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Использование полезных нагрузок Norito для использования с SDK Здесь вы можете найти необходимые инструменты (быстрые руководства по SDK), а также хэши и балансы. Вы можете выровнять настройки по умолчанию и использовать настройки по умолчанию.

## Проверка четности SDK- [Краткий старт Rust SDK](../sdks/rust) — Инструкции по Rust رجسٹر کرنا، submit کرنا، اور status poll کرنا دکھاتا ہے۔
- [Краткое руководство по Python SDK](../sdks/python) — Norito-помощники JSON для выполнения операций регистрации и монетирования.
- [Краткое руководство по JavaScript SDK](../sdks/javascript) — запросы Torii, помощники управления, а также оболочки типизированных запросов.

Подробное руководство по использованию CLI в разделе SDK и дополнительные сведения о возможностях использования CLI. Здесь можно просмотреть хэши транзакций, балансы, результаты запросов и многое другое.