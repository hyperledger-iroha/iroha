---
lang: ja
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Полаговый разбор рестра
説明: Воспроизведите детерминированный поток register -> mint -> transfer с CLI `iroha` итоговое состояние рестра.
スラッグ: /norito/ledger-walkthrough
---

チュートリアル ビデオ [Norito クイックスタート](./quickstart.md)、コマンド ライン、コマンドライン インターフェイス、CLI のインターフェイス`iroha`。 Вы зарегистрируете новую дефиницию актива, заминтите единицы на дефолтный операторский аккаунт, переведете問題は、そのような問題を解決することです。クイックスタート SDK Rust/Python/JavaScript から、CLI および SDK へのアクセスが可能です。

## Требования

- Следуйте [クイックスタート](./quickstart.md)、чтобы запустить одноузловую сеть через
  `docker compose -f defaults/docker-compose.single.yml up --build`。
- Убедитесь、что `iroha` (CLI) собран или скачан, и что вы можете достучаться до
  ピア `defaults/client.toml`。
- バージョン: `jq` (JSON ответов) および POSIX シェル
  最高です。

По всей инструкции заменяйте `$ADMIN_ACCOUNT` および `$RECEIVER_ACCOUNT` на нужные вам
ID は。バンドルを使用して、デモ版をバンドルします:

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

Подтвердите значения, выведя первые аккаунты:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. 起源の物語

CLI での操作:

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

Эти команды опираются на Norito-ответы、поэтому фильтрация и пагинация
SDK を使用してください。

## 2. Зарегистрируйте дефиницию актива

`coffee` と `wonderland` のミントテーブル:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI は отправленной транзакции (например、`0x5f…`) を実行します。 Сохраните его, чтобы
ありがとうございます。

## 3. Замитьте единицы на операторский аккаунт

Количество актива живет под парой `(asset definition, account)`. Замитьте 250
`7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` または `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

CLI を使用して (`$MINT_HASH`) を実行します。 Чтобы проверить баланс,
要点:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

または、чтобы получить только новый актив:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Переведите часть баланса на другой аккаунт

50 日以内に `$RECEIVER_ACCOUNT` を表示:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Сохраните хэг транзакции как `$TRANSFER_HASH`. Запросите Holdings на обоих аккаунтах,
の結果:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Проверьте доказательства реестра

Используйте сохраненные хэзи, чтобы подтвердить коммит обеих транзакций:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Вы также можете стримить последние блоки, чтобы увидеть, какой блок включил перевод:

```sh
# Стрим от последнего блока и остановка через ~5 секунд
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Norito ペイロード、SDK を参照してください。 Если вы повторите
этот поток в коде (см.quickstarts SDK ниже)、хэли и балансы совпадут при условии、
デフォルトのままです。

## Ссылки на паритет SDK

- [Rust SDK クイックスタート](../sdks/rust) — демонстрирует регистрацию инструкций,
  Rust とポーリングを実行します。
- [Python SDK クイックスタート](../sdks/python) — 登録/ミント
  Norito ベースの JSON ヘルパー。
- [JavaScript SDK クイックスタート](../sdks/javascript) — покрывает Torii запросы,
  ガバナンス ヘルパーと型付きクエリ ラッパー。

CLI のウォークスルー、SDK のインストール、
чтобы убедиться, что обе поверхности согласуются по хэлам транзакций, балансам и
そうです。