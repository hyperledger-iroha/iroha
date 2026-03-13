---
id: nexus-fee-model
lang: my
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus fee model updates
description: Mirror of `docs/source/nexus_fee_model.md`, documenting the lane settlement receipts and reconciliation surfaces.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
ဤစာမျက်နှာသည် `docs/source/nexus_fee_model.md` ဖြစ်သည်။ ဂျပန်၊ ဟီဘရူး၊ စပိန်၊ ပေါ်တူဂီ၊ ပြင်သစ်၊ ရုရှ၊ အာရဗီနှင့် အူရဒူ ဘာသာပြန်များ ပြောင်းရွှေ့နေချိန်တွင် မိတ္တူနှစ်ခုလုံးကို ချိန်ညှိထားပါ။
:::

# Nexus အခကြေးငွေ မော်ဒယ် အပ်ဒိတ်များ

တစ်စုတစ်စည်းတည်း အခြေချနေထိုင်သည့် router သည် ယခုအချိန်တွင် အဆုံးအဖြတ်ပေးသော လမ်းသွားဖြတ်ပိုင်းများကို ဖမ်းယူပါသည်။
အော်ပရေတာများသည် Nexus အခကြေးငွေမော်ဒယ်နှင့် ဓာတ်ငွေ့ကြွေးမြီများကို ညှိနှိုင်းနိုင်ပါသည်။

- အပြည့်အဝ router ဗိသုကာ၊ ကြားခံမူဝါဒ၊ တယ်လီမီတာမက်ထရစ်နှင့် ဖြန့်ချိမှုအတွက်
  sequencing `docs/settlement-router.md` ကိုကြည့်ပါ။ ဒီလမ်းညွှန်ချက်က ဘယ်လိုနည်းနဲ့ ရှင်းပြတယ်။
  ဤနေရာတွင် မှတ်တမ်းတင်ထားသော ကန့်သတ်ချက်များသည် NX-3 လမ်းပြမြေပုံနှင့် SREs တို့ကို မည်သို့ဆက်စပ်မည်နည်း။
  ထုတ်လုပ်မှုတွင် router ကိုစောင့်ကြည့်သင့်သည်။
- Gas asset configuration (`pipeline.gas.units_per_gas`) တစ်ခု ပါဝင်သည်။
  `twap_local_per_xor` ဒသမ `liquidity_profile` (`tier1`၊ `tier2`၊
  သို့မဟုတ် `tier3`) နှင့် `volatility_class` (`stable`၊ `elevated`၊ `dislocated`)။
  ဤအလံများသည် အခြေချရောက်တာအား ဖြည့်သွင်းသောကြောင့် ရလဒ် XOR ဖြစ်သည်။
  ကိုးကားချက်သည် လမ်းသွားအတွက် canonical TWAP နှင့် ဆံပင်ညှပ်အဆင့်နှင့် ကိုက်ညီသည်။
- ဓာတ်ငွေ့ပေးဆောင်သည့် အရောင်းအ၀ယ်တိုင်းတွင် `LaneSettlementReceipt` မှတ်တမ်းရှိသည်။  အသီးသီး
  ပြေစာသည် ခေါ်ဆိုသူမှပေးသော ရင်းမြစ်သတ်မှတ်မှုစနစ်၊ ဒေသတွင်း မိုက်ခရိုပမာဏ၊
  XOR ချက်ခြင်းကျသင့်သည်၊ ဆံပင်ညှပ်ပြီးနောက်မျှော်လင့်ထားသည့် XOR သည်သဘောပေါက်သည်။
  ကွဲပြားမှု (`xor_variance_micro`) နှင့် ပိတ်ဆို့အချိန်တံဆိပ်ကို မီလီစက္ကန့်များအတွင်း။
- လမ်းသွား/ဒေတာနေရာလွတ်အလိုက် လက်ခံဖြတ်ပိုင်းများကို စုစည်းပြီး ၎င်းတို့ကို ထုတ်ဝေပါ။
  `lane_settlement_commitments` မှတဆင့် `/v2/sumeragi/status`။  စုစုပေါင်း
  `total_local_micro`၊ `total_xor_due_micro`၊ နှင့်
  `total_xor_after_haircut_micro` သည် ညတိုင်းအတွက် ဘလောက်ကို ခြုံငုံထားသည်။
  ပြန်လည်သင့်မြတ်ရေး တင်ပို့မှု။
- `total_xor_variance_micro` ကောင်တာအသစ်တစ်ခုသည် ဘေးကင်းလုံခြုံရေးအနားသတ်မည်မျှရှိသည်ကို ခြေရာခံသည်။
  စားသုံးခဲ့သည် (XOR နှင့်ဆံပင်ညှပ်ပြီးနောက်မျှော်လင့်ချက်အကြားကွာခြားချက်)
  နှင့် `swap_metadata` သည် အဆုံးအဖြတ်ပြောင်းလဲခြင်း ဘောင်များကို မှတ်တမ်းတင်ထားသည်။
  (TWAP၊ epsilon၊ ငွေဖြစ်လွယ်မှုပရိုဖိုင်၊ နှင့် volatility_class) သို့မှသာ စာရင်းစစ်များလုပ်နိုင်သည်
  runtime configuration မပါဘဲကိုးကားထည့်သွင်းမှုများကိုစစ်ဆေးပါ။

စားသုံးသူများသည် လက်ရှိလမ်းကြောတစ်လျှောက် `lane_settlement_commitments` ကို ကြည့်ရှုနိုင်သည်။
အခကြေးငွေကြားခံများ၊ ဆံပင်ညှပ်အဆင့်များကို အတည်ပြုရန် dataspace ကတိကဝတ်လျှပ်တစ်ပြက်များ၊
နှင့် လဲလှယ်မှုလုပ်ဆောင်မှုသည် ပြင်ဆင်သတ်မှတ်ထားသော Nexus အခကြေးငွေမော်ဒယ်နှင့် ကိုက်ညီပါသည်။