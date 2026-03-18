---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#SoraFS Orchestrator GA Parity အစီရင်ခံစာ

Deterministic multi-fetch parity ကို SDK အလိုက် ခြေရာခံလိုက်ပါပြီ၊ ထို့ကြောင့် ထုတ်ပြန်လိုက်သော အင်ဂျင်နီယာများက ၎င်းကို အတည်ပြုနိုင်ပါသည်။
payload bytes၊ အတုံးလိုက်ဖြတ်ပိုင်းများ၊ ဝန်ဆောင်မှုပေးသည့်အစီရင်ခံစာများနှင့် ရမှတ်ဘုတ်ရလဒ်များသည် တစ်ဖက်တွင် ညှိနေဆဲဖြစ်သည်။
အကောင်အထည်ဖော်မှုများ။ ကြိုးတစ်ခုစီသည် အောက်တွင် canonical multi-provider အတွဲကို စားသုံးသည်။
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`၊ SF1 အစီအစဉ်၊ ပံ့ပိုးသူ
မက်တာဒေတာ၊ တယ်လီမီတာ လျှပ်တစ်ပြက်ရိုက်ချက်နှင့် တီးမှုတ်သူ ရွေးချယ်စရာများ။

## Rust Baseline

- **Command-** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Scope-** `MultiPeerFixture` အစီအစဉ်ကို လုပ်ငန်းစဉ်အတွင်း သံစုံတီးဝိုင်းမှတစ်ဆင့် နှစ်ကြိမ် လုပ်ဆောင်ပြီး အတည်ပြုခြင်း၊
  စုစည်းထားသော payload bytes၊ အတုံးလိုက်ပြေစာများ၊ ပံ့ပိုးပေးသူအစီရင်ခံစာများနှင့် ရမှတ်ဘုတ်ရလဒ်များ။ တူရိယာ
  အမြင့်ဆုံး တွဲဖက်ငွေကြေးနှင့် ထိရောက်သော အလုပ်တွဲအရွယ်အစား (`max_parallel × max_chunk_length`)ကိုလည်း ခြေရာခံပါသည်။
- **Performance guard:** လည်ပတ်မှုတစ်ခုစီသည် CI ဟာ့ဒ်ဝဲတွင် 2s အတွင်း ပြီးရပါမည်။
- **လုပ်ငန်းခွင်သတ်မှတ်မျက်နှာကျက်-** SF1 ပရိုဖိုင်နှင့်အတူ ကြိုးသည် `max_parallel = 3` ကို ခိုင်ခံ့စေပြီး၊
  ≤196608byte ဝင်းဒိုး။

နမူနာ မှတ်တမ်း အထွက်-

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK ကြိုးစည်း

- **Command-** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Scope:** တူညီသော fixture ကို `iroha_js_host::sorafsMultiFetchLocal` မှတဆင့် payloads နှိုင်းယှဉ်ခြင်း၊
  ဖြတ်ပိုင်းများ၊ ဝန်ဆောင်မှုပေးသူ အစီရင်ခံစာများနှင့် ဆက်တိုက်ပြေးနေသည့် အမှတ်စာရင်းများ။
- **Performance Guard:** လုပ်ဆောင်ချက်တစ်ခုစီသည် 2s အတွင်း ပြီးဆုံးရပါမည်။ ကြိုးသည် တိုင်းတာပြီး ပုံနှိပ်သည်။
  ကြာချိန်နှင့် reserved-byte မျက်နှာကျက် (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`)။

ဥပမာ အကျဉ်းချုပ် စာကြောင်း

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK Harness

- **Command-** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Scope-** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` တွင် သတ်မှတ်ထားသော parity suite ကို run သည် ။
  Norito တံတား (`sorafsLocalFetch`) မှတဆင့် SF1 ပွဲစဉ်များကို နှစ်ကြိမ်ပြန်ဖွင့်ခြင်း။ သံကြိုးက စစ်ဆေးပါတယ်။
  တူညီသော အဆုံးအဖြတ်ကို အသုံးပြု၍ payload bytes၊ အတုံးလိုက်ဖြတ်ပိုင်းများ၊ ဝန်ဆောင်မှုပေးသူ အစီရင်ခံစာများနှင့် အမှတ်စာရင်းများ ထည့်သွင်းမှုများ
  ဝန်ဆောင်မှုပေးသူ မက်တာဒေတာနှင့် တယ်လီမီတာ လျှပ်တစ်ပြက်ပုံများသည် Rust/JS အတွဲများအဖြစ်။
- **Bridge bootstrap:** ကြိုးသိုင်းသည် `dist/NoritoBridge.xcframework.zip` ကို ဝယ်လိုအားနှင့် ဝန်ထုပ်ဝန်ပိုးဖြစ်စေပါသည်။
  macOS အချပ် `dlopen` မှတဆင့်။ xcframework ပျောက်နေသည် သို့မဟုတ် SoraFS bindings များမရှိသောအခါ၊
  `cargo build -p connect_norito_bridge --release` သို့ ပြန်ကျသွားပြီး ဆန့်ကျင်ဘက်လင့်များ
  `target/release/libconnect_norito_bridge.dylib`၊ ထို့ကြောင့် CI တွင် လက်စွဲတပ်ဆင်မှုမလိုအပ်ပါ။
- **Performance Guard:** စီစစ်မှုတစ်ခုစီသည် CI ဟာ့ဒ်ဝဲပေါ်တွင် 2s အတွင်း အပြီးသတ်ရပါမည်။ ကြိုးသည် ပုံနှိပ်သည်။
  တိုင်းတာထားသော ကြာချိန်နှင့် reserved-byte မျက်နှာကျက် (`max_parallel = 3`, `peak_reserved_bytes ≤ 196 608`)။

ဥပမာ အကျဉ်းချုပ် စာကြောင်း

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python Bindings Harness

- **Command-** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Scope-** အဆင့်မြင့် `iroha_python.sorafs.multi_fetch_local` wrapper နှင့် ၎င်း၏ရိုက်ထည့်ခြင်းကို လေ့ကျင့်သည်။
  dataclasses ဖြစ်သောကြောင့် canonical fixture သည် wheel users ခေါ်သော တူညီသော API မှတဆင့် စီးဆင်းပါသည်။ စမ်းသပ်မှု
  `providers.json` မှ ပံ့ပိုးပေးသူ မက်တာဒေတာကို ပြန်လည်တည်ဆောက်ပြီး တယ်လီမီတာရိုက်ချက်အား ထိုးသွင်းကာ စစ်ဆေးအတည်ပြုသည်
  payload bytes၊ အတုံးလိုက်ဖြတ်ပိုင်းများ၊ ဝန်ဆောင်မှုပေးသည့်အစီရင်ခံစာများနှင့် Rust/JS/Swift တို့ကဲ့သို့ အမှတ်စာရင်းများ
  အစုံ။
- **Pre-req:** `maturin develop --release` ကို Run ပါ (သို့မဟုတ် ဘီးကို တပ်ဆင်ပါ) ထို့ကြောင့် `_crypto` ကို ဖော်ထုတ်ပေးပါသည်။
  pytest မခေါ်မီ `sorafs_multi_fetch_local` binding၊ ချည်နှောင်သည့်အခါ ကြိုးသည် အလိုအလျောက် ကျော်သွားသည်။
  မရနိုင်ပါ။
- **Performance guard:** Rust suite နှင့် အတူတူ ≤2s ဘတ်ဂျက်၊ pytest သည် စုစည်းထားသော byte အရေအတွက်ကို မှတ်တမ်းတင်သည်။
  ထုတ်ဝေခြင်းအတွက် ပံ့ပိုးပေးသူပါဝင်မှု အနှစ်ချုပ်နှင့်

Release Gating သည် ကြိုးတိုင်း (Rust, Python, JS, Swift) မှ အကျဉ်းချုပ် output ကို ဖမ်းယူသင့်သည်။
မော်ကွန်းတင်ထားသော အစီရင်ခံစာသည် တည်ဆောက်မှုတစ်ခုအား မမြှင့်တင်မီ ပေးဆောင်မှုလက်ခံဖြတ်ပိုင်းများနှင့် မက်ထရစ်များကို တူညီစွာကွဲပြားစေနိုင်သည်။ ပြေး
`ci/sdk_sorafs_orchestrator.sh` တွင် parity suite (Rust၊ Python bindings, JS, Swift) အားလုံးကို လုပ်ဆောင်ရန်
တဦးတည်းဖြတ်သန်း; CI ရှေးဟောင်းပစ္စည်းများသည် ထိုအကူအညီပေးသူထံမှ မှတ်တမ်းကောက်နုတ်ချက်နှင့် ထုတ်ပေးထားသည့် မှတ်တမ်းကို ပူးတွဲဖော်ပြသင့်သည်။
ထုတ်ဝေခွင့်လက်မှတ်သို့ `matrix.md` (SDK/အခြေအနေ/ကြာချိန်ဇယား) သည် ပြန်လည်သုံးသပ်သူများသည် တန်းတူညီမျှမှုကို စစ်ဆေးနိုင်စေရန်
စက်တွင်းအစုံကို ပြန်မဖွင့်ဘဲ matrix။