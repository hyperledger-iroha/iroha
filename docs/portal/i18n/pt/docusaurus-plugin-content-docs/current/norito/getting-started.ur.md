---
lang: pt
direction: ltr
source: docs/portal/docs/norito/getting-started.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito کا آغاز

O código de byte Kotodama é o código de bytecode Norito. کرنے, اسے مقامی طور پر چلانے اور Iroha نوڈ پر ڈپلائے کرنے کے کم سے کم ورک فلو کو دکھاتا ہے۔

## ضروریات

1. Cadeia de ferramentas de ferrugem (1,76 یا جدید) انسٹال کریں اور اس ریپو کو چیک آؤٹ کریں۔
2. O que você precisa fazer:
   - `koto_compile` - Kotodama کمپائلر ou IVM/Norito bytecode خارج کرتا ہے
   - `ivm_run` ou `ivm_tool` - مقامی اجرا اور معائنہ یوٹیلٹیز
   - `iroha_cli` - Torii کے ذریعے کنٹریکٹ ڈپلائے کرنے کے لئے استعمال ہوتا ہے

   ریپو کا Makefile ان بائنریز کو `PATH` میں توقع کرتا ہے۔ آپ پری بلٹ artefatos ڈاؤن لوڈ کر سکتے ہیں یا سورس سے بنا سکتے ہیں۔ O conjunto de ferramentas que você possui é o conjunto de ferramentas do Makefile helpers e os ajudantes do Makefile são os seguintes:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Você pode usar o cartão de crédito Iroha para usar o cartão Iroha. نیچے کی مثالیں فرض کرتی ہیں کہ Torii اس URL پر قابل رسائی ہے جو آپ کے `iroha_cli` پروفائل (`~/.config/iroha/cli.toml`) میں سیٹ ہے۔

## 1. Kotodama کنٹریکٹ کمپائل کریں

ریپو میں کم سے کم "olá mundo" کنٹریکٹ `examples/hello/hello.ko` میں موجود ہے۔ O bytecode Norito/IVM (`.to`) é o código do código:

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

O que fazer:

- `--abi 1` کنٹریکٹ کو ABI ورژن 1 پر لاک کرتا ہے (تحریر کے وقت واحد سپورٹڈ ورژن).
- `--max-cycles 0` لامحدود اجرا کی درخواست کرتا ہے؛ provas de conhecimento zero کے لئے preenchimento de ciclo کو محدود کرنے کے لئے مثبت عدد دیں۔

## 2. Norito آرٹیفیکٹ کا معائنہ (اختیاری)

ہیڈر اور شامل میٹا ڈیٹا کی پڑتال کے لئے `ivm_tool` استعمال کریں:

```sh
ivm_tool inspect target/examples/hello.to
```

آپ کو ABI ورژن, فعال flags اور ایکسپورٹڈ pontos de entrada نظر آئیں گے۔ یہ ڈپلائمنٹ سے پہلے ایک فوری verificação de sanidade ہے۔

## 3. کنٹریکٹ کو مقامی طور پر چلائیں

`ivm_run` کے ساتھ bytecode چلائیں تاکہ نوڈ کو چھیڑے بغیر برتاؤ کی تصدیق ہو:

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` é um sistema de chamada de sistema para `SET_ACCOUNT_DETAIL` syscall جاری کرتی ہے۔ مقامی اجرا اس وقت مفید ہے جب آپ on-chain شائع کرنے سے پہلے کنٹریکٹ لاجک پر تکرار کر رہے ہوں۔

## 4. `iroha_cli` کے ذریعے ڈپلائے کریں

Você pode usar o CLI کے ذریعے اسے نوڈ پر ڈپلائے کریں۔ A autoridade اکاؤنٹ، اس کی chave de assinatura, اور `.to` é a carga útil Base64 فراہم کریں:

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

یہ کمانڈ Torii کے ذریعے Norito manifesto + pacote de bytecode جمع کرتی ہے اور نتیجے میں ٹرانزیکشن کی حیثیت پرنٹ کرتی ہے۔ ٹرانزیکشن commit ہونے کے بعد, جواب میں دکھایا گیا código hash manifestos حاصل کرنے یا instâncias لسٹ کرنے کے لئے Como fazer isso:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii کے خلاف چلائیں

bytecode رجسٹر ہونے کے بعد، آپ ایک envio de instrução کر کے اسے کال کر سکتے ہیں جو محفوظ شدہ کوڈ کو Isso é (como `iroha_cli ledger transaction submit`). یقینی بنائیں کہ اکاؤنٹ permissões para syscalls (`set_account_detail`, `transfer_asset`, وغیرہ) کی اجازت دیتے ہیں۔

## تجاویز اور خرابیوں کا حل- `make examples-run` استعمال کریں تاکہ فراہم کردہ مثالیں ایک ہی قدم میں کمپائل اور چل سکیں۔ Você pode usar `PATH` como variável de ambiente `PATH`/`IVM` e substituir variáveis ​​de ambiente
- اگر `koto_compile` ABI ورژن مسترد کرے تو تصدیق کریں کہ کمپائلر اور نوڈ دونوں ABI v1 کو ہدف Isso é um problema (`koto_compile --abi` بغیر دلائل کے چلائیں تاکہ سپورٹ دکھے).
- CLI hex یا Chaves de assinatura Base64 قبول کرتا ہے۔ ٹیسٹنگ کے لئے `iroha_cli tools crypto keypair` سے نکلے ہوئے chaves استعمال کیے جا سکتے ہیں۔
- Cargas úteis Norito Kotodama سورس سے جوڑا جا سکے۔

یہ فلو CI اور انٹیگریشن ٹیسٹس میں استعمال ہونے والے مراحل کی عکاسی کرتا ہے۔ Kotodama گرامر, mapeamentos de syscall e Norito internals کی مزید تفصیل کے لئے دیکھیں:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`