---
lang: ur
direction: rtl
source: docs/portal/docs/norito/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: Norito کا فوری آغاز
تفصیل: ریلیز ٹولز اور پہلے سے طے شدہ سنگل پیر نیٹ ورک کے ساتھ Kotodama معاہدہ تخلیق ، توثیق اور تعینات کرتا ہے۔
سلگ: /نوریٹو /کوئیک اسٹارٹ
---

یہ واک تھرو اس بہاؤ کی عکاسی کرتا ہے جس کی ہم توقع کرتے ہیں کہ ڈویلپرز پہلی بار Norito اور Kotodama سیکھتے ہیں: ایک ڈٹرمینسٹک سنگل پیر نیٹ ورک کو بوٹ کریں ، معاہدہ مرتب کریں ، مقامی خشک رن کریں ، اور پھر اسے حوالہ سی ایل آئی کے ساتھ Torii پر دبائیں۔

مثال کے طور پر معاہدہ کالر اکاؤنٹ میں ایک کلیدی/ویلیو جوڑی لکھتا ہے تاکہ آپ `iroha_cli` کے ساتھ فوری طور پر ضمنی اثر چیک کرسکیں۔

## شرائط

- [Docker] (https://docs.docker.com/engine/install/) کمپوز V2 فعال (`defaults/docker-compose.single.yml` پر بیان کردہ نمونہ پیر کو شروع کرنے کے لئے استعمال کیا جاتا ہے)
- اگر آپ شائع شدہ افراد کو ڈاؤن لوڈ نہیں کرتے ہیں تو معاون بائنریز بنانے کے لئے زنگ ٹولچین (1.76+)۔
- بائنریز `koto_compile` ، `ivm_run` اور `iroha_cli`۔ آپ انہیں ورک اسپیس چیک آؤٹ سے تیار کرسکتے ہیں جیسا کہ ذیل میں دکھایا گیا ہے یا اسی ریلیز کے نمونے ڈاؤن لوڈ کرسکتے ہیں:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> مذکورہ بالا بائنریز باقی ورک اسپیس کے ساتھ ساتھ انسٹال کرنا محفوظ ہیں۔
> وہ کبھی بھی `serde`/`serde_json` سے لنک نہیں کرتے ہیں۔ Norito کوڈیکس کو اختتام سے آخر میں لاگو کیا جاتا ہے۔

## 1۔ ایک ہی پیر دیو نیٹ ورک شروع کریں

ذخیرے میں Docker کمپوز بنڈل شامل ہے جو `kagami swarm` (`defaults/docker-compose.single.yml`) کے ذریعہ تیار کیا گیا ہے۔ پہلے سے طے شدہ پیدائش ، کلائنٹ کی تشکیل اور صحت کی تحقیقات کو مربوط کریں تاکہ Torii `http://127.0.0.1:8080` میں قابل رسائی ہو۔

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

کنٹینر کو چلانے (پیش منظر یا انڈیکڈ) چھوڑ دیں۔ اس کے بعد کے تمام CLI کالز `defaults/client.toml` کا استعمال کرتے ہوئے اس ہم مرتبہ کی طرف اشارہ کرتے ہیں۔

## 2. معاہدہ لکھیں

ایک ورکنگ ڈائرکٹری بنائیں اور Kotodama کی کم سے کم مثال کو بچائیں:

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

> Kotodama ذرائع کو ورژن کنٹرول میں رکھنے کو ترجیح دیں۔ پورٹل پر میزبانی کی گئی مثالیں [مثال کے طور پر گیلری Norito] (./examples/) میں بھی دستیاب ہیں اگر آپ مزید مکمل نقطہ آغاز چاہتے ہیں۔

## 3. IVM کے ساتھ مرتب اور خشک رن

معاہدے کو بائیکوڈ IVM/Norito (`.to`) پر مرتب کریں اور اس بات کی تصدیق کے لئے اسے مقامی طور پر چلائیں کہ میزبان سیسکلز نیٹ ورک کو مارنے سے پہلے کام کرتے ہیں:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

رنر لاگ `info("Hello from Kotodama")` پرنٹ کرتا ہے اور مصنوعی میزبان کے خلاف سیسکل `SET_ACCOUNT_DETAIL` پر عمل کرتا ہے۔ اگر اختیاری بائنری `ivm_tool` دستیاب ہے تو ، `ivm_tool inspect target/quickstart/hello.to` ABI ہیڈر ، فیچر بٹس ، اور برآمد شدہ انٹری پوائنٹ کو دکھاتا ہے۔

## 4. Torii کے ذریعے بائیک کوڈ بھیجیں

نوڈ ابھی بھی چل رہا ہے ، CLI کا استعمال کرتے ہوئے Torii پر مرتب کردہ بائیک کوڈ بھیجیں۔ پہلے سے طے شدہ ترقی کی شناخت عوامی کلید سے `defaults/client.toml` پر اخذ کی گئی ہے ، لہذا اکاؤنٹ کی شناخت ہے
```
<i105-account-id>
```

Torii URL ، چین ID ، اور سائننگ کلید کی فراہمی کے لئے کنفیگریشن فائل کا استعمال کریں:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```CLI Norito کے ساتھ لین دین کو خفیہ کرتا ہے ، اسے ترقیاتی کلید کے ساتھ دستخط کرتا ہے ، اور اسے چلتے ہوئے ہم مرتبہ بھیجتا ہے۔ سیسکل `set_account_detail` کے لئے Docker لاگز کا مشاہدہ کریں یا پرعزم ٹرانزیکشن ہیش کے لئے CLI آؤٹ پٹ کی نگرانی کریں۔

## 5. حیثیت میں تبدیلی کی جانچ کریں

اکاؤنٹ کی تفصیل حاصل کرنے کے لئے وہی سی ایل آئی پروفائل استعمال کریں جس نے معاہدہ لکھا ہے:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

آپ کو JSON پے لوڈ کو Norito کے ذریعہ بیکڈ دیکھنا چاہئے:

```json
{
  "hello": "world"
}
```

اگر قیمت غائب ہے تو ، اس بات کی تصدیق کرتی ہے کہ Docker کمپوز سروس ابھی بھی چل رہی ہے اور `iroha` کے ذریعہ اطلاع دی گئی ٹرانزیکشن ہیش ریاست `Committed` تک پہنچ گئی۔

## اگلے اقدامات

- دیکھنے کے لئے خود کار طریقے سے تیار کردہ [مثال کے طور پر گیلری] (./examples/) دریافت کریں
  چونکہ مزید اعلی درجے کی Kotodama کے ٹکڑے Norito Syscalls میں نقش کیے گئے ہیں۔
- پڑھیں [Norito شروعاتی ہدایت نامہ حاصل کرنا] (./getting-started) وضاحت کے لئے
  مرتب/رنر ٹولنگ میں گہرا ، تعیناتی ظاہر اور IVM میٹا ڈیٹا۔
- جب اپنے معاہدوں پر تکرار کرتے ہو تو ، `npm run sync-norito-snippets` میں استعمال کریں
  ڈاؤن لوڈ کے قابل ٹکڑوں کو دوبارہ تخلیق کرنے کے لئے ورک اسپیس تاکہ پورٹل دستاویزات اور نمونے
  `crates/ivm/docs/examples/` میں ذرائع کے ساتھ ہم آہنگ رہیں۔