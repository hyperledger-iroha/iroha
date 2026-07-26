---
lang: az
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# GOST Performans İş Akışı

Bu qeyd performans zərfini necə izlədiyimizi və tətbiq etdiyimizi sənədləşdirir
TC26 GOST imzalama arxa hissəsi.

## Yerli olaraq çalışır

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

Pərdə arxasında hər iki hədəf `scripts/gost_bench.sh`-ə zəng edir, hansı ki:

1. `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot` yerinə yetirir.
2. `gost_perf_check`-i `target/criterion`-ə qarşı işlədir, medianları təsdiqləyir
   yoxlanılmış baza (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. Markdown xülasəsini mövcud olduqda `$GITHUB_STEP_SUMMARY`-ə daxil edir.

Reqressiya/təkmilləşdirməni təsdiq etdikdən sonra baza xəttini yeniləmək üçün aşağıdakıları yerinə yetirin:

```bash
make gost-bench-update
```

və ya birbaşa:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` dəzgah + yoxlayıcısını işlədir, əsas JSON-un üzərinə yazır və çap edir
yeni medianlar. Həmişə qərar qeydinin yanında yenilənmiş JSON-u təhvil verin
`crates/iroha_crypto/docs/gost_backend.md`.

### Cari istinad medianları

| Alqoritm | Median (µs) |
|----------------------|-------------|
| ed25519 | 69.67 |
| gost256_paramset_a | 1136.96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133.25 |
| gost512_paramset_a | 8944.39 |
| gost512_paramset_b | 8963.60 |
| secp256k1 | 160.53 |

## CI

`.github/workflows/gost-perf.yml` eyni skriptdən istifadə edir və həmçinin dudect zamanlama qoruyucusunu idarə edir.
Ölçülmüş median konfiqurasiya edilmiş tolerantlıqdan daha çox əsas xətti keçdikdə CI uğursuz olur
(standart olaraq 20%) və ya vaxt qoruyucusu sızma aşkar etdikdə, reqressiyalar avtomatik olaraq tutulur.

## Xülasə çıxışı

`gost_perf_check` müqayisə cədvəlini yerli olaraq çap edir və eyni məzmunu əlavə edir
`$GITHUB_STEP_SUMMARY`, buna görə də CI iş qeydləri və icra xülasələri eyni nömrələri paylaşır.