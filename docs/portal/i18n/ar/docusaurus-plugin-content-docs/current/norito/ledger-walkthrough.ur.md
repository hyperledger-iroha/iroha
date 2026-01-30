---
lang: ar
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: لیجر واک تھرو
description: `iroha` CLI کے ساتھ deterministic register -> mint -> transfer فلو دوبارہ بنائیں اور نتیجے میں لیجر اسٹیٹ کی تصدیق کریں۔
slug: /norito/ledger-walkthrough
---

یہ walkthrough [Norito quickstart](./quickstart.md) کی تکمیل کرتا ہے اور دکھاتا ہے کہ `iroha` CLI کے ساتھ لیجر اسٹیٹ کو کیسے بدلیں اور چیک کریں۔ آپ ایک نئی asset definition رجسٹر کریں گے، ڈیفالٹ آپریٹر اکاؤنٹ میں units mint کریں گے، بیلنس کا کچھ حصہ دوسرے اکاؤنٹ کو ٹرانسفر کریں گے، اور نتیجے میں آنے والی transactions اور holdings کی تصدیق کریں گے۔ ہر قدم Rust/Python/JavaScript SDK quickstarts میں کورڈ فلو کی عکاسی کرتا ہے تاکہ آپ CLI اور SDK کے درمیان parity کی تصدیق کر سکیں۔

## پیشگی تقاضے

- [quickstart](./quickstart.md) فالو کریں تاکہ سنگل-پیئر نیٹ ورک کو
  `docker compose -f defaults/docker-compose.single.yml up --build` کے ذریعے بوٹ کیا جا سکے۔
- یقینی بنائیں کہ `iroha` (CLI) build یا download ہے اور آپ `defaults/client.toml` سے peer تک پہنچ سکتے ہیں۔
- اختیاری مددگار: `jq` (JSON responses کی formatting) اور POSIX shell تاکہ نیچے دیے گئے environment-variable snippets استعمال ہو سکیں۔

اس گائیڈ میں `$ADMIN_ACCOUNT` اور `$RECEIVER_ACCOUNT` کو ان account IDs سے بدلیں جو آپ استعمال کرنا چاہتے ہیں۔ defaults bundle پہلے ہی demo keys سے اخذ کیے گئے دو accounts شامل کرتا ہے:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

پہلے چند accounts لسٹ کر کے ویلیوز کی تصدیق کریں:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. genesis اسٹیٹ کا معائنہ

CLI جس لیجر کو ٹارگٹ کر رہا ہے اسے ایکسپلور کریں:

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

یہ کمانڈز Norito-backed responses پر انحصار کرتی ہیں، لہذا filtering اور pagination deterministic ہیں اور وہی ہیں جو SDKs حاصل کرتے ہیں۔

## 2. asset definition رجسٹر کریں

`wonderland` ڈومین کے اندر `coffee` نام کا ایک نیا، لامحدود mintable asset بنائیں:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI submitted transaction hash (مثلاً `0x5f…`) پرنٹ کرتا ہے۔ اسے محفوظ کریں تاکہ بعد میں status کو query کیا جا سکے۔

## 3. آپریٹر اکاؤنٹ میں units mint کریں

asset quantities `(asset definition, account)` کے جوڑے کے تحت رہتی ہیں۔ `$ADMIN_ACCOUNT` میں `coffee#wonderland` کی 250 units mint کریں:

```sh
iroha --config defaults/client.toml asset mint \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --quantity 250
```

CLI output سے transaction hash (`$MINT_HASH`) محفوظ کریں۔ بیلنس دوبارہ چیک کرنے کے لئے چلائیں:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

یا صرف نئے asset کو ٹارگٹ کرنے کے لئے:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" \
  --limit 1 | jq .
```

## 4. بیلنس کا کچھ حصہ دوسرے اکاؤنٹ کو ٹرانسفر کریں

آپریٹر اکاؤنٹ سے `$RECEIVER_ACCOUNT` کو 50 units منتقل کریں:

```sh
iroha --config defaults/client.toml asset transfer \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

transaction hash کو `$TRANSFER_HASH` کے طور پر محفوظ کریں۔ دونوں اکاؤنٹس پر holdings query کریں تاکہ نئی balances کی تصدیق ہو:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${RECEIVER_ACCOUNT}\"}" --limit 1 | jq .
```

## 5. لیجر ایویڈنس کی تصدیق

محفوظ hashes استعمال کریں تاکہ دونوں transactions کے commit ہونے کی تصدیق ہو:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

آپ حالیہ blocks بھی stream کر سکتے ہیں تاکہ دیکھا جا سکے کہ transfer کس block میں شامل ہوا:

```sh
# جدید ترین block سے stream کریں اور ~5 seconds بعد رک جائیں
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

اوپر کی ہر کمانڈ وہی Norito payloads استعمال کرتی ہے جو SDKs استعمال کرتے ہیں۔ اگر آپ اس فلو کو کوڈ کے ذریعے دہرائیں (نیچے SDK quickstarts دیکھیں)، تو hashes اور balances اس وقت تک align رہیں گے جب تک آپ اسی نیٹ ورک اور defaults کو ٹارگٹ کرتے ہیں۔

## SDK parity لنکس

- [Rust SDK quickstart](../sdks/rust) — Rust سے instructions رجسٹر کرنا، transactions submit کرنا، اور status poll کرنا دکھاتا ہے۔
- [Python SDK quickstart](../sdks/python) — Norito-backed JSON helpers کے ساتھ وہی register/mint operations دکھاتا ہے۔
- [JavaScript SDK quickstart](../sdks/javascript) — Torii requests، governance helpers، اور typed query wrappers کو کور کرتا ہے۔

پہلے CLI walkthrough چلائیں، پھر اپنے پسندیدہ SDK کے ساتھ وہی منظرنامہ دہرائیں تاکہ دونوں سطحیں transaction hashes، balances، اور query outputs پر متفق ہوں۔
