---
lang: ja
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: 登録公園
説明: 流動決定レジスタを再現 -> ミント -> 平均 CLI `iroha` を転送し、台帳の結果を検証します。
スラッグ: /norito/ledger-walkthrough
---

完全なファイル [クイックスタート Norito](./quickstart.md) 監視コメント修飾子と検査官、元帳平均 CLI `iroha` を確認します。新しい定義を登録し、デフォルトで計算する操作者を決定し、ソルド対自動計算および検証を実行し、トランザクションおよび結果を転送します。クイックスタート SDK Rust/Python/JavaScript を参照して、CLI と SDK のコンポーネントを確認してください。

## 前提条件

- Suivez le [クイックスタート](./quickstart.md) demarrer le reseau モノペアを経由して追加
  `docker compose -f defaults/docker-compose.single.yml up --build`。
- `iroha` (CLI) の電気充電とプーベスの構成を保証します。
  結合ファイル ピア アベック `defaults/client.toml`。
- オプション : `jq` (レスポンス JSON 形式) およびシェル POSIX プール ファイルのアンシェル
  環境変数のスニペット。

長いガイド、`$ADMIN_ACCOUNT` と `$RECEIVER_ACCOUNT` の説明
comptez utiliser の compte que の ID。デジャドゥコンプを含むデフォルトのバンドル
デモの課題:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

値のリストとプルミエのコンパイルを確認します:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. レタ警部の起源

CLI でエクスプローラーとレジャーを開始します。

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

応答 Norito の応答コマンド、濾過とページネーションのドングル
 SDK の決定と連絡先の確認。

## 2. 登録者の定義

Creez un nouvel actif infiniment mintable appele `coffee` dans le Domaine
`wonderland` :

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

トランザクションのハッシュの CLI 添付ファイル (例、`0x5f...`)。コンセルベス・ル
相談者ル・ステータス・プラス・タードを注ぎます。

## 3. 運営者を管理する

生き生きとした活動の量 `(asset definition, account)`。ミンテス250
`coffee#wonderland` と `$ADMIN_ACCOUNT` の結合:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

CLI で出撃するトランザクション (`$MINT_HASH`) をアンコールしてください。注ぐ
検証者 le solde、executez :

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ああ、新しい活動を注いでください。

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. 転送者は、ソルドとオートルの関係を共有します

Deplacez 50 ユニット デュ コンテ オペレーター 対 `$RECEIVER_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

トランザクションのハッシュ値は `$TRANSFER_HASH` です。尋問 le avoirs des deux comptes
検証者レ・ヌーボー・ソルドを注ぐ：

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. 台帳の検証者

委員会での取引確認者を利用して、次のことを行います。

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

オーストラリアのストリーマーのブロックの最近のブロックに転送が含まれています:

```sh
# Stream depuis le dernier bloc et arretez apres ~5 secondes
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

ファイル コマンド ci-dessus utilisent ファイル ミーム ペイロード Norito クエリ SDK を示します。シヴー
コード経由でフラックスを再現 (クイックスタート SDK ci-desous を参照)、ハッシュなど
ソルデスは、ミームの検索とミームのデフォルトを解決します。

## 先取特権 SDK

- [Rust SDK クイックスタート](../sdks/rust) - 登録手順の説明、
  Rust のトランザクションおよびポーリングの要求。
- [Python SDK クイックスタート](../sdks/python) - montre les memes 操作 register/mint
  avec des helpers JSON は Norito を採用しています。
- [JavaScript SDK クイックスタート](../sdks/javascript) - リクエスト Torii、
  統治のヘルパーと要求タイプのラッパー。

ウォークスルー CLI の実行、シナリオの実行 SDK の実行
保証するものを優先して表面を一致させてください。
トランザクション、販売および要求の結果。