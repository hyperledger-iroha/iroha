---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-ایس ڈی کے انڈیکس
عنوان: SDK گائڈز SoraFS
سائڈبار_لیبل: ایس ڈی کے گائڈز
تفصیل: نمونے SoraFS کو مربوط کرنے کے لئے فی زبان کے ٹکڑوں۔
---

::: نوٹ کینونیکل ماخذ
:::

SoraFS ٹولچین کے ساتھ فراہم کردہ زبان کے مددگاروں کو ٹریک کرنے کے لئے اس حب کا استعمال کریں۔
زنگ کے ٹکڑوں کے لئے ، [مورچا SDK اسنیپٹس] (./developer-sdk-rust.md) پر جائیں۔

## زبان کے ذریعہ مددگار

- ** ازگر ** - `sorafs_multi_fetch_local` (مقامی آرکسٹریٹر دھواں ٹیسٹ) اور
  `sorafs_gateway_fetch` (E2E گیٹ وے مشقیں) اب قبول کرتا ہے a
  اختیاری `telemetry_region` پلس `transport_policy` کا ایک اوور رائڈ
  (`"soranet-first"`, `"soranet-strict"` or `"direct-only"`), mirroring the knobs
  سی ایل آئی رول آؤٹ۔ جب ایک مقامی کوئک پراکسی شروع ہوتا ہے ،
  `sorafs_gateway_fetch` براؤزر کے منشور کے ذریعے واپس کرتا ہے
  `local_proxy_manifest` تاکہ ٹیسٹ ٹرسٹ کے بنڈل کو منتقل کریں
  براؤزر اڈیپٹر۔
- ** جاوا اسکرپٹ ** - `sorafsMultiFetchLocal` ازگر کے مددگار کی عکاسی کرتا ہے ، لوٹ رہا ہے
  پے لوڈ بائٹس اور رسید کے خلاصے ، جبکہ `sorafsGatewayFetch` ورزش کرتا ہے
  Torii گیٹ ویز ، مقامی پراکسی ظاہر کرتا ہے اور اسی کو بے نقاب کرتا ہے
  ٹیلی میٹری/ٹرانسپورٹ کو بطور سی ایل آئی۔
- ** زنگ ** - خدمات شیڈولر کو براہ راست بھیج سکتی ہیں
  `sorafs_car::multi_fetch` ؛ حوالہ سے مشورہ کریں
  [مورچا SDK اسنیپٹس] (./developer-sdk-rust.md) پروف اسٹریم مددگاروں اور
  آرکسٹریٹر کا انضمام۔
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` reuses HTTP executor
  Torii اور احترام `GatewayFetchOptions`۔ اس کے ساتھ جوڑیں
  `ClientConfig.Builder#setSorafsGatewayUri` اور PQ اپ لوڈ انڈیکس
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) جب اپ لوڈ کرنا ضروری ہے
  صرف پی کیو صرف راستوں پر رہیں۔

## اسکور بورڈ اور پالیسی نوبس

ازگر (`sorafs_multi_fetch_local`) اور جاوا اسکرپٹ مددگار
(`sorafsMultiFetchLocal`) استعمال شدہ ٹیلی میٹری پر مبنی شیڈیولر کے اسکور بورڈ کو بے نقاب کریں
بذریعہ CLI:- پروڈکشن بائنریز اسکور بورڈ کو بطور ڈیفالٹ قابل بناتے ہیں۔ وضاحت کریں
  `use_scoreboard=True` (یا ان پٹ فراہم کریں `telemetry`) جب دوبارہ چلاتے ہو
  فکسچر تاکہ مددگار فراہم کرنے والوں کے وزن والے آرڈر کو رب سے حاصل کرے
  میٹا ڈیٹا اور حالیہ ٹیلی میٹری اسنیپ شاٹس کو ایڈورٹ کریں۔
- `return_scoreboard=True` کی وضاحت کریں کہ وزن کے ساتھ حساب شدہ وزن حاصل کریں
  چنک موصول ہوا تاکہ سی آئی لاگ ان تشخیص کو حاصل کرے۔
- ساتھیوں کو مسترد کرنے کے لئے میزیں `deny_providers` یا `boost_providers` کا استعمال کریں
  یا جب شیڈولر فراہم کنندگان کا انتخاب کرتا ہے تو `priority_delta` شامل کریں۔
- ڈیفالٹ کرنسی `"soranet-first"` کو رکھیں سوائے اس کے کہ ڈاؤن گریڈ کی صورت میں۔ فراہم کریں
  Torii صرف اس صورت میں جب تعمیل خطے کو ریلے سے بچنا چاہئے یا
  پائلٹوں کے لئے SNNET-5A فال بیک ، اور ریزرو `"soranet-strict"` کی تکرار کے دوران
  PQ- صرف گورننس کی منظوری کے ساتھ۔
- گیٹ وے مددگار بھی `scoreboardOutPath` اور `scoreboardNowUnixSecs` کو بے نقاب کرتے ہیں۔
  حساب کتاب اسکور بورڈ (پرچم کے آئینے کو برقرار رکھنے کے لئے `scoreboardOutPath` کی وضاحت کریں
  CLI `--scoreboard-out`) تاکہ `cargo xtask sorafs-adoption-check` نمونے کی توثیق کرے
  SDK ، اور `scoreboardNowUnixSecs` استعمال کریں جب فکسچر کو کسی قدر کی ضرورت ہوتی ہے
  تولیدی میٹا ڈیٹا کے لئے مستحکم `assume_now`۔ جاوا اسکرپٹ ہیلپر میں ،
  آپ `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` بھی سیٹ کرسکتے ہیں۔
  اگر لیبل کو خارج کردیا گیا ہے تو ، اس سے `region:<telemetryRegion>` (`sdk:js` پر فال بیک کے ساتھ) اخذ کیا گیا ہے۔
  ازگر کا مددگار ہر بار خود بخود `telemetry_source="sdk:python"` جاری کرتا ہے
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