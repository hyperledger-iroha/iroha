---
lang: ur
direction: rtl
source: docs/portal/docs/norito/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: Norito شروع کرنا
تفصیل: رہائی کے ٹولنگ اور پہلے سے طے شدہ سنگل پیر نیٹ ورک کے ساتھ Kotodama معاہدہ کی تعمیر ، توثیق اور تعینات کریں۔
سلگ: /نوریٹو /کوئیک اسٹارٹ
---

یہ واک تھرو اس ورک فلو کی پیروی کرتا ہے جس سے ہم توقع کرتے ہیں کہ جب وہ پہلی بار Norito اور Kotodama کو دریافت کریں تو: ایک واحد ہم مرتبہ ڈٹرمینسٹک نیٹ ورک شروع کریں ، معاہدہ مرتب کریں ، اسے مقامی طور پر خشک کریں ، پھر اسے حوالہ CLI کے ساتھ Torii کے ذریعے بھیجیں۔

مثال کے طور پر معاہدہ کالر کے اکاؤنٹ میں کلیدی/ویلیو جوڑی لکھتا ہے تاکہ آپ `iroha_cli` کے ساتھ فوری طور پر ضمنی اثر کو چیک کرسکیں۔

## شرائط

- [Docker] (https://docs.docker.com/engine/install/) کمپوز V2 ایکٹو کے ساتھ (`defaults/docker-compose.single.yml` میں بیان کردہ مثال کے طور پر شروع کرنے کے لئے استعمال کیا جاتا ہے)۔
- اگر آپ شائع شدہ افراد کو ڈاؤن لوڈ نہیں کرتے ہیں تو معاون بائنریز بنانے کے لئے ٹولچین زنگ (1.76+)۔
- بائنریز `koto_compile` ، `ivm_run` اور `iroha_cli`۔ آپ انہیں نیچے کی طرح ورک اسپیس چیک آؤٹ سے تعمیر کرسکتے ہیں یا اسی ریلیز نمونے ڈاؤن لوڈ کرسکتے ہیں:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> مذکورہ بالا بائنریز باقی ورک اسپیس کے ساتھ انسٹال کرنا محفوظ ہیں۔
> وہ کبھی بھی `serde`/`serde_json` سے لنک نہیں کرتے ہیں۔ Norito کوڈیکس کو اختتام سے آخر میں لاگو کیا جاتا ہے۔

## 1۔ ایک ہم مرتبہ دیو نیٹ ورک شروع کریں

ذخیرہ میں ایک بنڈل Docker جامع شامل ہے جو `kagami swarm` (`defaults/docker-compose.single.yml`) کے ذریعہ تیار کیا گیا ہے۔ یہ پہلے سے طے شدہ جینیسیس ، کلائنٹ کی تشکیل اور صحت کی تحقیقات کو جوڑتا ہے تاکہ Torii `Kotodama` تک قابل رسائی ہو۔

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

کنٹینر کو گھومنے دیں (پیش منظر میں یا پس منظر میں)۔ اس کے بعد کے تمام سی ایل آئی کمانڈز اس ہم مرتبہ کو `defaults/client.toml` کے ذریعے نشانہ بناتے ہیں۔

## 2. معاہدہ لکھیں

ایک ورکنگ ڈائرکٹری بنائیں اور کم سے کم Kotodama کو بچائیں مثال:

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

> Kotodama ذرائع کو ورژن کنٹرول میں رکھنے کو ترجیح دیں۔ پورٹل پر میزبانی کی گئی مثالیں [مثال کے طور پر گیلری Norito] (./examples/) میں بھی دستیاب ہیں اگر آپ کو ایک زیادہ سے زیادہ نقطہ آغاز چاہتے ہیں۔

## 3. IVM کے ساتھ مرتب اور خشک رن

معاہدے کو بائیکوڈ IVM/Norito (`.to`) میں مرتب کریں اور اس بات کی تصدیق کرنے کے لئے مقامی طور پر اس پر عمل کریں کہ میزبان سیسکلز نیٹ ورک کو مارنے سے پہلے کامیاب ہوجاتے ہیں:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

رنر لاگ `info("Hello from Kotodama")` پرنٹ کرتا ہے اور مصنوعی میزبان کے خلاف سیسکل `SET_ACCOUNT_DETAIL` انجام دیتا ہے۔ اگر اختیاری بائنری `ivm_tool` دستیاب ہے تو ، `ivm_tool inspect target/quickstart/hello.to` ABI ہیڈر ، فیچر بٹس اور برآمد شدہ انٹری پوائنٹ کو دکھاتا ہے۔

## 4. Torii کے ذریعے بائیک کوڈ جمع کروائیں

نوڈ ابھی بھی چل رہا ہے ، CLI کے ساتھ Torii پر مرتب کردہ بائیک کوڈ بھیجیں۔ پہلے سے طے شدہ ترقی کی شناخت `defaults/client.toml` میں عوامی کلید سے اخذ کی گئی ہے ، لہذا اکاؤنٹ کی شناخت ہے
```
soraカタカナ...
```

URL Torii ، چین ID اور دستخطی کی کلید فراہم کرنے کے لئے کنفیگریشن فائل کا استعمال کریں:```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

سی ایل آئی Norito کے ساتھ لین دین کو انکوڈ کرتا ہے ، اسے دیو کی کلید کے ساتھ دستخط کرتا ہے ، اور اسے پھانسی دینے والے ہم مرتبہ کو بھیجتا ہے۔ `set_account_detail` سیسکل کے لئے Docker لاگز کی نگرانی کریں یا پرعزم ٹرانزیکشن ہیش کے لئے CLI آؤٹ پٹ پڑھیں۔

## 5. ریاست کی تبدیلی کی جانچ کریں

اکاؤنٹ کی تفصیلات کو بازیافت کرنے کے لئے وہی سی ایل آئی پروفائل استعمال کریں جو معاہدے نے لکھا ہے:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

آپ کو JSON پے لوڈ کو Norito کے ذریعہ بیکڈ دیکھنا چاہئے:

```json
{
  "hello": "world"
}
```

اگر قیمت غائب ہے تو ، تصدیق کریں کہ Docker کمپوز سروس ابھی بھی چل رہی ہے اور `iroha` کے ذریعہ اطلاع دی گئی ٹرانزیکشن ہیش `Committed` ریاست میں پہنچ گئی ہے۔

## اگلے اقدامات

- دیکھنے کے لئے خود کار طریقے سے تیار کردہ [مثال کے طور پر گیلری] (./examples/) دریافت کریں
  Kotodama Syscalls میں کس طرح مزید اعلی Kotodama اسنیپٹس کا نقشہ ہے۔
- وضاحت کے لئے [گائیڈ Norito شروع کرنا] (./getting-started) پڑھیں
  مرتب/رنر ٹولز ، مینی فیسٹ تعیناتی ، اور میٹا ڈیٹا IVM میں گہرا غوطہ۔
- جب اپنے معاہدوں پر تکرار کرتے ہو تو ، `npm run sync-norito-snippets` میں استعمال کریں
  ڈاؤن لوڈ کے قابل ٹکڑوں کو دوبارہ تخلیق کرنے کے لئے ورک اسپیس تاکہ پورٹل دستاویزات اور نمونے باقی رہیں
  `crates/ivm/docs/examples/` کے تحت ذرائع کے ساتھ ہم آہنگ۔