---
lang: ur
direction: rtl
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2026-01-03T18:07:56.917770+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# کمپیوٹ لین (SSC-1)

کمپیوٹ لین ڈٹرمینسٹک HTTP طرز کی کالوں کو قبول کرتی ہے ، ان کو Kotodama پر نقشہ بناتا ہے
انٹری پوائنٹس ، اور بلنگ اور گورننس ریویو کے لئے ریکارڈز پیمائش/رسیدیں۔
یہ آر ایف سی مینی فیسٹ اسکیما ، کال/رسید کے لفافے ، سینڈ باکس کے محافظوں کو منجمد کرتا ہے ،
اور پہلی ریلیز کے لئے ترتیب ڈیفالٹس۔

## منشور

- اسکیما: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`)۔
- `abi_version` `1` پر پن کیا گیا ہے ؛ ایک مختلف ورژن کے ساتھ ظاہر ہونے کو مسترد کردیا جاتا ہے
  توثیق کے دوران
- ہر راستہ اعلان کرتا ہے:
  - `id` (`service` ، `method`)
  - `entrypoint` (Kotodama انٹری پوائنٹ کا نام)
  - کوڈیک اجازت فہرست (`codecs`)
  - ٹی ٹی ایل/گیس/درخواست/رسپانس ٹوپیاں (`ttl_slots` ، `gas_budget` ، `max_*_bytes`)
  - تعی .ن/عملدرآمد کلاس (`determinism` ، `execution_class`)
  - SoraFS ingress/ماڈل ڈسکریپٹر (`input_limits` ، اختیاری `model`)
  - قیمتوں کا فیملی (`price_family`) + ریسورس پروفائل (`resource_profile`)
  - توثیق کی پالیسی (`auth`)
- سینڈ باکس گارڈیلز منشور `sandbox` بلاک میں رہتے ہیں اور سب کے ذریعہ شیئر کیے جاتے ہیں
  راستے (وضع/بے ترتیب/اسٹوریج اور غیر ارادیت پسندانہ سیسکل مسترد)۔

مثال: `fixtures/compute/manifest_compute_payments.json`۔

## کال ، درخواستیں ، اور رسیدیں

- اسکیما: `ComputeRequest` ، `ComputeCall` ، `ComputeCallSummary` ، `ComputeReceipt` ،
  `ComputeMetering` ، `ComputeOutcome` in
  `crates/iroha_data_model/src/compute/mod.rs`۔
- `ComputeRequest::hash()` کیننیکل درخواست ہیش تیار کرتا ہے (ہیڈر رکھے جاتے ہیں
  ایک جینیاتی `BTreeMap` میں اور پے لوڈ `payload_hash` کے طور پر لے جایا جاتا ہے)۔
- `ComputeCall` نام کی جگہ/روٹ ، کوڈیک ، ٹی ٹی ایل/گیس/رسپانس کیپ کو اپنی گرفت میں لے لیتا ہے ،
  ریسورس پروفائل + پرائس فیملی ، AUTH (`Public` یا UAID-BOOND
  `ComputeAuthn`) ، تعی .ن (`Strict` بمقابلہ `BestEffort`) ، عمل درآمد کلاس
  اشارے (CPU/GPU/TEE) ، SoraFS ان پٹ بائٹس/ٹکڑوں ، اختیاری اسپانسر کا اعلان کیا گیا
  بجٹ ، اور کیننیکل درخواست کا لفافہ۔ درخواست ہیش کے لئے استعمال کی جاتی ہے
  ری پلے تحفظ اور روٹنگ۔
- راستے اختیاری SoraFS ماڈل حوالہ جات اور ان پٹ کی حدود کو سرایت کرسکتے ہیں
  (inline/chunk caps) ؛ منشور سینڈ باکس کے قواعد گیٹ جی پی یو/ٹی اشارے۔
- `ComputePriceWeights::charge_units` پیمائش کے اعداد و شمار کو بل والے کمپیوٹ میں تبدیل کرتا ہے
  چکروں اور ایگریس بائٹس پر سیل ڈویژن کے ذریعے یونٹ۔
- `ComputeOutcome` رپورٹیں `Success` ، `Timeout` ، `OutOfMemory` ،
  `BudgetExhausted` ، یا `InternalError` اور اختیاری طور پر جوابی ہیش/ شامل ہے
  آڈٹ کے لئے سائز/کوڈیک۔

مثال کے طور پر:
- کال کریں: `fixtures/compute/call_compute_payments.json`
- رسید: `fixtures/compute/receipt_compute_payments.json`

## سینڈ باکس اور وسائل کے پروفائلز- SoraFS عملدرآمد کے موڈ کو `IvmOnly` کو ڈیفالٹ کے ذریعہ لاک کرتا ہے ،
  درخواست ہیش سے بیجوں کا تعی.
  غیر تعصب کے سیسکلز تک رسائی حاصل کریں ، اور اسے مسترد کردیں۔ جی پی یو/ٹی اشارے کے ذریعہ گیٹڈ ہیں
  `allow_gpu_hints`/`allow_tee_hints` عملدرآمد کو برقرار رکھنے کے لئے۔
- `ComputeResourceBudget` سائیکل ، لکیری میموری ، اسٹیک پر فی پروفائل ٹوپیاں سیٹ کرتا ہے
  سائز ، IO بجٹ ، اور ایگریس ، نیز جی پی یو کے اشارے اور واسی لائٹ مددگاروں کے لئے ٹوگل۔
- ڈیفالٹس دو پروفائلز (`cpu-small` ، `cpu-balanced`) کے تحت جہاز بھیج دیں
  `defaults::compute::resource_profiles` ڈٹرمینسٹک فال بیکس کے ساتھ۔

## قیمتوں کا تعین اور بلنگ یونٹ

- پرائس فیملیز (`ComputePriceWeights`) نقشہ میں چکروں اور ایگریس بائٹس کو کمپیوٹ میں
  اکائیوں ؛ ڈیفالٹس `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` کے ساتھ چارج کریں
  `unit_label = "cu"`۔ فیملیز کو `price_family` کے ذریعہ منشور میں اور
  داخلے کے وقت نافذ
- پیمائش کے ریکارڈ `charged_units` کے علاوہ خام سائیکل/انگریز/ایگریس/دورانیہ رکھتے ہیں
  مفاہمت کے لئے کل۔ چارجز کو پھانسی کے طبقے کے ذریعہ بڑھایا جاتا ہے اور
  تعی .ن ضرب (`ComputePriceAmplifiers`) اور اس کے ذریعہ
  `compute.economics.max_cu_per_call` ؛ ایڈریس کے ذریعہ کلیمپ کیا جاتا ہے
  `compute.economics.max_amplification_ratio` کو پابند ردعمل بڑھاوا۔
- کفیل بجٹ (`ComputeCall::sponsor_budget_cu`) کے خلاف نافذ کیا گیا ہے
  فی کال/روزانہ کیپس ؛ بلڈ یونٹوں کو اعلان کردہ کفیل بجٹ سے زیادہ نہیں ہونا چاہئے۔
- گورننس کی قیمتوں کی تازہ کاریوں میں رسک کلاس کی حدود استعمال ہوتی ہیں
  `compute.economics.price_bounds` اور بیس لائن کنبے میں ریکارڈ کیا گیا
  `compute.economics.price_family_baseline` ؛ استعمال کریں
  `ComputeEconomics::apply_price_update` اپ ڈیٹ کرنے سے پہلے ڈیلٹا کو درست کرنے کے لئے
  the active family map. Torii اپڈیٹس کے استعمال کو تشکیل دیں
  `ConfigUpdate::ComputePricing` ، اور کسو اسی حدود کے ساتھ اس کا اطلاق کرتا ہے
  گورننس میں ترمیمات کا تعی .ن رکھیں۔

## کنفیگریشن

`crates/iroha_config/src/parameters` میں نئی ​​کمپیوٹ کنفیگریشن زندہ رہتی ہے:

- صارف کا نظارہ: `Compute` (`user.rs`) ENV کے ساتھ اوور رائڈس کے ساتھ:
  - `COMPUTE_ENABLED` (پہلے سے طے شدہ `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- قیمتوں کا تعین/معاشیات: `compute.economics` کیپچرز
  `max_cu_per_call`/`max_amplification_ratio` ، فیس اسپلٹ ، اسپانسر کیپس
  (فی کال اور روزانہ کیو) ، قیمت فیملی بیس لائنز + رسک کلاسز/حدود کے لئے
  گورننس کی تازہ کارییں ، اور عملدرآمد کلاس ملٹیپلر (جی پی یو/ٹی/بہترین کوشش)۔
- اصل / ڈیفالٹس: `actual.rs` / `defaults.rs::compute` پارسڈ کو بے نقاب کریں
  `Compute` ترتیبات (نام کی جگہیں ، پروفائلز ، قیمت کے کنبے ، سینڈ باکس)۔
- غلط تشکیلات (خالی نام کی جگہیں ، ڈیفالٹ پروفائل/فیملی لاپتہ ، ٹی ٹی ایل کیپ
  الٹا) پارسنگ کے دوران `InvalidComputeConfig` کے طور پر منظر عام پر لایا جاتا ہے۔

## ٹیسٹ اور فکسچر

۔
  `crates/iroha_data_model/src/compute/mod.rs` (`fixtures_round_trip` دیکھیں ،
  `request_hash_is_stable` ، `pricing_rounds_up_units`)۔
- JSON فکسچر `fixtures/compute/` میں براہ راست رہتے ہیں اور ڈیٹا ماڈل کے ذریعہ استعمال کیے جاتے ہیں
  رجعت کوریج کے لئے ٹیسٹ۔

## سلو کنٹرول اور بجٹ- `compute.slo.*` ترتیب گیٹ وے SLO knobs (فلائٹ قطار میں قطار) کو بے نقاب کرتی ہے
  گہرائی ، آر پی ایس کیپ ، اور تاخیر کے اہداف) میں
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`۔ پہلے سے طے شدہ: 32
  ان فلائٹ ، 512 ہر راستے میں قطار ، 200 آر پی ایس ، پی 50 25 ایم ایس ، پی 95 75 ایم ایس ، پی 99 120 ایم ایس۔
- ایس ایل او کے خلاصے اور ایک درخواست/ایگریس پر قبضہ کرنے کے لئے ہلکا پھلکا بینچ کنٹرول چلائیں
  اسنیپ شاٹ: `کارگو رن -پی ایکس ایکسک -بن کمپیوٹ_گیٹ وے -بینچ [مینی فیسٹ_پاتھ]
  [iterations] [concurrency] [out_dir]` (defaults: `fixtures/compute/manifest_compute_payments.json`,
  128 تکرار ، ہم آہنگی 16 ، آؤٹ پٹ کے تحت
  `artifacts/compute_gateway/bench_summary.{json,md}`)۔ بینچ استعمال کرتا ہے
  تعی .ن پے لوڈ (`fixtures/compute/payload_compute_payments.json`) اور
  ورزش کے دوران ری پلے تصادم سے بچنے کے لئے فی درخواست ہیڈر
  `echo`/`uppercase`/`sha3` انٹری پوائنٹس۔

## SDK/CLI پیریٹی فکسچر

- کیننیکل فکسچر `fixtures/compute/` کے تحت رہتے ہیں: ظاہر ، کال ، پے لوڈ ، اور
  گیٹ وے طرز کا جواب/رسید لے آؤٹ۔ پے لوڈ ہیشوں کو کال سے ملنا چاہئے
  `request.payload_hash` ؛ مددگار پے لوڈ میں رہتا ہے
  `fixtures/compute/payload_compute_payments.json`۔
- CLI جہاز `iroha compute simulate` اور `iroha compute invoke`:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` LIVE
  `javascript/iroha_js/src/compute.js` کے تحت رجعت ٹیسٹ کے ساتھ
  `javascript/iroha_js/test/computeExamples.test.js`۔
- سوئفٹ: `ComputeSimulator` اسی فکسچر کو لوڈ کرتا ہے ، پے لوڈ ہیشوں کی توثیق کرتا ہے ،
  اور ٹیسٹوں کے ساتھ انٹری پوائنٹ کو نقالی کرتا ہے
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`۔
- CLI/JS/سوئفٹ مددگار سب ایک ہی Norito فکسچر کا اشتراک کرتے ہیں تاکہ SDKs کر سکیں
  درخواست کی تعمیر کی توثیق کریں اور بغیر کسی ٹکرائے بغیر آف لائن ہیش ہینڈلنگ
  گیٹ وے چلا رہا ہے۔