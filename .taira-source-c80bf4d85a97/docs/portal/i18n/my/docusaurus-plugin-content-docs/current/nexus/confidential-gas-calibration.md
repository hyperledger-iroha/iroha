---
slug: /nexus/confidential-gas-calibration
lang: my
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Confidential Gas Calibration Ledger
description: Release-quality measurements backing the confidential gas schedule.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# လျှို့ဝှက်ဓာတ်ငွေ့ ချိန်ညှိခြင်းအခြေခံများ

ဤစာရင်းဇယားသည် လျှို့ဝှက်ဓာတ်ငွေ့ ချိန်ညှိခြင်း၏ တရားဝင်ရလဒ်များကို ခြေရာခံသည်။
စံနှုန်းများ အတန်းတစ်ခုစီသည် ဖမ်းယူထားသော ထုတ်လွှတ်မှုအရည်အသွေး တိုင်းတာမှုအစုံကို မှတ်တမ်းတင်ထားသည်။
[Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates) တွင် ဖော်ပြထားသည့် လုပ်ထုံးလုပ်နည်း။

| ရက်စွဲ (UTC) | ကျူးလွန် | ကိုယ်ရေးအကျဉ်း | `ns/op` | `gas/op` | `ns/gas` | မှတ်စုများ |
| ---| ---| ---| ---| ---| ---| ---|
| 2025-10-18 | 3c70a7d3 | အခြေခံလိုင်း-နီယွန် | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | ဆိုင်းငံ့ | အခြေခံလိုင်း-simd-ကြားနေ | — | — | — | CI host `bench-x86-neon0` တွင် x86_64 ကြားနေ run ရန် စီစဉ်ထားသည်။ GAS-214 လက်မှတ်ကိုကြည့်ပါ။ ခုံတန်းဝင်းဒိုးပြီးသည်နှင့် (အကြိုစာရင်းစစ်စာရင်းပစ်မှတ်များ 2.1 ထွက်ရှိသည်)။ |
| 2026-04-13 | ဆိုင်းငံ့ | အခြေခံလိုင်း-avx2 | — | — | — | တူညီသော commit/build ကိုအသုံးပြု၍ AVX2 ကို ချိန်ညှိခြင်း နောက်ဆက်တွဲလုပ်ဆောင်ခြင်း လက်ခံသူ `bench-x86-avx2a` လိုအပ်သည်။ GAS-214 သည် `baseline-neon` နှင့် မြစ်ဝကျွန်းပေါ် နှိုင်းယှဉ်မှုဖြင့် လည်ပတ်မှုနှစ်ခုလုံးကို အကျုံးဝင်သည်။ |

`ns/op` သည် Criterion ဖြင့်တိုင်းတာသော ညွှန်ကြားချက်တစ်ခုအတွက် ပျမ်းမျှ နံရံနာရီကို စုစည်းထားသည်။
`gas/op` သည် သက်ဆိုင်ရာ အချိန်ဇယားကုန်ကျစရိတ်များ၏ ဂဏန်းသင်္ချာဆိုလိုချက်ဖြစ်သည်
`iroha_core::gas::meter_instruction`; `ns/gas` သည် ပေါင်းစည်းထားသော နာနိုစက္ကန့်များကို ပိုင်းခြားသည်။
ညွှန်ကြားချက်နမူနာကိုးခုတွင် စုစည်းထားသောဓာတ်ငွေ့။

*မှတ်ချက်။* လက်ရှိ arm64 host သည် စံသတ်မှတ်ချက် `raw.csv` မှ အနှစ်ချုပ်များကို ထုတ်လွှတ်ခြင်းမရှိပါ။
သေတ္တာ; တစ်ဂ်မတင်မီ `CRITERION_OUTPUT_TO=csv` ဖြင့် ပြန်ဖွင့်ပါ
လက်ခံမှုစာရင်းအတွက် လိုအပ်သောပစ္စည်းများကို ပူးတွဲပါရှိစေခြင်းဖြင့် ထုတ်ပြန်ပါ။
`target/criterion/` သည် `--save-baseline` ပြီးနောက် ပျောက်ဆုံးနေသေးပါက အပြေးကို စုဆောင်းပါ။
Linux host တစ်ခုပေါ်တွင် သို့မဟုတ် console output ကို release bundle တွင် အမှတ်အသားပြုပါ။
ယာယီရပ်တန့်-ကွာဟချက်။ အကိုးအကားအတွက်၊ နောက်ဆုံးထွက်ပြေးမှုမှ arm64 ကွန်ဆိုးလ်မှတ်တမ်း
`docs/source/confidential_assets_calibration_neon_20251018.log` တွင်နေထိုင်သည်။

တူညီသောလည်ပတ်မှုမှ ညွှန်ကြားချက်တစ်ခုစီတိုင်း (`cargo bench -p iroha_core --bench isi_gas_calibration`)

| ပို့ချ | ပျမ်းမျှ `ns/op` | အချိန်ဇယား `gas` | `ns/gas` |
| ---| ---| ---| ---|
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| စာရင်းသွင်းအကောင့် | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

အချိန်ဇယားကော်လံကို `gas::tests::calibration_bench_gas_snapshot` က ပြဋ္ဌာန်းထားသည်။
(စုစုပေါင်း 1,413 ဓာတ်ငွေ့ ညွှန်ကြားချက် ကိုးခုကို ဖြတ်၍) နှင့် နောင်ဖာထေးလျှင် ခရီးထွက်ပါမည်။
ချိန်ညှိကိရိယာများကို မွမ်းမံခြင်းမရှိဘဲ မီတာတိုင်းတာခြင်းကို ပြောင်းလဲပါ။