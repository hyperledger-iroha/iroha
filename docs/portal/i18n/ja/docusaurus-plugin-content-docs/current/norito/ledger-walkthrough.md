---
lang: ja
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: 台帳ウォークスルー
description: `iroha` CLI で決定的な register -> mint -> transfer フローを再現し、結果の台帳状態を検証します。
slug: /norito/ledger-walkthrough
---

このウォークスルーは [Norito クイックスタート](./quickstart.md) を補完し、`iroha` CLI を使って台帳状態を更新・確認する方法を示します。新しい資産定義を登録し、既定のオペレーターアカウントに単位をミントし、残高の一部を別アカウントへ転送し、結果のトランザクションと保有を確認します。各ステップは Rust/Python/JavaScript SDK のクイックスタートで扱うフローと対応しており、CLI と SDK の挙動が一致することを確認できます。

## 前提条件

- [クイックスタート](./quickstart.md) に従い、
  `docker compose -f defaults/docker-compose.single.yml up --build` で単一ピアネットワークを起動してください。
- `iroha` (CLI) がビルドまたはダウンロード済みで、`defaults/client.toml` で peer に到達できることを確認します。
- 任意の補助ツール: `jq` (JSON 応答の整形) と、下の環境変数スニペットに使う POSIX シェル。

ガイド全体で `$ADMIN_ACCOUNT` と `$RECEIVER_ACCOUNT` を使用するアカウント ID に置き換えてください。デフォルトの bundle にはデモ鍵由来の 2 つのアカウントが含まれています:

```sh
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

最初のアカウントを一覧して値を確認します:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Genesis 状態を確認する

まず CLI が対象としている台帳を探索します:

```sh
# Genesis で登録されている domains
iroha --config defaults/client.toml domain list all --table

# wonderland 内の accounts (必要なら --limit を増やす)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# 既に存在する asset definitions
iroha --config defaults/client.toml asset definition list all --table
```

これらのコマンドは Norito ベースの応答に依存するため、フィルタリングとページングは決定的で、SDK が受け取る内容と一致します。

## 2. 資産定義を登録する

`wonderland` ドメインに、無制限にミント可能な `coffee` を作成します:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI は送信されたトランザクションハッシュ (例: `0x5f…`) を表示します。後で状態を確認できるよう保存しておきます。

## 3. オペレーターアカウントにミントする

資産数量は `(asset definition, account)` の組み合わせに紐づきます。`coffee#wonderland` を 250 単位 `$ADMIN_ACCOUNT` にミントします:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

再び CLI 出力からトランザクションハッシュ (`$MINT_HASH`) を取得します。残高を確認するには次を実行します:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

または新しい資産だけに絞るには:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. 残高の一部を別アカウントへ転送する

オペレーターアカウントから `$RECEIVER_ACCOUNT` に 50 単位を移動します:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

トランザクションハッシュを `$TRANSFER_HASH` として保存します。両方のアカウントの保有を確認して新しい残高を検証します:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. 台帳の証跡を確認する

保存したハッシュを使って、両トランザクションがコミットされたことを確認します:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

最近のブロックをストリームして、どのブロックに転送が含まれたかを確認することもできます:

```sh
# 最新ブロックからストリームし、約 5 秒で停止
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

上記のすべてのコマンドは SDK と同じ Norito ペイロードを使います。コードでこのフローを再現する場合 (下の SDK クイックスタート参照)、同じネットワークとデフォルトを使っていればハッシュと残高が揃います。

## SDK パリティリンク

- [Rust SDK quickstart](../sdks/rust) — 命令の登録、トランザクション送信、Rust からのステータスポーリングを示します。
- [Python SDK quickstart](../sdks/python) — Norito JSON ヘルパーを使った register/mint を示します。
- [JavaScript SDK quickstart](../sdks/javascript) — Torii リクエスト、ガバナンスヘルパー、型付きクエリラッパーを扱います。

まず CLI のウォークスルーを実行し、その後に好みの SDK で同じシナリオを繰り返して、
トランザクションハッシュ、残高、クエリ結果が一致することを確認してください。
