---
lang: my
direction: ltr
source: docs/source/fastpq_metal_kernels.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0022f5f9c53445d26876f0097635092b5c685d332bfa25b13243c584d358dfe
source_last_modified: "2026-01-05T09:28:12.006723+00:00"
translation_last_reviewed: 2026-02-07
title: FASTPQ Metal Kernel Suite
translator: machine-google-reviewed
---

# FASTPQ Metal Kernel Suite

Apple Silicon backend သည် တစ်ခုစီပါရှိသော `fastpq.metallib` ကို ပေးပို့သည်။
သက်သေပြချက်ဖြင့်ကျင့်သုံးသော Metal Shading Language (MSL) kernel ဒီမှတ်စုက ရှင်းပြတယ်။
ရရှိနိုင်သော ဝင်ခွင့်အမှတ်များ၊ ၎င်းတို့၏ အစုအဖွဲ့ ကန့်သတ်ချက်များ၊ နှင့် သတ်မှတ်ပြဋ္ဌာန်းချက်များ
GPU လမ်းကြောင်းကို scalar fallback နှင့် အပြန်အလှန်လဲလှယ်နိုင်စေရန် အာမခံပါသည်။

Canonical အကောင်အထည်ဖော်မှုအောက်တွင်နေထိုင်သည်။
`crates/fastpq_prover/metal/kernels/` ဖြင့် ပြုစုထားပါသည်။
`fastpq-gpu` ကို macOS တွင် ဖွင့်ထားသည့်အခါတိုင်း `crates/fastpq_prover/build.rs`။
Runtime metadata (`metal_kernel_descriptors`) သည် အောက်ပါအချက်အလက်များကို ထင်ဟပ်စေသည်။
စံသတ်မှတ်ချက်များနှင့် ရောဂါရှာဖွေခြင်းများသည် တူညီသောအချက်များကို ဖော်ပြနိုင်သည်။ ပရိုဂရမ်စနစ်ဖြင့်။ 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/8】al

## Kernel စာရင်း| ဝင်ခွင့်အမှတ် | စစ်ဆင်ရေး | Threadgroup cap | ကြွေပြားစင် |ဦးထုပ် မှတ်စုများ |
| ----------- | ---------| --------------- | -------------- | -----|
| `fastpq_fft_columns` | ခြေရာခံကော်လံများ | FFT ကို ထပ်ဆင့်ပို့ပါ။ 256 threads | 32 အဆင့် | ပထမအဆင့်အတွက် မျှဝေထားသော-မှတ်ဉာဏ် အကွက်များကို အသုံးပြုပြီး အစီအစဉ်ရေးဆွဲသူက IFFT မုဒ်ကို တောင်းဆိုသည့်အခါ ပြောင်းပြန်အကွက်များကို အသုံးပြုသည်။ 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_fft_post_tiling` | အကွက်အတိမ်အနက်သို့ရောက်ပြီးနောက် | FFT/IFFT/LDE ကို အပြီးသတ်ပါ။ 256 threads | — | ကျန်ရှိနေသော လိပ်ပြာများကို စက်မမ်မိုရီအတွင်းမှ တိုက်ရိုက်လည်ပတ်ပြီး အိမ်ရှင်ထံသို့ မပြန်မီ နောက်ဆုံး coset/ပြောင်းပြန်အချက်များအား ကိုင်တွယ်သည်။ 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` | ကော်လံများအနှံ့ | 256 threads | 32 အဆင့် | အကဲဖြတ်သည့်ကြားခံထဲသို့ မိတ္တူကူးယူကာ၊ ပြင်ဆင်ထားသော coset ဖြင့် အကွက်လိုက်အဆင့်များကို လုပ်ဆောင်ပြီး လိုအပ်သည့်အခါ နောက်ဆုံးအဆင့်များကို `fastpq_fft_post_tiling` သို့ ထားခဲ့သည်။【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】 【crates/fastpq_pasters】
| `poseidon_trace_fused` | hash ကော်လံများနှင့် compute depth-1 မိဘများ | 256 threads | — | `poseidon_hash_columns` ကဲ့သို့ တူညီသော စုပ်ယူမှု/ပြောင်းလဲခြင်းများကို လုပ်ဆောင်ပြီး အရွက်များကို အထွက်ကြားခံတွင် တိုက်ရိုက်သိမ်းဆည်းကာ `(left,right)` အတွဲတစ်ခုစီကို `fastpq:v1:trace:node` ဒိုမိန်းအောက်တွင် ခေါက်ထားသောကြောင့် `(⌈columns / 2⌉)` ပြီးနောက် အရွက်များကို အခွံခွာပါ။ ထူးဆန်းသောကော်လံများသည် စက်ပေါ်ရှိ နောက်ဆုံးအရွက်ကို ပွားစေပြီး ပထမ Merkle အလွှာအတွက် CPU လှည့်ကွက်ကို ဖယ်ရှားပေးပါသည်။ 【crates/fastpq_prover/metal/kernels/poseidon2.metal:384】【crates/fastpq_prover/src/metal7】24
| `poseidon_permute` | Poseidon2 ကူးပြောင်းခြင်း (STATE_WIDTH = 3) | 256 threads | — | Threadgroups များသည် threadgroup memory တွင် round constants/ MDS အတန်းများကို cache လုပ်ပြီး၊ MDS အတန်းများကို per-thread registers များအဖြစ် ကူးယူကာ အဆင့် ၄ ဆင့်ရှိ လုပ်ငန်းစဉ်ပြည်နယ်များအတိုင်း လုပ်ဆောင်ပေးသောကြောင့် အဆက်မပြတ်ထုတ်ယူမှုတစ်ခုစီကို မတိုးတက်မီ အခြေအနေများစွာတွင် ပြန်လည်အသုံးပြုနိုင်ပါသည်။ လှည့်ပတ်မှုများသည် အပြည့်အဝမဖွင့်ရသေးဘဲ လမ်းကြောင်းတစ်ခုစီတိုင်းသည် ပြည်နယ်များစွာကို ဆက်လက်သွားလာနေဆဲဖြစ်ပြီး၊ ပေးပို့မှုတစ်ခုလျှင် ယုတ္တိတန်သော လိုင်းပေါင်း ≥4096 ကို အာမခံပါသည်။ `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` အား ပြန်လည်တည်ဆောက်ခြင်းမပြုဘဲ ပစ်လွှတ်သည့် အကျယ်နှင့် တစ်လမ်းသွားအသုတ်ကို တွယ်၊ metallib

ဖော်ပြချက်များအား runtime မှတဆင့် ရရှိနိုင်ပါသည်။
ပြသလိုသောကိရိယာအတွက် `fastpq_prover::metal_kernel_descriptors()`
တူညီသော မက်တာဒေတာ။

## Deterministic Goldilocks ဂဏန်းသင်္ချာ- kernel များအားလုံးသည် သတ်မှတ်ထားသော helpers များဖြင့် Goldilocks အကွက်ပေါ်တွင် အလုပ်လုပ်ပါသည်။
  `field.metal` (modular add/mul/sub၊ ပြောင်းပြန်၊ `pow5`)။【crates/fastpq_prover/metal/kernels/field.metal:1】
- FFT/LDE အဆင့်များသည် CPU စီစဉ်သူထုတ်လုပ်သည့် တူညီသော twiddle ဇယားများကို ပြန်လည်အသုံးပြုသည်။
  `compute_stage_twiddles` သည် အဆင့်တစ်ခုလျှင် twiddle တစ်ခုနှင့် host ကို ကြိုတင်တွက်ချက်သည်။
  dispatch တစ်ခုစီမတိုင်မီတွင် array ကို buffer slot 1 မှတစ်ဆင့် အပ်လုဒ်လုပ်သည်
  GPU လမ်းကြောင်းသည် စည်းလုံးမှု၏ ထပ်တူကျသော အမြစ်များကို အသုံးပြုပါသည်။ 【crates/fastpq_prover/src/metal.rs:1527】
- LDE အတွက် Coset မြှောက်ခြင်းကို နောက်ဆုံးအဆင့်သို့ ပေါင်းစပ်ထားသောကြောင့် GPU ကို ဘယ်တော့မှ မရနိုင်ပါ။
  CPU ခြေရာခံ အသွင်အပြင်နှင့် ကွဲပြားသည်။ လက်ခံသူသည် သုည-အကဲဖြတ်သည့်ကြားခံကို ဖြည့်ပေးသည်။
  မပို့မီ၊ padding အပြုအမူကို အဆုံးအဖြတ်ပေးသည်။ 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## Metallib မျိုးဆက်

`build.rs` သည် `.metal` တစ်ခုချင်းစီကို `.air` အရာဝတ္ထုများအဖြစ် စုစည်းပြီး
၎င်းတို့ကို `fastpq.metallib` တွင် ချိတ်ဆက်ထားပြီး အထက်ဖော်ပြပါ ဝင်မှတ်တိုင်းကို တင်ပို့သည်။
`FASTPQ_METAL_LIB` ကို ထိုလမ်းကြောင်းသို့ သတ်မှတ်ခြင်း (Build script က ဒါကို လုပ်ဆောင်ပါတယ်။
အလိုအလျောက်) မခွဲခြားဘဲ စာကြည့်တိုက်ကို ဖွင့်ရန် အချိန်ကို ခွင့်ပြုသည်။
`cargo` သည် ရှေးဟောင်းပစ္စည်းများကို တည်ဆောက်ထားသည့်နေရာမှ ဖြစ်သည်။【crates/fastpq_prover/build.rs:45】

CI လည်ပတ်မှုများနှင့် ညီမျှစေရန် သင်သည် စာကြည့်တိုက်ကို ကိုယ်တိုင်ပြန်ထုတ်နိုင်သည်-

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## Threadgroup sizing heuristics

`metal_config::fft_tuning` သည် စက်လည်ပတ်မှု အကျယ်နှင့် အမြင့်ဆုံး threads အလိုက် တွဲပေးသည်
threadgroup သည် planner ထဲသို့ runtime dispats များသည် hardware ကန့်သတ်ချက်များကိုလေးစားသည်။
မှတ်တမ်းအရွယ်အစား တိုးလာသည်နှင့်အမျှ ပုံသေများသည် 32/64/128/256 လမ်းကြောင်းများသို့ ချည်နှောင်ထားပြီး၊
ကြွေပြားအတိမ်အနက်သည် ယခု `log_len ≥ 12` တွင် အဆင့်ငါးဆင့်မှ လေးဆင့်သို့ ရွေ့လျားသွားပြီး၊
သဲလွန်စကိုဖြတ်သွားသည်နှင့် 12/14/16 အဆင့်များအတွက် shared-memory pass သည် အသက်ဝင်ပါသည်။
`log_len ≥ 18/20/22` ကို ကြွေပြားခင်းထားသော kernel သို့ အလုပ်မအပ်မီ။ အော်
overrides (`FASTPQ_METAL_FFT_LANES`၊ `FASTPQ_METAL_FFT_TILE_STAGES`) ဖြတ်သန်းစီးဆင်းသည်
`FftArgs::threadgroup_lanes`/`local_stage_limit` နှင့် kernels များဖြင့် အသုံးပြုသည်
အပေါ်က metallib ကို ပြန်မဆောက်ဘဲ။【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

ဖြေရှင်းထားသော ချိန်ညှိခြင်းတန်ဖိုးများကို ဖမ်းယူရန်နှင့် ၎င်းကိုစစ်ဆေးရန် `fastpq_metal_bench` ကိုသုံးပါ။
Multi-pass kernels များကို JSON တွင် (`post_tile_dispatches`) တွင် ကျင့်သုံးခဲ့သည်
စံညွှန်းအတွဲတစ်ခုကို ပို့ဆောင်ခြင်း။【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】