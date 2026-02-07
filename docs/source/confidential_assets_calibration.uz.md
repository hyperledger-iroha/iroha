---
lang: uz
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2025-12-29T18:16:35.932211+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Maxfiy gaz kalibrlash asoslari

Ushbu kitob maxfiy gaz kalibrlashning tasdiqlangan natijalarini kuzatib boradi
benchmarks. Har bir qatorda suratga olingan reliz sifati o'lchovlari to'plami hujjatlashtirilgan
`docs/source/confidential_assets.md#calibration-baselines--acceptance-gates` da tasvirlangan protsedura.

| Sana (UTC) | Majburiyat | Profil | `ns/op` | `gas/op` | `ns/gas` | Eslatmalar |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | asosiy neon | 2.93e5 | 1.57e2 | 1.87e3 | Darvin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 28.04.2026 | 8ea9b2a7 | bazaviy-neon-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | Darvin 25.0.0 arm64 (`rustc 1.91.0`). Buyruq: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; `docs/source/confidential_assets_calibration_neon_20260428.log` manziliga kiring. x86_64 pariteti (SIMD-neytral + AVX2) 2026-03-19 Tsyurix laboratoriya uyasiga rejalashtirilgan; artefaktlar mos keladigan buyruqlar bilan `artifacts/confidential_assets_calibration/2026-03-x86/` ostida tushadi va qo'lga kiritilgandan so'ng asosiy jadvalga birlashtiriladi. |
| 28.04.2026 | — | bazaviy-simd-neytral | — | — | — | **Apple Silicon-da bekor qilindi**—`ring` ABI platformasi uchun NEON-ni qo'llaydi, shuning uchun `RUSTFLAGS="-C target-feature=-neon"` dastgoh ishlamasdan oldin ishlamay qoladi (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`). Neytral ma'lumotlar CI xosti `bench-x86-neon0` da yopiq qoladi. |
| 28.04.2026 | — | baseline-avx2 | — | — | — | **X86_64 yuguruvchisi mavjud bo'lgunga qadar kechiktirildi**. `arch -x86_64` bu mashinada ikkilik fayllarni yarata olmaydi (bajariladigan faylda noto'g'ri protsessor turi; qarang: `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`). CI xost `bench-x86-avx2a` rekord manbai bo'lib qolmoqda. |

`ns/op` mezon bo'yicha o'lchangan har bir ko'rsatma uchun o'rtacha devor soatini jamlaydi;
`gas/op` - tegishli jadval xarajatlarining o'rtacha arifmetik qiymati
`iroha_core::gas::meter_instruction`; `ns/gas` yig'ilgan nanosoniyalarni ga ajratadi
to'qqiz instruktsiyali namunalar to'plami bo'ylab jamlangan gaz.

*Eslatma.* Joriy arm64 xosti `raw.csv` mezoni xulosalarini chiqarmaydi
quti; a teglashdan oldin `CRITERION_OUTPUT_TO=csv` yoki yuqori oqim tuzatish bilan qayta ishga tushiring
chiqaring, shuning uchun qabul qilish nazorat ro'yxatida talab qilinadigan artefaktlar ilova qilinadi.
Agar `target/criterion/` `--save-baseline` dan keyin ham yo'qolsa, yugurishni to'plang.
Linux xostida yoki konsol chiqishini relizlar to'plamiga ketma-ketlashtiring
vaqtinchalik uzilish. Malumot uchun, so'nggi ishga tushirishdan arm64 konsol jurnali
`docs/source/confidential_assets_calibration_neon_20251018.log` da yashaydi.

Xuddi shu ishga tushirilgan har bir ko'rsatma medianalari (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Ko'rsatma | median `ns/op` | jadval `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| RegisterAccount | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

### 28.04.2026 (Apple Silicon, NEON yoqilgan)

28.04.2026 yangilash uchun oʻrtacha kechikishlar (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`):| Ko'rsatma | median `ns/op` | jadval `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 8.58e6 | 200 | 4.29e4 |
| RegisterAccount | 4.40e6 | 200 | 2.20e4 |
| RegisterAssetDef | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66e4 |
| GrantAccountRole | 3.60e6 | 96 | 3.75e4 |
| RevokeAccountRole | 3.76e6 | 96 | 3.92e4 |
| ExecuteTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| MintAsset | 3.92e6 | 150 | 2.61e4 |
| TransferAsset | 3.59e6 | 180 | 1.99e4 |

Yuqoridagi jadvaldagi `ns/op` va `ns/gas` agregatlari yig'indisidan olingan.
bu medianlar (to'qqizta ko'rsatmalar to'plami bo'yicha jami `3.85717e7`ns va 1413
gaz birliklari).

Jadval ustuni `gas::tests::calibration_bench_gas_snapshot` tomonidan amalga oshiriladi
(to'qqizta ko'rsatmalar to'plami bo'ylab jami 1,413 gaz) va agar kelajakda yamoqlar bo'lsa, o'chib ketadi
kalibrlash moslamalarini yangilamasdan o'lchashni o'zgartirish.

## Majburiyatlar daraxti telemetriya dalillari (M2.2)

**M2.2** yo'l xaritasi vazifasiga ko'ra, har bir kalibrlash jarayoni yangisini olishi kerak
Merkle chegarasi qolishini isbotlash uchun majburiyat daraxti o'lchagichlari va ko'chirish hisoblagichlari
sozlangan chegaralar ichida:

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

Kalibrlash ish yukidan oldin va keyin qiymatlarni darhol yozib oling. A
har bir aktiv uchun bitta buyruq etarli; `xor#wonderland` uchun misol:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

Xom chiqishni (yoki Prometheus suratini) kalibrlash chiptasiga ulang, shunda
Boshqaruv tekshiruvchisi ildiz tarixining chegaralarini va nazorat nuqtalari oralig'ini tasdiqlashi mumkin
sharaflangan. `docs/source/telemetry.md#confidential-tree-telemetry-m22` da telemetriya qo'llanmasi
ogohlantirish kutishlari va tegishli Grafana panellarini kengaytiradi.

Tekshiruvchilar kesh hisoblagichlarini bir xil skrepga qo'shing, shunda ko'rib chiquvchilar tasdiqlashlari mumkin
o'tkazib yuborish darajasi 40% ogohlantirish chegarasidan past bo'lib qoldi:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

Olingan nisbatni (`miss / (hit + miss)`) kalibrlash eslatmasi ichida hujjatlang
SIMD-neytral xarajat modellashtirish mashqlar o'rniga issiq keshlar qayta ko'rsatish uchun
Halo2 tekshiruvi registrini sindirish.

## Neytral va AVX2dan voz kechish

SDK Kengashi PhaseC eshigini talab qilish uchun vaqtinchalik voz kechdi
`baseline-simd-neutral` va `baseline-avx2` o'lchovlari:

- **SIMD-neytral:** Apple Silicon-da `ring` kripto-versiyasi NEON-dan foydalanadi.
  ABI to'g'riligi. Funktsiyani o'chirish (`RUSTFLAGS="-C target-feature=-neon"`)
  dastgoh binari ishlab chiqarilishidan oldin qurilishni to'xtatadi (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`).
- **AVX2:** Mahalliy asboblar zanjiri x86_64 ikkilik fayllarni ishlab chiqara olmaydi (`arch -x86_64 rustc -V`
  → "Bajariladigan faylda noto'g'ri CPU turi"; qarang
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`).

`bench-x86-neon0` va `bench-x86-avx2a` CI xostlari onlayn bo'lgunga qadar NEON ishlaydi
yuqoridagi va telemetriya dalillari PhaseC qabul qilish mezonlariga javob beradi.
Voz kechish `status.md` da qayd etilgan va x86 uskunasi oʻrnatilgandan keyin qayta koʻrib chiqiladi.
mavjud.