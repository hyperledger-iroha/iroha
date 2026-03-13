---
id: payment-settlement-plan
lang: my
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Payment & Settlement Plan
sidebar_label: Payment & settlement plan
description: Playbook for routing SNS registrar revenue, reconciling steward/treasury splits, and producing evidence bundles.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Canonical အရင်းအမြစ်- [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md)။

လမ်းပြမြေပုံလုပ်ငန်း **SN-5 — ငွေပေးချေမှုနှင့် ဖြေရှင်းခြင်းဝန်ဆောင်မှု** သည် အဆုံးအဖြတ်တစ်ခုအား မိတ်ဆက်ပေးသည်။
Sora အမည်ဝန်ဆောင်မှုအတွက် ငွေပေးချေမှုအလွှာ။ မှတ်ပုံတင်ခြင်း၊ သက်တမ်းတိုးခြင်း သို့မဟုတ် ပြန်အမ်းငွေတိုင်း
ဖွဲ့စည်းတည်ဆောက်ထားသော Norito ကို ထုတ်လွှတ်ရမည်ဖြစ်ပြီး၊ ထို့ကြောင့် ဘဏ္ဍာတိုက်၊ ဘဏ္ဍာစိုးနှင့် အုပ်ချုပ်မှုတို့ ဆောင်ရွက်နိုင်သည်
စာရင်းဇယားများမပါဘဲ ငွေကြေးစီးဆင်းမှုကို ပြန်လည်ပြသပါ။ ဤစာမျက်နှာသည် Spec ကို ခွဲထုတ်သည်။
portal ပရိသတ်များအတွက်။

## အခွန်စံပြ

- အခြေခံအခကြေးငွေ (`gross_fee`) သည် မှတ်ပုံတင်အရာရှိစျေးနှုန်းမက်ထရစ်မှ ဆင်းသက်လာသည်။  
- Treasury မှ `gross_fee × 0.70` ကို လက်ခံရရှိသည်၊ ဘဏ္ဍာစိုးများသည် အကြွင်းအနှုတ်ကို လက်ခံရရှိသည်
  လွှဲပြောင်းပေးသည့် ဘောနပ်စ်များ (10%)။  
- အငြင်းပွားမှုများအတွင်း ဘဏ္ဍာစိုးအား ပေးချေမှုများကို ခေတ္တရပ်ရန် အုပ်ချုပ်မှုအား စိတ်ကြိုက်ရွေးချယ်ခွင့် ပိတ်ပင်ထားသည်။  
- အခြေချမှုအစုအဝေးများသည် `ledger_projection` ဘလောက်တစ်ခုကို ကွန်ကရစ်ဖြင့် ဖော်ထုတ်သည်။
  `Transfer` ISI များသည် အလိုအလျောက်စနစ်ဖြင့် XOR လှုပ်ရှားမှုများကို Torii သို့ တိုက်ရိုက်တင်နိုင်သည်။

## ဝန်ဆောင်မှုများနှင့် အလိုအလျောက်စနစ်

| အစိတ်အပိုင်း | ရည်ရွယ်ချက် | အထောက်အထား |
|-----------|---------|----------|
| `sns_settlementd` | မူဝါဒ၊ ဆိုင်းဘုတ်အစုအဝေးများ၊ မျက်နှာပြင်များ `/v2/sns/settlements` ကို အသုံးပြုသည်။ | JSON အတွဲ + ဟက်ရှ်။ |
| အခြေချတန်းစီ & စာရေးဆရာ | `iroha_cli app sns settlement ledger` မှ မောင်းနှင်သော Ideempotent တန်းစီ + လယ်ဂျာ တင်သွင်းသူ။ | Bundle hash ↔ tx hash manifest။ |
| အမျိုးသားပြန်လည်သင့်မြတ်ရေးအလုပ် | `docs/source/sns/reports/` အောက်တွင် နေ့စဉ်ကွာခြားချက် + လစဉ်ထုတ်ပြန်ချက်။ | Markdown + JSON အညွှန်း။ |
| ပြန်အမ်းငွေစားပွဲ | `/settlements/{id}/refund` မှတစ်ဆင့် အုပ်ချုပ်မှု-အတည်ပြုထားသော ပြန်အမ်းငွေ။ | `RefundRecordV1` + လက်မှတ်။ |

CI ကူညီပေးသူများသည် ဤစီးဆင်းမှုများကို ထင်ဟပ်စေသည်-

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## ကြည့်ရှုနိုင်မှုနှင့် အစီရင်ခံခြင်း။

- ဘဏ္ဍာတိုက်နှင့်ဆိုင်သော ဒိုင်ခွက်များ- `dashboards/grafana/sns_payment_settlement.json`
  ဘဏ္ဍာစိုးစုစုပေါင်းများ၊ လွှဲပြောင်းပေးချေမှုများ၊ တန်းစီခြင်းအတိမ်အနက်နှင့် ပြန်အမ်းငွေ ကြာချိန်။
- သတိပေးချက်များ- `dashboards/alerts/sns_payment_settlement_rules.yml` မော်နီတာများကို ဆိုင်းငံ့ထားသည်။
  အသက်အရွယ်၊ ပြန်လည်သင့်မြတ်ရေး ပျက်ကွက်မှုများ၊ လယ်ဂျာများ ပျံ့လွင့်နေသည်။
- ဖော်ပြချက်- နေ့စဉ်အချေအတင်များ (`settlement_YYYYMMDD.{json,md}`) လစဉ်အဖြစ်သို့ ရောက်ရှိလာသည်
  အစီရင်ခံစာများ (`settlement_YYYYMM.md`) သည် Git နှင့် the သို့ နှစ်ခုလုံးကို အပ်လုဒ်လုပ်ထားသည်။
  အုပ်ချုပ်မှုပစ္စည်းစတိုးဆိုင် (`s3://sora-governance/sns/settlements/<period>/`)။
- အုပ်ချုပ်ရေး ပက်ကတ်များ အစုအဝေး ဒက်ရှ်ဘုတ်များ၊ CLI မှတ်တမ်းများနှင့် ကောင်စီမတိုင်မီ အတည်ပြုချက်များ
  ဆိုင်းဘုတ်ပိတ်။

## စတင်စစ်ဆေးရန်စာရင်း

1. ရှေ့ပြေးပုံစံ ကိုးကားချက် + လယ်ဂျာအကူအညီပေးသူများနှင့် ဇာတ်ညွှန်းအတွဲတစ်ခုကို ဖမ်းယူပါ။
2. `sns_settlementd` ကို တန်းစီ + စာရေးဆရာ၊ ဝါယာကြိုးဒိုင်ခွက်များ၊ နှင့် လေ့ကျင့်ခန်းများဖြင့် စတင်ပါ။
   သတိပေးချက်စစ်ဆေးမှုများ (`promtool test rules ...`)။
3. ပြန်အမ်းငွေ ကူညီပေးသူ နှင့် လစဉ်ထုတ်ပြန်ချက် နမူနာပုံစံကို ပေးပို့ပါ။ mirror artefacts ထဲသို့ဝင်သည်။
   `docs/portal/docs/sns/reports/`။
4. လုပ်ဖော်ကိုင်ဖက်တစ်ဦးအား အစမ်းလေ့ကျင့်မှု (တစ်လအပြည့် အခြေချနေထိုင်ခြင်း) ကို လုပ်ဆောင်ပြီး ၎င်းကို ဖမ်းယူပါ။
   SN-5 ကို ပြီးပြည့်စုံကြောင်း အမှတ်အသားပြုထားသော အုပ်ချုပ်မှုမဲ။

အတိအကျ schema အဓိပ္ပါယ်ဖွင့်ဆိုချက်များအတွက် အရင်းအမြစ်စာရွက်စာတမ်းကို ပြန်ကိုးကားပါ။
မေးခွန်းများနှင့် အနာဂတ်ပြင်ဆင်မှုများ။