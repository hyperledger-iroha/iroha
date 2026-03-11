---
lang: ur
direction: rtl
source: docs/portal/docs/norito/getting-started.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito کے ساتھ شروعات کرنا

یہ فوری ٹیوٹوریل Kotodama معاہدے کو مرتب کرنے کے لئے ایک کم سے کم ورک فلو دکھاتا ہے ، جس میں پیدا شدہ Norito بائیکوڈ کی توثیق کرتے ہیں ، اسے مقامی طور پر چلاتے ہیں ، اور Iroha نوڈ میں تعینات کرتے ہیں۔

## تقاضے

1. مورچا ٹولچین (1.76 یا بعد میں) انسٹال کریں اور اس ذخیرے کو کلون کریں۔
2. معاون بائنریوں کی تعمیر یا ڈاؤن لوڈ کریں:
   - `koto_compile` - Kotodama مرتب ، جو بائیکوڈ IVM/Norito تیار کرتا ہے
   - `ivm_run` اور `ivm_tool` - مقامی لانچ اور معائنہ کی افادیت
   - `iroha_cli` - Torii کے ذریعے معاہدوں کی تعیناتی کے لئے استعمال کیا جاتا ہے

   ریپوزٹری کا میک فائل ان بائنریوں کی توقع کرتا ہے کہ `PATH` پر۔ آپ ریڈی میڈ نمونے کو ڈاؤن لوڈ کرسکتے ہیں یا ذرائع سے ان کی تعمیر کرسکتے ہیں۔ اگر آپ مقامی طور پر ٹولچین مرتب کررہے ہیں تو ، مددگاروں کو بائنریوں کے راستے پر میک فائل فراہم کریں:

   ```sh
   KOTO=./target/debug/koto_compile Kotodama=./target/debug/ivm_run make examples-run
   ```

3. اس بات کو یقینی بنائیں کہ تعیناتی مرحلے کے وقت نوڈ Iroha چل رہا ہے۔ ذیل میں دی گئی مثالوں میں یہ فرض کیا گیا ہے کہ Torii پروفائل `iroha_cli` (`~/.config/iroha/cli.toml`) سے URL سے قابل رسائی ہے۔

## 1. مرتب معاہدہ Kotodama

ریپوزٹری کا `examples/hello/hello.ko` پر کم سے کم "ہیلو ورلڈ" معاہدہ ہے۔ اس کو بائیکوڈ Norito/IVM (`.to`) پر مرتب کریں:

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

کلیدی جھنڈے:

- `--abi 1` معاہدہ ABI ورژن 1 (تحریر کے وقت واحد معاون ورژن) سے معاہدہ کرتا ہے۔
- `--max-cycles 0` لامحدود عملدرآمد کی درخواست کرتا ہے۔ صفر اور علم کے ثبوتوں کے ل lo لوپ پیڈنگ کو محدود کرنے کے لئے ایک مثبت نمبر پر سیٹ کریں۔

## 2. نمونہ Norito (اختیاری) چیک کریں

عنوان چیک کرنے کے لئے `ivm_tool` استعمال کریں اور ایمبیڈڈ میٹا ڈیٹا:

```sh
ivm_tool inspect target/examples/hello.to
```

آپ کو ABI ورژن ، فعال جھنڈے ، اور برآمد شدہ انٹری پوائنٹ دیکھیں گے۔ تعیناتی سے پہلے یہ ایک تیز سنجیدہ چیک ہے۔

## 3. مقامی طور پر معاہدہ چلائیں

نوڈ کو کال کیے بغیر سلوک کی تصدیق کے لئے `ivm_run` کے ذریعے بائیک کوڈ چلائیں:

```sh
ivm_run target/examples/hello.to --args '{}'
```

مثال `hello` لاگ کو ایک سلام لکھتا ہے اور سیسکل `SET_ACCOUNT_DETAIL` کو کال کرتا ہے۔ آن چین کو شائع کرنے سے پہلے معاہدہ منطق کی تکرار کرتے وقت مقامی طور پر چلنا مفید ہے۔

## 4. `iroha_cli` کے ذریعے تعینات کریں

جب آپ معاہدے سے مطمئن ہوں تو ، اسے CLI کے ذریعے نوڈ پر تعینات کریں۔ اتھارٹی اکاؤنٹ ، اس کی دستخطی کلید اور یا تو `.to` فائل یا بیس 64 پے لوڈ کی وضاحت کریں:

```sh
iroha_cli app contracts deploy \
  --authority i105... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

کمانڈ Torii کے ذریعے مینی فیسٹ بنڈل Norito + بائیک کوڈ بھیجتا ہے اور ٹرانزیکشن کی حیثیت کو پرنٹ کرتا ہے۔ ارتکاب کرنے کے بعد ، جواب میں دکھائے جانے والے کوڈ ہیش کو ظاہر یا مثالوں کی فہرست حاصل کرنے کے لئے استعمال کیا جاسکتا ہے۔

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii کے ذریعے لانچ کریں

ایک بار بائیک کوڈ رجسٹر ہونے کے بعد ، آپ اسے ایک ایسی ہدایت بھیج کر کال کرسکتے ہیں جو ذخیرہ شدہ کوڈ کا حوالہ دیتا ہے (مثال کے طور پر ، `iroha_cli ledger transaction submit` یا آپ کے ایپلی کیشن کلائنٹ کے ذریعے)۔ اس بات کو یقینی بنائیں کہ اکاؤنٹ کی اجازت مطلوبہ سیسکلز (`set_account_detail` ، `transfer_asset` ، وغیرہ) کی اجازت دیتی ہے۔

## اشارے اور خرابیوں کا سراغ لگانا- ایک رن میں مثالوں کی تعمیر اور چلانے کے لئے `make examples-run` استعمال کریں۔ ماحولیاتی متغیرات کو اوور رائڈ `KOTO`/`IVM` اگر بائنری `PATH` میں نہیں ہیں۔
- اگر `koto_compile` ABI ورژن کو مسترد کرتا ہے تو ، چیک کریں کہ مرتب اور نوڈ ABI V1 کو نشانہ بنا رہے ہیں (`koto_compile --abi` کو تعاون دیکھنے کے لئے کوئی دلائل کے بغیر چلائیں)۔
- سی ایل آئی نے ہیکس یا بیس 64 میں دستخط کرنے والی چابیاں قبول کیں۔ ٹیسٹوں کے ل you ، آپ `iroha_cli tools crypto keypair` کے ذریعہ جاری کردہ چابیاں استعمال کرسکتے ہیں۔
- جب Norito پے لوڈ کو ڈیبگ کرتے ہو تو ، `ivm_tool disassemble` کمانڈ مفید ہے ، جو Kotodama ذرائع سے ہدایات کا موازنہ کرنے میں مدد کرتا ہے۔

یہ بہاؤ CI اور انضمام ٹیسٹ میں استعمال ہونے والے اقدامات کی عکاسی کرتا ہے۔ Kotodama گرائمر ، سیسکلز میپنگ ، اور Norito انٹرنلز میں گہری ڈوبکی کے ل see ، دیکھیں:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`