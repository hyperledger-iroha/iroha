---
lang: uz
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2025-12-29T18:16:35.965528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 3 dastgoh to'plami

Iroha 3 o'rindiqli to'plam staking paytida biz ishonadigan issiq yo'llarni, haq
zaryadlash, isbotni tekshirish, rejalashtirish va isbotlash so'nggi nuqtalari. sifatida ishlaydi
Deterministik moslamalar bilan `xtask` buyrug'i (sobit urug'lar, sobit kalit materiallari,
va barqaror so'rov yuklari) shuning uchun natijalar xostlar bo'ylab takrorlanishi mumkin.

## To'plamni ishga tushirish

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

Bayroqlar:

- `--iterations` stsenariy namunasi bo'yicha iteratsiyalarni nazorat qiladi (standart: 64).
- `--sample-count` medianani hisoblash uchun har bir stsenariyni takrorlaydi (standart: 5).
- `--json-out|--csv-out|--markdown-out` chiqish artefaktlarini tanlang (barchasi ixtiyoriy).
- `--threshold` medianalarni asosiy chegaralar bilan solishtiradi (`--no-threshold` o'rnatilgan
  o'tkazib yuborish).
- `--flamegraph-hint` Markdown hisobotiga `cargo flamegraph` bilan izoh beradi
  stsenariyni profillash uchun buyruq.

CI elim `ci/i3_bench_suite.sh` da yashaydi va yuqoridagi yo'llarga sukut bo'yicha; o'rnatish
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` tungi vaqtda ishlash vaqtini sozlash uchun.

## Ssenariylar

- `fee_payer` / `fee_sponsor` / `fee_insufficient` - to'lovchi va homiy debeti
  va tanqislikni rad etish.
- `staking_bond` / `staking_slash` - bog'lanish/bog'lanish navbati bilan va bo'lmasdan
  kesish.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` -
  majburiyat sertifikatlari, JDG attestatsiyalari va ko'prik orqali imzolarni tekshirish
  isbotlangan foydali yuklar.
- `commit_cert_assembly` - sertifikatlar uchun dayjest yig'ilishi.
- `access_scheduler` - ziddiyatli kirishni rejalashtirish.
- `torii_proof_endpoint` - Axum isbotlangan so'nggi nuqtani tahlil qilish + tekshirish bo'ylab sayohat.

Har bir stsenariy iteratsiya uchun median nanosekundlarni, o'tkazish qobiliyatini va a
tez regressiyalar uchun deterministik ajratish hisoblagichi. Ostonalar yashaydi
`benchmarks/i3/thresholds.json`; apparat o'zgarganda u erda chegaralar paydo bo'ladi va
hisobot bilan birga yangi artefaktni bajaring.

## Nosozliklarni bartaraf etish

- Shovqinli regressiyalarning oldini olish uchun dalillarni to'plashda protsessor chastotasini/gubernatorini belgilang.
- Tadqiqotlar uchun `--no-threshold` dan foydalaning, so'ngra asosiy chiziq bo'lganda qayta yoqing.
  yangilangan.
- Bitta stsenariyni profillash uchun `--iterations 1` ni o'rnating va ostida qayta ishga tushiring.
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.