---
lang: ja
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: ٌجر واک تھرو
説明: `iroha` CLI کے ساتھ 確定的レジスタ -> ミント -> 転送 فلو دوبارہ بنائیں اور نتیجے میں لیجر اسٹیٹ کی تصدیقありがとう
スラッグ: /norito/ledger-walkthrough
---

یہ ウォークスルー [Norito クイックスタート](./quickstart.md) ٩ی تکمیل کرتا ہے اور دکھاتا ہے کہ `iroha` CLI کے ساتھ لیجر اسٹیٹ کو کیسے بدلیں اور چیک کریں۔資産定義 جسٹر کریں گے، ڈیفالٹ آپریٹر اکاؤنٹ میں 単位ミント کریں گے، بیلنس کا資本金 資本金 資本金 資本金 取引金 保有株数 資本金ありがとうございますRust/Python/JavaScript SDK クイックスタート فلو کی عکاسی کرتا ہے تاکہ آپ CLI اور SDK کے درمیان parity کی تصدیقありがとうございます

## پیشگی تقاضے

- [クイックスタート](./quickstart.md) فالو کریں تاکہ سنگل-پیئر نیٹ ورک کو
  `docker compose -f defaults/docker-compose.single.yml up --build` ذریعے بوٹ کیا جا سکے۔
- یقینی بنائیں کہ `iroha` (CLI) ビルド یا ダウンロード ہے اور آپ `defaults/client.toml` سے ピア تک پہنچ سکتے ہیں۔
- 処理: `jq` (JSON 応答の書式設定) POSIX シェルの処理 環境変数のスニペットの処理

アカウント `$ADMIN_ACCOUNT` `$RECEIVER_ACCOUNT` アカウント ID アカウント ID アカウント ID アカウント ID アカウント ID アカウント ID アカウント ID アカウント ID アカウント ID アカウント ID アカウント ID アカウント ID アカウント ID アカウント ID アカウント IDやあデフォルトバンドル پہلے ہی デモキー سے اخذ کیے گئے دو アカウント شامل کرتا ہے:

```sh
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

アカウントの数:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. ジェネシス اسٹیٹ کا معائنہ

CLI の操作:

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

Norito でバックアップされた応答 フィルタリング ページネーション決定論的 SDK حاصل کرتے ہیں۔

## 2. 資産の定義

`wonderland` 価額 `coffee` 価額 鋳造可能資産:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI が送信したトランザクション ハッシュ (مثلاً `0x5f…`) پرنٹ کرتا ہے۔ステータス クエリ ステータス クエリ ステータス ステータス

## 3. 単位ミント کریں

資産数量 `(asset definition, account)` جوڑے کے تحت رہتی ہیں۔ `$ADMIN_ACCOUNT` 価 `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` 250 単位ミント:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

CLI 出力「トランザクション ハッシュ (`$MINT_HASH`)」名前:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

資産の価値:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. بیلنس کا کچھ حصہ دوسرے اکاؤنٹ کو ٹرانسفر کریں

`$RECEIVER_ACCOUNT` 50 ユニットの数:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

トランザクション ハッシュ کو `$TRANSFER_HASH` کے طور پر محفوظ کریں۔保有株のクエリ、残高、および残高のクエリ:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. और देखें

ハッシュ値の計算とトランザクションのコミット値の計算:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

ブロック ストリーム ブロック ブロック ブロック ブロック ブロック ブロック ブロック ブロック ブロック ブロック ブロック ブロック ブロック ブロック ブロック ブロック ブロック

```sh
# جدید ترین block سے stream کریں اور ~5 seconds بعد رک جائیں
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Norito ペイロードと SDK の数ハッシュ バランス 整列 ハッシュ バランス 整列رہیں گے جب تک آپ اسی نیٹ ورک اور デフォルト کو ٹارگٹ کرتے ہیں۔

## SDK パリティ

- [Rust SDK クイックスタート](../sdks/rust) — Rust の手順、トランザクションの送信、ステータス ポーリング、ステータス ポーリング
- [Python SDK クイックスタート](../sdks/python) — Norito でサポートされる JSON ヘルパー - 登録/ミント操作の登録/ミント操作
- [JavaScript SDK クイックスタート](../sdks/javascript) - Torii リクエスト、ガバナンス ヘルパー、型付きクエリ ラッパー

CLI ウォークスルー پائیں، پھر اپنے پسندیدہ SDK کے ساتھ وہی منظرنامہ دہرائیں تاکہ دونوںトランザクション ハッシュ、残高、クエリ出力の詳細