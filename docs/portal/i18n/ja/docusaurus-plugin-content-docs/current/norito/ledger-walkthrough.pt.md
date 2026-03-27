---
lang: ja
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: 台帳のウォークスルー
説明: 完全な確定性を登録するための複製 -> ミント -> CLI `iroha` の転送、元帳の結果の検証。
スラッグ: /norito/ledger-walkthrough
---

Este ウォークスルーの補足 [Norito クイックスタート](./quickstart.md) は、CLI `iroha` を検査するための最も重要な情報です。登録担当者が新しい定義を決定し、オペレータ パドラオの管理者が管理し、トランスファー ホールディングスの成果物としての譲渡側の検証を確認してください。 SDK の最新のクイックスタートは、Rust/Python/JavaScript を使用して CLI と SDK を確認することができます。

## 前提条件

- Siga o [クイックスタート](./quickstart.md) による Unico ピアの最初の実行
  `docker compose -f defaults/docker-compose.single.yml up --build`。
- Garanta que `iroha` (o CLI) esteja compilado ou baixado e que voce consiga
  アクセス ピア `defaults/client.toml`。
- ヘルパーオプション: `jq` (JSON 形式) シェル POSIX パラメータ
  OS のスニペットは、さまざまな環境の米国の環境を提供します。

長い間ギアを交換してください `$ADMIN_ACCOUNT` と `$RECEIVER_ACCOUNT` ペロス ID
飛行機に乗ってください。 O バンドル パドラオ ジャ インクルード デュアス コンタス デリバダス ダス
デモ版:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

os valores listando を primeiras contas として確認します。

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. 起源の検査

帳簿や CLI を調べてみましょう:

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

Esses コマンドは、Norito の応答投稿に依存し、ページを決定するためのフィルターと SDK の受信を決定します。

## 2. 活動を定義する登録

無限の新しいミントテーブル シャマド `coffee` デントロ ドミニオ
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

O CLI インプリメまたはハッシュ ダ トランザクション エンビアダ (例、`0x5f...`)。ガーデオパラ
相談者はステータスが遅れています。

## 3. Minte unidades na conta do operador

`(asset definition, account)` の生存期間の量として。ミンテ250
`7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` と `$ADMIN_ACCOUNT` の結果:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

新しい、ハッシュ ダ トランザクション (`$MINT_HASH`) をキャプチャして CLI を実行します。パラ
サルドの確認、乗りました:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ああ、新たな活動を続けてください:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. トランスフィラ・パルテ・ド・サルド・パラ・コンタ

Mova 50 は `$RECEIVER_ACCOUNT` の操作を実行します:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

`$TRANSFER_HASH` を守ります。 OSホールディングスとアンバスに問い合わせる
新しいサルドスの検証内容として:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. 証拠として台帳を検証する

コミットされたtransacoesフォームとしてOSハッシュサルボスパラ確認クエリアンバスを使用します。

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

最近のブロックの音声ストリームを含む、すべてのブロックを共有します。
トランスファーレンシア:

```sh
# Stream a partir do ultimo bloco e pare apos ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Cada コマンド acima usa os mesmos ペイロード Norito que os SDK。声の繰り返し
codigo 経由の este fluxo (SDK abaixo の veja os クイックスタート)、os ハッシュとサルドス
デフォルトでは、システムがデフォルトのままになっています。

## SDK のリンク

- [Rust SDK クイックスタート](../sdks/rust) - レジストラのデモ、
  サブメーターのトランザクションと、Rust に関するコンサルタントのステータス。
- [Python SDK クイックスタート](../sdks/python) - メスマス オペラの登録/ミントとしてのほとんど
  com ヘルパー JSON respaldados por Norito。
- [JavaScript SDK クイックスタート](../sdks/javascript) - cobre リクエスト Torii、
  統治のヘルパーとクエリのヒントのラッパー。

CLI プライムのウォークスルー、SDK のデポジット、シナリオのデポジット
保証権を優先するための権利保護協定の締結
transacao、saldos e はクエリを出力します。