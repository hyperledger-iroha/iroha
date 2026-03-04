---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-ایس ڈی کے انڈیکس
عنوان: SoraFS SDK گائیڈز
سائڈبار_لیبل: ایس ڈی کے گائڈز
تفصیل: SoraFS نمونے کو مربوط کرنے کے لئے زبان کے ٹکڑوں۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/developer/sdk/index.md` کا آئینہ دار ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

اس حب کو زبان کے مددگاروں کی پیروی کرنے کے لئے استعمال کریں جو SoraFS ٹولچین کے ساتھ آتے ہیں۔
زنگ آلود ٹکڑوں کے لئے ، [مورچا SDK اسنیپٹس] (./developer-sdk-rust.md) پر جائیں۔

## زبان کے ذریعہ مددگار

- ** ازگر ** - `sorafs_multi_fetch_local` (مقامی آرکسٹریٹر دھواں ٹیسٹ) اور
  `sorafs_gateway_fetch` (گیٹ وے E2E مشقیں) اب `telemetry_region` کو قبول کریں
  اختیاری کے علاوہ `transport_policy` کا ایک اوور رائڈ
  (`"soranet-first"`, `"soranet-strict"` or `"direct-only"`), mirroring the control knobs
  سی ایل آئی رول آؤٹ۔ جب ایک مقامی کوئک پراکسی اوپر جاتا ہے تو ، `sorafs_gateway_fetch` واپس آجاتا ہے
  براؤزر `local_proxy_manifest` میں ظاہر ہوتا ہے تاکہ ٹیسٹ ٹرسٹ کے بنڈل کو پاس کریں
  براؤزر اڈیپٹر کے لئے۔
- ** جاوا اسکرپٹ ** - `sorafsMultiFetchLocal` ازگر کے مددگار کو آئینہ دار ، لوٹ رہا ہے
  پے لوڈ بائٹس اور رسید کے خلاصے ، جبکہ `sorafsGatewayFetch` ورزش کرتا ہے
  گیٹ وے Torii ، زنجیروں کو مقامی پراکسی ظاہر کرتا ہے اور اسی اوور رائڈز کو بے نقاب کرتا ہے
  سی ایل آئی ٹیلی میٹری/ٹرانسپورٹ۔
- ** زنگ ** - خدمات شیڈولر کو براہ راست اس کے ذریعے سرایت کرسکتی ہیں
  `sorafs_car::multi_fetch` ؛ حوالہ دیکھیں
  [مورچا SDK اسنیپٹس] (./developer-sdk-rust.md) پروف اسٹریم مددگاروں اور
  آرکسٹریٹر انضمام۔
- **Android** - `HttpClientTransport.sorafsGatewayFetch(...)` reuses the HTTP executor
  Torii اور آنر `GatewayFetchOptions` کا۔ کے ساتھ مل کر
  `ClientConfig.Builder#setSorafsGatewayUri` اور PQ اپ لوڈ اشارے
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) جب اپ لوڈ کی ضرورت ہوتی ہے
  صرف پی کیو صرف راستوں پر رہیں۔

## اسکور بورڈ اور پالیسی نوبس

ازگر (`sorafs_multi_fetch_local`) اور جاوا اسکرپٹ مددگار
(`sorafsMultiFetchLocal`) استعمال شدہ ٹیلی میٹری کے ساتھ شیڈولر اسکور بورڈ کو بے نقاب کریں
سی ایل آئی کے ذریعے:- پروڈکشن بائنریز اسکور بورڈ کو بطور ڈیفالٹ قابل بناتے ہیں۔ `use_scoreboard=True` سیٹ کریں
  (یا `telemetry` ان پٹ فراہم کریں) جب مددگار کے لئے فکسچر کھیل رہے ہو
  اشتہاری میٹا ڈیٹا اور اسنیپ شاٹس پر مبنی فراہم کنندگان کا وزن کا آرڈر دینا
  حالیہ ٹیلی میٹری۔
- رسیدوں کے ساتھ حساب شدہ وزن حاصل کرنے کے لئے `return_scoreboard=True` سیٹ کریں
  حصہ کا ، CI لاگز کو تشخیص پر قبضہ کرنے کی اجازت دیتا ہے۔
- ساتھیوں کو مسترد کرنے یا شامل کرنے کے لئے `deny_providers` یا `boost_providers` صفوں کا استعمال کریں
  `priority_delta` جب شیڈولر فراہم کرنے والوں کا انتخاب کرتا ہے۔
- پہلے سے طے شدہ کرنسی `"soranet-first"` کو برقرار رکھیں جب تک کہ آپ ڈاون گریڈ تیار نہیں کررہے ہیں۔
  `"direct-only"` صرف اس وقت فراہم کریں جب تعمیل والے خطے کو ریلے سے بچنے کی ضرورت ہو
  یا جب SNNET-5A فال بیک بیک کی مشق کرتے ہو ، اور صرف PQ-ONLY پائلٹوں کے لئے `"soranet-strict"`
  حکومت کی منظوری کے ساتھ۔
- گیٹ وے مددگار بھی `scoreboardOutPath` اور `scoreboardNowUnixSecs` کو بے نقاب کرتے ہیں۔
  حساب کتاب اسکور بورڈ کو برقرار رکھنے کے لئے `scoreboardOutPath` سیٹ کریں (پرچم کی آئینہ دار ہے
  `--scoreboard-out` CLI سے) Torii کے لئے توثیق کرنے کے لئے
  SDK نمونے ، اور `scoreboardNowUnixSecs` استعمال کریں جب فکسچر کی ضرورت ہوتی ہے
  `assume_now` تولیدی میٹا ڈیٹا کے لئے مستحکم قدر۔ جاوا اسکرپٹ ہیلپر میں ،
  آپ `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` بھی سیٹ کرسکتے ہیں۔
  جب لیبل کو خارج کردیا جاتا ہے تو ، یہ `region:<telemetryRegion>` (`sdk:js` پر فال بیک) حاصل کرتا ہے۔
  ازگر کا مددگار خود بخود `telemetry_source="sdk:python"` جاری کرتا ہے
  اسکور بورڈ پر برقرار رہتا ہے اور میٹا ڈیٹا کو غیر فعال کرتا رہتا ہے۔

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