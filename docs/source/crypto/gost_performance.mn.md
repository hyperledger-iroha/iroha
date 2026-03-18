---
lang: mn
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ГОСТ гүйцэтгэлийн ажлын урсгал

Энэхүү тэмдэглэл нь бидний гүйцэтгэлийн дугтуйг хэрхэн дагаж мөрдөж байгааг баримтжуулдаг
TC26 ГОСТ гарын үсэг зурах арын хэсэг.

## Орон нутагт гүйж байна

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

Хөшигний ард хоёулангийнх нь зорилтууд `scripts/gost_bench.sh` гэж нэрлэдэг бөгөөд үүнд:

1. `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`-г ажиллуулна.
2. `gost_perf_check`-г `target/criterion`-ийн эсрэг ажиллуулж, медиануудын эсрэг баталгаажуулна.
   бүртгэгдсэн суурь (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. Боломжтой үед Markdown хураангуйг `$GITHUB_STEP_SUMMARY`-д оруулна.

Регресс/сайжруулалтыг баталсны дараа суурь үзүүлэлтийг шинэчлэхийн тулд:

```bash
make gost-bench-update
```

эсвэл шууд:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` нь вандан + шалгагчийг ажиллуулж, үндсэн JSON-г дарж бичиж, хэвлэдэг
шинэ медианууд. Үргэлж шинэчлэгдсэн JSON-г шийдвэрийн бичлэгийн хажууд оруулаарай
`crates/iroha_crypto/docs/gost_backend.md`.

### Одоогийн лавлагааны медианууд

| Алгоритм | Медиан (µs) |
|----------------------|-------------|
| ed25519 | 69.67 |
| gost256_paramset_a | 1136.96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133.25 |
| gost512_paramset_a | 8944.39 |
| gost512_paramset_b | 8963.60 |
| secp256k1 | 160.53 |

## CI

`.github/workflows/gost-perf.yml` нь ижил скрипт ашигладаг бөгөөд мөн dudect timing guard-ыг ажиллуулдаг.
Хэмжсэн медиан нь тохируулсан хүлцэлээс илүү суурь үзүүлэлтээс хэтэрсэн тохиолдолд CI амжилтгүй болно
(Өгөгдмөлөөр 20%) эсвэл цагны хамгаалалт гоожиж байгааг илрүүлэх үед регресс автоматаар баригддаг.

## Дүгнэлт гаралт

`gost_perf_check` нь харьцуулах хүснэгтийг дотооддоо хэвлэж, ижил агуулгыг хавсаргана.
`$GITHUB_STEP_SUMMARY`, тиймээс CI ажлын бүртгэл болон гүйлтийн хураангуй нь ижил тоог хуваалцдаг.