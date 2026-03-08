---
lang: my
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

SDK နှင့် Codec ပိုင်ရှင်များအတွက် # IH58 ထုတ်ဝေမှုမှတ်စု

အဖွဲ့များ- Rust SDK၊ TypeScript/JavaScript SDK၊ Python SDK၊ Kotlin SDK၊ Codec tooling

အကြောင်းအရာ- ယခု `docs/account_structure.md` သည် IH58 အကောင့် ID ကို ရောင်ပြန်ဟပ်ပါသည်။
အကောင်အထည်ဖော်ခြင်း။ SDK အပြုအမူနှင့် စမ်းသပ်မှုများကို canonical spec နှင့် ချိန်ညှိပါ။

အဓိက ကိုးကားချက်များ-
- လိပ်စာကုဒ်ဒက် + ခေါင်းစီးအပြင်အဆင် — `docs/account_structure.md` §2
- Curve registry — `docs/source/references/address_curve_registry.md`
- Norm v1 ဒိုမိန်း ကိုင်တွယ်ခြင်း — `docs/source/references/address_norm_v1.md`
- Fixture vectors — `fixtures/account/address_vectors.json`

လုပ်ဆောင်ချက်များ-
1. ** Canonical အထွက်-** `AccountId::to_string()`/Display သည် IH58 ကိုသာ ထုတ်လွှတ်ရမည်
   (နံပါတ် `@domain` နောက်ဆက်တွဲ)။ Canonical hex သည် အမှားရှာပြင်ခြင်းအတွက် (`0x...`) ဖြစ်သည်။
2. **လက်ခံထားသော သွင်းအားစုများ-** ခွဲခြမ်းစိတ်ဖြာသူများသည် IH58 (နှစ်သက်ရာ)၊ `sora` ဖိသိပ်မှုကို လက်ခံရမည်၊
   နှင့် canonical hex (`0x...` only; bare hex ကို ငြင်းပယ်သည်)။ သွင်းအားစုများ သယ်ဆောင်လာနိုင်သည်။
   လမ်းကြောင်း အရိပ်အမြွက်များအတွက် `@<domain>` နောက်ဆက်တွဲ။ `<label>@<domain>` (rejected legacy form) aliases လိုအပ်သည်။
   ဖြေရှင်းသူ။  3. **ဖြေရှင်းသူများ-** domainless IH58/sora ခွဲခြမ်းစိတ်ဖြာခြင်း domain-selector တစ်ခု လိုအပ်သည်
   ရွေးချယ်သူသည် သွယ်ဝိုက်သောပုံသေမဟုတ်ပါက ဖြေရှင်းသူ (ပြင်ဆင်ထားသော ပုံသေကိုသုံးပါ။
   ဒိုမိန်းတံဆိပ်)။ UAID (`uaid:...`) နှင့် opaque (`opaque:...`) စာသားများ လိုအပ်သည်
   ဖြေရှင်းသူများ။
4. **IH58 checksum-** `IH58PRE || prefix || payload` ကျော် Blake2b-512 ကိုသုံးပါ၊ ယူပါ။
   ပထမ 2 bytes ။ ချုံ့ထားသော အက္ခရာအခြေခံသည် **105** ဖြစ်သည်။
5. ** Curve gating-** SDKs များသည် မူရင်း Ed25519-only သို့ဖြစ်သည်။ တိကျပြတ်သားသော ရွေးချယ်မှုကို ပေးပါ။
   ML-DSA/GOST/SM (Swift build flags; JS/Android `configureCurveSupport`)။ လုပ်ပါ။
   secp256k1 ကို Rust ပြင်ပတွင် ပုံမှန်အားဖြင့် ဖွင့်ထားသည်ဟု မထင်ပါ။
6. ** CAIP-10 မရှိပါ :** ပို့ဆောင်ထားသော CAIP-10 မြေပုံဆွဲခြင်းမရှိသေးပါ။ မဖော်ထုတ်ပါနှင့်
   CAIP-10 ပြောင်းလဲမှုများအပေါ် မူတည်သည်။

codecs/tests များကို အပ်ဒိတ်လုပ်ပြီးသည်နှင့် အတည်ပြုပါ။ အဖွင့်မေးခွန်းများကို ခြေရာခံနိုင်ပါသည်။
account-addressing RFC thread တွင်။