---
lang: ur
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: ریکارڈ کا دورہ
تفصیل: ایک لازمی بہاؤ رجسٹر کو دوبارہ پیش کریں -> ٹکسال -> CLI `iroha` کا استعمال کرتے ہوئے منتقلی اور اس کے نتیجے میں رجسٹر کی حیثیت کو چیک کریں۔
سلگ: /نوریٹو /لیجر واک تھرو
---

یہ واک تھرو [Norito کوئیک اسٹارٹ] (./quickstart.md) کو CLI `iroha` کا استعمال کرتے ہوئے رجسٹری ریاست میں ترمیم اور معائنہ کرنے کا طریقہ دکھا کر مکمل کرتا ہے۔ آپ ورچوئل آپریٹر اکاؤنٹ میں ایک نئی اثاثہ تعریف ، ٹکسال یونٹوں کو رجسٹر کریں گے ، بیلنس کا کچھ حصہ دوسرے اکاؤنٹ میں منتقل کریں گے ، اور اس کے نتیجے میں لین دین اور ہولڈنگ کی تصدیق کریں گے۔ ہر قدم زنگ/ازگر/جاوا اسکرپٹ کوئیک اسٹارٹ میں ڈھکے ہوئے بہاؤ کو آئینہ دیتا ہے تاکہ آپ سی ایل آئی اور ایس ڈی کے سلوک کے مابین خط و کتابت کی تصدیق کرسکیں۔

## شرائط

- ایک سنگل نوڈ نیٹ ورک کے ذریعے چلانے کے لئے [کوئیک اسٹارٹ] (./quickstart.md) پر عمل کریں
  `docker compose -f defaults/docker-compose.single.yml up --build`۔
- اس بات کو یقینی بنائیں کہ `iroha` (CLI) بنایا گیا ہے یا بھری ہوئی ہے اور یہ کہ آپ `defaults/client.toml` کا استعمال کرتے ہوئے ہم مرتبہ تک رسائی حاصل کرسکتے ہیں۔
- اختیاری ٹولز: `jq` (JSON رسپانس فارمیٹ) اور نچلے حصے میں ماحولیاتی متغیر حصوں کے لئے ایک POSIX شیل۔

گائیڈ کے دوران ، `$ADMIN_ACCOUNT` اور `$RECEIVER_ACCOUNT` کو اکاؤنٹ IDs کے ساتھ تبدیل کریں جس کا آپ استعمال کرنے کا ارادہ رکھتے ہیں۔ پہلے سے طے شدہ بنڈل میں پہلے ہی ڈسپلے کی چابیاں سے اخذ کردہ دو حساب کتاب شامل ہیں:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

پہلے حساب کتاب کی فہرست دے کر اقدار کی تصدیق کریں:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. پیدائش کی حیثیت چیک کریں

سی ایل آئی کے اہداف کو کس رجسٹری کے اہداف کی کھوج سے شروع کریں:

```sh
# Domains المسجلة في genesis
iroha --config defaults/client.toml domain list all --table

# Accounts داخل wonderland (استبدل --limit بعدد اكبر عند الحاجة)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions الموجودة مسبقا
iroha --config defaults/client.toml asset definition list all --table
```

یہ احکامات Norito کے ذریعہ تعاون یافتہ ردعمل پر انحصار کرتے ہیں ، لہذا فلٹرنگ اور پارٹیشننگ ایک متنازعہ اور ایک جیسی ہیں جو SDKs وصول کرتی ہے۔

## 2. اثاثہ کی تعریف کو رجسٹر کریں

`coffee` کے نام سے ایک نیا لامحدود ٹکسال والا اثاثہ بنائیں `wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

سی ایل آئی جمع کروائی گئی ٹرانزیکشن ہیش (جیسے `0x5f…`) پرنٹ کرتی ہے۔ بعد میں حیثیت کے بارے میں پوچھ گچھ کرنے کے لئے اسے محفوظ کریں۔

## 3. آپریٹر کے اکاؤنٹ میں ٹکسال یونٹ

اثاثوں کی مقدار جوڑی `(asset definition, account)` کے تحت ہے۔ `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` کے 250 یونٹوں سے `$ADMIN_ACCOUNT` میں پوچھیں:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

ایک بار پھر ٹرانزیکشن ہیش (`$MINT_HASH`) کو CLI آؤٹ پٹ سے محفوظ کریں۔ توازن کی جانچ پڑتال کے لئے:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

یا صرف نئے اثاثہ کو نشانہ بنانا:

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

`$TRANSFER_HASH` کے بطور ٹرانزیکشن ہیش کو محفوظ کریں۔ نئے بیلنس کی تصدیق کے لئے دو اکاؤنٹس میں ہولڈنگز کے بارے میں پوچھ گچھ کریں:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. لاگ ثبوت کی تصدیق کریں

اس بات کی تصدیق کے لئے محفوظ شدہ ہیشوں کا استعمال کریں کہ دونوں لین دین کا ارتکاب کیا گیا تھا:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

آپ حالیہ بلاکس کو یہ بھی نشر کرسکتے ہیں کہ کس بلاک میں تبدیلی شامل ہے:

```sh
# Stream من اخر كتلة والتوقف بعد ~5 ثوان
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

مذکورہ بالا تمام احکامات SDKs کی طرح Norito پے لوڈ کا استعمال کرتے ہیں۔ اگر آپ اس بہاؤ کو کوڈ کے ذریعے دہراتے ہیں (نیچے ایس ڈی کے کے لئے کوئیک اسٹارٹ ملاحظہ کریں) ، ہیش اور بیلنس اس وقت تک مماثل ہوں گے جب تک کہ وہ ایک ہی نیٹ ورک کو نشانہ بنائیں اور وہی مفروضے بنائیں۔

## SDK برابری کے لنکس

-۔
۔
۔

پہلے CLI واک تھرو کو چلائیں اور پھر اپنے ترجیحی SDK کا استعمال کرتے ہوئے منظر نامے کو دہرائیں تاکہ یہ یقینی بنایا جاسکے کہ دونوں سطحیں ٹرانزیکشن ہیشوں ، توازن اور استفسار کے نتائج سے مماثل ہیں۔