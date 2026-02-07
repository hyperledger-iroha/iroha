---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SoraFS آرکیسٹریٹر کے لئے # GA پیریٹی رپورٹ

اس بات کو یقینی بنانے کے لئے اب فی ایس ڈی کے کا پتہ لگایا جاتا ہے
ریلیز انجینئر اس بات کو یقینی بناسکتے ہیں کہ بائٹس پے لوڈ ، حصہ رسیدیں ، فراہم کنندہ
رپورٹس اور اسکور بورڈ کے نتائج نفاذ کے مابین مستقل رہتے ہیں۔
ہر استعمال سے کیننیکل ملٹی فراہم کرنے والے بنڈل کا استعمال ہوتا ہے
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/` ، جس میں SF1 پلان شامل ہے ،
فراہم کنندہ میٹا ڈیٹا ، ٹیلی میٹری اسنیپ شاٹ اور آرکسٹریٹر کے اختیارات۔

## مورچا بیس لائن

- ** کمانڈ: ** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
۔
  جمع شدہ پے لوڈ بائٹس ، چنک رسیدیں ، فراہم کنندہ کی رپورٹیں اور نتائج کی جانچ کرنا
  اسکور بورڈ. یہ آلہ چوٹی کے مقابلہ اور موثر کو بھی ٹریک کرتا ہے
  ورکنگ سیٹ سائز (`max_parallel × max_chunk_length`)۔
- ** پرفارمنس گارڈ: ** ہر رن کو CI ہارڈ ویئر پر 2 s میں مکمل ہونا چاہئے۔
- ** ورکنگ سیٹ چھت: ** پروفائل کے لئے SF1 استعمال `max_parallel = 3` استعمال کرتا ہے ،
  ونڈو ≤ 196608 بائٹس دینا۔

نمونہ لاگ آؤٹ پٹ:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## جاوا اسکرپٹ SDK کنٹرول

- ** کمانڈ: ** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- ** دائرہ کار: ** `iroha_js_host::sorafsMultiFetchLocal` کے ذریعہ وہی حقیقت ادا کرتا ہے ،
  پے لوڈ ، رسیدوں ، فراہم کنندہ کی رپورٹوں اور اسکور بورڈ اسنیپ شاٹس کے درمیان موازنہ کرنا
  یکے بعد دیگرے لانچ
- ** پرفارمنس گارڈ: ** ہر لانچ کو 2 s میں مکمل ہونا چاہئے۔ کنٹرول پرنٹس
  ماپا مدت اور چھت محفوظ بائٹس (`max_parallel = 3` ، `peak_reserved_bytes ≤ 196 608`)۔

مثال کے طور پر خلاصہ لائن:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## سوئفٹ ایس ڈی کے کنٹرول

- ** کمانڈ: ** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- ** دائرہ کار: ** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` سے پیریٹی سویٹ لانچ کرتا ہے ،
  Norito برج (`sorafsLocalFetch`) کے ذریعے دو بار SF1 حقیقت کھیلنا۔ استعمال کی جانچ پڑتال
  اسی کا استعمال کرتے ہوئے پے لوڈ بائٹس ، حص ch ے کی رسیدیں ، فراہم کنندہ رپورٹس اور اندراجات اسکور بورڈ
  زنگ/جے ایس سوئٹ کی طرح ڈٹرمینسٹک فراہم کنندہ میٹا ڈیٹا اور ٹیلی میٹری اسنیپ شاٹس۔
- ** برج بوٹسٹریپ: ** ہارنس ڈیمانڈ پر `dist/NoritoBridge.xcframework.zip` کھولیں
  `dlopen` کے ذریعے میکوس سلائس کو لوڈ کرتا ہے۔ جب XCFramework غائب ہے یا اس میں SoraFS بائنڈنگز نہیں ہیں ،
  فال بیک `cargo build -p connect_norito_bridge --release` پر انجام دیا جاتا ہے اور اس کے ساتھ منسلک ہوتا ہے
  `target/release/libconnect_norito_bridge.dylib` ، CI میں دستی ترتیب کے بغیر۔
- ** پرفارمنس گارڈ: ** ہر لانچ کو CI ہارڈ ویئر پر 2 s میں مکمل ہونا چاہئے۔ کنٹرول پرنٹس
  ماپا مدت اور چھت محفوظ بائٹس (`max_parallel = 3` ، `peak_reserved_bytes ≤ 196 608`)۔

مثال کے طور پر خلاصہ لائن:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## ازگر بائنڈنگز کنٹرول- ** کمانڈ: ** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- ** دائرہ کار: ** اعلی سطحی ریپر `iroha_python.sorafs.multi_fetch_local` اور اس کی ٹائپڈ چیک کرتا ہے
  ڈیٹاکلاسس تاکہ کیننیکل فکسچر اسی API سے گزرتا ہے جسے کہا جاتا ہے
  صارفین پہیے `providers.json` ، انجیکشن سے ٹیسٹ فراہم کرنے والے میٹا ڈیٹا کو دوبارہ تعمیر کرتا ہے
  ٹیلی میٹری اسنیپ شاٹ اور چیک پے لوڈ بائٹس ، حص ch ے کی رسیدیں ، فراہم کنندہ کی رپورٹیں اور
  اسکور بورڈ کے مشمولات مورچا/جے ایس/سوئفٹ سوئٹ جیسی ہیں۔
- **Pre-req:** Run `maturin develop --release` (or install wheel) to `_crypto`
  Pytest سے پہلے `sorafs_multi_fetch_local` کا پابند بائنڈنگ ؛ ہارنس آٹو اسکیپس ،
  جب بائنڈنگ دستیاب نہیں ہے۔
- ** پرفارمنس گارڈ: ** ایک ہی بجٹ ≤ 2 s مورچا سویٹ کے طور پر ؛ پائسٹسٹ لاگ نمبر
  جمع شدہ بائٹس اور ریلیز آرٹ فیکٹ کے لئے شرکت فراہم کرنے والوں کا خلاصہ۔

ریلیز گیٹنگ میں ہر ایک استعمال (زنگ ، ازگر ، جے ایس ، سوئفٹ) کی سمری آؤٹ پٹ کو حاصل کرنا چاہئے تاکہ
محفوظ شدہ رپورٹ پروموشن سے پہلے پے لوڈ کی رسیدوں اور میٹرکس کا یکساں طور پر موازنہ کرسکتی ہے
تعمیر. تمام برابری سوئٹ کو عملی جامہ پہنانے کے لئے `ci/sdk_sorafs_orchestrator.sh` چلائیں
(زنگ ، ازگر بائنڈنگز ، جے ایس ، سوئفٹ) ایک پاس میں ؛ سی آئی نوادرات کو لاگ ان اقتباس سے منسلک کرنا ہوگا
اس مددگار اور پیدا کردہ `matrix.md` (ٹیبل SDK/حیثیت/دورانیہ) ٹکٹ جاری کرنے کے لئے ،
تاکہ جائزہ لینے والے پیریٹی میٹرکس کو مقامی طور پر دوبارہ چلائے بغیر آڈٹ کرسکیں۔