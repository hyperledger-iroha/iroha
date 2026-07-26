---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-ایس ڈی کے انڈیکس
عنوان: SoraFS SDK گائیڈز
سائڈبار_لیبل: ایس ڈی کے گائڈز
تفصیل: SoraFS نمونے کو مربوط کرنے کے لئے زبان سے متعلق مخصوص ٹکڑوں۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/developer/sdk/index.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

اس حب کو زبان کے مددگاروں کی پیروی کرنے کے لئے استعمال کریں جو SoraFS ٹولچین کے ساتھ جہاز بھیجتے ہیں۔
زنگ آلود ٹکڑوں کے لئے [مورچا SDK ٹکڑوں] (./developer-sdk-rust.md) پر جائیں۔

## زبان کے ذریعہ مددگار

- ** ازگر ** - `sorafs_multi_fetch_local` (مقامی آرکسٹریٹر دھواں ٹیسٹ) اور
  `sorafs_gateway_fetch` (E2E گیٹ وے مشقیں) اب `telemetry_region` کو قبول کریں
  اختیاری کے علاوہ `transport_policy` کا ایک اوور رائڈ
  (`"soranet-first"`, `"soranet-strict"` or `"direct-only"`), reflecting the
  سی ایل آئی رول آؤٹ۔ جب ایک مقامی کوئک پراکسی اٹھایا جاتا ہے ،
  `sorafs_gateway_fetch` براؤزر کو ظاہر کرتا ہے
  `local_proxy_manifest` ٹیسٹ کے لئے ٹرسٹ بنڈل اڈاپٹر کو پہنچانے کے لئے
  براؤزر کا
- ** جاوا اسکرپٹ ** - `sorafsMultiFetchLocal` ازگر کے مددگار کی عکاسی کرتا ہے ، لوٹ رہا ہے
  پے لوڈ بائٹس اور رسید کے خلاصے ، جبکہ `sorafsGatewayFetch` ورزش کرتا ہے
  Torii گیٹ وے ، زنجیروں کو مقامی پراکسی ظاہر کرتا ہے اور اسی اوور رائڈز کو بے نقاب کرتا ہے
  CLI سے ٹیلی میٹری/ٹرانسپورٹ۔
- ** مورچا ** - خدمات شیڈولر کو براہ راست اس کے ذریعے سرایت کرسکتی ہیں
  `sorafs_car::multi_fetch` ؛ کے حوالہ سے مشورہ کریں
  [مورچا SDK اسنیپٹس] (./developer-sdk-rust.md) پروف اسٹریم اور انضمام مددگاروں کے لئے
  آرکسٹریٹر کا
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` reuses the HTTP executor
  Torii اور احترام `GatewayFetchOptions`۔ اس کے ساتھ جوڑیں
  `ClientConfig.Builder#setSorafsGatewayUri` اور PQ اپ لوڈ اشارے
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) جب بڑھتا ہے محدود ہونا ضروری ہے
  صرف PQ-صرف راستوں پر۔

## اسکور بورڈ اور پالیسی نوبس

ازگر (`sorafs_multi_fetch_local`) اور جاوا اسکرپٹ مددگار
(`sorafsMultiFetchLocal`) استعمال شدہ ٹیلی میٹری کے ساتھ شیڈیولر کے اسکور بورڈ کو بے نقاب کریں
بذریعہ CLI:- پروڈکشن بائنریز اسکور بورڈ کو بطور ڈیفالٹ قابل بناتے ہیں۔ قائم کرتا ہے
  کھیلتے وقت `use_scoreboard=True` (یا `telemetry` کے ان پٹ فراہم کریں)
  مددگار کے لئے فکسچر سپلائرز کے وزن کے آرڈر کو حاصل کرنے کے لئے
  اشتہارات اور حالیہ ٹیلی میٹری اسنیپ شاٹس سے میٹا ڈیٹا۔
- حساب شدہ وزن کے ساتھ ساتھ `return_scoreboard=True` کو سیٹ کریں
  حص con ہ رسیدیں اور CI لاگز کو تشخیص پر قبضہ کرنے کی اجازت دیں۔
- ساتھیوں کو مسترد کرنے یا شامل کرنے کے لئے فکسز `deny_providers` یا `boost_providers` استعمال کریں
  `priority_delta` جب شیڈولر سپلائرز کا انتخاب کرتا ہے۔
- پہلے سے طے شدہ کرنسی `"soranet-first"` کو برقرار رکھیں جب تک کہ آپ ڈاون گریڈ تیار نہ کریں۔
  `"direct-only"` صرف اس وقت فراہم کرتا ہے جب تعمیل والے خطے کو لازمی طور پر ریلے سے بچنا چاہئے
  یا جب SNNET-5A فال بیک کی جانچ کرتے ہو ، اور صرف PQ-Olly پائلٹوں کے لئے `"soranet-strict"`
  گورننس کی منظوری کے ساتھ۔
- گیٹ وے مددگار بھی `scoreboardOutPath` اور `scoreboardNowUnixSecs` کو بے نقاب کرتے ہیں۔
  حساب کتاب اسکور بورڈ کو برقرار رکھنے کے لئے `scoreboardOutPath` تشکیل دیں (پرچم کی عکاسی کرتا ہے
  `--scoreboard-out` CLI سے) `cargo xtask sorafs-adoption-check` کے لئے نمونے کی توثیق کرنے کے لئے
  SDK ، اور `scoreboardNowUnixSecs` استعمال کریں جب فکسچر کی مستحکم قیمت کی ضرورت ہوتی ہے
  `assume_now` قابل تولیدی میٹا ڈیٹا کے لئے۔ جاوا اسکرپٹ مددگار میں آپ بھی کر سکتے ہیں
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` سیٹ کریں ؛ جب چھوڑ دیا جائے
  لیبل `region:<telemetryRegion>` (`sdk:js` پر فال بیک کے ساتھ) حاصل کرتا ہے۔ کا مددگار
  جب اسکور بورڈ برقرار رہتا ہے تو ازگر خود بخود `telemetry_source="sdk:python"` جاری کرتا ہے
  اور مضمر میٹا ڈیٹا کو غیر فعال رکھتا ہے۔

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