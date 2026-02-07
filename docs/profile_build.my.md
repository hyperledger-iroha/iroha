---
lang: my
direction: ltr
source: docs/profile_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-12-29T18:16:35.912559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Profileing `iroha_data_model` Build

`iroha_data_model` တွင် နှေးကွေးသော တည်ဆောက်မှု အဆင့်များကို ရှာဖွေရန်၊ helper script ကို run ပါ-

```sh
./scripts/profile_build.sh
```

၎င်းသည် `cargo build -p iroha_data_model --timings` ကို လုပ်ဆောင်ပြီး `target/cargo-timings/` သို့ အချိန်ကိုက် အစီရင်ခံစာများကို ရေးသားသည်။
`cargo-timing.html` ကို ဘရောက်ဆာတွင်ဖွင့်ပြီး မည်သည့်သေတ္တာများ သို့မဟုတ် တည်ဆောက်မှုအဆင့်များ အချိန်အများဆုံးယူသည်ကို ကြည့်ရန် ကြာချိန်အလိုက် လုပ်ဆောင်စရာများကို စီပါ။

အနှေးဆုံးအလုပ်များတွင် အကောင်းဆုံးဖြစ်အောင် အာရုံစိုက်ရန် အချိန်များကို အသုံးပြုပါ။