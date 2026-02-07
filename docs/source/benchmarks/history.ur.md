---
lang: ur
direction: rtl
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2026-01-03T18:08:00.425813+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# جی پی یو بینچ مارک کیپچر ہسٹری (فاسٹ پی کیو ڈبلیو پی 5-بی)

یہ فائل `python3 scripts/fastpq/update_benchmark_history.py` کے ذریعہ تیار کی گئی ہے۔
یہ فاسٹ پی کیو اسٹیج 7 WP5-B کو ہر لپیٹے ہوئے GPU کا سراغ لگا کر نجات دیتا ہے
بینچ مارک آرٹ فیکٹ ، پوسیڈن مائکروبینچ مینی فیسٹ ، اور معاون جھاڑو کے تحت
`benchmarks/`۔ بنیادی گرفتاریوں کو اپ ڈیٹ کریں اور اسکرپٹ کو دوبارہ شروع کریں جب بھی کوئی نیا
بنڈل لینڈز یا ٹیلی میٹری کو تازہ شواہد کی ضرورت ہے۔

## دائرہ کار اور اپ ڈیٹ کا عمل

- نئے GPU کیپچر (`scripts/fastpq/wrap_benchmark.py` کے ذریعے) تیار کریں یا لپیٹیں ،
  انہیں کیپچر میٹرکس میں شامل کریں ، اور اس جنریٹر کو دوبارہ تازہ کرنے کے لئے دوبارہ چلائیں
  میزیں
- جب پوسیڈن مائکروبینچ ڈیٹا موجود ہے تو ، اس کے ساتھ برآمد کریں
  `scripts/fastpq/export_poseidon_microbench.py` استعمال کرکے مینی فیسٹ کو دوبارہ تعمیر کریں
  `scripts/fastpq/aggregate_poseidon_microbench.py`۔
- ریکارڈ مرکل کی دہلیز کو ان کے JSON آؤٹ پٹ کے تحت اسٹور کرکے ریکارڈ کریں
  `benchmarks/merkle_threshold/` ؛ یہ جنریٹر معروف فائلوں کو لسٹ کرتا ہے تاکہ آڈٹ ہوں
  کراس ریفرنس سی پی یو بمقابلہ جی پی یو کی دستیابی کر سکتے ہیں۔

## فاسٹ پی کیو اسٹیج 7 جی پی یو بینچ مارک

| بنڈل | پسدید | موڈ | GPU پسدید | GPU دستیاب | ڈیوائس کلاس | GPU | LDE MS (CPU/GPU/SU) | پوسیڈن ایم ایس (سی پی یو/جی پی یو/ایس یو) |
| ------- | --------- | -------- | ------------- | --------------- | ---------------- | ---------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | CUDA | GPU | CUDA-SM80 | ہاں | xeon-rtx | Nvidia RTX 6000 ADA | 1512.9/880.7/1.72 | -/ -/ - |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | دھات | GPU | کوئی نہیں | ہاں | ایپل-ایم 4 | ایپل جی پی یو 40 کور | 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | دھات | GPU | دھات | ہاں | ایپل-ایم 2-الٹرا | ایپل ایم 2 الٹرا | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | دھات | GPU | دھات | ہاں | ایپل-ایم 2-الٹرا | ایپل ایم 2 الٹرا | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | دھات | GPU | دھات | ہاں | ایپل-ایم 2-الٹرا | ایپل ایم 2 الٹرا | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | اوپن سی ایل | GPU | اوپن سی ایل | ہاں | neverse-mi300 | AMD جبلت MI300A | 4518.5/688.9/6.56 | 2780.4/905.6/3.07 |

> کالم: `Backend` بنڈل کے نام سے ماخوذ ہے۔ `Mode`/`GPU backend`/`GPU available`
> سی پی یو فال بیکس یا گمشدہ جی پی یو کو بے نقاب کرنے کے لئے لپیٹے `benchmarks` بلاک سے کاپی کی گئی ہے۔
> دریافت (مثال کے طور پر ، `gpu_backend=none` کے باوجود `Mode=gpu`)۔ ایس یو = اسپیڈ اپ تناسب (سی پی یو/جی پی یو)۔

## پوسیڈن مائکروبینچ اسنیپ شاٹس

`benchmarks/poseidon/manifest.json` پہلے سے طے شدہ-VS-اسکلر پوسیڈن کو جمع کرتا ہے
مائکروبینچ ہر دھات کے بنڈل سے برآمد ہوتا ہے۔ نیچے دیئے گئے جدول کو تازہ دم کیا گیا ہے
جنریٹر اسکرپٹ ، لہذا سی آئی اور گورننس کے جائزے تاریخی اسپیڈ اپ کو مختلف کرسکتے ہیں
لپیٹے ہوئے فاسٹ پی کیو رپورٹس کو کھولے بغیر۔

| خلاصہ | بنڈل | ٹائم اسٹیمپ | پہلے سے طے شدہ ایم ایس | اسکیلر ایم ایس | اسپیڈ اپ |
| --------- | -------- | ----------- | -------------- | ----------- | --------- |
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06: 11: 01Z | 2167.7 | 2152.2 | 0.99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06: 04: 07Z | 1990.5 | 1994.5 | 1.00 |

## مرکل کی دہلیز جھاڑوحوالہ کی گرفتاری جمع کروائی گئی
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
`benchmarks/merkle_threshold/` کے تحت براہ راست رہیں۔ فہرست اندراجات سے پتہ چلتا ہے کہ میزبان
جب سویپ بھاگتا ہے تو دھات کے آلات بے نقاب ؛ جی پی یو سے چلنے والے کیپچرز کو رپورٹ کرنا چاہئے
`metal_available=true`۔

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` - `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` - `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` - `metal_available=True`

ایپل سلیکن کیپچر (`takemiyacStudio.lan_25.0.0_arm64`) `docs/source/benchmarks.md` میں استعمال ہونے والی کیننیکل GPU بیس لائن ہے۔ میک او ایس 14 اندراجات ماحول کے لئے صرف سی پی یو صرف بیس لائنوں کے طور پر باقی ہیں جو دھات کے آلات کو بے نقاب نہیں کرسکتے ہیں۔

## قطار-یوزج سنیپ شاٹس

`scripts/fastpq/check_row_usage.py` کے ذریعے پکڑے گئے گواہ ڈیکوڈس منتقلی کو ثابت کرتے ہیں
گیجٹ کی قطار کی کارکردگی۔ `artifacts/fastpq_benchmarks/` کے تحت JSON نوادرات رکھیں
اور یہ جنریٹر آڈیٹرز کے لئے ریکارڈ شدہ منتقلی کے تناسب کا خلاصہ کرے گا۔

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` - بیچز = 2 ، ٹرانسفر_راٹیو اوسط = 0.629 (کم سے کم = 0.625 ، زیادہ سے زیادہ = 0.633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` - بیچز = 2 ، ٹرانسفر_راٹیو اوسط = 0.619 (کم سے کم = 0.613 ، زیادہ سے زیادہ = 0.625)