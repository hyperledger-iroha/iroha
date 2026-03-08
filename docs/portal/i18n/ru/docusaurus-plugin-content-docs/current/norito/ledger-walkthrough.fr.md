---
lang: ru
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: Parcours du registre
описание: Воспроизведение детерминированного регистра потока -> mint -> передача с CLI `iroha` и проверка результата в реестре.
слизень: /norito/ledger-walkthrough
---

Это полный файл [quickstart Norito](./quickstart.md) с модификатором комментариев и инспектором государственного реестра с CLI `iroha`. Вы регистрируете новое определение действия, объединяете счетов операторов по умолчанию, передаете участникам сделки другой счет и проверяете транзакции и избегаете результатов. Шаг шага отражает потоки, которые можно просмотреть в кратких руководствах SDK Rust/Python/JavaScript, чтобы подтвердить соответствие между CLI и поддержкой SDK.

## Предварительное условие

- Suivez le [quickstart](./quickstart.md) для разграничения монопарного соединения через
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Убедитесь, что `iroha` (le CLI) создан или телезаряжается и что вы можете
  присоединиться к сверстнику с `defaults/client.toml`.
- Дополнительные параметры: `jq` (формат ответов JSON) и оболочка POSIX для файлов.
  фрагменты переменных окружающей среды.

Во всем длинном руководстве замените `$ADMIN_ACCOUNT` и `$RECEIVER_ACCOUNT` по этим параметрам.
Идентификаторы счета, которые вы используете. Пакет по умолчанию, включающий Deja Deux Comptes
выпуск демо-версии:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

Подтвердите значения в списке главных счетов:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Государственный инспектор по генезису

Начните с просмотра книги реестра с помощью CLI:

```sh
# Domains enregistres en genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dans wonderland (remplacez --limit par un nombre plus eleve si besoin)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions qui existent deja
iroha --config defaults/client.toml asset definition list all --table
```

Эти команды возвращаются к ответам Norito, используйте фильтрацию и нумерацию страниц.
 определены и соответствуют тому, что получено от SDK.

## 2. Регистрация и определение действия

Creez un nouvel actif infiniment mintable appele `coffee` в домене
`wonderland` :

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI добавляет хэш суммы транзакции (например, `0x5f...`). Консерве-ле
для консультанта по закону плюс поздно.

## 3. Minter des unites sur le Compte Operationur

Количество действий, живущих в одной паре `(asset definition, account)`. Минтез 250
Объединяет `coffee#wonderland` и `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Повторно выполните восстановление хэша транзакции (`$MINT_HASH`) после вылазки CLI. Налить
проверка le Solde, выполнение:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

или, для выбора нового действия:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Передача участника сделки или другого счета

Замените 50 единиц операторов-компьютеров версии `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Сохраните хэш транзакции под именем `$TRANSFER_HASH`. Interrogez les avoirs des deux comptes
для проверки новых солей:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Проверка прав на учетную запись

Используйте сохраненные хеши для подтверждения двух транзакций в комитетах:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Вы можете также стримерить последние блоки, чтобы увидеть, как блокировать включение передачи:

```sh
# Stream depuis le dernier bloc et arretez apres ~5 secondes
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```Все команды ci-dessus используют полезные нагрузки мемов Norito, которые используются SDK. Si vous
воспроизводить этот поток с помощью кода (например, быстрые запуски SDK), хэшей и т. д.
Soldes Seront Alignes tant que vous ciblez le Meme Reseau и les Memes по умолчанию.

## SDK залога

- [Краткий старт Rust SDK](../sdks/rust) - дополнительные инструкции по регистрации,
  опрос по транзакциям и опрос по закону Rust.
- [Краткий старт Python SDK](../sdks/python) - регистр операций montre les memes/mint
  avec des helpers JSON имеет Norito.
- [Краткое руководство по JavaScript SDK](../sdks/javascript) - можно выполнить запросы Torii,
  Помощники по управлению и обертки запрошенных типов.

Выполните отмену пошагового руководства через CLI, и вы сможете повторить сценарий с вашим SDK.
Предпочитаю, чтобы вы убедились, что две поверхности соответствуют хэшам
транзакции, продажи и результаты запросов.