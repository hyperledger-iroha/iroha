---
lang: my
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2025-12-29T18:16:35.932211+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# လျှို့ဝှက်ဓာတ်ငွေ့ ချိန်ညှိခြင်းအခြေခံများ

ဤစာရင်းဇယားသည် လျှို့ဝှက်ဓာတ်ငွေ့ ချိန်ညှိခြင်း၏ တရားဝင်ရလဒ်များကို ခြေရာခံသည်။
စံနှုန်းများ အတန်းတစ်ခုစီသည် ဖမ်းယူထားသော ထုတ်လွှတ်မှုအရည်အသွေး တိုင်းတာမှုအစုံကို မှတ်တမ်းတင်ထားသည်။
`docs/source/confidential_assets.md#calibration-baselines--acceptance-gates` တွင်ဖော်ပြထားသောလုပ်ထုံးလုပ်နည်း။

| ရက်စွဲ (UTC) | ကျူးလွန် | ကိုယ်ရေးအကျဉ်း | `ns/op` | `gas/op` | `ns/gas` | မှတ်စုများ |
| ---| ---| ---| ---| ---| ---| ---|
| 2025-10-18 | 3c70a7d3 | အခြေခံလိုင်း-နီယွန် | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | အခြေခံလိုင်း-နီယွန်-20260428 | 4.29e6 | 1.57e2 | 2.73e4 | Darwin 25.0.0 arm64 (`rustc 1.91.0`)။ Command: `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`; `docs/source/confidential_assets_calibration_neon_20260428.log` တွင်ဝင်ရောက်ပါ။ x86_64 parity runs (SIMD-neutral + AVX2) ကို 2026-03-19 Zurich lab slot အတွက် စီစဉ်ထားပါသည်။ Artefacts များသည် ကိုက်ညီသော command များဖြင့် `artifacts/confidential_assets_calibration/2026-03-x86/` အောက်တွင် ရောက်ရှိမည်ဖြစ်ပြီး ဖမ်းပြီးသည်နှင့် အခြေခံဇယားတွင် ပေါင်းထည့်မည်ဖြစ်သည်။ |
| 2026-04-28 | — | အခြေခံလိုင်း-simd-ကြားနေ | — | — | — | Apple Silicon တွင် **Waived**—`ring` သည် ပလပ်ဖောင်း ABI အတွက် NEON ကို ပြဋ္ဌာန်းထားသောကြောင့် `RUSTFLAGS="-C target-feature=-neon"` သည် ခုံတန်းရှည်များ မလည်ပတ်မီ (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`) တွင် ပျက်သွားပါသည်။ ကြားနေဒေတာကို CI လက်ခံဆောင်ရွက်ပေးသူ `bench-x86-neon0` တွင် ပိတ်ထားပါသည်။ |
| 2026-04-28 | — | အခြေခံလိုင်း-avx2 | — | — | — | x86_64 အပြေးသမားတစ်ဦးရရှိနိုင်သည်အထိ **ရွှေ့ဆိုင်းထားသည်။ `arch -x86_64` သည် ဤစက်ပေါ်တွင် binary များမပေါက်နိုင်ပါ (“Executeable in Bad CPU type”; `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log` ကိုကြည့်ပါ)။ CI လက်ခံဆောင်ရွက်ပေးသူ `bench-x86-avx2a` သည် မှတ်တမ်း၏အရင်းအမြစ်အဖြစ် ကျန်ရှိနေပါသည်။ |

`ns/op` သည် Criterion ဖြင့်တိုင်းတာသော ညွှန်ကြားချက်တစ်ခုအတွက် ပျမ်းမျှ နံရံနာရီကို စုစည်းထားသည်။
`gas/op` သည် သက်ဆိုင်ရာ အချိန်ဇယားကုန်ကျစရိတ်၏ ဂဏန်းသင်္ချာဆိုလိုချက်ဖြစ်သည်
`iroha_core::gas::meter_instruction`; `ns/gas` သည် ပေါင်းစည်းထားသော နာနိုစက္ကန့်များကို ပိုင်းခြားသည်။
ညွှန်ကြားချက်နမူနာကိုးခုတွင် စုစည်းထားသောဓာတ်ငွေ့။

*မှတ်ချက်။* လက်ရှိ arm64 host သည် Criterion `raw.csv` မှ အနှစ်ချုပ်များကို ထုတ်လွှတ်ခြင်းမရှိပါ။
သေတ္တာ; တစ်ဂ်မတင်မီ `CRITERION_OUTPUT_TO=csv` ဖြင့် ပြန်ဖွင့်ပါ
လက်ခံမှုစာရင်းအတွက် လိုအပ်သောပစ္စည်းများကို ပူးတွဲပါရှိစေခြင်းဖြင့် ထုတ်ပြန်ပါ။
`target/criterion/` သည် `--save-baseline` ပြီးနောက် ပျောက်ဆုံးနေသေးပါက အပြေးကို စုဆောင်းပါ။
Linux host တစ်ခုပေါ်တွင် သို့မဟုတ် console output ကို release bundle တွင် အမှတ်အသားပြုပါ။
ယာယီရပ်တန့်-ကွာဟချက်။ အကိုးအကားအတွက်၊ နောက်ဆုံးထွက်ပြေးမှုမှ arm64 ကွန်ဆိုးလ်မှတ်တမ်း
`docs/source/confidential_assets_calibration_neon_20251018.log` တွင် နေထိုင်သည်။

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

### 2026-04-28 (Apple Silicon၊ NEON ဖွင့်ထားသည်)

2026-04-28 ပြန်လည်စတင်ခြင်းအတွက် အလယ်အလတ်ကြာချိန်များ (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`)| ပို့ချ | ပျမ်းမျှ `ns/op` | အချိန်ဇယား `gas` | `ns/gas` |
| ---| ---| ---| ---|
| RegisterDomain | 8.58e6 | 200 | 4.29e4 |
| စာရင်းသွင်းအကောင့် | 4.40e6 | 200 | 2.20e4 |
| RegisterAssetDef | 4.23e6 | 200 | 2.12e4 |
| SetAccountKV_small | 3.79e6 | 67 | 5.66e4 |
| GrantAccountRole | 3.60e6 | 96 | 3.75e4 |
| RevokeAccountRole | 3.76e6 | 96 | 3.92e4 |
| ExecuteTrigger_empty_args | 2.71e6 | 224 | 1.21e4 |
| MintAsset | 3.92e6 | 150 | 2.61e4 |
| TransferAsset | 3.59e6 | 180 | 1.99e4 |

အထက်ပါဇယားရှိ `ns/op` နှင့် `ns/gas` တို့သည် ပေါင်းလဒ်မှဆင်းသက်လာပါသည်
အဆိုပါ မီဒီယာများ (စုစုပေါင်း `3.85717e7`ns ညွှန်ကြားချက်ကိုးခုနှင့် 1,413
ဓာတ်ငွေ့ယူနစ်)။

အချိန်ဇယားကော်လံကို `gas::tests::calibration_bench_gas_snapshot` က ပြဋ္ဌာန်းထားသည်။
(စုစုပေါင်း 1,413 ဓာတ်ငွေ့ ညွှန်ကြားချက် ကိုးခုကို ဖြတ်၍) နှင့် နောင်ဖာထေးလျှင် ခရီးထွက်ပါမည်။
ချိန်ညှိကိရိယာများကို မွမ်းမံခြင်းမရှိဘဲ မီတာတိုင်းတာခြင်းကို ပြောင်းလဲပါ။

## ကတိကဝတ် Tree Telemetry အထောက်အထား (M2.2)

လမ်းပြမြေပုံလုပ်ငန်း **M2.2** တွင်၊ ချိန်ညှိခြင်းလုပ်ဆောင်မှုတိုင်းသည် အသစ်ကို ဖမ်းယူရမည်ဖြစ်သည်။
Merkle နယ်နိမိတ်ကို သက်သေပြရန်အတွက် ကတိကဝတ်-သစ်ပင် တိုင်းတာမှုများနှင့် ဖယ်ရှားရေးကောင်တာများ
ပြင်ဆင်သတ်မှတ်ထားသော ဘောင်များအတွင်း-

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

ချိန်ညှိခြင်းလုပ်ငန်းမစမီနှင့် အပြီးတွင် တန်ဖိုးများကို ချက်ချင်းမှတ်တမ်းတင်ပါ။ တစ်
ပိုင်ဆိုင်မှုတစ်ခုအတွက် command တစ်ခုတည်းသည် လုံလောက်ပါသည်။ `4cuvDVPuLBKJyN6dPbRQhmLh68sU` အတွက် ဥပမာ

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="4cuvDVPuLBKJyN6dPbRQhmLh68sU"}'
```

ကုန်ကြမ်းအထွက် (သို့မဟုတ် Prometheus) ကို ချိန်ညှိခြင်းလက်မှတ်တွင် ပူးတွဲပါ
အုပ်ချုပ်မှုပြန်လည်သုံးသပ်သူသည် root-history caps နှင့် checkpoint ကြားကာလများကိုအတည်ပြုနိုင်သည်။
ဂုဏ်ပြုပါသည်။ `docs/source/telemetry.md#confidential-tree-telemetry-m22` ရှိ တယ်လီမီတာလမ်းညွှန်
သတိပေးချက် မျှော်လင့်ချက်များနှင့် ဆက်စပ် Grafana အကန့်များပေါ်တွင် ချဲ့ထွင်သည်။

သုံးသပ်သူများ အတည်ပြုနိုင်စေရန် တူညီသောခြစ်ရာတွင် စိစစ်ရေးကက်ရှ်ကောင်တာများကို ထည့်သွင်းပါ။
miss ratio သည် 40% သတိပေးချက်အဆင့်အောက်၌ရှိနေသည်-

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

စံချိန်စံညွှန်းမှတ်စုအတွင်း ရရှိလာသော အချိုး (`miss / (hit + miss)`) ကို မှတ်တမ်းတင်ပါ
SIMD-neutral cost modeling လေ့ကျင့်ခန်းများအစား warm cache ကို ပြန်သုံးပြရန်
Halo2 verifier registry ကို နှိပ်ခြင်း။

## ကြားနေ & AVX2 စွန့်လွှတ်မှု

SDK ကောင်စီသည် လိုအပ်သော PhaseC ဂိတ်အတွက် ယာယီစွန့်လွှတ်မှုကို ခွင့်ပြုခဲ့သည်။
`baseline-simd-neutral` နှင့် `baseline-avx2` တိုင်းတာချက်များ-

- **SIMD-ကြားနေ-** Apple Silicon တွင် `ring` crypto backend သည် NEON အတွက် ပြဋ္ဌာန်းသည်
  ABI မှန်ကန်မှု။ အင်္ဂါရပ်ကို ပိတ်ခြင်း (`RUSTFLAGS="-C target-feature=-neon"`)
  bench binary (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`) မထုတ်လုပ်မီ တည်ဆောက်မှုကို ဖျက်ပစ်သည်။
- **AVX2:** ဒေသတွင်း ကိရိယာကွင်းဆက်သည် x86_64 ဒွိနရီများ မပေါက်နိုင်ပါ (`arch -x86_64 rustc -V`
  → “Execute လုပ်နိုင်သော CPU အမျိုးအစားမကောင်း”; ကြည့်ပါ။
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`)။

CI ဝန်ဆောင်မှုပေးသည့် `bench-x86-neon0` နှင့် `bench-x86-avx2a` သည် အွန်လိုင်းမပေါ်မချင်း NEON လည်ပတ်သည်
အထက်ဖော်ပြပါ ကြေးနန်းဆိုင်ရာ သက်သေအထောက်အထားများသည် PhaseC လက်ခံမှုစံနှုန်းများကို ဖြည့်ဆည်းပေးသည်။
စွန့်လွှတ်မှုကို `status.md` တွင် မှတ်တမ်းတင်ထားပြီး x86 ဟာ့ဒ်ဝဲဖြစ်သည်နှင့် ပြန်လည်ကြည့်ရှုမည်
ရရှိနိုင်