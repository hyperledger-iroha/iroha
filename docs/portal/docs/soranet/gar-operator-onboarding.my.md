---
lang: my
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 565d4e8bf0a043b2c83a03ec87a8c71a30da34f56d94a28cad03677963b3e69a
source_last_modified: "2025-12-29T18:16:35.206864+00:00"
translation_last_reviewed: 2026-02-07
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
translator: machine-google-reviewed
---

ထပ်ခါတလဲလဲလုပ်နိုင်သော SNNet-9 လိုက်လျောညီထွေမှုဖွဲ့စည်းပုံကို ထုတ်ပြရန် ဤအကျဉ်းကိုသုံးပါ၊
စာရင်းစစ်ဖော်ရွေသောလုပ်ငန်းစဉ်။ အော်ပရေတာတိုင်းကို တရားစီရင်ရေးဆိုင်ရာ ပြန်လည်သုံးသပ်မှုဖြင့် တွဲလိုက်ပါ။
တူညီသော digests နှင့် အထောက်အထား layout ကိုအသုံးပြုသည်။

## ခြေလှမ်းများ

1. **ဖွဲ့စည်းပုံကို စုစည်းပါ**
   - `governance/compliance/soranet_opt_outs.json` ကို တင်သွင်းပါ။
   - သင်၏ `operator_jurisdictions` ကို ထုတ်ပြန်ထားသော သက်သေခံချက်အချက်များကို ပေါင်းစပ်ပါ။
     [တရားစီရင်ရေးဆိုင်ရာ သုံးသပ်ချက်](gar-jurisdictional-review) တွင်။
2. **အတည်ပြုပါ**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - ရွေးချယ်နိုင်သော- `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **အထောက်အထားများ ဖမ်းယူရန်**
   - `artifacts/soranet/compliance/<YYYYMMDD>/` အောက်တွင် သိမ်းဆည်းပါ။
     - `config.json` (နောက်ဆုံးလိုက်နာမှုပိတ်ဆို့)
     - `attestations.json` (URIs + digests)
     - တရားဝင်မှတ်တမ်းများ
     - လက်မှတ်ထိုးထားသော PDFs/Norito စာအိတ်များအတွက် ကိုးကားချက်များ
4. **အသက်သွင်းရန်**
   - စတင်ဖြန့်ချိခြင်း (`gar-opt-out-<date>`)၊ တီးမှုတ်သူ/SDK စနစ်များကို ပြန်လည်အသုံးချရန်၊
     နှင့် `compliance_*` ဖြစ်ရပ်များကို မျှော်လင့်ထားသည့် မှတ်တမ်းများတွင် ထုတ်လွှတ်ကြောင်း အတည်ပြုပါ။
5. **ပိတ်ပါ**
   - အထောက်အထားအစုအဝေးကို အုပ်ချုပ်ရေးကောင်စီထံ တင်ပြပါ။
   - စဖွင့်ခြင်းဝင်းဒိုး + အတည်ပြုသူများကို GAR မှတ်တမ်းစာအုပ်တွင် မှတ်တမ်းတင်ပါ။
   - တရားစီရင်ရေးဆိုင်ရာပြန်လည်သုံးသပ်ရေးဇယားမှ နောက်ပြန်လည်သုံးသပ်သည့်ရက်စွဲများကို အချိန်ဇယားဆွဲပါ။

## အမြန်စစ်ဆေးရန်

- [ ] `jurisdiction_opt_outs` သည် canonical catalogue နှင့် ကိုက်ညီသည်။
- [ ] အထောက်အထား အတိအကျကို ကူးယူထားသည်။
- [ ] validation commands များ run ပြီး archived ။
- [ ] `artifacts/soranet/compliance/<date>/` တွင် သိမ်းဆည်းထားသော အထောက်အထားအတွဲ။
- [ ] လွှင့်တင် tag + GAR မှတ်တမ်းစာအုပ်ကို အပ်ဒိတ်လုပ်ထားသည်။
- [ ] နောက်-သုံးသပ်ချက်သတိပေးချက်များကို သတ်မှတ်ထားသည်။

## ကိုလည်းကြည့်ပါ။

- [GAR တရားစီရင်ရေးဆိုင်ရာ ပြန်လည်သုံးသပ်ခြင်း](gar-jurisdictional-review)
- [GAR Compliance Playbook (အရင်းအမြစ်)](../../../source/soranet/gar_compliance_playbook.md)