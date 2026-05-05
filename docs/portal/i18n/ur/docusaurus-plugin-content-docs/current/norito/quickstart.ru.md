---
lang: ur
direction: rtl
source: docs/portal/docs/norito/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: کوئیک اسٹارٹ Norito
تفصیل: ریلیز ٹولز اور ڈیفالٹ سنگل نوڈ نیٹ ورک کے ساتھ معاہدہ Kotodama جمع ، تصدیق اور تعینات کریں۔
سلگ: /نوریٹو /کوئیک اسٹارٹ
---

یہ واک تھرو اس عمل کی عکاسی کرتا ہے جس کی ہم توقع کرتے ہیں کہ ڈویلپرز کی پیروی کریں گے جب Norito اور Kotodama کا سامنا کرنا پڑتا ہے: ایک جینیاتی سنگل نوڈ نیٹ ورک لائیں ، معاہدہ مرتب کریں ، مقامی خشک رن کریں ، پھر اسے حوالہ CLI کے ساتھ Kotodama ایکس کے ذریعے جمع کروائیں۔

نمونہ کا معاہدہ کالر کے اکاؤنٹ میں کلیدی/ویلیو جوڑی لکھتا ہے تاکہ آپ `iroha_cli` کا استعمال کرکے فوری طور پر ضمنی اثر کی جانچ کرسکیں۔

## تقاضے

- [Docker] (https://docs.docker.com/engine/install/) کمپوز V2 فعال (`defaults/docker-compose.single.yml` میں بیان کردہ نمونہ پیر کو شروع کرنے کے لئے استعمال کیا جاتا ہے)۔
- اگر آپ شائع شدہ افراد کو ڈاؤن لوڈ نہیں کرتے ہیں تو معاون بائنریز کی تعمیر کے لئے زنگ ٹولچین (1.76+)۔
- بائنریز `koto_compile` ، `ivm_run` اور `iroha_cli`۔ انہیں چیک آؤٹ ورک اسپیس سے جمع کیا جاسکتا ہے ، جیسا کہ ذیل میں دکھایا گیا ہے ، یا ریلیز کے متعلقہ نمونے ڈاؤن لوڈ کریں:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> یہ بائنریز باقی کام کی جگہ کے ساتھ ہی انسٹال کرنا محفوظ ہیں۔
> وہ کبھی بھی `serde`/`serde_json` سے لنک نہیں کرتے ہیں۔ Norito کوڈیکس اختتام سے آخر تک استعمال ہوتے ہیں۔

## 1. سنگل نوڈ دیو نیٹ ورک شروع کریں

ریپوزٹری میں Docker کمپوز بنڈل ہے جو `kagami swarm` (`defaults/docker-compose.single.yml`) کے ذریعہ تیار کیا گیا ہے۔ یہ پہلے سے طے شدہ جینیسیس ، کلائنٹ کی تشکیل اور صحت کی تحقیقات کو جوڑتا ہے تاکہ Torii `http://127.0.0.1:8080` پر دستیاب ہو۔

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

کنٹینر کو چلاتے ہوئے (پس منظر یا پیش منظر میں) چھوڑیں۔ اس کے بعد کی تمام سی ایل آئی کالز اس ہم مرتبہ کو `defaults/client.toml` کے ذریعے نشانہ بنائیں گی۔

## 2. معاہدہ لکھیں

ایک ورکنگ ڈائرکٹری بنائیں اور کم سے کم مثال Kotodama کو بچائیں:

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

> Kotodama ذرائع کو ورژن کنٹرول میں رکھنے کو ترجیح دیں۔ پورٹل پر پوسٹ کی گئی مثالوں میں [Norito مثالوں کی گیلری] (./examples/) میں بھی دستیاب ہیں اگر آپ کو ایک امیر اسٹارٹر کٹ کی ضرورت ہو۔

## 3. IVM کے ساتھ تالیف اور خشک رن

معاہدے کو بائیکوڈ IVM/Norito (`.to`) پر مرتب کریں اور اس بات کو یقینی بنانے کے لئے مقامی طور پر چلائیں کہ میزبان سیسکلز نیٹ ورک تک رسائی سے پہلے گزرتے ہیں:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

رنر لاگ ان `info("Hello from Kotodama")` اور مصنوعی میزبان کے خلاف سیسکل `SET_ACCOUNT_DETAIL` پر عمل درآمد کرتا ہے۔ اگر اختیاری `ivm_tool` بائنری دستیاب ہے تو ، `ivm_tool inspect target/quickstart/hello.to` کمانڈ ABI ہیڈر ، فیچر بٹس ، اور برآمد شدہ انٹری پوائنٹس کو دکھاتا ہے۔

## 4. Torii کے ذریعے بائیکوڈ بھیجیں

جب نوڈ چل رہا ہے تو ، CLI کے توسط سے Torii پر مرتب کردہ بائیک کوڈ بھیجیں۔ پہلے سے طے شدہ دیو شناخت `defaults/client.toml` میں عوامی کلید سے اخذ کی گئی ہے ، لہذا اکاؤنٹ کی شناخت یہ ہے کہ:
```
<i105-account-id>
```

URL Torii ، چین ID اور دستخطی پر دستخط کرنے کے لئے کنفیگریشن فائل کا استعمال کریں:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```سی ایل آئی ٹرانزیکشن Norito کو انکوڈ کرتا ہے ، اسے دیو کی کلید کے ساتھ دستخط کرتا ہے اور اسے چلتے ہوئے ہم مرتبہ بھیجتا ہے۔ سیسکل `set_account_detail` کے لئے Docker لاگز کی نگرانی کریں یا پرعزم ٹرانزیکشن ہیش کے لئے CLI آؤٹ پٹ کی نگرانی کریں۔

## 5. ریاستی تبدیلیوں کے لئے چیک کریں

اکاؤنٹ کی تفصیل حاصل کرنے کے لئے وہی سی ایل آئی پروفائل استعمال کریں جس نے معاہدہ ریکارڈ کیا:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

آپ کو Norito پر مبنی JSON پے لوڈ دیکھنا چاہئے:

```json
{
  "hello": "world"
}
```

اگر قیمت غائب ہے تو ، اس بات کو یقینی بنائیں کہ Docker کمپوز سروس ابھی بھی چل رہی ہے اور یہ کہ `iroha` تیار کرنے والے ٹرانزیکشن ہیش ریاست `Committed` تک پہنچ چکے ہیں۔

## اگلے اقدامات

- دیکھنے کے لئے خود کار طریقے سے تیار کردہ [مثال کے طور پر گیلری] (./examples/) دریافت کریں
  کس طرح مزید اعلی درجے کے ٹکڑوں Kotodama کو SYSCALLS Norito میں نقش کیا جاتا ہے۔
- مزید گہرائی کے لئے [Norito شروعاتی ہدایت نامہ حاصل کرنا] (./getting-started) پڑھیں
  مرتب/رنر ٹولز کی وضاحت ، تعی .ن کی تعیناتی اور IVM میٹا ڈیٹا۔
- جب اپنے معاہدوں پر کام کرتے ہو تو ، `npm run sync-norito-snippets` in استعمال کریں
  ڈاؤن لوڈ کے قابل ٹکڑوں کو دوبارہ تخلیق کرنے اور پورٹل دستاویزات اور نمونے رکھنے کے لئے ورک اسپیس
  `crates/ivm/docs/examples/` میں ذرائع کے ساتھ ہم آہنگ۔