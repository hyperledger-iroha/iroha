---
lang: ur
direction: rtl
source: docs/portal/docs/norito/getting-started.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito کا استعمال شروع کریں

یہ فوری ہدایت نامہ Kotodama نوڈ مرتب کرنے کے لئے کم سے کم ورک فلو کو ظاہر کرتا ہے ، نتیجے میں Norito بائیکوڈ اسکین کریں ، اسے مقامی طور پر چلائیں ، اور پھر اسے Iroha نوڈ میں تعینات کریں۔

## شرائط

1. مورچا ٹولچین (1.76 یا بعد میں) انسٹال کریں اور اس ذخیرے کو کلون کریں۔
2. معاون بائنریوں کی تعمیر یا ڈاؤن لوڈ کریں:
   - `koto_compile` - Kotodama مرتب جو بائیکوڈ IVM/Norito برآمد کرتا ہے
   - `ivm_run` اور `ivm_tool` - مقامی آپریشن اور معائنہ کے اوزار
   - `iroha_cli` - Torii کے ذریعے معاہدوں کو شائع کرنے کے لئے استعمال کیا جاتا ہے

   ریپوزٹری میں میک فائل `PATH` کے تحت ان بائنریز کی توقع کرتا ہے۔ آپ ریڈی میڈ میڈیٹ فیکٹ کو ڈاؤن لوڈ کرسکتے ہیں یا انہیں ماخذ سے تعمیر کرسکتے ہیں۔ اگر آپ مقامی طور پر ٹولچین بناتے ہیں تو ، میک فائل مددگاروں میں بائنریز کی طرف اشارہ کریں:

   ```sh
   KOTO=./target/debug/koto_compile Kotodama=./target/debug/ivm_run make examples-run
   ```

3. تصدیق کریں کہ نوڈ Iroha چل رہا ہے جب تعیناتی کا مرحلہ پہنچ جاتا ہے۔ ذیل میں دی گئی مثالوں میں یہ فرض کیا گیا ہے کہ Torii پروفائل `iroha_cli` (`~/.config/iroha/cli.toml`) میں تشکیل شدہ URL پر دستیاب ہے۔

## 1. مرتب نوڈس Kotodama

ذخیرہ میں `examples/hello/hello.ko` پر ایک سادہ "ہیلو ورلڈ" معاہدہ شامل ہے۔ اس کو بائیکوڈ Norito/IVM (`.to`) پر مرتب کریں:

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

سب سے اہم میڈیا:

- `--abi 1` ABI ورژن 1 پر معاہدہ انسٹال کرتا ہے (تحریر کے وقت واحد معاون ورژن)۔
- `--max-cycles 0` لامحدود عملدرآمد کی درخواست کرتا ہے۔ صفر اور علم کے ثبوتوں کے لئے سائیکل بھرتی کی حد کے لئے ایک مثبت نمبر مرتب کریں۔

## 2. چیک ٹریس Norito (اختیاری)

ہیڈر اور ایمبیڈڈ میٹا ڈیٹا کی جانچ پڑتال کے لئے `ivm_tool` استعمال کریں:

```sh
ivm_tool inspect target/examples/hello.to
```

آپ کو ABI ورژن ، چالو جھنڈے ، اور برآمد شدہ انٹری پوائنٹس دیکھنا چاہ .۔ اشاعت سے پہلے یہ ایک فوری چیک ہے۔

## 3. مقامی طور پر نوڈس چلائیں

نوڈ کو چھوئے بغیر سلوک کی تصدیق کرنے کے لئے `ivm_run` کے ذریعے بائٹ کوڈ پر عمل کریں:

```sh
ivm_run target/examples/hello.to --args '{}'
```

مثال `hello` ایک سلام رجسٹر کرتا ہے اور سیسکل `SET_ACCOUNT_DETAIL` جاری کرتا ہے۔ چین پر شائع کرنے سے پہلے معاہدہ کی منطق کی تکرار کرتے وقت مقامی آپریشن مفید ہے۔

## 4. `iroha_cli` کے ذریعے اشاعت

جب آپ معاہدے سے مطمئن ہوں تو ، اسے CLI کا استعمال کرتے ہوئے نوڈ پر تعینات کریں۔ ایک درست اکاؤنٹ اور اس کی دستخطی کلید اور یا تو `.to` یا BASE64 فارمیٹ میں پے لوڈ فائل فراہم کریں:

```sh
iroha_cli app contracts deploy \
  --authority ih58... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

کمانڈ Torii کے ذریعے مینیفیسٹ Norito + بائیک کوڈ بھیجتا ہے اور اس کے نتیجے میں لین دین کی حیثیت کو پرنٹ کرتا ہے۔ لین دین کے ارتکاب کے بعد ، جواب میں دکھائے جانے والے ہیش کوڈ کو ظاہر یا فہرست کی فہرست کو بازیافت کرنے کے لئے استعمال کیا جاسکتا ہے۔

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii کے ذریعے بجلی

رجسٹرڈ بائیکوڈ کے ساتھ ، آپ اس ہدایت کو بھیج کر اس کی درخواست کرسکتے ہیں جو ذخیرہ شدہ کوڈ کا حوالہ دیتا ہے (جیسے `iroha_cli ledger transaction submit` یا کسی ایپلی کیشن کلائنٹ کے ذریعے)۔ اس بات کو یقینی بنائیں کہ اکاؤنٹ کی اجازت مطلوبہ سیسکلز (`set_account_detail` ، `transfer_asset` ، وغیرہ) کی اجازت دیتی ہے۔

## اشارے اور خرابیوں کا سراغ لگانا

- ایک ہی وقت میں مثالوں کو مرتب کرنے اور ان پر عمل درآمد کے لئے `make examples-run` استعمال کریں۔ ماحولیاتی متغیرات کو اوور رائڈ کریں `KOTO`/`IVM` اگر بائنری `PATH` پر سیٹ نہیں ہیں۔
- اگر `koto_compile` ABI ورژن کو مسترد کرتا ہے تو ، تصدیق کریں کہ مرتب اور نوڈ کو نشانہ بنایا جاتا ہے ABI V1 (سپورٹ کو دیکھنے کے لئے پیرامیٹرز کے بغیر `koto_compile --abi` چلائیں)۔
- سی ایل آئی ہیکس یا بیس 64 فارمیٹ میں دستخط کرنے والی چابیاں قبول کرتا ہے۔ جانچ کے ل you آپ `iroha_cli tools crypto keypair` کے ذریعہ جاری کردہ چابیاں استعمال کرسکتے ہیں۔
- جب Norito پے لوڈ کو ڈیبگ کرتے ہو تو ، `ivm_tool disassemble` کمانڈ Kotodama ماخذ کو ہدایت کو پابند کرنے میں مدد کرتا ہے۔یہ بہاؤ CI اور انضمام ٹیسٹ میں استعمال ہونے والے اقدامات کی عکاسی کرتا ہے۔ Kotodama قواعد اور Norito کے اندر بائنڈنگ SYSCALLS کے بارے میں مزید معلومات کے لئے ، دیکھیں:

- `docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`