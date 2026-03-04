---
lang: my
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-12-29T18:16:35.091815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Brownout / Downgrade Response Playbook

1. **ဖော်ထုတ်ရန်**
   - သတိပေးချက် `soranet_privacy_circuit_events_total{kind="downgrade"}` မီးလောင်မှု သို့မဟုတ်
     brownout webhook သည် အုပ်ချုပ်ရေးမှ အစပျိုးသည်။
   - `kubectl logs soranet-relay` သို့မဟုတ် 5 မိနစ်အတွင်း systemd ဂျာနယ်မှတဆင့်အတည်ပြုပါ။

2. **တည်ငြိမ်အောင်**
   - Freeze guard rotation (`relay guard-rotation disable --ttl 30m`)။
   - သက်ရောက်မှုရှိသော သုံးစွဲသူများအတွက် တိုက်ရိုက်သာ အစားထိုးမှုကို ဖွင့်ပါ။
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`)။
   - လက်ရှိလိုက်နာမှု config hash (`sha256sum compliance.toml`) ကိုရိုက်ပါ။

၃။ **ရောဂါရှာဖွေခြင်း**
   - နောက်ဆုံးပေါ် လမ်းညွှန်လျှပ်တစ်ပြက်ရိုက်ချက်များနှင့် ထပ်လောင်းတိုင်းတာမှုအစုအဝေးကို စုဆောင်းပါ-
     `soranet-relay support-bundle --output /tmp/bundle.tgz`။
   - PoW တန်းစီ၏အတိမ်အနက်၊ အခိုးအငွေ့ကောင်တာများနှင့် GAR အမျိုးအစားကွဲများကို သတိပြုပါ။
   - PQ လိုငွေပြမှု၊ လိုက်နာမှု အစားထိုးမှု သို့မဟုတ် ထပ်ဆင့်ပေးပို့မှု ပျက်ကွက်ခြင်း ရှိမရှိ ခွဲခြားသတ်မှတ်ပါ။

4. **အရှိန်မြှင့်**
   - အုပ်ချုပ်မှုတံတား (`#soranet-incident`) ကို အကျဉ်းချုပ်နှင့် အတွဲလိုက် hash ဖြင့် အကြောင်းကြားပါ။
   - အချိန်တံဆိပ်ခေါင်းများနှင့် လျော့ပါးရေးအဆင့်များအပါအဝင် သတိပေးချက်သို့ ချိတ်ဆက်ထားသော အဖြစ်အပျက်လက်မှတ်ကို ဖွင့်ပါ။

5. **ပြန်လည်ရယူရန်**
   - root အကြောင်းရင်းကိုဖြေရှင်းပြီးသည်နှင့်လှည့်ခြင်းကိုပြန်ဖွင့်ပါ။
     (`relay guard-rotation enable`) နှင့် တိုက်ရိုက်-သီးသန့် overridesများကို ပြန်ပြောင်းပါ။
   - KPIs ကို မိနစ် 30 စောင့်ကြည့်ပါ။ အညိုကွက်အသစ်များ မပေါ်လာကြောင်း သေချာပါစေ။

6. **သေဆုံးမှု**
   - အုပ်ချုပ်မှုပုံစံပုံစံကို အသုံးပြု၍ အဖြစ်အပျက်အစီရင်ခံစာကို ၄၈ နာရီအတွင်း တင်ပြပါ။
   - ချို့ယွင်းချက်မုဒ်အသစ်တွေ့ရှိပါက runbook များကို အပ်ဒိတ်လုပ်ပါ။