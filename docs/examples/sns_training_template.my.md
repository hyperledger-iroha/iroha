---
lang: my
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-12-29T18:16:35.079313+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS လေ့ကျင့်ရေးဆလိုက်ပုံစံ

ဤ Markdown ကောက်ကြောင်းသည် စည်းရုံးရေးမှူးများအတွက် လိုက်လျောညီထွေရှိသင့်သော ဆလိုက်များကို ထင်ဟပ်စေသည်။
၎င်းတို့၏ဘာသာစကားအုပ်စုများ။ ဤအပိုင်းများကို Keynote/PowerPoint/Google သို့ ကူးယူပါ။
ကျည်ဆန်အမှတ်များ၊ ဖန်သားပြင်ဓာတ်ပုံများနှင့် ပုံများကို လိုအပ်သလို ဆလိုက်ဆွဲပြီး နေရာချပါ။

## ခေါင်းစဉ်လျှော
- အစီအစဉ်- "Sora အမည် ဝန်ဆောင်မှု စတင်ခြင်း"
- စာတန်းထိုး- နောက်ဆက် + စက်ဝန်းကို သတ်မှတ်ပါ (ဥပမာ၊ `.sora — 2026‑03`)
- တင်ဆက်သူများ + ဆက်နွယ်မှု

## KPI ဦးတည်ချက်
- ဖန်သားပြင်ဓာတ်ပုံ သို့မဟုတ် `docs/portal/docs/sns/kpi-dashboard.md` ကို ထည့်သွင်းပါ။
- နောက်ဆက်တွဲစစ်ထုတ်မှုများ၊ ARPU ဇယား၊ အေးခဲနေသောခြေရာခံကိရိယာအကြောင်းရှင်းပြသည့်ကျည်ဆန်စာရင်း
- PDF/CSV တင်ပို့ခြင်းအတွက် ခေါ်ဆိုမှုများ

## ဘဝသံသရာကို ဖော်ပြပါ။
- ပုံကြမ်း- မှတ်ပုံတင်အရာရှိ → Torii → အုပ်ချုပ်မှု → DNS/ဂိတ်ဝေး
- `docs/source/sns/registry_schema.md` ကို ရည်ညွှန်းသည့် အဆင့်များ
- မှတ်ချက်များနှင့်အတူ နမူနာ manifest ကောက်နုတ်ချက်

## အငြင်းပွားမှုနှင့် လေ့ကျင့်ခန်းများ အေးခဲခြင်း။
- အုပ်ထိန်းသူ၏ဝင်ရောက်စွက်ဖက်မှုအတွက် Flow Diagram
- `docs/source/sns/governance_playbook.md` ကိုရည်ညွှန်းခြင်းစစ်ဆေးရန်စာရင်း
- ဥပမာ အေးခဲထားသော လက်မှတ်အချိန်စာရင်း

## နောက်ဆက်တွဲ ဖမ်းယူ
- `cargo xtask sns-annex ... --portal-entry ...` ကိုပြသသည့် command အတိုအထွာ
- Grafana JSON ကို `artifacts/sns/regulatory/<suffix>/<cycle>/` အောက်တွင် သိမ်းဆည်းရန် သတိပေးချက်
- `docs/source/sns/reports/.<suffix>/<cycle>.md` သို့ လင့်ခ်ချိတ်ပါ။

## နောက်တစ်ဆင့်
- သင်တန်းအကြံပြုချက်လင့်ခ် (`docs/examples/sns_training_eval_template.md` ကိုကြည့်ပါ)
- Slack/Matrix ချန်နယ်လက်ကိုင်များ
- လာမည့်မှတ်တိုင်ရက်စွဲများ