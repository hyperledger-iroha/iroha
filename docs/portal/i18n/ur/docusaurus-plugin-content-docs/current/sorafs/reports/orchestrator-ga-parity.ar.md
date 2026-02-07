---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

فارمیٹر SoraFS کے لئے # GA پیریٹی رپورٹ

اب ہر ایس ڈی کے کی ناگزیر ملٹی بازیافت کی برابری کا پتہ لگایا جارہا ہے تاکہ ریلیز انجینئر اس کی تصدیق کرسکیں
پے لوڈ بائٹس ، حص conk ے کی رسیدیں ، فراہم کنندہ کی رپورٹیں ، اور اسکور بورڈ اسکور ایک جیسے ہی رہتے ہیں
درخواستیں ہر کنٹرول ذیل میں معیاری ملٹی فراہم کرنے والا پیکٹ کھاتا ہے
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/` ، جو SF1 پلان اور ڈیٹا اکٹھا کرتا ہے
فراہم کنندہ میٹا اور اسنیپ شاٹ ٹیلی میٹری اور آرکسٹریٹر کے اختیارات۔

## زنگ آلود میں بیس لائن

- ** کمانڈ: ** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
۔
  جمع شدہ پے لوڈ بائٹس ، حصہ رسیدیں ، فراہم کنندہ کی رپورٹیں ، اور اسکور بورڈ اسکور سے۔ جیسا کہ
  میٹرکس ٹریک چوٹی متوازی اور موثر ورکنگ سیٹ سائز (`max_parallel x max_chunk_length`) کو ٹریک کرتا ہے۔
- ** کارکردگی میں رکاوٹ: ** ہر رن کو سی آئی ہارڈ ویئر پر 2 سیکنڈ کے اندر مکمل کرنا ضروری ہے۔
- ** کام کرنے کی حد کی چھت: ** SF1 پروفائل کے ساتھ استعمال `max_parallel = 3` ، جس کے نتیجے میں ہوتا ہے
  ونڈو <= 196608 بائٹس۔

مثال کے طور پر لاگ آؤٹ پٹ:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## جاوا اسکرپٹ ایس ڈی کے کی کنٹرول

- ** کمانڈ: ** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- ** دائرہ کار: ** `iroha_js_host::sorafsMultiFetchLocal` کے ذریعے اسی حقیقت کو دوبارہ تیار کرتا ہے ، اور موازنہ کرتا ہے
  پے لوڈز ، وصول ، فراہم کنندہ کی رپورٹیں ، اور اسکور بورڈ اسنیپ شاٹس لگاتار دو رنز میں۔
- ** کارکردگی میں رکاوٹ: ** ہر عملدرآمد کو 2 s کے اندر ختم ہونا چاہئے۔ کنٹرول پیمائش کی مدت اور چھت پرنٹ کرتا ہے
  محفوظ بائٹس (`max_parallel = 3` ، `peak_reserved_bytes <= 196608`)۔

مثال کے طور پر خلاصہ لائن:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## سوئفٹ ایس ڈی کے کے لئے استعمال

- ** کمانڈ: ** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- ** رینج: ** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` میں بیان کردہ پیریٹی گروپ پر قبضہ کرتا ہے ،
  حقیقت کے ساتھ SF1 برج Norito (`sorafsLocalFetch`) کے ذریعے دو بار دوبارہ شروع ہوا۔ چیک کریں
  اسی کا استعمال کرتے ہوئے پے لوڈ بائٹس ، حص ch ے کی رسیدیں ، فراہم کنندہ کی رپورٹیں ، اور اسکور بورڈ اندراجات
  فراہم کنندہ لازمی میٹا ڈیٹا اور ٹیل میٹری اسنیپ شاٹس آف مورچا/جے ایس پیکیجز۔
- ** پل کی تشکیل: ** ڈیمانڈ اور بوجھ پر کنٹرول ڈیکمپریس `dist/NoritoBridge.xcframework.zip`
  `dlopen` کے ذریعے میکوس چپ۔ جب XCFramework غیر حاضر ہے یا SoraFS لنکس سے محروم ہے ، تو یہ واپس آجاتا ہے
  `cargo build -p connect_norito_bridge --release` اور اس کے ساتھ جڑتا ہے
  `target/release/libconnect_norito_bridge.dylib` ، لہذا CI میں کسی بھی دستی سیٹ اپ کی ضرورت نہیں ہے۔
- ** کارکردگی میں رکاوٹ: ** ہر عمل کو سی آئی ہارڈ ویئر پر 2 سیکنڈ کے اندر ختم کرنا ہوگا۔ کنٹرول مدت پرنٹ کرتا ہے
  ماپا اور چھت محفوظ بائٹس (`max_parallel = 3` ، `peak_reserved_bytes <= 196608`)۔

مثال کے طور پر خلاصہ لائن:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## ازگر بائنڈنگ کے لئے استعمال

- ** کمانڈ: ** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Scope:** Exercising high-level wrapper `iroha_python.sorafs.multi_fetch_local` and dataclasses
  ٹائپ پر پابندی ہے تاکہ معیاری حقیقت اسی انٹرفیس سے گزرتی ہے جو صارفین کے ذریعہ استعمال ہوتا ہے
  پہیے پہیے ٹیسٹ فراہم کنندہ میٹا ڈیٹا کو `providers.json` سے دوبارہ تعمیر کرتا ہے ، اور انجیکشن
  ٹیلی میٹری اسنیپ شاٹ ، پے لوڈ بائٹس ، چنک رسیدیں ، فراہم کنندہ کی رپورٹیں ، اور مواد کی جانچ پڑتال کریں
  اسکور بورڈ جیسے مورچا/جے ایس/سوئفٹ پیکیجز۔
- ** شرط: ** `maturin develop --release` (یا پہیے انسٹال کریں) `_crypto` بائنڈنگ کو ظاہر کرنے کے لئے چلائیں
  `sorafs_multi_fetch_local` pytest چلانے سے پہلے ؛ جب یہ جاری نہیں ہے تو خود بخود استعمال کو چھوڑ دیتا ہے
  لنکنگ دستیاب ہے۔
- ** کارکردگی کی رکاوٹ: ** وہی <= 2s کو مورچا پیکیج کی طرح حد تک ؛ پیٹس نے جمع کردہ بائٹس کی تعداد کو ریکارڈ کیا
  اور ریلیز کے ٹکڑے میں فراہم کنندگان کے حصہ کا خلاصہ۔ریلیز کی منظوری پائپ لائن کو ہر استعمال (زنگ ، ازگر ، جے ایس ، سوئفٹ) سے ڈائجسٹ آؤٹ پٹ پر قبضہ کرنا چاہئے۔
لہذا آرکائیوڈ رپورٹ اپ گریڈ کرنے سے پہلے یکساں طور پر پے لوڈ کی رسیدوں اور میٹرکس کا موازنہ کرسکتی ہے
تعمیر. تعمیر. تمام مساوات سیٹ (زنگ اور ازگر) کو نافذ کرنے کے لئے `ci/sdk_sorafs_orchestrator.sh` چلائیں
ایک پاس میں پابندیاں ، جے ایس ، اور سوئفٹ) ؛ اس سے لاگ ان ٹکڑا CI آؤٹ پٹ کے ساتھ منسلک ہونا چاہئے
ہیلپر پلس تیار کردہ `matrix.md` فائل (SDK ٹیبل/حیثیت/دورانیہ) رہائی کے ٹکٹ کے ساتھ
تاکہ جائزہ لینے والے مقامی طور پر مجموعہ کو دوبارہ شروع کیے بغیر مساوات میٹرکس کا آڈٹ کرسکیں۔