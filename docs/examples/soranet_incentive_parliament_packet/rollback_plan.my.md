---
lang: my
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 47b6ac4be21202943d4145c604557a2ee50823acc139633dd6cf690a81cbce8e
source_last_modified: "2026-01-22T14:35:37.885394+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Relay Incentive Rollback Plan

အုပ်ချုပ်ရေးမှ တောင်းဆိုလာပါက အလိုအလျောက် ထပ်ဆင့်ပေးချေမှုများကို ပိတ်ရန် ဤဖွင့်စာအုပ်ကို အသုံးပြုပါ။
သို့မဟုတ် telemetry guardrails မီးလောင်လျှင်ရပ်ပါ။

1. **Freeze automation.** တီးခတ်သူလုပ်သူတိုင်းရှိ မက်လုံးများ daemon ကို ရပ်ပါ
   (`systemctl stop soranet-incentives.service` သို့မဟုတ် ညီမျှသော ကွန်တိန်နာ
   ဖြန့်ကျက်ခြင်း) နှင့် လုပ်ငန်းစဉ်သည် မလည်ပတ်တော့ကြောင်း အတည်ပြုပါ။
2. **Drain ကို ဆိုင်းငံ့ထားသော ညွှန်ကြားချက်များ** ကို Run ပါ။
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   ထူးထူးခြားခြား ငွေပေးချေမှု ညွှန်ကြားချက်များ မရှိစေရန် သေချာစေရန်။ ရလဒ်ကို သိမ်းဆည်းပါ။
   စာရင်းစစ်အတွက် Norito ပေးချေမှုများ။
3. **အုပ်ချုပ်မှုခွင့်ပြုချက်ကို ရုပ်သိမ်းပါ။** ပြင်ဆင်ရန် `reward_config.json`၊ သတ်မှတ်
   `"budget_approval_id": null` နှင့် configuration ကို ပြန်လည်အသုံးပြုပါ။
   `iroha app sorafs incentives service init` (သို့မဟုတ် `update-config` ကိုအသုံးပြုပါက၊
   တာရှည်နတ်ဆိုး)။ ငွေပေးချေမှုအင်ဂျင်သည် ယခုပိတ်၍မရပါ။
   `MissingBudgetApprovalId`၊ ထို့ကြောင့် daemon သည် အသစ်တစ်ခုမပြီးမချင်း mint ပေးချေမှုများကို ငြင်းဆန်သည်
   အတည်ပြုချက် hash ကို ပြန်ယူသည်။ git commit နှင့် SHA-256 ကို မှတ်တမ်းတင်ပါ။
   အဖြစ်အပျက်မှတ်တမ်းတွင် ပြင်ဆင်ထားသော config ကို။
4. **Sora လွှတ်တော်ကို အကြောင်းကြားပါ။** ထုတ်ယူထားသော ငွေပေးချေမှုစာရင်းဇယား၊ အရိပ်ပြေးကို ပူးတွဲပါ
   အစီရင်ခံစာနှင့် အဖြစ်အပျက်အကျဉ်းချုပ်။ လွှတ်တော် မိနစ်များတွင် ဟက်ရ်ှကို မှတ်သားထားရမည်။
   ရုပ်သိမ်းထားသောဖွဲ့စည်းပုံနှင့် daemon ရပ်တန့်ချိန်။
5. ** ပြန်သိမ်းခြင်း မှန်ကန်မှု။** daemon ကို ပိတ်ထားသည်အထိ ထားပါ-
   - တယ်လီမီတာသတိပေးချက်များ (`soranet_incentives_rules.yml`) သည် >=24 နာရီအတွက် အစိမ်းရောင်ဖြစ်သည်၊
   - ဘဏ္ဍာတိုက်ပြန်လည်သင့်မြတ်ရေးအစီရင်ခံစာတွင် ပျောက်ဆုံးနေသောလွှဲပြောင်းမှုများကို သုညနှင့်ပြသထားသည်။
   - လွှတ်တော်က ဘတ်ဂျက် hash အသစ်ကို အတည်ပြုသည်။

အုပ်ချုပ်ရေးမှ ဘတ်ဂျက်ခွင့်ပြုချက် hash ကို ပြန်လည်ထုတ်ပေးပြီးသည်နှင့်၊ `reward_config.json` ကို အပ်ဒိတ်လုပ်ပါ။
နောက်ဆုံးပေါ် telemetry တွင် `shadow-run` command ကို ပြန်လည် run၊
နှင့် incentives daemon ကို ပြန်လည်စတင်ပါ။