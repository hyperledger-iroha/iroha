---
lang: ja
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: 市長のレコリード
説明: 登録の決定 -> ミント -> 転送コントロール CLI `iroha` を再現し、台帳の結果を検証します。
スラッグ: /norito/ledger-walkthrough
---

検査は完全に補完され、[Norito](./quickstart.md) CLI `iroha` で検査が行われます。レジストラは新しい活動の定義、欠陥のあるオペラ座の管理、転送の部分のバランスを確認し、取引の結果を確認します。 Rust/Python/JavaScript の SDK クイックスタートを参照して、CLI と SDK を確認してください。

## 以前の要件

- Sigue el [クイックスタート](./quickstart.md) ソロピア経由で最初に実行
  `docker compose -f defaults/docker-compose.single.yml up --build`。
- `iroha` (el CLI) のコンパイルを実行し、プエダを削除する
  アルカンザール エル ピア ウサンド `defaults/client.toml`。
- オプションのヘルパー: `jq` (JSON 形式) および POSIX パラメータのアンシェル
  変数のスニペットが表示されます。

ギアのラルゴ、リエンプラザ `$ADMIN_ACCOUNT` と `$RECEIVER_ACCOUNT` コンロス
ユーザーの飛行機の ID。欠陥やドス クエンタスを含むエル バンドル
ラス クラーベスのデリバダのデモ:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

ロス・ヴァローレス・リストアンド・ラス・プリメラス・クエンタスを確認:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. 検査の起源

Empieza は CLI で帳簿を調べます:

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

Norito からのコマンドの実行、フィルタリングとページの決定と、SDK の受信との一致。

## 2. 活動定義登録

新しい活動を無限に作成し、実行可能なラマド `coffee` デントロ デル ドミニオを作成します。
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI のインプリメエル ハッシュ デ ラ トランザクション環境 (例、`0x5f...`)。グアルダロ パラ コンサルタント エル スタド マス タルデ。

## 3. アクニャ ユニダデス アン ラ クエンタ デル オペラドール

`(asset definition, account)` での活動を続けてください。アクーニャ
`7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` と `$ADMIN_ACCOUNT` の 250 単位:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

新しい、トランザクション ハッシュ (`$MINT_HASH`) の CLI のキャプチャ。パラ
検証残高、排出量:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ああ、新しい活動をするときは、ソロで活動してください:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. バランスを保つためのトランスフィエレ

Mueve 50 unidades de la cuenta del operador a `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

ハッシュ デ トランザクション コモ `$TRANSFER_HASH` を保護します。アンバスでロスホールディングスに問い合わせてください
cuentas para verificar los nuevos 残高:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. 台帳の証拠を検証する

米国は、確認のための安全な措置を講じます:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

転送ブロックを含むブロックの確認:

```sh
# Stream desde el ultimo bloque y detente despues de ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Cada commando anterior usa los missmos ペイロード Norito que los SDK。 Siレプリカ
este flujo mediante codigo (SDK abajo のクイックスタート)、ハッシュとバランスの損失
デフォルトと一致します。

## SDK を実装する

- [Rust SDK クイックスタート](../sdks/rust) - 登録サービスの説明、
  Rust との取引やコンサルタントのサポートに感謝します。
- [Python SDK クイックスタート](../sdks/python) - 登録/ミント操作の管理
  con helpers JSON respaldados por Norito。
- [JavaScript SDK クイックスタート](../sdks/javascript) - キューブ ソリチュード Torii、
  ゴベルナンザのヘルパーとクエリのラッパー。

CLI の最初の記録、SDK のシナリオの繰り返し
取引上の優先権を確保するための優先権、
相談結果のバランスをとります。