---
lang: ja
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: ヤスデ
説明: 登録 -> 造幣 -> 転送 CLI `iroha` والتحقق من حالة السجل الناتجة。
スラッグ: /norito/ledger-walkthrough
---

[Norito クイックスタート](./quickstart.md) を使用してください。 CLI `iroha`。 ستسجل تعريف اصل جديدا، وتسك وحدات في حساب المشغل الافتراضي، وتنقل جزءا من الرصيد الىあなたのことを忘れないでください。開発者ガイド クイックスタート ガイド Rust/Python/JavaScript 開発者 管理者 管理者 CLI SDK。

## ああ、ああ

- [クイックスタート](./quickstart.md) 概要
  `docker compose -f defaults/docker-compose.single.yml up --build`。
- ピア `iroha` (CLI) ピア `defaults/client.toml` を実行します。
- バージョン: `jq` (JSON) POSIX バージョン。

`$ADMIN_ACCOUNT` と `$RECEIVER_ACCOUNT` を確認してください。バンドルのバンドルのバンドル:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

回答:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. 起源

CLI :

```sh
# Domains المسجلة في genesis
iroha --config defaults/client.toml domain list all --table

# Accounts داخل wonderland (استبدل --limit بعدد اكبر عند الحاجة)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland.universal"}' \
  --limit 10 --table

# Asset definitions الموجودة مسبقا
iroha --config defaults/client.toml asset definition list all --table
```

تعتمد هذه الاوامر على ردود مدعومة ب Norito، لذا يكون الترشيح والتقسيم حتميين ومتطابقين SDK をダウンロードします。

## 2. いいえ

アクセスポイント `coffee` アクセスポイント `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI ハッシュ المقدمة (مثلا `0x5f…`)。ありがとうございます。

## 3. いいえ、いいえ。

`(asset definition, account)` を確認してください。 250 日 `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` 日 `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

ハッシュ ハッシュ (`$MINT_HASH`) CLI を使用します。重要なポイント:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

回答:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. 問題を解決する

50 時間以内に `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

ハッシュ値は `$TRANSFER_HASH` です。回答:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. 問題を解決する

回答:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

عرض المزيد المزيد

```sh
# Stream من اخر كتلة والتوقف بعد ~5 ثوان
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

ペイロードと Norito SDK が含まれています。 هذا التدفق عبر الكود (انظر クイックスタート للـ SDK ادناه)، فستتطابق الهاشات والارصدة ما دمت最高のパフォーマンスを見せてください。

## 開発 SDK

- [Rust SDK クイックスタート](../sdks/rust) — Rust をインストールします。
- [Python SDK クイックスタート](../sdks/python) — 登録/造幣局 JSON مدعومة ب Norito。
- [JavaScript SDK クイックスタート](../sdks/javascript) — يغطي طلبات Torii، ومساعدات الحوكمة، واغلفة الاستعلامات المtyped。

CLI のセキュリティとセキュリティ SDK のセキュリティの強化ありがとうございます。