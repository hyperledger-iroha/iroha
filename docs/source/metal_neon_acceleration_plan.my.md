---
lang: my
direction: ltr
source: docs/source/metal_neon_acceleration_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 628eb2c7776bf818a310dd4bae51e3fc655f92e885d0cd9da7ff487fd9128102
source_last_modified: "2025-12-29T18:16:35.976997+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# သတ္တုနှင့် NEON အရှိန်မြှင့်အစီအစဉ် (Swift & Rust)

ဤစာရွက်စာတမ်းသည် အဆုံးအဖြတ်ပေးသော ဟာ့ဒ်ဝဲကို ဖွင့်ရန်အတွက် မျှဝေထားသော အစီအစဉ်ကို ဖမ်းယူပါသည်။
အရှိန်မြှင့်ခြင်း (Metal GPU + NEON / Accelerate SIMD + StrongBox ပေါင်းစပ်မှု)
Rust အလုပ်ခွင်နှင့် Swift SDK။ ၎င်းသည် ခြေရာခံထားသော လမ်းပြမြေပုံကို ဖြေရှင်းပေးသည်။
**Hardware Acceleration Workstream (macOS/iOS)** အောက်တွင် နှင့် လက်-ပိတ်ကို ပံ့ပိုးပေးသည်။
Rust IVM အဖွဲ့၊ Swift တံတားပိုင်ရှင်များနှင့် တယ်လီမီတာကိရိယာများအတွက် ရှေးဟောင်းပစ္စည်း။

> နောက်ဆုံးမွမ်းမံမှု- 2026-01-12  
> ပိုင်ရှင်များ- IVM စွမ်းဆောင်ရည် TL၊ Swift SDK ဦးဆောင်

## ပန်းတိုင်

1. သတ္တုဖြင့် Apple ဟာ့ဒ်ဝဲတွင် Rust GPU kernels (Poseidon/BN254/CRC64) ကို ပြန်လည်အသုံးပြုပါ
   CPU လမ်းကြောင်းများကို အဆုံးအဖြတ်ပေးသော တန်းတူညီမျှမှုဖြင့် အရိပ်အာနိသင်များကို တွက်ချက်ပါ။
2. အရှိန်မြှင့်ခလုတ်များ (`AccelerationConfig`) ကို အဆုံးမှ အဆုံးအထိ Swift အက်ပ်များကို ထုတ်ပါ
   ABI/parity အာမခံချက်များကို ထိန်းသိမ်းထားစဉ် Metal/NEON/StrongBox တွင် ရွေးချယ်နိုင်ပါသည်။
3. တန်းတူညီမျှမှု/စံနှုန်းဒေတာနှင့် အလံကို ပေါ်အောင်ပြုလုပ်ရန်အတွက် CI + ဒက်ရှ်ဘုတ်များ
   CPU နှင့် GPU/ SIMD လမ်းကြောင်းများတစ်လျှောက် ဆုတ်ယုတ်မှုများ။
4. Android (AND2) နှင့် Swift အကြား StrongBox/secure-enclave သင်ခန်းစာများကို မျှဝေပါ။
   လက်မှတ်ထိုးစီးဆင်းမှုများကို အဆုံးအဖြတ်ကျကျ ချိန်ညှိထားရန် (IOS4)။

**အပ်ဒိတ် (CRC64 + Stage‑1 ပြန်လည်ဆန်းသစ်ခြင်း):** CRC64 GPU အထောက်အကူများကို ယခု `norito::core::hardware_crc64` တွင် 192KiB ပုံသေဖြတ်တောက်မှုဖြင့် (`NORITO_GPU_CRC64_MIN_BYTES` မှတဆင့် ထပ်ရေးပါ သို့မဟုတ် ပြတ်သားစွာ အထောက်အကူပေးသည့် လမ်းကြောင်း `NORITO_CRC64_GPU_LIB`) နှင့် ဆက်လက်ထိန်းသိမ်းထားစဉ်။ JSON Stage-1 ဖြတ်တောက်မှုများကို ပြန်လည်စံသတ်မှတ်ထားပြီး (`examples/stage1_cutover` → `benchmarks/norito_stage1/cutover.csv`)၊ စကေးဖြတ်တောက်မှုကို 4KiB တွင်ထားရှိကာ Stage-1 GPU မူရင်းကို 192KiB (`NORITO_STAGE1_GPU_MIN_BYTES`) တွင် ကြီးမားသော CPU နှင့် GPU များကို လွှင့်တင်ရာတွင် ကုန်ကျစရိတ်များစွာရှိနေစေပါသည်။

## ပစ္စည်းနှင့် ပိုင်ရှင်များ

| မှတ်တိုင် | ပေးပို့နိုင်သော | ပိုင်ရှင်(များ) | ပစ်မှတ် |
|----------|-------------|----------------|--------|
| သံချေးတက် WP2-A/B | Metal Shader Interfaces သည် CUDA kernels | IVM Perf TL | ဖေဖော်ဝါရီ 2026 |
| သံချေး WP2-C | သတ္တု BN254 parity စမ်းသပ်မှုများ & CI လမ်းကြော | IVM Perf TL | Q2 2026 |
| Swift IOS6 | ကြိုးတပ်ထားသော တံတားခလုတ်များ (`connect_norito_set_acceleration_config`) + SDK API + နမူနာများ | Swift Bridge ပိုင်ရှင်များ | ပြီးပြီ (ဇန်နဝါရီ 2026) |
| Swift IOS5 | config အသုံးပြုမှုကို သရုပ်ပြသည့် နမူနာအက်ပ်/စာရွက်စာတမ်းများ | Swift DX TL | Q2 2026 |
| Telemetry | ဒက်ရှ်ဘုတ်သည် အရှိန်အဟုန် ညီမျှခြင်း + စံနှုန်း မက်ထရစ်များ | Swift အစီအစဉ် PM / Telemetry | စမ်းသပ်ဒေတာ Q2 2026 |
| CI | XCFramework မီးခိုးကြိုးကို ကိရိယာရေကန်တွင် CPU နှင့် သတ္တု/NEON လေ့ကျင့်ခန်း | Swift QA ဦးဆောင် | Q2 2026 |
| StrongBox | ဟာ့ဒ်ဝဲ-ကျောထောက်နောက်ခံပြု လက်မှတ်ရေးထိုးခြင်း တူညီမှုစမ်းသပ်မှုများ (မျှဝေထားသော vectors) | Android Crypto TL / Swift လုံခြုံရေး | Q3 2026 |

## အင်တာဖေ့စ်များနှင့် API စာချုပ်များ### သံချေး (`ivm::AccelerationConfig`)
- ရှိပြီးသားအကွက်များ (`enable_simd`၊ `enable_metal`၊ `enable_cuda`၊ `max_gpus`၊ သတ်မှတ်ချက်များ)။
- ပထမဆုံးအသုံးပြုရန် ကြာမြင့်ချိန် (Rust #15875) ကိုရှောင်ရှားရန် တိကျရှင်းလင်းသော သတ္တုပူနွေးမှုကို ထည့်ပါ။
- ဒက်ရှ်ဘုတ်များအတွက် parity API များကို ပံ့ပိုးပေးသည်-
  - e.g. `ivm::vector::metal_status()` -> {enabled, parity, last_error}။
- စံနှုန်းသတ်မှတ်ခြင်းမက်ထရစ်များ ( Merkle သစ်ပင်အချိန်များ ၊ CRC ဖြတ်သန်းမှု ) ကို ထုတ်ပေးခြင်း။
  `ci/xcode-swift-parity` အတွက် တယ်လီမီတာချိတ်များ။
- ယခုအခါ သတ္တုအိမ်ရှင်သည် စုစည်းထားသော `fastpq.metallib` ကို တင်ပြီး FFT/IFFT/LDE ကို ပေးပို့သည်။
  နှင့် Poseidon kernels တို့သည် အခါတိုင်းတွင် CPU အကောင်အထည်ဖော်မှုဆီသို့ ပြန်ရောက်သွားသည်။
  metallib သို့မဟုတ် စက်ပစ္စည်းတန်းစီခြင်းကို မရရှိနိုင်ပါ။

### C FFI (`connect_norito_bridge`)
- အသစ်ဖွဲ့စည်းပုံ `connect_norito_acceleration_config` (ပြီးပါပြီ)။
- Getter လွှမ်းခြုံမှုသည် ယခု setter ကို mirror ရန် `connect_norito_get_acceleration_config` (config only) နှင့် `connect_norito_get_acceleration_state` (config + parity) တို့ ပါဝင်သည်။
- SPM/CocoaPods သုံးစွဲသူများအတွက် ခေါင်းစီးမှတ်ချက်များတွင် စာရွက်စာတမ်းဖွဲ့စည်းပုံပုံစံ။

### Swift (`AccelerationSettings`)
- ပုံသေများ- သတ္တုကိုဖွင့်ထားပြီး၊ CUDA ပိတ်ထားသည်၊ ကန့်သတ်ချက်များ (အမွေဆက်ခံမှု) မရှိပါ။
- အနုတ်လက္ခဏာတန်ဖိုးများကို လျစ်လျူရှုထားသည်။ `apply()` ကို `IrohaSDK` မှ အလိုအလျောက် ခေါ်ဆိုပါသည်။
- ယခု `AccelerationSettings.runtimeState()` သည် `connect_norito_get_acceleration_state` ကို ပေါ်နေသည်
  payload (config + Metal/CUDA parity status) ထို့ကြောင့် Swift dashboards များသည် တူညီသော telemetry ကို ထုတ်လွှတ်သည်
  Rust (`supported/configured/available/parity`) အဖြစ်။ အကူအညီပေးသူက `nil` ကို ပြန်ပေးသည်။
  စစ်ဆေးမှုများကို သယ်ဆောင်ရလွယ်ကူစေရန် တံတားမရှိတော့ပါ။
- `AccelerationBackendStatus.lastError` ကို disable/error အကြောင်းပြချက်မှ ကူးယူသည်။
  `connect_norito_get_acceleration_state` သည် string ဖြစ်သည်နှင့်တပြိုင်နက် မူရင်းကြားခံကို လွှတ်ပေးသည်။
  ရုပ်လုံးပေါ်လာသောကြောင့် မိုဘိုင်းတန်းတူ ဒက်ရှ်ဘုတ်များသည် Metal/CUDA တွင် အဘယ်ကြောင့် ပိတ်ထားသည်ကို မှတ်သားနိုင်ပါသည်။
  အိမ်ရှင်တိုင်း။
- `AccelerationSettingsLoader` (`IrohaSwift/Sources/IrohaSwift/AccelerationSettingsLoader.swift`၊
  ယခု `IrohaSwift/Tests/IrohaSwiftTests/AccelerationSettingsLoaderTests.swift`) အောက်တွင် စမ်းသပ်မှုများ
  Norito သရုပ်ပြကဲ့သို့ တူညီသော ဦးစားပေး အစီအစဉ်ဖြင့် အော်ပရေတာမှ ထင်ရှားပေါ်လွင်မှုကို ဖြေရှင်းသည်- ဂုဏ်
  `NORITO_ACCEL_CONFIG_PATH`၊ ရှာဖွေမှုအစုအဝေး `acceleration.{json,toml}` / `client.{json,toml}`၊
  ရွေးချယ်ထားသော ရင်းမြစ်ကို မှတ်တမ်းယူပြီး မူရင်းအတိုင်း ပြန်သွားပါ။ အပလီကေးရှင်းများအတွက် စိတ်ကြိုက် loaders များ မလိုအပ်တော့ပါ။
  Rust `iroha_config` မျက်နှာပြင်ကို မှန်ကြည့်သည်။
- ခလုတ်များနှင့် တယ်လီမီတာ ပေါင်းစပ်မှုကိုပြသရန် နမူနာအက်ပ်များနှင့် README ကို အပ်ဒိတ်လုပ်ပါ။

### Telemetry (Dashboards + Exporters)
- Parity feed (mobile_parity.json):
  - `acceleration.metal/neon/strongbox` -> {enabled, parity, perf_delta_pct}။
  - `perf_delta_pct` အခြေခံ CPU နှင့် GPU နှိုင်းယှဉ်မှုကို လက်ခံပါ။
  - `acceleration.metal.disable_reason` မှန်များ `AccelerationBackendStatus.lastError`
    ထို့ကြောင့် Swift အလိုအလျောက်စနစ်သည် Rust ကဲ့သို့ တူညီသောသစ္စာရှိမှုဖြင့် disabled GPU များကို အလံပြနိုင်သည်။
    ဒက်ရှ်ဘုတ်များ
- CI ဖိဒ် (mobile_ci.json):
  - `acceleration_bench.metal_vs_cpu_merkle_ms` -> {cpu၊ metal}
  - `acceleration_bench.neon_crc64_throughput_mb_s` --> နှစ်ချက်။
- တင်ပို့သူများသည် Rust စံညွှန်းများ သို့မဟုတ် CI လည်ပတ်မှုများမှ ရင်းမြစ်မက်ထရစ်များ ရှိရပါမည် (ဥပမာ၊ run
  `ci/xcode-swift-parity` ၏ အစိတ်အပိုင်းအဖြစ် သတ္တု/CPU မိုက်ခရိုခုံး။### ဖွဲ့စည်းမှုခလုတ်များနှင့် ပုံသေများ (WP6-C)
- `AccelerationConfig` ပုံသေများ- macOS တည်ဆောက်မှုများတွင် `enable_metal = true`၊ CUDA အင်္ဂါရပ်ကို ပြုစုသောအခါ `enable_cuda = true`၊ `max_gpus = None` (ထုပ်မပါ)။ Swift `AccelerationSettings` ထုပ်ပိုးမှုသည် `connect_norito_set_acceleration_config` မှတဆင့် အလားတူမူရင်းများကို အမွေဆက်ခံပါသည်။
- Norito Merkle heuristics (GPU နှင့် CPU): `merkle_min_leaves_gpu = 8192` ≥8192 အရွက်ပါသော သစ်ပင်များအတွက် GPU hashing ကို ဖွင့်ပေးသည် ။ backend overrides (`merkle_min_leaves_metal`, `merkle_min_leaves_cuda`) သည် အတိအလင်းသတ်မှတ်ထားခြင်းမရှိပါက တူညီသောအဆင့်သို့ ပုံသေဖြစ်သည်။
- CPU preference heuristics (SHA2 ISA present)- AArch64 (ARMv8 SHA2) နှင့် x86/x86_64 (SHA-NI) နှစ်ခုလုံးတွင် CPU လမ်းကြောင်းသည် `prefer_cpu_sha2_max_leaves_* = 32_768` အရွက်အထိ ဦးစားပေးနေဆဲဖြစ်သည်။ GPU သတ်မှတ်ချက်ထက် အကျုံးဝင်သည်။ ဤတန်ဖိုးများကို `AccelerationConfig` မှတစ်ဆင့် ပြင်ဆင်သတ်မှတ်နိုင်ပြီး စံနှုန်းအထောက်အထားများဖြင့်သာ ချိန်ညှိသင့်သည်။

## စမ်းသပ်ခြင်းဗျူဟာ

1. **ယူနစ် တူညီမှု စမ်းသပ်မှုများ (Rust)**- သတ္တု စေ့များသည် CPU အထွက်များနှင့် ကိုက်ညီကြောင်း သေချာပါစေ။
   အဆုံးအဖြတ်ပေးသော vector များ; `cargo test -p ivm --features metal` အောက်တွင် run ပါ။
   ယခု `crates/fastpq_prover/src/metal.rs` သည် macOS သီးသန့်စမ်းသပ်မှုများကို ပေးပို့သည်
   FFT/IFFT/LDE နှင့် Poseidon တို့ကို စကလာအကိုးအကားနှင့် ဆန့်ကျင်သည့် လေ့ကျင့်ခန်း။
2. **Swift smoke harness**- CPU နှင့် Metal ကို execute လုပ်ရန် IOS6 test runner ကို တိုးချဲ့ပါ
   emulator များနှင့် StrongBox စက်နှစ်ခုလုံးတွင် ကုဒ်သွင်းခြင်း (Merkle/CRC64)၊ နှိုင်းယှဉ်
   ရလဒ်များနှင့် မှတ်တမ်း တူညီမှု အခြေအနေ။
3. **CI**- အပ်ဒိတ်လုပ်ရန် `norito_bridge_ios.yml` (`make swift-ci` ကိုခေါ်ဆိုထားပြီး)
   ပစ္စည်းများဆီသို့ အရှိန်မြှင့်တိုင်းတာမှုများ၊ run သည် Buildkite ကိုအတည်ပြုကြောင်းသေချာပါစေ။
   `ci/xcframework-smoke:<lane>:device_tag` စုစည်းမှု အပြောင်းအလဲများကို မထုတ်ဝေမီ မက်တာဒေတာ၊
   ညီမျှခြင်း/စံနှုန်း ပျံ့လွင့်မှုတွင် လမ်းကြောကို ပျက်ကွက်စေသည်။
4. **Dashboards**- အကွက်အသစ်များသည် CLI အထွက်တွင် ယခုတင်ပြသည်။ ပို့ကုန်လုပ်ငန်းရှင်များ သေချာစွာ ထုတ်လုပ်သည်။
   ဒက်ရှ်ဘုတ်များကို တိုက်ရိုက်လွှင့်မလှန်မီ ဒေတာ။

## WP2-A Metal Shader Plan (Poseidon Pipelines)

ပထမဆုံး WP2 မှတ်တိုင်သည် Poseidon Metal kernels အတွက် အစီအစဉ်ဆွဲခြင်းလုပ်ငန်းကို အကျုံးဝင်သည်။
၎င်းသည် CUDA အကောင်အထည်ဖော်မှုကို ထင်ဟပ်စေသည်။ အစီအစဥ်သည် အားထုတ်မှုကို စေ့စေ့များအဖြစ် ပိုင်းခြားပြီး၊
အစီအစဉ်ဆွဲခြင်း နှင့် မျှဝေထားသော စဉ်ဆက်မပြတ် ဇာတ်ကြောင်းကြောင့် နောက်ပိုင်းတွင် အလုပ်အပေါ် အာရုံစူးစိုက်နိုင်မည်ဖြစ်သည်။
အကောင်အထည်ဖော်ခြင်းနှင့်စမ်းသပ်ခြင်း။

### Kernel နယ်ပယ်

1. `poseidon_permute`- `state_count` သည် သီးခြားပြည်နယ်များကို ခွင့်ပြုသည်။ ချည်တိုင်း၊
   `STATE_CHUNK` (ပြည်နယ် 4 ခု) ကို ပိုင်ဆိုင်ပြီး `TOTAL_ROUNDS` အားလုံးကို အသုံးပြု၍ ထပ်ခါထပ်ခါ လုပ်ဆောင်သည်
   threadgroup-shared round constants များကို ပေးပို့ချိန်၌ လုပ်ဆောင်သည်။
2. `poseidon_hash_columns`- ကျဲကျဲ `PoseidonColumnSlice` ကက်တလောက်ကို ဖတ်သည်နှင့်
   ကော်လံတိုင်း၏ Merkle-friendly hashing ကို လုပ်ဆောင်သည် (CPU နှင့် ကိုက်ညီသည်။
   `PoseidonColumnBatch` အပြင်အဆင်)။ ၎င်းသည် တူညီသော threadgroup constant buffer ကိုအသုံးပြုသည်။
   permute kernel အနေဖြင့် `(states_per_lane * block_count)` ကို လှည့်ပတ်သည်။
   kernel သည် တန်းစီတင်ပြမှုများကို အလွတ်မပေးစေရန် အထွက်များသည်။
3. `poseidon_trace_fused`- ခြေရာခံဇယားအတွက် parent/leaf digest များကိုတွက်ချက်သည်
   pass တစ်ခုတည်း၌။ ပေါင်းစပ်ထားသော kernel သည် `PoseidonFusedArgs` ဖြစ်သောကြောင့် host ကိုစားသုံးသည်။
   ဆက်စပ်မှုမရှိသော ဒေသများနှင့် `leaf_offset`/`parent_offset` ကို ဖော်ပြနိုင်ပြီး၊
   ၎င်းသည် round/MDS ဇယားအားလုံးကို အခြား kernels များနှင့် မျှဝေသည်။

### Command Scheduling & Host Contracts- kernel ပေးပို့မှုတိုင်းသည် `MetalPipelines::command_queue` မှတဆင့် လုပ်ဆောင်သည်။
  လိုက်လျောညီထွေရှိသော အချိန်ဇယားကို တွန်းအားပေးသည် (ပစ်မှတ် ~2 ms) နှင့် တန်းစီနေသော ပန်ကာ-အထွက် ထိန်းချုပ်မှုများ
  `FASTPQ_METAL_QUEUE_FANOUT` နှင့် ထိတွေ့ခဲ့သည်။
  `FASTPQ_METAL_COLUMN_THRESHOLD`။ သွေးပူလမ်းကြောင်း `with_metal_state`
  Poseidon kernel သုံးခုလုံးကို ရှေ့ဆုံးတွင် စုစည်းထားသောကြောင့် ပထမအကြိမ် ပေးပို့ခြင်းမပြုပါ။
  ပိုက်လိုင်းဖန်တီးမှုဒဏ်ငွေ ပေးဆောင်ပါ။
- Threadgroup အရွယ်အစားသည် လက်ရှိ Metal FFT/LDE ပုံသေများကို ထင်ဟပ်စေသည်- ပစ်မှတ်မှာ ဖြစ်သည်။
  အုပ်စုတစ်ခုလျှင် 256 threads ပါ၀င်သော hard cap ဖြင့် တင်ပြမှုတစ်ခုလျှင် 8,192 ခု။ ဟိ
  host သည် ပါဝါနည်းသော စက်များအတွက် `states_per_lane` မြှောက်ခြင်းကို လျှော့ချနိုင်သည်
  ပတ်၀န်းကျင်ကို ခေါ်ဆိုခြင်းအား ကျော်သည် (`FASTPQ_METAL_POSEIDON_STATES_PER_BATCH`
  Shader logic ကိုမွမ်းမံခြင်းမရှိဘဲ WP2-B တွင်ထည့်သွင်းရန်။
- Column staging သည် FFT မှအသုံးပြုထားပြီးသော double-buffered pool ကိုလိုက်နာသည်။
  ပိုက်လိုင်းများ။ Poseidon kernels များသည် အဆိုပါ အဆင့်ရှိ ကြားခံများတွင် အကြမ်းညွှန်များကို လက်ခံသည်။
  မှတ်ဉာဏ်သတ်မှတ်မှုကို ထိန်းထားပေးသည့် ကမ္ဘာလုံးဆိုင်ရာ အစုအဝေးခွဲဝေမှုများကို ဘယ်တော့မှ မထိပါနှင့်
  CUDA host နှင့် ညှိထားသည်။

### မျှဝေထားသော ကိန်းသေများ

- တွင်ဖော်ပြထားသော `PoseidonSnapshot`
  `docs/source/fastpq/poseidon_metal_shared_constants.md` သည် ယခုအခါ စံသတ်မှတ်ချက်ဖြစ်သည်။
  round constants နှင့် MDS matrix အတွက် အရင်းအမြစ်။ Metal (`poseidon2.metal`) နှစ်မျိုးလုံး၊
  နှင့် CUDA (`fastpq_cuda.cu`) kernels များကို ထင်ရှားသည့်အချိန်တိုင်းတွင် ပြန်လည်ထုတ်ပေးရပါမည်
  အပြောင်းအလဲများ။
- WP2-B သည် runtime နှင့် manifest ကိုဖတ်သည့်သေးငယ်သော host loader ကိုထည့်လိမ့်မည်။
  SHA-256 ကို telemetry (`acceleration.poseidon_constants_sha`) သို့ ထုတ်လွှတ်သည်။
  တူညီသော ဒက်ရှ်ဘုတ်များသည် ထုတ်ဝေထားသော အရိပ်အာဝါသ ကိန်းသေများနှင့် ကိုက်ညီကြောင်း အခိုင်အမာ အာမခံနိုင်ပါသည်။
  လျှပ်တစ်ပြက်။
- သွေးပူချိန်တွင် ကျွန်ုပ်တို့သည် `TOTAL_ROUNDS x STATE_WIDTH` ကိန်းသေများကို တစ်ခုအဖြစ်သို့ ကူးယူပါမည်။
  `MTLBuffer` နှင့် စက်တစ်ခုလျှင် တစ်ကြိမ် အပ်လုဒ်လုပ်ပါ။ ထို့နောက် kernel တစ်ခုစီသည် data ကို ကူးယူသည်။
  ၎င်း၏အပိုင်းကို မလုပ်ဆောင်မီ threadgroup memory သို့ တိကျသေချာစေရန်၊
  ပျံသန်းမှုတွင် command buffers အများအပြားလည်ပတ်နေချိန်၌ပင် အမိန့်ပေးသည်။

### မှန်ကန်ကြောင်း ချိတ်များ

- ယူနစ်စစ်ဆေးမှုများ (`cargo test -p fastpq_prover --features fastpq-gpu`) တစ်ခုတိုးလာပါမည်။
  embedded shader constants ကို ခွဲပြီး ၎င်းတို့နှင့် နှိုင်းယှဥ်စေသော အခိုင်အမာပြောဆိုချက်
  GPU fixture suite ကို မလုပ်ဆောင်မီ manifest ၏ SHA။
- ရှိပြီးသား kernel ကိန်းဂဏန်းအချက်အလက်များကို ပြောင်းသွားသည် (`FASTPQ_METAL_TRACE_DISPATCH`၊
  `FASTPQ_METAL_QUEUE_FANOUT`, queue depth telemetry) လိုအပ်သော အထောက်အထားများ ဖြစ်လာသည်။
  WP2 ထွက်ပေါက်အတွက်- စမ်းသပ်လည်ပတ်မှုတိုင်းသည် အချိန်ဇယားကို မည်သည့်အခါမျှ မချိုးဖောက်ကြောင်း သက်သေပြရပါမည်။
  configured fan-out နှင့် fused trace kernel သည် တန်းစီခြင်းကို အောက်ဘက်တွင် ထိန်းသိမ်းထားသည်။
  adaptive window ပါ။
- Swift XCFramework မီးခိုးကြိုးနှင့် Rust စံနှုန်းအပြေးသမားများ စတင်ပါမည်။
  `acceleration.poseidon.permute_p90_ms{cpu,metal}` ကို တင်ပို့နေသောကြောင့် WP2-D ဇယားဆွဲနိုင်ပါသည်။
  တယ်လီမက်ထရီဖိဒ်အသစ်များကို ပြန်လည်တီထွင်ခြင်းမရှိဘဲ သတ္တု-နှင့်-CPU မြစ်ဝကျွန်းပေါ်ဒေသများ။

## WP2-B Poseidon Manifest Loader & Self-Test Parity- ယခု `fastpq_prover::poseidon_manifest()` ကို မြှုပ်နှံပြီး ခွဲခြမ်းစိတ်ဖြာလိုက်ပါ။
  `artifacts/offline_poseidon/constants.ron`၊ ၎င်း၏ SHA-256 ကိုတွက်ချက်သည်။
  (`poseidon_manifest_sha256()`)၊ CPU နှင့် လျှပ်တစ်ပြက်ရိုက်ချက်အား အတည်ပြုသည်။
  GPU အလုပ်တစ်ခုခုမလုပ်ဆောင်မီ poseidon ဇယားများ။ `build_metal_context()` သည် မှတ်တမ်းဖြစ်သည်။
  တယ်လီမက်ထရီ တင်ပို့သူများ ထုတ်ဝေနိုင်စေရန် သွေးပူချိန်အတွင်း ချေဖျက်ပါ။
  `acceleration.poseidon_constants_sha`။
- manifest parser သည် မကိုက်ညီသော width/rate/ round-count tuples နှင့် ငြင်းပယ်ပါသည်။
  Manifest MDS matrix သည် scalar အကောင်အထည်ဖော်မှုနှင့် ညီမျှကြောင်း သေချာစေပြီး၊
  Canonical Table များကို ပြန်လည်ထုတ်ပေးသောအခါ အသံတိတ်ပျံ့လွင့်သွားပါသည်။
- ထည့်ထားသော `crates/fastpq_prover/tests/poseidon_manifest_consistency.rs`၊
  `poseidon2.metal` တွင် ထည့်သွင်းထားသော Poseidon ဇယားများကို ခွဲခြမ်းစိပ်ဖြာပြီး
  `fastpq_cuda.cu` နှင့် kernels နှစ်ခုလုံးသည် အတိအကျတူညီကြောင်း အခိုင်အမာဆိုသည်
  ကိန်းသေများကို ထင်ရှားစေသည်။ တစ်စုံတစ်ယောက်သည် shader/CUDA ကို တည်းဖြတ်ပါက ယခု CI ပျက်သွားပါသည်။
  canonical manifest ကို ပြန်မထုတ်ဘဲ ဖိုင်များ။
- Future parity hooks (WP2-C/D) သည် `poseidon_manifest()` ကို အဆင့်မြှင့်တင်နိုင်သည်
  ကိန်းသေများကို GPU ကြားခံအဖြစ်သို့ ဝိုင်းပြီး Norito မှတစ်ဆင့် အချေအတင်ကို ဖော်ထုတ်ရန်
  telemetry feeds ။

## WP2-C BN254 သတ္တုပိုက်လိုင်းများနှင့် ပါရီတီစစ်ဆေးမှုများ- ** နယ်ပယ်နှင့် ကွာဟချက်-** လက်ခံဆောင်ရွက်ပေးသူများ၊ တူညီသောကြိုးများ နှင့် `bn254_status()` တို့ကို တိုက်ရိုက်ထုတ်လွှင့်နေပြီး `crates/fastpq_prover/metal/kernels/bn254.metal` သည် ယခုအခါ Montgomery primitives နှင့် threadgroup-synchronized FFT/LDE loops များကို အကောင်အထည်ဖော်နေပါသည်။ ပေးပို့မှုတစ်ခုစီသည် တစ်ခုချင်းအဆင့် အတားအဆီးများဖြင့် ချည်အုပ်စုတစ်ခုအတွင်းရှိ ကော်လံတစ်ခုလုံးကို လုပ်ဆောင်နေသောကြောင့် kernels များသည် အဆင့်လိုက်ဖော်ပြချက်များကို အပြိုင်လုပ်ဆောင်သည်။ Telemetry သည် ယခုအခါ ကြိုးတပ်ထားပြီး၊ အချိန်ဇယားကို အစားထိုးခြင်းများကို ဂုဏ်ယူသောကြောင့် Goldilocks kernels အတွက် ကျွန်ုပ်တို့အသုံးပြုသည့် တူညီသောအထောက်အထားဖြင့် ပုံသေဖွင့်ခြင်းအား တံခါးပိတ်နိုင်မည်ဖြစ်သည်။
- ** Kernel လိုအပ်ချက်များ-** ✅ အဆင့်အလိုက် twiddle/coset manifests များကို ပြန်လည်အသုံးပြုကာ အဝင်များ/အထွက်များကို တစ်ကြိမ်အဖြစ် ပြောင်းလဲကာ ကော်လံတစ်ခုလျှင် threadgroup အတွင်းရှိ radix-2 အဆင့်အားလုံးကို လုပ်ဆောင်ခြင်းဖြင့် multi-dispatch synchronisation မလိုအပ်ပါ။ Montgomery အကူအညီပေးသူများသည် FFT/LDE အကြား မျှဝေထားဆဲဖြစ်သောကြောင့် ကွင်းပတ်ဂျီသြမေတြီကိုသာ ပြောင်းလဲထားသည်။
- **အိမ်ရှင်ဝိုင်ယာကြိုးများ-** ✅ `crates/fastpq_prover/src/metal.rs` သည် canonical ခြေလက်အင်္ဂါများကို အဆင့်လိုက်လုပ်သည်၊ LDE ကြားခံကို သုညပြည့်စေသည်၊ ကော်လံတစ်ခုလျှင် ချည်မျှင်အုပ်စုတစ်ခုကို ရွေးချယ်ကာ `bn254_status()` ကို တံခါးပေါက်အတွက် ထုတ်ပြသည်။ telemetry အတွက် အပို host အပြောင်းအလဲများ မလိုအပ်ပါ။
- **Build guards:** `fastpq.metallib` သည် ကြွေပြားစေ့များကို ပို့ဆောင်ပေးသည်၊ ထို့ကြောင့် shader လွင့်သွားပါက CI သည် လျင်မြန်စွာ မအောင်မြင်သေးပါ။ မည်သည့် အနာဂတ် အကောင်းဆုံး ပြုပြင်မှုများမဆို compile-time switches များထက် telemetry/feature gates များနောက်တွင် ရှိနေသည်။
- **Parity fixtures:** ✅ `bn254_parity` စမ်းသပ်မှုများသည် CPU ပစ္စည်းများနှင့် GPU FFT/LDE အထွက်များကို နှိုင်းယှဉ်ပြီး ယခု Metal hardware ပေါ်တွင် တိုက်ရိုက်လည်ပတ်နေပါသည်။ kernel ကုဒ်လမ်းကြောင်းအသစ်များပေါ်လာပါက tampered-manifest စမ်းသပ်မှုများကိုစိတ်ထဲထားပါ။
- **တယ်လီမီတာနှင့် စံနှုန်းများ-** `fastpq_metal_bench` သည် ယခု ထုတ်လွှတ်သည်-
  - FFT/LDE single-column batches အတွက် FFT/LDE single-column batches အတွက် ကျိုးကြောင်းဆီလျော်သော thread အရေအတွက်၊ ယုတ္တိတန်သော thread အရေအတွက်နှင့် ပိုက်လိုင်းကန့်သတ်ချက်များကို အကျဉ်းချုပ်ဖော်ပြသော `bn254_dispatch` ဘလောက်တစ်ခု။ နှင့်
  - CPU အခြေခံလိုင်းအတွက် `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` ကို မှတ်တမ်းတင်သည့် `bn254_metrics` ဘလောက်တစ်ခုသည် CPU အခြေခံလိုင်းနှင့် မည်သည့် GPU နောက်ကွယ်မှ လည်ပတ်နေပါစေ။
  စံသတ်မှတ်ထားသော wrapper သည် မြေပုံနှစ်ခုလုံးကို ထုပ်ပိုးထားသော artefact တစ်ခုစီတွင် မိတ္တူကူးထားသောကြောင့် WP2-D ဒက်ရှ်ဘုတ်များမှ အညွှန်းတပ်ထားသော latencies/geometry သည် အကြမ်းထည် လည်ပတ်မှုခင်းကျင်းခြင်းကို နောက်ပြန်မဖြစ်အောင် အင်ဂျင်နီယာချုပ်လုပ်ခြင်းမပြုဘဲ ထည့်သွင်းပါသည်။ `FASTPQ_METAL_THREADGROUP` သည် ယခုအခါ BN254 FFT/LDE ပေးပို့မှုများကိုလည်း အသုံးပြုနိုင်ပြီဖြစ်ပြီး perf အတွက် အသုံးပြုနိုင်သော ခလုတ်များကို ပြုလုပ်ပေးပါသည်။ တံမြက်လှည်းများ။

## အဖွင့်မေးခွန်းများ (မေလ 2027 တွင် ဖြေရှင်းခဲ့သည်)1. **သတ္တုအရင်းအမြစ်ရှင်းလင်းခြင်း-** `warm_up_metal()` သည် thread-local ကို ပြန်လည်အသုံးပြုသည်
   `OnceCell` နှင့် ယခုအခါ ခုခံနိုင်စွမ်း/ဆုတ်ယုတ်မှု စမ်းသပ်မှုများ ရှိသည်။
   (`crates/ivm/src/vector.rs::warm_up_metal_reuses_cached_state`/
   `warm_up_metal_is_noop_on_non_metal_targets`)၊ ထို့ကြောင့် app lifecycle ကူးပြောင်းမှုများ
   ပေါက်ကြားခြင်း သို့မဟုတ် နှစ်ဆစတင်ခြင်းမရှိဘဲ သွေးပူလမ်းကြောင်းကို လုံခြုံစွာခေါ်ဆိုနိုင်သည်။
2. ** စံသတ်မှတ်ချက်များ-** သတ္တုလမ်းကြောင်းများသည် CPU ၏ 20% အတွင်း ရှိနေရမည်
   FFT/IFFT/LDE အတွက် အခြေခံမျဥ်းနှင့် Poseidon CRC/ Merkle အကူအညီများအတွက် 15% အတွင်း၊
   `acceleration.*_perf_delta_pct > 0.20` (သို့မဟုတ် ပျောက်ဆုံးနေချိန်တွင်) မီးသင့်သည်ဟု သတိပေးချက်
   mobile parity feed တွင် 20k ခြေရာကောက်အစုအဝေးတွင် IFFT ဆုတ်ယုတ်မှုများ
   WP2-D တွင် ဖော်ပြထားသည့် တန်းစီ override fix ဖြင့် ယခု တံခါးပိတ်ထားသည်။
3. **StrongBox အမှားအယွင်း-** Swift သည် Android ဘက်ပြန်ပြန်ဖွင့်စာအုပ်ကို လိုက်နာသည်။
   ပံ့ပိုးမှုအစီအစဥ်စာအုပ်တွင် သက်သေခံချက်မအောင်မြင်မှုများကို မှတ်တမ်းတင်ခြင်း။
   (`docs/source/sdk/swift/support_playbook.md`) မှ အလိုအလျောက်ပြောင်းခြင်း။
   စာရင်းစစ်မှတ်တမ်းဖြင့် HKDF ကျောထောက်နောက်ခံပြုထားသော ဆော့ဖ်ဝဲလ်လမ်းကြောင်း၊ parity vector များ
   လက်ရှိ OA ပွဲစဉ်များမှတစ်ဆင့် မျှဝေနေထိုင်ပါ။
4. **Telemetry storage-** Acceleration captures and device pool proofs များ
   `configs/swift/` အောက်တွင် သိမ်းဆည်းထားသည် (ဥပမာ၊
   `configs/swift/xcframework_device_pool_snapshot.json`) နှင့် တင်ပို့ရောင်းချသူများ
   တူညီသော အပြင်အဆင်ကို မှန်သင့်သည် (`artifacts/swift/telemetry/acceleration/*.json`
   သို့မဟုတ် `.prom`) ထို့ကြောင့် Buildkite မှတ်ချက်များနှင့် portal ဒက်ရှ်ဘုတ်များသည် ၎င်းကို ထည့်သွင်းနိုင်သည်။
   ad-hoc မခြစ်ဘဲ ကျွေးသည်။

## နောက်ထပ်အဆင့်များ (ဖေဖော်ဝါရီ 2026)

- [x] Rust: land Metal host ပေါင်းစပ်မှု (`crates/fastpq_prover/src/metal.rs`) နှင့်
      Swift အတွက် kernel interface ကိုဖော်ထုတ်ပါ။ doc hand-off တွဲပြီး ခြေရာခံပါ။
      လျင်မြန်သောတံတားမှတ်ချက်။
- [x] Swift- SDK အဆင့် အရှိန်မြှင့်ဆက်တင်များကို ဖော်ထုတ်ပါ (ဇန်နဝါရီ 2026 ပြီးသည်)။
- [x] Telemetry- `scripts/acceleration/export_prometheus.py` သည် ယခု ပြောင်းလဲသွားပါသည်။
      `cargo xtask acceleration-state --format json` အထွက်သည် Prometheus သို့
      textfile (ရွေးချယ်နိုင်သော `--instance` အညွှန်းပါရှိသော) ထို့ကြောင့် CI လည်ပတ်မှုများသည် GPU/CPU ကို ပူးတွဲနိုင်သည်
      enablement၊ thresholds နှင့် parity/disable အကြောင်းပြချက်များသည် textfile သို့ တိုက်ရိုက်
      စိတ်ကြိုက်ခြစ်ခြင်းမရှိဘဲ စုဆောင်းသူများ၊
- [x] Swift QA- `scripts/acceleration/acceleration_matrix.py` အများအပြားကို စုစည်းထားသည်
      စက်ပစ္စည်းဖြင့် သော့ခတ်ထားသော JSON သို့မဟုတ် Markdown ဇယားများသို့ အရှိန်နှုန်း-အခြေအနေ ဖမ်းယူမှုများ
      မီးခိုးကြိုးကို အဆုံးအဖြတ်ပေးသော “CPU vs Metal/CUDA” မက်ထရစ်ကို အညွှန်းပေးသည်။
      နမူနာ-app တွဲပြီး အပ်လုဒ်လုပ်ရန်။ Markdown output သည် ၎င်းကို ထင်ဟပ်စေသည်။
      ဒက်ရှ်ဘုတ်များသည် တူညီသောပစ္စည်းကို ထည့်သွင်းနိုင်သောကြောင့် Buildkite အထောက်အထားဖော်မတ်။
- [x] `irohad` သည် တန်းစီခြင်း/သုည-ဖြည့်စွက်တင်ပို့သူများကို ပို့ဆောင်ပေးသည့် ယခု status.md ကို အပ်ဒိတ်လုပ်ပါ။
      env/config validation tests များသည် Metal queue overrides များကို ဖုံးအုပ်ထားသောကြောင့် WP2-D
      တယ်လီမီတာနှင့် စည်းနှောင်မှုများတွင် တိုက်ရိုက် အထောက်အထား ပူးတွဲပါရှိသည်။【crates/irohad/src/main.rs:2664】【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【status.md:1546】

Telemetry/export အကူအညီပေးသည့် အမိန့်များ-

```bash
# Prometheus textfile from a single capture
cargo xtask acceleration-state --format json > artifacts/acceleration_state_macos_m4.json
python3 scripts/acceleration/export_prometheus.py \
  --input artifacts/acceleration_state_macos_m4.json \
  --output artifacts/acceleration_state_macos_m4.prom \
  --instance macos-m4

# Aggregate multiple captures into a Markdown matrix
python3 scripts/acceleration/acceleration_matrix.py \
  --state macos-m4=artifacts/acceleration_state_macos_m4.json \
  --state sim-m3=artifacts/acceleration_state_sim_m3.json \
  --format markdown \
  --output artifacts/acceleration_matrix.md
```

## WP2-D ထုတ်ဝေမှု စံညွှန်းနှင့် စည်းနှောင်ခြင်း မှတ်စုများ- **20k-row release capture-** macOS14 တွင် သတ္တုနှင့် CPU စံနှုန်းကို မှတ်တမ်းတင်ထားသည်။
  (arm64၊ လမ်းသွား-ဟန်ချက်ညီသော ကန့်သတ်ဘောင်များ၊ padded 32,768-row လမ်းကြောင်း၊ ကော်လံနှစ်ခု) နှင့်
  JSON အစုအဝေးကို `fastpq_metal_bench_20k_release_macos14_arm64.json` သို့ စစ်ကြည့်ပါ။
  စံနှုန်းသည် လည်ပတ်မှုအချိန်ဇယားများအပြင် Poseidon microbench အထောက်အထားများကို ထုတ်ပေးပါသည်။
  WP2-D တွင် Metal တန်းစီခြင်းဆိုင်ရာ heuristics အသစ်နှင့် ချိတ်ဆက်ထားသော GA အရည်အသွေး လက်ရာများ ရှိသည်။ ခေါင်းကြီး
  deltas (ဇယားအပြည့်အစုံ `docs/source/benchmarks.md` တွင်နေထိုင်သည်)

  | စစ်ဆင်ရေး | CPU ဆိုသည်မှာ (ms) | သတ္တုဆိုလို (ms) | အရှိန်မြှင့် |
  |-----------|----------------|-----------------|---------|
  | FFT (32,768 သွင်းအားစု) | 12.741 | 10.963 | 1.16× |
  | IFFT (32,768 သွင်းအားစု) | 17.499 | 25.688 | 0.68× *(ဆုတ်ယုတ်မှု- အဆုံးအဖြတ်ကို ထိန်းသိမ်းရန် တန်းစီနေသော ပန်ကာ-အထွက်ကို တွန်းထုတ်သည်၊ နောက်ဆက်တွဲ ချိန်ညှိမှု လိုအပ်သည်)* |
  | LDE (262,144 သွင်းအားစု) | 68.389 | 65,701 | 1.04× |
  | Poseidon hash ကော်လံများ (524,288 inputs) | 1,728.835 | 1,447.076 | 1.19× |

  ဖမ်းယူမှုမှတ်တမ်းတစ်ခုစီသည် `zero_fill` အချိန်များ (33,554,432 bytes အတွက် 9.651ms) နှင့်
  `poseidon_microbench` ထည့်သွင်းမှုများ (မူလလမ်းကြောင်း 596.229ms နှင့် scalar 656.251ms၊
  1.10× speedup) ထို့ကြောင့် ဒက်ရှ်ဘုတ်သုံးစွဲသူများသည် ၎င်းနှင့်တကွ တန်းစီခြင်းဖိအားကို ကွဲပြားစေနိုင်သည်။
  ပင်မလုပ်ငန်းများ။
- **Bindings/docs cross-link:** `docs/source/benchmarks.md` သည် ယခု ကိုးကားပါသည်။
  JSON နှင့် ပြန်လည်ထုတ်လုပ်သူ အမိန့်ကို ထုတ်ပြန်လိုက်သည်၊ သတ္တုတန်းစီအား အစားထိုးမှုများကို အတည်ပြုထားသည်။
  `iroha_config` env/manifest စမ်းသပ်မှုများမှတစ်ဆင့် `irohad` သည် တိုက်ရိုက်ထုတ်လွှင့်သည်
  `fastpq_metal_queue_*` တိုင်းထွာချက်များ ထို့ကြောင့် ဒက်ရှ်ဘုတ်များမပါဘဲ IFFT ဆုတ်ယုတ်မှုများကို အလံပြသည်
  ad-hoc မှတ်တမ်းကိုခြစ်ခြင်း။ Swift ၏ `AccelerationSettings.runtimeState` က ဖော်ထုတ်ပေးသည်။
  တူညီသော telemetry payload ကို JSON အစုအဝေးတွင် တင်ပို့ပြီး WP2-D ကို ပိတ်ပါ။
  ပြန်လည်ထုတ်လုပ်နိုင်သော လက်ခံမှုအခြေခံလိုင်းဖြင့် စည်းနှောင်ခြင်း/ doc ကွာဟချက်။ 【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【crates/irohad/src/main.rs:2664】
- **IFFT တန်းစီပြင်ဆင်ခြင်း-** Inverse FFT အတွဲများကို ယခုအချိန်တိုင်းတွင် တန်းစီအများအပြားပေးပို့ခြင်းကို ကျော်သွားသည်
  အလုပ်ဝန်သည် ပန်ကာအထွက်အဆင့်နှင့် မပြည့်မီပါ (လမ်းကြော-ဟန်ချက်ညီသော ကော်လံ ၁၆ ခု၊
  ပရိုဖိုင်)၊ ထိန်းသိမ်းထားစဉ် အထက်ဖော်ပြပါ Metal-vs-CPU ဆုတ်ယုတ်မှုကို ဖယ်ရှားခြင်း။
  FFT/LDE/Poseidon အတွက် တန်းစီလမ်းကြောင်းများစွာရှိ ကြီးမားသော ကော်လံများ။