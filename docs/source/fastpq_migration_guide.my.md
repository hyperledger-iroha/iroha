---
lang: my
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-05T09:28:12.006936+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! FASTPQ ထုတ်လုပ်မှု ရွှေ့ပြောင်းခြင်းလမ်းညွှန်

ဤစာအုပ်တွင် Stage6 ထုတ်လုပ်မှု FASTPQ သက်သေကို မည်သို့အတည်ပြုရမည်ကို ဖော်ပြထားပါသည်။
ဤရွှေ့ပြောင်းနေထိုင်မှုအစီအစဉ်၏ တစ်စိတ်တစ်ပိုင်းအဖြစ် သတ်မှတ်သတ်မှတ်ထားသော နေရာယူထားသည့် နောက်ခံအစွန်းကို ဖယ်ရှားခဲ့သည်။
၎င်းသည် `docs/source/fastpq_plan.md` ရှိ အဆင့်သတ်မှတ်ထားသော အစီအစဉ်ကို ဖြည့်စွက်ပြီး သင်ခြေရာခံနေပြီဟု ယူဆပါသည်။
`status.md` ရှိ အလုပ်ခွင်အခြေအနေ။

## ပရိသတ်နှင့် နယ်ပယ်
- မှန်ကန်သောအော်ပရေတာများသည် ထုတ်လုပ်မှုသက်သေကို ဇာတ်ညွှန်း သို့မဟုတ် mainnet ပတ်၀န်းကျင်တွင် ဖြန့်ချိသည်။
- ထုတ်လုပ်မှုနောက်ကွယ်မှပို့ဆောင်မည့် binaries သို့မဟုတ် containers များကိုဖန်တီးသောအင်ဂျင်နီယာများကို ဖြန့်ချိပါ။
- SRE/ကြည့်ရှုနိုင်မှုအဖွဲ့များသည် တယ်လီမီတာအချက်ပြမှုများနှင့် သတိပေးချက်အသစ်များကို ကြိုးပေးသည်။

နယ်ပယ်ပြင်ပ- Kotodama စာချုပ်ရေးသားခြင်းနှင့် IVM ABI အပြောင်းအလဲများ (အတွက် `docs/source/nexus.md` ကိုကြည့်ပါ
အကောင်အထည်ဖော်မှုပုံစံ)။

## Feature Matrix
| မဂ် | ကုန်တင်အင်္ဂါရပ်များကိုဖွင့် | ရလဒ် | ဘယ်အချိန်မှာသုံးရမလဲ |
| ----| ----------------------- | ------| ----------- |
| Production prover (မူရင်း) | _none_ | အဆင့်၆ FASTPQ နောက်ကွယ်မှ FFT/LDE အစီအစဉ်ရေးဆွဲသူနှင့် DEEP-FRI ပိုက်လိုင်းပါရှိသည်။ 【crates/fastpq_prover/src/backend.rs:1144】 | ထုတ်လုပ်မှု binaries အားလုံးအတွက် မူရင်း။ |
| ရွေးချယ်နိုင်သော GPU အရှိန်မြှင့်ခြင်း | `fastpq_prover/fastpq-gpu` | CUDA/Metal kernels များကို အလိုအလျောက် CPU လှည့်စားခြင်းဖြင့် ဖွင့်ပေးသည်။【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | ပံ့ပိုးထားသော အရှိန်မြှင့်စက်များဖြင့် လက်ခံဆောင်ရွက်ပေးသည်။ |

## တည်ဆောက်မှုလုပ်ငန်းစဉ်
1. **CPU သီးသန့်တည်ဆောက်မှု**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   ထုတ်လုပ်မှုနောက်ခံကို မူရင်းအတိုင်း စုစည်းထားသည်။ အပိုအင်္ဂါရပ်များမလိုအပ်ပါ။

2. **GPU-ဖွင့်ထားသော တည်ဆောက်မှု (ချန်လှပ်ထားနိုင်သည်)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU ပံ့ပိုးမှုသည် တည်ဆောက်နေစဉ်အတွင်း ရရှိနိုင်သော `nvcc` ပါသော SM80+ CUDA ကိရိယာအစုံ လိုအပ်ပါသည်။ 【crates/fastpq_prover/Cargo.toml:11】

၃။ **မိမိကိုယ်ကို စမ်းသပ်ခြင်း**
   ```bash
   cargo test -p fastpq_prover
   ```
   ထုပ်ပိုးခြင်းမပြုမီ Stage6 လမ်းကြောင်းကို အတည်ပြုရန် ထုတ်ဝေမှုတစ်ခုလျှင် တစ်ကြိမ်လုပ်ဆောင်ပါ။

### သတ္တုတူးလ်ကွင်းဆက်ပြင်ဆင်မှု (macOS)
1. မတည်ဆောက်မီ Metal command-line ကိရိယာများကို တပ်ဆင်ပါ- `xcode-select --install` (CLI ကိရိယာများ ပျောက်ဆုံးနေပါက) နှင့် GPU တူးလ်ကွင်းဆက်ကို ရယူရန် `xcodebuild -downloadComponent MetalToolchain`။ build script သည် `xcrun metal`/`xcrun metallib` ကို တိုက်ရိုက်ခေါ်ပြီး binaries များမရှိပါက လျင်မြန်စွာ ပျက်သွားမည်။【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. CI မတိုင်မီ ပိုက်လိုင်းကို အတည်ပြုရန်၊ တည်ဆောက်မှု script ကို စက်တွင်းတွင် ထင်ဟပ်နိုင်သည်-
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   တည်ဆောက်မှုအောင်မြင်သောအခါ `FASTPQ_METAL_LIB=<path>` ထွက်လာသည်။ runtime သည် metallib ကို အဆုံးအဖြတ်ပေးသော load ရန် ထိုတန်ဖိုးကို ဖတ်သည်။【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. Metal toolchain မပါဘဲ cross-compiling လုပ်သောအခါ `FASTPQ_SKIP_GPU_BUILD=1` ကို သတ်မှတ်ပါ။ build သည် သတိပေးချက်ကို print ထုတ်ပြီး စီစဉ်သူသည် CPU လမ်းကြောင်းပေါ်တွင် ရှိနေပါသည်။【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Metal မရရှိနိုင်ပါက Nodes များသည် CPU သို့ အလိုအလျောက်ပြန်ကျသွားသည် (ဘောင်ဘောင်ပျောက်နေသော၊ ပံ့ပိုးမထားသော GPU၊ သို့မဟုတ် `FASTPQ_METAL_LIB` ဗလာ)၊ build script သည် env var ကိုရှင်းလင်းစေပြီး အစီအစဉ်ဆွဲသူက အဆင့်နှိမ့်ပေးပါသည်။【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】### စာရင်းကို ထုတ်ပြန်ရန် (အဆင့် ၆)
အောက်ဖော်ပြပါအရာတိုင်း ပြီးပြည့်စုံပြီး ပူးတွဲပါရှိသည်အထိ FASTPQ ထုတ်ဝေခွင့်လက်မှတ်ကို ပိတ်ဆို့ထားပါ။

1. **ဒုတိယ သက်သေ တိုင်းတာချက်များ** — လတ်လတ်ဆတ်ဆတ် ဖမ်းယူထားသော `fastpq_metal_bench_*.json` ကို စစ်ဆေးပါ နှင့်
   `benchmarks.operations` နေရာတွင် `operation = "lde"` ကိုအတည်ပြုပါ (နှင့် mirrored
   `report.operations` နမူနာ) `gpu_mean_ms ≤ 950` အစီရင်ခံသည် 20000 အတန်းအလုပ်ဝန် (32768 padded
   အတန်းများ)။ မျက်နှာကျက်အပြင်ဘက်တွင် ဖမ်းယူမှုများကို စစ်ဆေးရန်စာရင်းကို လက်မှတ်မထိုးမီ ပြန်လည်လုပ်ဆောင်ရန် လိုအပ်သည်။
2. **လက်မှတ်ထိုးထားသော မန်နီးဖက်စ်** — Run
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   ထို့ကြောင့် ထုတ်ဝေခွင့်လက်မှတ်တွင် မန်နီးဖက်စ်နှင့် ၎င်း၏ သီးခြားလက်မှတ်ပါရှိသည်။
   (`artifacts/fastpq_bench_manifest.sig`)။ သုံးသပ်သူများသည် ရှေ့မှောက်တွင် digest/signature အတွဲကို အတည်ပြုပါသည်။
   ထုတ်ဝေမှုကို မြှင့်တင်နေသည်။【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 မက်ထရစ် သရုပ်ဖော်ပုံ (တည်ဆောက်ထားသည်
   `scripts/fastpq/capture_matrix.sh`) မှတစ်ဆင့် 20k အတန်းကြမ်းပြင်ကို ကုဒ်လုပ်ထားပြီး၊
   ဆုတ်ယုတ်မှုကို အမှားပြင်ခြင်း
3. **အထောက်အထား ပူးတွဲပါဖိုင်များ** — သတ္တုစံနှုန်း JSON၊ stdout မှတ်တမ်း (သို့မဟုတ် တူရိယာခြေရာခံ) ကို အပ်လုဒ်လုပ်ပါ။
   CUDA/Metal manifest outputs နှင့် ထုတ်ဝေခွင့်လက်မှတ်အတွက် သီးခြားလက်မှတ်။ စစ်ဆေးရန်စာရင်းကို ထည့်သွင်းပါ။
   လက်မှတ်ရေးထိုးခြင်းအတွက် အသုံးပြုသည့် အများသူငှာသော့လက်ဗွေနှင့် လက်ဗွေလက်ရာများအားလုံးကို လင့်ခ်ချိတ်သင့်သည်၊ ထို့ကြောင့် downstream စာရင်းစစ်များ
   အတည်ပြုခြင်းအဆင့်ကို ပြန်ဖွင့်နိုင်သည်။【artifacts/fastpq_benchmarks/README.md:65】### သတ္တုအတည်ပြုခြင်းလုပ်ငန်းအသွားအလာ
1. GPU-enabled build တစ်ခုပြီးနောက်၊ `FASTPQ_METAL_LIB` အမှတ် `.metallib` (`echo $FASTPQ_METAL_LIB`) တွင် အတည်ပြုပါ၊ သို့မှသာ runtime သည် ၎င်းကို အဆုံးအဖြတ်ပေးနိုင်သည်။【crates/fastpq_prover/build.rs:188】
2. GPU လမ်းကြောင်းများဖြင့် parity suite ကို ဖွင့်ပါ-\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`။ ထောက်လှမ်းမှု မအောင်မြင်ပါက နောက်ခံသည် သတ္တု kernels များကို ကျင့်သုံးပြီး တိကျသော CPU လှည့်ကွက်ကို မှတ်တမ်းတင်ပါမည်။ 【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. ဒက်ရှ်ဘုတ်များအတွက် စံနမူနာတစ်ခုကို ရိုက်ကူးပါ-\
   စုစည်းထားသော သတ္တုစာကြည့်တိုက် (`fd -g 'fastpq.metallib' target/release/build | head -n1`) ကိုရှာပါ။
   ၎င်းကို `FASTPQ_METAL_LIB` မှတစ်ဆင့် ထုတ်ယူပြီး run\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`။
  Canonical `fastpq-lane-balanced` ပရိုဖိုင်သည် ယခုဖမ်းယူမှုတိုင်းကို 32,768 အတန်း (2¹⁵) အထိ မြှင့်တင်ပေးသောကြောင့် JSON သည် `rows` နှင့် `padded_rows` နှစ်ခုလုံးကို Metal LDE latency နှင့်အတူ သယ်ဆောင်သည်။ `zero_fill` သို့မဟုတ် တန်းစီဆက်တင်များ AppleM-series host များပေါ်တွင် ပစ်မှတ်ထားသည့် GPU LDE ကို 950ms (<1s) ထက်ကျော်လွန်ပါက ဖမ်းယူမှုကို ပြန်လည်လုပ်ဆောင်ပါ။ အခြားထွက်ရှိထားသော အထောက်အထားများနှင့်အတူ ရရှိလာသော JSON/log ကို သိမ်းဆည်းပါ။ ညစဉ် macOS အလုပ်အသွားအလာသည် တူညီသောလည်ပတ်မှုကို လုပ်ဆောင်ပြီး နှိုင်းယှဉ်ရန်အတွက် ၎င်း၏လက်ရာများကို အပ်လုဒ်လုပ်ပါသည်။【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  Poseidon-only telemetry (ဥပမာ၊ တူရိယာခြေရာခံတစ်ခုကို မှတ်တမ်းတင်ရန်) လိုအပ်သောအခါ၊ အထက်ပါ command သို့ `--operation poseidon_hash_columns` ကို ထည့်ပါ။ ခုံတန်းလျားသည် `FASTPQ_GPU=gpu` ကို လေးစားဆဲဖြစ်ပြီး၊ `metal_dispatch_queue.poseidon` ကို ထုတ်လွှတ်ကာ `poseidon_profiles` ဘလောက်အသစ်ကို ထည့်သွင်းထားသောကြောင့် ထုတ်လွှတ်မှုအစုအဝေးတွင် Poseidon တစ်ဆို့ခြင်းကို အတိအလင်း မှတ်တမ်းတင်ထားသည်။
  ယခုအခါ အထောက်အထားများတွင် `zero_fill.{bytes,ms,queue_delta}` နှင့် `kernel_profiles` (kernel တစ်ခုလျှင်
  နေထိုင်မှု၊ ခန့်မှန်းခြေ GB/s နှင့် ကြာချိန် ကိန်းဂဏာန်းများ) ထို့ကြောင့် GPU ၏ စွမ်းဆောင်ရည်ကို ဂရပ်ဖစ်မပါဘဲ ပုံဖော်နိုင်သည်။
  ကုန်ကြမ်းအစအနများကို ပြန်လည်လုပ်ဆောင်ခြင်းနှင့် `twiddle_cache` ပိတ်ဆို့ခြင်း ( hits/misses + `before_ms`/`after_ms`)
  ကက်ရှ် twiddle အပ်လုဒ်များသည် အကျိုးသက်ရောက်မှုရှိကြောင်း သက်သေပြသည်။ `--trace-dir` သည် အောက်ရှိကြိုးကို ပြန်လည်စတင်သည်။
  `xcrun xctrace record` နှင့်
  JSON နှင့်အတူ အချိန်တံဆိပ်ရိုက်ထားသော `.trace` ဖိုင်ကို သိမ်းဆည်းပါ။ သင်စိတ်ကြိုက်တစ်ခု ပေးနိုင်သေးသည်။
  ရိုက်ကူးသည့်အခါ `--trace-output` (ရွေးချယ်နိုင်သော `--trace-template` / `--trace-seconds`)
  စိတ်ကြိုက်တည်နေရာ/ပုံစံ စာရင်းစစ်အတွက် JSON သည် `metal_trace_{template,seconds,output}` ကို မှတ်တမ်းတင်ပါသည်။ 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】ဖမ်းယူမှုတစ်ခုစီသည် `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` ကို run ပြီးနောက် ထုတ်ဝေမှုသည် `metadata.metal_trace` အပါအဝင် Grafana ဘုတ်အဖွဲ့/သတိပေးချက်အစုအဝေး (`dashboards/grafana/fastpq_acceleration.json`၊ `dashboards/alerts/fastpq_acceleration_rules.yml`) အတွက် မက်တာဒေတာကို သယ်ဆောင်ပါသည်။ ယခု အစီရင်ခံစာသည် လုပ်ဆောင်ချက်တစ်ခုလျှင် `speedup` အရာဝတ္ထု (`speedup.ratio`၊ `speedup.delta_ms`)၊ wrapper hoists `zero_fill_hotspots` (bytes၊ latency၊ GB/s မှဆင်းသက်လာပြီး Metal queue 100010NI) သို့ ကောင်တာများသို့ သယ်ဆောင်ပါသည်။ `benchmarks.kernel_summary`၊ `twiddle_cache` ဘလောက်ကို နဂိုအတိုင်းထားကာ၊ `post_tile_dispatches` ဘလောက်/အကျဉ်းချုပ်အသစ်ကို မိတ္တူကူးထားသောကြောင့် သုံးသပ်သူများသည် ဖမ်းယူစဉ်အတွင်း multi-pass kernel ကို သက်သေပြနိုင်ပြီး၊ ယခု ကိုးကားထားသော Poseidon microbench အထောက်အထားများကို အကျဉ်းချုပ် 700008NI သို့ လုပ်ဆောင်နိုင်ပြီဖြစ်သည်။ အစီရင်ခံစာကြမ်းကို ပြန်လည်မသုံးသပ်ဘဲ scalar-vs-default latency။ manifest gate သည် တူညီသောပိတ်ဆို့ခြင်းကိုဖတ်ပြီး ၎င်းကိုချန်လှပ်ထားသည့် GPU အထောက်အထားအစုအဝေးများကို ပယ်ချပြီး အော်ပရေတာများအား ကြွေပြားခင်းသည့်လမ်းကြောင်းကို ကျော်သွားသည့်အခါတိုင်း သို့မဟုတ် အော်ပရေတာများအား ဖမ်းယူမှုများကို ပြန်လည်ဆန်းသစ်ခိုင်းစေပါသည်။ မှားယွင်းသတ်မှတ်ထားသည်။【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq】
  Poseidon2 Metal kernel သည် တူညီသော အဖုများ မျှဝေပါသည်- `FASTPQ_METAL_POSEIDON_LANES` (32–256၊ powers နှစ်ခု) နှင့် `FASTPQ_METAL_POSEIDON_BATCH` (1-32 states per lane) သည် သင့်အား launch width နှင့် per-lane work ကို ပြန်လည်တည်ဆောက်ခြင်းမပြုဘဲ၊ လက်ခံဆောင်ရွက်ပေးသူသည် ပေးပို့မှုတိုင်းတွင် အဆိုပါတန်ဖိုးများကို `PoseidonArgs` မှတစ်ဆင့် ပေးပို့ပါသည်။ ပုံမှန်အားဖြင့် runtime သည် `MTLDevice::{is_low_power,is_headless,location}` သည် VRAM-အဆင့် လွှတ်တင်မှုများဆီသို့ discrete GPU များကို ဘက်လိုက်စေရန် စစ်ဆေးသည် (`256×24` ≥48GiB ကိုအစီရင်ခံသောအခါ၊ `256×20` သည် 32GiB၊ Prometheus နိမ့်နေချိန်တွင်) `256×8` (နှင့် အထက် 128/64 လမ်းကြား အစိတ်အပိုင်းများသည် လမ်းသွားတစ်ခုလျှင် 8/6 states တွင် ကပ်နေသည်) ထို့ကြောင့် အော်ပရေတာအများစုသည် env vars ကို ကိုယ်တိုင်သတ်မှတ်ရန် ဘယ်သောအခါမှ မလိုအပ်ပါ။【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover.1】1:9 ယခု `fastpq_metal_bench` သည် `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` အောက်တွင် သူ့ကိုယ်သူ ပြန်လည်လုပ်ဆောင်ပြီး `poseidon_microbench` ဘလောက်တစ်ခုကို ထုတ်လွှတ်လိုက်ပြီး ပစ်လွှတ်သည့်ပရိုဖိုင်များအပြင် တိုင်းတာထားသော အမြန်နှုန်းမြှင့်ခြင်းနှင့် စကလာလမ်းကြားကို မှတ်တမ်းတင်ထားသောကြောင့် ထုတ်လွှတ်လိုက်သော အစုအစည်းများသည် kernel အသစ်သည် အမှန်တကယ် 700080NI အကျုံ့သွားကြောင်း သက်သေပြနိုင်သည် `poseidon_pipeline` ပိတ်ဆို့ခြင်းကြောင့် Stage7 အထောက်အထားများသည် နေရာယူမှုအဆင့်အသစ်များနှင့်အတူ အတုံးအတိမ်အနက်/ထပ်နေသောအဖုများကို ဖမ်းယူသည်။ ပုံမှန်လည်ပတ်မှုများအတွက် env ကို မသတ်မှတ်ထားပါနှင့်။ ကြိုးသည် ပြန်လည်လုပ်ဆောင်မှုကို အလိုအလျောက် စီမံပေးသည်၊ ကလေးကို ဖမ်းယူ၍မရပါက ပျက်ကွက်မှုများကို မှတ်တမ်းတင်ပြီး `FASTPQ_GPU=gpu` ကို သတ်မှတ်လိုက်သောအခါ ချက်ချင်းထွက်သော်လည်း GPU backend မရနိုင်သောကြောင့် silent CPU တုံ့ပြန်မှုများသည် perf ထဲသို့ ဘယ်သောအခါမှ ခိုးမဝင်ပါ။ အနုပညာပစ္စည်းများ။ 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】`metal_dispatch_queue.poseidon` မြစ်ဝကျွန်းပေါ်ဒေသ၊ မျှဝေထားသော `column_staging` ကောင်တာများ သို့မဟုတ် `poseidon_profiles`/`poseidon_microbench` အထောက်အထားများ မအောင်မြင်သော သို့မဟုတ် သက်သေပြရန် ပျက်ကွက်သည့် မည်သည့်ဖမ်းယူမှုကိုမဆို ပြန်လည်စတင်ရမည်ဖြစ်ကြောင်း ထုပ်ပိုးမှုတွင် Poseidon ဖမ်းယူမှုများကို ပယ်ချပါသည်။ speedup.【scripts/fastpq/wrap_benchmark.py:732】 ဒက်ရှ်ဘုတ်များ သို့မဟုတ် CI မြစ်ဝကျွန်းပေါ်ဒေသများအတွက် သီးခြား JSON လိုအပ်သောအခါတွင် `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` ကိုဖွင့်ပါ။ အကူအညီပေးသူက ထုပ်ပိုးထားသော ပစ္စည်းများနှင့် အကြမ်းထည် `fastpq_metal_bench*.json` ဖမ်းယူမှုများကို လက်ခံသည်၊ ပုံသေ/စကေးအချိန်ဇယားများ၊ မက်တာဒေတာကို ချိန်ညှိခြင်းနှင့် မှတ်တမ်းတင်ထားသော အမြန်နှုန်းဖြင့် `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` ကို ထုတ်လွှတ်သည်။【scripts/fastpq/export_poseidon_microbench.py:1】
  `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` ကိုလုပ်ဆောင်ခြင်းဖြင့် run ကို အပြီးသတ်ပါ ထို့ကြောင့် Stage6 ထုတ်ပြန်မှုစစ်ဆေးချက်စာရင်းသည် `<1 s` LDE မျက်နှာကျက်ကို တွန်းအားပေးပြီး ထုတ်ဝေမှုနှင့်အတူ ပေးပို့သော လက်မှတ်ရေးထိုးထားသော manifest/digest အတွဲတစ်ခုကို ထုတ်လွှတ်ပါသည်။ လက်မှတ်။ 【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. ထုတ်လွှင့်ခြင်းမပြုမီ တယ်လီမီတာကို စစ်ဆေးပါ- Prometheus အဆုံးမှတ် (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) ကို လှန်ပြီး မမျှော်လင့်ထားသော `resolved="cpu"` အတွက် `telemetry::fastpq.execution_mode` မှတ်တမ်းများကို စစ်ဆေးပါ။ ထည့်သွင်းမှုများ။ 【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. ၎င်းအား ရည်ရွယ်ချက်ရှိရှိ တွန်းအားပေးခြင်းဖြင့် CPU အားနောက်ပြန်လမ်းကြောင်းကို မှတ်တမ်းတင်ပါ (`FASTPQ_GPU=cpu` သို့မဟုတ် `zk.fastpq.execution_mode = "cpu"`) သို့မှသာ SRE playbooks များသည် အဆုံးအဖြတ်နှင့် လိုက်လျောညီထွေရှိနေမည်ဖြစ်သည်။ အပြုအမူ။ 【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. ရွေးချယ်နိုင်သော ချိန်ညှိခြင်း- မူရင်းအားဖြင့် host သည် လမ်းကြောင်းတိုများအတွက် 16 လမ်းသွား၊ အလယ်အလတ်အတွက် 32 လမ်းနှင့် 64/128 `log_len ≥ 10/14` တစ်ကြိမ်၊ `log_len ≥ 10/14` တွင် 256 သို့ ဆင်းသက်ပြီး `log_len ≥ 18` တွင် ယခု မျှဝေထားသော-memory tile ကို အဆင့်လေးဆင့်အထိ သိမ်းဆည်းထားသည်။ `log_len ≥ 12`၊ နှင့် `log_len ≥ 18/20/22` အတွက် 12/14/16 အဆင့်များ အထက်ပါ အဆင့်များကို မလုပ်ဆောင်မီ `FASTPQ_METAL_FFT_LANES` (8 နှင့် 256 အကြား ပါဝါ-နှစ်ခု) နှင့်/သို့မဟုတ် `FASTPQ_METAL_FFT_TILE_STAGES` (1-16) ကို ထုတ်ယူပါ။ FFT/IFFT နှင့် LDE ကော်လံအတွဲအရွယ်အစားနှစ်ခုလုံးသည် ဖြေရှင်းထားသော threadgroup width မှဆင်းသက်လာပါသည် (≈2048 logical threads၊ ကော်လံ 32 ခုတွင် ကန့်သတ်ထားပြီး ယခု ဒိုမိန်းကြီးထွားလာသည်နှင့်အမျှ 32 → 16 → 8 → 4 → 2 → 1) သည် LDE လမ်းကြောင်းသည် ၎င်း၏ဒိုမိန်းထုပ်များကို ဆက်လက်ကျင့်သုံးနေချိန်တွင်၊ hosts များတစ်လျှောက် bit-for-bit နှိုင်းယှဉ်မှုများ လိုအပ်သောအခါတွင် တူညီသော override ကို LDE dispatcher ထံသို့ အသုံးပြုရန် အဆုံးအဖြတ်ပေးသော FFT batch အရွယ်အစားကို ပင်ထိုးရန် `FASTPQ_METAL_FFT_COLUMNS` (1-32) ကို သတ်မှတ်ပါ။ LDE ကြွေပြားအတိမ်အနက်သည် FFT heuristics ကိုလည်း ထင်ဟပ်စေသည်- `log₂ ≥ 18/20/22` ဖြင့် ခြေရာခံများသည် 12/10/8 မျှဝေထားသော-မှတ်ဉာဏ်အဆင့်များကိုသာ လုပ်ဆောင်ပြီး ကျယ်ပြန့်သောလိပ်ပြာများကို ကြွေပြားခင်းထားသော kernel သို့မလွှဲပြောင်းမီ—ထိုကန့်သတ်ချက်ကို `FASTPQ_METAL_LDE_TILE_STAGES` (1-322) မှ ကျော်လွန်နိုင်သည်။ runtime thread သည် Metal kernel args မှတဆင့် တန်ဖိုးအားလုံးကို ပေါင်းစည်းသည်၊ ပံ့ပိုးမထားသော overrides များကို ကုပ်လိုက်ကာ၊ ဖြေရှင်းပြီးသားတန်ဖိုးများကို မှတ်တမ်းပြုပေးသောကြောင့် လက်တွေ့စမ်းသပ်မှုများသည် metallib ကို ပြန်လည်တည်ဆောက်ခြင်းမပြုဘဲ မျိုးပွားနိုင်သည်။ စံသတ်မှတ်ချက် JSON သည် ဖြေရှင်းပြီးသော ချိန်ညှိခြင်းနှင့် လက်ခံဆောင်ရွက်ပေးသူ သုည-ဖြည့်စွက်ဘတ်ဂျက် (`zero_fill.{bytes,ms,queue_delta}`) တို့ကို LDE ကိန်းဂဏာန်းများမှတစ်ဆင့် ပေါ်လွင်စေသောကြောင့် တန်းစီနေသည့် မြစ်ဝကျွန်းပေါ်များကို ဖမ်းယူမှုတစ်ခုစီနှင့် တိုက်ရိုက်ချိတ်ဆက်ထားပြီး ယခု `column_staging` ဘလောက်တစ်ခုကို ထပ်ထည့်သည် (အသုတ်လိုက်၊ အပြားလိုက်၊ အပြားလိုက်၊ အပြားလိုက်၊ အပြားလိုက်၊ အပြားလိုက်၊ စောင့်ဆိုင်းနေပါသည်) ဝန်ဆောင်မှုပေးသူများ စောင့်ဆိုင်းနိုင်သည် double-buffered ပိုက်လိုင်းအားဖြင့်။ GPU သည် zero-fill telemetry ကိုအစီရင်ခံရန်ငြင်းဆိုသောအခါ၊ ယခုအခါ ကြိုးသည် host-side buffer မှသတ်မှတ်ထားသောအချိန်ကိုပေါင်းစပ်ပြီး `zero_fill` ဘလောက်ထဲသို့ထိုးသွင်းလိုက်သောကြောင့် အထောက်အထားမပြဘဲ ဘယ်သောအခါမှသင်္ဘောမပို့ပါ။ နယ်ပယ်။ al_bench.rs:575 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. Multi-queue dispatch သည် discrete Macs များတွင် အလိုအလျောက်ဖြစ်သည်- `Device::is_low_power()` သည် false သို့မဟုတ် Metal device မှ Slot/External တည်နေရာကို သတင်းပို့သည့်အခါ host သည် `MTLCommandQueue`s နှစ်ခုကို ချက်ချင်းထုတ်ပေးသည်၊ အလုပ်ဝန်သည် ≥16 ကော်လံ (fan-bince မှစကေးဖြင့်) ကော်လံနှစ်ခုလုံးကို ရှည်လျားစွာထားရှိသည်နှင့်တစ်ပြိုင်နက် ပရိတ်သတ်များသာ ထွက်သွားပါသည်။ အဆုံးအဖြတ်ကို အလျှော့မပေးဘဲ GPU လမ်းကြောင်းများ အလုပ်ရှုပ်နေပါသည်။ စက်များတွင် ပြန်လည်ထုတ်လုပ်နိုင်သော ဖမ်းယူမှုများ လိုအပ်သည့်အခါတိုင်း `FASTPQ_METAL_QUEUE_FANOUT` (1-4 တန်းစီ) နှင့် `FASTPQ_METAL_COLUMN_THRESHOLD` (ပန်ကာမထွက်ခွာမီ အနည်းဆုံး စုစုပေါင်းကော်လံများ) ဖြင့် မူဝါဒကို အစားထိုးပါ။ parity tests များသည် အဆိုပါ overrides များကို တွန်းအားပေးသောကြောင့် multi-GPU Macs များကို ဖုံးအုပ်ထားပြီး ဖြေရှင်းထားသော fan-out/threshold ကို တန်းစီ-အတိမ်အနက်ဘေးတွင် မှတ်သားထားသည်။ တယ်လီမီတာ။### မှတ်တမ်း တင်ရန်
| Artefact | ဖမ်းယူ | မှတ်စုများ |
|----------|---------|-------|
| `.metallib` အတွဲ | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` နှင့် `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` ၏နောက်တွင် `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` နှင့် `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`။ | သတ္တု CLI/toolchain ကို တပ်ဆင်ပြီး ဤကော်မတီအတွက် တိကျသေချာသော စာကြည့်တိုက်ကို ထုတ်လုပ်ထားကြောင်း သက်သေပြပါသည်။ 【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| ပတ်ဝန်းကျင် လျှပ်တစ်ပြက် | တည်ဆောက်ပြီးနောက် `echo $FASTPQ_METAL_LIB`; သင်၏ထွက်ခွင့်လက်မှတ်နှင့်အတူ ပကတိလမ်းကြောင်းကို ထိန်းသိမ်းပါ။ | အထွက်ဗလာဆိုသည်မှာ သတ္တုကို ပိတ်ထားခြင်း၊ ပို့ဆောင်ရေးပစ္စည်းများတွင် ရရှိနိုင်သည့် GPU လမ်းသွားများ၏ တန်ဖိုးစာရွက်စာတမ်းများကို မှတ်တမ်းတင်ခြင်း။ 【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| GPU တူညီမှုမှတ်တမ်း | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` နှင့် `backend="metal"` သို့မဟုတ် အဆင့်နှိမ့်ချသတိပေးချက်ပါရှိသော အတိုအထွာကို သိမ်းဆည်းပါ။ | တည်ဆောက်မှုကို မမြှင့်တင်မီ kernels လည်ပတ်နေသည် (သို့မဟုတ် အဆုံးအဖြတ်ကျကျ) လည်ပတ်ကြောင်း သရုပ်ပြသည်။ 【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| Benchmark output | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; ထုပ်ပြီး `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]` မှတဆင့် လက်မှတ်ထိုးပါ။ | ရစ်ပတ်ထားသော JSON မှတ်တမ်းများသည် `speedup.ratio`၊ `speedup.delta_ms`၊ FFT ချိန်ညှိခြင်း၊ padded အတန်းများ (32,768)၊ ကြွယ်ဝသော `zero_fill`/`kernel_profiles`၊ ပြားချပ်ချပ်ချပ် Prometheus၊ `metal_dispatch_queue.poseidon`/`poseidon_profiles` တုံးများ (`--operation poseidon_hash_columns` ကိုအသုံးပြုသောအခါ) နှင့် ခြေရာခံ မက်တာဒေတာ ထို့ကြောင့် GPU LDE သည် ≤950ms ရှိနေပြီး Poseidon သည် <1s; အစုအစည်းနှစ်ခုလုံးနှင့် ထုတ်လုပ်ထားသော `.json.asc` လက်မှတ်ကို ထုတ်ဝေခွင့်လက်မှတ်ဖြင့် သိမ်းဆည်းထားပါ၊ သို့မှသာ ဒက်ရှ်ဘုတ်များနှင့် စာရင်းစစ်များသည် ပစ္စည်းများ ပြန်လည်လည်ပတ်ခြင်းမပြုဘဲ အတည်ပြုနိုင်သည် အလုပ်များ။【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
| Bench manifest | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`။ | GPU နှစ်ခုလုံးကို မှန်ကန်ကြောင်း အတည်ပြုသည်၊ LDE ဆိုလိုသည်မှာ `<1 s` မျက်နှာကျက်ကို ချိုးဖျက်ပါက၊ BLAKE3/SHA-256 ၏ အချေအတင်များကို မှတ်တမ်းတင်ပြီး လက်မှတ်ရေးထိုးထားသော မန်နီးဖက်စ်ကို ထုတ်လွှတ်သောကြောင့် ထုတ်ပြန်ချက်စစ်ဆေးရန်စာရင်းသည် အတည်မပြုနိုင်ဘဲ ရှေ့မတိုးနိုင်ပါ။ မက်ထရစ်များ။【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| CUDA အတွဲ | SM80 ဓာတ်ခွဲခန်း host တွင် `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` ကိုဖွင့်ပြီး JSON ကို `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` သို့ ခြုံ/ရေးထိုးပါ (`--label device_class=xeon-rtx-sm80` ကိုသုံးပါ ထို့ကြောင့် ဒက်ရှ်ဘုတ်များသည် မှန်ကန်သော အတန်းကို ရွေးယူပါ)၊ လမ်းကြောင်းကို `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` သို့ ပေါင်းထည့်ကာ သိမ်းဆည်းထားပါ မန်နီးဖက်စ်ကို ပြန်လည်မထုတ်မီ `.json`/`.asc` သတ္တုအနုပညာနှင့် တွဲပါ။ စာရင်းသွင်းထားသော `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` သည် စာရင်းစစ်သူများ မျှော်လင့်ထားသည့် အတွဲဖော်မတ်အတိအကျကို သရုပ်ဖော်သည်။【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】 |
| Telemetry အထောက်အထား | စတင်ချိန်တွင် `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` နှင့် `telemetry::fastpq.execution_mode` မှတ်တမ်း။ | အသွားအလာမဖွင့်မီ Prometheus/OTEL သည် `device_class="<matrix>", backend="metal"` (သို့မဟုတ် အဆင့်နှိမ့်ချမှုမှတ်တမ်း) ကို ဖော်ထုတ်အတည်ပြုပါသည်။ 【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】| အတင်းအကြပ် CPU drill | `FASTPQ_GPU=cpu` သို့မဟုတ် `zk.fastpq.execution_mode = "cpu"` ဖြင့် အတိုအထွာတစ်ခုကို လုပ်ဆောင်ပြီး အဆင့်နှိမ့်မှုမှတ်တမ်းကို ဖမ်းယူပါ။ | ထုတ်ဝေမှု အလယ်အလတ်တွင် ပြန်လှည့်ရန် လိုအပ်ပါက SRE runbooks များကို အဆုံးအဖြတ်ပေးသော လမ်းကြောင်းနှင့် လိုက်လျောညီထွေဖြစ်အောင် ထားပါ။【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| ခြေရာခံဖမ်းယူ (ချန်လှပ်ထားနိုင်သည်) | `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` ဖြင့် တူညီသောစမ်းသပ်မှုကို ပြန်လုပ်ကာ ထုတ်လွှတ်လိုက်သော ပေးပို့မှုလမ်းကြောင်းကို သိမ်းဆည်းပါ။ | စံချိန်စံညွှန်းများ ပြန်လည်မလုပ်ဆောင်ဘဲ နောက်ပိုင်းတွင် ပရိုဖိုင်းသုံးသပ်ချက်များအတွက် occupancy/threadgroup အထောက်အထားများကို ထိန်းသိမ်းထားသည်။【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

ဘာသာစကားမျိုးစုံသုံး `fastpq_plan.*` ဖိုင်များသည် ဤစစ်ဆေးမှုစာရင်းကို ကိုးကားသောကြောင့် အဆင့်မြှင့်တင်ခြင်းနှင့် ထုတ်လုပ်ရေးအော်ပရေတာများသည် တူညီသောအထောက်အထားလမ်းကြောင်းအတိုင်း လိုက်နာဆောင်ရွက်ပါသည်။【docs/source/fastpq_plan.md:1】

## မျိုးပွားနိုင်သောတည်ဆောက်မှုများ
မျိုးပွားနိုင်သော Stage6 အနုပညာပစ္စည်းများကို ထုတ်လုပ်ရန် ပင်ထိုးထားသော ကွန်တိန်နာ အလုပ်အသွားအလာကို အသုံးပြုပါ-

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

helper script သည် `rust:1.88.0-slim-bookworm` toolchain image (နှင့် GPU အတွက် `nvidia/cuda:12.2.2-devel-ubuntu22.04`) ကို တည်ဆောက်ပြီး container အတွင်းတွင် build ကို run ကာ `manifest.json`၊ `sha256s.txt` နှင့် compiled binaries ကို ပစ်မှတ်အထွက်သို့ စာရေးသည်။ လမ်းညွှန်။【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

ပတ်ဝန်းကျင်ကို လွှမ်းမိုးသည်-
- `FASTPQ_RUST_IMAGE`၊ `FASTPQ_RUST_TOOLCHAIN` – တိကျသော Rust အခြေခံ/တက်ဂ်ကို တွယ်ပါ။
- `FASTPQ_CUDA_IMAGE` - GPU ပစ္စည်းများထုတ်လုပ်သည့်အခါ CUDA အခြေခံကို လဲလှယ်ပါ။
- `FASTPQ_CONTAINER_RUNTIME` - သတ်သတ်မှတ်မှတ် runtime ကို တွန်းအားပေးပါ။ မူရင်း `auto` သည် `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` ကို ကြိုးစားသည်။
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` - runtime အလိုအလျောက် သိရှိခြင်းအတွက် ကော်မာ-ခြားထားသော ဦးစားပေး အစီအစဉ် (မူရင်းမှာ `docker,podman,nerdctl`)။

## ဖွဲ့စည်းမှုမွမ်းမံမှုများ
1. သင်၏ TOML တွင် runtime execution mode ကို သတ်မှတ်ပါ-
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   တန်ဖိုးကို `FastpqExecutionMode` မှတဆင့် ခွဲခြမ်းစိပ်ဖြာပြီး စတင်ချိန်တွင် နောက်ခံဖိုင်သို့ အပိုင်းများကို ပိုင်းခြားထားပါသည်။ 【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. လိုအပ်ပါက စတင်ချိန်တွင် အစားထိုးပါ-
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI သည် node boot မလုပ်မီ ဖြေရှင်းထားသော config ကို အစားထိုးသည်။【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Developer များသည် တင်ပို့ခြင်းဖြင့် configs များကို မထိဘဲ ယာယီ ထောက်လှမ်းမှုကို တွန်းအားပေးနိုင်သည်။
   binary ကိုမစတင်မီ `FASTPQ_GPU={auto,cpu,gpu}`၊ override သည် logged ဖြစ်ပြီး ပိုက်လိုင်း
   ဖြေရှင်းပြီးသားမုဒ်ကို ဆက်လက်ဖော်ပြနေပါသည်။【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## အတည်ပြုချက်စာရင်း
1. **စတင်ခြင်းမှတ်တမ်းများ**
   - ပစ်မှတ် `telemetry::fastpq.execution_mode` မှ `FASTPQ execution mode resolved` ကို မျှော်လင့်ပါ
     `requested`၊ `resolved`၊ နှင့် `backend` အညွှန်းများ။ 【crates/fastpq_prover/src/backend.rs:208】
   - အလိုအလျောက် GPU ထောက်လှမ်းမှုတွင် `fastpq::planner` မှ ဒုတိယမှတ်တမ်းသည် နောက်ဆုံးလမ်းကြောင်းကို အစီရင်ခံသည်။
   - metallib ကို အောင်မြင်စွာ တင်ဆောင်သောအခါတွင် သတ္တုသည် `backend="metal"` မျက်နှာပြင်ကို လက်ခံဆောင်ရွက်ပေးသည်။ စုစည်းမှု သို့မဟုတ် တင်ခြင်း မအောင်မြင်ပါက build script သည် သတိပေးချက်တစ်ခု ထုတ်လွှတ်သည်၊ `FASTPQ_METAL_LIB` ကို ရှင်းလင်းပြီး ဆက်လက်မလုပ်ဆောင်မီ စီစဉ်သူသည် `GPU acceleration unavailable` ကို မှတ်တမ်းတင်ပါသည်။ CPU။ 【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/src/metal.rs:43】2. **Prometheus မက်ထရစ်များ**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   ကောင်တာကို `record_fastpq_execution_mode` ဖြင့် တိုးထားသည် (ယခု တံဆိပ်တပ်ထားသည်။
   `{device_class,chip_family,gpu_kind}`) သည် node တစ်ခု၏ လုပ်ဆောင်မှုကို ဖြေရှင်းသည့်အခါတိုင်း
   မုဒ်။ 【crates/iroha_telemetry/src/metrics.rs:8887】
   - သတ္တုလွှမ်းခြုံမှုအတွက်အတည်ပြုပါ။
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     သင်၏အသုံးချမှု ဒက်ရှ်ဘုတ်များနှင့်အတူ တိုးများလာပါသည်။【crates/iroha_telemetry/src/metrics.rs:5397】
   - `irohad --features fastpq-gpu` ဖြင့် စုစည်းထားသော macOS node များကို ထပ်မံဖော်ထုတ်ပါ။
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     နှင့်
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` ဒါကြောင့် Stage7 ဒက်ရှ်ဘုတ်များ
     တိုက်ရိုက် Prometheus ခြစ်ရာများမှ ဂျူတီစက်ဝန်းနှင့် တန်းစီခြင်းများကို ခြေရာခံနိုင်သည်။ 【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. ** Telemetry တင်ပို့ခြင်း**
   - OTEL သည် တူညီသောတံဆိပ်များဖြင့် `fastpq.execution_mode_resolutions_total` ကို ထုတ်လွှတ်ပါသည်။ သေချာပါစေ။
     GPU များ တက်ကြွနေချိန်တွင် ဒက်ရှ်ဘုတ်များ သို့မဟုတ် သတိပေးချက်များသည် မျှော်လင့်မထားသော `resolved="cpu"` ကို စောင့်ကြည့်သည်။

4. **စိတ်ဝိဥာဉ် သက်သေ/အထောက်အထား**
   - `iroha_cli` သို့မဟုတ် ပေါင်းစပ်ကြိုးကို ဖြတ်၍ သေးငယ်သောအသုတ်ကို လုပ်ဆောင်ပြီး အထောက်အထားများကို စစ်ဆေးအတည်ပြုပါ။
     တူညီသော parameters များဖြင့်ပြုစုထားသော peer ။

## ပြဿနာဖြေရှင်းခြင်း။
- **Resolved mode သည် GPU hosts ပေါ်တွင် CPU ရှိနေသည်** — binary ကို တည်ဆောက်ထားကြောင်း စစ်ဆေးပါ။
  `fastpq_prover/fastpq-gpu`၊ CUDA စာကြည့်တိုက်များသည် loader လမ်းကြောင်းပေါ်တွင်ရှိပြီး `FASTPQ_GPU` သည် အတင်းအကြပ်မလုပ်ပါ။
  `cpu`။
- **Apple Silicon တွင် သတ္တုမရရှိနိုင်ပါ** — CLI ကိရိယာများကို တပ်ဆင်ထားသည် (`xcode-select --install`)၊ `xcodebuild -downloadComponent MetalToolchain` ကို ပြန်ဖွင့်ပြီး တည်ဆောက်သည် အချည်းနှီးမဟုတ်သော `FASTPQ_METAL_LIB` လမ်းကြောင်းကို သေချာစစ်ဆေးပါ။ ဗလာ သို့မဟုတ် ပျောက်နေသော တန်ဖိုးသည် ဒီဇိုင်းအားဖြင့် နောက်ခံကို ပိတ်သည်။ 【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` အမှားများ** — Prover နှင့် Verifier နှစ်ခုလုံးသည် တူညီသော canonical catalog ကို အသုံးပြုကြောင်း သေချာပါစေ။
  `fastpq_isi` မှ ထုတ်လွှတ်သည်။ မျက်နှာပြင်သည် `Error::UnknownParameter` နှင့် မကိုက်ညီပါ။ 【crates/fastpq_prover/src/proof.rs:133】
- **မထင်မှတ်ထားသော CPU ဆုတ်ယုတ်မှု** — `cargo tree -p fastpq_prover --features` နှင့် စစ်ဆေးပါ။
  `fastpq_prover/fastpq-gpu` သည် GPU တည်ဆောက်မှုများတွင် ရှိနေကြောင်း အတည်ပြုပါ။ `nvcc`/CUDA စာကြည့်တိုက်များသည် ရှာဖွေမှုလမ်းကြောင်းပေါ်တွင် ရှိနေကြောင်း အတည်ပြုပါ။
- **တယ်လီမီတာ ကောင်တာ ပျောက်ဆုံးနေ** — node ကို `--features telemetry` ဖြင့် စတင်ကြောင်း အတည်ပြုပါ
  OTEL တင်ပို့မှုတွင် (ဖွင့်ထားလျှင်) မက်ထရစ်ပိုက်လိုင်း ပါဝင်သည်။【crates/iroha_telemetry/src/metrics.rs:8887】

## နောက်ပြန်ဆုတ်ခြင်းလုပ်ငန်းစဉ်
အဆုံးအဖြတ်ပေးသော နေရာယူသူ နောက်ခံကို ဖယ်ရှားလိုက်ပါပြီ။ ဆုတ်ယုတ်မှုတစ်ခု နောက်ပြန်ဆုတ်ရန် လိုအပ်ပါက၊
ယခင်က လူသိများသော ထုတ်ဝေမှုကောင်းသည့် ပစ္စည်းများကို ပြန်လည်အသုံးပြုပြီး အဆင့် 6 ကို ပြန်လည်မထုတ်ပေးမီ စုံစမ်းစစ်ဆေးပါ။
ဒွိစုံ အပြောင်းအလဲစီမံခန့်ခွဲမှု ဆုံးဖြတ်ချက်ကို မှတ်တမ်းတင်ပြီး ရှေ့သို့ လှည့်ပြီးမှသာ အပြီးသတ်ကြောင်း သေချာပါစေ။
ဆုတ်ယုတ်မှုကို နားလည်သည်။

3. `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` သည် မျှော်လင့်ထားသည့်အတိုင်း ထင်ဟပ်ကြောင်း သေချာစေရန် တယ်လီမီတာကို စောင့်ကြည့်ပါ
   placeholder ကွပ်မျက်ခြင်း။

## Hardware Baseline
| ကိုယ်ရေးအကျဉ်း | CPU | GPU | မှတ်စုများ |
| -------| ---| ---| -----|
| ကိုးကား (Stage6) | AMD EPYC7B12 (32 cores), 256GiB RAM | NVIDIA A10040GB (CUDA12.2) | 20000 အတန်း ပေါင်းစပ်ဖွဲ့စည်းမှု ≤1000ms ပြီးရပါမည်။【docs/source/fastpq_plan.md:131】 |
| CPU သီးသန့် | ≥32 ရုပ်ပိုင်းဆိုင်ရာ cores, AVX2 | – | အတန်း 20000 အတွက် ~0.9–1.2s ကို မျှော်လင့်ပါ။ အဆုံးအဖြတ်အတွက် `execution_mode = "cpu"` ကို ထားရှိပါ။ |## Regression Tests
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (GPU hosts များတွင်)
- ရွေးချယ်နိုင်သော ရွှေရောင် ခံစစ်မှူး စစ်ဆေးခြင်း-
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

သင်၏ ops runbook တွင် ဤစစ်ဆေးစာရင်းမှ သွေဖည်မှုများကို မှတ်တမ်းတင်ပြီး `status.md` ပြီးနောက် အပ်ဒိတ်လုပ်ပါ။
ရွှေ့ပြောင်းခြင်း ဝင်းဒိုး ပြီးပါပြီ။