---
lang: uz
direction: ltr
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2025-12-29T18:16:35.920451+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# GPU Benchmark Capture tarixi (FASTPQ WP5-B)

Ushbu fayl `python3 scripts/fastpq/update_benchmark_history.py` tomonidan yaratilgan.
U har bir oʻralgan GPUni kuzatish orqali yetkazib beriladigan FASTPQ Stage 7 WP5-B ni qondiradi
benchmark artefakt, Poseidon mikrobench manifest va yordamchi tekshiruvlar.
`benchmarks/`. Asosiy tasvirlarni yangilang va har safar yangisi paydo bo'lganda skriptni qayta ishga tushiring
to'plamli erlar yoki telemetriya yangi dalillarga muhtoj.

## Qo'llanish va yangilash jarayoni

- Yangi GPU tasvirlarini yaratish yoki o'rash (`scripts/fastpq/wrap_benchmark.py` orqali),
  ularni suratga olish matritsasiga qo'shing va yangilash uchun generatorni qayta ishga tushiring
  jadvallar.
- Poseidon microbench ma'lumotlari mavjud bo'lganda, uni eksport qiling
  `scripts/fastpq/export_poseidon_microbench.py` va manifestni qayta tiklang
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- JSON chiqimlarini ostida saqlash orqali Merkle chegaralarini tozalashni yozing
  `benchmarks/merkle_threshold/`; bu generator ma'lum fayllarni ro'yxatini ko'rsatadi, shuning uchun audit
  CPU va GPU mavjudligini o'zaro bog'lashi mumkin.

## FASTPQ 7-bosqich GPU ko'rsatkichlari

| To'plam | Backend | Rejim | GPU backend | GPU mavjud | Qurilma sinfi | GPU | LDE ms (CPU/GPU/SU) | Poseidon ms (CPU/GPU/SU) |
|-------|---------|------|-------------|---------------|--------------|-----|--------------------------------|---------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | gpu | cuda-sm80 | ha | xeon-rtx | NVIDIA RTX 6000 Ada | 1512.9/880.7/1.72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | metall | gpu | hech | ha | olma-m4 | Apple GPU 40 yadroli | 785,6/735,6/1,07 | 1803,8/1897,5/0,95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | metall | gpu | metall | ha | olma-m2-ultra | Apple M2 Ultra | 1581.1/1604.5/0.98 | 3589,9/3697,3/0,97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | metall | gpu | metall | ha | olma-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939,5/4083,3/0,96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | metall | gpu | metall | ha | olma-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939,5/4083,3/0,96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | opencl | gpu | opencl | ha | neoverse-mi300 | AMD Instinct MI300A | 4518,5/688,9/6,56 | 2780.4/905.6/3.07 |

> Ustunlar: `Backend` to'plam nomidan olingan; `Mode`/`GPU backend`/`GPU available`
> oʻralgan `benchmarks` blokidan protsessorning nosozliklari yoki etishmayotgan GPUni aniqlash uchun koʻchiriladi
> kashfiyot (masalan, `gpu_backend=none` `Mode=gpu` ga qaramasdan). SU = tezlikni oshirish nisbati (CPU/GPU).

## Poseidon Microbench suratlari

`benchmarks/poseidon/manifest.json` standart va skalar Poseidonni birlashtiradi
har bir metall to'plamdan eksport qilinadigan mikrobench ishlaydi. Quyidagi jadval tomonidan yangilangan
generator skripti, shuning uchun CI va boshqaruv sharhlari tarixiy tezlikni farq qilishi mumkin
o'ralgan FASTPQ hisobotlarini ochmasdan.

| Xulosa | To'plam | Vaqt tamg'asi | Standart ms | Skaler ms | Tezlashtirish |
|---------|--------|-----------|------------|-----------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167,7 | 2152.2 | 0,99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990.5 | 1994.5 | 1.00 |

## Merkle ostonasini tozalashMa'lumotnomalar orqali to'plangan
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
`benchmarks/merkle_threshold/` ostida yashash. Ro'yxat yozuvlari xost yoki yo'qligini ko'rsatadi
supurish paytida ochiq metall qurilmalar; GPU yoqilgan suratlar haqida xabar berishi kerak
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

Apple Silicon capture (`takemiyacStudio.lan_25.0.0_arm64`) `docs/source/benchmarks.md` da ishlatiladigan kanonik GPU asosidir; macOS 14 yozuvlari metall qurilmalarga ta'sir qila olmaydigan muhitlar uchun faqat protsessor uchun asos bo'lib qoladi.

## Qatordan foydalanish suratlari

`scripts/fastpq/check_row_usage.py` orqali olingan guvohlarning dekodlashlari transferni tasdiqlaydi
gadjetning qator samaradorligi. JSON artefaktlarini `artifacts/fastpq_benchmarks/` ostida saqlang
va bu generator auditorlar uchun qayd etilgan transfer nisbatlarini umumlashtiradi.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` - to'plamlar = 2, o'tkazish_nisbati o'rtacha = 0,629 (min=0,625, max=0,633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — partiyalar=2, transfer_nisbati oʻrtacha=0,619 (min=0,613, max=0,625)