---
lang: uz
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: db113723dbedefdb89333e2e8fb1483aed9b5603cd2e9bb93fb46f7206533518
source_last_modified: "2025-12-29T18:16:35.121357+00:00"
translation_last_reviewed: 2026-02-07
title: Confidential Gas Calibration Ledger
description: Release-quality measurements backing the confidential gas schedule.
slug: /nexus/confidential-gas-calibration
translator: machine-google-reviewed
---

# Maxfiy gaz kalibrlash asoslari

Ushbu kitob maxfiy gaz kalibrlashning tasdiqlangan natijalarini kuzatib boradi
benchmarks. Har bir qatorda suratga olingan reliz sifati o'lchovlari to'plami hujjatlashtirilgan
[Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates) da tavsiflangan protsedura.

| Sana (UTC) | Majburiyat | Profil | `ns/op` | `gas/op` | `ns/gas` | Eslatmalar |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | asosiy neon | 2.93e5 | 1.57e2 | 1.87e3 | Darvin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | kutilmoqda | bazaviy-simd-neytral | — | — | — | `bench-x86-neon0` CI xostida rejalashtirilgan x86_64 neytral ishlashi; GAS-214 chiptasiga qarang. Natijalar dastgoh oynasi tugagandan so'ng qo'shiladi (birlashtirishdan oldin nazorat ro'yxati maqsadlari 2.1 nashri). |
| 2026-04-13 | kutilmoqda | baseline-avx2 | — | — | — | Neytral ishga tushirish bilan bir xil majburiyat/qurilishdan foydalangan holda AVX2 kalibrlashni kuzatib boring; `bench-x86-avx2a` xostini talab qiladi. GAS-214 `baseline-neon` bilan delta taqqoslash bilan ikkala yugurishni ham qamrab oladi. |

`ns/op` mezon bo'yicha o'lchangan har bir ko'rsatma uchun o'rtacha devor soatini jamlaydi;
`gas/op` - tegishli jadval xarajatlarining o'rtacha arifmetik qiymati
`iroha_core::gas::meter_instruction`; `ns/gas` yig'ilgan nanosoniyalarni ga ajratadi
to'qqiz instruktsiyali namunalar to'plami bo'ylab jamlangan gaz.

*Eslatma.* Joriy arm64 xosti `raw.csv` mezoni xulosalarini chiqarmaydi
quti; a teglashdan oldin `CRITERION_OUTPUT_TO=csv` yoki yuqoridagi tuzatish bilan qayta ishga tushiring
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

Jadval ustuni `gas::tests::calibration_bench_gas_snapshot` tomonidan amalga oshiriladi
(to'qqizta ko'rsatmalar to'plami bo'ylab jami 1,413 gaz) va agar kelajakda yamoqlar bo'lsa, o'chib ketadi
kalibrlash moslamalarini yangilamasdan o'lchashni o'zgartirish.