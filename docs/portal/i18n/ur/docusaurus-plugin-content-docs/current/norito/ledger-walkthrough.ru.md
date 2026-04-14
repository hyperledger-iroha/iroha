---
lang: ur
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: رجسٹری کا مرحلہ وار تجزیہ
تفصیل: ڈٹرمینسٹک رجسٹر کو دوبارہ چلائیں -> ٹکسال -> CLI `iroha` سے منتقلی کا بہاؤ اور اس کے نتیجے میں رجسٹری ریاست کو چیک کریں۔
سلگ: /نوریٹو /لیجر واک تھرو
---

یہ واک تھرو [Norito quickstart] (./quickstart.md) کو Norito CLI کا استعمال کرتے ہوئے رجسٹری کی حیثیت کو کس طرح تبدیل کرنے اور چیک کرنے کا طریقہ دکھا کر ہے۔ آپ ایک نئی اثاثہ تعریف ، ڈیفالٹ آپریٹر اکاؤنٹ میں جمع یونٹوں کو جمع کریں گے ، توازن کا کچھ حصہ دوسرے اکاؤنٹ میں منتقل کریں گے اور اس کے نتیجے میں لین دین اور ہولڈنگ کی تصدیق کریں گے۔ ہر قدم زنگ/ازگر/جاوا اسکرپٹ ایس ڈی کے کوئیک اسٹارٹ میں ڈھکے ہوئے بہاؤ کو آئینہ دار کرتا ہے تاکہ آپ سی ایل آئی اور ایس ڈی کے سلوک کے مابین برابری کی تصدیق کرسکیں۔

## تقاضے

- ایک سنگل نوڈ نیٹ ورک کے ذریعے شروع کرنے کے لئے [کوئیک اسٹارٹ] (./quickstart.md) پر عمل کریں
  `docker compose -f defaults/docker-compose.single.yml up --build`۔
- اس بات کو یقینی بنائیں کہ `iroha` (CLI) بنایا گیا ہے یا ڈاؤن لوڈ کیا گیا ہے اور آپ پہنچ سکتے ہیں
  `defaults/client.toml` کے ذریعے ہم مرتبہ۔
- اختیاری مددگار: `jq` (JSON جوابات کی شکل دینا) اور POSIX شیل
  ذیل میں ماحولیاتی متغیر کے ساتھ ٹکڑوں۔

ہدایات کے دوران ، `$ADMIN_ACCOUNT` اور `$RECEIVER_ACCOUNT` کو اپنی ضرورت کے ساتھ تبدیل کریں
اکاؤنٹ IDs. پہلے سے طے شدہ بنڈل میں پہلے ہی ڈیمو کیز سے دو اکاؤنٹس حاصل کیے گئے ہیں:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

پہلے اکاؤنٹس کی نمائش کرکے اقدار کی تصدیق کریں:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. پیدائش کی حالت کا معائنہ کریں

رجسٹری کی جانچ کرکے شروع کریں جس کا سی ایل آئی اہداف ہے:

```sh
# Domains, зарегистрированные в genesis
iroha --config defaults/client.toml domain list all --table

# Accounts внутри wonderland (увеличьте --limit при необходимости)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland.universal"}' \
  --limit 10 --table

# Asset definitions, которые уже существуют
iroha --config defaults/client.toml asset definition list all --table
```

یہ کمانڈ Norito جوابات پر انحصار کرتے ہیں ، لہذا فلٹرنگ اور صفحہ بندی
ڈٹرمینسٹک اور وہی جو SDK حاصل کرتا ہے۔

## 2. اثاثہ کی تعریف کو رجسٹر کریں

ڈومین `wonderland` میں ایک نیا لامحدود ٹکسال اثاثہ `coffee` بنائیں:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI جمع کروائے گئے ٹرانزیکشن (مثال کے طور پر ، `0x5f…`) کے ہیش کو آؤٹ پٹ کرے گا۔ اس کے لئے محفوظ کریں
بعد میں حیثیت کو چیک کریں۔

## 3. اپنے آپریٹر اکاؤنٹ میں یونٹ شامل کریں

اثاثہ کی رقم جوڑی `(asset definition, account)` کے تحت رہتی ہے۔ 250 پکڑو
یونٹ `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` سے `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

ایک بار پھر ، ٹرانزیکشن ہیش (`$MINT_HASH`) کو CLI آؤٹ پٹ سے محفوظ کریں۔ اپنا توازن چیک کرنے کے لئے ،
کرو:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

یا ، صرف نیا اثاثہ حاصل کرنے کے لئے:

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

`$TRANSFER_HASH` کے بطور ٹرانزیکشن ہیش کو محفوظ کریں۔ دونوں اکاؤنٹس پر ہولڈنگ کی درخواست کریں ،
نئے بیلنس کی جانچ پڑتال کے لئے:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. رجسٹری کے ثبوت چیک کریں

دونوں لین دین کا ارتکاب کرنے کے لئے ذخیرہ شدہ ہیشوں کا استعمال کریں:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

آپ یہ دیکھنے کے لئے جدید بلاکس کو بھی اسٹریم کرسکتے ہیں کہ کون سا بلاک فعال ترجمہ:

```sh
# Стрим от последнего блока и остановка через ~5 секунд
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

مذکورہ بالا تمام کمانڈز SDK کی طرح ایک جیسے Norito پے لوڈ کا استعمال کرتے ہیں۔ اگر آپ دہراتے ہیں
کوڈ میں یہ بہاؤ (نیچے کوئیک اسٹارٹ ایس ڈی کے دیکھیں) ، ہیش اور بیلنس فراہم کردہ مماثل ہوں گے
کہ آپ ایک ہی نیٹ ورک اور ایک ہی ڈیفالٹس کو نشانہ بنا رہے ہیں۔

## SDK برابری کے لنکس-۔
  مورچا سے لین دین اور پولنگ کی حیثیت بھیجنا۔
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (../sdks/python) - وہی رجسٹر/ٹکسال آپریشنز دکھاتا ہے
  Norito کی حمایت یافتہ JSON مددگاروں کے ساتھ۔
-.
  گورننس مددگار اور ٹائپ شدہ استفسار ریپرس۔

پہلے CLI میں واک تھرو کریں ، پھر اسکرپٹ کو اپنے پسندیدہ SDK کے ساتھ دہرائیں ،
اس بات کو یقینی بنانے کے لئے کہ دونوں سطحیں ٹرانزیکشن ہیشوں ، بیلنس اور پر متفق ہیں
استفسار کے نتائج۔