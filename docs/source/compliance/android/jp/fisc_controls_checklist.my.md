---
lang: my
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2025-12-29T18:16:35.928660+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC လုံခြုံရေးထိန်းချုပ်မှုစာရင်း - Android SDK

| လယ် | တန်ဖိုး |
|---------|-------|
| ဗားရှင်း | 0.1 (2026-02-12) |
| နယ်ပယ် | ဂျပန်ဘဏ္ဍာရေးဆိုင်ရာ ဖြန့်ကျက်မှုများတွင် အသုံးပြုသည့် Android SDK + အော်ပရေတာတူးလ်
| ပိုင်ရှင်များ | လိုက်နာမှုနှင့် ဥပဒေ (Daniel Park)၊ Android ပရိုဂရမ် ဦးဆောင် |

## ထိန်းချုပ်မက်ထရစ်

| FISC ထိန်းချုပ်မှု | အကောင်အထည်ဖော်မှုအသေးစိတ် | အထောက်အထားများ / ကိုးကားချက်များ | အဆင့်အတန်း |
|--------------|--------------------------------|--------------------------------|--------|
| **System configuration integrity** | `ClientConfig` သည် manifest hashing, schema validation, and read-only runtime access. စီစဉ်သတ်မှတ်မှု ပြန်လည်စတင်ခြင်း မအောင်မြင်မှုများသည် အပြေးစာအုပ်တွင် မှတ်တမ်းတင်ထားသော `android.telemetry.config.reload` ဖြစ်ရပ်များကို ထုတ်လွှတ်သည်။ | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2။ | ✅ ဟိုဟာ |
| **ဝင်ရောက်ထိန်းချုပ်မှုနှင့် အထောက်အထားစိစစ်ခြင်း** | SDK သည် Torii TLS မူဝါဒများနှင့် `/v1/pipeline` လက်မှတ်ထိုးတောင်းဆိုမှုများကို ဂုဏ်ပြုပါသည်။ အော်ပရေတာ အလုပ်အသွားအလာများ တိုးလာမှုအတွက် ပံ့ပိုးကူညီမှု Playbook §4–5 နှင့် Norito လက်မှတ်ရေးထိုးထားသော အထောက်အထားများမှတစ်ဆင့် ဂိတ်ပေါက်များကို ထပ်ရေးပါ။ | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (အလုပ်အသွားအလာကို အစားထိုးသည်)။ | ✅ Implemented |
| ** ကူးယူဖော်ပြသောသော့စီမံခန့်ခွဲမှု** | StrongBox-နှစ်သက်သောဝန်ဆောင်မှုပေးသူများ၊ သက်သေခံချက်အတည်ပြုချက်နှင့် ကိရိယာမက်ထရစ်အကျုံးဝင်မှုသည် KMS လိုက်နာမှုကို သေချာစေသည်။ `artifacts/android/attestation/` အောက်တွင် သိမ်းဆည်းပြီး အဆင်သင့်မက်ထရစ်တွင် ခြေရာခံထားသည့် အထောက်အထားကြိုးကြိုးအထွက်များ။ | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`။ | ✅ ဟိုဟာ |
| **သစ်ခုတ်ခြင်း၊ စောင့်ကြည့်ခြင်းနှင့် ထိန်းသိမ်းခြင်း** | တယ်လီမက်ထရီ တုံ့ပြန်မှုမူဝါဒသည် အထိခိုက်မခံသောဒေတာကို ဟက်ကာ၊ စက်ပစ္စည်း၏အရည်အသွေးများကို ပုံးဆွဲပြီး ထိန်းသိမ်းမှုကို တွန်းအားပေးသည် (7/30/90/365 ရက်ကြာဝင်းဒိုးများ)။ ပံ့ပိုးမှု Playbook §8 သည် ဒက်ရှ်ဘုတ် သတ်မှတ်ချက်များကို ဖော်ပြသည်; `telemetry_override_log.md` တွင် မှတ်တမ်းတင်ထားသော overrides | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`။ | ✅ ဟိုဟာ |
| **လုပ်ငန်းဆောင်တာများနှင့် အပြောင်းအလဲစီမံခန့်ခွဲမှု** | GA ဖြတ်တောက်ခြင်းလုပ်ငန်းစဉ် (Support Playbook §7.2) နှင့် `status.md` အပ်ဒိတ်များ ဖြန့်ချိရန် အဆင်သင့်ခြေရာခံ။ `docs/source/compliance/android/eu/sbom_attestation.md` မှတစ်ဆင့် ချိတ်ဆက်ထားသော အထောက်အထားများ (SBOM၊ Sigstore) ကို ထုတ်ပြန်ပါ။ | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`။ | ✅ ဟိုဟာ |
| **အခင်းဖြစ်ပွားမှုနှင့် တုံ့ပြန်ချက်** | Playbook သည် ပြင်းထန်မှုမက်ထရစ်၊ SLA တုံ့ပြန်မှုပြတင်းပေါက်များနှင့် လိုက်နာမှုသတိပေးချက်အဆင့်များကို သတ်မှတ်သည်။ telemetry overrides + ပရမ်းပတာ အစမ်းလေ့ကျင့်မှုများသည် လေယာဉ်မှူးများရှေ့တွင် မျိုးပွားနိုင်မှုကို သေချာစေသည်။ | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`။ | ✅ ဟိုဟာ |
| **ဒေတာနေထိုင်ခွင့်/ဒေသသတ်မှတ်ခြင်း** | JP ဖြန့်ကျက်မှုအတွက် တယ်လီမီတာ စုဆောင်းသူများ၊ StrongBox သက်သေခံချက်အစုအဝေးများကို ဒေသအတွင်း သိမ်းဆည်းထားပြီး ပါတနာလက်မှတ်များမှ ကိုးကားထားသည်။ ဘီတာ (AND5) မတိုင်မီ ဂျပန်ဘာသာစကားဖြင့် စာရွက်စာတမ်းများရရှိနိုင်ကြောင်း ဒေသန္တရပြုခြင်းအစီအစဉ်ကို သေချာစေသည်။ | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`။ | 🈺 In Progress (ဒေသအလိုက် လုပ်ဆောင်ဆဲ) |

## ပြန်လည်သုံးသပ်သူ မှတ်စုများ

- ထိန်းချုပ်ထားသော ပါတနာ စတင်မ၀င်မီ Galaxy S23/S24 အတွက် စက်ပစ္စည်း-မက်ထရစ်များကို စစ်ဆေးအတည်ပြုပါ (အဆင်သင့် doc အတန်း `s23-strongbox-a`၊ `s24-strongbox-a` ကိုကြည့်ပါ)။
- JP ဖြန့်ကျက်မှုများတွင် တယ်လီမီတာ စုဆောင်းသူများအား DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`) တွင် သတ်မှတ်ထားသော တူညီသော ထိန်းသိမ်းမှု/ပယ်ဖျက်ခြင်း ယုတ္တိကို တွန်းအားပေးကြောင်း သေချာပါစေ။
- Capture confirmation from external auditors once banking partners review this checklist.