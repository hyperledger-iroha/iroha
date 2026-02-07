---
lang: my
direction: ltr
source: docs/source/benchmarks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a5420a123c456aad264ceb70d744b20b09848f7dca23700b4ee1370144bb57c
source_last_modified: "2025-12-29T18:16:35.920013+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# စံညွှန်းအစီရင်ခံစာ

လည်ပတ်မှုအလိုက် အသေးစိတ် လျှပ်တစ်ပြက်ရိုက်ချက်များနှင့် FASTPQ WP5-B မှတ်တမ်းကို တိုက်ရိုက်ထုတ်လွှင့်သည်။
[`benchmarks/history.md`](benchmarks/history.md); ပူးတွဲပါရှိသည့်အခါ အဆိုပါအညွှန်းကို အသုံးပြုပါ။
လမ်းပြမြေပုံသုံးသပ်ချက် သို့မဟုတ် SRE စာရင်းစစ်များအတွက် အထောက်အထားများ။ ၎င်းကို ပြန်ထုတ်ပါ။
GPU အသစ်ဖမ်းတိုင်း `python3 scripts/fastpq/update_benchmark_history.py`
သို့မဟုတ် Poseidon သည် မြေယာကို ထင်ရှားစေသည်။

## အရှိန်အဟုန် အထောက်အထား အတွဲ

GPU သို့မဟုတ် ရောစပ်မုဒ် စံနှုန်းတစ်ခုစီတိုင်းတွင် အသုံးပြုထားသော အရှိန်မြှင့်ဆက်တင်များ ပါဝင်ရပါမည်။
ထို့ကြောင့် WP6-B/WP6-C သည် အချိန်ကိုက်အနုပညာပစ္စည်းများနှင့်အတူ ဖွဲ့စည်းမှုဆိုင်ရာ တူညီမှုကို သက်သေပြနိုင်သည်။

- ပြေးချိန်တစ်ခုစီမတိုင်မှီ/ပြီးနောက် လျှပ်တစ်ပြက်ရိုက်ချက်ကို ဖမ်းယူပါ။
  `cargo xtask acceleration-state --format json > artifacts/acceleration_state_<stamp>.json`
  (လူသားဖတ်နိုင်သောမှတ်တမ်းများအတွက် `--format table` ကိုသုံးပါ။) ဤမှတ်တမ်းသည် `enable_{metal,cuda}`၊
  Merkle သတ်မှတ်ချက်များ၊ SHA-2 CPU ဘက်လိုက်မှု ကန့်သတ်ချက်များ၊ ရှာဖွေတွေ့ရှိထားသော နောက်ခံကျန်းမာရေးဘစ်များနှင့် မည်သည့်အရာမဆို၊
  ကပ်စေးကပ်နေသော တူညီမှုအမှားများ သို့မဟုတ် အကြောင်းပြချက်များကို ပိတ်ပါ။
- ထုပ်ပိုးထားသော စံနှုန်းရလဒ်ဘေးတွင် JSON ကို သိမ်းဆည်းပါ။
  (`artifacts/fastpq_benchmarks/*.json`၊ `benchmarks/poseidon/*.json`၊ Merkle လှည်း၊
  ဖမ်းယူမှုများ စသည်ဖြင့်) ထို့ကြောင့် သုံးသပ်သူများသည် အချိန်နှင့် ဖွဲ့စည်းမှုတို့ကို အတူတကွ ကွဲပြားနိုင်သည်။
- Knob အဓိပ္ပါယ်ဖွင့်ဆိုချက်များနှင့် ပုံသေများသည် `docs/source/config/acceleration.md` တွင် နေထိုင်ပါသည်။ ဘယ်အချိန်မှာ
  အစားထိုးမှုများကို အသုံးပြုသည် (ဥပမာ၊ `ACCEL_MERKLE_MIN_LEAVES_GPU`၊ `ACCEL_ENABLE_CUDA`)
  hosts များတစ်လျှောက် ပြန်လည်ထုတ်လုပ်နိုင်စေရန်အတွက် ၎င်းတို့ကို run metadata တွင် မှတ်သားထားပါ။

## Norito အဆင့်-၁ စံသတ်မှတ်ချက် (WP5-B/C)

- Command- `cargo xtask stage1-bench [--size <bytes|Nk|Nm>]... [--iterations <n>]`
  အရွယ်အစားအလိုက် အချိန်ဇယားများဖြင့် `benchmarks/norito_stage1/` အောက်တွင် JSON + Markdown ကို ထုတ်လွှတ်သည်
  scalar နှင့် အရှိန်မြှင့်ထားသော structural-index builder အတွက်။
- နောက်ဆုံးထွက်ပြေးမှုများ (macOS aarch64၊ dev ပရိုဖိုင်) တွင် တိုက်ရိုက်ထုတ်လွှင့်သည်။
  `benchmarks/norito_stage1/latest.{json,md}` နှင့် လတ်ဆတ်သော cutover CSV တို့မှ
  `examples/stage1_cutover` (`benchmarks/norito_stage1/cutover.csv`) SIMD ပြသည်
  ~6–8KiB မှစတင်၍ အနိုင်ရသည်။ GPU/parallel Stage-1 သည် ယခုအခါ **192KiB** သို့ ပုံသေသတ်မှတ်ထားသည်
  ပစ်လွှတ်ခြင်းကို ရှောင်ရှားရန် ဖြတ်တောက်ခြင်း (`NORITO_STAGE1_GPU_MIN_BYTES=<n>` ကို အစားထိုးရန်)
  ပိုကြီးသော payloads အတွက် accelerators ကိုဖွင့်ထားစဉ် စာရွက်စာတမ်းငယ်များပေါ်တွင်။

## Enum နှင့် Trait Object Dispatch

- Compile time (debug build): 16.58s
- Runtime (စံသတ်မှတ်ချက်၊ နိမ့်သည် ပိုကောင်းသည်)။
  - `enum`: 386 ps (ပျမ်းမျှ)
  - `trait_object`: 1.56 ns (ပျမ်းမျှ)

ဤတိုင်းတာမှုများသည် ဘောက်စ်ပုံစံအရာဝတ္တုကို အကောင်အထည်ဖော်ခြင်းနှင့် ပတ်သက်၍ enum-based dispatch ကို နှိုင်းယှဉ်သည့် microbenchmark မှ လာပါသည်။

## Poseidon CUDA သုတ်လိမ်းခြင်း။

Poseidon စံသတ်မှတ်ချက် (`crates/ivm/benches/bench_poseidon.rs`) တွင် ယခုအခါ single-hash permutations နှင့် batched helpers အသစ်များကို ကျင့်သုံးသည့် အလုပ်တာဝန်များ ပါဝင်သည်။ အစုံလိုက်ကို ဖွင့်ပါ-

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

စံသတ်မှတ်ချက်သည် `target/criterion/poseidon*_many` အောက်တွင် ရလဒ်များကို မှတ်တမ်းတင်ပါမည်။ GPU လုပ်သားတစ်ဦးကို ရနိုင်သည့်အခါ၊ JSON အနှစ်ချုပ်များကို တင်ပို့ပါ (ဥပမာ၊ `target/criterion/**/new/benchmark.json` ကို `benchmarks/poseidon/criterion_poseidon2_many_cuda.json` သို့ မိတ္တူကူးပါ) (ဥပမာ၊ `target/criterion/**/new/benchmark.json` ကို မိတ္တူကူးပြီး `benchmarks/poseidon/` သို့ ကူးယူပါ) သို့မှသာ downstream အဖွဲ့များသည် CPU နှင့် CUbatch တစ်ခုစီအတွက် အရွယ်အစားအားဖြင့် နှိုင်းယှဉ်နိုင်ပါသည်။ အထူးသီးသန့် GPU လမ်းကြောင်းကို တိုက်ရိုက်မလွှင့်မချင်း၊ စံညွှန်းသည် SIMD/CPU အကောင်အထည်ဖော်မှုသို့ ပြန်ကျသွားပြီး အတွဲလိုက်စွမ်းဆောင်ရည်အတွက် အသုံးဝင်သော ဆုတ်ယုတ်မှုဒေတာကို ထောက်ပံ့ပေးနေဆဲဖြစ်သည်။

ထပ်ခါတလဲလဲ ဖမ်းယူမှုများအတွက် (အချိန်ကိုက်ဒေတာဖြင့် တူညီသောအထောက်အထားများကို သိမ်းဆည်းရန်) ကို လုပ်ဆောင်ပါ။

```bash
cargo xtask poseidon-cuda-bench --json-out benchmarks/poseidon/poseidon_cuda_latest.json \
  --markdown-out benchmarks/poseidon/poseidon_cuda_latest.md --allow-overwrite
```မည်သည့်အစေ့မှ အဆုံးအဖြတ်ပေးသော Poseidon2/6 အသုတ်များ၊ CUDA ကျန်းမာရေး မှတ်တမ်းများ/ပိတ်ရခြင်းအကြောင်းများ၊ စစ်ဆေးမှုများ
scalar လမ်းကြောင်းနှင့် တူညီသည်၊ နှင့် Metal နှင့်အတူ ops/sec + speedup အနှစ်ချုပ်များကို ထုတ်လွှတ်သည်
runtime အခြေအနေ (အင်္ဂါရပ်အလံ၊ ရရှိနိုင်မှု၊ နောက်ဆုံးအမှား)။ CPU-only host များသည် scalar ကိုရေးသားဆဲဖြစ်သည်။
ပျောက်ဆုံးနေသော အရှိန်မြှင့်ကိရိယာကို ကိုးကားပြီး မှတ်သားပါ၊ ထို့ကြောင့် CI သည် GPU မပါဘဲပင် အနုပညာလက်ရာများကို ထုတ်ဝေနိုင်သည်။
အပြေးသမား။

## FASTPQ သတ္တုစံနှုန်း (Apple Silicon)

GPU လမ်းကြောသည် macOS 14 (arm64) တွင် အပ်ဒိတ်လုပ်ထားသော `fastpq_metal_bench` ၏ အဆုံးမှအဆုံးပြေးမှုအား လမ်းကြော-ဟန်ချက်ညီသော ဘောင်သတ်မှတ်မှု၊ ယုတ္တိအတန်း 20,000 (32,768 အထိ) နှင့် ကော်လံအုပ်စု 16 ခုကို ဖမ်းယူထားသည်။ ထုပ်ပိုးထားသော ပစ္စည်းသည် `artifacts/fastpq_benchmarks/fastpq_metal_bench_20k_refresh.json` တွင်တည်ရှိပြီး `traces/fastpq_metal_trace_*_rows20000_iter5.trace` အောက်တွင် ယခင်ရိုက်ကူးမှုများနှင့်အတူ သတ္တုခြေရာခံ သိမ်းဆည်းထားသည်။ ပျမ်းမျှအချိန်များ (`benchmarks.operations[*]` မှ) ယခုဖတ်ရသည်-

| စစ်ဆင်ရေး | CPU ဆိုသည်မှာ (ms) | သတ္တုဆိုလို (ms) | Speedup (x) |
|----------------|----------------|-----------------|----------------|
| FFT (32,768 သွင်းအားစု) | 83.29 | 79.95 | 1.04 |
| IFFT (32,768 သွင်းအားစု) | 93.90 | 78.61 | 1.20 |
| LDE (262,144 သွင်းအားစု) | 669.54 | 657.67 | 1.02 |
| Poseidon hash ကော်လံများ (524,288 inputs) | 29,087.53 | 30,004.90 | 0.97 |

လေ့လာတွေ့ရှိချက်များ-

- FFT/ IFFT နှစ်ခုစလုံးသည် ပြန်လည်ဆန်းသစ်ထားသော BN254 kernels မှ အကျိုးခံစားခွင့်များ (IFFT သည် ယခင်ဆုတ်ယုတ်မှုကို ~20%) ဖြင့် ရှင်းလင်းစေသည်။
- LDE သည် တန်းတူညီမျှမှုအနီးတွင် ရှိနေသည်။ ယခု သုည-ဖြည့်စွက်သည် 33,554,432 padded bytes 18.66ms ပျမ်းမျှဖြင့် မှတ်တမ်းတင်ထားသောကြောင့် JSON အစုအဝေးသည် တန်းစီခြင်းကို ဖမ်းယူပါသည်။
- Poseidon hashing သည် ဤဟာ့ဒ်ဝဲပေါ်တွင် CPU-ချည်နှောင်နေဆဲဖြစ်သည်။ သတ္တုလမ်းကြောင်းသည် နောက်ဆုံးပေါ်တန်းစီထိန်းချုပ်မှုများကို လက်ခံကျင့်သုံးသည့်တိုင်အောင် Poseidon microbench နှင့် နှိုင်းယှဉ်ကြည့်ပါ။
- ယခုဖမ်းယူမှုတစ်ခုစီသည် `AccelerationSettings.runtimeState().metal.lastError` ကို မှတ်တမ်းတင်ထားသည်။
  အင်ဂျင်နီယာများသည် တိကျသော disable အကြောင်းပြချက်ဖြင့် CPU အမှားများကို သရုပ်ဖော်သည် (မူဝါဒခလုတ်၊
  တူညီမှု ချို့ယွင်းချက်၊ စက်ပစ္စည်းမရှိပါ) အခြေခံစံအမှတ်အသားတွင် တိုက်ရိုက်။

လည်ပတ်မှုကို ပြန်လည်ထုတ်လုပ်ရန်၊ သတ္တုစေ့များကို တည်ဆောက်ပြီး လုပ်ဆောင်ပါ-

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 --output fastpq_metal_bench_20k.json
```

`artifacts/fastpq_benchmarks/` အောက်တွင် ထွက်ပေါ်လာသော JSON ကို Metal trace ဖြင့် တွဲ၍ အဆုံးအဖြတ်ပေးသော အထောက်အထားများကို မျိုးပွားနိုင်စေပါသည်။

## FASTPQ CUDA အလိုအလျောက်စနစ်

CUDA host များသည် SM80 စံနှုန်းကို အဆင့်တစ်ဆင့်ဖြင့် လုပ်ဆောင်နိုင်ပြီး၊

```bash
cargo xtask fastpq-cuda-suite \
  --rows 20000 --iterations 5 --columns 16 \
  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \
  --label device_class=xeon-rtx --device rtx-ada
```

အကူအညီပေးသူက `fastpq_cuda_bench`၊ အညွှန်းများ/စက်/မှတ်စုများ၊ ဂုဏ်ထူးဆောင်များမှတဆင့် စာသားများကို ခေါ်ဆိုသည်
`--require-gpu` နှင့် `scripts/fastpq/wrap_benchmark.py` မှတဆင့် (မူလအားဖြင့်) ထုပ်ပိုးခြင်း/လက္ခဏာများ။
အထွက်များတွင် အကြမ်း JSON၊ `artifacts/fastpq_benchmarks/` အောက်တွင် ထုပ်ပိုးထားသော အတွဲ၊
အတိအကျ commands/env ကိုမှတ်တမ်းတင်သော output ဘေးတွင် `<name>_plan.json`
အဆင့် 7 ဖမ်းယူမှုများကို GPU အပြေးသမားများတွင် ပြန်လည်ထုတ်လုပ်နိုင်သည်။ `--sign-output` နှင့် ထည့်ပါ။
လက်မှတ်များလိုအပ်သည့်အခါ `--gpg-key <id>`၊ ထုတ်လွှတ်ရန် `--dry-run` ကိုသုံးပါ။
ခုံတန်းလျားကို မလုပ်ဆောင်ဘဲ အစီအစဉ်/လမ်းကြောင်းများ။

### GA ဖမ်းယူမှု (macOS 14 arm64၊ လမ်းသွား-ဟန်ချက်ညီသော)

WP2-D အား ကျေနပ်စေရန်အတွက် GA-ready နှင့် တူညီသော host ပေါ်တွင် ထွက်ရှိသည့် တည်ဆောက်မှုကိုလည်း မှတ်တမ်းတင်ထားပါသည်။
တန်းစီခြင်းဆိုင်ရာ heuristics အဖြစ်ထုတ်ဝေခဲ့သည်။
`fastpq_metal_bench_20k_release_macos14_arm64.json`။ ပစ္စည်းနှစ်ခုကို ဖမ်းယူထားသည်။
ကော်လံအတွဲများ (လမ်းသွား-ဟန်ချက်ညီသော၊ အတန်း ၃၂,၇၆၈ တန်းအထိ) နှင့် Poseidon ပါ၀င်သည်
ဒိုင်ခွက်သုံးစွဲမှုအတွက် microbench နမူနာများ။| စစ်ဆင်ရေး | CPU ဆိုသည်မှာ (ms) | သတ္တုဆိုလို (ms) | အရှိန်မြှင့် | မှတ်စုများ |
|-----------|----------------|-----------------|---------|--------|
| FFT (32,768 သွင်းအားစု) | 12.741 | 10.963 | 1.16× | GPU kernels များသည် ပြန်လည်ဆန်းသစ်ထားသော တန်းစီခြင်းအဆင့်များကို ခြေရာခံသည်။ |
| IFFT (32,768 သွင်းအားစု) | 17.499 | 25.688 | 0.68× | ကွန်ဆာဗေးတစ် လူတန်းစား ပရိတ်သတ်-ထွက်ခြင်းသို့ ခြေရာခံ ဆုတ်ယုတ်မှု။ heuristics ကို ဆက်ထိန်းပါ။ |
| LDE (262,144 သွင်းအားစု) | 68.389 | 65,701 | 1.04× | အသုတ်နှစ်ခုလုံးအတွက် 9.651ms တွင် 33,554,432 bytes ကို သုညဖြင့် ဖြည့်သွင်းသည်။ |
| Poseidon hash ကော်လံများ (524,288 inputs) | 1,728.835 | 1,447.076 | 1.19× | Poseidon တန်းစီခြင်းကို မြှင့်တင်ပြီးနောက် GPU သည် CPU ကို နောက်ဆုံးတွင် ကျော်တက်သွားသည်။ |

JSON တွင် ထည့်သွင်းထားသော Poseidon microbench တန်ဖိုးများသည် 1.10× မြန်နှုန်းမြှင့်ခြင်း (မူလလမ်းကြောင်းဖြစ်သည်
596.229ms နှင့် scalar 656.251ms နှင့် ထပ်ကာထပ်ကာ ငါးကြိမ်)၊ ထို့ကြောင့် ဒက်ရှ်ဘုတ်များသည် ယခုဇယားကွက်ကို လုပ်နိုင်ပါပြီ။
ပင်မခုံတန်းလျားများနှင့်အတူ တစ်လမ်းသွား တိုးတက်မှုများ။ ပြေးခြင်းကို ပြန်ထုတ်သည်-

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 \
  --output fastpq_metal_bench_20k_release_macos14_arm64.json
```

ထုပ်ပိုးထားသော JSON နှင့် `FASTPQ_METAL_TRACE_CHILD=1` ခြေရာများကို အောက်တွင် သိမ်းဆည်းထားပါ
`artifacts/fastpq_benchmarks/` ထို့ကြောင့် နောက်ဆက်တွဲ WP2-D/WP2-E သုံးသပ်ချက်များသည် GA နှင့် ကွဲပြားနိုင်သည်
အလုပ်ချိန်ကို ပြန်မလုပ်ဆောင်ဘဲ အစောပိုင်း refresh run များကို ဖမ်းယူပါ။

လတ်ဆတ်သော `fastpq_metal_bench` တစ်ခုစီကို ယခုဖမ်းယူရာတွင်လည်း `bn254_metrics` ဘလောက်တစ်ခုရေးသည်၊
CPU အတွက် `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` entries တွေကို ဖော်ထုတ်ပေးတယ်။
အခြေခံလိုင်းနှင့် မည်သည့် GPU နောက်ကွယ်တွင်မဆို (Metal/CUDA) တက်ကြွနေသည်၊ **နှင့်** a
`bn254_dispatch` ဘလောက်သည် လေ့လာထားသော threadgroup widths၊ logical thread
ကော်လံတစ်ခုတည်း BN254 FFT/LDE ပေးပို့မှုများအတွက် အရေအတွက်များနှင့် ပိုက်လိုင်းကန့်သတ်ချက်များ။ ဟိ
benchmark wrapper သည် မြေပုံနှစ်ခုလုံးကို `benchmarks.bn254_*` သို့ကူးယူသည်၊ ထို့ကြောင့် ဒိုင်ခွက်များနှင့်
Prometheus တင်ပို့သူများသည် ပြန်လည်ခွဲခြမ်းစိတ်ဖြာခြင်းမရှိဘဲ အညွှန်းတပ်ထားသော latency နှင့် ဂျီသြမေတြီကို ခြစ်နိုင်သည်
ကုန်ကြမ်းစစ်ဆင်ရေး array ။ `FASTPQ_METAL_THREADGROUP` သည် ယခုအခါတွင် ထပ်လောင်းအကျုံးဝင်ပါသည်။
BN254 kernels ကိုလည်း ချည်မျှင်အစုအဝေးများကို တစ်ခုမှ ပြန်ထုတ်ပေးနိုင်သည်။ ခလုတ်။

အောက်ခြေ ဒက်ရှ်ဘုတ်များကို ရိုးရှင်းစေရန် `python3 scripts/benchmarks/export_csv.py` ကို အသုံးပြုပါ။
အစုအဝေးတစ်ခုဖမ်းပြီးနောက်။ အကူအညီပေးသူက `poseidon_microbench_*.json` ကို ပြားချပ်စေပါသည်။
`.csv` ဖိုင်များနှင့် ကိုက်ညီသောကြောင့် အလိုအလျောက်စနစ်ဆိုင်ရာ အလုပ်များသည် ပုံသေနှင့် scalar လမ်းများ ကွဲပြားနိုင်သည်
စိတ်ကြိုက်ခွဲခြမ်းစိတ်ဖြာမှုများ။

## Poseidon microbench (သတ္တု)

ယခု `fastpq_metal_bench` သည် `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` အောက်တွင် သူ့ကိုယ်သူ ပြန်လည်လုပ်ဆောင်ပြီး အချိန်များကို `benchmarks.poseidon_microbench` သို့ မြှင့်တင်ပေးပါသည်။ ကျွန်ုပ်တို့သည် နောက်ဆုံးထွက်သတ္တုဖမ်းယူမှုများကို `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <wrapped_json>` ဖြင့် တင်ပို့ပြီး `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json` မှတစ်ဆင့် စုစည်းထားသည်။ အောက်ပါအကျဉ်းချုပ်များသည် `benchmarks/poseidon/` အောက်တွင် တိုက်ရိုက်ထုတ်လွှင့်သည်-

| အနှစ်ချုပ် | အတွဲလိုက် | ပုံသေဆိုလို (ms) | Scalar mean (ms) | Speedup vs scalar | ကော်လံ x ပြည်နယ်များ | ပါတယ်လို့ |
|---------|----------------|--------------------|------------------|--------------------------------|--------------------------------|----------------|
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 1,990.49 | 1,994.53 | 1.002 | 64 x 262,144 | 5 |
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2,167.66 | 2,152.18 | 0.993 | 64 x 262,144 | 5 |ဖမ်းယူမှုနှစ်ခုစလုံးသည် ပြေးနှုန်းတစ်ခုလျှင် 262,144 ပြည်နယ်များ (trace log2 = 12) ကို သွေးပူမှုတစ်ကြိမ်ပြုလုပ်ခြင်းဖြင့် hash လုပ်ထားသည်။ "မူလ" လမ်းကြောသည် ညှိထားသော multi-state kernel နှင့် သက်ဆိုင်ပြီး "scalar" သည် နှိုင်းယှဉ်ရန်အတွက် လမ်းသွားတစ်ခုစီတွင် kernel ကို လော့ခ်ချထားသည်။

## Merkle တံခါးပေါက်သည် အဟုန်ပေါက်သည်။

`merkle_threshold` ဥပမာ (`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`) သည် Metal-vs-CPU Merkle hashing လမ်းကြောင်းများကို အလေးပေးဖော်ပြသည်။ နောက်ဆုံးထွက် AppleSilicon ဖမ်းယူမှု (Darwin 25.0.0 arm64, `ivm::metal_available()=true`) သည် ကိုက်ညီသော CSV ထုတ်ယူမှုဖြင့် `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` တွင် နေထိုင်ပါသည်။ CPU သီးသန့် macOS 14 အခြေခံလိုင်းများသည် Metal မပါဘဲ host အတွက် `benchmarks/merkle_threshold/macos14_arm64_{cpu,metal}.json` အောက်တွင် ရှိနေပါသည်။

| အရွက် | CPU အကောင်းဆုံး (ms) | သတ္တုအကောင်းဆုံး (ms) | အရှိန်မြှင့် |
|--------|----------------|-----------------|---------|
| 1,024 | 23.01 | 19.69 | 1.17× |
| 4,096 | 50.87 | 62.12 | 0.82× |
| 8.192 | 95.77 | 96.57 | 0.99× |
| 16,384 | 64.48 | 58.98 | 1.09× |
| 32,768 | 109.49 | 87.68 | 1.25× |
| 65,536 | 177.72 | 137.93 | 1.29× |

ပိုကြီးသောအရွက်များသည် Metal (1.09–1.29×); သေးငယ်သောပုံးများသည် CPU တွင် ပိုမိုမြန်ဆန်စွာလည်ပတ်နေသေးသောကြောင့် CSV သည် ခွဲခြမ်းစိတ်ဖြာရန်အတွက် ကော်လံနှစ်ခုလုံးကို သိမ်းဆည်းထားသည်။ GPU နှင့် CPU ဆုတ်ယုတ်မှု ဒက်ရှ်ဘုတ်များကို ချိန်ညှိထားရန် ပရိုဖိုင်တစ်ခုစီဘေးရှိ `metal_available` အလံကို CSV ကူညီသူမှ ထိန်းသိမ်းထားသည်။

မျိုးပွားခြင်း အဆင့်များ-

```bash
cargo run --release -p ivm --features metal --example merkle_threshold -- --json \
  > benchmarks/merkle_threshold/<hostname>_$(uname -r)_$(uname -m).json
```

လက်ခံဆောင်ရွက်ပေးသူသည် တိကျပြတ်သားသော Metal ဖွင့်ထားရန် လိုအပ်ပါက `FASTPQ_METAL_LIB`/`FASTPQ_GPU` ကို သတ်မှတ်ပြီး CPU + GPU ဖမ်းယူမှု နှစ်ခုစလုံးကို စာရင်းသွင်းထားပါက WP1-F သည် မူဝါဒသတ်မှတ်ချက်များကို ဇယားကွက်ရေးဆွဲနိုင်ပါသည်။

ဦးခေါင်းမပါသော အခွံမှ အလုပ်လုပ်သောအခါ၊ `IVM_DEBUG_METAL_ENUM=1` ကို ကိရိယာစာရင်းကောက်ယူခြင်းကို မှတ်တမ်းတင်ရန်နှင့် `MTLCreateSystemDefaultDevice()` ကို ကျော်ဖြတ်ရန် `IVM_FORCE_METAL_ENUM=1` ကို သတ်မှတ်ပါ။ CLI သည် မူရင်း Metal စက်ပစ္စည်းကို မတောင်းမီ ** မတိုင်မီ ** Core Graphics စက်ရှင်ကို ပူနွေးစေပြီး `MTLCopyAllDevices()` မှ သုညသို့ ပြန်သွားသောအခါ `MTLCreateSystemDefaultDevice()` သို့ ပြန်သွားသည် ။ အကယ်၍ လက်ခံသူသည် မည်သည့်စက်ပစ္စည်းကိုမျှ သတင်းမပို့သေးပါက၊ ဖမ်းယူမှုသည် `metal_available=false` (အသုံးဝင်သော CPU အခြေခံလိုင်းများကို `macos14_arm64_*` အောက်တွင် နေထိုင်သည်) ကို ဆက်လက်ထိန်းသိမ်းထားမည်ဖြစ်ပြီး GPU တန်ဆာပလာများသည် `FASTPQ_GPU=metal` ကို ဆက်လက်ဖွင့်ထားသင့်သောကြောင့် အစုအဝေးသည် ရွေးချယ်ထားသော နောက်ခံဖိုင်ကို မှတ်တမ်းတင်ထားသည်။

`fastpq_metal_bench` သည် `FASTPQ_DEBUG_METAL_ENUM=1` မှတစ်ဆင့် အလားတူအဖုတစ်ခုကို ဖော်ထုတ်ပေးသည်၊ ၎င်းသည် `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` ရလဒ်များကို GPU လမ်းကြောင်းပေါ်တွင် ဆက်နေရန် မဆုံးဖြတ်မီ နောက်ခံမှ ထုတ်ပေးသည်။ `FASTPQ_GPU=gpu` သည် ပတ်ထားသော JSON တွင် `backend="none"` ကို ဆက်လက်တင်ပြသည့်အခါတိုင်း ၎င်းကိုဖွင့်ထားသောကြောင့် ဖမ်းယူထားသည့်အစုအဝေးသည် သတ္တုဟတ်ဝဲအား စာရင်းကောက်ယူသည့် host မှ အတိအကျ မှတ်တမ်းတင်ပါသည်။ `FASTPQ_GPU=gpu` ကို သတ်မှတ်လိုက်သောအခါတွင် ကြိုးသည် ချက်ချင်းပျက်သွားသော်လည်း အရှိန်မြှင့်စက်ကို ရှာမတွေ့ပါ၊ အမှားရှာအနှိပ်ခလုတ်ကို ညွှန်ပြသောကြောင့် ထုတ်လွှတ်မှုအစုအဝေးသည် အတင်းအကြပ် GPU နောက်ကွယ်တွင် CPU အမှားကို ဘယ်သောအခါမှ မဝှက်ထားပေ။ ပြေးသည်။【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】

CSV အကူအညီပေးသူက တစ်ပရိုဖိုင်ဇယားများ (ဥပမာ `macos14_arm64_*.csv` နှင့် `takemiyacStudio.lan_25.0.0_arm64.csv`) ကို ထုတ်လွှတ်သည်) `metal_available` အလံကို ထိန်းသိမ်းထားသောကြောင့် ဆုတ်ယုတ်မှု ဒက်ရှ်ဘုတ်များသည် စိတ်ကြိုက်ခွဲခြမ်းစိတ်ဖြာခြင်းမရှိဘဲ CPU နှင့် GPU တိုင်းတာမှုများကို ထည့်သွင်းနိုင်သည်။