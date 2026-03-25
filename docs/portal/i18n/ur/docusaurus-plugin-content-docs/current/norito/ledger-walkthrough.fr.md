---
lang: ur
direction: rtl
source: docs/portal/docs/norito/ledger-walkthrough.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: رجسٹری کو براؤز کریں
تفصیل: ایک تعی .ن شدہ رجسٹر -> ٹکسال -> CLI `iroha` کے ساتھ منتقلی کے بہاؤ کو دوبارہ پیش کریں اور نتیجے میں ہونے والے لیجر کی حالت کو چیک کریں۔
سلگ: /نوریٹو /لیجر واک تھرو
---

اس کورس میں [کوئیک اسٹارٹ Norito] (./quickstart.md) کو CLI `iroha` کے ساتھ لیجر اسٹیٹ میں ترمیم اور معائنہ کرنے کا طریقہ ظاہر کرکے پورا کیا گیا ہے۔ آپ ایک نئی اثاثہ تعریف ، ٹکسال یونٹ کو پہلے سے طے شدہ آپریٹر اکاؤنٹ میں ریکارڈ کریں گے ، بیلنس کا کچھ حصہ کسی دوسرے اکاؤنٹ میں منتقل کریں گے اور اس کے نتیجے میں لین دین اور کریڈٹ کی تصدیق کریں گے۔ ہر قدم CLI اور SDK سلوک کے مابین برابری کی تصدیق کے لئے زنگ/ازگر/جاوا اسکرپٹ SDK کوئیک اسٹارٹ کے ذریعہ ڈھکے ہوئے بہاؤ کی عکاسی کرتا ہے۔

## شرائط

- سنگل پیر نیٹ ورک کے ذریعے شروع کرنے کے لئے [کوئیک اسٹارٹ] (./quickstart.md) کی پیروی کریں
  `docker compose -f defaults/docker-compose.single.yml up --build`۔
- یقینی بنائیں کہ `iroha` (CLI) بنایا گیا ہے یا ڈاؤن لوڈ کیا گیا ہے اور آپ کر سکتے ہیں
  `defaults/client.toml` کے ساتھ ہم مرتبہ میں شامل ہوں۔
- اختیاری ٹولز: `jq` (JSON ردعمل کو فارمیٹ کرنا) اور ایک POSIX شیل
  ماحولیاتی متغیر ٹکڑوں کے نیچے۔

گائیڈ کے دوران ، `$ADMIN_ACCOUNT` اور `$RECEIVER_ACCOUNT` کو تبدیل کریں
اکاؤنٹ IDs جن کا آپ استعمال کرنے کا ارادہ رکھتے ہیں۔ پہلے سے طے شدہ بنڈل میں پہلے ہی دو اکاؤنٹس شامل ہیں
ڈیمو کیز سے:

```sh
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

پہلے اکاؤنٹس کی فہرست دے کر اقدار کی تصدیق کریں:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. پیدائش کی حالت کا معائنہ کریں

CLI کے توسط سے ہدف لیجر کی تلاش کرکے شروع کریں:

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

یہ کمانڈ Norito جوابات پر انحصار کرتے ہیں ، لہذا فلٹرنگ اور پیجنگ ہیں
 DETINSITIST اور SDKs کے وصول کردہ سے مطابقت رکھتا ہے۔

## 2. اثاثہ کی تعریف کو محفوظ کریں

ڈومین میں `coffee` نامی ایک نیا لامحدود منیبل اثاثہ بنائیں
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI جمع کروائے گئے ٹرانزیکشن ہیش کو دکھاتا ہے (مثال کے طور پر ، `0x5f...`)۔ اسے رکھو
بعد میں حیثیت دیکھنے کے لئے.

## 3. آپریٹر اکاؤنٹ پر منٹر یونٹ

اثاثوں کی مقدار جوڑی `(asset definition, account)` کے تحت رہتی ہے۔ منٹیز 250
`$ADMIN_ACCOUNT` میں `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` کی اکائیاں:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

ایک بار پھر ، CLI آؤٹ پٹ سے ٹرانزیکشن ہیش (`$MINT_HASH`) حاصل کریں۔ کے لئے
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

آپریٹر اکاؤنٹ سے 50 یونٹوں کو `$RECEIVER_ACCOUNT` میں منتقل کریں:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

`$TRANSFER_HASH` کے بطور ٹرانزیکشن ہیش کو محفوظ کریں۔ دونوں اکاؤنٹس کے اثاثوں سے استفسار کریں
نئے بیلنس کی جانچ پڑتال کے لئے:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. لیجر شواہد کی جانچ کریں

محفوظ شدہ ہیشوں کا استعمال اس بات کی تصدیق کے لئے کریں کہ دونوں لین دین کا ارتکاب کیا گیا ہے:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

آپ حالیہ بلاکس کو بھی اسٹریم کرسکتے ہیں کہ یہ دیکھنے کے لئے کہ کس بلاک میں منتقلی شامل ہے:

```sh
# Stream depuis le dernier bloc et arretez apres ~5 secondes
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```مذکورہ بالا تمام احکامات SDKs کی طرح ایک ہی Norito پے لوڈ کا استعمال کرتے ہیں۔ اگر آپ
اس بہاؤ کو کوڈ کے ذریعے دوبارہ پیش کریں (نیچے SDK کوئیک اسٹارٹ دیکھیں) ، ہیش اور
جب تک آپ ایک ہی نیٹ ورک اور ایک ہی ڈیفالٹس کو نشانہ بناتے ہیں اس وقت تک توازن منسلک ہوجائے گا۔

## SDK برابری کے لنکس

-۔
  مورچا سے لین دین کو جمع کرانے اور حیثیت کی پولنگ۔
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (../sdks/python) - وہی رجسٹر/ٹکسال آپریشنز دکھاتا ہے
  Norito کے تعاون سے JSON مددگاروں کے ساتھ۔
-.
  گورننس مددگار اور ٹائپ شدہ استفسار ریپرس۔

پہلے CLI واک تھرو چلائیں ، پھر اپنے SDK کے ساتھ منظر نامے کو دہرائیں
اس بات کو یقینی بنانے کے لئے ترجیح دی کہ دونوں سطحیں ہیشوں سے مماثل ہیں
لین دین ، ​​بیلنس اور استفسار کے نتائج۔