---
slug: /nexus/confidential-gas-calibration
lang: mn
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Confidential Gas Calibration Ledger
description: Release-quality measurements backing the confidential gas schedule.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Нууц хийн шалгалт тохируулгын суурь

Энэхүү дэвтэр нь хийн нууц тохируулгын баталгаажуулсан гаралтыг хянадаг
жишиг үзүүлэлтүүд. Мөр бүр нь авсан хувилбарын чанарын хэмжилтийн багцыг баримтжуулна
[Нууцлагдмал хөрөнгө ба ЗК шилжүүлэг](./confidential-assets#calibration-baselines--acceptance-gates) хэсэгт тайлбарласан процедур.

| Огноо (UTC) | Амлах | Профайл | `ns/op` | `gas/op` | `ns/gas` | Тэмдэглэл |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | суурь-неон | 2.93e5 | 1.57e2 | 1.87e3 | Дарвин 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | хүлээгдэж байна | baseline-simd-neutral | — | — | — | CI хост `bench-x86-neon0` дээр хуваарьт x86_64 төвийг сахисан ажиллуулах; GAS-214 тасалбарыг үзнэ үү. Үнэлгээний цонх дууссаны дараа илэрц нэмэгдэх болно (нэгдсэн шалгах хуудасны зорилтот хувилбар 2.1). |
| 2026-04-13 | хүлээгдэж байна | baseline-avx2 | — | — | — | Төвийг сахисан гүйлттэй ижил commit/build ашиглан AVX2 шалгалт тохируулга хийх; `bench-x86-avx2a` хостыг шаарддаг. GAS-214 нь `baseline-neon`-тай гурвалжин харьцуулалт бүхий хоёр гүйлтийг хамардаг. |

`ns/op` шалгуур үзүүлэлтээр хэмжсэн заавар бүрийн дундаж ханын цагийг нэгтгэдэг;
`gas/op` нь харгалзах хуваарийн зардлын арифметик дундаж юм.
`iroha_core::gas::meter_instruction`; `ns/gas` нь нийлбэр наносекундыг хуваана
есөн заавар бүхий дээжийн багц дахь нийлбэр хий.

*Тэмдэглэл.* Одоогийн arm64 хост нь `raw.csv` шалгуурын хураангуйг гаргадаггүй.
хайрцаг; a. шошголохоос өмнө `CRITERION_OUTPUT_TO=csv` эсвэл дээд талын засварыг ашиглан дахин ажиллуулна уу
суллах тул хүлээн авах хяналтын хуудсанд шаардлагатай олдворуудыг хавсаргана.
Хэрэв `target/criterion/` `--save-baseline` дараа байхгүй хэвээр байвал гүйлтийг цуглуул.
Линукс хост дээр эсвэл консолын гаралтыг хувилбарын багц болгон цуваа болгож a
түр зуурын завсарлага. Лавлагааны хувьд arm64 консолын хамгийн сүүлийн үеийн лог
`docs/source/confidential_assets_calibration_neon_20251018.log`-д амьдардаг.

Нэг зааварчилгааны медианууд (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Заавар | медиан `ns/op` | хуваарь `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| Бүртгүүлэх Данс | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

Хуваарийн баганыг `gas::tests::calibration_bench_gas_snapshot` мөрддөг
(есөн заавар бүхий багцад нийт 1,413 хий) бөгөөд ирээдүйд засварууд хийгдвэл хаах болно
тохируулгын төхөөрөмжийг шинэчлэхгүйгээр тоолуурыг өөрчлөх.