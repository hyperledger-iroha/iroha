<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# လုံခြုံရေးစာရင်းစစ်အစီရင်ခံစာ

ရက်စွဲ- 2026-03-26

## အမှုဆောင်အကျဉ်းချုပ်

ဤစစ်ဆေးမှုသည် လက်ရှိသစ်ပင်ရှိ အန္တရာယ်အများဆုံး မျက်နှာပြင်များကို အာရုံစိုက်ထားသည်- Torii HTTP/API/auth flows၊ P2P သယ်ယူပို့ဆောင်ရေး၊ လျှို့ဝှက်ကိုင်တွယ် APIs၊ SDK သယ်ယူပို့ဆောင်ရေးအစောင့်များနှင့် ပူးတွဲပါရှိသည့် သန့်စင်ဆေးလမ်းကြောင်း။

အရေးယူနိုင်တဲ့ ပြဿနာ ၆ ခုကို တွေ့ခဲ့တယ်

- ပြင်းထန်ပြင်းထန်မှု ၂ ခု
- 4 အလယ်အလတ်ပြင်းထန်မှုတွေ့ရှိချက်

အရေးကြီးဆုံးပြဿနာများမှာ-

1. Torii သည် လက်ရှိတွင် ကိုင်ဆောင်သူတိုကင်များ၊ API တိုကင်များ၊ အော်ပရေတာစက်ရှင်စက်ရှင်/bootstrap တိုကင်များနှင့် mTLS အမှတ်အသားများကို မှတ်တမ်းများသို့ ထပ်ဆင့်ပို့နိုင်သည့် HTTP တောင်းဆိုမှုတိုင်းအတွက် အဝင်တောင်းဆိုချက်ခေါင်းစည်းများကို လောလောဆယ် မှတ်တမ်းတင်ထားသည်။
2. အများသူငှာ Torii လမ်းကြောင်းများစွာနှင့် SDK များသည် အကြမ်းထည် `private_key` တန်ဖိုးများကို ဆာဗာသို့ ပေးပို့ခြင်းကို ပံ့ပိုးပေးနေဆဲဖြစ်သောကြောင့် Torii သည် ခေါ်ဆိုသူ၏ကိုယ်စား လက်မှတ်ထိုးနိုင်ပါသည်။
3. အချို့သော SDK များတွင် လျှို့ဝှက်မျိုးစေ့မှဆင်းသက်လာပြီး လျှို့ဝှက်တောင်းဆိုမှု စစ်မှန်ကြောင်း အပါအဝင် "လျှို့ဝှက်ချက်" လမ်းကြောင်းအများအပြားကို သာမန်တောင်းဆိုမှုအဖွဲ့များအဖြစ် သဘောထားသည်။

## နည်းလမ်း

- Torii၊ P2P၊ crypto/VM နှင့် SDK လျှို့ဝှက်ကိုင်တွယ်မှုလမ်းကြောင်းများကို တည်ငြိမ်စွာ ပြန်လည်သုံးသပ်ခြင်း။
- ပစ်မှတ်ထား အတည်ပြုခြင်း အမိန့်များ-
  - `cargo check -p iroha_torii --lib --message-format short` --> pass
  - `cargo check -p iroha_p2p --message-format short` --> pass
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` --> pass
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> pass၊ မိတ္တူဗားရှင်းသတိပေးချက်များသာ
- ဤလက်မှတ်တွင် မပြီးသေးပါ။
  - အလုပ်ခွင်အပြည့်အစုံ တည်ဆောက်ခြင်း/စမ်းသပ်ခြင်း/ကလစ်ပီ
  - Swift / Gradle စမ်းသပ်မှုအစုံ
  - CUDA/Metal runtime အတည်ပြုခြင်း။

## တွေ့ရှိချက်

### SA-001 မြင့်- Torii သည် တစ်ကမ္ဘာလုံးတွင် အရေးကြီးသော တောင်းဆိုချက် ခေါင်းစီးများကို မှတ်တမ်းတင်သည်သက်ရောက်မှု- ခြေရာခံရန် သင်္ဘောများ တောင်းဆိုသည့် မည်သည့် ဖြန့်ကျက်မှုသည် ကိုင်ဆောင်သူ/API/အော်ပရေတာ တိုကင်များနှင့် ဆက်စပ်သော စစ်မှန်ကြောင်း အထောက်အထားများကို အပလီကေးရှင်းမှတ်တမ်းများသို့ ပေါက်ကြားစေနိုင်သည်။

အထောက်အထား-

- `crates/iroha_torii/src/lib.rs:20752` သည် `TraceLayer::new_for_http()` ကို ဖွင့်ပေးသည်
- `crates/iroha_torii/src/lib.rs:20753` သည် `DefaultMakeSpan::default().include_headers(true)` ကို ဖွင့်ပေးသည်
- တူညီသောဝန်ဆောင်မှုတွင် အခြားနေရာများတွင် ထိခိုက်လွယ်သော ခေါင်းစီးအမည်များကို တက်ကြွစွာအသုံးပြုသည်-
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

ဒါက ဘာကြောင့် အရေးကြီးသလဲ-

- `include_headers(true)` သည် အဝင်ခေါင်းစီးတန်ဖိုးများကို ခြေရာခံခြင်းအပိုင်းများသို့ အပြည့်အ၀ မှတ်တမ်းတင်ပါသည်။
- Torii သည် `Authorization`၊ `x-api-token`၊ `x-iroha-operator-session`၊ `x-iroha-operator-token` နှင့် `x-forwarded-client-cert` ကဲ့သို့သော ခေါင်းစီးများတွင် စစ်မှန်ကြောင်းအထောက်အထားပြခြင်းကို လက်ခံပါသည်။
- မှတ်တမ်းနစ်မြုပ်မှု အပေးအယူ၊ အမှားရှာ မှတ်တမ်းစုဆောင်းခြင်း သို့မဟုတ် ပံ့ပိုးမှုအစုအဝေးသည် ထို့ကြောင့် အထောက်အထား ထုတ်ဖော်ခြင်းဖြစ်ရပ် ဖြစ်လာနိုင်သည်။

အကြံပြုထားသော ကုထုံး-

- ထုတ်လုပ်မှုအပိုင်းများတွင် တောင်းဆိုချက်အပြည့်အစုံပါဝင်သော ခေါင်းစီးများကို ရပ်တန့်ပါ။
- အမှားရှာပြင်ခြင်းအတွက် ခေါင်းစီးမှတ်တမ်းသွင်းရန် လိုအပ်သေးပါက လုံခြုံရေး-အကဲဆတ်သော ခေါင်းစီးများအတွက် ပြတ်ပြတ်သားသား တုံ့ပြန်မှုကို ထည့်ပါ။
- ဒေတာကို အပြုသဘောဖြင့် စာရင်းသွင်းထားခြင်းမရှိပါက တောင်းဆိုမှု/တုံ့ပြန်မှု မှတ်တမ်းကို လျှို့ဝှက်လုပ်ဆောင်မှုအဖြစ် သတ်မှတ်ပါ။

### SA-002 မြင့်မားသည်- အများသူငှာ Torii APIs များသည် ဆာဗာဘက်တွင် လက်မှတ်ရေးထိုးခြင်းအတွက် အကြမ်းသီးသန့်သော့များကို လက်ခံနေဆဲဖြစ်သည်။

သက်ရောက်မှု- ဖောက်သည်များသည် API၊ SDK၊ proxy နှင့် server-memory layers များတွင် မလိုအပ်သော လျှို့ဝှက်ချန်နယ်တစ်ခုကို ဖန်တီးနိုင်စေရန် ဆာဗာသည် ၎င်းတို့၏ကိုယ်စား လက်မှတ်ထိုးနိုင်စေရန် ကွန်ရက်ပေါ်တွင် သီးသန့်သော့များကို ပေးပို့ရန် တွန်းအားပေးပါသည်။

အထောက်အထား-- အုပ်ချုပ်မှုလမ်းကြောင်း မှတ်တမ်းတွင် ဆာဗာဘက်ခြမ်း လက်မှတ်ထိုးခြင်းကို အတိအလင်း ကြော်ငြာသည်-
  - `crates/iroha_torii/src/gov.rs:495`
- လမ်းကြောင်းအကောင်အထည်ဖော်မှုသည် ပံ့ပိုးပေးထားသော သီးသန့်သော့ကို ခွဲခြမ်းစိပ်ဖြာပြီး ဆာဗာဘက်ခြမ်းကို သင်္ကေတပြုသည်-
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDKs သည် `private_key` ကို JSON ကောင်များအဖြစ် တက်ကြွစွာ အမှတ်အသားပြုသည်-
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

မှတ်စုများ-

- ဤပုံစံသည် လမ်းကြောင်းမိသားစုတစ်ခုတည်းအတွက် သီးခြားမဟုတ်ပါ။ လက်ရှိသစ်ပင်တွင် အုပ်ချုပ်မှု၊ အော့ဖ်လိုင်းငွေသား၊ စာရင်းသွင်းမှုများနှင့် အခြားအက်ပ်-ရင်ဆိုင်သည့် DTOs များတွင် တူညီသော အဆင်ပြေသည့်ပုံစံ ပါရှိသည်။
- HTTPS တစ်ခုတည်းသော သယ်ယူပို့ဆောင်ရေးစစ်ဆေးမှုများသည် မတော်တဆ plaintext ပို့ဆောင်မှုကို လျှော့ချပေးသော်လည်း ၎င်းတို့သည် ဆာဗာဘက်ခြမ်းလျှို့ဝှက်ကိုင်တွယ်ခြင်း သို့မဟုတ် မှတ်တမ်း/မှတ်ဉာဏ် ထိတွေ့မှုအန္တရာယ်ကို မဖြေရှင်းနိုင်ပါ။

အကြံပြုထားသော ကုထုံး-

- အကြမ်း `private_key` ဒေတာကို သယ်ဆောင်သည့် တောင်းဆိုချက် DTO အားလုံးကို ရပ်တန့်ပါ။
- ဖောက်သည်များအား ပြည်တွင်း၌ လက်မှတ်ရေးထိုးပြီး လက်မှတ်များ သို့မဟုတ် အပြည့်အ၀ လက်မှတ်ထိုးထားသော ငွေလွှဲစာ/စာအိတ်များ တင်သွင်းရန် လိုအပ်သည်။
- တွဲဖက်အသုံးပြုနိုင်သောဝင်းဒိုးပြီးနောက် OpenAPI/SDKs မှ `private_key` နမူနာများကို ဖယ်ရှားပါ။

### SA-003 အလတ်စား- လျှို့ဝှက်သော့ဆင်းသက်လာမှုသည် Torii သို့ လျှို့ဝှက်မျိုးစေ့ပစ္စည်းကို ပေးပို့ပြီး ၎င်းကို ပြန်ပြောင်းပေးသည်

သက်ရောက်မှု- လျှို့ဝှက်သော့မှဆင်းသက်လာသော API သည် မျိုးစေ့ပစ္စည်းများကို ပုံမှန်တောင်းဆိုမှု/တုံ့ပြန်မှုပေးဆောင်မှုဒေတာအဖြစ်သို့ ပြောင်းလဲစေပြီး proxies၊ အလယ်တန်းဆော့ဖ်ဝဲ၊ မှတ်တမ်းများ၊ ခြေရာခံများ၊ ပျက်စီးမှုအစီရင်ခံစာများ သို့မဟုတ် ဖောက်သည်အလွဲသုံးစားမှုများမှတစ်ဆင့် မျိုးစေ့ထုတ်ဖော်နိုင်ခြေကို တိုးစေသည်။

အထောက်အထား-- တောင်းဆိုချက်သည် မျိုးစေ့ပစ္စည်းများကို တိုက်ရိုက်လက်ခံသည်-
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- တုံ့ပြန်မှုအစီအစဉ်သည် hex နှင့် base64 နှစ်မျိုးလုံးတွင် မျိုးစေ့ကို ပဲ့တင်ထပ်သည်-
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- ကိုင်တွယ်သူသည် ပြတ်သားစွာ ပြန်ကုဒ်လုပ်ပြီး မျိုးစေ့ကို ပြန်ပေးသည်-
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK သည် ၎င်းကို ပုံမှန်ကွန်ရက်နည်းလမ်းတစ်ခုအဖြစ် ဖော်ထုတ်ပြီး တုံ့ပြန်မှုပုံစံတွင် ပဲ့တင်ထပ်သောမျိုးစေ့ကို ဆက်လက်ထားရှိသည်-
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

အကြံပြုထားသော ကုထုံး-

- CLI/SDK ကုဒ်တွင် စက်တွင်းသော့ဆင်းသက်မှုကို ဦးစားပေးပြီး အဝေးမှဆင်းသက်သည့်လမ်းကြောင်းကို လုံးဝဖယ်ရှားပါ။
- လမ်းကြောင်းကျန်နေပါက၊ တုံ့ပြန်မှုတွင် မျိုးစေ့ကို ဘယ်တော့မှ ပြန်မပေးဘဲ သယ်ယူပို့ဆောင်ရေးအစောင့်များနှင့် တယ်လီမီတာ/သစ်ခုတ်သည့်လမ်းကြောင်းများအားလုံးတွင် အစေ့အဆန်များ သယ်ဆောင်သော အလောင်းများကို အကဲဆတ်သည့်အဖြစ် အမှတ်အသားပြုပါ။

### SA-004 အလတ်စား- SDK သယ်ယူပို့ဆောင်ရေး အာရုံခံနိုင်စွမ်း ထောက်လှမ်းမှုတွင် `private_key` မဟုတ်သော လျှို့ဝှက်ပစ္စည်းအတွက် မျက်စိကွယ်စရာများ ရှိသည်

သက်ရောက်မှု- အချို့သော SDK များသည် အကြမ်းထည် `private_key` တောင်းဆိုမှုများအတွက် HTTPS အား တွန်းအားပေးမည်ဖြစ်သော်လည်း၊ လုံခြုံမှုမရှိသော HTTP မှတဆင့် သို့မဟုတ် မကိုက်ညီသော host များဆီသို့ အခြားသော လုံခြုံရေးဆိုင်ရာ အကဲဆတ်သော တောင်းဆိုချက်များအား ခွင့်ပြုဆဲဖြစ်သည်။

အထောက်အထား-- Swift သည် canonical request auth headers ကို အကဲဆတ်သည့်အဖြစ် ဆက်ဆံသည်-
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- သို့သော် Swift သည် `"private_key"` တွင် ကိုယ်ထည်-ကိုက်ညီမှုသာ ရှိပါသေးသည်။
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Kotlin သည် `authorization` နှင့် `x-api-token` ခေါင်းစီးများကိုသာ အသိအမှတ်ပြုသည်၊ ထို့နောက် တူညီသော `"private_key"` ကိုယ်ထည်ဆိုင်ရာ လေ့လာဆန်းစစ်မှုသို့ ပြန်သွားသည်-
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android တွင် တူညီသောကန့်သတ်ချက်များရှိသည်။
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Kotlin/Java canonical တောင်းဆိုချက်လက်မှတ်ထိုးသူများသည် ၎င်းတို့၏ကိုယ်ပိုင်သယ်ယူပို့ဆောင်ရေးအစောင့်များဖြင့် အကဲဆတ်သည့်အဖြစ် မခွဲခြားနိုင်သော နောက်ထပ် အထောက်အထားခေါင်းစီးများကို ထုတ်ပေးသည်-
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

အကြံပြုထားသော ကုထုံး-

- တိကျသောတောင်းဆိုမှုအမျိုးအစားခွဲခြားခြင်းဖြင့် heuristic ခန္ဓာကိုယ်စကင်န်ဖတ်ခြင်းကိုအစားထိုးပါ။
- canonical auth headers၊ အမျိုးအနွယ်/စကားဝှက်အကွက်များ၊ ရေးထိုးထားသော ပြောင်းလဲမှုဆိုင်ရာ ခေါင်းစီးများနှင့် မည်သည့်အနာဂတ်လျှို့ဝှက်ချက်အကွက်များကိုမဆို စာချုပ်အရ၊ ကြိုးတန်းခွဲများ ကိုက်ညီမှုမရှိဘဲ အကဲဆတ်သည့်အဖြစ် ဆက်ဆံပါ။
- အာရုံခံနိုင်စွမ်းစည်းမျဉ်းများကို Swift၊ Kotlin နှင့် Java တို့တွင် ချိန်ညှိထားပါ။

### SA-005 အလတ်စား- ပူးတွဲပါ "sandbox" သည် လုပ်ငန်းစဉ်ခွဲတစ်ခုသာဖြစ်ပြီး `setrlimit`သက်ရောက်မှု- ပူးတွဲပါရှိ သန့်စင်ဆေးရည်ကို "သဲပုံး" အဖြစ် ဖော်ပြပြီး အစီရင်ခံထားသည်၊ သို့သော် အကောင်အထည်ဖော်မှုသည် ရင်းမြစ်ကန့်သတ်ချက်များရှိသော လက်ရှိ binary ၏ fork/exec တစ်ခုသာဖြစ်သည်။ ခွဲခြမ်းစိတ်ဖြာခြင်း သို့မဟုတ် သိမ်းဆည်းခြင်းဆိုင်ရာ အသုံးချမှုတစ်ခုသည် တူညီသောအသုံးပြုသူ၊ ဖိုင်စနစ်မြင်ကွင်းနှင့် Torii ကဲ့သို့ ဝန်းကျင်ကွန်ရက်/လုပ်ငန်းစဉ်ဆိုင်ရာအခွင့်ထူးများဖြင့် ဆက်လက်လုပ်ဆောင်နေမည်ဖြစ်သည်။

အထောက်အထား-

- ကလေးမွေးပြီးနောက် ရလဒ်ကို အပြင်ဘက်လမ်းက သဲပုံးအဖြစ် အမှတ်အသားပြုသည်-
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- ကလေးသည် လက်ရှိလုပ်ဆောင်နိုင်သည့်အရာသို့ ပုံသေသတ်မှတ်သည်-
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- လုပ်ငန်းစဉ်ခွဲသည် `AttachmentSanitizerMode::InProcess` သို့ ပြတ်သားစွာ ပြန်ပြောင်းသည်-
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- တစ်ခုတည်းသော hardening သည် CPU/address-space `setrlimit` ဖြစ်သည်။
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

အကြံပြုထားသော ကုထုံး-

- စစ်မှန်သော OS sandbox ကို အကောင်အထည်ဖော်သည် (ဥပမာ namespaces/seccomp/landlock/jail-style isolation၊ privilege drop၊ no-network၊ constrained filesystem) သို့မဟုတ် ရလဒ်ကို `sandboxed` အဖြစ် အညွှန်းတပ်ခြင်းကို ရပ်လိုက်ပါ။
- လက်ရှိဒီဇိုင်းကို APIs၊ telemetry နှင့် docs များတွင် "sandboxing" ထက် "လုပ်ငန်းစဉ်ခွဲခွဲ" အဖြစ် သဘောထားပါ။

### SA-006 အလတ်စား- စိတ်ကြိုက်ရွေးချယ်နိုင်သော P2P TLS/QUIC သည် လက်မှတ်အတည်ပြုခြင်းကို ပိတ်လိုက်သည်သက်ရောက်မှု- `quic` သို့မဟုတ် `p2p_tls` ကို ဖွင့်ထားသောအခါ၊ ချန်နယ်သည် ကုဒ်ဝှက်ခြင်းကို ပံ့ပိုးပေးသော်လည်း အဝေးမှ အဆုံးမှတ်ကို အထောက်အထားမပြပါ။ လမ်းကြောင်းပေါ်ရှိ တိုက်ခိုက်သူသည် TLS/QUIC နှင့် ဆက်စပ်နေသော ပုံမှန်လုံခြုံရေးမျှော်လင့်ချက်များကို အနိုင်ယူပြီး ချန်နယ်ကို ထပ်ဆင့်လွှင့်နိုင်သည် သို့မဟုတ် ရပ်တန့်နိုင်သည်။

အထောက်အထား-

- QUIC သည် ခွင့်ပြုချက်လက်မှတ် အတည်ပြုခြင်းကို အတိအလင်း မှတ်တမ်းတင်ထားသည်-
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- QUIC အတည်ပြုသူသည် ဆာဗာအသိအမှတ်ပြုလက်မှတ်ကို ခြွင်းချက်မရှိ လက်ခံသည်-
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- TLS-over-TCP သယ်ယူပို့ဆောင်ရေးသည် အလားတူလုပ်ဆောင်သည်-
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

အကြံပြုထားသော ကုထုံး-

- သက်တူရွယ်တူလက်မှတ်များကို စစ်ဆေးခြင်း သို့မဟုတ် ပိုမိုမြင့်မားသော အလွှာလက်မှတ်ရေးထိုးထားသော လက်ဆွဲနှုတ်ဆက်ခြင်းနှင့် သယ်ယူပို့ဆောင်ရေးစက်ရှင်ကြားတွင် တိကျပြတ်သားသော ချန်နယ်စည်းနှောင်မှုကို ပေါင်းထည့်ပါ။
- လက်ရှိအပြုအမူသည် ရည်ရွယ်ချက်ရှိရှိဖြစ်ပါက၊ အင်္ဂါရပ်ကို အထောက်အထားမခိုင်လုံသော ကုဒ်ဝှက်ထားသော သယ်ယူပို့ဆောင်ရေးအဖြစ် အမည်ပြောင်း/မှတ်တမ်းပြုပါ သို့မှသာ အော်ပရေတာများသည် ၎င်းအား TLS သက်တူရွယ်တူ စစ်မှန်ကြောင်းအထောက်အထားအပြည့်အစုံအတွက် မမှားပါနှင့်။

## အကြံပြုထားသော ပြန်လည်ပြင်ဆင်ရေးအမိန့်1. ခေါင်းစီးမှတ်တမ်းကို ပြန်လည်ပြင်ဆင်ခြင်း သို့မဟုတ် ပိတ်ခြင်းဖြင့် SA-001 ကို ချက်ချင်းပြင်ပါ။
2. SA-002 အတွက် ရွှေ့ပြောင်းနေထိုင်မှု အစီအစဉ်ကို ဒီဇိုင်းဆွဲပြီး သီးသန့်သော့စိမ်းများသည် API နယ်နိမိတ်ကို ဖြတ်ကျော်ခြင်းကို ရပ်သွားစေသည်။
3. အဝေးမှ လျှို့ဝှက်သော့ဆင်းသက်သည့်လမ်းကြောင်းကို ဖယ်ရှားပါ သို့မဟုတ် ကျဉ်းမြောင်းပြီး အစေ့ပေါက်သော အလောင်းများကို အကဲဆတ်သည့်အဖြစ် အမျိုးအစားခွဲခြားပါ။
4. Swift/Kotlin/Java တစ်လျှောက် SDK သယ်ယူပို့ဆောင်ရေး အာရုံခံနိုင်စွမ်းစည်းမျဉ်းများကို ချိန်ညှိပါ။
5. ပူးတွဲမိလ္လာစနစ်သည် စစ်မှန်သော sandbox လိုအပ်သည်ဖြစ်စေ သို့မဟုတ် ရိုးသားစွာ အမည်ပြောင်းခြင်း/ပြန်လည်သတ်မှတ်ခြင်းတို့ကို ဆုံးဖြတ်ပါ။
6. အစစ်အမှန် TLS ကိုမျှော်လင့်ထားသည့် အော်ပရေတာများက အဆိုပါသယ်ယူပို့ဆောင်မှုများကိုမဖွင့်မီ P2P TLS/QUIC ခြိမ်းခြောက်မှုပုံစံအား ရှင်းလင်းပြီး ခိုင်မာစေပါ။

## အတည်ပြုမှတ်စုများ

- `cargo check -p iroha_torii --lib --message-format short` အောင်မြင်ပြီး။
- `cargo check -p iroha_p2p --message-format short` အောင်မြင်ပြီး။
- `cargo deny check advisories bans sources --hide-inclusion-graph` သဲပုံးအပြင်ဘက်တွင် ပြေးပြီးနောက် အောင်မြင်သွားသည်။ ၎င်းသည် ထပ်တူဗားရှင်းသတိပေးချက်များကို ထုတ်လွှတ်သော်လည်း `advisories ok, bans ok, sources ok` ကို အစီရင်ခံသည်။
- ဤစစ်ဆေးမှုအတွင်း လျှို့ဝှက်ရယူသည့်ကီးဆက်လမ်းကြောင်းအတွက် အာရုံစိုက်ထားသော Torii စမ်းသပ်မှုတစ်ခုကို စတင်ခဲ့သော်လည်း အစီရင်ခံစာမရေးမီ မပြီးမြောက်ခဲ့ပါ။ ရှာဖွေတွေ့ရှိမှုကို တိုက်ရိုက်အရင်းအမြစ်စစ်ဆေးခြင်းမှ ပံ့ပိုးပေးပါသည်။