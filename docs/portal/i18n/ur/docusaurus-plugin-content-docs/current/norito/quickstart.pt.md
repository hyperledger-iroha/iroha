---
lang: ur
direction: rtl
source: docs/portal/docs/norito/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: Norito کوئیک اسٹارٹ
تفصیل: ریلیز ٹولنگ اور معیاری سنگل پیر نیٹ ورک کے ساتھ Kotodama معاہدہ کی تعمیر ، توثیق اور تعی .ن کریں۔
سلگ: /نوریٹو /کوئیک اسٹارٹ
---

یہ واک تھرو اس بہاؤ کی آئینہ دار ہے جس کی ہم توقع کرتے ہیں کہ ڈویلپرز پہلی بار Norito اور Kotodama سیکھتے ہیں: ایک واحد ہم مرتبہ ڈٹرمینسٹک نیٹ ورک شروع کریں ، معاہدہ مرتب کریں ، مقامی طور پر اس کو خشک کریں ، اور پھر اسے حوالہ CLI کے ساتھ Torii کے ذریعے بھیجیں۔

مثال کے طور پر معاہدہ کالر کے اکاؤنٹ میں کلیدی/ویلیو جوڑی لکھتا ہے تاکہ آپ `iroha_cli` کے ساتھ فوری طور پر ضمنی اثر کو چیک کرسکیں۔

## شرائط

- [Docker] (https://docs.docker.com/engine/install/) کمپوز V2 فعال (`defaults/docker-compose.single.yml` میں بیان کردہ مثال کے طور پر شروع کرنے کے لئے استعمال کیا جاتا ہے)۔
- اگر آپ شائع شدہ افراد کو ڈاؤن لوڈ نہیں کرتے ہیں تو مددگار بائنریز مرتب کرنے کے لئے ٹولچین زنگ (1.76+)۔
- بائنریز `koto_compile` ، `ivm_run` اور `iroha_cli`۔ آپ انہیں ورک اسپیس چیک آؤٹ سے تیار کرسکتے ہیں جیسا کہ ذیل میں دکھایا گیا ہے یا اسی ریلیز نمونے ڈاؤن لوڈ کرسکتے ہیں:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> مذکورہ بالا بائنریز باقی کام کی جگہ کے ساتھ ساتھ انسٹال کرنا محفوظ ہیں۔
> وہ کبھی بھی `serde`/`serde_json` کے ساتھ نہیں لنک کرتے ہیں۔ Norito کوڈیکس کو اختتام سے آخر میں لاگو کیا جاتا ہے۔

## 1۔ ایک ہم مرتبہ دیو نیٹ ورک شروع کریں

ذخیرے میں Docker کمپوز بنڈل شامل ہے جو `kagami swarm` (`defaults/docker-compose.single.yml`) کے ذریعہ تیار کیا گیا ہے۔ یہ معیاری پیدائش ، کلائنٹ کی تشکیل ، اور صحت کی تحقیقات کو جوڑتا ہے تاکہ Torii `Kotodama` سے قابل رسائی ہو۔

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

کنٹینر چلتے ہوئے (پیش منظر میں یا علیحدہ) چھوڑ دیں۔ اس کے بعد کے تمام CLI کالز `defaults/client.toml` کے ذریعے اس ہم مرتبہ کی طرف اشارہ کرتے ہیں۔

## 2. معاہدہ لکھیں

ایک ورکنگ ڈائرکٹری بنائیں اور Kotodama کی کم سے کم مثال محفوظ کریں:

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

معاہدے کو بائیکوڈ IVM/Norito (`.to`) پر مرتب کریں اور اس بات کی تصدیق کے لئے اسے مقامی طور پر چلائیں کہ میزبان سیسکلز نیٹ ورک کو چھونے سے پہلے کام کرتے ہیں:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

رنر `info("Hello from Kotodama")` لاگ پرنٹ کرتا ہے اور مذاق والے میزبان کے خلاف `SET_ACCOUNT_DETAIL` سیسکل کو انجام دیتا ہے۔ اگر اختیاری بائنری `ivm_tool` دستیاب ہے تو ، `ivm_tool inspect target/quickstart/hello.to` ABI ہیڈر ، فیچر بٹس اور برآمد شدہ انٹری پوائنٹس کو دکھاتا ہے۔

## 4. Torii کے ذریعے بائیکوڈ بھیجیں

نوڈ ابھی بھی چل رہا ہے ، CLI کا استعمال کرتے ہوئے Torii پر مرتب کردہ بائیک کوڈ بھیجیں۔ پہلے سے طے شدہ ترقی کی شناخت عوامی کلید سے `defaults/client.toml` پر اخذ کی گئی ہے ، لہذا اکاؤنٹ ID اور
```
<i105-account-id>
```

Torii URL ، چین ID ، اور سائننگ کلید فراہم کرنے کے لئے کنفیگریشن فائل کا استعمال کریں:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```سی ایل آئی Norito کے ساتھ لین دین کو انکوڈ کرتا ہے ، اسے دیو کی کلید کے ساتھ دستخط کرتا ہے اور اسے چلانے والے ہم مرتبہ پر بھیجتا ہے۔ `set_account_detail` سیسکل کیلئے Docker لاگز دیکھیں یا پرعزم ٹرانزیکشن ہیش کے لئے CLI آؤٹ پٹ کی نگرانی کریں۔

## 5. ریاست کی تبدیلی کو چیک کریں

اکاؤنٹ کی تفصیل لانے کے لئے وہی سی ایل آئی پروفائل استعمال کریں جو معاہدہ ریکارڈ کیا گیا ہے:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

آپ کو JSON پے لوڈ کو Norito کے ذریعہ تعاون یافتہ دیکھنا چاہئے:

```json
{
  "hello": "world"
}
```

اگر قیمت غائب ہے تو ، اس بات کی تصدیق کریں کہ Docker کمپوز سروس ابھی بھی چل رہی ہے اور `iroha` کے ذریعہ اطلاع دی گئی ٹرانزیکشن ہیش ریاست `Committed` تک پہنچ گئی ہے۔

## اگلے اقدامات

- دیکھنے کے لئے خود بخود تیار کردہ [مثال کے طور پر گیلری] (./examples/) دریافت کریں
  Kotodama Syscalls میں کس طرح مزید اعلی Kotodama اسنیپٹس کا نقشہ ہے۔
- وضاحت کے لئے [Norito] (./getting-started) پڑھیں
  مرتب/رنر ٹولنگ کی گہری تفہیم ، ظاہر اور IVM میٹا ڈیٹا کی تعیناتی۔
- جب اپنے معاہدوں پر تکرار کرتے ہو تو ، ورک اسپیس میں `npm run sync-norito-snippets` استعمال کریں
  ڈاؤن لوڈ کے قابل ٹکڑوں کو دوبارہ تخلیق کریں اور پورٹل دستاویزات اور نمونے کو `crates/ivm/docs/examples/` میں ذرائع کے ساتھ ہم آہنگ رکھیں۔