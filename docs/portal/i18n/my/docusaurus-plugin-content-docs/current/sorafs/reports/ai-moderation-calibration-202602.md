---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: AI Moderation Calibration Report (2026-02)
summary: Baseline calibration dataset, thresholds, and scoreboard for the first MINFO-1 governance release.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# AI Moderation Calibration အစီရင်ခံစာ - ဖေဖော်ဝါရီလ 2026

ဤအစီရင်ခံစာသည် **MINFO-1** အတွက် စတင်ချိန်ညှိခြင်းဆိုင်ရာ အနုပညာပစ္စည်းများကို ထုပ်ပိုးထားသည်။ ဟိ
ဒေတာအတွဲ၊ မန်နီးဖက်စ်နှင့် အမှတ်ပေးဇယားတို့ကို သုံးသပ်ပြီး 2026-02-05 တွင် ထုတ်လုပ်ခဲ့သည်။
2026-02-10 ရက်နေ့ တွင် ဝန်ကြီးဌာန ကောင်စီ နှင့် အုပ်ချုပ်မှု DAG တွင် ကျောက်ချရပ်နား၊
`912044`။

## Dataset Manifest

- **ဒေတာအတွဲရည်ညွှန်း-** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **ထည့်သွင်းမှုများ-** မန်နီးဖက်စ် 480၊ အပိုင်း 12,800၊ မက်တာဒေတာ 920၊ အသံ 160
- **အညွှန်းရောနှောမှု-** ဘေးကင်းမှု 68%, သံသယ 19%, တိုးမြင့် 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **ဖြန့်ဖြူးခြင်း-** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

သရုပ်ပြမှု အပြည့်အစုံသည် `docs/examples/ai_moderation_calibration_manifest_202602.json` တွင် တည်ရှိသည်။
နှင့် ထုတ်ပြန်ရာတွင် ဖမ်းယူထားသော အုပ်ချုပ်မှု လက်မှတ် နှင့် အပြေးသမား ဟက်ရှ် ပါရှိသည်။
အချိန်။

## အမှတ်စာရင်း အကျဉ်းချုပ်

ချိန်ညှိမှုများသည် opset 17 နှင့် အဆုံးအဖြတ်ရှိသော မျိုးစေ့ပိုက်လိုင်းဖြင့် လည်ပတ်ခဲ့သည်။ ဟိ
ပြီးပြည့်စုံသော အမှတ်စာရင်း JSON (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
hashes နှင့် telemetry digest များကို မှတ်တမ်းတင်သည်။ အောက်ပါဇယားသည် အထင်ရှားဆုံးဖြစ်သည်။
အရေးကြီးသော တိုင်းတာမှုများ။

| မော်ဒယ် (မိသားစု) | Brier | ECE | AUROC | Precision@quarantine | Recall@ Escalate |
| -------------- | -----| ---| -----| --------------------| --------------- |
| ViT-H/14 ဘေးကင်းရေး (အမြင်အာရုံ) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B လုံခြုံမှု (ဘက်စုံ) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| Perceptual ensemble (အသိဥာဏ်) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

ပေါင်းစပ်မက်ထရစ်များ- `Brier = 0.126`၊ `ECE = 0.034`၊ `AUROC = 0.982`။ စီရင်ချက်ချသည်။
စံကိုက်ချိန်ညှိဝင်းဒိုးတစ်လျှောက် ဖြန့်ဖြူးမှုသည် လွန်သွားသည် 91.2%, quarantine 6.8%၊
မန်နီးဖက်စ်တွင် မှတ်တမ်းတင်ထားသော မူဝါဒမျှော်မှန်းချက်များနှင့် ကိုက်ညီသော 2.0% ကို မြှင့်တင်ပါ။
အနှစ်ချုပ်။ မှားယွင်းသော အပြုသဘောဆောင်သော ကျောပိုးအိတ်သည် သုညတွင် ရှိနေခဲ့ပြီး ဒရုန်းရမှတ် (၇.၁%)၊
20% သတိပေးချက်အဆင့်အောက်သို့ ကျဆင်းသွားသည်။

## အဆင့်သတ်မှတ်ချက်များနှင့် အကောင့်ပိတ်ခြင်း။

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- အုပ်ချုပ်မှုလှုပ်ရှားမှု- `MINFO-2026-02-07`
- `2026-02-10T11:33:12Z` တွင် `ministry-council-seat-03` မှ လက်မှတ်ရေးထိုးခဲ့သည်

CI သည် `artifacts/ministry/ai_moderation/2026-02/` တွင် လက်မှတ်ထိုးထားသောအတွဲကို သိမ်းဆည်းထားသည်။
moderation runner binaries နှင့်တွဲပြီး။ ထင်ရှားသော အချေအတင်နှင့် အမှတ်စာရင်း
အထက်ဖော်ပြပါ hashe များကို စစ်ဆေးခြင်းနှင့် အယူခံဝင်စဉ်အတွင်း ကိုးကားရပါမည်။

## ဒက်ရှ်ဘုတ်များနှင့် သတိပေးချက်များ

Moderation SRE များသည် Grafana ဒိုင်ခွက်ကို တင်သွင်းသင့်သည်။
`dashboards/grafana/ministry_moderation_overview.json` နှင့် ကိုက်ညီပါသည်။
Prometheus သတိပေးချက်စည်းမျဉ်းများ `dashboards/alerts/ministry_moderation_rules.yml`
(စမ်းသပ်မှု အကျုံးဝင်မှုသည် `dashboards/alerts/tests/ministry_moderation_rules.test.yml` အောက်တွင် ရှိနေသည်)။
ဤရှေးဟောင်းပစ္စည်းများသည် စားသုံးမိသော ဆိုင်များ၊ လွင့်ပျံနေသော ငြောင့်ငယ်များနှင့် quarantine အတွက် သတိပေးချက်များ ထုတ်လွှတ်သည်
တန်းစီပွားခြင်းအတွက် တောင်းဆိုထားသော စောင့်ကြည့်မှုလိုအပ်ချက်များကို ကျေနပ်စေပါသည်။
[AI Moderation Runner Specification](../../ministry/ai-moderation-runner.md)။