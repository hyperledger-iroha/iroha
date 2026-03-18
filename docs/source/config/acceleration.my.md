---
lang: my
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Acceleration & Norito Heuristics အကိုးအကား

`[accel]` ဘလောက်သည် `iroha_config` လိုင်းများမှတဆင့်
`crates/irohad/src/main.rs:1895` သို့ `ivm::set_acceleration_config`။ အိမ်ရှင်တိုင်း
VM ကို ချက်ခြင်းမပြုလုပ်မီ တူညီသော အဖုများကို အသုံးပြုထားသောကြောင့် အော်ပရေတာများက အဆုံးအဖြတ်ပေးနိုင်ပါသည်။
scalar/SIMD မှားယွင်းမှုများကို ထိန်းသိမ်းထားစဉ်တွင် မည်သည့် GPU နောက်ကွယ်မှ ခွင့်ပြုထားသည်ကို ရွေးပါ။
Swift၊ Android နှင့် Python bindings များသည် bridge layer မှတဆင့် တူညီသော manifest ကို load လုပ်ပါသည်။
ဤမူရင်းများကို မှတ်တမ်းပြုစုခြင်းသည် ဟာ့ဒ်ဝဲ-အရှိန်မြှင့်ခြင်းနောက်ကွယ်တွင် WP6-C ကို ပိတ်ဆို့စေပါသည်။

### `accel` (ဟာ့ဒ်ဝဲ အရှိန်မြှင့်ခြင်း)

အောက်ပါဇယားသည် `docs/source/references/peer.template.toml` နှင့် ကြေးမုံ
`iroha_config::parameters::user::Acceleration` အဓိပ္ပါယ်၊ ပတ်ဝန်းကျင်ကို ဖော်ထုတ်ခြင်း။
သော့တစ်ခုစီကို လွှမ်းမိုးနိုင်သော ကိန်းရှင်။

| သော့ | Env var | ပုံသေ | ဖော်ပြချက် |
|-----|---------|---------|-------------|
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` | SIMD/NEON/AVX လုပ်ဆောင်မှုကို ဖွင့်ပါ။ `false` သောအခါ၊ VM သည် အဆုံးအဖြတ်ပေးသော တူညီမှုဖမ်းယူမှုများကို လွယ်ကူစေရန်အတွက် vector ops နှင့် Merkle hashing အတွက် scalar backend များကို တွန်းအားပေးသည်။ |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` | ၎င်းကိုစုစည်းပြီးသောအခါ CUDA နောက်ခံကိုဖွင့်ပြီး runtime သည် ရွှေရောင်ဗတ်တာစစ်ဆေးမှုများအားလုံးကိုဖြတ်သန်းသည်။ |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` | macOS တည်ဆောက်မှုများတွင် Metal backend ကိုဖွင့်ပါ။ မှန်သည့်တိုင်၊ တူညီမှုမတူညီမှုများဖြစ်ပေါ်ပါက Metal ကိုယ်တိုင်စမ်းသပ်မှုများသည် runtime တွင် backend ကိုပိတ်နိုင်သည်။ |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0` (အော်တို) | runtime ကနဦးအစတွင် ရုပ်ပိုင်းဆိုင်ရာ GPU မည်မျှရှိသည်ကို စာလုံးပါရှိသည်။ `0` ဆိုသည်မှာ "ကိုက်ညီသောဟာ့ဒ်ဝဲလ်ပန်ကာ-အထွက်" ကိုဆိုလိုပြီး `GpuManager` ဖြင့် ချုပ်ထားသည်။ |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | Merkle အရွက်သည် GPU သို့ offload များမတင်မီ အနည်းဆုံးအရွက်လိုအပ်သည်။ PCIe overhead (`crates/ivm/src/byte_merkle_tree.rs:49`) ကိုရှောင်ရှားရန် ဤသတ်မှတ်ချက်အောက်ရှိတန်ဖိုးများသည် CPU ပေါ်တွင် ဟက်ခ်လုပ်နေပါသည်။ |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0` (ကမ္ဘာ့အမွေအနှစ်) | GPU သတ်မှတ်ချက်အတွက် သတ္တု-သီးသန့် အစားထိုးမှု။ `0` တွင် Metal သည် `merkle_min_leaves_gpu` ကို အမွေဆက်ခံသည်။ |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0` (ကမ္ဘာ့အမွေအနှစ်) | GPU အဆင့်သတ်မှတ်မှုအတွက် CUDA သီးသန့် override။ `0` တွင် CUDA သည် `merkle_min_leaves_gpu` ကို အမွေဆက်ခံသည်။ |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0` (32768 အတွင်းပိုင်း) | GPU hashing ထက် ARMv8 SHA-2 ညွှန်ကြားချက်များ အနိုင်ရသင့်သည့် သစ်ပင်အရွယ်အစားကို စာလုံးပါရှိသည်။ `0` သည် `32_768` အရွက် (`crates/ivm/src/byte_merkle_tree.rs:59`) ၏ မူရင်းအတိုင်း စုစည်းထားသည်။ |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | `0` (32768 အတွင်းပိုင်း) | အထက်ဖော်ပြပါအတိုင်းဖြစ်သော်လည်း x86/x86_64 host များအတွက် SHA-NI (`crates/ivm/src/byte_merkle_tree.rs:63`) ကို အသုံးပြုထားသည်။ |

`enable_simd` သည် RS16 erasure coding (Torii DA ingest + tooling) ကိုလည်း ထိန်းချုပ်ပါသည်။ အဲဒါကို ပိတ်လိုက်ပါ။
ဟာ့ဒ်ဝဲတစ်လျှောက်တွင် အထွက်များကို အဆုံးအဖြတ်ပေးသည့် အတိုင်းအတာကို ထိန်းသိမ်းထားစဉ် စကလာတန်းတူမျိုးဆက်ကို တွန်းအားပေးသည်။

နမူနာဖွဲ့စည်းပုံ-

```toml
[accel]
enable_simd = true
enable_cuda = true
enable_metal = true
max_gpus = 2
merkle_min_leaves_gpu = 12288
merkle_min_leaves_metal = 8192
merkle_min_leaves_cuda = 16384
prefer_cpu_sha2_max_leaves_aarch64 = 65536
```

နောက်ဆုံးသော့ငါးခုအတွက် သုညတန်ဖိုးများသည် "စုစည်းထားသော ပုံသေကို ထားရှိရန်" ကိုဆိုလိုသည်။ အိမ်ရှင်မဖြစ်ရဘူး။
ကွဲလွဲနေသော အစားထိုးမှုများကို သတ်မှတ်ခြင်း (ဥပမာ၊ CUDA သီးသန့် သတ်မှတ်ချက်များကို တွန်းအားပေးနေစဉ် CUDA ကို ပိတ်ခြင်း)
သို့မဟုတ်ပါက တောင်းဆိုချက်ကို လျစ်လျူရှုထားပြီး နောက်ကွယ်မှသည် ကမ္ဘာလုံးဆိုင်ရာမူဝါဒကို ဆက်လက်လိုက်နာနေပါသည်။

### runtime အခြေအနေ စစ်ဆေးခြင်း။အသုံးပြုထားသည့်အရာကို လျှပ်တစ်ပြက်ရိုက်ရန် `cargo xtask acceleration-state [--format table|json]` ကိုဖွင့်ပါ။
Metal/CUDA runtime health bits များနှင့်အတူ ဖွဲ့စည်းမှု။ အမိန့်ကိုဆွဲသည်။
လက်ရှိ `ivm::acceleration_config`၊ တူညီမှုအခြေအနေနှင့် စေးကပ်သော အမှားအယွင်းများ (တစ်ခုလျှင်
backend ကိုပိတ်ထားသည်) ထို့ကြောင့် လုပ်ဆောင်ချက်များသည် ရလဒ်ကို တူညီမှုအဖြစ် တိုက်ရိုက်ပေးနိုင်ပါသည်။
ဒက်ရှ်ဘုတ်များ သို့မဟုတ် အဖြစ်အပျက် သုံးသပ်ချက်။

```
$ cargo xtask acceleration-state
Acceleration Configuration
--------------------------
enable_simd: yes
enable_metal: yes
enable_cuda: no
max_gpus: 1
merkle_min_leaves_gpu: 8192
merkle_min_leaves_metal: 8192
merkle_min_leaves_cuda: auto
prefer_cpu_sha2_max_leaves_aarch64: auto
prefer_cpu_sha2_max_leaves_x86: auto

Backend Status
--------------
Backend Supported  Configured  Available  ParityOK  Last error
SIMD    yes        yes         yes        yes       -
Metal   yes        yes         yes        yes       -
CUDA    no         no          no         no        policy disabled (no CUDA libraries present)
```

လျှပ်တစ်ပြက်ရိုက်ချက်အား အလိုအလျောက်စနစ်ဖြင့် ထည့်သွင်းရန်လိုအပ်သည့်အခါ `--format json` ကို အသုံးပြုပါ (JSON
ဇယားတွင်ပြသထားသည့်တူညီသောအကွက်များပါရှိသည်။)

ယခု `acceleration_runtime_errors()` သည် SIMD အဘယ်ကြောင့် ပြန်ကျသွားသည်ကို ယခုခေါ်ဆိုသည်-
`disabled by config`၊ `forced scalar override`၊ `simd unsupported on hardware` သို့မဟုတ်
ထောက်လှမ်းမှုအောင်မြင်သောအခါတွင် `simd unavailable at runtime` သည် လုပ်ဆောင်ချက်ဆက်လက်လုပ်ဆောင်နေပါသည်။
vectors မပါဘဲ။ ပေါ်လစီကို ပြန်ဖျက်ခြင်း သို့မဟုတ် ပြန်ဖွင့်ခြင်းအား ဖျက်ခြင်းဖြင့် မက်ဆေ့ချ်ကို လွှတ်လိုက်ပါသည်။
SIMD ကို ပံ့ပိုးသည့် host များပေါ်တွင်

### ကွာဟမှုစစ်ဆေးမှုများ

အဆုံးအဖြတ်ရလဒ်များကိုသက်သေပြရန် CPU-only နှင့် accel-on ကြားတွင် `AccelerationConfig` ကိုလှန်ပါ။
`poseidon_instructions_match_across_acceleration_configs` ဆုတ်ယုတ်မှုသည် ၎င်းကို လုပ်ဆောင်သည်။
Poseidon2/6 opcode များကို နှစ်ကြိမ်—ပထမတွင် `enable_cuda`/`enable_metal` ဖြင့် `false` သို့သတ်မှတ်ပြီးနောက်၊
နှစ်ခုစလုံးကို ဖွင့်ထားခြင်းဖြင့်—GPU များရှိနေသောအခါတွင် ထပ်တူထပ်မျှ အထွက်များ နှင့် CUDA parity ကို အခိုင်အမာပြောပါသည်။【crates/ivm/tests/crypto.rs:100】
backends ရှိမရှိမှတ်တမ်းတင်ရန် run နှင့်အတူ `acceleration_runtime_status()` ကိုရိုက်ပါ။
ဓာတ်ခွဲခန်းမှတ်တမ်းများတွင် ပြင်ဆင်သတ်မှတ်/ရနိုင်သည်။

```rust
let baseline = ivm::acceleration_config();
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: false,
    enable_metal: false,
    ..baseline
});
// run CPU-only parity workload
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: true,
    enable_metal: true,
    ..baseline
});

When isolating SIMD/NEON differences, set `enable_simd = false` to force scalar
execution. The `disabling_simd_forces_scalar_and_preserves_outputs` regression
forces the scalar backend and asserts vector ops stay bit-identical to the
SIMD-enabled baseline on the same host while surfacing the `simd` status/error
fields via `acceleration_runtime_status`/`acceleration_runtime_errors`.【crates/ivm/tests/acceleration_simd.rs:9】
```

### GPU သည် ပုံသေနှင့် ဖြစ်စဉ်များ

`MerkleTree` GPU offload သည် `8192` တွင် မူလအတိုင်းထွက်သွားပြီး၊ CPU SHA-2
ဦးစားပေးသတ်မှတ်ချက်များသည် ဗိသုကာတစ်ခုလျှင် `32_768` တွင်ရှိနေပါသည်။ CUDA လည်းမဟုတ်တဲ့အခါ
သတ္တုကို ရရှိနိုင်သည် သို့မဟုတ် ကျန်းမာရေးစစ်ဆေးမှုများဖြင့် ပိတ်ထားပြီး VM သည် အလိုအလျောက် ကျသွားပါသည်။
SIMD/scalar hashing သို့ ပြန်သွားရန်နှင့် အထက်ဖော်ပြပါ နံပါတ်များသည် အဆုံးအဖြတ်ကို မထိခိုက်စေပါ။

`max_gpus` သည် `GpuManager` တွင် ထည့်ထားသော ရေကန်အရွယ်အစားကို ကုပ်ထားသည်။ `max_gpus = 1` ကိုဖွင့်သတ်မှတ်ခြင်း။
Multi-GPU host များသည် အရှိန်မြှင့်ခြင်းကို ခွင့်ပြုနေချိန်တွင် telemetry ကို ရိုးရှင်းစေသည်။ အော်ပရေတာများ လုပ်နိုင်သည်။
FASTPQ သို့မဟုတ် CUDA Poseidon အလုပ်များအတွက် ကျန်ရှိသော စက်များကို သိမ်းဆည်းရန် ဤခလုတ်ကို အသုံးပြုပါ။

### နောက်တစ်ခု အရှိန်မြှင့်ရန် ပစ်မှတ်များနှင့် ဘတ်ဂျက်များ

နောက်ဆုံးထွက် FastPQ သတ္တုခြေရာကောက် (`fastpq_metal_bench_20k_latest.json`၊ 32K အတန်းများ × 16
ကော်လံများ၊ 5 iters) ZK အလုပ်တာဝန်များကို လွှမ်းမိုးထားသည့် Poseidon ကော်လံကို ပြသသည်-

- `poseidon_hash_columns`- CPU ဆိုသည်မှာ **3.64s** နှင့် GPU အဓိပ္ပာယ် **3.55s** (1.03×)။
- `lde`- CPU ဆိုသည်မှာ **1.75s** နှင့် GPU အဓိပ္ပာယ် **1.57s** (1.12×)။

IVM/Crypto သည် လာမည့် accel sweep တွင် ဤ kernel နှစ်ခုကို ပစ်မှတ်ထားမည်ဖြစ်သည်။ အခြေခံဘတ်ဂျက်များ-

- အထက်ဖော်ပြပါ အဓိပ္ပါယ်မှာ CPU/SIMD parity ကို သို့မဟုတ် အောက်တွင်ထားရှိကာ ဖမ်းယူပါ။
  `acceleration_runtime_status()` သည် အပြေးတစ်ခုစီနှင့်တွဲလျက် ဖြစ်သောကြောင့် Metal/CUDA ရရှိနိုင်မှုဖြစ်သည်။
  ဘတ်ဂျက်နံပါတ်များဖြင့် မှတ်တမ်းတင်ထားသည်။
- `poseidon_hash_columns` အတွက် ပစ်မှတ် ≥1.3× နှင့် `lde` အတွက် ≥1.2× တစ်ကြိမ် ညှိထားသော Metal
  နှင့် CUDA kernels များသည် အထွက်များ သို့မဟုတ် တယ်လီမီတာ အညွှန်းများ ပြောင်းလဲခြင်းမရှိဘဲ မြေနေရာ၊

JSON ခြေရာကောက်နှင့် `cargo xtask acceleration-state --format json` လျှပ်တစ်ပြက်ရိုက်ချက်တို့ကို ပူးတွဲပါ။
အနာဂတ်ဓာတ်ခွဲခန်းသည် အလုပ်လုပ်နေသောကြောင့် CI/ဆုတ်ယုတ်မှုများသည် ဘတ်ဂျက်များနှင့် နောက်ခံကျန်းမာရေး နှစ်ခုလုံးကို အခိုင်အမာရရှိနိုင်ပါသည်။
CPU-only နှင့် accel-on လုပ်ဆောင်မှုများကို နှိုင်းယှဉ်ခြင်း။

### Norito heuristics (compile-time defaults)Norito ၏ အပြင်အဆင် နှင့် ဖိသိပ်မှု ဆိုင်ရာ သဘောတရားများသည် `crates/norito/src/core/heuristics.rs` တွင် နေထိုင်သည်
binary တစ်ခုစီတွင် စုစည်းထားသည်။ ၎င်းတို့ကို runtime တွင် သတ်မှတ်၍မရသော်လည်း ထုတ်ဖော်ပြသခြင်း
သွင်းအားစုများသည် SDK နှင့် အော်ပရေတာအဖွဲ့များအား Norito သည် GPU ပြီးသည်နှင့် မည်သို့ပြုမူမည်ကို ခန့်မှန်းကူညီပေးသည်
compression kernels ကို ဖွင့်ထားသည်။
ယခုအခါ အလုပ်ခွင်သည် Norito ကို ပုံသေဖြင့်ဖွင့်ထားသည့် `gpu-compression` အင်္ဂါရပ်ဖြင့် တည်ဆောက်သည်၊
ထို့ကြောင့် GPU zstd နောက်ခံများကို စုစည်းထားသည်။ runtime ရရှိနိုင်မှုသည် hardware ပေါ်တွင်မူတည်နေဆဲဖြစ်သည်၊
ကူညီသူစာကြည့်တိုက် (`libgpuzstd_*`/`gpuzstd_cuda.dll`) နှင့် `allow_gpu_compression`
config အလံ။ `cargo build -p gpuzstd_metal --release` နှင့် Metal helper ကိုတည်ဆောက်ပါ။
`libgpuzstd_metal.dylib` ကို loader လမ်းကြောင်းပေါ်တွင်ထားပါ။ လက်ရှိ Metal helper သည် GPU ကို လုပ်ဆောင်သည်။
match-finding/sequence generation နှင့် in-crate deterministic zstd frame ကို အသုံးပြုသည်။
လက်ခံသူပေါ်ရှိ ကုဒ်ပြောင်းကိရိယာ (Huffman/FSE + ဘောင်တပ်ဆင်မှု)၊ decode သည် in-crate frame ကိုအသုံးပြုသည်။
GPU ပိတ်ဆို့ရေးကုဒ်ကို ကြိုးတပ်မထားသည့်တိုင်အောင် ပံ့ပိုးမထားသောဘောင်များအတွက် CPU zstd နောက်ခံကုဒ်ဖြင့် ဒီကုဒ်ဒါ။

| လယ် | ပုံသေ | ရည်ရွယ်ချက် |
|---------|---------|---------|
| `min_compress_bytes_cpu` | `256` bytes | ဤအရာအောက်တွင်၊ ဝန်ဆောင်ခများသည် zstd ကို လုံးဝကျော်သွားသည် ။ |
| `min_compress_bytes_gpu` | `1_048_576` ဘိုက် (1MiB) | `norito::core::hw::has_gpu_compression()` မှန်သောအခါတွင် ဤကန့်သတ်ချက်နှင့်အထက်တွင် ပေးဆောင်မှုများသည် GPU zstd သို့ပြောင်းသည်။ |
| `zstd_level_small` / `zstd_level_large` | `1` / `3` | <32KiB နှင့် ≥32KiB payloads အသီးသီးအတွက် CPU ချုံ့မှုအဆင့်များ။ |
| `zstd_level_gpu` | `1` | ကွန်ဆာဗေးတစ် GPU အဆင့်သည် အမိန့်စာတန်းများကိုဖြည့်နေစဉ် latency တသမတ်တည်းရှိနေရန်။ |
| `large_threshold` | `32_768` bytes | "အသေး" နှင့် "ကြီး" CPU zstd အဆင့်များကြား အရွယ်အစား နယ်နိမိတ်။ |
| `aos_ncb_small_n` | `64` အတန်း | ဤအတန်း၏အောက်တွင် အသေးငယ်ဆုံး payload ကိုရွေးချယ်ရန်အတွက် adaptive encoders များသည် AoS နှင့် NCB အပြင်အဆင်နှစ်ခုလုံးကို စူးစမ်းလေ့လာပါသည်။ |
| `combo_no_delta_small_n_if_empty` | `2` တန်း | အတန်း 1-2 တွင် ဆဲလ်အလွတ်များပါရှိသောအခါ u32/id delta ကုဒ်နံပါတ်များကို ဖွင့်ခြင်းကို တားဆီးသည်။ |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` | Deltas ကန်သွင်းရာတွင် အနည်းဆုံး အတန်းနှစ်တန်းရှိပါသည် ။ |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` | ကောင်းစွာ ပြုမူထားသော ထည့်သွင်းမှုများအတွက် မြစ်ဝကျွန်းပေါ်အသွင်ပြောင်းမှုအားလုံးကို မူရင်းအတိုင်း ဖွင့်ထားသည်။ |
| `combo_enable_name_dict` | `true` | hit ratios များသည် memory overhead ကို အကြောင်းပြုပြီး ကော်လံတစ်ခုစီအဘိဓာန်များကို ခွင့်ပြုသည်။ |
| `combo_dict_ratio_max` | `0.40` | အတန်းများ၏ 40% ထက်ပိုကွဲပြားသောအခါ အဘိဓာန်များကို ပိတ်ပါ။ |
| `combo_dict_avg_len_min` | `8.0` | အဘိဓာန်များ မတည်ဆောက်မီ ပျမ်းမျှစာကြောင်းအရှည် ≥8 လိုအပ်သည် (အတိုကောက်အမည်များ လိုင်းတွင်ရှိနေသည်)။ |
| `combo_dict_max_entries` | `1024` | အကန့်အသတ်ရှိသော မမ်မိုရီအသုံးပြုမှုကို အာမခံရန် အဘိဓာန်ထည့်သွင်းမှုများတွင် ခဲစာလုံးထုပ်ပါ။ |

ဤအယူဝါဒများသည် GPU-ဖွင့်ထားသော host များကို CPU-သီးသန့်လုပ်ဖော်ကိုင်ဖက်များနှင့် လိုက်လျောညီထွေဖြစ်စေသည်- ရွေးချယ်ကိရိယာ
ဝါယာကြိုးဖော်မတ်ကို ပြောင်းလဲမည့် မည်သည့်အခါမျှ ဆုံးဖြတ်ချက်မချဘဲ တံခါးခုံများကို ပြင်ဆင်ထားသည်။
ထုတ်ဝေမှုနှုန်း။ ပရိုဖိုင်းလုပ်ခြင်းသည် ပိုမိုကောင်းမွန်သည့် အကျိုးအမြတ်အချက်များကို ဖော်ထုတ်သောအခါ၊ Norito သည် အပ်ဒိတ်လုပ်သည်
Canonical `Heuristics::canonical` အကောင်အထည်ဖော်မှုနှင့် `docs/source/benchmarks.md` အပေါင်း
`status.md` သည် ဗားရှင်းပြောင်းထားသော အထောက်အထားများနှင့်အတူ အပြောင်းအလဲကို မှတ်တမ်းတင်သည်။GPU zstd helper သည် တူညီသော `min_compress_bytes_gpu` ကိုဖြတ်တောက်ထားချိန်တွင်ပင်၊
တိုက်ရိုက်ခေါ်သည် (ဥပမာ `norito::core::gpu_zstd::encode_all`) ဆိုတော့ သေးသေးလေးပါ။
GPU ရရှိနိုင်မှု မခွဲခြားဘဲ payload များသည် CPU လမ်းကြောင်းပေါ်တွင် အမြဲရှိနေပါသည်။

### ပြဿနာဖြေရှင်းခြင်းနှင့် တူညီမှုစစ်ဆေးခြင်းစာရင်း

- `cargo xtask acceleration-state --format json` ဖြင့် Snapshot runtime state ကို သိမ်းထားပါ။
  ပျက်ကွက်သည့်မှတ်တမ်းများနှင့်အတူ အထွက်နှုန်း၊ အစီရင်ခံစာသည် စီစဉ်သတ်မှတ်ထားသော/ရရှိနိုင်သည့် နောက်ခံများကို ပြသသည်။
  ပေါင်းစည်းခြင်း/နောက်ဆုံးအမှား လိုင်းများ။
- drift ကိုဖယ်ရှားရန် accel parity regression ကို စက်တွင်းတွင် ပြန်လည်လုပ်ဆောင်ပါ-
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  (CPU-only run ပြီး accel-on)။ ပြေးမှုအတွက် `acceleration_runtime_status()` မှတ်တမ်းတင်ပါ။
- backend သည် ကိုယ်တိုင်စမ်းသပ်မှုများ မအောင်မြင်ပါက၊ node အား CPU-only mode တွင် ထားရှိပါ (`enable_metal =
  false`, `enable_cuda = false`) နှင့် ဖမ်းယူထားသော ညီမျှခြင်းအထွက်နှင့်အတူ အဖြစ်အပျက်တစ်ခုကို ဖွင့်ပါ
  backend ကိုဖွင့်ခိုင်းမည့်အစား ရလဒ်များသည် မုဒ်များတစ်လျှောက်တွင် အဆုံးအဖြတ်အတိုင်း ရှိနေရပါမည်။
- **CUDA parity smoke (lab NV hardware):** Run
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  sm_8x ဟာ့ဒ်ဝဲပေါ်တွင်၊ `cargo xtask acceleration-state --format json` ကိုဖမ်းယူပြီး ပူးတွဲပါ။
  စံသတ်မှတ်ထားသောပစ္စည်းများအတွက် အခြေအနေလျှပ်တစ်ပြက်ပုံ (GPU မော်ဒယ်/ဒရိုက်ဗာ ပါ၀င်သည်)။