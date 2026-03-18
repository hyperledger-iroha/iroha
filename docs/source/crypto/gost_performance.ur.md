---
lang: ur
direction: rtl
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2026-01-03T18:07:57.084090+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# گوسٹ پرفارمنس ورک فلو

یہ نوٹ دستاویز کرتا ہے کہ ہم کس طرح کارکردگی کے لفافے کو ٹریک اور نافذ کرتے ہیں
TC26 GOST سائننگ بیکینڈ۔

## مقامی طور پر چل رہا ہے

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

پردے کے پیچھے دونوں اہداف `scripts/gost_bench.sh` پر کال کرتے ہیں ، جو:

1. `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot` پر عملدرآمد کرتا ہے۔
2. `gost_perf_check` کے خلاف `gost_perf_check` چلتا ہے ، جس میں میڈینوں کی تصدیق ہوتی ہے
   چیک ان بیس لائن (`crates/iroha_crypto/benches/gost_perf_baseline.json`)۔
3. جب دستیاب ہو تو `$GITHUB_STEP_SUMMARY` میں مارک ڈاون سمری کو انجیکشن لگاتا ہے۔

رجعت/بہتری کی منظوری کے بعد بیس لائن کو تازہ دم کرنے کے لئے ، چلائیں:

```bash
make gost-bench-update
```

یا براہ راست:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` بینچ + چیکر چلاتا ہے ، بیس لائن JSON ، اور پرنٹس کو اوور رائٹ کرتا ہے
نئے میڈینز۔ فیصلے کے ریکارڈ کے ساتھ ساتھ ہمیشہ تازہ ترین JSON کا ارتکاب کریں
`crates/iroha_crypto/docs/gost_backend.md`۔

### موجودہ حوالہ میڈین

| الگورتھم | میڈین (µs) |
| ------------------------ | ------------- |
| ED25519 | 69.67 |
| gost256_paramset_a | 1136.96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133.25 |
| gost512_paramset_a | 8944.39 |
| gost512_paramset_b | 8963.60 |
| SECP256K1 | 160.53 |

## CI

`.github/workflows/gost-perf.yml` ایک ہی اسکرپٹ کا استعمال کرتا ہے اور ڈڈکٹ ٹائمنگ گارڈ بھی چلاتا ہے۔
CI ناکام ہوجاتا ہے جب ناپنے والا میڈین تشکیل شدہ رواداری سے زیادہ بیس لائن سے زیادہ ہوجاتا ہے
(ڈیفالٹ کے لحاظ سے 20 ٪) یا جب ٹائمنگ گارڈ کسی لیک کا پتہ لگاتا ہے ، لہذا رجعتیں خود بخود پک جاتی ہیں۔

## خلاصہ آؤٹ پٹ

`gost_perf_check` موازنہ ٹیبل کو مقامی طور پر پرنٹ کرتا ہے اور اسی مواد کو شامل کرتا ہے
`$GITHUB_STEP_SUMMARY` ، لہذا CI ملازمت اور رن سمریوں کو ایک ہی نمبروں کا اشتراک کرتے ہیں۔