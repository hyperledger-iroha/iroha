---
lang: my
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 676798a4cf7c3e7737a0f80640f3f268a2f625f92afdd359ac528881d2aeb046
source_last_modified: "2025-12-29T18:16:35.060950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Documentation Automation Baseline (AND5)

လမ်းပြမြေပုံပါ အကြောင်းအရာ AND5 သည် မှတ်တမ်းပြုစုခြင်း၊ နေရာချထားခြင်းနှင့် ထုတ်ဝေခြင်း လိုအပ်သည်။
AND6 (CI & Compliance) မစတင်မီ စာရင်းစစ်နိုင်စေရန် အလိုအလျောက်စနစ်။ ဒီဖိုဒါ
AND5/AND6 ကိုးကားသော အမိန့်များ၊ ရှေးဟောင်းပစ္စည်းများ၊ နှင့် အထောက်အထား အပြင်အဆင်များကို မှတ်တမ်းတင်သည်၊
ဖမ်းယူထားသော အစီအစဥ်များကို ပြန်လှန်ကြည့်ပါ။
`docs/source/sdk/android/developer_experience_plan.md` နှင့်
`docs/source/sdk/android/parity_dashboard_plan.md`။

## ပိုက်လိုင်းများနှင့် အမိန့်များ

| တာဝန် | အမိန့်(များ) | မျှော်လင့်ထားသည့်အရာများ | မှတ်စုများ |
|------|------------------|--------------------|-------|
| Localization ဆောင်းပါးတိုစင့်ခ်လုပ်ခြင်း | `python3 scripts/sync_docs_i18n.py` (ပြေးနှုန်းတစ်ခုလျှင် `--lang <code>` ကို ကျော်ဖြတ်နိုင်သည်) | `docs/automation/android/i18n/<timestamp>-sync.log` အောက်တွင် သိမ်းဆည်းထားသော မှတ်တမ်းဖိုင် နှင့် ဘာသာပြန်ထားသော ဆောင်းပါးတိုများ | ဘာသာပြန်ချလံများနှင့် `docs/i18n/manifest.json` ကို ထပ်တူပြု၍ သိမ်းဆည်းထားသည်။ မှတ်တမ်းသည် ထိမိသော ဘာသာစကားကုဒ်များကို မှတ်တမ်းတင်ပြီး အခြေခံလိုင်းတွင် ဖမ်းယူထားသော git commit ကို မှတ်တမ်းတင်သည်။ |
| Norito ခံစစ်မှူး + parity အတည်ပြုခြင်း | `ci/check_android_fixtures.sh` (္စ `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | ထုတ်လုပ်ထားသော အနှစ်ချုပ် JSON ကို `docs/automation/android/parity/<stamp>-summary.json` | သို့ ကူးယူပါ။ `java/iroha_android/src/test/resources` ပေးချေမှုများ၊ ဟက်ရှ်များကို ထင်ရှားစွာပြသခြင်းနှင့် လက်မှတ်ထိုးထားသော တပ်ဆင်မှုအရှည်များကို အတည်ပြုသည်။ `artifacts/android/fixture_runs/` လက်အောက်ရှိ cadence အထောက်အထားနှင့်အတူ အကျဉ်းချုပ်ကို ပူးတွဲပါ ။ |
| နမူနာပြသခြင်းနှင့် ထုတ်ဝေခြင်း အထောက်အထား | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (စမ်းသပ်မှုများ + SBOM + သက်သေပြချက်) | Provenance အစုအဝေး မက်တာဒေတာနှင့် `docs/automation/android/samples/<version>/` အောက်တွင် သိမ်းဆည်းထားသည့် `docs/source/sdk/android/samples/` မှ ရရှိလာသော `sample_manifest.json` AND5 နမူနာအက်ပ်များကို ချိတ်ဆက်ပြီး အလိုအလျောက်စနစ်အား အတူတကွထုတ်လွှတ်သည်—စမ်းသပ်မှုအတွက် ထုတ်လုပ်ထားသော မန်နီးဖက်စ်၊ SBOM ဟက်ရှ်နှင့် သက်သေပြချက်မှတ်တမ်းကို ဖမ်းယူပါ။ |
| Parity ဒက်ရှ်ဘုတ် ဖိဒ် | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` နောက်တွင် `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | `metrics.prom` လျှပ်တစ်ပြက်ရိုက်ချက် သို့မဟုတ် Grafana မှ JSON ကို `docs/automation/android/parity/<stamp>-metrics.prom` သို့ ကူးယူပါ | ဒက်ရှ်ဘုတ်အစီအစဥ်ကို ဖြည့်စွက်ပေးသောကြောင့် AND5/AND7 အုပ်ချုပ်ရေးသည် မမှန်ကန်သောတင်ပြချက်ကောင်တာများနှင့် တယ်လီမီတာအသုံးပြုမှုကို အတည်ပြုနိုင်မည်ဖြစ်သည်။ |

## အထောက်အထားတွေ ဖမ်းတယ်။

1. **Timestamp အရာအားလုံး။** UTC အချိန်တံဆိပ်တုံးများကို အသုံးပြု၍ ဖိုင်များကို အမည်ပေးပါ။
   (`YYYYMMDDTHHMMSSZ`) ထို့ကြောင့် တူညီသော ဒက်ရှ်ဘုတ်များ၊ အုပ်ချုပ်မှုမိနစ်များနှင့် ထုတ်ဝေခြင်း
   docs သည် တူညီသော run ကို အဆုံးအဖြတ်ကျကျ ကိုးကားနိုင်သည်။
2. **Reference commits.** log တစ်ခုစီတွင် run ၏ git commit hash ပါဝင်သင့်သည်
   ထို့အပြင် သက်ဆိုင်ရာဖွဲ့စည်းပုံ (ဥပမာ၊ `ANDROID_PARITY_PIPELINE_METADATA`)။
   ကိုယ်ရေးကိုယ်တာပြန်လည်ဖြေရှင်းရန် လိုအပ်သောအခါတွင်၊ လုံခြုံသောအခန်းသို့ လင့်ခ်တစ်ခုထည့်သွင်းပါ။
3. ** အနည်းငယ်မျှသော အကြောင်းအရာကို သိမ်းဆည်းပါ။** ကျွန်ုပ်တို့သည် တည်ဆောက်ထားသော အနှစ်ချုပ်များကိုသာ စစ်ဆေးပါ (JSON၊
   `.prom`၊ `.log`)။ ကြီးမားသောပစ္စည်းများ (APK အစုအဝေးများ၊ ဖန်သားပြင်ဓာတ်ပုံများ) ထဲတွင် ရှိနေသင့်သည်။
   `artifacts/` သို့မဟုတ် မှတ်တမ်းတွင် မှတ်တမ်းတင်ထားသော လက်မှတ်ထိုးထားသော hash ဖြင့် အရာဝတ္ထုသိုလှောင်မှု။
4. **အခြေအနေထည့်သွင်းမှုများကို အပ်ဒိတ်လုပ်ပါ။** `status.md` တွင် AND5 မှတ်တိုင်များ တိုးလာသောအခါ၊ ကိုးကားပါ
   သက်ဆိုင်ရာဖိုင် (ဥပမာ၊ `docs/automation/android/parity/20260324T010203Z-summary.json`)
   ထို့ကြောင့် စာရင်းစစ်များသည် CI မှတ်တမ်းများကို မခြစ်ဘဲ အခြေခံအချက်များကို ခြေရာခံနိုင်သည်။

ဤအပြင်အဆင်ကို လိုက်နာခြင်းဖြင့် ရရှိနိုင်သော “docs/automation baselines များကို ကျေနပ်စေသည်။
စာရင်းစစ်” AND6 သည် Android စာရွက်စာတမ်းပြုစုခြင်းပရိုဂရမ်ကို ကိုးကား၍ သိမ်းဆည်းထားခြင်းဖြစ်သည်။
ထုတ်ပြန်ထားသော အစီအစဉ်များနှင့်အတူ lockstep တွင်။