---
lang: he
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Пошаговый разбор реестра
תיאור: Воспроизведите детерминированный поток register -> מנטה -> העברה с CLI `iroha` и проверьте итоговое состояние реестра.
slug: /norito/ledger-walkthrough
---

צור הדרכה дополняет [Norito התחלה מהירה](./quickstart.md), בדוק, как менять и проверять состояние состояние `iroha`. Вы зарегистрируете новую дефиницию ACTIVа, заминтите единицы на дефолтный операторский актелева, на другой аккаунт и проверите итоговые транзакции и владения. Каждый шаг отражает потоки, покрытые в Quickstart SDK Rust/Python/JavaScript.

## Требования

- Следуйте [Quickstart](./quickstart.md), чтобы запустить одноузловую сеть через
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Убедитесь, что `iroha` (CLI) собран или скачан, и что вы можете достучаться до
  עמית через `defaults/client.toml`.
- אופציות אופציות: `jq` (פורמט JSON ответов) ו- POSIX shell עבור
  сниппетов с переменными окружения ниже.

По всей инструкции заменяйте `$ADMIN_ACCOUNT` ו-`$RECEIVER_ACCOUNT` на нужные вам
ID аккаунтов. חבילת ה-дефолтном уже есть два аккаунта, полученных из demo-ключей:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

Подтвердите значения, выведя первые аккаунты:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Осмотрите состояние בראשית

Начните с изучения реестра, на который нацелен CLI:

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

Эти команды опираются на Norito-ответы, поэтому фильтрация и пагинация
детерминированы и совпадают с тем, что получают SDK.

## 2. Зарегистрируйте дефиницию актива

Создайте новый бесконечно mintable актив `coffee` в домене `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI выведет хэш отправленной транзакции (например, `0x5f…`). Сохраните его, чтобы
позже проверить статус.

## 3. Замитьте единицы на операторский аккаунт

Количество актива живет под парой `(asset definition, account)`. Замитьте 250
единиц `coffee#wonderland` על `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
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
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" \
  --limit 1 | jq .
```

## 4. Переведите часть баланса на другой аккаунт

הצג 50 אקדמיות אופטימליות עבור `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Сохраните хэш транзакции как `$TRANSFER_HASH`. Запросите אחזקות на обоих аккаунтах,
чтобы проверить новые балансы:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${RECEIVER_ACCOUNT}\"}" --limit 1 | jq .
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

Все команды выше используют те же Norito, что и SDK. Если вы повторите
этот поток в коде (см. quickstarts SDK ниже), хэши и балансы совпадут при условии,
что вы нацелены на ту же сеть и те же ברירות מחדל.

## Ссылки на паритет SDK- [התחלה מהירה של SDK של חלודה](../sdks/rust) — демонстрирует регистрацию инструкций,
  отправку транзакций и סקרים статуса из Rust.
- [התחלה מהירה של Python SDK](../sdks/python) — פנה לרישום/נטה
  с עוזרי JSON בגיבוי Norito.
- [התחלה מהירה של JavaScript SDK](../sdks/javascript) — покрывает Torii запросы,
  עוזרי ממשל ועטיפות שאילתות מוקלדות.

הדרכה מפורטת ב-CLI, מצא את התוכנית עם ה-SDK של החברה,
чтобы убедиться, что обе поверхности согласуются по хэшам транзакций, балансам и
результатам запросов.