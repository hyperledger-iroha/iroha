---
lang: my
direction: ltr
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2025-12-29T18:16:35.955568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Poseidon သတ္တုမျှခြေများ

သတ္တုစေ့များ၊ CUDA kernels၊ Rust prover နှင့် SDK fixture တိုင်းကို မျှဝေရမည်
ဟာ့ဒ်ဝဲကို အရှိန်မြှင့်ရန်အတွက် အတိအကျတူညီသော Poseidon2 ဘောင်များ
hashing အဆုံးအဖြတ်။ ဤစာတမ်းသည် canonical snapshot ကို မည်ကဲ့သို့ မှတ်တမ်းတင်သည်။
၎င်းကို ပြန်လည်ထုတ်ပေးပြီး GPU ပိုက်လိုင်းများသည် ဒေတာကို ထည့်သွင်းရန် မည်သို့မျှော်လင့်ထားသည်။

## Snapshot Manifest

ကန့်သတ်ချက်များကို `PoseidonSnapshot` RON စာရွက်စာတမ်းအဖြစ် ထုတ်ပြန်ထားသည်။ မိတ္တူတွေဖြစ်ကြတယ်။
ဗားရှင်းထိန်းချုပ်မှုအောက်တွင်ထားရှိထားသောကြောင့် GPU toolchains နှင့် SDKs များသည် build-time ကို အားမကိုးပါ။
ကုဒ်မျိုးဆက်။

| မဂ် | ရည်ရွယ်ချက် | SHA-256 |
|------|---------|---------|
| `artifacts/offline_poseidon/constants.ron` | `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}` မှ ထုတ်လုပ်ထားသော Canonical snapshot GPU တည်ဆောက်မှုအတွက် အမှန်တရားအရင်းအမြစ်။ | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` | Canonical လျှပ်တစ်ပြက်ရိုက်ချက်အား Mirrors ဖြင့်ပြသထားသောကြောင့် Swift ယူနစ်စမ်းသပ်မှုများနှင့် XCFramework မီးခိုးကြိုးသည် Metal kernels မျှော်မှန်းထားသည့်ကိန်းသေများနှင့်တူညီပါသည်။ | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | Android/Kotlin အစုံလိုက်များသည် တူညီမှုနှင့် အမှတ်စဉ်စမ်းသပ်မှုများအတွက် ထပ်တူကျသော သရုပ်ကို မျှဝေပါသည်။ | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

အသုံးပြုသူတိုင်းသည် ကိန်းသေများကို GPU တစ်ခုသို့ မချိတ်ဆက်မီ hash ကို စစ်ဆေးရပါမည်။
ပိုက်လိုင်း။ မန်နီးဖက်စ်သည် ပြောင်းလဲသောအခါ (ပါရာမီတာအသစ် သို့မဟုတ် ပရိုဖိုင်)၊ SHA နှင့်
အောက်ပိုင်းကြည့်မှန်များကို လော့ခ်ချသည့်အဆင့်တွင် အပ်ဒိတ်လုပ်ရပါမည်။

## ပြန်လည်ဆန်းသစ်ခြင်း။

မန်နီးဖက်စ်ကို `xtask` ကို အသုံးပြုခြင်းဖြင့် Rust အရင်းအမြစ်များမှ ထုတ်လုပ်သည်
ကူညီသူ။ command သည် canonical file နှင့် SDK mirrors နှစ်ခုလုံးကို ရေးသားသည်-

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

ဦးတည်ရာများကို ကျော်ရန် `--constants <path>`/`--vectors <path>` ကိုသုံးပါ သို့မဟုတ်
Canonical လျှပ်တစ်ပြက်ရိုက်ချက်ကိုသာ ပြန်လည်ထုတ်ပေးသောအခါ `--no-sdk-mirror`။ အထောက် အကူပြုမည်။
အလံကိုချန်လှပ်ထားသည့်အခါ Swift နှင့် Android သစ်ပင်များတွင် အနုပညာပစ္စည်းများကို မှန်ပြောင်း၊
CI အတွက် hashe များကို ချိန်ညှိပေးသည်။

## သတ္တု/CUDA တည်ဆောက်မှုများကို အစာကျွေးခြင်း။

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` နှင့်
  `crates/fastpq_prover/cuda/fastpq_cuda.cu` မှ ပြန်လည်ထုတ်ပေးရပါမည်။
  ဇယားပြောင်းသည့်အခါတိုင်း ထင်ရှားသည်။
- Rounded နှင့် MDS ကိန်းသေများကို ဆက်တိုက် `MTLBuffer`/`__constant` အဖြစ် အဆင့်သတ်မှတ်ထားပါသည်။
  မန်နီးဖက်စ် အပြင်အဆင်နှင့် ကိုက်ညီသည့် အပိုင်းများ- `round_constants[round][state_width]`
  နောက်တွင် 3x3 MDS မက်ထရစ်။
- `fastpq_prover::poseidon_manifest()` သည် လျှပ်တစ်ပြက်ရိုက်ချက်အား ဖွင့်ပြီး အတည်ပြုသည်။
  runtime (Metal warm-up ကာလအတွင်း) သို့မှသာ diagnostic tooling သည် ၎င်းကို စိတ်ချနိုင်သည်။
  shader constants မှတဆင့်ထုတ်ဝေထားသော hash နှင့်ကိုက်ညီသည်။
  `fastpq_prover::poseidon_manifest_sha256()`။
- SDK fixture readers (Swift `PoseidonSnapshot`၊ Android `PoseidonSnapshot`) နှင့်
  Norito အော့ဖ်လိုင်းတူးလ်သည် GPU သီးသန့်ကို ဟန့်တားသည့် တူညီသော manifest အပေါ်မှီခိုသည်
  ကန့်သတ်ဘောင်များ။

## အတည်ပြုခြင်း။

1. မန်နီးဖက်စ်ကို ပြန်လည်ထုတ်ပေးပြီးနောက်၊ လေ့ကျင့်ခန်းပြုလုပ်ရန် `cargo test -p xtask` ကိုဖွင့်ပါ။
   Poseidon fixture မျိုးဆက်ယူနစ် စမ်းသပ်မှုများ။
2. SHA-256 အသစ်ကို ဤစာရွက်စာတမ်းနှင့် စောင့်ကြည့်သည့် ဒက်ရှ်ဘုတ်များတွင် မှတ်တမ်းတင်ပါ။
   GPU လက်ရာများ။
3. `cargo test -p fastpq_prover poseidon_manifest_consistency` ပိုင်းခြားစိတ်ဖြာချက်
   တည်ဆောက်ချိန်၌ `poseidon2.metal` နှင့် `fastpq_cuda.cu` နှင့် ၎င်းတို့၏
   အမှတ်စဉ်ကိန်းသေများသည် CUDA/Metal ဇယားများနှင့် ထိန်းကျောင်းထားသည့် မန်နီးဖက်စ်နှင့် ကိုက်ညီသည်။
   သော့ခတ်အဆင့်တွင် စံပြလျှပ်တစ်ပြက်ရိုက်ချက်။GPU တည်ဆောက်မှု ညွှန်ကြားချက်များနှင့်အတူ မန်နီးဖက်စ်ကို ထိန်းသိမ်းခြင်းသည် Metal/CUDA ကို ပေးသည်။
အလုပ်အသွားအလာသည် အဆုံးအဖြတ်ရှိသော လက်ဆွဲနှုတ်ဆက်ခြင်းဖြစ်သည်- kernels များသည် ၎င်းတို့၏မှတ်ဉာဏ်ကို အကောင်းဆုံးဖြစ်အောင် လုပ်ဆောင်ရန် အခမဲ့ဖြစ်သည်။
မျှဝေထားသောကိန်းသေများကို blob တွင်ထည့်သွင်းပြီး hash အတွင်းသို့ထည့်သရွေ့ layout ကိုပြုလုပ်ပါ။
ညီမျှခြင်းစစ်ဆေးမှုများအတွက် telemetry ။