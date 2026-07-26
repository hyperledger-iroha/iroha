---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-ایس ڈی کے انڈیکس
عنوان: SoraFS کے لئے SDK گائڈز
سائڈبار_لیبل: ایس ڈی کے ڈائریکٹریز
تفصیل: نمونے SoraFS کو مربوط کرنے کے لئے زبان سے متعلق مخصوص ٹکڑوں۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/sorafs/developer/sdk/index.md` کی عکاسی کرتا ہے۔ اس بات کو یقینی بنائیں کہ اس وقت تک دونوں کاپیاں ہم آہنگی میں رکھیں جب تک کہ پرانا اسفنکس کلسٹر ریٹائر نہ ہوجائے۔
:::

SoraFS ٹول سیریز کے ساتھ بھیجے گئے زبان سے متعلق مددگاروں کو ٹریک کرنے کے لئے اس مرکز کا استعمال کریں۔
زنگ آلود ٹکڑوں کے لئے [مورچا SDK ٹکڑوں] (./developer-sdk-rust.md) پر جائیں۔

## زبان کی مدد

- ** ازگر ** - `sorafs_multi_fetch_local` (مقامی فارمیٹر کے لئے دھواں ٹیسٹ) اور
  `sorafs_gateway_fetch` (گیٹ وے E2E مشقیں) اب اختیاری طور پر `telemetry_region` کو قبول کرتا ہے
  بائی پاس `transport_policy` کے ساتھ
  (`"soranet-first"`, `"soranet-strict"` or `"direct-only"`), mirroring the firing buttons on
  سی ایل آئی۔ جب مقامی کوئک پراکسی چلاتے ہو تو ، `sorafs_gateway_fetch` براؤزر کے تحت ظاہر ہوتا ہے
  `local_proxy_manifest` تاکہ ٹیسٹ ٹرسٹ بنڈل کو براؤزر اڈیپٹر کو منتقل کرسکے۔
- ** جاوا اسکرپٹ ** - `sorafsMultiFetchLocal` ازگر کے مددگار کی عکاسی کرتا ہے اور پے لوڈ بائٹس اور خلاصے واپس کرتا ہے
  رسیدیں ، جبکہ `sorafsGatewayFetch` گیٹ وے Torii ، اور مقامی پراکسی مینی فیسٹ سے گزرتا ہے ،
  یہ اسی ٹیلی میٹری/ٹرانسپورٹ اوور رائڈس کو بے نقاب کرتا ہے جو سی ایل آئی میں پائے جاتے ہیں۔
- ** زنگ ** - خدمات میں براہ راست `sorafs_car::multi_fetch` کے ذریعے شیڈولر شامل ہوسکتا ہے۔ دیکھو
  [مورچا SDK اسنیپٹس] (./developer-sdk-rust.md) پروف اسٹریم مددگاروں اور فارمیٹر انضمام کے لئے۔
- ** Android ** - `HttpClientTransport.sorafsGatewayFetch(…)` نجی HTTP پورٹ کو دوبارہ استعمال کرتا ہے
  Torii کے ساتھ اور `GatewayFetchOptions` کے ساتھ ارتکاب کرتا ہے۔ اس کے ساتھ جوڑیں
  `ClientConfig.Builder#setSorafsGatewayUri` اور PQ کے ساتھ ٹپ میں اضافہ کریں
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) جب لفٹوں پر عمل پیرا ہونا ضروری ہے
  صرف پی کیو ٹریک کے ساتھ۔

## اسکور بورڈ کیز اور پالیسیاں

یہ ازگر کے مددگار (`sorafs_multi_fetch_local`) اور جاوا اسکرپٹ دونوں کو بے نقاب کرتا ہے
(`sorafsMultiFetchLocal`) CLI کے ذریعہ استعمال ہونے والا ٹیلی میٹری سے واقف اسکور بورڈ:

- پروڈکشن بائنریز اسکور بورڈ کو بطور ڈیفالٹ قابل بناتی ہیں۔ `use_scoreboard=True` سیٹ کریں
  (یا اندراجات کو محفوظ کریں `telemetry`) جب فکسچر کو دوبارہ شروع کریں تاکہ اسسٹنٹ آرڈر نکال سکے
  ایڈورٹ ڈیٹا اور حالیہ ٹیلی میٹک اسنیپ شاٹس کے وزن والے فراہم کنندگان۔
- `return_scoreboard=True` کو گنتی کی رسیدوں کے ساتھ حساب شدہ وزن حاصل کرنے کے لئے سیٹ کریں تاکہ آپ کر سکیں
  تشخیص کی گرفتاری سے سی آئی لاگ ان۔
- ساتھیوں کو مسترد کرنے یا شامل کرنے کے لئے اریوں `deny_providers` یا `boost_providers` استعمال کریں
  `priority_delta` جب شیڈولر فراہم کرنے والوں کا انتخاب کرتا ہے۔
- ڈیفالٹ `"soranet-first"` رکھیں جب تک کہ آپ ڈاؤن گریڈ کرنے کی تیاری نہیں کر رہے ہیں۔ درخواست دیں
  `"direct-only"` صرف اس وقت جب تعمیل کا علاقہ ریلے سے بچنا چاہئے یا تربیت دیتے وقت
  سنیٹ -5 اے باؤنس ، گورننس کی منظوری کے ساتھ صرف پی کیو-صرف پائلٹوں کے لئے `"soranet-strict"` ریزرو کریں۔
-گیٹ ایڈز `scoreboardOutPath` اور `scoreboardNowUnixSecs` بھی ظاہر کرتے ہیں۔ ایڈجسٹ کریں
  `scoreboardOutPath` حساب شدہ اسکور بورڈ کو بچانے کے لئے (CLI پرچم `--scoreboard-out` کی عکاسی کرتا ہے)
  تاکہ `cargo xtask sorafs-adoption-check` SDK نمونے کی جانچ کرسکے ، اور استعمال کرسکے
  `scoreboardNowUnixSecs` جب فکسچر کو اعداد و شمار کے لئے ایک مقررہ `assume_now` قدر کی ضرورت ہوتی ہے
  تولیدی وضاحتی۔ جاوا اسکرپٹ مددگار میں آپ سیٹ بھی کرسکتے ہیں
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ؛ اور جب آپ اسٹیکر کو حذف کرتے ہیں
  `region:<telemetryRegion>` (`sdk:js` پر فال بیک کے ساتھ) سے ماخوذ ہے۔ ازگر کا مددگار خود بخود جاری کیا جاتا ہے
  `telemetry_source="sdk:python"` جب بھی اسکور بورڈ محفوظ ہوتا ہے وہ میٹا ڈیٹا رکھتا ہے
  مضمر غیر فعال ہے۔

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```