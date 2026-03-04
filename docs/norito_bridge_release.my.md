---
lang: my
direction: ltr
source: docs/norito_bridge_release.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9dc9862d4806d355fd83c885de92775712a7b32c68c010d29f4fc74229d054b
source_last_modified: "2026-01-06T05:24:53.995808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# NoritoBridge ထုပ်ပိုးမှု

ဤလမ်းညွှန်ချက်တွင် `NoritoBridge` Swift bindings များထုတ်ဝေရန် လိုအပ်သောအဆင့်များကို အကြမ်းဖျင်းဖော်ပြထားသည်
Swift Package Manager နှင့် CocoaPods တို့မှ စားသုံးနိုင်သော XCFramework တစ်ခု။ ဟိ
အလုပ်အသွားအလာသည် ထိုသင်္ဘောကို Rust သေတ္တာဖြင့် သော့ခတ်ထားသည့်အဆင့်တွင် Swift ရှေးဟောင်းပစ္စည်းများကို ထိန်းသိမ်းပေးသည်
Iroha ၏ Norito ကုဒ်ဒက်။ ထုတ်ဝေထားသော စားသုံးခြင်းဆိုင်ရာ အဆုံးမှ အဆုံး ညွှန်ကြားချက်များအတွက်
အက်ပ်အတွင်းရှိ ပစ္စည်းများ (Xcode ပရောဂျက် ဝိုင်ယာကြိုးများ၊ ChaChaPoly အသုံးပြုမှု စသည်) ကိုကြည့်ပါ။
`docs/connect_swift_integration.md`။

> **မှတ်ချက်-** ဤစီးဆင်းမှုအတွက် CI အလိုအလျောက်စနစ်သည် လိုအပ်သည်နှင့် macOS တည်ဆောက်သူများမှ ဆင်းသက်လာမည်ဖြစ်သည်။
> Apple ကိရိယာတန်ဆာပလာသည် အွန်လိုင်းတွင် ပေါ်လာသည် (ဖြန့်ချိရေးအင်ဂျင်နီယာချုပ် macOS တည်ဆောက်သူ၏နောက်ကွယ်တွင် ခြေရာခံထားသည်)။
> ထိုအချိန်အထိ အောက်ပါအဆင့်များကို develop Mac တွင် ကိုယ်တိုင်လုပ်ဆောင်ရပါမည်။

## လိုအပ်ချက်များ

- နောက်ဆုံးပေါ်တည်ငြိမ်သော Xcode အမိန့်ပေးစာလိုင်းကိရိယာများ ထည့်သွင်းထားသည့် macOS လက်ခံဆောင်ရွက်ပေးသူ။
- အလုပ်ခွင် `rust-toolchain.toml` နှင့် ကိုက်ညီသော သံချေးတက်တူးလ်ကြိုး။
- Swift toolchain 5.7 နှင့်အထက်။
- CocoaPods (ရူဘီကျောက်မျက်များမှတဆင့်) ဗဟိုသတ်မှတ်ချက်များသိုလှောင်ရာသို့ဖြန့်ချိပါက။
- Swift artifacts များကို တဂ်လုပ်ခြင်းအတွက် Hyperledger Iroha ထုတ်ပေးသည့် ဆိုင်းဘုတ်ခလုတ်များကို ဝင်ရောက်ကြည့်ရှုပါ။

## ဗားရှင်းပုံစံ

1. Norito codec (`crates/norito/Cargo.toml`) အတွက် Rust crate ဗားရှင်းကို သတ်မှတ်ပါ။
2. ထုတ်ဝေမှုအမှတ်အသားဖြင့် အလုပ်ခွင်ကို tag (ဥပမာ `v2.1.0`)။
3. Swift package နှင့် CocoaPods podspec အတွက် တူညီသော semantic ဗားရှင်းကို အသုံးပြုပါ။
4. Rust crate သည် ၎င်း၏ဗားရှင်းကို တိုးလာသောအခါ၊ လုပ်ငန်းစဉ်ကို ပြန်လုပ်ကာ ကိုက်ညီမှုတစ်ခုကို ထုတ်ဝေပါ။
   လျင်မြန်သော ရှေးဟောင်းပစ္စည်း။ ဗားရှင်းများတွင် စမ်းသပ်နေစဉ် မက်တာဒေတာ နောက်ဆက်တွဲများ (ဥပမာ `-alpha.1`) ပါဝင်နိုင်သည်။

## အဆင့်ဆင့်တည်ဆောက်ပါ။

1. repository root မှ XCFramework ကိုစုစည်းရန် helper script ကိုခေါ်ပါ-

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   script သည် iOS နှင့် macOS ပစ်မှတ်များအတွက် Rust Bridge စာကြည့်တိုက်ကို စုစည်းပြီး ၎င်းကို စုစည်းထားသည်။
   XCFramework လမ်းညွှန်တစ်ခုတည်းအောက်ရှိ static libraries များ။
   ၎င်းသည် `dist/NoritoBridge.artifacts.json` ကို ထုတ်လွှတ်ပြီး Bridge ဗားရှင်းကို ဖမ်းယူသည်။
   ပလပ်ဖောင်းတစ်ခုလျှင် SHA-256 hashes (ဗားရှင်းကို `NORITO_BRIDGE_VERSION` ဖြင့် အစားထိုးမည်ဆိုပါက၊
   လိုအပ်သည်)။

2. ဖြန့်ဝေရန်အတွက် XCFramework ကို ဇစ်ဖွင့်ပါ-

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. အသစ်ကိုညွှန်ပြရန် Swift ပက်ကေ့ဂျ်မန်နီးဖက်စ် (`IrohaSwift/Package.swift`) ကို အပ်ဒိတ်လုပ်ပါ။
   ဗားရှင်းနှင့် checksum-

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   binary ပစ်မှတ်ကို သတ်မှတ်သောအခါ `Package.swift` တွင် checksum ကို မှတ်တမ်းတင်ပါ။

4. `IrohaSwift/IrohaSwift.podspec` ကို ဗားရှင်းအသစ်၊ checksum နှင့် archive ဖြင့် အပ်ဒိတ်လုပ်ပါ။
   URL

5. ** တံတားသည် ပို့ကုန်အသစ်များရရှိပါက ခေါင်းစီးများကို ပြန်လည်ထုတ်ပေးပါ။** Swift တံတားကို ယခု ဖော်ထုတ်လိုက်ပြီဖြစ်သည်။
   `connect_norito_set_acceleration_config` ထို့ကြောင့် `AccelerationSettings` သည် Metal/
   GPU နောက်ခံများ။ `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h` သေချာပါစေ။
   ဇစ်မတင်မီ `crates/connect_norito_bridge/include/connect_norito_bridge.h` နှင့် ကိုက်ညီသည်။

6. တဂ်မတင်မီ Swift validation suite ကိုဖွင့်ပါ-

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   ပထမ command သည် Swift package (`AccelerationSettings` အပါအဝင်) ဆက်ရှိနေကြောင်းသေချာစေသည်
   အစိမ်းရောင်; ဒုတိယ သည် fixture parity ကို တရားဝင်စေပြီး parity/CI dashboards များကို ထုတ်ပေးသည်၊
   Buildkite တွင် ပြဋ္ဌာန်းထားသည့် တူညီသော တယ်လီမီတာ စစ်ဆေးမှုများကို လေ့ကျင့်ခန်းလုပ်သည် (ထိုအပါအဝငျ
   `ci/xcframework-smoke:<lane>:device_tag` မက်တာဒေတာ လိုအပ်ချက်)။

7. ထုတ်လုပ်ထားသော ပစ္စည်းများကို ထုတ်ဝေရေးဌာနခွဲတစ်ခုတွင် ထည့်သွင်းပြီး ကတိကဝတ်ကို tag လုပ်ပါ။

##ထုတ်ဝေခြင်း။

### Swift Package Manager

- tag ကို public Git repository သို့ တွန်းပါ။
- ပက်ကေ့ဂျ်အညွှန်း (Apple သို့မဟုတ် ကွန်မြူနတီကြေးမုံ) ဖြင့် တဂ်ကို ရရှိနိုင်ကြောင်း သေချာပါစေ။
- စားသုံးသူများသည် ယခု `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")` ကို မှီခိုနိုင်ပါပြီ။

### CocoaPods

1. pod ကို စက်တွင်းတွင် အတည်ပြုပါ-

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. အပ်ဒိတ်လုပ်ထားသော podspec ကို တွန်းပါ-

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. CocoaPods အညွှန်းတွင် ဗားရှင်းအသစ်ကို အတည်ပြုပါ။

## CI ထည့်သွင်းစဉ်းစားခြင်း။

- ထုပ်ပိုးမှုစခရစ်ကိုလုပ်ဆောင်ခြင်း၊ မော်ကွန်းတင်ခြင်း၊ နှင့် အပ်လုဒ်တင်ခြင်းများပြုလုပ်သည့် macOS အလုပ်တစ်ခုဖန်တီးပါ။
  အလုပ်အသွားအလာ ရလဒ်အဖြစ် checksum ကို ထုတ်ပေးသည်။
- Gate သည် အသစ်ထုတ်လုပ်ထားသော မူဘောင်များနှင့်ဆန့်ကျင်ဘက် Swift သရုပ်ပြအက်ပ်တည်ဆောက်မှုတွင် ဖြန့်ချိသည်။
- မအောင်မြင်မှုများကို အဖြေရှာရာတွင် အထောက်အကူပြုရန် တည်ဆောက်မှုမှတ်တမ်းများကို သိမ်းဆည်းပါ။

## နောက်ထပ် အလိုအလျောက်စနစ်ဆိုင်ရာ စိတ်ကူးများ

- လိုအပ်သောပစ်မှတ်အားလုံးကို ဖော်ထုတ်ပြီးသည်နှင့် `xcodebuild -create-xcframework` ကို တိုက်ရိုက်အသုံးပြုပါ။
- ဆော့ဖ်ဝဲအင်ဂျင်နီယာစက်များအပြင်သို့ ဖြန့်ဖြူးခြင်းအတွက် လက်မှတ်ရေးထိုးခြင်း/ အသိအမှတ်ပြုခြင်း ပေါင်းစပ်ပါ။
- SPM ကို တွဲထိုးခြင်းဖြင့် ထုပ်ပိုးထားသောဗားရှင်းဖြင့် လော့ခ်ချသည့်အဆင့်တွင် ပေါင်းစပ်စစ်ဆေးမှုများကို ထားရှိပါ။
  ထုတ်ဝေမှု tag အား မှီခိုမှု။