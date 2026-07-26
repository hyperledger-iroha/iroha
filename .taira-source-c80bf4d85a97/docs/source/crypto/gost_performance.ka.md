---
lang: ka
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# GOST შესრულების სამუშაო პროცესი

ეს ჩანაწერი ადასტურებს, თუ როგორ ვადევნებთ თვალყურს და ვახორციელებთ შესრულების კონვერტს
TC26 GOST ხელმოწერის უკანა ნაწილი.

## ლოკალურად სირბილი

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

კულისებში ორივე სამიზნე უწოდებს `scripts/gost_bench.sh`, რომელიც:

1. ახორციელებს `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`.
2. აწარმოებს `gost_perf_check`-ს `target/criterion`-ის წინააღმდეგ, ამოწმებს მედიანებს
   რეგისტრირებული საბაზისო (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. შემოაქვს Markdown-ის შეჯამება `$GITHUB_STEP_SUMMARY`-ში, როცა ეს ხელმისაწვდომია.

რეგრესიის/გაუმჯობესების დამტკიცების შემდეგ საბაზისო ხაზის გასაახლებლად, გაუშვით:

```bash
make gost-bench-update
```

ან პირდაპირ:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` აწარმოებს bench + checker-ს, გადაწერს საბაზისო JSON-ს და ბეჭდავს
ახალი მედიანები. ყოველთვის ჩააბარეთ განახლებული JSON გადაწყვეტილების ჩანაწერთან ერთად
`crates/iroha_crypto/docs/gost_backend.md`.

### აქტუალური მითითების მედიანები

| ალგორითმი | მედიანა (µs) |
|---------------------|-------------|
| ed25519 | 69.67 |
| gost256_paramset_a | 1136.96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133.25 |
| gost512_paramset_a | 8944.39 |
| gost512_paramset_b | 8963.60 |
| secp256k1 | 160.53 |

## CI

`.github/workflows/gost-perf.yml` იყენებს იმავე სკრიპტს და ასევე ამუშავებს dudect-ის დროის დაცვას.
CI ვერ ხერხდება, როდესაც გაზომილი მედიანა აღემატება საწყის ხაზს კონფიგურირებულ ტოლერანტობაზე მეტით
(ნაგულისხმევად 20%) ან როდესაც დროის დამცავი აღმოაჩენს გაჟონვას, ასე რომ, რეგრესია დაფიქსირებულია ავტომატურად.

## შემაჯამებელი გამომავალი

`gost_perf_check` ბეჭდავს შედარების ცხრილს ადგილობრივად და ამატებს იმავე შინაარსს
`$GITHUB_STEP_SUMMARY`, ასე რომ, CI სამუშაო ჟურნალები და გაშვებული შეჯამებები იზიარებს იგივე ნომრებს.