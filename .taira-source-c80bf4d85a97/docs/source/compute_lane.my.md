---
lang: my
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ကွန်ပျူတာလမ်းသွယ် (SSC-1)

တွက်ချက်မှုလမ်းကြောင်းသည် အဆုံးအဖြတ်ပေးသော HTTP ပုံစံခေါ်ဆိုမှုများကို လက်ခံသည်၊ ၎င်းတို့ကို Kotodama တွင် မြေပုံဆွဲသည်
ငွေတောင်းခံခြင်းနှင့် အုပ်ချုပ်မှုပြန်လည်သုံးသပ်ခြင်းအတွက် တိုင်းတာခြင်း/ပြေစာများ မှတ်တမ်းများ။
ဤ RFC သည် manifest schema၊ ခေါ်ဆိုမှု/လက်ခံစာအိတ်များ၊ sandbox guardrails ကို ရပ်တန့်စေသည်
နှင့် ပထမထုတ်ဝေမှုအတွက် ဖွဲ့စည်းမှုပုံသေများ။

## ဖော်ပြချက်

- အစီအစဉ်- `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`)။
- `abi_version` ကို `1` တွင် ချိတ်ထားသည်။ ကွဲပြားသောဗားရှင်းဖြင့် ဖော်ပြချက်များကို ပယ်ချပါသည်။
  အတည်ပြုနေစဉ်။
- လမ်းကြောင်းတစ်ခုစီသည်-
  - `id` (`service`၊ `method`)
  - `entrypoint` (Kotodama ဝင်ခွင့်အမှတ်အမည်)
  - codec ခွင့်ပြုစာရင်း (`codecs`)
  - TTL/ဓာတ်ငွေ့/တောင်းဆိုချက်/တုံ့ပြန်မှုထုပ်များ (`ttl_slots`၊ `gas_budget`၊ `max_*_bytes`)
  - အဆုံးအဖြတ်ပေးခြင်း/ စီရင်ချက်အတန်း (`determinism`၊ `execution_class`)
  - SoraFS ingress/ model descriptors (`input_limits`၊ optional `model`)
  - စျေးနှုန်းမိသားစု (`price_family`) + အရင်းအမြစ်ပရိုဖိုင် (`resource_profile`)
  - စစ်မှန်ကြောင်းအထောက်အထားပြခြင်းမူဝါဒ (`auth`)
- Sandbox guardrails များသည် manifest `sandbox` block တွင်နေထိုင်ပြီး အားလုံးမျှဝေကြသည်
  လမ်းကြောင်းများ (မုဒ်/ကျပန်း/သိုလှောင်မှုနှင့် အဆုံးအဖြတ်မရှိသော syscall ငြင်းပယ်ခြင်း)။

ဥပမာ- `fixtures/compute/manifest_compute_payments.json`။

## ဖုန်းခေါ်ဆိုမှုများ၊ တောင်းဆိုမှုများနှင့် ပြေစာများ

- အစီအစဉ်- `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`၊
  `ComputeMetering`၊ `ComputeOutcome` in
  `crates/iroha_data_model/src/compute/mod.rs`။
- `ComputeRequest::hash()` သည် canonical request hash ကိုထုတ်ပေးသည် (ခေါင်းစီးများကိုသိမ်းဆည်းထားသည်။
  အဆုံးအဖြတ်ပေးသော `BTreeMap` နှင့် payload ကို `payload_hash` အဖြစ် သယ်ဆောင်ပါသည်။
- `ComputeCall` သည် namespace/route၊ codec၊ TTL/gas/response cap၊
  အရင်းအမြစ်ပရိုဖိုင် + စျေးနှုန်းမိသားစု၊ အထောက်အထား (`Public` သို့မဟုတ် UAID-ဘောင်းဘီ
  `ComputeAuthn`)၊ အဆုံးအဖြတ်ပေးခြင်း (`Strict` vs `BestEffort`)၊ စီရင်ချက်အတန်း
  အရိပ်အမြွက်များ (CPU/GPU/TEE)၊ SoraFS ထည့်သွင်းကြေငြာထားသော bytes/chunks၊ ရွေးချယ်နိုင်သော ပံ့ပိုးကူညီသူ
  ဘတ်ဂျက်၊ နှင့် canonical တောင်းဆိုစာအိတ်။ တောင်းဆိုချက် hash ကို အသုံးပြုသည်။
  replay အကာအကွယ်နှင့် routing ။
- လမ်းကြောင်းများသည် ရွေးချယ်နိုင်သော SoraFS မော်ဒယ်အကိုးအကားများနှင့် ထည့်သွင်းမှုကန့်သတ်ချက်များကို ထည့်သွင်းနိုင်သည်
  (inline/အတုံးအခဲထုပ်များ); သဲပုံးစည်းမျဉ်းများ ဂိတ်ပေါက် GPU/TEE အရိပ်အမြွက်များ။
- `ComputePriceWeights::charge_units` သည် မီတာတိုင်းတာခြင်းဒေတာကို ငွေပေးချေထားသော ကွန်ပျူတာအဖြစ်သို့ ပြောင်းပေးသည်။
  သံသရာနှင့် egress bytes များပေါ်တွင် ceil-division မှတဆင့်ယူနစ်များ။
- `ComputeOutcome` အစီရင်ခံစာများ `Success`, `Timeout`, `OutOfMemory`၊
  `BudgetExhausted`၊ သို့မဟုတ် `InternalError` နှင့် ရွေးချယ်နိုင်သော တုံ့ပြန်မှု hashe များ ပါဝင်သည်/
  စာရင်းစစ်အတွက် အရွယ်အစား/ကုဒ်ဒက်။

ဥပမာများ-
- ခေါ်ဆိုရန်- `fixtures/compute/call_compute_payments.json`
ပြေစာ- `fixtures/compute/receipt_compute_payments.json`

## Sandbox နှင့် အရင်းအမြစ်ပရိုဖိုင်များ- `ComputeSandboxRules` သည် ပုံမှန်အားဖြင့် execution mode ကို `IvmOnly` သို့ လော့ခ်ချသည်၊
  တောင်းဆိုချက် hash မှ အဆုံးအဖြတ်ကျပန်းကျပန်းမျိုးစေ့များ၊ ဖတ်ရန်သာ SoraFS ခွင့်ပြုသည်
  ဝင်ရောက်ကြည့်ရှုပြီး အဆုံးအဖြတ်မရှိသော syscalls များကို ငြင်းပယ်သည်။ GPU/TEE အရိပ်အမြွက်များကို ကန့်သတ်ထားသည်။
  အဆုံးအဖြတ်အတိုင်း ဆက်လက်လုပ်ဆောင်ရန် `allow_gpu_hints`/`allow_tee_hints`။
- `ComputeResourceBudget` သည် per-profile caps များကို cycles၊ linear memory၊ stack တွင် သတ်မှတ်သည်
  အရွယ်အစား၊ IO ဘတ်ဂျက်နှင့် ထုတ်ယူမှုများ၊ GPU အရိပ်အမြွက်များနှင့် WASI-lite အကူအညီများအတွက် ခလုတ်များ။
- ပုံမှန်အားဖြင့် ပရိုဖိုင်နှစ်ခု (`cpu-small`၊ `cpu-balanced`) အောက်တွင်
  အဆုံးအဖြတ်ပေးသော ဆုတ်ယုတ်မှုများနှင့်အတူ `defaults::compute::resource_profiles`။

## စျေးနှုန်းနှင့်ငွေပေးချေမှုယူနစ်

- ဈေးနှုန်းမိသားစုများ (`ComputePriceWeights`) မြေပုံစက်ဝန်းများနှင့် တွက်ချက်မှုသို့ egress bytes
  ယူနစ်များ; ပုံမှန်အားဖြင့် `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` ဖြင့် ကောက်ခံပါသည်။
  `unit_label = "cu"`။ မိသားစုများကို မန်နီးဖက်စ်များတွင် `price_family` ဖြင့် သော့ခတ်ထားသည်။
  ဝင်ခွင့်တွင် ပြဋ္ဌာန်းထားသည်။
- တိုင်းတာခြင်းမှတ်တမ်းများသည် `charged_units` နှင့် ကုန်ကြမ်းစက်ဝန်း/အဝင်/အထွက်/ကြာချိန်တို့ကို သယ်ဆောင်သည်။
  ပြန်လည်သင့်မြတ်ရေး အတွက် စုစုပေါင်း အခကြေးငွေများကို ကွပ်မျက်မှု အတန်းအစားနှင့် ချဲ့ထွင်သည်။
  အဆုံးအဖြတ်ပေးသော မြှောက်ကိန်းများ (`ComputePriceAmplifiers`) ဖြင့် ကန့်သတ်ထားသည်။
  `compute.economics.max_cu_per_call`; egress က ချုပ်ထားတယ်။
  တုံ့ပြန်မှုချဲ့ထွင်ခြင်းအတွက် `compute.economics.max_amplification_ratio`။
- စပွန်ဆာဘတ်ဂျက်များ (`ComputeCall::sponsor_budget_cu`) ကို ဆန့်ကျင်ရန် ပြဋ္ဌာန်းထားသည်။
  ဖုန်းခေါ်ဆိုမှု/နေ့စဉ်ထုပ်ပိုးမှုများ၊ ကောက်ခံထားသော ယူနစ်များသည် ကြေညာထားသော စပွန်ဆာဘတ်ဂျက်ထက် မကျော်လွန်ရပါ။
- အုပ်ချုပ်မှုစျေးနှုန်းမွမ်းမံမှုများသည် စွန့်စားရသည့်လူတန်းစားအပိုင်းများကို အသုံးပြုသည်။
  `compute.economics.price_bounds` နှင့် မှတ်တမ်းတင်ထားသော အခြေခံမိသားစုများ
  `compute.economics.price_family_baseline`; အသုံးပြု
  မွမ်းမံမွမ်းမံခြင်းမပြုမီ deltas ကိုအတည်ပြုရန် `ComputeEconomics::apply_price_update`
  အသက်ဝင်သော မိသားစုမြေပုံ။ Torii config အပ်ဒိတ်များကို အသုံးပြုပါ။
  `ConfigUpdate::ComputePricing` နှင့် kiso သည် ၎င်းကို တူညီသော ဘောင်များဖြင့် အသုံးချသည်။
  အုပ်ချုပ်မှု တည်းဖြတ်မှု အဆုံးအဖြတ် ထားရှိပါ။

## ဖွဲ့စည်းမှု

အသစ်သော တွက်ချက်မှုပုံစံသည် `crates/iroha_config/src/parameters` တွင်ရှိသည်-

- အသုံးပြုသူမြင်ကွင်း- `Compute` (`user.rs`) env overrides များ-
  - `COMPUTE_ENABLED` (မူရင်း `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- ဈေးနှုန်း/ဘောဂဗေဒ- `compute.economics` ရိုက်ကူးမှုများ
  `max_cu_per_call`/`max_amplification_ratio`၊ အခကြေးငွေခွဲပြီး၊ စပွန်ဆာထုပ်များ
  (ခေါ်ဆိုမှုတစ်ခုနှင့်နေ့စဉ် CU)၊ စျေးနှုန်းမိသားစုအခြေခံစည်းမျဉ်းများ + အန္တရာယ်အတန်းများ/ကန့်သတ်ချက်များ
  အုပ်ချုပ်မှုဆိုင်ရာ အပ်ဒိတ်များ၊ နှင့် အကောင်အထည်ဖော်မှုအဆင့် မြှောက်ကိန်းများ (GPU/TEE/ အကောင်းဆုံး-အားထုတ်မှု)။
- အမှန်တကယ်/မူရင်းများ- `actual.rs` / `defaults.rs::compute` ခွဲခြမ်းစိတ်ဖြာပြီး ဖော်ထုတ်ထားသည်
  `Compute` ဆက်တင်များ (namespaces၊ ပရိုဖိုင်များ၊ ဈေးနှုန်း မိသားစုများ၊ sandbox)။
- မမှန်ကန်သော configs (အမည်ကွက်လပ်များ၊ မူရင်းပရိုဖိုင်/မိသားစု ပျောက်ဆုံးနေသော၊ TTL ထုပ်
  ပြောင်းပြန်လှန်မှုများ) ကို ခွဲခြမ်းစိတ်ဖြာနေစဉ်အတွင်း `InvalidComputeConfig` အဖြစ် ပေါ်နေပါသည်။

## စာမေးပွဲများနှင့် ပွဲစဉ်များ

- Deterministic helpers (`request_hash`၊ စျေးနှုန်း) နှင့် fixture အသွားအပြန်များ တိုက်ရိုက်နေထိုင်သည်
  `crates/iroha_data_model/src/compute/mod.rs` (`fixtures_round_trip` ကိုကြည့်ပါ၊
  `request_hash_is_stable`၊ `pricing_rounds_up_units`)။
- JSON ပစ္စည်းများသည် `fixtures/compute/` တွင်နေထိုင်ပြီး data-model ဖြင့်အသုံးပြုသည်
  ဆုတ်ယုတ်မှုလွှမ်းခြုံမှုအတွက် စမ်းသပ်မှုများ။

## SLO ကြိုးဝိုင်းနှင့် ဘတ်ဂျက်များ- `compute.slo.*` ဖွဲ့စည်းမှုပုံစံသည် ဂိတ်ဝေး SLO ခလုတ်များ (လေယာဉ်ခရီးစဉ်အတွင်း တန်းစီခြင်း
  အတိမ်အနက်၊ RPS အထုပ်နှင့် latency ပစ်မှတ်များ) တွင်
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`။ ပုံသေများ- ၃၂
  ပျံသန်းမှုတွင်၊ လမ်းကြောင်းတစ်ခုလျှင် 512 စီစီ၊ 200 RPS၊ p50 25ms၊ p95 75ms၊ p99 120ms။
- SLO အနှစ်ချုပ်များနှင့် တောင်းဆိုချက်/ထွက်ခြင်းကို ဖမ်းယူရန် ပေါ့ပါးသော ခုံတန်းရှည်ကြိုးကို အသုံးပြုပါ။
  လျှပ်တစ်ပြက်- `ကုန်တင်ကုန်ချ -p xtask --bin compute_gateway -- ခုံတန်းလျား [manifest_path]
  [iterations] [concurrency] [out_dir]` (defaults: `fixtures/compute/manifest_compute_payments.json`၊
  128 ထပ်တူကျသည်၊ တူညီသော 16၊ အထွက်များအောက်တွင်
  `artifacts/compute_gateway/bench_summary.{json,md}`)။ ခုံတန်းလျားကို အသုံးပြုသည်။
  အဆုံးအဖြတ်ပေးသော ဝန်တင်များ (`fixtures/compute/payload_compute_payments.json`) နှင့်
  လေ့ကျင့်ခန်းလုပ်နေစဉ်တွင် ထပ်ကာထပ်ကာ တိုက်မိခြင်းများကို ရှောင်ရှားရန် တောင်းဆိုချက်တစ်ခုချင်းစီ
  `echo`/`uppercase`/`sha3` ဝင်ခွင့်အမှတ်များ။

## SDK/CLI တူညီသော အတွဲများ

- Canonical fixtures များသည် `fixtures/compute/` အောက်တွင် နေထိုင်သည်- manifest၊ call၊ payload နှင့်
  ဂိတ်ဝေးပုံစံ တုံ့ပြန်မှု/ပြေစာ အပြင်အဆင်။ Payload hash များသည် ခေါ်ဆိုမှုနှင့် ကိုက်ညီရပါမည်။
  `request.payload_hash`; အထောက်အကူပေးသော ဝန်ထုပ်ဝန်ပိုးသည် နေထိုင်သည်။
  `fixtures/compute/payload_compute_payments.json`။
- CLI သည် `iroha compute simulate` နှင့် `iroha compute invoke` ကို ပို့ဆောင်သည်-

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` တိုက်ရိုက်နေထိုင်သည်
  `javascript/iroha_js/src/compute.js` အောက်ရှိ ဆုတ်ယုတ်မှုစမ်းသပ်မှုများ
  `javascript/iroha_js/test/computeExamples.test.js`။
- Swift- `ComputeSimulator` သည် တူညီသော ပစ္စည်များကို တင်ကာ payload hashe များကို တရားဝင်စေသည်၊
  နှင့် entrypoints များကို စစ်ဆေးမှုများဖြင့် အတုယူပါ။
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`။
- CLI/JS/Swift အကူအညီပေးသူများသည် တူညီသော Norito ပစ္စည်းများကို SDKs လုပ်နိုင်စေရန် မျှဝေပါသည်။
  တောင်းဆိုမှုတည်ဆောက်မှုကို တရားဝင်အောင်ပြုလုပ်ပြီး hash ကို မထိဘဲ အော့ဖ်လိုင်းတွင် ကိုင်တွယ်ပါ။
  ပြေးတံခါးပေါက်။