---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# GA پیریٹی رپورٹ SoraFS آرکیسٹریٹر

ملٹی بازیافت کے تعی .ن پسندانہ برابری کو اب ایس ڈی کے نے ٹریک کیا ہے تاکہ
ریلیز انجینئرز اس بات کی تصدیق کرسکتے ہیں کہ پے لوڈ بائٹس ، حصہ رسیدیں ،
فراہم کنندہ رپورٹس اور اسکور بورڈ کے نتائج کے درمیان منسلک رہتے ہیں
عمل درآمد۔ ہر استعمال میں کیننیکل ملٹی فراہم کرنے والا بنڈل کھاتا ہے
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/` ، جس میں SF1 پلان شامل ہے ،
میٹا ڈیٹا فراہم کرنے والا ، ٹیلی میٹری اسنیپ شاٹ اور آرکسٹریٹر کے اختیارات۔

## مورچا بیس لائن

- ** کمانڈ: ** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- ** دائرہ کار: ** پلان `MultiPeerFixture` پر عملدرآمد کرتا ہے جس میں دو بار پروسیس آرکسٹریٹر کے ذریعے ،
  جمع شدہ پے لوڈ بائٹس ، حصہ رسیدیں ، فراہم کنندہ کی رپورٹیں اور چیک کرکے
  اسکور بورڈ کے نتائج۔ آلہ سازی کا مقابلہ بھی جدید مقابلہ کے ساتھ ہوتا ہے
  اور ورکنگ سیٹ (`max_parallel × max_chunk_length`) کا موثر سائز۔
- ** پرفارمنس گارڈ: ** ہر عمل کو سی آئی ہارڈ ویئر پر 2 s میں ختم کرنا ہوگا۔
- ** ورکنگ سیٹ چھت: ** SF1 پروفائل کے ساتھ ، کنٹرول `max_parallel = 3` نافذ کرتا ہے ،
  ونڈو ≤ 196608 بائٹس دینا۔

نمونہ لاگ آؤٹ پٹ:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## جاوا اسکرپٹ SDK کنٹرول

- ** کمانڈ: ** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- ** دائرہ کار: ** `iroha_js_host::sorafsMultiFetchLocal` کے ذریعے اسی حقیقت کو دوبارہ چلاتا ہے ،
  پے لوڈ ، رسیدوں ، فراہم کنندہ کی رپورٹوں اور اسکور بورڈ اسنیپ شاٹس کے درمیان موازنہ کرنا
  لگاتار پھانسی۔
- ** پرفارمنس گارڈ: ** ہر عمل کو 2 s میں ختم کرنا ہوگا۔ کنٹرول پرنٹ کرتا ہے
  ماپا مدت اور محفوظ بائٹ کی حد (`max_parallel = 3` ، `peak_reserved_bytes ≤ 196 608`)۔

مثال کے طور پر خلاصہ لائن:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## سوئفٹ ایس ڈی کے کنٹرول

- ** کمانڈ: ** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- ** دائرہ کار: ** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` میں بیان کردہ برابری سوٹ پر عملدرآمد کرتا ہے ،
  برج Norito (`sorafsLocalFetch`) کے ذریعے دو بار SF1 حقیقت کو دوبارہ چلانا۔ استعمال کی جانچ پڑتال
  استعمال کرتے ہوئے پے لوڈ بائٹس ، حص ch ے کی رسیدیں ، فراہم کنندہ کی رپورٹیں اور اسکور بورڈ اندراجات
  مورچا/جے ایس سوئٹ کی طرح ایک ہی عزم میٹا ڈیٹا فراہم کرنے والا اور ٹیلی میٹری اسنیپ شاٹس۔
- ** برج بوٹسٹریپ: ** ڈیمانڈ اور بوجھ پر کنٹرول ڈیکمپریس `dist/NoritoBridge.xcframework.zip`
  `dlopen` کے ذریعے میکوس سلائس۔ جب XCFramework غائب ہے یا SoraFS پابند نہیں ہے ، تو یہ
  `cargo build -p connect_norito_bridge --release` پر سوئچ کرتا ہے اور پابند ہوتا ہے
  `target/release/libconnect_norito_bridge.dylib` ، CI میں دستی سیٹ اپ کے بغیر۔
- ** پرفارمنس گارڈ: ** ہر عمل کو سی آئی ہارڈ ویئر پر 2 s میں ختم کرنا ہوگا۔ کنٹرول پرنٹ کرتا ہے
  ماپا مدت اور محفوظ بائٹ کی حد (`max_parallel = 3` ، `peak_reserved_bytes ≤ 196 608`)۔

مثال کے طور پر خلاصہ لائن:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## ازگر بائنڈنگز کنٹرول- ** کمانڈ: ** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
۔
  تاکہ کیننیکل حقیقت پہیے والے صارفین کی طرح API سے گزر جائے۔ تعمیر نو ٹیسٹ
  `providers.json` سے میٹا ڈیٹا فراہم کرنے والا ، ٹیلی میٹری اسنیپ شاٹ کو انجیکشن کرتا ہے اور پے لوڈ بائٹس کو چیک کرتا ہے ،
  زنگ/جے ایس/سوئفٹ سوٹ جیسے حص recips ے کی رسیدیں ، فراہم کنندہ کی رپورٹیں اور اسکور بورڈ مواد۔
۔
  `sorafs_multi_fetch_local` pytest کی درخواست کرنے سے پہلے ؛ جب بائنڈنگ دستیاب نہیں ہے تو آٹو اسکیپ کنٹرول۔
- ** پرفارمنس گارڈ: ** ایک ہی بجٹ ≤ 2 s مورچا سویٹ کے طور پر ؛ پیسٹیسٹ بائٹس کی تعداد کو جمع کرتا ہے
  اور ریلیز آرٹیکٹیکٹ کے لئے فراہم کنندہ کی شرکت کا خلاصہ۔

ریلیز گیٹنگ میں ہر ایک استعمال (زنگ ، ازگر ، جے ایس ، سوئفٹ) کی سمری آؤٹ پٹ کو حاصل کرنا ہوگا۔
آرکائیوڈ رپورٹ اس سے پہلے پے لوڈ کی رسیدوں اور میٹرکس کا موازنہ کر سکتی ہے
ایک تعمیر کو فروغ دیں۔ ہر پیریٹی سویٹ لانچ کرنے کے لئے `ci/sdk_sorafs_orchestrator.sh` چلائیں
(زنگ ، ازگر بائنڈنگز ، جے ایس ، سوئفٹ) ایک ہی پاس میں ؛ CI نمونے لازمی طور پر نچوڑ کو منسلک کریں
اس مددگار کے علاوہ `matrix.md` پیدا (SDK/STATUS/دورانیہ کی میز) کو جاری کرنے والے ٹکٹ میں لاگ ان کریں تاکہ
جائزہ لینے والے مقامی طور پر سوٹ کو دوبارہ شروع کیے بغیر پیریٹی میٹرکس کا آڈٹ کرسکتے ہیں۔