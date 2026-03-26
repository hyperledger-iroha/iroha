---
lang: es
direction: ltr
source: docs/portal/docs/norito/getting-started.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito کا آغاز

El código de bytes Kotodama es un código de bytes Norito. کرنے، اسے مقامی طور پر چلانے اور Iroha نوڈ پر ڈپلائے کرنے کے کم سے کم ورک فلو کو دکھاتا ہے۔

## ضروریات

1. Cadena de herramientas Rust (1.76 یا جدید) انسٹال کریں اور اس ریپو کو چیک آؤٹ کریں۔
2. معاون بائنریز کو بنائیں یا ڈاؤن لوڈ کریں:
   - `koto_compile` - Kotodama Código de bytes y código de bytes IVM/Norito
   - `ivm_run` اور `ivm_tool` - مقامی اجرا اور معائنہ یوٹیلٹیز
   - `iroha_cli` - Torii کے ذریعے کنٹریکٹ ڈپلائے کرنے کے لئے استعمال ہوتا ہے

   ریپو کا Makefile ان بائنریز کو `PATH` میں توقع کرتا ہے۔ آپ پری بلٹ artefactos ڈاؤن لوڈ کر سکتے ہیں یا سورس سے بنا سکتے ہیں۔ Esta es la cadena de herramientas, los asistentes de Makefile y los asistentes de Makefile que están disponibles:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. جب آپ ڈپلائمنٹ مرحلے تک پہنچیں تو یقینی بنائیں کہ Iroha نوڈ چل رہا ہو۔ نیچے کی مثالیں فرض کرتی ہیں کہ Torii اس URL پر قابل رسائی ہے جو آپ کے `iroha_cli` پروفائل (`~/.config/iroha/cli.toml`) میں سیٹ ہے۔

## 1. Kotodama کنٹریکٹ کمپائل کریں

ریپو میں کم سے کم "hola mundo" کنٹریکٹ `examples/hello/hello.ko` میں موجود ہے۔ Código de bytes Norito/IVM (`.to`) میں کمپائل کریں:

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

اہم فلیگز:- `--abi 1` کنٹریکٹ کو ABI ورژن 1 پر لاک کرتا ہے (تحریر کے وقت واحد سپورٹڈ ورژن).
- `--max-cycles 0` لامحدود اجرا کی درخواست کرتا ہے؛ pruebas de conocimiento cero کے لئے ciclo de relleno کو محدود کرنے کے لئے مثبت عدد دیں۔

## 2. Norito آرٹیفیکٹ کا معائنہ (اختیاری)

ہیڈر اور شامل میٹا ڈیٹا کی پڑتال کے لئے `ivm_tool` استعمال کریں:

```sh
ivm_tool inspect target/examples/hello.to
```

آپ کو ABI ورژن، فعال banderas اور ایکسپورٹڈ puntos de entrada نظر آئیں گے۔ یہ ڈپلائمنٹ سے پہلے ایک فوری control de cordura ہے۔

## 3. کنٹریکٹ کو مقامی طور پر چلائیں

`ivm_run` کے ساتھ bytecode چلائیں تاکہ نوڈ کو چھیڑے بغیر برتاؤ کی تصدیق ہو:

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` مثال ایک سلام لاگ کرتی ہے اور `SET_ACCOUNT_DETAIL` syscall جاری کرتی ہے۔ مقامی اجرا اس وقت مفید ہے جب آپ on-chain شائع کرنے سے پہلے کنٹریکٹ لاجک پر تکرار کر رہے ہوں۔

## 4. `iroha_cli` کے ذریعے ڈپلائے کریں

جب آپ کنٹریکٹ سے مطمئن ہوں تو CLI کے ذریعے اسے نوڈ پر ڈپلائے کریں۔ Esta autoridad tiene una clave de firma, y `.to` tiene una carga útil Base64:

```sh
iroha_cli app contracts deploy \
  --authority soraカタカナ... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

یہ کمانڈ Torii کے ذریعے Norito manifiesto + paquete de código de bytes جمع کرتی ہے اور نتیجے میں ٹرانزیکشن کی حیثیت پرنٹ کرتی ہے۔ ٹرانزیکشن commit ہونے کے بعد، جواب میں دکھایا گیا manifiestos hash de código حاصل کرنے یا instancias لسٹ کرنے کے لئے استعمال کیا جا سکتا ہے:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii کے خلاف چلائیںbytecode رجسٹر ہونے کے بعد، آپ ایک instrucción enviar کر کے اسے کال کر سکتے ہیں جو محفوظ شدہ کوڈ کو حوالہ دے (مثلا `iroha_cli ledger transaction submit` یا آپ کے ایپ کلائنٹ کے ذریعے). Varios permisos de llamadas al sistema (`set_account_detail`, `transfer_asset`, etc.) ہیں۔

## تجاویز اور خرابیوں کا حل

- `make examples-run` Utilice `PATH` para cambiar las variables de entorno y anular variables de entorno `KOTO`/`IVM`.
- اگر `koto_compile` ABI ورژن مسترد کرے تو تصدیق کریں کہ کمپائلر اور نوڈ دونوں ABI v1 کو ہدف بنا رہے ہیں (`koto_compile --abi` بغیر دلائل کے چلائیں تاکہ سپورٹ دکھے).
- CLI hexadecimal یا Claves de firma Base64 قبول کرتا ہے۔ ٹیسٹنگ کے لئے `iroha_cli tools crypto keypair` سے نکلے ہوئے teclas استعمال کیے جا سکتے ہیں۔
- Cargas útiles Norito کی ڈیبگنگ میں `ivm_tool disassemble` سب کمانڈ مدد کرتی ہے تاکہ ہدایات کو Kotodama سورس سے جوڑا جا سکے۔

یہ فلو CI اور انٹیگریشن ٹیسٹس میں استعمال ہونے والے مراحل کی عکاسی کرتا ہے۔ Kotodama asignaciones de llamadas al sistema y componentes internos de Norito:

-`docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`