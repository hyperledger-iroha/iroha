---
lang: my
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-12-29T18:16:35.086260+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraGlobal Gateway Billing Reconciliation

- **Window:** `<from>/<to>`
- **အိမ်ငှား-** `<tenant-id>`
- **Catalog Version:** `<catalog-version>`
- **အသုံးပြုမှု လျှပ်တစ်ပြက်-** `<path or hash>`
- **Guardrails:** soft cap `<soft-cap-xor> XOR`၊ hard cap `<hard-cap-xor> XOR`၊ သတိပေးချက်အဆင့် `<alert-threshold>%`
- ** ငွေပေးသူ -> ငွေတိုက်-** `<payer>` -> `<treasury>` `<asset-definition>`
- **စုစုပေါင်းပေးရမည့်အချိန်-** `<total-xor> XOR` (`<total-micros>` micro-XOR)

## Line Item စစ်ဆေးခြင်း။
- [ ] အသုံးပြုမှုထည့်သွင်းမှုများသည် ကတ်တလောက်မီတာအိုင်ဒီများနှင့် တရားဝင်ငွေပေးချေသည့်ဒေသများသာ အကျုံးဝင်ပါသည်။
- [ ] အရေအတွက်ယူနစ်များသည် ကတ်တလောက်အဓိပ္ပါယ်ဖွင့်ဆိုချက်များနှင့် ကိုက်ညီသည် (တောင်းဆိုမှုများ၊ GiB၊ ms စသည်ဖြင့်)
- [ ] ကတ်တလောက်အလိုက် တိုင်းဒေသကြီးအမြှောက်များနှင့် လျှော့စျေးအဆင့်များကို ထည့်သွင်းထားသည်။
- [ ] CSV/Parquet ထုတ်ယူမှုများသည် JSON ပြေစာလိုင်းပစ္စည်းများနှင့် ကိုက်ညီပါသည်။

## Guardrail အကဲဖြတ်ခြင်း။
- [ ] Soft cap သတိပေးချက်အဆင့်ကို ရောက်ပြီလား။ `<yes/no>` (ဟုတ်ပါက သတိပေးချက် အထောက်အထား ပူးတွဲပါ)
- [ ] Hard cap ကျော်လွန်သွားပါသလား။ `<yes/no>` (ဟုတ်ပါက၊ ထပ်လောင်းအတည်ပြုချက်ကို ပူးတွဲပါ)
- [ ] အနည်းဆုံး ပြေစာလွှာ ကျေနပ်သည်။

## စာရင်းဇယားဆွဲခြင်း။
- [ ] လွှဲပြောင်းမှုအသုတ်စုစုပေါင်းသည် ပြေစာတွင် `total_micros` နှင့် ညီမျှသည်
- [ ] ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက် ငွေကြေးနှင့် ကိုက်ညီပါသည်။
- [ ] ငွေပေးသူနှင့် ငွေတိုက်စာရင်းများသည် အိမ်ငှားနှင့် မှတ်တမ်း၏ အော်ပရေတာနှင့် ကိုက်ညီသည်။
- [ ] Norito/JSON မှတ်တမ်းများကို ပြန်လည်ပြသရန်အတွက် ပူးတွဲပါ

## အငြင်းပွားမှု/ညှိနှိုင်းမှု မှတ်စုများ
- ကွဲလွဲမှုကို သတိပြုမိသည်- `<variance detail>`
- အဆိုပြုထားသော ပြုပြင်ပြောင်းလဲမှု- `<delta and rationale>`
သက်သေအထောက်အထား- `<logs/dashboards/alerts>`

## ခွင့်ပြုချက်
- ငွေတောင်းခံမှု ဆန်းစစ်သူ- `<name + signature>`
- Treasury reviewer- `<name + signature>`
- အုပ်ချုပ်မှုပက်ကတ် hash- `<hash/reference>`