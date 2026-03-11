---
lang: ur
direction: rtl
source: docs/portal/docs/norito/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: فاسٹ اسٹارٹ Norito
تفصیل: ورژننگ ٹولز اور سنگل نوڈ ورچوئل نیٹ ورک کا استعمال کرتے ہوئے Kotodama نوڈس کی تعمیر ، توثیق اور تعی .ن کریں۔
سلگ: /نوریٹو /کوئیک اسٹارٹ
---

یہ عملی گائیڈ اس ورک فلو کی عکاسی کرتا ہے جس سے ہم توقع کرتے ہیں کہ پہلی بار Norito اور Kotodama سیکھتے وقت ڈویلپرز کی پیروی کریں گے: سنگل نوڈ ڈٹرمینسٹک نیٹ ورک چلائیں ، نوڈس مرتب کریں ، مقامی خشک رن انجام دیں ، اور پھر اسے حوالہ CLI کا استعمال کرتے ہوئے Kotodama ایکس پر بھیجیں۔

مثال کے طور پر معاہدہ کالے کے اکاؤنٹ میں ایک کلیدی/ویلیو جوڑی لکھتا ہے تاکہ آپ براہ راست `iroha_cli` کے ذریعے ضمنی اثر کو چیک کرسکیں۔

## تقاضے

- [Docker] (https://docs.docker.com/engine/install/) کمپوز V2 فعال (`defaults/docker-compose.single.yml` میں مخصوص پیر کے نمونے کو چلانے کے لئے استعمال کیا جاتا ہے)۔
- اگر آپ نے پوسٹ ڈاؤن لوڈ نہیں کی ہے تو معاون بائنریز کی تعمیر کے لئے زنگ ٹول سیریز (1.76+)۔
- بائنریز `koto_compile` ، `ivm_run` اور `iroha_cli`۔ آپ اسے ورک اسپیس چیک آؤٹ سے بنا سکتے ہیں جیسا کہ ذیل میں دکھایا گیا ہے یا اسی ریلیز نمونے ڈاؤن لوڈ کرسکتے ہیں:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> مذکورہ بالا بائنریز باقی کام کی جگہ کے ساتھ ساتھ انسٹال کرنا محفوظ ہیں۔
> `serde`/`serde_json` سے متعلق نہیں ؛ Norito کوڈیکس شروع سے ختم ہونے تک نافذ کیا جاتا ہے۔

## 1۔ ایک نوڈ کے ساتھ ترقیاتی نیٹ ورک چلائیں

ذخیرے میں Docker کا ایک بنڈل شامل ہے جس میں `kagami swarm` (`defaults/docker-compose.single.yml`) کے ذریعہ تیار کیا گیا ہے۔ لنکس ڈیفالٹ جینیسیس ، کلائنٹ کی تشکیل ، اور صحت کی تحقیقات تاکہ Torii `http://127.0.0.1:8080` پر دستیاب ہو۔

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

کنٹینر چلتے ہوئے (اوپر یا منقطع) چھوڑ دیں۔ اس کے بعد کے تمام سی ایل آئی کمانڈز اس ہم مرتبہ کو `defaults/client.toml` کے ذریعے نشانہ بناتے ہیں۔

## 2. معاہدہ لکھنا

ایک ورکنگ فولڈر بنائیں اور آسان Kotodama کو محفوظ کریں مثال:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> کاپی کنٹرول کے تحت Kotodama ذرائع کو بچانا افضل ہے۔ اگر آپ کو ایک زیادہ سے زیادہ نقطہ آغاز چاہتے ہیں تو پورٹل پر [مثال کے طور پر Norito] (./examples/) کے تحت پورٹل پر بھی تیار کردہ مثالیں موجود ہیں۔

## 3. IVM کے ساتھ اسمبلی اور خشک رن

نوڈ کو بائیکوڈ IVM/Norito (`.to`) پر مرتب کریں اور پھر نیٹ ورک کو چھونے سے پہلے میزبان کو کامیاب سیسکلز کو یقینی بنانے کے لئے اسے مقامی طور پر اس پر عمل کریں:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

رنر پرنٹ کرتا ہے `info("Hello from Kotodama")` اور ورچوئل ہوسٹ پر Syscall `SET_ACCOUNT_DETAIL` پرفارم کرتا ہے۔ اگر اختیاری بائنری `ivm_tool` دستیاب ہے تو ، `ivm_tool inspect target/quickstart/hello.to` برآمد شدہ ABI ہیڈر ، فیچر بٹس ، اور انٹری پوائنٹس دکھاتا ہے۔

## 4. Torii کے ذریعے بائیکوڈ بھیجیں

نوڈ ابھی بھی چل رہا ہے ، CLI کا استعمال کرتے ہوئے Torii پر مرتب کردہ بائیک کوڈ بھیجیں۔ پہلے سے طے شدہ ترقی کی شناخت عوامی کلید سے `defaults/client.toml` پر اخذ کی گئی ہے ، لہذا اکاؤنٹ کی شناخت یہ ہے کہ:
```
i105...
```

Torii ، چین ID ، اور دستخطی پر دستخط کرنے کے لئے کنفیگریشن فائل کا استعمال کریں:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

سی ایل آئی Norito کے ساتھ لین دین کو انکوڈ کرتا ہے ، اسے ترقیاتی کلید کے ساتھ دستخط کرتا ہے ، اور اسے ورکنگ پیئر کو بھیجتا ہے۔ Syscall `set_account_detail` کے لئے لاگز Docker کی نگرانی کریں یا پرعزم ٹرانزیکشن کے ہیش کے لئے CLI آؤٹ پٹ پر عمل کریں۔

## 5۔ حیثیت میں تبدیلی کی تصدیق کریں

معاہدے کے ذریعہ لکھے ہوئے اکاؤنٹ کی تفصیل لانے کے لئے وہی CLI پروفائل استعمال کریں:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

آپ کو Norito پر مبنی پے لوڈ JSON دیکھنا چاہئے:

```json
{
  "hello": "world"
}
```

اگر قیمت غائب ہے تو ، چیک کریں کہ Docker کمپوز سروس ابھی بھی چل رہی ہے اور `iroha` کے ذریعہ اطلاع دی گئی ٹرانزیکشن ہیش `Committed` ریاست تک پہنچ گئی ہے۔

## اگلے اقدامات- تلاش کرنے کے لئے خود بخود تیار کردہ [مثالوں کی گیلری] (./examples/) دریافت کریں
  مزید اعلی درجے کی Kotodama اسنیپٹس کس طرح کرتے ہیں Norito Syscalls سے ملتے ہیں۔
- پڑھیں [ابتدائی گائیڈ حاصل کرنا Norito] (./getting-started) گہری وضاحت کے لئے۔
  مرتب/رنر ٹولز اور پبلشنگ منشور اور IVM میٹا ڈیٹا کے لئے۔
- جب اپنے معاہدوں پر تکرار کرتے ہو تو ، `npm run sync-norito-snippets` in استعمال کریں
  ڈاؤن لوڈ کے قابل نچوڑوں کو دوبارہ پیدا کرنے کے لئے ورک اسپیس تاکہ پورٹل دستاویزات اور نوادرات باقی رہیں
  `crates/ivm/docs/examples/` کے تحت ذرائع کے ساتھ ہم آہنگ۔