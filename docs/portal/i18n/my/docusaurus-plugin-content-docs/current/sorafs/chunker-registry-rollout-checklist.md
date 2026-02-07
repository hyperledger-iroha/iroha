---
id: chunker-registry-rollout-checklist
lang: my
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
:::

# SoraFS Registry ထုတ်ယူမှု စစ်ဆေးစာရင်း

ဤစစ်ဆေးမှုစာရင်းသည် chunker ပရိုဖိုင်အသစ်တစ်ခုမြှင့်တင်ရန် လိုအပ်သောအဆင့်များ သို့မဟုတ်
စီမံအုပ်ချုပ်မှုအပြီးတွင် ပြန်လည်သုံးသပ်ခြင်းမှ ထုတ်လုပ်ရေးအထိ ဝန်ဆောင်မှုပေးသူ၏ ဝင်ခွင့်အတွဲ
ပဋိညာဉ်စာချုပ်ကို အတည်ပြုပြီးဖြစ်သည်။

> ** နယ်ပယ်-** ပြုပြင်မွမ်းမံထားသော ထုတ်ဝေမှုအားလုံးနှင့် သက်ဆိုင်သည်။
> `sorafs_manifest::chunker_registry`၊ ဝန်ဆောင်မှုပေးသူ ဝင်ခွင့်စာအိတ်များ သို့မဟုတ်
> canonical fixture အတွဲများ (`fixtures/sorafs_chunker/*`)။

## 1. အကြိုပျံသန်းမှု အတည်ပြုခြင်း။

1. ပြိုင်ပွဲများကို ပြန်လည်ထုတ်ပေးပြီး အဆုံးအဖြတ်ပေးမှုကို အတည်ပြုပါ-
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. အဆုံးအဖြတ်ပေးသော ကိန်းဂဏာန်းများကို အတည်ပြုပါ။
   `docs/source/sorafs/reports/sf1_determinism.md` (သို့မဟုတ် သက်ဆိုင်ရာ ပရိုဖိုင်
   အစီရင်ခံစာ) ပြန်လည်ထုတ်လုပ်ထားသော ရှေးဟောင်းပစ္စည်းများနှင့် ကိုက်ညီသည်။
3. `sorafs_manifest::chunker_registry` ဖြင့် compiles သေချာပါစေ။
   လည်ပတ်ခြင်းဖြင့် `ensure_charter_compliance()`
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. အဆိုပြုချက်စာတမ်းကို အပ်ဒိတ်လုပ်ပါ-
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` အရ ကောင်စီဝင်မိနစ်
   - Determinism အစီရင်ခံစာ

## 2. အုပ်ချုပ်မှု ဆိုင်းဘုတ်

1. Tooling Working Group အစီရင်ခံစာနှင့် အဆိုပြုချက်ကို Sora အား တင်ပြပါ။
   ပါလီမန် အခြေခံအဆောက်အအုံဆိုင်ရာ ကော်မတီ။
2. အတည်ပြုချက်အသေးစိတ်ကို မှတ်တမ်းတင်ပါ။
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`။
၃။ လွှတ်တော်မှ လက်မှတ်ရေးထိုးထားသော စာအိတ်ကို ပြိုင်ပွဲများနှင့်အတူ ထုတ်ဝေပါ-
   `fixtures/sorafs_chunker/manifest_signatures.json`။
4. စာအိတ်ကို အုပ်ချုပ်မှုရယူရန် အကူအညီပေးသူမှတစ်ဆင့် ဝင်ရောက်ကြည့်ရှုနိုင်ကြောင်း အတည်ပြုပါ-
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. အဆင့်မြှင့်တင်မှု

[ဇာတ်ညွှန်းပြဇာတ်ပြစာအုပ်](./staging-manifest-playbook) ကို ကိုးကားပါ
ဤအဆင့်များ၏အသေးစိတ်ဖော်ပြချက်။

1. Torii ကို `torii.sorafs` ရှာဖွေတွေ့ရှိမှုကို ဖွင့်ထားပြီး ဝင်ခွင့်ကို အသုံးပြုပါ
   ပြဋ္ဌာန်းချက် (`enforce_admission = true`) ကို ဖွင့်ထားသည်။
2. အတည်ပြုပြီးသော ဝန်ဆောင်မှုပေးသူ၏ ဝင်ခွင့်စာအိတ်များကို အဆင့်သတ်မှတ်ထားသော မှတ်ပုံတင်ခြင်းသို့ တွန်းပို့ပါ။
   `torii.sorafs.discovery.admission.envelopes_dir` မှကိုးကားထားသောလမ်းညွှန်။
3. ရှာဖွေတွေ့ရှိမှု API မှတစ်ဆင့် ပံ့ပိုးပေးသော ကြော်ငြာများ ပျံ့နှံ့နေကြောင်း အတည်ပြုပါ-
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. အုပ်ချုပ်မှုဆိုင်ရာ ခေါင်းစီးများဖြင့် ဖော်ပြခြင်း/အစီအစဥ် အဆုံးအချက်များကို လေ့ကျင့်ပါ-
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. တယ်လီမီတာ ဒက်ရှ်ဘုတ်များ (`torii_sorafs_*`) ကို အတည်ပြုပြီး သတိပေးချက် စည်းမျဉ်းများကို အစီရင်ခံပါ။
   အမှားအယွင်းမရှိဘဲ ပရိုဖိုင်အသစ်။

## 4. ထုတ်လုပ်မှု စတင်ခြင်း

1. ထုတ်လုပ်မှု Torii node များနှင့် ဆန့်ကျင်ဘက် အဆင့်များကို ပြန်လုပ်ပါ။
2. စဖွင့်ခြင်းဝင်းဒိုး (ရက်စွဲ/အချိန်၊ ကျေးဇူးတော်ကာလ၊ ပြန်လှည့်ခြင်းအစီအစဉ်) ကို ကြေညာပါ။
   အော်ပရေတာနှင့် SDK ချန်နယ်များ။
3. ပါဝင်သော ထုတ်ပြန်ချက် PR ကို ပေါင်းစည်းပါ-
   - မွမ်းမံပြင်ဆင်ထားသော ပစ္စည်းများနှင့် စာအိတ်
   - စာရွက်စာတမ်းပြောင်းလဲမှုများ (ပဋိဉာဉ်အကိုးအကားများ၊ ဆုံးဖြတ်ခြင်းအစီရင်ခံစာ)
   - လမ်းပြမြေပုံ/အခြေအနေ ပြန်လည်ဆန်းသစ်ခြင်း။
4. ထုတ်ဝေမှုကို tag လုပ်ပြီး သက်သေအထောက်အထားအတွက် လက်မှတ်ရေးထိုးထားသော ရှေးဟောင်းပစ္စည်းများကို သိမ်းဆည်းပါ။

## 5. Post-Rollout စာရင်းစစ်

1. နောက်ဆုံး မက်ထရစ်များကို ဖမ်းယူပါ (ရှာဖွေတွေ့ရှိမှု အရေအတွက်၊ အောင်မြင်မှုနှုန်း၊ အမှားအယွင်း
   ဟစ်စတိုဂရမ်များ) ထွက်ရှိပြီးနောက် 24 နာရီ။
2. `status.md` ကို တိုတိုအကျဉ်းချုပ်နှင့် အဆုံးအဖြတ်ခံယူမှုအစီရင်ခံစာသို့ လင့်ခ်ဖြင့် အပ်ဒိတ်လုပ်ပါ။
3. မည်သည့်နောက်ဆက်တွဲလုပ်ဆောင်စရာများကိုမဆို (ဥပမာ၊ နောက်ထပ်ပရိုဖိုင်ရေးသားခြင်းလမ်းညွှန်ချက်) တွင် ထည့်သွင်းပါ။
   `roadmap.md`။