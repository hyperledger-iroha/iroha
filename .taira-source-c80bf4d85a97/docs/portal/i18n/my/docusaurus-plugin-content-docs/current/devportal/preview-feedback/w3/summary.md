---
id: preview-feedback-w3-summary
lang: my
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W3 beta feedback & status
sidebar_label: W3 summary
description: Live digest for the 2026 beta preview wave (finance, observability, SDK, and ecosystem cohorts).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| အမျိုးအမည် | အသေးစိတ် |
| ---| ---|
| လှိုင်း | W3 — ဘီတာအုပ်စုများ (ဘဏ္ဍာရေး + ops + SDK ပါတနာ + ဂေဟစနစ် ထောက်ခံသူ) |
| ဒိုးပတ် | 2026-02-18 → 2026-02-28 |
| Artefact tag | `preview-20260218` |
| Tracker ပြဿနာ | `DOCS-SORA-Preview-W3` |
| ပါဝင်သူများ | Finance-beta-01, observability-ops-02, partner-sdk-03, ecosystem-advocate-04 |

## ပေါ်လွင်ချက်များ

1. **အဆုံးမှအဆုံး အထောက်အထား ပိုက်လိုင်း။** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` သည် တစ်လှိုင်းအနှစ်ချုပ် (`artifacts/docs_portal_preview/preview-20260218-summary.json`)၊ အချေအတင် (`preview-20260218-digest.md`) နှင့် `docs/portal/src/data/previewFeedbackSummary.json` ကို ပြန်လည်ဆန်းသစ်ပေးသည် ဖြစ်သောကြောင့် အုပ်ချုပ်မှု တစ်ခုတည်းသော ဝေဖန်သုံးသပ်သူ ကွပ်ကဲမှုအပေါ် အားကိုးနိုင်ပါသည်။
2. **Telemetry + အုပ်ချုပ်မှု အကျုံးဝင်မှု။** ပြန်လည်သုံးသပ်သူ လေးဦးစလုံးသည် checksum-gated access ကို အသိအမှတ်ပြုပြီး တုံ့ပြန်ချက်တင်ပြပြီး အချိန်မီ ရုတ်သိမ်းလိုက်ပါသည်။ အညွှန်းသည် တုံ့ပြန်ချက်ပြဿနာများ (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) လှိုင်းအတွင်းစုဆောင်းထားသော Grafana နှင့်တွဲပြီး ကိုးကားပါသည်။
3. **Portal surfacing.** ပြန်လည်ဆန်းသစ်ထားသော portal ဇယားသည် ယခုအချိန်တွင် ပိတ်ထားသော W3 wave ကို latency နှင့် response-rate metrics များဖြင့် ပြသထားပြီး၊ အောက်ဖော်ပြပါ မှတ်တမ်းစာမျက်နှာအသစ်သည် JSON မှတ်တမ်းအကြမ်းကို မဆွဲထုတ်သော စာရင်းစစ်များအတွက် event timeline ကို ထင်ဟပ်စေသည်။

## လုပ်ဆောင်ချက်ပစ္စည်းများ

| ID | ဖော်ပြချက် | ပိုင်ရှင် | အဆင့်အတန်း |
| ---| ---| ---| ---|
| W3-A1 | အကြိုကြည့်ရှုမှု မှတ်တမ်းကို ဖမ်းယူပြီး ခြေရာခံကိရိယာသို့ ပူးတွဲပါ။ | Docs/DevRel ဦးဆောင် | ✅ 2026-02-28 | ပြီးစီးခဲ့သည်။
| W3-A2 | ပေါ်တယ် + လမ်းပြမြေပုံ/ အခြေအနေသို့ အထောက်အထားများကို ကြေးမုံသတင်းစာ ဖိတ်ခေါ်/ချေဖျက်ပါ။ | Docs/DevRel ဦးဆောင် | ✅ 2026-02-28 | ပြီးစီးခဲ့သည်။

## ထွက်ပေါက်အကျဉ်းချုပ် (2026-02-28)

- ဖိတ်ကြားချက်များကို 2026-02-18 တွင် ပေးပို့ခဲ့ပြီး မိနစ်ပိုင်းအကြာတွင် မှတ်တမ်းတင်ထားသော အသိအမှတ်ပြုမှုများ၊ နောက်ဆုံး တယ်လီမီတာစစ်ဆေးမှု အောင်မြင်ပြီးနောက် 2026-02-28 တွင် အစမ်းကြည့်ရှုခွင့်ကို ရုပ်သိမ်းခဲ့သည်။
- `artifacts/docs_portal_preview/` အောက်တွင် ရိုက်ကူးထားသော Digest + အကျဉ်းချုပ်၊ `artifacts/docs_portal_preview/feedback_log.json` မှ ကျောက်ချရပ်နားထားသော ကုန်ကြမ်းမှတ်တမ်းဖြင့် ပြန်လည်ပြသနိုင်မည်ဖြစ်သည်။
- အုပ်ချုပ်မှုခြေရာခံကိရိယာ `DOCS-SORA-Preview-20260218` ဖြင့် `docs-preview/20260218` အရ တင်သွင်းထားသော ပြဿနာနောက်ဆက်တွဲများ။ CSP/Try it notes များကို ကြည့်ရှုနိုင်မှု/ဘဏ္ဍာရေးဆိုင်ရာ ပိုင်ရှင်များထံ လမ်းကြောင်းပေးကာ စုစည်းမှုမှ ချိတ်ဆက်ထားသည်။
- ခြေရာခံအတန်းကို 🈴 ပြီးမြောက်အောင် အပ်ဒိတ်လုပ်ပြီး ကျန်ရှိသော DOCS-SORA ဘီတာ-အဆင်သင့်ဖြစ်ခြင်းလုပ်ငန်းကို ပြီးမြောက်အောင် လုပ်ဆောင်ပြီး ပေါ်တယ်တုံ့ပြန်ချက်ဇယားသည် အပိတ်လှိုင်းကို ထင်ဟပ်စေသည်။