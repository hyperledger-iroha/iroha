---
lang: my
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e992cea8d0c835b30bd9e91860f6b6f87bed79a2c25bd6d0544639685834f80c
source_last_modified: "2025-12-29T18:16:35.146583+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-settlement-faq
title: Settlement FAQ
description: Operator-facing answers covering settlement routing, XOR conversion, telemetry, and audit evidence.
translator: machine-google-reviewed
---

ဤစာမျက်နှာသည် အတွင်းပိုင်းဖြေရှင်းမှု FAQ (`docs/source/nexus_settlement_faq.md`) ကို ထင်ဟပ်စေသည်
ထို့ကြောင့် ပေါ်တယ်စာဖတ်သူများသည် အဆိုပါလမ်းညွှန်ချက်ကို တူးဖော်ခြင်းမပြုဘဲ တူညီသောလမ်းညွှန်ချက်ကို ပြန်လည်သုံးသပ်နိုင်သည်။
mono-repo ၎င်းသည် ငွေပေးချေမှုဆိုင်ရာ Router မှ ပေးချေမှုများကို မည်သို့လုပ်ဆောင်သည်၊ မည်သို့သော တိုင်းတာမှုများကို ရှင်းပြထားသည်။
စောင့်ကြည့်ရန်၊ နှင့် SDKs များသည် Norito payloads များကို မည်သို့ပေါင်းစပ်သင့်သနည်း။

## ပေါ်လွင်ချက်များ

1. **လမ်းကြောမြေပုံဆွဲခြင်း** — ဒေတာနေရာတစ်ခုစီသည် `settlement_handle` ကို ကြေညာသည်
   (`xor_global`၊ `xor_lane_weighted`၊ `xor_hosted_custody` သို့မဟုတ်
   `xor_dual_fund`)။ အောက်မှာ နောက်ဆုံးလမ်းသွား ကက်တလောက်နဲ့ တိုင်ပင်ပါ။
   `docs/source/project_tracker/nexus_config_deltas/`။
2. **Deterministic conversion** — router သည် အခြေချနေထိုင်မှုအားလုံးကို XOR မှတဆင့် ပြောင်းပေးသည်။
   အုပ်ချုပ်မှုမှ အတည်ပြုထားသော ငွေဖြစ်လွယ်မှု အရင်းအမြစ်များ။ ကိုယ်ပိုင်လမ်းများ ကြိုတင်ရန်ပုံငွေ XOR ကြားခံများ;
   မူဝါဒပြင်ပတွင် ကြားခံများ လွင့်ပျံလာသောအခါမှသာ ဆံပင်ညှပ်ခြင်းများ သက်ရောက်သည်။
3. **Telemetry** — နာရီ `nexus_settlement_latency_seconds`၊ ပြောင်းလဲခြင်းကောင်တာများ၊
   ဆံပင်ညှပ်ကိရိယာများ။ ဒက်ရှ်ဘုတ်များသည် `dashboards/grafana/nexus_settlement.json` တွင် နေထိုင်ပါသည်။
   နှင့် `dashboards/alerts/nexus_audit_rules.yml` တွင် သတိပေးချက်များ။
4. **အထောက်အထား** — မော်ကွန်းပုံစံပြင်ဆင်မှုများ၊ router မှတ်တမ်းများ၊ တယ်လီမီတာတင်ပို့မှုများနှင့်
   စာရင်းစစ်များအတွက် ပြန်လည်သင့်မြတ်ရေးအစီရင်ခံစာများ။
5. **SDK တာဝန်** — SDK တစ်ခုစီတိုင်းသည် အခြေချကူညီသူများ၊ လမ်းသွား ID များကို ဖော်ထုတ်ရမည်၊
   Router နှင့် တန်းတူညီစေရန် Norito payload ကုဒ်နံပါတ်များ။

## ဥပမာ စီးဆင်းခြင်း။

| လမ်းသွားအမျိုးအစား | ဖမ်းမိရန် အထောက်အထား | သက်သေပြချက် |
|--------------------|------------------------------------------------|
| သီးသန့် `xor_hosted_custody` | Router log + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC သည် ဒက်ဘစ်အဆုံးအဖြတ်ပေးသော XOR ကို ကြားခံဆောင်ရွက်ပေးပြီး ဆံပင်ညှပ်များသည် မူဝါဒအတွင်းတွင် ရှိနေသည်။ |
| အများသူငှာ `xor_global` | Router မှတ်တမ်း + DEX/TWAP ရည်ညွှန်းချက် + latency/conversion metrics | မျှဝေထားသော ငွေဖြစ်လွယ်မှုလမ်းကြောင်းသည် ထုတ်ဝေထားသော TWAP တွင် လွှဲပြောင်းမှုကို သုညဆံပင်ပုံစံဖြင့် စျေးနှုန်းပေးသည်။ |
| ဟိုက်ဘရစ် `xor_dual_fund` | အများသူငှာ ခွဲခြမ်းနှင့် အကာအရံများကို ပြသသည့် Router မှတ်တမ်း + တယ်လီမီတာ ကောင်တာ | အကာအရံ/အများပြည်သူ ရောထွေးလေးစားသော အုပ်ချုပ်မှုအချိုးအစားနှင့် ခြေထောက်တစ်ဖက်စီတွင် အသုံးပြုထားသော ဆံပင်ညှပ်ကို မှတ်တမ်းတင်ထားသည်။ |

##အသေးစိတ်လိုအပ်ပါသလား။

- FAQ အပြည့်အစုံ- `docs/source/nexus_settlement_faq.md`
- Settlement router spec- `docs/source/settlement_router.md`
- CBDC မူဝါဒဖွင့်စာအုပ်- `docs/source/cbdc_lane_playbook.md`
- လည်ပတ်မှုစာရင်းစာအုပ်- [Nexus စစ်ဆင်ရေး](./nexus-operations)