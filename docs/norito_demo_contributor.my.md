---
lang: my
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-12-29T18:16:35.099277+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#Norito SwiftUI Demo ပါဝင်ကူညီသူလမ်းညွှန်

ဤစာရွက်စာတမ်းသည် SwiftUI သရုပ်ပြလုပ်ဆောင်ရန် လိုအပ်သော လက်စွဲစနစ်ထည့်သွင်းမှုအဆင့်များကို ဖမ်းယူထားသည်။
local Torii node နှင့် mock ledger ၎င်းသည် `docs/norito_bridge_release.md` ဖြင့် ဖြည့်စွက်ထားသည်။
နေ့စဉ် ဖွံ့ဖြိုးတိုးတက်ရေး လုပ်ငန်းများကို အာရုံစိုက်ပါ။ ပေါင်းစည်းခြင်း၏ နက်နဲသော သင်ခန်းစာအတွက်
Norito တံတား/ Xcode ပရောဂျက်များသို့ stack stack ချိတ်ဆက်ပါ၊ `docs/connect_swift_integration.md` ကိုကြည့်ပါ။

## ပတ်ဝန်းကျင် တပ်ဆင်ခြင်း။

1. `rust-toolchain.toml` တွင် သတ်မှတ်ထားသော Rust toolchain ကို ထည့်သွင်းပါ။
2. macOS တွင် Swift 5.7+ နှင့် Xcode command line တူးလ်များကို ထည့်သွင်းပါ။
3. (ချန်လှပ်ထားနိုင်သည်) အလင်းရောင်အတွက် [SwiftLint](https://github.com/realm/SwiftLint) ကို ထည့်သွင်းပါ။
4. သင့် host ပေါ်ရှိ node များကို compile သေချာစေရန် `cargo build -p irohad` ကို run ပါ။
5. `examples/ios/NoritoDemoXcode/Configs/demo.env.example` ကို `.env` သို့ ကူးယူပြီး ချိန်ညှိပါ
   သင့်ပတ်ဝန်းကျင်နှင့်ကိုက်ညီသော တန်ဖိုးများ။ အပလီကေးရှင်းသည် စတင်ချိန်တွင် ဤကိန်းရှင်များကို ဖတ်သည်-
   - `TORII_NODE_URL` — အခြေခံ REST URL (WebSocket URL များသည် ၎င်းမှဆင်းသက်လာသည်)။
   - `CONNECT_SESSION_ID` — 32-byte စက်ရှင်အမှတ်အသား (base64/base64url)။
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — `/v2/connect/session` မှ ပြန်ပေးသော တိုကင်များ။
   - `CONNECT_CHAIN_ID` — ထိန်းချုပ်လက်ဆွဲစဉ်အတွင်း ကွင်းဆက်အမှတ်အသားကို ကြေညာခဲ့သည်။
   - `CONNECT_ROLE` — UI (`app` သို့မဟုတ် `wallet`) တွင် မူရင်းကဏ္ဍကို ကြိုတင်ရွေးချယ်ထားသည်။
   - လက်ဖြင့်စမ်းသပ်ခြင်းအတွက် ရွေးချယ်နိုင်သောအကူအညီများ- `CONNECT_PEER_PUB_B64`၊ `CONNECT_SHARED_KEY_B64`၊
     `CONNECT_APPROVE_ACCOUNT_ID`, `CONNECT_APPROVE_PRIVATE_KEY_B64`၊
     `CONNECT_APPROVE_SIGNATURE_B64`။

## Bootstrapping Torii + mock လယ်ဂျာ

repository သည် Torii node တစ်ခုကို စတင်သည့် helper scripts များကို in-memory ledger အကြို-
သရုပ်ပြအကောင့်များဖြင့် တင်ဆောင်သည်-

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

ဇာတ်ညွှန်းသည်-

- Torii node သည် `artifacts/torii.log` သို့ မှတ်တမ်းဝင်သည်။
- `artifacts/metrics.prom` သို့ လယ်ဂျာမက်ထရစ်များ (Prometheus ဖော်မတ်)။
- `artifacts/torii.jwt` သို့ ဖောက်သည်ဝင်ရောက်ခွင့် တိုကင်များ။

`start.sh` သည် သင် `Ctrl+C` ကို နှိပ်သည်အထိ သရုပ်ပြမျိုးတူကို ဆက်လက်လုပ်ဆောင်သည်။ အဲဒါက ready-state လို့ရေးတယ်။
`artifacts/ios_demo_state.json` (အခြားပစ္စည်းများအတွက် အမှန်တရား၏ရင်းမြစ်)၊
လက်ရှိအသုံးပြုနေသော Torii stdout မှတ်တမ်းကို မိတ္တူကူးပြီး၊ Prometheus ခြစ်သည်အထိ `/metrics` ၏ စစ်တမ်းများ
ရရှိနိုင်ပြီး ပြင်ဆင်ထားသောအကောင့်များကို `torii.jwt` (ကိုယ်ရေးကိုယ်တာသော့များအပါအဝင်၊
config ကသူတို့ကိုပေးတဲ့အခါ) ။ အထွက်ကို အစားထိုးရန် script သည် `--artifacts` ကို လက်ခံသည်။
စိတ်ကြိုက် Torii ဖွဲ့စည်းမှုပုံစံများနှင့် ကိုက်ညီရန် `--telemetry-profile`၊
အပြန်အလှန်အကျိုးသက်ရောက်မှုမရှိသော CI အလုပ်များအတွက် `--exit-after-ready`။

`SampleAccounts.json` တွင် ထည့်သွင်းမှုတစ်ခုစီသည် အောက်ပါအကွက်များကို ပံ့ပိုးပေးသည်-

- `name` (string, optional) — အကောင့် metadata `alias` အဖြစ် သိမ်းဆည်းထားသည်။
- `public_key` (multihash string၊ လိုအပ်သည်) — အကောင့်လက်မှတ်ထိုးသူအဖြစ် အသုံးပြုသည်။
- `private_key` (ချန်လှပ်ထားနိုင်သည်) — client credential generation အတွက် `torii.jwt` တွင် ပါဝင်သည်။
- `domain` (ချန်လှပ်ထားလျှင်) ပိုင်ဆိုင်မှုဒိုမိန်းသို့ ပုံသေသတ်မှတ်ထားသည်။
- `asset_id` (စာတန်း၊ လိုအပ်သည်) — အကောင့်အတွက် mint အတွက် ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက်။
- `initial_balance` (စာတန်း၊ လိုအပ်သည်) — အကောင့်ထဲသို့ ထည့်ထားသော ဂဏန်းပမာဏ။

## SwiftUI သရုပ်ပြကို လုပ်ဆောင်နေသည်။

1. `docs/norito_bridge_release.md` တွင်ဖော်ပြထားသည့်အတိုင်း XCFramework ကိုတည်ဆောက်ပြီး အစုအဝေးပြုလုပ်ပါ။
   ဒီမိုပရောဂျက်သို့ (ရည်ညွှန်းချက်များသည် ပရောဂျက်တွင် `NoritoBridge.xcframework` ကို မျှော်လင့်ထားသည်။
   root)။
2. Xcode တွင် `NoritoDemoXcode` ပရောဂျက်ကိုဖွင့်ပါ။
3. `NoritoDemo` အစီအစဉ်ကို ရွေးချယ်ပြီး iOS simulator သို့မဟုတ် စက်ပစ္စည်းကို ပစ်မှတ်ထားပါ။
4. `.env` ဖိုင်ကို အစီအစဉ်၏ ပတ်၀န်းကျင် ကိန်းရှင်များမှတစ်ဆင့် ကိုးကားကြောင်း သေချာပါစေ။
   `/v2/connect/session` မှ တင်ပို့သော `CONNECT_*` တန်ဖိုးများကို ဖြည့်သွင်းခြင်းဖြင့် UI သည်
   အက်ပ်စတင်သောအခါ ကြိုတင်ဖြည့်ပါ။
5. ဟာ့ဒ်ဝဲအရှိန်မြှင့်မှု ပုံသေများကို အတည်ပြုပါ- `App.swift` ခေါ်ဆိုမှုများ
   `DemoAccelerationConfig.load().apply()` ထို့ကြောင့် သရုပ်ပြသည်လည်းကောင်း ရွေးချယ်သည်။
   `NORITO_ACCEL_CONFIG_PATH` ပတ်ဝန်းကျင်ကို အစားထိုးခြင်း သို့မဟုတ် အစုအဝေးတစ်ခု
   `acceleration.{json,toml}`/`client.{json,toml}` ဖိုင်။ သင်ရှိပါက ဤထည့်သွင်းမှုများကို ဖယ်ရှား/ပြင်ဆင်ပါ။
   မ run မီ CPU အား တွန်းအားပေးလိုပါသည်။
6. အပလီကေးရှင်းကို တည်ဆောက်ပြီး စတင်လိုက်ပါ။ မဟုတ်ပါက ပင်မစခရင်သည် Torii URL/တိုကင်အတွက် အချက်ပြသည်။
   `.env` မှတစ်ဆင့် သတ်မှတ်ပြီးဖြစ်သည်။
7. အကောင့်အပ်ဒိတ်များစာရင်းသွင်းရန် သို့မဟုတ် တောင်းဆိုချက်များကို အတည်ပြုရန် "ချိတ်ဆက်ပါ" စက်ရှင်တစ်ခုကို စတင်ပါ။
8. IRH လွှဲပြောင်းမှုကို တင်သွင်းပြီး Torii မှတ်တမ်းများနှင့်အတူ မျက်နှာပြင်ပေါ်ရှိ မှတ်တမ်းအထွက်ကို စစ်ဆေးပါ။

### ဟာ့ဒ်ဝဲ အရှိန်မြှင့်စက်များ (သတ္တု / NEON)

`DemoAccelerationConfig` သည် Rust node configuration ကို ထင်ဟပ်နေသဖြင့် developer များ လေ့ကျင့်ခန်းလုပ်နိုင်သည်။
Hard-coding အဆင့်သတ်မှတ်ချက်များမပါဘဲ သတ္တု/NEON လမ်းကြောင်းများ။ loader သည် အောက်ပါတို့ကို ရှာဖွေသည်။
လွှင့်တင်မည့်နေရာများ-

1. `NORITO_ACCEL_CONFIG_PATH` (`.env`/scheme အကြောင်းပြချက်များတွင် သတ်မှတ်ထားသည်) — ပကတိလမ်းကြောင်း သို့မဟုတ်
   `tilde`-ညွှန်ပြချက်ကို `iroha_config` JSON/TOML ဖိုင်သို့ ချဲ့ထားသည်။
2. `acceleration.{json,toml}` သို့မဟုတ် `client.{json,toml}` ဟု အမည်ပေးထားသည့် စီစဉ်သတ်မှတ်ဖိုင်များ စုစည်းထားသည်။
3. အရင်းအမြစ်နှစ်ခုစလုံးကို မရရှိနိုင်ပါက၊ ပုံသေဆက်တင်များ (`AccelerationSettings()`) ကျန်ရှိနေပါသည်။

ဥပမာ `acceleration.toml` အတိုအထွာ-

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

အကွက်များ `nil` ကို ချန်ထားခြင်းဖြင့် အလုပ်ခွင် ပုံသေများကို အမွေဆက်ခံသည်။ အနှုတ်နံပါတ်များကို လျစ်လျူရှုခြင်း၊
ပျောက်ဆုံးနေသော `[accel]` အပိုင်းများသည် သတ်မှတ်ထားသော CPU အပြုအမူသို့ ပြန်ရောက်သွားပါသည်။ လည်ပတ်နေချိန်
တံတားသည် သတ္တုပံ့ပိုးမှုမပါပဲ Simulator သည် scalar လမ်းကြောင်းကို တိတ်တဆိတ် ထိန်းသိမ်းပေးသည်။
config တောင်းဆိုမှု Metal။

## ပေါင်းစပ်စစ်ဆေးမှုများ

- ပေါင်းစပ်စစ်ဆေးမှုများသည် `Tests/NoritoDemoTests` တွင်တည်ရှိသည် (macOS CI ပြီးသည်နှင့်ထည့်သွင်းရမည့်
  ရနိုင်သည်)။
- Tests များသည် Torii ကို လှည့်ပတ်ပြီး WebSocket စာရင်းသွင်းမှုများ၊ တိုကင်ကို လေ့ကျင့်ပါ။
  လက်ကျန်ငွေများနှင့် Swift ပက်ကေ့ခ်ျမှတဆင့် လွှဲပြောင်းစီးဆင်းမှုများ။
- စမ်းသပ်လည်ပတ်မှုများမှမှတ်တမ်းများကို `artifacts/tests/<timestamp>/` တွင် မက်ထရစ်များနှင့်အတူ သိမ်းဆည်းထားသည်။
  နမူနာ လယ်ဂျာအမှိုက်များ။

## CI parity စစ်ဆေးမှုများ

- ဒီမို သို့မဟုတ် မျှဝေထားသော ပွဲစဉ်များကို ထိသော PR ကိုမပို့မီ `make swift-ci` ကိုဖွင့်ပါ။ ဟိ
  ပစ်မှတ်သည် fixture parity checks များကိုလုပ်ဆောင်သည်၊ dashboard feeds များကိုအတည်ပြုပေးပြီး၊
  ဒေသအလိုက် အကျဉ်းချုပ်များ။ CI တွင်တူညီသောအလုပ်အသွားအလာသည် Buildkite မက်တာဒေတာပေါ်တွင်မူတည်သည်။
  (`ci/xcframework-smoke:<lane>:device_tag`) ထို့ကြောင့် ဒက်ရှ်ဘုတ်များသည် ရလဒ်များကို သတ်မှတ်ပေးနိုင်သည်။
  မှန်ကန်သော Simulator သို့မဟုတ် StrongBox လမ်းသွား—သင်သည် ၎င်းကို ချိန်ညှိပါက မက်တာဒေတာ ရှိနေကြောင်း အတည်ပြုပါ။
  ပိုက်လိုင်း သို့မဟုတ် အေးဂျင့် တဂ်များ။
- `make swift-ci` မအောင်မြင်သောအခါ၊ `docs/source/swift_parity_triage.md` ရှိ အဆင့်များကို လိုက်နာပါ။
  မည်သည့်လမ်းကြော လိုအပ်သည်ကို ဆုံးဖြတ်ရန် ပြန်ဆိုထားသော `mobile_ci` ကို ပြန်လည်သုံးသပ်ပါ။
  ပြန်လည်ရှင်သန်ခြင်း သို့မဟုတ် အဖြစ်အပျက်နောက်ဆက်တွဲ။

## ပြဿနာဖြေရှင်းခြင်း။

- သရုပ်ပြသည် Torii သို့ မချိတ်ဆက်နိုင်ပါက node URL နှင့် TLS ဆက်တင်များကို စစ်ဆေးပါ။
- JWT တိုကင် (လိုအပ်ပါက) သည် တရားဝင်ပြီး သက်တမ်းမကုန်ကြောင်း သေချာပါစေ။
- server-side အမှားများအတွက် `artifacts/torii.log` ကိုစစ်ဆေးပါ။
- WebSocket ပြဿနာများအတွက်၊ client log window သို့မဟုတ် Xcode console output ကိုစစ်ဆေးပါ။