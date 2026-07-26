---
lang: my
direction: ltr
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2025-12-29T18:16:35.920451+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# GPU Benchmark Capture History (FASTPQ WP5-B)

ဤဖိုင်ကို `python3 scripts/fastpq/update_benchmark_history.py` မှထုတ်ပေးပါသည်။
၎င်းသည် ထုပ်ပိုးထားသော GPU တိုင်းကို ခြေရာခံခြင်းဖြင့် ပေးပို့နိုင်သော FASTPQ အဆင့် 7 WP5-B ကို ကျေနပ်စေသည်။
စံသတ်မှတ်ချက်လက်ရာများ၊ Poseidon microbench သရုပ်ဖော်ပုံနှင့် အရန်အရာများ
`benchmarks/`။ နောက်ခံရိုက်ကူးမှုများကို အပ်ဒိတ်လုပ်ပြီး အသစ်တစ်ခုတိုင်းတွင် ဇာတ်ညွှန်းကို ပြန်လည်လုပ်ဆောင်ပါ။
အစုအဝေးမြေများ သို့မဟုတ် တယ်လီမီတာ သက်သေအသစ် လိုအပ်သည်။

## နယ်ပယ်နှင့် အပ်ဒိတ်လုပ်ငန်းစဉ်

- GPU ဖမ်းယူမှုအသစ်များ (`scripts/fastpq/wrap_benchmark.py` မှတဆင့်) ထုတ်လုပ်ခြင်း သို့မဟုတ် ခြုံခြင်း
  ၎င်းတို့ကို capture matrix တွင်ထည့်သွင်းပြီး ပြန်လည်စတင်ရန် ဤမီးစက်ကို ပြန်ဖွင့်ပါ။
  စားပွဲများ။
- Poseidon microbench ဒေတာရှိသောအခါ၊ ၎င်းကိုတင်ပို့ပါ။
  `scripts/fastpq/export_poseidon_microbench.py` သုံးပြီး မန်နီးဖက်စ်ကို ပြန်လည်တည်ဆောက်ပါ။
  `scripts/fastpq/aggregate_poseidon_microbench.py`။
- ၎င်းတို့၏ JSON အထွက်များကို အောက်တွင် သိမ်းဆည်းခြင်းဖြင့် Merkle အဆင့်သတ်မှတ်မှုကို မှတ်တမ်းတင်ပါ။
  `benchmarks/merkle_threshold/`; ဤဂျင်နရေတာသည် သိထားသောဖိုင်များကို စာရင်းပြုစုထားသောကြောင့် စာရင်းစစ်သည်။
  Cross-reference CPU နှင့် GPU ရရှိနိုင်မှု။

## FASTPQ အဆင့် 7 GPU စံသတ်မှတ်ချက်များ

| အတွဲ | နောက်ခံလူ | မုဒ် | GPU နောက်ခံ | GPU ရနိုင်သည် | စက်ပစ္စည်းအတန်း | GPU | LDE ms (CPU/GPU/SU) | Poseidon ms (CPU/GPU/SU) |
|---------|---------|------|----------------|----------------|----------------|-----|--------------------------------|---------------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | gpu | cuda-sm80 | ဟုတ်တယ် | xeon-rtx | NVIDIA RTX 6000 Ada | 1512.9/880.7/1.72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | သတ္တု | gpu | မရှိ | ဟုတ်တယ် | apple-m4 | Apple GPU 40-core | 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | သတ္တု | gpu | သတ္တု | ဟုတ်တယ် | apple-m2-ultra | Apple M2 Ultra | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | သတ္တု | gpu | သတ္တု | ဟုတ်တယ် | apple-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | သတ္တု | gpu | သတ္တု | ဟုတ်တယ် | apple-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | opencl | gpu | opencl | ဟုတ်တယ် | neoverse-mi300 | AMD Instinct MI300A | 4518.5/688.9/6.56 | 2780.4/905.6/3.07 |

> ကော်လံများ- `Backend` သည် အစုအစည်းအမည်မှ ဆင်းသက်လာသည်။ `Mode`/`GPU backend`/`GPU available`
> CPU မှားယွင်းမှုများ သို့မဟုတ် ပျောက်ဆုံးနေသော GPU များကို ဖော်ထုတ်ရန် ပတ်ထားသော `benchmarks` ဘလောက်မှ ကူးယူထားပါသည်။
> ရှာဖွေတွေ့ရှိမှု (ဥပမာ၊ `gpu_backend=none`၊ `Mode=gpu`)။ SU = အရှိန်မြှင့်နှုန်း (CPU/GPU)။

## Poseidon Microbench ပုံရိပ်များ

`benchmarks/poseidon/manifest.json` သည် မူရင်း-vs-scalar Poseidon ကို စုစည်းသည်
သတ္တုအစုအဝေးတစ်ခုစီမှ ထုတ်ယူထားသော microbench များ။ အောက်ပါဇယားကို ပြန်လည်ဆန်းသစ်ထားသည်။
ဂျင်နရေတာ ဇာတ်ညွှန်း၊ ထို့ကြောင့် CI နှင့် အုပ်ချုပ်မှု ပြန်လည်သုံးသပ်ခြင်းများသည် သမိုင်းဆိုင်ရာ အရှိန်မြှင့်မှုများကို ကွဲပြားစေနိုင်သည်။
ထုပ်ပိုးထားသော FASTPQ အစီရင်ခံစာများကို မဖွင့်ဘဲ၊

| အနှစ်ချုပ် | အတွဲ | အချိန်တံဆိပ်ခေါင်း | ပုံသေ ms | Scalar ms | အရှိန်မြှင့် |
|---------|--------|-----------|---------------------|----------------|--------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | 2152.2 | 0.99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | ၁၉၉၀.၅ | ၁၉၉၄.၅ | 1.00 |

## Merkle Threshold Sweepsအကိုးအကားတွေကနေတဆင့် စုစည်းရိုက်ကူးထားတာပါ။
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
`benchmarks/merkle_threshold/` အောက်တွင် နေထိုင်ပါ။ စာရင်းသွင်းချက်များသည် အိမ်ရှင်ရှိမရှိကို ပြသသည်။
တံမြက်စည်းများ ပြေးသွားသောအခါ သတ္တုပစ္စည်းများကို ထိတွေ့ခြင်း၊ GPU ဖွင့်ထားသော ဖမ်းယူမှုများကို သတင်းပို့သင့်သည်။
`metal_available=true`။

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

Apple Silicon capture (`takemiyacStudio.lan_25.0.0_arm64`) သည် `docs/source/benchmarks.md` တွင်အသုံးပြုထားသော canonical GPU အခြေခံလိုင်းဖြစ်သည်။ macOS 14 ထည့်သွင်းမှုများသည် သတ္တုစက်ပစ္စည်းများကို ဖော်ထုတ်၍မရသော ပတ်ဝန်းကျင်များအတွက် CPU သီးသန့်အခြေခံလိုင်းများအဖြစ် ကျန်ရှိနေပါသည်။

## အတန်း-အသုံးပြုမှု ပုံရိပ်များ

`scripts/fastpq/check_row_usage.py` မှတစ်ဆင့် ရိုက်ကူးထားသော သက်သေခံ ကုဒ်များသည် လွှဲပြောင်းမှုကို သက်သေပြပါသည်။
gadget ၏အတန်းထိရောက်မှု။ JSON လက်ရာများကို `artifacts/fastpq_benchmarks/` အောက်တွင် ထားပါ။
နှင့် ဤဂျင်နရေတာသည် စာရင်းစစ်များအတွက် မှတ်တမ်းတင်ထားသော လွှဲပြောင်းမှုအချိုးများကို အကျဉ်းချုပ်ပေးပါမည်။

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — batch=2၊ transfer_ratio avg=0.629 (min=0.625၊ max=0.633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — အတွဲများ=2၊ transfer_ratio avg=0.619 (min=0.613၊ max=0.625)