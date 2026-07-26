---
lang: am
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# GOST አፈጻጸም የስራ ፍሰት

ይህ ማስታወሻ የአፈጻጸም ፖስታውን እንዴት እንደምንከታተል እና እንደምናስፈጽም ያሳያል
TC26 GOST ፊርማ ጀርባ።

## በአገር ውስጥ በመሮጥ ላይ

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

ከትዕይንቱ በስተጀርባ ሁለቱም ኢላማዎች `scripts/gost_bench.sh` ብለው ይጠሩታል፡

1. `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot` ያስፈጽማል.
2. `gost_perf_check`ን ከ `target/criterion` ጋር ያካሂዳል፣ ሚድያዎችን በ
   ተመዝግቦ የገባ መነሻ መስመር (`crates/iroha_crypto/benches/gost_perf_baseline.json`)።
3. ማርክዳውን ማጠቃለያ ሲገኝ ወደ `$GITHUB_STEP_SUMMARY` ያስገባል።

መሻሻል/ማሻሻያ ካፀደቀ በኋላ የመነሻ መስመሩን ለማደስ፣ ያሂዱ፡-

```bash
make gost-bench-update
```

ወይም በቀጥታ፡-

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` አግዳሚ ወንበር + ቼክን ያስኬዳል፣ የመነሻ መስመርን JSON ይደግማል እና ያትማል።
አዲሱ ሚድያዎች. ሁልጊዜ የተሻሻለውን JSON ከውሳኔው መዝገብ ጋር አስገባ
`crates/iroha_crypto/docs/gost_backend.md`.

### የአሁን የማጣቀሻ ሚድያዎች

| አልጎሪዝም | ሚዲያን (µs) |
|------------------|-----------|
| ed25519 | 69.67 |
| gost256_paramset_a | 1136.96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133.25 |
| gost512_paramset_a | 8944.39 |
| gost512_paramset_b | 8963.60 |
| ሰከንድ256k1 | 160.53 |

## CI

`.github/workflows/gost-perf.yml` ተመሳሳዩን ስክሪፕት ይጠቀማል እና እንዲሁም የዱዴት ጊዜ ጥበቃን ይሰራል።
የሚለካው ሚዲያን ከተዋቀረው መቻቻል በላይ ከመነሻው ሲያልፍ CI አይሳካም።
(በነባሪ 20%) ወይም የጊዜ ጠባቂው መፍሰስን ሲያገኝ፣ስለዚህ መመለሻዎች በራስ-ሰር ይያዛሉ።

## ማጠቃለያ ውጤት

`gost_perf_check` የንፅፅር ሠንጠረዡን በአገር ውስጥ ያትማል እና ተመሳሳይ ይዘትን በ
`$GITHUB_STEP_SUMMARY`፣ስለዚህ CI የስራ ምዝግብ ማስታወሻዎች እና አሂድ ማጠቃለያዎች ተመሳሳይ ቁጥሮች ይጋራሉ።