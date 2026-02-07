---
id: chunker-registry-charter
lang: my
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Charter
sidebar_label: Chunker Registry Charter
description: Governance charter for chunker profile submissions and approvals.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
:::

#SoraFS Chunker Registry Governance Charter

Sora ပါလီမန်အခြေခံအဆောက်အဦ panel မှ 2025-10-29 (ကြည့်ပါ
> `docs/source/sorafs/council_minutes_2025-10-29.md`)။ မည်သည့်ပြင်ဆင်မှုမဆို လိုအပ်ပါသည်။
> တရားဝင်အုပ်ချုပ်မှုမဲ၊ အကောင်အထည်ဖော်ရေးအဖွဲ့များသည် ဤစာတမ်းကို လိုက်နာရမည်ဖြစ်သည်။
> အစားထိုး ပဋိဉာဉ်ကို အတည်ပြုမပြီးမချင်း စံသတ်မှတ်ချက်။

ဤပဋိညာဉ်စာတမ်းသည် SoraFS အပိုင်းကို ပြောင်းလဲခြင်းအတွက် လုပ်ငန်းစဉ်နှင့် အခန်းကဏ္ဍများကို သတ်မှတ်ပါသည်။
မှတ်ပုံတင်။ ၎င်းသည် [Chunker Profile Authoring Guide](./chunker-profile-authoring.md) မည်ကဲ့သို့အသစ်ဖြစ်သည်ကို ဖော်ပြခြင်းဖြင့် ဖြည့်စွက်သည်

## နယ်ပယ်

ပဋိဉာဉ်သည် `sorafs_manifest::chunker_registry` နှင့် ဝင်ခွင့်တိုင်းတွင် အကျုံးဝင်ပါသည်။
မှတ်ပုံတင်ကို စားသုံးသည့် မည်သည့်ကိရိယာအတွက်မဆို (ဖော်ပြရန် CLI၊ ဝန်ဆောင်မှုပေးသူ-ကြော်ငြာ CLI၊
SDKs)။ ၎င်းသည် alias ကို တွန်းအားပေးပြီး စစ်ဆေးထားသော ပုံစံကွဲများကို ကိုင်တွယ်သည်။
`chunker_registry::ensure_charter_compliance()`-

- ပရိုဖိုင် ID များသည် monotonically တိုးလာသော အပြုသဘောဆောင်သော ကိန်းပြည့်များဖြစ်သည်။
- Canonical handle `namespace.name@semver` ** must** သည် ပထမဆုံးအဖြစ် ပေါ်လာသည် ။
- Alias ကြိုးများကို ဖြတ်ညှပ်ကပ်ထားပြီး ထူးခြားပြီး Canonical လက်ကိုင်များနှင့် မတိုက်မိပါစေနှင့်
  အခြားပါဝင်မှုများ။

## ရာထူးတာဝန်များ

- **စာရေးသူ(များ)** - အဆိုပြုချက်အား ပြင်ဆင်ပါ၊ ပြင်ဆင်မှုများ၊ ပြင်ဆင်မှုများ ပြုလုပ်ပြီး စုဆောင်းပါ။
  အဆုံးအဖြတ် အထောက်အထား။
- **Tooling Working Group (TWG)** - ထုတ်ဝေထားသော အဆိုပြုချက်အား အတည်ပြုသည်။
  စာရင်းစစ်ဆေးပြီး registry invariants များကို ထိန်းသိမ်းထားကြောင်း သေချာစေသည်။
- ** အုပ်ချုပ်ရေးကောင်စီ (GC)** – TWG အစီရင်ခံစာကို ပြန်လည်သုံးသပ်ပြီး အဆိုပြုချက်ကို လက်မှတ်ရေးထိုးသည်။
  စာအိတ်၊ ထုတ်ဝေခြင်း/ကန့်ကွက်ခြင်း အချိန်ဇယားများကို အတည်ပြုသည်။
- **Storage Team** - မှတ်ပုံတင်ခြင်းကို အကောင်အထည်ဖော်ခြင်းနှင့် ထုတ်ဝေမှုများကို ထိန်းသိမ်းသည်။
  စာရွက်စာတမ်းအပ်ဒိတ်များ။

## Lifecycle လုပ်ငန်းအသွားအလာ

1. **အဆိုပြုလွှာတင်သွင်းခြင်း**
   - စာရေးသူသည် စာရေးဆရာလမ်းညွှန်မှ အတည်ပြုချက်စာရင်းကို လုပ်ဆောင်ပြီး ဖန်တီးသည်။
     `ChunkerProfileProposalV1` JSON အောက်
     `docs/source/sorafs/proposals/`။
   - CLI output မှ-
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - ပြိုင်ပွဲများ၊ အဆိုပြုချက်၊ အဆုံးအဖြတ်အစီရင်ခံစာနှင့် မှတ်ပုံတင်ခြင်းပါ၀င်သော PR ကို တင်သွင်းပါ။
     အပ်ဒိတ်များ

2. **Tooling Review (TWG)**
   - တရားဝင်စစ်ဆေးရန်စာရင်း (ပစ္စည်များ၊ fuzz၊ manifest/PoR ပိုက်လိုင်း) ကို ပြန်ဖွင့်ပါ။
   - `cargo test -p sorafs_car --chunker-registry` ကို run ပြီးသေချာအောင်လုပ်ပါ။
     `ensure_charter_compliance()` သည် ဝင်ခွင့်အသစ်ဖြင့် အောင်မြင်သွားသည်။
   - CLI အပြုအမူကို အတည်ပြုပါ (`--list-profiles`၊ `--promote-profile`၊ တိုက်ရိုက်ကြည့်ရှုခြင်း
     `--json-out=-`) သည် အပ်ဒိတ်လုပ်ထားသော နာမည်များနှင့် လက်ကိုင်များကို ထင်ဟပ်စေသည်။
   - တွေ့ရှိချက်များနှင့် ဖြတ်/မအောင်မြင်မှု အခြေအနေတို့ကို အကျဉ်းချုပ် အစီရင်ခံစာတိုတစ်ခု ထုတ်ပါ။

3. **ကောင်စီခွင့်ပြုချက် (GC)**
   - TWG အစီရင်ခံစာနှင့် အဆိုပြုချက် မက်တာဒေတာကို ပြန်လည်သုံးသပ်ပါ။
   - အဆိုပြုချက်အမှတ်အသား (`blake3("sorafs-chunker-profile-v1" || bytes)`) ကို လက်မှတ်ရေးထိုးပါ။
     နှင့် တွဲလျက် ထိန်းသိမ်းထားသော ကောင်စီစာအိတ်တွင် လက်မှတ်များ ပါ၀င်သည်။
     ပွဲစဉ်များ
   - အုပ်ချုပ်မှုမိနစ်များတွင် မဲရလဒ်ကို မှတ်တမ်းတင်ပါ။

4. **ထုတ်ဝေမှု**
   - PR ကို ပေါင်းစည်းပြီး မွမ်းမံခြင်း၊
     - `sorafs_manifest::chunker_registry_data`။
     - စာရွက်စာတမ်း (`chunker_registry.md`၊ ရေးသားခြင်း/ကိုက်ညီမှုလမ်းညွှန်များ)။
     - ပြင်ဆင်မှုများနှင့် အဆုံးအဖြတ်အစီရင်ခံစာများ။
   - ပရိုဖိုင်အသစ်အတွက် အော်ပရေတာများနှင့် SDK အဖွဲ့များကို အကြောင်းကြားပြီး စတင်ဖြန့်ချိရန် စီစဉ်ထားသည်။

5. **လျှော့စျေး/နေဝင်ချိန်**
   - ရှိပြီးသားပရိုဖိုင်ကို အစားထိုးသည့် အဆိုပြုချက်များတွင် ထုတ်ဝေမှုနှစ်ခု ပါဝင်ရမည်။
     ဝင်းဒိုး (ကျေးဇူးတော်ကာလ) နှင့် အဆင့်မြှင့်တင်မှု အစီအစဉ်။
     မှတ်ပုံတင်စာရင်းတွင် ရွှေ့ပြောင်းခြင်းဆိုင်ရာ စာရင်းဇယားကို အပ်ဒိတ်လုပ်ပါ။

6. **အရေးပေါ်ပြောင်းလဲမှုများ**
   - ဖယ်ရှားခြင်း သို့မဟုတ် ပြင်ဆင်ခြင်းများသည် အများစု၏ ထောက်ခံချက်ဖြင့် ကောင်စီမဲတစ်ခု လိုအပ်သည်။
   - TWG သည် အန္တရာယ်လျော့ပါးရေးအဆင့်များကို မှတ်တမ်းတင်ပြီး အဖြစ်အပျက်မှတ်တမ်းကို အပ်ဒိတ်လုပ်ရပါမည်။

## Tooling မျှော်မှန်းချက်များ

- `sorafs_manifest_chunk_store` နှင့် `sorafs_manifest_stub` ဖော်ထုတ်ရန်-
  - မှတ်ပုံတင်ခြင်းစစ်ဆေးခြင်းအတွက် `--list-profiles`။
  - အသုံးပြုထားသော canonical metadata block ကိုထုတ်လုပ်ရန် `--promote-profile=<handle>`
    ပရိုဖိုင်ကို ကြော်ငြာတဲ့အခါ။
  - `--json-out=-` သည် stdout သို့ အစီရင်ခံစာများကို တိုက်ရိုက်ထုတ်လွှင့်ရန်၊ ပြန်လည်ထုတ်လုပ်နိုင်သော ပြန်လည်သုံးသပ်မှုကို ဖွင့်ပေးသည်
    သစ်လုံးများ
- `ensure_charter_compliance()` ကို သက်ဆိုင်ရာ binaries များတွင် စတင်ချိန်တွင် ခေါ်ဆိုပါသည်။
  (`manifest_chunk_store`၊ `provider_advert_stub`)။ CI စစ်ဆေးမှု အသစ်များ ပျက်ကွက်ပါက ဖြေဆိုရပါမည်။
  စာတမ်းများသည် ပဋိညာဉ်ကို ဖောက်ဖျက်ပါသည်။

## မှတ်တမ်းထားရှိခြင်း။

- `docs/source/sorafs/reports/` တွင် အဆုံးအဖြတ်ပေးသည့် အစီရင်ခံစာအားလုံးကို သိမ်းဆည်းပါ။
- ကောင်စီ၏ အကိုးအကား မိနစ်များတွင် chunker ဆုံးဖြတ်ချက်များ အောက်တွင် နေထိုင်ပါသည်။
  `docs/source/sorafs/migration_ledger.md`။
- အဓိက registry တစ်ခုစီ ပြောင်းလဲပြီးနောက် `roadmap.md` နှင့် `status.md` ကို အပ်ဒိတ်လုပ်ပါ။

## ကိုးကား

- စာရေးဆရာလမ်းညွှန်- [Chunker Profile Authoring Guide](./chunker-profile-authoring.md)
- ကိုက်ညီမှုစစ်ဆေးစာရင်း- `docs/source/sorafs/chunker_conformance.md`
- မှတ်ပုံတင်ခြင်းအကိုးအကား- [Chunker Profile Registry](./chunker-registry.md)