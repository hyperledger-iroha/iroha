---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# آرکسٹریٹر GA پیریٹی رپورٹ SoraFS

ملٹی بازیافت کے تعی .ن پسندانہ برابری کی نگرانی اب ایس ڈی کے کے ذریعہ کی جاتی ہے لہذا ریلیز انجینئر اس بات کی تصدیق کرسکتے ہیں
پے لوڈ بائٹس ، حصہ رسیدیں ، فراہم کنندہ رپورٹس اور اسکور بورڈ کے نتائج کے درمیان سیدھ میں رہتا ہے
عمل درآمد۔ ہر استعمال میں کیننیکل ملٹی فراہم کرنے والا بنڈل کھاتا ہے
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/` ، جو SF1 پلان ، فراہم کنندہ میٹا ڈیٹا ، ٹیلی میٹری اسنیپ شاٹ اور پیکج کرتا ہے
آرکسٹریٹر کے اختیارات۔

## بیسرسٹ

- ** کمانڈ: ** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
۔
  جمع شدہ پے لوڈ بائٹس ، حصہ رسیدیں ، فراہم کنندہ رپورٹس اور اسکور بورڈ کے نتائج۔ آلہ سازی
  یہ چوٹی کے مقابلے اور موثر ورکنگ سیٹ سائز (`max_parallel x max_chunk_length`) کو بھی ٹریک کرتا ہے۔
- ** پرفارمنس گارڈ: ** ہر رن کو CI ہارڈ ویئر پر 2 s میں مکمل ہونا چاہئے۔
- ** ورکنگ سیٹ چھت: ** SF1 پروفائل کے ساتھ کنٹرول `max_parallel = 3` لاگو ہوتا ہے ، جس کے نتیجے میں a
  ونڈو <= 196608 بائٹس۔

لاگ آؤٹ پٹ کی مثال:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## جاوا اسکرپٹ SDK کنٹرول

- ** کمانڈ: ** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- ** دائرہ کار: ** پے لوڈ کا موازنہ کرتے ہوئے ، `iroha_js_host::sorafsMultiFetchLocal` کے ذریعے اسی حقیقت کو دوبارہ پیش کرتا ہے ،
  رسیدیں ، فراہم کنندہ کی رپورٹیں اور اسکور بورڈ اسنیپ شاٹس کے مابین مسلسل رنز۔
- ** پرفارمنس گارڈ: ** ہر عمل کو 2 s میں ختم کرنا ہوگا۔ کنٹرول پیمائش کی مدت اور پرنٹ کرتا ہے
  محفوظ بائٹ چھت (`max_parallel = 3` ، `peak_reserved_bytes <= 196608`)۔

خلاصہ لائن مثال:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## سوئفٹ ایس ڈی کے کنٹرول

- ** کمانڈ: ** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- ** دائرہ کار: ** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` میں بیان کردہ برابری سوٹ پر عملدرآمد کرتا ہے ،
  برج Norito (`sorafsLocalFetch`) کے ذریعے دو بار SF1 حقیقت کھیلنا۔ کنٹرول پے لوڈ بائٹس کی جانچ پڑتال کرتا ہے ،
  ایک ہی عصبی فراہم کنندہ میٹا ڈیٹا کا استعمال کرتے ہوئے حصہ کی رسیدیں ، فراہم کنندہ کی رپورٹیں اور اسکور بورڈ اندراجات اور
  زنگ/جے ایس سوئٹ کے ٹیلی میٹری سنیپ شاٹس۔
۔
  `dlopen`۔ جب XCFramework غائب ہے یا SoraFS پابند نہیں ہے تو ، یہ واپس آجاتا ہے
  `cargo build -p connect_norito_bridge --release` اور `target/release/libconnect_norito_bridge.dylib` کے خلاف لنکس ،
  لہذا CI پر کسی دستی سیٹ اپ کی ضرورت نہیں ہے۔
- ** پرفارمنس گارڈ: ** ہر عمل کو سی آئی ہارڈ ویئر پر 2 s میں ختم کرنا ہوگا۔ کنٹرول پیمائش کی مدت اور پرنٹ کرتا ہے
  محفوظ بائٹس کی چھت (`max_parallel = 3` ، `peak_reserved_bytes <= 196608`)۔

خلاصہ لائن مثال:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## ازگر بائنڈنگ کا استعمال- ** کمانڈ: ** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
۔
  ٹائپ کیا گیا تاکہ کیننیکل فکسچر وہی API سے گزرتا ہے جو پہیے والے صارفین استعمال کرتے ہیں۔ ٹیسٹ
  `providers.json` سے فراہم کنندہ میٹا ڈیٹا کی تشکیل نو ، ٹیلی میٹری اسنیپ شاٹ اور چیک انجیکشن کرتا ہے
  پے لوڈ بائٹس ، منڈ رسیدیں ، فراہم کنندہ رپورٹس اور اسکور بورڈ مواد کے برابر مورچا/جے ایس/سوئفٹ سوئٹ۔
۔
  پیئسٹ کو کال کرنے سے پہلے `sorafs_multi_fetch_local` کو پابند کرنا ؛ جب پابند ہونے پر استعمال خود کو نظرانداز کرتا ہے
  دستیاب نہیں ہے۔
- ** پرفارمنس گارڈ: ** ایک ہی بجٹ <= 2 s مورچا سویٹ کے طور پر ؛ پائسٹ ریکارڈز بائٹ گنتی
  جمع اور ریلیز آرٹیکٹیکٹ کے لئے فراہم کنندہ کی شرکت کا خلاصہ۔

ریلیز گیٹنگ میں ہر ایک استعمال (زنگ ، ازگر ، جے ایس ، سوئفٹ) کی سمری آؤٹ پٹ پر قبضہ کرنا ہوگا۔
محفوظ شدہ رپورٹ کو فروغ دینے سے پہلے یکساں طور پر پے لوڈ کی رسیدوں اور میٹرکس کا موازنہ کیا جاسکتا ہے
ایک تعمیر تمام پیریٹی سوئٹ چلانے کے لئے `ci/sdk_sorafs_orchestrator.sh` چلائیں (مورچا ، ازگر
ایک ہی پاس میں پابندیاں ، جے ایس ، سوئفٹ) ؛ CI نمونے لازمی طور پر اس مددگار سے لاگ انکیپٹ منسلک کریں
اس کے علاوہ ریلیز ٹکٹ پر پیدا شدہ `matrix.md` (SDK/حیثیت/مدت کی میز)
مقامی طور پر سوٹ کو دوبارہ چلائے بغیر پیریٹی میٹرکس کا آڈٹ کریں۔