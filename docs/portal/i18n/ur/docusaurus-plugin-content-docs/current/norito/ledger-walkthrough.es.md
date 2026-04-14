---
lang: ur
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: لیجر ٹور
تفصیل: `iroha` CLI کے ساتھ ایک تعی .ن شدہ رجسٹر -> ٹکسال -> منتقلی کے بہاؤ کو دوبارہ پیش کرتا ہے اور اس کے نتیجے میں لیجر اسٹیٹ کی جانچ پڑتال کرتا ہے۔
سلگ: /نوریٹو /لیجر واک تھرو
---

یہ واک تھرو [Norito کوئیک اسٹارٹ] (./quickstart.md) کو Norito CLI کے ساتھ لیجر کی حیثیت کو کس طرح تبدیل اور معائنہ کرنے کا طریقہ دکھا کر ہے۔ آپ ایک نئی اثاثہ تعریف ، پہلے سے طے شدہ تاجر اکاؤنٹ میں یونٹوں کو مسدود کریں گے ، توازن کا کچھ حصہ دوسرے اکاؤنٹ میں منتقل کریں گے اور اس کے نتیجے میں لین دین اور ہولڈنگ کی تصدیق کریں گے۔ ہر قدم زنگ/ازگر/جاوا اسکرپٹ ایس ڈی کے کوئیک اسٹارٹ میں ڈھکے ہوئے بہاؤ کو آئینہ دیتا ہے تاکہ آپ سی ایل آئی اور ایس ڈی کے کے مابین برابری کی تصدیق کرسکیں۔

## شرائط

- سنگل ہم مرتبہ نیٹ ورک کے ذریعے شروع کرنے کے لئے [کوئیک اسٹارٹ] (./quickstart.md) کی پیروی کریں
  `docker compose -f defaults/docker-compose.single.yml up --build`۔
- اس بات کو یقینی بنائیں کہ `iroha` (CLI) مرتب یا ڈاؤن لوڈ کیا گیا ہے اور آپ کر سکتے ہیں
  `defaults/client.toml` کا استعمال کرتے ہوئے ہم مرتبہ تک پہنچیں۔
- اختیاری مددگار: `jq` (JSON رسپانس فارمیٹنگ) اور ایک POSIX شیل
  ماحولیاتی متغیر ٹکڑوں کو ذیل میں استعمال کیا جاتا ہے۔

گائیڈ کے دوران ، `$ADMIN_ACCOUNT` اور `$RECEIVER_ACCOUNT` کو تبدیل کریں
اکاؤنٹ IDs جن کا آپ استعمال کرنے کا ارادہ رکھتے ہیں۔ پہلے سے طے شدہ بنڈل میں پہلے ہی دو اکاؤنٹس شامل ہیں
ڈیمو کیز سے ماخوذ:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

پہلے اکاؤنٹس کی فہرست دے کر اقدار کی تصدیق کریں:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. پیدائش کی حالت کا معائنہ کریں

لیجر کی کھوج سے شروع کریں جس کی طرف سی ایل آئی اشارہ کرتا ہے:

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

یہ کمانڈ Norito کے ذریعہ تعاون یافتہ جوابات پر مبنی ہیں ، لہذا فلٹرنگ اور صفحہ بندی کا تعین کرنے والا ہے اور ایس ڈی کے کیا وصول کرتے ہیں اس سے ملتے ہیں۔

## 2. اثاثہ کی تعریف کو رجسٹر کریں

ڈومین کے اندر `coffee` نامی ایک نیا لامحدود تحریری اثاثہ بناتا ہے
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI جمع کروائے گئے ٹرانزیکشن کی ہیش پرنٹ کرتا ہے (مثال کے طور پر ، `0x5f...`)۔ بعد میں حیثیت کی جانچ پڑتال کے لئے اسے محفوظ کریں۔

## 3. آپریٹر کے اکاؤنٹ میں کیش یونٹ

اثاثہ کی رقم جوڑی `(asset definition, account)` کے تحت رہتی ہے۔ پالنا
`$ADMIN_ACCOUNT` میں `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` کے 250 یونٹ:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

ایک بار پھر ، CLI آؤٹ پٹ سے ٹرانزیکشن ہیش (`$MINT_HASH`) پر قبضہ کریں۔ کے لئے
بیلنس چیک کریں ، عمل کریں:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

یا ، صرف نئے اثاثہ کو نشانہ بنانا:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. توازن کا ایک حصہ کسی دوسرے اکاؤنٹ میں منتقل کریں

آپریٹر کے اکاؤنٹ سے 50 یونٹ کو `$RECEIVER_ACCOUNT` میں منتقل کریں:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

`$TRANSFER_HASH` کے بطور ٹرانزیکشن ہیش کو محفوظ کریں۔ دونوں میں ہولڈنگز چیک کریں
نئے بیلنس کی تصدیق کے لئے اکاؤنٹس:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. لیجر شواہد کی تصدیق کریں

اس بات کی تصدیق کے لئے محفوظ شدہ ہیشوں کا استعمال کریں کہ دونوں لین دین کی تصدیق ہوگئی ہے:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

آپ حالیہ بلاکس کو بھی اسٹریم کرسکتے ہیں کہ یہ دیکھنے کے لئے کہ منتقلی کو کس بلاک میں شامل کیا گیا ہے:

```sh
# Stream desde el ultimo bloque y detente despues de ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```مذکورہ بالا ہر کمانڈ SDKs کی طرح ایک ہی Norito پے لوڈ کا استعمال کرتی ہے۔ اگر آپ جواب دیتے ہیں
کوڈ کے ذریعے یہ بہاؤ (نیچے SDK کوئیک اسٹارٹ دیکھیں) ، ہیش اور بیلنس
جب تک آپ ایک ہی نیٹ ورک اور ڈیفالٹس کی طرف اشارہ کریں گے تب تک وہ میچ کریں گے۔

## SDK کے ساتھ برابری پابندیاں

-۔
  لین دین بھیجیں اور مورچا سے حیثیت چیک کریں۔
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (../sdks/python) - وہی رجسٹر/ٹکسال آپریشنز دکھاتا ہے
  Norito کے تعاون سے JSON مددگاروں کے ساتھ۔
-.
  گورننس مددگار اور ٹائپ شدہ استفسار ریپرس۔

پہلے CLI واک تھرو چلائیں ، پھر اپنے SDK کے ساتھ منظر نامے کو دہرائیں
اس بات کو یقینی بنانے کے لئے ترجیح دی کہ دونوں سطحیں لین دین کے ہیشوں پر متفق ہوں ،
توازن اور مشاورت کے نتائج۔