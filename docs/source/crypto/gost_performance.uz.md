---
lang: uz
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# GOST ishlash jarayoni

Ushbu eslatma biz uchun ishlash konvertini qanday kuzatishimiz va amalga oshirishimizni hujjatlashtiradi
TC26 GOST imzolash orqa tomoni.

## Mahalliy ravishda ishlaydi

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

Sahna ortida ikkala nishon ham `scripts/gost_bench.sh` ni chaqiradi, bu:

1. `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot` ni bajaradi.
2. `gost_perf_check` ni `target/criterion` ga qarshi ishga tushiradi, medianalarni `target/criterion`ga qarshi tekshiradi.
   ro'yxatdan o'tgan asosiy chiziq (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. Agar mavjud bo'lsa, Markdown xulosasini `$GITHUB_STEP_SUMMARY` ichiga kiritadi.

Regressiya/yaxshilanishni ma'qullagandan so'ng asosiy chiziqni yangilash uchun quyidagilarni bajaring:

```bash
make gost-bench-update
```

yoki to'g'ridan-to'g'ri:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` dastgoh + tekshirgichni ishga tushiradi, asosiy JSON-ning ustiga yozadi va chop etadi
yangi medianlar. Har doim yangilangan JSON-ni qaror yozuvi bilan birga bajaring
`crates/iroha_crypto/docs/gost_backend.md`.

### Joriy mos yozuvlar medianalari

| Algoritm | Median (µs) |
|----------------------|-------------|
| ed25519 | 69.67 |
| gost256_paramset_a | 1136.96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133.25 |
| gost512_paramset_a | 8944.39 |
| gost512_paramset_b | 8963.60 |
| secp256k1 | 160.53 |

## CI

`.github/workflows/gost-perf.yml` xuddi shu skriptdan foydalanadi va shuningdek, dudect timing guardni ishlaydi.
O'lchangan o'rtacha qiymat sozlangan tolerantlikdan ko'proq asosiy chiziqdan oshib ketganda, CI bajarilmaydi
(Sukut bo'yicha 20%) yoki vaqt qo'riqchisi qochqinni aniqlaganda, regressiyalar avtomatik ravishda ushlanadi.

## Xulosa chiqish

`gost_perf_check` taqqoslash jadvalini mahalliy sifatida chop etadi va bir xil tarkibni qo'shadi
`$GITHUB_STEP_SUMMARY`, shuning uchun CI ish jurnallari va ishga tushirish xulosalari bir xil raqamlarga ega.