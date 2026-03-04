---
lang: my
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-05T09:28:11.999442+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Env → Config ရွှေ့ပြောင်းခြင်း ခြေရာခံကိရိယာ

ဤခြေရာခံကိရိယာသည် ပေါ်ပေါက်လာသော ထုတ်လုပ်မှု-ရင်ဆိုင်နေရသော ပတ်ဝန်းကျင်-ပြောင်းလဲနိုင်သော ခလုတ်များကို အကျဉ်းချုပ်ဖော်ပြသည်။
`docs/source/agents/env_var_inventory.{json,md}` ဖြင့် နှင့် ရည်ရွယ်ထားသော ရွှေ့ပြောင်းခြင်း။
`iroha_config` သို့လမ်းကြောင်း (သို့မဟုတ် တိကျသေချာသော dev/test-only scopeing)။


မှတ်ချက်- `ci/check_env_config_surface.sh` ** ထုတ်လုပ်ရေး** env အသစ်လုပ်သောအခါ ယခုအခါ ပျက်ကွက်ပါသည်။
`ENV_CONFIG_GUARD_ALLOW=1` မဟုတ်ပါက shims သည် `AGENTS_BASE_REF` နှင့် ဆက်စပ်နေပါသည်။
သတ်မှတ်; ထပ်မရေးမီ ဤနေရာတွင် ရည်ရွယ်ချက်ရှိရှိ ထပ်လောင်းထည့်မှုများကို မှတ်တမ်းတင်ပါ။

## ပြီးမြောက်အောင် ရွှေ့ပြောင်းခြင်း။- **IVM ABI ဖယ်ထုတ်ခြင်း** — ဖယ်ရှားထားသော `IVM_ALLOW_NON_V1_ABI`; အခု compiler က ငြင်းပါတယ်။
  non-v1 ABI များသည် error လမ်းကြောင်းကို ကာကွယ်သည့် ယူနစ်စမ်းသပ်မှုဖြင့် ခြွင်းချက်မရှိ၊
- **IVM အမှားရှာနဖူးစည်း env shim** — `IVM_SUPPRESS_BANNER` env ဖယ်ထုတ်လိုက်သည် ။
  နဖူးစည်းစာတန်းကို နှိပ်ကွပ်ခြင်းကို ပရိုဂရမ်မာစနစ်သတ်မှတ်သူမှတစ်ဆင့် ရရှိနိုင်ပါသည်။
- **IVM cache/sizing** — Threaded cache/prover/GPU မှတဆင့် အရွယ်အစား
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`၊
  `accel.max_gpus`) နှင့် runtime env shims ကို ဖယ်ရှားလိုက်ပါ။ အခု အိမ်ရှင်တွေက ဖုန်းဆက်တယ်။
  `ivm::ivm_cache::configure_limits` နှင့် `ivm::zk::set_prover_threads`၊ စမ်းသပ်အသုံးပြုမှုများ၊
  env overrides များအစား `CacheLimitsGuard`။
- **တန်းစီခြင်းကို ချိတ်ဆက်ပါ** — Added `connect.queue.root` (မူရင်း-
  `~/.iroha/connect`) ကို client config သို့ ချည်နှောင်ပြီး CLI မှတဆင့် လည်းကောင်း၊
  JS ရောဂါရှာဖွေရေး။ JS အကူအညီပေးသူများသည် config ကိုဖြေရှင်းခြင်း (သို့မဟုတ်တိကျသော `rootDir`) နှင့်
  `allowEnvOverride` မှတစ်ဆင့် dev/test တွင် `IROHA_CONNECT_QUEUE_ROOT` ကိုသာ ဂုဏ်ပြုပါသည်။
  တင်းပလိတ်များသည် ခလုတ်ကို မှတ်တမ်းတင်ထားသောကြောင့် အော်ပရေတာများသည် env overrides များမလိုအပ်တော့ပါ။
- **Izanami ကွန်ရက်ရွေးချယ်မှု** — အတွက် တိကျပြတ်သားသော `allow_net` CLI/config အလံကို ထည့်ထားသည်
  Izanami ပရမ်းပတာတူးလ်၊ ယခု run သည် `allow_net=true`/`--allow-net` လိုအပ်ပြီး
- **IVM နဖူးစည်း Beep** — `IROHA_BEEP` env shim ကို config-driven ဖြင့် အစားထိုးခဲ့သည်
  `ivm.banner.{show,beep}` ခလုတ်များ (မူလ- မှန်/မှန်)။ စတင်ခြင်းနဖူးစည်း / ဘီပီ
  ယခုအခါ ဝိုင်ယာကြိုးများသည် ထုတ်လုပ်မှုတွင်သာ ဖွဲ့စည်းမှုပုံစံကို ဖတ်သည်။ dev/test build က ဂုဏ်ယူနေတုန်းပဲ။
  Manual toggles အတွက် env override
- **DA spool override (စမ်းသပ်မှုများသာ)** — `IROHA_DA_SPOOL_DIR` သည် ယခု override ဖြစ်သည်
  `cfg(test)` အကူအညီပေးသူများနောက်တွင် ခြံခတ်ထားသည်။ ထုတ်လုပ်မှုကုဒ်သည် အမြဲတမ်း spool အရင်းအမြစ်ဖြစ်သည်။
  configuration မှလမ်းကြောင်း။
- **Crypto ပင်ကိုယ်** — `IROHA_DISABLE_SM_INTRINSICS` / အစားထိုး
  `IROHA_ENABLE_SM_INTRINSICS` နှင့်အတူ config-driven
  `crypto.sm_intrinsics` မူဝါဒ (`auto`/`force-enable`/`force-disable`) နှင့်
  `IROHA_SM_OPENSSL_PREVIEW` အစောင့်ကို ဖယ်ရှားခဲ့သည်။ Hosts မှာ မူဝါဒကို ကျင့်သုံးပါတယ်။
  စတင်ခြင်း၊ ခုံတန်းလျားများ/စမ်းသပ်မှုများကို `CRYPTO_SM_INTRINSICS` နှင့် OpenSSL တို့မှ ရွေးချယ်နိုင်သည်
  ယခု preview သည် config အလံကိုသာလေးစားသည်။
  Izanami သည် `--allow-net`/persisted config လိုအပ်နေပြီဖြစ်ပြီး စမ်းသပ်မှုများအပေါ် ယခုမှီခိုနေရသည်
  ambient env toggles ထက် ထိုခလုတ်။
- **FastPQ GPU ချိန်ညှိခြင်း** — `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}` ကို ထည့်ထားသည်။
  config knobs (မူရင်းများ- `None`/`None`/`false`/`false`/`false`) နှင့် CLI ခွဲခြမ်းစိတ်ဖြာခြင်းမှတဆင့် ၎င်းတို့ကို ချည်နှောင်ပါ
  `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` shims များသည် ယခုအခါ dev/test fallbacks အဖြစ် ပြုမူနေပြီး၊
  configuration loads ပြီးတာနဲ့ လစ်လျူရှုခံရတယ် (config က သူတို့ကို မသတ်မှတ်ထားရင်တောင်) docs/inventory တွေပါ။
  ရွှေ့ပြောင်းခြင်းကို အလံပြရန် ပြန်လည်ဆန်းသစ်ထားသည်။ 【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`၊
  `IVM_DEBUG_REGALLOC`, `IVM_DEBUG_METAL_ENUM`, `IVM_DEBUG_METAL_SELFTEST`၊
  `IVM_FORCE_METAL_ENUM`, `IVM_FORCE_METAL_SELFTEST_FAIL`, `IVM_FORCE_CUDA_SELFTEST_FAIL`၊
  `IVM_DISABLE_METAL`၊ `IVM_DISABLE_CUDA`) သည် မျှဝေထားသောမှတစ်ဆင့် debug/test builds များ၏ နောက်ကွယ်တွင် ပိတ်ထားသည်
  အကူအညီပေးသူ ဖြစ်သောကြောင့် ထုတ်လုပ်ရေး binaries များသည် ဒေသဆိုင်ရာ ရောဂါရှာဖွေမှုအတွက် အဖုများကို ထိန်းသိမ်းထားစဉ် ၎င်းတို့ကို လျစ်လျူရှုထားသည်။ Env
  dev/test-only scope ကို ထင်ဟပ်စေရန် inventory ကို ပြန်လည်ထုတ်ပေးပါသည်။- **FASTPQ ပွဲစဉ်များ အပ်ဒိတ်များ** — ယခု `FASTPQ_UPDATE_FIXTURES` သည် FASTPQ ပေါင်းစပ်မှုတွင်သာ ပေါ်လာသည်
  စမ်းသပ်မှုများ; ထုတ်လုပ်မှုရင်းမြစ်များသည် env toggle ကို မဖတ်တော့ဘဲ စာရင်းအင်းသည် စမ်းသပ်မှုသာဖြစ်ကြောင်း ထင်ဟပ်ပါသည်။
  အတိုင်းအတာ။
- ** Inventory refresh + scope detection** — env inventory tooling သည် ယခု `build.rs` ဖိုင်များအဖြစ် tag လုပ်ထားသည်။
  နယ်ပယ်တည်ဆောက်ခြင်းနှင့် ခြေရာခံခြင်း `#[cfg(test)]`/integration harness modules ထို့ကြောင့် စမ်းသပ်ရန်သာခလုတ်များ (ဥပမာ၊
  `IROHA_TEST_*`၊ `IROHA_RUN_IGNORED`) နှင့် CUDA တည်ဆောက်မှုအလံများသည် ထုတ်လုပ်မှုအရေအတွက်အပြင်ဘက်တွင် ပေါ်လာသည်။
  env-config guard diff စိမ်းလန်းနေစေရန် Inventory ကို ဒီဇင်ဘာ 07၊ 2025 (518 refs / 144 vars) တွင် ပြန်လည်ထုတ်ပေးပါသည်။
- **P2P topology env shim release guard** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` သည် ယခုအခါ အဆုံးအဖြတ်တစ်ခုကို အစပျိုးလိုက်သည်
  ဖြန့်ချိမှုတည်ဆောက်မှုများတွင် စတင်ခြင်းဆိုင်ရာ အမှားအယွင်း (ဒီဘာဂ်/စမ်းသပ်မှုတွင် သတိပေးချက်သာ) ဖြစ်သောကြောင့် ထုတ်လုပ်ရေးဆုံမှတ်များကိုသာ အားကိုးပါသည်။
  `network.peer_gossip_period_ms`။ အစောင့်အကြပ်များနှင့် ကိုက်ညီစေရန်အတွက် env စာရင်းကို ပြန်လည်ထုတ်ပေးခဲ့သည်။
  အပ်ဒိတ်လုပ်ထားသော အမျိုးအစားခွဲသည် ယခုအခါ `cfg!` ကို အမှားရှာ/စမ်းသပ်မှုအဖြစ် သတ်မှတ်ပေးထားသည်။

## ဦးစားပေးရွှေ့ပြောင်းမှုများ (ထုတ်လုပ်မှုလမ်းကြောင်းများ)

- _None (inventory ကို cfg!/debug detection ဖြင့် ပြန်လည်စတင်သည်၊ P2P shim hardening ပြီးနောက် env-config guard အစိမ်းရောင်)။_

## Dev/test-only သည် ခြံစည်းရိုးသို့ ပြောင်းသွားသည်

- လက်ရှိ သုတ်သင်ရှင်းလင်းခြင်း (ဒီဇင်ဘာ 07၊ 2025)- တည်ဆောက်-သပ်သပ် CUDA အလံများ (`IVM_CUDA_*`) ကို `build` အဖြစ် သတ်မှတ်ပြီး
  ကြိုးကြိုးခလုတ်များ (`IROHA_TEST_*`၊ `IROHA_RUN_IGNORED`၊ `IROHA_SKIP_BIND_CHECKS`) အဖြစ် ယခု စာရင်းသွင်းလိုက်ပါ
  စာရင်းထဲတွင် `test`/`debug` (`cfg!`-စောင့်ကြပ်ထားသော shims အပါအဝင်)။ ထပ်လောင်းကာရံရန်မလိုအပ်ပါ။
  `cfg(test)` / ခုံတန်းလျားသီးသန့် TODO အမှတ်အသားများပါရှိသော အကူအညီများနောက်တွင် အနာဂတ်ထပ်တိုးမှုများကို ထားရှိပါ။

## Build-time envs (ဖြစ်စဥ်အတိုင်းထားခဲ့ပါ)

- ကုန်တင်/ဝန်ဆောင်မှု envs (`CARGO_*`၊ `OUT_DIR`၊ `DOCS_RS`၊ `PROFILE`၊ `CUDA_HOME`၊
  `CUDA_PATH`, `JSONSTAGE1_CUDA_ARCH`, `FASTPQ_SKIP_GPU_BUILD` စသည်ဖြင့်) ကျန်ရှိနေသည်
  build-script စိုးရိမ်မှုများနှင့် runtime config ရွှေ့ပြောင်းခြင်းအတွက် နယ်ပယ်ပြင်ပတွင် ရှိနေပါသည်။

## နောက်တစ်ခုက လုပ်ဆောင်ချက်တွေပါ။

1) ထုတ်လုပ်မှု env shims အသစ်များကိုဖမ်းမိရန် config-surface updates များပြီးနောက် `make check-env-config-surface` ကို run ပါ။
   အစောပိုင်းတွင် စနစ်ခွဲပိုင်ရှင်များ/ ETA များကို သတ်မှတ်ပေးသည်။  
2) သိမ်းပြီးနောက် သိုလှောင်မှု (`make check-env-config-surface`) ကို ပြန်လည်စတင်ပါ။
   tracker သည် guardrails အသစ်များနှင့် လိုက်လျောညီထွေရှိနေပြီး env-config guard diff သည် ဆူညံသံကင်းမဲ့နေပါသည်။