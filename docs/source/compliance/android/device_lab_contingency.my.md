---
lang: my
direction: ltr
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4016b82d86dc61a9de5e345950d02aeadf26db4cc26777c60db336c57479ba15
source_last_modified: "2025-12-29T18:16:35.923121+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# စက်ပစ္စည်းဓာတ်ခွဲခန်း အရေးပေါ်မှတ်တမ်း

Android စက်-ဓာတ်ခွဲခန်း အရေးပေါ်အစီအစဉ်၏ အသက်သွင်းမှုတိုင်းကို ဤနေရာတွင် မှတ်တမ်းတင်ပါ။
လိုက်နာမှုပြန်လည်သုံးသပ်ခြင်းနှင့် အနာဂတ်အဆင်သင့်စစ်ဆေးမှုများအတွက် လုံလောက်သောအသေးစိတ်အချက်အလက်များကို ထည့်သွင်းပါ။

| ရက်စွဲ | အစပျိုး | အရေးယူဆောင်ရွက်မှုများ | နောက်ဆက်တွဲ | ပိုင်ရှင် |
|------|---------|----------------|---------------------|--------|
| 2026-02-11 | Pixel8 Pro လမ်းကြောပြတ်တောက်ပြီး Pixel8a ပေးပို့မှုနှောင့်နှေးပြီးနောက် စွမ်းဆောင်ရည်သည် 78% သို့ ကျဆင်းသွားသည် (`android_strongbox_device_matrix.md` ကိုကြည့်ပါ)။ | Pixel7 ၏ အဓိက CI ပစ်မှတ်သို့ အရောင်းမြှင့်တင်ခဲ့သည်၊ မျှဝေထားသော Pixel6 ရေယာဉ်စုကို ငှားရမ်းကာ၊ လက်လီ-ပိုက်ဆံအိတ်နမူနာအတွက် Firebase Test Lab မီးခိုးစမ်းသပ်မှုများနှင့် AND6 အစီအစဉ်အတွက် ပြင်ပ StrongBox ဓာတ်ခွဲခန်းနှင့် ချိတ်ဆက်ထားသည်။ | Pixel8 Pro အတွက် အမှားအယွင်းရှိသော USB-C hub ကို အစားထိုးပါ (2026-02-15 ရက်စွဲပါ); Pixel8a ဆိုက်ရောက်မှုနှင့် ပြန်လည်အခြေခံနိုင်မှု အစီရင်ခံစာကို အတည်ပြုပါ။ | Hardware Lab Lead |
| 2026-02-13 | Pixel8 Pro hub ကို အစားထိုးလိုက်ပြီး GalaxyS24 ကို အတည်ပြုပြီး စွမ်းဆောင်ရည် 85% ကို ပြန်လည်ရယူသည်။ | Pixel7 သည် ဒုတိယလမ်းသွားဖြစ်ပြီး၊ `android-strongbox-attestation` Buildkite အလုပ်အား `pixel8pro-strongbox-a` နှင့် `s24-strongbox-a`၊ အဆင်သင့်မက်ထရစ် + အထောက်အထားမှတ်တမ်း အပ်ဒိတ်ဖြင့် ပြန်လည်ဖွင့်ထားသည်။ | Pixel8a ပေးပို့မှု ETA (ဆိုင်းငံ့ဆဲ); spare hub inventory ကို မှတ်တမ်းတင်ထားပါ။ | Hardware Lab Lead |