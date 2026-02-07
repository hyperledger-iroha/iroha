---
lang: ur
direction: rtl
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2026-01-03T18:08:01.692664+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 3 بینچ سویٹ

Iroha 3 بینچ سویٹ کے اوقات گرم راستوں پر ہم اسٹیکنگ ، فیس کے دوران انحصار کرتے ہیں
چارجنگ ، پروف کی توثیق ، ​​نظام الاوقات ، اور پروف اختتامی نکات۔ یہ ایک کے طور پر چلتا ہے
`xtask` کمانڈ ڈٹرمینسٹک فکسچر (فکسڈ بیج ، فکسڈ کلیدی مواد ،
اور مستحکم درخواست پے لوڈز) لہذا نتائج میزبانوں میں تولیدی ہیں۔

## سویٹ چل رہا ہے

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

جھنڈے:

- `--iterations` ہر منظر نامے کے نمونے (پہلے سے طے شدہ: 64) تکرار کو کنٹرول کرتا ہے۔
- `--sample-count` میڈین (پہلے سے طے شدہ: 5) کی گنتی کے لئے ہر منظر کو دہراتا ہے۔
- `--json-out|--csv-out|--markdown-out` آؤٹ پٹ نمونے (تمام اختیاری) کا انتخاب کریں۔
- `--threshold` میڈینوں کا موازنہ بیس لائن حد سے زیادہ کرتا ہے (`--no-threshold` سیٹ کریں
  چھوڑنے کے لئے)۔
- `--flamegraph-hint` `cargo flamegraph` کے ساتھ مارک ڈاون رپورٹ کی تشریح کرتا ہے
  کسی منظر نامے کی پروفائل کرنے کا حکم دیں۔

CI گلو `ci/i3_bench_suite.sh` میں رہتا ہے اور مذکورہ بالا راستوں سے پہلے سے طے شدہ ہے۔ سیٹ
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` نائٹ لیز میں رن ٹائم ٹیون کرنے کے لئے۔

## منظرنامے

- `fee_payer` / `fee_sponsor` / `fee_insufficient` - ادائیگی کرنے والا بمقابلہ اسپانسر ڈیبٹ
  اور کمی کو مسترد کرنا۔
- `staking_bond` / `staking_slash` - بانڈ / انبونڈ قطار کے ساتھ اور بغیر
  سلیشنگ
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` -
  سکیورٹی کی توثیق سرٹیفکیٹ ، جے ڈی جی کی تصدیقوں ، اور پل پر
  پروف پے لوڈ۔
- `commit_cert_assembly` - کمٹمنٹ سرٹیفکیٹ کے لئے اسمبلی کو ہضم کریں۔
-`access_scheduler`-تنازعات سے آگاہ رسائی سیٹ شیڈولنگ۔
- `torii_proof_endpoint` - AXUM پروف اینڈ پوائنٹ پارسنگ + تصدیقی گول سفر۔

ہر منظر نامے میں میڈین نانو سیکنڈ فی تکرار ، تھروپپٹ ، اور ایک کو ریکارڈ کیا جاتا ہے
فوری دباؤ کے ل detter تعصب الاٹمنٹ کاؤنٹر۔ دہلیز میں رہتے ہیں
`benchmarks/i3/thresholds.json` ؛ جب ہارڈ ویئر میں تبدیلی آتی ہے اور
ایک رپورٹ کے ساتھ ساتھ نئے نمونے کا ارتکاب کریں۔

## خرابیوں کا سراغ لگانا

- شور کے دباؤ سے بچنے کے لئے ثبوت اکٹھا کرتے وقت سی پی یو فریکوینسی/گورنر کو پن کریں۔
- ریسرچ رنز کے لئے `--no-threshold` استعمال کریں ، پھر بیس لائن ہونے کے بعد دوبارہ قابل ہوجائیں
  تازہ دم
- ایک ہی منظر نامے کی پروفائل کرنے کے لئے ، `--iterations 1` سیٹ کریں اور اس کے تحت دوبارہ چلائیں
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`۔