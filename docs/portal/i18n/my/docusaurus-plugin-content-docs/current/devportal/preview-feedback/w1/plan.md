---
id: preview-feedback-w1-plan
lang: my
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W1 partner preflight plan
sidebar_label: W1 plan
description: Tasks, owners, and evidence checklist for the partner preview cohort.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

| အမျိုးအမည် | အသေးစိတ် |
| ---| ---|
| လှိုင်း | W1 — ပါတနာများနှင့် Torii ပေါင်းစည်းသူများ |
| ပစ်မှတ် window | Q2 2025 ရက်သတ္တပတ် 3 |
| Artefact tag (စီစဉ်ထားသည်) | `preview-2025-04-12` |
| Tracker ပြဿနာ | `DOCS-SORA-Preview-W1` |

## ရည်ရွယ်ချက်များ

1. ပါတနာအစမ်းကြည့်ရှုမှုစည်းမျဉ်းများအတွက် တရားဝင် + အုပ်ချုပ်မှုအတည်ပြုချက်များကို လုံခြုံစေပါသည်။
2. ဖိတ်ကြားမှုအစုအဝေးတွင်အသုံးပြုသည့် Try it proxy နှင့် telemetry လျှပ်တစ်ပြက်ရိုက်ချက်များကို အဆင့်သတ်မှတ်ပါ။
3. checksum-အတည်ပြုထားသော အစမ်းကြည့်ရှုမှု နှင့် စစ်ဆေးခြင်းရလဒ်များကို ပြန်လည်စတင်ပါ။
4. ဖိတ်ကြားချက်များမပို့မီ ပါတနာစာရင်း + တောင်းဆိုမှုပုံစံများကို အပြီးသတ်ပါ။

## အလုပ်ပျက်ခြင်း။

| ID | တာဝန် | ပိုင်ရှင် | စူးစူး | အဆင့်အတန်း | မှတ်စုများ |
| ---| ---| ---| ---| ---| ---|
| W1-P1 | အကြိုကြည့်ရှုခြင်းဆိုင်ရာ စည်းကမ်းချက်များ နောက်ဆက်တွဲအတွက် တရားဝင်ခွင့်ပြုချက်ရယူပါ။ Docs/DevRel ဦးဆောင် → တရားဝင် | 2025-04-05 | ✅ ပြီးစီး | တရားဝင်လက်မှတ် `DOCS-SORA-Preview-W1-Legal` သည် 2025-04-05 တွင် လက်မှတ် ရေးထိုးထားသည်။ PDF ကို tracker တွင်တွဲထားသည်။ |
| W1-P2 | ဖမ်းယူစမ်းပါ ပရောက်စီ စတိတ်စင်ဝင်းဒိုး (2025-04-10) နှင့် ပရောက်စီကျန်းမာရေးကို အတည်ပြုပါ | Docs/DevRel + Ops | 2025-04-06 | ✅ ပြီးစီး | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` သည် 2025-04-06 ကို ကွပ်မျက်ခဲ့သည်။ CLI စာသား + `.env.tryit-proxy.bak` ကို သိမ်းဆည်းပြီးပါပြီ။ |
| W1-P3 | အစမ်းကြည့်ရှုခြင်းအနုပညာဖန်တီးမှု (`preview-2025-04-12`)၊ run `scripts/preview_verify.sh` + `npm run probe:portal`၊ မော်ကွန်းတင်ဖော်ပြသူ/ချက်လက်မှတ်များ | Portal TL | 2025-04-08 | ✅ ပြီးစီး | `artifacts/docs_preview/W1/preview-2025-04-12/` အောက်တွင် သိမ်းဆည်းထားသော Artefact + အတည်ပြုခြင်းမှတ်တမ်းများ probe output ကို tracker တွင်တွဲထားသည်။ |
| W1-P4 | ပါတနာ သုံးစွဲမှုပုံစံများကို ပြန်လည်သုံးသပ်ပါ (`DOCS-SORA-Preview-REQ-P01…P08`)၊ အဆက်အသွယ်များကို အတည်ပြုပါ + NDAs | အုပ်ချုပ်မှုဆက်ဆံရေးရုံး | 2025-04-07 | ✅ ပြီးစီး | တောင်းဆိုချက်ရှစ်ခုစလုံးကို အတည်ပြုခဲ့သည် (နောက်ဆုံးနှစ်ချက်ကို 2025-04-11 တွင် ရှင်းလင်းခဲ့သည်); ခြေရာခံကိရိယာတွင် လင့်ခ်ချိတ်ထားသော အတည်ပြုချက်များ။ |
| W1-P5 | ပါတနာတစ်ခုစီအတွက် ဖိတ်ကြားချက်မူကြမ်း (`docs/examples/docs_preview_invite_template.md`)၊ `<preview_tag>` နှင့် `<request_ticket>` သတ်မှတ် | Docs/DevRel ဦးဆောင် | 2025-04-08 | ✅ ပြီးစီး | ဖိတ်ကြားချက်မူကြမ်း 2025-04-12 15:00UTC တွင် artefact လင့်ခ်များနှင့်အတူ ပေးပို့ခဲ့သည်။ |

## ကြိုတင်စစ်ဆေးရန်စာရင်း

> အကြံပြုချက်- အဆင့် 1-5 ကို အလိုအလျောက်လုပ်ဆောင်ရန် `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` (တည်ဆောက်ခြင်း၊ စစ်ဆေးခြင်းအတည်ပြုခြင်း၊ ပေါ်တယ်စုံစမ်းစစ်ဆေးခြင်း၊ လင့်ခ်စစ်ဆေးခြင်းနှင့် စမ်းသုံးကြည့်ပါ ပရောက်စီအပ်ဒိတ်)။ script သည် tracker ပြဿနာတွင် ပူးတွဲပါနိုင်သော JSON မှတ်တမ်းကို မှတ်တမ်းတင်ပါသည်။

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12`) `build/checksums.sha256` နှင့် `build/release.json` ကို ပြန်ထုတ်ရန်။
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`။
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`။
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` နှင့် `build/link-report.json` တို့ကို ဖော်ပြချက်၏ဘေးတွင် တင်ထားသည်။
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (သို့မဟုတ် `--tryit-target` မှတဆင့် သင့်လျော်သောပစ်မှတ်ကို ပေးဆောင်ပါ); အပ်ဒိတ်လုပ်ထားသော `.env.tryit-proxy` ကို အပ်နှံပြီး ပြန်သိမ်းရန်အတွက် `.bak` ကို သိမ်းထားပါ။
6. W1 tracker ပြဿနာကို မှတ်တမ်းလမ်းကြောင်းများဖြင့် အပ်ဒိတ်လုပ်ပါ (ဖော်ပြချက် စစ်ဆေးမှုရလဒ်၊ စုံစမ်းစစ်ဆေးမှု အထွက်၊ စမ်းသုံးကြည့်ပါ ပရောက်စီပြောင်းလဲမှု၊ Grafana လျှပ်တစ်ပြက်ရိုက်ချက်များ)။

## သက်သေစာရင်း

- [x] `DOCS-SORA-Preview-W1` တွင် ပူးတွဲပါရှိသော တရားဝင်ခွင့်ပြုချက် (PDF သို့မဟုတ် လက်မှတ်လင့်ခ်) လက်မှတ်ရေးထိုးထားသည်။
- [x] Grafana `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` အတွက် ဖန်သားပြင်ဓာတ်ပုံများ။
- [x] `preview-2025-04-12` ဖော်ပြချက် + checksum မှတ်တမ်း `artifacts/docs_preview/W1/` အောက်တွင် သိမ်းဆည်းထားသည်။
- [x] `invite_sent_at` အချိန်တံဆိပ်များဖြည့်ထားသော စာရင်းဇယားကို ဖိတ်ကြားပါ (ခြေရာခံ W1 မှတ်တမ်းကိုကြည့်ပါ)။
- [x] တုံ့ပြန်ချက် ရှေးဟောင်းပစ္စည်းများ [`preview-feedback/w1/log.md`](./log.md) တွင် ပါတနာတစ်ဦးလျှင် အတန်းတစ်တန်း (စာရင်းဇယား/တယ်လီမီတာ/ပြဿနာဒေတာဖြင့် 2025-04-26 ကို အပ်ဒိတ်လုပ်ထားသည်)။

လုပ်ဆောင်စရာများ တိုးတက်မှုအဖြစ် ဤအစီအစဥ်ကို အပ်ဒိတ်လုပ်ပါ။ လမ်းပြမြေပုံကို ထိန်းသိမ်းရန် ခြေရာခံသူက ၎င်းကို ကိုးကားသည်။
စာရင်းစစ်။

## တုံ့ပြန်ချက် အလုပ်အသွားအလာ

1. သုံးသပ်သူတိုင်းအတွက် နမူနာပုံစံကို ပွားပါ။
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)၊
   မက်တာဒေတာကိုဖြည့်ပါ၊ ပြီးပြည့်စုံသောမိတ္တူကို အောက်တွင် သိမ်းဆည်းပါ။
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`။
2. ဖိတ်ခေါ်ချက်များ၊ တယ်လီမီတာ စစ်ဆေးရေးဂိတ်များနှင့် တိုက်ရိုက်ထုတ်လွှင့်မှုမှတ်တမ်းအတွင်း ဖွင့်ထားသော ပြဿနာများကို အကျဉ်းချုပ်ဖော်ပြပါ။
   [`preview-feedback/w1/log.md`](./log.md) ထို့ကြောင့် အုပ်ချုပ်မှုပြန်လည်သုံးသပ်သူများသည် လှိုင်းတစ်ခုလုံးကို ပြန်ဖွင့်နိုင်သည်
   repository ကိုမချန်ဘဲ။
3. အသိပညာ-စစ်ဆေးခြင်း သို့မဟုတ် စစ်တမ်းထုတ်ယူမှုများ ရောက်ရှိလာသောအခါ၊ ၎င်းတို့ကို မှတ်တမ်းတွင် ဖော်ပြထားသည့် အနုပညာလမ်းကြောင်းတွင် ပူးတွဲပါရှိသည်။
   နှင့် tracker ပြဿနာကို ချိတ်ဆက်ပါ။