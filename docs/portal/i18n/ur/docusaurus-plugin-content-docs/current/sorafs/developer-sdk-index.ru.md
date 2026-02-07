---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-ایس ڈی کے انڈیکس
عنوان: SDK گائڈز SoraFS
سائڈبار_لیبل: ایس ڈی کے گائڈز
تفصیل: نمونے کو مربوط کرنے کے لئے زبان کے ٹکڑوں SoraFS۔
---

::: نوٹ کینونیکل ماخذ
:::

اس حب کا استعمال زبان کے مددگاروں سے باخبر رہنے کے ل tool ٹولچین SoraFS کے ساتھ بھیجے جائیں۔
زنگ آلود ٹکڑوں کے لئے ، [مورچا SDK اسنیپٹس] (./developer-sdk-rust.md) پر جائیں۔

## زبان کے مددگار

- ** ازگر ** - `sorafs_multi_fetch_local` (مقامی آرکسٹریٹر دھواں ٹیسٹ) اور
  `sorafs_gateway_fetch` (گیٹ وے E2E مشقیں) اب اختیاری قبول کریں
  `telemetry_region` کے علاوہ `transport_policy` کے لئے اوور رائڈ
  (`"soranet-first"` ، `"soranet-strict"` یا `"direct-only"`) ، رول آؤٹ نوبس کی عکاسی کرتا ہے
  سی ایل آئی۔ جب مقامی کوئک پراکسی سامنے آجائے تو ، `sorafs_gateway_fetch` واپس آجاتا ہے
  `local_proxy_manifest` میں براؤزر ظاہر ہوتا ہے تاکہ ٹیسٹ ٹرسٹ کے بنڈل کو پاس کرسکیں
  براؤزر اڈیپٹر۔
- ** جاوا اسکرپٹ ** - `sorafsMultiFetchLocal` ازگر کے مددگار کی عکاسی کرتا ہے ، پے لوڈ بائٹس کو واپس کرتا ہے
  اور رسیدوں کے خلاصے ، جبکہ `sorafsGatewayFetch` ورزش کرتا ہے Torii گیٹ وے ،
  مقامی پراکسی کے مظاہروں کو آگے بڑھاتا ہے اور اسی ٹیلی میٹری/ٹرانسپورٹ اوور رائڈز کو ظاہر کرتا ہے ،
  CLI کی طرح
- ** زنگ ** - خدمات براہ راست `sorafs_car::multi_fetch` کے ذریعے شیڈولر کو سرایت کرسکتی ہیں۔
  پروف اسٹریم مددگاروں اور انضمام کے لئے [مورچا SDK اسنیپٹس] (./developer-sdk-rust.md) دیکھیں
  آرکیسٹریٹر۔
- **Android** - `HttpClientTransport.sorafsGatewayFetch(…)` reuses Torii HTTP executor
  اور `GatewayFetchOptions` کو مدنظر رکھتا ہے۔ کے ساتھ مل کر
  `ClientConfig.Builder#setSorafsGatewayUri` اور PQ اپ لوڈ اشارے
  .

## اسکور بورڈ اور پالیسی نوبس

ازگر (`sorafs_multi_fetch_local`) اور جاوا اسکرپٹ (`sorafsMultiFetchLocal`) مددگار
CLI کے ذریعہ استعمال شدہ ٹیلی میٹری سے واقف شیڈیولر اسکور بورڈ مرتب کریں:- پروڈکشن بائنریز میں اسکور بورڈ بطور ڈیفالٹ شامل ہے۔ `use_scoreboard=True` انسٹال کریں
  (یا `telemetry` اندراجات کو پاس کریں) جب فکسچر کھیل رہے ہو تو مددگار آؤٹ پٹ
  میٹا ڈیٹا اشتہارات اور تازہ ترین ٹیلی میٹری اسنیپ شاٹس سے فراہم کنندگان کا وزن کا حکم۔
- `return_scoreboard=True` کو انسٹال وصولیوں کے ساتھ حساب شدہ وزن حاصل کرنے کے لئے انسٹال کریں ،
  سی آئی لاگ کو تشخیصی ریکارڈ کرنے کی اجازت دینا۔
- ساتھیوں کو مسترد کرنے یا شامل کرنے کے لئے `deny_providers` یا `boost_providers` صفوں کا استعمال کریں
  `priority_delta` جب شیڈولر فراہم کنندگان کا انتخاب کرتا ہے۔
- پہلے سے طے شدہ پوز `"soranet-first"` رکھیں جب تک کہ آپ ڈاؤن گریڈ کرنے کی تیاری نہیں کررہے ہیں۔
  `"direct-only"` صرف اس وقت بتائیں جب تعمیل والے خطے کو ریلے سے بچنے کے لئے ضروری ہو یا کب
  SNNET-5A فال بیک بیک ریہرسلز ، اور PQ- صرف پائلٹوں کے لئے `"soranet-strict"` کے ساتھ
  گورننس کی منظوری۔
- گیٹ وے مددگار بھی `scoreboardOutPath` اور `scoreboardNowUnixSecs` کو بے نقاب کرتے ہیں۔
  حساب کتاب اسکور بورڈ کو بچانے کے لئے `scoreboardOutPath` سیٹ کریں (پرچم کے مطابق ہے
  CLI `--scoreboard-out`) تاکہ `cargo xtask sorafs-adoption-check` توثیق کرسکے
  جب فکسچر کی ضرورت ہوتی ہے تو SDK نمونے ، اور `scoreboardNowUnixSecs` استعمال کریں
  مستحکم قیمت `assume_now` تولیدی میٹا ڈیٹا کے لئے۔ جاوا اسکرپٹ مددگار میں آپ کر سکتے ہیں
  اضافی طور پر `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` انسٹال کریں۔
  اگر لیبل کو خارج کردیا گیا ہے تو ، یہ `region:<telemetryRegion>` (`sdk:js` پر فال بیک) ہے۔ ازگر ہیلپر
  اسکور بورڈ کی بچت کرتے وقت خود بخود `telemetry_source="sdk:python"` لکھتا ہے
  مضمر میٹا ڈیٹا غیر فعال۔

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