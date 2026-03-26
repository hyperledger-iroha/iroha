---
lang: ur
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: لیجر واک تھرو
تفصیل: رجسٹر -> ٹکسال -> سے CLI `iroha` کے ساتھ منتقلی سے ایک تعصب کا سلسلہ چلائیں اور نتیجے میں ہونے والے لیجر کی حالت کو چیک کریں۔
سلگ: /نوریٹو /لیجر واک تھرو
---

اس واک تھرو کی تکمیل [Norito کوئیک اسٹارٹ] (./quickstart.md) یہ دکھا کر کہ کس طرح Norito CLI کے ساتھ لیجر اسٹیٹ کو تبدیل اور معائنہ کیا جائے۔ آپ ایک نئی اثاثہ تعریف ، ٹکسال یونٹ کو معیاری تاجر اکاؤنٹ میں رجسٹر کریں گے ، توازن کا کچھ حصہ دوسرے اکاؤنٹ میں منتقل کریں گے ، اور اس کے نتیجے میں لین دین اور ہولڈنگ کی تصدیق کریں گے۔ ہر قدم سی ایل آئی اور ایس ڈی کے کے مابین برابری کی تصدیق کے لئے زنگ/ازگر/جاوا اسکرپٹ ایس ڈی کے کوئیک اسٹارٹ میں ڈھکے ہوئے بہاؤ کو آئینہ دیتا ہے۔

## شرائط

- سنگل پیر نیٹ ورکنگ کے ذریعے شروع کرنے کے لئے [کوئیک اسٹارٹ] (./quickstart.md) پر عمل کریں
  `docker compose -f defaults/docker-compose.single.yml up --build`۔
- اس بات کو یقینی بنائیں کہ `iroha` (CLI) مرتب یا ڈاؤن لوڈ کیا گیا ہے اور آپ کر سکتے ہیں
  `defaults/client.toml` کا استعمال کرتے ہوئے ہم مرتبہ تک رسائی حاصل کریں۔
- اختیاری مددگار: `jq` (JSON رسپانس فارمیٹنگ) اور ایک POSIX شیل
  ماحولیاتی متغیر ٹکڑوں کو ذیل میں استعمال کیا جاتا ہے۔

گائیڈ کے دوران ، `$ADMIN_ACCOUNT` اور `$RECEIVER_ACCOUNT` کو اپنے کوڈ IDs کے ساتھ تبدیل کریں۔
اکاؤنٹ جو آپ استعمال کرنے کا ارادہ رکھتے ہیں۔ معیاری بنڈل میں پہلے ہی دو اکاؤنٹس شامل ہیں
ڈیمو کیز:

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

پہلے اکاؤنٹس کی فہرست دے کر مقدار کی تصدیق کریں:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. پیدائش کی حیثیت کا معائنہ کریں

لیجر کی تلاش کرکے شروع کریں کہ سی ایل آئی کو نشانہ بنا رہا ہے:

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

یہ احکامات Norito کے تعاون سے جوابات پر منحصر ہیں ، لہذا فلٹر اور پیجنگ تعصب پسند ہیں اور ایس ڈی کے جو کچھ وصول کرتے ہیں اس سے ملتے ہیں۔

## 2. اثاثہ کی تعریف کو رجسٹر کریں

ڈومین کے اندر `coffee` نامی ایک نیا لامحدود ٹکسال اثاثہ بنائیں
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI بھیجے گئے ٹرانزیکشن کی ہیش پرنٹ کرتا ہے (مثال کے طور پر ، `0x5f...`)۔ اس کے لئے محفوظ کریں
بعد میں حیثیت کو چیک کریں۔

## 3. آپریٹر کے اکاؤنٹ میں ٹکسال یونٹ

اثاثوں کی مقدار جوڑی `(asset definition, account)` کے تحت رہتی ہے۔ ٹکسال 250
`7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` سے `$ADMIN_ACCOUNT` تک یونٹ:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

ایک بار پھر ، CLI آؤٹ پٹ میں ٹرانزیکشن ہیش (`$MINT_HASH`) پر قبضہ کریں۔ کے لئے
توازن کی تصدیق کے لئے ، چلائیں:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

یا ، صرف نئے اثاثے کو نشانہ بنانا:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. توازن کا ایک حصہ کسی دوسرے اکاؤنٹ میں منتقل کریں

آپریٹر اکاؤنٹ سے 50 یونٹوں کو `$RECEIVER_ACCOUNT` میں منتقل کریں:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

ٹرانزیکشن ہیش کو `$TRANSFER_HASH` کے طور پر اسٹور کریں۔ دونوں میں ہولڈنگ سے مشورہ کریں
نئے بیلنس کی جانچ پڑتال کے لئے اکاؤنٹس:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. لیجر شواہد کی جانچ کریں

اس بات کی تصدیق کے لئے محفوظ شدہ ہیشوں کا استعمال کریں کہ دونوں لین دین کا ارتکاب کیا گیا تھا:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

آپ حالیہ بلاکس کو بھی اسٹریم کرسکتے ہیں کہ یہ دیکھنے کے لئے کہ کون سا بلاک شامل ہے
منتقلی:

```sh
# Stream a partir do ultimo bloco e pare apos ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

مذکورہ بالا ہر کمانڈ SDKs کی طرح ایک ہی Norito پے لوڈ کا استعمال کرتی ہے۔ اگر آپ دہراتے ہیں
یہ بہاؤ کوڈ کے ذریعے (نیچے SDK کوئیک اسٹارٹ دیکھیں) ، ہیش اور بیلنس
جب تک آپ ایک ہی نیٹ ورک اور ایک ہی ڈیفالٹس کو نشانہ بناتے ہیں اس وقت تک سیدھ میں رہیں گے۔

## SDK برابری کے لنکس-۔
  لین دین جمع کروائیں اور مورچا سے حیثیت چیک کریں۔
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (../sdks/python) - وہی رجسٹر/ٹکسال آپریشنز دکھاتا ہے
  Norito کے تعاون سے JSON مددگاروں کے ساتھ۔
-.
  گورننس مددگار اور ٹائپ شدہ استفسار ریپرس۔

پہلے CLI واک تھرو چلائیں ، پھر اپنے SDK کے ساتھ منظر نامے کو دہرائیں۔
اس بات کو یقینی بنانے کی ترجیح کہ دونوں سطحیں ہیشوں پر متفق ہوں
ٹرانزیکشن ، بیلنس اور استفسار آؤٹ پٹس۔