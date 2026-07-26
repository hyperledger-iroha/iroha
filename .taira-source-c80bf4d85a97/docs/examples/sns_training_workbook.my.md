---
lang: my
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS လေ့ကျင့်ရေးစာအုပ်ပုံစံ

လေ့ကျင့်ရေးအုပ်စုတစ်ခုစီအတွက် ဤအလုပ်စာအုပ်ကို ပုံသေလက်ကမ်းစာစောင်အဖြစ် အသုံးပြုပါ။ အစားထိုးပါ။
တက်ရောက်သူများကို မဖြန့်ဝေမီ နေရာကိုင်ဆောင်သူ (`<...>`)။

## အပိုင်းအသေးစိတ်
- နောက်ဆက်တွဲ- `<.sora | .nexus | .dao>`
- သံသရာ- `<YYYY-MM>`
- ဘာသာစကား- `<ar/es/fr/ja/pt/ru/ur>`
- ကူညီဆောင်ရွက်ပေးသူ- `<name>`

## Lab 1 — KPI တင်ပို့ခြင်း။
1. ပေါ်တယ် KPI ဒက်ရှ်ဘုတ် (`docs/portal/docs/sns/kpi-dashboard.md`) ကိုဖွင့်ပါ။
2. နောက်ဆက်တွဲ `<suffix>` နှင့် အချိန်အပိုင်းအခြား `<window>` ဖြင့် စစ်ထုတ်ပါ။
3. PDF + CSV ဓာတ်ပုံများကို ထုတ်ယူပါ။
4. ဤနေရာတွင် တင်ပို့ထားသော JSON/PDF ၏ SHA-256 ကို မှတ်တမ်းတင်ပါ- `______________________`။

## ဓာတ်ခွဲခန်း 2 — Manifest drill
1. `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json` မှ နမူနာ မန်နီးဖက်စ်ကို ရယူပါ။
2. `cargo run --bin sns_manifest_check -- --input <file>` ဖြင့် အတည်ပြုပါ။
3. `scripts/sns_zonefile_skeleton.py` ဖြင့် ဖြေရှင်းသူအရိုးစုကို ဖန်တီးပါ။
4. ကွဲပြားမှုအနှစ်ချုပ်ကို ကူးထည့်ပါ-
   ```
   <git diff output>
   ```

## ဓာတ်ခွဲခန်း ၃ — အငြင်းပွားမှု သရုပ်ဖော်ခြင်း။
1. အေးခဲခြင်းကို စတင်ရန် အုပ်ထိန်းသူ CLI ကို အသုံးပြုပါ (case ID `<case-id>`)။
2. အငြင်းပွားမှု hash- `______________________` ကို မှတ်တမ်းတင်ပါ။
3. အထောက်အထားမှတ်တမ်းကို `artifacts/sns/training/<suffix>/<cycle>/logs/` သို့ အပ်လုဒ်လုပ်ပါ။

## Lab 4 — နောက်ဆက်တွဲ အလိုအလျောက်စနစ်
1. Grafana ဒိုင်ခွက် JSON ကို ထုတ်ယူပြီး `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json` သို့ ကူးယူပါ။
2. Run-
   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. နောက်ဆက်တွဲလမ်းကြောင်း + SHA-256 အထွက်- `________________________________` ကို ကူးထည့်ပါ။

## မှတ်ချက်
- ဘာလဲ မရှင်းဘူးလား။
- ဘယ်ဓာတ်ခွဲခန်းတွေက အချိန်နဲ့အမျှ လည်ပတ်နေလဲ။
- Tooling bug များကို တွေ့ရှိပါသလား။

ပြီးစီးသွားသော အလုပ်စာအုပ်များကို သင်တန်းနည်းပြထံ ပြန်လည်ပေးပို့ပါ။ သူတို့လက်အောက်မှာ ရှိတယ်။
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`။