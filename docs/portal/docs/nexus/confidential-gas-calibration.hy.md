---
lang: hy
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

# Գազի չափորոշման գաղտնի ելակետեր

Այս մատյանը հետևում է գազի գաղտնի տրամաչափման վավերացված արդյունքներին
հենանիշներ. Յուրաքանչյուր տող փաստաթղթավորում է թողարկման որակի չափման հավաքածու, որը նկարահանվել է
ընթացակարգը նկարագրված է [Գաղտնի ակտիվներ և ZK փոխանցումներ] (./confidential-assets#calibration-baselines--acceptance-gates):

| Ամսաթիվ (UTC) | Պարտավորել | Անձնագիր | `ns/op` | `gas/op` | `ns/gas` | Ծանոթագրություններ |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | բազային-նեոնային | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | սպասվող | բազային-simd-չեզոք | — | — | — | Պլանավորված x86_64 չեզոք գործարկում CI հոսթի `bench-x86-neon0`-ում; տես ԳԱԶ-214 տոմսը։ Արդյունքները կավելացվեն նստարանային պատուհանի ավարտից հետո (նախապես միաձուլման ստուգաթերթի թիրախների թողարկում 2.1): |
| 2026-04-13 | սպասվող | բազային-avx2 | — | — | — | Հետևել AVX2 տրամաչափումը` օգտագործելով նույն commit/build, ինչ չեզոք գործարկումը; պահանջում է հոսթ `bench-x86-avx2a`: GAS-214-ն ընդգրկում է երկու գործարկումները՝ `baseline-neon`-ի հետ դելտա համեմատությամբ: |

`ns/op`-ը միավորում է պատի ժամացույցի մեդիանը՝ չափանիշով չափված յուրաքանչյուր հրահանգի համար.
`gas/op`-ը համապատասխան ժամանակացույցի ծախսերի թվաբանական միջինն է
`iroha_core::gas::meter_instruction`; `ns/gas`-ը բաժանում է ամփոփված նանվայրկյանները
ամփոփված գազը ինը հրահանգներից բաղկացած նմուշի հավաքածուում:

*Նշում.* Ներկայիս arm64 հոսթը չի թողարկում `raw.csv` չափանիշի ամփոփագրերը
տուփ; կրկնել `CRITERION_OUTPUT_TO=csv`-ով կամ վերին հոսանքով շտկելով՝ նախքան a-ին հատկորոշելը
թողարկել, որպեսզի ընդունման ստուգաթերթով պահանջվող արտեֆակտները կցվեն:
Եթե `target/criterion/`-ը դեռ բացակայում է `--save-baseline`-ից հետո, հավաքեք վազքը
Linux հոսթի վրա կամ սերիականացնել կոնսոլի ելքը թողարկման փաթեթում որպես a
ժամանակավոր կանգառ: Տեղեկատվության համար, arm64 կոնսոլի գրանցամատյանը վերջին գործարկումից
ապրում է `docs/source/confidential_assets_calibration_neon_20251018.log`-ում:

Մեկ հրահանգի միջինները նույն գործարկումից (`cargo bench -p iroha_core --bench isi_gas_calibration`).

| Հրահանգ | միջին `ns/op` | ժամանակացույց `gas` | `ns/gas` |
| --- | --- | --- | --- |
| Գրանցվել Դոմեն | 3.46e5 | 200 | 1.73e3 |
| Գրանցվել Հաշիվ | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

Ժամանակացույցի սյունակը պարտադրված է `gas::tests::calibration_bench_gas_snapshot`-ով
(ընդհանուր 1,413 գազ ամբողջ ինը հրահանգների հավաքածուում) և կկանգնեցվի, եթե ապագա կարկատվեն
փոխել հաշվառումը` առանց տրամաչափման սարքերը թարմացնելու: