---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# آرکسٹریٹر GA پیریٹی رپورٹ SoraFS

ملٹی بازیافت کے تعی .ن پسندانہ برابری کو اب ایس ڈی کے نے ٹریک کیا ہے تاکہ
ریلیز انجینئرز اس بات کی تصدیق کرسکتے ہیں کہ پے لوڈ بائٹس ، حصہ رسیدیں ،
فراہم کنندہ رپورٹس اور اسکور بورڈ کے نتائج کے درمیان منسلک رہتے ہیں
عمل درآمد۔ ہر استعمال کے تحت کیننیکل ملٹی فراہم کرنے والا بنڈل کھاتا ہے
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/` ، جو SF1 پلان کو پیکج کرتا ہے ،
فراہم کنندہ میٹا ڈیٹا ، ٹیلی میٹری اسنیپ شاٹ اور آرکسٹریٹر کے اختیارات۔

## روسٹ بیس لائن

- ** کمانڈ: ** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
۔
  جمع شدہ پے لوڈ بائٹس ، حصہ کی رسیدیں ، فراہم کنندہ کی رپورٹیں اور تصدیق کرنا
  اسکور بورڈ کے نتائج۔ یہ آلہ چوٹی کی ہم آہنگی کو بھی ٹریک کرتا ہے
  اور ورکنگ سیٹ (`max_parallel x max_chunk_length`) کا موثر سائز۔
- ** پرفارمنس گارڈ: ** ہر عمل کو سی آئی ہارڈ ویئر پر 2 s میں مکمل کرنا چاہئے۔
- ** ورکنگ سیٹ چھت: ** SF1 پروفائل کے ساتھ کنٹرول `max_parallel = 3` کا اطلاق ہوتا ہے ،
  ونڈو <= 196608 بائٹس دینا۔

نمونہ لاگ آؤٹ پٹ:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## جاوا اسکرپٹ SDK کنٹرول

- ** کمانڈ: ** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- ** دائرہ کار: ** `iroha_js_host::sorafsMultiFetchLocal` کے ذریعہ وہی حقیقت ادا کرتا ہے ،
  پے لوڈ ، رسیدوں ، فراہم کنندہ کی رپورٹوں اور اسکور بورڈ اسنیپ شاٹس کے درمیان موازنہ کرنا
  لگاتار پھانسی۔
- ** پرفارمنس گارڈ: ** ہر عملدرآمد کو 2 s کے اندر ختم ہونا چاہئے۔ کنٹرول پرنٹ کرتا ہے
  ماپا مدت اور محفوظ بائٹس کی چھت (`max_parallel = 3` ، `peak_reserved_bytes <= 196608`)۔

مثال کے طور پر خلاصہ لائن:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## سوئفٹ ایس ڈی کے کنٹرول

- ** کمانڈ: ** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- ** دائرہ کار: ** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` میں بیان کردہ برابری سویٹ چلاتا ہے ،
  Norito برج (`sorafsLocalFetch`) کے ذریعے دو بار SF1 حقیقت کھیلنا۔ استعمال
  پے لوڈ بائٹس ، حصہ رسیدیں ، فراہم کنندہ رپورٹس اور اسکور بورڈ اندراجات کا استعمال کرتے ہوئے تصدیق کرتا ہے
  مورچا/جے ایس سوئٹ کی طرح ایک ہی عزم میٹا ڈیٹا فراہم کرنے والا اور ٹیلی میٹری اسنیپ شاٹس۔
- ** برج بوٹسٹریپ: ** ڈیمانڈ اور بوجھ پر کنٹرول ڈیکمپریس `dist/NoritoBridge.xcframework.zip`
  `dlopen` کے ذریعے میکوس سلائس۔ اگر XCFramework غائب ہے یا SoraFS بائنڈنگز نہیں ہے تو ، اس میں فالس بیک ہے
  `cargo build -p connect_norito_bridge --release` اور اس کے خلاف لنک
  `target/release/libconnect_norito_bridge.dylib` ، آئی سی میں دستی سیٹ اپ کے بغیر۔
- ** پرفارمنس گارڈ: ** ہر عمل کو سی آئی ہارڈ ویئر پر 2 s میں ختم کرنا ہوگا۔ کنٹرول پرنٹ کرتا ہے
  ماپا مدت اور محفوظ بائٹس (`max_parallel = 3` ، `peak_reserved_bytes <= 196608`) کی چھت۔

مثال کے طور پر خلاصہ لائن:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## ازگر بائنڈنگز کنٹرول- ** کمانڈ: ** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- ** دائرہ کار: ** اعلی سطحی ریپر `iroha_python.sorafs.multi_fetch_local` اور اس کے ڈیٹا کلائے چلتا ہے
  ٹائپ کیا گیا تاکہ کیننیکل فکسچر اسی API کے ذریعے بہتا ہو جو پہیے کھاتے ہیں۔ ٹیسٹ
  `providers.json` سے دوبارہ فراہم کرنے والے میٹا ڈیٹا کو ، ٹیلی میٹری اسنیپ شاٹ کو انجیکشن لگاتا ہے اور تصدیق کرتا ہے
  پے لوڈ بائٹس ، منڈ رسیدیں ، فراہم کنندہ کی رپورٹیں اور اسکور بورڈ مواد کی طرح
  مورچا/جے ایس/سوئفٹ سوئٹ۔
۔
  Pytest کی درخواست کرنے سے پہلے `sorafs_multi_fetch_local` کو پابند کرنا ؛ جب خود کو ختم کیا جائے تو
  بائنڈنگ دستیاب نہیں ہے۔
- ** پرفارمنس گارڈ: ** ایک ہی بجٹ <= 2s مورچا سویٹ کے طور پر ؛ Pytest کی گنتی کو ریکارڈ کرتا ہے
  ریلیز آرٹیکٹیکٹ کے لئے جمع بائٹس اور فراہم کنندہ کی شرکت کا خلاصہ۔

ریلیز گیٹنگ میں ہر ایک استعمال (زنگ ، ازگر ، جے ایس ، سوئفٹ) کی سمری آؤٹ پٹ کو حاصل کرنا ہوگا۔
محفوظ شدہ رپورٹ میں پے لوڈ کی رسیدوں اور میٹرکس کا مستقل طور پر موازنہ کیا جاسکتا ہے
ایک تعمیر کو فروغ دیں۔ ہر پیریٹی سویٹ کو چلانے کے لئے `ci/sdk_sorafs_orchestrator.sh` چلائیں
(زنگ ، ازگر بائنڈنگز ، جے ایس ، سوئفٹ) ایک ہی پاس میں ؛ CI نمونے لازمی طور پر منسلک کریں
اس مددگار کے علاوہ `matrix.md` پیدا (SDK/حیثیت/دورانی ٹیبل) کا لاگ ان نچوڑ ٹکٹ پر ٹکٹ پر
جائزہ لینے والوں کے لئے مقامی طور پر سوٹ کو دوبارہ جاری کیے بغیر پیریٹی میٹرکس کا آڈٹ کرنے کے لئے جاری کریں۔