---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ffbb145e1e0aa9dc71bdb6896c4f8be69eb6226194c5c165905af1ac243cc9
source_last_modified: "2025-12-29T18:16:35.199832+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Capacity Marketplace Validation
tags: [SF-2c, acceptance, checklist]
summary: Acceptance checklist covering provider onboarding, dispute workflows, and treasury reconciliation gating the SoraFS capacity marketplace general availability.
translator: machine-google-reviewed
---

#SoraFS စွမ်းရည်စျေးကွက် မှန်ကန်ကြောင်း စစ်ဆေးချက်စာရင်း

**သုံးသပ်ချက်ဝင်းဒိုး-** 2026-03-18 → 2026-03-24  
** အစီအစဉ်ပိုင်ရှင်များ-** သိုလှောင်မှုအဖွဲ့ (`@storage-wg`)၊ အုပ်ချုပ်ရေးကောင်စီ (`@council`)၊ ငွေတိုက်အဖွဲ့ (`@treasury`)  
** နယ်ပယ်-** ဝန်ဆောင်မှုပေးသူ စတင်အသုံးပြုသည့် ပိုက်လိုင်းများ၊ အငြင်းပွားမှု စီရင်ချက်စီးဆင်းမှုများ၊ SF-2c GA အတွက် လိုအပ်သော ဘဏ္ဍာတိုက်ပြန်လည်သင့်မြတ်ရေး လုပ်ငန်းစဉ်များ။

ပြင်ပအော်ပရေတာများအတွက် စျေးကွက်ဖွင့်ခြင်းမပြုမီ အောက်ပါစစ်ဆေးစာရင်းကို ပြန်လည်သုံးသပ်ရပါမည်။ အတန်းတစ်ခုစီသည် စာရင်းစစ်များ ပြန်ဖွင့်နိုင်သည့် အဆုံးအဖြတ်ဆိုင်ရာ အထောက်အထားများ (စမ်းသပ်မှုများ၊ ပြင်ဆင်မှုများ၊ သို့မဟုတ် စာရွက်စာတမ်းများ) နှင့် ချိတ်ဆက်ထားသည်။

## လက်ခံစစ်ဆေးမှုစာရင်း

### ဝန်ဆောင်မှုပေးသူ စတင်လက်ခံခြင်း။

| စစ်ဆေး | အတည်ပြုချက် | အထောက်အထား |
|---------|------------------|----------|
| Registry သည် canonical စွမ်းရည်ကြေငြာချက်များကိုလက်ခံသည် | ပေါင်းစပ်စမ်းသပ်မှုလေ့ကျင့်ခန်း `/v1/sorafs/capacity/declare` ကို အက်ပ် API မှတစ်ဆင့်၊ လက်မှတ်ကိုင်တွယ်ခြင်း၊ မက်တာဒေတာဖမ်းယူခြင်းနှင့် node မှတ်ပုံတင်ခြင်းသို့ လက်လွှဲခြင်းတို့ကို စစ်ဆေးခြင်း။ | `crates/iroha_torii/src/routing.rs:7654` |
| စမတ်စာချုပ်သည် ကိုက်ညီမှုမရှိသော ဝန်တင်များ | ယူနစ်စမ်းသပ်မှုသည် ပံ့ပိုးပေးသူ ID များနှင့် ကျူးလွန်ထားသော GiB အကွက်များကို ဆက်လက်မလုပ်ဆောင်မီ လက်မှတ်ရေးထိုးထားသော ကြေငြာချက်နှင့် ကိုက်ညီကြောင်း သေချာစေသည်။ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI သည် တရားဝင် စတင်အသုံးပြုခြင်းဆိုင်ရာ ပစ္စည်းများကို ထုတ်လွှတ်သည် | CLI harness သည် အဆုံးအဖြတ်ပေးသော Norito/JSON/Base64 အထွက်များကို ရေးသားပြီး အသွားအပြန်ခရီးများကို အတည်ပြုပေးသောကြောင့် အော်ပရေတာများသည် အော့ဖ်လိုင်းကြေငြာချက်များကို အဆင့်မြှင့်တင်နိုင်ပါသည်။ | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| အော်ပရေတာလမ်းညွှန်သည် ဝင်ခွင့်အလုပ်အသွားအလာနှင့် အုပ်ချုပ်မှုအစောင့်အကြပ်များ | စာရွက်စာတမ်းသည် ကြေငြာချက်အစီအစဉ်၊ မူဝါဒပုံသေများနှင့် ကောင်စီအတွက် ပြန်လည်သုံးသပ်ခြင်းအဆင့်များကို စာရင်းကောက်ပါသည်။ | `../storage-capacity-marketplace.md` |

### အငြင်းပွားမှုဖြေရှင်းခြင်း။

| စစ်ဆေး | အတည်ပြုချက် | အထောက်အထား |
|---------|------------------|----------|
| အငြင်းပွားမှုမှတ်တမ်းများသည် canonical payload digest | ဖြင့် ဆက်လက်တည်ရှိနေပါသည်။ ယူနစ်စမ်းသပ်မှုသည် အငြင်းပွားမှုကို မှတ်ပုံတင်သည်၊ သိမ်းဆည်းထားသော ဝန်ဆောင်ခကို ကုဒ်လုပ်ကာ လယ်ဂျာသတ်မှတ်မှုကို အာမခံရန်အတွက် ဆိုင်းငံ့နေသောအခြေအနေကို အခိုင်အမာအတည်ပြုသည်။ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI အငြင်းပွားမှု မီးစက်သည် canonical schema | CLI စမ်းသပ်မှုသည် Base64/Norito အထွက်များနှင့် `CapacityDisputeV1` အတွက် JSON အနှစ်ချုပ်များကို အကျုံးဝင်ပြီး အထောက်အထားအစုအစည်းများကို တိကျစွာ hash ဖြင့် သေချာစေပါသည်။ | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| ပြန်လည်ကစားသည့်စမ်းသပ်မှုသည် အငြင်းပွားမှု/ပြစ်ဒဏ်သတ်မှတ်မှုကို သက်သေပြသည် | အထောက်အထား-ပျက်ကွက်သော တယ်လီမီတာကို နှစ်ကြိမ်ပြန်ဖွင့်ခြင်းသည် တူညီသော စာရင်းဇယား၊ ခရက်ဒစ်နှင့် အငြင်းပွားမှု လျှပ်တစ်ပြက်ပုံများကို ထုတ်လုပ်ပေးသောကြောင့် မျဥ်းစောင်းများသည် ရွယ်တူများအကြားတွင် အဆုံးအဖြတ်ပေးပါသည်။ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook စာရွက်စာတမ်းများ မြင့်တက်လာခြင်းနှင့် ရုတ်သိမ်းခြင်း စီးဆင်းမှု | လည်ပတ်မှုလမ်းညွှန်သည် ကောင်စီလုပ်ငန်းအသွားအလာ၊ အထောက်အထားလိုအပ်ချက်များနှင့် နောက်ပြန်ဆွဲသည့်လုပ်ငန်းစဉ်များကို ဖမ်းယူထားသည်။ | `../dispute-revocation-runbook.md` |

### Treasury Reconciliation

| စစ်ဆေး | အတည်ပြုချက် | အထောက်အထား |
|---------|------------------|----------|
| လယ်ဂျာတွင် 30 ရက်ကြာ စိမ်ထားမှု | စိမ်ထားသော စမ်းသပ်မှုသည် ပေးချေမှုဆိုင်ရာ ပြတင်းပေါက် 30 တွင် ဝန်ဆောင်မှုပေးသူ ငါးဦးကို ဖြန့်ကျက်ပြီး မျှော်မှန်းထားသည့် ပေးချေမှုဆိုင်ရာ ရည်ညွှန်းချက်နှင့် ကွဲပြားသော စာရင်းစာရွက်များကို ကွဲပြားစေသည်။ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| လယ်ဂျာ တင်ပို့မှု ပြန်လည်သင့်မြတ်ရေးကို ညစဉ်ညတိုင်း မှတ်တမ်းတင် | `capacity_reconcile.py` သည် လုပ်ဆောင်ပြီးသော XOR လွှဲပြောင်းတင်ပို့မှုများနှင့် အခကြေးငွေစာရင်းဆိုင်ရာ မျှော်လင့်ချက်များကို နှိုင်းယှဉ်ပြီး၊ Prometheus မက်ထရစ်များကို ထုတ်လွှတ်ကာ Alertmanager မှတစ်ဆင့် ဘဏ္ဍာတိုက်အတည်ပြုချက်ကို ဂိတ်ပေါက်ပေးပါသည်။ | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| ငွေတောင်းခံခြင်းဆိုင်ရာ ဒက်ရှ်ဘုတ်များတွင် ပြစ်ဒဏ်များနှင့် accrual telemetry | Grafana သွင်းကုန်မြေကွက်များ GiB·hour အမြတ်ငွေ၊ ခေါ်ဆိုမှုပေါ်ရှိ မြင်နိုင်စွမ်းအတွက် သပိတ်ကောင်တာများနှင့် ချည်နှောင်ထားသော အပေါင်ပစ္စည်း။ | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| ထုတ်ဝေသည့် အစီရင်ခံစာ မော်ကွန်းတိုက် နည်းစနစ် နှင့် ပြန်ဖွင့်သည့် အမိန့်များ | အသေးစိတ်အချက်အလက်များကို အစီရင်ခံရာတွင် စာရင်းစစ်များအတွက် နယ်ပယ်ချဲ့ထွင်မှု၊ လုပ်ဆောင်မှုအမိန့်များနှင့် စောင့်ကြည့်နိုင်မှုချိတ်များ။ | `./sf2c-capacity-soak.md` |

## အကောင်အထည်ဖော်မှုမှတ်စုများ

အကောင့်ပိတ်ခြင်းမပြုမီ validation suite ကို ပြန်ဖွင့်ပါ-

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

အော်ပရေတာများသည် `sorafs_manifest_stub capacity {declaration,dispute}` ဖြင့် စတင်ခြင်း/အငြင်းပွားမှုတောင်းဆိုမှု payload များကို ပြန်လည်ထုတ်ပေးပြီး ရရှိလာသော JSON/Norito bytes ကို အုပ်ချုပ်မှုလက်မှတ်နှင့်အတူ သိမ်းဆည်းထားသင့်သည်။

## နိမိတ်လက္ခဏာများ

| Artefact | မဂ် | blake2b-256 |
|----------|------|-------------|
| ဝန်ဆောင်မှုပေးသူ စတင်လက်ခံခြင်း အတည်ပြုချက် ပက်ကတ် | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| အငြင်းပွားမှုဖြေရှင်းရေး အတည်ပြုချက်ထုပ်ပိုး | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| ဘဏ္ဍာတိုက်ပြန်လည်သင့်မြတ်ရေး သဘောတူညီချက် | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

ဤအရာဝတ္ထုများ၏ လက်မှတ်ရေးထိုးထားသောမိတ္တူများကို ထုတ်ဝေမှုအစုအဝေးဖြင့် သိမ်းဆည်းပြီး အုပ်ချုပ်မှုပြောင်းလဲမှုမှတ်တမ်းတွင် ၎င်းတို့ကို ချိတ်ဆက်ပါ။

## ခွင့်ပြုချက်

- Storage Team Lead — @storage-tl (2026-03-24)  
- အုပ်ချုပ်ရေးကောင်စီအတွင်းရေးမှူး — @council-sec (2026-03-24)  
- Treasury Operations Lead — @treasury-ops (2026-03-24)