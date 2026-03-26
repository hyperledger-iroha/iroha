---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f5fc3fbeb5265e7be7bde13a658c662706e6d4c30d4385675eda93892513de3
source_last_modified: "2026-01-22T15:55:00+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ur
direction: rtl
source: docs/portal/docs/norito/getting-started.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Norito کا آغاز

یہ مختصر رہنما Kotodama کنٹریکٹ کو کمپائل کرنے، بنے ہوئے Norito bytecode کا معائنہ کرنے، اسے مقامی طور پر چلانے اور Iroha نوڈ پر ڈپلائے کرنے کے کم سے کم ورک فلو کو دکھاتا ہے۔

## ضروریات

1. Rust toolchain (1.76 یا جدید) انسٹال کریں اور اس ریپو کو چیک آؤٹ کریں۔
2. معاون بائنریز کو بنائیں یا ڈاؤن لوڈ کریں:
   - `koto_compile` - Kotodama کمپائلر جو IVM/Norito bytecode خارج کرتا ہے
   - `ivm_run` اور `ivm_tool` - مقامی اجرا اور معائنہ یوٹیلٹیز
   - `iroha_cli` - Torii کے ذریعے کنٹریکٹ ڈپلائے کرنے کے لئے استعمال ہوتا ہے

   ریپو کا Makefile ان بائنریز کو `PATH` میں توقع کرتا ہے۔ آپ پری بلٹ artifacts ڈاؤن لوڈ کر سکتے ہیں یا سورس سے بنا سکتے ہیں۔ اگر آپ toolchain کو مقامی طور پر کمپائل کریں تو Makefile helpers کو بائنریز کی طرف پوائنٹ کریں:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. جب آپ ڈپلائمنٹ مرحلے تک پہنچیں تو یقینی بنائیں کہ Iroha نوڈ چل رہا ہو۔ نیچے کی مثالیں فرض کرتی ہیں کہ Torii اس URL پر قابل رسائی ہے جو آپ کے `iroha_cli` پروفائل (`~/.config/iroha/cli.toml`) میں سیٹ ہے۔

## 1. Kotodama کنٹریکٹ کمپائل کریں

ریپو میں کم سے کم "hello world" کنٹریکٹ `examples/hello/hello.ko` میں موجود ہے۔ اسے Norito/IVM bytecode (`.to`) میں کمپائل کریں:

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

اہم فلیگز:

- `--abi 1` کنٹریکٹ کو ABI ورژن 1 پر لاک کرتا ہے (تحریر کے وقت واحد سپورٹڈ ورژن).
- `--max-cycles 0` لامحدود اجرا کی درخواست کرتا ہے؛ zero-knowledge proofs کے لئے cycle padding کو محدود کرنے کے لئے مثبت عدد دیں۔

## 2. Norito آرٹیفیکٹ کا معائنہ (اختیاری)

ہیڈر اور شامل میٹا ڈیٹا کی پڑتال کے لئے `ivm_tool` استعمال کریں:

```sh
ivm_tool inspect target/examples/hello.to
```

آپ کو ABI ورژن، فعال flags اور ایکسپورٹڈ entry points نظر آئیں گے۔ یہ ڈپلائمنٹ سے پہلے ایک فوری sanity check ہے۔

## 3. کنٹریکٹ کو مقامی طور پر چلائیں

`ivm_run` کے ساتھ bytecode چلائیں تاکہ نوڈ کو چھیڑے بغیر برتاؤ کی تصدیق ہو:

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` مثال ایک سلام لاگ کرتی ہے اور `SET_ACCOUNT_DETAIL` syscall جاری کرتی ہے۔ مقامی اجرا اس وقت مفید ہے جب آپ on-chain شائع کرنے سے پہلے کنٹریکٹ لاجک پر تکرار کر رہے ہوں۔

## 4. `iroha_cli` کے ذریعے ڈپلائے کریں

جب آپ کنٹریکٹ سے مطمئن ہوں تو CLI کے ذریعے اسے نوڈ پر ڈپلائے کریں۔ ایک authority اکاؤنٹ، اس کی signing key، اور `.to` فائل یا Base64 payload فراہم کریں:

```sh
iroha_cli app contracts deploy \
  --authority <i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

یہ کمانڈ Torii کے ذریعے Norito manifest + bytecode bundle جمع کرتی ہے اور نتیجے میں ٹرانزیکشن کی حیثیت پرنٹ کرتی ہے۔ ٹرانزیکشن commit ہونے کے بعد، جواب میں دکھایا گیا code hash manifests حاصل کرنے یا instances لسٹ کرنے کے لئے استعمال کیا جا سکتا ہے:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii کے خلاف چلائیں

bytecode رجسٹر ہونے کے بعد، آپ ایک instruction submit کر کے اسے کال کر سکتے ہیں جو محفوظ شدہ کوڈ کو حوالہ دے (مثلا `iroha_cli ledger transaction submit` یا آپ کے ایپ کلائنٹ کے ذریعے). یقینی بنائیں کہ اکاؤنٹ permissions مطلوبہ syscalls (`set_account_detail`, `transfer_asset`, وغیرہ) کی اجازت دیتے ہیں۔

## تجاویز اور خرابیوں کا حل

- `make examples-run` استعمال کریں تاکہ فراہم کردہ مثالیں ایک ہی قدم میں کمپائل اور چل سکیں۔ اگر بائنریز `PATH` میں نہیں ہیں تو `KOTO`/`IVM` environment variables کو override کریں۔
- اگر `koto_compile` ABI ورژن مسترد کرے تو تصدیق کریں کہ کمپائلر اور نوڈ دونوں ABI v1 کو ہدف بنا رہے ہیں (`koto_compile --abi` بغیر دلائل کے چلائیں تاکہ سپورٹ دکھے).
- CLI hex یا Base64 signing keys قبول کرتا ہے۔ ٹیسٹنگ کے لئے `iroha_cli tools crypto keypair` سے نکلے ہوئے keys استعمال کیے جا سکتے ہیں۔
- Norito payloads کی ڈیبگنگ میں `ivm_tool disassemble` سب کمانڈ مدد کرتی ہے تاکہ ہدایات کو Kotodama سورس سے جوڑا جا سکے۔

یہ فلو CI اور انٹیگریشن ٹیسٹس میں استعمال ہونے والے مراحل کی عکاسی کرتا ہے۔ Kotodama گرامر، syscall mappings اور Norito internals کی مزید تفصیل کے لئے دیکھیں:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
