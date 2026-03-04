---
lang: ur
direction: rtl
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2026-01-03T18:07:57.759135+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# خفیہ گیس انشانکن بیس لائنز

یہ لیجر خفیہ گیس انشانکن کے توثیق شدہ نتائج کو ٹریک کرتا ہے
بینچ مارک۔ ہر صف میں رہائی کے معیار کی پیمائش کی دستاویز کی گئی ہے جس کے ساتھ پکڑا گیا ہے
`docs/source/confidential_assets.md#calibration-baselines--acceptance-gates` میں بیان کردہ طریقہ کار۔

| تاریخ (UTC) | ارتکاب | پروفائل | `ns/op` | `gas/op` | `ns/gas` | نوٹ |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3C70A7D3 | بیس لائن نیون | 2.93E5 | 1.57E2 | 1.87e3 | ڈارون 25.0.0 ARM64E (ہوسٹینفو) ؛ `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018` ؛ `cargo test -p iroha_core bench_repro -- --ignored` ؛ `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5` ؛ `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8EA9B2A7 | بیس لائن نیون -20260428 | 4.29e6 | 1.57E2 | 2.73E4 | ڈارون 25.0.0 ARM64 (`rustc 1.91.0`)۔ کمانڈ: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428` ؛ `docs/source/confidential_assets_calibration_neon_20260428.log` پر لاگ ان کریں۔ x86_64 پیریٹی رنز (سم ڈی نیوٹرل + اے وی ایکس 2) 2026-03-19 زیورک لیب سلاٹ کے لئے شیڈول ہیں۔ نوادرات `artifacts/confidential_assets_calibration/2026-03-x86/` کے تحت مماثل کمانڈز کے ساتھ اتریں گے اور ایک بار پکڑے جانے کے بعد بیس لائن ٹیبل میں ضم ہوجائیں گے۔ |
| 2026-04-28 | - | بیس لائن سمڈ غیر جانبدار | - | - | - | ** معاف کیا گیا ** ایپل سلیکن پر - `ring` پلیٹ فارم ABI کے لئے نیین کو نافذ کرتا ہے ، لہذا `RUSTFLAGS="-C target-feature=-neon"` بینچ چلانے سے پہلے ہی ناکام ہوجاتا ہے (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`)۔ غیر جانبدار اعداد و شمار CI میزبان `bench-x86-neon0` پر گیٹڈ رہتا ہے۔ |
| 2026-04-28 | - | بیس لائن-ای وی ایکس 2 | - | - | - | ** موخر ** جب تک ایک x86_64 رنر دستیاب نہ ہو۔ `arch -x86_64` اس مشین پر بائنریز نہیں بنا سکتا ("ایگزیکٹو میں خراب سی پی یو کی قسم" I `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log` دیکھیں)۔ CI میزبان `bench-x86-avx2a` ریکارڈ کا ذریعہ بنی ہوئی ہے۔ |

`ns/op` معیار کے ذریعہ ماپنے والی میڈین وال گھڑی فی ہدایت کو جمع کرتا ہے۔
`gas/op` سے متعلقہ شیڈول کے اخراجات کا ریاضی کا مطلب ہے
`iroha_core::gas::meter_instruction` ؛ `ns/gas` سمیت نانو سیکنڈ کو تقسیم کرتا ہے
نو-انسٹرکشن نمونے کے سیٹ میں سمیت گیس۔

* نوٹ۔* موجودہ ARM64 میزبان معیار کو خارج نہیں کرتا ہے
باکس ؛ `CRITERION_OUTPUT_TO=csv` کے ساتھ دوبارہ چلائیں یا ٹیگ کرنے سے پہلے ایک اپ اسٹریم فکس
جاری کریں تاکہ قبولیت چیک لسٹ کے ذریعہ درکار نوادرات منسلک ہوں۔
اگر `target/criterion/` `--save-baseline` کے بعد بھی غائب ہے تو ، رن جمع کریں
لینکس کے میزبان پر یا کنسول آؤٹ پٹ کو ریلیز کے بنڈل میں ایک کے طور پر سیریلائز کریں
عارضی اسٹاپ گیپ۔ حوالہ کے لئے ، ARM64 کنسول تازہ ترین رن سے لاگ ان کریں
`docs/source/confidential_assets_calibration_neon_20251018.log` پر رہتا ہے۔

ایک ہی رن (`cargo bench -p iroha_core --bench isi_gas_calibration`) سے فی انسٹرکشن میڈینز:

| ہدایات | میڈین `ns/op` | شیڈول `gas` | `ns/gas` |
| --- | --- | --- | --- |
| رجسٹر ڈومین | 3.46e5 | 200 | 1.73e3 |
| رجسٹر اکاؤنٹ | 3.15E5 | 200 | 1.58e3 |
| رجسٹراسیٹ ڈیف | 3.41E5 | 200 | 1.71e3 |
| setaccountkv_small | 3.28E5 | 67 | 4.90e3 |
| گرانٹ ایککونٹرول | 3.33E5 | 96 | 3.47E3 |
| ریووکی کوکونٹرول | 3.12e5 | 96 | 3.25E3 |
| exectetrigger_empty_args | 1.42E5 | 224 | 6.33e2 |
| منٹاسیٹ | 1.56e5 | 150 | 1.04E3 |
| ٹرانسفراسیٹ | 3.68E5 | 180 | 2.04E3 |

### 2026-04-28 (ایپل سلیکن ، نیین فعال)

2026-04-28 ریفریش (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`) کے لئے میڈین لیٹینسیز:| ہدایات | میڈین `ns/op` | شیڈول `gas` | `ns/gas` |
| --- | --- | --- | --- |
| رجسٹر ڈومین | 8.58e6 | 200 | 4.29e4 |
| رجسٹر اکاؤنٹ | 4.40e6 | 200 | 2.20e4 |
| رجسٹراسیٹ ڈیف | 4.23e6 | 200 | 2.12E4 |
| setaccountkv_small | 3.79e6 | 67 | 5.66e4 |
| گرانٹ ایککونٹرول | 3.60E6 | 96 | 3.75E4 |
| ریووکی کوکونٹرول | 3.76e6 | 96 | 3.92E4 |
| exectetrigger_empty_args | 2.71E6 | 224 | 1.21E4 |
| منٹاسیٹ | 3.92e6 | 150 | 2.61E4 |
| ٹرانسفراسیٹ | 3.59e6 | 180 | 1.99e4 |

`ns/op` اور `ns/gas` مذکورہ جدول میں مجموعی کے مجموعی سے اخذ کیا گیا ہے
یہ میڈین (کل `3.85717e7`ns نو انسٹرکشن سیٹ اور 1،413 میں
گیس یونٹ)۔

شیڈول کالم `gas::tests::calibration_bench_gas_snapshot` کے ذریعہ نافذ کیا گیا ہے
(نو-انسٹرکشن سیٹ میں کل 1،413 گیس) اور اگر مستقبل کے پیچ ہوں تو سفر کریں گے
انشانکن فکسچر کو اپ ڈیٹ کیے بغیر پیمائش کو تبدیل کریں۔

## عزم ٹری ٹیلی میٹری ثبوت (M2.2)

فی روڈ میپ ٹاسک ** M2.2 ** ، ہر انشانکن رن کو نیا پر قبضہ کرنا ہوگا
مرکل فرنٹیئر کے قیام کو ثابت کرنے کے لئے عزم-ٹری گیجز اور بے دخلی کاؤنٹرز
تشکیل شدہ حدود میں:

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

انشانکن کام کے بوجھ سے پہلے اور اس کے بعد فوری طور پر اقدار کو ریکارڈ کریں۔ a
ہر اثاثہ میں سنگل کمانڈ کافی ہے۔ `xor#wonderland` کے لئے مثال:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

خام آؤٹ پٹ (یا Prometheus اسنیپ شاٹ) کو انشانکن ٹکٹ سے منسلک کریں
گورننس کا جائزہ لینے والا روٹ ہسٹری کیپس اور چوکی کے وقفوں کی تصدیق کرسکتا ہے
اعزاز `docs/source/telemetry.md#confidential-tree-telemetry-m22` میں ٹیلی میٹری گائیڈ
توقعات اور اس سے وابستہ Grafana پینلز کو آگاہ کرنے پر توسیع کرتا ہے۔

ایک ہی کھرچنے میں تصدیق کنندہ کیشے کاؤنٹرز کو شامل کریں تاکہ جائزہ لینے والے تصدیق کرسکیں
مس تناسب 40 ٪ انتباہی دہلیز سے نیچے رہا:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

انشانکن نوٹ کے اندر اخذ کردہ تناسب (`miss / (hit + miss)`) کی دستاویز کریں
سم ڈی غیر جانبدار لاگت ماڈلنگ کی مشقوں کو ظاہر کرنے کے لئے
ہالو 2 تصدیق کنندہ رجسٹری کو پھینکنا۔

## غیر جانبدار اور اے وی ایکس 2 چھوٹ

ایس ڈی کے کونسل نے فاسیک گیٹ کی ضرورت کے لئے عارضی چھوٹ دی
`baseline-simd-neutral` اور `baseline-avx2` پیمائش:

۔
  ابی درستگی۔ خصوصیت کو غیر فعال کرنا (`RUSTFLAGS="-C target-feature=-neon"`)
  بینچ بائنری تیار ہونے سے پہلے تعمیر کو ختم کرتا ہے (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`)۔
- ** اے وی ایکس 2: ** مقامی ٹولچین x86_64 بائنریز (`arch -x86_64 rustc -V`
  → "برے سی پی یو کی قسم قابل عمل" ؛ دیکھو
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`)۔

جب تک کہ CI `bench-x86-neon0` اور `bench-x86-avx2a` کی میزبانی نہیں کرتا ہے ، نیین رن
اوپر کے علاوہ ٹیلی میٹری شواہد فاسیک قبولیت کے معیار کو پورا کرتے ہیں۔
چھوٹ `status.md` میں ریکارڈ کی گئی ہے اور ایک بار x86 ہارڈ ویئر ہونے کے بعد اس پر نظرثانی کی جائے گی
دستیاب ہے۔