---
lang: my
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraNet Relay Incentive ပါလီမန် Packet

ဤအစုအဝေးသည် အတည်ပြုရန် Sora ပါလီမန်မှ လိုအပ်သည့်အရာများကို ဖမ်းယူထားသည်။
အလိုအလျောက် ထပ်ဆင့်ပြန်ပေးငွေများ (SNNet-7):

- `reward_config.json` - Norito-serialisable reward engine configuration အဆင်သင့်ဖြစ်ပါပြီ
  `iroha app sorafs incentives service init` ဖြင့် စားသုံးရန်။ ဟိ
  `budget_approval_id` သည် အုပ်ချုပ်မှုမိနစ်များတွင် ဖော်ပြထားသော hash နှင့် ကိုက်ညီပါသည်။
- `shadow_daemon.json` - ပြန်လည်ပြသခြင်းဖြင့် အသုံးပြုခဲ့သော အကျိုးခံစားခွင့်နှင့် ငွေချေးစာချုပ်များ
  ကြိုး (`shadow-run`) နှင့် ထုတ်လုပ်မှု daemon။
- `economic_analysis.md` - 2025-10 အတွက် တရားမျှတမှု အကျဉ်းချုပ် -> 2025-11
  အရိပ်တူခြင်း
- `rollback_plan.md` - အလိုအလျောက်ပေးချေမှုများကို ပိတ်ရန်အတွက် လုပ်ငန်းလည်ပတ်မှုဆိုင်ရာ ကစားစာအုပ်။
- ပစ္စည်းများကို ပံ့ပိုးပေးခြင်း- `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`၊
  `dashboards/grafana/soranet_incentives.json`၊
  `dashboards/alerts/soranet_incentives_rules.yml`။

## သမာဓိစစ်ဆေးခြင်း။

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

ပါလီမန် မိနစ်များတွင် မှတ်တမ်းတင်ထားသော တန်ဖိုးများနှင့် အချေအတင်များကို နှိုင်းယှဉ်ပါ။ စိစစ်ပါ။
တွင်ဖော်ပြထားသည့်အတိုင်း အရိပ်ပြေးလက်မှတ်
`docs/source/soranet/reports/incentive_shadow_run.md`။

## Packet ကို အပ်ဒိတ်လုပ်ခြင်း။

1. `reward_config.json` အား ဆုလာဘ်အလေးချိန်များ၊ အခြေခံပေးချေမှု သို့မဟုတ် အချိန်တိုင်းတွင် ပြန်လည်စတင်ပါ။
   အတည်ပြုချက် hash အပြောင်းအလဲ။
2. ရက် 60 ကြာ အရိပ်တူခြင်းအား ပြန်ဖွင့်ပါ၊ `economic_analysis.md` ကို အပ်ဒိတ်လုပ်ပါ။
   အသစ်တွေ့ရှိချက်များနှင့် JSON + ခွဲထုတ်ထားသော လက်မှတ်အတွဲကို ကတိပြုပါ။
3. မွမ်းမံထားသောအတွဲကို Observatory dashboard နှင့်အတူ လွှတ်တော်သို့တင်ပြပါ။
   သက်တမ်းတိုးခွင့်ပြုချက်ရယူသည့်အခါ တင်ပို့ခြင်း။