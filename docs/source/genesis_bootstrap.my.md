---
lang: my
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2025-12-29T18:16:35.962003+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ယုံကြည်ရသောရွယ်တူများမှ Genesis Bootstrap

Iroha သည် ဒေသန္တရ `genesis.file` မပါဘဲ ရွယ်တူများသည် ယုံကြည်စိတ်ချရသောရွယ်တူများထံမှ လက်မှတ်ရေးထိုးထားသော ဥပါဒ်ပိတ်ဆို့ခြင်းကို ရယူနိုင်သည်
Norito-ကုဒ်ဝှက်ထားသော bootstrap ပရိုတိုကောကို အသုံးပြုခြင်း။

- **Protocol:** ရွယ်တူများသည် `GenesisRequest` (မက်တာဒေတာအတွက် `Preflight`၊ payload အတွက် `Fetch`) နှင့်
  `GenesisResponse` ဘောင်များကို `request_id` သော့ခတ်ထားသည်။ တုံ့ပြန်သူများတွင် ကွင်းဆက် ID၊ လက်မှတ်ထိုးသည့် pubkey၊
  hash နှင့် ရွေးချယ်နိုင်သော အရွယ်အစား အရိပ်အမြွက်။ payload များကို `Fetch` တွင်သာ ပြန်ပေးမည်ဖြစ်ပြီး တောင်းဆိုချက် ids ပွားနေသည်
  `DuplicateRequest` ကို လက်ခံရယူပါ။
- **အစောင့်များ-** တုံ့ပြန်သူများသည် ခွင့်ပြုစာရင်း (`genesis.bootstrap_allowlist` သို့မဟုတ် ယုံကြည်ရသော လုပ်ဖော်ကိုင်ဖက်များ
  သတ်မှတ်ထားသည်)၊ chain-id/pubkey/hash ကိုက်ညီမှု၊ နှုန်းကန့်သတ်ချက်များ (`genesis.bootstrap_response_throttle`) နှင့်
  ဦးထုပ်အရွယ်အစား (`genesis.bootstrap_max_bytes`)။ ခွင့်ပြုစာရင်းပြင်ပမှ တောင်းဆိုချက်များ `NotAllowed` တို့ကို လက်ခံရရှိသည်။
  သော့မှားဖြင့် ရေးထိုးထားသော payload များသည် `MismatchedPubkey` ကို လက်ခံရရှိသည် ။
- **Requester flow:** သိုလှောင်မှုဗလာဖြစ်ပြီး `genesis.file` ကို သတ်မှတ်မထားသည့်အခါ (လည်းကောင်း၊
  `genesis.bootstrap_enabled=true`)၊ node သည် ရွေးချယ်နိုင်မှုဖြင့် ယုံကြည်စိတ်ချရသော ရွယ်တူများကို ရွေးချယ်နိုင်သည်
  `genesis.expected_hash`၊ ထို့နောက် payload ကိုရယူပြီး၊ `validate_genesis_block` မှတစ်ဆင့် လက်မှတ်များကို တရားဝင်အတည်ပြုပေးသည်။
  ပိတ်ဆို့ခြင်းကို မကျင့်သုံးမီ Kura နှင့်အတူ `genesis.bootstrap.nrt` ကို ဆက်လက်လုပ်ဆောင်ပါ။ Bootstrap ပြန်စမ်းပါ။
  ဂုဏ်ပြု `genesis.bootstrap_request_timeout`၊ `genesis.bootstrap_retry_interval` နှင့်
  `genesis.bootstrap_max_attempts`။
- ** ပျက်ကွက်မှုမုဒ်များ- ** ခွင့်ပြုစာရင်းလွဲချော်မှုများ၊ ကွင်းဆက်/ပုဘ်ကီး/ hash မကိုက်ညီမှုများ၊ အရွယ်အစားအတွက် တောင်းဆိုမှုများကို ပယ်ချပါသည်။
  ဦးထုပ်ချိုးဖောက်မှုများ၊ နှုန်းကန့်သတ်ချက်များ၊ ဒေသန္တရဥပါဒ်မရှိသော၊ သို့မဟုတ် ထပ်နေသည့် တောင်းဆိုချက် ids များ။ ကွဲလွဲနေသော hashe များ
  ရွယ်တူချင်းများ ဖြတ်၍ အကျိူးကို ဖျက်ချသည်။ တုံ့ပြန်သူများ/ အချိန်ကုန်သွားခြင်းများသည် ဒေသတွင်း ဖွဲ့စည်းမှုပုံစံသို့ ပြန်ရောက်သွားခြင်းမရှိပါ။
- **အော်ပရေတာအဆင့်များ-** အနည်းဆုံးယုံကြည်ရသောရွယ်တူတစ်ဦးသည် မှန်ကန်သောဥပါဒ်တစ်ခုဖြင့်ရောက်ရှိနိုင်ကြောင်းသေချာစေရန်၊ စီစဉ်သတ်မှတ်ပါ
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` နှင့် ပြန်လည်ကြိုးစားသည့်ခလုတ်များကို လည်းကောင်း၊
  ကိုက်ညီမှုမရှိသော ပေးဆောင်မှုများကို လက်ခံခြင်းမှရှောင်ရှားရန် `expected_hash` ကို ပင်ထိုးရွေးချယ်နိုင်သည်။ Persisted payloads တွေ ဖြစ်နိုင်ပါတယ်။
  `genesis.file` ကို `genesis.bootstrap.nrt` သို့ ညွှန်ပြခြင်းဖြင့် နောက်ဘွတ်ဖိနပ်များတွင် ပြန်လည်အသုံးပြုသည်။