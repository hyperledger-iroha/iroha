---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e832438aa843ed1ce6d018055a67bd1688ccb4cead9f73b87033469256015d
source_last_modified: "2026-01-22T16:26:46.526983+00:00"
translation_last_reviewed: 2026-02-07
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
translator: machine-google-reviewed
---

Reserve+Rent ပေါ်လစီ (လမ်းပြမြေပုံ အကြောင်းအရာ **SFM‑6**) သည် ယခုအခါ `sorafs reserve` ကို တင်ပို့လိုက်ပြီဖြစ်သည်။
CLI အကူအညီပေးသူများ နှင့် `scripts/telemetry/reserve_ledger_digest.py` ဘာသာပြန်သူမို့
ဘဏ္ဍာတိုက်လည်ပတ်မှုများသည် အဆုံးအဖြတ်ပေးသောငှားရမ်းမှု/အရန်ငွေလွှဲပြောင်းမှုများကို ထုတ်လွှတ်နိုင်သည်။ ဒီစာမျက်နှာက မှန်ပါတယ်။
`docs/source/sorafs_reserve_rent_plan.md` တွင် သတ်မှတ်ထားသော အလုပ်အသွားအလာကို ရှင်းပြထားသည်။
Grafana + Alertmanager သို့ လွှဲပြောင်းမှုအသစ်ကို မည်သို့မည်ပုံ ကြိုးပေးရမည်နည်း ထို့ကြောင့် စီးပွားရေးနှင့်
အုပ်ချုပ်မှုပြန်လည်သုံးသပ်သူများသည် ငွေပေးချေမှုစက်ဝန်းတိုင်းကို စစ်ဆေးနိုင်သည်။

## အစမှအဆုံး အလုပ်အသွားအလာ

1. ** ကိုးကားချက် + လယ်ဂျာပုံဆွဲခြင်း **
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account soraカタカナ... \
    --treasury-account soraカタカナ... \
    --reserve-account soraカタカナ... \
    --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   လယ်ဂျာအကူအညီပေးသူက `ledger_projection` ဘလောက်တစ်ခုကို ပူးတွဲပါရှိသည် (ငှားရမ်းခ၊ အရန်၊
   ချို့ယွင်းချက်၊ ငွေဖြည့်မြစ်ဝကျွန်းပေါ်ဒေသ၊ underwriting booleans) နှင့် Norito `Transfer`
   ဘဏ္ဍာတိုက်နှင့် အရန်ငွေစာရင်းများကြား XOR ကို ရွှေ့ရန် ISI များ လိုအပ်သည်။

2. **အချေအတင် + Prometheus/NDJSON အထွက်များကို ဖန်တီးပါ**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   ချေဖျက်ပေးသူသည် မိုက်ခရို-XOR စုစုပေါင်းကို XOR သို့ ပုံမှန်ဖြစ်စေသည်၊ ရှိမရှိ မှတ်တမ်းတင်သည်။
   projection သည် underwrite နှင့်ကိုက်ညီပြီး **transfer feed** metrics များကို ထုတ်လွှတ်ပါသည်။
   `sorafs_reserve_ledger_transfer_xor` နှင့်
   `sorafs_reserve_ledger_instruction_total`။ လယ်ဂျာများစွာကို ဘယ်အချိန်မှာ ဖြစ်ဖို့လိုမလဲ။
   လုပ်ဆောင်ပြီးသား (ဥပမာ၊ ပံ့ပိုးပေးသူတစ်စု)၊ `--ledger`/`--label` အတွဲများကို ထပ်ခါထပ်ခါ နှင့်
   ကူညီသူသည် အဆုံးအဖြတ်တိုင်းပါ၀င်သော NDJSON/Prometheus ဖိုင်တစ်ခုအား ရေးသားသည်၊ ထို့ကြောင့်၊
   ဒိုင်ခွက်များသည် စိတ်ကြိုက်ကော်မပါဘဲ လည်ပတ်မှုတစ်ခုလုံးကို စုပ်ယူသည်။ `--out-prom`
   ဖိုင်သည် node-exporter textfile စုဆောင်းသူအား ပစ်မှတ်သည်—`.prom` ဖိုင်ထဲသို့ ပစ်ထည့်ပါ။
   တင်ပို့သူ၏ ကြည့်ရှုထားသည့် လမ်းညွှန် သို့မဟုတ် ၎င်းကို တယ်လီမီတာပုံးသို့ အပ်လုဒ်လုပ်ပါ။
   `--ndjson-out` က အတူတူ ကျွေးနေချိန်မှာ Reserve dashboard အလုပ်က စားသုံးသည်
   ဒေတာပိုက်လိုင်းများတွင် payload များ။

3. ** ပစ္စည်းများထုတ်ဝေခြင်း + အထောက်အထားများ **
   - `artifacts/sorafs_reserve/ledger/<provider>/` နှင့် လင့်ခ်အောက်ရှိ အချေအတင်များကို သိမ်းဆည်းပါ။
     သင်၏အပတ်စဉ်စီးပွားရေးအစီရင်ခံစာမှ Markdown အနှစ်ချုပ်။
   - ငှားရမ်းမှုပျက်ဆီးခြင်းသို့ JSON အချေအတင်ကို ပူးတွဲပါ (ဒါမှ စာရင်းစစ်များသည် ၎င်းကို ပြန်လည်ဖွင့်နိုင်သည်။
     သင်္ချာ) နှင့် အုပ်ချုပ်မှုဆိုင်ရာ အထောက်အထားထုပ်ပိုးမှုအတွင်း checksum ကို ထည့်သွင်းပါ။
   - digest သည် ငွေဖြည့်သွင်းခြင်း သို့မဟုတ် underwriting ချိုးဖောက်မှုတစ်ခုဖြစ်ကြောင်း အချက်ပြပါက၊ သတိပေးချက်ကို ကိုးကားပါ။
     ID များ (`SoraFSReserveLedgerTopUpRequired`၊
     `SoraFSReserveLedgerUnderwritingBreach`) နှင့် မည်သည့် ISIS လွှဲပြောင်းမှုများကို မှတ်သားထားပါ။
     လျှောက်ထားသည်။

## မက်ထရစ်များ → ဒက်ရှ်ဘုတ်များ → သတိပေးချက်များ

| အရင်းအမြစ်မက်ထရစ် | Grafana အကန့် | သတိပေးချက် / မူဝါဒချိတ် | မှတ်စုများ |
|----------------|----------------|---------------------|------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | `dashboards/grafana/sorafs_capacity_health.json` တွင် “DA Rent Distribution (XOR/hour)” အပတ်စဉ် ဘဏ္ဍာတိုက်အကြေအကြေကို ကျွေးမွေးပါ။ အရန်စီးဆင်းမှုတွင် ပေါက်ပွားမှုများသည် `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`) သို့ ပြန့်ပွားသည်။ |
| `torii_da_rent_gib_months_total` | “စွမ်းဆောင်ရည်အသုံးပြုမှု (GiB-လများ)” (တူညီသော ဒက်ရှ်ဘုတ်) | ပြေစာသိမ်းဆည်းမှုသည် XOR လွှဲပြောင်းမှုများနှင့် ကိုက်ညီကြောင်း သက်သေပြရန် လယ်ဂျာအနှစ်ချုပ်နှင့် တွဲပါ။ |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | `dashboards/grafana/sorafs_reserve_economics.json` ရှိ “လျှပ်တစ်ပြက်ရိုက်ချက် (XOR)” + အခြေအနေကတ်များ `SoraFSReserveLedgerTopUpRequired` မီးလောင်သောအခါ `requires_top_up=1`; `SoraFSReserveLedgerUnderwritingBreach` မီးလောင်သောအခါ `meets_underwriting=0`။ |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | “အမျိုးအစားအလိုက် လွှဲပြောင်းမှုများ”၊ “နောက်ဆုံးပေါ် လွှဲပြောင်းမှုခွဲခြမ်းစိတ်ဖြာခြင်း” နှင့် `dashboards/grafana/sorafs_reserve_economics.json` တွင် အကျုံးဝင်သည့်ကတ်များ | `SoraFSReserveLedgerInstructionMissing`၊ `SoraFSReserveLedgerRentTransferMissing` နှင့် `SoraFSReserveLedgerTopUpTransferMissing` သည် ငှားရမ်းခ/ငွေဖြည့်ရန် လိုအပ်သော်လည်း လွှဲပြောင်းမှုဖိဒ်မရှိသည့်အခါ သို့မဟုတ် သုညသို့သတိပေးသည်။ အကျုံးဝင်သည့်ကတ်များသည် တူညီသောကိစ္စများတွင် 0% သို့ ကျဆင်းသည်။ |

အငှားစက်ဝန်းတစ်ခုပြီးသွားသောအခါ၊ Prometheus/NDJSON လျှပ်တစ်ပြက်ရိုက်ချက်များကို ပြန်လည်စတင်ပါ၊ အတည်ပြုပါ
Grafana အကန့်များသည် `label` အသစ်ကို ကောက်ယူပြီး ဖန်သားပြင်ဓာတ်ပုံများ ပူးတွဲပါရှိကြောင်း +
ငှားရမ်းမှု အုပ်ချုပ်မှု ပက်ကတ်သို့ သတိပေးချက် မန်နေဂျာ ID များ။ ဒါက CLI projection ကို သက်သေပြပြီး၊
တယ်လီမီတာနှင့် အုပ်ချုပ်မှုဆိုင်ရာ ပစ္စည်းအားလုံးသည် **တူညီသော** လွှဲပြောင်းပေးပို့မှုဖိဒ်နှင့် အရင်းခံဖြစ်သည်။
လမ်းပြမြေပုံ၏ စီးပွားရေးဆိုင်ရာ ဒက်ရှ်ဘုတ်များကို Reserve+Rent နှင့် ချိန်ညှိထားသည်။
အလိုအလျောက်စနစ်။ လွှမ်းခြုံကတ်များသည် 100% (သို့မဟုတ် 1.0) နှင့် သတိပေးချက်အသစ်များကို ဖတ်သင့်သည်။
ငှားရမ်းခနှင့် ကြိုတင်ငွေဖြည့်လွှဲပြောင်းမှုများသည် မှတ်တမ်းတွင် ပါရှိသည်နှင့် တစ်ပြိုင်နက် ရှင်းလင်းသင့်သည်။