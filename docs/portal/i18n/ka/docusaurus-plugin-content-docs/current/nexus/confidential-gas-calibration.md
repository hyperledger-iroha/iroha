---
slug: /nexus/confidential-gas-calibration
lang: ka
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Confidential Gas Calibration Ledger
description: Release-quality measurements backing the confidential gas schedule.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# კონფიდენციალური გაზის კალიბრაციის საბაზისო ხაზები

ეს წიგნი აკონტროლებს კონფიდენციალური გაზის კალიბრაციის დადასტურებულ შედეგებს
ეტალონები. თითოეული მწკრივი ადასტურებს გამოშვების ხარისხის გაზომვის კომპლექტს, რომელიც აღბეჭდილია
[Confidential Assets & ZK Transfers] (./confidential-assets#calibration-baselines--acceptance-gates) აღწერილი პროცედურა.

| თარიღი (UTC) | ვალდებულება | პროფილი | `ns/op` | `gas/op` | `ns/gas` | შენიშვნები |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | საბაზისო-ნეონი | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | მომლოდინე | საბაზისო-simd-ნეიტრალური | — | — | — | დაგეგმილი x86_64 ნეიტრალური გაშვება CI ჰოსტზე `bench-x86-neon0`; იხილეთ ბილეთი GAS-214. შედეგები დაემატება სკამების ფანჯრის დასრულების შემდეგ (წინასწარ შერწყმის საკონტროლო სიის მიზნების გამოშვება 2.1). |
| 2026-04-13 | მომლოდინე | საბაზისო-avx2 | — | — | — | შემდგომი AVX2 კალიბრაცია იგივე commit/build-ის გამოყენებით, როგორც ნეიტრალური გაშვება; მოითხოვს ჰოსტი `bench-x86-avx2a`. GAS-214 მოიცავს ორივე გაშვებას დელტა შედარებით `baseline-neon`-თან. |

`ns/op` აერთიანებს მედიანურ კედლის საათს კრიტერიუმით გაზომილი ინსტრუქციის მიხედვით;
`gas/op` არის შესაბამისი გრაფიკის ხარჯების არითმეტიკული საშუალო
`iroha_core::gas::meter_instruction`; `ns/gas` შეჯამებულ ნანოწამებს ყოფს
შეჯამებული გაზი ცხრა ინსტრუქციის ნიმუშის კომპლექტში.

*შენიშვნა.* ამჟამინდელი arm64 ჰოსტი არ გამოსცემს `raw.csv` კრიტერიუმის შეჯამებებს
ყუთი; ხელახლა გაუშვით `CRITERION_OUTPUT_TO=csv` ან ზედა ნაკადის შესწორებით a-ს მონიშვნამდე
გაათავისუფლეთ ისე, რომ არტეფაქტები, რომლებიც მოთხოვნილი იყო მიღების სიით, თან ერთვის.
თუ `target/criterion/` კვლავ აკლია `--save-baseline`-ის შემდეგ, შეაგროვეთ გაშვება
Linux ჰოსტზე ან კონსოლის გამომავალი სერიულირება გამოშვების პაკეტში, როგორც a
დროებითი გაჩერება. ცნობისთვის, arm64 კონსოლის ჟურნალი უახლესი გაშვებიდან
ცხოვრობს `docs/source/confidential_assets_calibration_neon_20251018.log`-ზე.

თითო ინსტრუქციის მედიანა ერთი და იგივე გაშვებიდან (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| ინსტრუქცია | მედიანა `ns/op` | განრიგი `gas` | `ns/gas` |
| --- | --- | --- | --- |
| რეგისტრაცია დომენი | 3.46e5 | 200 | 1.73e3 |
| რეგისტრაცია ანგარიში | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

განრიგის სვეტი აღსრულებულია `gas::tests::calibration_bench_gas_snapshot`-ით
(სულ 1,413 გაზი ცხრა ინსტრუქციის კომპლექტში) და გაქრება, თუ მომავალში დაყენდება
გაზომვის შეცვლა კალიბრაციის მოწყობილობების განახლების გარეშე.