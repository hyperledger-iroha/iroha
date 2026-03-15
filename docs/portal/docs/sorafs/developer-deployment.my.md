---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cac03d504c6a7dcfacaa4b298e14f0a71ccbcb5ec58f1977b5bf124300c8ec61
source_last_modified: "2026-01-05T09:28:11.865094+00:00"
translation_last_reviewed: 2026-02-07
id: developer-deployment
title: SoraFS Deployment Notes
sidebar_label: Deployment Notes
description: Checklist for promoting the SoraFS pipeline from CI to production.
translator: machine-google-reviewed
---

::: Canonical Source ကို သတိပြုပါ။
:::

# ဖြန့်ကျက်မှတ်စုများ

SoraFS ထုပ်ပိုးမှုလုပ်ငန်းအသွားအလာသည် ဆုံးဖြတ်ချက်ခိုင်မာစေသောကြောင့် CI မှပြောင်းသွားသည်
ထုတ်လုပ်မှုသည် အဓိကအားဖြင့် လည်ပတ်မှုဆိုင်ရာ အစောင့်အကြပ်များ လိုအပ်သည်။ ဤစစ်ဆေးရန်စာရင်းကို အသုံးပြုပါ။
ကိရိယာတန်ဆာပလာကို စစ်မှန်သော တံခါးပေါက်များနှင့် သိုလှောင်မှု ပံ့ပိုးပေးသူများထံ ဖြန့်ချီခြင်း။

## လေယာဉ်အကြို

- **Registry alignment** — chunker profiles ကို အတည်ပြုပြီး manifests များကို ကိုးကားပါ။
  တူညီသော `namespace.name@semver` tuple (`docs/source/sorafs/chunker_registry.md`)။
- **ဝင်ခွင့်မူဝါဒ** — လက်မှတ်ရေးထိုးထားသော ပံ့ပိုးပေးသူ ကြော်ငြာများနှင့် နာမည်တူအထောက်အထားများကို ပြန်လည်သုံးသပ်ပါ။
  `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`) အတွက် လိုအပ်ပါသည်။
- **Pin registry runbook** — `docs/source/sorafs/runbooks/pin_registry_ops.md` ကိုထားပါ။
  ပြန်လည်ရယူခြင်းအခြေအနေများ (အမည်များလှည့်ခြင်း၊ ကူးယူခြင်း မအောင်မြင်မှုများ) အတွက် အဆင်ပြေသည်။

## ပတ်ဝန်းကျင်ဖွဲ့စည်းမှု

- Gateways သည် အထောက်အထား streaming endpoint (`POST /v2/sorafs/proof/stream`) ကို ဖွင့်ထားရမည်
  ထို့ကြောင့် CLI သည် telemetry အနှစ်ချုပ်များကို ထုတ်လွှတ်နိုင်သည်။
- ပုံသေများတွင် `sorafs_alias_cache` မူဝါဒကို ပြင်ဆင်ပါ။
  `iroha_config` သို့မဟုတ် CLI အကူအညီပေးသူ (`sorafs_cli manifest submit --alias-*`)။
- လုံခြုံသောလျှို့ဝှက်မန်နေဂျာမှတစ်ဆင့် တိုက်ရိုက်ထုတ်လွှင့်မှုတိုကင်များ (သို့မဟုတ် Torii အထောက်အထားများ) ပေးပါ။
- တယ်လီမီတာတင်ပို့သူများကို ဖွင့်ပါ (`torii_sorafs_proof_stream_*`၊
  `torii_sorafs_chunk_range_*`) နှင့် သင်၏ Prometheus/OTel stack သို့ ပို့ဆောင်ပါ။

## မဟာဗျူဟာ ရေးဆွဲခြင်း။

1. **အပြာ/အစိမ်း ဖော်ပြချက်များ**
   - စတင်ဖြန့်ချိမှုတစ်ခုစီအတွက် တုံ့ပြန်ချက်များကို သိမ်းဆည်းရန် `manifest submit --summary-out` ကို အသုံးပြုပါ။
   - စွမ်းရည်ကိုဖမ်းစားရန် `torii_sorafs_gateway_refusals_total` ကိုစောင့်ကြည့်ပါ။
     စောစောစီးစီး မတိုက်ဆိုင်ပါ။
2. ** သက်သေအထောက်အထား **
   - `sorafs_cli proof stream` တွင် မအောင်မြင်မှုများကို ဖြန့်ကျက်ပိတ်ဆို့သူများအဖြစ် ဆက်ဆံပါ။ စောင့်နေချိန်
     spikes များသည် ဝန်ဆောင်မှုပေးသူကို ပိတ်ဆို့ခြင်း သို့မဟုတ် မှားယွင်းသတ်မှတ်ထားသော အဆင့်များကို ညွှန်ပြလေ့ရှိသည်။
   - CAR ကိုသေချာစေရန် `proof verify` သည် post-pin မီးခိုးစမ်းသပ်မှု၏တစ်စိတ်တစ်ပိုင်းဖြစ်သင့်သည်
     ဝန်ဆောင်မှုပေးသူများမှ စီစဉ်ပေးထားသည့် မန်နီးဖက်စ် အချေအတင်နှင့် ကိုက်ညီနေသေးသည်။
3. **တယ်လီမီတာ ဒက်ရှ်ဘုတ်များ**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` ကို Grafana သို့ တင်သွင်းပါ။
   - pin registry ကျန်းမာရေးအတွက် ထပ်လောင်းအကန့်များ အလွှာ
     (`docs/source/sorafs/runbooks/pin_registry_ops.md`) နှင့် အတုံးအကွာအဝေး ကိန်းဂဏန်းများ။
4. **ရင်းမြစ်ပေါင်းစုံဖွင့်ခြင်း**
   - အဆင့်မြှင့်တင်မှု အဆင့်များကို လိုက်နာပါ။
     `docs/source/sorafs/runbooks/multi_source_rollout.md` ကိုဖွင့်သောအခါ
     သံစုံတီးဝိုင်း၊ နှင့် စာရင်းစစ်များအတွက် အမှတ်စာရင်းဘုတ်/ကြေးနန်းဆိုင်ရာ အနုပညာပစ္စည်းများကို သိမ်းဆည်းပါ။

## အဖြစ်အပျက်ကို ကိုင်တွယ်ဖြေရှင်းခြင်း။

- `docs/source/sorafs/runbooks/` ရှိ တိုးမြင့်လာမှုလမ်းကြောင်းများကို လိုက်နာပါ-
  - ဂိတ်ဝေးပြတ်တောက်မှုနှင့် stream-token အတွက် `sorafs_gateway_operator_playbook.md`
    ပင်ပန်းနွမ်းနယ်ခြင်း။
  ကူးယူမှုဆိုင်ရာ အငြင်းပွားမှုများ ဖြစ်ပေါ်သည့်အခါ - `dispute_revocation_runbook.md`။
  - node အဆင့်ထိန်းသိမ်းမှုအတွက် `sorafs_node_ops.md`။
  - သံစုံတီးဝိုင်းကို အစားထိုးခြင်း၊ သက်တူရွယ်တူများကို အမည်ပျက်စာရင်းသွင်းခြင်းအတွက် `multi_source_rollout.md` နှင့်
    အဆင့်လိုက် ဖြန့်ချိမှုများ။
- ရှိရင်းစွဲမှတစ်ဆင့် GovernanceLog ရှိ အထောက်အထားပျက်ကွက်မှုများနှင့် latency ကွဲလွဲချက်များကို မှတ်တမ်းတင်ပါ။
  PoR tracker APIs များကို အုပ်ချုပ်မှုမှ ပံ့ပိုးပေးသူ၏ စွမ်းဆောင်ရည်ကို အကဲဖြတ်နိုင်စေရန်။

## နောက်တစ်ဆင့်

- သံစုံတီးဝိုင်းအော်တိုစနစ် (`sorafs_car::multi_fetch`) ကို တစ်ကြိမ် ပေါင်းစပ်ပါ။
  multi-source fetch orchestrator (SF-6b) မြေများ။
- SF-13/SF-14 အောက်တွင် PDP/PoTR အဆင့်မြှင့်တင်မှုများကို ခြေရာခံပါ။ CLI နှင့် docs သည် ပြောင်းလဲလာလိမ့်မည်။
  ထိုအထောက်အထားများ တည်ငြိမ်သွားသည်နှင့် မျက်နှာပြင် သတ်မှတ်ရက်များနှင့် အဆင့်ရွေးချယ်မှု။

ဤအသုံးပြုမှုမှတ်စုများကို အမြန်စတင်ခြင်းနှင့် CI ချက်ပြုတ်နည်းများဖြင့် ပေါင်းစပ်ခြင်းဖြင့်၊ အဖွဲ့များ
ဒေသတွင်း စမ်းသပ်မှုများမှ ထုတ်လုပ်မှုအဆင့် SoraFS ပိုက်လိုင်းများဆီသို့ ရွှေ့နိုင်သည်။
ထပ်ခါတလဲလဲ၊ စောင့်ကြည့်နိုင်သော လုပ်ငန်းစဉ်။